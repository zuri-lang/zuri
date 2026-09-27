//! Reading Rust declarations.
//!
//! What crosses from a Rust library is its C ABI surface: functions in
//! `extern "C"` blocks, `extern "C" fn` definitions exported under a
//! fixed name, and the `#[repr(C)]` types they take. This reads that
//! surface as a crate writes it, so the source can be pasted as it
//! stands: function bodies, `impl` blocks, `use` lines and anything
//! else without an ABI meaning are skipped, `#[cfg(...)]` is evaluated
//! for the platform, and a type may be used before it is declared.
//!
//! Enums with fields follow the layouts `repr(C)`, `repr(C, int)` and
//! `repr(int)` define, so a value passes as a dictionary naming its
//! variant.

use std::sync::{Arc, Mutex};

use rustc_hash::FxHashMap;

use super::cdecl::SyntaxError;
use super::declare::{Constant, FunctionDecl, Scope, VariableDecl};
use super::types::{
  Abi, CType, Encoding, EnumInfo, FieldSpec, IntRole, Kind, Record, Signature, TypeRef, Variant,
  Variants, builtin,
};

type Parse<T> = Result<T, SyntaxError>;

fn error_at(line: usize, column: usize, message: impl Into<String>) -> SyntaxError {
  SyntaxError {
    message: message.into(),
    line,
    column,
  }
}

