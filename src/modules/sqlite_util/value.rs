//! Moving values across the boundary between SQLite's storage classes
//! and Zuri's types.
//!
//! SQLite stores five things: NULL, a 64-bit integer, a double, text,
//! and a blob. Zuri has `nil`, one `number` type that is an f64, a
//! `bigint` for whole numbers beyond it, `bool`, `string` and `bytes`.
//! The mapping below is the whole of the translation, and every native
//! that reads or writes a value goes through it so the rules cannot
//! drift between the statement path, the user-function path and the
//! blob path.

use std::os::raw::{c_char, c_int, c_void};

use libsqlite3_sys as ffi;
use num_bigint::BigInt;
use num_traits::ToPrimitive;

use crate::vm::object::ZuriContext;
use crate::vm::value::Value;
use crate::vm::vm::VM;

/// The largest whole number an f64 represents exactly. An integer from
/// SQLite beyond this becomes a `bigint` rather than a `number`, since
/// handing back a silently rounded id is worse than handing back a type
/// the program has to think about.
const EXACT_INTEGER_LIMIT: i64 = 9_007_199_254_740_992;

/// Whether `n` is a whole number that survives the trip through i64.
///
/// Used to decide whether a Zuri number binds as INTEGER or as REAL.
/// Zuri has a single number type, so the value itself is the only
/// evidence available about which the program meant.
fn integral(n: f64) -> Option<i64> {
  if !n.is_finite() || n.fract() != 0.0 {
    return None;
  }

  if n < -(EXACT_INTEGER_LIMIT as f64) || n > EXACT_INTEGER_LIMIT as f64 {
    return None;
  }

  Some(n as i64)
}

/// Turns an i64 out of SQLite into the narrowest Zuri type that holds
/// it exactly.
pub fn integer_value(ctx: &mut ZuriContext, n: i64) -> Value {
  if n.abs() <= EXACT_INTEGER_LIMIT {
    return Value::number(n as f64);
  }

  ctx.heap().alloc_bigint(BigInt::from(n))
}

/// As `integer_value`, for the callback paths that hold a VM rather
/// than a native's context.
pub fn integer_value_vm(vm: &mut VM, n: i64) -> Value {
  if n.abs() <= EXACT_INTEGER_LIMIT {
    return Value::number(n as f64);
  }

  vm.heap_mut().alloc_bigint(BigInt::from(n))
}

/// How a Zuri value wants to be handed to SQLite.
///
/// Resolved before any FFI call so the one place that decides what a
/// value means is separate from the several places that then bind it,
/// return it from a function, or compare it.
pub enum Bound {
  Null,
  Integer(i64),
  Real(f64),
  Text(String),
  Blob(Vec<u8>),
}

/// Works out how `value` binds, or says why it cannot.
///
/// Lists and dictionaries are deliberately refused here rather than
/// quietly encoded: SQLite has no structured type, and a driver that
/// guessed JSON would make the round trip lossy in a way the program
/// never asked for. `sql.sqlite` encodes them on the way in, where the
/// choice is visible.
pub fn bind_of(value: Value) -> Result<Bound, String> {
  if value.is_nil() {
    return Ok(Bound::Null);
  }

  if value.is_bool() {
    return Ok(Bound::Integer(if value.as_bool() { 1 } else { 0 }));
  }

  if value.is_number() {
    let n = value.as_number();
    return Ok(match integral(n) {
      Some(i) => Bound::Integer(i),
      None => Bound::Real(n),
    });
  }

  if value.is_bigint() {
    let big = value.as_bigint();
    return match big.to_i64() {
      Some(i) => Ok(Bound::Integer(i)),
      None => Err(format!(
        "bigint {big} is out of range for sqlite, which stores whole numbers as 64 bit integers"
      )),
    };
  }

  if value.is_string() {
    return Ok(Bound::Text(value.as_str().to_string()));
  }

  if value.is_bytes() {
    return Ok(Bound::Blob(value.with_bytes(|b| b.to_vec())));
  }

  Err(format!(
    "cannot store a {} in sqlite; expected nil, a bool, a number, a bigint, a string or bytes",
    value.type_name()
  ))
}

/// Binds `value` to parameter `index` (1 based) of `stmt`.
///
/// Text and blobs are passed with SQLITE_TRANSIENT so SQLite takes its
/// own copy; the Rust buffer is a temporary that goes out of scope as
/// soon as this returns.
pub fn bind(stmt: *mut ffi::sqlite3_stmt, index: c_int, value: Value) -> Result<i32, String> {
  let code = match bind_of(value)? {
    Bound::Null => unsafe { ffi::sqlite3_bind_null(stmt, index) },
    Bound::Integer(n) => unsafe { ffi::sqlite3_bind_int64(stmt, index, n) },
    Bound::Real(n) => unsafe { ffi::sqlite3_bind_double(stmt, index, n) },
    Bound::Text(text) => unsafe {
      ffi::sqlite3_bind_text(
        stmt,
        index,
        text.as_ptr() as *const c_char,
        text.len() as c_int,
        ffi::SQLITE_TRANSIENT(),
      )
    },
    Bound::Blob(data) => unsafe {
      ffi::sqlite3_bind_blob(
        stmt,
        index,
        data.as_ptr() as *const c_void,
        data.len() as c_int,
        ffi::SQLITE_TRANSIENT(),
      )
    },
  };

  Ok(code)
}

