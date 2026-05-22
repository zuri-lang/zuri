#!-- part of the ssl module

import .constants { TLS_server_method, TLS_method }
import .context { SSLContext }
import .ssl { SSL }

import socket { Socket, IP_LOCAL, AF_INET, SOCK_STREAM, IPPROTO_TCP }

/**
 * TLS enabled Socket version powered by OpenSSL.
 * @printable
 */
class TLSSocket {

  /**
   * This property holds the host bound, to be bound to or connected to by the current socket.
   * Whenever a host is not given, the host will default to localhost.
   * 
   * @type string
   */
  var host = IP_LOCAL

  /**
   * The port currently bound or connected to by the socket.
   * 
   * @type number
   */
  var port = 0

  /**
   * The socket family (which must be one of the `AF_` variables).
   * The default family for the socket is AF_INET.
   * 
   * @type number
   */
  var family = AF_INET

  /**
   * The type of socket stream used by the socket.
   * The default socket type is `SOCK_STREAM`.
   * 
   * @type number
   */
  var type = SOCK_STREAM

  /**
   * The current operating protocol of the socket that controls the 
   * underlying behavior of the socket. The default is `IPPROTO_TCP`.
   * 
   * @type number
   */
  var protocol = IPPROTO_TCP

  /**
   * The file descriptor id of the current socket on the host machine.
   * 
   * @type number
   */
  var id = -1

  /**
   * `true` when the socket is a client to a server socket, `false` otherwise.
   * 
   * @type bool
   */
  var is_client = false

  /**
   * `true` when the socket is bound to a given port on the device, `false` 
   * otherwise.
   * 
   * @type bool
   */
  var is_bound = false

  /**
   * `true` when the socket is connected to a server socket, `false` otherwise.
   * 
   * @type bool
   */
  var is_connected = false

  /**
   * `true` when the socket is currently listening on a host device port as a 
   * server, `false` otherwise.
   * 
   * @type bool
   */
  var is_listening = false
  
  /**
   * `true` when the socket is closed, `false` otherwise.
   * 
   * @type bool
   */
  var is_closed = false

  /**
   * `true` when the socket is shutdown, `false` otherwise.
   * 
   * @type bool
   */
  var is_shutdown = false

  /**
   * `true` when the socket is running in a blocking mode, `false` otherwise.
   * 
   * @type bool
   */
  var is_blocking = false

  /**
   * The property holds the reason for which the last `shutdown` operation 
   * was called or `-1` if `shutdown` was never requested.
   * 
   * @type number
   */
  var shutdown_reason = -1

  /**
   * The amount of time in milliseconds that the socket waits before it 
   * terminates a `send` operation. This is equal to the `SO_SNDTIMEO`.
   * 
   * @type number
   */
  var send_timeout = -1

  /**
   * The amount of time in milliseconds that the socket waits before it 
   * terminates a `receive` operation. This is equal to the `SO_RCVTIMEO`.
   * 
   * @type number
   */
  var receive_timeout = -1

  /**
   * @param [[socket.Socket]] socket
   * @param [[ssl.SSLContext]]? context
   * @param [[ssl.SSL]]? ssl
   * @constructor
   */
  TLSSocket(socket, context, ssl) {
    if socket != nil and !instance_of(socket, Socket)
      raise TypeError('instance of Socket expected in first argument')
    if context != nil and !instance_of(context, SSLContext)
      raise TypeError('instance of SSLContext expected in second argument')

    if !socket self._socket = Socket()
    else self._socket = socket

    if !context self._context = SSLContext(TLS_method)
    else self._context = context

    self._ssl = ssl ? ssl : SSL(self._context)
  }

  /**
   * Initiates a connection to the given host on the specified port. If host is `nil`, it will 
   * connect on to the current hostn specified on the socket.
   * 
   * @param string host
   * @param int port
   * @param int? timeout: Default is 300,000ms (i.e. 300 seconds)
   * @returns bool
   */
  connect(host, port, timeout) {
    if self._socket.connect(host, port, timeout) {
      if self._ssl.set_fd(self._socket.id) {
        return self._ssl.connect()
      }
    }
    return false
  }

  /**
   * Binds this socket to the given port on the given host. If host is `nil` or not specified, it will connect 
   * on to the current hostn specified on the socket. 
   * 
   * @param int port
   * @param string? host
   * @returns bool
   */
  bind(port, host) {
    return self._socket.bind(port, host)
  }

  /**
   * Sends the specified message to the socket. When this methods accepts a file as a message, 
   * the file is read and the resultant bytes of the file content is streamed to the socket.
   * 
   * @note the flags parameter is currently redundant and is kept only to remain compatible with future plans for this method.
   * @param string|bytes|file message
   * @param int? flags
   * @returns number greater than -1 if successful indicating the total number of bytes sent or -1 if it fails.
   */
  send(message, flags) {
    if !self._ssl 
      return self._socket.send(message, flags)
    return self._ssl.write(message)
  }

  /**
   * Receives bytes of the given length from the socket. If the length is not given, it default length of 
   * -1 indicating that the total available data on the socket stream will be read. 
   * If no data is available for read on the socket, the socket will wait to receive data or until the 
   * `receive_timeout` which is also equal to the `SO_RCVTIMEO` setting of the socket has elapsed before or 
   * until it has received the total number of bytes required (whichever comes first).
   * 
   * @note the flags parameter is currently redundant and is kept only to remain compatible with future plans for this method.
   * @param int? length
   * @param int? flags
   * @returns string
   */
  receive(length, flags) {
    if !self._ssl
      return self._socket.receive(length, flags)
    return self._ssl.read(length, self._socket.is_blocking)
  }

