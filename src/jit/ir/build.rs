//! Bytecode to IR.
//!
//! Each bytecode basic block becomes one IR block whose parameters are
//! the registers live on entry to it, all `Tagged`. Within a block the
//! builder keeps, for every register, whichever representations of its
//! value it has produced so far: the tagged value, an unboxed number, an
//! integer, a condition, an object pointer. An operation asks for the
//! representation it needs and gets the cheapest one available, adding a
//! guard only when nothing proves the value already.
//!
//! What to speculate on comes from the interpreter's feedback: the kinds
//! of value each arithmetic and comparison site has seen, what each
//! index site was applied to, and the inline caches the field and method
//! sites filled. A site with no feedback, or with mixed feedback, runs
//! through its runtime helper instead.
//!
//! Representations only ever widen here. Turning a loop's tagged block
//! parameters into unboxed ones is `passes::repr`'s job, once the whole
//! function is visible.

use rustc_hash::{FxHashMap, FxHashSet};

use super::{BlockId, Cmp, FrameState, Func, GuardKind, Op, Terminator, Ty, ValueId};
use crate::jit::typeflow;
use crate::vm::chunk::{Instr, ParamType, kind};
use crate::vm::object::ObjFunction;
use crate::vm::value::Value;

/// Everything the builder reads from the interpreter's caches, copied
/// out on the VM's own thread so the build itself can run anywhere.
#[derive(Clone, Debug, Default)]
pub struct Feedback {
  /// Per instruction, the union of `chunk::kind` bits its operands have
  /// been seen holding. For an index or method site, the receiver's.
  pub kinds: Vec<u8>,
  /// A field site's cached class, as `Value` bits, and the field's slot.
  pub fields: FxHashMap<usize, (u64, u16)>,
  /// An invoke site's cached key: a class's bits for an instance
  /// receiver, `builtins::method_table_key`'s kind id for a primitive.
  pub invokes: FxHashMap<usize, u64>,
  /// A global site's resolved root slot.
  pub globals: FxHashMap<usize, u32>,
  /// The invoke-cache key a list receiver gets.
  pub list_key: u64,
  /// Sites where compiled code has already deoptimized. Nothing is
  /// speculated at them again.
  pub blocked: FxHashSet<usize>,
  /// No arithmetic, comparison or index speculation anywhere, after the
  /// function has deoptimized too often.
  pub sites_off: bool,
  /// No field or method speculation anywhere, for the same reason.
  pub fields_off: bool,
}

impl Feedback {
  fn open(&self, ip: usize) -> bool {
    !self.blocked.contains(&ip)
  }
}

/// What the builder knows about a register's value beyond its
/// representations.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Known {
  Unknown,
  Number,
  Bool,
  List,
  Instance(u64),
}

impl Known {
  /// Whether a value of this kind can be a heap object, and so has to be
  /// read back after a collection.
  fn may_be_object(self) -> bool {
    !matches!(self, Known::Number | Known::Bool)
  }
}

#[derive(Clone, Copy, Debug)]
struct RegView {
  tagged: Option<ValueId>,
  num: Option<ValueId>,
  int: Option<ValueId>,
  cond: Option<ValueId>,
  ptr: Option<ValueId>,
  known: Known,
}

impl RegView {
  const EMPTY: RegView = RegView {
    tagged: None,
    num: None,
    int: None,
    cond: None,
    ptr: None,
    known: Known::Unknown,
  };

  fn tagged(v: ValueId) -> RegView {
    RegView {
      tagged: Some(v),
      ..RegView::EMPTY
    }
  }
}

/// Why a function cannot be built. The baseline tier keeps running it.
pub type BuildError = String;

pub fn build(proto: &ObjFunction, feedback: &Feedback) -> Result<Func, BuildError> {
  Builder::new(proto, feedback)?.run()
}

struct Builder<'a> {
  proto: &'a ObjFunction,
  feedback: &'a Feedback,
  code: &'a [Instr],
  live: typeflow::LivenessFacts,
  func: Func,
  /// The IR block each bytecode leader starts.
  leader_block: FxHashMap<usize, BlockId>,
  /// For each leader block, which register each parameter carries.
  param_regs: FxHashMap<BlockId, Vec<u8>>,
  regs: Vec<RegView>,
  block: BlockId,
  /// Back edges seen, as (block ending in the jump, loop header ip), for
  /// placing safepoints once the loop bodies are known.
  back_edges: Vec<(BlockId, usize)>,
  /// Registers a closure made here captures. The closure reads and
  /// writes them in the register file, so every value one takes is
  /// written there as it is made, and each is read back after anything
  /// that could have run the closure.
  captured: Vec<bool>,
  /// Whether the function makes closures at all, and so may leave open
  /// upvalues over its registers that returning has to close.
  makes_closures: bool,
  /// Set when the instruction being translated went through its runtime
  /// helper, which leaves its result in the register file already.
  in_memory: bool,
}

