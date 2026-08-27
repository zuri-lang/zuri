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

use rustc_hash::FxHashMap;

use crate::jit::engine::JitEngine;
use crate::jit::{CompileFacts, EntryFn, typeflow};
use crate::vm::object::ObjFunction;

/// A raw pointer wrapper that's `Send` purely as an opaque token.
/// See above for why it's never dereferenced here.
pub struct SendPtr(pub *const ObjFunction);
unsafe impl Send for SendPtr {}

pub struct CompileJob {
  pub proto: SendPtr,
  /// Carried through purely for `VM::drain_jit_results`'s log line
  /// (see `codegen::compile`'s own docs on what this means).
  pub speculative_params: Option<u64>,
  /// Same purpose as `speculative_params`, carried through purely for
  /// the log line (see `jit::typeflow::SpeculativeRegs`'s own docs).
  pub speculative_regs: Option<typeflow::SpeculativeRegs>,
  pub facts: CompileFacts,
}

pub struct CompileResult {
  pub proto: SendPtr,
  pub osr_ids: FxHashMap<usize, i32>,
  pub speculative_params: Option<u64>,
  pub speculative_regs: Option<typeflow::SpeculativeRegs>,
  /// `Ok(entry_fn)` on success or a human-readable failure reason.
  pub outcome: Result<EntryFn, String>,
}

pub struct JitCompilerHandle {
  pub job_tx: Option<Sender<CompileJob>>,
  pub result_rx: Receiver<CompileResult>,
  pub results_pending: Arc<AtomicBool>,
  pub shutdown: Arc<AtomicBool>,
  pub thread: Option<std::thread::JoinHandle<()>>,
}

impl Drop for JitCompilerHandle {
  fn drop(&mut self) {
    self.shutdown.store(true, Ordering::Release);
    drop(self.job_tx.take());
    if let Some(thread) = self.thread.take() {
      let _ = thread.join();
    }
  }
}

/// Spawns the single background compiler thread and returns the
/// job/result channel handles the VM uses to talk to it. The thread
/// runs until VM shutdown, at which point `JitCompilerHandle::drop`
/// signals shutdown and joins the worker cleanly before heap deallocation.
pub fn spawn() -> JitCompilerHandle {
  let (job_tx, job_rx) = channel::<CompileJob>();
  let (result_tx, result_rx) = channel::<CompileResult>();
  let results_pending = Arc::new(AtomicBool::new(false));
  let worker_pending = Arc::clone(&results_pending);
  let shutdown = Arc::new(AtomicBool::new(false));
  let worker_shutdown = Arc::clone(&shutdown);

  let thread = std::thread::Builder::new()
    .name("zuri-jit-compiler".to_string())
    .spawn(move || compiler_loop(job_rx, result_tx, worker_pending, worker_shutdown))
    .expect("zuri: failed to spawn the background JIT compiler thread");

  JitCompilerHandle {
    job_tx: Some(job_tx),
    result_rx,
    results_pending,
    shutdown,
    thread: Some(thread),
  }
}

fn compiler_loop(
  job_rx: Receiver<CompileJob>,
  result_tx: Sender<CompileResult>,
  results_pending: Arc<AtomicBool>,
  shutdown: Arc<AtomicBool>,
) {
  let mut engine = JitEngine::new();
  for job in job_rx {
    if shutdown.load(Ordering::Relaxed) {
      return;
    }
    let proto = unsafe { &*job.proto.0 };
    let (outcome, osr_ids) = match engine.compile_function(
      proto,
      job.speculative_params,
      job.speculative_regs,
      job.facts,
      Some(&shutdown),
    ) {
      Ok((entry, osr_ids)) => (Ok(entry), osr_ids),
      Err(e) => (Err(e), FxHashMap::default()),
    };

    if shutdown.load(Ordering::Relaxed) {
      return;
    }

    let result = CompileResult {
      proto: job.proto,
      osr_ids,
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
