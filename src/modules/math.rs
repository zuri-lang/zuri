use crate::modules::BuiltinModuleDef;
use crate::vm::value::Value;
use crate::vm::vm::VM;

pub static MODULE: BuiltinModuleDef = BuiltinModuleDef {
  name: "_math",
  build,
};

/// Re-exports the already globally-registered `sum` native under the
/// `math` namespace -- preserves `import math; math.sum(...)` exactly
/// as it worked before this registry existed.
fn build(_: &mut VM) -> Vec<(&'static str, Value)> {
  vec![
    ("Infinity", Value::number(f64::INFINITY)),
    ("NaN", Value::number(f64::NAN)),
  ]
}
