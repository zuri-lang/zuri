import curl { 
  Option, 
  Info, 
  Curl, 
  CurlList, 
  CurlMime
}
import .message {
  Message
}
import .constants

var _flags_regex = '/\((\\\?([^)]+))?\)/'
var _list_regex = '/^[*] LIST \(\\\?([^)]*)\) "([^"]+)" (.*)$/'

/**
 * The POP3 class provides an interface for connecting to a POP3 (Post Office Protocol)
 * server and interacting with it via the POP3 protocol.
 * 
 * This class includes operations for listing, retrieving and deleting messages,
 * checking mailbox statistics, and performing keep-alive pings.
 * 
 * Configuration is applied through chainable setter methods. Only the server
 * __host__ and __port__ are accepted at construction time.
 * 
 * ### Example
 * 
 * ```blade
 * import mail
 * 
 * var client = mail.POP3('pop3.example.com', 995)
 *   .auth('user@example.com', 's3cr3t')
 *   .tls(mail.TLS_ALL)
 * 
 * echo client.stat()
 * client.quit()
 * ```
 */
class POP3 {
  var _username
  var _password
  var _tls = constants.TLS_TRY
  var _debug = false
  var _verify_peer = false
  var _verify_host = false
  var _proxy
  var _proxy_user
  var _proxy_pass
  var _verify_proxy_peer = false
  var _verify_proxy_host = false
  var _timeout = 30000  # in milliseconds (default = 30s)

  var _curl
  var _base_url

  /**
   * Creates a new POP3 client that will connect to __host__ on __port__.
   * 
   * All further configuration (credentials, TLS mode, proxy, timeouts, etc.)
   * is applied via the chainable setter methods on this class.
   * 
   * @param string host The hostname or IP address of the POP3 server. (Default: `'localhost'`)
   * @param number port The port number of the POP3 server. (Default: `110`)
   * @constructor
   */
  POP3(host, port) {
    if host != nil and !is_string(host)
      raise TypeError('string expected in argument 1 (host)')
    if port != nil and !is_number(port)
      raise TypeError('number expected in argument 2 (port)')

    self._host = host ? host : 'localhost'
    self._port = port ? port : 110
    self._base_url = '${self._tls == constants.TLS_ALL ? "pop3s" : "pop3"}://${self._host}:${self._port}'
  }

  _init(url) {
    if !url url = self._base_url
    else url = self._base_url + url

    var curl = self._curl

    if !curl {
      curl = Curl()

      curl.set_option(Option.USE_SSL, self._tls)
      curl.set_option(Option.USERNAME, self._username)
      curl.set_option(Option.PASSWORD, self._password)
      curl.set_option(Option.SSL_VERIFYPEER, self._verify_peer)
      curl.set_option(Option.SSL_VERIFYHOST, self._verify_host)
      curl.set_option(Option.VERBOSE, self._debug)
      curl.set_option(Option.TIMEOUT_MS, self._timeout)
  
      if self._proxy {
        curl.set_option(Option.PROXY, self._proxy)
        if self._proxy_user curl.set_option(Option.PROXYUSERNAME, self._proxy_user)
        if self._proxy_pass curl.set_option(Option.PROXYPASSWORD, self._proxy_pass)
        curl.set_option(Option.PROXY_SSL_VERIFYPEER, self._verify_proxy_peer)
        curl.set_option(Option.PROXY_SSL_VERIFYHOST, self._verify_proxy_host)
      }

      self._curl = curl
    }

    curl.set_option(Option.URL, url)

    return curl
  }

  _to_list(data, type, name) {
    return data.trim().split('\n').reduce(@(initial, x) {
      var data = x.trim().split(' ')

      initial.append({
        uid: to_number(data[0]),
        size: to_number(data[1]),
      })
      
      return initial
    }, [])
  }

  _to_uid_list(data, type, name) {
    return data.trim().split('\n').reduce(@(initial, x) {
      var data = x.trim().split(' ')

      initial.append({
        uid: to_number(data[0]),
        id: data[1],
      })
      
      return initial
    }, [])
  }

  /**
   * Sets the credentials used to authenticate with the POP3 server.
   * 
   * @param string username The login username for the POP3 account.
   * @param string password The password for the POP3 account.
   * @returns POP3
   */
  auth(username, password) {
    if !is_string(username)
      raise TypeError('string expected in argument 1 (username)')
    if !is_string(password)
      raise TypeError('string expected in argument 2 (password)')

    self._username = username
    self._password = password
    return self
  }

