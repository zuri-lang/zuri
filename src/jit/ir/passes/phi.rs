//! Removes block parameters that can only ever hold one value.
//!
//! The builder gives every block a parameter for each register live into
//! it, whether or not the register changes along the way, so a join after
//! an `if` that never touches `x` still takes `x` as a parameter. Where
//! every edge passes the same value, or the parameter itself around a
//! loop, the parameter is that value.

use rustc_hash::FxHashMap;

use super::{replace_all, resolve};
use crate::jit::ir::{BlockId, Func, ValueId};

pub fn run(f: &mut Func) {
  let roots = f.roots();
  // What each removed parameter stands for. The uses are rewritten once
  // at the end; until then an edge's arguments are read through this.
  let mut replaced: FxHashMap<ValueId, ValueId> = FxHashMap::default();
  loop {
    let preds = f.predecessors();
    let mut changed = false;
    for bi in 0..f.blocks.len() {
      let b = BlockId(bi as u32);
      if roots.contains(&b) || preds[bi].is_empty() {
        continue;
      }
      let mut k = f.blocks[bi].params.len();
      while k > 0 {
        k -= 1;
        let p = f.blocks[bi].params[k];
        let Some(only) = sole_incoming(f, &replaced, &preds[bi], b, k, p) else {
          continue;
        };
        remove_param(f, &preds[bi], b, k, only);
        replaced.insert(p, only);
        changed = true;
      }
    }
    if !changed {
      break;
    }
  }
  replace_all(f, &replaced);
}

/// The one value parameter `k` of `b` receives, not counting the
/// parameter passed back to itself, if there is exactly one.
fn sole_incoming(
  f: &Func,
  replaced: &FxHashMap<ValueId, ValueId>,
  preds: &[BlockId],
  b: BlockId,
  k: usize,
  p: ValueId,
) -> Option<ValueId> {
  let mut only: Option<ValueId> = None;
  for &pred in preds {
    for args in edge_args(f, pred, b) {
      let v = resolve(replaced, args[k]);
      if v == p {
        continue;
      }
      match only {
        None => only = Some(v),
        Some(o) if o == v => {},
        Some(_) => return None,
      }
    }
  }
  only
}

fn edge_args<'f>(f: &'f Func, from: BlockId, to: BlockId) -> Vec<&'f Vec<ValueId>> {
  use crate::jit::ir::Terminator;
  match &f.block(from).term {
    Terminator::Jump { target, args } if *target == to => vec![args],
    Terminator::Branch {
      then_block,
      then_args,
      else_block,
      else_args,
      ..
    } => {
      let mut v = Vec::new();
      if *then_block == to {
        v.push(then_args);
      }
      if *else_block == to {
        v.push(else_args);
      }
      v
    },
    _ => Vec::new(),
  }
}

/// Drops parameter `k` of `b`, remembering which register it held and
/// the one value it always had.
fn remove_param(f: &mut Func, preds: &[BlockId], b: BlockId, k: usize, only: ValueId) {
  let block = f.block_mut(b);
  block.params.remove(k);
  if k < block.param_regs.len() {
    let reg = block.param_regs.remove(k);
    block.fixed_regs.push((reg, only));
  }
  let mut seen = Vec::new();
  for &pred in preds {
    if seen.contains(&pred) {
      continue;
    }
    seen.push(pred);
    for args in f.edge_args_mut(pred, b) {
      args.remove(k);
    }
  }
}
