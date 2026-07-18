use core::fmt;
use std::collections::HashMap;

use crate::vm::value::Value;

/// A register-based instruction, modeled loosely on Lua's VM: most ops name
/// a destination register plus one or two source registers, rather than
/// pushing/popping an operand stack.
#[derive(Clone, Copy, Debug)]
#[allow(dead_code)]
pub enum Instr {
  LoadConst {
    dst: u8,
    const_idx: u16,
  },
  LoadNil {
    dst: u8,
  },
  LoadBool {
    dst: u8,
    val: bool,
  },
  Move {
    dst: u8,
    src: u8,
  },

  Add {
    dst: u8,
    a: u8,
    b: u8,
  },
  Sub {
    dst: u8,
    a: u8,
    b: u8,
  },
  Mul {
    dst: u8,
    a: u8,
    b: u8,
  },
  Div {
    dst: u8,
    a: u8,
    b: u8,
  },
  Pow {
    dst: u8,
    a: u8,
    b: u8,
  },
  Floor {
    dst: u8,
    a: u8,
    b: u8,
  },
  Mod {
    dst: u8,
    a: u8,
    b: u8,
  },
  Neg {
    dst: u8,
    src: u8,
  },
  Not {
    dst: u8,
    src: u8,
  },
  Concat {
    dst: u8,
    a: u8,
    b: u8,
  },

  BitAnd {
    dst: u8,
    a: u8,
    b: u8,
  },
  BitOr {
    dst: u8,
    a: u8,
    b: u8,
  },
  BitXor {
    dst: u8,
    a: u8,
    b: u8,
  },
  BitShl {
    dst: u8,
    a: u8,
    b: u8,
  },
  BitShr {
    dst: u8,
    a: u8,
    b: u8,
  },
  BitUshr {
    dst: u8,
    a: u8,
    b: u8,
  },
  BitNot {
    dst: u8,
    src: u8,
  },

  Eq {
    dst: u8,
    a: u8,
    b: u8,
  },
  Neq {
    dst: u8,
    a: u8,
    b: u8,
  },
  Lt {
    dst: u8,
    a: u8,
    b: u8,
  },
  Le {
    dst: u8,
    a: u8,
    b: u8,
  },
  Gt {
    dst: u8,
    a: u8,
    b: u8,
  },
  Ge {
    dst: u8,
    a: u8,
    b: u8,
  },

  /// Unconditional relative jump.
  Jmp {
    offset: i16,
  },
  /// Jump by `offset` if register `cond` is falsey.
  JmpIfFalse {
    cond: u8,
    offset: i16,
  },
  /// Jump by `offset` if register `cond` is truthy -- the complement of
  /// JmpIfFalse, used for short-circuiting `or`.
  JmpIfTrue {
    cond: u8,
    offset: i16,
  },

  /// Call the function in register `func`. Arguments are expected to
  /// already sit in registers `func+1 ..= func+num_args`, matching Zuri's
  /// convention of laying out args right after the callee. The return
  /// value is written into `dst` in the *caller's* window.
  Call {
    dst: u8,
    func: u8,
    num_args: u8,
  },
  Return {
    src: u8,
  },

  Print {
    src: u8,
  },

  /// Globals are looked up by name (a string constant) rather than by a
  /// fixed slot, so that recursive functions can find themselves without
  /// needing a pointer to their own not-yet-allocated ObjFunction baked
  /// into their own constant pool.
  GetGlobal {
    dst: u8,
    name_const: u16,
  },
  SetGlobal {
    name_const: u16,
    src: u8,
  },
  AssignGlobal {
    name_const: u16,
    src: u8,
  },

  /// Create a new closure from the function prototype stored at
  /// `constants[proto_const]`, capturing upvalues per that prototype's
  /// own (compile-time, static) descriptor list, sourced from the
  /// CURRENTLY EXECUTING frame's registers/closure.
  Closure {
    dst: u8,
    proto_const: u16,
  },
  GetUpval {
    dst: u8,
    idx: u8,
  },
  SetUpval {
    idx: u8,
    src: u8,
  },
  /// Close every open upvalue pointing at a register >= `from` (relative
  /// to the current frame), copying its current value out of the
  /// register into the upvalue's own storage. Emitted at block exit so a
  /// register reused for something else (e.g. the next loop iteration's
  /// local) doesn't silently corrupt a closure that captured it.
  CloseUpvalues {
    from: u8,
  },

