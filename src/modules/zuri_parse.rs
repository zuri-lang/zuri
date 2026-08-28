//! `_zuri_parse` builtin module; the native backing for `zuri.parse()`.
//!
//! Runs the real `Lexer`/`Parser` pipeline (the SAME grammar the
//! compiler itself uses; no second, parallel parser to keep in sync)
//! and converts the resulting `Vec<Decl>` into a list of generic node
//! dicts, each shaped `{ kind, line, col, fields }`. `libs/zuri/ast.zu`
//! wraps each dict into a proper `Node` instance, recursively.
//!
//! Every comment and doc block the source contains comes back too, as
//! its own `"Comment"`/`"DocBlock"`-kind node sitting exactly where it
//! appeared lexically (see `Decl::Trivia`/`Stmt::Trivia` in
//! `src/compiler/ast.rs`, and the parser's own trivia-draining logic in
//! `src/compiler/parser.rs`); this is what makes the result "lossless
//! enough to rewrite the file from" for anything at statement/
//! declaration granularity. A comment written INSIDE an expression
//! (`foo(a, # x` on one line, `b)` on the next, ...) is never attached
//! inside that expression's own node (there's no node kind for it, and
//! never will be: CLAUDE.md's Zuri style never places a comment there
//! in the first place). It's still captured, though, not dropped: the
//! parser's trivia buffer doesn't distinguish "seen while parsing this
//! statement's own expression" from "seen while chasing trailing
//! whitespace toward the next one", so it comes back as an ordinary
//! sibling node immediately after the smallest enclosing statement/
//! declaration/class-member, in that statement's own surrounding list,
//! rather than nested inside the expression it visually sat in. Source
//! written against CLAUDE.md's own comment convention never hits this,
//! since every comment there is already its own whole line between
//! statements; this only affects source that breaks that convention,
//! and even then costs only where the comment ends up attached in the
//! tree, not the comment itself.

use crate::builtins::enforce::ArgType;
use crate::compiler::ast::{Decl, Expr, Stmt, Type};
use crate::compiler::lexer::Lexer;
use crate::compiler::parser::Parser;
use crate::compiler::token::{Token, TokenKind};
use crate::enforce_arg_count;
use crate::enforce_arg_type;
use crate::modules::zuri_lex::token_kind_name;
use crate::modules::{BuiltinModuleDef, native};
use crate::vm::object::ZuriContext;
use crate::vm::value::Value;
use crate::vm::vm::VM;

pub static MODULE: BuiltinModuleDef = BuiltinModuleDef {
  name: "_zuri_parse",
  build,
};

fn build(vm: &mut VM) -> Vec<(&'static str, Value)> {
  vec![("parse", native(vm, "parse", 1, false, parse_fn))]
}

/// `_zuri_parse.parse(source)`: parses `source` in full and returns
/// its top-level declarations as a list of node dicts. See this
/// module's own doc comment for the exact shape and what is/isn't
/// preserved. Raises (rather than returning a partial result) if
/// `source` has a syntax error; there is no meaningful AST to hand
/// back for input the parser itself couldn't make sense of.
fn parse_fn(ctx: &mut ZuriContext) -> Result<Value, String> {
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

  Ok(decl_list_to_value(ctx, &decls))
}

// Position helpers: best-effort `(line, col)` for a node, from
// whatever the current AST actually carries. Most `Expr`/`Stmt`/`Decl`
// variants don't store a position of their own (only ones embedding a
// `Token`, or an explicit trailing source-line `u32` on the handful of
// operator/delimiter nodes that need one for error messages); for
// everything else, this borrows the position of the first meaningful
// child, and only falls back to `(None, None)` for a genuinely
// position-less leaf/structural node (`Decl::None`, an empty block,
// ...). This doesn't affect reconstruction fidelity; rebuilding
// source from this AST needs structure, literal values, and comments,
// never a node's own line/col; it's purely a best-effort convenience
// for a consumer that wants to point at where something is.

