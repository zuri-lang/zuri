//! The coroutine pool: a small, configurable number of persistent
//! worker OS threads, each owning its own totally independent `VM`/
//! `Heap` ("isolate"). A coroutine is a task queued onto this pool; a
//! channel is a plain thread-safe queue of already-`capture`d
//! messages. Nothing here ever shares a `Value`, a heap pointer, or
//! compiled bytecode between threads -- see `transfer` for what
//! actually crosses, and why that's the only thing that safely can.

use std::any::Any;
use std::cell::RefCell;
use std::collections::VecDeque;
use std::panic::{self, AssertUnwindSafe};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, OnceLock, PoisonError};
use std::thread;
use std::time::{Duration, Instant};

use crate::vm::object::Heap;
use crate::vm::value::Value;
use crate::vm::vm::VM;

use super::transfer::{self, TransferGraph};

/// `mutex.lock().unwrap()`, but tolerant of poisoning: a panic while
/// SOME OTHER thread held this exact lock (never expected in ordinary
/// operation, but possible if a bug elsewhere manages to panic while
/// touching shared pool/channel/coroutine state directly, as opposed
/// to inside a coroutine's own isolated VM -- see `worker_loop`'s own
/// docs on why THAT kind of panic is handled separately) doesn't
/// cascade into every future access panicking too. The guarded data
/// here is always a plain queue/slot/flag with no invariant that a
/// half-finished mutation could violate in a way that matters, so
/// recovering it is safe.
fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
  m.lock().unwrap_or_else(PoisonError::into_inner)
}

const MAX_POOL_SIZE: usize = 4096;

/// `Ptr::type_name` a `Coroutine`/`Channel` handle is tagged with --
/// shared between `coroutine.rs` (which allocates these) and
/// `transfer.rs` (which needs to recognize them as thread-safe,
/// freely-shareable handles rather than exclusive resources to move --
/// see `transfer::capture_value`'s own docs on the distinction).
pub const COROUTINE_PTR_TYPE: &str = "zuri_coroutine";
pub const CHANNEL_PTR_TYPE: &str = "zuri_channel";

// ---------------------------------------------------------------------
// Pool sizing/lifecycle
// ---------------------------------------------------------------------

static POOL: OnceLock<CoroutinePool> = OnceLock::new();
static CONFIGURED_SIZE: Mutex<Option<usize>> = Mutex::new(None);

/// Sets how many worker threads the pool starts with. Only takes
/// effect if the pool hasn't started yet (its size is fixed for the
/// rest of the process once the first coroutine actually runs) --
/// returns `false` rather than an error in that case, since "someone
/// already spawned something" isn't really exceptional, just too
/// late.
pub fn configure(n: usize) -> Result<bool, String> {
  if n == 0 || n > MAX_POOL_SIZE {
    return Err(format!(
      "coroutine pool size must be between 1 and {}, got {}",
      MAX_POOL_SIZE, n
    ));
  }
  if POOL.get().is_some() {
    return Ok(false);
  }
  let mut configured = lock(&CONFIGURED_SIZE);
  if POOL.get().is_some() {
    return Ok(false);
  }
  *configured = Some(n);
  Ok(true)
}

fn default_size() -> usize {
  thread::available_parallelism().map(|n| n.get()).unwrap_or(4)
}

/// The number of logical CPUs this machine reports -- informational
/// only, doesn't start the pool.
pub fn cpu_count() -> usize {
  default_size()
}

/// The pool's real worker count. Starts the pool (with whatever size
/// `configure` set, or the CPU count otherwise) if it hasn't already.
pub fn pool_size() -> usize {
  pool().size
}

/// Coroutines actively being run by a worker RIGHT NOW -- doesn't
/// include ones still waiting in the queue. Starts the pool if it
/// hasn't already (there's nothing running on a pool that was never
/// started).
pub fn active_count() -> usize {
  pool().running.load(Ordering::Acquire)
}

/// Coroutines queued but not yet picked up by a worker. Starts the
/// pool if it hasn't already.
pub fn queued_count() -> usize {
  lock(&pool().queue).len()
}

/// Whether `shutdown()` has been called. Starts the pool if it hasn't
/// already -- consistent with every other pool-state query here, and
/// harmless: a pool nobody has used yet obviously isn't shut down.
pub fn is_shutdown() -> bool {
  pool().shutting_down.load(Ordering::Acquire)
}

