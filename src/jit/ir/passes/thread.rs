//! Branches whose outcome is already known on the way in.
//!
//! `a and b` and `a or b` in a condition build a block that merges a
//! boolean from each side and branches on it. Along the edge that comes
//! from testing `a` alone, that boolean is `a` or its negation, which the
//! edge has already decided, so the edge can go straight to where the
//! merged branch would send it. The merge is then left with one way in
//! and folds into the block before it, which puts a comparison back
//! beside the branch that tests it.
//!
//! A parameter of the merge block that the new destination uses becomes a
//! parameter of the destination too, so every use still sees one
//! definition that dominates it. When a use cannot be served that way,
//! the edge is left alone.

use rustc_hash::FxHashSet;

use super::entries::prune_unreachable;
use super::replace_uses;
use crate::jit::ir::{BlockId, FrameState, Func, Op, Terminator, ValueId, dominates, dominators};

/// Returns whether anything changed.
pub fn run(f: &mut Func) -> bool {
  let mut changed = false;
  loop {
    let mut progress = false;
    for bi in 0..f.blocks.len() {
      for side in [true, false] {
        progress |= thread_edge(f, BlockId(bi as u32), side);
      }
    }
    progress |= merge_single_entries(f);
    if !progress {
      break;
    }
    changed = true;
  }
  if changed {
    prune_unreachable(f);
  }
  changed
}

/// What `v`, passed along the `side` edge of a branch on `cond`, is known
/// to be there.
fn known_on_edge(f: &Func, v: ValueId, cond: ValueId, side: bool) -> Option<bool> {
  if v == cond {
    return Some(side);
  }
  let def = f.def_inst(v)?;
  match def.op {
    Op::BNot if def.args[0] == cond => Some(!side),
    Op::ConstBool(b) => Some(b),
    _ => None,
  }
}

/// Sends the `side` edge of `p`'s branch past the block it leads to, when
/// that block only branches on a parameter the edge already decides.
fn thread_edge(f: &mut Func, p: BlockId, side: bool) -> bool {
  let Terminator::Branch {
    cond,
    then_block,
    then_args,
    else_block,
    else_args,
  } = &f.block(p).term
  else {
    return false;
  };
  let cond = *cond;
  let (b, args, other) = if side {
    (*then_block, then_args.clone(), *else_block)
  } else {
    (*else_block, else_args.clone(), *then_block)
  };
  if b == p {
    return false;
  }
  let block = f.block(b);
  if !block.insts.is_empty() {
    return false;
  }
  let Terminator::Branch {
    cond: q,
    then_block: bt,
    then_args: bta,
    else_block: be,
    else_args: bea,
  } = &block.term
  else {
    return false;
  };
  let Some(k) = block.params.iter().position(|v| v == q) else {
    return false;
  };
  let Some(taken) = known_on_edge(f, args[k], cond, side) else {
    return false;
  };
  let (s, template) = if taken {
    (*bt, bta.clone())
  } else {
    (*be, bea.clone())
  };
  if s == b || s == p || s == other {
    return false;
  }
  let params = block.params.clone();
  let param_regs = block.param_regs.clone();

  // Every use of the merge block's parameters, other than its own
  // branch, has to stay dominated by a definition once the edge bypasses
  // it. A use the destination dominates moves to a parameter of the
  // destination; one the destination cannot reach without going through
  // the merge block keeps the merge block as its only way in; anything
  // else rules the edge out.
  let order = f.reverse_postorder();
  let idom = dominators(f, &order);
  let reach = reachable_avoiding(f, s, b);
  let mut moved: Vec<usize> = Vec::new();
  for (j, &param) in params.iter().enumerate() {
    let mut needs = false;
    for u in use_blocks(f, param, b) {
      if !reach.contains(&u) {
        continue;
      }
      if dominates(&idom, s, u) {
        needs = true;
      } else {
        return false;
      }
    }
    if needs {
      moved.push(j);
    }
  }
  // A new parameter takes a value from every edge into the destination.
  // The merge block dominates everything that uses its parameters, the
  // destination included, so each existing edge can pass the parameter.
  if !moved.is_empty() && !dominates(&idom, b, s) {
    return false;
  }

  let substitute = |v: ValueId| match params.iter().position(|&x| x == v) {
    Some(j) => args[j],
    None => v,
  };
  let mut new_args: Vec<ValueId> = template.iter().map(|&v| substitute(v)).collect();
  let dominated: Vec<BlockId> = order
    .iter()
    .copied()
    .filter(|&u| dominates(&idom, s, u))
    .collect();
  for &j in &moved {
    let ty = f.ty(params[j]);
    let fresh = f.add_block_param(s, ty);
    let dest = f.block_mut(s);
    if dest.param_regs.len() + 1 == dest.params.len() {
      dest
        .param_regs
        .push(param_regs.get(j).copied().unwrap_or(0));
    }
    for pred in f.predecessors()[s.0 as usize].clone() {
      for edge in f.edge_args_mut(pred, s) {
        edge.push(params[j]);
      }
    }
    for &u in &dominated {
      rename_in_block(f, u, params[j], fresh);
    }
    new_args.push(args[j]);
  }

  let term = &mut f.block_mut(p).term;
  if let Terminator::Branch {
    then_block,
    then_args,
    else_block,
    else_args,
    ..
  } = term
  {
    if side {
      *then_block = s;
      *then_args = new_args;
    } else {
      *else_block = s;
      *else_args = new_args;
    }
  }
  true
}

