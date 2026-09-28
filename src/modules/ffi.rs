//! `_ffi`: the native half of the `ffi` module.
//!
//! These natives take and return the handles `libs/ffi` wraps in its
//! classes, and hand back finished instances of those classes wherever a
//! pointer, a type or a callback comes out. Checking and conversion live
//! in `ffi_util`; this file is the table and the argument plumbing.

use std::sync::Arc;

use num_traits::ToPrimitive;

use crate::modules::ffi_util::call::{self, ForeignFunction};
use crate::modules::ffi_util::callback::{self, CallbackCore};
use crate::modules::ffi_util::convert::{self, Dest};
use crate::modules::ffi_util::declare::{Constant, Scope, ScopeRef, would_cycle};
use crate::modules::ffi_util::library::{self, Library, OpenFlags};
use crate::modules::ffi_util::link::{self, LinkOptions};
use crate::modules::ffi_util::memory::{Block, PointerData, read_bytes, terminated_len};
use crate::modules::ffi_util::types::{
  self, Abi, CType, Encoding, FieldSpec, Kind, LONG_DOUBLE, LongDoubleRepr, Record, TypeRef,
  builtin,
};
use crate::modules::ffi_util::{
  self as util, CALLBACK, DECLARATIONS, Fail, LIBRARY, POINTER, TYPE, cdecl, handle_of, rustdecl,
};
use crate::modules::{BuiltinModuleDef, native};
use crate::vm::object::{ModuleNamespace, ObjModule, ZuriContext, write_barrier};
use crate::vm::value::Value;
use crate::vm::vm::VM;

pub static MODULE: BuiltinModuleDef = BuiltinModuleDef {
  name: "_ffi",
  build,
};

type Native = fn(&mut ZuriContext) -> Result<Value, String>;

