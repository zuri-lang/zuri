/**
 * @module hash
 *
 * This module provides a framework for cryptographic and non-cryptographic encryption.
 *
 * Examples,
 *
 * ```zuri-repl
 * %> import hash
 * %>
 * %> hash.md5('Hello, World')
 * '82bb413746aee42f89dea2b59614f9ef'
 * %>
 * %> hash.sha256('Hello, World')
 * '03675ac53ff9cd1535ccc7dfcdfa2c458c5218371f418dc136f2d19ac1fbe8a5'
 * %>
 * %> hash.hmac_sha256('mykey', 'Hello, World')
 * '61035d3d2119ffdfd710913bf4161d5fba1c2d9431f7de7ef398d359eb1d2481'
 * %>
 * %> hash.hmac_sha256(bytes([10, 11, 12]), 'My secure text!')
 * 'd782079145a3476fd4e018d44dd024034fa91f626f7f30f2009200c5ac757723'
 * ```
 *
 * @copyright 2021, Richard Ore and Zuri contributors
 */

import _hash
import math
import convert

/**
 * Returns the identification hash of a value as used in the underlying dictionary
 * implementation.
 *
 * A class may override the result of this function by implementing the
 * `to_hash` decorator.
 *
 * @param any value
 * @returns number
 */
def id(value) {
  return _hash.id(value)
}

/**
 * Returns the hash digest for the given data using the given algorithm.
 *
 * Supported algorithms includes:
 *
 * - **FNV1 family**: `fnv1`, `fnv1a`, `fnv164`, `fnv1a64`.
 * - **MD family**: `md2`, `md4`, `md5`.
 * - **SHA family**: `sha`, `sha1`, `sha224`, `sha256`, `sha384`, `sha512`, `sha512-224`, `sha512-256`, `md5-sha1`.
 * - **SHA3 family**: `sha3-224`, `sha3-256`, `sha3-384`, `sha3-512`.
 * - **SHAKE family (XOF)**: `shake128`, `shake256`.
 * - **RIPEMD family**: `ripemd160`.
 * - **WHIRLPOOL family**: `whirlpool`.
 * - **Blake family**: `blake2s256`, `blake2b512`.
 * - **SM family**: `sm3`.
 *
 * By default, this function returns the hexadecimal string representing the hash (since this is the
 * most common application level usage). The function accepts a third boolean argument `as_bytes`
 * which allows callers to specify if the result should be returned in the raw digest byte stream or not.
 *
 * @note Algorithm names are not case-sensitive.
 * @param string algorithm
 * @param {string|bytes} data
 * @param bool? as_bytes
 * @returns {string|bytes}
 */
def hash(algorithm, data, as_bytes) {
  if !is_string(algorithm) {
    raise Exception('algorithm must be string')
  }
  
  if !is_string(data) and !is_bytes(data) {
    raise Exception('data must be of type bytes or string')
  }

  if as_bytes != nil and !is_bool(as_bytes) {
    raise Exception('as_bytes must be of type bool')
  }

  var result

  using algorithm.lower() {
    when 'fnv1' result = fnv1(data, true)
    when 'fnv164' result = fnv1_64(data, true)
    when 'fnv1a' result = fnv1a(data, true)
    when 'fnv1a64' result = fnv1a_64(data, true)
    when 'gost' result = gost(data, as_bytes)
    default result = _hash.hash(algorithm, data)
  }

  if as_bytes {
    return result
  }

  return convert.bytes_to_hex(result)
}


#################################################################################
# MD DEFINITION BEGINS
#################################################################################

/**
 * Returns the md4 hash of the given string or bytes.
 *
 * @param {string|bytes} str
 * @param bool? as_bytes
 * @returns {string|bytes}
 */
def md4(str, as_bytes) {
  return hash('md4', str, as_bytes)
}

/**
 * Returns the md5 hash of the given string or bytes.
 *
 * @param {string|bytes} str
 * @param bool? as_bytes
 * @returns {string|bytes}
 */
