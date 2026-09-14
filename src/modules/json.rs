//! `json` builtin module; `encode`/`decode` between Zuri values and
//! JSON text (RFC 8259).
//!
//! ## Encoding
//!
//! `nil`, `bool`, `number`, `string`, `list`, and `dict` map onto their
//! obvious JSON counterparts (a dict's keys are stringified via their
//! own `Display`, matching how every other object-with-string-keys
//! language treats a non-string map key). An INSTANCE is encoded by
//! calling its class's `@to_json` decorator method, if it declares
//! one: see `libs/set.zu`'s and `libs/url.zu`'s own `@to_json`
//! methods for existing examples of this exact convention; and
//! JSON-encoding whatever that method returns instead. An instance
//! whose class declares no `@to_json` (or any other value with no
//! sensible JSON representation; a function, a class, a file, a
//! bigint, bytes) is a hard encode error, not a silent `null`/`"..."`
//! substitution.
//!
//! ## Decoding
//!
//! A standard recursive-descent parser producing the matching Zuri
//! primitives: JSON `null` -> nil, `true`/`false` -> bool, a JSON
//! number -> Zuri number (always, even for something written as an
//! integer; this VM has no separate int type), a JSON string ->
//! string (with full `\uXXXX`/surrogate-pair support), a JSON array ->
//! list, a JSON object -> dict.

use std::fs;

use crate::builtins::enforce::ArgType;
use crate::modules::{BuiltinModuleDef, native, optional_bool};
use crate::vm::object::ZuriContext;
use crate::vm::value::Value;
use crate::vm::vm::VM;
use crate::{enforce_arg_range, enforce_arg_type, enforce_arg_type_opt};

/// Default for `encode`'s third (`max_depth`) argument: see
/// `encode_fn`'s own doc comment.
const DEFAULT_MAX_DEPTH: f64 = 1024.0;

pub static MODULE: BuiltinModuleDef = BuiltinModuleDef {
  name: "json",
  build,
};

fn build(vm: &mut VM) -> Vec<(&'static str, Value)> {
  vec![
    ("encode", native(vm, "encode", 1, true, encode_fn)),
    ("decode", native(vm, "decode", 1, true, decode_fn)),
    ("parse", native(vm, "parse", 1, true, parse_fn)),
    ("dump", native(vm, "dump", 2, true, dump_fn)),
  ]
}

// encode

/// `json.encode(value, compact: ?bool, max_depth: ?number)`.
///
/// `compact` (default `true`) chooses between a single-line, no-
/// whitespace encoding and a formatted one indented two spaces per
/// nesting level, per spec ("a non-compact one is formatted using two
/// spaces instead of tabs"). `max_depth` (default `1024`) bounds how
/// deeply nested containers; and `@to_json` calls, which recurse
/// back into this same encoder; are allowed to go, so a cyclic
/// `@to_json` override (or a pathologically deep structure) fails
/// with a catchable error instead of overflowing the Rust call stack.
fn encode_fn(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_range!(ctx, 1, 3);

  let value = ctx.args[0];
  let compact = optional_bool(ctx, 1, true)?;
  let max_depth = optional_max_depth(ctx, 2)?;

  let out = encode_source(ctx, value, compact, max_depth)?;
  Ok(ctx.heap().alloc_string(out))
}

/// Same idea as `optional_bool`, for `encode`/`dump`'s shared
/// `max_depth` argument; missing or `nil` means "use the default
/// (1024)"; anything present that isn't a non-negative integer is an
/// error.
fn optional_max_depth(ctx: &ZuriContext, idx: usize) -> Result<usize, String> {
  match ctx.args.get(idx) {
    None => Ok(DEFAULT_MAX_DEPTH as usize),
    Some(v) if v.is_nil() => Ok(DEFAULT_MAX_DEPTH as usize),
    Some(v) if v.is_number() => {
      let n = v.as_number();
      if n.fract() != 0.0 || n < 0.0 {
        Err(format!(
          "{}() expects argument {} (max_depth) to be a non-negative integer, got {}",
          ctx.name,
          idx + 1,
          n
        ))
      } else {
        Ok(n as usize)
      }
    },
    Some(v) => Err(format!(
      "{}() expects argument {} to be a number, got {}",
      ctx.name,
      idx + 1,
      v.type_name()
    )),
  }
}

