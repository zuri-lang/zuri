#![allow(unused)]

use std::sync::LazyLock;

use crate::{
  builtins::{MethodTable, build, method, method_opt, to_string},
  vm::{object::ZuriContext, value::Value},
};

pub static FUNCTION_METHODS: LazyLock<MethodTable> = LazyLock::new(|| {
  build(vec![
    // methods
    method("to_string", to_string),
    method_opt("call", 0, call),
  ])
});

fn call(ctx: &mut ZuriContext) -> Result<Value, String> {
  let val = ctx
    .vm
    .call_value(ctx.args[0], &ctx.args[1..])
    .map_err(|e| ctx.vm.describe_exception(e))?;

  Ok(val)
}
