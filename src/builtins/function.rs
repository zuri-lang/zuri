#![allow(unused)]

use std::sync::LazyLock;

use crate::{
  builtins::{
    MethodTable, build,
    enforce::{ArgType, enforce_method_arg_count, enforce_method_arg_type},
    method, method_n, method_opt, to_string,
  },
  vm::{
    object::{Obj, ZuriContext},
    value::Value,
  },
};

pub static FUNCTION_METHODS: LazyLock<MethodTable> = LazyLock::new(|| {
  build(vec![
    // methods
    method("to_string", to_string),
    method("arity", arity),
    method("is_variadic", is_variadic),
    method("name", name),
    method_opt("call", 0, call),
    method_n("apply", 1, apply),
  ])
});

fn call(ctx: &mut ZuriContext) -> Result<Value, String> {
  let val = ctx
    .vm
    .call_value(ctx.args[0], &ctx.args[1..])
    .map_err(|e| ctx.vm.rethrow(e))?;

  Ok(val)
}

/// `call`'s sibling for an argument list that isn't known until
/// runtime. Zuri has no spread at a call site, so a wrapper that has
/// to forward whatever it was handed (a spy, a decorator, a
/// table-driven test runner) has no way to express the call at all
/// without this.
///
/// Building the argument slice here is GC-safe without pinning for
/// the same reason `call`'s is: allocation alone never collects (see
/// `Heap::alloc_sized`), collection only happens at the VM's own
/// safepoints, and `call_value` copies every argument into the
/// callee's registers before a single bytecode instruction of it
/// runs.
fn apply(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  enforce_method_arg_type!(ctx, 1, ArgType::List);

  let args: Vec<Value> = ctx.args[1].as_list().into_iter().collect();

  ctx
    .vm
    .call_value(ctx.args[0], &args)
    .map_err(|e| ctx.vm.rethrow(e))
}

fn arity(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);

  let v = ctx.args[0];
  let r = match unsafe { &*v.as_obj() } {
    Obj::Closure(x) => v.as_closure().function.as_func().arity,
    Obj::BoundMethod(x) => {
      let method = v.as_bound_method().method;
      match unsafe { &*method.as_obj() } {
        Obj::Closure(x) => method.as_closure().function.as_func().arity,
        _ => method.as_native().min_arity,
      }
    },
    Obj::Native(x) => v.as_native().min_arity,
    Obj::Func(x) => v.as_func().arity,
    _ => 0,
  };

  Ok(Value::number(r as f64))
}

fn is_variadic(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);

  let v = ctx.args[0];
  let r = match unsafe { &*v.as_obj() } {
    Obj::Closure(x) => v.as_closure().function.as_func().variadic,
    Obj::BoundMethod(x) => {
      let method = v.as_bound_method().method;
      match unsafe { &*method.as_obj() } {
        Obj::Closure(x) => method.as_closure().function.as_func().variadic,
        _ => method.as_native().variadic,
      }
    },
    Obj::Native(x) => v.as_native().variadic,
    Obj::Func(x) => v.as_func().variadic,
    _ => false,
  };

  Ok(Value::bool(r))
}

fn name(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);

  let v = ctx.args[0];
  let r = match unsafe { &*v.as_obj() } {
    Obj::Closure(x) => &v.as_closure().function.as_func().name,
    Obj::BoundMethod(x) => {
      let method = v.as_bound_method().method;
      &match unsafe { &*method.as_obj() } {
        Obj::Closure(x) => method.as_closure().function.as_func().name.clone(),
        _ => method.as_native().name.to_string().clone(),
      }
    },
    Obj::Native(x) => &v.as_native().name.to_string(),
    Obj::Func(x) => &v.as_func().name,
    _ => &String::new(),
  };

  Ok(ctx.heap().alloc_string(r))
}
