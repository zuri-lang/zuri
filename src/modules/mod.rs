//! Registry of builtin (native) modules -- e.g. `import io`, `import
//! math`. A builtin module can only ever export plain functions and
//! constants, never a class, so each one is just a flat list of
//! (name, Value) members built once per VM run (see
//! `vm::modules::builtin_module`, which caches the result the same
//! way a real `.zu` module is cached).
//!
//! Adding a new builtin module means adding one file here exposing a
//! `pub static MODULE: BuiltinModuleDef`, then listing it in
//! `REGISTRY` below -- nothing else needs to change.

mod base64;
mod crypto;
mod date;
mod hash;
mod io;
mod json;
mod math;
mod os;
mod r#struct;

use crate::vm::object::{NativeFn, NativeFunction};
use crate::vm::value::Value;
use crate::vm::vm::VM;

#[derive(Clone, Copy)]
pub struct BuiltinModuleDef {
  pub name: &'static str,
  pub build: fn(&mut VM) -> Vec<(&'static str, Value)>,
}

pub static REGISTRY: &[BuiltinModuleDef] = &[
  math::MODULE,
  io::MODULE,
  os::MODULE,
  hash::MODULE,
  crypto::MODULE,
  json::MODULE,
  base64::MODULE,
  date::MODULE,
  r#struct::MODULE,
];

pub fn find(name: &str) -> Option<&'static BuiltinModuleDef> {
  REGISTRY.iter().find(|m| m.name == name)
}

/// Allocate a free (non-method) native function Value -- the
/// module-scoped equivalent of `natives.rs`'s `register`, just
/// handing back the Value instead of also binding it as a VM global.
pub fn native(
  vm: &mut VM,
  name: &'static str,
  min_arity: u8,
  variadic: bool,
  func: NativeFn,
) -> Value {
  vm.heap_mut().alloc_native(NativeFunction {
    is_method: false,
    name,
    min_arity,
    variadic,
    func,
  })
}