#[derive(Clone, Debug, PartialEq)]
enum Tok {
  Ident(String),
  Lifetime,
  Int(i128),
  Float(f64),
  Str(String),
  Char(char),
  Punct(&'static str),
  Eof,
}

#[derive(Clone, Debug)]
struct Token {
  tok: Tok,
  line: usize,
  column: usize,
}

const PUNCTUATORS: &[&str] = &[
  "::", "->", "=>", "==", "!=", "<=", ">=", "&&", "||", "<<", ">>", "..", "#", "!", "{", "}", "(",
  ")", "[", "]", "<", ">", ";", ",", ":", "=", "*", "&", "|", "^", "+", "-", "/", "%", ".", "?",
  "@", "~", "$",
];

fn lex(source: &str) -> Parse<Vec<Token>> {
  let chars: Vec<char> = source.chars().collect();
  let mut out = Vec::new();
  let mut i = 0;
  let mut line = 1;
  let mut column = 1;

  macro_rules! advance {
    () => {{
      if chars[i] == '\n' {
        line += 1;
        column = 1;
      } else {
        column += 1;
      }
      i += 1;
    }};
  }

  while i < chars.len() {
    let c = chars[i];

    if c.is_whitespace() {
      advance!();
      continue;
    }

    if c == '/' && chars.get(i + 1) == Some(&'/') {
      while i < chars.len() && chars[i] != '\n' {
        advance!();
      }
      continue;
    }

    if c == '/' && chars.get(i + 1) == Some(&'*') {
      let (l, col) = (line, column);
      let mut depth = 0;
      loop {
        if i >= chars.len() {
          return Err(error_at(l, col, "a comment is never closed"));
        }
        if chars[i] == '/' && chars.get(i + 1) == Some(&'*') {
          depth += 1;
          advance!();
          advance!();
          continue;
        }
        if chars[i] == '*' && chars.get(i + 1) == Some(&'/') {
          depth -= 1;
          advance!();
          advance!();
          if depth == 0 {
            break;
          }
          continue;
        }
        advance!();
      }
      continue;
    }

    let (tl, tc) = (line, column);
    let push = |out: &mut Vec<Token>, tok: Tok| {
      out.push(Token {
        tok,
        line: tl,
        column: tc,
      })
    };

    // Raw strings: r"...", r#"..."#, and their byte forms.
    let raw_start = if c == 'r' {
      Some(i + 1)
    } else if c == 'b' && chars.get(i + 1) == Some(&'r') {
      Some(i + 2)
    } else {
      None
    };
    if let Some(start) = raw_start
      && matches!(chars.get(start), Some('"') | Some('#'))
    {
      let mut hashes = 0;
      let mut j = start;
      while chars.get(j) == Some(&'#') {
        hashes += 1;
        j += 1;
      }
      if chars.get(j) == Some(&'"') {
        while i <= j {
          advance!();
        }
        let mut text = String::new();
        loop {
          if i >= chars.len() {
            return Err(error_at(tl, tc, "a raw string is never closed"));
          }
          if chars[i] == '"' && (1..=hashes).all(|k| chars.get(i + k) == Some(&'#')) {
            for _ in 0..=hashes {
              advance!();
            }
            break;
          }
          text.push(chars[i]);
          advance!();
        }
        push(&mut out, Tok::Str(text));
        continue;
      }
    }

    if c == '"'
      || (c == 'b' && chars.get(i + 1) == Some(&'"'))
      || (c == 'c' && chars.get(i + 1) == Some(&'"'))
    {
      if c != '"' {
        advance!();
      }
      advance!();
      let mut text = String::new();
      loop {
        let Some(&ch) = chars.get(i) else {
          return Err(error_at(tl, tc, "a string is never closed"));
        };
        if ch == '"' {
          advance!();
          break;
        }
        if ch == '\\' {
          advance!();
          let esc = chars.get(i).copied().unwrap_or('\\');
          advance!();
          match esc {
            'n' => text.push('\n'),
            't' => text.push('\t'),
            'r' => text.push('\r'),
            '0' => text.push('\0'),
            '\n' => {
              while chars.get(i).is_some_and(|c| c.is_whitespace()) {
                advance!();
              }
            },
            'x' => {
              let hex: String = chars[i..(i + 2).min(chars.len())].iter().collect();
              advance!();
              advance!();
              text.push(u8::from_str_radix(&hex, 16).unwrap_or(0) as char);
            },
            'u' => {
              let mut hex = String::new();
              advance!();
              while let Some(&h) = chars.get(i) {
                advance!();
                if h == '}' {
                  break;
                }
                hex.push(h);
              }
              text.push(
                u32::from_str_radix(&hex, 16)
                  .ok()
                  .and_then(char::from_u32)
                  .unwrap_or('\u{fffd}'),
              );
            },
            other => text.push(other),
          }
          continue;
        }
        text.push(ch);
        advance!();
      }
      push(&mut out, Tok::Str(text));
      continue;
    }

    if c == '\'' {
      // A character literal, or a lifetime.
      let next = chars.get(i + 1).copied();
      let closes = if next == Some('\\') {
        true
      } else {
        chars.get(i + 2) == Some(&'\'')
      };
      if closes {
        advance!();
        let ch = if chars[i] == '\\' {
          advance!();
          let esc = chars[i];
          advance!();
          match esc {
            'n' => '\n',
            't' => '\t',
            'r' => '\r',
            '0' => '\0',
            other => other,
          }
        } else {
          let ch = chars[i];
          advance!();
          ch
        };
        while i < chars.len() && chars[i] != '\'' {
          advance!();
        }
        advance!();
        push(&mut out, Tok::Char(ch));
      } else {
        advance!();
        while chars
          .get(i)
          .is_some_and(|c| c.is_alphanumeric() || *c == '_')
        {
          advance!();
        }
        push(&mut out, Tok::Lifetime);
      }
      continue;
    }

    if c.is_alphabetic() || c == '_' {
      let mut name = String::new();
      if c == 'r' && chars.get(i + 1) == Some(&'#') {
        advance!();
        advance!();
      }
      while chars
        .get(i)
        .is_some_and(|ch| ch.is_alphanumeric() || *ch == '_')
      {
        name.push(chars[i]);
        advance!();
      }
      push(&mut out, Tok::Ident(name));
      continue;
    }

    if c.is_ascii_digit() {
      let mut text = String::new();
      let mut float = false;
      while let Some(&ch) = chars.get(i) {
        if ch.is_ascii_alphanumeric() || ch == '_' {
          text.push(ch);
          advance!();
        } else if ch == '.' && chars.get(i + 1).is_some_and(|d| d.is_ascii_digit()) && !float {
          float = true;
          text.push(ch);
          advance!();
        } else {
          break;
        }
      }
      let clean: String = text.chars().filter(|c| *c != '_').collect();
      let tok = rust_number(&clean, float)
        .ok_or_else(|| error_at(tl, tc, format!("'{text}' is not a number")))?;
      push(&mut out, tok);
      continue;
    }

    let rest: String = chars[i..chars.len().min(i + 2)].iter().collect();
    let Some(p) = PUNCTUATORS.iter().find(|p| rest.starts_with(**p)) else {
      return Err(error_at(tl, tc, format!("unexpected character '{c}'")));
    };
    for _ in 0..p.len() {
      advance!();
    }
    push(&mut out, Tok::Punct(p));
  }

  out.push(Token {
    tok: Tok::Eof,
    line,
    column,
  });
  Ok(out)
}

fn rust_number(text: &str, float: bool) -> Option<Tok> {
  const SUFFIXES: &[&str] = &[
    "i128", "u128", "isize", "usize", "i64", "u64", "i32", "u32", "i16", "u16", "i8", "u8", "f32",
    "f64",
  ];
  let mut body = text;
  let mut is_float = float;
  for s in SUFFIXES {
    if let Some(stripped) = body.strip_suffix(s)
      && !stripped.is_empty()
      && !(stripped.starts_with("0x") && s.starts_with('f'))
    {
      body = stripped;
      if s.starts_with('f') {
        is_float = true;
      }
      break;
    }
  }

  if is_float || (!body.starts_with("0x") && body.contains(['e', 'E'])) {
    return body.parse::<f64>().ok().map(Tok::Float);
  }

  let value = if let Some(h) = body.strip_prefix("0x") {
    u128::from_str_radix(h, 16).ok()?
  } else if let Some(o) = body.strip_prefix("0o") {
    u128::from_str_radix(o, 8).ok()?
  } else if let Some(b) = body.strip_prefix("0b") {
    u128::from_str_radix(b, 2).ok()?
  } else {
    body.parse::<u128>().ok()?
  };
  Some(Tok::Int(value as i128))
}

// Syntax.

#[derive(Clone, Debug)]
enum Ty {
  Path {
    segments: Vec<String>,
    generics: Vec<Ty>,
    line: usize,
    column: usize,
  },
  Ptr {
    mutable: bool,
    inner: Box<Ty>,
  },
  Ref {
    mutable: bool,
    inner: Box<Ty>,
  },
  Array {
    inner: Box<Ty>,
    length: Box<Expr>,
  },
  Slice,
  Tuple(Vec<Ty>),
  Never,
  Fn {
    abi: Option<String>,
    params: Vec<Ty>,
    returns: Box<Ty>,
    variadic: bool,
  },
}

#[derive(Clone, Debug)]
enum Expr {
  Int(i128),
  Float(f64),
  Str(String),
  Path(String, usize, usize),
  Unary(&'static str, Box<Expr>),
  Binary(&'static str, Box<Expr>, Box<Expr>),
  Cast(Box<Expr>, Ty),
  SizeOf(bool, Ty),
}

#[derive(Default, Clone)]
struct Attrs {
  repr: Vec<String>,
  pack: Option<usize>,
  align: Option<usize>,
  no_mangle: bool,
  export_name: Option<String>,
  link_name: Option<String>,
  cfg_false: bool,
}

#[derive(Clone)]
struct Field {
  name: String,
  ty: Ty,
}

#[derive(Clone)]
enum VariantShape {
  Unit,
  Tuple(Vec<Ty>),
  Named(Vec<Field>),
}

#[derive(Clone)]
struct VariantSyntax {
  name: String,
  shape: VariantShape,
  discriminant: Option<Expr>,
}

#[derive(Clone)]
enum Item {
  Struct {
    name: String,
    attrs: Attrs,
    fields: Option<Vec<Field>>,
    line: usize,
    column: usize,
  },
  Union {
    name: String,
    attrs: Attrs,
    fields: Vec<Field>,
    line: usize,
    column: usize,
  },
  Enum {
    name: String,
    attrs: Attrs,
    variants: Vec<VariantSyntax>,
    line: usize,
    column: usize,
  },
  Alias {
    name: String,
    ty: Ty,
  },
  Opaque {
    name: String,
  },
  Function {
    name: String,
    symbol: Option<String>,
    abi: String,
    params: Vec<(Option<String>, Ty)>,
    returns: Ty,
    variadic: bool,
    line: usize,
    column: usize,
  },
  Static {
    name: String,
    symbol: String,
    ty: Ty,
  },
  Const {
    name: String,
    value: Expr,
  },
}

struct Parser {
  tokens: Vec<Token>,
  pos: usize,
}

impl Parser {
  fn peek(&self) -> &Token {
    &self.tokens[self.pos.min(self.tokens.len() - 1)]
  }

  fn peek_at(&self, n: usize) -> &Token {
    &self.tokens[(self.pos + n).min(self.tokens.len() - 1)]
  }

  fn next(&mut self) -> Token {
    let t = self.peek().clone();
    if !matches!(t.tok, Tok::Eof) {
      self.pos += 1;
    }
    t
  }

  fn is_punct(&self, p: &str) -> bool {
    matches!(&self.peek().tok, Tok::Punct(q) if *q == p)
  }

  fn is_ident(&self, name: &str) -> bool {
    matches!(&self.peek().tok, Tok::Ident(n) if n == name)
  }

  fn eat_punct(&mut self, p: &str) -> bool {
    if self.is_punct(p) {
      self.pos += 1;
      true
    } else {
      false
    }
  }

  fn eat_ident(&mut self, name: &str) -> bool {
    if self.is_ident(name) {
      self.pos += 1;
      true
    } else {
      false
    }
  }

  fn expect_punct(&mut self, p: &str) -> Parse<()> {
    if self.eat_punct(p) {
      return Ok(());
    }
    let t = self.peek().clone();
    Err(error_at(
      t.line,
      t.column,
      format!("expected '{p}', found {}", describe(&t)),
    ))
  }

  fn ident(&mut self) -> Parse<(String, usize, usize)> {
    let t = self.next();
    match t.tok {
      Tok::Ident(n) => Ok((n, t.line, t.column)),
      _ => Err(error_at(
        t.line,
        t.column,
        format!("expected a name, found {}", describe(&t)),
      )),
    }
  }

  fn fail<T>(&self, message: impl Into<String>) -> Parse<T> {
    let t = self.peek();
    Err(error_at(t.line, t.column, message))
  }

  /// Skips a balanced group starting at the current opening bracket.
  fn skip_group(&mut self) -> Parse<()> {
    let open = self.next();
    let (o, c) = match open.tok {
      Tok::Punct("{") => ("{", "}"),
      Tok::Punct("(") => ("(", ")"),
      Tok::Punct("[") => ("[", "]"),
      _ => return Err(error_at(open.line, open.column, "expected a bracket")),
    };
    let mut depth = 1;
    while depth > 0 {
      let t = self.next();
      match t.tok {
        Tok::Punct(p) if p == o => depth += 1,
        Tok::Punct(p) if p == c => depth -= 1,
        Tok::Eof => {
          return Err(error_at(
            open.line,
            open.column,
            format!("'{o}' is never closed"),
          ));
        },
        _ => {},
      }
    }
    Ok(())
  }

  /// Skips an item with no ABI meaning up to its end.
  fn skip_item(&mut self) -> Parse<()> {
    loop {
      match &self.peek().tok {
        Tok::Punct(";") => {
          self.pos += 1;
          return Ok(());
        },
        Tok::Punct("{") => {
          self.skip_group()?;
          return Ok(());
        },
        Tok::Punct("(") | Tok::Punct("[") => self.skip_group()?,
        Tok::Eof => return Ok(()),
        _ => self.pos += 1,
      }
    }
  }

  fn attributes(&mut self) -> Parse<Attrs> {
    let mut attrs = Attrs::default();

    while self.is_punct("#") {
      self.pos += 1;
      let inner = self.eat_punct("!");
      self.expect_punct("[")?;
      let start = self.pos;
      let mut depth = 1;
      while depth > 0 {
        match self.next().tok {
          Tok::Punct("[") => depth += 1,
          Tok::Punct("]") => depth -= 1,
          Tok::Eof => return self.fail("an attribute is never closed"),
          _ => {},
        }
      }
      if inner {
        continue;
      }
      let body: Vec<Token> = self.tokens[start..self.pos - 1].to_vec();
      read_attribute(&body, &mut attrs)?;
    }

    Ok(attrs)
  }

  fn visibility(&mut self) -> Parse<()> {
    if self.eat_ident("pub") && self.is_punct("(") {
      self.skip_group()?;
    }
    Ok(())
  }

  fn items(&mut self, out: &mut Vec<Item>, until_brace: bool) -> Parse<()> {
    loop {
      if until_brace && self.eat_punct("}") {
        return Ok(());
      }
      if matches!(self.peek().tok, Tok::Eof) {
        if until_brace {
          return self.fail("a block is never closed");
        }
        return Ok(());
      }
      self.item(out)?;
    }
  }

  fn item(&mut self, out: &mut Vec<Item>) -> Parse<()> {
    let attrs = self.attributes()?;
    self.visibility()?;

    let t = self.peek().clone();
    let Tok::Ident(word) = &t.tok else {
      if self.eat_punct(";") {
        return Ok(());
      }
      return Err(error_at(
        t.line,
        t.column,
        format!("expected an item, found {}", describe(&t)),
      ));
    };

    if attrs.cfg_false {
      return self.skip_item();
    }

    match word.as_str() {
      "use" | "extern" if word == "use" || self.peek_at(1).tok == Tok::Ident("crate".into()) => {
        self.skip_item()
      },
      "mod" => {
        self.pos += 1;
        let _ = self.ident()?;
        if self.eat_punct(";") {
          return Ok(());
        }
        self.expect_punct("{")?;
        self.items(out, true)
      },
      "impl" | "trait" | "macro_rules" => self.skip_item(),
      "struct" => self.struct_item(attrs, out),
      "union" => self.union_item(attrs, out),
      "enum" => self.enum_item(attrs, out),
      "type" => {
        self.pos += 1;
        let (name, ..) = self.ident()?;
        if self.generics()? {
          return self.fail(format!(
            "'{name}' is generic, and a generic type has no single layout"
          ));
        }
        self.expect_punct("=")?;
        let ty = self.ty()?;
        self.expect_punct(";")?;
        out.push(Item::Alias { name, ty });
        Ok(())
      },
      "const" if !matches!(&self.peek_at(1).tok, Tok::Ident(n) if n == "fn" || n == "extern" || n == "unsafe") =>
      {
        self.pos += 1;
        let (name, ..) = self.ident()?;
        self.expect_punct(":")?;
        let _ty = self.ty()?;
        self.expect_punct("=")?;
        let value = self.expression()?;
        self.expect_punct(";")?;
        out.push(Item::Const { name, value });
        Ok(())
      },
      "static" => {
        self.pos += 1;
        self.eat_ident("mut");
        let (name, ..) = self.ident()?;
        self.expect_punct(":")?;
        let ty = self.ty()?;
        let exported = attrs.no_mangle || attrs.export_name.is_some();
        self.skip_item()?;
        if exported {
          let symbol = attrs.export_name.clone().unwrap_or_else(|| name.clone());
          out.push(Item::Static { name, symbol, ty });
        }
        Ok(())
      },
      _ => self.function_or_block(attrs, out),
    }
  }

  fn function_or_block(&mut self, attrs: Attrs, out: &mut Vec<Item>) -> Parse<()> {
    // Qualifiers in any order: const, async, unsafe, safe, extern "ABI".
    let mut abi: Option<String> = None;
    let mut is_extern = false;

    loop {
      match &self.peek().tok {
        Tok::Ident(n)
          if n == "unsafe" || n == "const" || n == "async" || n == "safe" || n == "default" =>
        {
          self.pos += 1;
        },
        Tok::Ident(n) if n == "extern" => {
          self.pos += 1;
          is_extern = true;
          if let Tok::Str(s) = &self.peek().tok {
            abi = Some(s.clone());
            self.pos += 1;
          }
        },
        _ => break,
      }
    }

    if is_extern && self.is_punct("{") {
      let abi = abi.unwrap_or_else(|| "C".into());
      self.pos += 1;
      return self.extern_block(&abi, out);
    }

    if !self.eat_ident("fn") {
      return self.skip_item();
    }

    let (name, line, column) = self.ident()?;

    if !is_extern {
      // A Rust-ABI function has no stable calling convention to bind.
      return self.skip_item();
    }

    let abi = abi.unwrap_or_else(|| "C".into());

    if self.generics()? {
      return Err(error_at(
        line,
        column,
        format!("'{name}' is generic, and a generic function has no symbol"),
      ));
    }

    let (params, variadic) = self.params()?;
    let returns = self.return_type()?;

    while !self.is_punct("{") && !self.is_punct(";") {
      if matches!(self.peek().tok, Tok::Eof) {
        return self.fail("a function is never finished");
      }
      self.pos += 1;
    }
    self.skip_item()?;

    let symbol = if let Some(e) = &attrs.export_name {
      Some(e.clone())
    } else if attrs.no_mangle {
      Some(name.clone())
    } else {
      return Err(error_at(
        line,
        column,
        format!(
          "'{name}' has neither #[no_mangle] nor #[export_name], so the library exports it under a \
           mangled name that cannot be looked up"
        ),
      ));
    };

    out.push(Item::Function {
      name,
      symbol,
      abi,
      params,
      returns,
      variadic,
      line,
      column,
    });
    Ok(())
  }

  fn extern_block(&mut self, abi: &str, out: &mut Vec<Item>) -> Parse<()> {
    loop {
      if self.eat_punct("}") {
        return Ok(());
      }
      if matches!(self.peek().tok, Tok::Eof) {
        return self.fail("an extern block is never closed");
      }

      let attrs = self.attributes()?;
      self.visibility()?;

      while self.eat_ident("safe") || self.eat_ident("unsafe") {}

      if attrs.cfg_false {
        self.skip_item()?;
        continue;
      }

      if self.eat_ident("fn") {
        let (name, line, column) = self.ident()?;
        let (params, variadic) = self.params()?;
        let returns = self.return_type()?;
        self.expect_punct(";")?;
        let symbol = attrs.link_name.clone().or(Some(name.clone()));
        out.push(Item::Function {
          name,
          symbol,
          abi: abi.to_string(),
          params,
          returns,
          variadic,
          line,
          column,
        });
      } else if self.eat_ident("static") {
        self.eat_ident("mut");
        let (name, ..) = self.ident()?;
        self.expect_punct(":")?;
        let ty = self.ty()?;
        self.expect_punct(";")?;
        let symbol = attrs.link_name.clone().unwrap_or_else(|| name.clone());
        out.push(Item::Static { name, symbol, ty });
      } else if self.eat_ident("type") {
        let (name, ..) = self.ident()?;
        self.expect_punct(";")?;
        out.push(Item::Opaque { name });
      } else {
        self.skip_item()?;
      }
    }
  }

  fn params(&mut self) -> Parse<(Vec<(Option<String>, Ty)>, bool)> {
    self.expect_punct("(")?;
    let mut params = Vec::new();
    let mut variadic = false;

    while !self.eat_punct(")") {
      let _ = self.attributes()?;

      if self.eat_punct("..") {
        // `...` lexes as `..` and `.`, and may carry a name: `args: ...`.
        self.eat_punct(".");
        variadic = true;
        self.eat_punct(",");
        continue;
      }

      self.eat_ident("mut");
      let name = match (&self.peek().tok, &self.peek_at(1).tok) {
        (Tok::Ident(n), Tok::Punct(":")) if self.peek_at(2).tok != Tok::Punct(":") => {
          let n = n.clone();
          self.pos += 2;
          Some(n)
        },
        _ => None,
      };

      if self.is_punct("..") {
        self.pos += 1;
        self.eat_punct(".");
        variadic = true;
      } else {
        let ty = self.ty()?;
        params.push((name.filter(|n| n != "_"), ty));
      }

      if !self.eat_punct(",") {
        self.expect_punct(")")?;
        break;
      }
    }

    Ok((params, variadic))
  }

  fn return_type(&mut self) -> Parse<Ty> {
    if self.eat_punct("->") {
      return self.ty();
    }
    Ok(Ty::Tuple(Vec::new()))
  }

  fn struct_item(&mut self, attrs: Attrs, out: &mut Vec<Item>) -> Parse<()> {
    self.pos += 1;
    let (name, line, column) = self.ident()?;

    if self.generics()? {
      if attrs.repr.iter().any(|r| r == "C" || r == "transparent") {
        return Err(error_at(
          line,
          column,
          format!("'{name}' is generic, and a generic type has no single layout"),
        ));
      }
      return self.skip_item();
    }

    let fields = if self.eat_punct(";") {
      Some(Vec::new())
    } else if self.is_punct("(") {
      let tys = self.tuple_fields()?;
      self.where_clause()?;
      self.expect_punct(";")?;
      Some(
        tys
          .into_iter()
          .enumerate()
          .map(|(i, ty)| Field {
            name: i.to_string(),
            ty,
          })
          .collect(),
      )
    } else {
      self.where_clause()?;
      Some(self.named_fields()?)
    };

    out.push(Item::Struct {
      name,
      attrs,
      fields,
      line,
      column,
    });
    Ok(())
  }

  /// Reads a generic parameter list, if there is one, and says whether
  /// it declares any type or const parameter. Lifetimes alone leave a
  /// type with one layout, so they do not count.
  fn generics(&mut self) -> Parse<bool> {
    if !self.eat_punct("<") {
      return Ok(false);
    }

    let mut depth = 1;
    let mut typed = false;
    let mut expect_param = true;

    while depth > 0 {
      let t = self.next();
      match &t.tok {
        Tok::Punct("<") => depth += 1,
        Tok::Punct(">") => depth -= 1,
        Tok::Punct(">>") => depth -= 2,
        Tok::Punct(",") if depth == 1 => expect_param = true,
        Tok::Lifetime if depth == 1 => expect_param = false,
        Tok::Ident(_) if depth == 1 && expect_param => {
          typed = true;
          expect_param = false;
        },
        Tok::Eof => return Err(error_at(t.line, t.column, "a generic list is never closed")),
        _ => {
          if depth == 1 {
            expect_param = false;
          }
        },
      }
    }

    Ok(typed)
  }

  fn where_clause(&mut self) -> Parse<()> {
    if self.eat_ident("where") {
      while !self.is_punct("{") && !self.is_punct(";") && !matches!(self.peek().tok, Tok::Eof) {
        self.pos += 1;
      }
    }
    Ok(())
  }

  fn union_item(&mut self, attrs: Attrs, out: &mut Vec<Item>) -> Parse<()> {
    self.pos += 1;
    let (name, line, column) = self.ident()?;
    if self.generics()? {
      return Err(error_at(
        line,
        column,
        format!("'{name}' is generic, and a generic type has no single layout"),
      ));
    }
    let fields = self.named_fields()?;
    out.push(Item::Union {
      name,
      attrs,
      fields,
      line,
      column,
    });
    Ok(())
  }

  fn named_fields(&mut self) -> Parse<Vec<Field>> {
    self.expect_punct("{")?;
    let mut fields = Vec::new();

    while !self.eat_punct("}") {
      let _ = self.attributes()?;
      self.visibility()?;
      let (name, ..) = self.ident()?;
      self.expect_punct(":")?;
      let ty = self.ty()?;
      fields.push(Field { name, ty });
      if !self.eat_punct(",") {
        self.expect_punct("}")?;
        break;
      }
    }

    Ok(fields)
  }

  fn tuple_fields(&mut self) -> Parse<Vec<Ty>> {
    self.expect_punct("(")?;
    let mut tys = Vec::new();

    while !self.eat_punct(")") {
      let _ = self.attributes()?;
      self.visibility()?;
      tys.push(self.ty()?);
      if !self.eat_punct(",") {
        self.expect_punct(")")?;
        break;
      }
    }

    Ok(tys)
  }

  fn enum_item(&mut self, attrs: Attrs, out: &mut Vec<Item>) -> Parse<()> {
    self.pos += 1;
    let (name, line, column) = self.ident()?;
    if self.generics()? {
      return Err(error_at(
        line,
        column,
        format!("'{name}' is generic, and a generic type has no single layout"),
      ));
    }

    self.expect_punct("{")?;
    let mut variants = Vec::new();

    while !self.eat_punct("}") {
      let _ = self.attributes()?;
      let (vname, ..) = self.ident()?;
      let shape = if self.is_punct("(") {
        VariantShape::Tuple(self.tuple_fields()?)
      } else if self.is_punct("{") {
        VariantShape::Named(self.named_fields()?)
      } else {
        VariantShape::Unit
      };
      let discriminant = if self.eat_punct("=") {
        Some(self.expression()?)
      } else {
        None
      };
      variants.push(VariantSyntax {
        name: vname,
        shape,
        discriminant,
      });
      if !self.eat_punct(",") {
        self.expect_punct("}")?;
        break;
      }
    }

    out.push(Item::Enum {
      name,
      attrs,
      variants,
      line,
      column,
    });
    Ok(())
  }

  fn ty(&mut self) -> Parse<Ty> {
    let t = self.peek().clone();

    match &t.tok {
      Tok::Punct("*") => {
        self.pos += 1;
        let mutable = if self.eat_ident("mut") {
          true
        } else if self.eat_ident("const") {
          false
        } else {
          return self.fail("a raw pointer is *const or *mut");
        };
        Ok(Ty::Ptr {
          mutable,
          inner: Box::new(self.ty()?),
        })
      },
      Tok::Punct("&") | Tok::Punct("&&") => {
        self.pos += 1;
        if self.peek().tok == Tok::Lifetime {
          self.pos += 1;
        }
        let mutable = self.eat_ident("mut");
        let inner = self.ty()?;
        let inner = if t.tok == Tok::Punct("&&") {
          Ty::Ref {
            mutable: false,
            inner: Box::new(inner),
          }
        } else {
          inner
        };
        Ok(Ty::Ref {
          mutable,
          inner: Box::new(inner),
        })
      },
      Tok::Punct("[") => {
        self.pos += 1;
        let inner = self.ty()?;
        if self.eat_punct(";") {
          let length = self.expression()?;
          self.expect_punct("]")?;
          return Ok(Ty::Array {
            inner: Box::new(inner),
            length: Box::new(length),
          });
        }
        self.expect_punct("]")?;
        let _ = inner;
        Ok(Ty::Slice)
      },
      Tok::Punct("(") => {
        self.pos += 1;
        let mut items = Vec::new();
        while !self.eat_punct(")") {
          items.push(self.ty()?);
          if !self.eat_punct(",") {
            self.expect_punct(")")?;
            break;
          }
        }
        Ok(Ty::Tuple(items))
      },
      Tok::Punct("!") => {
        self.pos += 1;
        Ok(Ty::Never)
      },
      Tok::Ident(n) if n == "unsafe" || n == "extern" || n == "fn" || n == "for" => self.fn_type(),
      Tok::Ident(n) if n == "dyn" || n == "impl" => {
        self.fail("a trait object has no C layout; pass a pointer to a concrete type instead")
      },
      Tok::Punct("::") | Tok::Ident(_) => {
        self.eat_punct("::");
        let mut segments = Vec::new();
        let mut generics = Vec::new();
        loop {
          let (seg, ..) = self.ident()?;
          segments.push(seg);
          if self.is_punct("::") && matches!(self.peek_at(1).tok, Tok::Punct("<")) {
            self.pos += 1;
          }
          if self.eat_punct("<") {
            generics.clear();
            while !self.eat_punct(">") {
              if self.peek().tok == Tok::Lifetime {
                self.pos += 1;
              } else {
                generics.push(self.ty()?);
              }
              if !self.eat_punct(",") {
                if self.is_punct(">>") {
                  // `Option<NonNull<T>>` closes two lists with one token.
                  self.tokens[self.pos].tok = Tok::Punct(">");
                  self.tokens.insert(
                    self.pos,
                    Token {
                      tok: Tok::Punct(">"),
                      ..self.peek().clone()
                    },
                  );
                }
                self.expect_punct(">")?;
                break;
              }
            }
          }
          if !self.eat_punct("::") {
            break;
          }
        }
        Ok(Ty::Path {
          segments,
          generics,
          line: t.line,
          column: t.column,
        })
      },
      _ => Err(error_at(
        t.line,
        t.column,
        format!("expected a type, found {}", describe(&t)),
      )),
    }
  }

  fn fn_type(&mut self) -> Parse<Ty> {
    if self.eat_ident("for") {
      self.expect_punct("<")?;
      while !self.eat_punct(">") {
        self.pos += 1;
      }
    }
    self.eat_ident("unsafe");
    let mut abi = None;
    if self.eat_ident("extern") {
      abi = Some("C".to_string());
      if let Tok::Str(s) = &self.peek().tok {
        abi = Some(s.clone());
        self.pos += 1;
      }
    }
    if !self.eat_ident("fn") {
      return self.fail("expected 'fn'");
    }
    let (params, variadic) = self.params()?;
    let returns = self.return_type()?;
    Ok(Ty::Fn {
      abi,
      params: params.into_iter().map(|(_, t)| t).collect(),
      returns: Box::new(returns),
      variadic,
    })
  }

  // Constant expressions.

  fn expression(&mut self) -> Parse<Expr> {
    self.binary(0)
  }

  fn binary(&mut self, min: u8) -> Parse<Expr> {
    let mut left = self.cast_expr()?;
    loop {
      let (op, prec) = match &self.peek().tok {
        Tok::Punct(p) => match *p {
          "||" => ("||", 1),
          "&&" => ("&&", 2),
          "==" | "!=" | "<" | ">" | "<=" | ">=" => (*p, 3),
          "|" => ("|", 4),
          "^" => ("^", 5),
          "&" => ("&", 6),
          "<<" | ">>" => (*p, 7),
          "+" | "-" => (*p, 8),
          "*" | "/" | "%" => (*p, 9),
          _ => break,
        },
        _ => break,
      };
      if prec < min.max(1) {
        break;
      }
      self.pos += 1;
      let right = self.binary(prec + 1)?;
      left = Expr::Binary(op, Box::new(left), Box::new(right));
    }
    Ok(left)
  }

  fn cast_expr(&mut self) -> Parse<Expr> {
    let mut e = self.unary()?;
    while self.eat_ident("as") {
      let ty = self.ty()?;
      e = Expr::Cast(Box::new(e), ty);
    }
    Ok(e)
  }

  fn unary(&mut self) -> Parse<Expr> {
    if self.eat_punct("-") {
      return Ok(Expr::Unary("-", Box::new(self.unary()?)));
    }
    if self.eat_punct("!") {
      return Ok(Expr::Unary("!", Box::new(self.unary()?)));
    }
    self.primary()
  }

  fn primary(&mut self) -> Parse<Expr> {
    let t = self.next();
    match t.tok {
      Tok::Int(v) => Ok(Expr::Int(v)),
      Tok::Float(f) => Ok(Expr::Float(f)),
      Tok::Char(c) => Ok(Expr::Int(c as i128)),
      Tok::Str(s) => Ok(Expr::Str(s)),
      Tok::Ident(ref n) if n == "true" => Ok(Expr::Int(1)),
      Tok::Ident(ref n) if n == "false" => Ok(Expr::Int(0)),
      Tok::Punct("(") => {
        let e = self.expression()?;
        self.expect_punct(")")?;
        Ok(e)
      },
      Tok::Punct("&") => self.primary(),
      Tok::Ident(first) => {
        let mut path = vec![first];
        while self.eat_punct("::") {
          if self.eat_punct("<") {
            let ty = self.ty()?;
            self.expect_punct(">")?;
            self.expect_punct("(")?;
            self.expect_punct(")")?;
            return match path.last().unwrap().as_str() {
              "size_of" => Ok(Expr::SizeOf(true, ty)),
              "align_of" => Ok(Expr::SizeOf(false, ty)),
              other => Err(error_at(
                t.line,
                t.column,
                format!("'{other}' cannot be evaluated here"),
              )),
            };
          }
          let (seg, ..) = self.ident()?;
          path.push(seg);
        }
        // `Self::X`, `crate::X` and module prefixes do not change which
        // constant is meant; an enum's own name does.
        let meaningful: Vec<String> = path
          .into_iter()
          .filter(|p| !matches!(p.as_str(), "crate" | "self" | "super" | "Self"))
          .collect();
        let key = if meaningful.len() >= 2 {
          meaningful[meaningful.len() - 2..].join("::")
        } else {
          meaningful.join("::")
        };
        Ok(Expr::Path(key, t.line, t.column))
      },
      _ => Err(error_at(
        t.line,
        t.column,
        format!("expected a constant, found {}", describe(&t)),
      )),
    }
  }
}

fn describe(t: &Token) -> String {
  match &t.tok {
    Tok::Eof => "the end of the input".into(),
    Tok::Ident(n) => format!("'{n}'"),
    Tok::Punct(p) => format!("'{p}'"),
    Tok::Str(_) => "a string".into(),
    Tok::Lifetime => "a lifetime".into(),
    _ => "a literal".into(),
  }
}

/// Folds one attribute's tokens into `attrs`.
fn read_attribute(body: &[Token], attrs: &mut Attrs) -> Parse<()> {
  let name = match body.first().map(|t| &t.tok) {
    Some(Tok::Ident(n)) => n.clone(),
    _ => return Ok(()),
  };

  // `#[unsafe(no_mangle)]`, as the 2024 edition spells it.
  if name == "unsafe" && body.get(1).map(|t| &t.tok) == Some(&Tok::Punct("(")) && body.len() > 3 {
    return read_attribute(&body[2..body.len() - 1], attrs);
  }

  match name.as_str() {
    "no_mangle" => attrs.no_mangle = true,
    "export_name" | "link_name" => {
      let value = body.iter().find_map(|t| match &t.tok {
        Tok::Str(s) => Some(s.clone()),
        _ => None,
      });
      if name == "export_name" {
        attrs.export_name = value;
      } else {
        attrs.link_name = value;
      }
    },
    "repr" => {
      let mut i = 2;
      while i < body.len() {
        match &body[i].tok {
          Tok::Ident(r) if r == "packed" || r == "align" => {
            let mut n = if r == "packed" { Some(1) } else { None };
            if body.get(i + 1).map(|t| &t.tok) == Some(&Tok::Punct("(")) {
              if let Some(Tok::Int(v)) = body.get(i + 2).map(|t| &t.tok) {
                n = Some(*v as usize);
              }
              i += 3;
            }
            if r == "packed" {
              attrs.pack = n;
            } else {
              attrs.align = n;
            }
          },
          Tok::Ident(r) => attrs.repr.push(r.clone()),
          _ => {},
        }
        i += 1;
      }
    },
    "cfg" => {
      let mut pos = 2;
      let value = cfg(body, &mut pos)?;
      if !value {
        attrs.cfg_false = true;
      }
    },
    _ => {},
  }

  Ok(())
}

/// Evaluates a `cfg` predicate for the running platform.
fn cfg(body: &[Token], pos: &mut usize) -> Parse<bool> {
  let t = body
    .get(*pos)
    .cloned()
    .ok_or_else(|| error_at(0, 0, "an empty cfg"))?;
  let Tok::Ident(name) = &t.tok else {
    return Err(error_at(t.line, t.column, "expected a cfg predicate"));
  };
  *pos += 1;

  match name.as_str() {
    "all" | "any" | "not" => {
      *pos += 1;
      let mut results = Vec::new();
      while body.get(*pos).map(|t| &t.tok) != Some(&Tok::Punct(")")) {
        results.push(cfg(body, pos)?);
        if body.get(*pos).map(|t| &t.tok) == Some(&Tok::Punct(",")) {
          *pos += 1;
        }
      }
      *pos += 1;
      Ok(match name.as_str() {
        "all" => results.iter().all(|r| *r),
        "any" => results.iter().any(|r| *r),
        _ => !results.first().copied().unwrap_or(false),
      })
    },
    _ => {
      let value = if body.get(*pos).map(|t| &t.tok) == Some(&Tok::Punct("=")) {
        *pos += 1;
        let v = match body.get(*pos).map(|t| &t.tok) {
          Some(Tok::Str(s)) => s.clone(),
          _ => String::new(),
        };
        *pos += 1;
        Some(v)
      } else {
        None
      };

      Ok(match (name.as_str(), value.as_deref()) {
        ("unix", None) => cfg!(unix),
        ("windows", None) => cfg!(windows),
        ("target_os", Some(os)) => os == std::env::consts::OS,
        ("target_family", Some(f)) => f == std::env::consts::FAMILY,
        ("target_arch", Some(a)) => a == std::env::consts::ARCH,
        ("target_pointer_width", Some(w)) => w == "64",
        ("target_endian", Some(e)) => e == "little",
        ("target_vendor", Some(v)) => {
          (v == "apple" && cfg!(target_vendor = "apple"))
            || (v == "pc" && cfg!(windows))
            || (v == "unknown" && cfg!(target_os = "linux"))
        },
        ("target_env", Some(e)) => {
          (e == "gnu" && cfg!(target_env = "gnu"))
            || (e == "musl" && cfg!(target_env = "musl"))
            || (e == "msvc" && cfg!(target_env = "msvc"))
            || (e.is_empty() && cfg!(target_vendor = "apple"))
        },
        _ => false,
      })
    },
  }
}

// Resolution.

/// What a Rust type name refers to while the items are being resolved.
enum Named {
  Ready(TypeRef),
  Alias(Ty),
}

/// An item waiting to be filled in, by its index in the item list.
#[derive(Clone, Copy)]
enum Pending {
  Record(usize),
  FieldlessEnum(usize),
  Transparent(usize),
}

struct Resolver<'a> {
  scope: &'a mut Scope,
  items: Vec<Item>,
  names: FxHashMap<String, Named>,
  /// Types declared but not yet filled in. Rust items may refer to each
  /// other in any order, so each is filled in the first time something
  /// needs it.
  pending: FxHashMap<String, Pending>,
  /// Constants not yet evaluated, for the same reason.
  consts: FxHashMap<String, usize>,
  /// What is being resolved right now, to catch a type or constant
  /// that depends on itself.
  resolving: Vec<String>,
}

/// Reads `source` into `scope`.
pub fn declare(source: &str, scope: &mut Scope) -> Parse<()> {
  let tokens = lex(source)?;
  let mut parser = Parser { tokens, pos: 0 };
  let mut items = Vec::new();
  parser.items(&mut items, false)?;

  let mut r = Resolver {
    scope,
    items,
    names: FxHashMap::default(),
    pending: FxHashMap::default(),
    consts: FxHashMap::default(),
    resolving: Vec::new(),
  };

  // Every nominal type exists before any is filled in.
  for index in 0..r.items.len() {
    match &r.items[index] {
      Item::Struct { name, attrs, .. } => {
        let name = name.clone();
        if attrs.repr.iter().any(|r| r == "transparent") {
          r.pending.insert(name, Pending::Transparent(index));
          continue;
        }
        let record = Record::new(Some(name.clone()), false);
        if !attrs.repr.iter().any(|r| r == "C") {
          record.set_opaque(format!(
            "'{name}' has no #[repr(C)], and Rust's default layout is unspecified, so it can only be used through a pointer"
          ));
        }
        r.names.insert(
          name.clone(),
          Named::Ready(CType::new(Kind::Record(record), name.clone())),
        );
        r.pending.insert(name, Pending::Record(index));
      },
      Item::Union { name, .. } => {
        let name = name.clone();
        let record = Record::new(Some(name.clone()), true);
        r.names.insert(
          name.clone(),
          Named::Ready(CType::new(Kind::Record(record), name.clone())),
        );
        r.pending.insert(name, Pending::Record(index));
      },
      Item::Enum {
        name,
        variants,
        attrs,
        ..
      } => {
        let name = name.clone();
        let has_fields = variants
          .iter()
          .any(|v| !matches!(v.shape, VariantShape::Unit));
        if has_fields {
          let tagged_union = attrs.repr.iter().any(|r| r == "C");
          let record = Record::new(Some(name.clone()), !tagged_union);
          r.names.insert(
            name.clone(),
            Named::Ready(CType::new(Kind::Record(record), name.clone())),
          );
          r.pending.insert(name, Pending::Record(index));
        } else {
          r.pending.insert(name, Pending::FieldlessEnum(index));
        }
      },
      Item::Opaque { name } => {
        let name = name.clone();
        let record = Record::new(Some(name.clone()), false);
        record.set_opaque(format!(
          "'{name}' is an extern type, which has no size and is only used through a pointer"
        ));
        r.names.insert(
          name.clone(),
          Named::Ready(CType::new(Kind::Record(record), name)),
        );
      },
      Item::Alias { name, ty } => {
        let (name, ty) = (name.clone(), ty.clone());
        r.names.insert(name, Named::Alias(ty));
      },
      Item::Const { name, .. } => {
        let name = name.clone();
        r.consts.insert(name, index);
      },
      _ => {},
    }
  }

  // Whatever nothing needed yet is filled in now, in declaration order.
  for index in 0..r.items.len() {
    let name = match &r.items[index] {
      Item::Struct { name, .. } | Item::Union { name, .. } | Item::Enum { name, .. } => {
        Some(name.clone())
      },
      _ => None,
    };
    if let Some(name) = name {
      r.fill(&name, 0, 0)?;
    }
  }

  for index in 0..r.items.len() {
    if let Item::Const { name, .. } = &r.items[index] {
      let name = name.clone();
      r.evaluate_const(&name)?;
    }
  }

  for index in 0..r.items.len() {
    match r.items[index].clone() {
      Item::Function {
        name,
        symbol,
        abi,
        params,
        returns,
        variadic,
        line,
        column,
      } => {
        let abi = rust_abi(&abi).map_err(|e| error_at(line, column, e))?;
        let mut param_types = Vec::new();
        let mut param_names = Vec::new();
        for (pname, pty) in &params {
          let t = r.resolve(pty, false)?;
          if t.is_void() {
            continue;
          }
          param_types.push(t);
          param_names.push(pname.clone());
        }
        let returns = text_return(&r.resolve(&returns, true)?);
        let sig = Signature {
          returns,
          params: param_types,
          param_names,
          variadic,
          abi,
        };
        r.scope.add_function(FunctionDecl {
          symbol: symbol.unwrap_or_else(|| name.clone()),
          name,
          sig: Arc::new(sig),
        });
      },
      Item::Static { name, symbol, ty } => {
        let t = r.resolve(&ty, false)?;
        r.scope.add_variable(VariableDecl {
          name,
          symbol,
          ty: t,
        });
      },
      _ => {},
    }
  }

  // Everything named becomes visible to later sources.
  let names: Vec<String> = r.names.keys().cloned().collect();
  for name in names {
    let ty = r.named(&name, 0, 0)?;
    r.scope.typedefs.insert(name, ty);
  }

  Ok(())
}

fn rust_abi(abi: &str) -> Result<Abi, String> {
  match abi {
    "C" | "C-unwind" | "system" | "system-unwind" | "cdecl" | "cdecl-unwind" => Ok(Abi::Default),
    "win64" | "win64-unwind" => Abi::parse("win64"),
    "sysv64" | "sysv64-unwind" => Abi::parse("sysv64"),
    "Rust" => Err("an extern \"Rust\" function has no stable calling convention to bind".into()),
    other => Err(format!("the \"{other}\" ABI is not supported")),
  }
}

/// `*const c_char` returned from a function is text the callee keeps,
/// and converts to a string, as `const char *` does in C.
fn text_return(ty: &TypeRef) -> TypeRef {
  let Kind::Pointer(info) = &ty.kind else {
    return ty.clone();
  };
  if !info.target.is_const || info.text.is_some() {
    return ty.clone();
  }
  if matches!(
    info.target.kind,
    Kind::Int {
      size: 1,
      role: IntRole::Character,
      ..
    }
  ) {
    return CType::pointer_with(
      &info.target,
      Some(Encoding::Utf8),
      info.nonnull,
      ty.name.clone(),
    );
  }
  ty.clone()
}

impl Resolver<'_> {
  /// Fills in the type `name` if it is still waiting to be.
  fn fill(&mut self, name: &str, line: usize, column: usize) -> Parse<()> {
    let Some(pending) = self.pending.get(name).copied() else {
      return Ok(());
    };

    if self.resolving.iter().any(|n| n == name) {
      // A record reached again while it is being filled is only ever
      // reached through a pointer, which needs nothing more of it.
      if matches!(pending, Pending::Record(_)) {
        return Ok(());
      }
      return Err(error_at(line, column, format!("'{name}' contains itself")));
    }

    self.pending.remove(name);
    self.resolving.push(name.to_string());

    let result = match (
      pending,
      self.items[match pending {
        Pending::Record(i) | Pending::FieldlessEnum(i) | Pending::Transparent(i) => i,
      }]
      .clone(),
    ) {
      (
        Pending::Record(_) | Pending::Transparent(_),
        Item::Struct {
          name,
          attrs,
          fields,
          line,
          column,
        },
      ) => self.fill_struct(&name, &attrs, fields.as_deref(), line, column),
      (
        Pending::Record(_),
        Item::Union {
          name,
          attrs,
          fields,
          line,
          column,
        },
      ) => self.fill_union(&name, &attrs, &fields, line, column),
      (
        Pending::Record(_),
        Item::Enum {
          name,
          attrs,
          variants,
          line,
          column,
        },
      ) => self.fill_data_enum(&name, &attrs, &variants, line, column),
      (
        Pending::FieldlessEnum(_),
        Item::Enum {
          name,
          attrs,
          variants,
          line,
          column,
        },
      ) => self
        .fieldless_enum(&name, &attrs, &variants, line, column)
        .map(|ty| {
          self.names.insert(name.clone(), Named::Ready(ty));
        }),
      _ => Ok(()),
    };

    self.resolving.pop();
    result
  }

