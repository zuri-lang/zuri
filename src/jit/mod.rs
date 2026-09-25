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
pub mod ir;
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

/// Write each compiled function's finished machine code to
/// `tmp/jitasm/<symbol>.bin`; `ZURI_JIT_LOG_ASM=1`. Cranelift is built
/// here without its own disassembler, so the bytes go out raw for
/// `objdump -b binary -m i386:x86-64` to decode. Reading the real
/// instruction stream is the only way to settle whether a fast path
/// costs what its IR suggests it costs.
pub fn log_asm_enabled() -> bool {
  static ENABLED: OnceLock<bool> = OnceLock::new();
  *ENABLED.get_or_init(|| std::env::var_os("ZURI_JIT_LOG_ASM").is_some())
}

/// Run both tiers; `ZURI_JIT_TIER2=1`. A warm function gets profiling
/// tier-1 code first, and once that code has done enough work, tier 2
/// compiles it from the feedback gathered. A function tier 2 declines
/// stays in tier 1.
pub fn tier2_enabled() -> bool {
  static ENABLED: OnceLock<bool> = OnceLock::new();
  *ENABLED.get_or_init(|| std::env::var("ZURI_JIT_TIER2").is_ok_and(|v| v != "0"))
}

/// Skip tier 1 and compile every warm function straight through tier 2;
/// `ZURI_JIT_TIER2=direct`. Tier 2 then works from the interpreter's
/// feedback alone. For exercising tier 2's compiler on everything that
/// warms up, not for running programs.
pub fn tier2_direct() -> bool {
  static DIRECT: OnceLock<bool> = OnceLock::new();
  *DIRECT.get_or_init(|| std::env::var("ZURI_JIT_TIER2").is_ok_and(|v| v == "direct"))
}

/// One interpreter frame that a deoptimization inside a call built into
/// its caller has to push, as `runtime::zuri_jit_deopt_inlined` reads it.
#[derive(Clone, Copy, Debug)]
pub struct DeoptFrame {
  /// The `ObjFunction` the frame runs.
  pub proto: usize,
  /// Where in the calling function the call is; the caller resumes just
  /// past it once this frame returns.
  pub call_ip: usize,
  /// Where the frame's register 0 sits, from the compiled function's.
  pub offset: u8,
  /// The caller's register the result goes to.
  pub dst: u8,
  pub closure: ir::FrameClosure,
}

/// Set in `VM::jit_ip` and in a frame's `ip` for a position inside a
/// call built into its caller; the rest of the word is an index for
/// `inline_position`.
pub const INLINE_POSITION: usize = 1 << (usize::BITS - 1);

/// Positions inside calls built into their callers, each a list of
/// `(function, ip)` from the innermost call out to the compiled function
/// itself. Only ever appended to; a position stays valid as long as the
/// code publishing it can run, which is forever.
static INLINE_POSITIONS: std::sync::Mutex<Vec<Box<[(usize, usize)]>>> =
  std::sync::Mutex::new(Vec::new());

/// Registers a position inside a call built into its caller, returning
/// the value compiled code publishes for it.
pub fn register_inline_position(spots: Vec<(usize, usize)>) -> usize {
  let mut positions = INLINE_POSITIONS.lock().unwrap_or_else(|e| e.into_inner());
  positions.push(spots.into_boxed_slice());
  INLINE_POSITION | (positions.len() - 1)
}

/// The functions and positions a published inline position stands for,
/// innermost first.
pub fn inline_position(position: usize) -> Option<Vec<(usize, usize)>> {
  if position & INLINE_POSITION == 0 {
    return None;
  }
  let positions = INLINE_POSITIONS.lock().unwrap_or_else(|e| e.into_inner());
  positions
    .get(position & !INLINE_POSITION)
    .map(|spots| spots.to_vec())
}

