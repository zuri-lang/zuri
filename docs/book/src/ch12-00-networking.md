# Networking

The `net` module is Zuri's socket layer. It covers TCP, UDP, Unix domain
sockets, TLS, DTLS, address parsing and polling. The `http` module is built
on top of it and provides a client and a server for HTTP/1.1 and HTTP/2.

Every example in this chapter runs a server in an isolate and a client in
the main program, which lets you test network code without needing two
terminals. [Chapter 11](ch11-00-isolates.md) explains isolates.

## TCP

A `TcpStream` can be either end of a connection. `bind()` and `accept()`
make it a listener, and `connect()` makes it a client. `accept()` blocks
until a client connects, unless the listener is in non-blocking mode, in
which case it returns `nil` when nobody is waiting.

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

There are three important things in this pair of files.

**The server is in its own file.** It has to be, because `echo_server` uses
`net`, and a function that uses an imported module can't be spawned from the
file that imported it. In `server.zu`, `net` is resolved inside the isolate
instead of being captured from the caller. [Chapter 11](ch11-00-isolates.md)
explains this rule and the alternative.

**Binding to port `0`** asks the operating system for a free port, and
`local_address()` tells you which one it chose. That's the right way to
write a test, and the right way to run several servers in one process. The
port is sent back to the main program through the channel, since the main
program can't know it in advance.

**The `shutdown(net.Shutdown.WRITE)` call is required here.**
`read_as_string()` keeps reading until the other side stops writing. Without
a half-close from the client, the server would keep waiting for more of the
request while the client waits for the response. That's a deadlock, and it
looks just like a hang. `Shutdown` has `READ`, `WRITE` and `BOTH`.

### Reading From a Socket

| Method | Behavior |
| --- | --- |
| `read(length)` | Reads up to `length` bytes, whatever has arrived so far |
| `read_exact(length)` | Reads exactly `length` bytes, waiting for them if necessary |
| `read_all()` | Reads everything until the other side closes, as `bytes` |
| `read_as_string()` | The same, decoded as UTF-8 |
| `peek(length)` | Looks at incoming bytes without consuming them |

It's normal for `read()` to return fewer bytes than you asked for, and it
isn't an error. That's what `read_exact()` is for.

### Writing to a Socket

`write(data)` writes as much as it can and tells you how much it wrote.
`write_all(data)` keeps writing until everything has been sent, and it's
almost always the one you want.

### Socket Options

```zuri,ignore
socket.set_read_timeout(5000)
socket.set_write_timeout(5000)
socket.set_nodelay(true)
socket.set_non_blocking(true)
socket.set_ttl(64)
```

Timeouts in this module are in **milliseconds**, unlike the `isolate`
module, which uses seconds. It's easy to mix them up in a program that uses
both.

**Set a read timeout on every socket that talks to the network.** If a
socket has no timeout and the other side never responds, the thread using it
never gets control back.

`set_nodelay(true)` turns off Nagle's algorithm, which is what you want for
a request and response protocol where latency matters more than the number
of packets.

`set_non_blocking(true)` makes a read or write that can't proceed fail
instead of waiting. For a non-blocking socket watched with a `net.Poller`,
`try_read(length)` and `try_write(data)` report this as a result instead of
an error. `try_read()` returns `nil` when nothing has arrived, and
`try_write()` returns how many bytes the socket accepted, which is `0` when
it has no room. `TlsStream` has both methods, plus `try_flush()` for data it
has encrypted but not yet sent.

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

The second line uses a placeholder because the real value is different every
time. `bind('127.0.0.1:0')` asks the operating system for any free port, and
it picks a different one each run.

`receive_from(length)` returns the bytes of the datagram. To find out who
sent it, use `peek_from(length)`, which returns `{ data, address }` without
consuming the datagram. So a responder peeks, reads and then replies.

If a datagram is larger than the buffer you provide, it's truncated and the
rest is discarded. Make the buffer large enough for the largest message your
protocol sends.

UDP sockets also have `connect()` to set a fixed peer, `set_broadcast()`,
and `join_multicast_v4()` and `join_multicast_v6()`.

## Unix Domain Sockets

A Unix domain socket is identified by a path on the file system instead of
an address on the network. It's the usual way for two programs on the same
machine to communicate when one of them is a server. Databases, message
brokers and system daemons nearly all listen on one.

