//! Built-in method dispatch for non-instance receivers (numbers,
//! strings, lists, dicts, bytes, ranges, bools, nil, functions) plus
//! the one universal fallback every OTHER value (instances) gets too --
//! `to_string()`.
//!
//! Mirrors the C runtime's per-type builtin-method tables: rather than
//! a chain of `if receiver.is_x() { ... } else if receiver.is_y() {
//! ... }` at every call site, each primitive "kind" gets its own
//! `FxHashMap<name, NativeFunction>`, built once and looked up in O(1) --
//! the exact same shape `ObjClass::methods` already gives a real class,
//! just for values that don't have one. `Instr::Invoke`'s handler is
//! the only caller; the resolved `NativeFunction` is run through the
//! ordinary `VM::call_native` path (arity checking included for free),
//! with the receiver spliced in as the implicit first argument -- the
//! same calling convention a real bound method already uses.

use std::sync::LazyLock;

use rustc_hash::FxHashMap;

use crate::builtins::bigint::BIGINT_METHODS;
use crate::builtins::bool::BOOL_METHODS;
use crate::builtins::bytes::BYTES_METHODS;
use crate::builtins::dict::DICT_METHODS;
use crate::builtins::enforce::enforce_method_arg_count;
use crate::builtins::file::FILE_METHODS;
use crate::builtins::function::FUNCTION_METHODS;
use crate::builtins::list::LIST_METHODS;
use crate::builtins::number::NUMBER_METHODS;
use crate::builtins::object::OBJECT_TO_STRING;
use crate::builtins::ptr::PTR_METHODS;
use crate::builtins::range::RANGE_METHODS;
use crate::builtins::string::STRING_METHODS;
use crate::vm::object::{NativeFn, NativeFunction, Obj, ZuriContext};
use crate::vm::value::Value;

mod bigint;
mod bool;
mod bytes;
mod dict;
pub mod enforce;
pub mod file;
mod function;
mod list;
mod number;
mod object;
pub mod ptr;
mod range;
mod string;

/// Every primitive "kind" that owns its own builtin-method table.
/// Instances/classes are deliberately absent -- an instance's real
/// class methods take priority (checked by the caller before ever
/// reaching `lookup`), and a class itself never gets a builtin method
/// at all (see `lookup`'s early return).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum Kind {
  Number,
  Bool,
  BigInt,
  String,
  List,
  Dict,
  Bytes,
  Range,
  Function,
  File,
  Ptr,
  Nil,
}

impl Kind {
  fn of(v: Value) -> Option<Self> {
    // Cheap tag-bit checks first -- no pointer dereference at all.
    if v.is_number() {
      return Some(Kind::Number);
    }
    if v.is_bool() {
      return Some(Kind::Bool);
    }
    if v.is_nil() {
      return Some(Kind::Nil);
    }
    if !v.is_obj() {
      return None;
    }
    // One dereference, one match, rather than a chain of is_string()/
    // is_list()/... checks each re-dereferencing the same pointer.
    match unsafe { &*v.as_obj() } {
      Obj::Str(_) => Some(Kind::String),
      Obj::List(_) => Some(Kind::List),
      Obj::Dict(_) => Some(Kind::Dict),
      Obj::Bytes(_) => Some(Kind::Bytes),
      Obj::Range { .. } => Some(Kind::Range),
      Obj::BigInt(_) => Some(Kind::BigInt),
      Obj::File(_) => Some(Kind::File),
      Obj::Ptr(_) => Some(Kind::Ptr),
      Obj::Func(_) | Obj::Closure(_) | Obj::Native(_) | Obj::BoundMethod(_) | Obj::Class(_) => {
        Some(Kind::Function)
      },
      Obj::Instance(_) | Obj::Upvalue(_) | Obj::Module(_) | Obj::ModuleBinding(_) => None,
    }
  }
}

type MethodTable = FxHashMap<&'static str, NativeFunction>;

/// `min_arity` is always 1 -- the implicit receiver, spliced in by the
/// caller as `args[0]` -- since every builtin below currently takes no
/// further arguments; `call_native`'s existing arity check is what
/// turns e.g. `x.length(1)` into a proper ArgumentError for free,
/// rather than every method re-validating its own arg count.
pub fn method(name: &'static str, func: NativeFn) -> (&'static str, NativeFunction) {
  (
    name,
    NativeFunction {
      name,
      min_arity: 1,
      variadic: false,
      is_method: true,
      func,
    },
  )
}

