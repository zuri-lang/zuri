//! `_zuri_compile` builtin module; the native backing for
//! `zuri.compile()`.
//!
//! Runs the real `Lexer` -> `Parser` -> `Compiler` pipeline (the exact
//! same one `import`/the REPL/`run_file` use, just stopped one step
//! short of actually executing the result) and converts the top-level
//! script's `Chunk::code` into a flat list of instruction nodes, each
//! shaped `{ op, line, fields }`. `libs/zuri/compile.zu` wraps each
//! dict into a proper `Instr` instance.
//!
//! An instruction whose fields are an index into a side table
//! (`Chunk::constants`, `Chunk::jump_tables`, `Chunk::param_checks`)
//! carries the RESOLVED value inline, alongside the raw index, so
//! nothing here requires a caller to also fetch and correlate a
//! separate constants/tables array just to make sense of one
//! instruction. A `LoadConst` that loads a nested function prototype
//! (from a `Closure` instruction's own constant) recurses into the
//! very same conversion for that function's own chunk, so a script
//! with functions/methods in it exposes their bodies too, not just the
//! top-level code around them.

use rustc_hash::FxHashMap;

use crate::builtins::enforce::ArgType;
use crate::compiler::compiler::Compiler;
use crate::compiler::lexer::Lexer;
use crate::compiler::parser::Parser;
use crate::enforce_arg_count;
use crate::enforce_arg_type;
use crate::modules::{BuiltinModuleDef, native};
use crate::vm::chunk::{Chunk, Instr, JumpKey, ParamType, ParamTypeCheck};
use crate::vm::object::{ObjFunction, ZuriContext};
use crate::vm::value::Value;
use crate::vm::vm::VM;

pub static MODULE: BuiltinModuleDef = BuiltinModuleDef {
  name: "_zuri_compile",
  build,
};

fn build(vm: &mut VM) -> Vec<(&'static str, Value)> {
  vec![("compile", native(vm, "compile", 1, false, compile_fn))]
}

/// `_zuri_compile.compile(source)`: compiles `source` as a standalone
/// script and returns its instructions as a flat list; see this
/// module's own doc comment for exactly what each instruction node
/// carries. Raises on a lexer/parser/compiler error, same as
/// `zuri.parse()`; there is no partial bytecode to hand back for
/// source that didn't compile.
fn compile_fn(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 1);
  enforce_arg_type!(ctx, 0, ArgType::String);

  let source = ctx.args[0].as_str().to_string();
  let mut lexer = Lexer::new(&source);
  let mut parser = Parser::new(&mut lexer);

  let decls = match parser.parse() {
    Ok(decls) => decls,
    Err(errors) => {
      let msg = errors
        .iter()
        .map(|e| e.to_string())
        .collect::<Vec<_>>()
        .join("\n  ");
      return Err(format!("failed to parse source:\n  {}", msg));
    },
  };

  let chunk = Box::new(Chunk::new());
  let compiler = Compiler::new(decls, chunk, ctx.heap(), std::rc::Rc::from("<zuri.compile>"));

  let top_fn = match compiler.compile() {
    Ok(f) => f,
    Err(errors) => {
      let msg = errors
        .iter()
        .map(|e| e.to_string())
        .collect::<Vec<_>>()
        .join("\n  ");
      return Err(format!("failed to compile source:\n  {}", msg));
    },
  };

  Ok(instructions_list(ctx, &top_fn))
}

fn kv(ctx: &mut ZuriContext, key: &str, value: Value) -> (Value, Value) {
  let k = ctx.heap().alloc_string(key);
  (k, value)
}

fn opt_num(n: Option<u32>) -> Value {
  n.map(|n| Value::number(n as f64)).unwrap_or(Value::nil())
}

fn instr_node(ctx: &mut ZuriContext, op: &str, line: Option<u32>, fields: Vec<(Value, Value)>) -> Value {
  let op_k = ctx.heap().alloc_string("op");
  let op_v = ctx.heap().alloc_string(op);
  let line_k = ctx.heap().alloc_string("line");
  let line_v = opt_num(line);
  let fields_k = ctx.heap().alloc_string("fields");
  let fields_v = ctx.heap().alloc_dict(fields);

  ctx
    .heap()
    .alloc_dict(vec![(op_k, op_v), (line_k, line_v), (fields_k, fields_v)])
}

