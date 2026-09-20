# Appendix F: The Standard Library Index

Every module in the standard library, what it is for, and where the book
covers it. All of them are reachable with a bare `import`, with no package
manager and no dependency to add.

Each module's source is in `libs/`, and every public function in it carries
a doc block stating its parameters, its defaults and its edge cases.

## Data and Serialisation

| Module | What it is for | Book |
| --- | --- | --- |
| `json` | encode, decode, read and write JSON | [13](ch13-00-stdlib-tour.md) |
| `yaml` | parse YAML, with anchors, tags and multi-document streams | [13](ch13-00-stdlib-tour.md) |
| `toml` | parse and write TOML, and edit one without disturbing its layout | [13](ch13-00-stdlib-tour.md) |
| `csv` | read and write CSV, with dialect detection | [13](ch13-00-stdlib-tour.md) |
| `struct` | pack and unpack binary layouts | [10](ch10-00-binary-data.md) |
| `base64` | Base64 encode and decode | [10](ch10-00-binary-data.md) |
| `convert` | base, hex, binary, octal and unicode conversions | [13](ch13-00-stdlib-tour.md) |
| `compress` | deflate, zlib, gzip, zstd, lz4, bzip2, brotli, tar, zip, checksums | [10](ch10-00-binary-data.md) |

## Text and Markup

| Module | What it is for | Book |
| --- | --- | --- |
| `html` | a WHATWG-conformant parser, a DOM and CSS selectors | [13](ch13-00-stdlib-tour.md) |
| `wire` | templating, with directives as HTML attributes | [14](ch14-00-wire.md) |
| `url` | parse, build and percent-encode URLs | [13](ch13-00-stdlib-tour.md) |
| `mime` | detect a media type from a name or from content | [13](ch13-00-stdlib-tour.md) |
| `colors` | ANSI terminal colour, with graceful degradation | [13](ch13-00-stdlib-tour.md) |

## Time

| Module | What it is for | Book |
| --- | --- | --- |
| `date` | dates, times, formatting, parsing and IANA time zones | [13](ch13-00-stdlib-tour.md) |

## Cryptography and Identity

| Module | What it is for | Book |
| --- | --- | --- |
| `hash` | digests and HMACs, plus PBKDF2 | [10](ch10-00-binary-data.md) |
| `bcrypt` | password hashing | [13](ch13-00-stdlib-tour.md) |
| `crypto` | RSA signing and HKDF | [13](ch13-00-stdlib-tour.md) |
| `uuid` | UUID versions 1, 3, 4, 5, 6, 7 and 8 | [13](ch13-00-stdlib-tour.md) |
| `jwt` | sign, verify and decode JSON Web Tokens, with JWKS | [13](ch13-00-stdlib-tour.md) |

## Structure and Validation

| Module | What it is for | Book |
| --- | --- | --- |
| `validate` | a fluent schema builder | [13](ch13-00-stdlib-tour.md) |
| `types` | checked coercion between types | [13](ch13-00-stdlib-tour.md) |
| `set` | an ordered set with the usual algebra | [13](ch13-00-stdlib-tour.md) |
| `enum` | named constants from a list or a dictionary | [13](ch13-00-stdlib-tour.md) |
| `array` | typed fixed-width numeric arrays, `Int8` through `Double` | [13](ch13-00-stdlib-tour.md) |

## The System

| Module | What it is for | Book |
| --- | --- | --- |
| `os` | processes, filesystem, paths, environment, signals | [9](ch09-00-files.md) |
| `env` | a `.env` file into the environment, and typed values back out | [19](ch19-00-env.md) |
| `io` | standard streams, the terminal, and in-memory files | [9](ch09-00-files.md), [10](ch10-00-binary-data.md) |
| `stat` | the `S_IS*` predicates over a file mode | [13](ch13-00-stdlib-tour.md) |
| `args` | a command-line parser with subcommands and `--help` | [20](ch20-00-args.md) |
| `log` | levelled, structured logging with pluggable transports | [13](ch13-00-stdlib-tour.md) |
| `isolate` | OS-thread concurrency, channels and broadcasts | [11](ch11-00-isolates.md) |

