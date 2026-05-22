/**
 * @module url
 *  
 * This module provides classes and functions for parsing and processing URLs.
 * This module supports username and passwords in URLs in order to support an 
 * arbitrary number of RFC combinations but this does not strictly conform to 
 * RFC1738.
 * 
 * The scope of URL in this module have not been limited to HTTP or any protocol 
 * for that matter. However, where deductible, the module tries to conform to the
 * most appropriate URL for the specified scheme.
 * 
 * Constructing a URL is vey simple. Here is an example.
 * 
 * ### Example
 * 
 * ```zuri-repl
 * %> import url
 * %> var link = url.Url('https', 'example.com', 9000)
 * %> link.absolute_url()
 * 'https://example.com:9000'
 * ```
 * 
 * What each function and class method does are easy to deduce from their names.
 * 
 * For example, we can use the `parse()` function to convert a URL string into a URL 
 * instance like below.
 * 
 * ```zuri-repl
 * %> link = url.parse('https://example.com:9000')
 * %> link.scheme
 * 'https'
 * %> link.port
 * '9000'
 * ```
 * 
 * @copyright 2021, Richard Ore and Zuri contributors
 */

import types


/**
 * Exception thrown when a url is malformed
 */
class UrlMalformedException < Exception {
  /**
   * @param string message
   * @constructor
   */
  UrlMalformedException(message) {
    parent(message)
  }
}


# a list of schemes that does not conform to the standard ://
# after the scheme name in their urls
var _SIMPLE_SCHEMES = [
  'mailto', 'tel',  # lowercase
  'MAILTO', 'TEL',  # uppercase
]

var _ipv6_regex = '/(?:^|(?<=\\s))(([0-9a-fA-F]{1,4}:)' +
  '{7,7}[0-9a-fA-F]{1,4}|([0-9a-fA-F]{1,4}:){1,7}:|([0-9a-fA-F]' +
  '{1,4}:){1,6}:[0-9a-fA-F]{1,4}|([0-9a-fA-F]{1,4}:){1,5}(:[0-9a-fA-F]' + 
  '{1,4}){1,2}|([0-9a-fA-F]{1,4}:){1,4}(:[0-9a-fA-F]{1,4}){1,3}|([0-9a-fA-F]' + 
  '{1,4}:){1,3}(:[0-9a-fA-F]{1,4}){1,4}|([0-9a-fA-F]{1,4}:){1,2}(:[0-9a-fA-F]' +
  '{1,4}){1,5}|[0-9a-fA-F]{1,4}:((:[0-9a-fA-F]{1,4}){1,6})|:((:[0-9a-fA-F]{1,4})' +
  '{1,7}|:)|fe80:(:[0-9a-fA-F]{0,4}){0,4}%[0-9a-zA-Z]{1,}|::(ffff(:0{1,4}){0,1}:)' +
  '{0,1}((25[0-5]|(2[0-4]|1{0,1}[0-9]){0,1}[0-9])\\.){3,3}(25[0-5]|(2[0-4]|1{0,1}' +
  '[0-9]){0,1}[0-9])|([0-9a-fA-F]{1,4}:){1,4}:((25[0-5]|(2[0-4]|1{0,1}[0-9]){0,1}' +
  '[0-9])\\.){3,3}(25[0-5]|(2[0-4]|1{0,1}[0-9]){0,1}[0-9]))(?=\\s|$)/'

var _url_punctuations_re = '/^[\-._~:\/?#\[\]@!$&\'()*+,;%=]/'


/**
 * The Url class provides functionalities for parsing and processing URLs.
 *
 * @serializable
 * @printable
 */
class Url {

  /**
   * The url scheme e.g. http, https, ftp, tcp etc.
   */
  var scheme

  /**
   * The host information contained in the url
   */
  var host

  /**
   * The port information contained in the url whenever the url doesn't 
   * indicate, we try to make a best guess based on the scheme.
   */
  var port

  /**
   * The path of the URL.
   * @default /
   */
  var path = '/'

  /**
   * Hash information contained in the url and it's beginning is indicated by the 
   * hash (#) sign. This value is especially relevant to some http/https urls 
   * and are usually references to the content of the document 
   * at the given url
   */
  var hash

  /**
   * Query/Search information contained in the url and it's beginning is indicated by the 
   * question (?) sign. This value is especially relevant to some http/https urls and are 
   * usually used to convey data to endpoint based on the GET method.
   */
  var query

  /**
   * Username information for authentication are sometimes embedded in urls. When such information
   * exist, this property holds the information
   */
  var username

  /**
   * Password information for authentication are sometimes embedded in urls. When such information
   * exist, this property holds the information
   */
  var password

  /**
   * `true` if the url contains the :// section. `false` otherwise.
   */
  var has_slash = false