  /// Collect `count` consecutive registers starting at `start` into a
  /// new List, placed in `dst`.
  MakeList {
    dst: u8,
    start: u8,
    count: u8,
  },
  /// Collect a dict from TWO back-to-back runs of `count` registers
  /// each, starting at `start`: keys occupy `start..start+count`,
  /// values occupy `start+count..start+2*count`. Placed in `dst`.
  MakeDict {
    dst: u8,
    start: u8,
    count: u8,
  },

  /// Build a new class shell: clones the superclass's already-merged
  /// method table, field-slot layout, and constructor (inheriting them
  /// wholesale), or starts empty if `superclass` is None. Statics are
  /// deliberately NOT inherited here -- see `ObjClass`'s doc comment.
  MakeClass {
    dst: u8,
    name_const: u16,
    superclass: Option<u8>,
  },
  /// Register one of this class's OWN instance fields, extending its
  /// (already superclass-seeded) field_slots/field_count. Only ever
  /// emitted while a class is being declared.
  DeclareField {
    class: u8,
    name_const: u16,
  },
  SetFieldInit {
    class: u8,
    src: u8,
  },
  SetMethod {
    class: u8,
    name_const: u16,
    src: u8,
  },
  /// Register one of this class's OWN static members (field or method
  /// -- both are just Values in the same table). Only ever emitted
  /// while a class is being declared.
  DeclareStatic {
    class: u8,
    name_const: u16,
    src: u8,
  },
  /// Resolve this class's constructor: if `methods` contains an entry
  /// keyed by the class's own name (declared just above, in this same
  /// declaration), that becomes the constructor, overriding whatever
  /// was inherited by `MakeClass`. Emitted once, after every method has
  /// been added.
  FinalizeClass {
    class: u8,
    name_const: u16,
  },
  /// Read a named field (instance) or static member (class) from `obj`.
  GetField {
    dst: u8,
    obj: u8,
    name_const: u16,
  },
  SetField {
    obj: u8,
    name_const: u16,
    src: u8,
  },
  /// Fused "look up method by name on `obj`'s class, then call it" --
  /// dynamic dispatch through the receiver's ACTUAL runtime class. The
  /// compiler always duplicates the receiver into register `obj + 1`
  /// before emitting this (see `Compiler::compile_invoke`), which is
  /// where the callee's own register 0 (self) ends up; user arguments
  /// follow at `obj + 2 ..`.
  Invoke {
    dst: u8,
    obj: u8,
    method_const: u16,
    num_args: u8,
  },
  /// Like `Invoke`, but looks the method up on the STATIC class in
  /// register `superclass` directly -- no dynamic dispatch -- while
  /// still binding the current `self` (duplicated by the compiler into
  /// `superclass + 1`, same convention as `Invoke`). This is what makes
  /// `parent.foo()` call the lexically-fixed ancestor's method even
  /// when `self`'s actual runtime class is several levels further down.
  InvokeSuper {
    dst: u8,
    superclass: u8,
    method_const: u16,
    num_args: u8,
  },

  /// `obj[idx]` -- supported for List, Bytes (yields a number 0-255),
  /// String (yields a 1-character string, indexed by Unicode scalar
  /// value, not byte offset), and Dict (`idx` used as a key via
  /// Value::equals, not coerced to a number).
  GetIndex {
    dst: u8,
    obj: u8,
    idx: u8,
  },
  /// `obj[idx] = src` -- List and Bytes overwrite an existing element
  /// in place (index must already be in bounds); Dict inserts or
  /// updates a key. Strings are immutable and always error here.
  SetIndex {
    obj: u8,
    idx: u8,
    src: u8,
  },
  /// `obj[lo, hi]` -- supported for List, Bytes, and String only (not
  /// Dict). Both bounds are INCLUSIVE. A Nil value in `lo` (register
  /// content, not a compile-time fact) defaults to 0; a Nil in `hi`
  /// defaults to the last valid index -- see Instr::GetSlice's own
  /// handler in vm.rs for the full resolution rules.
  GetSlice {
    dst: u8,
    obj: u8,
    lo: u8,
    hi: u8,
  },

  /// `lower..upper` -- valid in either direction (`upper` may be less
  /// than, equal to, or greater than `lower`); both bounds must resolve
  /// to numbers at runtime (checked in Instr::MakeRange's own handler).
  MakeRange {
    dst: u8,
    lower: u8,
    upper: u8,
  },

  /// Dispatch table for a `using` statement's CONSTANT case labels --
  /// built once at compile time (see Compiler::compile_using) as
  /// `chunk.jump_tables[table_idx]`. A hit sets `ip` directly to that
  /// case body's absolute instruction index in O(1), regardless of how
  /// many `when` arms the statement has. A miss (subject's value isn't
  /// hashable, or it just doesn't match any constant label) falls
  /// through to the next instruction, where any NON-constant (dynamic)
  /// labels are checked sequentially instead.
  UsingJump {
    subject: u8,
    table_idx: u16,
  },

