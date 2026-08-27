//! The isolate pool: a small, configurable number of persistent
//! isolate OS threads, each owning its own totally independent `VM`/
//! `Heap` ("isolate"). A isolate is a task queued onto this pool; a
//! channel is a plain thread-safe queue of already-`capture`d
//! messages. Nothing here ever shares a `Value`, a heap pointer, or
//! compiled bytecode between threads: see `transfer` for what
//! actually crosses, and why that's the only thing that safely can.

use std::any::Any;
use std::cell::RefCell;
use std::collections::VecDeque;
use std::panic::{self, AssertUnwindSafe};
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicUsize, Ordering};
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
/// touching shared pool/channel/isolate state directly, as opposed
/// to inside a isolate's own isolated VM: see `isolate_loop`'s own
/// docs on why THAT kind of panic is handled separately) doesn't
/// cascade into every future access panicking too. The guarded data
/// here is always a plain queue/slot/flag with no invariant that a
/// half-finished mutation could violate in a way that matters, so
/// recovering it is safe.
fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
  m.lock().unwrap_or_else(PoisonError::into_inner)
}

const MAX_POOL_SIZE: usize = 4096;

/// `Ptr::type_name` a `Isolate`/`Channel` handle is tagged with;
/// shared between `isolate.rs` (which allocates these) and
/// `transfer.rs` (which needs to recognize them as thread-safe,
/// freely-shareable handles rather than exclusive resources to move:
/// see `transfer::capture_value`'s own docs on the distinction).
pub const ISOLATE_PTR_TYPE: &str = "zuri::isolate";
pub const CHANNEL_PTR_TYPE: &str = "zuri::channel";

// ---------------------------------------------------------------------
// Pool sizing/lifecycle
// ---------------------------------------------------------------------

static POOL: OnceLock<IsolatePool> = OnceLock::new();
static CONFIGURED_SIZE: Mutex<Option<usize>> = Mutex::new(None);

