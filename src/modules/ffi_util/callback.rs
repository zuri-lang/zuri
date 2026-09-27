//! C calling back into Zuri.
//!
//! A callback is a libffi closure: a small piece of executable code with
//! the C signature the library expects, which lands in `trampoline`
//! with the arguments and a pointer to the callback's own state.
//!
//! Where it runs depends on the thread C calls it from.
//!
//! On the thread of the isolate that made the callback, while a foreign
//! call from that isolate is in progress, it runs at once. This is the
//! ordinary case: `qsort` calling its comparator.
//!
//! On any other thread, the call is posted to the owning isolate's inbox
//! and the calling thread waits for the answer. The isolate answers at
//! its next safepoint, inside `ffi.serve()`, or at once when it is
//! waiting in a threaded call.
//!
//! Two things may never cross back into C: a Rust panic, which is caught
//! at the trampoline, and a Zuri error, which cannot unwind through C
//! frames. An error is trapped on the VM, C gets the callback's error
//! value (zero unless the program chose one), and the error is raised
//! again as soon as control is back in Zuri.

use std::cell::Cell;
use std::collections::VecDeque;
use std::marker::PhantomData;
use std::os::raw::c_void;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::ThreadId;
use std::time::Duration;

use libffi_sys as raw;

use super::call::Cif;
use super::convert::{self, Dest};
use super::types::{Kind, Signature};
use super::{Fail, state};
use crate::modules::os_util::signal;
use crate::vm::value::Value;
use crate::vm::vm::VM;

thread_local! {
  /// The VM whose foreign call is in progress on this thread, or null.
  static ACTIVE: Cell<*mut VM> = const { Cell::new(std::ptr::null_mut()) };

  /// Set while the collector runs a C destructor on this thread.
  static RELEASING: Cell<bool> = const { Cell::new(false) };
}

/// Runs `f`, a destructor the collector is calling, with every callback
/// refused for its length. The collection that got here may be halfway
/// through a sweep, or on another isolate whose owner is waiting for it,
/// so running Zuri code or posting a call and waiting could never be safe.
pub fn releasing<R>(f: impl FnOnce() -> R) -> R {
  let previous = RELEASING.with(|slot| slot.replace(true));
  let result = f();
  RELEASING.with(|slot| slot.set(previous));
  result
}

/// Publishes a VM for the trampolines on this thread, putting back
/// whatever was published before when dropped. The borrow it holds keeps
/// the VM from being used by anything else while C may be calling in.
pub struct VmGuard<'a> {
  previous: *mut VM,
  _owner: PhantomData<&'a mut VM>,
}

impl Drop for VmGuard<'_> {
  fn drop(&mut self) {
    ACTIVE.with(|slot| slot.set(self.previous));
  }
}

pub fn enter(vm: &mut VM) -> VmGuard<'_> {
  let previous = ACTIVE.with(|slot| slot.replace(vm as *mut VM));
  VmGuard {
    previous,
    _owner: PhantomData,
  }
}

/// Calls waiting for an isolate to answer them.
pub struct Inbox {
  queue: Mutex<VecDeque<Arc<Request>>>,
  /// How many calls are queued, readable without the lock; the
  /// safepoint polls this.
  waiting: AtomicUsize,
  wake: Condvar,
  closed: AtomicBool,
  thread: ThreadId,
}

struct Request {
  core: Arc<CallbackCore>,
  args: usize,
  ret: usize,
  done: Mutex<bool>,
  finished: Condvar,
}

impl Inbox {
  fn new() -> Inbox {
    Inbox {
      queue: Mutex::new(VecDeque::new()),
      waiting: AtomicUsize::new(0),
      wake: Condvar::new(),
      closed: AtomicBool::new(false),
      thread: std::thread::current().id(),
    }
  }

  #[inline]
  pub fn has_pending(&self) -> bool {
    self.waiting.load(Ordering::Relaxed) > 0
  }

  pub fn notify(&self) {
    let _lock = self.queue.lock().unwrap();
    self.wake.notify_all();
  }

