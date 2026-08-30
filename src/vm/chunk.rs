use core::fmt;
use std::cell::{Cell, OnceCell, RefCell};

use rustc_hash::FxHashMap;

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
  /// Jump by `offset` if register `cond` is truthy; the complement of
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
  /// deliberately NOT inherited here: see `ObjClass`'s doc comment.
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
  ///; both are just Values in the same table). Only ever emitted
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
  /// follow at `obj + 2 ..`. This applies uniformly to every dispatch
  /// path reached from here; a real class method (`invoke_prebound`),
  /// a field holding a callable (`dispatch_call(base, obj+1, ...)`), AND
  /// a builtin native (`builtins::lookup`); so any new fallback added
  /// here must read its arguments starting at `obj + 2`, never `obj + 1`.
  Invoke {
    dst: u8,
    obj: u8,
    method_const: u16,
    num_args: u8,
  },
  /// Like `Invoke`, but looks the method up on the STATIC class in
  /// register `superclass` directly; no dynamic dispatch; while
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
  /// `parent(args)`; calls the SUPERCLASS's own resolved constructor
  /// (`ObjClass::constructor`) directly on the CURRENT `self`, the
  /// constructor equivalent of `InvokeSuper`'s method calls. Unlike an
  /// ordinary `SomeClass(args)` call, this never allocates a new
  /// instance. `superclass` is a register holding the class Value; `self`
  /// is duplicated by the compiler into `superclass + 1` (same
  /// convention `InvokeSuper` uses), with `num_args` more values
  /// following it. "No superclass at all" is a compile-time fact and
  /// panics, same as `parent.method()` in that position already does;
  /// a superclass that resolves but has no constructor is the one
  /// genuinely dynamic case, and raises a catchable `AccessError`.
  CallSuperCtor {
    dst: u8,
    superclass: u8,
    num_args: u8,
  },

  /// `import PATH`; loads (or reuses the cached) module for
  /// `path_const`, resolved relative to `importer_const` (the CURRENT
  /// file's own path, a compile-time constant) when `path_const` starts
  /// with `.`/`..`, or via the standard search path otherwise. Leaves
  /// the raw `Obj::Module` in `dst`; never binds any name by itself --
  /// the compiler follows this with `GetField` (selective form),
  /// `ImportAll` (`{*}` form), or `MakePromoted` (default whole-module
  /// form) to actually bind names. Only executes a module's top-level
  /// code the FIRST time it's imported (see `vm::modules`).
  Import {
    dst: u8,
    path_const: u16,
    importer_const: u16,
  },
  /// `import PATH { * }` (`exported: false`) or `import @PATH { * }`
  /// (`exported: true`); merges every PUBLIC name currently in
  /// `module`'s namespace into whichever globals table the CURRENTLY
  /// EXECUTING function's own top-level bindings belong to (the
  /// running module's namespace, or the VM's root table for the main
  /// script/REPL: see `ObjFunction::globals_module`). No local/
  /// module-binding variable is created, matching the documented
  /// behavior. Each merged entry lands public (externally visible via
  /// `GetField`/`Invoke`) when `exported` is `true`, or local-only
  /// (usable within this module's own code, invisible from outside)
  /// when it's `false`, per spec: imports are local by default, and
  /// `@` is what actually re-exports them, wildcards included.
  ImportAll {
    module: u8,
    exported: bool,
  },
  /// `import PATH [as NAME]` (default, non-selective, non-`{*}` form)
  ///; wraps `module` together with whichever of its members is named
  /// `name_const` into a callable `ObjModuleBinding`. `name_const` is
  /// the LOCAL binding name (NAME, or the last import path segment),
  /// enabling both `NAME.other_member` access and, if that name matches
  /// a callable module member, direct `NAME(...)` "function promotion".
  MakePromoted {
    dst: u8,
    module: u8,
    name_const: u16,
  },

  /// `obj[idx]`; supported for List, Bytes (yields a number 0-255),
  /// String (yields a 1-character string, indexed by Unicode scalar
  /// value, not byte offset), and Dict (`idx` used as a key via
  /// Value::equals, not coerced to a number).
  GetIndex {
    dst: u8,
    obj: u8,
    idx: u8,
  },
  /// `obj[idx] = src`; List and Bytes overwrite an existing element
  /// in place (index must already be in bounds); Dict inserts or
  /// updates a key. Strings are immutable and always error here.
  SetIndex {
    obj: u8,
    idx: u8,
    src: u8,
  },
  /// `obj[lo, hi]`; supported for List, Bytes, and String only (not
  /// Dict). Both bounds are INCLUSIVE. A Nil value in `lo` (register
  /// content, not a compile-time fact) defaults to 0; a Nil in `hi`
  /// defaults to the last valid index: see Instr::GetSlice's own
  /// handler in vm.rs for the full resolution rules.
  GetSlice {
    dst: u8,
    obj: u8,
    lo: u8,
    hi: u8,
  },

  /// `lower..upper`; valid in either direction (`upper` may be less
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

  /// `raise EXPR`; EXPR must evaluate to an Error (or subclass)
  /// instance; the VM validates this and overwrites its `stacktrace`
  /// field, then propagates it as a catchable error. Also what
  /// `Compiler::compile_assert` desugars into on a failed assertion.
  Raise {
    src: u8,
  },
  /// `catch { body } as var { error_block }`. Pushed BEFORE `body`
  /// compiles, popped by `Instr::PopCatch` on normal completion.
  /// `var_reg` (if the `as` clause was present) is a register in the
  /// SAME frame this instruction executes in; pre-loaded with Nil --
  /// that either stays Nil (no error) or gets overwritten with the
  /// caught error by the VM's unwind logic. `offset` is a relative
  /// jump-style offset (same encoding/patching as Jmp) to where
  /// execution resumes on EITHER path; normal fallthrough past
  /// PopCatch, or an error jumping there directly.
  PushCatch {
    var_reg: Option<u8>,
    offset: i16,
  },
  /// Marks normal (non-exceptional) completion of a catch body --
  /// pops the handler `PushCatch` registered. Never reached if an
  /// error unwound past this point instead.
  PopCatch,

  /* Folded Instructions */
  AddImm {
    dst: u8,
    a: u8,
    imm_const: u16,
  },
  SubImm {
    dst: u8,
    a: u8,
    imm_const: u16,
  },
  MulImm {
    dst: u8,
    a: u8,
    imm_const: u16,
  },
  LtImm {
    dst: u8,
    a: u8,
    imm_const: u16,
  },
  LeImm {
    dst: u8,
    a: u8,
    imm_const: u16,
  },
  GtImm {
    dst: u8,
    a: u8,
    imm_const: u16,
  },
  GeImm {
    dst: u8,
    a: u8,
    imm_const: u16,
  },
  EqImm {
    dst: u8,
    a: u8,
    imm_const: u16,
  },
  NeqImm {
    dst: u8,
    a: u8,
    imm_const: u16,
  },

  /// Enforces a declared parameter type annotation; `def f(x: number)`
  /// compiles this as the very first thing the function body does for
  /// `x`, one instruction per typed parameter, in parameter order.
  /// `check_idx` indexes `Chunk::param_checks` for the actual type list/
  /// nullability/name (a `u16` side-table index, same pattern as
  /// `LoadConst`'s `const_idx`, rather than baking that data into the
  /// instruction itself; a union type plus a parameter name string
  /// doesn't fit in fixed-width fields). Raises `TypeError` on mismatch
  /// (see `VM`'s own handler); never a no-op unless the check passes,
  /// but reading `reg` itself has no other effect on it. An UNTYPED
  /// parameter (or one whose only declared type is `any`) gets no
  /// instruction at all: see `Compiler::emit_param_type_checks`.
  CheckParamType {
    reg: u8,
    check_idx: u16,
  },
}

