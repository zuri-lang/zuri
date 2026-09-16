# Networking

The `net` module is the socket layer: TCP, UDP, unix domain sockets, TLS,
DTLS, address parsing and polling. The `http` module sits on top of it and gives you a client and
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

```zuri,ignore
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

Three things in that pair of files are doing real work.

**The server lives in its own file.** It has to: `echo_server` refers to
`net`, and a function that references an imported module cannot be spawned
from the file that did the importing. In `server.zu`, `net` is resolved
inside the isolate instead of being captured from the caller.
[Chapter 11](ch11-00-isolates.md) covers the rule and the alternative.

**Binding to port `0`** asks the operating system for a free port, and
`local_address()` reports which one it gave. That is the right way to write
a test, and the right way to run several servers in one process. The port
travels back to the main program through the channel, because the main
program cannot know it in advance.

**`shutdown(net.Shutdown.WRITE)` is not optional here.**
`read_as_string()` reads until the peer stops writing, so without a
half-close from the client the server would still be waiting for more
request while the client waits for a response — a deadlock that looks
exactly like a hang. `Shutdown` has `READ`, `WRITE` and `BOTH`.

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

```zuri,ignore
socket.set_read_timeout(5000)
socket.set_write_timeout(5000)
socket.set_nodelay(true)
socket.set_non_blocking(true)
socket.set_ttl(64)
```

Timeouts here are **milliseconds**, which is the opposite of the `isolate`
module's seconds — easy to mix up in a program that uses both.

**Set a read timeout on anything that talks to the network.** A socket
with no timeout and a peer that never answers is a thread that never
comes back.

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
127.0.0.1:<port>
ping
pong
```

The second line is written with a placeholder because the real one is not
predictable: `bind('127.0.0.1:0')` asks the operating system for any free
port, and it picks a different one every run.

`receive_from(length)` gives you the datagram's bytes. To learn who sent
it, `peek_from(length)` returns `{ data, address }` without consuming the
datagram, so the pattern for a responder is peek, read, reply.

A datagram larger than the buffer you give is truncated, and the rest is
discarded. Size the buffer for your protocol's largest message.

There is also `connect()` to fix a peer, `set_broadcast()`, and
`join_multicast_v4()` / `join_multicast_v6()`.

## Unix Sockets

A unix domain socket is a path on the filesystem rather than an address
on the network. It is how two programs on one machine usually talk when
one of them is a server: databases, message brokers and system daemons
nearly all listen on one.

There is no port to collide with and nothing to reach it from another
host. And because the socket is a file, the filesystem decides who may
connect — a socket in a directory only one user can enter is reachable
by only that user, which is a stronger answer than binding to loopback
and trusting everyone on the machine.

`pair()` gives two sockets already connected to each other, with no
path and nothing left on disk:

```zuri
import net

var ends = net.pair()

ends[0].write_all('ping')
echo ends[1].read_exact(4)

ends[1].write_all('pong')
echo ends[0].read_exact(4).to_string()

ends[0].close()
ends[1].close()
```

```console
(70 69 6e 67)
pong
```

A server binds a path and accepts on it, exactly as a `TcpStream`
binds an address:

```zuri,ignore
import net

var server = net.UnixStream()
server.bind('/run/app.sock')

while true {
  var client = server.accept()
  client.write_all('hello\n')
  client.close()
}
```

Two things differ from TCP and both come from the socket being a file.
`bind()` refuses a path that already exists, including one a crashed
process left behind, so a server that expects to be restarted deletes
a stale path first. And closing frees the descriptor without removing
the file, because by then another process may have bound the same path
and removing it would break them.

`net.is_supported()` is false on Windows, where these are not
available. Every call raises there rather than the module being
missing, so a program that can fall back to TCP tests it rather than
catching an error.

## Addresses

```zuri
import net

var address = net.SocketAddr.parse('127.0.0.1:8080')

echo address.to_string()
```

```console
127.0.0.1:8080
```

`resolve()` turns a name into addresses:

```zuri,ignore
echo net.resolve('localhost:80').map(@(a) => a.to_string())
```

```console
[[::1]:80, 127.0.0.1:80]
```

It returns a list because a name can have several addresses, and what comes
back depends on the machine: a host with IPv6 configured answers for both
families, one without gives only `[127.0.0.1:80]`. Never assume a position
in that list, and never assume a family.

`net.ip` has `IpAddress`, `Ipv4Address` and `Ipv6Address` for parsing,
comparing and classifying addresses, which is what you want before you
trust an `X-Forwarded-For` header.

## TLS

`net.tls` wraps an established TCP stream:

