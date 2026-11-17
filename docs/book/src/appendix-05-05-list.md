# List Methods

Every method of the built-in `list` type, with its signature, what it
returns, and any behavior you might not expect.

| Method | Returns | Summary |
| --- | --- | --- |
| [`length()`](#length) | `number` | Returns the number of items in the list. |
| [`append(value)`](#append) |  | Adds _value_ to the end of the list. |
| [`clear()`](#clear) |  | Removes all items from the list. |
| [`clone()`](#clone) | `list` | Returns a new list containing all items from the _list_. |
| [`count(value)`](#count) | `number` | Returns the number of times _value_ occurs in the list. |
| [`extend(list: list)`](#extend) |  | Adds every item of _list_ to the end of this list, in order. |
| [`index_of(value, start_index: ?int)`](#index_of) | `number` | Returns the index of the first occurrence of _value_ in the list, starting the search at _start_index_ if it's given, or `-1` if the list doesn't contain _value_. |
| [`last_index_of(value, end_index: ?int)`](#last_index_of) | `number` | Returns the index of the last occurrence of _value_ in the list, searching from the end, or `-1` if the list doesn't contain _value_. |
| [`insert(value, index: int)`](#insert) |  | Inserts _value_ into the list at _index_. |
| [`pop()`](#pop) | `any` | Removes the last item in a list and returns the value of that item. |
| [`shift(count: ?int)`](#shift) | `any` | Removes _count_ items from the start of the list and returns them. |
| [`remove_at(index: int)`](#remove_at) | `any` | Removes the item at the given index from the list, and returns it. |
| [`remove(value)`](#remove) | `any` | Removes the first occurrence of _value_ from the list. |
| [`reverse()`](#reverse) | `list` | Returns a new list containing the items in the original list in reverse order. |
| [`sort(comparator: ?function)`](#sort) | `list` | Sorts the items in the list in place, and returns the sorted list. |
| [`contains(value)`](#contains) | `boolean` | Returns `true` if the list contains _value_, and `false` otherwise. |
| [`delete(start: int, end: int)`](#delete) | `number` | Deletes a range of items from the list starting from the start to the end limit and returns the number of items removed. |
| [`first()`](#first) | `any` | Returns the first item in the list or `nil` if the list is empty. |
| [`last()`](#last) | `any` | Returns the last item in the list or `nil` if the list is empty. |
| [`is_empty()`](#is_empty) | `boolean` | Returns `true` if the list is empty or `false` otherwise. |
| [`take(n: int)`](#take) | `list` | Returns a new list containing the first _n_ items, or a copy of the whole list if _n_ is greater than or equal to `length()`. |
| [`get(index: int)`](#get) | `any` | Returns the value at the given index. |
| [`compact()`](#compact) | `list` | Returns a new list containing the items in the original list but with all `nil` values removed. |
| [`unique()`](#unique) | `list` | Returns a new list containing the unique values from the original list. |
| [`zip(...lists: list)`](#zip) | `list` | Returns a list that contains the items in the original list merged with corresponding items from the individual arguments. |
| [`zip_from(list: list)`](#zip_from) | `list` | The same as `list.zip()` except that instead of accepting an arbitrary list or arguments, it accepts a single list that should contain other lists. |
| [`to_dict()`](#to_dict) | `dict` | Returns a number indexed dictionary representing the list. |
| [`each(callback: function)`](#each) |  | Calls `callback` for each element in the list, with the element as the argument. |
| [`map(callback: function)`](#map) | `list` | Creates a new list populated with the results of calling a provided function on every element in the calling list. |
| [`filter(callback: function)`](#filter) | `list` | Creates a new list with all elements that pass the test implemented by the provided function. |
| [`reduce(callback: function, initial)`](#reduce) | `any` | Applies a function against an accumulator and each element in the list (from left to right) to reduce it to a single value and returns the accumulated result of the callback function. |
| [`some(callback: function)`](#some) | `boolean` | Tests whether at least one element in the list passes the test implemented by the provided function. |
| [`every(callback: function)`](#every) | `boolean` | Tests whether all elements in the list pass the test implemented by the provided function. |
| [`find(callback: function)`](#find) | `any` | Returns the value of the first element in the list that satisfies the provided testing function. |
| [`find_index(callback: function)`](#find_index) | `number` | Returns the index of the first element in the list that satisfies the provided testing function. |
| [`find_last(callback: function)`](#find_last) | `any` | Returns the value of the last element in the list that satisfies the provided testing function. |
| [`find_last_index(callback: function)`](#find_last_index) | `number` | Returns the index of the last element in the list that satisfies the provided testing function. |
| [`find_all(callback: function)`](#find_all) | `list` | Returns a new list containing all elements of the calling list that satisfy the provided testing function. |
| [`partition(callback: function)`](#partition) | `list` | Splits the list into two lists, and returns them in a list: the first holds the elements for which `callback` returns true, and the second holds the rest. |
| [`to_string()`](#to_string) | `string` | Returns the string representation of the list. |

## `length()`

```zuri,ignore
length() -> number
```

Returns the number of items in the list. <br>

For example:

```zuri-repl
%> ['A', 'B', 'C'].length()
3
```

**Returns** `number`

## `append()`

```zuri,ignore
append(value)
```

Adds _value_ to the end of the list.

For example:

```zuri-repl
%> var a = [1,2,3]
%> a.append(4)
%> a
[1, 2, 3, 4]
```

**Parameters**

- `value` (`any`)

## `clear()`

```zuri,ignore
clear()
```

Removes all items from the list.<br>

For example:

```zuri-repl
%> var a = [1,2,3,4,5]
%> a
[1, 2, 3, 4, 5]
%> a.clear()
%> a
[]
```

## `clone()`

```zuri,ignore
clone() -> list
```

Returns a new list containing all items from the _list_. The new list is
a shallow copy of the original list. This is equivalent to `list[,]`.

For example:

```zuri-repl
%> var a = [1, 2, 3]
%> var b = a.clone()
%> a.append(4)
%> a
[1, 2, 3, 4]
%> b
[1, 2, 3]
```

**Returns** `list`

## `count()`

```zuri,ignore
count(value) -> number
```

Returns the number of times _value_ occurs in the list.

For example:

```zuri-repl
%> [1, 2, 1, 3, 2, 1, 1].count(1)
4
```

**Parameters**

- `value` (`any`)

**Returns** `number`

## `extend()`

```zuri,ignore
extend(list: list)
```

Adds every item of _list_ to the end of this list, in order. Unlike `+`,
which creates a new list, this changes the list it's called on.

For example:

```zuri-repl
%> var a = [1, 2, 3]
%> var b = [4, 5, 6]
%> a.extend(b)
%> a
[1, 2, 3, 4, 5, 6]
%> b
[4, 5, 6]
```

**Parameters**

- `list` (`list`)

## `index_of()`

```zuri,ignore
index_of(value, start_index: ?int) -> number
```

Returns the index of the first occurrence of _value_ in the list,
starting the search at _start_index_ if it's given, or `-1` if the list
doesn't contain _value_.

For example:

```zuri-repl
%> [1,2].index_of(3)
-1
%> [4,5,6,5].index_of(5)
1
%> ['a', 'b', 'r', 'a', 'h', 'a', 'm'].index_of('a')
0
%> ['a', 'b', 'r', 'a', 'h', 'a', 'm'].index_of('a', 1)
3
```

**Parameters**

- `value` (`any`)
- `start_index` (`?int`)

**Returns** `number`

## `last_index_of()`

```zuri,ignore
last_index_of(value, end_index: ?int) -> number
```

Returns the index of the last occurrence of _value_ in the list,
searching from the end, or `-1` if the list doesn't contain _value_.

If _end_index_ is given, only a match at or before that index counts.
That's the same position that the second parameter of `index_of()` uses,
so for any index `n`, `index_of(value, n)` and `last_index_of(value, n)`
give the first and last matches in the two halves that `n` splits the
list into.

Values are compared in the same way as in `index_of()`, by value, not by
identity, so two separate dictionaries with the same entries match each
other.

For example:

```zuri-repl
%> [1,2].last_index_of(3)
-1
%> [4,5,6,5].last_index_of(5)
3
%> ['a', 'b', 'r', 'a', 'h', 'a', 'm'].last_index_of('a')
5
%> ['a', 'b', 'r', 'a', 'h', 'a', 'm'].last_index_of('a', 4)
3
```

**Parameters**

- `value` (`any`)
- `end_index` (`?int`)

**Returns** `number`

## `insert()`

```zuri,ignore
insert(value, index: int)
```

Inserts _value_ into the list at _index_. An index of zero, as in
`list.insert(value, 0)`, adds the value at the start, and
`list.insert(value, list.length())` is the same as `list.append(value)`.
If _index_ is greater than `length()`, the gap is filled with `nil`.

For example:

```zuri-repl
%> var a = [1,2,3]
%> a.insert(4, 0)
%> a
[4, 1, 2, 3]
%> a.insert(5, a.length())
%> a
[4, 1, 2, 3, 5]
%> a.insert(6, 3)
%> a
[4, 1, 2, 6, 3, 5]
%> a.insert(7, 11)
%> a
[4, 1, 2, 6, 3, 5, nil, nil, nil, nil, nil, 7]
```

**Parameters**

- `value` (`any`)
- `index` (`int`)

## `pop()`

```zuri,ignore
pop() -> any
```

Removes the last item in a list and returns the value of that item.<br>

For example:

```zuri-repl
%> var a = [4, 5, 6]
%> a.pop()
6
%> a
[4, 5]
```

**Returns** `any`

## `shift()`

```zuri,ignore
shift(count: ?int) -> any
```

Removes _count_ items from the start of the list and returns them.
_count_ defaults to 1. If one item is removed, the method returns that
item. If more than one is removed, it returns a list of them.

If _count_ is greater than the length of the list, the list is cleared
and `nil` is returned.

For example:

```zuri-repl
%> var a = [9, 8, 7, 6, 5, 4, 3, 2, 1, 0]
%> a.shift()
9
%> a
[8, 7, 6, 5, 4, 3, 2, 1, 0]
%> a.shift(3)
[8, 7, 6]
%> a
[5, 4, 3, 2, 1, 0]
%> a.shift(10)
%> a
[]
```

**Parameters**

- `count` (`?int`)

**Returns** `any`

## `remove_at()`

```zuri,ignore
remove_at(index: int) -> any
```

Removes the item at the given index from the list, and returns it. If
the index is less than `0` or greater than `length() - 1`, an error is
raised.

For example:

```zuri-repl
%> var a = [1, 2, 3, 4, 5]
%> a.remove_at(3)
4
%> a
[1, 2, 3, 5]
%> a.remove_at(6)
Unhandled Error: list index 6 out of range at remove_at()
%> a.remove_at(-1)
Unhandled Error: list index -1 out of range at remove_at()
```

**Parameters**

- `index` (`int`)

**Returns** `any`

## `remove()`

```zuri,ignore
remove(value) -> any
```

Removes the first occurrence of _value_ from the list.

For example:

```zuri-repl
%> var a = ['Kirk', 'Tasha', 'Emily', 'Kirk']
%> a.remove('Kirk')
%> a
[Tasha, Emily, Kirk]
```

Only the first `Kirk` is removed.

**Parameters**

- `value` (`any`)

**Returns** `any`

## `reverse()`

```zuri,ignore
reverse() -> list
```

Returns a new list containing the items in the original list in reverse
order.

For example:

```zuri-repl
%> var a = ['apple', 'mango', 'banana', 'orange', 'peach']
%> a.reverse()
[peach, orange, banana, mango, apple]
```

**Returns** `list`

## `sort()`

```zuri,ignore
sort(comparator: ?function) -> list
```

Sorts the items in the list in place, and returns the sorted list. Items
of different types are sorted by type, in this order:

`nil`, booleans, numbers, strings, ranges, lists, dictionaries, files,
bytes, functions, classes and modules.

Items of the same type are sorted by value, so the number `5` comes
before `8`.

For example:

```zuri-repl
%> var a  = ['A', 5, false, nil, [21, 13, 46]]
%> a.sort()
%> a
[nil, false, 5, A, [13, 21, 46]]
```

> The boolean comes before the number, the number comes
> before the string, and the string comes before the list.
> Lists inside the list are sorted too.

## Sorting by something else

Pass a comparator to choose the order yourself. It's called with two
items, and returns a negative number to put the first item first, a
positive number to put the second item first, or zero to leave them as
they are:

```zuri-repl
%> [3, 1, 2].sort(@(a, b) => b - a)
[3, 2, 1]
%> ['pear', 'fig', 'banana'].sort(@(a, b) => a.length() - b.length())
[fig, pear, banana]
```

The sort is stable, so items that the comparator treats as equal keep
the order they were in. That means you can sort a list by one thing and
then by another to order it by both:

```zuri,ignore
people.sort(@(a, b) => a.name.compare(b.name))
people.sort(@(a, b) => a.age - b.age)
```

This leaves people of the same age in order of name.

**Parameters**

- `comparator` (`function`)

**Returns** `list`

> **Note:** A comparator only sorts the list it's given. Lists inside the
> list, which `sort()` sorts without a comparator, are left as they are,
> because only the comparator knows what the order should be.

> **Note:** A comparator that contradicts itself produces some order, not
> an error, because no order could satisfy it.

## `contains()`

```zuri,ignore
contains(value) -> boolean
```

Returns `true` if the list contains _value_, and `false` otherwise.

For example:

```zuri-repl
%>  ['dog', 'cat', 'wolf', 'tiger'].contains('cat')
true
%>  ['dog', 'cat', 'wolf', 'tiger'].contains('giraffe')
false
```

**Parameters**

- `value` (`any`)

**Returns** `boolean`

## `delete()`

```zuri,ignore
delete(start: int, end: int) -> number
```

Deletes a range of items from the list starting from the start to the
end limit and returns the number of items removed. If the start and end
are the same, this will be equivalent to `list.remove_at(start)`.

For example:

```zuri-repl
%> var a = [1, 2, 3, 4, 5, 6, 7, 8, 9]
%> a.delete(3, 6)
4
%> a
[1, 2, 3, 8, 9]
%> a.delete(1,1)  # equal start and end
1
%> a
[1, 3, 8, 9]
```

**Parameters**

- `start` (`int`)
- `end` (`int`)

**Returns** `number`

## `first()`

```zuri,ignore
first() -> any
```

Returns the first item in the list or `nil` if the list is empty.

For example:

```zuri-repl
%> ['c', 'd', 'a', 'b'].first()
'c'
```

**Returns** `any`

## `last()`

```zuri,ignore
last() -> any
```

Returns the last item in the list or `nil` if the list is empty.

For example:

```zuri-repl
%> ['c', 'd', 'a', 'b'].last()
'b'
```

**Returns** `any`

## `is_empty()`

```zuri,ignore
is_empty() -> boolean
```

Returns `true` if the list is empty or `false` otherwise.

For example:

```zuri-repl
%> [1, 2].is_empty()
false
%> [].is_empty()
true
```

**Returns** `boolean`

## `take()`

```zuri,ignore
take(n: int) -> list
```

Returns a new list containing the first _n_ items, or a copy of the
whole list if _n_ is greater than or equal to `length()`. If _n_ is
negative, this returns all but the last `-n` items, which is the same as
`take(length() + n)`.

For example:

```zuri-repl
%> var a = [10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20]
%> a.take(4)
[10, 11, 12, 13]
%> a.take(11) # more than there are
[10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20]
%> a.take(-5) # all but the last five
[10, 11, 12, 13, 14, 15]
```

**Parameters**

- `n` (`int`)

**Returns** `list`

## `get()`

```zuri,ignore
get(index: int) -> any
```

Returns the value at the given index. If _index_ is outside the list,
from `0` to `length() - 1`, an error is raised. This is the same as
`list[index]`.

For example:

```zuri-repl
%> [13, 14, 15, 16].get(1)
14
%> [13, 14, 15, 16].get(6)
Unhandled Error: list index 6 out of range at get()
```

**Parameters**

- `index` (`int`)

**Returns** `any`

## `compact()`

```zuri,ignore
compact() -> list
```

Returns a new list containing the items in the original list but with
all `nil` values removed.

For example:

```zuri-repl
%> [21, nil, 14, 'age', nil, nil, [], 11].compact()
[21, 14, age, [], 11]
```

**Returns** `list`

## `unique()`

```zuri,ignore
unique() -> list
```

Returns a new list containing the unique values from the original list.

For example:

```zuri-repl
%> [1, 1, 3, 5].unique()
[1, 3, 5]
```

**Returns** `list`

## `zip()`

```zuri,ignore
zip(...lists: list) -> list
```

Returns a list that contains the items in the original list merged with
corresponding items from the individual arguments. This generates a list
of length equal to the length of the original argument.

If the size of any of the arguments is less than the size of the
original list, it's corresponding entry will be `nil`.

For example:

```zuri-repl
%> var a = [4, 5, 6]
%> var b = [7, 8, 9]
%> [1, 2, 3].zip(a, b)
[[1, 4, 7], [2, 5, 8], [3, 6, 9]]
%> [1, 2].zip(a, b)
[[1, 4, 7], [2, 5, 8]]
%> a.zip([1, 2], [8])
[[4, 1, 8], [5, 2, nil], [6, nil, nil]]
%> [1, 2].zip([3])
[[1, 3], [2, nil]]
%> [1].zip([10, 11], [12, 13, 14])
[[1, 10, 12]]
%> [[1, 2], [3]].zip(a, b)
[[[1, 2], 4, 7], [[3], 5, 8]]
```

**Parameters**

- `lists` (`...list`)

**Returns** `list`

## `zip_from()`

```zuri,ignore
zip_from(list: list) -> list
```

The same as `list.zip()` except that instead of accepting an arbitrary
list or arguments, it accepts a single list that should contain other
lists.

For example:

```zuri-repl
%> [1, 2].zip_from([[3, 4]])
[[1, 3], [2, 4]]
```

**Parameters**

- `list` (`list`)

**Returns** `list`

## `to_dict()`

```zuri,ignore
to_dict() -> dict
```

Returns a number indexed dictionary representing the list.

For example:

```zuri-repl
%> ['English', 'French', 'Spanish'].to_dict()
{0: English, 1: French, 2: Spanish}
```

**Returns** `dict`

## `each()`

```zuri,ignore
each(callback: function)
```

Calls `callback` for each element in the list, with the element as the
argument.

Example:

```zuri,ignore
['A', 'B', 'C'].each(@(r) {
  echo r
})

# Output: A B C
```

> `each()` doesn't return a new list. It only calls the
> callback for each element. To create a new list from the
> original one, use `map()` instead.

**Parameters**

- `callback` (`function`): The function to call for each element.

**Raises** `Error` if the callback isn't a function.

## `map()`

```zuri,ignore
map(callback: function) -> list
```

Creates a new list populated with the results of calling a provided
function on every element in the calling list.

Example:

```zuri,ignore
echo [1, 2, 3].map(@(x) {
  return x * 2
})

# Output: [2, 4, 6]
```

**Parameters**

- `callback` (`function`): The function to execute on each element in
  the list. It receives the current element and its index as arguments.

**Returns** `list`

**Raises** `Error` if the callback is not a function.

## `filter()`

```zuri,ignore
filter(callback: function) -> list
```

Creates a new list with all elements that pass the test implemented by
the provided function.

It returns a new list with the elements that pass the test. If no
elements pass the test, an empty list will be returned.

Example:

```zuri,ignore
echo [1, 2, 3].filter(@(x) {
  return x % 2 == 0
})

# Output: [2]
```

**Parameters**

- `callback` (`function`): The function to test each element of the
  list. It receives the current element and its index as arguments.

**Returns** `list`

**Raises** `Error` if the callback is not a function.

## `reduce()`

```zuri,ignore
reduce(callback: function, initial) -> any
```

Applies a function against an accumulator and each element in the list
(from left to right) to reduce it to a single value and returns the
accumulated result of the callback function.

Example:

```zuri,ignore
echo [1, 2, 3].reduce(@(acc, x) {
  return acc + x
})

# Output: 6
```

**Parameters**

- `callback` (`function`): The function to execute on each element in
  the list. It receives the current element and its index as arguments.
- `initial`: The initial value to use as the accumulator. If no initial
  value is provided, the first element of the list will be used as the
  initial accumulator, and the iteration will start from the second
  element.

**Returns** `any`

**Raises** `Error` if the callback is not a function.

## `some()`

```zuri,ignore
some(callback: function) -> boolean
```

Tests whether at least one element in the list passes the test
implemented by the provided function.

Example:

```zuri,ignore
echo [1, 2, 3].some(@(x) {
  return x % 2 == 0
})

# Output: true
```

The `some` method returns `true` if the callback function returns a
truthy value for at least one element in the list. If the callback
function returns a falsy value for all elements, `some` will return
`false`. If the list is empty, `some` will return `false` by default.

**Parameters**

- `callback` (`function`): The function to test each element of the
  list. It receives the current element and its index as arguments.

**Returns** `boolean`

**Raises** `Error` if the callback is not a function.

## `every()`

```zuri,ignore
every(callback: function) -> boolean
```

Tests whether all elements in the list pass the test implemented by the
provided function.

Example:

```zuri,ignore
echo [1, 2, 3].every(@(x) {
  return x > 0
})

# Output: true
```

The `every` method returns `true` if the callback function returns a
truthy value for every element in the list. If the callback function
returns a falsy value for any element, `every` will return `false`. If
the list is empty, `every` will return `true` by default.

**Parameters**

- `callback` (`function`): The function to test each element of the
  list. It receives the current element and its index as arguments.

**Returns** `boolean`

**Raises** `Error` if the callback is not a function.

## `find()`

```zuri,ignore
find(callback: function) -> any
```

Returns the value of the first element in the list that satisfies the
provided testing function. If no elements satisfy the testing function,
`find` returns `nil`.

Example:

```zuri,ignore
echo [1, 2, 3].find(@(x) {
  return x % 2 == 0
})

# Output: 2
```

**Parameters**

- `callback` (`function`): The function to test each element of the
  list. It receives the current element and its index as arguments.

**Returns** `any`: The first element that satisfies it, or `nil`.

**Raises** `Error` if the callback is not a function.

## `find_index()`

```zuri,ignore
find_index(callback: function) -> number
```

Returns the index of the first element in the list that satisfies the
provided testing function. If no elements satisfy the testing function,
`find_index` returns `-1`.

Example:

```zuri,ignore
echo [1, 2, 3].find_index(@(x) {
  return x % 2 == 0
})

# Output: 1
```

**Parameters**

- `callback` (`function`): The function to test each element of the
  list. It receives the current element and its index as arguments.

**Returns** `number`: The index of the first element that satisfies it,
or `-1`.

**Raises** `Error` if the callback is not a function.

## `find_last()`

```zuri,ignore
find_last(callback: function) -> any
```

Returns the value of the last element in the list that satisfies the
provided testing function. If no elements satisfy the testing function,
`find_last` returns `nil`.

Example:

```zuri,ignore
echo [1, 2, 3].find_last(@(x) {
  return x % 2 == 0
})

# Output: 2
```

**Parameters**

- `callback` (`function`): The function to test each element of the
  list. It receives the current element and its index as arguments.

**Returns** `any`: The last element that satisfies it, or `nil`.

**Raises** `Error` if the callback is not a function.

## `find_last_index()`

```zuri,ignore
find_last_index(callback: function) -> number
```

Returns the index of the last element in the list that satisfies the
provided testing function. If no elements satisfy the testing function,
`find_last_index` returns `-1`.

Example:

```zuri,ignore
echo [1, 2, 3].find_last_index(@(x) {
  return x % 2 == 0
})

# Output: 1
```

**Parameters**

- `callback` (`function`): The function to test each element of the
  list. It receives the current element and its index as arguments.

**Returns** `number`: The index of the last element that satisfies it,
or `-1`.

**Raises** `Error` if the callback is not a function.

## `find_all()`

```zuri,ignore
find_all(callback: function) -> list
```

Returns a new list containing all elements of the calling list that
satisfy the provided testing function.

Example:

```zuri,ignore
echo [1, 2, 3].find_all(@(x) {
  return x % 2 == 0
})

# Output: [2]
```

**Parameters**

- `callback` (`function`): The function to test each element of the
  list. It receives the current element and its index as arguments.

**Returns** `list`

**Raises** `Error` if the callback is not a function.

## `partition()`

```zuri,ignore
partition(callback: function) -> list
```

Splits the list into two lists, and returns them in a list: the first
holds the elements for which `callback` returns true, and the second
holds the rest.

Example:

```zuri,ignore
echo [1, 2, 3].partition(@(x) {
  return x % 2 == 0
})

# Output: [[2], [1, 3]]
```

**Parameters**

- `callback` (`function`): The function that tests each element. It's
  called with the element and its index.

**Returns** `list`

**Raises** `Error` if the callback isn't a function.

## `to_string()`

```zuri,ignore
to_string() -> string
```

Returns the string representation of the list.

```zuri-repl
%> [1, 'two', 3].to_string()
'[1, two, 3]'
```

**Returns** `string`