  /**
   * `true` if the original url contains a path segment even if its just an `/` and false if the
   * path value of `/` was implied.
   * @type bool
   */
  var empty_path = false

  /**
   * @param string scheme
   * @param string host
   * @param string? port
   * @param string? path
   * @param string? query
   * @param string? hash
   * @param string? username
   * @param string? password
   * @param bool? has_slash
   * @param bool? empty_path
   * @constructor 
   */
  Url(scheme, host, port, path, query, hash, username, password, has_slash, empty_path) {
    if scheme != nil and !is_string(scheme)
      raise TypeError('scheme must be a string')
    if host != nil and !is_string(host)
      raise TypeError('host must be a string')
    if port != nil and !is_string(port) and !is_int(port)
      raise TypeError('port must be a string or an integer')
    if path != nil and !is_string(path)
      raise TypeError('path must be a string')
    if query != nil and !is_string(query)
      raise TypeError('query must be a string')
    if hash != nil and !is_string(hash)
      raise TypeError('hash must be a string')
    if username != nil and !is_string(username)
      raise TypeError('username must be a string')
    if password != nil and !is_string(password)
      raise TypeError('password must be a string')
    if has_slash != nil and !is_bool(has_slash)
      raise TypeError('has_slash must be a boolean')
    if empty_path != nil and !is_bool(empty_path)
      raise TypeError('empty_path must be a boolean')

    if is_number(port) port = to_string(port)

    self.scheme = scheme
    self.host = host
    self.port = port
    self.path = path
    self.query = query
    self.hash = hash
    self.username = username
    self.password = password
    self.has_slash = has_slash
    self.empty_path = empty_path
  }

  /**
   * Returns a clone of the current url.
   * @returns Url
   */
  clone() {
    return Url(self.scheme, self.host, self.port, self.path, self.query, self.hash, self.username, self.password, self.has_slash, self.empty_path)
  }

  /**
   * Returns the url authority.
   * 
   * The authority component is preceded by a double slash ("//") and is
   * terminated by the next slash ("/"), question mark ("?"), or number
   * sign ("#") character, or by the end of the URI.
   *
   * @note mailto scheme does not have an authority. For this reason, mailto schemes return an empty string as authority.
   * @returns string
   */
  authority() {
    if !_SIMPLE_SCHEMES.contains(self.scheme) {
      var authority = ''

      # some schemes do not allow the userinfo and/or port subcomponents
      # RFC 3986 [Page 17]
      if self.username {
        authority += '${self.username}:'

        if self.password authority += self.password
        authority += '@'
      }
      authority += self.host

      # URI producers and normalizers should omit the ":" delimiter that
      # separates host from port if the port component is empty
      if self.port and self.port > 0 authority += ':${self.port}'

      return authority
    }
    return ''
  }

  /**
   * Returns true if the host of the url is a valid ipv4 address
   * and false otherwise.
   *
   * @returns bool
   */
  host_is_ipv4() {
    if self.host {
      return self.host.match('/^((25[0-5]|(2[0-4]|1[0-9]|[1-9]|)[0-9])(\\.(?!$)|$)){4}$/')
    }
    return false
  }

  /**
   * Returns true if the host of the url is a valid ipv6 address
   * and false otherwise.
   *
   * @returns bool
   */
  host_is_ipv6() {
    if self.host {
      var matched = self.host.match(_ipv6_regex)
      return matched and matched.length() > 0
    }
    return false
  }

  /**
   * Returns absolute url string of the url object.
   *
   * @returns string
   */
  absolute_url() {
    var url = '${self.scheme}:'

    if !_SIMPLE_SCHEMES.contains(self.scheme) or self.has_slash {
      url += '//'
    }

    # build the username:password symbol
    if self.username {
      url += self.username
      if self.password {
        url += ':${self.password}'
      }
      url += '@'
    }

    url += self.host

    if self.port {
      url += ':${self.port}'
    }
    if self.path {
      if self.path == '/' and _SIMPLE_SCHEMES.contains(self.scheme) {
        # do nothing...
      } else {
        url += self.path
      }
    }
    if self.hash {
      url += '#${self.hash}'
    }
    if self.query {
      url += '?${self.query}'
    }

    return url
  }

  /**
   * Returns a string representation of the url object. This will 
   * only be the same as the absolute url if the original string is 
   * an absolute url.
   * 
   * @returns string
   */
  to_string() {
    var result = ''
    var has_colon = _SIMPLE_SCHEMES.contains(self.scheme.lower())

    result += self.scheme or ''
    result += self.has_slash ? '://' : (has_colon ? ':' : '')
    
    if self.username {
      result += self.username
      result += self.password ? ':' + self.password : ''
      if self.host {
        result += '@'
      }
    }

    if self.host and self.host.index_of(':') != -1 {
      # ipv6 address
      result += '[' + self.host + ']'
    } else {
      result += self.host or ''
    }

    result += self.port ? ':' + self.port : ''
    result += self.path and !self.empty_path ? self.path : ''

    if self.query {
      result += self.query ? '?' + self.query : ''
    }

    result += self.hash ? '#' + self.hash : ''

    return result
  }