/// A compile-time-constant `using` case label's value, in a form that's
/// both `Hash` and `Eq`; `Value` itself can't be (NaN-boxed f64 bit
/// patterns don't have well-behaved hashing/equality for arbitrary
/// floats), so this is a small, deliberately narrow parallel
/// representation built only from the handful of AST literal shapes
/// `Compiler::compile_using` treats as constant (see `expr_as_jump_key`
/// in compiler.rs) and the matching runtime Values that can appear as a
/// `using` subject (see `value_to_jump_key` in vm.rs). A label/subject
/// that isn't one of these; a list, dict, instance, negative-number
/// literal, BigNumber, etc.; always falls back to the sequential
/// dynamic-label path, never into this table.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum JumpKey {
  Nil,
  Bool(bool),
  /// The label/subject's exact `f64::to_bits()` pattern; an exact
  /// bit-for-bit match, not IEEE `==`, so e.g. `-0.0` and `0.0` (equal
  /// under `==`, different bit patterns) could in principle land in
  /// different entries. Not a concern for any label written as an
  /// ordinary integer or float literal, which is the only way a key
  /// enters this table to begin with.
  Number(u64),
  Str(String),
}

/// One `Instr::GetField`/`Instr::SetField` site's monomorphic inline
/// cache entry: see `Chunk::field_cache`.
///
/// `#[repr(C)]` because `jit::codegen` reads both fields by baked
/// compile-time offset from a baked cell address, so declaration order
/// here is part of the generated code's contract, not an implementation
/// detail.
#[derive(Clone, Debug, Default)]
#[repr(C)]
pub struct FieldCacheCell {
  /// The last-seen receiver's class, as its `Value` bits. `0` means
  /// "empty, never filled": no real `Value` is ever all-zero bits (a
  /// heap value always carries `Value`'s own quiet-NaN + sign tagging),
  /// so an empty cell can never accidentally match a live class.
  pub class_bits: Cell<u64>,
  /// That class's BYTE offset for this site's field name --
  /// `slot * size_of::<Value>()`, pre-multiplied so generated code adds
  /// it straight to the instance's fields base pointer with no shift.
  pub byte_offset: Cell<u64>,
}

