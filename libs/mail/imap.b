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
 * The IMAP class provides an interface for connecting to an IMAP (Internet Mail Access Protocol)
 * server and interacting with it via the IMAP protocol.
 * 
 * This class includes operations for creating, deleting, and renaming mailboxes, checking for new
 * messages, permanently removing messages, setting and clearing flags, searching, and selective
 * fetching of message attributes, texts, and portions.
 * 
 * Configuration is applied through chainable setter methods. Only the server
 * __host__ and __port__ are accepted at construction time.
 * 
 * ### Example
 * 
 * ```blade
 * import mail
 * 
 * var client = mail.IMAP('imap.example.com', 993)
 *   .auth('user@example.com', 's3cr3t')
 *   .tls(mail.TLS_ALL)
 * 
 * var messages = client.search('UNSEEN', 'INBOX')
 * client.quit()
 * ```
 */
class IMAP {
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
   * Creates a new IMAP client that will connect to __host__ on __port__.
   * 
   * All further configuration (credentials, TLS mode, proxy, timeouts, etc.)
   * is applied via the chainable setter methods on this class.
   * 
   * @param string host The hostname or IP address of the IMAP server. (Default: `'localhost'`)
   * @param number port The port number of the IMAP server. (Default: `143`)
   * @constructor
   */
  IMAP(host, port) {
    if host != nil and !is_string(host)
      raise TypeError('string expected in argument 1 (host)')
    if port != nil and !is_number(port)
      raise TypeError('number expected in argument 2 (port)')

    self._host = host ? host : 'localhost'
    self._port = port ? port : 143
    self._base_url = '${self._tls == constants.TLS_ALL ? "imaps" : "imap"}://${self._host}:${self._port}'
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
    var list = []
    var lines = data.split('\n')

    for line in lines {
      using type {
        when 'list' {
            var match = line.trim().match(_list_regex)
            if match {

              # handle cases where some servers wrap the name in quotations
              match[3] = match[3].trim('"')

              list.append({
                name: match[3],
                separator: match[2],
                flags: match[1].split('/ \\\?/'),
                path: match[3].replace(match[2], '/'),
                is_parent: name ? match[3] == name.trim('/') : false,
              })
            }
        }
        when 'search' {
          line = line.replace('/\r?\n/', '').trim().replace('* SEARCH ', '', false)
          if line {
            list.extend(line.split(' '))
          }
        }
      }
    }

    return list
  }

  _examine(data) {
    var lines = data.split('/\r?\n/')

    var result = {
      flags: [],
      permanent_flags: [],
      mails: 0,
      recents: 0,
      uid_validity: 0,
      next_uid: 0,
      highest_mod_seq: 0,
      comment: nil,
    }

    for line in lines {
      if line.starts_with('* FLAGS') {
        result.flags = line.match(_flags_regex)[2].split('/ \\\?/')
      } else if line.starts_with('* OK [PERMANENTFLAGS') {
        var matches = line.match(_flags_regex)
        if matches.length() > 1 {
          result.permanent_flags = matches[1].trim('\\').split('/ \\\?/')
        }
        result.comment = line.split('] ')[1]
      } else if line.ends_with(' EXISTS') {
        result.mails = to_number(line[2, line.index_of(' EXISTS')])
      } else if line.ends_with(' RECENT') {
        result.recents = to_number(line[2, line.index_of(' RECENT')])
      } else if line.starts_with('* OK [UIDVALIDITY') {
        result.uid_validity = to_number(line[18, line.index_of(']', 18)])
      } else if line.starts_with('* OK [UIDNEXT') {
        result.next_uid = to_number(line[14, line.index_of(']', 18)])
      } else if line.starts_with('* OK [HIGHESTMODSEQ') {
        result.highest_mod_seq = to_number(line[20, line.index_of(']', 18)])
      }
    }

    return result
  }

  /**
   * Sets the credentials used to authenticate with the IMAP server.
   * 
   * @param string username The login username for the IMAP account.
   * @param string password The password for the IMAP account.
   * @returns IMAP
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
   * [[IMAP.proxy]].
   * 
   * @param string username The login username for the proxy account.
   * @param string password The password for the proxy account.
   * @returns IMAP
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
   * Sets the TLS mode for the IMAP connection.
   * 
   * Accepted values are the `TLS_*` constants exported by this module:
   * [[mail.TLS_TRY]] (default), [[mail.TLS_CONTROL]], [[mail.TLS_ALL]], or
   * [[mail.TLS_NONE]].
   * 
   * @param number mode One of the `mail.TLS_*` constants.
   * @returns IMAP
   */
  tls(mode) {
    if !is_number(mode)
      raise TypeError('number expected in argument 1 (mode)')
    self._tls = mode
    self._base_url = '${self._tls == constants.TLS_ALL ? "imaps" : "imap"}://${self._host}:${self._port}'
    return self
  }

