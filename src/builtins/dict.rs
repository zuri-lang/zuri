#![allow(unused)]

use std::{ops::Index, sync::LazyLock};

use crate::{
  builtins::{MethodTable, build, method, method_n, to_string},
  vm::{object::ZuriContext, value::Value},
};

pub static DICT_METHODS: LazyLock<MethodTable> = LazyLock::new(|| {
  build(vec![
    // methods
    method_n("@key", 1, _key),
    method_n("@value", 1, _value),
    method("to_string", to_string),
    method("length", length),
  ])
});

fn length(ctx: &mut ZuriContext) -> Result<Value, String> {
  Ok(Value::number(ctx.args[0].dict_len() as f64))
}

fn _key(ctx: &mut ZuriContext) -> Result<Value, String> {
  let val = ctx.args[1];
  let (keys, _): (Vec<Value>, Vec<Value>) = ctx.args[0].as_dict().into_iter().unzip();

  if val.is_nil() {
    if keys.is_empty() {
      return Ok(Value::bool(false));
    }

    return Ok(keys.first().unwrap().clone());
  }

  if let Some(index) = keys.iter().position(|&v| v.equals(&val)) {
    if index < keys.len() - 1 {
      return Ok(keys[index + 1]);
    }
  }

  Ok(Value::nil())
}

fn _value(ctx: &mut ZuriContext) -> Result<Value, String> {
  let key = ctx.args[1];
  let (keys, values): (Vec<Value>, Vec<Value>) = ctx.args[0].as_dict().into_iter().unzip();

  if let Some(index) = keys.iter().position(|&v| v.equals(&key)) {
    if index < keys.len() {
      return Ok(values[index]);
    }
  }

  Ok(Value::nil())
}
