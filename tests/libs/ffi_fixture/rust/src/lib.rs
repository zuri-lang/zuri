//! The Rust library the ffi suites call into.
//!
//! The suites hand this file to `declare_rust()` exactly as it is, so it
//! is written the way a real crate exposes a C ABI: `use` lines, doc
//! comments, `impl` blocks and function bodies all sit around the
//! declarations that matter.

use std::ffi::{CStr, CString, c_char, c_int};
use std::num::NonZeroU32;

/// A point in the plane.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Point {
  pub x: f64,
  pub y: f64,
}

impl Point {
  fn length(&self) -> f64 {
    (self.x * self.x + self.y * self.y).sqrt()
  }
}

/// A slice as it crosses the C ABI.
#[repr(C)]
pub struct IntSlice {
  pub ptr: *const i32,
  pub len: usize,
}

/// Shapes, as a Rust enum with fields.
#[repr(C)]
pub enum Shape {
  Circle { radius: f64 },
  Rect { w: f64, h: f64 },
  Empty,
}

/// A token, laid out with a primitive representation.
#[repr(u8)]
pub enum Token {
  Number(i64),
  Plus,
  Word { len: u32, upper: bool },
}

/// An operation, as a fieldless enum with explicit discriminants.
#[repr(u8)]
#[derive(Clone, Copy)]
pub enum Op {
  Add = 1,
  Mul = 2,
  Neg = Op::Mul as u8 + 2,
}

#[repr(C, packed)]
pub struct Packed {
  pub a: u8,
  pub b: u32,
}

#[repr(C, align(16))]
pub struct Aligned {
  pub v: u32,
}

#[repr(transparent)]
pub struct Meters(pub f64);

/// Opaque to C: only ever handed out behind a pointer.
pub struct Counter {
  count: u64,
}

pub type Callback = extern "C" fn(f64, f64) -> f64;

pub const RS_LIMIT: usize = 4 * 16;
pub const RS_GREETING: &str = "hi";

#[unsafe(no_mangle)]
pub static RS_VERSION: u32 = 7;

#[unsafe(no_mangle)]
pub static mut RS_COUNTER: i32 = 0;

#[unsafe(no_mangle)]
pub extern "C" fn rs_distance(a: Point, b: Point) -> f64 {
  Point {
    x: a.x - b.x,
    y: a.y - b.y,
  }
  .length()
}

#[unsafe(no_mangle)]
pub extern "C" fn rs_midpoint(a: &Point, b: &Point) -> Point {
  Point {
    x: (a.x + b.x) / 2.0,
    y: (a.y + b.y) / 2.0,
  }
}

#[unsafe(no_mangle)]
pub extern "C" fn rs_length_or_zero(p: Option<&Point>) -> f64 {
  p.map(|p| p.length()).unwrap_or(0.0)
}

#[unsafe(no_mangle)]
pub extern "C" fn rs_add_i128(a: i128, b: i128) -> i128 {
  a.wrapping_add(b)
}

#[unsafe(no_mangle)]
pub extern "C" fn rs_mul_u128(a: u128, b: u128) -> u128 {
  a.wrapping_mul(b)
}

#[unsafe(no_mangle)]
pub extern "C" fn rs_i128_mix(a: u8, b: i128, c: u8, d: i128) -> i128 {
  a as i128 + b * 10 + c as i128 * 100 + d * 1000
}

