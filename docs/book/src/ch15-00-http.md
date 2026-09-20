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
  - [Request Methods](#request-methods)
  - [Request Bodies](#request-bodies)
  - [Uploading Files](#uploading-files)
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
  - [Forms and File Uploads](#forms-and-file-uploads)
  - [Request Validation](#request-validation)
  - [The Response Object](#the-response-object)
  - [Middleware](#middleware)
  - [Errors](#errors)
  - [Static Files](#static-files)
  - [Compression](#compression)
  - [Streaming a Response Body](#streaming-a-response-body)
  - [Setting Cookies](#setting-cookies)
  - [Content Negotiation](#content-negotiation)
- [Built-in Middleware](#built-in-middleware)
  - [`logger()`](#logger)
  - [`request_id()`](#request_id)
  - [`security_headers()`](#security_headers)
  - [`cors()`](#cors)
  - [`basic_auth()`](#basic_auth)
  - [`bearer_auth()`](#bearer_auth)
  - [`jwt_auth()`](#jwt_auth)
  - [`rate_limit()`](#rate_limit)
  - [`etag()`](#etag)
  - [`force_https()`](#force_https)
  - [Ordering](#ordering)
- [Sessions](#sessions)
  - [Nothing Happens Until Something Uses It](#nothing-happens-until-something-uses-it)
  - [Signing In](#signing-in)
  - [Flash Messages](#flash-messages)
  - [Where Sessions Are Kept](#where-sessions-are-kept)
  - [When a Session Ends](#when-a-session-ends)
  - [The Cookie](#the-cookie)
  - [Signing the Cookie](#signing-the-cookie)
  - [What a Session May Hold](#what-a-session-may-hold)
  - [Across Workers](#across-workers)
  - [What Sessions Do Not Do](#what-sessions-do-not-do)
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
[Request Methods](#request-methods) covers each of them.

### A First Server

```zuri,ignore
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

> Blocks in this chapter that list several calls together — three ways to
> save, four filters, a family of methods — are **reference listings**, not
> programs. They show the shape of each call rather than a sequence you
> could run, and several would conflict if pasted into one file. Anything
> presented as a complete program on this page runs as written.

## A Round Trip You Can Run

Most examples in this chapter call a host that does not exist, or start a
server that never returns — neither of which you can paste into a terminal
and watch. This one you can. It runs a real server on a real port and makes
real requests against it, in two files.

The server goes in its own file, because a function that references an
imported module cannot be spawned from the file that imported it. See
[Concurrency with Isolates](ch11-00-isolates.md) for the rule.

<span class="filename">Filename: service.zu</span>

```zuri,ignore
import http

def serve(port_channel) {
  var server = http.server(0, '127.0.0.1')

  server.get('/health', @(request, response) {
    response.json({ status: 'ok' })
  })

  server.post('/echo', @(request, response) {
    response.json({ heard: request.json_body().message }, 201)
  })

  # Bind first so the port is known, hand it back, then serve.
  server.bind()
  port_channel.send(server.socket.local_address().port())
  server.listen()
}
```

<span class="filename">Filename: main.zu</span>

```zuri,ignore
import http
import isolate
import .service

var channel = isolate.channel(1)
var task = isolate.spawn(service.serve, channel)
var port = channel.recv()

var api = http.client('http://127.0.0.1:${port}')

echo api.get('/health').as_dict()
echo api.post('/echo', { message: 'hello' }).status

task.cancel()
```

```console
$ zuri run main.zu
{status: ok}
201
```

Four details in there are worth carrying into your own code.

**Port `0` asks the operating system for a free port.** `listen()` would
bind for you, but then nobody could ask which port it got, so the example
calls `bind()` explicitly, reads the bound address, and only then starts
accepting. That is the pattern for any test or any process running more
than one server.

**The port travels over a channel.** The client cannot know it in advance,
and the two halves are in different isolates with separate heaps, so a
shared variable is not an option.

**`request.json_body()` parses the request body**, and
`response.json(value, status)` writes the response. The names are not
symmetrical because they are doing different jobs: one decodes what
arrived, the other encodes and sets a status.

**`task.cancel()` ends it.** `listen()` runs until the server is closed, so
without that the program would never exit.


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

```zuri,ignore
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

```zuri,ignore
api.user_agent = 'my-service/2.1'
api.max_redirects = 3
api.set_header('X-Client-Version', '2.1')
```

> **Note**
> Build a client once and keep it. A fresh client per request throws
> away the connection pool, the cookie jar and the TLS configuration —
> which is to say, it throws away most of what a client is for.

### Request Methods

Every method has a function of its own. They exist twice over, with
identical signatures: on the module, where they go through the shared
client, and on any `HttpClient` you build yourself.

The three methods that carry a body take it as their second argument;
the rest take the options dictionary in that position.

| Call | Sends a body |
| --- | --- |
| `get(url, options)` | no |
| `post(url, data, options)` | yes |
| `put(url, data, options)` | yes |
| `patch(url, data, options)` | yes |
| `delete(url, options)` | no |
| `head(url, options)` | no |
| `options(url, options)` | no |
| `trace(url, options)` | no |
| `request(method, url, options)` | if `options` says so |

Every argument after the first is optional, so the short forms all
work:

```zuri,ignore
import http

# Reading
http.get('https://example.com/items')
http.get('https://example.com/items', { query: { page: 2 } })

# Creating and updating
http.post('https://example.com/items', { name: 'Widget' })
http.put('https://example.com/items/1', { name: 'Widget', price: 9 })
http.patch('https://example.com/items/1', { price: 11 })

# Removing
http.delete('https://example.com/items/1')

# Asking about a resource without fetching it
var probe = http.head('https://example.com/large.iso')
echo probe.headers.get('content-length')

# Asking what a resource supports
echo http.options('https://example.com/items').headers.get('allow')
```

The same calls on a client of your own, which is what you want for
anything that runs more than once:

```zuri,ignore
var api = http.client('https://api.example.com')

api.get('/items')
api.post('/items', { name: 'Widget' })
api.delete('/items/1')
```

`request()` takes the method as an argument, which is how you send one
that has no function of its own — a WebDAV `PROPFIND`, or anything
else a service has invented:

```zuri,ignore
api.request('PROPFIND', '/files/', {
  headers: { 'Depth': '1' },
  body: '<propfind xmlns="DAV:"><allprop/></propfind>',
  content_type: 'application/xml',
})
```

Method names are case-sensitive on the wire, and every registered one
is uppercase; a lowercase name is uppercased for you.

> **Note**
> `head()` does not follow redirects by default, and `delete()`,
> `options()` and `trace()` send no body. A `DELETE` that genuinely
> needs one — some APIs want a reason in the body — can use
> `request('DELETE', url, { json: ... })`.

### Request Bodies

The second argument to `post()`, `put()` and `patch()` is the body,
and what it is decides how it is sent:

```zuri,ignore
api.post('/items', { name: 'Widget', price: 9 })   # JSON
api.post('/items', ['a', 'b'])                     # JSON
api.post('/items', 'raw text')                     # sent as-is
api.post('/items', file('photo.jpg', 'rb').read()) # sent as-is
api.post('/items', form_builder)                   # multipart
```

A dictionary or list becomes JSON with a matching `Content-Type`; a
string or bytes is sent exactly as given, with no content type unless
you name one.

For anything else, or to be explicit, name it in the options
dictionary:

| Option | Sends | Content-Type |
| --- | --- | --- |
| `body` | a string or bytes, exactly as given | none, unless `content_type` is set |
| `json` | any value, JSON-encoded | `application/json` |
| `form` | a dictionary | `application/x-www-form-urlencoded` |
| `multipart` | a `MultipartBuilder` | `multipart/form-data`, boundary included |
| `content_type` | — | overrides whichever of the above applied |

```zuri,ignore
# A login form
api.post('/login', nil, {
  form: { username: 'ada', password: secret },
})

# An explicit content type over a raw body
api.put('/documents/1', nil, {
  body: markdown_source,
  content_type: 'text/markdown; charset=utf-8',
})

# A form whose field repeats
api.post('/search', nil, {
  form: { tag: ['new', 'featured'], q: 'zuri' },
})
```

`Content-Length` is set for you from the body, and cannot be
overridden — two parties disagreeing about how long a body is, is a
request-smuggling bug rather than a formatting choice.

### Uploading Files

A file upload is a `multipart/form-data` body, which
`MultipartBuilder` assembles:

```zuri,ignore
import http

var form = http.MultipartBuilder()

form.add_field('title', 'Holiday')
form.add_field('album', '2026')
form.add_file('photo', 'beach.jpg', file('beach.jpg', 'rb').read(), 'image/jpeg')

var response = http.post('https://example.com/photos', form)
```

Passing the builder as the body is enough — the `Content-Type`, the
boundary parameter and the `Content-Length` all follow from it.

| Method | Does |
| --- | --- |
| `add_field(name, value)` | adds a plain form field; numbers and booleans are stringified |
| `add_file(name, filename, content, content_type)` | adds a file part; `content` is bytes or a string, `content_type` defaults to `application/octet-stream` |
| `content_type()` | the `Content-Type` header value, boundary included |
| `boundary()` | the boundary string |
| `build()` | the assembled body, as bytes |
| `length()` | how many parts have been added |

Several files under one field name is just several calls — that is
what an `<input type="file" multiple>` sends:

```zuri,ignore
for path in paths {
  form.add_file('attachments', os.base_name(path), file(path, 'rb').read())
}
```

A filename that is not plain ASCII is sent twice: once as a
transliterated `filename`, and once as an RFC 5987 `filename*`, which
is what lets the real name survive a recipient that only understands
one of the two.

If you need the body separately — to sign it, to log its size, to send
it somewhere this client is not going — build it yourself:

```zuri,ignore
var body = form.build()

api.post('/photos', nil, {
  body,
  content_type: form.content_type(),
})
```

> **Note**
> `build()` assembles the whole body in memory. For an upload large
> enough that this matters, send the file as a raw body with its own
> content type instead — `multipart/form-data` only earns its overhead
> when there are fields alongside the file.

The boundary is generated from the platform's secure random source,
not from a timestamp or a counter — a boundary a peer can predict is a
boundary a peer can write into a field value to forge extra parts.

### Reading a Response

```zuri,ignore
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

```zuri,ignore
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

```zuri,ignore
api.get('/search', {
  query: { q: 'zuri', page: 2, tag: ['new', 'featured'] },
  headers: { 'Accept-Language': 'en-GB' },
})
```

A list value repeats the parameter, which is how a query string
carries more than one value under one name. Per-request headers are
merged over the client's own.

### Authentication

```zuri,ignore
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

```zuri,ignore
http.get(url, { follow_redirects: false })
```

`head()` does not follow redirects by default, since the point of a
HEAD is usually to inspect the very response a redirect would hide.

### Cookies

A client with a cookie jar carries cookies between requests, so a
login and the requests after it behave the way a browser would:

```zuri,ignore
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

```zuri,ignore
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

```zuri,ignore
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

```zuri,ignore
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

```zuri,ignore
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
`options()` register for one method. `any()` registers `GET`, `POST`,
`PUT`, `PATCH`, `DELETE` and `OPTIONS` at once. `handle()` takes the
method as an argument, which is how a method with no function of its
own — `PROPFIND`, or anything a service has invented — gets a route:

```zuri,ignore
server.handle('PROPFIND', '/files/' + '*path', list_properties)
```

Three things are answered without a handler:

- `HEAD` falls back to the `GET` route for the same path, and the body
  is dropped on the way out.
- `OPTIONS` answers `204` with an `Allow` header listing what the path
  actually accepts.
- A path that exists for other methods answers `405`, again with
  `Allow` — rather than a `404`, which would be a lie.

Name a route to build URLs from it later:

```zuri,ignore
server.get('/users/:id', show_user, 'user.show')

server.routes().url_for('user.show', { id: 42 })    # '/users/42'
```

### The Request Object

```zuri,ignore
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
  request.bearer_token()              # the token from an Authorization header

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

```zuri,ignore
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

### Forms and File Uploads

A submitted form arrives as a request body, in one of two encodings,
and both are read through the same methods. Which one a browser sends
is decided by the `enctype` on the `<form>`: the default
`application/x-www-form-urlencoded` for a form of plain fields, and
`multipart/form-data` for one that carries a file.

```zuri,ignore
server.post('/signup', @(request, response) {
  var email = request.form_field('email', '')
  var password = request.form_field('password', '')

  if email.is_empty() {
    response.json({ error: 'email is required' }, 422)
    return
  }

  create_account(email, password)
  response.redirect('/welcome', 303)
})
```

| Method | Returns |
| --- | --- |
| `form()` | every field as `name -> value`, keeping the first of a repeated name |
| `form_all()` | every field as `name -> [value, ...]` |
| `form_field(name, fallback)` | one field's first value |
| `files()` | uploaded files as `name -> [UploadedFile, ...]` |
| `file(name)` | the first file uploaded under `name`, or `nil` |

The body is parsed on first use and cached, so reading `form()` in a
middleware and again in the handler costs one parse. A body that is
neither encoding gives an empty dictionary rather than raising — use
`request.json_body()` for a JSON body and `request.text()` for
anything else.

A field a form can repeat — a set of checkboxes, a multi-select —
needs `form_all()`, since `form()` keeps only the first value:

```zuri,ignore
var tags = request.form_all().get('tags', [])
```

> **Note**
> A redirect after a successful `POST` should be `303 See Other`, as
> above. It is what stops a browser re-submitting the form when the
> user reloads the resulting page.

#### Receiving an upload

```html
<form method="post" action="/avatar" enctype="multipart/form-data">
  <input type="text" name="caption">
  <input type="file" name="avatar">
  <button>Upload</button>
</form>
```

```zuri,ignore
import os

server.post('/avatar', @(request, response) {
  var upload = request.file('avatar')

  if upload == nil {
    response.json({ error: 'no file was submitted' }, 400)
    return
  }

  if upload.size() > 2 * 1024 * 1024 {
    response.json({ error: 'the image must be under 2 MB' }, 413)
    return
  }

  var destination = os.join_paths('./uploads', upload.safe_name())
  upload.save_to(destination)

  response.json({
    saved: upload.safe_name(),
    caption: request.form_field('caption', ''),
    bytes: upload.size(),
  })
})
```

An `UploadedFile` carries:

| Member | Is |
| --- | --- |
| `filename` | the name the client claimed, exactly as sent |
| `safe_name()` | that name reduced to one path-safe segment |
| `content_type` | the type the client declared, or `application/octet-stream` |
| `content` | the bytes |
| `size()` | how many of them |
| `name` | the form field it arrived under |
| `headers` | every header on that part of the body |
| `save_to(path)` | writes the content, and returns the byte count |
| `to_text()` | the content decoded as UTF-8 |

An `<input type="file" multiple>` sends several parts under one name,
which is what `files()` returns a list for:

```zuri,ignore
for upload in request.files().get('attachments', []) {
  upload.save_to(os.join_paths('./uploads', upload.safe_name()))
}
```

#### Two things a handler must not trust

**The filename.** It is attacker-controlled, and it routinely contains
a full local path from a Windows client, `..` segments from a hostile
one, or a NUL byte meant to truncate a later check. Never join it to a
path directly:

```zuri,ignore
os.join_paths('./uploads', upload.filename)     # no
os.join_paths('./uploads', upload.safe_name())  # yes
```

`safe_name()` drops every directory component — both separators — and
reduces the rest to letters, digits, `.`, `-` and `_`, returning
`'unnamed'` when nothing usable is left. Better still, name the file
yourself and keep the client's name as a label:

```zuri,ignore
var stored = '${uuid.v4()}.jpg'
upload.save_to(os.join_paths('./uploads', stored))
record_upload(stored, upload.filename)
```

**The content type.** `content_type` is whatever the client wrote in
the part header and says nothing about what the bytes are. A file
claiming `image/png` may be anything at all. When the difference
matters, look at the bytes — `mime.detect_from_header()` reads a real
file's leading bytes, so sniffing an upload means writing it somewhere
first:

```zuri,ignore
import mime
import os

var staged = os.join_paths(os.temp_dir(), uuid.v4())
upload.save_to(staged)

var actual = mime.detect_from_header(file(staged, 'rb'))

if actual != 'image/png' and actual != 'image/jpeg' {
  file(staged).delete()
  response.json({ error: 'that is not an image' }, 415)
  return
}

os.rename(staged, os.join_paths('./uploads', stored_name))
```

#### Size limits

The body is read into memory, bounded by the server's
`max_body_size` — 10 MiB by default, which is deliberately small:

```zuri,ignore
server.max_body_size = 50 * 1024 * 1024   # accept uploads up to 50 MB
```

A request that *announces* a body over the limit is refused with `413
Content Too Large` before a byte of it is read, which is the whole
point of `Content-Length`. One that lies about its length is cut off
at the limit and also refused. Either way the connection then closes,
since a body that was only partly read leaves nothing safe to parse
after it.

A client asking permission first with `Expect: 100-continue` gets its
answer before it sends anything — the server checks the announced
length against the limit and either says `100 Continue` or refuses
outright, so a rejected upload costs one round trip rather than a
whole transfer.

For an upload too large to want in memory at all, take it as a raw
body and stream it rather than as a form field. `multipart/form-data`
earns its overhead only when there are fields alongside the file.

### Request Validation

A request validates itself against a `validate` schema:

```zuri,ignore
import http
import validate

var create_user = validate.schema({
  name:  validate.required().string().max_length(100),
  email: validate.required().string().email(),
  age:   validate.required().integer().gte(18).lte(120),
})

server.post('/users', @(request, response) {
  catch {
    var data = request.validate(create_user)
    response.json(create_account(data), 201)
  } as error {
    response.json({ errors: create_user.group_errors(error.errors) }, 422)
  }
})
```

`validate()` returns the input it checked, so the happy path is one
line and the data you go on to use is the data that was validated.
Failure raises the schema's own `validate.ValidationError`, carrying
an `errors` list of `{ field, message }`; `group_errors()` turns that
into a dictionary keyed by field, which is the shape most front ends
want:

```json
{
  "errors": {
    "email": ["The email field must be a valid email address."],
    "age": ["The age field must be greater than or equal to 18."]
  }
}
```

To branch rather than catch, validate the input yourself — there is
no separate API for it:

```zuri,ignore
var result = create_user.check(request.input())

if !result.valid {
  response.json({ errors: result.errors }, 422)
  return
}
```

The rules themselves — and there are around eighty of them, including
cross-field ones like `confirmed()` and `required_if()` — belong to
the `validate` module rather than to this one.

#### What gets validated

`request.input()` is the dictionary `validate()` checks. Three sources
are merged, each overriding the one before it:

1. route parameters, from the pattern that matched
2. query string parameters
3. the body — a JSON object's keys, or the submitted form fields

So one schema covers `POST /users` with a JSON body, `GET /users?…`
with a query string, and `/users/:id` with a route parameter, without
the handler caring which arrived.

Take one source on its own by naming it:

```zuri,ignore
request.validate(schema, 'body')     # only the body
request.validate(schema, 'query')    # only the query string
request.validate(schema, 'params')   # only the route parameters
```

Two things are deliberately left out of the merge:

- **Uploaded files.** Nothing a schema can say about a file is
  expressible as a rule over its bytes; reach them with
  `request.file()` and check them as [Forms and File
  Uploads](#forms-and-file-uploads) describes.
- **A JSON body that is not an object.** An array or a bare string has
  no names to merge, so it contributes nothing; read it with
  `request.json_body()`.

A body that fails to parse as JSON also contributes nothing rather
than raising, which leaves the schema's own `required` rules to report
what is missing. That is a better answer to a client than a parser
message.

#### Values from the wire are strings

A query string and a urlencoded form carry text and nothing else.
`?age=36` is the string `'36'`, not the number `36`, and that changes
which rules hold:

| Rule | On `'36'` | Because |
| --- | --- | --- |
| `integer()`, `numeric()`, `gt()`, `gte()`, `lt()`, `lte()` | reads it as 36 | these coerce |
| `size()`, `min()`, `max()`, `between()` | reads it as 2 | these measure *size*, which for a string is its character count |

That is `validate`'s documented behaviour, not an accident of this
module: `min(8)` on a password means eight characters. It only
surprises when a schema written against a JSON body is later pointed
at a query string.

Write a schema that has to serve both with the value rules:

```zuri,ignore
age: validate.required().integer().gte(18).lte(120)   # both
age: validate.required().integer().between(18, 120)   # JSON bodies only
```

`input()` does not coerce anything on your behalf. It would have to
guess, and a postcode of `'01234'` or a version of `'1.0'` silently
becoming a number is worse than the rule you have to pick deliberately.

#### Repeated fields

A name may legally repeat in a query string or a form, so those
sources arrive as `name -> [values]`. `input()` flattens a name
carrying exactly one value to that value, and leaves a name carrying
several as a list:

```
?tag=a           ->  { tag: 'a' }
?tag=a&tag=b     ->  { tag: ['a', 'b'] }
```

That is what lets a scalar rule see a scalar. A field that must
*always* be a list, however many values arrived, is better read
through `form_all()` or `request.query` directly and validated with
`validate`'s `.*` wildcard.

> **Note**
> The `http` module does not import `validate`. `validate()` takes any
> object with a `check_or_raise()` method and calls it, so a server
> that validates nothing never pays to load a schema engine — and a
> schema of your own, or from somewhere else, works just as well.

### The Response Object

```zuri,ignore
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

```zuri,ignore
server.use(@(request, response, next) {
  var started = time()
  next()
  echo '${request.method} ${request.path} ${response.status} ' +
    '${(time() - started) * 1000}ms'
})
```

Not calling `next()` is how a middleware short-circuits, which is
exactly what an authentication or rate-limiting layer wants:

```zuri,ignore
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

Anything a middleware wants to hand to the handler goes on
`request.context`, which is a plain dictionary that exists for exactly
that:

```zuri,ignore
server.use(@(request, response, next) {
  request.context['started_at'] = time()
  next()
})
```

A middleware that wants to act on the *response* calls `next()` first
and then works on what came back:

```zuri,ignore
server.use(@(request, response, next) {
  next()
  response.header('X-Served-By', hostname)
})
```

The ones most services need are already written — see
[Built-in Middleware](#built-in-middleware).

### Errors

A handler that raises becomes a `500`, and the exception message never
reaches the client — an exception message routinely carries a file
path, a query, or a fragment of the data being processed, and none of
that belongs in a reply to whoever triggered it.

```zuri,ignore
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

```zuri,ignore
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

```zuri,ignore
server.serve_files('/', './dist', { fallback: 'index.html' })
```

### Compression

Responses are compressed on the way out, and it is on by default. A
response is compressed when all of the following hold:

- `server.compression` is on (it is),
- the body is at least `server.compression_min_size` bytes (1024),
- its media type is one that benefits — anything `text/`, plus JSON,
  JavaScript, XML, SVG, WebAssembly and the `+json`/`+xml` types,
- the client's `Accept-Encoding` accepts `br` or `gzip`,
- nothing already set a `Content-Encoding`,
- and the compressed body actually came out smaller.

Brotli is preferred over gzip when the client accepts both. `Vary:
Accept-Encoding` is added either way, so a shared cache in front
cannot serve a compressed body to a client that asked for none.

Turning it off, or moving the threshold:

```zuri,ignore
server.compression = false          # off entirely
server.compression_min_size = 4096  # only bodies over 4 KiB
```

The list is an allowlist rather than a denylist on purpose. A JPEG, an
MP4 or a zip is already compressed; running it through brotli spends
CPU to make it slightly larger.

> **Note**
> Responses built with `response.file()` — which includes everything
> `serve_files()` serves — are **not** compressed on the fly. They are
> streamed from disk rather than held in memory, and compressing them
> per request would mean reading them into memory to do it. Compress
> those ahead of time and let the static handler pick the compressed
> file up:
>
> ```zuri
> server.serve_files('/static', './public', { precompressed: true })
> ```
>
> With that on, a request for `site.css` from a client that accepts
> brotli is answered with `site.css.br` if it exists, and with
> `site.css.gz` if that exists and gzip is accepted — at no CPU cost
> per request, and with the correct `Content-Encoding` and `Vary`.

On the client side there is nothing to configure: `Accept-Encoding:
gzip, br, deflate, zstd` is sent by default and a compressed response
body is decoded before you see it.

### Streaming a Response Body

A response body can come from a callback instead of memory:

```zuri,ignore
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

### Setting Cookies

```zuri,ignore
response.set_cookie('theme', 'dark', {
  max_age: 86400,
  secure: true,
  same_site: 'Strict',
})

response.clear_cookie('theme')
```

`http_only` defaults to `true` and `same_site` to `'Lax'`, which are
what a cookie carrying anything sensitive should have; pass them
explicitly to opt out. A cookie whose name carries the `__Secure-` or
`__Host-` prefix has that prefix's rules applied for it, rather than
being sent in a form the browser will silently refuse to store.

For state that belongs to a visitor rather than to the browser, use a
session, which keeps the state on the server and puts only an
identifier in the cookie. [Sessions](#sessions) covers it.

### Content Negotiation

```zuri,ignore
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

## Built-in Middleware

`http.middleware` holds the middleware nearly every public service
ends up wanting. Each function *returns* a middleware, so it is called
at the point you register it:

```zuri
import http
import http.middleware

var server = http.server(3000)

server.use(middleware.request_id())
server.use(middleware.logger())
server.use(middleware.security_headers())
```

They are ordinary middleware with no special standing — read any of
them as a worked example of writing your own.

None of them raises when it is built. That matters for the three that
take a verifier — [`basic_auth()`](#basic_auth),
[`bearer_auth()`](#bearer_auth) and [`jwt_auth()`](#jwt_auth) — where
**no verifier means the middleware is disabled**: it passes every
request straight through rather than refusing one, and rather than
failing at registration.

```zuri
# Authentication, off. Everything else in the chain is untouched.
server.use(middleware.jwt_auth(nil))
```

That is the right shape for a middleware, and it is deliberately
useful: blanking a verifier switches its middleware off without
unpicking the chain around it, which is what you want when bisecting a
request that is failing somewhere in a stack of them.

Disabling is opt-in and never a fallback: a verifier that is *present*
and refuses a request still refuses it.

### `logger()`

Writes one line per request, after the response is finished.

```zuri
server.use(middleware.logger())
```

```
127.0.0.1 - [Wed, 09 Sep 2026 20:09:17 GMT] "GET /static/big.txt HTTP/1.1" 200 320 3.07ms
```

That is the Common Log Format with the response time added, which
every log analyser already understands. The size comes from
`Content-Length` when the body is file-backed or streamed, so a static
file logs its real size rather than zero.

| Option | Default | Meaning |
| --- | --- | --- |
| `sink` | `echo` | a function taking the finished line |
| `format` | the format above | a function `(request, response, milliseconds)` returning the line |
| `trust_proxy` | `false` | log the forwarded client address rather than the peer |

```zuri
server.use(middleware.logger({
  sink: @(line) { log_file.puts(line + '\n') },
  trust_proxy: true,
}))
```

Structured logging is a `format` that returns JSON:

```zuri
server.use(middleware.logger({
  format: @(request, response, elapsed) {
    return json.encode({
      method: request.method,
      path: request.path,
      status: response.status,
      ms: elapsed,
      id: request.context.get('request_id', nil),
    })
  },
}))
```

A request that raises is still logged, with the `500` its handler
produced, and the failure then carries on to whatever handles it —
swallowing it here would turn every error into a silent success.

### `request_id()`

Gives every request an identifier, so one request can be followed
across a log file and across services.

```zuri
server.use(middleware.request_id())
```

The identifier lands on `request.context['request_id']` and in the
response's `X-Request-Id`. One supplied by the client is echoed rather
than replaced, which is what makes a trace continue across a hop.

The single optional argument is the header name:

```zuri
server.use(middleware.request_id('X-Correlation-Id'))
```

An identifier that came from outside is written into your logs, so it
is capped at 64 characters and stripped of anything that is not
plainly printable before it is trusted that far.

### `security_headers()`

Adds the response headers a browser acts on to harden a page.

```zuri
server.use(middleware.security_headers())
```

| Option | Default | Header |
| --- | --- | --- |
| `content_type_options` | `'nosniff'` | `X-Content-Type-Options` — stops the browser second-guessing a declared content type |
| `frame_options` | `'DENY'` | `X-Frame-Options` — refuses to be framed, which is what clickjacking needs |
| `referrer_policy` | `'strict-origin-when-cross-origin'` | `Referrer-Policy` — keeps paths and queries out of outbound referrers |
| `hsts_max_age` | `31536000` | `Strict-Transport-Security` |
| `hsts_subdomains` | `false` | adds `includeSubDomains` |
| `content_security_policy` | not set | `Content-Security-Policy` |

```zuri
server.use(middleware.security_headers({
  content_security_policy: "default-src 'self'; img-src 'self' data:",
  hsts_subdomains: true,
  frame_options: 'SAMEORIGIN',
}))
```

Pass `nil` for any of them to leave that header off entirely.

`Content-Security-Policy` has no default because a wrong one breaks
the page and a permissive one is theatre; it depends on what your
application actually loads.

`Strict-Transport-Security` is only sent over HTTPS. Over cleartext it
is at best ignored, and at worst a way to lock a client out of a host
that has no certificate.

Every header is set with `set_default`, so a handler that set its own
keeps it.

### `cors()`

Answers CORS preflights and adds the headers a browser needs before it
will let script read a response from another origin.

```zuri
server.use(middleware.cors({
  origins: ['https://app.example.com'],
  credentials: true,
}))
```

| Option | Default | Meaning |
| --- | --- | --- |
| `origins` | `['*']` | origins allowed, matched exactly; `'*'` allows any |
| `methods` | the seven usual ones | what a preflight is told is allowed |
| `headers` | reflect what was asked | request headers script may set |
| `expose` | none | response headers script may read |
| `credentials` | `false` | whether cookies and `Authorization` may be sent |
| `max_age` | `86400` | how long a preflight may be cached, in seconds |

A request with no `Origin` is not a cross-origin request and passes
straight through. An `Origin` that is not on the list gets no CORS
headers at all — the browser then refuses to hand the response to
script, which is the correct outcome; answering with an error instead
would leak whether the resource exists.

> **Note**
> A wildcard and `credentials` cannot be combined — every browser
> rejects that pairing. When `credentials` is set, the requesting
> origin is echoed back instead, which makes the `origins` list the
> only thing standing between an attacker's page and an authenticated
> response. Name the origins explicitly whenever credentials are in
> play.

`Vary: Origin` is added whenever the answer depends on the request's
origin, so a shared cache cannot serve one origin's response to
another.

### `basic_auth()`

Requires HTTP Basic authentication.

```zuri
server.use(middleware.basic_auth(@(user, password) {
  return user == 'admin' and http.util.secure_equals(password, secret)
}, 'Admin area'))
```

The first argument is called with the username and password and
returns whether they are acceptable. The second is the realm named in
the challenge, and defaults to `'Restricted'`.

On success the username is put on `request.context['user']`. On
failure the response is `401` with a `WWW-Authenticate` header, which
is what makes a browser show its credentials prompt.

Passing `nil` in place of the verifier disables the middleware
entirely — see [the note above](#built-in-middleware).

Compare secrets with `http.util.secure_equals()` rather than `==`: it
compares in constant time, so a wrong guess and a nearly-right one
take the same time and the comparison does not hand over the secret
one character at a time.

To guard part of a site rather than all of it, wrap it:

```zuri,ignore
var guard = middleware.basic_auth(check_credentials)

server.use(@(request, response, next) {
  if request.path.starts_with('/admin') {
    guard(request, response, next)
    return
  }
  next()
})
```

Note that the `next()` a guard calls on success continues the *whole*
remaining chain, not just the handler. Two authentication middleware
registered globally therefore both apply to every request; scope each
of them by prefix, as above, when they are meant for different parts
of the site.

### `bearer_auth()`

Requires a bearer token — the usual shape for an API.

```zuri
server.use(middleware.bearer_auth(@(token) {
  var claims = verify_jwt(token)
  return claims == nil ? false : claims.subject
}))
```

The verifier is called with the token and returns either something
falsy to refuse it, or a value to attach — that value lands on
`request.context['user']`, so returning the subject, the claims, or a
whole user record all work.

Failure is `401` with a `WWW-Authenticate: Bearer` challenge and a
JSON body. The realm is the optional second argument and defaults to
`'api'`. A `nil` verifier disables the middleware, as with
`basic_auth()`.

`middleware.parse_bearer(header)` and `middleware.parse_basic(header)`
are exported separately for code that needs to read an `Authorization`
header without installing a middleware at all, and
`request.bearer_token()` reads the token straight off a request.

For a JSON Web Token specifically, [`jwt_auth()`](#jwt_auth) does the
verification, the claim handling and the RFC 6750 challenges rather
than leaving them to the verifier you write here.

### `jwt_auth()`

Requires a valid JSON Web Token, verified by the `jwt` module.

```zuri,ignore
import http.middleware
import jwt

server.use(middleware.jwt_auth(
  jwt.Verifier(secret, { algorithms: ['HS256'], audience: 'api' })
))
```

The first argument is either a `jwt.Verifier` or any function taking
the token and returning its claims. The function form is what covers a
key set resolved by `kid`, or anything else the `jwt` module can do
that a fixed verifier cannot:

```zuri
server.use(middleware.jwt_auth(@(token) {
  return jwt.verify_with_jwks(token, keys, { audience: 'api' })
}))
```

| Option | Default | Meaning |
| --- | --- | --- |
| `realm` | `'api'` | the realm named in the challenge |
| `optional` | `false` | attach the claims when a valid token is present, but do not refuse a request without one |
| `scopes` | none | a list of scopes every token must carry |

On success the claims land on `request.context['claims']`, and the
`sub` claim — the usual place an issuer puts the account a token
speaks for — on `request.context['user']`:

```zuri
server.get('/me', @(request, response) {
  response.json({
    account: request.context.get('user', nil),
    issued_at: request.context.get('claims', {}).get('iat', nil),
  })
})
```

Failures follow RFC 6750 §3:

| Situation | Answer |
| --- | --- |
| no token at all | `401`, `WWW-Authenticate: Bearer realm="api"` |
| the token does not verify | `401`, with `error="invalid_token"` |
| valid, but missing a required scope | `403`, with `error="insufficient_scope"` and the scope that was needed |

The distinction in that last row is the point of `scopes`: a token
that failed to authenticate and a token that authenticated but is not
allowed to do this are different problems, and answering both with
`401` tells a client to go and get a new token when a new token will
not help.

> **Note**
> A challenge never says *which* check a token failed. An expired
> token and a forged one get exactly the same
> `error_description`, because which one it was is useful to your
> logs and useful to an attacker, and to nobody else. If you need the
> reason, log it from the verifier you passed in.

`optional` is for a route that behaves differently when it knows who
is asking without requiring it — a public page that shows an edit
button to its author:

```zuri,ignore
server.use(middleware.jwt_auth(verifier, { optional: true }))

server.get('/posts/:id', @(request, response) {
  var viewer = request.context.get('user', nil)
  response.json(render_post(request.param('id'), viewer))
})
```

A token that is *present* but invalid is still refused under
`optional`. Ignoring a bad token would let a client tamper with one
and get the anonymous view rather than an error, which hides exactly
the problem worth surfacing.

A `nil` verifier disables the middleware: every request passes
through unauthenticated, and `request.context['claims']` is simply
never set. Nothing here raises when it is built, so registering it
never needs a `catch` around it.

```zuri,ignore
var verifier = production ? jwt.Verifier(secret, options) : nil

server.use(middleware.jwt_auth(verifier))
```

Like `HttpRequest.validate()`, this does not import the `jwt` module —
the verifier is built by the caller, which keeps the token format the
application's business and means a server that authenticates nothing
never pays to load it.

### `rate_limit()`

Limits how many requests one client may make in a window of time.

```zuri
server.use(middleware.rate_limit({ limit: 100, window: 60 }))
```

| Option | Default | Meaning |
| --- | --- | --- |
| `limit` | `60` | requests allowed per window |
| `window` | `60` | the window, in seconds |
| `key` | the client address | a function of the request returning the bucket key |
| `trust_proxy` | `false` | derive the address from forwarding headers |

Every response carries the current state, so a well-behaved client can
back off before it is refused:

```
RateLimit-Limit: 100
RateLimit-Remaining: 87
RateLimit-Reset: 41
```

Over the limit is `429 Too Many Requests` with a `Retry-After`.

Rate-limit per API key rather than per address by supplying a `key`:

```zuri
server.use(middleware.rate_limit({
  limit: 1000,
  window: 3600,
  key: @(request) {
    return request.header('x-api-key', nil) or request.client_ip() or 'anonymous'
  },
}))
```

> **Note**
> The counter lives in memory, so it is per worker: running with
> `workers: 4` makes the effective limit four times `limit`. That is
> a deliberate trade — a shared counter would need shared state, and
> this is meant to blunt a runaway client rather than to meter
> billing. Divide `limit` by the worker count if the exact number
> matters, or keep the count somewhere both workers can see.

### `etag()`

Computes a weak `ETag` over a finished response body and answers `304
Not Modified` when the client already has that version.

```zuri
server.use(middleware.etag())
```

The single optional argument is the smallest body worth tagging, in
bytes; it defaults to `128`, below which the validator costs more than
the body it would save.

A handler that already set its own `ETag` is left alone — it knows
something about the resource that hashing the bytes does not — and so
are file-backed responses, which `serve_files()` already tags from the
file's size and modification time.

Because it works on the finished body, it pairs with anything: a
rendered template, a JSON document, a generated report.

### `force_https()`

Redirects every request that arrived over cleartext to the same URL
over HTTPS.

```zuri
server.use(middleware.force_https())
```

| Option | Default | Meaning |
| --- | --- | --- |
| `status` | `308` | the redirect status; `308` preserves the method and body |
| `port` | the default | the HTTPS port, when it is not 443 |

This belongs on the cleartext listener, which usually exists only to
perform this redirect:

```zuri
# port 80: redirect and nothing else
var redirector = http.server(80, '0.0.0.0')
redirector.use(middleware.force_https())
```

Pair it with `security_headers()`'s `Strict-Transport-Security` on the
TLS listener, so that after the first visit the browser stops making
the cleartext request at all.

### Ordering

Middleware run outermost first, so the order they are registered in is
the order they wrap the request. A workable default:

```zuri,ignore
server.use(middleware.request_id())        # so everything after can log it
server.use(middleware.logger())            # so it sees the final status
server.use(middleware.force_https())       # before any work is done
server.use(middleware.security_headers())
server.use(middleware.cors({ origins: allowed }))
server.use(middleware.rate_limit({ limit: 100 }))
server.use(middleware.jwt_auth(verifier))   # after the cheap refusals
server.use(middleware.etag())              # innermost: it needs the finished body
```

The reasoning behind each position: identifiers before anything that
might log, logging outside everything so it records what actually
happened, cheap refusals before expensive ones, and anything that
inspects the response body innermost, where the body exists.

## Sessions

A session is state that belongs to one visitor, kept on the server and
found again by a cookie the browser sends back. Only the identifier
travels, so a visitor can neither read what the session holds nor
change it, and the cookie is worth nothing to anyone who cannot
present the exact value that was issued.

```zuri,ignore
import http

var server = http.server(3000)

server.use(http.session.session())

server.get('/', @(request, response) {
  var seen = request.session().get('seen', 0) + 1

  request.session().set('seen', seen)
  response.text('visit ${seen}')
})

server.listen()
```

`session()` is middleware. Register it once, above anything that reads
a session, and every handler below it reaches its own through
`request.session()`.

### Nothing Happens Until Something Uses It

A request that never touches its session costs nothing: no read from
the store, no write, and no `Set-Cookie`. The record is created the
first time something is written to the session, which is what keeps a
crawler working through a public site from filling the store with
empty sessions.

A request that only reads an existing session writes nothing back
either, beyond moving the idle expiry along at most once every
`touch_interval` seconds.

### Signing In

```zuri,ignore
server.post('/login', @(request, response) {
  var account = authenticate(request.form())

  if account == nil {
    response.status = 401
    response.html(render_login('Those details do not match.'))

    return
  }

  request.session().regenerate()
  request.session().set('account', account.id)

  response.redirect('/')
})
```

`regenerate()` is the line to get right. It gives the session a new
identifier and destroys the record the old one named, keeping
everything the session holds.

Without it, an attacker who can set a cookie in the victim's browser
beforehand — through a stray subdomain, an open redirect, a shared
machine — knows the identifier the victim will be signed in under, and
can simply use it afterwards. That is session fixation, and a new
identifier is the whole of the defence. Call it whenever what the
session means changes: signing in, elevating to administrator, a
step-up authentication.

Signing out is `destroy()`, which removes the record and has the
response expire the cookie:

```zuri,ignore
server.post('/logout', @(request, response) {
  request.session().destroy()
  response.redirect('/')
})
```

`clear()` is the other one: it empties the session without ending it,
keeping the identifier and the cookie.

### Flash Messages

A handler that does the work and redirects cannot render the message
saying what happened. A flash carries it to the page that can:

```zuri,ignore
server.post('/posts', @(request, response) {
  create_post(request.form())

  request.session().flash('notice', 'Your post is up.')
  response.redirect('/posts')
})

server.get('/posts', @(request, response) {
  response.html(render(posts(), request.session().take_flash('notice')))
})
```

A flash is spent by the next request that touches the session at all,
whether or not that request asks for this one. A page that looked at
the session and did not read the message does not leave it for the
page after; a request that never touched its session — a static file,
an image — leaves it waiting.

### Where Sessions Are Kept

Four things can hold a session, and swapping between them changes one
line.

**`FileStore`, one file per session.** This is the default, because it
needs no setup and is shared between the workers `http.serve()`
starts. Given no directory it uses a private subdirectory of the
platform's temporary directory, created `0700`:

```zuri,ignore
server.use(http.session.session())
```

That directory is cleared on whatever schedule the platform keeps, and
on most of them at every reboot, so name your own for anything that
has to outlive the host:

```zuri,ignore
server.use(http.session.session({
  store: http.session.FileStore('/var/lib/app/sessions'),
}))
```

A session file is a bearer credential in the same way the cookie is.
The directory is created `0700` and each file `0600`, and a directory
that every user on the machine can reach is refused rather than used —
which is why the temporary directory itself is never the default.
Pass `strict_permissions: false` to accept one anyway.

**`SqlStore`, one row per session.** It lives in its own import, so a
program using the default store never loads the `sql` module:

```zuri,ignore
import http.session.sql { SqlStore }
import sql

var store = SqlStore(sql.pool('postgres://localhost/app'))
store.migrate()

server.use(http.session.session({ store }))
```

`migrate()` creates the table and its index if they are not there, and
is safe to call on every start. It takes a `sql.Connection` or a
`sql.Pool`; a server wants the pool.

**`MemoryStore`, for a test.** Nothing survives a restart, and nothing
is shared between workers, so a browser whose next request lands on a
different worker arrives with a session that worker has never heard
of. It is the right store for a test and the wrong one for traffic.

**Something of your own.** A store is five methods, none of which sees
an identifier or understands a payload:

```zuri,ignore
class RedisStore < http.session.SessionStore {
  @new(client) {
    self._client = client
  }

  read(key) {
    return self._client.get('session:' + key)
  }

  write(key, payload, expires_at) {
    self._client.set_with_ttl('session:' + key, payload, (expires_at - time()).ceil())
  }

  destroy(key) {
    self._client.remove('session:' + key)
  }

  gc(now) {
    # Redis expires keys itself.
    return 0
  }
}
```

`touch()` has a working default built on `read()` and `write()`;
override it where the backend can move an expiry on its own.

The key a store is handed is the SHA-256 of the identifier, not the
identifier. Someone who reads the directory, the table, or a backup of
either learns what is in the sessions but cannot resume one, because
the value the browser presents is the preimage.

### When a Session Ends

Two clocks, and a session ends at whichever runs out first:

| Option | Default | |
| --- | --- | --- |
| `idle_timeout` | `7200` | seconds of inactivity; rolls forward while the visitor is active |
| `lifetime` | `86400` | seconds the session may live however active it is |

Either may be `nil` to remove that limit, but not both.

The absolute lifetime is what asks a tab left open overnight to sign
in again.

`touch_interval` is how often a request that only read the session
bothers to move the idle expiry. It is the difference between a store
write on every request and a store write once a minute. It defaults to
sixty seconds, or half the idle timeout where that is shorter, and one
set by hand has to stay under `idle_timeout` — otherwise a session in
constant use still expires, because nothing ever moves it.

Nothing schedules a sweep of expired sessions, so one rides along with
ordinary traffic: `gc_probability` (default `0.01`) is the chance that
a write also sweeps. Set it to `0` where a cron job calls `store.gc()`
instead.

### The Cookie

| Option | Default | |
| --- | --- | --- |
| `name` | `'zuri_session'` | |
| `path` | `'/'` | |
| `domain` | `nil` | `nil` scopes the cookie to the exact host, which is the narrower choice |
| `secure` | `nil` | `nil` follows the request's own scheme |
| `http_only` | `true` | |
| `same_site` | `'Lax'` | `'Strict'`, `'Lax'` or `'None'` |
| `persistent` | `false` | whether the cookie outlives the browser |

`secure` following the request is what lets development over cleartext
work while production over TLS gets `Secure` without being told. A
deployment behind a proxy that terminates TLS sets `secure: true`
itself.

`persistent: false` sends a cookie that ends with the browser session,
which is what a sign-in should normally do. `persistent: true` sends
`Max-Age` instead, and moves it forward on every request.

### Signing the Cookie

An identifier is 32 bytes from the platform's cryptographic generator,
which is far beyond guessing. Signing adds nothing against that, and
everything against volume: with a secret set, a cookie this server did
not issue is thrown out after one HMAC, rather than after a read from
disk or a query to the database.

```zuri,ignore
import env

server.use(http.session.session({ secret: env.require('SESSION_SECRET') }))
```

Set it on anything facing the open internet, and give every worker the
same secret — a cookie issued by one is otherwise refused by the next.
Turning signing on refuses the cookies issued before it, so it signs
everyone out once.

### What a Session May Hold

Whatever JSON holds: strings, numbers, booleans, `nil`, lists and
dictionaries of those. A class instance is not JSON, and storing one
raises when the session is written.

Sessions are for identity and small state — who is signed in, which
steps of a form are done, what to say on the next page. A payload over
`max_size` (default 65536 bytes) raises `SessionError`; the answer to
that is a row in a database with the session holding its key.

### Across Workers

`http.serve()` runs each worker in its own isolate, so the store is
built inside `setup` rather than handed in from outside:

```zuri,ignore
# app.zu
import http
import http.session

def setup(server) {
  server.use(http.session.session({
    store: http.session.FileStore('/var/lib/app/sessions'),
    secret: os.get_env('SESSION_SECRET'),
  }))

  server.get('/', @(request, response) {
    response.text(request.session().get('account', 'nobody'))
  })
}
```

A `sql` connection belongs to the isolate that opened it, so a worker
using `SqlStore` opens its own pool in `setup` too.

### What Sessions Do Not Do

Two requests writing the same session at the same moment — parallel
requests from one browser tab, usually — both succeed, and the one
that finishes last is the one that survives. The file store writes
through a rename, so a reader never sees half a session, and the SQL
store writes in one statement; neither takes a lock. Do not use a
session as a counter that several requests increment at once.

## TLS

```zuri,ignore
var server = http.server(443, '0.0.0.0')
server.load_certs('/etc/certs/site.crt', '/etc/certs/site.key')
server.listen()
```

Or from strings, with more control:

```zuri,ignore
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

```zuri,ignore
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

```zuri,ignore
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
`HttpServer`. It can live in the main script or in a module. What it
cannot do is reach for an imported module, because a module value
cannot cross into an isolate, so a `setup` that needs one like the
example above does, belongs in a module itself. The isolate resolves
it there by name, and its imports are resolved again on that side.

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

```zuri,ignore
import os

http.serve(app.setup, {
  port: 3000,
  on_ready: @(address, stop) {
    echo 'listening on ${address}'
    os.on_signal('INT', @() {
      stop()
      return true
    })
    os.on_signal('TERM', @() {
      stop()
      return true
    })
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
| `http.session` | `Session`, `SessionStore`, `FileStore`, `MemoryStore` |
| `http.session.sql` | `SqlStore`, for sessions kept in a database |
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

Two other standard library modules meet this one without it depending
on either: `HttpRequest.validate()` takes a schema from `validate`,
and `middleware.jwt_auth()` takes a verifier from `jwt`. Both are
duck-typed, so neither module is loaded by a server that does not use
it.

Every error this module raises descends from `HttpError`:
`ProtocolError` for a malformed message, `ConnectionError` for a
connection that failed, `TimeoutError`, `TooLargeError`,
`TooManyRedirectsError`, `StatusError`, `UnsupportedProtocolError` and
`SessionError`.
