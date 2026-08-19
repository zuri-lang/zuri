#![allow(unused)]

//! Standardized argument validation for native function/method bodies
//! -- the Rust-side equivalent of the C runtime's `ENFORCE_ARG_COUNT`/
//! `ENFORCE_ARG_TYPE` macros. Exists so every native gets identically
//! phrased errors for free, instead of every file hand-rolling its own
//! `format!("... expects a string, got {}", ...)` (which had already
//! started drifting inconsistent across `string.rs` alone).
//!
//! These are declarative macros (`macro_rules!`), not functions --
//! they need to `return Err(...)` out of the CALLER's native fn body on
//! failure, which only a macro can do; a function call can't make its
//! caller return. Each is only meaningful as the first thing a native
//! does with a given argument, same as `?`.
//!
//! `#[macro_export]` puts these at the crate root (`crate::enforce_arg_count!`,
//! or `zuri::enforce_arg_count!` from an external crate), not nested
//! under this module -- deliberately, so a library author writing their
//! own natives against this crate doesn't need to know or care that
//! `enforce.rs` is where they happen to live.

use crate::vm::value::Value;

/// Mirrors the C runtime's `IS_STRING`/`IS_NUMBER`/... predicates --
/// one variant per `Value::is_*` check, usable as the type argument to
/// `enforce_arg_type!`/`enforce_arg_types!`. Bundles both the runtime
/// check (`matches`) and its human-readable name (`label`) so an error
/// message is never more than one call away from being generated
/// automatically, and both stay impossible to accidentally desync from
/// each other.
#[derive(Clone, Copy)]
pub enum ArgType {
  Number,
  Bool,
  String,
  Bytes,
  BigInt,
  List,
  Dict,
  Range,
  /// Anything `Instr::Call` can invoke -- closure, bound method, or
  /// native -- matching `Value::is_callable()`.
  Function,
  Class,
  /// Any instance, regardless of class. For "must be an instance of
  /// SPECIFICALLY class X", check `v.is_instance() &&
  /// v.as_instance().class.as_class().name == "X"` by hand -- narrow
  /// enough, and rare enough among CURRENT natives, not to warrant its
  /// own variant yet.
  Instance,
  Nil,
  /// A `Ptr` wrapping specifically the resource named by the given
  /// tag (see `ObjPtr::type_name`) -- e.g.
  /// `ArgType::PtrOf("sqlite3_connection")`. Distinct from a bare
  /// "any Ptr" check: a `gd_image` handed to a function expecting a
  /// `sqlite3_connection` should fail here, not at the `downcast`
  /// call inside the native body.
  PtrOf(&'static str),
  /// Accepts anything -- useful for `enforce_arg_types!`'s uniform
  /// call shape when only SOME positions need a real constraint.
  Any,
}

impl ArgType {
  pub fn matches(self, v: Value) -> bool {
    match self {
      ArgType::Number => v.is_number(),
      ArgType::Bool => v.is_bool(),
      ArgType::String => v.is_string(),
      ArgType::Bytes => v.is_bytes(),
      ArgType::BigInt => v.is_bigint(),
      ArgType::List => v.is_list(),
      ArgType::Dict => v.is_dict(),
      ArgType::Range => v.is_range(),
      ArgType::Function => v.is_callable(),
      ArgType::Class => v.is_class(),
      ArgType::Instance => v.is_instance(),
      ArgType::PtrOf(tag) => v.is_ptr_type(tag),
      ArgType::Nil => v.is_nil(),
      ArgType::Any => true,
    }
  }

