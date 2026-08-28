//! `_zuri_lex` builtin module; the native backing for `zuri.tokenize()`.
//!
//! Runs the real `Lexer` over a source string and hands back every
//! token it produces, in order, as a plain dict; comments, doc
//! blocks, and newlines included, nothing filtered the way the parser
//! filters them. `libs/zuri/token.zu` wraps each dict into a proper
//! `Token` instance; this file only builds the raw data.

use crate::builtins::enforce::ArgType;
use crate::compiler::lexer::Lexer;
use crate::compiler::token::TokenKind;
use crate::enforce_arg_count;
use crate::enforce_arg_type;
use crate::modules::{BuiltinModuleDef, native};
use crate::vm::object::ZuriContext;
use crate::vm::value::Value;
use crate::vm::vm::VM;

pub static MODULE: BuiltinModuleDef = BuiltinModuleDef {
  name: "_zuri_lex",
  build,
};

fn build(vm: &mut VM) -> Vec<(&'static str, Value)> {
  vec![("tokenize", native(vm, "tokenize", 1, false, tokenize_fn))]
}

/// This token kind's variant name, exactly as written in
/// `TokenKind`'s own definition (`src/compiler/token.rs`); the string a
/// caller compares `token.kind` against. Also reused by `zuri_parse`
/// to name an `Expr::Binary`/`Unary`/`Logical`/`Circuit` node's own
/// operator, so an AST consumer sees the exact same operator names
/// `tokenize()` already uses rather than a second, only-loosely-
/// related vocabulary.
pub(crate) fn token_kind_name(kind: &TokenKind) -> &'static str {
  match kind {
    TokenKind::None => "None",
    TokenKind::Newline => "Newline",
    TokenKind::Lparen => "Lparen",
    TokenKind::Rparen => "Rparen",
    TokenKind::Lbracket => "Lbracket",
    TokenKind::Rbracket => "Rbracket",
    TokenKind::Lbrace => "Lbrace",
    TokenKind::Rbrace => "Rbrace",
    TokenKind::Semicolon => "Semicolon",
    TokenKind::Comma => "Comma",
    TokenKind::Backslash => "Backslash",
    TokenKind::Bang => "Bang",
    TokenKind::BangEq => "BangEq",
    TokenKind::Colon => "Colon",
    TokenKind::At => "At",
    TokenKind::Dot => "Dot",
    TokenKind::Range => "Range",
    TokenKind::TriDot => "TriDot",
    TokenKind::Plus => "Plus",
    TokenKind::PlusEq => "PlusEq",
    TokenKind::Increment => "Increment",
    TokenKind::Minus => "Minus",
    TokenKind::MinusEq => "MinusEq",
    TokenKind::Decrement => "Decrement",
    TokenKind::Multiply => "Multiply",
    TokenKind::MultiplyEq => "MultiplyEq",
    TokenKind::Pow => "Pow",
    TokenKind::PowEq => "PowEq",
    TokenKind::Divide => "Divide",
    TokenKind::DivideEq => "DivideEq",
    TokenKind::Floor => "Floor",
    TokenKind::FloorEq => "FloorEq",
    TokenKind::Equal => "Equal",
    TokenKind::EqualEq => "EqualEq",
    TokenKind::Less => "Less",
    TokenKind::LessEq => "LessEq",
    TokenKind::Lshift => "Lshift",
    TokenKind::LshiftEq => "LshiftEq",
    TokenKind::Greater => "Greater",
    TokenKind::GreaterEq => "GreaterEq",
    TokenKind::Rshift => "Rshift",
    TokenKind::RshiftEq => "RshiftEq",
    TokenKind::Urshift => "Urshift",
    TokenKind::UrshiftEq => "UrshiftEq",
    TokenKind::Percent => "Percent",
    TokenKind::PercentEq => "PercentEq",
    TokenKind::Amp => "Amp",
    TokenKind::AmpEq => "AmpEq",
    TokenKind::Bar => "Bar",
    TokenKind::BarEq => "BarEq",
    TokenKind::Tilde => "Tilde",
    TokenKind::TildeEq => "TildeEq",
    TokenKind::Xor => "Xor",
    TokenKind::XorEq => "XorEq",
    TokenKind::Question => "Question",
    TokenKind::Arrow => "Arrow",
    TokenKind::And => "And",
    TokenKind::As => "As",
    TokenKind::Assert => "Assert",
    TokenKind::Break => "Break",
    TokenKind::Catch => "Catch",
    TokenKind::Class => "Class",
    TokenKind::Const => "Const",
    TokenKind::Continue => "Continue",
    TokenKind::Def => "Def",
    TokenKind::Default => "Default",
    TokenKind::Do => "Do",
    TokenKind::Echo => "Echo",
    TokenKind::Else => "Else",
    TokenKind::False => "False",
    TokenKind::For => "For",
    TokenKind::If => "If",
    TokenKind::Import => "Import",
    TokenKind::In => "In",
    TokenKind::Iter => "Iter",
    TokenKind::Nil => "Nil",
    TokenKind::Or => "Or",
    TokenKind::Parent => "Parent",
    TokenKind::Raise => "Raise",
    TokenKind::Return => "Return",
    TokenKind::Self_ => "Self_",
    TokenKind::Static => "Static",
    TokenKind::True => "True",
    TokenKind::Using => "Using",
    TokenKind::Var => "Var",
    TokenKind::When => "When",
    TokenKind::While => "While",
    TokenKind::Literal(_) => "Literal",
    TokenKind::BigNumber(_) => "BigNumber",
    TokenKind::Integer(_) => "Integer",
    TokenKind::Double(_) => "Double",
    TokenKind::BinNumber(_) => "BinNumber",
    TokenKind::OctNumber(_) => "OctNumber",
    TokenKind::HexNumber(_) => "HexNumber",
    TokenKind::Identifier(_) => "Identifier",
    TokenKind::Decorator(_) => "Decorator",
    TokenKind::Interpolation(_) => "Interpolation",
    TokenKind::Comment(_) => "Comment",
    TokenKind::DocBlock(_) => "DocBlock",
    TokenKind::Eof => "Eof",
    TokenKind::Error(..) => "Error",
  }
}