fn instructions_list(ctx: &mut ZuriContext, f: &ObjFunction) -> Value {
  let mut vals = Vec::with_capacity(f.chunk.code.len());
  for i in 0..f.chunk.code.len() {
    let instr = f.chunk.code[i];
    let line = f.chunk.lines.get(i).copied();
    vals.push(instr_to_value(ctx, &instr, &f.chunk, line));
  }
  ctx.heap().alloc_list(vals)
}

/// `{ name, arity, variadic, instructions }` for a nested function
/// prototype reached through a `Closure` instruction's own constant;
/// richer than the bare list `compile()` itself returns for the
/// top-level script, since a nested function needs its own identity
/// alongside its body.
fn function_to_value(ctx: &mut ZuriContext, f: &ObjFunction) -> Value {
  let name = f.name.clone();
  let arity = f.arity;
  let variadic = f.variadic;
  let instructions = instructions_list(ctx, f);

  let name_v = ctx.heap().alloc_string(name);
  let f_fields = vec![
    kv(ctx, "name", name_v),
    kv(ctx, "arity", Value::number(arity as f64)),
    kv(ctx, "variadic", Value::bool(variadic)),
    kv(ctx, "instructions", instructions),
  ];
  ctx.heap().alloc_dict(f_fields)
}

/// A resolved constant-pool value, converted for embedding directly on
/// whichever instruction referenced it. Every runtime value shape that
/// can actually land in `Chunk::constants` is handled explicitly
/// (numbers, strings, bigints, bytes, and function prototypes for
/// `Closure`); anything else falls back to a `"<kind>"` placeholder
/// string rather than panicking, since new constant-pool value shapes
/// are a compiler concern this module shouldn't have to track in
/// lockstep to stay correct.
fn const_value_to_value(ctx: &mut ZuriContext, v: Value) -> Value {
  if v.is_nil() {
    Value::nil()
  } else if v.is_bool() {
    Value::bool(v.as_bool())
  } else if v.is_number() {
    Value::number(v.as_number())
  } else if v.is_string() {
    let s = v.as_str().to_string();
    ctx.heap().alloc_string(s)
  } else if v.is_bigint() {
    let b = v.as_bigint().clone();
    ctx.heap().alloc_bigint(b)
  } else if v.is_bytes() {
    let b = v.as_bytes();
    ctx.heap().alloc_bytes(b)
  } else if v.is_func() {
    function_to_value(ctx, v.as_func())
  } else {
    let placeholder = format!("<{}>", v.type_name());
    ctx.heap().alloc_string(placeholder)
  }
}

fn resolve_const(ctx: &mut ZuriContext, chunk: &Chunk, idx: u16) -> Value {
  const_value_to_value(ctx, chunk.constants[idx as usize])
}

fn jump_key_to_value(ctx: &mut ZuriContext, k: &JumpKey) -> Value {
  match k {
    JumpKey::Nil => Value::nil(),
    JumpKey::Bool(b) => Value::bool(*b),
    JumpKey::Number(bits) => Value::number(f64::from_bits(*bits)),
    JumpKey::Str(s) => ctx.heap().alloc_string(s.clone()),
  }
}

/// `chunk.jump_tables[table_idx]`, resolved into a list of `{ label,
/// offset }` entries (a list of pairs rather than a real Zuri dict,
/// so a `nil`/`false`/`0` label can't collide with each other the way
/// they might as literal dict keys); `offset` is the absolute
/// instruction index `Instr::UsingJump` jumps straight to on a match.
fn jump_table_to_value(ctx: &mut ZuriContext, table: &FxHashMap<JumpKey, usize>) -> Value {
  let mut entries = Vec::with_capacity(table.len());
  for (key, offset) in table {
    let label_v = jump_key_to_value(ctx, key);
    let fields = vec![
      kv(ctx, "label", label_v),
      kv(ctx, "offset", Value::number(*offset as f64)),
    ];
    entries.push(ctx.heap().alloc_dict(fields));
  }
  ctx.heap().alloc_list(entries)
}

