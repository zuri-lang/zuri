#!-- part of the ssl module

import reflect
import _ssl

/**
 * SSL context representation class
 */
class SSLContext {

  /**
   * @note Method must be a valid SSL method pointer.
   * @param ptr method
   * @constructor
   */
  SSLContext(method) {
    if !reflect.is_ptr(method)
      raise TypeError('SSL method expected')
    self._method = method
    self._ptr = _ssl.ctx(method)
  }

  /**
   * Enables or disables the verification flags for the given mode on the context.
   * 
   * @note The verification of certificates can be controlled by a set of logically or'ed mode flags.
   * @note If the mode is SSL_VERIFY_NONE none of the other flags may be set.
   * @param int mode
   * @param bool? disable - Default: false
   */
  set_verify(mode, disable) {
    if disable == nil disable = false

    if !is_int(mode)
      raise TypeError('integer expected in argument 1')
    if !is_bool(disable)
      raise TypeError('boolean expected in argument 2')

    _ssl.ctx_set_verify(self._ptr, mode, disable)
  }

  /**
   * Sets the default locations for trusted CA certificates.
   * 
   * @param string locations
   */
  set_verify_locations(locations) {
    if !is_string(locations)
      raise TypeError('location path string expected')
    _ssl.ctx_set_verify_locations(self._ptr, locations)
  }

  /**
   * Loads the given SSL/TLS certificate pairs for the given SSL/TLS context.
   * 
   * @param string|file cert_file
   * @param string|file private_key_file
   * @returns bool
   */
  load_certs(cert_file, private_key_file) {
    if !is_string(cert_file) and !is_file(cert_file)
      raise TypeError('cert_file must be a string or file')
    if private_key_file != nil and !is_string(private_key_file) and !is_file(private_key_file)
      raise TypeError('private_key_file must be a string or file')
    if !private_key_file private_key_file = cert_file

    if is_file(cert_file) cert_file = cert_file.abs_path()
    if is_file(private_key_file) private_key_file = private_key_file.abs_path()

    return _ssl.load_certs(self._ptr, cert_file, private_key_file)
  }

  /**
   * Sets the list of allowed ciphers. This list must be colon (:) separated.
   * 
   * @param string ciphers
   * @returns bool
   */
  set_ciphers(ciphers) {
    if !is_string(ciphers)
      raise TypeError('string expected')
    return _ssl.set_ciphers(self._ptr, ciphers)
  }

  /**
   * Frees this Context and all associated resources
   */
  free() {
    _ssl.ctx_free(self._ptr)
  }

  /**
   * Returns the raw OpenSSl SSL_CTX pointer.
   * 
   * @returns ptr
   */
  get_pointer() {
    return self._ptr
  }
}


import .constants

/**
 * TLSContext is a specialized Context providing generic TLS support 
 * for both client and server mode.
 */
class TLSContext < SSLContext {

  /**
   * @constructor
   */
  TLSContext() {
    parent(_ssl.TLS_method())
  }
}



/**
 * TLSClientContext is a specialized Context for supporting TLS clients.
 */
class TLSClientContext < SSLContext {

  /**
   * @constructor
   */
  TLSClientContext() {
    parent(_ssl.TLS_client_method())
  }
}



/**
 * TLSServerContext is a specialized Context for supporting TLS servers.
 */
class TLSServerContext < SSLContext {

  /**
   * @constructor
   */
  TLSServerContext() {
    parent(_ssl.TLS_server_method())
  }
}

/**
 * SSLv23Context is a specialized Context providing generic SSLv23 support 
 * for both client and server mode.
 */
class SSLv23Context < SSLContext {

  /**
   * @constructor
   */
  SSLv23Context() {
    parent(_ssl.SSLv23_method())
  }
}



/**
 * SSLv23ClientContext is a specialized Context for supporting SSLv23 clients.
 */
class SSLv23ClientContext < SSLContext {

  /**
   * @constructor
   */
  SSLv23ClientContext() {
    parent(_ssl.SSLv23_client_method())
  }
}



/**
 * SSLv23ServerContext is a specialized Context for supporting SSLv23 servers.
 */
class SSLv23ServerContext < SSLContext {

  /**
   * @constructor
   */
  SSLv23ServerContext() {
    parent(_ssl.SSLv23_server_method())
  }
}
