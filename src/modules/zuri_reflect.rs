//! `_zuri_reflect` builtin module; the native backing for
//! `zuri.reflect`.
//!
//! Pure runtime introspection; everything here reads metadata the
//! compiler/VM already tracks on a live function, class, or module
//! object (name, arity, methods, fields, superclass, module members).
//! It deliberately does NOT know anything about source position or
//! doc comments; those are a `zuri.parse()` concern (see
//! `libs/zuri/ast.zu`), not a live-object one.

use crate::builtins::enforce::ArgType;
use crate::enforce_arg_count;
use crate::enforce_arg_type;
use crate::modules::{BuiltinModuleDef, native};
use crate::vm::object::ZuriContext;
use crate::vm::value::Value;
use crate::vm::vm::VM;

pub static MODULE: BuiltinModuleDef = BuiltinModuleDef {
  name: "_zuri_reflect",
  build,
};

fn build(vm: &mut VM) -> Vec<(&'static str, Value)> {
  vec![
    ("kind", native(vm, "kind", 1, false, kind_fn)),
    (
      "pointer_type",
      native(vm, "pointer_type", 1, false, pointer_type_fn),
    ),
    (
      "function_info",
      native(vm, "function_info", 1, false, function_info_fn),
    ),
    (
      "class_info",
      native(vm, "class_info", 1, false, class_info_fn),
    ),
    (
      "module_info",
      native(vm, "module_info", 1, false, module_info_fn),
    ),
    ("info", native(vm, "info", 1, false, info_fn)),
    ("has_prop", native(vm, "has_prop", 2, false, has_prop_fn)),
    ("get_prop", native(vm, "get_prop", 2, false, get_prop_fn)),
    ("get_props", native(vm, "get_props", 1, false, get_props_fn)),
    (
      "has_method",
      native(vm, "has_method", 2, false, has_method_fn),
    ),
    (
      "get_method",
      native(vm, "get_method", 2, false, get_method_fn),
    ),
    (
      "bind_method",
      native(vm, "bind_method", 2, false, bind_method_fn),
    ),
  ]
}

/// The class backing `object`, if it has one; an instance's own
/// class, or a class value used directly. `None` for anything else
/// (a module, a plain value, ...), which is what makes every
/// `has_method`/`get_method`/`bind_method` below a safe "no" for
/// non-instance/non-class input rather than needing its own type
/// check at every call site.
fn resolve_class_value(object: Value) -> Option<Value> {
  if object.is_instance() {
    Some(object.as_instance().class)
  } else if object.is_class() {
    Some(object)
  } else {
    None
  }
}

/// The module namespace backing `object`, if it has one; a raw
/// module, or the module a promoted `import PATH` binding wraps.
fn resolve_module_value(object: Value) -> Option<Value> {
  if object.is_module() {
    Some(object)
  } else if object.is_module_binding() {
    Some(object.as_module_binding().module)
  } else {
    None
  }
}

/// `_zuri_reflect.kind(value)`: this value's runtime type tag, one of
/// `"nil"`, `"bool"`, `"number"`, `"string"`, `"bytes"`, `"bigint"`,
/// `"list"`, `"dict"`, `"function"`, `"class"`, `"instance"`,
/// `"module"`, `"range"`, `"file"`, or a `Ptr`'s own registered tag
/// name. A closure, a native function, and a bound method are all
/// reported as `"function"`; from Zuri's own point of view they're
/// interchangeably callable, and `function_info` handles all three
/// uniformly.
fn kind_fn(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 1);
  let name = ctx.args[0].type_name();
  Ok(ctx.heap().alloc_string(name))
}

/// `_zuri_reflect.pointer_type(value)`: the registered resource tag a
/// `Ptr` value was allocated with (e.g. `"zuri::net::UdpStream"` for a
/// live `net.UdpSocket`'s internal handle), or `nil` if `value` isn't
/// a pointer at all.
///
/// `kind(value)` already reports this SAME tag for a pointer (it's
/// what makes a `Ptr`'s `kind` specific rather than a generic
/// `"pointer"` label); this exists as its own named accessor anyway
/// so code that specifically wants "is this a pointer, and if so what
/// kind" doesn't have to reach for the general-purpose `kind` and
/// remember that pointers are the one case where it returns something
/// more specific than a broad category. A pointer's wrapped Rust value
/// itself stays fully opaque either way; this only ever names what
/// kind of resource it is, never exposes its contents.
fn pointer_type_fn(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 1);
  match ctx.args[0].ptr_type_name() {
    Some(tag) => Ok(ctx.heap().alloc_string(tag)),
    None => Ok(Value::nil()),
  }
}