/// Sets how many isolate threads the pool starts with. Only takes
/// effect if the pool hasn't started yet (its size is fixed for the
/// rest of the process once the first isolate actually runs);
/// returns `false` rather than an error in that case, since "someone
/// already spawned something" isn't really exceptional, just too
/// late.
pub fn configure(n: usize) -> Result<bool, String> {
  if n == 0 || n > MAX_POOL_SIZE {
    return Err(format!(
      "isolate pool size must be between 1 and {}, got {}",
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
  thread::available_parallelism()
    .map(|n| n.get())
    .unwrap_or(4)
}

/// The number of logical CPUs this machine reports; informational
/// only, doesn't start the pool.
pub fn cpu_count() -> usize {
  default_size()
}

/// The pool's real isolate count. Starts the pool (with whatever size
/// `configure` set, or the CPU count otherwise) if it hasn't already.
pub fn pool_size() -> usize {
  pool().size
}

/// Isolates actively being run by a isolate RIGHT NOW; doesn't
/// include ones still waiting in the queue. Starts the pool if it
/// hasn't already (there's nothing running on a pool that was never
/// started).
pub fn active_count() -> usize {
  pool().running.load(Ordering::Acquire)
}

/// Isolates queued but not yet picked up by a isolate. Starts the
/// pool if it hasn't already.
pub fn queued_count() -> usize {
  lock(&pool().queue).len()
}

/// Whether `shutdown()` has been called. Starts the pool if it hasn't
/// already; consistent with every other pool-state query here, and
/// harmless: a pool nobody has used yet obviously isn't shut down.
pub fn is_shutdown() -> bool {
  pool().shutting_down.load(Ordering::Acquire)
}

fn pool() -> &'static IsolatePool {
  POOL.get_or_init(|| {
    let size = lock(&CONFIGURED_SIZE).unwrap_or_else(default_size);
    IsolatePool::start(size)
  })
}

struct IsolateCompiler {
  job_tx: std::sync::mpsc::Sender<crate::jit::background::CompileJob>,
  shutdown: Arc<AtomicBool>,
  thread: Option<std::thread::JoinHandle<()>>,
}

struct IsolatePool {
  queue: Mutex<VecDeque<Task>>,
  not_empty: Condvar,
  size: usize,
  /// Queued-or-running task count, kept for `shutdown()` to know when
  /// there's genuinely nothing left in flight. Incremented in `spawn`,
  /// decremented once a task's `finish()` has actually run (success,
  /// ordinary failure, or a caught panic all count).
  in_flight: AtomicUsize,
  /// Isolates a isolate has actually picked up and is currently
  /// running; the subset of `in_flight` that isn't still sitting in
  /// `queue`. Purely for introspection (`active_count()`).
  running: AtomicUsize,
  /// Set by `shutdown()`, checked by `spawn()`. One-way: once a pool
  /// starts shutting down it never accepts work again for the rest of
  /// the process.
  shutting_down: AtomicBool,
  /// Notified whenever `in_flight` changes, so `shutdown()` can block
  /// on it rather than polling. Paired with `idle_lock` purely for the
  /// `Condvar` API; there's no real data to protect, `in_flight`
  /// already is atomic.
  idle: Condvar,
  idle_lock: Mutex<()>,
  compiler: Mutex<Option<IsolateCompiler>>,
}

/// Isolate thread names share this prefix; checked by the panic hook
/// below to tell a fully-handled isolate panic apart from a real,
/// nowhere-else-caught one on any other thread.
const ISOLATE_THREAD_PREFIX: &str = "zuri-isolate-";

impl IsolatePool {
  fn start(size: usize) -> Self {
    install_isolate_panic_hook();
    for i in 0..size {
      thread::Builder::new()
        .name(format!("{}{}", ISOLATE_THREAD_PREFIX, i))
        .stack_size(8 * 1024 * 1024)
        .spawn(isolate_loop)
        .expect("failed to spawn isolate isolate thread");
    }
    IsolatePool {
      queue: Mutex::new(VecDeque::new()),
      not_empty: Condvar::new(),
      size,
      in_flight: AtomicUsize::new(0),
      running: AtomicUsize::new(0),
      shutting_down: AtomicBool::new(false),
      idle: Condvar::new(),
      idle_lock: Mutex::new(()),
      compiler: Mutex::new(None),
    }
  }

  fn get_or_spawn_compiler(&self) -> std::sync::mpsc::Sender<crate::jit::background::CompileJob> {
    let mut comp = lock(&self.compiler);
    if let Some(c) = comp.as_ref() {
      return c.job_tx.clone();
    }
    let (job_tx, shutdown, thread) =
      crate::jit::background::spawn_named("zuri-jit-compiler-isolate".to_string());
    *comp = Some(IsolateCompiler {
      job_tx: job_tx.clone(),
      shutdown,
      thread: Some(thread),
    });
    job_tx
  }

  fn stop_compiler(&self) {
    let mut comp = lock(&self.compiler);
    if let Some(mut c) = comp.take() {
      c.shutdown.store(true, Ordering::Release);
      drop(c.job_tx);
      if let Some(thread) = c.thread.take() {
        let _ = thread.join();
      }
    }
  }

  /// Notifies whoever's in `shutdown()` waiting on `in_flight` to
  /// reach zero.
  fn task_completed(&self) {
    let prev = self.in_flight.fetch_sub(1, Ordering::AcqRel);
    self.running.fetch_sub(1, Ordering::AcqRel);
    if prev == 1 {
      self.stop_compiler();
    }
    if self.shutting_down.load(Ordering::Relaxed) && prev == 1 {
      drop(lock(&self.idle_lock));
      self.idle.notify_all();
    }
  }
}

/// A isolate panic is always caught by `catch_unwind` in `isolate_loop`
/// and surfaced to Zuri as an ordinary `IsolateError`; it was
/// never actually a crash. Printing Rust's own default "thread ...
/// panicked at ..." notice for one anyway would look exactly like an
/// unhandled crash to anyone watching stderr, which is actively
/// misleading for something the pool fully recovered from. This
/// installs a hook that skips the default report for isolate threads
/// specifically and defers to whatever hook was already installed
/// (Rust's own default, unless something else replaced it first) for
/// every other thread, main included; a REAL uncaught panic
/// anywhere else still gets reported exactly as before.
fn install_isolate_panic_hook() {
  static INSTALLED: std::sync::Once = std::sync::Once::new();
  INSTALLED.call_once(|| {
    let previous = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
      let is_isolate = thread::current()
        .name()
        .is_some_and(|n| n.starts_with(ISOLATE_THREAD_PREFIX));
      if !is_isolate {
        previous(info);
      }
    }));
  });
}

/// One pending call: "run this named, non-capturing function/method
/// with these arguments": see `transfer::Home`/`NamedKind` for how
/// the callee is identified without ever moving bytecode.
struct Task {
  callee: TransferGraph,
  args: TransferGraph,
  state: Arc<IsolateState>,
}

/// A isolate thread's own isolate: one `VM`/`Heap`, built once and
/// reused for every task this thread ever picks up; loading a
/// task's home (see `transfer::Home`) is cached per-isolate, so only
/// the very first task from a given module/entry script pays to
/// compile and run it.
struct IsolateIsolate {
  vm: VM,
}

impl IsolateIsolate {
  fn new() -> Self {
    let mut vm = VM::new(Heap::new());
    vm.init();
    IsolateIsolate { vm }
  }
}

