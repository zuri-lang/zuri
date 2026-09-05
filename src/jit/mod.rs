//! A Cranelift-backed baseline JIT tier for the Zuri VM.
//!
//! # Architecture in one page
//!
//! The interpreter (`vm::vm::VM::run_until`) is a register machine over
//! one flat `Vec<Value>` (`VM::registers`), sliced into per-frame
//! windows. The JIT compiler maps each bytecode register to a Cranelift
//! SSA `Variable` (`reg_vars`) which Cranelift's backtracking register
//! allocator places directly into physical CPU hardware registers during
//! steady-state execution.
//!
//! When interacting across boundaries (runtime helper calls, GC
//! safepoints, on-stack replacement, or deoptimization), live registers
//! are synchronized to/from `VM::registers` via `flush_live`/`reload_live`.
//!
//! - **Mixed-mode execution** (compiled code calling an as-yet-uncompiled
//!   function): the callee's frame is pushed with the same `base`
//!   convention the interpreter already uses, so `VM::call_value` can
//!   run it through `run_until` as if the caller were also interpreted.
//!   See `vm::vm::VM::call_value`.
//! - **GC safepoints**: live registers are flushed to `VM::registers`
//!   before safepoints (`emit_safepoint`), so the existing non-moving,
//!   root-scanning collector (`VM::collect_garbage`) sees a JIT frame's
//!   registers correctly as long as that frame's `CallFrame` is on
//!   `VM::frames`. Compiled code calls back into `VM::collect_garbage`
//!   at loop back-edges and call sites.
//! - **On-stack replacement**: jumping into the middle of a compiled
//!   function needs no value reconstruction either; the OSR entry
//!   block is just another predecessor of the target loop header block
//!   that hasn't initialized any local Cranelift SSA variables (there
//!   are none; everything lives in `VM::registers` already) and jumps
//!   straight there. See `codegen::FuncCompiler::compile` and
//!   `warmup::osr_threshold`.
//! - **Errors bail to the interpreter**: rather than reimplementing
//!   `catch`/`raise` unwinding as generated machine code, a function
//!   containing `Instr::PushCatch`/`Instr::Raise` is simply never
//!   selected for compilation (see `codegen::is_eligible`); it always
//!   runs interpreted, where the existing `catch_stack` unwinder
//!   handles it. A compiled function can still raise indirectly (an
//!   arithmetic type error, a callee that itself raises, ...); when
//!   that happens control leaves compiled code entirely and propagates
//!   the error up to whatever Rust frame invoked it (interpreter's
//!   `dispatch_call`, `call_value`, or an enclosing compiled caller's
//!   own call-site helper), mirroring how an interpreted `Err(Value)`
//!   already propagates. See `runtime`'s module docs for the exact
//!   error-channel protocol.
//! - **Operator overloading**: a binary op's fast path (both operands
//!   plain numbers) is inlined directly as machine code; anything else
//!  ; strings, lists, bigints, a class's `@add` override, ... --
//!   calls straight back into the same Rust methods the interpreter
//!   itself uses (`VM::binary_add`, `VM::compare`, ...), so there's
//!   exactly one place that implements what `+`/`*`/`<`/... mean for a
//!   non-numeric operand, shared by both tiers. See `runtime`'s
//!   arithmetic helpers.
//!
//! # Module layout
//!
//! - `warmup`; the size-scaled call/OSR warm-up threshold curves.
//! - `engine`; owns the `cranelift_jit::JITModule`, registers every
//!   `runtime` helper as a linkable symbol, and drives one function's
//!   compilation from a `&ObjFunction` to a `CompiledFunction`.
//! - `codegen`; the actual bytecode -> Cranelift IR translator.
//! - `runtime`; the small, fixed set of `extern "C" fn`s compiled
//!   code calls out to for anything that isn't worth (or safe to)
//!   inline directly as machine code.

pub mod background;
pub mod codegen;
pub mod engine;
pub mod escape;
pub mod runtime;
pub mod typeflow;
pub mod warmup;

use std::sync::OnceLock;

use rustc_hash::FxHashMap;

pub use engine::JitEngine;

/// Emit a one-line `[jit] compiled '...'`/`'... ineligible: ...'`
/// message per compilation attempt; `ZURI_JIT_LOG=1`.
pub fn log_enabled() -> bool {
  static ENABLED: OnceLock<bool> = OnceLock::new();
  *ENABLED.get_or_init(|| std::env::var_os("ZURI_JIT_LOG").is_some())
}

