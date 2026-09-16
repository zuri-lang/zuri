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

That one function is the whole assembly, and the order of its lines is the
order of the application.

**It builds the board first**, because everything else needs it. Passing
`data_dir` through rather than letting `Board` find it means a test, or a
second instance, can point at a different directory without touching
`config`.

**`auto_reload: true` on the template engine** re-reads a template when the
file changes, which is what you want while writing one. It is also the
first line to reconsider before a deployment, since it costs a filesystem
check per render.

**`port == nil ? config.PORT : port` is not `port or config.PORT`.** That
distinction is load-bearing here: port `0` is a legitimate value meaning
"let the operating system pick a free one", and `0` is falsy, so the `or`
form would silently turn it into the configured port. This is the
negative-and-zero trap from [Chapter 3](ch03-02-data-types.md) in a place
where it would really bite.

**`max_body_size` is set explicitly.** A server that accepts a request body
of any size is a server anyone can exhaust with a single request. 64 KiB is
generous for a form that carries a title and a column.

**Middleware is installed before the routes.** Middleware runs outermost
first, so anything registered here wraps every route added afterwards —
including `/health` and the static files below.

**The routes are registered by handing each module what it needs.**
`api.register(server, board)` and `pages.register(server, board, view)` are
the same shape: a function, given its dependencies, that attaches things to
the server. The page routes get the template engine; the API routes do not,
because they never render one.

**`/health` is defined here rather than in a route module**, because it is
about the process rather than about tasks. It reports a task count so that
a monitoring check proves the board actually loaded, not merely that the
socket answers.

**`serve_files()` comes last**, mapping `/static` onto a directory. It is
last because more specific routes should be registered before a catch-all
prefix.

And `build()` returns the server **without binding a socket**. Nothing here
listens. That separation is what lets the entry point decide how to serve
it — one process, a pool of isolates, or a test harness that never listens
at all — and it is what makes the next section possible.

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

A middleware takes three arguments: the request, the response, and a
`next` function. **`next()` is where everything below it runs** — the other
middleware, and eventually the route handler itself.

That one fact explains the shape of both functions here. Code written
*before* the `next()` call happens on the way in; code written *after* it
happens on the way out, once the handler has finished and the response is
populated. A middleware is therefore a pair of moments, not a single step,
and the call in the middle is the seam between them.

### Reading the Logger

```zuri,ignore
  server.use(@(request, response, next) {
    var started = time()

    next()

    var elapsed = ((time() - started) * 1000).round()
    log.info('${request.method} ${request.path} -> ${response.status} (${elapsed}ms)')
  })
```

The timestamp is taken on the way in, before anything else runs. The log
line is written on the way out, which is the only point at which
`response.status` is known — on the way in, nothing has decided it yet.

That is the whole reason timing middleware works: the same function body
runs at both ends of the request, with a local variable surviving in
between.

### Reading the Error Handler

```zuri,ignore
  server.use(@(request, response, next) {
    catch {
      next()
    } as error {
      _render_error(request, response, error)
    }
  })
```

This one has nothing before `next()` and nothing after it. All its work is
in the handler, and what it catches is **everything that raised anywhere
below it** — a route, the board, the model, the store.

That is the mechanism the whole application leans on. A `catch` around
`next()` catches the routes, because `next()` *is* the routes.

### Why the Logger Comes First

Middleware run **outermost first**, in the order they were added. So the
logger wraps the error handler, which wraps the routes:

```text
logger        starts the clock
  errors        catches whatever escapes
    routes        handles the request
  errors        turns an error into a status
logger        writes the line, with that status
```

Read the order bottom to top on the way out and the reason becomes obvious:
a request that **failed** still gets logged, and it gets logged with the
status the error handler chose rather than with whatever the response held
when the exception was raised.

Swap the two and a failing request would be logged before the error handler
had set a status — or not at all, if the exception escaped the logger
first.

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