  /**
   * Sets the credentials used to authenticate with the proxy server.
   * 
   * This method has no effect unless a proxy address has been configured via
   * [[POP3.proxy]].
   * 
   * @param string username The login username for the proxy account.
   * @param string password The password for the proxy account.
   * @returns POP3
   */
  proxy_auth(username, password) {
    if !is_string(username)
      raise TypeError('string expected in argument 1 (username)')
    if !is_string(password)
      raise TypeError('string expected in argument 2 (password)')

    self._proxy_user = username
    self._proxy_pass = password
    return self
  }

  /**
   * Sets the TLS mode for the POP3 connection.
   * 
   * Accepted values are the `TLS_*` constants exported by this module:
   * [[mail.TLS_TRY]] (default), [[mail.TLS_CONTROL]], [[mail.TLS_ALL]], or
   * [[mail.TLS_NONE]].
   * 
   * @param number mode One of the `mail.TLS_*` constants.
   * @returns POP3
   */
  tls(mode) {
    if !is_number(mode)
      raise TypeError('number expected in argument 1 (mode)')
    self._tls = mode
    self._base_url = '${self._tls == constants.TLS_ALL ? "pop3s" : "pop3"}://${self._host}:${self._port}'
    return self
  }

  /**
   * Enables or disables verbose debug output for the underlying transport.
   * 
   * When enabled, low-level connection details are printed to standard output,
   * which is useful when diagnosing connectivity issues.
   * 
   * @param bool enabled `true` to enable debug output, `false` to disable it. (Default: `false`)
   * @returns POP3
   */
  debug(enabled) {
    if enabled != nil and !is_bool(enabled)
      raise TypeError('boolean expected in argument 1 (enabled)')
    self._debug = enabled == nil ? true : enabled
    return self
  }

  /**
   * Controls whether the remote server's SSL/TLS peer certificate is verified.
   * 
   * Disabling verification is not recommended in production environments as it
   * opens the connection to man-in-the-middle attacks.
   * 
   * @param bool enabled `true` to verify the peer certificate. (Default: `false`)
   * @returns POP3
   */
  verify_peer(enabled) {
    if enabled != nil and !is_bool(enabled)
      raise TypeError('boolean expected in argument 1 (enabled)')
    self._verify_peer = enabled == nil ? true : enabled
    return self
  }

  /**
   * Controls whether the remote server's SSL/TLS hostname is verified against
   * the certificate's Common Name or Subject Alternative Names.
   * 
   * @param bool enabled `true` to verify the host certificate. (Default: `false`)
   * @returns POP3
   */
  verify_host(enabled) {
    if enabled != nil and !is_bool(enabled)
      raise TypeError('boolean expected in argument 1 (enabled)')
    self._verify_host = enabled == nil ? true : enabled
    return self
  }

  /**
   * Configures an HTTP or SOCKS proxy through which all POP3 traffic is routed.
   * 
   * Provide proxy credentials separately via [[POP3.proxy_auth]].
   * 
   * @param string address The full proxy URL, e.g. `'http://proxy.example.com:8080'`.
   * @returns POP3
   */
  proxy(address) {
    if !is_string(address)
      raise TypeError('string expected in argument 1 (address)')
    self._proxy = address
    return self
  }

  /**
   * Controls whether the proxy server's SSL/TLS peer certificate is verified.
   * 
   * When not explicitly set, this inherits the value of [[POP3.verify_peer]].
   * 
   * @param bool enabled `true` to verify the proxy peer certificate.
   * @returns POP3
   */
  verify_proxy_peer(enabled) {
    if enabled != nil and !is_bool(enabled)
      raise TypeError('boolean expected in argument 1 (enabled)')
    self._verify_proxy_peer = enabled == nil ? true : enabled
    return self
  }

  /**
   * Controls whether the proxy server's SSL/TLS hostname is verified.
   * 
   * When not explicitly set, this inherits the value of [[POP3.verify_host]].
   * 
   * @param bool enabled `true` to verify the proxy host certificate.
   * @returns POP3
   */
  verify_proxy_host(enabled) {
    if enabled != nil and !is_bool(enabled)
      raise TypeError('boolean expected in argument 1 (enabled)')
    self._verify_proxy_host = enabled == nil ? true : enabled
    return self
  }

  /**
   * Sets the maximum time, in milliseconds, to wait for the server to respond
   * before aborting the connection.
   * 
   * @param number ms Timeout duration in milliseconds. (Default: `30000`)
   * @returns POP3
   */
  timeout(ms) {
    if !is_number(ms)
      raise TypeError('number expected in argument 1 (ms)')
    self._timeout = ms
    return self
  }

