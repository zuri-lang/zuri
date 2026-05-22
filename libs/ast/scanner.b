#!-- part of the ast module

import .token { * }

/**
 * Zuri source code scanner.
 * 
 * @printable
 */
class Scanner {
  /**
   * Reports if an error was encountered in the scaner.
   * 
   * @type bool
   * @readonly
   */
  var has_error = false

  /**
   * The string to being scanned.
   * 
   * @type string
   * @readonly
   */
  var source

  /**
   * a list of tokens processed by this scanner
   */
  var _tokens = []

  /**
   * scanner location indicators
   */
  var _line = 1
  var _current = 0
  var _start = 0


  /**
   * interpolation tracking
   */
  var _max_interpolation_nest = 8

  /**
   * a keyword to token map
   */
  var _keywords = {
    'and': TokenType.AND,
    'as': TokenType.AS,
    'assert': TokenType.ASSERT,
    'break': TokenType.BREAK,
    'catch': TokenType.CATCH,
    'class': TokenType.CLASS,
    'continue': TokenType.CONTINUE,
    'def': TokenType.DEF,
    'default': TokenType.DEFAULT,
    'do': TokenType.DO,
    'echo': TokenType.ECHO,
    'else': TokenType.ELSE,
    'false': TokenType.FALSE,
    'for': TokenType.FOR,
    'if': TokenType.IF,
    'import': TokenType.IMPORT,
    'in': TokenType.IN,
    'iter': TokenType.ITER,
    'nil': TokenType.NIL,
    'or': TokenType.OR,
    'parent': TokenType.PARENT,
    'raise': TokenType.RAISE,
    'return': TokenType.RETURN,
    'self': TokenType.SELF,
    'static': TokenType.STATIC,
    'true': TokenType.TRUE,
    'using': TokenType.USING,
    'var': TokenType.VAR,
    'when': TokenType.WHEN,
    'while': TokenType.WHILE
  }

  /**
   * @param string source
   * @constructor 
   */
  Scanner(source, file) {
    if !is_string(source)
      raise TypeError('string expected for Zuri source code expected')
    
    # to track the quote that started an interpolation
    self._interpolating = []
    self._file = file
    
    self.source = source
  }

  /**
   * checks to see if we have reached the end of the source
   */
  _is_at_end() {
    return self._current >= self.source.length()
  }

  _is_digit(c) {
    if !c return false
    var _ = ord(c)
    return _ >= 48 and _ <= 57  # 0 - 9
  }

  _is_alpha(c) {
    if !c return false
    var _ = ord(c)
    return (_ >= 97 and _ <= 122) or  # a - z
          (_ >= 65 and _ <= 90) or _ == 95  # A - Z or _
  }

  _is_alphanumeric(c) {
    return self._is_alpha(c) or self._is_digit(c)
  }

  _is_binary(c) {
    if !c return false
    return c == '0' or c == '1'
  }

  _is_octal(c) {
    if !c return false
    var _ = ord(c)
    return _ >= 48 and _ <= 55  # 0 - 7
  }

  _is_hexadecimal(c) {
    if !c return false
    var _ = ord(c)
    return self._is_digit(c) or  # 0 - 9
        (_ >= 97 and _ <= 102) or # a - f
        (_ >= 65 and _ <= 70)   # A - F
  }

  /**
   * returns the current token while moving the pointer forward
   */
  _advance() {
    var val = self.source[self._current]
    self._current++
    if val == '\n' self._line++
    return val
  }

  /**
   * advances if the current character is the given character
   */
  _match(c) {
    if self._is_at_end() return false
    if self.source[self._current] != c return false
    self._current++
    return true
  }

  /**
   * returns the character at the next position without
   * consuming the current one.
   */
  _next() {
    if self._current + 1 >= self.source.length() return '\0'
    return self.source[self._current + 1]
  }

  /**
   * returns the character at the next position without
   * consuming the current one.
   */
  _previous() {
    if self._current == 0 return '\0'
    return self.source[self._current - 1]
  }

  /**
   * returns the character at the current position without
   * consuming it.
   */
  _peek() {
    if self._is_at_end() return '\0'
    return self.source[self._current]
  }

