#![allow(unused)]

use std::cmp::Ordering;
use std::sync::LazyLock;

use crate::{
  builtins::{
    MethodTable, build,
    enforce::{
      ArgType, enforce_method_arg_count, enforce_method_arg_range, enforce_method_arg_type,
      enforce_method_arg_type_opt,
    },
    method, method_n, method_opt, to_string,
  },
  vm::{
    object::{Obj, ZuriContext},
    value::Value,
  },
};

pub static LIST_METHODS: LazyLock<MethodTable> = LazyLock::new(|| {
  build(vec![
    // iterable protocol
    method_n("@key", 1, _key),
    method_n("@value", 1, _value),
    method("to_string", to_string),
    method("length", length),
    method_n("append", 1, append),
    method("clear", clear),
    method("clone", clone_list),
    method_n("count", 1, count),
    method_n("extend", 1, extend),
    method_opt("index_of", 1, index_of),
    method_n("insert", 2, insert),
    method("pop", pop),
    method_opt("shift", 0, shift),
    method_n("remove_at", 1, remove_at),
    method_n("remove", 1, remove),
    method("reverse", reverse),
    method("sort", sort),
    method_n("contains", 1, contains),
    method_n("delete", 2, delete),
    method("first", first),
    method("last", last),
    method("is_empty", is_empty),
    method_n("take", 1, take),
    method_opt("get", 1, get),
    method("compact", compact),
    method("unique", unique),
    method_opt("zip", 0, zip),
    method_n("zip_from", 1, zip_from),
    method("to_dict", to_dict),
    method_n("each", 1, each),
    method_n("map", 1, map_fn),
    method_n("filter", 1, filter),
    method_n("some", 1, some_fn),
    method_n("every", 1, every),
    method_opt("reduce", 1, reduce),
  ])
});

//-----------------------------------------------------------------------------------
// Helpers
//-----------------------------------------------------------------------------------

/// Run `f` with mutable access to the underlying `Vec<Value>` storage
/// of a list Value. Every caller here is only ever dispatched for a
/// list receiver (see `builtins::Kind::of`), so a non-list `v` would
/// be an internal bug, not a user-triggerable error.
fn with_list_mut<F, R>(v: Value, f: F) -> R
where
  F: FnOnce(&mut Vec<Value>) -> R,
{
  match unsafe { &*v.as_obj() } {
    Obj::List(items) => f(&mut items.borrow_mut()),
    _ => unreachable!("with_list_mut called on a non-list Value"),
  }
}

//-----------------------------------------------------------------------------------
// Implementations
//-----------------------------------------------------------------------------------

fn length(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  Ok(Value::number(ctx.args[0].list_len() as f64))
}

fn append(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  let item = ctx.args[1];
  with_list_mut(ctx.args[0], |v| v.push(item));
  Ok(Value::nil())
}

fn clear(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  with_list_mut(ctx.args[0], |v| v.clear());
  Ok(Value::nil())
}

fn clone_list(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  let items = ctx.args[0].as_list();
  Ok(ctx.vm.heap_mut().alloc_list(items))
}

fn count(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  let target = ctx.args[1];
  let n = ctx.args[0]
    .as_list()
    .iter()
    .filter(|v| v.equals(&target))
    .count();
  Ok(Value::number(n as f64))
}

fn extend(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  enforce_method_arg_type!(ctx, 1, ArgType::List);
  let other = ctx.args[1].as_list();
  with_list_mut(ctx.args[0], |v| v.extend(other));
  Ok(Value::nil())
}

fn index_of(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_range!(ctx, 1, 2);
  enforce_method_arg_type_opt!(ctx, 2, ArgType::Number);

  let target = ctx.args[1];
  let start = match ctx.args.get(2) {
    Some(v) => v.as_number().max(0.0) as usize,
    None => 0,
  };

  let items = ctx.args[0].as_list();
  if start >= items.len() {
    return Ok(Value::number(-1.0));
  }

  for (i, item) in items.iter().enumerate().skip(start) {
    if item.equals(&target) {
      return Ok(Value::number(i as f64));
    }
  }
  Ok(Value::number(-1.0))
}

fn insert(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 2);
  enforce_method_arg_type!(ctx, 2, ArgType::Number);

  let item = ctx.args[1];
  let index = ctx.args[2].as_number();
  if index < 0.0 {
    return Err(format!(
      "'{}' expects argument 2 to be a non-negative number, got {}",
      ctx.name, index
    ));
  }
  let index = index as usize;

  with_list_mut(ctx.args[0], |v| {
    if index >= v.len() {
      while v.len() < index {
        v.push(Value::nil());
      }
      v.push(item);
    } else {
      v.insert(index, item);
    }
  });
  Ok(Value::nil())
}

