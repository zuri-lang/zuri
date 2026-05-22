#!-- part of the http module

import .request { HttpRequest }
import .response { HttpResponse }
import .exception { HttpException }
import .status

import os
import mime
import hash
import socket as so
import reflect

/**
 * HTTP server.
 * 
 * @printable
 */
class HttpServer {

  /**
   * The host address to which this server will be bound. Default value is 
   * socket.IP_LOCAL (127.0.0.1)
   * @type string
   */
  var host = so.IP_LOCAL

  /**
   * The port to which this server will be bound to on the host.
   * @type number
   */
  var port = 0

  /**
   * The working Socket instance for the HttpServer.
   * @type [[socket.Socket]]
   */
  var socket

  /**
   * A boolean value indicating whether to reuse socket addresses or not.
   * Default value is `true`.
   * @type bool
   */
  var reuse_address = true

  /**
   * The timeout in milliseconds after which an attempt to read clients 
   * request data will be terminated. Default value is 2,000 (2 seconds).
   * @type number
   */
  var read_timeout = 2000

  /**
   * The timeout in milliseconds after which an attempt to write response data to 
   * clients will be terminated. 
   * 
   * If we cannot send response to a client after the stipulated time, it will be 
   * assumed such clients have disconnected and existing connections for that 
   * client will be closed and their respective sockets will be discarded. Default 
   * value is 2,000 (2 seconds).
   * 
   * @type number
   */
  var write_timeout = 2000

  # status trackers.
  var _is_listening = false

  # event handler lists.
  var _connect_listeners = []
  var _disconnect_listeners = []
  var _received_listeners = []
  var _reply_listeners = []
  var _error_listeners = []

  # See https://www.rfc-editor.org/rfc/rfc9110.html#methods
  # for more information regards why these methods were 
  # implemented by default.
  var _routes = {
    # Transfer a current representation of the target resource.
    'GET': {},
    # Same as GET, but do not transfer the response content.
    'HEAD': {},
    # Perform resource-specific processing on the request content.
    'POST': {},
    # Replace all current representations of the target resource with the request content.
    'PUT': {},
    # Patches all current representations of the target resource with the request content.
    'PATCH': {},
    # Remove all current representations of the target resource.
    'DELETE': {},
    # Establish a tunnel to the server identified by the target resource.
    'CONNECT': {},
    # Describe the communication options for the target resource.
    'OPTIONS': {},
    # Perform a message loop-back test along the path to the target resource.
    'TRACE': {},
  }

  var _none_handler = @(req, res) {
    res.status = 404
    res.content_type('text/plain')
    res.write('404 - Not Found.')
  }

  /**
   * @param int port
   * @param string? host
   * @constructor
   */
  HttpServer(port, host) {

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

    self.socket = so.Socket()
  }

  /**
   * Stops the server.
   */
  close() {
    self._is_listening = false
    if !self.socket.is_closed
      self.socket.close()
  }

  /**
   * Adds a function to be called when a new client connects.
   * 
   * @note Function _function_ MUST accept at one parameter which will be passed the client Socket object.
   * @note Multiple `on_connect()` may be set on a single instance.
   * @param function(1) function
   */
  on_connect(function) {
    if !is_function(function)
      raise TypeError('argument 1 (function) must be a function')
      
    # var fn_arity = reflect.get_function_metadata(function).arity
    # if fn_arity != 1 
    #   raise ArgumentError('function must accept exactly one argument (client)')

    self._connect_listeners.append(function)
  }

  /**
   * Adds a function to be called when a new client disconnects.
   * 
   * @note Function _function_ MUST accept at one parameter which will be passed the client.
   * @note Multiple `on_disconnect()` may be set on a single instance.
   * @param function(1) function
   */
  on_disconnect(function) {
    if !is_function(function)
      raise TypeError('argument 1 (function) must be a function')
      
    # var fn_arity = reflect.get_function_metadata(function).arity
    # if fn_arity != 1 
    #   raise ArgumentError('function must accept exactly one argument (info)')

    self._disconnect_listeners.append(function)
  }

  /**
   * Adds a function to be called when the server receives a message from a client.
   * 
   * > Function _fn_ MUST accept TWO parameters. First parameter will accept the HttpRequest 
   * > object and the second will accept the HttpResponse object.
   * 
   * @note Multiple `on_receive()` may be set on a single instance.
   * @param function(2) handler
   */
  on_receive(handler) {
    if !is_function(handler)
      raise TypeError('argument 1 (handler) must be a function')

    # var fn_arity = reflect.get_function_metadata(handler).arity
    # if fn_arity != 2 
    #   raise ArgumentError('handler must accept two arguments (request, response)')

    self._received_listeners.append(handler)
  }

