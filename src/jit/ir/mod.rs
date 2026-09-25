//! The optimizing tier's intermediate representation.
//!
//! The baseline tier (`jit::codegen`) translates bytecode straight into
//! Cranelift, one instruction at a time, and has to settle every question
//! about a value at the point it is used. This tier first builds the
//! function as SSA over typed values, runs whole-function passes over
//! that, and only then lowers it to Cranelift. A fact proven once holds
//! for every later use, a guard that dominates another makes the second
//! one redundant, and a check that cannot change inside a loop moves out
//! of it.
//!
//! # Values
//!
//! Every value has a type that says how it is represented in a machine
//! register, not what Zuri type it is:
//!
//! - `Tagged`: a NaN-boxed `Value`, exactly as it sits in `VM::registers`.
//! - `F64`: a number, unboxed. A Zuri number's bits are its `f64` bits, so
//!   moving between the two is free.
//! - `I64`: a whole number known to fit, used for indices and counters.
//! - `Bool`: a condition, one byte.
//! - `Ptr`: a raw pointer into a heap object: the object itself, a list's
//!   element buffer, an instance's fields. Only valid until the next point
//!   where a collection can run, which the builder guarantees by never
//!   letting one live across such a point; see below.
//!
//! # Leaving compiled code
//!
//! A guard that fails deoptimizes: the frame state attached to it names
//! the bytecode position to resume at and the value every live register
//! holds there. Lowering writes those values back into `VM::registers`
//! and hands the frame to the interpreter.
//!
//! Anything this tier does not specialize runs through the same runtime
//! helper the baseline tier calls for it, which reads and writes the
//! register file. Such an operation carries a frame state too: its live
//! registers are written out before the call and its result is read back
//! after.
//!
//! # Moving collection
//!
//! The nursery moves objects. Any operation that can allocate, call Zuri
//! code or reach a safepoint is marked as one that may collect, and the
//! builder follows every such operation with a fresh read of each live
//! register that might hold a heap object. Uses after that point refer to
//! the new values, so a pointer derived before a collection can never be
//! used after it, and no pass has to reason about collection on its own.
//!
//! # Inlined calls
//!
//! A call to a function the feedback names is built into the caller's IR
//! rather than made. The callee's registers sit where a real call would
//! put them, just past the caller's call window, so every register
//! number in the IR is relative to the compiled function's own frame.
//! A frame state inside an inlined body belongs to that body's frame in
//! `Func::frames`, which says how to rebuild the interpreter frames a
//! real call would have pushed when compiled code gives up there.

pub mod build;
pub mod lower;
pub mod passes;

use std::fmt;

use rustc_hash::FxHashMap;

use crate::vm::chunk::Instr;

/// How a value is represented in a machine register.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Ty {
  Tagged,
  F64,
  I64,
  Bool,
  Ptr,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ValueId(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BlockId(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct InstId(pub u32);

/// The comparisons `FCmp` and `ICmp` perform.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Cmp {
  Eq,
  Ne,
  Lt,
  Le,
  Gt,
  Ge,
}

impl Cmp {
  /// The same comparison with its operands exchanged.
  pub fn swapped(self) -> Cmp {
    match self {
      Cmp::Eq => Cmp::Eq,
      Cmp::Ne => Cmp::Ne,
      Cmp::Lt => Cmp::Gt,
      Cmp::Le => Cmp::Ge,
      Cmp::Gt => Cmp::Lt,
      Cmp::Ge => Cmp::Le,
    }
  }
}

/// Bits no Zuri value has: an object at a null address. An operation
/// answers this where the method it stands for would raise or has no
/// answer, and a guard after it sends that case to the interpreter.
pub const NO_VALUE: u64 = crate::vm::value::QNAN | crate::vm::value::SIGN_BIT;

/// Where compiled code resumes in the interpreter, and what every live
/// register holds at that point.
///
/// `ip` is a position in the bytecode of `frame`, an index into
/// `Func::frames`. The registers are those of every frame from the
/// compiled function's own down to that one, numbered from the compiled
/// function's frame.
///
/// A guard moved away from where the bytecode checks it resumes at the
/// place it moved to, but its failure belongs to the check it was:
/// `blame` names that one, as a frame and a position, so the site that
/// stops speculating is the one that was wrong.
#[derive(Clone, Debug, PartialEq)]
pub struct FrameState {
  pub ip: usize,
  pub regs: Vec<(u8, ValueId)>,
  pub frame: u16,
  pub blame: Option<(u16, usize)>,
}

impl FrameState {
  /// A state for the compiled function's own frame.
  pub fn root(ip: usize, regs: Vec<(u8, ValueId)>) -> FrameState {
    FrameState {
      ip,
      regs,
      frame: 0,
      blame: None,
    }
  }

  /// The frame and position whose speculation failed when this state
  /// is left through.
  pub fn site(&self) -> (u16, usize) {
    self.blame.unwrap_or((self.frame, self.ip))
  }
}

/// Where the closure an inlined frame runs comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameClosure {
  /// The register the call took its callee from.
  Reg(u8),
  /// A method, which lives as long as its class and never moves.
  Const(u64),
}

