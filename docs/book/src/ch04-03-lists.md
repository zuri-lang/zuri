# Lists

A list is an ordered sequence of values. It can hold values of any type,
including other lists, and it grows and shrinks as you add and remove
elements.

```zuri
var items = [3, 1, 4, 1, 5]
var mixed = [
  1,
  'two',
  [3],
  { four: 4 },
]
var empty = []

echo items.length()
echo mixed[3].four
echo empty.is_empty()
```

```console
5
4
true
```

Lists have 40 methods, which fall into six groups: reading, searching,
adding and removing, ordering, transforming and iterating. This section goes
through them in that order. Throughout, keep an eye on whether a method
**changes** the list you call it on or returns a new one. Zuri's lists have
both kinds of methods, and you can't tell which is which from the name.

## Reading Elements

```zuri
var l = [3, 1, 4, 1, 5]

echo l.length()
echo l.is_empty()
echo l[0]
echo l[-1]
echo l.first()
echo l.last()
echo l[1, 3]
```

```console
5
false
3
5
3
5
[1, 4]
```

### Indexing

Indexes start at zero, and negative indexes count back from the end:

```zuri
var l = ['a', 'b', 'c', 'd']

echo l[0]
echo l[3]
echo l[-1]
echo l[-4]
```

```console
a
d
d
a
```

`l[-1]` is the last element, which saves you from writing
`l[l.length() - 1]` all the time.

Using an index that's out of range raises an error instead of returning
`nil`:

```zuri
var l = [1, 2, 3]

catch {
  echo l[99]
} as e {
  echo e.message
}
```

```console
index 99 out of bounds (length 3)
```

`get()` can return a fallback value, but **only if you give it one**. With
just one argument, `get(i)` raises an error in the same way as `l[i]`:

```zuri
var l = [1, 2, 3]

echo l.get(1)
echo l.get(99, 'missing')

catch {
  echo l.get(99)
} as e {
  echo e.message
}
```

```console
2
missing
list index 99 out of range at get()
```

### Slicing

`l[a, b]` returns a new list with the elements from `a` up to but **not
including** `b`:

```zuri
var l = [1, 2, 3, 4, 5]

echo l[1, 3]
echo l[,3]
echo l[3,]
echo l[-2,]
```

```console
[2, 3]
[1, 2, 3]
[4, 5]
[4, 5]
```

You can leave out either bound. `l[, b]` starts at the beginning, and
`l[a, ]` goes to the end. Negative bounds count from the end, as they do for
indexes.

Slices check their bounds in the same way as indexes. If a slice goes past
the end, it raises an error instead of returning what it can:

```zuri
catch {
  echo [1, 2, 3][1, 99]
} as e {
  echo e.message
}
```

```console
slice bounds 1..99 out of range (length 3)
```

Since a slice's upper bound is exclusive, `length()` is always a valid upper
bound. `l[0, l.length()]` is the whole list, and `l[3, 3]` on a
three-element list is an empty list rather than an error.

```zuri
var l = [1, 2, 3]

echo l[0, l.length()]
echo l[3, 3]
```

```console
[1, 2, 3]
[]
```

String slicing follows the same rule, which is why
[Strings](ch04-01-strings.md) uses `s[split_at + 1, s.length()]` to mean
"the rest of the string" instead of using some large number.

A slice is a copy, so changing one doesn't affect the other:

```zuri
var original = [1, 2, 3]
var part = original[0, 2]

part[0] = 99

echo original
echo part
```

```console
[1, 2, 3]
[99, 2]
```

### Nested Lists

A list can contain other lists, and you can chain indexes to reach into
them:

```zuri
var grid = [
  [1, 2],
  [3, 4],
  [5, 6],
]

echo grid[1]
echo grid[1][0]
echo grid.length()
echo grid[1].length()
```

```console
[3, 4]
3
3
2
```

`grid.length()` counts the rows, not the individual elements. There's no
two-dimensional index syntax. `grid[1, 0]` is a *slice* from row one to row
zero, so it's empty.

## Combining Lists

### Concatenation with `+`

`+` combines two lists into a new one and leaves the original lists
unchanged:

```zuri
var a = [1, 2]
var b = [3]

echo a + b
echo a
echo b
```

```console
[1, 2, 3]
[1, 2]
[3]
```

That's how `+` differs from `extend()`. `a + b` creates a third list, while
`a.extend(b)` changes `a` in place and returns it. Use `+` when you want to
keep the original lists, and `extend()` when you're adding to an existing
list.

### Repetition with `*`

`*` repeats a list a given number of times:

```zuri
echo [1, 2] * 3
echo [0] * 5
```

```console
[1, 2, 1, 2, 1, 2]
[0, 0, 0, 0, 0]
```

`[0] * 5` is the usual way to create a list of a fixed size filled with a
starting value, so you don't need to write a loop for it.

The count works the same way as it does for strings. Zero or any negative
number gives an empty list, and a fractional count is truncated.

```zuri
echo [1] * 0
echo [1] * -3
```

```console
[]
[]
```

There's one trap to watch for. Repetition copies the *elements*, so when an
element is itself a list, every copy refers to the same list:

```zuri
var grid = [
      [0, 0],
    ] *
    3

grid[0][0] = 9

echo grid
```

```console
[[9, 0], [9, 0], [9, 0]]
```

A single assignment changed every row, because there's really only one row.
To build nested lists, use a loop or `map()` instead:

```zuri
var grid = 0..3.to_list().map(@(i) => [0, 0])

grid[0][0] = 9

echo grid
```

```console
[[9, 0], [0, 0], [0, 0]]
```

## Comparing Lists

`==` compares lists by value, element by element, however deeply they're
nested:

```zuri
echo [1, 2] == [1, 2]
echo [
      1,
      [2, 3],
    ] ==
    [
      1,
      [2, 3],
    ]
echo [1, 2] == [2, 1]
echo [1, 2] == [1, 2, 3]
```

```console
true
true
false
false
```

Both the order and the length have to match. To compare two lists as *sets*,
sort copies of them first, or use the `set` module described in [Chapter
13](ch13-00-stdlib-tour.md).

## Searching

```zuri
var l = [3, 1, 4, 1, 5]

echo l.contains(4)
echo l.index_of(1)
echo l.count(1)
```

```console
true
1
2
```

`index_of()` returns `-1` if the value isn't in the list. `last_index_of()`
searches from the end:

```zuri
var l = [3, 1, 4, 1, 5]

echo l.index_of(1)
echo l.last_index_of(1)
echo l.last_index_of(1, 2)
echo l.last_index_of(9)
```

```console
1
3
1
-1
```

Both methods take a second argument that limits *where a match can be*, so
together they split the list at an index. `index_of(x, n)` finds the first
match at or after `n`, and `last_index_of(x, n)` finds the last match at or
before `n`. Both compare by value, so you can search a list of dictionaries
using a dictionary literal.

## Adding and Removing Elements

These methods change the list in place:

| Method | Effect |
| --- | --- |
| `append(v)` | Adds `v` to the end |
| `insert(v, at)` | Inserts `v` at the given position |
| `extend(other)` | Appends every element of another list |
| `pop()` | Removes and returns the last element |
| `shift()` | Removes and returns the first element |
| `remove(v)` | Removes the first element equal to `v` |
| `remove_at(i)` | Removes the element at position `i` |
| `delete(from, to)` | Removes an inclusive range and returns how many elements were removed |
| `clear()` | Removes every element |

```zuri
var l = [3, 1, 4]

l.insert(9, 1)
echo l
echo l.shift()
echo l
```

```console
[3, 9, 1, 4]
3
[9, 1, 4]
```

```zuri
var l = [1, 2, 3, 4, 5]
echo l.delete(1, 3)
echo l
```

```console
3
[1, 5]
```

Note that `delete()` takes a **from** and a **to** position, both inclusive,
and returns the number of elements it removed, not the list.

## Sorting and Reversing

```zuri
var l = [3, 1, 2]

echo l.sort()
echo l
echo l.reverse()
echo l
```

```console
[1, 2, 3]
[1, 2, 3]
[3, 2, 1]
[1, 2, 3]
```