fn build(vm: &mut VM) -> Vec<(&'static str, Value)> {
  let table: &[(&'static str, u8, bool, Native)] = &[
    ("install", 1, false, install),
    ("platform", 0, false, platform),
    // Types.
    ("builtin_type", 1, false, builtin_type),
    ("pointer_type", 2, false, pointer_type),
    ("array_type", 2, false, array_type),
    ("const_type", 1, false, const_type),
    ("function_type", 4, false, function_type),
    ("slice_type", 2, false, slice_type),
    ("record_type", 2, false, record_type),
    ("record_add_field", 5, false, record_add_field),
    ("record_set_pack", 2, false, record_set_pack),
    ("record_set_align", 2, false, record_set_align),
    ("record_fields", 1, false, record_fields),
    ("record_offset", 2, false, record_offset),
    ("enum_type", 2, false, enum_type),
    ("enum_add", 3, false, enum_add),
    ("enum_constants", 1, false, enum_constants),
    ("type_name", 1, false, type_name),
    ("type_kind", 1, false, type_kind),
    ("type_size", 1, false, type_size),
    ("type_align", 1, false, type_align),
    ("type_is_const", 1, false, type_is_const),
    ("type_target", 1, false, type_target),
    ("type_length", 1, false, type_length),
    ("type_equals", 2, false, type_equals),
    ("type_signature", 1, false, type_signature),
    ("parse_type", 2, false, parse_type),
    // Memory.
    ("alloc", 2, false, alloc),
    ("alloc_bytes", 2, false, alloc_bytes),
    ("malloc", 1, false, malloc),
    ("alloc_string", 2, false, alloc_string),
    ("at", 2, false, at),
    ("ptr_address", 1, false, ptr_address),
    ("ptr_is_null", 1, false, ptr_is_null),
    ("ptr_type", 1, false, ptr_type),
    ("ptr_cast", 2, false, ptr_cast),
    ("ptr_offset", 2, false, ptr_offset),
    ("ptr_add", 2, false, ptr_add),
    ("ptr_equals", 2, false, ptr_equals),
    ("ptr_size", 1, false, ptr_size),
    ("ptr_read", 3, false, ptr_read),
    ("ptr_write", 4, false, ptr_write),
    ("ptr_get", 2, false, ptr_get),
    ("ptr_set", 3, false, ptr_set),
    ("ptr_get_field", 2, false, ptr_get_field),
    ("ptr_set_field", 3, false, ptr_set_field),
    ("ptr_field", 2, false, ptr_field),
    ("ptr_read_string", 4, false, ptr_read_string),
    ("ptr_write_string", 4, false, ptr_write_string),
    ("ptr_read_bytes", 3, false, ptr_read_bytes),
    ("ptr_write_bytes", 3, false, ptr_write_bytes),
    ("ptr_to_list", 2, false, ptr_to_list),
    ("ptr_copy", 3, false, ptr_copy),
    ("ptr_fill", 3, false, ptr_fill),
    ("ptr_compare", 3, false, ptr_compare),
    ("ptr_own", 2, false, ptr_own),
    ("ptr_free", 1, false, ptr_free),
    ("ptr_is_freed", 1, false, ptr_is_freed),
    ("ptr_is_owned", 1, false, ptr_is_owned),
    ("ptr_describe", 1, false, ptr_describe),
    // Libraries.
    ("open", 4, false, open),
    ("find", 2, false, find),
    ("libc_name", 0, false, libc_name),
    ("libm_name", 0, false, libm_name),
    ("lib_path", 1, false, lib_path),
    ("lib_close", 1, false, lib_close),
    ("lib_is_closed", 1, false, lib_is_closed),
    ("lib_symbol", 3, false, lib_symbol),
    ("lib_function", 3, false, lib_function),
    ("lib_variable", 3, false, lib_variable),
    // Functions.
    ("function", 3, false, function),
    ("threaded", 1, false, threaded),
    ("is_foreign", 1, false, is_foreign),
    ("function_info", 1, false, function_info),
    // Callbacks.
    ("callback", 3, false, callback_new),
    ("callback_pointer", 1, false, callback_pointer),
    ("callback_release", 1, false, callback_release),
    ("callback_is_released", 1, false, callback_is_released),
    ("callback_type", 1, false, callback_type),
    ("serve", 1, false, serve),
    // errno.
    ("errno", 0, false, errno),
    ("set_errno", 1, false, set_errno),
    ("last_error", 0, false, last_error),
    // Declarations.
    ("declarations", 0, false, declarations),
    ("declare_c", 2, false, declare_c),
    ("declare_rust", 2, false, declare_rust),
    ("include", 2, false, include),
    ("scope_type", 2, false, scope_type),
    ("scope_constant", 2, false, scope_constant),
    ("scope_constants", 1, false, scope_constants),
    ("scope_types", 1, false, scope_types),
    ("scope_functions", 1, false, scope_functions),
    ("scope_variables", 1, false, scope_variables),
    ("bind", 3, false, bind),
    // Static libraries.
    ("link", 2, false, link_archives),
    ("default_cache", 0, false, default_cache),
  ];

  table
    .iter()
    .map(|(name, arity, variadic, f)| (*name, native(vm, name, *arity, *variadic, *f)))
    .collect()
}

// Argument plumbing.

/// Returns a `Result<Value, Fail>` from a native, raising its failure.
/// The result is computed before `ctx` is borrowed again.
macro_rules! finish {
  ($ctx:expr, $e:expr) => {{
    let result = $e;
    result.map_err(|fail| util::raise($ctx.vm, fail))
  }};
}

macro_rules! attempt {
  ($ctx:expr, $e:expr) => {
    match $e {
      Ok(v) => v,
      Err(fail) => return Err(util::raise($ctx.vm, fail)),
    }
  };
}

fn arg(ctx: &ZuriContext, i: usize) -> Value {
  ctx.args.get(i).copied().unwrap_or(Value::nil())
}

fn handle(ctx: &ZuriContext, i: usize, tag: &str, what: &str) -> Result<Value, Fail> {
  let v = arg(ctx, i);
  match handle_of(v) {
    Some(h) if h.is_ptr_type(tag) => Ok(h),
    _ => Err(Fail::type_error(format!(
      "{}() expects {what} for argument {}, got {}",
      ctx.name,
      i + 1,
      v.argument_type_name()
    ))),
  }
}

fn type_arg(ctx: &ZuriContext, i: usize) -> Result<TypeRef, Fail> {
  let h = handle(ctx, i, TYPE, "an ffi type")?;
  convert::type_of(h)
}

fn optional_type(ctx: &ZuriContext, i: usize) -> Result<Option<TypeRef>, Fail> {
  if arg(ctx, i).is_nil() {
    return Ok(None);
  }
  type_arg(ctx, i).map(Some)
}

fn pointer_arg(ctx: &ZuriContext, i: usize) -> Result<(Value, PointerData), Fail> {
  let h = handle(ctx, i, POINTER, "a Pointer")?;
  Ok((h, convert::pointer_data(h)))
}

fn library_arg(ctx: &ZuriContext, i: usize) -> Result<Arc<Library>, Fail> {
  let h = handle(ctx, i, LIBRARY, "a Library")?;
  let cell = h.as_ptr_cell().borrow();
  Ok(cell.downcast_ref::<Arc<Library>>().unwrap().clone())
}

fn scope_arg(ctx: &ZuriContext, i: usize) -> Result<ScopeRef, Fail> {
  let h = handle(ctx, i, DECLARATIONS, "a Declarations")?;
  let cell = h.as_ptr_cell().borrow();
  Ok(cell.downcast_ref::<ScopeRef>().unwrap().clone())
}

fn callback_arg(ctx: &ZuriContext, i: usize) -> Result<Arc<CallbackCore>, Fail> {
  let h = handle(ctx, i, CALLBACK, "a Callback")?;
  let cell = h.as_ptr_cell().borrow();
  Ok(cell.downcast_ref::<Arc<CallbackCore>>().unwrap().clone())
}

fn string_arg(ctx: &ZuriContext, i: usize, what: &str) -> Result<String, Fail> {
  let v = arg(ctx, i);
  if !v.is_string() {
    return Err(Fail::type_error(format!(
      "{}() expects {what} as a string, got {}",
      ctx.name,
      v.argument_type_name()
    )));
  }
  Ok(v.as_str().to_string())
}

fn optional_string(ctx: &ZuriContext, i: usize, what: &str) -> Result<Option<String>, Fail> {
  if arg(ctx, i).is_nil() {
    return Ok(None);
  }
  string_arg(ctx, i, what).map(Some)
}

/// A whole number argument, as an `i64`.
fn int_arg(ctx: &ZuriContext, i: usize, what: &str) -> Result<i64, Fail> {
  let v = arg(ctx, i);
  if v.is_number() {
    let n = v.as_number();
    if n.fract() == 0.0 && n.is_finite() && n.abs() < 9.2e18 {
      return Ok(n as i64);
    }
  }
  if v.is_bigint()
    && let Some(n) = v.as_bigint().to_i64()
  {
    return Ok(n);
  }
  Err(Fail::type_error(format!(
    "{}() expects {what} as a whole number, got {}",
    ctx.name,
    v.argument_type_name()
  )))
}

fn count_arg(ctx: &ZuriContext, i: usize, what: &str) -> Result<usize, Fail> {
  let n = int_arg(ctx, i, what)?;
  if n < 0 {
    return Err(Fail::range(format!(
      "{}(): {what} cannot be negative",
      ctx.name
    )));
  }
  Ok(n as usize)
}

fn optional_int(ctx: &ZuriContext, i: usize, what: &str, default: i64) -> Result<i64, Fail> {
  if arg(ctx, i).is_nil() {
    return Ok(default);
  }
  int_arg(ctx, i, what)
}

fn encoding_arg(ctx: &ZuriContext, i: usize) -> Result<Encoding, Fail> {
  let Some(name) = optional_string(ctx, i, "an encoding")? else {
    return Ok(Encoding::Utf8);
  };
  Encoding::parse(&name).ok_or_else(|| {
    Fail::value(format!(
      "unknown encoding '{name}'; expected 'utf-8', 'utf-16', 'utf-32' or 'wide'"
    ))
  })
}

fn string_list(ctx: &ZuriContext, i: usize, what: &str) -> Result<Vec<String>, Fail> {
  let v = arg(ctx, i);
  if v.is_nil() {
    return Ok(Vec::new());
  }
  if !v.is_list() {
    return Err(Fail::type_error(format!(
      "{}() expects {what} as a list of strings",
      ctx.name
    )));
  }
  let mut out = Vec::new();
  for item in v.as_list() {
    if !item.is_string() {
      return Err(Fail::type_error(format!(
        "{}() expects {what} as a list of strings",
        ctx.name
      )));
    }
    out.push(item.as_str().to_string());
  }
  Ok(out)
}

/// An instance of the class that suits `ty`.
pub fn type_value(vm: &mut VM, ty: TypeRef) -> Result<Value, Fail> {
  let class = match &ty.kind {
    Kind::Record(r) if r.variants.get().is_some_and(|v| v.is_some()) => "StructType",
    Kind::Record(r) if r.is_union => "UnionType",
    Kind::Record(_) => "StructType",
    Kind::Enum(_) => "EnumType",
    _ => "Type",
  };
  let handle = vm.heap_mut().alloc_ptr(TYPE, ty);
  util::wrap(vm, class, handle)
}

fn str_value(vm: &mut VM, s: impl Into<String>) -> Value {
  vm.heap_mut().alloc_string(s.into())
}

fn dict_value(vm: &mut VM, pairs: Vec<(&str, Value)>) -> Value {
  let mut built = Vec::with_capacity(pairs.len());
  for (k, v) in pairs {
    let key = vm.heap_mut().alloc_string(k);
    built.push((key, v));
  }
  vm.heap_mut().alloc_dict(built)
}

fn address_value(vm: &mut VM, address: usize) -> Value {
  convert::number_value(vm, address as i128)
}

// Setup.

/// `install(classes)`: registers `libs/ffi`'s classes by name.
fn install(ctx: &mut ZuriContext) -> Result<Value, String> {
  let classes = arg(ctx, 0);
  if !classes.is_dict() {
    return Err("install() expects a dictionary of classes".into());
  }
  for (k, v) in classes.as_dict() {
    if k.is_string() && v.is_class() {
      util::register_class(ctx.vm, k.as_str(), v);
    }
  }
  Ok(Value::nil())
}

fn platform(ctx: &mut ZuriContext) -> Result<Value, String> {
  let os = str_value(ctx.vm, std::env::consts::OS);
  let arch = str_value(ctx.vm, std::env::consts::ARCH);
  let long_double = str_value(
    ctx.vm,
    match LONG_DOUBLE {
      LongDoubleRepr::X87 => "x87",
      LongDoubleRepr::Quad => "binary128",
      LongDoubleRepr::Double => "double",
    },
  );
  let layout = str_value(
    ctx.vm,
    if types::MSVC_LAYOUT {
      "msvc"
    } else {
      "itanium"
    },
  );
  Ok(dict_value(
    ctx.vm,
    vec![
      ("os", os),
      ("arch", arch),
      ("pointer_size", Value::number(types::POINTER_SIZE as f64)),
      ("long_size", Value::number(types::LONG_SIZE as f64)),
      ("wchar_size", Value::number(types::WCHAR.0 as f64)),
      ("char_signed", Value::bool(types::CHAR_SIGNED)),
      ("long_double", long_double),
      ("complex", Value::bool(types::HAS_COMPLEX)),
      ("layout", layout),
    ],
  ))
}

// Types.

fn builtin_type(ctx: &mut ZuriContext) -> Result<Value, String> {
  let name = attempt!(ctx, string_arg(ctx, 0, "a type name"));
  let Some(ty) = builtin(&name) else {
    return Err(util::raise(
      ctx.vm,
      Fail::value(format!("there is no built-in type '{name}'")),
    ));
  };
  finish!(ctx, type_value(ctx.vm, ty))
}

/// `pointer_type(target, options)`: options carry `nonnull` and `text`.
fn pointer_type(ctx: &mut ZuriContext) -> Result<Value, String> {
  let target = attempt!(ctx, type_arg(ctx, 0));
  let options = arg(ctx, 1);
  let mut nonnull = false;
  let mut text = None;

  if options.is_dict() {
    for (k, v) in options.as_dict() {
      if !k.is_string() {
        continue;
      }
      match k.as_str() {
        "nonnull" => nonnull = v.is_bool() && v.as_bool(),
        "text" if v.is_string() => {
          text = Some(attempt!(
            ctx,
            Encoding::parse(v.as_str())
              .ok_or_else(|| Fail::value(format!("unknown encoding '{}'", v.as_str())))
          ));
        },
        _ => {},
      }
    }
  }

  let name = types::pointer_name(&target);
  let ty = CType::pointer_with(&target, text, nonnull, name);
  finish!(ctx, type_value(ctx.vm, ty))
}

fn array_type(ctx: &mut ZuriContext) -> Result<Value, String> {
  let element = attempt!(ctx, type_arg(ctx, 0));
  let length = if arg(ctx, 1).is_nil() {
    None
  } else {
    Some(attempt!(ctx, count_arg(ctx, 1, "the length")))
  };
  if let Err(e) = element.require_size() {
    return Err(util::raise(
      ctx.vm,
      Fail::type_error(format!("cannot make an array: {e}")),
    ));
  }
  let ty = CType::array_of(&element, length);
  finish!(ctx, type_value(ctx.vm, ty))
}

fn const_type(ctx: &mut ZuriContext) -> Result<Value, String> {
  let ty = attempt!(ctx, type_arg(ctx, 0));
  let ty = CType::constant(&ty);
  finish!(ctx, type_value(ctx.vm, ty))
}

/// `function_type(returns, params, variadic, abi)`.
fn function_type(ctx: &mut ZuriContext) -> Result<Value, String> {
  let sig = attempt!(ctx, signature_args(ctx, 0));
  let ty = CType::function(sig);
  finish!(ctx, type_value(ctx.vm, ty))
}

/// A signature from `(returns, params, variadic, abi)` starting at
/// argument `at`.
fn signature_args(ctx: &ZuriContext, at: usize) -> Result<types::Signature, Fail> {
  let returns = type_arg(ctx, at)?;
  let params_value = arg(ctx, at + 1);
  if !params_value.is_list() {
    return Err(Fail::type_error(format!(
      "{}() expects the parameter types as a list",
      ctx.name
    )));
  }
  let mut params = Vec::new();
  for p in params_value.as_list() {
    let h = handle_of(p)
      .filter(|h| h.is_ptr_type(TYPE))
      .ok_or_else(|| {
        Fail::type_error(format!(
          "{}(): every parameter type must be an ffi type",
          ctx.name
        ))
      })?;
    params.push(convert::type_of(h)?);
  }
  let variadic = arg(ctx, at + 2).is_bool() && arg(ctx, at + 2).as_bool();
  let abi = match optional_string(ctx, at + 3, "a calling convention")? {
    Some(name) => Abi::parse(&name).map_err(Fail::value)?,
    None => Abi::Default,
  };
  call::signature(returns, params, variadic, abi)
}

fn slice_type(ctx: &mut ZuriContext) -> Result<Value, String> {
  let element = attempt!(ctx, type_arg(ctx, 0));
  let mutable = arg(ctx, 1).is_bool() && arg(ctx, 1).as_bool();
  let ty = types::slice_of(&element, mutable);
  finish!(ctx, type_value(ctx.vm, ty))
}

/// `record_type(name, is_union)`.
fn record_type(ctx: &mut ZuriContext) -> Result<Value, String> {
  let name = attempt!(ctx, optional_string(ctx, 0, "a name"));
  let is_union = arg(ctx, 1).is_bool() && arg(ctx, 1).as_bool();
  let record = Record::new(name.clone(), is_union);
  record.mark_defined();
  let word = if is_union { "union" } else { "struct" };
  let display = name.unwrap_or_else(|| format!("{word} <anonymous>"));
  let ty = CType::new(Kind::Record(record), display);
  finish!(ctx, type_value(ctx.vm, ty))
}

fn record_of(ctx: &ZuriContext, i: usize) -> Result<(TypeRef, Arc<Record>), Fail> {
  let ty = type_arg(ctx, i)?;
  let record = ty
    .record()
    .cloned()
    .ok_or_else(|| Fail::type_error(format!("'{}' is not a struct or union", ty.name)))?;
  Ok((ty, record))
}

/// `record_add_field(type, name, field_type, bits, align)`.
fn record_add_field(ctx: &mut ZuriContext) -> Result<Value, String> {
  let (_, record) = attempt!(ctx, record_of(ctx, 0));
  let name = attempt!(ctx, string_arg(ctx, 1, "a member name"));
  let ty = attempt!(ctx, type_arg(ctx, 2));
  let bits = if arg(ctx, 3).is_nil() {
    None
  } else {
    Some(attempt!(ctx, count_arg(ctx, 3, "the width in bits")) as u32)
  };
  let align = if arg(ctx, 4).is_nil() {
    None
  } else {
    Some(attempt!(ctx, count_arg(ctx, 4, "the alignment")))
  };
  if let Some(a) = align
    && !a.is_power_of_two()
  {
    return Err(util::raise(
      ctx.vm,
      Fail::value(format!("alignment must be a power of two, not {a}")),
    ));
  }
  attempt!(
    ctx,
    record
      .add_field(FieldSpec {
        name,
        ty,
        bits,
        align
      })
      .map_err(Fail::from_type)
  );
  Ok(Value::nil())
}

fn record_set_pack(ctx: &mut ZuriContext) -> Result<Value, String> {
  let (_, record) = attempt!(ctx, record_of(ctx, 0));
  let n = attempt!(ctx, count_arg(ctx, 1, "the packing"));
  attempt!(ctx, record.set_pack(n).map_err(Fail::from_type));
  Ok(Value::nil())
}

fn record_set_align(ctx: &mut ZuriContext) -> Result<Value, String> {
  let (_, record) = attempt!(ctx, record_of(ctx, 0));
  let n = attempt!(ctx, count_arg(ctx, 1, "the alignment"));
  attempt!(ctx, record.set_align(n).map_err(Fail::from_type));
  Ok(Value::nil())
}

/// Each member as `{ name, type, offset, bits, bit_offset }`.
fn record_fields(ctx: &mut ZuriContext) -> Result<Value, String> {
  let (_, record) = attempt!(ctx, record_of(ctx, 0));
  let layout = attempt!(ctx, record.layout().map_err(Fail::from_type));
  let fields = layout.flat_fields();
  let mut out = Vec::with_capacity(fields.len());

  for f in fields {
    if f.name == "@tag" {
      continue;
    }
    let name = str_value(ctx.vm, f.name.clone());
    let ty = finish!(ctx, type_value(ctx.vm, f.ty.clone()))?;
    let (bits, bit_offset) = match f.bits {
      Some((b, w)) => (Value::number(w as f64), Value::number(b as f64)),
      None => (Value::nil(), Value::nil()),
    };
    out.push(dict_value(
      ctx.vm,
      vec![
        ("name", name),
        ("type", ty),
        ("offset", Value::number(f.offset as f64)),
        ("bits", bits),
        ("bit_offset", bit_offset),
      ],
    ));
  }

  Ok(ctx.vm.heap_mut().alloc_list(out))
}

fn record_offset(ctx: &mut ZuriContext) -> Result<Value, String> {
  let (ty, record) = attempt!(ctx, record_of(ctx, 0));
  let name = attempt!(ctx, string_arg(ctx, 1, "a member name"));
  let layout = attempt!(ctx, record.layout().map_err(Fail::from_type));
  match layout.find(&name) {
    Some(f) => Ok(Value::number(f.offset as f64)),
    None => Err(util::raise(
      ctx.vm,
      Fail::value(format!("'{}' has no member named '{name}'", ty.name)),
    )),
  }
}

/// `enum_type(name, underlying)`.
fn enum_type(ctx: &mut ZuriContext) -> Result<Value, String> {
  let name = attempt!(ctx, optional_string(ctx, 0, "a name"));
  let underlying = match attempt!(ctx, optional_type(ctx, 1)) {
    Some(t) => t,
    None => builtin("int").unwrap(),
  };
  if !matches!(underlying.kind, Kind::Int { .. }) {
    return Err(util::raise(
      ctx.vm,
      Fail::type_error(format!(
        "an enum is stored as an integer type, not '{}'",
        underlying.name
      )),
    ));
  }
  let info = Arc::new(types::EnumInfo {
    underlying,
    constants: std::sync::Mutex::new(Vec::new()),
  });
  let display = name.unwrap_or_else(|| "enum <anonymous>".into());
  let ty = CType::new(Kind::Enum(info), display);
  finish!(ctx, type_value(ctx.vm, ty))
}

fn enum_add(ctx: &mut ZuriContext) -> Result<Value, String> {
  let ty = attempt!(ctx, type_arg(ctx, 0));
  let Kind::Enum(info) = &ty.kind else {
    return Err(util::raise(
      ctx.vm,
      Fail::type_error(format!("'{}' is not an enum", ty.name)),
    ));
  };
  let name = attempt!(ctx, string_arg(ctx, 1, "a constant name"));
  let value = attempt!(ctx, int_arg(ctx, 2, "the value")) as i128;

  let (size, signed) = match info.underlying.kind {
    Kind::Int { size, signed, .. } => (size, signed),
    _ => (4, true),
  };
  let bits = (size * 8).min(127);
  let (low, high) = if signed {
    (-(1i128 << (bits - 1)), (1i128 << (bits - 1)) - 1)
  } else {
    (0, (1i128 << bits) - 1)
  };
  if value < low || value > high {
    return Err(util::raise(
      ctx.vm,
      Fail::range(format!(
        "{value} does not fit in '{}'",
        info.underlying.name
      )),
    ));
  }

  let mut constants = info.constants.lock().unwrap();
  if constants.iter().any(|(n, _)| *n == name) {
    drop(constants);
    return Err(util::raise(
      ctx.vm,
      Fail::value(format!("'{}' already has a constant '{name}'", ty.name)),
    ));
  }
  constants.push((name, value));
  Ok(Value::nil())
}

fn enum_constants(ctx: &mut ZuriContext) -> Result<Value, String> {
  let ty = attempt!(ctx, type_arg(ctx, 0));
  let pairs: Vec<(String, i128)> = match &ty.kind {
    Kind::Enum(info) => info.constants.lock().unwrap().clone(),
    Kind::Record(r) => match r.variants.get() {
      Some(Some(v)) => v
        .cases
        .iter()
        .map(|c| (c.name.clone(), c.discriminant))
        .collect(),
      _ => Vec::new(),
    },
    _ => {
      return Err(util::raise(
        ctx.vm,
        Fail::type_error(format!("'{}' is not an enum", ty.name)),
      ));
    },
  };
  let mut out = Vec::new();
  for (name, value) in pairs {
    let key = str_value(ctx.vm, name);
    let v = convert::number_value(ctx.vm, value);
    out.push((key, v));
  }
  Ok(ctx.vm.heap_mut().alloc_dict(out))
}

fn type_name(ctx: &mut ZuriContext) -> Result<Value, String> {
  let ty = attempt!(ctx, type_arg(ctx, 0));
  Ok(str_value(ctx.vm, ty.name.clone()))
}

fn type_kind(ctx: &mut ZuriContext) -> Result<Value, String> {
  let ty = attempt!(ctx, type_arg(ctx, 0));
  Ok(str_value(ctx.vm, ty.kind_name()))
}

fn type_size(ctx: &mut ZuriContext) -> Result<Value, String> {
  let ty = attempt!(ctx, type_arg(ctx, 0));
  let size = attempt!(ctx, ty.require_size().map_err(Fail::from_type));
  Ok(Value::number(size as f64))
}

fn type_align(ctx: &mut ZuriContext) -> Result<Value, String> {
  let ty = attempt!(ctx, type_arg(ctx, 0));
  let align = attempt!(ctx, ty.require_align().map_err(Fail::from_type));
  Ok(Value::number(align as f64))
}

fn type_is_const(ctx: &mut ZuriContext) -> Result<Value, String> {
  let ty = attempt!(ctx, type_arg(ctx, 0));
  Ok(Value::bool(ty.is_const))
}

/// What a pointer points at, an array holds, or an enum is stored as.
fn type_target(ctx: &mut ZuriContext) -> Result<Value, String> {
  let ty = attempt!(ctx, type_arg(ctx, 0));
  let target = match &ty.kind {
    Kind::Pointer(p) => Some(p.target.clone()),
    Kind::Array { element, .. } => Some(element.clone()),
    Kind::Enum(e) => Some(e.underlying.clone()),
    _ => None,
  };
  match target {
    Some(t) => finish!(ctx, type_value(ctx.vm, t)),
    None => Ok(Value::nil()),
  }
}

fn type_length(ctx: &mut ZuriContext) -> Result<Value, String> {
  let ty = attempt!(ctx, type_arg(ctx, 0));
  match &ty.kind {
    Kind::Array {
      length: Some(n), ..
    } => Ok(Value::number(*n as f64)),
    _ => Ok(Value::nil()),
  }
}

fn type_equals(ctx: &mut ZuriContext) -> Result<Value, String> {
  let a = attempt!(ctx, type_arg(ctx, 0));
  let Some(h) = handle_of(arg(ctx, 1)).filter(|h| h.is_ptr_type(TYPE)) else {
    return Ok(Value::bool(false));
  };
  let b = attempt!(ctx, convert::type_of(h));
  Ok(Value::bool(CType::same(&a, &b)))
}

/// `{ returns, params, variadic, abi }` for a function type or a
/// pointer to one.
fn type_signature(ctx: &mut ZuriContext) -> Result<Value, String> {
  let ty = attempt!(ctx, type_arg(ctx, 0));
  let Some(sig) = ty.function_pointer().cloned() else {
    return Ok(Value::nil());
  };
  signature_value(ctx, &sig)
}

fn signature_value(ctx: &mut ZuriContext, sig: &types::Signature) -> Result<Value, String> {
  let returns = finish!(ctx, type_value(ctx.vm, sig.returns.clone()))?;
  let mut params = Vec::new();
  for p in &sig.params {
    params.push(finish!(ctx, type_value(ctx.vm, p.clone()))?);
  }
  let params = ctx.vm.heap_mut().alloc_list(params);
  let abi = str_value(ctx.vm, sig.abi.name());
  Ok(dict_value(
    ctx.vm,
    vec![
      ("returns", returns),
      ("params", params),
      ("variadic", Value::bool(sig.variadic)),
      ("abi", abi),
    ],
  ))
}

/// `parse_type(spelling, declarations)`.
fn parse_type(ctx: &mut ZuriContext) -> Result<Value, String> {
  let spelling = attempt!(ctx, string_arg(ctx, 0, "a C type name"));
  let scope = if arg(ctx, 1).is_nil() {
    Scope::new()
  } else {
    attempt!(ctx, scope_arg(ctx, 1))
  };
  let parsed = {
    let mut guard = scope.lock().unwrap();
    cdecl::type_name(&spelling, &mut guard)
  };
  let ty = attempt!(ctx, parsed.map_err(Fail::from));
  finish!(ctx, type_value(ctx.vm, ty))
}

// Memory.

fn pointer_value(ctx: &mut ZuriContext, data: PointerData) -> Result<Value, String> {
  finish!(ctx, convert::pointer_instance(ctx.vm, data))
}

/// `alloc(type, count)`: zeroed memory for `count` values.
fn alloc(ctx: &mut ZuriContext) -> Result<Value, String> {
  let ty = attempt!(ctx, type_arg(ctx, 0));
  let count = attempt!(ctx, count_arg(ctx, 1, "the count"));
  let size = attempt!(ctx, ty.require_size().map_err(Fail::from_type));
  let align = attempt!(ctx, ty.require_align().map_err(Fail::from_type));
  let total = attempt!(
    ctx,
    size.checked_mul(count).ok_or_else(|| Fail::range(format!(
      "{count} values of '{}' overflow the address space",
      ty.name
    )))
  );
  let block = attempt!(ctx, Block::allocate(total, align));
  pointer_value(
    ctx,
    PointerData {
      address: block.base,
      element: Some(ty),
      block: Some(block),
    },
  )
}

/// `alloc_bytes(size, align)`.
fn alloc_bytes(ctx: &mut ZuriContext) -> Result<Value, String> {
  let size = attempt!(ctx, count_arg(ctx, 0, "the size"));
  let align = attempt!(ctx, optional_int(ctx, 1, "the alignment", 16)) as usize;
  if !align.is_power_of_two() {
    return Err(util::raise(
      ctx.vm,
      Fail::value(format!("alignment must be a power of two, not {align}")),
    ));
  }
  let block = attempt!(ctx, Block::allocate(size, align));
  pointer_value(
    ctx,
    PointerData {
      address: block.base,
      element: None,
      block: Some(block),
    },
  )
}

fn malloc(ctx: &mut ZuriContext) -> Result<Value, String> {
  let size = attempt!(ctx, count_arg(ctx, 0, "the size"));
  let block = attempt!(ctx, Block::malloc(size));
  pointer_value(
    ctx,
    PointerData {
      address: block.base,
      element: None,
      block: Some(block),
    },
  )
}

/// `alloc_string(text, encoding)`: a NUL-terminated copy of `text`.
fn alloc_string(ctx: &mut ZuriContext) -> Result<Value, String> {
  let text = attempt!(ctx, string_arg(ctx, 0, "the text"));
  let encoding = attempt!(ctx, encoding_arg(ctx, 1));
  let bytes = convert::encode(&text, encoding);
  let block = attempt!(ctx, Block::allocate(bytes.len(), encoding.unit()));
  unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), block.base as *mut u8, bytes.len()) };

  let element = match encoding.resolve() {
    Encoding::Utf8 => builtin("char"),
    Encoding::Utf16 => builtin("char16_t"),
    _ if encoding == Encoding::Wide => builtin("wchar_t"),
    _ => builtin("char32_t"),
  };

  pointer_value(
    ctx,
    PointerData {
      address: block.base,
      element,
      block: Some(block),
    },
  )
}

