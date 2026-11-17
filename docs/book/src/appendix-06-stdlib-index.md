# Appendix F: The Standard Library Index

This appendix lists every module in the standard library, what it's for, and
where the book covers it. You can use any of them with a plain `import`,
without a package manager or any dependencies.

The source code for each module is in `libs/`, and every public function in
it has a doc comment that describes its parameters, their default values and
its edge cases.

## Data and Serialization

| Module | What it's for | Book |
| --- | --- | --- |
| `json` | Encode, decode, read and write JSON | [13](ch13-00-stdlib-tour.md) |
| `yaml` | Parse YAML, with anchors, tags and multi-document streams | [13](ch13-00-stdlib-tour.md) |
| `toml` | Parse and write TOML, and edit one without disturbing its layout | [13](ch13-00-stdlib-tour.md) |
| `csv` | Read and write CSV, with dialect detection | [13](ch13-00-stdlib-tour.md) |
| `struct` | Pack and unpack binary layouts | [10](ch10-00-binary-data.md) |
| `base64` | Base64 encode and decode | [10](ch10-00-binary-data.md) |
| `convert` | Base, hex, binary, octal and Unicode conversions | [13](ch13-00-stdlib-tour.md) |
| `compress` | Deflate, zlib, gzip, zstd, lz4, bzip2, brotli, tar, zip, checksums | [10](ch10-00-binary-data.md) |

## Text and Markup

| Module | What it's for | Book |
| --- | --- | --- |
| `html` | A WHATWG-conformant parser, a DOM and CSS selectors | [13](ch13-00-stdlib-tour.md) |
| `markdown` | CommonMark and GitHub Flavored Markdown to HTML, with Pandoc-style extensions | None |
| `wire` | Templating, with directives as HTML attributes | [14](ch14-00-wire.md) |
| `url` | Parse, build and percent-encode URLs | [13](ch13-00-stdlib-tour.md) |
| `mime` | Detect a media type from a name or from content | [13](ch13-00-stdlib-tour.md) |
| `colors` | ANSI terminal colors, falling back gracefully on terminals with fewer colors | [13](ch13-00-stdlib-tour.md) |

## Time

| Module | What it's for | Book |
| --- | --- | --- |
| `date` | Dates, times, formatting, parsing and IANA time zones | [13](ch13-00-stdlib-tour.md) |

## Cryptography and Identity

| Module | What it's for | Book |
| --- | --- | --- |
| `hash` | Digests and HMACs, plus PBKDF2 | [10](ch10-00-binary-data.md) |
| `bcrypt` | Password hashing | [13](ch13-00-stdlib-tour.md) |
| `crypto` | RSA signing and HKDF | [13](ch13-00-stdlib-tour.md) |
| `uuid` | UUID versions 1, 3, 4, 5, 6, 7 and 8 | [13](ch13-00-stdlib-tour.md) |
| `jwt` | Sign, verify and decode JSON Web Tokens, with JWKS | [13](ch13-00-stdlib-tour.md) |

## Validation and Structure

| Module | What it's for | Book |
| --- | --- | --- |
| `validate` | A fluent schema builder | [13](ch13-00-stdlib-tour.md) |
| `types` | Type checks, one for each type | [13](ch13-00-stdlib-tour.md) |
| `set` | An ordered set with the usual algebra | [13](ch13-00-stdlib-tour.md) |
| `enum` | Named constants from a list or a dictionary | [13](ch13-00-stdlib-tour.md) |
| `array` | Typed fixed-width numeric arrays, `Int8` through `Double` | [13](ch13-00-stdlib-tour.md) |

## The Operating System

| Module | What it's for | Book |
| --- | --- | --- |
| `os` | Processes, filesystem, paths, environment, signals | [9](ch09-00-files.md) |
| `env` | A `.env` file into the environment, and typed values back out | [19](ch19-00-env.md) |
| `io` | Standard streams, the terminal, and in-memory files | [9](ch09-00-files.md), [10](ch10-00-binary-data.md) |
| `stat` | The `S_IS*` predicates over a file mode | [13](ch13-00-stdlib-tour.md) |
| `args` | A command-line parser with subcommands and `--help` | [20](ch20-00-args.md) |
| `log` | Levelled, structured logging with pluggable transports | [13](ch13-00-stdlib-tour.md) |
| `isolate` | OS-thread concurrency, channels and broadcasts | [11](ch11-00-isolates.md) |
| `ffi` | Calling C and Rust libraries, callbacks, and linking static libraries | [26](ch26-00-ffi.md) |

## Testing

| Module | What it's for | Book |
| --- | --- | --- |
| `test` | Suites, matchers, test doubles, snapshots and reports | [23](ch23-00-testing.md) |

## Databases

