#!-- part of the http module

import .exception { HttpException }
import .status { * }
import .response { HttpResponse }
import .defaults

import url
import socket
import url
import ssl
import hash
import mime
import zlib
import json
import convert

var _host_name_prefix = '/^[a-z0-9\-]+[.]/'
var _host_name_postfix = '/[.][a-z0-9\-]+$/'

def _process_header(header, meta_callback) {
  var result = {
    headers: {},
    cookies: []
  }

  if header {
    # Follow redirect headers...
    var data = header.trim().split('\r\n')

    iter var i = 0; i < data.length(); i++ {
      var d = data[i].index_of(':')
      if d > -1 {
        var key = data[i][0,d]
        var value = data[i][d + 1,].trim()

        # According to: https://datatracker.ietf.org/doc/html/rfc7230#section-3.2.6
        # A string of text is parsed as a single value if it is quoted using
        # double-quote marks
        # if value.starts_with('"') and value.ends_with('"')
        #   value = value[1,-1]

        # handle cookies in header
        if key.lower() == 'set-cookie' {
          if result.cookies.contains(key) {
            result.cookies.append(value)
          } else {
            result.cookies = [value]
          }
        } else {
          result.headers.set(key, value)
        }
      } else if(data[i].lower().starts_with('http/')){
        var split = data[i].split(' ')
        var http_version = split[0].replace('~http/~', '')

        # call back with (version, status code)
        if meta_callback meta_callback(http_version, to_number(split[1]))
      }
    }
  }

  return result
}

def _get_url_encoded_parts(data) {
  var result = {}

  var parts = data.split('&')
  for p in parts {
    p = p.split('=')
    if p {
      var name = url.decode(p[0].replace('+', ' '))
      if p.length() == 2 {
        result.set(name, url.decode(p[1].replace('+', ' ')))
      } else {
        result.set(name, nil)
      }
    }
  }

  return result
}

def _create_send_request_body(data, content_type, self_files) {
  if data {

    if !is_dict(data) and self_files {
      raise HttpException('data must be a dictionary when files are not empty')
    } else if !is_dict(data) and !self_files {
      return [to_string(data), content_type]
    }

    var data_objects = data.clone()
    var file_objects = {}

    for k, v in data_objects {
      if is_file(v) {
        file_objects.add(k, v)
        data_objects.remove(k)
      }
    }

    # enforce correct content type if request contains files.
    if file_objects.length() > 0 {
      content_type = 'multipart/form-data'
    }

    # We need to capture multipart/form-data here since it needs 
    # to specify boundaries right in the content-type header.
    using content_type {
      when 'application/x-www-form-urlencoded' {

        var body = ''
        for k, v in data_objects {
          if v {
            body += '&${url.encode(k, true)}=${url.encode(to_string(v), true)}'
          } else {
            body += '&${url.encode(k, true)}='
          }
        }
        
        return [body.ltrim('&').trim(), content_type]
      }
      when 'application/json' {
        return [json.encode(data_objects), content_type]
      }
      when 'multipart/form-data' {
        var boundary = ('-' * 10) + hash.md5(to_string(microtime()))
        content_type += '; boundary=${boundary}'

        var body = bytes(0)
        for k, v in data_objects {
          body += '--${boundary}\r\n'.to_bytes()
          body += 'Content-Disposition: form-data; name="${k}"\r\n'.to_bytes()
          body += '\r\n'.to_bytes()
          body += to_string(v).to_bytes()
          body += '\r\n'.to_bytes()
        }

        for k, f in file_objects {
          body += '--${boundary}\r\n'.to_bytes()
          body += 'Content-Disposition: form-data; name="${k}"; filename="${f.name()}"\r\n'.to_bytes()
          body += 'Content-Type: ${mime.detect(f)}\r\n'.to_bytes()
          body += '\r\n'.to_bytes()

          var content = f.read()
          body += is_string(content) ? content.to_bytes() : content
          body += '\r\n'.to_bytes()
        }

        # closing boundary marker
        body += '--${boundary}--'.to_bytes()

        return [body, content_type]
      }
      default {
        return [data, content_type]
      }
    }
  }

  return [data, content_type]
}