  /// Waits up to `timeout` for a call to be posted or for `notify`.
  pub fn wait(&self, timeout: Duration) {
    let queue = self.queue.lock().unwrap();
    if queue.is_empty() && !self.closed.load(Ordering::Acquire) {
      let _ = self.wake.wait_timeout(queue, timeout);
    }
  }

  /// Refuses every call from now on and answers the waiting ones with
  /// their error values. Runs when the isolate's VM goes away.
  pub fn close(&self) {
    self.closed.store(true, Ordering::Release);
    let pending: Vec<Arc<Request>> = self.queue.lock().unwrap().drain(..).collect();
    self.waiting.store(0, Ordering::Release);

    for request in pending {
      signal::finish_async();
      unsafe { request.core.write_error_value(request.ret as *mut c_void) };
      request.complete();
    }

    self.wake.notify_all();
  }

  /// Posts a call and blocks until the owning isolate has answered it.
  /// Returns false when the isolate is gone and the call was not run.
  fn post_and_wait(
    &self,
    core: &Arc<CallbackCore>,
    args: *mut *mut c_void,
    ret: *mut c_void,
  ) -> bool {
    if self.closed.load(Ordering::Acquire) {
      return false;
    }

    let request = Arc::new(Request {
      core: core.clone(),
      args: args as usize,
      ret: ret as usize,
      done: Mutex::new(false),
      finished: Condvar::new(),
    });

    {
      let mut queue = self.queue.lock().unwrap();
      if self.closed.load(Ordering::Acquire) {
        return false;
      }
      queue.push_back(request.clone());
      self.waiting.fetch_add(1, Ordering::Release);
      signal::start_async();
      self.wake.notify_all();
    }

    let mut done = request.done.lock().unwrap();
    while !*done {
      done = request.finished.wait(done).unwrap();
    }

    true
  }

  fn pop(&self) -> Option<Arc<Request>> {
    let mut queue = self.queue.lock().unwrap();
    let request = queue.pop_front()?;
    self.waiting.fetch_sub(1, Ordering::Release);
    Some(request)
  }
}

impl Request {
  fn complete(&self) {
    let mut done = self.done.lock().unwrap();
    *done = true;
    self.finished.notify_all();
  }
}

/// This VM's inbox, created on first use.
pub fn inbox(vm: &mut VM) -> Arc<Inbox> {
  let st = state(vm);
  st.inbox
    .get_or_insert_with(|| Arc::new(Inbox::new()))
    .clone()
}

/// Answers every call posted to `inbox`. Raises the first error a
/// callback raised while answering them.
pub fn service(vm: &mut VM, inbox: &Arc<Inbox>) -> Result<usize, Fail> {
  let mut answered = 0;

  while let Some(request) = inbox.pop() {
    signal::finish_async();
    unsafe {
      run_here(
        vm,
        &request.core,
        request.args as *mut *mut c_void,
        request.ret as *mut c_void,
      );
    }
    request.complete();
    answered += 1;
  }

  if let Some(error) = take_trapped(vm) {
    return Err(Fail::Raised(error));
  }

  Ok(answered)
}

/// `service` for whatever this VM has waiting, if it has an inbox at all.
pub fn service_pending(vm: &mut VM) -> Result<usize, Fail> {
  let Some(inbox) = vm.ffi.as_ref().and_then(|s| s.inbox.clone()) else {
    return Ok(0);
  };

  if !inbox.has_pending() {
    return Ok(0);
  }

  service(vm, &inbox)
}

/// The safepoint's entry: answers posted calls, returning the error one
/// of them raised as a Zuri value.
pub fn service_at_safepoint(vm: &mut VM) -> Result<(), Value> {
  match service_pending(vm) {
    Ok(_) => Ok(()),
    Err(Fail::Raised(error)) => Err(error),
    Err(fail) => {
      let message = super::raise(vm, fail);
      let _ = message;
      Err(
        vm.take_pending_error()
          .unwrap_or_else(|| vm.raise("Error", "callback failed")),
      )
    },
  }
}