/// One function body in a compiled function: the function itself, at
/// index 0, and every call built into it.
#[derive(Clone, Debug)]
pub struct InlineFrame {
  /// The frame the call was made from. The compiled function's own
  /// frame is its own parent.
  pub parent: u16,
  /// The `ObjFunction` running in this frame.
  pub proto: usize,
  /// Where in the parent the call is.
  pub call_ip: usize,
  /// Which register the call's result goes to, in the parent's own
  /// numbering.
  pub dst: u8,
  /// Where this frame's register 0 sits.
  pub offset: u8,
  pub closure: FrameClosure,
}

/// An integer operation checked against the interpreter's doubles.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum IntOp {
  Add,
  Sub,
  Mul,
}

/// A bitwise operator on two `I64`s, with Zuri's rules for the shifts:
/// the amount is its low 32 bits, and an amount of the operand's width or
/// more gives 0 rather than wrapping around.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BitOp {
  And,
  Or,
  Xor,
  Shl,
  /// `>>`, keeping the sign.
  Shr,
  /// `>>>`, on the low 32 bits of the left operand taken as unsigned.
  Ushr,
}

/// A number method that works on its receiver alone, as an `F64` to
/// `F64` operation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FUnary {
  Sqrt,
  Abs,
  Floor,
  Ceil,
  Trunc,
  /// Half away from zero.
  Round,
  /// -1, 1, or the receiver itself when it is a zero of either sign.
  Sign,
  /// Toward zero and saturated to the `i64` range, as `int()` gives.
  Int,
}

/// A number method that answers a question about its receiver.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FTest {
  IsNan,
  IsInf,
  IsFinite,
  /// `to_bool()`: at or above zero, which NaN is not.
  NonNegative,
}

/// What a guard checks. A failed guard deoptimizes through its frame
/// state.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum GuardKind {
  /// The value is a number. Produces it as an `F64`.
  Number,
  /// The value is a whole number that fits an `i64`. Produces the
  /// integer.
  Int,
  /// The value is a boolean. Produces it as a `Bool`.
  Bool,
  /// The value is a list. Produces its object pointer.
  List,
  /// The value is an instance of the class whose `Value` bits are given.
  /// Produces its object pointer.
  Instance(u64),
  /// The value is a whole number an `I64` holds exactly, and not -0.
  /// Integer arithmetic has one zero, so an operand it takes must not be
  /// the zero it would lose the sign of. Produces the `I64`.
  Whole,
  /// `op` on two `I64`s from `Whole` guards or earlier arithmetic, whose
  /// result is the interpreter's double exactly: no overflow, within
  /// `-2^53..2^53`, and not a zero the doubles would make -0, as a
  /// product with a negative operand does. Produces the `I64`.
  Arith(IntOp),
  /// The value is a heap object with this `object::OBJ_TAG_*` tag: a
  /// string, bytes, dict or range. Produces its object pointer.
  Tag(u8),
  /// An `I64` index is inside `0..len`, where `len` is the second
  /// operand. Produces nothing.
  Bounds,
  /// A `Bool` is true. Produces nothing.
  True,
  /// The value passes parameter type check `check` of the function in
  /// `frame`. The frame is named here rather than taken from the guard's
  /// state, since a guard hoisted out of a loop resumes at the loop header,
  /// which may sit in an outer frame. Only made for checks whose every type
  /// can be tested inline. Produces nothing; a failed check leaves the
  /// interpreter to raise.
  Param { frame: u16, check: u16 },
  /// The value is a closure whose prototype is the function whose `Value`
  /// bits are given. Produces nothing.
  Proto(u64),
  /// Every element of the list whose pointer is the operand is a number,
  /// or with `whole` set a whole number an `i64` holds. Produces nothing.
  Elems { whole: bool },
}

impl GuardKind {
  pub fn result_ty(self) -> Option<Ty> {
    match self {
      GuardKind::Number => Some(Ty::F64),
      GuardKind::Int | GuardKind::Whole | GuardKind::Arith(_) => Some(Ty::I64),
      GuardKind::Bool => Some(Ty::Bool),
      GuardKind::List | GuardKind::Instance(_) | GuardKind::Tag(_) => Some(Ty::Ptr),
      GuardKind::Bounds
      | GuardKind::True
      | GuardKind::Param { .. }
      | GuardKind::Proto(_)
      | GuardKind::Elems { .. } => None,
    }
  }
}

/// One operation. Operands are `ValueId`s held in `Inst::args`, in the
/// order each variant's docs give.
#[derive(Clone, Debug)]
pub enum Op {
  /// A constant `Value`, by its bits.
  ConstTagged(u64),
  ConstF64(f64),
  ConstI64(i64),
  ConstBool(bool),

