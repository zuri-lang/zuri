//! NaN boxing.
//!
//! An IEEE-754 double has this layout:
//!
//!   sign(1) exponent(11) mantissa(52)
//!
//! Every bit pattern where the exponent is all-ones (0x7FF) and the
//! mantissa is non-zero is a NaN. Real arithmetic never *needs* to produce
//! more than one specific NaN bit pattern, so we can steal the rest of that
//! space (there are 2^52 - 2 of them, minus the ones IEEE reserves for
//! infinities) to encode every other kind of value: nil, true, false, and
//! pointers to heap objects (strings, functions, ...).
//!unsafe
//! This is the same trick used by JavaScriptCore, SpiderMonkey, LuaJIT and
//! the clox VM from "Crafting Interpreters" -- here it's adapted to a
//! register machine and extended with a Str/Function object model.
//!
//! Layout used here:
//!
//!   0111111111111100000000000000000000000000000000000000000000000000  <- QNAN (quiet NaN, "impossible" for real numbers we produce)
//!   plus the sign bit as an extra tag bit:
//!
//!   QNAN unset, sign unset  -> ordinary f64
//!   QNAN set,   sign unset  -> tagged singleton (nil / true / false), tag in low 2 bits
//!   QNAN set,   sign set    -> pointer to a heap-allocated Obj, stored in the low 48 bits
//!
//! 48 bits is enough to hold every real pointer on x86_64 / AArch64 today
//! (both use at most 48 address bits), so no pointer information is lost.

use std::cell::Cell;

use itertools::Itertools;
use num_bigint::BigInt;

use crate::vm::object::{NativeFunction, Obj, ObjClosure, ObjFunction, UpvalueState};

const QNAN: u64 = 0x7ffc_0000_0000_0000; // exponent all 1s + top mantissa bit set: guaranteed non-NaN-we-produce
const SIGN_BIT: u64 = 0x8000_0000_0000_0000;

const TAG_NIL: u64 = 0b01;
const TAG_FALSE: u64 = 0b10;
const TAG_TRUE: u64 = 0b11;

// set only for boxed integers; unset for nil/true/false and for pointers
// const TAG_INT: u64 = 1 << 49;

const NIL_VAL: u64 = QNAN | TAG_NIL;
const FALSE_VAL: u64 = QNAN | TAG_FALSE;
const TRUE_VAL: u64 = QNAN | TAG_TRUE;

/// Mask covering exactly the low 48 bits, where we stash a pointer.
const PTR_MASK: u64 = 0x0000_ffff_ffff_ffff;

#[derive(Clone, Copy, Debug)]
#[repr(transparent)]
pub struct Value(u64);

impl Value {
  #[inline]
  pub fn nil() -> Value {
    Value(NIL_VAL)
  }

  #[inline]
  pub fn bool(b: bool) -> Value {
    Value(if b { TRUE_VAL } else { FALSE_VAL })
  }

  // #[inline]
  // pub fn integer(n: i64) -> Value {
  //   debug_assert!(
  //     n >= -(1i64 << 47) && n < (1i64 << 47),
  //     "integer out of range for a 48-bit packed Value"
  //   );
  //   Value(QNAN | TAG_INT | ((n as u64) & PTR_MASK))
  // }

  #[inline]
  pub fn number(n: f64) -> Value {
    // A computation could in principle produce an actual NaN (0.0 / 0.0).
    // Canonicalize it to a single fixed NaN pattern that does NOT collide
    // with QNAN (we use quiet-NaN-without-our-extra-bit), so it can never
    // be mistaken for one of our tagged values.
    if n.is_nan() {
      Value(0x7ff8_0000_0000_0000)
    } else {
      Value(n.to_bits())
    }
  }

  /// Wrap a raw pointer to a heap object as a Value. Safety: the pointer
  /// must stay valid for as long as this Value (and any copy of it) is
  /// reachable -- see `Heap` in object.rs, which owns every object for the
  /// lifetime of the VM.
  #[inline]
  pub fn obj(ptr: *const Obj) -> Value {
    let bits = ptr as u64;
    debug_assert_eq!(bits & !PTR_MASK, 0, "pointer does not fit in 48 bits");
    Value(SIGN_BIT | QNAN | bits)
  }

  // #[inline]
  // pub fn is_int(&self) -> bool {
  //   (self.0 & (QNAN | SIGN_BIT | TAG_INT)) == (QNAN | TAG_INT)
  // }

  #[inline]
  pub fn is_number(&self) -> bool {
    (self.0 & QNAN) != QNAN
  }

  #[inline]
  pub fn is_nil(&self) -> bool {
    self.0 == NIL_VAL
  }

  #[inline]
  pub fn is_bool(&self) -> bool {
    self.0 == TRUE_VAL || self.0 == FALSE_VAL
  }

