//! Everything behind `_ffi`, the native half of the `ffi` module.
//!
//! The pieces, in the order a call passes through them: `types` knows
//! how every C type is laid out, `library` finds and loads code,
//! `declare` with `cdecl` and `rustdecl` turns source text into types
//! and signatures, `convert` moves values between Zuri and C memory,
//! `call` makes the call, `callback` lets C call back in, and `memory`
//! owns the native memory a program allocates. `link` turns a static
//! archive into something loadable, and `wide_return` carries the one
//! result libffi cannot: a 128-bit integer under Microsoft x64.
//!
//! Every resource reaches Zuri as a `Ptr` tagged with one of the
//! constants below, wrapped in an instance of the matching class from
//! `libs/ffi`. The classes are registered with the VM when that module
//! loads, so a native can hand back a finished `Pointer` or `Type`
//! without running any Zuri code to build it.

pub mod call;
pub mod callback;
pub mod cdecl;
pub mod convert;
pub mod declare;
pub mod floats;
pub mod library;
pub mod link;
pub mod memory;
pub mod rustdecl;
pub mod types;
#[cfg(target_arch = "x86_64")]
pub mod wide_return;

use std::any::Any;
use std::sync::Arc;

use rustc_hash::FxHashMap;

use crate::vm::value::Value;
use crate::vm::vm::VM;

pub const TYPE: &str = "zuri::ffi::type";
pub const POINTER: &str = "zuri::ffi::pointer";
pub const LIBRARY: &str = "zuri::ffi::library";
pub const FUNCTION: &str = "zuri::ffi::function";
pub const CALLBACK: &str = "zuri::ffi::callback";
pub const DECLARATIONS: &str = "zuri::ffi::declarations";

/// Builds a fresh payload for a handle crossing to another isolate.
pub type SharedPayload = Arc<dyn Fn() -> Box<dyn Any + Send> + Send + Sync>;

/// A way to give another isolate its own handle on what `value` holds,
/// when it is one of this module's handles. Every one of them is a
/// reference-counted description, so both sides can keep using it.
pub fn shareable(value: Value) -> Option<(&'static str, SharedPayload)> {
  let tag = value.ptr_type_name()?;
  let cell = value.as_ptr_cell().borrow();

  macro_rules! share {
    ($payload:ty, $tag:expr) => {{
      let payload = cell.downcast_ref::<$payload>()?.clone();
      let make: SharedPayload = Arc::new(move || Box::new(payload.clone()) as Box<dyn Any + Send>);
      Some(($tag, make))
    }};
  }

  match tag {
    TYPE => share!(types::TypeRef, TYPE),
    POINTER => share!(memory::PointerData, POINTER),
    LIBRARY => share!(Arc<library::Library>, LIBRARY),
    FUNCTION => share!(Arc<call::ForeignFunction>, FUNCTION),
    DECLARATIONS => share!(declare::ScopeRef, DECLARATIONS),
    CALLBACK => share!(Arc<callback::CallbackCore>, CALLBACK),
    _ => None,
  }
}

/// What the `ffi` module keeps per VM.
pub struct VmState {
  /// The classes `libs/ffi` registered, by name, each held in a native
  /// root so a collection can move it.
  classes: FxHashMap<String, usize>,
  /// Where calls from other threads to this VM's callbacks wait.
  pub inbox: Option<Arc<callback::Inbox>>,
  /// `errno` as the most recent foreign call on this VM left it.
  pub errno: i32,
  /// `GetLastError()` the same way, on Windows.
  pub last_error: u32,
  /// An error a callback raised during the call in progress, rooted,
  /// waiting to be raised again once C has returned.
  pub trapped: Option<usize>,
}

impl VmState {
  fn new() -> VmState {
    VmState {
      classes: FxHashMap::default(),
      inbox: None,
      errno: 0,
      last_error: 0,
      trapped: None,
    }
  }
}

impl Drop for VmState {
  fn drop(&mut self) {
    if let Some(inbox) = &self.inbox {
      inbox.close();
    }
  }
}

pub fn state(vm: &mut VM) -> &mut VmState {
  vm.ffi.get_or_insert_with(|| Box::new(VmState::new()))
}

/// Registers one of `libs/ffi`'s classes under `name`.
pub fn register_class(vm: &mut VM, name: &str, class: Value) {
  let previous = state(vm).classes.get(name).copied();

  match previous {
    Some(slot) => vm.replace_native_root(slot, class),
    None => {
      let slot = vm.retain_native_root(class);
      state(vm).classes.insert(name.to_string(), slot);
    },
  }
}

pub fn class(vm: &mut VM, name: &str) -> Option<Value> {
  let slot = vm.ffi.as_ref()?.classes.get(name).copied()?;
  let value = vm.native_root(slot);
  (!value.is_nil()).then_some(value)
}

/// `class`, loading the `ffi` package first when the class is not
/// registered yet.
///
/// A handle can reach an isolate that loaded only the part of
/// `libs/ffi` its value came from, so the class a native needs may not
/// be registered there. Loading the package registers all of them. It
/// runs Zuri code, which can collect, so a caller holding values across
/// this pins them.
fn class_loading(vm: &mut VM, name: &str) -> Result<Option<Value>, Value> {
  if let Some(c) = class(vm, name) {
    return Ok(Some(c));
  }

  crate::vm::modules::import(vm, "", "ffi")?;
  Ok(class(vm, name))
}