def md5(str, as_bytes) {
  return hash('md5', str, as_bytes)
}

/**
 * Returns the md5 hash of the given file.
 *
 * @param file file
 * @param bool? as_bytes
 * @returns {string|bytes}
 */
def md5_file(f, as_bytes) {
  if !is_file(f) {
    raise Exception('file expected')
  }

  if f.mode().contains('b') {
    f = file(f.abs_path(), 'rb')
  }

  return md5(f.read(), as_bytes)
}

#################################################################################
# SHA DEFINITION BEGINS
#################################################################################

/**
 * Returns the sha1 hash of the given string or bytes.
 *
 * @param {string|bytes} str
 * @param bool? as_bytes
 * @returns {string|bytes}
 */
def sha1(str, as_bytes) {
  return hash('sha1', str, as_bytes)
}

/**
 * Returns the sha224 hash of the given string or bytes.
 *
 * @param {string|bytes} str
 * @param bool? as_bytes
 * @returns {string|bytes}
 */
def sha224(str, as_bytes) {
  return hash('sha224', str, as_bytes)
}

/**
 * Returns the sha256 hash of the given string or bytes.
 *
 * @param {string|bytes} str
 * @param bool? as_bytes
 * @returns {string|bytes}
 */
def sha256(str, as_bytes) {
  return hash('sha256', str, as_bytes)
}

/**
 * Returns the sha384 hash of the given string or bytes.
 *
 * @param {string|bytes} str
 * @param bool? as_bytes
 * @returns {string|bytes}
 */
def sha384(str, as_bytes) {
  return hash('sha384', str, as_bytes)
}

/**
 * Returns the sha512 hash of the given string or bytes.
 *
 * @param {string|bytes} str
 * @param bool? as_bytes
 * @returns {string|bytes}
 */
def sha512(str, as_bytes) {
  return hash('sha512', str, as_bytes)
}

#################################################################################
# FNV DEFINITION BEGINS
#################################################################################

/**
 * Returns the 32 bit fnv1 hash of the given string or bytes.
 *
 * @param {string|bytes} data
 * @param bool? as_bytes
 * @returns {string|bytes}
 */
def fnv1(data, as_bytes) {
  if !is_string(data) and !is_bytes(data) {
    raise Exception('data must be of type bytes or string')
  }

  if as_bytes != nil and !is_bool(as_bytes) {
    raise Exception('as_bytes must be of type bool')
  }

  var result = _hash.fnv1(data)

  if as_bytes {
    return result
  }

  return convert.bytes_to_hex(result)
}

/**
 * Returns the 64 bit fnv1 hash of the given string or bytes.
 *
 * @param {string|bytes} data
 * @param bool? as_bytes
 * @returns {string|bytes}
 */
def fnv1_64(data, as_bytes) {
  if !is_string(data) and !is_bytes(data) {
    raise Exception('data must be of type bytes or string')
  }

  if as_bytes != nil and !is_bool(as_bytes) {
    raise Exception('as_bytes must be of type bool')
  }

  var result = _hash.fnv1_64(data)

  if as_bytes {
    return result
  }

  return convert.bytes_to_hex(result)
}

/**
 * Returns the 32 bit fnv1a hash of the given string or bytes.
 *
 * @param {string|bytes} data
 * @param bool? as_bytes
 * @returns {string|bytes}
 */
def fnv1a(data, as_bytes) {
  if !is_string(data) and !is_bytes(data) {
    raise Exception('data must be of type bytes or string')
  }

  if as_bytes != nil and !is_bool(as_bytes) {
    raise Exception('as_bytes must be of type bool')
  }

  var result = _hash.fnv1a(data)

  if as_bytes {
    return result
  }

  return convert.bytes_to_hex(result)
}

/**
 * Returns the 64 bit fnv1a hash of the given string or bytes.
 *
 * @param {string|bytes} data
 * @param bool? as_bytes
 * @returns {string|bytes}
 */
