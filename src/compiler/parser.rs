use std::fmt::{self, Display};
use std::io::IsTerminal;

use nu_ansi_term::{Color, Style};

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
}

impl ParserError {
  pub fn new(message: String, token: Token) -> Self {
    Self {
      message,
      line_number: token.line,
      offset: token.column,
    }
  }
}

// Compact one-liner, no source snippet; used when there's no source
// text handy to render one (or nowhere better to put a `Display` impl).
// The CLI's real rendering is `render`, below, which shows the offending
// line with a caret under the token instead of spelling the position out
// in prose.
impl Display for ParserError {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    write!(
      f,
      "SyntaxError: {} ({}:{})",
      self.message, self.line_number, self.offset
    )
  }
}

impl ParserError {
  /// The full developer-facing diagnostic: a `rustc`-style header, an
  /// `--> path:line:col` locator, and (when the source is available) the
  /// offending line itself with a caret under where the token starts.
  /// Just the start, not a full underline: a token's *describe()* text
  /// doesn't always match its real source width (an `Eof` describes as
  /// `<eof>`, a string literal's content excludes its quotes, ...), so a
  /// single caret is the only thing guaranteed to land in the right
  /// place for every token kind.
  // Same palette as `VM::format_uncaught`'s runtime-error rendering
  // (bold red header, cyan locator, dimmed gutter, bold red pointer) so
  // a syntax error and an uncaught error read as the same family of
  // diagnostic instead of two different tools' output pasted together.
  pub fn render(&self, path: &str, source: &str) -> String {
    let use_color = std::io::stderr().is_terminal();
    let err_style = if use_color {
      Style::new().fg(Color::Red).bold()
    } else {
      Style::new()
    };
    let locator_style = if use_color {
      Style::new().fg(Color::Cyan)
    } else {
      Style::new()
    };
    let dim_style = if use_color {
      Style::new().dimmed()
    } else {
      Style::new()
    };
    let marker_style = if use_color {
      Style::new().fg(Color::Red).bold()
    } else {
      Style::new()
    };

    let mut lines = vec![
      format!(
        "{}",
        err_style.paint(format!("SyntaxError: {}", self.message))
      ),
      format!(
        "  {} {}:{}:{}",
        locator_style.paint("-->"),
        path,
        self.line_number,
        self.offset
      ),
    ];

    if let Some(line_text) = source.lines().nth(self.line_number.saturating_sub(1)) {
      let gutter = self.line_number.to_string();
      let pad = " ".repeat(gutter.len());
      let caret_indent = " ".repeat(self.offset.saturating_sub(1));

      lines.push(format!("{}", dim_style.paint(format!("{} |", pad))));
      lines.push(format!(
        "{} {}",
        dim_style.paint(format!("{} |", gutter)),
        line_text
      ));
      lines.push(format!(
        "{} {}{}",
        dim_style.paint(format!("{} |", pad)),
        caret_indent,
        marker_style.paint("^")
      ));
    }

    lines.join("\n")
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
  lexer: &'a mut Lexer,
  block_count: usize,
  current: Token,
  previous: Token,
  last_previous: Token,
  anonymous_count: usize,
  functions_count: usize,
  pub errors: Vec<ParserError>,
  // Tokens already pulled out of the lexer by `peek_at` but not yet
  // consumed by `advance`; a real FIFO, unlike the old scan-then-
  // rewind-the-lexer trick, so it can look arbitrarily far past a run of
  // newlines (blank lines, comment-only lines, any mix of both) instead
  // of exactly one token ahead.
  lookahead: std::collections::VecDeque<Token>,
  // Comment/DocBlock tokens `scan_real_token` pulled off the lexer but
  // kept out of the grammar's sight, waiting to be turned into `Decl::
  // Trivia`/`Stmt::Trivia` siblings by whichever list-building loop
  // (`parse`, `block`, `class_decl`) next reaches a "between items" point.
  // Always empty once fully drained; see `drain_trivia_into_decls`/
  // `drain_trivia_into_stmts`.
  pending_trivia: Vec<Token>,
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
  pub fn new(lexer: &'a mut Lexer) -> Self {
    Self {
      lexer: lexer,
      block_count: 0,
      current: Token {
        kind: TokenKind::None,
        line: 0,
        column: 0,
        start: 0,
        end: 0,
      },
      previous: Token {
        kind: TokenKind::None,
        line: 0,
        column: 0,
        start: 0,
        end: 0,
      },
      last_previous: Token {
        kind: TokenKind::None,
        line: 0,
        column: 0,
        start: 0,
        end: 0,
      },
      anonymous_count: 0,
      functions_count: 0,
      errors: Vec::new(),
      lookahead: std::collections::VecDeque::new(),
      pending_trivia: Vec::new(),
    }
  }

  // Utility

  // Anchors to `current`; the right choice when the message is about
  // whatever comes next (an unmet expectation, a token that shouldn't be
  // here). When the message is instead about a token already consumed
  // (a keyword that's invalid in this context, an operator whose target
  // was bad), use `report_error_at` with `self.previous()` explicitly --
  // `current` has already moved past it by the time the check runs.
  fn report_error(&mut self, message: String) {
    let token = self.peek().clone();
    self.report_error_at(message, token);
  }

  fn report_error_at(&mut self, message: String, token: Token) {
    self.errors.push(ParserError::new(message, token));
  }

  fn mark(&self) -> Checkpoint {
    // Anchor to `current`'s own (already-correct) position rather than the
    // lexer's raw cursor. `peek_at` scans ahead of `current` to fill
    // `lookahead` and never rewinds that scan, so the lexer's cursor can
    // sit well past `current` by the time this is called; `current.line`/
    // `.column` are what's actually being pointed at.
    Checkpoint {
      line: self.current.line,
      col: self.current.column,
    }
  }

  // Thin sugar over mark()/finish() for a single leaf expression. Only one
  // mutable borrow of `self` is ever in play (the closure's own param), so
  // this doesn't fight the borrow checker the way a wider closure would.
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

  // 1-indexed lookahead: `peek_at(1)` is the token right after `current`,
  // `peek_at(2)` the one after that, etc. Fills `lookahead` on demand by
  // scanning ahead (skipping the same trivia `advance` would), so unlike
  // the old single-hop peek this can see past any number of tokens without
  // disturbing `current`/`previous`. Note it does *not* leave the lexer's
  // own cursor where it found it; the scan-ahead is one-way, which is
  // exactly why `mark()` reads position off `current` and not the lexer.
  fn peek_at(&mut self, n: usize) -> Token {
    debug_assert!(n >= 1, "peek_at is 1-indexed; peek_at(0) is not current");
    while self.lookahead.len() < n {
      let tok = self.scan_real_token();
      self.lookahead.push_back(tok);
    }
    self.lookahead[n - 1].clone()
  }

  #[inline]
  fn previous(&self) -> &Token {
    &self.previous
  }

  #[inline]
  fn is_at_end(&self) -> bool {
    matches!(self.current.kind, TokenKind::Eof)
  }

  // Pulls one grammar-visible token straight from the lexer, silently
  // skipping the priming sentinel (`None`) and reporting+skipping
  // lexer-level `Error` tokens as they're found. Comment/DocBlock tokens
  // are real tokens too, but the grammar itself never sees them; they're
  // stashed onto `pending_trivia` instead of being discarded, for the
  // `Decl::Trivia`/`Stmt::Trivia` nodes `parse`/`block`/`class_decl` build
  // from them (see `drain_trivia_into_decls`/`drain_trivia_into_stmts`).
  // Shared by `advance` (when there's nothing already queued) and
  // `peek_at` (to fill the queue), so both see identical trivia handling,
  // and since `peek_at`'s lookahead queue is a strict forward FIFO that's
  // never rewound at the lexer level, each comment in the source is
  // captured here exactly once no matter how far ahead the grammar peeks.
  fn scan_real_token(&mut self) -> Token {
    loop {
      let tok = self.lexer.scan();

      if tok.kind == TokenKind::None {
        continue;
      }

      if matches!(tok.kind, TokenKind::Comment(..) | TokenKind::DocBlock(..)) {
        self.pending_trivia.push(tok);
        continue;
      }

      if let TokenKind::Error(ref message, ..) = tok.kind {
        self
          .errors
          .push(ParserError::new(message.clone(), tok.clone()));
        continue;
      }

      return tok;
    }
  }

  // Turns every trivia token buffered since the last drain into a sibling
  // `Decl::Trivia`/`Stmt::Trivia`, in the order the tokens were scanned.
  // Called at each "between items" point in `parse`/`block`/`class_decl`
  // (never inside `declaration`/`statement` themselves, whose own leading
  // `ignore_newlines` would otherwise swallow trivia before the item it
  // precedes has even started being built). See the doc comments on
  // those three call sites for why each drain point is where it is.
  fn drain_trivia_into_decls(&mut self, out: &mut Vec<Decl>) {
    for tok in self.pending_trivia.drain(..) {
      out.push(Decl::Trivia(tok));
    }
  }

  fn drain_trivia_into_stmts(&mut self, out: &mut Vec<Stmt>) {
    for tok in self.pending_trivia.drain(..) {
      out.push(Stmt::Trivia(tok));
    }
  }

  fn advance(&mut self) -> &Token {
    self.last_previous = self.previous.clone();
    self.previous = self.current.clone();
    self.current = match self.lookahead.pop_front() {
      Some(tok) => tok,
      None => self.scan_real_token(),
    };

    &self.previous
  }

  // Puts `current` back and un-does the last `advance`, so a token
  // consumed on spec (e.g. "is this keyword actually a keyword here?")
  // can be handed back for `statement`/`declaration` to reparse as a
  // plain expression. Pushes onto the front of `lookahead` rather than
  // asking the lexer to rewind its own cursor; the lexer only ever
  // moves forward now, so this works regardless of whether `current` came
  // from a fresh scan or was already sitting in the lookahead queue (the
  // old cursor-based rewind assumed the former and could desync from a
  // queued token).
  fn rewind(&mut self) {
    self.lookahead.push_front(self.current.clone());
    self.current = self.previous.clone();
    self.previous = self.last_previous.clone();
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

  // A binary/logical operator already swallows the newline that follows
  // it, so `x +\n  y` and `x and\n  y` work. This is the other half: if
  // the operator instead opens the *next* line (`x\n  + y`), the newline
  // sits before it, where the operator-loop's `match_tok!` can't see past
  // it. Look past every `Newline` in a row; a blank line is just two of
  // them, and a comment-only line is a `Comment` sandwiched between two,
  // which `peek_at` already skips over since comments are trivia to the
  // grammar; and, if a continuation token is waiting past all of them,
  // eat the newlines so the caller's own `match_tok!` finds the operator
  // right where it left off.
  fn skip_newline_before(&mut self, is_continuation: impl Fn(&TokenKind) -> bool) {
    if !matches!(self.current.kind, TokenKind::Newline) {
      return;
    }

    let mut ahead = 1;
    while matches!(self.peek_at(ahead).kind, TokenKind::Newline) {
      ahead += 1;
    }

    if !is_continuation(&self.peek_at(ahead).kind) {
      return;
    }

    for _ in 0..ahead {
      self.advance();
    }
  }

  // Parts

  fn get_type_from_token(&mut self, token: Token) -> Type {
    if let TokenKind::Identifier(name) = token.kind.clone() {
      match name.as_str() {
        "any" => Type::Any,
        "bool" => Type::Bool,
        "int" => Type::Int,
        "number" => Type::Number,
        "bigint" => Type::BigInt,
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

  fn parse_type(&mut self) -> Expr {
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
    let name = consume_tok!(self, TokenKind::Identifier(_), "Expected argument name");

    let type_hint = if match_tok!(self, TokenKind::Colon) {
      self.parse_type()
    } else {
      Expr::TypeHint(vec![Type::Any], true)
    };

    Expr::Argument(name, Box::new(type_hint))
  }

  // Composers

  fn compose_one_binary(&mut self, expr: Expr, kind: TokenKind, line: u32) -> Expr {
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

  // Expressions

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
      let right = self.expression();
      expr = Expr::Binary(Box::new(expr), TokenKind::Plus, Box::new(right), line);

      // The token right after a `${...}` expression is always one the
      // lexer produced for the STATIC text that follows it, never
      // something to hand to `self.expression()` (which climbs full
      // operator precedence and would happily keep going past the
      // string's own closing quote, e.g. swallowing a trailing `==
      // other` into what should have been a plain literal segment).
      // `Interpolation(_)` means more text then another `${...}`
      // follows, so the loop continues; `Literal(_)` is always the
      // final segment up to the closing quote, so it terminates the
      // loop right after being appended.
      if check_tok!(self, TokenKind::Interpolation(_)) {
        let segment = self.advance().clone();
        let seg_line = segment.line as u32;
        let literal = self.compose_literal(segment);
        expr = Expr::Binary(Box::new(expr), TokenKind::Plus, Box::new(literal), seg_line);
        continue;
      }

      if check_tok!(self, TokenKind::Literal(_)) {
        let segment = self.advance().clone();
        let seg_line = segment.line as u32;
        let literal = self.compose_literal(segment);
        expr = Expr::Binary(Box::new(expr), TokenKind::Plus, Box::new(literal), seg_line);
      }

      break;
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
      TokenKind::At | TokenKind::Def => self.anonymous(),
      _ => {
        self.report_error_at(
          format!("Unexpected token {:?}", prev.describe()),
          prev.clone(),
        );
        self.literal()
      },
    }
  }

  // Every chain function below (range, factor, term, shift, bit_and,
  // bit_xor, bit_or, comparison, equality, and, or) takes a FRESH checkpoint
  // for each Binary/Range node it builds — right after ignore_newlines(),
  // anchored to that node's own right-hand operand — rather than sharing
  // one checkpoint across the whole chain. Newlines are legal (and ignored)
  // between an operator and its right-hand side per the grammar, so a chain
  // can span many source lines; sharing a single position across all of
  // them would misreport every error on a multi-line chain's tail as
  // happening at its head.

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
      // A leading `.` on the next line continues the chain; look past
      // any run of newlines (blank lines, comment-only lines, or both)
      // for it before giving up.
      self.skip_newline_before(|k| matches!(k, TokenKind::Dot));

      if match_tok!(self, TokenKind::Dot) {
        callee = self.finish_dot(callee);
      } else if match_tok!(self, TokenKind::Lparen) {
        callee = self.finish_call(callee);
      } else if match_tok!(self, TokenKind::Lbracket) {
        callee = self.finish_index(callee);
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
    // Captured before `call()` runs so an "invalid assignment target"
    // error below can point at where the bad expression actually starts,
    // not wherever `++`/`--` happened to land the cursor.
    let start_token = self.peek().clone();
    let expr = self.call();

    if match_tok!(self, TokenKind::Increment) {
      let line = self.previous().line as u32;
      let plus_one = Box::new(self.compose_one_binary(expr.clone(), TokenKind::Plus, line));

      return match expr {
        Expr::Get(expression, name) => Expr::Set(expression, name, plus_one),
        Expr::Identifier(_) | Expr::Index(..) => Expr::Assign(Box::new(expr), plus_one),
        other => {
          self.report_error_at("invalid assignment target".to_string(), start_token);
          other
        },
      };
    }

    if match_tok!(self, TokenKind::Decrement) {
      let line = self.previous().line as u32;
      let sub_one = Box::new(self.compose_one_binary(expr.clone(), TokenKind::Minus, line));

      return match expr {
        Expr::Get(expression, name) => Expr::Set(expression, name, sub_one),
        Expr::Identifier(_) | Expr::Index(..) => Expr::Assign(Box::new(expr), sub_one),
        other => {
          self.report_error_at("invalid assignment target".to_string(), start_token);
          other
        },
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

    while {
      self.skip_newline_before(|k| matches!(k, factor_operators!()));
      match_tok!(self, factor_operators!())
    } {
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

    while {
      // Unlike every other operator handled by `skip_newline_before`,
      // `-` doubles as a unary prefix (`unary_operators!()`), so a `-` at
      // the start of the next line is genuinely ambiguous with a brand
      // new statement that happens to start with a negation (`x\n-y` as
      // two statements vs. `x - y` as one). Only `+` has no such reading,
      // so only `+` gets to auto-continue; a leading `-` still requires
      // the trailing-operator style (`x -\n  y`) to be unambiguous.
      self.skip_newline_before(|k| matches!(k, TokenKind::Plus));
      match_tok!(self, term_operators!())
    } {
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

    while {
      self.skip_newline_before(|k| matches!(k, shift_operators!()));
      match_tok!(self, shift_operators!())
    } {
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

    while {
      self.skip_newline_before(|k| matches!(k, TokenKind::Amp));
      match_tok!(self, TokenKind::Amp)
    } {
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

    while {
      self.skip_newline_before(|k| matches!(k, TokenKind::Xor));
      match_tok!(self, TokenKind::Xor)
    } {
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

    while {
      self.skip_newline_before(|k| matches!(k, TokenKind::Bar));
      match_tok!(self, TokenKind::Bar)
    } {
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

    while {
      self.skip_newline_before(|k| matches!(k, comparison_operators!()));
      match_tok!(self, comparison_operators!())
    } {
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

    while {
      self.skip_newline_before(|k| matches!(k, equality_operators!()));
      match_tok!(self, equality_operators!())
    } {
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

    while {
      self.skip_newline_before(|k| matches!(k, TokenKind::And));
      match_tok!(self, TokenKind::And)
    } {
      let op = self.previous().clone().kind;
      self.ignore_newlines();
      let right = self.equality();
      expr = Expr::Circuit(Box::new(expr), op, Box::new(right));
    }

    expr
  }

  fn or(&mut self) -> Expr {
    let mut expr = self.and();

    while {
      self.skip_newline_before(|k| matches!(k, TokenKind::Or));
      match_tok!(self, TokenKind::Or)
    } {
      let op = self.previous().clone().kind;
      self.ignore_newlines();
      let right = self.and();
      expr = Expr::Circuit(Box::new(expr), op, Box::new(right));
    }

    expr
  }

  fn conditional(&mut self) -> Expr {
    let mut expr = self.or();

    // The `?` and the `:` may each begin a new line, the same way a
    // binary operator can, so a ternary spread across several lines
    // reads with the operators leading each branch. A leading `?` after
    // an expression, or a leading `:` once a `?` has been seen, can only
    // be a ternary, so skipping the newline before them is unambiguous.
    self.skip_newline_before(|k| matches!(k, TokenKind::Question));

    if match_tok!(self, TokenKind::Question) {
      self.ignore_newlines();
      let truth = self.conditional();

      self.skip_newline_before(|k| matches!(k, TokenKind::Colon));
      consume_tok!(self, TokenKind::Colon, "Expected ':' in tenary operation.");
      self.ignore_newlines();

      let falsy = self.conditional();
      expr = Expr::Condition(Box::new(expr), Box::new(truth), Box::new(falsy));
    }

    expr
  }

  fn assignment(&mut self) -> Expr {
    // Same reasoning as `assign_expr`'s own `start_token`: captured
    // before `conditional()` runs so a bad target's error points at
    // where that expression starts, not at the `=`/`+=`/etc. after it.
    let start_token = self.peek().clone();
    let mut expr = self.conditional();

    if match_tok!(self, assignment_operators!()) {
      let type_token = self.previous().clone();
      self.ignore_newlines();
      let valid_target = matches!(expr, Expr::Identifier(_) | Expr::Index(..));

      if matches!(type_token.kind.clone(), TokenKind::Equal) {
        let value = self.assignment();
        expr = if valid_target {
          Expr::Assign(Box::new(expr), Box::new(value))
        } else {
          self.report_error_at("invalid assignment target".to_string(), start_token.clone());
          value
        };
      } else {
        let right = self.assignment();
        let binary = Expr::Binary(
          Box::new(expr.clone()),
          get_assignment_alt(type_token.clone().kind),
          Box::new(right),
          type_token.line as u32,
        );
        expr = if valid_target {
          Expr::Assign(Box::new(expr), Box::new(binary))
        } else {
          self.report_error_at("invalid assignment target".to_string(), start_token.clone());
          binary
        };
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

    self.anonymous_count += 1;

    self.functions_count -= 1;
    Expr::Anonymous(Box::new(function))
  }

  // Statements

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
    // Same reasoning as `parse`'s priming drain: this is what makes
    // comments right after the opening `{` visible here rather than
    // getting silently eaten by `statement`'s own leading `ignore_newlines`.
    self.drain_trivia_into_stmts(&mut vals);

    while !check_tok!(self, TokenKind::Rbrace) && !self.is_at_end() {
      vals.push(self.statement());
      self.drain_trivia_into_stmts(&mut vals);
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

  // Everything this function builds is synthetic — desugared sugar for
  // `for k, v in iterable { ... }` that doesn't correspond to any single
  // token the user actually wrote. Every synthesized node here shares one
  // `start` (the position of the loop variable, right after `for`), an
  // honest span for "generated by the for-loop desugaring at this point".
  //
  // That's the opposite of the binary-chain functions above, whose chain
  // nodes are built directly from real user-written operands that can span
  // multiple lines and so each need their own accurate position. There's no
  // user-written sub-expression here for most of these nodes to point at
  // more precisely than "the for statement" — except the actual loop body
  // (`self.statement()`), which is real user code and keeps its own,
  // independently correct position.
  fn for_stmt(&mut self) -> Stmt {
    let key_id = consume_tok!(self, TokenKind::Identifier(_), "Variable name expected");
    let mut value_id = key_id.clone();

    let key_decl_name = key_id.copy_to(TokenKind::Identifier("$key".to_string()));
    let mut key_track_id = key_decl_name.clone();

    let key_nil = self.compose_nil();
    let mut key = Stmt::Var(key_decl_name, Box::new(key_nil), None, false);

    let value_decl_name = key_id.clone();
    let value_nil = self.compose_nil();
    let mut value = Stmt::Var(value_decl_name, Box::new(value_nil), None, false);

    let mut is_two_var = false;
    if match_tok!(self, TokenKind::Comma) {
      is_two_var = true;
      key = value;
      key_track_id = key_id.clone();

      value_id = consume_tok!(self, TokenKind::Identifier(_), "Variable name expected");

      let value_decl_name2 = value_id.clone();
      let value_nil2 = self.compose_nil();
      value = Stmt::Var(value_decl_name2, Box::new(value_nil2), None, false);
    }

    consume_tok!(self, TokenKind::In, "Expected 'in' after 'for' statement.");

    let iterable = self.expression();

    // Fast path for `for x in LOWER..UPPER { body }` (single-variable
    // form): see for_range_fast_path's own doc comment for why.
    if !is_two_var {
      if let Expr::Range(lower, upper, line) = iterable.clone() {
        return self.for_range_fast_path(key_id, *lower, *upper, line);
      }
    }

    // Evaluate the iterable expression exactly ONCE, into a synthetic
    // local ($iter) declared OUTSIDE the loop. Cloning `iterable` directly
    // into the @key/@value getters below (both inside the generated
    // while-loop's body) would re-evaluate; and for a list/range
    // literal, re-allocate; it on every single iteration. Evaluating
    // once is also the only semantically correct behavior for an iterable
    // with side effects or internal generator state.
    let iter_name = key_id.copy_to(TokenKind::Identifier("$iter".to_string()));
    let iter_decl = Stmt::Var(iter_name.clone(), Box::new(iterable), None, false);

    let mut stmt_list = Vec::new();

    // key = $iter.@key(key)
    {
      let get_name = key_track_id.copy_to(TokenKind::Identifier("@key".to_string()));
      let iter_expr = self.compose_id(iter_name.clone());
      let getter = self.compose_get(iter_expr, get_name);
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

    // value = $iter.@value(key)
    {
      let get_name = value_id.copy_to(TokenKind::Identifier("@value".to_string()));
      let iter_expr = self.compose_id(iter_name.clone());
      let getter = self.compose_get(iter_expr, get_name);
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

    Stmt::Block(vec![iter_decl, key, value, body])
  }

  /// Fast path for `for x in LOWER..UPPER { body }` (single-variable
  /// form only; range key/value order intentionally differs once an
  /// index variable is also requested, so `for k, v in a..b` always
  /// falls through to the general @key/@value path in `for_stmt`).
  /// Compiles directly to a counting loop instead of allocating an
  /// Obj::Range and invoking `@key`/`@value` through the generic
  /// Instr::Invoke -> builtins::lookup -> FxHashMap<&str,..> dispatch
  /// path once per iteration.
  ///
  /// Every synthesized name here is prefixed with `$`, which the
  /// lexer never produces from user source, so these can never
  /// collide with a real user identifier; same trick `for_stmt`'s
  /// own `$key` already relies on.
  ///
  /// NOTE: for LOWER == UPPER, `Range::_key`'s own native
  /// implementation currently returns `Value::bool(false)` as its
  /// very first key (not `nil`), which the general for-loop path's
  /// `if key == nil break` check does NOT catch; so today, `for x
  /// in a..a { ... }` raises a TypeError from `_value` on its first
  /// iteration rather than doing nothing. This fast path instead
  /// treats LOWER == UPPER as a correctly empty loop (zero
  /// iterations), which is the intended behavior; this is a
  /// deliberate, beneficial behavior change for that one edge case,
  /// not an oversight.
  fn for_range_fast_path(&mut self, var_id: Token, lower: Expr, upper: Expr, line: u32) -> Stmt {
    let lower_name = var_id.copy_to(TokenKind::Identifier("$lower".to_string()));
    let upper_name = var_id.copy_to(TokenKind::Identifier("$upper".to_string()));
    let count_name = var_id.copy_to(TokenKind::Identifier("$count".to_string()));
    let step_name = var_id.copy_to(TokenKind::Identifier("$step".to_string()));
    let n_name = var_id.copy_to(TokenKind::Identifier("$n".to_string()));

    // var $lower = LOWER ; var $upper = UPPER  (each evaluated exactly
    // once, in the same order the original Expr::Range compilation
    // would have evaluated them)
    let lower_decl = Stmt::Var(lower_name.clone(), Box::new(lower), None, false);
    let upper_decl = Stmt::Var(upper_name.clone(), Box::new(upper), None, false);

    // $upper >= $lower ; decides both direction and, reused below,
    // which of the two diffs becomes $count.
    let ge = Expr::Logical(
      Box::new(self.compose_id(upper_name.clone())),
      TokenKind::GreaterEq,
      Box::new(self.compose_id(lower_name.clone())),
      line,
    );
    let asc_diff = Expr::Binary(
      Box::new(self.compose_id(upper_name.clone())),
      TokenKind::Minus,
      Box::new(self.compose_id(lower_name.clone())),
      line,
    );
    let desc_diff = Expr::Binary(
      Box::new(self.compose_id(lower_name.clone())),
      TokenKind::Minus,
      Box::new(self.compose_id(upper_name.clone())),
      line,
    );
    // var $count = $upper >= $lower ? $upper - $lower : $lower - $upper
    let count_val = Expr::Condition(Box::new(ge), Box::new(asc_diff), Box::new(desc_diff));
    let count_decl = Stmt::Var(count_name.clone(), Box::new(count_val), None, false);

    // var $step = $upper >= $lower ? 1 : -1
    let ge_for_step = Expr::Logical(
      Box::new(self.compose_id(upper_name.clone())),
      TokenKind::GreaterEq,
      Box::new(self.compose_id(lower_name.clone())),
      line,
    );
    let step_val = Expr::Condition(
      Box::new(ge_for_step),
      Box::new(Expr::Integer(1)),
      Box::new(Expr::Integer(-1)),
    );
    let step_decl = Stmt::Var(step_name.clone(), Box::new(step_val), None, false);

    // var $n = 0
    let n_decl = Stmt::Var(n_name.clone(), Box::new(Expr::Integer(0)), None, false);

    // var VAR = $lower
    let lower_expr = self.compose_id(lower_name.clone());
    let var_decl = Stmt::Var(var_id.clone(), Box::new(lower_expr), None, false);

    // parse the loop body — real user code, keeps its own position
    let body = self.statement();

    // VAR += $step  ;  $n += 1
    // Placed AFTER Stmt::FixContinue, mirroring iter_stmt's own
    // desugaring, so `continue` inside the body still advances the
    // loop variable instead of skipping straight back to the
    // condition check.
    let var_advance = Stmt::Expression(Box::new(Expr::Assign(
      Box::new(self.compose_id(var_id.clone())),
      Box::new(Expr::Binary(
        Box::new(self.compose_id(var_id.clone())),
        TokenKind::Plus,
        Box::new(self.compose_id(step_name.clone())),
        line,
      )),
    )));
    let n_advance = Stmt::Expression(Box::new(Expr::Assign(
      Box::new(self.compose_id(n_name.clone())),
      Box::new(Expr::Binary(
        Box::new(self.compose_id(n_name.clone())),
        TokenKind::Plus,
        Box::new(Expr::Integer(1)),
        line,
      )),
    )));

    let block_body = Stmt::Block(vec![body, Stmt::FixContinue, var_advance, n_advance]);

    // while $n < $count { ... }
    let cond = Expr::Logical(
      Box::new(self.compose_id(n_name)),
      TokenKind::Less,
      Box::new(self.compose_id(count_name)),
      line,
    );
    let while_stmt = Stmt::While(Box::new(cond), Box::new(block_body));

    Stmt::Block(vec![
      lower_decl, upper_decl, count_decl, step_decl, n_decl, var_decl, while_stmt,
    ])
  }

  fn assert_stmt(&mut self) -> Stmt {
    let expr = self.expression();
    let mut message = None;

    if match_tok!(self, TokenKind::Comma) {
      self.ignore_newlines_only();
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
          self.report_error_at(
            "'when' or 'default' state cannot exist after a default state".to_string(),
            self.previous().clone(),
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
          TokenKind::Newline => {},
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

    let exported = match_tok!(self, TokenKind::At);

    // `.` (same directory), `..` (parent directory, lexed as a `Range`
    // token; there's no dedicated ".." token kind), and identifiers can
    // all repeat and interleave freely in a relative import path (e.g.
    // `..package..root_package..module`), so this is one unified loop
    // rather than "one leading `..`, then only dots/identifiers"; the
    // latter silently stopped consuming after the FIRST embedded `..`.
    while match_tok!(
      self,
      TokenKind::Dot | TokenKind::Range | TokenKind::Identifier(_)
    ) {
      let token = self.previous().clone();

      match token.kind.clone() {
        TokenKind::Dot => paths.push(".".to_string()),
        TokenKind::Range => paths.push("..".to_string()),
        TokenKind::Identifier(name) => paths.push(name.clone()),
        _ => {},
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
          if !elements.is_empty() {
            self.report_error_at(
              "Cannot import selected items and everything from the same import statement."
                .to_string(),
              element.clone(),
            );
          }

          imports_all = true;
          break;
        } else if let TokenKind::Identifier(name) = element.kind.clone()
          && name.starts_with("_")
        {
          self.report_error_at(
            "Cannot import private items from module".to_string(),
            element.clone(),
          );
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
      match paths.last() {
        Some(last) => {
          let synthesized = self.previous().copy_to(TokenKind::Literal(last.clone()));
          name = self.compose_id(synthesized);
        },
        // `import` with no path at all (e.g. `import { x }`); nothing
        // to synthesize a default name from, so say so instead of
        // panicking on the empty `paths`.
        None => self.report_error("Expected a module path after 'import'".to_string()),
      }
    }

    Stmt::Import(final_path, Box::new(name), elements, imports_all, exported)
  }

  fn catch_stmt(&mut self) -> Stmt {
    let body = self.match_block("Expected '{' after 'catch' statement.".to_string());
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
          self.match_block("Expected '{' after error variable.".to_string()),
        ));
      }
    }

    Stmt::Catch(Box::new(body), catch_body, name)
  }

  // Like for statement and do..while statement, iter statement is also desugared
  // into while statement
  fn iter_stmt(&mut self) -> Stmt {
    // Enclosing parens are optional; match_tok! already consumed it if present.
    if match_tok!(self, TokenKind::Lparen) {}

    let mut final_stmts = Vec::new();

    let top_stmt = if !match_tok!(self, TokenKind::Semicolon) {
      // `var` is optional here too, same reason.
      if match_tok!(self, TokenKind::Var) {}

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

    let condition = if !match_tok!(self, TokenKind::Semicolon) {
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

    // Closing paren is optional, same as the opening one above.
    if match_tok!(self, TokenKind::Rparen) {}

    let body = self.match_block("Expected '{' at the start of iter body.".to_string());

    let mut block_body = Vec::new();
    block_body.push(body);

    block_body.push(Stmt::FixContinue);
    for stmt in final_stmts {
      block_body.push(stmt);
    }

    let block_body_stmt = Stmt::Block(block_body);

    let mut final_body = Vec::new();

    if let Some(stmt) = top_stmt {
      final_body.push(stmt);
    }

    final_body.push(Stmt::While(Box::new(condition), Box::new(block_body_stmt)));

    Stmt::Block(final_body)
  }

  fn statement(&mut self) -> Stmt {
    self.ignore_newlines();

    let result = match self.advance().kind {
      TokenKind::As => {
        self.report_error_at(
          "'as' is only a valid keyword in catch context".to_string(),
          self.previous().clone(),
        );
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
      TokenKind::Def => {
        if matches!(self.peek().kind, TokenKind::Identifier(_))
          || matches!(self.peek().kind, TokenKind::Decorator(_))
        {
          Stmt::Decl(Box::new(self.function_decl()))
        } else {
          self.rewind();
          self.expression_stmt(false)
        }
      },
      TokenKind::Continue => Stmt::Continue,
      TokenKind::Break => Stmt::Break,
      TokenKind::Return => {
        if self.functions_count == 0 {
          self.report_error_at(
            "'return' is only a valid keyword in function context".to_string(),
            self.previous().clone(),
          );
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
      _ => {
        self.rewind();
        self.expression_stmt(false)
      },
    };

    self.ignore_newlines();

    result
  }

  // Declarations

  fn var_decl(&mut self, is_constant: bool) -> Stmt {
    let mut declarations = Vec::new();

    loop {
      let name_token = consume_tok!(self, TokenKind::Identifier(_), "Variable name expected.");

      let type_hint = if match_tok!(self, TokenKind::Colon) {
        // type hinting
        Some(Box::new(self.parse_type()))
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
      self.ignore_newlines();

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
      Box::new(self.parse_type())
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
    // Which list the most recently parsed real member landed in, so a
    // trailing comment right before the closing `}` (nothing left to
    // peek ahead at, so the "which list does this precede" trick below
    // doesn't apply) can instead follow whatever it comes right AFTER.
    // `None` only while the body has had no real member yet.
    let mut last_member_was_method: Option<bool> = None;

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

      // The `ignore_newlines` above can itself walk all the way past a
      // trailing comment up to the closing `}` (or EOF on malformed
      // input), which would make the `while` condition above false on
      // its *next* check without the loop body ever running again,
      // so this can't be folded into that condition. It has to be
      // re-checked here, every iteration, before touching `properties`/
      // `methods`.
      if check_tok!(self, TokenKind::Rbrace) || self.is_at_end() {
        break;
      }

      // Peek past an optional `static` to see whether the member that's
      // about to be parsed is a field (`var`/`const`) or a method, so any
      // comments buffered since the last member land in whichever list
      // this one actually precedes.
      let member_start = if matches!(self.current.kind, TokenKind::Static) {
        self.peek_at(1).kind
      } else {
        self.current.kind.clone()
      };

      if matches!(member_start, TokenKind::Var | TokenKind::Const) {
        self.drain_trivia_into_decls(&mut properties);
      } else {
        self.drain_trivia_into_decls(&mut methods);
      }

      let is_static = match_tok!(self, TokenKind::Static);

      if match_tok!(self, TokenKind::Var) {
        properties.push(self.class_field(is_static, false));
        last_member_was_method = Some(false);
      } else if match_tok!(self, TokenKind::Const) {
        properties.push(self.class_field(is_static, true));
        last_member_was_method = Some(false);
      } else {
        // `def` before a method name is optional sugar; consumed and ignored.
        if match_tok!(self, TokenKind::Def) {}

        methods.push(self.method_decl(is_static));
        last_member_was_method = Some(true);
      }

      self.ignore_newlines();
    }

    // Trailing comments after the last member, before the closing `}`:
    // nothing in `properties`/`methods` "comes after" them, so they're
    // attached to whichever list the member right BEFORE them belongs
    // to (a trailing comment after a class's last field, with no methods
    // following, reads as commentary on that field, not on an unrelated
    // and possibly nonexistent method); `methods` only as a fallback for
    // a body with no real members at all to follow. A caller wanting the
    // true interleaved order across both lists should merge and sort by
    // `(line, col)`, see `ast.zu`'s docs. Deliberately outside the loop
    // above rather than in a branch of it: the loop can exit either via
    // its own top-of-body break or via the outer `while` condition going
    // false first (exactly the case the comment on that break explains),
    // so this is the one point guaranteed to run either way.
    match last_member_was_method {
      Some(false) => self.drain_trivia_into_decls(&mut properties),
      _ => self.drain_trivia_into_decls(&mut methods),
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
      TokenKind::Def => {
        if matches!(self.peek().kind, TokenKind::Identifier(_))
          || matches!(self.peek().kind, TokenKind::Decorator(_))
        {
          self.function_decl()
        } else {
          self.rewind();
          Decl::Stmt(Box::new(self.statement()))
        }
      },
      TokenKind::Class => self.class_decl(),
      TokenKind::Lbrace => {
        if !check_tok!(self, TokenKind::Newline) && self.block_count == 0 {
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

    // Prime `current` (it starts as an unscanned `None` sentinel) and
    // skip any leading blank lines, same as `declaration`'s own leading
    // `ignore_newlines` would do. Done here first so *this* drain, not
    // declaration's, is the one that sees comments at the very top of the
    // file. Every drain below relies on the item just parsed having
    // already consumed its own trailing newlines/trivia (`declaration`
    // always ends with `ignore_newlines`), so by the time control returns
    // here `pending_trivia` holds exactly what sits between that item and
    // the next one.
    self.ignore_newlines();
    self.drain_trivia_into_decls(&mut result);

    while !self.is_at_end() {
      result.push(self.declaration());
      self.drain_trivia_into_decls(&mut result);
    }

    if self.errors.is_empty() {
      Ok(result)
    } else {
      Err(self.errors.clone())
    }
  }
}
