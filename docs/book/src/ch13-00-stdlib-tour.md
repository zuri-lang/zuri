# A Tour of the Standard Library

The standard library comes with every installation. There's nothing to add
to a manifest, no package manager to run and no dependencies to resolve, so
`import json` works in a file you created ten seconds ago.

This chapter looks at what's available, grouped by what you'd use it for,
with a working example of each module. It's an overview rather than a
reference. [Appendix F](appendix-06-stdlib-index.md) is the index, every
module has doc comments in `libs/`, and the three largest modules have their
own chapters: [Wire](ch14-00-wire.md), [HTTP](ch15-00-http.md) and
[Imagine](ch16-00-imagine.md).

Read through it once to see what exists. You don't need to remember the
details. The point is that six months from now, when you're about to write
something by hand, you'll remember that it's already in the standard
library.

## Data Formats

### `json`

```zuri
import json

var data = {
  name: 'Ada',
  langs: ['zuri', 'rust'],
  active: true,
}

echo json.encode(data)
echo json.decode('{"a":1}').a
echo json.encode({ name: 'Ada' }, false)
```

```console
{"name":"Ada","langs":["zuri","rust"],"active":true}
1
{
  "name": "Ada"
}
```

`encode(value, compact, max_depth)` produces compact output by default.
`parse(path)` reads and decodes a file, and `dump(value, file)` writes one.
A class that defines `@to_json()` controls how its instances are encoded.

### `yaml`

```zuri
import yaml

echo yaml.parse('name: zuri\ntags:\n  - fast\n  - small')
```

```console
{name: zuri, tags: [fast, small]}
```

Anchors, aliases, tags, documents with several parts and block scalars are
all supported.

### `toml`

```zuri
import toml

echo toml.parse('[package]\nname = "zuri"\nversion = "1.0"\n')
```

```console
{package: {name: zuri, version: 1.0}}
```

`parse()` and `dump()` treat a document as data. `edit()` treats it as a
file someone wrote by hand. The `Document` it returns is written back byte
for byte until you change it, and a change only affects the line it's on.

```zuri
import toml

var doc = toml.edit('# what we ship\n[package]\nversion = "0.9.0"  # bump me\n')
doc.set('package.version', '1.0.0')

echo doc.to_string()
```

```console
# what we ship
[package]
version = "1.0.0"  # bump me

```

That's what you need when a program edits someone else's configuration file:
the comment is kept, and so is the spacing on the line that changed.

### `csv`

```zuri
import csv

echo csv.parse('a,b\n1,2')
```

```console
[[a, b], [1, 2]]
```

`Reader` and `Writer` stream large files, `Dialect` configures separators
and quoting, and `sniff_dialect()` guesses the dialect from a sample.

### `struct`

For binary layouts, covered in [Chapter 10](ch10-00-binary-data.md).

### `base64`, `convert`

```zuri
import convert

echo convert.bytes_to_hex(bytes([255, 0]))
echo convert.to_base(255, 16)
echo convert.from_base('ff', 16)
```

```console
ff00
ff
255
```

`convert` handles conversions between number bases that you'd otherwise
write by hand, and also has helpers for hex, binary, octal and Unicode.

## Text and Markup

### `html`

A parser that follows the WHATWG HTML standard, a full DOM and CSS
selectors:

```zuri
import html

var doc = html.parse('<ul><li class="a">one</li><li>two</li></ul>')

echo doc.query_selector('li.a').text_content()
echo doc.query_selector_all('li').length()
```

```console
one
2
```

The DOM supports traversal, changes and serialization, so you can use this
one module for scraping, as a template backend or to sanitize HTML.

### `wire`

Templates, with directives written as HTML attributes instead of a second
syntax mixed into your markup:

```zuri
import wire

echo wire.render_string('<p x-text="msg"></p>', { msg: 'hi' })
```

```console
<p>hi</p>
```

Everything is escaped by default, and escaped correctly for where it
appears. A value in an attribute, in a URL and in a `<script>` block each
need a different kind of escaping, and Wire knows which is which because it
parses your template's structure instead of treating it as plain text.

[Chapter 14](ch14-00-wire.md) covers Wire in full.

### `url`

```zuri
import url

var parsed = url.parse('https://user@example.com:8443/a/b?q=1#top')

echo parsed.host
echo parsed.port
echo parsed.get_param('q')
```

```console
example.com
8443
1
```

`encode()`, `decode()` and `parse_query()` handle percent-encoding.

### `mime`

```zuri
import mime

echo mime.detect_from_name('a.png')
```

```console
image/png
```

`detect(file)` identifies a file's type by looking at its contents instead
of trusting its extension, which is the check you want for uploaded files.

### `colors`