fn expr_pos(e: &Expr) -> (Option<usize>, Option<usize>) {
  match e {
    Expr::Identifier(t) | Expr::Get(_, t) | Expr::Set(_, t, _) | Expr::Argument(t, _) => {
      (Some(t.line), Some(t.column))
    },
    Expr::Unary(_, _, line)
    | Expr::Binary(_, _, _, line)
    | Expr::Logical(_, _, _, line)
    | Expr::Range(_, _, line)
    | Expr::Call(_, _, line)
    | Expr::Index(_, _, line)
    | Expr::Slice(_, _, _, line) => (Some(*line as usize), None),
    Expr::Grouping(inner) => expr_pos(inner),
    Expr::Circuit(a, ..) => expr_pos(a),
    Expr::Condition(cond, ..) => expr_pos(cond),
    Expr::List(items) => items.first().map(expr_pos).unwrap_or((None, None)),
    Expr::Dict(keys, values) => keys
      .first()
      .or(values.first())
      .map(expr_pos)
      .unwrap_or((None, None)),
    Expr::Assign(target, _) => expr_pos(target),
    Expr::Anonymous(decl) => decl_pos(decl),
    Expr::Nil
    | Expr::Bool(_)
    | Expr::Integer(_)
    | Expr::Float(_)
    | Expr::BigNumber(_)
    | Expr::Literal(_)
    | Expr::Parent
    | Expr::Self_
    | Expr::TypeHint(..) => (None, None),
  }
}

fn stmt_pos(s: &Stmt) -> (Option<usize>, Option<usize>) {
  match s {
    Stmt::Var(t, ..) | Stmt::Trivia(t) => (Some(t.line), Some(t.column)),
    Stmt::Echo(e) | Stmt::Expression(e) | Stmt::Raise(e) | Stmt::Return(e) => expr_pos(e),
    Stmt::If(cond, ..) | Stmt::While(cond, _) | Stmt::Assert(cond, _) => expr_pos(cond),
    Stmt::Using(subject, ..) => expr_pos(subject),
    Stmt::Import(_, name_expr, ..) => expr_pos(name_expr),
    Stmt::Catch(body, ..) => stmt_pos(body),
    Stmt::Block(stmts) => stmts.first().map(stmt_pos).unwrap_or((None, None)),
    Stmt::Decl(d) => decl_pos(d),
    Stmt::VarList(list) => list.first().map(stmt_pos).unwrap_or((None, None)),
    Stmt::None | Stmt::FixContinue | Stmt::Continue | Stmt::Break => (None, None),
  }
}

fn decl_pos(d: &Decl) -> (Option<usize>, Option<usize>) {
  match d {
    Decl::Function(t, ..)
    | Decl::Method(t, ..)
    | Decl::Property(t, ..)
    | Decl::Class(t, ..)
    | Decl::Trivia(t) => (Some(t.line), Some(t.column)),
    Decl::Stmt(s) => stmt_pos(s),
    Decl::Block(stmts) => stmts.first().map(stmt_pos).unwrap_or((None, None)),
    Decl::Import(_, name_expr, ..) => expr_pos(name_expr),
    Decl::None => (None, None),
  }
}

// Node construction

fn kv(ctx: &mut ZuriContext, key: &str, value: Value) -> (Value, Value) {
  let k = ctx.heap().alloc_string(key);
  (k, value)
}

fn opt_pos(pos: Option<usize>) -> Value {
  pos.map(|n| Value::number(n as f64)).unwrap_or(Value::nil())
}

fn make_node(
  ctx: &mut ZuriContext,
  kind: &str,
  pos: (Option<usize>, Option<usize>),
  fields: Vec<(Value, Value)>,
) -> Value {
  let kind_k = ctx.heap().alloc_string("kind");
  let kind_v = ctx.heap().alloc_string(kind);
  let line_k = ctx.heap().alloc_string("line");
  let col_k = ctx.heap().alloc_string("col");
  let fields_k = ctx.heap().alloc_string("fields");
  let fields_v = ctx.heap().alloc_dict(fields);
  let line_v = opt_pos(pos.0);
  let col_v = opt_pos(pos.1);

  ctx.heap().alloc_dict(vec![
    (kind_k, kind_v),
    (line_k, line_v),
    (col_k, col_v),
    (fields_k, fields_v),
  ])
}

