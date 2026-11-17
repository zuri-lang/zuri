# Binary Data and Streams

Text is easy to work with, but protocols, file formats, images and checksums
aren't text. This chapter covers the four tools Zuri provides for working
with binary data: the `bytes` type, the `struct` module, `io.BytesIO` and
the `compress` module.

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

`bytes(n)` creates a buffer of `n` zero bytes. `bytes(list)` creates one
from a list of numbers, each between 0 and 255. Calling `to_bytes()` on a
string gives you its UTF-8 encoding.

Bytes are always printed as hexadecimal values in parentheses, so you can't
mistake them for a list.

### Reading Bytes

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

Indexing gives you a **number**, and slicing gives you `bytes`.
`to_string()` decodes the bytes as UTF-8, and `to_list()` returns them as a
list of numbers.

`index_of()` and `last_index_of()` both return `-1` if the byte isn't found,
and both take a second argument that limits where a match can be, so you can
search either side of a given index.

### Slicing

`b[a, b]` returns a new byte buffer with the bytes from `a` up to but **not
including** `b`:

```zuri
var b = bytes([10, 20, 30, 40, 50])

echo b[1, 3]
echo b[,3]
echo b[3,]
echo b[-2,]
```

```console
(14 1e)
(0a 14 1e)
(28 32)
(28 32)
```

The rules are the same as for lists. You can leave out either bound:
`b[, n]` starts at the beginning, and `b[n, ]` goes to the end. Negative
bounds count back from the end, so `b[-2, ]` is the last two bytes.

Be careful with the difference between indexing and slicing, because for
`bytes` they return **different types**:

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

An index gives you the numeric value of a single byte, while a slice of
length one gives you a byte buffer that contains that byte. Writing `b[0]`
when you meant `b[0, 1]` is the most common mistake here, and instead of an
error where you slice, it shows up later as a `number` where `bytes` was
expected.

#### Slices Check Their Bounds

A slice that goes past the end raises an error instead of returning as much
as it can:

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

Because the upper bound is exclusive, `length()` itself is always a valid
upper bound, and an empty slice is allowed:

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

An empty byte buffer is printed as `()`.

#### Slices Are Copies

Slicing creates a new buffer, so writing to the slice doesn't change the
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

This matters when you're parsing a buffer. If you take a header out with
`frame[0, 4]`, you can change it freely, and the frame you're reading from
stays the same. It also means that slicing in a loop makes a copy every
time. A parser that works through a large buffer should keep track of an
offset and slice each field once, rather than slicing off the rest of the
buffer at every step.

#### Slicing Before Decoding

This is the usual approach when a buffer contains text of a known length:

```zuri
var b = bytes([72, 101, 108, 108, 111, 33])

echo b[0, 5].to_string()
echo b.to_string()
```

```console
Hello
Hello!
```

`to_string()` decodes the entire buffer it's called on, so the slice is what
limits how much is decoded. If you slice in the middle of a multi-byte
character, the result isn't valid UTF-8. Decode whole characters, or keep
the leftover bytes for the next read.

#### Slices Can't Be Assigned To

You can read a slice, but you can't write to one:

```zuri,ignore
var b = bytes([10, 20, 30])

b[0, 2] = bytes([1, 1])
```

```console
SyntaxError: invalid assignment target
```

Assign one index at a time instead, or rebuild the buffer with `extend()`.
You can assign to a single index, and the value must be a valid byte:

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

This is different from `bytes([300])`, which wraps the value around instead
of raising an error. Creating a buffer is lenient, but assigning to one
isn't.

### Changing Bytes

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

Unlike `reverse()` on a list, `bytes.reverse()` **changes the buffer in
place**. So do `append`, `extend`, `pop` and `remove`.

`dispose()` frees the buffer's memory immediately instead of waiting for the
garbage collector, which can be useful when you've just finished with a very
large buffer.

### Splitting Bytes

`split()` takes a `bytes` delimiter, not a number:

```zuri
echo bytes([1, 2, 3, 4, 5]).split(bytes([3]))
```

```console
[(01 02), (04 05)]
```

### Iterating Over Bytes

A byte buffer is iterable, and every form of iteration that works with lists
also works here. Each value you get is always a **number** between 0 and
255, never a one-character string.

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

With two variables, you get the index first and the value second:

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

Use `iter` when you're decoding a structure and the position determines what
comes next, for example when you read a two-byte length, skip that many
bytes, and then read the next field:

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

`each()` takes a function, which receives the **value first and the index
second**, as it does everywhere else:

```zuri
bytes([72, 105]).each(@(value, index) {
  echo '${index}=${value}'
})
```

```console
0=72
1=105
```

If you want characters rather than numbers, convert the buffer first.
`b.to_string()` decodes the whole buffer as UTF-8, and `b.to_list()` gives
you the numbers as an ordinary list.

## Binary Files

Add `b` to the file mode to read the file as `bytes` instead of a string.
Writes then accept `bytes` too:

```zuri
var out = file('data.bin', 'wb')
out.write(bytes([1, 2, 3]))
out.close()

echo file('data.bin', 'rb').read()
```

```console
(01 02 03)
```

Reading a file that isn't valid UTF-8 without the `b` raises an error
instead of silently inserting replacement characters. That's the behavior
you want, because a decoding failure tells you something.

## `struct`: Packing and Unpacking

`struct` converts between Zuri values and a fixed binary layout. A format
string is a series of fields separated by `/`, each written as
`CODE[COUNT][:NAME]`.

```zuri
import struct

var header = struct.pack(
  'N:magic/n:version/Z8:name/C:flags',
  0x5A555249,
  1,
  'task',
  3,
)

echo header
echo header.length()
echo struct.unpack('N:magic/n:version/Z8:name/C:flags', header)
```