fn param_type_to_value(ctx: &mut ZuriContext, chunk: &Chunk, t: &ParamType) -> Value {
  let (kind, class_name): (&str, Option<String>) = match t {
    ParamType::Bool => ("Bool", None),
    ParamType::Int => ("Int", None),
    ParamType::Number => ("Number", None),
    ParamType::BigInt => ("BigInt", None),
    ParamType::String => ("String", None),
    ParamType::Bytes => ("Bytes", None),
    ParamType::List => ("List", None),
    ParamType::Dict => ("Dict", None),
    ParamType::Range => ("Range", None),
    ParamType::File => ("File", None),
    ParamType::Function => ("Function", None),
    ParamType::Class => ("Class", None),
    ParamType::Callable => ("Callable", None),
    ParamType::Iterable => ("Iterable", None),
    ParamType::Instance(name_const) => (
      "Instance",
      Some(chunk.constants[*name_const as usize].as_str().to_string()),
    ),
  };

  let class_name_v = match class_name {
    Some(n) => ctx.heap().alloc_string(n),
    None => Value::nil(),
  };
  let kind_k = ctx.heap().alloc_string("kind");
  let kind_v = ctx.heap().alloc_string(kind);
  let class_k = ctx.heap().alloc_string("class_name");

  ctx
    .heap()
    .alloc_dict(vec![(kind_k, kind_v), (class_k, class_name_v)])
}

/// `chunk.param_checks[check_idx]`, resolved into `{ param_name,
/// position, nullable, types }`; `types` is a list of the same
/// `{ kind, class_name }` shape `param_type_to_value` builds.
fn param_check_to_value(ctx: &mut ZuriContext, chunk: &Chunk, check: &ParamTypeCheck) -> Value {
  let param_name = check.param_name.clone();
  let position = check.position;
  let nullable = check.nullable;

  let mut types = Vec::with_capacity(check.types.len());
  for t in &check.types {
    types.push(param_type_to_value(ctx, chunk, t));
  }
  let types_v = ctx.heap().alloc_list(types);

  let name_v = ctx.heap().alloc_string(param_name);
  let fields = vec![
    kv(ctx, "param_name", name_v),
    kv(ctx, "position", Value::number(position as f64)),
    kv(ctx, "nullable", Value::bool(nullable)),
    kv(ctx, "types", types_v),
  ];
  ctx.heap().alloc_dict(fields)
}

fn resolve_param_check(ctx: &mut ZuriContext, chunk: &Chunk, idx: u16) -> Value {
  let check = chunk.param_checks[idx as usize].clone();
  param_check_to_value(ctx, chunk, &check)
}