/// Dump each compiled function's full Cranelift IR to stderr;
/// `ZURI_JIT_LOG_IR=1`. Separate from `log_enabled` since this is
/// substantially noisier (one full IR listing per compiled function).
pub fn log_ir_enabled() -> bool {
  static ENABLED: OnceLock<bool> = OnceLock::new();
  *ENABLED.get_or_init(|| std::env::var_os("ZURI_JIT_LOG_IR").is_some())
}

/// A compiled function's single machine-code entry point, callable
/// either as an ordinary call (`osr_id = -1`, starts at bytecode `ip
/// 0`) or as an on-stack-replacement entry (`osr_id >= 0`, jumps
/// straight into the loop header that `osr_id` identifies: see
/// `CompiledFunction::osr_ids`).
///
/// Calling convention (see this module's own docs for why this is
/// enough state to need no other bridging machinery):
/// - `vm`: the owning `VM`, as a raw pointer; compiled code and every
///   `runtime` helper it calls treat this like `&mut VM` would be used
///   from Rust; it's never aliased (nothing else touches this `VM`
///   while compiled code is running, single-threaded end to end).
/// - `base`: absolute index into `VM::registers` where this call's
///   register window starts; identical in meaning to `CallFrame::base`
///   in `vm::vm`.
/// - `closure`: the tagged `Value` of the specific `ObjClosure` this
///   invocation is running as (needed for `GetUpval`/`SetUpval`/
///   `Instr::Closure`'s own upvalue capture).
/// - returns: the function's return value's raw bit pattern (see
///   `Value::to_bits`), valid ONLY if `VM::jit_pending_error` is
///   nil when this call returns; a non-nil pending error means
///   the return value is meaningless and the caller must propagate the
///   error instead. See `runtime`'s module docs.
pub type EntryFn = unsafe extern "C" fn(
  vm: *mut crate::vm::vm::VM,
  base: u64,
  closure: u64,
  osr_id: i32,
  a0: u64,
  a1: u64,
  a2: u64,
  a3: u64,
) -> u64;

/// A successfully compiled function, cached on `ObjFunction::jit` for
/// as long as the VM lives. Machine code is never unloaded or
/// recompiled once produced; Zuri programs are short-lived processes,
/// not long-running servers that would need to reclaim or re-optimize
/// tiered-up code, so there's no eviction policy to implement.
pub struct CompiledFunction {
  pub entry: EntryFn,
  /// Bytecode ip (a loop header; the target of some backward
  /// `Instr::Jmp`) -> the small dense integer `entry` accepts as
  /// `osr_id` to jump directly into that loop's header block. Built by
  /// `codegen` while scanning the function for backward edges;
  /// consulted by `vm::vm::VM`'s own `Instr::Jmp` handler once a
  /// specific loop's back-edge count crosses
  /// `JitInfo::osr_threshold`.
  pub osr_ids: FxHashMap<usize, i32>,
}