  /// Evaluates the constant `name` if it is still waiting to be.
  fn evaluate_const(&mut self, name: &str) -> Parse<()> {
    let Some(index) = self.consts.remove(name) else {
      return Ok(());
    };

    let Item::Const { value, .. } = self.items[index].clone() else {
      return Ok(());
    };

    if self.resolving.iter().any(|n| n == name) {
      return Err(error_at(
        0,
        0,
        format!("the constant '{name}' depends on itself"),
      ));
    }

    self.resolving.push(name.to_string());
    let constant = match &value {
      Expr::Str(s) => Ok(Constant::Str(s.clone())),
      Expr::Float(f) => Ok(Constant::Float(*f)),
      other => self.eval(other).map(Constant::Int),
    };
    self.resolving.pop();

    self.scope.add_constant(name, constant?);
    Ok(())
  }

  fn named(&mut self, name: &str, line: usize, column: usize) -> Parse<TypeRef> {
    self.fill(name, line, column)?;

    match self.names.get(name) {
      Some(Named::Ready(t)) => return Ok(t.clone()),
      Some(Named::Alias(ty)) => {
        if self.resolving.iter().any(|n| n == name) {
          return Err(error_at(
            line,
            column,
            format!("the type alias '{name}' refers to itself"),
          ));
        }
        let ty = ty.clone();
        self.resolving.push(name.to_string());
        let resolved = self.resolve(&ty, false);
        self.resolving.pop();
        let resolved = CType::renamed(&resolved?, name.to_string());
        self
          .names
          .insert(name.to_string(), Named::Ready(resolved.clone()));
        return Ok(resolved);
      },
      None => {},
    }

    if let Some(t) = self.scope.typedef(name) {
      return Ok(t);
    }

    if let Some(t) = primitive(name) {
      return Ok(t);
    }

    Err(error_at(line, column, format!("unknown type '{name}'")))
  }

