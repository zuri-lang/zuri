# Defining Functions

## Declaration

```zuri
def greet(name) {
  return 'Hello, ' + name
}

echo greet('Zuri')
```

```console
Hello, Zuri
```

`def`, a name, a parameter list in parentheses, a block. There is no return
type to declare, no forward declaration, and no separate signature.

A function with no `return` yields `nil`:

```zuri
def silent() {
  echo 'working'
}

echo silent()
```

```console
working
nil
```

There are no implicit returns. The last expression in a body is not its
value; if you want something back, say `return`.

## Arity Is Not Enforced

Call a function with fewer arguments than it declares and the missing ones
arrive as `nil`. Call it with more and the extras are dropped:

```zuri
def greet(name) {
  return 'Hello, ' + name
}

echo greet()
echo greet('Zuri', 'ignored')
```

```console
Hello, nil
Hello, Zuri
```

This is not an oversight. It is the mechanism optional parameters are built
on, since there is no default-value syntax in a parameter list. You write
the default in the body instead:

```zuri
def greet(name, greeting) {
  greeting = greeting or 'Hello'

  return '${greeting}, ${name}!'
}

echo greet('Ada')
echo greet('Ada', 'Hi')
```

```console
Hello, Ada!
Hi, Ada!
```

### The `or` Trap in Defaults

Because `or` does the work, a legitimately falsy argument is replaced by
the default. In Zuri that set includes `0`, `''`, `false`, **and every
negative number**:

```zuri
def indent(text, width) {
  width = width or 2

  return ' ' * width + text
}

echo '[' + indent('a') + ']'
echo '[' + indent('a', 0) + ']'
```

```console
[  a]
[  a]
```

The caller asked for zero indentation and got two. When a falsy value is a
legitimate input, test for `nil` explicitly:

```zuri
def indent(text, width) {
  if width == nil {
    width = 2
  }

  return ' ' * width + text
}

echo '[' + indent('a') + ']'
echo '[' + indent('a', 0) + ']'
```

```console
[  a]
[a]
```

Use `or` for a default when every falsy value is genuinely absent — a name,
a path, a list. Use `== nil` for numbers and booleans.

If you want the runtime to **reject** a missing argument rather than
default it, annotate the parameter.
[Type Annotations](ch05-03-type-annotations.md) covers that.

## Variadic Functions

A final parameter prefixed with `...` collects every remaining argument
into a list:

```zuri
def count(...items) {
  return items
}

echo count()
echo count(1)
echo count(1, 2, 3)
```

```console
[]
[1]
[1, 2, 3]
```

Three facts about what it captures are worth being precise about.

**It is always a list**, including when nothing was passed. `count()` gives
`[]`, not `nil`, so `items.length()` is safe without a check.

**It captures arguments, not contents.** Passing a list passes *one*
argument, which arrives as a list nested inside the variadic list:

```zuri
def count(...items) {
  return items
}

echo count([1, 2])
echo count([1, 2], 3)
```

```console
[[1, 2]]
[[1, 2], 3]
```

There is no spread operator at the call site. `count(...list)` does not
parse, and there is no way to expand a list back into separate arguments.
When a function should accept a collection, take the list as an ordinary
parameter.

**Named parameters bind first.** A variadic can follow named ones, and it
takes whatever is left over:

```zuri
def named(a, ...rest) {
  return [a, rest]
}

echo named()
echo named(1)
echo named(1, 2, 3)
```

```console
[nil, []]
[1, []]
[1, [2, 3]]
```

The variadic must be **last**. `def mid(a, ...rest, b)` is a syntax error,
because there is no rule that could decide how many arguments `rest`
should keep.

A practical use is a function that formats an arbitrary number of pieces:

```zuri
def log_line(level, ...parts) {
  return '[${level}] ' + ' '.join(parts)
}

echo log_line('warn', 'disk', 'almost', 'full')
echo log_line('info')
```

```console
[warn] disk almost full
[info] 
```

## Functions Are Module-Scoped

A named `def` binds a **module-level** name wherever it is written. That
includes a `def` nested inside another function:

```zuri
def outer() {
  def helper() {
    return 'from helper'
  }

  return helper()
}

echo outer()
echo helper()
```

```console
from helper
from helper
```

`helper` becomes visible at module level the moment `outer()` runs, because
running `outer` is what executes the declaration. Before that call, the
name does not exist at all.

When you want a helper that stays private to one function, bind an
anonymous function to a `var`:

```zuri
def outer() {
  var helper = @() => 'private'

  return helper()
}

echo outer()
```

```console
private
```

That one is an ordinary local, and it disappears when `outer` returns.

## Redeclaring a Name

Declaring the same function name twice in one module is **not** an error.
The second declaration replaces the first, and the last one to execute
wins:

```zuri
def pick() {
  return 'first'
}

def pick() {
  return 'second'
}

echo pick()
```

```console
second
```