  /**
   * _add_token(type: number [, literal: string])
   * adds a new token to the list of tokens
   */
  _add_token(type, literal) {
    if !literal {
      literal = self.source[self._start, self._current].trim()
    }
    self._tokens.append(Token(type, literal, self._line, self._file))
  }

  /**
   * skips block comments
   */
  _skip_block_comment() {
    var nesting = 1
    while nesting > 0 {
      if self._is_at_end() return

      # internal comment open
      if self._peek() == '/' and self._next() == '*' {
        self._advance()
        self._advance()
        nesting++
      }

      # comment close
      else if self._peek() == '*' and self._next() == '/' {
        self._advance()
        self._advance()
        nesting--
      }

      else self._advance()
    }
  }

  /**
   * skips whitespace and comments
   */
  _skip_whitespace() {
    while true {
      var c = self._peek()

      using c {

        # whitespace
        when ' ' self._advance()
        when '\r' self._advance()
        when '\t' self._advance()

        # single line comment
        when '#' {
          self._advance()
          while self._peek() != '\n' and !self._is_at_end()
            self._advance()
          self._add_token(TokenType.COMMENT)
          if !self._is_at_end()
            self._advance()
          self._start = self._current
        }

        default return
      }
    }
  }

  /**
   * parses a decorator name
   */
  _decorator() {
    while self._is_alpha(self._peek()) or self._is_digit(self._peek())
      self._advance()
    self._add_token(TokenType.DECORATOR)
  }

  /**
   * parses a string surrounded by the quote c.
   */
  _string(c) {
    while self._peek() != c and !self._is_at_end() {
      # interpolation expression
      if self._peek() == '$' and self._next() == '{' and self._previous() != '\\' {
        self._current++
        self._current++

        if self._interpolating.length() < self._max_interpolation_nest {
          self._interpolating.append(c)
          self._add_token(TokenType.INTERPOLATION)
          return
        }

        raise Exception('maximum interpolation nesting exceeded')
      }

      if self._peek() == '\\' and (self._next() == c or self._next() == '\\')
        self._advance()
      
      self._advance()
    }

    if self._is_at_end() 
      raise Exception('unterminated string on line ${self._line}')

    self._match(c)
    self._add_token(TokenType.LITERAL, self.source[self._start, self._current])
  }

  /**
   * parses a valid Zuri number
   */
  _number() {
    if self._previous() == '0' {
      if self._match('b') {   # binary number
        while self._is_binary(self._peek())
          self._advance()

        self._add_token(TokenType.BIN_NUMBER)
        return
      } else if self._match('c') {  # octal number
        while self._is_octal(self._peek())
          self._advance()

        self._add_token(TokenType.OCT_NUMBER)
        return
      } else if self._match('x') {  # hex number
        while self._is_hexadecimal(self._peek())
          self._advance()

        self._add_token(TokenType.HEX_NUMBER)
        return
      }
    }

    while self._is_digit(self._peek())
      self._advance()

    if self._peek() == '.' and self._is_digit(self._next()) {
      self._advance()

      while self._is_digit(self._peek())
        self._advance()

      # E or e are only valid when followed by a digit and
      # occurring after a dot.
      if (self._peek() == 'e' or self._peek() == 'E') and
          (self._next() == '+' or self._next() == '-') {
        
        self._advance()
        self._advance()

        while self._is_digit(self._peek())
          self._advance()
      }
    }

    self._add_token(TokenType.REG_NUMBER)
  }

  /**
   * scans identifiers and keywords
   */
  _identifier() {
    while self._is_alphanumeric(self._peek())
      self._advance()

    var text = self.source[self._start, self._current].trim()
    self._add_token(self._keywords.get(text, TokenType.IDENTIFIER), text)
  }

