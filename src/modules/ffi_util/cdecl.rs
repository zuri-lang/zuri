//! Reading C declarations.
//!
//! The input is what a header says about an API: typedefs, structs,
//! unions, enums, function prototypes and `extern` variables. Enough of
//! the preprocessor runs for real header text to read as it was written:
//! object-like `#define`s are expanded and become constants,
//! `#if`/`#ifdef` and friends are evaluated with the target platform's
//! predefined macros, `#pragma pack` is honoured, and including one of
//! the C standard headers is accepted because the types they declare are
//! already known. Including any other file is refused, as is expanding a
//! function-like macro; declarations are read as given, never fetched.

use std::sync::Arc;

use super::declare::{Constant, FunctionDecl, Macro, Scope, VariableDecl};
use super::types::{
  Abi, CType, Encoding, EnumInfo, FieldSpec, IntRole, Kind, LONG_SIZE, Record, Signature, TypeRef,
  builtin,
};

#[derive(Debug)]
pub struct SyntaxError {
  pub message: String,
  pub line: usize,
  pub column: usize,
}

type Parse<T> = Result<T, SyntaxError>;

#[derive(Clone, Debug, PartialEq)]
pub enum Tok {
  Ident(String),
  Int(i128, bool),
  Float(f64),
  Str(String),
  Char(i128),
  Punct(&'static str),
  /// Where a `#pragma pack` changed the packing, so records declared
  /// after it, and only those, see the new value. Zero restores the
  /// default.
  Pack(usize),
  Eof,
}

#[derive(Clone, Debug)]
pub struct Token {
  pub tok: Tok,
  pub line: usize,
  pub column: usize,
  /// The first token on its line, which is what makes a `#` a directive.
  pub line_start: bool,
}

const PUNCTUATORS: &[&str] = &[
  "...", "<<=", ">>=", "->", "++", "--", "<<", ">>", "<=", ">=", "==", "!=", "&&", "||", "+=",
  "-=", "*=", "/=", "%=", "&=", "|=", "^=", "##", "::", "{", "}", "(", ")", "[", "]", ";", ",",
  ":", "=", "*", "&", "|", "^", "~", "!", "?", "<", ">", "+", "-", "/", "%", ".", "#",
];

fn error_at(line: usize, column: usize, message: impl Into<String>) -> SyntaxError {
  SyntaxError {
    message: message.into(),
    line,
    column,
  }
}

// Lexing.

pub fn lex(source: &str) -> Parse<Vec<Token>> {
  let chars: Vec<char> = source.chars().collect();
  let mut out = Vec::new();
  let mut i = 0;
  let mut line = 1;
  let mut column = 1;
  let mut line_start = true;

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

    // A backslash before a newline joins the two lines.
    if c == '\\' && chars.get(i + 1) == Some(&'\n') {
      i += 2;
      line += 1;
      column = 1;
      continue;
    }
    if c == '\\' && chars.get(i + 1) == Some(&'\r') && chars.get(i + 2) == Some(&'\n') {
      i += 3;
      line += 1;
      column = 1;
      continue;
    }

    if c == '\n' {
      advance!();
      line_start = true;
      continue;
    }

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
      advance!();
      advance!();
      loop {
        if i >= chars.len() {
          return Err(error_at(l, col, "a comment is never closed"));
        }
        if chars[i] == '*' && chars.get(i + 1) == Some(&'/') {
          advance!();
          advance!();
          break;
        }
        advance!();
      }
      continue;
    }

    let (tok_line, tok_column) = (line, column);
    let starts_line = line_start;
    line_start = false;

    let push = |out: &mut Vec<Token>, tok: Tok| {
      out.push(Token {
        tok,
        line: tok_line,
        column: tok_column,
        line_start: starts_line,
      })
    };

    // String and character literals, with their encoding prefixes.
    let prefix_len = if c == 'L' || c == 'U' {
      1
    } else if c == 'u' {
      if chars.get(i + 1) == Some(&'8') { 2 } else { 1 }
    } else {
      0
    };

    let quote_at = i + prefix_len;
    let is_literal = matches!(chars.get(quote_at), Some('"') | Some('\''))
      && (prefix_len == 0 || c == 'L' || c == 'U' || c == 'u');

    if is_literal {
      for _ in 0..prefix_len {
        advance!();
      }
      let quote = chars[i];
      advance!();
      let mut text = String::new();
      loop {
        let Some(&ch) = chars.get(i) else {
          return Err(error_at(tok_line, tok_column, "a literal is never closed"));
        };
        if ch == '\n' {
          return Err(error_at(
            tok_line,
            tok_column,
            "a literal runs past the end of its line",
          ));
        }
        if ch == quote {
          advance!();
          break;
        }
        if ch == '\\' {
          advance!();
          let Some(&esc) = chars.get(i) else {
            return Err(error_at(line, column, "an escape is cut off"));
          };
          advance!();
          let decoded = match esc {
            'n' => '\n',
            't' => '\t',
            'r' => '\r',
            '0'..='7' => {
              let mut value = esc.to_digit(8).unwrap();
              for _ in 0..2 {
                match chars.get(i).and_then(|d| d.to_digit(8)) {
                  Some(d) => {
                    value = value * 8 + d;
                    advance!();
                  },
                  None => break,
                }
              }
              char::from_u32(value).unwrap_or('\u{fffd}')
            },
            'x' => {
              let mut value = 0u32;
              while let Some(d) = chars.get(i).and_then(|d| d.to_digit(16)) {
                value = value.wrapping_mul(16).wrapping_add(d);
                advance!();
              }
              char::from_u32(value).unwrap_or('\u{fffd}')
            },
            'a' => '\u{7}',
            'b' => '\u{8}',
            'f' => '\u{c}',
            'v' => '\u{b}',
            'e' => '\u{1b}',
            other => other,
          };
          text.push(decoded);
          continue;
        }
        text.push(ch);
        advance!();
      }

      if quote == '"' {
        push(&mut out, Tok::Str(text));
      } else {
        // A multi-character constant packs its characters into an int,
        // first character highest, the way GCC and Clang do.
        let value = if text.chars().count() == 1 {
          text.chars().next().unwrap() as i128
        } else {
          text
            .chars()
            .fold(0i128, |v, ch| (v << 8) | (ch as i128 & 0xff))
        };
        push(&mut out, Tok::Char(value));
      }
      continue;
    }

    if c.is_ascii_alphabetic() || c == '_' || c == '$' {
      let mut name = String::new();
      while let Some(&ch) = chars.get(i) {
        if ch.is_ascii_alphanumeric() || ch == '_' || ch == '$' {
          name.push(ch);
          advance!();
        } else {
          break;
        }
      }
      push(&mut out, Tok::Ident(name));
      continue;
    }

    if c.is_ascii_digit() || (c == '.' && chars.get(i + 1).is_some_and(|d| d.is_ascii_digit())) {
      let mut text = String::new();
      while let Some(&ch) = chars.get(i) {
        let exponent_sign = (ch == '+' || ch == '-')
          && matches!(
            text.chars().last(),
            Some('e') | Some('E') | Some('p') | Some('P')
          )
          && !text.starts_with("0x")
          && !text.starts_with("0X")
          || ((ch == '+' || ch == '-') && matches!(text.chars().last(), Some('p') | Some('P')));
        if ch.is_ascii_alphanumeric() || ch == '.' || ch == '\'' || exponent_sign {
          if ch != '\'' {
            text.push(ch);
          }
          advance!();
        } else {
          break;
        }
      }
      let tok = number(&text)
        .ok_or_else(|| error_at(tok_line, tok_column, format!("'{text}' is not a number")))?;
      push(&mut out, tok);
      continue;
    }

    let rest: String = chars[i..chars.len().min(i + 3)].iter().collect();
    let Some(p) = PUNCTUATORS.iter().find(|p| rest.starts_with(**p)) else {
      return Err(error_at(
        tok_line,
        tok_column,
        format!("unexpected character '{c}'"),
      ));
    };

    for _ in 0..p.chars().count() {
      advance!();
    }
    push(&mut out, Tok::Punct(p));
  }

  out.push(Token {
    tok: Tok::Eof,
    line,
    column,
    line_start: true,
  });
  Ok(out)
}

fn number(text: &str) -> Option<Tok> {
  let lower = text.to_ascii_lowercase();
  let is_hex = lower.starts_with("0x");
  let is_float = if is_hex {
    lower.contains('p') || lower.contains('.')
  } else {
    lower.contains('.') || lower.contains('e')
  };

  if is_float {
    let trimmed = lower.trim_end_matches(['f', 'l']);
    if is_hex {
      return hex_float(trimmed).map(Tok::Float);
    }
    return trimmed.parse::<f64>().ok().map(Tok::Float);
  }

  let digits_end = lower
    .find(|c: char| matches!(c, 'u' | 'l') || (!is_hex && c == 'i'))
    .unwrap_or(lower.len());
  let (digits, suffix) = lower.split_at(digits_end);
  let unsigned = suffix.contains('u');

  let value = if let Some(hex) = digits.strip_prefix("0x") {
    u128::from_str_radix(hex, 16).ok()?
  } else if let Some(bin) = digits.strip_prefix("0b") {
    u128::from_str_radix(bin, 2).ok()?
  } else if digits.len() > 1 && digits.starts_with('0') {
    u128::from_str_radix(&digits[1..], 8).ok()?
  } else {
    digits.parse::<u128>().ok()?
  };

  Some(Tok::Int(value as i128, unsigned))
}

