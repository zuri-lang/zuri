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

`def`, a name, a parameter list, a block. No return type, no type
annotations required, no separate declaration and definition.

A function with no `return` gives back `nil`.

## Arity Is Not Enforced

Call a function with fewer arguments than it declares and the missing ones
are `nil`. Call it with more and the extras are dropped:

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

This is deliberate, and it is how optional parameters work: there is no
default-value syntax in a parameter list, so you write the default in the
body:

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

Because `or` is doing the work, this pattern turns a legitimately falsy
argument into the default. When `0`, `''`, `false` or a negative number are
real inputs, test for `nil` instead:

```zuri
def indent(text, width) {
  if width == nil {
    width = 2
  }
  return ' ' * width + text
}
```

If you want the runtime to reject a missing argument, annotate the
parameter. [Type Annotations](ch05-03-type-annotations.md) covers that.

## Variadic Functions

A final parameter prefixed with `...` collects everything left over into a
list:

```zuri
def count(...items) {
  return items.length()
}

echo count()
echo count(1, 2, 3)
```

```console
0
3
```

It can follow named parameters:

```zuri
def log(level, ...parts) {
  echo '[${level}] ' + ' '.join(parts)
}

log('warn', 'disk', 'almost', 'full')
```

```console
[warn] disk almost full
```

The variadic parameter is always a list, even when nothing was passed. Only
the last parameter may be variadic.

There is no spread at the **call** site. You cannot expand a list back into
separate arguments; pass the list itself.

## Functions Are Module-Scoped

A named `def` binds a **module-level** name, wherever it is written. That
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
running `outer` is what executes the declaration.

When you want a helper that stays private to one function, bind an
anonymous function to a `var`:

```zuri
def outer() {
  var helper = @() => 'private'
  return helper()
}
```

That one is a local, and it disappears when `outer` returns.

## Declaration Order Matters

A function must be declared before the line that calls it, at the top
level:

```zuri,ignore
echo later()

def later() {
  return 'nope'
}
```

```console
Unhandled UndefinedError: undefined global 'later'
```

There is no hoisting. Declarations execute in order like everything else.

Inside a function body this does not apply, because the name is resolved
when the call runs rather than when it is compiled. That is what makes
mutual recursion work:

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
```

```console
true
```

## Functions as Values

Pass one around like any other value:

```zuri
def apply_twice(f, x) {
  return f(f(x))
}

def increment(n) {
  return n + 1
}

echo apply_twice(increment, 5)
```

```console
7
```

Every function carries a small amount of metadata:

```zuri
def named(a, b, ...c) {}

echo named.name()
echo named.arity()
echo named.is_variadic()
```

```console
named
3
true
```

`arity()` counts declared parameters, the variadic one included.

`call()` invokes a function with a list of arguments, which is the closest
thing Zuri has to a spread:

```zuri
def add(a, b) {
  return a + b
}

echo add.call(2, 3)
```

```console
5
```

`is_callable()` tells you whether a value can be called at all, which
covers functions, native functions and classes.
