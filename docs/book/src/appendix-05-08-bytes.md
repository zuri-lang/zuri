# `bytes`

24 methods. See [Bytes](ch10-00-binary-data.md) for the guided introduction.

| Method | Returns |
| --- | --- |
| [`length()`](#length) | `number` |
| [`is_empty()`](#is_empty) | `boolean` |
| [`append(n: int)`](#append) | `bytes` |
| [`clone()`](#clone) | `bytes` |
| [`extend(n: bytes)`](#extend) | `bytes` |
| [`index_of(byte: int, start_index: ?number)`](#index_of) | `number` |
| [`pop()`](#pop) | `number` |
| [`remove(index: number)`](#remove) | `bytes` |
| [`reverse()`](#reverse) | `bytes` |
| [`first()`](#first) | `number` |
| [`last()`](#last) | `number` |
| [`get(index: number)`](#get) | `number` |
| [`take(n: int)`](#take) | `bytes` |
| [`split(delimiter: bytes)`](#split) | `list` |
| [`dispose()`](#dispose) |  |
| [`is_alpha()`](#is_alpha) | `boolean` |
| [`is_alnum()`](#is_alnum) | `boolean` |
| [`is_number()`](#is_number) | `boolean` |
| [`is_lower(n)`](#is_lower) | `boolean` |
| [`is_upper()`](#is_upper) | `boolean` |
| [`is_space()`](#is_space) | `boolean` |
| [`to_list()`](#to_list) | `list` |
| [`to_string()`](#to_string) | `string` |
| [`each(callback: function)`](#each) | `void` |

## `length()`

Returns the number of bytes in the byte stream.

```zuri
%> bytes([25, 57]).length()
2
```

- **Returns** `number`

## `is_empty()`

Returns `true` if the byte stream holds no bytes at all, and
`false` otherwise. Equivalent to testing `length() == 0`, and
unaffected by whatever the bytes happen to contain: a stream of
zero bytes is not empty.

```zuri
%> bytes(0).is_empty()
true
%> bytes(3).is_empty()
false
%> 'hi'.to_bytes().is_empty()
false
```

- **Returns** `boolean`

## `append(n: int)`

Adds an item to the top of a byte stream.

For example,

```zuri
%> var a = bytes([0x40, 0x75])
%> a.append(0x16)
%> echo a
(40 75 16)
```

- **Parameter** `int` n The byte to add.
- **Returns** `bytes`

## `clone()`

Returns a deep clone of the byte stream.

For example,

```zuri
%> bytes([19, 11]).clone()
(13 b)
```

- **Returns** `bytes`

## `extend(n: bytes)`

Extends the byte stream with the bytes from the given byte
stream.

For example,

```zuri
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

   stream will be modified.

- **Note** `extend()` is an in-place action so the original byte
- **Parameter** `bytes` n The byte stream to extend with.
- **Returns** `bytes`

## `index_of(byte: int, start_index: ?number)`

Returns the index of the first occurrence of the given byte
in the byte stream.

```zuri
%> bytes([25, 57, 25]).index_of(57)
1
%> bytes([25, 57, 25]).index_of(25, 1)
2
```

   from. Defaults to 0.

- **Parameter** `int` byte The byte to search for.
- **Parameter** `?number` start_index The index to start the search
- **Returns** `number`

## `pop()`

Removes the last item in a byte stream and returns it.

```zuri
%> var a = bytes([79, 43, 9])
%> a.pop()
9
%> a
(4f 2b)
```

- **Returns** `number`

## `remove(index: number)`

Removes the item at the specified index in the byte stream
and return the previous value at the specified index.

```zuri
%> var a = bytes([25, 57, 25])
%> a.remove(1)
57
%> a
(25 25)
```

- **Parameter** `number` index The index to remove.
- **Returns** `bytes`

## `reverse()`

Reverses the items in the byte stream.

```zuri
%> bytes([5, 4, 3, 2, 1]).reverse()
(1 2 3 4 5)
```

- **Returns** `bytes`

## `first()`

Returns the first item in the byte stream or `nil` if the
byte stream is empty.

```zuri
%> bytes([25, 57, 42]).first()
25
```

- **Returns** `number`

## `last()`

Returns the last item in the byte stream or `nil` if the
byte stream is empty.

```zuri
%> bytes([25, 57, 42]).last()
42
```

- **Returns** `number`

## `get(index: number)`

Returns the item at the specified index in the byte stream.

- **Parameter** `number` index The index to get the item from.
- **Returns** `number`

## `take(n: int)`

Returns a new byte stream containing the first _n_ items in the
bytes or a new copy of the bytes if _n_ greater than or equals
to the `bytes.length()`. If `n < 0`, returns
`bytes.take(bytes.length() - n)`.

For example:

```zuri
%> var a = bytes([10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20])
%> a.take(4)
(0a 0b 0c 0d)
%> a.take(11) # taking more than the size of the bytes
(0a 0b 0c 0d 0e 0f 10 11 12 13 14)
%> a.take(-5)   # taking n < 0
(0a 0b 0c 0d 0e 0f)
```

- **Parameter** `int` n
- **Returns** `bytes`

## `split(delimiter: bytes)`

Splits the content of a byte stream based on the specified
delimiter.

For example,

```zuri
%> bytes(0).split(bytes(0))
[]
%> echo 'test'.to_bytes().split(bytes(0))
[(74), (65), (73), (74)]
```

- **Parameter** `bytes` delimiter The delimiter to split on.
- **Returns** `list`

## `dispose()`

Due to the nature of byte stream and their use-case
(especially streaming data), it is easy for the system
memory to get filled up with data in the byte stream. The
method allows users to reset a byte stream and empty it.

> This method allows a fine-grained control on manual memory
> management of byte stream.

For example,

```zuri
%> var a = bytes([13, 36])
%> a.dispose()
%> a
()
```

## `is_alpha()`

Returns `true` if the byte stream only contains alpha
characters, `false` otherwise.

```zuri
%> bytes([65, 66, 67]).is_alpha()
true
%> bytes([65, 66, 67, 128]).is_alpha()
false
```

- **Returns** `boolean`

## `is_alnum()`

Returns `true` if the byte stream only contains alpha
characters and numbers, `false` otherwise.

```zuri
%> bytes([65, 66, 67, 48, 49, 50]).is_alnum()
true
%> bytes([65, 66, 67, 48, 49, 50, 8]).is_alnum()
false
```

- **Returns** `boolean`

## `is_number()`

Returns `true` if the byte stream only contains numbers,
`false` otherwise.

```zuri
%> bytes([48, 49, 50]).is_number()
true
%> bytes([48, 49, 50, 68]).is_number()
false
```

- **Returns** `boolean`

## `is_lower(n)`

Returns `true` if the byte stream only contains lower case
characters, `false` otherwise.

```zuri
%> bytes([97, 98, 99]).is_lower()
true
%> bytes([97, 98, 99, 68]).is_lower()
false
```

- **Returns** `boolean`

## `is_upper()`

Returns `true` if the byte stream only contains upper case
characters, `false` otherwise.

```zuri
%> bytes([65, 66, 67]).is_upper()
true
%> bytes([65, 66, 67, 98]).is_upper()
false
```

- **Returns** `boolean`

## `is_space()`

Returns `true` if the byte stream only contains space
characters, `false` otherwise.

```zuri
%> bytes([32, 32, 32]).is_space()
true
%> bytes([32, 32, 32, 68]).is_space()
false
```

- **Returns** `boolean`

## `to_list()`

Returns the byte stream as a list of bytes.

```zuri
%> bytes([0x31, 0x55, 0x149, 0x215]).to_list()
[49, 85, 233, 33]
```

- **Returns** `list`

## `to_string()`

Returns the byte stream as a string.

```zuri
%> bytes([65, 66, 67, 68, 69]).to_string()
'ABCDE'
```

- **Returns** `string`

## `each(callback: function)`

Iterates over each byte of the bytes object, calling the
provided callback function with the byte and its index.

   arguments: the byte and its index.
Example:
```zuri
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

- **Parameter** `function` callback A function that takes two
- **Raises** `Error` if the callback is not a function.
- **Returns** `void`
