use std::sync::LazyLock;

use crate::{
  builtins::{
    MethodTable, build,
    enforce::{ArgType, enforce_method_arg_count},
    method, method_n, to_string,
  },
  enforce_arg_type,
  vm::{object::ZuriContext, value::Value},
};

pub static NUMBER_METHODS: LazyLock<MethodTable> = LazyLock::new(|| {
  build(vec![
    // methods
    method("to_string", to_string),
    method("abs", abs),
    method_n("max", 1, max),
  ])
});

fn abs(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  Ok(Value::number(ctx.args[0].as_number().abs()))
}

fn max(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  enforce_arg_type!(ctx, 1, ArgType::Number);

  Ok(Value::number(
    ctx.args[0].as_number().max(ctx.args[1].as_number()),
  ))
}
