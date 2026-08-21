//! The coroutine pool: a small, configurable number of persistent
//! worker OS threads, each owning its own totally independent `VM`/
//! `Heap` ("isolate"). A coroutine is a task queued onto this pool; a
//! channel is a plain thread-safe queue of already-`capture`d
//! messages. Nothing here ever shares a `Value`, a heap pointer, or
//! compiled bytecode between threads -- see `transfer` for what
//! actually crosses, and why that's the only thing that safely can.

use std::collections::VecDeque;
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::thread;

use crate::vm::object::Heap;
use crate::vm::value::Value;
use crate::vm::vm::VM;

use super::transfer::{self, TransferGraph};

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
  let mut configured = CONFIGURED_SIZE.lock().unwrap();
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

fn pool() -> &'static CoroutinePool {
  POOL.get_or_init(|| {
    let size = CONFIGURED_SIZE.lock().unwrap().unwrap_or_else(default_size);
    CoroutinePool::start(size)
  })
}

struct CoroutinePool {
  queue: Mutex<VecDeque<Task>>,
  not_empty: Condvar,
  size: usize,
}

impl CoroutinePool {
  fn start(size: usize) -> Self {
    for i in 0..size {
      thread::Builder::new()
        .name(format!("zuri-coroutine-{}", i))
        .spawn(worker_loop)
        .expect("failed to spawn coroutine worker thread");
    }
    CoroutinePool {
      queue: Mutex::new(VecDeque::new()),
      not_empty: Condvar::new(),
      size,
    }
  }
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
      let mut queue = pool.queue.lock().unwrap();
      while queue.is_empty() {
        queue = pool.not_empty.wait(queue).unwrap();
      }
      queue.pop_front().unwrap()
    };
    run_task(&mut isolate, task);
  }
}

fn run_task(isolate: &mut WorkerIsolate, task: Task) {
  let result = (|| -> Result<TransferGraph, String> {
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
  })();
  task.state.finish(result);
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
}

impl CoroutineState {
  fn new() -> Self {
    CoroutineState {
      slot: Mutex::new(Slot::Pending),
      cv: Condvar::new(),
    }
  }

  fn finish(&self, result: Result<TransferGraph, String>) {
    let mut slot = self.slot.lock().unwrap();
    *slot = match result {
      Ok(g) => Slot::Ok(g),
      Err(m) => Slot::Err(m),
    };
    drop(slot);
    self.cv.notify_all();
  }

  /// Blocks the calling thread until the coroutine finishes. Callable
  /// more than once (and from more than one joiner) -- always returns
  /// the same, already-computed outcome once it's in.
  pub fn join(&self) -> JoinOutcome {
    let mut slot = self.slot.lock().unwrap();
    loop {
      match &*slot {
        Slot::Pending => slot = self.cv.wait(slot).unwrap(),
        Slot::Ok(g) => return JoinOutcome::Ok(g.clone()),
        Slot::Err(m) => return JoinOutcome::Err(m.clone()),
      }
    }
  }

  pub fn try_join(&self) -> JoinOutcome {
    match &*self.slot.lock().unwrap() {
      Slot::Pending => JoinOutcome::Pending,
      Slot::Ok(g) => JoinOutcome::Ok(g.clone()),
      Slot::Err(m) => JoinOutcome::Err(m.clone()),
    }
  }

  pub fn is_done(&self) -> bool {
    !matches!(&*self.slot.lock().unwrap(), Slot::Pending)
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
  p.queue.lock().unwrap().push_back(task);
  p.not_empty.notify_one();
  Ok(state)
}

// ---------------------------------------------------------------------
// Channels
// ---------------------------------------------------------------------

pub enum RecvOutcome {
  Value(TransferGraph),
  Closed,
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
    let mut inner = self.inner.lock().unwrap();
    loop {
      if inner.closed {
        return Err(());
      }
      let full = inner.capacity.is_some_and(|cap| inner.queue.len() >= cap);
      if !full {
        break;
      }
      inner = self.not_full.wait(inner).unwrap();
    }
    inner.queue.push_back(value);
    drop(inner);
    self.not_empty.notify_one();
    Ok(())
  }

  /// Blocks until a message arrives or the channel is closed AND
  /// drained.
  pub fn recv(&self) -> RecvOutcome {
    let mut inner = self.inner.lock().unwrap();
    loop {
      if let Some(v) = inner.queue.pop_front() {
        drop(inner);
        self.not_full.notify_one();
        return RecvOutcome::Value(v);
      }
      if inner.closed {
        return RecvOutcome::Closed;
      }
      inner = self.not_empty.wait(inner).unwrap();
    }
  }

  /// `None` means "empty, but still open" -- the one outcome `recv`
  /// never produces, since it would just keep waiting instead.
  pub fn try_recv(&self) -> Option<RecvOutcome> {
    let mut inner = self.inner.lock().unwrap();
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
    let mut inner = self.inner.lock().unwrap();
    inner.closed = true;
    drop(inner);
    self.not_empty.notify_all();
    self.not_full.notify_all();
  }

  pub fn is_closed(&self) -> bool {
    self.inner.lock().unwrap().closed
  }

  pub fn len(&self) -> usize {
    self.inner.lock().unwrap().queue.len()
  }
}
