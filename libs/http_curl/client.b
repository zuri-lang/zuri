#!-- part of the http module

import ._process

import .request { HttpRequest }

import url
import socket
import .response { HttpResponse }

/**
 * Handles http requests.
 * 
 * @note This client do not currently support the compress, deflate and gzip transfer encoding.
 */
class HttpClient {
  
  /**
   * The user agent of the client used to make the request. 
   * Default value &mdash; `Zuri HTTP Client/1.0`.
   * @type string
   */
  var user_agent = 'Zuri HTTP Client/1.0'

  /**
   * Indicates if we receive a redirect from a server, this flag tells us whether 
   * we should follow it or not. Default value is `true`.
   * @type bool
   */
  var follow_redirect = true

  /**
   * Indicates if the site you're connecting to uses a different host name that what
   * they have mentioned in their server certificate's commonName (or subjectAltName) 
   * fields, connection will fail. You can skip this check by setting to true, but this 
   * will make the connection less secure.
   * @type bool
   */
  var skip_hostname_verification = false

  /**
   * Indicates if you want to connect to a site who isn't using a certificate that is
   * signed by one of the certs in the CA bundle you have, you can skip the verification 
   * of the server's certificate. This makes the connection A LOT LESS SECURE.
   * @type bool
   */
  var skip_peer_verification = false

  # ...
  var cookie_file

  /**
   * The site that refers us to the current site
   * @type string
   */
  var referer

  /**
   * If you have a CA cert for the server stored someplace else than in the default bundle.
   * @type string
   */
  var ca_cert

  /**
   * The connect timeout duration in milliseconds. Default value is 60,000 (1 minute).
   * @type number
   */
  var connect_timeout = 60000

  /**
   * The receive timeout duration in milliseconds. Default value is 300,000 (5 minutes).
   * @type number
   */
  var receive_timeout = 300000

  /**
   * A dictionary of headers sent along with the request.
   * @type dict
   */
  var headers = {}

  /**
   * Indicates whether to remove the expect header or not only applies to requests with 
   * files in the body
   * @type bool
   */
  var no_expect = false

  /**
   * Sends an Http request and returns a HttpResponse.
   * 
   * @param string uri
   * @param string? method: Default value is `GET`.
   * @param string|dict|nil data
   * @returns HttpResponse
   * @raises  SocketException
   * @raises  Exception
   */
  send_request(uri, method, data) {

    if !uri or !is_string(uri) 
      raise Exception('invalid url')

    if !method method = 'GET'

    if data != nil and !is_string(data) and !is_dict(data)
      raise Exception('string expected, ${typeof(data)} given')

    var request = HttpRequest()
    request.headers = self.headers

    if self.referer and !self.headers.contains('Referer') {
      request.headers.extend({
        'Referer': self.referer,
      })
    }

    var hst = request.headers.get('Host', nil)
    if hst and !uri.starts_with(hst) and !uri.starts_with('http://${hst}') and !uri.starts_with('https://${hst}') {
      uri = hst + uri
    }

    # parse the url into component parts
    uri = url.parse(uri)

    return request.send(uri, method.upper(), data, {
      follow_redirect: self.follow_redirect,
      connect_timeout: self.connect_timeout,
      receive_timeout: self.receive_timeout,
      skip_hostname_verification: self.skip_hostname_verification,
      skip_peer_verification: self.skip_peer_verification,
      user_agent: self.user_agent,
      cookie_file: self.cookie_file,
    })
  }

  /**
   * Sends an Http GET request and returns an HttpResponse.
   * 
   * @param string url
   * @returns HttpResponse
   * @raises  Exception
   * @raises  SocketException
   * @raises  HttpException
   */
  get(url) {
    return self.send_request(url, 'GET')
  }

  /**
   * Sends an Http POST request and returns an HttpResponse.
   * 
   * @param string url
   * @param string|bytes|nil data
   * @returns HttpResponse
   * @raises  Exception
   * @raises  SocketException
   * @raises  HttpException
   */
  post(url, data) {
    return self.send_request(url, 'POST', data)
  }

  /**
   * Sends an Http PUT request and returns an HttpResponse.
   * 
   * @param string url
   * @param string|bytes|nil data
   * @returns HttpResponse
   * @raises  Exception
   * @raises  SocketException
   * @raises  HttpException
   */
  put(url, data) {
    return self.send_request(url, 'PUT', data)
  }

  /**
   * Sends an Http DELETE request and returns an HttpResponse.
   * 
   * @param string url
   * @returns HttpResponse
   * @raises  Exception
   * @raises  SocketException
   * @raises  HttpException
   */
  delete(url) {
    return self.send_request(url, 'DELETE', nil)
  }
}
