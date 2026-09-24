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
//! This is the same trick used by JavaScriptCore, SpiderMonkey, LuaJIT and
//! the clox VM from "Crafting Interpreters"; here it's adapted to a
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

use std::cell::{Cell, RefCell};

use itertools::Itertools;
use num_bigint::{BigInt, Sign};
use num_traits::{Signed, ToPrimitive, Zero};

use crate::vm::object::{
  ASCII_NO, ASCII_YES, DictStorage, FileHandle, NativeFunction, Obj, ObjBoundMethod, ObjClass,
  ObjClosure, ObjFunction, ObjInstance, ObjModule, ObjModuleBinding, ObjPtr, UpvalueState,
  write_barrier,
};

use crate::vm::object::ListStorage;

// Visible at `pub(crate)` (not just private) because the Cranelift JIT
// backend (see `jit::codegen`) needs to replicate these exact bit tests
// as inline machine code for its fast paths; e.g. "is both operands a
// plain number"; rather than paying a real function-call round trip
// into `Value::is_number()` for the hottest possible check in the
// entire VM. `Value` being `#[repr(transparent)]` around a `u64` is
// what makes this safe to do from outside this module: the JIT never
// needs to know anything about `Obj`'s layout (which is NOT replicated
// this way: see `jit::runtime` for why anything touching a heap
// object's contents instead calls back into real Rust code).
pub(crate) const QNAN: u64 = 0x7ffc_0000_0000_0000; // exponent all 1s + top mantissa bit set: guaranteed non-NaN-we-produce
pub(crate) const SIGN_BIT: u64 = 0x8000_0000_0000_0000;

pub(crate) const TAG_NIL: u64 = 0b01;
pub(crate) const TAG_FALSE: u64 = 0b10;
pub(crate) const TAG_TRUE: u64 = 0b11;

// set only for boxed integers; unset for nil/true/false and for pointers
// const TAG_INT: u64 = 1 << 49;

pub(crate) const NIL_VAL: u64 = QNAN | TAG_NIL;
pub(crate) const FALSE_VAL: u64 = QNAN | TAG_FALSE;
pub(crate) const TRUE_VAL: u64 = QNAN | TAG_TRUE;

/// Mask covering exactly the low 48 bits, where we stash a pointer.
pub(crate) const PTR_MASK: u64 = 0x0000_ffff_ffff_ffff;

#[derive(Clone, Copy, Debug)]
#[repr(transparent)]
pub struct Value(u64);

impl Value {
  #[inline]
  pub fn nil() -> Value {
    Value(NIL_VAL)
  }

  /// `nil` in a const context; `ListStorage`'s inline buffer needs to
  /// be initialised in `const fn new`.
  pub const fn nil_const() -> Value {
    Value(NIL_VAL)
  }

  /// Reconstruct a `Value` from its raw NaN-boxed bit pattern; used
  /// only at the JIT/native-code boundary (see `jit::runtime`), where
  /// compiled code passes register contents across the ABI as plain
  /// `u64`s rather than `Value`s. Every bit pattern this can be called
  /// with was itself produced by `to_bits()` on a real `Value` sitting
  /// in `VM::registers`, so this never manufactures an invalid tag.
  #[inline(always)]
  pub fn from_bits(bits: u64) -> Value {
    Value(bits)
  }

  /// The inverse of `from_bits`; the raw bit pattern the JIT stores
  /// directly into a register slot (`VM::registers` is just `[Value]`,
  /// and `Value` is `#[repr(transparent)]` around a `u64`, so this is a
  /// same-layout reinterpretation, not a real conversion).
  #[inline(always)]
  pub fn to_bits(&self) -> u64 {
    self.0
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
  /// reachable: see `Heap` in object.rs, which owns every object for the
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
    self.is_obj() && matches!(unsafe { &*self.as_obj() }, Obj::Str(..))
  }