/// One `Instr::Invoke` site's monomorphic cache, covering both shapes
/// that site can take.
///
/// `#[repr(C)]` for the same reason `FieldCacheCell` is: the address of
/// an individual cell is baked into generated code and its fields are
/// read by fixed offset.
///
/// `key` is `0` when the cell has never been filled, and otherwise
/// either a receiver class's `Value` bits (an instance receiver, with
/// `payload` the resolved method's own bits) or
/// `builtins::method_table_key`'s small non-zero kind id (a primitive
/// receiver, with `payload` the address of a `&'static NativeFunction`).
/// The two can't be confused: a class's bits are a NaN-boxed pointer,
/// nowhere near the handful of small integers a kind id uses.
#[derive(Clone, Debug, Default)]
#[repr(C)]
pub struct InvokeCacheCell {
  pub key: Cell<u64>,
  pub payload: Cell<u64>,
}

/// One shape an `Instr::CheckParamType` may demand; deliberately a
/// closed, exhaustive match on `Value`/`Obj`'s own tag space (see
/// `Value::is_number`/`is_obj`/`Obj`'s discriminant), never a call
/// through the `is_*`/`instance_of` GLOBAL FUNCTIONS a Zuri program can
/// see and call itself: those are ordinary closures/natives from the
/// bytecode's point of view, and routing a parameter check through one
/// would force both the interpreter and (worse) the JIT to treat every
/// single typed parameter as a real dynamic call. Checking the `Value`
/// representation directly, the same way `Instr::GetIndex`'s fast path
/// or any other guard already does, is what lets `jit::codegen` compile
/// this to a few inline tag tests instead.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParamType {
  Bool,
  Int,
  Number,
  BigInt,
  String,
  Bytes,
  List,
  Dict,
  Range,
  File,
  /// A closure, bound method, or native; NOT a class (that's
  /// `Class`). Matches `Value::is_callable()` minus the class case.
  Function,
  /// `type` in source; a class value itself, e.g. passing `Vec3` as
  /// an argument rather than a `Vec3` instance.
  Class,
  /// Anything `Instr::Call`/`Instr::Invoke` can invoke, INCLUDING a
  /// class (calling one constructs an instance); `Value::is_callable()`
  /// exactly.
  Callable,
  /// A list, dict, string, bytes, or an instance whose class declares
  /// both `@value` and `@key`.
  Iterable,
  /// A specific user class, by name. `u16` indexes `Chunk::constants`
  /// for the class's name (a string); resolved through the SAME
  /// `Chunk::global_cache` a plain `Instr::GetGlobal` uses (keyed by
  /// the owning `Instr::CheckParamType`'s own bytecode position), so a
  /// hot function's repeated calls pay the name lookup once, not once
  /// per call. Matches the named class OR any of its subclasses (walks
  /// `ObjClass::superclass`), same as the reference C runtime's
  /// `instance_of`.
  Instance(u16),
}

