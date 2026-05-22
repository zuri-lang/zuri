/**
 * @module socket
 *
 * This module provides access to the underlying system socket management 
 * implementations. It is meant to be used to provide more controlled and 
 * specific operating system features and for implementing various standard 
 * and custom network protocols and specifications for which Zuri does not
 * provide a built-in implementation for.
 * 
 * This module defines a lot of constant that whose value complies with the 
 * operating system specification and they should be used instead of a finite 
 * value wherever available as values for these constants can change across 
 * different OS implementations.
 * 
 * ### What's a Socket
 * 
 * Sockets are bidirectional communication medias for information exchange between
 * various processes within the same machine or different machines.
 * 
 * There are three important concepts that must important to know when working with 
 * sockets.
 * 
 * 1. `Family`: This refer to the general group of sockets that a specific 
 * protocol handled by a socket belongs to. This is any of the `AF_` constants.
 * 2. `Types`: The type of communication between the two processes involved. And can 
 * only be one of `SOCK_STREAM` or `SOCK_DGRAM`.
 * 3. `Protocol`: This is to identify the variant protocol on which one or more 
 * network protocols are based on. Typically `0` or any of the `IP_` constants.
 * 
 * A simple socket may be instantiated as follows:
 * 
 * ```zuri
 * import socket { Socket }
 * var sock = Socket()
 * ```
 * > The `{ Socket }` in the import statement means we are only importing the `Socket` 
 * > class and not the entire `socket` module. Other examples here will skip the assume 
 * > you are importing just what you need out of the package but will not show the import 
 * > statement.
 * 
 * The example above instantiates a socket without any arguments, and it is equivalent to:
 * 
 * ```zuri
 * Socket(AF_INET, SOCK_STREAM, IPPROTO_TCP)
 * ```
 * 
 * You can establish a connection with another socket with a known address and port 
 * as follows:
 * 
 * ```zuri
 * var socket = Socket()
 * socket.connect('127.0.0.1', 4000)
 * ```
 * 
 * The above example connects to the process listening at port 4000 on host with IP 
 * address 127.0.0.1. A connection is a pre-requisite to writing or reading from a socket.
 * 
 * After connecting to a socket, you can read and write data as follows:
 * 
 * ```zuri
 * var socket = Socket()
 * socket.connect('127.0.0.1', 4000)
 * 
 * var message_from_client = socket.receive()
 * socket.send('You sent: ' + message_from_client)
 * ```
 * 
 * The above example simply replies the client with `You sent: ` + whatever the client 
 * actually sent.
 * 
 * Due to resource limitations, its good practice to always ensure to close sockets when 
 * done with it. Doing this is pretty simple.
 * 
 * ```zuri
 * socket.close()
 * ```
 * 
 * @copyright 2021, Richard Ore and Zuri contributors
 */

import _socket

# Types

/**
 * stream socket
 */
var SOCK_STREAM = _socket.SOCK_STREAM

/**
 * datagram socket
 */
var SOCK_DGRAM = _socket.SOCK_DGRAM

/**
 * raw-protocol interface
 */
var SOCK_RAW = _socket.SOCK_RAW

/**
 * reliably-delivered message
 */
var SOCK_RDM = _socket.SOCK_RDM

/**
 * sequenced packet stream
 */
var SOCK_SEQPACKET = _socket.SOCK_SEQPACKET


# Option flags per-socket.

/**
 * Turn on debugging info recording
 */
var SO_DEBUG = _socket.SO_DEBUG

/**
 * Socket has had listen()
 */
var SO_ACCEPTCONN = _socket.SO_ACCEPTCONN

/**
 * Allow local address reuse
 */
var SO_REUSEADDR = _socket.SO_REUSEADDR

/**
 * Keep connections alive
 */
var SO_KEEPALIVE = _socket.SO_KEEPALIVE

/**
 * Just use interface addresses
 */
var SO_DONTROUTE = _socket.SO_DONTROUTE

/**
 * Permit sending of broadcast msgs
 */