fn hex_float(text: &str) -> Option<f64> {
  let body = text.strip_prefix("0x")?;
  let (mantissa, exponent) = match body.find('p') {
    Some(p) => (&body[..p], body[p + 1..].parse::<i32>().ok()?),
    None => (body, 0),
  };
  let (whole, frac) = match mantissa.find('.') {
    Some(d) => (&mantissa[..d], &mantissa[d + 1..]),
    None => (mantissa, ""),
  };
  let mut value = 0f64;
  for c in whole.chars() {
    value = value * 16.0 + c.to_digit(16)? as f64;
  }
  let mut scale = 1.0 / 16.0;
  for c in frac.chars() {
    value += c.to_digit(16)? as f64 * scale;
    scale /= 16.0;
  }
  Some(value * 2f64.powi(exponent))
}

// Preprocessing.

/// The standard headers whose contents are already known.
const STANDARD_HEADERS: &[&str] = &[
  "assert.h",
  "complex.h",
  "ctype.h",
  "errno.h",
  "float.h",
  "inttypes.h",
  "iso646.h",
  "limits.h",
  "locale.h",
  "math.h",
  "setjmp.h",
  "signal.h",
  "stdalign.h",
  "stdarg.h",
  "stdatomic.h",
  "stdbool.h",
  "stddef.h",
  "stdint.h",
  "stdio.h",
  "stdlib.h",
  "stdnoreturn.h",
  "string.h",
  "tgmath.h",
  "threads.h",
  "time.h",
  "uchar.h",
  "wchar.h",
  "wctype.h",
  "sys/types.h",
  "cstdint",
  "cstddef",
  "cstdlib",
  "cstdio",
  "cstring",
  "cstdarg",
];

/// Macros every platform's compiler defines that headers test for.
fn predefined(name: &str) -> Option<Vec<Tok>> {
  let one = || Some(vec![Tok::Int(1, false)]);

  match name {
    "__CHAR_BIT__" => Some(vec![Tok::Int(8, false)]),
    "__STDC__" | "__STDC_HOSTED__" => one(),
    "__STDC_VERSION__" => Some(vec![Tok::Int(201710, false)]),
    "__SIZEOF_POINTER__" => Some(vec![Tok::Int(8, false)]),
    "__SIZEOF_LONG__" => Some(vec![Tok::Int(LONG_SIZE as i128, false)]),
    "__SIZEOF_INT__" => Some(vec![Tok::Int(4, false)]),
    "__SIZEOF_LONG_LONG__" => Some(vec![Tok::Int(8, false)]),
    "_WIN32" | "_WIN64" if cfg!(windows) => one(),
    "_MSC_VER" if cfg!(windows) => Some(vec![Tok::Int(1930, false)]),
    "_M_X64" | "_M_AMD64" if cfg!(all(windows, target_arch = "x86_64")) => one(),
    "__GNUC__" if cfg!(not(windows)) => Some(vec![Tok::Int(4, false)]),
    "__unix__" | "__unix" if cfg!(all(unix, not(target_vendor = "apple"))) => one(),
    "__linux__" | "__linux" | "linux" if cfg!(target_os = "linux") => one(),
    "__gnu_linux__" if cfg!(all(target_os = "linux", target_env = "gnu")) => one(),
    "__APPLE__" | "__MACH__" if cfg!(target_vendor = "apple") => one(),
    "__x86_64__" | "__x86_64" | "__amd64__" | "__amd64" if cfg!(target_arch = "x86_64") => one(),
    "__aarch64__" | "__arm64__" if cfg!(target_arch = "aarch64") => one(),
    "__LP64__" | "_LP64" if cfg!(not(windows)) => one(),
    "__LITTLE_ENDIAN__" => one(),
    "__ZURI_FFI__" => one(),
    _ => None,
  }
}

struct Conditional {
  /// This branch's lines are being read.
  active: bool,
  /// Some branch of this `#if` has already been taken.
  taken: bool,
  /// The enclosing region is being read at all.
  outer: bool,
}

pub fn preprocess(tokens: Vec<Token>, scope: &mut Scope) -> Parse<Vec<Token>> {
  let mut out = Vec::new();
  let mut stack: Vec<Conditional> = Vec::new();
  let mut i = 0;

  let active = |stack: &Vec<Conditional>| stack.last().is_none_or(|c| c.active);

  while i < tokens.len() {
    let token = &tokens[i];

    if matches!(token.tok, Tok::Eof) {
      if let Some(_open) = stack.last() {
        return Err(error_at(
          token.line,
          token.column,
          "an #if is never closed with #endif",
        ));
      }
      out.push(token.clone());
      break;
    }

    if token.line_start && token.tok == Tok::Punct("#") {
      let mut end = i + 1;
      while end < tokens.len() && !tokens[end].line_start {
        end += 1;
      }
      let line: Vec<Token> = tokens[i + 1..end].to_vec();
      let here = token.clone();
      i = end;

      let Some(first) = line.first() else {
        continue;
      };
      let Tok::Ident(directive) = &first.tok else {
        if active(&stack) {
          return Err(error_at(
            first.line,
            first.column,
            "expected a preprocessor directive",
          ));
        }
        continue;
      };

      match directive.as_str() {
        "if" | "ifdef" | "ifndef" => {
          let outer = active(&stack);
          let value = if !outer {
            false
          } else if directive == "if" {
            eval_condition(&line[1..], scope, &here)?
          } else {
            let Some(Tok::Ident(name)) = line.get(1).map(|t| &t.tok) else {
              return Err(error_at(
                here.line,
                here.column,
                format!("#{directive} needs a macro name"),
              ));
            };
            let defined = is_defined(scope, name);
            if directive == "ifdef" {
              defined
            } else {
              !defined
            }
          };
          stack.push(Conditional {
            active: outer && value,
            taken: value,
            outer,
          });
        },
        "elif" | "elifdef" | "elifndef" => {
          let Some(top) = stack.last() else {
            return Err(error_at(
              here.line,
              here.column,
              format!("#{directive} without #if"),
            ));
          };
          let (outer, taken) = (top.outer, top.taken);
          let value = if !outer || taken {
            false
          } else if directive == "elif" {
            eval_condition(&line[1..], scope, &here)?
          } else {
            let Some(Tok::Ident(name)) = line.get(1).map(|t| &t.tok) else {
              return Err(error_at(
                here.line,
                here.column,
                format!("#{directive} needs a macro name"),
              ));
            };
            let defined = is_defined(scope, name);
            if directive == "elifdef" {
              defined
            } else {
              !defined
            }
          };
          let top = stack.last_mut().unwrap();
          top.active = outer && !taken && value;
          top.taken = taken || value;
        },
        "else" => {
          let Some(top) = stack.last_mut() else {
            return Err(error_at(here.line, here.column, "#else without #if"));
          };
          top.active = top.outer && !top.taken;
          top.taken = true;
        },
        "endif" => {
          if stack.pop().is_none() {
            return Err(error_at(here.line, here.column, "#endif without #if"));
          }
        },
        _ if !active(&stack) => {},
        "define" => {
          let Some(Tok::Ident(name)) = line.get(1).map(|t| t.tok.clone()) else {
            return Err(error_at(
              here.line,
              here.column,
              "#define needs a macro name",
            ));
          };
          let name_token = &line[1];
          // A parenthesis touching the name makes it function-like.
          let function_like = line.get(2).is_some_and(|t| {
            t.tok == Tok::Punct("(")
              && t.line == name_token.line
              && t.column == name_token.column + name.len()
          });
          let body = if function_like {
            Macro::Function
          } else {
            Macro::Object(line[2..].to_vec())
          };
          if let Macro::Object(ref tokens) = body
            && let Some(value) = constant_of(tokens, scope)
          {
            scope.add_constant(&name, value);
          }
          scope.macros.insert(name, body);
        },
        "undef" => {
          if let Some(Tok::Ident(name)) = line.get(1).map(|t| &t.tok) {
            scope.macros.remove(name);
            scope.constants.retain(|(n, _)| n != name);
          }
        },
        "include" | "include_next" | "import" => {
          let header = include_name(&line[1..]);
          let known = header
            .as_deref()
            .is_some_and(|h| STANDARD_HEADERS.contains(&h));
          if !known {
            return Err(error_at(
              here.line,
              here.column,
              format!(
                "cannot include {}: declarations are read as given, so paste in the ones the program needs",
                header
                  .map(|h| format!("'{h}'"))
                  .unwrap_or_else(|| "a file".into())
              ),
            ));
          }
          if header.as_deref() == Some("stdio.h") && scope.type_name("FILE").is_none() {
            let record = Record::new(Some("_IO_FILE".into()), false);
            let ty = CType::new(Kind::Record(record), "FILE");
            scope.typedefs.insert("FILE".into(), ty);
          }
          if header.as_deref() == Some("stdarg.h") && scope.type_name("va_list").is_none() {
            let ty = CType::renamed(&builtin("void *").unwrap(), "va_list");
            scope.typedefs.insert("va_list".into(), ty);
          }
        },
        "pragma" => {
          let before = scope.pack;
          pragma(&line[1..], scope, &here)?;
          if scope.pack != before {
            out.push(Token {
              tok: Tok::Pack(scope.pack.unwrap_or(0)),
              line: here.line,
              column: here.column,
              line_start: false,
            });
          }
        },
        "error" => {
          let text = line[1..]
            .iter()
            .map(token_text)
            .collect::<Vec<_>>()
            .join(" ");
          return Err(error_at(here.line, here.column, format!("#error {text}")));
        },
        "warning" | "line" | "ident" | "sccs" | "assert" | "unassert" => {},
        other => {
          return Err(error_at(
            first.line,
            first.column,
            format!("unknown directive '#{other}'"),
          ));
        },
      }
      continue;
    }

    if active(&stack) {
      expand(token, &tokens, &mut i, scope, &mut Vec::new(), &mut out)?;
    } else {
      i += 1;
    }
  }

  Ok(join_strings(out))
}

