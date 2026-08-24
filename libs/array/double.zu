import _struct
import ._base { Array }


/**
 * Maximum value that "should" exist in a list passed to DoubleArray.
 * @type number
 */
var DOUBLE_MAX = 1.79769313486231570815E+308

/**
 * Minimum value that "should" exist in a list passed to DoubleArray.
 * @type number
 */
var DOUBLE_MIN = 2.22507385850720138309E-308


/**
 * class DoubleArray represents an array of twos-complement 16-bit signed 
 * integers in the platform byte order.
 * 
 * @printable
 * @iterable
 * @serializable
 */
class DoubleArray < Array {

  var _data
  var _bit_size = 8
  var _data_type = 'd'

  /**
   * - If n is a number, it creates a new DoubleArray that can hold up to n 
   * number of elements, but with all the elements set to 0. 
   * - If n is a list, it creates a new DoubleArray with its elements set to 
   * the values in the list.
   * 
   * @param {number|list} n
   * @constructor
   */
  DoubleArray(n) {
    if is_number(n) {
      self._data = bytes(n * self._bit_size)
    } else if is_list(n) {
      # validate
      for item in n {
        if !is_number(item) {
          raise ValueError('invalid DoubleArray value')
        }
      }

      self._data = _struct.pack('${self._data_type}${n.length()}', n)
    } else {
      raise TypeError('number or list expected, ${typeof(n)} given')
    }
  }

  /**
   * Adds the given _value_ to the end of the array.
   * 
   * @param int value
   */
  append(value) {
    if !is_number(value)
      raise TypeError('number expected')
    if value < DOUBLE_MIN or value > DOUBLE_MAX
      raise ValueError('value out of float range')

    var as_bytes = _struct.pack(self._data_type, [value])
    self._data.extend(as_bytes)
    as_bytes.dispose()
  }

  _do_set(start, as_bytes) {
    self._data[start] = as_bytes[0]
    self._data[start + 1] = as_bytes[1]
    self._data[start + 2] = as_bytes[2]
    self._data[start + 3] = as_bytes[3]
    self._data[start + 4] = as_bytes[4]
    self._data[start + 5] = as_bytes[5]
    self._data[start + 6] = as_bytes[6]
    self._data[start + 7] = as_bytes[7]
  }

  /**
   * Sets the value at the given index.
   * 
   * @param number index
   * @param number value
   * @returns number
   */
  set(index, value) {
    if !is_number(index)
      raise ArgumentError('Arrays are numerically indexed')
    if !is_number(value)
      raise ArgumentError('DoubleArray stores numerical values')

    var as_bytes = _struct.pack(self._data_type, [value])
    var start = index * self._bit_size
    
    if self.length() <= index {
      var diff = index - self.length() + 1 # +1 for the new data

      var extension = bytes(diff * self._bit_size)
      self._data.extend(extension)
      extension.dispose()
    }

    self._do_set(start, as_bytes)
    as_bytes.dispose()

    return value
  }

  /**
   * Updates the content of the current array by appending all the contents 
   * of _array_ to the end of the array in exact order.
   * 
   * @param DoubleArray array
   */
  extend(array) {
    if !instance_of(array, DoubleArray)
      raise TypeError('instance of DoubleArray expected')
    self._data.extend(array.to_bytes())
  }

  /**
   * Returns a new array containing the elements in the original array 
   * in reverse order.
   * 
   * @returns [[array.DoubleArray]]
   */
  reverse() {
    var data = self.to_list().reverse()
    return DoubleArray(data)
  }

  /**
   * Returns a new DoubleArray containing all items from the current array. 
   * The new array is a shallow copy of the original array.
   * 
   * @returns DoubleArray
   */
  clone() {
    return DoubleArray(self.to_list())
  }
}
