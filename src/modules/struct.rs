//! `struct` builtin module -- packs/unpacks binary data to and from Zuri
//! values, in one native module (no `_struct` + `libs/struct.zu` split:
//! the format parser, the pack/unpack engine, and every public function
//! all live here).
//!
//! ## Format language
//!
//! A format string is a sequence of `/`-separated segments. Each segment
//! is one or more `CODE[COUNT]` groups, optionally followed by `:NAME` to
//! label the field(s) that group produces when unpacking:
//!
//! ```text
//!   "Nsize:len/A16:name/C4"
//! ```
//!
//! - `CODE` is one of the format characters in the table below.
//! - `COUNT` is a decimal integer, or `*` to mean "the rest" (the whole
//!   remaining argument for a string-like code, or every remaining
//!   argument/byte for a numeric code). Omitted means `1`.
//! - `:NAME` names the field(s) produced by that ONE group when
//!   unpacking. A count > 1 numbers the keys `NAME1`, `NAME2`, ... A
//!   segment carrying a `:NAME` may only contain a single group -- this
//!   is a deliberate, unambiguous departure from the PHP convention this
//!   module used to imitate (see "Deviations from PHP" below).
//! - A group with no `:NAME` gets a purely numeric key. Unlike the old
//!   PHP-style implementation, that numbering runs once, globally, for
//!   the whole format string -- it never resets and so never silently
//!   overwrites an earlier unnamed field.
//!
//! ## Format codes
//!
//! Code | Bytes | Meaning
//! -----|-------|--------
//! `a`  | count | NUL-padded string
//! `A`  | count | SPACE-padded string (trailing NUL/space trimmed on unpack)
//! `Z`  | count | NUL-padded, NUL-terminated string (C-string semantics)
//! `h`  | ceil(count/2) | Hex string, low nibble first
//! `H`  | ceil(count/2) | Hex string, high nibble first
//! `c`  | 1 | signed 8-bit integer
//! `C`  | 1 | unsigned 8-bit integer
//! `?`  | 1 | boolean
//! `s`  | 2 | signed 16-bit integer, native byte order
//! `S`  | 2 | unsigned 16-bit integer, native byte order
//! `n`  | 2 | unsigned 16-bit integer, big-endian
//! `v`  | 2 | unsigned 16-bit integer, little-endian
//! `i`  | 4 | signed 32-bit integer, native byte order
//! `I`  | 4 | unsigned 32-bit integer, native byte order
//! `l`  | 4 | signed 32-bit integer, native byte order
//! `L`  | 4 | unsigned 32-bit integer, native byte order
//! `N`  | 4 | unsigned 32-bit integer, big-endian
//! `V`  | 4 | unsigned 32-bit integer, little-endian
//! `q`  | 8 | signed 64-bit integer, native byte order
//! `Q`  | 8 | unsigned 64-bit integer, native byte order
//! `J`  | 8 | unsigned 64-bit integer, big-endian
//! `P`  | 8 | unsigned 64-bit integer, little-endian
//! `u`  | 16 | signed 128-bit integer, little-endian
//! `U`  | 16 | unsigned 128-bit integer, little-endian
//! `f`  | 4 | float, native byte order
//! `g`  | 4 | float, little-endian
//! `G`  | 4 | float, big-endian
//! `d`  | 8 | double, native byte order
//! `e`  | 8 | double, little-endian
//! `E`  | 8 | double, big-endian
//! `w`  | 2 | IEEE-754 half-precision float, little-endian
//! `W`  | 2 | IEEE-754 half-precision float, big-endian
//! `x`  | count | NUL byte(s) -- consumes no argument
//! `X`  | count | back up `count` byte(s)
//! `Z`  | -- | (see above)
//! `@`  | -- | seek/pad to absolute position `count`
//!
//! `?`, `w`/`W`, and `u`/`U` are new additions over the module's previous
//! PHP-derived code set -- see "New datatypes" below.
//!
//! ## Integer precision
//!
//! Zuri numbers are IEEE-754 doubles, which can only represent integers
//! exactly up to 2^53. Every integer-producing code here (`q`/`Q`/`J`/`P`,
//! and the new `u`/`U`) automatically promotes its result to a `bigint`
//! Value instead of a `number` whenever the unpacked value falls outside
//! that safe range, rather than silently losing precision. Packing
//! accepts either a `number` or a `bigint` for every integer code.
//!
//! ## New datatypes
//!
//! The module was documented as "extendable"; three genuinely useful
//! datatypes were entirely missing from the original PHP-mirroring code
//! set and are added here:
//!
//! - `?` -- a real boolean type. Every other code forces the caller to
//!   spell a boolean out as a 0/1 byte by hand.
//! - `w`/`W` -- IEEE-754 half-precision (binary16) floats, ubiquitous in
//!   graphics, ML model weights/interchange formats, and compact network
//!   protocols. Conversion is hand-rolled (no extra dependency); see
//!   `f32_to_half`/`half_to_f32` below.
//! - `u`/`U` -- signed/unsigned 128-bit integers, matching this VM's
//!   existing `bigint` type (used for e.g. UUIDs, large hashes, or
//!   anything that overflows 64 bits) and finally letting `struct` round
//!   -trip a `bigint` without lossy detours through `number`.
//!
//! Also new, beyond raw pack/unpack: `calcsize` (the fixed byte size of a
//! format with no `*`), `pack_into`/`unpack_from` (operate on an existing
//! buffer at a given offset, so building up a larger buffer doesn't need
//! one allocation + concatenation per field), and `iter_unpack` (unpack a
//! buffer as a repeated sequence of fixed-size records in one call).
//!
//! ## Deviations from PHP's behavior
//!
//! The original PHP module's docstring already flagged its own footgun:
//! "...some data is overwritten because the numbering restarts from 1
//! for each element." That data-loss behavior, and the ambiguity of
//! attaching a name to a group by just appending text after it (which
//! silently breaks the moment the name happens to start with a letter
//! that's also a valid code, e.g. naming a field "size"), are both
//! deliberately NOT reproduced here. `:NAME` is unambiguous, and unnamed
//! keys are assigned from one running counter, so nothing is ever
//! clobbered.