/// `at(address, type)`: a pointer to a known address.
fn at(ctx: &mut ZuriContext) -> Result<Value, String> {
  let address = attempt!(ctx, int_arg(ctx, 0, "the address"));
  if address < 0 {
    return Err(util::raise(
      ctx.vm,
      Fail::range("an address cannot be negative"),
    ));
  }
  let element = attempt!(ctx, optional_type(ctx, 1));
  pointer_value(ctx, PointerData::raw(address as usize, element))
}

fn ptr_address(ctx: &mut ZuriContext) -> Result<Value, String> {
  let (_, data) = attempt!(ctx, pointer_arg(ctx, 0));
  Ok(address_value(ctx.vm, data.address))
}

fn ptr_is_null(ctx: &mut ZuriContext) -> Result<Value, String> {
  let (_, data) = attempt!(ctx, pointer_arg(ctx, 0));
  Ok(Value::bool(data.is_null()))
}

fn ptr_type(ctx: &mut ZuriContext) -> Result<Value, String> {
  let (_, data) = attempt!(ctx, pointer_arg(ctx, 0));
  match data.element {
    Some(t) => finish!(ctx, type_value(ctx.vm, t)),
    None => Ok(Value::nil()),
  }
}

fn ptr_cast(ctx: &mut ZuriContext) -> Result<Value, String> {
  let (_, data) = attempt!(ctx, pointer_arg(ctx, 0));
  let element = attempt!(ctx, optional_type(ctx, 1));
  pointer_value(ctx, PointerData { element, ..data })
}