```zuri,ignore
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

## Above the Socket Layer

Everything so far has been bytes on a socket. Most programs want a protocol
on top of that, and the standard library brings two of them.

**`http`** is a complete HTTP/1.1 and HTTP/2 client and server: routing,
middleware, cookies, multipart uploads, static files, server-sent events
and WebSockets. It is large enough to have its own chapter, and
[Chapter 15](ch15-00-http.md) is it. The one-line version:

```zuri,ignore
import http

var response = http.get('https://example.com')

echo response.status
echo response.as_text()
```

**`net.tls`** wraps a `TcpStream` in TLS, as shown above, for protocols
that are not HTTP — a mail client, a database driver, a custom binary
protocol.

A rough guide to which layer you want:

| You are writing | Reach for |
| --- | --- |
| a web API, or a client for one | `http` |
| a browser-facing server | `http` |
| a client for an existing non-HTTP protocol | `net.tcp`, plus `net.tls` if it is encrypted |
| a protocol of your own design | `net.tcp` and `struct` |
| discovery, telemetry, games | `net.udp` |
| anything waiting on many sockets at once | `net.poll` |

## A Worked Example

A length-prefixed request/response protocol of the kind `net` is for: each
message is a four-byte big-endian length followed by that many bytes of
JSON. This is the pattern behind most binary protocols, and it is worth
writing once by hand.

<span class="filename">Filename: framed.zu</span>

```zuri,ignore
import struct

# Reads one frame, or returns nil once the peer has hung up.
#
# read_exact() raises rather than returning short when the stream ends
# mid-read, and a clean disconnect between frames looks exactly like
# that, so the end of the conversation arrives here as an error.
def read_frame(stream) {
  var header

  catch {
    header = stream.read_exact(4)
  } as e {
    return nil
  }

  var length = struct.unpack('N:size', header).size

  return stream.read_exact(length).to_string()
}

def write_frame(stream, payload) {
  stream.write_all(struct.pack('N', payload.length()))
  stream.write_all(payload)
}
```

<span class="filename">Filename: server.zu</span>

```zuri,ignore
import net
import json
import .framed

def serve(port_channel) {
  var listener = net.TcpStream()
  listener.bind('127.0.0.1:0')

  port_channel.send(listener.local_address())

  var client = listener.accept()

  while true {
    var request = framed.read_frame(client)

    if request == nil {
      break
    }

    var parsed = json.decode(request)

    framed.write_frame(client, json.encode({ reply: parsed.message }))
  }

  client.close()
  listener.close()
}
```

<span class="filename">Filename: main.zu</span>

```zuri,ignore
import net
import json
import isolate
import .framed
import .server

var channel = isolate.channel(1)
var task = isolate.spawn(server.serve, channel)

var client = net.TcpStream()
client.connect(channel.recv())
client.set_read_timeout(2000)

framed.write_frame(client, json.encode({ message: 'first' }))
echo framed.read_frame(client)

framed.write_frame(client, json.encode({ message: 'second' }))
echo framed.read_frame(client)

client.close()
task.join()
```

```console
{"reply":"first"}
{"reply":"second"}
```

The framing is the whole point. TCP is a stream of bytes with no message
boundaries in it: one `write_all()` may arrive as three reads, and three
writes may arrive as one. `read_exact(4)` followed by `read_exact(length)`
is what puts the boundaries back, and `read()` alone would not — it returns
whatever has arrived, which is why the table above distinguishes the two.

The end of the conversation is the other thing worth studying.
`read_exact()` **raises** when the stream ends before it has the bytes it
was promised, and a peer that hangs up cleanly between frames produces
exactly that. Catching it and returning `nil` is what turns "the connection
closed" from a crash into the loop's normal exit.

Note also that both ends share `framed.zu`. A protocol implemented twice,
once per end, is a protocol that will eventually disagree with itself.

One last detail, easily missed: the reply key is `reply`, not `echo`.
`echo` is a keyword, so it cannot be a bare dictionary key — `{ echo: x }`
is a syntax error. Quote it as `{ 'echo': x }` if you need that exact
name.

## The Rest of the Module

`net.poll` answers "which of these sockets can I read right now?" without a
thread per socket, which is how you serve many connections from one
isolate. It takes a `UnixStream` alongside a `TcpStream` or a `TlsStream`. `net.addr` and `net.ip` parse, format and classify addresses —
`is_private()`, `is_loopback()`, `is_multicast()` and the rest — which is
what you want before trusting an address a client sent you. `net.dtls` is
TLS over UDP.

[Appendix F](appendix-06-stdlib-index.md) lists every submodule.
