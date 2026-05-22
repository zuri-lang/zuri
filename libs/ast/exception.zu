#!-- part of the ast module

/**
 * Exception raised for errors during parsing.
 */
class ParseException < Exception {
  /**
   * @param string message
   * @param Token token
   * @constructor 
   */
  ParseException(message, token) {
    parent('Error at ${token.literal} on line ${token.line} in ${token.file}: ${message}')
  }
}