  fn resolve(&mut self, ty: &Ty, returning: bool) -> Parse<TypeRef> {
    match ty {
      Ty::Tuple(items) if items.is_empty() => Ok(builtin("void").unwrap()),
      Ty::Never => Ok(builtin("void").unwrap()),
      Ty::Tuple(_) => Err(error_at(
        0,
        0,
        "a tuple has no C layout; declare a #[repr(C)] struct instead",
      )),
      Ty::Slice => Err(error_at(
        0,
        0,
        "a slice is not FFI-safe; pass a pointer and a length, or use ffi.slice() for a (ptr, len) struct",
      )),
      Ty::Ptr { mutable, inner } => {
        let target = self.pointee(inner)?;
        let target = if *mutable {
          target
        } else {
          CType::constant(&target)
        };
        let name = super::types::pointer_name(&target);
        Ok(CType::pointer_with(&target, None, false, name))
      },
      Ty::Ref { mutable, inner } => {
        if let Ty::Path { segments, .. } = inner.as_ref()
          && segments.last().is_some_and(|s| s == "str")
        {
          return Err(error_at(
            0,
            0,
            "&str is not FFI-safe; pass *const u8 and a length instead",
          ));
        }
        if let Ty::Slice = inner.as_ref() {
          return self.resolve(inner, returning);
        }
        let target = self.pointee(inner)?;
        let target = if *mutable {
          target
        } else {
          CType::constant(&target)
        };
        let name = super::types::pointer_name(&target);
        Ok(CType::pointer_with(&target, None, true, name))
      },
      Ty::Array { inner, length } => {
        let element = self.resolve(inner, false)?;
        let n = self.eval(length)?;
        if n < 0 {
          return Err(error_at(0, 0, "an array cannot have a negative length"));
        }
        Ok(CType::array_of(&element, Some(n as usize)))
      },
      Ty::Fn {
        abi,
        params,
        returns,
        variadic,
      } => {
        let sig = self.fn_signature(abi.as_deref(), params, returns, *variadic)?;
        let function = CType::function(sig);
        let name = super::types::pointer_name(&function);
        Ok(CType::pointer_with(&function, None, true, name))
      },
      Ty::Path {
        segments,
        generics,
        line,
        column,
      } => {
        let last = segments.last().unwrap().as_str();
        match (last, generics.as_slice()) {
          ("Option", [inner]) => self.optional(inner, *line, *column),
          ("NonNull", [inner]) | ("Box", [inner]) => {
            let target = self.pointee(inner)?;
            let name = super::types::pointer_name(&target);
            Ok(CType::pointer_with(&target, None, true, name))
          },
          (
            "MaybeUninit" | "ManuallyDrop" | "Cell" | "UnsafeCell" | "Wrapping" | "AtomicPtr",
            [inner],
          ) => {
            if last == "AtomicPtr" {
              let target = self.pointee(inner)?;
              return Ok(CType::pointer_to(&target));
            }
            self.resolve(inner, returning)
          },
          ("PhantomData", _) | ("PhantomPinned", _) => {
            // Zero-sized, so it takes up no space in a struct.
            let record = Record::new(Some("PhantomData".into()), false);
            record.mark_defined();
            Ok(CType::new(Kind::Record(record), "PhantomData"))
          },
          ("String" | "Vec" | "str" | "HashMap" | "Rc" | "Arc" | "RefCell", _) => Err(error_at(
            *line,
            *column,
            format!("'{last}' has no stable layout and cannot cross the C ABI"),
          )),
          (_, []) => self.named(last, *line, *column),
          _ => Err(error_at(
            *line,
            *column,
            format!("'{last}' is generic, and a generic type has no single layout"),
          )),
        }
      },
    }
  }