def fnv1a_64(data, as_bytes) {
  if !is_string(data) and !is_bytes(data) {
    raise Exception('data must be of type bytes or string')
  }

  if as_bytes != nil and !is_bool(as_bytes) {
    raise Exception('as_bytes must be of type bool')
  }

  var result = _hash.fnv1a_64(data)

  if as_bytes {
    return result
  }

  return convert.bytes_to_hex(result)
}

#################################################################################
# WHIRLPOOL DEFINITION BEGINS
#################################################################################

/**
 * Returns the whirlpool hash of the given string or bytes.
 *
 * @param {string|bytes} str
 * @param bool? as_bytes
 * @returns {string|bytes} 
 */
def whirlpool(str, as_bytes) {
  return hash('whirlpool', str, as_bytes)
}

#################################################################################
# GOST DEFINITION BEGINS
#################################################################################

/**
 * Returns the Gost cryptographic hash of the given string or bytes.
 *
 * @param {string|bytes} data
 * @param bool? as_bytes
 * @returns {string|bytes} 
 */
def gost(data, as_bytes) {
  if !is_string(data) and !is_bytes(data) {
    raise Exception('data must be of type bytes or string')
  }

  if as_bytes != nil and !is_bool(as_bytes) {
    raise Exception('as_bytes must be of type bool')
  }

  var result = _hash.gost(data)

  if as_bytes {
    return result
  }

  return convert.bytes_to_hex(result)
}

#################################################################################
# SHA3 DEFINITION BEGINS
#################################################################################

/**
 * Returns the SHA3-224 cryptographic hash of the given string or bytes.
 * 
 * @param {string|bytes} data
 * @param bool? as_bytes
 * @returns {string|bytes} 
 */
def sha3_224(data, as_bytes)  {
  return hash('sha3-224', data, as_bytes)
}

/**
 * Returns the SHA3-256 cryptographic hash of the given string or bytes.
 * 
 * @param {string|bytes} data
 * @param bool? as_bytes
 * @returns {string|bytes} 
 */
def sha3_256(data, as_bytes)  {
  return hash('sha3-256', data, as_bytes)
}

/**
 * Returns the SHA3-384 cryptographic hash of the given string or bytes.
 * 
 * @param {string|bytes} data
 * @param bool? as_bytes
 * @returns {string|bytes} 
 */
def sha3_384(data, as_bytes)  {
  return hash('sha3-384', data, as_bytes)
}

/**
 * Returns the SHA3-512 cryptographic hash of the given string or bytes.
 * 
 * @param {string|bytes} data
 * @param bool? as_bytes
 * @returns {string|bytes} 
 */
def sha3_512(data, as_bytes)  {
  return hash('sha3-512', data, as_bytes)
}

#################################################################################
# SHAKE DEFINITION BEGINS
#################################################################################

/**
 * Returns the SHAKE-128 cryptographic hash of the given string or bytes.
 * 
 * @param {string|bytes} data
 * @param bool? as_bytes
 * @returns {string|bytes} 
 */
def shake128(data, as_bytes) {
  return hash('shake128', data, as_bytes)
}

/**
 * Returns the SHAKE-256 cryptographic hash of the given string or bytes.
 * 
 * @param {string|bytes} data
 * @param bool? as_bytes
 * @returns {string|bytes} 
 */
def shake256(data, as_bytes) {
  return hash('shake256', data, as_bytes)
}

#################################################################################
# BLAKE DEFINITION BEGINS
#################################################################################

/**
 * Returns the BLAKE2B-512 cryptographic hash of the given string or bytes.
 *
 * @param {string|bytes} data
 * @param bool? as_bytes
 * @returns {string|bytes}
 */
def blake2b512(data, as_bytes) {
  return hash('blake2b512', data, as_bytes)
}

/**
 * Returns the BLAKE2S-256 cryptographic hash of the given string or bytes.
 *
 * @param {string|bytes} data
 * @param bool? as_bytes
 * @returns {string|bytes}
 */