/// The plain string an identifier-carrying `Token` names; used
/// wherever the AST stores a `Token` purely to carry a name (a
/// variable, a function, a field, a class), never a full `Token`
/// dict the way `tokenize()`'s output does; `zuri.parse()`'s nodes
/// only need the name itself here, not its own separate span (the
/// owning node's `line`/`col` already covers it).
fn token_identifier_name(tok: &Token) -> String {
  match &tok.kind {
    TokenKind::Identifier(s) | TokenKind::Decorator(s) => s.clone(),
    other => format!("{}", other),
  }
}

fn opt_expr_to_value(ctx: &mut ZuriContext, e: &Option<Box<Expr>>) -> Value {
  match e {
    Some(e) => expr_to_value(ctx, e),
    None => Value::nil(),
  }
}

fn opt_stmt_to_value(ctx: &mut ZuriContext, s: &Option<Box<Stmt>>) -> Value {
  match s {
    Some(s) => stmt_to_value(ctx, s),
    None => Value::nil(),
  }
}

fn expr_list_to_value(ctx: &mut ZuriContext, items: &[Expr]) -> Value {
  let mut vals = Vec::with_capacity(items.len());
  for item in items {
    vals.push(expr_to_value(ctx, item));
  }
  ctx.heap().alloc_list(vals)
}

fn stmt_list_to_value(ctx: &mut ZuriContext, items: &[Stmt]) -> Value {
  let mut vals = Vec::with_capacity(items.len());
  for item in items {
    vals.push(stmt_to_value(ctx, item));
  }
  ctx.heap().alloc_list(vals)
}

fn decl_list_to_value(ctx: &mut ZuriContext, items: &[Decl]) -> Value {
  let mut vals = Vec::with_capacity(items.len());
  for item in items {
    vals.push(decl_to_value(ctx, item));
  }
  ctx.heap().alloc_list(vals)
}

fn type_to_value(ctx: &mut ZuriContext, t: &Type) -> Value {
  let (kind, pos, class_name): (&str, (Option<usize>, Option<usize>), Option<String>) = match t {
    Type::Any => ("Any", (None, None), None),
    Type::Bool => ("Bool", (None, None), None),
    Type::Int => ("Int", (None, None), None),
    Type::Number => ("Number", (None, None), None),
    Type::BigInt => ("BigInt", (None, None), None),
    Type::String => ("String", (None, None), None),
    Type::Bytes => ("Bytes", (None, None), None),
    Type::List => ("List", (None, None), None),
    Type::Dict => ("Dict", (None, None), None),
    Type::Range => ("Range", (None, None), None),
    Type::File => ("File", (None, None), None),
    Type::Function => ("Function", (None, None), None),
    Type::Type => ("Type", (None, None), None),
    Type::Callable => ("Callable", (None, None), None),
    Type::Iterable => ("Iterable", (None, None), None),
    Type::Instance(tok) => (
      "Instance",
      (Some(tok.line), Some(tok.column)),
      Some(token_identifier_name(tok)),
    ),
  };

  let class_name_value = match class_name {
    Some(n) => ctx.heap().alloc_string(n),
    None => Value::nil(),
  };
  let fields = vec![kv(ctx, "class_name", class_name_value)];
  make_node(ctx, kind, pos, fields)
}

fn types_list_to_value(ctx: &mut ZuriContext, items: &[Type]) -> Value {
  let mut vals = Vec::with_capacity(items.len());
  for item in items {
    vals.push(type_to_value(ctx, item));
  }
  ctx.heap().alloc_list(vals)
}

