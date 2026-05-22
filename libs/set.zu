/**
 * @module set
 * 
 * This module provides functionalities for working with mathematical sets. 
 * 
 * Sets are collections of unique values. You can iterate through sets in the 
 * same order in which they were initialized or set by [[set.Set.add()]]. 
 * This module assumes that all Sets containing exactly the same elements 
 * irrespective of their order are equal.
 * 
 * The example below shows a brief introduction to working with sets.
 * 
 * ```zuri
 * import set
 * 
 * var my_set = set()
 * 
 * my_set.add(11) # <Set(1) {1}>
 * my_set.add(56) # <Set(2) {1, 5}>
 * my_set.add(97) # <Set(2) {1, 5}>
 * my_set.add('some text') # <Set(3) {1, 5, 'some text'}>
 * var o = { a: 1, b: 2 }
 * my_set.add(o)
 * 
 * # o is referencing a different object, but contains the same value
 * # so it will not be added
 * my_set.add({ a: 1, b: 2 })
 * 
 * my_set.contains(11) # true
 * my_set.contains(32) # false, since 32 has not been added to the set
 * my_set.contains(97) # true
 * my_set.contains(121 ** 0.5) # true
 * my_set.contains('Some Text'.lower()) # true
 * my_set.contains(o) # true
 * ```
 * 
 * Sets objects can also be created with initial values (or their actual values) 
 * by passing a list or a dictionary to the constructor. If a list is passed, all 
 * unique elements in the list will be added to the Set object. 
 * 
 * For example,
 * 
 * ```zuri
 * import set
 * 
 * var set_a = set([1, 2, 3, 4, 5])
 * var set_b = set([4, 5, 6, 7, 8])
 * 
 * echo set_a.intersect(set_b) # <Set(2) {4, 5}>
 * ```
 * 
 * When a dictionary is passed to the Set constructor, the dictionary keys which are 
 * unique themselves will be added to the Set object.
 * 
 * For example:
 * 
 * ```zuri
 * import set
 * 
 * var set_a = set({
 *   a: 10,
 *   b: 21,
 * })
 * 
 * echo set_a # <Set(2) {a, b}>
 * ```
 * 
 * @copyright Richard Ore, 2025
 */

import _reflect


/**
 * The Set class provides some methods that allow you to compose sets like 
 * you would with mathematical operations.
 * 
 * @printable
 * @serializable
 * @iterable
 * @numeric
 */
class Set {

  var _items = []

  /**
   * Creates a new Set object from a list or dictionary or an empty Set object 
   * when no argument is passed.
   * 
   * @param {list|dict|nil} items
   * @constructor
   */
  Set(items) {
    if items != nil {
      if !is_list(items) and !is_dict(items) {
        raise TypeError('expected list or dictionary, ${typeof(items)} given')
      }
  
      if is_dict(items) {
        items = items.keys()
      }
  
      # only keep unique items
      self._items = items.unique()
    }
  }

  /**
   * Returns a new set containing elements which are in either or both of this 
   * set and the given set.
   * 
   * @param [[set.Set]] other
   * @returns [[set.Set]]
   */
  union(other) {
    if !instance_of(other, Set) {
      raise TypeError('instance of Set expected, ${typeof(other)} given')
    }

    var items = self.to_list()
    items.extend(other.to_list())

    return Set(items)
  }

  /**
   * Returns a new set containing elements in both this set and the given set.
   * 
   * @param [[set.Set]] other
   * @returns [[set.Set]]
   */
  intersect(other) {
    if !instance_of(other, Set) {
      raise TypeError('instance of Set expected, ${typeof(other)} given')
    }

    var common = []

    if !other.is_empty() {
      for item in self._items {
        if other.contains(item) {
          common.append(item)
        }
      }
    }

    return Set(common)
  }

  /**
   * Returns a new set containing elements in this set but not in the given set.
   * 
   * @param [[set.Set]] other
   * @returns [[set.Set]]
   */
  difference(other) {
    if !instance_of(other, Set) {
      raise TypeError('instance of Set expected, ${typeof(other)} given')
    }

    var diff = []

    if other.is_empty() {
      diff.extend(self._items)
    } else {
      for item in self._items {
        if !other.contains(item) {
          diff.append(item)
        }
      }
    }

    return Set(diff)
  }

  /**
   * Returns a new set containing elements which are in either this set or 
   * the given set, but not in both.
   * 
   * @param [[set.Set]] other
   * @returns [[set.Set]]
   */
  symmetric_difference(other) {
    if !instance_of(other, Set) {
      raise TypeError('instance of Set expected, ${typeof(other)} given')
    }

    var diff = []

    if self.is_empty() {
      diff.extend(other.to_list())
    } else if other.is_empty() {
      diff.extend(self._items)
    } else {
      for item in self._items {
        if !other.contains(item) {
          diff.append(item)
        }
      }

      for item in other {
        if !self.contains(item) {
          diff.append(item)
        }
      }
    }

    return Set(diff)
  }

