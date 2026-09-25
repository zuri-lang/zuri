//! Local rewrites that need nothing but the operation and what defines
//! its operands: boxing that is undone straight away, a guard on a value
//! whose construction already proves it, a double negation, a branch on a
//! negated condition.
//!
//! Frame states also name the unboxed value behind a box rather than the
//! box itself. Leaving compiled code boxes every state value on the way
//! out anyway, so a box that only a frame state used no longer runs on
//! the fast path at all.

use rustc_hash::FxHashSet;

use super::replace_uses;
use crate::jit::ir::{Func, GuardKind, Op, Terminator, Ty, ValueId};

pub fn run(f: &mut Func) {
  unbox_states(f);
  let small = small_integers(f);
  loop {
    let mut changed = false;
    for bi in 0..f.blocks.len() {
      // Whatever folds is pure and has nothing left using it, so it goes
      // from the block there and then.
      let insts = std::mem::take(&mut f.blocks[bi].insts);
      let mut keep = Vec::with_capacity(insts.len());
      for id in insts {
        match fold(f, id, &small) {
          Some((from, to)) => {
            replace_uses(f, from, to);
            changed = true;
          },
          None => keep.push(id),
        }
      }
      f.blocks[bi].insts = keep;
      changed |= branch_on_not(f, bi);
    }
    if !changed {
      return;
    }
  }
}

fn unbox_states(f: &mut Func) {
  // An integer converted for the box goes too: leaving compiled code
  // converts an `I64` state value the same way.
  let unboxed = |f: &Func, v: ValueId| {
    let v = match f.def_inst(v) {
      Some(d) if matches!(d.op, Op::BoxF64 | Op::BoxBool) => d.args[0],
      _ => v,
    };
    match f.def_inst(v) {
      Some(d) if matches!(d.op, Op::IntToF64) => d.args[0],
      _ => v,
    }
  };
  for i in 0..f.insts.len() {
    let Some(state) = &f.insts[i].state else {
      continue;
    };
    let regs: Vec<(u8, ValueId)> = state
      .regs
      .iter()
      .map(|&(r, v)| (r, unboxed(f, v)))
      .collect();
    f.insts[i].state.as_mut().unwrap().regs = regs;
  }
  for b in 0..f.blocks.len() {
    let Terminator::Deopt(state) = &f.blocks[b].term else {
      continue;
    };
    let regs: Vec<(u8, ValueId)> = state
      .regs
      .iter()
      .map(|&(r, v)| (r, unboxed(f, v)))
      .collect();
    if let Terminator::Deopt(state) = &mut f.blocks[b].term {
      state.regs = regs;
    }
  }
}

/// `I64` values known to lie within 2^53 of zero, which a double holds
/// exactly: checked integer arithmetic, small constants, list lengths, and
/// block parameters that only ever receive such values.
fn small_integers(f: &Func) -> FxHashSet<ValueId> {
  let within = |c: i64| c.unsigned_abs() <= 1 << 53;
  let mut small: FxHashSet<ValueId> = FxHashSet::default();
  for b in &f.blocks {
    for &i in &b.insts {
      let inst = f.inst(i);
      let Some(v) = inst.result else {
        continue;
      };
      let is_small = match inst.op {
        Op::Guard(GuardKind::Arith(_)) | Op::ListLen => true,
        Op::ConstI64(c) => within(c),
        _ => false,
      };
      if is_small {
        small.insert(v);
      }
    }
  }
  // Parameters: assume every `I64` one is small and drop those with an
  // incoming value that is not, until nothing changes.
  let mut params: FxHashSet<ValueId> = f
    .blocks
    .iter()
    .flat_map(|b| b.params.iter().copied())
    .filter(|&p| f.ty(p) == Ty::I64)
    .collect();
  let mut incoming: Vec<(ValueId, ValueId)> = Vec::new();
  for b in &f.blocks {
    let mut edge = |target: crate::jit::ir::BlockId, args: &[ValueId]| {
      for (k, &a) in args.iter().enumerate() {
        if let Some(&p) = f.block(target).params.get(k) {
          incoming.push((p, a));
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
  // A parameter with no incoming edge at all is an entry's, and unknown.
  let fed: FxHashSet<ValueId> = incoming.iter().map(|&(p, _)| p).collect();
  params.retain(|p| fed.contains(p));
  loop {
    let bad: Vec<ValueId> = incoming
      .iter()
      .filter(|&&(p, a)| params.contains(&p) && !small.contains(&a) && !params.contains(&a))
      .map(|&(p, _)| p)
      .collect();
    if bad.is_empty() {
      break;
    }
    for p in bad {
      params.remove(&p);
    }
  }
  small.extend(params);
  small
}

/// The value an operation can be replaced by, when it is one it already
/// has.
fn fold(
  f: &Func,
  id: crate::jit::ir::InstId,
  small: &FxHashSet<ValueId>,
) -> Option<(ValueId, ValueId)> {
  let inst = f.inst(id);
  let result = inst.result?;
  let arg = *inst.args.first()?;
  let def = f.def_inst(arg).map(|d| (&d.op, d.args.first().copied()));
  let replacement = match (&inst.op, def) {
    // Unboxing what was just boxed, or guarding a number that was built
    // as one.
    (Op::UnboxF64 | Op::Guard(GuardKind::Number), Some((Op::BoxF64, Some(x)))) => x,
    (Op::UnboxBool | Op::Guard(GuardKind::Bool), Some((Op::BoxBool, Some(x)))) => x,
    (Op::BNot, Some((Op::BNot, Some(x)))) => x,
    // A whole-number check on a small integer made a double, boxed or not.
    (Op::Guard(GuardKind::Whole), Some((Op::IntToF64, Some(x)))) if small.contains(&x) => x,
    (Op::Guard(GuardKind::Whole), Some((Op::BoxF64, Some(y)))) => {
      let d = f.def_inst(y)?;
      if !matches!(d.op, Op::IntToF64) || !small.contains(&d.args[0]) {
        return None;
      }
      d.args[0]
    },
    _ => return None,
  };
  (replacement != result).then_some((result, replacement))
}

/// `branch(not c) a b` is `branch(c) b a`.
fn branch_on_not(f: &mut Func, bi: usize) -> bool {
  let Terminator::Branch { cond, .. } = &f.blocks[bi].term else {
    return false;
  };
  let Some(def) = f.def_inst(*cond) else {
    return false;
  };
  if !matches!(def.op, Op::BNot) {
    return false;
  }
  let inner = def.args[0];
  let Terminator::Branch {
    cond,
    then_block,
    then_args,
    else_block,
    else_args,
  } = std::mem::replace(&mut f.blocks[bi].term, Terminator::Unset)
  else {
    unreachable!()
  };
  let _ = cond;
  f.blocks[bi].term = Terminator::Branch {
    cond: inner,
    then_block: else_block,
    then_args: else_args,
    else_block: then_block,
    else_args: then_args,
  };
  true
}
