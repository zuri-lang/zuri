# Bytes Methods

Every method on the built-in `bytes` type, with its signature, what it
returns, and the cases where it does something other than the obvious
thing.

| Method | Returns | Summary |
| --- | --- | --- |
| [`bytes(n: number\|list)`](#bytes) | `bytes` | Creates a new byte stream. |
| [`length()`](#length) | `number` | Returns the number of bytes in the byte stream. |
| [`is_empty()`](#is_empty) | `boolean` | Returns `true` if the byte stream holds no bytes at all, and `false` otherwise. |
| [`append(n: int)`](#append) | `bytes` | Adds an item to the top of a byte stream. |
| [`clone()`](#clone) | `bytes` | Returns a deep clone of the byte stream. |
| [`extend(n: bytes)`](#extend) | `bytes` | Extends the byte stream with the bytes from the given byte stream. |
| [`index_of(byte: int, start_index: ?number)`](#index_of) | `number` | Returns the index of the first occurrence of the given byte in the byte stream. |
| [`pop()`](#pop) | `number` | Removes the last item in a byte stream and returns it. |
| [`remove(index: number)`](#remove) | `bytes` | Removes the item at the specified index in the byte stream and return the previous value at the specified index. |
| [`reverse()`](#reverse) | `bytes` | Reverses the items in the byte stream. |
| [`first()`](#first) | `number` | Returns the first item in the byte stream or `nil` if the byte stream is empty. |
| [`last()`](#last) | `number` | Returns the last item in the byte stream or `nil` if the byte stream is empty. |
| [`get(index: number)`](#get) | `number` | Returns the item at the specified index in the byte stream. |
| [`take(n: int)`](#take) | `bytes` | Returns a new byte stream containing the first _n_ items in the bytes or a new copy of the bytes if _n_ greater than or equals to the `bytes.length()`. |
| [`split(delimiter: bytes)`](#split) | `list` | Splits the content of a byte stream based on the specified delimiter. |
| [`dispose()`](#dispose) |  | Due to the nature of byte stream and their use-case (especially streaming data), it is easy for the system memory to get filled up with data in the byte stream. |
| [`is_alpha()`](#is_alpha) | `boolean` | Returns `true` if the byte stream only contains alpha characters, `false` otherwise. |
| [`is_alnum()`](#is_alnum) | `boolean` | Returns `true` if the byte stream only contains alpha characters and numbers, `false` otherwise. |
| [`is_number()`](#is_number) | `boolean` | Returns `true` if the byte stream only contains numbers, `false` otherwise. |
| [`is_lower(n)`](#is_lower) | `boolean` | Returns `true` if the byte stream only contains lower case characters, `false` otherwise. |
| [`is_upper()`](#is_upper) | `boolean` | Returns `true` if the byte stream only contains upper case characters, `false` otherwise. |
| [`is_space()`](#is_space) | `boolean` | Returns `true` if the byte stream only contains space characters, `false` otherwise. |
| [`to_list()`](#to_list) | `list` | Returns the byte stream as a list of bytes. |
| [`to_string()`](#to_string) | `string` | Returns the byte stream as a string. |
| [`each(callback: function)`](#each) | `void` | Iterates over each byte of the bytes object, calling the provided callback function with the byte and its index. |

## `bytes()`

```zuri,ignore
bytes(n: number|list) -> bytes
```

Creates a new byte stream.

For example,

```zuri,ignore
%> bytes(5)
(0 0 0 0 0)
%> bytes([65, 66, 67, 68, 69])
(41  42 43 44 45)
```

**Parameters**

- `n` (`number|list`) — The number of bytes or the list of bytes to
  create.

**Returns** `bytes`

> **Note:** If a number is given, creates an array of size number

> **Note:** If a list is given, converts the bytes list into an array of
> bytes.

## `length()`

```zuri,ignore
length() -> number
```

Returns the number of bytes in the byte stream.

```zuri,ignore
%> bytes([25, 57]).length()
2
```

**Returns** `number`

## `is_empty()`

```zuri,ignore
is_empty() -> boolean
```

Returns `true` if the byte stream holds no bytes at all, and `false`
otherwise. Equivalent to testing `length() == 0`, and unaffected by
whatever the bytes happen to contain: a stream of zero bytes is not
empty.

```zuri,ignore
%> bytes(0).is_empty()
true
%> bytes(3).is_empty()
false
%> 'hi'.to_bytes().is_empty()
false
```

**Returns** `boolean`

## `append()`

```zuri,ignore
append(n: int) -> bytes
```

Adds an item to the top of a byte stream.

For example,

```zuri,ignore
%> var a = bytes([0x40, 0x75])
%> a.append(0x16)
%> echo a
(40 75 16)
```

**Parameters**

- `n` (`int`) — The byte to add.

**Returns** `bytes`

## `clone()`

```zuri,ignore
clone() -> bytes
```

Returns a deep clone of the byte stream.

For example,

```zuri,ignore
%> bytes([19, 11]).clone()
(13 b)
```

**Returns** `bytes`

## `extend()`

```zuri,ignore
extend(n: bytes) -> bytes
```

Extends the byte stream with the bytes from the given byte stream.

For example,

```zuri,ignore
%> var a = bytes([33, 91, 126])
%> var b = bytes([119, 42])
%> a
(21 5b 7e)
%> b
(77 2a)
%> a.extend(b)
(21 5b 7e 77 2a)
%> a
(21 5b 7e 77 2a)
```

**Parameters**

- `n` (`bytes`) — The byte stream to extend with.

**Returns** `bytes`

> **Note:** `extend()` is an in-place action so the original byte stream
> will be modified.

## `index_of()`

```zuri,ignore
index_of(byte: int, start_index: ?number) -> number
```

Returns the index of the first occurrence of the given byte in the byte
stream.

```zuri,ignore
%> bytes([25, 57, 25]).index_of(57)
1
%> bytes([25, 57, 25]).index_of(25, 1)
2
```

**Parameters**

- `byte` (`int`) — The byte to search for.
- `start_index` (`?number`) — The index to start the search from.
  Defaults to 0.

**Returns** `number`

## `pop()`

```zuri,ignore
pop() -> number
```

Removes the last item in a byte stream and returns it.

```zuri,ignore
%> var a = bytes([79, 43, 9])
%> a.pop()
9
%> a
(4f 2b)
```

**Returns** `number`

## `remove()`

```zuri,ignore
remove(index: number) -> bytes
```

Removes the item at the specified index in the byte stream and return
the previous value at the specified index.

```zuri,ignore
%> var a = bytes([25, 57, 25])
%> a.remove(1)
57
%> a
(25 25)
```

**Parameters**

- `index` (`number`) — The index to remove.

**Returns** `bytes`

## `reverse()`

```zuri,ignore
reverse() -> bytes
```

Reverses the items in the byte stream.

```zuri,ignore
%> bytes([5, 4, 3, 2, 1]).reverse()
(1 2 3 4 5)
```

**Returns** `bytes`

## `first()`

```zuri,ignore
first() -> number
```

Returns the first item in the byte stream or `nil` if the byte stream is
empty.

```zuri,ignore
%> bytes([25, 57, 42]).first()
25
```

**Returns** `number`

## `last()`

```zuri,ignore
last() -> number
```

Returns the last item in the byte stream or `nil` if the byte stream is
empty.

```zuri,ignore
%> bytes([25, 57, 42]).last()
42
```

**Returns** `number`

## `get()`

```zuri,ignore
get(index: number) -> number
```

Returns the item at the specified index in the byte stream.

**Parameters**

- `index` (`number`) — The index to get the item from.

**Returns** `number`

## `take()`

```zuri,ignore
take(n: int) -> bytes
```

Returns a new byte stream containing the first _n_ items in the bytes or
a new copy of the bytes if _n_ greater than or equals to the
`bytes.length()`. If `n < 0`, returns `bytes.take(bytes.length() - n)`.

For example:

```zuri,ignore
%> var a = bytes([10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20])
%> a.take(4)
(0a 0b 0c 0d)
%> a.take(11) # taking more than the size of the bytes
(0a 0b 0c 0d 0e 0f 10 11 12 13 14)
%> a.take(-5)   # taking n < 0
(0a 0b 0c 0d 0e 0f)
```

**Parameters**

- `n` (`int`)

**Returns** `bytes`

## `split()`

```zuri,ignore
split(delimiter: bytes) -> list
```

Splits the content of a byte stream based on the specified delimiter.

For example,

```zuri,ignore
%> bytes(0).split(bytes(0))
[]
%> echo 'test'.to_bytes().split(bytes(0))
[(74), (65), (73), (74)]
```

**Parameters**

- `delimiter` (`bytes`) — The delimiter to split on.

**Returns** `list`

## `dispose()`

```zuri,ignore
dispose()
```

Due to the nature of byte stream and their use-case (especially
streaming data), it is easy for the system memory to get filled up with
data in the byte stream. The method allows users to reset a byte stream
and empty it.

> This method allows a fine-grained control on manual memory
> management of byte stream.

For example,

```zuri,ignore
%> var a = bytes([13, 36])
%> a.dispose()
%> a
()
```

## `is_alpha()`

```zuri,ignore
is_alpha() -> boolean
```

Returns `true` if the byte stream only contains alpha characters,
`false` otherwise.

```zuri,ignore
%> bytes([65, 66, 67]).is_alpha()
true
%> bytes([65, 66, 67, 128]).is_alpha()
false
```

**Returns** `boolean`

## `is_alnum()`

```zuri,ignore
is_alnum() -> boolean
```

Returns `true` if the byte stream only contains alpha characters and
numbers, `false` otherwise.

```zuri,ignore
%> bytes([65, 66, 67, 48, 49, 50]).is_alnum()
true
%> bytes([65, 66, 67, 48, 49, 50, 8]).is_alnum()
false
```

**Returns** `boolean`

## `is_number()`

```zuri,ignore
is_number() -> boolean
```

Returns `true` if the byte stream only contains numbers, `false`
otherwise.

```zuri,ignore
%> bytes([48, 49, 50]).is_number()
true
%> bytes([48, 49, 50, 68]).is_number()
false
```

**Returns** `boolean`

## `is_lower()`

```zuri,ignore
is_lower(n) -> boolean
```

Returns `true` if the byte stream only contains lower case characters,
`false` otherwise.

```zuri,ignore
%> bytes([97, 98, 99]).is_lower()
true
%> bytes([97, 98, 99, 68]).is_lower()
false
```

**Returns** `boolean`

## `is_upper()`

```zuri,ignore
is_upper() -> boolean
```

Returns `true` if the byte stream only contains upper case characters,
`false` otherwise.

```zuri,ignore
%> bytes([65, 66, 67]).is_upper()
true
%> bytes([65, 66, 67, 98]).is_upper()
false
```

**Returns** `boolean`

## `is_space()`

```zuri,ignore
is_space() -> boolean
```

Returns `true` if the byte stream only contains space characters,
`false` otherwise.

```zuri,ignore
%> bytes([32, 32, 32]).is_space()
true
%> bytes([32, 32, 32, 68]).is_space()
false
```

**Returns** `boolean`

## `to_list()`

```zuri,ignore
to_list() -> list
```

Returns the byte stream as a list of bytes.

```zuri,ignore
%> bytes([0x31, 0x55, 0x149, 0x215]).to_list()
[49, 85, 233, 33]
```

**Returns** `list`

## `to_string()`

```zuri,ignore
to_string() -> string
```

Returns the byte stream as a string.

```zuri,ignore
%> bytes([65, 66, 67, 68, 69]).to_string()
'ABCDE'
```

**Returns** `string`

## `each()`

```zuri,ignore
each(callback: function) -> void
```

Iterates over each byte of the bytes object, calling the provided
callback function with the byte and its index.

Example:

```zuri,ignore
var data = bytes([0x48, 0x65, 0x6C, 0x6C, 0x6F]) # "Hello" in bytes
data.each(def(byte, index) {
  echo 'Byte at index ${index}: ${byte}'
})

# Output:
# Byte at index 0: 72
# Byte at index 1: 101
# Byte at index 2: 108
# Byte at index 3: 108
# Byte at index 4: 111
```

**Parameters**

- `callback` (`function`) — A function that takes two arguments: the
  byte and its index.

**Returns** `void`

**Raises** `Error` if the callback is not a function.