  /// A pointer's target, where `c_void` and an unknown extern type are
  /// both fine.
  fn pointee(&mut self, ty: &Ty) -> Parse<TypeRef> {
    self.resolve(ty, false)
  }

  /// `Option<T>` where the niche makes it the same size as `T`: a
  /// reference, `NonNull`, `Box`, a function pointer, or a `NonZero`
  /// integer.
  fn optional(&mut self, inner: &Ty, line: usize, column: usize) -> Parse<TypeRef> {
    let t = self.resolve(inner, false)?;
    match &t.kind {
      Kind::Pointer(info) if info.nonnull => Ok(CType::pointer_with(
        &info.target,
        info.text,
        false,
        t.name.clone(),
      )),
      Kind::Int {
        size,
        signed,
        role: IntRole::NonZero,
      } => Ok(CType::new(
        Kind::Int {
          size: *size,
          signed: *signed,
          role: IntRole::OptionalNonZero,
        },
        format!("Option<{}>", t.name),
      )),
      _ => Err(error_at(
        line,
        column,
        format!(
          "Option<{}> has no guaranteed C layout; only references, NonNull, Box, function pointers and NonZero integers do",
          t.name
        ),
      )),
    }
  }

  fn fn_signature(
    &mut self,
    abi: Option<&str>,
    params: &[Ty],
    returns: &Ty,
    variadic: bool,
  ) -> Parse<Signature> {
    let abi = match abi {
      None => {
        return Err(error_at(
          0,
          0,
          "a Rust fn pointer has no stable calling convention; declare it extern \"C\" fn",
        ));
      },
      Some(a) => rust_abi(a).map_err(|e| error_at(0, 0, e))?,
    };
    let mut types = Vec::new();
    for p in params {
      let t = self.resolve(p, false)?;
      if !t.is_void() {
        types.push(t);
      }
    }
    let count = types.len();
    Ok(Signature {
      returns: self.resolve(returns, true)?,
      params: types,
      param_names: vec![None; count],
      variadic,
      abi,
    })
  }

