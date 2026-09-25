//! A separate, properly nested body for each way into the function.
//!
//! An on-stack-replacement entry jumps into a loop in the middle of a
//! nest. Left like that, the loops around the entry point have two ways
//! in, their headers no longer dominate their bodies, and nothing can be
//! hoisted out of them.
//!
//! Each entry therefore gets its own copy of the code it can reach,
//! arranged so that every loop in it is entered only at its header:
//!
//! - the loop the entry lands in, copied whole;
//! - for each loop around it, the rest of the iteration that was under
//!   way, copied as a run of blocks that ends where the loop would go
//!   round again;
//! - and there, a whole copy of that loop, nested loops and all, entered
//!   at its header like any other.
//!
//! Whatever comes after the outermost loop is copied once more. The
//! ordinary entry keeps the original blocks, which the entry points no
//! longer reach into.

use rustc_hash::{FxHashMap, FxHashSet};

use super::loops;
use crate::jit::ir::{BlockId, FrameState, Func, Terminator, ValueId};

pub fn run(f: &mut Func) {
  let entries = f.osr_entries.clone();
  if entries.is_empty() {
    return;
  }

  // The loop nest as the ordinary entry sees it. With the entry blocks
  // leading nowhere for the moment, every loop has one way in.
  let saved: Vec<Terminator> = entries
    .iter()
    .map(|&(_, b)| f.block(b).term.clone())
    .collect();
  for &(_, b) in &entries {
    f.block_mut(b).term = Terminator::Deopt(FrameState::root(0, Vec::new()));
  }
  let nest = loops::find(f);
  for (&(_, b), term) in entries.iter().zip(saved) {
    f.block_mut(b).term = term;
  }

  for (k, &(_, entry)) in entries.iter().enumerate() {
    let Terminator::Jump { target: header, .. } = f.block(entry).term else {
      continue;
    };
    // Every loop around the landing point, innermost first.
    let levels: Vec<FxHashSet<BlockId>> = nest
      .iter()
      .filter(|lp| lp.body.contains(&header))
      .map(|lp| lp.body.clone())
      .collect();
    let headers: Vec<BlockId> = nest
      .iter()
      .filter(|lp| lp.body.contains(&header))
      .map(|lp| lp.header)
      .collect();
    if levels.is_empty() || headers[0] != header {
      continue;
    }
    let new_entry = Copier::new(f, &levels, &headers).copy_entry(entry);
    for b in 0..f.blocks.len() {
      let b = BlockId(b as u32);
      if b != new_entry && f.block(b).term.successors().contains(&entry) {
        f.retarget(b, entry, new_entry);
      }
    }
    f.osr_entries[k].1 = new_entry;
  }
  prune_unreachable(f);
}

/// Which copy of a block: a whole loop at some level, the rest of an
/// iteration at some level, or the code after the whole nest.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Part {
  Whole(usize),
  Rest(usize),
  After,
}

struct Copier<'f> {
  f: &'f mut Func,
  levels: &'f [FxHashSet<BlockId>],
  headers: &'f [BlockId],
  blocks: FxHashMap<(Part, BlockId), BlockId>,
  values: FxHashMap<(Part, ValueId), ValueId>,
  pending: Vec<(Part, BlockId)>,
}

impl<'f> Copier<'f> {
  fn new(f: &'f mut Func, levels: &'f [FxHashSet<BlockId>], headers: &'f [BlockId]) -> Self {
    Copier {
      f,
      levels,
      headers,
      blocks: FxHashMap::default(),
      values: FxHashMap::default(),
      pending: Vec::new(),
    }
  }

  /// Copies the entry block and everything it leads to; returns the copy
  /// of the entry block.
  fn copy_entry(mut self, entry: BlockId) -> BlockId {
    // The entry block itself belongs to no loop; it jumps into the whole
    // copy of the innermost one.
    let copy = self.block(Part::After, entry);
    while let Some((part, b)) = self.pending.pop() {
      self.fill(part, b);
    }
    self.remap_values();
    copy
  }

  /// Where an edge from a block in `part` to `target` goes.
  fn follow(&self, part: Part, from: BlockId, target: BlockId) -> (Part, BlockId) {
    let within = |level: usize| self.levels[level].contains(&target);
    match part {
      Part::Whole(j) if within(j) => (Part::Whole(j), target),
      Part::Rest(j) if target == self.headers[j] => (Part::Whole(j), target),
      Part::Rest(j) if within(j) => (Part::Rest(j), target),
      Part::After
        if self.levels.iter().all(|l| !l.contains(&from))
          && within(0)
          && target == self.headers[0] =>
      {
        (Part::Whole(0), target)
      },
      Part::After => (Part::After, target),
      Part::Whole(j) | Part::Rest(j) => {
        // Leaving a loop: into the rest of the iteration of the nearest
        // loop around it that still holds the target, or past them all.
        match (j + 1..self.levels.len()).find(|&m| self.levels[m].contains(&target)) {
          Some(m) => (Part::Rest(m), target),
          None => (Part::After, target),
        }
      },
    }
  }