#[unsafe(no_mangle)]
pub extern "C" fn rs_i128_via(
  f: extern "C" fn(u8, i128, u8, i128, u8, i128) -> i128,
  x: i128,
) -> i128 {
  f(1, x, 2, x, 3, x) + 1
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rs_sum_i128(values: *const i128, n: usize) -> i128 {
  let values = unsafe { std::slice::from_raw_parts(values, n) };
  values.iter().sum()
}

// A `char` is a Unicode scalar value in a `u32`, which is exactly the
// guarantee the other side relies on.
#[allow(improper_ctypes_definitions)]
#[unsafe(no_mangle)]
pub extern "C" fn rs_next_char(c: char) -> char {
  char::from_u32(c as u32 + 1).unwrap_or('?')
}

#[unsafe(no_mangle)]
pub extern "C" fn rs_is_even(n: u32) -> bool {
  n % 2 == 0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rs_sum_slice(s: IntSlice) -> i64 {
  let values = unsafe { std::slice::from_raw_parts(s.ptr, s.len) };
  values.iter().map(|v| *v as i64).sum()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rs_count_chars(text: *const u8, len: usize) -> usize {
  let bytes = unsafe { std::slice::from_raw_parts(text, len) };
  std::str::from_utf8(bytes).map(|s| s.chars().count()).unwrap_or(0)
}

#[unsafe(no_mangle)]
pub extern "C" fn rs_area(shape: Shape) -> f64 {
  match shape {
    Shape::Circle { radius } => 3.0 * radius * radius,
    Shape::Rect { w, h } => w * h,
    Shape::Empty => 0.0,
  }
}

#[unsafe(no_mangle)]
pub extern "C" fn rs_make_shape(kind: u8, size: f64) -> Shape {
  match kind {
    0 => Shape::Circle { radius: size },
    1 => Shape::Rect { w: size, h: size * 2.0 },
    _ => Shape::Empty,
  }
}

#[unsafe(no_mangle)]
pub extern "C" fn rs_token_value(token: Token) -> i64 {
  match token {
    Token::Number(n) => n,
    Token::Plus => -1,
    Token::Word { len, upper } => len as i64 * if upper { 10 } else { 1 },
  }
}

#[unsafe(no_mangle)]
pub extern "C" fn rs_make_word(len: u32, upper: bool) -> Token {
  Token::Word { len, upper }
}

#[unsafe(no_mangle)]
pub extern "C" fn rs_apply(op: Op, a: i32, b: i32) -> i32 {
  match op {
    Op::Add => a + b,
    Op::Mul => a * b,
    Op::Neg => -a,
  }
}

#[unsafe(no_mangle)]
pub extern "C" fn rs_packed_sum(p: Packed) -> u32 {
  p.a as u32 + { p.b }
}

#[unsafe(no_mangle)]
pub extern "C" fn rs_aligned_value(a: Aligned) -> u32 {
  a.v
}

#[unsafe(no_mangle)]
pub extern "C" fn rs_double_meters(m: Meters) -> Meters {
  Meters(m.0 * 2.0)
}

#[unsafe(no_mangle)]
pub extern "C" fn rs_nonzero_or(n: Option<NonZeroU32>, fallback: u32) -> u32 {
  n.map(|v| v.get()).unwrap_or(fallback)
}

#[unsafe(no_mangle)]
pub extern "C" fn rs_fold(values: *const f64, n: usize, f: Callback, start: f64) -> f64 {
  let values = unsafe { std::slice::from_raw_parts(values, n) };
  values.iter().fold(start, |acc, v| f(acc, *v))
}

#[unsafe(no_mangle)]
pub extern "C" fn rs_maybe_call(f: Option<extern "C" fn(c_int) -> c_int>, x: c_int) -> c_int {
  match f {
    Some(f) => f(x),
    None => -1,
  }
}

#[unsafe(no_mangle)]
pub extern "C" fn rs_counter_new(start: u64) -> *mut Counter {
  Box::into_raw(Box::new(Counter { count: start }))
}

#[unsafe(no_mangle)]
pub extern "C" fn rs_counter_bump(counter: &mut Counter) -> u64 {
  counter.count += 1;
  counter.count
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rs_counter_free(counter: *mut Counter) {
  if !counter.is_null() {
    drop(unsafe { Box::from_raw(counter) });
  }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rs_greet(name: *const c_char) -> *mut c_char {
  let name = unsafe { CStr::from_ptr(name) }.to_string_lossy();
  CString::new(format!("hello, {name}")).unwrap().into_raw()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rs_string_free(text: *mut c_char) {
  if !text.is_null() {
    drop(unsafe { CString::from_raw(text) });
  }
}

#[unsafe(no_mangle)]
pub extern "C" fn rs_version_text() -> *const c_char {
  c"ffirust 0.1".as_ptr()
}

#[unsafe(export_name = "rs_renamed_symbol")]
pub extern "C" fn rs_internal_name(x: i32) -> i32 {
  x * 7
}

#[unsafe(no_mangle)]
pub extern "C" fn rs_bump_counter() -> i32 {
  unsafe {
    RS_COUNTER += 1;
    RS_COUNTER
  }
}

/// Sizes and alignments as rustc lays these types out, for checking the
/// module's own.
#[unsafe(no_mangle)]
pub extern "C" fn rs_layouts(out: *mut usize) {
  let table = [
    size_of::<Point>(),
    align_of::<Point>(),
    size_of::<Shape>(),
    align_of::<Shape>(),
    size_of::<Token>(),
    align_of::<Token>(),
    size_of::<Packed>(),
    align_of::<Packed>(),
    size_of::<Aligned>(),
    align_of::<Aligned>(),
    size_of::<Op>(),
    align_of::<Op>(),
    size_of::<i128>(),
    align_of::<i128>(),
  ];
  let out = unsafe { std::slice::from_raw_parts_mut(out, table.len()) };
  out.copy_from_slice(&table);
}

#[cfg(windows)]
#[unsafe(no_mangle)]
pub extern "C" fn rs_platform() -> c_int {
  1
}

#[cfg(not(windows))]
#[unsafe(no_mangle)]
pub extern "C" fn rs_platform() -> c_int {
  2
}

fn private_helper() -> i32 {
  42
}

mod inner {
  #[unsafe(no_mangle)]
  pub extern "C" fn rs_from_module() -> i32 {
    super::private_helper()
  }
}
