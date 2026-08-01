#![allow(unused)]

use std::sync::LazyLock;

use crate::{
  builtins::{
    MethodTable, build,
    enforce::{ArgType, enforce_method_arg_count, enforce_method_arg_type},
    method, method_n, to_string,
  },
  vm::{object::ZuriContext, value::Value},
};

pub static RANGE_METHODS: LazyLock<MethodTable> = LazyLock::new(|| {
  build(vec![
    // iterable protocol
    method_n("@key", 1, _key),
    method_n("@value", 1, _value),
    method("to_string", to_string),
    method("to_list", to_list),
    method("lower", lower),
    method("upper", upper),
    method("range", range_span),
    method_n("within", 1, within),
    method_n("loop", 1, loop_fn),
    method_n("step", 1, step_fn),
    method("get_step", get_step),
  ])
});

fn to_list(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);

  let (lower, upper) = ctx.args[0].as_range();
  let mut list = if upper > lower {
    ((lower as i64)..(upper as i64)).collect::<Vec<_>>()
  } else if lower >= 0.0 {
    ((upper as i64 + 1)..(lower as i64 + 1))
      .rev()
      .collect::<Vec<_>>()
  } else {
    ((upper as i64 + 1)..(lower as i64))
      .rev()
      .collect::<Vec<_>>()
  };

  let v = list
    .into_iter()
    .map(|x| Value::number(x as f64))
    .collect::<Vec<_>>();

  Ok(ctx.vm.heap_mut().alloc_list(v))
}

fn lower(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  let (l, _) = ctx.args[0].as_range();
  Ok(Value::number(l))
}

fn upper(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  let (_, u) = ctx.args[0].as_range();
  Ok(Value::number(u))
}

/// Count of numbers between the range's bounds -- direction-
/// independent (per spec, swapping bounds gives the same result).
fn range_span(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  let (l, u) = ctx.args[0].as_range();
  Ok(Value::number((u - l).abs()))
}

fn within(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  enforce_method_arg_type!(ctx, 1, ArgType::Number);

  let (l, u) = ctx.args[0].as_range();
  let (lo, hi) = if l <= u { (l, u) } else { (u, l) };
  let x = ctx.args[1].as_number();
  Ok(Value::bool(x >= lo && x <= hi))
}

/// Iterates from the lower bound up to (but not including) the upper
/// bound, regardless of the range's own written direction -- matching
/// the documented example where `(25..18).loop(...)` counts DOWN from
/// 25 to 19 (not up).
fn loop_fn(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  enforce_method_arg_type!(ctx, 1, ArgType::Function);

  let (l, u) = ctx.args[0].as_range();
  let callback = ctx.args[1];

  if l <= u {
    let mut i = l;
    while i < u {
      ctx
        .vm
        .call_value(callback, &[Value::number(i)])
        .map_err(|e| ctx.vm.describe_exception(e))?;
      i += 1.0;
    }
  } else {
    let mut i = l;
    while i > u {
      ctx
        .vm
        .call_value(callback, &[Value::number(i)])
        .map_err(|e| ctx.vm.describe_exception(e))?;
      i -= 1.0;
    }
  }

  Ok(Value::nil())
}

fn step_fn(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  enforce_method_arg_type!(ctx, 1, ArgType::Number);

  let size = ctx.args[1].as_number();
  if size <= 0.0 {
    return Err(format!(
      "'{}' expects a positive step size, got {}",
      ctx.name, size
    ));
  }
  ctx.args[0].range_set_step(size);
  Ok(ctx.args[0])
}

fn get_step(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  Ok(Value::number(ctx.args[0].range_step()))
}

//-----------------------------------------------------------------------------------
// Iterable Decorators (@key / @value)
//-----------------------------------------------------------------------------------

/// `index` here is an internal 0-based ITERATION COUNT (not a raw
/// value in the range) -- `_value` below turns it into the actual
/// number by scaling it by `step` and offsetting from `lower`. This
/// mirrors the pre-existing (step-less) behavior exactly when
/// `step == 1.0`.
fn _key(ctx: &mut ZuriContext) -> Result<Value, String> {
  let val = ctx.args[1];
  let (lower, upper) = ctx.args[0].as_range();
  let step = ctx.args[0].range_step();
  let step = if step > 0.0 { step } else { 1.0 };
  let width = (upper - lower).abs();
  let count = (width / step).ceil() as i64;

  if count <= 0 {
    return Ok(Value::nil());
  }

  if val.is_nil() {
    return Ok(Value::number(0.0));
  }

  if !val.is_number() {
    return Err(format!(
      "ranges are numerically indexed, {} given",
      val.type_name()
    ));
  }

  let index = val.as_number() as i64;
  if index < count - 1 {
    return Ok(Value::number(index as f64 + 1.0));
  }

  Ok(Value::nil())
}

fn _value(ctx: &mut ZuriContext) -> Result<Value, String> {
  if !ctx.args[1].is_number() {
    return Err("ranges are numerically indexed".to_string());
  }

  let index = ctx.args[1].as_number();
  let (lower, upper) = ctx.args[0].as_range();
  let step = ctx.args[0].range_step();
  let step = if step > 0.0 { step } else { 1.0 };
  let width = (upper - lower).abs();
  let count = (width / step).ceil();

  if index >= 0.0 && index < count {
    if upper >= lower {
      Ok(Value::number(lower + index * step))
    } else {
      Ok(Value::number(lower - index * step))
    }
  } else {
    Ok(Value::nil())
  }
}