  /// Which on-stack-replacement entry this call came in through, or -1
  /// for an ordinary call. Only appears in the prologue.
  OsrIndex,
  /// A register as it stands on entry, read from the register file.
  /// Only appears on the ordinary call's path out of the prologue.
  Param(u8),
  /// A register read from the register file where an interpreted frame
  /// left it, on entry through on-stack replacement. Only appears in an
  /// on-stack-replacement entry block.
  OsrParam(u8),
  /// A register read back from the register file after an operation
  /// that may have collected. Its one operand is the value it replaces,
  /// so whatever was known about that value's kind carries over; a
  /// collection moves objects but never changes what kind they are.
  Reload { reg: u8 },

  /// A global from `VM::global_slots`, by the slot the interpreter
  /// resolved it to.
  LoadGlobal(u32),
  /// Stores into a resolved global slot. The slots are roots, so no
  /// barrier. Operand: the value.
  StoreGlobal(u32),

  /// `F64` to `Tagged`.
  BoxF64,
  /// `Bool` to `Tagged`.
  BoxBool,
  /// `I64` to `F64`.
  IntToF64,
  /// `F64` to `I64`, for a number already known to be an integer in
  /// range, so the conversion is exact and needs no check.
  F64ToI64,
  /// A `Tagged` proven to be a number, as an `F64`.
  UnboxF64,
  /// A `Tagged` proven to be a boolean, as a `Bool`.
  UnboxBool,
  /// A `Tagged` proven to be a heap object, as its pointer.
  ObjPtr,

  /// Checks its operand and deoptimizes if the check fails.
  Guard(GuardKind),

