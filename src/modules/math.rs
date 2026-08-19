use crate::modules::BuiltinModuleDef;
use crate::vm::value::Value;
use crate::vm::vm::VM;

pub static MODULE: BuiltinModuleDef = BuiltinModuleDef {
  name: "_math",
  build,
};

fn build(_: &mut VM) -> Vec<(&'static str, Value)> {
  vec![
    ("Infinity", Value::number(f64::INFINITY)),
    ("NaN", Value::number(f64::NAN)),
  ]
}
