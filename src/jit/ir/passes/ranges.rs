//! Integers kept off the round trip through a double.
//!
//! A bitwise operator takes its operands as integers and gives back the
//! double the interpreter would hold, so a chain of them turns every
//! result into a double and straight back. The conversion loses nothing
//! while the integer stays within 2^53 of zero, and then the integer can
//! be used as it is.
//!
//! Ranges here are worked out over the whole function, loop parameters
//! included, to a fixed point. Inside a loop that is often not enough on
//! its own: `code = ((code << 2) | b) & mask` stays small exactly as long
//! as `mask` does, and `mask` comes from outside the loop, where nothing
//! bounds it. So for each loop this also tries bounds on what the loop
//! takes in from outside: the invariants it reads and the values its
//! parameters start from. When a bound makes a round trip exact, the
//! widest such bound is checked once in the preheader, by guards that
//! leave at the loop header, and the round trips inside go. A loop whose
//! check fails resumes in the interpreter from its start, and its header
//! is then a blocked site, so the function's next compile leaves it alone.

use rustc_hash::{FxHashMap, FxHashSet};

use super::ints::{self, EXACT};
use super::loops::{self, Loop};
use super::replace_uses;
use crate::jit::ir::build::Feedback;
use crate::jit::ir::{
  BitOp, BlockId, Cmp, FrameState, Func, GuardKind, Inst, InstId, IntOp, Op, Terminator, Ty,
  ValueId,
};

/// The lowest and highest value an `I64` can hold.
type Range = (i64, i64);

const FULL: Range = (i64::MIN, i64::MAX);

/// The bounds tried on a loop's inputs, widest first. Each is one less
/// than a power of two, which is what masks and packed codes look like.
const BOUNDS: [i64; 8] = [
  (1 << 52) - 1,
  (1 << 50) - 1,
  (1 << 48) - 1,
  (1 << 44) - 1,
  (1 << 40) - 1,
  (1 << 32) - 1,
  (1 << 24) - 1,
  (1 << 16) - 1,
];

/// How many times a value's range may widen before it is given up as
/// unknown, which is what keeps a counter from creeping up one step per
/// round.
const WIDENINGS: u8 = 3;

pub fn run(f: &mut Func, feedback: &Feedback) -> bool {
  let order = f.reverse_postorder();
  let preds = f.predecessors();
  let base = solve(f, &preds, &order);
  // Integers taken out of a double are exact whatever their size, and
  // `simplify` already folds their round trips. Nothing needs checking
  // for those.
  let known = ints::exact_integers(f);

  // Round trips the ranges prove on their own, wherever they are.
  let mut folds: Vec<(InstId, ValueId)> = round_trips(f, &order)
    .into_iter()
    .filter(|&(_, x)| exact(base[x.0 as usize]))
    .collect();

  let mut checks: Vec<(BlockId, usize, Vec<(ValueId, Range)>)> = Vec::new();
  if !feedback.sites_off {
    for lp in loops::find(f) {
      let Some(ip) = f.block(lp.header).ip else {
        continue;
      };
      if feedback.blocked.contains(&ip) {
        continue;
      }
      if let Some((preheader, bounds, proven)) = speculate(f, &preds, &lp, &order, &base, &known) {
        folds.extend(proven);
        checks.push((preheader, ip, bounds));
      }
    }
  }
  if folds.is_empty() {
    return false;
  }

  for (preheader, ip, bounds) in checks {
    check_bounds(f, preheader, ip, &bounds);
  }
  let mut gone: FxHashSet<InstId> = FxHashSet::default();
  for (id, x) in folds {
    if !gone.insert(id) {
      continue;
    }
    let result = f.inst(id).result.expect("a round trip has a result");
    replace_uses(f, result, x);
  }
  for b in &mut f.blocks {
    b.insts.retain(|i| !gone.contains(i));
  }
  true
}

