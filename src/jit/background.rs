//! A single dedicated background thread that does the expensive half
//! of JIT compilation (`cranelift_codegen::Context::compile` --
//! register allocation, instruction selection, machine-code encoding)
//! off the VM's own thread, so a function crossing its warmup
//! threshold never stalls the interpreter waiting for Cranelift's
//! optimizing backend to finish.
//!
//! # Why this is safe with no locking between the two threads
//!
//! Compiling a function splits cleanly into two stages (see
//! `jit::engine::JitEngine::build_ir`/`install_compiled`):
//!
//! 1. **Build IR** (`JitEngine::build_ir`, always on the VM's own
//!    thread): walks `proto`'s bytecode and produces an owned
//!    `cranelift_codegen::Context` holding pure IR; every constant
//!    `Value` the bytecode referenced is already baked into that IR as
//!    a raw immediate (see `jit::codegen`'s own docs), so once this
//!    step returns, the resulting `Context` has no remaining
//!    dependency on `proto`, the heap, or the GC. This is the only
//!    stage that touches `proto`, which is why it must stay
//!    synchronous: see `VM::sample_param_types`/`enqueue_or_ready`'s
//!    docs on how `proto`'s liveness is guaranteed for exactly this
//!    stage's duration.
//! 2. **Backend-compile** (`Context::compile`, done here): needs only
//!    the `Context` from step 1 (moved in, exclusively owned by this
//!    thread for the duration) and a `TargetIsa` handle. Cranelift's
//!    own `TargetIsa` trait is `Send + Sync` and `JitEngine` hands out
//!    an independent `Arc` clone of it (see `JitEngine::isa_handle`)
//!    that never touches `JITModule`; so this stage needs no lock,
//!    no shared mutable state, nothing from the VM beyond the `Context`
//!    it was given.
//!
//! The VM's own thread later installs the finished machine code
//! (`JitEngine::install_compiled`, plain memcpy + relocation fixups,
//! no register allocation) once it drains this thread's result channel
//!: see `VM::drain_jit_results`.
//!
//! # Why `*const ObjFunction` is safe to carry across this boundary
//!
//! `CompileJob`/`CompileResult` carry a raw `*const ObjFunction`
//! purely as an opaque identifier; this thread never dereferences
//! it, only Cranelift's `Context`/`TargetIsa` data. The VM's own
//! thread is what eventually dereferences it back (in
//! `VM::drain_jit_results`), and it keeps the function pinned as a GC
//! root (`VM::pending_jit_compiles`) for the entire round trip, from
//! the moment a job is sent here to the moment its result is drained,
//! so the pointer is always valid whenever anyone actually uses it.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};

use cranelift_codegen::Context;
use cranelift_codegen::control::ControlPlane;
use cranelift_codegen::isa::TargetIsa;
use cranelift_module::{FuncId, ModuleReloc};
use rustc_hash::FxHashMap;

use crate::jit::typeflow;
use crate::vm::object::ObjFunction;

/// A raw pointer wrapper that's `Send` purely as an opaque token.
/// See above for why it's never dereferenced here.
pub struct SendPtr(pub *const ObjFunction);
unsafe impl Send for SendPtr {}

pub struct CompileJob {
  pub ctx: Context,
  pub func_id: FuncId,
  pub osr_ids: FxHashMap<usize, i32>,
  pub proto: SendPtr,
  /// Carried through purely for `VM::drain_jit_results`'s log line
  /// (see `codegen::compile`'s own docs on what this means).
  pub speculative_params: Option<u64>,
  /// Same purpose as `speculative_params`, carried through purely for
  /// the log line (see `jit::typeflow::SpeculativeRegs`'s own docs).
  pub speculative_regs: Option<typeflow::SpeculativeRegs>,
}

pub struct CompileResult {
  pub proto: SendPtr,
  pub func_id: FuncId,
  pub osr_ids: FxHashMap<usize, i32>,
  pub speculative_params: Option<u64>,
  pub speculative_regs: Option<typeflow::SpeculativeRegs>,
  /// `Ok((code_bytes, alignment, relocations))` on success (exactly
  /// what `JitEngine::install_compiled` needs) or a human-readable
  /// failure reason (mirrors `codegen::compile`'s own `Err(String)`
  /// convention).
  pub outcome: Result<(Vec<u8>, u64, Vec<ModuleReloc>), String>,
}