/// One `Instr::CheckParamType` site's full declared constraint;
/// everything `parse_type`/`Expr::TypeHint` captured for this parameter,
/// carried through to bytecode. `Chunk::param_checks[check_idx]`.
#[derive(Clone, Debug)]
pub struct ParamTypeCheck {
  /// For the raised `TypeError`'s message only.
  pub param_name: String,
  /// 1-based position among ALL of the function's parameters (typed or
  /// not); also message-only.
  pub position: u32,
  /// `?` before the type list; a bare `nil` argument always passes,
  /// regardless of `types`.
  pub nullable: bool,
  /// One or more (a `|`-separated union in source); the argument must
  /// match AT LEAST ONE. Never empty, and never contains a param
  /// annotated `any`/left untyped; `Compiler::emit_param_type_checks`
  /// skips emitting any instruction at all for those, since there is
  /// nothing to check.
  pub types: Vec<ParamType>,
}

impl ParamType {
  /// Human-readable description for a `TypeError` message; mirrors
  /// `builtins::enforce::ArgType::label`'s phrasing (`"a number"`, `"a
  /// list"`, ...) for consistency with every other argument-type error
  /// this runtime raises. `Instance`'s label is the only one that isn't
  /// `'static` (it names whatever class the source actually wrote), so
  /// this returns an owned `String` throughout rather than mixing
  /// return types.
  pub fn label(self, chunk: &Chunk) -> String {
    match self {
      ParamType::Bool => "a bool".to_string(),
      ParamType::Int => "an int".to_string(),
      ParamType::Number => "a number".to_string(),
      ParamType::BigInt => "a bigint".to_string(),
      ParamType::String => "a string".to_string(),
      ParamType::Bytes => "bytes".to_string(),
      ParamType::List => "a list".to_string(),
      ParamType::Dict => "a dict".to_string(),
      ParamType::Range => "a range".to_string(),
      ParamType::File => "a file".to_string(),
      ParamType::Function => "a function".to_string(),
      ParamType::Class => "a type".to_string(),
      ParamType::Callable => "a callable".to_string(),
      ParamType::Iterable => "an iterable".to_string(),
      ParamType::Instance(name_const) => {
        format!("a {}", chunk.constants[name_const as usize].as_str())
      },
    }
  }
}

/// Joins a `ParamTypeCheck`'s `types` into one readable phrase, same
/// shape as `builtins::enforce::describe_types`; `"a number"`, `"a
/// number or a string"`, `"a number, a string, or an Error"`.
pub fn describe_param_types(types: &[ParamType], chunk: &Chunk) -> String {
  let labels: Vec<String> = types.iter().map(|t| t.label(chunk)).collect();
  match labels.as_slice() {
    [] => "a value".to_string(),
    [one] => one.clone(),
    [a, b] => format!("{a} or {b}"),
    _ => {
      let (last, rest) = labels.split_last().unwrap();
      format!("{}, or {}", rest.join(", "), last)
    },
  }
}

