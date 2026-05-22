#!-- part of the http module

import json
import mime
import date { Date }
import .exception { HttpException }

/**
 * Represents the response to an Http request.
 * 
 * @serializable
 * @printable
 */
class HttpResponse {

  /**
   * The HTTP version of the response
   * @type string
   */
  var version

  /**
   * The HTTP response status code
   * @type number
   */
  var status

  /**
   * The HTTP response headers
   * @type dictionary
   */
  var headers

  /**
   * Total time taken for the HTTP request that generated this HttpResponse to complete
   * @type number
   */
  var time_taken

  /**
   * The number of times the HTTP request that generated this HttpResponse was redirected.
   * @type number
   */
  var redirects

  /**
   * The final URL that provided the HttpResponse. This will sometimes differ from the 
   * original request URI.
   * @type string
   */
  var responder

  /**
   * The content of the HTTP response as bytes
   * @type bytes
   */
  var body

  /**
   * The cookies to be sent back to the client
   * @type list
   */
  var cookies

  /**
   * @param string body
   * @param int status
   * @param dict headers
   * @param list[string] cookies
   * @param string version
   * @param number time_taken
   * @param int redirects
   * @param string responder
   * @constructor 
   */
  HttpResponse(body, status, headers, cookies, version, time_taken, redirects, responder) {
    self.status = status ? status : 200
    self.body = body ? body : bytes(0)
    self.headers = headers ? headers : {
      'Content-Type': 'text/html; charset=utf-8',
      'X-Powered-By': 'Zuri',
      'Date': Date().format('r')
    }
    self.cookies = cookies ? cookies : []
    self.version = version ? version : '1.0'
    self.time_taken = time_taken ? time_taken : 0
    self.redirects = redirects
    self.responder = responder
  }

  /**
   * Writes data to the response stream. 
   * 
   * > This method should be prefered over writing directly to the body 
   * > property to prevent unexpected behaviors.
   * 
   * @param string|bytes data
   */
  write(data) {
    if !is_string(data) and !is_bytes(data)
      raise Exception('data must be bytes or string')
    if is_string(data) self.body += data.to_bytes()
    else self.body += data
  }

  /**
   * Writes a json encoded data to the response stream and sets the response 
   * `Content-Type` to `application/json`. If the status code is given, the
   * response will be sent with the given status code.
   * 
   * @param any data
   * @param number? status_code
   */
  json(data, status_code) {
    if status_code != nil {
      if !is_number(status_code)
        raise Exception('argument 2 (status_code) expects a number')
      self.status = status_code
    }
    self.content_type('application/json')
    self.write(json.encode(data))
  }

  /**
   * Writes a file into the response stream and sets the `Content-Type` to the 
   * correct mimetype for the file. If the status code is given, the
   * response will be sent with the given status code.
   * 
   * @param string path
   * @param number? status_code
   */
  file(path, status_code) {
    if status_code != nil {
      if !is_number(status_code)
        raise Exception('argument 2 (status_code) expects a number')
      self.status = status_code
    }

    self.content_type(mime.detect_from_name(path))
    var file_data = file(path, 'rb').read()
    self.write(file_data)
    file_data.dispose()
  }

  /**
   * Sets a cookie to be send back to a client with the given _key_ and _value_. 
   * When other parameters are given, they are used to construct a correct Set-Cookie 
   * header based on their named properties.
   * 
   * @param string key
   * @param string value
   * @param string? domain
   * @param string? path
   * @param string? expires
   * @param bool? secure
   * @param string? extras
   */
  set_cookie(key, value, domain, path, expires, secure, extras) {
    if !is_string(key) or !is_string(value)
      raise Exception('argument 1 (key) and argument 2 (value) must be string')
    if (domain != nil and !is_string(domain)) or
        (path != nil and !is_string(path)) or
        (expires != nil and !is_string(expires))
      raise Exception(
        'argument 3 (domain), argument 4 (path) and argument 5 (expires) must be string when given'
      )
    if secure != nil and !is_bool(secure)
      raise Exception('argument 6 (secure) must be a boolean')
    if extras != nil and !is_string(extras)
      raise Exception('argument 7 (extras) must be a string when given')

    # fix common prefix support for clients that implement them
    # NOTE: they have no effect when the client do not.
    if !path and !key.starts_with('__Host-') path = '/'
    if !secure and key.starts_with('__Secure-') secure = true
    
    var cookie = '${key}=${value}'
    if domain cookie += '; Domain=${domain}'
    if path cookie += '; Path=${path}'
    if expires cookie += '; Expires=${expires}'
    if secure cookie += '; Secure'
    if extras cookie += '; ${extra}'

    self.cookies.append(cookie)
  }

  /**
   * Redirects the client to a new location. This function simultaneously sets 
   * the `Location` header and returns a 30x status code. If the `status` 
   * parameter is not given, the function defaults to `302`.
   * 
   * @param string location
   * @param string? status
   * @note When supplying a status, it must be a 30x
   * @throw HttpException
   */
  redirect(location, status) {
    if !is_string(location)
      raise Exception('location must be a string')
    if status != nil and !is_number(status) and !is_int(status)
      raise Exception('status must be an integer if present')

    self.headers.set('Location', location)
    self.status = status ? status : 302

    if self.status < 300 or self.status > 399
      raise HttpException('redirect status code must be a 30x')
    self.body = bytes(0)
  }

  /**
   * Sets the content type of the HTTP response.
   * 
   * @param string mimetype
   */
  content_type(mimetype) {
    if !is_string(mimetype)
      raise Exception('argument 1 (mimetype) expects string')

    self.headers.set('Content-Type', mimetype)
  }

  @to_string() {
    return '<HttpResponse status: ${self.status}, version: ${self.version}, time_taken:' +
        ' ${self.time_taken}, redirects: ${self.redirects}, responder: ${self.responder}>'
  }

  @to_json() {
    return {
      status: self.status,
      version: self.version,
      time_taken: self.time_taken,
      redirects: self.redirects,
      responder: self.responder,
      headers: self.headers,
      cookies: self.cookies,
      body: self.body
    }
  }
}