/// Every field name here matches `Instr`'s own field names 1:1 (see
/// `src/vm/chunk.rs`); a `*_const`/`table_idx`/`check_idx` field keeps
/// its raw index under that exact name AND gets a resolved sibling
/// field (`const`/`table`/`check`) carrying what it actually points
/// at, per this module's own doc comment.
fn instr_to_value(ctx: &mut ZuriContext, instr: &Instr, chunk: &Chunk, line: Option<u32>) -> Value {
  match *instr {
    Instr::LoadConst { dst, const_idx } => {
      let resolved = resolve_const(ctx, chunk, const_idx);
      let f = vec![
        kv(ctx, "dst", Value::number(dst as f64)),
        kv(ctx, "const_idx", Value::number(const_idx as f64)),
        kv(ctx, "value", resolved),
      ];
      instr_node(ctx, "LoadConst", line, f)
    },
    Instr::LoadNil { dst } => {
      let f = vec![kv(ctx, "dst", Value::number(dst as f64))];
      instr_node(ctx, "LoadNil", line, f)
    },
    Instr::LoadBool { dst, val } => {
      let f = vec![
        kv(ctx, "dst", Value::number(dst as f64)),
        kv(ctx, "val", Value::bool(val)),
      ];
      instr_node(ctx, "LoadBool", line, f)
    },
    Instr::Move { dst, src } => {
      let f = vec![
        kv(ctx, "dst", Value::number(dst as f64)),
        kv(ctx, "src", Value::number(src as f64)),
      ];
      instr_node(ctx, "Move", line, f)
    },
    Instr::Add { dst, a, b } => binop_node(ctx, "Add", line, dst, a, b),
    Instr::Sub { dst, a, b } => binop_node(ctx, "Sub", line, dst, a, b),
    Instr::Mul { dst, a, b } => binop_node(ctx, "Mul", line, dst, a, b),
    Instr::Div { dst, a, b } => binop_node(ctx, "Div", line, dst, a, b),
    Instr::Pow { dst, a, b } => binop_node(ctx, "Pow", line, dst, a, b),
    Instr::Floor { dst, a, b } => binop_node(ctx, "Floor", line, dst, a, b),
    Instr::Mod { dst, a, b } => binop_node(ctx, "Mod", line, dst, a, b),
    Instr::Neg { dst, src } => unop_node(ctx, "Neg", line, dst, src),
    Instr::Not { dst, src } => unop_node(ctx, "Not", line, dst, src),
    Instr::Concat { dst, a, b } => binop_node(ctx, "Concat", line, dst, a, b),
    Instr::BitAnd { dst, a, b } => binop_node(ctx, "BitAnd", line, dst, a, b),
    Instr::BitOr { dst, a, b } => binop_node(ctx, "BitOr", line, dst, a, b),
    Instr::BitXor { dst, a, b } => binop_node(ctx, "BitXor", line, dst, a, b),
    Instr::BitShl { dst, a, b } => binop_node(ctx, "BitShl", line, dst, a, b),
    Instr::BitShr { dst, a, b } => binop_node(ctx, "BitShr", line, dst, a, b),
    Instr::BitUshr { dst, a, b } => binop_node(ctx, "BitUshr", line, dst, a, b),
    Instr::BitNot { dst, src } => unop_node(ctx, "BitNot", line, dst, src),
    Instr::Eq { dst, a, b } => binop_node(ctx, "Eq", line, dst, a, b),
    Instr::Neq { dst, a, b } => binop_node(ctx, "Neq", line, dst, a, b),
    Instr::Lt { dst, a, b } => binop_node(ctx, "Lt", line, dst, a, b),
    Instr::Le { dst, a, b } => binop_node(ctx, "Le", line, dst, a, b),
    Instr::Gt { dst, a, b } => binop_node(ctx, "Gt", line, dst, a, b),
    Instr::Ge { dst, a, b } => binop_node(ctx, "Ge", line, dst, a, b),
    Instr::Jmp { offset } => {
      let f = vec![kv(ctx, "offset", Value::number(offset as f64))];
      instr_node(ctx, "Jmp", line, f)
    },
    Instr::JmpIfFalse { cond, offset } => jmp_cond_node(ctx, "JmpIfFalse", line, cond, offset),
    Instr::JmpIfTrue { cond, offset } => jmp_cond_node(ctx, "JmpIfTrue", line, cond, offset),
    Instr::Call { dst, func, num_args } => {
      let f = vec![
        kv(ctx, "dst", Value::number(dst as f64)),
        kv(ctx, "func", Value::number(func as f64)),
        kv(ctx, "num_args", Value::number(num_args as f64)),
      ];
      instr_node(ctx, "Call", line, f)
    },
    Instr::Return { src } => {
      let f = vec![kv(ctx, "src", Value::number(src as f64))];
      instr_node(ctx, "Return", line, f)
    },
    Instr::Print { src } => {
      let f = vec![kv(ctx, "src", Value::number(src as f64))];
      instr_node(ctx, "Print", line, f)
    },
    Instr::GetGlobal { dst, name_const } => {
      let resolved = resolve_const(ctx, chunk, name_const);
      let f = vec![
        kv(ctx, "dst", Value::number(dst as f64)),
        kv(ctx, "name_const", Value::number(name_const as f64)),
        kv(ctx, "name", resolved),
      ];
      instr_node(ctx, "GetGlobal", line, f)
    },
    Instr::SetGlobal { name_const, src } => {
      let resolved = resolve_const(ctx, chunk, name_const);
      let f = vec![
        kv(ctx, "name_const", Value::number(name_const as f64)),
        kv(ctx, "name", resolved),
        kv(ctx, "src", Value::number(src as f64)),
      ];
      instr_node(ctx, "SetGlobal", line, f)
    },
    Instr::AssignGlobal { name_const, src } => {
      let resolved = resolve_const(ctx, chunk, name_const);
      let f = vec![
        kv(ctx, "name_const", Value::number(name_const as f64)),
        kv(ctx, "name", resolved),
        kv(ctx, "src", Value::number(src as f64)),
      ];
      instr_node(ctx, "AssignGlobal", line, f)
    },
    Instr::Closure { dst, proto_const } => {
      let resolved = resolve_const(ctx, chunk, proto_const);
      let f = vec![
        kv(ctx, "dst", Value::number(dst as f64)),
        kv(ctx, "proto_const", Value::number(proto_const as f64)),
        kv(ctx, "prototype", resolved),
      ];
      instr_node(ctx, "Closure", line, f)
    },
    Instr::GetUpval { dst, idx } => {
      let f = vec![
        kv(ctx, "dst", Value::number(dst as f64)),
        kv(ctx, "idx", Value::number(idx as f64)),
      ];
      instr_node(ctx, "GetUpval", line, f)
    },
    Instr::SetUpval { idx, src } => {
      let f = vec![
        kv(ctx, "idx", Value::number(idx as f64)),
        kv(ctx, "src", Value::number(src as f64)),
      ];
      instr_node(ctx, "SetUpval", line, f)
    },
    Instr::CloseUpvalues { from } => {
      let f = vec![kv(ctx, "from", Value::number(from as f64))];
      instr_node(ctx, "CloseUpvalues", line, f)
    },
    Instr::MakeList { dst, start, count } => {
      let f = vec![
        kv(ctx, "dst", Value::number(dst as f64)),
        kv(ctx, "start", Value::number(start as f64)),
        kv(ctx, "count", Value::number(count as f64)),
      ];
      instr_node(ctx, "MakeList", line, f)
    },
    Instr::MakeDict { dst, start, count } => {
      let f = vec![
        kv(ctx, "dst", Value::number(dst as f64)),
        kv(ctx, "start", Value::number(start as f64)),
        kv(ctx, "count", Value::number(count as f64)),
      ];
      instr_node(ctx, "MakeDict", line, f)
    },
    Instr::MakeClass { dst, name_const, superclass } => {
      let resolved = resolve_const(ctx, chunk, name_const);
      let superclass_v = superclass
        .map(|r| Value::number(r as f64))
        .unwrap_or(Value::nil());
      let f = vec![
        kv(ctx, "dst", Value::number(dst as f64)),
        kv(ctx, "name_const", Value::number(name_const as f64)),
        kv(ctx, "name", resolved),
        kv(ctx, "superclass", superclass_v),
      ];
      instr_node(ctx, "MakeClass", line, f)
    },
    Instr::DeclareField { class, name_const } => {
      let resolved = resolve_const(ctx, chunk, name_const);
      let f = vec![
        kv(ctx, "class_reg", Value::number(class as f64)),
        kv(ctx, "name_const", Value::number(name_const as f64)),
        kv(ctx, "name", resolved),
      ];
      instr_node(ctx, "DeclareField", line, f)
    },
    Instr::SetFieldInit { class, src } => {
      let f = vec![
        kv(ctx, "class_reg", Value::number(class as f64)),
        kv(ctx, "src", Value::number(src as f64)),
      ];
      instr_node(ctx, "SetFieldInit", line, f)
    },
    Instr::SetMethod { class, name_const, src } => {
      let resolved = resolve_const(ctx, chunk, name_const);
      let f = vec![
        kv(ctx, "class_reg", Value::number(class as f64)),
        kv(ctx, "name_const", Value::number(name_const as f64)),
        kv(ctx, "name", resolved),
        kv(ctx, "src", Value::number(src as f64)),
      ];
      instr_node(ctx, "SetMethod", line, f)
    },
    Instr::DeclareStatic { class, name_const, src } => {
      let resolved = resolve_const(ctx, chunk, name_const);
      let f = vec![
        kv(ctx, "class_reg", Value::number(class as f64)),
        kv(ctx, "name_const", Value::number(name_const as f64)),
        kv(ctx, "name", resolved),
        kv(ctx, "src", Value::number(src as f64)),
      ];
      instr_node(ctx, "DeclareStatic", line, f)
    },
    Instr::FinalizeClass { class } => {
      let f = vec![kv(ctx, "class_reg", Value::number(class as f64))];
      instr_node(ctx, "FinalizeClass", line, f)
    },
    Instr::GetField { dst, obj, name_const } => {
      let resolved = resolve_const(ctx, chunk, name_const);
      let f = vec![
        kv(ctx, "dst", Value::number(dst as f64)),
        kv(ctx, "obj", Value::number(obj as f64)),
        kv(ctx, "name_const", Value::number(name_const as f64)),
        kv(ctx, "name", resolved),
      ];
      instr_node(ctx, "GetField", line, f)
    },
    Instr::SetField { obj, name_const, src } => {
      let resolved = resolve_const(ctx, chunk, name_const);
      let f = vec![
        kv(ctx, "obj", Value::number(obj as f64)),
        kv(ctx, "name_const", Value::number(name_const as f64)),
        kv(ctx, "name", resolved),
        kv(ctx, "src", Value::number(src as f64)),
      ];
      instr_node(ctx, "SetField", line, f)
    },
    Instr::Invoke { dst, obj, method_const, num_args } => {
      let resolved = resolve_const(ctx, chunk, method_const);
      let f = vec![
        kv(ctx, "dst", Value::number(dst as f64)),
        kv(ctx, "obj", Value::number(obj as f64)),
        kv(ctx, "method_const", Value::number(method_const as f64)),
        kv(ctx, "method", resolved),
        kv(ctx, "num_args", Value::number(num_args as f64)),
      ];
      instr_node(ctx, "Invoke", line, f)
    },
    Instr::InvokeSuper { dst, superclass, method_const, num_args } => {
      let resolved = resolve_const(ctx, chunk, method_const);
      let f = vec![
        kv(ctx, "dst", Value::number(dst as f64)),
        kv(ctx, "superclass", Value::number(superclass as f64)),
        kv(ctx, "method_const", Value::number(method_const as f64)),
        kv(ctx, "method", resolved),
        kv(ctx, "num_args", Value::number(num_args as f64)),
      ];
      instr_node(ctx, "InvokeSuper", line, f)
    },
    Instr::CallSuperCtor { dst, superclass, num_args } => {
      let f = vec![
        kv(ctx, "dst", Value::number(dst as f64)),
        kv(ctx, "superclass", Value::number(superclass as f64)),
        kv(ctx, "num_args", Value::number(num_args as f64)),
      ];
      instr_node(ctx, "CallSuperCtor", line, f)
    },
    Instr::Import { dst, path_const, importer_const } => {
      let path_v = resolve_const(ctx, chunk, path_const);
      let importer_v = resolve_const(ctx, chunk, importer_const);
      let f = vec![
        kv(ctx, "dst", Value::number(dst as f64)),
        kv(ctx, "path_const", Value::number(path_const as f64)),
        kv(ctx, "path", path_v),
        kv(ctx, "importer_const", Value::number(importer_const as f64)),
        kv(ctx, "importer", importer_v),
      ];
      instr_node(ctx, "Import", line, f)
    },
    Instr::ImportAll { module } => {
      let f = vec![kv(ctx, "module", Value::number(module as f64))];
      instr_node(ctx, "ImportAll", line, f)
    },
    Instr::MakePromoted { dst, module, name_const } => {
      let resolved = resolve_const(ctx, chunk, name_const);
      let f = vec![
        kv(ctx, "dst", Value::number(dst as f64)),
        kv(ctx, "module", Value::number(module as f64)),
        kv(ctx, "name_const", Value::number(name_const as f64)),
        kv(ctx, "name", resolved),
      ];
      instr_node(ctx, "MakePromoted", line, f)
    },
    Instr::GetIndex { dst, obj, idx } => {
      let f = vec![
        kv(ctx, "dst", Value::number(dst as f64)),
        kv(ctx, "obj", Value::number(obj as f64)),
        kv(ctx, "idx", Value::number(idx as f64)),
      ];
      instr_node(ctx, "GetIndex", line, f)
    },
    Instr::SetIndex { obj, idx, src } => {
      let f = vec![
        kv(ctx, "obj", Value::number(obj as f64)),
        kv(ctx, "idx", Value::number(idx as f64)),
        kv(ctx, "src", Value::number(src as f64)),
      ];
      instr_node(ctx, "SetIndex", line, f)
    },
    Instr::GetSlice { dst, obj, lo, hi } => {
      let f = vec![
        kv(ctx, "dst", Value::number(dst as f64)),
        kv(ctx, "obj", Value::number(obj as f64)),
        kv(ctx, "lo", Value::number(lo as f64)),
        kv(ctx, "hi", Value::number(hi as f64)),
      ];
      instr_node(ctx, "GetSlice", line, f)
    },
    Instr::MakeRange { dst, lower, upper } => {
      let f = vec![
        kv(ctx, "dst", Value::number(dst as f64)),
        kv(ctx, "lower", Value::number(lower as f64)),
        kv(ctx, "upper", Value::number(upper as f64)),
      ];
      instr_node(ctx, "MakeRange", line, f)
    },
    Instr::UsingJump { subject, table_idx } => {
      let table = jump_table_to_value(ctx, &chunk.jump_tables[table_idx as usize]);
      let f = vec![
        kv(ctx, "subject", Value::number(subject as f64)),
        kv(ctx, "table_idx", Value::number(table_idx as f64)),
        kv(ctx, "table", table),
      ];
      instr_node(ctx, "UsingJump", line, f)
    },
    Instr::Raise { src } => {
      let f = vec![kv(ctx, "src", Value::number(src as f64))];
      instr_node(ctx, "Raise", line, f)
    },
    Instr::PushCatch { var_reg, offset } => {
      let var_v = var_reg.map(|r| Value::number(r as f64)).unwrap_or(Value::nil());
      let f = vec![
        kv(ctx, "var_reg", var_v),
        kv(ctx, "offset", Value::number(offset as f64)),
      ];
      instr_node(ctx, "PushCatch", line, f)
    },
    Instr::PopCatch => instr_node(ctx, "PopCatch", line, vec![]),
    Instr::AddImm { dst, a, imm_const } => imm_node(ctx, "AddImm", line, chunk, dst, a, imm_const),
    Instr::SubImm { dst, a, imm_const } => imm_node(ctx, "SubImm", line, chunk, dst, a, imm_const),
    Instr::MulImm { dst, a, imm_const } => imm_node(ctx, "MulImm", line, chunk, dst, a, imm_const),
    Instr::LtImm { dst, a, imm_const } => imm_node(ctx, "LtImm", line, chunk, dst, a, imm_const),
    Instr::LeImm { dst, a, imm_const } => imm_node(ctx, "LeImm", line, chunk, dst, a, imm_const),
    Instr::GtImm { dst, a, imm_const } => imm_node(ctx, "GtImm", line, chunk, dst, a, imm_const),
    Instr::GeImm { dst, a, imm_const } => imm_node(ctx, "GeImm", line, chunk, dst, a, imm_const),
    Instr::EqImm { dst, a, imm_const } => imm_node(ctx, "EqImm", line, chunk, dst, a, imm_const),
    Instr::NeqImm { dst, a, imm_const } => imm_node(ctx, "NeqImm", line, chunk, dst, a, imm_const),
    Instr::CheckParamType { reg, check_idx } => {
      let resolved = resolve_param_check(ctx, chunk, check_idx);
      let f = vec![
        kv(ctx, "reg", Value::number(reg as f64)),
        kv(ctx, "check_idx", Value::number(check_idx as f64)),
        kv(ctx, "check", resolved),
      ];
      instr_node(ctx, "CheckParamType", line, f)
    },
  }
}

