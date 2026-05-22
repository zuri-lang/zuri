#!-- part of the http module

import ._process

import .exception { 
  HttpException 
}
import .status { * }
import .response { 
  HttpResponse 
}

import url
import socket
import json
import curl { 
  Option, 
  Info, 
  Curl, 
  CurlList, 
  CurlMime, 
  Auth 
}
# import ssl

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
   * A dictionary containing the cookies sent with the request.
   * @type dictionary
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
  var auth_method = Auth.ANY

  # Private fields.
  var _body_type = 'application/x-www-form-urlencoded'

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

  _get_url_encoded_parts(data) {
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
        self.queries = self._get_url_encoded_parts(query)
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
            
        var content_headers = _process.process_header(content_header).headers,
            dispositions = content_headers.get('Content-Disposition', nil)

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
    var content_type = self.headers.get('Content-Type', ' ')
    if content_type {
      content_type = content_type.split(';')
    }

    var type = content_type[0].trim().lower()
    self._body_type = type

    using type {
      when 'application/x-www-form-urlencoded' {
        self.body = self._get_url_encoded_parts(body.to_string())  
        body.dispose()  # free body binary data
      }
      when 'application/json' {
        self.body = json.decode(body.to_string())
        body.dispose()  # free body binary data
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
      raise HttpException('raw_data must be string')
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

      self.headers = _process.process_header('\r\n'.join(headers[1,])).headers

      # To parse the host, first we try to retrieve the `X-Forwarded-Host` header 
      # is it exists. If it does, we simply set our host to whatever value it has. 
      # Otherwise, We check the `Host` header. If it was set, our host will be set to that. 
      # Otherwise, our host will be an empty string.
      self.host = self.headers.get('X-Forwarded-Host', self.headers.get('Host', '').split(':')[0])

      self._read_cookies()

      # Make sure we have all body contents.
      body = body.to_bytes()
      if self.headers.contains('Content-Length') {
        var content_length = to_number(self.headers['Content-Length'])
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

  _create_send_request_body(curl, data) {
    if data {
      if !is_dict(data) and self.files {
        raise HttpException('data must be a dictionary when files are not empty')
      } else if !is_dict(data) and !self.files {
        curl.set_option(Option.POSTFIELDS, data)
      } else {
        var mime = CurlMime(curl)

        for k, v in data {
          if !is_file(v) {
            mime.add(k, v)
          } else {
            mime.add_file(k, v.abs_path())
          }
        }
  
        if self.files {
          for k, v in files {
            mime.add_file(k, v)
          }
        }
  
        curl.set_option(Option.MIMEPOST, mime)
      }
    }
  }

  /**
   * Sends the given request to the given uri using the given method and 
   * optionally passing the data if given.
   * 
   * @param Url uri
   * @param string method
   * @param string|bytes|nil data
   * @param dict? options
   * @returns HttpResponse
   * @raises  HttpException
   */
  send(uri, method, data, options) {

    # arguments validation.
    if !instance_of(uri, url.Url)
      raise HttpException('uri must be an instance of Url')
    if !is_string(method)
      raise HttpException('method must be string')
    if data != nil and !is_string(data) and !is_bytes(data) and !is_dict(data)
      raise HttpException('data must be string, bytes or dictionary')
    if options != nil and !is_dict(options)
      raise Exception('options must be a dictionary')

    if options == nil options = {}

    var responder = uri.absolute_url(), error, referer

    var time_taken = 0, 
        redirect_count = 0, 
        http_version = '1.0', 
        status_code = 0

    var curl = Curl()
    curl.set_option(Option.URL, responder)
    if method.upper() != 'GET' {
      curl.set_option(Option.CUSTOMREQUEST, method.upper())
    }

    var user_agent = options.get('user_agent', nil)
    if user_agent curl.set_option(Option.USERAGENT, user_agent)

    curl.set_option(Option.FOLLOWLOCATION, options.get('follow_redirect', true))
    curl.set_option(Option.CONNECTTIMEOUT_MS, options.get('connect_timeout', 2000))
    curl.set_option(Option.TIMEOUT_MS, options.get('receive_timeout', 2000))
    curl.set_option(Option.SSL_VERIFYPEER, !options.get('skip_peer_verification', false))
    curl.set_option(Option.SSL_VERIFYHOST, !options.get('skip_hostname_verification', false))

    if uri.username {
      curl.set_option(Option.USERNAME, uri.username)
      curl.set_option(Option.PASSWORD, uri.password)

      # Set the authentication method
      curl.set_option(Option.HTTPAUTH, self.auth_method)
    }

    # Handle cookies.
    var cookie_file = options.get('cookie_file', nil)
    if cookie_file curl.set_option(Option.COOKIEFILE, cookie_file)
    if self.cookies {
      var cookie = ''
      for k, v in self.cookies {
        cookie += '${k}=${v};'
      }
      curl.set_option(Option.COOKIE, cookie)
    }

    # Just trying to get a little performance boost.
    if uri.scheme.lower() == 'https' {
      curl.set_option(Option.DEFAULT_PROTOCOL, 'https')
    }

    # Set request headers
    var headers = ['']
    for key, value in self.headers {

      # Make sure to always override user set Content-Length header 
      # to avoid unexpected behavior.
      if key.lower() != 'content-length' {
        headers.append('${key}: ${value}')
      }
    }
    curl.set_option(Option.HTTPHEADER, CurlList(headers))
    
    # handle files and data here...
    self._create_send_request_body(curl, data)

    var result = curl.send()

    # Convert raw headers into a dictionary.
    var response_headers = result.headers.trim().split('\r\n\r\n')
    if response_headers.length() > 1 {
      response_headers = response_headers[-1]
    } else {
      response_headers = response_headers[0]
    }
    response_headers = _process.process_header(response_headers)

    # return a valid HttpResponse
    var response =  HttpResponse(
      result.body, 
      curl.get_info(Info.RESPONSE_CODE), 
      response_headers.headers,
      response_headers.cookies,
      curl.get_info(Info.HTTP_VERSION), 
      curl.get_info(Info.TOTAL_TIME), 
      curl.get_info(Info.REDIRECT_COUNT), 
      curl.get_info(Info.EFFECTIVE_URL)
    )

    # Free curl.
    curl.close()

    return response
  }

  /**
   * Returns a dictionary representation of the HttpRequest instance.
   * 
   * @returns dict
   */
  to_dict() {
    return {
      request_uri: self.request_uri,
      path: self.path,
      method: self.method,
      host: self.host,
      ip: self.ip,
      ipv6: self.ipv6,
      headers: self.headers,
      queries: self.queries,
      cookies: self.cookies,
      body: self.body,
    }
  }

  /**
   * Returns a string representation of the HttpRequest instance.
   * 
   * @returns string
   */
  to_string() {
    return '<HttpRequest method=${self.method}, path=${self.path}>'
  }

  @to_string() {
    return self.to_string()
  }

  @to_json() {
    return self.to_dict()
  }
}