  /// Whether this string holds nothing but ASCII, which is what makes a
  /// codepoint index also a byte index.
  ///
  /// Answered from the flag cached on `Obj::Str` and computed on the
  /// first ask, so a string being indexed in a loop is scanned once
  /// rather than once per read. Compiled code reads the same flag
  /// directly (`obj_str_ascii_offset`) instead of calling here, which is
  /// what lets it index a string without a scan at all; it sees
  /// `ASCII_UNKNOWN` until something has asked once, and takes its slow
  /// path until then.
  ///
  /// `false` for anything that is not a string, so a caller that has not
  /// already checked cannot be misled by the answer.
  pub fn str_is_ascii(&self) -> bool {
    if !self.is_obj() {
      return false;
    }

    let obj = unsafe { &*self.as_obj() };
    match obj {
      Obj::Str(string) => match string.form().get() {
        ASCII_YES => true,
        ASCII_NO => false,
        _ => {
          let known = obj.str_text().as_bytes().is_ascii();
          string.form().set(if known { ASCII_YES } else { ASCII_NO });
          known
        },
      },
      _ => false,
    }
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

  #[inline]
  pub fn is_class(&self) -> bool {
    self.is_obj() && matches!(unsafe { &*self.as_obj() }, Obj::Class(_))
  }

  #[inline]
  pub fn is_instance(&self) -> bool {
    self.is_obj() && matches!(unsafe { &*self.as_obj() }, Obj::Instance(_))
  }

  #[inline]
  pub fn is_bound_method(&self) -> bool {
    self.is_obj() && matches!(unsafe { &*self.as_obj() }, Obj::BoundMethod(_))
  }

  /// Is this something Instr::Call can invoke; closure OR native. Same
  /// concept to a Zuri program; different call paths internally.
  pub fn is_callable(&self) -> bool {
    self.is_closure() || self.is_bound_method() || self.is_native() || self.is_class()
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
    unsafe { &*self.as_obj() }.str_text()
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

  pub fn as_bytes(&self) -> Vec<u8> {
    debug_assert!(self.is_bytes());
    match unsafe { &*self.as_obj() } {
      Obj::Bytes(b) => b.borrow().clone(),
      _ => unreachable!("as_bytes() called on a non-bytes Value"),
    }
  }

  /// Number of bytes; `Instr::GetIndex`/`SetIndex`/`GetSlice`'s
  /// bounds-checking entry point for a `bytes` receiver; avoids
  /// `as_bytes()`'s full clone just to check a length.
  /// Run `f` with read-only access to a bytes value's storage.
  ///
  /// The counterpart to `as_bytes` for callers that only read: that
  /// one hands back a full clone, which on a large buffer costs more
  /// than whatever the caller was going to do with it. Allocating
  /// while the borrow is live would risk the GC re-entering the same
  /// `RefCell`, so callers that build a new object collect first and
  /// allocate after.
  pub fn with_bytes<R>(&self, f: impl FnOnce(&[u8]) -> R) -> R {
    debug_assert!(self.is_bytes());
    match unsafe { &*self.as_obj() } {
      Obj::Bytes(b) => f(&b.borrow()),
      _ => unreachable!("with_bytes() called on a non-bytes Value"),
    }
  }

  /// `with_bytes` for a caller that writes back into the buffer.
  ///
  /// Same rule as the read-only version, and it matters more here:
  /// allocating anything while the borrow is live lets the collector
  /// re-enter this `RefCell`, so build whatever you are going to
  /// allocate before the call or after it, never inside `f`. Handing
  /// out `&mut Vec<u8>` rather than `&mut [u8]` is deliberate; a
  /// caller resizing a buffer in place (a decoder filling an empty
  /// one, say) would otherwise have to allocate a second one and
  /// copy it back.
  pub fn with_bytes_mut<R>(&self, f: impl FnOnce(&mut Vec<u8>) -> R) -> R {
    debug_assert!(self.is_bytes());
    match unsafe { &*self.as_obj() } {
      Obj::Bytes(b) => f(&mut b.borrow_mut()),
      _ => unreachable!("with_bytes_mut() called on a non-bytes Value"),
    }
  }

  /// `with_bytes`'s list counterpart; see its docs.
  pub fn with_list<R>(&self, f: impl FnOnce(&[Value]) -> R) -> R {
    debug_assert!(self.is_list());
    match unsafe { &*self.as_obj() } {
      Obj::List(items) => f(&items.borrow()),
      _ => unreachable!("with_list() called on a non-list Value"),
    }
  }

  /// `with_bytes`'s dict counterpart; hands over the storage itself
  /// rather than a slice, since callers want `entries` and the hash
  /// index both. See `with_bytes`'s docs.
  pub fn with_dict<R>(&self, f: impl FnOnce(&DictStorage) -> R) -> R {
    debug_assert!(self.is_dict());
    match unsafe { &*self.as_obj() } {
      Obj::Dict(storage) => f(&storage.borrow()),
      _ => unreachable!("with_dict() called on a non-dict Value"),
    }
  }

  pub fn bytes_len(&self) -> usize {
    debug_assert!(self.is_bytes());
    match unsafe { &*self.as_obj() } {
      Obj::Bytes(b) => {
        // SAFETY: nothing in this VM holds a live Ref/RefMut on a bytes
        // object across a re-entrant call into anything that could touch
        // the SAME object, and execution is single-threaded, so bypassing
        // RefCell's runtime flag cannot observe real aliasing. The
        // `debug_assert` keeps that invariant honest without letting the
        // two build profiles run different code.
        debug_assert!(b.try_borrow().is_ok(), "bytes_len over a live borrow");
        let vec_ref: &Vec<u8> = unsafe { &*b.as_ptr() };
        vec_ref.len()
      },
      _ => unreachable!("bytes_len() called on a non-bytes Value"),
    }
  }

  /// The bytes counterpart of `list_get`, and released from `RefCell`'s
  /// runtime flag for the same reason and under the same conditions: a
  /// borrow-flag check and its branch per byte is real cost on any loop
  /// walking a buffer, and there is no aliasing for the flag to catch.
  pub fn bytes_get(&self, index: usize) -> Option<u8> {
    debug_assert!(self.is_bytes());
    match unsafe { &*self.as_obj() } {
      Obj::Bytes(b) => {
        // SAFETY: see `bytes_len`.
        debug_assert!(b.try_borrow().is_ok(), "bytes_get over a live borrow");
        let vec_ref: &Vec<u8> = unsafe { &*b.as_ptr() };
        vec_ref.get(index).copied()
      },
      _ => unreachable!("bytes_get() called on a non-bytes Value"),
    }
  }

  /// Returns false if `index` is out of bounds; callers (Instr::SetIndex's
  /// handler) are expected to have already bounds-checked via
  /// `bytes_len()`, so this is a defensive double-check, not the
  /// primary bounds enforcement.
  pub fn bytes_set(&self, index: usize, value: u8) -> bool {
    debug_assert!(self.is_bytes());
    match unsafe { &*self.as_obj() } {
      Obj::Bytes(b) => {
        // SAFETY: see `bytes_len`.
        debug_assert!(b.try_borrow_mut().is_ok(), "bytes_set over a live borrow");
        let vec_ref: &mut Vec<u8> = unsafe { &mut *b.as_ptr() };
        match vec_ref.get_mut(index) {
          Some(slot) => {
            *slot = value;
            true
          },
          None => false,
        }
      },
      _ => unreachable!("bytes_set() called on a non-bytes Value"),
    }
  }

  pub fn as_bigint(&self) -> &BigInt {
    debug_assert!(self.is_bigint());
    match unsafe { &*self.as_obj() } {
      Obj::BigInt(b) => b,
      _ => unreachable!("as_bigint() called on a non-bigint Value"),
    }
  }

  pub fn as_list(&self) -> Vec<Value> {
    debug_assert!(self.is_list());
    match unsafe { &*self.as_obj() } {
      Obj::List(items) => items.borrow().to_vec(),
      _ => unreachable!("as_list() called on a non-list Value"),
    }
  }

  pub fn list_len(&self) -> usize {
    debug_assert!(self.is_list());
    match unsafe { &*self.as_obj() } {
      Obj::List(items) => {
        // SAFETY: nothing in this VM holds a live Ref/RefMut on a List
        // across a re-entrant call into anything that could touch the
        // SAME list, and execution is single-threaded, so bypassing
        // RefCell's runtime flag cannot observe real aliasing. The
        // `debug_assert` keeps that invariant honest without letting the
        // two build profiles run different code.
        debug_assert!(items.try_borrow().is_ok(), "list_len over a live borrow");
        let vec_ref: &ListStorage = unsafe { &*items.as_ptr() };
        vec_ref.len()
      },
      _ => unreachable!("list_len() called on a non-list Value"),
    }
  }

  pub fn list_get(&self, index: usize) -> Option<Value> {
    debug_assert!(self.is_list());
    match unsafe { &*self.as_obj() } {
      Obj::List(items) => {
        // SAFETY: see `list_len`.
        debug_assert!(items.try_borrow().is_ok(), "list_get over a live borrow");
        let vec_ref: &ListStorage = unsafe { &*items.as_ptr() };
        vec_ref.get(index).copied()
      },
      _ => unreachable!("list_get() called on a non-list Value"),
    }
  }

  pub fn list_set(&self, index: usize, value: Value) -> bool {
    debug_assert!(self.is_list());
    let ok = match unsafe { &*self.as_obj() } {
      Obj::List(items) => {
        // SAFETY: see `list_len`.
        debug_assert!(
          items.try_borrow_mut().is_ok(),
          "list_set over a live borrow"
        );
        let vec_ref: &mut ListStorage = unsafe { &mut *items.as_ptr() };
        match vec_ref.get_mut(index) {
          Some(slot) => {
            *slot = value;
            true
          },
          None => false,
        }
      },
      _ => unreachable!("list_set() called on a non-list Value"),
    };
    if ok {
      write_barrier(self.as_obj());
    }
    ok
  }

  pub fn as_dict(&self) -> Vec<(Value, Value)> {
    debug_assert!(self.is_dict());
    match unsafe { &*self.as_obj() } {
      Obj::Dict(storage) => storage.borrow().entries.clone(),
      _ => unreachable!("as_dict() called on a non-dict Value"),
    }
  }

  pub fn dict_len(&self) -> usize {
    debug_assert!(self.is_dict());
    match unsafe { &*self.as_obj() } {
      Obj::Dict(storage) => storage.borrow().len(),
      _ => unreachable!("dict_len() called on a non-dict Value"),
    }
  }

  pub fn dict_get(&self, key: &Value) -> Option<Value> {
    debug_assert!(self.is_dict());
    match unsafe { &*self.as_obj() } {
      Obj::Dict(storage) => storage.borrow().get(key),
      _ => unreachable!("dict_get() called on a non-dict Value"),
    }
  }

  pub fn dict_set(&self, key: Value, value: Value) {
    debug_assert!(self.is_dict());
    match unsafe { &*self.as_obj() } {
      Obj::Dict(storage) => storage.borrow_mut().set(key, value),
      _ => unreachable!("dict_set() called on a non-dict Value"),
    }
    write_barrier(self.as_obj());
  }

  /// Position of `key` in insertion order; O(1) average via the
  /// same index `dict_get` uses. What `@key`/`@value` (see
  /// `builtins/dict.rs`) use instead of a linear scan to step forward
  /// through a dict during iteration.
  pub fn dict_index_of(&self, key: &Value) -> Option<usize> {
    debug_assert!(self.is_dict());
    match unsafe { &*self.as_obj() } {
      Obj::Dict(storage) => storage.borrow().index_of(key),
      _ => unreachable!("dict_index_of() called on a non-dict Value"),
    }
  }

  pub fn dict_key_at(&self, index: usize) -> Option<Value> {
    debug_assert!(self.is_dict());
    match unsafe { &*self.as_obj() } {
      Obj::Dict(storage) => storage.borrow().entries.get(index).map(|(k, _)| *k),
      _ => unreachable!("dict_key_at() called on a non-dict Value"),
    }
  }

  pub fn dict_value_at(&self, index: usize) -> Option<Value> {
    debug_assert!(self.is_dict());
    match unsafe { &*self.as_obj() } {
      Obj::Dict(storage) => storage.borrow().entries.get(index).map(|(_, v)| *v),
      _ => unreachable!("dict_value_at() called on a non-dict Value"),
    }
  }

  #[inline]
  pub fn is_range(&self) -> bool {
    self.is_obj() && matches!(unsafe { &*self.as_obj() }, Obj::Range { .. })
  }

  pub fn as_range(&self) -> (f64, f64) {
    debug_assert!(self.is_range());
    match unsafe { &*self.as_obj() } {
      Obj::Range { lower, upper, .. } => (*lower, *upper),
      _ => unreachable!("as_range() called on a non-range Value"),
    }
  }

  pub fn as_upvalue(&self) -> &Cell<UpvalueState> {
    debug_assert!(self.is_upvalue());
    match unsafe { &*self.as_obj() } {
      Obj::Upvalue(cell) => cell,
      _ => unreachable!("as_upvalue() called on a non-upvalue Value"),
    }
  }

  pub fn as_bound_method(&self) -> &ObjBoundMethod {
    debug_assert!(self.is_bound_method());
    match unsafe { &*self.as_obj() } {
      Obj::BoundMethod(b) => b,
      _ => unreachable!("as_bound_method() called on a non-bound-method Value"),
    }
  }

  /// Same trust model as every other `as_*` here: safe only because
  /// `Value` is never used across a stale heap pointer (see `Heap`'s own
  /// docs). Returned as a `Ref`/`RefMut` rather than `&`/`&mut` because
  /// `ObjClass`'s tables are `RefCell`-wrapped: see `Obj::Class`.
  pub fn as_class(&self) -> std::cell::Ref<'_, ObjClass> {
    debug_assert!(self.is_class());
    match unsafe { &*self.as_obj() } {
      Obj::Class(c) => c.borrow(),
      _ => unreachable!("as_class() called on a non-class Value"),
    }
  }

  pub fn as_class_mut(&self) -> std::cell::RefMut<'_, ObjClass> {
    debug_assert!(self.is_class());
    match unsafe { &*self.as_obj() } {
      Obj::Class(c) => c.borrow_mut(),
      _ => unreachable!("as_class_mut() called on a non-class Value"),
    }
  }

