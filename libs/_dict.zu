# -- Extensions for builtin dict object

class _DictExtension > dict {

  /**
   * Iterates over each key-value pair in the dictionary, calling the provided callback function with the
   * value and key as arguments.
   *
   * @param {function} callback - The function to call for each key-value pair.
   * @throws {Exception} If the callback is not a function.
   * @returns {void}
   *
   * Example:
   *
   * ```zuri
   * var myDict = {a: 1, b: 2, c: 3}
   * myDict.each(@(value, key) {
   *   echo '${key}: ${value}'
   * })
   *
   * # Output:
   * # a: 1
   * # b: 2
   * # c: 3
   * ```
   */
  static each(callback) {
    if !is_function(callback) {
      raise Exception('Callback must be a function')
    }

    for key, value in self {
      callback(value, key)
    }
  }

  /**
   * Creates a new dictionary containing only the key-value pairs for which the provided callback function returns true.
   * The callback function is called with the value and key as arguments.
   *
   * @param {function} callback - The function to test each key-value pair. It should return true to keep the pair, or
   *    false to exclude it.
   * @throws {Exception} If the callback is not a function.
   * @returns {dict}
   *
   * Example:
   *
   * ```zuri
   * var myDict = {a: 1, b: 2, c: 3}
   * var filteredDict = myDict.filter(@(value, key) {
   *   return value > 1
   * })
   * echo filteredDict
   *
   * # Output: {'b': 2, 'c': 3}
   * ```
   */
  static filter(callback) {
    if !is_function(callback) {
      raise Exception('Callback must be a function')
    }

    var result = {}

    for key, value in self {
      if callback(value, key) {
        result.set(key, value)
      }
    }

    return result
  }

  /**
   * Tests whether at least one key-value pair in the dictionary passes the test implemented by the provided callback function.
   * The callback function is called with the value and key as arguments. The method returns true if the callback returns true
   * for any key-value pair, otherwise it returns false.
   *
   * @param {function} callback - The function to test each key-value pair. It should return true to indicate a passing pair, or
   *    false to indicate a failing pair.
   * @throws {Exception} If the callback is not a function.
   * @returns {boolean}
   *
   * Example:
   *
   * ```zuri
   * var myDict = {a: 1, b: 2, c: 3}
   * var hasGreaterThanTwo = myDict.some(@(value, key) {
   *   return value > 2
   * })
   * echo hasGreaterThanTwo
   *
   * # Output: true
   * ```
   */
  static some(callback) {
    if !is_function(callback) {
      raise Exception('Callback must be a function')
    }

    for key, value in self {
      if callback(value, key) {
        return true
      }
    }

    return false
  }

  /**
   * Tests whether all key-value pairs in the dictionary pass the test implemented by the provided callback function.
   * The callback function is called with the value and key as arguments. The method returns true if the callback returns
   * true for every key-value pair, otherwise it returns false.
   *
   * @param {function} callback - The function to test each key-value pair. It should return true to indicate a passing pair, or
   *    false to indicate a failing pair.
   * @throws {Exception} If the callback is not a function.
   * @returns {boolean}
   *
   * Example:
   *
   * ```zuri
   * var myDict = {a: 1, b: 2, c: 3}
   * var allGreaterThanZero = myDict.every(@(value, key) {
   *   return value > 0
   * })
   * echo allGreaterThanZero
   *
   * # Output: true
   * ```
   */
  static every(callback) {
    if !is_function(callback) {
      raise Exception('Callback must be a function')
    }

    for key, value in self {
      if !callback(value, key) {
        return false
      }
    }

    return true
  }

  /**'
   * Reduces the dictionary to a single value by iteratively combining each key-value pair using the provided callback function.
   * The callback function is called with the accumulator, value, key, and the dictionary itself as arguments. The method returns
   * the final accumulated value after processing all key-value pairs in the dictionary.
   *
   * @param {function} callback - The function to execute on each key-value pair in the dictionary. It should return the updated
   *    accumulator value after processing the pair.
   * @param {any} initial - The initial value to use as the first argument to the first call of the callback function.
   * @throws {Exception} If the callback is not a function.
   * @returns {any}
   *
   * Example:
   *
   * ```zuri
   * var myDict = {a: 1, b: 2, c: 3}
   * var sum = myDict.reduce(@(accumulator, value, key) {
   *   return accumulator + value
   * }, 0)
   * echo sum
   *
   * # Output: 6
   * ```
   */
  static reduce(callback, initial) {
    if !is_function(callback) {
      raise Exception('Callback must be a function')
    }

    var accumulator = initial

    for key, value in self {

      # Never pass nil keys to the callback, as they are not enumerable
      if key != nil {
        accumulator = callback(accumulator, value, key, self)
      }
    }

    return accumulator
  }
}