def blake2s256(data, as_bytes) {
  return hash('blake2s256', data, as_bytes)
}

#################################################################################
# RIPEMD IMPLEMENTATION BEGINS
#################################################################################

/**
 * Returns the RIPEMD-160 cryptographic hash of the given string or bytes.
 *
 * @param {string|bytes} data
 * @param bool? as_bytes
 * @returns {string|bytes}
 */
def ripemd160(data, as_bytes) {
  return hash('ripemd160', data, as_bytes)
}


#################################################################################
# HMAC IMPLEMENTATION BEGINS
#################################################################################

# list of allowed hash functions allowed for computing hmac hash
var _hmac_allowed = [
  md4, md5, sha1,
  sha224, sha256, sha384,
  sha512, whirlpool, gost,
]

/**
 * Computes an HMAC with the key and str using the given method.
 *
 * @param function method
 * @param {string|bytes} key
 * @param {string|bytes} str
 * @param bool? as_bytes
 * @returns {string|bytes} 
 */
def hmac(method, key, str, as_bytes) {
  if !_hmac_allowed.contains(method)
    raise ValueError('invalid HMAC method')

  if !is_string(key) and !is_bytes(key) {
    raise Exception('bytes or string expected in argument 2')
  }

  if !is_string(str) and !is_bytes(str) {
    raise Exception('bytes or string expected in argument 3')
  }

  if as_bytes != nil and !is_bool(as_bytes) {
    raise Exception('bool expected in argument 4')
  }

  # convert key and str to array of bytes.
  if is_string(key) {
    key = key.to_bytes()
  } else {
    key = key.clone()
  }

  if is_string(str) {
    str = str.to_bytes()
  } else {
    str = str.clone()
  }

  var BLOCK_SIZE = 64

  # Keys longer than blockSize are shortened by hashing them
  if key.length() > BLOCK_SIZE {
    # key is outputSize bytes long
    key = method(key, true)
  }

  # Keys shorter than blockSize are padded to blockSize by
  # padding with zeros on the right
  iter var i = key.length(); i < BLOCK_SIZE; i++ {
    key.append(0x00)
  }

  # Outer padded key
  var outer = bytes(BLOCK_SIZE)
  iter var i = 0; i < key.length(); i++ {
    outer[i] = 0x5C ^ key[i]
  }

  iter var i = key.length(); i < BLOCK_SIZE; i++ {
    outer[i] = 0x5C ^ 0x00
  }

  # Inner padded key
  var inner = bytes(BLOCK_SIZE)
  iter var i = 0; i < key.length(); i++ {
    inner[i] = 0x36 ^ key[i]
  }

  iter var i = key.length(); i < BLOCK_SIZE; i++ {
    inner[i] = 0x36 ^ 0x00
  }

  var inner_hash = method(inner + str, true)

  return method(outer + inner_hash, as_bytes)
}

/**
 * Returns the HMAC-MD4 cryptographic hash of the given string or bytes.
 *
 * @param {string|bytes} key
 * @param {string|bytes} str
 * @param bool? as_bytes
 * @returns {string|bytes} 
 */
def hmac_md4(key, str, as_bytes) {
  return hmac(md4, key, str, as_bytes)
}

/**
 * Returns the HMAC-MD5 cryptographic hash of the given string or bytes.
 *
 * @param {string|bytes} key
 * @param {string|bytes} str
 * @param bool? as_bytes
 * @returns {string|bytes} 
 */
def hmac_md5(key, str, as_bytes) {
  return hmac(md5, key, str, as_bytes)
}
/**
 * Returns the HMAC-SHA1 cryptographic hash of the given string or bytes.
 *
 * @param {string|bytes} key
 * @param {string|bytes} str
 * @param bool? as_bytes
 * @returns {string|bytes} 
 */
def hmac_sha1(key, str, as_bytes) {
  return hmac(sha1, key, str, as_bytes)
}