/// A failure on its way to becoming a Zuri error.
pub enum Fail {
  /// One of the prelude's error classes: `TypeError`, `RangeError`.
  Builtin(&'static str, String),
  /// One of `ffi`'s own: `PointerError`, `LoadError`.
  Ffi(&'static str, String),
  /// A `DeclarationError`, which carries where in the source it was.
  Declaration {
    message: String,
    line: usize,
    column: usize,
  },
  /// A Zuri error value already raised by code the native called, to
  /// be passed on untouched.
  Raised(Value),
}

impl Fail {
  pub fn type_error(message: impl Into<String>) -> Fail {
    Fail::Builtin("TypeError", message.into())
  }

  pub fn range(message: impl Into<String>) -> Fail {
    Fail::Builtin("RangeError", message.into())
  }

  pub fn value(message: impl Into<String>) -> Fail {
    Fail::Builtin("ValueError", message.into())
  }

  pub fn argument(message: impl Into<String>) -> Fail {
    Fail::Builtin("ArgumentError", message.into())
  }

  pub fn ffi(message: impl Into<String>) -> Fail {
    Fail::Ffi("FfiError", message.into())
  }

  pub fn pointer(message: impl Into<String>) -> Fail {
    Fail::Ffi("PointerError", message.into())
  }

  pub fn load(message: impl Into<String>) -> Fail {
    Fail::Ffi("LoadError", message.into())
  }

  pub fn symbol(message: impl Into<String>) -> Fail {
    Fail::Ffi("SymbolError", message.into())
  }

  pub fn callback(message: impl Into<String>) -> Fail {
    Fail::Ffi("CallbackError", message.into())
  }

  pub fn link(message: impl Into<String>) -> Fail {
    Fail::Ffi("LinkError", message.into())
  }

  /// A description error from the type layer, which reports plain
  /// strings, raised as the `FfiError` it is.
  pub fn from_type(message: String) -> Fail {
    Fail::Ffi("FfiError", message)
  }
}

impl From<cdecl::SyntaxError> for Fail {
  fn from(e: cdecl::SyntaxError) -> Fail {
    Fail::Declaration {
      message: e.message,
      line: e.line,
      column: e.column,
    }
  }
}

/// Turns `fail` into the error a native returns: the error value is
/// built and parked with the VM, which raises it as it is.
pub fn raise(vm: &mut VM, fail: Fail) -> String {
  let error = match fail {
    Fail::Raised(error) => error,
    Fail::Builtin(class_name, message) => vm.raise(class_name, message),
    Fail::Ffi(class_name, message) => build_error(vm, class_name, &[message], None),
    Fail::Declaration {
      message,
      line,
      column,
    } => build_error(vm, "DeclarationError", &[message], Some((line, column))),
  };

  vm.rethrow(error)
}

fn build_error(
  vm: &mut VM,
  class_name: &str,
  message: &[String],
  position: Option<(usize, usize)>,
) -> Value {
  let text = message.join("");

  let class = match class_loading(vm, class_name) {
    Ok(Some(class)) => class,
    Ok(None) => return vm.raise("Error", text),
    Err(error) => return error,
  };

  let mut args = vec![vm.heap_mut().alloc_string(text.clone())];
  if let Some((line, column)) = position {
    args.push(Value::number(line as f64));
    args.push(Value::number(column as f64));
  }

  match vm.call_value(class, &args) {
    Ok(error) => error,
    Err(error) => error,
  }
}

/// The `_handle` a wrapper instance carries, or the value itself when
/// it is already a bare handle.
pub fn handle_of(value: Value) -> Option<Value> {
  if value.is_ptr() {
    return Some(value);
  }

  if !value.is_instance() {
    return None;
  }

  let instance = value.as_instance();
  let class = instance.class.as_class();
  let slot = *class.field_slots.get("_handle")?;
  let handle = instance.fields[slot as usize].get();
  handle.is_ptr().then_some(handle)
}

/// Wraps `handle` in a fresh instance of the registered class `name`,
/// without running its constructor.
pub fn wrap(vm: &mut VM, name: &str, handle: Value) -> Result<Value, Fail> {
  let pin = vm.pin_values([handle]);
  let found = class_loading(vm, name);
  let handle = vm.pinned(pin);

  let class = match found {
    Ok(Some(class)) => class,
    Ok(None) => {
      return Err(Fail::ffi(format!(
        "the ffi module has no '{name}' class to build"
      )));
    },
    Err(error) => return Err(Fail::Raised(error)),
  };

  let (field_count, slot) = {
    let c = class.as_class();
    (c.field_count, c.field_slots.get("_handle").copied())
  };

  let Some(slot) = slot else {
    return Err(Fail::ffi(format!("the '{name}' class has no handle field")));
  };

  let instance = vm.heap_mut().alloc_instance(class, field_count as usize);
  instance.as_instance().fields[slot as usize].set(handle);
  Ok(instance)
}

/// Whether `value` is an instance of the registered class `name` or a
/// subclass of it.
pub fn is_instance_of(vm: &mut VM, value: Value, name: &str) -> bool {
  if !value.is_instance() {
    return false;
  }

  let Some(class) = class(vm, name) else {
    return false;
  };

  let mut current = Some(value.as_instance().class);
  while let Some(c) = current {
    if c.equals(&class) {
      return true;
    }
    current = c.as_class().superclass;
  }

  false
}
