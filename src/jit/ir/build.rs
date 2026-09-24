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
//!
//! A call whose callee the feedback knows is built in place. The callee's
//! bytecode is translated into the caller's IR behind a guard that the
//! callee is still the one it was, with its registers where a real call
//! would put them, just past the caller's call window. A callee without
//! branches goes straight into the calling block, so its arguments keep
//! whatever representation they had; one with branches gets its own
//! blocks and returns through a block that picks the caller up again.

use rustc_hash::{FxHashMap, FxHashSet};

use super::{
  BlockId, Cmp, FrameClosure, FrameState, Func, GuardKind, InlineFrame, Op, Terminator, Ty, ValueId,
};
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
  /// For a global slot holding a closure some call here reaches through,
  /// that closure's function, with what to build it from.
  pub global_callees: FxHashMap<u32, Box<Callee>>,
  /// For a method call site whose cache holds a class, the method that
  /// class resolves it to.
  pub invoke_callees: FxHashMap<usize, Box<Callee>>,
}

/// A function a call can be built into its caller from.
#[derive(Clone, Debug, Default)]
pub struct Callee {
  /// The `ObjFunction`, whose bytecode is read while building.
  pub proto: usize,
  /// The function's own `Value` bits, which a closure called through a
  /// register is checked against.
  pub proto_bits: u64,
  /// A method's closure, as `Value` bits. Methods live as long as their
  /// class and never move.
  pub closure: u64,
  /// The callee's own feedback.
  pub feedback: Feedback,
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

/// How deep calls are built into one another.
pub const MAX_INLINE_DEPTH: usize = 4;
/// The longest callee built in, in bytecode instructions.
pub const MAX_INLINE_OPS: usize = 64;
/// How much callee bytecode one function takes in altogether.
pub const MAX_INLINED_TOTAL: usize = 512;

/// Registers are numbered from the compiled function's frame, and a
/// register number is a byte.
const REGISTER_LIMIT: usize = 256;

/// One function being translated: the compiled function itself, or a
/// callee being built into it.
struct Frame<'a> {
  proto: &'a ObjFunction,
  feedback: &'a Feedback,
  code: &'a [Instr],
  live: typeflow::LivenessFacts,
  /// The IR block each bytecode leader starts. Empty for a callee built
  /// straight into its caller's block.
  leader_block: FxHashMap<usize, BlockId>,
  /// This frame's index in `Func::frames`.
  id: u16,
  /// Where this frame's register 0 sits.
  offset: u8,
  /// Registers of the frames around this one that have to survive the
  /// call: whatever they read after it, and the register a callee
  /// closure was taken from. Every frame state here carries them, and so
  /// does every block of this frame's own.
  outer: Vec<u8>,
  /// Where this frame's `Return` goes.
  exit: Exit,
  /// Whether the function makes closures at all, and so may leave open
  /// upvalues over its registers that returning has to close.
  makes_closures: bool,
}

/// What a `Return` does in the frame being translated.
#[derive(Clone, Copy)]
enum Exit {
  /// Returns from the compiled function.
  Root,
  /// Ends a callee built straight into its caller's block; the builder
  /// picks the caller up with the returned register's view.
  Straight,
  /// Jumps to the block that picks the caller up, passing the result
  /// first and then the frame's `outer` registers.
  Branch(BlockId),
}

impl<'a> Frame<'a> {
  fn new(
    proto: &'a ObjFunction,
    feedback: &'a Feedback,
    id: u16,
    offset: u8,
    outer: Vec<u8>,
    exit: Exit,
  ) -> Result<Self, BuildError> {
    let code = &proto.chunk.code[..];
    for (ip, instr) in code.iter().enumerate() {
      if let Some(reason) = unsupported(instr) {
        return Err(format!("{reason} at ip {ip}"));
      }
    }
    let preds = typeflow::build_predecessors(proto);
    let live = typeflow::liveness(proto, &preds);
    Ok(Frame {
      proto,
      feedback,
      code,
      live,
      leader_block: FxHashMap::default(),
      id,
      offset,
      outer,
      exit,
      makes_closures: code.iter().any(|i| matches!(i, Instr::Closure { .. })),
    })
  }
}