/**
 * Returns the HMAC-SHA224 cryptographic hash of the given string or bytes.
 *
 * @param {string|bytes} key
 * @param {string|bytes} str
 * @param bool? as_bytes
 * @returns {string|bytes} 
 */
def hmac_sha224(key, str, as_bytes) {
  return hmac(sha224, key, str, as_bytes)
}

/**
 * Returns the HMAC-SHA256 cryptographic hash of the given string or bytes.
 *
 * @param {string|bytes} key
 * @param {string|bytes} str
 * @param bool? as_bytes
 * @returns {string|bytes} 
 */
def hmac_sha256(key, str, as_bytes) {
  return hmac(sha256, key, str, as_bytes)
}

/**
 * Returns the HMAC-SHA384 cryptographic hash of the given string or bytes.
 *
 * @param {string|bytes} key
 * @param {string|bytes} str
 * @param bool? as_bytes
 * @returns {string|bytes} 
 */
def hmac_sha384(key, str, as_bytes) {
  return hmac(sha384, key, str, as_bytes)
}

/**
 * Returns the HMAC-SHA512 cryptographic hash of the given string or bytes.
 *
 * @param {string|bytes} key
 * @param {string|bytes} str
 * @param bool? as_bytes
 * @returns {string|bytes} 
 */
def hmac_sha512(key, str, as_bytes) {
  return hmac(sha512, key, str, as_bytes)
}

/**
 * Returns the HMAC-WHIRLPOOL cryptographic hash of the given string or bytes.
 *
 * @param {string|bytes} key
 * @param {string|bytes} str
 * @param bool? as_bytes
 * @returns {string|bytes} 
 */
def hmac_whirlpool(key, str, as_bytes) {
  return hmac(whirlpool, key, str, as_bytes)
}

/**
 * Returns the HMAC-GOST cryptographic hash of the given string or bytes.
 *
 * @param {string|bytes} key
 * @param {string|bytes} str
 * @param bool? as_bytes
 * @returns {string|bytes} 
 */
def hmac_gost(key, str, as_bytes) {
  return hmac(gost, key, str, as_bytes)
}

#################################################################################
# PBKDF2 DEFINITION BEGINS
#################################################################################