/// Joins adjacent string literals into one, as C does before parsing.
fn join_strings(tokens: Vec<Token>) -> Vec<Token> {
  let mut out: Vec<Token> = Vec::with_capacity(tokens.len());

  for t in tokens {
    if let Tok::Str(next) = &t.tok
      && let Some(Token {
        tok: Tok::Str(previous),
        ..
      }) = out.last_mut()
    {
      previous.push_str(next);
      continue;
    }
    out.push(t);
  }

  out
}

fn token_text(t: &Token) -> String {
  match &t.tok {
    Tok::Ident(s) => s.clone(),
    Tok::Int(v, _) => v.to_string(),
    Tok::Float(f) => f.to_string(),
    Tok::Str(s) => format!("\"{s}\""),
    Tok::Char(c) => format!("'{}'", char::from_u32(*c as u32).unwrap_or('?')),
    Tok::Punct(p) => p.to_string(),
    Tok::Pack(_) | Tok::Eof => String::new(),
  }
}

fn is_defined(scope: &Scope, name: &str) -> bool {
  scope.macro_named(name).is_some() || predefined(name).is_some()
}

fn include_name(tokens: &[Token]) -> Option<String> {
  match tokens.first().map(|t| &t.tok) {
    Some(Tok::Str(s)) => Some(s.clone()),
    Some(Tok::Punct("<")) => {
      let mut name = String::new();
      for t in &tokens[1..] {
        if t.tok == Tok::Punct(">") {
          return Some(name);
        }
        name.push_str(&token_text(t));
      }
      None
    },
    _ => None,
  }
}

fn pragma(tokens: &[Token], scope: &mut Scope, here: &Token) -> Parse<()> {
  let Some(Tok::Ident(name)) = tokens.first().map(|t| &t.tok) else {
    return Ok(());
  };

  if name != "pack" {
    // Any other pragma is a request to some compiler, and ignoring one
    // it does not understand is what a compiler does.
    return Ok(());
  }

  let args: Vec<&Tok> = tokens[1..]
    .iter()
    .map(|t| &t.tok)
    .filter(|t| **t != Tok::Punct("(") && **t != Tok::Punct(")") && **t != Tok::Punct(","))
    .collect();

  let value = |t: &Tok| -> Parse<usize> {
    match t {
      Tok::Int(v, _) if [1, 2, 4, 8, 16].contains(v) => Ok(*v as usize),
      _ => Err(error_at(
        here.line,
        here.column,
        "#pragma pack takes 1, 2, 4, 8 or 16",
      )),
    }
  };

  match args.as_slice() {
    [] => scope.pack = None,
    [Tok::Ident(w)] if w == "push" => scope.pack_stack.push(scope.pack),
    [Tok::Ident(w)] if w == "pop" => scope.pack = scope.pack_stack.pop().flatten(),
    [Tok::Ident(w), n] if w == "push" => {
      scope.pack_stack.push(scope.pack);
      scope.pack = Some(value(n)?);
    },
    [n] => scope.pack = Some(value(n)?),
    _ => {
      return Err(error_at(
        here.line,
        here.column,
        "unrecognised #pragma pack",
      ));
    },
  }

  Ok(())
}

/// Copies `tokens[*i]` to `out`, expanding it if it names an
/// object-like macro, and advances `i`.
fn expand(
  token: &Token,
  tokens: &[Token],
  i: &mut usize,
  scope: &Scope,
  hidden: &mut Vec<String>,
  out: &mut Vec<Token>,
) -> Parse<()> {
  *i += 1;

  let Tok::Ident(name) = &token.tok else {
    out.push(token.clone());
    return Ok(());
  };

  if hidden.contains(name) {
    out.push(token.clone());
    return Ok(());
  }

  let body = match scope.macro_named(name) {
    Some(Macro::Object(body)) => body,
    Some(Macro::Function) => {
      if tokens.get(*i).is_some_and(|t| t.tok == Tok::Punct("(")) {
        return Err(error_at(
          token.line,
          token.column,
          format!(
            "'{name}' is a function-like macro, which declarations cannot expand; write out what it expands to"
          ),
        ));
      }
      out.push(token.clone());
      return Ok(());
    },
    None => match predefined(name) {
      Some(toks) => toks
        .into_iter()
        .map(|tok| Token {
          tok,
          line: token.line,
          column: token.column,
          line_start: false,
        })
        .collect(),
      None => {
        out.push(token.clone());
        return Ok(());
      },
    },
  };

  hidden.push(name.clone());
  let relocated: Vec<Token> = body
    .into_iter()
    .map(|t| Token {
      line: token.line,
      column: token.column,
      line_start: false,
      ..t
    })
    .collect();

  let mut j = 0;
  while j < relocated.len() {
    let t = relocated[j].clone();
    expand(&t, &relocated, &mut j, scope, hidden, out)?;
  }
  hidden.pop();

  Ok(())
}

/// The value an object-like macro's body denotes, when it is a constant.
fn constant_of(tokens: &[Token], scope: &Scope) -> Option<Constant> {
  if tokens.is_empty() {
    return None;
  }

  let mut expanded = Vec::new();
  let mut i = 0;
  while i < tokens.len() {
    let t = tokens[i].clone();
    expand(&t, tokens, &mut i, scope, &mut Vec::new(), &mut expanded).ok()?;
  }
  let mut expanded = join_strings(expanded);

  if let [
    Token {
      tok: Tok::Str(s), ..
    },
  ] = expanded.as_slice()
  {
    return Some(Constant::Str(s.clone()));
  }

  if let [
    Token {
      tok: Tok::Float(f), ..
    },
  ] = expanded.as_slice()
  {
    return Some(Constant::Float(*f));
  }

  if let [
    Token {
      tok: Tok::Punct("-"),
      ..
    },
    Token {
      tok: Tok::Float(f), ..
    },
  ] = expanded.as_slice()
  {
    return Some(Constant::Float(-*f));
  }

  expanded.push(Token {
    tok: Tok::Eof,
    line: 0,
    column: 0,
    line_start: true,
  });

  let mut parser = Parser::for_expression(expanded, scope);
  let value = parser.expression().ok()?;
  if !matches!(parser.peek().tok, Tok::Eof) {
    return None;
  }
  Some(Constant::Int(value))
}