/// A `Call` site's statically-resolved callee, computed once by
/// `vm::vm::VM::resolve_call_targets` (the only place with the live
/// global-table access needed to resolve a name) and consumed by
/// `codegen::FuncCompiler` to skip `jit::runtime::zuri_jit_call_prepare`'s
/// resolver entirely.
///
/// - `SelfRecursive`: the callee register is proven (see
///   `escape::self_reference_facts`) to always hold this exact
///   function's own closure; no runtime guard needed, since there's
///   nothing to misidentify; codegen emits a relocation-resolved direct
///   `call` to its own `FuncId`.
/// - `Known`: the callee register is proven (see
///   `escape::global_ref_facts`) to hold an unmodified read of some
///   other global name that, at this function's compile time, already
///   resolves to a different, already-JIT-compiled closure. Unlike
///   `SelfRecursive`, the global binding itself could still be
///   reassigned before this call site actually runs, so codegen still
///   guards it with a cheap value-identity comparison (`guard_bits`)
///   against the callee register's current contents at runtime,
///   falling back to the ordinary resolver on a miss. `entry` is a raw
///   `EntryFn` address, sound to bake as a compile-time immediate
///   because `CompiledFunction`'s own docs guarantee compiled code is
///   never unloaded or recompiled once produced.
/// - `Construct`: same proof as `Known`, except the global resolved to
///   a class; so this site is a constructor call, and codegen emits
///   `jit::runtime::zuri_jit_new_prepare`'s construction shape (which
///   yields the new instance) rather than `zuri_jit_call_prepare`'s
///   ordinary-call shape (which yields the callee's return value).
///   Carries nothing else because, unlike `Known`, there's nothing
///   worth baking: the constructor's own compiled entry generally
///   doesn't exist yet when the site's caller is compiled, so it still
///   gets resolved per call. This is purely a "which of the two shapes
///   belongs here" hint and needs no guard of its own;
///   `zuri_jit_new_prepare` re-checks the callee register itself and
///   returns zero (falling through to the same general path an
///   unclassified site would take) if the global has since been
///   reassigned to something that isn't a class.
/// - `ConstructKnown`: a `Construct` site whose whole construction
///   shape was additionally proven ahead of time (see
///   `vm::vm::VM::resolve_construct_target`); no class in the
///   ancestry declares a field initializer, and the constructor is a
///   known non-variadic closure whose `ObjFunction` address is baked
///   as `proto_ptr`. Codegen guards it the same way `emit_self_invoke`
///   does, on class identity plus a `method_table_generation` match,
///   then uses the lean `jit::runtime::zuri_jit_construct_prepare`
///   instead of re-walking the class -> constructor -> prototype chain
///   for every instance built.
///
///   `guard_bits` being stable to compare against isn't automatic: it
///   only holds because `Heap::alloc_class` allocates classes straight
///   into the old generation. Baked against a still-young class, this
///   guard (and `self_class_bits`, and `Known`'s) would stop matching
///   the instant a minor collection relocated it, silently disabling
///   the fast path for the rest of the process: see `alloc_class`'s
///   own docs.
#[derive(Clone, Copy, Debug)]
pub enum CallTarget {
  SelfRecursive,
  Known {
    /// The callee's compiled entry point, or `0` when it hasn't been
    /// compiled yet. A `0` here still carries useful information --
    /// `codegen` can inline such a callee (see
    /// `FuncCompiler::try_emit_inlined_call`), which needs only its
    /// bytecode, not its machine code; so the resolution is recorded
    /// either way and `emit_known_call`'s direct-dispatch path is
    /// gated on a non-zero entry.
    entry: usize,
    /// The callee's prototype, as its `Value` bits; what generated
    /// code guards on.
    ///
    /// Deliberately the prototype and not the closure: `alloc_closure`
    /// leaves an ordinary top-level function's closure in the nursery,
    /// so its address changes the first time it survives a minor
    /// collection, and a guard baked against it would stop matching
    /// forever after. A prototype is always `alloc_old` and never
    /// moves. See `object::obj_closure_function_offset`.
    ///
    /// Guarding on the prototype rather than the exact closure is also
    /// sound for the fast paths this protects: two closures sharing a
    /// prototype differ only in captured upvalues, and both consumers
    /// either call through the closure actually in the register
    /// (`emit_known_call`) or require an upvalue-free callee
    /// (`try_emit_inlined_call`).
    guard_bits: u64,
    /// The callee's own `ObjFunction`, as a raw pointer, so `codegen`
    /// can read its bytecode when deciding whether to inline it.
    ///
    /// Only ever dereferenced at compile time, never by generated code,
    /// and sound then for the same reason `ConstructKnown::proto_ptr`
    /// is: `Heap::alloc_function` allocates every `ObjFunction`
    /// directly into the non-moving old generation, and the closure
    /// `guard_bits` names is live throughout the resolution that
    /// produced this.
    proto_ptr: usize,
  },
  /// A call whose callee register is proven to hold a specific builtin
  /// native (`vm::natives`), resolved at compile time.
  ///
  /// Guarded on the native's own `Value` bits, which is sound only
  /// because `Heap::alloc_native` puts every native in the non-moving
  /// old generation: see its docs. A reassigned global fails the
  /// guard and falls back to the ordinary resolver, so this stays
  /// correct even though the binding is mutable.
  KnownNative {
    /// The native's own `NativeFn` pointer; what generated code
    /// guards on.
    ///
    /// Deliberately not the `Obj::Native`'s address: `Heap::alloc_native`
    /// leaves natives in the nursery (see its docs for the GC hazard
    /// that forced that), so their address changes the first time a
    /// minor collection promotes them, and an address guard would stop
    /// matching forever after. A `NativeFn` is a `'static` function
    /// pointer, unaffected by relocation. See
    /// `object::obj_native_func_offset`.
    guard_fn: u64,
    /// The `NativeFunction` itself, as a raw pointer, used only during
    /// compilation to read the native's name when choosing an
    /// intrinsic.
    ///
    /// Never baked into generated code and never dereferenced at
    /// runtime: the object is a young allocation that relocates on
    /// promotion (see `Heap::alloc_native`), so an address that
    /// outlived this compilation would dangle. The runtime helper
    /// re-reads the callee from its register instead, which the guard
    /// has already proven is this native.
    native_ptr: usize,
  },
  Construct,
  ConstructKnown {
    guard_bits: u64,
    generation: u64,
    field_count: u16,
    /// The constructor closure's own `Value` bits. Safe to bake only
    /// because `Heap::alloc_closure` allocates class methods into the
    /// non-relocating old generation: see its docs, and
    /// `VM::resolve_construct_target`, which re-checks that rather
    /// than assuming it.
    ctor_bits: u64,
    proto_ptr: usize,
  },
}

