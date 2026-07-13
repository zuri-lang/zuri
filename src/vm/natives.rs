use crate::vm::object::{Heap, NativeFn, NativeFunction};
use crate::vm::value::Value;
use crate::vm::vm::VM;
use std::time::{SystemTime, UNIX_EPOCH};

pub fn install(vm: &mut VM) {
  register(vm, "time", 0, false, native_time);
  register(vm, "abs", 1, false, native_abs);
  register(vm, "sum", 1, true, native_sum);
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

fn native_time(_heap: &mut Heap, _args: &[Value]) -> Result<Value, String> {
  let now = SystemTime::now().duration_since(UNIX_EPOCH).map_err(|e| {
    format!(
      "time() failed: system clock is set before the Unix epoch: {}",
      e
    )
  })?;
  Ok(Value::number(now.as_secs_f64()))
}

fn native_abs(_heap: &mut Heap, args: &[Value]) -> Result<Value, String> {
  let v = args[0];
  if !v.is_number() {
    return Err(format!("abs() expects a number, got {}", v.type_name()));
  }
  Ok(Value::number(v.as_number().abs()))
}

fn native_sum(_heap: &mut Heap, args: &[Value]) -> Result<Value, String> {
  let mut total = 0.0;
  for (i, v) in args.iter().enumerate() {
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
