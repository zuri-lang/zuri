# Anonymous Functions and Closures

## Syntax

There are two parts of an anonymous function that you can vary
independently: how it starts and how its body is written. That gives you a
small grid of options rather than a list to memorize.

**The start.** `def` is the keyword, and `@` is its shorthand. They mean the
same thing.

**The body.** A block in braces returns a value with `return`. An arrow
returns the single expression that follows it.

```zuri
var block_long = def(x) {
  return x * x
}
var block_short = @(x) {
  return x * x
}

var arrow_long = def(x) => x * x
var arrow_short = @(x) => x * x

echo [block_long(3), block_short(3), arrow_long(3), arrow_short(3)]
```

```console
[9, 9, 9, 9]
```

When there are **no parameters**, you can also leave out the empty
parentheses:

```zuri
var a = @() {
  return 1
}
var b = @() => 2
var c = def() {
  return 3
}
var d = def() => 4
var e = @ => 5
var f = @{
  return 6
}
var g = def {
  return 7
}
var h = def => 8

echo [a(), b(), c(), d(), e(), f(), g(), h()]
```

```console
[1, 2, 3, 4, 5, 6, 7, 8]
```

All eight forms create the same thing. Here's when to use each:

- **`@(x) => expr`** for a callback that's a single expression. This is what
  most Zuri code uses, including the standard library.
- **`@(x) { ... }`** when the body needs more than one statement.
- **`def(x) { ... }`** when the function is long enough that you want it to
  look like the declarations around it.
- **`@ => expr`** for a thunk, a value that's computed when it's needed and
  takes no arguments.

Here are the two most common forms as you'd typically see them:

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

The arrow form doesn't use `return`, because the expression *is* the return
value. Mixing the two up is a mistake the compiler won't always catch.
`@(x) => return x` doesn't parse, but `@(x) { x }` parses fine and returns
`nil`.

Anonymous functions accept variadic parameters and type annotations in the
same way as named functions:

```zuri
var sum = @(...numbers) => numbers.reduce(@(a, b) => a + b, 0)
var doubled = @(n: number) => n * 2

echo sum(1, 2, 3)
echo doubled(4)

catch {
  doubled('four')
} as e {
  echo e.message.replace('/@anon\d+/', '@anonN')
}
```

```console
6
8
@anonN() expects parameter 'n' (argument 1) to be a number, got string
```

The real error message contains a number instead of the `N` shown here.
Anonymous functions are named `@anonN`, numbered in the order the compiler
finds them in the file. This helps when one shows up in a stack trace, but
don't rely on the number, because adding another anonymous function earlier
in the file changes the numbers of all the ones after it.

## Closures

A function keeps the variables it uses from its surroundings, and keeps them
alive even after the enclosing function has returned:

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

`a` and `b` each have their **own** `n`, because each call to
`make_counter()` created a new one.

## Closures Capture Variables, Not Values

A closure captures the variable itself, not a copy of its value at that
moment:

```zuri
def demo() {
  var total = 0
  var add = @(n) {
    total += n
  }

  add(5)
  add(10)

  return total
}

echo demo()
```

```console
15
```

That's what makes accumulators work, and it's also why closures over loop
variables can be surprising. If you want each iteration to capture its own
value, declare a new variable inside the loop body:

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

`captured` is a new variable on each pass through the loop, so each closure
gets its own.

## Closures Over Parameters

Parameters are captured in the same way, which is the basis of partial
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

## Recursive Anonymous Functions

An anonymous function doesn't have a name it can use to call itself. Assign
it to a variable first:

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
`factorial` has been assigned.

## Comparing Functions

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

Functions are compared by identity, so two functions written separately are
never equal, even if their bodies are identical.

## Example: A Rate Limiter

Closures are most useful when a function needs to remember something between
calls without storing it in a global variable. Here's a rate limiter. It
returns a function that answers the question "am I allowed to do this now?",
and it keeps its own count where nothing else can get at it.

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

The three functions share a single `used` variable, because they all closed
over the same variable in the same call to `make_limiter`. Calling
`make_limiter` again would create a second, independent set of three.

This is the closest you can get to a private field in Zuri without writing a
class. A caller has no way to read or change `used` except through the three
functions you gave them.