fn pop(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  Ok(with_list_mut(ctx.args[0], |v| {
    v.pop().unwrap_or(Value::nil())
  }))
}

/// `shift([count])` -- removes `count` items (default 1) from the
/// front. Per spec: if `count` exceeds the list's current length, the
/// ENTIRE list is cleared and `nil` is returned (not a partial/short
/// list of whatever happened to be available).
fn shift(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_range!(ctx, 0, 1);
  enforce_method_arg_type_opt!(ctx, 1, ArgType::Number);

  let count = match ctx.args.get(1) {
    Some(v) => v.as_number().max(0.0) as usize,
    None => 1,
  };

  let len = ctx.args[0].list_len();

  if count > len {
    with_list_mut(ctx.args[0], |v| v.clear());
    return Ok(Value::nil());
  }

  let removed: Vec<Value> = with_list_mut(ctx.args[0], |v| v.drain(0..count).collect());

  if removed.len() == 1 {
    return Ok(removed[0]);
  }
  Ok(ctx.vm.heap_mut().alloc_list(removed))
}

fn remove_at(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  enforce_method_arg_type!(ctx, 1, ArgType::Number);

  let idx = ctx.args[1].as_number();
  let len = ctx.args[0].list_len();
  if idx < 0.0 || idx as usize >= len {
    return Err(format!(
      "list index {} out of range at remove_at()",
      idx as i64
    ));
  }
  let idx = idx as usize;
  Ok(with_list_mut(ctx.args[0], |v| v.remove(idx)))
}

fn remove(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  let target = ctx.args[1];
  with_list_mut(ctx.args[0], |v| {
    if let Some(pos) = v.iter().position(|item| item.equals(&target)) {
      v.remove(pos);
    }
  });
  Ok(Value::nil())
}

/// Per spec this returns a NEW list in reverse order -- unlike `sort`,
/// it does not mutate the receiver in place.
fn reverse(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  let mut items = ctx.args[0].as_list();
  items.reverse();
  Ok(ctx.vm.heap_mut().alloc_list(items))
}

/// Precedence bucket for `sort()`, matching the documented ordering:
/// nil, boolean, numbers, strings, ranges, lists, dictionaries, bytes,
/// functions/classes -- anything else (instances, upvalues, ...) sorts
/// last. There is no `file` type in this VM, so that documented bucket
/// is simply absent here.
fn sort_rank(v: &Value) -> u8 {
  if v.is_nil() {
    0
  } else if v.is_bool() {
    1
  } else if v.is_number() {
    2
  } else if v.is_string() {
    3
  } else if v.is_range() {
    4
  } else if v.is_list() {
    5
  } else if v.is_dict() {
    6
  } else if v.is_bytes() {
    7
  } else if v.is_callable() {
    8
  } else {
    9
  }
}

fn compare_values(a: &Value, b: &Value) -> Ordering {
  let (ra, rb) = (sort_rank(a), sort_rank(b));
  if ra != rb {
    return ra.cmp(&rb);
  }
  if a.is_number() {
    return a
      .as_number()
      .partial_cmp(&b.as_number())
      .unwrap_or(Ordering::Equal);
  }
  if a.is_bool() {
    return a.as_bool().cmp(&b.as_bool());
  }
  if a.is_string() {
    return a.as_str().cmp(b.as_str());
  }
  if a.is_list() {
    let mut sa = a.as_list();
    let mut sb = b.as_list();
    sa.sort_by(compare_values);
    sb.sort_by(compare_values);
    for (x, y) in sa.iter().zip(sb.iter()) {
      let c = compare_values(x, y);
      if c != Ordering::Equal {
        return c;
      }
    }
    return sa.len().cmp(&sb.len());
  }
  Ordering::Equal
}

/// Sorts in-place and returns the (same) list -- also sorting any
/// directly-nested lists' own items, matching the documented example.
fn sort(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  with_list_mut(ctx.args[0], |v| {
    for item in v.iter() {
      if item.is_list() {
        let mut inner = item.as_list();
        inner.sort_by(compare_values);
        with_list_mut(*item, |iv| {
          iv.clear();
          iv.extend(inner);
        });
      }
    }
    v.sort_by(compare_values);
  });
  Ok(ctx.args[0])
}

fn contains(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  let target = ctx.args[1];
  let found = ctx.args[0].as_list().iter().any(|v| v.equals(&target));
  Ok(Value::bool(found))
}