struct Builder<'a> {
  cx: Frame<'a>,
  /// The compiled function's own feedback.
  func_feedback: &'a Feedback,
  func: Func,
  /// For each block that starts a bytecode block, which register each
  /// parameter carries.
  param_regs: FxHashMap<BlockId, Vec<u8>>,
  regs: Vec<RegView>,
  block: BlockId,
  /// Back edges seen, as the block ending in the jump, the loop header
  /// and its position, and the frame it is in, for placing safepoints
  /// once the loop bodies are known.
  back_edges: Vec<(BlockId, BlockId, usize, u16)>,
  /// Registers a closure made in the compiled function captures. The
  /// closure reads and writes them in the register file, so every value
  /// one takes is written there as it is made, and each is read back
  /// after anything that could have run the closure. A callee that makes
  /// closures is never built in, so only the compiled function's own
  /// registers can be captured.
  captured: Vec<bool>,
  /// Set when the instruction being translated went through its runtime
  /// helper, which leaves its result in the register file already.
  in_memory: bool,
  /// The value a callee built straight into its caller returned.
  returned: Option<RegView>,
  /// The functions being built into one another, outermost first.
  inlining: Vec<usize>,
  /// Callee bytecode built in so far.
  inlined_ops: usize,
  /// One past the highest register any frame uses.
  extent: usize,
}

impl<'a> Builder<'a> {
  fn new(proto: &'a ObjFunction, feedback: &'a Feedback) -> Result<Self, BuildError> {
    let cx = Frame::new(proto, feedback, 0, 0, Vec::new(), Exit::Root)?;
    let mut captured = typeflow::captured_registers(proto);
    captured.resize(REGISTER_LIMIT, false);
    Ok(Builder {
      cx,
      func_feedback: feedback,
      func: Func::new(proto.num_registers as usize, proto as *const ObjFunction as usize),
      param_regs: FxHashMap::default(),
      regs: vec![RegView::EMPTY; REGISTER_LIMIT],
      block: BlockId(0),
      back_edges: Vec::new(),
      captured,
      in_memory: false,
      returned: None,
      inlining: vec![proto as *const ObjFunction as usize],
      inlined_ops: 0,
      extent: proto.num_registers as usize,
    })
  }

  /// A register of the frame being translated, numbered from the
  /// compiled function's frame.
  fn at(&self, r: u8) -> u8 {
    self.cx.offset + r
  }

  fn view(&self, r: u8) -> RegView {
    self.regs[self.at(r) as usize]
  }

  fn view_mut(&mut self, r: u8) -> &mut RegView {
    let at = self.at(r) as usize;
    &mut self.regs[at]
  }

  /// A new block in the frame being translated.
  fn new_block(&mut self, ip: Option<usize>) -> BlockId {
    let b = self.func.add_block(ip);
    self.func.block_mut(b).frame = self.cx.id;
    b
  }

  /// A block starting the bytecode block at `ip` of the frame being
  /// translated, taking every register live there and the frame's outer
  /// registers as parameters.
  fn leader(&mut self, ip: usize) -> BlockId {
    let b = self.new_block(Some(ip));
    let mut regs: Vec<u8> = self.cx.live.live_regs_at(ip).map(|r| self.cx.offset + r).collect();
    regs.extend(self.cx.outer.iter().copied());
    for _ in &regs {
      self.func.add_block_param(b, Ty::Tagged);
    }
    self.func.block_mut(b).param_regs = regs.clone();
    self.param_regs.insert(b, regs);
    b
  }

  fn run(mut self) -> Result<Func, BuildError> {
    let leaders = self.leaders();

    // Loop headers an interpreted frame can jump into, numbered the way
    // the baseline tier numbers them: by first appearance of a backward
    // `Jmp`.
    let mut osr_ids: FxHashMap<usize, i32> = FxHashMap::default();
    for (ip, instr) in self.cx.code.iter().enumerate() {
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
      let b = self.leader(ip);
      self.cx.leader_block.insert(ip, b);
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
    let first = self.cx.leader_block[&0];
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
        let header = self.cx.leader_block[&ip];
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
      self.func.set_term(check, Terminator::Deopt(FrameState::root(0, Vec::new())));
    }

    self.build_blocks(&leaders)?;
    self.place_safepoints();
    self.func.extent = self.extent;
    Ok(self.func)
  }

