//! Local rewrites that need nothing but the operation and what defines
//! its operands: boxing that is undone straight away, a guard on a value
//! whose construction already proves it, a double negation, a branch on a
//! negated condition.
//!
//! Numbers go the same way. An integer made a double and checked back
//! into an integer is the integer it started as, when the double holds it
//! exactly; a comparison of two such doubles is a comparison of the
//! integers; `==` on two boxed numbers compares the numbers; and a check
//! the operands' ranges already decide is gone.
//!
//! Frame states also name the unboxed value behind a box rather than the
//! box itself. Leaving compiled code boxes every state value on the way
//! out anyway, so a box that only a frame state used no longer runs on
//! the fast path at all.

use rustc_hash::FxHashSet;

use super::ints::{self, EXACT};
use super::replace_uses;
use crate::jit::ir::{BlockId, Cmp, Func, GuardKind, InstId, Op, Terminator, Ty, ValueId};

pub fn run(f: &mut Func) {
  unbox_states(f);
  let exact = ints::exact_integers(f);
  loop {
    let mut changed = false;
    for bi in 0..f.blocks.len() {
      changed |= rewrite_block(f, BlockId(bi as u32), &exact);
      // Whatever folds is pure and has nothing left using it, so it goes
      // from the block there and then, as does a check that cannot fail.
      let insts = std::mem::take(&mut f.blocks[bi].insts);
      let mut keep = Vec::with_capacity(insts.len());
      for id in insts {
        if always_passes(f, id) {
          changed = true;
          continue;
        }
        match fold(f, id, &exact) {
          Some((from, to)) => {
            replace_uses(f, from, to);
            changed = true;
          },
          None => keep.push(id),
        }
      }
      f.blocks[bi].insts = keep;
      changed |= branch_on_not(f, bi);
    }
    if !changed {
      return;
    }
  }
}

fn unbox_states(f: &mut Func) {
  // An integer converted for the box goes too: leaving compiled code
  // converts an `I64` state value the same way.
  let unboxed = |f: &Func, v: ValueId| {
    let v = match f.def_inst(v) {
      Some(d) if matches!(d.op, Op::BoxF64 | Op::BoxBool) => d.args[0],
      _ => v,
    };
    match f.def_inst(v) {
      Some(d) if matches!(d.op, Op::IntToF64) => d.args[0],
      _ => v,
    }
  };
  for i in 0..f.insts.len() {
    let Some(state) = &f.insts[i].state else {
      continue;
    };
    let regs: Vec<(u8, ValueId)> = state
      .regs
      .iter()
      .map(|&(r, v)| (r, unboxed(f, v)))
      .collect();
    f.insts[i].state.as_mut().unwrap().regs = regs;
  }
  for b in 0..f.blocks.len() {
    let Terminator::Deopt(state) = &f.blocks[b].term else {
      continue;
    };
    let regs: Vec<(u8, ValueId)> = state
      .regs
      .iter()
      .map(|&(r, v)| (r, unboxed(f, v)))
      .collect();
    if let Terminator::Deopt(state) = &mut f.blocks[b].term {
      state.regs = regs;
    }
  }
}

/// The value an operation can be replaced by, when it is one it already
/// has.
fn fold(f: &Func, id: InstId, exact: &FxHashSet<ValueId>) -> Option<(ValueId, ValueId)> {
  let inst = f.inst(id);
  let result = inst.result?;
  let arg = *inst.args.first()?;
  let def = f.def_inst(arg).map(|d| (&d.op, d.args.first().copied()));
  let replacement = match (&inst.op, def) {
    // Unboxing what was just boxed, or guarding a number that was built
    // as one.
    (Op::UnboxF64 | Op::Guard(GuardKind::Number), Some((Op::BoxF64, Some(x)))) => x,
    (Op::UnboxBool | Op::Guard(GuardKind::Bool), Some((Op::BoxBool, Some(x)))) => x,
    (Op::BNot, Some((Op::BNot, Some(x)))) => x,
    // An integer check, or the bitwise operators' conversion, on an
    // integer a double holds exactly, made a double, boxed or not.
    (Op::Guard(GuardKind::Whole | GuardKind::Int) | Op::WrapI64, _) => {
      let x = converted(f, arg)?;
      if !exact.contains(&x) {
        return None;
      }
      x
    },
    // An index that is never negative counts from the front already.
    (Op::WrapIndex, _) if ints::range(f, arg).is_some_and(|(lo, _)| lo >= 0) => arg,
    _ => return None,
  };
  (replacement != result).then_some((result, replacement))
}

