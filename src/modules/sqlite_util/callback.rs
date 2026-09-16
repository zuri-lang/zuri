//! The path from SQLite back into Zuri: user-defined functions,
//! aggregates, collations, hooks, the authorizer and the progress
//! handler.
//!
//! Every one of these is invoked by SQLite from inside a call this
//! module made, which makes two things true of all of them.
//!
//! A Rust panic must not cross back into C, so each trampoline catches
//! its own unwind and turns it into an error for SQLite.
//!
//! A Zuri error cannot be raised from here either, for the same reason.
//! It is parked on the connection's `ConnState` instead and re-raised
//! by whichever native was driving SQLite, once control is back on this
//! side of the boundary. Once one callback on a connection has failed
//! every later one fails immediately without re-entering Zuri: the
//! statement is already being torn down, and running more of the
//! program's code during that teardown would bury the original error.

use std::cell::Cell;
use std::marker::PhantomData;
use std::os::raw::{c_char, c_int, c_void};
use std::panic::{AssertUnwindSafe, catch_unwind};

use libsqlite3_sys as ffi;

use super::{ConnState, cstr_to_string, value};
use crate::vm::value::Value;
use crate::vm::vm::VM;

thread_local! {
  /// The VM a trampoline should call back into, valid only while a
  /// native on this thread is inside a SQLite call that can re-enter.
  /// Null at every other moment, which is what a stray callback (one
  /// SQLite runs during its own shutdown, say) sees.
  static ACTIVE_VM: Cell<*mut VM> = const { Cell::new(std::ptr::null_mut()) };
}

/// Publishes `vm` for the trampolines to find, restoring whatever was
/// published before once it drops.
///
/// Save and restore rather than set and clear because these nest: a
/// user-defined function is free to run a query of its own, and the
/// inner call must not leave the outer one without a VM when it ends.
///
/// The borrow it holds is deliberate. It lasts exactly as long as the
/// guard, so the compiler refuses any use of the same `&mut VM` while
/// SQLite might be handing it to a callback, which is what keeps the
/// two paths from aliasing.
pub struct VmGuard<'a> {
  previous: *mut VM,
  owner: PhantomData<&'a mut VM>,
}

impl Drop for VmGuard<'_> {
  fn drop(&mut self) {
    ACTIVE_VM.with(|slot| slot.set(self.previous));
  }
}

pub fn enter_vm(vm: &mut VM) -> VmGuard<'_> {
  let previous = ACTIVE_VM.with(|slot| slot.replace(vm as *mut VM));

  VmGuard {
    previous,
    owner: PhantomData,
  }
}

/// Runs `body` against the published VM, or returns `None` when there
/// is none, which means SQLite called back at a moment no native was
/// driving it.
fn with_vm<R>(body: impl FnOnce(&mut VM) -> R) -> Option<R> {
  let ptr = ACTIVE_VM.with(|slot| slot.get());

  if ptr.is_null() {
    return None;
  }

  // SAFETY: the pointer was taken from the `&mut VM` that `enter_vm`
  // borrowed, and that borrow is still live (the guard has not
  // dropped, or the pointer would be back to what it replaced). The
  // borrow is what stops the native that published it from touching
  // the same VM while this reborrow exists.
  Some(body(unsafe { &mut *ptr }))
}

/// What a registration is for. Function and collation registrations are
/// addressed by name and arity when replaced or deleted; the hooks are
/// one per connection and replace whatever was there.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Kind {
  Scalar,
  Aggregate,
  Collation,
  UpdateHook,
  CommitHook,
  RollbackHook,
  Authorizer,
  Progress,
}

/// One thing registered against a connection.
///
/// SQLite holds the address of this as the callback's user data, which
/// is why `ConnState` keeps these boxed: the address has to survive the
/// vector growing.
pub struct Registration {
  pub state: *mut ConnState,
  pub kind: Kind,
  /// The slot in `VM::native_roots` holding the Zuri callable.
  pub slot: usize,
  /// An aggregate's second callable, the one that produces the final
  /// value. `None` for everything else.
  pub final_slot: Option<usize>,
  /// Name and arity for the registrations addressed by them, so
  /// replacing `soundex(1)` does not disturb `soundex(2)`.
  pub name: String,
  pub arity: i32,
}

