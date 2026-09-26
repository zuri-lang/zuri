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

use rustc_hash::{FxHashMap, FxHashSet};

use super::replace_uses;
use crate::jit::ir::build::Feedback;
use crate::jit::ir::{
  BlockId, FrameState, Func, GuardKind, InstId, Op, Terminator, Ty, ValueDef, ValueId,
};

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
    // Which candidates a number is boxed straight into, and which other
    // candidates feed each one.
    let mut carried: FxHashSet<ValueId> = FxHashSet::default();
    let mut fed_by: Vec<(ValueId, ValueId)> = Vec::new();
    for &p in &candidates {
      let (b, k) = param_position(f, p);
      // A value checked on the way in goes back to the interpreter at the
      // block, which needs a bytecode position to resume at.
      let resumable = f.block(b).ip.is_some();
      for &pred in &preds[b.0 as usize] {
        for args in incoming(f, pred, b) {
          let v = args[k];
          match numeric_source(f, v) {
            Source::Boxed | Source::Const(_) => {
              carried.insert(p);
            },
            Source::Param if v == p => {},
            Source::Param if candidates.contains(&v) => fed_by.push((p, v)),
            Source::Tagged if entry_of(f, &osr_blocks, v).is_some() => {},
            Source::Tagged if !resumable => dropped.push(p),
            Source::Tagged if osr_blocks.contains(&pred) => {},
            Source::Tagged if checked_on_entry(f, p, b) && jumps_only_to(f, pred, b) => {},
            Source::Tagged if checked_in_body(f, p) && jumps_only_to(f, pred, b) => {},
            _ => dropped.push(p),
          }
        }
      }
    }
    // Something has to actually carry a number in, straight or through
    // other such parameters, or this is only a value passed around a loop
    // untouched.
    loop {
      let before = carried.len();
      for &(p, from) in &fed_by {
        if carried.contains(&from) {
          carried.insert(p);
        }
      }
      if carried.len() == before {
        break;
      }
    }
    dropped.extend(candidates.iter().copied().filter(|p| !carried.contains(p)));
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

  // Unbox what flows into each retyped parameter. A value the frame held
  // when compiled code took over is checked once, in the entry that
  // received it, however many parameters it reaches.
  let mut from_frame: FxHashMap<ValueId, ValueId> = FxHashMap::default();
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
          Source::Tagged if entry_of(f, &osr_blocks, v).is_some() => match from_frame.get(&v) {
            Some(&n) => n,
            None => {
              let n = check_on_entry(f, entry_of(f, &osr_blocks, v).unwrap(), v);
              from_frame.insert(v, n);
              n
            },
          },
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

/// The on-stack-replacement entry that received `v` from the frame, when
/// it is one of the frame's values and the entry goes straight on to its
/// loop.
fn entry_of(f: &Func, osr_blocks: &FxHashSet<BlockId>, v: ValueId) -> Option<BlockId> {
  let id = f.def_inst_id(v)?;
  if !matches!(f.inst(id).op, Op::OsrParam(_)) {
    return None;
  }
  osr_blocks
    .iter()
    .copied()
    .find(|&b| f.block(b).insts.contains(&id) && matches!(f.block(b).term, Terminator::Jump { .. }))
}

