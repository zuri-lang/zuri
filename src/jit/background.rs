//! A pool of background threads that does JIT compilation
//! (`JitEngine::compile_function`: the bytecode -> IR walk, then
//! Cranelift's register allocation, instruction selection and
//! machine-code encoding) off the VM's own thread, so a function
//! crossing its warmup threshold never stalls the interpreter waiting
//! for the optimizing backend to finish.
//!
//! # Why a pool and not one thread
//!
//! One worker is enough for a benchmark with a handful of hot
//! functions, and wrong for a real program with thousands: every
//! function that warms up after the first waits behind whatever is
//! already on the queue, and keeps running interpreted for the whole
//! wait. That queue is deepest exactly at startup, when a program is
//! warming its whole working set at once and staying interpreted costs
//! the most.
//!
//! # What the workers share
//!
//! One `Receiver`, behind a `Mutex`. `std::sync::mpsc` has a single
//! consumer by construction, so a worker takes the lock, dequeues one
//! job, and drops the lock again before compiling anything: exactly one
//! worker is ever parked in `recv` at a time, and the rest take the
//! queue as jobs arrive. Nothing else is shared. Each worker owns its
//! own `JitEngine`, and with it its own `JITModule` and its own
//! executable memory.
//!
//! A prototype is only ever queued once at a time (`JitInfo::compiling`
//! is set before the job is sent and cleared only when its result is
//! drained), so no two workers ever compile the same function.
//!
//! # Why this needs no locking against the VM
//!
//! Everything the VM could be mutating underneath a worker is
//! snapshotted into the job before it is sent: `CompileFacts` is built
//! on the VM's own thread in `VM::enqueue_compile`, by value, and every
//! constant `Value` the bytecode referenced is baked into the IR as a
//! raw immediate (see `jit::codegen`'s own docs). What a worker reads
//! through `proto` is the bytecode and the constant pool, both written
//! once when the function was compiled from source and never touched
//! again.
//!
//! The VM's own thread installs the finished machine code once it
//! drains the result channel: see `VM::drain_jit_results`.
//!
//! # Why `*const ObjFunction` is safe to carry across this boundary
//!
//! `CompileJob`/`CompileResult` carry a raw `*const ObjFunction`, and a
//! worker really does dereference it -- that is the function it
//! compiles. Two things make that sound. The VM pins the function as a
//! GC root (`VM::pending_jit_compiles`) for the entire round trip, from
//! the moment a job is sent here to the moment its result is drained,
//! so it cannot be collected while a worker holds it. And a function
//! never moves even when a collection does run: `Heap::alloc_function`
//! allocates straight into old-generation storage precisely so its
//! address is fixed for life, the same fact `codegen::func_ptr_const`
//! relies on to bake it in as an immediate.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex, OnceLock};

use rustc_hash::FxHashMap;

use crate::jit::engine::JitEngine;
use crate::jit::{CompileFacts, EntryFn, typeflow};
use crate::vm::object::ObjFunction;

/// A raw pointer wrapper that's `Send` purely as an opaque token.
/// See above for why it's never dereferenced here.
pub struct SendPtr(pub *const ObjFunction);
unsafe impl Send for SendPtr {}

pub struct CompileJob {
  /// The function to compile, or a null pointer when this job is the
  /// wake-up `CompileJob::shutdown_signal()` sends.
  pub proto: SendPtr,
  /// Carried through purely for `VM::drain_jit_results`'s log line
  /// (see `codegen::compile`'s own docs on what this means).
  pub speculative_params: Option<u64>,
  /// Same purpose as `speculative_params`, carried through purely for
  /// the log line (see `jit::typeflow::SpeculativeRegs`'s own docs).
  pub speculative_regs: Option<typeflow::SpeculativeRegs>,
  pub facts: CompileFacts,
  /// Present when the function should go through the optimizing tier;
  /// what its IR builder reads instead of `facts`.
  pub tier2: Option<Box<crate::jit::ir::build::Feedback>>,
  /// Optional target for the finished result. When provided (e.g. for
  /// shared isolate workers), the result is sent to this channel and its
  /// `results_pending` flag is updated, allowing multiple VMs to share a single
  /// compiler queue.
  pub reply_to: Option<(Sender<CompileResult>, Arc<AtomicBool>)>,
}

