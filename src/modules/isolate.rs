//! `_isolate` builtin module; native backing for `libs/isolate.zu`.
//!
//! Real concurrency, not cooperative-only: isolates run on a small,
//! configurable pool of actual OS threads (see `isolate_util::pool`),
//! each with its own fully independent `VM`/`Heap`; this VM's object
//! model (raw heap pointers, non-atomic inline caches, a `thread_local!`
//! GC remembered set) was never built to be shared across threads, so
//! nothing here shares one. A isolate's arguments and return value
//! cross thread boundaries as a heap-independent snapshot instead (see
//! `isolate_util::transfer`), and its spawn target is looked up by
//! name against a freshly-loaded copy of its own defining module/entry
//! script on whichever isolate picks it up.

use std::sync::Arc;
use std::time::Duration;

use crate::builtins::enforce::ArgType;
use crate::modules::isolate_util::{pool, transfer};
use crate::modules::{BuiltinModuleDef, native};
use crate::vm::object::{ListStorage, ZuriContext};
use crate::vm::value::Value;
use crate::vm::vm::VM;
use crate::{enforce_arg_count, enforce_arg_ptr, enforce_arg_type, enforce_arg_type_any_of};

pub static MODULE: BuiltinModuleDef = BuiltinModuleDef {
  name: "_isolate",
  build,
};

fn build(vm: &mut VM) -> Vec<(&'static str, Value)> {
  vec![
    ("configure", native(vm, "configure", 1, false, configure)),
    ("pool_size", native(vm, "pool_size", 0, false, pool_size)),
    ("cpu_count", native(vm, "cpu_count", 0, false, cpu_count)),
    ("shutdown", native(vm, "shutdown", 1, false, shutdown)),
    (
      "active_count",
      native(vm, "active_count", 0, false, active_count),
    ),
    (
      "queued_count",
      native(vm, "queued_count", 0, false, queued_count),
    ),
    ("is_shutdown", native(vm, "is_shutdown", 0, false, is_shutdown)),
    ("spawn", native(vm, "spawn", 3, false, spawn)),
    ("map", native(vm, "map", 3, false, map_batch)),
    ("join", native(vm, "join", 2, false, join)),
    ("try_join", native(vm, "try_join", 1, false, try_join)),
    ("is_done", native(vm, "is_done", 1, false, is_done)),
    ("status", native(vm, "status", 1, false, status)),
    ("name", native(vm, "name", 1, false, name)),
    ("cancel", native(vm, "cancel", 1, false, cancel)),
    (
      "is_cancelled",
      native(vm, "is_cancelled", 1, false, is_cancelled),
    ),
    (
      "current_is_cancelled",
      native(vm, "current_is_cancelled", 0, false, current_is_cancelled),
    ),
    ("wait_any", native(vm, "wait_any", 2, false, wait_any)),
    ("wait_all", native(vm, "wait_all", 2, false, wait_all)),
    ("select", native(vm, "select", 2, false, select)),
    (
      "channel_new",
      native(vm, "channel_new", 1, false, channel_new),
    ),
    (
      "channel_send",
      native(vm, "channel_send", 3, false, channel_send),
    ),
    (
      "channel_recv",
      native(vm, "channel_recv", 2, false, channel_recv),
    ),
    (
      "channel_try_recv",
      native(vm, "channel_try_recv", 1, false, channel_try_recv),
    ),
    (
      "channel_close",
      native(vm, "channel_close", 1, false, channel_close),
    ),
    (
      "channel_is_closed",
      native(vm, "channel_is_closed", 1, false, channel_is_closed),
    ),
    (
      "channel_len",
      native(vm, "channel_len", 1, false, channel_len),
    ),
    #[cfg(debug_assertions)]
    (
      "debug_panic",
      native(vm, "debug_panic", 0, false, debug_panic),
    ),
  ]
}

// ---------------------------------------------------------------------
// Small shared helpers
// ---------------------------------------------------------------------

