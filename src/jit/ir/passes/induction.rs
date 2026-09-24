//! Loop counters and the checks that depend on them.
//!
//! A counter is a header parameter that starts from some number, moves by
//! the same small whole step on every back edge, and is what the header
//! tests to decide whether to go round again. Inside the loop such a
//! counter is a whole number lying between its start and the bound it was
//! tested against, so once the start is known to be whole and the bound
//! known to be in range, every `Guard(Int)` on the counter is an exact
//! conversion that cannot fail.
//!
//! A list index that is the counter needs no bounds check either when the
//! counter can be shown to stay within the list: when it starts at zero or
//! more, counts up, and is tested against the list's own length, or
//! against a bound that is checked once to be no more than that length.
//! Counting down, the start is checked once against the length instead,
//! and the bound the counter stays above keeps it from going negative.
//!
//! The checks that make all of this hold run once, in the preheader, as
//! guards that leave at the loop header. A loop that fails one resumes in
//! the interpreter from its start, and the header is then a blocked site,
//! so the function's next compile leaves this loop alone.

use rustc_hash::FxHashSet;

use super::loops::{self, Loop};
use super::replace_uses;
use crate::jit::ir::build::Feedback;
use crate::jit::ir::{
  BlockId, Cmp, FrameState, Func, GuardKind, InstId, Op, Terminator, Ty, ValueDef, ValueId,
};

/// The largest step a counter may take. Anything whole up to this keeps
/// the counter exact for as long as the bound keeps it within 2^53.
const MAX_STEP: f64 = (1u64 << 20) as f64;

/// The magnitude a counter's start and bound are held to. Every value
/// the counter takes, up to one step past the bound, then stays below
/// 2^53, where a float counts whole numbers exactly, so the integer
/// form and the float the interpreter would compute always agree.
const EXACT_LIMIT: f64 = (1u64 << 52) as f64;

pub fn run(f: &mut Func, feedback: &Feedback) {
  if feedback.sites_off {
    return;
  }
  for lp in loops::find(f) {
    let Some(ip) = f.block(lp.header).ip else {
      continue;
    };
    if feedback.blocked.contains(&ip) {
      continue;
    }
    if let Some(counter) = counter_of(f, &lp) {
      rewrite(f, &lp, &counter, ip);
    }
  }
}

/// A loop's counter, as its header tests it.
struct Counter {
  /// The header parameter.
  param: ValueId,
  /// Its position among the header's parameters.
  index: usize,
  /// Whether it counts up.
  up: bool,
  /// How the header compares it to `bound` to stay in the loop, with the
  /// counter on the left.
  cmp: Cmp,
  /// What it is compared against, defined outside the loop.
  bound: ValueId,
  /// The preheader, whose jump to the header gives the counter its start.
  preheader: BlockId,
}