fn binop_node(ctx: &mut ZuriContext, op: &str, line: Option<u32>, dst: u8, a: u8, b: u8) -> Value {
  let f = vec![
    kv(ctx, "dst", Value::number(dst as f64)),
    kv(ctx, "a", Value::number(a as f64)),
    kv(ctx, "b", Value::number(b as f64)),
  ];
  instr_node(ctx, op, line, f)
}

fn unop_node(ctx: &mut ZuriContext, op: &str, line: Option<u32>, dst: u8, src: u8) -> Value {
  let f = vec![
    kv(ctx, "dst", Value::number(dst as f64)),
    kv(ctx, "src", Value::number(src as f64)),
  ];
  instr_node(ctx, op, line, f)
}

fn jmp_cond_node(ctx: &mut ZuriContext, op: &str, line: Option<u32>, cond: u8, offset: i16) -> Value {
  let f = vec![
    kv(ctx, "cond", Value::number(cond as f64)),
    kv(ctx, "offset", Value::number(offset as f64)),
  ];
  instr_node(ctx, op, line, f)
}

fn imm_node(
  ctx: &mut ZuriContext,
  op: &str,
  line: Option<u32>,
  chunk: &Chunk,
  dst: u8,
  a: u8,
  imm_const: u16,
) -> Value {
  let resolved = resolve_const(ctx, chunk, imm_const);
  let f = vec![
    kv(ctx, "dst", Value::number(dst as f64)),
    kv(ctx, "a", Value::number(a as f64)),
    kv(ctx, "imm_const", Value::number(imm_const as f64)),
    kv(ctx, "imm", resolved),
  ];
  instr_node(ctx, op, line, f)
}
