# `range`

8 methods. See [Ranges](ch04-05-ranges-and-iteration.md) for the guided introduction.

| Method | Returns |
| --- | --- |
| [`lower()`](#lower) | `number` |
| [`upper()`](#upper) | `number` |
| [`within(value: number)`](#within) | `boolean` |
| [`step(size: int)`](#step) | `range` |
| [`get_step()`](#get_step) | `number` |
| [`loop(callback: function)`](#loop) | `void` |
| [`to_list()`](#to_list) | `list` |
| [`to_string()`](#to_string) | `string` |

## `lower()`

Returns the lower limit of the range.

For example:

```zuri
%> (10..100).lower()
10
```

- **Returns** `number`

## `upper()`

Returns the upper limit of the range.

For example:

```zuri
%> (20..30).upper()
30
```

- **Returns** `number`

## `within(value: number)`

Returns true if the given number falls somewhere within the
or false otherwise.

For example:

```zuri
%> (93..21).within(103)
false
%> (93..21).within(57)
true
```

- **Parameter** `number` value
- **Returns** `boolean`

## `step(size: int)`

Sets the step size of the range.

For example:

```zuri
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

- **Parameter** `int` size - The step size of the range.
- **Returns** `range`

## `get_step()`

Returns the step size of the range.

- **Returns** `number`

## `loop(callback: function)`

Iterates over each number in the range, calling the provided
callback function with the number, its index.

   arguments: the number, its index.
Example:
```zuri
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

- **Parameter** `function` callback A function that takes two
- **Raises** `Error` if the callback is not a function.
- **Returns** `void`

## `to_list()`

Returns the range as a list of its individual numbers,
stepping from the lower limit to the upper limit
(exclusive), or in reverse when the range descends.

```zuri
%> (1..5).to_list()
[1, 2, 3, 4]
%> (5..1).to_list()
[5, 4, 3, 2]
```

- **Returns** `list`

## `to_string()`

Returns the string representation of the range.

```zuri
%> (1..5).to_string()
'1..5'
```

- **Returns** `string`