fn isolate_loop() {
  let mut isolate = IsolateIsolate::new();
  let pool = pool();
  loop {
    let task = {
      let mut queue = lock(&pool.queue);
      while queue.is_empty() {
        queue = pool
          .not_empty
          .wait(queue)
          .unwrap_or_else(PoisonError::into_inner);
      }
      queue.pop_front().unwrap()
    };
    pool.running.fetch_add(1, Ordering::AcqRel);

    let compiler_tx = pool.get_or_spawn_compiler();
    isolate.vm.set_shared_jit_compiler(compiler_tx);

    // Cloned out BEFORE `run_task` runs, not read off `task` afterward:
    // a caught panic (see below) means `task` may never come back from
    // that call in any usable form, but the isolate's own result
    // slot still needs to be resolved either way.
    let state = task.state.clone();

    // Ambient for the DURATION of this one task; `is_current_cancelled`
    // reads it back with no explicit handle needed, the same way a
    // spawned function never has to be handed its own `Isolate` back
    // just to ask "was I cancelled?". Cleared unconditionally
    // afterward (both the Ok and panic arms below), never left
    // pointing at a finished task's state while this thread picks up
    // its next one.
    CURRENT_ISOLATE.with(|c| *c.borrow_mut() = Some(state.clone()));

    // `catch_unwind` isolates a panic to just the ONE isolate that
    // caused it, rather than taking down every other isolate and the
    // main thread with it: see `Cargo.toml`'s own note on why
    // `panic = "abort"` had to go for this to even be possible.
    // `AssertUnwindSafe` because `&mut isolate.vm` isn't provably
    // unwind-safe on its own (a panic mid-mutation could leave its
    // internal state; registers, GC bookkeeping; torn); the promise
    // that makes this sound is the one kept right below: a torn
    // isolate is never reused, only rebuilt from scratch.
    match panic::catch_unwind(AssertUnwindSafe(|| run_task(&mut isolate, &task))) {
      Ok(result) => state.finish(result),
      Err(payload) => {
        // `&*payload`, not `&payload`: `payload` is `Box<dyn Any +
        // Send>`, and `Box<dyn Any + Send>` itself implements `Any`
        // (it's `'static` too); a bare `&payload` coerces to `&dyn
        // Any` by treating the BOX ITSELF as the trait object, not by
        // dereferencing into what it holds, so `downcast_ref` would
        // always be asking "is the payload literally a `Box`", never
        // "what's inside it". The explicit deref forces the reference
        // at the actual panic value instead.
        state.finish(Err(format!(
          "isolate panicked: {}",
          panic_message(&*payload)
        )));
        isolate = IsolateIsolate::new();
      },
    }
    CURRENT_ISOLATE.with(|c| *c.borrow_mut() = None);
    pool.task_completed();
  }
}

thread_local! {
  /// The isolate THIS isolate thread is currently running, if any --
  /// what lets `is_current_cancelled` answer "was I cancelled?" with
  /// no explicit handle passed in, the same way each isolate's own
  /// isolate needs no explicit parameter either. Set/cleared around
  /// each task in `isolate_loop`; `None` between tasks and on any
  /// thread that isn't a isolate isolate at all.
  static CURRENT_ISOLATE: RefCell<Option<Arc<IsolateState>>> = const { RefCell::new(None) };
}

/// Whether the isolate currently running ON THIS THREAD has been
/// `cancel()`ed. `false` (never `true`) on a thread that isn't a
/// isolate isolate, or between tasks on one that is; there's
/// nothing to have been cancelled either way.
pub fn is_current_cancelled() -> bool {
  CURRENT_ISOLATE.with(|c| c.borrow().as_ref().is_some_and(|s| s.is_cancelled()))
}

/// Whether this thread is currently running a isolate at all;
/// distinct from `is_current_cancelled`, which is `false` both when
/// there's no current isolate AND when there is one but it hasn't
/// been cancelled. The blocking primitives below need to tell those
/// two apart: a plain blocking wait from the main thread (or any other
/// non-isolate caller) has nothing to poll for and should just block
/// the old way, at zero extra cost.
fn in_isolate_context() -> bool {
  CURRENT_ISOLATE.with(|c| c.borrow().is_some())
}

/// How often a blocking wait inside a isolate re-checks whether ITS
/// OWN isolate (the caller, not whatever it's waiting on) has been
/// cancelled. `cancel()` itself wakes `wait_any`/`wait_all`/`select`
/// immediately (they already sit on `wake_gate`), but `join`/`send`/
/// `recv` wait on their own per-object `Condvar` instead; putting
/// THOSE on `wake_gate` too would mean every blocked join/send/recv in
/// the whole process wakes up on every unrelated channel send or
/// isolate finishing, which turns the common case from "wakes the
/// one relevant waiter" into an O(waiters) storm per event. Polling a
/// private condvar at a short, fixed interval instead keeps the common
/// case cheap and exactly as it was; the cost is that a cancellation
/// can take up to this long to be noticed, which is a fair trade for a
/// cooperative cancellation model that already doesn't promise instant
/// interruption of running code either.
const CANCEL_POLL_INTERVAL: Duration = Duration::from_millis(50);

