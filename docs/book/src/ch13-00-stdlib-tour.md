# A Tour of the Standard Library

The standard library is part of the installation. There is nothing to add
to a manifest, no package manager to run, and no dependency to resolve;
`import json` works in a file you created ten seconds ago.

This chapter walks through what is there, grouped by the job you would
reach for it to do, with a working example of each. It is a tour rather
than a reference: [Appendix F](appendix-06-stdlib-index.md) is the index,
every module carries doc blocks in `libs/`, and the three largest modules
have chapters of their own — [Wire](ch14-00-wire.md),
[HTTP](ch15-00-http.md) and [Imagine](ch16-00-imagine.md).

Read it once to learn what exists. The value of a tour like this is not
remembering the details; it is recognising, six months from now, that the
thing you are about to write by hand is already here.

## Data Formats

### `json`

```zuri
import json

var data = { name: 'Ada', langs: ['zuri', 'rust'], active: true }

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

`encode(value, compact, max_depth)` defaults to compact. `parse(path)`
reads and decodes a file; `dump(value, file)` writes one. A class that
defines `@to_json()` controls its own encoding.

### `yaml`

```zuri
import yaml

echo yaml.parse('name: zuri\ntags:\n  - fast\n  - small')
```

```console
{name: zuri, tags: [fast, small]}
```

Anchors, aliases, tags, multi-document streams and block scalars are all
supported.

### `toml`

```zuri
import toml

echo toml.parse('[package]\nname = "zuri"\nversion = "1.0"\n')
```

```console
{package: {name: zuri, version: 1.0}}
```

`parse()` and `dump()` treat a document as data. `edit()` treats it as a
file somebody wrote: the `Document` it returns renders back byte for byte
until it is changed, and a change disturbs only the line it lands on.

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

That is what a program editing somebody else's configuration file needs:
the comment stayed, and so did the spacing on the line that changed.

### `csv`

```zuri
import csv

echo csv.parse('a,b\n1,2')
```

```console
[[a, b], [1, 2]]
```

`Reader` and `Writer` stream large files, `Dialect` configures separators
and quoting, and `sniff_dialect()` guesses from a sample.

### `struct`

Binary layouts. Covered in [Chapter 10](ch10-00-binary-data.md).

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

`convert` handles every base-to-base conversion you would otherwise write
by hand, plus hex, binary, octal and unicode helpers.

## Text and Markup

### `html`

A WHATWG-conformant parser, a real DOM, and CSS selectors:

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

The DOM supports traversal, mutation and serialisation, which makes it a
scraper, a templating backend and a sanitiser in one module.

### `wire`

Templating, with directives expressed as HTML attributes rather than a
second syntax layered over your markup:

```zuri
import wire

echo wire.render_string('<p x-text="msg"></p>', { msg: 'hi' })
```

```console
<p>hi</p>
```

Everything is escaped by default, and escaped correctly for where it sits:
a value in an attribute, in a URL and in a `<script>` block are three
different escapes, and Wire knows which is which because it parses your
template as structure rather than text.

[Chapter 14](ch14-00-wire.md) is the full treatment.

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

`detect(file)` sniffs content rather than trusting the extension, which is
the check you want on an upload.

### `colors`

ANSI colour for terminal output, and conversion between every colour space
you are likely to have a value in:

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

The conversions are what make the same code work on a terminal that cannot
do what you asked: a true-colour value becomes the nearest of 256, and 256
becomes the nearest of 16. `hex()`, `rgb()`, `hsl()`, `hsv()`, `hwb()`,
`cmyk()` and `xyz()` each take a colour in that space and produce the
escape sequence for it.

## Time

### `date`

```zuri
import date

var d = date.date(2026, 9, 11, 8, 30, 0)