  pub fn label(self) -> &'static str {
    match self {
      ArgType::Number => "a number",
      ArgType::Bool => "a bool",
      ArgType::String => "a string",
      ArgType::Bytes => "bytes",
      ArgType::BigInt => "a bigint",
      ArgType::List => "a list",
      ArgType::Dict => "a dict",
      ArgType::Range => "a range",
      ArgType::Function => "a function",
      ArgType::Class => "a class",
      ArgType::Instance => "an instance",
      // Static-str-only limitation: a Ptr's error text can't embed
      // the tag through this fn alone (unlike every other variant,
      // the "expected" description here is itself dynamic data, not
      // baked into the variant's own match arm). Handled by
      // `describe_types`/the macros calling `type_name()` directly
      // instead where the ACTUAL received value's tag matters more
      // than the generic label anyway -- see `ptr_type_mismatch_msg`
      // below for the real (informative) message every `enforce_*`
      // callsite actually emits for this variant.
      ArgType::PtrOf(_) => "a pointer",
      ArgType::Nil => "nil",
      ArgType::Any => "a value",
    }
  }
}

/// Joins several `ArgType` labels into one readable phrase --
/// `"a number"`, `"a number or a list"`, `"a string, a list, or a
/// dict"` -- for the `*_any_of*!` macros' error messages. `pub`, not
/// `pub(crate)`, since the free-function `enforce_arg_type_any_of!` is
/// itself `#[macro_export]`ed and needs this reachable from wherever
/// that macro expands, including an external crate.
pub fn describe_types(types: &[ArgType]) -> String {
  let labels: Vec<&str> = types.iter().map(|t| t.label()).collect();
  match labels.as_slice() {
    [] => "a value".to_string(),
    [one] => one.to_string(),
    [a, b] => format!("{} or {}", a, b),
    _ => {
      let (last, rest) = labels.split_last().unwrap();
      format!("{}, or {}", rest.join(", "), last)
    },
  }
}

/// Enforce that this call received EXACTLY `$n` arguments. For a
/// method-style native (receiver spliced into `ctx.args[0]` by
/// `Instr::Invoke`/`call_native`, see `builtins/mod.rs`'s own doc
/// comment), `$n` counts the receiver too, matching how every existing
/// native already indexes `ctx.args` directly. Meant for natives
/// registered `variadic` (e.g. via `method_opt`) whose min_arity floor
/// alone isn't precise enough -- an EXACT-arity native's count is
/// already fully enforced up front by `VM::call_native`, before the
/// body ever runs, so this would just be a redundant second check
/// there.
#[macro_export]
macro_rules! enforce_arg_count {
  ($ctx:expr, $n:expr) => {
    if $ctx.args.len() != $n {
      return Err(format!(
        "{}() expects {} argument{}, got {}",
        $ctx.name,
        $n,
        if $n == 1 { "" } else { "s" },
        $ctx.args.len()
      ));
    }
  };
}

/// Like `enforce_arg_count!`, but for a RANGE -- `trim([chr])` accepts
/// 1 or 2 (receiver + optional char), `index_of(str, [start])` accepts
/// 2 or 3, etc.
#[macro_export]
macro_rules! enforce_arg_range {
  ($ctx:expr, $min:expr, $max:expr) => {
    #[allow(unused_comparisons)]
    if $ctx.args.len() < $min || $ctx.args.len() > $max {
      return Err(format!(
        "{}() expects between {} and {} arguments, got {}",
        $ctx.name,
        $min,
        $max,
        $ctx.args.len()
      ));
    }
  };
}

/// Enforce that `ctx.args[$idx]` exists and matches `$ty` (an
/// `ArgType`). `$idx` is a raw position into `ctx.args`, same as every
/// other place in this codebase indexes it directly -- 0 is the
/// receiver for a method-style native, 1.. are the real arguments; for
/// a free-function native with no implicit receiver (see `natives.rs`)
/// it's 0.. directly. Missing entirely (an out-of-range index) is
/// reported distinctly from "wrong type", since a native should only
/// ever hit that case for an argument it declared itself, not a caller
/// mistake `call_native`'s arity check would already have caught.
#[macro_export]
macro_rules! enforce_arg_type {
  ($ctx:expr, $idx:expr, $ty:expr) => {
    match $ctx.args.get($idx) {
      Some(v) if $crate::builtins::enforce::ArgType::matches($ty, *v) => {},
      Some(v) => {
        return Err(format!(
          "{}() expects argument {} to be {}, got {}",
          $ctx.name,
          $idx + 1,
          $crate::builtins::enforce::ArgType::label($ty),
          v.type_name()
        ));
      },
      None => {
        return Err(format!(
          "{}() expects argument {} ({}), but it was not given",
          $ctx.name,
          $idx + 1,
          $crate::builtins::enforce::ArgType::label($ty)
        ));
      },
    }
  };
}