  pub fn as_instance(&self) -> &ObjInstance {
    debug_assert!(self.is_instance());
    match unsafe { &*self.as_obj() } {
      Obj::Instance(i) => i,
      _ => unreachable!("as_instance() called on a non-instance Value"),
    }
  }

  #[inline]
  pub fn is_file(&self) -> bool {
    self.is_obj() && matches!(unsafe { &*self.as_obj() }, Obj::File(_))
  }

  pub fn as_file_cell(&self) -> &RefCell<FileHandle> {
    debug_assert!(self.is_file());
    match unsafe { &*self.as_obj() } {
      Obj::File(f) => f,
      _ => unreachable!("as_file_cell() called on a non-file Value"),
    }
  }

  pub fn range_step(&self) -> f64 {
    debug_assert!(self.is_range());
    match unsafe { &*self.as_obj() } {
      Obj::Range { step, .. } => step.get(),
      _ => unreachable!("range_step() called on a non-range Value"),
    }
  }

  pub fn range_set_step(&self, s: f64) {
    debug_assert!(self.is_range());
    match unsafe { &*self.as_obj() } {
      Obj::Range { step, .. } => step.set(s),
      _ => unreachable!("range_set_step() called on a non-range Value"),
    }
  }

  #[inline]
  pub fn is_module(&self) -> bool {
    self.is_obj() && matches!(unsafe { &*self.as_obj() }, Obj::Module(_))
  }

