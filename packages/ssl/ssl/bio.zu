#!-- part of the ssl module

import _ssl
import reflect
import .ssl { SSL }
import .constants

var _close_opts = [constants.BIO_NOCLOSE, constants.BIO_CLOSE]

/**
 * SSL Binary Input/Output implementation
 */
class BIO {

  # SSL tracker
  var _ssl

  /**
   * @note Method must be a valid SSL BIO_* method
   * @param ptr method
   * @constructor
   */
  BIO(method) {
    if !reflect.is_ptr(method)
      raise TypeError('SSL BIO method expected')
    self._ptr = _ssl.new_bio(method)
  }

  /**
   * Sets the working SSL instance for this BIO.
   * 
   * @note Option must be one of the BIO constants if given.
   * @param [[ssl.SSL]] ssl
   * @param int? option: Default value is `BIO_NOCLOSE`
   */
  set_ssl(ssl, option) {
    if !instance_of(ssl, SSL)
      raise TypeError('instance of SSL expected')

    if !option option = constants.BIO_CLOSE
    if !is_int(option)
      raise ValueError('option must be a BIO_* constant')

    self._ssl = ssl
    _ssl.set_ssl(self._ptr, ssl.get_pointer(), option)
  }

  /**
   * Sets the hostname for the current connected BIO socket.
   * 
   * @param string name
   */
  set_conn_hostname(name) {
    if !is_string(name)
      raise TypeError('string expected')
    _ssl.set_conn_hostname(self._ptr, name)
  }

  /**
   * Sets the address name for the current accepted BIO socket.
   * 
   * @param string name
   */
  set_accept_tname(name) {
    if !is_string(name)
      raise TypeError('string expected')
    _ssl.set_accept_name(self._ptr, name)
  }

  /**
   * Sets the address for the current connected BIO socket.
   * 
   * @param string address
   */
  set_conn_address(address) {
    if !is_string(address)
      raise TypeError('string expected')
    _ssl.set_conn_address(self._ptr, address)
  }

  /**
   * Sets the port for the current connected BIO socket.
   * 
   * @param int|string port
   */
  set_conn_port(port) {
    if is_int(port) port = '${port}'

    if !is_string(port)
      raise TypeError('integer or string expected')

    _ssl.set_conn_port(self._ptr, port)
  }

  /**
   * Sets the port for the current accepted BIO socket.
   * 
   * @param int|string port
   */
  set_accept_port(port) {
    if is_int(port) port = '${port}'

    if !is_string(port)
      raise TypeError('integer or string expected')

    _ssl.set_accept_port(self._ptr, port)
  }

  /**
   * Sets the socket family for the current connected BIO socket.
   * 
   * @param int family
   */
  set_conn_family(family) {
    if !is_int(family)
      raise TypeError('integer expected')

    _ssl.set_conn_family(self._ptr, family)
  }

  /**
   * Sets the socket family for the current accepted BIO socket.
   * 
   * @param int family
   */
  set_accept_family(family) {
    if !is_int(family)
      raise TypeError('integer expected')

    _ssl.set_accept_family(self._ptr, family)
  }

  /**
   * Returns the hostname for the current connected BIO socket.
   * 
   * @returns string
   */
  get_conn_hostname() {
    return _ssl.get_conn_hostname(self._ptr)
  }

  /**
   * Returns the hostname for the current accepted BIO socket.
   * 
   * @returns string
   */
  get_accept_name() {
    return _ssl.get_accept_name(self._ptr)
  }

  /**
   * Returns the address for the current connected BIO socket.
   * 
   * @returns string
   */
  get_conn_address() {
    return _ssl.get_conn_address(self._ptr)
  }

  /**
   * Returns the port for the current connected BIO socket.
   * 
   * @returns string
   */
  get_conn_port() {
    return to_int(_ssl.get_conn_port(self._ptr))
  }

  /**
   * Returns the port for the current accepted BIO socket.
   * 
   * @returns string
   */
  get_accept_port() {
    return to_int(_ssl.get_accept_port(self._ptr))
  }

  /**
   * Returns the family for the current connected BIO socket.
   * 
   * @returns int
   */
  get_conn_family() {
    return _ssl.get_conn_family(self._ptr)
  }

  /**
   * Returns the family for the current accepted BIO socket.
   * 
   * @returns int
   */
  get_accept_family() {
    return _ssl.get_accept_family(self._ptr)
  }

  /**
   * Returns the current socket file descriptor.
   * It returns `-1` on failure or a positive integer on success.
   * 
   * @returns number
   */
  get_fd() {
    return _ssl.bio_get_fd(self._ptr)
  }