var SO_BROADCAST = _socket.SO_BROADCAST

/**
 * Bypass hardware when possible
 */
var SO_USELOOPBACK = _socket.SO_USELOOPBACK

/**
 * Linger on close if data present (in ticks)
 */
var SO_LINGER = _socket.SO_LINGER

/**
 * Leave received OOB data in line
 */
var SO_OOBINLINE = _socket.SO_OOBINLINE

/**
 * Allow local address & port reuse
 */
var SO_REUSEPORT = _socket.SO_REUSEPORT

/**
 * Timestamp received dgram traffic
 */
var SO_TIMESTAMP = _socket.SO_TIMESTAMP


# Additional options, not kept in so_options.

/**
 * Send buffer size
 */
var SO_SNDBUF = _socket.SO_SNDBUF

/**
 * Receive buffer size
 */
var SO_RCVBUF = _socket.SO_RCVBUF

/**
 * Send low-water mark
 */
var SO_SNDLOWAT = _socket.SO_SNDLOWAT

/**
 * Receive low-water mark
 */
var SO_RCVLOWAT = _socket.SO_RCVLOWAT

/**
 * Send timeout
 */
var SO_SNDTIMEO = _socket.SO_SNDTIMEO

/**
 * Receive timeout
 */
var SO_RCVTIMEO = _socket.SO_RCVTIMEO

/**
 * Get error status and clear
 */
var SO_ERROR = _socket.SO_ERROR

/**
 * Get socket type
 */
var SO_TYPE = _socket.SO_TYPE


# Level number for (get/set)sockopt() to apply to socket itself.

/**
 * Options for socket level
 */
var SOL_SOCKET = _socket.SOL_SOCKET


# Address families.

/**
 * Unspecified
 */
var AF_UNSPEC = _socket.AF_UNSPEC

/**
 * Local to host (pipes)
 */
var AF_UNIX = _socket.AF_UNIX

/**
 * Backward compatibility with AF_UNIX
 */
var AF_LOCAL = _socket.AF_LOCAL

/**
 * Internetwork: UDP, TCP, etc.
 */
var AF_INET = _socket.AF_INET

/**
 * Arpanet imp addresses
 */
var AF_IMPLINK = _socket.AF_IMPLINK

/**
 * PUP protocols: e.g. BSP
 */
var AF_PUP = _socket.AF_PUP

/**
 * MIT CHAOS protocols
 */
var AF_CHAOS = _socket.AF_CHAOS

/**
 * XEROX NS protocols
 */
var AF_NS = _socket.AF_NS

/**
 * ISO protocols
 */
var AF_ISO = _socket.AF_ISO

/**
 * ISO protocols (same as AF_ISO)
 */
var AF_OSI = _socket.AF_OSI

/**
 * European computer manufacturers
 */
var AF_ECMA = _socket.AF_ECMA

/**
 * Datakit protocols
 */
var AF_DATAKIT = _socket.AF_DATAKIT

/**
 * CCITT protocols, X.25 etc
 */
var AF_CCITT = _socket.AF_CCITT

/**
 * IBM SNA
 */
var AF_SNA = _socket.AF_SNA

/**
 * DECnet
 */
var AF_DECnet = _socket.AF_DECnet

/**
 * DEC Direct data link interface
 */
var AF_DLI = _socket.AF_DLI

/**
 * LAT
 */
var AF_LAT = _socket.AF_LAT

/**
 * NSC Hyperchannel
 */
var AF_HYLINK = _socket.AF_HYLINK

/**
 * Apple Talk
 */
var AF_APPLETALK = _socket.AF_APPLETALK

/**
 * IPv6
 */
var AF_INET6 = _socket.AF_INET6


# Protocol families, same as address families on most platforms.

/**
 * IPPROTO_IP
 */
var IPPROTO_IP = _socket.IPPROTO_IP

/**
 * IPPROTO_ICMP
 */
var IPPROTO_ICMP = _socket.IPPROTO_ICMP

/**
 * IPPROTO_IGMP
 */