// ---------------------------------------------------------------------
// wait_any / select
// ---------------------------------------------------------------------

/// Shared wakeup signal for `wait_any_isolates`/`select_channels`.
fn wake_gate() -> &'static (Mutex<()>, Condvar) {
  static GATE: OnceLock<(Mutex<()>, Condvar)> = OnceLock::new();
  GATE.get_or_init(|| (Mutex::new(()), Condvar::new()))
}

static ACTIVE_WAKE_LISTENERS: AtomicUsize = AtomicUsize::new(0);

struct WakeListenerGuard;
impl WakeListenerGuard {
  fn new() -> Self {
    ACTIVE_WAKE_LISTENERS.fetch_add(1, Ordering::SeqCst);
    WakeListenerGuard
  }
}
impl Drop for WakeListenerGuard {
  fn drop(&mut self) {
    ACTIVE_WAKE_LISTENERS.fetch_sub(1, Ordering::SeqCst);
  }
}

fn wake_all_waiters() {
  let (m, cv) = wake_gate();
  drop(lock(m));
  cv.notify_all();
}

/// Outcome of `wait_any_isolates`.
pub enum WaitAnyOutcome {
  Ready(usize),
  TimedOut,
  /// The CALLING isolate was cancelled while blocked here.
  Cancelled,
}

/// Outcome of `wait_all_isolates`.
pub enum WaitAllOutcome {
  Ready,
  TimedOut,
  /// The CALLING isolate was cancelled while blocked here.
  Cancelled,
}

/// Outcome of `select_channels`.
pub enum SelectOutcome {
  Ready(usize, RecvOutcome),
  TimedOut,
  /// The CALLING isolate was cancelled while blocked here.
  Cancelled,
}

/// Blocks until at least one of `states` has finished, returning its
/// index into the slice; ties (more than one already done) resolve
/// to whichever comes first in the caller's own list.
pub fn wait_any_isolates(
  states: &[Arc<IsolateState>],
  timeout: Option<Duration>,
) -> WaitAnyOutcome {
  if let Some(i) = states.iter().position(|s| s.is_done()) {
    return WaitAnyOutcome::Ready(i);
  }
  let _guard = WakeListenerGuard::new();
  let deadline = timeout.map(|d| Instant::now() + d);
  let (m, cv) = wake_gate();
  let mut guard = lock(m);
  loop {
    if let Some(i) = states.iter().position(|s| s.is_done()) {
      return WaitAnyOutcome::Ready(i);
    }
    if is_current_cancelled() {
      return WaitAnyOutcome::Cancelled;
    }
    guard = match deadline.map(|dl| dl.saturating_duration_since(Instant::now())) {
      Some(d) if d.is_zero() => return WaitAnyOutcome::TimedOut,
      Some(d) => {
        let (g, result) = cv
          .wait_timeout(guard, d)
          .unwrap_or_else(PoisonError::into_inner);
        if result.timed_out() {
          return match states.iter().position(|s| s.is_done()) {
            Some(i) => WaitAnyOutcome::Ready(i),
            None => WaitAnyOutcome::TimedOut,
          };
        }
        g
      },
      None => cv.wait(guard).unwrap_or_else(PoisonError::into_inner),
    };
  }
}

/// Blocks until EVERY one of `states` has finished.
pub fn wait_all_isolates(
  states: &[Arc<IsolateState>],
  timeout: Option<Duration>,
) -> WaitAllOutcome {
  let started = Instant::now();
  for s in states {
    if is_current_cancelled() {
      return WaitAllOutcome::Cancelled;
    }
    if s.is_done() {
      continue;
    }
    let remaining = match timeout {
      Some(t) => {
        let elapsed = started.elapsed();
        if elapsed >= t {
          return if states.iter().all(|s| s.is_done()) {
            WaitAllOutcome::Ready
          } else {
            WaitAllOutcome::TimedOut
          };
        }
        Some(t - elapsed)
      },
      None => None,
    };

    match s.join_inner(remaining) {
      JoinOutcome::Ok(_) | JoinOutcome::Err(_) => {},
      JoinOutcome::Pending => return WaitAllOutcome::TimedOut,
      JoinOutcome::Cancelled => return WaitAllOutcome::Cancelled,
    }
  }
  WaitAllOutcome::Ready
}

