//! Natural loops.
//!
//! A back edge is an edge into a block that dominates its source. The
//! loop it closes is the header plus every block that reaches the edge's
//! source without going through the header. Back edges to the same header
//! make one loop.

use rustc_hash::{FxHashMap, FxHashSet};

use crate::jit::ir::{BlockId, Func, dominates, dominators};

pub struct Loop {
  pub header: BlockId,
  pub body: FxHashSet<BlockId>,
  /// The sources of the back edges.
  pub latches: Vec<BlockId>,
}

/// Every loop in `f`, innermost first.
pub fn find(f: &Func) -> Vec<Loop> {
  let order = f.reverse_postorder();
  let idom = dominators(f, &order);
  let preds = f.predecessors();

  let mut loops: FxHashMap<BlockId, Loop> = FxHashMap::default();
  for &b in &order {
    for s in f.block(b).term.successors() {
      if !dominates(&idom, s, b) {
        continue;
      }
      let lp = loops.entry(s).or_insert_with(|| Loop {
        header: s,
        body: FxHashSet::from_iter([s]),
        latches: Vec::new(),
      });
      if !lp.latches.contains(&b) {
        lp.latches.push(b);
      }
      let mut work = vec![b];
      while let Some(x) = work.pop() {
        if lp.body.insert(x) {
          work.extend(preds[x.0 as usize].iter().copied());
        }
      }
    }
  }

  let mut loops: Vec<Loop> = loops.into_values().collect();
  loops.sort_by_key(|lp| (lp.body.len(), lp.header));
  loops
}