  /// Builds every bytecode block of the frame being translated.
  fn build_blocks(&mut self, leaders: &[usize]) -> Result<(), BuildError> {
    for (i, &start) in leaders.iter().enumerate() {
      let end = leaders.get(i + 1).copied().unwrap_or(self.cx.code.len());
      self.build_block(start, end)?;
    }
    Ok(())
  }

  /// Every bytecode position that starts a basic block.
  fn leaders(&self) -> Vec<usize> {
    let mut set: FxHashSet<usize> = FxHashSet::default();
    set.insert(0);
    let code = self.cx.code;
    for (ip, instr) in code.iter().enumerate() {
      match *instr {
        Instr::Jmp { offset }
        | Instr::JmpIfFalse { offset, .. }
        | Instr::JmpIfTrue { offset, .. } => {
          set.insert(jump_target(ip, offset));
          if ip + 1 < code.len() {
            set.insert(ip + 1);
          }
        },
        Instr::UsingJump { table_idx, .. } => {
          set.extend(self.cx.proto.chunk.jump_tables[table_idx as usize].values().copied());
          if ip + 1 < code.len() {
            set.insert(ip + 1);
          }
        },
        Instr::Return { .. } | Instr::Raise { .. } => {
          if ip + 1 < code.len() {
            set.insert(ip + 1);
          }
        },
        _ => {},
      }
    }
    let mut v: Vec<usize> = set.into_iter().filter(|&ip| ip < code.len()).collect();
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
    let b = self.cx.leader_block[&start];
    self.block = b;
    self.regs = vec![RegView::EMPTY; REGISTER_LIMIT];
    let params = self.func.block(b).params.clone();
    for (&r, &p) in self.param_regs[&b].clone().iter().zip(&params) {
      self.regs[r as usize] = RegView::tagged(p);
    }

    if self.translate_run(start, end)? {
      return Ok(());
    }
    // Fell off the end of the block into the next leader.
    let next = self.cx.leader_block[&end];
    let args = self.edge_args(next);
    self.func.set_term(self.block, Terminator::Jump { target: next, args });
    Ok(())
  }

  /// Translates `start..end` into the current block. Returns whether an
  /// instruction ended the block.
  fn translate_run(&mut self, start: usize, end: usize) -> Result<bool, BuildError> {
    for ip in start..end {
      let instr = self.cx.code[ip];
      self.in_memory = false;
      if self.translate(ip, instr)? {
        return Ok(true);
      }
      if let Some(dst) = typeflow::any_dst(&instr)
        && self.captured[self.at(dst) as usize]
        && !self.in_memory
      {
        let v = self.tagged(dst);
        let at = self.at(dst);
        self.push(Op::StoreReg(at), vec![v], None, None);
      }
    }
    Ok(false)
  }

  // --- representations -------------------------------------------------
  //
  // These take a register of the frame being translated; the `_at` forms
  // take one numbered from the compiled function's frame.

  fn tagged(&mut self, r: u8) -> ValueId {
    let at = self.at(r);
    self.tagged_at(at)
  }

  fn tagged_at(&mut self, at: u8) -> ValueId {
    let view = self.regs[at as usize];
    if let Some(v) = view.tagged {
      return v;
    }
    let v = if let Some(n) = view.num {
      self.value(Op::BoxF64, vec![n], Ty::Tagged)
    } else if let Some(i) = view.int {
      let n = self.value(Op::IntToF64, vec![i], Ty::F64);
      self.regs[at as usize].num = Some(n);
      self.value(Op::BoxF64, vec![n], Ty::Tagged)
    } else if let Some(c) = view.cond {
      self.value(Op::BoxBool, vec![c], Ty::Tagged)
    } else {
      // Never written on this path; the interpreter would read nil.
      self.value(Op::ConstTagged(Value::nil().to_bits()), vec![], Ty::Tagged)
    };
    self.regs[at as usize].tagged = Some(v);
    v
  }