  #[inline]
  pub fn is_module_binding(&self) -> bool {
    self.is_obj() && matches!(unsafe { &*self.as_obj() }, Obj::ModuleBinding(_))
  }

  pub fn as_module(&self) -> std::cell::Ref<'_, ObjModule> {
    debug_assert!(self.is_module());
    match unsafe { &*self.as_obj() } {
      Obj::Module(m) => m.borrow(),
      _ => unreachable!("as_module() called on a non-module Value"),
    }
  }

  pub fn as_module_mut(&self) -> std::cell::RefMut<'_, ObjModule> {
    debug_assert!(self.is_module());
    match unsafe { &*self.as_obj() } {
      Obj::Module(m) => m.borrow_mut(),
      _ => unreachable!("as_module_mut() called on a non-module Value"),
    }
  }

  pub fn as_module_binding(&self) -> &ObjModuleBinding {
    debug_assert!(self.is_module_binding());
    match unsafe { &*self.as_obj() } {
      Obj::ModuleBinding(b) => b,
      _ => unreachable!("as_module_binding() called on a non-module-binding Value"),
    }
  }

  #[inline]
  pub fn is_ptr(&self) -> bool {
    self.is_obj() && matches!(unsafe { &*self.as_obj() }, Obj::Ptr(_))
  }

  /// The wrapped resource's type tag, or `None` if this isn't a Ptr at
  /// all; the first check a native should make before downcasting.
  pub fn ptr_type_name(&self) -> Option<&'static str> {
    if !self.is_ptr() {
      return None;
    }
    match unsafe { &*self.as_obj() } {
      Obj::Ptr(cell) => Some(cell.borrow().type_name),
      _ => unreachable!(),
    }
  }

  /// Convenience for a native that already knows the expected tag --
  /// `v.is_ptr_type("zuri::sql::sqlite3::connection")` before doing anything else
  /// with `v`.
  pub fn is_ptr_type(&self, expected: &str) -> bool {
    self.ptr_type_name() == Some(expected)
  }

  pub fn as_ptr_cell(&self) -> &RefCell<ObjPtr> {
    debug_assert!(self.is_ptr());
    match unsafe { &*self.as_obj() } {
      Obj::Ptr(cell) => cell,
      _ => unreachable!("as_ptr_cell() called on a non-ptr Value"),
    }
  }

  /// Truthiness for control flow: nil and false are falsy, everything
  /// else (including 0 and "") is truthy.
  #[inline]
  pub fn is_falsey(&self) -> bool {
    self.is_nil()
      || (self.is_bool() && !self.as_bool())
      || (self.is_number() && self.as_number() <= 0.0)
      // `<= 0` via the sign, so a truthiness test doesn't allocate a
      // throwaway BigInt zero to compare against every time.
      || (self.is_bigint() && self.as_bigint().sign() != Sign::Plus)
      || (self.is_string() && self.as_str().is_empty())
      || (self.is_bytes() && self.bytes_len() == 0)
  }

  pub fn equals(&self, other: &Value) -> bool {
    if self.is_number() && other.is_number() {
      return self.as_number() == other.as_number();
    }
    if self.is_obj() && other.is_obj() {
      unsafe {
        return match (&*self.as_obj(), &*other.as_obj()) {
          (a @ Obj::Str(..), b @ Obj::Str(..)) => a.str_text() == b.str_text(),
          (Obj::BigInt(a), Obj::BigInt(b)) => a == b,
          (Obj::Bytes(a), Obj::Bytes(b)) => a.borrow().eq(&*b.borrow()),
          (Obj::Func(a), Obj::Func(b)) => std::ptr::eq(a, b),
          (Obj::Closure(a), Obj::Closure(b)) => std::ptr::eq(a, b),
          (Obj::BoundMethod(a), Obj::BoundMethod(b)) => std::ptr::eq(a, b),
          (Obj::Upvalue(a), Obj::Upvalue(b)) => std::ptr::eq(a, b),
          (Obj::Class(a), Obj::Class(b)) => std::ptr::eq(a, b),
          (Obj::Instance(a), Obj::Instance(b)) => std::ptr::eq(a, b),
          (Obj::List(a), Obj::List(b)) => {
            let a = a.borrow();
            let b = b.borrow();
            a.len() == b.len() && a.iter().zip(b.iter()).all(|(x, y)| x.equals(y))
          },
          (Obj::Dict(a), Obj::Dict(b)) => {
            let a = a.borrow();
            let b = b.borrow();
            a.entries.len() == b.entries.len()
              && a
                .entries
                .iter()
                .all(|(k, v)| b.get(k).map(|v2| v.equals(&v2)).unwrap_or(false))
          },
          (Obj::File(a), Obj::File(b)) => std::ptr::eq(a, b),
          (
            Obj::Range {
              lower: l1,
              upper: u1,
              ..
            },
            Obj::Range {
              lower: l2,
              upper: u2,
              ..
            },
          ) => l1 == l2 && u1 == u2,
          (Obj::Ptr(a), Obj::Ptr(b)) => std::ptr::eq(a, b),
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
      "number"
    } else if self.is_nil() {
      "nil"
    } else if self.is_bool() {
      "bool"
    } else if self.is_obj() {
      unsafe {
        match &*self.as_obj() {
          Obj::Str(..) => "string",
          Obj::Bytes(_) => "bytes",
          Obj::BigInt(_) => "bigint",
          Obj::List(_) => "list",
          Obj::Dict(_) => "dict",
          Obj::Func(_) | Obj::Closure(_) | Obj::Native(_) | Obj::BoundMethod(_) => "function",
          Obj::Upvalue(_) => "upvalue",
          Obj::Class(_) => "class",
          Obj::Instance(_) => "instance",
          Obj::Module(_) => "module",
          Obj::ModuleBinding(_) => "module",
          Obj::Range { .. } => "range",
          Obj::File(_) => "file",
          Obj::Ptr(cell) => cell.borrow().type_name,
        }
      }
    } else {
      "unknown"
    }
  }

  /// Like `type_name()`, but for a class or an instance of one, reports
  /// the user-given class name instead of the generic "class"/"instance"
  ///; what error messages want when they say what type was actually
  /// passed. Everything else just defers to `type_name()`, which is
  /// already just as accurate for any type that isn't user-named.
  pub fn argument_type_name(&self) -> String {
    if self.is_obj() {
      unsafe {
        match &*self.as_obj() {
          Obj::Instance(i) => return i.class.as_class().name.clone(),
          _ => {},
        }
      }
    }
    self.type_name().to_string()
  }
}

/// Zuri's `%` on two numbers; `f64::rem` semantics exactly, but
/// computed with an integer remainder whenever both operands are
/// integers, which is the overwhelmingly common case.
///
/// Rust's `f64 % f64` lowers to a `fmod` call, and in this build that
/// resolves to `compiler_builtins`' portable SOFTWARE implementation
/// rather than anything the hardware does; it showed up as ~13% of
/// `benchmarks/fasta.zu`, whose random generator is
/// `(seed * IA + IC) % IM` over small integers. A single `idiv` is
/// dramatically cheaper for exactly that shape.
///
/// The result is bit-identical to `a % b`, not merely close, under
/// these guards (verified by exhaustive random and edge-case
/// differential testing before this was written):
/// - **Both magnitudes strictly below 2^63.** `as i64` SATURATES, so
///   without this the boundary value `9223372036854775808.0` converts
///   to `i64::MAX` and silently round-trips back to itself, giving a
///   wrong answer. This also makes the `i64::MIN / -1` overflow case
///   unreachable, and rejects both infinities and NaN for free (every
///   comparison against them is false).
/// - **Both integral**, checked by round-tripping through `i64`.
/// - **Divisor non-zero**, since integer division by zero traps where
///   `fmod` would return NaN.
///
/// `copysign` is not cosmetic: `fmod` gives a zero result the sign of
/// the DIVIDEND (`-4.0 % 2.0` is `-0.0`), while an integer remainder
/// gives `+0`. That was the only discrepancy the differential test
/// found, and this is what closes it.
#[inline]
/// Narrows a number to `i64` for the bitwise operators, modularly.
///
/// `as i64` saturates, which collapses every value at or above `2^63`
/// onto `i64::MAX` and loses the low bits the operators are asking
/// about: `16000000000000000000 & 255` is mathematically `0`, and
/// saturating answers `255`. Reducing modulo `2^64` keeps all 64 low
/// bits, so a mask or shift below that width stays exact. This is also
/// what an `i64`-width version of ECMAScript's `ToInt32` does, and what
/// the compiled tier gets for free from wrapping integer arithmetic.
///
/// A non-finite operand has no low bits to keep and narrows to `0`.
pub fn num_to_wrapped_i64(v: f64) -> i64 {
  /// `2^63`, exactly representable.
  const I64_SPAN: f64 = 9223372036854775808.0;
  /// `2^64`, likewise.
  const U64_SPAN: f64 = 18446744073709551616.0;

  if !v.is_finite() {
    return 0;
  }
  let t = v.trunc();
  if t >= -I64_SPAN && t < I64_SPAN {
    return t as i64;
  }
  // `rem_euclid` on a power of two is exact for every finite input, and
  // so is the shift back into signed range below it.
  let m = t.rem_euclid(U64_SPAN);
  if m >= I64_SPAN {
    (m - U64_SPAN) as i64
  } else {
    m as i64
  }
}

/// `num-bigint` panics outright when handed a zero divisor, which would
/// take the whole process down instead of surfacing as a catchable Zuri
/// error. The division-flavoured bigint operators go through these so the
/// VM can raise a `RangeError` the same way a script can handle it.
pub fn big_div(a: BigInt, b: BigInt) -> Result<BigInt, String> {
  if b.is_zero() {
    return Err("bigint division by zero".to_string());
  }
  Ok(a / b)
}

/// Floor division, matching what `//` does for plain numbers. `num-bigint`
/// truncates towards zero, so a negative result that did not divide evenly
/// needs to be nudged down by one.
pub fn big_floordiv(a: BigInt, b: BigInt) -> Result<BigInt, String> {
  if b.is_zero() {
    return Err("bigint division by zero".to_string());
  }
  let (q, r) = (&a / &b, &a % &b);
  if !r.is_zero() && (r.is_negative() != b.is_negative()) {
    return Ok(q - 1);
  }
  Ok(q)
}

pub fn big_rem(a: BigInt, b: BigInt) -> Result<BigInt, String> {
  if b.is_zero() {
    return Err("bigint modulo by zero".to_string());
  }
  Ok(a % b)
}

/// Exponentiation. The exponent has to fit in a `u32` because that is what
/// `num-bigint`'s binary exponentiation takes, and anything bigger would not
/// fit in memory anyway. A negative exponent has no integral answer, so it is
/// rejected rather than silently truncated to zero.
pub fn big_pow(a: BigInt, b: BigInt) -> Result<BigInt, String> {
  match b.to_u32() {
    Some(exp) => Ok(a.pow(exp)),
    None if b.is_negative() => Err("bigint '**' does not accept a negative exponent".to_string()),
    None => Err("bigint '**' exponent is too large".to_string()),
  }
}

pub fn num_rem(a: f64, b: f64) -> f64 {
  /// `2^63`, exactly representable, so the comparison is exact.
  const I64_LIMIT: f64 = 9223372036854775808.0;

  if a.abs() < I64_LIMIT && b.abs() < I64_LIMIT {
    let ia = a as i64;
    let ib = b as i64;
    if ib != 0 && ia as f64 == a && ib as f64 == b {
      return ((ia % ib) as f64).copysign(a);
    }
  }
  a % b
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
          obj @ Obj::Str(..) => write!(f, "{}", obj.str_text()),
          Obj::Bytes(s) => write!(
            f,
            "({})",
            s.borrow().iter().map(|f| format!("{:02x}", f)).format(" ")
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
          Obj::BoundMethod(b) => write!(f, "{}", b.method),
          Obj::Class(c) => write!(f, "<class {}>", c.borrow().name),
          Obj::Instance(i) => write!(f, "<instance of {}>", i.class.as_class().name),
          Obj::Module(m) => {
            let m = m.borrow();
            write!(f, "<module {} at {}>", m.name, m.path)
          },
          Obj::ModuleBinding(b) => {
            let module_path = b.module.as_module().path.clone();
            write!(f, "<module {} at {}>", b.bind_name, module_path)
          },
          Obj::List(items) => {
            write!(f, "[")?;
            for (i, item) in items.borrow().iter().enumerate() {
              if i > 0 {
                write!(f, ", ")?;
              }
              write!(f, "{}", item)?;
            }
            write!(f, "]")
          },
          Obj::Dict(storage) => {
            let storage = storage.borrow();
            write!(f, "{{")?;
            for (i, (k, v)) in storage.entries.iter().enumerate() {
              if i > 0 {
                write!(f, ", ")?;
              }
              write!(f, "{}: {}", k, v)?;
            }
            write!(f, "}}")
          },
          Obj::Range { lower, upper, step } => {
            let s = step.get();
            if s == 1.0 {
              write!(f, "{}..{}", lower, upper)
            } else {
              write!(f, "<range {}..{}, step={}>", lower, upper, s)
            }
          },
          Obj::File(fh_cell) => {
            let fh = fh_cell.borrow();
            write!(f, "<file at {} in mode {}>", fh.path, fh.mode)
          },
          Obj::Ptr(cell) => write!(
            f,
            "<ptr {} at {:p}>",
            cell.borrow().type_name,
            self.as_obj()
          ),
        }
      }
    } else {
      write!(f, "<invalid value>")
    }
  }
}