  /**
   * Adds a function to be called when the server sends a reply to a client.
   * 
   * > Function _function_ MUST accept one parameter which will be passed the HttpResponse object.
   * 
   * @note Multiple `on_sent()` may be set on a single instance.
   * @param function(1) function
   */
  on_reply(function) {
    if !is_function(function)
      raise TypeError('argument 1 (function) must be a function')
      
    # var fn_arity = reflect.get_function_metadata(function).arity
    # if fn_arity != 1 
    #   raise ArgumentError('function must accept exactly one argument (response)')

    self._reply_listeners.append(function)
  }

  /**
   * Adds a function to be called when the server encounters an error with a client.
   * 
   * > Function _function_ MUST accept two parameters. The first argument will be passed the 
   * > `Exception` object and the second will be passed the client `Socket` object.
   * 
   * @note Multiple `on_error()` may be set on a single instance.
   * @param function(2) function
   */
  on_error(function) {
    if !is_function(function)
      raise TypeError('argument 1 (function) must be a function')
      
    # var fn_arity = reflect.get_function_metadata(function).arity
    # if fn_arity != 2 
    #   raise ArgumentError('function must accept exactly two arguments (exception, client)')

    self._error_listeners.append(function)
  }

  /**
   * Sets up a request handler that will be called when a request with the given method 
   * has a path that matches the one specified.
   * 
   * If the path ends with a `/`, it also matches all routes that starts with the path 
   * so long as there is no other path that matches the request better. The exception 
   * to this is when the path is an ordinary `/` (root path) in which case it won't 
   * match any other route except for the root path.
   * 
   * For example, if the path is declared as `/user/`, it will match the request for 
   * `/user/record/1` unless another handle has been registered for `/user/record` in 
   * which case the handle for `/user/record` will handle the request since it is the 
   * handler for the closest path.
   * 
   * @param string method
   * @param string path
   * @param function(2) handler
   */
  handle(method, path, handler) {
    if !is_string(method)
      raise TypeError('argument 1 (method) must be a string')
    if !is_string(path)
      raise TypeError('argument 2 (path) must be a string')
    if !is_function(handler)
      raise TypeError('argument 3 (handler) must be a function')

    # var fn_arity = reflect.get_function_metadata(handler).arity
    # if fn_arity != 2 
    #   raise ArgumentError('handler must accept two arguments (request, response)')

    self._routes[method.upper()].set(path, handler)
  }

  /**
   * Sets up the handle to invoke when a request is not processed. That is, when it does 
   * not match a registered route and no `on_receive()` handler is set.
   * 
   * @param function(2) handler
   */
  none_handler(handler) {
    if !is_function(handler)
      raise TypeError('argument 1 (handler) must be a function')

    # var fn_arity = reflect.get_function_metadata(handler).arity
    # if fn_arity != 2 
    #   raise ArgumentError('handler must accept two arguments (request, response)')

    self._none_handler = handler
  }

  /**
   * Setup the given base_path to serve static files from the given directory.
   *
   * If cache is set to true, and a default value is not set for tag, static
   * file tagging will be automatically enabled.
   *
   * @param string base_path
   * @param string directory
   * @param number? cache_age = 0
   * @param bool? tag = false
   */
  serve_files(base_path, directory, cache_age, tag) {
    if !is_string(base_path)
      raise TypeError('argument 1 (base_path) must be a string')
    if !is_string(directory)
      raise TypeError('argument 2 (directory) must be a string')

    if cache_age == nil cache_age = 0
    if tag == nil tag = cache_age > 0

    if !is_number(cache_age)
      raise TypeError('argument 3 (cache_age) must be a number')
    if !is_bool(tag)
      raise TypeError('argument 4 (tag) must be a boolean')

    def static_file_handler(request, response) {
      if request.method == 'GET' and request.path.starts_with('/' + base_path.ltrim('/')) {
        var static_path = request.path[base_path.length(),].ltrim('/')
        var reader = file(os.join_paths(directory, static_path), 'rb')

        if reader.exists() {
          response.headers['Content-Type'] = mime.detect_from_name(static_path)

          # cache for 1 year
          if cache_age > 0 {
            response.headers['Cache-Control'] = 'public, max-age=${cache_age}, s-maxage=${cache_age}, immutable'
          }

          var content = reader.read()

          if tag {
            response.headers['Etag'] = 'W/"${hash.md5(content)}"'
          }

          response.write(content)
          content.dispose()
        } else if self._none_handler {
          self._none_handler(request, response)
        }
      }
    }

    self.on_receive(static_file_handler)
  }