  fn num(&mut self, r: u8, ip: usize) -> ValueId {
    let view = self.view(r);
    if let Some(n) = view.num {
      return n;
    }
    if let Some(i) = view.int {
      let n = self.value(Op::IntToF64, vec![i], Ty::F64);
      self.view_mut(r).num = Some(n);
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
    let view = self.view_mut(r);
    view.num = Some(n);
    view.known = Known::Number;
    n
  }

  fn int(&mut self, r: u8, ip: usize) -> ValueId {
    if let Some(i) = self.view(r).int {
      return i;
    }
    let source = match self.view(r).num {
      Some(n) => n,
      None => self.tagged(r),
    };
    let state = self.state(ip);
    let i = self
      .push(Op::Guard(GuardKind::Int), vec![source], Some(Ty::I64), Some(state))
      .unwrap();
    let view = self.view_mut(r);
    view.int = Some(i);
    view.known = Known::Number;
    i
  }

  /// `r` as a condition: true when the value is truthy.
  fn truthy(&mut self, r: u8) -> ValueId {
    if let Some(c) = self.view(r).cond {
      return c;
    }
    let t = self.tagged(r);
    let falsey = self.value(Op::IsFalsey, vec![t], Ty::Bool);
    self.value(Op::BNot, vec![falsey], Ty::Bool)
  }

  fn list_ptr(&mut self, r: u8, ip: usize) -> ValueId {
    let view = self.view(r);
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
    let view = self.view_mut(r);
    view.ptr = Some(p);
    view.known = Known::List;
    p
  }

  fn instance_ptr(&mut self, r: u8, class: u64, ip: usize) -> ValueId {
    let view = self.view(r);
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
    let view = self.view_mut(r);
    view.ptr = Some(p);
    view.known = Known::Instance(class);
    p
  }

  fn set_num(&mut self, r: u8, n: ValueId) {
    *self.view_mut(r) = RegView {
      num: Some(n),
      known: Known::Number,
      ..RegView::EMPTY
    };
  }

  fn set_cond(&mut self, r: u8, c: ValueId) {
    *self.view_mut(r) = RegView {
      cond: Some(c),
      known: Known::Bool,
      ..RegView::EMPTY
    };
  }

  fn set_tagged(&mut self, r: u8, t: ValueId) {
    *self.view_mut(r) = RegView::tagged(t);
  }

  /// What the interpreter needs to resume at `ip`: every register live
  /// there, tagged, and the registers the frames around it still need.
  fn state(&mut self, ip: usize) -> FrameState {
    let mut regs: Vec<u8> = self.cx.live.live_regs_at(ip).map(|r| self.cx.offset + r).collect();
    regs.extend(self.cx.outer.iter().copied());
    regs.sort_unstable();
    let regs = regs.into_iter().map(|r| (r, self.tagged_at(r))).collect();
    FrameState {
      ip,
      regs,
      frame: self.cx.id,
    }
  }

  /// The values to pass along an edge into `target`.
  fn edge_args(&mut self, target: BlockId) -> Vec<ValueId> {
    let regs = self.param_regs[&target].clone();
    regs.into_iter().map(|r| self.tagged_at(r)).collect()
  }

  /// Everything that happens after an operation that may have collected:
  /// the register file is the only place values survived, so each live
  /// register that could hold an object is read back, pointers derived
  /// from the old values are dropped, and whatever was known about each
  /// value's kind carries over. The frames around this one get the same
  /// treatment for the registers they still need.
  fn after_collect(&mut self, next_ip: usize, written: Option<u8>) {
    let live: FxHashSet<u8> = if next_ip < self.cx.code.len() {
      self.cx.live.live_regs_at(next_ip).collect()
    } else {
      FxHashSet::default()
    };
    for r in 0..self.cx.proto.num_registers {
      if Some(r) == written {
        continue;
      }
      if !live.contains(&r) {
        *self.view_mut(r) = RegView::EMPTY;
        continue;
      }
      let at = self.at(r);
      self.reload(at);
    }
    for at in self.cx.outer.clone() {
      self.reload(at);
    }
    if let Some(dst) = written {
      let at = self.at(dst);
      let fresh = self.value(Op::Reload { reg: at }, vec![], Ty::Tagged);
      self.regs[at as usize] = RegView::tagged(fresh);
    }
  }

  /// Reads register `at` back from the register file after a possible
  /// collection, when its value could be an object.
  fn reload(&mut self, at: u8) {
    let view = self.regs[at as usize];
    if self.captured[at as usize] {
      // A closure may have set it to anything at all.
      let old = self.tagged_at(at);
      let fresh = self.value(Op::Reload { reg: at }, vec![old], Ty::Tagged);
      self.regs[at as usize] = RegView::tagged(fresh);
      return;
    }
    if !view.known.may_be_object() {
      // A number or a boolean is its own bits; nothing moved.
      return;
    }
    let Some(old) = view.tagged else {
      return;
    };
    let fresh = self.value(Op::Reload { reg: at }, vec![old], Ty::Tagged);
    self.regs[at as usize] = RegView {
      tagged: Some(fresh),
      known: view.known,
      ..RegView::EMPTY
    };
  }

  /// Runs `instr` through its runtime helper.
  fn generic(&mut self, ip: usize, instr: Instr) -> Result<(), BuildError> {
    if super::lower::generic_helper(&instr).is_none() {
      return Err(format!("no runtime helper for {instr:?} at ip {ip}"));
    }
    if let Some(what) = self.baseline_faster(ip, &instr) {
      let place = if self.cx.id == 0 {
        String::new()
      } else {
        format!(" of '{}', built in,", self.cx.proto.display_name())
      };
      return Err(format!("{what} at ip {ip}{place} runs faster in the baseline tier"));
    }
    let state = self.state(ip);
    let args = state.regs.iter().map(|&(_, v)| v).collect();
    let frame = self.cx.id;
    self.push(Op::Generic { instr, ip, frame }, args, None, Some(state));
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
    let seen = self.cx.feedback.kinds.get(ip).copied().unwrap_or(0);
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
    let feedback = self.cx.feedback;
    let seen = feedback.kinds.get(ip).copied().unwrap_or(0);
    !feedback.sites_off && feedback.open(ip) && seen & !kind::NUMBER == 0
  }

  fn list_site(&self, ip: usize) -> bool {
    let feedback = self.cx.feedback;
    !feedback.sites_off && feedback.open(ip) && feedback.kinds.get(ip).copied().unwrap_or(0) == kind::LIST
  }

  fn field_site(&self, ip: usize) -> Option<(u64, u16)> {
    let feedback = self.cx.feedback;
    if feedback.fields_off || !feedback.open(ip) {
      return None;
    }
    feedback.fields.get(&ip).copied()
  }

  /// A method call whose receiver has only ever been a list: the
  /// interpreter's receiver feedback, or the key compiled code left in
  /// the site's cache.
  fn list_invoke_site(&self, ip: usize) -> bool {
    let feedback = self.cx.feedback;
    if feedback.fields_off || !feedback.open(ip) {
      return false;
    }
    feedback.kinds.get(ip).copied().unwrap_or(0) == kind::LIST
      || feedback.invokes.get(&ip) == Some(&feedback.list_key)
  }

  fn imm(&self, idx: u16) -> Option<f64> {
    let c = self.cx.proto.chunk.constants[idx as usize];
    c.is_number().then(|| c.as_number())
  }

  // --- instructions ----------------------------------------------------

  /// Translates one instruction. Returns whether it ended the block.
  fn translate(&mut self, ip: usize, instr: Instr) -> Result<bool, BuildError> {
    match instr {
      Instr::LoadConst { dst, const_idx } => {
        let c = self.cx.proto.chunk.constants[const_idx as usize];
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
        *self.view_mut(dst) = self.view(src);
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
        if self.view(a).num.is_some() && self.view(b).num.is_some() =>
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
        let eq = if let Some(x) = self.view(a).num {
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
        let r = if let Some(c) = self.view(src).cond {
          self.value(Op::BNot, vec![c], Ty::Bool)
        } else {
          let t = self.tagged(src);
          self.value(Op::IsFalsey, vec![t], Ty::Bool)
        };
        self.set_cond(dst, r);
      },

      Instr::Jmp { offset } => {
        let target_ip = jump_target(ip, offset);
        let target = self.cx.leader_block[&target_ip];
        if offset < 0 {
          self.back_edges.push((self.block, target, target_ip, self.cx.id));
        }
        let args = self.edge_args(target);
        self.func.set_term(self.block, Terminator::Jump { target, args });
        return Ok(true);
      },
      Instr::JmpIfFalse { cond, offset } | Instr::JmpIfTrue { cond, offset } => {
        let target = self.cx.leader_block[&jump_target(ip, offset)];
        let next = self.cx.leader_block[&(ip + 1)];
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
        match self.cx.exit {
          Exit::Root => {
            if self.cx.makes_closures {
              self.close_upvalues(ip, src);
            }
            let v = self.tagged(src);
            self.func.set_term(self.block, Terminator::Return(v));
          },
          Exit::Straight => {
            let view = self.view(src);
            self.returned = Some(view);
          },
          Exit::Branch(cont) => {
            let mut args = vec![self.tagged(src)];
            for at in self.cx.outer.clone() {
              args.push(self.tagged_at(at));
            }
            self.func.set_term(self.block, Terminator::Jump { target: cont, args });
          },
        }
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

      Instr::GetGlobal { dst, .. } if self.cx.feedback.globals.contains_key(&ip) => {
        let slot = self.cx.feedback.globals[&ip];
        let t = self.value(Op::LoadGlobal(slot), vec![], Ty::Tagged);
        self.set_tagged(dst, t);
      },
      Instr::SetGlobal { src, .. } | Instr::AssignGlobal { src, .. }
        if self.cx.feedback.globals.contains_key(&ip) =>
      {
        let slot = self.cx.feedback.globals[&ip];
        let v = self.tagged(src);
        self.push(Op::StoreGlobal(slot), vec![v], None, None);
      },
      Instr::CheckParamType { reg, check_idx } if inline_param_check(self.cx.proto, check_idx) => {
        let v = self.tagged(reg);
        let state = self.state(ip);
        self.push(
          Op::Guard(GuardKind::Param(check_idx)),
          vec![v],
          None,
          Some(state),
        );
        let check = &self.cx.proto.chunk.param_checks[check_idx as usize];
        let numeric = !check.nullable
          && check
            .types
            .iter()
            .all(|t| matches!(t, ParamType::Number | ParamType::Int));
        if numeric {
          self.view_mut(reg).known = Known::Number;
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
        && self.cx.proto.chunk.constants[method_const as usize].as_str() == "length" =>
      {
        let p = self.list_ptr(obj, ip);
        let len = self.value(Op::ListLen, vec![p], Ty::I64);
        *self.view_mut(dst) = RegView {
          int: Some(len),
          known: Known::Number,
          ..RegView::EMPTY
        };
      },

      Instr::Call { dst, func, num_args } if self.call_callee(ip, func).is_some() => {
        let callee = self.call_callee(ip, func).unwrap();
        let closure = FrameClosure::Reg(self.at(func));
        match self.inline_plan(ip, callee, dst, func + 1, num_args, closure) {
          Some((proto, outer)) => {
            let f = self.tagged(func);
            let state = self.state(ip);
            self.push(
              Op::Guard(GuardKind::Proto(callee.proto_bits)),
              vec![f],
              None,
              Some(state),
            );
            self.build_inline(ip, callee, proto, outer, dst, func + 1, closure)?;
          },
          None => self.generic(ip, instr)?,
        }
      },
      Instr::Invoke {
        dst,
        obj,
        num_args,
        ..
      } if self.invoke_callee(ip).is_some() => {
        let (class, callee) = self.invoke_callee(ip).unwrap();
        let closure = FrameClosure::Const(callee.closure);
        match self.inline_plan(ip, callee, dst, obj + 1, num_args + 1, closure) {
          Some((proto, outer)) => {
            self.instance_ptr(obj, class, ip);
            // The receiver's copy in the callee's first register is the
            // same value, and just as checked.
            if self.view(obj + 1).tagged == self.view(obj).tagged {
              *self.view_mut(obj + 1) = self.view(obj);
            }
            self.build_inline(ip, callee, proto, outer, dst, obj + 1, closure)?;
          },
          None => self.generic(ip, instr)?,
        }
      },

      _ => self.generic(ip, instr)?,
    }
    Ok(false)
  }

  // --- calls built in place ---------------------------------------------

  /// The callee a call through register `func` reaches, when the register
  /// was just read from a global whose closure the feedback knows.
  fn call_callee(&self, ip: usize, func: u8) -> Option<&'a Callee> {
    let feedback: &'a Feedback = self.cx.feedback;
    if !feedback.open(ip) {
      return None;
    }
    let t = self.view(func).tagged?;
    let Op::LoadGlobal(slot) = &self.func.def_inst(t)?.op else {
      return None;
    };
    feedback.global_callees.get(slot).map(|c| &**c)
  }

  /// The class a method call's receiver has had, and the method it
  /// resolves to there.
  fn invoke_callee(&self, ip: usize) -> Option<(u64, &'a Callee)> {
    let feedback: &'a Feedback = self.cx.feedback;
    if feedback.fields_off || !feedback.open(ip) {
      return None;
    }
    let class = *feedback.invokes.get(&ip)?;
    let callee = feedback.invoke_callees.get(&ip)?;
    Some((class, &**callee))
  }

  /// Whether a call at `ip` can be built in, and if so the callee's
  /// function and the registers of the frames around it that must
  /// survive the call. `window` is the caller register holding the
  /// callee's register 0, with `count` arguments from there.
  fn inline_plan(
    &self,
    ip: usize,
    callee: &'a Callee,
    dst: u8,
    window: u8,
    count: u8,
    closure: FrameClosure,
  ) -> Option<(&'a ObjFunction, Vec<u8>)> {
    // SAFETY: the VM holds every function the feedback names until the
    // compile it was gathered for has finished.
    let proto: &'a ObjFunction = unsafe { &*(callee.proto as *const ObjFunction) };
    let code = &proto.chunk.code;
    // Once the compiled function has deoptimized often enough to lose
    // its speculation, nothing is built in either: a callee's guards
    // failing would keep it deoptimizing with no recompile left to fix it.
    let root = &self.func_feedback;
    let fits = !root.sites_off
      && !root.fields_off
      && self.inlining.len() <= MAX_INLINE_DEPTH
      && !self.inlining.contains(&callee.proto)
      && code.len() <= MAX_INLINE_OPS
      && self.inlined_ops + code.len() <= MAX_INLINED_TOTAL
      && !proto.variadic
      && proto.arity == count
      && proto.upvalues.is_empty()
      && !code
        .iter()
        .any(|i| matches!(i, Instr::Closure { .. }) || unsupported(i).is_some());
    if !fits {
      return None;
    }
    let offset = self.at(window) as usize;
    if offset + proto.num_registers as usize > REGISTER_LIMIT {
      return None;
    }
    let mut outer: Vec<u8> = if ip + 1 < self.cx.code.len() {
      self
        .cx
        .live
        .live_regs_at(ip + 1)
        .filter(|&r| r != dst)
        .map(|r| self.at(r))
        .collect()
    } else {
      Vec::new()
    };
    outer.extend(self.cx.outer.iter().copied());
    if let FrameClosure::Reg(r) = closure {
      outer.push(r);
    }
    outer.sort_unstable();
    outer.dedup();
    // The callee's registers start where the call's do, so anything the
    // caller still needs has to sit below them.
    if outer.iter().any(|&r| r as usize >= offset) {
      return None;
    }
    Some((proto, outer))
  }

  /// Builds `callee`'s body in place of the call at `ip`, whose guard has
  /// already been placed, and leaves the builder in the caller after the
  /// call with the result in `dst`.
  #[allow(clippy::too_many_arguments)]
  fn build_inline(
    &mut self,
    ip: usize,
    callee: &'a Callee,
    proto: &'a ObjFunction,
    outer: Vec<u8>,
    dst: u8,
    window: u8,
    closure: FrameClosure,
  ) -> Result<(), BuildError> {
    let offset = self.at(window);
    let parent = self.cx.id;
    let id = self.func.frames.len() as u16;
    if crate::jit::log_enabled() {
      eprintln!(
        "[jit] building '{}' into '{}' at ip {}",
        proto.display_name(),
        self.cx.proto.display_name(),
        ip
      );
    }
    self.func.frames.push(InlineFrame {
      parent,
      proto: callee.proto,
      call_ip: ip,
      dst,
      offset,
      closure,
    });
    self.extent = self.extent.max(offset as usize + proto.num_registers as usize);
    self.inlined_ops += proto.chunk.code.len();
    self.inlining.push(callee.proto);

    // Everything past the arguments starts out unwritten.
    for r in proto.arity..proto.num_registers {
      self.regs[(offset + r) as usize] = RegView::EMPTY;
    }
    let known: Vec<Known> = outer.iter().map(|&r| self.regs[r as usize].known).collect();
    let straight = !proto.chunk.code.iter().any(|i| {
      matches!(
        i,
        Instr::Jmp { .. } | Instr::JmpIfFalse { .. } | Instr::JmpIfTrue { .. } | Instr::UsingJump { .. }
      )
    });

    let cont = if straight {
      None
    } else {
      let cont = self.new_block(None);
      self.func.add_block_param(cont, Ty::Tagged);
      for _ in &outer {
        self.func.add_block_param(cont, Ty::Tagged);
      }
      Some(cont)
    };
    let exit = match cont {
      Some(cont) => Exit::Branch(cont),
      None => Exit::Straight,
    };
    let frame = Frame::new(proto, &callee.feedback, id, offset, outer.clone(), exit)?;
    let caller = std::mem::replace(&mut self.cx, frame);
    self.returned = None;
    let built = if straight {
      self.translate_run(0, proto.chunk.code.len()).map(|_| ())
    } else {
      let leaders = self.leaders();
      for &l in &leaders {
        let b = self.leader(l);
        self.cx.leader_block.insert(l, b);
      }
      let entry = self.cx.leader_block[&0];
      let args = self.edge_args(entry);
      self.func.set_term(self.block, Terminator::Jump { target: entry, args });
      self.build_blocks(&leaders)
    };
    self.cx = caller;
    self.inlining.pop();
    built?;

    // The callee's registers mean nothing to the caller once it returns.
    for r in 0..proto.num_registers {
      self.regs[(offset + r) as usize] = RegView::EMPTY;
    }
    match cont {
      None => match self.returned.take() {
        Some(view) => *self.view_mut(dst) = view,
        None => {
          // Every way through the callee raised, so nothing comes after
          // the call; what follows is built into a block nothing reaches.
          self.block = self.new_block(None);
          *self.view_mut(dst) = RegView::EMPTY;
        },
      },
      Some(cont) => {
        self.block = cont;
        let params = self.func.block(cont).params.clone();
        for ((&r, &p), &k) in outer.iter().zip(&params[1..]).zip(&known) {
          self.regs[r as usize] = RegView {
            tagged: Some(p),
            known: if self.captured[r as usize] { Known::Unknown } else { k },
            ..RegView::EMPTY
          };
        }
        *self.view_mut(dst) = RegView::tagged(params[0]);
      },
    }
    Ok(())
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
    self.push(Op::Generic { instr, ip, frame: 0 }, args, None, Some(state));
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
        reg: self.at(subject),
        frame: self.cx.id,
      },
      vec![t],
      Ty::I64,
    );

    let mut targets: Vec<usize> = self.cx.proto.chunk.jump_tables[table_idx as usize]
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
        let b = self.cx.leader_block[t];
        (b, self.edge_args(b))
      })
      .collect();
    let miss = self.cx.leader_block[&(ip + 1)];
    let miss_args = self.edge_args(miss);

    for ((block, args), &target_ip) in edges.into_iter().zip(&targets) {
      let want = self.value(Op::ConstI64(target_ip as i64), vec![], Ty::I64);
      let hit = self.value(Op::ICmp(Cmp::Eq), vec![found, want], Ty::Bool);
      let next = self.new_block(None);
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
    for (latch, header, header_ip, frame) in edges {
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
        frame,
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