/// Folds a block reached by a single jump into the block that jumps to
/// it.
fn merge_single_entries(f: &mut Func) -> bool {
  let mut roots: FxHashSet<BlockId> = FxHashSet::default();
  roots.insert(f.entry);
  for &(_, b) in &f.osr_entries {
    roots.insert(b);
  }
  let mut changed = false;
  for xi in 0..f.blocks.len() {
    let x = BlockId(xi as u32);
    loop {
      let Terminator::Jump { target: y, args } = &f.block(x).term else {
        break;
      };
      let (y, args) = (*y, args.clone());
      if y == x || roots.contains(&y) {
        break;
      }
      let preds = f.predecessors();
      if preds[y.0 as usize].len() != 1 {
        break;
      }
      let params = f.block(y).params.clone();
      for (&param, &arg) in params.iter().zip(&args) {
        replace_uses(f, param, arg);
      }
      let moved = std::mem::take(&mut f.block_mut(y).insts);
      let term = std::mem::replace(&mut f.block_mut(y).term, Terminator::Unset);
      f.block_mut(x).insts.extend(moved);
      f.block_mut(x).term = term;
      let emptied = f.block_mut(y);
      emptied.params.clear();
      emptied.param_regs.clear();
      emptied.fixed_regs.clear();
      emptied.term = Terminator::Deopt(FrameState::root(0, Vec::new()));
      changed = true;
    }
  }
  changed
}

/// Every block reachable from `from`, itself included, without passing
/// through `avoid`.
fn reachable_avoiding(f: &Func, from: BlockId, avoid: BlockId) -> FxHashSet<BlockId> {
  let mut seen = FxHashSet::default();
  let mut stack = vec![from];
  while let Some(b) = stack.pop() {
    if b != avoid && seen.insert(b) {
      stack.extend(f.block(b).term.successors());
    }
  }
  seen
}

/// The blocks that use `v`, apart from `owner`'s terminator.
fn use_blocks(f: &Func, v: ValueId, owner: BlockId) -> Vec<BlockId> {
  let mut out = Vec::new();
  for (bi, block) in f.blocks.iter().enumerate() {
    let b = BlockId(bi as u32);
    let in_insts = block.insts.iter().any(|&i| {
      let inst = f.inst(i);
      inst.args.contains(&v)
        || inst
          .state
          .as_ref()
          .is_some_and(|s| s.regs.iter().any(|&(_, x)| x == v))
    });
    let in_regs = block.fixed_regs.iter().any(|&(_, x)| x == v);
    let in_term = b != owner && term_uses(&block.term, v);
    if in_insts || in_regs || in_term {
      out.push(b);
    }
  }
  out
}

fn term_uses(term: &Terminator, v: ValueId) -> bool {
  match term {
    Terminator::Jump { args, .. } => args.contains(&v),
    Terminator::Branch {
      cond,
      then_args,
      else_args,
      ..
    } => *cond == v || then_args.contains(&v) || else_args.contains(&v),
    Terminator::Return(x) => *x == v,
    Terminator::Deopt(state) => state.regs.iter().any(|&(_, x)| x == v),
    Terminator::Unset => false,
  }
}

/// Replaces `from` with `to` in everything block `b` holds.
fn rename_in_block(f: &mut Func, b: BlockId, from: ValueId, to: ValueId) {
  let swap = |v: &mut ValueId| {
    if *v == from {
      *v = to;
    }
  };
  let insts = f.block(b).insts.clone();
  for i in insts {
    let inst = &mut f.insts[i.0 as usize];
    inst.args.iter_mut().for_each(swap);
    if let Some(state) = &mut inst.state {
      state.regs.iter_mut().for_each(|(_, v)| swap(v));
    }
  }
  let block = f.block_mut(b);
  block.fixed_regs.iter_mut().for_each(|(_, v)| swap(v));
  match &mut block.term {
    Terminator::Jump { args, .. } => args.iter_mut().for_each(swap),
    Terminator::Branch {
      cond,
      then_args,
      else_args,
      ..
    } => {
      swap(cond);
      then_args.iter_mut().for_each(swap);
      else_args.iter_mut().for_each(swap);
    },
    Terminator::Return(v) => swap(v),
    Terminator::Deopt(state) => state.regs.iter_mut().for_each(|(_, v)| swap(v)),
    Terminator::Unset => {},
  }
}
