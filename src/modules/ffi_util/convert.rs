//! Moving values across the boundary: a Zuri value into C memory as a
//! given type, and C memory back out as a Zuri value.
//!
//! Every conversion is checked. An integer that does not fit its type
//! is a `RangeError`, never a truncation; a value of the wrong kind is a
//! `TypeError` that names the type it was headed for.
//!
//! A value stored as a call argument may need memory of its own: a
//! string becomes a NUL-terminated copy, a list becomes an array, a
//! Zuri function becomes a callback. That memory lives in `Scratch` and
//! lasts exactly as long as the call. Stored anywhere else, where it
//! would have to outlive the statement, those conversions are refused
//! and the error says how to make the memory last.

use std::sync::Arc;

use num_bigint::BigInt;
use num_traits::ToPrimitive;

use super::callback::{self, CallbackCore};
use super::memory::{PointerData, terminated_len};
use super::types::{CType, Encoding, IntRole, Kind, LONG_DOUBLE, LongDoubleRepr, TypeRef, builtin};
use super::{CALLBACK, FUNCTION, Fail, POINTER, floats, handle_of, is_instance_of, wrap};
use crate::vm::value::Value;
use crate::vm::vm::VM;

/// Integers up to this magnitude come back as numbers; past it, as
/// bigints, so no value is ever rounded.
const SAFE_INTEGER: i128 = 1 << 53;

/// Memory a call's arguments need for the duration of the call.
#[derive(Default)]
pub struct Scratch {
  buffers: Vec<Vec<u128>>,
  pub callbacks: Vec<Arc<CallbackCore>>,
  pub copy_back: Vec<CopyBack>,
}

/// A list or dictionary passed where C may write through the pointer,
/// to be updated once the call has returned.
pub struct CopyBack {
  /// Where the value is pinned, since a callback during the call can
  /// run a collection that moves it.
  pub pin: usize,
  pub ty: TypeRef,
  pub address: usize,
  /// Elements for a list; `None` for a dictionary.
  pub count: Option<usize>,
}

impl Scratch {
  /// Zeroed, 16-byte aligned memory that stays put until the scratch is
  /// dropped.
  pub fn buffer(&mut self, len: usize) -> *mut u8 {
    let mut storage = vec![0u128; len.div_ceil(16).max(1)];
    let address = storage.as_mut_ptr() as *mut u8;
    self.buffers.push(storage);
    address
  }

  /// Keeps `bytes` alive for the call and returns where they are.
  pub fn keep(&mut self, bytes: &[u8]) -> *mut u8 {
    let address = self.buffer(bytes.len());
    // SAFETY: the buffer was just allocated with at least `len` bytes.
    unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), address, bytes.len()) };
    address
  }
}

impl Drop for Scratch {
  fn drop(&mut self) {
    for core in self.callbacks.drain(..) {
      core.release();
    }
  }
}

/// Where a store is headed.
pub enum Dest<'a> {
  /// An argument of the call in progress, which may borrow memory.
  Call(&'a mut Scratch),
  /// Memory that outlives the statement.
  Memory,
}

impl Dest<'_> {
  fn scratch(&mut self) -> Option<&mut Scratch> {
    match self {
      Dest::Call(s) => Some(s),
      Dest::Memory => None,
    }
  }
}

// Storing.

/// Writes `value` into `dest` as a `ty`. `dest` need not be aligned.
pub fn store(
  vm: &mut VM,
  ty: &TypeRef,
  value: Value,
  dest: *mut u8,
  to: &mut Dest,
) -> Result<(), Fail> {
  match &ty.kind {
    Kind::Void => Err(Fail::type_error("nothing can be stored as 'void'")),
    Kind::Function(_) => Err(Fail::type_error(format!(
      "a function is not a value; store a pointer to '{}' instead",
      ty.name
    ))),
    Kind::Bool => {
      if !value.is_bool() {
        return Err(expected(ty, "a bool", value));
      }
      unsafe { dest.write(value.as_bool() as u8) };
      Ok(())
    },
    Kind::Int { size, signed, role } => {
      if *role == IntRole::OptionalNonZero && value.is_nil() {
        unsafe { std::ptr::write_bytes(dest, 0, *size) };
        return Ok(());
      }
      let bytes = integer_bytes(ty, value, *size, *signed, *role)?;
      if matches!(role, IntRole::NonZero | IntRole::OptionalNonZero)
        && bytes.iter().all(|b| *b == 0)
      {
        return Err(Fail::range(format!(
          "'{}' can never be zero{}",
          ty.name,
          if *role == IntRole::OptionalNonZero {
            "; pass nil for None"
          } else {
            ""
          }
        )));
      }
      unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), dest, *size) };
      Ok(())
    },
    Kind::Enum(info) => {
      let number = if value.is_string() {
        let name = value.as_str();
        let found = info
          .value_of(name)
          .ok_or_else(|| Fail::value(format!("'{name}' is not a constant of '{}'", ty.name)))?;
        number_value(vm, found)
      } else {
        value
      };
      store(vm, &info.underlying, number, dest, to)
    },
    Kind::Float => {
      let n = float_of(ty, value)?;
      unsafe { (dest as *mut f32).write_unaligned(n as f32) };
      Ok(())
    },
    Kind::Double => {
      let n = float_of(ty, value)?;
      unsafe { (dest as *mut f64).write_unaligned(n) };
      Ok(())
    },
    Kind::LongDouble => {
      let n = float_of(ty, value)?;
      match LONG_DOUBLE {
        LongDoubleRepr::X87 => unsafe {
          std::ptr::copy_nonoverlapping(floats::to_x87(n).as_ptr(), dest, 16)
        },
        LongDoubleRepr::Quad => unsafe {
          std::ptr::copy_nonoverlapping(floats::to_quad(n).as_ptr(), dest, 16)
        },
        LongDoubleRepr::Double => unsafe { (dest as *mut f64).write_unaligned(n) },
      }
      Ok(())
    },
    Kind::ComplexFloat | Kind::ComplexDouble => {
      let (re, im) = complex_of(ty, value)?;
      if matches!(ty.kind, Kind::ComplexFloat) {
        unsafe {
          (dest as *mut f32).write_unaligned(re as f32);
          (dest.add(4) as *mut f32).write_unaligned(im as f32);
        }
      } else {
        unsafe {
          (dest as *mut f64).write_unaligned(re);
          (dest.add(8) as *mut f64).write_unaligned(im);
        }
      }
      Ok(())
    },
    Kind::RustChar => {
      let code = if value.is_string() {
        let mut chars = value.as_str().chars();
        match (chars.next(), chars.next()) {
          (Some(c), None) => c as u32,
          _ => return Err(expected(ty, "a one-character string", value)),
        }
      } else if value.is_number() {
        let n = value.as_number();
        let code = n as u32;
        if n.fract() != 0.0 || n < 0.0 || char::from_u32(code).is_none() {
          return Err(Fail::range(format!("{n} is not a Unicode scalar value")));
        }
        code
      } else {
        return Err(expected(ty, "a one-character string", value));
      };
      unsafe { (dest as *mut u32).write_unaligned(code) };
      Ok(())
    },
    Kind::Pointer(_) => {
      let address = pointer_value(vm, ty, value, to)?;
      unsafe { (dest as *mut usize).write_unaligned(address) };
      Ok(())
    },
    Kind::Array { element, length } => store_array(vm, ty, element, *length, value, dest, to),
    Kind::Record(_) => store_record(vm, ty, value, dest, to),
  }
}

