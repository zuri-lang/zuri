use crate::vm::object::{NativeFn, NativeFunction, ZuriContext};
use crate::vm::value::Value;
use crate::vm::vm::VM;
use std::time::{SystemTime, UNIX_EPOCH};

pub fn install(vm: &mut VM) {
  register(vm, "time", 0, false, time);
  register(vm, "abs", 1, false, abs);
  register(vm, "sum", 1, true, sum);
  register(vm, "bytes", 1, false, bytes);
  // register(vm, "gc", 0, false, gc);
}

fn register(vm: &mut VM, name: &'static str, min_arity: u8, variadic: bool, func: NativeFn) {
  let native = NativeFunction {
    name,
    min_arity,
    variadic,
    func,
  };
  let value = vm.heap.alloc_native(native);
  vm.define_global(name, value);
}

fn time(_ctx: &mut ZuriContext) -> Result<Value, String> {
  let now = SystemTime::now().duration_since(UNIX_EPOCH).map_err(|e| {
    format!(
      "time() failed: system clock is set before the Unix epoch: {}",
      e
    )
  })?;
  Ok(Value::number(now.as_secs_f64()))
}

fn abs(ctx: &mut ZuriContext) -> Result<Value, String> {
  let v = ctx.args[0];
  if !v.is_number() {
    return Err(format!("abs() expects a number, got {}", v.type_name()));
  }
  Ok(Value::number(v.as_number().abs()))
}

fn sum(ctx: &mut ZuriContext) -> Result<Value, String> {
  let mut total = 0.0;
  for (i, v) in ctx.args.iter().enumerate() {
    if !v.is_number() {
      return Err(format!(
        "sum() expects numbers, argument {} is a {}",
        i + 1,
        v.type_name()
      ));
    }
    total += v.as_number();
  }
  Ok(Value::number(total))
}

fn bytes(ctx: &mut ZuriContext) -> Result<Value, String> {
  let v = ctx.args[0];
  if v.is_number() {
    let bytes = ctx.heap().alloc_bytes(vec![0; v.as_number() as usize]);
    return Ok(bytes);
  } else if v.is_list() {
    let is_valid_list = v
      .as_list()
      .iter()
      .all(|f| f.is_number() && 0.0 <= f.as_number() && f.as_number() <= 255.0);

    if !is_valid_list {
      return Err(format!(
        "bytes() expects a list of numbers, got {}",
        v.type_name()
      ));
    }

    let bytes = ctx.heap().alloc_bytes(
      v.as_list()
        .iter()
        .map(|f| f.as_number() as u8)
        .collect::<Vec<_>>(),
    );

    return Ok(bytes);
  }

  return Err(format!(
    "bytes() expects a number or list, got {}",
    v.type_name()
  ));
}

/// Force an immediate mark-and-sweep collection, bypassing the usual
/// allocation-threshold heuristic. Mainly useful for exercising or
/// benchmarking the collector directly from a script -- set the
/// ZURI_GC_LOG environment variable before running to see what each
/// collection actually freed.
#[allow(unused)]
fn gc(ctx: &mut ZuriContext) -> Result<Value, String> {
  ctx.vm.collect_garbage();
  Ok(Value::nil())
}