ANSI colors for terminal output, and conversions between all the color
spaces you're likely to have a value in:

```zuri
import colors

echo colors.hex_to_rgb('#ff8800')
echo colors.rgb_to_hex(255, 136, 0)
echo colors.rgb_to_ansi256(255, 136, 0)
```

```console
[255, 136, 0, 1]
ff8800
214
```

`colors.text(value, color, background)` wraps a string in the escape codes:

```zuri,ignore
import colors

echo colors.text('warning', colors.text_color.yellow)
```

The conversions are what let the same code work on a terminal that can't
display the colors you asked for. A true-color value is converted to the
nearest of 256 colors, and a 256-color value to the nearest of 16. `hex()`,
`rgb()`, `hsl()`, `hsv()`, `hwb()`, `cmyk()` and `xyz()` each take a color
in their color space and return the escape sequence for it.

## Time

### `date`

```zuri
import date

var d = date(2026, 9, 11, 8, 30, 0)

echo d.format('Y-m-d H:i:s')
echo d.format('l, jS F Y')
echo date.parse('2026-09-11').format('Y-m-d')
```

```console
2026-09-11 08:30:00
Friday, 11th September 2026
2026-09-11
```

The format codes are single letters: `Y` for a four-digit year, `m` for a
zero-padded month, `d` for a zero-padded day, `H` for the hour on a 24-hour
clock, `i` for minutes, `s` for seconds, `l` for the name of the weekday,
`F` for the name of the month, and `jS` for the day with an ordinal suffix.

`localtime()` and `gmtime()` return the current time, and
`from_time(seconds)` converts a Unix timestamp. The module includes the IANA
time zone database, so `from_timezone('Europe/London', ...)` handles
daylight saving time changes correctly.

## Cryptography and Identity

### `hash`

Digests and HMACs, covered in [Chapter 10](ch10-00-binary-data.md).

### `bcrypt`

Password hashing, which is a different problem from computing a digest:

```zuri
import bcrypt

var stored = bcrypt.hash('secret')

echo bcrypt.compare('secret', stored)
echo bcrypt.get_rounds(stored)
```

```console
true
10
```

Use `bcrypt` for passwords and `hash` for everything else. `needs_rehash()`
tells you when a stored hash was created with a lower cost than you now
require.

### `crypto`

RSA signatures and HKDF key derivation.

### `uuid`

```zuri
import uuid

echo uuid.v4().length()
echo uuid.is_valid(uuid.v7())
```

```console
36
true
```

Versions 1, 3, 4, 5, 6, 7 and 8 are all available. `v4` is the random
version you'll usually want, and `v7` is ordered by time, which makes it a
better choice for database keys.

### `jwt`

Signing, verifying and decoding JSON Web Tokens, with a JWKS client for keys
that rotate.

## Validation and Structure

### `validate`

A schema builder with a fluent API:

```zuri
import validate

var schema = validate.schema({
  name: validate.required().string().max_length(10),
  age: validate.required().integer().min(0),
})

echo schema.check({ name: 'Ada', age: 36 })
echo schema.check({ name: 'a name that is far too long', age: 200.5 })
```

```console
{valid: true, errors: []}
{valid: false, errors: [{field: name, message: The name field must not exceed 10 characters.}, {field: age, message: The age field must be an integer.}]}
```

`check_or_raise()` raises an error instead of returning the result.
`extend()`, `only()` and `except()` build one schema from another, which is
how you keep a schema for creating a record and a schema for updating it in
sync.

### `types`

Type checks, one for each type, as an alternative to the built-in `is_*`
functions:

```zuri
import types

echo types.of(42)
echo types.int(42)
echo types.int(4.2)
echo types.digit('7')
echo types.alpha('a')
echo types.iterable([1])
echo types.instance(ValueError('x'), Error)
```

```console
number
true
false
true
true
true
true
```

Each one **answers a question** and returns a boolean, and none of them
convert anything. `types.int(4.2)` is `false` because `4.2` isn't an
integer, not because converting it failed.

`types.of()` is the same as `typeof()`. `digit()`, `alpha()` and `char()`
check the content of strings in ways the built-in functions don't, and
`instance(value, Class)` checks the whole inheritance chain.

When you want to convert a value rather than check it, use the methods on
the value itself: `to_number()`, `to_string()`, `to_bigint()`, `to_list()`
and `to_bytes()`.

### `set`

```zuri
import set

var s = set([1, 2, 2, 3])
echo s.length()
```

```console
3
```

Union, intersection, difference and subset checks, with insertion order
preserved.

### `enum`

```zuri
import enum

var Color = enum(['RED', 'GREEN'])
echo Color.RED
```

```console
0
```

Pass a dictionary instead of a list to choose the values yourself.

### `array`