pub fn log_facts_enabled() -> bool {
  static ENABLED: OnceLock<bool> = OnceLock::new();
  *ENABLED.get_or_init(|| std::env::var_os("ZURI_JIT_LOG_FACTS").is_some())
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
///   falling back to the ordinary resolver on a miss. `entry` only says
///   the callee had compiled code when this caller compiled; the call
///   itself reads the callee's current entry, since a callee that
///   deoptimizes drops its code.
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
    /// The callee's compiled entry point when this caller was compiled,
    /// or `0` when it hadn't been compiled yet. A `0` here still carries
    /// useful information: `codegen` can inline such a callee (see
    /// `FuncCompiler::try_emit_inlined_call`), which needs only its
    /// bytecode, not its machine code; so the resolution is recorded
    /// either way and `emit_known_call`'s direct-dispatch path is
    /// gated on a non-zero entry. The call reads the entry afresh.
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
    /// The native's name, needed only to decide whether it has an
    /// inline intrinsic body.
    ///
    /// Carried as the `&'static str` itself rather than as a pointer to
    /// the `NativeFunction` it lives on. That object is a young
    /// allocation and relocates when a minor collection promotes it
    /// (see `Heap::alloc_native`); since compilation runs on the
    /// background thread, a minor collection on the main thread can move
    /// it out from under a raw pointer between the moment the job is
    /// built and the moment the compiler reads it. The name string is
    /// `'static` and never moves, so it is safe to hand across.
    native_name: &'static str,
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
    native_name: &'static str,
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
  /// Global names holding a whole number at the moment compilation was
  /// requested.
  ///
  /// A module-level `const GRID = 128` is compiled to an ordinary
  /// `SetGlobal` with the const-ness discarded, so nothing downstream
  /// can tell it apart from a mutable binding. Without this, a function
  /// that only READS such a global proves nothing about it:
  /// `typeflow::IntFacts` has no notion of globals at all, and
  /// `TypeFacts` only tracks the ones written in the same function. The
  /// consequence shows up in generated code as index arithmetic done in
  /// floating point and converted back, with a whole-number proof, at
  /// every single use; `benchmarks/navier-stokes.zu` spends 28% of its
  /// time on exactly that, because its strides come from
  /// `const ROW = WIDTH + 2`.
  ///
  /// This is a snapshot, not a proof: the binding is mutable as far as
  /// anything here knows, so `codegen` re-verifies it at each
  /// `Instr::GetGlobal` that relies on it. That check runs once per
  /// read of the global rather than once per index derived from it,
  /// which in a nested loop is the whole difference.
  pub global_ints: rustc_hash::FxHashSet<String>,
  /// `global_ints` widened to every number. Same defect, one level up:
  /// `TypeFacts` does carry global slots, but seeds them only from a
  /// `SetGlobal` in the SAME function, so a function that merely reads
  /// `const DT = 0.1` proves nothing numeric about it and every
  /// arithmetic op using it pays a runtime `is_number` check.
  pub global_numbers: rustc_hash::FxHashSet<String>,
  pub speculative_lists: Option<u64>,
  pub speculative_ints: Option<u64>,
  /// Bit `i` bets that list parameter `i` holds nothing but whole
  /// numbers, so an element read out of it can feed an index or an
  /// integer operation without its own whole-number check. Sampled
  /// from a bounded prefix by `VM::sample_param_elem_types` and made
  /// sound by the full scan `codegen::emit_entry_dispatch` emits once
  /// per entry; a list that fails the scan runs the general body.
  /// Set when a previous compilation's element scan was observed to
  /// fail; both element bets below are dropped when it is.
  pub elem_speculation_off: bool,
  pub speculative_int_lists: Option<u64>,
  /// The same bet widened to any number, fractions included: what a
  /// list of floats gets proven with, so a read out of it skips
  /// `is_number` without being claimed whole.
  pub speculative_num_lists: Option<u64>,
  pub known_classes: FxHashMap<u64, FxHashMap<String, u16>>,
  /// Method name -> the `ObjFunction` the COMPILING function's own class
  /// resolves it to, for every method that class has. Only meaningful
  /// alongside `self_class_bits`, whose class+generation guard is what
  /// makes the mapping safe to bake in; see `emit_invoke_inline`.
  ///
  /// The pointer is to an `ObjFunction`, which never moves, so it stays
  /// valid for the whole compile. The closure wrapping it is deliberately
  /// not recorded: closures are young allocations that relocate.
  pub self_method_protos: FxHashMap<String, usize>,
  /// Bytecode positions an earlier compilation of this function gave up
  /// at; see `JitInfo::deopt_sites`. Copied out of the prototype when
  /// the job is built, like the rest of this struct, so nothing here is
  /// read from a worker while the VM is still running.
  pub deopt_sites: rustc_hash::FxHashSet<usize>,
  /// The compiling VM's young-generation budget, baked into the
  /// safepoint check as an immediate; see `Heap::young_budget`. Carried
  /// per compilation because an isolate's heap collects on a smaller
  /// one than the main VM's does.
  pub young_budget: usize,
  /// Drop every field-class bet in this function; see
  /// `JitInfo::field_speculation_off`.
  pub field_speculation_off: bool,
  /// `Chunk::feedback` as it stood when compilation was requested, one
  /// byte of `chunk::kind` bits per instruction. Copied rather than read
  /// in place because the interpreter keeps writing the live cells while
  /// a worker compiles.
  pub site_kinds: Vec<u8>,
  /// Take no type bets from `site_kinds` at all; see
  /// `JitInfo::site_speculation_off`.
  pub site_speculation_off: bool,
  /// For each `GetField`/`SetField` site whose field cache held a class
  /// when compilation was requested, that class's `Value` bits and the
  /// field's slot on it; see `VM::resolve_site_classes`. Copied for the
  /// same reason as `site_kinds`.
  pub site_classes: FxHashMap<usize, (u64, u16)>,
  /// Each `GetField`/`SetField` site that has seen instances of more than
  /// one class, with the class declaring the field there, its depth in
  /// `ObjClass::display` and the field's slot. `None` when that class sits
  /// deeper than the display records; the site is then left to its cache.
  pub site_families: FxHashMap<usize, Option<(u64, u8, u16)>>,
  /// Build the profiling kind of tier-1 code: it records the same site
  /// feedback the interpreter does and counts its entries and loop turns
  /// toward tier-up. See `JitInfo::profiling`.
  pub profile: bool,
  /// Where the function's site feedback cells live, one byte per
  /// instruction, when `profile` is set. See `Chunk::ensure_feedback`.
  pub feedback_cells: usize,
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