def _handle_content_encoding(data, encodings) {
    
  for encoding in encodings.split(',', false) {
    encoding = encoding.trim().lower()

    using encoding {
      when 'compress', 'x-compress' {
        data = zlib.uncompress(data)
      }
      when 'gzip', 'x-gzip' {
        data = zlib.ungzip(data)
      }
      when 'deflate' {
        data = zlib.undeflate(data)
      }
      default {
        raise HttpException('unsupported encoding ${encoding}')
      }
    }
  }

  return is_string(data) ? data : data.to_string()
}

def _get_cookie_line(cookie, host) {
  if is_string(cookie) {
    return 'Set-Cookie: ${cookie}'
  } else if is_dict(cookie) {
    var name = cookie.get('name', nil),
        value = cookie.get('value', nil),
        domain = cookie.get('domain', host),
        path = cookie.get('path', '/'),
        expires = cookie.get('expires', nil),
        max_age = cookie.get('max_age', '/'),
        secure = cookie.get('secure', false),
        same_site = cookie.get('same_site', nil),
        http_only = cookie.get('http_only', false)

    if name and value {
      var line = 'Set-Cookie: ${name}=${value}'
      if domain line += '; Domain=${domain}'
      if path line += '; Path=${path}'
      if expires line += '; Expires=${expires}'
      if max_age line += '; Max-Age=${max_age}'
      if secure line += '; Secure'
      if same_site line += '; SameSite=${same_site}'
      if http_only line += '; HttpOnly'

      return line
    }
  }
  
  return ''
}

def _get_content_length(headers) {
  for key, value in headers {
    if key.lower() == 'content-length' {
      return to_number(value)
    }
  }

  return -1
}

def _is_chuncked_encoding(headers) {
  for key, value in headers {
    if key.lower() == 'transfer-encoding' {
      return value.index_of('chunked') > -1
    }
  }

  return false
}

def _get_chunk_size(bytes) {
  return convert.hex_to_decimal(bytes.to_string())
}

def _parse_chunked_encoding(data, overflow) {
  if !overflow overflow = -1
  # echo '>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>> RAW DATA START <<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<'
  # echo data
  # echo overflow
  # echo '>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>> RAW DATA ENDS <<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<'


  # gracefully handle chunked data transfer
  # 
  # According to: https://datatracker.ietf.org/doc/html/rfc7230#section-4.1
  # 
  # chunked-body   = *chunk
  #           last-chunk
  #           trailer-part
  #           CRLF
  # 
  # chunk          = chunk-size [ chunk-ext ] CRLF
  #                   chunk-data CRLF
  # chunk-size     = 1*HEXDIG
  # last-chunk     = 1*("0") [ chunk-ext ] CRLF
  if !data return [data, 0, 0, false]

  var last_index = 0
  var total_size = 0
  var has_zero_size = false

  var chunks = bytes(0)

  if overflow > 0 {
    if data.length() > overflow {
      chunks += data[,overflow]
      last_index += overflow + 2 # skip the \r\n that should be inevitable.
    }
  }

  if overflow == 0 and data.length() > 2 {
    # if the data starts with \r\n and overflow is specifically 0,
    # we want to skip the \r\n part and continue directly into the data.
    if data[0] = 13 and data[1] = 10 {
      last_index += 2
    }
  }

  while true {
    var next_index = data.index_of('\r\n'.to_bytes(), last_index)

    if next_index == -1 {
      # If we don't find CRLF, it might be an incomplete chunk size or marker.
      # Save the remaining data as overflow for the next read.
      overflow = data.length() - last_index
      break
    }

    # chunk-size     = 1*HEXDIG
    # The chunk size is hex, but may have extensions.
    var size_line = data[last_index, next_index].to_string()
    var chunk_ext_index = size_line.index_of(';')
    if chunk_ext_index > -1 {
      size_line = size_line[0, chunk_ext_index]
    }
    
    var size = -1
    catch {
      size = convert.hex_to_decimal(size_line.trim())
    }

    if size == -1 {
      # This is NOT a valid hex. It might be that we found CRLF in the middle of 
      # some data that we thought was a chunk size marker but wasn't.
      # However, in chunked encoding, after each chunk-data there is a CRLF.
      # If we are here, we are EXPECTING a chunk size.
      # If it's not a valid hex, the stream is likely malformed.
      # To be safe, we'll treat it as the end of processing for this block.
      break
    }

    if size == 0 {
      has_zero_size = true
      last_index = next_index + 2
      
      # Handle Trailers (RFC 7230 Section 4.1.2)
      # A trailer is essentially a list of headers after the last chunk.
      # It ends with an empty line (CRLF CRLF).
      var trailer_end = data.index_of('\r\n\r\n'.to_bytes(), last_index)
      if trailer_end > -1 {
        last_index = trailer_end + 4
      } else {
        # If we don't find the end of the trailer, it might be incomplete.
        # But for now, we'll just skip what we have.
      }
      break
    }

    var start = next_index + 2
    var end = start + size

    if data.length() < end {
      # Not enough data for the full chunk.
      overflow = data.length() - last_index
      break
    }

    chunks += data[start, end]
    total_size += size

    # Skip the data and the trailing CRLF
    last_index = end + 2

    if data.length() < last_index {
        # We have the data but not the trailing CRLF yet.
        # This is common in slow networks.
        # However, since we now read the whole body before calling this, 
        # this shouldn't happen unless the stream is truly truncated.
        overflow = data.length() - end # Should be 0 or 1
        break
    }
  }

  return [chunks, total_size, overflow, has_zero_size]
}