  /**
   * Returns a boolean indicating if this set has no elements in common with 
   * the given set.
   * 
   * @param [[set.Set]] other
   * @returns bool
   */
  is_disjoint(other) {
    if !instance_of(other, Set) {
      raise TypeError('instance of Set expected, ${typeof(other)} given')
    }

    var intersect = self.intersect(other)
    var is_disjoint = intersect.is_empty()

    # free resource
    intersect.clear()

    return is_disjoint
  }

  /**
   * Returns a boolean indicating if all elements of this set are in the given set.
   * 
   * @param [[set.Set]] other
   * @returns bool
   */
  is_subset(other) {
    if !instance_of(other, Set) {
      raise TypeError('instance of Set expected, ${typeof(other)} given')
    }

    # important early exit
    if self.length() <= other.length() {
      return false
    }

    for item in other {
      if !self.contains(item) {
        return false
      }
    }

    return true
  }

  /**
   * Returns a boolean indicating if all elements of the given set are in this set.
   * 
   * @param [[set.Set]] other
   * @returns bool
   */
  is_superset(other) {
    if !instance_of(other, Set) {
      raise TypeError('instance of Set expected, ${typeof(other)} given')
    }

    # important early exit
    if self.length() >= other.length() {
      return false
    }

    for item in self {
      if !other.contains(item) {
        return false
      }
    }

    return true
  }

  /**
   * Returns a boolean value indicating whether this set is an empty set or not.
   * 
   * @returns bool
   */
  is_empty() {
    return self._items.is_empty()
  }

  /**
   * Returns a boolean asserting whether an element is present with the 
   * given value in the Set or not.
   * 
   * @param any value
   * @returns bool
   */
  contains(value) {
    return self._items.contains(value)
  }

  /**
   * Returns the number of values in the Set object.
   * 
   * @returns number
   */
  length() {
    return self._items.length()
  }

  /**
   * Removes all elements from the Set object.
   * 
   * @returns bool
   */
  clear() {
    return self._items.clear()
  }

  /**
   * Removes the element associated to the `value` and returns a boolean 
   * asserting whether an element was successfully removed or not. Once an 
   * element is removed, calling `set.contains(value)` will return false 
   * afterwards.
   * 
   * @param any value
   * @returns any
   */
  remove(value) {
    return self._items.remove(value)
  }

  /**
   * Inserts a new element with the specified value in to the Set object, if 
   * there isn't an element with the same value already in the Set.
   * 
   * @param any value
   * @returns bool
   */
  add(value) {
    if !self._items.contains(value) {
      self._items.append(value)
      return true
    }

    return false
  }

  /**
   * Returns a new Set which is an exact replica of the current Set.
   * 
   * @returns [[set.Set]]
   */
  clone() {
    return Set(self.to_list())
  }

  /**
   * Calls function `callback` once for each value present in the Set, in 
   * insertion order.
   * 
   * @param function callback
   */
  each(callback) {
    if !is_function(callback) {
      raise ArgumentError('callback function expected, ${typeof(callback)} given')
    }

    var fn_meta = _reflect.getfunctionmetadata(function)

    if fn_meta.arity > 1 {
      for index, item in self._items {
        callback(item, index)
      }
    } else {
      for item in self._items {
        callback(item)
      }
    }
  }

  /**
   * Returns a string that represents the current Set.
   * 
   * @returns string
   */
  to_string() {
    return '<Set(${self.length()}) {' +
      ', '.join(self._items) +
    '}>'
  }

  /**
   * Returns the current Set as a list of elements.
   * 
   * @returns list
   */
  to_list() {
    return self._items.clone()
  }

  @to_string() {
    return self.to_string()
  }

  @to_list() {
    return self.to_list()
  }

  @to_json() {
    return self.to_list()
  }

  @itern(index) {
    if index == nil return 0
    if !is_number(index)
      raise ArgumentError('sets are numerically indexed')
    if index < self._items.length() - 1 return index + 1
    return nil
  }

  @iter(index) {
    return self._items[index]
  }

  def + {
    if !instance_of(__arg__, Set) {
      raise NumericError('operation + not defined for Set and ${typeof(__arg__)}')
    }

    return self.union(__arg__)
  }

  def - {
    if !instance_of(__arg__, Set) {
      raise NumericError('operation - not defined for Set and ${typeof(__arg__)}')
    }

    return self.intersect(__arg__)
  }

  def = {
    if !instance_of(__arg__, Set) {
      raise NumericError('operation = not defined for Set and ${typeof(__arg__)}')
    }

    if self.length() != __arg__.length() {
      return false
    }

    for item in __arg__ {
      if !self.contains(item) {
        return false
      }
    }

    return true
  }
}


/**
 * Default export function for the [[set.Set]] class.
 * 
 * @param {list|dict|nil} items
 * @returns [[set.Set]]
 * @default
 */
def set(items) {
  return Set(items)
}