#[derive(Default, Clone, Debug)]
pub struct Chunk {
  pub code: Vec<Instr>,
  pub constants: Vec<Value>,
  /// One entry per `using` statement that has at least one constant
  /// case label: see `Instr::UsingJump`.
  pub jump_tables: Vec<FxHashMap<JumpKey, usize>>,
  /// Parallel to `code`; `lines[i]` is the source line `code[i]` was
  /// compiled from (statement granularity: see
  /// `Compiler::emit`/`FunctionScope::current_line`). Used only to
  /// build a stack trace on a raised or uncaught error: see
  /// `VM::build_stacktrace`.
  pub lines: Vec<u32>,
  /// Inline cache for global variable access: maps a GetGlobal/
  /// SetGlobal/AssignGlobal instruction's own position in `code` to
  /// the global slot it resolved to the FIRST time it executed. Every
  /// later execution of that same instruction skips the name lookup
  /// (a string hash + FxHashMap probe) entirely and indexes straight
  /// into VM::global_slots. Never invalidated; once a name resolves
  /// to a slot it keeps that slot for the life of the VM (globals are
  /// never renamed or removed, only reassigned in place).
  pub global_cache: RefCell<FxHashMap<usize, (bool, u32)>>,
  /// Monomorphic inline cache for `Instr::GetField`/`Instr::SetField`
  /// on an INSTANCE receiver (never consulted by the interpreter, which
  /// has no analogous per-instruction cache of its own); one cell per
  /// instruction position, so both the `jit::runtime` helpers and
  /// JIT-GENERATED CODE ITSELF can reach a site's entry by fixed
  /// address, with no hash and no `RefCell` borrow. See
  /// `FieldCacheCell` for the cell itself and
  /// `jit::codegen::FuncCompiler::emit_ic_get_field` for the generated
  /// fast path that reads it directly.
  ///
  /// Allocated once, lazily, at exactly `code.len()` cells (see
  /// `field_cache_cell`) and never resized; generated code bakes the
  /// address of an individual cell as an immediate, so the backing
  /// allocation must outlive the compiled code and never move.
  ///
  /// "Same class bits as last time" is a genuine proof of "same class"
  /// here, not just a strong hint: `Heap::alloc_class` allocates every
  /// `ObjClass` directly into the non-moving old generation, so a
  /// class's address is fixed for its whole life and can never be
  /// recycled underneath a cached entry the way a once-nursery address
  /// can (contrast `invoke_cache`, whose own docs spell out the
  /// relocation hazard that applies to it).
  field_cache: OnceCell<Box<[FieldCacheCell]>>,
  /// Same idea as `field_cache`, for `Instr::Invoke`: see
  /// `InvokeCacheCell`. Was a `RefCell<FxHashMap<usize, _>>`, which cost
  /// a borrow-flag check and a hash probe on every dynamic method call
  /// before it could answer a question a single load answers now.
  ///
  /// Unlike `field_cache`, a false HIT on the class-method half hands
  /// back `payload` for the WRONG class, dereferenced as a closure; a
  /// genuine memory-safety risk, not just a data bug. In practice this
  /// needs two coincidences at once: a nursery address getting reused by
  /// a DIFFERENT class object specifically (not just any object), AND
  /// the exact same call site being hit again with THAT class as the new
  /// receiver before its cache entry is ever overwritten by an
  /// intervening real miss. Not yet observed in this project's own
  /// extensive stress testing, and not fixed here; the honest fix is
  /// invalidating every live chunk's cache on each collection (no
  /// registry of "every live chunk" exists to do that cheaply today) or
  /// switching the cached class key to something collision-proof against
  /// relocation; flagging clearly rather than leaving silent. The
  /// primitive half has no such hazard: a `NativeFunction` is `'static`.
  invoke_cache: OnceCell<Box<[InvokeCacheCell]>>,
  /// One entry per `Instr::CheckParamType` this chunk emits, in the
  /// order they're emitted (parameter order); `check_idx` indexes
  /// straight into this, same relationship `const_idx` has to
  /// `constants`.
  pub param_checks: Vec<ParamTypeCheck>,
}

impl Chunk {
  /// This instruction position's own inline-cache cell, allocating the
  /// whole per-chunk array on first use. Returns `None` for an `ip`
  /// past the array's length, which can only happen if `code` grew
  /// AFTER the array was sized (the REPL appends to a live chunk);
  /// callers just fall back to the uncached lookup, which is always
  /// correct, only slower.
  pub fn field_cache_cell(&self, ip: usize) -> Option<&FieldCacheCell> {
    self
      .field_cache
      .get_or_init(|| {
        (0..self.code.len())
          .map(|_| FieldCacheCell::default())
          .collect()
      })
      .get(ip)
  }

  /// This instruction position's own `Instr::Invoke` cache cell; the
  /// `field_cache_cell` of `invoke_cache`, with the same lazy allocation
  /// and the same `None` for an `ip` past the array.
  pub fn invoke_cache_cell(&self, ip: usize) -> Option<&InvokeCacheCell> {
    self
      .invoke_cache
      .get_or_init(|| {
        (0..self.code.len())
          .map(|_| InvokeCacheCell::default())
          .collect()
      })
      .get(ip)
  }

  pub fn new() -> Chunk {
    Chunk {
      code: Vec::new(),
      constants: Vec::new(),
      jump_tables: Vec::new(),
      lines: Vec::new(),
      global_cache: RefCell::new(FxHashMap::default()),
      field_cache: OnceCell::new(),
      invoke_cache: OnceCell::new(),
      param_checks: Vec::new(),
    }
  }

  pub fn add_constant(&mut self, v: Value) -> u16 {
    self.constants.push(v);
    (self.constants.len() - 1) as u16
  }

  pub fn add_param_check(&mut self, check: ParamTypeCheck) -> u16 {
    self.param_checks.push(check);
    (self.param_checks.len() - 1) as u16
  }