fn eval_condition(tokens: &[Token], scope: &Scope, here: &Token) -> Parse<bool> {
  // `defined` is resolved before anything is expanded, and the
  // `__has_*` queries all answer no.
  let mut resolved = Vec::new();
  let mut i = 0;

  while i < tokens.len() {
    let t = &tokens[i];
    match &t.tok {
      Tok::Ident(name) if name == "defined" => {
        let (target, skip) = match (
          tokens.get(i + 1).map(|t| &t.tok),
          tokens.get(i + 2).map(|t| &t.tok),
        ) {
          (Some(Tok::Punct("(")), Some(Tok::Ident(n))) => (n.clone(), 4),
          (Some(Tok::Ident(n)), _) => (n.clone(), 2),
          _ => return Err(error_at(t.line, t.column, "'defined' needs a macro name")),
        };
        resolved.push(Token {
          tok: Tok::Int(is_defined(scope, &target) as i128, false),
          ..t.clone()
        });
        i += skip;
      },
      Tok::Ident(name) if name.starts_with("__has_") => {
        let mut depth = 0;
        i += 1;
        while i < tokens.len() {
          match tokens[i].tok {
            Tok::Punct("(") => depth += 1,
            Tok::Punct(")") => {
              depth -= 1;
              if depth == 0 {
                i += 1;
                break;
              }
            },
            _ => {},
          }
          i += 1;
        }
        resolved.push(Token {
          tok: Tok::Int(0, false),
          ..t.clone()
        });
      },
      _ => {
        resolved.push(t.clone());
        i += 1;
      },
    }
  }

  let mut expanded = Vec::new();
  let mut j = 0;
  while j < resolved.len() {
    let t = resolved[j].clone();
    expand(&t, &resolved, &mut j, scope, &mut Vec::new(), &mut expanded)?;
  }

  // Whatever identifiers survive expansion count as zero.
  let mut final_tokens: Vec<Token> = expanded
    .into_iter()
    .map(|t| match t.tok {
      Tok::Ident(ref n) if n != "true" && n != "false" => Token {
        tok: Tok::Int(0, false),
        ..t
      },
      Tok::Ident(ref n) => Token {
        tok: Tok::Int((n == "true") as i128, false),
        ..t
      },
      _ => t,
    })
    .collect();

  final_tokens.push(Token {
    tok: Tok::Eof,
    line: here.line,
    column: here.column,
    line_start: true,
  });

  let mut parser = Parser::for_expression(final_tokens, scope);
  let value = parser.expression()?;
  if !matches!(parser.peek().tok, Tok::Eof) {
    let t = parser.peek().clone();
    return Err(error_at(t.line, t.column, "unexpected token in #if"));
  }

  Ok(value != 0)
}

// Parsing.

#[derive(Default, Clone)]
struct Attributes {
  packed: bool,
  aligned: Option<usize>,
  abi: Option<Abi>,
  symbol: Option<String>,
  nonnull: bool,
}

#[derive(Clone, Copy, PartialEq)]
enum Storage {
  None,
  Typedef,
  Extern,
  Static,
}

struct Specifiers {
  base: TypeRef,
  storage: Storage,
  inline: bool,
  thread_local: bool,
  attrs: Attributes,
}

enum Derivation {
  Pointer {
    is_const: bool,
    nonnull: bool,
  },
  Array(Option<usize>),
  Function {
    params: Vec<(Option<String>, TypeRef)>,
    variadic: bool,
  },
}

struct Declarator {
  name: Option<String>,
  line: usize,
  column: usize,
  derivations: Vec<Derivation>,
  attrs: Attributes,
}

/// Where a parser's names are resolved: a scope it may add to, or one it
/// only reads, for evaluating a constant expression.
enum ScopeAccess<'a> {
  Write(&'a mut Scope),
  Read(&'a Scope),
}

impl ScopeAccess<'_> {
  fn get(&self) -> &Scope {
    match self {
      ScopeAccess::Write(s) => s,
      ScopeAccess::Read(s) => s,
    }
  }
}

pub struct Parser<'a> {
  tokens: Vec<Token>,
  pos: usize,
  scope: ScopeAccess<'a>,
  /// The packing in force at this point of the source.
  pack: Option<usize>,
}

/// Reads `source` into `scope`.
pub fn declare(source: &str, scope: &mut Scope) -> Parse<()> {
  let tokens = lex(source)?;
  let pack = scope.pack;
  let tokens = preprocess(tokens, scope)?;
  let mut parser = Parser {
    tokens,
    pos: 0,
    scope: ScopeAccess::Write(scope),
    pack,
  };
  parser.translation_unit()
}

/// Parses a C type name, such as `const char *` or `struct point[4]`,
/// against `scope`.
pub fn type_name(source: &str, scope: &mut Scope) -> Parse<TypeRef> {
  let mut tokens = lex(source)?;
  if tokens.len() == 1 {
    return Err(error_at(1, 1, "a type name is empty"));
  }
  let pack = scope.pack;
  tokens = preprocess(tokens, scope)?;
  let mut parser = Parser {
    tokens,
    pos: 0,
    scope: ScopeAccess::Write(scope),
    pack,
  };
  let ty = parser.type_name()?;
  if !matches!(parser.peek().tok, Tok::Eof) {
    let t = parser.peek().clone();
    return Err(error_at(
      t.line,
      t.column,
      format!("unexpected '{}' after the type", token_text(&t)),
    ));
  }
  Ok(ty)
}

const TYPE_KEYWORDS: &[&str] = &[
  "void",
  "char",
  "short",
  "int",
  "long",
  "float",
  "double",
  "signed",
  "unsigned",
  "_Bool",
  "bool",
  "_Complex",
  "__complex__",
  "__int128",
  "struct",
  "union",
  "enum",
  "__signed",
  "__signed__",
  "__unsigned",
];

const QUALIFIERS: &[&str] = &[
  "const",
  "volatile",
  "restrict",
  "__restrict",
  "__restrict__",
  "__const",
  "__const__",
  "__volatile__",
  "_Atomic",
  "_Nullable",
  "_Null_unspecified",
  "__nullable",
  "__null_unspecified",
  "__ptr64",
  "__ptr32",
  "__unaligned",
  "__w64",
];

const IGNORED_SPECIFIERS: &[&str] = &[
  "__extension__",
  "register",
  "auto",
  "_Noreturn",
  "noreturn",
  "__cdecl",
  "_cdecl",
  "__stdcall",
  "_stdcall",
  "__fastcall",
  "__thiscall",
  "__clrcall",
  "WINAPI",
  "APIENTRY",
  "CALLBACK",
  "__forceinline",
];