  fn eval(&mut self, e: &Expr) -> Parse<i128> {
    Ok(match e {
      Expr::Int(v) => *v,
      Expr::Float(f) => *f as i128,
      Expr::Str(_) => return Err(error_at(0, 0, "a string is not a number")),
      Expr::Path(name, line, column) => match {
        self.evaluate_const(name)?;
        if let Some(last) = name.rsplit("::").next() {
          self.evaluate_const(last)?;
        }
        self.scope.constant(name).or_else(|| {
          name
            .rsplit("::")
            .next()
            .and_then(|last| self.scope.constant(last))
        })
      } {
        Some(Constant::Int(v)) => v,
        Some(Constant::Float(f)) => f as i128,
        _ => {
          return Err(error_at(
            *line,
            *column,
            format!("'{name}' is not a known constant"),
          ));
        },
      },
      Expr::Unary(op, inner) => {
        let v = self.eval(inner)?;
        if *op == "-" { v.wrapping_neg() } else { !v }
      },
      Expr::Binary(op, l, r) => {
        let a = self.eval(l)?;
        let b = self.eval(r)?;
        match *op {
          "+" => a.wrapping_add(b),
          "-" => a.wrapping_sub(b),
          "*" => a.wrapping_mul(b),
          "/" | "%" if b == 0 => return Err(error_at(0, 0, "division by zero in a constant")),
          "/" => a / b,
          "%" => a % b,
          "<<" => a.wrapping_shl(b as u32),
          ">>" => a.wrapping_shr(b as u32),
          "&" => a & b,
          "|" => a | b,
          "^" => a ^ b,
          "==" => (a == b) as i128,
          "!=" => (a != b) as i128,
          "<" => (a < b) as i128,
          ">" => (a > b) as i128,
          "<=" => (a <= b) as i128,
          ">=" => (a >= b) as i128,
          "&&" => (a != 0 && b != 0) as i128,
          "||" => (a != 0 || b != 0) as i128,
          _ => return Err(error_at(0, 0, format!("'{op}' cannot be evaluated here"))),
        }
      },
      Expr::Cast(inner, ty) => {
        let v = self.eval(inner)?;
        let t = self.resolve(ty, false)?;
        match t.kind {
          Kind::Int { size, signed, .. } if size < 16 => {
            let bits = size * 8;
            let mask = (1i128 << bits) - 1;
            let m = v & mask;
            if signed && m >> (bits - 1) & 1 == 1 {
              m | !mask
            } else {
              m
            }
          },
          _ => v,
        }
      },
      Expr::SizeOf(size, ty) => {
        let t = self.resolve(ty, false)?;
        let v = if *size {
          t.require_size()
        } else {
          t.require_align()
        };
        v.map_err(|e| error_at(0, 0, e))? as i128
      },
    })
  }