/// Builds the `[status, value]` pair every isolate/channel query
/// native returns. `value` already exists (it's whatever
/// `transfer::materialize` or a plain leaf alloc just produced) but
/// sits only in a local Rust variable; pin it across the two further
/// allocations needed to assemble the pair (`status`'s own string,
/// then the list itself), so a collection triggered by either can't
/// invalidate it. Mirrors `VM::instantiate`'s own pin-across-
/// allocation pattern.
fn status_pair(vm: &mut VM, status: &'static str, value: Value) -> Value {
  let p = vm.pin_values([value]);
  let status_val = vm.heap_mut().alloc_string(status);
  let value = vm.pinned(p);
  let result = vm.heap_mut().alloc_list(vec![status_val, value]);
  vm.unpin(p);
  result
}

/// Like `status_pair`, but for `select`'s three-element result; the
/// winning channel's index alongside its status and value. `index` is
/// always a plain number (or `nil` on timeout), never a heap value, so
/// unlike `value` it needs no pinning of its own.
fn select_result(vm: &mut VM, index: Value, status: &'static str, value: Value) -> Value {
  let p = vm.pin_values([value]);
  let status_val = vm.heap_mut().alloc_string(status);
  let value = vm.pinned(p);
  let result = vm.heap_mut().alloc_list(vec![index, status_val, value]);
  vm.unpin(p);
  result
}

fn isolate_state_of(ctx: &ZuriContext, idx: usize) -> Result<Arc<pool::IsolateState>, String> {
  isolate_state_of_value(ctx.args[idx])
}

fn isolate_state_of_value(v: Value) -> Result<Arc<pool::IsolateState>, String> {
  let cell = v.as_ptr_cell();
  let borrowed = cell.borrow();
  borrowed
    .downcast_ref::<Arc<pool::IsolateState>>()
    .cloned()
    .ok_or_else(|| "invalid isolate handle".to_string())
}

fn channel_state_of(ctx: &ZuriContext, idx: usize) -> Result<Arc<pool::ChannelState>, String> {
  channel_state_of_value(ctx.args[idx])
}

fn channel_state_of_value(v: Value) -> Result<Arc<pool::ChannelState>, String> {
  let cell = v.as_ptr_cell();
  let borrowed = cell.borrow();
  borrowed
    .downcast_ref::<Arc<pool::ChannelState>>()
    .cloned()
    .ok_or_else(|| "invalid channel handle".to_string())
}

/// Reads an optional `timeout` argument; `nil` (the `.zu` side's
/// default for an omitted parameter) means "no timeout", anything else
/// must be a non-negative number of seconds.
fn optional_timeout(ctx: &ZuriContext, idx: usize) -> Result<Option<Duration>, String> {
  enforce_arg_type_any_of!(ctx, idx, [ArgType::Number, ArgType::Nil]);
  let v = ctx.args[idx];
  if v.is_nil() {
    return Ok(None);
  }
  let secs = v.as_number();
  if !secs.is_finite() || secs < 0.0 {
    return Err(format!(
      "{}() expects a non-negative timeout, got {}",
      ctx.name, secs
    ));
  }
  Ok(Some(Duration::from_secs_f64(secs)))
}

// ---------------------------------------------------------------------
// Pool sizing
// ---------------------------------------------------------------------

fn configure(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 1);
  enforce_arg_type!(ctx, 0, ArgType::Number);
  let n = ctx.args[0].as_number();
  if n.fract() != 0.0 || n < 1.0 {
    return Err(format!(
      "{}() expects a positive whole number, got {}",
      ctx.name, n
    ));
  }
  Ok(Value::bool(pool::configure(n as usize)?))
}

fn pool_size(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 0);
  Ok(Value::number(pool::pool_size() as f64))
}

fn cpu_count(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 0);
  Ok(Value::number(pool::cpu_count() as f64))
}

fn shutdown(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 1);
  let timeout = optional_timeout(ctx, 0)?;
  Ok(Value::bool(pool::shutdown(timeout)))
}

fn active_count(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 0);
  Ok(Value::number(pool::active_count() as f64))
}

fn queued_count(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 0);
  Ok(Value::number(pool::queued_count() as f64))
}

fn is_shutdown(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 0);
  Ok(Value::bool(pool::is_shutdown()))
}