```console
(5a 55 52 49 00 01 74 61 73 6b 00 00 00 00 03)
15
{magic: 1515541065, version: 1, name: task, flags: 3}
```

`unpack()` returns a dictionary, using the `:NAME` you gave each field as
its key. Fields without a name are numbered:

```zuri
echo struct.unpack('N/N', struct.pack('N2', 7, 8))
```

```console
{1: 7, 2: 8}
```

When a single name has a count greater than one, the keys are numbered:

```zuri
echo struct.unpack('n3:vals', struct.pack('n3', 1, 2, 3))
```

```console
{vals1: 1, vals2: 2, vals3: 3}
```

### Format Codes

Strings:

| Code | Size | Meaning |
| --- | --- | --- |
| `a` | count | String padded with NUL bytes |
| `A` | count | String padded with spaces |
| `Z` | count | String padded with NUL bytes and NUL-terminated, as in C |
| `h` / `H` | ceil(count/2) | Hex string, low or high nibble first |

Integers. The letter determines the width and the byte order:

| Code | Size | Meaning |
| --- | --- | --- |
| `c` / `C` | 1 | Signed / unsigned 8-bit |
| `?` | 1 | Boolean |
| `s` / `S` | 2 | Signed / unsigned 16-bit, native byte order |
| `n` / `v` | 2 | Unsigned 16-bit, big endian / little endian |
| `i` `l` / `I` `L` | 4 | Signed / unsigned 32-bit, native byte order |
| `N` / `V` | 4 | Unsigned 32-bit, big endian / little endian |
| `q` / `Q` | 8 | Signed / unsigned 64-bit, native byte order |
| `J` / `P` | 8 | Unsigned 64-bit, big endian / little endian |
| `u` / `U` | 16 | Signed / unsigned 128-bit, little endian |

Floating-point numbers:

| Code | Size | Meaning |
| --- | --- | --- |
| `f` / `g` / `G` | 4 | Float: native / little endian / big endian |
| `d` / `e` / `E` | 8 | Double: native / little endian / big endian |
| `w` / `W` | 2 | Half precision: little endian / big endian |

Padding:

| Code | Meaning |
| --- | --- |
| `x` | Writes `count` NUL bytes without using an argument |
| `X` | Moves back `count` bytes |
| `@` | Moves to the absolute position `count` |

`n`, `N` and `J` use network byte order, which is what almost every network
protocol needs.

A count of `*` means "all of the rest".

### Precision

Zuri numbers are doubles, so they can only represent integers exactly up to
2^53. All of the 64-bit and 128-bit codes (`q`, `Q`, `J`, `P`, `u` and `U`)
automatically return a **bigint** when the unpacked value is outside that
range, instead of silently losing digits. When packing, they accept either a
number or a bigint.

### Other Functions

`calcsize(format)` returns the size in bytes of a format that doesn't
contain `*`. `pack_into(buffer, offset, format, ...)` and
`unpack_from(format, buffer, offset)` work on an existing buffer at a given
position, which lets you build one large frame without creating and
concatenating a new buffer for each field.

## `io.BytesIO`

`BytesIO` is a file-like object stored in memory. It has the same interface
as the handles returned by `file()`, so anything that accepts a file also
accepts a `BytesIO`:

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

The constructor takes a `bytes` value to start from and a mode. `BytesIO`
has `read`, `gets`, `write`, `puts`, `seek`, `tell`, `flush`, `close` and
`stats`, so a function written for files works in memory without any
changes.

This is where it's useful. You can test a function that writes a file
without touching the disk, and you can parse an in-memory buffer with code
that was written for streams.

## Compression

The `compress` module has a submodule for each format, and each submodule
has `compress()` and `decompress()` functions:

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

The available formats are `deflate`, `zlib`, `gzip`, `zstd`, `lz4`, `bzip2`
and `brotli`. `zlib` is also re-exported at the top level, so `compress()`
and `compress.decompress()` use zlib:

```zuri
echo compress.decompress(compress(raw)).to_string() == raw.to_string()
```

```console
true
```

Use `gzip` when another program needs to read the data, `zstd` when you want
the best balance of compression ratio and speed, `lz4` when speed is all
that matters, and `brotli` for text that you'll serve over HTTP.

### Archives

`compress.tar` and `compress.zip` read and write archives, and
`compress.checksum` provides `crc32()` and `adler32()`:

```zuri
import compress

echo compress.checksum.crc32('hello'.to_bytes())
```

```console
907060870
```

## Hashing and Encoding

`base64` lets you send binary data through channels that only accept text:

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

`hash` provides the digest algorithms:

```zuri
import hash

echo hash.sha256('hello')
echo hash.hmac_sha256('key', 'hello')
```

```console
2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824
9307b3b915efb5171ff14d8cb55fbcc798c6c0ef1456d66ded1a6aa723a58b7b
```

Each function takes an optional last argument, `as_bytes`. Pass `true` to
get raw `bytes` instead of a hex string.

The available algorithms are md2, md4, md5, sha1, sha224, sha256, sha384,
sha512, the sha3 family, shake128 and shake256, blake2b512, blake2s256,
ripemd160, whirlpool and gost. Each one has an `hmac_` version as well, and
there's also `pbkdf2()` for key derivation. `hash(algorithm, data)` lets you
choose the algorithm by name at run time.

To store passwords, use `bcrypt` instead of any of these. A fast hash is the
wrong tool for passwords, and [Chapter 13](ch13-00-stdlib-tour.md) covers
the right one.

## Example: A Length-Prefixed Frame

Most binary protocols consist of a header followed by a payload. Here's a
complete round trip:

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

`calcsize()` tells you where the header ends, slicing splits the frame in
two, and the `compressed` flag takes up a byte of its own, so the receiver
never has to guess whether the payload is compressed.
