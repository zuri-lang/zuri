use big_num::BigInt;
use core::fmt;
use std::collections::HashMap;
use std::fmt::Display;
use std::str::Chars;
use std::str::FromStr;

use crate::token::{Token, TokenKind};

fn is_digit(c: char) -> bool {
  c.is_ascii_digit()
}

fn is_alpha(c: char) -> bool {
  c.is_ascii_alphabetic() || c == '_'
}

fn is_alphanumeric(c: char) -> bool {
  is_alpha(c) || is_digit(c)
}

fn is_binary(c: char) -> bool {
  c == '0' || c == '1'
}

fn is_octal(c: char) -> bool {
  c >= '0' && c <= '7'
}

fn is_hex(c: char) -> bool {
  c.is_ascii_hexdigit()
}

#[derive(Debug, Clone)]
pub struct LexerError {
  pub message: String,
  pub line: usize,
  pub column: usize,
}

impl LexerError {
  pub fn new(message: String, line: usize, column: usize) -> Self {
    Self {
      message,
      line,
      column,
    }
  }
}

impl Display for LexerError {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    write!(f, "[{}:{}] {}", self.line, self.column, self.message)
  }
}

#[derive(Clone)]
pub struct Lexer<'a> {
  pub source: Chars<'a>,
  pub tokens: Vec<Token>,
  pub line: usize,
  pub current: usize,
  pub errors: Vec<LexerError>,

  // private fields
  lines: HashMap<usize, usize>,
  start: usize,
  start_line: usize,
  total_count: usize,
  interpolating: Vec<char>,
}

impl<'a> Lexer<'a> {
  #[inline]
  pub fn new(source: &'a str) -> Self {
    Self {
      source: source.chars(),
      line: 1,
      start_line: 1,
      current: 0,
      start: 0,
      total_count: source.len(),
      lines: HashMap::new(),
      tokens: Vec::new(),
      errors: Vec::new(),
      interpolating: Vec::new(),
    }
  }

  fn make_error(&mut self, message: String) {
    self
      .errors
      .push(LexerError::new(message, self.line, self.start));
  }

  fn is_at_end(&self) -> bool {
    self.current >= self.total_count
  }

  fn advance(&mut self) -> char {
    let val = self.source.clone().nth(self.current).unwrap();
    self.current += 1;

    if val == '\n' {
      self.line += 1;
      self.lines.insert(self.line, self.current);
    }

    val
  }

  pub fn match_char(&mut self, c: char) -> bool {
    if self.is_at_end() || c != self.source.clone().nth(self.current).unwrap() {
      false
    } else {
      self.current += 1;

      if c == '\n' {
        self.line += 1;
        self.lines.insert(self.line, self.current);
      }

      true
    }
  }

  fn next(&mut self) -> char {
    if self.current + 1 > self.total_count {
      '\0'
    } else {
      self.source.clone().nth(self.current + 1).unwrap()
    }
  }

  fn previous(&mut self) -> char {
    if self.current == 0 {
      '\0'
    } else {
      self.source.clone().nth(self.current - 1).unwrap()
    }
  }

  fn peek(&mut self) -> char {
    if self.is_at_end() {
      '\0'
    } else {
      self.source.clone().nth(self.current).unwrap()
    }
  }

  fn peek_n(&mut self, n: usize) -> char {
    let n = n - 1;

    if self.is_at_end() || self.current + n >= self.total_count {
      '\0'
    } else {
      self.source.clone().nth(self.current + n).unwrap()
    }
  }

  fn add_token(&mut self, kind: TokenKind) {
    let line_start = if self.lines.is_empty() || self.start_line == 1 {
      0
    } else {
      *self.lines.get(&self.start_line).unwrap()
    };

    let column = self.start - line_start + 1;

    self.tokens.push(Token::new(kind, self.start_line, column));
  }

  fn skip_block_comments(&mut self) {
    let mut nesting: i32 = 1;

    while nesting > 0 {
      if self.is_at_end() {
        self.make_error("unbalanced block comment".to_string());
        return;
      }

      if self.peek() == '/' && self.next() == '*' {
        nesting += 1;
        self.advance();
        self.advance();
      } else if self.peek() == '*' && self.next() == '/' {
        nesting -= 1;
        self.advance();
        self.advance();
      } else {
        self.advance();
      }
    }
  }

