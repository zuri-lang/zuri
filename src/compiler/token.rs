use std::fmt::{self, Debug};

use num_bigint::BigInt;

#[derive(Clone, Debug, PartialEq)]
pub enum TokenKind {
  None,

  // symbols
  Newline,
  Lparen,
  Rparen,
  Lbracket,
  Rbracket,
  Lbrace,
  Rbrace,
  Semicolon,
  Comma,
  Backslash,
  Bang,
  BangEq,
  Colon,
  At,
  Dot,
  Range,
  TriDot,
  Plus,
  PlusEq,
  Increment,
  Minus,
  MinusEq,
  Decrement,
  Multiply,
  MultiplyEq,
  Pow,
  PowEq,
  Divide,
  DivideEq,
  Floor,
  FloorEq,
  Equal,
  EqualEq,
  Less,
  LessEq,
  Lshift,
  LshiftEq,
  Greater,
  GreaterEq,
  Rshift,
  RshiftEq,
  Urshift,
  UrshiftEq,
  Percent,
  PercentEq,
  Amp,
  AmpEq,
  Bar,
  BarEq,
  Tilde,
  TildeEq,
  Xor,
  XorEq,
  Question,
  Arrow,

  // keywords
  And,
  As,
  Assert,
  Break,
  Catch,
  Class,
  Const,
  Continue,
  Def,
  Default,
  Do,
  Echo,
  Else,
  False,
  For,
  If,
  Import,
  In,
  Iter,
  Nil,
  Or,
  Parent,
  Raise,
  Return,
  Self_,
  Static,
  True,
  Using,
  Var,
  When,
  While,

  // types token
  Literal(String),
  BigNumber(BigInt),
  Integer(i64),
  Double(f64),
  BinNumber(i64),
  OctNumber(i64),
  HexNumber(i64),
  Identifier(String),
  Decorator(String),
  Interpolation(String),
  Comment(String),
  DocBlock(String),

  // end of file
  Eof,
  Error(String, usize, usize),
}

impl fmt::Display for TokenKind {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    match self {
      TokenKind::None => write!(f, ""),
      TokenKind::Eof => write!(f, "{}", "<EOF>"),

      // types token
      TokenKind::Error(s, line, column) => {
        write!(f, "Error{{v={}, line={}, column={}}}", s, line, column)
      },
      TokenKind::Identifier(s) => write!(f, "Identifier{{v={}}}", s),
      TokenKind::Decorator(s) => write!(f, "Decorator{{v={}}}", s),
      TokenKind::Interpolation(s) => write!(f, "Interpolation{{v={}}}", s),
      TokenKind::Literal(s) => write!(f, "Literal{{v={}}}", s),
      TokenKind::Comment(s) => write!(f, "Comment{{v={}}}", s),
      TokenKind::DocBlock(s) => write!(f, "DocBlock{{v={}}}", s),
      TokenKind::BigNumber(s) => write!(f, "BigNumber{{v={}}}", s),
      TokenKind::Integer(n) => write!(f, "Integer{{v={}}}", n),
      TokenKind::Double(n) => write!(f, "Double{{v={}}}", n),
      TokenKind::BinNumber(s) => write!(f, "BinNumber{{v={}}}", s),
      TokenKind::OctNumber(n) => write!(f, "OctNumber{{v={}}}", n),
      TokenKind::HexNumber(n) => write!(f, "HexNumber{{v={}}}", n),
      TokenKind::Newline => write!(f, "Newline"),

