#![allow(unused)]

use std::fmt::{self, Display};

use crate::compiler::ast::{Decl, Expr, Node, NodeKind, Stmt, Type};
use crate::compiler::lexer::Lexer;
use crate::compiler::token::*;

use crate::{
  assignment_operators, comparison_operators, equality_operators, factor_operators,
  shift_operators, term_operators, unary_operators,
};

macro_rules! match_tok {
  ($self:ident, $($pattern:pat_param)|+) => {
    if matches!($self.current.kind.clone(), $($pattern)|+) {
      $self.advance();
      true
    } else {
      false
    }
  };
}

macro_rules! check_tok {
  ($self:ident, $($pattern:pat_param)|+) => {
    if matches!($self.current.kind.clone(), $($pattern)|+) {
      true
    } else {
      false
    }
  };
}

macro_rules! consume_tok {
  ($self:ident, $($pattern:pat_param)|+, $message:expr) => {
    if matches!($self.current.kind.clone(), $($pattern)|+) {
      $self.advance().clone()
    } else {
      $self.report_error($message.to_string());
      EMPTY_TOKEN.clone()
    }
  };
}

#[derive(Debug, Clone)]
pub struct ParserError {
  pub message: String,
  pub line_number: usize,
  pub offset: usize,
  pub length: usize,
  pub token_text: String,
}

impl ParserError {
  pub fn new(message: String, token: Token) -> Self {
    Self {
      message: message,
      line_number: token.line,
      offset: token.column,
      length: format!("{}", token).len(),
      token_text: token.describe(),
    }
  }
}

impl Display for ParserError {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    write!(f, "SyntaxError at '{}': {}", self.token_text, self.message)
  }
}

/// A snapshot of "where in the source the next node should be anchored".
///
/// Call `.finish()` once you've built the Expr/Stmt/Decl (or another
/// Node) it should wrap.
#[derive(Clone, Copy)]
struct Checkpoint {
  line: usize,
  col: usize,
}

impl Checkpoint {
  fn finish<T: Into<NodeKind>>(self, kind: T) -> Node {
    Node {
      kind: kind.into(),
      line: self.line,
      col: self.col,
    }
  }
}

impl Display for Checkpoint {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    write!(f, "CheckPoint{{line={}, col={}}}", self.line, self.col)
  }
}

pub struct Parser<'a> {
  lexer: &'a mut Lexer<'a>,
  block_count: usize,
  current: Token,
  previous: Token,
  last_previous: Token,
  anonymous_count: usize,
  functions_count: usize,
  pub errors: Vec<ParserError>,
}

impl<'a> Display for Parser<'a> {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    write!(
      f,
      "Parser{{current={}, blocks={}, anonymous={}, errors={}}}",
      self.current,
      self.block_count,
      self.anonymous_count,
      self.errors.len()
    )
  }
}

impl<'a> Parser<'a> {
  pub fn new(lexer: &'a mut Lexer<'a>) -> Self {
    Self {
      lexer: lexer,
      block_count: 0,
      current: Token {
        kind: TokenKind::None,
        line: 0,
        column: 0,
      },
      previous: Token {
        kind: TokenKind::None,
        line: 0,
        column: 0,
      },
      last_previous: Token {
        kind: TokenKind::None,
        line: 0,
        column: 0,
      },
      anonymous_count: 0,
      functions_count: 0,
      errors: Vec::new(),
    }
  }

  //-----------------------------------------------------------------------------------
  // Utility
  //-----------------------------------------------------------------------------------

  fn report_error(&mut self, message: String) {
    let line = self.lexer.line.to_string();
    let token = self.peek().clone();
    self.errors.push(ParserError::new(message, token));
  }

  fn mark(&self) -> Checkpoint {
    Checkpoint {
      line: self.lexer.line,
      col: self.lexer.current,
    }
  }

  // Thin sugar over mark()/finish() for the common case of a single leaf
  // expression — kept because closures are genuinely fine here: there's only
  // ever one mutable borrow of `self` in play (the closure's own parameter).
  pub fn w<F, T>(&mut self, f: F) -> Node
  where
    F: FnOnce(&mut Self) -> T,
    T: Into<NodeKind>,
  {
    let start = self.mark();
    start.finish(f(self))
  }

  #[inline]
  fn peek(&self) -> &Token {
    &self.current
  }

  fn peek_next(&mut self) -> Token {
    self.advance();
    let value = self.peek().clone();
    self.rewind();
    value
  }

  #[inline]
  fn previous(&self) -> &Token {
    &self.previous
  }

  #[inline]
  fn is_at_end(&self) -> bool {
    matches!(self.current.kind, TokenKind::Eof)
  }

  fn advance(&mut self) -> &Token {
    self.last_previous = self.previous.clone();
    self.previous = self.current.clone();

    loop {
      self.current = self.lexer.scan();
      if self.current.kind == TokenKind::None {
        continue;
      }

      if matches!(self.current.kind.clone(), TokenKind::Error(..),) {
        if let TokenKind::Error(message, line, col) = self.current.kind.clone() {
          self
            .errors
            .push(ParserError::new(message, self.current.clone()));
        }

        continue;
      }

      break;
    }

    &self.previous
  }

  fn rewind(&mut self) {
    self.current = self.previous.clone();
    self.previous = self.last_previous.clone();
    self.lexer.rewind();
  }

  fn match_token(&mut self, kind: TokenKind) -> bool {
    if check_tok!(self, kind) {
      self.advance();
      return true;
    }

    false
  }

  fn end_statement(&mut self) {
    if match_tok!(self, TokenKind::Eof)
      || self.is_at_end()
      || (self.block_count > 0 && check_tok!(self, TokenKind::Rbrace))
    {
      return;
    }

    if match_tok!(self, TokenKind::Semicolon) {
      while match_tok!(self, TokenKind::Newline | TokenKind::Semicolon) {}
      return;
    }

    consume_tok!(self, TokenKind::Newline, "End of statement expected");

    while match_tok!(
      self,
      TokenKind::Newline | TokenKind::Semicolon | TokenKind::None
    ) {}
  }

