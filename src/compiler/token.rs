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

  //  * end of file
  Eof,
  Error(String, usize, usize),
}

impl fmt::Display for TokenKind {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    match self {
      // TokenKind::Eof => write!(f, "{}", "Eof"),

      // types token
      TokenKind::Identifier(s) => write!(f, "Identifier{{v={}}}", s),
      TokenKind::Decorator(s) => write!(f, "Decorator{{v={}}}", s),
      TokenKind::Interpolation(s) => write!(f, "Interpolation{{v={}}}", s),
      TokenKind::Literal(s) => write!(f, "Literal{{v={}}}", s),
      TokenKind::BigNumber(s) => write!(f, "BigNumber{{v={}}}", s),
      TokenKind::Integer(n) => write!(f, "Integer{{v={}}}", n),
      TokenKind::Double(n) => write!(f, "Double{{v={}}}", n),
      TokenKind::BinNumber(s) => write!(f, "BinNumber{{v={}}}", s),
      TokenKind::OctNumber(n) => write!(f, "OctNumber{{v={}}}", n),
      TokenKind::HexNumber(n) => write!(f, "HexNumber{{v={}}}", n),

      _ => write!(f, "{:?}", self),
    }
  }
}

#[derive(Clone, PartialEq)]
pub struct Token {
  pub kind: TokenKind,
  pub line: usize,
  pub column: usize,
}

impl Token {
  #[inline]
  pub fn new(kind: TokenKind, line: usize, column: usize) -> Self {
    Self { kind, line, column }
  }

  #[inline]
  pub fn copy_to(&self, kind: TokenKind) -> Self {
    Self::new(kind, self.line, self.column)
  }

  /// Human-readable label for this token, used in syntax-error
  /// messages -- e.g. "def" for a `Def` keyword, "x" for
  /// `Identifier("x")`, "@my_decorator" for a decorator. Falls back to
  /// a lowercased Debug form for symbol/keyword tokens without their
  /// own payload; not hand-tuned per symbol, but covers every current
  /// TokenKind reasonably.
  pub fn describe(&self) -> String {
    match &self.kind {
      TokenKind::Literal(s) | TokenKind::Identifier(s) | TokenKind::Interpolation(s) => s.clone(),
      TokenKind::Decorator(s) => format!("@{}", s),
      TokenKind::Eof => "end of input".to_string(),
      other => format!("{:?}", other).to_lowercase(),
    }
  }
}

impl fmt::Display for Token {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    match self.kind.clone() {
      TokenKind::Literal(s) | TokenKind::Interpolation(s) => write!(f, "{}", s),
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
    write!(
      f,
      "Token{{kind={} at line={}, column={}}}",
      self.kind, self.line, self.column
    )
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