impl<'a> Builder<'a> {
  fn new(proto: &'a ObjFunction, feedback: &'a Feedback) -> Result<Self, BuildError> {
    let code = &proto.chunk.code[..];
    for (ip, instr) in code.iter().enumerate() {
      if let Some(reason) = unsupported(instr) {
        return Err(format!("{reason} at ip {ip}"));
      }
    }
    let preds = typeflow::build_predecessors(proto);
    let live = typeflow::liveness(proto, &preds);
    Ok(Builder {
      proto,
      feedback,
      code,
      live,
      func: Func::new(proto.num_registers as usize),
      leader_block: FxHashMap::default(),
      param_regs: FxHashMap::default(),
      regs: vec![RegView::EMPTY; proto.num_registers as usize],
      block: BlockId(0),
      back_edges: Vec::new(),
      captured: typeflow::captured_registers(proto),
      makes_closures: code.iter().any(|i| matches!(i, Instr::Closure { .. })),
      in_memory: false,
    })
  }

  fn run(mut self) -> Result<Func, BuildError> {
    let leaders = self.leaders();

    // Loop headers an interpreted frame can jump into, numbered the way
    // the baseline tier numbers them: by first appearance of a backward
    // `Jmp`.
    let mut osr_ids: FxHashMap<usize, i32> = FxHashMap::default();
    for (ip, instr) in self.code.iter().enumerate() {
      if let Instr::Jmp { offset } = *instr
        && offset < 0
      {
        let target = jump_target(ip, offset);
        let next = osr_ids.len() as i32;
        osr_ids.entry(target).or_insert(next);
      }
    }

    let prologue = self.func.add_block(None);
    self.func.entry = prologue;
    for &ip in &leaders {
      let b = self.func.add_block(Some(ip));
      let regs: Vec<u8> = self.live.live_regs_at(ip).collect();
      for _ in &regs {
        self.func.add_block_param(b, Ty::Tagged);
      }
      self.func.block_mut(b).param_regs = regs.clone();
      self.leader_block.insert(ip, b);
      self.param_regs.insert(b, regs);
    }
    self.func.osr_ids = osr_ids;
    let mut osr: Vec<(usize, i32)> = self.func.osr_ids.iter().map(|(&ip, &id)| (ip, id)).collect();
    osr.sort_unstable_by_key(|&(_, id)| id);

    // The prologue sends an ordinary call to the first instruction and an
    // entry into a loop to that loop.
    self.block = prologue;
    let dispatch = if osr.is_empty() {
      None
    } else {
      let which = self.value(Op::OsrIndex, vec![], Ty::I64);
      let none = self.value(Op::ConstI64(-1), vec![], Ty::I64);
      let ordinary = self.value(Op::ICmp(Cmp::Eq), vec![which, none], Ty::Bool);
      Some((which, ordinary))
    };

    // The ordinary entry reads whatever is live at the first instruction,
    // which in practice is the parameters.
    let first = self.leader_block[&0];
    let ordinary_entry = match dispatch {
      Some(_) => self.func.add_block(None),
      None => prologue,
    };
    self.block = ordinary_entry;
    let mut args = Vec::new();
    for &r in &self.param_regs[&first].clone() {
      let v = self.push(Op::Param(r), vec![], Some(Ty::Tagged), None).unwrap();
      // An argument can arrive in a machine register rather than the
      // register file, where a closure capturing it would look.
      if self.captured[r as usize] {
        self.push(Op::StoreReg(r), vec![v], None, None);
      }
      args.push(v);
    }
    self.func.set_term(ordinary_entry, Terminator::Jump { target: first, args });

    // An interpreted frame entering a loop leaves every live register in
    // the register file; its entry block reads them from there. A chain
    // of comparisons picks the entry the frame asked for, and an id no
    // entry claims hands the frame straight back to the interpreter.
    if let Some((which, ordinary)) = dispatch {
      let mut entries = Vec::new();
      for &(ip, id) in &osr {
        let header = self.leader_block[&ip];
        let block = self.func.add_block(None);
        self.block = block;
        let mut args = Vec::new();
        for &r in &self.param_regs[&header].clone() {
          let v = self.push(Op::OsrParam(r), vec![], Some(Ty::Tagged), None).unwrap();
          args.push(v);
        }
        self.func.set_term(block, Terminator::Jump { target: header, args });
        self.func.osr_entries.push((id, block));
        entries.push((id, block));
      }
      let mut check = self.func.add_block(None);
      self.func.set_term(
        prologue,
        Terminator::Branch {
          cond: ordinary,
          then_block: ordinary_entry,
          then_args: Vec::new(),
          else_block: check,
          else_args: Vec::new(),
        },
      );
      for (id, block) in entries {
        self.block = check;
        let want = self.value(Op::ConstI64(id as i64), vec![], Ty::I64);
        let hit = self.value(Op::ICmp(Cmp::Eq), vec![which, want], Ty::Bool);
        let next = self.func.add_block(None);
        self.func.set_term(
          check,
          Terminator::Branch {
            cond: hit,
            then_block: block,
            then_args: Vec::new(),
            else_block: next,
            else_args: Vec::new(),
          },
        );
        check = next;
      }
      self.func.set_term(check, Terminator::Deopt(FrameState { ip: 0, regs: Vec::new() }));
    }

    for (i, &start) in leaders.iter().enumerate() {
      let end = leaders.get(i + 1).copied().unwrap_or(self.code.len());
      self.build_block(start, end)?;
    }

    self.place_safepoints();
    Ok(self.func)
  }

