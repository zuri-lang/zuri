//! Loop-invariant code motion.
//!
//! Every loop gets a preheader: a block that all entries into the loop
//! pass through, the ordinary one and any on-stack-replacement entry
//! alike, taking the same parameters as the header. Whatever the loop
//! computes the same way on every iteration moves there and runs once.
//!
//! What counts as the same on every iteration: operands defined outside
//! the loop, or header parameters every back edge passes back unchanged.
//! Pure operations move freely. A guard or a read from memory only moves
//! from a block every iteration passes through, so it never runs on entry
//! when the loop body would not have run it; a memory read only moves
//! when nothing in the loop writes that kind of memory; and nothing that
//! produces a raw pointer moves out of a loop that can collect, since a
//! pointer from before a collection is not valid after it.
//!
//! A guard that moves deoptimizes at the loop header, with the values the
//! preheader was given: the interpreter then runs the loop from its start,
//! which is where the moved check first applies.

use rustc_hash::{FxHashMap, FxHashSet};

use crate::jit::ir::{
  BlockId, FrameState, Func, InstId, Op, Terminator, Ty, ValueDef, ValueId, dominates, dominators,
};

pub fn run(f: &mut Func) {
  // Innermost first, so what an inner loop hoists can move again out of
  // the loop around it. The loops are found afresh each time round: the
  // preheader an inner loop gets is part of the loop around it, and so is
  // everything moved into it.
  let mut done: FxHashSet<BlockId> = FxHashSet::default();
  loop {
    let Some(lp) = super::loops::find(f).into_iter().find(|lp| !done.contains(&lp.header)) else {
      return;
    };
    done.insert(lp.header);
    hoist_loop(f, lp.header, &lp.body, &lp.latches);
  }
}

fn hoist_loop(f: &mut Func, header: BlockId, body: &FxHashSet<BlockId>, latches: &[BlockId]) {
  let header_ip = f.block(header).ip;
  let Some(header_ip) = header_ip else {
    return;
  };

  // What the loop does to memory decides which reads may move.
  let mut collects = false;
  let mut writes_lists = false;
  let mut writes_elems = false;
  let mut writes_fields = false;
  let mut writes_upvals = false;
  let mut writes_globals = false;
  for &b in body {
    for &i in &f.block(b).insts {
      match f.inst(i).op {
        Op::StoreElem => writes_elems = true,
        Op::ListAppend => {
          writes_lists = true;
          writes_elems = true;
        },
        Op::StoreField(_) => writes_fields = true,
        Op::StoreUpval => writes_upvals = true,
        Op::StoreGlobal(_) => writes_globals = true,
        ref op if op.may_collect() => collects = true,
        _ => {},
      }
    }
  }

  // Header parameters every back edge passes back unchanged.
  let params = f.block(header).params.clone();
  let mut unchanged: Vec<bool> = vec![true; params.len()];
  for &latch in latches {
    for args in f.edge_args_mut(latch, header) {
      for (k, a) in args.iter().enumerate() {
        if *a != params[k] {
          unchanged[k] = false;
        }
      }
    }
  }

  let preheader = make_preheader(f, header, body);
  let pre_params = f.block(preheader).params.clone();
  let mut outside: FxHashMap<ValueId, ValueId> = FxHashMap::default();
  for (k, &p) in params.iter().enumerate() {
    if unchanged[k] {
      outside.insert(p, pre_params[k]);
    }
  }

  let order = f.reverse_postorder();
  let idom = dominators(f, &order);
  let every_iteration =
    |b: BlockId| latches.iter().all(|&latch| dominates(&idom, b, latch));

  let mut home: FxHashMap<InstId, BlockId> = FxHashMap::default();
  for (bi, blk) in f.blocks.iter().enumerate() {
    for &i in &blk.insts {
      home.insert(i, BlockId(bi as u32));
    }
  }
  let is_outside = |f: &Func, v: ValueId, moved: &FxHashMap<ValueId, ValueId>| {
    if moved.contains_key(&v) {
      return true;
    }
    match f.values[v.0 as usize].def {
      ValueDef::Param(b, _) => !body.contains(&b),
      ValueDef::Inst(i) => home.get(&i).is_none_or(|b| !body.contains(b)),
    }
  };

  let state = FrameState {
    ip: header_ip,
    regs: f.entry_regs(header, &pre_params),
    frame: f.block(header).frame,
    blame: None,
  };

  for &b in order.iter().filter(|b| body.contains(b)) {
    let insts = f.block(b).insts.clone();
    let mut keep = Vec::with_capacity(insts.len());
    for id in insts {
      let inst = f.inst(id).clone();
      let movable = match &inst.op {
        Op::Guard(_) => every_iteration(b),
        Op::ListLen | Op::ListData => every_iteration(b) && !collects && !writes_lists,
        Op::LoadElem => every_iteration(b) && !collects && !writes_elems,
        Op::LoadField(_) => every_iteration(b) && !collects && !writes_fields,
        Op::LoadGlobal(_) => every_iteration(b) && !collects && !writes_globals,
        Op::UpvalCell(_) => every_iteration(b) && !collects,
        Op::LoadUpval => every_iteration(b) && !collects && !writes_upvals,
        op => is_pure(op),
      };
      let produces_ptr = inst.result.is_some_and(|r| f.ty(r) == Ty::Ptr);
      let operands_outside = inst.args.iter().all(|&a| is_outside(f, a, &outside));
      if !movable || !operands_outside || (produces_ptr && collects) {
        keep.push(id);
        continue;
      }

      // Move it, reading the preheader's copy of any header parameter.
      let args: Vec<ValueId> = inst
        .args
        .iter()
        .map(|a| outside.get(a).copied().unwrap_or(*a))
        .collect();
      // A moved guard resumes at the header but blames its own check.
      let new_state = inst.state.as_ref().map(|old| FrameState {
        blame: Some(old.site()),
        ..state.clone()
      });
      let pos = f.block(preheader).insts.len();
      let result_ty = inst.result.map(|r| f.ty(r));
      let moved = f.insert(preheader, pos, inst.op.clone(), args, result_ty, new_state);
      if let (Some(old), Some(new)) = (inst.result, moved) {
        super::replace_uses(f, old, new);
        outside.insert(new, new);
      }
    }
    f.block_mut(b).insts = keep;
  }
}

