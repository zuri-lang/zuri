# Debugging

Something is not doing what you expected. This chapter is about closing
that gap: reading what the runtime tells you, recognising the handful of
messages that account for most confusion, and the techniques that turn a
vague "it's broken" into a line number.

## Reading an Error

An uncaught error prints three things: what went wrong, where, and how you
got there.

```console
Unhandled ValueError: bottomed out
  --> /path/to/main.zu:3

  1 | def recurse(n) {
  2 |   if n <= 0 {
> 3 |     raise ValueError('bottomed out')
  4 |   }
  5 |   recurse(n - 1)

Stack trace (most recent call last):
  at recurse() /path/to/main.zu:3
  at recurse() /path/to/main.zu:5
  ... 19 more frames ...
  at recurse() /path/to/main.zu:5
```

The type and message come first. Then the source around the failure, with
the offending line marked. Then the call stack, innermost first.

A deep stack is truncated in the middle, because the top and the bottom are
the parts that tell you anything: the top is where it broke, the bottom is
where you started, and two hundred identical recursive frames in between
are noise.

The process exits with status `1`.

## The Messages You Will Actually See

Every message below is real output. Run this and you get all of them:

```zuri
class Config {

  @new() {
    self.name = 'default'
  }
}

def needs_string(s: string) {
  return s
}

def show(label, work) {
  catch {
    work()
  } as e {
    echo '${label}: ${e.type} — ${e.message}'
  }
}

show('name never declared', @() => undeclared_name)
show('key not in dict', @() { var d = { a: 1 }; return d['b'] })
show('field not on class', @() { var c = Config(); c.nope = 1 })
show('property not on class', @() { var c = Config(); return c.missing })
show('method on nil', @() { var x = nil; return x.f() })
show('operator on nil', @() => nil + 1)
show('index past the end', @() => [1][9])
show('wrong argument type', @() => needs_string(5))
```

```console
name never declared: UndefinedError — undefined global 'undeclared_name'
key not in dict: PropertyError — undefined key 'b' in dict
field not on class: PropertyError — undefined field 'nope' on instance of 'Config'
property not on class: PropertyError — undefined property 'missing' on instance of 'Config'
method on nil: TypeError — object of type nil does not define method 'f'
operator on nil: TypeError — operator '+' not defined for call signature (nil, number)
index past the end: RangeError — index 9 out of bounds (length 1)
wrong argument type: TypeError — needs_string() expects parameter 's' (argument 1) to be a string, got number
```

What each one usually means in practice:

| Message | What to look for |
| --- | --- |
| `undefined global 'x'` | a typo, or a `def` that appears **below** the top-level line calling it |
| `undefined key 'x' in dict` | a key that is genuinely absent; `get(key, fallback)` is the fix when absence is legal |
| `undefined field 'x' on instance of 'C'` | a typo in a field name — classes are sealed, so this cannot create one |
| `undefined property 'x' on instance of 'C'` | reading a field or method the class never declared |
| `undefined member 'x' on module m` | the module did not export it, or the export needs an `@` |
| `object of type nil does not define method 'f'` | something upstream returned `nil` |
| `operator '+' not defined for call signature (nil, number)` | the same, one step earlier |
| `'x' is private and can only be accessed via 'self' or 'parent'` | a leading underscore, reached from outside |
| `'x' is already declared in this scope` | two `var`s of one name in one block |
| `cannot assign to constant 'x'` | writing to a local `const` |
| `module 'x' could not be found` | usually a missing leading `.` on a sibling import |

The two `nil` messages are worth internalising. **Zuri never tells you
where a `nil` came from**, because by the time it causes trouble the value
has already been passed along. When you see one, stop looking at the line
that failed and look at whatever produced the value.

## Techniques

### Echo the Value and Its Type

Dynamic typing means the surprise is almost always that something is not
what you assumed it was:

```zuri
var value = '42'

echo typeof(value)
echo value
echo value + 1
echo value.to_number() + 1
```

```console
string
42
421
43
```

One line of `typeof()` would have saved the third line's confusion.

### Print an Instance Through `to_string()`

`echo` does not call `to_string()` for you:

```zuri
class Point {

  @new(x, y) {
    self.x = x
    self.y = y
  }

  to_string() {
    return '(${self.x}, ${self.y})'
  }
}

var p = Point(1, 2)

echo p
echo p.to_string()
```

```console
<instance of Point>
(1, 2)
```

Define `to_string()` on any class you expect to look at while debugging.
The five minutes it costs are repaid the first time you print a list of
them.

### Encode Nested Data Instead of Echoing It

A deep dictionary printed by `echo` is one unreadable line. `json.encode()`
with `compact` off indents it:

```zuri
import json

var request = {
  method: 'POST',
  headers: { accept: 'application/json' },
  body: { title: 'write chapter 19', tags: ['docs', 'zuri'] },
}

echo json.encode(request, false)
```