Take a close look at those two methods. **`sort()` changes the list and
returns it. `reverse()` returns a new list and leaves the original
unchanged.** This difference is the most common cause of list bugs in Zuri
code.

To sort by something else, pass a comparison function. It receives two items
and returns a negative number if the first item should come first, a
positive number if the second should come first, or zero to keep their
current order:

```zuri
var people = [
  { name: 'Ada', age: 36 },
  { name: 'Bob', age: 24 },
]

var by_age = people.sort(@(a, b) => a.age - b.age).map(@(p) => p.name)

echo by_age
```

```console
[Bob, Ada]
```

You can't subtract strings, so to compare text in a comparison function, use
`compare()`, which returns exactly what a comparison function needs:
`people.sort(@(a, b) => a.name.compare(b.name))`.

## Transforming Lists

All of these methods return a new list and leave the original unchanged:

```zuri
var l = [1, 2, 3, 4]

echo l.map(@(x) => x * 2)
echo l.filter(@(x) => x > 2)
echo l.unique()
echo l.compact()
echo l.take(2)
echo l.clone()
```

```console
[2, 4, 6, 8]
[3, 4]
[1, 2, 3, 4]
[1, 2, 3, 4]
[1, 2]
[1, 2, 3, 4]
```

`compact()` removes `nil` elements:

```zuri
echo [1, nil, 2, nil].compact()
```

```console
[1, 2]
```

`reduce()` combines the elements of a list into a single value. If you don't
give it an initial value, it starts with the first element:

```zuri
echo [1, 2, 3].reduce(@(acc, x) => acc + x)
echo [1, 2, 3].reduce(@(acc, x) => acc + x, 100)
```

```console
6
106
```

`partition()` splits a list in one pass, and returns the matching elements
first:

```zuri
echo [1, 2, 3, 4].partition(@(x) => x % 2 == 0)
```

```console
[[2, 4], [1, 3]]
```

## Finding Elements

```zuri
var l = [1, 2, 3]

echo l.find(@(x) => x > 1)
echo l.find_index(@(x) => x > 1)
echo l.find_last(@(x) => x > 1)
echo l.find_last_index(@(x) => x > 1)
echo l.find_all(@(x) => x > 1)
```

```console
2
1
3
2
[2, 3]
```

## Testing Every Element

```zuri
echo [1, 2, 3].every(@(x) => x > 0)
echo [1, 2, 3].some(@(x) => x > 2)
```

```console
true
true
```

`some()` stops at the first element that matches, and `every()` stops at the
first one that doesn't.

## Iterating Over a List

There are four ways to visit every element, and each one suits a different
situation. Choose based on what you need inside the loop.

### `for`, When You Only Need the Values

```zuri
for value in ['a', 'b', 'c'] {
  echo value
}
```

```console
a
b
c
```

This is the default choice. Use it whenever the position doesn't matter.

### `for` With Two Variables, When You Also Need the Index

```zuri
for index, value in ['a', 'b', 'c'] {
  echo '${index}: ${value}'
}
```

```console
0: a
1: b
2: c
```

With two variables, you get the **key first and the value second**, and for
a list the key is the index. There's no need to call `length()` or keep a
counter yourself.

### `iter`, When You Need to Control the Position

```zuri
var items = ['a', 'b', 'c']

iter var i = 0; i < items.length(); i++ {
  echo '${i}: ${items[i]}'
}
```

```console
0: a
1: b
2: c
```

`iter` takes more typing, so it's only worth using when you don't simply
move forward one element at a time, for example when going backward,
stepping by two, comparing each element with its neighbor, or changing the
index inside the loop.

```zuri
var items = ['a', 'b', 'c', 'd']

iter var i = items.length() - 1; i >= 0; i-- {
  echo items[i]
}

iter var i = 0; i < items.length(); i += 2 {
  echo items[i]
}
```

```console
d
c
b
a
a
c
```

### `each()`, When You Already Have a Function

```zuri
[10, 20].each(@(value, index) {
  echo '${index}: ${value}'
})
```

```console
0: 10
1: 20
```

