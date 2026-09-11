# Control Flow

Control flow is how a program decides what to run next: run this only when
that is true, run this until that stops being true, run this once for every
item in a collection. Zuri has seven statements for it, and this section
covers all of them.

## Blocks and Single Statements

Every control-flow statement takes a **body**. A body is either a block in
braces, or a single statement:

```zuri
var ready = true

if ready {
  echo 'launching'
}

if ready echo 'launching again'
```

```console
launching
launching again
```

Both forms are the language. `if`, `else`, `while`, `do`, `for`, and a
`when` branch inside `using` all accept either one.

`iter` is the exception. Its body must be a block, because the semicolons
in its header would otherwise be ambiguous with the statement that follows:

```zuri,ignore
iter var i = 0; i < 3; i++ echo i
```

```console
SyntaxError: Expected '{' at the start of iter body.
```

> The standard library and the examples in this book brace every body
> except a short `when` branch. Braces make a one-line body easy to extend
> into a three-line one later, and they keep the shape of a nested
> statement obvious at a glance. It is a convention, not a rule — write
> whichever form reads better in the code you are writing.

## `if` / `else`

```zuri
var score = 73

if score >= 90 {
  echo 'A'
} else if score >= 70 {
  echo 'B'
} else {
  echo 'C'
}
```

```console
B
```

There is no `elif`; `else if` is two keywords, and it works because the
body of an `else` can itself be an `if` statement.

The condition is any expression at all, and it is judged by **truthiness**
rather than by being a `bool`:

```zuri
var name = ''

if name {
  echo 'have a name'
} else {
  echo 'no name'
}
```

```console
no name
```

That is convenient and it is also where most bugs in new Zuri code come
from, because the falsy set includes every negative number. Re-read the
table in [Data Types](ch03-02-data-types.md) before you write `if count`
or `if position`.

## The Conditional Expression

`? :` is the expression form of `if`. It chooses between two values rather
than between two statements:

```zuri
var age = 20
var status = age >= 18 ? 'adult' : 'minor'

echo status
```

```console
adult
```

It nests, and it can span several lines. The rule when breaking it across
lines is that `?` and `:` **end** a line rather than beginning one:

```zuri
var n = 7

var size = n > 100 ?
  'large' :
  n > 5 ?
    'medium' :
    'small'

echo size
```

```console
medium
```

Starting a line with `?` or `:` does not parse.

Use `? :` when you are producing a value and `if` when you are performing
an action. A conditional expression whose branches are both side effects is
harder to read than the `if` it replaced.

## `while`

`while` tests before each pass, so a body may run zero times:

```zuri
var n = 0

while n < 3 {
  echo 'while ${n}'
  n++
}
```

```console
while 0
while 1
while 2
```

## `do` / `while`

`do` runs the body once and *then* tests, so the body always runs at least
once. Reach for it when the test depends on something the body produces:

```zuri
var m = 10

do {
  echo 'do ${m}'
  m++
} while m < 3
```

```console
do 10
```

The condition was false from the very start, and the body still ran.

## `iter`

`iter` is the counting loop. Its header has three clauses separated by
semicolons: an initialiser, a condition, and an update.

```zuri
iter var i = 0; i < 3; i++ {
  echo 'iter ${i}'
}
```

```console
iter 0
iter 1
iter 2
```

The initialiser runs once. The condition is tested before every pass. The
update runs after every pass. A variable declared in the initialiser
belongs to the loop and does not exist after it.

Every clause is optional:

```zuri
var k = 0

iter ; k < 2; {
  echo 'bare ${k}'
  k++
}
```

```console
bare 0
bare 1
```

`iter ; ; { ... }` loops forever, though `while true` says it more plainly.

The header may be spread over several lines, which is worth doing when the
condition is long:

```zuri
iter var i = 1;
  i <= 3;
  i++
{
  echo i
}
```

```console
1
2
3
```

## `for` / `in`

`for` walks an iterable: a list, a dictionary, a string, a range, a byte
stream, or any class that implements the iterator protocol.

With one variable you get the **value**:

```zuri
for ch in 'hey' {
  echo ch
}
```

```console
h
e
y
```

With two, you get the **key first and the value second**:

```zuri
for index, value in ['a', 'b'] {
  echo '${index} ${value}'
}

for key, value in { x: 1, y: 2 } {
  echo '${key}=${value}'
}
```