  fn skip_whitespace(&mut self) {
    loop {
      if self.is_at_end() {
        break;
      }

      let c: char = self.peek();

      match c {
        ' ' | '\t' | '\r' => self.advance(),
        '#' => {
          while self.peek() != '\n' && !self.is_at_end() {
            self.advance();
          }
          break;
        },
        '/' => {
          if self.next() == '*' {
            self.advance();
            self.advance();
            self.skip_block_comments();
          }
          break;
        },
        _ => break,
      };
    }
  }

  fn decorator(&mut self) {
    while is_alphanumeric(self.peek()) {
      self.advance();
    }
    self.add_token(TokenKind::Decorator(
      self.source.as_str()[self.start..self.current].to_string(),
    ));
  }

  fn string(&mut self, c: char) {
    while self.peek() != c && !self.is_at_end() {
      if self.peek() == '&' && self.next() == '{' && self.previous() != '\\' {
        // We're at the start of an interpolation

        self.interpolating.push(c);
        self.current += 1;

        let inter_value = self.unescape_string(c.clone());

        self.add_token(TokenKind::Interpolation(inter_value));
        self.current += 1;
        return;
      }

      if self.peek() == '\\' && (self.next() == c || self.next() == '\\') {
        self.advance();
      }
      self.advance();
    }

    if self.is_at_end() {
      let message = format!("Unterminated string on line {}", self.line);
      self.make_error(message);
    }

    self.match_char(c.clone());

    let inter_value = self.unescape_string(c.clone());
    self.add_token(TokenKind::Literal(inter_value));
  }

  fn number(&mut self) {
    if self.previous() == '0' {
      if self.match_char('b') {
        // binary number
        while is_binary(self.peek()) {
          self.advance();
        }

        self.add_token(TokenKind::BinNumber(
          i64::from_str_radix(&self.source.as_str()[self.start..self.current], 2).unwrap(),
        ));
        return;
      } else if self.match_char('c') {
        // octal number
        while is_octal(self.peek()) {
          self.advance();
        }

        self.add_token(TokenKind::OctNumber(
          i64::from_str_radix(&self.source.as_str()[self.start..self.current], 8).unwrap(),
        ));
        return;
      } else if self.match_char('x') {
        // hex number
        while is_hex(self.peek()) {
          self.advance();
        }

        self.add_token(TokenKind::HexNumber(
          i64::from_str_radix(&self.source.as_str()[self.start..self.current], 16).unwrap(),
        ));
        return;
      }
    }

    while is_digit(self.peek()) || (self.peek() == '_' && is_digit(self.peek_n(2))) {
      self.advance();
    }

    if self.peek() == 'n' {
      // we've encountered a big integer
      self.advance();

      self.add_token(TokenKind::BigNumber(BigInt::from_str_radix(
        &self.source.as_str()[self.start..self.current],
        10,
      )));
      return;
    }

    if self.peek() == '.' && is_digit(self.next()) {
      // we've encountered a float
      self.advance();
      self.advance();
    }

    while is_digit(self.peek()) || (self.peek() == '_' && is_digit(self.peek_n(2))) {
      self.advance();
    }

    if self.peek() == 'e' || self.peek() == 'E' {
      self.advance();

      if self.peek() == '+' || self.peek() == '-' {
        self.advance();
      }

      while is_digit(self.peek()) {
        self.advance();
      }
    }

    let number = &self.source.as_str()[self.start..self.current].replace("_", "");

    if number.contains(".") {
      self.add_token(TokenKind::Double(f64::from_str(number).unwrap()));
    } else {
      self.add_token(TokenKind::Integer(i64::from_str(number).unwrap()));
    }
  }

  fn identifier(&mut self) {
    while is_alphanumeric(self.peek()) {
      self.advance();
    }

    let name = &self.source.as_str()[self.start..self.current];

    match name {
      "and" => self.add_token(TokenKind::And),
      "as" => self.add_token(TokenKind::As),
      "assert" => self.add_token(TokenKind::Assert),
      "break" => self.add_token(TokenKind::Break),
      "catch" => self.add_token(TokenKind::Catch),
      "class" => self.add_token(TokenKind::Class),
      "const" => self.add_token(TokenKind::Const),
      "continue" => self.add_token(TokenKind::Continue),
      "def" => self.add_token(TokenKind::Def),
      "default" => self.add_token(TokenKind::Default),
      "do" => self.add_token(TokenKind::Do),
      "echo" => self.add_token(TokenKind::Echo),
      "else" => self.add_token(TokenKind::Else),
      "false" => self.add_token(TokenKind::False),
      "finally" => self.add_token(TokenKind::Finally),
      "for" => self.add_token(TokenKind::For),
      "if" => self.add_token(TokenKind::If),
      "import" => self.add_token(TokenKind::Import),
      "in" => self.add_token(TokenKind::In),
      "iter" => self.add_token(TokenKind::Iter),
      "nil" => self.add_token(TokenKind::Nil),
      "new" => self.add_token(TokenKind::New),
      "or" => self.add_token(TokenKind::Or),
      "parent" => self.add_token(TokenKind::Parent),
      "raise" => self.add_token(TokenKind::Raise),
      "return" => self.add_token(TokenKind::Return),
      "self_" => self.add_token(TokenKind::Self_),
      "static" => self.add_token(TokenKind::Static),
      "true" => self.add_token(TokenKind::True),
      "try" => self.add_token(TokenKind::Try),
      "using" => self.add_token(TokenKind::Using),
      "var" => self.add_token(TokenKind::Var),
      "when" => self.add_token(TokenKind::When),
      "while" => self.add_token(TokenKind::While),
      _ => self.add_token(TokenKind::Identifier(name.to_string())),
    }
  }

