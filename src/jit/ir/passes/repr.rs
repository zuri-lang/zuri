//! Unboxed block parameters.
//!
//! The builder passes every value between blocks tagged, so a loop that
//! adds to a running total boxes it at the bottom of every iteration and
//! guards it back into a number at the top of the next. A parameter whose
//! every incoming value is a boxed number, a number constant, or another
//! such parameter can carry the number itself instead; its tagged uses get
//! a box, which is a bitcast, and the guards on it fold away.
//!
//! An on-stack-replacement entry passes the parameter whatever the
//! interpreter left in the register, so that edge gets a number guard of
//! its own, deoptimizing at the loop header if the frame holds something
//! else. So does an ordinary edge carrying a value of unknown kind, such
//! as a list element, when the block checks the parameter is a number
//! before doing anything else: checking on the way in is the same check,
//! made once per entry instead of once per iteration.

use rustc_hash::FxHashSet;

use super::replace_uses;
use crate::jit::ir::{BlockId, FrameState, Func, GuardKind, Op, Ty, ValueId, ValueDef};

pub fn run(f: &mut Func) {
  let preds = f.predecessors();
  let roots = f.roots();
  let osr_blocks: FxHashSet<BlockId> = f.osr_entries.iter().map(|&(_, b)| b).collect();

  // Every tagged parameter of a block with predecessors starts as a
  // candidate; any whose inputs are not all numbers drops out, which can
  // drop others that depended on it, until nothing changes.
  let mut candidates: FxHashSet<ValueId> = FxHashSet::default();
  for (bi, b) in f.blocks.iter().enumerate() {
    if roots.contains(&BlockId(bi as u32)) || preds[bi].is_empty() {
      continue;
    }
    for &p in &b.params {
      if f.ty(p) == Ty::Tagged {
        candidates.insert(p);
      }
    }
  }

  loop {
    let mut dropped = Vec::new();
    for &p in &candidates {
      let (b, k) = param_position(f, p);
      let mut boxed_input = false;
      for &pred in &preds[b.0 as usize] {
        for args in incoming(f, pred, b) {
          let v = args[k];
          match numeric_source(f, v) {
            Source::Boxed | Source::Const(_) => boxed_input = true,
            Source::Param if v == p || candidates.contains(&v) => {},
            Source::Tagged if osr_blocks.contains(&pred) => {},
            Source::Tagged if checked_on_entry(f, p, b) && jumps_only_to(f, pred, b) => {},
            _ => {
              dropped.push(p);
            },
          }
        }
      }
      // Something has to actually carry a number in, or this is only a
      // value passed around a loop untouched.
      if !boxed_input {
        dropped.push(p);
      }
    }
    if dropped.is_empty() {
      break;
    }
    for p in dropped {
      candidates.remove(&p);
    }
  }

  if candidates.is_empty() {
    return;
  }

  // Retype the parameters, and give each block's tagged uses a box of
  // the new number.
  let mut chosen: Vec<ValueId> = candidates.into_iter().collect();
  chosen.sort();
  for &p in &chosen {
    f.values[p.0 as usize].ty = Ty::F64;
    let (b, _) = param_position(f, p);
    let boxed = f.insert(b, 0, Op::BoxF64, vec![p], Some(Ty::Tagged), None).unwrap();
    replace_uses(f, p, boxed);
    // `replace_uses` also rewrote the box's own operand.
    let box_inst = f.def_inst_id(boxed).unwrap();
    f.insts[box_inst.0 as usize].args[0] = p;
  }

  // Unbox what flows into each retyped parameter.
  for &p in &chosen {
    let (b, k) = param_position(f, p);
    let header_ip = f.block(b).ip;
    let mut seen = Vec::new();
    for &pred in &preds[b.0 as usize] {
      if seen.contains(&pred) {
        continue;
      }
      seen.push(pred);
      let edge_count = incoming(f, pred, b).len();
      for e in 0..edge_count {
        let args = incoming(f, pred, b)[e].clone();
        let v = args[k];
        let unboxed = match numeric_source(f, v) {
          Source::Boxed => f.def_inst(v).unwrap().args[0],
          Source::Const(x) => {
            let pos = f.block(pred).insts.len();
            f.insert(pred, pos, Op::ConstF64(x), vec![], Some(Ty::F64), None).unwrap()
          },
          Source::Param if f.ty(v) == Ty::F64 => v,
          _ => {
            // A value of unknown kind, from an on-stack-replacement entry
            // or one the block checks first thing: check it on the way in,
            // and go back to the interpreter at the block if it is not a
            // number.
            let state = FrameState {
              ip: header_ip.expect("a retyped parameter starts a bytecode block"),
              regs: f.entry_regs(b, &args),
              frame: f.block(b).frame,
              blame: None,
            };
            let pos = f.block(pred).insts.len();
            f.insert(
              pred,
              pos,
              Op::Guard(GuardKind::Number),
              vec![v],
              Some(Ty::F64),
              Some(state),
            )
            .unwrap()
          },
        };
        f.edge_args_mut(pred, b)[e][k] = unboxed;
      }
    }
  }
}