// ---------------------------------------------------------------------
// Isolates
// ---------------------------------------------------------------------

/// Returns `["error", message]` rather than a Rust `Err` for anything
/// that goes wrong; unlike every OTHER native in this codebase,
/// `spawn()`'s failures are all genuine `IsolateError` territory
/// (an un-transferable argument, a bad spawn target), and only the
/// `.zu` wrapper can raise that specific class (see the module docs
/// on why `IsolateError` lives in Zuri source, not the prelude).
/// Matches `join`/`channel_recv`/etc.'s own `[status, value]` shape.
fn spawn(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 3);
  enforce_arg_type!(ctx, 0, ArgType::Function);
  enforce_arg_type!(ctx, 1, ArgType::List);
  enforce_arg_type_any_of!(ctx, 2, [ArgType::String, ArgType::Nil]);
  let fn_val = ctx.args[0];
  if !fn_val.is_closure() && !fn_val.is_bound_method() {
    let msg = ctx.vm.heap_mut().alloc_string(format!(
      "{}() expects a plain function, method, or bound method but not a native or a class",
      ctx.name
    ));
    return Ok(status_pair(ctx.vm, "error", msg));
  }
  let name = (!ctx.args[2].is_nil()).then(|| ctx.args[2].as_str().to_string());
  match pool::spawn(ctx.vm, fn_val, ctx.args[1], name) {
    Ok(state) => {
      let ptr = ctx.vm.heap_mut().alloc_ptr(pool::ISOLATE_PTR_TYPE, state);
      Ok(status_pair(ctx.vm, "ok", ptr))
    },
    Err(msg) => {
      let value = ctx.vm.heap_mut().alloc_string(msg);
      Ok(status_pair(ctx.vm, "error", value))
    },
  }
}

fn map_batch(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 3);
  enforce_arg_type!(ctx, 0, ArgType::Function);
  enforce_arg_type!(ctx, 1, ArgType::List);
  enforce_arg_type_any_of!(ctx, 2, [ArgType::Number, ArgType::Nil]);

  let fn_val = ctx.args[0];
  if !fn_val.is_closure() && !fn_val.is_bound_method() {
    let msg = ctx.vm.heap_mut().alloc_string(format!(
      "{}() expects a plain function, method, or bound method but not a native or a class",
      ctx.name
    ));
    return Ok(status_pair(ctx.vm, "error", msg));
  }

  let items = ctx.args[1].as_list();
  if items.is_empty() {
    let empty_list = ctx.vm.heap_mut().alloc_list(Vec::new());
    return Ok(status_pair(ctx.vm, "ok", empty_list));
  }

  let timeout = optional_timeout(ctx, 2)?;
  let states = match pool::spawn_batch(ctx.vm, fn_val, &items) {
    Ok(s) => s,
    Err(msg) => {
      let value = ctx.vm.heap_mut().alloc_string(msg);
      return Ok(status_pair(ctx.vm, "error", value));
    },
  };

  match pool::wait_all_isolates(&states, timeout) {
    pool::WaitAllOutcome::TimedOut => Ok(status_pair(ctx.vm, "timeout", Value::nil())),
    pool::WaitAllOutcome::Cancelled => Ok(status_pair(ctx.vm, "cancelled", Value::nil())),
    pool::WaitAllOutcome::Ready => {
      // One pinned root (the list itself) instead of one per result:
      // `vm.gc_pins` gets a full linear scan on every GC, so pinning
      // every materialized value individually made a large map() pay
      // O(pin count) root-scan cost on each of the many collections a
      // long join loop triggers. Writing straight into the list's own
      // slots is exactly as safe as pinning each value: `list_set`
      // calls `write_barrier` itself, so it's correct whether the list
      // is still young or has already been promoted to old partway
      // through the loop.
      let count = states.len();
      let list_val = ctx
        .vm
        .heap_mut()
        .alloc_list(ListStorage::from_elem(Value::nil(), count));
      let pin_mark = ctx.vm.pin_values([list_val]);
      for (idx, s) in states.iter().enumerate() {
        match s.join() {
          pool::JoinOutcome::Ok(graph) => {
            let val = transfer::materialize(ctx.vm, &graph)?;
            list_val.list_set(idx, val);
          },
          pool::JoinOutcome::Err(msg) => {
            ctx.vm.unpin(pin_mark);
            let value = ctx.vm.heap_mut().alloc_string(msg);
            return Ok(status_pair(ctx.vm, "error", value));
          },
          pool::JoinOutcome::Pending | pool::JoinOutcome::Cancelled => unreachable!(),
        }
      }
      ctx.vm.unpin(pin_mark);
      Ok(status_pair(ctx.vm, "ok", list_val))
    },
  }
}

