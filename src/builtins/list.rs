#![allow(unused)]

use std::sync::LazyLock;

use crate::{
  builtins::{MethodTable, build, method, method_n, to_string},
  vm::{object::ZuriContext, value::Value},
};

pub static LIST_METHODS: LazyLock<MethodTable> = LazyLock::new(|| {
  build(vec![
    // methods
    method_n("@key", 1, _key),
    method_n("@value", 1, _value),
    method("to_string", to_string),
    method("length", length),
  ])
});

fn length(ctx: &mut ZuriContext) -> Result<Value, String> {
  Ok(Value::number(ctx.args[0].list_len() as f64))
}

fn _key(ctx: &mut ZuriContext) -> Result<Value, String> {
  let val = ctx.args[1];
  let obj = ctx.args[0].as_list();

  if obj.is_empty() {
    return Ok(Value::bool(false));
  }

  if val.is_nil() {
    return Ok(Value::number(0.0));
  }

  if !val.is_number() {
    return Err(format!(
      "lists are numerically indexed, {} given",
      val.type_name()
    ));
  }

  let index = val.as_number() as usize;
  if (index < obj.len() - 1) {
    return Ok(Value::number(index as f64 + 1.0));
  }

  Ok(Value::nil())
}

fn _value(ctx: &mut ZuriContext) -> Result<Value, String> {
  if !ctx.args[1].is_number() {
    return Err("lists are numerically indexed".to_string());
  }

  let index = ctx.args[1].as_number();
  let obj = ctx.args[0].as_list();

  if (index > -1.0 && index < obj.len() as f64) {
    let v = obj.get(index as usize).unwrap();
    return Ok(v.clone());
  }

  Ok(Value::nil())
}
