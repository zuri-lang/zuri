//! Integer arithmetic that only ever becomes a double.
//!
//! The builder does arithmetic on integers wherever a site has only seen
//! whole numbers. That pays when the result is an index, a loop counter or
//! a comparison operand: integer operations are cheap and the checks that
//! keep them exact are few. When every result of a run of such operations
//! goes straight back to a double, as in `1.0 / ((i + j) * (i + j + 1))`,
//! the run pays for its overflow checks and its conversions and gains
//! nothing. Double arithmetic gives the interpreter's answer exactly, so
//! such a run is done on doubles instead. A whole-number check whose only
//! purpose was feeding it goes with it, down to the number check it
//! contains when its value is not already known to be one.

use rustc_hash::{FxHashMap, FxHashSet};

use super::replace_all;
use crate::jit::ir::{Func, GuardKind, InstId, IntOp, Op, Terminator, Ty, ValueId};

/// Returns whether anything changed.
pub fn run(f: &mut Func) -> bool {
  let mut candidates = find_candidates(f);
  if candidates.is_empty() {
    return false;
  }
  settle(f, &mut candidates);
  if candidates.is_empty() {
    return false;
  }
  rewrite(f, &candidates);
  true
}

/// Every integer arithmetic guard and every whole-number guard.
fn find_candidates(f: &Func) -> FxHashSet<ValueId> {
  let mut out = FxHashSet::default();
  for b in &f.blocks {
    for &i in &b.insts {
      let inst = f.inst(i);
      let Some(v) = inst.result else {
        continue;
      };
      match inst.op {
        Op::Guard(GuardKind::Arith(_)) => {
          out.insert(v);
        },
        Op::Guard(GuardKind::Whole) => {
          out.insert(v);
        },
        _ => {},
      }
    }
  }
  out
}

/// Drops every candidate something other than a conversion to a double,
/// another remaining candidate or a frame state uses, until none is left
/// to drop.
fn settle(f: &Func, candidates: &mut FxHashSet<ValueId>) {
  // Who uses each candidate: an instruction, or `None` for a use that is
  // never float-bound, such as a branch condition or an edge argument.
  let mut users: FxHashMap<ValueId, Vec<Option<InstId>>> = FxHashMap::default();
  for b in &f.blocks {
    for &i in &b.insts {
      for a in &f.inst(i).args {
        if candidates.contains(a) {
          users.entry(*a).or_default().push(Some(i));
        }
      }
    }
    let mut other = |v: &ValueId| {
      if candidates.contains(v) {
        users.entry(*v).or_default().push(None);
      }
    };
    match &b.term {
      Terminator::Jump { args, .. } => args.iter().for_each(&mut other),
      Terminator::Branch {
        cond,
        then_args,
        else_args,
        ..
      } => {
        other(cond);
        then_args.iter().for_each(&mut other);
        else_args.iter().for_each(&mut other);
      },
      Terminator::Return(v) => other(v),
      Terminator::Deopt(_) | Terminator::Unset => {},
    }
  }

  loop {
    let dropped: Vec<ValueId> = candidates
      .iter()
      .copied()
      .filter(|v| {
        users.get(v).into_iter().flatten().any(|u| match u {
          None => true,
          Some(i) => {
            let inst = f.inst(*i);
            match inst.op {
              Op::IntToF64 => false,
              Op::Guard(GuardKind::Arith(_)) => {
                !inst.result.is_some_and(|r| candidates.contains(&r))
              },
              _ => true,
            }
          },
        })
      })
      .collect();
    if dropped.is_empty() {
      break;
    }
    for v in dropped {
      candidates.remove(&v);
    }
  }
}

/// Replaces each remaining candidate with its double: the operation on
/// doubles for an arithmetic guard, the checked double itself for a
/// whole-number guard. Conversions of a candidate to a double become the
/// double, and frame states take it in the integer's place.
fn rewrite(f: &mut Func, candidates: &FxHashSet<ValueId>) {
  let mut double: FxHashMap<ValueId, ValueId> = FxHashMap::default();
  let mut gone: FxHashSet<InstId> = FxHashSet::default();

  for b in f.reverse_postorder() {
    let mut pos = 0;
    while pos < f.block(b).insts.len() {
      let i = f.block(b).insts[pos];
      let inst = f.inst(i).clone();
      let Some(v) = inst.result.filter(|v| candidates.contains(v)) else {
        pos += 1;
        continue;
      };
      match inst.op {
        Op::Guard(GuardKind::Arith(op)) => {
          let mut operands = [inst.args[0], inst.args[1]];
          for a in &mut operands {
            *a = match double.get(a) {
              Some(&d) => d,
              None => {
                let d = f
                  .insert(b, pos, Op::IntToF64, vec![*a], Some(Ty::F64), None)
                  .unwrap();
                pos += 1;
                d
              },
            };
          }
          let fop = match op {
            IntOp::Add => Op::FAdd,
            IntOp::Sub => Op::FSub,
            IntOp::Mul => Op::FMul,
          };
          let d = f
            .insert(b, pos, fop, operands.to_vec(), Some(Ty::F64), None)
            .unwrap();
          pos += 1;
          double.insert(v, d);
        },
        _ => {
          // A whole-number check on a double, or on a double boxed, was
          // only ever for the integers; one on anything else still has
          // to check it is a number.
          let checked = inst.args[0];
          let boxed = f
            .def_inst(checked)
            .filter(|d| matches!(d.op, Op::BoxF64))
            .map(|d| d.args[0]);
          let d = if f.ty(checked) == Ty::F64 {
            checked
          } else if let Some(unboxed) = boxed {
            unboxed
          } else {
            let d = f
              .insert(
                b,
                pos,
                Op::Guard(GuardKind::Number),
                vec![checked],
                Some(Ty::F64),
                inst.state.clone(),
              )
              .unwrap();
            pos += 1;
            d
          };
          double.insert(v, d);
        },
      }
      gone.insert(i);
      pos += 1;
    }
  }

  // A conversion of a candidate is now the candidate's double.
  let mut map = double.clone();
  for b in &f.blocks {
    for &i in &b.insts {
      let inst = f.inst(i);
      if matches!(inst.op, Op::IntToF64)
        && let Some(&d) = double.get(&inst.args[0])
      {
        map.insert(inst.result.unwrap(), d);
        gone.insert(i);
      }
    }
  }
  for b in &mut f.blocks {
    b.insts.retain(|i| !gone.contains(i));
  }
  replace_all(f, &map);
}
