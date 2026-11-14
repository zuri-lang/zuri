# JSON-RPC

The `rpc` module is JSON-RPC 2.0, on both ends and over everything it
travels on: as the body of an HTTP request, over a WebSocket, a TCP or
TLS socket, a Unix domain socket, a pipe to a child process, or any
other stream of bytes. It is written in Zuri on top of `json`, `http`,
`isolate` and `net`, and it holds to the specification exactly.

JSON-RPC is a remote procedure call protocol, and a small one. One
program sends the name of a method and its parameters; the other runs
it and sends back the result or an error. That is very nearly all of
it, and the smallness is the point. The protocol says nothing about
what the methods are, how the two programs are connected, or which of
them is in charge, so it fits wherever two programs need to call each
other. Services call each other with it over HTTP. Blockchain nodes
publish their whole API through it, over HTTP for calls and over
WebSockets for what they push to subscribers. Mining pools, wallets,
trading systems, editors and their tools, and plugin hosts speak it
over sockets and pipes.

The module is built in the same spirit. A service answers messages and
nothing else, so the same service answers whatever carried the message
there; an HTTP route, a WebSocket and a socket server are each a few
lines around it.

- [Following Along](#following-along)
- [Introduction](#introduction)
- [Messages](#messages)
  - [Requests and Notifications](#requests-and-notifications)
  - [Responses](#responses)
  - [Errors and Their Codes](#errors-and-their-codes)
  - [Reading and Writing JSON](#reading-and-writing-json)
  - [Batches](#batches)
- [Services](#services)
  - [Answering Requests](#answering-requests)
  - [Failing Properly](#failing-properly)
  - [Notifications](#notifications)
  - [Methods Without a Handler](#methods-without-a-handler)
  - [Batches and Their Limit](#batches-and-their-limit)
- [JSON-RPC Over HTTP](#json-rpc-over-http)
  - [Serving a Service](#serving-a-service)
  - [Calling a Service](#calling-a-service)
  - [Statuses](#statuses)
  - [Who Is Calling](#who-is-calling)
- [Framing](#framing)
  - [Content-Length Headers](#content-length-headers)
  - [One Message to a Line](#one-message-to-a-line)
  - [One Message to Each Read](#one-message-to-each-read)
  - [Choosing a Framing](#choosing-a-framing)
  - [Limits](#limits)
- [Transports](#transports)
  - [Standard Streams](#standard-streams)
  - [Sockets](#sockets)
  - [Child Processes](#child-processes)
  - [WebSockets](#websockets)
  - [Between Isolates](#between-isolates)
  - [A Transport of Your Own](#a-transport-of-your-own)
- [Endpoints](#endpoints)
  - [Calling the Other Side](#calling-the-other-side)
  - [Sending a Batch](#sending-a-batch)
  - [Answering Later](#answering-later)
  - [Calls in Both Directions](#calls-in-both-directions)
- [Reading the Connection](#reading-the-connection)
  - [Serving](#serving)
  - [Listening While Doing Other Work](#listening-while-doing-other-work)
  - [Reading It Yourself](#reading-it-yourself)
  - [Timeouts](#timeouts)
  - [Stopping and Closing](#stopping-and-closing)
- [Serving Many Connections](#serving-many-connections)
- [Watching the Conversation](#watching-the-conversation)
- [Errors](#errors)
- [What the Module Refuses](#what-the-module-refuses)
- [Module Reference](#module-reference)

## Following Along

A service says which methods it answers, and answers one message at a
time:

```zuri
import rpc

var calculator = rpc.Service().on_request(
  'add',
  @(params) => params[0] + params[1],
)

echo calculator.answer(
  '{"jsonrpc": "2.0", "id": 1, "method": "add", "params": [2, 3]}',
)
```

```console
{"jsonrpc":"2.0","id":1,"result":5}
```

The message went in as JSON text and the answer came out the same way.
Nothing touched a network. Everything else in this chapter is about
how the message gets to a service and how its answer gets back: as the
body of an HTTP request, over a WebSocket or a socket, or between two
isolates of one program.

## Introduction

JSON-RPC has three kinds of message, each a small JSON object whose
`jsonrpc` member is `"2.0"`.

A **request** names a `method`, carries its `params`, and carries an
`id`. The other side must answer it, and the answer carries the same
`id`.

A **notification** is a request without an `id`. Nothing answers it,
not even when it fails. It is for things the sender wants done but has
no need to hear back about: a log line, a progress report, a change
the other side should know of.

A **response** carries the `id` of the request it answers, and exactly
one of `result` and `error`.

Parameters are either a list, matched to the method's parameters by
position, or a dictionary, matched by name. Nothing else is allowed: a
call to `square(4)` sends `[4]`, never `4`.

Messages can also travel together as a **batch**, a JSON array of
them. The requests in a batch are answered together, as an array of
responses, each matched to its request by `id`.

The protocol has no transport of its own, and two kinds carry it. In
an **exchange**, such as an HTTP request and its response, one message
or batch goes each way and that is the end of it. On a **connection**,
such as a WebSocket or a socket, messages flow both ways for as long as
it stays open, and neither side is the client: either may send
requests and notifications at any time, including while it is in the
middle of answering one. On a stream of bytes, both ends must also
agree on where one message stops and the next begins. That agreement
is the framing.

The module has a layer for each of these:

| Layer | What it holds |
| --- | --- |
| messages | `Request`, `Notification`, `Response`, `RpcError`, and `encode()`, `decode()` and `read()` |
| services | `Service`, and the `Context` its handlers are given |
| HTTP | `http_handler()` to serve a service, and `HttpClient` to call one |
| framing | `HeaderFraming`, `LineFraming` and `MessageFraming` |
| transports | `StdioTransport`, `SocketTransport`, `ProcessTransport`, `WebSocketTransport`, `ChannelTransport` and `pipe()` |
| endpoints | `Endpoint`, a service on a connection |
| servers | `serve()`, an endpoint for every connection to a listening socket |

`import rpc` reaches all of it.

## Messages

### Requests and Notifications

A message is built from what it says, and `rpc.encode()` writes it as
JSON-RPC sends it:

```zuri
import rpc

var call = rpc.Request('add', [2, 3], 1)
var note = rpc.Notification('log', { level: 'info', text: 'started' })

echo call
echo rpc.encode(call)
echo rpc.encode(note)
```

```console
Request(add #1)
{"jsonrpc":"2.0","id":1,"method":"add","params":[2,3]}
{"jsonrpc":"2.0","method":"log","params":{"level":"info","text":"started"}}
```

An `id` is a string or a number, and is whatever the sender chooses to
match the answer by. An endpoint numbers its own requests from `1`.

`params` of `nil` leaves the member out of the message, which the
specification allows for a method that takes nothing. Anything other
than a list, a dictionary or `nil` is refused when the message is
built, before it can be sent:

```zuri
import rpc

catch {
  rpc.Request('square', 4, 1)
} as error {
  echo error.message
}
```

```console
params must be a list, a dictionary or nil, not number
```

### Responses

`Response.success()` answers a request with its result, and
`Response.failure()` with an error:

```zuri
import rpc

var done = rpc.Response.success(1, 5)
var missing = rpc.RpcError(rpc.METHOD_NOT_FOUND, 'Method not found: mul')
var failed = rpc.Response.failure(2, missing)

echo rpc.encode(done)
echo rpc.encode(failed)
echo failed.is_error()
```

```console
{"jsonrpc":"2.0","id":1,"result":5}
{"jsonrpc":"2.0","id":2,"error":{"code":-32601,"message":"Method not found: mul"}}
true
```

A response's `id` is `nil` only when it answers a message whose `id`
could not be read, such as one that was not JSON at all. A result of
`nil` is a result like any other, and is sent as `null`.

### Errors and Their Codes

An error is an `RpcError`: a `code`, a short `message`, and optional
`data` carrying whatever else the other side needs to know.

```zuri
import rpc

var error = rpc.RpcError(rpc.INVALID_PARAMS, 'a name is required', {
  field: 'name',
})

echo error.code
echo error.message
echo error.to_dict()
```

```console
-32602
a name is required
{code: -32602, message: a name is required, data: {field: name}}
```

The specification reserves the codes from `-32768` to `-32000`, and
defines these:

| Constant | Code | Meaning |
| --- | --- | --- |
| `PARSE_ERROR` | `-32700` | the message is not valid JSON |
| `INVALID_REQUEST` | `-32600` | the JSON is not a valid message |
| `METHOD_NOT_FOUND` | `-32601` | the method does not exist |
| `INVALID_PARAMS` | `-32602` | the method cannot take these parameters |
| `INTERNAL_ERROR` | `-32603` | the method failed while it ran |
| `SERVER_ERROR_MIN` to `SERVER_ERROR_MAX` | `-32099` to `-32000` | errors an implementation defines for itself |

An application's own errors take any code outside the reserved range.
Pick them once, write them down, and keep them stable: the code is
what the other side's program matches on, and the message is for the
person reading the log.

`RpcError` is an `Error`, so it is raised and caught like any other.
Raised from a request handler, it is the answer.

### Reading and Writing JSON

`rpc.decode()` reads the JSON text of a message, and checks it against
every rule of the specification:

```zuri
import rpc

var text = '{"jsonrpc": "2.0", "id": "a7", "method": "subtract", ' +
    '"params": {"minuend": 42, "subtrahend": 23}}'
var call = rpc.decode(text)

echo call
echo call.params.minuend - call.params.subtrahend
```

```console
Request(subtract #a7)
19
```

What it reads is a `Request`, a `Notification` or a `Response`, told
apart the way the specification tells them apart: a message with a
`method` and an `id` is a request, one with a `method` and no `id` is a
notification, and one with a `result` or an `error` is a response.

Anything that breaks a rule is refused with an `RpcError` carrying the
code it would be answered with:

```zuri
import rpc

var attempts = [
  '{"jsonrpc": "2.0", "method": 1}',
  '{"jsonrpc": "1.0", "id": 1, "method": "ping"}',
  '{"jsonrpc": "2.0", "id": 1, "method": "ping", "params": 1}',
  '{"jsonrpc": "2.0", "id": 1, "result": 1, "error": null}',
  '{"jsonrpc": "2.0", "id": 1',
]

for text in attempts {
  catch {
    rpc.decode(text)
  } as error {
    echo '${error.code} ${error.message}'
  }
}
```

```console
-32600 Invalid Request: method must be a string
-32600 Invalid Request: the jsonrpc member must be '2.0'
-32600 Invalid Request: params must be an array or an object
-32600 Invalid Request: a response needs exactly one of result and error
-32700 Parse error: json.decode(): expected ',' or '}' in object
```

`rpc.read()` does the same for a value already decoded from JSON, for
a program that got the JSON from somewhere that decodes it already.
`rpc.encode()` is the other direction, for any message or list of
them.

### Batches

A list of messages is encoded as a batch:

```zuri
import rpc

echo rpc.encode([
  rpc.Request('add', [1, 2], 1),
  rpc.Notification('log', ['adding']),
])
```

```console
[{"jsonrpc":"2.0","id":1,"method":"add","params":[1,2]},{"jsonrpc":"2.0","method":"log","params":["adding"]}]
```

A batch decodes to a list. Each entry in it is read on its own, so one
bad entry does not spoil the rest: in its place is the `RpcError`
saying what is wrong with it, ready to be answered.

```zuri
import rpc

var batch = rpc.decode(
  '[' +
      '{"jsonrpc": "2.0", "id": 1, "method": "sum", "params": [1, 2]},' +
      '{"jsonrpc": "2.0", "method": "notify_hello"},' +
      '{"foo": "boo"}' +
      ']',
)

for entry in batch {
  if instance_of(entry, rpc.RpcError) {
    echo 'refused: ${entry.message}'
  } else {
    echo entry
  }
}
```

```console
Request(sum #1)
Notification(notify_hello)
refused: Invalid Request: the jsonrpc member must be '2.0'
```

An empty batch, `[]`, is not a message at all, and decoding one raises
`INVALID_REQUEST`.

## Services

A `Service` holds the handler for every method a program answers.
`answer()` takes one message, or a batch, as JSON text or as its
bytes, and returns the JSON text of the answer, or `nil` when there is
nothing to answer.

### Answering Requests

`on_request()` gives a method its handler. The handler is called with
the request's `params` and a `Context`, and what it returns is the
result sent back:

```zuri
import rpc

var service = rpc.Service()
    .on_request('add', @(params) => params[0] + params[1])
    .on_request(
      'whoami',
      @(params, context) => '${context.method} #${context.id}',
    )

echo service.answer(
  '{"jsonrpc": "2.0", "id": 1, "method": "add", "params": [2, 3]}',
)
echo service.answer('{"jsonrpc": "2.0", "id": 2, "method": "whoami"}')
echo service.answer(
  '{"jsonrpc": "2.0", "id": 3, "method": "mul", "params": [2, 3]}',
)
```

```console
{"jsonrpc":"2.0","id":1,"result":5}
{"jsonrpc":"2.0","id":2,"result":"whoami #2"}
{"jsonrpc":"2.0","id":3,"error":{"code":-32601,"message":"Method not found: mul"}}
```

The `Context` names the request's `id` and `method`, the `endpoint`
handling it, and the `request` it arrived in, which for a message from
HTTP is the HTTP request. A handler that has no use for the context
leaves it out, as `add` does. A request for a method with no handler is
answered with `METHOD_NOT_FOUND`, as `mul` is.

A handler is replaced by calling `on_request()` again with the same
method. Method names that start with `rpc.` are reserved by the
specification, and `on_request()` refuses them.

A message that is not valid UTF-8, or not valid JSON, is answered with
`PARSE_ERROR` and an `id` of `nil`, since its `id` cannot be read.

### Failing Properly

A handler fails a request by raising. An `RpcError` is sent back as it
is, code, message, data and all. Any other error is sent back as an
`INTERNAL_ERROR` carrying the error's message:

```zuri
import rpc

var service = rpc.Service()
    .on_request('divide', @(params) {
      if params.length() != 2 or params[1] == 0 {
        raise rpc.RpcError(
          rpc.INVALID_PARAMS,
          'divide takes a number and a non-zero divisor',
        )
      }

      return params[0] / params[1]
    })
    .on_request('save', @(params) {
      raise Error('the disk is full')
    })

echo service.answer(
  '{"jsonrpc": "2.0", "id": 1, "method": "divide", "params": [1, 0]}',
)
echo service.answer(
  '{"jsonrpc": "2.0", "id": 2, "method": "save", "params": ["notes"]}',
)
```

```console
{"jsonrpc":"2.0","id":1,"error":{"code":-32602,"message":"divide takes a number and a non-zero divisor"}}
{"jsonrpc":"2.0","id":2,"error":{"code":-32603,"message":"the disk is full"}}
```

An error that is not an `RpcError` also goes to the service's
[`on_error()`](#watching-the-conversation) handler, since it is a
failure in the program rather than a refusal the program meant to
make. A handler that must not let its failures' messages reach the
other side catches them and raises an `RpcError` of its own instead.

On the calling side, an error answer raises from `request()` as the
`RpcError` the other side sent.

### Notifications

`on_notification()` gives a notification its handler. It is called
the same way, with the `params` and a `Context`, and whatever it
returns is ignored, since nothing is sent back:

```zuri
import rpc

var seen = []
var service = rpc.Service().on_notification('log', @(params) {
  seen.append(params.text)
})

echo service.answer(
  '{"jsonrpc": "2.0", "method": "log", "params": {"text": "started"}}',
)
echo seen
```

```console
nil
[started]
```

A notification for a method with no handler is dropped, and a
notification handler that raises sends nothing back either: the error
goes to `on_error()`. That is the specification's rule, and it is
deliberate. The sender asked not to be answered.

### Methods Without a Handler

`on_unhandled()` sets a fallback for every request and notification
whose method has no handler of its own. It is called like any other
handler, and `context.method` says which method it is standing in for:

```zuri
import rpc

var service = rpc.Service().on_unhandled(@(params, context) {
  if context.method.starts_with('legacy.') {
    return 'retired: ${context.method}'
  }

  raise rpc.RpcError(
    rpc.METHOD_NOT_FOUND,
    'Method not found: ${context.method}',
  )
})

echo service.answer('{"jsonrpc": "2.0", "id": 1, "method": "legacy.export"}')
echo service.answer('{"jsonrpc": "2.0", "id": 2, "method": "export"}')
```

```console
{"jsonrpc":"2.0","id":1,"result":"retired: legacy.export"}
{"jsonrpc":"2.0","id":2,"error":{"code":-32601,"message":"Method not found: export"}}
```

It is the place for a family of methods answered the same way, for a
proxy that passes calls on, and for logging what a client asks for
that nothing answers. Raising `METHOD_NOT_FOUND` from it refuses a
method exactly as a service without a fallback would. Methods whose
names start with `rpc.` never reach it.

### Batches and Their Limit

A batch is answered as a batch. Each request in it gets its answer,
in place, and each notification gets none; a batch of nothing but
notifications has no answer at all:

```zuri
import rpc

var service = rpc.Service().on_request(
  'add',
  @(params) => params[0] + params[1],
)

echo service.answer(
  '[' +
      '{"jsonrpc": "2.0", "id": 1, "method": "add", "params": [1, 2]},' +
      '{"jsonrpc": "2.0", "method": "add", "params": [3, 4]},' +
      '{"jsonrpc": "2.0", "id": 3, "method": "sub"}' +
      ']',
)
```

```console
[{"jsonrpc":"2.0","id":1,"result":3},{"jsonrpc":"2.0","id":3,"error":{"code":-32601,"message":"Method not found: sub"}}]
```

One message holding a million requests is one message, and a service
that took it whole would do a million calls' work for it. A batch of
more than `batch_limit()` messages, `1000` unless set otherwise, is
refused whole, before any of it runs:

```zuri
import rpc

var service = rpc.Service().set_batch_limit(2).on_request('ping', @() => 'pong')

var call = '{"jsonrpc": "2.0", "id": 1, "method": "ping"}'

echo service.answer('[${call}, ${call}, ${call}]')
```

```console
{"jsonrpc":"2.0","id":null,"error":{"code":-32600,"message":"Invalid Request: a batch of 3 messages is over the limit of 2"}}
```

`set_batch_limit(nil)` takes a batch of any size, for a service that
only ever hears from programs it trusts.

## JSON-RPC Over HTTP

Over HTTP, each message, or batch, is the body of a `POST`, and its
answer is the body of the response. Every exchange stands on its own:
nothing is framed, and no connection is kept between calls beyond what
HTTP keeps for itself. It is how services call each other, and how
most of the JSON-RPC in the world is spoken.

### Serving a Service

`rpc.http_handler()` turns a service into a route handler for an
`http` server. Here the server runs on an isolate of its own and the
client calls it:

```zuri
import rpc
import isolate

def serve_calculator(ready) {
  import http
  import rpc

  var calculator = rpc.Service().on_request(
    'add',
    @(params) => params[0] + params[1],
  )

  var server = http.server(0, '127.0.0.1')

  server.post('/rpc', rpc.http_handler(calculator))
  server.bind()
  ready.send(server.socket.local_address().port())
  server.listen()
}

var ready = isolate.channel(1)
var server = isolate.spawn(serve_calculator, ready)
var calculator = rpc.HttpClient('http://127.0.0.1:${ready.recv()}/rpc')

echo calculator.request('add', [2, 3])
echo calculator.request_batch([
  [
    'add',
    [1, 2],
  ],
  [
    'add',
    [3, 4],
  ],
])

server.cancel()
```

```console
5
[3, 7]
```

In production the service is built in the setup each `http` worker
runs, and the server spreads its requests across a pool of isolates:

```zuri,ignore
import http
import rpc

def setup(server) {
  var api = rpc.Service().on_request('add', @(params) => params[0] + params[1])

  server.post('/rpc', rpc.http_handler(api))
}

http.serve(setup, { host: '0.0.0.0', port: 8545 })
```

Everything `http` offers a route applies to this one: middleware,
TLS, HTTP/2, compression, body limits, and the rest of
[Chapter 15](ch15-00-http.md).

A request that arrives over HTTP is answered in the response to that
HTTP request, so its handler answers by returning: `context.defer()`
raises there, since there is no connection to send a later answer on.

### Calling a Service

`rpc.HttpClient` calls a service at a URL. Each call is one `POST`:

```zuri,ignore
import rpc

var node = rpc.HttpClient('https://node.example.com', {
  headers: { Authorization: 'Bearer ${token}' },
  timeout: 10,
})

echo node.request('eth_blockNumber', [])
node.notify('log', ['checked the block number'])

var answers = node.request_batch([
  ['eth_blockNumber', []],
  ['eth_gasPrice', []],
])
```

`request()` returns the result or raises the `RpcError` the service
answered with. `notify()` expects nothing back. `request_batch()`
sends every call in one `POST` and returns the answers in the order of
the calls, whatever order the server sent them in, with each failed
call's `RpcError` in its place.

The options are `headers`, sent with every call, `timeout`, the most
seconds to wait for an answer, and `client`, the `http.HttpClient` to
send through, for its proxy, TLS and connection settings. Each call
can also take a timeout of its own.

### Statuses

The statuses on the wire follow common practice:

| Status | When |
| --- | --- |
| `200` | an answer, whether it is a result or a JSON-RPC error |
| `204` | nothing to answer: a notification, or a batch of them |
| `405` | a method other than `POST`, with an `Allow: POST` header |
| `415` | a body that is not `application/json` in UTF-8 |

A JSON-RPC error never changes the status: `METHOD_NOT_FOUND` is a
`200` whose body says so. The `http` server's own limits apply before
the service sees anything, so a body over its `max_body_size` is a
`413`. Register the handler with `server.any()` and every method other
than `POST` gets its `405`; with `server.post()`, they get the server's
`404`.

On the calling side, any other status raises an `RpcHttpError`
carrying the `status` and the `body`. Some servers send a JSON-RPC
error with a status other than `200`, as older JSON-RPC over HTTP
drafts asked for; when the body of such a response is a JSON-RPC
answer, `HttpClient` reads it as one, and the error raises as the
`RpcError` it is.

### Who Is Calling

Every handler gets the HTTP request it came in as `context.request`,
so it can read headers, the client's address, or whatever a middleware
left in the request's `context`. Authentication belongs in a
middleware, which refuses a request before the service sees it:

```zuri,ignore
def setup(server) {
  var api = rpc.Service().on_request('balance', @(params, context) {
    return accounts.balance(context.request.context.user)
  })

  server.use(@(request, response, next) {
    var user = tokens.user_for(request.bearer_token())

    if user == nil {
      response.text('who are you?\n', 401)
      return
    }

    request.context.user = user
    next()
  })

  server.post('/rpc', rpc.http_handler(api))
}
```

## Framing

A stream of bytes has no edges. A program reading a socket gets bytes
in whatever pieces the network delivers them, and two messages may
arrive in one piece, or one message in ten. A framing says where each
message ends, so the reader can put them back together.

Both framings work the same way. `feed()` takes the next bytes, in
whatever pieces they come, and returns every message they complete.
`frame()` goes the other way, turning a message's text into the bytes
to send.

### Content-Length Headers

`HeaderFraming` puts a short header block in front of each message,
giving its length in bytes:

```zuri
import rpc

var framed = rpc.HeaderFraming().frame('{"jsonrpc":"2.0","method":"ping"}')

echo framed.to_string().split('\r\n')
```

```console
[Content-Length: 33, , {"jsonrpc":"2.0","method":"ping"}]
```

The header block is ASCII: lines of `Name: value`, each ended by
`\r\n`, then a blank line, then exactly as many bytes as
`Content-Length` says. The length counts bytes rather than characters,
so a message in any language frames correctly.

Reading it back, the pieces can be split anywhere at all:

```zuri
import rpc

var framing = rpc.HeaderFraming()
var stream = framing.frame('{"a":1}') + framing.frame('{"b":2}')

var first = framing.feed(stream[0, 30])
var second = framing.feed(stream[30, stream.length()])

echo first.map(@(m) => m.to_string())
echo second.map(@(m) => m.to_string())
echo framing.pending()
```

```console
[{"a":1}]
[{"b":2}]
0
```

The first piece held all of one message and the start of the next.
`feed()` handed back the whole one and kept the rest, and `pending()`
says how many bytes it is keeping. The second piece finished the
second message.

Header names are read without regard to case. `Content-Length` is
required. `Content-Type` may be given, and if it names a charset, the
charset must be UTF-8. Any other header is read past and ignored.

### One Message to a Line

`LineFraming` ends each message with a line break instead, the framing
known as newline-delimited JSON:

```zuri
import rpc

var framing = rpc.LineFraming()
var lines = framing.feed('{"a":1}\n{"b":'.to_bytes())

echo lines.length()
echo framing.feed('2}\r\n'.to_bytes())[0].to_string()
echo framing.frame('{"c":3}').to_string().trim()
```

```console
1
{"b":2}
{"c":3}
```

A line may end in `\n` or `\r\n`, and an empty line is skipped.
`json.encode()` never writes a raw line break, since a line break
inside a string is escaped, so a line is always exactly one message.
A peer that pretty-prints its JSON over several lines cannot use this
framing.

### One Message to Each Read

`MessageFraming` frames nothing. It is for a transport that keeps
messages apart itself, a WebSocket above all, whose every read returns
exactly one message: `feed()` takes each piece as one whole message,
and `frame()` hands the text back as it is.

```zuri
import rpc

var framing = rpc.MessageFraming()

echo framing.feed('{"a":1}'.to_bytes())[0].to_string()
echo framing.frame('{"b":2}').to_string()
```

```console
{"a":1}
{"b":2}
```

A transport that wants it says so with a `framing()` method of its
own, and an endpoint over it uses that framing unless told otherwise.
`rpc.websocket()` does exactly that.

### Choosing a Framing

Both ends must use the same framing, so the choice is usually made by
whatever is on the other end. An endpoint uses its transport's own
framing when it has one, and `HeaderFraming` otherwise, unless told
something else:

```zuri,ignore
var endpoint = rpc.endpoint(transport).set_framing(rpc.LineFraming())
```

Where the choice is yours, `HeaderFraming` is the sturdier of the two:
a reader knows how much is coming before it arrives, and refuses a
message that is too large without reading it.

### Limits

Every framing refuses a message larger than `max_size()`, 64 MiB by
default, and `HeaderFraming` refuses a header block longer than 8 KiB.
Without them, a peer could make a reader hold any amount of memory by
never finishing a message.

```zuri
import rpc

var framing = rpc.HeaderFraming().set_max_size(1024)

catch {
  framing.feed('Content-Length: 4096\r\n\r\n'.to_bytes())
} as error {
  echo error.message
}
```

```console
a message of 4096 bytes is over the 1024-byte limit
```

The message is refused as soon as its header arrives, before any of it
is read.

A framing that cannot read its stream raises `RpcFramingError`. Every
reason is the same in one respect: nothing after it in the stream can
be found, because the reader no longer knows where the next message
starts. The connection is over at that point.

## Transports

A transport carries the bytes. It is any object with three methods:

- `read(max)` returns the next bytes that arrive, at most `max` of
  them, waiting until at least one does. Once the stream has ended, it
  returns empty bytes.
- `write(data)` sends all of `data`.
- `close()` ends the stream in the direction it writes.

A transport may also have `can_listen()`, true when another isolate
can read it while the one that made it writes to it. That is what
[`listen()`](#listening-while-doing-other-work) needs; without the
method, the answer is false.

### Standard Streams

`rpc.stdio()` talks over the program's own standard input and output.
It is the transport of a program that another program starts and
talks to: the parent writes to the child's stdin and reads its stdout.

```zuri,ignore
# calculator.zu
import io
import rpc

rpc
    .endpoint(rpc.stdio())
    .on_request('add', @(params) => params[0] + params[1])
    .on_notification('log', @(params) {
      io.stderr.write('${params[0]}\n')
    })
    .serve()
```

Stdout belongs to the protocol. Anything else the program prints there
lands in the middle of the stream and breaks it for the other side, so
a program serving over stdio sends everything else to stderr, as this
one does with its log. Every write the transport makes is flushed at
once.

### Sockets

`rpc.socket()` talks over a connected `TcpStream`, `UnixStream` or
`TlsStream` from `net`:

```zuri
import net
import rpc
import isolate

var ready = isolate.channel()

var server = isolate.spawn(@(ready) {
  import net
  import rpc

  var listener = net.TcpStream()
  listener.bind('127.0.0.1:0')
  ready.send(listener.local_address().to_string())

  var connection = listener.accept()

  rpc
      .endpoint(rpc.socket(connection))
      .on_request('add', @(params) => params[0] + params[1])
      .serve()

  connection.close()
  listener.close()
}, ready)

var stream = net.TcpStream()
stream.connect(ready.recv())

var client = rpc.endpoint(rpc.socket(stream))

echo client.request('add', [20, 22])
client.close()
server.join()
```

```console
42
```

The server binds to port `0`, so the system picks a free one, and
sends the address back over a channel for the client to connect to. It
serves the one connection it accepts, and its `serve()` returns when
the client closes its end.

A socket is held by one isolate at a time, so a socket endpoint reads
with `serve()` and `request()` rather than `listen()`.

This server serves one connection and ends. A server for many clients
at once is [`rpc.serve()`](#serving-many-connections), which gives
every connection an endpoint of its own.

### Child Processes

`rpc.process()` talks to a child process over its standard streams.
Spawn it with `stdin` and `stdout` both `'pipe'`:

```zuri,ignore
import os
import rpc

var child = os.spawn(
  'zuri',
  ['run', 'calculator.zu'],
  { stdin: 'pipe', stdout: 'pipe' },
)
var calculator = rpc.endpoint(rpc.process(child))

echo calculator.request('add', [2, 3]) # 5
calculator.notify('log', ['added two numbers'])

calculator.close()
child.wait()
```

Closing the endpoint closes the child's stdin. The calculator above
reads that as the end of the connection, its `serve()` returns, and
the program ends, which is what `wait()` waits for. A child's streams
are held by the isolate that spawned it, so this endpoint reads with
`serve()` and `request()` too.

### WebSockets

`rpc.websocket()` talks over a WebSocket from `http.websocket`, either
one a route accepted or one `websocket.connect()` opened. Each
JSON-RPC message is one WebSocket message, so an endpoint over it uses
a `MessageFraming` without being told to.

A WebSocket is a connection in both directions, so either side calls
the other whenever it likes. That is what makes it the transport of
choice for a server that pushes to its clients, such as a node
streaming new blocks to whoever subscribed:

```zuri
import http.websocket
import rpc
import isolate

def serve_shop(ready) {
  import http
  import http.websocket
  import rpc

  var server = http.server(0, '127.0.0.1')

  server.get('/ws', @(request, response) {
    var socket = websocket.accept(request, response)

    rpc.endpoint(rpc.websocket(socket)).on_request('total', @(params, context) {
      var price = context.endpoint.request('price_of', [params.item])

      return price * params.count
    }).serve()
  })

  server.bind()
  ready.send(server.socket.local_address().port())
  server.listen()
}

var ready = isolate.channel(1)
var shop = isolate.spawn(serve_shop, ready)
var socket = websocket.connect('ws://127.0.0.1:${ready.recv()}/ws')

var client = rpc
    .endpoint(rpc.websocket(socket))
    .on_request('price_of', @(params) => params[0] == 'tea' ? 3 : 5)

echo client.request('total', { item: 'tea', count: 4 })

client.close()
shop.cancel()
```

```console
12
```

The route accepts the WebSocket and serves an endpoint on it for as
long as the client stays connected. Asked for a total, the server asks
the client for a price before it answers. A WebSocket is held by one
isolate at a time, so an endpoint over one reads with `serve()` and
`request()`.

### Between Isolates

`rpc.pipe()` returns two transports joined to each other: what one
writes, the other reads. Each end is a `ChannelTransport` over a pair
of isolate channels, and channels cross isolates, so either end can be
handed to another isolate and used from there.

```zuri
import rpc

var ends = rpc.pipe()

ends[0].write('ping'.to_bytes())
echo ends[1].read(4096).to_string()
```

```console
ping
```

A pipe is the natural way to give one part of a program a JSON-RPC
interface to another, and the easiest way to test an endpoint: the
server and its test talk exactly as they would over a socket, with
nothing listening on the network.

### A Transport of Your Own

Anything with the three methods is a transport. This one wraps
another and counts what it sends:

```zuri
import rpc
import isolate

class Counted {

  @new(inner) {
    self.inner = inner
    self.sent = 0
  }

  read(max) {
    return self.inner.read(max)
  }

  write(data) {
    self.sent += data.length()
    self.inner.write(data)
  }

  close() {
    self.inner.close()
  }
}

var ends = rpc.pipe()

isolate.spawn(@(transport) {
  import rpc

  rpc
      .endpoint(transport)
      .on_request('add', @(params) => params[0] + params[1])
      .serve()
}, ends[1])

var counted = Counted(ends[0])
var client = rpc.endpoint(counted)

client.request('add', [1, 2])
client.close()

echo '${counted.sent} bytes sent'
```

```console
76 bytes sent
```

## Endpoints

An endpoint is a service on a connection. `rpc.endpoint(transport)`
makes one, and everything in [Services](#services) holds for it: its
handlers, its fallback, its batch limit and its errors. The connection
adds the other direction. An endpoint calls the other side, and a
handler on one can answer later than it returns.

### Calling the Other Side

`request()` sends a request and waits for its answer:

```zuri,ignore
var sum = client.request('add', [2, 3])
```

It returns the result, raises the `RpcError` the other side answered
with, and raises `RpcClosedError` if the connection ends first. While
it waits, it handles everything else that arrives, exactly as
`serve()` would: other answers, notifications, and requests from the
other side.

`notify()` sends a notification, and returns as soon as it is sent:

```zuri,ignore
client.notify('log', { level: 'info', text: 'started' })
```

`send_request()` sends a request and returns at once, with the
request's `id`. The answer goes to a callback when it arrives:

```zuri
import rpc
import isolate

var ends = rpc.pipe()

isolate.spawn(@(transport) {
  import rpc

  rpc
      .endpoint(transport)
      .on_request('add', @(params) => params[0] + params[1])
      .on_request('fail', @(params) {
        raise rpc.RpcError(-32001, 'no')
      })
      .serve()
}, ends[1])

var client = rpc.endpoint(ends[0])

client.send_request('add', [1, 2], @(result, error) {
  echo 'add: ${result}'
})
client.send_request('fail', nil, @(result, error) {
  echo 'fail: ${error.message}'
})

echo 'waiting'
echo client.request('add', [3, 4])
client.close()
```

```console
waiting
add: 3
fail: no
7
```

A callback is called with the result and `nil`, or with `nil` and the
error. Callbacks run on the isolate that owns the endpoint, while it
is reading: here, while `request()` waited for its own answer, the two
earlier answers arrived first and their callbacks ran. When the
connection ends with a callback still waiting, it is called with an
`RpcClosedError`.

### Sending a Batch

`request_batch()` sends several requests as one batch and waits for
every answer. Each call is a pair of a method and its params, and the
answers come back in the order of the calls, whatever order the other
side sent them in:

```zuri
import rpc
import isolate

var ends = rpc.pipe()

isolate.spawn(@(transport) {
  import rpc

  rpc
      .endpoint(transport)
      .on_request('add', @(params) => params[0] + params[1])
      .serve()
}, ends[1])

var client = rpc.endpoint(ends[0])

var answers = client.request_batch([
  [
    'add',
    [1, 2],
  ],
  [
    'multiply',
    [3, 4],
  ],
  [
    'add',
    [5, 6],
  ],
])

for answer in answers {
  if instance_of(answer, rpc.RpcError) {
    echo 'failed: ${answer.message}'
  } else {
    echo answer
  }
}

client.close()
```

```console
3
failed: Method not found: multiply
11
```

A batch can partly succeed, so a failed call does not raise: its
place in the list holds the `RpcError` it was answered with. Only an
ending connection or a timeout raises, since then no answer can be
trusted to come.

### Answering Later

A handler normally answers by returning. Sometimes the answer comes
from work still to be done: a job on another isolate, a reply from a
third program, an event that has not happened yet. The handler then
calls `context.defer()`, keeps the context, and answers through it
with `reply()` or `fail()` when the answer is ready:

```zuri
import rpc

var ends = rpc.pipe()
var waiting = []

var server = rpc
    .endpoint(ends[0])
    .set_framing(rpc.LineFraming())
    .on_request('next_job', @(params, context) {
      waiting.append(context.defer())
    })
    .on_notification('add_job', @(params) {
      for context in waiting {
        context.reply(params)
      }

      waiting = []
    })

server.handle({ jsonrpc: '2.0', id: 1, method: 'next_job' })
server.handle({ jsonrpc: '2.0', id: 2, method: 'next_job' })
server.handle({
  jsonrpc: '2.0',
  method: 'add_job',
  params: { file: 'cat.png' },
})

echo ends[1].read(4096).to_string().trim()
echo ends[1].read(4096).to_string().trim()
```

```console
{"jsonrpc":"2.0","id":1,"result":{"file":"cat.png"}}
{"jsonrpc":"2.0","id":2,"result":{"file":"cat.png"}}
```

Once a request is deferred, what its handler returns is ignored. It is
answered exactly once: answering it a second time, or answering a
request that was never deferred, raises `ValueError`. A handler that
raises after deferring fails the request with that error, unless it
was already answered. A request deferred from inside a batch is
answered on its own, after the batch's other answers.

### Calls in Both Directions

Either side can call the other at any time, including from inside a
handler. Here the server, asked for an order's total, asks the client
for a price first:

```zuri
import rpc
import isolate

var ends = rpc.pipe()

isolate.spawn(@(transport) {
  import rpc

  rpc.endpoint(transport).on_request('total', @(params, context) {
    var price = context.endpoint.request('price_of', [params.item])

    return price * params.count
  }).serve()
}, ends[1])

var prices = { tea: 3, cake: 5 }
var client = rpc
    .endpoint(ends[0])
    .on_request('price_of', @(params) => prices[params[0]])

echo client.request('total', { item: 'tea', count: 4 })
client.close()
```

```console
12
```

The client's `request('total')` is waiting when the server's request
for `price_of` arrives, and it answers that while it waits, then goes
on waiting for its own answer. Nothing has to be arranged for this:
every wait handles what arrives.

## Reading the Connection

An endpoint reads its transport in one of two ways, and every handler
runs on the isolate that owns the endpoint either way.

### Serving

`serve()` reads and answers messages until the connection ends,
`stop()` is called, or the endpoint is closed. It is the whole program
for a server that does nothing but answer, as every server so far in
this chapter has been.

A message that is not valid UTF-8, or not valid JSON, is answered with
`PARSE_ERROR` and an `id` of `nil`, since its `id` cannot be read. A
stream whose framing cannot be read raises `RpcFramingError` out of
`serve()`.

### Listening While Doing Other Work

A program that waits on other things as well cannot sit inside
`serve()`. `listen()` reads the transport on an isolate of its own,
decodes each message there, and returns a `Channel` of what arrives.
The program takes items from it when it is ready, alongside its other
channels, and passes each to `dispatch()`:

```zuri
import rpc
import isolate

var ends = rpc.pipe()

var caller = isolate.spawn(@(transport) {
  import rpc

  var client = rpc.endpoint(transport)
  var squares = [3, 4, 5].map(@(n) => client.request('square', [n]))

  client.close()

  return squares
}, ends[0])

# The work runs on a worker of its own, and its results come back on
# `done` whenever they are ready.
var jobs = isolate.channel()
var done = isolate.channel()

isolate.spawn(
  @(jobs, done) {
    var job = jobs.recv()

    while job != nil {
      done.send({ id: job.id, result: job.value * job.value })
      job = jobs.recv()
    }
  },
  jobs,
  done,
)

var waiting = {}
var server = rpc.endpoint(ends[1]).on_request('square', @(params, context) {
  waiting[context.id] = context.defer()
  jobs.send({ id: context.id, value: params[0] })
})

var inbox = server.listen()

while !server.is_closed() {
  var ready = isolate.select([inbox, done])

  if ready[0] == inbox {
    server.dispatch(ready[1])
  } else {
    waiting[ready[1].id].reply(ready[1].result)
    waiting.remove(ready[1].id)
  }
}

jobs.close()
echo caller.join()
```

```console
[9, 16, 25]
```

The server defers each request and hands the work to a worker. Its
loop waits on both the connection and the worker's results, and
whichever is ready first is dealt with first, so the endpoint never
stops answering while work is under way.

The reader isolate does the framing and the JSON decoding, so a large
message costs the owning isolate nothing until it is dispatched.
Everything is still sent from the owning isolate, and every handler
still runs there.

Only a transport another isolate can read can be listened to. That is
stdio and a pipe; a socket, a WebSocket and a child process are held
by one isolate at a time, and `listen()` on one raises `ValueError`. Set the
framing before calling `listen()`, since the reader isolate takes the
framing with it.

`serve()` and `request()` work on a listening endpoint too, taking
what arrives from the same channel.

### Reading It Yourself

A program that reads the transport itself, such as one polling many
connections at once, hands each read to `feed()`. It handles every
message the bytes complete, exactly as `serve()` would have, and empty
bytes end the connection. `rpc.serve()` drives every connection it
holds this way.

### Timeouts

`request()` and `request_batch()` take a timeout in seconds:

```zuri,ignore
catch {
  var report = client.request('build_report', nil, 30)
} as error {
  if instance_of(error, rpc.RpcTimeoutError) {
    echo 'gave up on the report'
  }
}
```

A timeout needs the endpoint to be listening. An endpoint reading its
transport itself is inside that transport's read while it waits, and
waits as long as the read does. Over a socket, the socket's own
`set_read_timeout()` is the limit, and a read that runs out raises the
socket's error.

An answer that arrives after its request timed out has nothing waiting
for it, and goes to `on_error()`.

### Stopping and Closing

`stop()` makes `serve()` return once the message it is handling is
done, which lets a handler end the conversation:

```zuri,ignore
server.on_request('shutdown', @(params, context) {
  context.endpoint.stop()
  return 'bye'
})
```

`close()` closes the transport. Nothing more can be sent, and sending
raises `RpcClosedError`; every `send_request()` callback still waiting
is called with an `RpcClosedError`. `is_closed()` is true once the
endpoint is closed or the connection has ended from the other side.

## Serving Many Connections

`rpc.serve()` binds a listening socket and gives every connection
accepted on it an endpoint of its own, spread across a pool of worker
isolates. Each worker polls the connections it holds rather than
blocking on one, so a quiet client costs a descriptor and not a
thread, and one worker serves hundreds of long-lived connections side
by side.

```zuri
import isolate
import net
import rpc

isolate.configure(6)

def setup(endpoint, peer) {
  endpoint.on_request('add', @(params) => params[0] + params[1])
}

def run(ready) {
  import rpc

  rpc.serve(setup, {
    port: 0,
    workers: 2,
    framing: 'line',
    max_connections: 1,
    on_ready: @(address, stop) {
      ready.send(address.to_string())
    },
  })
}

var ready = isolate.channel(1)
var server = isolate.spawn(run, ready)

var stream = net.TcpStream()
stream.connect(ready.recv())

var client = rpc.endpoint(rpc.socket(stream)).set_framing(rpc.LineFraming())

echo client.request('add', [20, 22])
client.close()
server.join()
```

```console
42
```

`setup` runs inside a worker for every connection, with the
connection's endpoint and the client's address, and gives the endpoint
its handlers. A worker shares nothing with the others, so `setup` is a
function of a module, or one that uses nothing but its own imports,
and builds whatever a connection needs. Each endpoint is a full peer:
its handlers can call the client back, and it can notify the client
whenever it has something to say.

| Option | Meaning | Default |
| --- | --- | --- |
| `host` | the address to listen on | `'127.0.0.1'` |
| `port` | the port to listen on; `0` picks a free one | `8000` |
| `path` | a Unix domain socket to listen on, in place of `host` and `port` | `nil` |
| `workers` | how many worker isolates serve connections | the number of CPUs |
| `backlog` | how many accepted connections may wait for a worker | `workers * 4` |
| `framing` | `'header'` for `Content-Length` headers, `'line'` for one message to a line | `'header'` |
| `max_message_size` | the largest message accepted, in bytes | 64 MiB |
| `max_connections_per_worker` | the most connections one worker holds | `256` |
| `idle_timeout` | seconds a connection may stay silent before it is closed | `nil`, never |
| `read_timeout` | seconds a read may wait once a message has begun | `30` |
| `write_timeout` | seconds a write may wait | `30` |
| `cert_chain`, `private_key` | PEM strings that put every connection behind TLS | `nil` |
| `on_ready` | called once bound, with the address and a `stop` function | `nil` |
| `max_connections` | stop accepting after this many | `nil`, never |

`on_ready` is called on the isolate that called `serve()`, once the
socket is bound. The `stop` it is given stops accepting connections;
every worker then finishes the message it is handling, closes its
connections and ends, and `serve()` returns. That is what a signal
handler calls to shut a server down cleanly. Reaching
`max_connections` also stops accepting, but serves the connections
already accepted until each closes, which is what the example above
relies on.

Each worker holds a thread of the isolate pool for as long as the
server runs. When `serve()` is the first thing in a program to use an
isolate it sizes the pool itself; otherwise, call `isolate.configure()`
first thing, as the example does, with room for the workers and
whatever else the program runs.

With `cert_chain` and `private_key`, every connection is TLS, and a
client that fails the handshake is closed without reaching a worker's
handlers. With `path`, the server listens on a Unix domain socket,
the way local daemons offer an API to the programs on their machine;
the socket file is removed when the server stops, and Unix domain
sockets need a platform that has them, which `net.unix.is_supported()`
answers.

A handler that calls its client with `request()` holds its worker
until the answer arrives, and every other connection on that worker
waits with it. For a call that can take a while, `send_request()` with
a callback keeps the worker free.

## Watching the Conversation

`on_trace()` sees the text of every message, as it is sent and as it
arrives:

```zuri
import rpc
import isolate

var ends = rpc.pipe()

isolate.spawn(@(transport) {
  import rpc

  rpc
      .endpoint(transport)
      .on_request('add', @(params) => params[0] + params[1])
      .serve()
}, ends[1])

var client = rpc.endpoint(ends[0]).on_trace(@(direction, text) {
  echo '${direction}: ${text}'
})

client.request('add', [1, 2])
client.close()
```

```console
out: {"jsonrpc":"2.0","id":1,"method":"add","params":[1,2]}
in: {"jsonrpc":"2.0","id":1,"result":3}
```

`on_error()` sees every problem the other side is not told about: an
error a notification handler raised, an error other than an `RpcError`
a request handler raised, an error a callback raised, a response
nothing was waiting for, and a connection that failed. It is called
with the error and the message it concerns, as a dictionary, or `nil`
when no one message is to blame:

```zuri,ignore
server.on_error(@(error, message) {
  io.stderr.write('${error.type}: ${error.message}\n')
})
```

Without an `on_error()` handler, these are dropped. A server that
leaves it unset has no way to learn its own handlers are failing, so
set it.

## Errors

Every error the module raises is one of these:

| | |
| --- | --- |
| `RpcError` | a JSON-RPC error, with a `code`: the other side's answer, or a message that broke the rules |
| `RpcHttpError` | an HTTP status with no JSON-RPC answer, with the `status` and the `body` |
| `RpcFramingError` | the stream's framing cannot be read, and nothing after it can be either |
| `RpcClosedError` | the connection ended before an answer, or the endpoint has been closed |
| `RpcTimeoutError` | a request's timeout ran out before its answer |
| `ValueError` | a mistake in the calling program: params that are not structured, a reserved method name, a request answered twice |

The first is the one a program handles as part of its work. The next
four are about the transport, and the last is a bug to fix.

## What the Module Refuses

**It will not send params that are not a list or a dictionary.** The
specification allows nothing else, and a message that breaks it is
refused when it is built, not when the other side receives it.

**It will not accept a message that breaks the specification.** A
missing or wrong `jsonrpc` member, a method that is not a string, an
`id` that is not a string, a number or `null`, a response with both a
result and an error: each is refused with the code the specification
gives it, never guessed at.

**It will not read text that is not UTF-8.** JSON is UTF-8, and a
message that is not is answered with `PARSE_ERROR` rather than read
with its bad bytes replaced. A header block must be ASCII, and an HTTP
body that declares another charset is refused with `415`.

**It will not answer a response.** A malformed response goes to
`on_error()`, never back to the peer, so two endpoints can never fall
into answering each other's errors forever.

**It will not let a handler claim a reserved name.** Method names that
start with `rpc.` belong to the specification, and neither
`on_request()`, `on_notification()` nor `on_unhandled()` will answer
them.

**It will not do unbounded work for one message.** A batch over the
service's limit is refused whole, and a framing refuses an oversized
message from its header, before reading a byte of it.

**It will not defer what has nowhere to be answered.** A request that
arrived over HTTP is answered in its HTTP response, and `defer()` on
it raises rather than leaving the client waiting for an answer that
cannot come.

**It will not listen to a transport only one isolate can hold.** A
socket, a WebSocket or a child process is read where it is held, and
`listen()` on one raises rather than reading it from somewhere it
cannot be.

## Module Reference

The standard library reference documents every class and method. The
shape of the module:

| | |
| --- | --- |
| `rpc.Service()` | a service: handlers, and `answer()` |
| `rpc.http_handler(service)` | an `http` route handler answering with a service |
| `rpc.HttpClient(url, options)` | a client calling a service over HTTP |
| `rpc.endpoint(transport)` | an `Endpoint` over a transport |
| `rpc.serve(setup, options)` | an endpoint for every connection to a listening socket |
| `rpc.stdio()` | the program's own standard streams |
| `rpc.socket(stream)` | a connected `net` stream |
| `rpc.process(child)` | a child process's standard streams |
| `rpc.websocket(socket)` | a WebSocket from `http.websocket` |
| `rpc.pipe()` | two transports joined to each other |
| `rpc.encode(message)` | a message, or a list of them, as JSON text |
| `rpc.decode(text)` | JSON text as a message, or a batch of them |
| `rpc.read(value)` | a decoded value as a message, or a batch of them |

The messages:

| | |
| --- | --- |
| `Request(method, params, id)` | a call that expects an answer |
| `Notification(method, params)` | a call that expects none |
| `Response(id, result, error)` | an answer, also made by `Response.success()` and `Response.failure()` |
| `RpcError(code, message, data)` | an error, with `to_dict()` |
| `PARSE_ERROR`, `INVALID_REQUEST`, `METHOD_NOT_FOUND` | the codes the specification defines |
| `INVALID_PARAMS`, `INTERNAL_ERROR` | the rest of them |
| `SERVER_ERROR_MIN`, `SERVER_ERROR_MAX` | the range set aside for implementations |

On a `Service`, and so on an `Endpoint`:

| | |
| --- | --- |
| `on_request`, `on_notification`, `on_unhandled` | what it answers |
| `on_error`, `on_trace` | what it reports |
| `set_batch_limit`, `batch_limit` | the most messages a batch may hold, `DEFAULT_BATCH_LIMIT` by default |
| `answer` | one message or batch in, its answer out |

On an `Endpoint` as well:

| | |
| --- | --- |
| `set_framing` | how its messages are told apart |
| `request`, `request_batch`, `send_request`, `notify` | calling the other side |
| `send`, `handle` | sending and handling messages as they are |
| `serve`, `stop` | reading on the isolate that calls it |
| `listen`, `dispatch` | reading on an isolate of its own |
| `feed` | handling what the program read itself |
| `close`, `is_closed` | ending the connection |

On an `HttpClient`: `request`, `request_batch`, `notify` and `send`.

On a `Context`:

| | |
| --- | --- |
| `endpoint`, `id`, `method`, `request` | what is being handled, where, and what it arrived in |
| `is_notification` | whether anything is answered |
| `defer`, `is_deferred` | taking over answering |
| `reply`, `fail`, `is_answered` | answering a deferred request |

The framings, `HeaderFraming`, `LineFraming` and `MessageFraming`:

| | |
| --- | --- |
| `feed(data)` | the messages the next bytes complete |
| `frame(message)` | the bytes that send a message |
| `set_max_size(size)`, `max_size()` | the largest message accepted |
| `pending()` | how many bytes are held for an unfinished message |

The transports, `StdioTransport`, `SocketTransport`, `ProcessTransport`,
`WebSocketTransport` and `ChannelTransport`, each have `read(max)`,
`write(data)`, `close()` and `can_listen()`, and `WebSocketTransport`
has `framing()`.
