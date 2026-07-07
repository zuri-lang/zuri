use crate::token::{Token, TokenKind};
use big_num::BigInt;

#[derive(Debug, Clone, PartialEq)]
pub enum Primitive {
  Void,
  Bool,
  Int8,
  Int16,
  Int32,
  Int64,
  UInt8,
  UInt16,
  UInt32,
  UInt64,
  Float32,
  Float64,
  String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Type {
  Infer,
  Primitive(Primitive),
  Defined(Token),
  Typed(Box<Type>, Box<Type>),
  Vector(Box<Type>, Vec<Type>),
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
  Range(Box<Expr>, Box<Expr>),
  Grouping(Box<Expr>),
  Identifier(Token),
  Condition(Box<Expr>, Box<Expr>, Box<Expr>),
  Call(Box<Expr>, Vec<Expr>),
  Get(Box<Expr>, Token),
  Set(Box<Expr>, Token, Box<Expr>),
  Index(Box<Expr>, Box<Expr>),
  Slice(Box<Expr>, Box<Expr>, Box<Expr>),
  List(Vec<Expr>),
  Dict(Vec<Expr>, Vec<Expr>),
  New(Box<Type>, Vec<Expr>),
  Parent,
  Self_,
  Assign(Box<Expr>, Box<Expr>),
  Anonymous(Box<Decl>),
  Argument(Token, Box<Type>),
}

impl From<Expr> for NodeKind {
  fn from(e: Expr) -> Self {
    NodeKind::Expr(e)
  }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Stmt {
  None,
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
  Var(Token, Box<Expr>, Box<Type>, bool),
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
  Function(Token, Vec<Expr>, Box<Type>, Box<Stmt>, bool),
  Method(Token, Vec<Expr>, Box<Type>, Box<Stmt>, bool, bool),
  Property(Token, Box<Expr>, Box<Type>, bool, bool),
  Class(Token, Option<Box<Expr>>, Vec<Decl>, Vec<Decl>, bool),
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
