import curl { 
  Option, 
  Info, 
  Curl, 
  CurlList
}
import .message {
  Message
}
import .constants

/**
 * SMTP class can be used to send email messages through an SMTP server.
 * 
 * Configuration is done via chainable setter methods rather than a constructor
 * dictionary, keeping the constructor simple and explicit. Only the server
 * __host__ and __port__ are accepted at construction time; all other options
 * are applied through the dedicated methods below.
 * 
 * ### Example
 * 
 * ```zuri
 * import mail
 * 
 * var msg = mail.Message()
 *    .from('someone@example.com')
 *    .to('hello@domain.com')
 *    .subject('Hello, World')
 *    .text('Welcome to Zuri Mail!')
 * 
 * var client = mail.SMTP('smtp.example.com', 465)
 *   .auth('user@example.com', 's3cr3t')
 *   .tls(mail.TLS_ALL)
 *   .debug(true)
 * 
 * client.add_message(msg).send()
 * ```
 */
class SMTP {
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
  
  var messages = []

  /**
   * Creates a new SMTP client that will connect to __host__ on __port__.
   * 
   * All further configuration (credentials, TLS mode, proxy, timeouts, etc.)
   * is applied via the chainable setter methods on this class.
   * 
   * @param string host The hostname or IP address of the SMTP server. (Default: `'localhost'`)
   * @param number port The port number of the SMTP server. (Default: `465`)
   * @constructor
   */
  SMTP(host, port) {
    if host != nil and !is_string(host)
      raise TypeError('string expected in argument 1 (host)')
    if port != nil and !is_number(port)
      raise TypeError('number expected in argument 2 (port)')

    self._host = host ? host : 'localhost'
    self._port = port ? port : 465
  }

  _init() {
    var curl = Curl()

    curl.set_option(Option.URL, '${self._tls == constants.TLS_NONE ? "smtp" : "smtps"}://${self._host}:${self._port}')
    curl.set_option(Option.USE_SSL, self._tls)
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

    return curl
  }

  /**
   * Sets the credentials used to authenticate with the SMTP server.
   * 
   * @param string username The login username for the SMTP account.
   * @param string password The password for the SMTP account.
   * @returns SMTP
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
   * [[SMTP.proxy]].
   * 
   * @param string username The login username for the proxy account.
   * @param string password The password for the proxy account.
   * @returns SMTP
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
   * Sets the TLS mode for the SMTP connection.
   * 
   * Accepted values are the `TLS_*` constants exported by this module:
   * [[mail.TLS_TRY]] (default), [[mail.TLS_CONTROL]], [[mail.TLS_ALL]], or
   * [[mail.TLS_NONE]].
   * 
   * @param number mode One of the `mail.TLS_*` constants.
   * @returns SMTP
   */
  tls(mode) {
    if !is_number(mode)
      raise TypeError('number expected in argument 1 (mode)')
    self._tls = mode
    return self
  }

  /**
   * Enables or disables verbose debug output for the underlying transport.
   * 
   * When enabled, low-level connection details are printed to standard output,
   * which is useful when diagnosing connectivity issues.
   * 
   * @param bool enabled `true` to enable debug output, `false` to disable it. (Default: `false`)
   * @returns SMTP
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
   * @returns SMTP
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
   * @returns SMTP
   */
  verify_host(enabled) {
    if enabled != nil and !is_bool(enabled)
      raise TypeError('boolean expected in argument 1 (enabled)')
    self._verify_host = enabled == nil ? true : enabled
    return self
  }

  /**
   * Configures an HTTP or SOCKS proxy through which all SMTP traffic is routed.
   * 
   * Provide proxy credentials separately via [[SMTP.proxy_auth]].
   * 
   * @param string address The full proxy URL, e.g. `'http://proxy.example.com:8080'`.
   * @returns SMTP
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
   * When not explicitly set, this inherits the value of [[SMTP.verify_peer]].
   * 
   * @param bool enabled `true` to verify the proxy peer certificate.
   * @returns SMTP
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
   * When not explicitly set, this inherits the value of [[SMTP.verify_host]].
   * 
   * @param bool enabled `true` to verify the proxy host certificate.
   * @returns SMTP
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
   * @returns SMTP
   */
  timeout(ms) {
    if !is_number(ms)
      raise TypeError('number expected in argument 1 (ms)')
    self._timeout = ms
    return self
  }

  /**
   * Adds an email message to the list of messages to be sent.
   * 
   * @param Message message
   * @returns SMTP
   */
  add_message(message) {
    if !instance_of(message, Message) {
      raise ValueError('instance of mail.Message expected')
    }

    self.messages.append(message)
    return self
  }

  /**
   * Tests the connection to the SMTP server.
   * 
   * @returns bool
   */
  test_connection() {
    var curl = self._init()
    curl.set_option(Option.CONNECT_ONLY, true)

    # send the email
    curl.send()

    return curl.get_info(Info.RESPONSE_CODE) == 250
  }

  /**
   * Verifies an email address against the SMTP server using the `VRFY` command.
   * 
   * @param string address
   * @returns bool
   */
  verify(address) {
    var curl = self._init()
    curl.set_option(Option.MAIL_RCPT, CurlList([address]))

    # send the VRFY and close
    curl.send()
    curl.close()

    return curl.get_info(Info.RESPONSE_CODE) == 252
  }

  /**
   * Sends all queued email messages and returns the SMTP response code(s).
   * 
   * If only one message was queued the return value is a single number;
   * if multiple messages were queued a list of response codes is returned,
   * one per message in the order they were added.
   * 
   * @returns number|list[number]
   */
  send() {
    var response_codes = []

    var curl = self._init()
    curl.set_option(Option.USERNAME, self._username)
    curl.set_option(Option.PASSWORD, self._password)

    for message in self.messages {
      var mail = message.build(curl)

      curl.set_option(Option.MAIL_FROM, mail.from)
      curl.set_option(Option.MAIL_RCPT, CurlList(mail.to))
      curl.set_option(Option.HTTPHEADER, CurlList(mail.headers))
      curl.set_option(Option.MIMEPOST, mail.mime)

      # send the email
      curl.send()

      # add response to results
      response_codes.append(curl.get_info(Info.RESPONSE_CODE))
    }

    # close the connection
    curl.close()

    # return single response if there was only one message or a list of 
    # response codes if more than one message was sent in the transport.
    return response_codes.length() == 1 ? response_codes[0] : response_codes
  }
}


/**
 * Returns a new [[mail.SMTP]] instance configured to connect to __host__ on __port__.
 * 
 * This is a convenience factory equivalent to `SMTP(host, port)`.
 * 
 * @param string host The hostname or IP address of the SMTP server. (Default: `'localhost'`)
 * @param number port The port number of the SMTP server. (Default: `465`)
 * @returns SMTP
 */
def smtp(host, port) {
  return SMTP(host, port)
}
