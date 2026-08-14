#![allow(unused)]

use std::sync::LazyLock;

use crate::{
  builtins::{
    MethodTable, build,
    enforce::{
      ArgType, enforce_method_arg_count, enforce_method_arg_range, enforce_method_arg_type,
      enforce_method_arg_type_opt,
    },
    method, method_n, method_opt,
  },
  vm::{
    object::{Obj, ZuriContext},
    value::Value,
  },
};

pub static BYTES_METHODS: LazyLock<MethodTable> = LazyLock::new(|| {
  build(vec![
    // iterable protocol
    method_n("@key", 1, _key),
    method_n("@value", 1, _value),
    // NOTE: bytes.to_string() decodes as text, unlike every other
    // kind's to_string() (which reuses Value's own Display, and for
    // bytes that's the "(41 42 ...)" hex form used by echo) -- so this
    // deliberately does NOT reuse the shared `to_string` from
    // `builtins::mod`.
    method("to_string", bytes_to_string),
    method("length", length),
    method_n("append", 1, append),
    method("clone", clone_bytes),
    method_n("extend", 1, extend),
    method_opt("index_of", 1, index_of),
    method("pop", pop),
    method_n("remove", 1, remove),
    method("reverse", reverse),
    method("first", first),
    method("last", last),
    method_n("get", 1, get),
    method_n("split", 1, split),
    method("is_alpha", is_alpha),
    method("is_alnum", is_alnum),
    method("is_number", is_number),
    method("is_lower", is_lower),
    method("is_upper", is_upper),
    method("is_space", is_space),
    method("dispose", dispose),
    method("to_list", to_list),
    method_n("each", 1, each),
  ])
});

//-----------------------------------------------------------------------------------
// Helpers
//-----------------------------------------------------------------------------------

/// Run `f` with mutable access to the underlying `Vec<u8>` storage of
/// a bytes Value -- the `Obj::Bytes` counterpart to `list.rs`'s
/// `with_list_mut`, needed for anything push/insert/remove/drain-
/// shaped that `Value`'s own `bytes_get`/`bytes_set` API (fixed-size
/// element access only) doesn't cover.
fn with_bytes_mut<F, R>(v: Value, f: F) -> R
where
  F: FnOnce(&mut Vec<u8>) -> R,
{
  match unsafe { &*v.as_obj() } {
    Obj::Bytes(b) => f(&mut b.borrow_mut()),
    _ => unreachable!("with_bytes_mut called on a non-bytes Value"),
  }
}

/// A byte-stream element must be a whole number in 0..=255 -- same
/// constraint `Instr::SetIndex`'s bytes arm already enforces for
/// `bytes[i] = x`.
fn expect_byte(ctx: &ZuriContext, idx: usize) -> Result<u8, String> {
  let v = ctx.args[idx];
  if !v.is_number() {
    return Err(format!(
      "'{}' expects a number, got {}",
      ctx.name,
      v.type_name()
    ));
  }
  let n = v.as_number();
  if n.fract() != 0.0 || !(0.0..=255.0).contains(&n) {
    return Err(format!(
      "'{}' expects an integer in 0..=255, got {}",
      ctx.name, n
    ));
  }
  Ok(n as u8)
}

//-----------------------------------------------------------------------------------
// Implementations
//-----------------------------------------------------------------------------------

fn length(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  Ok(Value::number(ctx.args[0].bytes_len() as f64))
}

fn append(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  let byte = expect_byte(ctx, 1)?;
  with_bytes_mut(ctx.args[0], |v| v.push(byte));
  Ok(Value::nil())
}

fn clone_bytes(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  let bytes = ctx.args[0].as_bytes();
  Ok(ctx.vm.heap_mut().alloc_bytes(bytes))
}

/// In-place: appends the OTHER byte stream's content onto this one, as
/// documented ("extend() is an in-place action so the original byte
/// stream will be modified").
fn extend(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  enforce_method_arg_type!(ctx, 1, ArgType::Bytes);
  let other = ctx.args[1].as_bytes();
  with_bytes_mut(ctx.args[0], |v| v.extend(other));
  Ok(ctx.args[0])
}