fn expr_to_value(ctx: &mut ZuriContext, e: &Expr) -> Value {
  let pos = expr_pos(e);

  match e {
    Expr::Nil => make_node(ctx, "Nil", pos, vec![]),
    Expr::Bool(b) => {
      let f = vec![kv(ctx, "value", Value::bool(*b))];
      make_node(ctx, "Bool", pos, f)
    },
    Expr::Integer(n) => {
      let f = vec![kv(ctx, "value", Value::number(*n as f64))];
      make_node(ctx, "Integer", pos, f)
    },
    Expr::Float(n) => {
      let f = vec![kv(ctx, "value", Value::number(*n))];
      make_node(ctx, "Float", pos, f)
    },
    Expr::BigNumber(n) => {
      let v = ctx.heap().alloc_bigint(n.clone());
      let f = vec![kv(ctx, "value", v)];
      make_node(ctx, "BigNumber", pos, f)
    },
    Expr::Literal(s) => {
      let v = ctx.heap().alloc_string(s.clone());
      let f = vec![kv(ctx, "value", v)];
      make_node(ctx, "Literal", pos, f)
    },
    Expr::Unary(op, inner, _) => {
      let op_v = ctx.heap().alloc_string(token_kind_name(op));
      let inner_v = expr_to_value(ctx, inner);
      let f = vec![kv(ctx, "op", op_v), kv(ctx, "operand", inner_v)];
      make_node(ctx, "Unary", pos, f)
    },
    Expr::Binary(l, op, r, _) => {
      let op_v = ctx.heap().alloc_string(token_kind_name(op));
      let l_v = expr_to_value(ctx, l);
      let r_v = expr_to_value(ctx, r);
      let f = vec![kv(ctx, "left", l_v), kv(ctx, "op", op_v), kv(ctx, "right", r_v)];
      make_node(ctx, "Binary", pos, f)
    },
    Expr::Logical(l, op, r, _) => {
      let op_v = ctx.heap().alloc_string(token_kind_name(op));
      let l_v = expr_to_value(ctx, l);
      let r_v = expr_to_value(ctx, r);
      let f = vec![kv(ctx, "left", l_v), kv(ctx, "op", op_v), kv(ctx, "right", r_v)];
      make_node(ctx, "Logical", pos, f)
    },
    Expr::Circuit(l, op, r) => {
      let op_v = ctx.heap().alloc_string(token_kind_name(op));
      let l_v = expr_to_value(ctx, l);
      let r_v = expr_to_value(ctx, r);
      let f = vec![kv(ctx, "left", l_v), kv(ctx, "op", op_v), kv(ctx, "right", r_v)];
      make_node(ctx, "Circuit", pos, f)
    },
    Expr::Grouping(inner) => {
      let v = expr_to_value(ctx, inner);
      let f = vec![kv(ctx, "inner", v)];
      make_node(ctx, "Grouping", pos, f)
    },
    Expr::Range(lo, hi, _) => {
      let lo_v = expr_to_value(ctx, lo);
      let hi_v = expr_to_value(ctx, hi);
      let f = vec![kv(ctx, "lower", lo_v), kv(ctx, "upper", hi_v)];
      make_node(ctx, "Range", pos, f)
    },
    Expr::Identifier(tok) => {
      let name = ctx.heap().alloc_string(token_identifier_name(tok));
      let f = vec![kv(ctx, "name", name)];
      make_node(ctx, "Identifier", pos, f)
    },
    Expr::Condition(cond, then_e, else_e) => {
      let cond_v = expr_to_value(ctx, cond);
      let then_v = expr_to_value(ctx, then_e);
      let else_v = expr_to_value(ctx, else_e);
      let f = vec![
        kv(ctx, "condition", cond_v),
        kv(ctx, "then", then_v),
        kv(ctx, "otherwise", else_v),
      ];
      make_node(ctx, "Condition", pos, f)
    },
    Expr::Call(callee, args, _) => {
      let callee_v = expr_to_value(ctx, callee);
      let args_v = expr_list_to_value(ctx, args);
      let f = vec![kv(ctx, "callee", callee_v), kv(ctx, "arguments", args_v)];
      make_node(ctx, "Call", pos, f)
    },
    Expr::Get(obj, tok) => {
      let obj_v = expr_to_value(ctx, obj);
      let name = ctx.heap().alloc_string(token_identifier_name(tok));
      let f = vec![kv(ctx, "object", obj_v), kv(ctx, "name", name)];
      make_node(ctx, "Get", pos, f)
    },
    Expr::Set(obj, tok, val) => {
      let obj_v = expr_to_value(ctx, obj);
      let name = ctx.heap().alloc_string(token_identifier_name(tok));
      let val_v = expr_to_value(ctx, val);
      let f = vec![
        kv(ctx, "object", obj_v),
        kv(ctx, "name", name),
        kv(ctx, "value", val_v),
      ];
      make_node(ctx, "Set", pos, f)
    },
    Expr::Index(obj, idx, _) => {
      let obj_v = expr_to_value(ctx, obj);
      let idx_v = expr_to_value(ctx, idx);
      let f = vec![kv(ctx, "object", obj_v), kv(ctx, "index", idx_v)];
      make_node(ctx, "Index", pos, f)
    },
    Expr::Slice(obj, lo, hi, _) => {
      let obj_v = expr_to_value(ctx, obj);
      let lo_v = expr_to_value(ctx, lo);
      let hi_v = expr_to_value(ctx, hi);
      let f = vec![
        kv(ctx, "object", obj_v),
        kv(ctx, "lower", lo_v),
        kv(ctx, "upper", hi_v),
      ];
      make_node(ctx, "Slice", pos, f)
    },
    Expr::List(items) => {
      let items_v = expr_list_to_value(ctx, items);
      let f = vec![kv(ctx, "items", items_v)];
      make_node(ctx, "List", pos, f)
    },
    Expr::Dict(keys, values) => {
      let keys_v = expr_list_to_value(ctx, keys);
      let values_v = expr_list_to_value(ctx, values);
      let f = vec![kv(ctx, "keys", keys_v), kv(ctx, "values", values_v)];
      make_node(ctx, "Dict", pos, f)
    },
    Expr::Parent => make_node(ctx, "Parent", pos, vec![]),
    Expr::Self_ => make_node(ctx, "Self", pos, vec![]),
    Expr::Assign(target, value) => {
      let target_v = expr_to_value(ctx, target);
      let value_v = expr_to_value(ctx, value);
      let f = vec![kv(ctx, "target", target_v), kv(ctx, "value", value_v)];
      make_node(ctx, "Assign", pos, f)
    },
    Expr::Anonymous(decl) => {
      let decl_v = decl_to_value(ctx, decl);
      let f = vec![kv(ctx, "declaration", decl_v)];
      make_node(ctx, "Anonymous", pos, f)
    },
    Expr::TypeHint(types, nullable) => {
      let types_v = types_list_to_value(ctx, types);
      let f = vec![
        kv(ctx, "types", types_v),
        kv(ctx, "nullable", Value::bool(*nullable)),
      ];
      make_node(ctx, "TypeHint", pos, f)
    },
    Expr::Argument(tok, type_hint) => {
      let name = ctx.heap().alloc_string(token_identifier_name(tok));
      let type_v = expr_to_value(ctx, type_hint);
      let f = vec![kv(ctx, "name", name), kv(ctx, "type_hint", type_v)];
      make_node(ctx, "Argument", pos, f)
    },
  }
}