def _verify_hostname(certificate, host) {
  host = host.lower()

  var current_time = time()
  if certificate.not_before.to_time() > current_time or
      certificate.not_after.to_time() < current_time {
    return false
  }

  if certificate.subject_name.lower() == '/cn=' + host {
    return true
  }

  if certificate.extensions and certificate.extensions.subjectAltName {
    # match against DNS subject alt names
    var alt_names = certificate.extensions.subjectAltName.
        split(',').map(@(x) {
          return x.trim()[4,]
        })

    for name in alt_names {
      name = name.lower()

      var pre_match = name.starts_with('*.')
      var post_match = name.ends_with('.*')

      if pre_match {
        name = name[2,]
      }
      if post_match {
        name = name[,-2]
      }

      if name == host {
        return true
      }

      if pre_match and post_match {
        if host.replace(_host_name_postfix, '').replace(_host_name_prefix, '') == name {
          return true
        } 
      } else if pre_match {
        if host.replace(_host_name_prefix, '') == name {
          return true
        }
      } else if post_match {
        if host.replace(_host_name_postfix, '') == name {
          return true
        }
      }
    }
  }

  return false
}

/**
 * Http request handler and object.
 * 
 * @serializable
 * @printable
 */
class HttpRequest {

  /**
   * The original request URL as sent in the raw request.
   * @type string
   */
  var request_uri

  /**
   * The requested path or file. E.g. if the Request URI is `/users?sort=desc`, 
   * then the path is `/users`.
   * @type string
   */
  var path

  /**
   * The HTTP method of the request: GET (the default), POST, PUT, etc.
   * @type string
   */
  var method

  /**
   * The hostname derived from the `Host` header or the first instance of 
   * `X-Forwarded-Host` if set.
   * @type string
   */
  var host

  /**
   * The IP address of the remote client that initiated the request.
   * @type string
   */
  var ip

  /**
   * The IPv6 address of the remote client that initiated the request.
   * @type string
   */
  var ipv6

  /**
   * A dictionary containing the headers sent with the request.
   * @type dictionary
   */
  var headers = {}

  /**
   * A dictionary containing the entries of the URI query string.
   * @type dictionary
   */
  var queries = {}

  /**
   * A list or dictionary containing the cookies sent with the request.
   * @type {list|dictionary}
   */
  var cookies = {}

  /**
   * A dictionary containing all data submitted in the request body.
   * @type dictionary
   */
  var body = {}

  /**
   * A dictionary containing the data of all files uploaded in the request.
   * @type dictionary
   */
  var files = {}

  /**
   * The HTTP version used for the request.
   * @type string
   */
  var http_version = '1.1'

  /**
   * The HTTP authentication method to use when the uri contains a credential. 
   * Default value is `Auth.ANY`.
   * @type Auth
   */
  # var auth_method = Auth.ANY

