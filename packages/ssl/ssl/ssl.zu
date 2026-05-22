#!-- part of the ssl module

import _ssl
import .context { SSLContext }

import date


/**
 * SSL interface class
 */
class SSL {

  /**
   * @param [[ssl.SSLContext]] context
   * @constructor
   */
  SSL(context) {
    if !instance_of(context, SSLContext)
      raise TypeError('instance of SSLContext expected')
    self._context = context
    self._ptr = _ssl.new(context.get_pointer())
  }

  /**
   * Puts this SSL instance in the connected mode.
   */
  set_connect_state() {
    _ssl.set_connect_state(self._ptr)
  }

  /**
   * Puts this SSL instance in the accept mode.
   */
  set_accept_state() {
    _ssl.set_accept_state(self._ptr)
  }

  /**
   * Returns the current socket file descriptor.
   * It returns `-1` on failure or a positive integer on success.
   * 
   * @returns number
   */
  get_fd() {
    return _ssl.get_fd(self._ptr)
  }

  /**
   * Sets the socket file descriptor for this SSL.
   * 
   * @param int fd
   * @returns bool
   */
  set_fd(fd) {
    if !is_int(fd)
      raise TypeError('fd must be an integer')

    return _ssl.set_fd(self._ptr, fd)
  }

  /**
   * Begins accepting data on SSL and returns `true` if successful or 
   * `false` otherwise.
   * 
   * @returns bool
   */
  accept() {
    return _ssl.accept(self._ptr) == 1
  }

  /**
   * Connects to an SSL server instance.
   * 
   * @returns bool
   * @throws
   */
  connect() {
    /* catch {
      var res = _ssl.connect(self._ptr)
      return res
    } as e

    if e {
      if e.message.index_of('eof while reading') {
        raise Exception('timeout')
      }

      raise e
    } */

    # _ssl.set_connect_state(self._ptr)
    return _ssl.connect(self._ptr)
  }

  /**
   * Writes data to the current I/O stream and return an integer representing 
   * the total bytes written.
   * 
   * @param string|bytes data
   * @returns int 
   */
  write(data) {
    if !is_string(data) and !is_bytes(data)
      raise TypeError('string or bytes expected')

    if is_string(data) data = data.to_bytes()

    var result = _ssl.write(self._ptr, data)
    if result == false {
      var err = self.error()
      if err {
        raise Exception(err)
      }

      return 0
    }
    
    return data.length()
  }

  /**
   * Reads data off the I/O and returns it. Set _length_ to -1 to read 
   * till no data is available in the stream.
   * 
   * @param int? length: Default value is -1
   * @param bool? is_blocking: Default value is false
   * @returns string
   */
  read(length, is_blocking) {
    if !length length = -1
    if is_blocking == nil is_blocking = false
    
    if !is_int(length)
      raise TypeError('integer expected in argument 1')
    if !is_bool(is_blocking)
      raise TypeError('boolean expected in argument 2')
    
    var result = _ssl.read(self._ptr, length, is_blocking)
    if result == nil {
      var err = self.error()
      if err {
        raise Exception(err)
      }

      return ''
    }

    return result
  }

  /**
   * Returns the last SSL error number
   * 
   * @param int? code
   * @returns int
   */
  error(code) {
    if !code code = -1
    return _ssl.error_string(self._ptr, code)
  }

  /**
   * Shutdown the SSL object.
   */
  shutdown() {
    _ssl.shutdown(self._ptr)
  }

  /**
   * Sets the Server Name Indication (SNI) for use by Secure Sockets 
   * Layer (SSL). This function should be called on a client SSL 
   * session before the TLS handshake for the SNI extension 
   * to be set properly.
   * 
   * @param string name
   * @returns bool
   */
  set_tlsext_host_name(name) {
    return _ssl.set_tlsext_host_name(self._ptr, name)
  }

  _parse_cert_time(time) {
    if time.length() > 13 {
      # it uses four digit year e.g.20250728235959Z
      return date(
        to_number(time[0,4]), # year
        to_number(time[4,6]), # month
        to_number(time[6,8]), # day
        to_number(time[8,10]), # hour
        to_number(time[10,12]), # minute
        to_number(time[12,14]) # second
      )
    } else {
      # it uses two digit year e.g. 250728235959Z
      return date(
        to_number(time[0,2]) + 2000, # year
        to_number(time[2,4]), # month
        to_number(time[4,6]), # day
        to_number(time[6,8]), # hour
        to_number(time[8,10]), # minute
        to_number(time[10,12]) # second
      )
    }
  }

  /**
   * Returns informations about the peer certificate in a dictionary.
   * 
   * The returned information includes:
   * 
   * - `subject_name`
   * - `issuer_name`
   * - `serial_number`
   * - `not_before`
   * - `not_after`
   * - `public_key`
   * - `extensions`
   * - `algorithm`
   * 
   * @returns dict
   */
  get_peer_certificate() {
    var cert =  _ssl.get_peer_certificate(self._ptr)

    if cert.not_before {
      cert.not_before = self._parse_cert_time(cert.not_before)
    }

    if cert.not_after {
      cert.not_after = self._parse_cert_time(cert.not_after)
    }

    return cert
  }

  /**
   * Frees this SSL and all associated resources.
   */
  free() {
    _ssl.ssl_free(self._ptr)
  }

  /**
   * Returns the raw OpenSSl SSL pointer.
   * 
   * @returns ptr
   */
  get_pointer() {
    return self._ptr
  }
}
