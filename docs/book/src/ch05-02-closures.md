# Anonymous Functions and Closures

## Three Spellings

An anonymous function can be written three ways, and they compile to the
same thing:

```zuri
var square = def(x) { return x * x }
var square = @(x) { return x * x }
var square = @(x) => x * x
```

- `def(...) { ... }` is the long form. Use it when the body has several
  statements and you want it to look like the declarations around it.
- `@(...) { ... }` is the shorthand. This is what most Zuri code uses for a
  callback with a real body.
- `@(...) => expr` is the arrow form. The expression is returned, so there
  is no `return` and no braces. Use it when the body is one expression.

The arrow form works with `def` too, and the parameter list may be dropped
entirely when there are none:

```zuri
var answer = @ => 42
echo answer()
```

```console
42
```

Compare the three in the place they actually show up:

```zuri
echo ['  a ', ' b'].map(@(t) => t.trim())

echo [1, 2, 3].filter(@(n) {
  if n == 2 {
    return false
  }
  return n > 0
})
```

```console
[a, b]
[1, 3]
```

Anonymous functions are variadic-capable and take type annotations exactly
like named ones.

## Closures

A function carries the variables it referenced from its surroundings, and
keeps them alive after the enclosing function has returned:

```zuri
def make_counter() {
  var n = 0

  return @() {
    n++
    return n
  }
}

var a = make_counter()
var b = make_counter()

echo a()
echo a()
echo b()
```

```console
1
2
1
```

`a` and `b` each closed over their **own** `n`. Each call to
`make_counter()` created a fresh one.

## Capture Is by Reference

A closure captures the variable, not a snapshot of its value:

```zuri
def demo() {
  var total = 0
  var add = @(n) { total += n }

  add(5)
  add(10)

  return total
}

echo demo()
```

```console
15
```

This is what makes accumulator patterns work, and it is what makes loop
variables surprising. If you want a per-iteration capture, declare a fresh
variable inside the loop body:

```zuri
var fns = []

iter var i = 0; i < 3; i++ {
  var captured = i
  fns.append(@() => captured)
}

echo fns.map(@(f) => f())
```

```console
[0, 1, 2]
```

`captured` is a new variable on each pass, so each closure gets its own.

## Closures over Parameters

Parameters are captured the same way, which is the basis of partial
application:

```zuri
def adder(amount) {
  return @(n) => n + amount
}

var add_five = adder(5)
var add_ten = adder(10)

echo add_five(1)
echo add_ten(1)
```

```console
6
11
```

## Recursion in an Anonymous Function

An anonymous function has no name to call itself by. Bind it first:

```zuri
var factorial

factorial = @(n) {
  if n <= 1 {
    return 1
  }
  return n * factorial(n - 1)
}

echo factorial(5)
```

```console
120
```

The variable is captured by reference, so by the time the body runs,
`factorial` is bound.

## Functions Compare by Identity

```zuri
def f() {}

var g = f
echo f == g
echo f == @() {}
```

```console
true
false
```

Two separately written functions are never equal, even with identical
bodies.

## A Worked Example

Closures are at their most useful when a function needs to remember
something between calls without that something becoming a global. Here is a
rate limiter: it hands back a function that answers "may I do this now?",
and keeps its own tally where nothing else can reach it.

```zuri
def make_limiter(max_per_window: number) {
  var used = 0

  return {
    allow: @() {
      if used >= max_per_window {
        return false
      }

      used++
      return true
    },

    remaining: @() => max_per_window - used,

    reset: @() {
      used = 0
    },
  }
}

var limiter = make_limiter(2)
var allow = limiter.allow
var remaining = limiter.remaining
var reset = limiter.reset

echo allow()
echo allow()
echo allow()
echo remaining()

reset()
echo allow()
```

```console
true
true
false
0
true
```

Three separate functions share one `used`, because all three closed over
the same variable in the same call to `make_limiter`. A second call to
`make_limiter` would produce a second, independent trio.

This is the closest Zuri gets to a private field without a class, and it is
worth knowing for exactly that reason: there is no way for a caller to read
or write `used` except through the three functions you gave them.