fn delete(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 2);
  enforce_method_arg_type!(ctx, 1, ArgType::Number);
  enforce_method_arg_type!(ctx, 2, ArgType::Number);

  let lower = ctx.args[1].as_number();
  let upper = ctx.args[2].as_number();
  let len = ctx.args[0].list_len();

  if lower < 0.0 || upper < 0.0 || lower > upper || lower as usize >= len || upper as usize >= len {
    return Err(format!(
      "'{}' invalid range {}..{} for a list of length {}",
      ctx.name, lower, upper, len
    ));
  }

  let lo = lower as usize;
  let hi = upper as usize;
  let removed = with_list_mut(ctx.args[0], |v| v.drain(lo..=hi).count());
  Ok(Value::number(removed as f64))
}

fn first(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  Ok(ctx.args[0].list_get(0).unwrap_or(Value::nil()))
}

fn last(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  let len = ctx.args[0].list_len();
  if len == 0 {
    return Ok(Value::nil());
  }
  Ok(ctx.args[0].list_get(len - 1).unwrap_or(Value::nil()))
}

fn is_empty(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  Ok(Value::bool(ctx.args[0].list_len() == 0))
}

/// `take(n)` -- first `n` items, or the whole list (copied) if
/// `n >= length()`. For `n < 0`, per spec this is `length() + n`
/// items from the front (verified against the documented example:
/// an 11-element list, `take(-5)`, yields 6 elements).
fn take(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  enforce_method_arg_type!(ctx, 1, ArgType::Number);

  let items = ctx.args[0].as_list();
  let len = items.len() as i64;
  let n = ctx.args[1].as_number() as i64;

  let take_count = if n < 0 { (len + n).max(0) } else { n.min(len) };

  let result: Vec<Value> = items.into_iter().take(take_count as usize).collect();
  Ok(ctx.vm.heap_mut().alloc_list(result))
}

fn get(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_range!(ctx, 1, 2);
  enforce_method_arg_type!(ctx, 1, ArgType::Number);

  let idx = ctx.args[1].as_number();
  let len = ctx.args[0].list_len();
  if idx < 0.0 || idx as usize >= len {
    return if ctx.args.len() == 3 {
      Ok(ctx.args[2])
    } else {
      Err(format!("list index {} out of range at get()", idx as i64))
    };
  }
  Ok(ctx.args[0].list_get(idx as usize).unwrap())
}

fn compact(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  let items: Vec<Value> = ctx.args[0]
    .as_list()
    .into_iter()
    .filter(|v| !v.is_nil())
    .collect();
  Ok(ctx.vm.heap_mut().alloc_list(items))
}

fn unique(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  let mut result: Vec<Value> = Vec::new();
  for item in ctx.args[0].as_list() {
    if !result.iter().any(|v| v.equals(&item)) {
      result.push(item);
    }
  }
  Ok(ctx.vm.heap_mut().alloc_list(result))
}

/// `zip(items...: list...)` -- variadic list of other lists, merged
/// column-wise with the receiver. Missing entries (when another list
/// is shorter than the receiver) become `nil`, per spec.
fn zip(ctx: &mut ZuriContext) -> Result<Value, String> {
  for (i, v) in ctx.args.iter().enumerate().skip(1) {
    if !v.is_list() {
      return Err(format!(
        "'{}' expects argument {} to be a list, got {}",
        ctx.name,
        i,
        v.type_name()
      ));
    }
  }

  let base = ctx.args[0].as_list();
  let others: Vec<Vec<Value>> = ctx.args[1..].iter().map(|v| v.as_list()).collect();

  let mut result = Vec::with_capacity(base.len());
  for (i, item) in base.into_iter().enumerate() {
    let mut row = vec![item];
    for other in &others {
      row.push(other.get(i).copied().unwrap_or(Value::nil()));
    }
    result.push(ctx.vm.heap_mut().alloc_list(row));
  }
  Ok(ctx.vm.heap_mut().alloc_list(result))
}

/// Same as `zip`, but the "other lists" are given as a single list of
/// lists instead of separate variadic arguments.
fn zip_from(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  enforce_method_arg_type!(ctx, 1, ArgType::List);

  let base = ctx.args[0].as_list();
  let outer = ctx.args[1].as_list();

  let mut others: Vec<Vec<Value>> = Vec::with_capacity(outer.len());
  for v in &outer {
    if !v.is_list() {
      return Err(format!(
        "'{}' expects a list of lists, got {}",
        ctx.name,
        v.type_name()
      ));
    }
    others.push(v.as_list());
  }

  let mut result = Vec::with_capacity(base.len());
  for (i, item) in base.into_iter().enumerate() {
    let mut row = vec![item];
    for other in &others {
      row.push(other.get(i).copied().unwrap_or(Value::nil()));
    }
    result.push(ctx.vm.heap_mut().alloc_list(row));
  }
  Ok(ctx.vm.heap_mut().alloc_list(result))
}

