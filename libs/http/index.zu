/**
 * @module http
 *
 * The `http` module provides a rich library to help in building HTTP 
 * clients and servers. The module also provides a few generic abstractions 
 * for simple HTTP operations such as a GET request and supports basic
 * routing.
 * 
 * ### Examples
 * 
 * The example below shows making a GET request to fetch a webpage.
 * 
 * ```zuri
 * import http
 * 
 * echo http.get('http://example.com')
 * # <class HttpResponse instance at 0x600002adacd0>
 * ```
 * 
 * There is a `post()` and `put()` alternative to the `get()` method called 
 * above and they are documented below.
 * 
 * For a more controlled HTTP request, you should use the HttpClient class. 
 * Below is an example of such implementation that sets the timeout for 
 * receiving response back from the server to 30 seconds.
 * 
 * ```zuri
 * import http
 * 
 * var client = http.HttpClient()
 * client.receive_timeout = 30000 # Optional
 * var res = client.send_request('http://example.com/endpoint?query=1', 'GET')
 * echo res.body.to_string()
 * ```
 * 
 * Creating a server with the `http` module is also a breeze. 
 * The example below shows an implementation of an HTTP API server listening on port 
 * 3000 and simple returns the JSON of the request object itself.
 * 
 * ```zuri
 * import http
 * import json
 * 
 * var server = http.server(3000)
 * server.handle('GET', '/', @(request, response) {
 *   response.json(request)
 * })
 * server.listen()
 * ```
 * 
 * Not only is it super simple to create an HTTP server, it is also very easy to create 
 * a TLS/HTTPS server with few modifications.
 * 
 * The following code creates a TLS version of the same server we created above.
 * 
 * ```zuri
 * import http
 * import json
 * 
 * var server = http.tls_server(3000)
 * if server.load_certs('/path/to/tlscert.crt', '/path/to/tlskey.key') {
 *   server.handle('GET', '/', @(request, response) {
 *     response.json(request)
 *   })
 *   server.listen()
 * }
 * ```
 * 
 * To create a TLS server, we use the `tls_server()` alternative to the `server()` function 
 * and load our certificates before we start to listen for incoming connections. It's that 
 * simple.
 * 
 * ---
 * 
 * The `http` module client does make some basic assumption as to the type of data to be 
 * sent in request bodies and for this reason, it will (unless asked not to) automatically 
 * convert dictionaries into JSON objects and create multipart/form-data request for you.
 * 
 * Natively, the `http` module will automatically encode and decode requests with the 
 * following content types:
 * 
 * - multipart/form-data
 * - application/x-www-form-urlencoded
 * - application/json
 * 
 * In the absence of any content-type in the request header or response header from a
 * server as the case may be, the module defaults to the `application/x-www-form-urlencoded` 
 * content type.
 * 
 * That been said, it gives the tools required to craft any request body of your choice.
 * 
 * @copyright 2021, Richard Ore and Zuri contributors
 */

import .response { HttpResponse }
import .status { * }
import .client { HttpClient }
import .server { HttpServer }
import .tls_server { TLSServer }

# single HttpClient for all requests lifetime
var _client = HttpClient()
_client.follow_redirect = true

/**
 * Sets the default request headers for the current module instance.
 *  
 * This function returns HttpClient in order to allow for idiomatic 
 * chaining such as:
 * 
 * ```zuri
 * import http
 * echo http.set_headers({
 *   'Authorization': 'Bearer SomeAPIBearerToken',
 *   'Host': 'example.com',
 * }).get('http://example.com/current-user').body.to_string()
 * ```
 * 
 * @param dict headers
 * @returns HttpClient
 * @raises  Exception
 */
def set_headers(headers) {
  if !is_dict(headers)
    raise TypeError('headers must be a dictionary')
  _client.headers = headers
  return _client
}
 
/**
 * Sends an Http GET request and returns an HttpResponse
 * or throws one of SocketException or Exception if it fails.
 * 
 * @param string url
 * @param dict? headers
 * @returns HttpResponse
 * @raises  Exception
 * @raises  SocketException
 * @raises  HttpException
 */
def get(url, headers) {
  return _client.get(url, headers)
}
 
/**
 * Sends an Http POST request and returns an HttpResponse.
 * 
 * @param string url
 * @param string|bytes|nil data
 * @param dict? headers
 * @returns HttpResponse
 * @raises  Exception
 * @raises  SocketException
 * @raises  HttpException
 */
def post(url, data, headers) {
  return _client.post(url, data, headers)
}
 
/**
 * Sends an Http PUT request and returns an HttpResponse.
 * 
 * @param string url
 * @param string|bytes|nil data
 * @param dict? headers
 * @returns HttpResponse
 * @raises  Exception
 * @raises  SocketException
 * @raises  HttpException
 */
def put(url, data, headers) {
  return _client.put(url, data, headers)
}

/**
 * Sends an Http PATCH request and returns an HttpResponse.
 * 
 * @param string url
 * @param string|bytes|nil data
 * @param dict? headers
 * @returns HttpResponse
 * @raises  Exception
 * @raises  SocketException
 * @raises  HttpException
 */
def patch(url, data, headers) {
  return _client.patch(url, data, headers)
}

/**
 * Sends an Http DELETE request and returns an HttpResponse.
 * 
 * @param string url
 * @param dict? headers
 * @returns HttpResponse
 * @raises  Exception
 * @raises  SocketException
 * @raises  HttpException
 */
def delete(url, headers) {
  return _client.delete(url, headers)
}

/**
 * Sends an Http OPTIONS request and returns an HttpResponse.
 * 
 * @param string url
 * @param dict? headers
 * @returns HttpResponse
 * @raises  Exception
 * @raises  SocketException
 * @raises  HttpException
 */
def options(url, headers) {
  return _client.options(url, headers)
}

/**
 * Sends an Http TRACE request and returns an HttpResponse.
 * 
 * @param string url
 * @param dict? headers
 * @returns HttpResponse
 * @raises  Exception
 * @raises  SocketException
 * @raises  HttpException
 */
def trace(url, headers) {
  return _client.trace(url, headers)
}

/**
 * Sends an Http HEAD request and returns an HttpResponse.
 * 
 * @param string url
 * @param dict? headers
 * @returns HttpResponse
 * @raises  Exception
 * @raises  SocketException
 * @raises  HttpException
 */
def head(url, headers) {
  return _client.head(url, headers)
}

/**
 * Returns the default shared client.
 *
 * @returns HttpClient
 */
def client() {
  return _client
}

/**
 * Creates an new HttpServer instance.
 * 
 * @param int port
 * @param string address
 * @returns HttpServer
 * @raises  Exception
 * @raises  SocketException
 * @raises  HttpException
 */
def server(port, address) {
  return HttpServer(port, address)
}


/**
 * Creates an new TLSServer instance.
 *
 * @param int port
 * @param string? host
 * @returns TLSServer
 * @throws Exception, SocketException, HttpException
 */
def tls_server(port, host) {
  return TLSServer(port, host)
}
 