/// Shared body of `encode`/`dump`: renders `value` to a JSON string
/// under the given `compact`/`max_depth` settings.
fn encode_source(
  ctx: &mut ZuriContext,
  value: Value,
  compact: bool,
  max_depth: usize,
) -> Result<String, String> {
  let opts = EncodeOpts { compact, max_depth };
  let mut out = String::new();
  encode_value(ctx, value, &mut out, 0, &opts)?;
  Ok(out)
}

struct EncodeOpts {
  compact: bool,
  max_depth: usize,
}

fn push_indent(out: &mut String, depth: usize) {
  for _ in 0..depth {
    out.push_str("  ");
  }
}

/// Appends `v`'s JSON representation onto `out`. Recurses for list/
/// dict elements and for whatever an `@to_json` override returns;
/// `depth` counts every such recursive step, checked against
/// `opts.max_depth` up front.
fn encode_value(
  ctx: &mut ZuriContext,
  v: Value,
  out: &mut String,
  depth: usize,
  opts: &EncodeOpts,
) -> Result<(), String> {
  if depth > opts.max_depth {
    return Err(format!(
      "json.encode(): maximum encoding depth of {} exceeded",
      opts.max_depth
    ));
  }

  if v.is_nil() {
    out.push_str("null");
  } else if v.is_bool() {
    out.push_str(if v.as_bool() { "true" } else { "false" });
  } else if v.is_number() {
    let n = v.as_number();
    if n.is_nan() || n.is_infinite() {
      return Err("json.encode(): cannot encode NaN or Infinity as JSON".to_string());
    }
    out.push_str(&format!("{}", n));
  } else if v.is_string() {
    encode_json_string(v.as_str(), out);
  } else if v.is_list() {
    let items = v.as_list();
    if items.is_empty() {
      out.push_str("[]");
    } else {
      // Pinned, and re-read via `ctx.vm.pinned(mark + i)` right before
      // EACH recursive call below, rather than iterated directly out
      // of `items`; a sibling element's own `@to_json` call
      // (reached recursively from `encode_value` below) can trigger a
      // collection, and `items` itself is just a plain owned `Vec`,
      // not a GC root, so any not-yet-visited element still sitting
      // in it would go stale the moment that happens. See
      // `VM::pin_values`'s own docs.
      let n = items.len();
      let mark = ctx.vm.pin_values(items);
      if opts.compact {
        out.push('[');
        for i in 0..n {
          if i > 0 {
            out.push(',');
          }
          let item = ctx.vm.pinned(mark + i);
          encode_value(ctx, item, out, depth + 1, opts)?;
        }
        out.push(']');
      } else {
        out.push_str("[\n");
        for i in 0..n {
          push_indent(out, depth + 1);
          let item = ctx.vm.pinned(mark + i);
          encode_value(ctx, item, out, depth + 1, opts)?;
          if i + 1 < n {
            out.push(',');
          }
          out.push('\n');
        }
        push_indent(out, depth);
        out.push(']');
      }
      ctx.vm.unpin(mark);
    }
  } else if v.is_dict() {
    let pairs = v.as_dict();
    if pairs.is_empty() {
      out.push_str("{}");
    } else {
      // Same reasoning as the list case above; `mark + 2*i` is key
      // `i`, `mark + 2*i + 1` is value `i`.
      let n = pairs.len();
      let mark = ctx
        .vm
        .pin_values(pairs.into_iter().flat_map(|(k, val)| [k, val]));
      if opts.compact {
        out.push('{');
        for i in 0..n {
          if i > 0 {
            out.push(',');
          }
          let k = ctx.vm.pinned(mark + 2 * i);
          let val = ctx.vm.pinned(mark + 2 * i + 1);
          let key_str = format!("{}", k);
          encode_json_string(&key_str, out);
          out.push(':');
          encode_value(ctx, val, out, depth + 1, opts)?;
        }
        out.push('}');
      } else {
        out.push_str("{\n");
        for i in 0..n {
          push_indent(out, depth + 1);
          let k = ctx.vm.pinned(mark + 2 * i);
          let val = ctx.vm.pinned(mark + 2 * i + 1);
          let key_str = format!("{}", k);
          encode_json_string(&key_str, out);
          out.push_str(": ");
          encode_value(ctx, val, out, depth + 1, opts)?;
          if i + 1 < n {
            out.push(',');
          }
          out.push('\n');
        }
        push_indent(out, depth);
        out.push('}');
      }
      ctx.vm.unpin(mark);
    }
  } else if v.is_instance() {
    let method = {
      let class = v.as_instance().class.as_class();
      class.methods.get("@to_json").copied()
    };
    match method {
      Some(m) => {
        let result = ctx.vm.call_value(m, &[v]).map_err(|e| ctx.vm.rethrow(e))?;
        encode_value(ctx, result, out, depth + 1, opts)?;
      },
      None => {
        let class_name = v.as_instance().class.as_class().name.clone();
        return Err(format!(
          "cannot convert an instance of '{}' to JSON: the class does not implement '@to_json'",
          class_name
        ));
      },
    }
  } else {
    return Err(format!(
      "cannot convert a {} to JSON",
      v.argument_type_name()
    ));
  }
  Ok(())
}