fn expected(ty: &CType, what: &str, got: Value) -> Fail {
  Fail::type_error(format!(
    "'{}' expects {what}, got {}",
    ty.name,
    got.argument_type_name()
  ))
}

/// The bytes of `value` as an integer of `size` bytes, little-endian.
fn integer_bytes(
  ty: &CType,
  value: Value,
  size: usize,
  signed: bool,
  role: IntRole,
) -> Result<[u8; 16], Fail> {
  let out_of_range = |shown: String| {
    let (low, high) = bounds(size, signed);
    Fail::range(format!(
      "{shown} is out of range for '{}', which holds {low} to {high}",
      ty.name
    ))
  };

  if value.is_number() {
    let n = value.as_number();

    if !n.is_finite() || n.fract() != 0.0 {
      return Err(expected(ty, "an integer", value));
    }

    // Past 2^127 an f64 is still integral but no C type here holds it.
    if n.abs() >= 1.7e38 {
      return Err(out_of_range(format!("{n}")));
    }

    return fit(n as i128, size, signed).ok_or_else(|| out_of_range(format!("{n}")));
  }

  if value.is_bigint() {
    let big = value.as_bigint();

    if size == 16 && !signed {
      let n = big.to_u128().ok_or_else(|| out_of_range(big.to_string()))?;
      return Ok(n.to_le_bytes());
    }

    let n = big.to_i128().ok_or_else(|| out_of_range(big.to_string()))?;
    return fit(n, size, signed).ok_or_else(|| out_of_range(big.to_string()));
  }

  if role == IntRole::Character && value.is_string() {
    let mut chars = value.as_str().chars();
    let (Some(c), None) = (chars.next(), chars.next()) else {
      return Err(expected(ty, "a number or a one-character string", value));
    };

    let code = c as u32;
    let limit: u32 = match size {
      1 => 0x7f,
      2 => 0xffff,
      _ => 0x10ffff,
    };

    if code > limit {
      return Err(Fail::range(format!(
        "'{c}' does not fit in a single '{}'; pass a string where a pointer is expected",
        ty.name
      )));
    }

    return fit(code as i128, size, signed).ok_or_else(|| out_of_range(format!("'{c}'")));
  }

  let what = if role == IntRole::Character {
    "a number or a one-character string"
  } else {
    "an integer"
  };
  Err(expected(ty, what, value))
}

/// The smallest and largest value an integer type holds, as text.
fn bounds(size: usize, signed: bool) -> (String, String) {
  if size == 16 {
    return if signed {
      (i128::MIN.to_string(), i128::MAX.to_string())
    } else {
      ("0".into(), u128::MAX.to_string())
    };
  }

  let bits = size * 8;
  if signed {
    let high = (1i128 << (bits - 1)) - 1;
    ((-high - 1).to_string(), high.to_string())
  } else {
    ("0".into(), ((1i128 << bits) - 1).to_string())
  }
}

fn fit(n: i128, size: usize, signed: bool) -> Option<[u8; 16]> {
  if size < 16 {
    let bits = size * 8;
    let (low, high) = if signed {
      (-(1i128 << (bits - 1)), (1i128 << (bits - 1)) - 1)
    } else {
      (0, (1i128 << bits) - 1)
    };
    if n < low || n > high {
      return None;
    }
  } else if !signed && n < 0 {
    return None;
  }

  Some(n.to_le_bytes())
}

fn float_of(ty: &CType, value: Value) -> Result<f64, Fail> {
  if value.is_number() {
    return Ok(value.as_number());
  }

  if value.is_bigint() {
    return value.as_bigint().to_f64().ok_or_else(|| {
      Fail::range(format!(
        "{} is out of range for '{}'",
        value.as_bigint(),
        ty.name
      ))
    });
  }

  Err(expected(ty, "a number", value))
}

