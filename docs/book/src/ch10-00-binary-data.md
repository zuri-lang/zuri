# Binary Data and Streams

Text is convenient. Protocols, file formats, images and checksums are not
text, and this chapter is about the four tools Zuri gives you for them:
the `bytes` type, the `struct` module, `io.BytesIO`, and `compress`.

## The `bytes` Type

`bytes` is a mutable buffer of 8-bit values.

```zuri
var b = bytes(3)
var c = bytes([1, 2, 3, 4, 5])
var d = 'Hi'.to_bytes()

echo b
echo c
echo d
```

```console
(00 00 00)
(01 02 03 04 05)
(48 69)
```

`bytes(n)` allocates `n` zero bytes. `bytes(list)` builds one from numbers,
each of which must be in `0..256`. `to_bytes()` on a string gives you its
UTF-8 encoding.

The hexadecimal-in-parentheses rendering is how `bytes` always prints,
which makes it impossible to confuse one with a list at a glance.

### Reading

```zuri
var b = bytes([1, 2, 3, 4, 5])

echo b.length()
echo b[0]
echo b[1, 3]
echo b.first()
echo b.last()
echo b.get(2)
echo b.index_of(3)
echo b.last_index_of(3)
echo b.to_list()
echo 'Hi'.to_bytes().to_string()
```

```console
5
1
(02 03)
1
5
3
2
2
[1, 2, 3, 4, 5]
Hi
```

Indexing gives a **number**. Slicing gives `bytes`. `to_string()` decodes
as UTF-8, `to_list()` gives numbers.

`index_of()` and `last_index_of()` both return `-1` when the byte is not
there, and both take a second argument bounding where a match may sit,
so they search the two halves either side of one index.

### Slicing

`b[a, b]` takes the bytes from `a` up to but **not including** `b`, and
returns a new byte stream:

```zuri
var b = bytes([10, 20, 30, 40, 50])

echo b[1, 3]
echo b[, 3]
echo b[3, ]
echo b[-2, ]
```

```console
(14 1e)
(0a 14 1e)
(28 32)
(28 32)
```

The rules are exactly the list's. Either bound may be omitted: `b[, n]`
starts at the beginning and `b[n, ]` runs to the end. Negative bounds count
back from the end, so `b[-2, ]` is the last two bytes.

Note the difference between an index and a slice, because for `bytes` the
two return **different types**:

```zuri
var b = bytes([10, 20, 30])

echo b[0]
echo typeof(b[0])
echo b[0, 1]
echo typeof(b[0, 1])
```

```console
10
number
(0a)
bytes
```

One index gives you the numeric value of that byte. A slice of length one
gives you a byte stream containing it. Reaching for `b[0]` when you meant
`b[0, 1]` is the most common slip here, and it shows up as a `number` where
a `bytes` was expected rather than as an error at the slicing site.

#### Bounds Are Checked

A slice that runs past the end raises rather than returning what it can:

```zuri
var b = bytes([10, 20, 30, 40, 50])

catch {
  echo b[1, 99]
} as e {
  echo '${e.type}: ${e.message}'
}
```

```console
RangeError: slice bounds 1..99 out of range (length 5)
```

`length()` itself is always a legal upper bound, because the bound is
exclusive, and an empty slice is legal rather than an error:

```zuri
var b = bytes([10, 20, 30, 40, 50])

echo b[0, b.length()]
echo b[2, 2]
echo b[2, 2].is_empty()
```

```console
(0a 14 1e 28 32)
()
true
```

An empty byte stream prints as `()`.

#### A Slice Is a Copy

Slicing allocates a new stream, so writing through one does not disturb the
original:

```zuri
var original = bytes([10, 20, 30])
var part = original[0, 2]

part[0] = 99

echo original
echo part
```

```console
(0a 14 1e)
(63 14)
```

That matters when you are parsing a buffer. Pulling a header out with
`frame[0, 4]` gives you something you can modify freely, and the frame you
are still reading from is untouched. It also means slicing in a loop copies
every time, so a parser that walks a large buffer should carry an offset
and slice once per field rather than re-slicing the remainder each step.

#### Slice, Then Decode

The common shape when a buffer holds text with a known extent:

```zuri
var b = bytes([72, 101, 108, 108, 111, 33])

echo b[0, 5].to_string()
echo b.to_string()
```

```console
Hello
Hello!
```

`to_string()` decodes the whole stream it is called on, so the slice is
what limits the extent. Slicing on a byte boundary in the middle of a
multi-byte character produces a stream that is not valid UTF-8; decode
whole units, or keep the tail for the next read.

#### There Is No Slice Assignment

A slice can be read but not written to:

```zuri,ignore
var b = bytes([10, 20, 30])

b[0, 2] = bytes([1, 1])
```

```console
SyntaxError: invalid assignment target
```

Assign to one index at a time, or rebuild the stream with `extend()`. A
single index does accept assignment, and the value has to be a real byte:

```zuri
var b = bytes([72, 101, 108, 108, 111, 33])

b[0] = 74

echo b.to_string()

catch {
  b[1] = 300
} as e {
  echo '${e.type}: ${e.message}'
}
```

