/** 
 * @module type
 * 
 * Provides type validation and conversion capabilities
 * 
 * This module is wrapper around the builtin functions
 * where applicable and does and return the same thing as the builtin
 * alternative.
 * 
 * @copyright 2021, Richard Ore and Zuri contributors
 */


/**
 * Returns the name of the type of value
 * 
 * @note method implemented as part of core language features
 * 
 * @param any value
 * @returns string
 */
def of(value) { 
  return typeof(value)
}

/**
 * Returns true if the value is a character and digit,
 * otherwise returns false.
 * 
 * @param char value
 * @returns bool
 */
def digit(value) {
  if !is_string(value) or !value.length() == 1
    raise ValueError('char expected')
  var _ = ord(value)
  return _ >= 48 and _ <= 57
}

/**
 * Returns true if the value is a character and alphabetic,
 * otherwise returns false.
 * 
 * @param char value
 * @returns bool
 */
def alpha(value) {
  if !is_string(value) or !value.length() == 1
    raise ValueError('char expected')
  var _ = ord(value)
  return (_ >= 65 and _ <= 90) or (_ >= 97 and _ <= 122)
}

/**
 * Returns true if the value is an integer or false otherwise.
 * 
 * @param any value
 * @returns bool
 */
def int(value) {
  return is_int(value)
}

/**
 * Returns true if the value is a boolean or false otherwise.
 * 
 * @param any value
 * @returns bool
 */
def bool(value) {
  return is_bool(value)
}

/**
 * Returns true if the value is a number or false otherwise.
 * 
 * @note this method also returns true for integers.
 * 
 * @param any value
 * @returns bool
 */
def number(value) {
  return is_number(value)
}

/**
 * Returns true if the value is a single character or false otherwise.
 * 
 * @param any value
 * @returns bool
 */
def char(value) {
  return is_string(value) and value.length() == 1
}

/**
 * Returns true if the value is a string or false otherwise.
 * 
 * @param any value
 * @returns bool
 */
def string(value) {
  return is_string(value)
}

/**
 * Returns true if the value is a bytes or false otherwise.
 * 
 * @param any value
 * @returns bool
 */
def bytes(value) {
  return is_bytes(value)
}

/**
 * Returns true if the value is a list or false otherwise.
 * 
 * @param any value
 * @returns bool
 */
def list(value) {
  return is_list(value)
}

/**
 * Returns true if the value is a dictionary or false otherwise.
 * 
 * @param any value
 * @returns bool
 */
def dict(value) {
  return is_dict(value)
}

/**
 * Returns true if the value is an object or false otherwise.
 * 
 * @param any value
 * @returns bool
 */
def object(value) {
  return is_object(value)
}

/**
 * Returns true if the value is a function or false otherwise.
 * 
 * @param any value
 * @returns bool
 */
def function(value) {
  return is_function(value)
}

/**
 * Returns true if the value is a class or false otherwise.
 * 
 * @param any value
 * @returns bool
 */
def is_a_class(value) {
  return is_class(value)
}

/**
 * Returns true if the value is a file or false otherwise.
 * 
 * @param any value
 * @returns bool
 */
def file(value) {
  return is_file(value)
}

/**
 * Returns true if the value is an iterable or false otherwise.
 * 
 * @param any value
 * @returns bool
 */
def iterable(value) {
  return is_iterable(value)
}

/**
 * Returns true if the value is a callable function or class and false 
 * otherwise.
 * 
 * @param any value
 * @returns bool
 */
def callable(value) {
  return is_callable(value)
}

/**
 * Returns true if the value is an instance the given class, false
 * otherwise.
 * 
 * @param any value
 * @param class type
 * @returns bool
 */
def instance(value, type) {
  if !is_class(type)
    raise ValueError('class expected')
  return instance_of(value, type)
}


/**
 * The Convert class handles conversion from one type to another
 */
class Convert {

  /**
   * @param any value
   * @constructor 
   */
  Convert(value) {
    self.value = value
  }

  /**
   * Converts the value into an integer.
   * 
   * @note classes may override the return value by declaring a `to_int()` function.
   * @returns bool
   */
  to_int() {
    return to_int(self.value)
  }

  /**
   * Converts the value into a number.
   * 
   * @note classes may override the return value by declaring a `to_number()` function.
   * @returns bool
   */
  to_number() {
    return to_number(self.value)
  }

  /**
   * Converts the value into a string.
   * 
   * @note classes may override the return value by declaring a `to_string()` function.
   * @returns bool
   */
  to_string() {
    return to_string(self.value)
  }

  /**
   * Converts the value into a boolean.
   * 
   * @note classes may override the return value by declaring a `to_bool()` function.
   * @returns bool
   */
  to_bool() {
    return to_bool(self.value)
  }

  /**
   * Converts the value into a list.
   * 
   * @note classes may override the return value by declaring a `to_list()` function.
   * @returns bool
   */
  to_list() {
    return to_list(self.value)
  }

  /**
   * Converts the value value into a dictionary.
   * 
   * @note classes may override the return value by declaring a `to_dict()` function.
   * @returns bool
   */
  to_dict() {
    return to_dict(self.value)
  }
}