  /**
   * the private scanning helper
   */
  _scan() {
    if self._is_at_end() {
      return
    }

    var c = self._advance()

    using c {
      when '(' self._add_token(TokenType.LPAREN)
      when ')' self._add_token(TokenType.RPAREN)
      when '[' self._add_token(TokenType.LBRACKET)
      when ']' self._add_token(TokenType.RBRACKET)
      when '{' self._add_token(TokenType.LBRACE)
      when '}' {
        if self._interpolating.length() > 0 {
          self._string(self._interpolating.pop())
        } else {
          self._add_token(TokenType.RBRACE)
        }
      }
      when ',' self._add_token(TokenType.COMMA)
      when ';' self._add_token(TokenType.SEMICOLON)
      when '@' {
        if !self._is_alpha(self._peek()) {
          self._add_token(TokenType.AT)
        } else {
          self._decorator()
        }
      }
      when '.' {
        if self._match('.') {
          self._add_token(self._match('.') ? TokenType.TRI_DOT : TokenType.RANGE)
        } else {
          self._add_token(TokenType.DOT)
        }
      }
      when '-' {
        if self._match('-') {
          self._add_token(TokenType.DECREMENT)
        } else if self._match('=') {
          self._add_token(TokenType.MINUS_EQ)
        } else {
          self._add_token(TokenType.MINUS)
        }
      }
      when '+' {
        if self._match('+') {
          self._add_token(TokenType.INCREMENT)
        } else if self._match('=') {
          self._add_token(TokenType.PLUS_EQ)
        } else {
          self._add_token(TokenType.PLUS)
        }
      }
      when '*' {
        if self._match('*') {
          self._add_token(self._match('=') ? TokenType.POW_EQ : TokenType.POW)
        } else {
          self._add_token(self._match('=') ? TokenType.MULTIPLY_EQ : TokenType.MULTIPLY)
        }
      }
      when '/' {
        if self._match('/') {
          self._add_token(self._match('=') ? TokenType.FLOOR_EQ : TokenType.FLOOR)
        } else if self._match('*') {
          if self._match('*') {
            self._advance()
            self._skip_block_comment()
            self._add_token(TokenType.DOC)
            self._start = self._current
          } else {
            self._advance()
            self._skip_block_comment()
            self._add_token(TokenType.COMMENT)
            self._start = self._current
          }
        } else {
          self._add_token(self._match('=') ? TokenType.DIVIDE_EQ : TokenType.DIVIDE)
        }
      }
      when '\\' self._add_token(TokenType.BACKSLASH)
      when ':' self._add_token(TokenType.COLON)
      when '<' {
        if self._match('<') {
          self._add_token(self._match('=') ? TokenType.LSHIFT_EQ : TokenType.LSHIFT)
        } else {
          self._add_token(self._match('=') ? TokenType.LESS_EQ : TokenType.LESS)
        }
      }
      when '>' {
        if self._match('>') {
          if self._match('>') {
            self._add_token(self._match('=') ? TokenType.URSHIFT_EQ : TokenType.URSHIFT)
          } else {
            self._add_token(self._match('=') ? TokenType.RSHIFT_EQ : TokenType.RSHIFT)
          }
        } else {
          self._add_token(self._match('=') ? TokenType.GREATER_EQ : TokenType.GREATER)
        }
      }
      when '!' self._add_token(self._match('=') ? TokenType.BANG_EQ : TokenType.BANG)
      when '=' self._add_token(self._match('=') ? TokenType.EQUAL_EQ : TokenType.EQUAL)
      when '%' self._add_token(self._match('=') ? TokenType.PERCENT_EQ : TokenType.PERCENT)
      when '&' self._add_token(self._match('=') ? TokenType.AMP_EQ : TokenType.AMP)
      when '|' self._add_token(self._match('=') ? TokenType.BAR_EQ : TokenType.BAR)
      when '^' self._add_token(self._match('=') ? TokenType.XOR_EQ : TokenType.XOR)
      when '~' self._add_token(self._match('=') ? TokenType.TILDE_EQ : TokenType.TILDE)
      when '?' self._add_token(TokenType.QUESTION)

      # newline token
      when '\n' self._add_token(TokenType.NEWLINE)

      when "'" self._string("'")
      when '"' self._string('"')

      default {
        if self._is_digit(c) {
          self._number()
        } else if self._is_alpha(c) {
          self._identifier()
        } else {
          raise Exception("Unexpected character '${c}'")
        }
      }
    }
  }

  /**
   * Scans the source and returns a list of tokens.
   * 
   * @returns list[Token]
   */
  scan() {

    while !self._is_at_end() {
      self._skip_whitespace()
      
      # scan tokens here...
      self._start = self._current
      self._scan()
    }

    self._tokens.append(Token(TokenType.EOF, 'end of file', self._line, self._file))
    return self._tokens
  }

  @to_string() {
    return '<ast::Scanner>'
  }
}