  @to_string() {
    return '<Url href=${self.to_string()}>'
  }

  @to_json() {
    return {
      scheme: self.scheme,
      host: self.host,
      port: self.port,
      path: self.path,
      query: self.query,
      hash: self.hash,
      username: self.username,
      password: self.password,
      has_slash: self.has_slash,
      empty_path: self.empty_path,
    }
  }
}



/**
 * URL-encodes a string
 * 
 * This function is convenient when encoding a string to be used in 
 * a query part of a URL, as a convenient way to pass variables to 
 * the next page.
 *
 * If strict mode is enabled, space character is encoded with the 
 * percent (%) sign in order to conform with RFC 3986. Otherwise,
 * is is encoded with the plus (+) sign in order to align with
 * the default encoding used by modern browsers.
 *
 * @param string url
 * @param bool? strict: Default value is `false`
 * @returns string
 */
def encode(url, strict) {
  if !is_string(url)
    raise TypeError('string expected at parameter 1')

  if strict != nil and !is_bool(strict) 
    raise TypeError('boolean expected at parameter 2')

  var result = ''
  url.ascii(true)

  for c in url {
    # keep alphanumeric and other accepted characters intact
    if ';/:@&$,#ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789*-_.~()%'.index_of(c.upper()) != -1
      result += c
    # when in strict mode
    else if strict and '=?'.index_of(c) != -1
      result += c
    # when not in strict mode
    else if !strict and c == ' ' result += '+'
    # encode all other characters
    else {
      result += '%${hex(ord(c))}'.upper()
    }
  }

  return result
}

/**
 * Decodes URL-encoded string. This function decodes any %## encoding in the given
 * string and plus symbols ('+') to a space character.
 * 
 * @param string url
 * @returns string
 */
def decode(url) {
  if !is_string(url)
    raise TypeError('string expected')

  # quick exit strategy
  if url.index_of('%') == -1 return url

  var lookup_table = '0123456789abcdef'

  var result = ''

  iter var i = 0; i < url.length(); i++ {
    if url[i] == '%' {
      # decode percent-encoded data here
      var hexdata = url[i+1, i+3].lower()

      if hexdata.length() != 2 raise UrlMalformedException('bad encoding')

      result += chr((lookup_table.index_of(hexdata[0]) * 16) + lookup_table.index_of(hexdata[1]))
      i += 2
    } 
    # + should be converted to space as most browsers
    # will encode space to + (non-strict Url.decode mode)
    # else if url[i] == '+' result += ' '
    else result += url[i]
  }

  return result
}

/**
 * Parses given url string into a Url object. If the strict argument is 
 * set to `true`, the parser will raise an Exception when it encounters 
 * a malformed url.
 * 
 * @param string url
 * @param bool? strict: Default value is `false`
 * @returns Url
 */
