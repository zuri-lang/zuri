use num_bigint::BigInt;
use std::collections::HashMap;
use std::str::Chars;
use std::str::FromStr;

use crate::compiler::token::{Token, TokenKind};

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

#[derive(Clone)]
pub struct Lexer<'a> {
  pub source: Chars<'a>,
  pub line: usize,
  pub current: usize,

  // private fields
  lexing_type: usize,
  start: usize,
  start_line: usize,
  total_count: usize,
  interpolating: Vec<char>,
  lines: HashMap<usize, usize>,
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
      lexing_type: 0,
      total_count: source.len(),
      lines: HashMap::new(),
      interpolating: Vec::new(),
    }
  }

  fn make_error(&mut self, message: String) -> Token {
    self.make_token(TokenKind::Error(message, self.line, self.start))
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

  pub fn rewind(&mut self) {
    // println!("{}, {}, {}", self.current, self.start, self.line);

    let val = if self.is_at_end() {
      '\0'
    } else {
      self.source.clone().nth(self.current - 1).unwrap()
    };
    if val == '\n' || val == '\0' {
      self.line -= 1;
      // self.lines.remove(&self.line);
    }

    self.current = self.start;
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

  fn make_token(&mut self, kind: TokenKind) -> Token {
    let mut line_start = if self.lines.is_empty() || self.start_line == 1 {
      0
    } else {
      *self.lines.get(&self.start_line).unwrap_or(&0)
    };

    if line_start > self.start {
      line_start = self.source.as_str()[0..self.start + 1]
        .rfind(|f| f == '\n')
        .unwrap_or(0);

      self.line = *self
        .lines
        .keys()
        .filter(|f| self.lines[f] <= line_start)
        .last()
        .unwrap_or(&1);
    }

    // println!(
    //   "start = {}, line start = {}, start line = {}",
    //   self.start, line_start, self.start_line
    // );
    let column = self.start - line_start + 1;

    Token::new(kind, self.start_line, column)
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

  fn decorator(&mut self) -> Token {
    while is_alphanumeric(self.peek()) {
      self.advance();
    }
    self.make_token(TokenKind::Decorator(
      self.source.as_str()[self.start..self.current].to_string(),
    ))
  }

  fn string(&mut self, c: char) -> Token {
    while self.peek() != c && !self.is_at_end() {
      if self.peek() == '$' && self.next() == '{' && self.previous() != '\\' {
        // We're at the start of an interpolation

        self.interpolating.push(c);
        self.current += 1;

        let inter_value = self.unescape_string(c.clone());

        let result = if inter_value.is_ok() {
          self.make_token(TokenKind::Interpolation(inter_value.unwrap()))
        } else {
          inter_value.unwrap_err()
        };
        self.current += 1;
        return result;
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

    if inter_value.is_ok() {
      self.make_token(TokenKind::Literal(inter_value.unwrap()))
    } else {
      inter_value.unwrap_err()
    }
  }

  fn number(&mut self) -> Token {
    if self.previous() == '0' {
      if self.match_char('b') {
        // binary number
        while is_binary(self.peek()) {
          self.advance();
        }

        return self.make_token(TokenKind::BinNumber(
          i64::from_str_radix(&self.source.as_str()[self.start..self.current], 2).unwrap(),
        ));
      } else if self.match_char('c') {
        // octal number
        while is_octal(self.peek()) {
          self.advance();
        }

        return self.make_token(TokenKind::OctNumber(
          i64::from_str_radix(&self.source.as_str()[self.start..self.current], 8).unwrap(),
        ));
      } else if self.match_char('x') {
        // hex number
        while is_hex(self.peek()) {
          self.advance();
        }

        return self.make_token(TokenKind::HexNumber(
          i64::from_str_radix(&self.source.as_str()[self.start..self.current], 16).unwrap(),
        ));
      }
    }

    while is_digit(self.peek()) || (self.peek() == '_' && is_digit(self.peek_n(2))) {
      self.advance();
    }

    if self.peek() == 'n' {
      // we've encountered a big integer
      self.advance();

      return self.make_token(TokenKind::BigNumber(
        BigInt::from_str(&self.source.as_str()[self.start..self.current - 1])
          .unwrap_or(BigInt::ZERO),
      ));
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
      self.make_token(TokenKind::Double(f64::from_str(number).unwrap_or(0.0)))
    } else {
      self.make_token(TokenKind::Integer(i64::from_str(number).unwrap_or(0)))
    }
  }

  fn identifier(&mut self) -> Token {
    while is_alphanumeric(self.peek()) {
      self.advance();
    }

    let name = &self.source.as_str()[self.start..self.current];

    match name {
      "and" => self.make_token(TokenKind::And),
      "as" => self.make_token(TokenKind::As),
      "assert" => self.make_token(TokenKind::Assert),
      "break" => self.make_token(TokenKind::Break),
      "catch" => self.make_token(TokenKind::Catch),
      "class" => self.make_token(TokenKind::Class),
      "const" => self.make_token(TokenKind::Const),
      "continue" => self.make_token(TokenKind::Continue),
      "def" => self.make_token(TokenKind::Def),
      "default" => self.make_token(TokenKind::Default),
      "do" => self.make_token(TokenKind::Do),
      "echo" => self.make_token(TokenKind::Echo),
      "else" => self.make_token(TokenKind::Else),
      "false" => self.make_token(TokenKind::False),
      "for" => self.make_token(TokenKind::For),
      "if" => self.make_token(TokenKind::If),
      "import" => self.make_token(TokenKind::Import),
      "in" => self.make_token(TokenKind::In),
      "iter" => self.make_token(TokenKind::Iter),
      "nil" => self.make_token(TokenKind::Nil),
      "or" => self.make_token(TokenKind::Or),
      "parent" => self.make_token(TokenKind::Parent),
      "raise" => self.make_token(TokenKind::Raise),
      "return" => self.make_token(TokenKind::Return),
      "self" => self.make_token(TokenKind::Self_),
      "static" => self.make_token(TokenKind::Static),
      "true" => self.make_token(TokenKind::True),
      "using" => self.make_token(TokenKind::Using),
      "var" => self.make_token(TokenKind::Var),
      "when" => self.make_token(TokenKind::When),
      "while" => self.make_token(TokenKind::While),
      _ => self.make_token(TokenKind::Identifier(name.to_string())),
    }
  }

  fn unescape_string(&mut self, quote: char) -> Result<String, Token> {
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
          return Err(self.make_error("invalid unicode escape sequence".to_string()));
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
          return Err(self.make_error("invalid unicode escape sequence".to_string()));
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
          return Err(self.make_error("invalid hex escape sequence".to_string()));
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

    Ok(final_str)
  }

  pub fn scan(&mut self) -> Token {
    self.skip_whitespace();

    self.start = self.current;
    self.start_line = self.line;

    if self.is_at_end() {
      return self.make_token(TokenKind::Eof);
    }

    let c = self.advance();

    match c {
      '(' => self.make_token(TokenKind::Lparen),
      ')' => self.make_token(TokenKind::Rparen),
      '[' => self.make_token(TokenKind::Lbracket),
      ']' => self.make_token(TokenKind::Rbracket),
      '{' => self.make_token(TokenKind::Lbrace),
      '}' => {
        if !self.interpolating.is_empty() {
          if let Some(v) = self.interpolating.pop() {
            self.string(v)
          } else {
            self.make_token(TokenKind::Interpolation("".to_string()))
          }
        } else {
          self.make_token(TokenKind::Rbrace)
        }
      },
      ',' => self.make_token(TokenKind::Comma),
      ';' => self.make_token(TokenKind::Semicolon),
      '@' => {
        if !is_alpha(self.peek()) {
          self.make_token(TokenKind::At)
        } else {
          self.decorator()
        }
      },
      '.' => {
        if self.match_char('.') {
          if self.match_char('.') {
            self.make_token(TokenKind::TriDot)
          } else {
            self.make_token(TokenKind::Range)
          }
        } else {
          self.make_token(TokenKind::Dot)
        }
      },
      '-' => {
        if self.match_char('-') {
          self.make_token(TokenKind::Decrement)
        } else if self.match_char('=') {
          self.make_token(TokenKind::MinusEq)
        } else {
          self.make_token(TokenKind::Minus)
        }
      },
      '+' => {
        if self.match_char('+') {
          self.make_token(TokenKind::Increment)
        } else if self.match_char('=') {
          self.make_token(TokenKind::PlusEq)
        } else {
          self.make_token(TokenKind::Plus)
        }
      },
      '*' => {
        if self.match_char('*') {
          if self.match_char('=') {
            self.make_token(TokenKind::PowEq)
          } else {
            self.make_token(TokenKind::Pow)
          }
        } else if self.match_char('=') {
          self.make_token(TokenKind::MultiplyEq)
        } else {
          self.make_token(TokenKind::Multiply)
        }
      },
      '/' => {
        if self.match_char('/') {
          if self.match_char('=') {
            self.make_token(TokenKind::FloorEq)
          } else {
            self.make_token(TokenKind::Floor)
          }
        } else if self.match_char('=') {
          self.make_token(TokenKind::DivideEq)
        } else {
          self.make_token(TokenKind::Divide)
        }
      },
      '\\' => self.make_token(TokenKind::Backslash),
      ':' => self.make_token(TokenKind::Colon),
      '<' => {
        if self.match_char('<') {
          if self.match_char('=') {
            self.make_token(TokenKind::LshiftEq)
          } else {
            self.make_token(TokenKind::Lshift)
          }
        } else if self.match_char('=') {
          self.make_token(TokenKind::LessEq)
        } else {
          self.make_token(TokenKind::Less)
        }
      },
      '>' => {
        if self.lexing_type == 0 && self.match_char('>') {
          if self.match_char('>') {
            if self.match_char('=') {
              self.make_token(TokenKind::UrshiftEq)
            } else {
              self.make_token(TokenKind::Urshift)
            }
          } else {
            if self.match_char('=') {
              self.make_token(TokenKind::RshiftEq)
            } else {
              self.make_token(TokenKind::Rshift)
            }
          }
        } else if self.match_char('=') {
          self.make_token(TokenKind::GreaterEq)
        } else {
          self.make_token(TokenKind::Greater)
        }
      },
      '!' => {
        if self.match_char('=') {
          self.make_token(TokenKind::BangEq)
        } else {
          self.make_token(TokenKind::Bang)
        }
      },
      '=' => {
        if self.match_char('=') {
          self.make_token(TokenKind::EqualEq)
        } else if self.match_char('>') {
          self.make_token(TokenKind::Arrow)
        } else {
          self.make_token(TokenKind::Equal)
        }
      },
      '%' => {
        if self.match_char('=') {
          self.make_token(TokenKind::PercentEq)
        } else {
          self.make_token(TokenKind::Percent)
        }
      },
      '&' => {
        if self.match_char('=') {
          self.make_token(TokenKind::AmpEq)
        } else {
          self.make_token(TokenKind::Amp)
        }
      },
      '|' => {
        if self.match_char('=') {
          self.make_token(TokenKind::BarEq)
        } else {
          self.make_token(TokenKind::Bar)
        }
      },
      '^' => {
        if self.match_char('=') {
          self.make_token(TokenKind::XorEq)
        } else {
          self.make_token(TokenKind::Xor)
        }
      },
      '~' => {
        if self.match_char('=') {
          self.make_token(TokenKind::TildeEq)
        } else {
          self.make_token(TokenKind::Tilde)
        }
      },
      '?' => self.make_token(TokenKind::Question),
      '\n' => self.make_token(TokenKind::Newline),
      '\'' | '"' => self.string(c),
      _ => {
        if is_digit(c) {
          self.number()
        } else if is_alpha(c) {
          self.identifier()
        } else {
          self.make_error(format!("Unexpected character {}", c))
        }
      },
    }
  }

  pub fn start_lexing_type(&mut self) {
    self.lexing_type += 1;
  }

  pub fn stop_lexing_type(&mut self) {
    if self.lexing_type > 0 {
      self.lexing_type -= 1;
    }
  }
}