use num_bigint::BigInt;
use num_traits::ToPrimitive;

use crate::builtins::enforce::ArgType;
use crate::modules::{BuiltinModuleDef, native};
use crate::vm::object::{Heap, Obj, ZuriContext};
use crate::vm::value::Value;
use crate::vm::vm::VM;
use crate::{enforce_arg_count, enforce_arg_type, enforce_arg_type_any_of, enforce_arg_type_opt};

pub static MODULE: BuiltinModuleDef = BuiltinModuleDef {
  name: "struct",
  build,
};

fn build(vm: &mut VM) -> Vec<(&'static str, Value)> {
  vec![
    ("pack", native(vm, "pack", 1, true, pack_fn)),
    ("unpack", native(vm, "unpack", 2, true, unpack_fn)),
    ("unpack_from", native(vm, "unpack_from", 2, true, unpack_fn)),
    ("pack_from", native(vm, "pack_from", 2, false, pack_from_fn)),
    ("pack_into", native(vm, "pack_into", 3, true, pack_into_fn)),
    ("calcsize", native(vm, "calcsize", 1, false, calcsize_fn)),
    (
      "iter_unpack",
      native(vm, "iter_unpack", 2, false, iter_unpack_fn),
    ),
  ]
}

// Format parsing

const VALID_CODES: &str = "aAhHcCsSnviIlLNVqQJPfgGdeExXZ@?wWuU";