  /**
   * Enables or disables verbose debug output for the underlying transport.
   * 
   * When enabled, low-level connection details are printed to standard output,
   * which is useful when diagnosing connectivity issues.
   * 
   * @param bool enabled `true` to enable debug output, `false` to disable it. (Default: `false`)
   * @returns IMAP
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
   * @returns IMAP
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
   * @returns IMAP
   */
  verify_host(enabled) {
    if enabled != nil and !is_bool(enabled)
      raise TypeError('boolean expected in argument 1 (enabled)')
    self._verify_host = enabled == nil ? true : enabled
    return self
  }

  /**
   * Configures an HTTP or SOCKS proxy through which all IMAP traffic is routed.
   * 
   * Provide proxy credentials separately via [[IMAP.proxy_auth]].
   * 
   * @param string address The full proxy URL, e.g. `'http://proxy.example.com:8080'`.
   * @returns IMAP
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
   * When not explicitly set, this inherits the value of [[IMAP.verify_peer]].
   * 
   * @param bool enabled `true` to verify the proxy peer certificate.
   * @returns IMAP
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
   * When not explicitly set, this inherits the value of [[IMAP.verify_host]].
   * 
   * @param bool enabled `true` to verify the proxy host certificate.
   * @returns IMAP
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
   * @returns IMAP
   */
  timeout(ms) {
    if !is_number(ms)
      raise TypeError('number expected in argument 1 (ms)')
    self._timeout = ms
    return self
  }

  /**
   * Executes a raw IMAP command.
   * 
   * @param string command The command to execute.
   * @param string? path The path segment of the request url.
   * @returns string The response from the server.
   */
  exec(command, path) {
    if command != nil and !is_string(command)
      raise TypeError('string expected in argument 1 (command)')
    if path != nil and !is_string(path)
      raise TypeError('string expected in argument 2 (path)')

    var curl = self._init(path)
    curl.set_option(Option.CUSTOMREQUEST, command)
    return curl.send().body.to_string()
  }

  /**
   * Gets a list of the mailbox directories on the server.
   * 
   * @param string? path
   * @returns list
   */
  get_dirs(path) {
    if !is_string(path)
      raise TypeError('string expected in argument 1 (path)')
    return self._to_list(self.exec(nil, path), 'list', path)
  }

  /**
   * Gets a list of mailbox directories subscribed to by the current 
   * user on the server.
   * 
   * @returns list
   */
  get_subscribed_dirs() {
    return self._to_list(self.exec('LSUB "" *'), 'list')
  }

  /**
   * Instructs the server that the client now wishes to select a particular mailbox or folder 
   * with the name _name_, and any commands that relate to a folder should assume this folder 
   * as the target of that command. For example, an INBOX or a subfolder such as, 
   * "To Do.This Weekend". Once a mailbox is selected, the state of the connection becomes 
   * "Selected".
   * 
   * @see https://www.marshallsoft.com/ImapSearch.htm for more help.
   * @param string name
   * @returns dictionary
   */
  select(name) {
    if !name raise ArgumentError('name required')
    return self._examine(self.exec('SELECT ${name}'))
  }

  /**
   * This function does the exact same thing as `select()`, except that it selects the folder 
   * in read-only mode, meaning that no changes can be effected on the folder.
   * 
   * @param string name
   * @returns dictionary
   */
  examine(name) {
    if !name raise ArgumentError('name required')
    return self._examine(self.exec('EXAMINE ${name}'))
  }

  /**
   * Creates a new mailbox or folder with the given name.
   * 
   * @param string name
   * @returns list
   */
  create(name) {
    if !name raise ArgumentError('name required')
    return self._to_list(self.exec('CREATE ${name}'))
  }

  /**
   * Deletes the mailbox or folder with the given name.
   * 
   * @param string name
   * @returns list
   */
  delete(name) {
    if !name raise ArgumentError('name required')
    return self._to_list(self.exec('DELETE ${name}'))
  }

  /**
   * Renames a mailbox or folder with the name `old_name` to a the name `new_name`.
   * 
   * @param string old_name
   * @param string new_name
   * @returns list
   */
  rename(old_name, new_name) {
    if !old_name or !new_name raise ArgumentError('old and new name required')
    return self._to_list(self.exec('RENAME ${old_name} ${new_name}'))
  }

  /**
   * Adds the specified mailbox name to the server's set of "active" or "subscribed" 
   * mailboxes for the current user as returned by `lsub()` and returns `true` if 
   * successful or `false` otherwise.
   * 
   * @param string name
   * @returns bool
   */
  subscribe(name) {
    if !name raise ArgumentError('name required')
    self.exec('SUBSCRIBE ${name}')
    return self._curl.get_info(Info.RESPONSE_CODE) == 250
  }