fn complex_of(ty: &CType, value: Value) -> Result<(f64, f64), Fail> {
  if value.is_number() {
    return Ok((value.as_number(), 0.0));
  }

  if value.is_list() && value.list_len() == 2 {
    let re = value.list_get(0).unwrap();
    let im = value.list_get(1).unwrap();
    if re.is_number() && im.is_number() {
      return Ok((re.as_number(), im.as_number()));
    }
  }

  Err(expected(
    ty,
    "a number or a list of two numbers, [real, imaginary]",
    value,
  ))
}

/// Encodes `text` with a terminating zero code unit.
pub fn encode(text: &str, encoding: Encoding) -> Vec<u8> {
  match encoding.resolve() {
    Encoding::Utf8 => {
      let mut out = Vec::with_capacity(text.len() + 1);
      out.extend_from_slice(text.as_bytes());
      out.push(0);
      out
    },
    Encoding::Utf16 => {
      let mut out = Vec::with_capacity(text.len() * 2 + 2);
      for unit in text.encode_utf16() {
        out.extend_from_slice(&unit.to_le_bytes());
      }
      out.extend_from_slice(&[0, 0]);
      out
    },
    _ => {
      let mut out = Vec::with_capacity(text.len() * 4 + 4);
      for c in text.chars() {
        out.extend_from_slice(&(c as u32).to_le_bytes());
      }
      out.extend_from_slice(&[0, 0, 0, 0]);
      out
    },
  }
}

/// The encoding a string passed to a pointer of type `ty` travels in,
/// or `None` when that pointer does not take text.
fn text_encoding(ty: &CType) -> Option<Encoding> {
  let info = ty.pointer()?;

  if let Some(text) = info.text {
    return Some(text);
  }

  match &info.target.kind {
    Kind::Void => Some(Encoding::Utf8),
    Kind::Int {
      size: 1,
      role: IntRole::Character,
      ..
    } => Some(Encoding::Utf8),
    Kind::Int { size: 1, .. } => Some(Encoding::Utf8),
    Kind::Int {
      size,
      role: IntRole::Character,
      ..
    } => {
      if *size == super::types::WCHAR.0 && info.target.name.contains("wchar_t") {
        Some(Encoding::Wide)
      } else if *size == 2 {
        Some(Encoding::Utf16)
      } else {
        Some(Encoding::Utf32)
      }
    },
    _ => None,
  }
}

/// The address `value` stands for as a pointer of type `ty`.
fn pointer_value(vm: &mut VM, ty: &TypeRef, value: Value, to: &mut Dest) -> Result<usize, Fail> {
  let info = ty
    .pointer()
    .expect("pointer_value is only reached for pointer types");

  if value.is_nil() {
    if info.nonnull {
      return Err(Fail::type_error(format!(
        "'{}' can never be null, so nil cannot be passed for it",
        ty.name
      )));
    }
    return Ok(0);
  }

  if let Some(handle) = handle_of(value) {
    if handle.is_ptr_type(POINTER) {
      let data = pointer_data(handle);
      if let Some(block) = &data.block
        && block.is_freed()
      {
        return Err(Fail::pointer("this pointer's memory has been freed"));
      }
      if data.address == 0 && info.nonnull {
        return Err(Fail::type_error(format!("'{}' can never be null", ty.name)));
      }
      return Ok(data.address);
    }

    if handle.is_ptr_type(CALLBACK) {
      let cell = handle.as_ptr_cell().borrow();
      let core = cell.downcast_ref::<Arc<CallbackCore>>().unwrap();
      if core.is_released() {
        return Err(Fail::callback("this callback has been released"));
      }
      return Ok(core.code());
    }

    if handle.is_ptr_type(FUNCTION) {
      let cell = handle.as_ptr_cell().borrow();
      let function = cell
        .downcast_ref::<Arc<super::call::ForeignFunction>>()
        .unwrap();
      return Ok(function.address);
    }
  }

  if value.is_bound_method() {
    let receiver = value.as_bound_method().receiver;
    if receiver.is_ptr_type(FUNCTION) {
      let cell = receiver.as_ptr_cell().borrow();
      let function = cell
        .downcast_ref::<Arc<super::call::ForeignFunction>>()
        .unwrap();
      return Ok(function.address);
    }
  }

  let Some(scratch) = to.scratch() else {
    return Err(Fail::type_error(format!(
      "only nil, a Pointer, a Callback or a foreign function can be stored as '{}' in memory; \
       {} would not outlive this statement, so allocate it with ffi.alloc() first",
      ty.name,
      value.argument_type_name()
    )));
  };

  if value.is_string() {
    let Some(encoding) = text_encoding(ty) else {
      return Err(Fail::type_error(format!(
        "a string cannot be passed as '{}', which does not point at characters",
        ty.name
      )));
    };
    let bytes = encode(value.as_str(), encoding);
    return Ok(scratch.keep(&bytes) as usize);
  }

  if value.is_bytes() {
    // The bytes value's own storage, with nothing copied. It stays put
    // for the call as long as nothing resizes that same value, which
    // only a callback could do.
    let address = value.with_bytes_mut(|b| b.as_mut_ptr() as usize);
    return Ok(address);
  }

  if let Some(sig) = ty.function_pointer() {
    if value.is_callable() && !value.is_class() {
      let core = callback::CallbackCore::temporary(vm, value, sig.clone())?;
      let code = core.code();
      if let Dest::Call(scratch) = to {
        scratch.callbacks.push(core);
      }
      return Ok(code);
    }
  }

  let target = info.target.clone();

  if value.is_list() {
    let size = target
      .require_size()
      .map_err(|e| Fail::type_error(format!("a list cannot be passed as '{}': {e}", ty.name)))?;
    let items = value.as_list();
    let count = items.len();
    let address = match to.scratch() {
      Some(s) => s.buffer(size * count),
      None => unreachable!(),
    };

    for (i, item) in items.into_iter().enumerate() {
      store(vm, &target, item, unsafe { address.add(i * size) }, to)?;
    }

    if !target.is_const {
      let pin = vm.pin_values([value]);
      if let Dest::Call(scratch) = to {
        scratch.copy_back.push(CopyBack {
          pin,
          ty: target,
          address: address as usize,
          count: Some(count),
        });
      }
    }

    return Ok(address as usize);
  }

  if value.is_dict() && target.is_record() {
    let size = target.require_size().map_err(Fail::from_type)?;
    let address = match to.scratch() {
      Some(s) => s.buffer(size),
      None => unreachable!(),
    };

    store(vm, &target, value, address, to)?;

    if !target.is_const {
      let pin = vm.pin_values([value]);
      if let Dest::Call(scratch) = to {
        scratch.copy_back.push(CopyBack {
          pin,
          ty: target,
          address: address as usize,
          count: None,
        });
      }
    }

    return Ok(address as usize);
  }

  if value.is_callable() && ty.function_pointer().is_none() {
    return Err(Fail::type_error(format!(
      "a function passed as '{}' needs a signature; wrap it with ffi.callback() first",
      ty.name
    )));
  }

  Err(Fail::type_error(format!(
    "'{}' expects a Pointer, nil, a string, bytes, a list or a dictionary, got {}",
    ty.name,
    value.argument_type_name()
  )))
}