def parse(url, strict) {
  if !is_string(url) 
    raise TypeError('string expected in argument 1 (url)')
  if strict != nil and !is_bool(strict)
    raise TypeError('boolean expected in argument 2 (strict)')
    
  if strict == nil strict = true
  url = url.trim() # support urls surrounded by whitespaces

  var scheme = '', host = '', port, path = '', query, hash, username, password
  var skip_scheme = false, has_slash = false, empty_path = false

  # following that most urls written without indicating the scheme
  # are usually http urls, default url scheme to http if none was given
  # do not do this only when the scheme is mailto:
  if url.index_of('://') < 0 {
    var match_found = false
    for sc in _SIMPLE_SCHEMES {
      if url.starts_with(sc) {
        match_found = true
        break
      }
    }

    if !match_found {
      # set temporary host value so that we don't have to scan for the 
      # host anymore.
      if url.trim().starts_with('/') {
        host = ' '
      }

      url = url.ltrim('/')
      scheme = 'http'
      skip_scheme = true
    }
  }

  # query/search and hash entries tracker
  var query_starts = false
  var hash_starts = false

  iter var i = 0; i < url.length(); i++ {

    # simple anonymous function to scan port
    var _scan_port = @{
        var _port = ''
        i++
        while i < url.length() and types.digit(url[i]) { # id_digit
          _port += url[i]
          i++
        }
        port = to_number(_port)
    }

    if url[i] == ':' {
      if !scheme {
        # scan the scheme
        scheme = url[0, i]

        # skipping // if scheme is nota simple scheme that does not use the //
        if !_SIMPLE_SCHEMES.contains(scheme) {
          # if the // is missing, it's a malformed url
          if url[i,i+3] != '://' {
            if strict raise UrlMalformedException('expected // at index ${i}')
            else break
          }
          i += 2
          if !skip_scheme has_slash = true
        }

        # Scheme names consist of a sequence of characters beginning with a
        # letter and followed by any combination of letters, digits, plus
        # ("+"), period ("."), or hyphen ("-").  Although schemes are case-
        # insensitive, the canonical form is lowercase and documents that
        # specify schemes must do so with lowercase letters.  An implementation
        # should accept uppercase letters as equivalent to lowercase in scheme
        # names (e.g., allow "HTTP" as well as "http") for the sake of robustness.
        # https://tools.ietf.org/html/rfc3986#section-3.1
        if !scheme.match('/(?:^|[^a-z0-9.+-])([a-z][a-z0-9.+-]*)$/i') {
          if strict raise UrlMalformedException('invalid scheme')
          else break
        }
      } else if !port {
        # scan the port number
        _scan_port()
      }
    } else if !host and scheme {
      if i < url.length() and url[i] == '/' {
        path = '/'
        continue
      }

      # scan the host
      while i < url.length() and (types.digit(url[i]) or types.alpha(url[i]) or 
          url[i] == '.' or url[i] == '@' or url[i] == '-' or url[i] == '_' or url[i] == '+') {
        host += url[i]
        i++
      }

      if i < url.length() - 1 and url[i] == ':' {
        if url.length() - 1 > i and (types.alpha(url[i + 1]) or url[i + 1] == '@') {
          # username password combo encountered...
          username = host
          host = '' # we'll need to rescan for the host later...

          password = ''

          if url[i + 1] != '@' {
            password = ''
            i++

            while i < url.length() and url[i] != '@' {
              password += url[i]
              i++
            }

            if url.length() == i {
              # we read the entire url without a terminating @ sign...
              # something is wrong with this url and the url is definitely
              # malformed...
              if strict raise UrlMalformedException('url not complete')
              else break
            }

            # we need to go back to @ to let host be scanned completely
            # at the next iteration because we are now on the first
            # character of the host segment
            i--
          }
        }
        _scan_port()
      }

      # check if the host contains username data
      # e.g. mailto:username@example.com
      if host and host.index_of('@') > -1 {
        var _ = host.split('@')
        username = _[0]
        host = _[1]
      }

      if i == url.length() - 1 and url[i] != '?' and url[i] != '#' {
        host += url[i]
      }

      # we'll need to backtrack a a step to actually detect the next query and hash correctly.
      if i > 0 and i < url.length() and url[i].match(_url_punctuations_re) {
        i--
      }

      if host.ends_with('/') {
        host = host[,-1]
      }
    } else if path == '' and host {
      # scan the address

      # the path is terminated by the first question mark ("?") or 
      # number sign ("#") character, or by the end of the URI
      while i < url.length() and url[i] != '?' and url[i] != '#' {
        path += url[i]
        i++
      }

      if path == '' {
        path = '/'
        empty_path = true
      }

      # the path cannot begin with two slash characters
      # https://tools.ietf.org/html/rfc3986#section-3.3
      if path.starts_with('//') {
        if strict raise UrlMalformedException('invalid path')
        else break
      }

      # what should we parse next
      query_starts = i < url.length() and url[i] == '?'
      hash_starts = i < url.length() and url[i] == '#'

      if !path.starts_with('/') and !path.starts_with('.') {
        # if we haven't scanned everything in which case we'll take our path as is.
        # path = host.trim() + path
        # host = ''
        if !path.matches(_url_punctuations_re) {
          path = '/' + path
        }
      }
    } else if query_starts {

      if hash_starts {
        # this is an edge-case where we have the url hash coming
        # before the url query segment.
        # while this may be allowed by specific implementations,
        # but RFC 3986 doesn't allow this.
        # for this library, we are going strictly RFC 3986
        # https://tools.ietf.org/html/rfc3986#section-3.4
        if strict raise UrlMalformedException('query not allowed at index ${i}')
        else break
      }

      # scan the query
      # the query is always the last part of a url
      query = ''
      while i < url.length() and url[i] != '#' {
        query += url[i]
        i++
      }

      # reset for some abnormal urls that may be correct in users implementation
      query_starts = false

      hash_starts = i < url.length() and url[i] == '#'
    } else if hash_starts {
      # scan the hash
      hash = ''

      # we are still checking for the ? character
      # this ensures we don't erroneously
      while i < url.length() {
        hash += url[i]
        i++
      }
    }
  }

  # reset scheme if it never existed in the original link
  if skip_scheme scheme = ''

  # build a new Url instance and return
  return Url(scheme, host.trim(), port, path, query, hash, username, password, has_slash, empty_path)
}