/// `ptr_offset(p, bytes)`: the same pointer `bytes` further on.
fn ptr_offset(ctx: &mut ZuriContext) -> Result<Value, String> {
  let (_, data) = attempt!(ctx, pointer_arg(ctx, 0));
  let bytes = attempt!(ctx, int_arg(ctx, 1, "the offset"));
  let moved = attempt!(ctx, data.offset_by(bytes as isize, data.element.clone()));
  pointer_value(ctx, moved)
}

/// `ptr_add(p, n)`: `n` elements further on.
fn ptr_add(ctx: &mut ZuriContext) -> Result<Value, String> {
  let (_, data) = attempt!(ctx, pointer_arg(ctx, 0));
  let n = attempt!(ctx, int_arg(ctx, 1, "the count"));
  let step = attempt!(ctx, element_size(&data));
  let moved = attempt!(
    ctx,
    data.offset_by(n as isize * step as isize, data.element.clone())
  );
  pointer_value(ctx, moved)
}

fn element_type(data: &PointerData) -> Result<TypeRef, Fail> {
  data.element.clone().ok_or_else(|| {
    Fail::type_error(
      "this pointer has no element type; give it one with cast(), or use read() and write()",
    )
  })
}

fn element_size(data: &PointerData) -> Result<usize, Fail> {
  let ty = element_type(data)?;
  ty.require_size()
    .map_err(|e| Fail::type_error(format!("cannot step through the pointer: {e}")))
}

fn ptr_equals(ctx: &mut ZuriContext) -> Result<Value, String> {
  let (_, a) = attempt!(ctx, pointer_arg(ctx, 0));
  let Some(h) = handle_of(arg(ctx, 1)).filter(|h| h.is_ptr_type(POINTER)) else {
    return Ok(Value::bool(false));
  };
  let b = convert::pointer_data(h);
  Ok(Value::bool(a.address == b.address))
}

/// Bytes known to be addressable from this pointer on, or nil.
fn ptr_size(ctx: &mut ZuriContext) -> Result<Value, String> {
  let (_, data) = attempt!(ctx, pointer_arg(ctx, 0));
  Ok(match data.remaining() {
    Some(n) => Value::number(n as f64),
    None => Value::nil(),
  })
}

/// `ptr_read(p, type, offset)`.
fn ptr_read(ctx: &mut ZuriContext) -> Result<Value, String> {
  let (_, data) = attempt!(ctx, pointer_arg(ctx, 0));
  let ty = attempt!(ctx, type_arg(ctx, 1));
  let offset = attempt!(ctx, optional_int(ctx, 2, "the offset", 0));
  let size = attempt!(ctx, ty.require_size().map_err(Fail::from_type));
  let address = attempt!(ctx, data.check(offset as isize, size));
  finish!(ctx, convert::load(ctx.vm, &ty, address as *const u8))
}

/// `ptr_write(p, type, value, offset)`.
fn ptr_write(ctx: &mut ZuriContext) -> Result<Value, String> {
  let (_, data) = attempt!(ctx, pointer_arg(ctx, 0));
  let ty = attempt!(ctx, type_arg(ctx, 1));
  let value = arg(ctx, 2);
  let offset = attempt!(ctx, optional_int(ctx, 3, "the offset", 0));
  let size = attempt!(ctx, ty.require_size().map_err(Fail::from_type));
  let address = attempt!(ctx, data.check(offset as isize, size));
  attempt!(ctx, write_value(ctx.vm, &ty, value, address, size));
  Ok(Value::nil())
}

