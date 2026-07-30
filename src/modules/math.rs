use crate::modules::BuiltinModuleDef;
use crate::vm::value::Value;
use crate::vm::vm::VM;

pub static MODULE: BuiltinModuleDef = BuiltinModuleDef {
  name: "math",
  build,
};

/// Re-exports the already globally-registered `sum` native under the
/// `math` namespace -- preserves `import math; math.sum(...)` exactly
/// as it worked before this registry existed.
fn build(vm: &mut VM) -> Vec<(&'static str, Value)> {
  let mut members = Vec::new();
  if let Some(v) = vm.lookup_global("sum") {
    members.push(("sum", v));
  }
  members
}