/// A block every entry into the loop passes through on its way to the
/// header, with the header's parameters, placed on every edge from
/// outside the loop.
fn make_preheader(f: &mut Func, header: BlockId, body: &FxHashSet<BlockId>) -> BlockId {
  let preds = f.predecessors();
  let outside_preds: Vec<BlockId> = preds[header.0 as usize]
    .iter()
    .copied()
    .filter(|p| !body.contains(p))
    .collect();

  let pre = f.add_block_like(header);
  let tys: Vec<Ty> = f.block(header).params.iter().map(|&p| f.ty(p)).collect();
  let mut pre_params = Vec::with_capacity(tys.len());
  for ty in tys {
    pre_params.push(f.add_block_param(pre, ty));
  }
  f.block_mut(pre).param_regs = f.block(header).param_regs.clone();
  f.block_mut(pre).fixed_regs = f.block(header).fixed_regs.clone();
  f.set_term(
    pre,
    Terminator::Jump {
      target: header,
      args: pre_params,
    },
  );

  let mut seen = Vec::new();
  for p in outside_preds {
    if seen.contains(&p) {
      continue;
    }
    seen.push(p);
    f.retarget(p, header, pre);
  }
  pre
}

pub(super) fn is_pure(op: &Op) -> bool {
  matches!(
    op,
    Op::ConstTagged(_)
      | Op::ConstF64(_)
      | Op::ConstI64(_)
      | Op::ConstBool(_)
      | Op::BoxF64
      | Op::BoxBool
      | Op::IntToF64
      | Op::F64ToI64
      | Op::UnboxF64
      | Op::UnboxBool
      | Op::ObjPtr
      | Op::FAdd
      | Op::FSub
      | Op::FMul
      | Op::FDiv
      | Op::FNeg
      | Op::FFloorDiv
      | Op::FCmp(_)
      | Op::IAdd
      | Op::ISub
      | Op::IMul
      | Op::ICmp(_)
      | Op::BNot
      | Op::EqConst(_)
      | Op::FPow
      | Op::FUnary(_)
      | Op::FMax
      | Op::FMin
      | Op::FTest(_)
      | Op::FCall(_)
  )
}