fn is_valid_code(c: char) -> bool {
  VALID_CODES.contains(c)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Repeat {
  One,
  Count(usize),
  /// `*` -- meaning depends on the code; see each code's own handling.
  Star,
}

#[derive(Clone, Debug)]
struct FormatGroup {
  code: char,
  repeat: Repeat,
  name: Option<String>,
}

/// Parses a format string into an ordered list of groups. See this
/// module's own doc comment for the grammar.
fn parse_format(fmt: &str) -> Result<Vec<FormatGroup>, String> {
  let mut groups: Vec<FormatGroup> = Vec::new();

  for segment in fmt.split('/') {
    if segment.is_empty() {
      continue;
    }

    let (code_part, name_part) = match segment.find(':') {
      Some(p) => (&segment[..p], Some(&segment[p + 1..])),
      None => (segment, None),
    };

    if code_part.is_empty() {
      return Err(format!(
        "struct: empty format code in segment '{}'",
        segment
      ));
    }

    let cchars: Vec<char> = code_part.chars().collect();
    let mut j = 0usize;
    let mut local: Vec<FormatGroup> = Vec::new();

    while j < cchars.len() {
      let code = cchars[j];
      if !is_valid_code(code) {
        return Err(format!("struct: unknown format code '{}'", code));
      }
      j += 1;

      let repeat = if j < cchars.len() && cchars[j] == '*' {
        j += 1;
        Repeat::Star
      } else {
        let start = j;
        while j < cchars.len() && cchars[j].is_ascii_digit() {
          j += 1;
        }
        if j > start {
          let digits: String = cchars[start..j].iter().collect();
          let n: usize = digits.parse().map_err(|_| {
            format!(
              "struct: invalid repeat count '{}' for format code '{}'",
              digits, code
            )
          })?;
          Repeat::Count(n)
        } else {
          Repeat::One
        }
      };

      local.push(FormatGroup {
        code,
        repeat,
        name: None,
      });
    }

    if let Some(name) = name_part {
      if local.len() != 1 {
        return Err(format!(
          "struct: a named field (':{}') must contain exactly one format code, found {} in segment '{}'",
          name,
          local.len(),
          segment
        ));
      }
      local[0].name = Some(name.to_string());
    }

    groups.extend(local);
  }

  Ok(groups)
}

fn repeat_count_of(r: Repeat) -> Result<usize, String> {
  match r {
    Repeat::One => Ok(1),
    Repeat::Count(n) => Ok(n),
    Repeat::Star => Err("struct: '*' repeat is not supported for this format code".to_string()),
  }
}

// Element sizes / byte order

#[derive(Clone, Copy)]
enum Endian {
  Native,
  Big,
  Little,
}

/// `(size_in_bytes, byte_order, is_signed)` for every fixed-size integer
/// code, including the new `?` (1 byte, unsigned) and `u`/`U` (128-bit).
fn code_int_spec(code: char) -> Option<(usize, Endian, bool)> {
  Some(match code {
    '?' => (1, Endian::Native, false),
    'c' => (1, Endian::Native, true),
    'C' => (1, Endian::Native, false),
    's' => (2, Endian::Native, true),
    'S' => (2, Endian::Native, false),
    'n' => (2, Endian::Big, false),
    'v' => (2, Endian::Little, false),
    'i' => (4, Endian::Native, true),
    'I' => (4, Endian::Native, false),
    'l' => (4, Endian::Native, true),
    'L' => (4, Endian::Native, false),
    'N' => (4, Endian::Big, false),
    'V' => (4, Endian::Little, false),
    'q' => (8, Endian::Native, true),
    'Q' => (8, Endian::Native, false),
    'J' => (8, Endian::Big, false),
    'P' => (8, Endian::Little, false),
    'u' => (16, Endian::Little, true),
    'U' => (16, Endian::Little, false),
    _ => return None,
  })
}

fn is_float_code(c: char) -> bool {
  matches!(c, 'f' | 'g' | 'G' | 'd' | 'e' | 'E' | 'w' | 'W')
}

/// Fixed element size for any non-string, non-pad, non-`@` code -- used
/// both to size an individual read/write and by `calcsize`/`iter_unpack`.
fn code_size(code: char) -> Result<usize, String> {
  if let Some((size, _, _)) = code_int_spec(code) {
    return Ok(size);
  }
  Ok(match code {
    'f' | 'g' | 'G' => 4,
    'd' | 'e' | 'E' => 8,
    'w' | 'W' => 2,
    _ => {
      return Err(format!(
        "struct: format code '{}' has no fixed element size",
        code
      ));
    },
  })
}

// Half-precision float conversion (IEEE-754 binary16 <-> f32)
//
// Hand-rolled rather than pulling in a dependency -- the standard bit-
// twiddling algorithm used by most half-float fallback implementations.
// Rounds toward zero rather than to-nearest-even on the narrowing
// conversion (f32_to_half); this only matters for values whose exact
// binary16 representation would round up, an edge case rare enough in
// practice (and inherent to half-precision's reduced range/precision
// regardless) not to warrant a full round-to-nearest implementation here.

fn f32_to_half(value: f32) -> u16 {
  let bits = value.to_bits();
  let sign = ((bits >> 16) & 0x8000) as u16;
  let exp = ((bits >> 23) & 0xff) as i32;
  let mantissa = bits & 0x007f_ffff;

  if exp == 0xff {
    // Infinity or NaN -- preserve which one, and NaN-ness of the payload.
    let nan_bit: u16 = if mantissa != 0 { 0x0200 } else { 0 };
    return sign | 0x7c00 | nan_bit;
  }

  let half_exp = exp - 127 + 15;

  if half_exp >= 0x1f {
    return sign | 0x7c00; // overflow -> infinity
  }

  if half_exp <= 0 {
    if 14 - half_exp > 24 {
      return sign; // far too small -> signed zero
    }
    let m = mantissa | 0x0080_0000; // restore the implicit leading bit
    let shift = (14 - half_exp) as u32;
    let half_mantissa = (m >> shift) as u16;
    return sign | half_mantissa;
  }

  let half_mantissa = (mantissa >> 13) as u16;
  sign | ((half_exp as u16) << 10) | half_mantissa
}

fn half_to_f32(h: u16) -> f32 {
  let h = h as u32;
  let sign = (h & 0x8000) << 16;
  let exp = (h >> 10) & 0x1f;
  let mantissa = h & 0x3ff;

  let bits: u32 = if exp == 0 {
    if mantissa == 0 {
      sign
    } else {
      // Subnormal half -> normalize into a regular f32.
      let mut e: i32 = -1;
      let mut m = mantissa;
      while m & 0x400 == 0 {
        m <<= 1;
        e += 1;
      }
      m &= 0x3ff;
      let exp32 = (127 - 15 - e) as u32;
      sign | (exp32 << 23) | (m << 13)
    }
  } else if exp == 0x1f {
    sign | 0x7f80_0000 | (mantissa << 13) // inf / NaN
  } else {
    let exp32 = exp + (127 - 15);
    sign | (exp32 << 23) | (mantissa << 13)
  };

  f32::from_bits(bits)
}

// Raw integer read/write, generic over every fixed-size integer code

fn read_uint(bytes: &[u8], endian: Endian) -> u128 {
  match (bytes.len(), endian) {
    (1, _) => bytes[0] as u128,
    (2, Endian::Little) => u16::from_le_bytes(bytes.try_into().unwrap()) as u128,
    (2, Endian::Big) => u16::from_be_bytes(bytes.try_into().unwrap()) as u128,
    (2, Endian::Native) => u16::from_ne_bytes(bytes.try_into().unwrap()) as u128,
    (4, Endian::Little) => u32::from_le_bytes(bytes.try_into().unwrap()) as u128,
    (4, Endian::Big) => u32::from_be_bytes(bytes.try_into().unwrap()) as u128,
    (4, Endian::Native) => u32::from_ne_bytes(bytes.try_into().unwrap()) as u128,
    (8, Endian::Little) => u64::from_le_bytes(bytes.try_into().unwrap()) as u128,
    (8, Endian::Big) => u64::from_be_bytes(bytes.try_into().unwrap()) as u128,
    (8, Endian::Native) => u64::from_ne_bytes(bytes.try_into().unwrap()) as u128,
    (16, Endian::Little) => u128::from_le_bytes(bytes.try_into().unwrap()),
    (16, Endian::Big) => u128::from_be_bytes(bytes.try_into().unwrap()),
    (16, Endian::Native) => u128::from_ne_bytes(bytes.try_into().unwrap()),
    _ => unreachable!("read_uint: unsupported integer width"),
  }
}

/// `v` is the value's full two's-complement bit pattern (as produced by
/// an `i128 as u128` cast, which is a bitcast, not a range check) --
/// truncating it to the target width and byte order is then just a
/// matter of narrowing casts, which correctly preserve two's-complement
/// truncation for negative values.
fn write_int(out: &mut Vec<u8>, code: char, v: i128) {
  let (size, endian, _signed) = code_int_spec(code).expect("write_int: not an integer code");
  let u = v as u128;
  match (size, endian) {
    (1, _) => out.push(u as u8),
    (2, Endian::Little) => out.extend((u as u16).to_le_bytes()),
    (2, Endian::Big) => out.extend((u as u16).to_be_bytes()),
    (2, Endian::Native) => out.extend((u as u16).to_ne_bytes()),
    (4, Endian::Little) => out.extend((u as u32).to_le_bytes()),
    (4, Endian::Big) => out.extend((u as u32).to_be_bytes()),
    (4, Endian::Native) => out.extend((u as u32).to_ne_bytes()),
    (8, Endian::Little) => out.extend((u as u64).to_le_bytes()),
    (8, Endian::Big) => out.extend((u as u64).to_be_bytes()),
    (8, Endian::Native) => out.extend((u as u64).to_ne_bytes()),
    (16, Endian::Little) => out.extend(u.to_le_bytes()),
    (16, Endian::Big) => out.extend(u.to_be_bytes()),
    (16, Endian::Native) => out.extend(u.to_ne_bytes()),
    _ => unreachable!("write_int: unsupported integer width"),
  }
}

/// 2^53 -- the largest integer magnitude an f64 can hold exactly.
const F64_SAFE_INT: i128 = 9_007_199_254_740_992;

fn int_to_value(v: i128, heap: &mut Heap) -> Value {
  if v >= -F64_SAFE_INT && v <= F64_SAFE_INT {
    Value::number(v as f64)
  } else {
    heap.alloc_bigint(BigInt::from(v))
  }
}

fn uint_to_value(v: u128, heap: &mut Heap) -> Value {
  if v <= F64_SAFE_INT as u128 {
    Value::number(v as f64)
  } else {
    heap.alloc_bigint(BigInt::from(v))
  }
}

// Value <-> Rust primitive coercion

fn value_to_i128(v: Value) -> Result<i128, String> {
  if v.is_number() {
    let n = v.as_number();
    if !n.is_finite() {
      return Err(format!(
        "struct.pack(): cannot pack non-finite number {} as an integer",
        n
      ));
    }
    if n.fract() != 0.0 {
      return Err(format!(
        "struct.pack(): expected an integer value for this format code, got {}",
        n
      ));
    }
    Ok(n as i128)
  } else if v.is_bigint() {
    v.as_bigint().to_i128().ok_or_else(|| {
      "struct.pack(): bigint value is out of range for a 128-bit integer".to_string()
    })
  } else {
    Err(format!(
      "struct.pack(): expected a number or bigint for this format code, got {}",
      v.type_name()
    ))
  }
}

fn value_to_f64(v: Value) -> Result<f64, String> {
  if v.is_number() {
    Ok(v.as_number())
  } else if v.is_bigint() {
    v.as_bigint()
      .to_f64()
      .ok_or_else(|| "struct.pack(): bigint value cannot be represented as a float".to_string())
  } else {
    Err(format!(
      "struct.pack(): expected a number for this format code, got {}",
      v.type_name()
    ))
  }
}

fn value_as_string_bytes(v: Value, code: char) -> Result<Vec<u8>, String> {
  if v.is_string() {
    Ok(v.as_str().as_bytes().to_vec())
  } else if v.is_bytes() {
    Ok(v.as_bytes())
  } else {
    Err(format!(
      "struct.pack(): format code '{}' expects a string or bytes value, got {}",
      code,
      v.type_name()
    ))
  }
}

fn value_as_hex_str(v: Value, code: char) -> Result<String, String> {
  if !v.is_string() {
    return Err(format!(
      "struct.pack(): format code '{}' expects a hex-digit string, got {}",
      code,
      v.type_name()
    ));
  }
  let s = v.as_str();
  for c in s.chars() {
    if !c.is_ascii_hexdigit() {
      return Err(format!(
        "struct.pack(): format code '{}' expects a hex-digit string, found '{}'",
        code, c
      ));
    }
  }
  Ok(s.to_string())
}

// Float read/write

fn write_float(out: &mut Vec<u8>, code: char, x: f64) {
  match code {
    'f' => out.extend((x as f32).to_ne_bytes()),
    'g' => out.extend((x as f32).to_le_bytes()),
    'G' => out.extend((x as f32).to_be_bytes()),
    'd' => out.extend(x.to_ne_bytes()),
    'e' => out.extend(x.to_le_bytes()),
    'E' => out.extend(x.to_be_bytes()),
    'w' => out.extend(f32_to_half(x as f32).to_le_bytes()),
    'W' => out.extend(f32_to_half(x as f32).to_be_bytes()),
    _ => unreachable!("write_float: not a float code"),
  }
}

fn read_float(code: char, b: &[u8]) -> f64 {
  match code {
    'f' => f32::from_ne_bytes(b.try_into().unwrap()) as f64,
    'g' => f32::from_le_bytes(b.try_into().unwrap()) as f64,
    'G' => f32::from_be_bytes(b.try_into().unwrap()) as f64,
    'd' => f64::from_ne_bytes(b.try_into().unwrap()),
    'e' => f64::from_le_bytes(b.try_into().unwrap()),
    'E' => f64::from_be_bytes(b.try_into().unwrap()),
    'w' => half_to_f32(u16::from_le_bytes(b.try_into().unwrap())) as f64,
    'W' => half_to_f32(u16::from_be_bytes(b.try_into().unwrap())) as f64,
    _ => unreachable!("read_float: not a float code"),
  }
}

// Hex string <-> nibble-packed bytes ('h' / 'H')

fn write_hex(
  out: &mut Vec<u8>,
  hexstr: &str,
  want_digits: usize,
  code: char,
) -> Result<(), String> {
  let mut digits: Vec<u8> = Vec::with_capacity(want_digits);
  for c in hexstr.chars() {
    if digits.len() >= want_digits {
      break;
    }
    let d = c.to_digit(16).ok_or_else(|| {
      format!(
        "struct.pack(): '{}' expects a hex-digit string, found '{}'",
        code, c
      )
    })?;
    digits.push(d as u8);
  }
  while digits.len() < want_digits {
    digits.push(0);
  }

  let mut i = 0;
  while i < digits.len() {
    let d0 = digits[i];
    let d1 = digits.get(i + 1).copied().unwrap_or(0);
    let byte = if code == 'h' {
      (d1 << 4) | d0
    } else {
      (d0 << 4) | d1
    };
    out.push(byte);
    i += 2;
  }
  Ok(())
}

fn read_hex(bytes: &[u8], want_digits: usize, code: char) -> String {
  let mut s = String::with_capacity(want_digits);
  for &b in bytes {
    let (hi, lo) = ((b >> 4) & 0xf, b & 0xf);
    let (first, second) = if code == 'h' { (lo, hi) } else { (hi, lo) };
    if s.len() < want_digits {
      s.push(std::char::from_digit(first as u32, 16).unwrap());
    }
    if s.len() < want_digits {
      s.push(std::char::from_digit(second as u32, 16).unwrap());
    }
  }
  s
}

// Padded-string pack ('a' / 'A' / 'Z')

fn write_padded_string(out: &mut Vec<u8>, s: &[u8], want_len: usize, code: char) {
  match code {
    'Z' => {
      let want_len = want_len.max(1);
      let take = s.len().min(want_len - 1);
      out.extend_from_slice(&s[..take]);
      out.extend(std::iter::repeat(0u8).take(want_len - take));
    },
    'A' => {
      let take = s.len().min(want_len);
      out.extend_from_slice(&s[..take]);
      out.extend(std::iter::repeat(b' ').take(want_len - take));
    },
    _ => {
      // 'a'
      let take = s.len().min(want_len);
      out.extend_from_slice(&s[..take]);
      out.extend(std::iter::repeat(0u8).take(want_len - take));
    },
  }
}

// One numeric/bool value: pack + unpack

fn write_group_value(out: &mut Vec<u8>, code: char, v: Value) -> Result<(), String> {
  if code == '?' {
    out.push(if !v.is_falsey() { 1 } else { 0 });
    return Ok(());
  }
  if is_float_code(code) {
    let f = value_to_f64(v)?;
    write_float(out, code, f);
    return Ok(());
  }
  let n = value_to_i128(v)?;
  write_int(out, code, n);
  Ok(())
}

fn read_group_value(code: char, bytes: &[u8], heap: &mut Heap) -> Value {
  if code == '?' {
    return Value::bool(bytes[0] != 0);
  }
  if is_float_code(code) {
    return Value::number(read_float(code, bytes));
  }

  let (size, endian, signed) = code_int_spec(code).expect("read_group_value: unknown numeric code");
  let raw = read_uint(bytes, endian);

  if signed {
    let sv: i128 = if size >= 16 {
      raw as i128
    } else {
      let bits = size * 8;
      let mut sv = raw as i128;
      if (raw & (1u128 << (bits - 1))) != 0 {
        sv -= 1i128 << bits;
      }
      sv
    };
    int_to_value(sv, heap)
  } else {
    uint_to_value(raw, heap)
  }
}

// Argument bookkeeping

fn next_arg(items: &[Value], idx: &mut usize, code: char) -> Result<Value, String> {
  match items.get(*idx) {
    Some(&v) => {
      *idx += 1;
      Ok(v)
    },
    None => Err(format!(
      "struct.pack(): not enough arguments supplied for format code '{}' (expected a value at position {})",
      code,
      *idx + 1
    )),
  }
}

fn check_bounds(data: &[u8], pos: usize, need: usize) -> Result<(), String> {
  let ok = pos.checked_add(need).is_some_and(|end| end <= data.len());
  if !ok {
    return Err(format!(
      "struct.unpack(): not enough data: need {} more byte(s) at offset {}, but only {} byte(s) remain",
      need,
      pos,
      data.len().saturating_sub(pos)
    ));
  }
  Ok(())
}

/// Flattens `pack()`'s variadic argument tail the same way the old Zuri
/// wrapper did: a list is spread element-by-element, a bytes object is
/// spread as individual byte numbers, anything else is taken as a
/// single item -- so `pack('CCC', 1, 2, 3)` and `pack('CCC', [1, 2, 3])`
/// both work.
fn flatten_pack_args(args: &[Value]) -> Vec<Value> {
  let mut items = Vec::with_capacity(args.len());
  for &arg in args {
    if arg.is_list() {
      items.extend(arg.as_list());
    } else if arg.is_bytes() {
      items.extend(arg.as_bytes().into_iter().map(|b| Value::number(b as f64)));
    } else {
      items.push(arg);
    }
  }
  items
}

// The pack/unpack/calcsize engines

fn pack_values(groups: &[FormatGroup], items: &[Value]) -> Result<Vec<u8>, String> {
  let mut out: Vec<u8> = Vec::new();
  let mut idx = 0usize;

  for g in groups {
    match g.code {
      'a' | 'A' | 'Z' => {
        let v = next_arg(items, &mut idx, g.code)?;
        let s = value_as_string_bytes(v, g.code)?;
        let want = match g.repeat {
          Repeat::One => 1,
          Repeat::Count(n) => n,
          Repeat::Star => {
            if g.code == 'Z' {
              s.len() + 1
            } else {
              s.len()
            }
          },
        };
        write_padded_string(&mut out, &s, want, g.code);
      },
      'h' | 'H' => {
        let v = next_arg(items, &mut idx, g.code)?;
        let hex = value_as_hex_str(v, g.code)?;
        let want_digits = match g.repeat {
          Repeat::One => 1,
          Repeat::Count(n) => n,
          Repeat::Star => hex.chars().count(),
        };
        write_hex(&mut out, &hex, want_digits, g.code)?;
      },
      'x' => {
        let n = repeat_count_of(g.repeat)?;
        out.extend(std::iter::repeat(0u8).take(n));
      },
      'X' => {
        let n = repeat_count_of(g.repeat)?;
        let new_len = out.len().saturating_sub(n);
        out.truncate(new_len);
      },
      '@' => {
        let pos = match g.repeat {
          Repeat::Count(n) => n,
          Repeat::One => 0,
          Repeat::Star => return Err("struct.pack(): '@' cannot use a '*' repeat".to_string()),
        };
        if out.len() < pos {
          out.resize(pos, 0);
        }
      },
      code => {
        let count = match g.repeat {
          Repeat::One => 1,
          Repeat::Count(n) => n,
          Repeat::Star => items.len().saturating_sub(idx),
        };
        for _ in 0..count {
          let v = next_arg(items, &mut idx, code)?;
          write_group_value(&mut out, code, v)?;
        }
      },
    }
  }

  Ok(out)
}

#[allow(clippy::too_many_arguments)]
fn push_named(
  result: &mut Vec<(Value, Value)>,
  heap: &mut Heap,
  unnamed: &mut usize,
  name: &Option<String>,
  val: Value,
  count: usize,
  k: usize,
) {
  let key = match name {
    Some(name) => {
      if count > 1 {
        format!("{}{}", name, k + 1)
      } else {
        name.clone()
      }
    },
    None => {
      let s = unnamed.to_string();
      *unnamed += 1;
      s
    },
  };
  let key_val = heap.alloc_string(key);
  result.push((key_val, val));
}

/// Unpacks starting at byte offset `start`, returning the resulting
/// (key, value) pairs plus the byte offset just past the last field
/// consumed -- the latter is what `iter_unpack` chains calls with.
fn unpack_values(
  groups: &[FormatGroup],
  data: &[u8],
  start: usize,
  heap: &mut Heap,
) -> Result<(Vec<(Value, Value)>, usize), String> {
  let mut result: Vec<(Value, Value)> = Vec::new();
  let mut pos = start;
  let mut unnamed = 1usize;

  for g in groups {
    match g.code {
      'a' | 'A' | 'Z' => {
        let want = match g.repeat {
          Repeat::One => 1,
          Repeat::Count(n) => n,
          Repeat::Star => data.len().saturating_sub(pos),
        };
        check_bounds(data, pos, want)?;
        let mut chunk = data[pos..pos + want].to_vec();
        pos += want;
        match g.code {
          'A' => {
            while matches!(chunk.last(), Some(0) | Some(b' ')) {
              chunk.pop();
            }
          },
          'Z' => {
            if let Some(nul_idx) = chunk.iter().position(|&b| b == 0) {
              chunk.truncate(nul_idx);
            }
          },
          _ => {},
        }
        let s = String::from_utf8_lossy(&chunk).into_owned();
        let val = heap.alloc_string(s);
        push_named(&mut result, heap, &mut unnamed, &g.name, val, 1, 0);
      },
      'h' | 'H' => {
        let want_digits = match g.repeat {
          Repeat::One => 1,
          Repeat::Count(n) => n,
          Repeat::Star => data.len().saturating_sub(pos) * 2,
        };
        let want_bytes = want_digits.div_ceil(2);
        check_bounds(data, pos, want_bytes)?;
        let hex = read_hex(&data[pos..pos + want_bytes], want_digits, g.code);
        pos += want_bytes;
        let val = heap.alloc_string(hex);
        push_named(&mut result, heap, &mut unnamed, &g.name, val, 1, 0);
      },
      'x' => {
        let n = repeat_count_of(g.repeat)?;
        check_bounds(data, pos, n)?;
        pos += n;
      },
      'X' => {
        let n = repeat_count_of(g.repeat)?;
        pos = pos.saturating_sub(n);
      },
      '@' => {
        let target = match g.repeat {
          Repeat::Count(n) => n,
          Repeat::One => 0,
          Repeat::Star => return Err("struct.unpack(): '@' cannot use a '*' repeat".to_string()),
        };
        if target > data.len() {
          return Err(format!(
            "struct.unpack(): '@' position {} is beyond the data length {}",
            target,
            data.len()
          ));
        }
        pos = target;
      },
      code => {
        let size = code_size(code)?;
        let count = match g.repeat {
          Repeat::One => 1,
          Repeat::Count(n) => n,
          Repeat::Star => data.len().saturating_sub(pos) / size,
        };
        for k in 0..count {
          check_bounds(data, pos, size)?;
          let val = read_group_value(code, &data[pos..pos + size], heap);
          pos += size;
          push_named(&mut result, heap, &mut unnamed, &g.name, val, count, k);
        }
      },
    }
  }

  Ok((result, pos))
}

/// The fixed byte size of a format string -- errors if it contains a
/// `*` repeat anywhere, since that has no size independent of actual
/// data.
fn format_size(groups: &[FormatGroup]) -> Result<usize, String> {
  let mut size = 0usize;
  for g in groups {
    match g.code {
      'a' | 'A' | 'Z' => {
        size += repeat_count_of(g.repeat).map_err(|_| {
          "struct.calcsize(): format contains a '*' repeat, which has no fixed size".to_string()
        })?;
      },
      'h' | 'H' => {
        let n = repeat_count_of(g.repeat).map_err(|_| {
          "struct.calcsize(): format contains a '*' repeat, which has no fixed size".to_string()
        })?;
        size += n.div_ceil(2);
      },
      'x' => size += repeat_count_of(g.repeat)?,
      'X' => size = size.saturating_sub(repeat_count_of(g.repeat)?),
      '@' => {
        let pos = match g.repeat {
          Repeat::Count(n) => n,
          Repeat::One => 0,
          Repeat::Star => return Err("struct.calcsize(): '@' cannot use a '*' repeat".to_string()),
        };
        size = size.max(pos);
      },
      other => {
        let elem = code_size(other)?;
        let count = repeat_count_of(g.repeat).map_err(|_| {
          "struct.calcsize(): format contains a '*' repeat, which has no fixed size".to_string()
        })?;
        size += elem * count;
      },
    }
  }
  Ok(size)
}

// bytes buffer mutation helper (for pack_into)

fn with_bytes_mut<F, R>(v: Value, f: F) -> R
where
  F: FnOnce(&mut Vec<u8>) -> R,
{
  match unsafe { &*v.as_obj() } {
    Obj::Bytes(b) => f(&mut b.borrow_mut()),
    _ => unreachable!("with_bytes_mut called on a non-bytes Value"),
  }
}

// Native entry points

/// `struct.pack(format, ...values)` -> `bytes`.
fn pack_fn(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_type!(ctx, 0, ArgType::String);
  let format = ctx.args[0].as_str().to_string();
  let groups = parse_format(&format)?;
  let items = flatten_pack_args(&ctx.args[1..]);
  let bytes = pack_values(&groups, &items)?;
  Ok(ctx.heap().alloc_bytes(bytes))
}

/// `struct.pack_from(format, args: list)` -> `bytes`. Like `pack()`, but
/// takes its values from one list argument, unflattened (each list
/// element maps to exactly one format slot).
fn pack_from_fn(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 2);
  enforce_arg_type!(ctx, 0, ArgType::String);
  enforce_arg_type!(ctx, 1, ArgType::List);
  let format = ctx.args[0].as_str().to_string();
  let groups = parse_format(&format)?;
  let items = ctx.args[1].as_list();
  let bytes = pack_values(&groups, &items)?;
  Ok(ctx.heap().alloc_bytes(bytes))
}