  /**
   * Removes the specified mailbox name from the server's set of "active" or "subscribed" 
   * mailboxes for the current user as returned by `lsub()` and returns `true` if successful 
   * or `false` otherwise.
   * 
   * @param string name
   * @returns bool
   */
  unsubscribe(name) {
    if !name raise ArgumentError('name required')
    self.exec('UNSUBSCRIBE ${name}')
    return self._curl.get_info(Info.RESPONSE_CODE) == 250
  }

  /**
   * Returns a subset of names from the complete set of all names available to the client. 
   * Zero or more dictionaries are returned, containing the name attributes, hierarchy delimiter, 
   * and name. 
   * 
   * An empty (`""` string) _name_ argument indicates that the mailbox name is interpreted 
   * as by SELECT. A non-empty _name_ argument is the name of a mailbox or a level of mailbox 
   * hierarchy, and indicates the context in which the mailbox name is interpreted. 
   * 
   * An empty (`""` string) pattern argument is a special request to return the hierarchy delimiter 
   * and the root name of the name given in the reference.
   * 
   * The pattern character `*` is a wildcard, and matches zero or more characters at this position.  
   * The character `%` is similar to `*`, but it does not match a hierarchy delimiter.  If the `%` 
   * wildcard is the last character of a pattern argument, matching levels of hierarchy are also 
   * returned.  If these levels of hierarchy are not also selectable mailboxes, they are returned 
   * with the `\Noselect` pattern attribute.
   * 
   * The special name `INBOX` is included in the output from `list()`, if `INBOX` is supported by 
   * the server for the current user and if the uppercase string "INBOX" matches the interpreted 
   * reference and pattern arguments with wildcards as described above.  The criteria for omitting 
   * INBOX is whether `select('INBOX')` will return failure; it is not relevant whether the user's 
   * real INBOX resides on the server or another.
   * 
   * @param string name
   * @param string? pattern
   * @returns list
   */
  list(name, pattern) {
    if !name raise ArgumentError('name required')
    if !pattern and !is_string(pattern) pattern = '%'
    return self._to_list(self.exec('LIST ${name} ${pattern}'))
  }

  /**
   * Same as the `list()` function except that it returns a subset of names.
   * 
   * @param string name
   * @param string? pattern
   * @returns list
   */
  lsub(name, pattern) {
    if !name raise ArgumentError('name required')
    if !pattern and !is_string(pattern) pattern = '%'
    return self._to_list(self.exec('LSUB ${name} ${pattern}'))
  }

  /**
   * Requests the status of the indicated mailbox. 
   * 
   * It is important to know that unlike the LIST command, the STATUS command is not 
   * guaranteed to be fast in its response.  Under certain circumstances, it can be 
   * quite slow.
   * 
   * `attrs` values being one of:
   * 
   * - `MESSAGES`: The number of messages in the mailbox.
   * - `RECENT`: The number of messages with the \Recent flag set.
   * - `UIDNEXT`: The next unique identifier value of the mailbox.
   * - `UIDVALIDITY`: The unique identifier validity value of the mailbox.
   * - `UNSEEN`: The number of messages which do not have the \Seen flag set.
   * 
   * `attrs` values may be separated by space. e.g. `status('INBOX', 'UIDNEXT MESSAGES')`.
   * 
   * @param string name
   * @param string attrs
   * @returns bool|string
   */
  status(name, attrs) {
    if !name raise ArgumentError('name required')
    if !attrs raise ArgumentError('attrs required')
    var result = self.exec('STATUS ${name} (${filter})')
    if self._curl.get_info(Info.RESPONSE_CODE) == 250 {
      var response = {}
      var response_split =  result.split('(')[1][,-1].split(' ')
      iter var i = 0; i < response_split.length(); i++ {
        response[response_split[i]] = response_split[i++]
      }
      return response
    }
    return false
  }

  /**
   * Appends messages to a mailbox directories such as INBOX or top-level folders 
   * and returns `true` if it succeeds or `false` otherwise.
   * 
   * > NOTE:
   *    This isn't a copy/move command, you must supply a full message body to 
   *    append.
   * @param string folder
   * @param Message message
   * @returns bool
   */
  append(folder, message) {
    if !instance_of(message, Message)
      raise TypeError('instance of Message expected in second argument')

    var examine_result = self.examine(folder)
    # var selection_result = self.select(folder)
    
    if examine_result {
      var curl = self._init('/' + folder)
      var mail = message.build(curl)

      curl.set_option(Option.HTTPHEADER, CurlList(mail.headers))
      curl.set_option(Option.MIMEPOST, mail.mime)

      # append the new email
      curl.send()

      # add response to results
      return curl.get_info(Info.RESPONSE_CODE) == 250
    }

    return false
  }