impl Registration {
  /// Gives the VM back the slots this held. Called when the
  /// registration is replaced, deleted, or the connection closes.
  pub fn release(&self, vm: &mut VM) {
    vm.release_native_root(self.slot);

    if let Some(slot) = self.final_slot {
      vm.release_native_root(slot);
    }
  }

  /// Whether this registration is the one a replacement would displace.
  pub fn same_target(&self, kind: Kind, name: &str, arity: i32) -> bool {
    match kind {
      Kind::Scalar | Kind::Aggregate | Kind::Collation => {
        // A name can be registered as a scalar and later replaced by an
        // aggregate of the same arity; SQLite treats that as one entry,
        // so this deliberately does not compare `kind` for functions.
        let function = matches!(self.kind, Kind::Scalar | Kind::Aggregate);
        let replacing_function = matches!(kind, Kind::Scalar | Kind::Aggregate);

        if function && replacing_function {
          return self.name == name && self.arity == arity;
        }

        self.kind == kind && self.name == name && self.arity == arity
      }
      _ => self.kind == kind,
    }
  }
}

/// Reads the registration back out of the user-data pointer SQLite
/// hands every callback.
///
/// # Safety
///
/// `data` must be the address of a live `Registration`, which is what
/// every registration path here passes and what `ConnState` keeps alive
/// until the connection closes.
unsafe fn registration<'a>(data: *mut c_void) -> Option<&'a mut Registration> {
  if data.is_null() {
    return None;
  }

  Some(unsafe { &mut *(data as *mut Registration) })
}

/// Parks a Zuri error on the connection, so the native driving SQLite
/// re-raises it once it is back in charge.
fn trap(registration: &Registration, vm: &mut VM, error: Value) {
  let message = vm.describe_error(error);

  // SAFETY: `state` points at the `ConnState` box owned by the
  // connection this registration belongs to, which outlives every
  // callback made against it.
  let state = unsafe { &mut *registration.state };
  state.trap(vm, error, message);
}

/// True once this connection has a trapped error, meaning no further
/// callback should run.
fn poisoned(registration: &Registration) -> bool {
  // SAFETY: as in `trap`.
  unsafe { &*registration.state }.poisoned()
}

/// Calls a registered Zuri callable with `args`, parking anything it
/// raises. `None` means the call failed and the caller should report an
/// error to SQLite rather than produce a value.
fn invoke(registration: &Registration, slot: usize, args: Vec<Value>) -> Option<Value> {
  if poisoned(registration) {
    return None;
  }

  with_vm(|vm| {
    let callable = vm.native_root(slot);

    if callable.is_nil() {
      return None;
    }

    match vm.call_value(callable, &args) {
      Ok(value) => Some(value),
      Err(error) => {
        trap(registration, vm, error);
        None
      }
    }
  })
  .flatten()
}

/// Collects the arguments SQLite passed into Zuri values.
///
/// # Safety
///
/// `argv` must point at `argc` value pointers, which is SQLite's
/// contract for every function callback.
unsafe fn arguments(vm: &mut VM, argc: c_int, argv: *mut *mut ffi::sqlite3_value) -> Vec<Value> {
  let mut args = Vec::with_capacity(argc as usize);

  for index in 0..argc as isize {
    let arg = unsafe { *argv.offset(index) };
    args.push(value::argument(vm, arg));
  }

  args
}

/// The text a failed callback reports to SQLite.
///
/// The real error is already parked and about to be re-raised with its
/// own class intact, so this only has to be recognisable if it ever
/// surfaces on its own.
const CALLBACK_FAILED: &str = "a zuri callback raised";
const CALLBACK_PANICKED: &str = "a zuri callback panicked";