  #[inline]
  pub fn is_obj(&self) -> bool {
    (self.0 & (QNAN | SIGN_BIT)) == (QNAN | SIGN_BIT)
  }

  #[inline]
  pub fn is_string(&self) -> bool {
    self.is_obj() && matches!(unsafe { &*self.as_obj() }, Obj::Str(_))
  }

  #[inline]
  pub fn is_func(&self) -> bool {
    self.is_obj() && matches!(unsafe { &*self.as_obj() }, Obj::Func(_))
  }

  #[inline]
  pub fn is_closure(&self) -> bool {
    self.is_obj() && matches!(unsafe { &*self.as_obj() }, Obj::Closure(_))
  }

  #[inline]
  pub fn is_list(&self) -> bool {
    self.is_obj() && matches!(unsafe { &*self.as_obj() }, Obj::List(_))
  }

  #[inline]
  pub fn is_dict(&self) -> bool {
    self.is_obj() && matches!(unsafe { &*self.as_obj() }, Obj::Dict(_))
  }

  #[inline]
  pub fn is_upvalue(&self) -> bool {
    self.is_obj() && matches!(unsafe { &*self.as_obj() }, Obj::Upvalue(_))
  }

  #[inline]
  pub fn is_bytes(&self) -> bool {
    self.is_obj() && matches!(unsafe { &*self.as_obj() }, Obj::Bytes(_))
  }

  #[inline]
  pub fn is_bigint(&self) -> bool {
    self.is_obj() && matches!(unsafe { &*self.as_obj() }, Obj::BigInt(_))
  }

  #[inline]
  pub fn is_native(&self) -> bool {
    self.is_obj() && matches!(unsafe { &*self.as_obj() }, Obj::Native(_))
  }

  /// Is this something Instr::Call can invoke -- closure OR native. Same
  /// concept to a Zuri program; different call paths internally.
  pub fn is_callable(&self) -> bool {
    self.is_closure() || self.is_native()
  }

  // #[inline]
  // pub fn as_int(&self) -> i64 {
  //   debug_assert!(self.is_int());
  //   let raw = (self.0 & PTR_MASK) as i64;

  //   // sign-extend bit 47 out to a full i64 — same trick x86-64 uses for 48-bit canonical addresses
  //   (raw << 16) >> 16
  // }

  #[inline]
  pub fn as_number(&self) -> f64 {
    debug_assert!(self.is_number());
    f64::from_bits(self.0)
  }

  #[inline]
  pub fn as_bool(&self) -> bool {
    debug_assert!(self.is_bool());
    self.0 == TRUE_VAL
  }

  #[inline]
  pub fn as_obj(&self) -> *const Obj {
    debug_assert!(self.is_obj());
    (self.0 & PTR_MASK) as *const Obj
  }

  pub fn as_str(&self) -> &str {
    debug_assert!(self.is_string());
    match unsafe { &*self.as_obj() } {
      Obj::Str(s) => s.as_str(),
      _ => unreachable!("as_str() called on a non-string Value"),
    }
  }

  pub fn as_func(&self) -> &ObjFunction {
    debug_assert!(self.is_func());
    match unsafe { &*self.as_obj() } {
      Obj::Func(f) => f,
      _ => unreachable!("as_func() called on a non-function Value"),
    }
  }

  pub fn as_closure(&self) -> &ObjClosure {
    debug_assert!(self.is_closure());
    match unsafe { &*self.as_obj() } {
      Obj::Closure(c) => c,
      _ => unreachable!("as_closure() called on a non-closure Value"),
    }
  }

  pub fn as_native(&self) -> &NativeFunction {
    debug_assert!(self.is_native());
    match unsafe { &*self.as_obj() } {
      Obj::Native(c) => c,
      _ => unreachable!("as_native() called on a non-native Value"),
    }
  }

  pub fn as_bytes(&self) -> &[u8] {
    debug_assert!(self.is_bytes());
    match unsafe { &*self.as_obj() } {
      Obj::Bytes(b) => b.as_slice(),
      _ => unreachable!("as_bytes() called on a non-bytes Value"),
    }
  }

  pub fn as_bigint(&self) -> &BigInt {
    debug_assert!(self.is_bigint());
    match unsafe { &*self.as_obj() } {
      Obj::BigInt(b) => b,
      _ => unreachable!("as_bigint() called on a non-bigint Value"),
    }
  }

  pub fn as_list(&self) -> &[Value] {
    debug_assert!(self.is_list());
    match unsafe { &*self.as_obj() } {
      Obj::List(items) => items.as_slice(),
      _ => unreachable!("as_list() called on a non-list Value"),
    }
  }

  pub fn as_dict(&self) -> &[(Value, Value)] {
    debug_assert!(self.is_dict());
    match unsafe { &*self.as_obj() } {
      Obj::Dict(pairs) => pairs.as_slice(),
      _ => unreachable!("as_dict() called on a non-dict Value"),
    }
  }