fn pool() -> &'static CoroutinePool {
  POOL.get_or_init(|| {
    let size = lock(&CONFIGURED_SIZE).unwrap_or_else(default_size);
    CoroutinePool::start(size)
  })
}

struct CoroutinePool {
  queue: Mutex<VecDeque<Task>>,
  not_empty: Condvar,
  size: usize,
  /// Queued-or-running task count, kept for `shutdown()` to know when
  /// there's genuinely nothing left in flight. Incremented in `spawn`,
  /// decremented once a task's `finish()` has actually run (success,
  /// ordinary failure, or a caught panic all count).
  in_flight: AtomicUsize,
  /// Coroutines a worker has actually picked up and is currently
  /// running -- the subset of `in_flight` that isn't still sitting in
  /// `queue`. Purely for introspection (`active_count()`).
  running: AtomicUsize,
  /// Set by `shutdown()`, checked by `spawn()`. One-way: once a pool
  /// starts shutting down it never accepts work again for the rest of
  /// the process.
  shutting_down: AtomicBool,
  /// Notified whenever `in_flight` changes, so `shutdown()` can block
  /// on it rather than polling. Paired with `idle_lock` purely for the
  /// `Condvar` API -- there's no real data to protect, `in_flight`
  /// already is atomic.
  idle: Condvar,
  idle_lock: Mutex<()>,
}

/// Worker thread names share this prefix -- checked by the panic hook
/// below to tell a fully-handled worker panic apart from a real,
/// nowhere-else-caught one on any other thread.
const WORKER_THREAD_PREFIX: &str = "zuri-coroutine-";

impl CoroutinePool {
  fn start(size: usize) -> Self {
    install_worker_panic_hook();
    for i in 0..size {
      thread::Builder::new()
        .name(format!("{}{}", WORKER_THREAD_PREFIX, i))
        .spawn(worker_loop)
        .expect("failed to spawn coroutine worker thread");
    }
    CoroutinePool {
      queue: Mutex::new(VecDeque::new()),
      not_empty: Condvar::new(),
      size,
      in_flight: AtomicUsize::new(0),
      running: AtomicUsize::new(0),
      shutting_down: AtomicBool::new(false),
      idle: Condvar::new(),
      idle_lock: Mutex::new(()),
    }
  }

  /// Notifies whoever's in `shutdown()` waiting on `in_flight` to
  /// reach zero. Momentarily taking `idle_lock` before notifying,
  /// rather than just calling `notify_all`, avoids the same
  /// lost-wakeup window `wake_all_waiters` guards against -- see its
  /// own docs.
  fn task_completed(&self) {
    self.in_flight.fetch_sub(1, Ordering::AcqRel);
    self.running.fetch_sub(1, Ordering::AcqRel);
    drop(lock(&self.idle_lock));
    self.idle.notify_all();
  }
}

/// A worker panic is always caught by `catch_unwind` in `worker_loop`
/// and surfaced to Zuri as an ordinary `CoroutineError` -- it was
/// never actually a crash. Printing Rust's own default "thread ...
/// panicked at ..." notice for one anyway would look exactly like an
/// unhandled crash to anyone watching stderr, which is actively
/// misleading for something the pool fully recovered from. This
/// installs a hook that skips the default report for worker threads
/// specifically and defers to whatever hook was already installed
/// (Rust's own default, unless something else replaced it first) for
/// every other thread, main included -- a REAL uncaught panic
/// anywhere else still gets reported exactly as before.
fn install_worker_panic_hook() {
  static INSTALLED: std::sync::Once = std::sync::Once::new();
  INSTALLED.call_once(|| {
    let previous = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
      let is_worker = thread::current()
        .name()
        .is_some_and(|n| n.starts_with(WORKER_THREAD_PREFIX));
      if !is_worker {
        previous(info);
      }
    }));
  });
}

/// One pending call: "run this named, non-capturing function/method
/// with these arguments" -- see `transfer::Home`/`NamedKind` for how
/// the callee is identified without ever moving bytecode.
struct Task {
  callee: TransferGraph,
  args: TransferGraph,
  state: Arc<CoroutineState>,
}

/// A worker thread's own isolate: one `VM`/`Heap`, built once and
/// reused for every task this thread ever picks up -- loading a
/// task's home (see `transfer::Home`) is cached per-isolate, so only
/// the very first task from a given module/entry script pays to
/// compile and run it.
struct WorkerIsolate {
  vm: VM,
}