/// Builds `{ name, arity, variadic, is_method, owning_class_name,
/// source_path }` for a closure or bound method (backed by a real
/// `ObjFunction`), or the same shape with `owning_class_name`/
/// `source_path` left `nil` for a native function (which carries no
/// such metadata).
fn describe_function(ctx: &mut ZuriContext, value: Value) -> Result<Value, String> {
  let (name, arity, variadic, is_method, owning_class_name, source_path) = if value.is_closure() {
    let proto = value.as_closure().function;
    let f = proto.as_func();
    (
      f.name.clone(),
      f.arity,
      f.variadic,
      f.is_method,
      f.owning_class_name.clone(),
      Some(f.source_path.to_string()),
    )
  } else if value.is_bound_method() {
    return describe_function(ctx, value.as_bound_method().method);
  } else if value.is_native() {
    let n = value.as_native();
    (
      n.name.to_string(),
      n.min_arity,
      n.variadic,
      n.is_method,
      None,
      None,
    )
  } else {
    return Err(format!(
      "reflect.function_info() expects a function, got {}",
      value.type_name()
    ));
  };

  let name_key = ctx.heap().alloc_string("name");
  let name_value = ctx.heap().alloc_string(name);
  let arity_key = ctx.heap().alloc_string("arity");
  let variadic_key = ctx.heap().alloc_string("variadic");
  let is_method_key = ctx.heap().alloc_string("is_method");
  let owning_class_key = ctx.heap().alloc_string("owning_class_name");
  let owning_class_value = match owning_class_name {
    Some(n) => ctx.heap().alloc_string(n),
    None => Value::nil(),
  };
  let source_path_key = ctx.heap().alloc_string("source_path");
  let source_path_value = match source_path {
    Some(p) => ctx.heap().alloc_string(p),
    None => Value::nil(),
  };

  Ok(ctx.heap().alloc_dict(vec![
    (name_key, name_value),
    (arity_key, Value::number(arity as f64)),
    (variadic_key, Value::bool(variadic)),
    (is_method_key, Value::bool(is_method)),
    (owning_class_key, owning_class_value),
    (source_path_key, source_path_value),
  ]))
}

/// `_zuri_reflect.function_info(f)`: metadata for a function-like
/// value (a closure, a native, or a bound method); see
/// `describe_function` for the exact shape. Raises if `f` isn't
/// callable as a function (a class, while itself callable to
/// construct an instance, is a `class_info` subject instead).
fn function_info_fn(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 1);
  let value = ctx.args[0];
  describe_function(ctx, value)
}

/// `_zuri_reflect.class_info(c)`: `{ name, superclass_name, methods,
/// fields, statics }` for a class value.
///
/// `superclass_name` is `nil` for a class with no superclass.
/// `methods` is a dict from method name to that method's own
/// `function_info`-shaped entry (own + inherited, pre-merged, matching
/// how method lookup itself works: an override shadows the inherited
/// entry of the same name rather than both appearing). `fields` and
/// `statics` are plain lists of names, ordered by declaration (their
/// underlying slot index); own + inherited for `fields`, own-only
/// for `statics` (statics are deliberately not inherited by this
/// runtime; see `ObjClass`'s own docs).
fn class_info_fn(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 1);
  let value = ctx.args[0];

  if !value.is_class() {
    return Err(format!(
      "reflect.class_info() expects a class, got {}",
      value.type_name()
    ));
  }

  let (name, superclass_name, method_entries, mut field_entries, mut static_entries) = {
    let class = value.as_class();

    let superclass_name = class.superclass.map(|sc| sc.as_class().name.clone());

    let method_entries: Vec<(String, Value)> =
      class.methods.iter().map(|(k, v)| (k.clone(), *v)).collect();

    let field_entries: Vec<(String, u16)> = class
      .field_slots
      .iter()
      .map(|(k, &slot)| (k.clone(), slot))
      .collect();

    let static_entries: Vec<(String, u16)> = class
      .static_slots
      .iter()
      .map(|(k, &slot)| (k.clone(), slot))
      .collect();

    (
      class.name.clone(),
      superclass_name,
      method_entries,
      field_entries,
      static_entries,
    )
  };

  field_entries.sort_by_key(|(_, slot)| *slot);
  static_entries.sort_by_key(|(_, slot)| *slot);

  let mut method_pairs = Vec::with_capacity(method_entries.len());
  for (name, method_value) in method_entries {
    let key = ctx.heap().alloc_string(name);
    let info = describe_function(ctx, method_value)?;
    method_pairs.push((key, info));
  }
  let methods_value = ctx.heap().alloc_dict(method_pairs);

  let fields_value = {
    let items: Vec<Value> = field_entries
      .into_iter()
      .map(|(name, _)| ctx.heap().alloc_string(name))
      .collect();
    ctx.heap().alloc_list(items)
  };

  let statics_value = {
    let items: Vec<Value> = static_entries
      .into_iter()
      .map(|(name, _)| ctx.heap().alloc_string(name))
      .collect();
    ctx.heap().alloc_list(items)
  };

  let name_key = ctx.heap().alloc_string("name");
  let name_value = ctx.heap().alloc_string(name);
  let superclass_key = ctx.heap().alloc_string("superclass_name");
  let superclass_value = match superclass_name {
    Some(n) => ctx.heap().alloc_string(n),
    None => Value::nil(),
  };
  let methods_key = ctx.heap().alloc_string("methods");
  let fields_key = ctx.heap().alloc_string("fields");
  let statics_key = ctx.heap().alloc_string("statics");

  Ok(ctx.heap().alloc_dict(vec![
    (name_key, name_value),
    (superclass_key, superclass_value),
    (methods_key, methods_value),
    (fields_key, fields_value),
    (statics_key, statics_value),
  ]))
}