/// Stores `value` at `address`, leaving the memory untouched when the
/// conversion fails partway.
fn write_value(
  vm: &mut VM,
  ty: &TypeRef,
  value: Value,
  address: usize,
  size: usize,
) -> Result<(), Fail> {
  let mut staging = vec![0u128; size.div_ceil(16).max(1)];
  let staged = staging.as_mut_ptr() as *mut u8;
  // A record or array written as a whole keeps any members the value
  // leaves out as they were, so the staging area starts as a copy.
  unsafe { std::ptr::copy_nonoverlapping(address as *const u8, staged, size) };
  convert::store(vm, ty, value, staged, &mut Dest::Memory)?;
  unsafe { std::ptr::copy_nonoverlapping(staged, address as *mut u8, size) };
  Ok(())
}

fn ptr_get(ctx: &mut ZuriContext) -> Result<Value, String> {
  let (_, data) = attempt!(ctx, pointer_arg(ctx, 0));
  let index = attempt!(ctx, int_arg(ctx, 1, "the index"));
  let ty = attempt!(ctx, element_type(&data));
  let size = attempt!(ctx, element_size(&data));
  let address = attempt!(ctx, data.check(index as isize * size as isize, size));
  finish!(ctx, convert::load(ctx.vm, &ty, address as *const u8))
}

fn ptr_set(ctx: &mut ZuriContext) -> Result<Value, String> {
  let (_, data) = attempt!(ctx, pointer_arg(ctx, 0));
  let index = attempt!(ctx, int_arg(ctx, 1, "the index"));
  let value = arg(ctx, 2);
  let ty = attempt!(ctx, element_type(&data));
  let size = attempt!(ctx, element_size(&data));
  let address = attempt!(ctx, data.check(index as isize * size as isize, size));
  attempt!(ctx, write_value(ctx.vm, &ty, value, address, size));
  Ok(Value::nil())
}

fn record_target(data: &PointerData) -> Result<(TypeRef, Arc<Record>), Fail> {
  let ty = element_type(data)?;
  let record = ty.record().cloned().ok_or_else(|| {
    Fail::type_error(format!(
      "the pointer points at '{}', which has no members",
      ty.name
    ))
  })?;
  Ok((ty, record))
}

fn find_field(data: &PointerData, name: &str) -> Result<(types::Field, usize), Fail> {
  let (ty, record) = record_target(data)?;
  let layout = record.layout().map_err(Fail::from_type)?;
  let field = layout
    .find(name)
    .filter(|f| f.name != "@tag")
    .ok_or_else(|| Fail::value(format!("'{}' has no member named '{name}'", ty.name)))?;
  Ok((field, layout.size))
}

fn ptr_get_field(ctx: &mut ZuriContext) -> Result<Value, String> {
  let (_, data) = attempt!(ctx, pointer_arg(ctx, 0));
  let name = attempt!(ctx, string_arg(ctx, 1, "a member name"));
  let (field, size) = attempt!(ctx, find_field(&data, &name));
  let base = attempt!(ctx, data.check(0, size));
  finish!(ctx, convert::load_field(ctx.vm, &field, base as *const u8))
}

fn ptr_set_field(ctx: &mut ZuriContext) -> Result<Value, String> {
  let (_, data) = attempt!(ctx, pointer_arg(ctx, 0));
  let name = attempt!(ctx, string_arg(ctx, 1, "a member name"));
  let value = arg(ctx, 2);
  let (field, size) = attempt!(ctx, find_field(&data, &name));
  let base = attempt!(ctx, data.check(0, size));

  // Staged like a whole write, so a failed conversion leaves the member
  // as it was.
  let mut staging = vec![0u128; size.div_ceil(16).max(1)];
  let staged = staging.as_mut_ptr() as *mut u8;
  unsafe { std::ptr::copy_nonoverlapping(base as *const u8, staged, size) };
  attempt!(
    ctx,
    convert::store_field(ctx.vm, &field, value, staged, &mut Dest::Memory)
  );
  unsafe { std::ptr::copy_nonoverlapping(staged, base as *mut u8, size) };
  Ok(Value::nil())
}

/// A pointer to one member, typed as the member.
fn ptr_field(ctx: &mut ZuriContext) -> Result<Value, String> {
  let (_, data) = attempt!(ctx, pointer_arg(ctx, 0));
  let name = attempt!(ctx, string_arg(ctx, 1, "a member name"));
  let (field, _) = attempt!(ctx, find_field(&data, &name));
  if field.bits.is_some() {
    return Err(util::raise(
      ctx.vm,
      Fail::type_error(format!("'{name}' is a bitfield, which has no address")),
    ));
  }
  // An array member decays to a pointer to its first element, as the
  // member itself does in C, which is what makes a flexible array
  // member reachable at all.
  let element = match &field.ty.kind {
    Kind::Array { element, .. } => element.clone(),
    _ => field.ty.clone(),
  };
  let moved = attempt!(ctx, data.offset_by(field.offset as isize, Some(element)));
  pointer_value(ctx, moved)
}

/// `ptr_read_string(p, length, encoding, offset)`.
fn ptr_read_string(ctx: &mut ZuriContext) -> Result<Value, String> {
  let (_, data) = attempt!(ctx, pointer_arg(ctx, 0));
  let length = if arg(ctx, 1).is_nil() {
    None
  } else {
    Some(attempt!(ctx, count_arg(ctx, 1, "the length")))
  };
  let encoding = attempt!(ctx, encoding_arg(ctx, 2));
  let offset = attempt!(ctx, optional_int(ctx, 3, "the offset", 0));
  let unit = encoding.unit();

  let start = attempt!(ctx, data.check(offset as isize, 0));
  let moved = attempt!(ctx, data.offset_by(offset as isize, None));

  let count = match length {
    Some(n) => {
      attempt!(ctx, data.check(offset as isize, n * unit));
      n
    },
    None => {
      let limit = moved.remaining().map(|r| r / unit);
      let n = unsafe { terminated_len(start, unit, limit) };
      if limit.is_some_and(|l| n >= l) {
        return Err(util::raise(
          ctx.vm,
          Fail::pointer("no terminator within the block; give the length to read"),
        ));
      }
      n
    },
  };

  finish!(
    ctx,
    convert::read_text(ctx.vm, start, encoding, Some(count), false)
  )
}

/// `ptr_write_string(p, text, encoding, offset)`: writes the text and a
/// terminator, returning the bytes written.
fn ptr_write_string(ctx: &mut ZuriContext) -> Result<Value, String> {
  let (_, data) = attempt!(ctx, pointer_arg(ctx, 0));
  let text = attempt!(ctx, string_arg(ctx, 1, "the text"));
  let encoding = attempt!(ctx, encoding_arg(ctx, 2));
  let offset = attempt!(ctx, optional_int(ctx, 3, "the offset", 0));
  let bytes = convert::encode(&text, encoding);
  let address = attempt!(ctx, data.check(offset as isize, bytes.len()));
  unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), address as *mut u8, bytes.len()) };
  Ok(Value::number(bytes.len() as f64))
}

fn ptr_read_bytes(ctx: &mut ZuriContext) -> Result<Value, String> {
  let (_, data) = attempt!(ctx, pointer_arg(ctx, 0));
  let length = attempt!(ctx, count_arg(ctx, 1, "the length"));
  let offset = attempt!(ctx, optional_int(ctx, 2, "the offset", 0));
  let address = attempt!(ctx, data.check(offset as isize, length));
  let bytes = unsafe { read_bytes(address, length) };
  Ok(ctx.vm.heap_mut().alloc_bytes(bytes))
}

fn ptr_write_bytes(ctx: &mut ZuriContext) -> Result<Value, String> {
  let (_, data) = attempt!(ctx, pointer_arg(ctx, 0));
  let value = arg(ctx, 1);
  if !value.is_bytes() {
    return Err(util::raise(
      ctx.vm,
      Fail::type_error("write_bytes() expects bytes"),
    ));
  }
  let offset = attempt!(ctx, optional_int(ctx, 2, "the offset", 0));
  let len = value.bytes_len();
  let address = attempt!(ctx, data.check(offset as isize, len));
  value.with_bytes(|b| unsafe {
    std::ptr::copy_nonoverlapping(b.as_ptr(), address as *mut u8, b.len())
  });
  Ok(Value::number(len as f64))
}

fn ptr_to_list(ctx: &mut ZuriContext) -> Result<Value, String> {
  let (_, data) = attempt!(ctx, pointer_arg(ctx, 0));
  let count = attempt!(ctx, count_arg(ctx, 1, "the count"));
  let ty = attempt!(ctx, element_type(&data));
  let size = attempt!(ctx, element_size(&data));
  let base = attempt!(ctx, data.check(0, size * count));

  let mut items = Vec::with_capacity(count);
  for i in 0..count {
    let item = attempt!(
      ctx,
      convert::load(ctx.vm, &ty, (base + i * size) as *const u8)
    );
    items.push(item);
  }
  Ok(ctx.vm.heap_mut().alloc_list(items))
}

/// `ptr_copy(dest, src, length)`: overlapping ranges are fine.
fn ptr_copy(ctx: &mut ZuriContext) -> Result<Value, String> {
  let (_, dest) = attempt!(ctx, pointer_arg(ctx, 0));
  let length = attempt!(ctx, count_arg(ctx, 2, "the length"));
  let dest_address = attempt!(ctx, dest.check(0, length));

  let source = arg(ctx, 1);
  if source.is_bytes() {
    if source.bytes_len() < length {
      return Err(util::raise(
        ctx.vm,
        Fail::range("the bytes are shorter than the length to copy"),
      ));
    }
    source.with_bytes(|b| unsafe { std::ptr::copy(b.as_ptr(), dest_address as *mut u8, length) });
    return Ok(Value::nil());
  }

  let (_, src) = attempt!(ctx, pointer_arg(ctx, 1));
  let src_address = attempt!(ctx, src.check(0, length));
  unsafe { std::ptr::copy(src_address as *const u8, dest_address as *mut u8, length) };
  Ok(Value::nil())
}

