#![allow(unused)]

use std::sync::LazyLock;

use crate::{
  builtins::{MethodTable, build, method, to_string},
  vm::{object::ZuriContext, value::Value},
};

pub static NUMBER_METHODS: LazyLock<MethodTable> = LazyLock::new(|| {
  build(vec![
    // methods
    method("to_string", to_string),
    method("abs", abs),
  ])
});

fn abs(ctx: &mut ZuriContext) -> Result<Value, String> {
  Ok(Value::number(ctx.args[0].as_number().abs()))
}
