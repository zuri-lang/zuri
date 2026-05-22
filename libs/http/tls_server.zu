#!-- part of the ssl module

import .request { HttpRequest }
import .response { HttpResponse }
import .exception { HttpException }
import .server { HttpServer }
import .status
import .defaults

import socket as so
import ssl.constants
import ssl.socket { TLSSocket }

/**
 * TLS server
 * 
 * @printable
 */
class TLSServer < HttpServer {

  /**
   * The SSL/TLS certificate file that will be used be used by a secured server for
   * serving requests.
   * 
   * @type string
   * @note do not set a value to it directly. Use `load_certs()` instead.
   */
  var cert_file

  /**
   * The SSL/TLS private key file that will be used be used by a secured server for 
   * serving requests.
   * 
   * @type string
   * @note do not set a value to it directly. Use `load_certs()` instead.
   */
  var private_key_file

  /**
   * This value controls whether the client certificate should be verified 
   * or not.
   * 
   * @type boolean
   */
  var verify_certs = true

  /**
   * @param int port
   * @param string? host
   * @constructor
   */
  TLSServer(port, host) {

    if !is_int(port) or port <= 0 {
      raise ValueError('invalid port number')
    } else {
      self.port = port
    }

    if host != nil and !is_string(host) {
      raise ValueError('invalid host')
    } else if host != nil {
      self.host = host
    }

    self.socket = TLSSocket()
  }

  /**
   * Loads the given SSL/TLS certificate pairs for the given SSL/TLS context.
   * 
   * @param string|file cert_file
   * @param string|file|nil private_key_file
   * @returns bool
   */
  load_certs(cert_file, private_key_file) {
    if !private_key_file private_key_file = cert_file

    self.socket.get_context().set_verify(self.verify_certs ? constants.SSL_VERIFY_PEER : constants.SSL_VERIFY_NONE)

    if self.socket.get_context().load_certs(cert_file, private_key_file) {
      self.cert_file = cert_file
      self.private_key_file = private_key_file

      return self.socket.get_context().set_ciphers(defaults.ciphers)
    } else {
      # raise Exception('could not load certificate(s)')
      return false
    }
  }

  /**
   * Binds to the instance port and host and starts listening for incoming 
   * connection from HTTPS clients.
   */
  listen() {
    if !self.cert_file
      raise HttpException('no certificate loaded for secure server')
    if !self.private_key_file 
      raise HttpException('no private key loaded for secure server')

    parent.listen()
  }

  @to_string() {
    return '<TLSServer ${self.host}:${self.port}>'
  }
}