Typed numeric arrays with a fixed width, from `Int8Array` to `Uint64Array`,
plus `FloatArray` and `DoubleArray`. They store values at their declared
width instead of as doubles, which matters for memory use and for working
with binary formats.

```zuri
import array

var ints = array.Int32Array([1, 2, 3])
echo ints.length()
```

```console
3
```

## The Operating System

### `os`

Processes, the file system, paths and environment variables, covered in
[Chapter 9](ch09-00-files.md).

`os.exec(command)` runs a shell command and returns its output.
`os.spawn(command, args, options)` starts a process that you can communicate
with. `os.on_signal(name, handler)` installs a signal handler.
`os.at_exit(handler)` registers cleanup code that runs however the program
ends, and `os.set_exit_code(code)` sets the exit status without ending the
program right away.

### `env`

Configuration: a `.env` file loaded into the process environment, and values
read back out already converted to the right type. Covered in [Chapter
19](ch19-00-env.md).

```zuri,ignore
import env

env.load()

var port = env.int('PORT', 8080)
var debug = env.bool('DEBUG', false)
var secret = env.require('SESSION_SECRET')
```

Variables that are already set in the environment aren't changed, so the
file provides defaults and the deployment environment can override them.

### `io`

Standard streams, terminal control and in-memory files:

```zuri,ignore
import io

var name = io.readline('Your name: ')
var secret = io.readline('Password: ', true)

echo io.stdout.is_tty()
```

`io.TTY` puts the terminal into raw mode, reads individual key presses and
moves the cursor, which is what an interactive program needs. `io.BytesIO`
is the in-memory file described in [Chapter 10](ch10-00-binary-data.md).

`io.capture(body)` collects everything `body` writes to standard output
instead of printing it. That includes `echo`, `print()` and `io.stdout`:

```zuri
import io

def greet(name) {
  echo 'Hello, ${name}!'
}

var out = io.capture(@{
  greet('Ada')
})

echo 'captured ${out.length()} characters'
```

```console
captured 12 characters
```

Captures can be nested, and `capture_begin()` and `capture_end()` do the
same thing manually, for when the body might raise an error and you still
want its output. This is how the tests in [Chapter 23](ch23-00-testing.md)
check what a function prints, and how output from passing tests is kept out
of the report.

### `test`

Test suites, matchers, mocks, snapshots and reports, covered in [Chapter
23](ch23-00-testing.md).

```zuri,ignore
import test { * }

describe('slug', @{
  it('lowercases and joins', @{
    expect(slug('Hello World')).to_be('hello-world')
  })
})

run()
```

### `stat`

The `S_IS*` functions for the `mode` value returned by `file().stats()`,
plus a function to format it:

```zuri
import stat

file('notes.txt', 'w').write('x')
file('notes.txt').chmod(0c644)

var info = file('notes.txt').stats()

echo stat.S_ISREG(info.mode)
echo stat.S_ISDIR(info.mode)
echo stat.file_mode(info.mode)
```

```console
true
false
-rw-r--r--
```

`S_ISREG`, `S_ISDIR`, `S_ISLNK`, `S_ISCHR`, `S_ISBLK`, `S_ISFIFO` and
`S_ISSOCK` each check for one kind of entry. `S_IMODE(mode)` removes the
type bits and keeps the permissions, and `file_mode(mode)` formats the whole
value the way `ls -l` does.

### `args`

A command-line parser with subcommands, typed options, automatic `--help`
and output that wraps to the terminal width. Covered in [Chapter
20](ch20-00-args.md).

```zuri,ignore
import args

var parser = args.Parser('greet')
parser.add_option('name', 'Who to greet', {
  short_name: 'n',
  type: args.STRING,
})
parser.add_command('history', 'Show past greetings')

var parsed = parser.parse()
```

The help text is generated from the same declarations that parse the
arguments, so the two can't get out of sync.

### `log`

```zuri
import log

log.info('server started')
log.error('connection refused')
```

A `Logger` attaches structured fields to every record, a `child()` logger
inherits the fields of its parent, and transports send the records to the
console, to a file or to a destination you write yourself.

### `isolate`

Concurrency, covered in [Chapter 11](ch11-00-isolates.md).

### `ffi`

Calling C and Rust code. `ffi` loads shared libraries, reads their C headers
or Rust source files as declarations, calls their functions with checked
conversions, and turns Zuri functions into callbacks. It can also link
static libraries into loadable ones.

```zuri
import ffi

var c = ffi.open(ffi.LIBC).declare('size_t strlen(const char *s);')

echo c.strlen('interop')
```

```console
7
```

Covered in [Chapter 26](ch26-00-ffi.md).

## Networking

### `net`

