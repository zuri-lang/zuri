# Middleware, Logging and Errors

Two pieces of behaviour belong to every request and to no handler: logging
what happened, and turning a domain error into a status code. Both are
middleware.

<span class="filename">Filename: app.zu</span>

```zuri,ignore
import http
import log
import wire

import .config
import .models { TaskError }
import .routes { api, pages }
import .storage { Board, NotFoundError }

/**
 * Builds the application.
 *
 * @param ?number port: defaults to `config.PORT`; pass `0` to let the
 *    operating system choose a free one.
 * @param ?string data_dir: defaults to `config.DATA_DIR`
 * @returns HttpServer: bound to nothing yet; the caller decides how to
 *    serve it.
 */
def build(port, data_dir) {
  var board = Board(data_dir)

  var view = wire.wire({
    root: config.TEMPLATE_DIR,
    auto_reload: true,
  })

  var server = http.HttpServer(port == nil ? config.PORT : port, config.HOST)

  server.max_body_size = 64 * 1024

  _install_middleware(server)

  api.register(server, board)
  pages.register(server, board, view)

  server.get('/health', @(request, response) {
    response.json({ ok: true, tasks: board.all().length() })
  })

  server.serve_files('/static', config.STATIC_DIR)

  return server
}
```

`build()` returns a configured server without binding a socket. That
separation is what lets the entry point decide how to serve it, and it
means a caller can point the whole application at a different port and data
directory in one call.

`max_body_size` is set explicitly. A server that will accept a request body
of any size is a server anyone can exhaust.

## The Middleware

```zuri
/**
 * Request logging, and one error handler that turns every exception
 * the routes can raise into the right status code. Handlers below it
 * are then free to raise and say nothing about HTTP.
 */
def _install_middleware(server) {
  server.use(@(request, response, next) {
    var started = time()

    next()

    var elapsed = ((time() - started) * 1000).round()
    log.info('${request.method} ${request.path} -> ${response.status} (${elapsed}ms)')
  })

  server.use(@(request, response, next) {
    catch {
      next()
    } as error {
      _render_error(request, response, error)
    }
  })
}
```

A middleware takes the request, the response and a `next` function, and
`next()` is where everything below it runs. Code before the call happens on
the way in, code after it on the way out. Middleware run outermost first,
in the order they were added.

The logger goes first so it wraps the error handler, which means a request
that failed still gets logged, with the status the error handler chose.

### The Module Already Has One

`http.middleware.logger()` is a ready-made request logger, and in a real
application it is what you would reach for:

```zuri,ignore
import http.middleware

server.use(middleware.logger())
```

It writes the Common Log Format extended with the response time, which
every log analyser already parses, and it takes a `sink` to send lines
somewhere other than standard output, a `format` to build the line
yourself, and `trust_proxy` to log the forwarded client address instead of
the peer.

The nine lines above are written out by hand here because a middleware you
have read the whole of teaches more than one you called. Once you can see
what `next()` does, swap in the real one.

## One Place That Knows About Status Codes

```zuri
def _render_error(request, response, error) {
  var status = 500
  var body = { error: 'internal error' }

  if instance_of(error, NotFoundError) {
    status = 404
    body = { error: error.message }
  } else if instance_of(error, TaskError) {
    status = 422
    body = { error: error.message, field: error.field }
  } else {
    log.error('unhandled: ' + error.message)
  }

  if request.path.starts_with('/api') or request.wants_json() {
    response.json(body, status)
    return
  }

  response.html('<h1>' + status + '</h1><p>' + body.error + '</p>', status)
}
```

This is the payoff for defining `NotFoundError` and `TaskError` as real
classes instead of raising `Error('not found')` everywhere.
`instance_of()` maps a domain error to a status code, once.

Three details matter.

**Unknown errors are a 500 with a generic body**, and the real message goes
to the log. An error message can contain a path, a query, or a fragment of
a file. It goes where operators can read it, not where users can.

**The response shape follows the client.** A request under `/api`, or one
whose `Accept` header asks for JSON, gets JSON. Everything else gets HTML.
`request.wants_json()` reads the header for you.

**A handler that has already committed a response** is not overwritten,
because the error handler only ever runs when `next()` raised.

## What It Looks Like

```console
2026-09-11T02:29:55+01:00 INFO [taskboard]: task board on http://127.0.0.1:8000
2026-09-11T02:29:55+01:00 INFO [taskboard]: POST /api/tasks -> 201 (15ms)
2026-09-11T02:29:55+01:00 INFO [taskboard]: GET / -> 200 (409ms)
2026-09-11T02:30:02+01:00 INFO [taskboard]: POST /api/tasks -> 422 (3ms)
2026-09-11T02:30:02+01:00 INFO [taskboard]: GET /api/tasks/nope -> 404 (0ms)
2026-09-11T02:30:03+01:00 INFO [taskboard]: GET /static/app.css -> 200 (1ms)
```

`log` puts the timestamp, the level and the module name on every line and
colours it when the output is a terminal. The module name comes from where
the call was made, so a message from `routes/api.zu` says so without being
told.

The 409ms on that first `GET /` is Wire compiling the two templates. Every
render after it walks the cached instruction tree instead, which is the
1-3ms the later lines show.

## Middleware Worth Adding

`http.middleware` has ready-made pieces for the things every application
eventually wants: CORS, compression, rate limiting, and authentication.
Each is a function you pass to `server.use()`, in the position you want it
in the chain.