fn ptr_fill(ctx: &mut ZuriContext) -> Result<Value, String> {
  let (_, data) = attempt!(ctx, pointer_arg(ctx, 0));
  let byte = attempt!(ctx, int_arg(ctx, 1, "the byte"));
  if !(0..=255).contains(&byte) {
    return Err(util::raise(
      ctx.vm,
      Fail::range(format!("{byte} is not a byte")),
    ));
  }
  let length = attempt!(ctx, count_arg(ctx, 2, "the length"));
  let address = attempt!(ctx, data.check(0, length));
  unsafe { std::ptr::write_bytes(address as *mut u8, byte as u8, length) };
  Ok(Value::nil())
}

fn ptr_compare(ctx: &mut ZuriContext) -> Result<Value, String> {
  let (_, a) = attempt!(ctx, pointer_arg(ctx, 0));
  let (_, b) = attempt!(ctx, pointer_arg(ctx, 1));
  let length = attempt!(ctx, count_arg(ctx, 2, "the length"));
  let x = attempt!(ctx, a.check(0, length));
  let y = attempt!(ctx, b.check(0, length));
  let left = unsafe { std::slice::from_raw_parts(x as *const u8, length) };
  let right = unsafe { std::slice::from_raw_parts(y as *const u8, length) };
  Ok(Value::number(match left.cmp(right) {
    std::cmp::Ordering::Less => -1.0,
    std::cmp::Ordering::Equal => 0.0,
    std::cmp::Ordering::Greater => 1.0,
  }))
}

/// `ptr_own(p, destructor)`: takes ownership of memory C allocated.
fn ptr_own(ctx: &mut ZuriContext) -> Result<Value, String> {
  let (handle, data) = attempt!(ctx, pointer_arg(ctx, 0));

  if data.is_null() {
    return Err(util::raise(
      ctx.vm,
      Fail::pointer("a null pointer owns nothing"),
    ));
  }
  if data.block.is_some() {
    return Err(util::raise(
      ctx.vm,
      Fail::pointer("this pointer's memory already has an owner"),
    ));
  }

  let destructor = if arg(ctx, 1).is_nil() {
    None
  } else {
    let Some(f) = call::function_of(arg(ctx, 1)) else {
      return Err(util::raise(
        ctx.vm,
        Fail::type_error(
          "own() takes a foreign function as the destructor, or nil for the C allocator's free",
        ),
      ));
    };
    if f.sig.params.len() != 1 || f.sig.params[0].pointer().is_none() || f.sig.variadic {
      return Err(util::raise(
        ctx.vm,
        Fail::type_error(format!(
          "a destructor takes one pointer, but '{}' is {}",
          f.name,
          f.sig.describe()
        )),
      ));
    }
    attempt!(ctx, f.prepare());
    Some(f)
  };

  let block = Block::adopt(data.address, None, destructor);
  let mut cell = handle.as_ptr_cell().borrow_mut();
  if let Some(d) = cell.downcast_mut::<PointerData>() {
    d.block = Some(block);
  }
  Ok(Value::nil())
}

/// `ptr_free(p)`: releases owned memory now.
fn ptr_free(ctx: &mut ZuriContext) -> Result<Value, String> {
  let (_, data) = attempt!(ctx, pointer_arg(ctx, 0));
  let Some(block) = data.block else {
    return Err(util::raise(
      ctx.vm,
      Fail::pointer(
        "this pointer does not own its memory; release it with the library's own function, or own() it first",
      ),
    ));
  };
  if data.address != block.base {
    return Err(util::raise(
      ctx.vm,
      Fail::pointer("only a pointer to the start of an allocation can free it"),
    ));
  }
  attempt!(ctx, block.free(ctx.vm));
  Ok(Value::nil())
}

fn ptr_is_freed(ctx: &mut ZuriContext) -> Result<Value, String> {
  let (_, data) = attempt!(ctx, pointer_arg(ctx, 0));
  Ok(Value::bool(data.block.is_some_and(|b| b.is_freed())))
}

fn ptr_is_owned(ctx: &mut ZuriContext) -> Result<Value, String> {
  let (_, data) = attempt!(ctx, pointer_arg(ctx, 0));
  Ok(Value::bool(data.block.is_some()))
}

fn ptr_describe(ctx: &mut ZuriContext) -> Result<Value, String> {
  let (_, data) = attempt!(ctx, pointer_arg(ctx, 0));
  let text = match &data.element {
    Some(t) => format!("<Pointer {:#x} to {}>", data.address, t.name),
    None => format!("<Pointer {:#x}>", data.address),
  };
  Ok(str_value(ctx.vm, text))
}

// Libraries.

/// `open(name, paths, lazy, global)`, with a nil name for the process.
fn open(ctx: &mut ZuriContext) -> Result<Value, String> {
  let name = attempt!(ctx, optional_string(ctx, 0, "a library name"));
  let paths = attempt!(ctx, string_list(ctx, 1, "the search paths"));
  let flags = OpenFlags {
    lazy: arg(ctx, 2).is_bool() && arg(ctx, 2).as_bool(),
    global: arg(ctx, 3).is_bool() && arg(ctx, 3).as_bool(),
  };

  let opened = match &name {
    None => Library::open_exact(None, flags),
    Some(n) => Library::open(n, &paths, flags),
  };

  let lib = attempt!(ctx, opened.map_err(Fail::load));
  let handle = ctx.vm.heap_mut().alloc_ptr(LIBRARY, Arc::new(lib));
  finish!(ctx, util::wrap(ctx.vm, "Library", handle))
}

fn find(ctx: &mut ZuriContext) -> Result<Value, String> {
  let name = attempt!(ctx, string_arg(ctx, 0, "a library name"));
  let paths = attempt!(ctx, string_list(ctx, 1, "the search paths"));
  Ok(match library::find(&name, &paths) {
    Some(p) => str_value(ctx.vm, p),
    None => Value::nil(),
  })
}

fn libc_name(ctx: &mut ZuriContext) -> Result<Value, String> {
  Ok(str_value(ctx.vm, library::libc_name()))
}

fn libm_name(ctx: &mut ZuriContext) -> Result<Value, String> {
  Ok(str_value(ctx.vm, library::libm_name()))
}

fn lib_path(ctx: &mut ZuriContext) -> Result<Value, String> {
  let lib = attempt!(ctx, library_arg(ctx, 0));
  Ok(if lib.is_process() {
    Value::nil()
  } else {
    str_value(ctx.vm, lib.path.clone())
  })
}

fn lib_close(ctx: &mut ZuriContext) -> Result<Value, String> {
  let lib = attempt!(ctx, library_arg(ctx, 0));
  lib.close();
  Ok(Value::nil())
}

fn lib_is_closed(ctx: &mut ZuriContext) -> Result<Value, String> {
  let lib = attempt!(ctx, library_arg(ctx, 0));
  Ok(Value::bool(lib.is_closed()))
}

fn open_library(lib: &Library) -> Result<(), Fail> {
  if lib.is_closed() {
    return Err(Fail::load(format!(
      "the library '{}' has been closed",
      lib.path
    )));
  }
  Ok(())
}

/// Splits `name@VERSION`, which asks for a versioned symbol.
fn split_version(name: &str) -> (&str, Option<&str>) {
  match name.split_once('@') {
    Some((n, v)) if !n.is_empty() && !v.is_empty() => (n, Some(v.trim_start_matches('@'))),
    _ => (name, None),
  }
}

fn resolve(lib: &Library, name: &str) -> Result<usize, Fail> {
  open_library(lib)?;
  let (symbol, version) = split_version(name);
  let found = lib.symbol(symbol, version).map_err(Fail::symbol)?;
  found.ok_or_else(|| {
    let place = if lib.is_process() {
      "the running process".to_string()
    } else {
      format!("'{}'", lib.path)
    };
    Fail::symbol(format!("'{name}' is not exported by {place}"))
  })
}

/// `lib_symbol(lib, name, type)`: the symbol's address, or nil.
fn lib_symbol(ctx: &mut ZuriContext) -> Result<Value, String> {
  let lib = attempt!(ctx, library_arg(ctx, 0));
  let name = attempt!(ctx, string_arg(ctx, 1, "a symbol name"));
  let element = attempt!(ctx, optional_type(ctx, 2));
  attempt!(ctx, open_library(&lib));
  let (symbol, version) = split_version(&name);
  let found = attempt!(ctx, lib.symbol(symbol, version).map_err(Fail::symbol));
  match found {
    Some(address) => pointer_value(ctx, PointerData::raw(address, element)),
    None => Ok(Value::nil()),
  }
}

/// `lib_function(lib, name, function_type)`.
fn lib_function(ctx: &mut ZuriContext) -> Result<Value, String> {
  let lib = attempt!(ctx, library_arg(ctx, 0));
  let name = attempt!(ctx, string_arg(ctx, 1, "a function name"));
  let ty = attempt!(ctx, type_arg(ctx, 2));
  let Some(sig) = ty.function_pointer().cloned() else {
    return Err(util::raise(
      ctx.vm,
      Fail::type_error(format!("'{}' is not a function type", ty.name)),
    ));
  };
  let address = attempt!(ctx, resolve(&lib, &name));
  let (plain, _) = split_version(&name);
  let f = ForeignFunction::new(plain.to_string(), address, sig, Some(lib));
  attempt!(ctx, f.prepare());
  Ok(call::function_value(ctx.vm, Arc::new(f)))
}