## Testing

| Module | What it is for | Book |
| --- | --- | --- |
| `test` | suites, matchers, test doubles, snapshots and reports | [23](ch23-00-testing.md) |

## Databases

| Module | What it is for | Book |
| --- | --- | --- |
| `sql` | one contract for every relational database, with SQLite, PostgreSQL and MySQL adapters | [17](ch17-00-sql.md) |

## Mail

| Module | What it is for | Book |
| --- | --- | --- |
| `mail` | messages, SMTP, IMAP and POP3, both server ends, and DKIM | [18](ch18-00-mail.md) |

## The Network

| Module | What it is for | Book |
| --- | --- | --- |
| `net` | TCP, UDP, unix sockets, TLS, DTLS, addresses and polling | [12](ch12-00-networking.md) |
| `http` | an HTTP/1.1 and HTTP/2 client and server | [15](ch15-00-http.md) |

## Graphics

| Module | What it is for | Book |
| --- | --- | --- |
| `imagine` | decode, draw, filter and encode images | [16](ch16-00-imagine.md) |

## The Language Itself

| Module | What it is for | Book |
| --- | --- | --- |
| `zuri` | lexing, parsing, compiling and runtime reflection | [21](ch21-00-metaprogramming.md) |
| `math` | the mathematical constants | [4](ch04-02-numbers.md) |

## Packages and Their Submodules

Most of the larger modules are packages: a directory whose `index.zu`
re-exports what its parts make public. `import http` reaches almost all of
`http` without naming a submodule. Import a submodule directly when you
want only that part of it, or when a name would otherwise collide.

### `array`

One module per element width, each exporting a single class.

| Submodule | Class | Element |
| --- | --- | --- |
| `array.int8` | `Int8Array` | signed 8-bit |
| `array.uint8` | `UInt8Array` | unsigned 8-bit |
| `array.int16` | `Int16Array` | signed 16-bit |
| `array.uint16` | `UInt16Array` | unsigned 16-bit |
| `array.int32` | `Int32Array` | signed 32-bit |
| `array.uint32` | `UInt32Array` | unsigned 32-bit |
| `array.int64` | `Int64Array` | signed 64-bit |
| `array.uint64` | `UInt64Array` | unsigned 64-bit |
| `array.float` | `FloatArray` | 32-bit float |
| `array.double` | `DoubleArray` | 64-bit float |

### `compress`

| Submodule | What it is for |
| --- | --- |
| `compress.deflate` | raw DEFLATE streams |
| `compress.zlib` | DEFLATE with a zlib header |
| `compress.gzip` | DEFLATE with a gzip header and trailer |
| `compress.zstd` | Zstandard, levels 1 to 22 |
| `compress.lz4` | LZ4, block and frame formats |
| `compress.bzip2` | bzip2 |
| `compress.brotli` | Brotli |
| `compress.tar` | reading and writing TAR archives |
| `compress.zip` | reading and writing ZIP archives |
| `compress.checksum` | CRC32, CRC32C, Adler-32 |

### `html`

| Submodule | What it is for |
| --- | --- |
| `html.tokenizer` | the WHATWG tokenizer: text in, tokens out |
| `html.parser` | tree construction: tokens in, a document out |
| `html.node` | the document tree and everything you can do to it |
| `html.selector` | finding nodes with CSS selectors |
| `html.serialize` | writing a document back out, minified or pretty |
| `html.entities` | named character references, both directions |
| `html.elements` | the element tables tree construction consults |
| `html.namespaces` | the five namespace URIs the parser deals in |

### `http`

