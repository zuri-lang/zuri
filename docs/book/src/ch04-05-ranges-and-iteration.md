# Ranges and Iteration

## Ranges

`a..b` creates a range, which represents the integers from `a` up to but not
including `b`.

`..` binds more tightly than a method call, so you can call a method on a
range directly without parentheses:

```zuri
echo 1..5.to_list()
echo 1..10.step(3).get_step()
```

```console
[1, 2, 3, 4]
3
```

You only need parentheses when the bounds are themselves expressions,
because `..` only accepts primary expressions: `(n * 2)..(n * 3)`.

> Property accesses and method calls are expressions too, so `0..self.size`
> and `0..items.length()` mean `(0..self).size` and `(0..items).length()`.
> If you want the bound to be the property instead of applying it to the
> range, write `0..(self.size)` and `0..(items.length())`.
>
> This is a consequence of the previous rule. `..` has to bind more tightly
> than `.` so that `1..10.step(3)` creates a range with a step, and once it
> does, `0..self.step(5)` can't mean the instance's own `step` method. The
> parentheses make it clear which one you mean, because the compiler has no
> way of guessing.

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

`range()` returns the distance between the two bounds.

A range is a value in its own right, not just loop syntax. You can store it,
pass it to a function and return it:

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

If you write the larger bound first, the range counts down:

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

The same rule applies in both directions: the first bound is included and
the second isn't.

### Steps

```zuri
for i in 1..10.step(3) {
  echo i
}
```

```console
1
4
7
```

`step()` returns a new range with the given step size. `get_step()` returns
the step, which is `1` by default.

`to_list()` turns a range into a list with a step of one, and ignores any
step you've set:

```zuri
echo 1..5.to_list()
```

```console
[1, 2, 3, 4]
```

### Checking Whether a Value Is in a Range

`within()` checks a value against the bounds, **including both of them**. It
works the same way regardless of direction, so `10..1.within(10)` and
`1..10.within(10)` give the same answer:

```zuri
var r = 1..10

echo r.within(5)
echo r.within(10)
```

```console
true
true
```

This is different from iteration, which leaves out the upper bound. If you
want to know whether a loop over the range would visit a particular number,
compare it against `lower()` and `upper()` yourself.

### Iterating Over a Range

With one variable, `for` gives you the values. With two, it gives you the
position first and the value second:

```zuri
for value in 3..6 {
  echo value
}

for index, value in 3..6 {
  echo '${index}: ${value}'
}
```

```console
3
4
5
0: 3
1: 4
2: 5
```

The position always counts from zero, wherever the range starts. That's what
makes it useful: `3..6` gives the values 3, 4 and 5 at positions 0, 1 and 2.

`loop()` takes a callback instead. It respects both the step and the
direction:

```zuri
25..18.loop(@(i) {
  print('${i} ')
})
print('\n')
```

```console
25 24 23 22 21 20 19 
```

An `iter` loop doesn't need a range at all. Its three clauses can express
everything a range can, and more, since the step can be any expression:

```zuri
iter var i = 3; i < 6; i++ {
  echo i
}
```

```console
3
4
5
```

Use a range with `for` when the bounds are what matter, and `iter` when the
way you step through the values matters more.

## Iterable Types

You can use `for ... in` with:

- **Lists**, which give the index and the value
- **Dictionaries**, which give the key and the value, in insertion order
- **Strings**, which give the position and the character
- **Bytes**, which give the index and the numeric value of the byte
- **Ranges**, which give the position and the value
- **Any class** that defines `@key()` and `@value()`

`is_iterable()` tells you whether a value can be iterated over:

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

There's nothing magic about `for`. Under the hood, it's a loop that makes
two method calls.

For `for value in thing`, Zuri evaluates `thing` **once**, and then repeats
these steps:

1. `key = thing.@key(key)`, starting with `key` set to `nil`
2. If `key` is `nil`, stop
3. `value = thing.@value(key)`
4. Run the body

In other words, `@key(previous)` returns the key that comes after the
previous one, or `nil` when there are no more, and `@value(key)` returns the
value stored under that key.

All of the built-in types implement this protocol, and your own classes can
too, as [Chapter 6](ch06-03-decorated-methods.md) shows.

Because the expression after `in` is evaluated only once, this is safe:

```zuri,ignore
for line in read_the_whole_file() {
  echo line
}
```

The function is called once, not once for every line.