  _get_response_header_string(headers, cookies) {
    var result
    for x, y in headers {
      result += '${x}: ${y}\r\n'
    }
    for x in cookies {
      result += 'Set-Cookie: ${x}\r\n'
    }
    return result
  }

  _process_received(message, client) {
    if !message or !client or !client.is_connected return

    var request = HttpRequest(),
        response = HttpResponse()

    if !request.parse(message, client)
      response.status = status.BAD_REQUEST

    var feedback = bytes(0)

    # If we have an error in the request message itself, we don't even want to 
    # forward processing to callers. 
    # This is a server level error and should terminate immediately.
    if response.status == status.OK {
      var on_receive_called = false,
          router_matched = false

      # call the received listeners on the request object.
      self._received_listeners.each(@(fn) {
        fn(request, response)
        on_receive_called = true
      })

      # Call the handler registered against this request if any.
      var router_method = self._routes.get(request.method)
      if router_method {
        var route_handler = router_method.get(request.path)
        if route_handler {
          route_handler(request, response)
          router_matched = true
        } else if request.path != '/' {
          var possible_handlers = {}

          # Find the handler that matches the request path the most
          # 
          # This is achieved by iterating through all registered routes 
          # for the request method and finding all possible matches. 
          # If at least one match is found, we return the path that 
          # matches the requested route the longest.
          for path, _handler in router_method {
            if path != "/" and  path.ends_with('/') and request.path.starts_with(path) {
              possible_handlers.set(path, _handler)
              router_matched = true
            }
          }

          if router_matched and possible_handlers {
            # find the longest route
            var longest_route = possible_handlers.keys().reduce(@(prev, x) {
              return x.length() > prev.length() ? x : prev
            })

            route_handler = possible_handlers.get(longest_route)
            possible_handlers.clear()
            route_handler(request, response)
          }
        }
      }

      if !on_receive_called and !router_matched {
        # the request was not handled by on_receive or a route handler.
        # we'll need to call the none_handler() to allow the user to 
        # return any kind of error such as 404, 500 etc.
        self._none_handler(request, response)
      }

      if response.body {
        var content_length = 'Content-Length: ${response.body.length()}\r\n'.to_bytes()
        feedback += content_length
        content_length.dispose()
      }
    }

    # clear file buffers...
    if request.files {
      for f in request.files  {
        f.content.dispose()
      }
    }

    var hdrs = self._get_response_header_string(response.headers, response.cookies).to_bytes()
    feedback += hdrs
    hdrs.dispose()
    
    feedback += '\r\n'.to_bytes()
    feedback += response.body
    response.body.dispose()

    var hdrv = (
      'HTTP/${response.version} ${response.status} ' +
      '${status.map.get(response.status, 'UNKNOWN')}\r\n'
    ).to_bytes()
    feedback =  hdrv + feedback
    hdrv.dispose()
           
    if client.is_connected {
      client.send(feedback)
    }

    # call the reply listeners.
    self._reply_listeners.each(@(fn) {
      fn(response)
    })

    feedback.dispose()
  }

  /**
   * Binds to the instance port and host and starts listening for incoming 
   * connection from HTTP clients.
   */
  listen() {
    if !self.socket.is_listening {
      self.socket.set_option(so.SO_REUSEADDR, is_bool(self.reuse_address) ? self.reuse_address : true)
      self.socket.bind(self.port, self.host)
      
      assert self.socket.listen(), 'socket failed to listen'

      self._is_listening = true
      var client

      while self._is_listening {

        catch {
          client = self.socket.accept()
          
          # call the connect listeners.
          self._connect_listeners.each(@(fn) {
            fn(client)
          })

          if client.is_connected {
            if is_number(self.read_timeout)
              client.set_option(so.SO_RCVTIMEO, self.read_timeout)
            if is_number(self.write_timeout)
              client.set_option(so.SO_SNDTIMEO, self.write_timeout)
          }

          self._process_received(client.receive(), client)
        } as e

        if client {
          # call the disconnect listeners.
          self._disconnect_listeners.each(@(fn) {
            fn(client)
          })

          client.close()

          if e {
            if self._error_listeners.length() == 0 {
              raise e
            } else {
              # call the error listeners.
              self._error_listeners.each(@(fn) {
                fn(e, client)
              })
            }
          }
        }
      }
    }
  }

  @to_string() {
    return '<HttpServer ${self.host}:${self.port}>'
  }
}
