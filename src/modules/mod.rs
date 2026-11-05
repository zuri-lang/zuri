//! Registry of builtin (native) modules; e.g. `import io`, `import
//! math`. A builtin module can only ever export plain functions and
//! constants, never a class, so each one is just a flat list of
//! (name, Value) members built once per VM run (see
//! `vm::modules::builtin_module`, which caches the result the same
//! way a real `.zu` module is cached).
//!
//! Adding a new builtin module means adding one file here exposing a
//! `pub static MODULE: BuiltinModuleDef`, then listing it in
//! `REGISTRY` below; nothing else needs to change.

mod base64;
mod compress;
pub(crate) mod compress_util;
mod crypto;
mod date;
mod ffi;
pub(crate) mod ffi_util;
mod foreign_panic;
pub(crate) mod hash;
mod http;
mod imagine;
mod io;
mod isolate;
mod isolate_util;
mod json;
mod math;
mod net_dtls;
mod net_poll;
mod net_resolver;
pub(crate) mod net_tcp;
pub(crate) mod net_tls;
mod net_udp;
pub(crate) mod net_unix;
mod os;
pub(crate) mod os_util;
mod sqlite;
pub(crate) mod sqlite_util;
mod r#struct;
mod zuri_compile;
mod zuri_lex;
mod zuri_parse;
mod zuri_reflect;
mod zuri_watch;

use std::cell::RefCell;

use crate::vm::object::{NativeFn, NativeFunction, ZuriContext};
use crate::vm::value::Value;
use crate::vm::vm::VM;

/// The largest read that goes through the reused buffer. A bigger one
/// gets a buffer of its own, so a single huge read does not hold that
/// much memory for the rest of the thread's life.
const READ_SCRATCH_LIMIT: usize = 1 << 20;

thread_local! {
  static READ_SCRATCH: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
}

/// Runs a read of at most `length` bytes and returns the bytes that
/// arrived.
///
/// The length a socket or decoder read takes is an upper bound, and the
/// read usually returns far less: a database client asks for 64 KiB to
/// receive a reply of a few hundred bytes. Zeroing a fresh buffer of
/// the whole length on every call costs more than the read itself, so
/// the bytes land in a buffer reused across calls and only the filled
/// part is copied out. A read that runs while another is using that
/// buffer, which a callback during a TLS handshake could cause, takes a
/// fresh one instead.
pub(crate) fn read_partial<E>(
  length: usize,
  read: impl FnOnce(&mut [u8]) -> Result<usize, E>,
) -> Result<Vec<u8>, E> {
  let mut read = Some(read);

  let reused = READ_SCRATCH.with(|cell| {
    if length > READ_SCRATCH_LIMIT {
      return None;
    }

    let mut scratch = cell.try_borrow_mut().ok()?;

    if scratch.len() < length {
      scratch.resize(length, 0);
    }

    let read = read.take().unwrap();

    Some(read(&mut scratch[..length]).map(|count| scratch[..count].to_vec()))
  });

  if let Some(result) = reused {
    return result;
  }

  let mut buffer = vec![0u8; length];
  let count = (read.take().unwrap())(&mut buffer)?;

  if count == length {
    return Ok(buffer);
  }

  Ok(buffer[..count].to_vec())
}

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
  http::MODULE,
  imagine::MODULE,
  crypto::MODULE,
  json::MODULE,
  base64::MODULE,
  date::MODULE,
  ffi::MODULE,
  sqlite::MODULE,
  r#struct::MODULE,
  compress::MODULE,
  isolate::MODULE,
  net_dtls::MODULE,
  net_poll::MODULE,
  net_resolver::MODULE,
  net_tcp::MODULE,
  net_unix::MODULE,
  net_tls::MODULE,
  net_udp::MODULE,
  zuri_compile::MODULE,
  zuri_lex::MODULE,
  zuri_parse::MODULE,
  zuri_reflect::MODULE,
  zuri_watch::MODULE,
];

pub fn find(name: &str) -> Option<&'static BuiltinModuleDef> {
  REGISTRY.iter().find(|m| m.name == name)
}

/// The message a socket's I/O error reaches Zuri code with. Windows
/// reports a read or write that outlived its timeout as `WSAETIMEDOUT`,
/// which it words as a failed connection attempt, so a timeout is
/// named as one here whatever the platform called it.
pub fn socket_error(error: std::io::Error) -> String {
  if error.kind() == std::io::ErrorKind::TimedOut {
    return String::from("operation timed out");
  }

  error.to_string()
}

/// A Zuri class whose instances a native builds directly, without
/// running its constructor: the class, and the slot of each field the
/// native fills. Handing Zuri code a dict for its constructor to copy
/// from costs the dict, its keys and the copy for every instance.
pub(crate) struct Shape<const N: usize> {
  class: Value,
  field_count: usize,
  slots: [usize; N],
}

impl<const N: usize> Shape<N> {
  /// The shape of `class` for the fields `names`, in the order `build`
  /// takes their values. Fails when `class` is not a class or declares
  /// no field by one of the names.
  pub(crate) fn of(class: Value, names: [&str; N]) -> Result<Self, String> {
    if !class.is_class() {
      return Err(format!("expected a class, got {}", class.type_name()));
    }

    let c = class.as_class();
    let mut slots = [0; N];
    for (slot, name) in slots.iter_mut().zip(names) {
      match c.field_slots.get(name) {
        Some(&found) => *slot = found as usize,
        None => return Err(format!("class '{}' has no field '{}'", c.name, name)),
      }
    }

    Ok(Self {
      class,
      field_count: c.field_count as usize,
      slots,
    })
  }

  /// A new instance holding `values` in the fields `of` named, and nil
  /// in any other. The instance is young, so filling it needs no write
  /// barrier.
  pub(crate) fn build(&self, vm: &mut VM, values: [Value; N]) -> Value {
    let instance = vm.heap_mut().alloc_instance(self.class, self.field_count);
    let fields = &instance.as_instance().fields;
    for (&slot, value) in self.slots.iter().zip(values) {
      fields[slot].set(value);
    }
    instance
  }
}

/// Allocate a free (non-method) native function Value; the
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

fn optional_bool(ctx: &ZuriContext, idx: usize, default: bool) -> Result<bool, String> {
  match ctx.args.get(idx) {
    None => Ok(default),
    Some(v) if v.is_nil() => Ok(default),
    Some(v) if v.is_bool() => Ok(v.as_bool()),
    Some(v) => Err(format!(
      "{}() expects argument {} to be a bool, got {}",
      ctx.name,
      idx + 1,
      v.type_name()
    )),
  }
}

fn optional_number(ctx: &ZuriContext, idx: usize, default: f64) -> Result<f64, String> {
  match ctx.args.get(idx) {
    None => Ok(default),
    Some(v) if v.is_nil() => Ok(default),
    Some(v) if v.is_number() => Ok(v.as_number()),
    Some(v) => Err(format!(
      "{}() expects argument {} to be a number, got {}",
      ctx.name,
      idx + 1,
      v.type_name()
    )),
  }
}