  /**
   * Requests a checkpoint of the currently selected mailbox.  A checkpoint refers to 
   * any implementation-dependent housekeeping associated with the mailbox (e.g., 
   * resolving the server's in-memory state of the mailbox with the state on its disk) 
   * that is not normally executed as part of each command.  A checkpoint MAY take a 
   * non-instantaneous amount of real time to complete.  
   * 
   * If a server implementation has no such housekeeping considerations, `check()` is 
   * equivalent to NOOP.
   * 
   * @returns bool
   */
  check() {
    self.exec('CHECK')
    return self._curl.get_info(Info.RESPONSE_CODE) == 250
  }

  /**
   * Permanently removes all messages that have the `\Deleted` flag set from the currently 
   * selected mailbox, and returns to the authenticated state from the selected state.
   * 
   * No messages are removed, and no error is given, if the mailbox is selected by an 
   * `examine()` or is otherwise selected read-only.
   * 
   * @returns bool
   */
  close() {
    self.exec('CLOSE')
    return self._curl.get_info(Info.RESPONSE_CODE) == 250
  }

  /**
   * Clears the deleted messages in a mailbox folder and returns `true` on 
   * success or `false` otherwise.
   * 
   * @param string path
   * @returns bool
   */
  expunge(path) {
    if !is_string(path)
      raise TypeError('string expected in argument 1 (path)')

    self.exec('EXPUNGE', path)
    return self._curl.get_info(Info.RESPONSE_CODE) == 250
  }

  /**
   * Finds all occurrences of the __query__ in the specified __folder__ and 
   * return a list of message UIDs that matches the search query.
   * 
   * The __query__ can contain a message sequence set and a number of search 
   * criteria keywords including flags such as ANSWERED, DELETED, DRAFT, FLAGGED, 
   * NEW, RECENT and SEEN. For more information about the search criteria please
   * see RFC-3501 section 6.4.4 for more details.
   * 
   * When __query__ is empty, it defaults to `NEW`. __folder__ defaults to `INBOX`
   *  when empty.
   * 
   * @see: https://datatracker.ietf.org/doc/html/rfc9051#section-6.4.4 for more.
   * @param string? query
   * @param string? folder
   */
  search(query, folder) {
    if query != nil and !is_string(query)
      raise TypeError('string expected in argument 1 (query)')
    if folder != nil and !is_string(folder)
      raise TypeError('string expected in argument 2 (folder)')

    if !folder folder = 'INBOX'
    if !query query = 'NEW'

    return self._to_list(self.exec('SEARCH ${query}', '/${folder}'), 'search')
  }

  /**
   * Retrieves a message with the give __uid__ in the specified mailbox __path__. If 
   * the __uid__ is not given, it attempts to retrieve the message with a UID of 1. If 
   * __path__ is not given, it will attempt to retrieve the message from the `INBOX` 
   * folder.
   * 
   * @param number? uid
   * @param string? path
   */
  fetch(uid, path) {
    if uid != nil and !is_number(uid)
      raise TypeError('number expected in argument 1 (uid)')
    if path != nil and !is_string(path)
      raise TypeError('string expected in argument 2 (path)')

    if !uid uid = 1
    if !path path = 'INBOX'
    return self.exec(nil, '/${path}/;UID=${uid}')
  }

  /**
   * Copies the specified message(s) to the end of the specified destination mailbox.
   * 
   * @note COPYUID responses are not yet supported
   * @returns bool
   */
  copy(id, destination, path) {
    if !id raise ArgumentError('id required')
    if !destination raise Exception('destination required')
    self.exec('COPY ${id} ${destination}', path)
    return self._curl.get_info(Info.RESPONSE_CODE) == 250
  }

  /**
   * Alters data associated with a message in the mailbox.
   * 
   * @note command must be one of `FLAGS`, `+FLAGS`, or `-FLAGS`, optionally with a 
   *    suffix of `.SILENT`.
   * @see https://datatracker.ietf.org/doc/html/rfc9051#section-6.4.6 for more.
   * @returns bool
   */
  store(id, command, flags) {
    if !id raise ArgumentError('id required')
    if !command or !command.match('^[+-]?FLAGS([.]SILENT)?')
      raise Exception('invalid command')
    if !flags raise ArgumentError('flags required')

    self.exec('STORE ${id} ${command.upper()} ${flags.upper()}', path)
    return self._curl.get_info(Info.RESPONSE_CODE) == 250
  }

  /**
   * Closes the current IMAP session and disposes all associated network handles.
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
 * Returns a new [[mail.IMAP]] instance configured to connect to __host__ on __port__.
 * 
 * This is a convenience factory equivalent to `IMAP(host, port)`.
 * 
 * @param string host The hostname or IP address of the IMAP server. (Default: `'localhost'`)
 * @param number port The port number of the IMAP server. (Default: `143`)
 * @returns IMAP
 * @default
 */
def imap(host, port) {
  return IMAP(host, port)
}
