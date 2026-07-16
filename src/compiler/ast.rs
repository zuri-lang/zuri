use crate::compiler::token::{Token, TokenKind};
use num_bigint::BigInt;

#[derive(Debug, Clone, PartialEq)]
pub enum Type {
  Any,
  Bool,
  Int,
  Number,
  String,
  Bytes,
  List,
  Dict,
  Range,
  File,
  Function,
  Type,
  Callable,
  Iterable,
  Instance(Token),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
  Nil,
  Bool(bool),
  Integer(i64),
  Float(f64),
  BigNumber(BigInt),
  Literal(String),
  Unary(TokenKind, Box<Expr>),
  Binary(Box<Expr>, TokenKind, Box<Expr>),
  Logical(Box<Expr>, TokenKind, Box<Expr>),
  Circuit(Box<Expr>, TokenKind, Box<Expr>),
  Grouping(Box<Expr>),
  Range(Box<Expr>, Box<Expr>),
  Identifier(Token),
  Condition(Box<Expr>, Box<Expr>, Box<Expr>),
  Call(Box<Expr>, Vec<Expr>),
  Get(Box<Expr>, Token),
  Set(Box<Expr>, Token, Box<Expr>),
  Index(Box<Expr>, Box<Expr>),
  Slice(Box<Expr>, Box<Expr>, Box<Expr>),
  List(Vec<Expr>),
  Dict(Vec<Expr>, Vec<Expr>),
  Parent,
  Self_,
  Assign(Box<Expr>, Box<Expr>),
  Anonymous(Box<Decl>),
  TypeHint(Vec<Type>, bool),
  Argument(Token, Box<Expr>),
}

impl From<Expr> for NodeKind {
  fn from(e: Expr) -> Self {
    NodeKind::Expr(e)
  }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Stmt {
  None,
  FixContinue,
  Echo(Box<Expr>),
  Expression(Box<Expr>),
  If(Box<Expr>, Box<Stmt>, Option<Box<Stmt>>),
  While(Box<Expr>, Box<Stmt>),
  Continue,
  Break,
  Raise(Box<Expr>),
  Return(Box<Expr>),
  Assert(Box<Expr>, Option<Box<Expr>>),
  Using(Box<Expr>, Vec<Expr>, Vec<Stmt>, Option<Box<Stmt>>),
  Import(String, Box<Expr>, Vec<Expr>, bool),
  Catch(Box<Stmt>, Option<Box<Stmt>>, Option<Box<Expr>>),
  Block(Vec<Stmt>),
  Decl(Box<Decl>),
  Var(Token, Box<Expr>, Option<Box<Expr>>, bool),
  VarList(Vec<Stmt>),
}

impl From<Stmt> for NodeKind {
  fn from(e: Stmt) -> Self {
    NodeKind::Stmt(e)
  }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Decl {
  None,
  Stmt(Box<Stmt>),
  Block(Vec<Stmt>),
  Import(String, Box<Expr>, Vec<Expr>, bool),
  Function(Token, Vec<Expr>, Box<Stmt>, bool),
  Method(Token, Vec<Expr>, Box<Stmt>, bool, bool),
  Property(Token, Box<Expr>, Box<Expr>, bool, bool),
  Class(Token, Option<Box<Expr>>, Vec<Decl>, Vec<Decl>, bool),
}

impl Decl {
  pub fn is_method(&self, name: &str) -> bool {
    match self {
      Decl::Method(token, _, _, _, _) => match token.kind.clone() {
        TokenKind::Identifier(token_name) => token_name.eq(name),
        _ => false,
      },
      _ => false,
    }
  }
}

impl From<Decl> for NodeKind {
  fn from(e: Decl) -> Self {
    NodeKind::Decl(e)
  }
}

#[derive(Debug, Clone, PartialEq)]
pub enum NodeKind {
  Type(Type),
  Expr(Expr),
  Stmt(Stmt),
  Decl(Decl),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Node {
  pub kind: NodeKind,
  pub line: usize,
  pub col: usize,
}

impl From<Node> for NodeKind {
  fn from(e: Node) -> Self {
    e.kind
  }
}

impl From<&mut Node> for NodeKind {
  fn from(e: &mut Node) -> Self {
    e.clone().kind
  }
}

impl Node {
  pub fn mark_from(&mut self, token: Token) -> Self {
    self.line = token.line;
    self.col = token.column;
    self.clone()
  }
}