/// The error a callback trapped during the call that just finished.
pub fn take_trapped(vm: &mut VM) -> Option<Value> {
  let slot = vm.ffi.as_mut()?.trapped.take()?;
  let error = vm.native_root(slot);
  vm.release_native_root(slot);
  Some(error)
}

fn trap(vm: &mut VM, error: Value) {
  if state(vm).trapped.is_some() {
    // The first error is the one worth reporting; later ones are almost
    // always its consequences.
    return;
  }

  let slot = vm.retain_native_root(error);
  state(vm).trapped = Some(slot);
}

/// Everything a callback needs, shared by the closure and the Zuri
/// object that owns it.
pub struct CallbackCore {
  sig: Arc<Signature>,
  cif: Arc<Cif>,
  closure: *mut raw::ffi_closure,
  code: usize,
  /// What C calls instead of the closure when the result is a 128-bit
  /// integer returned in `xmm0`.
  #[cfg(target_arch = "x86_64")]
  wide: Mutex<Option<super::wide_return::Stub>>,
  /// The native root holding the Zuri function, on the owning VM.
  slot: AtomicUsize,
  owner: Arc<Inbox>,
  /// The owning VM, compared against the published one and never
  /// dereferenced from any other thread.
  owner_vm: usize,
  released: AtomicBool,
  /// What C gets back when the Zuri function raises: the callback's
  /// error value, already converted, or zeros.
  error_value: Vec<u8>,
}

// SAFETY: the closure and cif are immutable once prepared. The only
// state touched from more than one thread is behind atomics or the
// inbox's locks, and the VM is only ever used from its own thread.
unsafe impl Send for CallbackCore {}
unsafe impl Sync for CallbackCore {}

impl CallbackCore {
  /// A callback for the call in progress, released when the call
  /// returns.
  pub fn temporary(
    vm: &mut VM,
    function: Value,
    sig: Arc<Signature>,
  ) -> Result<Arc<CallbackCore>, Fail> {
    CallbackCore::new(vm, function, sig, None)
  }

  pub fn new(
    vm: &mut VM,
    function: Value,
    sig: Arc<Signature>,
    error_value: Option<Value>,
  ) -> Result<Arc<CallbackCore>, Fail> {
    if sig.variadic {
      return Err(Fail::type_error(
        "a callback cannot be variadic; C gives a variadic function no way to find its arguments",
      ));
    }

    for p in &sig.params {
      if matches!(p.kind, Kind::Array { .. }) {
        return Err(Fail::type_error(format!(
          "a callback parameter cannot be the array '{}'",
          p.name
        )));
      }
    }

    let cif = Arc::new(Cif::new(&sig, &[]).map_err(Fail::from_type)?);

    let mut error_bytes = vec![0u8; cif.return_size.max(16)];
    if let Some(value) = error_value
      && !sig.returns.is_void()
    {
      store_return(vm, &sig, value, error_bytes.as_mut_ptr())?;
    }

    let owner = inbox(vm);
    let slot = vm.retain_native_root(function);

    let mut code: *mut c_void = std::ptr::null_mut();
    let closure = unsafe { raw::ffi_closure_alloc(size_of::<raw::ffi_closure>(), &mut code) }
      as *mut raw::ffi_closure;

    if closure.is_null() {
      vm.release_native_root(slot);
      return Err(Fail::callback(
        "could not allocate executable memory for a callback",
      ));
    }

    let core = Arc::new(CallbackCore {
      sig,
      cif,
      closure,
      code: code as usize,
      #[cfg(target_arch = "x86_64")]
      wide: Mutex::new(None),
      slot: AtomicUsize::new(slot),
      owner,
      owner_vm: vm as *mut VM as usize,
      released: AtomicBool::new(false),
      error_value: error_bytes,
    });

    // The closure holds its own reference, given back in `release`.
    let user_data = Arc::into_raw(core.clone()) as *mut c_void;

    let status = unsafe {
      raw::ffi_prep_closure_loc(
        closure,
        core.cif.as_ptr(),
        Some(trampoline),
        user_data,
        code,
      )
    };

    if status != raw::ffi_status_FFI_OK {
      unsafe { drop(Arc::from_raw(user_data as *const CallbackCore)) };
      core.release_on(vm);
      return Err(Fail::callback("libffi could not prepare the callback"));
    }

    #[cfg(target_arch = "x86_64")]
    if core.cif.wide_return {
      let Some(stub) = super::wide_return::Stub::new(core.code, core.cif.slots.len()) else {
        core.release_on(vm);
        return Err(Fail::callback(format!(
          "at most {} callbacks returning a 128-bit integer under the win64 convention \
           can be alive at once; release one first",
          super::wide_return::STUBS
        )));
      };
      *core.wide.lock().unwrap() = Some(stub);
    }

    Ok(core)
  }