/// Everything `vm::vm::VM::enqueue_compile` resolves ahead of time
/// (via live VM/global-table access `jit::escape` deliberately never
/// touches) and hands to `codegen::compile`, bundled into one struct
/// rather than an ever-growing parameter list as this tier gains more
/// static-proof-driven fast paths. See each field's producer for the
#[derive(Clone)]
pub enum ResolvedGlobal {
  Class {
    guard_bits: u64,
    generation: u64,
    field_count: u16,
    ctor_bits: u64,
    proto_ptr: usize,
    safety: escape::ClassFieldSafety,
    field_slots: FxHashMap<String, u16>,
    simple_ctor_param_slots: Option<Vec<u16>>,
  },
  Native {
    guard_fn: u64,
    native_ptr: usize,
  },
  Closure {
    entry: usize,
    guard_bits: u64,
    proto_ptr: usize,
  },
}

/// A snapshot of facts resolved about `proto` before compilation, each
/// carrying its own whole-function or whole-class proof with a
/// soundness argument specific to it.
#[derive(Default)]
pub struct CompileFacts {
  pub self_field_slots: FxHashMap<String, u16>,
  pub self_numeric_fields: rustc_hash::FxHashSet<String>,
  pub numeric_fields: rustc_hash::FxHashSet<String>,
  pub param_field_slots: FxHashMap<u8, (u64, FxHashMap<String, u16>)>,
  pub self_class_bits: Option<(u64, u64)>,
  pub globals_snapshot: FxHashMap<String, ResolvedGlobal>,
  pub global_lists: rustc_hash::FxHashSet<String>,
  pub speculative_lists: Option<u64>,
  pub speculative_ints: Option<u64>,
  pub known_classes: FxHashMap<u64, FxHashMap<String, u16>>,
}

/// One construction site's compile-time view of the class it builds.
pub struct ConstructInfo {
  /// Which field names are safe to read via `GetField` because no
  /// method of the same name could shadow them.
  ///
  /// Lets `escape::analyze_one` treat `d.x` on a freshly constructed
  /// `d` as the plain field read it almost always is, instead of
  /// assuming it might materialize a `BoundMethod` that leaks `d`.
  /// Without this, every object a function reads its own fields off of
  /// is classified as escaping; which is to say essentially every
  /// object; and nothing could ever be scalar-replaced.
  pub safety: escape::ClassFieldSafety,
  /// Field name -> slot index, for resolving `GetField`/`SetField` on
  /// a scalar-replaced instance without any runtime lookup.
  pub field_slots: FxHashMap<String, u16>,
  pub field_count: u16,
  /// `Some(slots)` when the constructor does nothing but copy each of
  /// its parameters straight into a field: `slots[i]` is the field
  /// slot parameter `i` ends up in. `None` disqualifies the site from
  /// scalar replacement.
  ///
  /// This is what makes replacing the allocation possible at all: the
  /// object can't simply not exist if something has to call a
  /// constructor with it as `self`, so the constructor's effect has to
  /// be reproducible inline, and "store these arguments into these
  /// slots" is exactly that, with no call left to make. It's also by
  /// far the most common constructor ever written.
  pub simple_ctor_param_slots: Option<Vec<u16>>,
}