/**
 * Derives a cryptographic key from a password using the PBKDF2 key
 * derivation function defined in RFC 2898 §5.2 (PKCS #5 v2.0), as
 * updated by RFC 8018.
 *
 * ## Examples
 *
 * ### Password storage  (derive then verify)
 *
 * ```zuri
 * import hash
 *
 * var salt = 'f3a8c2b104d7e569'   # 16 random bytes in production
 * var dk   = hash.pbkdf2('sha256', 'correct horse battery staple', salt, 600000)
 * # → 64 lowercase hex characters (32 bytes)
 *
 * # Verification: re-derive and compare.
 * if hash.pbkdf2('sha256', candidate, salt, 600000) == dk {
 *   echo 'Password correct'
 * }
 * ```
 *
 * ### Raw-bytes key for symmetric encryption
 *
 * ```zuri
 * import hash
 *
 * # 32-byte key for AES-256, returned as a bytes object.
 * var key = hash.pbkdf2('sha256', passphrase, salt, 600000, 32, true)
 * ```
 *
 * ### Longer key with SHA-512
 *
 * ```zuri
 * import hash
 *
 * # 64 bytes; dk_len == hLen so only one T block is needed.
 * var dk = hash.pbkdf2('sha512', password, salt, 210000, 64)
 * echo dk.length()   # 128 hex characters = 64 bytes
 * ```
 *
 * ### Key that spans multiple PRF blocks
 *
 * ```zuri
 * import hash
 *
 * # 40 bytes with SHA-1 (hLen = 20) requires two T blocks.
 * var dk = hash.pbkdf2('sha1', 'secret', 'nacl', 4096, 40)
 * echo dk.length()   # 80 hex characters = 40 bytes
 * ```
 *
 * > ## Security notes
 * >
 * > - Always use a **unique, randomly generated salt** for every password.
 * >   Never derive the salt from the username, email, or any other
 * >   predictable input.
 * > - Tune `iterations` so that derivation takes ~100 ms on your target
 * >   hardware. Re-benchmark as server capacity increases over time.
 * > - For pure password storage where output size is not a concern,
 * >   consider `bcrypt` (built into Zuri's `hash` module). PBKDF2 is
 * >   most appropriate when you need an arbitrarily long output — symmetric
 * >   keys, key wrapping, or protocol key schedules.
 * > - When comparing derived keys, use a constant-time equality function
 * >   to prevent timing side-channel attacks.
 *
 * @param  {string}         algorithm   HMAC variant used as the PRF. One of:
 *    `'sha1'`, `'sha224'`, `'sha256'`, `'sha384'`, `'sha512'`, `'md5'`.
 *    Prefer `'sha256'` or `'sha512'` for new designs.
 * @param  {string|bytes} password    The password (HMAC key). Any string
 *    or bytes value is accepted.
 * @param  {string|bytes} salt        The cryptographic salt. Use at least
 *    16 bytes of random data per password.
 *    Never reuse a salt across different passwords.
 * @param  {number}         iterations  Iteration count `c` (must be >= 1).
 *    OWASP 2023 minimums:
 *    'sha1'   → 1 300 000
 *    'sha256' →   600 000
 *    'sha512' →   210 000
 * @param  {number}         dk_len      Derived key length in bytes.
 *    Defaults to the PRF output length (hLen) when nil or omitted.
 *    Maximum: (2^32 - 1) * hLen.
 * @param  {bool}           as_bytes  true  → return a bytes object.
 *    false → return a lowercase hex string (default).
 *
 * @return string | bytes  The derived key.
 *
 * @throws Exception
 */