impl WorkerIsolate {
  fn new() -> Self {
    let mut vm = VM::new(Heap::new());
    vm.init();
    WorkerIsolate { vm }
  }
}

fn worker_loop() {
  let mut isolate = WorkerIsolate::new();
  let pool = pool();
  loop {
    let task = {
      let mut queue = lock(&pool.queue);
      while queue.is_empty() {
        queue = pool.not_empty.wait(queue).unwrap_or_else(PoisonError::into_inner);
      }
      queue.pop_front().unwrap()
    };
    pool.running.fetch_add(1, Ordering::AcqRel);

    // Cloned out BEFORE `run_task` runs, not read off `task` afterward:
    // a caught panic (see below) means `task` may never come back from
    // that call in any usable form, but the coroutine's own result
    // slot still needs to be resolved either way.
    let state = task.state.clone();

    // Ambient for the DURATION of this one task -- `is_current_cancelled`
    // reads it back with no explicit handle needed, the same way a
    // spawned function never has to be handed its own `Coroutine` back
    // just to ask "was I cancelled?". Cleared unconditionally
    // afterward (both the Ok and panic arms below), never left
    // pointing at a finished task's state while this thread picks up
    // its next one.
    CURRENT_COROUTINE.with(|c| *c.borrow_mut() = Some(state.clone()));

    // `catch_unwind` isolates a panic to just the ONE coroutine that
    // caused it, rather than taking down every other coroutine and the
    // main thread with it -- see `Cargo.toml`'s own note on why
    // `panic = "abort"` had to go for this to even be possible.
    // `AssertUnwindSafe` because `&mut isolate.vm` isn't provably
    // unwind-safe on its own (a panic mid-mutation could leave its
    // internal state -- registers, GC bookkeeping -- torn); the promise
    // that makes this sound is the one kept right below: a torn
    // isolate is never reused, only rebuilt from scratch.
    match panic::catch_unwind(AssertUnwindSafe(|| run_task(&mut isolate, &task))) {
      Ok(result) => state.finish(result),
      Err(payload) => {
        // `&*payload`, not `&payload`: `payload` is `Box<dyn Any +
        // Send>`, and `Box<dyn Any + Send>` itself implements `Any`
        // (it's `'static` too) -- a bare `&payload` coerces to `&dyn
        // Any` by treating the BOX ITSELF as the trait object, not by
        // dereferencing into what it holds, so `downcast_ref` would
        // always be asking "is the payload literally a `Box`", never
        // "what's inside it". The explicit deref forces the reference
        // at the actual panic value instead.
        state.finish(Err(format!(
          "coroutine panicked: {}",
          panic_message(&*payload)
        )));
        isolate = WorkerIsolate::new();
      },
    }
    CURRENT_COROUTINE.with(|c| *c.borrow_mut() = None);
    pool.task_completed();
  }
}

thread_local! {
  /// The coroutine THIS worker thread is currently running, if any --
  /// what lets `is_current_cancelled` answer "was I cancelled?" with
  /// no explicit handle passed in, the same way each worker's own
  /// isolate needs no explicit parameter either. Set/cleared around
  /// each task in `worker_loop`; `None` between tasks and on any
  /// thread that isn't a coroutine worker at all.
  static CURRENT_COROUTINE: RefCell<Option<Arc<CoroutineState>>> = const { RefCell::new(None) };
}

/// Whether the coroutine currently running ON THIS THREAD has been
/// `cancel()`ed. `false` (never `true`) on a thread that isn't a
/// coroutine worker, or between tasks on one that is -- there's
/// nothing to have been cancelled either way.
pub fn is_current_cancelled() -> bool {
  CURRENT_COROUTINE.with(|c| c.borrow().as_ref().is_some_and(|s| s.is_cancelled()))
}

// ---------------------------------------------------------------------
// wait_any / select
// ---------------------------------------------------------------------