fn counter_of(f: &Func, lp: &Loop) -> Option<Counter> {
  let header = lp.header;
  let preds = f.predecessors();

  // One way in from outside, the preheader licm made.
  let mut outside: Vec<BlockId> = preds[header.0 as usize]
    .iter()
    .copied()
    .filter(|p| !lp.body.contains(p))
    .collect();
  outside.dedup();
  let [preheader] = outside[..] else {
    return None;
  };
  if !matches!(f.block(preheader).term, Terminator::Jump { target, .. } if target == header) {
    return None;
  }

  // The header decides whether to go round again, and the way back in
  // is entered from the header alone, so everything else in the loop
  // runs only while the test holds.
  let Terminator::Branch {
    cond,
    then_block,
    else_block,
    ..
  } = f.block(header).term
  else {
    return None;
  };
  let (inside, stays_when_true) =
    match (lp.body.contains(&then_block), lp.body.contains(&else_block)) {
      (true, false) => (then_block, true),
      (false, true) => (else_block, false),
      _ => return None,
    };
  if preds[inside.0 as usize] != [header] {
    return None;
  }

  let test = f.def_inst(cond)?;
  let Op::FCmp(cmp) = test.op else {
    return None;
  };
  // Negating a float comparison is only right when neither side is NaN.
  // The counter never is, and the bound's range check fails for NaN.
  let cmp = if stays_when_true { cmp } else { negate(cmp) };
  let (lhs, rhs) = (test.args[0], test.args[1]);
  let params = &f.block(header).params;
  let (param, cmp, bound) = if params.contains(&lhs) {
    (lhs, cmp, rhs)
  } else if params.contains(&rhs) {
    (rhs, swap(cmp), lhs)
  } else {
    return None;
  };
  if f.ty(param) != Ty::F64 || !defined_outside(f, lp, bound) {
    return None;
  }
  let index = params.iter().position(|&p| p == param)?;

  // Every back edge moves the counter by a whole step, all one way.
  let mut up = None;
  for &latch in &lp.latches {
    let edges = edge_values(f, latch, header, index);
    if edges.is_empty() {
      return None;
    }
    for v in edges {
      let step = step_of(f, v, param)?;
      match up {
        None => up = Some(step > 0.0),
        Some(u) if u == (step > 0.0) => {},
        Some(_) => return None,
      }
    }
  }
  let up = up?;
  let fits = match cmp {
    Cmp::Lt | Cmp::Le => up,
    Cmp::Gt | Cmp::Ge => !up,
    Cmp::Eq | Cmp::Ne => false,
  };
  fits.then_some(Counter {
    param,
    index,
    up,
    cmp,
    bound,
    preheader,
  })
}

fn rewrite(f: &mut Func, lp: &Loop, c: &Counter, header_ip: usize) {
  // The integer checks on the counter, in blocks the header's test
  // covers.
  let mut guards: Vec<InstId> = Vec::new();
  for &b in &lp.body {
    if b == lp.header {
      continue;
    }
    for &i in &f.block(b).insts {
      let inst = f.inst(i);
      if matches!(inst.op, Op::Guard(GuardKind::Int)) && strip_box(f, inst.args[0]) == c.param {
        guards.push(i);
      }
    }
  }
  if guards.is_empty() {
    return;
  }

  // The step each back edge takes, read before anything changes.
  let mut steps: Vec<(BlockId, Vec<i64>)> = Vec::new();
  for &latch in &lp.latches {
    let edge_steps = edge_values(f, latch, lp.header, c.index)
      .into_iter()
      .map(|v| step_of(f, v, c.param).expect("counter_of checked every step") as i64)
      .collect();
    steps.push((latch, edge_steps));
  }

  let mut pre = Preheader::new(f, c, header_ip);

  // The start is a whole number, and it and the bound keep every value
  // exact.
  let start = pre.start;
  let start_int = pre.guard_value(f, GuardKind::Int, vec![start], Ty::I64);
  let low = pre.push(f, Op::ConstI64(-(EXACT_LIMIT as i64)), vec![], Ty::I64);
  let high = pre.push(f, Op::ConstI64(EXACT_LIMIT as i64), vec![], Ty::I64);
  let above = pre.push(f, Op::ICmp(Cmp::Ge), vec![start_int, low], Ty::Bool);
  pre.guard(f, GuardKind::True, vec![above]);
  let below = pre.push(f, Op::ICmp(Cmp::Le), vec![start_int, high], Ty::Bool);
  pre.guard(f, GuardKind::True, vec![below]);
  if !bound_in_range(f, c) {
    let (cmp, limit) = if c.up {
      (Cmp::Le, EXACT_LIMIT)
    } else {
      (Cmp::Ge, -EXACT_LIMIT)
    };
    let limit = pre.push(f, Op::ConstF64(limit), vec![], Ty::F64);
    let ok = pre.push(f, Op::FCmp(cmp), vec![pre.bound, limit], Ty::Bool);
    pre.guard(f, GuardKind::True, vec![ok]);
  }

  // The counter's integer twin: a parameter of its own, starting from
  // the checked start and stepping alongside it on every back edge.
  let header = lp.header;
  let twin = f.add_block_param(header, Ty::I64);
  for args in f.edge_args_mut(c.preheader, header) {
    args.push(start_int);
  }
  for (latch, edge_steps) in steps {
    for (e, step) in edge_steps.into_iter().enumerate() {
      let pos = f.block(latch).insts.len();
      let k = f.insert(latch, pos, Op::ConstI64(step), vec![], Some(Ty::I64), None).unwrap();
      let next = f
        .insert(latch, pos + 1, Op::IAdd, vec![twin, k], Some(Ty::I64), None)
        .unwrap();
      f.edge_args_mut(latch, header)[e].push(next);
    }
  }

  // Every integer check on the counter is the twin, and every use of the
  // counter as a number reads the twin converted. The float parameter is
  // left feeding only itself, and dead code removal takes it.
  for g in guards {
    let result = f.inst(g).result.expect("an integer guard has a result");
    replace_uses(f, result, twin);
    for b in 0..f.blocks.len() {
      f.blocks[b].insts.retain(|&i| i != g);
    }
  }
  let as_float = f.insert(header, 0, Op::IntToF64, vec![twin], Some(Ty::F64), None).unwrap();
  replace_uses(f, c.param, as_float);
  // A frame state holds the integer itself: leaving compiled code boxes
  // it as a number anyway, so the conversion only runs where something
  // really computes with the counter.
  let to_twin = |v: &mut ValueId| {
    if *v == as_float {
      *v = twin;
    }
  };
  for inst in &mut f.insts {
    if let Some(state) = &mut inst.state {
      state.regs.iter_mut().for_each(|(_, v)| to_twin(v));
    }
  }
  for b in &mut f.blocks {
    if let Terminator::Deopt(state) = &mut b.term {
      state.regs.iter_mut().for_each(|(_, v)| to_twin(v));
    }
    b.fixed_regs.iter_mut().for_each(|(_, v)| to_twin(v));
  }
  integer_test(f, header, as_float, twin);

  let indices: FxHashSet<ValueId> = FxHashSet::from_iter([twin]);
  remove_bounds_checks(f, lp, c, &mut pre, start_int, &indices);
}

