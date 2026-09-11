# Networking

The `net` module is the socket layer: TCP, UDP, TLS, DTLS, address parsing
and polling. The `http` module sits on top of it and gives you a client and
a server that speak HTTP/1.1 and HTTP/2.

Every example in this chapter runs a server in an isolate and a client in
the main program, which is how you test network code without two terminals.
[Chapter 11](ch11-00-isolates.md) is the background.

## TCP

A `TcpStream` is both ends. `bind()` and `accept()` make it a listener;
`connect()` makes it a client.

<span class="filename">Filename: server.zu</span>

```zuri
import net

def echo_server(port_channel) {
  var listener = net.TcpStream()
  listener.bind('127.0.0.1:0')

  port_channel.send(listener.local_address())

  var client = listener.accept()
  var received = client.read_as_string()

  client.write_all('echo: ' + received)
  client.close()
  listener.close()
}
```

<span class="filename">Filename: main.zu</span>

```zuri
import net
import isolate
import .server

var channel = isolate.channel(1)
var task = isolate.spawn(server.echo_server, channel)
var address = channel.recv()

var client = net.TcpStream()
client.connect(address)

client.write_all('hello')
client.shutdown(net.Shutdown.WRITE)

echo client.read_as_string()

client.close()
task.join()
```

```console
echo: hello
```

Binding to port `0` asks the operating system for a free port, and
`local_address()` tells you which one you got. That is the right way to
write a test, and the right way to run several servers in one process.

The `shutdown(net.Shutdown.WRITE)` matters. `read_as_string()` reads until
the peer stops writing, so without a half-close the two ends wait for each
other forever. `Shutdown` has `READ`, `WRITE` and `BOTH`.

### Reading

| Method | Behaviour |
| --- | --- |
| `read(length)` | up to `length` bytes, whatever has arrived |
| `read_exact(length)` | exactly `length` bytes, waiting for them |
| `read_all()` | everything until the peer closes, as `bytes` |
| `read_as_string()` | the same, decoded as UTF-8 |
| `peek(length)` | look without consuming |

`read()` returning fewer bytes than you asked for is normal, not an error.
That is what `read_exact()` is for.

### Writing

`write(data)` writes what it can and tells you how much. `write_all(data)`
loops until everything is out, and is what you want almost always.

### Options

```zuri
socket.set_read_timeout(5000)
socket.set_write_timeout(5000)
socket.set_nodelay(true)
socket.set_non_blocking(true)
socket.set_ttl(64)
```

Timeouts are milliseconds. **Set a read timeout on anything that talks to
the network.** A socket with no timeout and a peer that never answers is a
thread that never comes back.

`set_nodelay(true)` disables Nagle's algorithm, which is what you want for
a request/response protocol where latency matters more than packet count.

## UDP

```zuri
import net

var a = net.UdpSocket()
a.bind('127.0.0.1:0')

var b = net.UdpSocket()
b.bind('127.0.0.1:0')
b.set_read_timeout(2000)

a.send_to('ping', b.local_address())

var pending = b.peek_from(1024)
echo pending.data.to_string()
echo pending.address.to_string()

echo b.receive_from(1024).to_string()

b.send_to('pong', pending.address)

a.set_read_timeout(2000)
echo a.receive_from(1024).to_string()
```

```console
ping
127.0.0.1:54611
ping
pong
```

`receive_from(length)` gives you the datagram's bytes. To learn who sent
it, `peek_from(length)` returns `{ data, address }` without consuming the
datagram, so the pattern for a responder is peek, read, reply.

A datagram larger than the buffer you give is truncated, and the rest is
discarded. Size the buffer for your protocol's largest message.

There is also `connect()` to fix a peer, `set_broadcast()`, and
`join_multicast_v4()` / `join_multicast_v6()`.

## Addresses

```zuri
import net

var address = net.SocketAddr.parse('127.0.0.1:8080')

echo address.to_string()
echo net.resolve('localhost:80').map(@(a) => a.to_string())
```

```console
127.0.0.1:8080
[127.0.0.1:80]
```

`resolve()` returns a list, because a name can have several addresses.

`net.ip` has `IpAddress`, `Ipv4Address` and `Ipv6Address` for parsing,
comparing and classifying addresses, which is what you want before you
trust an `X-Forwarded-For` header.

## TLS

`net.tls` wraps an established TCP stream:

```zuri
import net
import net.tls

var config = tls.TlsConfig()
config.set_cert_chain(certificate_pem, private_key_pem)

var listener = net.TcpStream()
listener.bind('127.0.0.1:0')

var client = listener.accept()
var secure = tls.TlsStream.accept(client, config)

echo secure.read_as_string()
```

A `TlsStream` has the same read and write interface a `TcpStream` does, so
code written against one works against the other.

On the client side, `tls.TlsStream.connect(socket, config, hostname)`
performs the handshake and verifies the certificate. `config.add_ca_pem()`
adds a trust anchor, and `config.require_client_cert(true)` turns on mutual
TLS. `peer_certificate()` gives you the other end's certificate once the
handshake is done.