impl<'a> Parser<'a> {
  fn for_expression(tokens: Vec<Token>, scope: &'a Scope) -> Parser<'a> {
    Parser {
      tokens,
      pos: 0,
      scope: ScopeAccess::Read(scope),
      pack: None,
    }
  }

  fn scope(&self) -> &Scope {
    self.scope.get()
  }

  fn scope_mut(&mut self) -> Parse<&mut Scope> {
    let t = self.peek().clone();
    match &mut self.scope {
      ScopeAccess::Write(s) => Ok(s),
      ScopeAccess::Read(_) => Err(error_at(
        t.line,
        t.column,
        "a declaration cannot appear here",
      )),
    }
  }

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

  fn expect_punct(&mut self, p: &str) -> Parse<()> {
    if self.eat_punct(p) {
      return Ok(());
    }
    let t = self.peek().clone();
    Err(error_at(
      t.line,
      t.column,
      format!("expected '{p}', found {}", describe_token(&t)),
    ))
  }

  fn fail<T>(&self, message: impl Into<String>) -> Parse<T> {
    let t = self.peek();
    Err(error_at(t.line, t.column, message))
  }

  fn translation_unit(&mut self) -> Parse<()> {
    while !matches!(self.peek().tok, Tok::Eof) {
      self.external_declaration()?;
    }
    Ok(())
  }

  /// Applies any packing changes at the current position.
  fn take_packing(&mut self) {
    while let Tok::Pack(n) = self.peek().tok {
      self.pack = (n != 0).then_some(n);
      self.pos += 1;
    }
  }

  fn external_declaration(&mut self) -> Parse<()> {
    self.take_packing();

    if matches!(self.peek().tok, Tok::Eof) {
      return Ok(());
    }

    if self.eat_punct(";") {
      return Ok(());
    }

    // `extern "C" { ... }`, as headers shared with C++ write it.
    if self.is_ident("extern")
      && let Tok::Str(language) = &self.peek_at(1).tok
    {
      if language != "C" && language != "C++" {
        return self.fail(format!("unknown linkage \"{language}\""));
      }
      self.pos += 2;
      if self.eat_punct("{") {
        loop {
          self.take_packing();
          if self.is_punct("}") {
            break;
          }
          if matches!(self.peek().tok, Tok::Eof) {
            return self.fail("an extern \"C\" block is never closed");
          }
          self.external_declaration()?;
        }
        self.expect_punct("}")?;
        return Ok(());
      }
    }

    if self.is_ident("_Static_assert") || self.is_ident("static_assert") {
      return self.static_assert();
    }

    let start = self.peek().clone();
    let specs = self.specifiers()?;

    if self.eat_punct(";") {
      // `struct x { ... };` or `enum { ... };` on its own.
      return Ok(());
    }

    loop {
      let decl = self.declarator(false)?;
      let mut attrs = specs.attrs.clone();
      merge(&mut attrs, &decl.attrs);
      let trailing = self.attributes()?;
      merge(&mut attrs, &trailing);

      let Some(name) = decl.name.clone() else {
        return Err(error_at(
          start.line,
          start.column,
          "a declaration needs a name",
        ));
      };

      let ty = self.apply(
        &specs.base,
        &decl.derivations,
        &attrs,
        decl.line,
        decl.column,
      )?;

      // A function body: an inline helper in a header. Nothing is bound
      // for it, since the library need not export it.
      if self.is_punct("{") {
        self.skip_braces()?;
        if !(specs.storage == Storage::Static || specs.inline) {
          self.declare_function(&name, &ty, &attrs)?;
        }
        return Ok(());
      }

      if self.eat_punct("=") {
        return Err(error_at(
          decl.line,
          decl.column,
          format!("'{name}' has an initializer; declare it without one"),
        ));
      }

      match specs.storage {
        Storage::Typedef => {
          let named = CType::renamed(&ty, name.clone());
          self.scope_mut()?.typedefs.insert(name, named);
        },
        Storage::Static => {},
        _ => {
          if matches!(ty.kind, Kind::Function(_)) {
            if !specs.inline {
              self.declare_function(&name, &ty, &attrs)?;
            }
          } else {
            if specs.thread_local {
              return Err(error_at(
                decl.line,
                decl.column,
                format!(
                  "'{name}' is thread-local, and a thread-local variable has no single address to bind"
                ),
              ));
            }
            let symbol = attrs.symbol.clone().unwrap_or_else(|| name.clone());
            self
              .scope_mut()?
              .add_variable(VariableDecl { name, symbol, ty });
          }
        },
      }

      if self.eat_punct(",") {
        continue;
      }
      self.expect_punct(";")?;
      return Ok(());
    }
  }

  fn declare_function(&mut self, name: &str, ty: &TypeRef, attrs: &Attributes) -> Parse<()> {
    let Kind::Function(sig) = &ty.kind else {
      return Ok(());
    };

    let returns = text_return(&sig.returns);
    let sig = Signature {
      returns,
      params: sig.params.clone(),
      param_names: sig.param_names.clone(),
      variadic: sig.variadic,
      abi: attrs.abi.unwrap_or(sig.abi),
    };

    let symbol = attrs.symbol.clone().unwrap_or_else(|| name.to_string());
    self.scope_mut()?.add_function(FunctionDecl {
      name: name.to_string(),
      symbol,
      sig: Arc::new(sig),
    });
    Ok(())
  }

  fn static_assert(&mut self) -> Parse<()> {
    let start = self.next();
    self.expect_punct("(")?;
    let value = self.conditional()?;
    let mut message = String::from("static assertion failed");
    if self.eat_punct(",") {
      if let Tok::Str(s) = self.next().tok {
        message = format!("static assertion failed: {s}");
      }
    }
    self.expect_punct(")")?;
    self.eat_punct(";");
    if value == 0 {
      return Err(error_at(start.line, start.column, message));
    }
    Ok(())
  }

  fn skip_braces(&mut self) -> Parse<()> {
    let open = self.peek().clone();
    self.expect_punct("{")?;
    let mut depth = 1;
    while depth > 0 {
      match self.next().tok {
        Tok::Punct("{") => depth += 1,
        Tok::Punct("}") => depth -= 1,
        Tok::Eof => return Err(error_at(open.line, open.column, "a body is never closed")),
        _ => {},
      }
    }
    Ok(())
  }

  fn starts_type(&self) -> bool {
    match &self.peek().tok {
      Tok::Ident(n) => {
        TYPE_KEYWORDS.contains(&n.as_str())
          || QUALIFIERS.contains(&n.as_str())
          || IGNORED_SPECIFIERS.contains(&n.as_str())
          || matches!(
            n.as_str(),
            "typedef"
              | "extern"
              | "static"
              | "inline"
              | "__inline"
              | "__inline__"
              | "_Thread_local"
              | "thread_local"
              | "__thread"
              | "__attribute__"
              | "__attribute"
              | "__declspec"
              | "_Alignas"
              | "alignas"
              | "__typeof__"
              | "typeof"
          )
          || self.scope().type_name(n).is_some()
      },
      Tok::Punct("[") => self.peek_at(1).tok == Tok::Punct("["),
      _ => false,
    }
  }

  fn specifiers(&mut self) -> Parse<Specifiers> {
    let start = self.peek().clone();
    let mut storage = Storage::None;
    let mut inline = false;
    let mut thread_local = false;
    let mut attrs = Attributes::default();
    let mut is_const = false;

    let mut signed: Option<bool> = None;
    let mut longs = 0;
    let mut short = false;
    let mut complex = false;
    let mut word: Option<&'static str> = None;
    let mut named: Option<TypeRef> = None;

    loop {
      let t = self.peek().clone();
      let Tok::Ident(n) = &t.tok else {
        if self.is_punct("[") && self.peek_at(1).tok == Tok::Punct("[") {
          self.skip_cpp_attribute()?;
          continue;
        }
        break;
      };

      match n.as_str() {
        "typedef" => storage = Storage::Typedef,
        "extern" => storage = Storage::Extern,
        "static" => storage = Storage::Static,
        "inline" | "__inline" | "__inline__" => inline = true,
        "_Thread_local" | "thread_local" | "__thread" => thread_local = true,
        "const" | "__const" | "__const__" => is_const = true,
        q if QUALIFIERS.contains(&q) => {
          if q == "_Atomic" && self.peek_at(1).tok == Tok::Punct("(") {
            self.pos += 2;
            named = Some(self.type_name()?);
            self.expect_punct(")")?;
            continue;
          }
        },
        s if IGNORED_SPECIFIERS.contains(&s) => {},
        "__vectorcall" => return self.fail("the __vectorcall convention is not supported"),
        "__attribute__" | "__attribute" | "__declspec" => {
          let more = self.attributes()?;
          merge(&mut attrs, &more);
          continue;
        },
        "_Alignas" | "alignas" => {
          self.pos += 1;
          self.expect_punct("(")?;
          let align = if self.starts_type() {
            let ty = self.type_name()?;
            ty.require_align()
              .map_err(|e| error_at(t.line, t.column, e))?
          } else {
            self.conditional()? as usize
          };
          self.expect_punct(")")?;
          attrs.aligned = Some(attrs.aligned.unwrap_or(1).max(align));
          continue;
        },
        "__typeof__" | "typeof" => {
          return self.fail("typeof is not supported in declarations");
        },
        "signed" | "__signed" | "__signed__" => signed = Some(true),
        "unsigned" | "__unsigned" => signed = Some(false),
        "long" => longs += 1,
        "short" => short = true,
        "_Complex" | "__complex__" => complex = true,
        "void" | "char" | "int" | "float" | "double" | "_Bool" | "bool" | "__int128" => {
          if word.is_some() && !(word == Some("int") || n == "int") {
            return self.fail(format!("'{n}' cannot be combined with '{}'", word.unwrap()));
          }
          let this: &'static str = match n.as_str() {
            "void" => "void",
            "char" => "char",
            "int" => "int",
            "float" => "float",
            "double" => "double",
            "__int128" => "__int128",
            _ => "bool",
          };
          if word.is_none() || word == Some("int") {
            word = Some(this);
          }
        },
        "struct" | "union" => {
          named = Some(self.record_specifier()?);
          continue;
        },
        "enum" => {
          named = Some(self.enum_specifier()?);
          continue;
        },
        other => {
          if named.is_some() || word.is_some() || signed.is_some() || longs > 0 || short {
            break;
          }
          if let Some(ty) = self.scope().type_name(other) {
            named = Some(ty);
          } else {
            break;
          }
        },
      }

      self.pos += 1;
    }

    let base = if let Some(ty) = named {
      if word.is_some() || signed.is_some() || longs > 0 || short {
        return Err(error_at(
          start.line,
          start.column,
          format!("'{}' cannot take other type words", ty.name),
        ));
      }
      ty
    } else {
      let key = match (word, signed, longs, short, complex) {
        (Some("float"), None, 0, false, true) => "float _Complex",
        (Some("double"), None, 0, false, true) => "double _Complex",
        (_, _, _, _, true) => {
          return Err(error_at(
            start.line,
            start.column,
            "_Complex applies to float and double only",
          ));
        },
        (Some("void"), None, 0, false, _) => "void",
        (Some("bool"), None, 0, false, _) => "bool",
        (Some("char"), None, 0, false, _) => "char",
        (Some("char"), Some(true), 0, false, _) => "signed char",
        (Some("char"), Some(false), 0, false, _) => "unsigned char",
        (Some("float"), None, 0, false, _) => "float",
        (Some("double"), None, 0, false, _) => "double",
        (Some("double"), None, 1, false, _) => "long double",
        (Some("__int128"), Some(false), 0, false, _) => "unsigned __int128",
        (Some("__int128"), _, 0, false, _) => "__int128",
        (None | Some("int"), s, 0, true, _) => {
          if s == Some(false) {
            "unsigned short"
          } else {
            "short"
          }
        },
        (None | Some("int"), s, 1, false, _) => {
          if s == Some(false) {
            "unsigned long"
          } else {
            "long"
          }
        },
        (None | Some("int"), s, 2, false, _) => {
          if s == Some(false) {
            "unsigned long long"
          } else {
            "long long"
          }
        },
        (Some("int"), s, 0, false, _) | (None, s @ Some(_), 0, false, _) => {
          if s == Some(false) {
            "unsigned int"
          } else {
            "int"
          }
        },
        (None, None, 0, false, _) => {
          let t = self.peek().clone();
          return Err(error_at(
            t.line,
            t.column,
            match &t.tok {
              Tok::Ident(n) => format!("unknown type name '{n}'"),
              _ => format!("expected a type, found {}", describe_token(&t)),
            },
          ));
        },
        _ => {
          return Err(error_at(
            start.line,
            start.column,
            "these type words do not combine",
          ));
        },
      };
      builtin(key).unwrap()
    };

    let base = if is_const {
      CType::constant(&base)
    } else {
      base
    };

    Ok(Specifiers {
      base,
      storage,
      inline,
      thread_local,
      attrs,
    })
  }

