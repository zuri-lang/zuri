//! What is known about `I64` values without running anything: the range
//! an operation's result must fall in, and which integers a double holds
//! exactly.
//!
//! An `I64` stands for the double the interpreter would hold, so the two
//! can be traded freely only when converting one to the other loses
//! nothing. That holds for anything within 2^53 of zero, and for anything
//! a whole-number check took out of a double in the first place.

use rustc_hash::FxHashSet;

use crate::jit::ir::{BitOp, BlockId, Func, GuardKind, Op, Terminator, Ty, ValueId};

/// The largest magnitude up to which a double counts every integer.
pub const EXACT: i64 = 1 << 53;

/// The bounds an `I64` value is known to lie within, from what defines
/// it. Block parameters are unknown.
pub fn range(f: &Func, v: ValueId) -> Option<(i64, i64)> {
  range_at(f, v, 4)
}

fn range_at(f: &Func, v: ValueId, depth: u8) -> Option<(i64, i64)> {
  let inst = f.def_inst(v)?;
  match inst.op {
    Op::ConstI64(c) => Some((c, c)),
    Op::BytesLoad | Op::StrByte => Some((0, 255)),
    Op::ListLen | Op::BytesLen | Op::StrByteLen | Op::StrLength | Op::ObjLength => Some((0, EXACT)),
    Op::Guard(GuardKind::Arith(_)) => Some((-EXACT, EXACT - 1)),
    Op::IBit(BitOp::Ushr) => Some((0, u32::MAX as i64)),
    // A shift keeping the sign moves towards zero, or towards -1 from
    // below it.
    Op::IBit(BitOp::Shr) if depth > 0 => {
      let (lo, hi) = range_at(f, inst.args[0], depth - 1)?;
      Some((lo.min(0), hi.max(0)))
    },
    // Anding with something never negative keeps no more than its bits.
    Op::IBit(BitOp::And) if depth > 0 => {
      let a = range_at(f, inst.args[0], depth - 1).filter(|r| r.0 >= 0);
      let b = range_at(f, inst.args[1], depth - 1).filter(|r| r.0 >= 0);
      match (a, b) {
        (Some(a), Some(b)) => Some((0, a.1.min(b.1))),
        (Some(r), None) | (None, Some(r)) => Some((0, r.1)),
        (None, None) => None,
      }
    },
    _ => None,
  }
}

/// `I64` values whose double is exact: within 2^53 of zero by their
/// range, taken out of a double by a whole-number check, or block
/// parameters that only ever receive such values.
pub fn exact_integers(f: &Func) -> FxHashSet<ValueId> {
  with_params(f, |f, inst, v| {
    matches!(inst.op, Op::Guard(GuardKind::Int | GuardKind::Whole)) || small(f, v)
  })
}

/// `I64` values within 2^53 of zero: by their range, or as block
/// parameters that only ever receive such values.
pub fn small_integers(f: &Func) -> FxHashSet<ValueId> {
  with_params(f, |f, _, v| small(f, v))
}

fn small(f: &Func, v: ValueId) -> bool {
  range(f, v).is_some_and(|(lo, hi)| lo >= -EXACT && hi <= EXACT)
}

/// The `I64` results `holds` picks out, and the block parameters that
/// only ever receive them.
fn with_params(
  f: &Func,
  holds: impl Fn(&Func, &crate::jit::ir::Inst, ValueId) -> bool,
) -> FxHashSet<ValueId> {
  let mut out: FxHashSet<ValueId> = FxHashSet::default();
  for b in &f.blocks {
    for &i in &b.insts {
      let inst = f.inst(i);
      if let Some(v) = inst.result
        && f.ty(v) == Ty::I64
        && holds(f, inst, v)
      {
        out.insert(v);
      }
    }
  }

  // Parameters: assume every `I64` one qualifies and drop those with an
  // incoming value that does not, until nothing changes.
  let mut params: FxHashSet<ValueId> = f
    .blocks
    .iter()
    .flat_map(|b| b.params.iter().copied())
    .filter(|&p| f.ty(p) == Ty::I64)
    .collect();
  let incoming = incoming_pairs(f);
  // A parameter with no incoming edge at all is an entry's, and unknown.
  let fed: FxHashSet<ValueId> = incoming.iter().map(|&(p, _)| p).collect();
  params.retain(|p| fed.contains(p));
  loop {
    let bad: Vec<ValueId> = incoming
      .iter()
      .filter(|&&(p, a)| params.contains(&p) && !out.contains(&a) && !params.contains(&a))
      .map(|&(p, _)| p)
      .collect();
    if bad.is_empty() {
      break;
    }
    for p in bad {
      params.remove(&p);
    }
  }
  out.extend(params);
  out
}

/// Every (parameter, value passed to it) pair over every edge.
fn incoming_pairs(f: &Func) -> Vec<(ValueId, ValueId)> {
  let mut out = Vec::new();
  for b in &f.blocks {
    let mut edge = |target: BlockId, args: &[ValueId]| {
      for (k, &a) in args.iter().enumerate() {
        if let Some(&p) = f.block(target).params.get(k) {
          out.push((p, a));
        }
      }
    };
    match &b.term {
      Terminator::Jump { target, args } => edge(*target, args),
      Terminator::Branch {
        then_block,
        then_args,
        else_block,
        else_args,
        ..
      } => {
        edge(*then_block, then_args);
        edge(*else_block, else_args);
      },
      _ => {},
    }
  }
  out
}