`each()` takes a function, which makes it easy to combine with other
methods. You can chain it after `map()` or `filter()` without a temporary
variable, and you can pass it a function you've already defined by name.

Pay attention to the argument order. **`each()` passes the value first and
the index second**, which is the opposite of `for`. All of the list
callbacks in this chapter, including `map`, `filter`, `find`, `some` and
`every`, take `(value, index)`, so there's only one rule to remember. `for`
loops over key and value pairs, while `each` calls a function with each
value and also tells it the position.

`break` and `continue` work in `for` and `iter` loops, but not inside an
`each()` callback. There, `return` only ends the current call, not the whole
iteration. If you need to stop early, use a loop instead, or `find()` or
`some()`, which stop by themselves.

## Combining

```zuri
echo ['a', 'b'].zip([1, 2])
echo [
  1, 2, 3,
].zip_from([
  [4, 5, 6],
])
```

```console
[[a, 1], [b, 2]]
[[1, 4], [2, 5], [3, 6]]
```

`zip()` pairs up the elements of two lists. `zip_from()` takes a list of
lists and zips all of them together.

## Lists Are Passed by Reference

Assigning a list to another variable doesn't copy it:

```zuri
var a = [1, 2]
var b = a
b.append(3)
echo a
```

```console
[1, 2, 3]
```

Use `clone()` when you need a separate copy. The copy is shallow, which
means any nested lists inside it are still shared.

```zuri
var original = [
  [1, 2],
  3,
]
var copy = original.clone()

copy[1] = 99
copy[0].append(4)

echo original
echo copy
```

```console
[[1, 2, 4], 3]
[[1, 2, 4], 99]
```

Replacing the top-level `3` only changed the copy. Appending to the *nested*
list changed both, because both lists refer to the same inner list. If you
need a deep copy, copy each level you care about yourself.

## Summary: Which Methods Change the List

You'll probably want to refer back to this table.

| Changes the list | Returns a new list |
| --- | --- |
| `append`, `insert`, `extend` | `map`, `filter`, `find_all` |
| `pop`, `shift`, `remove`, `remove_at` | `unique`, `compact`, `take` |
| `delete`, `clear` | `reverse`, `clone`, `zip`, `zip_from` |
| `sort` | `partition` |

`sort()` is the one that trips people up. It's in the left column, but it
also returns the list, so `var sorted = items.sort()` sorts `items` too. If
you need both the original order and the sorted order, clone the list first:

```zuri
var items = [3, 1, 2]
var sorted = items.clone().sort()

echo items
echo sorted
```

```console
[3, 1, 2]
[1, 2, 3]
```

## Example: A Simple Report

Here's a small report generator. It takes a list of records, drops the
incomplete ones, groups the rest, and prints a summary. It uses `filter`,
`map`, `reduce`, `sort` and `each` together, which is how you'll usually see
them.

```zuri
var sales = [
  { region: 'north', amount: 120 },
  { region: 'south', amount: 80 },
  { region: 'north', amount: 45 },
  { region: 'east', amount: nil },
  { region: 'south', amount: 200 },
]

def totals_by_region(records) {
  var totals = {}

  records.filter(@(r) => r.amount != nil).each(@(r) {
    totals[r.region] = totals.get(r.region, 0) + r.amount
  })

  return totals
}

var totals = totals_by_region(sales)
var grand = totals.values().reduce(@(acc, n) => acc + n, 0)

totals.to_list()[0].sort().each(@(region) {
  echo '${region.rpad(6)} ${totals[region]}'
})

echo 'total  ${grand}'
```

```console
north  165
south  280
total  445
```

`east` doesn't appear in the report at all, rather than appearing with a
total of zero. Its only record was filtered out before anything was added
up, so the key was never created. That's usually what you want in a report.
If it isn't, add every region to the dictionary first.

Two other details are worth pointing out. `totals.get(r.region, 0)` provides
a starting value for a key that doesn't exist yet, which is what lets the
dictionary act as an accumulator. And `totals.to_list()[0]` gets the keys
(`to_list()` returns the keys and the values as two parallel lists), which
are then sorted so the report always comes out in the same order, regardless
of the order the records arrived in.