fn store_array(
  vm: &mut VM,
  ty: &TypeRef,
  element: &TypeRef,
  length: Option<usize>,
  value: Value,
  dest: *mut u8,
  to: &mut Dest,
) -> Result<(), Fail> {
  let Some(length) = length else {
    return Err(Fail::type_error(format!(
      "'{}' has no length, so it cannot be stored as a whole; write its elements through a pointer",
      ty.name
    )));
  };

  let step = element.require_size().map_err(Fail::from_type)?;
  let total = step * length;

  if (value.is_string() || value.is_bytes()) && step == 1 {
    let bytes: Vec<u8> = if value.is_string() {
      value.as_str().as_bytes().to_vec()
    } else {
      value.as_bytes()
    };

    if bytes.len() > length {
      return Err(Fail::range(format!(
        "{} bytes do not fit in '{}'",
        bytes.len(),
        ty.name
      )));
    }

    unsafe {
      std::ptr::write_bytes(dest, 0, total);
      std::ptr::copy_nonoverlapping(bytes.as_ptr(), dest, bytes.len());
    }
    return Ok(());
  }

  if !value.is_list() {
    return Err(expected(ty, "a list", value));
  }

  let items = value.as_list();

  if items.len() > length {
    return Err(Fail::range(format!(
      "{} elements do not fit in '{}'",
      items.len(),
      ty.name
    )));
  }

  unsafe { std::ptr::write_bytes(dest, 0, total) };

  for (i, item) in items.into_iter().enumerate() {
    store(vm, element, item, unsafe { dest.add(i * step) }, to)?;
  }

  Ok(())
}

fn store_record(
  vm: &mut VM,
  ty: &TypeRef,
  value: Value,
  dest: *mut u8,
  to: &mut Dest,
) -> Result<(), Fail> {
  let record = ty.record().unwrap().clone();
  let layout = record.layout().map_err(Fail::from_type)?;

  // A pointer to a value of the same type is copied from.
  if let Some(handle) = handle_of(value)
    && handle.is_ptr_type(POINTER)
  {
    let data = pointer_data(handle);
    let same = data.element.as_ref().is_some_and(|e| CType::same(e, ty));

    if !same {
      return Err(Fail::type_error(format!(
        "a Pointer can only stand in for a '{}' value when it points at one",
        ty.name
      )));
    }

    let source = data.check(0, layout.size)?;
    unsafe { std::ptr::copy(source as *const u8, dest, layout.size) };
    return Ok(());
  }

  if let Some(Some(variants)) = record.variants.get() {
    return store_variant(vm, ty, variants, value, dest, to);
  }

  if !value.is_dict() {
    return Err(expected(ty, "a dictionary", value));
  }

  unsafe { std::ptr::write_bytes(dest, 0, layout.size) };

  let pairs = value.as_dict();

  if record.is_union && pairs.len() > 1 {
    return store_union_members(vm, ty, &layout.fields, &pairs, layout.size, dest, to);
  }

  for (key, item) in pairs {
    if !key.is_string() {
      return Err(Fail::type_error(format!(
        "the keys of a '{}' value must be member names",
        ty.name
      )));
    }

    let name = key.as_str().to_string();
    let Some(field) = layout.find(&name) else {
      return Err(Fail::type_error(format!(
        "'{}' has no member named '{name}'",
        ty.name
      )));
    };

    store_field(vm, &field, item, dest, to)?;
  }

  Ok(())
}

