# Control Flow

Control flow is how a program decides what to run next. You might want to
run some code only when a condition is true, keep running it until a
condition becomes false, or run it once for each item in a collection. Zuri
has seven statements for this, and this section covers all of them.

## Blocks and Single Statements

Every control flow statement has a **body**, which is either a block in
braces or a single statement:

```zuri
var ready = true

if ready {
  echo 'launching'
}

if ready {
  echo 'launching again'
}
```

```console
launching
launching again
```

Both forms are valid. `if`, `else`, `while`, `do`, `for` and the `when`
branches of a `using` statement all accept either one.

The exception is `iter`. Its body has to be a block, because otherwise the
semicolons in its header could be confused with the statement that follows:

```zuri,ignore
iter var i = 0; i < 3; i++ echo i
```

```console
SyntaxError: Expected '{' at the start of iter body.
```

> The standard library and the examples in this book use braces for every
> body except short `when` branches. With braces, it's easy to turn a
> one-line body into a three-line one later, and the structure of nested
> statements is easier to see. This is only a convention, though, so use
> whichever form reads better in your code.

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

There's no `elif`. `else if` is simply two keywords, which works because the
body of an `else` can itself be an `if` statement.

The condition can be any expression. It's checked for **truthiness**, so it
doesn't have to be a `bool`:

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

This is convenient, but it's also the most common source of bugs in new Zuri
code, because a perfectly valid `0` is falsy. Look at the table in [Data
Types](ch03-02-data-types.md) again before you write `if count` or
`if position`.

## The Conditional Expression

`? :` is the expression version of `if`. Instead of choosing between two
statements, it chooses between two values:

```zuri
var age = 20
var status = age >= 18 ? 'adult' : 'minor'

echo status
```

```console
adult
```

Conditional expressions can be nested, and they can span several lines. When
you split one across lines, `?` and `:` can go at the end of a line or at
the start of the next one. Putting each operator at the start of its branch
makes the structure easier to follow:

```zuri
var n = 7

var size = n > 100 ? 'large' : n > 5 ? 'medium' : 'small'

echo size
```

```console
medium
```

Use `? :` when you need a value, and `if` when you want to do something. A
conditional expression whose branches only exist for their side effects is
harder to read than the `if` statement it replaced.

## `while`

`while` checks its condition before each pass, so the body might not run at
all:

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

`do` runs the body once *before* it checks the condition, so the body always
runs at least once. This is useful when the condition depends on something
the body computes:

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

The condition was false from the start, but the body still ran once.

## `iter`

`iter` is the counting loop. Its header has three clauses separated by
semicolons: an initializer, a condition and an update.

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

The initializer runs once. The condition is checked before each pass, and
the update runs after each pass. A variable declared in the initializer
belongs to the loop and doesn't exist after the loop ends.

The initializer can declare several variables, and the update can contain
several expressions, separated by commas. The updates run in the order
they're written:

```zuri
iter var i = 0, j = 10; i < 3; i++, j-- {
  echo '${i} ${j}'
}
```

```console
0 10
1 9
2 8
```

All three clauses are optional:

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

`iter ; ; { ... }` loops forever, but `while true` is easier to read.

You can spread the header over several lines, which helps when the condition
is long:

```zuri
iter var i = 1; i <= 3; i++ {
  echo i
}
```

```console
1
2
3
```

## `for` / `in`

`for` loops over an iterable, such as a list, a dictionary, a string, a
range, a byte stream, or an instance of any class that implements the
iterator protocol.

With one variable, you get each **value**:

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

With two variables, you get the **key first and the value second**:

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

For a list, the key is the index. For a dictionary, it's the key, and for a
string, it's the position of the character. Dictionaries are iterated in
insertion order, as is anything else whose order is preserved.

Ranges don't include their upper bound:

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

The expression after `in` is evaluated only **once**, before the first pass.
If you call a function there, it's called once, not once per element:

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

[Chapter 6](ch06-03-decorated-methods.md) shows how to make your own classes
work with `for` by defining `@key()` and `@value()`.

## `break` and `continue`

`continue` skips to the next pass, and `break` exits the loop completely:

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

Both apply to the innermost loop, and Zuri doesn't have loop labels. To exit
a nested loop, you can use a flag:

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

Or, more often, you can move the nested loop into a function and `return`
from it:

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

The second version is better. Its name says what it's looking for, and
there's no flag to keep in sync.

## `using` / `when`

`using` compares one value against several possible values:

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

When a `when` lists several values, any one of them can match, so this
`when` matches `'B'` **or** `'C'`. The first branch that matches runs, and
then the statement ends. There's no fall-through, so you don't need `break`.

Matching uses the same equality as `==`. That means `using` works with
numbers, strings and booleans, and compares instances by identity, or with
`@eq` if their class defines it.

`default` is optional. If nothing matches and there's no `default`, nothing
happens:

```zuri
using 99 {
  when 1 echo 'one'
}

echo 'nothing ran, and that is not an error'
```

```console
nothing ran, and that is not an error
```

A `when` branch can use a block when it needs more than one statement:

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

Use `using` whenever you need to choose between many outcomes based on one
value. A chain of `else if` statements that compares the same variable again
and again does the same job with more code.

## `assert`

`assert` checks a condition and raises an `AssertError` if it's false:

```zuri
var items = [1]

assert 1 + 1 == 2
assert items.length() == 1, 'list should have one item'

echo 'both assertions held'
```

```console
both assertions held
```

The optional second argument is the message for the error:

```zuri,ignore
assert 1 == 2, 'arithmetic is broken'
```

```console
Unhandled AssertError: arithmetic is broken
```

Use assertions for things you believe are already true, meaning conditions
that should never happen. Don't use them to check input that might really be
wrong. Raise a `ValueError` for that instead. [Chapter
7](ch07-00-error-handling.md) explains the difference in more detail.

## `return`

`return` exits the current function. It returns the value you give it, or
`nil` if you don't give it one:

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

A function that reaches its closing brace without a `return` returns `nil`.
There are no implicit returns, so the last expression in a function body
isn't its return value.

`return` can only be used inside a function. At the top level of a script,
it's a syntax error, because there's nothing to return from.