  fn fieldless_enum(
    &mut self,
    name: &str,
    attrs: &Attrs,
    variants: &[VariantSyntax],
    line: usize,
    column: usize,
  ) -> Parse<TypeRef> {
    let underlying = self.enum_repr(name, attrs, line, column, false)?;
    let mut constants = Vec::new();
    let mut next = 0i128;

    for v in variants {
      if let Some(d) = &v.discriminant {
        next = self.eval(d)?;
      }
      constants.push((v.name.clone(), next));
      self
        .scope
        .add_constant(&format!("{name}::{}", v.name), Constant::Int(next));
      next = next.wrapping_add(1);
    }

    let info = Arc::new(EnumInfo {
      underlying,
      constants: Mutex::new(constants),
    });

    Ok(CType::new(Kind::Enum(info), name.to_string()))
  }

  /// The integer type an enum's discriminant is stored in.
  fn enum_repr(
    &self,
    name: &str,
    attrs: &Attrs,
    line: usize,
    column: usize,
    data: bool,
  ) -> Parse<TypeRef> {
    for r in &attrs.repr {
      if let Some(t) = primitive(r)
        && matches!(t.kind, Kind::Int { .. })
      {
        return Ok(t);
      }
    }

    if attrs.repr.iter().any(|r| r == "C") {
      return Ok(builtin("int").unwrap());
    }

    Err(error_at(
      line,
      column,
      format!(
        "'{name}' has no #[repr]; {} Rust's default enum layout is unspecified",
        if data {
          "an enum with fields needs #[repr(C)] or #[repr(u8)] and the like, because"
        } else {
          "give it #[repr(C)] or an integer repr, because"
        }
      ),
    ))
  }