def pbkdf2(algorithm, password, salt, iterations, dk_len, as_bytes) {

  # -------------------------------------------------------------------------
  # Internal helpers
  # -------------------------------------------------------------------------

  # Encode the 1-based block index as a 4-byte big-endian bytes object.
  # This is INT(i) from RFC 2898 §5.2 — appended to the salt for U_1.
  def _block_counter(i) {
    return bytes([
      (i >> 24) & 0xff,
      (i >> 16) & 0xff,
      (i >>  8) & 0xff,
       i        & 0xff,
    ])
  }

  # -------------------------------------------------------------------------
  # 1.  Validate inputs and build the PRF dispatch table
  # -------------------------------------------------------------------------

  # Map algorithm name to the HMAC function and its output length in bytes.
  # h_len = hex output length / 2 (since hmac_* returns lowercase hex).
  var _algos = {
    'md5':    { fn: @(k, d) => hmac_md5(k, d, true),    h_len: 16 },
    'sha1':   { fn: @(k, d) => hmac_sha1(k, d, true),   h_len: 20 },
    'sha224': { fn: @(k, d) => hmac_sha224(k, d, true), h_len: 28 },
    'sha256': { fn: @(k, d) => hmac_sha256(k, d, true), h_len: 32 },
    'sha384': { fn: @(k, d) => hmac_sha384(k, d, true), h_len: 48 },
    'sha512': { fn: @(k, d) => hmac_sha512(k, d, true), h_len: 64 },
  }

  var algo_key = algorithm.lower()
  if !_algos.contains(algo_key) {
    raise Exception(
      'hash.pbkdf2: unsupported algorithm "' + algorithm + '". ' +
      'Accepted values: md5, sha1, sha224, sha256, sha384, sha512.'
    )
  }

  if !is_int(iterations) or iterations < 1 {
    raise Exception(
      'hash.pbkdf2: iterations must be a positive integer >= 1, got: ' +
      iterations
    )
  }

  var algo  = _algos[algo_key]
  var h_fn  = algo.fn      # @(key: bytes, data: bytes) → hex string
  var h_len = algo.h_len   # PRF output length in bytes

  # Default dk_len to the natural PRF output length when not supplied.
  if dk_len == nil { dk_len = h_len }

  if !is_int(dk_len) or dk_len < 1 {
    raise Exception(
      'hash.pbkdf2: dk_len must be a positive integer, got: ' + dk_len
    )
  }

  # RFC 2898 §5.2 step 1 — enforce the maximum derived-key length.
  var max_len = 4294967295 * h_len
  if dk_len > max_len {
    raise Exception(
      'hash.pbkdf2: dk_len (' + dk_len + ') exceeds the maximum ' +
      'allowed length (' + max_len + ') for algorithm "' + algorithm + '".'
    )
  }

  # -------------------------------------------------------------------------
  # 2.  Coerce password and salt to bytes
  #
  #     hmac_* accepts string | bytes for both key and data.  We normalise
  #     to bytes here so the rest of the function is type-uniform and the
  #     bytes concatenation operator (+) works correctly when building the
  #     U_1 data argument (salt + block_counter).
  # -------------------------------------------------------------------------

  if is_string(password) {
    password = password.to_bytes()
  }

  if is_string(salt)     {
    salt     = salt.to_bytes()
  }

  # -------------------------------------------------------------------------
  # 3.  Derive block counts  (RFC 2898 §5.2 step 2)
  #
  #     l = CEIL(dk_len / h_len)     — number of T_i blocks
  #     r = dk_len - (l - 1) * h_len — bytes consumed from the last block
  # -------------------------------------------------------------------------

  var l = math.ceil(dk_len / h_len)
  var r = dk_len - (l - 1) * h_len

  # -------------------------------------------------------------------------
  # 4.  Main PBKDF2 derivation loop  (RFC 2898 §5.2 step 3)
  #
  #     For each 1-based block index i in [1 … l]:
  #
  #       U_1 = PRF(password,  salt || INT(i))
  #       U_j = PRF(password,  U_{j-1})         for j = 2 … c
  #       T_i = U_1 XOR U_2 XOR … XOR U_c
  #
  #     The password is always the HMAC key — it never changes.
  #     The data argument is:
  #       - U_1: the salt bytes with the 4-byte counter appended (+).
  #       - U_j: the bytes object from the previous round directly.
  #
  #     hmac_* returns a hex string; convert.hex_to_bytes() converts it to a
  #     bytes object so indexing gives numeric values for XOR, and so it
  #     can be passed as data to the next round without re-encoding.
  # -------------------------------------------------------------------------

  var dk = []   # derived key accumulated as a list of byte integers

  iter var block_idx = 1; block_idx <= l; block_idx++ {

    # U_1: data is salt concatenated with the big-endian block index.
    var u = h_fn(password, salt + _block_counter(block_idx))

    # T_i is initialised to U_1.  We keep it as a mutable list so
    # individual byte positions can be XOR-updated without allocation.
    var t = []
    iter var b = 0; b < h_len; b++ {
      t.append(u[b])
    }

    # U_2 … U_c: chain the previous U as data; XOR each into T_i.
    iter var j = 1; j < iterations; j++ {
      # u is already a bytes object — pass it directly as HMAC data.
      u = h_fn(password, u)

      # XOR U_j into T_i one byte at a time using bytes index access.
      iter var b = 0; b < h_len; b++ {
        t[b] = t[b] ^ u[b]
      }
    }

    # Append T_i to the output accumulator.
    # The last block contributes only r bytes (RFC §5.2, final step).
    var take = (block_idx == l) ? r : h_len
    iter var b = 0; b < take; b++ {
      dk.append(t[b])
    }
  }

  # -------------------------------------------------------------------------
  # 5.  Return in the requested format
  # -------------------------------------------------------------------------

  var final = bytes(dk)

  if as_bytes {
    return final         # bytes object — suitable for direct crypto use
  }

  return convert.bytes_to_hex(final)   # lowercase hex string (default)
}