  /// Every bytecode position that starts a basic block.
  fn leaders(&self) -> Vec<usize> {
    let mut set: FxHashSet<usize> = FxHashSet::default();
    set.insert(0);
    for (ip, instr) in self.code.iter().enumerate() {
      match *instr {
        Instr::Jmp { offset }
        | Instr::JmpIfFalse { offset, .. }
        | Instr::JmpIfTrue { offset, .. } => {
          set.insert(jump_target(ip, offset));
          if ip + 1 < self.code.len() {
            set.insert(ip + 1);
          }
        },
        Instr::UsingJump { table_idx, .. } => {
          set.extend(self.proto.chunk.jump_tables[table_idx as usize].values().copied());
          if ip + 1 < self.code.len() {
            set.insert(ip + 1);
          }
        },
        Instr::Return { .. } | Instr::Raise { .. } => {
          if ip + 1 < self.code.len() {
            set.insert(ip + 1);
          }
        },
        _ => {},
      }
    }
    let mut v: Vec<usize> = set.into_iter().filter(|&ip| ip < self.code.len()).collect();
    v.sort_unstable();
    v
  }

  fn push(
    &mut self,
    op: Op,
    args: Vec<ValueId>,
    ty: Option<Ty>,
    state: Option<FrameState>,
  ) -> Option<ValueId> {
    self.func.push(self.block, op, args, ty, state)
  }

  fn value(&mut self, op: Op, args: Vec<ValueId>, ty: Ty) -> ValueId {
    self.push(op, args, Some(ty), None).unwrap()
  }

  fn build_block(&mut self, start: usize, end: usize) -> Result<(), BuildError> {
    let b = self.leader_block[&start];
    self.block = b;
    self.regs = vec![RegView::EMPTY; self.func.num_registers];
    let params = self.func.block(b).params.clone();
    for (&r, &p) in self.param_regs[&b].clone().iter().zip(&params) {
      self.regs[r as usize] = RegView::tagged(p);
    }

    for ip in start..end {
      let instr = self.code[ip];
      self.in_memory = false;
      if self.translate(ip, instr)? {
        return Ok(());
      }
      if let Some(dst) = typeflow::any_dst(&instr)
        && self.captured[dst as usize]
        && !self.in_memory
      {
        let v = self.tagged(dst);
        self.push(Op::StoreReg(dst), vec![v], None, None);
      }
    }
    // Fell off the end of the block into the next leader.
    let next = self.leader_block[&end];
    let args = self.edge_args(next);
    self.func.set_term(self.block, Terminator::Jump { target: next, args });
    Ok(())
  }

  // --- representations -------------------------------------------------

  fn tagged(&mut self, r: u8) -> ValueId {
    let view = self.regs[r as usize];
    if let Some(v) = view.tagged {
      return v;
    }
    let v = if let Some(n) = view.num {
      self.value(Op::BoxF64, vec![n], Ty::Tagged)
    } else if let Some(i) = view.int {
      let n = self.value(Op::IntToF64, vec![i], Ty::F64);
      self.regs[r as usize].num = Some(n);
      self.value(Op::BoxF64, vec![n], Ty::Tagged)
    } else if let Some(c) = view.cond {
      self.value(Op::BoxBool, vec![c], Ty::Tagged)
    } else {
      // Never written on this path; the interpreter would read nil.
      self.value(Op::ConstTagged(Value::nil().to_bits()), vec![], Ty::Tagged)
    };
    self.regs[r as usize].tagged = Some(v);
    v
  }

  fn num(&mut self, r: u8, ip: usize) -> ValueId {
    let view = self.regs[r as usize];
    if let Some(n) = view.num {
      return n;
    }
    if let Some(i) = view.int {
      let n = self.value(Op::IntToF64, vec![i], Ty::F64);
      self.regs[r as usize].num = Some(n);
      return n;
    }
    let t = self.tagged(r);
    let n = if view.known == Known::Number {
      self.value(Op::UnboxF64, vec![t], Ty::F64)
    } else {
      let state = self.state(ip);
      self
        .push(Op::Guard(GuardKind::Number), vec![t], Some(Ty::F64), Some(state))
        .unwrap()
    };
    let view = &mut self.regs[r as usize];
    view.num = Some(n);
    view.known = Known::Number;
    n
  }

  fn int(&mut self, r: u8, ip: usize) -> ValueId {
    if let Some(i) = self.regs[r as usize].int {
      return i;
    }
    let source = match self.regs[r as usize].num {
      Some(n) => n,
      None => self.tagged(r),
    };
    let state = self.state(ip);
    let i = self
      .push(Op::Guard(GuardKind::Int), vec![source], Some(Ty::I64), Some(state))
      .unwrap();
    let view = &mut self.regs[r as usize];
    view.int = Some(i);
    view.known = Known::Number;
    i
  }

  /// `r` as a condition: true when the value is truthy.
  fn truthy(&mut self, r: u8) -> ValueId {
    if let Some(c) = self.regs[r as usize].cond {
      return c;
    }
    let t = self.tagged(r);
    let falsey = self.value(Op::IsFalsey, vec![t], Ty::Bool);
    self.value(Op::BNot, vec![falsey], Ty::Bool)
  }

  fn list_ptr(&mut self, r: u8, ip: usize) -> ValueId {
    let view = self.regs[r as usize];
    if let (Some(p), Known::List) = (view.ptr, view.known) {
      return p;
    }
    let t = self.tagged(r);
    let p = if view.known == Known::List {
      self.value(Op::ObjPtr, vec![t], Ty::Ptr)
    } else {
      let state = self.state(ip);
      self
        .push(Op::Guard(GuardKind::List), vec![t], Some(Ty::Ptr), Some(state))
        .unwrap()
    };
    let view = &mut self.regs[r as usize];
    view.ptr = Some(p);
    view.known = Known::List;
    p
  }