fn stmt_to_value(ctx: &mut ZuriContext, s: &Stmt) -> Value {
  let pos = stmt_pos(s);

  match s {
    Stmt::None => make_node(ctx, "None", pos, vec![]),
    Stmt::FixContinue => make_node(ctx, "FixContinue", pos, vec![]),
    Stmt::Echo(e) => {
      let v = expr_to_value(ctx, e);
      let f = vec![kv(ctx, "value", v)];
      make_node(ctx, "Echo", pos, f)
    },
    Stmt::Expression(e) => {
      let v = expr_to_value(ctx, e);
      let f = vec![kv(ctx, "value", v)];
      make_node(ctx, "Expression", pos, f)
    },
    Stmt::If(cond, then_s, else_s) => {
      let cond_v = expr_to_value(ctx, cond);
      let then_v = stmt_to_value(ctx, then_s);
      let else_v = opt_stmt_to_value(ctx, else_s);
      let f = vec![
        kv(ctx, "condition", cond_v),
        kv(ctx, "then", then_v),
        kv(ctx, "otherwise", else_v),
      ];
      make_node(ctx, "If", pos, f)
    },
    Stmt::While(cond, body) => {
      let cond_v = expr_to_value(ctx, cond);
      let body_v = stmt_to_value(ctx, body);
      let f = vec![kv(ctx, "condition", cond_v), kv(ctx, "body", body_v)];
      make_node(ctx, "While", pos, f)
    },
    Stmt::Continue => make_node(ctx, "Continue", pos, vec![]),
    Stmt::Break => make_node(ctx, "Break", pos, vec![]),
    Stmt::Raise(e) => {
      let v = expr_to_value(ctx, e);
      let f = vec![kv(ctx, "value", v)];
      make_node(ctx, "Raise", pos, f)
    },
    Stmt::Return(e) => {
      let v = expr_to_value(ctx, e);
      let f = vec![kv(ctx, "value", v)];
      make_node(ctx, "Return", pos, f)
    },
    Stmt::Assert(cond, msg) => {
      let cond_v = expr_to_value(ctx, cond);
      let msg_v = opt_expr_to_value(ctx, msg);
      let f = vec![kv(ctx, "condition", cond_v), kv(ctx, "message", msg_v)];
      make_node(ctx, "Assert", pos, f)
    },
    Stmt::Using(subject, labels, bodies, default) => {
      let subject_v = expr_to_value(ctx, subject);
      let labels_v = expr_list_to_value(ctx, labels);
      let bodies_v = stmt_list_to_value(ctx, bodies);
      let default_v = opt_stmt_to_value(ctx, default);
      let f = vec![
        kv(ctx, "subject", subject_v),
        kv(ctx, "labels", labels_v),
        kv(ctx, "bodies", bodies_v),
        // Not `"default"`: that's a reserved keyword, and a dict
        // field named after one can never be read back with plain
        // dot-access syntax (`node.fields.default` doesn't parse).
        kv(ctx, "default_body", default_v),
      ];
      make_node(ctx, "Using", pos, f)
    },
    Stmt::Import(path, name_expr, elements, imports_all, exported) => {
      let path_v = ctx.heap().alloc_string(path.clone());
      let name_v = expr_to_value(ctx, name_expr);
      let elements_v = expr_list_to_value(ctx, elements);
      let f = vec![
        kv(ctx, "path", path_v),
        kv(ctx, "name", name_v),
        kv(ctx, "elements", elements_v),
        kv(ctx, "imports_all", Value::bool(*imports_all)),
        kv(ctx, "exported", Value::bool(*exported)),
      ];
      make_node(ctx, "Import", pos, f)
    },
    Stmt::Catch(body, catch_body, error_var) => {
      let body_v = stmt_to_value(ctx, body);
      let catch_v = opt_stmt_to_value(ctx, catch_body);
      let var_v = opt_expr_to_value(ctx, error_var);
      let f = vec![
        kv(ctx, "body", body_v),
        kv(ctx, "catch_body", catch_v),
        kv(ctx, "error_var", var_v),
      ];
      make_node(ctx, "Catch", pos, f)
    },
    Stmt::Block(stmts) => {
      let v = stmt_list_to_value(ctx, stmts);
      let f = vec![kv(ctx, "statements", v)];
      make_node(ctx, "Block", pos, f)
    },
    Stmt::Decl(d) => {
      let v = decl_to_value(ctx, d);
      let f = vec![kv(ctx, "declaration", v)];
      make_node(ctx, "Decl", pos, f)
    },
    Stmt::Var(tok, init, type_hint, is_constant) => {
      let name = ctx.heap().alloc_string(token_identifier_name(tok));
      let init_v = expr_to_value(ctx, init);
      let type_v = opt_expr_to_value(ctx, type_hint);
      let f = vec![
        kv(ctx, "name", name),
        kv(ctx, "value", init_v),
        kv(ctx, "type_hint", type_v),
        kv(ctx, "is_constant", Value::bool(*is_constant)),
      ];
      make_node(ctx, "Var", pos, f)
    },
    Stmt::VarList(list) => {
      let v = stmt_list_to_value(ctx, list);
      let f = vec![kv(ctx, "declarations", v)];
      make_node(ctx, "VarList", pos, f)
    },
    Stmt::Trivia(tok) => trivia_to_value(ctx, tok, pos),
  }
}