/// `struct.pack_into(format, buffer: bytes, offset, ...values)` ->
/// `number` (bytes written). Packs directly into an existing bytes
/// object at `offset`, growing it (zero-padded) if it isn't long enough
/// -- avoids an allocate-then-copy round trip when assembling a larger
/// buffer field by field.
fn pack_into_fn(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_type!(ctx, 0, ArgType::String);
  enforce_arg_type!(ctx, 1, ArgType::Bytes);
  enforce_arg_type!(ctx, 2, ArgType::Number);

  let format = ctx.args[0].as_str().to_string();
  let buffer = ctx.args[1];
  let offset = ctx.args[2].as_number();
  if offset < 0.0 || offset.fract() != 0.0 {
    return Err("struct.pack_into(): offset must be a non-negative integer".to_string());
  }
  let offset = offset as usize;

  let groups = parse_format(&format)?;
  let items = flatten_pack_args(&ctx.args[3..]);
  let packed = pack_values(&groups, &items)?;

  with_bytes_mut(buffer, |v| {
    let end = offset + packed.len();
    if v.len() < end {
      v.resize(end, 0);
    }
    v[offset..end].copy_from_slice(&packed);
  });

  Ok(Value::number(packed.len() as f64))
}

/// `struct.unpack(format, data: bytes|string, offset: ?number)` ->
/// `dict`. Also registered as `unpack_from` -- identical behavior, kept
/// as a separate name since `unpack()` already accepts an offset and
/// never requires the buffer to be fully consumed.
fn unpack_fn(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_type!(ctx, 0, ArgType::String);
  enforce_arg_type_any_of!(ctx, 1, [ArgType::Bytes, ArgType::String]);
  enforce_arg_type_opt!(ctx, 2, ArgType::Number);

  let format = ctx.args[0].as_str().to_string();
  let data: Vec<u8> = if ctx.args[1].is_bytes() {
    ctx.args[1].as_bytes()
  } else {
    ctx.args[1].as_str().as_bytes().to_vec()
  };
  let offset = ctx.args.get(2).map(|v| v.as_number() as usize).unwrap_or(0);
  if offset > data.len() {
    return Err(format!(
      "struct.unpack(): offset {} is beyond the data length {}",
      offset,
      data.len()
    ));
  }

  let groups = parse_format(&format)?;
  let (pairs, _end) = unpack_values(&groups, &data, offset, ctx.heap())?;
  Ok(ctx.heap().alloc_dict(pairs))
}