/// The integer `v` is the double of, looking through a box.
fn converted(f: &Func, v: ValueId) -> Option<ValueId> {
  let mut d = f.def_inst(v)?;
  if matches!(d.op, Op::BoxF64) {
    d = f.def_inst(d.args[0])?;
  }
  matches!(d.op, Op::IntToF64).then(|| d.args[0])
}

/// A `Guard(True)` of a constant `true`.
fn always_passes(f: &Func, id: InstId) -> bool {
  let inst = f.inst(id);
  matches!(inst.op, Op::Guard(GuardKind::True))
    && f
      .def_inst(inst.args[0])
      .is_some_and(|d| matches!(d.op, Op::ConstBool(true)))
}

/// Rewrites comparisons in `b` into cheaper ones: numbers compared as
/// the integers they were made from, `==` on boxed numbers as a number
/// comparison, and an integer comparison the operands' ranges decide as
/// its answer. Returns whether anything changed.
fn rewrite_block(f: &mut Func, b: BlockId, exact: &FxHashSet<ValueId>) -> bool {
  let mut changed = false;
  let mut pos = 0;
  while pos < f.block(b).insts.len() {
    let id = f.block(b).insts[pos];
    let inst = f.inst(id).clone();
    let rewritten: Option<(Op, Vec<ValueId>)> = match inst.op {
      Op::FCmp(cmp) => {
        let x = as_integer(f, inst.args[0], exact);
        let y = as_integer(f, inst.args[1], exact);
        match (x, y) {
          // Two constants are left for the backend to fold.
          (Some(Integer::Value(a)), Some(y)) => {
            let b_ = materialize(f, b, &mut pos, y);
            Some((Op::ICmp(cmp), vec![a, b_]))
          },
          (Some(x), Some(Integer::Value(c))) => {
            let a = materialize(f, b, &mut pos, x);
            Some((Op::ICmp(cmp), vec![a, c]))
          },
          _ => None,
        }
      },
      Op::TaggedEq => {
        let x = as_number(f, inst.args[0]);
        let y = as_number(f, inst.args[1]);
        match (x, y) {
          (Some(Number::Value(a)), Some(y)) => {
            let c = number_value(f, b, &mut pos, y);
            Some((Op::FCmp(Cmp::Eq), vec![a, c]))
          },
          (Some(x), Some(Number::Value(c))) => {
            let a = number_value(f, b, &mut pos, x);
            Some((Op::FCmp(Cmp::Eq), vec![a, c]))
          },
          _ => None,
        }
      },
      Op::EqConst(k) => match as_number(f, inst.args[0]) {
        Some(Number::Value(a)) => {
          let c = number_value(f, b, &mut pos, Number::Const(k));
          Some((Op::FCmp(Cmp::Eq), vec![a, c]))
        },
        _ => None,
      },
      Op::ICmp(cmp) => {
        decided(f, cmp, inst.args[0], inst.args[1]).map(|v| (Op::ConstBool(v), vec![]))
      },
      _ => None,
    };
    if let Some((op, args)) = rewritten {
      // None of these can leave compiled code, and the state a
      // `TaggedEq` carried for its `@eq` exit would only keep values
      // alive for nothing.
      let slot = &mut f.insts[id.0 as usize];
      slot.op = op;
      slot.args = args;
      slot.state = None;
      changed = true;
    }
    pos += 1;
  }
  changed
}