This follows from `def` being a statement that runs: the second one
executes after the first and rebinds the name, exactly as a second `var` at
the top level does.

It is worth knowing because nothing warns you. Two functions of the same
name in a long file, or a name that collides with one a wildcard import
brought in, will silently resolve to whichever ran last. Keep names
distinct.

Classes are stricter. Declaring the same **method** twice in one class is a
compile error:

```zuri,ignore
class Duplicate {
  m() {}
  m() {}
}
```

```console
SyntaxError: multiple declaration for method 'm' found in class 'Duplicate'
```

## Declaration Order

A function must be declared before the **top-level** line that calls it:

```zuri,ignore
echo later()

def later() {
  return 'nope'
}
```

```console
Unhandled UndefinedError: undefined global 'later'
```

There is no hoisting. Declarations execute in order, like everything else.

Inside a function body the rule does not apply, because a name is resolved
when the call runs rather than when it is compiled. That is what makes
mutual recursion work with no forward declaration:

```zuri
def is_even(n) {
  if n == 0 {
    return true
  }

  return is_odd(n - 1)
}

def is_odd(n) {
  if n == 0 {
    return false
  }

  return is_even(n - 1)
}

echo is_even(10)
echo is_odd(7)
```

```console
true
true
```

`is_even` refers to `is_odd` before it exists. That is fine, because the
reference is resolved when `is_even(10)` runs, by which time both
declarations have executed.

## Functions as Values

A function is a value, so it can be passed, stored and returned:

```zuri
def apply_twice(f, x) {
  return f(f(x))
}

def increment(n) {
  return n + 1
}

echo apply_twice(increment, 5)
echo apply_twice(@(s) => s + '!', 'hi')
```

```console
7
hi!!
```

### What a Function Knows About Itself

Every callable carries four methods:

```zuri
def named(a, b, ...c) {
  return [a, b, c]
}

echo named.name()
echo named.arity()
echo named.is_variadic()
echo named.to_string()
```

```console
named
3
true
<function named(3)>
```

`arity()` counts **declared parameters**, the variadic one included, so a
variadic function's arity is a minimum rather than a requirement.

An anonymous function is named after the order the compiler met it:

```zuri
var f = @(x) => x

echo f.name()
```

```console
@anon0
```

A method read off an instance keeps its receiver, and counts it:

```zuri
class Greeter {

  hello() {
    return 'hi'
  }
}

var bound = Greeter().hello

echo bound()
echo bound.name()
echo bound.arity()
```

```console
hi
hello
1
```

`hello()` declares no parameters, and `arity()` reports `1`, because the
instance is the first one.

### `call()`

`call()` invokes a function with the arguments you give it:

```zuri
def add(a, b) {
  return a + b
}

echo add.call(2, 3)
```

```console
5
```

`call()` takes the arguments **individually**, exactly as a normal call
does. It is not a spread: `add.call([2, 3])` passes one argument, a list.
Its use is calling something whose identity you only have as a value —
a handler out of a dictionary, a method from `zuri.reflect` — in a place
where the ordinary call syntax reads badly.

### Testing Callability

```zuri
def f() {}

echo is_callable(f)
echo is_callable(print)
echo is_callable(Error)
echo is_callable(42)

echo is_function(f)
echo is_function(Error)
```

```console
true
true
true
false
true
false
```

`is_callable()` is true for anything you can put parentheses after,
**classes included** — calling a class constructs an instance.
`is_function()` is narrower: true for a `def`, an anonymous function and a
built-in, false for a class.

## A Worked Example

A small pipeline builder, using most of this section: functions as values,
a variadic parameter, a default, and a returned closure.

```zuri
def pipeline(...stages) {
  return @(input) {
    var value = input

    for stage in stages {
      value = stage(value)
    }

    return value
  }
}

def strip(text) {
  return text.trim()
}

def collapse_spaces(text) {
  return text.replace('/\s+/', ' ')
}

def truncate(text, limit) {
  limit = limit or 20

  if text.length() <= limit {
    return text
  }

  return text[0, limit - 1] + '…'
}

var tidy = pipeline(strip, collapse_spaces, @(t) => truncate(t, 12))

echo '[' + tidy('   too    many   spaces here   ') + ']'
echo '[' + tidy('  short  ') + ']'
echo '[' + pipeline()('untouched') + ']'
```

```console
[too many sp…]
[short]
[untouched]
```

Four things to take from it.

`pipeline()` takes its stages variadically and returns a closure over them,
so the returned function is a new function specialised to those stages.

`pipeline()` with no arguments still works, because a variadic parameter is
an empty list rather than `nil`, and a `for` over an empty list runs zero
times.

The third stage is wrapped in `@(t) => truncate(t, 12)` because
`truncate` takes two parameters and a stage takes one. That wrapper is the
thing a spread operator would otherwise be for.

And `limit = limit or 20` is safe here precisely because `0` is not a
sensible limit. Had it been, this would need the `== nil` form.
