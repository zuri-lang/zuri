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
    object::{DictStorage, Obj, ZuriContext, write_barrier},
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

/// Run `f` with mutable access to the underlying `DictStorage` of a
/// dict Value; the `Obj::Dict` counterpart to `list.rs`'s
/// `with_list_mut`.
fn with_dict_mut<F, R>(v: Value, f: F) -> R
where
  F: FnOnce(&mut DictStorage) -> R,
{
  let result = match unsafe { &*v.as_obj() } {
    Obj::Dict(storage) => f(&mut storage.borrow_mut()),
    _ => unreachable!("with_dict_mut called on a non-dict Value"),
  };
  // Coarse and unconditional: see `write_barrier`'s own docs, and
  // `list.rs`'s `with_list_mut` (its exact counterpart).
  write_barrier(v.as_obj());
  result
}

/// Recursively clones List/Dict CONTENTS (not just their top-level
/// container), so `.clone()` really is the documented "deep copy";
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
  let pairs: Vec<(Value, Value)> = ctx.args[0].with_dict(|s| {
    s.entries
      .iter()
      .filter(|(_, v)| !v.is_nil())
      .copied()
      .collect()
  });
  Ok(ctx.vm.heap_mut().alloc_dict(pairs))
}

fn contains(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  let target = ctx.args[1];
  Ok(Value::bool(ctx.args[0].dict_get(&target).is_some()))
}

/// Adds all key-value pairs from `x` into this dict, in-place;
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
  let ks: Vec<Value> = ctx.args[0].with_dict(|s| s.entries.iter().map(|(k, _)| *k).collect());
  Ok(ctx.vm.heap_mut().alloc_list(ks))
}

fn values(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  let vs: Vec<Value> = ctx.args[0].with_dict(|s| s.entries.iter().map(|(_, v)| *v).collect());
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
  let found = ctx.args[0].with_dict(|s| {
    s.entries
      .iter()
      .find(|(_, v)| v.equals(&target))
      .map(|(k, _)| *k)
  });
  Ok(found.unwrap_or(Value::nil()))
}

fn to_list(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  let (keys, values) = ctx.args[0].with_dict(|s| {
    let keys: Vec<Value> = s.entries.iter().map(|(k, _)| *k).collect();
    let values: Vec<Value> = s.entries.iter().map(|(_, v)| *v).collect();
    (keys, values)
  });
  let keys_list = ctx.vm.heap_mut().alloc_list(keys);
  let values_list = ctx.vm.heap_mut().alloc_list(values);
  Ok(ctx.vm.heap_mut().alloc_list(vec![keys_list, values_list]))
}

/// `list.rs`'s `pin_each_call` counterpart for a dict's (key, value)
/// pairs: see its own docs for why every read from `gc_pins` here
/// must be fresh, never cached across a `call_value`. Layout: `mark`
/// = `dict_val`, `mark + 1` = `callback`, `mark + 2 + 2*i` = key `i`,
/// `mark + 3 + 2*i` = value `i`.
fn pin_each_call(ctx: &mut ZuriContext, dict_val: Value, callback: Value) -> (usize, usize) {
  let pairs = dict_val.as_dict();
  let count = pairs.len();
  let mark = ctx.vm.pin_values(
    std::iter::once(dict_val)
      .chain(std::iter::once(callback))
      .chain(pairs.into_iter().flat_map(|(k, v)| [k, v])),
  );
  (mark, count)
}

fn each(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  enforce_method_arg_type!(ctx, 1, ArgType::Function);

  let (mark, count) = pin_each_call(ctx, ctx.args[0], ctx.args[1]);

  for i in 0..count {
    let callback = ctx.vm.pinned(mark + 1);
    let k = ctx.vm.pinned(mark + 2 + 2 * i);
    let v = ctx.vm.pinned(mark + 3 + 2 * i);
    ctx
      .vm
      .call_value(callback, &[v, k])
      .map_err(|e| ctx.vm.rethrow(e))?;
  }

  let dict_val = ctx.vm.pinned(mark);
  ctx.vm.unpin(mark);
  Ok(dict_val)
}