/// Writes `s` as a quoted, escaped JSON string literal onto `out`.
fn encode_json_string(s: &str, out: &mut String) {
  out.push('"');
  for c in s.chars() {
    match c {
      '"' => out.push_str("\\\""),
      '\\' => out.push_str("\\\\"),
      '\n' => out.push_str("\\n"),
      '\r' => out.push_str("\\r"),
      '\t' => out.push_str("\\t"),
      '\u{08}' => out.push_str("\\b"),
      '\u{0C}' => out.push_str("\\f"),
      c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
      c => out.push(c),
    }
  }
  out.push('"');
}

// decode

/// `json.decode(text, allow_comments: ?bool)`.
///
/// `allow_comments` (default `true`) permits `//` line comments and
/// `/* ... */` block comments anywhere whitespace would otherwise be
/// legal; a JSONC-style relaxation of strict RFC 8259, off only when
/// explicitly disabled.
fn decode_fn(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_range!(ctx, 1, 2);
  enforce_arg_type!(ctx, 0, ArgType::String);
  enforce_arg_type_opt!(ctx, 1, ArgType::Bool);

  let source = ctx.args[0].as_str().to_string();
  let allow_comments = optional_bool(ctx, 1, true)?;
  decode_source(ctx, &source, allow_comments)
}

/// Shared body of `decode`/`parse`: parses `source` under the given
/// `allow_comments` setting into a Zuri value.
fn decode_source(
  ctx: &mut ZuriContext,
  source: &str,
  allow_comments: bool,
) -> Result<Value, String> {
  let mut input = JsonInput::new(source, allow_comments);

  let value = parse_value(ctx, &mut input)?;
  input.skip_ws();
  if !input.at_end() {
    return Err("json.decode(): unexpected trailing data after JSON value".to_string());
  }
  Ok(value)
}

/// A `Vec<char>`-backed cursor over the source text; mirrors the
/// same "index a Vec<char>, not a byte string" approach this crate's
/// own `Lexer` (see `src/compiler/lexer.rs`) already uses, so a
/// multi-byte UTF-8 character never gets split across a byte offset.
struct JsonInput {
  chars: Vec<char>,
  pos: usize,
  /// Whether `skip_ws` also consumes `//`/`/* */` comments: see
  /// `decode_fn`'s own doc comment.
  allow_comments: bool,
}

impl JsonInput {
  fn new(s: &str, allow_comments: bool) -> Self {
    JsonInput {
      chars: s.chars().collect(),
      pos: 0,
      allow_comments,
    }
  }

  fn at_end(&self) -> bool {
    self.pos >= self.chars.len()
  }

  fn peek(&self) -> Option<char> {
    self.chars.get(self.pos).copied()
  }

  fn peek_at(&self, offset: usize) -> Option<char> {
    self.chars.get(self.pos + offset).copied()
  }

  fn advance(&mut self) -> Option<char> {
    let c = self.peek();
    if c.is_some() {
      self.pos += 1;
    }
    c
  }

  /// Skips whitespace and, when `allow_comments` is set, `//` line
  /// comments and `/* ... */` block comments; interleaved freely,
  /// same as ordinary whitespace, so e.g. `[1, /* two */ 2]` parses.
  /// An unterminated block comment is left for the surrounding parse
  /// to report as "unexpected end of input", rather than duplicating
  /// that error here.
  fn skip_ws(&mut self) {
    loop {
      while matches!(self.peek(), Some(c) if c.is_whitespace()) {
        self.pos += 1;
      }

      if self.allow_comments && self.peek() == Some('/') {
        match self.peek_at(1) {
          Some('/') => {
            self.pos += 2;
            while !self.at_end() && self.peek() != Some('\n') {
              self.pos += 1;
            }
            continue;
          },
          Some('*') => {
            self.pos += 2;
            while !self.at_end() && !(self.peek() == Some('*') && self.peek_at(1) == Some('/')) {
              self.pos += 1;
            }
            if !self.at_end() {
              self.pos += 2; // closing '*/'
            }
            continue;
          },
          _ => {},
        }
      }

      break;
    }
  }
}