echo d.format('Y-m-d H:i:s')
echo d.format('l, jS F Y')
echo date.parse('2026-09-11').format('Y-m-d')
```

```console
2026-09-11 08:30:00
Friday, 11th September 2026
2026-09-11
```

The format codes are single letters: `Y` four-digit year, `m` zero-padded
month, `d` zero-padded day, `H` 24-hour, `i` minutes, `s` seconds, `l`
weekday name, `F` month name, `jS` day with an ordinal suffix.

`localtime()` and `gmtime()` give the current time, `from_time(seconds)`
converts a Unix timestamp, and the module carries a real IANA time zone
database, so `from_timezone('Europe/London', ...)` does the right thing
across a daylight-saving boundary.

## Cryptography and Identity

### `hash`

Digests and HMACs. Covered in [Chapter 10](ch10-00-binary-data.md).

### `bcrypt`

Password hashing, which is a different problem from digesting:

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

Use this for passwords and `hash` for everything else. `needs_rehash()`
tells you when a stored hash was made with a lower cost than you now
require.

### `crypto`

RSA signing and HKDF key derivation.

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

Versions 1, 3, 4, 5, 6, 7 and 8 are all there. `v4` is the random one you
usually want; `v7` is time-ordered, which makes it a better database key.

### `jwt`

Signing, verifying and decoding JSON Web Tokens, with a JWKS client for
rotating keys.

## Validation and Structure

### `validate`

A fluent schema builder:

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

`check_or_raise()` raises instead of returning. `extend()`, `only()` and
`except()` build one schema from another, which is how a create schema and
an update schema stay in sync.

### `types`

Type predicates, one per type, as an alternative to the `is_*` built-ins:

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

Each one **answers a question** and returns a boolean; none of them
converts anything. `types.int(4.2)` is `false` because `4.2` is not an
integer, not because it failed to become one.

`types.of()` is `typeof()`. `digit()`, `alpha()` and `char()` are the
string-shape tests the built-ins do not cover, and `instance(value, Class)`
walks the inheritance chain.

When you want conversion rather than a question, the methods on the value
do it: `to_number()`, `to_string()`, `to_bigint()`, `to_list()`,
`to_bytes()`.

### `set`

```zuri
import set

var s = set.set([1, 2, 2, 3])
echo s.length()
```

```console
3
```

Union, intersection, difference and subset tests, with insertion order
preserved.

### `enum`

```zuri
import enum

var Color = enum.enum(['RED', 'GREEN'])
echo Color.RED
```

```console
0
```

Pass a dictionary instead of a list to choose the values yourself.

### `array`

Typed, fixed-width numeric arrays: `Int8Array` through `Uint64Array`, plus
`FloatArray` and `DoubleArray`. They store values in their declared width
rather than as doubles, which matters for memory and for talking to binary
formats.

```zuri
import array

var ints = array.Int32Array([1, 2, 3])
echo ints.length()
```

```console
3
```

## The System

### `os`

Processes, the filesystem, paths and the environment. Covered in
[Chapter 9](ch09-00-files.md).

`os.exec(command)` runs a shell command and gives you its output.
`os.spawn(command, args, options)` starts a process you can talk to.
`os.on_signal(name, handler)` installs a signal handler.
`os.at_exit(handler)` registers cleanup that runs however the program
ends, and `os.set_exit_code(code)` decides the status it ends with
without ending it there and then.

### `env`

Configuration: a `.env` file read into the process environment, and
values read back out already converted. Covered in
[Chapter 19](ch19-00-env.md).

```zuri,ignore
import env

env.load()

var port = env.int('PORT', 8080)
var debug = env.bool('DEBUG', false)
var secret = env.require('SESSION_SECRET')
```

Names already set in the environment are left alone, so the file is a
set of defaults and the deployment is what overrides them.

### `io`

Standard streams, terminal control and in-memory files:

```zuri,ignore
import io

var name = io.readline('Your name: ')
var secret = io.readline('Password: ', true)

echo io.stdout.is_tty()
```

`io.TTY` puts the terminal into raw mode, reads single keypresses and moves
the cursor, which is what an interactive program needs. `io.BytesIO` is the
in-memory file from [Chapter 10](ch10-00-binary-data.md).

`io.capture(body)` collects everything `body` writes to standard output
instead of printing it, `echo`, `print()` and `io.stdout` alike:

```zuri
import io

def greet(name) {
  echo 'Hello, ${name}!'
}

var out = io.capture(@{ greet('Ada') })

echo 'captured ${out.length()} characters'
```

```console
captured 12 characters
```

Captures nest, and `capture_begin()`/`capture_end()` are the manual pair
for when the body might raise and you want its output anyway. This is
what lets [Chapter 22](ch22-00-testing.md) assert on what a function
prints, and keep a passing test's output out of the report.

### `test`

Suites, matchers, mocks, snapshots and reports. Covered in
[Chapter 22](ch22-00-testing.md).

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

The `S_IS*` predicates over the `mode` word from `file().stats()`, plus a
renderer for it:

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
`S_ISSOCK` each answer one question about the kind of entry.
`S_IMODE(mode)` strips the type bits and leaves the permissions;
`file_mode(mode)` renders the whole thing the way `ls -l` does.

### `args`

A command-line parser with subcommands, typed options, automatic
`--help` and wrapped terminal output.

### `log`

```zuri
import log