impl CompileJob {
  /// A job that carries no work, sent purely to wake a compiler thread
  /// out of a blocking `recv()` so it can notice that the shutdown flag
  /// is set. One per worker, since each consumes exactly the message
  /// that woke it.
  ///
  /// Dropping the sender would do the same job if the pool held the
  /// only one, but it does not: every VM sharing this compiler keeps a
  /// clone of it alive (see `VM::set_shared_jit_compiler`), so the
  /// channel stays connected for as long as the isolate workers do.
  /// Sending the wake-up is what makes shutdown independent of who
  /// else is still holding a sender.
  ///
  /// `proto` is deliberately null. `compiler_loop` checks the shutdown
  /// flag the moment `recv()` returns and never touches the job.
  pub fn shutdown_signal() -> Self {
    CompileJob {
      proto: SendPtr(std::ptr::null()),
      speculative_params: None,
      speculative_regs: None,
      facts: CompileFacts::default(),
      tier2: None,
      reply_to: None,
    }
  }
}

pub struct CompileResult {
  pub proto: SendPtr,
  pub osr_ids: FxHashMap<usize, i32>,
  pub speculative_params: Option<u64>,
  pub speculative_regs: Option<typeflow::SpeculativeRegs>,
  /// Carried back purely so `ZURI_JIT_LOG` can report what this
  /// compilation actually bet on; the facts themselves were consumed
  /// by the compile.
  pub speculative_lists: Option<u64>,
  pub speculative_ints: Option<u64>,
  /// Which tier produced `outcome`.
  pub tier: u8,
  /// `Ok(entry_fn)` on success or a human-readable failure reason.
  pub outcome: Result<EntryFn, String>,
}

pub struct JitCompilerHandle {
  pub job_tx: Option<Sender<CompileJob>>,
  pub result_rx: Receiver<CompileResult>,
  pub results_pending: Arc<AtomicBool>,
  pub shutdown: Arc<AtomicBool>,
  pub threads: Vec<std::thread::JoinHandle<()>>,
}

impl Drop for JitCompilerHandle {
  fn drop(&mut self) {
    self.shutdown.store(true, Ordering::Release);

    // The flag alone cannot interrupt a blocking `recv()`, so every
    // worker has to be sent something before it will look at it.
    if let Some(job_tx) = self.job_tx.as_ref() {
      for _ in 0..self.threads.len() {
        let _ = job_tx.send(CompileJob::shutdown_signal());
      }
    }

    drop(self.job_tx.take());
    for thread in self.threads.drain(..) {
      let _ = thread.join();
    }
  }
}

/// How many compiler workers to run: one per core the VM itself is not
/// running on, floored at one. Handing the pool every core would put
/// compilation straight back into competition with the interpreter,
/// which is the thing moving it off-thread was for. Read once and
/// cached; `ZURI_JIT_THREADS` overrides it, purely for measurement (the
/// same treatment `jit::warmup`'s thresholds get).
pub fn worker_count() -> usize {
  static COUNT: OnceLock<usize> = OnceLock::new();
  *COUNT.get_or_init(|| {
    if let Some(n) = std::env::var("ZURI_JIT_THREADS")
      .ok()
      .and_then(|s| s.parse::<usize>().ok())
    {
      return n.max(1);
    }
    std::thread::available_parallelism()
      .map(|n| n.get().saturating_sub(1))
      .unwrap_or(1)
      .max(1)
      .clamp(1, 4)
  })
}

/// Spawns the background compiler pool and returns the job/result
/// channel handles the VM uses to talk to it. The workers run until VM
/// shutdown, at which point `JitCompilerHandle::drop` signals shutdown
/// and joins them cleanly before heap deallocation.
pub fn spawn() -> JitCompilerHandle {
  let (job_tx, job_rx) = channel::<CompileJob>();
  let (result_tx, result_rx) = channel::<CompileResult>();
  let results_pending = Arc::new(AtomicBool::new(false));
  let shutdown = Arc::new(AtomicBool::new(false));
  let threads = spawn_workers(
    "zuri-jit-compiler",
    job_rx,
    result_tx,
    Arc::clone(&results_pending),
    Arc::clone(&shutdown),
  );

  JitCompilerHandle {
    job_tx: Some(job_tx),
    result_rx,
    results_pending,
    shutdown,
    threads,
  }
}

/// The isolate pool's own compiler, shared by every isolate worker VM:
/// results go back through each job's own `reply_to` channel rather
/// than a shared one, since those VMs live on different threads (see
/// `VM::set_shared_jit_compiler`).
pub fn spawn_named(
  name: String,
) -> (
  Sender<CompileJob>,
  Arc<AtomicBool>,
  Vec<std::thread::JoinHandle<()>>,
) {
  let (job_tx, job_rx) = channel::<CompileJob>();
  let (result_tx, _result_rx) = channel::<CompileResult>();
  let results_pending = Arc::new(AtomicBool::new(false));
  let shutdown = Arc::new(AtomicBool::new(false));
  let threads = spawn_workers(
    &name,
    job_rx,
    result_tx,
    results_pending,
    Arc::clone(&shutdown),
  );

  (job_tx, shutdown, threads)
}