fn to_dict(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  let items = ctx.args[0].as_list();
  let pairs: Vec<(Value, Value)> = items
    .into_iter()
    .enumerate()
    .map(|(i, v)| (Value::number(i as f64), v))
    .collect();
  Ok(ctx.vm.heap_mut().alloc_dict(pairs))
}

fn each(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  enforce_method_arg_type!(ctx, 1, ArgType::Function);

  let items = ctx.args[0].as_list();
  let callback = ctx.args[1];

  for (i, item) in items.into_iter().enumerate() {
    ctx
      .vm
      .call_value(callback, &[item, Value::number(i as f64)])
      .map_err(|e| ctx.vm.describe_exception(e))?;
  }

  Ok(ctx.args[0])
}

fn map_fn(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  enforce_method_arg_type!(ctx, 1, ArgType::Function);

  let items = ctx.args[0].as_list();
  let callback = ctx.args[1];

  let mut result = Vec::with_capacity(items.len());
  for (i, item) in items.into_iter().enumerate() {
    let mapped = ctx
      .vm
      .call_value(callback, &[item, Value::number(i as f64)])
      .map_err(|e| ctx.vm.describe_exception(e))?;
    result.push(mapped);
  }
  Ok(ctx.vm.heap_mut().alloc_list(result))
}

fn filter(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  enforce_method_arg_type!(ctx, 1, ArgType::Function);

  let items = ctx.args[0].as_list();
  let callback = ctx.args[1];

  let mut result = Vec::new();
  for (i, item) in items.into_iter().enumerate() {
    let keep = ctx
      .vm
      .call_value(callback, &[item, Value::number(i as f64)])
      .map_err(|e| ctx.vm.describe_exception(e))?;
    if !keep.is_falsey() {
      result.push(item);
    }
  }
  Ok(ctx.vm.heap_mut().alloc_list(result))
}

fn some_fn(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  enforce_method_arg_type!(ctx, 1, ArgType::Function);

  let items = ctx.args[0].as_list();
  let callback = ctx.args[1];

  for (i, item) in items.into_iter().enumerate() {
    let result = ctx
      .vm
      .call_value(callback, &[item, Value::number(i as f64)])
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

  let items = ctx.args[0].as_list();
  let callback = ctx.args[1];

  for (i, item) in items.into_iter().enumerate() {
    let result = ctx
      .vm
      .call_value(callback, &[item, Value::number(i as f64)])
      .map_err(|e| ctx.vm.describe_exception(e))?;
    if result.is_falsey() {
      return Ok(Value::bool(false));
    }
  }
  Ok(Value::bool(true))
}

fn reduce(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_range!(ctx, 1, 2);
  enforce_method_arg_type!(ctx, 1, ArgType::Function);

  let callback = ctx.args[1];
  let list_val = ctx.args[0];
  let items = ctx.args[0].as_list();

  let (mut acc, start_idx) = match ctx.args.get(2) {
    Some(&initial) => (initial, 0usize),
    None => {
      if items.is_empty() {
        return Ok(Value::nil());
      }
      (items[0], 1usize)
    },
  };

  for i in start_idx..items.len() {
    acc = ctx
      .vm
      .call_value(
        callback,
        &[acc, items[i], Value::number(i as f64), list_val],
      )
      .map_err(|e| ctx.vm.describe_exception(e))?;
  }

  Ok(acc)
}

//-----------------------------------------------------------------------------------
// Iterable Decorators (@key / @value)
//-----------------------------------------------------------------------------------

fn _key(ctx: &mut ZuriContext) -> Result<Value, String> {
  let val = ctx.args[1];
  let len = ctx.args[0].list_len();
  if val.is_nil() {
    if len == 0 {
      return Ok(Value::bool(false));
    }
    return Ok(Value::number(0.0));
  }
  if !val.is_number() {
    return Err(format!(
      "lists are numerically indexed, {} given",
      val.type_name()
    ));
  }
  let index = val.as_number() as usize;
  if len > 0 && index < len - 1 {
    return Ok(Value::number(index as f64 + 1.0));
  }
  Ok(Value::nil())
}

fn _value(ctx: &mut ZuriContext) -> Result<Value, String> {
  if !ctx.args[1].is_number() {
    return Err("lists are numerically indexed".to_string());
  }
  let index = ctx.args[1].as_number();
  let obj = ctx.args[0];
  if index > -1.0 && index < obj.list_len() as f64 {
    return Ok(obj.list_get(index as usize).unwrap());
  }
  Ok(Value::nil())
}