/// Checks that `v` is a number at the end of `entry`, going back to the
/// interpreter at the loop the entry leads to if it is not.
fn check_on_entry(f: &mut Func, entry: BlockId, v: ValueId) -> ValueId {
  let Terminator::Jump { target, args } = f.block(entry).term.clone() else {
    unreachable!("an entry that goes straight on to its loop")
  };
  let state = FrameState {
    ip: f
      .block(target)
      .ip
      .expect("an entry leads to a bytecode block"),
    regs: f.entry_regs(target, &args),
    frame: f.block(target).frame,
    blame: None,
  };
  let pos = f.block(entry).insts.len();
  f.insert(
    entry,
    pos,
    Op::Guard(GuardKind::Number),
    vec![v],
    Some(Ty::F64),
    Some(state),
  )
  .unwrap()
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
///
/// A loop entered from a frame already running is handed whatever numbers
/// the interpreter holds. Where everything else reaching the parameter is
/// an integer a double holds exactly, the entry bets that the frame's
/// number is a whole one too, checking it on the way in; a loop that loses
/// the bet has its header blocked, and the next compile leaves it be.
pub fn integers(f: &mut Func, feedback: &Feedback) {
  let preds = f.predecessors();
  let roots = f.roots();
  let exact = super::ints::exact_integers(f);
  let mut candidates: FxHashSet<ValueId> = FxHashSet::default();
  for (bi, b) in f.blocks.iter().enumerate() {
    if roots.contains(&BlockId(bi as u32)) || preds[bi].is_empty() {
      continue;
    }
    candidates.extend(b.params.iter().copied().filter(|&p| f.ty(p) == Ty::F64));
  }
  // What a frame hands an entry: a number check on a value the frame
  // held when compiled code took over, with where the check sits.
  let entry_number = |f: &Func, v: ValueId| -> Option<(BlockId, usize)> {
    if feedback.sites_off {
      return None;
    }
    let d = f.def_inst(v)?;
    let state = d.state.as_ref()?;
    let (frame, ip) = state.site();
    let from_frame = f
      .def_inst(*d.args.first()?)
      .is_some_and(|a| matches!(a.op, Op::OsrParam(_)));
    let checked = matches!(d.op, Op::Guard(GuardKind::Number)) && from_frame;
    if !checked || frame != 0 || feedback.blocked.contains(&ip) {
      return None;
    }
    let id = f.def_inst_id(v)?;
    f.blocks.iter().enumerate().find_map(|(b, block)| {
      let pos = block.insts.iter().position(|&i| i == id)?;
      Some((BlockId(b as u32), pos))
    })
  };
  let wanted = integer_uses(f);
  loop {
    let mut dropped = Vec::new();
    let mut entered: Vec<ValueId> = Vec::new();
    let mut linked: Vec<(ValueId, ValueId)> = Vec::new();
    // A parameter fed only by whole constants and other such parameters
    // is as much an integer as one fed a converted integer directly: a
    // counter's header receives its start and the join after its step.
    for &p in &candidates {
      let (b, k) = param_position(f, p);
      let mut from_entry = false;
      let mut all_exact = true;
      for &pred in &preds[b.0 as usize] {
        for args in incoming(f, pred, b) {
          let v = args[k];
          match integer_source(f, v) {
            Some(IntSource::Converted(x)) => all_exact &= exact.contains(&x),
            Some(IntSource::Const(_)) => {},
            Some(IntSource::Param) if v == p => {},
            Some(IntSource::Param) if candidates.contains(&v) => linked.push((p, v)),
            _ if entry_number(f, v).is_some() => from_entry = true,
            _ => dropped.push(p),
          }
        }
      }
      // The bet on an entry only pays when the parameter then stays an
      // integer a double holds exactly.
      if from_entry && !all_exact {
        dropped.push(p);
      }
      if from_entry {
        entered.push(p);
      }
    }
    // Nor does it pay when the number only ever goes into arithmetic on
    // doubles: then the integer is converted back at every use. Some
    // parameter the number reaches has to be used as an integer.
    if !entered.is_empty() {
      let mut reach: FxHashSet<ValueId> = entered.iter().copied().collect();
      loop {
        let before = reach.len();
        for &(a, b) in &linked {
          if reach.contains(&a) || reach.contains(&b) {
            reach.insert(a);
            reach.insert(b);
          }
        }
        if reach.len() == before {
          break;
        }
      }
      if !reach.iter().any(|p| wanted.contains(p)) {
        dropped.extend(entered);
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
          _ => match entry_number(f, v) {
            Some((entry, pos)) => {
              let checked = f.def_inst(v).unwrap();
              let (tagged, state) = (checked.args[0], checked.state.clone());
              f.insert(
                entry,
                pos + 1,
                Op::Guard(GuardKind::Whole),
                vec![tagged],
                Some(Ty::I64),
                state,
              )
              .unwrap()
            },
            None => v,
          },
        };
        f.edge_args_mut(pred, b)[e][k] = integer;
      }
    }
  }
}

/// `F64` values used as integers somewhere: checked into one for an
/// index or a bitwise operator, or checked whole for integer arithmetic
/// whose result is itself used as more than a double. Looks through a
/// box.
fn integer_uses(f: &Func) -> FxHashSet<ValueId> {
  let mut users: FxHashMap<ValueId, Vec<InstId>> = FxHashMap::default();
  for b in &f.blocks {
    for &i in &b.insts {
      for &a in &f.inst(i).args {
        users.entry(a).or_default().push(i);
      }
    }
  }
  let used_as_more_than_double = |v: ValueId| {
    users.get(&v).is_some_and(|us| {
      us.iter().any(|&u| {
        !matches!(
          f.inst(u).op,
          Op::IntToF64 | Op::Guard(GuardKind::Arith(_)) | Op::BoxF64
        )
      })
    })
  };
  let as_integer = |v: ValueId| {
    users.get(&v).is_some_and(|us| {
      us.iter().any(|&u| {
        let inst = f.inst(u);
        match inst.op {
          Op::Guard(GuardKind::Int) | Op::WrapI64 => true,
          Op::Guard(GuardKind::Whole) => inst.result.is_some_and(|r| {
            used_as_more_than_double(r)
              || users.get(&r).is_some_and(|rs| {
                rs.iter().any(|&ru| {
                  let ri = f.inst(ru);
                  matches!(ri.op, Op::Guard(GuardKind::Arith(_)))
                    && ri.result.is_some_and(used_as_more_than_double)
                })
              })
          }),
          _ => false,
        }
      })
    })
  };
  let mut out = FxHashSet::default();
  for b in &f.blocks {
    for &p in &b.params {
      if f.ty(p) != Ty::F64 {
        continue;
      }
      let boxed = users.get(&p).into_iter().flatten().filter_map(|&u| {
        let inst = f.inst(u);
        matches!(inst.op, Op::BoxF64)
          .then_some(inst.result)
          .flatten()
      });
      if as_integer(p) || boxed.collect::<Vec<_>>().into_iter().any(as_integer) {
        out.insert(p);
      }
    }
  }
  out
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
    let checks_number = matches!(
      inst.op,
      Op::Guard(GuardKind::Number | GuardKind::Whole | GuardKind::Int)
    );
    if checks_number && inst.args[0] == p {
      return inst.state.as_ref().is_some_and(|s| Some(s.ip) == block.ip);
    }
    if inst.op.has_effect() {
      return false;
    }
  }
  false
}

/// Whether the function checks `p` is a number somewhere, whole or not,
/// the bet its feedback took at that site. A value of unknown kind coming
/// into `p`'s block can then be checked on the way in instead,
/// deoptimizing at the block, which resumes the interpreter at a point it
/// can always go on from.
fn checked_in_body(f: &Func, p: ValueId) -> bool {
  f.blocks.iter().any(|b| {
    b.insts.iter().any(|&i| {
      let inst = f.inst(i);
      matches!(
        inst.op,
        Op::Guard(GuardKind::Number | GuardKind::Whole | GuardKind::Int)
      ) && inst.args[0] == p
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