/// `lib_variable(lib, name, type)`: a pointer to a global.
fn lib_variable(ctx: &mut ZuriContext) -> Result<Value, String> {
  let lib = attempt!(ctx, library_arg(ctx, 0));
  let name = attempt!(ctx, string_arg(ctx, 1, "a variable name"));
  let ty = attempt!(ctx, type_arg(ctx, 2));
  let address = attempt!(ctx, resolve(&lib, &name));
  pointer_value(ctx, PointerData::raw(address, Some(ty)))
}

// Functions and callbacks.

/// `function(pointer, function_type, name)`: a callable for code at an
/// address.
fn function(ctx: &mut ZuriContext) -> Result<Value, String> {
  let (_, data) = attempt!(ctx, pointer_arg(ctx, 0));
  if data.is_null() {
    return Err(util::raise(
      ctx.vm,
      Fail::pointer("a null pointer cannot be called"),
    ));
  }
  let ty = attempt!(ctx, type_arg(ctx, 1));
  let Some(sig) = ty.function_pointer().cloned() else {
    return Err(util::raise(
      ctx.vm,
      Fail::type_error(format!("'{}' is not a function type", ty.name)),
    ));
  };
  let name = attempt!(ctx, optional_string(ctx, 2, "a name"))
    .unwrap_or_else(|| format!("{:#x}", data.address));
  let f = ForeignFunction::new(name, data.address, sig, None);
  attempt!(ctx, f.prepare());
  Ok(call::function_value(ctx.vm, Arc::new(f)))
}

fn foreign_arg(ctx: &ZuriContext, i: usize) -> Result<Arc<ForeignFunction>, Fail> {
  call::function_of(arg(ctx, i)).ok_or_else(|| {
    Fail::type_error(format!(
      "{}() expects a foreign function, got {}",
      ctx.name,
      arg(ctx, i).argument_type_name()
    ))
  })
}

fn threaded(ctx: &mut ZuriContext) -> Result<Value, String> {
  let f = attempt!(ctx, foreign_arg(ctx, 0));
  callback::inbox(ctx.vm);
  Ok(call::function_value(ctx.vm, Arc::new(f.threaded())))
}

fn is_foreign(ctx: &mut ZuriContext) -> Result<Value, String> {
  Ok(Value::bool(call::function_of(arg(ctx, 0)).is_some()))
}

/// `{ name, pointer, type, threaded, library }`.
fn function_info(ctx: &mut ZuriContext) -> Result<Value, String> {
  let f = attempt!(ctx, foreign_arg(ctx, 0));
  let name = str_value(ctx.vm, f.name.clone());
  let fn_type = CType::function(types::Signature {
    returns: f.sig.returns.clone(),
    params: f.sig.params.clone(),
    param_names: f.sig.param_names.clone(),
    variadic: f.sig.variadic,
    abi: f.sig.abi,
  });
  let pointer_type = CType::pointer_to(&fn_type);
  let ptr = pointer_value(ctx, PointerData::raw(f.address, Some(fn_type.clone())))?;
  let ty = finish!(ctx, type_value(ctx.vm, pointer_type))?;
  let lib = match &f.library {
    Some(l) if !l.is_process() => str_value(ctx.vm, l.path.clone()),
    _ => Value::nil(),
  };
  Ok(dict_value(
    ctx.vm,
    vec![
      ("name", name),
      ("pointer", ptr),
      ("type", ty),
      ("threaded", Value::bool(f.threaded)),
      ("library", lib),
    ],
  ))
}

/// `callback(function, function_type, error_value)`.
fn callback_new(ctx: &mut ZuriContext) -> Result<Value, String> {
  let function = arg(ctx, 0);
  if !function.is_callable() || function.is_class() {
    return Err(util::raise(
      ctx.vm,
      Fail::type_error("callback() expects a function"),
    ));
  }
  let ty = attempt!(ctx, type_arg(ctx, 1));
  let Some(sig) = ty.function_pointer().cloned() else {
    return Err(util::raise(
      ctx.vm,
      Fail::type_error(format!("'{}' is not a function type", ty.name)),
    ));
  };
  let error_value = if arg(ctx, 2).is_nil() {
    None
  } else {
    Some(arg(ctx, 2))
  };

  let core = attempt!(ctx, CallbackCore::new(ctx.vm, function, sig, error_value));

  // A lasting callback can be called from any thread at any time, so
  // the safepoints start looking for calls posted to this VM.
  ctx.vm.async_armed = true;
  super::os_util::signal::arm_async();

  let handle = ctx.vm.heap_mut().alloc_ptr(CALLBACK, core);
  finish!(ctx, util::wrap(ctx.vm, "Callback", handle))
}

fn callback_pointer(ctx: &mut ZuriContext) -> Result<Value, String> {
  let core = attempt!(ctx, callback_arg(ctx, 0));
  if core.is_released() {
    return Err(util::raise(
      ctx.vm,
      Fail::callback("this callback has been released"),
    ));
  }
  let sig = core.signature().clone();
  let fn_type = CType::new(Kind::Function(sig.clone()), sig.describe());
  pointer_value(ctx, PointerData::raw(core.code(), Some(fn_type)))
}

fn callback_release(ctx: &mut ZuriContext) -> Result<Value, String> {
  let core = attempt!(ctx, callback_arg(ctx, 0));
  core.release_on(ctx.vm);
  Ok(Value::nil())
}

fn callback_is_released(ctx: &mut ZuriContext) -> Result<Value, String> {
  let core = attempt!(ctx, callback_arg(ctx, 0));
  Ok(Value::bool(core.is_released()))
}

fn callback_type(ctx: &mut ZuriContext) -> Result<Value, String> {
  let core = attempt!(ctx, callback_arg(ctx, 0));
  let sig = core.signature().clone();
  let fn_type = CType::new(Kind::Function(sig.clone()), sig.describe());
  let ty = CType::pointer_to(&fn_type);
  finish!(ctx, type_value(ctx.vm, ty))
}

/// `serve(timeout)`: answers callback calls posted from other threads,
/// waiting up to `timeout` milliseconds for the first when there are
/// none. Returns how many were answered.
fn serve(ctx: &mut ZuriContext) -> Result<Value, String> {
  let timeout = if arg(ctx, 0).is_nil() {
    None
  } else {
    let n = arg(ctx, 0);
    if !n.is_number() || n.as_number() < 0.0 {
      return Err(util::raise(
        ctx.vm,
        Fail::type_error("serve() expects a timeout in milliseconds, or nil"),
      ));
    }
    Some(std::time::Duration::from_secs_f64(n.as_number() / 1000.0))
  };

  let inbox = callback::inbox(ctx.vm);
  let mut answered = attempt!(ctx, callback::service(ctx.vm, &inbox));

  if answered == 0
    && let Some(t) = timeout
  {
    inbox.wait(t);
    answered = attempt!(ctx, callback::service(ctx.vm, &inbox));
  }

  Ok(Value::number(answered as f64))
}

// errno.

fn errno(ctx: &mut ZuriContext) -> Result<Value, String> {
  let value = ctx.vm.ffi.as_ref().map(|s| s.errno).unwrap_or(0);
  Ok(Value::number(value as f64))
}

fn set_errno(ctx: &mut ZuriContext) -> Result<Value, String> {
  let n = attempt!(ctx, int_arg(ctx, 0, "the value"));
  util::state(ctx.vm).errno = n as i32;
  call::errno::set(n as i32);
  Ok(Value::nil())
}

fn last_error(ctx: &mut ZuriContext) -> Result<Value, String> {
  let value = ctx.vm.ffi.as_ref().map(|s| s.last_error).unwrap_or(0);
  Ok(Value::number(value as f64))
}

// Declarations.

fn declarations(ctx: &mut ZuriContext) -> Result<Value, String> {
  let handle = ctx.vm.heap_mut().alloc_ptr(DECLARATIONS, Scope::new());
  finish!(ctx, util::wrap(ctx.vm, "Declarations", handle))
}

fn declare_c(ctx: &mut ZuriContext) -> Result<Value, String> {
  let scope = attempt!(ctx, scope_arg(ctx, 0));
  let source = attempt!(ctx, string_arg(ctx, 1, "the declarations"));
  let result = {
    let mut guard = scope.lock().unwrap();
    cdecl::declare(&source, &mut guard)
  };
  attempt!(ctx, result.map_err(Fail::from));
  Ok(Value::nil())
}

fn declare_rust(ctx: &mut ZuriContext) -> Result<Value, String> {
  let scope = attempt!(ctx, scope_arg(ctx, 0));
  let source = attempt!(ctx, string_arg(ctx, 1, "the declarations"));
  let result = {
    let mut guard = scope.lock().unwrap();
    rustdecl::declare(&source, &mut guard)
  };
  attempt!(ctx, result.map_err(Fail::from));
  Ok(Value::nil())
}

fn include(ctx: &mut ZuriContext) -> Result<Value, String> {
  let scope = attempt!(ctx, scope_arg(ctx, 0));
  let other = attempt!(ctx, scope_arg(ctx, 1));
  if would_cycle(&scope, &other) {
    return Err(util::raise(
      ctx.vm,
      Fail::value("including these declarations would make them include themselves"),
    ));
  }
  scope.lock().unwrap().includes.push(other);
  Ok(Value::nil())
}

fn scope_type(ctx: &mut ZuriContext) -> Result<Value, String> {
  let scope = attempt!(ctx, scope_arg(ctx, 0));
  let spelling = attempt!(ctx, string_arg(ctx, 1, "a type name"));
  let parsed = {
    let mut guard = scope.lock().unwrap();
    cdecl::type_name(&spelling, &mut guard)
  };
  let ty = attempt!(ctx, parsed.map_err(Fail::from));
  finish!(ctx, type_value(ctx.vm, ty))
}