  fn ignore_newlines_only(&mut self) {
    while match_tok!(self, TokenKind::Newline) {}
  }

  fn ignore_newlines(&mut self) {
    while match_tok!(
      self,
      TokenKind::Newline | TokenKind::Semicolon | TokenKind::None
    ) {}
  }

  //-----------------------------------------------------------------------------------
  // Parts
  //-----------------------------------------------------------------------------------

  fn get_type_from_token(&mut self, token: Token) -> Type {
    if let TokenKind::Identifier(name) = token.kind.clone() {
      match name.as_str() {
        "any" => Type::Any,
        "bool" => Type::Bool,
        "int" => Type::Int,
        "number" => Type::Number,
        "string" => Type::String,
        "bytes" => Type::Bytes,
        "list" => Type::List,
        "dict" => Type::Dict,
        "range" => Type::Range,
        "file" => Type::File,
        "function" => Type::Function,
        "type" => Type::Type,
        "callable" => Type::Callable,
        "iterable" => Type::Iterable,
        _ => Type::Instance(token),
      }
    } else {
      Type::Any
    }
  }

  fn parse_type(&mut self, class_type: bool) -> Expr {
    let is_nullable = match_tok!(self, TokenKind::Question);

    let mut types = Vec::new();

    loop {
      let token = consume_tok!(self, TokenKind::Identifier(_), "Expected type name");

      types.push(self.get_type_from_token(token));

      if !match_tok!(self, TokenKind::Bar) {
        break;
      }
    }

    Expr::TypeHint(types, is_nullable)
  }

  fn parse_args(&mut self) -> Expr {
    let start = self.mark();

    let name = consume_tok!(self, TokenKind::Identifier(_), "Expected argument name");

    let type_hint = if match_tok!(self, TokenKind::Colon) {
      self.parse_type(false)
    } else {
      Expr::TypeHint(vec![Type::Any], true)
    };

    Expr::Argument(name, Box::new(type_hint))
  }

  //-----------------------------------------------------------------------------------
  // Composers
  //-----------------------------------------------------------------------------------

  fn compose_one_binary(&mut self, expr: Expr, kind: TokenKind, line: u32) -> Expr {
    let start = self.mark();
    let one = Expr::Integer(1);

    Expr::Binary(Box::new(expr), kind, Box::new(one), line)
  }

  fn compose_nil(&mut self) -> Expr {
    Expr::Nil
  }

  fn compose_bool(&mut self, val: bool) -> Expr {
    Expr::Bool(val)
  }

  fn compose_id(&mut self, id: Token) -> Expr {
    Expr::Identifier(id)
  }

  fn compose_get(&mut self, expr: Expr, id: Token) -> Expr {
    Expr::Get(Box::new(expr), id)
  }

  fn compose_assign(&mut self, expr: Expr, value: Expr) -> Expr {
    Expr::Assign(Box::new(expr), Box::new(value))
  }

  fn compose_call(&mut self, callee: Expr, args: Vec<Expr>, line: u32) -> Expr {
    Expr::Call(Box::new(callee), args, line)
  }

  fn compose_literal(&mut self, token: Token) -> Expr {
    if let TokenKind::Literal(v) = token.kind {
      Expr::Literal(v.clone())
    } else if let TokenKind::Identifier(v) = token.kind {
      Expr::Literal(v.clone())
    } else {
      Expr::Literal(token.to_string())
    }
  }

  //-----------------------------------------------------------------------------------
  // Expressions
  //-----------------------------------------------------------------------------------

  fn grouping(&mut self) -> Expr {
    self.ignore_newlines();
    let expr = self.expression();
    self.ignore_newlines();
    consume_tok!(self, TokenKind::Rparen, "Expected ')' after expression");

    Expr::Grouping(Box::new(expr))
  }

  fn finish_call(&mut self, callee: Expr) -> Expr {
    let line = self.previous().line as u32;
    let mut args = Vec::new();
    self.ignore_newlines();

    if !check_tok!(self, TokenKind::Rparen) {
      args.push(self.expression());

      while match_tok!(self, TokenKind::Comma) {
        self.ignore_newlines();

        if check_tok!(self, TokenKind::Rparen) {
          break;
        }

        args.push(self.expression());
      }
    }

    self.ignore_newlines();
    consume_tok!(self, TokenKind::Rparen, "Expected ')' after arguments");

    Expr::Call(Box::new(callee), args, line)
  }

  fn finish_index(&mut self, callee: Expr) -> Expr {
    let line = self.previous().line as u32;
    self.ignore_newlines();
    let mut expr = if !check_tok!(self, TokenKind::Comma) {
      self.expression()
    } else {
      Expr::Nil
    };

    if match_tok!(self, TokenKind::Comma) {
      self.ignore_newlines();
      let upper = if !check_tok!(self, TokenKind::Rbracket) {
        self.expression()
      } else {
        Expr::Nil
      };

      expr = Expr::Slice(Box::new(callee), Box::new(expr), Box::new(upper), line);
    } else {
      expr = Expr::Index(Box::new(callee), Box::new(expr), line);
    }

    self.ignore_newlines();
    consume_tok!(self, TokenKind::Rbracket, "Expected ']' after index");

    expr
  }

  fn finish_dot(&mut self, callee: Expr) -> Expr {
    let line = self.previous().line as u32;
    self.ignore_newlines();

    // `prop` is a real, independently addressable piece of syntax (the
    // property name after the dot), so — unlike most of the nodes below —
    // it deliberately gets its own checkpoint instead of sharing `start`.
    let prop_start = self.mark();
    let prop_token = consume_tok!(
      self,
      TokenKind::Identifier(_),
      "Expected property name after '.'"
    );

    if match_tok!(self, assignment_operators!()) {
      let token = self.previous().clone();

      if matches!(token.kind.clone(), TokenKind::Equal) {
        let value = self.expression();
        return Expr::Set(
          Box::new(callee.clone()),
          prop_token.clone(),
          Box::new(value),
        );
      }

      let get = Expr::Get(Box::new(callee.clone()), prop_token.clone());

      let rhs = self.assignment();
      let binary_value = Expr::Binary(
        Box::new(get),
        get_assignment_alt(token.kind),
        Box::new(rhs),
        line,
      );

      return Expr::Set(
        Box::new(callee.clone()),
        prop_token.clone(),
        Box::new(binary_value),
      );
    }

    Expr::Get(Box::new(callee.clone()), prop_token)
  }

