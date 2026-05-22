/**
 * @module base64
 *
 * This module provides interface for encoding binary data into strings and 
 * decoding such encoded strings back into binary data based on the base64 
 * encoding specified in [RFC4648](https://datatracker.ietf.org/doc/html/rfc4648)
 * 
 * @copyright 2021, Richard Ore and Zuri contributors
 */

import _base64

/**
 * Encodes a byte array into a base64 string
 * 
 * @param bytes data
 * @returns string
 */
def encode(data) {
  return _base64.encode(data)
}

/**
 * Decodes a base64 string into it's corresponding bytes.
 * 
 * @param string data
 * @returns bytes
 */
def decode(data) {
  return _base64.decode(data)
}
