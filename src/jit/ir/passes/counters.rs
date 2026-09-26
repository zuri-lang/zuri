//! Integer counters whose checked steps cannot fail.
//!
//! A loop parameter that starts from constants and comes back round as
//! itself plus or minus a constant, where the way back is only taken once
//! a comparison has bounded the new value, lies between known limits the
//! whole time the loop runs. `i-- > 0` counting down from 50 stays in
//! 0..50. When those limits keep every step well inside what a double
//! holds exactly, the step's checks for overflow and for leaving the
//! exact range are proven, and it becomes a plain integer operation.

use super::loops;
use crate::jit::ir::{
  BlockId, Cmp, Func, GuardKind, InstId, IntOp, Op, Terminator, ValueId, dominates, dominators,
};

/// How far from zero a counter's limits may lie for its steps to need no
/// check: every value it takes, a step either side included, then stays
/// well inside the integers a double holds exactly.
const LIMIT: i64 = 1 << 52;

pub fn run(f: &mut Func) -> bool {
  let order = f.reverse_postorder();
  let idom = dominators(f, &order);
  let preds = f.predecessors();
  let mut proven: Vec<InstId> = Vec::new();
  for lp in loops::find(f) {
    let header = lp.header;
    let outside: Vec<BlockId> = preds[header.0 as usize]
      .iter()
      .copied()
      .filter(|p| !lp.body.contains(p))
      .collect();
    for (k, &param) in f.block(header).params.clone().iter().enumerate() {
      if let Some(steps) = counter_steps(f, &idom, &preds, header, k, param, &outside, &lp.latches)
      {
        proven.extend(steps);
      }
    }
  }
  for &id in &proven {
    let op = match f.inst(id).op {
      Op::Guard(GuardKind::Arith(IntOp::Add)) => Op::IAdd,
      _ => Op::ISub,
    };
    let inst = &mut f.insts[id.0 as usize];
    inst.op = op;
    inst.state = None;
  }
  !proven.is_empty()
}

/// The checked steps of the counter `param`, the `k`th parameter of
/// `header`, when its limits prove them.
#[allow(clippy::too_many_arguments)]
fn counter_steps(
  f: &Func,
  idom: &[Option<BlockId>],
  preds: &[Vec<BlockId>],
  header: BlockId,
  k: usize,
  param: ValueId,
  outside: &[BlockId],
  latches: &[BlockId],
) -> Option<Vec<InstId>> {
  // Where it starts.
  let mut lo = i64::MAX;
  let mut hi = i64::MIN;
  for &p in outside {
    for v in edge_values(f, p, header, k) {
      let c = const_i64(f, v)?;
      lo = lo.min(c);
      hi = hi.max(c);
    }
  }
  if lo > hi {
    return None;
  }
  // How it comes back: every back edge brings a checked step of it by the
  // same kind of constant, taken only past a comparison that bounds the
  // stepped value on the side the loop continues.
  let mut steps = Vec::new();
  let mut down = None;
  for &latch in latches {
    for v in edge_values(f, latch, header, k) {
      let id = f.def_inst_id(v)?;
      let inst = f.inst(id);
      let sub = match inst.op {
        Op::Guard(GuardKind::Arith(IntOp::Sub)) => true,
        Op::Guard(GuardKind::Arith(IntOp::Add)) => false,
        _ => return None,
      };
      if inst.args[0] != param {
        return None;
      }
      let step = const_i64(f, inst.args[1])?;
      if !(1..=LIMIT).contains(&step) {
        return None;
      }
      // Stepping by a positive amount with `Sub` counts down, with `Add`
      // up; every step has to go the same way.
      if *down.get_or_insert(sub) != sub {
        return None;
      }
      let bound = bound_on_way_back(f, idom, preds, latch, v, sub)?;
      if sub {
        lo = lo.min(bound);
      } else {
        hi = hi.max(bound);
      }
      steps.push((id, step));
    }
  }
  if steps.is_empty() {
    return None;
  }
  // Counting down it never rises past where it started, counting up it
  // never falls below; a step moves it one step further at most.
  let max_step = steps.iter().map(|&(_, s)| s).max().unwrap_or(0);
  let (low, high) = if down == Some(true) {
    (lo - max_step, hi)
  } else {
    (lo, hi + max_step)
  };
  if low < -LIMIT || high > LIMIT {
    return None;
  }
  Some(steps.into_iter().map(|(id, _)| id).collect())
}