  FAdd,
  FSub,
  FMul,
  FDiv,
  FNeg,
  /// Zuri's `%`: the truncated remainder, which takes the sign of the
  /// dividend.
  FMod,
  /// Zuri's `//`: floor of the quotient.
  FFloorDiv,
  FCmp(Cmp),
  IAdd,
  ISub,
  IMul,
  ICmp(Cmp),
  IBit(BitOp),
  /// An `F64` as the `I64` Zuri's bitwise operators work on: truncated,
  /// wrapped modulo 2^64, and 0 for an infinity or NaN.
  WrapI64,
  /// An integer view as the bitwise operators want it. A view holds
  /// `i64::MAX` only when its number is 2^63, which saturates there on
  /// conversion and wraps to `i64::MIN`; this gives `i64::MIN` for it.
  Unsaturate,
  /// Zuri truthiness of a `Tagged`, as a `Bool` that is true when the
  /// value is falsey.
  IsFalsey,
  BNot,
  /// A `Tagged` equals a number constant, by Zuri's `==`: false for
  /// anything that is not a number.
  EqConst(f64),
  /// Zuri's `==` on two `Tagged` values, as a `Bool`. Lists and dicts
  /// compare by content, so this reads memory a store can change and is
  /// never merged with or moved past another.
  TaggedEq,
  /// `**` on two `F64`s.
  FPow,
  /// A number method on an `F64`.
  FUnary(FUnary),
  /// `max()` and `min()` on two `F64`s: a NaN gives way to the other
  /// number, and `-0` counts as less than `0`.
  FMax,
  FMin,
  /// A number predicate on an `F64`, as a `Bool`.
  FTest(FTest),
  /// A number method the runtime computes, through the named helper,
  /// on one or two `F64`s.
  FCall(&'static str),

  /// Element count of a list, from its object pointer.
  ListLen,
  /// Element buffer of a list, from its object pointer.
  ListData,
  /// Element `idx` of an element buffer. Operands: data, idx.
  LoadElem,
  /// A list's first element, or its last with `last` set, or nil when it
  /// has none. Operand: list pointer.
  ListEnd { last: bool },
  /// `append()`: adds a value to the end of a list, growing it when it
  /// is full, and runs the write barrier. Growing takes memory outside
  /// the collected heap, so it never collects. Operands: list pointer,
  /// value.
  ListAppend,
  /// Stores into element `idx` and runs the write barrier. Operands: list
  /// pointer, data, idx, value.
  StoreElem,
  /// Field slot `n` of an instance. Operand: instance pointer.
  LoadField(u16),
  /// Stores field slot `n` and runs the write barrier. Operands: instance
  /// pointer, value.
  StoreField(u16),

  /// Whether a string holds only ASCII, so that its byte length is its
  /// length and its byte at an index is its character there. Classifies
  /// the string, flattening a rope, the first time anything asks. A
  /// string never changes, so neither does the answer. Operand: string
  /// pointer.
  StrAscii,
  /// A flat string's byte length. Operand: string pointer.
  StrByteLen,
  /// The byte at an index of a flat string. Operands: string pointer,
  /// index.
  StrByte,
  /// The one-character string for an ASCII byte, from the VM's own
  /// table. Operand: the byte.
  AsciiChar,
  /// `length()` on a string: its codepoint count, which a rope keeps
  /// without being flattened. Operand: string pointer.
  StrLength,
  /// `ord()` on a string, as an `F64`: NaN where the method would raise,
  /// for a guard to catch. Operand: string pointer.
  StrOrd,
  /// An index counted from the end when it is negative: `idx + len`
  /// then, `idx` otherwise. Operands: index, length.
  WrapIndex,
  /// The index `get()` reads: the number with its fraction dropped, or
  /// -1 for a negative number or NaN, which no length admits. Operand:
  /// an `F64`.
  GetIndex,
  /// A byte stream's length. Operand: bytes pointer.
  BytesLen,
  /// The byte at an index. Operands: bytes pointer, index.
  BytesLoad,
  /// Stores a byte. Operands: bytes pointer, index, the byte as an
  /// `I64` already checked to be in range.
  BytesStore,
  /// `get(idx, fallback)` on a list: the element, or the fallback when
  /// the index is out of range. Operands: list pointer, index, fallback.
  ListGetOr,
  /// A dict's value for a key, or the fallback when it has none.
  /// Operands: dict, key, fallback.
  DictGet,
  /// Sets a dict's value for a key, with the write barrier. Growing the
  /// table takes memory outside the collected heap, so this never
  /// collects. Operands: dict, key, value.
  DictSet,
  /// Whether a dict has a key. Operands: dict, key.
  DictContains,
  /// A dict's entry count. Operand: dict.
  DictLen,
  /// `length()` on a list, string, bytes or dict, whichever it is, or -1
  /// for anything else. Operand: the value.
  ObjLength,
  /// Whether a `Tagged` is exactly the given bits.
  IsBits(u64),
  /// The iteration protocol's next key for a sequence of the given
  /// length: 0 after nil (nil for an empty one), `k + 1` after `k` while
  /// that is in range, nil at the end. `NO_VALUE` for a key that is
  /// neither nil nor a number. Operands: previous key, length.
  NextKey,
  /// How many values a range yields. Operand: range pointer.
  RangeCount,
  /// The range's value at a position, or nil past the end. Operands:
  /// range pointer, position as an `F64`.
  RangeAt,
  /// A list of up to two elements, allocated inline in the nursery. The
  /// runtime makes it instead when the nursery is full, which may
  /// collect, so the reloads after it read memory only then. `dst`,
  /// `start` and `count` are the `MakeList` instruction's own, in
  /// `frame`. Operands: the elements.
  NewList { dst: u8, start: u8, count: u8, frame: u16 },

  /// Writes a register straight to the register file. A register some
  /// closure here captures is read there by the closure, so every value
  /// it takes goes to memory as it is made. Operand: the value.
  StoreReg(u8),
  /// The running closure's upvalue cell `n`, as its object pointer.
  /// Deoptimizes if the slot holds anything but an upvalue, which a
  /// closure built from this function never does.
  UpvalCell(u8),
  /// The value an upvalue cell holds, open or closed. Operand: the cell.
  LoadUpval,
  /// Sets the value an upvalue cell holds, with the write barrier a
  /// closed cell needs. Operands: the cell, the value.
  StoreUpval,
  /// Looks up a `using` subject in jump table `table` of `frame`'s
  /// function, giving the target position or `USING_NO_MATCH`. The
  /// subject is written to register `reg` first, where the runtime reads
  /// it. Operand: the subject.
  UsingTarget { table: u16, reg: u8, frame: u16 },

  /// Runs one bytecode instruction of `frame`'s function through the
  /// runtime helper the baseline tier uses for it. The frame state's
  /// registers are written out first, which covers everything the
  /// instruction reads; the result, if any, is its destination register
  /// read back afterwards. `ip` is the instruction's own position, which
  /// some helpers need to find their inline cache. The instruction's
  /// registers are numbered from `frame`'s register 0.
  Generic { instr: Instr, ip: usize, frame: u16 },

  /// A GC and signal safepoint.
  Safepoint,
  /// Marks the deoptimization ending this block as one for code that had
  /// not run when the function was compiled, so there was nothing to
  /// build it from. The interpreter runs it and records what it sees, the
  /// compiled code is thrown away, and the next compile builds the site
  /// from that feedback instead of giving up on it.
  Unreached,
}

impl Op {
  /// Whether this operation can allocate, call Zuri code or collect.
  /// Every value derived from a heap pointer before one of these is
  /// stale after it.
  pub fn may_collect(&self) -> bool {
    matches!(self, Op::Generic { .. } | Op::Safepoint | Op::NewList { .. })
  }

  /// Whether this operation reads or writes memory another operation
  /// could change, so it cannot be merged with or moved past such an
  /// operation freely.
  pub fn touches_memory(&self) -> bool {
    matches!(
      self,
      Op::ListLen
        | Op::ListData
        | Op::LoadElem
        | Op::ListEnd { .. }
        | Op::ListAppend
        | Op::ListGetOr
        | Op::BytesLen
        | Op::BytesLoad
        | Op::BytesStore
        | Op::DictGet
        | Op::DictSet
        | Op::DictContains
        | Op::DictLen
        | Op::ObjLength
        | Op::NewList { .. }
        | Op::StoreElem
        | Op::LoadField(_)
        | Op::StoreField(_)
        | Op::Generic { .. }
        | Op::Safepoint
        | Op::Reload { .. }
        | Op::Param(_)
        | Op::OsrParam(_)
        | Op::LoadGlobal(_)
        | Op::StoreGlobal(_)
        | Op::StoreReg(_)
        | Op::TaggedEq
        | Op::UpvalCell(_)
        | Op::LoadUpval
        | Op::StoreUpval
        | Op::UsingTarget { .. }
    )
  }