/// Shared by `Stmt::Trivia`/`Decl::Trivia`: a `#`-comment becomes a
/// `"Comment"`-kind node, a `/* ... */` becomes a `"DocBlock"`-kind
/// node; both carry `{ text }`, the comment's own content, with the
/// leading `#`/wrapping `/*`/`*/` already stripped off (same content
/// the raw `Token` itself carries).
fn trivia_to_value(ctx: &mut ZuriContext, tok: &Token, pos: (Option<usize>, Option<usize>)) -> Value {
  let (kind, text) = match &tok.kind {
    TokenKind::Comment(s) => ("Comment", s.clone()),
    TokenKind::DocBlock(s) => ("DocBlock", s.clone()),
    _ => unreachable!("Decl::Trivia/Stmt::Trivia always wraps a Comment or DocBlock token"),
  };
  let text_v = ctx.heap().alloc_string(text);
  let f = vec![kv(ctx, "text", text_v)];
  make_node(ctx, kind, pos, f)
}

fn decl_to_value(ctx: &mut ZuriContext, d: &Decl) -> Value {
  let pos = decl_pos(d);

  match d {
    Decl::None => make_node(ctx, "None", pos, vec![]),
    Decl::Stmt(s) => {
      let v = stmt_to_value(ctx, s);
      let f = vec![kv(ctx, "statement", v)];
      make_node(ctx, "Stmt", pos, f)
    },
    Decl::Block(stmts) => {
      let v = stmt_list_to_value(ctx, stmts);
      let f = vec![kv(ctx, "statements", v)];
      make_node(ctx, "Block", pos, f)
    },
    Decl::Import(path, name_expr, elements, imports_all) => {
      let path_v = ctx.heap().alloc_string(path.clone());
      let name_v = expr_to_value(ctx, name_expr);
      let elements_v = expr_list_to_value(ctx, elements);
      let f = vec![
        kv(ctx, "path", path_v),
        kv(ctx, "name", name_v),
        kv(ctx, "elements", elements_v),
        kv(ctx, "imports_all", Value::bool(*imports_all)),
      ];
      make_node(ctx, "Import", pos, f)
    },
    Decl::Function(tok, params, body, is_variadic) => {
      let name = ctx.heap().alloc_string(token_identifier_name(tok));
      let params_v = expr_list_to_value(ctx, params);
      let body_v = stmt_to_value(ctx, body);
      let f = vec![
        kv(ctx, "name", name),
        kv(ctx, "parameters", params_v),
        kv(ctx, "body", body_v),
        kv(ctx, "is_variadic", Value::bool(*is_variadic)),
      ];
      make_node(ctx, "Function", pos, f)
    },
    Decl::Method(tok, params, body, is_variadic, is_static) => {
      let name = ctx.heap().alloc_string(token_identifier_name(tok));
      let params_v = expr_list_to_value(ctx, params);
      let body_v = stmt_to_value(ctx, body);
      let f = vec![
        kv(ctx, "name", name),
        kv(ctx, "parameters", params_v),
        kv(ctx, "body", body_v),
        kv(ctx, "is_variadic", Value::bool(*is_variadic)),
        kv(ctx, "is_static", Value::bool(*is_static)),
      ];
      make_node(ctx, "Method", pos, f)
    },
    Decl::Property(tok, value, type_hint, is_static, is_constant) => {
      let name = ctx.heap().alloc_string(token_identifier_name(tok));
      let value_v = expr_to_value(ctx, value);
      let type_v = expr_to_value(ctx, type_hint);
      let f = vec![
        kv(ctx, "name", name),
        kv(ctx, "value", value_v),
        kv(ctx, "type_hint", type_v),
        kv(ctx, "is_static", Value::bool(*is_static)),
        kv(ctx, "is_constant", Value::bool(*is_constant)),
      ];
      make_node(ctx, "Property", pos, f)
    },
    Decl::Class(tok, superclass, properties, methods, is_extension) => {
      let name = ctx.heap().alloc_string(token_identifier_name(tok));
      let superclass_v = match superclass {
        Some(e) => expr_to_value(ctx, e),
        None => Value::nil(),
      };
      let properties_v = decl_list_to_value(ctx, properties);
      let methods_v = decl_list_to_value(ctx, methods);
      let f = vec![
        kv(ctx, "name", name),
        kv(ctx, "superclass", superclass_v),
        kv(ctx, "properties", properties_v),
        kv(ctx, "methods", methods_v),
        kv(ctx, "is_extension", Value::bool(*is_extension)),
      ];
      make_node(ctx, "Class", pos, f)
    },
    Decl::Trivia(tok) => trivia_to_value(ctx, tok, pos),
  }
}