/// An `F64` operand as an integer comparison can take it.
enum Integer {
  /// An exact integer the double was made from.
  Value(ValueId),
  /// A whole constant a double holds exactly.
  Const(i64),
}

fn as_integer(f: &Func, v: ValueId, exact: &FxHashSet<ValueId>) -> Option<Integer> {
  let d = f.def_inst(v)?;
  match d.op {
    Op::IntToF64 if exact.contains(&d.args[0]) => Some(Integer::Value(d.args[0])),
    Op::ConstF64(c) if c.fract() == 0.0 && c.abs() <= EXACT as f64 => {
      Some(Integer::Const(c as i64))
    },
    _ => None,
  }
}

/// `x` as a value, placing a constant just before position `pos` of `b`.
fn materialize(f: &mut Func, b: BlockId, pos: &mut usize, x: Integer) -> ValueId {
  match x {
    Integer::Value(v) => v,
    Integer::Const(c) => {
      let v = f
        .insert(b, *pos, Op::ConstI64(c), vec![], Some(Ty::I64), None)
        .unwrap();
      *pos += 1;
      v
    },
  }
}

/// A `Tagged` operand known to be a number.
enum Number {
  /// The boxed double.
  Value(ValueId),
  Const(f64),
}

fn as_number(f: &Func, v: ValueId) -> Option<Number> {
  let d = f.def_inst(v)?;
  match d.op {
    Op::BoxF64 => Some(Number::Value(d.args[0])),
    Op::ConstTagged(bits) => {
      let value = crate::vm::value::Value::from_bits(bits);
      value.is_number().then(|| Number::Const(value.as_number()))
    },
    _ => None,
  }
}

fn number_value(f: &mut Func, b: BlockId, pos: &mut usize, x: Number) -> ValueId {
  match x {
    Number::Value(v) => v,
    Number::Const(c) => {
      let v = f
        .insert(b, *pos, Op::ConstF64(c), vec![], Some(Ty::F64), None)
        .unwrap();
      *pos += 1;
      v
    },
  }
}

/// The answer to `x cmp y` on integers, when their ranges leave only one.
fn decided(f: &Func, cmp: Cmp, x: ValueId, y: ValueId) -> Option<bool> {
  let (xl, xh) = ints::range(f, x)?;
  let (yl, yh) = ints::range(f, y)?;
  match cmp {
    Cmp::Lt if xh < yl => Some(true),
    Cmp::Lt if xl >= yh => Some(false),
    Cmp::Le if xh <= yl => Some(true),
    Cmp::Le if xl > yh => Some(false),
    Cmp::Gt if xl > yh => Some(true),
    Cmp::Gt if xh <= yl => Some(false),
    Cmp::Ge if xl >= yh => Some(true),
    Cmp::Ge if xh < yl => Some(false),
    Cmp::Eq if xh < yl || xl > yh => Some(false),
    Cmp::Ne if xh < yl || xl > yh => Some(true),
    _ => None,
  }
}

/// `branch(not c) a b` is `branch(c) b a`.
fn branch_on_not(f: &mut Func, bi: usize) -> bool {
  let Terminator::Branch { cond, .. } = &f.blocks[bi].term else {
    return false;
  };
  let Some(def) = f.def_inst(*cond) else {
    return false;
  };
  if !matches!(def.op, Op::BNot) {
    return false;
  }
  let inner = def.args[0];
  let Terminator::Branch {
    cond,
    then_block,
    then_args,
    else_block,
    else_args,
  } = std::mem::replace(&mut f.blocks[bi].term, Terminator::Unset)
  else {
    unreachable!()
  };
  let _ = cond;
  f.blocks[bi].term = Terminator::Branch {
    cond: inner,
    then_block: else_block,
    then_args: else_args,
    else_block: then_block,
    else_args: then_args,
  };
  true
}