/// This token kind's payload, as a `Value`, for the ones that carry
/// one; `nil` for every fixed symbol/keyword, which has nothing beyond
/// its own `kind` name to report.
fn token_kind_value(ctx: &mut ZuriContext, kind: &TokenKind) -> Value {
  match kind {
    TokenKind::Literal(s)
    | TokenKind::Identifier(s)
    | TokenKind::Decorator(s)
    | TokenKind::Interpolation(s)
    | TokenKind::Comment(s)
    | TokenKind::DocBlock(s) => ctx.heap().alloc_string(s.clone()),
    // This runtime has no separate int type; every numeric token kind
    // becomes an ordinary Zuri number, same as the compiler itself
    // does for `Expr::Integer`/`Expr::Float`. A bigint literal keeps
    // its own bigint representation, since that's a real distinct
    // runtime type here.
    TokenKind::Integer(n) => Value::number(*n as f64),
    TokenKind::Double(n) => Value::number(*n),
    TokenKind::BinNumber(n) | TokenKind::OctNumber(n) | TokenKind::HexNumber(n) => {
      Value::number(*n as f64)
    },
    TokenKind::BigNumber(n) => ctx.heap().alloc_bigint(n.clone()),
    // A malformed-input error token; message plus the (line, offset)
    // the lexer itself reported, distinct from the token's own
    // line/column (which is where the error TOKEN starts, not
    // necessarily the same position the lexer's own diagnostic names).
    TokenKind::Error(message, line, offset) => {
      let message_key = ctx.heap().alloc_string("message");
      let message_value = ctx.heap().alloc_string(message.clone());
      let line_key = ctx.heap().alloc_string("line");
      let offset_key = ctx.heap().alloc_string("offset");
      ctx.heap().alloc_dict(vec![
        (message_key, message_value),
        (line_key, Value::number(*line as f64)),
        (offset_key, Value::number(*offset as f64)),
      ])
    },
    _ => Value::nil(),
  }
}

fn token_to_dict(ctx: &mut ZuriContext, source: &[char], token: &crate::compiler::token::Token) -> Value {
  let kind_key = ctx.heap().alloc_string("kind");
  let kind_value = ctx.heap().alloc_string(token_kind_name(&token.kind));

  let line_key = ctx.heap().alloc_string("line");
  let column_key = ctx.heap().alloc_string("column");
  let start_key = ctx.heap().alloc_string("start");
  let end_key = ctx.heap().alloc_string("end");

  let text: String = source[token.start..token.end].iter().collect();
  let text_key = ctx.heap().alloc_string("text");
  let text_value = ctx.heap().alloc_string(text);

  let value_key = ctx.heap().alloc_string("value");
  let value_value = token_kind_value(ctx, &token.kind);

  ctx.heap().alloc_dict(vec![
    (kind_key, kind_value),
    (line_key, Value::number(token.line as f64)),
    (column_key, Value::number(token.column as f64)),
    (start_key, Value::number(token.start as f64)),
    (end_key, Value::number(token.end as f64)),
    (text_key, text_value),
    (value_key, value_value),
  ])
}

/// `_zuri_lex.tokenize(source)`: scans `source` in full and returns
/// every token the lexer produces, in source order, as a list of
/// dicts shaped `{ kind, line, column, start, end, text, value }`.
///
/// `kind` is the `TokenKind` variant name (`"Identifier"`, `"Plus"`,
/// `"Comment"`, ...). `line`/`column` are 1-indexed; `start`/`end` are
/// character offsets into `source` (not bytes) spanning the token's
/// exact text, which `text` already holds pre-sliced. `value` carries
/// the token's payload where one exists (a literal/identifier/comment's
/// string, a number, a bigint) and is `nil` for every fixed symbol or
/// keyword.
///
/// Nothing is filtered out: `Comment`/`DocBlock`/`Newline` tokens are
/// included (the parser's own grammar skips these, but `tokenize()`
/// reports the lexer's raw output), and the list always ends with one
/// `Eof` entry. A lexer-level problem (an unterminated string, an
/// unbalanced doc block, an unexpected character) does not raise;
/// it surfaces as an ordinary `Error`-kind entry in the list, with the
/// problem description in `value.message`, so this function is total
/// over any input, including malformed source.
fn tokenize_fn(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 1);
  enforce_arg_type!(ctx, 0, ArgType::String);

  let source_str = ctx.args[0].as_str().to_string();
  let source_chars: Vec<char> = source_str.chars().collect();
  let mut lexer = Lexer::new(&source_str);

  let mut tokens = Vec::new();
  loop {
    let tok = lexer.scan();
    let is_eof = matches!(tok.kind, TokenKind::Eof);
    tokens.push(token_to_dict(ctx, &source_chars, &tok));
    if is_eof {
      break;
    }
  }

  Ok(ctx.heap().alloc_list(tokens))
}