fn parse_value(ctx: &mut ZuriContext, input: &mut JsonInput) -> Result<Value, String> {
  input.skip_ws();
  match input.peek() {
    Some('{') => parse_object(ctx, input),
    Some('[') => parse_array(ctx, input),
    Some('"') => {
      let s = parse_json_string(input)?;
      Ok(ctx.heap().alloc_string(s))
    },
    Some('t') => parse_literal(input, "true", Value::bool(true)),
    Some('f') => parse_literal(input, "false", Value::bool(false)),
    Some('n') => parse_literal(input, "null", Value::nil()),
    Some(c) if c == '-' || c.is_ascii_digit() => parse_number(input),
    Some(c) => Err(format!("json.decode(): unexpected character '{}'", c)),
    None => Err("json.decode(): unexpected end of input".to_string()),
  }
}

fn parse_literal(input: &mut JsonInput, literal: &str, value: Value) -> Result<Value, String> {
  for expected in literal.chars() {
    match input.advance() {
      Some(c) if c == expected => {},
      _ => {
        return Err(format!(
          "json.decode(): invalid literal, expected '{}'",
          literal
        ));
      },
    }
  }
  Ok(value)
}

fn parse_number(input: &mut JsonInput) -> Result<Value, String> {
  let start = input.pos;

  if input.peek() == Some('-') {
    input.advance();
  }
  while matches!(input.peek(), Some(c) if c.is_ascii_digit()) {
    input.advance();
  }
  if input.peek() == Some('.') {
    input.advance();
    while matches!(input.peek(), Some(c) if c.is_ascii_digit()) {
      input.advance();
    }
  }
  if matches!(input.peek(), Some('e') | Some('E')) {
    input.advance();
    if matches!(input.peek(), Some('+') | Some('-')) {
      input.advance();
    }
    while matches!(input.peek(), Some(c) if c.is_ascii_digit()) {
      input.advance();
    }
  }

  let text: String = input.chars[start..input.pos].iter().collect();
  text
    .parse::<f64>()
    .map(Value::number)
    .map_err(|_| format!("json.decode(): invalid number '{}'", text))
}

fn parse_json_string(input: &mut JsonInput) -> Result<String, String> {
  input.advance(); // opening '"'
  let mut result = String::new();

  loop {
    match input.advance() {
      None => return Err("json.decode(): unterminated string".to_string()),
      Some('"') => return Ok(result),
      Some('\\') => match input.advance() {
        Some('"') => result.push('"'),
        Some('\\') => result.push('\\'),
        Some('/') => result.push('/'),
        Some('b') => result.push('\u{08}'),
        Some('f') => result.push('\u{0C}'),
        Some('n') => result.push('\n'),
        Some('r') => result.push('\r'),
        Some('t') => result.push('\t'),
        Some('u') => {
          let cp = read_hex4(input)?;
          if (0xD800..=0xDBFF).contains(&cp) {
            // High surrogate; must be followed by a low surrogate
            // to form one real codepoint (RFC 8259 §7).
            if input.advance() != Some('\\') || input.advance() != Some('u') {
              return Err("json.decode(): expected low surrogate after high surrogate".to_string());
            }
            let low = read_hex4(input)?;
            if !(0xDC00..=0xDFFF).contains(&low) {
              return Err("json.decode(): invalid low surrogate".to_string());
            }
            let combined = 0x10000 + ((cp - 0xD800) << 10) + (low - 0xDC00);
            match char::from_u32(combined) {
              Some(c) => result.push(c),
              None => return Err("json.decode(): invalid unicode surrogate pair".to_string()),
            }
          } else {
            match char::from_u32(cp) {
              Some(c) => result.push(c),
              None => return Err("json.decode(): invalid unicode escape".to_string()),
            }
          }
        },
        _ => return Err("json.decode(): invalid escape sequence".to_string()),
      },
      Some(c) => result.push(c),
    }
  }
}