/// The least (counting down) or greatest (counting up) value `v` can have
/// when the back edge from `latch` carries it: a branch on comparing `v`
/// with a constant decides the side `latch` is on. The side has to be
/// entered only from that branch, so every way to the latch passes the
/// comparison.
fn bound_on_way_back(
  f: &Func,
  idom: &[Option<BlockId>],
  preds: &[Vec<BlockId>],
  latch: BlockId,
  v: ValueId,
  down: bool,
) -> Option<i64> {
  // Walk up from the latch through the blocks that dominate it, looking
  // for the branch whose one side every path to the latch takes.
  let mut at = latch;
  loop {
    let up = idom.get(at.0 as usize).copied().flatten()?;
    // Above the roots sits a virtual one with no block of its own.
    if up.0 as usize >= f.blocks.len() {
      return None;
    }
    if let Terminator::Branch {
      cond,
      then_block,
      else_block,
      ..
    } = &f.block(up).term
      && then_block != else_block
    {
      let only_from = |b: BlockId| preds[b.0 as usize] == [up];
      let side = if dominates(idom, *then_block, latch) && only_from(*then_block) {
        Some(true)
      } else if dominates(idom, *else_block, latch) && only_from(*else_block) {
        Some(false)
      } else {
        None
      };
      if let Some(side) = side
        && let Some(bound) = compared_bound(f, *cond, v, side, down)
      {
        return Some(bound);
      }
    }
    if up == at {
      return None;
    }
    at = up;
  }
}

/// What `cond` being `side` says about `v`: its least value counting
/// down, or its greatest counting up.
fn compared_bound(f: &Func, cond: ValueId, v: ValueId, side: bool, down: bool) -> Option<i64> {
  let def = f.def_inst(cond)?;
  let Op::ICmp(cmp) = def.op else {
    return None;
  };
  let (cmp, k) = if def.args[0] == v {
    (cmp, const_i64(f, def.args[1])?)
  } else if def.args[1] == v {
    (cmp.swapped(), const_i64(f, def.args[0])?)
  } else {
    return None;
  };
  // As the comparison holds on this side: `v cmp k`.
  let cmp = if side { cmp } else { negated(cmp) };
  match (cmp, down) {
    (Cmp::Gt, true) => k.checked_add(1),
    (Cmp::Ge, true) => Some(k),
    (Cmp::Lt, false) => k.checked_sub(1),
    (Cmp::Le, false) => Some(k),
    _ => None,
  }
}

/// The comparison that holds exactly when `c` does not. Integers have no
/// NaN, so this is exact.
fn negated(c: Cmp) -> Cmp {
  match c {
    Cmp::Eq => Cmp::Ne,
    Cmp::Ne => Cmp::Eq,
    Cmp::Lt => Cmp::Ge,
    Cmp::Le => Cmp::Gt,
    Cmp::Gt => Cmp::Le,
    Cmp::Ge => Cmp::Lt,
  }
}

fn const_i64(f: &Func, v: ValueId) -> Option<i64> {
  match f.def_inst(v)?.op {
    Op::ConstI64(c) => Some(c),
    _ => None,
  }
}

/// The values the edges from `from` to `to` bring for `to`'s `k`th
/// parameter.
fn edge_values(f: &Func, from: BlockId, to: BlockId, k: usize) -> Vec<ValueId> {
  let mut out = Vec::new();
  match &f.block(from).term {
    Terminator::Jump { target, args } if *target == to => out.push(args[k]),
    Terminator::Branch {
      then_block,
      then_args,
      else_block,
      else_args,
      ..
    } => {
      if *then_block == to {
        out.push(then_args[k]);
      }
      if *else_block == to {
        out.push(else_args[k]);
      }
    },
    _ => {},
  }
  out
}
