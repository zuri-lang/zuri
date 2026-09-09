# HTTP

The `http` module is Zuri's HTTP stack: a client, a server, and the
pieces both are built from. It speaks HTTP/1.1 and HTTP/2, over
cleartext or TLS, and it is meant to face the internet directly.

That last part is a design decision rather than a boast. Most
language runtimes ship an HTTP server that is fine for development and
then expect a reverse proxy in front of it in production — something
else terminates TLS, serves the static files, compresses the
responses, enforces the request limits, and speaks HTTP/2 to the
browser. Everything on that list is in this module, done properly,
because a standard library that leaves them out is not really shipping
a server.

- [Introduction](#introduction)
  - [A First Request](#a-first-request)
  - [A First Server](#a-first-server)
- [Making Requests](#making-requests)
  - [The Shared Client](#the-shared-client)
  - [Building a Client](#building-a-client)
  - [Request Bodies](#request-bodies)
  - [Reading a Response](#reading-a-response)
  - [Query Parameters and Headers](#query-parameters-and-headers)
  - [Authentication](#authentication)
  - [Redirects](#redirects)
  - [Cookies](#cookies)
  - [Timeouts and Retries](#timeouts-and-retries)
  - [Streaming a Response](#streaming-a-response)
  - [TLS and Certificates](#tls-and-certificates)
- [Serving Requests](#serving-requests)
  - [Routing](#routing)
  - [The Request Object](#the-request-object)
  - [The Response Object](#the-response-object)
  - [Middleware](#middleware)
  - [Errors](#errors)
  - [Static Files](#static-files)
  - [Streaming a Response Body](#streaming-a-response-body)
  - [Cookies and Sessions](#cookies-and-sessions)
  - [Content Negotiation](#content-negotiation)
- [TLS](#tls)
- [HTTP/2](#http2)
- [WebSockets](#websockets)
- [Server-Sent Events](#server-sent-events)
- [Reverse Proxying](#reverse-proxying)
- [Running in Production](#running-in-production)
  - [Using More Than One Core](#using-more-than-one-core)
  - [Limits](#limits)
  - [Graceful Shutdown](#graceful-shutdown)
  - [Behind Another Proxy](#behind-another-proxy)
  - [Security Headers](#security-headers)
- [What the Module Refuses](#what-the-module-refuses)
- [Module Reference](#module-reference)

## Introduction

### A First Request

```zuri
import http

echo http.get('https://example.com').as_text()
```

`get()`, `post()`, `put()`, `patch()`, `delete()`, `head()`,
`options()` and `trace()` all exist at module level and all go through
one shared client, which keeps its connections open between calls.

### A First Server

```zuri
import http

var server = http.server(3000)

server.get('/', @(request, response) {
  response.html('<h1>Hello</h1>')
})

server.get('/users/:id', @(request, response) {
  response.json({ id: request.param('id') })
})

server.listen()
```

Two things are already true of that server that are worth noticing:
`GET /users/42` matches the second route and hands the handler
`'42'`, and a `HEAD /` request is answered by the first route with the
headers a `GET` would have produced and no body, because that is what
HEAD means.

## Making Requests

### The Shared Client

The module-level functions use a single `HttpClient` that lives for
the life of the program. It pools connections, so a second request to
a host it has already talked to skips the TCP handshake and, over
HTTPS, the TLS handshake as well.

```zuri
import http

var page = http.get('https://example.com')

echo page.status          # 200
echo page.headers.get('content-type')
echo page.as_text()
```

Reach that client with `http.shared_client()` when you want a setting
to apply to every casual call in a program, and build your own for
anything more specific.

### Building a Client

```zuri
var api = http.client('https://api.example.com', {
  headers: { 'Authorization': 'Bearer ' + token },
  read_timeout: 5000,
})

var me = api.get('/users/me').raise_for_status().as_dict()
```

A client built with a base URL prefixes it onto any target that is not
already absolute, so the rest of the program addresses the service by
path. Everything in the options dictionary is a field of `HttpClient`,
plus `headers`, and every one of them can also be set afterwards:

```zuri
api.user_agent = 'my-service/2.1'
api.max_redirects = 3
api.set_header('X-Client-Version', '2.1')
```

> **Note**
> Build a client once and keep it. A fresh client per request throws
> away the connection pool, the cookie jar and the TLS configuration —
> which is to say, it throws away most of what a client is for.

### Request Bodies

The second argument to `post()`, `put()` and `patch()` is the body,
and what it is decides how it is sent:

```zuri
api.post('/items', { name: 'Widget', price: 9 })   # JSON
api.post('/items', 'raw text')                     # sent as-is
api.post('/items', file_bytes)                     # sent as-is
```

For anything else, name it in the options dictionary:

| Option | Sends |
| --- | --- |
| `body` | a string or bytes, exactly as given |
| `json` | any value, JSON-encoded |
| `form` | a dictionary, `application/x-www-form-urlencoded` |
| `multipart` | a `MultipartBuilder` |

```zuri
api.post('/login', nil, {
  form: { username: 'ada', password: secret },
})
```

A file upload is a `MultipartBuilder`:

```zuri
var form = http.MultipartBuilder()
form.add_field('title', 'Holiday')
form.add_file('photo', 'beach.jpg', file('beach.jpg', 'rb').read(), 'image/jpeg')

api.post('/photos', form)
```

The boundary is generated from the platform's secure random source,
not from a timestamp or a counter — a boundary a peer can predict is a
boundary a peer can write into a field value to forge extra parts.

### Reading a Response

```zuri
var response = api.get('/items/1')

response.status              # 200
response.reason_phrase()     # 'OK'
response.version             # '1.1' or '2'
response.headers.get('etag')

response.as_text()           # the body, decoded as UTF-8
response.as_dict()           # the body, parsed as JSON
response.as_bytes()          # the body, untouched

response.is_ok()             # 2xx
response.is_redirect()       # 3xx
response.is_error()          # 4xx or 5xx
```

`raise_for_status()` turns a 4xx or 5xx into a `StatusError` and
returns the response otherwise, so it can be used inline. The response
stays reachable on the raised error, which matters because that is
where an API usually explains what went wrong:

```zuri
catch {
  var data = api.get('/items/1').raise_for_status().as_dict()
} as error {
  echo error.response.status
  echo error.response.as_text()
}
```

A compressed response body is decompressed automatically — gzip,
deflate, brotli and zstd — and the `Content-Encoding` header is
removed once it has been, so nothing downstream tries to decode it a
second time.

### Query Parameters and Headers

```zuri
api.get('/search', {
  query: { q: 'zuri', page: 2, tag: ['new', 'featured'] },
  headers: { 'Accept-Language': 'en-GB' },
})
```

A list value repeats the parameter, which is how a query string
carries more than one value under one name. Per-request headers are
merged over the client's own.

### Authentication

```zuri
api.get('/me', { auth: ['bearer', token] })
api.get('/me', { auth: ['basic', 'ada', 'lovelace'] })
```

Credentials passed this way are scoped to the origin you addressed. If
the response is a redirect to a different scheme, host or port, they
are dropped rather than followed — a redirect to a host of someone
else's choosing is otherwise a way to collect whatever `Authorization`
header was in flight.

### Redirects

Redirects are followed by default, up to `max_redirects` (10), and the
number followed is on the response:

```zuri
var final = http.get('https://example.com/old')

final.redirects    # how many were followed
final.responder    # the URL that finally answered
```

A `303`, and in practice a `301` or `302`, becomes a `GET` with no
body when followed — that is what every browser and every other client
does, and a server that meant otherwise should have sent `307` or
`308`, both of which preserve the method and body here.

Turn following off per request or per client:

```zuri
http.get(url, { follow_redirects: false })
```

`head()` does not follow redirects by default, since the point of a
HEAD is usually to inspect the very response a redirect would hide.

### Cookies

A client with a cookie jar carries cookies between requests, so a
login and the requests after it behave the way a browser would:

```zuri
var session = http.client('https://example.com')
session.enable_cookies()

session.post('/login', nil, { form: { user: 'ada', password: secret } })
session.get('/dashboard')     # sends the session cookie
```

The jar applies RFC 6265's matching rules: a cookie without a `Domain`
is host-only, one with a `Domain` reaches subdomains, a path prefix
matches below itself, and a `Secure` cookie is never sent over
cleartext. A server trying to set a cookie for a domain it does not
control is refused.

### Timeouts and Retries

```zuri
var api = http.client('https://api.example.com', {
  connect_timeout: 5000,
  read_timeout: 10000,
  write_timeout: 10000,
})

api.get('/slow', { timeout: 30000 })   # this one request
```

All timeouts are milliseconds. `max_retries` retries a failed request,
but only when the method is idempotent — replaying a `POST` can mean
charging a card twice, so `GET`, `HEAD`, `PUT`, `DELETE`, `OPTIONS`
and `TRACE` are retried and nothing else is.

A pooled connection the far end closed while it was idle fails on the
next write and looks exactly like a network failure. That specific
case is retried once for an idempotent request even at the default
`max_retries` of zero, because otherwise every connection a server
reaps would surface as a spurious error.

### Streaming a Response

A large response does not have to be held in memory:

```zuri
var response = api.get('/exports/large.csv', { stream: true })
var out = file('large.csv', 'wb')

while true {
  var chunk = response.body_reader.read(65536)
  if chunk.length() == 0 {
    break
  }
  out.write(chunk)
}

out.close()
api.finish(response)
```

`finish()` hands the connection back to the pool once you are done
with the body. Until then the connection is yours, since there is no
way for the client to know when you have finished reading.

### TLS and Certificates

Certificates are verified against the platform's trust store by
default. To talk to a service with an internal or self-signed
certificate, trust its authority:

```zuri
api.add_ca(file('/etc/ssl/internal-ca.pem').read())
```

There is also `verify = false`, which turns verification off
completely. It exists for local development, and it makes the
connection encrypted but unauthenticated — which is to say, trivially
interceptable. `add_ca()` is the right answer everywhere else.

## Serving Requests

### Routing

A route pattern is made of literal segments, `:name` parameters, and
an optional trailing catch-all:

| Pattern | Matches | Captures |
| --- | --- | --- |
| `/users` | `/users` | |
| `/users/:id` | `/users/42` | `id` = `'42'` |
| `/users/:id/posts/:post` | `/users/4/posts/7` | `id`, `post` |
| `/files/` + `*path` | `/files/css/site.css` | `path` = `'css/site.css'` |

```zuri
server.get('/users/:id', @(request, response) {
  response.json({ id: request.param('id') })
})
```

A literal segment always beats a parameter, and a parameter always
beats a catch-all, so `/users/new` and `/users/:id` can both exist and
the specific one wins — regardless of which was registered first.
Matching runs over a trie, so a router with a thousand routes costs
the same per request as one with ten.

`get()`, `post()`, `put()`, `patch()`, `delete()`, `head()` and
`options()` register for one method; `any()` registers for all of
them; `handle()` takes the method as an argument.

Three things are answered without a handler:

- `HEAD` falls back to the `GET` route for the same path, and the body
  is dropped on the way out.
- `OPTIONS` answers `204` with an `Allow` header listing what the path
  actually accepts.
- A path that exists for other methods answers `405`, again with
  `Allow` — rather than a `404`, which would be a lie.

Name a route to build URLs from it later:

```zuri
server.get('/users/:id', show_user, 'user.show')

server.routes().url_for('user.show', { id: 42 })    # '/users/42'
```

### The Request Object

```zuri
server.post('/items', @(request, response) {
  request.method          # 'POST'
  request.path            # decoded and normalised
  request.target          # exactly as it arrived on the wire
  request.version         # '1.1' or '2'
  request.secure          # whether it came over TLS

  request.param('id')                 # a route parameter
  request.query_param('page', '1')    # a query parameter, with a default
  request.header('accept')            # a header field
  request.cookie('session')           # a cookie

  request.text()          # the body as text
  request.json_body()     # the body parsed as JSON
  request.form()          # urlencoded or multipart fields
  request.file('avatar')  # an uploaded file
})
```

`path` is percent-decoded and then normalised, in that order — which
is the only order that works, since `%2e%2e%2f` is `../` written to
survive a naive check. Route on `path`; `target` is the raw form and
matching on it is how directory traversal gets through.

An uploaded file is an `UploadedFile`:

```zuri
var upload = request.file('avatar')

upload.filename       # what the client claimed
upload.safe_name()    # that, reduced to one path-safe segment
upload.content_type   # what the client declared
upload.size()
upload.save_to('/var/uploads/' + upload.safe_name())
```

`filename` and `content_type` are both attacker-controlled and say
nothing about what the bytes are. `safe_name()` drops any directory
component — including a Windows one — and reduces the rest to letters,
digits, `.`, `-` and `_`.

### The Response Object

```zuri
response.text('plain')                    # text/plain
response.html('<h1>hi</h1>')              # text/html
response.json({ ok: true })               # application/json
response.xml('<doc/>')                    # application/xml
response.write('more')                    # append, no content type

response.file('/var/www/report.pdf')      # streamed from disk
response.download('/var/www/report.pdf')  # ...as an attachment
response.render('pages/home', { user })   # a Wire template

response.redirect('/elsewhere')           # 302 by default
response.redirect('/elsewhere', 308)      # method-preserving

response.status = 201
response.header('X-Thing', 'value')
response.content_type('text/csv')
response.cache_for(3600)
response.no_cache()
```

Every writer sets a sensible `Content-Type` alongside the body, and
each returns the response, so they chain.

`file()` streams from disk rather than reading the file into memory,
which is what makes serving something larger than you would like to
hold in memory a one-liner.

### Middleware

A middleware takes `(request, response, next)` and decides whether the
rest of the chain runs:

```zuri
server.use(@(request, response, next) {
  var started = time()
  next()
  echo '${request.method} ${request.path} ${response.status} ' +
    '${(time() - started) * 1000}ms'
})
```

Not calling `next()` is how a middleware short-circuits, which is
exactly what an authentication or rate-limiting layer wants:

```zuri
server.use(@(request, response, next) {
  if request.header('x-api-key') != expected {
    response.json({ error: 'unauthorized' }, 401)
    return
  }
  next()
})
```

Middleware run in the order they were added, outermost first — so a
logger added first sees the final status of everything added after it.

`http.middleware` has the ones most services end up needing:

```zuri
import http.middleware

server.use(middleware.request_id())
server.use(middleware.logger())
server.use(middleware.security_headers())
server.use(middleware.cors({ origins: ['https://app.example.com'] }))
server.use(middleware.rate_limit({ limit: 100, window: 60 }))
server.use(middleware.etag())
server.use(middleware.basic_auth(@(user, password) {
  return user == 'admin' and http.util.secure_equals(password, secret)
}))
```

Compare secrets with `http.util.secure_equals()` rather than `==`, so
that a wrong guess and a nearly-right one take the same time.

### Errors

A handler that raises becomes a `500`, and the exception message never
reaches the client — an exception message routinely carries a file
path, a query, or a fragment of the data being processed, and none of
that belongs in a reply to whoever triggered it.

```zuri
server.on_error(@(error, connection) {
  log.error('${error.message}')
})

server.error_handler(@(request, response, error) {
  response.status = 500
  response.render('errors/500')
})

server.not_found(@(request, response) {
  response.status = 404
  response.render('errors/404')
})
```

`on_error()` listeners see everything, including connection-level
failures that never reached a handler. `error_handler()` produces the
response body for a failed request; without one, the server sends a
bare `500`.

### Static Files

```zuri
server.serve_files('/static', './public', {
  cache_age: 86400,
  precompressed: true,
})
```

That single call brings with it the things a static file needs to
actually behave well in front of a browser or a CDN:

- `ETag` and `Last-Modified`, and `304 Not Modified` for a conditional
  request that still holds
- `Range` requests, answered with `206` and a `Content-Range`, and
  `416` for a range that falls outside the file
- `If-Match`, `If-Unmodified-Since`, `If-None-Match`, `If-Modified-Since`
  and `If-Range`, evaluated in the order RFC 9110 lays down
- `Content-Type` from the file extension
- `index.html` for a request that names a directory
- with `precompressed`, a `.br` or `.gz` sibling served in place of the
  original when the client accepts that coding

Every request path is percent-decoded, normalised, joined to the root,
and then checked to still be inside the root — all three, because
doing any two of them is not enough. Dotfiles are not served at all:
`.env`, `.git` and `.htpasswd` all end up in a web root at some point
in a project's life, and none of them should ever go out.

For a single-page application, `fallback` serves the shell for any
path that names no file, which is what makes client-side routes work
on reload:

```zuri
server.serve_files('/', './dist', { fallback: 'index.html' })
```

### Streaming a Response Body

A response body can come from a callback instead of memory:

```zuri
server.get('/export.csv', @(request, response) {
  response.content_type('text/csv')

  response.stream(@(writer) {
    writer.write('id,name\n')

    for row in rows {
      writer.write('${row.id},${row.name}\n')
      writer.flush()
    }
  })
})
```

Unless a `Content-Length` was set beforehand, the body is framed with
chunked transfer encoding on HTTP/1.1 and as an ordinary DATA stream
on HTTP/2 — the handler does not have to know which.

### Cookies and Sessions

```zuri
response.set_cookie('session', token, {
  max_age: 86400,
  secure: true,
  same_site: 'Strict',
})

response.clear_cookie('session')
```

`http_only` defaults to `true` and `same_site` to `'Lax'`, which are
what a session cookie should have; pass them explicitly to opt out. A
cookie whose name carries the `__Secure-` or `__Host-` prefix has that
prefix's rules applied for it, rather than being sent in a form the
browser will silently refuse to store.

### Content Negotiation

```zuri
import http.negotiate

var type = negotiate.best_match(
  request.header('accept'),
  ['application/json', 'text/html']
)
```

`best_match()` picks what the client would most like out of what you
can produce, honouring quality weights, with your own order deciding
ties. `request.accepts('text/html')` and `request.wants_json()` cover
the common cases.

## TLS

```zuri
var server = http.server(443, '0.0.0.0')
server.load_certs('/etc/certs/site.crt', '/etc/certs/site.key')
server.listen()
```

Or from strings, with more control:

```zuri
server.use_tls(cert_chain_pem, private_key_pem, {
  min_version: '1.2',
  client_ca: internal_ca_pem,
  require_client_cert: true,
})
```

`cert_chain` must be the server certificate followed by any
intermediates. Leaving the intermediates out is the single most common
TLS misconfiguration, and it fails only for the clients that do not
happen to have them cached — which is to say, it fails in a way that
looks fine from your own browser.

> **Note**
> The certificate and key are parsed when the first handshake runs,
> not when they are set, so a malformed or mismatched pair surfaces as
> a failed connection rather than as an error from `use_tls()`. Make a
> request against the server as part of starting it if you want to
> find out early.

Turning on TLS also advertises `h2` through ALPN, so a browser gets
HTTP/2 without anything else being configured.

## HTTP/2

HTTP/2 needs nothing turned on. `HttpServer` speaks it when ALPN
negotiates `h2` over TLS, and when a cleartext client opens with the
HTTP/2 connection preface. `HttpClient` uses it whenever a server
negotiates `h2` for an HTTPS connection. Routes, middleware and
handlers are the same code either way; `request.version` is `'2'` when
it applies.

What that gets you is one connection carrying every request, HPACK
header compression, and flow control — rather than the six-connection
scramble HTTP/1.1 forces a browser into.

Set `server.http2 = false` to turn it off, which also stops `h2` being
advertised in ALPN.

`http.h2` exposes the machinery underneath — the frame codec, HPACK
and its Huffman coding — for anything that needs to speak the protocol
directly.

> **Note**
> Streams are multiplexed on the wire but handled in the order they
> complete: one connection is served by one isolate, so a request that
> arrives while another is being handled is read, buffered, and
> answered next rather than in parallel. That is a throughput
> property, not a correctness one, and `http.serve()` is how you get
> more than one connection served at a time.

## WebSockets

```zuri
import http.websocket as ws

server.get('/ws', @(request, response) {
  var socket = ws.accept(request, response, { protocols: ['chat'] })

  while socket.is_open() {
    var message = socket.receive()

    if message == nil or message.is_close() {
      break
    }

    socket.send('you said: ' + message.text())
  }
})
```

`accept()` completes the RFC 6455 handshake and takes the connection
over; the server writes nothing more on it. Fragmented messages are
reassembled, ping frames are answered, and a close frame from the peer
is answered and then handed back so you can see the code and reason.

The client side is `ws.connect()`:

```zuri
var socket = ws.connect('wss://example.com/ws')

socket.send('hello')
echo socket.receive().text()
socket.close()
```

Client frames are masked with a key from the platform's secure random
source, as the RFC requires — the mask is what stops a hostile page
from steering a proxy into caching a forged response.

## Server-Sent Events

Where a WebSocket gives you a duplex connection, server-sent events
give you a long-lived response body and nothing else — which is all a
live feed of updates needs, and it survives proxies that would refuse
an upgrade.

```zuri
import http.sse

server.get('/events', @(request, response) {
  sse.stream(response, @(events) {
    for update in updates {
      events.send(update, 'update', update.id)
    }
  })
})
```

A browser's `EventSource` reconnects on its own and sends back the
last id it saw; `sse.last_event_id(request)` reads it, which is how a
stream resumes where it left off instead of replaying from the start.

## Reverse Proxying

```zuri
var api = http.ReverseProxy('http://127.0.0.1:9000', {
  strip_prefix: '/api',
})

server.any('/api/' + '*path', @(request, response) {
  api.handle(request, response)
})
```

Hop-by-hop headers are stripped in both directions — including any
field the message's own `Connection` header names, which is how an
endpoint declares an extra one. Forwarding a client-supplied
`Transfer-Encoding` to an upstream that frames it differently is the
classic request-smuggling setup, so this is not optional.

`X-Forwarded-For`, `X-Forwarded-Proto`, `X-Forwarded-Host` and RFC
7239's `Forwarded` are added, and the response body is streamed rather
than buffered.

Several upstreams get a `LoadBalancer`, which is round-robin with an
upstream that fails taken out of rotation for a while rather than
retried on every request:

```zuri
var pool = http.LoadBalancer([
  'http://10.0.0.1:9000',
  'http://10.0.0.2:9000',
], { recovery_time: 30 })
```

## Running in Production

### Using More Than One Core

`listen()` serves connections on the calling isolate, one at a time.
`http.serve()` runs the same pipeline across a pool of isolates: one
accept loop hands each connection to whichever worker takes it next.

```zuri
# app.zu
import http

def setup(server) {
  server.get('/', @(request, response) {
    response.text('hello')
  })

  server.serve_files('/static', './public', { cache_age: 86400 })
}
```

```zuri
# main.zu
import http
import .app

http.serve(app.setup, {
  host: '0.0.0.0',
  port: 3000,
  workers: 8,
})
```

`setup` is called once inside each worker with that worker's own
`HttpServer`. It has to be a function defined in a module rather than
a closure in the main script: an isolate resolves a function by module
binding, and a closure that captured an imported module cannot cross
the boundary at all.

Isolates share no memory, so anything a worker needs — a cache, a
connection pool, a counter — is per worker. That is the trade the
model makes: no locks and no shared-heap garbage collection pauses, in
exchange for state that has to be either per worker or in something
outside the process.

`backlog` bounds how many accepted connections may queue for a free
worker. Bounding it is deliberate: an unbounded queue under load means
accepting connections faster than they can be served and then
answering all of them late, rather than letting the kernel's own
listen backlog apply back-pressure.

### Limits

Every part of a request whose size a peer controls has a ceiling:

| Setting | Default | Bounds |
| --- | --- | --- |
| `max_line_size` | 8 KiB | the request line, and each header line |
| `max_header_size` | 64 KiB | the header section |
| `max_header_count` | 100 | how many header fields |
| `max_body_size` | 10 MiB | the request body |
| `header_timeout` | 10s | how long the whole head may take to arrive |
| `keep_alive_timeout` | 5s | how long an idle connection is held |
| `max_keep_alive_requests` | 1000 | how many requests one connection may serve |

`header_timeout` is the one that is easy to leave out and matters
most: without it, a client sending one byte every few seconds holds a
connection open indefinitely while never tripping any single read's
timeout. That is what slowloris is.

Raise `max_body_size` deliberately for a service that takes uploads,
rather than discovering it by accident:

```zuri
server.max_body_size = 100 * 1024 * 1024
```

A request that announces a body over the limit is refused with `413`
before a byte of it is read, which is the whole point of the header.

### Graceful Shutdown

```zuri
import os

http.serve(app.setup, {
  port: 3000,
  on_ready: @(address, stop) {
    echo 'listening on ${address}'
    os.on_signal('SIGINT', @(signal) { stop() })
    os.on_signal('SIGTERM', @(signal) { stop() })
  },
})
```

`stop()` closes the listener, which is what breaks the blocking
accept; `serve()` then waits for every worker to finish the connection
it is on before returning. For a single-isolate server, `close()` does
the same thing.

### Behind Another Proxy

If something else really is in front — a CDN, a load balancer you
control — tell the server, and tell it which addresses to believe:

```zuri
server.trust_proxy = true
server.trusted_proxies = ['10.0.0.1', '10.0.0.2']
```

`request.client_ip(true, server.trusted_proxies)` then walks the
forwarding chain in from the proxy end and returns the rightmost entry
that is not itself a trusted proxy. Taking the leftmost entry — the
common shortcut — hands an attacker whatever client address they care
to claim, which matters the moment an address is used for rate
limiting, allowlisting or an audit trail.

Left off, forwarding headers are ignored entirely and the peer is the
client. That is the right default: those headers are request headers,
which is to say anyone can write anything in them.

### Security Headers

```zuri
server.use(middleware.security_headers({
  content_security_policy: "default-src 'self'",
  hsts_subdomains: true,
}))
```

`X-Content-Type-Options`, `X-Frame-Options` and `Referrer-Policy` get
sensible defaults, `Strict-Transport-Security` is sent only over HTTPS
where it is meaningful, and `Content-Security-Policy` is left unset
because it is too application-specific to guess at.

## What the Module Refuses

Some of what this module does is refuse things, and it is worth being
explicit about which, since each refusal is a request that some other
implementation would have accepted:

- A header field name with whitespace before its colon. `Content-Length : 5`
  is read as a length by some parsers and as an unknown field by
  others, and that disagreement is a smuggled request.
- Two `Content-Length` fields that disagree, for the same reason.
- A `Content-Length` alongside a `Transfer-Encoding`.
- A `Transfer-Encoding` whose last coding is not `chunked`.
- An obsolete folded header line — RFC 9112 deprecated it, and
  intermediaries unfold it differently.
- An HTTP/1.1 request with no `Host`, or with two.
- A header value containing CR, LF or NUL, at the point it is set:
  a value that can inject a newline is a response-splitting bug
  wherever it eventually lands.
- An uppercase header field name over HTTP/2, and any of the
  connection-specific fields there.
- A static file path that resolves outside the root, before or after
  percent-decoding, and any dotfile.

## Module Reference

| Module | What it holds |
| --- | --- |
| `http` | the facade: `get()`, `post()`, `server()`, `client()`, `serve()` |
| `http.status` | status codes, reason phrases, and predicates |
| `http.headers` | `Headers`, field validation, canonical names |
| `http.cookies` | `Cookie`, `CookieJar`, and both cookie header formats |
| `http.request` | `HttpRequest`, query string encoding and decoding |
| `http.response` | `HttpResponse` |
| `http.router` | `Router`, `Route`, `RouteMatch` |
| `http.middleware` | CORS, logging, security headers, auth, rate limiting |
| `http.files` | `StaticFiles`, byte ranges, validators |
| `http.multipart` | `MultipartBuilder`, `UploadedFile`, the parser |
| `http.negotiate` | `Accept` parsing and matching |
| `http.body` | message framing, chunked encoding, content codings |
| `http.h1` | the HTTP/1.1 wire codec |
| `http.h2` | HTTP/2: frames, HPACK, connections |
| `http.websocket` | RFC 6455, client and server |
| `http.sse` | server-sent events |
| `http.proxy` | `ReverseProxy`, `LoadBalancer` |
| `http.stream` | the buffered connection both protocols read and write through |
| `http.util` | dates, header parameters, percent coding, path normalisation |
| `http.errors` | the error hierarchy |

Every error this module raises descends from `HttpError`:
`ProtocolError` for a malformed message, `ConnectionError` for a
connection that failed, `TimeoutError`, `TooLargeError`,
`TooManyRedirectsError`, `StatusError` and `UnsupportedProtocolError`.
