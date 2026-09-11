# `list`

39 methods. See [Lists](ch04-03-lists.md) for the guided introduction.

| Method | Returns |
| --- | --- |
| [`length()`](#length) | `number` |
| [`append(value: any)`](#append) | `list` |
| [`clear()`](#clear) |  |
| [`clone()`](#clone) | `list` |
| [`count(value)`](#count) | `number` |
| [`extend(list: list)`](#extend) | `list` |
| [`index_of(value: any, start_index: ?int)`](#index_of) | `number` |
| [`insert(value: any, index: int)`](#insert) | `list` |
| [`pop()`](#pop) | `any` |
| [`shift(count: ?int)`](#shift) | `any` |
| [`remove_at(index: int)`](#remove_at) | `any` |
| [`remove(value: any)`](#remove) | `any` |
| [`reverse()`](#reverse) | `list` |
| [`sort()`](#sort) | `list` |
| [`contains(value: any)`](#contains) | `boolean` |
| [`delete(start: int, end: int)`](#delete) | `number` |
| [`first()`](#first) | `any` |
| [`last()`](#last) | `any` |
| [`is_empty()`](#is_empty) | `boolean` |
| [`take(n: int)`](#take) | `list` |
| [`get(index: int)`](#get) | `any` |
| [`compact()`](#compact) | `list` |
| [`unique()`](#unique) | `list` |
| [`zip(...)`](#zip) | `list` |
| [`zip_from(list: list)`](#zip_from) | `list` |
| [`to_dict()`](#to_dict) | `dict` |
| [`each(callback: function)`](#each) |  |
| [`map(callback: function)`](#map) | `list` |
| [`filter(callback: function)`](#filter) | `list` |
| [`reduce(callback: function, initial: any)`](#reduce) | `any` |
| [`some(callback: function)`](#some) | `boolean` |
| [`every(callback: function)`](#every) | `boolean` |
| [`find(callback: function)`](#find) | `list` |
| [`find_index(callback: function)`](#find_index) | `list` |
| [`find_last(callback: function)`](#find_last) | `list` |
| [`find_last_index(callback: function)`](#find_last_index) | `list` |
| [`find_all(callback: function)`](#find_all) | `list` |
| [`partition(callback: function)`](#partition) | `list` |
| [`to_string()`](#to_string) | `string` |

## `length()`

Returns the number of items in the list.

For example:

```zuri
%> ['A', 'B', 'C'].length()
3
```

- **Returns** `number`

## `append(value: any)`

Adds the given value _x_ to the end of the list.

For example:

```zuri
%> var a = [1,2,3]
%> a.append(4)
%> a
[1, 2, 3, 4]
```

- **Parameter** `any` value
- **Returns** `list`

## `clear()`

Removes all items from the list.

For example:

```zuri
%> var a = [1,2,3,4,5]
%> a
[1, 2, 3, 4, 5]
%> a.clear()
%> a
[]
```

## `clone()`

Returns a new list containing all items from the _list_. The
new list is a shallow copy of the original list. This is
equivalent to `list[,]`.

For example:

```zuri
%> var a = [1, 2, 3]
%> var b = a.clone()
%> a.append(4)
%> a
[1, 2, 3, 4]
%> b
[1, 2, 3]
```

- **Returns** `list`

## `count(value)`

Returns the number of times item _x_ occurs in the list.

For example:

```zuri
%> [1, 2, 1, 3, 2, 1, 1].count(1)
4
```

- **Parameter** `any` value
- **Returns** `number`

## `extend(list: list)`

Updates the content of the _list_ by appending all the
contents of list _x_ to the end of the original list in
exact order. This is equivalent to `list + x`.

For example:

```zuri
%> var a = [1, 2, 3]
%> var b = [4, 5, 6]
%> a.extend(b)
%> a
[1, 2, 3, 4, 5, 6]
%> b
[4, 5, 6]
```

- **Parameter** `list` list
- **Returns** `list`

## `index_of(value: any, start_index: ?int)`

Returns the zero-based index of the first occurrence of the
value _x_ in the list starting from the given _start_index_
or `-1` if the list does not contain the value _x_.

For example:

```zuri
%> [1,2].index_of(3)
-1
%> [4,5,6,5].index_of(5)
1
%> ['a', 'b', 'r', 'a', 'h', 'a', 'm'].index_of('a')
0
%> ['a', 'b', 'r', 'a', 'h', 'a', 'm'].index_of('a', 1)
3
```

- **Parameter** `any` value
- **Parameter** `?int` start_index
- **Returns** `number`

## `insert(value: any, index: int)`

Inserts the item _x_ into the list at the specified _index_.
By specifying an index of zero (`list.insert(x, 0)`), one
can prepend the list and `list.insert(x, list.length())` is
equivalent to `list.append(x)`. If the _index_ specified is
greater than `list.length()`, the list will be padded with
`nil` up till the index preceding the specified index.

For example:

```zuri
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

- **Parameter** `any` value
- **Parameter** `int` index
- **Returns** `list`

## `pop()`

Removes the last item in a list and returns the value of
that item.

For example:

```zuri
%> var a = [4, 5, 6]
%> a.pop()
6
%> a
[4, 5]
```

- **Returns** `any`

## `shift(count: ?int)`

Removed the specified count of items from the beginning of
the list and returns it. If _count_ is not specified,
_count_ defaults to 1. If one item is shifted, the method
returns that item. If more than one item is shifted, the
method returns a list containing the shifted items.

> The square brackets (`[]`) around the _`count: number`_ in
> the method definition indicates that the parameter is
> optional and does not mean you have to type the square
> brackets.

If the number of items required to be shifted exceeds the
size of the list, the list is cleared and `nil` is returned.

For example:

```zuri
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

- **Parameter** `?int` count
- **Returns** `any`

## `remove_at(index: int)`

Removes the item at the specified index in the list and
returns it. If the index is less than `0` or greater than
`list.length() - 1`, an Error is raised.

For example:

```zuri
%> var a = [1, 2, 3, 4, 5]
%> a.remove_at(3)
4
%> a
[1, 2, 3, 5]
%> a.remove_at(6)
Unhandled Error: list index 6 out of range at remove_at()
  StackTrace:
    <repl>:1 -> @.script()
%> a.remove_at(-1)
Unhandled Error: list index -1 out of range at remove_at()
  StackTrace:
    <repl>:1 -> @.script()
```

- **Parameter** `int` index
- **Returns** `any`

## `remove(value: any)`

Removes the first occurrence of item _x_ from the list.

For example:

```zuri
%> var a = ['Kirk', 'Tasha', 'Emily', 'Kirk']
%> a.remove('Kirk')
%> a
[Tasha, Emily, Kirk]
```

Notice that only the first occurrence of `Kirk` was removed.

- **Parameter** `any` value
- **Returns** `any`

## `reverse()`

Returns a new list containing the items in the original list
in reverse order.

For example:

```zuri
%> var a = ['apple', 'mango', 'banana', 'orange', 'peach']
%> a.reverse()
[peach, orange, banana, mango, apple]
```

- **Returns** `list`

## `sort()`

Sorts the items in the list in-place and returns the sorted
list. Sorting in Lists follows are strict set of precedence
based on the object type. The order for sorting is as
follows in ascending orders:

`nil`, boolean, numbers, strings, ranges, lists,
dictionaries, file, bytes, functions, classes and modules.

When the corresponding items in the list are of the same
type, they are sorted based on their respective values
according to the type. For example, the number `5` is less
than `8` and as such will appear first in the sort.

For example:

```zuri
%> var a  = ['A', 5, false, nil, [21, 13, 46]]
%> a.sort()
%> a
[nil, false, 5, A, [13, 21, 46]]
```

> Notice how the boolean value precedes the number and how
> the number in turn precedes the string and the strings in
> turn, precedes the list in the result. Also, note that the
> items of the inner list is sorted.

- **Returns** `list`

## `contains(value: any)`

Returns `true` if the list contains the item _x_ or `false`
otherwise.

For example:

```zuri
%>  ['dog', 'cat', 'wolf', 'tiger'].contains('cat')
true
%>  ['dog', 'cat', 'wolf', 'tiger'].contains('giraffe')
false
```

- **Parameter** `any` value
- **Returns** `boolean`

## `delete(start: int, end: int)`

Deletes a range of items from the list starting from the
start to the end limit and returns the number of items
removed. If the start and end are the same, this will be
equivalent to `list.remove_at(start)`.

For example:

```zuri
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

- **Parameter** `int` start
- **Parameter** `int` end
- **Returns** `number`

## `first()`

Returns the first item in the list or `nil` if the list is
empty.

For example:

```zuri
%> ['c', 'd', 'a', 'b'].first()
'c'
```

- **Returns** `any`

## `last()`

Returns the last item in the list or `nil` if the list is
empty.

For example:

```zuri
%> ['c', 'd', 'a', 'b'].last()
'b'
```

- **Returns** `any`

## `is_empty()`

Returns `true` if the list is empty or `false` otherwise.

For example:

```zuri
%> [1, 2].is_empty()
false
%> [].is_empty()
true
```

- **Returns** `boolean`

## `take(n: int)`

Returns a new list containing the first _n_ items in the
list or a new copy of the list if _n_ greater than or equals
to the `list.length()`. If `n < 0`, returns
`list.take(list.length() - n)`.

For example:

```zuri
%> var a = [10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20]
%> a.take(4)
[10, 11, 12, 13]
%> a.take(11) # taking more than the size of the list
[10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20]
%> a.take(-5)   # taking n < 0
[10, 11, 12, 13, 14, 15]
```

- **Parameter** `int` n
- **Returns** `list`

## `get(index: int)`

Returns the value at the specified index in the list. If
_index_ is outside the boundary of the list indexes
(`0..(list.length() - 1)`), an Error is thrown. This method
is equivalent to `list[index]`.

For example:

```zuri
%> [13, 14, 15, 16].get(1)
14
%> [13, 14, 15, 16].get(6)
Unhandled Error: list index 6 out of range at get()
  StackTrace:
    <repl>:1 -> @.script()
```

- **Parameter** `int` index
- **Returns** `any`

## `compact()`

Returns a new list containing the items in the original list
but with all `nil` values removed.

For example:

```zuri
%> [21, nil, 14, 'age', nil, nil, [], 11].compact()
[21, 14, age, [], 11]
```

- **Returns** `list`

## `unique()`

Returns a new list containing the unique values from the
original list.

For example:

```zuri
%> [1, 1, 3, 5].unique()
[1, 3, 5]
```

- **Returns** `list`

## `zip(...)`

Returns a list that contains the items in the original list
merged with corresponding items from the individual
arguments. This generates a list of length equal to the
length of the original argument.

If the size of any of the arguments is less than the size of
the original list, it's corresponding entry will be `nil`.

For example:

```zuri
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

- **Parameter** `...list` lists
- **Returns** `list`

## `zip_from(list: list)`

The same as `list.zip()` except that instead of accepting an
arbitrary list or arguments, it accepts a single list that
should contain other lists.

For example:

```zuri
%> [1, 2].zip_from([[3, 4]])
[[1, 3], [2, 4]]
```

- **Parameter** `list` list
- **Returns** `list`

## `to_dict()`

Returns a number indexed dictionary representing the list.

For example:

```zuri
%> ['English', 'French', 'Spanish'].to_dict()
{0: English, 1: French, 2: Spanish}
```

- **Returns** `dict`

## `each(callback: function)`

Iterates over each element in the list, calling the provided
callback function with the current element as an argument.

   element in the list.
Example:
```zuri
['A', 'B', 'C'].each(@(r) {
  echo r
})
# Output: A B C
```
> The `each` method does not return a new list; it simply
> executes the callback for each element. If you want to
> create a new list based on the original, consider using
> the `map` method instead.

- **Parameter** `function` callback The function to execute for each
- **Raises** `Error` if the callback is not a function.

## `map(callback: function)`

Creates a new list populated with the results of calling a
provided function on every element in the calling list.

   element in the list. It receives the current element and
   its index as arguments.
Example:
```zuri
echo [1, 2, 3].map(@(x) {
  return x * 2
})
# Output: [2, 4, 6]
```

- **Parameter** `function` callback The function to execute on each
- **Raises** `Error` if the callback is not a function.
- **Returns** `list`

## `filter(callback: function)`

Creates a new list with all elements that pass the test
implemented by the provided function.

It returns a new list with the elements that pass the test.
If no elements pass the test, an empty list will be
returned.

   of the list. It receives the current element and its
   index as arguments.
Example:
```zuri
echo [1, 2, 3].filter(@(x) {
  return x % 2 == 0
})
# Output: [2]
```

- **Parameter** `function` callback The function to test each element
- **Raises** `Error` if the callback is not a function.
- **Returns** `list`

## `reduce(callback: function, initial: any)`

Applies a function against an accumulator and each element
in the list (from left to right) to reduce it to a single
value and returns the accumulated result of the callback
function.

   element in the list. It receives the current element and
   its index as arguments.
   If no initial value is provided, the first element of the
   list will be used as the initial accumulator, and the
   iteration will start from the second element.
Example:
```zuri
echo [1, 2, 3].reduce(@(acc, x) {
  return acc + x
})
# Output: 6
```

- **Parameter** `function` callback The function to execute on each
- **Raises** `Error` if the callback is not a function.
- **Parameter** initial The initial value to use as the accumulator.
- **Returns** `any`

## `some(callback: function)`

Tests whether at least one element in the list passes the
test implemented by the provided function.

   of the list. It receives the current element and its
   index as arguments.
Example:
```zuri
echo [1, 2, 3].some(@(x) {
  return x % 2 == 0
})
# Output: true
```
The `some` method returns `true` if the callback function
returns a truthy value for at least one element in the list.
If the callback function returns a falsy value for all
elements, `some` will return `false`. If the list is empty,
`some` will return `false` by default.

- **Parameter** `function` callback The function to test each element
- **Raises** `Error` if the callback is not a function.
- **Returns** `boolean`

## `every(callback: function)`

Tests whether all elements in the list pass the test
implemented by the provided function.

   of the list. It receives the current element and its
   index as arguments.
Example:
```zuri
echo [1, 2, 3].every(@(x) {
  return x > 0
})
# Output: true
```
The `every` method returns `true` if the callback function
returns a truthy value for every element in the list. If the
callback function returns a falsy value for any element,
`every` will return `false`. If the list is empty, `every`
will return `true` by default.

- **Parameter** `function` callback The function to test each element
- **Raises** `Error` if the callback is not a function.
- **Returns** `boolean`

## `find(callback: function)`

Returns the value of the first element in the list that
satisfies the provided testing function. If no elements
satisfy the testing function, `find` returns `nil`.

   of the list. It receives the current element and its
   index as arguments.
Example:
```zuri
echo [1, 2, 3].find(@(x) {
  return x % 2 == 0
})
# Output: 2
```

- **Parameter** `function` callback The function to test each element
- **Raises** `Error` if the callback is not a function.
- **Returns** `list`

## `find_index(callback: function)`

Returns the index of the first element in the list that
satisfies the provided testing function. If no elements
satisfy the testing function, `find_index` returns `-1`.

   of the list. It receives the current element and its
   index as arguments.
Example:
```zuri
echo [1, 2, 3].find_index(@(x) {
  return x % 2 == 0
})
# Output: 1
```

- **Parameter** `function` callback The function to test each element
- **Raises** `Error` if the callback is not a function.
- **Returns** `list`

## `find_last(callback: function)`

Returns the value of the last element in the list that
satisfies the provided testing function. If no elements
satisfy the testing function, `find_last` returns `nil`.

   of the list. It receives the current element and its
   index as arguments.
Example:
```zuri
echo [1, 2, 3].find_last(@(x) {
  return x % 2 == 0
})
# Output: 2
```

- **Parameter** `function` callback The function to test each element
- **Raises** `Error` if the callback is not a function.
- **Returns** `list`

## `find_last_index(callback: function)`

Returns the index of the last element in the list that
satisfies the provided testing function. If no elements
satisfy the testing function, `find_last_index` returns
`-1`.

   of the list. It receives the current element and its
   index as arguments.
Example:
```zuri
echo [1, 2, 3].find_last_index(@(x) {
  return x % 2 == 0
})
# Output: 1
```

- **Parameter** `function` callback The function to test each element
- **Raises** `Error` if the callback is not a function.
- **Returns** `list`

## `find_all(callback: function)`

Returns a new list containing all elements of the calling
list that satisfy the provided testing function.

   of the list. It receives the current element and its
   index as arguments.
Example:
```zuri
echo [1, 2, 3].find_all(@(x) {
  return x % 2 == 0
})
# Output: [2]
```

- **Parameter** `function` callback The function to test each element
- **Raises** `Error` if the callback is not a function.
- **Returns** `list`

## `partition(callback: function)`

Returns an list containing two lists: the first with
elements that satisfy the provided testing function, and the
second with elements that do not satisfy the testing
function.

   of the list. It receives the current element and its
   index as arguments.
Example:
```zuri
echo [1, 2, 3].partition(@(x) {
  return x % 2 == 0
})
# Output: [[2], [1, 3]]
```

- **Parameter** `function` callback The function to test each element
- **Raises** `Error` if the callback is not a function.
- **Returns** `list`

## `to_string()`

Returns the string representation of the list.

```zuri
%> [1, 'two', 3].to_string()
'[1, two, 3]'
```

- **Returns** `string`