/// `xFunc` for a scalar user-defined function.
pub unsafe extern "C" fn scalar(
  context: *mut ffi::sqlite3_context,
  argc: c_int,
  argv: *mut *mut ffi::sqlite3_value,
) {
  let outcome = catch_unwind(AssertUnwindSafe(|| {
    let data = unsafe { ffi::sqlite3_user_data(context) };
    let Some(registration) = (unsafe { registration(data) }) else {
      return false;
    };

    if poisoned(registration) {
      return false;
    }

    let args = with_vm(|vm| unsafe { arguments(vm, argc, argv) });
    let Some(args) = args else {
      return false;
    };

    match invoke(registration, registration.slot, args) {
      Some(result) => {
        value::result(context, result);
        true
      }
      None => false,
    }
  }));

  match outcome {
    Ok(true) => {}
    Ok(false) => value::set_error(context, CALLBACK_FAILED),
    Err(_) => value::set_error(context, CALLBACK_PANICKED),
  }
}

/// Per-group scratch for an aggregate, living in the memory SQLite
/// allocates through `sqlite3_aggregate_context`.
///
/// It holds the accumulator's root slot, biased by one so that the
/// zeroed memory SQLite hands back on the first step reads as "nothing
/// yet" rather than as slot zero.
#[repr(C)]
struct AggregateCell {
  slot_plus_one: usize,
}

/// The scratch for the group currently being aggregated, allocating it
/// on first use. Null only if SQLite could not allocate.
unsafe fn aggregate_cell(context: *mut ffi::sqlite3_context) -> *mut AggregateCell {
  unsafe {
    ffi::sqlite3_aggregate_context(context, std::mem::size_of::<AggregateCell>() as c_int)
      as *mut AggregateCell
  }
}

/// `xStep` for an aggregate.
///
/// The Zuri side is a fold: the callable is passed the accumulator so
/// far followed by the row's arguments, and returns the next
/// accumulator. The accumulator starts as `nil`.
pub unsafe extern "C" fn aggregate_step(
  context: *mut ffi::sqlite3_context,
  argc: c_int,
  argv: *mut *mut ffi::sqlite3_value,
) {
  let outcome = catch_unwind(AssertUnwindSafe(|| {
    let data = unsafe { ffi::sqlite3_user_data(context) };
    let Some(registration) = (unsafe { registration(data) }) else {
      return false;
    };

    if poisoned(registration) {
      return false;
    }

    let cell = unsafe { aggregate_cell(context) };
    if cell.is_null() {
      return false;
    }

    let previous = unsafe { (*cell).slot_plus_one };

    let args = with_vm(|vm| {
      let accumulator = if previous == 0 {
        Value::nil()
      } else {
        vm.native_root(previous - 1)
      };

      let mut args = Vec::with_capacity(argc as usize + 1);
      args.push(accumulator);
      args.extend(unsafe { arguments(vm, argc, argv) });
      args
    });

    let Some(args) = args else {
      return false;
    };

    let Some(next) = invoke(registration, registration.slot, args) else {
      return false;
    };

    // The accumulator has to stay rooted between steps: nothing else
    // refers to it, and a collection can run inside the next step's own
    // callback. Reusing the group's existing slot rather than taking a
    // fresh one keeps this to one slot per group rather than one per
    // row.
    with_vm(|vm| {
      if previous == 0 {
        let slot = vm.retain_native_root(next);
        unsafe { (*cell).slot_plus_one = slot + 1 };
      } else {
        vm.replace_native_root(previous - 1, next);
      }
    });

    true
  }));

  match outcome {
    Ok(true) => {}
    Ok(false) => value::set_error(context, CALLBACK_FAILED),
    Err(_) => value::set_error(context, CALLBACK_PANICKED),
  }
}

