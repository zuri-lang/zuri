# Variables and Constants

## `var`

You introduce a name with `var`:

```zuri
var greeting = 'hello'
var count = 0
```

A `var` with no initializer holds `nil`:

```zuri
var pending
echo pending
```

```console
nil
```

You can declare several names in one statement, separated by commas:

```zuri
var a = 1, b = 2, c
echo [a, b, c]
```

```console
[1, 2, nil]
```

Names are made of letters, digits and underscores, and may not start with a
digit. `firstName`, `first_name` and `_first_name` are all legal and all
different. The convention in Zuri code, and throughout the standard
library, is `snake_case` for variables and functions, `PascalCase` for
classes, and `SCREAMING_SNAKE_CASE` for constants. A **leading underscore
means private**, and that is not only a convention: [Chapter 8](ch08-00-modules.md)
shows how the module system enforces it.

## Reassignment and Types

Zuri is dynamically typed. A variable holds whatever you put in it, and
what you put in it can change:

```zuri
var value = 42
value = 'now a string'
value = [1, 2, 3]
```

That freedom is real, and the language does not stop you. Good Zuri code
still keeps one variable to one kind of thing, because the JIT compiler
specialises on the types it observes and a variable that changes shape
gives it nothing to work with. [Chapter 18](ch18-00-performance.md) goes
into that.

## `const`

`const` declares a name that cannot be reassigned:

```zuri
def area(radius) {
  const PI = 3.141592653589793
  return PI * radius * radius
}
```

Writing to it is a compile-time error:

```zuri
def f() {
  const K = 1
  K = 2
}
```

```console
SyntaxError: cannot assign to constant 'K'
  --> /path/to/main.zu:3:3
  |
3 |   K = 2
  |   ^
```

A `const` must be given a value at the point of declaration. `const K`
alone does not compile.

The `const` keyword is enforced in local scopes. At the top level of a
module, `const` declares an ordinary module global and the reassignment
check does not apply, so reach for it inside functions, methods and blocks
where it does its job.

`const` prevents **rebinding**, not mutation. A constant list is still a
list you can append to:

```zuri
def demo() {
  const items = [1, 2]
  items.append(3)
  echo items
}
```

```console
[1, 2, 3]
```

## Scope

A block is a scope. Braces open one, and every name declared inside is gone
when the block closes:

```zuri
var x = 'outer'
{
  var x = 'inner'
  echo x
}
echo x
```

```console
inner
outer
```

This is called **shadowing**: the inner `x` hides the outer one for the
length of the block. A function body, a loop body, an `if` branch and a
bare `{ ... }` all work the same way.

Shadowing across scopes is allowed. Declaring the same name twice in *one*
scope is not:

```zuri
def f() {
  var a = 1
  var a = 2
}
```

```console
SyntaxError: 'a' is already declared in this scope
  --> /path/to/main.zu:3:6
  |
3 |   var a = 2
  |       ^
```

At the top level of a script or module, a second `var a` rebinds the
existing global rather than erroring, which is what lets the REPL redefine
things as you experiment.

## Compound Assignment

Every arithmetic and bitwise operator has an assignment form:

```zuri
var n = 5
n += 10     # 15
n **= 2     # 225
n //= 7     # 32
n <<= 1     # 64
```

The full set is `+= -= *= /= //= **= %= &= |= ^= ~= <<= >>= >>>=`.

There are also `++` and `--`, which increment and decrement in place:

```zuri
var n = 5
n++
echo n
n--
echo n
```

```console
6
5
```

Both are postfix only. `++n` is a syntax error. Used inside a larger
expression, `n++` updates `n` and evaluates to the **new** value:

```zuri
var j = 5
echo j++
echo j
```

```console
6
6
```

They appear most often in the third clause of an `iter` loop, where the
value is discarded and only the update matters.