      // symbols
      TokenKind::Lparen => write!(f, "("),
      TokenKind::Rparen => write!(f, ")"),
      TokenKind::Lbracket => write!(f, "["),
      TokenKind::Rbracket => write!(f, "]"),
      TokenKind::Lbrace => write!(f, "{{"),
      TokenKind::Rbrace => write!(f, "}}"),
      TokenKind::Semicolon => write!(f, ";"),
      TokenKind::Comma => write!(f, ","),
      TokenKind::Backslash => write!(f, "\\"),
      TokenKind::Bang => write!(f, "!"),
      TokenKind::BangEq => write!(f, "!="),
      TokenKind::Colon => write!(f, ":"),
      TokenKind::At => write!(f, "@"),
      TokenKind::Dot => write!(f, "."),
      TokenKind::Range => write!(f, ".."),
      TokenKind::TriDot => write!(f, "..."),
      TokenKind::Plus => write!(f, "+"),
      TokenKind::PlusEq => write!(f, "+="),
      TokenKind::Increment => write!(f, "++"),
      TokenKind::Minus => write!(f, "-"),
      TokenKind::MinusEq => write!(f, "-="),
      TokenKind::Decrement => write!(f, "--"),
      TokenKind::Multiply => write!(f, "*"),
      TokenKind::MultiplyEq => write!(f, "*="),
      TokenKind::Pow => write!(f, "**"),
      TokenKind::PowEq => write!(f, "**="),
      TokenKind::Divide => write!(f, "/"),
      TokenKind::DivideEq => write!(f, "/="),
      TokenKind::Floor => write!(f, "//"),
      TokenKind::FloorEq => write!(f, "//="),
      TokenKind::Equal => write!(f, "="),
      TokenKind::EqualEq => write!(f, "=="),
      TokenKind::Less => write!(f, "<"),
      TokenKind::LessEq => write!(f, "<="),
      TokenKind::Lshift => write!(f, "<<"),
      TokenKind::LshiftEq => write!(f, "<<="),
      TokenKind::Greater => write!(f, ">"),
      TokenKind::GreaterEq => write!(f, ">="),
      TokenKind::Rshift => write!(f, ">>"),
      TokenKind::RshiftEq => write!(f, ">>="),
      TokenKind::Urshift => write!(f, ">>>"),
      TokenKind::UrshiftEq => write!(f, ">>>="),
      TokenKind::Percent => write!(f, "%"),
      TokenKind::PercentEq => write!(f, "%="),
      TokenKind::Amp => write!(f, "&"),
      TokenKind::AmpEq => write!(f, "&="),
      TokenKind::Bar => write!(f, "|"),
      TokenKind::BarEq => write!(f, "|="),
      TokenKind::Tilde => write!(f, "~"),
      TokenKind::TildeEq => write!(f, "~="),
      TokenKind::Xor => write!(f, "^"),
      TokenKind::XorEq => write!(f, "^="),
      TokenKind::Question => write!(f, "?"),
      TokenKind::Arrow => write!(f, "->"),

      // keywords
      TokenKind::And => write!(f, "and"),
      TokenKind::As => write!(f, "as"),
      TokenKind::Assert => write!(f, "assert"),
      TokenKind::Break => write!(f, "break"),
      TokenKind::Catch => write!(f, "catch"),
      TokenKind::Class => write!(f, "class"),
      TokenKind::Const => write!(f, "const"),
      TokenKind::Continue => write!(f, "continue"),
      TokenKind::Def => write!(f, "def"),
      TokenKind::Default => write!(f, "default"),
      TokenKind::Do => write!(f, "do"),
      TokenKind::Echo => write!(f, "echo"),
      TokenKind::Else => write!(f, "else"),
      TokenKind::False => write!(f, "false"),
      TokenKind::For => write!(f, "for"),
      TokenKind::If => write!(f, "if"),
      TokenKind::Import => write!(f, "import"),
      TokenKind::In => write!(f, "in"),
      TokenKind::Iter => write!(f, "iter"),
      TokenKind::Nil => write!(f, "nil"),
      TokenKind::Or => write!(f, "or"),
      TokenKind::Parent => write!(f, "parent"),
      TokenKind::Raise => write!(f, "raise"),
      TokenKind::Return => write!(f, "return"),
      TokenKind::Self_ => write!(f, "self"),
      TokenKind::Static => write!(f, "static"),
      TokenKind::True => write!(f, "true"),
      TokenKind::Using => write!(f, "using"),
      TokenKind::Var => write!(f, "var"),
      TokenKind::When => write!(f, "when"),
      TokenKind::While => write!(f, "while"),
    }
  }
}

#[derive(Clone, PartialEq)]
pub struct Token {
  pub kind: TokenKind,
  pub line: usize,
  pub column: usize,
  // Char offsets (not bytes; the lexer scans a Vec<char>) into the
  // source this token came from, covering the token's exact text
  // including any delimiters (a string literal's quotes, a doc block's
  // `/*`/`*/`, ...). Lets a caller recover a token's literal source
  // slice without re-lexing, which is what `zuri.tokenize()` needs for
  // its `text` field.
  pub start: usize,
  pub end: usize,
}

impl Token {
  #[inline]
  pub fn new(kind: TokenKind, line: usize, column: usize, start: usize, end: usize) -> Self {
    Self {
      kind,
      line,
      column,
      start,
      end,
    }
  }

  #[inline]
  pub fn copy_to(&self, kind: TokenKind) -> Self {
    Self::new(kind, self.line, self.column, self.start, self.end)
  }

  /// Human-readable label for this token, used in syntax-error
  /// messages; e.g. "def" for a `Def` keyword, "x" for
  /// `Identifier("x")`, "@my_decorator" for a decorator. Falls back to
  /// a lowercased Debug form for symbol/keyword tokens without their
  /// own payload; not hand-tuned per symbol, but covers every current
  /// TokenKind reasonably.
  pub fn describe(&self) -> String {
    match &self.kind {
      TokenKind::Literal(s)
      | TokenKind::Identifier(s)
      | TokenKind::Interpolation(s)
      | TokenKind::Comment(s)
      | TokenKind::DocBlock(s) => s.clone(),
      TokenKind::Decorator(s) => format!("@{}", s),
      TokenKind::BigNumber(s) => format!("{}n", s.to_string()),
      TokenKind::Integer(n) => format!("{}", n),
      TokenKind::Double(n) => format!("{}", n),
      TokenKind::BinNumber(s) => format!("{:#b}", s),
      TokenKind::OctNumber(n) => format!("0c{:o}", n),
      TokenKind::HexNumber(n) => format!("{:#x}", n),
      TokenKind::Eof => "<eof>".to_string(),
      TokenKind::Newline => "<newline>".to_string(),
      other => format!("{}", other).to_lowercase(),
    }
  }
}