  /// Whether this operation has an effect beyond producing its result,
  /// so it must stay even when the result is unused.
  pub fn has_effect(&self) -> bool {
    matches!(
      self,
      Op::Guard(_)
        | Op::ListAppend
        | Op::BytesStore
        | Op::DictSet
        | Op::NewList { .. }
        | Op::StoreElem
        | Op::StoreField(_)
        | Op::StoreGlobal(_)
        | Op::StoreReg(_)
        | Op::UpvalCell(_)
        | Op::StoreUpval
        | Op::UsingTarget { .. }
        | Op::Generic { .. }
        | Op::Safepoint
        | Op::Unreached
    )
  }
}

#[derive(Clone, Debug)]
pub struct Inst {
  pub op: Op,
  pub args: Vec<ValueId>,
  pub result: Option<ValueId>,
  /// Present on every operation that can leave compiled code: a guard,
  /// a runtime helper, a safepoint.
  pub state: Option<FrameState>,
}

#[derive(Clone, Debug)]
pub enum Terminator {
  Jump { target: BlockId, args: Vec<ValueId> },
  Branch {
    cond: ValueId,
    then_block: BlockId,
    then_args: Vec<ValueId>,
    else_block: BlockId,
    else_args: Vec<ValueId>,
  },
  /// Returns a `Tagged` value to the caller.
  Return(ValueId),
  /// Hands the frame to the interpreter at the state's position.
  Deopt(FrameState),
  /// Placeholder while the block is being built.
  Unset,
}

impl Terminator {
  pub fn successors(&self) -> Vec<BlockId> {
    match self {
      Terminator::Jump { target, .. } => vec![*target],
      Terminator::Branch {
        then_block,
        else_block,
        ..
      } => vec![*then_block, *else_block],
      Terminator::Return(_) | Terminator::Deopt(_) | Terminator::Unset => vec![],
    }
  }

  pub fn uses(&self) -> Vec<ValueId> {
    match self {
      Terminator::Jump { args, .. } => args.clone(),
      Terminator::Branch {
        cond,
        then_args,
        else_args,
        ..
      } => {
        let mut v = vec![*cond];
        v.extend(then_args);
        v.extend(else_args);
        v
      },
      Terminator::Return(v) => vec![*v],
      Terminator::Deopt(state) => state.regs.iter().map(|&(_, v)| v).collect(),
      Terminator::Unset => vec![],
    }
  }
}

#[derive(Clone, Debug)]
pub struct Block {
  /// Values flowing in along every edge, one per parameter.
  pub params: Vec<ValueId>,
  /// For a block that starts a bytecode block, the register each
  /// parameter carries, in the same order. What an on-stack-replacement
  /// entry reads from the register file to fill them.
  pub param_regs: Vec<u8>,
  /// Registers live on entry that no longer have a parameter because
  /// every edge brings the same value, with that value. A frame state
  /// for resuming at this block's position needs them as much as the
  /// parameters.
  pub fixed_regs: Vec<(u8, ValueId)>,
  pub insts: Vec<InstId>,
  pub term: Terminator,
  /// The bytecode position this block starts at, when it starts one.
  pub ip: Option<usize>,
  /// The frame whose bytecode `ip` is in.
  pub frame: u16,
}

/// Where a value comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ValueDef {
  Inst(InstId),
  Param(BlockId, u32),
}

#[derive(Clone, Debug)]
pub struct ValueData {
  pub ty: Ty,
  pub def: ValueDef,
}

/// One function in the optimizing tier's IR.
#[derive(Clone, Debug)]
pub struct Func {
  pub blocks: Vec<Block>,
  pub insts: Vec<Inst>,
  pub values: Vec<ValueData>,
  pub entry: BlockId,
  pub num_registers: usize,
  /// One past the highest register any frame uses, counting the calls
  /// built in. The register file has to reach this far, and a collection
  /// has to look this far, while the function runs.
  pub extent: usize,
  /// How many young bytes may be allocated before a collection is owed,
  /// as the heap had it when the function was compiled.
  pub young_budget: u64,
  /// Loop header ip to its on-stack-replacement id, matching what the
  /// interpreter looks up to jump into compiled code mid-loop.
  pub osr_ids: FxHashMap<usize, i32>,
  /// The block each on-stack-replacement id enters at. Like `entry`, a
  /// root of the control flow graph: it reads the loop header's registers
  /// from memory and jumps to the header, so nothing computed before the
  /// loop on the ordinary path is available through it.
  pub osr_entries: Vec<(i32, BlockId)>,
  /// The compiled function's own frame, then one entry for each call
  /// built into it.
  pub frames: Vec<InlineFrame>,
}

