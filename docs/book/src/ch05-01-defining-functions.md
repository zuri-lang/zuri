# Defining Functions

## Declaring a Function

```zuri
def greet(name) {
  return 'Hello, ' + name
}

echo greet('Zuri')
```

```console
Hello, Zuri
```

A function declaration is `def`, followed by a name, a parameter list in
parentheses and a block. You don't declare a return type, you don't need a
forward declaration, and there's no separate signature.

A function without a `return` statement returns `nil`:

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

There are no implicit returns. The last expression in a function body isn't
its return value, so if you want to return something, use `return`.

## Argument Counts Aren't Checked

If you call a function with fewer arguments than it declares, the missing
parameters are `nil`. If you call it with more, the extra arguments are
ignored:

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

This is deliberate. It's how optional parameters work, since there's no
syntax for default values in a parameter list. Instead, you set the default
in the function body:

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

### Watch Out for `or` in Defaults

Because the default comes from `or`, any argument that's falsy gets replaced
by the default, even when it was passed on purpose. In Zuri, the falsy
values include `0`, `NaN`, `''` and `false`:

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

The caller asked for no indentation and got two spaces. When a falsy value
is a valid input, check for `nil` explicitly instead:

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

Use `or` for a default when a falsy value can only mean the argument is
missing, as with a name, a path or a list. For numbers and booleans, use
`== nil`.

If you want the runtime to **reject** a missing argument instead of using a
default, add a type annotation to the parameter. [Type
Annotations](ch05-03-type-annotations.md) explains how.

## Variadic Functions

If the last parameter starts with `...`, it collects all of the remaining
arguments into a list:

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

There are three things to be clear about when it comes to what a variadic
parameter receives.

**It's always a list**, even when no arguments were passed. `count()` gives
you `[]`, not `nil`, so you can safely call `items.length()` without
checking first.

**It collects arguments, not their contents.** If you pass a list, that's
*one* argument, and it ends up as a list nested inside the variadic list:

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