  fn unescape_string(&mut self, quote: char) -> String {
    let mut final_str = String::new();

    let end = self.current - 1;
    let mut i = self.start + 1;

    while i < end {
      let c = self.source.clone().nth(i).unwrap();

      if c == '\\' && i + 9 < end && self.source.clone().nth(i + 1).unwrap() == 'U' {
        if let Ok(number) = u32::from_str_radix(&self.source.as_str()[i + 2..i + 10], 16) {
          let char = char::from_u32(number).unwrap();
          final_str.push(char);
          i += 9;
        } else {
          // // treat it as regular string
          // final_str.push(c);
          // Or throw an error
          self.make_error("invalid unicode escape sequence".to_string());
        }
      } else if c == '\\' && i + 5 < end && self.source.clone().nth(i + 1).unwrap() == 'u' {
        if let Ok(number) = u32::from_str_radix(&self.source.as_str()[i + 2..i + 6], 16) {
          let char = char::from_u32(number).unwrap();
          final_str.push(char);
          i += 5;
        } else {
          // // treat it as regular string
          // final_str.push(c);
          // Or throw an error
          self.make_error("invalid unicode escape sequence".to_string());
        }
      } else if c == '\\' && i + 3 < end && self.source.clone().nth(i + 1).unwrap() == 'x' {
        if let Ok(number) = u32::from_str_radix(&self.source.as_str()[i + 2..i + 4], 16) {
          let char = char::from_u32(number).unwrap();
          final_str.push(char);
          i += 3;
        } else {
          // // treat it as regular string
          // final_str.push(c);
          // Or throw an error
          self.make_error("invalid hex escape sequence".to_string());
        }
      } else if c == '\\' && i + 1 < end {
        let next = self.source.clone().nth(i + 1).unwrap();

        match next {
          '0' => final_str.push('\0'),
          'a' => final_str.push('\x07'),
          'b' => final_str.push('\x08'),
          'f' => final_str.push('\x0C'),
          'n' => final_str.push('\n'),
          't' => final_str.push('\t'),
          'r' => final_str.push('\r'),
          'v' => final_str.push('\x0B'),
          '\'' => {
            // using unicode for } since syntax highlighters seem to have issue with the character
            if quote != '\'' && quote != '\u{0125}' {
              final_str.push('\\');
            }
            final_str.push(next);
          },
          '"' => {
            // same reason as above
            if quote != '"' && quote != '\u{0125}' {
              final_str.push('\\');
            }
            final_str.push(next);
          },
          _ => final_str.push(next),
        };

        i += 1;
      } else {
        final_str.push(c);
      }

      // i++
      i += 1;
    }

    final_str
  }