/// Turns the header's test into an integer comparison when the bound is
/// a list length: the counter and the length are both whole and far
/// below 2^53, so comparing them as integers is the same test.
fn integer_test(f: &mut Func, header: BlockId, as_float: ValueId, twin: ValueId) {
  let Terminator::Branch { cond, .. } = f.block(header).term else {
    return;
  };
  let Some(test) = f.def_inst_id(cond) else {
    return;
  };
  let Op::FCmp(cmp) = f.inst(test).op else {
    return;
  };
  let args = f.inst(test).args.clone();
  let length_of = |f: &Func, v: ValueId| {
    let d = f.def_inst(v)?;
    if !matches!(d.op, Op::IntToF64) {
      return None;
    }
    let len = d.args[0];
    f.def_inst(len).is_some_and(|l| matches!(l.op, Op::ListLen)).then_some(len)
  };
  let new_args = if args[0] == as_float {
    length_of(f, args[1]).map(|len| vec![twin, len])
  } else if args[1] == as_float {
    length_of(f, args[0]).map(|len| vec![len, twin])
  } else {
    None
  };
  if let Some(new_args) = new_args {
    let inst = &mut f.insts[test.0 as usize];
    inst.op = Op::ICmp(cmp);
    inst.args = new_args;
  }
}

/// Drops the bounds checks on the counter that the loop's test, and at
/// most one check per list length in the preheader, already cover.
fn remove_bounds_checks(
  f: &mut Func,
  lp: &Loop,
  c: &Counter,
  pre: &mut Preheader,
  start_int: ValueId,
  indices: &FxHashSet<ValueId>,
) {
  let mut non_negative = false;
  let mut covered: Vec<ValueId> = Vec::new();
  let blocks: Vec<BlockId> = lp.body.iter().copied().filter(|&b| b != lp.header).collect();
  for b in blocks {
    let mut keep = Vec::with_capacity(f.block(b).insts.len());
    for i in f.block(b).insts.clone() {
      let inst = f.inst(i);
      let is_check = matches!(inst.op, Op::Guard(GuardKind::Bounds)) && indices.contains(&inst.args[0]);
      if !is_check {
        keep.push(i);
        continue;
      }
      let len = inst.args[1];
      if !covered.contains(&len) {
        if !defined_outside(f, lp, len) {
          keep.push(i);
          continue;
        }
        if !non_negative {
          ensure_non_negative(f, c, pre, start_int);
          non_negative = true;
        }
        ensure_below(f, c, pre, start_int, len);
        covered.push(len);
      }
    }
    f.block_mut(b).insts = keep;
  }
}