  # Private fields.
  var _body_type = 'application/x-www-form-urlencoded'
  var _client
  var _last_uri

  _read_cookies() {
    var cookies = self.headers.get('Cookie', nil)
    if cookies {
      cookies = cookies.split(';')
      for cookie in cookies {
        cookie = cookie.trim().split('=')
        if cookie {
          if cookie.length() == 2 {
            self.cookies.set(cookie[0], cookie[1])
          } else {
            self.cookies.set(cookie[0], nil)
          }
        }
      }

      # We want to contain all cookie information in the cookies property
      # and not have them sprinkled everywhere. To do this, we need to
      # consider cookies as not part of the headers.
      self.headers.remove('Cookie')
    }
  }

  _decode_line1(line1) {
    var parts = line1.split(' ')

    if parts.length() == 3 {
      self.method = parts[0]
      self.request_uri = parts[1]
      self.http_version = parts[2].lower().replace('~http\\/~', '')

      var uri_parts = parts[1].split('?')

      # The request path must exist before both a query and an hash.
      self.path = uri_parts[0].split('#')[0]

      if uri_parts.length() > 1 and uri_parts[1] {
        # A query exists and it must do so before any hash.
        var query = uri_parts[1].split('#')[0]
        self.queries = _get_url_encoded_parts(query)
      }

      return true
    }

    return false
  }

  _decode_multipart(body, boundary) {

    var boundaries = body.split('--${boundary}'.to_bytes())
    var contents = []

    for bound in boundaries {
      # bound = bound.ltrim('\r').ltrim('\n')

      # We don't want to treat empty bounds.
      var content_start = bound.to_string().index_of('\r\n\r\n')
      if content_start != -1 {
        var content_header = bound[,content_start].to_string(),
            content_body = bound[content_start + 4,]

        var content_headers = _process_header(content_header).headers
        var dispositions = content_headers.get('Content-Disposition', nil)

        if dispositions {

          # Expand the content-disposition header content to get the correct mapping.
          # disposition := "Content-Disposition" ":"
          #              disposition-type
          #              *(";" disposition-parm)

          # disposition-type := "inline"
          #                   / "attachment"
          #                   / extension-token
          #                   ; values are not case-sensitive

          # disposition-parm := filename-parm / parameter

          # filename-parm := "filename" "=" value;

          dispositions = dispositions.split(';')

          # The first directive is always form-data.
          if dispositions.length() < 2 or dispositions[0] != 'form-data'
            return false

          var disposition = {}

          iter var i = 1; i < dispositions.length(); i++ {

            # Directives are case-insensitive and have arguments that use
            # quoted-string syntax after the '=' sign
            var d = dispositions[i].split('=')
            var dname = d[0].trim().lower()

            if d.length() == 2 {

              var dvalue = d[1].trim()
              if dvalue.starts_with('"') and dvalue.ends_with('"')
                dvalue = dvalue[1,-1]

              disposition[dname] = dvalue
            } else {
              disposition.set(dname, nil)
            }
          }

          # and the header must also include a name parameter to identify the
          # relevant field.
          if !disposition.contains('name')
            return false

          var content_type = content_headers.get('Content-Type', nil),
              name = disposition.name
              # value = content_body.rtrim('\n').rtrim('\r')

          if content_type {

            # We are dealing with an uploaded file.
            self.files[name] = {
              mime: content_type.trim(),
              content: content_body,
              size: content_body.length(),
            }

            self.files[name].extend(disposition)
          } else {
            self.body[name] = content_body.to_string().trim()
          }
          contents.append(disposition)
        }
      }

      bound.dispose()   # free the binary data
    }

    return true
  }

  _decode_body(body) {

    # We need to capture multipart/form-data here since it allows
    # specifying boundaries right in the content-type header.
    var content_type = self.headers.get('content-type', ' ')
    if content_type {
      content_type = content_type.split(';')
    }

    var type = content_type[0].trim().lower()
    self._body_type = type

    # TODO: Add support for multipart/byteranges
    using type {
      when 'application/x-www-form-urlencoded' {
        self.body = _get_url_encoded_parts(body.to_string())
        body.dispose()  # free body binary data
      }
      when 'application/json' {
        self.body = json.decode(body.to_string())
      }
      when 'multipart/form-data' {
        # Content type should declare a boundary but nothing else.
        if content_type.length() != 2 return false

        var bound_spec = content_type[1].trim().split('=')

        # Make sure we have a valid boundary=xyz label.
        if bound_spec.length() != 2 or bound_spec[0].lower() != 'boundary' {
          body.dispose()   # free body binary data
          return false
        }

        if !self._decode_multipart(body, bound_spec[1]) {
          body.dispose()   # free body binary data
          return false
        }

        body.dispose()   # free body binary data
      }
      default {
        self.body = body
      }
    }

    return true
  }