| Submodule | What it is for |
| --- | --- |
| `http.client` | `HttpClient`, the request side |
| `http.server` | `HttpServer`, the listening side |
| `http.worker` | serving across several isolates |
| `http.router` | matching a method and path to a handler |
| `http.middleware` | CORS, access logging, security headers, and the rest |
| `http.request` | the request object handlers receive |
| `http.response` | the response object handlers return |
| `http.headers` | `Headers`, with the field-name rules of RFC 9110 |
| `http.cookies` | `Cookie` and `CookieJar` |
| `http.session` | server-side sessions, and the stores that keep them |
| `http.session.sql` | keeping sessions in a relational database |
| `http.body` | reading and writing message bodies |
| `http.multipart` | `multipart/form-data`, including file uploads |
| `http.files` | serving files from disk, with ranges and caching |
| `http.stream` | chunked and streaming transfers |
| `http.sse` | server-sent events |
| `http.websocket` | the WebSocket protocol |
| `http.proxy` | forwarding requests to another server |
| `http.negotiate` | parsing `Accept`-style headers |
| `http.status` | the IANA status codes and their reason phrases |
| `http.h1` | the HTTP/1.1 wire format |
| `http.errors` | the module's error hierarchy |

### `imagine`

| Submodule | What it is for |
| --- | --- |
| `imagine.image` | `Image`, the pixel buffer everything else operates on |
| `imagine.canvas` | drawing: lines, shapes, fills, text |
| `imagine.color` | `Color`, and conversion between colour spaces |
| `imagine.filters` | blur, sharpen, convolution, and the rest |
| `imagine.font` | loading and measuring fonts |
| `imagine.strokefont` | the built-in stroke font, with no file to load |
| `imagine.formats` | decoding and encoding PNG, JPEG, GIF, WebP and more |
| `imagine.animation` | multi-frame images |
| `imagine.constants` | the named constants the module understands |
| `imagine.errors` | the module's error hierarchy |

### `io`

| Submodule | What it is for |
| --- | --- |
| `io.bytesio` | `BytesIO`, a file-shaped object backed by memory |
| `io.tty` | terminal control: raw mode, size, cursor |

### `isolate`

| Submodule | What it is for |
| --- | --- |
| `isolate.channel` | bounded multi-producer, multi-consumer queues |
| `isolate.broadcast` | one-to-many publish and subscribe |
| `isolate.error` | `IsolateError` |

### `jwt`

| Submodule | What it is for |
| --- | --- |
| `jwt.core` | `encode()`, `decode()`, `sign()`, `verify()` |
| `jwt.signer` | `Signer`, a reusable configured signer |
| `jwt.verifier` | `Verifier`, a reusable configured verifier |
| `jwt.token` | the `Token` object a complete decode returns |
| `jwt.jwks` | resolving a signing key from a JSON Web Key Set |
| `jwt.codec` | algorithm identifiers and the low-level encoding |
| `jwt.errors` | the module's error hierarchy |

### `log`

| Submodule | What it is for |
| --- | --- |
| `log.logger` | the module-level `info()`, `warn()`, `error()` and friends |
| `log.level` | the `LogLevel` enum and the default level |
| `log.transport` | `Transport`, the base class every sink extends |
| `log.console` | `ConsoleTransport`, the default |
| `log.file` | `FileTransport`, with size-based rotation |
| `log.dispatch` | configuring which transports receive what |

### `net`

| Submodule | What it is for |
| --- | --- |
| `net.tcp` | `TcpSocket` and `TcpStream` |
| `net.udp` | `UdpSocket` |
| `net.unix` | `UnixStream`, over a path rather than an address |
| `net.tls` | TLS over a TCP stream |
| `net.dtls` | DTLS over a UDP socket |
| `net.ip` | parsing, formatting and classifying IP addresses |
| `net.addr` | `SocketAddrV4` and `SocketAddrV6` |
| `net.poll` | asking which of a set of sockets is ready |

### `os`

| Submodule | What it is for |
| --- | --- |
| `os.path` | joining, resolving and comparing path strings |
| `os.fs` | directories, permissions, symlinks, globbing |
| `os.env` | reading, writing and listing environment variables |
| `os.process` | process identity, subprocesses, signals |
| `os.system` | facts about the process, the runtime and the machine |
| `os.tempfile` | the temporary directory, and scratch files in it |

### `sql`