```console
Jello!
NumericError: bytes element must be an integer in 0..=255, got 300
```

Note the difference from `bytes([300])`, which wraps rather than raising.
Construction is lenient; assignment is not.

### Writing

```zuri
var b = bytes([1, 2, 3])

b.append(4)
b.extend(bytes([9]))
echo b
echo b.pop()
echo b.reverse()
```

```console
(01 02 03 04 09)
9
(04 03 02 01)
```

Unlike a list, `bytes.reverse()` **mutates in place**. So do `append`,
`extend`, `pop` and `remove`.

`dispose()` releases the buffer's memory immediately rather than waiting
for the collector, which matters when you have just finished with something
very large.

### Splitting

`split()` takes a `bytes` delimiter, not a number:

```zuri
echo bytes([1, 2, 3, 4, 5]).split(bytes([3]))
```

```console
[(01 02), (04 05)]
```

### Walking

A byte stream is iterable, and every form that works on a list works here.
What you get out is always a **number** between 0 and 255, never a
one-character string.

```zuri
var b = bytes([72, 105])

for value in b {
  echo value
}
```

```console
72
105
```

Two variables give you the index first and the value second:

```zuri
var b = bytes([72, 105])

for index, value in b {
  echo '${index}: ${value}'
}
```

```console
0: 72
1: 105
```

`iter` is the form to use when you are decoding a structure and the
position drives the walk — reading a two-byte length, then skipping that
many bytes, then reading the next field:

```zuri
var b = bytes([72, 105, 33])

iter var i = 0; i < b.length(); i += 2 {
  echo '${i}: ${b[i]}'
}
```

```console
0: 72
2: 33
```

And `each()` takes a function, with the **value first and the index
second** as everywhere else:

```zuri
bytes([72, 105]).each(@(value, index) {
  echo '${index}=${value}'
})
```

```console
0=72
1=105
```

When you want the characters rather than the numbers, convert first:
`b.to_string()` decodes the whole stream as UTF-8, and `b.to_list()` gives
you the numbers as an ordinary list.

## Binary Files

Append `b` to the file mode and reads give you `bytes` instead of a string,
and writes accept `bytes`:

```zuri
var out = file('data.bin', 'wb')
out.write(bytes([1, 2, 3]))
out.close()

echo file('data.bin', 'rb').read()
```

```console
(01 02 03)
```

Reading a non-UTF-8 file without the `b` fails rather than silently
producing replacement characters, which is the behaviour you want: a
decoding failure is information.

## `struct`: Packing and Unpacking

`struct` converts between Zuri values and a fixed binary layout. A format
string is a sequence of `/`-separated fields, each `CODE[COUNT][:NAME]`.

```zuri
import struct

var header = struct.pack('N:magic/n:version/Z8:name/C:flags',
  0x5A555249, 1, 'task', 3)

echo header
echo header.length()
echo struct.unpack('N:magic/n:version/Z8:name/C:flags', header)
```

```console
(5a 55 52 49 00 01 74 61 73 6b 00 00 00 00 03)
15
{magic: 1515541065, version: 1, name: task, flags: 3}
```

`unpack()` hands back a dictionary keyed by the `:NAME` you gave each
field. Fields with no name get numbered:

```zuri
echo struct.unpack('N/N', struct.pack('N2', 7, 8))
```

```console
{1: 7, 2: 8}
```

A count greater than one under a single name numbers the keys:

```zuri
echo struct.unpack('n3:vals', struct.pack('n3', 1, 2, 3))
```

```console
{vals1: 1, vals2: 2, vals3: 3}
```

### The Format Codes

Strings:

| Code | Size | Meaning |
| --- | --- | --- |
| `a` | count | NUL-padded string |
| `A` | count | space-padded string |
| `Z` | count | NUL-padded and NUL-terminated, like C |
| `h` / `H` | ceil(count/2) | hex string, low or high nibble first |

Integers. The letter tells you the width and the byte order:

| Code | Size | Meaning |
| --- | --- | --- |
| `c` / `C` | 1 | signed / unsigned 8-bit |
| `?` | 1 | boolean |
| `s` / `S` | 2 | signed / unsigned 16-bit, native order |
| `n` / `v` | 2 | unsigned 16-bit, big- / little-endian |
| `i` `l` / `I` `L` | 4 | signed / unsigned 32-bit, native order |
| `N` / `V` | 4 | unsigned 32-bit, big- / little-endian |
| `q` / `Q` | 8 | signed / unsigned 64-bit, native order |
| `J` / `P` | 8 | unsigned 64-bit, big- / little-endian |
| `u` / `U` | 16 | signed / unsigned 128-bit, little-endian |

Floats:

| Code | Size | Meaning |
| --- | --- | --- |
| `f` / `g` / `G` | 4 | float: native / little / big endian |
| `d` / `e` / `E` | 8 | double: native / little / big endian |
| `w` / `W` | 2 | half-precision: little / big endian |

Padding:

| Code | Meaning |
| --- | --- |
| `x` | write `count` NUL bytes, consumes no argument |
| `X` | back up `count` bytes |
| `@` | seek to absolute position `count` |