  pub fn add_jump_table(&mut self) -> u16 {
    self.jump_tables.push(FxHashMap::default());
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

/// Human-readable name for an instruction's own variant, ignoring its
/// operands; used only by the ZURI_OPCODE_PROFILE instrumentation in
/// vm.rs to identify which opcodes (and which adjacent PAIRS of
/// opcodes) actually dominate real execution, so instruction fusion
/// can target what's really hot instead of a guess.
pub fn instr_name(instr: &Instr) -> &'static str {
  match instr {
    Instr::LoadConst { .. } => "LoadConst",
    Instr::LoadNil { .. } => "LoadNil",
    Instr::LoadBool { .. } => "LoadBool",
    Instr::Move { .. } => "Move",
    Instr::Add { .. } => "Add",
    Instr::Sub { .. } => "Sub",
    Instr::Mul { .. } => "Mul",
    Instr::Div { .. } => "Div",
    Instr::Pow { .. } => "Pow",
    Instr::Floor { .. } => "Floor",
    Instr::Mod { .. } => "Mod",
    Instr::Neg { .. } => "Neg",
    Instr::Not { .. } => "Not",
    Instr::Concat { .. } => "Concat",
    Instr::BitAnd { .. } => "BitAnd",
    Instr::BitOr { .. } => "BitOr",
    Instr::BitXor { .. } => "BitXor",
    Instr::BitShl { .. } => "BitShl",
    Instr::BitShr { .. } => "BitShr",
    Instr::BitUshr { .. } => "BitUshr",
    Instr::BitNot { .. } => "BitNot",
    Instr::Eq { .. } => "Eq",
    Instr::Neq { .. } => "Neq",
    Instr::Lt { .. } => "Lt",
    Instr::Le { .. } => "Le",
    Instr::Gt { .. } => "Gt",
    Instr::Ge { .. } => "Ge",
    Instr::Jmp { .. } => "Jmp",
    Instr::JmpIfFalse { .. } => "JmpIfFalse",
    Instr::JmpIfTrue { .. } => "JmpIfTrue",
    Instr::Call { .. } => "Call",
    Instr::Return { .. } => "Return",
    Instr::Print { .. } => "Print",
    Instr::GetGlobal { .. } => "GetGlobal",
    Instr::SetGlobal { .. } => "SetGlobal",
    Instr::AssignGlobal { .. } => "AssignGlobal",
    Instr::Closure { .. } => "Closure",
    Instr::GetUpval { .. } => "GetUpval",
    Instr::SetUpval { .. } => "SetUpval",
    Instr::CloseUpvalues { .. } => "CloseUpvalues",
    Instr::MakeList { .. } => "MakeList",
    Instr::MakeDict { .. } => "MakeDict",
    Instr::MakeClass { .. } => "MakeClass",
    Instr::DeclareField { .. } => "DeclareField",
    Instr::SetFieldInit { .. } => "SetFieldInit",
    Instr::SetMethod { .. } => "SetMethod",
    Instr::DeclareStatic { .. } => "DeclareStatic",
    Instr::FinalizeClass { .. } => "FinalizeClass",
    Instr::GetField { .. } => "GetField",
    Instr::SetField { .. } => "SetField",
    Instr::Invoke { .. } => "Invoke",
    Instr::InvokeSuper { .. } => "InvokeSuper",
    Instr::CallSuperCtor { .. } => "CallSuperCtor",
    Instr::Import { .. } => "Import",
    Instr::ImportAll { .. } => "ImportAll",
    Instr::MakePromoted { .. } => "MakePromoted",
    Instr::GetIndex { .. } => "GetIndex",
    Instr::SetIndex { .. } => "SetIndex",
    Instr::GetSlice { .. } => "GetSlice",
    Instr::MakeRange { .. } => "MakeRange",
    Instr::UsingJump { .. } => "UsingJump",
    Instr::Raise { .. } => "Raise",
    Instr::PushCatch { .. } => "PushCatch",
    Instr::PopCatch => "PopCatch",
    Instr::AddImm { .. } => "AddImm",
    Instr::SubImm { .. } => "SubImm",
    Instr::MulImm { .. } => "MulImm",
    Instr::LtImm { .. } => "LtImm",
    Instr::LeImm { .. } => "LeImm",
    Instr::GtImm { .. } => "GtImm",
    Instr::GeImm { .. } => "GeImm",
    Instr::EqImm { .. } => "EqImm",
    Instr::NeqImm { .. } => "NeqImm",
    Instr::CheckParamType { .. } => "CheckParamType",
  }
}
