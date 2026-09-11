# Lists

A list is an ordered, growable sequence. It holds values of any type,
including other lists, and it grows and shrinks as you work with it.

```zuri
var items = [3, 1, 4, 1, 5]
var mixed = [1, 'two', [3], { four: 4 }]
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

Lists carry 39 methods, and they fall into six groups: reading, searching,
adding and removing, ordering, transforming, and walking. This section
takes them in that order. The one distinction to keep in mind throughout
is whether a method **mutates** the list you called it on or returns a new
one, because Zuri's lists do both and the method names do not tell you
which.

## Reading

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

An index counts from zero, and a negative index counts back from the end:

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

`l[-1]` is the last element, which saves writing `l[l.length() - 1]`
everywhere.

Indexing out of range raises rather than returning `nil`:

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

`get()` is the form that can fall back, but **only when you give it
something to fall back to**. `get(i)` with one argument raises exactly like
`l[i]` does:

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

`l[a, b]` takes the elements from `a` up to but **not including** `b`, and
gives you a new list:

```zuri
var l = [1, 2, 3, 4, 5]

echo l[1, 3]
echo l[, 3]
echo l[3, ]
echo l[-2, ]
```

```console
[2, 3]
[1, 2, 3]
[4, 5]
[4, 5]
```

Either bound may be left out: `l[, b]` starts at the beginning and `l[a, ]`
runs to the end. Negative bounds count from the end, just as an index does.

A slice checks its bounds, exactly as an index does. Running past the end
raises rather than returning what it can:

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

The one bound that is always legal is `length()` itself, since a slice's
upper bound is exclusive. `l[0, l.length()]` is the whole list, and
`l[3, 3]` on a three-element list is empty rather than an error.

```zuri
var l = [1, 2, 3]

echo l[0, l.length()]
echo l[3, 3]
```

```console
[1, 2, 3]
[]
```

This same rule governs string slicing, which is why
[Strings](ch04-01-strings.md) uses `s[split_at + 1, s.length()]` rather
than a large number to mean "the rest".

A slice is a copy. Changing one does not touch the other:

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

A list holds lists, and indexes chain:

```zuri
var grid = [[1, 2], [3, 4], [5, 6]]

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

`grid.length()` counts rows, not elements. There is no two-dimensional
index; `grid[1, 0]` is a *slice* from row one to row zero, which is empty.

## Combining Lists

### Concatenation with `+`

`+` joins two lists into a new one, leaving both operands alone:

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

That is the difference between `+` and `extend()`: `a + b` builds a third
list, while `a.extend(b)` modifies `a` in place and returns it. Use `+`
when you want to keep the originals, and `extend()` when you are
accumulating.

### Repetition with `*`

`*` repeats a list a whole number of times:

```zuri
echo [1, 2] * 3
echo [0] * 5
```

```console
[1, 2, 1, 2, 1, 2]
[0, 0, 0, 0, 0]
```

`[0] * 5` is the idiom for a fixed-size list of a starting value, and it is
worth knowing before you write the loop.

The count behaves exactly as it does for strings: zero and any negative
number both produce an empty list, and a fractional count is truncated.

```zuri
echo [1] * 0
echo [1] * -3
```

```console
[]
[]
```

There is one trap. Repetition copies the *elements*, and when an element is
itself a list, all the copies are the same list:

```zuri
var grid = [[0, 0]] * 3

grid[0][0] = 9

echo grid
```

```console
[[9, 0], [9, 0], [9, 0]]
```

One assignment changed every row, because there is only one row. Build
nested structure with a loop, or with `map()`:

```zuri
var grid = 0..3.to_list().map(@(i) => [0, 0])

grid[0][0] = 9

echo grid
```

```console
[[9, 0], [0, 0], [0, 0]]
```

## Comparing Lists

`==` compares lists by value, element by element, however deeply they
nest:

```zuri
echo [1, 2] == [1, 2]
echo [1, [2, 3]] == [1, [2, 3]]
echo [1, 2] == [2, 1]
echo [1, 2] == [1, 2, 3]
```

```console
true
true
false
false
```

Order matters and length matters. To compare two lists as *sets*, sort
copies of them first, or use the `set` module from
[Chapter 13](ch13-00-stdlib-tour.md).

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

`index_of()` gives `-1` when the value is absent. `last_index_of()`
searches from the other end:

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

Both take a second argument bounding *where a match may sit*, so the
pair splits the list at one index: `index_of(x, n)` finds the first
match at or after `n`, `last_index_of(x, n)` the last one at or before
it. Both compare by value, so a list of dictionaries can be searched
with a dictionary literal.

## Adding and Removing

These mutate the list in place:

| Method | Effect |
| --- | --- |
| `append(v)` | add to the end |
| `insert(v, at)` | insert at a position |
| `extend(other)` | append every element of another list |
| `pop()` | remove and return the last element |
| `shift()` | remove and return the first element |
| `remove(v)` | remove the first element equal to `v` |
| `remove_at(i)` | remove by position |
| `delete(from, to)` | remove an inclusive range, return how many went |
| `clear()` | empty it |

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

Note that `delete()` takes a **from** and a **to**, both inclusive, and
returns the number of elements removed rather than the list.

## Ordering

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

Look closely at those two. **`sort()` mutates and returns the list.
`reverse()` returns a new list and leaves the original alone.** That
asymmetry is the single most common source of list bugs in Zuri code.