`net.dtls` is the same thing over UDP.

## Polling

`net.poll` waits on many sockets at once without a thread each. It is the
right tool when you have thousands of mostly-idle connections and the wrong
tool when you have a handful of busy ones, where an isolate per connection
is simpler and faster.

## HTTP

### The Client

```zuri
import http

var response = http.get('https://example.com')

echo response.status
echo response.as_text()
```

The module-level `get`, `post`, `put`, `patch`, `delete`, `head`,
`options` and `trace` use a shared client. For anything more than a
one-off, make your own with a base URL:

```zuri
var client = http.client('http://127.0.0.1:8080')

var r = client.get('/tasks/7')
echo r.status
echo r.as_dict()
```

A client keeps connections alive between requests, so reusing one is
meaningfully faster than calling the module functions in a loop.

A response carries:

| Member | What it is |
| --- | --- |
| `status` | the numeric status code |
| `headers` | a `Headers` object |
| `body` | the raw `bytes` |
| `as_text()` | the body decoded |
| `as_dict()` | the body parsed as JSON |
| `as_bytes()` | the body as `bytes` |
| `is_ok()`, `is_redirect()`, `is_error()` | status class tests |
| `raise_for_status()` | raise unless the status is a success |

Posting JSON is just posting a dictionary:

```zuri
var r = client.post('/tasks', { title: 'write chapter 12' })
echo r.status
echo r.as_dict()
```

```console
201
{created: write chapter 12}
```

### The Server

```zuri
import http

var server = http.server(8000, '127.0.0.1')

server.get('/', @(request, response) {
  response.text('hello world')
})

server.get('/tasks/:id', @(request, response) {
  response.json({ id: request.param('id') })
})

server.post('/tasks', @(request, response) {
  response.json({ created: request.json_body().title }, 201)
})

server.listen()
```

`get`, `post`, `put`, `patch`, `delete`, `head` and `options` register a
route. A path segment starting with `:` is a named parameter, read with
`request.param()`. A trailing `*name` captures the rest of the path.

`listen()` binds and serves until the server is closed.

### The Request

| Member | What it is |
| --- | --- |
| `method`, `path`, `query_string`, `version` | the request line |
| `headers` | a `Headers` object |
| `body` | the raw `bytes` |
| `param(name, fallback)` | a route parameter |
| `query_param(name, fallback)` | a query-string parameter |
| `cookie(name, fallback)` | a cookie |
| `text()` | the body as a string |
| `json_body()` | the body parsed as JSON |
| `form()`, `files()` | a parsed form submission |
| `client_ip(trust_proxy)` | the peer address |
| `wants_json()`, `accepts(type)` | content negotiation |

### The Response

`text()`, `html()`, `json()`, `xml()`, `file()`, `download()` and
`render()` each set a content type and a body. Every one takes an optional
status:

```zuri
response.json({ error: 'not found' }, 404)
```

`render(path, variables)` renders a Wire template, which is what the
capstone uses. `stream(handler)` streams a response body in chunks, and
`redirect(location, status)` does what it says.

Headers and cookies:

```zuri
response.header('X-Request-Id', id)
response.set_cookie('session', token, { http_only: true, max_age: 3600 })
response.cache_for(300)
response.no_cache()
```

### Serving Across Cores

`listen()` serves on one thread. `http.serve()` runs a pool of isolates,
one per core by default:

<span class="filename">Filename: app.zu</span>

```zuri
import http

def setup(server) {
  server.get('/', @(request, response) {
    response.text('hello from a worker')
  })
}
```

<span class="filename">Filename: main.zu</span>

```zuri
import http
import .app

http.serve(app.setup, { port: 8000, workers: 4 })
```

`setup` is called once inside each worker with that worker's own server.
It must be a function defined in a module, for the same reason every
spawned function must be: an isolate resolves a function by module binding.

The options are `port`, `host`, `workers`, `backlog`, and
`cert_chain`/`private_key` for TLS.

### Serving Manually

When you need the port before the first connection, or want to stop the
loop from inside a handler, run the accept loop yourself:

```zuri
server.bind()
echo server.address().to_string()

while server.is_listening() {
  var client

  catch {
    client = server.accept()
  } as e

  if e {
    break
  }

  server.serve_connection(client)
}
```

That is exactly what `listen()` does, and it is what the tests in this
repository use so a server and a client can talk inside one process.

### The Rest of the Module

`http.middleware` has logging, CORS, compression, rate limiting and
authentication. `http.websocket` upgrades a connection to a WebSocket.
`http.sse` sends server-sent events. `http.proxy` has a reverse proxy and a
load balancer. `http.files` serves a directory with conditional and range
requests. `http.h2` is HTTP/2, negotiated over TLS automatically.

The capstone in [Chapter 20](ch20-00-task-board.md) puts routing,
middleware, JSON, templates and static files together into one application.