/// Shared wakeup signal for `wait_any_coroutines`/`select_channels`. A
/// coroutine finishing or a channel changing has no way to know in
/// advance whether one of THESE calls happens to be waiting on it, so
/// rather than each `CoroutineState`/`ChannelState` tracking its own
/// list of interested waiters, every such event just notifies this one
/// condvar and a waiter re-scans its own (always small) candidate list
/// each time it wakes. Simpler and just as correct as per-object
/// waiter bookkeeping, at the cost of a wider wakeup fan-out that
/// doesn't matter at this scale.
fn wake_gate() -> &'static (Mutex<()>, Condvar) {
  static GATE: OnceLock<(Mutex<()>, Condvar)> = OnceLock::new();
  GATE.get_or_init(|| (Mutex::new(()), Condvar::new()))
}

/// Called after any state change a `wait_any`/`select` predicate might
/// depend on (a coroutine finishing, a channel gaining a value or
/// closing). Momentarily taking the gate's mutex before notifying --
/// rather than just calling `notify_all` -- is what avoids a lost
/// wakeup: it guarantees this can't land in the gap between a waiter's
/// last check and the moment it actually starts waiting on the
/// condvar, which is the usual race for a condvar guarding a predicate
/// that lives in a mutex OTHER than the one it waits on.
fn wake_all_waiters() {
  let (m, cv) = wake_gate();
  drop(lock(m));
  cv.notify_all();
}

/// Blocks until at least one of `states` has finished, returning its
/// index into the slice -- ties (more than one already done) resolve
/// to whichever comes first in the caller's own list. Gives up and
/// returns `None` once `timeout` elapses with none ready.
pub fn wait_any_coroutines(states: &[Arc<CoroutineState>], timeout: Option<Duration>) -> Option<usize> {
  let deadline = timeout.map(|d| Instant::now() + d);
  let (m, cv) = wake_gate();
  let mut guard = lock(m);
  loop {
    if let Some(i) = states.iter().position(|s| s.is_done()) {
      return Some(i);
    }
    guard = match deadline.map(|dl| dl.saturating_duration_since(Instant::now())) {
      Some(d) if d.is_zero() => return None,
      Some(d) => {
        let (g, result) = cv.wait_timeout(guard, d).unwrap_or_else(PoisonError::into_inner);
        if result.timed_out() {
          return states.iter().position(|s| s.is_done());
        }
        g
      },
      None => cv.wait(guard).unwrap_or_else(PoisonError::into_inner),
    };
  }
}

/// Blocks until EVERY one of `states` has finished, returning `true`.
/// Gives up early and returns `false` if `timeout` elapses first with
/// at least one still pending. Unlike `wait_any_coroutines` there's no
/// "which one" to report -- the caller already has the whole list and
/// can `join()` each once this returns `true`.
pub fn wait_all_coroutines(states: &[Arc<CoroutineState>], timeout: Option<Duration>) -> bool {
  let deadline = timeout.map(|d| Instant::now() + d);
  let (m, cv) = wake_gate();
  let mut guard = lock(m);
  loop {
    if states.iter().all(|s| s.is_done()) {
      return true;
    }
    guard = match deadline.map(|dl| dl.saturating_duration_since(Instant::now())) {
      Some(d) if d.is_zero() => return states.iter().all(|s| s.is_done()),
      Some(d) => {
        let (g, result) = cv.wait_timeout(guard, d).unwrap_or_else(PoisonError::into_inner);
        if result.timed_out() {
          return states.iter().all(|s| s.is_done());
        }
        g
      },
      None => cv.wait(guard).unwrap_or_else(PoisonError::into_inner),
    };
  }
}

/// Blocks until at least one of `states` (channels) has a value ready
/// to receive or is closed, returning its index and the outcome --
/// already taken off the winning channel's own queue, same as
/// `try_recv`. Same ordering/timeout behavior as
/// `wait_any_coroutines`.
pub fn select_channels(
  states: &[Arc<ChannelState>],
  timeout: Option<Duration>,
) -> Option<(usize, RecvOutcome)> {
  let deadline = timeout.map(|d| Instant::now() + d);
  let (m, cv) = wake_gate();
  let mut guard = lock(m);
  loop {
    for (i, s) in states.iter().enumerate() {
      if let Some(outcome) = s.try_recv() {
        return Some((i, outcome));
      }
    }
    guard = match deadline.map(|dl| dl.saturating_duration_since(Instant::now())) {
      Some(d) if d.is_zero() => return None,
      Some(d) => {
        let (g, result) = cv.wait_timeout(guard, d).unwrap_or_else(PoisonError::into_inner);
        if result.timed_out() {
          for (i, s) in states.iter().enumerate() {
            if let Some(outcome) = s.try_recv() {
              return Some((i, outcome));
            }
          }
          return None;
        }
        g
      },
      None => cv.wait(guard).unwrap_or_else(PoisonError::into_inner),
    };
  }
}