  fn interpolation(&mut self) -> Expr {
    let mut expr = self.compose_literal(self.previous().clone());

    loop {
      let line = self.previous().line as u32;
      let mark = self.mark();
      let right = self.expression();
      expr = Expr::Binary(Box::new(expr), TokenKind::Plus, Box::new(right), line);

      if !check_tok!(self, TokenKind::Interpolation(_) | TokenKind::Literal(_)) || self.is_at_end()
      {
        break;
      }
    }

    expr
  }

  fn literal(&mut self) -> Expr {
    self.compose_literal(self.previous().clone())
  }

  fn identifier(&mut self) -> Expr {
    Expr::Identifier(self.previous().clone())
  }

  fn primary(&mut self) -> Expr {
    let start = self.mark();

    // Move unto the primary token itself
    let prev = self.advance().clone();

    match prev.kind {
      TokenKind::False => Expr::Bool(false),
      TokenKind::True => Expr::Bool(true),
      TokenKind::Nil => Expr::Nil,
      TokenKind::Self_ => Expr::Self_,
      TokenKind::Parent => Expr::Parent,
      TokenKind::Double(v) => Expr::Float(v),
      TokenKind::Integer(v) => Expr::Integer(v),
      TokenKind::BinNumber(v) => Expr::Integer(v),
      TokenKind::OctNumber(v) => Expr::Integer(v),
      TokenKind::HexNumber(v) => Expr::Integer(v),
      TokenKind::BigNumber(ref v) => Expr::BigNumber(v.clone()),
      TokenKind::Literal(_) => self.literal(),
      TokenKind::Identifier(_) => self.identifier(),
      TokenKind::Interpolation(_) => self.interpolation(),
      TokenKind::Lparen => self.grouping(),
      TokenKind::Lbrace => self.dict(),
      TokenKind::Lbracket => self.list(),
      TokenKind::At => self.anonymous(),
      _ => {
        self.report_error(format!("Unexpected token {:?}", prev.describe()));
        self.literal()
      },
    }
  }

  // NOTE ON every chain function below (range, factor, term, shift,
  // bit_and, bit_xor, bit_or, comparison, equality, and, or) takes a FRESH
  // checkpoint for each Binary/Range node it builds — right after
  // ignore_newlines(), i.e. anchored to that node's own right-hand operand —
  // rather than sharing one checkpoint across the whole chain. Newlines are
  // legal (and ignored) between an operator and its right-hand side per the
  // grammar, so a chain can legitimately span many source lines; sharing a
  // single position across all of them would misreport every error on a
  // multi-line chain's tail as happening at its head.

  fn range(&mut self) -> Expr {
    let mut expr = self.primary();

    while match_tok!(self, TokenKind::Range) {
      let line = self.previous().line as u32;
      self.ignore_newlines();
      let upper = self.primary();
      expr = Expr::Range(Box::new(expr), Box::new(upper), line);
    }

    expr
  }

  fn do_call(&mut self, callee: &mut Expr) -> Expr {
    let mut callee = callee.clone();

    loop {
      if match_tok!(self, TokenKind::Dot) {
        callee = self.finish_dot(callee);
      } else if match_tok!(self, TokenKind::Lparen) {
        callee = self.finish_call(callee);
      } else if match_tok!(self, TokenKind::Lbracket) {
        callee = self.finish_index(callee);
      } else if matches!(self.peek().clone().kind, TokenKind::Newline)
        && matches!(self.peek_next().clone().kind, TokenKind::Dot)
      {
        self.advance();
      } else {
        break;
      }
    }

    callee
  }

  fn call(&mut self) -> Expr {
    let mut expr = self.range();
    self.do_call(&mut expr)
  }

  fn assign_expr(&mut self) -> Expr {
    let start = self.mark();
    let expr = self.call();

    if match_tok!(self, TokenKind::Increment) {
      let line = self.previous().line as u32;
      let plus_one = Box::new(self.compose_one_binary(expr.clone(), TokenKind::Plus, line));

      return match expr {
        Expr::Get(expression, name) => Expr::Set(expression, name, plus_one),
        _ => Expr::Assign(Box::new(expr), plus_one),
      };
    }

    if match_tok!(self, TokenKind::Decrement) {
      let line = self.previous().line as u32;
      let sub_one = Box::new(self.compose_one_binary(expr.clone(), TokenKind::Minus, line));

      return match expr {
        Expr::Get(expression, name) => Expr::Set(expression, name, sub_one),
        _ => Expr::Assign(Box::new(expr), sub_one),
      };
    }

    expr
  }

  fn unary(&mut self) -> Expr {
    if match_tok!(self, unary_operators!()) {
      let op = self.previous().clone().kind;
      let line = self.previous().line as u32;
      self.ignore_newlines();
      let right = self.unary();
      return Expr::Unary(op, Box::new(right), line);
    }

    self.assign_expr()
  }

  fn factor(&mut self) -> Expr {
    let mut expr = self.unary();

    while match_tok!(self, factor_operators!()) {
      let op = self.previous().clone().kind;
      let line = self.previous().line as u32;

      self.ignore_newlines();
      let right = self.unary();
      expr = Expr::Binary(Box::new(expr), op, Box::new(right), line);
    }

    expr
  }

  fn term(&mut self) -> Expr {
    let mut expr = self.factor();

    while match_tok!(self, term_operators!()) {
      let op = self.previous().clone().kind;
      let line = self.previous().line as u32;
      self.ignore_newlines();
      let right = self.factor();
      expr = Expr::Binary(Box::new(expr), op, Box::new(right), line);
    }

    expr
  }

