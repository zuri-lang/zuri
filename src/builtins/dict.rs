#![allow(unused)]

use std::{ops::Index, sync::LazyLock};

use crate::{
  builtins::{
    MethodTable, build,
    enforce::{
      ArgType, enforce_method_arg_count, enforce_method_arg_range, enforce_method_arg_type,
    },
    method, method_n, method_opt, to_string,
  },
  vm::{
    object::{DictStorage, Obj, ZuriContext},
    value::Value,
  },
};

pub static DICT_METHODS: LazyLock<MethodTable> = LazyLock::new(|| {
  build(vec![
    // iterable protocol
    method_n("@key", 1, _key),
    method_n("@value", 1, _value),
    method("to_string", to_string),
    method("length", length),
    method_n("add", 2, add),
    method_n("set", 2, set),
    method("clear", clear),
    method("clone", clone_dict),
    method("compact", compact),
    method_n("contains", 1, contains),
    method_n("extend", 1, extend),
    method_opt("get", 1, get),
    method("keys", keys),
    method("values", values),
    method_n("remove", 1, remove),
    method("is_empty", is_empty),
    method_n("find_key", 1, find_key),
    method("to_list", to_list),
    method_n("each", 1, each),
    method_n("filter", 1, filter),
    method_n("some", 1, some_fn),
    method_n("every", 1, every),
    method_opt("reduce", 1, reduce),
  ])
});

//-----------------------------------------------------------------------------------
// Helpers
//-----------------------------------------------------------------------------------

/// Run `f` with mutable access to the underlying `DictStorage` of a
/// dict Value -- the `Obj::Dict` counterpart to `list.rs`'s
/// `with_list_mut`.
fn with_dict_mut<F, R>(v: Value, f: F) -> R
where
  F: FnOnce(&mut DictStorage) -> R,
{
  match unsafe { &*v.as_obj() } {
    Obj::Dict(storage) => f(&mut storage.borrow_mut()),
    _ => unreachable!("with_dict_mut called on a non-dict Value"),
  }
}

/// Recursively clones List/Dict CONTENTS (not just their top-level
/// container), so `.clone()` really is the documented "deep copy" --
/// a nested list/dict inside the original can be mutated afterward
/// without the clone seeing it. Every other kind of Value (numbers,
/// strings, instances, closures, ...) is left as-is: strings are
/// already immutable, and instances/closures are reference types
/// throughout the rest of this VM (e.g. `list.clone()` doesn't deep-
/// copy instance elements either), so there's nothing further to copy
/// for those.
fn deep_clone(ctx: &mut ZuriContext, v: Value) -> Value {
  if v.is_list() {
    let items = v.as_list();
    let cloned: Vec<Value> = items
      .into_iter()
      .map(|item| deep_clone(ctx, item))
      .collect();
    ctx.vm.heap_mut().alloc_list(cloned)
  } else if v.is_dict() {
    let pairs = v.as_dict();
    let cloned: Vec<(Value, Value)> = pairs
      .into_iter()
      .map(|(k, val)| (deep_clone(ctx, k), deep_clone(ctx, val)))
      .collect();
    ctx.vm.heap_mut().alloc_dict(cloned)
  } else {
    v
  }
}

//-----------------------------------------------------------------------------------
// Implementations
//-----------------------------------------------------------------------------------

fn length(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  Ok(Value::number(ctx.args[0].dict_len() as f64))
}

fn add(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 2);
  let key = ctx.args[1];
  let value = ctx.args[2];
  with_dict_mut(ctx.args[0], |s| s.set(key, value));
  Ok(Value::nil())
}

fn set(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 2);
  let key = ctx.args[1];
  let value = ctx.args[2];
  with_dict_mut(ctx.args[0], |s| s.set(key, value));
  Ok(Value::nil())
}

fn clear(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  with_dict_mut(ctx.args[0], |s| *s = DictStorage::new());
  Ok(Value::nil())
}

fn clone_dict(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  let pairs = ctx.args[0].as_dict();
  let cloned: Vec<(Value, Value)> = pairs
    .into_iter()
    .map(|(k, v)| (deep_clone(ctx, k), deep_clone(ctx, v)))
    .collect();
  Ok(ctx.vm.heap_mut().alloc_dict(cloned))
}

fn compact(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  let pairs: Vec<(Value, Value)> = ctx.args[0]
    .as_dict()
    .into_iter()
    .filter(|(_, v)| !v.is_nil())
    .collect();
  Ok(ctx.vm.heap_mut().alloc_dict(pairs))
}

fn contains(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  let target = ctx.args[1];
  let found = ctx.args[0].as_dict().iter().any(|(k, _)| k.equals(&target));
  Ok(Value::bool(found))
}

/// Adds all key-value pairs from `x` into this dict, in-place --
/// mirrors `list.extend()`'s in-place mutation.
fn extend(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  enforce_method_arg_type!(ctx, 1, ArgType::Dict);
  let other = ctx.args[1].as_dict();
  with_dict_mut(ctx.args[0], |s| {
    for (k, v) in other {
      s.set(k, v);
    }
  });
  Ok(Value::nil())
}

fn get(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_range!(ctx, 1, 2);
  let key = ctx.args[1];
  match ctx.args[0].dict_get(&key) {
    Some(v) => Ok(v),
    None => Ok(ctx.args.get(2).copied().unwrap_or(Value::nil())),
  }
}