```console
0 a
1 b
x=1
y=2
```

For a list the key is the index, for a dictionary it is the key, and for a
string it is the character position. Dictionaries iterate in insertion
order, and so does everything else with an order to preserve.

Ranges are exclusive at the top:

```zuri
for i in 0..3 {
  echo i
}
```

```console
0
1
2
```

The iterable expression is evaluated exactly **once**, before the first
pass. Calling a function in that position calls it once, not once per
element:

```zuri
def source() {
  echo 'source() called'
  return [1, 2, 3]
}

for n in source() {
  echo n
}
```

```console
source() called
1
2
3
```

[Chapter 6](ch06-03-decorated-methods.md) shows how to make your own class
work with `for` by defining `@key()` and `@value()`.

## `break` and `continue`

`continue` skips to the next pass, and `break` leaves the loop entirely:

```zuri
iter var i = 0; i < 5; i++ {
  if i == 1 {
    continue
  }

  if i == 3 {
    break
  }

  echo i
}
```

```console
0
2
```

Both apply to the innermost enclosing loop, and there are no loop labels.
To leave a nested loop you either carry a flag:

```zuri
var found = false

iter var i = 1; i < 4; i++ {
  iter var j = 1; j < 4; j++ {
    if i * j == 4 {
      echo 'found ${i} x ${j}'
      found = true
      break
    }
  }

  if found {
    break
  }
}
```

```console
found 2 x 2
```

or, more often, put the nested loop in a function and `return` out of it:

```zuri
def first_product(target) {
  iter var i = 1; i < 4; i++ {
    iter var j = 1; j < 4; j++ {
      if i * j == target {
        return [i, j]
      }
    }
  }

  return nil
}

echo first_product(4)
echo first_product(99)
```

```console
[2, 2]
nil
```

The second version says what it is looking for in its name, and it has no
flag to keep in sync. Prefer it.

## `using` / `when`

`using` compares one subject against several candidate values:

```zuri
var grade = 'B'

using grade {
  when 'A' echo 'excellent'
  when 'B', 'C' echo 'good'
  default echo 'try again'
}
```

```console
good
```

Several values on one `when` are alternatives, not a sequence: this `when`
matches `'B'` **or** `'C'`. The first branch that matches runs, and then the
statement is over — there is no fall-through, and no `break` to remember.

Matching uses the same equality as `==`, which means `using` works on
numbers, strings and booleans, and compares instances by identity.

`default` is optional. When nothing matches and there is no `default`,
nothing happens:

```zuri
using 99 {
  when 1 echo 'one'
}

echo 'nothing ran, and that is not an error'
```

```console
nothing ran, and that is not an error
```

A `when` branch takes a block when it needs one:

```zuri
var command = 'q'

using command {
  when 'a' echo 'adding'
  when 'q' {
    echo 'saving'
    echo 'goodbye'
  }
  default echo 'unknown command'
}
```

```console
saving
goodbye
```

`using` is the right shape whenever you are dispatching on one value to
many outcomes. A chain of `else if` comparing the same variable over and
over is the same thing written longer.

## `assert`

`assert` checks a condition and raises an `AssertError` when it is false:

```zuri
var items = [1]

assert 1 + 1 == 2
assert items.length() == 1, 'list should have one item'

echo 'both assertions held'
```

```console
both assertions held
```

The optional second argument is the message the error carries:

```zuri,ignore
assert 1 == 2, 'arithmetic is broken'
```

```console
Unhandled AssertError: arithmetic is broken
```

Assertions state what you believe is already true. They are for conditions
that should be impossible, not for checking input that might legitimately
be wrong — raise a `ValueError` for that.
[Chapter 7](ch07-00-error-handling.md) draws the line in detail.

## `return`

`return` leaves the current function, with a value if you give it one and
`nil` if you do not:

```zuri
def classify(n) {
  if n < 0 {
    return 'negative'
  }

  if n == 0 {
    return 'zero'
  }

  return 'positive'
}

echo classify(-5)
echo classify(0)
echo classify(3)
```

```console
negative
zero
positive
```

A function that reaches its closing brace without a `return` yields `nil`.
There are no implicit returns; the last expression in a body is not its
value.

`return` is only legal inside a function. At the top level of a script it
is a syntax error, because there is nothing to return from.