/// The bounds that make some of `lp`'s round trips exact, with the
/// preheader to check them in and the round trips they prove.
#[allow(clippy::type_complexity)]
fn speculate(
  f: &Func,
  preds: &[Vec<BlockId>],
  lp: &Loop,
  order: &[BlockId],
  base: &[Option<Range>],
  known: &FxHashSet<ValueId>,
) -> Option<(BlockId, Vec<(ValueId, Range)>, Vec<(InstId, ValueId)>)> {
  let header = lp.header;
  let mut outside: Vec<BlockId> = preds[header.0 as usize]
    .iter()
    .copied()
    .filter(|p| !lp.body.contains(p))
    .collect();
  outside.dedup();
  let [preheader] = outside[..] else {
    return None;
  };
  let Terminator::Jump { target, args } = &f.block(preheader).term else {
    return None;
  };
  if *target != header {
    return None;
  }
  let entry_args = args.clone();

  let body: Vec<BlockId> = order
    .iter()
    .copied()
    .filter(|b| lp.body.contains(b))
    .collect();
  let trips = round_trips(f, &body);
  let unproven: Vec<(InstId, ValueId)> = trips
    .iter()
    .copied()
    .filter(|&(_, x)| !exact(base[x.0 as usize]) && !known.contains(&x))
    .collect();
  if unproven.is_empty() {
    return None;
  }

  let mut defined: FxHashSet<ValueId> = FxHashSet::default();
  for &b in &body {
    defined.extend(f.block(b).params.iter().copied());
    defined.extend(f.block(b).insts.iter().filter_map(|&i| f.inst(i).result));
  }

  // What the loop reads from outside, and whether an `&` takes it: a mask
  // wants bounding from zero, anything else either side of it.
  let mut inputs: Vec<(ValueId, bool)> = Vec::new();
  let mut masked: FxHashSet<ValueId> = FxHashSet::default();
  for &b in &body {
    for &i in &f.block(b).insts {
      let inst = f.inst(i);
      for &a in &inst.args {
        if f.ty(a) == Ty::I64 && !defined.contains(&a) {
          if matches!(inst.op, Op::IBit(BitOp::And)) {
            masked.insert(a);
          }
          if !inputs.iter().any(|&(v, _)| v == a) {
            inputs.push((a, false));
          }
        }
      }
    }
  }
  for input in &mut inputs {
    input.1 = masked.contains(&input.0);
  }
  let params: Vec<(usize, ValueId)> = f
    .block(header)
    .params
    .iter()
    .copied()
    .enumerate()
    .filter(|&(_, p)| f.ty(p) == Ty::I64)
    .collect();

  let plan = Plan {
    f,
    preds,
    body: &body,
    header,
    base,
    unproven: &unproven,
  };

  // The widest bound that proves as much as any does.
  let mut best: Option<(usize, i64)> = None;
  for &bound in &BOUNDS {
    let proven = plan.proves(&plan.assume(&inputs, bound), &FxHashSet::default());
    if proven > best.map_or(0, |(n, _)| n) {
      best = Some((proven, bound));
    }
  }
  let (proven, bound) = best?;

  // Only the bounds it cannot do without. A parameter left unchecked
  // takes its start from outside as well as what comes back round.
  let mut assumed = plan.assume(&inputs, bound);
  let mut open: FxHashSet<ValueId> = FxHashSet::default();
  for (v, _) in inputs.iter().copied() {
    let Some(r) = assumed.remove(&v) else {
      continue;
    };
    if plan.proves(&assumed, &open) < proven {
      assumed.insert(v, r);
    }
  }
  for &(_, p) in &params {
    open.insert(p);
    if plan.proves(&assumed, &open) < proven {
      open.remove(&p);
    }
  }

  let ranges = plan.solve(&assumed, &open);
  let mut bounds: Vec<(ValueId, Range)> = assumed
    .into_iter()
    .filter(|&(v, r)| !within(base[v.0 as usize], r))
    .collect();
  for &(k, p) in &params {
    if open.contains(&p) {
      continue;
    }
    let start = *entry_args.get(k)?;
    let r = ranges[p.0 as usize]?;
    if r == FULL {
      return None;
    }
    if !within(base[start.0 as usize], r) {
      bounds.push((start, r));
    }
  }
  bounds.sort_by_key(|&(v, _)| v.0);
  let folds = trips
    .into_iter()
    .filter(|&(_, x)| exact(ranges[x.0 as usize]))
    .collect();
  Some((preheader, bounds, folds))
}