There's no port that can clash with another program, and no way to reach the
socket from another machine. And because the socket is a file, the file
system decides who can connect. A socket inside a directory that only one
user can access can only be reached by that user, which is more secure than
binding to the loopback address and trusting everyone on the machine.

`pair()` returns two sockets that are already connected to each other, with
no path and nothing left on disk:

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

A server binds to a path and accepts connections on it, just as a
`TcpStream` binds to an address:

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

There are two differences from TCP, and both are because the socket is a
file. First, `bind()` fails if the path already exists, even if it was left
behind by a process that crashed, so a server that expects to be restarted
should delete a stale path first. Second, closing the socket frees the file
descriptor but doesn't delete the file, because by then another process
might have bound the same path, and deleting it would break that process.

`net.is_supported()` returns false on Windows, where Unix domain sockets
aren't available. On Windows, every call raises an error, but the module
itself still exists, so a program that can fall back to TCP should check
`is_supported()` rather than catch an error.

## Addresses

```zuri
import net

var address = net.SocketAddr.parse('127.0.0.1:8080')

echo address.to_string()
```

```console
127.0.0.1:8080
```

`resolve()` converts a host name into addresses:

```zuri,ignore
echo net.resolve('localhost:80').map(@(a) => a.to_string())
```

```console
[[::1]:80, 127.0.0.1:80]
```

It returns a list because a name can have several addresses, and the result
depends on the machine. A host with IPv6 configured returns addresses for
both families, while one without IPv6 returns only `[127.0.0.1:80]`. Don't
assume that an address is at a particular position in the list, or that it
belongs to a particular family.

`net.ip` provides `IpAddress`, `Ipv4Address` and `Ipv6Address` for parsing,
comparing and classifying addresses, which you'll need before you trust an
`X-Forwarded-For` header.

## TLS

`net.tls` wraps an existing TCP connection in TLS:

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

A `TlsStream` has the same methods for reading and writing as a `TcpStream`,
so code written for one works with the other.

On the client side, `tls.TlsStream.connect(socket, config, hostname)`
performs the handshake and verifies the server's certificate.
`config.add_ca_pem()` adds a trusted certificate authority, and
`config.require_client_cert(true)` turns on mutual TLS. Once the handshake
has finished, `peer_certificate()` returns the other side's certificate.

`net.dtls` provides the same thing over UDP.

## Polling

`net.poll` waits on many sockets at once without needing a thread for each
one. It's the right tool when you have thousands of connections that are
mostly idle, and the wrong tool when you have just a few busy ones. In that
case, using an isolate per connection is simpler and faster.

`net.Poller` uses epoll on Linux, kqueue on macOS and FreeBSD, `WSAPoll` on
Windows and `poll` on other systems. With epoll and kqueue, the time
`wait()` takes doesn't grow with the number of registered sockets. With
`WSAPoll` and `poll`, each call checks all of them.

If a registered socket is closed or sent to another isolate, `wait()`
reports `net.ERROR` for its token until you call `remove()`. A new socket
that's given the same file descriptor is reported under its own token.

### Accepting Connections Without Blocking the Thread

`accept()` on a blocking listener waits inside the runtime, and nothing else
on that thread can run while it waits. A signal handled with
`os.on_signal()` isn't delivered, and a flag telling the server to stop
isn't checked, until a connection happens to arrive. On an idle server, that
might never happen, which is why pressing Ctrl+C on one can seem to do
nothing.

`net.Acceptor` is an accept loop that doesn't have this problem. It switches
the listener to non-blocking mode and waits on a poller instead. `next()`
returns a connection when there is one, and returns `nil` when its interval
passes without a connection arriving. That gives the loop a chance to check
anything else it needs to.

```zuri
import net

var listener = net.TcpStream()
listener.bind('127.0.0.1:0')

var acceptor = net.Acceptor(listener, 50)

# Nobody has connected yet, so the round comes back empty.
echo acceptor.next()

var client = net.TcpStream()
client.connect(listener.local_address())

var served = acceptor.wait_for_one()

echo served.peer_address().ip().to_string()

served.close()
client.close()
listener.close()
```

```console
nil
127.0.0.1
```