fn join(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 2);
  enforce_arg_ptr!(ctx, 0, pool::ISOLATE_PTR_TYPE);
  let timeout = optional_timeout(ctx, 1)?;
  let state = isolate_state_of(ctx, 0)?;
  let outcome = match timeout {
    Some(d) => state.join_timeout(d),
    None => state.join(),
  };
  match outcome {
    pool::JoinOutcome::Pending => Ok(status_pair(ctx.vm, "pending", Value::nil())),
    pool::JoinOutcome::Cancelled => Ok(status_pair(ctx.vm, "cancelled", Value::nil())),
    pool::JoinOutcome::Ok(graph) => {
      let value = transfer::materialize(ctx.vm, &graph)?;
      Ok(status_pair(ctx.vm, "ok", value))
    },
    pool::JoinOutcome::Err(msg) => {
      let value = ctx.vm.heap_mut().alloc_string(msg);
      Ok(status_pair(ctx.vm, "error", value))
    },
  }
}

fn try_join(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 1);
  enforce_arg_ptr!(ctx, 0, pool::ISOLATE_PTR_TYPE);
  let state = isolate_state_of(ctx, 0)?;
  match state.try_join() {
    pool::JoinOutcome::Pending => Ok(status_pair(ctx.vm, "pending", Value::nil())),
    pool::JoinOutcome::Ok(graph) => {
      let value = transfer::materialize(ctx.vm, &graph)?;
      Ok(status_pair(ctx.vm, "ok", value))
    },
    pool::JoinOutcome::Err(msg) => {
      let value = ctx.vm.heap_mut().alloc_string(msg);
      Ok(status_pair(ctx.vm, "error", value))
    },
    pool::JoinOutcome::Cancelled => {
      unreachable!("try_join never blocks, so it can never observe the CALLER being cancelled")
    },
  }
}

fn is_done(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 1);
  enforce_arg_ptr!(ctx, 0, pool::ISOLATE_PTR_TYPE);
  let state = isolate_state_of(ctx, 0)?;
  Ok(Value::bool(state.is_done()))
}

fn status(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 1);
  enforce_arg_ptr!(ctx, 0, pool::ISOLATE_PTR_TYPE);
  let state = isolate_state_of(ctx, 0)?;
  let s = match state.peek() {
    pool::JoinOutcome::Pending => "pending",
    pool::JoinOutcome::Ok(_) => "done",
    pool::JoinOutcome::Err(_) => "error",
    pool::JoinOutcome::Cancelled => {
      unreachable!("peek never blocks, so it can never observe the CALLER being cancelled")
    },
  };
  Ok(ctx.vm.heap_mut().alloc_string(s))
}

fn name(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 1);
  enforce_arg_ptr!(ctx, 0, pool::ISOLATE_PTR_TYPE);
  let state = isolate_state_of(ctx, 0)?;
  match state.name() {
    Some(n) => Ok(ctx.vm.heap_mut().alloc_string(n)),
    None => Ok(Value::nil()),
  }
}

fn cancel(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 1);
  enforce_arg_ptr!(ctx, 0, pool::ISOLATE_PTR_TYPE);
  let state = isolate_state_of(ctx, 0)?;
  state.cancel();
  Ok(Value::nil())
}

fn is_cancelled(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 1);
  enforce_arg_ptr!(ctx, 0, pool::ISOLATE_PTR_TYPE);
  let state = isolate_state_of(ctx, 0)?;
  Ok(Value::bool(state.is_cancelled()))
}