/// Stores a union given several members, which is what reading one
/// back from C produces. They are accepted when they agree: each is
/// written on its own and must leave the same bytes the others do.
fn store_union_members(
  vm: &mut VM,
  ty: &TypeRef,
  fields: &[super::types::Field],
  pairs: &[(Value, Value)],
  size: usize,
  dest: *mut u8,
  to: &mut Dest,
) -> Result<(), Fail> {
  let mut first: Option<Vec<u8>> = None;

  for (key, item) in pairs {
    if !key.is_string() {
      return Err(Fail::type_error(format!(
        "the keys of a '{}' value must be member names",
        ty.name
      )));
    }

    let name = key.as_str();
    let Some(field) = fields.iter().find(|f| f.name == name) else {
      return Err(Fail::type_error(format!(
        "'{}' has no member named '{name}'",
        ty.name
      )));
    };

    let mut alone = vec![0u8; size.max(1)];
    store_field(vm, field, *item, alone.as_mut_ptr(), to)?;

    let span = match field.bits {
      Some(_) => size,
      None => field.ty.size().unwrap_or(size).min(size),
    };

    match &first {
      None => first = Some(alone),
      Some(existing) => {
        let common = span.min(size);
        if existing[..common] != alone[..common] {
          return Err(Fail::type_error(format!(
            "a '{}' holds one member at a time, and '{name}' disagrees with the others",
            ty.name
          )));
        }
      },
    }
  }

  if let Some(bytes) = first {
    unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), dest, size) };
  }

  Ok(())
}

/// Writes one member of the record at `base`.
pub fn store_field(
  vm: &mut VM,
  field: &super::types::Field,
  value: Value,
  base: *mut u8,
  to: &mut Dest,
) -> Result<(), Fail> {
  match field.bits {
    None => store(vm, &field.ty, value, unsafe { base.add(field.offset) }, to),
    Some((bit, width)) => {
      let (signed, size) = match &field.ty.kind {
        Kind::Int { signed, size, .. } => (*signed, *size),
        Kind::Enum(e) => match e.underlying.kind {
          Kind::Int { signed, size, .. } => (signed, size),
          _ => (false, 4),
        },
        _ => (false, 1),
      };

      let raw_value = if field.ty.kind_name() == "bool" {
        if !value.is_bool() {
          return Err(expected(&field.ty, "a bool", value));
        }
        value.as_bool() as i128
      } else {
        let mut scratch_bytes = [0u8; 16];
        let number = if let Kind::Enum(e) = &field.ty.kind
          && value.is_string()
        {
          let name = value.as_str();
          let v = e.value_of(name).ok_or_else(|| {
            Fail::value(format!("'{name}' is not a constant of '{}'", field.ty.name))
          })?;
          number_value(vm, v)
        } else {
          value
        };
        let bytes = integer_bytes(&field.ty, number, size, signed, IntRole::Plain)?;
        scratch_bytes.copy_from_slice(&bytes);
        i128::from_le_bytes(scratch_bytes)
      };

      let width = width as usize;
      let (low, high) = if signed {
        (-(1i128 << (width - 1)), (1i128 << (width - 1)) - 1)
      } else {
        (0, (1i128 << width) - 1)
      };

      if raw_value < low || raw_value > high {
        return Err(Fail::range(format!(
          "{raw_value} does not fit in the {width}-bit field '{}', which holds {low} to {high}",
          field.name
        )));
      }

      write_bits(base, bit, width, raw_value as u128);
      Ok(())
    },
  }
}

fn write_bits(base: *mut u8, bit: usize, width: usize, value: u128) {
  let first = bit / 8;
  let shift = bit % 8;
  let span = (shift + width).div_ceil(8);
  let mut window = [0u8; 17];

  unsafe { std::ptr::copy_nonoverlapping(base.add(first), window.as_mut_ptr(), span) };

  let mut current = u128::from_le_bytes(window[..16].try_into().unwrap());
  let mask = if width == 128 {
    u128::MAX
  } else {
    (1u128 << width) - 1
  };
  current &= !(mask << shift);
  current |= (value & mask) << shift;

  let bytes = current.to_le_bytes();
  window[..16].copy_from_slice(&bytes);
  unsafe { std::ptr::copy_nonoverlapping(window.as_ptr(), base.add(first), span.min(16)) };
}

fn read_bits(base: *const u8, bit: usize, width: usize, signed: bool) -> i128 {
  let first = bit / 8;
  let shift = bit % 8;
  let span = (shift + width).div_ceil(8).min(16);
  let mut window = [0u8; 16];

  unsafe { std::ptr::copy_nonoverlapping(base.add(first), window.as_mut_ptr(), span) };

  let raw = u128::from_le_bytes(window) >> shift;
  let mask = if width == 128 {
    u128::MAX
  } else {
    (1u128 << width) - 1
  };
  let unsigned = raw & mask;

  if signed && width < 128 && unsigned >> (width - 1) & 1 == 1 {
    (unsigned | !mask) as i128
  } else {
    unsigned as i128
  }
}

