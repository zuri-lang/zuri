//! A Cranelift-backed baseline JIT tier for the Zuri VM.
//!
//! # Architecture in one page
//!
//! The interpreter (`vm::vm::VM::run_until`) is a register machine over
//! one flat `Vec<Value>` (`VM::registers`), sliced into per-frame
//! windows. This JIT tier does NOT introduce a second, separate
//! representation of a function's local state -- every VM register a
//! compiled function touches is read and written through exactly the
//! same memory `VM::registers` already uses, via a raw pointer to the
//! frame's own window (see `CompiledFunction`'s calling convention,
//! below). This one decision is what makes every one of the harder
//! requirements for this tier tractable with a REASONABLE amount of
//! code, instead of needing a second, parallel deoptimization/state-
//! reconstruction machinery most JITs need because their compiled
//! code keeps VM state in real machine registers/an SSA form that has
//! to be painstakingly reconstructed at every boundary:
//!
//! - **Mixed-mode execution** (compiled code calling an as-yet-uncompiled
//!   function): the callee's frame is pushed with the SAME `base`
//!   convention the interpreter already uses, so `VM::call_value` can
//!   run it through `run_until` exactly as if the caller were also
//!   interpreted. See `vm::vm::VM::call_value`.
//! - **GC safepoints**: since a register's value is never anywhere BUT
//!   `VM::registers` (no separate SSA copy to flush first), the
//!   existing non-moving, root-scanning collector
//!   (`VM::collect_garbage`) already sees a JIT frame's registers
//!   correctly as long as that frame's `CallFrame` (base + function
//!   pointer) is on `VM::frames` while compiled code runs -- which it
//!   always is (pushed before entry, popped after). Compiled code just
//!   has to call back into `VM::collect_garbage` at the same two kinds
//!   of point the interpreter is willing to pause at: loop back-edges
//!   and call sites (see `codegen::FuncCompiler::emit_safepoint`).
//! - **On-stack replacement**: jumping into the MIDDLE of a compiled
//!   function needs no value reconstruction either -- the OSR entry
//!   block is just another predecessor of the target loop header block
//!   that hasn't initialized any local Cranelift SSA variables (there
//!   are none; everything lives in `VM::registers` already) and simply
//!   jumps straight there. See `codegen::FuncCompiler::compile` and
//!   `warmup::osr_threshold`.
//! - **Exceptions bail to the interpreter**: rather than reimplementing
//!   `catch`/`raise` unwinding as generated machine code, a function
//!   that contains `Instr::PushCatch`/`Instr::Raise` is simply never
//!   selected for compilation at all (see `codegen::is_eligible`) --
//!   it always runs interpreted, where the existing, already-correct
//!   `catch_stack` unwinder handles it. A compiled function CAN still
//!   raise indirectly (an arithmetic type error, a callee that itself
//!   raises, ...); when that happens control leaves compiled code
//!   entirely and propagates the exception up to whatever Rust frame
//!   invoked this compiled function to begin with (interpreter's
//!   `dispatch_call`, `call_value`, or an enclosing compiled caller's
//!   own call-site helper) -- exactly mirroring how an interpreted
//!   `Err(Value)` already propagates. See `runtime`'s module docs for
//!   the exact error-channel protocol.
//! - **Operator overloading**: a binary op's fast path (both operands
//!   plain numbers) is inlined directly as machine code; anything else
//!   -- strings, lists, bigints, a class's `@add` override, ... --
//!   calls straight back into the EXACT SAME Rust methods the
//!   interpreter itself uses (`VM::binary_add`, `VM::compare`, ...),
//!   so there is exactly one place that implements what `+`/`*`/`<`/...
//!   mean for a non-numeric operand, shared by both tiers. See
//!   `runtime`'s arithmetic helpers.
//!
//! # Module layout
//!
//! - `warmup` -- the size-scaled call/OSR warm-up threshold curves.
//! - `engine` -- owns the `cranelift_jit::JITModule`, registers every
//!   `runtime` helper as a linkable symbol, and drives one function's
//!   compilation from a `&ObjFunction` to a `CompiledFunction`.
//! - `codegen` -- the actual bytecode -> Cranelift IR translator.
//! - `runtime` -- the small, fixed set of `extern "C" fn`s compiled
//!   code calls out to for anything that isn't worth (or safe to)
//!   inline directly as machine code.