/// Same as method, but allows specifying `arity` of the function.
/// The arity provided shoukd be the number of arguments expected by the function without counting the implicit receiver.
/// For example, `x.length(1)` has arity 1.
pub fn method_n(name: &'static str, arity: u8, func: NativeFn) -> (&'static str, NativeFunction) {
  (
    name,
    NativeFunction {
      name,
      min_arity: arity + 1,
      variadic: false,
      is_method: true,
      func,
    },
  )
}

/// Same as `method_n`, but the trailing arguments beyond
/// `required_extra` are OPTIONAL rather than fixed -- `min_arity`
/// becomes a floor and the native itself is responsible for checking
/// `ctx.args.len()` to see how many of its optional parameters were
/// actually supplied. Used for spec'd-optional trailing params like
/// `trim([chr])` or `index_of(str, [start])`, where `method_n`'s exact-
/// arity check would be too strict.
pub fn method_opt(
  name: &'static str,
  required_extra: u8,
  func: NativeFn,
) -> (&'static str, NativeFunction) {
  (
    name,
    NativeFunction {
      name,
      min_arity: 1 + required_extra,
      variadic: true,
      is_method: true,
      func,
    },
  )
}

pub fn build(entries: Vec<(&'static str, NativeFunction)>) -> MethodTable {
  entries.into_iter().collect()
}

static NIL_METHODS: LazyLock<MethodTable> =
  LazyLock::new(|| build(vec![method("to_string", to_string)]));

fn table_for(kind: Kind) -> &'static MethodTable {
  match kind {
    Kind::Number => &NUMBER_METHODS,
    Kind::Bool => &BOOL_METHODS,
    Kind::String => &STRING_METHODS,
    Kind::List => &LIST_METHODS,
    Kind::Dict => &DICT_METHODS,
    Kind::Bytes => &BYTES_METHODS,
    Kind::Range => &RANGE_METHODS,
    Kind::Function => &FUNCTION_METHODS,
    Kind::BigInt => &BIGINT_METHODS,
    Kind::File => &FILE_METHODS,
    Kind::Ptr => &PTR_METHODS,
    Kind::Nil => &NIL_METHODS,
  }
}

/// Resolve `name` as a builtin method on `receiver`. `None` isn't an
/// error -- it just means `name` isn't one of these, and the caller
/// (`Instr::Invoke`) reports its own "undefined"/"cannot call" error
/// with whatever wording fits its context (instance vs. bare
/// primitive).
///
/// Classes are excluded outright: a class is a template, not a value
/// with contents to stringify or measure.
/// Every `Kind`, in declaration order -- the index space
/// `OPERATOR_NAMES` is built over.
const ALL_KINDS: [Kind; 12] = [
  Kind::Number,
  Kind::Bool,
  Kind::BigInt,
  Kind::String,
  Kind::List,
  Kind::Dict,
  Kind::Bytes,
  Kind::Range,
  Kind::Function,
  Kind::File,
  Kind::Ptr,
  Kind::Nil,
];

/// Per-kind bitmask summarising which `@`-prefixed (operator/protocol)
/// methods that kind's table defines, keyed by the single byte that
/// FOLLOWS the `@`. Derived from the tables themselves, so adding a new
/// one anywhere is picked up automatically and this cannot drift out of
/// sync with them.
///
/// A byte rather than the whole name deliberately: the point is to
/// answer "no" in a load, a shift and an AND, with no string
/// comparison at all. A first-byte collision (two decorators sharing
/// the letter after `@`) only costs a fall-through to the real table
/// lookup, never a wrong answer -- so correctness never depends on the
/// summary being precise, only on it never CLEARING a bit for something
/// the table has.
///
/// This matters because `VM::try_operator_override` asks millions of
/// times and almost always gets `None`: every arithmetic operation
/// whose operands are not both numeric consults it, which includes
/// every single string concatenation.
static OPERATOR_MASKS: LazyLock<[u64; 12]> = LazyLock::new(|| {
  ALL_KINDS.map(|kind| {
    table_for(kind)
      .keys()
      .filter(|name| name.starts_with('@'))
      .fold(0u64, |mask, name| mask | deco_bit(name).unwrap_or(u64::MAX))
  })
});