| Module | What it's for | Book |
| --- | --- | --- |
| `sql` | One contract for every relational database, with SQLite, PostgreSQL and MySQL adapters, schema changes, migrations and models | [17](ch17-00-sql.md) |

## Mail

| Module | What it's for | Book |
| --- | --- | --- |
| `mail` | Messages, SMTP, IMAP and POP3, both server ends, and DKIM | [18](ch18-00-mail.md) |

## Networking

| Module | What it's for | Book |
| --- | --- | --- |
| `net` | TCP, UDP, Unix domain sockets, TLS, DTLS, addresses and polling | [12](ch12-00-networking.md) |
| `http` | An HTTP/1.1 and HTTP/2 client and server | [15](ch15-00-http.md) |
| `rpc` | JSON-RPC 2.0 over HTTP, WebSockets, sockets and streams, both ends | [28](ch28-00-rpc.md) |

## Graphics

| Module | What it's for | Book |
| --- | --- | --- |
| `imagine` | Decode, draw, filter and encode images | [16](ch16-00-imagine.md) |

## Numbers

| Module | What it's for | Book |
| --- | --- | --- |
| `math` | The mathematical constants | [4](ch04-02-numbers.md) |

## Metaprogramming

| Module | What it's for | Book |
| --- | --- | --- |
| `zuri` | Lexing, parsing, compiling and runtime reflection | [21](ch21-00-metaprogramming.md) |

## Packages and Their Submodules

Most of the larger modules are packages, which are directories whose
`index.zu` re-exports what their parts make public. `import http` gives you
access to almost all of `http` without naming a submodule. Import a
submodule directly when you only want that part of it, or when a name would
otherwise clash with another.

### `array`

There's one module for each element width, and each one exports a single
class.

| Submodule | Class | Element |
| --- | --- | --- |
| `array.int8` | `Int8Array` | Signed 8-bit |
| `array.uint8` | `UInt8Array` | Unsigned 8-bit |
| `array.int16` | `Int16Array` | Signed 16-bit |
| `array.uint16` | `UInt16Array` | Unsigned 16-bit |
| `array.int32` | `Int32Array` | Signed 32-bit |
| `array.uint32` | `UInt32Array` | Unsigned 32-bit |
| `array.int64` | `Int64Array` | Signed 64-bit |
| `array.uint64` | `UInt64Array` | Unsigned 64-bit |
| `array.float` | `FloatArray` | 32-bit float |
| `array.double` | `DoubleArray` | 64-bit float |

### `compress`

| Submodule | What it's for |
| --- | --- |
| `compress.deflate` | Raw DEFLATE streams |
| `compress.zlib` | DEFLATE with a zlib header |
| `compress.gzip` | DEFLATE with a gzip header and trailer |
| `compress.zstd` | Zstandard, levels 1 to 22 |
| `compress.lz4` | LZ4, block and frame formats |
| `compress.bzip2` | Bzip2 |
| `compress.brotli` | Brotli |
| `compress.tar` | Reading and writing TAR archives |
| `compress.zip` | Reading and writing ZIP archives |
| `compress.checksum` | CRC32, CRC32C, Adler-32 |

### `ffi`

| Submodule | What it's for |
| --- | --- |
| `ffi.errors` | Every error the module raises, under `FfiError` |
| `ffi.types` | `Type`, and the struct, union and enum builders |
| `ffi.pointer` | `Pointer`: native memory and access through it |
| `ffi.library` | `Library`: a loaded shared library |
| `ffi.declare` | `Declarations`: C and Rust source read into types and signatures |
| `ffi.callback` | `Callback`: a Zuri function behind a C function pointer |

### `html`

| Submodule | What it's for |
| --- | --- |
| `html.tokenizer` | The WHATWG tokenizer: text in, tokens out |
| `html.parser` | Tree construction: tokens in, a document out |
| `html.node` | The document tree and everything you can do to it |
| `html.selector` | Finding nodes with CSS selectors |
| `html.serialize` | Writing a document back out, minified or pretty |
| `html.entities` | Named character references, both directions |
| `html.elements` | The element tables tree construction consults |
| `html.namespaces` | The five namespace URIs the parser deals in |

### `http`

| Submodule | What it's for |
| --- | --- |
| `http.client` | `HttpClient`, the request side |
| `http.server` | `HttpServer`, the listening side |
| `http.worker` | Serving across several isolates |
| `http.router` | Matching a method and path to a handler |
| `http.middleware` | CORS, access logging, security headers, and the rest |
| `http.request` | The request object handlers receive |
| `http.response` | The response object handlers return |
| `http.headers` | `Headers`, with the field-name rules of RFC 9110 |
| `http.cookies` | `Cookie` and `CookieJar` |
| `http.session` | Server-side sessions, and the stores that keep them |
| `http.session.sql` | Keeping sessions in a relational database |
| `http.body` | Reading and writing message bodies |
| `http.multipart` | `multipart/form-data`, including file uploads |
| `http.files` | Serving files from disk, with ranges and caching |
| `http.stream` | Chunked and streaming transfers |
| `http.sse` | Server-sent events |
| `http.websocket` | The WebSocket protocol |
| `http.proxy` | Forwarding requests to another server |
| `http.negotiate` | Parsing `Accept`-style headers |
| `http.status` | The IANA status codes and their reason phrases |
| `http.h1` | The HTTP/1.1 wire format |
| `http.errors` | The module's error hierarchy |

