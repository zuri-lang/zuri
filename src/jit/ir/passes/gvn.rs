//! Global value numbering over the dominator tree.
//!
//! An operation that computes the same thing as one dominating it is
//! replaced by that one's result. For a guard, being dominated by the same
//! check means the value has already been proven, so the second check goes
//! entirely; this is what removes the repeated `is_number` tests the
//! builder emits for a value used several times.
//!
//! Reads from memory take part too, but only while nothing could have
//! changed what they read. Each kind of memory (list headers, list
//! elements, instance fields, globals, upvalue cells and what they hold)
//! has its own version, bumped by any
//! write to it and by any operation that may run arbitrary code. A block
//! inherits the versions its immediate dominator ended with only when that
//! dominator is also its sole predecessor; any merge or loop header starts
//! fresh, so a write on some other path in is never missed. A field store
//! also makes its value available to a later read of the same field on
//! the same object.

use rustc_hash::FxHashMap;

use super::replace_uses;
use crate::jit::ir::{BlockId, Func, GuardKind, InstId, Op, ValueId, dominators};

/// The alias classes memory reads are versioned by.
#[derive(Clone, Copy, Default)]
struct Versions {
  header: u32,
  elems: u32,
  fields: u32,
  globals: u32,
  cells: u32,
  upvals: u32,
}