/// `xFinal` for an aggregate.
///
/// SQLite calls this once per group even when no row ever reached
/// `xStep` (an aggregate over an empty set), in which case the
/// accumulator is still `nil` and the group has no slot to release.
pub unsafe extern "C" fn aggregate_final(context: *mut ffi::sqlite3_context) {
  let outcome = catch_unwind(AssertUnwindSafe(|| {
    let data = unsafe { ffi::sqlite3_user_data(context) };
    let Some(registration) = (unsafe { registration(data) }) else {
      return false;
    };

    // `sqlite3_aggregate_context` with a size of zero asks for whatever
    // is already there without allocating, which is what distinguishes a
    // group that stepped from one that never did.
    let cell = unsafe { ffi::sqlite3_aggregate_context(context, 0) as *mut AggregateCell };
    let previous = if cell.is_null() {
      0
    } else {
      unsafe { (*cell).slot_plus_one }
    };

    let accumulator = with_vm(|vm| {
      if previous == 0 {
        Value::nil()
      } else {
        vm.native_root(previous - 1)
      }
    });

    let Some(accumulator) = accumulator else {
      return false;
    };

    let finished = registration
      .final_slot
      .and_then(|slot| invoke(registration, slot, vec![accumulator]));

    // Released whichever way the call went: the group is over, and a
    // failed aggregate still has to give its slot back.
    if previous != 0 {
      with_vm(|vm| vm.release_native_root(previous - 1));
      unsafe { (*cell).slot_plus_one = 0 };
    }

    match finished {
      Some(result) => {
        value::result(context, result);
        true
      }
      None => false,
    }
  }));

  match outcome {
    Ok(true) => {}
    Ok(false) => value::set_error(context, CALLBACK_FAILED),
    Err(_) => value::set_error(context, CALLBACK_PANICKED),
  }
}

/// `xCompare` for a custom collation.
///
/// Returns zero when the callback cannot run, which SQLite reads as
/// "equal". There is no way to report a failure from a collation, so
/// the trapped error is what actually surfaces; treating everything as
/// equal keeps the sort stable and finite until it does.
pub unsafe extern "C" fn collation(
  data: *mut c_void,
  left_len: c_int,
  left: *const c_void,
  right_len: c_int,
  right: *const c_void,
) -> c_int {
  let outcome = catch_unwind(AssertUnwindSafe(|| {
    let Some(registration) = (unsafe { registration(data) }) else {
      return 0;
    };

    if poisoned(registration) {
      return 0;
    }

    let left = unsafe { slice_text(left, left_len) };
    let right = unsafe { slice_text(right, right_len) };

    let args = with_vm(|vm| {
      vec![
        vm.heap_mut().alloc_string(left),
        vm.heap_mut().alloc_string(right),
      ]
    });

    let Some(args) = args else {
      return 0;
    };

    match invoke(registration, registration.slot, args) {
      Some(result) if result.is_number() => {
        let ordering = result.as_number();

        if ordering < 0.0 {
          -1
        } else if ordering > 0.0 {
          1
        } else {
          0
        }
      }
      _ => 0,
    }
  }));

  outcome.unwrap_or(0)
}

/// Reads one side of a collation comparison. SQLite passes the length
/// separately and the text is not NUL terminated.
unsafe fn slice_text(ptr: *const c_void, len: c_int) -> String {
  if ptr.is_null() || len <= 0 {
    return String::new();
  }

  let bytes = unsafe { std::slice::from_raw_parts(ptr as *const u8, len as usize) };
  String::from_utf8_lossy(bytes).into_owned()
}

/// `sqlite3_update_hook`, called after each row a statement inserts,
/// updates or deletes.
pub unsafe extern "C" fn update_hook(
  data: *mut c_void,
  operation: c_int,
  database: *const c_char,
  table: *const c_char,
  rowid: i64,
) {
  let _ = catch_unwind(AssertUnwindSafe(|| {
    let Some(registration) = (unsafe { registration(data) }) else {
      return;
    };

    if poisoned(registration) {
      return;
    }

    let action = match operation {
      ffi::SQLITE_INSERT => "insert",
      ffi::SQLITE_UPDATE => "update",
      ffi::SQLITE_DELETE => "delete",
      _ => "unknown",
    };

    let database = unsafe { cstr_to_string(database) };
    let table = unsafe { cstr_to_string(table) };

    let args = with_vm(|vm| {
      let action = vm.heap_mut().alloc_string(action);
      let database = vm.heap_mut().alloc_string(database);
      let table = vm.heap_mut().alloc_string(table);
      let rowid = value::integer_value_vm(vm, rowid);

      vec![action, database, table, rowid]
    });

    if let Some(args) = args {
      invoke(registration, registration.slot, args);
    }
  }));
}