var IPPROTO_IGMP = _socket.IPPROTO_IGMP

/**
 * IPPROTO_IPIP
 */
var IPPROTO_IPIP = _socket.IPPROTO_IPIP

/**
 * IPPROTO_TCP
 */
var IPPROTO_TCP = _socket.IPPROTO_TCP

/**
 * IPPROTO_EGP
 */
var IPPROTO_EGP = _socket.IPPROTO_EGP

/**
 * IPPROTO_PUP
 */
var IPPROTO_PUP = _socket.IPPROTO_PUP

/**
 * IPPROTO_UDP
 */
var IPPROTO_UDP = _socket.IPPROTO_UDP

/**
 * IPPROTO_IDP
 */
var IPPROTO_IDP = _socket.IPPROTO_IDP

/**
 * IPPROTO_TP
 */
var IPPROTO_TP = _socket.IPPROTO_TP

/**
 * IPPROTO_DCCP
 */
var IPPROTO_DCCP = _socket.IPPROTO_DCCP

/**
 * IPPROTO_IPV6
 */
var IPPROTO_IPV6 = _socket.IPPROTO_IPV6

/**
 * IPPROTO_RSVP
 */
var IPPROTO_RSVP = _socket.IPPROTO_RSVP

/**
 * IPPROTO_GRE
 */
var IPPROTO_GRE = _socket.IPPROTO_GRE

/**
 * IPPROTO_ESP
 */
var IPPROTO_ESP = _socket.IPPROTO_ESP

/**
 * IPPROTO_AH
 */
var IPPROTO_AH = _socket.IPPROTO_AH

/**
 * IPPROTO_MTP
 */
var IPPROTO_MTP = _socket.IPPROTO_MTP

/**
 * IPPROTO_BEETPH
 */
var IPPROTO_BEETPH = _socket.IPPROTO_BEETPH

/**
 * IPPROTO_ENCAP
 */
var IPPROTO_ENCAP = _socket.IPPROTO_ENCAP

/**
 * IPPROTO_PIM
 */
var IPPROTO_PIM = _socket.IPPROTO_PIM

/**
 * IPPROTO_COMP
 */
var IPPROTO_COMP = _socket.IPPROTO_COMP

/**
 * IPPROTO_SCTP
 */
var IPPROTO_SCTP = _socket.IPPROTO_SCTP

/**
 * IPPROTO_UDPLITE
 */
var IPPROTO_UDPLITE = _socket.IPPROTO_UDPLITE

/**
 * IPPROTO_MPLS
 */
var IPPROTO_MPLS = _socket.IPPROTO_MPLS

/**
 * IPPROTO_RAW
 */
var IPPROTO_RAW = _socket.IPPROTO_RAW

/**
 * IPPROTO_MAX
 */
var IPPROTO_MAX = _socket.IPPROTO_MAX


# howto arguments for shutdown(2), specified by Posix.1g.

/**
 * Shut down the reading side
 */
var SHUT_RD = _socket.SHUT_RD

/**
 * Shut down the writing side
 */
var SHUT_WR = _socket.SHUT_WR

/**
 * Shut down both sides
 */
var SHUT_RDWR = _socket.SHUT_RDWR


/**
 * Maximum queue length specifiable by listen.
 */
var SOMAXCONN = _socket.SOMAXCONN





/**
 * The non-designated address used to represent "no particular address"
 * (also referred to as "any address")
 * 
 * @type string
 */
var IP_ANY     = '0.0.0.0'

/**
 * The loopback address (also known as localhost).
 * 
 * @type string
 */
var IP_LOCAL   = '127.0.0.1'

/**
 * The SocketException class is the general Exception type thrown from sockets
 */
class SocketException < Exception {
  @to_string() {
    return '<SocketException: ${self.message}>'
  }
}

/**
 * The Socket class provides interface for working with Socket clients
 * and servers.
 * 
 * @printable
 */
class Socket {

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
   * @type int
   */
  var port = 0