  fn shift(&mut self) -> Expr {
    let mut expr = self.term();

    while match_tok!(self, shift_operators!()) {
      let op = self.previous().clone().kind;
      let line = self.previous().line as u32;
      self.ignore_newlines();
      let right = self.term();
      expr = Expr::Binary(Box::new(expr), op, Box::new(right), line);
    }

    expr
  }

  fn bit_and(&mut self) -> Expr {
    let mut expr = self.shift();

    while match_tok!(self, TokenKind::Amp) {
      let op = self.previous().clone().kind;
      let line = self.previous().line as u32;
      self.ignore_newlines();
      let right = self.shift();
      expr = Expr::Binary(Box::new(expr), op, Box::new(right), line);
    }

    expr
  }

  fn bit_xor(&mut self) -> Expr {
    let mut expr = self.bit_and();

    while match_tok!(self, TokenKind::Xor) {
      let op = self.previous().clone().kind;
      let line = self.previous().line as u32;
      self.ignore_newlines();
      let right = self.bit_and();
      expr = Expr::Binary(Box::new(expr), op, Box::new(right), line);
    }

    expr
  }

  fn bit_or(&mut self) -> Expr {
    let mut expr = self.bit_xor();

    while match_tok!(self, TokenKind::Bar) {
      let op = self.previous().clone().kind;
      let line = self.previous().line as u32;
      self.ignore_newlines();
      let right = self.bit_xor();
      expr = Expr::Binary(Box::new(expr), op, Box::new(right), line);
    }

    expr
  }

  fn comparison(&mut self) -> Expr {
    let mut expr = self.bit_or();

    while match_tok!(self, comparison_operators!()) {
      let op = self.previous().clone().kind;
      let line = self.previous().line as u32;
      self.ignore_newlines();
      let right = self.bit_or();
      expr = Expr::Logical(Box::new(expr), op, Box::new(right), line);
    }

    expr
  }

  fn equality(&mut self) -> Expr {
    let mut expr = self.comparison();

    while match_tok!(self, equality_operators!()) {
      let op = self.previous().clone().kind;
      let line = self.previous().line as u32;
      self.ignore_newlines();
      let right = self.comparison();
      expr = Expr::Binary(Box::new(expr), op, Box::new(right), line);
    }

    expr
  }

  fn and(&mut self) -> Expr {
    let mut expr = self.equality();

    while match_tok!(self, TokenKind::And) {
      let op = self.previous().clone().kind;
      self.ignore_newlines();
      let right = self.equality();
      expr = Expr::Circuit(Box::new(expr), op, Box::new(right));
    }

    expr
  }

  fn or(&mut self) -> Expr {
    let mut expr = self.and();

    while match_tok!(self, TokenKind::Or) {
      let op = self.previous().clone().kind;
      self.ignore_newlines();
      let right = self.and();
      expr = Expr::Circuit(Box::new(expr), op, Box::new(right));
    }

    expr
  }

  fn conditional(&mut self) -> Expr {
    let mut expr = self.or();

    if match_tok!(self, TokenKind::Question) {
      self.ignore_newlines();
      let truth = self.conditional();

      consume_tok!(self, TokenKind::Colon, "Expected ':' in tenary operation.");
      self.ignore_newlines();

      let falsy = self.conditional();
      expr = Expr::Condition(Box::new(expr), Box::new(truth), Box::new(falsy));
    }

    expr
  }

  fn assignment(&mut self) -> Expr {
    let mut expr = self.conditional();

    if match_tok!(self, assignment_operators!()) {
      let type_token = self.previous().clone();
      self.ignore_newlines();

      if matches!(type_token.kind.clone(), TokenKind::Equal) {
        let value = self.assignment();
        expr = Expr::Assign(Box::new(expr), Box::new(value));
      } else {
        let right = self.assignment();
        let binary = Expr::Binary(
          Box::new(expr.clone()),
          get_assignment_alt(type_token.clone().kind),
          Box::new(right),
          type_token.line as u32,
        );
        expr = Expr::Assign(Box::new(expr), Box::new(binary));
      }
    }

    expr
  }

  fn expression(&mut self) -> Expr {
    self.assignment()
  }

  fn dict(&mut self) -> Expr {
    self.ignore_newlines();

    let mut keys = Vec::new();
    let mut values = Vec::new();

    if !check_tok!(self, TokenKind::Rbrace) {
      loop {
        self.ignore_newlines();

        if !check_tok!(self, TokenKind::Rbrace) {
          let key_mark = self.mark();
          let key = if match_tok!(self, TokenKind::Identifier(_)) {
            self.literal()
          } else {
            self.expression()
          };

          keys.push(key.clone());
          self.ignore_newlines();

          if !match_tok!(self, TokenKind::Colon) {
            let missing = "Missing value in dictionary definition".to_string();
            let token = self.previous.clone();

            match key {
              Expr::Literal(v) => {
                values.push(self.compose_id(token.copy_to(TokenKind::Identifier(v))))
              },
              _ => self.report_error(missing),
            };
          } else {
            self.ignore_newlines();
            values.push(self.expression());
          }

          self.ignore_newlines();
        } else {
          break;
        }

        if !match_tok!(self, TokenKind::Comma) {
          break;
        }
      }
    }

    if keys.len() != values.len() {
      self.report_error("Key/value count mismatch dictionary definition".to_string());
    }

    self.ignore_newlines();
    consume_tok!(self, TokenKind::Rbrace, "Expected '}' after dictionary");

    Expr::Dict(keys, values)
  }

  fn list(&mut self) -> Expr {
    self.ignore_newlines();

    let mut items = Vec::new();

    if !check_tok!(self, TokenKind::Rbracket) {
      loop {
        self.ignore_newlines();

        if !check_tok!(self, TokenKind::Rbracket) {
          items.push(self.expression());
          self.ignore_newlines();
        } else {
          break;
        }

        if !match_tok!(self, TokenKind::Comma) {
          break;
        }
      }
    }

    self.ignore_newlines();
    consume_tok!(self, TokenKind::Rbracket, "Expected ']' at end of list");

    Expr::List(items)
  }