#[cfg(test)]
mod num_rem_tests {
  use super::num_rem;

  /// `num_rem` must be BIT-identical to `f64 % f64`, not merely close --
  /// the interpreter and the JIT both use it, and a difference would
  /// make results depend on which tier ran.
  #[test]
  fn matches_f64_rem_bit_for_bit() {
    let mut state = 0x243F6A8885A308D3u64;
    let mut next = || {
      state ^= state << 13;
      state ^= state >> 7;
      state ^= state << 17;
      state
    };

    let mut checked = 0u64;
    for _ in 0..200_000 {
      let a = ((next() as i64) >> (next() % 63)) as f64;
      let b = ((next() as i64) >> (next() % 63)) as f64;
      assert_eq!(num_rem(a, b).to_bits(), (a % b).to_bits(), "{a} % {b}");
      checked += 1;
    }
    assert!(checked > 0);

    // The cases the guards exist for, called out explicitly.
    let edges = [
      0.0f64,
      -0.0,
      1.0,
      -1.0,
      2.0,
      -2.0,
      -4.0,
      0.5,
      -0.5,
      1.5,
      -1.5,
      139968.0,
      3877.0,
      9007199254740992.0,
      -9007199254740992.0,
      // +/- 2^63 exactly: `as i64` saturates here, which is what the
      // magnitude guard exists to reject.
      9223372036854775808.0,
      -9223372036854775808.0,
      1e300,
      -1e300,
      f64::NAN,
      f64::INFINITY,
      f64::NEG_INFINITY,
    ];
    for &a in &edges {
      for &b in &edges {
        assert_eq!(num_rem(a, b).to_bits(), (a % b).to_bits(), "{a} % {b}");
      }
    }
  }

  /// The specific case an integer remainder gets wrong without the
  /// `copysign` fix-up: a zero result keeps the DIVIDEND's sign.
  #[test]
  fn negative_zero_result_keeps_its_sign() {
    assert!(num_rem(-4.0, 2.0).is_sign_negative());
    assert!(num_rem(4.0, 2.0).is_sign_positive());
  }
}