fn constant_value(vm: &mut VM, c: Constant) -> Result<Value, Fail> {
  Ok(match c {
    Constant::Int(v) => convert::number_value(vm, v),
    Constant::Float(f) => Value::number(f),
    Constant::Str(s) => vm.heap_mut().alloc_string(s),
    // Null is nil, as a null pointer loads anywhere else. Any other
    // address is a `Pointer` of the type it was cast to, never a
    // callable function: a sentinel has nothing at it to call.
    Constant::Pointer { address: 0, .. } => Value::nil(),
    Constant::Pointer { ty, address } => {
      let target = ty.pointer().map(|p| p.target.clone());
      convert::pointer_instance(vm, PointerData::raw(address, target))?
    },
  })
}

fn scope_constant(ctx: &mut ZuriContext) -> Result<Value, String> {
  let scope = attempt!(ctx, scope_arg(ctx, 0));
  let name = attempt!(ctx, string_arg(ctx, 1, "a constant name"));
  let found = scope.lock().unwrap().constant(&name);
  match found {
    Some(c) => finish!(ctx, constant_value(ctx.vm, c)),
    None => Err(util::raise(
      ctx.vm,
      Fail::value(format!("no constant named '{name}' was declared")),
    )),
  }
}

fn scope_constants(ctx: &mut ZuriContext) -> Result<Value, String> {
  let scope = attempt!(ctx, scope_arg(ctx, 0));
  let constants = scope.lock().unwrap().constants.clone();
  let mut out = Vec::new();
  for (name, c) in constants {
    let key = str_value(ctx.vm, name);
    let v = attempt!(ctx, constant_value(ctx.vm, c));
    out.push((key, v));
  }
  Ok(ctx.vm.heap_mut().alloc_dict(out))
}

/// Every named type, typedefs and tags alike, keyed as C spells them.
fn scope_types(ctx: &mut ZuriContext) -> Result<Value, String> {
  let scope = attempt!(ctx, scope_arg(ctx, 0));
  let mut entries: Vec<(String, TypeRef)> = {
    let guard = scope.lock().unwrap();
    guard
      .typedefs
      .iter()
      .chain(guard.tags.iter())
      .map(|(k, v)| (k.clone(), v.clone()))
      .collect()
  };
  entries.sort_by(|a, b| a.0.cmp(&b.0));
  let mut out = Vec::new();
  for (name, ty) in entries {
    let key = str_value(ctx.vm, name);
    let v = finish!(ctx, type_value(ctx.vm, ty))?;
    out.push((key, v));
  }
  Ok(ctx.vm.heap_mut().alloc_dict(out))
}

fn scope_functions(ctx: &mut ZuriContext) -> Result<Value, String> {
  let scope = attempt!(ctx, scope_arg(ctx, 0));
  let functions = scope.lock().unwrap().functions.clone();
  let mut out = Vec::new();
  for f in functions {
    let key = str_value(ctx.vm, f.name.clone());
    let ty = CType::new(Kind::Function(f.sig.clone()), f.sig.describe());
    let v = finish!(ctx, type_value(ctx.vm, ty))?;
    out.push((key, v));
  }
  Ok(ctx.vm.heap_mut().alloc_dict(out))
}

fn scope_variables(ctx: &mut ZuriContext) -> Result<Value, String> {
  let scope = attempt!(ctx, scope_arg(ctx, 0));
  let variables = scope.lock().unwrap().variables.clone();
  let mut out = Vec::new();
  for v in variables {
    let key = str_value(ctx.vm, v.name.clone());
    let t = finish!(ctx, type_value(ctx.vm, v.ty.clone()))?;
    out.push((key, t));
  }
  Ok(ctx.vm.heap_mut().alloc_dict(out))
}

/// `bind(declarations, library, allow_missing)`: a namespace holding a
/// callable for every declared function, a pointer for every variable,
/// every constant, and every typedef.
fn bind(ctx: &mut ZuriContext) -> Result<Value, String> {
  let scope = attempt!(ctx, scope_arg(ctx, 0));
  let lib = attempt!(ctx, library_arg(ctx, 1));
  let allow_missing = arg(ctx, 2).is_bool() && arg(ctx, 2).as_bool();
  attempt!(ctx, open_library(&lib));

  let (functions, variables, constants, typedefs) = {
    let guard = scope.lock().unwrap();
    let mut typedefs: Vec<(String, TypeRef)> = guard
      .typedefs
      .iter()
      .map(|(k, v)| (k.clone(), v.clone()))
      .collect();
    typedefs.sort_by(|a, b| a.0.cmp(&b.0));
    (
      guard.functions.clone(),
      guard.variables.clone(),
      guard.constants.clone(),
      typedefs,
    )
  };

  let mut missing = Vec::new();
  let mut members: Vec<(String, Value)> = Vec::new();

  for (name, ty) in typedefs {
    let v = finish!(ctx, type_value(ctx.vm, ty))?;
    members.push((name, v));
  }

  for (name, c) in constants {
    if name.contains("::") {
      continue;
    }
    let v = attempt!(ctx, constant_value(ctx.vm, c));
    members.push((name, v));
  }

  for v in variables {
    let found = attempt!(ctx, lib.symbol(&v.symbol, None).map_err(Fail::symbol));
    match found {
      Some(address) => {
        let p = pointer_value(ctx, PointerData::raw(address, Some(v.ty.clone())))?;
        members.push((v.name.clone(), p));
      },
      None => missing.push(v.name.clone()),
    }
  }

  for f in functions {
    let (symbol, version) = split_version(&f.symbol);
    let found = attempt!(ctx, lib.symbol(symbol, version).map_err(Fail::symbol));
    match found {
      Some(address) => {
        let function =
          ForeignFunction::new(f.name.clone(), address, f.sig.clone(), Some(lib.clone()));
        if let Err(fail) = function.prepare() {
          let message = match fail {
            Fail::Ffi(_, m) | Fail::Builtin(_, m) => m,
            _ => "its signature cannot be called".into(),
          };
          return Err(util::raise(
            ctx.vm,
            Fail::ffi(format!("cannot bind '{}': {message}", f.name)),
          ));
        }
        let value = call::function_value(ctx.vm, Arc::new(function));
        members.push((f.name.clone(), value));
      },
      None => missing.push(f.name.clone()),
    }
  }

  if !missing.is_empty() && !allow_missing {
    let place = if lib.is_process() {
      "the running process".to_string()
    } else {
      format!("'{}'", lib.path)
    };
    return Err(util::raise(
      ctx.vm,
      Fail::symbol(format!("{place} does not export {}", quote_list(&missing))),
    ));
  }

  let label = if lib.is_process() {
    "process".to_string()
  } else {
    std::path::Path::new(&lib.path)
      .file_name()
      .map(|n| n.to_string_lossy().into_owned())
      .unwrap_or_else(|| lib.path.clone())
  };

  let module = ctx.vm.heap_mut().alloc_module(ObjModule {
    name: label.clone(),
    path: format!("<ffi:{}>", lib.path),
    namespace: ModuleNamespace::new(),
    loaded: true,
  });

  for (name, value) in members {
    module.as_module_mut().namespace.set(&name, value);
    write_barrier(module.as_obj());
  }

  Ok(module)
}

fn quote_list(names: &[String]) -> String {
  let quoted: Vec<String> = names.iter().map(|n| format!("'{n}'")).collect();
  match quoted.len() {
    1 => quoted[0].clone(),
    _ => {
      let (last, rest) = quoted.split_last().unwrap();
      format!("{} and {last}", rest.join(", "))
    },
  }
}

// Static libraries.

/// `link(archives, options)`: the path of the shared library built from
/// the archives.
fn link_archives(ctx: &mut ZuriContext) -> Result<Value, String> {
  let archives = attempt!(ctx, string_list(ctx, 0, "the static libraries"));
  let options = arg(ctx, 1);
  let mut opts = LinkOptions::default();

  if options.is_dict() {
    for (k, v) in options.as_dict() {
      if !k.is_string() {
        continue;
      }
      let strings = |v: Value| -> Result<Vec<String>, Fail> {
        if !v.is_list() {
          return Err(Fail::type_error(format!(
            "the '{}' option is a list of strings",
            k.as_str()
          )));
        }
        v.as_list()
          .into_iter()
          .map(|s| {
            if s.is_string() {
              Ok(s.as_str().to_string())
            } else {
              Err(Fail::type_error(format!(
                "the '{}' option is a list of strings",
                k.as_str()
              )))
            }
          })
          .collect()
      };
      let text = |v: Value| -> Result<String, Fail> {
        if v.is_string() {
          Ok(v.as_str().to_string())
        } else {
          Err(Fail::type_error(format!(
            "the '{}' option is a string",
            k.as_str()
          )))
        }
      };
      match k.as_str() {
        "libraries" => opts.libraries = attempt!(ctx, strings(v)),
        "search_paths" => opts.search_paths = attempt!(ctx, strings(v)),
        "exports" => opts.exports = Some(attempt!(ctx, strings(v))),
        "flags" => opts.flags = attempt!(ctx, strings(v)),
        "linker" => opts.linker = Some(attempt!(ctx, text(v))),
        "cache" => opts.cache = Some(attempt!(ctx, text(v))),
        "rust" if v.is_bool() => opts.rust = Some(v.as_bool()),
        "rust" if v.is_nil() => {},
        other => {
          return Err(util::raise(
            ctx.vm,
            Fail::value(format!("link() has no option '{other}'")),
          ));
        },
      }
    }
  }

  let path = attempt!(ctx, link::link(&archives, &opts).map_err(Fail::link));
  Ok(str_value(ctx.vm, path))
}

fn default_cache(ctx: &mut ZuriContext) -> Result<Value, String> {
  let path = link::default_cache();
  Ok(str_value(ctx.vm, path.to_string_lossy().into_owned()))
}
