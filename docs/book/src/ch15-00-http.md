# HTTP

The `http` module is Zuri's HTTP stack. It contains a client, a server and
the building blocks they share. It supports HTTP/1.1 and HTTP/2, over plain
connections or TLS, and it's designed to be exposed directly to the
internet.

That last point is a deliberate design decision. Many language runtimes
include an HTTP server that's fine for development, but expect you to put a
reverse proxy in front of it in production to terminate TLS, serve static
files, compress responses, enforce request limits and speak HTTP/2 to
browsers. All of those features are built into this module.

- [Introduction](#introduction)
  - [A First Request](#a-first-request)
  - [A First Server](#a-first-server)
- [Making Requests](#making-requests)
  - [The Shared Client](#the-shared-client)
  - [Creating a Client](#creating-a-client)
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
  - [Proxies](#proxies)
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
  - [Sessions Are Created on First Use](#sessions-are-created-on-first-use)
  - [Signing In](#signing-in)
  - [Flash Messages](#flash-messages)
  - [Where Sessions Are Kept](#where-sessions-are-kept)
  - [When a Session Ends](#when-a-session-ends)
  - [The Cookie](#the-cookie)
  - [Signing the Cookie](#signing-the-cookie)
  - [What a Session May Hold](#what-a-session-may-hold)
  - [Sessions Across Workers](#sessions-across-workers)
  - [Session Limitations](#session-limitations)
- [TLS](#tls)
- [HTTP/2](#http2)
- [WebSockets](#websockets)
- [Server-Sent Events](#server-sent-events)
- [Reverse Proxying](#reverse-proxying)
- [Running in Production](#running-in-production)
  - [Using More Than One Core](#using-more-than-one-core)
  - [Limits](#limits)
  - [Graceful Shutdown](#graceful-shutdown)
  - [Running Behind a Proxy](#running-behind-a-proxy)
- [Requests the Module Rejects](#requests-the-module-rejects)
- [Module Reference](#module-reference)

## Introduction

### A First Request

```zuri
import http

echo http.get('https://example.com').as_text()
```

`get()`, `post()`, `put()`, `patch()`, `delete()`, `head()`, `options()` and
`trace()` are all available as module-level functions. They all use one
shared client, which keeps its connections open between calls. [Request
Methods](#request-methods) describes each of them.

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

This server already does two things worth noticing. `GET /users/42` matches
the second route and passes `'42'` to the handler. And a `HEAD /` request is
handled by the first route, which responds with the headers a `GET` would
have produced but no body, because that's what HEAD means.

> Code blocks in this chapter that list several calls together, such as
> three ways to save, four filters or a family of methods, are **reference
> listings**, not programs. They show the form of each call, not a sequence
> you could run, and some of them would conflict if you pasted them into one
> file. Anything shown as a complete program in this chapter runs as
> written.

## A Complete Example You Can Run

Most of the examples in this chapter call a host that doesn't exist, or
start a server that never returns, so you can't paste them into a terminal
and watch them work. This one is different. It runs a real server on a real
port and sends real requests to it, using two files.

The server goes in its own file, because a function that uses an imported
module can't be spawned from the file that imported it. See [Concurrency
with Isolates](ch11-00-isolates.md) for the details.

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

There are four details here that are worth using in your own code.

**Port `0` asks the operating system for a free port.** `listen()` would
bind the port for you, but then there'd be no way to find out which port it
got. So the example calls `bind()` explicitly, reads the bound address, and
only then starts accepting connections. Use this pattern for tests and for
any process that runs more than one server.

**The port is sent over a channel.** The client can't know the port in
advance, and the two halves run in different isolates with separate heaps,
so a shared variable isn't an option.

**`request.json_body()` parses the request body**, and
`response.json(value, status)` writes the response. The names don't match
because the two methods do different jobs: one decodes what arrived, and the
other encodes the response and sets a status.

**`task.cancel()` stops the server.** `listen()` runs until the server is
closed, so without that call, the program would never exit.


## Making Requests

### The Shared Client

The module-level functions use a single `HttpClient` that exists for the
lifetime of the program. It reuses connections, so a second request to a
host it has already connected to skips the TCP handshake, and for HTTPS, the
TLS handshake as well.

```zuri
import http

var page = http.get('https://example.com')

echo page.status # 200
echo page.headers.get('content-type')
echo page.as_text()
```

Use `http.shared_client()` to get that client when you want a setting to
apply to every simple request in a program, and create your own client for
anything more specific.

### Creating a Client

```zuri,ignore
var api = http.client('https://api.example.com', {
  headers: { 'Authorization': 'Bearer ' + token },
  read_timeout: 5000,
})

var me = api.get('/users/me').raise_for_status().as_dict()
```

If you give a client a base URL, it's added to the front of any target that
isn't already an absolute URL, so the rest of your program can refer to the
service by path alone. Every key in the options dictionary is a field of
`HttpClient`, apart from `headers`, and you can also set any of them later:

```zuri,ignore
api.user_agent = 'my-service/2.1'
api.max_redirects = 3
api.set_header('X-Client-Version', '2.1')
```

> **Note**
> Create a client once and reuse it. Creating a new client for each request
> throws away its connection pool, its cookie jar and its TLS configuration,
> which is most of what a client is for.

### Request Methods

Each HTTP method has its own function. Every function is available in two
places, with the same signature: on the module, where it uses the shared
client, and on any `HttpClient` you create.

The three methods that send a body take it as their second argument. The
others take the options dictionary in that position.

| Call | Sends a body |
| --- | --- |
| `get(url, options)` | No |
| `post(url, data, options)` | Yes |
| `put(url, data, options)` | Yes |
| `patch(url, data, options)` | Yes |
| `delete(url, options)` | No |
| `head(url, options)` | No |
| `options(url, options)` | No |
| `trace(url, options)` | No |
| `request(method, url, options)` | If `options` includes one |

Every argument after the first is optional, so all the short forms work:

```zuri,ignore
import http

# Reading
http.get('https://example.com/items')
http.get('https://example.com/items', {
  query: { page: 2 },
})

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

Here are the same calls on your own client, which is what you should use for
anything that runs more than once:

```zuri,ignore
var api = http.client('https://api.example.com')

api.get('/items')
api.post('/items', { name: 'Widget' })
api.delete('/items/1')
```

`request()` takes the method as an argument, so you can use it to send a
method that doesn't have its own function, such as a WebDAV `PROPFIND` or a
custom method a service has defined:

```zuri,ignore
api.request('PROPFIND', '/files/', {
  headers: { 'Depth': '1' },
  body: '<propfind xmlns="DAV:"><allprop/></propfind>',
  content_type: 'application/xml',
})
```

HTTP method names are case-sensitive, and all the registered methods are
uppercase. If you pass a lowercase name, it's converted to uppercase for
you.

> **Note**
> `head()` doesn't follow redirects by default, and `delete()`, `options()`
> and `trace()` don't send a body. If you need to send a `DELETE` with a
> body, because an API wants a reason in it for example, use
> `request('DELETE', url, { json: ... })`.

### Request Bodies

The second argument to `post()`, `put()` and `patch()` is the body, and its
type determines how it's sent:

```zuri,ignore
api.post('/items', { name: 'Widget', price: 9 }) # JSON
api.post('/items', ['a', 'b']) # JSON
api.post('/items', 'raw text') # sent as-is
api.post('/items', file('photo.jpg', 'rb').read()) # sent as-is
api.post('/items', form_builder) # multipart
```

A dictionary or a list is sent as JSON, with the matching `Content-Type`. A
string or bytes value is sent exactly as it is, with no content type unless
you set one.

For other kinds of body, or to be explicit, set it in the options
dictionary:

| Option | Sends | Content-Type |
| --- | --- | --- |
| `body` | A string or bytes, exactly as given | None, unless `content_type` is set |
| `json` | Any value, encoded as JSON | `application/json` |
| `form` | A dictionary | `application/x-www-form-urlencoded` |
| `multipart` | A `MultipartBuilder` | `multipart/form-data`, including the boundary |
| `content_type` | Nothing by itself | Overrides the content type set by any of the above |

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
  form: {
    tag: ['new', 'featured'],
    q: 'zuri',
  },
})
```

`Content-Length` is set for you based on the body, and you can't override
it. If the client and server disagree about how long a body is, that's a
request smuggling vulnerability, not a matter of formatting.

### Uploading Files

A file upload is a `multipart/form-data` body, and `MultipartBuilder` puts
one together for you:

```zuri,ignore
import http

var form = http.MultipartBuilder()

form.add_field('title', 'Holiday')
form.add_field('album', '2026')
form.add_file(
  'photo',
  'beach.jpg',
  file('beach.jpg', 'rb').read(),
  'image/jpeg',
)

var response = http.post('https://example.com/photos', form)
```

Passing the builder as the body is all you need to do. The `Content-Type`,
its boundary parameter and the `Content-Length` are all derived from it.

| Method | What it does |
| --- | --- |
| `add_field(name, value)` | Adds a plain form field. Numbers and booleans are converted to strings. |
| `add_file(name, filename, content, content_type)` | Adds a file. `content` is bytes or a string, and `content_type` defaults to `application/octet-stream`. |
| `content_type()` | Returns the value of the `Content-Type` header, including the boundary |
| `boundary()` | Returns the boundary string |
| `build()` | Returns the complete body, as bytes |
| `length()` | Returns how many parts have been added |

To send several files under the same field name, call `add_file()` several
times. That's what an `<input type="file" multiple>` sends:

```zuri,ignore
for path in paths {
  form.add_file('attachments', os.base_name(path), file(path, 'rb').read())
}
```

A filename that isn't plain ASCII is sent twice: once as a transliterated
`filename`, and once as an RFC 5987 `filename*`. That way, the real name
survives even if the recipient only understands one of the two.

If you need the body on its own, for example to sign it, log its size or
send it somewhere other than through this client, build it yourself:

```zuri,ignore
var body = form.build()

api.post('/photos', nil, { body, content_type: form.content_type() })
```

> **Note**
> `build()` assembles the entire body in memory. For an upload that's large
> enough for this to matter, send the file as a raw body with its own
> content type instead. `multipart/form-data` is only worth its overhead
> when there are other fields alongside the file.

The boundary is generated from the platform's secure random number
generator, not from a timestamp or a counter. If someone else could predict
the boundary, they could include it in a field value to forge extra parts.

### Reading a Response

```zuri,ignore
var response = api.get('/items/1')

response.status # 200
response.reason_phrase() # 'OK'
response.version # '1.1' or '2'
response.headers.get('etag')

response.as_text() # the body, decoded as UTF-8
response.as_dict() # the body, parsed as JSON
response.as_bytes() # the body, untouched

response.is_ok() # 2xx
response.is_redirect() # 3xx
response.is_error() # 4xx or 5xx
```

`raise_for_status()` raises a `StatusError` for a 4xx or 5xx status, and
otherwise returns the response, so you can use it inline. The response is
still available on the error, which matters because that's usually where an
API explains what went wrong:

```zuri,ignore
catch {
  var data = api.get('/items/1').raise_for_status().as_dict()
} as error {
  echo error.response.status
  echo error.response.as_text()
}
```

Compressed response bodies are decompressed automatically, whether they use
gzip, deflate, brotli or zstd. Once a body has been decompressed, the
`Content-Encoding` header is removed, so nothing later on tries to decode it
again.

### Query Parameters and Headers

```zuri,ignore
api.get('/search', {
  query: {
    q: 'zuri',
    page: 2,
    tag: ['new', 'featured'],
  },
  headers: { 'Accept-Language': 'en-GB' },
})
```

A list value repeats the parameter, which is how a query string sends more
than one value with the same name. Headers set for a request are merged with
the client's own headers, and take priority over them.

### Authentication

```zuri,ignore
api.get('/me', {
  auth: ['bearer', token],
})
api.get('/me', {
  auth: ['basic', 'ada', 'lovelace'],
})
```

Credentials passed this way only apply to the origin you sent the request
to. If the response redirects to a different scheme, host or port, the
credentials are dropped. Otherwise, a redirect to a host chosen by someone
else would be a way to collect your `Authorization` header.

### Redirects

Redirects are followed by default, up to `max_redirects` (10 by default),
and the response records how many were followed:

```zuri
var final = http.get('https://example.com/old')

final.redirects # how many were followed
final.responder # the URL that finally answered
```

When a `303` is followed, the request becomes a `GET` without a body, and in
practice the same happens for a `301` or `302`. That's what browsers and
other clients do. A server that wants the method and body to be kept should
send a `307` or `308`, and both of those are followed with the method and
body unchanged.

You can turn off redirect following for a single request or for a whole
client:

```zuri,ignore
http.get(url, { follow_redirects: false })
```

`head()` doesn't follow redirects by default, because a HEAD request is
usually meant to look at the exact response that a redirect would hide.

### Cookies

A client with a cookie jar keeps cookies between requests, so logging in and
the requests that follow behave as they would in a browser:

```zuri,ignore
var session = http.client('https://example.com')
session.enable_cookies()

session.post('/login', nil, {
  form: { user: 'ada', password: secret },
})
session.get('/dashboard') # sends the session cookie
```

The cookie jar follows the matching rules in RFC 6265. A cookie without a
`Domain` is only sent to the host that set it, a cookie with a `Domain` is
also sent to subdomains, a path matches every path below it, and a `Secure`
cookie is never sent over an unencrypted connection. If a server tries to
set a cookie for a domain it doesn't control, the cookie is rejected.

### Timeouts and Retries

```zuri,ignore
var api = http.client('https://api.example.com', {
  connect_timeout: 5000,
  read_timeout: 10000,
  write_timeout: 10000,
})

api.get('/slow', { timeout: 30000 }) # this one request
```

All timeouts are in milliseconds. `max_retries` retries a failed request,
but only for idempotent methods. Repeating a `POST` could mean charging
someone's card twice, so `GET`, `HEAD`, `PUT`, `DELETE`, `OPTIONS` and
`TRACE` are retried, and nothing else is.

If the server closed a pooled connection while it was idle, the next write
on that connection fails in a way that looks exactly like a network failure.
In that specific case, an idempotent request is retried once, even with the
default `max_retries` of zero. Otherwise, every connection that a server
closes for being idle would show up as an error.

### Streaming a Response

You don't have to hold a large response in memory:

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

`finish()` returns the connection to the pool once you've finished with the
body. Until then, the connection belongs to you, because the client has no
way of knowing when you've finished reading.

### TLS and Certificates

By default, certificates are verified against the platform's trusted
certificates. To connect to a service with an internal or self-signed
certificate, add its certificate authority:

```zuri,ignore
api.add_ca(file('/etc/ssl/internal-ca.pem').read())
```

There's also `verify = false`, which turns off verification completely. It's
meant for local development only. With it, the connection is encrypted but
not authenticated, which means anyone in the middle can easily intercept it.
Everywhere else, use `add_ca()`.

### Proxies

A client uses the proxy set in the environment, the same way command-line
tools do: `HTTPS_PROXY` for `https` addresses, `HTTP_PROXY` for `http`
addresses, and `ALL_PROXY` for both. Each variable can be written in
uppercase or lowercase. `NO_PROXY` is a comma-separated list of hosts that
are reached directly. A name in the list also covers every name under it, an
entry with a port only covers that port, and `*` covers everything. The
machine's own loopback addresses are never reached through a proxy.

A client you create yourself can specify its own proxy, or ignore the
environment completely:

```zuri,ignore
var api = http.client('https://api.example.com', {
  proxy: 'http://ada:secret@proxy.internal:3128',
})

var direct = http.client('https://api.example.com', { trust_env: false })
```

An `https` request is tunneled through the proxy with `CONNECT`, so the
proxy only passes the encrypted bytes along and can't read them, and the
certificate that's checked belongs to the destination. An `http` request is
passed to the proxy in full. Any credentials in the proxy address are only
sent to the proxy, as a `Proxy-Authorization` header. The connection to the
proxy itself uses plain HTTP, and an `https://` proxy address is rejected.

## Serving Requests

### Routing

A route pattern consists of literal segments, `:name` parameters and an
optional catch-all at the end:

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

A literal segment always takes priority over a parameter, and a parameter
always takes priority over a catch-all. So `/users/new` and `/users/:id` can
both exist, and the more specific route wins, whichever was registered
first. Routes are matched using a trie, so a router with a thousand routes
costs the same per request as one with ten.

`get()`, `post()`, `put()`, `patch()`, `delete()`, `head()` and `options()`
each register a route for one method. `any()` registers `GET`, `POST`,
`PUT`, `PATCH`, `DELETE` and `OPTIONS` all at once. `handle()` takes the
method as an argument, which is how you add a route for a method that
doesn't have its own function, such as `PROPFIND` or a custom method:

```zuri,ignore
server.handle('PROPFIND', '/files/' + '*path', list_properties)
```

Three kinds of request are answered without a handler:

- `HEAD` uses the `GET` route for the same path, and the body is removed
  from the response.
- `OPTIONS` gets a `204` response with an `Allow` header that lists the
  methods the path accepts.
- A request for a path that only exists for other methods gets a `405`
  response, again with an `Allow` header, rather than a `404`, which would
  be wrong.

Give a route a name so you can build URLs from it later:

```zuri,ignore
server.get('/users/:id', show_user, 'user.show')

server.routes().url_for('user.show', { id: 42 }) # '/users/42'
```

### The Request Object

```zuri,ignore
server.post('/items', @(request, response) {
  request.method # 'POST'
  request.path # decoded and normalized
  request.target # exactly as it arrived on the wire
  request.version # '1.1' or '2'
  request.secure # whether it came over TLS

  request.param('id') # a route parameter
  request.query_param('page', '1') # a query parameter, with a default
  request.header('accept') # a header field
  request.cookie('session') # a cookie
  request.bearer_token() # the token from an Authorization header

  request.text() # the body as text
  request.json_body() # the body parsed as JSON
  request.form() # urlencoded or multipart fields
  request.file('avatar') # an uploaded file
})
```

`path` is percent-decoded and then normalized, in that order. That's the
only order that works, because `%2e%2e%2f` is `../` encoded to get past a
naive check. Route on `path`. `target` is the raw form, and matching on it
is how directory traversal attacks get through.

An uploaded file is represented by an `UploadedFile`:

```zuri,ignore
var upload = request.file('avatar')

upload.filename # what the client claimed
upload.safe_name() # that, reduced to one path-safe segment
upload.content_type # what the client declared
upload.size()
upload.save_to('/var/uploads/' + upload.safe_name())
```

`filename` and `content_type` are both controlled by the client, and neither
tells you anything reliable about the contents. `safe_name()` removes any
directory part, including a Windows one, and reduces the rest to letters,
digits, `.`, `-` and `_`.

### Forms and File Uploads

A submitted form arrives as the request body in one of two encodings, and
you read both through the same methods. The `enctype` attribute on the
`<form>` decides which one a browser sends: the default,
`application/x-www-form-urlencoded`, for a form with only plain fields, and
`multipart/form-data` for a form that includes a file.

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
| `form()` | Every field as `name -> value`, keeping the first value of a repeated name |
| `form_all()` | Every field as `name -> [value, ...]` |
| `form_field(name, fallback)` | The first value of one field |
| `files()` | The uploaded files as `name -> [UploadedFile, ...]` |
| `file(name)` | The first file uploaded under `name`, or `nil` |

The body is parsed the first time you use it and then cached, so reading
`form()` in a middleware and again in the handler only parses it once. A
body in neither encoding gives you an empty dictionary instead of raising an
error. Use `request.json_body()` for a JSON body and `request.text()` for
anything else.

A field that can appear more than once, such as a group of checkboxes or a
multiple select, needs `form_all()`, because `form()` only keeps the first
value:

```zuri,ignore
var tags = request.form_all().get('tags', [])
```

> **Note**
> After a successful `POST`, redirect with `303 See Other`, as shown above.
> That stops the browser from submitting the form again if the user reloads
> the resulting page.

#### Receiving an Upload

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

An `UploadedFile` has these members:

| Member | Description |
| --- | --- |
| `filename` | The name the client sent, exactly as it was sent |
| `safe_name()` | That name reduced to a single segment that's safe to use in a path |
| `content_type` | The type the client declared, or `application/octet-stream` |
| `content` | The contents, as bytes |
| `size()` | The number of bytes |
| `name` | The form field the file was uploaded under |
| `headers` | All the headers on that part of the body |
| `save_to(path)` | Writes the contents to `path` and returns the number of bytes written |
| `to_text()` | The contents decoded as UTF-8 |

An `<input type="file" multiple>` sends several parts under one name, which
is why `files()` returns a list for each name:

```zuri,ignore
for upload in request.files().get('attachments', []) {
  upload.save_to(os.join_paths('./uploads', upload.safe_name()))
}
```

#### What a Handler Shouldn't Trust

**The filename.** The client controls it, and it often contains a full local
path from a Windows client, `..` segments from a malicious one, or a NUL
byte intended to cut off a later check. Never join it to a path directly:

```zuri,ignore
os.join_paths('./uploads', upload.filename) # no
os.join_paths('./uploads', upload.safe_name()) # yes
```

`safe_name()` removes every directory part, with either kind of separator,
and reduces the rest to letters, digits, `.`, `-` and `_`. If nothing usable
is left, it returns `'unnamed'`. Better still, choose the file name yourself
and keep the client's name as a label:

```zuri,ignore
var stored = '${uuid.v4()}.jpg'
upload.save_to(os.join_paths('./uploads', stored))
record_upload(stored, upload.filename)
```

**The content type.** `content_type` is whatever the client wrote in the
part's header, and it doesn't tell you anything about the actual contents. A
file that claims to be `image/png` could be anything. When the difference
matters, check the bytes themselves. `mime.detect_from_header()` reads the
first bytes of a real file, so to check an upload, you need to write it
somewhere first:

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

#### Size Limits

The body is read into memory, up to the server's `max_body_size`, which is
10 MiB by default. That's intentionally small:

```zuri,ignore
server.max_body_size = 50 * 1024 * 1024 # accept uploads up to 50 MB
```

If a request *declares* a body larger than the limit, it's rejected with
`413 Content Too Large` before any of the body is read, which is exactly
what `Content-Length` is for. A request that gives a false length is cut off
at the limit and also rejected. Either way, the connection is then closed,
since a partly read body leaves nothing that can safely be parsed after it.

A client that asks for permission first with `Expect: 100-continue` gets an
answer before it sends anything. The server checks the declared length
against the limit and either responds with `100 Continue` or rejects the
request straight away, so a rejected upload only costs one round trip
instead of a whole transfer.

For an upload that's too large to hold in memory, accept it as a raw body
and stream it, rather than as a form field. `multipart/form-data` is only
worth its overhead when there are other fields alongside the file.

### Request Validation

You can validate a request against a `validate` schema:

```zuri,ignore
import http
import validate

var create_user = validate.schema({
  name: validate.required().string().max_length(100),
  email: validate.required().string().email(),
  age: validate.required().integer().gte(18).lte(120),
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

`validate()` returns the input it checked, so the success path takes one
line, and the data you go on to use is the data that was validated. If
validation fails, it raises the schema's own `validate.ValidationError`,
which has an `errors` list of `{ field, message }` entries. `group_errors()`
turns that list into a dictionary keyed by field, which is the format most
front ends want:

```json
{
  "errors": {
    "email": ["The email field must be a valid email address."],
    "age": ["The age field must be greater than or equal to 18."]
  }
}
```

If you'd rather branch than catch an error, validate the input yourself.
There's no separate API for it:

```zuri,ignore
var result = create_user.check(request.input())

if !result.valid {
  response.json({ errors: result.errors }, 422)
  return
}
```

The rules themselves, of which there are around eighty, including rules that
compare fields such as `confirmed()` and `required_if()`, are part of the
`validate` module, not this one.

#### What Gets Validated

`request.input()` is the dictionary that `validate()` checks. It merges
three sources, each overriding the one before it:

1. Route parameters from the pattern that matched
2. Query string parameters
3. The body: the keys of a JSON object, or the submitted form fields

That means one schema works for `POST /users` with a JSON body,
`GET /users?…` with a query string and `/users/:id` with a route parameter,
and the handler doesn't need to care which one it got.

To use only one of the sources, name it:

```zuri,ignore
request.validate(schema, 'body') # only the body
request.validate(schema, 'query') # only the query string
request.validate(schema, 'params') # only the route parameters
```

Two things are intentionally left out of the merged input:

- **Uploaded files.** There's nothing a schema rule could usefully say about
  a file's bytes. Get files with `request.file()` and check them as
  described in [Forms and File Uploads](#forms-and-file-uploads).
- **A JSON body that isn't an object.** An array or a plain string has no
  names to merge, so it doesn't contribute anything. Read it with
  `request.json_body()`.

A body that can't be parsed as JSON also contributes nothing, instead of
raising an error. The schema's `required` rules then report what's missing,
which is a more helpful response to a client than a parser error.

#### Values From Requests Are Strings

Query strings and URL-encoded forms only contain text. `?age=36` gives you
the string `'36'`, not the number `36`, and that affects which rules apply:

| Rule | On `'36'` | Why |
| --- | --- | --- |
| `integer()`, `numeric()`, `gt()`, `gte()`, `lt()`, `lte()` | Treats it as 36 | These rules convert the value |
| `size()`, `min()`, `max()`, `between()` | Treats it as 2 | These rules measure *size*, which for a string is its number of characters |

That's how `validate` is documented to work, not a quirk of this module.
`min(8)` on a password means eight characters. It's only surprising when a
schema written for a JSON body is later used for a query string.

If a schema has to work with both, use the rules that check values:

```zuri,ignore
age: validate.required().integer().gte(18).lte(120)   # both
age: validate.required().integer().between(18, 120)   # JSON bodies only
```

`input()` doesn't convert anything for you. It would have to guess, and
silently turning a postcode like `'01234'` or a version like `'1.0'` into a
number would be worse than having to choose the right rule.

#### Repeated fields

A name can legitimately appear more than once in a query string or a form,
so those sources arrive as `name -> [values]`. `input()` replaces a list
holding exactly one value with that value, and leaves a list of several
values as it is:

```
?tag=a           ->  { tag: 'a' }
?tag=a&tag=b     ->  { tag: ['a', 'b'] }
```

That way, a rule for a single value sees a single value. If a field must
*always* be a list, however many values arrived, read it with `form_all()`
or from `request.query` directly, and validate it with `validate`'s `.*`
wildcard.

> **Note**
> The `http` module doesn't import `validate`. `validate()` accepts any
> object that has a `check_or_raise()` method and calls it, so a server that
> doesn't validate anything never has to load a schema engine, and your own
> schema objects, or ones from another library, work just as well.

### The Response Object

```zuri,ignore
response.text('plain') # text/plain
response.html('<h1>hi</h1>') # text/html
response.json({ ok: true }) # application/json
response.xml('<doc/>') # application/xml
response.write('more') # append, no content type

response.file('/var/www/report.pdf') # streamed from disk
response.download('/var/www/report.pdf') # ...as an attachment
response.render('pages/home', { user }) # a Wire template

response.redirect('/elsewhere') # 302 by default
response.redirect('/elsewhere', 308) # method-preserving

response.status = 201
response.header('X-Thing', 'value')
response.content_type('text/csv')
response.content_disposition('attachment', 'Résumé.pdf')
response.cache_for(3600)
response.no_cache()
```

Every method that writes a body also sets a suitable `Content-Type`, and
each one returns the response, so you can chain them.

`file()` streams the file from disk instead of reading it into memory, so
serving a large file takes only one line.

`content_disposition()` tells the browser whether to display the body or
save it, and under what name. A name with characters outside printable
ASCII, or one that contains a quote, a backslash or a percent sign, is sent
as RFC 6266 describes: in full in `filename*`, with a plain ASCII substitute
in `filename`. `download()` sends its file name the same way, so a body
written from memory and a file from disk are saved under the same name.

### Middleware

A middleware function takes `(request, response, next)` and decides whether
the rest of the chain runs:

```zuri,ignore
server.use(@(request, response, next) {
  var started = time()
  next()
  echo '${request.method} ${request.path} ${response.status} ' +
      '${(time() - started) * 1000}ms'
})
```

If a middleware doesn't call `next()`, the request stops there, which is
exactly what an authentication or rate-limiting layer needs:

```zuri,ignore
server.use(@(request, response, next) {
  if request.header('x-api-key') != expected {
    response.json({ error: 'unauthorized' }, 401)
    return
  }
  next()
})
```

Middleware runs in the order it was added, with the first one added on the
outside, so it wraps everything added after it.

Code after `next()` only runs if the rest of the chain returned normally. If
a handler raises an error, control never comes back there, and the server's
error handling later decides on the `500` that's sent to the client. A
middleware that needs to report on the response the client actually receives
should register a callback with `response.on_finish()` instead. It runs once
the response is final, including when the request failed:

```zuri,ignore
server.use(@(request, response, next) {
  var started = time()

  response.on_finish(@{
    echo '${request.method} ${request.path} ${response.status} ' +
        '${(time() - started) * 1000}ms'
  })

  next()
})
```

Each callback runs once, in the order it was registered, and a callback that
raises an error is skipped instead of costing the client its response.
`middleware.logger()` works this way.

If a middleware wants to pass something to the handler, it puts it in
`request.context`, which is a plain dictionary meant for exactly that:

```zuri,ignore
server.use(@(request, response, next) {
  request.context['started_at'] = time()
  next()
})
```

A middleware that needs to work with the *response* calls `next()` first,
and then works with the result:

```zuri,ignore
server.use(@(request, response, next) {
  next()
  response.header('X-Served-By', hostname)
})
```

The middleware that most services need is already available. See [Built-in
Middleware](#built-in-middleware).

### Errors

If a handler raises an error, the client gets a `500`, and the error message
is never sent to it. Error messages often contain a file path, a query or
part of the data being processed, and none of that should be sent to whoever
caused the error.

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

`on_error()` listeners see every error, including failures on the connection
that never reached a handler. `error_handler()` creates the response body
for a failed request. Without one, the server sends a plain `500`.

### Static Files

```zuri,ignore
server.serve_files('/static', './public', {
  cache_age: 86400,
  precompressed: true,
})
```

That one call handles everything a static file needs to work properly with a
browser or a CDN:

- `ETag` and `Last-Modified` headers, and `304 Not Modified` for a
  conditional request that still matches
- `Range` requests, answered with `206` and a `Content-Range` header, and
  `416` for a range outside the file
- `If-Match`, `If-Unmodified-Since`, `If-None-Match`, `If-Modified-Since`
  and `If-Range`, evaluated in the order RFC 9110 specifies
- `Content-Type` based on the file extension
- `index.html` for a request that names a directory
- With `precompressed`, a `.br` or `.gz` file next to the original, served
  instead of it when the client accepts that encoding

Every request path is percent-decoded, normalized and joined to the root,
and then checked to make sure it's still inside the root. All of these steps
are needed, because doing only some of them isn't enough. Dotfiles aren't
served at all. Files like `.env`, `.git` and `.htpasswd` end up in a web
root at some point in most projects, and none of them should ever be served.

For a single-page application, `fallback` serves the application's shell for
any path that doesn't name a file, so client-side routes work when the page
is reloaded:

```zuri,ignore
server.serve_files('/', './dist', { fallback: 'index.html' })
```

### Compression

Responses are compressed on the way out, and compression is on by default. A
response is compressed when all of these conditions are met:

- `server.compression` is on, which it is by default.
- The body is at least `server.compression_min_size` bytes, which is 1024 by
  default.
- Its media type benefits from compression. That includes anything starting
  with `text/`, as well as JSON, JavaScript, XML, SVG, WebAssembly and the
  `+json` and `+xml` types.
- The client's `Accept-Encoding` header accepts `br` or `gzip`.
- Nothing has already set a `Content-Encoding`.
- The compressed body is actually smaller than the original.

When the client accepts both, Brotli is used instead of gzip.
`Vary: Accept-Encoding` is added either way, so a shared cache in front of
the server can't send a compressed body to a client that didn't ask for one.

To turn compression off, or change the threshold:

```zuri,ignore
server.compression = false # off entirely
server.compression_min_size = 4096 # only bodies over 4 KiB
```

The list of media types is an allowlist rather than a denylist on purpose. A
JPEG, an MP4 or a zip file is already compressed, and running it through
Brotli would use CPU time only to make it slightly larger.

> **Note**
> Responses created with `response.file()`, which includes everything
> `serve_files()` serves, are **not** compressed on the fly. They're
> streamed from disk instead of being held in memory, and compressing them
> for each request would mean reading them into memory first. Compress those
> files ahead of time, and let the static file handler pick up the
> compressed versions:
>
> ```zuri
> server.serve_files('/static', './public', { precompressed: true })
> ```
>
> With this option on, a request for `site.css` from a client that accepts
> Brotli is answered with `site.css.br` if that file exists, or with
> `site.css.gz` if that exists and the client accepts gzip. This costs no
> CPU time per request, and the response gets the correct `Content-Encoding`
> and `Vary` headers.

On the client side, there's nothing to configure.
`Accept-Encoding: gzip, br, deflate, zstd` is sent by default, and
compressed response bodies are decoded before you see them.

### Streaming a Response Body

A response body can be produced by a callback instead of being held in
memory:

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

Unless a `Content-Length` was set beforehand, the body is sent with chunked
transfer encoding over HTTP/1.1, and as an ordinary DATA stream over HTTP/2.
The handler doesn't need to know which one is being used.

### Setting Cookies

```zuri,ignore
response.set_cookie('theme', 'dark', {
  max_age: 86400,
  secure: true,
  same_site: 'Strict',
})

response.clear_cookie('theme')
```

`http_only` defaults to `true` and `same_site` defaults to `'Lax'`, which is
what a cookie containing anything sensitive should have. Pass them
explicitly if you want something else. If a cookie's name starts with the
`__Secure-` or `__Host-` prefix, the rules for that prefix are applied
automatically, so the cookie isn't sent in a form that the browser would
silently refuse to store.

For state that belongs to a visitor rather than to the browser, use a
session. A session keeps the state on the server and only puts an identifier
in the cookie. [Sessions](#sessions) covers them.

### Content Negotiation

```zuri,ignore
import http.negotiate

var type = negotiate.best_match(request.header('accept'), [
  'application/json',
  'text/html',
])
```

`best_match()` chooses the format the client would most like from the ones
you can produce. It takes quality weights into account, and your own order
breaks ties. `request.accepts('text/html')` and `request.wants_json()` cover
the most common cases.

## Built-in Middleware

`http.middleware` contains the middleware that almost every public service
ends up needing. Each function *returns* a middleware, so you call it at the
point where you register it:

```zuri
import http
import http.middleware

var server = http.server(3000)

server.use(middleware.request_id())
server.use(middleware.logger())
server.use(middleware.security_headers())
```

They're ordinary middleware with no special privileges, so you can read any
of them as an example of how to write your own.

None of them raise an error when they're created. That matters for the three
that take a verifier, [`basic_auth()`](#basic_auth),
[`bearer_auth()`](#bearer_auth) and [`jwt_auth()`](#jwt_auth). For these,
**passing no verifier disables the middleware**. It lets every request
through, instead of rejecting requests or failing when you register it.

```zuri
# Authentication, off. Everything else in the chain is untouched.
server.use(middleware.jwt_auth(nil))
```

This is intentional, and it's useful. Setting a verifier to `nil` turns off
its middleware without changing the rest of the chain, which is what you
want when you're tracking down which middleware in a stack is causing a
request to fail.

A middleware is only disabled when you ask for it, never as a fallback. A
verifier that's *present* and rejects a request still rejects it.

### `logger()`

Writes one line for each request, after the response has finished.

```zuri
server.use(middleware.logger())
```

```
127.0.0.1 - [Wed, 09 Sep 2026 20:09:17 GMT] "GET /static/big.txt HTTP/1.1" 200 320 3.07ms
```

This is the Common Log Format with the response time added, which every log
analysis tool already understands. The size is taken from `Content-Length`
when the body comes from a file or is streamed, so a static file is logged
with its real size, not zero.

| Option | Default | Meaning |
| --- | --- | --- |
| `sink` | `echo` | A function that receives each finished line |
| `format` | The format shown above | A function `(request, response, milliseconds)` that returns the line |
| `trust_proxy` | `false` | Logs the forwarded client address instead of the address of the connection |

```zuri
server.use(
  middleware.logger({
    sink: @(line) {
      log_file.puts(line + '\n')
    },
    trust_proxy: true,
  }),
)
```

For structured logging, use a `format` that returns JSON:

```zuri
server.use(
  middleware.logger({
    format: @(request, response, elapsed) {
      return json.encode({
        method: request.method,
        path: request.path,
        status: response.status,
        ms: elapsed,
        id: request.context.get('request_id', nil),
      })
    },
  }),
)
```

Requests that raise an error are logged too, with the status the client was
sent after the server handled the failure. That's either the `500` from the
default error handling, or whatever status the error handler chose.

### `request_id()`

Gives every request an identifier, so you can follow a single request
through a log file and across services.

```zuri
server.use(middleware.request_id())
```

The identifier is stored in `request.context['request_id']` and sent in the
response's `X-Request-Id` header. If the client sends its own identifier,
that one is used instead of a new one, which lets a trace continue across
services.

The only optional argument is the header name:

```zuri
server.use(middleware.request_id('X-Correlation-Id'))
```

An identifier from outside your service ends up in your logs, so it's
limited to 64 characters, and anything that isn't plainly printable is
removed before it's used.

### `security_headers()`

Adds the response headers that tell a browser to apply extra security
measures to a page.

```zuri
server.use(middleware.security_headers())
```

| Option | Default | Header |
| --- | --- | --- |
| `content_type_options` | `'nosniff'` | `X-Content-Type-Options`, which stops the browser from guessing a different content type from the one declared |
| `frame_options` | `'DENY'` | `X-Frame-Options`, which stops the page from being shown in a frame, as clickjacking requires |
| `referrer_policy` | `'strict-origin-when-cross-origin'` | `Referrer-Policy`, which keeps paths and query strings out of the referrer sent to other sites |
| `hsts_max_age` | `31536000` | `Strict-Transport-Security` |
| `hsts_subdomains` | `false` | Adds `includeSubDomains` |
| `content_security_policy` | Not set | `Content-Security-Policy` |

```zuri
server.use(
  middleware.security_headers({
    content_security_policy: "default-src 'self'; img-src 'self' data:",
    hsts_subdomains: true,
    frame_options: 'SAMEORIGIN',
  }),
)
```

Pass `nil` for any option to leave that header out.

`Content-Security-Policy` has no default, because a wrong policy breaks the
page and an overly permissive one doesn't protect anything. The right policy
depends on what your application actually loads.

`Strict-Transport-Security` is only sent over HTTPS. Over an unencrypted
connection, browsers ignore it at best, and at worst it can lock clients out
of a host that doesn't have a certificate.

Every header is set with `set_default`, so a handler that sets its own value
keeps it.

### `cors()`

Responds to CORS preflight requests, and adds the headers a browser needs
before it lets a script read a response from another origin.

```zuri
server.use(
  middleware.cors({
    origins: ['https://app.example.com'],
    credentials: true,
  }),
)
```

| Option | Default | Meaning |
| --- | --- | --- |
| `origins` | `['*']` | The allowed origins, matched exactly. `'*'` allows any origin. |
| `methods` | The seven usual methods | The methods a preflight response says are allowed |
| `headers` | The headers the request asked for | The request headers a script may set |
| `expose` | None | The response headers a script may read |
| `credentials` | `false` | Whether cookies and `Authorization` headers may be sent |
| `max_age` | `86400` | How long a preflight response may be cached, in seconds |

A request without an `Origin` header isn't a cross-origin request, so it's
passed straight through. If the `Origin` isn't in the list, the response
gets no CORS headers at all. The browser then won't let the script see the
response, which is the right outcome. Responding with an error instead would
reveal whether the resource exists.

> **Note**
> You can't combine a wildcard with `credentials`, because every browser
> rejects that combination. When `credentials` is set, the requesting origin
> is sent back instead, which makes the `origins` list the only thing
> standing between an attacker's page and an authenticated response.
> Whenever you use credentials, list the allowed origins explicitly.

`Vary: Origin` is added whenever the response depends on the request's
origin, so a shared cache can't serve one origin's response to another.

### `basic_auth()`

Requires HTTP Basic authentication.

```zuri
server.use(
  middleware.basic_auth(@(user, password) {
    return user == 'admin' and http.util.secure_equals(password, secret)
  }, 'Admin area'),
)
```

The first argument is a function that's called with the username and
password, and returns whether they're valid. The second argument is the
realm named in the challenge, which defaults to `'Restricted'`.

If authentication succeeds, the username is stored in
`request.context['user']`. If it fails, the response is a `401` with a
`WWW-Authenticate` header, which makes the browser show its login prompt.

Passing `nil` instead of a verifier disables the middleware completely, as
described in [the note above](#built-in-middleware).

Compare secrets with `http.util.secure_equals()` instead of `==`. It
compares in constant time, so a wrong guess and an almost correct guess take
the same amount of time, and the comparison can't be used to discover the
secret one character at a time.

To protect only part of a site, wrap the middleware:

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

Note that when a guard succeeds, the `next()` it calls continues the
*entire* rest of the chain, not just the handler. If you register two
authentication middleware globally, both apply to every request. When
they're meant for different parts of the site, limit each one to a path
prefix, as shown above.

### `bearer_auth()`

Requires a bearer token, which is the usual approach for an API.

```zuri
server.use(
  middleware.bearer_auth(@(token) {
    var claims = verify_jwt(token)
    return claims == nil ? false : claims.subject
  }),
)
```

The verifier is called with the token. It returns a falsy value to reject
the token, or a value to attach to the request, which is stored in
`request.context['user']`. You can return the subject, the claims or a
complete user record.

A failure produces a `401` with a `WWW-Authenticate: Bearer` challenge and a
JSON body. The realm is the optional second argument, and defaults to
`'api'`. As with `basic_auth()`, a `nil` verifier disables the middleware.

`middleware.parse_bearer(header)` and `middleware.parse_basic(header)` are
also exported, for code that needs to read an `Authorization` header without
installing a middleware, and `request.bearer_token()` reads the token
directly from a request.

For JSON Web Tokens in particular, use [`jwt_auth()`](#jwt_auth), which
handles the verification, the claims and the RFC 6750 challenges for you,
instead of leaving them to the verifier you write here.

### `jwt_auth()`

Requires a valid JSON Web Token, verified with the `jwt` module.

```zuri,ignore
import http.middleware
import jwt

server.use(
  middleware.jwt_auth(
    jwt.Verifier(secret, {
      algorithms: ['HS256'],
      audience: 'api',
    }),
  ),
)
```

The first argument is either a `jwt.Verifier`, or a function that takes the
token and returns its claims. The function form lets you use a key set
selected by `kid`, or anything else the `jwt` module can do that a fixed
verifier can't:

```zuri
server.use(
  middleware.jwt_auth(@(token) {
    return jwt.verify_with_jwks(token, keys, { audience: 'api' })
  }),
)
```

| Option | Default | Meaning |
| --- | --- | --- |
| `realm` | `'api'` | The realm named in the challenge |
| `optional` | `false` | Attaches the claims when a valid token is present, but doesn't reject requests without one |
| `scopes` | None | A list of scopes that every token must include |

If verification succeeds, the claims are stored in
`request.context['claims']`, and the `sub` claim, which is where an issuer
usually puts the account the token represents, is stored in
`request.context['user']`:

```zuri
server.get('/me', @(request, response) {
  response.json({
    account: request.context.get('user', nil),
    issued_at: request.context.get('claims', {}).get('iat', nil),
  })
})
```

Failures follow section 3 of RFC 6750:

| Situation | Response |
| --- | --- |
| No token at all | `401`, with `WWW-Authenticate: Bearer realm="api"` |
| The token can't be verified | `401`, with `error="invalid_token"` |
| The token is valid but is missing a required scope | `403`, with `error="insufficient_scope"` and the scope that was needed |

The last row is the reason for `scopes`. A token that failed authentication
and a token that's authenticated but not allowed to do something are
different problems. Responding to both with `401` tells the client to get a
new token, when a new token won't help.

> **Note**
> A challenge never says *which* check a token failed. An expired token and
> a forged one get exactly the same `error_description`. Knowing which it
> was is useful for your logs and for an attacker, and nobody else. If you
> need the reason, log it from the verifier you passed in.

`optional` is for a route that behaves differently when it knows who's
making the request, but doesn't require it, such as a public page that shows
an edit button to its author:

```zuri,ignore
server.use(middleware.jwt_auth(verifier, { optional: true }))

server.get('/posts/:id', @(request, response) {
  var viewer = request.context.get('user', nil)
  response.json(render_post(request.param('id'), viewer))
})
```

With `optional`, a token that's *present* but invalid is still rejected. If
a bad token were ignored, a client could tamper with a token and get the
anonymous view instead of an error, which would hide exactly the kind of
problem you want to know about.

A `nil` verifier disables the middleware. Every request passes through
without authentication, and `request.context['claims']` is never set.
Nothing here raises an error when it's created, so you never need a `catch`
around the registration.

```zuri,ignore
var verifier = production ? jwt.Verifier(secret, options) : nil

server.use(middleware.jwt_auth(verifier))
```

Like `HttpRequest.validate()`, this middleware doesn't import the `jwt`
module. The caller creates the verifier, which leaves the choice of token
format to the application, and means a server that doesn't authenticate
anything never has to load the module.

### `rate_limit()`

Limits how many requests a single client can make in a given period of time.

```zuri
server.use(middleware.rate_limit({ limit: 100, window: 60 }))
```

| Option | Default | Meaning |
| --- | --- | --- |
| `limit` | `60` | The number of requests allowed per window |
| `window` | `60` | The length of the window, in seconds |
| `key` | The client's address | A function that takes the request and returns the key to count it under |
| `trust_proxy` | `false` | Takes the address from the forwarding headers |

Every response includes the current state of the limit, so a well-behaved
client can slow down before it's rejected:

```
RateLimit-Limit: 100
RateLimit-Remaining: 87
RateLimit-Reset: 41
```

A client over the limit gets `429 Too Many Requests` with a `Retry-After`
header.

To limit requests per API key instead of per address, provide a `key`
function:

```zuri
server.use(
  middleware.rate_limit({
    limit: 1000,
    window: 3600,
    key: @(request) {
      return request.header('x-api-key', nil) or
          request.client_ip() or
          'anonymous'
    },
  }),
)
```

> **Note**
> The counter is kept in memory, so each worker has its own. Running with
> `workers: 4` makes the effective limit four times `limit`. This is a
> deliberate trade-off. A shared counter would need shared state, and this
> middleware is meant to slow down a runaway client, not to meter usage for
> billing. If the exact number matters, divide `limit` by the number of
> workers, or keep the count somewhere all the workers can see it.

### `etag()`

Computes a weak `ETag` for the finished response body, and responds with
`304 Not Modified` when the client already has that version.

```zuri
server.use(middleware.etag())
```

The only optional argument is the smallest body size worth tagging, in
bytes. It defaults to `128`, because for smaller bodies, the validator costs
more than the body it would save.

If a handler has already set its own `ETag`, it's left alone, because the
handler knows something about the resource that hashing the bytes doesn't.
Responses backed by files are also left alone, because `serve_files()`
already tags them using the file's size and modification time.

Because it works with the finished body, it works with any kind of response,
such as a rendered template, a JSON document or a generated report.

### `force_https()`

Redirects every request that arrived over an unencrypted connection to the
same URL over HTTPS.

```zuri
server.use(middleware.force_https())
```

| Option | Default | Meaning |
| --- | --- | --- |
| `status` | `308` | The redirect status. `308` keeps the method and the body. |
| `port` | The default port | The HTTPS port, if it isn't 443 |

Use this on the unencrypted listener, which usually only exists to perform
this redirect:

```zuri
# port 80: redirect and nothing else
var redirector = http.server(80, '0.0.0.0')
redirector.use(middleware.force_https())
```

Combine it with `security_headers()`'s `Strict-Transport-Security` on the
TLS listener, so that after the first visit, the browser stops making
unencrypted requests altogether.

### Ordering

Middleware runs from the outside in, so the order in which you register it
is the order in which it wraps the request. Here's a reasonable default:

```zuri,ignore
server.use(middleware.request_id()) # so everything after can log it
server.use(middleware.logger()) # so it sees the final status
server.use(middleware.force_https()) # before any work is done
server.use(middleware.security_headers())
server.use(middleware.cors({ origins: allowed }))
server.use(middleware.rate_limit({ limit: 100 }))
server.use(middleware.jwt_auth(verifier)) # after the cheap refusals
server.use(middleware.etag()) # innermost: it needs the finished body
```

Here's the reasoning behind each position. Identifiers come before anything
that might log. Logging wraps everything else so that it records what
actually happened. Cheap checks that might reject a request come before
expensive ones. And anything that looks at the response body goes on the
inside, where the body exists.

## Sessions

A session is state that belongs to a single visitor. It's kept on the server
and found again through a cookie that the browser sends back. Only the
identifier is sent to the browser, so a visitor can't read or change what
the session contains, and the cookie is useless to anyone who can't present
the exact value that was issued.

```zuri,ignore
import http

var server = http.server(3000)

server.use(http.session())

server.get('/', @(request, response) {
  var seen = request.session().get('seen', 0) + 1

  request.session().set('seen', seen)
  response.text('visit ${seen}')
})

server.listen()
```

`session()` is a middleware. Register it once, before anything that reads a
session, and every handler after it can get its session with
`request.session()`.

### Sessions Are Created on First Use

A request that never uses its session costs nothing: the store isn't read or
written, and no `Set-Cookie` header is sent. The session record is only
created the first time something is written to the session, which stops a
crawler working through a public site from filling the store with empty
sessions.

A request that only reads an existing session doesn't write anything back
either, except to move the idle expiry time forward, at most once every
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

`regenerate()` is the line you need to get right. It gives the session a new
identifier and deletes the record stored under the old one, while keeping
everything the session contains.

Without it, an attacker who manages to set a cookie in the victim's browser
beforehand, through a forgotten subdomain, an open redirect or a shared
computer, knows which identifier the victim will be signed in under, and can
simply use it afterwards. This attack is called session fixation, and a new
identifier is the entire defense against it. Call `regenerate()` whenever
the meaning of the session changes, such as when a user signs in, becomes an
administrator or completes an extra authentication step.

To sign out, call `destroy()`, which deletes the record and tells the
response to expire the cookie:

```zuri,ignore
server.post('/logout', @(request, response) {
  request.session().destroy()
  response.redirect('/')
})
```

`clear()` is the other option. It empties the session without ending it,
keeping the identifier and the cookie.

### Flash Messages

A handler that does some work and then redirects can't render a message
describing what happened. A flash message carries that message to the page
that can:

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

A flash message is used up by the next request that touches the session in
any way, whether or not that request reads this particular message. If a
page looks at the session without reading the message, the message isn't
kept for the page after it. A request that never touches its session, such
as a request for a static file or an image, leaves the message waiting.

### Where Sessions Are Kept

There are four places a session can be stored, and switching between them
means changing one line.

**`FileStore` keeps one file per session.** This is the default, because it
doesn't need any setup and it's shared between the workers that
`http.serve()` starts. If you don't give it a directory, it uses a private
subdirectory of the platform's temporary directory, created with permissions
`0700`:

```zuri,ignore
server.use(http.session())
```

The platform clears that directory on its own schedule, which on most
systems means every reboot. For sessions that need to survive a restart of
the host, specify your own directory:

```zuri,ignore
server.use(
  http.session({ store: http.session.FileStore('/var/lib/app/sessions') }),
)
```

A session file is a credential, just like the cookie. The directory is
created with permissions `0700` and each file with `0600`, and a directory
that every user on the machine can access is rejected instead of used.
That's why the temporary directory itself is never the default. Pass
`strict_permissions: false` to accept such a directory anyway.

**`SqlStore` keeps one row per session.** It has its own import, so a
program that uses the default store never loads the `sql` module:

```zuri,ignore
import http.session.sql { SqlStore }
import sql

var store = SqlStore(sql.pool('postgres://localhost/app'))
store.migrate()

server.use(http.session({ store }))
```

`migrate()` creates the table and its index if they don't already exist, and
it's safe to call every time the program starts. It accepts a
`sql.Connection` or a `sql.Pool`, and a server should use a pool.

**`MemoryStore` is for tests.** Nothing survives a restart, and nothing is
shared between workers, so if a browser's next request goes to a different
worker, that worker has never heard of its session. It's the right store for
tests and the wrong one for real traffic.

**You can write your own store.** A store has five methods, and none of them
sees a session identifier or needs to understand the contents of a session:

```zuri,ignore
class RedisStore < http.session.SessionStore {
  @new(client) {
    self._client = client
  }

  read(key) {
    return self._client.get('session:' + key)
  }

  write(key, payload, expires_at) {
    self._client.set_with_ttl(
      'session:' + key,
      payload,
      (expires_at - time()).ceil(),
    )
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

`touch()` has a default implementation built on `read()` and `write()`.
Override it if your backend can update an expiry time by itself.

The key passed to a store is the SHA-256 hash of the identifier, not the
identifier itself. Someone who can read the directory, the table or a backup
of either can see what's in the sessions, but can't resume one, because they
don't have the value the browser presents.

### When a Session Ends

A session has two time limits, and it ends when either one runs out:

| Option | Default | Meaning |
| --- | --- | --- |
| `idle_timeout` | `7200` | Seconds of inactivity allowed. The limit moves forward while the visitor is active. |
| `lifetime` | `86400` | Seconds the session can last, however active the visitor is |

You can set either one to `nil` to remove that limit, but not both.

The absolute lifetime is what makes a user sign in again after leaving a tab
open overnight.

`touch_interval` is how often a request that only reads the session updates
the idle expiry time. It's the difference between writing to the store on
every request and writing once a minute. It defaults to sixty seconds, or
half of the idle timeout if that's shorter. If you set it yourself, it has
to be less than `idle_timeout`. Otherwise, even a session that's constantly
in use would expire, because nothing would ever move its expiry time
forward.

Nothing schedules a regular cleanup of expired sessions, so cleanup happens
as part of ordinary traffic. `gc_probability` (`0.01` by default) is the
chance that a write also cleans up expired sessions. If you'd rather run
`store.gc()` from a cron job, set it to `0`.

### The Cookie

| Option | Default | Meaning |
| --- | --- | --- |
| `name` | `'zuri_session'` | |
| `path` | `'/'` | |
| `domain` | `nil` | `nil` limits the cookie to the exact host, which is the more restrictive choice |
| `secure` | `nil` | `nil` follows the scheme of the request |
| `http_only` | `true` | |
| `same_site` | `'Lax'` | `'Strict'`, `'Lax'` or `'None'` |
| `persistent` | `false` | Whether the cookie lasts after the browser closes |

Because `secure` follows the request by default, development over plain HTTP
works, while production over TLS gets the `Secure` attribute automatically.
A deployment behind a proxy that terminates TLS should set `secure: true`
itself.

`persistent: false` sends a cookie that ends when the browser session ends,
which is what a sign-in should usually do. `persistent: true` sends a
`Max-Age` instead, and moves it forward on every request.

### Signing the Cookie

An identifier is 32 bytes from the platform's cryptographic random number
generator, which is far too large to guess. Signing doesn't add any
protection against guessing, but it helps a lot against high volumes of fake
cookies. With a secret set, a cookie that this server didn't issue is
rejected after a single HMAC calculation, instead of after a disk read or a
database query.

```zuri,ignore
import env

server.use(http.session({ secret: env.require('SESSION_SECRET') }))
```

Set a secret on any server that's exposed to the internet, and give every
worker the same secret, because otherwise a cookie issued by one worker is
rejected by the next. Turning on signing rejects all cookies issued before
it, so everyone is signed out once.

### What a Session May Hold

A session can hold anything JSON can represent: strings, numbers, booleans,
`nil`, and lists and dictionaries of those values. A class instance can't be
represented as JSON, so storing one raises an error when the session is
written.

Sessions are meant for identity and small amounts of state, such as who's
signed in, which steps of a form have been completed, or what message to
show on the next page. If the contents are larger than `max_size` (65536
bytes by default), a `SessionError` is raised. If you need to store more
than that, put it in a database row and keep the row's key in the session.

### Sessions Across Workers

`http.serve()` runs each worker in its own isolate, so create the store
inside `setup` instead of passing it in from outside:

```zuri,ignore
# app.zu
import http
import http.session

def setup(server) {
  server.use(
    http.session({
      store: http.session.FileStore('/var/lib/app/sessions'),
      secret: os.get_env('SESSION_SECRET'),
    }),
  )

  server.get('/', @(request, response) {
    response.text(request.session().get('account', 'nobody'))
  })
}
```

A `sql` connection belongs to the isolate that opened it, so a worker that
uses `SqlStore` also opens its own pool in `setup`.

### Session Limitations

If two requests write the same session at the same time, which usually means
parallel requests from the same browser tab, both succeed, and whichever
finishes last wins. The file store writes by renaming a file, so a reader
never sees half a session, and the SQL store writes in a single statement,
but neither one takes a lock. Don't use a session as a counter that several
requests increment at the same time.

## TLS

```zuri,ignore
var server = http.server(443, '0.0.0.0')
server.load_certs('/etc/certs/site.crt', '/etc/certs/site.key')
server.listen()
```

Or from strings, which gives you more control:

```zuri,ignore
server.use_tls(cert_chain_pem, private_key_pem, {
  min_version: '1.2',
  client_ca: internal_ca_pem,
  require_client_cert: true,
})
```

`cert_chain` must contain the server certificate followed by any
intermediate certificates. Leaving out the intermediates is the most common
TLS mistake, and it only fails for clients that don't already have them
cached. That means it often looks fine from your own browser.

> **Note**
> The certificate and key are parsed when the first handshake happens, not
> when you set them, so an invalid or mismatched pair shows up as a failed
> connection rather than as an error from `use_tls()`. If you want to find
> out early, make a request to the server as part of starting it.

Turning on TLS also advertises `h2` through ALPN, so browsers get HTTP/2
without any other configuration.

## HTTP/2

You don't need to turn HTTP/2 on. `HttpServer` uses it when ALPN negotiates
`h2` over TLS, and when a client on an unencrypted connection starts with
the HTTP/2 connection preface. `HttpClient` uses it whenever a server
negotiates `h2` for an HTTPS connection. Routes, middleware and handlers are
the same code either way, and `request.version` is `'2'` when HTTP/2 is in
use.

With HTTP/2, a single connection carries every request, headers are
compressed with HPACK, and flow control is built in, instead of the six
parallel connections a browser has to open with HTTP/1.1.

To turn it off, set `server.http2 = false`, which also stops `h2` from being
advertised through ALPN.

`http.h2` provides the lower-level pieces, including the frame codec, HPACK
and its Huffman coding, for code that needs to work with the protocol
directly.

> **Note**
> Streams are multiplexed on the connection, but they're handled in the
> order they complete. Each connection is served by one isolate, so a
> request that arrives while another is being handled is read, buffered and
> answered next, not in parallel. This affects throughput, not correctness,
> and `http.serve()` is how you handle more than one request at a time.

## WebSockets

```zuri
import http.websocket as ws

server.get('/ws', @(request, response) {
  var socket = ws.accept(request, response, {
    protocols: ['chat'],
  })

  while socket.is_open() {
    var message = socket.receive()

    if message == nil or message.is_close() {
      break
    }

    socket.send('you said: ' + message.text())
  }
})
```

`accept()` completes the RFC 6455 handshake and takes over the connection,
so the server doesn't write anything else on it. Fragmented messages are
reassembled, ping frames are answered automatically, and when the other side
sends a close frame, it's answered and then returned to you so that you can
see the code and reason.

On the client side, use `ws.connect()`:

```zuri,ignore
var socket = ws.connect('wss://example.com/ws')

socket.send('hello')
echo socket.receive().text()
socket.close()
```

As the RFC requires, client frames are masked with a key from the platform's
secure random number generator. The mask is what stops a malicious page from
tricking a proxy into caching a forged response.

## Server-Sent Events

A WebSocket gives you a two-way connection. Server-sent events give you a
long-lived response body and nothing else, which is all you need for a live
feed of updates, and they work through proxies that would refuse a
connection upgrade.

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

A browser's `EventSource` reconnects automatically and sends the last event
ID it received. `sse.last_event_id(request)` reads it, so a stream can
continue where it left off instead of starting over.

## Reverse Proxying

```zuri
var api = http.ReverseProxy('http://127.0.0.1:9000', { strip_prefix: '/api' })

server.any('/api/' + '*path', @(request, response) {
  api.handle(request, response)
})
```

Hop-by-hop headers are removed in both directions, including any field named
in the message's own `Connection` header, which is how an endpoint declares
an additional hop-by-hop header. Passing a client's `Transfer-Encoding` on
to an upstream server that frames the message differently is the classic
setup for request smuggling, so this step can't be turned off.

`X-Forwarded-For`, `X-Forwarded-Proto`, `X-Forwarded-Host` and the RFC 7239
`Forwarded` header are added, and the response body is streamed instead of
buffered.

For several upstream servers, use a `LoadBalancer`. It uses round-robin, and
when an upstream fails, it's taken out of the rotation for a while instead
of being retried on every request:

```zuri
var pool = http.LoadBalancer(
  ['http://10.0.0.1:9000', 'http://10.0.0.2:9000'],
  { recovery_time: 30 },
)
```

## Running in Production

### Using More Than One Core

`listen()` serves every connection on the isolate that called it. It watches
all of the connections and only does the work for each one that can be done
without waiting, so a client that goes quiet, sends half a request or stops
reading doesn't hold up anyone else. A handler does run until it finishes,
though, so a slow handler delays every other connection, and a streamed body
or a WebSocket keeps the isolate busy until it's done.

`http.serve()` runs the same pipeline across a pool of isolates. A single
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

http.serve(app.setup, { host: '0.0.0.0', port: 3000, workers: 8 })
```

`setup` is called once inside each worker, with that worker's own
`HttpServer`. It can be defined in the main script or in a module, and it
can use anything it imports. A module it uses is loaded again inside the
worker, not shared with it. Once `setup` registers more than a few routes,
it's better to put it in its own module, as shown above.

Isolates don't share memory, so anything a worker needs, such as a cache, a
connection pool or a counter, exists separately in each worker. That's the
trade-off this model makes: no locks and no garbage collection pauses on a
shared heap, in exchange for state that has to be kept either per worker or
somewhere outside the process.

`backlog` limits how many accepted connections can wait for a free worker.
The limit is intentional. With an unlimited queue, a server under load would
accept connections faster than it could serve them and then answer all of
them late, instead of letting the kernel's own listen backlog apply
backpressure.

### Limits

Every part of a request whose size the client controls has a limit:

| Setting | Default | Limits |
| --- | --- | --- |
| `max_line_size` | 8 KiB | The request line, and each header line |
| `max_header_size` | 64 KiB | The header section |
| `max_header_count` | 100 | The number of header fields |
| `max_body_size` | 10 MiB | The request body |
| `header_timeout` | 10s | How long the client can take to send the whole request head |
| `keep_alive_timeout` | 5s | How long an idle connection is kept open |
| `max_keep_alive_requests` | 0 (unlimited) | How many requests a single connection can serve |
| `max_open_connections` | 1024 | How many connections `listen()` keeps open at once |

`header_timeout` is the easiest one to forget, and the most important.
Without it, a client that sends one byte every few seconds can keep a
connection open forever without ever exceeding the timeout of any single
read. That's how a slowloris attack works.

If a service accepts uploads, raise `max_body_size` deliberately, rather
than finding out about the limit by accident:

```zuri
server.max_body_size = 100 * 1024 * 1024
```

A request that declares a body larger than the limit is rejected with `413`
before any of the body is read, which is exactly what the header is for.

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

`stop()` closes the listener, which ends the accept loop. Each worker then
closes its connections, after giving any data already queued on each one a
chance to be sent, and then `serve()` returns. A handler can do the same by
calling `close()` on the server its worker was given:

```zuri,ignore
http.serve(@(server) {
  server.post('/shutdown', @(request, response) {
    response.text('stopping')
    server.close()
  })
}, { port: 3000 })
```

For a server running on a single isolate, `close()` ends the accept loop as
well, and `listen()` closes its remaining connections in the same way.

### Running Behind a Proxy

If there really is something in front of the server, such as a CDN or a load
balancer you control, tell the server about it, and tell it which addresses
to trust:

```zuri
server.trust_proxy = true
server.trusted_proxies = ['10.0.0.1', '10.0.0.2']
```

`request.client_ip(true, server.trusted_proxies)` then works through the
forwarding chain starting from the proxy's end, and returns the rightmost
address that isn't a trusted proxy. Taking the leftmost address instead,
which is a common shortcut, lets an attacker claim any client address they
like. That matters as soon as you use an address for rate limiting, an
allowlist or an audit log.

If you don't configure this, forwarding headers are ignored completely and
the connection's address is treated as the client's. That's the right
default, because those headers are ordinary request headers, which anyone
can fill with anything.

## Requests the Module Rejects

Part of this module's job is to reject certain requests. It's worth listing
them explicitly, because other implementations would accept each of these:

- A header field name with whitespace before the colon. Some parsers read
  `Content-Length : 5` as a length and others as an unknown field, and that
  disagreement can be used to smuggle a request.
- Two `Content-Length` fields with different values, for the same reason.
- A `Content-Length` together with a `Transfer-Encoding`.
- A `Transfer-Encoding` whose last coding isn't `chunked`.
- An obsolete folded header line. RFC 9112 deprecated them, and
  intermediaries unfold them in different ways.
- An HTTP/1.1 request with no `Host` header, or with two.
- A header value containing CR, LF or NUL, which is rejected at the point
  it's set. A value that can insert a newline can cause response splitting
  wherever it ends up.
- An uppercase header field name in HTTP/2, and any of the
  connection-specific fields there.
- A static file path that resolves to somewhere outside the root, before or
  after percent-decoding, and any dotfile.

## Module Reference

| Module | Contents |
| --- | --- |
| `http` | The main interface: `get()`, `post()`, `server()`, `client()`, `serve()` |
| `http.status` | Status codes, reason phrases and checks |
| `http.headers` | `Headers`, field validation and canonical names |
| `http.cookies` | `Cookie`, `CookieJar` and both cookie header formats |
| `http.session` | `Session`, `SessionStore`, `FileStore`, `MemoryStore` |
| `http.session.sql` | `SqlStore`, for sessions stored in a database |
| `http.request` | `HttpRequest`, and encoding and decoding query strings |
| `http.response` | `HttpResponse` |
| `http.router` | `Router`, `Route`, `RouteMatch` |
| `http.middleware` | CORS, logging, security headers, authentication, rate limiting |
| `http.files` | `StaticFiles`, byte ranges and validators |
| `http.multipart` | `MultipartBuilder`, `UploadedFile` and the parser |
| `http.negotiate` | Parsing and matching `Accept` headers |
| `http.body` | Message framing, chunked encoding and content codings |
| `http.h1` | The HTTP/1.1 wire format |
| `http.loop` | `ConnectionLoop`, for many connections on one thread |
| `http.h2` | HTTP/2: frames, HPACK and connections |
| `http.websocket` | RFC 6455, client and server |
| `http.sse` | Server-sent events |
| `http.proxy` | `ReverseProxy`, `LoadBalancer` |
| `http.stream` | The buffered connection that both protocols read from and write to |
| `http.util` | Dates, header parameters, percent-encoding and path normalization |
| `http.errors` | The error hierarchy |

Two other standard library modules work with this one without it depending
on either of them. `HttpRequest.validate()` accepts a schema from
`validate`, and `middleware.jwt_auth()` accepts a verifier from `jwt`. Both
rely on duck typing, so a server that doesn't use them never loads either
module.

Every error this module raises descends from `HttpError`:
`ProtocolError` for a malformed message, `ConnectionError` for a
connection that failed, `TimeoutError`, `TooLargeError`,
`TooManyRedirectsError`, `StatusError`, `UnsupportedProtocolError` and
`SessionError`.