/// `_zuri_reflect.module_info(m)`: `{ name, path, loaded, members }`
/// for a module value. `members` is a dict from every top-level name
/// the module's namespace declares to that binding's own `kind`
/// string (see `kind_fn`); e.g. `{ add: "function", VERSION:
/// "string" }`. Accepts either a raw module (`Obj::Module`, what
/// `import PATH { * }` and `import PATH.member` both resolve through)
/// or a promoted binding (`Obj::ModuleBinding`, what a plain `import
/// PATH` binds its local name to); both describe the same
/// underlying module.
fn module_info_fn(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 1);
  let mut value = ctx.args[0];

  if value.is_module_binding() {
    value = value.as_module_binding().module;
  }

  if !value.is_module() {
    return Err(format!(
      "reflect.module_info() expects a module, got {}",
      ctx.args[0].type_name()
    ));
  }

  let (name, path, loaded, member_entries) = {
    let module = value.as_module();
    let member_entries: Vec<(String, Value)> = module
      .namespace
      .names
      .keys()
      .map(|k| (k.clone(), module.namespace.get(k).unwrap_or(Value::nil())))
      .collect();
    (
      module.name.clone(),
      module.path.clone(),
      module.loaded,
      member_entries,
    )
  };

  let mut member_pairs = Vec::with_capacity(member_entries.len());
  for (name, member_value) in member_entries {
    let key = ctx.heap().alloc_string(name);
    let kind = ctx.heap().alloc_string(member_value.type_name());
    member_pairs.push((key, kind));
  }
  let members_value = ctx.heap().alloc_dict(member_pairs);

  let name_key = ctx.heap().alloc_string("name");
  let name_value = ctx.heap().alloc_string(name);
  let path_key = ctx.heap().alloc_string("path");
  let path_value = ctx.heap().alloc_string(path);
  let loaded_key = ctx.heap().alloc_string("loaded");
  let members_key = ctx.heap().alloc_string("members");

  Ok(ctx.heap().alloc_dict(vec![
    (name_key, name_value),
    (path_key, path_value),
    (loaded_key, Value::bool(loaded)),
    (members_key, members_value),
  ]))
}

/// `_zuri_reflect.info(value)`: dispatches to `function_info`/
/// `class_info`/`module_info` based on `kind(value)`. Raises for a
/// value with no reflectable shape (a number, a list, an instance,
/// ...); `reflect` describes callables, classes, and modules only.
fn info_fn(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 1);
  let value = ctx.args[0];

  if value.is_module() || value.is_module_binding() {
    module_info_fn(ctx)
  } else if value.is_class() {
    class_info_fn(ctx)
  } else if value.is_closure() || value.is_native() || value.is_bound_method() {
    function_info_fn(ctx)
  } else {
    Err(format!(
      "reflect.info() does not support values of type '{}'",
      value.type_name()
    ))
  }
}

// Instance/module property access, and instance/class method lookup.
//
// `set_prop`/`del_prop` themselves aren't implemented here at all:
// `src/vm/natives.rs` already registers GLOBAL `setprop`/`delprop`
// (alongside `getprop`/`hasprop`) that do exactly what's needed for
// the instance case, write barrier included; `libs/zuri/reflect.zu`
// calls straight through to those rather than duplicating field-slot
// lookup and mutation logic here a second time. `has_prop`/`get_prop`/
// `get_props` below still get their own natives because they ALSO
// need to work on a module (`getprop`/`hasprop` only accept an
// instance), which nothing existing covers.