```console
{
  "method": "POST",
  "headers": {
    "accept": "application/json"
  },
  "body": {
    "title": "write chapter 19",
    "tags": [
      "docs",
      "zuri"
    ]
  }
}
```

### Add Context on the Way Up

An error raised deep in a call chain says what failed, not what you were
doing at the time. Catch it, say what you were doing, and re-raise:

```zuri
def parse_port(raw) {
  if !raw.match('/^\d+$/') {
    raise ValueError('not a number')
  }

  return raw.to_number()
}

def load_settings(source, raw) {
  catch {
    return { port: parse_port(raw) }
  } as e {
    raise ValueError('while reading ${source}: ${e.message}')
  }
}

catch {
  load_settings('config.json', 'eighty')
} as e {
  echo e.message
}
```

```console
while reading config.json: not a number
```

"not a number" is a fact. "while reading config.json: not a number" is a
fact you can act on.

### Print the Stack Trace Yourself

The trace is a list on the error, so a handler can log it without letting
the program die:

```zuri
def inner() {
  raise ValueError('deep')
}

def outer() {
  inner()
}

catch {
  outer()
} as e {
  for frame in e.stacktrace {
    echo frame
  }
}
```

```console
/path/to/main.zu:2 -> inner()
/path/to/main.zu:6 -> outer()
/path/to/main.zu:10 -> @.script()
```

### Ask the Compiler What It Made of Your Code

When an expression does not behave the way you read it, the bytecode
settles the argument:

```zuri
import zuri

echo zuri.compile('var a = 2 * 3 ** 2').map(@(i) => i.op)
```

```console
[LoadConst, MulImm, LoadConst, Pow, SetGlobal, LoadNil, Return]
```

Read the order: the multiply happens **before** the power. That is `**`
sitting at the same precedence level as `*` and associating left, so
`2 * 3 ** 2` is `(2 * 3) ** 2` and not what most people first read. The
bytecode settles it in one line. [Chapter 17](ch17-00-metaprogramming.md)
covers `zuri.compile()` and `zuri.parse()` properly.

### Narrow It With `assert`

An `assert` is a claim you can leave in the code:

```zuri
def average(numbers) {
  assert !numbers.is_empty(), 'average() needs at least one number'

  return numbers.reduce(@(a, b) => a + b, 0) / numbers.length()
}

echo average([2, 4, 6])

catch {
  average([])
} as e {
  echo '${e.type}: ${e.message}'
}
```

```console
4
AssertError: average() needs at least one number
```

Without it, `average([])` would have returned `NaN` and the problem would
have surfaced somewhere else entirely, in a value that is truthy and looks
like a number.

### Watch the Collector

`ZURI_GC_LOG=1` reports garbage collector activity, which is the one to
reach for when memory rather than logic is the question:

```console
$ ZURI_GC_LOG=1 zuri main.zu
```

[Chapter 18](ch18-00-performance.md) lists the rest of the runtime's
diagnostic switches alongside what each one measures.

## The Traps Worth Knowing by Heart

These are the behaviours that produce a *wrong answer* rather than an
error, which makes them far more expensive to find.

**Negative numbers are falsy.** `var n = position or 0` turns `-1` into
`0`, and `if index` is false for `-1` and for `0` alike. Compare
explicitly.

**`NaN` is truthy.** `var x = a / b or fallback` does not protect you from
`0 / 0`. Test with `is_nan()`.

**`to_number()` returns `0` for text it cannot parse.** `'eighty'`, `''`
and `'12abc'` all become `0`, and so does `' 7 '` with its spaces. There is
no `NaN` and no error to catch, so a bad input silently becomes a valid
zero. Validate the text before converting it.

**`[]` and `{}` are truthy.** `if items` is true for an empty list. Use
`is_empty()`.

**`sort()` mutates; `reverse()` does not.** `var s = items.sort()` leaves
`items` sorted as well. Clone first when you need both orders.

**A method call on a string result was discarded.** `name.trim()` does
nothing on its own; strings are immutable, so you must assign the result.

**A nested `def` is module-level.** A function declared inside another
becomes visible everywhere once the outer one runs.

**`x++` evaluates to the new value.** Unlike C and JavaScript.

## Structuring Code You Can Reason About

Three habits pay for themselves the first time something breaks.

**Separate the decision from the effect.** A function that reads a file,
parses it and decides something is three functions. Split them, and the
parsing and the decision can both be exercised without a filesystem in the
way.

**Pass dependencies in.** A function that calls `time()` behaves
differently every second. One that takes a timestamp behaves identically
every time you call it with the same number, which means you can reproduce
a failure instead of waiting for it.

**Return values instead of printing them.** `echo` inside a function is
invisible to its caller and useless to anything that wants to check the
result. Return the string and let the caller decide.