  fn anonymous(&mut self) -> Expr {
    self.functions_count += 1;
    let start = self.mark();

    let name_token = self.previous().clone();

    let mut is_variadic = false;
    let mut parameters = Vec::new();

    if match_tok!(self, TokenKind::Lparen) {
      if !match_tok!(self, TokenKind::Rparen) {
        (parameters, is_variadic) = self.function_args();

        consume_tok!(
          self,
          TokenKind::Rparen,
          "Expected ')' after anonymous function arguments."
        );
      }
    }

    self.ignore_newlines();

    let body = if match_tok!(self, TokenKind::Arrow) {
      Stmt::Return(Box::new(self.expression()))
    } else {
      self.match_block("Expected '{' after function declaration".to_string())
    };

    let function = Decl::Function(
      name_token.copy_to(TokenKind::Identifier(format!(
        "@anon{}",
        self.anonymous_count
      ))),
      parameters,
      Box::new(body),
      is_variadic,
    );

    // Increment the count for the next guy
    self.anonymous_count += 1;

    self.functions_count -= 1;
    Expr::Anonymous(Box::new(function))
  }

  //-----------------------------------------------------------------------------------
  // Statements
  //-----------------------------------------------------------------------------------

  fn echo_stmt(&mut self) -> Stmt {
    let val = self.expression();
    self.end_statement();

    Stmt::Echo(Box::new(val))
  }

  fn expression_stmt(&mut self, is_iter: bool) -> Stmt {
    let val = self.expression();

    if !is_iter {
      self.end_statement();
    }

    Stmt::Expression(Box::new(val))
  }

  fn block(&mut self) -> Stmt {
    self.block_count += 1;

    let mut vals = Vec::new();
    self.ignore_newlines();

    while !check_tok!(self, TokenKind::Rbrace) && !self.is_at_end() {
      vals.push(self.statement());
    }

    consume_tok!(self, TokenKind::Rbrace, "Expected '}' at end of block.");
    self.block_count -= 1;

    Stmt::Block(vals)
  }

  fn match_block(&mut self, message: String) -> Stmt {
    self.ignore_newlines();
    consume_tok!(self, TokenKind::Lbrace, message.as_str());

    self.block()
  }

  fn if_stmt(&mut self) -> Stmt {
    let expr = self.expression();
    let body = self.statement();

    let else_branch = if match_tok!(self, TokenKind::Else) {
      Some(Box::new(self.statement()))
    } else {
      None
    };

    Stmt::If(Box::new(expr), Box::new(body), else_branch)
  }

  fn while_stmt(&mut self) -> Stmt {
    let condition = self.expression();
    let body = self.statement();

    Stmt::While(Box::new(condition), Box::new(body))
  }

  // do..while statement is desugared into a while statement.
  fn do_stmt(&mut self) -> Stmt {
    let body = self.statement();

    consume_tok!(self, TokenKind::While, "Expected 'while' after 'do' body.");
    let condition = self.expression();

    let mut final_body = Vec::new();

    final_body.push(body.clone());

    final_body.push(Stmt::While(Box::new(condition), Box::new(body)));

    Stmt::Block(final_body)
  }

  // NOTE ON everything this function builds is synthetic — desugared
  // sugar for `for k, v in iterable { ... }` that doesn't correspond to any
  // single token the user actually wrote. Every synthesized node here shares
  // one `start` (the position of the loop variable, right after `for`),
  // which is an honest span for "this code was generated by the for-loop
  // desugaring at this point".
  //
  // This is intentionally different from the
  // binary-chain functions those chain nodes are built directly from
  // real user-written operands that can span multiple lines, so each needs
  // its own accurate position. These for-loop nodes are entirely synthetic —
  // there's no user-written sub-expression for most of them to point at more
  // precisely than "the for statement". The one exception is the actual loop
  // body (`self.statement()`), which is real user code and keeps its own,
  // independently correct position.
  fn for_stmt(&mut self) -> Stmt {
    let key_id = consume_tok!(self, TokenKind::Identifier(_), "Variable name expected");
    let mut value_id = key_id.clone();

    // The variable that tracks the key/index ACROSS iterations. For a
    // single-variable loop (`for x in ...`) this must be the synthetic
    // `$key` -- invisible to user code -- because the user's own
    // variable holds only the VALUE and must not be clobbered by key
    // bookkeeping written back into it each iteration. For a
    // two-variable loop (`for k, v in ...`), the user's own first name
    // IS the key tracker -- exposing it as `k` is the whole point, so
    // it's swapped in below once we know we're in that form.
    let key_decl_name = key_id.copy_to(TokenKind::Identifier("$key".to_string()));
    let mut key_track_id = key_decl_name.clone();

    let key_nil = self.compose_nil();
    let mut key = Stmt::Var(key_decl_name, Box::new(key_nil), None, false);

    let value_decl_name = key_id.clone();
    let value_nil = self.compose_nil();
    let mut value = Stmt::Var(value_decl_name, Box::new(value_nil), None, false);

    if match_tok!(self, TokenKind::Comma) {
      key = value;
      key_track_id = key_id.clone();

      value_id = consume_tok!(self, TokenKind::Identifier(_), "Variable name expected");

      let value_decl_name2 = value_id.clone();
      let value_nil2 = self.compose_nil();
      value = Stmt::Var(value_decl_name2, Box::new(value_nil2), None, false);
    }

    consume_tok!(self, TokenKind::In, "Expected 'in' after 'for' statement.");

    let iterable = self.expression();

    let mut stmt_list = Vec::new();

    // key = object.@key(key)
    {
      let get_name = key_track_id.copy_to(TokenKind::Identifier("@key".to_string()));
      let getter = self.compose_get(iterable.clone(), get_name);
      let call_arg = self.compose_id(key_track_id.clone());
      let call = self.compose_call(getter, vec![call_arg], key_track_id.line as u32);
      let lhs = self.compose_id(key_track_id.clone());
      let assign = self.compose_assign(lhs, call);
      stmt_list.push(Stmt::Expression(Box::new(assign)));
    }

    // if key == nil { break }
    {
      let left = self.compose_id(key_track_id.clone());
      let right = self.compose_nil();
      let condition = Expr::Logical(
        Box::new(left),
        TokenKind::EqualEq,
        Box::new(right),
        key_track_id.line as u32,
      );
      let then_branch = Stmt::Break;
      stmt_list.push(Stmt::If(Box::new(condition), Box::new(then_branch), None));
    }

    // value = object.@value(key)
    {
      let get_name = value_id.copy_to(TokenKind::Identifier("@value".to_string()));
      let getter = self.compose_get(iterable.clone(), get_name);
      let call_arg = self.compose_id(key_track_id.clone());
      let call = self.compose_call(getter, vec![call_arg], value_id.line as u32);
      let lhs = self.compose_id(value_id.clone());
      let assign = self.compose_assign(lhs, call);
      stmt_list.push(Stmt::Expression(Box::new(assign)));
    }

    // parse the loop body — real user code, keeps its own position
    stmt_list.push(self.statement());

    let cond = self.compose_bool(true);
    let block = Stmt::Block(stmt_list);
    let body = Stmt::While(Box::new(cond), Box::new(block));

    Stmt::Block(vec![key, value, body])
  }

