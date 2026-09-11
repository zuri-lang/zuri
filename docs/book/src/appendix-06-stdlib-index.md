# Appendix F: The Standard Library Index

Every module, what it is for, and where the book covers it. All of them are
reachable with a bare `import`, with no package manager and no dependency
to add.

Each module's source lives in `libs/`, and every public function in it
carries a doc block with its parameters, defaults and edge cases. Reading
`libs/set.zu` is a faster way to learn `set` than any summary.

## Data and Serialisation

| Module | What it is for | Book |
| --- | --- | --- |
| `json` | encode, decode, read and write JSON | [13](ch13-00-stdlib-tour.md) |
| `yaml` | parse YAML, with anchors, tags and multi-document streams | [13](ch13-00-stdlib-tour.md) |
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
| `io` | standard streams, the terminal, and in-memory files | [9](ch09-00-files.md), [10](ch10-00-binary-data.md) |
| `stat` | the `S_IS*` predicates over a file mode | [13](ch13-00-stdlib-tour.md) |
| `args` | a command-line parser with subcommands and `--help` | [13](ch13-00-stdlib-tour.md) |
| `log` | levelled, structured logging with pluggable transports | [13](ch13-00-stdlib-tour.md) |
| `isolate` | OS-thread concurrency, channels and broadcasts | [11](ch11-00-isolates.md) |

## The Network

| Module | What it is for | Book |
| --- | --- | --- |
| `net` | TCP, UDP, TLS, DTLS, addresses and polling | [12](ch12-00-networking.md) |
| `http` | an HTTP/1.1 and HTTP/2 client and server | [15](ch15-00-http.md) |

## Graphics

| Module | What it is for | Book |
| --- | --- | --- |
| `imagine` | decode, draw, filter and encode images | [16](ch16-00-imagine.md) |

## The Language Itself

| Module | What it is for | Book |
| --- | --- | --- |
| `zuri` | lexing, parsing, compiling and runtime reflection | [17](ch17-00-metaprogramming.md) |
| `math` | the mathematical constants | [4](ch04-02-numbers.md) |

## Submodules

Several of these are packages, and their parts can be imported directly:

```text
compress.deflate  compress.zlib    compress.gzip     compress.zstd
compress.lz4      compress.bzip2   compress.brotli   compress.tar
compress.zip      compress.checksum

net.tcp    net.udp    net.tls    net.dtls   net.ip    net.addr   net.poll

http.status  http.headers  http.cookies  http.router   http.server
http.client  http.files    http.stream   http.body     http.multipart
http.middleware  http.proxy  http.sse    http.websocket  http.h2

os.env  os.path  os.fs  os.process  os.system  os.tempfile

io.tty  io.bytesio

isolate.channel  isolate.broadcast

zuri.token  zuri.ast  zuri.compile  zuri.reflect

log.level  log.transport  log.console  log.file  log.dispatch  log.logger

validate.rule  validate.rules  validate.schema  validate.validator

html.parser  html.tokenizer  html.node  html.selector  html.serialize
html.entities  html.elements  html.namespaces

wire.compile  wire.render  wire.loader  wire.filters  wire.expression

imagine.image  imagine.canvas  imagine.color  imagine.filters
imagine.font   imagine.formats imagine.animation

array.int8  array.uint8  array.int16  array.uint16  array.int32
array.uint32 array.int64 array.uint64 array.float   array.double

jwt.core  jwt.signer  jwt.verifier  jwt.token  jwt.jwks  jwt.codec
```

A package's `index.zu` re-exports what its parts make public, so
`import http` reaches most of `http`'s surface without naming a submodule.
Import a submodule when you want only that part, or when the name would
otherwise collide.

## Shadowing a Module

A file in `./.zuri/libs/` shadows a standard library module of the same
name, because that directory is searched first. See
[The Module System](ch08-00-modules.md).