/// Backs the ambient `isolate.is_cancelled()`; checks the isolate
/// the CALLING isolate thread is currently running, found via
/// `pool::is_current_cancelled()`'s thread-local rather than a handle
/// argument. Outside a isolate thread it's always `false`.
fn current_is_cancelled(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 0);
  Ok(Value::bool(pool::is_current_cancelled()))
}

fn wait_any(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 2);
  enforce_arg_type!(ctx, 0, ArgType::List);
  let timeout = optional_timeout(ctx, 1)?;
  let items = ctx.args[0].as_list();
  let mut states = Vec::with_capacity(items.len());
  for v in &items {
    if !v.is_ptr_type(pool::ISOLATE_PTR_TYPE) {
      return Err(format!(
        "{}() expects a list of isolates, got a value of type {}",
        ctx.name,
        v.type_name()
      ));
    }
    states.push(isolate_state_of_value(*v)?);
  }
  match pool::wait_any_isolates(&states, timeout) {
    pool::WaitAnyOutcome::Ready(i) => Ok(status_pair(ctx.vm, "ok", Value::number(i as f64))),
    pool::WaitAnyOutcome::TimedOut => Ok(status_pair(ctx.vm, "timeout", Value::nil())),
    pool::WaitAnyOutcome::Cancelled => Ok(status_pair(ctx.vm, "cancelled", Value::nil())),
  }
}

fn wait_all(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 2);
  enforce_arg_type!(ctx, 0, ArgType::List);
  let timeout = optional_timeout(ctx, 1)?;
  let items = ctx.args[0].as_list();
  let mut states = Vec::with_capacity(items.len());
  for v in &items {
    if !v.is_ptr_type(pool::ISOLATE_PTR_TYPE) {
      return Err(format!(
        "{}() expects a list of isolates, got a value of type {}",
        ctx.name,
        v.type_name()
      ));
    }
    states.push(isolate_state_of_value(*v)?);
  }
  let status = match pool::wait_all_isolates(&states, timeout) {
    pool::WaitAllOutcome::Ready => "ok",
    pool::WaitAllOutcome::TimedOut => "timeout",
    pool::WaitAllOutcome::Cancelled => "cancelled",
  };
  Ok(ctx.vm.heap_mut().alloc_string(status))
}

// ---------------------------------------------------------------------
// Channels
// ---------------------------------------------------------------------

fn channel_new(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 1);
  enforce_arg_type!(ctx, 0, ArgType::Number);
  let raw = ctx.args[0].as_number();
  let capacity = if raw <= 0.0 { None } else { Some(raw as usize) };
  let state = Arc::new(pool::ChannelState::new(capacity));
  Ok(ctx.vm.heap_mut().alloc_ptr(pool::CHANNEL_PTR_TYPE, state))
}

fn channel_send(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 3);
  enforce_arg_ptr!(ctx, 0, pool::CHANNEL_PTR_TYPE);
  let timeout = optional_timeout(ctx, 2)?;
  let state = channel_state_of(ctx, 0)?;
  let graph = transfer::capture(ctx.vm, ctx.args[1])?;
  let outcome = match timeout {
    Some(d) => state.send_timeout(graph, d),
    None => state.send(graph),
  };
  let status = match outcome {
    pool::SendOutcome::Sent => "ok",
    pool::SendOutcome::Closed => "closed",
    pool::SendOutcome::TimedOut => "timeout",
    pool::SendOutcome::Cancelled => "cancelled",
  };
  Ok(ctx.vm.heap_mut().alloc_string(status))
}

fn channel_recv(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 2);
  enforce_arg_ptr!(ctx, 0, pool::CHANNEL_PTR_TYPE);
  let timeout = optional_timeout(ctx, 1)?;
  let state = channel_state_of(ctx, 0)?;
  let outcome = match timeout {
    Some(d) => state.recv_timeout(d),
    None => state.recv(),
  };
  match outcome {
    pool::RecvOutcome::TimedOut => Ok(status_pair(ctx.vm, "timeout", Value::nil())),
    pool::RecvOutcome::Cancelled => Ok(status_pair(ctx.vm, "cancelled", Value::nil())),
    pool::RecvOutcome::Value(graph) => {
      let value = transfer::materialize(ctx.vm, &graph)?;
      Ok(status_pair(ctx.vm, "ok", value))
    },
    pool::RecvOutcome::Closed => Ok(status_pair(ctx.vm, "closed", Value::nil())),
  }
}