/// `sqlite3_commit_hook`. A callback returning `false` vetoes the
/// commit and turns it into a rollback; anything else lets it through.
pub unsafe extern "C" fn commit_hook(data: *mut c_void) -> c_int {
  let outcome = catch_unwind(AssertUnwindSafe(|| {
    let Some(registration) = (unsafe { registration(data) }) else {
      return 0;
    };

    if poisoned(registration) {
      return 1;
    }

    match invoke(registration, registration.slot, Vec::new()) {
      Some(result) => {
        if result.is_bool() && !result.as_bool() {
          1
        } else {
          0
        }
      }
      // The callback failed. Vetoing the commit is the safe reading:
      // the error is about to be raised, and letting a transaction
      // through on the back of a failed check would be worse.
      None => 1,
    }
  }));

  outcome.unwrap_or(1)
}

/// `sqlite3_rollback_hook`.
pub unsafe extern "C" fn rollback_hook(data: *mut c_void) {
  let _ = catch_unwind(AssertUnwindSafe(|| {
    let Some(registration) = (unsafe { registration(data) }) else {
      return;
    };

    if poisoned(registration) {
      return;
    }

    invoke(registration, registration.slot, Vec::new());
  }));
}

/// `sqlite3_set_authorizer`, consulted as a statement is prepared.
///
/// The callback is passed the action name and the five strings SQLite
/// supplies about it, and answers with `'allow'`, `'deny'` or
/// `'ignore'`. Anything else counts as denial, so a callback that
/// returns nothing by accident fails closed.
pub unsafe extern "C" fn authorizer(
  data: *mut c_void,
  action: c_int,
  first: *const c_char,
  second: *const c_char,
  database: *const c_char,
  trigger: *const c_char,
) -> c_int {
  let outcome = catch_unwind(AssertUnwindSafe(|| {
    let Some(registration) = (unsafe { registration(data) }) else {
      return ffi::SQLITE_OK;
    };

    if poisoned(registration) {
      return ffi::SQLITE_DENY;
    }

    let first = unsafe { cstr_to_string(first) };
    let second = unsafe { cstr_to_string(second) };
    let database = unsafe { cstr_to_string(database) };
    let trigger = unsafe { cstr_to_string(trigger) };

    let args = with_vm(|vm| {
      let action = Value::number(action as f64);
      let first = vm.heap_mut().alloc_string(first);
      let second = vm.heap_mut().alloc_string(second);
      let database = vm.heap_mut().alloc_string(database);
      let trigger = vm.heap_mut().alloc_string(trigger);

      vec![action, first, second, database, trigger]
    });

    let Some(args) = args else {
      return ffi::SQLITE_DENY;
    };

    match invoke(registration, registration.slot, args) {
      Some(result) if result.is_string() => match result.as_str() {
        "allow" => ffi::SQLITE_OK,
        "ignore" => ffi::SQLITE_IGNORE,
        _ => ffi::SQLITE_DENY,
      },
      _ => ffi::SQLITE_DENY,
    }
  }));

  outcome.unwrap_or(ffi::SQLITE_DENY)
}

/// `sqlite3_progress_handler`, called every so many virtual machine
/// instructions during a long-running statement. A callback returning
/// `false` interrupts it.
pub unsafe extern "C" fn progress(data: *mut c_void) -> c_int {
  let outcome = catch_unwind(AssertUnwindSafe(|| {
    let Some(registration) = (unsafe { registration(data) }) else {
      return 0;
    };

    if poisoned(registration) {
      return 1;
    }

    match invoke(registration, registration.slot, Vec::new()) {
      Some(result) => {
        if result.is_bool() && !result.as_bool() {
          1
        } else {
          0
        }
      }
      None => 1,
    }
  }));

  outcome.unwrap_or(1)
}