  /// `raise EXPR` -- EXPR must evaluate to an Exception (or subclass)
  /// instance; the VM validates this and overwrites its `stacktrace`
  /// field, then propagates it as a catchable error. Also what
  /// `Compiler::compile_assert` desugars into on a failed assertion.
  Raise {
    src: u8,
  },
  /// `catch { body } as var { error_block }`. Pushed BEFORE `body`
  /// compiles, popped by `Instr::PopCatch` on normal completion.
  /// `var_reg` (if the `as` clause was present) is a register in the
  /// SAME frame this instruction executes in -- pre-loaded with Nil --
  /// that either stays Nil (no exception) or gets overwritten with the
  /// caught exception by the VM's unwind logic. `offset` is a relative
  /// jump-style offset (same encoding/patching as Jmp) to where
  /// execution resumes on EITHER path -- normal fallthrough past
  /// PopCatch, or an exception jumping there directly.
  PushCatch {
    var_reg: Option<u8>,
    offset: i16,
  },
  /// Marks normal (non-exceptional) completion of a catch body --
  /// pops the handler `PushCatch` registered. Never reached if an
  /// exception unwound past this point instead.
  PopCatch,
}

/// A compile-time-constant `using` case label's value, in a form that's
/// both `Hash` and `Eq` -- `Value` itself can't be (NaN-boxed f64 bit
/// patterns don't have well-behaved hashing/equality for arbitrary
/// floats), so this is a small, deliberately narrow parallel
/// representation built only from the handful of AST literal shapes
/// `Compiler::compile_using` treats as constant (see `expr_as_jump_key`
/// in compiler.rs) and the matching runtime Values that can appear as a
/// `using` subject (see `value_to_jump_key` in vm.rs). A label/subject
/// that isn't one of these -- a list, dict, instance, negative-number
/// literal, BigNumber, etc. -- always falls back to the sequential
/// dynamic-label path, never into this table.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum JumpKey {
  Nil,
  Bool(bool),
  /// The label/subject's exact `f64::to_bits()` pattern -- an exact
  /// bit-for-bit match, not IEEE `==`, so e.g. `-0.0` and `0.0` (equal
  /// under `==`, different bit patterns) could in principle land in
  /// different entries. Not a concern for any label written as an
  /// ordinary integer or float literal, which is the only way a key
  /// enters this table to begin with.
  Number(u64),
  Str(String),
}

#[derive(Default, Clone, Debug)]
pub struct Chunk {
  pub code: Vec<Instr>,
  pub constants: Vec<Value>,
  /// One entry per `using` statement that has at least one constant
  /// case label -- see `Instr::UsingJump`.
  pub jump_tables: Vec<HashMap<JumpKey, usize>>,
  /// Parallel to `code` -- `lines[i]` is the source line `code[i]` was
  /// compiled from (statement granularity; see
  /// `Compiler::emit`/`FunctionScope::current_line`). Used only to
  /// build a stack trace on a raised or uncaught exception -- see
  /// `VM::build_stacktrace`.
  pub lines: Vec<u32>,
}

impl Chunk {
  pub fn new() -> Chunk {
    Chunk {
      code: Vec::new(),
      constants: Vec::new(),
      jump_tables: Vec::new(),
      lines: Vec::new(),
    }
  }

  pub fn add_constant(&mut self, v: Value) -> u16 {
    self.constants.push(v);
    (self.constants.len() - 1) as u16
  }

  pub fn add_jump_table(&mut self) -> u16 {
    self.jump_tables.push(HashMap::new());
    (self.jump_tables.len() - 1) as u16
  }

  pub fn emit(&mut self, instr: Instr) -> usize {
    self.code.push(instr);
    self.code.len() - 1
  }

  pub fn patch(&mut self, index: usize, instr: Instr) {
    self.code[index] = instr;
  }

  pub fn patch_false_jump(&mut self, cond: u8, offset: usize) {
    self.code[offset] = Instr::JmpIfFalse {
      cond,
      offset: (self.code.len() - offset - 1) as i16,
    };
  }

  pub fn patch_jump(&mut self, offset: usize) {
    self.code[offset] = Instr::Jmp {
      offset: (self.code.len() - offset - 1) as i16,
    };
  }
}

impl fmt::Display for Chunk {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    write!(f, "{:?}", self)
  }
}
