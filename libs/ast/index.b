/**
 * @module ast
 *
 * Provides interface for parsing Zuri code into Abstract Syntax Trees.
 * 
 * @copyright 2021, Richard Ore and Zuri contributors
 */

import json as js

import .scanner { Scanner }
import .parser { Parser }
import .expr { * }
import .stmt { * }
import .decl { * }
import .defn { * }
import .exception { * }
import .token { * }


/**
 * Parses a given source code and outputs Zuri AST objects.
 * 
 * @param string source
 * @param string? path
 * @returns [[ast.ParseResult]]
 */
def parse(source, path) {
  if !is_string(source)
    raise TypeError('string expected in argument 1 (source)')
  if path != nil and !is_string(path)
    raise TypeError('string expected in argument 2 (path)')

  # scan the source...
  var scanner = Scanner(source, path)
  var tokens = scanner.scan()
  
  # parse the scanned tokens
  var parser = Parser(tokens, path)
  return parser.parse()
}

/**
 * Parses the given file and outputs Zuri AST objects.
 * 
 * @param {file|string} source
 * @returns [[ast.ParseResult]]
 */
def parse_file(source) {
  if is_string(source) source = file(source)
  if !is_file(source)
    raise TypeError('file expected in argument 1 (source)')

  return parse(source.read(), source.abs_path())
}

/**
 * Parses the give source code and outputs a JSON representation of 
 * it's AST structure.
 * 
 * @param string source
 * @param string? path
 * @returns string
 */
def json(source, path) {
  # return Exception('not yet implemented')
  return js.Encoder(true, 0).encode(parse(source, path))
}