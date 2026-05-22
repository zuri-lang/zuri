# -- Extensions for builtin range object

class _RangeExtension > range {

  /**
   * Iterates over each number in the range, calling the provided callback function with the number, its index.
   *
   * @param {function} callback A function that takes two arguments: the number, its index.
   * @throws {Exception} if the callback is not a function.
   * @return void
   * 
   * Example:
   * 
   * ```zuri
   * var r = 0..5 # 0, 1, 2, 3, 4
   * r.each(@(num, index) => {
   *   print(`Number at index ${index}: ${num}`)
   * })
   * 
   * # Output:
   * # Number at index 0: 0
   * # Number at index 1: 1
   * # Number at index 2: 2
   * # Number at index 3: 3
   * # Number at index 4: 4
   * ```
   */
  static loop(callback) {
    if !is_function(callback) {
      raise Exception('Callback must be a function')
    }

    var lower = self.lower()
    var upper = self.upper()

    # if lower == upper {
    #   callback(lower, 0, self)
    #   return
    # }

    var step = self.get_step()

    if lower < upper {
      iter var i = lower, index = 0; i < upper; i += step, index++ {
        callback(i, index)
      }
    } else {
      iter var i = lower, index = 0; i > upper; i -= step, index++ {
        callback(i, index)
      }
    }
  }
}