/// One loop, and what bounding its inputs does for it.
struct Plan<'a> {
  f: &'a Func,
  preds: &'a [Vec<BlockId>],
  body: &'a [BlockId],
  header: BlockId,
  base: &'a [Option<Range>],
  unproven: &'a [(InstId, ValueId)],
}

impl Plan<'_> {
  /// The loop's inputs held to `bound`, where that narrows them.
  fn assume(&self, inputs: &[(ValueId, bool)], bound: i64) -> FxHashMap<ValueId, Range> {
    inputs
      .iter()
      .filter_map(|&(v, masked)| {
        let r = if masked {
          (0, bound)
        } else {
          (-bound - 1, bound)
        };
        (!within(self.base[v.0 as usize], r)).then_some((v, r))
      })
      .collect()
  }

  /// Ranges inside the loop with the inputs in `assumed` held to their
  /// bounds, and the header parameters outside `open` held to what comes
  /// back round.
  fn solve(
    &self,
    assumed: &FxHashMap<ValueId, Range>,
    open: &FxHashSet<ValueId>,
  ) -> Vec<Option<Range>> {
    let mut ranges = self.base.to_vec();
    for &b in self.body {
      for &p in &self.f.block(b).params {
        ranges[p.0 as usize] = None;
      }
      for &i in &self.f.block(b).insts {
        if let Some(v) = self.f.inst(i).result {
          ranges[v.0 as usize] = None;
        }
      }
    }
    for (&v, &r) in assumed {
      ranges[v.0 as usize] = Some(r);
    }
    run_to_fixpoint(
      self.f,
      self.preds,
      self.body,
      &mut ranges,
      Some((self.header, open)),
    );
    ranges
  }

  /// How many of the loop's unproven round trips those bounds prove.
  fn proves(&self, assumed: &FxHashMap<ValueId, Range>, open: &FxHashSet<ValueId>) -> usize {
    let ranges = self.solve(assumed, open);
    self
      .unproven
      .iter()
      .filter(|&&(_, x)| exact(ranges[x.0 as usize]))
      .count()
  }
}

/// Appends the checks on `bounds` to `preheader`, each leaving at the
/// loop header, `ip`.
fn check_bounds(f: &mut Func, preheader: BlockId, ip: usize, bounds: &[(ValueId, Range)]) {
  let params = f.block(preheader).params.clone();
  let state = FrameState {
    ip,
    regs: f.entry_regs(preheader, &params),
    frame: f.block(preheader).frame,
    blame: None,
  };
  for &(v, (lo, hi)) in bounds {
    for (cmp, limit) in [(Cmp::Ge, lo), (Cmp::Le, hi)] {
      let limit = f
        .push(preheader, Op::ConstI64(limit), vec![], Some(Ty::I64), None)
        .unwrap();
      let ok = f
        .push(
          preheader,
          Op::ICmp(cmp),
          vec![v, limit],
          Some(Ty::Bool),
          None,
        )
        .unwrap();
      f.push(
        preheader,
        Op::Guard(GuardKind::True),
        vec![ok],
        None,
        Some(state.clone()),
      );
    }
  }
}

/// Ranges of every `I64` value the blocks in `order` define.
fn solve(f: &Func, preds: &[Vec<BlockId>], order: &[BlockId]) -> Vec<Option<Range>> {
  let mut ranges: Vec<Option<Range>> = vec![None; f.values.len()];
  run_to_fixpoint(f, preds, order, &mut ranges, None);
  ranges
}