pub mod codegen;
pub mod engine;
pub mod runtime;
pub mod warmup;

use std::sync::OnceLock;

use rustc_hash::FxHashMap;

pub use engine::JitEngine;

/// Emit a one-line `[jit] compiled '...'`/`'... ineligible: ...'`
/// message per compilation attempt -- `ZURI_JIT_LOG=1`.
pub fn log_enabled() -> bool {
  static ENABLED: OnceLock<bool> = OnceLock::new();
  *ENABLED.get_or_init(|| std::env::var_os("ZURI_JIT_LOG").is_some())
}

/// Dump each compiled function's full Cranelift IR to stderr --
/// `ZURI_JIT_LOG_IR=1`. Separate from `log_enabled` since this is
/// substantially noisier (one full IR listing per compiled function).
pub fn log_ir_enabled() -> bool {
  static ENABLED: OnceLock<bool> = OnceLock::new();
  *ENABLED.get_or_init(|| std::env::var_os("ZURI_JIT_LOG_IR").is_some())
}

/// A compiled function's single machine-code entry point, callable
/// either as an ordinary call (`osr_id = -1`, starts at bytecode `ip
/// 0`) or as an on-stack-replacement entry (`osr_id >= 0`, jumps
/// straight into the loop header that `osr_id` identifies -- see
/// `CompiledFunction::osr_ids`).
///
/// Calling convention (see this module's own docs for why this is
/// enough state to need no other bridging machinery):
/// - `vm`: the owning `VM`, as a raw pointer -- compiled code and every
///   `runtime` helper it calls treat this exactly like `&mut VM` would
///   be used from Rust; it is never aliased (nothing else touches this
///   `VM` while compiled code is running, single-threaded end to end).
/// - `base`: absolute index into `VM::registers` where this call's
///   register window starts -- identical in meaning to `CallFrame::base`
///   in `vm::vm`.
/// - `closure`: the tagged `Value` of the specific `ObjClosure` this
///   invocation is running as (needed for `GetUpval`/`SetUpval`/
///   `Instr::Closure`'s own upvalue capture).
/// - returns: the function's return value's raw bit pattern (see
///   `Value::to_bits`), valid ONLY if `VM::jit_pending_exception` is
///   nil when this call returns -- a non-nil pending exception means
///   the return value is meaningless and the caller must propagate the
///   exception instead. See `runtime`'s module docs.
pub type EntryFn = unsafe extern "C" fn(vm: *mut crate::vm::vm::VM, base: u64, closure: u64, osr_id: i32) -> u64;

/// A successfully compiled function, cached on `ObjFunction::jit` for
/// as long as the VM lives. Machine code is never unloaded or
/// recompiled once produced -- Zuri programs are short-lived processes,
/// not long-running servers that would need to reclaim/re-optimize
/// tiered-up code, so there is no eviction policy to implement.
pub struct CompiledFunction {
  pub entry: EntryFn,
  /// Bytecode ip (a loop header -- the target of some backward
  /// `Instr::Jmp`) -> the small dense integer `entry` accepts as
  /// `osr_id` to jump directly into that loop's header block. Built by
  /// `codegen` while scanning the function for backward edges;
  /// consulted by `vm::vm::VM`'s own `Instr::Jmp` handler once a
  /// specific loop's back-edge count crosses
  /// `JitInfo::osr_threshold`.
  pub osr_ids: FxHashMap<usize, i32>,
}