/// `_zuri_reflect.has_prop(object, name)`: does `object` (an instance
/// or a module) have a property/member named `name`? `false` for
/// anything else, or for a name the object doesn't have; never
/// raises.
fn has_prop_fn(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 2);
  enforce_arg_type!(ctx, 1, ArgType::String);
  let object = ctx.args[0];
  let name = ctx.args[1].as_str();

  let found = if object.is_instance() {
    let inst = object.as_instance();
    inst.class.as_class().field_slots.contains_key(name)
  } else if let Some(module) = resolve_module_value(object) {
    module.as_module().namespace.names.contains_key(name)
  } else {
    false
  };

  Ok(Value::bool(found))
}

/// `_zuri_reflect.get_prop(object, name)`: the current value of
/// `object`'s (an instance or a module) property/member named `name`,
/// or `nil` if it has none by that name.
fn get_prop_fn(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 2);
  enforce_arg_type!(ctx, 1, ArgType::String);
  let object = ctx.args[0];
  let name = ctx.args[1].as_str();

  if object.is_instance() {
    let inst = object.as_instance();
    let slot = inst.class.as_class().field_slots.get(name).copied();
    match slot {
      Some(slot) => Ok(inst.fields[slot as usize].get()),
      None => Ok(Value::nil()),
    }
  } else if let Some(module) = resolve_module_value(object) {
    Ok(
      module
        .as_module()
        .namespace
        .get(name)
        .unwrap_or(Value::nil()),
    )
  } else {
    Ok(Value::nil())
  }
}

/// `_zuri_reflect.get_props(object)`: every property/member name
/// `object` (an instance or a module) has, as a list of strings.
/// Instance fields are ordered by declaration (their underlying slot
/// index); module members have no such inherent order, so their
/// ordering here isn't guaranteed. An empty list for anything else.
fn get_props_fn(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 1);
  let object = ctx.args[0];

  let names: Vec<String> = if object.is_instance() {
    let inst = object.as_instance();
    let class = inst.class.as_class();
    let mut entries: Vec<(String, u16)> = class
      .field_slots
      .iter()
      .map(|(k, &slot)| (k.clone(), slot))
      .collect();
    entries.sort_by_key(|(_, slot)| *slot);
    entries.into_iter().map(|(name, _)| name).collect()
  } else if let Some(module) = resolve_module_value(object) {
    module.as_module().namespace.names.keys().cloned().collect()
  } else {
    Vec::new()
  };

  let mut items = Vec::with_capacity(names.len());
  for name in names {
    items.push(ctx.heap().alloc_string(name));
  }
  Ok(ctx.heap().alloc_list(items))
}

/// `_zuri_reflect.has_method(object, name)`: does the class behind
/// `object` (an instance, or a class used directly) declare or inherit
/// a method named `name`? `false` for anything else.
fn has_method_fn(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 2);
  enforce_arg_type!(ctx, 1, ArgType::String);
  let object = ctx.args[0];
  let name = ctx.args[1].as_str();

  let found = match resolve_class_value(object) {
    Some(class_val) => class_val.as_class().methods.contains_key(name),
    None => false,
  };
  Ok(Value::bool(found))
}

/// `_zuri_reflect.get_method(object, name)`: the raw (unbound) closure
/// for method `name` on the class behind `object`, or `nil` if it
/// declares no such method. Calling the result directly does NOT
/// supply a receiver; see `bind_method` for a version that
/// does.
fn get_method_fn(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 2);
  enforce_arg_type!(ctx, 1, ArgType::String);
  let object = ctx.args[0];
  let name = ctx.args[1].as_str();

  let method = match resolve_class_value(object) {
    Some(class_val) => class_val.as_class().methods.get(name).copied(),
    None => None,
  };
  Ok(method.unwrap_or(Value::nil()))
}

/// `_zuri_reflect.bind_method(object, name)`: method `name` on
/// `object`'s class, bound to `object` itself as the receiver; the
/// result can be called directly with no receiver argument, unlike
/// `get_method`'s raw closure. `nil` if the class declares no such
/// method.
fn bind_method_fn(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 2);
  enforce_arg_type!(ctx, 1, ArgType::String);
  let object = ctx.args[0];
  let name = ctx.args[1].as_str();

  let method = match resolve_class_value(object) {
    Some(class_val) => class_val.as_class().methods.get(name).copied(),
    None => None,
  };
  match method {
    Some(m) => Ok(ctx.heap().alloc_bound_method(object, m)),
    None => Ok(Value::nil()),
  }
}