fn index_of(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_range!(ctx, 1, 2);
  enforce_method_arg_type!(ctx, 1, ArgType::Number);
  enforce_method_arg_type_opt!(ctx, 2, ArgType::Number);

  let target = expect_byte(ctx, 1)?;
  let start = match ctx.args.get(2) {
    Some(v) => v.as_number().max(0.0) as usize,
    None => 0,
  };

  let bytes = ctx.args[0].as_bytes();
  if start >= bytes.len() {
    return Ok(Value::number(-1.0));
  }
  match bytes[start..].iter().position(|&b| b == target) {
    Some(pos) => Ok(Value::number((start + pos) as f64)),
    None => Ok(Value::number(-1.0)),
  }
}

fn pop(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  let popped = with_bytes_mut(ctx.args[0], |v| v.pop());
  Ok(
    popped
      .map(|b| Value::number(b as f64))
      .unwrap_or(Value::nil()),
  )
}

fn remove(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  enforce_method_arg_type!(ctx, 1, ArgType::Number);

  let idx = ctx.args[1].as_number();
  let len = ctx.args[0].bytes_len();
  if idx < 0.0 || idx as usize >= len {
    return Err(format!(
      "bytes index {} out of range at remove()",
      idx as i64
    ));
  }
  let idx = idx as usize;
  let removed = with_bytes_mut(ctx.args[0], |v| v.remove(idx));
  Ok(Value::number(removed as f64))
}

/// Reverses the byte stream IN PLACE -- mirrors the phrasing/behavior
/// of `list.sort()` ("...in-place and returns the sorted list"), not
/// `list.reverse()` (which explicitly returns a NEW list).
fn reverse(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  with_bytes_mut(ctx.args[0], |v| v.reverse());
  Ok(ctx.args[0])
}

fn first(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  Ok(
    ctx.args[0]
      .bytes_get(0)
      .map(|b| Value::number(b as f64))
      .unwrap_or(Value::nil()),
  )
}

fn last(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  let len = ctx.args[0].bytes_len();
  if len == 0 {
    return Ok(Value::nil());
  }
  Ok(
    ctx.args[0]
      .bytes_get(len - 1)
      .map(|b| Value::number(b as f64))
      .unwrap_or(Value::nil()),
  )
}

fn get(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  enforce_method_arg_type!(ctx, 1, ArgType::Number);

  let idx = ctx.args[1].as_number();
  let len = ctx.args[0].bytes_len();
  if idx < 0.0 || idx as usize >= len {
    return Err(format!("bytes index {} out of range at get()", idx as i64));
  }
  Ok(Value::number(
    ctx.args[0].bytes_get(idx as usize).unwrap() as f64
  ))
}

/// Splits on a delimiter byte sequence, returning a list of new bytes
/// objects. An EMPTY delimiter (`bytes(0)`, i.e. a zero-length byte
/// stream) splits into one single-byte bytes object per input byte --
/// mirrors `string.split()`'s own empty-delimiter special case, and is
/// what the documented `'test'.to_bytes().split(bytes(0))` example
/// actually exercises.
fn split(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  enforce_method_arg_type!(ctx, 1, ArgType::Bytes);

  let data = ctx.args[0].as_bytes();
  let delim = ctx.args[1].as_bytes();

  let parts: Vec<Vec<u8>> = if delim.is_empty() {
    data.iter().map(|&b| vec![b]).collect()
  } else {
    let mut parts = Vec::new();
    let mut start = 0usize;
    let mut i = 0usize;
    while i + delim.len() <= data.len() {
      if data[i..i + delim.len()] == delim[..] {
        parts.push(data[start..i].to_vec());
        i += delim.len();
        start = i;
      } else {
        i += 1;
      }
    }
    parts.push(data[start..].to_vec());
    parts
  };

  let items: Vec<Value> = parts
    .into_iter()
    .map(|p| ctx.vm.heap_mut().alloc_bytes(p))
    .collect();
  Ok(ctx.vm.heap_mut().alloc_list(items))
}

fn is_alpha(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  let b = ctx.args[0].as_bytes();
  Ok(Value::bool(
    !b.is_empty() && b.iter().all(|c| c.is_ascii_alphabetic()),
  ))
}