  /// The copy of `b` for `part`, made if it does not exist yet.
  fn block(&mut self, part: Part, b: BlockId) -> BlockId {
    if let Some(&c) = self.blocks.get(&(part, b)) {
      return c;
    }
    let c = self.f.add_block_like(b);
    for p in self.f.block(b).params.clone() {
      let np = self.f.add_block_param(c, self.f.ty(p));
      self.values.insert((part, p), np);
    }
    self.f.block_mut(c).param_regs = self.f.block(b).param_regs.clone();
    self.blocks.insert((part, b), c);
    self.pending.push((part, b));
    c
  }

  /// Copies `b`'s instructions and terminator into its copy for `part`,
  /// still naming the original values; `remap_values` fixes them once
  /// every definition has its copy.
  fn fill(&mut self, part: Part, b: BlockId) {
    let c = self.blocks[&(part, b)];
    for i in self.f.block(b).insts.clone() {
      let inst = self.f.inst(i).clone();
      let ty = inst.result.map(|r| self.f.ty(r));
      let result = self.f.push(c, inst.op, inst.args, ty, inst.state);
      if let (Some(old), Some(new)) = (inst.result, result) {
        self.values.insert((part, old), new);
      }
    }
    self.f.block_mut(c).fixed_regs = self.f.block(b).fixed_regs.clone();
    let term = match self.f.block(b).term.clone() {
      Terminator::Jump { target, args } => {
        let (p, t) = self.follow(part, b, target);
        Terminator::Jump {
          target: self.block(p, t),
          args,
        }
      },
      Terminator::Branch {
        cond,
        then_block,
        then_args,
        else_block,
        else_args,
      } => {
        let (tp, tt) = self.follow(part, b, then_block);
        let (ep, et) = self.follow(part, b, else_block);
        Terminator::Branch {
          cond,
          then_block: self.block(tp, tt),
          then_args,
          else_block: self.block(ep, et),
          else_args,
        }
      },
      other => other,
    };
    self.f.set_term(c, term);
  }

  /// Points every operand in the copies at the copy of its definition.
  /// A block's own values come from its part; anything else it names was
  /// defined in a block that dominates it, whose copy is the nearest one
  /// on the way in: a whole loop's copy for the rest of its iteration,
  /// the enclosing levels after that, then the blocks outside every loop.
  fn remap_values(&mut self) {
    let copies: Vec<((Part, BlockId), BlockId)> =
      self.blocks.iter().map(|(&k, &v)| (k, v)).collect();
    for ((part, _), c) in copies {
      let order = self.lookup_order(part);
      let values = &self.values;
      let map = |v: ValueId| {
        order
          .iter()
          .find_map(|p| values.get(&(*p, v)).copied())
          .unwrap_or(v)
      };
      let map_state = |s: &FrameState| FrameState {
        ip: s.ip,
        regs: s.regs.iter().map(|&(r, v)| (r, map(v))).collect(),
        frame: s.frame,
        blame: s.blame,
      };
      for i in self.f.block(c).insts.clone() {
        let inst = &mut self.f.insts[i.0 as usize];
        inst.args.iter_mut().for_each(|a| *a = map(*a));
        if let Some(s) = &inst.state {
          inst.state = Some(map_state(s));
        }
      }
      let fixed = self
        .f
        .block(c)
        .fixed_regs
        .iter()
        .map(|&(r, v)| (r, map(v)))
        .collect();
      self.f.block_mut(c).fixed_regs = fixed;
      let term = match self.f.block(c).term.clone() {
        Terminator::Jump { target, args } => Terminator::Jump {
          target,
          args: args.into_iter().map(map).collect(),
        },
        Terminator::Branch {
          cond,
          then_block,
          then_args,
          else_block,
          else_args,
        } => Terminator::Branch {
          cond: map(cond),
          then_block,
          then_args: then_args.into_iter().map(map).collect(),
          else_block,
          else_args: else_args.into_iter().map(map).collect(),
        },
        Terminator::Return(v) => Terminator::Return(map(v)),
        Terminator::Deopt(s) => Terminator::Deopt(map_state(&s)),
        Terminator::Unset => Terminator::Unset,
      };
      self.f.set_term(c, term);
    }
  }

  fn lookup_order(&self, part: Part) -> Vec<Part> {
    let n = self.levels.len();
    let mut order = vec![part];
    match part {
      Part::Whole(j) => {
        for m in j + 1..n {
          order.push(Part::Rest(m));
          order.push(Part::Whole(m));
        }
      },
      Part::Rest(j) => {
        order.push(Part::Whole(j - 1));
        for m in j + 1..n {
          order.push(Part::Rest(m));
          order.push(Part::Whole(m));
        }
      },
      Part::After => {
        for m in 0..n {
          order.push(Part::Whole(m));
          order.push(Part::Rest(m));
        }
      },
    }
    order.push(Part::After);
    order
  }
}

/// Empties every block no entry reaches, so nothing unreachable still
/// counts as a way into a block that is reachable.
pub fn prune_unreachable(f: &mut Func) {
  let mut reachable = vec![false; f.blocks.len()];
  for b in f.reverse_postorder() {
    reachable[b.0 as usize] = true;
  }
  for (bi, block) in f.blocks.iter_mut().enumerate() {
    if reachable[bi] {
      continue;
    }
    block.insts.clear();
    block.params.clear();
    block.param_regs.clear();
    block.fixed_regs.clear();
    block.term = Terminator::Deopt(FrameState::root(0, Vec::new()));
  }
}
