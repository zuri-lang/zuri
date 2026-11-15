# JSON-RPC

The `rpc` module implements JSON-RPC 2.0, for both the calling side and the
answering side, over any kind of transport: in the body of an HTTP request,
over a WebSocket, a TCP or TLS socket, a Unix domain socket or a pipe to a
child process, or over any other stream of bytes. It's written in Zuri, on
top of `json`, `http`, `isolate` and `net`, and it follows the specification
exactly.

JSON-RPC is a remote procedure call protocol, and a small one. One program
sends the name of a method and its parameters, and the other runs the method
and sends back the result or an error. That's very nearly all there is to
it, and the small size is deliberate. The protocol doesn't say anything
about what the methods are, how the two programs are connected or which of
them is in charge, so it works wherever two programs need to call each
other. Services use it to call each other over HTTP. Blockchain nodes expose
their whole API through it, over HTTP for calls and over WebSockets for the
updates they push to subscribers. Mining pools, wallets, trading systems,
editors and their tools, and plugin hosts use it over sockets and pipes.

The module follows the same approach. A service only answers messages, so
the same service can answer whatever transport delivered the message.
Serving it from an HTTP route, a WebSocket or a socket server only takes a
few lines of code around it.

- [Following Along](#following-along)
- [Introduction](#introduction)
- [Messages](#messages)
  - [Requests and Notifications](#requests-and-notifications)
  - [Responses](#responses)
  - [Errors and Error Codes](#errors-and-error-codes)
  - [Reading and Writing JSON](#reading-and-writing-json)
  - [Batches](#batches)
- [Services](#services)
  - [Answering Requests](#answering-requests)
  - [Reporting Errors](#reporting-errors)
  - [Notifications](#notifications)
  - [Methods Without a Handler](#methods-without-a-handler)
  - [Batches and the Batch Limit](#batches-and-the-batch-limit)
- [JSON-RPC Over HTTP](#json-rpc-over-http)
  - [Serving a Service](#serving-a-service)
  - [Calling a Service](#calling-a-service)
  - [Status Codes](#status-codes)
  - [Identifying the Caller](#identifying-the-caller)
- [Framing](#framing)
  - [Content-Length Headers](#content-length-headers)
  - [One Message per Line](#one-message-per-line)
  - [One Message per Read](#one-message-per-read)
  - [Choosing a Framing](#choosing-a-framing)
  - [Limits](#limits)
- [Transports](#transports)
  - [Standard Streams](#standard-streams)
  - [Sockets](#sockets)
  - [Child Processes](#child-processes)
  - [WebSockets](#websockets)
  - [Between Isolates](#between-isolates)
  - [Writing Your Own Transport](#writing-your-own-transport)
- [Endpoints](#endpoints)
  - [Calling the Other Side](#calling-the-other-side)
  - [Sending a Batch](#sending-a-batch)
  - [Answering Later](#answering-later)
  - [Calls in Both Directions](#calls-in-both-directions)
- [Reading the Connection](#reading-the-connection)
  - [Serving Requests](#serving-requests)
  - [Listening While Doing Other Work](#listening-while-doing-other-work)
  - [Reading the Transport Yourself](#reading-the-transport-yourself)
  - [Timeouts](#timeouts)
  - [Stopping and Closing](#stopping-and-closing)
- [Serving Many Connections](#serving-many-connections)
- [Watching the Conversation](#watching-the-conversation)
- [Errors](#errors)
- [Design Limits](#design-limits)
- [Module Reference](#module-reference)

## Following Along

A service declares which methods it answers, and answers one message at a
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

The message went in as JSON text, and the answer came out the same way,
without using the network at all. Everything else in this chapter is about
how a message reaches a service and how its answer gets back, whether that's
in the body of an HTTP request, over a WebSocket or a socket, or between two
isolates in the same program.

## Introduction

JSON-RPC has three kinds of message. Each one is a small JSON object whose
`jsonrpc` member is `"2.0"`.

A **request** names a `method`, and includes its `params` and an `id`. The
other side has to answer it, and the answer includes the same `id`.

A **notification** is a request without an `id`. Nothing answers it, not
even when it fails. Notifications are for things the sender wants done but
doesn't need to hear back about, such as a log message, a progress report,
or a change the other side should know about.

A **response** includes the `id` of the request it answers, and exactly one
of `result` and `error`.

Parameters are either a list, which is matched to the method's parameters by
position, or a dictionary, which is matched by name. Nothing else is
allowed, so a call to `square(4)` sends `[4]`, never `4`.

Messages can also be sent together as a **batch**, which is a JSON array of
messages. The requests in a batch are answered together, as an array of
responses, each one matched to its request by `id`.

The protocol doesn't have its own transport, and it can be carried in two
different ways. In an **exchange**, such as an HTTP request and its
response, one message or batch goes in each direction and then the exchange
is over. On a **connection**, such as a WebSocket or a socket, messages flow
in both directions for as long as the connection stays open, and neither
side is the client. Either side can send requests and notifications at any
time, even while it's in the middle of answering one. On a stream of bytes,
both sides also have to agree on where one message ends and the next one
starts. That agreement is called the framing.

The module has a layer for each of these parts:

| Layer | What it contains |
| --- | --- |
| Messages | `Request`, `Notification`, `Response`, `RpcError`, and `encode()`, `decode()` and `read()` |
| Services | `Service`, and the `Context` passed to its handlers |
| HTTP | `http_handler()` to serve a service, and `HttpClient` to call one |
| Framing | `HeaderFraming`, `LineFraming` and `MessageFraming` |
| Transports | `StdioTransport`, `SocketTransport`, `ProcessTransport`, `WebSocketTransport`, `ChannelTransport` and `pipe()` |
| Endpoints | `Endpoint`, a service on a connection |
| Servers | `serve()`, which creates an endpoint for every connection to a listening socket |

`import rpc` gives you access to all of it.

## Messages

### Requests and Notifications

You build a message from its contents, and `rpc.encode()` writes it out in
the form JSON-RPC sends:

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

An `id` is a string or a number, and the sender can choose whatever value it
wants to match the answer with. An endpoint numbers its own requests
starting from `1`.

Setting `params` to `nil` leaves the member out of the message, which the
specification allows for a method that doesn't take any parameters. Anything
other than a list, a dictionary or `nil` is rejected when the message is
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
`Response.failure()` answers it with an error:

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

A response's `id` is only `nil` when it answers a message whose `id`
couldn't be read, such as a message that wasn't valid JSON at all. A result
of `nil` is a result like any other, and is sent as `null`.

### Errors and Error Codes

An error is an `RpcError`, which has a `code`, a short `message`, and
optional `data` containing anything else the other side needs to know.

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

The specification reserves the codes from `-32768` to `-32000`, and defines
these:

| Constant | Code | Meaning |
| --- | --- | --- |
| `PARSE_ERROR` | `-32700` | The message isn't valid JSON |
| `INVALID_REQUEST` | `-32600` | The JSON isn't a valid message |
| `METHOD_NOT_FOUND` | `-32601` | The method doesn't exist |
| `INVALID_PARAMS` | `-32602` | The method can't accept these parameters |
| `INTERNAL_ERROR` | `-32603` | The method failed while it was running |
| `SERVER_ERROR_MIN` to `SERVER_ERROR_MAX` | `-32099` to `-32000` | Errors defined by an implementation for its own use |

An application's own errors can use any code outside the reserved range.
Choose your codes once, document them and don't change them. The other
side's program checks the code, while the message is for whoever reads the
log.

`RpcError` is a subclass of `Error`, so you raise and catch it like any
other error. When a request handler raises one, it becomes the answer.

### Reading and Writing JSON

`rpc.decode()` reads a message from JSON text and checks it against every
rule in the specification:

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

It returns a `Request`, a `Notification` or a `Response`, distinguished in
the same way the specification distinguishes them. A message with a `method`
and an `id` is a request, one with a `method` but no `id` is a notification,
and one with a `result` or an `error` is a response.

Anything that breaks a rule is rejected with an `RpcError` that has the code
it would be answered with:

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

`rpc.read()` does the same for a value that has already been decoded from
JSON, for programs that get the JSON from somewhere that has already decoded
it. `rpc.encode()` works in the other direction, for any message or list of
messages.

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

A batch is decoded into a list. Each entry is read separately, so one bad
entry doesn't affect the others. In its place is an `RpcError` describing
what's wrong with it, ready to be sent as an answer.

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

An empty batch, `[]`, isn't a message at all, and decoding one raises
`INVALID_REQUEST`.

## Services

A `Service` holds the handler for every method that a program answers.
`answer()` takes a single message or a batch, as JSON text or as bytes, and
returns the JSON text of the answer, or `nil` if there's nothing to answer.

### Answering Requests

`on_request()` sets the handler for a method. The handler is called with the
request's `params` and a `Context`, and whatever it returns is sent back as
the result:

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

The `Context` gives the request's `id` and `method`, the `endpoint` handling
it, and the `request` it arrived in, which for a message sent over HTTP is
the HTTP request. A handler that doesn't need the context can leave it out,
as `add` does. A request for a method without a handler is answered with
`METHOD_NOT_FOUND`, as `mul` is.

To replace a handler, call `on_request()` again with the same method name.
The specification reserves method names that start with `rpc.`, so
`on_request()` rejects them.

A message that isn't valid UTF-8 or valid JSON is answered with
`PARSE_ERROR` and an `id` of `nil`, because its `id` can't be read.

### Reporting Errors

A handler fails a request by raising an error. An `RpcError` is sent back
exactly as it is, including its code, message and data. Any other error is
sent back as an `INTERNAL_ERROR` that includes the error's message:

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

An error that isn't an `RpcError` is also passed to the service's
[`on_error()`](#watching-the-conversation) handler, since it indicates a
failure in the program rather than a deliberate refusal. If a handler
mustn't let the messages of its errors reach the other side, it should catch
them and raise its own `RpcError` instead.

On the calling side, an error answer is raised by `request()` as the
`RpcError` that the other side sent.

### Notifications

`on_notification()` sets the handler for a notification. It's called in the
same way, with the `params` and a `Context`, and whatever it returns is
ignored, since nothing is sent back:

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

A notification for a method without a handler is dropped, and a notification
handler that raises an error doesn't send anything back either. Instead, the
error is passed to `on_error()`. That's the rule the specification sets, and
it makes sense: the sender asked not to be answered.

### Methods Without a Handler

`on_unhandled()` sets a fallback for every request and notification whose
method doesn't have its own handler. It's called like any other handler, and
`context.method` tells you which method it's handling:

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

It's useful for a group of methods that are all answered in the same way,
for a proxy that passes calls on, and for logging requests that nothing
answers. Raising `METHOD_NOT_FOUND` from it rejects a method, just as a
service without a fallback would. Methods whose names start with `rpc.`
never reach it.

### Batches and the Batch Limit

A batch is answered with a batch. Each request in it gets an answer, in the
same position, and each notification gets no answer. A batch that only
contains notifications has no answer at all:

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

A single message containing a million requests is still a single message,
and a service that accepted it would do the work of a million calls for it.
A batch with more than `batch_limit()` messages, which is `1000` by default,
is rejected completely, before any of it runs:

```zuri
import rpc

var service = rpc.Service().set_batch_limit(2).on_request('ping', @() => 'pong')

var call = '{"jsonrpc": "2.0", "id": 1, "method": "ping"}'

echo service.answer('[${call}, ${call}, ${call}]')
```

```console
{"jsonrpc":"2.0","id":null,"error":{"code":-32600,"message":"Invalid Request: a batch of 3 messages is over the limit of 2"}}
```

`set_batch_limit(nil)` accepts batches of any size, for a service that only
receives calls from programs it trusts.

## JSON-RPC Over HTTP

Over HTTP, each message or batch is the body of a `POST` request, and its
answer is the body of the response. Each exchange stands on its own: nothing
is framed, and no connection is kept between calls, apart from what HTTP
keeps itself. This is how services usually call each other, and it's how
most JSON-RPC traffic is sent.

### Serving a Service

`rpc.http_handler()` turns a service into a route handler for an `http`
server. Here, the server runs on its own isolate, and the client calls it:

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

In production, the service is created in the setup function that each `http`
worker runs, and the server spreads requests across a pool of isolates:

```zuri,ignore
import http
import rpc

def setup(server) {
  var api = rpc.Service().on_request('add', @(params) => params[0] + params[1])

  server.post('/rpc', rpc.http_handler(api))
}

http.serve(setup, { host: '0.0.0.0', port: 8545 })
```

Everything `http` provides for a route applies to this one too, including
middleware, TLS, HTTP/2, compression, body size limits and the rest of
[Chapter 15](ch15-00-http.md).

A request that arrives over HTTP is answered in the response to that HTTP
request, so its handler answers by returning a value. `context.defer()`
raises an error there, because there's no connection to send a later answer
on.

### Calling a Service

`rpc.HttpClient` calls a service at a URL. Each call is a single `POST`
request:

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

`request()` returns the result, or raises the `RpcError` that the service
answered with. `notify()` doesn't expect anything back. `request_batch()`
sends every call in a single `POST` and returns the answers in the same
order as the calls, whatever order the server sent them in, with an
`RpcError` in place of each call that failed.

The options are `headers`, which are sent with every call, `timeout`, the
maximum number of seconds to wait for an answer, and `client`, the
`http.HttpClient` to send requests through, for its proxy, TLS and
connection settings. Each call can also have its own timeout.

### Status Codes

The HTTP status codes follow common practice:

| Status | When |
| --- | --- |
| `200` | There's an answer, whether it's a result or a JSON-RPC error |
| `204` | There's nothing to answer, because the body was a notification or a batch of them |
| `405` | The request used a method other than `POST`. The response includes an `Allow: POST` header. |
| `415` | The body isn't `application/json` in UTF-8 |

A JSON-RPC error never changes the status code, so a `METHOD_NOT_FOUND`
error is sent as a `200` response whose body says so. The `http` server's
own limits apply before the service sees anything, so a body larger than its
`max_body_size` gets a `413`. If you register the handler with
`server.any()`, every method other than `POST` gets a `405`, and with
`server.post()`, they get the server's `404`.

On the calling side, any other status raises an `RpcHttpError` that includes
the `status` and the `body`. Some servers send a JSON-RPC error with a
status other than `200`, as older drafts of JSON-RPC over HTTP required.
When the body of such a response is a JSON-RPC answer, `HttpClient` reads it
as one, and the error is raised as the `RpcError` it is.

### Identifying the Caller

Every handler receives the HTTP request as `context.request`, so it can read
headers, the client's address, or anything a middleware has stored in the
request's `context`. Authentication belongs in a middleware, which rejects a
request before the service sees it:

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

A stream of bytes doesn't have any boundaries. A program reading from a
socket receives bytes in whatever chunks the network delivers them, so two
messages might arrive in one chunk, or one message in ten. A framing marks
where each message ends, so the reader can put the messages back together.

All the framings work in the same way. `feed()` takes the next bytes, in
whatever chunks they arrive, and returns every message they complete.
`frame()` works in the opposite direction, turning a message's text into the
bytes to send.

### Content-Length Headers

`HeaderFraming` puts a short block of headers in front of each message,
giving its length in bytes:

```zuri
import rpc

var framed = rpc.HeaderFraming().frame('{"jsonrpc":"2.0","method":"ping"}')

echo framed.to_string().split('\r\n')
```

```console
[Content-Length: 33, , {"jsonrpc":"2.0","method":"ping"}]
```

The header block is ASCII text: lines of the form `Name: value`, each ending
in `\r\n`, followed by a blank line, and then exactly as many bytes as
`Content-Length` specifies. The length counts bytes rather than characters,
so messages in any language are framed correctly.

When reading, the input can be split at any point at all:

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

The first chunk contained all of one message and the start of the next.
`feed()` returned the complete message and kept the rest, and `pending()`
tells you how many bytes it's keeping. The second chunk completed the second
message.

Header names are case-insensitive. `Content-Length` is required.
`Content-Type` is optional, and if it specifies a charset, the charset must
be UTF-8. Any other header is ignored.

### One Message per Line

`LineFraming` ends each message with a line break instead. This framing is
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

A line can end in `\n` or `\r\n`, and empty lines are skipped.
`json.encode()` never writes a raw line break, because a line break inside a
string is escaped, so each line always contains exactly one message. A peer
that pretty-prints its JSON over several lines can't use this framing.

### One Message per Read

`MessageFraming` doesn't add any framing at all. It's for transports that
keep messages separate themselves, most notably WebSockets, where every read
returns exactly one message. `feed()` treats each chunk as one complete
message, and `frame()` returns the text unchanged.

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

A transport that needs this framing says so with its own `framing()` method,
and an endpoint on that transport uses that framing unless you tell it
otherwise. `rpc.websocket()` does exactly this.

### Choosing a Framing

Both sides have to use the same framing, so the choice is usually made by
whatever is on the other side. An endpoint uses its transport's own framing
if it has one, and `HeaderFraming` otherwise, unless you specify something
else:

```zuri,ignore
var endpoint = rpc.endpoint(transport).set_framing(rpc.LineFraming())
```

When the choice is up to you, `HeaderFraming` is the more robust option. The
reader knows how much data is coming before it arrives, and can reject a
message that's too large without reading it.

### Limits

Every framing rejects a message larger than `max_size()`, which is 64 MiB by
default, and `HeaderFraming` rejects a header block longer than 8 KiB.
Without these limits, a peer could make a reader use any amount of memory by
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

The message is rejected as soon as its header arrives, before any of the
message is read.

A framing that can't read its stream raises an `RpcFramingError`. Whatever
the reason, the result is the same: nothing after that point in the stream
can be read, because the reader no longer knows where the next message
starts. At that point, the connection is over.

## Transports

A transport carries the bytes. It can be any object with these three
methods:

- `read(max)` returns the next bytes that arrive, up to `max` bytes, waiting
  until at least one byte arrives. Once the stream has ended, it returns
  empty bytes.
- `write(data)` sends all of `data`.
- `close()` closes the stream in the direction it writes.

A transport can also have a `can_listen()` method, which returns true when
another isolate can read from it while the isolate that created it writes to
it. That's what [`listen()`](#listening-while-doing-other-work) needs.
Without the method, the answer is false.

### Standard Streams

`rpc.stdio()` communicates over the program's own standard input and output.
It's the transport for a program that's started by another program which
then talks to it: the parent writes to the child's standard input and reads
its standard output.

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

Standard output belongs to the protocol. Anything else the program prints
there ends up in the middle of the stream and breaks it for the other side,
so a program that serves over stdio should send everything else to standard
error, as this one does with its log. Every write the transport makes is
flushed immediately.

### Sockets

`rpc.socket()` communicates over a connected `TcpStream`, `UnixStream` or
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

The server binds to port `0` so the system chooses a free port, and sends
the address back over a channel for the client to connect to. It serves the
one connection it accepts, and its `serve()` returns when the client closes
its end.

A socket can only be used by one isolate at a time, so an endpoint on a
socket reads with `serve()` and `request()` rather than `listen()`.

This server handles a single connection and then ends. For a server that
handles many clients at once, use
[`rpc.serve()`](#serving-many-connections), which gives each connection its
own endpoint.

### Child Processes

`rpc.process()` communicates with a child process over its standard streams.
Spawn the process with both `stdin` and `stdout` set to `'pipe'`:

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

Closing the endpoint closes the child's standard input. The calculator above
treats that as the end of the connection, its `serve()` returns and the
program ends, which is what `wait()` waits for. A child's streams belong to
the isolate that spawned it, so this endpoint also reads with `serve()` and
`request()`.

### WebSockets

`rpc.websocket()` communicates over a WebSocket from `http.websocket`,
either one accepted by a route or one opened with `websocket.connect()`.
Each JSON-RPC message is a single WebSocket message, so an endpoint on a
WebSocket uses a `MessageFraming` automatically.

A WebSocket is a two-way connection, so either side can call the other
whenever it likes. That makes it the best transport for a server that pushes
updates to its clients, such as a node that streams new blocks to its
subscribers:

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

The route accepts the WebSocket and serves an endpoint on it for as long as
the client stays connected. When asked for a total, the server asks the
client for a price before it answers. A WebSocket can only be used by one
isolate at a time, so an endpoint on a WebSocket reads with `serve()` and
`request()`.

### Between Isolates

`rpc.pipe()` returns two transports connected to each other, so that what
one writes, the other reads. Each end is a `ChannelTransport` over a pair of
isolate channels, and channels can be passed between isolates, so you can
pass either end to another isolate and use it there.

```zuri
import rpc

var ends = rpc.pipe()

ends[0].write('ping'.to_bytes())
echo ends[1].read(4096).to_string()
```

```console
ping
```

A pipe is the natural way to give one part of a program a JSON-RPC interface
to another part, and it's the easiest way to test an endpoint. The server
and its test communicate exactly as they would over a socket, without
anything listening on the network.

### Writing Your Own Transport

Any object with the three methods is a transport. This one wraps another
transport and counts how many bytes it sends:

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

An endpoint is a service on a connection. `rpc.endpoint(transport)` creates
one, and everything in [Services](#services) applies to it, including its
handlers, its fallback, its batch limit and its errors. The connection adds
the other direction: an endpoint can call the other side, and a handler on
an endpoint can send its answer after it returns.

### Calling the Other Side

`request()` sends a request and waits for the answer:

```zuri,ignore
var sum = client.request('add', [2, 3])
```

It returns the result, raises the `RpcError` that the other side answered
with, or raises an `RpcClosedError` if the connection ends first. While it
waits, it handles everything else that arrives, just as `serve()` would,
including other answers, notifications and requests from the other side.

`notify()` sends a notification and returns as soon as it's been sent:

```zuri,ignore
client.notify('log', { level: 'info', text: 'started' })
```

`send_request()` sends a request and returns immediately with the request's
`id`. The answer is passed to a callback when it arrives:

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

The callback is called with the result and `nil`, or with `nil` and the
error. Callbacks run on the isolate that owns the endpoint, while it's
reading. Here, while `request()` was waiting for its own answer, the answers
to the two earlier requests arrived first and their callbacks ran. If the
connection ends while a callback is still waiting, it's called with an
`RpcClosedError`.

### Sending a Batch

`request_batch()` sends several requests as a single batch and waits for all
of the answers. Each call is a pair of a method and its parameters, and the
answers come back in the same order as the calls, whatever order the other
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

A batch can partly succeed, so a failed call doesn't raise an error.
Instead, its position in the list holds the `RpcError` it was answered with.
Only a closed connection or a timeout raises an error, because in those
cases no answer can be relied on to arrive.

### Answering Later

A handler normally answers by returning a value. Sometimes, though, the
answer depends on work that hasn't been done yet, such as a job on another
isolate, a reply from a third program, or an event that hasn't happened yet.
In that case, the handler calls `context.defer()`, keeps the context, and
answers through it with `reply()` or `fail()` when the answer is ready:

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

Once a request has been deferred, whatever its handler returns is ignored.
It's answered exactly once, so answering it a second time, or answering a
request that was never deferred, raises a `ValueError`. If a handler raises
an error after deferring, the request fails with that error, unless it has
already been answered. A request deferred from inside a batch is answered
separately, after the batch's other answers.

### Calls in Both Directions

Either side can call the other at any time, even from inside a handler.
Here, when the server is asked for an order's total, it first asks the
client for a price:

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

The client's `request('total')` is waiting when the server's request for
`price_of` arrives. It answers that request while it waits, and then carries
on waiting for its own answer. You don't need to set anything up for this,
because every wait handles whatever arrives.

## Reading the Connection

An endpoint reads from its transport in one of two ways, and either way,
every handler runs on the isolate that owns the endpoint.

### Serving Requests

`serve()` reads and answers messages until the connection ends, `stop()` is
called or the endpoint is closed. For a server that only answers requests,
as every server so far in this chapter has, it's all you need.

A message that isn't valid UTF-8 or valid JSON is answered with
`PARSE_ERROR` and an `id` of `nil`, because its `id` can't be read. A stream
whose framing can't be read makes `serve()` raise an `RpcFramingError`.

### Listening While Doing Other Work

A program that also waits for other things can't simply stay inside
`serve()`. `listen()` reads the transport on its own isolate, decodes each
message there, and returns a `Channel` of the messages that arrive. The
program takes items from the channel when it's ready, alongside its other
channels, and passes each one to `dispatch()`:

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

The server defers each request and passes the work to a worker. Its loop
waits on both the connection and the worker's results, and handles whichever
is ready first, so the endpoint never stops answering while work is in
progress.

The reader isolate handles the framing and the JSON decoding, so a large
message costs the owning isolate nothing until it's dispatched. Everything
is still sent from the owning isolate, and every handler still runs there.

You can only listen to a transport that another isolate can read from, which
means stdio and pipes. Sockets, WebSockets and child processes can only be
used by one isolate at a time, and calling `listen()` on one of them raises
a `ValueError`. Set the framing before you call `listen()`, because the
reader isolate takes the framing with it.

`serve()` and `request()` also work on a listening endpoint, taking whatever
arrives from the same channel.

### Reading the Transport Yourself

A program that reads from the transport itself, such as one that polls many
connections at once, passes each chunk it reads to `feed()`. It handles
every message the bytes complete, just as `serve()` would, and empty bytes
end the connection. `rpc.serve()` handles every connection it holds in this
way.

### Timeouts

`request()` and `request_batch()` accept a timeout in seconds:

```zuri,ignore
catch {
  var report = client.request('build_report', nil, 30)
} as error {
  if instance_of(error, rpc.RpcTimeoutError) {
    echo 'gave up on the report'
  }
}
```

A timeout only works if the endpoint is listening. An endpoint that reads
its transport itself is stuck inside the transport's read while it waits, so
it waits for as long as the read does. Over a socket, the socket's own
`set_read_timeout()` sets the limit, and a read that times out raises the
socket's error.

An answer that arrives after its request has timed out has nothing waiting
for it, so it's passed to `on_error()`.

### Stopping and Closing

`stop()` makes `serve()` return once it has finished handling the current
message, which lets a handler end the conversation:

```zuri,ignore
server.on_request('shutdown', @(params, context) {
  context.endpoint.stop()
  return 'bye'
})
```

`close()` closes the transport. After that, nothing more can be sent, and
trying to send raises an `RpcClosedError`. Every `send_request()` callback
that's still waiting is called with an `RpcClosedError`. `is_closed()`
returns true once the endpoint has been closed, or the other side has ended
the connection.

## Serving Many Connections

`rpc.serve()` binds a listening socket and gives every connection it accepts
its own endpoint, spread across a pool of worker isolates. Each worker polls
the connections it holds instead of blocking on one, so a quiet client only
costs a file descriptor rather than a thread, and a single worker can serve
hundreds of long-lived connections side by side.

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

`setup` runs inside a worker for every connection, with the connection's
endpoint and the client's address, and sets up the endpoint's handlers.
Workers don't share anything with each other, so `setup` should be a
function in a module, or a function that only uses its own imports, and it
should create whatever the connection needs. Each endpoint is a full peer,
so its handlers can call the client back, and it can send the client
notifications whenever it has something to report.

| Option | Meaning | Default |
| --- | --- | --- |
| `host` | The address to listen on | `'127.0.0.1'` |
| `port` | The port to listen on. `0` chooses a free port. | `8000` |
| `path` | A Unix domain socket to listen on, instead of `host` and `port` | `nil` |
| `workers` | The number of worker isolates that serve connections | The number of CPUs |
| `backlog` | How many accepted connections can wait for a worker | `workers * 4` |
| `framing` | `'header'` for `Content-Length` headers, or `'line'` for one message per line | `'header'` |
| `max_message_size` | The largest message accepted, in bytes | 64 MiB |
| `max_connections_per_worker` | The maximum number of connections each worker holds | `256` |
| `idle_timeout` | How many seconds a connection can stay silent before it's closed | `nil`, meaning never |
| `read_timeout` | How many seconds a read can wait once a message has started | `30` |
| `write_timeout` | How many seconds a write can wait | `30` |
| `cert_chain`, `private_key` | PEM strings that put every connection behind TLS | `nil` |
| `on_ready` | A function called once the socket is bound, with the address and a `stop` function | `nil` |
| `max_connections` | The number of connections after which the server stops accepting new ones | `nil`, meaning never |

`on_ready` is called on the isolate that called `serve()`, once the socket
is bound. Calling the `stop` function it receives stops the server from
accepting connections. Every worker then finishes the message it's handling,
closes its connections and ends, and `serve()` returns. That's what a signal
handler should call to shut a server down cleanly. Reaching
`max_connections` also stops the server from accepting connections, but the
connections already accepted are served until each one closes, which is what
the example above relies on.

Each worker uses a thread from the isolate pool for as long as the server
runs. If `serve()` is the first thing in a program to use an isolate, it
sizes the pool itself. Otherwise, call `isolate.configure()` at the very
start, as the example does, with enough room for the workers and anything
else the program runs.

With `cert_chain` and `private_key`, every connection uses TLS, and a client
that fails the handshake is disconnected before it reaches a worker's
handlers. With `path`, the server listens on a Unix domain socket, which is
how local daemons provide an API to other programs on the same machine. The
socket file is removed when the server stops. Unix domain sockets need a
platform that supports them, which you can check with
`net.unix.is_supported()`.

A handler that calls its client with `request()` keeps its worker busy until
the answer arrives, and every other connection on that worker has to wait
too. For a call that might take a while, use `send_request()` with a
callback instead, which keeps the worker free.

## Watching the Conversation

`on_trace()` sees the text of every message, both as it's sent and as it
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

`on_error()` sees every problem that the other side isn't told about: an
error raised by a notification handler, an error other than an `RpcError`
raised by a request handler, an error raised by a callback, a response that
nothing was waiting for, and a connection that failed. It's called with the
error and the message it relates to, as a dictionary, or `nil` when no
single message is to blame:

```zuri,ignore
server.on_error(@(error, message) {
  io.stderr.write('${error.type}: ${error.message}\n')
})
```

Without an `on_error()` handler, these problems are silently dropped. A
server without one has no way to find out that its handlers are failing, so
always set it.

## Errors

Every error the module raises is one of these:

| Error | Raised when |
| --- | --- |
| `RpcError` | A JSON-RPC error, with a `code`. It's either the other side's answer, or a message that broke the rules. |
| `RpcHttpError` | An HTTP status arrived without a JSON-RPC answer. It includes the `status` and the `body`. |
| `RpcFramingError` | The stream's framing can't be read, and nothing after that point can be read either |
| `RpcClosedError` | The connection ended before an answer arrived, or the endpoint has been closed |
| `RpcTimeoutError` | A request's timeout expired before its answer arrived |
| `ValueError` | The calling program made a mistake, such as passing parameters that aren't structured, using a reserved method name or answering a request twice |

The first one is the error a program handles as part of its normal work. The
next four are about the transport, and the last one is a bug to fix.

## Design Limits

**It never sends parameters that aren't a list or a dictionary.** The
specification doesn't allow anything else, so a message that breaks this
rule is rejected when it's built, not when the other side receives it.

**It never accepts a message that breaks the specification.** A missing or
incorrect `jsonrpc` member, a method that isn't a string, an `id` that isn't
a string, a number or `null`, or a response with both a result and an error
are each rejected with the code the specification assigns, without any
guessing.

**It only reads text in UTF-8.** JSON is always UTF-8, so a message that
isn't is answered with `PARSE_ERROR` instead of being read with its invalid
bytes replaced. A header block must be ASCII, and an HTTP body that declares
a different charset is rejected with a `415`.

**It never answers a response.** An invalid response is passed to
`on_error()` and never sent back to the peer, so two endpoints can never get
stuck answering each other's errors forever.

**It doesn't let handlers use reserved names.** Method names that start with
`rpc.` belong to the specification, and `on_request()`, `on_notification()`
and `on_unhandled()` won't answer them.

**It never does an unlimited amount of work for a single message.** A batch
over the service's limit is rejected completely, and a framing rejects an
oversized message based on its header, before reading any of it.

**It doesn't defer a request that has nowhere to send its answer.** A
request that arrived over HTTP is answered in its HTTP response, so calling
`defer()` on it raises an error, instead of leaving the client waiting for
an answer that can never arrive.

**It doesn't listen to a transport that only one isolate can use.** A
socket, a WebSocket or a child process is read where it's held, so calling
`listen()` on one raises an error instead of trying to read it from
somewhere it can't be read.

## Module Reference

The standard library reference documents every class and method. Here's an
overview of the module:

| Function | Description |
| --- | --- |
| `rpc.Service()` | Creates a service, with handlers and `answer()` |
| `rpc.http_handler(service)` | Creates an `http` route handler that answers with a service |
| `rpc.HttpClient(url, options)` | Creates a client that calls a service over HTTP |
| `rpc.endpoint(transport)` | Creates an `Endpoint` on a transport |
| `rpc.serve(setup, options)` | Creates an endpoint for every connection to a listening socket |
| `rpc.stdio()` | Uses the program's own standard streams |
| `rpc.socket(stream)` | Uses a connected `net` stream |
| `rpc.process(child)` | Uses a child process's standard streams |
| `rpc.websocket(socket)` | Uses a WebSocket from `http.websocket` |
| `rpc.pipe()` | Creates two transports connected to each other |
| `rpc.encode(message)` | Encodes a message, or a list of messages, as JSON text |
| `rpc.decode(text)` | Decodes JSON text into a message or a batch of messages |
| `rpc.read(value)` | Reads a decoded value as a message or a batch of messages |

The message classes and constants:

| Name | Description |
| --- | --- |
| `Request(method, params, id)` | A call that expects an answer |
| `Notification(method, params)` | A call that doesn't expect an answer |
| `Response(id, result, error)` | An answer, which can also be created with `Response.success()` and `Response.failure()` |
| `RpcError(code, message, data)` | An error, with `to_dict()` |
| `PARSE_ERROR`, `INVALID_REQUEST`, `METHOD_NOT_FOUND` | Error codes defined by the specification |
| `INVALID_PARAMS`, `INTERNAL_ERROR` | The remaining codes defined by the specification |
| `SERVER_ERROR_MIN`, `SERVER_ERROR_MAX` | The range reserved for implementations |

Methods of a `Service`, and therefore also of an `Endpoint`:

| Methods | Purpose |
| --- | --- |
| `on_request`, `on_notification`, `on_unhandled` | Set what it answers |
| `on_error`, `on_trace` | Set what it reports |
| `set_batch_limit`, `batch_limit` | Set or get the maximum number of messages in a batch, which is `DEFAULT_BATCH_LIMIT` by default |
| `answer` | Takes a message or batch and returns its answer |

Additional methods of an `Endpoint`:

| Methods | Purpose |
| --- | --- |
| `set_framing` | Sets how its messages are separated |
| `request`, `request_batch`, `send_request`, `notify` | Call the other side |
| `send`, `handle` | Send and handle messages as they are |
| `serve`, `stop` | Read on the isolate that calls them |
| `listen`, `dispatch` | Read on a separate isolate |
| `feed` | Handles data the program read itself |
| `close`, `is_closed` | End the connection |

Methods of an `HttpClient`: `request`, `request_batch`, `notify` and `send`.

Members of a `Context`:

| Members | Purpose |
| --- | --- |
| `endpoint`, `id`, `method`, `request` | What's being handled, where, and what it arrived in |
| `is_notification` | Whether an answer will be sent |
| `defer`, `is_deferred` | Take over answering the request |
| `reply`, `fail`, `is_answered` | Answer a deferred request |

Methods of the framings, `HeaderFraming`, `LineFraming` and
`MessageFraming`:

| Method | Purpose |
| --- | --- |
| `feed(data)` | Returns the messages that the next bytes complete |
| `frame(message)` | Returns the bytes that send a message |
| `set_max_size(size)`, `max_size()` | Set or get the largest message accepted |
| `pending()` | Returns how many bytes are held for an unfinished message |

The transports, `StdioTransport`, `SocketTransport`, `ProcessTransport`,
`WebSocketTransport` and `ChannelTransport`, each have `read(max)`,
`write(data)`, `close()` and `can_listen()`, and `WebSocketTransport` also
has `framing()`.