TCP, UDP, Unix domain sockets, TLS, DTLS, addresses and polling, covered in
[Chapter 12](ch12-00-networking.md).

### `http`

An HTTP/1.1 and HTTP/2 client and server, with routing, middleware,
WebSockets, server-sent events, multipart uploads, static files and a
reverse proxy. It's introduced in [Chapter 12](ch12-00-networking.md),
covered in full in [Chapter 15](ch15-00-http.md), and used throughout
[Chapter 25](ch25-00-task-board.md).

### `rpc`

JSON-RPC 2.0, the protocol for calling methods in another program, for both
the calling side and the answering side. A service answers calls with
handlers, either behind an `http` route or on a connection. A client calls a
service over HTTP. An endpoint, which can run over a WebSocket, a socket, a
child process's standard streams or a pipe between isolates, both answers
and makes calls, so either side can call the other at any time.
`rpc.serve()` gives each connection to a listening socket its own endpoint,
over TCP, TLS or a Unix domain socket.

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

echo client.request('add', [2, 3])
client.close()
```

```console
5
```

Covered in [Chapter 28](ch28-00-rpc.md).

## Databases

### `sql`

A single API for relational databases, whichever one you use. `sql` defines
what a database adapter has to provide, and supplies everything that's the
same across databases: parameters, transactions and savepoints, cursors,
connection pools, introspection and a single error hierarchy. It comes with
adapters for SQLite, PostgreSQL and MySQL, and switching between them means
changing the connection string. On top of that, `db.schema` changes tables
using one description that works for every database, `sql.migrate` applies
versioned migrations, and `sql.model` gives each table a class.

```zuri,ignore
import sql

var db = sql.open('sqlite://./app.db')
var id = db.insert('posts', { title: 'Hello' })

for post in db.query('select * from posts where id = ?', [id]) {
  echo post.title
}
```

Covered in [Chapter 17](ch17-00-sql.md).

## Mail

### `mail`

Email messages, and the three protocols used to send and receive them, all
written in Zuri from the socket up. `mail.message()` builds a message from
text, HTML and attachments and works out the MIME structure for you, and
`mail.parse()` reads a message back in. `mail.smtp` sends mail, `mail.imap`
reads mail where it's stored, and `mail.pop3` downloads it. The server side
is included too: an SMTP server that uses your own handlers to decide what
to accept, and an IMAP server that serves messages from a mail store. One
store keeps mail on disk in Maildir format and the other keeps it in memory.
`mail.dkim` signs outgoing mail and verifies incoming mail.

```zuri,ignore
import mail

mail.send(
  'smtp://mail.example.com',
  mail.message({
    from: 'reports@example.com',
    to: 'ann@example.com',
    subject: 'Quarterly report',
    text: 'The numbers are in.',
  }),
  { username: 'reports', password: secret },
)
```

Covered in [Chapter 18](ch18-00-mail.md).

## Compression

### `compress`

`deflate`, `zlib`, `gzip`, `zstd`, `lz4`, `bzip2` and `brotli`, plus `tar`
and `zip` archives and `checksum` for CRC32 and Adler-32. Covered in
[Chapter 10](ch10-00-binary-data.md).

## Graphics

### `imagine`

Creating and editing images in an RGBA buffer, with drawing functions, text
in a built-in stroke font, filters, color space conversion, and reading and
writing the common image formats.

```zuri
import imagine { Image }

Image(400, 200, '#0f172a')
    .fill_circle(200, 100, 70, '#38bdf8')
    .circle(200, 100, 70, 'white', { thickness: 3 })
    .save('badge.png')
```

Almost every method returns an image, so you can chain operations.
`Image.open(path)` decodes an existing file, working out the format from the
file's contents instead of its extension:

```zuri,ignore
Image.open('photo.jpg').thumbnail(400, 400).save('thumb.webp')
```

Decoding and encoding are done in native code. Everything in between,
including filters, drawing and color conversion, is ordinary Zuri code that
you can read and extend in `libs/imagine`. [Chapter 16](ch16-00-imagine.md)
covers it in full.

## The Language Itself

### `zuri`

Lexing, parsing, compiling and reflection, all available from Zuri code:

```zuri
import zuri

echo zuri.tokenize('var x = 1').length() > 0
echo zuri.reflect.kind([1])
```

```console
true
list
```

[Chapter 21](ch21-00-metaprogramming.md) covers it in full.

### `math`

Mathematical constants. Everything else is a method of `number`, as
described in [Chapter 4](ch04-02-numbers.md).

## Learning More

The source code for every module is in `libs/`, and every public function
has a doc comment describing its parameters, their default values and its
edge cases. Reading `libs/set.zu` is a quicker way to learn the `set` module
than any summary, and the standard library is written to be readable.