`n`, `N` and `J` are the network-order codes, which is what you want for
almost every wire protocol.

A count of `*` means "the rest".

### Precision

Zuri numbers are doubles, so they hold integers exactly only up to 2^53.
Every 64-bit and 128-bit code (`q`, `Q`, `J`, `P`, `u`, `U`) automatically
produces a **bigint** when the unpacked value falls outside that range,
rather than quietly losing digits. Packing accepts either.

### The Rest of the Module

`calcsize(format)` gives the fixed byte size of a format with no `*` in it.
`pack_into(buffer, offset, format, ...)` and `unpack_from(format, buffer,
offset)` work on an existing buffer at a position, which is how you build
one large frame without allocating and concatenating per field.

## `io.BytesIO`

`BytesIO` is a file-shaped object backed by memory. It implements the same
interface `file()` handles do, which means anything that takes a file takes
a `BytesIO` too:

```zuri
import io

var buffer = io.BytesIO(bytes(0), 'w')

buffer.write('hello ')
buffer.write('world')

echo buffer.source.to_string()
```

```console
hello world
```

The constructor takes a `bytes` source and a mode. It has `read`, `gets`,
`write`, `puts`, `seek`, `tell`, `flush`, `close` and `stats`, so a
function written against files needs no changes to work in memory.

That is the useful part: test a function that writes a file without
touching the disk, and parse an in-memory buffer with code written for a
stream.

## Compression

The `compress` module has a submodule per format, each with `compress()`
and `decompress()`:

```zuri
import compress

var raw = ('the quick brown fox ' * 20).to_bytes()

echo raw.length()
echo compress.gzip.compress(raw).length()
echo compress.deflate.compress(raw).length()
echo compress.zstd.compress(raw).length()
echo compress.brotli.compress(raw).length()
echo compress.bzip2.compress(raw).length()
echo compress.lz4.compress(raw).length()
```

```console
400
47
29
41
31
75
37
```

The formats are `deflate`, `zlib`, `gzip`, `zstd`, `lz4`, `bzip2` and
`brotli`. `zlib` is re-exported at the top level, so `compress.compress()`
and `compress.decompress()` are the zlib pair:

```zuri
echo compress.decompress(compress.compress(raw)).to_string() == raw.to_string()
```

```console
true
```

Which to reach for: `gzip` when something else has to read it, `zstd` when
you want the best ratio-to-speed trade, `lz4` when speed is the only thing
that matters, `brotli` for text you will serve over HTTP.

### Archives

`compress.tar` and `compress.zip` read and write archives, and
`compress.checksum` has `crc32()` and `adler32()`:

```zuri
import compress

echo compress.checksum.crc32('hello'.to_bytes())
```

```console
907060870
```

## Hashing and Encoding

`base64` moves binary through text channels:

```zuri
import base64

var encoded = base64.encode('hello'.to_bytes())

echo encoded
echo base64.decode(encoded).to_string()
```

```console
aGVsbG8=
hello
```

`hash` covers the digest algorithms:

```zuri
import hash

echo hash.sha256('hello')
echo hash.hmac_sha256('key', 'hello')
```

```console
2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824
9307b3b915efb5171ff14d8cb55fbcc798c6c0ef1456d66ded1a6aa723a58b7b
```

Every function takes an optional final `as_bytes` argument; pass `true` to
get raw `bytes` instead of a hex string.

The algorithms are md2, md4, md5, sha1, sha224, sha256, sha384, sha512, the
sha3 family, shake128/256, blake2b512, blake2s256, ripemd160, whirlpool and
gost, each with an `hmac_` counterpart, plus `pbkdf2()` for key derivation.
`hash.hash(algorithm, data)` selects by name at runtime.

For password storage, use `bcrypt` rather than any of these. A fast hash is
the wrong tool for a password, and [Chapter 13](ch13-00-stdlib-tour.md)
covers the right one.

## A Worked Example: A Length-Prefixed Frame

Most binary protocols are a header followed by a payload. Here is the whole
round trip:

```zuri
import struct
import compress

var HEADER = 'N:length/C:compressed'

def encode_frame(payload) {
  var body = payload.to_bytes()
  var compressed = 0

  if body.length() > 128 {
    body = compress.gzip.compress(body)
    compressed = 1
  }

  var frame = struct.pack(HEADER, body.length(), compressed)
  frame.extend(body)

  return frame
}

def decode_frame(frame) {
  var size = struct.calcsize('N/C')
  var header = struct.unpack(HEADER, frame[0, size])
  var body = frame[size, size + header.length]

  if header.compressed == 1 {
    body = compress.gzip.decompress(body)
  }

  return body.to_string()
}

var frame = encode_frame('ping')
echo frame
echo decode_frame(frame)

echo decode_frame(encode_frame('the quick brown fox ' * 20)).length()
```

```console
(00 00 00 04 00 70 69 6e 67)
ping
400
```

`calcsize()` tells you where the header ends, slicing gives you the two
halves, and the `compressed` flag is one byte because a protocol that has
to guess is a protocol that breaks.