impl Func {
  pub fn new(num_registers: usize, proto: usize) -> Func {
    Func {
      blocks: Vec::new(),
      insts: Vec::new(),
      values: Vec::new(),
      entry: BlockId(0),
      num_registers,
      extent: num_registers,
      young_budget: 0,
      osr_ids: FxHashMap::default(),
      osr_entries: Vec::new(),
      frames: vec![InlineFrame {
        parent: 0,
        proto,
        call_ip: 0,
        dst: 0,
        offset: 0,
        closure: FrameClosure::Reg(0),
      }],
    }
  }

  pub fn add_block(&mut self, ip: Option<usize>) -> BlockId {
    let id = BlockId(self.blocks.len() as u32);
    self.blocks.push(Block {
      params: Vec::new(),
      param_regs: Vec::new(),
      fixed_regs: Vec::new(),
      insts: Vec::new(),
      term: Terminator::Unset,
      ip,
      frame: 0,
    });
    id
  }

  /// A new, empty block at the same bytecode position as `b`, in the
  /// same frame.
  pub fn add_block_like(&mut self, b: BlockId) -> BlockId {
    let (ip, frame) = (self.block(b).ip, self.block(b).frame);
    let id = self.add_block(ip);
    self.block_mut(id).frame = frame;
    id
  }

  /// Every register live on entry to `b`, for a frame state that
  /// resumes at its position: its parameters' registers paired with
  /// `values`, which stand for the parameters on the edge in question,
  /// and the registers whose parameters were folded away.
  pub fn entry_regs(&self, b: BlockId, values: &[ValueId]) -> Vec<(u8, ValueId)> {
    let block = self.block(b);
    let mut regs: Vec<(u8, ValueId)> =
      block.param_regs.iter().copied().zip(values.iter().copied()).collect();
    regs.extend(block.fixed_regs.iter().copied());
    regs.sort_unstable_by_key(|&(r, _)| r);
    regs
  }

  pub fn add_block_param(&mut self, block: BlockId, ty: Ty) -> ValueId {
    let index = self.blocks[block.0 as usize].params.len() as u32;
    let v = ValueId(self.values.len() as u32);
    self.values.push(ValueData {
      ty,
      def: ValueDef::Param(block, index),
    });
    self.blocks[block.0 as usize].params.push(v);
    v
  }

  /// Appends an operation to `block`, returning its result if it has one.
  pub fn push(
    &mut self,
    block: BlockId,
    op: Op,
    args: Vec<ValueId>,
    result_ty: Option<Ty>,
    state: Option<FrameState>,
  ) -> Option<ValueId> {
    let id = InstId(self.insts.len() as u32);
    let result = result_ty.map(|ty| {
      let v = ValueId(self.values.len() as u32);
      self.values.push(ValueData {
        ty,
        def: ValueDef::Inst(id),
      });
      v
    });
    self.insts.push(Inst {
      op,
      args,
      result,
      state,
    });
    self.blocks[block.0 as usize].insts.push(id);
    result
  }

  /// Inserts an operation into `block` at position `pos`, returning its
  /// result if it has one.
  pub fn insert(
    &mut self,
    block: BlockId,
    pos: usize,
    op: Op,
    args: Vec<ValueId>,
    result_ty: Option<Ty>,
    state: Option<FrameState>,
  ) -> Option<ValueId> {
    let result = self.push(block, op, args, result_ty, state);
    let id = self.blocks[block.0 as usize].insts.pop().unwrap();
    self.blocks[block.0 as usize].insts.insert(pos, id);
    result
  }

  /// The edges out of `block` that lead to `target`, as their argument
  /// lists, for passes that rewrite what flows along them.
  pub fn edge_args_mut(&mut self, block: BlockId, target: BlockId) -> Vec<&mut Vec<ValueId>> {
    match &mut self.blocks[block.0 as usize].term {
      Terminator::Jump { target: t, args } if *t == target => vec![args],
      Terminator::Branch {
        then_block,
        then_args,
        else_block,
        else_args,
        ..
      } => {
        let mut edges = Vec::new();
        if *then_block == target {
          edges.push(then_args);
        }
        if *else_block == target {
          edges.push(else_args);
        }
        edges
      },
      _ => Vec::new(),
    }
  }

  /// Points every edge from `block` to `from` at `to` instead.
  pub fn retarget(&mut self, block: BlockId, from: BlockId, to: BlockId) {
    match &mut self.blocks[block.0 as usize].term {
      Terminator::Jump { target, .. } => {
        if *target == from {
          *target = to;
        }
      },
      Terminator::Branch {
        then_block,
        else_block,
        ..
      } => {
        if *then_block == from {
          *then_block = to;
        }
        if *else_block == from {
          *else_block = to;
        }
      },
      _ => {},
    }
  }