/// Makes the counter never negative inside the loop.
fn ensure_non_negative(f: &mut Func, c: &Counter, pre: &mut Preheader, start_int: ValueId) {
  if c.up {
    // Counting up from zero or more.
    let zero = pre.push(f, Op::ConstI64(0), vec![], Ty::I64);
    let ok = pre.push(f, Op::ICmp(Cmp::Ge), vec![start_int, zero], Ty::Bool);
    pre.guard(f, GuardKind::True, vec![ok]);
  } else {
    // Counting down, staying above a bound of -1 or more, or at or above
    // one of 0 or more.
    let floor = if c.cmp == Cmp::Gt { -1.0 } else { 0.0 };
    let floor = pre.push(f, Op::ConstF64(floor), vec![], Ty::F64);
    let ok = pre.push(f, Op::FCmp(Cmp::Ge), vec![pre.bound, floor], Ty::Bool);
    pre.guard(f, GuardKind::True, vec![ok]);
  }
}

/// Makes the counter always below `len` inside the loop.
fn ensure_below(f: &mut Func, c: &Counter, pre: &mut Preheader, start_int: ValueId, len: ValueId) {
  if !c.up {
    // Counting down, the start is the most it ever is.
    pre.guard(f, GuardKind::Bounds, vec![start_int, len]);
    return;
  }
  // Counting up while below the list's own length needs nothing more.
  let is_len = f
    .def_inst(c.bound)
    .is_some_and(|d| matches!(d.op, Op::IntToF64) && d.args[0] == len);
  if c.cmp == Cmp::Lt && is_len {
    return;
  }
  // Otherwise the bound is checked against the length once.
  let len_f = pre.push(f, Op::IntToF64, vec![len], Ty::F64);
  let cmp = if c.cmp == Cmp::Lt { Cmp::Le } else { Cmp::Lt };
  let ok = pre.push(f, Op::FCmp(cmp), vec![pre.bound, len_f], Ty::Bool);
  pre.guard(f, GuardKind::True, vec![ok]);
}

/// Inserts operations at the end of the preheader, where every entry into
/// the loop passes once.
struct Preheader {
  block: BlockId,
  /// The counter's value on entry.
  start: ValueId,
  /// The bound as the preheader sees it.
  bound: ValueId,
  state: FrameState,
}

impl Preheader {
  fn new(f: &Func, c: &Counter, header_ip: usize) -> Self {
    let Terminator::Jump { args, .. } = &f.block(c.preheader).term else {
      unreachable!("the preheader ends in a jump to the header");
    };
    // The preheader's own parameters hold every register live on entry
    // to the loop. The jump can carry fewer, once a header parameter
    // that never changes has been folded away.
    let state = FrameState {
      ip: header_ip,
      regs: f.entry_regs(c.preheader, &f.block(c.preheader).params),
    };
    Self {
      block: c.preheader,
      start: args[c.index],
      bound: c.bound,
      state,
    }
  }

  fn push(&mut self, f: &mut Func, op: Op, args: Vec<ValueId>, ty: Ty) -> ValueId {
    let pos = f.block(self.block).insts.len();
    f.insert(self.block, pos, op, args, Some(ty), None).unwrap()
  }

  fn guard(&mut self, f: &mut Func, kind: GuardKind, args: Vec<ValueId>) {
    let pos = f.block(self.block).insts.len();
    f.insert(self.block, pos, Op::Guard(kind), args, None, Some(self.state.clone()));
  }

  fn guard_value(&mut self, f: &mut Func, kind: GuardKind, args: Vec<ValueId>, ty: Ty) -> ValueId {
    let pos = f.block(self.block).insts.len();
    f.insert(self.block, pos, Op::Guard(kind), args, Some(ty), Some(self.state.clone()))
      .unwrap()
  }
}