/// Settles `ranges` for the values `blocks` define. With `header` given,
/// that block's parameters outside the set take only what edges from
/// `blocks` bring them.
fn run_to_fixpoint(
  f: &Func,
  preds: &[Vec<BlockId>],
  blocks: &[BlockId],
  ranges: &mut [Option<Range>],
  header: Option<(BlockId, &FxHashSet<ValueId>)>,
) {
  let inside: FxHashSet<BlockId> = blocks.iter().copied().collect();
  let mut widened: FxHashMap<ValueId, u8> = FxHashMap::default();
  loop {
    let mut changed = false;
    for &b in blocks {
      for (k, &p) in f.block(b).params.iter().enumerate() {
        if f.ty(p) != Ty::I64 {
          continue;
        }
        let closed = header.is_some_and(|(h, open)| h == b && !open.contains(&p));
        let mut joined: Option<Range> = None;
        let mut unknown = false;
        for &pred in &preds[b.0 as usize] {
          if closed && !inside.contains(&pred) {
            continue;
          }
          for args in edge_args(f, pred, b) {
            match args.get(k).map(|a| ranges[a.0 as usize]) {
              Some(Some(r)) => joined = Some(join(joined, r)),
              Some(None) if inside.contains(&pred) => {},
              _ => unknown = true,
            }
          }
        }
        let next = if unknown { Some(FULL) } else { joined };
        changed |= update(ranges, &mut widened, p, next);
      }
      for &i in &f.block(b).insts {
        let inst = f.inst(i);
        let Some(v) = inst.result else {
          continue;
        };
        if f.ty(v) != Ty::I64 {
          continue;
        }
        let next = eval(f, inst, ranges);
        changed |= update(ranges, &mut widened, v, next);
      }
    }
    if !changed {
      break;
    }
  }
}

/// Moves `v` to `next`, giving it up as unknown once it has widened
/// too often. Whether anything changed.
fn update(
  ranges: &mut [Option<Range>],
  widened: &mut FxHashMap<ValueId, u8>,
  v: ValueId,
  next: Option<Range>,
) -> bool {
  let Some(mut next) = next else {
    return false;
  };
  let slot = &mut ranges[v.0 as usize];
  if let Some(old) = *slot {
    next = join(Some(old), next);
    if next == old {
      return false;
    }
    let n = widened.entry(v).or_insert(0);
    *n += 1;
    if *n > WIDENINGS {
      next = FULL;
    }
  }
  *slot = Some(next);
  true
}

/// The range of `inst`'s `I64` result, or `None` while an operand has
/// none yet.
fn eval(f: &Func, inst: &Inst, ranges: &[Option<Range>]) -> Option<Range> {
  let arg = |k: usize| ranges[inst.args[k].0 as usize];
  Some(match inst.op {
    Op::ConstI64(c) => (c, c),
    Op::BytesLoad | Op::StrByte => (0, 255),
    Op::ListLen | Op::BytesLen | Op::StrByteLen | Op::StrLength | Op::ObjLength => (0, EXACT),
    Op::IAdd => arith(IntOp::Add, arg(0)?, arg(1)?).unwrap_or(FULL),
    Op::ISub => arith(IntOp::Sub, arg(0)?, arg(1)?).unwrap_or(FULL),
    Op::IMul => arith(IntOp::Mul, arg(0)?, arg(1)?).unwrap_or(FULL),
    // Whatever gets through the check is inside the exact range.
    Op::Guard(GuardKind::Arith(op)) => {
      let checked = (-EXACT, EXACT - 1);
      match arith(op, arg(0)?, arg(1)?) {
        Some((lo, hi)) if lo.max(checked.0) <= hi.min(checked.1) => {
          (lo.max(checked.0), hi.min(checked.1))
        },
        _ => checked,
      }
    },
    // Anding with something never negative bounds the result whatever
    // the other side holds, which is what lets a loop's value that comes
    // back round through a mask start from the mask alone.
    Op::IBit(BitOp::And) => match (arg(0), arg(1)) {
      (Some(a), Some(b)) => bitwise(BitOp::And, a, b),
      (Some(r), None) | (None, Some(r)) if r.0 >= 0 => (0, r.1),
      _ => return None,
    },
    Op::IBit(op) => bitwise(op, arg(0)?, arg(1)?),
    // Taking back an integer made a double gives the integer itself,
    // while the double holds it exactly.
    Op::WrapI64 | Op::Guard(GuardKind::Whole | GuardKind::Int) => {
      match made_double(f, inst.args[0]) {
        Some(x) => {
          let r = ranges[x.0 as usize]?;
          if exact(Some(r)) { r } else { FULL }
        },
        None => FULL,
      }
    },
    _ => FULL,
  })
}