  /**
   * The socket family (which must be one of the `AF_` variables).
   * The default family for the socket is AF_INET.
   * 
   * @type int
   */
  var family = AF_INET

  /**
   * The type of socket stream used by the socket.
   * The default socket type is `SOCK_STREAM`.
   * 
   * @type int
   */
  var type = SOCK_STREAM

  /**
   * The current operating protocol of the socket that controls the 
   * underlying behavior of the socket. The default is `IPPROTO_TCP`.
   * 
   * @type int
   */
  var protocol = IPPROTO_TCP

  /**
   * The file descriptor id of the current socket on the host machine.
   * 
   * @type int
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
   * @type int
   */
  var shutdown_reason = -1

  /**
   * The amount of time in milliseconds that the socket waits before it 
   * terminates a `send` operation. This is equal to the `SO_SNDTIMEO`.
   * 
   * @type int
   */
  var send_timeout = -1

  /**
   * The amount of time in milliseconds that the socket waits before it 
   * terminates a `receive` operation. This is equal to the `SO_RCVTIMEO`.
   * 
   * @type int
   */
  var receive_timeout = -1

  /**
   * @param number family
   * @param number? type
   * @param number? protocol
   * @constructor  
   * 
   * @example Socket(AF_INET, SOCK_STREAM, IPPROTO_TCP)
   */
  Socket(family, type, protocol, id) {
    # NOTE: NEVER EVER SET `id` YOURSELF.
    # The parameter is meant to make `accept()`.

    if family self.family = family
    if type self.type = type
    if protocol self.protocol = protocol

    if !id {
      if !is_int(self.family) 
        raise ValueError('AF_* expected for family, ${typeof(self.family)} given')
      if !is_int(self.type) 
        raise ValueError('SOCK_* expected for type, ${typeof(self.type)} given')
      if !is_int(self.protocol) 
        raise TypeError('integer expected for protocol, ${typeof(self.protocol)} given')

      self.id = self._check_error(_socket.create(self.family, self.type, self.protocol))
    } else {
      self.id = id
    }
  }

  # checks if a response code is valid
  # returns the code if it is or throws an SocketException otherwise
  _check_error(code) {
    var err = _socket.error(code)
    if err {
      self.close()
      raise SocketException(err)
    }
    return code
  }

  /**
   * Initiates a connection to the given host on the specified port. If host is `nil`, it will 
   * connect on to the current host specified on the socket.
   * 
   * @param string host
   * @param int port
   * @param int? timeout: Defaults to 300,000ms (i.e. 300 seconds)
   * @returns bool
   */
  connect(host, port, timeout) {
    if !host host = self.host
    if !timeout timeout = 300000 # default timeout is 300 seconds

    if !port raise ArgumentError('port not specified')
    if !is_string(host) 
      raise TypeError('string expected for host, ${typeof(host)} given')
    if !is_int(port) 
      raise TypeError('integer expected for port, ${typeof(port)} given')
    if !is_int(timeout) 
      raise TypeError('integer expected for timeout, ${typeof(timeout)} given')

    if self.id == -1 or self.is_closed raise SocketException('socket is in an illegal state')

    if self.is_connected raise SocketException('socket has existing connection')

    var result = self._check_error(_socket.connect(self.id, host, port, self.family, timeout, self.is_blocking))
    if result {
      self.is_client = true
      self.is_connected = true
      self.is_listening = false
      self.is_bound = false
    }

    return result == 0
  }
  
  /**
   * Binds this socket to the given port on the given host. If host is `nil` or not specified, it will connect 
   * on to the current host specified on the socket.
   * 
   * @param int port
   * @param string? host
   * @returns bool
   */
  bind(port, host) {
    if !host host = self.host

    if !port raise ArgumentError('port not specified')
    if !is_string(host) 
      raise TypeError('string expected for host, ${typeof(host)} given')
    if !is_int(port) 
      raise TypeError('integer expected for port, ${typeof(port)} given')

    if self.id == -1 or self.is_closed raise SocketException('socket is in an illegal state')


    if self.is_bound raise SocketException('socket previously bound')

    var result = self._check_error(_socket.bind(self.id, host, port, self.family))
    if result {
      self.is_bound = true
      self.is_listening = false # it's freshly bound
      self.is_connected = false # a bound socket can't be connected ass well
      self.is_client = false # a bound socket cannot be a client
    }
    return result == 0
  }