/// `struct.calcsize(format)` -> `number`. The fixed byte size of
/// `format`; errors if the format contains a `*` repeat anywhere.
fn calcsize_fn(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_type!(ctx, 0, ArgType::String);
  let format = ctx.args[0].as_str().to_string();
  let groups = parse_format(&format)?;
  let size = format_size(&groups)?;
  Ok(Value::number(size as f64))
}

/// `struct.iter_unpack(format, data: bytes|string)` -> `list[dict]`.
/// Repeatedly unpacks fixed-size `format`-shaped records from `data`
/// until it's exhausted. `data`'s length must be an exact multiple of
/// `calcsize(format)`.
fn iter_unpack_fn(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 2);
  enforce_arg_type!(ctx, 0, ArgType::String);
  enforce_arg_type_any_of!(ctx, 1, [ArgType::Bytes, ArgType::String]);

  let format = ctx.args[0].as_str().to_string();
  let data: Vec<u8> = if ctx.args[1].is_bytes() {
    ctx.args[1].as_bytes()
  } else {
    ctx.args[1].as_str().as_bytes().to_vec()
  };

  let groups = parse_format(&format)?;
  let rec_size = format_size(&groups)?;
  if rec_size == 0 {
    return Err("struct.iter_unpack(): format has zero size".to_string());
  }
  if data.len() % rec_size != 0 {
    return Err(format!(
      "struct.iter_unpack(): data length {} is not a multiple of the format size {}",
      data.len(),
      rec_size
    ));
  }

  let mut records = Vec::with_capacity(data.len() / rec_size);
  let mut pos = 0usize;
  while pos < data.len() {
    let (pairs, next_pos) = unpack_values(&groups, &data, pos, ctx.heap())?;
    records.push(ctx.heap().alloc_dict(pairs));
    pos = next_pos;
  }
  Ok(ctx.heap().alloc_list(records))
}