  _receive_data(client, timeout, size, should_wait) {
    if size == nil size = -1
    if should_wait == nil should_wait = true

    var response = bytes(0)

    var receive_time_start = microtime()
    while true {
      var data

      catch {
        data = client.receive(size)
      } as error

      if error {
        if response.length() > 0 and error.message.contains('timed out') {
          break
        } else {
          raise HttpException(error.message)
        }
      }

      if !data {
        if !should_wait and response.length() > 0 {
          break
        }

        var time_taken = (microtime() - receive_time_start) / 1000
        if time_taken < timeout {
          continue
        } else {
          break
        }
      }

      var data_bytes = data.to_bytes()
      response += data_bytes
      data_bytes.dispose()
    }

    return response
  }

  /**
   * Parses a raw HTTP request string into a correct HttpRequest.
   *
   * @param string raw_data
   * @param Socket|TLSSocket|nil client
   * @returns boolean
   */
  parse(raw_data, client) {
    # reset files...
    self.body = {}
    self.files = {}
    self.headers = {}
    self.queries = {}
    self.cookies = {}

    if !is_string(raw_data)
      raise TypeError('raw_data must be string')
    # if !instance_of(client, socket.Socket)
    #   raise HttpException('invalid Socket')

    var socket_info = client.info()
    self.ip = socket_info.address
    self.ipv6 = socket_info.ipv6

    # separate the headers and the body
    var body_starts = raw_data.index_of('\r\n\r\n'), headers, body

    if body_starts {
      headers = raw_data[,body_starts].trim().split('\r\n')

      # Remember that body_start returns the position of the first \r in the '\r\n\r\n '
      # sequence.
      # +3 to remove the '\r\n\r\n' which is basically 4 characters.
      body = raw_data.ascii()[body_starts + 4,]
    }

    if headers {
      var line1 = headers[0]
      if !self._decode_line1(line1) return false

      _process_header('\r\n'.join(headers[1,])).
        headers.
        each(@(v, k) { self.headers[k.lower()] = v  })

      # To parse the host, first we try to retrieve the `X-Forwarded-Host` header
      # is it exists. If it does, we simply set our host to whatever value it has.
      # Otherwise, We check the `Host` header. If it was set, our host will be set to that.
      # Otherwise, our host will be an empty string.
      self.host = self.headers.get('x-forwarded-host', self.headers.get('host', '').split(':')[0])

      self._read_cookies()

      # Make sure we have all body contents.
      body = body.to_bytes()
      if self.headers.contains('content-length') {
        var content_length = to_number(self.headers['content-length'])
        var byte_length = body.length()
        if byte_length < content_length {
          var remaining_data = client.read(content_length - byte_length).to_bytes()
          body.extend(remaining_data)
          remaining_data.dispose()
        }
      }

      return self._decode_body(body)
    }

    return false
  }