/// Reads column `index` (0 based) of the current row.
///
/// Text comes back as a Zuri string and is assumed to be UTF-8, which
/// is what SQLite stores for a database opened by this module. Invalid
/// sequences are replaced rather than raising, so one damaged row does
/// not make the rest of a table unreadable.
pub fn column(ctx: &mut ZuriContext, stmt: *mut ffi::sqlite3_stmt, index: c_int) -> Value {
  match unsafe { ffi::sqlite3_column_type(stmt, index) } {
    ffi::SQLITE_NULL => Value::nil(),
    ffi::SQLITE_INTEGER => {
      let n = unsafe { ffi::sqlite3_column_int64(stmt, index) };
      integer_value(ctx, n)
    }
    ffi::SQLITE_FLOAT => Value::number(unsafe { ffi::sqlite3_column_double(stmt, index) }),
    ffi::SQLITE_BLOB => {
      let len = unsafe { ffi::sqlite3_column_bytes(stmt, index) } as usize;
      let ptr = unsafe { ffi::sqlite3_column_blob(stmt, index) } as *const u8;

      // A zero length blob is stored and read back as an empty one; the
      // pointer for it is null, which is not something to dereference.
      let data = if ptr.is_null() || len == 0 {
        Vec::new()
      } else {
        unsafe { std::slice::from_raw_parts(ptr, len) }.to_vec()
      };

      ctx.heap().alloc_bytes(data)
    }
    _ => {
      let text = column_text(stmt, index);
      ctx.heap().alloc_string(text)
    }
  }
}

/// The text of column `index`, read through SQLite's own length rather
/// than by scanning for a NUL, so a stored string containing one comes
/// back whole.
fn column_text(stmt: *mut ffi::sqlite3_stmt, index: c_int) -> String {
  let len = unsafe { ffi::sqlite3_column_bytes(stmt, index) } as usize;
  let ptr = unsafe { ffi::sqlite3_column_text(stmt, index) };

  if ptr.is_null() || len == 0 {
    return String::new();
  }

  let bytes = unsafe { std::slice::from_raw_parts(ptr, len) };
  String::from_utf8_lossy(bytes).into_owned()
}

/// Reads one of the arguments SQLite passes to a user-defined function.
pub fn argument(vm: &mut VM, arg: *mut ffi::sqlite3_value) -> Value {
  match unsafe { ffi::sqlite3_value_type(arg) } {
    ffi::SQLITE_NULL => Value::nil(),
    ffi::SQLITE_INTEGER => {
      let n = unsafe { ffi::sqlite3_value_int64(arg) };
      integer_value_vm(vm, n)
    }
    ffi::SQLITE_FLOAT => Value::number(unsafe { ffi::sqlite3_value_double(arg) }),
    ffi::SQLITE_BLOB => {
      let len = unsafe { ffi::sqlite3_value_bytes(arg) } as usize;
      let ptr = unsafe { ffi::sqlite3_value_blob(arg) } as *const u8;

      let data = if ptr.is_null() || len == 0 {
        Vec::new()
      } else {
        unsafe { std::slice::from_raw_parts(ptr, len) }.to_vec()
      };

      vm.heap_mut().alloc_bytes(data)
    }
    _ => {
      let len = unsafe { ffi::sqlite3_value_bytes(arg) } as usize;
      let ptr = unsafe { ffi::sqlite3_value_text(arg) };

      let text = if ptr.is_null() || len == 0 {
        String::new()
      } else {
        let bytes = unsafe { std::slice::from_raw_parts(ptr, len) };
        String::from_utf8_lossy(bytes).into_owned()
      };

      vm.heap_mut().alloc_string(text)
    }
  }
}

/// Hands a user-defined function's return value back to SQLite.
///
/// A value this module cannot store becomes an error on the function
/// call rather than a silent NULL, so a function returning the wrong
/// type fails the statement with something that names the problem.
pub fn result(context: *mut ffi::sqlite3_context, value: Value) {
  let bound = match bind_of(value) {
    Ok(bound) => bound,
    Err(message) => {
      set_error(context, &message);
      return;
    }
  };

  match bound {
    Bound::Null => unsafe { ffi::sqlite3_result_null(context) },
    Bound::Integer(n) => unsafe { ffi::sqlite3_result_int64(context, n) },
    Bound::Real(n) => unsafe { ffi::sqlite3_result_double(context, n) },
    Bound::Text(text) => unsafe {
      ffi::sqlite3_result_text(
        context,
        text.as_ptr() as *const c_char,
        text.len() as c_int,
        ffi::SQLITE_TRANSIENT(),
      )
    },
    Bound::Blob(data) => unsafe {
      ffi::sqlite3_result_blob(
        context,
        data.as_ptr() as *const c_void,
        data.len() as c_int,
        ffi::SQLITE_TRANSIENT(),
      )
    },
  }
}

/// Fails the running statement with `message`.
pub fn set_error(context: *mut ffi::sqlite3_context, message: &str) {
  unsafe {
    ffi::sqlite3_result_error(
      context,
      message.as_ptr() as *const c_char,
      message.len() as c_int,
    )
  };
}

/// The name SQLite's storage classes go by on the Zuri side.
pub fn type_name(code: c_int) -> &'static str {
  match code {
    ffi::SQLITE_INTEGER => "integer",
    ffi::SQLITE_FLOAT => "float",
    ffi::SQLITE_TEXT => "text",
    ffi::SQLITE_BLOB => "blob",
    _ => "null",
  }
}