  fn instance_ptr(&mut self, r: u8, class: u64, ip: usize) -> ValueId {
    let view = self.regs[r as usize];
    if let (Some(p), Known::Instance(c)) = (view.ptr, view.known)
      && c == class
    {
      return p;
    }
    let t = self.tagged(r);
    let p = if view.known == Known::Instance(class) {
      self.value(Op::ObjPtr, vec![t], Ty::Ptr)
    } else {
      let state = self.state(ip);
      self
        .push(
          Op::Guard(GuardKind::Instance(class)),
          vec![t],
          Some(Ty::Ptr),
          Some(state),
        )
        .unwrap()
    };
    let view = &mut self.regs[r as usize];
    view.ptr = Some(p);
    view.known = Known::Instance(class);
    p
  }

  fn set_num(&mut self, r: u8, n: ValueId) {
    self.regs[r as usize] = RegView {
      num: Some(n),
      known: Known::Number,
      ..RegView::EMPTY
    };
  }

  fn set_cond(&mut self, r: u8, c: ValueId) {
    self.regs[r as usize] = RegView {
      cond: Some(c),
      known: Known::Bool,
      ..RegView::EMPTY
    };
  }

  fn set_tagged(&mut self, r: u8, t: ValueId) {
    self.regs[r as usize] = RegView::tagged(t);
  }

  /// What the interpreter needs to resume at `ip`: every register live
  /// there, tagged.
  fn state(&mut self, ip: usize) -> FrameState {
    let live: Vec<u8> = self.live.live_regs_at(ip).collect();
    let regs = live.into_iter().map(|r| (r, self.tagged(r))).collect();
    FrameState { ip, regs }
  }

  /// The values to pass along an edge into `target`.
  fn edge_args(&mut self, target: BlockId) -> Vec<ValueId> {
    let regs = self.param_regs[&target].clone();
    regs.into_iter().map(|r| self.tagged(r)).collect()
  }

  /// Everything that happens after an operation that may have collected:
  /// the register file is the only place values survived, so each live
  /// register that could hold an object is read back, pointers derived
  /// from the old values are dropped, and whatever was known about each
  /// value's kind carries over.
  fn after_collect(&mut self, next_ip: usize, written: Option<u8>) {
    let live: FxHashSet<u8> = if next_ip < self.code.len() {
      self.live.live_regs_at(next_ip).collect()
    } else {
      FxHashSet::default()
    };
    for r in 0..self.func.num_registers as u8 {
      if Some(r) == written {
        continue;
      }
      if !live.contains(&r) {
        self.regs[r as usize] = RegView::EMPTY;
        continue;
      }
      let view = self.regs[r as usize];
      if self.captured[r as usize] {
        // A closure may have set it to anything at all.
        let old = self.tagged(r);
        let fresh = self.value(Op::Reload { reg: r }, vec![old], Ty::Tagged);
        self.set_tagged(r, fresh);
        continue;
      }
      if !view.known.may_be_object() {
        // A number or a boolean is its own bits; nothing moved.
        continue;
      }
      let Some(old) = view.tagged else {
        continue;
      };
      let fresh = self.value(Op::Reload { reg: r }, vec![old], Ty::Tagged);
      self.regs[r as usize] = RegView {
        tagged: Some(fresh),
        known: view.known,
        ..RegView::EMPTY
      };
    }
    if let Some(dst) = written {
      let fresh = self.value(Op::Reload { reg: dst }, vec![], Ty::Tagged);
      self.set_tagged(dst, fresh);
    }
  }

  /// Runs `instr` through its runtime helper.
  fn generic(&mut self, ip: usize, instr: Instr) -> Result<(), BuildError> {
    if super::lower::generic_helper(&instr).is_none() {
      return Err(format!("no runtime helper for {instr:?} at ip {ip}"));
    }
    if let Some(what) = self.baseline_faster(ip, &instr) {
      return Err(format!("{what} at ip {ip} runs faster in the baseline tier"));
    }
    let state = self.state(ip);
    let args = state.regs.iter().map(|&(_, v)| v).collect();
    self.push(Op::Generic { instr, ip }, args, None, Some(state));
    self.after_collect(ip + 1, typeflow::any_dst(&instr));
    self.in_memory = true;
    Ok(())
  }