  /**
   * Sends the specified message to the socket. When this methods accepts a file as a message, 
   * the file is read and the resultant bytes of the file content is streamed to the socket.
   * 
   * @param string|file|bytes|? message
   * @param int? flags: Not currently used.
   * @returns number greater than -1 if successful indicating the total number of bytes sent or -1 if it fails.
   */
  send(message, flags) {
    if !message message = ''
    if !flags flags = 0

    if !is_string(message) and !is_bytes(message) and !is_file(message) 
      raise TypeError('message must string, bytes or file; ${typeof(message)} given')
    if !is_int(flags) 
      raise TypeError('integer expected for flags, ${typeof(flags)} given')

    if self.id == -1 or self.is_closed or (self.is_shutdown and 
      (self.shutdown_reason == SHUT_WR or 
        self.shutdown_reason == SHUT_RDWR)) 
      raise SocketException('socket is in an illegal state')

    if !self.is_listening and !self.is_connected
      raise SocketException('socket not listening or connected')

    return self._check_error(_socket.send(self.id, message, flags))
  }

  /**
   * Sends the specified message to the socket on the given host and port.
   * 
   * UDP (`SOCK_DGRAM`) sockets that haven't been `connect()`'d require `sendto()` and recvfrom() to 
   * specify the peer address per datagram.
   * 
   * @param {string?} host
   * @param {int?} port
   * @param {int?} family
   * @param {string|file|bytes|nil} message
   * @param {int?} flags: Not currently used.
   * @returns number greater than -1 if successful indicating the total number of bytes sent or 
   *    -1 if it fails.
   */
  send_to(host, port, family, message, flags) {
    if !host host = self.host
    if !port port = self.port
    if !family family = self.family
    if !message message = ''
    if !flags flags = 0

    if !port raise ArgumentError('port not specified')
    if !is_string(host) 
      raise TypeError('string expected for host, ${typeof(host)} given')
    if !is_int(port) 
      raise TypeError('integer expected for port, ${typeof(port)} given')
    if !is_string(message) and !is_bytes(message) and !is_file(message) 
      raise TypeError('message must string, bytes or file; ${typeof(message)} given')
    if !is_int(flags) 
      raise TypeError('integer expected for flags, ${typeof(flags)} given')

    if self.id == -1 or self.is_closed or (self.is_shutdown and 
      (self.shutdown_reason == SHUT_WR or 
        self.shutdown_reason == SHUT_RDWR)) 
      raise SocketException('socket is in an illegal state')

    return self._check_error(_socket.sendto(self.id, message, flags, host, port, family))
  }

  /**
   * Receives bytes of the given length from the socket. If the length is not given, it default length of 
   * -1 indicating that the total available data on the socket stream will be read. 
   * If no data is available for read on the socket, the socket will wait to receive data or until the 
   * `receive_timeout` which is also equal to the `SO_RCVTIMEO` setting of the socket has elapsed before or 
   * until it has received the total number of bytes required (whichever comes first).
   * 
   * @param int? length
   * @param int? flags: Not currently used.
   * @returns string
   */
  receive(length, flags) {
    if !length length = -1
    if !flags flags = 0

    if !is_int(length) 
      raise TypeError('integer expected for length, ${typeof(length)} given')
    if !is_int(flags) 
      raise TypeError('integer expected for flags, ${typeof(flags)} given')

    if self.id == -1 or self.is_closed or (self.is_shutdown and 
      (self.shutdown_reason == SHUT_RD or 
        self.shutdown_reason == SHUT_RDWR)) 
      raise SocketException('socket is in an illegal state')

    if !self.is_listening and !self.is_connected
      raise SocketException('socket not listening or connected')
    
    var result = _socket.recv(self.id, length, flags)
    if is_string(result) or result == nil return result

    return self._check_error(result)
  }