  /// The address C calls.
  pub fn code(&self) -> usize {
    #[cfg(target_arch = "x86_64")]
    if let Some(stub) = self.wide.lock().unwrap().as_ref() {
      return stub.address();
    }

    self.code
  }

  pub fn signature(&self) -> &Arc<Signature> {
    &self.sig
  }

  pub fn is_released(&self) -> bool {
    self.released.load(Ordering::Acquire)
  }

  pub fn is_owned_by(&self, vm: &VM) -> bool {
    self.owner_vm == vm as *const VM as usize
  }

  /// Frees the closure and lets go of the Zuri function. Only the owning
  /// VM can drop the root; from anywhere else the root is left for the
  /// owning VM to reclaim when it ends.
  pub fn release_on(&self, vm: &mut VM) {
    if self.released.swap(true, Ordering::AcqRel) {
      return;
    }

    if self.is_owned_by(vm) {
      let slot = self.slot.swap(usize::MAX, Ordering::AcqRel);
      if slot != usize::MAX {
        vm.release_native_root(slot);
      }
    }

    self.free_code();

    unsafe {
      // The reference `new` gave the closure.
      drop(Arc::from_raw(self as *const CallbackCore));
    }
  }

  fn free_code(&self) {
    #[cfg(target_arch = "x86_64")]
    drop(self.wide.lock().unwrap().take());

    unsafe { raw::ffi_closure_free(self.closure as *mut c_void) };
  }

  /// Releases without a VM at hand: the closure is freed and the
  /// function stays rooted until its VM ends.
  pub fn release(&self) {
    if self.released.swap(true, Ordering::AcqRel) {
      return;
    }

    self.free_code();

    unsafe { drop(Arc::from_raw(self as *const CallbackCore)) };
  }

  /// # Safety
  ///
  /// `ret` must have room for the signature's return value.
  unsafe fn write_error_value(&self, ret: *mut c_void) {
    if self.sig.returns.is_void() {
      return;
    }
    let len = self.cif.return_size.min(self.error_value.len());
    unsafe { std::ptr::copy_nonoverlapping(self.error_value.as_ptr(), ret as *mut u8, len) };
  }
}

unsafe extern "C" fn trampoline(
  _cif: *mut raw::ffi_cif,
  ret: *mut c_void,
  args: *mut *mut c_void,
  data: *mut c_void,
) {
  let core = unsafe { &*(data as *const CallbackCore) };

  // Entered through `wide_return`: the result buffer comes first, ahead
  // of the callback's own arguments, and the result goes there.
  let (ret, args) = if core.cif.wide_return {
    unsafe { (*(*args as *const *mut c_void), args.add(1)) }
  } else {
    (ret, args)
  };

  let outcome = catch_unwind(AssertUnwindSafe(|| unsafe { dispatch(core, ret, args) }));

  if outcome.is_err() {
    unsafe { core.write_error_value(ret) };
  }
}