| Submodule | What it is for |
| --- | --- |
| `sql.driver` | the contract an adapter implements, and the capability flags |
| `sql.errors` | every error a database raises, under one root |
| `sql.params` | rewriting `?` and `:name` into whatever an engine wants |
| `sql.types` | how Zuri values and database values correspond |
| `sql.decimal` | `Decimal`, for a column a float must not hold |
| `sql.result` | `ResultSet` and `ExecResult` |
| `sql.cursor` | reading a result a row at a time |
| `sql.statement` | a statement compiled once and run many times |
| `sql.transaction` | `Transaction`, and the savepoints inside it |
| `sql.connection` | the `Connection` a program holds |
| `sql.crud` | building the four statements that are always the same |
| `sql.schema` | asking a database what is in it |
| `sql.pool` | keeping connections open and lending them out |
| `sql.sqlite` | the SQLite adapter, and its blobs, backups and hooks |
| `sql.postgres` | the PostgreSQL adapter, and LISTEN/NOTIFY |
| `sql.mysql` | the MySQL and MariaDB adapter |

### `mail`

| Submodule | What it is for |
| --- | --- |
| `mail.errors` | every error the mail stack raises, under one root |
| `mail.address` | reading and writing the addresses in a header |
| `mail.headers` | the header block, in order and without regard to case |
| `mail.encoding` | the encodings a header and a body use |
| `mail.content` | `Content-Type` and `Content-Disposition` |
| `mail.message` | a message, its MIME tree, and building one |
| `mail.dkim` | signing a message and checking a signature |
| `mail.sasl` | the authentication mechanisms all three protocols share |
| `mail.stream` | a line-oriented connection, and negotiating TLS over one |
| `mail.smtp` | the sending and receiving ends of SMTP |
| `mail.imap` | the client and server ends of IMAP, and where mail is kept |
| `mail.imap.parser` | the IMAP grammar |
| `mail.imap.store` | `MailStore`, `MaildirStore` and `MemoryStore` |
| `mail.pop3` | the client end of POP3 |
| `mail.pool` | running a mail server on more than one connection at once |

### `test`

| Submodule | What it is for |
| --- | --- |
| `test.expect` | `Expect` and every matcher on it |
| `test.runner` | collecting the declarations and running them |
| `test.reporter` | `Reporter`, and the seven built-in ones |
| `test.result` | `Case`, `Suite`, `Failure` and `Summary` |
| `test.mock` | `Mock`, `mock()` and `spy_on()` |
| `test.snapshot` | the snapshot store and its file format |
| `test.conduct` | discovering and running a directory of test files |
| `test.diff` | structural equality, and rendering what differs |
| `test.format` | rendering any value for a failure message |
| `test.source` | reading a stack trace back to the failing line |
| `test.style` | terminal colour, symbols and width |
| `test.error` | `AssertionError` and `TestSetupError` |
| `test.context` | what is true while one test is running |

### `validate`

| Submodule | What it is for |
| --- | --- |
| `validate.validators` | the one-line entry points, one per rule |
| `validate.validator` | `Validator`, the fluent builder |
| `validate.schema` | `Schema`, validating a whole dictionary at once |
| `validate.rule` | `Rule`, the base class custom rules extend |
| `validate.rules` | every built-in rule |

### `wire`

| Submodule | What it is for |
| --- | --- |
| `wire.compile` | turning a parsed template into an instruction tree |
| `wire.render` | walking a compiled template and writing the page |
| `wire.expression` | the language between `{{` and `}}` |
| `wire.filters` | the filters every template starts with |
| `wire.escape` | context-aware escaping |
| `wire.loader` | resolving the path in an `x-include` |
| `wire.normalize` | rewriting the pseudo elements before parsing |
| `wire.values` | how a template reads the values it is given |
| `wire.constants` | the directive names Wire reserves |
| `wire.errors` | the module's errors, and the locations they carry |

### `zuri`

| Submodule | What it is for |
| --- | --- |
| `zuri.token` | `tokenize()`, and the `Token` type it returns |
| `zuri.ast` | `parse()`, and the `Node` type it returns |
| `zuri.compile` | `compile()`, and the `Instr` type it returns |
| `zuri.reflect` | inspecting a live function, class, module or instance |

## Shadowing a Module

A file in `./.zuri/libs/` shadows a standard library module of the same
name, because that directory is searched first. See
[The Module System](ch08-00-modules.md).
