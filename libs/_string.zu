# -- Extensions for builtin string object

class _StringExtension > string {

  /**
   * Returns true if the string contains the specified substring, false otherwise.
   *
   * @param {string} str The substring to search for.
   * @return {boolean}
   */
  static contains(str) {
    return self.index_of(str) > -1
  }

  /**
   * Returns the lines of the string as an list as it would be if split on newline characters.
   *
   * @return {list}
   */
  static lines() {
    return self.split('\n')
  }

  /**
   * Iterates over each line of the string, calling the provided callback function with the line and
   * its index.
   *
   * @param {function} callback A function that takes two arguments: the line and its index.
   * @throws {Exception} if the callback is not a function.
   * @return void
   */
  static each_line(callback) {
    if !is_function(callback) {
      raise Exception('Callback must be a function')
    }

    for index, line in self.lines() {
      callback(line, index)
    }
  }

  /**
   * Iterates over each character of the string, calling the provided callback function with the character
   * and its index.
   *
   * @param {function} callback A function that takes two arguments: the character and its index.
   * @throws {Exception} if the callback is not a function.
   * @return {void}
   */
  static each(callback) {
    if !is_function(callback) {
      raise Exception('Callback must be a function')
    }

    iter var i = 0; i < self.length(); i++ {
      callback(self[i], i)
    }
  }
}
