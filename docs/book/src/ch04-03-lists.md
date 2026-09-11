# Lists

A list is an ordered, growable sequence of any values.

```zuri
var items = [3, 1, 4, 1, 5]
var mixed = [1, 'two', [3], { four: 4 }]
var empty = []
```

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

`l[a, b]` slices from `a` up to but not including `b`. `l[, b]` starts at
the beginning, `l[a, ]` runs to the end.

Indexing out of range raises an error. `get()` is the safe form, and takes
an optional fallback:

```zuri
echo [1, 2, 3].get(1)
echo [1, 2, 3].get(10, 'fallback')
```

```console
2
fallback
```

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

`index_of()` gives `-1` when the value is absent.

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

```zuri
[10, 20].each(@(value, index) {
  echo '${index}: ${value}'
})
```

```console
0: 10
1: 20
```

Every callback in this chapter receives **the value first and the index
second**. `for` gives them to you the other way round, key first. That is
not an inconsistency to memorise away; `each` is a callback over values,
and `for` is a loop over pairs.

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