  fn record_of(&mut self, name: &str) -> Arc<Record> {
    match self.names.get(name) {
      Some(Named::Ready(t)) if t.record().is_some() => t.record().unwrap().clone(),
      _ => unreachable!("every record was created in the first pass"),
    }
  }

  fn fill_struct(
    &mut self,
    name: &str,
    attrs: &Attrs,
    fields: Option<&[Field]>,
    line: usize,
    column: usize,
  ) -> Parse<()> {
    if attrs.repr.iter().any(|r| r == "transparent") {
      let Some(fields) = fields else {
        return Ok(());
      };
      let mut inner = None;
      for f in fields {
        let t = self.resolve(&f.ty, false)?;
        if t.size() != Some(0) {
          inner = Some(t);
        }
      }
      let Some(inner) = inner else {
        return Err(error_at(
          line,
          column,
          format!("'{name}' is transparent but holds nothing"),
        ));
      };
      self.names.insert(
        name.to_string(),
        Named::Ready(CType::renamed(&inner, name.to_string())),
      );
      return Ok(());
    }

    let record = self.record_of(name);

    if !attrs.repr.iter().any(|r| r == "C") {
      // Only usable behind a pointer, which is what Rust's own layout
      // allows.
      return Ok(());
    }

    let Some(fields) = fields else {
      return Ok(());
    };

    for f in fields {
      let ty = self.resolve(&f.ty, false)?;
      record
        .add_field(FieldSpec {
          name: f.name.clone(),
          ty,
          bits: None,
          align: None,
        })
        .map_err(|e| error_at(line, column, e))?;
    }
    record.mark_defined();

    if let Some(p) = attrs.pack {
      record.set_pack(p).map_err(|e| error_at(line, column, e))?;
    }
    if let Some(a) = attrs.align {
      record.set_align(a).map_err(|e| error_at(line, column, e))?;
    }
    Ok(())
  }

  fn fill_union(
    &mut self,
    name: &str,
    attrs: &Attrs,
    fields: &[Field],
    line: usize,
    column: usize,
  ) -> Parse<()> {
    let record = self.record_of(name);

    if !attrs.repr.iter().any(|r| r == "C") {
      record.set_opaque(format!(
        "'{name}' has no #[repr(C)], and Rust's default layout is unspecified, so it can only be used through a pointer"
      ));
      return Ok(());
    }

    for f in fields {
      let ty = self.resolve(&f.ty, false)?;
      record
        .add_field(FieldSpec {
          name: f.name.clone(),
          ty,
          bits: None,
          align: None,
        })
        .map_err(|e| error_at(line, column, e))?;
    }
    record.mark_defined();

    if let Some(p) = attrs.pack {
      record.set_pack(p).map_err(|e| error_at(line, column, e))?;
    }
    if let Some(a) = attrs.align {
      record.set_align(a).map_err(|e| error_at(line, column, e))?;
    }
    Ok(())
  }

  fn fill_data_enum(
    &mut self,
    name: &str,
    attrs: &Attrs,
    variants: &[VariantSyntax],
    line: usize,
    column: usize,
  ) -> Parse<()> {
    let tag = self.enum_repr(name, attrs, line, column, true)?;
    let tagged_union = attrs.repr.iter().any(|r| r == "C");
    let record = self.record_of(name);
    let at = |e: String| error_at(line, column, e);

    let mut cases = Vec::new();
    let mut next = 0i128;

    let payload_union = Record::new(Some(format!("{name}::payload")), true);

    for v in variants {
      if let Some(d) = &v.discriminant {
        next = self.eval(d)?;
      }

      let fields: Vec<(String, TypeRef)> = match &v.shape {
        VariantShape::Unit => Vec::new(),
        VariantShape::Tuple(tys) => {
          let mut out = Vec::new();
          for (i, t) in tys.iter().enumerate() {
            out.push((i.to_string(), self.resolve(t, false)?));
          }
          out
        },
        VariantShape::Named(fs) => {
          let mut out = Vec::new();
          for f in fs {
            out.push((f.name.clone(), self.resolve(&f.ty, false)?));
          }
          out
        },
      };

      let payload = if fields.is_empty() && tagged_union {
        None
      } else {
        let variant_record = Record::new(Some(format!("{name}::{}", v.name)), false);
        if !tagged_union {
          variant_record
            .add_field(FieldSpec {
              name: "@tag".into(),
              ty: tag.clone(),
              bits: None,
              align: None,
            })
            .map_err(at)?;
        }
        for (fname, fty) in &fields {
          variant_record
            .add_field(FieldSpec {
              name: fname.clone(),
              ty: fty.clone(),
              bits: None,
              align: None,
            })
            .map_err(at)?;
        }
        variant_record.mark_defined();
        let ty = CType::new(Kind::Record(variant_record), format!("{name}::{}", v.name));

        if tagged_union {
          payload_union
            .add_field(FieldSpec {
              name: v.name.clone(),
              ty: ty.clone(),
              bits: None,
              align: None,
            })
            .map_err(at)?;
        } else {
          record
            .add_field(FieldSpec {
              name: v.name.clone(),
              ty: ty.clone(),
              bits: None,
              align: None,
            })
            .map_err(at)?;
        }
        Some(ty)
      };

      cases.push(Variant {
        name: v.name.clone(),
        discriminant: next,
        payload,
      });
      self
        .scope
        .add_constant(&format!("{name}::{}", v.name), Constant::Int(next));
      next = next.wrapping_add(1);
    }

    if tagged_union {
      payload_union.mark_defined();
      record
        .add_field(FieldSpec {
          name: "tag".into(),
          ty: tag.clone(),
          bits: None,
          align: None,
        })
        .map_err(at)?;
      record
        .add_field(FieldSpec {
          name: "payload".into(),
          ty: CType::new(Kind::Record(payload_union), format!("{name}::payload")),
          bits: None,
          align: None,
        })
        .map_err(at)?;
    } else {
      record
        .add_field(FieldSpec {
          name: "@tag".into(),
          ty: tag.clone(),
          bits: None,
          align: None,
        })
        .map_err(at)?;
    }
    record.mark_defined();

    if let Some(a) = attrs.align {
      record.set_align(a).map_err(at)?;
    }

    let _ = record.variants.set(Some(Variants {
      tag,
      cases,
      tagged_union,
    }));

    Ok(())
  }
}

/// Rust's primitive and `core::ffi` type names.
fn primitive(name: &str) -> Option<TypeRef> {
  let c = |n: &str| builtin(n);
  match name {
    "i8" | "u8" | "i16" | "u16" | "i32" | "u32" | "i64" | "u64" | "i128" | "u128" | "isize"
    | "usize" | "f32" | "f64" | "bool" => c(name),
    "char" => c("rust char"),
    "c_void" => c("void"),
    "c_char" => c("char"),
    "c_schar" => c("signed char"),
    "c_uchar" => c("unsigned char"),
    "c_short" => c("short"),
    "c_ushort" => c("unsigned short"),
    "c_int" => c("int"),
    "c_uint" => c("unsigned int"),
    "c_long" => c("long"),
    "c_ulong" => c("unsigned long"),
    "c_longlong" => c("long long"),
    "c_ulonglong" => c("unsigned long long"),
    "c_float" => c("float"),
    "c_double" => c("double"),
    "size_t" | "c_size_t" => c("size_t"),
    "ssize_t" | "c_ssize_t" => c("ssize_t"),
    "ptrdiff_t" | "c_ptrdiff_t" => c("ptrdiff_t"),
    "intptr_t" => c("intptr_t"),
    "uintptr_t" => c("uintptr_t"),
    "wchar_t" => c("wchar_t"),
    n if n.starts_with("NonZero") => {
      let inner = n.trim_start_matches("NonZero").to_lowercase();
      let base = builtin(&inner)?;
      let Kind::Int { size, signed, .. } = base.kind else {
        return None;
      };
      Some(CType::new(
        Kind::Int {
          size,
          signed,
          role: IntRole::NonZero,
        },
        n,
      ))
    },
    n if n.starts_with("Atomic") && n != "AtomicPtr" => {
      let inner = n.trim_start_matches("Atomic").to_lowercase();
      builtin(&inner)
    },
    _ => None,
  }
}
