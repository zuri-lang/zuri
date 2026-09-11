# Ranges and Iteration

## Ranges

`a..b` builds a range value: the integers from `a` up to but not including
`b`.

```zuri
var r = 1..10

echo r
echo r.lower()
echo r.upper()
echo r.range()
```

```console
1..10
1
10
9
```

`range()` is the distance between the bounds.

A range is a value, not loop syntax. Store it, pass it to a function,
return it:

```zuri
def page(number) {
  var size = 20
  return (number * size)..((number + 1) * size)
}

echo page(2)
```

```console
40..60
```

### Counting Down

Write the larger bound first and the range runs backwards:

```zuri
for i in 10..1 {
  echo i
}
```

```console
10
9
8
7
6
5
4
3
2
```

The rule is the same in both directions: the first bound is included, the
second is not.

### Stepping

```zuri
for i in (1..10).step(3) {
  echo i
}
```

```console
1
4
7
```

`step()` gives back a new range carrying that stride. `get_step()` reads
it; the default is `1`.

`to_list()` materialises a range into a list at stride one, ignoring any
step you set:

```zuri
echo (1..5).to_list()
```

```console
[1, 2, 3, 4]
```

### Membership

`within()` tests against the bounds **inclusively on both sides**, and it
normalises direction, so `(10..1).within(10)` and `(1..10).within(10)` both
answer the same:

```zuri
var r = 1..10

echo r.within(5)
echo r.within(10)
```

```console
true
true
```

That differs from iteration, which excludes the upper bound. When you want
"would the loop visit this number", compare against `lower()` and `upper()`
yourself.

### `loop()`

`loop()` walks the range and calls a function for each value, honouring the
step and the direction:

```zuri
(25..18).loop(@(i) { print('${i} ') })
print('\n')
```

```console
25 24 23 22 21 20 19 
```

## What Is Iterable

`for ... in` works on:

- **lists**, giving index and value
- **dictionaries**, giving key and value, in insertion order
- **strings**, giving character position and character
- **bytes**, giving index and the numeric byte
- **ranges**, giving position and value
- **any class** that defines `@key()` and `@value()`

`is_iterable()` answers the question for any value:

```zuri
echo is_iterable([1, 2])
echo is_iterable('text')
echo is_iterable(42)
```

```console
true
true
false
```

## The Iterator Protocol

`for` is not magic. It desugars into a loop over two method calls.

Given `for value in thing`, Zuri evaluates `thing` **once**, then repeats:

1. `key = thing.@key(key)`, starting from `nil`
2. stop if `key` is `nil`
3. `value = thing.@value(key)`
4. run the body

So `@key(previous)` answers "what comes after this one?", returning `nil`
when there is nothing left, and `@value(key)` answers "what is stored
here?".

Everything built in implements this. So can your own classes, which is
what [Chapter 6](ch06-03-decorated-methods.md) shows.

Because the iterable is evaluated exactly once, this is safe:

```zuri
for line in read_the_whole_file() {
  echo line
}
```

The function runs once, not once per line.