  /**
   * Reads bytes of the given length from the socket. If the length is not given, it default length of 
   * -1 indicating that the total available data on the socket stream will be read. 
   * 
   * > This method differs from `receive()` in that it does not check for a socket having data to 
   * > read or not and will block until data of _length_ have been read or no more data is available for 
   * > reading.
   * 
   * @note Only use this function after a call to `receive()` has succeeded.
   * @param int? length: Default value is 1024.
   * @returns string
   */
  read(length) {
    return self.receive(length)
  }

  /**
   * Listen for connections on a socket
   * 
   * This method puts the socket in a state where it is willing to accept incoming connections and creates 
   * a queue limit of `queue_length` for incoming connections. If a connection request arrives with 
   * the queue full, the client may receive an error with an indication of `ECONNREFUSED`. 
   * Alternatively, if the underlying protocol supports retransmission, the request may be ignored 
   * so that retries may succeed.
   * 
   * When the `queue_length` is omitted or set to -1, the method will use the default queue limit of
   * the current platform which is usually equal to `SOMAXCONN`.
   * 
   * @note listen() call applies only to sockets of type `SOCK_STREAM` (which is the default).
   * @param int? queue_length: Default value is `SOMAXCONN`.
   * @returns bool
   */
  listen(queue_length) {
    return self._socket.listen(queue_length)
  }

  /**
   * Accepts a connection on a socket
   * 
   * This method extracts the first connection request on the queue of pending connections, creates a new socket 
   * with the same properties of the current socket, and allocates a new file descriptor for the socket.  If no 
   * pending connections are present on the queue, and the socket is not marked as non-blocking, accept() blocks 
   * the caller until a connection is present.  If the socket is marked non-blocking and no pending connections 
   * are present on the queue, accept() returns an error as described below.  
   * 
   * The accepted socket may not be used to accept more connections.  The original socket remains open.
   * 
   * @returns [[ssl.TLSSocket]]
   */
  accept() {
    var s = self._socket.accept()

    var ssl = SSL(self._context)
    ssl.set_fd(s.id)
    ssl.set_accept_state()

    # var accepted = ssl.accept()
    # if !accepted raise Exception(ssl.error(accepted))

    var client = TLSSocket(s, self._context, ssl)
    client.is_connected = true
    return client
  }

  /**
   * Closes the socket.
   * 
   * @returns bool
   */
  close() {
    if self._ssl {
      # self._ssl.shutdown()
      # self._ssl.free()
    }
    var result = self._socket.close()
    return result
  }

  /**
   * The shutdown() call causes all or part of a full-duplex connection on the socket associated with 
   * socket to be shut down.
   * 
   * @returns bool
   */
  shutdown() {
    if self._ssl {
      return self._ssl.shutdown()
    }
    return false
    # return self._socket.shutdown(how)
  }

  /**
   * Sets the options of the current socket.
   * 
   * @note Only `SO_` variables are valid option types.
   * @param int option
   * @param any value
   * @returns bool
   */
  set_option(option, value) {
    return self._socket.set_option(option, value)
  }

  /**
   * Gets the options set on the current socket.
   * 
   * @param int option
   * @returns any
   */
  get_option(option) {
    return self._socket.get_option(option)
  }

  /**
   * Sets if the socket should operate in blocking or non-blocking mode. `true` for blocking 
   * (default) and `false` for non-blocking.
   * 
   * @param bool mode
   * @returns bool
   */
  set_blocking(mode) {
    return self._socket.set_blocking(mode)
  }

  /**
   * Returns a dictionary containing the address, port and family of the current socket or an 
   * empty dictionary if the socket information could not be retrieved.
   * 
   * @returns dictionary
   */
  info() {
    return self._socket.info()
  }

  /**
   * Returns the underlying Socket instance.
   * 
   * @returns [[socket.Socket]]
   */
  get_socket() {
    return self._socket
  }

  /**
   * Returns the underlying SSLContext instance.
   * 
   * @returns [[ssl.SSLContext]]
   */
  get_context() {
    return self._context
  }

  /**
   * Returns the underlying SSL instance
   *
   * @returns [[ssl.SSL]]
   */
  get_ssl() {
    return self._ssl
  }

  /**
   * Sets the underlying SSL context to use.
   * 
   * @param [[ssl.SSLContext]] context
   */
  set_context(context) {
    if !instance_of(content, SSLContext)
      raise TypeError('instance of SSLContext expected')
    self._context = context
  }

  @to_string() {
    return '<TLSSocket closed: ${self.is_closed}, listening: ' +
        '${self.is_listening}, connected: ${self.is_connected}, bound: ${self.is_bound}>'
  }
}

/**
 * Returns a new instance of a TLSSocket.
 * 
 * @param [[socket.Socket]] socket
 * @param [[ssl.SSLContext]]? context
 * @param [[ssl.SSL]]? ssl
 * @default
 */
def socket(socket, context, ssl) {
  return TLSSocket(socket, context, ssl)
}