  fn assert_stmt(&mut self) -> Stmt {
    let expr = self.expression();
    let mut message = None;

    if match_tok!(self, TokenKind::Comma) {
      message = Some(Box::new(self.expression()));
    }

    Stmt::Assert(Box::new(expr), message)
  }

  fn using_stmt(&mut self) -> Stmt {
    let expr = self.expression();
    let mut case_labels = Vec::new();
    let mut case_bodies = Vec::new();
    let mut default_case = None;

    consume_tok!(
      self,
      TokenKind::Lbrace,
      "Expected '{' after 'using' statement."
    );
    self.ignore_newlines();

    let mut state = 0;

    while !check_tok!(self, TokenKind::Rbrace) && !self.is_at_end() {
      if match_tok!(
        self,
        TokenKind::When | TokenKind::Default | TokenKind::Newline
      ) {
        if state == 1 {
          self.report_error(
            "'when' or 'default' state cannot exist after a default state".to_string(),
          );
        }

        let prev = self.previous().clone();
        match prev.kind {
          TokenKind::When => {
            let mut tmp_cases = Vec::new();

            loop {
              self.ignore_newlines();
              tmp_cases.push(self.expression());

              if !check_tok!(self, TokenKind::Comma) || check_tok!(self, TokenKind::Lbrace) {
                break;
              }

              consume_tok!(self, TokenKind::Comma, "Expected ',' but found none.");
            }

            let stmt = self.statement();

            for case in tmp_cases {
              case_labels.push(case);
              case_bodies.push(stmt.clone());
            }
          },
          TokenKind::Default => {
            state = 1;
            default_case = Some(Box::new(self.statement()));
          },
          TokenKind::Newline => {}, // Do nothing!
          _ => {
            self.report_error("Invalid using statement".to_string());
          },
        };
      } else {
        self.report_error("Invalid using statement".to_string());
        break;
      }
    }

    consume_tok!(
      self,
      TokenKind::Rbrace,
      "Expected '}' at end of using statement."
    );

    Stmt::Using(Box::new(expr), case_labels, case_bodies, default_case)
  }

  fn import_stmt(&mut self) -> Stmt {
    let mut paths = Vec::new();
    let mut elements = Vec::new();
    let mut name = self.compose_nil();
    let sep = std::path::MAIN_SEPARATOR_STR;

    let mut name_is_nil = true;

    // range can only exist at the beginning of import path and
    // nowhere else within it
    if match_tok!(self, TokenKind::Range) {
      paths.push("..".to_string());
    }

    while match_tok!(self, TokenKind::Dot | TokenKind::Identifier(_)) {
      let token = self.previous().clone();

      if matches!(token.kind.clone(), TokenKind::Dot) {
        paths.push(".".to_string());
      } else if let TokenKind::Identifier(name) = token.kind.clone() {
        paths.push(name.clone());
      }
    }

    let mut imports_all = false;

    if match_tok!(self, TokenKind::Lbrace) {
      let mut scan = true;

      while !check_tok!(self, TokenKind::Rbrace) && scan {
        self.ignore_newlines();

        let element = consume_tok!(
          self,
          TokenKind::Identifier(_) | TokenKind::Multiply,
          "Expected identifier or '*' after import statement."
        );

        if matches!(element.kind.clone(), TokenKind::Multiply) {
          // We're required to import all
          if !elements.is_empty() {
            self.report_error(
              "Cannot import selected items and everything from the same import statement."
                .to_string(),
            );
          }

          imports_all = true;
          break;
        } else if let TokenKind::Identifier(name) = element.kind.clone()
          && name.starts_with("_")
        {
          self.report_error("Cannot import private items from module".to_string());
          break;
        }

        elements.push(self.compose_id(element));
        if !match_tok!(self, TokenKind::Comma) {
          scan = false;
        }

        self.ignore_newlines();
      }

      consume_tok!(
        self,
        TokenKind::Rbrace,
        "Expected '}' after import statement."
      );
    } else if match_tok!(self, TokenKind::As) {
      let token = consume_tok!(
        self,
        TokenKind::Identifier(_),
        "Expected identifier after 'as' keyword."
      );

      name = self.compose_id(token);
      name_is_nil = false;
    }

    let final_path = paths.join(sep);

    if name_is_nil {
      let synthesized = self
        .previous()
        .copy_to(TokenKind::Literal(paths.last().unwrap().clone()));
      name = self.compose_id(synthesized);
    }

    Stmt::Import(final_path, Box::new(name), elements, imports_all)
  }