fn filter(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  enforce_method_arg_type!(ctx, 1, ArgType::Function);

  let (mark, count) = pin_each_call(ctx, ctx.args[0], ctx.args[1]);

  // Pair INDICES, not the pairs themselves. A `(Value, Value)` pushed
  // here would be re-read fresh at the moment it was kept and then go
  // stale on the next iteration's own collection, since a plain Vec is
  // not a GC root; the pinned slots stay correct throughout.
  let mut kept = Vec::new();
  for i in 0..count {
    let callback = ctx.vm.pinned(mark + 1);
    let k = ctx.vm.pinned(mark + 2 + 2 * i);
    let v = ctx.vm.pinned(mark + 3 + 2 * i);
    let keep = ctx
      .vm
      .call_value(callback, &[v, k])
      .map_err(|e| ctx.vm.rethrow(e))?;
    if !keep.is_falsey() {
      kept.push(i);
    }
  }

  let pairs: Vec<(Value, Value)> = kept
    .into_iter()
    .map(|i| {
      (
        ctx.vm.pinned(mark + 2 + 2 * i),
        ctx.vm.pinned(mark + 3 + 2 * i),
      )
    })
    .collect();
  ctx.vm.unpin(mark);
  Ok(ctx.vm.heap_mut().alloc_dict(pairs))
}

fn some_fn(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  enforce_method_arg_type!(ctx, 1, ArgType::Function);

  let (mark, count) = pin_each_call(ctx, ctx.args[0], ctx.args[1]);

  for i in 0..count {
    let callback = ctx.vm.pinned(mark + 1);
    let k = ctx.vm.pinned(mark + 2 + 2 * i);
    let v = ctx.vm.pinned(mark + 3 + 2 * i);
    let result = ctx
      .vm
      .call_value(callback, &[v, k])
      .map_err(|e| ctx.vm.rethrow(e))?;
    if !result.is_falsey() {
      ctx.vm.unpin(mark);
      return Ok(Value::bool(true));
    }
  }
  ctx.vm.unpin(mark);
  Ok(Value::bool(false))
}

fn every(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  enforce_method_arg_type!(ctx, 1, ArgType::Function);

  let (mark, count) = pin_each_call(ctx, ctx.args[0], ctx.args[1]);

  for i in 0..count {
    let callback = ctx.vm.pinned(mark + 1);
    let k = ctx.vm.pinned(mark + 2 + 2 * i);
    let v = ctx.vm.pinned(mark + 3 + 2 * i);
    let result = ctx
      .vm
      .call_value(callback, &[v, k])
      .map_err(|e| ctx.vm.rethrow(e))?;
    if result.is_falsey() {
      ctx.vm.unpin(mark);
      return Ok(Value::bool(false));
    }
  }
  ctx.vm.unpin(mark);
  Ok(Value::bool(true))
}

/// Same shape as `list.reduce()`, but keys stand in for indices, per
/// spec ("For dictionaries, keys are used instead of indices").
fn reduce(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_range!(ctx, 1, 2);
  enforce_method_arg_type!(ctx, 1, ArgType::Function);

  let (mark, count) = pin_each_call(ctx, ctx.args[0], ctx.args[1]);
  let initial_idx = ctx.args.get(2).map(|&v| ctx.vm.pin_values([v]));

  let (mut acc, start_idx) = match initial_idx {
    Some(idx) => (ctx.vm.pinned(idx), 0usize),
    None => {
      if count == 0 {
        ctx.vm.unpin(mark);
        return Ok(Value::nil());
      }
      (ctx.vm.pinned(mark + 3), 1usize)
    },
  };

  for i in start_idx..count {
    let callback = ctx.vm.pinned(mark + 1);
    let k = ctx.vm.pinned(mark + 2 + 2 * i);
    let v = ctx.vm.pinned(mark + 3 + 2 * i);
    let dict_val = ctx.vm.pinned(mark);
    acc = ctx
      .vm
      .call_value(callback, &[acc, v, k, dict_val])
      .map_err(|e| ctx.vm.rethrow(e))?;
  }

  ctx.vm.unpin(mark);
  Ok(acc)
}

// @key / @value: iterable protocol decorators.

fn _key(ctx: &mut ZuriContext) -> Result<Value, String> {
  let val = ctx.args[1];
  let dict = ctx.args[0];
  let len = dict.dict_len();

  if len == 0 {
    return Ok(Value::nil());
  }

  if val.is_nil() {
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
