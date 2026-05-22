#!-- part of the ast module

import enum

var TokenType = enum([
  # symbols
  'NEWLINE',  # newline token
  'LPAREN',  # left parenthesis (`(`) token
  'RPAREN',  # right parenthesis (`)`) token
  'LBRACKET',  # left bracket (`[`) token
  'RBRACKET',  # right bracket (`]`) token
  'LBRACE',  # left brace (`{`) token
  'RBRACE',  # right brace (`}`) token
  'SEMICOLON',  # semicolon (`;`) token
  'COMMA',  # comma (`,`) token
  'BACKSLASH',  # backslash (`\`) token
  'BANG',  # not (`!`) token
  'BANG_EQ',  # not equal (`!=`) token
  'COLON',  # colon (`:`) token
  'AT',  # at (`@`) token
  'DOT',  # dot (`.`) token
  'RANGE',  # range (`..`) token
  'TRI_DOT',  # tri-dot (`...`) token
  'PLUS',  # plus (`+`) token
  'PLUS_EQ',  # plus equal (`+=`) token
  'INCREMENT',  # increment (`++`) token
  'MINUS',  # minus (`-`) token
  'MINUS_EQ',  # minus equal (`-=`) token
  'DECREMENT',  # decrement (`--`) token
  'MULTIPLY',  # multiply (`*`) token
  'MULTIPLY_EQ',  # multiply equal (`*=`) token
  'POW',  # pow (`**`) token
  'POW_EQ',  # pow equal (`**=`) token
  'DIVIDE',  # divide (`/`) token
  'DIVIDE_EQ',  # divide equal (`/=`) token
  'FLOOR',  # floor division (`//`) token
  'FLOOR_EQ',  # floor divide equal (`//=`) token
  'EQUAL',  # assignment (`=`) token
  'EQUAL_EQ',  # equality (`==`) token
  'LESS',  # less than (`<`) token
  'LESS_EQ',  # less than or equal (`<=`) token
  'LSHIFT',  # left shift (`<<`) token
  'LSHIFT_EQ',  # left shift equal (`<<=`) token
  'GREATER',  # greater than (`>`) token
  'GREATER_EQ',  # greater than or equal (`>=`) token
  'RSHIFT',  # right shift (`>>`) token
  'RSHIFT_EQ',  # right shift equal (`>>=`) token
  'URSHIFT',  # unsigned right shift (`>>>`) token
  'URSHIFT_EQ',  # unsigned right shift equal (`>>>=`) token
  'PERCENT',  # modulos (`%`) token
  'PERCENT_EQ',  # modulos equal (`%=`) token
  'AMP',  # ampersand (`&`) token
  'AMP_EQ',  # and equal (`&=`) token
  'BAR',  # bar (`|`) token
  'BAR_EQ',  # bar equal (`|=`) token
  'TILDE',  # tilde/not (`~`) token
  'TILDE_EQ',  # tilde equal (`~=`) token
  'XOR',  # exclusive or (`^`) token
  'XOR_EQ',  # exclusive or equal (`^=`) token
  'QUESTION',  # question (`?`) token

  # keywords
  'AND',  # and token
  'AS',  # as token
  'ASSERT',  # assert token
  'BREAK',  # break token
  'CATCH',  # catch token
  'CLASS',  # class token
  'CONTINUE',  # continue token
  'DEF',  # def token
  'DEFAULT',  # default token
  'RAISE',  # raise token
  'DO',  # do token
  'ECHO',  # echo token
  'ELSE',  # else token
  'FALSE',  # false token
  'FOR',  # for token
  'IF',  # if token
  'IMPORT',  # import token
  'IN',  # in token
  'ITER',  # iter token
  'NIL',  # nil token
  'OR',  # or token
  'PARENT',  # parent token
  'RETURN',  # return token
  'SELF',  # self token
  'STATIC',  # static token
  'TRUE',  # true token
  'USING',  # using token
  'VAR',  # var token
  'WHEN',  # when token
  'WHILE',  # while token

  # types token
  'LITERAL',  # string literal token
  'REG_NUMBER',  # regular number token
  'BIN_NUMBER',  # binary number token
  'OCT_NUMBER',  # octal number token
  'HEX_NUMBER',  # hexadecimal number token
  'IDENTIFIER',  # identifier token
  'DECORATOR',  # decorator token
  'INTERPOLATION',  # interpolation token

  # comments
  'COMMENT',  # comment token
  'DOC',  # doc block token

  #  * end of file
  'EOF',  # eof token

  # errors
  'ERROR',  # error token
  'EMPTY',  # empty token
])


/**
 * Zuri source code token.
 * 
 * @serializable
 * @printable
 */
class Token {
  /**
   * @param number type
   * @param string literal
   * @param number line
   * @constructor 
   */
  Token(type, literal, line, file) {
    self.type = type
    self.literal = literal
    self.line = line
    self.file = file
  }

  @to_string() {
    return "<ast::Token type=${self.type} literal='${self.literal}' line=${self.line} file='${self.file}'>"
  }

  @to_json() {
    return {
      type: self.type,
      literal: self.literal,
      line: self.line,
      file: self.file,
    }
  }
}