fn panic_message(payload: &(dyn Any + Send)) -> String {
  if let Some(s) = payload.downcast_ref::<&str>() {
    (*s).to_string()
  } else if let Some(s) = payload.downcast_ref::<String>() {
    s.clone()
  } else {
    "unknown panic payload".to_string()
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  /// A panic isn't caught anywhere in this module without going
  /// through `worker_loop`'s full task-queue/isolate machinery, so
  /// this exercises the actual mechanism (`catch_unwind` plus the
  /// `&*payload` deref -- see that call site's own docs on why a bare
  /// `&payload` silently reads the wrong thing) directly, without
  /// needing a real `VM`/`Task`/pool.
  #[test]
  fn panic_message_reads_a_plain_string_literal_panic() {
    let result = panic::catch_unwind(|| panic!("boom"));
    let payload = result.expect_err("closure was supposed to panic");
    assert_eq!(panic_message(&*payload), "boom");
  }

  #[test]
  fn panic_message_reads_a_formatted_panic() {
    let n = 42;
    let result = panic::catch_unwind(|| panic!("boom: {n}"));
    let payload = result.expect_err("closure was supposed to panic");
    assert_eq!(panic_message(&*payload), "boom: 42");
  }

  /// The specific bug this whole helper exists to avoid: a bare
  /// `&payload` (no explicit deref) coerces `Box<dyn Any + Send>`
  /// itself into the trait object, since the box is `Any` too --
  /// every downcast then silently fails no matter what's inside it,
  /// falling through to "unknown panic payload" instead of the real
  /// message. This pins that failure mode down so it can't regress
  /// unnoticed.
  #[test]
  fn panic_message_via_bare_reference_loses_the_message() {
    let result = panic::catch_unwind(|| panic!("boom"));
    let payload = result.expect_err("closure was supposed to panic");
    assert_eq!(panic_message(&payload), "unknown panic payload");
  }
}

fn run_task(isolate: &mut WorkerIsolate, task: &Task) -> Result<TransferGraph, String> {
  let callee = transfer::materialize(&mut isolate.vm, &task.callee)?;
  // `callee` sits only in this local until `call_value` copies it
  // into a register -- pin it across `args`' own materialize call,
  // which can itself allocate (and therefore collect).
  let pin = isolate.vm.pin_values([callee]);
  let args_val = transfer::materialize(&mut isolate.vm, &task.args)?;
  let callee = isolate.vm.pinned(pin);
  isolate.vm.unpin(pin);

  let args = args_val.as_list();

  // `VM::call_value` only ever accepts a closure or a native, never
  // a bare `ObjBoundMethod` -- so `instance.method` used directly as
  // a spawn target needs its receiver spliced back in as arg 0
  // (exactly what `Instr::Invoke` already does for an ordinary
  // `instance.method(...)` call site; this is that same convention,
  // just applied by hand since there's no such instruction here).
  let (target, full_args) = if callee.is_bound_method() {
    let bm = callee.as_bound_method();
    if !bm.method.is_closure() {
      return Err("coroutine spawn target is not a callable function".to_string());
    }
    let mut full = Vec::with_capacity(args.len() + 1);
    full.push(bm.receiver);
    full.extend(args);
    (bm.method, full)
  } else if callee.is_closure() {
    (callee, args)
  } else {
    return Err("coroutine spawn target is not a callable function".to_string());
  };

  match isolate.vm.call_value(target, &full_args) {
    Ok(ret) => transfer::capture(&isolate.vm, ret),
    Err(exc) => Err(isolate.vm.describe_exception(exc)),
  }
}

// ---------------------------------------------------------------------
// Coroutine handles
// ---------------------------------------------------------------------

enum Slot {
  Pending,
  Ok(TransferGraph),
  /// A rendered, human-readable failure -- either the spawned
  /// function's own uncaught exception (see `VM::describe_exception`)
  /// or an infrastructure failure (bad spawn target, a value that
  /// couldn't cross the isolate boundary, ...). Deliberately not the
  /// original exception `Value` itself: that `Value` lives on the
  /// WORKER's own heap and can't be handed back across the thread
  /// boundary any more than any other `Value` can -- see `transfer`'s
  /// own docs. `libs/coroutine.zu` wraps this text in its own
  /// `CoroutineError` on `.join()`.
  Err(String),
}

pub enum JoinOutcome {
  Pending,
  Ok(TransferGraph),
  Err(String),
}

pub struct CoroutineState {
  slot: Mutex<Slot>,
  cv: Condvar,
  /// Set once `join()`/`try_join()` has actually reported a finished
  /// outcome (`Ok` or `Err`) to someone -- see `Drop`'s own docs.
  observed: AtomicBool,
  /// Set by `cancel()`, read by `is_current_cancelled()` from inside
  /// the coroutine's own execution -- purely COOPERATIVE, same as
  /// every other language's cancellation token: nothing here stops
  /// already-running code on its own. A coroutine that never checks
  /// simply runs to completion regardless of this flag.
  cancelled: AtomicBool,
}

impl CoroutineState {
  fn new() -> Self {
    CoroutineState {
      slot: Mutex::new(Slot::Pending),
      cv: Condvar::new(),
      observed: AtomicBool::new(false),
      cancelled: AtomicBool::new(false),
    }
  }

  pub fn cancel(&self) {
    self.cancelled.store(true, Ordering::Relaxed);
  }

  pub fn is_cancelled(&self) -> bool {
    self.cancelled.load(Ordering::Relaxed)
  }

  fn finish(&self, result: Result<TransferGraph, String>) {
    let mut slot = lock(&self.slot);
    *slot = match result {
      Ok(g) => Slot::Ok(g),
      Err(m) => Slot::Err(m),
    };
    drop(slot);
    self.cv.notify_all();
    wake_all_waiters();
  }

  /// Blocks the calling thread until the coroutine finishes. Callable
  /// more than once (and from more than one joiner) -- always returns
  /// the same, already-computed outcome once it's in.
  pub fn join(&self) -> JoinOutcome {
    let mut slot = lock(&self.slot);
    loop {
      match &*slot {
        Slot::Pending => slot = self.cv.wait(slot).unwrap_or_else(PoisonError::into_inner),
        Slot::Ok(g) => {
          self.observed.store(true, Ordering::Relaxed);
          return JoinOutcome::Ok(g.clone());
        },
        Slot::Err(m) => {
          self.observed.store(true, Ordering::Relaxed);
          return JoinOutcome::Err(m.clone());
        },
      }
    }
  }

  /// Like `join()`, but gives up and returns `JoinOutcome::Pending`
  /// (indistinguishable from "still running" -- from the caller's own
  /// point of view, that's exactly what a timeout means) if `timeout`
  /// elapses first. Never dropped early by a spurious wakeup:
  /// `wait_timeout_while` re-checks the predicate itself in a loop.
  pub fn join_timeout(&self, timeout: Duration) -> JoinOutcome {
    let slot = lock(&self.slot);
    let (slot, result) = self
      .cv
      .wait_timeout_while(slot, timeout, |s| matches!(s, Slot::Pending))
      .unwrap_or_else(PoisonError::into_inner);
    if result.timed_out() {
      return JoinOutcome::Pending;
    }
    match &*slot {
      Slot::Pending => JoinOutcome::Pending,
      Slot::Ok(g) => {
        self.observed.store(true, Ordering::Relaxed);
        JoinOutcome::Ok(g.clone())
      },
      Slot::Err(m) => {
        self.observed.store(true, Ordering::Relaxed);
        JoinOutcome::Err(m.clone())
      },
    }
  }

  pub fn try_join(&self) -> JoinOutcome {
    match &*lock(&self.slot) {
      Slot::Pending => JoinOutcome::Pending,
      Slot::Ok(g) => {
        self.observed.store(true, Ordering::Relaxed);
        JoinOutcome::Ok(g.clone())
      },
      Slot::Err(m) => {
        self.observed.store(true, Ordering::Relaxed);
        JoinOutcome::Err(m.clone())
      },
    }
  }

  pub fn is_done(&self) -> bool {
    !matches!(&*lock(&self.slot), Slot::Pending)
  }

  /// Like `try_join`, but never marks the outcome "observed" -- pure
  /// introspection for `Coroutine.status()`. `try_join`/`join`
  /// themselves double as "I've seen this failure, don't warn about
  /// it going unhandled" (see `Drop`'s own docs); a caller just
  /// checking progress shouldn't accidentally suppress that warning
  /// for a failure it never actually looked at.
  pub fn peek(&self) -> JoinOutcome {
    match &*lock(&self.slot) {
      Slot::Pending => JoinOutcome::Pending,
      Slot::Ok(g) => JoinOutcome::Ok(g.clone()),
      Slot::Err(m) => JoinOutcome::Err(m.clone()),
    }
  }
}

impl Drop for CoroutineState {
  /// A coroutine's failure doesn't otherwise go anywhere unless
  /// something calls `join()`/`try_join()` on it -- exactly like a
  /// plain `std::thread` whose `JoinHandle` is dropped without ever
  /// being joined, an uncaught exception or a caught panic inside a
  /// fire-and-forget `spawn()` would silently vanish once the last
  /// `Coroutine` handle (and the pool's own internal one) goes out of
  /// scope. A dropped `String` costs nothing to check for and losing
  /// a real failure silently is worse than one unwanted log line, so
  /// this reports it -- the same trade-off Rust's own default panic
  /// hook already makes for an unjoined thread.
  fn drop(&mut self) {
    if self.observed.load(Ordering::Relaxed) {
      return;
    }
    if let Slot::Err(message) = &*self.slot.get_mut().unwrap_or_else(PoisonError::into_inner) {
      eprintln!(
        "warning: a coroutine failed but its result was never checked \
         (no join()/try_join() was called before its handle was dropped): {}",
        message
      );
    }
  }
}

/// Captures `callee`/`args` (on the CALLING isolate's own heap, via
/// `vm`) and queues them for the pool to pick up. `args` is expected
/// to be a Zuri list Value; capturing it as one graph is what makes
/// aliasing BETWEEN arguments (two args pointing at the same nested
/// list, say) survive the trip along with aliasing within each one.
pub fn spawn(vm: &VM, callee: Value, args: Value) -> Result<Arc<CoroutineState>, String> {
  let callee = transfer::capture(vm, callee)?;
  let args = transfer::capture(vm, args)?;
  let state = Arc::new(CoroutineState::new());
  let task = Task {
    callee,
    args,
    state: state.clone(),
  };
  let p = pool();
  {
    // Checking `shutting_down` and enqueuing under the SAME lock is
    // what makes this race-free against a concurrent `shutdown()`:
    // either this sees the flag already set and bails out, or
    // `shutdown()` hasn't set it yet and this task is safely counted
    // in `in_flight` before shutdown can start waiting for zero.
    let mut queue = lock(&p.queue);
    if p.shutting_down.load(Ordering::Acquire) {
      return Err("cannot spawn: the coroutine pool is shutting down".to_string());
    }
    p.in_flight.fetch_add(1, Ordering::AcqRel);
    queue.push_back(task);
  }
  p.not_empty.notify_one();
  Ok(state)
}

/// Stops the pool from accepting any further `spawn()` calls, then
/// blocks until every task already queued or running has finished --
/// nothing in flight is abandoned. One-way: once this returns (or even
/// while it's still waiting), `spawn()` keeps failing for the rest of
/// the process.
///
/// Returns `false` rather than blocking forever if `timeout` elapses
/// with work still outstanding.
pub fn shutdown(timeout: Option<Duration>) -> bool {
  let p = pool();
  {
    let _queue = lock(&p.queue);
    p.shutting_down.store(true, Ordering::Release);
  }
  let deadline = timeout.map(|d| Instant::now() + d);
  let mut guard = lock(&p.idle_lock);
  loop {
    if p.in_flight.load(Ordering::Acquire) == 0 {
      return true;
    }
    guard = match deadline.map(|dl| dl.saturating_duration_since(Instant::now())) {
      Some(d) if d.is_zero() => return p.in_flight.load(Ordering::Acquire) == 0,
      Some(d) => {
        let (g, result) = p.idle.wait_timeout(guard, d).unwrap_or_else(PoisonError::into_inner);
        if result.timed_out() {
          return p.in_flight.load(Ordering::Acquire) == 0;
        }
        g
      },
      None => p.idle.wait(guard).unwrap_or_else(PoisonError::into_inner),
    };
  }
}

// ---------------------------------------------------------------------
// Channels
// ---------------------------------------------------------------------

pub enum RecvOutcome {
  Value(TransferGraph),
  Closed,
}

pub enum SendOutcome {
  Sent,
  Closed,
  TimedOut,
}

struct ChannelInner {
  queue: VecDeque<TransferGraph>,
  /// `None` = unbounded.
  capacity: Option<usize>,
  closed: bool,
}

pub struct ChannelState {
  inner: Mutex<ChannelInner>,
  not_empty: Condvar,
  not_full: Condvar,
}

impl ChannelState {
  pub fn new(capacity: Option<usize>) -> Self {
    ChannelState {
      inner: Mutex::new(ChannelInner {
        queue: VecDeque::new(),
        capacity,
        closed: false,
      }),
      not_empty: Condvar::new(),
      not_full: Condvar::new(),
    }
  }

  /// Blocks while the channel is at capacity. `Err` only for "the
  /// channel is closed" -- the caller's own native wrapper turns that
  /// into whatever's idiomatic on the Zuri side.
  pub fn send(&self, value: TransferGraph) -> Result<(), ()> {
    let mut inner = lock(&self.inner);
    loop {
      if inner.closed {
        return Err(());
      }
      let full = inner.capacity.is_some_and(|cap| inner.queue.len() >= cap);
      if !full {
        break;
      }
      inner = self.not_full.wait(inner).unwrap_or_else(PoisonError::into_inner);
    }
    inner.queue.push_back(value);
    drop(inner);
    self.not_empty.notify_one();
    wake_all_waiters();
    Ok(())
  }

  /// Like `send`, but gives up (returning `TimedOut`) if `timeout`
  /// elapses before the channel has room.
  pub fn send_timeout(&self, value: TransferGraph, timeout: Duration) -> SendOutcome {
    let inner = lock(&self.inner);
    let (mut inner, result) = self
      .not_full
      .wait_timeout_while(inner, timeout, |inner| {
        !inner.closed && inner.capacity.is_some_and(|cap| inner.queue.len() >= cap)
      })
      .unwrap_or_else(PoisonError::into_inner);
    if inner.closed {
      return SendOutcome::Closed;
    }
    if result.timed_out() {
      return SendOutcome::TimedOut;
    }
    inner.queue.push_back(value);
    drop(inner);
    self.not_empty.notify_one();
    wake_all_waiters();
    SendOutcome::Sent
  }

  /// Blocks until a message arrives or the channel is closed AND
  /// drained.
  pub fn recv(&self) -> RecvOutcome {
    let mut inner = lock(&self.inner);
    loop {
      if let Some(v) = inner.queue.pop_front() {
        drop(inner);
        self.not_full.notify_one();
        return RecvOutcome::Value(v);
      }
      if inner.closed {
        return RecvOutcome::Closed;
      }
      inner = self.not_empty.wait(inner).unwrap_or_else(PoisonError::into_inner);
    }
  }

  /// Like `recv`, but gives up (returning `None`) if `timeout` elapses
  /// first with nothing to receive and the channel still open.
  pub fn recv_timeout(&self, timeout: Duration) -> Option<RecvOutcome> {
    let inner = lock(&self.inner);
    let (mut inner, _result) = self
      .not_empty
      .wait_timeout_while(inner, timeout, |inner| inner.queue.is_empty() && !inner.closed)
      .unwrap_or_else(PoisonError::into_inner);
    if let Some(v) = inner.queue.pop_front() {
      drop(inner);
      self.not_full.notify_one();
      return Some(RecvOutcome::Value(v));
    }
    if inner.closed {
      return Some(RecvOutcome::Closed);
    }
    None
  }

  /// `None` means "empty, but still open" -- the one outcome `recv`
  /// never produces, since it would just keep waiting instead.
  pub fn try_recv(&self) -> Option<RecvOutcome> {
    let mut inner = lock(&self.inner);
    if let Some(v) = inner.queue.pop_front() {
      drop(inner);
      self.not_full.notify_one();
      return Some(RecvOutcome::Value(v));
    }
    if inner.closed {
      return Some(RecvOutcome::Closed);
    }
    None
  }

  pub fn close(&self) {
    let mut inner = lock(&self.inner);
    inner.closed = true;
    drop(inner);
    self.not_empty.notify_all();
    self.not_full.notify_all();
    wake_all_waiters();
  }

  pub fn is_closed(&self) -> bool {
    lock(&self.inner).closed
  }

  pub fn len(&self) -> usize {
    lock(&self.inner).queue.len()
  }
}