  pub fn scan(&mut self) {
    self.skip_whitespace();

    self.start = self.current;
    self.start_line = self.line;

    if self.is_at_end() {
      return;
    }

    let c = self.advance();

    match c {
      '(' => self.add_token(TokenKind::Lparen),
      ')' => self.add_token(TokenKind::Rparen),
      '[' => self.add_token(TokenKind::Lbracket),
      ']' => self.add_token(TokenKind::Rbracket),
      '{' => self.add_token(TokenKind::Lbrace),
      '}' => {
        if !self.interpolating.is_empty() {
          if let Some(v) = self.interpolating.pop() {
            self.string(v);
          }
        } else {
          self.add_token(TokenKind::Rbrace)
        }
      },
      ',' => self.add_token(TokenKind::Comma),
      ';' => self.add_token(TokenKind::Semicolon),
      '@' => {
        if !is_alpha(self.peek()) {
          self.add_token(TokenKind::At);
        } else {
          self.decorator();
        }
      },
      '.' => {
        if self.match_char('.') {
          if self.match_char('.') {
            self.add_token(TokenKind::TriDot);
          } else {
            self.add_token(TokenKind::Range);
          }
        } else {
          self.add_token(TokenKind::Dot);
        }
      },
      '-' => {
        if self.match_char('-') {
          self.add_token(TokenKind::Decrement);
        } else if self.match_char('=') {
          self.add_token(TokenKind::MinusEq);
        } else {
          self.add_token(TokenKind::Minus);
        }
      },
      '+' => {
        if self.match_char('+') {
          self.add_token(TokenKind::Increment);
        } else if self.match_char('=') {
          self.add_token(TokenKind::PlusEq);
        } else {
          self.add_token(TokenKind::Plus);
        }
      },
      '*' => {
        if self.match_char('*') {
          if self.match_char('=') {
            self.add_token(TokenKind::PowEq);
          } else {
            self.add_token(TokenKind::Pow);
          }
        } else if self.match_char('=') {
          self.add_token(TokenKind::MultiplyEq);
        } else {
          self.add_token(TokenKind::Multiply);
        }
      },
      '/' => {
        if self.match_char('/') {
          if self.match_char('=') {
            self.add_token(TokenKind::FloorEq);
          } else {
            self.add_token(TokenKind::Floor);
          }
        } else if self.match_char('=') {
          self.add_token(TokenKind::DivideEq);
        } else {
          self.add_token(TokenKind::Divide);
        }
      },
      '\\' => self.add_token(TokenKind::Backslash),
      ':' => self.add_token(TokenKind::Colon),
      '<' => {
        if self.match_char('<') {
          if self.match_char('=') {
            self.add_token(TokenKind::LshiftEq);
          } else {
            self.add_token(TokenKind::Lshift);
          }
        } else if self.match_char('=') {
          self.add_token(TokenKind::LessEq);
        } else {
          self.add_token(TokenKind::Less);
        }
      },
      '>' => {
        if self.match_char('>') {
          if self.match_char('>') {
            if self.match_char('=') {
              self.add_token(TokenKind::UrshiftEq);
            } else {
              self.add_token(TokenKind::Urshift);
            }
          } else {
            if self.match_char('=') {
              self.add_token(TokenKind::RshiftEq);
            } else {
              self.add_token(TokenKind::Rshift);
            }
          }
        } else if self.match_char('=') {
          self.add_token(TokenKind::GreaterEq);
        } else {
          self.add_token(TokenKind::Greater);
        }
      },
      '!' => {
        if self.match_char('=') {
          self.add_token(TokenKind::BangEq);
        } else {
          self.add_token(TokenKind::Bang);
        }
      },
      '=' => {
        if self.match_char('=') {
          self.add_token(TokenKind::EqualEq);
        } else if self.match_char('>') {
          self.add_token(TokenKind::Arrow);
        } else {
          self.add_token(TokenKind::Equal);
        }
      },
      '%' => {
        if self.match_char('=') {
          self.add_token(TokenKind::PercentEq);
        } else {
          self.add_token(TokenKind::Percent);
        }
      },
      '&' => {
        if self.match_char('=') {
          self.add_token(TokenKind::AmpEq);
        } else {
          self.add_token(TokenKind::Amp);
        }
      },
      '|' => {
        if self.match_char('=') {
          self.add_token(TokenKind::BarEq);
        } else {
          self.add_token(TokenKind::Bar);
        }
      },
      '^' => {
        if self.match_char('=') {
          self.add_token(TokenKind::XorEq);
        } else {
          self.add_token(TokenKind::Xor);
        }
      },
      '~' => {
        if self.match_char('=') {
          self.add_token(TokenKind::TildeEq);
        } else {
          self.add_token(TokenKind::Tilde);
        }
      },
      '?' => self.add_token(TokenKind::Question),
      '\n' => self.add_token(TokenKind::Newline),
      '\'' | '"' => self.string(c),
      _ => {
        if is_digit(c) {
          self.number();
        } else if is_alpha(c) {
          self.identifier();
        } else {
          self.make_error(format!("Unexpected character {}", c))
        }
      },
    };
  }

  pub fn run(&mut self) -> Result<&mut Vec<Token>, Vec<LexerError>> {
    while !self.is_at_end() {
      self.skip_whitespace();
      self.scan();
    }

    self.add_token(TokenKind::Eof);

    if self.errors.is_empty() {
      Ok(self.tokens.as_mut())
    } else {
      Err(self.errors.clone())
    }
  }
}