/// Whether the bound is already known to keep the counter within
/// `EXACT_LIMIT`.
fn bound_in_range(f: &Func, c: &Counter) -> bool {
  let Some(def) = f.def_inst(c.bound) else {
    return false;
  };
  match def.op {
    Op::ConstF64(x) => x.abs() <= EXACT_LIMIT,
    // A list's length.
    Op::IntToF64 => f.def_inst(def.args[0]).is_some_and(|d| matches!(d.op, Op::ListLen)),
    _ => false,
  }
}

/// The step `v` takes from `param`, when `v` is `param` plus or minus a
/// whole constant within `MAX_STEP`.
fn step_of(f: &Func, v: ValueId, param: ValueId) -> Option<f64> {
  let def = f.def_inst(v)?;
  let (base, step) = match def.op {
    Op::FAdd => match (constant(f, def.args[0]), constant(f, def.args[1])) {
      (_, Some(s)) => (def.args[0], s),
      (Some(s), None) => (def.args[1], s),
      _ => return None,
    },
    Op::FSub => (def.args[0], -constant(f, def.args[1])?),
    _ => return None,
  };
  let whole = step != 0.0 && step.fract() == 0.0 && step.abs() <= MAX_STEP;
  (whole && as_counter(f, base) == param).then_some(step)
}

/// `v` with the integer round trip the builder puts on an index taken
/// off: `IntToF64(Guard(Int)(x))` is `x` for the counter.
fn as_counter(f: &Func, v: ValueId) -> ValueId {
  let Some(def) = f.def_inst(v) else {
    return v;
  };
  if !matches!(def.op, Op::IntToF64) {
    return v;
  }
  match f.def_inst(def.args[0]) {
    Some(g) if matches!(g.op, Op::Guard(GuardKind::Int)) => strip_box(f, g.args[0]),
    _ => v,
  }
}

/// `x` for `BoxF64(x)`, otherwise `v` itself.
fn strip_box(f: &Func, v: ValueId) -> ValueId {
  match f.def_inst(v) {
    Some(d) if matches!(d.op, Op::BoxF64) => d.args[0],
    _ => v,
  }
}

fn constant(f: &Func, v: ValueId) -> Option<f64> {
  match f.def_inst(v)?.op {
    Op::ConstF64(x) => Some(x),
    _ => None,
  }
}

fn defined_outside(f: &Func, lp: &Loop, v: ValueId) -> bool {
  match f.values[v.0 as usize].def {
    ValueDef::Param(b, _) => !lp.body.contains(&b),
    ValueDef::Inst(i) => f
      .blocks
      .iter()
      .enumerate()
      .find(|(_, b)| b.insts.contains(&i))
      .is_none_or(|(bi, _)| !lp.body.contains(&BlockId(bi as u32))),
  }
}

/// The values `from` passes as parameter `k` of `to`, one per edge.
fn edge_values(f: &Func, from: BlockId, to: BlockId, k: usize) -> Vec<ValueId> {
  match &f.block(from).term {
    Terminator::Jump { target, args } if *target == to => vec![args[k]],
    Terminator::Branch {
      then_block,
      then_args,
      else_block,
      else_args,
      ..
    } => {
      let mut v = Vec::new();
      if *then_block == to {
        v.push(then_args[k]);
      }
      if *else_block == to {
        v.push(else_args[k]);
      }
      v
    },
    _ => Vec::new(),
  }
}

fn negate(c: Cmp) -> Cmp {
  match c {
    Cmp::Lt => Cmp::Ge,
    Cmp::Le => Cmp::Gt,
    Cmp::Gt => Cmp::Le,
    Cmp::Ge => Cmp::Lt,
    Cmp::Eq => Cmp::Ne,
    Cmp::Ne => Cmp::Eq,
  }
}

fn swap(c: Cmp) -> Cmp {
  match c {
    Cmp::Lt => Cmp::Gt,
    Cmp::Le => Cmp::Ge,
    Cmp::Gt => Cmp::Lt,
    Cmp::Ge => Cmp::Le,
    other => other,
  }
}