### `imagine`

| Submodule | What it's for |
| --- | --- |
| `imagine.image` | `Image`, the pixel buffer everything else operates on |
| `imagine.canvas` | Drawing: lines, shapes, fills, text |
| `imagine.color` | `Color`, and conversion between color spaces |
| `imagine.filters` | Blur, sharpen, convolution, and the rest |
| `imagine.font` | Loading and measuring fonts |
| `imagine.strokefont` | The built-in stroke font, with no file to load |
| `imagine.formats` | Decoding and encoding PNG, JPEG, GIF, WebP and more |
| `imagine.animation` | Multi-frame images |
| `imagine.constants` | The named constants the module understands |
| `imagine.errors` | The module's error hierarchy |

### `io`

| Submodule | What it's for |
| --- | --- |
| `io.bytesio` | `BytesIO`, a file-shaped object backed by memory |
| `io.tty` | Terminal control: raw mode, size, cursor |

### `isolate`

| Submodule | What it's for |
| --- | --- |
| `isolate.channel` | Bounded multi-producer, multi-consumer queues |
| `isolate.broadcast` | One-to-many publish and subscribe |
| `isolate.error` | `IsolateError` |

### `jwt`

| Submodule | What it's for |
| --- | --- |
| `jwt.core` | `encode()`, `decode()`, `sign()`, `verify()` |
| `jwt.signer` | `Signer`, a reusable configured signer |
| `jwt.verifier` | `Verifier`, a reusable configured verifier |
| `jwt.token` | The `Token` object a complete decode returns |
| `jwt.jwks` | Resolving a signing key from a JSON Web Key Set |
| `jwt.codec` | Algorithm identifiers and the low-level encoding |
| `jwt.errors` | The module's error hierarchy |

### `log`

| Submodule | What it's for |
| --- | --- |
| `log.logger` | The module-level `info()`, `warn()`, `error()` and friends |
| `log.level` | The `LogLevel` enum and the default level |
| `log.transport` | `Transport`, the base class every sink extends |
| `log.console` | `ConsoleTransport`, the default |
| `log.file` | `FileTransport`, with size-based rotation |
| `log.dispatch` | Configuring which transports receive what |

### `markdown`

| Submodule | What it's for |
| --- | --- |
| `markdown.node` | `Node`, the parsed document and everything you can do to it |
| `markdown.renderer` | `HtmlRenderer`, turning the tree into HTML one node type at a time |
| `markdown.errors` | `MarkdownError` and `MetadataError` |

### `net`

| Submodule | What it's for |
| --- | --- |
| `net.tcp` | `TcpSocket` and `TcpStream` |
| `net.udp` | `UdpSocket` |
| `net.unix` | `UnixStream`, over a path rather than an address |
| `net.tls` | TLS over a TCP stream |
| `net.dtls` | DTLS over a UDP socket |
| `net.ip` | Parsing, formatting and classifying IP addresses |
| `net.addr` | `SocketAddrV4` and `SocketAddrV6` |
| `net.poll` | Asking which of a set of sockets is ready |

### `os`

| Submodule | What it's for |
| --- | --- |
| `os.path` | Joining, resolving and comparing path strings |
| `os.fs` | Directories, permissions, symlinks, globbing |
| `os.env` | Reading, writing and listing environment variables |
| `os.process` | Process identity, subprocesses, signals |
| `os.system` | Facts about the process, the runtime and the machine |
| `os.tempfile` | The temporary directory, and scratch files in it |
| `os.watch` | Filesystem events, and the exit of child processes |

### `rpc`

| Submodule | What it's for |
| --- | --- |
| `rpc.message` | `Request`, `Notification`, `Response`, and reading and writing them as JSON |
| `rpc.error` | `RpcError`, the codes the specification defines, and the module's other errors |
| `rpc.service` | `Service` and the `Context` its handlers are given |
| `rpc.http` | `http_handler()` and `HttpClient`, JSON-RPC over HTTP |
| `rpc.framing` | `HeaderFraming`, `LineFraming` and `MessageFraming`, telling messages apart |
| `rpc.transport` | The transports, WebSockets among them, and `pipe()` between isolates |
| `rpc.endpoint` | `Endpoint`, a service on a connection |
| `rpc.server` | `serve()`, an endpoint for every connection to a listening socket |

### `sql`