/// Whether `b` checks `p` is a number, at `b`'s own position, before it
/// does anything that could be observed.
fn checked_on_entry(f: &Func, p: ValueId, b: BlockId) -> bool {
  let block = f.block(b);
  for &i in &block.insts {
    let inst = f.inst(i);
    if matches!(inst.op, Op::Guard(GuardKind::Number)) && inst.args[0] == p {
      return inst.state.as_ref().is_some_and(|s| Some(s.ip) == block.ip);
    }
    if inst.op.has_effect() {
      return false;
    }
  }
  false
}

/// Whether `pred` reaches `b` by a plain jump, so a check placed at its
/// end runs only on the way into `b`.
fn jumps_only_to(f: &Func, pred: BlockId, b: BlockId) -> bool {
  matches!(f.block(pred).term, crate::jit::ir::Terminator::Jump { target, .. } if target == b)
}

enum Source {
  /// A `BoxF64`.
  Boxed,
  /// A tagged constant that is a number.
  Const(f64),
  /// A block parameter.
  Param,
  /// Some other tagged value.
  Tagged,
  /// Anything else, which is not a number.
  Other,
}

fn numeric_source(f: &Func, v: ValueId) -> Source {
  match f.values[v.0 as usize].def {
    ValueDef::Param(..) => Source::Param,
    ValueDef::Inst(i) => match &f.inst(i).op {
      Op::BoxF64 => Source::Boxed,
      Op::ConstTagged(bits) => {
        let value = crate::vm::value::Value::from_bits(*bits);
        if value.is_number() {
          Source::Const(value.as_number())
        } else {
          Source::Other
        }
      },
      _ if f.ty(v) == Ty::Tagged => Source::Tagged,
      _ => Source::Other,
    },
  }
}

fn param_position(f: &Func, p: ValueId) -> (BlockId, usize) {
  match f.values[p.0 as usize].def {
    ValueDef::Param(b, k) => {
      // Parameters can have been removed ahead of this one since it was
      // made, so find it by value rather than trusting the index.
      let k = f.block(b).params.iter().position(|&x| x == p).unwrap_or(k as usize);
      (b, k)
    },
    ValueDef::Inst(_) => unreachable!("not a block parameter"),
  }
}

fn incoming(f: &Func, from: BlockId, to: BlockId) -> Vec<Vec<ValueId>> {
  use crate::jit::ir::Terminator;
  match &f.block(from).term {
    Terminator::Jump { target, args } if *target == to => vec![args.clone()],
    Terminator::Branch {
      then_block,
      then_args,
      else_block,
      else_args,
      ..
    } => {
      let mut v = Vec::new();
      if *then_block == to {
        v.push(then_args.clone());
      }
      if *else_block == to {
        v.push(else_args.clone());
      }
      v
    },
    _ => Vec::new(),
  }
}