pub struct JitCompilerHandle {
  pub job_tx: Sender<CompileJob>,
  pub result_rx: Receiver<CompileResult>,
  /// Set by the compiler thread after a finished result is already in
  /// `result_rx`; cleared by `VM::drain_jit_results` before it drains.
  ///
  /// Exists purely so the VM can skip `result_rx` entirely on the
  /// overwhelmingly common path. `VM::tiered_entry` runs on every
  /// single `Instr::Call`/`Invoke`, but a real compile result lands
  /// only a handful of times in an entire process lifetime; and
  /// `Receiver::try_recv` is not free (it walks the channel's own
  /// atomic state machine), so unconditionally polling it per call
  /// costs several percent of total runtime on call-heavy code. A
  /// relaxed load of an uncontended, read-mostly flag costs nothing by
  /// comparison.
  ///
  /// The store/clear order on both sides is what makes this safe, and
  /// neither may be swapped:
  ///
  /// - Producer sends first, then sets. So observing `true` guarantees
  ///   the corresponding `send` has already happened and a following
  ///   `try_recv` is certain to see it; the reverse order could set
  ///   the flag for a result not yet in the channel, let the VM clear
  ///   it and find nothing, and strand that result forever (its
  ///   function would keep `compiling` set and never be installed).
  /// - Consumer clears first, then drains. A result arriving during
  ///   the drain re-sets the flag rather than being lost; worst case
  ///   it was already drained by the in-flight loop and the next call
  ///   does one spurious (empty, harmless) drain.
  pub results_pending: Arc<AtomicBool>,
}

/// Spawns the single background compiler thread and returns the
/// job/result channel handles the VM uses to talk to it. The thread
/// runs for the rest of the process; like the compiled code it
/// produces (see `CompiledFunction`'s own docs), it is never torn
/// down; when the job sender is dropped (VM shutdown), its loop below
/// exits and the thread ends naturally, which this deliberately does
/// not wait on (a short-lived CLI process has no need to join a
/// background worker before exiting).
pub fn spawn(isa: Arc<dyn TargetIsa>) -> JitCompilerHandle {
  let (job_tx, job_rx) = channel::<CompileJob>();
  let (result_tx, result_rx) = channel::<CompileResult>();
  let results_pending = Arc::new(AtomicBool::new(false));
  let worker_pending = Arc::clone(&results_pending);

  std::thread::Builder::new()
    .name("zuri-jit-compiler".to_string())
    .spawn(move || compiler_loop(isa, job_rx, result_tx, worker_pending))
    .expect("zuri: failed to spawn the background JIT compiler thread");

  JitCompilerHandle {
    job_tx,
    result_rx,
    results_pending,
  }
}

fn compiler_loop(
  isa: Arc<dyn TargetIsa>,
  job_rx: Receiver<CompileJob>,
  result_tx: Sender<CompileResult>,
  results_pending: Arc<AtomicBool>,
) {
  let mut ctrl_plane = ControlPlane::default();
  for mut job in job_rx {
    // See this module's docs: `compile`'s own returned reference
    // borrows `job.ctx` for its own duration, so it's dropped
    // immediately (not held past this statement) to leave `job.ctx`
    // free for the separate `compiled_code()`/`&job.ctx.func` (shared,
    // immutable) borrows used just below; the same pattern
    // `cranelift_module`'s own `define_function` uses internally.
    let compile_result = job.ctx.compile(&*isa, &mut ctrl_plane);
    let outcome = match compile_result {
      Ok(_) => {
        let compiled_code = job
          .ctx
          .compiled_code()
          .expect("Context::compile just succeeded");
        let alignment = compiled_code.buffer.alignment as u64;
        let bytes = compiled_code.code_buffer().to_vec();
        let relocs = compiled_code
          .buffer
          .relocs()
          .iter()
          .map(|r| ModuleReloc::from_mach_reloc(r, &job.ctx.func, job.func_id))
          .collect();
        Ok((bytes, alignment, relocs))
      },
      Err(e) => Err(format!("backend compile failed: {e:?}")),
    };

    let result = CompileResult {
      proto: job.proto,
      func_id: job.func_id,
      osr_ids: std::mem::take(&mut job.osr_ids),
      speculative_params: job.speculative_params,
      speculative_regs: job.speculative_regs,
      outcome,
    };
    // A closed result channel means the VM has shut down; nothing
    // left to report to, so just stop.
    if result_tx.send(result).is_err() {
      return;
    }
    // Strictly after the `send` above: see `results_pending`'s docs.
    results_pending.store(true, Ordering::Release);
  }
}