  /// Why the baseline tier would run `instr` faster than a call to its
  /// runtime helper here, if it would. The baseline tier has an inline
  /// path for each of these, so a function that reaches one on a path it
  /// actually takes is left to that tier. A site the interpreter never
  /// ran costs nothing either way.
  fn baseline_faster(&self, ip: usize, instr: &Instr) -> Option<&'static str> {
    let seen = self.feedback.kinds.get(ip).copied().unwrap_or(0);
    match instr {
      Instr::Add { .. }
      | Instr::Sub { .. }
      | Instr::Mul { .. }
      | Instr::Div { .. }
      | Instr::Pow { .. }
      | Instr::Floor { .. }
      | Instr::Mod { .. }
      | Instr::Neg { .. }
      | Instr::BitAnd { .. }
      | Instr::BitOr { .. }
      | Instr::BitXor { .. }
      | Instr::BitShl { .. }
      | Instr::BitShr { .. }
      | Instr::BitUshr { .. }
      | Instr::BitNot { .. }
      | Instr::Lt { .. }
      | Instr::Le { .. }
      | Instr::Gt { .. }
      | Instr::Ge { .. }
      | Instr::AddImm { .. }
      | Instr::SubImm { .. }
      | Instr::MulImm { .. }
      | Instr::LtImm { .. }
      | Instr::LeImm { .. }
      | Instr::GtImm { .. }
      | Instr::GeImm { .. }
        if seen != 0 && seen & !kind::NUMBER == 0 =>
      {
        // Only numbers ever reached it, which the baseline tier handles
        // inline. Once anything else has too, the baseline tier's number
        // check fails as often as not and it calls this same helper.
        Some("an operation on numbers")
      },
      Instr::Call { .. } if seen != 0 => Some("a call"),
      Instr::Invoke { .. } if seen != 0 => Some("a method call"),
      Instr::GetIndex { .. } | Instr::SetIndex { .. } if seen != 0 => Some("an index"),
      Instr::MakeList { count, .. } if *count <= 2 => Some("a small list"),
      Instr::CheckParamType { .. } => Some("a parameter check"),
      _ => None,
    }
  }

  /// Whether to treat an arithmetic or comparison site as working on
  /// numbers: it has only ever seen numbers, or it has never run at all.
  /// A loop compiled while it runs has sites further down that have not
  /// had their first turn yet; guessing numbers there costs one
  /// deoptimization if the guess is wrong, after which the site is
  /// blocked and runs through its helper.
  fn numeric_site(&self, ip: usize) -> bool {
    let seen = self.feedback.kinds.get(ip).copied().unwrap_or(0);
    !self.feedback.sites_off && self.feedback.open(ip) && seen & !kind::NUMBER == 0
  }

  fn list_site(&self, ip: usize) -> bool {
    !self.feedback.sites_off
      && self.feedback.open(ip)
      && self.feedback.kinds.get(ip).copied().unwrap_or(0) == kind::LIST
  }

  fn field_site(&self, ip: usize) -> Option<(u64, u16)> {
    if self.feedback.fields_off || !self.feedback.open(ip) {
      return None;
    }
    self.feedback.fields.get(&ip).copied()
  }

  /// A method call whose receiver has only ever been a list: the
  /// interpreter's receiver feedback, or the key compiled code left in
  /// the site's cache.
  fn list_invoke_site(&self, ip: usize) -> bool {
    if self.feedback.fields_off || !self.feedback.open(ip) {
      return false;
    }
    self.feedback.kinds.get(ip).copied().unwrap_or(0) == kind::LIST
      || self.feedback.invokes.get(&ip) == Some(&self.feedback.list_key)
  }

  fn imm(&self, idx: u16) -> Option<f64> {
    let c = self.proto.chunk.constants[idx as usize];
    c.is_number().then(|| c.as_number())
  }

  // --- instructions ----------------------------------------------------

  /// Translates one instruction. Returns whether it ended the block.
  fn translate(&mut self, ip: usize, instr: Instr) -> Result<bool, BuildError> {
    match instr {
      Instr::LoadConst { dst, const_idx } => {
        let c = self.proto.chunk.constants[const_idx as usize];
        if c.is_number() {
          let n = self.value(Op::ConstF64(c.as_number()), vec![], Ty::F64);
          self.set_num(dst, n);
        } else {
          let t = self.value(Op::ConstTagged(c.to_bits()), vec![], Ty::Tagged);
          self.set_tagged(dst, t);
        }
      },
      Instr::LoadNil { dst } => {
        let t = self.value(Op::ConstTagged(Value::nil().to_bits()), vec![], Ty::Tagged);
        self.set_tagged(dst, t);
      },
      Instr::LoadBool { dst, val } => {
        let c = self.value(Op::ConstBool(val), vec![], Ty::Bool);
        self.set_cond(dst, c);
      },
      Instr::Move { dst, src } => {
        self.regs[dst as usize] = self.regs[src as usize];
      },

      Instr::Add { dst, a, b }
      | Instr::Sub { dst, a, b }
      | Instr::Mul { dst, a, b }
      | Instr::Div { dst, a, b }
      | Instr::Mod { dst, a, b }
      | Instr::Floor { dst, a, b }
        if self.numeric_site(ip) =>
      {
        let x = self.num(a, ip);
        let y = self.num(b, ip);
        let op = match instr {
          Instr::Add { .. } => Op::FAdd,
          Instr::Sub { .. } => Op::FSub,
          Instr::Mul { .. } => Op::FMul,
          Instr::Div { .. } => Op::FDiv,
          Instr::Mod { .. } => Op::FMod,
          _ => Op::FFloorDiv,
        };
        let r = self.value(op, vec![x, y], Ty::F64);
        self.set_num(dst, r);
      },
      Instr::AddImm { dst, a, imm_const }
      | Instr::SubImm { dst, a, imm_const }
      | Instr::MulImm { dst, a, imm_const }
        if self.numeric_site(ip) && self.imm(imm_const).is_some() =>
      {
        let x = self.num(a, ip);
        let k = self.imm(imm_const).unwrap();
        let y = self.value(Op::ConstF64(k), vec![], Ty::F64);
        let op = match instr {
          Instr::AddImm { .. } => Op::FAdd,
          Instr::SubImm { .. } => Op::FSub,
          _ => Op::FMul,
        };
        let r = self.value(op, vec![x, y], Ty::F64);
        self.set_num(dst, r);
      },
      Instr::Neg { dst, src } if self.numeric_site(ip) => {
        let x = self.num(src, ip);
        let r = self.value(Op::FNeg, vec![x], Ty::F64);
        self.set_num(dst, r);
      },

      Instr::Lt { dst, a, b }
      | Instr::Le { dst, a, b }
      | Instr::Gt { dst, a, b }
      | Instr::Ge { dst, a, b }
        if self.numeric_site(ip) =>
      {
        let x = self.num(a, ip);
        let y = self.num(b, ip);
        let r = self.value(Op::FCmp(cmp_of(&instr)), vec![x, y], Ty::Bool);
        self.set_cond(dst, r);
      },
      Instr::LtImm { dst, a, imm_const }
      | Instr::LeImm { dst, a, imm_const }
      | Instr::GtImm { dst, a, imm_const }
      | Instr::GeImm { dst, a, imm_const }
        if self.numeric_site(ip) && self.imm(imm_const).is_some() =>
      {
        let x = self.num(a, ip);
        let k = self.imm(imm_const).unwrap();
        let y = self.value(Op::ConstF64(k), vec![], Ty::F64);
        let r = self.value(Op::FCmp(cmp_of(&instr)), vec![x, y], Ty::Bool);
        self.set_cond(dst, r);
      },
      Instr::Eq { dst, a, b } | Instr::Neq { dst, a, b }
        if self.regs[a as usize].num.is_some() && self.regs[b as usize].num.is_some() =>
      {
        let x = self.num(a, ip);
        let y = self.num(b, ip);
        let cmp = if matches!(instr, Instr::Eq { .. }) {
          Cmp::Eq
        } else {
          Cmp::Ne
        };
        let r = self.value(Op::FCmp(cmp), vec![x, y], Ty::Bool);
        self.set_cond(dst, r);
      },
      Instr::Eq { dst, a, b } | Instr::Neq { dst, a, b } => {
        let x = self.tagged(a);
        let y = self.tagged(b);
        let eq = self.value(Op::TaggedEq, vec![x, y], Ty::Bool);
        let r = if matches!(instr, Instr::Eq { .. }) {
          eq
        } else {
          self.value(Op::BNot, vec![eq], Ty::Bool)
        };
        self.set_cond(dst, r);
      },
      Instr::Pow { dst, a, b } if self.numeric_site(ip) => {
        let x = self.num(a, ip);
        let y = self.num(b, ip);
        let r = self.value(Op::FPow, vec![x, y], Ty::F64);
        self.set_num(dst, r);
      },
      Instr::EqImm { dst, a, imm_const } | Instr::NeqImm { dst, a, imm_const }
        if self.imm(imm_const).is_some() =>
      {
        let k = self.imm(imm_const).unwrap();
        let eq = if let Some(x) = self.regs[a as usize].num {
          let y = self.value(Op::ConstF64(k), vec![], Ty::F64);
          self.value(Op::FCmp(Cmp::Eq), vec![x, y], Ty::Bool)
        } else {
          let t = self.tagged(a);
          self.value(Op::EqConst(k), vec![t], Ty::Bool)
        };
        let r = if matches!(instr, Instr::EqImm { .. }) {
          eq
        } else {
          self.value(Op::BNot, vec![eq], Ty::Bool)
        };
        self.set_cond(dst, r);
      },
      Instr::Not { dst, src } => {
        let r = if let Some(c) = self.regs[src as usize].cond {
          self.value(Op::BNot, vec![c], Ty::Bool)
        } else {
          let t = self.tagged(src);
          self.value(Op::IsFalsey, vec![t], Ty::Bool)
        };
        self.set_cond(dst, r);
      },

      Instr::Jmp { offset } => {
        let target_ip = jump_target(ip, offset);
        let target = self.leader_block[&target_ip];
        if offset < 0 {
          self.back_edges.push((self.block, target_ip));
        }
        let args = self.edge_args(target);
        self.func.set_term(self.block, Terminator::Jump { target, args });
        return Ok(true);
      },
      Instr::JmpIfFalse { cond, offset } | Instr::JmpIfTrue { cond, offset } => {
        let target = self.leader_block[&jump_target(ip, offset)];
        let next = self.leader_block[&(ip + 1)];
        let truthy = self.truthy(cond);
        let target_args = self.edge_args(target);
        let next_args = self.edge_args(next);
        let (then_block, then_args, else_block, else_args) = match instr {
          Instr::JmpIfTrue { .. } => (target, target_args, next, next_args),
          _ => (next, next_args, target, target_args),
        };
        self.func.set_term(
          self.block,
          Terminator::Branch {
            cond: truthy,
            then_block,
            then_args,
            else_block,
            else_args,
          },
        );
        return Ok(true);
      },
      Instr::Return { src } => {
        if self.makes_closures {
          self.close_upvalues(ip, src);
        }
        let v = self.tagged(src);
        self.func.set_term(self.block, Terminator::Return(v));
        return Ok(true);
      },
      Instr::Raise { .. } => {
        // No handler here to catch it: the interpreter raises it from
        // this frame.
        let state = self.state(ip);
        self.func.set_term(self.block, Terminator::Deopt(state));
        return Ok(true);
      },
      Instr::UsingJump { subject, table_idx } => {
        self.using_jump(ip, subject, table_idx);
        return Ok(true);
      },

      Instr::GetUpval { dst, idx } => {
        let cell = self.upval_cell(idx, ip);
        let v = self.value(Op::LoadUpval, vec![cell], Ty::Tagged);
        self.set_tagged(dst, v);
      },
      Instr::SetUpval { idx, src } => {
        let cell = self.upval_cell(idx, ip);
        let v = self.tagged(src);
        self.push(Op::StoreUpval, vec![cell, v], None, None);
      },

      Instr::GetGlobal { dst, .. } if self.feedback.globals.contains_key(&ip) => {
        let slot = self.feedback.globals[&ip];
        let t = self.value(Op::LoadGlobal(slot), vec![], Ty::Tagged);
        self.set_tagged(dst, t);
      },
      Instr::SetGlobal { src, .. } | Instr::AssignGlobal { src, .. }
        if self.feedback.globals.contains_key(&ip) =>
      {
        let slot = self.feedback.globals[&ip];
        let v = self.tagged(src);
        self.push(Op::StoreGlobal(slot), vec![v], None, None);
      },
      Instr::CheckParamType { reg, check_idx } if inline_param_check(self.proto, check_idx) => {
        let v = self.tagged(reg);
        let state = self.state(ip);
        self.push(
          Op::Guard(GuardKind::Param(check_idx)),
          vec![v],
          None,
          Some(state),
        );
        let check = &self.proto.chunk.param_checks[check_idx as usize];
        let numeric = !check.nullable
          && check
            .types
            .iter()
            .all(|t| matches!(t, ParamType::Number | ParamType::Int));
        if numeric {
          self.regs[reg as usize].known = Known::Number;
        }
      },

      Instr::GetIndex { dst, obj, idx } if self.list_site(ip) => {
        let p = self.list_ptr(obj, ip);
        let i = self.int(idx, ip);
        let len = self.value(Op::ListLen, vec![p], Ty::I64);
        let state = self.state(ip);
        self.push(Op::Guard(GuardKind::Bounds), vec![i, len], None, Some(state));
        let data = self.value(Op::ListData, vec![p], Ty::Ptr);
        let e = self.value(Op::LoadElem, vec![data, i], Ty::Tagged);
        self.set_tagged(dst, e);
      },
      Instr::SetIndex { obj, idx, src } if self.list_site(ip) => {
        let p = self.list_ptr(obj, ip);
        let i = self.int(idx, ip);
        let len = self.value(Op::ListLen, vec![p], Ty::I64);
        let state = self.state(ip);
        self.push(Op::Guard(GuardKind::Bounds), vec![i, len], None, Some(state));
        let data = self.value(Op::ListData, vec![p], Ty::Ptr);
        let v = self.tagged(src);
        self.push(Op::StoreElem, vec![p, data, i, v], None, None);
      },

      Instr::GetField { dst, obj, .. } if self.field_site(ip).is_some() => {
        let (class, slot) = self.field_site(ip).unwrap();
        let p = self.instance_ptr(obj, class, ip);
        let v = self.value(Op::LoadField(slot), vec![p], Ty::Tagged);
        self.set_tagged(dst, v);
      },
      Instr::SetField { obj, src, .. } if self.field_site(ip).is_some() => {
        let (class, slot) = self.field_site(ip).unwrap();
        let p = self.instance_ptr(obj, class, ip);
        let v = self.tagged(src);
        self.push(Op::StoreField(slot), vec![p, v], None, None);
      },

      Instr::Invoke {
        dst,
        obj,
        method_const,
        num_args: 0,
      } if self.list_invoke_site(ip)
        && self.proto.chunk.constants[method_const as usize].as_str() == "length" =>
      {
        let p = self.list_ptr(obj, ip);
        let len = self.value(Op::ListLen, vec![p], Ty::I64);
        self.regs[dst as usize] = RegView {
          int: Some(len),
          known: Known::Number,
          ..RegView::EMPTY
        };
      },

      _ => self.generic(ip, instr)?,
    }
    Ok(false)
  }

  fn upval_cell(&mut self, idx: u8, ip: usize) -> ValueId {
    let state = self.state(ip);
    self
      .push(Op::UpvalCell(idx), vec![], Some(Ty::Ptr), Some(state))
      .unwrap()
  }

  /// Closes the upvalues still open over this frame before it returns
  /// `src`, which is read back afterwards like any helper's operand.
  fn close_upvalues(&mut self, ip: usize, src: u8) {
    let instr = Instr::CloseUpvalues { from: 0 };
    let state = self.state(ip);
    let args = state.regs.iter().map(|&(_, v)| v).collect();
    self.push(Op::Generic { instr, ip }, args, None, Some(state));
    if self.regs[src as usize].known.may_be_object() || self.captured[src as usize] {
      let old = self.tagged(src);
      let fresh = self.value(Op::Reload { reg: src }, vec![old], Ty::Tagged);
      let known = if self.captured[src as usize] {
        Known::Unknown
      } else {
        self.regs[src as usize].known
      };
      self.regs[src as usize] = RegView {
        tagged: Some(fresh),
        known,
        ..RegView::EMPTY
      };
    }
  }

  /// A `using` dispatch: the runtime finds the case's position, and a
  /// chain of comparisons against every position the table holds picks
  /// the block. No match, or none of them, falls through to `ip + 1`.
  fn using_jump(&mut self, ip: usize, subject: u8, table_idx: u16) {
    let t = self.tagged(subject);
    let found = self.value(
      Op::UsingTarget {
        table: table_idx,
        reg: subject,
      },
      vec![t],
      Ty::I64,
    );

    let mut targets: Vec<usize> = self.proto.chunk.jump_tables[table_idx as usize]
      .values()
      .copied()
      .collect();
    targets.sort_unstable();
    targets.dedup();
    // Every edge's values come from this block, so they reach every link
    // of the chain.
    let edges: Vec<(BlockId, Vec<ValueId>)> = targets
      .iter()
      .map(|t| {
        let b = self.leader_block[t];
        (b, self.edge_args(b))
      })
      .collect();
    let miss = self.leader_block[&(ip + 1)];
    let miss_args = self.edge_args(miss);

    for ((block, args), &target_ip) in edges.into_iter().zip(&targets) {
      let want = self.value(Op::ConstI64(target_ip as i64), vec![], Ty::I64);
      let hit = self.value(Op::ICmp(Cmp::Eq), vec![found, want], Ty::Bool);
      let next = self.func.add_block(None);
      self.func.set_term(
        self.block,
        Terminator::Branch {
          cond: hit,
          then_block: block,
          then_args: args,
          else_block: next,
          else_args: Vec::new(),
        },
      );
      self.block = next;
    }
    self.func.set_term(
      self.block,
      Terminator::Jump {
        target: miss,
        args: miss_args,
      },
    );
  }

  /// A safepoint on every back edge of a loop that runs anything able to
  /// allocate, so a collection owed inside it gets run. Placed once the
  /// whole function is built, when the loop bodies are known.
  fn place_safepoints(&mut self) {
    let armed = crate::modules::os_util::signal::armed();
    let edges = std::mem::take(&mut self.back_edges);
    for (latch, header_ip) in edges {
      let header = self.leader_block[&header_ip];
      if !armed && !self.loop_may_collect(header, latch) {
        continue;
      }
      let Terminator::Jump { target, args } = self.func.block(latch).term.clone() else {
        continue;
      };
      let regs = self.param_regs[&header].clone();
      let state = FrameState {
        ip: header_ip,
        regs: regs.iter().copied().zip(args.iter().copied()).collect(),
      };
      self.func.push(latch, Op::Safepoint, args.clone(), None, Some(state));
      // Anything the loop carries that could be an object has to be read
      // back after the safepoint, the same as after any other operation
      // that may collect.
      let mut fresh = Vec::with_capacity(args.len());
      for (&r, &a) in regs.iter().zip(&args) {
        // A signal handler run here can reach a captured register.
        let stays = !self.captured[r as usize]
          && self.func.def_inst(a).is_some_and(|i| {
            matches!(
              i.op,
              Op::BoxF64 | Op::BoxBool | Op::ConstF64(_) | Op::ConstBool(_)
            )
          });
        if stays {
          fresh.push(a);
        } else {
          let v = self
            .func
            .push(latch, Op::Reload { reg: r }, vec![a], Some(Ty::Tagged), None)
            .unwrap();
          fresh.push(v);
        }
      }
      self
        .func
        .set_term(latch, Terminator::Jump { target, args: fresh });
    }
  }

  /// Whether any block of the loop from `header` to `latch` holds an
  /// operation that may collect. The loop is every block that reaches
  /// the latch without passing back through the header.
  fn loop_may_collect(&self, header: BlockId, latch: BlockId) -> bool {
    let preds = self.func.predecessors();
    let mut body: FxHashSet<BlockId> = FxHashSet::default();
    body.insert(header);
    let mut work = vec![latch];
    while let Some(b) = work.pop() {
      if body.insert(b) {
        work.extend(preds[b.0 as usize].iter().copied());
      }
    }
    body.iter().any(|&b| {
      self
        .func
        .block(b)
        .insts
        .iter()
        .any(|&i| self.func.inst(i).op.may_collect())
    })
  }
}

/// Whether every type parameter check `check_idx` accepts can be tested
/// inline. An instance or iterable type needs the runtime's lookup.
fn inline_param_check(proto: &ObjFunction, check_idx: u16) -> bool {
  proto.chunk.param_checks[check_idx as usize]
    .types
    .iter()
    .all(|t| !matches!(t, ParamType::Instance(_) | ParamType::Iterable))
}

fn jump_target(ip: usize, offset: i16) -> usize {
  (ip as isize + 1 + offset as isize) as usize
}

fn cmp_of(instr: &Instr) -> Cmp {
  match instr {
    Instr::Lt { .. } | Instr::LtImm { .. } => Cmp::Lt,
    Instr::Le { .. } | Instr::LeImm { .. } => Cmp::Le,
    Instr::Gt { .. } | Instr::GtImm { .. } => Cmp::Gt,
    _ => Cmp::Ge,
  }
}

/// Why this tier cannot take a function containing `instr`, if it
/// cannot. Everything else either gets a specialized translation or runs
/// through its runtime helper.
fn unsupported(instr: &Instr) -> Option<&'static str> {
  match instr {
    Instr::PushCatch { .. } | Instr::PopCatch => Some("catch"),
    _ => None,
  }
}