fn select(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 2);
  enforce_arg_type!(ctx, 0, ArgType::List);
  let timeout = optional_timeout(ctx, 1)?;
  let items = ctx.args[0].as_list();
  let mut states = Vec::with_capacity(items.len());
  for v in &items {
    if !v.is_ptr_type(pool::CHANNEL_PTR_TYPE) {
      return Err(format!(
        "{}() expects a list of channels, got a value of type {}",
        ctx.name,
        v.type_name()
      ));
    }
    states.push(channel_state_of_value(*v)?);
  }
  match pool::select_channels(&states, timeout) {
    pool::SelectOutcome::TimedOut => {
      Ok(select_result(ctx.vm, Value::nil(), "timeout", Value::nil()))
    },
    pool::SelectOutcome::Cancelled => Ok(select_result(
      ctx.vm,
      Value::nil(),
      "cancelled",
      Value::nil(),
    )),
    pool::SelectOutcome::Ready(i, pool::RecvOutcome::Value(graph)) => {
      let value = transfer::materialize(ctx.vm, &graph)?;
      let index = Value::number(i as f64);
      Ok(select_result(ctx.vm, index, "ok", value))
    },
    pool::SelectOutcome::Ready(i, pool::RecvOutcome::Closed) => {
      let index = Value::number(i as f64);
      Ok(select_result(ctx.vm, index, "closed", Value::nil()))
    },
    pool::SelectOutcome::Ready(_, pool::RecvOutcome::TimedOut | pool::RecvOutcome::Cancelled) => {
      unreachable!(
        "try_recv never produces TimedOut/Cancelled; select_channels only calls try_recv"
      )
    },
  }
}

fn channel_try_recv(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 1);
  enforce_arg_ptr!(ctx, 0, pool::CHANNEL_PTR_TYPE);
  let state = channel_state_of(ctx, 0)?;
  match state.try_recv() {
    None => Ok(status_pair(ctx.vm, "empty", Value::nil())),
    Some(pool::RecvOutcome::Value(graph)) => {
      let value = transfer::materialize(ctx.vm, &graph)?;
      Ok(status_pair(ctx.vm, "ok", value))
    },
    Some(pool::RecvOutcome::Closed) => Ok(status_pair(ctx.vm, "closed", Value::nil())),
    Some(pool::RecvOutcome::TimedOut | pool::RecvOutcome::Cancelled) => {
      unreachable!("try_recv never blocks, so it never produces TimedOut/Cancelled")
    },
  }
}

fn channel_close(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 1);
  enforce_arg_ptr!(ctx, 0, pool::CHANNEL_PTR_TYPE);
  let state = channel_state_of(ctx, 0)?;
  state.close();
  Ok(Value::nil())
}

fn channel_is_closed(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 1);
  enforce_arg_ptr!(ctx, 0, pool::CHANNEL_PTR_TYPE);
  let state = channel_state_of(ctx, 0)?;
  Ok(Value::bool(state.is_closed()))
}

fn channel_len(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 1);
  enforce_arg_ptr!(ctx, 0, pool::CHANNEL_PTR_TYPE);
  let state = channel_state_of(ctx, 0)?;
  Ok(Value::number(state.len() as f64))
}

// ---------------------------------------------------------------------
// Debug-only
// ---------------------------------------------------------------------

/// Deliberately panics; exists ONLY so the library test suite has a
/// way to verify, end to end, that a Rust-level panic inside a
/// isolate's own execution is isolated to that one isolate rather
/// than taking down the whole process (see `pool::isolate_loop`).
/// `#[cfg(debug_assertions)]`, so this never exists in a release
/// build; there is no way to reach it from a shipped binary.
#[cfg(debug_assertions)]
fn debug_panic(_ctx: &mut ZuriContext) -> Result<Value, String> {
  panic!("_isolate.debug_panic() was called")
}