`...` marks a parameter, not an argument, so `count(...list)` doesn't parse.
To call a function with a list of arguments, use
[`apply()`](appendix-05-10-function.md#apply):

```zuri
def count(...args) {
  return args.length()
}

echo count.apply([1, 2, 3])
```

```console
3
```

If a function is meant to accept a collection, though, it's simpler to take
the list as an ordinary parameter and avoid both.

**Named parameters are filled first.** A variadic parameter can come after
named ones, and it receives whatever arguments are left over:

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

The variadic parameter has to be **last**. `def mid(a, ...rest, b)` is a
syntax error, because there'd be no way to decide how many arguments `rest`
should take.

A typical use is a function that formats any number of values:

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

## Where a Function Is Visible

A `def` is scoped the same way as a `var`. At the top level of a file, it
creates a module-level name. Anywhere else, it creates a local in the
enclosing block, which goes away when the block ends:

```zuri
def outer() {
  def helper() {
    return 'from helper'
  }

  return helper()
}

echo outer()

catch {
  helper()
} as e {
  echo e.message
}
```

```console
from helper
undefined global 'helper'
```

A helper defined inside a function belongs to that function. Code outside
the function can't call it, and it can't interfere with anything outside the
function either.

This works in any block, not only in function bodies:

```zuri
def choose(verbose) {
  if verbose {
    def describe(n) {
      return 'the number ${n}'
    }

    return describe(7)
  }

  return '7'
}

echo choose(true)
echo choose(false)
```

```console
the number 7
7
```

### Helpers That Call Each Other

Helpers declared in the same block can call each other, in either order:

```zuri
def parity(n) {
  def even(k) {
    if k == 0 {
      return true
    }

    return odd(k - 1)
  }

  def odd(k) {
    if k == 0 {
      return false
    }

    return even(k - 1)
  }

  return even(n)
}

echo parity(10)
echo parity(7)
```

```console
true
false
```

`even` refers to `odd` before `odd` is declared. This works because the
compiler reserves a slot for every `def` in a block before it compiles any
of them. The declaration isn't moved, though. Until the `def` line itself
runs, the slot holds `nil`, so calling a helper from a line *above* its
declaration raises an error.

A helper can also call itself:

```zuri
def factorial_of(n) {
  def factorial(k) {
    if k <= 1 {
      return 1
    }

    return k * factorial(k - 1)
  }

  return factorial(n)
}

echo factorial_of(6)
```

```console
720
```

### Helpers Are Closures

A nested `def` is a closure over the local variables around it, and it can
outlive the call that created it:

```zuri
def make_counter(start) {
  var count = start

  def bump() {
    count++
    return count
  }

  return bump
}

var bump = make_counter(10)

echo bump()
echo bump()
```

```console
11
12
```

An anonymous function assigned to a `var` does the same job. It's usually
the better choice when the helper is a single line, or when you're passing
it straight to something else:

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

Use whichever reads better. A `def` gives the function a proper name in
stack traces and can call itself without an extra `var`, while an anonymous
function is shorter.

## One Declaration per Name

Declaring two functions with the same name in the same scope is a compile
error:

```zuri,ignore
def pick() {
  return 'first'
}

def pick() {
  return 'second'
}
```

```console
SyntaxError: multiple declaration for function 'pick' found
  --> /path/to/main.zu:5:5
  |
5 | def pick() {
  |     ^
```

A different parameter list doesn't make it a different function. **Zuri
doesn't support overloading**, so each name refers to exactly one function.

```zuri,ignore
def render(value) {}
def render(value, width) {}
```

```console
SyntaxError: multiple declaration for function 'render' found
```

If you want one function to handle different kinds of input, make the extra
parameters optional and check them in the body, as the `greeting` parameter
above does.

The check applies per **scope**, just like it does for `var`. A function
declared inside another function is a separate declaration in a separate
scope, so the compiler allows it, and it shadows the outer function for as
long as its scope lasts:

```zuri
def render() {
  return 'top level'
}

def wrapper() {
  def render() {
    return 'inner'
  }

  return render()
}

echo wrapper()
echo render()
```

```console
inner
top level
```

`wrapper`'s own `render` is local to `wrapper`. It handles every call made
inside that function and disappears when the call ends, leaving the
module-level `render` unchanged.

Classes follow the same rule for their methods:

```zuri,ignore
class Duplicate {
  m() {}
  m() {}
}
```

```console
SyntaxError: multiple declaration for method 'm' found in class 'Duplicate'
```

The only exception is the REPL. Typing a `def` again there replaces the
previous version, because fixing what you just typed is exactly what the
REPL is for.

## Declaration Order

A function has to be declared before any **top-level** line that calls it:

```zuri,ignore
echo later()

def later() {
  return 'nope'
}
```

```console
Unhandled UndefinedError: undefined global 'later'
```

Declarations aren't hoisted. They run in order, like everything else.

Inside a function body, this rule doesn't apply, because names are resolved
when the call runs, not when the code is compiled. That's why mutual
recursion works without a forward declaration:

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

`is_even` refers to `is_odd` before `is_odd` exists. That's fine, because
the name is only looked up when `is_even(10)` runs, and by then both
declarations have run.

Helpers nested inside a function get the same freedom in a different way,
because their slots are all reserved before any of their bodies are
compiled. Either way, what matters is that every declaration has run before
the first call is made.

## Functions as Values

A function is a value, so you can pass it around, store it and return it:

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

### Function Metadata

Every callable value has these four methods:

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

`arity()` counts the **declared parameters**, including the variadic one, so
for a variadic function, the arity is a minimum rather than an exact
requirement.

Anonymous functions are named according to the order in which the compiler
found them:

```zuri
var f = @(x) => x

echo f.name()
```

```console
@anon0
```

A method read from an instance keeps its instance, and counts it as a
parameter:

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

`hello()` doesn't declare any parameters, but `arity()` returns `1`, because
the instance counts as the first parameter.

### `call()`

`call()` calls a function with the arguments you pass to it:

```zuri
def add(a, b) {
  return a + b
}

echo add.call(2, 3)
```

```console
5
```

`call()` takes the arguments **one by one**, just like a normal call. It
doesn't spread a list, so `add.call([2, 3])` passes a single argument, which
is a list. It's useful when you only have the function as a value, such as a
handler from a dictionary or a method from `zuri.reflect`, and the normal
call syntax would be awkward to read.

### Checking Whether Something Can Be Called

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

`is_callable()` returns true for anything you can call, **including
classes**, since calling a class creates an instance. `is_function()` is
narrower. It returns true for a `def`, an anonymous function or a built-in
function, and false for a class.

## Example: A Text Pipeline

Here's a small pipeline builder that uses most of this section: functions as
values, a variadic parameter, a default value and a returned closure.

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

There are four things to notice.

`pipeline()` takes its stages as a variadic parameter and returns a closure
over them, so each call returns a new function built for those particular
stages.

Calling `pipeline()` without any arguments still works, because the variadic
parameter is an empty list rather than `nil`, and a `for` loop over an empty
list simply doesn't run.

The third stage is wrapped in `@(t) => truncate(t, 12)`, because `truncate`
takes two parameters while each stage takes only one. In languages with a
spread operator, you might use that instead.

Finally, `limit = limit or 20` is safe here only because `0` isn't a
sensible limit. If it were, you'd need the `== nil` form instead.