/// Blocks until at least one of `states` (channels) has a value ready
/// to receive or is closed, returning its index and the outcome;
/// already taken off the winning channel's own queue, same as
/// `try_recv`. Same ordering/timeout behavior as
/// `wait_any_isolates`.
pub fn select_channels(states: &[Arc<ChannelState>], timeout: Option<Duration>) -> SelectOutcome {
  for (i, s) in states.iter().enumerate() {
    if let Some(outcome) = s.try_recv() {
      return SelectOutcome::Ready(i, outcome);
    }
  }
  let _guard = WakeListenerGuard::new();
  let deadline = timeout.map(|d| Instant::now() + d);
  let (m, cv) = wake_gate();
  let mut guard = lock(m);
  loop {
    for (i, s) in states.iter().enumerate() {
      if let Some(outcome) = s.try_recv() {
        return SelectOutcome::Ready(i, outcome);
      }
    }
    if is_current_cancelled() {
      return SelectOutcome::Cancelled;
    }
    guard = match deadline.map(|dl| dl.saturating_duration_since(Instant::now())) {
      Some(d) if d.is_zero() => return SelectOutcome::TimedOut,
      Some(d) => {
        let (g, result) = cv
          .wait_timeout(guard, d)
          .unwrap_or_else(PoisonError::into_inner);
        if result.timed_out() {
          for (i, s) in states.iter().enumerate() {
            if let Some(outcome) = s.try_recv() {
              return SelectOutcome::Ready(i, outcome);
            }
          }
          return SelectOutcome::TimedOut;
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
  /// through `isolate_loop`'s full task-queue/isolate machinery, so
  /// this exercises the actual mechanism (`catch_unwind` plus the
  /// `&*payload` deref: see that call site's own docs on why a bare
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

fn run_task(isolate: &mut IsolateIsolate, task: &Task) -> Result<TransferGraph, String> {
  let mark = isolate.vm.pin_values(std::iter::empty());
  match transfer::materialize(&mut isolate.vm, &task.callee) {
    Ok(v) => {
      isolate.vm.pin_values([v]);
    },
    Err(e) => {
      isolate.vm.unpin(mark);
      return Err(e);
    },
  };
  match transfer::materialize(&mut isolate.vm, &task.args) {
    Ok(v) => {
      isolate.vm.pin_values([v]);
    },
    Err(e) => {
      isolate.vm.unpin(mark);
      return Err(e);
    },
  };
  let callee = isolate.vm.pinned(mark);
  let args_val = isolate.vm.pinned(mark + 1);

  let args = args_val.as_list();

  // `VM::call_value` only ever accepts a closure or a native, never
  // a bare `ObjBoundMethod`; so `instance.method` used directly as
  // a spawn target needs its receiver spliced back in as arg 0
  // (exactly what `Instr::Invoke` already does for an ordinary
  // `instance.method(...)` call site; this is that same convention,
  // just applied by hand since there's no such instruction here).
  let (target, full_args) = if callee.is_bound_method() {
    let bm = callee.as_bound_method();
    if !bm.method.is_closure() {
      isolate.vm.unpin(mark);
      return Err("isolate spawn target is not a callable function".to_string());
    }
    let mut full = Vec::with_capacity(args.len() + 1);
    full.push(bm.receiver);
    full.extend(args);
    (bm.method, full)
  } else if callee.is_closure() {
    (callee, args)
  } else {
    isolate.vm.unpin(mark);
    return Err("isolate spawn target is not a callable function".to_string());
  };

  let result = match isolate.vm.call_value(target, &full_args) {
    Ok(ret) => transfer::capture(&isolate.vm, ret),
    Err(exc) => Err(isolate.vm.format_isolate_error(exc)),
  };
  isolate.vm.unpin(mark);
  result
}

// ---------------------------------------------------------------------
// Isolate handles
// ---------------------------------------------------------------------

enum Slot {
  Pending,
  Ok(TransferGraph),
  /// A rendered, human-readable failure; either the spawned
  /// function's own uncaught error (see `VM::describe_error`)
  /// or an infrastructure failure (bad spawn target, a value that
  /// couldn't cross the isolate boundary, ...). Deliberately not the
  /// original error `Value` itself: that `Value` lives on the
  /// ISOLATE's own heap and can't be handed back across the thread
  /// boundary any more than any other `Value` can: see `transfer`'s
  /// own docs. `libs/isolate.zu` wraps this text in its own
  /// `IsolateError` on `.join()`.
  Err(String),
}

pub enum JoinOutcome {
  Pending,
  Ok(TransferGraph),
  Err(String),
  /// The CALLING isolate (not the one being joined) was cancelled
  /// while blocked here.
  Cancelled,
}

const STATUS_PENDING: u8 = 0;
const STATUS_OK: u8 = 1;
const STATUS_ERR: u8 = 2;

pub struct IsolateState {
  status: AtomicU8,
  slot: Mutex<Slot>,
  cv: Condvar,
  /// Set once `join()`/`try_join()` has actually reported a finished
  /// outcome (`Ok` or `Err`) to someone: see `Drop`'s own docs.
  observed: AtomicBool,
  /// Set by `cancel()`, read by `is_current_cancelled()` from inside
  /// the isolate's own execution; purely COOPERATIVE, same as
  /// every other language's cancellation token: nothing here stops
  /// already-running code on its own. A isolate that never checks
  /// simply runs to completion regardless of this flag.
  cancelled: AtomicBool,
  /// Set once, at `spawn()` time, by whoever used `spawn_named()`
  /// instead of plain `spawn()`. Purely a debugging/introspection
  /// label; never read for anything that affects behavior; so it
  /// gets folded into the unobserved-failure warning (see `Drop`) and
  /// exposed read-only via `Isolate.name()`.
  name: Option<String>,
}

impl IsolateState {
  fn new(name: Option<String>) -> Self {
    IsolateState {
      status: AtomicU8::new(STATUS_PENDING),
      slot: Mutex::new(Slot::Pending),
      cv: Condvar::new(),
      observed: AtomicBool::new(false),
      cancelled: AtomicBool::new(false),
      name,
    }
  }

  pub fn name(&self) -> Option<&str> {
    self.name.as_deref()
  }

  pub fn cancel(&self) {
    self.cancelled.store(true, Ordering::Relaxed);
    if ACTIVE_WAKE_LISTENERS.load(Ordering::Relaxed) > 0 {
      wake_all_waiters();
    }
  }

  pub fn is_cancelled(&self) -> bool {
    self.cancelled.load(Ordering::Relaxed)
  }

  fn finish(&self, result: Result<TransferGraph, String>) {
    let new_status = match &result {
      Ok(_) => STATUS_OK,
      Err(_) => STATUS_ERR,
    };
    {
      let mut slot = lock(&self.slot);
      *slot = match result {
        Ok(g) => Slot::Ok(g),
        Err(m) => Slot::Err(m),
      };
      self.status.store(new_status, Ordering::Release);
    }
    self.cv.notify_all();
    if ACTIVE_WAKE_LISTENERS.load(Ordering::Relaxed) > 0 {
      wake_all_waiters();
    }
  }

  /// Blocks the calling thread until the isolate finishes. Callable
  /// more than once (and from more than one joiner); always returns
  /// the same, already-computed outcome once it's in.
  ///
  /// If the CALLING isolate (not this one) is cancelled while
  /// blocked here, gives up early with `JoinOutcome::Cancelled` --
  /// join() is itself a blocking wait a cancelled isolate can be
  /// stuck in, same as `Channel.send`/`recv`.
  pub fn join(&self) -> JoinOutcome {
    self.join_inner(None)
  }

  /// Like `join()`, but gives up and returns `JoinOutcome::Pending`
  /// (indistinguishable from "still running"; from the caller's own
  /// point of view, that's exactly what a timeout means) if `timeout`
  /// elapses first.
  pub fn join_timeout(&self, timeout: Duration) -> JoinOutcome {
    self.join_inner(Some(timeout))
  }

  /// Shared implementation for `join`/`join_timeout`.
  fn join_inner(&self, deadline: Option<Duration>) -> JoinOutcome {
    if deadline.is_none() && !in_isolate_context() {
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

    let started = Instant::now();
    loop {
      let remaining = deadline.map(|d| d.saturating_sub(started.elapsed()));
      let tick = remaining.map_or(CANCEL_POLL_INTERVAL, |r| r.min(CANCEL_POLL_INTERVAL));

      let slot = lock(&self.slot);
      let (slot, _result) = self
        .cv
        .wait_timeout_while(slot, tick, |s| matches!(s, Slot::Pending))
        .unwrap_or_else(PoisonError::into_inner);
      match &*slot {
        Slot::Ok(g) => {
          self.observed.store(true, Ordering::Relaxed);
          return JoinOutcome::Ok(g.clone());
        },
        Slot::Err(m) => {
          self.observed.store(true, Ordering::Relaxed);
          return JoinOutcome::Err(m.clone());
        },
        Slot::Pending => {},
      }
      drop(slot);
      if remaining.is_some_and(|r| r.is_zero()) {
        return JoinOutcome::Pending;
      }
      if is_current_cancelled() {
        return JoinOutcome::Cancelled;
      }
    }
  }

  pub fn try_join(&self) -> JoinOutcome {
    if !self.is_done() {
      return JoinOutcome::Pending;
    }
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
    self.status.load(Ordering::Acquire) != STATUS_PENDING
  }

  /// Like `try_join`, but never marks the outcome "observed"; pure
  /// introspection for `Isolate.status()`.
  pub fn peek(&self) -> JoinOutcome {
    match self.status.load(Ordering::Acquire) {
      STATUS_PENDING => JoinOutcome::Pending,
      _ => match &*lock(&self.slot) {
        Slot::Pending => JoinOutcome::Pending,
        Slot::Ok(g) => JoinOutcome::Ok(g.clone()),
        Slot::Err(m) => JoinOutcome::Err(m.clone()),
      },
    }
  }
}

impl Drop for IsolateState {
  /// A isolate's failure doesn't otherwise go anywhere unless
  /// something calls `join()`/`try_join()` on it; exactly like a
  /// plain `std::thread` whose `JoinHandle` is dropped without ever
  /// being joined, an uncaught error or a caught panic inside a
  /// fire-and-forget `spawn()` would silently vanish once the last
  /// `Isolate` handle (and the pool's own internal one) goes out of
  /// scope. A dropped `String` costs nothing to check for and losing
  /// a real failure silently is worse than one unwanted log line, so
  /// this reports it; the same trade-off Rust's own default panic
  /// hook already makes for an unjoined thread.
  fn drop(&mut self) {
    if self.observed.load(Ordering::Relaxed) {
      return;
    }
    if let Slot::Err(message) = &*self.slot.get_mut().unwrap_or_else(PoisonError::into_inner) {
      match &self.name {
        Some(name) => eprintln!(
          "warning: isolate '{}' failed but its result was never checked \
           (no join()/try_join() was called before its handle was dropped): {}",
          name, message
        ),
        None => eprintln!(
          "warning: a isolate failed but its result was never checked \
           (no join()/try_join() was called before its handle was dropped): {}",
          message
        ),
      }
    }
  }
}

/// Captures `callee`/`args` (on the CALLING isolate's own heap, via
/// `vm`) and queues them for the pool to pick up.
pub fn spawn(
  vm: &VM,
  callee: Value,
  args: Value,
  name: Option<String>,
) -> Result<Arc<IsolateState>, String> {
  let callee = transfer::capture(vm, callee)?;
  let args = transfer::capture(vm, args)?;
  let state = Arc::new(IsolateState::new(name));
  let task = Task {
    callee,
    args,
    state: state.clone(),
  };
  let p = pool();
  {
    let mut queue = lock(&p.queue);
    if p.shutting_down.load(Ordering::Acquire) {
      return Err("cannot spawn: the isolate pool is shutting down".to_string());
    }
    p.in_flight.fetch_add(1, Ordering::AcqRel);
    queue.push_back(task);
  }
  p.not_empty.notify_one();
  Ok(state)
}

/// Spawns a batch of isolates for `map()`, capturing the callee once
/// and submitting all tasks under a single lock.
pub fn spawn_batch(
  vm: &VM,
  callee: Value,
  args_items: &[Value],
) -> Result<Vec<Arc<IsolateState>>, String> {
  let callee_graph = transfer::capture(vm, callee)?;
  let mut states = Vec::with_capacity(args_items.len());
  let mut tasks = Vec::with_capacity(args_items.len());

  for &item in args_items {
    let args_graph = transfer::capture_as_args_list(vm, item)?;
    let state = Arc::new(IsolateState::new(None));
    tasks.push(Task {
      callee: callee_graph.clone(),
      args: args_graph,
      state: state.clone(),
    });
    states.push(state);
  }

  let p = pool();
  {
    let mut queue = lock(&p.queue);
    if p.shutting_down.load(Ordering::Acquire) {
      return Err("cannot spawn: the isolate pool is shutting down".to_string());
    }
    p.in_flight.fetch_add(tasks.len(), Ordering::AcqRel);
    queue.extend(tasks);
  }
  p.not_empty.notify_all();
  Ok(states)
}

/// Stops the pool from accepting any further `spawn()` calls, then
/// blocks until every task already queued or running has finished;
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
      p.stop_compiler();
      return true;
    }
    guard = match deadline.map(|dl| dl.saturating_duration_since(Instant::now())) {
      Some(d) if d.is_zero() => {
        let done = p.in_flight.load(Ordering::Acquire) == 0;
        if done {
          p.stop_compiler();
        }
        return done;
      },
      Some(d) => {
        let (g, result) = p
          .idle
          .wait_timeout(guard, d)
          .unwrap_or_else(PoisonError::into_inner);
        if result.timed_out() {
          let done = p.in_flight.load(Ordering::Acquire) == 0;
          if done {
            p.stop_compiler();
          }
          return done;
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
  /// `recv`/`recv_timeout` only; never produced by `try_recv`, which
  /// doesn't block in the first place.
  TimedOut,
  /// The CALLING isolate was cancelled while blocked here.
  Cancelled,
}

pub enum SendOutcome {
  Sent,
  Closed,
  TimedOut,
  /// The CALLING isolate was cancelled while blocked here.
  Cancelled,
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

  /// Blocks while the channel is at capacity. If the CALLING
  /// isolate is cancelled while blocked here, gives up early with
  /// `SendOutcome::Cancelled`.
  pub fn send(&self, value: TransferGraph) -> SendOutcome {
    self.send_inner(value, None)
  }

  /// Like `send`, but gives up (returning `TimedOut`) if `timeout`
  /// elapses before the channel has room.
  pub fn send_timeout(&self, value: TransferGraph, timeout: Duration) -> SendOutcome {
    self.send_inner(value, Some(timeout))
  }

  /// Shared implementation for `send`/`send_timeout`; same
  /// no-poll-unless-needed and `CANCEL_POLL_INTERVAL`-ticked shape as
  /// `IsolateState::join_inner`: see its own docs for why.
  fn send_inner(&self, value: TransferGraph, deadline: Option<Duration>) -> SendOutcome {
    if deadline.is_none() && !in_isolate_context() {
      let mut inner = lock(&self.inner);
      loop {
        if inner.closed {
          return SendOutcome::Closed;
        }
        let full = inner.capacity.is_some_and(|cap| inner.queue.len() >= cap);
        if !full {
          break;
        }
        inner = self
          .not_full
          .wait(inner)
          .unwrap_or_else(PoisonError::into_inner);
      }
      inner.queue.push_back(value);
      drop(inner);
      self.not_empty.notify_one();
      wake_all_waiters();
      return SendOutcome::Sent;
    }

    let started = Instant::now();
    loop {
      // See `join_inner`'s own comment on why the timeout check comes
      // AFTER the real attempt, not before; a zero tick still gets
      // one genuine chance to see the channel already has room.
      let remaining = deadline.map(|d| d.saturating_sub(started.elapsed()));
      let tick = remaining.map_or(CANCEL_POLL_INTERVAL, |r| r.min(CANCEL_POLL_INTERVAL));

      let inner = lock(&self.inner);
      let (mut inner, _result) = self
        .not_full
        .wait_timeout_while(inner, tick, |inner| {
          !inner.closed && inner.capacity.is_some_and(|cap| inner.queue.len() >= cap)
        })
        .unwrap_or_else(PoisonError::into_inner);
      if inner.closed {
        return SendOutcome::Closed;
      }
      let full = inner.capacity.is_some_and(|cap| inner.queue.len() >= cap);
      if !full {
        inner.queue.push_back(value);
        drop(inner);
        self.not_empty.notify_one();
        wake_all_waiters();
        return SendOutcome::Sent;
      }
      drop(inner);
      if remaining.is_some_and(|r| r.is_zero()) {
        return SendOutcome::TimedOut;
      }
      if is_current_cancelled() {
        return SendOutcome::Cancelled;
      }
    }
  }

  /// Blocks until a message arrives or the channel is closed AND
  /// drained. If the CALLING isolate is cancelled while blocked
  /// here, gives up early with `RecvOutcome::Cancelled`.
  pub fn recv(&self) -> RecvOutcome {
    self.recv_inner(None)
  }

  /// Like `recv`, but gives up with `RecvOutcome::TimedOut` if
  /// `timeout` elapses first with nothing to receive and the channel
  /// still open.
  pub fn recv_timeout(&self, timeout: Duration) -> RecvOutcome {
    self.recv_inner(Some(timeout))
  }

  /// Shared implementation for `recv`/`recv_timeout`; same shape as
  /// `send_inner`/`join_inner`: see `CANCEL_POLL_INTERVAL`'s docs.
  fn recv_inner(&self, deadline: Option<Duration>) -> RecvOutcome {
    if deadline.is_none() && !in_isolate_context() {
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
        inner = self
          .not_empty
          .wait(inner)
          .unwrap_or_else(PoisonError::into_inner);
      }
    }

    let started = Instant::now();
    loop {
      // See `join_inner`'s own comment on why the timeout check comes
      // AFTER the real attempt, not before; a zero tick still gets
      // one genuine chance to see a value that's already queued.
      let remaining = deadline.map(|d| d.saturating_sub(started.elapsed()));
      let tick = remaining.map_or(CANCEL_POLL_INTERVAL, |r| r.min(CANCEL_POLL_INTERVAL));

      let inner = lock(&self.inner);
      let (mut inner, _result) = self
        .not_empty
        .wait_timeout_while(inner, tick, |inner| inner.queue.is_empty() && !inner.closed)
        .unwrap_or_else(PoisonError::into_inner);
      if let Some(v) = inner.queue.pop_front() {
        drop(inner);
        self.not_full.notify_one();
        return RecvOutcome::Value(v);
      }
      if inner.closed {
        return RecvOutcome::Closed;
      }
      drop(inner);
      if remaining.is_some_and(|r| r.is_zero()) {
        return RecvOutcome::TimedOut;
      }
      if is_current_cancelled() {
        return RecvOutcome::Cancelled;
      }
    }
  }

  /// `None` means "empty, but still open"; the one outcome `recv`
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