  fn catch_stmt(&mut self) -> Stmt {
    let mut body = self.match_block("Expected '{' after 'catch' statement.".to_string());
    let mut name = None;
    let mut catch_body = None;

    if match_tok!(self, TokenKind::As) {
      let id = consume_tok!(
        self,
        TokenKind::Identifier(_),
        "Error variable name expected after 'as'."
      );

      name = Some(Box::new(self.compose_id(id)));

      if check_tok!(self, TokenKind::Lbrace) {
        catch_body = Some(Box::new(
          self.match_block("Expected '{' after exception variable.".to_string()),
        ));
      }
    }

    Stmt::Catch(Box::new(body), catch_body, name)
  }

  // Like for statement and do..while statement, iter statement is also desugared
  // into while statement
  fn iter_stmt(&mut self) -> Stmt {
    if match_tok!(self, TokenKind::Lparen) {
      // Do nothing
    }

    let mut final_stmts = Vec::new();

    let mut top_stmt = if !match_tok!(self, TokenKind::Semicolon) {
      if match_tok!(self, TokenKind::Var) {
        // do nothing...
      }

      let res = Some(self.var_decl(false));

      consume_tok!(
        self,
        TokenKind::Semicolon,
        "Expected ';' after iter initializer."
      );
      self.ignore_newlines();

      res
    } else {
      None
    };

    let mut condition = if !match_tok!(self, TokenKind::Semicolon) {
      let res = self.expression();

      consume_tok!(
        self,
        TokenKind::Semicolon,
        "Expected ';' after iter condition."
      );
      self.ignore_newlines();

      res
    } else {
      self.compose_bool(true)
    };

    if !check_tok!(self, TokenKind::Lbrace | TokenKind::Rparen) {
      loop {
        final_stmts.push(self.expression_stmt(true));
        self.ignore_newlines();

        if !check_tok!(self, TokenKind::Comma) {
          break;
        }
      }
    }

    if match_tok!(self, TokenKind::Rparen) {
      // Do nothing
    }

    let body = self.match_block("Expected '{' at the start of iter body.".to_string());

    // Add the iterator to the end of the body to make a larger block
    let mut block_body = Vec::new();
    block_body.push(body);

    block_body.push(Stmt::FixContinue);
    for stmt in final_stmts {
      block_body.push(stmt);
    }

    let block_body_stmt = Stmt::Block(block_body);

    let mut final_body = Vec::new();

    // If top_stmt is Some, then push it to final_body
    if let Some(stmt) = top_stmt {
      final_body.push(stmt);
    }

    final_body.push(Stmt::While(Box::new(condition), Box::new(block_body_stmt)));

    Stmt::Block(final_body)
  }

  fn statement(&mut self) -> Stmt {
    self.ignore_newlines();

    let start = self.mark();

    let result = match self.advance().kind {
      TokenKind::As => {
        self.report_error("'as' is only a valid keyword in catch context".to_string());
        Stmt::None
      },
      TokenKind::Var => {
        let result = self.var_decl(false);
        self.end_statement();
        result
      },
      TokenKind::Const => {
        let result = self.var_decl(true);
        self.end_statement();
        result
      },
      TokenKind::Const => {
        let result = self.var_decl(true);
        self.end_statement();
        result
      },
      TokenKind::Echo => self.echo_stmt(),
      TokenKind::If => self.if_stmt(),
      TokenKind::While => self.while_stmt(),
      TokenKind::Do => self.do_stmt(),
      TokenKind::Iter => self.iter_stmt(),
      TokenKind::For => self.for_stmt(),
      TokenKind::Using => self.using_stmt(),
      TokenKind::Assert => self.assert_stmt(),
      TokenKind::Lbrace => self.block(),
      TokenKind::Import => self.import_stmt(),
      TokenKind::Catch => self.catch_stmt(),
      TokenKind::Def => Stmt::Decl(Box::new(self.function_decl())),
      TokenKind::Continue => Stmt::Continue,
      TokenKind::Break => Stmt::Break,
      TokenKind::Return => {
        if self.functions_count == 0 {
          self.report_error("'return' is only a valid keyword in function context".to_string());
        }

        let mut result = self.compose_nil();

        if !self.is_at_end()
          && !check_tok!(
            self,
            TokenKind::Newline | TokenKind::Semicolon | TokenKind::Rbrace
          )
        {
          result = self.expression();
        }

        Stmt::Return(Box::new(result))
      },
      TokenKind::Raise => {
        let mut result = self.compose_nil();

        if !self.is_at_end()
          && !check_tok!(
            self,
            TokenKind::Newline | TokenKind::Semicolon | TokenKind::Rbrace
          )
        {
          result = self.expression();
        }

        Stmt::Raise(Box::new(result))
      },
      TokenKind::Lbrace => self.block(),
      _ => {
        self.rewind();
        self.expression_stmt(false)
      },
    };

    self.ignore_newlines();

    result
  }

  //-----------------------------------------------------------------------------------
  // Declarations
  //-----------------------------------------------------------------------------------

  fn var_decl(&mut self, is_constant: bool) -> Stmt {
    let mut declarations = Vec::new();

    loop {
      let name_token = consume_tok!(self, TokenKind::Identifier(_), "Variable name expected.");

      let type_hint = if match_tok!(self, TokenKind::Colon) {
        // type hinting
        Some(Box::new(self.parse_type(false)))
      } else {
        None
      };

      let declaration = if match_tok!(self, TokenKind::Equal) {
        Stmt::Var(
          name_token.clone(),
          Box::new(self.expression()),
          type_hint,
          is_constant,
        )
      } else {
        if is_constant {
          self.report_error("Constant value not declared".to_string());
          Stmt::None
        } else {
          Stmt::Var(
            name_token.clone(),
            Box::new(self.compose_nil()),
            type_hint,
            false,
          )
        }
      };

      if !match_tok!(self, TokenKind::Comma) {
        if declarations.is_empty() {
          return declaration;
        } else {
          declarations.push(declaration);
        }

        break;
      } else {
        declarations.push(declaration);
        self.ignore_newlines();
      }
    }

    Stmt::VarList(declarations)
  }