fn is_alnum(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  let b = ctx.args[0].as_bytes();
  Ok(Value::bool(
    !b.is_empty() && b.iter().all(|c| c.is_ascii_alphanumeric()),
  ))
}

fn is_number(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  let b = ctx.args[0].as_bytes();
  Ok(Value::bool(
    !b.is_empty() && b.iter().all(|c| c.is_ascii_digit()),
  ))
}

fn is_lower(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  let b = ctx.args[0].as_bytes();
  let cased: Vec<&u8> = b.iter().filter(|c| c.is_ascii_alphabetic()).collect();
  Ok(Value::bool(
    !cased.is_empty() && cased.iter().all(|c| c.is_ascii_lowercase()),
  ))
}

fn is_upper(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  let b = ctx.args[0].as_bytes();
  let cased: Vec<&u8> = b.iter().filter(|c| c.is_ascii_alphabetic()).collect();
  Ok(Value::bool(
    !cased.is_empty() && cased.iter().all(|c| c.is_ascii_uppercase()),
  ))
}

fn is_space(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  let b = ctx.args[0].as_bytes();
  Ok(Value::bool(
    !b.is_empty() && b.iter().all(|c| c.is_ascii_whitespace()),
  ))
}

/// Resets and empties the byte stream -- manual memory management, per
/// the doc's own framing. Functionally identical to truncating to
/// zero length; kept as its own native (rather than aliased to a
/// `clear`, which isn't itself a documented bytes method) to match the
/// documented name exactly.
fn dispose(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  with_bytes_mut(ctx.args[0], |v| v.clear());
  Ok(Value::nil())
}

fn to_list(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  let items: Vec<Value> = ctx.args[0]
    .as_bytes()
    .into_iter()
    .map(|b| Value::number(b as f64))
    .collect();
  Ok(ctx.vm.heap_mut().alloc_list(items))
}

fn bytes_to_string(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  let bytes = ctx.args[0].as_bytes();
  let s = String::from_utf8_lossy(&bytes).into_owned();
  Ok(ctx.vm.heap_mut().alloc_string(s))
}

fn each(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  enforce_method_arg_type!(ctx, 1, ArgType::Function);

  let bytes = ctx.args[0].as_bytes();
  // See `string.rs::each`'s identical pin -- `ctx.args` itself isn't
  // a GC root, so both the returned bytes object and the reused
  // `callback` need to survive here via `gc_pins` instead.
  let mark = ctx.vm.pin_values([ctx.args[0], ctx.args[1]]);

  for (i, b) in bytes.into_iter().enumerate() {
    let callback = ctx.vm.pinned(mark + 1);
    ctx
      .vm
      .call_value(
        callback,
        &[Value::number(b as f64), Value::number(i as f64)],
      )
      .map_err(|e| ctx.vm.describe_exception(e))?;
  }

  let bytes_val = ctx.vm.pinned(mark);
  ctx.vm.unpin(mark);
  Ok(bytes_val)
}

//-----------------------------------------------------------------------------------
// Iterable Decorators (@key / @value) -- unchanged from before this pass
//-----------------------------------------------------------------------------------

fn _key(ctx: &mut ZuriContext) -> Result<Value, String> {
  let val = ctx.args[1];
  let len = ctx.args[0].bytes_len();

  if len == 0 {
    return Ok(Value::nil());
  }

  if val.is_nil() {
    return Ok(Value::number(0.0));
  }
  if !val.is_number() {
    return Err(format!(
      "bytes are numerically indexed, {} given",
      val.type_name()
    ));
  }
  let index = val.as_number() as usize;
  if len > 0 && index < len - 1 {
    return Ok(Value::number(index as f64 + 1.0));
  }
  Ok(Value::nil())
}

fn _value(ctx: &mut ZuriContext) -> Result<Value, String> {
  if !ctx.args[1].is_number() {
    return Err("bytes are numerically indexed".to_string());
  }
  let index = ctx.args[1].as_number();
  let obj = ctx.args[0];
  if index > -1.0 && index < obj.bytes_len() as f64 {
    return Ok(Value::number(obj.bytes_get(index as usize).unwrap() as f64));
  }
  Ok(Value::nil())
}
