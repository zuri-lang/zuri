# Control Flow

Every control-flow body in Zuri is a **block**:

```zuri
if ready {
  launch()
}
```

The parser will also accept a single unbraced statement after `if`,
`while`, `for`, `iter` and `do`. Zuri code does not use that form. Braces
everywhere is the house style, it is what the standard library is written
in, and it is what this book uses throughout. The one place a body stays
unbraced is a short `when` branch inside a `using` statement, which keeps
dispatch tables readable.

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

The condition is any expression, and it is judged by truthiness, not by
being a `bool`. Re-read the falsy table in [Data Types](ch03-02-data-types.md)
before you write `if count`.

## `while`

```zuri
var n = 0
while n < 3 {
  echo 'while ' + n
  n++
}
```

```console
while 0
while 1
while 2
```

## `do` / `while`

Runs the body once before testing, which is what you want when the test
depends on something the body produces:

```zuri
var m = 10
do {
  echo 'do ' + m
  m++
} while m < 3
```

```console
do 10
```

The condition was false from the start, and the body still ran once.

## `iter`

`iter` is the counting loop. Three clauses, separated by semicolons:
initialiser, condition, update.

```zuri
iter var i = 0; i < 3; i++ {
  echo 'iter ' + i
}
```

```console
iter 0
iter 1
iter 2
```

Every clause is optional:

```zuri
var k = 0
iter ; k < 2; {
  echo 'bare ' + k
  k++
}
```

```console
bare 0
bare 1
```

`iter ; ; { ... }` is an infinite loop, though `while true` says it better.

The clauses may be spread over several lines, which helps when the
condition is long:

```zuri
iter var i = 1;
  i <= 10;
  i++
{
  echo i
}
```

The variable declared in the initialiser is scoped to the loop.

## `for` / `in`

`for` walks anything iterable: lists, dictionaries, strings, ranges, bytes,
and any class that defines the iterator protocol.

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

With two, you get the **key and then the value**:

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

For a list the key is the index; for a dictionary it is the key; for a
string it is the character position. Dictionaries iterate in insertion
order.

Ranges work the way you expect, exclusive at the top:

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

The iterable expression is evaluated exactly once, before the first
iteration, so calling a function in that position does not call it again
per element.

Chapter 6 shows how to make your own class work with `for` by defining
`@key()` and `@value()`.

## `break` and `continue`

`continue` skips to the next iteration; `break` leaves the loop:

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

Both apply to the innermost enclosing loop. There are no loop labels, so
breaking out of nested loops means a flag or a function:

```zuri
var found = false

iter var i = 0; i < 3; i++ {
  iter var j = 0; j < 3; j++ {
    if i * j == 4 {
      found = true
      break
    }
  }

  if found {
    break
  }
}
```

The usual alternative is to put the nested loop in a function and `return`
out of it, which reads better in most cases.

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

Several values on one `when` are alternatives, not a sequence. The first
`when` that matches runs, and then the statement is over. There is no
fall-through and no `break` to remember.

`default` is optional. If nothing matches and there is no `default`,
nothing happens:

```zuri
using 99 {
  when 1 echo 'one'
}
echo 'no default, nothing ran'
```

```console
no default, nothing ran
```

Matching uses the same equality as `==`, so `using` works on numbers,
strings and booleans.

A `when` body may stay on one line when it is a single statement and the
line stays under about sixty characters. Anything longer takes a block:

```zuri
using command {
  when 'a' add(bookmarks)
  when 'q' {
    save(bookmarks)
    echo 'Bye.'
    return
  }
}
```

## `assert`

`assert` checks an invariant and raises an `AssertError` when it fails:

```zuri
assert 1 + 1 == 2
assert items.length() == 1, 'list should have one item'
```

The second argument is the message. Assertions are for conditions that
should be impossible, not for validating input; use a raised `ValueError`
for that. [Chapter 7](ch07-00-error-handling.md) draws the line.

## `return`

`return` leaves the current function, optionally with a value. A function
with no `return` yields `nil`.

`return` is only legal inside a function. At the top level of a script it
is a syntax error, because there is nothing to return from.
