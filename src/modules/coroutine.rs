//! `_coroutine` builtin module -- native backing for `libs/coroutine.zu`.
//!
//! Real concurrency, not cooperative-only: coroutines run on a small,
//! configurable pool of actual OS threads (see `coroutine_util::pool`),
//! each with its own fully independent `VM`/`Heap` -- this VM's object
//! model (raw heap pointers, non-atomic inline caches, a `thread_local!`
//! GC remembered set) was never built to be shared across threads, so
//! nothing here shares one. A coroutine's arguments and return value
//! cross thread boundaries as a heap-independent snapshot instead (see
//! `coroutine_util::transfer`), and its spawn target is looked up by
//! name against a freshly-loaded copy of its own defining module/entry
//! script on whichever worker picks it up.

use std::sync::Arc;

use crate::builtins::enforce::ArgType;
use crate::modules::coroutine_util::{pool, transfer};
use crate::modules::{BuiltinModuleDef, native};
use crate::vm::object::ZuriContext;
use crate::vm::value::Value;
use crate::vm::vm::VM;
use crate::{enforce_arg_count, enforce_arg_ptr, enforce_arg_type};

pub static MODULE: BuiltinModuleDef = BuiltinModuleDef {
  name: "_coroutine",
  build,
};

fn build(vm: &mut VM) -> Vec<(&'static str, Value)> {
  vec![
    ("configure", native(vm, "configure", 1, false, configure)),
    ("pool_size", native(vm, "pool_size", 0, false, pool_size)),
    ("cpu_count", native(vm, "cpu_count", 0, false, cpu_count)),
    ("spawn", native(vm, "spawn", 2, false, spawn)),
    ("join", native(vm, "join", 1, false, join)),
    ("try_join", native(vm, "try_join", 1, false, try_join)),
    ("is_done", native(vm, "is_done", 1, false, is_done)),
    (
      "channel_new",
      native(vm, "channel_new", 1, false, channel_new),
    ),
    (
      "channel_send",
      native(vm, "channel_send", 2, false, channel_send),
    ),
    (
      "channel_recv",
      native(vm, "channel_recv", 1, false, channel_recv),
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
  ]
}

// ---------------------------------------------------------------------
// Small shared helpers
// ---------------------------------------------------------------------

/// Builds the `[status, value]` pair every coroutine/channel query
/// native returns. `value` already exists (it's whatever
/// `transfer::materialize` or a plain leaf alloc just produced) but
/// sits only in a local Rust variable -- pin it across the two further
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

fn coroutine_state_of(ctx: &ZuriContext, idx: usize) -> Result<Arc<pool::CoroutineState>, String> {
  let cell = ctx.args[idx].as_ptr_cell();
  let borrowed = cell.borrow();
  borrowed
    .downcast_ref::<Arc<pool::CoroutineState>>()
    .cloned()
    .ok_or_else(|| "invalid coroutine handle".to_string())
}

fn channel_state_of(ctx: &ZuriContext, idx: usize) -> Result<Arc<pool::ChannelState>, String> {
  let cell = ctx.args[idx].as_ptr_cell();
  let borrowed = cell.borrow();
  borrowed
    .downcast_ref::<Arc<pool::ChannelState>>()
    .cloned()
    .ok_or_else(|| "invalid channel handle".to_string())
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
      "configure() expects a positive whole number, got {}",
      n
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

// ---------------------------------------------------------------------
// Coroutines
// ---------------------------------------------------------------------

/// Returns `["error", message]` rather than a Rust `Err` for anything
/// that goes wrong -- unlike every OTHER native in this codebase,
/// `spawn()`'s failures are all genuine `CoroutineError` territory
/// (an un-transferable argument, a bad spawn target), and only the
/// `.zu` wrapper can raise that specific class (see the module docs
/// on why `CoroutineError` lives in Zuri source, not the prelude).
/// Matches `join`/`channel_recv`/etc.'s own `[status, value]` shape.
fn spawn(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 2);
  enforce_arg_type!(ctx, 0, ArgType::Function);
  enforce_arg_type!(ctx, 1, ArgType::List);
  let fn_val = ctx.args[0];
  if !fn_val.is_closure() && !fn_val.is_bound_method() {
    let msg = ctx.vm.heap_mut().alloc_string(
      "spawn() expects a plain function, method, or bound method -- not a native or a class",
    );
    return Ok(status_pair(ctx.vm, "error", msg));
  }
  match pool::spawn(ctx.vm, fn_val, ctx.args[1]) {
    Ok(state) => {
      let ptr = ctx.vm.heap_mut().alloc_ptr(pool::COROUTINE_PTR_TYPE, state);
      Ok(status_pair(ctx.vm, "ok", ptr))
    },
    Err(msg) => {
      let value = ctx.vm.heap_mut().alloc_string(msg);
      Ok(status_pair(ctx.vm, "error", value))
    },
  }
}

fn join(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 1);
  enforce_arg_ptr!(ctx, 0, pool::COROUTINE_PTR_TYPE);
  let state = coroutine_state_of(ctx, 0)?;
  match state.join() {
    pool::JoinOutcome::Pending => unreachable!("join() always blocks until finished"),
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
  enforce_arg_ptr!(ctx, 0, pool::COROUTINE_PTR_TYPE);
  let state = coroutine_state_of(ctx, 0)?;
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
  }
}

fn is_done(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 1);
  enforce_arg_ptr!(ctx, 0, pool::COROUTINE_PTR_TYPE);
  let state = coroutine_state_of(ctx, 0)?;
  Ok(Value::bool(state.is_done()))
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
  enforce_arg_count!(ctx, 2);
  enforce_arg_ptr!(ctx, 0, pool::CHANNEL_PTR_TYPE);
  let state = channel_state_of(ctx, 0)?;
  let graph = transfer::capture(ctx.vm, ctx.args[1])?;
  Ok(Value::bool(state.send(graph).is_ok()))
}

fn channel_recv(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 1);
  enforce_arg_ptr!(ctx, 0, pool::CHANNEL_PTR_TYPE);
  let state = channel_state_of(ctx, 0)?;
  match state.recv() {
    pool::RecvOutcome::Value(graph) => {
      let value = transfer::materialize(ctx.vm, &graph)?;
      Ok(status_pair(ctx.vm, "ok", value))
    },
    pool::RecvOutcome::Closed => Ok(status_pair(ctx.vm, "closed", Value::nil())),
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