  /**
   * Receives bytes of the given length from the socket and returns the a dictionary that contains:
   * 
   * - string `data`: The received data.
   * - string `host`: The host address of the peer.
   * - int `port`: The port number of the peer.
   * 
   * @note The receive_from() method is especially useful for UDP sockets.
   * 
   * @param int? length
   * @param int? flags: Not currently used.
   * @returns {dict}
   * @throws SocketException
   */
  receive_from(length, flags) {
    if !length length = -1
    if !flags flags = 0

    if !is_int(length) 
      raise TypeError('integer expected for length, ${typeof(length)} given')
    if !is_int(flags) 
      raise TypeError('integer expected for flags, ${typeof(flags)} given')

    var result = _socket.recvfrom(self.id, length, flags)
    if result == -1 {
      return self._check_error(result)
    } else if result == nil {
      raise SocketException('receive_from timed out')
    }

    return {
      data: result[0],
      host: result[1],
      port: result[2],
    }
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
   * 
   * @param int? length: Default value is `1024`
   * @returns string
   */
  read(length) {
    if !length length = 1024

    if !is_int(length) 
      raise TypeError('integer expected for length, ${typeof(length)} given')

    if self.id == -1 or self.is_closed or (self.is_shutdown and 
      (self.shutdown_reason == SHUT_RD or 
        self.shutdown_reason == SHUT_RDWR)) 
      raise SocketException('socket is in an illegal state')

    if !self.is_listening and !self.is_connected
      raise SocketException('socket not listening or connected')
    
    var result = _socket.read(self.id, length, 0)
    if is_string(result) or result == nil return result

    return self._check_error(result)
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
   * @note listen() call applies only to sockets of type `SOCK_STREAM` (which is the default)
   * 
   * @param int? queue_length
   * @returns bool
   */
  listen(queue_length) {
    if !queue_length queue_length = SOMAXCONN # default to 128 simultaneous clients...

    if !is_int(queue_length) 
      raise TypeError('integer expected for queue_length, ${typeof(queue_length)} given')
    if queue_length > SOMAXCONN 
      raise ValueError('maximum queue length exceeded')

    if !self.is_bound or self.is_listening or self.is_closed 
      raise SocketException('socket is in an illegal state')

    var result = self._check_error(_socket.listen(self.id, queue_length))
    if result {
      self.is_listening = true
    }

    return result == 0
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
   * The accepted socket may not be used to accept more connections.
   * The original socket remains open.
   * 
   * @returns Socket
   */
  accept() {
    if self.is_bound and self.is_listening and !self.is_closed {
      var result = _socket.accept(self.id)

      if result and result != -1  {
        var socket = Socket(self.family, self.type, self.protocol, result[0])
        socket.host = result[1]
        socket.port = result[2]
        socket.is_client = true
        socket.is_connected = true
        socket.is_shutdown = false
        socket.is_closed = false
        socket.is_bound = false
        socket.is_listening = false
        return socket
      }
    }

    raise SocketException('socket not bound/listening')
  }

  /**
   * Closes the socket.
   * 
   * @returns bool
   */
  close() {
    # silently ignore multiple calls to close()
    if self.is_closed return true
    
    if self._check_error(_socket.close(self.id)) == 0 {
      self.is_connected = false
      self.is_listening = false
      self.is_bound = false
      self.is_client = false # may be reused as a server...
      self.is_closed = true
      return true
    }
    
    return false
  }

  /**
   * The shutdown() call causes all or part of a full-duplex connection on the socket associated with 
   * socket to be shut down.  If how is `SHUT_RD`, further receives will be disallowed.  If how is `SHUT_WR`, 
   * further sends will be disallowed.  If how is `SHUT_RDWR`, further sends and receives will be disallowed.
   * 
   * When _how_ is not specified, it defaults to `SHUT_RD`.
   * 
   * @param int? how
   * @returns bool
   */
  shutdown(how) {
    if !how how = SHUT_RD
    
    if !is_int(how) 
      raise TypeError('integer expected for how, ${typeof(how)} given')

    if how < SHUT_RD or how > SHUT_RDWR
      raise ValueError('expected one of SHUT_* flags')

    # consecutive call to the same shutdown type should be ignored
    if self.is_shutdown and self.shutdown_reason == how return true

    if self.is_closed raise SocketException('socket is in an illegal state')

    self.is_connected = false
    self.is_listening = false
    self.is_bound = false
    self.is_client = false # may be reused as a server...
    self.is_shutdown = true

    var result = self._check_error(_socket.shutdown(self.id, how)) >= 0
    if result {
      self.shutdown_reason = how
    }
    
    return result == 0
  }

  /**
   * Sets the options of the current socket.
   * 
   * @note Only `SO_` variables are valid option types.
   * 
   * @param int option
   * @param any value
   * @returns bool
   */
  set_option(option, value) {
    if !option or !value 
      raise ArgumentError('both option and value are required')
    if !is_int(option) 
      raise TypeError('integer expected for option, ${typeof(option)} given')

    if option == SO_TYPE or option == SO_ERROR
      raise ValueError('the given option is read-only')

    if !self.is_closed and !self.is_shutdown {
      var result = self._check_error(_socket.setsockopt(self.id, option, value)) >= 0

      if result {
        # get an update on SO_SNDTIMEO and SO_RCVTIMEO
        if option == SO_SNDTIMEO self.send_timeout = value
        else if option == SO_RCVTIMEO self.receive_timeout = value
      }

      return result == 0
    }

    return false
  }

  /**
   * Gets the options set on the current socket
   * 
   * @param int option
   * @returns any
   */
  get_option(option) {
    if !is_int(option) 
      raise TypeError('integer expected for option, ${typeof(option)} given')

    # we have a local copy of SO_RCVTIMEO and SO_SNDTIMEO
    # we can simply return them when required
    if option == SO_RCVTIMEO return self.receive_timeout
    else if option == SO_SNDTIMEO return self.send_timeout

    return _socket.getsockopt(self.id, option)
  }

  /**
   * Sets if the socket should operate in blocking or non-blocking mode. `true` for blocking 
   * (default) and `false` for non-blocking. The method returns `true` if the operation succeeded or 
   * `false` otherwise. 
   * 
   * @param {bool} mode
   * @returns {bool}
   */
  set_blocking(mode) {
    if !is_bool(mode) raise TypeError('boolean expected')
    self.is_blocking = mode
    return _socket.setblocking(self.id, mode)
  }

  /**
   * Returns a dictionary containing the address, ipv6, port and family of the current 
   * socket or an empty dictionary if the socket information could not be retrieved.
   * 
   * @returns dictionary
   */
  info() {
    return _socket.getsockinfo(self.id)
  }

  @to_string() {
    return '<Socket id: ${self.id}, closed: ${self.is_closed}, listening: ' +
        '${self.is_listening}, connected: ${self.is_connected}, bound: ${self.is_bound}>'
  }
}

/**
 * Returns ip and name information of a given address.
 * 
 * @param number address
 * @param string? type: Default value is `http`
 * @param int? family: Default value is [AF_INET]
 * @returns dictionary
 */
def get_address_info(address, type, family) {
  if !is_string(address)
    raise TypeError('string expected for address, ${typeof(address)} given')
  if type != nil and !is_string(type)
    raise TypeError('string expected for type, ${typeof(type)} given')

  if !type type = 'http'
  if !family family = AF_INET

  return _socket.getaddrinfo(address, type, family)
}

/**
 * Returns a new instance of a Socket.
 * 
 * @param number family
 * @param number? type
 * @param number? protocol
 * @returns Socket
 * 
 * @example socket(AF_INET, SOCK_STREAM, IPPROTO_TCP)
 * @default
 */
def socket(family, type, protocol) {
  return Socket(family, type, protocol)
}
