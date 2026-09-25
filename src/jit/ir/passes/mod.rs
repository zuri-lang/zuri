//! Whole-function passes over the IR.
//!
//! Each pass keeps the function valid SSA: `Func::verify` runs after every
//! one of them in debug builds, so a pass that breaks dominance or an
//! edge's arity is caught at the pass that did it rather than somewhere in
//! lowering.

pub mod elems;
pub mod entries;
pub mod gvn;
pub mod induction;
pub mod licm;
pub mod loops;
pub mod phi;
pub mod repr;
pub mod simplify;
pub mod sink;

use rustc_hash::FxHashMap;

use super::build::Feedback;
use super::{Func, Terminator, ValueId};

/// Runs every pass, in order. `feedback` is what the function was built
/// from, which says where speculating has already failed.
pub fn run(f: &mut Func, feedback: &Feedback) -> Result<(), String> {
  check(f, "build")?;
  entries::run(f);
  check(f, "entries")?;
  phi::run(f);
  check(f, "phi")?;
  repr::run(f);
  check(f, "repr")?;
  simplify::run(f);
  check(f, "simplify")?;
  gvn::run(f);
  check(f, "gvn")?;
  licm::run(f);
  check(f, "licm")?;
  // Every entry into a loop now comes through its preheader, so a header
  // parameter that only an on-stack-replacement entry used to vary is
  // down to one value and goes.
  phi::run(f);
  check(f, "phi")?;
  induction::run(f, feedback);
  check(f, "induction")?;
  elems::run(f, feedback);
  check(f, "elems")?;
  // Nothing after this point builds a frame state, so the values kept for
  // folded-away parameters need not stay alive any longer.
  for b in &mut f.blocks {
    b.fixed_regs.clear();
  }
  gvn::run(f);
  check(f, "gvn")?;
  simplify::run(f);
  check(f, "simplify")?;
  dce(f);
  check(f, "dce")?;
  sink::run(f);
  check(f, "sink")?;
  Ok(())
}

fn check(f: &Func, after: &str) -> Result<(), String> {
  if cfg!(debug_assertions) {
    f.verify()
      .map_err(|e| format!("invalid IR after {after}: {e}"))?;
  }
  Ok(())
}

/// Points every use of `from` at `to`: operands, frame states and edges.
pub fn replace_uses(f: &mut Func, from: ValueId, to: ValueId) {
  if from == to {
    return;
  }
  rewrite_uses(f, |v| {
    if *v == from {
      *v = to;
    }
  });
}

/// Applies a whole set of replacements in one walk. A replacement may
/// itself be replaced; each use ends up at the last value in its chain.
pub fn replace_all(f: &mut Func, map: &FxHashMap<ValueId, ValueId>) {
  if map.is_empty() {
    return;
  }
  rewrite_uses(f, |v| *v = resolve(map, *v));
}

/// Where `v` ends up after following `map`.
pub fn resolve(map: &FxHashMap<ValueId, ValueId>, mut v: ValueId) -> ValueId {
  while let Some(&next) = map.get(&v) {
    v = next;
  }
  v
}

fn rewrite_uses(f: &mut Func, swap: impl Fn(&mut ValueId)) {
  let swap = &swap;
  for inst in &mut f.insts {
    inst.args.iter_mut().for_each(swap);
    if let Some(state) = &mut inst.state {
      state.regs.iter_mut().for_each(|(_, v)| swap(v));
    }
  }
  for b in &mut f.blocks {
    b.fixed_regs.iter_mut().for_each(|(_, v)| swap(v));
    match &mut b.term {
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
}

/// Removes everything nothing needs, block parameters included.
///
/// Liveness starts from what has an effect, what a branch tests, what a
/// function returns and what a frame state holds, and flows back through
/// operands. A block parameter is live only when something live uses it,
/// and only then are the values its edges bring live, so a value that
/// only ever feeds itself around a loop goes along with its parameter.
pub fn dce(f: &mut Func) {
  use super::{BlockId, ValueDef};

  let mut live = vec![false; f.values.len()];
  let mut live_inst = vec![false; f.insts.len()];
  let mut work: Vec<ValueId> = Vec::new();
  let mark = |v: ValueId, live: &mut Vec<bool>, work: &mut Vec<ValueId>| {
    if !live[v.0 as usize] {
      live[v.0 as usize] = true;
      work.push(v);
    }
  };

  for b in &f.blocks {
    for &i in &b.insts {
      let inst = f.inst(i);
      if inst.op.has_effect() {
        live_inst[i.0 as usize] = true;
        for &a in &inst.args {
          mark(a, &mut live, &mut work);
        }
        if let Some(state) = &inst.state {
          for &(_, v) in &state.regs {
            mark(v, &mut live, &mut work);
          }
        }
      }
    }
    match &b.term {
      Terminator::Branch { cond, .. } => mark(*cond, &mut live, &mut work),
      Terminator::Return(v) => mark(*v, &mut live, &mut work),
      Terminator::Deopt(state) => {
        for &(_, v) in &state.regs {
          mark(v, &mut live, &mut work);
        }
      },
      Terminator::Jump { .. } | Terminator::Unset => {},
    }
    for &(_, v) in &b.fixed_regs {
      mark(v, &mut live, &mut work);
    }
  }

  let preds = f.predecessors();
  while let Some(v) = work.pop() {
    match f.values[v.0 as usize].def {
      ValueDef::Inst(i) => {
        live_inst[i.0 as usize] = true;
        let inst = f.inst(i);
        for &a in &inst.args {
          mark(a, &mut live, &mut work);
        }
        if let Some(state) = &inst.state {
          for &(_, s) in &state.regs {
            mark(s, &mut live, &mut work);
          }
        }
      },
      ValueDef::Param(b, _) => {
        let Some(k) = f.block(b).params.iter().position(|&p| p == v) else {
          continue;
        };
        for &pred in &preds[b.0 as usize] {
          for args in edge_args(f, pred, b) {
            mark(args[k], &mut live, &mut work);
          }
        }
      },
    }
  }

  for bi in 0..f.blocks.len() {
    let insts = std::mem::take(&mut f.blocks[bi].insts);
    f.blocks[bi].insts = insts
      .into_iter()
      .filter(|&i| live_inst[i.0 as usize])
      .collect();
  }

  // Dead parameters go from each block and from every edge into it.
  for bi in 0..f.blocks.len() {
    let b = BlockId(bi as u32);
    let mut k = f.blocks[bi].params.len();
    while k > 0 {
      k -= 1;
      let p = f.blocks[bi].params[k];
      if live[p.0 as usize] {
        continue;
      }
      let block = f.block_mut(b);
      block.params.remove(k);
      if k < block.param_regs.len() {
        block.param_regs.remove(k);
      }
      let mut seen = Vec::new();
      for &pred in &preds[bi] {
        if seen.contains(&pred) {
          continue;
        }
        seen.push(pred);
        for args in f.edge_args_mut(pred, b) {
          args.remove(k);
        }
      }
    }
  }
}

/// The argument lists of the edges from `from` into `to`.
fn edge_args(f: &Func, from: super::BlockId, to: super::BlockId) -> Vec<&Vec<ValueId>> {
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