  fn function_args(&mut self) -> (Vec<Expr>, bool) {
    self.ignore_newlines();

    let mut params = Vec::new();
    let mut variadic = false;

    while check_tok!(self, TokenKind::Identifier(_) | TokenKind::TriDot) {
      let token = self.peek().clone();

      if match_tok!(self, TokenKind::TriDot) {
        variadic = true;

        let id = consume_tok!(
          self,
          TokenKind::Identifier(_),
          "Variable parameter name expected."
        );

        params.push(Expr::Argument(
          id,
          Box::new(Expr::TypeHint(vec![Type::List], false)),
        ));
        break;
      }

      params.push(self.parse_args());

      if !check_tok!(self, TokenKind::Rparen) {
        consume_tok!(
          self,
          TokenKind::Comma,
          "Expected ',' between function arguments."
        );
        self.ignore_newlines();
      }
    }

    return (params, variadic);
  }

  fn function_decl(&mut self) -> Decl {
    self.functions_count += 1;
    let name = consume_tok!(self, TokenKind::Identifier(_), "Function name expected.");

    consume_tok!(self, TokenKind::Lparen, "Expected '(' after function name.");
    self.ignore_newlines();

    let (parameters, is_variadic) = self.function_args();

    self.ignore_newlines();
    consume_tok!(
      self,
      TokenKind::Rparen,
      "Expected ')' after function arguments."
    );

    self.ignore_newlines();

    let body = self.match_block("Expected '{' after function declaration".to_string());

    self.functions_count -= 1;
    Decl::Function(name, parameters, Box::new(body), is_variadic)
  }

  fn class_field(&mut self, is_static: bool, is_constant: bool) -> Decl {
    let name = consume_tok!(self, TokenKind::Identifier(_), "Class field name expected.");

    let type_hint = if match_tok!(self, TokenKind::Colon) {
      Box::new(self.parse_type(false))
    } else {
      Box::new(Expr::TypeHint(vec![Type::List], false))
    };

    let value = if match_tok!(self, TokenKind::Equal) {
      Box::new(self.expression())
    } else {
      Box::new(self.compose_nil())
    };

    self.end_statement();
    self.ignore_newlines();

    Decl::Property(name, value, type_hint, is_static, is_constant)
  }

  fn method_decl(&mut self, is_static: bool) -> Decl {
    self.functions_count += 1;
    let name = consume_tok!(
      self,
      TokenKind::Identifier(_) | TokenKind::Decorator(_),
      "Method name expected."
    );

    consume_tok!(self, TokenKind::Lparen, "Expected '(' after method name.");
    self.ignore_newlines();

    let (parameters, is_variadic) = self.function_args();

    self.ignore_newlines();
    consume_tok!(
      self,
      TokenKind::Rparen,
      "Expected ')' after method arguments."
    );

    self.ignore_newlines();

    let body = self.match_block("Expected '{' after method declaration".to_string());

    self.functions_count -= 1;
    Decl::Method(name, parameters, Box::new(body), is_variadic, is_static)
  }

  fn class_decl(&mut self) -> Decl {
    let name = consume_tok!(self, TokenKind::Identifier(_), "Class name expected.");

    let mut properties = Vec::new();
    let mut methods = Vec::new();
    let mut is_extension = false;

    let superclass = if match_tok!(self, TokenKind::Less) {
      let target_class_name =
        consume_tok!(self, TokenKind::Identifier(_), "Superclass name expected.");

      Some(Box::new(self.compose_id(target_class_name)))
    } else {
      if match_tok!(self, TokenKind::Greater) {
        is_extension = true;

        let target_class_name = consume_tok!(
          self,
          TokenKind::Identifier(_),
          "Target class name expected."
        );

        Some(Box::new(self.compose_id(target_class_name)))
      } else {
        None
      }
    };

    self.ignore_newlines();
    consume_tok!(
      self,
      TokenKind::Lbrace,
      "Expected '{' after class declaration."
    );
    self.ignore_newlines();

    while !check_tok!(self, TokenKind::Rbrace) && !self.is_at_end() {
      self.ignore_newlines();

      let is_static = match_tok!(self, TokenKind::Static);

      if match_tok!(self, TokenKind::Var) {
        properties.push(self.class_field(is_static, false));
      } else if match_tok!(self, TokenKind::Const) {
        properties.push(self.class_field(is_static, true));
      } else {
        if match_tok!(self, TokenKind::Def) {
          // Do nothing. It changes nothing about our program
        }

        methods.push(self.method_decl(is_static));
      }

      self.ignore_newlines();
    }

    consume_tok!(
      self,
      TokenKind::Rbrace,
      "Expected '}' after class declaration."
    );

    Decl::Class(name, superclass, properties, methods, is_extension)
  }

  fn declaration(&mut self) -> Decl {
    self.ignore_newlines();

    let result = match self.advance().kind {
      TokenKind::Def => self.function_decl(),
      TokenKind::Class => self.class_decl(),
      TokenKind::Lbrace => {
        if !check_tok!(self, TokenKind::Newline) && self.block_count == 0 {
          let start = self.mark();
          let mut dict = self.dict();

          Decl::Stmt(Box::new(Stmt::Expression(Box::new(
            self.do_call(&mut dict),
          ))))
        } else {
          Decl::Stmt(Box::new(self.block()))
        }
      },
      _ => {
        self.rewind();
        Decl::Stmt(Box::new(self.statement()))
      },
    };

    self.ignore_newlines();

    result
  }

  pub fn parse(&mut self) -> Result<Vec<Decl>, Vec<ParserError>> {
    let mut result = Vec::new();

    while !self.is_at_end() {
      result.push(self.declaration());
    }

    if self.errors.is_empty() {
      Ok(result)
    } else {
      Err(self.errors.clone())
    }
  }
}
