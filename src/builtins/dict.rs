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
  let dict = ctx.args[0];
  let len = dict.dict_len();

  if val.is_nil() {
    if len == 0 {
      return Ok(Value::bool(false));
    }
    return Ok(dict.dict_key_at(0).unwrap());
  }

  if let Some(index) = dict.dict_index_of(&val) {
    if index < len - 1 {
      return Ok(dict.dict_key_at(index + 1).unwrap());
    }
  }

  Ok(Value::nil())
}

fn _value(ctx: &mut ZuriContext) -> Result<Value, String> {
  let key = ctx.args[1];
  let dict = ctx.args[0];

  if let Some(index) = dict.dict_index_of(&key) {
    return Ok(dict.dict_value_at(index).unwrap());
  }

  Ok(Value::nil())
}