  fn skip_cpp_attribute(&mut self) -> Parse<()> {
    self.expect_punct("[")?;
    self.expect_punct("[")?;
    let mut depth = 2;
    while depth > 0 {
      match self.next().tok {
        Tok::Punct("[") => depth += 1,
        Tok::Punct("]") => depth -= 1,
        Tok::Eof => return self.fail("an attribute is never closed"),
        _ => {},
      }
    }
    Ok(())
  }

  /// Reads any run of `__attribute__((...))`, `__declspec(...)` and
  /// `asm("...")`.
  fn attributes(&mut self) -> Parse<Attributes> {
    let mut attrs = Attributes::default();

    loop {
      match &self.peek().tok {
        Tok::Ident(n) if n == "__attribute__" || n == "__attribute" => {
          self.pos += 1;
          self.expect_punct("(")?;
          self.expect_punct("(")?;
          while !self.is_punct(")") {
            let t = self.next();
            let Tok::Ident(name) = t.tok else {
              if t.tok == Tok::Punct(",") {
                continue;
              }
              return Err(error_at(t.line, t.column, "expected an attribute name"));
            };
            let name = name.trim_matches('_').to_string();
            match name.as_str() {
              "packed" => attrs.packed = true,
              "aligned" => {
                if self.eat_punct("(") {
                  let n = self.conditional()? as usize;
                  self.expect_punct(")")?;
                  attrs.aligned = Some(n);
                } else {
                  attrs.aligned = Some(16);
                }
              },
              "ms_abi" => {
                attrs.abi = Some(Abi::parse("win64").map_err(|e| error_at(t.line, t.column, e))?)
              },
              "sysv_abi" => {
                attrs.abi = Some(Abi::parse("sysv64").map_err(|e| error_at(t.line, t.column, e))?)
              },
              _ => {
                if self.is_punct("(") {
                  self.skip_parens()?;
                }
              },
            }
          }
          self.expect_punct(")")?;
          self.expect_punct(")")?;
        },
        Tok::Ident(n) if n == "__declspec" => {
          self.pos += 1;
          self.expect_punct("(")?;
          while !self.is_punct(")") {
            let t = self.next();
            match t.tok {
              Tok::Ident(ref name) if name == "align" => {
                self.expect_punct("(")?;
                attrs.aligned = Some(self.conditional()? as usize);
                self.expect_punct(")")?;
              },
              Tok::Ident(_) => {
                if self.is_punct("(") {
                  self.skip_parens()?;
                }
              },
              Tok::Eof => return self.fail("__declspec is never closed"),
              _ => {},
            }
          }
          self.expect_punct(")")?;
        },
        Tok::Ident(n) if n == "__asm__" || n == "__asm" || n == "asm" => {
          self.pos += 1;
          self.expect_punct("(")?;
          let mut symbol = String::new();
          while let Tok::Str(s) = &self.peek().tok {
            symbol.push_str(s);
            self.pos += 1;
          }
          self.expect_punct(")")?;
          attrs.symbol = Some(symbol);
        },
        Tok::Punct("[") if self.peek_at(1).tok == Tok::Punct("[") => self.skip_cpp_attribute()?,
        _ => return Ok(attrs),
      }
    }
  }

  fn skip_parens(&mut self) -> Parse<()> {
    self.expect_punct("(")?;
    let mut depth = 1;
    while depth > 0 {
      match self.next().tok {
        Tok::Punct("(") => depth += 1,
        Tok::Punct(")") => depth -= 1,
        Tok::Eof => return self.fail("a parenthesis is never closed"),
        _ => {},
      }
    }
    Ok(())
  }

  fn record_specifier(&mut self) -> Parse<TypeRef> {
    let keyword = self.next();
    let is_union = keyword.tok == Tok::Ident("union".into());
    let word = if is_union { "union" } else { "struct" };

    let mut attrs = self.attributes()?;

    let tag = match &self.peek().tok {
      Tok::Ident(n) if !self.is_punct("{") => {
        let n = n.clone();
        self.pos += 1;
        Some(n)
      },
      _ => None,
    };

    let more = self.attributes()?;
    merge(&mut attrs, &more);

    let defining = self.is_punct("{");

    let ty = match &tag {
      Some(t) => {
        let key = format!("{word} {t}");
        let existing = self.scope().tag(&key);
        match existing {
          Some(ty) if !defining || !ty.record().is_some_and(|r| r.is_defined()) => ty,
          Some(_) => {
            return Err(error_at(
              keyword.line,
              keyword.column,
              format!("'{key}' is already defined"),
            ));
          },
          None => {
            let record = Record::new(Some(t.clone()), is_union);
            let ty = CType::new(Kind::Record(record), key.clone());
            self.scope_mut()?.tags.insert(key, ty.clone());
            ty
          },
        }
      },
      None => {
        if !defining {
          return Err(error_at(
            keyword.line,
            keyword.column,
            format!("an anonymous {word} needs a body"),
          ));
        }
        let record = Record::new(None, is_union);
        CType::new(Kind::Record(record), format!("{word} <anonymous>"))
      },
    };

    if !defining {
      return Ok(ty);
    }

    let record = ty.record().unwrap().clone();
    self.expect_punct("{")?;

    if let Some(p) = self.pack {
      record
        .set_pack(p)
        .map_err(|e| error_at(keyword.line, keyword.column, e))?;
    }

    loop {
      self.take_packing();
      if self.eat_punct("}") {
        break;
      }
      if matches!(self.peek().tok, Tok::Eof) {
        return Err(error_at(
          keyword.line,
          keyword.column,
          format!("a {word} body is never closed"),
        ));
      }
      if self.eat_punct(";") {
        continue;
      }
      if self.is_ident("_Static_assert") || self.is_ident("static_assert") {
        self.static_assert()?;
        continue;
      }

      let member_start = self.peek().clone();
      let specs = self.specifiers()?;

      // An anonymous struct or union member.
      if self.eat_punct(";") {
        if specs.base.is_record() && specs.base.record().unwrap().tag.is_none() {
          record
            .add_field(FieldSpec {
              name: String::new(),
              ty: specs.base.clone(),
              bits: None,
              align: specs.attrs.aligned,
            })
            .map_err(|e| error_at(member_start.line, member_start.column, e))?;
        }
        continue;
      }

      loop {
        let (name, ty, line, column, member_attrs) = if self.is_punct(":") {
          (
            String::new(),
            specs.base.clone(),
            member_start.line,
            member_start.column,
            Attributes::default(),
          )
        } else {
          let decl = self.declarator(false)?;
          let mut a = specs.attrs.clone();
          merge(&mut a, &decl.attrs);
          let ty = self.apply(&specs.base, &decl.derivations, &a, decl.line, decl.column)?;
          let Some(name) = decl.name else {
            return Err(error_at(decl.line, decl.column, "a member needs a name"));
          };
          (name, ty, decl.line, decl.column, a)
        };

        let bits = if self.eat_punct(":") {
          Some(self.conditional()? as u32)
        } else {
          None
        };

        let trailing = self.attributes()?;
        let mut member_attrs = member_attrs;
        merge(&mut member_attrs, &trailing);

        let mut align = member_attrs.aligned;
        if member_attrs.packed {
          align = None;
        }

        record
          .add_field(FieldSpec {
            name,
            ty,
            bits,
            align,
          })
          .map_err(|e| error_at(line, column, e))?;

        if self.eat_punct(",") {
          continue;
        }
        self.expect_punct(";")?;
        break;
      }
    }

    record.mark_defined();

    let trailing = self.attributes()?;
    merge(&mut attrs, &trailing);

    if attrs.packed {
      record
        .set_pack(1)
        .map_err(|e| error_at(keyword.line, keyword.column, e))?;
    }
    if let Some(a) = attrs.aligned {
      record
        .set_align(a)
        .map_err(|e| error_at(keyword.line, keyword.column, e))?;
    }

    Ok(ty)
  }