  /**
   * Executes a raw POP3 command.
   * 
   * @param string command The command to execute.
   * @param string? path The path segment of the request url.
   * @param bool? no_transfer Set to `true` if the command will return the requested data 
   *    as response response. Default `false`.
   * @returns string The response from the server.
   */
  exec(command, path, no_transfer) {
    if command != nil and !is_string(command)
      raise TypeError('string expected in argument 1 (command)')
    if path != nil and !is_string(path)
      raise TypeError('string expected in argument 2 (path)')
    if no_transfer != nil and !is_bool(no_transfer)
      raise TypeError('boolean expected in argument 3 (no_transfer)')

    var curl = self._init(path)
    curl.set_option(Option.CUSTOMREQUEST, command)

    if no_transfer {
      curl.set_option(Option.NOBODY, true)
    }

    return curl.send().body.to_string()
  }

  /**
   * Returns a list of dictionaries containing the `uid` and `size` of each message in the 
   * mail if the _uid_ argument is not given or the content of the message identified by the 
   * given _uid_.
   * 
   * @param number? uid
   * @returns list[dictionary]|string
   */
  list(uid) {
    if uid != nil and !is_number(uid)
      raise TypeError('number expected at argument 1 (uid)')

    return uid == nil ? 
      self._to_list(self._init().send().body.to_string()) :
      self._init('/${uid}').send().body.to_string()
  }

  /**
   * Returns a list of dictionaries containing the `uid` and `id` for every message in the mailbox 
   * based on their unique ids.
   * 
   * @returns list[dictionary]
   */
  uid_list() {
    return self._to_uid_list(self.exec('UIDL'))
  }

  /**
   * Retrieves the whole message with the specified _uid_.
   * 
   * @param number uid
   * @returns string
   */
  retr(uid) {
    if !is_number(uid)
      raise TypeError('number expected in argument 1 (uid)')
    
    return self._init('/${uid}').send().body.to_string()
  }

  /**
   * Returns a dictionary containing the message `count` and `size` of the mailbox.
   * 
   * @returns dictionary
   */
  stat() {
    var curl = self._init()
    curl.set_option(Option.CUSTOMREQUEST, 'STAT')
    curl.set_option(Option.NOBODY, true)

    var data = curl.send().headers.trim().split('\r\n').last().split(' ')
    
    if data[0].lower() == '+ok' {
      return {
        count: to_number(data[1]),
        size: to_number(data[2])
      }
    }

    raise Exception(' '.join(data))
  }

  /**
   * Instructs the POP3 server to mark the message _uid_ as deleted. Any future reference 
   * to the message-number associated with the message in a POP3 command generates an error.  
   * The POP3 server does not actually delete the message until the POP3 session enters the 
   * UPDATE state.
   * 
   * @param number uid
   */
  delete(uid) {
    if !is_string(uid)
      raise TypeError('string expected in argument 1 (uid)')

    self.exec('DELE', uid, true)
  }

  /**
   * Does nothing. It merely asks the server to reply with a positive response.
   * 
   * @note It's useful for a keep-alive.
   */
  noop() {
    return self.exec('NOOP', nil, true)
  }

  /**
   * Instructs the server to unmark any messages that have been marked as deleted.
   */
  rset() {
    return self.exec('RSET', nil, true)
  }

  /**
   * Retrieves the header for the message identified by `uid` plus `count` lines 
   * of the message after the header of message.
   * 
   * @param number uid
   * @param number? count (Default: 0)
   * @returns string
   */
  top(uid, count) {
    if !is_number(uid)
      raise TypeError('number expected in argument 1 (uid)')
    if count != nil and !is_number(count)
      raise TypeError('number expected in argument 2 (count)')

    if !count count = 0
    return self.exec('TOP ${uid} ${count}')
  }

  /**
   * Closes the current POP3 session and disposes all associated network handles.
   */
  quit() {
    if self._curl {
      self._curl.close()
    }
  }

  /**
   * Returns the raw handle to the underlying networking (curl) client.
   */
  get_handle() {
    return self._curl
  }
}


/**
 * Returns a new [[mail.POP3]] instance configured to connect to __host__ on __port__.
 * 
 * This is a convenience factory equivalent to `POP3(host, port)`.
 * 
 * @param string host The hostname or IP address of the POP3 server. (Default: `'localhost'`)
 * @param number port The port number of the POP3 server. (Default: `110`)
 * @returns POP3
 * @default
 */
def pop3(host, port) {
  return POP3(host, port)
}