fn keys(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  let ks: Vec<Value> = ctx.args[0].as_dict().into_iter().map(|(k, _)| k).collect();
  Ok(ctx.vm.heap_mut().alloc_list(ks))
}

fn values(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  let vs: Vec<Value> = ctx.args[0].as_dict().into_iter().map(|(_, v)| v).collect();
  Ok(ctx.vm.heap_mut().alloc_list(vs))
}

fn remove(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  let key = ctx.args[1];
  let existing = ctx.args[0].dict_get(&key);
  if existing.is_some() {
    let remaining: Vec<(Value, Value)> = ctx.args[0]
      .as_dict()
      .into_iter()
      .filter(|(k, _)| !k.equals(&key))
      .collect();
    with_dict_mut(ctx.args[0], |s| *s = DictStorage::from_pairs(remaining));
  }
  Ok(existing.unwrap_or(Value::nil()))
}

fn is_empty(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  Ok(Value::bool(ctx.args[0].dict_len() == 0))
}

fn find_key(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  let target = ctx.args[1];
  for (k, v) in ctx.args[0].as_dict() {
    if v.equals(&target) {
      return Ok(k);
    }
  }
  Ok(Value::nil())
}

fn to_list(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  let pairs = ctx.args[0].as_dict();
  let keys: Vec<Value> = pairs.iter().map(|(k, _)| *k).collect();
  let values: Vec<Value> = pairs.iter().map(|(_, v)| *v).collect();
  let keys_list = ctx.vm.heap_mut().alloc_list(keys);
  let values_list = ctx.vm.heap_mut().alloc_list(values);
  Ok(ctx.vm.heap_mut().alloc_list(vec![keys_list, values_list]))
}

fn each(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  enforce_method_arg_type!(ctx, 1, ArgType::Function);

  let pairs = ctx.args[0].as_dict();
  let callback = ctx.args[1];

  for (k, v) in pairs {
    ctx
      .vm
      .call_value(callback, &[v, k])
      .map_err(|e| ctx.vm.describe_exception(e))?;
  }

  Ok(ctx.args[0])
}

fn filter(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  enforce_method_arg_type!(ctx, 1, ArgType::Function);

  let pairs = ctx.args[0].as_dict();
  let callback = ctx.args[1];

  let mut kept = Vec::new();
  for (k, v) in pairs {
    let keep = ctx
      .vm
      .call_value(callback, &[v, k])
      .map_err(|e| ctx.vm.describe_exception(e))?;
    if !keep.is_falsey() {
      kept.push((k, v));
    }
  }
  Ok(ctx.vm.heap_mut().alloc_dict(kept))
}

fn some_fn(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  enforce_method_arg_type!(ctx, 1, ArgType::Function);

  let pairs = ctx.args[0].as_dict();
  let callback = ctx.args[1];

  for (k, v) in pairs {
    let result = ctx
      .vm
      .call_value(callback, &[v, k])
      .map_err(|e| ctx.vm.describe_exception(e))?;
    if !result.is_falsey() {
      return Ok(Value::bool(true));
    }
  }
  Ok(Value::bool(false))
}

fn every(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  enforce_method_arg_type!(ctx, 1, ArgType::Function);

  let pairs = ctx.args[0].as_dict();
  let callback = ctx.args[1];

  for (k, v) in pairs {
    let result = ctx
      .vm
      .call_value(callback, &[v, k])
      .map_err(|e| ctx.vm.describe_exception(e))?;
    if result.is_falsey() {
      return Ok(Value::bool(false));
    }
  }
  Ok(Value::bool(true))
}

/// Same shape as `list.reduce()`, but keys stand in for indices, per
/// spec ("For dictionaries, keys are used instead of indices").
fn reduce(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_range!(ctx, 1, 2);
  enforce_method_arg_type!(ctx, 1, ArgType::Function);

  let callback = ctx.args[1];
  let dict_val = ctx.args[0];
  let pairs = ctx.args[0].as_dict();

  let (mut acc, start_idx) = match ctx.args.get(2) {
    Some(&initial) => (initial, 0usize),
    None => {
      if pairs.is_empty() {
        return Ok(Value::nil());
      }
      (pairs[0].1, 1usize)
    },
  };

  for i in start_idx..pairs.len() {
    let (k, v) = pairs[i];
    acc = ctx
      .vm
      .call_value(callback, &[acc, v, k, dict_val])
      .map_err(|e| ctx.vm.describe_exception(e))?;
  }

  Ok(acc)
}

//-----------------------------------------------------------------------------------
// Iterable Decorators (@key / @value) -- unchanged from before this pass
//-----------------------------------------------------------------------------------

fn _key(ctx: &mut ZuriContext) -> Result<Value, String> {
  let val = ctx.args[1];
  let dict = ctx.args[0];
  let len = dict.dict_len();

  if val.is_nil() {
    if len == 0 {
      return Ok(Value::bool(false));
    }
    return Ok(dict.dict_key_at(0).unwrap());
  }

  if let Some(index) = dict.dict_index_of(&val) {
    if index < len - 1 {
      return Ok(dict.dict_key_at(index + 1).unwrap());
    }
  }

  Ok(Value::nil())
}

fn _value(ctx: &mut ZuriContext) -> Result<Value, String> {
  let key = ctx.args[1];
  let dict = ctx.args[0];

  if let Some(index) = dict.dict_index_of(&key) {
    return Ok(dict.dict_value_at(index).unwrap());
  }

  Ok(Value::nil())
}
