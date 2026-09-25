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
use crate::jit::ir::{BlockId, FrameState, Func, GuardKind, Op, Ty, ValueDef, ValueId};

pub fn run(f: &mut Func) {
  numbers(f);
  bools(f);
}

/// Tagged parameters that only ever carry numbers become `F64`s.
fn numbers(f: &mut Func) {
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
            Source::Tagged if checked_in_body(f, p) && jumps_only_to(f, pred, b) => {},
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
    let boxed = f
      .insert(b, 0, Op::BoxF64, vec![p], Some(Ty::Tagged), None)
      .unwrap();
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
            f.insert(pred, pos, Op::ConstF64(x), vec![], Some(Ty::F64), None)
              .unwrap()
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

/// Tagged parameters every incoming value of which is a boxed boolean or a
/// boolean constant carry the boolean itself. That is what `and` and `or`
/// leave at their join, so a condition built from them branches on the
/// comparison rather than boxing it and testing the box. A test of such a
/// parameter for being falsy becomes a negation.
fn bools(f: &mut Func) {
  let preds = f.predecessors();
  let roots = f.roots();
  let mut candidates: FxHashSet<ValueId> = FxHashSet::default();
  for (bi, b) in f.blocks.iter().enumerate() {
    if roots.contains(&BlockId(bi as u32)) || preds[bi].is_empty() {
      continue;
    }
    candidates.extend(b.params.iter().copied().filter(|&p| f.ty(p) == Ty::Tagged));
  }
  loop {
    let mut dropped = Vec::new();
    for &p in &candidates {
      let (b, k) = param_position(f, p);
      let mut carried = false;
      for &pred in &preds[b.0 as usize] {
        for args in incoming(f, pred, b) {
          let v = args[k];
          match bool_source(f, v) {
            Some(BoolSource::Boxed | BoolSource::Const(_)) => carried = true,
            Some(BoolSource::Param) if v == p || candidates.contains(&v) => {},
            _ => dropped.push(p),
          }
        }
      }
      if !carried {
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

  let mut chosen: Vec<ValueId> = candidates.into_iter().collect();
  chosen.sort();
  for &p in &chosen {
    f.values[p.0 as usize].ty = Ty::Bool;
    let tests: Vec<usize> = (0..f.insts.len())
      .filter(|&i| matches!(f.insts[i].op, Op::IsFalsey) && f.insts[i].args[0] == p)
      .collect();
    let (b, _) = param_position(f, p);
    let boxed = f
      .insert(b, 0, Op::BoxBool, vec![p], Some(Ty::Tagged), None)
      .unwrap();
    replace_uses(f, p, boxed);
    let box_inst = f.def_inst_id(boxed).unwrap();
    f.insts[box_inst.0 as usize].args[0] = p;
    for i in tests {
      f.insts[i].op = Op::BNot;
      f.insts[i].args[0] = p;
    }
  }
  for &p in &chosen {
    let (b, k) = param_position(f, p);
    let mut seen = Vec::new();
    for &pred in &preds[b.0 as usize] {
      if seen.contains(&pred) {
        continue;
      }
      seen.push(pred);
      let edge_count = incoming(f, pred, b).len();
      for e in 0..edge_count {
        let v = incoming(f, pred, b)[e][k];
        let unboxed = match bool_source(f, v) {
          Some(BoolSource::Boxed) => f.def_inst(v).unwrap().args[0],
          Some(BoolSource::Const(c)) => {
            let pos = f.block(pred).insts.len();
            f.insert(pred, pos, Op::ConstBool(c), vec![], Some(Ty::Bool), None)
              .unwrap()
          },
          _ => v,
        };
        f.edge_args_mut(pred, b)[e][k] = unboxed;
      }
    }
  }
}

enum BoolSource {
  /// A `BoxBool`.
  Boxed,
  /// A tagged `true` or `false`.
  Const(bool),
  /// A block parameter.
  Param,
}

fn bool_source(f: &Func, v: ValueId) -> Option<BoolSource> {
  match f.values[v.0 as usize].def {
    ValueDef::Param(..) => Some(BoolSource::Param),
    ValueDef::Inst(i) => match &f.inst(i).op {
      Op::BoxBool => Some(BoolSource::Boxed),
      Op::ConstTagged(bits) => {
        let value = crate::vm::value::Value::from_bits(*bits);
        value.is_bool().then(|| BoolSource::Const(value.as_bool()))
      },
      _ => None,
    },
  }
}

/// `F64` parameters every incoming value of which is an integer made a
/// double, or a whole-number constant, carry the `I64`. A counter stepped
/// somewhere induction does not look, such as `i--` inside a loop's
/// condition, then goes round the loop as the integer instead of being
/// checked back into one every time. Runs after induction, which turns the
/// ordinary counters into integers with cheaper steps of its own.
pub fn integers(f: &mut Func) {
  let preds = f.predecessors();
  let roots = f.roots();
  let mut candidates: FxHashSet<ValueId> = FxHashSet::default();
  for (bi, b) in f.blocks.iter().enumerate() {
    if roots.contains(&BlockId(bi as u32)) || preds[bi].is_empty() {
      continue;
    }
    candidates.extend(b.params.iter().copied().filter(|&p| f.ty(p) == Ty::F64));
  }
  loop {
    let mut dropped = Vec::new();
    // A parameter fed only by whole constants and other such parameters
    // is as much an integer as one fed a converted integer directly: a
    // counter's header receives its start and the join after its step.
    for &p in &candidates {
      let (b, k) = param_position(f, p);
      for &pred in &preds[b.0 as usize] {
        for args in incoming(f, pred, b) {
          let v = args[k];
          match integer_source(f, v) {
            Some(IntSource::Converted(_) | IntSource::Const(_)) => {},
            Some(IntSource::Param) if v == p || candidates.contains(&v) => {},
            _ => dropped.push(p),
          }
        }
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

  let mut chosen: Vec<ValueId> = candidates.into_iter().collect();
  chosen.sort();
  for &p in &chosen {
    f.values[p.0 as usize].ty = Ty::I64;
    let (b, _) = param_position(f, p);
    let double = f
      .insert(b, 0, Op::IntToF64, vec![p], Some(Ty::F64), None)
      .unwrap();
    replace_uses(f, p, double);
    let conv = f.def_inst_id(double).unwrap();
    f.insts[conv.0 as usize].args[0] = p;
  }
  for &p in &chosen {
    let (b, k) = param_position(f, p);
    let mut seen = Vec::new();
    for &pred in &preds[b.0 as usize] {
      if seen.contains(&pred) {
        continue;
      }
      seen.push(pred);
      let edge_count = incoming(f, pred, b).len();
      for e in 0..edge_count {
        let v = incoming(f, pred, b)[e][k];
        let integer = match integer_source(f, v) {
          Some(IntSource::Converted(x)) => x,
          Some(IntSource::Const(c)) => {
            let pos = f.block(pred).insts.len();
            f.insert(pred, pos, Op::ConstI64(c), vec![], Some(Ty::I64), None)
              .unwrap()
          },
          _ => v,
        };
        f.edge_args_mut(pred, b)[e][k] = integer;
      }
    }
  }
}

enum IntSource {
  /// An `IntToF64` of this integer.
  Converted(ValueId),
  /// A whole-number constant within 2^53, and not -0.
  Const(i64),
  /// A block parameter.
  Param,
}

fn integer_source(f: &Func, v: ValueId) -> Option<IntSource> {
  match f.values[v.0 as usize].def {
    ValueDef::Param(..) => Some(IntSource::Param),
    ValueDef::Inst(i) => match &f.inst(i).op {
      Op::IntToF64 => Some(IntSource::Converted(f.inst(i).args[0])),
      Op::ConstF64(c) => {
        let whole = c.fract() == 0.0 && c.abs() <= (1u64 << 53) as f64;
        (whole && !(*c == 0.0 && c.is_sign_negative())).then(|| IntSource::Const(*c as i64))
      },
      _ => None,
    },
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

/// Whether the function checks `p` is a number somewhere, the bet its
/// feedback took at that site. A value of unknown kind coming into `p`'s
/// block can then be checked on the way in instead, deoptimizing at the
/// block, which resumes the interpreter at a point it can always go on
/// from.
fn checked_in_body(f: &Func, p: ValueId) -> bool {
  f.blocks.iter().any(|b| {
    b.insts.iter().any(|&i| {
      let inst = f.inst(i);
      matches!(inst.op, Op::Guard(GuardKind::Number)) && inst.args[0] == p
    })
  })
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
      let k = f
        .block(b)
        .params
        .iter()
        .position(|&x| x == p)
        .unwrap_or(k as usize);
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