  fn enum_specifier(&mut self) -> Parse<TypeRef> {
    let keyword = self.next();
    let mut attrs = self.attributes()?;

    let tag = match &self.peek().tok {
      Tok::Ident(n) => {
        let n = n.clone();
        self.pos += 1;
        Some(n)
      },
      _ => None,
    };

    let fixed = if self.eat_punct(":") {
      Some(self.type_name()?)
    } else {
      None
    };

    if !self.is_punct("{") {
      let Some(t) = tag else {
        return Err(error_at(
          keyword.line,
          keyword.column,
          "an anonymous enum needs a body",
        ));
      };
      let key = format!("enum {t}");
      if let Some(ty) = self.scope().tag(&key) {
        return Ok(ty);
      }
      // A forward-declared enum is an int until it says otherwise.
      let info = Arc::new(EnumInfo {
        underlying: fixed.unwrap_or_else(|| builtin("int").unwrap()),
        constants: std::sync::Mutex::new(Vec::new()),
      });
      let ty = CType::new(Kind::Enum(info), key.clone());
      self.scope_mut()?.tags.insert(key, ty.clone());
      return Ok(ty);
    }

    self.expect_punct("{")?;
    let mut constants: Vec<(String, i128)> = Vec::new();
    let mut next = 0i128;

    while !self.eat_punct("}") {
      let t = self.next();
      let Tok::Ident(name) = t.tok else {
        return Err(error_at(t.line, t.column, "expected an enumerator name"));
      };
      let _ = self.attributes()?;
      if self.eat_punct("=") {
        next = self.conditional()?;
      }
      // MSVC makes every enum without a fixed type an `int`, and
      // truncates each constant to fit as it is declared.
      if fixed.is_none() && cfg!(windows) {
        next = cast(next, &builtin("int").unwrap());
      }
      constants.push((name.clone(), next));
      self.scope_mut()?.add_constant(&name, Constant::Int(next));
      next = next.wrapping_add(1);
      if !self.eat_punct(",") {
        self.expect_punct("}")?;
        break;
      }
    }

    let trailing = self.attributes()?;
    merge(&mut attrs, &trailing);

    let underlying = match fixed {
      Some(t) => t,
      None => enum_underlying(&constants, attrs.packed),
    };

    let info = Arc::new(EnumInfo {
      underlying,
      constants: std::sync::Mutex::new(constants),
    });

    let name = match &tag {
      Some(t) => format!("enum {t}"),
      None => "enum <anonymous>".to_string(),
    };
    let ty = CType::new(Kind::Enum(info), name.clone());

    if tag.is_some() {
      self.scope_mut()?.tags.insert(name, ty.clone());
    }

    Ok(ty)
  }

  /// A declarator: the part of a declaration after the type words, with
  /// the name somewhere inside it. `abstract_ok` allows the name to be
  /// missing, as in a type name or a parameter.
  fn declarator(&mut self, abstract_ok: bool) -> Parse<Declarator> {
    let mut attrs = self.attributes()?;
    let mut pointers = Vec::new();

    while self.is_punct("*") || self.is_punct("^") {
      self.pos += 1;
      let mut is_const = false;
      let mut nonnull = false;
      loop {
        match &self.peek().tok {
          Tok::Ident(q) if q == "const" || q == "__const" || q == "__const__" => is_const = true,
          Tok::Ident(q) if q == "_Nonnull" || q == "__nonnull" => nonnull = true,
          Tok::Ident(q)
            if QUALIFIERS.contains(&q.as_str()) || IGNORED_SPECIFIERS.contains(&q.as_str()) => {},
          Tok::Ident(q) if q == "__attribute__" || q == "__declspec" => {
            let more = self.attributes()?;
            merge(&mut attrs, &more);
            continue;
          },
          _ => break,
        }
        self.pos += 1;
      }
      pointers.push(Derivation::Pointer { is_const, nonnull });
    }

    // Calling conventions may sit between the pointers and the name.
    while let Tok::Ident(n) = &self.peek().tok {
      if IGNORED_SPECIFIERS.contains(&n.as_str()) {
        self.pos += 1;
      } else {
        break;
      }
    }

    let here = self.peek().clone();
    let mut name = None;
    let mut inner = Vec::new();

    let nested = self.is_punct("(") && {
      let after = &self.peek_at(1).tok;
      match after {
        Tok::Punct("*") | Tok::Punct("^") | Tok::Punct("(") | Tok::Punct("[") => true,
        Tok::Ident(n) => {
          !(self.scope().type_name(n).is_some()
            || TYPE_KEYWORDS.contains(&n.as_str())
            || QUALIFIERS.contains(&n.as_str()))
            || IGNORED_SPECIFIERS.contains(&n.as_str())
            || n == "__attribute__"
        },
        _ => false,
      }
    };

    if nested {
      self.pos += 1;
      let d = self.declarator(abstract_ok)?;
      self.expect_punct(")")?;
      name = d.name;
      inner = d.derivations;
      merge(&mut attrs, &d.attrs);
    } else if let Tok::Ident(n) = &here.tok
      && !QUALIFIERS.contains(&n.as_str())
    {
      name = Some(n.clone());
      self.pos += 1;
    }

    if name.is_none() && !abstract_ok && !nested {
      return Err(error_at(
        here.line,
        here.column,
        format!("expected a name, found {}", describe_token(&here)),
      ));
    }

    let mut suffixes = Vec::new();
    loop {
      if self.eat_punct("[") {
        while let Tok::Ident(q) = &self.peek().tok {
          if q == "static" || QUALIFIERS.contains(&q.as_str()) || q == "const" {
            self.pos += 1;
          } else {
            break;
          }
        }
        if self.eat_punct("]") {
          suffixes.push(Derivation::Array(None));
          continue;
        }
        if self.is_punct("*") && self.peek_at(1).tok == Tok::Punct("]") {
          self.pos += 2;
          suffixes.push(Derivation::Array(None));
          continue;
        }
        let at = self.peek().clone();
        let n = self.conditional()?;
        if n < 0 {
          return Err(error_at(
            at.line,
            at.column,
            "an array cannot have a negative length",
          ));
        }
        self.expect_punct("]")?;
        suffixes.push(Derivation::Array(Some(n as usize)));
      } else if self.is_punct("(") {
        self.pos += 1;
        let (params, variadic) = self.parameters()?;
        suffixes.push(Derivation::Function { params, variadic });
      } else {
        break;
      }
    }

    let trailing = self.attributes()?;
    merge(&mut attrs, &trailing);

    let mut derivations = pointers;
    suffixes.reverse();
    derivations.extend(suffixes);
    derivations.extend(inner);

    Ok(Declarator {
      name,
      line: here.line,
      column: here.column,
      derivations,
      attrs,
    })
  }

  fn parameters(&mut self) -> Parse<(Vec<(Option<String>, TypeRef)>, bool)> {
    let mut params = Vec::new();
    let mut variadic = false;

    if self.eat_punct(")") {
      return Ok((params, false));
    }

    // `(void)` declares no parameters.
    if self.is_ident("void") && self.peek_at(1).tok == Tok::Punct(")") {
      self.pos += 2;
      return Ok((params, false));
    }

    loop {
      if self.eat_punct("...") {
        variadic = true;
        self.expect_punct(")")?;
        break;
      }

      let specs = self.specifiers()?;
      let decl = self.declarator(true)?;
      let mut attrs = specs.attrs.clone();
      merge(&mut attrs, &decl.attrs);
      let ty = self.apply(
        &specs.base,
        &decl.derivations,
        &attrs,
        decl.line,
        decl.column,
      )?;

      // Arrays and functions as parameters are pointers.
      let ty = match &ty.kind {
        Kind::Array { element, .. } => CType::pointer_to(element),
        Kind::Function(_) => CType::pointer_to(&ty),
        _ => ty,
      };

      params.push((decl.name, ty));

      if self.eat_punct(",") {
        continue;
      }
      self.expect_punct(")")?;
      break;
    }

    Ok((params, variadic))
  }

  /// Applies a declarator's derivations to the base type.
  fn apply(
    &self,
    base: &TypeRef,
    derivations: &[Derivation],
    attrs: &Attributes,
    line: usize,
    column: usize,
  ) -> Parse<TypeRef> {
    let mut ty = base.clone();

    for d in derivations {
      ty = match d {
        Derivation::Pointer { is_const, nonnull } => {
          let name = super::types::pointer_name(&ty);
          let p = CType::pointer_with(&ty, None, *nonnull, name);
          if *is_const { CType::constant(&p) } else { p }
        },
        Derivation::Array(n) => {
          if ty.is_void() || matches!(ty.kind, Kind::Function(_)) {
            return Err(error_at(
              line,
              column,
              format!("cannot make an array of '{}'", ty.name),
            ));
          }
          CType::array_of(&ty, *n)
        },
        Derivation::Function { params, variadic } => {
          if matches!(ty.kind, Kind::Function(_) | Kind::Array { .. }) {
            return Err(error_at(
              line,
              column,
              format!("a function cannot return '{}'", ty.name),
            ));
          }
          let sig = Signature {
            returns: ty.clone(),
            params: params.iter().map(|(_, t)| t.clone()).collect(),
            param_names: params.iter().map(|(n, _)| n.clone()).collect(),
            variadic: *variadic,
            abi: attrs.abi.unwrap_or(Abi::Default),
          };
          CType::function(sig)
        },
      };
    }

    Ok(ty)
  }

  /// A type name, as inside `sizeof(...)` or a cast.
  fn type_name(&mut self) -> Parse<TypeRef> {
    let specs = self.specifiers()?;
    let decl = self.declarator(true)?;
    if let Some(name) = &decl.name {
      return Err(error_at(
        decl.line,
        decl.column,
        format!("a type name cannot declare '{name}'"),
      ));
    }
    self.apply(
      &specs.base,
      &decl.derivations,
      &specs.attrs,
      decl.line,
      decl.column,
    )
  }

