//! What kind of element a list holds.
//!
//! A loop that reads numbers out of a list checks every element it
//! reads: that it is a number, or a whole number when it is used as an
//! index. When the list is the same one on every iteration and nothing in
//! the loop can put anything else into it, checking the whole list once
//! before the loop proves every one of those reads, and the per-element
//! checks go.
//!
//! Nothing in the loop may run arbitrary code, since that could store
//! anything anywhere. A store the loop makes itself is fine when the value
//! is of the claimed kind; otherwise the list stored into must be a
//! different list, which is checked once alongside the elements.
//!
//! The check walks the whole list, so it is only placed before a loop no
//! other loop encloses, and only one that leaves through its own test
//! and nowhere else. Anywhere deeper it would run once per iteration of
//! the loop around it, and a loop that can stop early, as a search does,
//! could pay for a walk far longer than the part it reads.

use rustc_hash::FxHashMap;

use super::loops::{self, Loop};
use crate::jit::ir::build::Feedback;
use crate::jit::ir::{
  BlockId, Cmp, FrameState, Func, GuardKind, InstId, Op, Terminator, Ty, ValueDef, ValueId,
};

pub fn run(f: &mut Func, feedback: &Feedback) {
  if feedback.sites_off {
    return;
  }
  let loops = loops::find(f);
  for (i, lp) in loops.iter().enumerate() {
    let nested = loops[i + 1..].iter().any(|outer| outer.body.contains(&lp.header));
    if nested {
      continue;
    }
    let Some(ip) = f.block(lp.header).ip else {
      continue;
    };
    if feedback.blocked.contains(&ip) {
      continue;
    }
    claim(f, lp, ip);
  }
}

/// What a store puts into a list, as far as the claims care.
#[derive(Clone, Copy, PartialEq)]
enum Stored {
  Whole,
  Number,
  Unknown,
}

fn claim(f: &mut Func, lp: &Loop, header_ip: usize) {
  let Some(preheader) = preheader_of(f, lp) else {
    return;
  };
  let leaves_early = lp.body.iter().any(|&b| {
    b != lp.header && f.block(b).term.successors().iter().any(|s| !lp.body.contains(s))
  });
  if leaves_early {
    return;
  }

  // The element checks each list's reads carry, and what the loop stores.
  let mut guards: FxHashMap<ValueId, Vec<InstId>> = FxHashMap::default();
  let mut stores: Vec<(ValueId, Stored)> = Vec::new();
  let mut loaded_from: FxHashMap<ValueId, ValueId> = FxHashMap::default();
  for &b in &lp.body {
    for &i in &f.block(b).insts {
      let inst = f.inst(i);
      if inst.op.may_collect() {
        return;
      }
      match inst.op {
        Op::LoadElem => {
          if let Some(list) = list_of(f, inst.args[0])
            && defined_outside(f, lp, list)
          {
            loaded_from.insert(inst.result.unwrap(), list);
          }
        },
        Op::StoreElem => stores.push((inst.args[0], stored(f, inst.args[3]))),
        Op::ListAppend => stores.push((inst.args[0], stored(f, inst.args[1]))),
        _ => {},
      }
    }
  }
  for &b in &lp.body {
    for &i in &f.block(b).insts {
      let inst = f.inst(i);
      if let Op::Guard(GuardKind::Number | GuardKind::Int) = inst.op
        && let Some(&list) = loaded_from.get(&inst.args[0])
      {
        guards.entry(list).or_default().push(i);
      }
    }
  }

  let state = FrameState {
    ip: header_ip,
    regs: f.entry_regs(preheader, &f.block(preheader).params),
    frame: f.block(preheader).frame,
    blame: None,
  };
  let mut lists: Vec<ValueId> = guards.keys().copied().collect();
  lists.sort();
  for list in lists {
    let checks = &guards[&list];
    let whole = checks
      .iter()
      .any(|&g| matches!(f.inst(g).op, Op::Guard(GuardKind::Int)));

    // Every store keeps the claim, or goes to a list checked to be a
    // different one.
    let mut distinct: Vec<ValueId> = Vec::new();
    let mut holds = true;
    for &(target, what) in &stores {
      let keeps = match what {
        Stored::Whole => true,
        Stored::Number => !whole,
        Stored::Unknown => false,
      };
      if keeps {
        continue;
      }
      if target == list || !defined_outside(f, lp, target) {
        holds = false;
        break;
      }
      if !distinct.contains(&target) {
        distinct.push(target);
      }
    }
    if !holds {
      continue;
    }

    let mut pos = f.block(preheader).insts.len();
    f.insert(
      preheader,
      pos,
      Op::Guard(GuardKind::Elems { whole }),
      vec![list],
      None,
      Some(state.clone()),
    );
    pos += 1;
    for other in distinct {
      let differ = f
        .insert(preheader, pos, Op::ICmp(Cmp::Ne), vec![list, other], Some(Ty::Bool), None)
        .unwrap();
      f.insert(
        preheader,
        pos + 1,
        Op::Guard(GuardKind::True),
        vec![differ],
        None,
        Some(state.clone()),
      );
      pos += 2;
    }

    // Each read is now known to be what its check asked for.
    for &g in checks {
      let elem = f.inst(g).args[0];
      if matches!(f.inst(g).op, Op::Guard(GuardKind::Number)) {
        let inst = &mut f.insts[g.0 as usize];
        inst.op = Op::UnboxF64;
        inst.state = None;
      } else {
        let block = block_of(f, g);
        let at = f.block(block).insts.iter().position(|&i| i == g).unwrap();
        let num = f.insert(block, at, Op::UnboxF64, vec![elem], Some(Ty::F64), None).unwrap();
        let inst = &mut f.insts[g.0 as usize];
        inst.op = Op::F64ToI64;
        inst.args = vec![num];
        inst.state = None;
      }
    }
  }
}

/// The list an element buffer belongs to.
fn list_of(f: &Func, data: ValueId) -> Option<ValueId> {
  let d = f.def_inst(data)?;
  matches!(d.op, Op::ListData).then(|| d.args[0])
}

fn stored(f: &Func, v: ValueId) -> Stored {
  match f.ty(v) {
    Ty::I64 => Stored::Whole,
    Ty::F64 => Stored::Number,
    Ty::Tagged => match f.def_inst(v).map(|d| &d.op) {
      Some(Op::BoxF64) => Stored::Number,
      Some(Op::ConstTagged(bits)) => {
        let value = crate::vm::value::Value::from_bits(*bits);
        if !value.is_number() {
          Stored::Unknown
        } else if value.as_number().fract() == 0.0 {
          Stored::Whole
        } else {
          Stored::Number
        }
      },
      _ => Stored::Unknown,
    },
    Ty::Bool | Ty::Ptr => Stored::Unknown,
  }
}

fn preheader_of(f: &Func, lp: &Loop) -> Option<BlockId> {
  let preds = f.predecessors();
  let mut outside: Vec<BlockId> = preds[lp.header.0 as usize]
    .iter()
    .copied()
    .filter(|p| !lp.body.contains(p))
    .collect();
  outside.dedup();
  let [pre] = outside[..] else {
    return None;
  };
  matches!(f.block(pre).term, Terminator::Jump { target, .. } if target == lp.header).then_some(pre)
}

fn block_of(f: &Func, i: InstId) -> BlockId {
  let bi = f.blocks.iter().position(|b| b.insts.contains(&i)).unwrap();
  BlockId(bi as u32)
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