/// Like `enforce_arg_type!`, but a MISSING argument is not an error --
/// only a present-but-wrong-typed one is. For a spec'd-optional
/// trailing parameter (`lpad(width, [fill])`'s `fill`), pair this with
/// a plain `ctx.args.get($idx)` read for the actual default-value
/// fallback; this macro only validates, it never substitutes.
#[macro_export]
macro_rules! enforce_arg_type_opt {
  ($ctx:expr, $idx:expr, $ty:expr) => {
    if let Some(v) = $ctx.args.get($idx) {
      if !$crate::builtins::enforce::ArgType::matches($ty, *v) {
        return Err(format!(
          "{}() expects argument {} to be {}, got {}",
          $ctx.name,
          $idx + 1,
          $crate::builtins::enforce::ArgType::label($ty),
          v.type_name()
        ));
      }
    }
  };
}

/// Enforce several REQUIRED argument types in one call:
/// `enforce_arg_types!(ctx, 1 => ArgType::String, 2 => ArgType::Number)`.
/// Purely `enforce_arg_type!` repeated per pair -- exists so a native
/// with several positional constraints reads as one declaration block
/// at the top of the function instead of a stack of near-identical
/// lines.
#[macro_export]
macro_rules! enforce_arg_types {
  ($ctx:expr, $($idx:expr => $ty:expr),+ $(,)?) => {
    $( $crate::enforce_arg_type!($ctx, $idx, $ty); )+
  };
}

/// Like `enforce_arg_type!`, but accepts a UNION of shapes --
/// `enforce_arg_type_any_of!(ctx, 0, [ArgType::Number, ArgType::List])`
/// for something like `bytes(n | list)`, where no single `ArgType`
/// describes what's actually accepted.
#[macro_export]
macro_rules! enforce_arg_type_any_of {
  ($ctx:expr, $idx:expr, [$($ty:expr),+ $(,)?]) => {
    match $ctx.args.get($idx) {
      Some(v) if [$($ty),+].iter().any(|t| $crate::builtins::enforce::ArgType::matches(*t, *v)) => {},
      Some(v) => {
        return Err(format!(
          "{}() expects argument {} to be {}, got {}",
          $ctx.name,
          $idx + 1,
          $crate::builtins::enforce::describe_types(&[$($ty),+]),
          v.type_name()
        ));
      },
      None => {
        return Err(format!(
          "{}() expects argument {} ({}), but it was not given",
          $ctx.name,
          $idx + 1,
          $crate::builtins::enforce::describe_types(&[$($ty),+])
        ));
      },
    }
  };
}

/// Optional-argument counterpart to `enforce_arg_type_any_of!` -- a
/// missing argument is not an error, only a present-but-wrong-shaped
/// one is. Same relationship `enforce_arg_type_opt!` has to
/// `enforce_arg_type!`.
#[macro_export]
macro_rules! enforce_arg_type_any_of_opt {
  ($ctx:expr, $idx:expr, [$($ty:expr),+ $(,)?]) => {
    if let Some(v) = $ctx.args.get($idx) {
      if ![$($ty),+].iter().any(|t| $crate::builtins::enforce::ArgType::matches(*t, *v)) {
        return Err(format!(
          "{}() expects argument {} to be {}, got {}",
          $ctx.name,
          $idx + 1,
          $crate::builtins::enforce::describe_types(&[$($ty),+]),
          v.type_name()
        ));
      }
    }
  };
}