  // Constant expressions.

  pub fn expression(&mut self) -> Parse<i128> {
    let mut value = self.conditional()?;
    while self.eat_punct(",") {
      value = self.conditional()?;
    }
    Ok(value)
  }

  fn conditional(&mut self) -> Parse<i128> {
    let condition = self.binary(0)?;
    if self.eat_punct("?") {
      let yes = self.expression()?;
      self.expect_punct(":")?;
      let no = self.conditional()?;
      return Ok(if condition != 0 { yes } else { no });
    }
    Ok(condition)
  }

  fn binary(&mut self, min: u8) -> Parse<i128> {
    let mut left = self.unary()?;

    loop {
      let (op, prec) = match &self.peek().tok {
        Tok::Punct(p) => match *p {
          "||" => ("||", 1),
          "&&" => ("&&", 2),
          "|" => ("|", 3),
          "^" => ("^", 4),
          "&" => ("&", 5),
          "==" | "!=" => (*p, 6),
          "<" | ">" | "<=" | ">=" => (*p, 7),
          "<<" | ">>" => (*p, 8),
          "+" | "-" => (*p, 9),
          "*" | "/" | "%" => (*p, 10),
          _ => break,
        },
        _ => break,
      };

      if prec < min.max(1) {
        break;
      }

      let at = self.next();
      let right = self.binary(prec + 1)?;

      left = match op {
        "||" => ((left != 0) || (right != 0)) as i128,
        "&&" => ((left != 0) && (right != 0)) as i128,
        "|" => left | right,
        "^" => left ^ right,
        "&" => left & right,
        "==" => (left == right) as i128,
        "!=" => (left != right) as i128,
        "<" => (left < right) as i128,
        ">" => (left > right) as i128,
        "<=" => (left <= right) as i128,
        ">=" => (left >= right) as i128,
        "<<" => left.wrapping_shl(right as u32),
        ">>" => left.wrapping_shr(right as u32),
        "+" => left.wrapping_add(right),
        "-" => left.wrapping_sub(right),
        "*" => left.wrapping_mul(right),
        "/" | "%" => {
          if right == 0 {
            return Err(error_at(
              at.line,
              at.column,
              "division by zero in a constant expression",
            ));
          }
          if op == "/" {
            left.wrapping_div(right)
          } else {
            left.wrapping_rem(right)
          }
        },
        _ => unreachable!(),
      };
    }

    Ok(left)
  }

  fn unary(&mut self) -> Parse<i128> {
    let t = self.peek().clone();

    match &t.tok {
      Tok::Punct("-") => {
        self.pos += 1;
        Ok(self.unary()?.wrapping_neg())
      },
      Tok::Punct("+") => {
        self.pos += 1;
        self.unary()
      },
      Tok::Punct("~") => {
        self.pos += 1;
        Ok(!self.unary()?)
      },
      Tok::Punct("!") => {
        self.pos += 1;
        Ok((self.unary()? == 0) as i128)
      },
      Tok::Ident(n) if n == "sizeof" || n == "_Alignof" || n == "alignof" || n == "__alignof__" => {
        self.pos += 1;
        let is_size = n == "sizeof";
        self.expect_punct("(")?;
        if !self.starts_type() {
          return Err(error_at(
            t.line,
            t.column,
            format!("{n} takes a type name here"),
          ));
        }
        let ty = self.type_name()?;
        self.expect_punct(")")?;
        let value = if is_size {
          ty.require_size()
        } else {
          ty.require_align()
        };
        value
          .map(|v| v as i128)
          .map_err(|e| error_at(t.line, t.column, e))
      },
      Tok::Punct("(") => {
        // A cast, or a parenthesised expression.
        if matches!(&self.peek_at(1).tok, Tok::Ident(_)) && {
          self.pos += 1;
          let is_type = self.starts_type();
          self.pos -= 1;
          is_type
        } {
          self.pos += 1;
          let ty = self.type_name()?;
          self.expect_punct(")")?;
          let value = self.unary()?;
          return Ok(cast(value, &ty));
        }
        self.pos += 1;
        let value = self.expression()?;
        self.expect_punct(")")?;
        Ok(value)
      },
      _ => self.primary(),
    }
  }

  fn primary(&mut self) -> Parse<i128> {
    let t = self.next();
    match t.tok {
      Tok::Int(v, _) => Ok(v),
      Tok::Char(c) => Ok(c),
      Tok::Float(f) => Ok(f as i128),
      Tok::Ident(ref n) if n == "true" => Ok(1),
      Tok::Ident(ref n) if n == "false" => Ok(0),
      Tok::Ident(ref n) => match self.scope().constant(n) {
        Some(Constant::Int(v)) => Ok(v),
        Some(Constant::Float(f)) => Ok(f as i128),
        Some(Constant::Str(_)) => Err(error_at(
          t.line,
          t.column,
          format!("'{n}' is a string, not a number"),
        )),
        None => Err(error_at(
          t.line,
          t.column,
          format!("'{n}' is not a known constant"),
        )),
      },
      _ => Err(error_at(
        t.line,
        t.column,
        format!("expected a constant, found {}", describe_token(&t)),
      )),
    }
  }
}

fn describe_token(t: &Token) -> String {
  match &t.tok {
    Tok::Eof | Tok::Pack(_) => "the end of the input".into(),
    Tok::Ident(n) => format!("'{n}'"),
    Tok::Punct(p) => format!("'{p}'"),
    Tok::Str(_) => "a string".into(),
    Tok::Int(..) | Tok::Float(_) | Tok::Char(_) => "a number".into(),
  }
}

fn merge(into: &mut Attributes, from: &Attributes) {
  into.packed |= from.packed;
  into.nonnull |= from.nonnull;
  if from.aligned.is_some() {
    into.aligned = from.aligned;
  }
  if from.abi.is_some() {
    into.abi = from.abi;
  }
  if from.symbol.is_some() {
    into.symbol = from.symbol.clone();
  }
}

/// A value converted to an integer type the way a C cast does it.
fn cast(value: i128, ty: &CType) -> i128 {
  match &ty.kind {
    Kind::Bool => (value != 0) as i128,
    Kind::Int { size, signed, .. } if *size < 16 => {
      let bits = size * 8;
      let mask = (1i128 << bits) - 1;
      let v = value & mask;
      if *signed && v >> (bits - 1) & 1 == 1 {
        v | !mask
      } else {
        v
      }
    },
    Kind::Enum(e) => cast(value, &e.underlying),
    _ => value,
  }
}

/// The type an enum without a fixed one gets. GCC and Clang choose
/// `int` when every value fits and the next type up otherwise, and
/// `packed` picks the smallest type that holds them all. MSVC always
/// chooses `int`.
fn enum_underlying(constants: &[(String, i128)], packed: bool) -> TypeRef {
  let low = constants.iter().map(|(_, v)| *v).min().unwrap_or(0);
  let high = constants.iter().map(|(_, v)| *v).max().unwrap_or(0);

  let fits = |size: u32, signed: bool| {
    if signed {
      low >= -(1i128 << (size * 8 - 1)) && high < (1i128 << (size * 8 - 1))
    } else {
      low >= 0 && high < (1i128 << (size * 8))
    }
  };

  if packed && !cfg!(windows) {
    for (size, name_signed, name_unsigned) in [
      (1, "signed char", "unsigned char"),
      (2, "short", "unsigned short"),
      (4, "int", "unsigned int"),
    ] {
      if low >= 0 && fits(size, false) {
        return builtin(name_unsigned).unwrap();
      }
      if fits(size, true) {
        return builtin(name_signed).unwrap();
      }
    }
  }

  if fits(4, true) || cfg!(windows) {
    builtin("int").unwrap()
  } else if fits(4, false) {
    builtin("unsigned int").unwrap()
  } else if fits(8, true) {
    builtin("long long").unwrap()
  } else {
    builtin("unsigned long long").unwrap()
  }
}

/// A function returning `const char *` hands back text it keeps; that
/// return value converts to a string. The same for `const wchar_t *`.
fn text_return(ty: &TypeRef) -> TypeRef {
  let Kind::Pointer(info) = &ty.kind else {
    return ty.clone();
  };

  if !info.target.is_const || info.text.is_some() {
    return ty.clone();
  }

  let target_name = info.target.name.trim_start_matches("const ");
  let encoding = match (&info.target.kind, target_name) {
    (
      Kind::Int {
        size: 1,
        role: IntRole::Character,
        ..
      },
      "char",
    ) => Encoding::Utf8,
    (
      Kind::Int {
        role: IntRole::Character,
        ..
      },
      "wchar_t",
    ) => Encoding::Wide,
    _ => return ty.clone(),
  };

  CType::pointer_with(&info.target, Some(encoding), info.nonnull, ty.name.clone())
}
