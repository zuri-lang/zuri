# Variables and Constants

A variable is a name for a value. In Zuri you introduce one with `var`,
and from then on the name stands for whatever you last put in it.

```zuri
var greeting = 'hello'
var count = 0

echo greeting
echo count
```

```console
hello
0
```

That is the whole idea. The rest of this section is the detail: what a name
may be called, what happens when you leave the value out, where the name
can be seen, and what `const` adds.

## Declaring

A `var` with no initializer holds `nil`, the value that means "nothing
here yet":

```zuri
var pending

echo pending
```

```console
nil
```

You can declare several names in one statement by separating them with
commas, and each one may or may not have a value:

```zuri
var a = 1, b = 2, c

echo [a, b, c]
```

```console
[1, 2, nil]
```

This is worth using when the names genuinely belong together — the three
components of a colour, the lower and upper bound of a window — and worth
avoiding when they do not, because one long line of declarations is harder
to read than three short ones.

`var` is a declaration, not an expression. It has no value, which is why
you cannot write `if var x = f()` or `echo var y = 1`. Declare first, then
use the name.

## Naming

A name is made of letters, digits and underscores, and may not start with
a digit. `firstName`, `first_name` and `_first_name` are all legal, and all
three are different names.

Zuri code follows one set of conventions everywhere, and the standard
library is written in it:

| Kind of name | Convention | Example |
| --- | --- | --- |
| variable, function, method | `snake_case` | `read_line`, `total_count` |
| class | `PascalCase` | `Account`, `HttpClient` |
| constant | `SCREAMING_SNAKE_CASE` | `MAX_RETRIES` |
| private anything | leading underscore | `_cache`, `_retry()` |

The leading underscore is the one convention that is not only a
convention. A name beginning with `_` is private, and the compiler enforces
it: another file cannot reach `module._helper`, and code outside a class
cannot reach `instance._field`. [Chapter 6](ch06-04-encapsulation.md) and
[Chapter 8](ch08-00-modules.md) cover what that means in each case.

The 31 keywords listed at the end of the previous section cannot be used as
names at all. Trying produces a syntax error at the point of declaration.

## Reassignment

Zuri is dynamically typed. A variable holds a value, not a type, and
assigning a different kind of value to the same name is legal:

```zuri
var value = 42
value = 'now a string'
value = [1, 2, 3]

echo value
```

```console
[1, 2, 3]
```

Legal is not the same as advisable. A name that holds a number on one line
and a list twenty lines later is a name no reader can predict. Reach for a
second variable instead; they are free.

Assignment is an expression, and it evaluates to the value assigned. That
lets you chain:

```zuri
var x, y

x = y = 5

echo '${x} ${y}'
```

```console
5 5
```

## Compound Assignment

Every arithmetic and bitwise operator has an assignment form, which applies
the operator to the variable's current value and stores the result:

```zuri
var n = 5

n += 10     # 15
n **= 2     # 225
n //= 7     # 32
n <<= 1     # 64

echo n
```

```console
64
```

The full set is `+=`, `-=`, `*=`, `/=`, `//=`, `**=`, `%=`, `&=`, `|=`,
`^=`, `~=`, `<<=`, `>>=` and `>>>=`. Each one means exactly what the
matching binary operator means, applied in place.

Compound assignment works on anything you can assign to, not just plain
variables:

```zuri
var totals = { food: 0 }
var scores = [1, 2, 3]

totals.food += 12
scores[0] *= 10

echo totals
echo scores
```

```console
{food: 12}
[10, 2, 3]
```

## Increment and Decrement

`++` and `--` add or subtract one in place:

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

Two rules to remember. First, **both are postfix only**. `++n` is a syntax
error; the operator goes after the name.

Second, when `n++` appears inside a larger expression it updates `n` and
then evaluates to the **new** value:

```zuri
var j = 5

echo j++
echo j
```

```console
6
6
```

If you have written C, Java or JavaScript, this is the opposite of what you
expect: there, `j++` gives you the old value. In Zuri it gives you the new
one. The habit that avoids the question entirely is to let `++` be a
statement of its own, or to use it in the update clause of an `iter` loop
where the value is discarded:

```zuri
iter var i = 0; i < 3; i++ {
  echo i
}
```

```console
0
1
2
```

## `const`

`const` declares a name that cannot be reassigned:

```zuri
def area(radius) {
  const PI = 3.141592653589793

  return PI * radius * radius
}

echo area(2)
```

```console
12.566370614359172
```

Writing to it is caught when the file is compiled, before anything runs:

```zuri,ignore
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

A `const` must be given a value where it is declared. `const K` on its own
does not compile, which is the point: a constant with no value would be a
constant `nil` forever.

The `const` keyword is enforced in local scopes — inside functions,
methods, and blocks. At the top level of a module, `const` declares an
ordinary module global and the reassignment check does not apply. Use it
where it does its work, which is inside the code that would otherwise be
tempted to reassign.

### Constant Does Not Mean Frozen

`const` prevents **rebinding the name**. It says nothing about the value.
A constant list is still a list, and a list is still mutable:

```zuri
def demo() {
  const items = [1, 2]

  items.append(3)
  echo items
}

demo()
```

```console
[1, 2, 3]
```

`items = [4]` would be an error; `items.append(3)` is not. If you need a
collection nothing can change, copy it at the boundary where you hand it
out rather than relying on the declaration.

## Scope

A block is a scope. Braces open one, and every name declared inside it is
gone when the block closes:

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

The inner `x` hides the outer one for the length of the block. This is
called **shadowing**, and it is allowed deliberately: a short block can use
a short name without worrying about what that name means outside it.

Function bodies, loop bodies, `if` branches and bare `{ ... }` blocks all
work this way. So does the initialiser of an `iter` loop, whose variable
belongs to the loop:

```zuri,ignore
iter var i = 0; i < 2; i++ {
  echo i
}

echo i
```

```console
0
1
Unhandled UndefinedError: undefined global 'i'
  --> /path/to/main.zu:4
```

After the loop, `i` is not a variable at all, and reading it is an error.
That error is the good outcome: a name that does not exist is a mistake
worth hearing about, and Zuri never invents a silent `nil` to paper over
one.

### One Declaration Per Scope

Shadowing across scopes is allowed. Declaring the same name twice in *one*
scope is not:

```zuri,ignore
def f() {
  var a = 1
  var a = 2
}
```

```console
SyntaxError: 'a' is already declared in this scope
  --> /path/to/main.zu:3:7
  |
3 |   var a = 2
  |       ^
```

The exception is the top level of a script or module, where a second
`var a` rebinds the existing global instead of erroring. That is what lets
you retype a declaration in the REPL while you are experimenting.

### Nested Functions Are Not Local

One scoping rule surprises nearly everyone, so it belongs here rather than
in the functions chapter. A `def` written inside another function does not
create a local name. It declares a module-level function, which becomes
visible everywhere once the enclosing function has run:

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

If you want a helper that is genuinely local to one function, put an
anonymous function in a `var`:

```zuri,ignore
def outer() {
  var helper = @() => 'local only'

  return helper()
}

echo outer()
echo helper()
```

```console
local only
Unhandled UndefinedError: undefined global 'helper'
  --> /path/to/main.zu:8
```

The first call works because `helper` is a local variable inside `outer`.
The second fails because, outside `outer`, no such name was ever created.

[Chapter 5](ch05-01-defining-functions.md) covers anonymous functions
properly. For now: `def` inside `def` is module-level, `var f = @() => ...`
is local.