impl fmt::Display for Token {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    match self.kind.clone() {
      TokenKind::Literal(s)
      | TokenKind::Interpolation(s)
      | TokenKind::Comment(s)
      | TokenKind::DocBlock(s) => {
        write!(f, "{}", s)
      },
      TokenKind::Decorator(s) => write!(f, "{}", s),
      _ => write!(
        f,
        "Token{{kind={} at line={}, column={}}}",
        self.kind, self.line, self.column
      ),
    }
  }
}

impl fmt::Debug for Token {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    write!(f, "{}", self)
  }
}

#[macro_export]
macro_rules! assignment_operators {
  () => {
    TokenKind::Equal
      | TokenKind::PlusEq
      | TokenKind::MinusEq
      | TokenKind::MultiplyEq
      | TokenKind::DivideEq
      | TokenKind::FloorEq
      | TokenKind::PowEq
      | TokenKind::PercentEq
      | TokenKind::AmpEq
      | TokenKind::BarEq
      | TokenKind::TildeEq
      | TokenKind::XorEq
      | TokenKind::LshiftEq
      | TokenKind::RshiftEq
      | TokenKind::UrshiftEq
  };
}

#[macro_export]
macro_rules! factor_operators {
  () => {
    TokenKind::Multiply | TokenKind::Divide | TokenKind::Floor | TokenKind::Pow | TokenKind::Percent
  };
}

#[macro_export]
macro_rules! term_operators {
  () => {
    TokenKind::Plus | TokenKind::Minus
  };
}

#[macro_export]
macro_rules! equality_operators {
  () => {
    TokenKind::EqualEq | TokenKind::BangEq
  };
}

#[macro_export]
macro_rules! shift_operators {
  () => {
    TokenKind::Lshift | TokenKind::Rshift | TokenKind::Urshift
  };
}

#[macro_export]
macro_rules! comparison_operators {
  () => {
    TokenKind::EqualEq
      | TokenKind::BangEq
      | TokenKind::Less
      | TokenKind::LessEq
      | TokenKind::Greater
      | TokenKind::GreaterEq
  };
}

#[macro_export]
macro_rules! unary_operators {
  () => {
    TokenKind::Bang | TokenKind::Minus | TokenKind::Tilde
  };
}

pub fn get_assignment_alt(kind: TokenKind) -> TokenKind {
  match kind {
    TokenKind::PlusEq => TokenKind::Plus,
    TokenKind::MinusEq => TokenKind::Minus,
    TokenKind::MultiplyEq => TokenKind::Multiply,
    TokenKind::DivideEq => TokenKind::Divide,
    TokenKind::FloorEq => TokenKind::Floor,
    TokenKind::PowEq => TokenKind::Pow,
    TokenKind::PercentEq => TokenKind::Percent,
    TokenKind::AmpEq => TokenKind::Amp,
    TokenKind::BarEq => TokenKind::Bar,
    TokenKind::TildeEq => TokenKind::Tilde,
    TokenKind::XorEq => TokenKind::Xor,
    TokenKind::LshiftEq => TokenKind::Lshift,
    TokenKind::RshiftEq => TokenKind::Rshift,
    TokenKind::UrshiftEq => TokenKind::Urshift,
    _ => kind,
  }
}

pub static EMPTY_TOKEN: Token = Token {
  kind: TokenKind::Eof,
  line: 0,
  column: 0,
  start: 0,
  end: 0,
};

pub static KEYWORD_TOKENS: &[TokenKind] = &[
  TokenKind::And,
  TokenKind::As,
  TokenKind::Assert,
  TokenKind::Break,
  TokenKind::Catch,
  TokenKind::Class,
  TokenKind::Const,
  TokenKind::Continue,
  TokenKind::Def,
  TokenKind::Default,
  TokenKind::Do,
  TokenKind::Echo,
  TokenKind::Else,
  TokenKind::False,
  TokenKind::For,
  TokenKind::If,
  TokenKind::Import,
  TokenKind::In,
  TokenKind::Iter,
  TokenKind::Nil,
  TokenKind::Or,
  TokenKind::Parent,
  TokenKind::Raise,
  TokenKind::Return,
  TokenKind::Self_,
  TokenKind::Static,
  TokenKind::True,
  TokenKind::Using,
  TokenKind::Var,
  TokenKind::When,
  TokenKind::While,
];