| Submodule | What it's for |
| --- | --- |
| `sql.driver` | The contract an adapter implements, and the capability flags |
| `sql.errors` | Every error a database raises, under one root |
| `sql.params` | Rewriting `?` and `:name` into whatever an engine wants |
| `sql.types` | How Zuri values and database values correspond |
| `sql.decimal` | `Decimal`, for a column a float must not hold |
| `sql.result` | `ResultSet` and `ExecResult` |
| `sql.cursor` | Reading a result a row at a time |
| `sql.statement` | A statement compiled once and run many times |
| `sql.transaction` | `Transaction`, and the savepoints inside it |
| `sql.connection` | The `Connection` a program holds |
| `sql.crud` | Building the four statements that are always the same |
| `sql.schema` | Asking a database what is in it, and changing it |
| `sql.blueprint` | A table described once for every engine |
| `sql.ddl` | Turning a blueprint into one engine's statements |
| `sql.migrate` | Versioned schema changes, applied once each and recorded |
| `sql.model` | Models: records that save and check themselves, queries and associations |
| `sql.pool` | Keeping connections open and lending them out |
| `sql.sqlite` | The SQLite adapter, and its blobs, backups and hooks |
| `sql.postgres` | The PostgreSQL adapter, and LISTEN/NOTIFY |
| `sql.mysql` | The MySQL and MariaDB adapter |

### `mail`

| Submodule | What it's for |
| --- | --- |
| `mail.errors` | Every error the mail stack raises, under one root |
| `mail.address` | Reading and writing the addresses in a header |
| `mail.headers` | The header block, in order and without regard to case |
| `mail.encoding` | The encodings a header and a body use |
| `mail.content` | `Content-Type` and `Content-Disposition` |
| `mail.message` | A message, its MIME tree, and building one |
| `mail.dkim` | Signing a message and checking a signature |
| `mail.sasl` | The authentication mechanisms all three protocols share |
| `mail.stream` | A line-oriented connection, and negotiating TLS over one |
| `mail.smtp` | The sending and receiving ends of SMTP |
| `mail.imap` | The client and server ends of IMAP, and where mail is kept |
| `mail.imap.parser` | The IMAP grammar |
| `mail.imap.store` | `MailStore`, `MaildirStore` and `MemoryStore` |
| `mail.pop3` | The client end of POP3 |
| `mail.pool` | Running a mail server on more than one connection at once |

### `test`

| Submodule | What it's for |
| --- | --- |
| `test.expect` | `Expect` and every matcher on it |
| `test.runner` | Collecting the declarations and running them |
| `test.reporter` | `Reporter`, and the seven built-in ones |
| `test.result` | `Case`, `Suite`, `Failure` and `Summary` |
| `test.mock` | `Mock`, `mock()` and `spy_on()` |
| `test.snapshot` | The snapshot store and its file format |
| `test.conduct` | Discovering and running a directory of test files |
| `test.diff` | Structural equality, and rendering what differs |
| `test.format` | Rendering any value for a failure message |
| `test.source` | Reading a stack trace back to the failing line |
| `test.style` | Terminal colors, symbols and width |
| `test.error` | `AssertionError` and `TestSetupError` |
| `test.context` | What is true while one test is running |

### `validate`

| Submodule | What it's for |
| --- | --- |
| `validate.validators` | The one-line entry points, one per rule |
| `validate.validator` | `Validator`, the fluent builder |
| `validate.schema` | `Schema`, validating a whole dictionary at once |
| `validate.rule` | `Rule`, the base class custom rules extend |
| `validate.rules` | Every built-in rule |

### `wire`

| Submodule | What it's for |
| --- | --- |
| `wire.compile` | Turning a parsed template into an instruction tree |
| `wire.render` | Walking a compiled template and writing the page |
| `wire.expression` | The language between `{{` and `}}` |
| `wire.filters` | The filters every template starts with |
| `wire.escape` | Context-aware escaping |
| `wire.loader` | Resolving the path in an `x-include` |
| `wire.normalize` | Rewriting the pseudo elements before parsing |
| `wire.values` | How a template reads the values it is given |
| `wire.constants` | The directive names Wire reserves |
| `wire.errors` | The module's errors, and the locations they carry |

### `zuri`

| Submodule | What it's for |
| --- | --- |
| `zuri.token` | `tokenize()`, and the `Token` type it returns |
| `zuri.ast` | `parse()`, and the `Node` type it returns |
| `zuri.compile` | `compile()`, and the `Instr` type it returns |
| `zuri.reflect` | Inspecting a live function, class, module or instance |
| `zuri.watch` | Telling `zuri run --watch` about the files a program depends on |

## Shadowing a Module

A file in `./.zuri/libs/` takes priority over a standard library module with
the same name, because that directory is searched first. See [The Module
System](ch08-00-modules.md).