The interval only affects how quickly a stopped server notices that it
should stop, not how quickly a connection is handled, because a new
connection ends the wait immediately. It doesn't add any delay while
connections are arriving either, because `next()` tries `accept()` first and
only uses the poller when there's nothing waiting.

This makes a server that you can stop with Ctrl+C:

```zuri,ignore
import net
import os

var listener = net.TcpStream()
listener.bind('127.0.0.1:8080')

var running = true

os.on_signal('INT', @() {
  running = false
  return true
})

var acceptor = net.Acceptor(listener)

while running {
  var client = acceptor.next()

  if client == nil {
    continue
  }

  serve(client)
}

listener.close()
```

The handler returns `true` to show that it has handled the signal. If a
handler returns a falsy value, it declines the signal, and the process is
terminated by the signal as it would be if no handler were registered.

`http` accepts connections this way in both `HttpServer.listen()` and
`http.serve()`, and so do the SMTP and IMAP servers in `mail`. A server
built on any of these can already be stopped with Ctrl+C.

## Above the Socket Layer

So far, everything has been about sending bytes over a socket. Most programs
need a protocol on top of that, and the standard library provides two.

**`http`** is a complete HTTP/1.1 and HTTP/2 client and server, with
routing, middleware, cookies, multipart uploads, static files, server-sent
events and WebSockets. It's big enough to need its own chapter, which is
[Chapter 15](ch15-00-http.md). Here's the shortest possible example:

```zuri,ignore
import http

var response = http.get('https://example.com')

echo response.status
echo response.as_text()
```

**`net.tls`** wraps a `TcpStream` in TLS, as shown above, for protocols
other than HTTP, such as a mail client, a database driver or a custom binary
protocol.

Here's a rough guide to which layer to use:

| If you're writing | Use |
| --- | --- |
| A web API, or a client for one | `http` |
| A server that browsers connect to | `http` |
| A client for an existing protocol that isn't HTTP | `net.tcp`, plus `net.tls` if the protocol is encrypted |
| A protocol of your own | `net.tcp` and `struct` |
| Service discovery, telemetry or games | `net.udp` |
| Anything that waits on many sockets at once | `net.poll` |
| A server that has to stop on Ctrl+C | `net.Acceptor` |

## Example: A Length-Prefixed Protocol

This example implements a request and response protocol of the kind `net` is
designed for. Each message is a four-byte big-endian length, followed by
that many bytes of JSON. Most binary protocols follow this pattern, and it's
worth writing one by hand at least once.

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

The framing is the important part. TCP is a stream of bytes with no message
boundaries. One `write_all()` might arrive as three reads, and three writes
might arrive as one. Calling `read_exact(4)` and then `read_exact(length)`
restores the boundaries, which `read()` on its own wouldn't do, because it
returns whatever has arrived. That's why the table above lists both methods.

The end of the conversation is also worth looking at. `read_exact()`
**raises an error** when the stream ends before all of the expected bytes
have arrived, and that's exactly what happens when the other side closes the
connection cleanly between messages. Catching the error and returning `nil`
turns a closed connection into the loop's normal exit, rather than a crash.

Notice also that both sides share `framed.zu`. If you implement a protocol
twice, once for each side, the two implementations will eventually disagree.

One last detail that's easy to miss: the reply key is `reply`, not `echo`.
`echo` is a keyword, so it can't be used as a bare dictionary key, and
`{ echo: x }` is a syntax error. If you need that exact name, quote it:
`{ 'echo': x }`.

## Other Submodules

`net.poll` tells you which sockets you can read from right now without a
thread for each socket, which is how one isolate can serve many connections.
It accepts a `UnixStream` as well as a `TcpStream` or a `TlsStream`.
`net.addr` and `net.ip` parse, format and classify addresses with methods
such as `is_private()`, `is_loopback()` and `is_multicast()`, which you'll
need before trusting an address a client sent you. `is_global()` answers the
question a server should ask before connecting to an address provided by
someone else, such as a URL to fetch: is the address on the public internet,
or in one of the ranges that the IANA registries reserve, from loopback and
private ranges to carrier-grade NAT and documentation prefixes? `net.dtls`
is TLS over UDP.

[Appendix F](appendix-06-stdlib-index.md) lists all of the submodules.