  pub fn ty(&self, v: ValueId) -> Ty {
    self.values[v.0 as usize].ty
  }

  pub fn block(&self, b: BlockId) -> &Block {
    &self.blocks[b.0 as usize]
  }

  pub fn block_mut(&mut self, b: BlockId) -> &mut Block {
    &mut self.blocks[b.0 as usize]
  }

  pub fn inst(&self, i: InstId) -> &Inst {
    &self.insts[i.0 as usize]
  }

  /// Which operation defines `v`, when an operation does.
  pub fn def_inst_id(&self, v: ValueId) -> Option<InstId> {
    match self.values[v.0 as usize].def {
      ValueDef::Inst(i) => Some(i),
      ValueDef::Param(..) => None,
    }
  }

  /// The operation defining `v`, when an operation does.
  pub fn def_inst(&self, v: ValueId) -> Option<&Inst> {
    match self.values[v.0 as usize].def {
      ValueDef::Inst(i) => Some(self.inst(i)),
      ValueDef::Param(..) => None,
    }
  }

  pub fn set_term(&mut self, b: BlockId, term: Terminator) {
    self.blocks[b.0 as usize].term = term;
  }

  /// Predecessors of every block, in block order.
  pub fn predecessors(&self) -> Vec<Vec<BlockId>> {
    let mut preds = vec![Vec::new(); self.blocks.len()];
    for (i, b) in self.blocks.iter().enumerate() {
      for s in b.term.successors() {
        preds[s.0 as usize].push(BlockId(i as u32));
      }
    }
    preds
  }

  /// Every block control can start at: the ordinary entry, then each
  /// on-stack-replacement entry.
  /// Where control can start. The prologue dispatches every entry,
  /// on-stack replacement included, so there is exactly one.
  pub fn roots(&self) -> Vec<BlockId> {
    vec![self.entry]
  }

  /// Blocks reachable from a root in reverse postorder, the order passes
  /// and lowering visit them in so every definition is seen before its
  /// uses outside loops.
  pub fn reverse_postorder(&self) -> Vec<BlockId> {
    let mut seen = vec![false; self.blocks.len()];
    let mut post = Vec::with_capacity(self.blocks.len());
    // Later roots first, so after the reversal the ordinary entry leads.
    for root in self.roots().into_iter().rev() {
      if seen[root.0 as usize] {
        continue;
      }
      seen[root.0 as usize] = true;
      let mut stack: Vec<(BlockId, usize)> = vec![(root, 0)];
      while let Some((b, next)) = stack.pop() {
        let succs = self.block(b).term.successors();
        if next < succs.len() {
          stack.push((b, next + 1));
          let s = succs[next];
          if !seen[s.0 as usize] {
            seen[s.0 as usize] = true;
            stack.push((s, 0));
          }
        } else {
          post.push(b);
        }
      }
    }
    post.reverse();
    post
  }

  /// Checks the structural rules every pass relies on and returns the
  /// first one broken. Run after building and after every pass in debug
  /// builds.
  pub fn verify(&self) -> Result<(), String> {
    let order = self.reverse_postorder();
    let doms = dominators(self, &order);
    let mut def_block: Vec<Option<BlockId>> = vec![None; self.values.len()];
    let mut def_pos: Vec<usize> = vec![0; self.values.len()];
    for (bi, b) in self.blocks.iter().enumerate() {
      for &p in &b.params {
        def_block[p.0 as usize] = Some(BlockId(bi as u32));
      }
      for (pos, &i) in b.insts.iter().enumerate() {
        if let Some(r) = self.inst(i).result {
          def_block[r.0 as usize] = Some(BlockId(bi as u32));
          def_pos[r.0 as usize] = pos + 1;
        }
      }
    }
    let reachable: Vec<bool> = {
      let mut r = vec![false; self.blocks.len()];
      for b in &order {
        r[b.0 as usize] = true;
      }
      r
    };

    let check_use = |user: BlockId, pos: usize, v: ValueId| -> Result<(), String> {
      let Some(db) = def_block[v.0 as usize] else {
        return Err(format!("v{} used in b{} but never defined", v.0, user.0));
      };
      if db == user {
        if def_pos[v.0 as usize] > pos {
          return Err(format!("v{} used in b{} before its definition", v.0, user.0));
        }
        return Ok(());
      }
      if !dominates(&doms, db, user) {
        return Err(format!(
          "v{} defined in b{} does not dominate its use in b{}",
          v.0, db.0, user.0
        ));
      }
      Ok(())
    };

    for &b in &order {
      let block = self.block(b);
      if matches!(block.term, Terminator::Unset) {
        return Err(format!("b{} has no terminator", b.0));
      }
      for (pos, &i) in block.insts.iter().enumerate() {
        let inst = self.inst(i);
        for &a in &inst.args {
          check_use(b, pos, a)?;
        }
        if let Some(state) = &inst.state {
          for &(_, v) in &state.regs {
            check_use(b, pos, v)?;
          }
        }
      }
      for v in block.term.uses() {
        check_use(b, block.insts.len(), v)?;
      }
      let term_edges: Vec<(BlockId, &Vec<ValueId>)> = match &block.term {
        Terminator::Jump { target, args } => vec![(*target, args)],
        Terminator::Branch {
          then_block,
          then_args,
          else_block,
          else_args,
          ..
        } => vec![(*then_block, then_args), (*else_block, else_args)],
        _ => vec![],
      };
      for (target, args) in term_edges {
        let params = &self.block(target).params;
        if !reachable[target.0 as usize] {
          return Err(format!("b{} jumps to unreachable b{}", b.0, target.0));
        }
        if params.len() != args.len() {
          return Err(format!(
            "b{} passes {} values to b{}, which takes {}",
            b.0,
            args.len(),
            target.0,
            params.len()
          ));
        }
        for (a, p) in args.iter().zip(params) {
          if self.ty(*a) != self.ty(*p) {
            return Err(format!(
              "b{} passes v{} ({:?}) to b{}'s v{} ({:?})",
              b.0,
              a.0,
              self.ty(*a),
              target.0,
              p.0,
              self.ty(*p)
            ));
          }
        }
      }
    }
    Ok(())
  }
}

