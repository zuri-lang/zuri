# Variables and Constants

A variable is a name for a value. In Zuri, you declare one with `var`, and
from then on the name refers to whatever value you last assigned to it.

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

That's really all there is to it. The rest of this section covers the
details: which names are allowed, what happens if you leave out the value,
where a name is visible, and what `const` adds.

## Declaring Variables

A `var` without an initializer holds `nil`, the value that means "nothing
here yet":

```zuri
var pending

echo pending
```

```console
nil
```

You can declare several names in one statement by separating them with
commas, and each one can have a value or not:

```zuri
var a = 1, b = 2, c

echo [a, b, c]
```

```console
[1, 2, nil]
```

This is useful when the names really do belong together, such as the three
components of a color or the lower and upper bounds of a range. When they
don't, it's better to avoid it, because one long line of declarations is
harder to read than three short ones.

`var` is a declaration, not an expression. It doesn't have a value, so you
can't write `if var x = f()` or `echo var y = 1`. Declare the variable
first, then use it.

## Naming Rules

A name can contain letters, digits and underscores, but it can't start with
a digit. `firstName`, `first_name` and `_first_name` are all valid, and
they're three different names.

Zuri code uses the same naming conventions everywhere, and the standard
library follows them too:

| Kind of name | Convention | Example |
| --- | --- | --- |
| Variable, function, method | `snake_case` | `read_line`, `total_count` |
| Class | `PascalCase` | `Account`, `HttpClient` |
| Constant | `SCREAMING_SNAKE_CASE` | `MAX_RETRIES` |
| Anything private | Leading underscore | `_cache`, `_retry()` |

The leading underscore is more than a convention. A name that starts with
`_` is private, and the compiler enforces it. Another file can't access
`module._helper`, and code outside a class can't access `instance._field`.
[Chapter 6](ch06-04-encapsulation.md) and [Chapter 8](ch08-00-modules.md)
explain what this means for classes and modules.

You can't use any of the 31 keywords listed in the previous section as
names. If you try, you'll get a syntax error at the declaration.

## Reassignment

Zuri is dynamically typed. A variable holds a value rather than having a
type, so you're allowed to assign a different kind of value to the same
name:

```zuri
var value = 42
value = 'now a string'
value = [1, 2, 3]

echo value
```

```console
[1, 2, 3]
```

Just because you can doesn't mean you should, though. If a name holds a
number on one line and a list twenty lines later, nobody reading the code
can predict what it holds. Use a second variable instead.

Assignment is an expression, and its value is the value that was assigned.
This means you can chain assignments:

```zuri
var x, y

x = y = 5

echo '${x} ${y}'
```

```console
5 5
```

## Compound Assignment

Each arithmetic and bitwise operator has an assignment form, which applies
the operator to the variable's current value and stores the result back in
the variable:

```zuri
var n = 5

n += 10 # 15
n **= 2 # 225
n //= 7 # 32
n <<= 1 # 64

echo n
```

```console
64
```

The full set is `+=`, `-=`, `*=`, `/=`, `//=`, `**=`, `%=`, `&=`, `|=`,
`^=`, `~=`, `<<=`, `>>=` and `>>>=`. Each one does the same thing as the
corresponding binary operator, but updates the variable in place.

Compound assignment works with anything you can assign to, not only plain
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

There are two rules to remember. First, **both are postfix only**. `++n` is
a syntax error, because the operator has to come after the name.

Second, when `n++` is used inside a larger expression, it updates `n` and
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

If you're used to C, Java or JavaScript, this is the opposite of what you'd
expect. In those languages, `j++` gives you the old value, but in Zuri it
gives you the new one. The easiest way to avoid any confusion is to use `++`
only as a statement of its own, or in the update clause of an `iter` loop,
where its value isn't used:

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

`const` declares a name that can't be reassigned:

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

Assigning to it is caught when the file is compiled, before any code runs:

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

A `const` has to be given a value when it's declared, so `const K` on its
own doesn't compile. A constant without a value would just be `nil` forever.

`const` is enforced in local scopes, meaning inside functions, methods and
blocks. At the top level of a module, `const` declares an ordinary
module-level variable, and reassignment isn't checked. Use it inside code
where the check is useful, which is where you might otherwise be tempted to
reassign the name.

### Constants Aren't Frozen

`const` prevents you from **rebinding the name**, but it doesn't affect the
value. A constant list is still a list, and lists can still be changed:

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

`items = [4]` would be an error, but `items.append(3)` isn't. If you need a
collection that can't be changed, give out a copy instead of relying on the
declaration.

## Scope

Every block is a scope. Braces open a new scope, and any name declared
inside it disappears when the block ends:

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

Inside the block, the inner `x` hides the outer one. This is called
**shadowing**, and it's allowed on purpose, so that a short block can use a
short name without worrying about what that name means outside the block.

Function bodies, loop bodies, `if` branches and bare `{ ... }` blocks all
behave this way. So does the initializer of an `iter` loop, whose variable
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

After the loop, `i` no longer exists, and reading it is an error. That's
what you want. Using a name that doesn't exist is a mistake, and Zuri
reports it instead of quietly giving you `nil`.

### One Declaration per Scope

You can shadow a name in an inner scope, but you can't declare the same name
twice in the *same* scope:

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

The exception is the top level of a script or module. There, a second
`var a` rebinds the existing global instead of raising an error. That's what
lets you retype a declaration in the REPL while you're experimenting.

### Functions Follow the Same Rule

A `def` is scoped the same way as a `var`. A function declared at the top
level of a file creates a module-level name. A function declared inside
another function is local to that function, and nothing outside can see it:

```zuri,ignore
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
Unhandled UndefinedError: undefined global 'helper'
  --> /path/to/main.zu:9
```

The first call works because `helper` is local to `outer`. The second one
fails, because outside `outer` that name was never defined.

[Chapter 5](ch05-01-defining-functions.md) covers this in more detail.