/// `a op b` over whole ranges, when no result can leave `i64`.
fn arith(op: IntOp, a: Range, b: Range) -> Option<Range> {
  let (a0, a1, b0, b1) = (a.0 as i128, a.1 as i128, b.0 as i128, b.1 as i128);
  let (lo, hi) = match op {
    IntOp::Add => (a0 + b0, a1 + b1),
    IntOp::Sub => (a0 - b1, a1 - b0),
    IntOp::Mul => {
      let corners = [a0 * b0, a0 * b1, a1 * b0, a1 * b1];
      (
        *corners.iter().min().unwrap(),
        *corners.iter().max().unwrap(),
      )
    },
  };
  (lo >= i64::MIN as i128 && hi <= i64::MAX as i128).then_some((lo as i64, hi as i64))
}

fn bitwise(op: BitOp, a: Range, b: Range) -> Range {
  // A shift amount the operand's width or more gives 0, so only a known
  // amount below that says where the bits go.
  let amount = (b.0 == b.1 && (0..64).contains(&b.0)).then_some(b.0 as u32);
  match op {
    // Anding with something never negative keeps no more than its bits.
    BitOp::And => match (a.0 >= 0, b.0 >= 0) {
      (true, true) => (0, a.1.min(b.1)),
      (true, false) => (0, a.1),
      (false, true) => (0, b.1),
      (false, false) => same_width(a, b),
    },
    BitOp::Or | BitOp::Xor => {
      if a.0 >= 0 && b.0 >= 0 {
        (0, ones(a.1.max(b.1)))
      } else {
        same_width(a, b)
      }
    },
    BitOp::Shl => match amount {
      Some(s) => {
        let lo = (a.0 as i128) << s;
        let hi = (a.1 as i128) << s;
        if lo >= i64::MIN as i128 && hi <= i64::MAX as i128 {
          (lo as i64, hi as i64)
        } else {
          FULL
        }
      },
      None => FULL,
    },
    BitOp::Shr => match amount {
      Some(s) => (a.0 >> s, a.1 >> s),
      None => (a.0.min(0), a.1.max(0)),
    },
    BitOp::Ushr => (0, u32::MAX as i64),
  }
}

/// The smallest range of the form `-2^n..2^n - 1` holding both, which
/// the bitwise operators keep to: every bit above `n` repeats the sign.
fn same_width(a: Range, b: Range) -> Range {
  let reach = [a.0, a.1, b.0, b.1]
    .into_iter()
    .map(|x| if x < 0 { !x } else { x })
    .max()
    .unwrap();
  let o = ones(reach);
  (-o - 1, o)
}

/// The smallest `2^n - 1` at or above `x`, for `x` not negative.
fn ones(x: i64) -> i64 {
  if x <= 0 {
    return 0;
  }
  (u64::MAX >> (x as u64).leading_zeros()) as i64
}

fn join(a: Option<Range>, b: Range) -> Range {
  match a {
    Some(a) => (a.0.min(b.0), a.1.max(b.1)),
    None => b,
  }
}

fn exact(r: Option<Range>) -> bool {
  r.is_some_and(|(lo, hi)| lo >= -EXACT && hi <= EXACT)
}

fn within(r: Option<Range>, bound: Range) -> bool {
  r.is_some_and(|(lo, hi)| lo >= bound.0 && hi <= bound.1)
}

/// The integer `v` is the double of, looking through a box.
fn made_double(f: &Func, v: ValueId) -> Option<ValueId> {
  let mut d = f.def_inst(v)?;
  if matches!(d.op, Op::BoxF64) {
    d = f.def_inst(d.args[0])?;
  }
  matches!(d.op, Op::IntToF64).then(|| d.args[0])
}

/// Every integer taken back out of a double made from it, in `blocks`:
/// the instruction, and the integer it can be replaced by.
fn round_trips(f: &Func, blocks: &[BlockId]) -> Vec<(InstId, ValueId)> {
  let mut out = Vec::new();
  for &b in blocks {
    for &i in &f.block(b).insts {
      let inst = f.inst(i);
      if matches!(
        inst.op,
        Op::WrapI64 | Op::Guard(GuardKind::Whole | GuardKind::Int)
      ) && let Some(x) = made_double(f, inst.args[0])
      {
        out.push((i, x));
      }
    }
  }
  out
}

/// The argument lists of the edges from `from` into `to`.
fn edge_args(f: &Func, from: BlockId, to: BlockId) -> Vec<&Vec<ValueId>> {
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