/// Immediate dominator of every reachable block, indexed by block id.
/// Every root is dominated only by a virtual block above them all, whose
/// id is one past the last real block and which is its own dominator.
/// Cooper, Harvey and Kennedy's iterative scheme, which settles in a pass
/// or two on the reducible graphs bytecode produces.
pub fn dominators(f: &Func, order: &[BlockId]) -> Vec<Option<BlockId>> {
  let virtual_root = BlockId(f.blocks.len() as u32);
  let mut rpo_index = vec![usize::MAX; f.blocks.len() + 1];
  rpo_index[virtual_root.0 as usize] = 0;
  for (i, b) in order.iter().enumerate() {
    rpo_index[b.0 as usize] = i + 1;
  }
  let mut preds = f.predecessors();
  preds.push(Vec::new());
  for root in f.roots() {
    preds[root.0 as usize].push(virtual_root);
  }
  let mut idom: Vec<Option<BlockId>> = vec![None; f.blocks.len() + 1];
  idom[virtual_root.0 as usize] = Some(virtual_root);

  let intersect = |idom: &Vec<Option<BlockId>>, mut a: BlockId, mut b: BlockId| -> BlockId {
    while a != b {
      while rpo_index[a.0 as usize] > rpo_index[b.0 as usize] {
        a = idom[a.0 as usize].unwrap();
      }
      while rpo_index[b.0 as usize] > rpo_index[a.0 as usize] {
        b = idom[b.0 as usize].unwrap();
      }
    }
    a
  };

  let mut changed = true;
  while changed {
    changed = false;
    for &b in order.iter() {
      let mut new_idom: Option<BlockId> = None;
      for &p in &preds[b.0 as usize] {
        if rpo_index[p.0 as usize] == usize::MAX || idom[p.0 as usize].is_none() {
          continue;
        }
        new_idom = Some(match new_idom {
          None => p,
          Some(cur) => intersect(&idom, p, cur),
        });
      }
      if new_idom.is_some() && idom[b.0 as usize] != new_idom {
        idom[b.0 as usize] = new_idom;
        changed = true;
      }
    }
  }
  idom
}

/// Whether `a` dominates `b`, given `dominators`' result.
pub fn dominates(idom: &[Option<BlockId>], a: BlockId, mut b: BlockId) -> bool {
  loop {
    if a == b {
      return true;
    }
    match idom[b.0 as usize] {
      Some(up) if up != b => b = up,
      _ => return false,
    }
  }
}

impl fmt::Display for Func {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    for b in self.reverse_postorder() {
      let block = self.block(b);
      write!(f, "b{}(", b.0)?;
      for (i, p) in block.params.iter().enumerate() {
        if i > 0 {
          write!(f, ", ")?;
        }
        write!(f, "v{}: {:?}", p.0, self.ty(*p))?;
      }
      write!(f, ")")?;
      if let Some(ip) = block.ip {
        write!(f, "  ; ip {ip}")?;
      }
      if let Some(&(id, _)) = self.osr_entries.iter().find(|&&(_, e)| e == b) {
        write!(f, "  ; osr entry {id}")?;
      }
      writeln!(f)?;
      for &i in &block.insts {
        let inst = self.inst(i);
        write!(f, "  ")?;
        if let Some(r) = inst.result {
          write!(f, "v{}: {:?} = ", r.0, self.ty(r))?;
        }
        write!(f, "{:?}", inst.op)?;
        for a in &inst.args {
          write!(f, " v{}", a.0)?;
        }
        if let Some(state) = &inst.state {
          write!(f, "  @{}", state.ip)?;
          if state.frame != 0 {
            write!(f, " in frame {}", state.frame)?;
          }
        }
        writeln!(f)?;
      }
      writeln!(f, "  {:?}", block.term)?;
    }
    Ok(())
  }
}
