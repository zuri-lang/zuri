use std::sync::LazyLock;

use num_traits::{Signed, ToPrimitive};

use crate::{
  builtins::{
    MethodTable, build,
    enforce::{ArgType, enforce_method_arg_count},
    method, method_n, to_string,
  },
  enforce_arg_type,
  vm::{object::ZuriContext, value::Value},
};

pub static BIGINT_METHODS: LazyLock<MethodTable> = LazyLock::new(|| {
  build(vec![
    // methods
    method("to_string", to_string),
    method("to_number", to_number),
    method("abs", abs),
    method_n("max", 1, max),
  ])
});

fn to_number(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  Ok(Value::number(
    ctx.args[0].as_bigint().to_f64().unwrap_or(0.0),
  ))
}

fn abs(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  Ok(
    ctx
      .vm
      .heap_mut()
      .alloc_bigint(ctx.args[0].as_bigint().abs()),
  )
}

fn max(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  enforce_arg_type!(ctx, 1, ArgType::BigInt);

  let max = ctx.args[0].as_bigint().max(ctx.args[1].as_bigint());

  Ok(ctx.vm.heap_mut().alloc_bigint(max.clone()))
}