/// Spawns `worker_count()` threads over one shared job queue; see this
/// module's own docs on why a `Mutex` around the `Receiver` is all the
/// coordination the pool needs.
fn spawn_workers(
  name: &str,
  job_rx: Receiver<CompileJob>,
  result_tx: Sender<CompileResult>,
  results_pending: Arc<AtomicBool>,
  shutdown: Arc<AtomicBool>,
) -> Vec<std::thread::JoinHandle<()>> {
  let jobs = Arc::new(Mutex::new(job_rx));
  let count = worker_count();
  let mut threads = Vec::with_capacity(count);
  for i in 0..count {
    let jobs = Arc::clone(&jobs);
    let result_tx = result_tx.clone();
    let results_pending = Arc::clone(&results_pending);
    let shutdown = Arc::clone(&shutdown);
    let thread = std::thread::Builder::new()
      .name(format!("{name}-{i}"))
      .spawn(move || compiler_loop(jobs, result_tx, results_pending, shutdown))
      .expect("zuri: failed to spawn a background JIT compiler thread");
    threads.push(thread);
  }
  threads
}

fn compiler_loop(
  jobs: Arc<Mutex<Receiver<CompileJob>>>,
  result_tx: Sender<CompileResult>,
  results_pending: Arc<AtomicBool>,
  shutdown: Arc<AtomicBool>,
) {
  let mut engine = JitEngine::new();
  loop {
    if shutdown.load(Ordering::Relaxed) {
      return;
    }
    // Scoped so the queue lock is released before this worker starts
    // compiling; holding it across `compile_function` would collapse
    // the pool back down to one worker. A poisoned lock means another
    // worker panicked mid-dequeue, which says nothing about this job,
    // so take the queue anyway.
    let job = {
      let job_rx = jobs
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
      match job_rx.recv() {
        Ok(job) => job,
        Err(_) => return,
      }
    };

    // Re-checked here, not just at the top of the loop: a worker parks
    // in `recv()` for as long as there is no work, so the flag can only
    // have been noticed after something arrived; and the thing that
    // arrived may well be the wake-up
    // `CompileJob::shutdown_signal()` sent, which holds a null `proto`
    // and must not be dereferenced.
    if shutdown.load(Ordering::Relaxed) {
      return;
    }

    let proto = unsafe { &*job.proto.0 };
    let speculative_lists = job.facts.speculative_lists;
    let speculative_ints = job.facts.speculative_ints;
    // A panic inside Cranelift is a compiler bug, but it is a bug about
    // this one function. Letting it unwind would take the worker down
    // with it, and once every worker is gone nothing the VM queues ever
    // compiles again. Report the function as ineligible instead and
    // carry on with the next job.
    let mut tier = 1;
    let tier2 = job.tier2;
    let compiled = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
      // The optimizing tier declines functions it cannot build; those
      // still get the baseline tier's code.
      if let Some(feedback) = &tier2 {
        match engine.compile_tier2(proto, feedback, Some(&shutdown)) {
          Ok(done) => {
            tier = 2;
            return Ok(done);
          },
          Err(reason) => {
            if crate::jit::log_enabled() {
              eprintln!(
                "[jit] '{}' stays in the baseline tier: {reason}",
                proto.display_name()
              );
            }
          },
        }
      }
      engine.compile_function(
        proto,
        job.speculative_params,
        job.speculative_regs,
        job.facts,
        Some(&shutdown),
      )
    }));
    let (outcome, osr_ids) = match compiled {
      Ok(Ok((entry, osr_ids))) => (Ok(entry), osr_ids),
      Ok(Err(e)) => (Err(e), FxHashMap::default()),
      Err(payload) => {
        engine.recover_from_panic();
        let reason = payload
          .downcast_ref::<&str>()
          .map(|s| s.to_string())
          .or_else(|| payload.downcast_ref::<String>().cloned())
          .unwrap_or_else(|| "unknown panic".to_string());
        (Err(format!("compiler panicked: {reason}")), FxHashMap::default())
      },
    };

    if shutdown.load(Ordering::Relaxed) {
      return;
    }

    let result = CompileResult {
      proto: job.proto,
      osr_ids,
      speculative_params: job.speculative_params,
      speculative_regs: job.speculative_regs,
      speculative_lists,
      speculative_ints,
      tier,
      outcome,
    };
    if let Some((reply_tx, reply_pending)) = job.reply_to {
      if reply_tx.send(result).is_ok() {
        reply_pending.store(true, Ordering::Release);
      }
    } else {
      if result_tx.send(result).is_err() {
        return;
      }
      results_pending.store(true, Ordering::Release);
    }
  }
}