  /**
   * Sets the socket file descriptor for this BIO
   * 
   * @param int fd
   * @param int? opt: Default value is `BIO_NOCLOSE`
   */
  set_fd(fd, opt) {
    if !is_int(fd)
      raise TypeError('fd must be an integer')
    if opt != nil and !_close_opts.contains(fd)
      raise ValueError('opt must be one of BIO_CLOSE or BIO_NOCLOSE')

    if !opt opt = constants.BIO_NOCLOSE

    _ssl.bio_set_fd(self._ptr, fd, opt)
  }

  /**
   * Converts the BIO into a non-blocking I/O stream if b is `true`, otherwise 
   * converts it into a blocking stream.
   * 
   * @param bool? is_blocking: Default value is `true`.
   */
  set_non_blocking(is_blocking) {
    if !is_blocking is_blocking = true

    if !is_bool(is_blocking)
      raise TypeError('boolean expected')

    _ssl.set_nbio(self._ptr, is_blocking)
  }

  /**
   * It appends bio, which may be a single BIO or a chain of BIOs, 
   * to the current BIO stack (unless the current pinter is `nil`). 
   * It then makes a control call on BIO _bio_ and returns it.
   * 
   * @param [[ssl.BIO]] bio
   * @returns self
   */
  push(bio) {
    if !instance_of(bio, BIO)
      raise TypeError('instance of BIO expected')
    if bio {
      _ssl.push(self._ptr, bio.get_pointer())
    }
    return self
  }

  /**
   * Removes this BIO from any chain is is part of
   */
  pop() {
    _ssl.pop(self._ptr)
  }

  /**
   * Writes data to the current I/O stream and returns the total bytes written.
   * 
   * @param string|bytes data
   * @returns int
   */
  write(data) {
    if !is_string(data) and !is_bytes(data)
      raise TypeError('string or bytes expected')

    if is_bytes(data) data = to_string(data)

    var result = _ssl.bio_write(self._ptr, data)
    if result == -1
      raise Exception(self.error_string())
    
    return result
  }

  /**
   * Reads data off the I/O and returns it.
   * 
   * @param int? length: Default value is `1024`
   * @returns string
   */
  read(length) {
    if !length length = 1024
    if !is_int(length)
      raise TypeError('integer expected')
    
    var result = _ssl.bio_read(self._ptr, length)
    if result == nil {
      raise Exception(self.error_string())
    }

    return result
  }

  /**
   * Returns `true` if this BIO needs to retry its last operation. 
   * `false` otherwise.
   * 
   * @returns bool
   */
  should_retry() {
    return _ssl.should_retry(self._ptr)
  }

  /**
   * Attempts to establish a connection to the host.
   * 
   * @returns int
   */
  do_connect() {
    return _ssl.do_connect(self._ptr)
  }

  /**
   * Attempts to accept the connected socket.
   * 
   * @returns int
   */
  do_accept() {
    return _ssl.do_accept(self._ptr)
  }

  /**
   * Returns the last SSL error number.
   * 
   * @param int? code
   * @returns int
   */
  error(code) {
    if code != nil and !is_number(code) and !is_int(code)
      raise TypeError('integer expected')
      
    if !code code = -1
    return _ssl.error(self._ptr, code)
  }

  /**
   * Returns the last SSL error as string.
   * 
   * @returns string
   */
  error_string() {
    if !code code = -1
    return _ssl.error_string(self._ptr, code)
  }

  /**
   * Frees this BIO and all associated resources.
   */
  free() {
    _ssl.free(self._ptr)
  }

  /**
   * Returns the raw OpenSSl BIO pointer.
   * 
   * @returns ptr
   */
  get_pointer() {
    return self._ptr
  }
}





/**
 * SSLBIO is a generic BIO for SSL I/O
 */
class SSLBIO < BIO {

  /**
   * @constructor
   */
  SSLBIO() {
    parent(constants.BIO_f_ssl)
  }
}



/**
 * ConnectBIO is a generic BIO for new secured connections
 */
class ConnectBIO < BIO {

  /**
   * @constructor
   */
  ConnectBIO() {
    parent(constants.BIO_s_connect)
  }
}



/**
 * AcceptedBIO is a generic BIO for accepting new secured 
 * connections from a TLS server
 */
class AcceptedBIO < BIO {

  /**
   * @constructor
   */
  AcceptedBIO() {
    parent(constants.BIO_s_accept)
  }
}
