# Range Methods

Every method on the built-in `range` type, with its signature, what it
returns, and the cases where it does something other than the obvious
thing.

| Method | Returns | Summary |
| --- | --- | --- |
| [`lower()`](#lower) | `number` | Returns the lower limit of the range. |
| [`upper()`](#upper) | `number` | Returns the upper limit of the range. |
| [`range()`](#range) | `number` | Returns a number equal to the numbers between the range. |
| [`within(value: number)`](#within) | `boolean` | Returns true if the given number falls somewhere within the or false otherwise. |
| [`step(size: int)`](#step) | `range` | Sets the step size of the range. |
| [`get_step()`](#get_step) | `number` | Returns the step size of the range. |
| [`loop(callback: function)`](#loop) | `void` | Iterates over each number in the range, calling the provided callback function with the number, its index. |
| [`to_list()`](#to_list) | `list` | Returns the range as a list of its individual numbers, stepping from the lower limit to the upper limit (exclusive), or in reverse when the range descends. |
| [`to_string()`](#to_string) | `string` | Returns the string representation of the range. |

## `lower()`

```zuri,ignore
lower() -> number
```

Returns the lower limit of the range.

For example:

```zuri-repl
%> (10..100).lower()
10
```

**Returns** `number`

## `upper()`

```zuri,ignore
upper() -> number
```

Returns the upper limit of the range.

For example:

```zuri-repl
%> (20..30).upper()
30
```

**Returns** `number`

## `range()`

```zuri,ignore
range() -> number
```

Returns a number equal to the numbers between the range.

For example:

```zuri-repl
%> (21..93).range()
72
```

The result of stays the same irrespective of the direction of the range.
For example, swapping the upper and lower limit of our previous still
returns the same result.

```zuri-repl
%> (21..93).range()
72
```

**Returns** `number`

## `within()`

```zuri,ignore
within(value: number) -> boolean
```

Returns true if the given number falls somewhere within the or false
otherwise.

For example:

```zuri-repl
%> (93..21).within(103)
false
%> (93..21).within(57)
true
```

**Parameters**

- `value` (`number`)

**Returns** `boolean`

## `step()`

```zuri,ignore
step(size: int) -> range
```

Sets the step size of the range.

For example:

```zuri-repl
%> var a = (10..100).step(20)
%> a
<range 10..100, step=20>
%> for i in a {
..   echo i
.. }
10
30
50
70
90
```

**Parameters**

- `size` (`int`) — The step size of the range.

**Returns** `range`

## `get_step()`

```zuri,ignore
get_step() -> number
```

Returns the step size of the range.

**Returns** `number`

## `loop()`

```zuri,ignore
loop(callback: function) -> void
```

Iterates over each number in the range, calling the provided callback
function with the number, its index.

Example:

```zuri,ignore
var r = 0..5 # 0, 1, 2, 3, 4
r.loop(@(num, index) {
  echo 'Number at index ${index}: ${num}'
})

# Output:
# Number at index 0: 0
# Number at index 1: 1
# Number at index 2: 2
# Number at index 3: 3
# Number at index 4: 4
```

**Parameters**

- `callback` (`function`) — A function that takes two arguments: the
  number, its index.

**Returns** `void`

**Raises** `Error` if the callback is not a function.

## `to_list()`

```zuri,ignore
to_list() -> list
```

Returns the range as a list of its individual numbers, stepping from the
lower limit to the upper limit (exclusive), or in reverse when the range
descends.

```zuri-repl
%> (1..5).to_list()
[1, 2, 3, 4]
%> (5..1).to_list()
[5, 4, 3, 2]
```

**Returns** `list`

## `to_string()`

```zuri,ignore
to_string() -> string
```

Returns the string representation of the range.

```zuri-repl
%> (1..5).to_string()
'1..5'
```

**Returns** `string`