log.info('server started')
log.error('connection refused')
```

A `Logger` binds structured fields, a `child()` logger inherits them, and
transports send records to the console, a file or somewhere you write
yourself.

### `isolate`

Concurrency. Covered in [Chapter 11](ch11-00-isolates.md).

## The Network

### `net`

TCP, UDP, unix domain sockets, TLS, DTLS, addresses and polling. Covered
in [Chapter 12](ch12-00-networking.md).

### `http`

Client and server, HTTP/1.1 and HTTP/2, with routing, middleware,
WebSockets, server-sent events, multipart uploads, static files and a
reverse proxy. Introduced in [Chapter 12](ch12-00-networking.md), covered
fully in [Chapter 15](ch15-00-http.md), and used throughout
[Chapter 24](ch24-00-task-board.md).

## Databases

### `sql`

One way to talk to a relational database, whichever one it is. `sql`
defines what an adapter has to provide and supplies everything that is
the same across engines: parameters, transactions and savepoints,
cursors, pooling, introspection, and one error hierarchy. SQLite,
PostgreSQL and MySQL adapters ship with it, and changing between them
means changing the connection string.

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

Messages and the three protocols that move them, written in Zuri from
the socket up. `mail.message()` builds a message out of text, HTML and
files and works out the MIME tree from what went in; `mail.parse()`
reads one back. `mail.smtp` sends, `mail.imap` reads mail where it is
kept, and `mail.pop3` takes it away. Both server ends are here too: an
SMTP server that decides what to accept through handlers of your own,
and an IMAP server that answers out of a mail store, of which one keeps
mail on disk in Maildir format and one keeps it in the process.
`mail.dkim` signs outgoing mail and checks incoming mail.

```zuri,ignore
import mail

mail.send('smtp://mail.example.com', mail.message({
  from: 'reports@example.com',
  to: 'ann@example.com',
  subject: 'Quarterly report',
  text: 'The numbers are in.',
}), { username: 'reports', password: secret })
```

Covered in [Chapter 18](ch18-00-mail.md).

## Compression

### `compress`

`deflate`, `zlib`, `gzip`, `zstd`, `lz4`, `bzip2` and `brotli`, plus `tar`
and `zip` archives and `checksum` for CRC32 and Adler-32. Covered in
[Chapter 10](ch10-00-binary-data.md).

## Graphics

### `imagine`

Image creation and manipulation on an RGBA buffer: drawing primitives,
text with a built-in stroke font, filters, colour-space conversion, and
reading and writing the common formats.

```zuri
import imagine { Image }

Image(400, 200, '#0f172a')
  .fill_circle(200, 100, 70, '#38bdf8')
  .circle(200, 100, 70, 'white', { thickness: 3 })
  .save('badge.png')
```

Almost every method returns an image, so operations chain.
`Image.open(path)` decodes an existing file, with the format taken from the
contents rather than the extension:

```zuri,ignore
Image.open('photo.jpg')
  .thumbnail(400, 400)
  .save('thumb.webp')
```

Decoding and encoding are native; everything between them — filters,
drawing, colour conversion — is ordinary Zuri you can read in `libs/imagine`
and extend. [Chapter 16](ch16-00-imagine.md) is the full treatment.

## The Language Itself

### `zuri`

Lexing, parsing, compiling and reflection, all reachable from Zuri code:

```zuri
import zuri

echo zuri.tokenize('var x = 1').length() > 0
echo zuri.reflect.kind([1])
```

```console
true
list
```

[Chapter 20](ch20-00-metaprogramming.md) is the full treatment.

### `math`

The constants. Everything else is a method on `number`; see
[Chapter 4](ch04-02-numbers.md).

## Finding the Rest

Every module's source is in `libs/`, and every public function in it
carries a doc block with its parameters, its defaults and its edge cases.
Reading `libs/set.zu` is a faster way to learn `set` than any summary, and
the standard library is written to be read.