fn read_hex4(input: &mut JsonInput) -> Result<u32, String> {
  let mut n = 0u32;
  for _ in 0..4 {
    let c = input
      .advance()
      .ok_or_else(|| "json.decode(): unterminated unicode escape".to_string())?;
    let d = c
      .to_digit(16)
      .ok_or_else(|| "json.decode(): invalid hex digit in unicode escape".to_string())?;
    n = n * 16 + d;
  }
  Ok(n)
}

fn parse_array(ctx: &mut ZuriContext, input: &mut JsonInput) -> Result<Value, String> {
  input.advance(); // '['
  let mut items = Vec::new();

  input.skip_ws();
  if input.peek() == Some(']') {
    input.advance();
    return Ok(ctx.heap().alloc_list(items));
  }

  loop {
    let item = parse_value(ctx, input)?;
    items.push(item);
    input.skip_ws();

    match input.advance() {
      Some(',') => {
        input.skip_ws();
        continue;
      },
      Some(']') => break,
      _ => return Err("json.decode(): expected ',' or ']' in array".to_string()),
    }
  }

  Ok(ctx.heap().alloc_list(items))
}

// parse / dump; file-backed decode/encode

/// `json.parse(path, allow_comments: ?bool)`; reads the file at
/// `path` and decodes its content the same way `decode` would. A
/// missing file gets its own distinct message (rather than the raw,
/// somewhat opaque OS error text) since "the path doesn't exist" is
/// by far the most common failure a caller needs to branch on; any
/// other I/O failure (permissions, a directory given instead of a
/// file, ...) still surfaces with the underlying OS error attached.
fn parse_fn(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_range!(ctx, 1, 2);
  enforce_arg_type!(ctx, 0, ArgType::String);
  enforce_arg_type_opt!(ctx, 1, ArgType::Bool);

  let path = ctx.args[0].as_str().to_string();
  let allow_comments = optional_bool(ctx, 1, true)?;

  let source = read_json_file(&path)?;
  decode_source(ctx, &source, allow_comments)
}

fn read_json_file(path: &str) -> Result<String, String> {
  fs::read_to_string(path).map_err(|e| {
    if e.kind() == std::io::ErrorKind::NotFound {
      format!("json.parse(): file '{}' does not exist", path)
    } else {
      format!("json.parse(): could not read file '{}': {}", path, e)
    }
  })
}

/// `json.dump(value, path, compact: ?bool, max_depth: ?number)`;
/// encodes `value` exactly as `encode` would, then writes the result
/// to `path`, creating the file if it doesn't exist and overwriting
/// it if it does. Encoding errors (an un-`@to_json`-able instance, a
/// depth overflow, ...) are reported before any file is touched; a
/// failure to create/write the file itself is reported separately,
/// with the underlying OS error attached.
fn dump_fn(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_range!(ctx, 2, 4);
  enforce_arg_type!(ctx, 1, ArgType::String);
  enforce_arg_type_opt!(ctx, 2, ArgType::Bool);
  enforce_arg_type_opt!(ctx, 3, ArgType::Number);

  let value = ctx.args[0];
  let path = ctx.args[1].as_str().to_string();
  let compact = optional_bool(ctx, 2, true)?;
  let max_depth = optional_max_depth(ctx, 3)?;

  let encoded = encode_source(ctx, value, compact, max_depth)?;

  fs::write(&path, encoded)
    .map_err(|e| format!("json.dump(): could not write file '{}': {}", path, e))?;

  Ok(Value::bool(true))
}

fn parse_object(ctx: &mut ZuriContext, input: &mut JsonInput) -> Result<Value, String> {
  input.advance(); // '{'
  let mut pairs = Vec::new();

  input.skip_ws();
  if input.peek() == Some('}') {
    input.advance();
    return Ok(ctx.heap().alloc_dict(pairs));
  }

  loop {
    input.skip_ws();
    if input.peek() != Some('"') {
      return Err("json.decode(): expected a string key in object".to_string());
    }
    let key = parse_json_string(input)?;
    let key_val = ctx.heap().alloc_string(key);

    input.skip_ws();
    if input.advance() != Some(':') {
      return Err("json.decode(): expected ':' after object key".to_string());
    }

    let val = parse_value(ctx, input)?;
    pairs.push((key_val, val));
    input.skip_ws();

    match input.advance() {
      Some(',') => continue,
      Some('}') => break,
      _ => return Err("json.decode(): expected ',' or '}' in object".to_string()),
    }
  }

  Ok(ctx.heap().alloc_dict(pairs))
}
