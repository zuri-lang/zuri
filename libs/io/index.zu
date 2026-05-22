/**
 * @module io
 * 
 * This module provides interfaces for working with to I/O stream and TTYs 
 * as well as expose the operating system standard I/O for easy access.
 * 
 * Some I/O operations that should belong to this module have been merged as 
 * core features and offered as built-in functions for Zuri. Specifically
 * file I/O features that can be accessed via the built-in `file()` function. 
 * 
 * The standard I/O streams are also files and you can call almost all file 
 * methods on them. Whenever a file method is not supported, you'll get an error 
 * message telling you that such operation is not supported for standard streams.
 * 
 * ### Example
 * 
 * The following example shows how to use the `io` module for accepting user name 
 * and printing the result.
 * 
 * ```zuri
 * import io
 * 
 * var name = io.readline('What is your name?')
 * echo name
 * ```
 * 
 * @copyright 2021, Richard Ore and Zuri contributors
 */

import _io
import _os

import .tty { TTY }
import .bytesio { BytesIO }

/**
 * Set I/O position from the beginning.
 * 
 * @type int
 */
var SEEK_SET = 0

/**
 * Set I/O position from the current position.
 * 
 * @type int
 */
var SEEK_CUR = 1

/**
 * Set I/O position from the end.
 * 
 * @type int
 */
var SEEK_END = 2


/** 
 * Stdin is a file handle to the standard input file of the system.
 * @type file
 */
var stdin = _io.stdin

/**
 * Stdout is a file handle to the standard output file of the system.
 * @type file
 */
var stdout = _io.stdout

/**
 * Stderr is a file handle to the standard error file of the system.
 * @type file
 */
var stderr = _io.stderr

/**
 * Flushes the content of the given file handle
 */
def flush(file) {
  _io.flush(file)
}

/**
 * Writes character c to the screen.
 * 
 * @param char|number c
 */
def putc(c) {
  _io.putc(c)
}

/**
 * Reads character(s) from standard input
 *
 * When length is given, gets `length` number of characters
 * else, gets a single character
 * 
 * @returns char|string
 */
def getc() {
  return _io.getc()
}

/**
 * Reads character(s) from standard input without printing to standard output
 *
 * When length is given, gets `length` number of characters
 * else, gets a single character.
 * 
 * @returns char|string
 */
def getch() {
  return _io.getch()
}

/**
 * Reads an entire line from standard input. If a _message_ is given, the
 * message will be printed before it begins to wait for a user input. If 
 * _secure_ is `true`, the user's input will not be printing and _obscure_text_ 
 * will be printed instead.
 * 
 * @note Newlines will not be added automatically for messages.
 * @param string? message
 * @param bool? secure
 * @param string? obscure_text: Default value is `*`.
 * @returns string
 */
def readline(message, secure, obscure_text) {

  if message != nil and !is_string(message)
    raise TypeError('string expected in argument 1 (message)')
  if secure != nil and !is_bool(secure)
    raise TypeError('boolean expected in argument 2 (secure)')
  if obscure_text != nil and !is_string(obscure_text)
    raise TypeError('string expected in argument 3 (obscure_text)')

  if secure == nil secure = false
  if obscure_text == nil obscure_text = '*'

  if message
    stdout.write('${message} ')

  var result = ''
  var input

  if !secure {
    while (input = stdin.read()) and input != '\n' and input != '\0'
      result += input
  } else {
    while (input = getch()) and input != '\n' and input != '\r' and input != '\0' {
    if ord(input) != 0x7f and input != '\b' {
        result += input
        stdout.write(obscure_text)
      } else {
        if result.length() > 0
          stdout.write('\b \b')
        result = result[,result.length() - 1]
      }
    }
  }

  return result
}

