#![allow(unused)]

use std::sync::LazyLock;

use crate::{
  builtins::{MethodTable, build, method, method_n, to_string},
  vm::{object::ZuriContext, value::Value},
};

pub static RANGE_METHODS: LazyLock<MethodTable> = LazyLock::new(|| {
  build(vec![
    // methods
    method_n("@key", 1, _key),
    method_n("@value", 1, _value),
    method("to_string", to_string),
  ])
});

fn _key(ctx: &mut ZuriContext) -> Result<Value, String> {
  let val = ctx.args[1];
  let (lower, upper) = ctx.args[0].as_range();
  let range = if lower > upper {
    lower - upper
  } else {
    upper - lower
  } as usize;

  if range == 0 {
    return Ok(Value::bool(false));
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

  let index = val.as_number() as usize;
  if (index < range - 1) {
    return Ok(Value::number(index as f64 + 1.0));
  }

  Ok(Value::nil())
}

fn _value(ctx: &mut ZuriContext) -> Result<Value, String> {
  if !ctx.args[1].is_number() {
    return Err("ranges are numerically indexed".to_string());
  }

  let index = ctx.args[1].as_number() as usize;
  let (lower, upper) = ctx.args[0].as_range();
  let range = if lower > upper {
    lower - upper
  } else {
    upper - lower
  } as usize;

  if index < range {
    if upper > lower {
      return Ok(Value::number(index as f64 + lower));
    } else {
      return Ok(Value::number(lower - index as f64));
    }
  }

  Ok(Value::nil())
}