fn store_variant(
  vm: &mut VM,
  ty: &TypeRef,
  variants: &super::types::Variants,
  value: Value,
  dest: *mut u8,
  to: &mut Dest,
) -> Result<(), Fail> {
  let layout = ty.record().unwrap().layout().map_err(Fail::from_type)?;

  let (name, fields) = if value.is_string() {
    (value.as_str().to_string(), Vec::new())
  } else if value.is_dict() {
    let variant_key = value
      .as_dict()
      .into_iter()
      .find(|(k, _)| k.is_string() && k.as_str() == "variant")
      .map(|(_, v)| v);
    let Some(name) = variant_key.filter(|v| v.is_string()) else {
      return Err(Fail::type_error(format!(
        "a '{}' value is a dictionary with a 'variant' key naming the variant",
        ty.name
      )));
    };
    let rest = value
      .as_dict()
      .into_iter()
      .filter(|(k, _)| !(k.is_string() && k.as_str() == "variant"))
      .collect::<Vec<_>>();
    (name.as_str().to_string(), rest)
  } else {
    return Err(expected(
      ty,
      "a variant name or a dictionary with a 'variant' key",
      value,
    ));
  };

  let Some(case) = variants.cases.iter().find(|c| c.name == name) else {
    return Err(Fail::value(format!(
      "'{}' has no variant named '{name}'",
      ty.name
    )));
  };

  unsafe { std::ptr::write_bytes(dest, 0, layout.size) };

  let tag = number_value(vm, case.discriminant);
  store(vm, &variants.tag, tag, dest, to)?;

  let Some(payload) = &case.payload else {
    if !fields.is_empty() {
      return Err(Fail::type_error(format!(
        "the '{name}' variant of '{}' has no fields",
        ty.name
      )));
    }
    return Ok(());
  };

  let payload_offset = if variants.tagged_union {
    layout.find("payload").map(|f| f.offset).unwrap_or(0)
  } else {
    0
  };

  let payload_layout = payload
    .record()
    .unwrap()
    .layout()
    .map_err(Fail::from_type)?;
  let base = unsafe { dest.add(payload_offset) };

  for (key, item) in fields {
    let key_name = if key.is_string() {
      key.as_str().to_string()
    } else if key.is_number() {
      format!("{}", key.as_number())
    } else {
      return Err(Fail::type_error(
        "variant fields are named by string or position",
      ));
    };

    let Some(field) = payload_layout.find(&key_name).filter(|f| f.name != "@tag") else {
      return Err(Fail::type_error(format!(
        "the '{name}' variant of '{}' has no field '{key_name}'",
        ty.name
      )));
    };

    store_field(vm, &field, item, base, to)?;
  }

  Ok(())
}

/// A value for an integer that fits a number, or a bigint when it does
/// not.
pub fn number_value(vm: &mut VM, n: i128) -> Value {
  if (-SAFE_INTEGER..=SAFE_INTEGER).contains(&n) {
    Value::number(n as f64)
  } else {
    vm.heap_mut().alloc_bigint(BigInt::from(n))
  }
}

fn unsigned_value(vm: &mut VM, n: u128) -> Value {
  if n <= SAFE_INTEGER as u128 {
    Value::number(n as f64)
  } else {
    vm.heap_mut().alloc_bigint(BigInt::from(n))
  }
}

// Loading.

/// Reads a `ty` out of `src`, which need not be aligned.
pub fn load(vm: &mut VM, ty: &TypeRef, src: *const u8) -> Result<Value, Fail> {
  match &ty.kind {
    Kind::Void => Ok(Value::nil()),
    Kind::Function(_) => Err(Fail::type_error(format!(
      "a function has no value to read; read a pointer to '{}' instead",
      ty.name
    ))),
    Kind::Bool => Ok(Value::bool(unsafe { *src } != 0)),
    Kind::Int { size, signed, role } => {
      let value = load_int(vm, src, *size, *signed);
      if *role == IntRole::OptionalNonZero && value.is_number() && value.as_number() == 0.0 {
        return Ok(Value::nil());
      }
      Ok(value)
    },
    Kind::Enum(e) => load(vm, &e.underlying, src),
    Kind::Float => Ok(Value::number(
      unsafe { (src as *const f32).read_unaligned() } as f64,
    )),
    Kind::Double => Ok(Value::number(unsafe {
      (src as *const f64).read_unaligned()
    })),
    Kind::LongDouble => {
      let n = match LONG_DOUBLE {
        LongDoubleRepr::X87 => floats::from_x87(unsafe { std::slice::from_raw_parts(src, 16) }),
        LongDoubleRepr::Quad => floats::from_quad(unsafe { std::slice::from_raw_parts(src, 16) }),
        LongDoubleRepr::Double => unsafe { (src as *const f64).read_unaligned() },
      };
      Ok(Value::number(n))
    },
    Kind::ComplexFloat => {
      let re = unsafe { (src as *const f32).read_unaligned() } as f64;
      let im = unsafe { (src.add(4) as *const f32).read_unaligned() } as f64;
      Ok(
        vm.heap_mut()
          .alloc_list(vec![Value::number(re), Value::number(im)]),
      )
    },
    Kind::ComplexDouble => {
      let re = unsafe { (src as *const f64).read_unaligned() };
      let im = unsafe { (src.add(8) as *const f64).read_unaligned() };
      Ok(
        vm.heap_mut()
          .alloc_list(vec![Value::number(re), Value::number(im)]),
      )
    },
    Kind::RustChar => {
      let code = unsafe { (src as *const u32).read_unaligned() };
      let c = char::from_u32(code)
        .ok_or_else(|| Fail::value(format!("{code:#x} is not a Unicode scalar value")))?;
      Ok(vm.heap_mut().alloc_string(c.to_string()))
    },
    Kind::Pointer(info) => {
      let address = unsafe { (src as *const usize).read_unaligned() };
      load_pointer(vm, ty, info, address)
    },
    Kind::Array { element, length } => {
      let Some(length) = length else {
        return Err(Fail::type_error(format!(
          "'{}' has no length, so it cannot be read as a whole; read its elements through a pointer",
          ty.name
        )));
      };
      load_array(vm, element, *length, src)
    },
    Kind::Record(_) => load_record(vm, ty, src),
  }
}

fn load_int(vm: &mut VM, src: *const u8, size: usize, signed: bool) -> Value {
  let mut bytes = [0u8; 16];
  unsafe { std::ptr::copy_nonoverlapping(src, bytes.as_mut_ptr(), size) };

  if signed && bytes[size - 1] & 0x80 != 0 {
    for b in bytes.iter_mut().skip(size) {
      *b = 0xff;
    }
  }

  if !signed && size == 16 {
    return unsigned_value(vm, u128::from_le_bytes(bytes));
  }

  number_value(vm, i128::from_le_bytes(bytes))
}