/// Like `enforce_arg_type!`, but specifically for `ArgType::PtrOf(tag)` --
/// produces a message naming the ACTUAL wrapped type on a mismatch
/// (e.g. "expects argument 1 to be a sqlite3_connection, got a
/// gd_image") instead of the generic "a pointer" `ArgType::label`
/// falls back to for this variant. Free-function form; see
/// `enforce_method_arg_ptr!` below for the method-style counterpart.
#[macro_export]
macro_rules! enforce_arg_ptr {
  ($ctx:expr, $idx:expr, $tag:expr) => {
    match $ctx.args.get($idx) {
      Some(v) if v.is_ptr_type($tag) => {},
      Some(v) if v.is_ptr() => {
        return Err(format!(
          "{}() expects argument {} to be a {}, got a {}",
          $ctx.name,
          $idx + 1,
          $tag,
          v.ptr_type_name().unwrap_or("ptr")
        ));
      },
      Some(v) => {
        return Err(format!(
          "{}() expects argument {} to be a {}, got {}",
          $ctx.name,
          $idx + 1,
          $tag,
          v.type_name()
        ));
      },
      None => {
        return Err(format!(
          "{}() expects argument {} (a {}), but it was not given",
          $ctx.name,
          $idx + 1,
          $tag
        ));
      },
    }
  };
}

// Method-style variants.
//
// Every native registered through `builtins::method`/`method_n`/
// `method_opt` (see `builtins/mod.rs`) is invoked with the RECEIVER
// spliced into `ctx.args[0]` -- `Instr::Invoke`'s calling convention,
// not something the user ever typed. A user calling `x.abs(1)` wrote
// ONE argument; `ctx.args.len()` at that point is TWO. The
// `enforce_arg_*!` family above reports raw `ctx.args` counts/indices
// verbatim, which is exactly right for a free function (`abs(x)` --
// no implicit receiver, `ctx.args` IS what the user typed) but wrong
// for a method: it would tell the user `'abs' expects 1 argument, got
// 2` for a call that, from where they're sitting, took 0 and got 1.
//
// These mirror the free-function macros exactly, but every user-facing
// number has the receiver subtracted back out: `$n`/`$min`/`$max`
// count only the REAL arguments (what a Zuri program actually wrote
// between the parens), and `$idx` for a type check is that same
// 1-based real-argument position -- which, since the receiver already
// occupies `ctx.args[0]`, happens to equal the raw `ctx.args` index
// directly (`ctx.args[1]` IS argument 1, so the index itself needs no
// further adjustment -- only the printed numbers in the free-function
// versions' `$idx + 1` did, and this family drops that `+ 1`).
//
// Deliberately NOT `#[macro_export]`/`pub` -- these encode a calling
// convention (`Instr::Invoke`'s receiver-splicing) that's purely an
// implementation detail of how THIS runtime dispatches `obj.method()`
// calls. A library author writing their own natives against this
// crate calls THEIR functions directly with no such implicit receiver,
// so only the free-function macros above are meaningful public API;
// these stay an internal `builtins::` convenience, usable only from
// within this crate.
//
// Every method-table native already goes through `VM::call_native`'s
// own arity check first (see its call site in `Instr::Invoke`/
// `dispatch_call`), which guarantees `ctx.args.len() >= 1` (the
// receiver) before a native body ever runs -- so `ctx.args.len() - 1`
// below can never underflow.

macro_rules! enforce_method_arg_count {
  ($ctx:expr, $n:expr) => {
    let __real = $ctx.args.len() - 1;
    if __real != $n {
      return Err(format!(
        "'{}' expects {} argument{}, got {}",
        $ctx.name,
        $n,
        if $n == 1 { "" } else { "s" },
        __real
      ));
    }
  };
}

macro_rules! enforce_method_arg_range {
  ($ctx:expr, $min:expr, $max:expr) => {
    let __real = $ctx.args.len() - 1;
    #[allow(unused_comparisons)]
    if __real < $min || __real > $max {
      return Err(format!(
        "'{}' expects between {} and {} arguments, got {}",
        $ctx.name, $min, $max, __real
      ));
    }
  };
}