/// The `OPERATOR_MASKS` bit for a decorator, from the byte after its
/// `@`. `None` for a name too short to have one, which no real
/// decorator is.
#[inline]
fn deco_bit(deco: &str) -> Option<u64> {
  let b = *deco.as_bytes().get(1)?;
  Some(1u64 << (b % 64))
}

/// Resolve an OPERATOR decorator (`@add`, `@lshift`, ...) on a builtin
/// receiver -- `lookup` specialized for the one caller that asks
/// millions of times and almost always gets `None`. Same answer as
/// `lookup` for any `@`-prefixed name, reached without hashing.
pub fn lookup_operator(receiver: Value, deco: &str) -> Option<&'static NativeFunction> {
  // `OPERATOR_MASKS` only summarises `@`-prefixed entries, so a plain
  // method name would get a wrong `None` here rather than a slower
  // answer. Every caller passes a decorator literal; this catches a
  // future one that does not.
  debug_assert!(
    deco.starts_with('@'),
    "lookup_operator is only valid for '@'-prefixed decorators, got '{deco}'"
  );
  if receiver.is_class() {
    return None;
  }
  let kind = Kind::of(receiver)?;
  if OPERATOR_MASKS[kind as usize] & deco_bit(deco)? == 0 {
    return None;
  }
  table_for(kind).get(deco)
}

pub fn lookup(receiver: Value, name: &str) -> Option<&'static NativeFunction> {
  if receiver.is_class() {
    return None;
  }
  if let Some(kind) = Kind::of(receiver) {
    return table_for(kind).get(name);
  }
  // Anything left (instances, and anything else with no primitive
  // kind, e.g. a bigint) only ever gets the universal fallback, and
  // only when its own class hasn't already declared `to_string`.
  if name == "to_string" {
    return Some(&OBJECT_TO_STRING);
  }
  None
}

/// `Value` already implements `Display` with exactly the
/// representation every other part of the VM uses (echo, string
/// interpolation, ...), so this is the one source of truth -- reachable
/// here as `.to_string()` too, for every kind except a class.
fn to_string(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  let s = format!("{}", ctx.args[0]);
  Ok(ctx.heap().alloc_string(s))
}

#[cfg(test)]
mod operator_lookup_tests {
  use super::*;

  /// `lookup_operator` indexes `OPERATOR_NAMES` by `kind as usize`, so
  /// `ALL_KINDS` must stay in `Kind`'s own declaration order. Reordering
  /// either one alone would silently hand every kind another kind's
  /// operator list.
  #[test]
  fn all_kinds_matches_discriminant_order() {
    for (i, kind) in ALL_KINDS.iter().enumerate() {
      assert_eq!(*kind as usize, i, "ALL_KINDS[{i}] is out of order");
    }
  }

  /// The fast negative path must never hide a method the real table
  /// has: for every kind, every `@` name it defines must have its bit
  /// set. (The converse is deliberately NOT required -- a spare set bit
  /// only costs a fall-through, never a wrong answer.)
  #[test]
  fn operator_masks_never_hide_a_real_method() {
    for kind in ALL_KINDS {
      for (name, _) in table_for(kind).iter() {
        if name.starts_with('@') {
          let bit = deco_bit(name).expect("a decorator always has a byte after '@'");
          assert!(
            OPERATOR_MASKS[kind as usize] & bit != 0,
            "{name} would be missed for {kind:?}"
          );
        }
      }
    }
  }

  /// And the summary must agree with `lookup` itself, answer for
  /// answer, on every `@` name any table defines.
  #[test]
  fn lookup_operator_agrees_with_lookup() {
    let decos = [
      "@add", "@sub", "@mul", "@div", "@mod", "@pow", "@floordiv", "@and", "@or", "@xor",
      "@lshift", "@rshift", "@urshift", "@lt", "@lte", "@gt", "@gte", "@neg", "@not", "@key",
      "@value",
    ];
    let probes = [
      Value::number(1.0),
      Value::bool(true),
      Value::nil(),
    ];
    for v in probes {
      for deco in decos {
        assert_eq!(
          lookup_operator(v, deco).is_some(),
          lookup(v, deco).is_some(),
          "disagreement on {deco}"
        );
      }
    }
  }
}