/// What a pointer read out of memory or returned by a call becomes.
pub fn load_pointer(
  vm: &mut VM,
  ty: &TypeRef,
  info: &super::types::PointerInfo,
  address: usize,
) -> Result<Value, Fail> {
  if address == 0 {
    return Ok(Value::nil());
  }

  if let Some(encoding) = info.text {
    return Ok(read_text(vm, address, encoding, None, true)?);
  }

  if let Kind::Function(sig) = &info.target.kind {
    let function =
      super::call::ForeignFunction::from_address(address, sig.clone(), None, ty.name.clone());
    return Ok(super::call::function_value(vm, Arc::new(function)));
  }

  pointer_instance(vm, PointerData::raw(address, Some(info.target.clone())))
}

/// A `Pointer` instance for `data`.
pub fn pointer_instance(vm: &mut VM, data: PointerData) -> Result<Value, Fail> {
  let handle = vm.heap_mut().alloc_ptr(POINTER, data);
  wrap(vm, "Pointer", handle)
}

pub fn pointer_data(handle: Value) -> PointerData {
  let cell = handle.as_ptr_cell().borrow();
  cell
    .downcast_ref::<PointerData>()
    .expect("a pointer handle wraps PointerData")
    .clone()
}

/// Reads text at `address`: `length` code units when given, otherwise
/// up to the terminator. `lossy` decides whether malformed text is an
/// error or is patched with U+FFFD.
pub fn read_text(
  vm: &mut VM,
  address: usize,
  encoding: Encoding,
  length: Option<usize>,
  lossy: bool,
) -> Result<Value, Fail> {
  let encoding = encoding.resolve();
  let unit = encoding.unit();
  let count = match length {
    Some(n) => n,
    None => unsafe { terminated_len(address, unit, None) },
  };

  let bytes = unsafe { std::slice::from_raw_parts(address as *const u8, count * unit) };

  let text = match encoding {
    Encoding::Utf8 => match std::str::from_utf8(bytes) {
      Ok(s) => s.to_string(),
      Err(_) if lossy => String::from_utf8_lossy(bytes).into_owned(),
      Err(e) => {
        return Err(Fail::value(format!(
          "the text is not valid UTF-8 at byte {}",
          e.valid_up_to()
        )));
      },
    },
    Encoding::Utf16 => {
      let units: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .collect();
      match String::from_utf16(&units) {
        Ok(s) => s,
        Err(_) if lossy => String::from_utf16_lossy(&units),
        Err(_) => return Err(Fail::value("the text is not valid UTF-16")),
      }
    },
    _ => {
      let mut s = String::with_capacity(count);
      for c in bytes.chunks_exact(4) {
        let code = u32::from_le_bytes([c[0], c[1], c[2], c[3]]);
        match char::from_u32(code) {
          Some(ch) => s.push(ch),
          None if lossy => s.push('\u{fffd}'),
          None => {
            return Err(Fail::value(format!(
              "the text holds {code:#x}, which is not a Unicode scalar value"
            )));
          },
        }
      }
      s
    },
  };

  Ok(vm.heap_mut().alloc_string(text))
}

fn load_array(
  vm: &mut VM,
  element: &TypeRef,
  length: usize,
  src: *const u8,
) -> Result<Value, Fail> {
  let step = element.require_size().map_err(Fail::from_type)?;

  // A C string buffer reads as the string in it; other byte arrays as
  // bytes.
  if let Kind::Int {
    size: 1,
    role,
    signed,
  } = &element.kind
  {
    let raw_bytes = unsafe { std::slice::from_raw_parts(src, length) };
    let plain_char =
      *role == IntRole::Character && element.name.trim_start_matches("const ") == "char";
    let _ = signed;

    if plain_char {
      let end = raw_bytes.iter().position(|b| *b == 0).unwrap_or(length);
      let text = String::from_utf8_lossy(&raw_bytes[..end]).into_owned();
      return Ok(vm.heap_mut().alloc_string(text));
    }

    return Ok(vm.heap_mut().alloc_bytes(raw_bytes.to_vec()));
  }

  let mut items = Vec::with_capacity(length);
  for i in 0..length {
    let item = load(vm, element, unsafe { src.add(i * step) })?;
    let pin = vm.pin_values([item]);
    items.push(pin);
  }

  let values: Vec<Value> = items.iter().map(|p| vm.pinned(*p)).collect();
  Ok(vm.heap_mut().alloc_list(values))
}

fn load_record(vm: &mut VM, ty: &TypeRef, src: *const u8) -> Result<Value, Fail> {
  let record = ty.record().unwrap().clone();

  if let Some(Some(variants)) = record.variants.get() {
    return load_variant(vm, ty, variants, src);
  }

  let layout = record.layout().map_err(Fail::from_type)?;
  let mut pairs = Vec::new();

  for field in layout.flat_fields() {
    let item = load_field(vm, &field, src)?;
    let key = vm.heap_mut().alloc_string(field.name.clone());
    let pin = vm.pin_values([key, item]);
    pairs.push(pin);
  }

  let values: Vec<(Value, Value)> = pairs
    .iter()
    .map(|p| (vm.pinned(*p), vm.pinned(*p + 1)))
    .collect();
  Ok(vm.heap_mut().alloc_dict(values))
}

/// Reads one member of the record at `base`.
pub fn load_field(
  vm: &mut VM,
  field: &super::types::Field,
  base: *const u8,
) -> Result<Value, Fail> {
  match field.bits {
    None => load(vm, &field.ty, unsafe { base.add(field.offset) }),
    Some((bit, width)) => {
      let signed = match &field.ty.kind {
        Kind::Int { signed, .. } => *signed,
        Kind::Enum(e) => matches!(e.underlying.kind, Kind::Int { signed: true, .. }),
        _ => false,
      };
      let n = read_bits(base, bit, width as usize, signed);
      if matches!(field.ty.kind, Kind::Bool) {
        return Ok(Value::bool(n != 0));
      }
      Ok(number_value(vm, n))
    },
  }
}