macro_rules! enforce_method_arg_type {
  ($ctx:expr, $idx:expr, $ty:expr) => {
    match $ctx.args.get($idx) {
      Some(v) if $crate::builtins::enforce::ArgType::matches($ty, *v) => {},
      Some(v) => {
        return Err(format!(
          "'{}' expects argument {} to be {}, got {}",
          $ctx.name,
          $idx,
          $crate::builtins::enforce::ArgType::label($ty),
          v.type_name()
        ));
      },
      None => {
        return Err(format!(
          "'{}' expects argument {} ({}), but it was not given",
          $ctx.name,
          $idx,
          $crate::builtins::enforce::ArgType::label($ty)
        ));
      },
    }
  };
}

macro_rules! enforce_method_arg_type_opt {
  ($ctx:expr, $idx:expr, $ty:expr) => {
    if let Some(v) = $ctx.args.get($idx) {
      if !$crate::builtins::enforce::ArgType::matches($ty, *v) {
        return Err(format!(
          "'{}' expects argument {} to be {}, got {}",
          $ctx.name,
          $idx,
          $crate::builtins::enforce::ArgType::label($ty),
          v.type_name()
        ));
      }
    }
  };
}

macro_rules! enforce_method_arg_types {
  ($ctx:expr, $($idx:expr => $ty:expr),+ $(,)?) => {
    $( enforce_method_arg_type!($ctx, $idx, $ty); )+
  };
}

macro_rules! enforce_method_arg_type_any_of {
  ($ctx:expr, $idx:expr, [$($ty:expr),+ $(,)?]) => {
    match $ctx.args.get($idx) {
      Some(v) if [$($ty),+].iter().any(|t| $crate::builtins::enforce::ArgType::matches(*t, *v)) => {},
      Some(v) => {
        return Err(format!(
          "'{}' expects argument {} to be {}, got {}",
          $ctx.name,
          $idx,
          $crate::builtins::enforce::describe_types(&[$($ty),+]),
          v.type_name()
        ));
      },
      None => {
        return Err(format!(
          "'{}' expects argument {} ({}), but it was not given",
          $ctx.name,
          $idx,
          $crate::builtins::enforce::describe_types(&[$($ty),+])
        ));
      },
    }
  };
}

macro_rules! enforce_method_arg_type_any_of_opt {
  ($ctx:expr, $idx:expr, [$($ty:expr),+ $(,)?]) => {
    if let Some(v) = $ctx.args.get($idx) {
      if ![$($ty),+].iter().any(|t| $crate::builtins::enforce::ArgType::matches(*t, *v)) {
        return Err(format!(
          "'{}' expects argument {} to be {}, got {}",
          $ctx.name,
          $idx,
          $crate::builtins::enforce::describe_types(&[$($ty),+]),
          v.type_name()
        ));
      }
    }
  };
}

macro_rules! enforce_method_arg_ptr {
  ($ctx:expr, $idx:expr, $tag:expr) => {
    match $ctx.args.get($idx) {
      Some(v) if v.is_ptr_type($tag) => {},
      Some(v) if v.is_ptr() => {
        return Err(format!(
          "'{}' expects argument {} to be a {}, got a {}",
          $ctx.name,
          $idx,
          $tag,
          v.ptr_type_name().unwrap_or("ptr")
        ));
      },
      Some(v) => {
        return Err(format!(
          "'{}' expects argument {} to be a {}, got {}",
          $ctx.name,
          $idx,
          $tag,
          v.type_name()
        ));
      },
      None => {
        return Err(format!(
          "'{}' expects argument {} (a {}), but it was not given",
          $ctx.name, $idx, $tag
        ));
      },
    }
  };
}

pub(crate) use enforce_method_arg_count;
pub(crate) use enforce_method_arg_ptr;
pub(crate) use enforce_method_arg_range;
pub(crate) use enforce_method_arg_type;
pub(crate) use enforce_method_arg_type_any_of;
pub(crate) use enforce_method_arg_type_any_of_opt;
pub(crate) use enforce_method_arg_type_opt;
pub(crate) use enforce_method_arg_types;