  pub fn as_upvalue(&self) -> &Cell<UpvalueState> {
    debug_assert!(self.is_upvalue());
    match unsafe { &*self.as_obj() } {
      Obj::Upvalue(cell) => cell,
      _ => unreachable!("as_upvalue() called on a non-upvalue Value"),
    }
  }

  /// Truthiness for control flow: nil and false are falsy, everything
  /// else (including 0 and "") is truthy.
  #[inline]
  pub fn is_falsey(&self) -> bool {
    self.is_nil()
      || (self.is_bool() && !self.as_bool())
      || (self.is_number() && self.as_number() <= 0.0)
      || (self.is_bigint() && self.as_bigint() <= &BigInt::from(0))
  }

  pub fn equals(&self, other: &Value) -> bool {
    if self.is_number() && other.is_number() {
      return self.as_number() == other.as_number();
    }
    if self.is_obj() && other.is_obj() {
      unsafe {
        return match (&*self.as_obj(), &*other.as_obj()) {
          (Obj::Str(a), Obj::Str(b)) => a == b,
          (Obj::BigInt(a), Obj::BigInt(b)) => a == b,
          (Obj::Bytes(a), Obj::Bytes(b)) => a.eq(b),
          (Obj::Func(a), Obj::Func(b)) => std::ptr::eq(a, b),
          (Obj::Closure(a), Obj::Closure(b)) => std::ptr::eq(a, b),
          (Obj::Upvalue(a), Obj::Upvalue(b)) => std::ptr::eq(a, b),
          (Obj::List(a), Obj::List(b)) => {
            a.len() == b.len() && a.iter().zip(b.iter()).all(|(x, y)| x.equals(y))
          },
          (Obj::Dict(a), Obj::Dict(b)) => {
            a.len() == b.len()
              && a
                .iter()
                .all(|(k, v)| b.iter().any(|(k2, v2)| k.equals(k2) && v.equals(v2)))
          },

          _ => false,
        };
      }
    }
    // nil/bool cases: raw bits already uniquely identify the value
    self.0 == other.0
  }

  pub fn type_name(&self) -> &'static str {
    /* if self.is_int() {
      "int"
    } else */
    if self.is_number() {
      "float"
    } else if self.is_nil() {
      "nil"
    } else if self.is_bool() {
      "bool"
    } else if self.is_obj() {
      unsafe {
        match &*self.as_obj() {
          Obj::Str(_) => "string",
          Obj::Bytes(_) => "bytes",
          Obj::BigInt(_) => "bigint",
          Obj::List(_) => "list",
          Obj::Dict(_) => "dict",
          Obj::Func(_) | Obj::Closure(_) | Obj::Native(_) => "function",
          Obj::Upvalue(_) => "upvalue",
        }
      }
    } else {
      "unknown"
    }
  }
}

impl std::fmt::Display for Value {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    /* if self.is_int() {
      write!(f, "{}", self.as_int())
    } else */
    if self.is_number() {
      write!(f, "{}", self.as_number())
    } else if self.is_nil() {
      write!(f, "nil")
    } else if self.is_bool() {
      write!(f, "{}", self.as_bool())
    } else if self.is_obj() {
      unsafe {
        match &*self.as_obj() {
          Obj::Str(s) => write!(f, "{}", s),
          Obj::Bytes(s) => write!(
            f,
            "({})",
            s.iter().map(|f| format!("{:02x}", f)).format(" ")
          ),
          Obj::BigInt(v) => write!(f, "{}n", v.to_string()),
          Obj::Func(func) => write!(
            f,
            "<function {}({}{})>",
            func.name,
            func.arity,
            if func.variadic { "..." } else { "" }
          ),
          Obj::Closure(c) => {
            let func = c.function.as_func();
            write!(
              f,
              "<function {}({}{})>",
              func.name,
              func.arity,
              if func.variadic { "..." } else { "" }
            )
          },
          Obj::Native(func) => write!(
            f,
            "<function {}({}{})>",
            func.name,
            func.min_arity,
            if func.variadic { "..." } else { "" }
          ),
          Obj::Upvalue(_) => write!(f, "<upvalue>"),
          Obj::List(items) => {
            write!(f, "[")?;
            for (i, item) in items.iter().enumerate() {
              if i > 0 {
                write!(f, ", ")?;
              }
              write!(f, "{}", item)?;
            }
            write!(f, "]")
          },
          Obj::Dict(pairs) => {
            write!(f, "{{")?;
            for (i, (k, v)) in pairs.iter().enumerate() {
              if i > 0 {
                write!(f, ", ")?;
              }
              write!(f, "{}: {}", k, v)?;
            }
            write!(f, "}}")
          },
        }
      }
    } else {
      write!(f, "<invalid value>")
    }
  }
}