fn load_variant(
  vm: &mut VM,
  ty: &TypeRef,
  variants: &super::types::Variants,
  src: *const u8,
) -> Result<Value, Fail> {
  let layout = ty.record().unwrap().layout().map_err(Fail::from_type)?;
  let tag = load(vm, &variants.tag, src)?;
  let discriminant = if tag.is_number() {
    tag.as_number() as i128
  } else {
    tag.as_bigint().to_i128().unwrap_or(i128::MAX)
  };

  let Some(case) = variants
    .cases
    .iter()
    .find(|c| c.discriminant == discriminant)
  else {
    return Err(Fail::value(format!(
      "discriminant {discriminant} matches no variant of '{}'",
      ty.name
    )));
  };

  let mut pins = Vec::new();
  let key = vm.heap_mut().alloc_string("variant");
  let name = vm.heap_mut().alloc_string(case.name.clone());
  pins.push(vm.pin_values([key, name]));

  if let Some(payload) = &case.payload {
    let payload_offset = if variants.tagged_union {
      layout.find("payload").map(|f| f.offset).unwrap_or(0)
    } else {
      0
    };
    let payload_layout = payload
      .record()
      .unwrap()
      .layout()
      .map_err(Fail::from_type)?;
    let base = unsafe { src.add(payload_offset) };

    for field in payload_layout.flat_fields() {
      if field.name == "@tag" {
        continue;
      }
      let item = load_field(vm, &field, base)?;
      let key = vm.heap_mut().alloc_string(field.name.clone());
      pins.push(vm.pin_values([key, item]));
    }
  }

  let values: Vec<(Value, Value)> = pins
    .iter()
    .map(|p| (vm.pinned(*p), vm.pinned(*p + 1)))
    .collect();
  Ok(vm.heap_mut().alloc_dict(values))
}

/// Writes every copied-back argument's final contents into the list or
/// dictionary it came from.
pub fn copy_back(vm: &mut VM, scratch: &mut Scratch) -> Result<(), Fail> {
  let pending = std::mem::take(&mut scratch.copy_back);

  for entry in pending {
    match entry.count {
      Some(count) => {
        let step = entry.ty.require_size().map_err(Fail::from_type)?;
        for i in 0..count {
          let item = load(vm, &entry.ty, (entry.address + i * step) as *const u8)?;
          let list = vm.pinned(entry.pin);
          list.list_set(i, item);
        }
      },
      None => {
        let fresh = load(vm, &entry.ty, entry.address as *const u8)?;
        let dict = vm.pinned(entry.pin);
        for (k, v) in fresh.as_dict() {
          dict.dict_set(k, v);
        }
      },
    }
  }

  Ok(())
}

/// Whether `value` is one of `libs/ffi`'s `Typed` wrappers, which fix
/// the C type of a variadic argument.
pub fn typed_argument(vm: &mut VM, value: Value) -> Option<(TypeRef, Value)> {
  if !is_instance_of(vm, value, "Typed") {
    return None;
  }

  let instance = value.as_instance();
  let class = instance.class.as_class();
  let type_slot = *class.field_slots.get("_type")?;
  let value_slot = *class.field_slots.get("_value")?;
  let ty_handle = handle_of(instance.fields[type_slot as usize].get())?;
  let inner = instance.fields[value_slot as usize].get();
  drop(class);

  Some((type_of(ty_handle).ok()?, inner))
}

/// The C type an untyped variadic argument is passed as.
pub fn promoted_type(vm: &mut VM, value: Value) -> Result<TypeRef, Fail> {
  if value.is_number() {
    let n = value.as_number();
    if n.fract() == 0.0 && n.is_finite() {
      if n >= i32::MIN as f64 && n <= i32::MAX as f64 {
        return Ok(builtin("int").unwrap());
      }
      if n.abs() < 9.3e18 {
        return Ok(builtin("long long").unwrap());
      }
    }
    return Ok(builtin("double").unwrap());
  }

  if value.is_bigint() {
    let big = value.as_bigint();
    if big.to_i64().is_some() {
      return Ok(builtin("long long").unwrap());
    }
    if big.to_u64().is_some() {
      return Ok(builtin("unsigned long long").unwrap());
    }
    return Err(Fail::range(format!(
      "{big} is too large to pass as a variadic argument"
    )));
  }

  if value.is_bool() {
    return Ok(builtin("int").unwrap());
  }

  if value.is_string() {
    return Ok(builtin("string").unwrap());
  }

  if value.is_nil() || value.is_bytes() || handle_of(value).is_some() || value.is_callable() {
    let _ = vm;
    return Ok(builtin("void *").unwrap());
  }

  Err(Fail::type_error(format!(
    "cannot tell which C type to pass {} as; give it one with a type's of() method",
    value.argument_type_name()
  )))
}

/// The C type behind a `Type` handle.
pub fn type_of(handle: Value) -> Result<TypeRef, Fail> {
  if !handle.is_ptr_type(super::TYPE) {
    return Err(Fail::type_error("expected an ffi type"));
  }

  let cell = handle.as_ptr_cell().borrow();
  Ok(cell.downcast_ref::<TypeRef>().unwrap().clone())
}

/// A bool argument to a variadic call travels as an `int`.
pub fn promote_bool(value: Value) -> Value {
  if value.is_bool() {
    Value::number(value.as_bool() as i32 as f64)
  } else {
    value
  }
}