unsafe fn dispatch(core: &CallbackCore, ret: *mut c_void, args: *mut *mut c_void) {
  if core.is_released() || RELEASING.with(|slot| slot.get()) {
    unsafe { core.write_error_value(ret) };
    return;
  }

  let active = ACTIVE.with(|slot| slot.get());

  if !active.is_null() && active as usize == core.owner_vm {
    let vm = unsafe { &mut *active };
    unsafe { run_here(vm, core, args, ret) };
    return;
  }

  if std::thread::current().id() == core.owner.thread {
    // The owning thread, but not from inside a call this module made,
    // so the VM is in no state to run anything and C gets the error
    // value.
    unsafe { core.write_error_value(ret) };
    return;
  }

  // Keeping a reference for the request, which may outlive this frame's
  // view of `core` if the callback is released while the call waits.
  let arc = unsafe {
    Arc::increment_strong_count(core as *const CallbackCore);
    Arc::from_raw(core as *const CallbackCore)
  };

  if !core.owner.post_and_wait(&arc, args, ret) {
    unsafe { core.write_error_value(ret) };
  }
}

/// Runs the callback's Zuri function on `vm` with the C arguments at
/// `args`, writing its result to `ret`.
unsafe fn run_here(vm: &mut VM, core: &CallbackCore, args: *mut *mut c_void, ret: *mut c_void) {
  // Published for anything the function itself calls into C, which may
  // call back again.
  let previous = ACTIVE.with(|slot| slot.replace(vm as *mut VM));
  let mark = vm.pin_values([]);

  let outcome = (|| -> Result<(), Fail> {
    let mut values = Vec::with_capacity(core.sig.params.len());

    for (cif_index, slot) in core.cif.slots.iter().enumerate() {
      let Some(param) = slot else {
        continue;
      };
      let ty = &core.sig.params[*param];
      let src = unsafe { *args.add(cif_index) } as *const u8;
      let value = convert::load(vm, ty, src)?;
      values.push(vm.pin_values([value]));
    }

    let arguments: Vec<Value> = values.iter().map(|p| vm.pinned(*p)).collect();
    let function = vm.native_root(core.slot.load(Ordering::Acquire));

    let result = vm.call_value(function, &arguments).map_err(Fail::Raised)?;

    if !core.sig.returns.is_void() {
      store_return(vm, &core.sig, result, ret as *mut u8)?;
    }

    Ok(())
  })();

  if let Err(fail) = outcome {
    let error = match fail {
      Fail::Raised(error) => error,
      other => {
        super::raise(vm, other);
        vm.take_pending_error().unwrap_or(Value::nil())
      },
    };
    if !error.is_nil() {
      trap(vm, error);
    }
    unsafe { core.write_error_value(ret) };
  }

  vm.unpin(mark);
  ACTIVE.with(|slot| slot.set(previous));
}

/// Writes a callback's result for C. A small integer is widened to a
/// full register the way a C compiler returns it, which callers rely on.
fn store_return(vm: &mut VM, sig: &Signature, value: Value, ret: *mut u8) -> Result<(), Fail> {
  let ty = &sig.returns;

  let widen = match &ty.kind {
    Kind::Int { size, signed, .. } if *size < 8 => Some((*size, *signed)),
    Kind::Bool => Some((1, false)),
    Kind::RustChar => Some((4, false)),
    Kind::Enum(e) => match e.underlying.kind {
      Kind::Int { size, signed, .. } if size < 8 => Some((size, signed)),
      _ => None,
    },
    _ => None,
  };

  let mut dest = Dest::Memory;

  let Some((size, signed)) = widen else {
    return convert::store(vm, ty, value, ret, &mut dest).map_err(|fail| returning(fail, ty));
  };

  let mut narrow = [0u8; 16];
  convert::store(vm, ty, value, narrow.as_mut_ptr(), &mut dest)
    .map_err(|fail| returning(fail, ty))?;

  let mut wide = [0u8; 8];
  wide[..size].copy_from_slice(&narrow[..size]);
  if signed && narrow[size - 1] & 0x80 != 0 {
    for b in wide.iter_mut().skip(size) {
      *b = 0xff;
    }
  }

  unsafe { std::ptr::copy_nonoverlapping(wide.as_ptr(), ret, 8) };
  Ok(())
}

fn returning(fail: Fail, ty: &super::types::CType) -> Fail {
  match fail {
    Fail::Builtin(class, message) => Fail::Builtin(
      class,
      format!("a callback returning '{}': {message}", ty.name),
    ),
    other => other,
  }
}
