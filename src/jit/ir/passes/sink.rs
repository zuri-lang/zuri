//! Moves pure operations out of a loop they are only used after.
//!
//! The builder computes values where the bytecode does, so a number a
//! loop boxes for the `return` after it gets boxed on every iteration.
//! A pure operation whose every use is in one block outside its loop
//! moves to the start of that block. Operands are defined above the loop,
//! or are themselves moved by the same rule, so they still dominate it.

use rustc_hash::FxHashMap;

use super::licm::is_pure;
use super::loops;
use crate::jit::ir::{BlockId, Func, InstId, ValueId};

pub fn run(f: &mut Func) {
  let loops = loops::find(f);
  if loops.is_empty() {
    return;
  }
  loop {
    let users = use_blocks(f);
    let mut moved = false;
    for lp in &loops {
      let blocks: Vec<BlockId> = lp.body.iter().copied().collect();
      for b in blocks {
        let insts = f.block(b).insts.clone();
        let mut keep: Vec<InstId> = Vec::with_capacity(insts.len());
        for id in insts {
          let inst = f.inst(id);
          let target = inst
            .result
            .filter(|_| is_pure(&inst.op))
            .and_then(|r| single_block(&users, r))
            .filter(|u| !lp.body.contains(u));
          match target {
            Some(u) => {
              f.block_mut(u).insts.insert(0, id);
              moved = true;
            },
            None => keep.push(id),
          }
        }
        f.block_mut(b).insts = keep;
      }
      if moved {
        break;
      }
    }
    if !moved {
      return;
    }
  }
}

/// The one block every use of `v` is in, if there is exactly one.
fn single_block(users: &FxHashMap<ValueId, Vec<BlockId>>, v: ValueId) -> Option<BlockId> {
  match users.get(&v).map(Vec::as_slice) {
    Some([only]) => Some(*only),
    _ => None,
  }
}

/// For each value, the distinct blocks that use it.
fn use_blocks(f: &Func) -> FxHashMap<ValueId, Vec<BlockId>> {
  let mut users: FxHashMap<ValueId, Vec<BlockId>> = FxHashMap::default();
  let mut note = |v: ValueId, b: BlockId| {
    let list = users.entry(v).or_default();
    if !list.contains(&b) {
      list.push(b);
    }
  };
  for (bi, blk) in f.blocks.iter().enumerate() {
    let b = BlockId(bi as u32);
    for &i in &blk.insts {
      let inst = f.inst(i);
      for &a in &inst.args {
        note(a, b);
      }
      if let Some(state) = &inst.state {
        for &(_, v) in &state.regs {
          note(v, b);
        }
      }
    }
    for v in blk.term.uses() {
      note(v, b);
    }
    for &(_, v) in &blk.fixed_regs {
      note(v, b);
    }
  }
  users
}
