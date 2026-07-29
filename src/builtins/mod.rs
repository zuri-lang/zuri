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
use crate::builtins::function::FUNCTION_METHODS;
use crate::builtins::list::LIST_METHODS;
use crate::builtins::number::NUMBER_METHODS;
use crate::builtins::object::OBJECT_TO_STRING;
use crate::builtins::range::RANGE_METHODS;
use crate::builtins::string::STRING_METHODS;
use crate::vm::object::{NativeFn, NativeFunction, Obj, ZuriContext};
use crate::vm::value::Value;

mod bigint;
mod bool;
mod bytes;
mod dict;
pub mod enforce;
mod function;
mod list;
mod number;
mod object;
mod range;
mod string;

/// Every primitive "kind" that owns its own builtin-method table.
/// Instances/classes are deliberately absent -- an instance's real
/// class methods take priority (checked by the caller before ever
/// reaching `lookup`), and a class itself never gets a builtin method
/// at all (see `lookup`'s early return).
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
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
    // One dereference, one match -- previously this fell through up to
    // six separate is_x() checks (is_string, is_list, ..., is_callable
    // which is itself four more), each re-dereferencing the same
    // pointer and re-matching against the same Obj variants.
    match unsafe { &*v.as_obj() } {
      Obj::Str(_) => Some(Kind::String),
      Obj::List(_) => Some(Kind::List),
      Obj::Dict(_) => Some(Kind::Dict),
      Obj::Bytes(_) => Some(Kind::Bytes),
      Obj::Range { .. } => Some(Kind::Range),
      Obj::BigInt(_) => Some(Kind::BigInt),
      Obj::Func(_) | Obj::Closure(_) | Obj::Native(_) | Obj::BoundMethod(_) | Obj::Class(_) => {
        Some(Kind::Function)
      },
      Obj::Instance(_) | Obj::Upvalue(_) => None,
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
