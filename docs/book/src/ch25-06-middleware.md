# Middleware, Logging and Errors

Two kinds of behavior apply to every request but don't belong in any
handler: logging what happened, and turning a domain error into a status
code. Both are implemented as middleware.

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

  var view = wire({ root: config.TEMPLATE_DIR, auto_reload: true })

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

That one function puts the whole application together, and the order of its
lines is the order of the application.

**It creates the board first**, because everything else needs it. Passing
`data_dir` through, instead of letting `Board` find it, means that a test or
a second instance can use a different directory without changing `config`.

**`auto_reload: true` on the template engine** reads a template again when
its file changes, which is what you want while you're writing one. It's also
the first setting to reconsider before you deploy, because it costs a file
system check on every render.

**`port == nil ? config.PORT : port` isn't the same as
`port or config.PORT`.** The difference matters here. Port `0` is a valid
value that means "let the operating system choose a free port", but `0` is
falsy, so the `or` version would silently replace it with the configured
port. This is the problem with zero from [Chapter 3](ch03-02-data-types.md),
in a place where it would really cause trouble.

**`max_body_size` is set explicitly.** A server that accepts request bodies
of any size can be exhausted by anyone with a single request. 64 KiB is
plenty for a form that sends a title and a column.

**Middleware is installed before the routes.** Middleware runs from the
outside in, so anything registered here wraps every route added afterwards,
including `/health` and the static files below.

**Routes are registered by passing each module what it needs.**
`api.register(server, board)` and `pages.register(server, board, view)` work
the same way: each is a function that receives its dependencies and adds
routes to the server. The page routes get the template engine, and the API
routes don't, because they never render a template.

**`/health` is defined here instead of in a route module**, because it's
about the process rather than about tasks. It reports the number of tasks,
so that a monitoring check can confirm that the board actually loaded, not
just that the server responds.

**`serve_files()` comes last** and maps `/static` to a directory. It's last
because more specific routes should be registered before a route that
matches a whole prefix.

And `build()` returns the server **without binding a socket**. Nothing here
starts listening. That separation lets the entry point decide how to run the
server, whether as a single process, with a pool of isolates, or in a test
harness that never listens at all, and it's what makes the next section
possible.

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
    log.info(
      '${request.method} ${request.path} -> ${response.status} (${elapsed}ms)',
    )
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

A middleware function takes three arguments: the request, the response and a
`next` function. **Calling `next()` runs everything that comes after the
middleware**, including the other middleware and finally the route handler
itself.

That explains the structure of both functions here. Code *before* the call
to `next()` runs on the way in, and code *after* it runs on the way out,
once the handler has finished and the response has been filled in. So a
middleware function covers two points in time, not just one, and the call in
the middle separates them.

### How the Logger Works

```zuri,ignore
server.use(@(request, response, next) {
  var started = time()

  next()

  var elapsed = ((time() - started) * 1000).round()
  log.info(
    '${request.method} ${request.path} -> ${response.status} (${elapsed}ms)',
  )
})
```

The time is recorded on the way in, before anything else runs. The log line
is written on the way out, because that's the only point at which
`response.status` is known. On the way in, nothing has set it yet.

That's why timing middleware works at all. The same function body runs at
both ends of the request, and a local variable keeps its value in between.

### How the Error Handler Works

```zuri,ignore
server.use(@(request, response, next) {
  catch {
    next()
  } as error {
    _render_error(request, response, error)
  }
})
```

This middleware doesn't do anything before or after `next()`. All of its
work happens in the handler, and it catches **every error raised anywhere
after it**, whether in a route, the board, the model or the store.

The whole application depends on this. A `catch` around `next()` catches
errors from the routes, because `next()` *is* the routes.

### Why the Logger Comes First

Middleware runs **from the outside in**, in the order it was added. So the
logger wraps the error handler, which wraps the routes:

```text
logger        starts the clock
  errors        catches whatever escapes
    routes        handles the request
  errors        turns an error into a status
logger        writes the line, with that status
```

If you follow the order from the bottom up on the way out, the reason
becomes clear. A request that **failed** is still logged, and it's logged
with the status the error handler chose, rather than whatever the response
contained when the error was raised.

If you swapped the two, a failed request would be logged before the error
handler had set its status, or it wouldn't be logged at all if the error
went past the logger first.

### The Built-in Logger

`http.middleware.logger()` is a ready-made request logger, and in a real
application, it's what you'd use:

```zuri,ignore
import http.middleware

server.use(middleware.logger())
```

It writes the Common Log Format with the response time added, which every
log analysis tool already understands. It accepts a `sink` to send lines
somewhere other than standard output, a `format` to build each line
yourself, and `trust_proxy` to log the forwarded client address instead of
the address of the connection.

The nine lines above are written out here because you learn more from a
middleware you've read in full than from one you've only called. Once you
understand what `next()` does, switch to the built-in one.

## A Single Place for Status Codes

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

This is what you gain from defining `NotFoundError` and `TaskError` as real
classes, instead of raising `Error('not found')` everywhere. `instance_of()`
maps each domain error to a status code, in one place.

There are three details that matter.

**Unknown errors become a 500 with a generic message**, and the real message
goes to the log. An error message can contain a path, a query or part of a
file. It belongs where the people running the service can read it, not where
users can.

**The format of the response depends on the client.** A request under
`/api`, or one whose `Accept` header asks for JSON, gets a JSON response,
and everything else gets HTML. `request.wants_json()` checks the header for
you.

**A response that a handler has already sent** isn't overwritten, because
the error handler only runs when `next()` raised an error.

## The Output

```console
2026-09-11T02:29:55+01:00 INFO [taskboard]: task board on http://127.0.0.1:8000
2026-09-11T02:29:55+01:00 INFO [taskboard]: POST /api/tasks -> 201 (15ms)
2026-09-11T02:29:55+01:00 INFO [taskboard]: GET / -> 200 (409ms)
2026-09-11T02:30:02+01:00 INFO [taskboard]: POST /api/tasks -> 422 (3ms)
2026-09-11T02:30:02+01:00 INFO [taskboard]: GET /api/tasks/nope -> 404 (0ms)
2026-09-11T02:30:03+01:00 INFO [taskboard]: GET /static/app.css -> 200 (1ms)
```

`log` adds the time, the level and the module name to every line, and uses
colors when the output is a terminal. The module name is based on where the
call was made, so a message from `routes/api.zu` shows that without you
having to say so.

The 409ms on the first `GET /` is the time Wire took to compile the two
templates. Every later render uses the cached instruction tree, which is why
the later lines show 1 to 3ms.

## Other Useful Middleware

`http.middleware` has ready-made middleware for what every application
eventually needs, such as CORS, compression, rate limiting and
authentication. Each one is a function you pass to `server.use()`, at the
position in the chain where you want it.