  /**
   * Send HTTP requests to the given uri for the given method
   * and data (if given).
   *
   * @param url uri
   * @param string method
   * @param string|bytes|dict|nil data
   * @param dict? headers
   * @params dict? options
   */
  send(uri, method, data, headers, options) {
    # arguments validation.
    if !instance_of(uri, url.Url)
      raise TypeError('uri must be an instance of Url')
    if !is_string(method)
      raise TypeError('method must be string')
    if data != nil and !is_string(data) and !is_bytes(data) and !is_dict(data)
      raise TypeError('data must be string, bytes or dictionary')
    if headers != nil and !is_dict(headers)
      raise TypeError('headers must be a dictionary')
    if options != nil and !is_dict(options)
      raise TypeError('options must be a dictionary')

    if options == nil options = {}

    var client_headers = headers or self.headers

    # cache the request with for case-insensitive matching.
    var client_headers_cache = {}
    var has_useragent = false
    client_headers.each(@(v, k){
      var name = k.lower()

      # user is not allowed to override content-length to avoid
      # undefined behaviors.
      if name != 'content-length' {
        client_headers_cache[name] = [v, k]
      }

      if name == 'user-agent' {
        has_useragent = true
      }
    })

    # keep original values of important values
    # because some servers write a relative redirect header
    # we need to be able to resolve the correct redirect.
    var original_host = uri.host
    var original_scheme = uri.scheme

    var responder = uri.absolute_url(), error, referer
    var request_headers = {}, cookies = [], body
    var server_certificate

    var should_connect = true,
        time_taken = 0,
        redirect_count = 0,
        http_version = '1.0',
        status_code = 0

    var follow_redirect = options.get('follow_redirect', true),
        max_redirects = options.get('max_redirects', 5),
        connect_timeout = options.get('connect_timeout', 2000),
        send_timeout = options.get('send_timeout', 2000),
        receive_timeout = options.get('receive_timeout', 2000),
        user_agent = options.get('user_agent', nil),
        verify_peer = options.get('verify_peer', true),
        verify_hostname = options.get('verify_hostname', true)


    var start = microtime()

    while should_connect {
      if !uri.host uri.host = original_host
      if !uri.scheme uri.scheme = original_scheme

      var resolved_host = socket.get_address_info(uri.host)

      if resolved_host {
        var host = resolved_host.ip,
            port = uri.port,
            is_secure = uri.scheme == 'https',
            path = uri.path or '/'

        # construct message
        var message = '${method.upper()} ${path}'
        if uri.query message += '?${uri.query}'
        # if uri.hash message += '#${uri.hash}'
        message += ' HTTP/1.1\r\n'

        if !client_headers_cache.contains('host') {
          message += 'Host: ${uri.host}\r\n'
        }

        # add custom headers
        for key, value in client_headers_cache {
          if key != 'content-type' {
            message += '${value[1]}: ${value[0]}\r\n'
          }
        }

        if !has_useragent and user_agent {
          message += 'User-Agent: ${user_agent}\r\n'
        }

        if referer and !client_headers_cache.contains('referer') {
          message += 'Referer: ${referer}\r\n'
        }

        if self.cookies {
          if is_list(self.cookies) {
            for cookie in self.cookies {
              var cline = _get_cookie_line(cookie, uri.host)
              if cline {
                message += cline.to_bytes()
                message += '\r\n'.to_bytes()
              }
            }
          } else if is_dict(self.cookies) {
            self.cookies.each(@(v, k) {
              var cline = _get_cookie_line({name: k, value: v}, uri.host)
              if cline {
                message += cline.to_bytes()
                message += '\r\n'.to_bytes()
              }
            })
          }
        }


        # process the request body
        var resolved_data = _create_send_request_body(
          data,
          client_headers_cache.get(
            'content-type',
            [self._body_type]
          )[0].lower(),
          self.files
        )

        # set correct content type and body
        message += 'Content-Type: ${resolved_data[1]}\r\n'
        data = resolved_data[0]

        if data {
          # append the correct content length to the message
          message += 'Content-Length: ${data.length()}\r\n'
        } else {
          # message += 'Content-Length: 0\r\n'
        }

        # convert message to binary in order to play nice with files
        message = message.to_bytes()

        # append the body
        if data {
          message += '\r\n'.to_bytes()
          message += is_bytes(data) ? data : data.to_bytes()
        } else {
          message += '\r\n'.to_bytes()
        }


        # connect to the url host on the specified port and send the request message
        var client
        var client_reused = false
        if self._client {
          if self._client.is_connected and
             self._last_uri and
             self._last_uri.host == uri.host and
             self._last_uri.port == uri.port and
             self._last_uri.scheme == uri.scheme {
            client = self._client
            client_reused = true
          } else {
            self._client.close()
            self._client = nil
          }
        }

        if !client_reused {
          # do real request here...
          client = socket.Socket()

          client.set_option(socket.SO_SNDTIMEO, send_timeout)
          client.set_option(socket.SO_RCVTIMEO, receive_timeout)
          client.set_blocking(false)
          client.set_option(socket.SO_REUSEADDR, true)

          if is_secure {
            client = ssl.TLSSocket(client, ssl.TLSClientContext())
            client.get_context().set_ciphers(defaults.ciphers)
            client.get_ssl().set_tlsext_host_name(uri.host)

            if verify_peer == false {
              client.get_context().set_verify(ssl.SSL_VERIFY_PEER, false)
            } else {
              client.get_context().set_verify(ssl.SSL_VERIFY_PEER, true)
            }
          }

          var local_port = to_number(port or 0)
          if local_port == 0 {
            local_port = is_secure ? 443 : 80
          }

          if !client.connect(host, local_port, connect_timeout) {
            should_connect = false
            raise HttpException('could not connect')
          }

          if is_secure {
            server_certificate = client.get_ssl().get_peer_certificate()

            if verify_hostname {
              if !_verify_hostname(server_certificate, uri.host) {
                client.close()
                raise HttpException('bad certificate')
              }
            }
          }
        } else {
          # If reused, we might still want to update timeouts?
          client.set_option(socket.SO_SNDTIMEO, send_timeout)
          client.set_option(socket.SO_RCVTIMEO, receive_timeout)

          if is_secure {
            server_certificate = client.get_ssl().get_peer_certificate()
          }
        }

        # echo '>>>>>>>>>>>>>>>>>>>>>>>>>> MESSAGE START <<<<<<<<<<<<<<<<<<<<<<<<<<'
        # echo message.to_string()
        # echo '>>>>>>>>>>>>>>>>>>>>>>>>>> MESSAGE END <<<<<<<<<<<<<<<<<<<<<<<<<<<<'

        # send the request message
        catch {
          client.send(message)
        }

        if true {

          # receive the response...
          var response_data = self._receive_data(client, receive_timeout, -1, false)

          if response_data.length() == 0 {
            if !client_headers_cache.get('connection', nil) == 'keep-alive' {
              raise HttpException('failed to read response')
            }

            continue
          }

          # Handle 100-Continue
          var test_response_data = response_data[,12].to_string()
          while test_response_data.starts_with('HTTP/1.1 100') or
                test_response_data.starts_with('HTTP/1.0 100') {
            var next_response_start = response_data.index_of('\r\n\r\n'.to_bytes())
            if next_response_start > -1 {
              var old_data = response_data
              response_data = response_data[next_response_start + 4,]
              old_data.dispose()

              if response_data.length() == 0 {
                response_data = self._receive_data(client, receive_timeout, -1, false)
              }
            } else {
              break # Should not happen with valid 100 Continue
            }
          }

          if response_data.length() == 0 {
            raise HttpException('failed to read response after 100 Continue')
          }

          # separate the headers and the body
          var header_ends = response_data.index_of('\r\n\r\n'.to_bytes())

          if header_ends > -1 {
            request_headers = response_data[0,header_ends].to_string().trim()
            body = response_data[header_ends + 4, response_data.length()]
          } else {
            # Clear the headers here. It may currently be a dictionary.
            request_headers = ''
          }

          var parsed_headers = _process_header(request_headers, @(version, status) {
            http_version = version
            status_code  = status
          })

          request_headers = parsed_headers.headers
          cookies = parsed_headers.cookies

          # build local headers cache
          var headers_cache = {}
          for key, value in request_headers {
            headers_cache[key.lower()] = value
          }

          # According to https://datatracker.ietf.org/doc/html/rfc7230#section-3.3
          #
          # Responses to the HEAD request method (Section 4.3.2
          # of [RFC7231]) never include a message body because the associated
          # response header fields (e.g., Transfer-Encoding, Content-Length,
          # etc.), if present, indicate only what their values would have been if
          # the request method had been GET
          if method.upper() != 'HEAD' {

            # ensure to handle chunked transfers first because a message can contain
            # content-length header and still be chunked in which case the specification
            # says the content-length may be irrelevant (obviously, since by the time
            # you read all the chunks completely you'll still arrive at the same length).
            if headers_cache.contains('transfer-encoding') and headers_cache['transfer-encoding'].index_of('chunked') > -1 {
              var chunk_overflow = -1, has_zero_size = false

              # we already have some body from the initial read.
              # Check if it contains the end-of-chunked-body marker '0\r\n\r\n'
              if body.index_of('0\r\n\r\n'.to_bytes()) > -1 {
                has_zero_size = true
              }

              while !has_zero_size {
                var data = self._receive_data(client, receive_timeout, -1, false)
                if data.length() == 0 {
                  # If we get no data, it might be a timeout or connection closed.
                  # For chunked, we MUST see the 0 chunk.
                  # However, if the connection is closed, we must stop.
                  break
                }

                body += data

                if data.index_of('0\r\n\r\n'.to_bytes()) > -1 or body.index_of('0\r\n\r\n'.to_bytes()) > -1 {
                  has_zero_size = true
                }
                data.dispose()
              }

              var parsed_chunk = _parse_chunked_encoding(body, -1)
              body.dispose()
              body = parsed_chunk[0]
            }

            # gracefully handle responses being sent in multiple packets
            # if the request header contains the Content-Length,
            # get that length and keep reading until we have read the total
            # length of the response.
            else if headers_cache.contains('content-length') {
              var content_length = to_number(headers_cache['content-length']) - 2

              # According to: https://datatracker.ietf.org/doc/html/rfc7230#section-3.4
              # A client that receives an incomplete response message, which can
              # occur when a connection is closed prematurely or when decoding a
              # supposedly chunked transfer coding fails, MUST record the message as
              # incomplete.
              #
              # But it also says in https://datatracker.ietf.org/doc/html/rfc9112#section-8
              # that,
              # A message that uses a valid Content-Length is incomplete if the size of
              # the message body received (in octets) is less than the value given by
              # Content-Length.
              while body.length() < content_length {
                # append the new data in the stream
                var data = self._receive_data(client, receive_timeout, -1, false)
                # var data = self._receive_data(client, receive_timeout)

                if data.length() > 0 {
                  body += data
                  data.dispose()
                }
              }
            }
          }

          # handle content-encodings (compress, x-compress, deflate, gzip, and x-gzip)
          if headers_cache.contains('content-encoding') {
            body = _handle_content_encoding(body, headers_cache['content-encoding'])
          }

          # close client or keep it for next request
          if headers_cache.get('connection', '').lower() == 'keep-alive' {
            self._client = client
            self._last_uri = uri.clone()
          } else {
            client.close()
            self._client = nil
            self._last_uri = nil
          }

          if follow_redirect and headers_cache.contains('location') {
            if redirect_count >= max_redirects {
              should_connect = false
            } else {
              redirect_count++
              referer = uri.absolute_url()
              uri = url.parse(headers_cache['location'])
              responder = referer

              # Update Host header for the new URI
              if uri.host {
                client_headers_cache['host'] = [uri.host, 'Host']
              } else {
                uri.host = original_host
              }
            }
          } else {
            should_connect = false
          }

          # normalize body to bytes
          if is_string(body) {
            body = body.to_bytes()
          }
        } else {
          should_connect = false
          raise HttpException('could not connect')
        }
      } else {
        should_connect = false
        raise HttpException('could not resolve ip address')
      }
    }

    time_taken += (microtime() - start) / 1000

    # return a valid HttpResponse
    var http_response = HttpResponse(body, status_code, request_headers, cookies, http_version,
      time_taken, redirect_count, responder or uri.host)

    # set the certificate if it exists.
    http_response.certificate = server_certificate

    return http_response
  }

  /**
   * Closes any persistent connection maintained by this request.
   */
  close() {
    if self._client {
      self._client.close()
      self._client = nil
      self._last_uri = nil
    }
  }

  /**
   * Returns the request as a string.
   */
  to_string() {
    return '<HttpRequest method=${self.method}, path=${self.path}>'
  }

  /**
   * Returns the request as a JSON object.
   */
  to_json() {
    return {
      request_uri: self.request_uri,
      path: self.path,
      method: self.method,
      host: self.host,
      ip: self.ip,
      headers: self.headers,
      queries: self.queries,
      cookies: self.cookies,
      body: to_string(self.body),
    }
  }

  @to_string() {
    return self.to_string()
  }

  @to_json() {
    return self.to_json()
  }
}