`sort()` takes no comparator. To sort by a computed key, decorate, sort,
and undecorate:

```zuri
var people = [{ name: 'Ada', age: 36 }, { name: 'Bob', age: 24 }]

var by_age = people
  .map(@(p) => [p.age, p.name])
  .sort()
  .map(@(pair) => pair[1])

echo by_age
```

```console
[Bob, Ada]
```

## Transforming

Every one of these returns a new list and leaves the receiver untouched:

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

`compact()` drops `nil` entries:

```zuri
echo [1, nil, 2, nil].compact()
```

```console
[1, 2]
```

`reduce()` folds the list down to one value. With no initial value it
starts from the first element:

```zuri
echo [1, 2, 3].reduce(@(acc, x) => acc + x)
echo [1, 2, 3].reduce(@(acc, x) => acc + x, 100)
```

```console
6
106
```

`partition()` splits in one pass, matches first:

```zuri
echo [1, 2, 3, 4].partition(@(x) => x % 2 == 0)
```

```console
[[2, 4], [1, 3]]
```

## Finding

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

## Asking About Every Element

```zuri
echo [1, 2, 3].every(@(x) => x > 0)
echo [1, 2, 3].some(@(x) => x > 2)
```

```console
true
true
```

`some()` stops at the first match; `every()` stops at the first failure.

## Walking

There are four ways to visit every element, and they are not
interchangeable. Pick by what you need in the body.

### `for`, When You Want the Values

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

This is the default. Reach for it whenever the position does not matter.

### `for` With Two Variables, When You Want the Index Too

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

With two variables you get the **key first and the value second**, and for
a list the key is the index. No call to `length()`, no manual counter.

### `iter`, When You Need Control of the Position

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

`iter` costs more typing and earns it only when the traversal is not one
step forward per pass: walking backwards, walking in twos, comparing an
element with its neighbour, or advancing the index from inside the body.

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

### `each()`, When You Have a Function Already

```zuri
[10, 20].each(@(value, index) {
  echo '${index}: ${value}'
})
```

```console
0: 10
1: 20
```

`each()` takes a function, which makes it the one that composes: it chains
onto a `map()` or a `filter()` without a temporary variable, and it accepts
a function you already have by name.

Watch the argument order. **`each()` hands the callback the value first and
the index second**, which is the opposite of `for`. Every list callback in
this chapter follows the same rule — `map`, `filter`, `find`, `some`,
`every` all take `(value, index)` — so it is one thing to remember rather
than several. `for` is a loop over key-value pairs; `each` is a callback
over values that happens to tell you where it is.

`break` and `continue` work inside `for` and `iter`. They do not exist
inside an `each()` callback; `return` there ends that one call, not the
walk. When you need to stop early, use a loop, or `find()` / `some()`,
which stop on their own.

## Combining

```zuri
echo ['a', 'b'].zip([1, 2])
echo [1, 2, 3].zip_from([[4, 5, 6]])
```

```console
[[a, 1], [b, 2]]
[[1, 4], [2, 5], [3, 6]]
```

`zip()` pairs with one other list. `zip_from()` takes a list of lists and
zips across all of them at once.

## Lists Are References

Assigning a list does not copy it:

```zuri
var a = [1, 2]
var b = a
b.append(3)
echo a
```

```console
[1, 2, 3]
```

Use `clone()` when you need an independent copy. The clone is shallow:
nested lists inside it are still shared.

```zuri
var original = [[1, 2], 3]
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

Replacing the top-level `3` affected only the copy. Appending to the
*nested* list affected both, because both lists point at the same inner
list. When you need a deep copy, copy the levels you care about yourself.

## Mutating or Not: The Summary

This is the table to come back to.

| Mutates the receiver | Returns a new list |
| --- | --- |
| `append`, `insert`, `extend` | `map`, `filter`, `find_all` |
| `pop`, `shift`, `remove`, `remove_at` | `unique`, `compact`, `take` |
| `delete`, `clear` | `reverse`, `clone`, `zip`, `zip_from` |
| `sort` | `partition` |

`sort()` is the one that catches people out: it is in the left column, and
it also returns the list, so `var sorted = items.sort()` leaves `items`
sorted too. If you need both orders, clone first:

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

## A Worked Example

A tiny report generator: take a list of records, drop the incomplete ones,
group what is left, and print a summary. It uses `filter`, `map`,
`reduce`, `sort` and `each` together, which is how they usually show up.

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

  records
    .filter(@(r) => r.amount != nil)
    .each(@(r) {
      totals[r.region] = totals.get(r.region, 0) + r.amount
    })

  return totals
}

var totals = totals_by_region(sales)
var grand = totals.values().reduce(@(acc, n) => acc + n, 0)

totals
  .to_list()[0]
  .sort()
  .each(@(region) {
    echo '${region.rpad(6)} ${totals[region]}'
  })

echo 'total  ${grand}'
```

```console
north  165
south  280
total  445
```

`east` is absent from the report, not present with a zero, because its one
record was filtered out before any accumulating happened and nothing ever
created the key. That is usually what you want from a report; when it is
not, seed the dictionary with every region first.

Two other details are worth pulling out. `totals.get(r.region, 0)` supplies
a starting value for a key that does not exist yet, which is what turns a
dictionary into an accumulator. And `totals.to_list()[0]` takes the keys —
`to_list()` returns keys and values as two parallel lists — which are then
sorted so the report comes out in a stable order rather than in whatever
order the records happened to arrive.