/// What makes two operations interchangeable, beyond their operands.
#[derive(Clone, PartialEq, Eq, Hash)]
enum Key {
  Pure(&'static str, u64),
  Guard(GuardKind),
  Header(&'static str, u32),
  Elem(u32),
  ListEnd(bool, u32, u32),
  Field(u16, u32),
  Global(u32, u32),
  Cell(u8, u32),
  Upval(u32),
}

pub fn run(f: &mut Func) {
  let order = f.reverse_postorder();
  let idom = dominators(f, &order);
  let preds = f.predecessors();
  let mut children: Vec<Vec<BlockId>> = vec![Vec::new(); f.blocks.len() + 1];
  for &b in &order {
    if let Some(d) = idom[b.0 as usize] {
      children[d.0 as usize].push(b);
    }
  }

  let mut table: FxHashMap<(Key, Vec<ValueId>), ValueId> = FxHashMap::default();
  let mut next_version = 1u32;
  let virtual_root = f.blocks.len();
  // (block, versions it starts with, table entries to undo when done)
  let mut stack: Vec<(BlockId, Versions)> = Vec::new();
  for &root in children[virtual_root].iter().rev() {
    stack.push((root, fresh(&mut next_version)));
  }
  let mut undo: Vec<Vec<(Key, Vec<ValueId>)>> = Vec::new();

  // An explicit walk: visit a block, then its children, then drop what it
  // added to the table.
  enum Step {
    Enter(BlockId, Versions),
    Leave,
  }
  let mut work: Vec<Step> = stack.into_iter().map(|(b, v)| Step::Enter(b, v)).collect();
  while let Some(step) = work.pop() {
    match step {
      Step::Leave => {
        for key in undo.pop().unwrap() {
          table.remove(&key);
        }
      },
      Step::Enter(b, mut versions) => {
        let added = visit(f, b, &mut versions, &mut table, &mut next_version);
        undo.push(added);
        work.push(Step::Leave);
        for &c in children[b.0 as usize].iter().rev() {
          let inherit = preds[c.0 as usize].len() == 1 && preds[c.0 as usize][0] == b;
          let v = if inherit {
            versions
          } else {
            fresh(&mut next_version)
          };
          work.push(Step::Enter(c, v));
        }
      },
    }
  }
}

fn fresh(next: &mut u32) -> Versions {
  let v = Versions {
    header: *next,
    elems: *next + 1,
    fields: *next + 2,
    globals: *next + 3,
    cells: *next + 4,
    upvals: *next + 5,
  };
  *next += 6;
  v
}

fn bump(next: &mut u32) -> u32 {
  *next += 1;
  *next
}

/// Numbers every operation in `b`, removing the redundant ones, and
/// returns the table entries it added.
fn visit(
  f: &mut Func,
  b: BlockId,
  versions: &mut Versions,
  table: &mut FxHashMap<(Key, Vec<ValueId>), ValueId>,
  next: &mut u32,
) -> Vec<(Key, Vec<ValueId>)> {
  let mut added = Vec::new();
  let mut keep: Vec<InstId> = Vec::with_capacity(f.block(b).insts.len());
  let insts = f.block(b).insts.clone();
  for id in insts {
    let inst = f.inst(id).clone();

    // Writes and anything that may run code invalidate what they touch.
    match &inst.op {
      Op::StoreElem => versions.elems = bump(next),
      Op::ListAppend => {
        versions.header = bump(next);
        versions.elems = bump(next);
      },
      Op::StoreUpval => versions.upvals = bump(next),
      Op::StoreGlobal(_) => versions.globals = bump(next),
      Op::StoreField(slot) => {
        versions.fields = bump(next);
        // The stored value is what a read of that field sees next.
        let key = (Key::Field(*slot, versions.fields), vec![inst.args[0]]);
        table.insert(key.clone(), inst.args[1]);
        added.push(key);
      },
      _ if inst.op.may_collect() => *versions = fresh(next),
      _ => {},
    }

    let Some(key) = key_of(&inst.op, versions) else {
      keep.push(id);
      continue;
    };
    let entry = (key, inst.args.clone());
    match table.get(&entry) {
      Some(&earlier) => {
        // A guard with no result simply goes; one with a result hands
        // its uses to the earlier one.
        if let Some(r) = inst.result {
          replace_uses(f, r, earlier);
        }
      },
      None => {
        if let Some(r) = inst.result {
          table.insert(entry.clone(), r);
          added.push(entry);
        } else if matches!(inst.op, Op::Guard(_)) {
          // A result-less guard is recorded against a placeholder: only
          // whether it happened matters.
          table.insert(entry.clone(), ValueId(u32::MAX));
          added.push(entry);
        }
        keep.push(id);
      },
    }
  }
  f.block_mut(b).insts = keep;
  added
}

fn key_of(op: &Op, v: &Versions) -> Option<Key> {
  Some(match op {
    Op::ConstTagged(bits) => Key::Pure("const_tagged", *bits),
    Op::ConstF64(x) => Key::Pure("const_f64", x.to_bits()),
    Op::ConstI64(x) => Key::Pure("const_i64", *x as u64),
    Op::ConstBool(x) => Key::Pure("const_bool", *x as u64),
    Op::BoxF64 => Key::Pure("box_f64", 0),
    Op::BoxBool => Key::Pure("box_bool", 0),
    Op::IntToF64 => Key::Pure("int_to_f64", 0),
    Op::F64ToI64 => Key::Pure("f64_to_i64", 0),
    Op::UnboxF64 => Key::Pure("unbox_f64", 0),
    Op::UnboxBool => Key::Pure("unbox_bool", 0),
    Op::ObjPtr => Key::Pure("obj_ptr", 0),
    Op::FAdd => Key::Pure("fadd", 0),
    Op::FSub => Key::Pure("fsub", 0),
    Op::FMul => Key::Pure("fmul", 0),
    Op::FDiv => Key::Pure("fdiv", 0),
    Op::FNeg => Key::Pure("fneg", 0),
    Op::FMod => Key::Pure("fmod", 0),
    Op::FFloorDiv => Key::Pure("ffloordiv", 0),
    Op::FCmp(c) => Key::Pure("fcmp", *c as u64),
    Op::IAdd => Key::Pure("iadd", 0),
    Op::ISub => Key::Pure("isub", 0),
    Op::IMul => Key::Pure("imul", 0),
    Op::ICmp(c) => Key::Pure("icmp", *c as u64),
    Op::IsFalsey => Key::Pure("is_falsey", 0),
    Op::BNot => Key::Pure("bnot", 0),
    Op::EqConst(x) => Key::Pure("eq_const", x.to_bits()),
    Op::FPow => Key::Pure("fpow", 0),
    Op::FUnary(u) => Key::Pure("funary", *u as u64),
    Op::FMax => Key::Pure("fmax", 0),
    Op::FMin => Key::Pure("fmin", 0),
    Op::FTest(t) => Key::Pure("ftest", *t as u64),
    Op::FCall(helper) => Key::Pure("fcall", helper.as_ptr() as u64),
    Op::Guard(kind) => Key::Guard(*kind),
    Op::ListLen => Key::Header("len", v.header),
    Op::ListData => Key::Header("data", v.header),
    Op::LoadElem => Key::Elem(v.elems),
    Op::ListEnd { last } => Key::ListEnd(*last, v.header, v.elems),
    Op::LoadField(slot) => Key::Field(*slot, v.fields),
    Op::LoadGlobal(slot) => Key::Global(*slot, v.globals),
    Op::UpvalCell(n) => Key::Cell(*n, v.cells),
    Op::LoadUpval => Key::Upval(v.upvals),
    _ => return None,
  })
}
