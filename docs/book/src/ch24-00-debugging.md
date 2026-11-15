# Debugging

Something isn't doing what you expected. This chapter is about tracking down
why: reading what the runtime tells you, recognizing the few messages that
cause most of the confusion, and the techniques that turn a vague "it's
broken" into a line number.

## Reading an Error Message

An uncaught error prints three things: what went wrong, where it went wrong,
and how the program got there.

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

The error type and message come first, then the source code around the
failure, with the line that failed marked, and finally the call stack,
starting with the innermost call.

A deep stack is shortened in the middle, because the top and the bottom are
the useful parts. The top shows where the error happened, the bottom shows
where your program started, and two hundred identical recursive calls in
between don't tell you anything.

The process exits with status `1`.

## Common Error Messages

Every message below is real output. If you run this, you'll see all of them:

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
show('key not in dict', @() {
  var d = { a: 1 }
  return d['b']
})
show('field not on class', @() {
  var c = Config()
  c.nope = 1
})
show('property not on class', @() {
  var c = Config()
  return c.missing
})
show('method on nil', @() {
  var x = nil
  return x.f()
})
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

Here's what each one usually means in practice:

| Message | What to look for |
| --- | --- |
| `undefined global 'x'` | A typo, or a `def` that's declared **below** the top-level line that calls it |
| `undefined key 'x' in dict` | A key that really isn't there. If it's allowed to be missing, use `get(key, fallback)`. |
| `undefined field 'x' on instance of 'C'` | A typo in a field name. Classes are sealed, so this can't create a new field. |
| `undefined property 'x' on instance of 'C'` | Reading a field or method that the class never declared |
| `undefined member 'x' on module m` | The module doesn't export it, or the import needs an `@` to re-export it |
| `object of type nil does not define method 'f'` | Something earlier in the code returned `nil` |
| `operator '+' not defined for call signature (nil, number)` | The same problem, one step earlier |
| `'x' is private and can only be accessed via 'self' or 'parent'` | A name with a leading underscore, used from outside the class |
| `'x' is already declared in this scope` | Two `var` declarations with the same name in the same block |
| `cannot assign to constant 'x'` | Assigning to a local `const` |
| `module 'x' could not be found` | Usually a missing leading `.` on an import of a sibling module |

The two messages about `nil` are worth remembering. **Zuri can't tell you
where a `nil` came from**, because by the time it causes a problem, the
value has already been passed along. When you see one, don't look at the
line that failed. Look at whatever produced the value.

## Debugging Techniques

### Print the Value and Its Type

In a dynamically typed language, the surprise is almost always that a value
isn't what you assumed:

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

A single `typeof()` would have cleared up the confusion on the third line.

### Give Your Classes `@to_string()`

`echo` displays an instance using its class's `@to_string()` method, or as
`<instance of Point>` if the class doesn't have one:

```zuri
class Point {

  @new(x, y) {
    self.x = x
    self.y = y
  }

  @to_string() {
    return '(${self.x}, ${self.y})'
  }
}

var p = Point(1, 2)

echo p
echo [p, Point(3, 4)]
```

```console
(1, 2)
[(1, 2), (3, 4)]
```

Define `@to_string()` on any class you expect to look at while debugging. It
only takes a few minutes, and it pays off the first time you print a list of
instances.

### Encode Nested Data Instead of Printing It

A deeply nested dictionary printed with `echo` is a single unreadable line.
`json.encode()` with `compact` turned off indents it for you:

```zuri
import json

var request = {
  method: 'POST',
  headers: { accept: 'application/json' },
  body: {
    title: 'write chapter 19',
    tags: ['docs', 'zuri'],
  },
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

### Add Context to Errors

An error raised deep in a chain of calls says what failed, but not what the
program was doing at the time. Catch it, add what you were doing, and raise
it again:

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

"Not a number" is a fact. "While reading config.json: not a number" is a
fact you can do something about.

### Print the Stack Trace Yourself

The stack trace is a list on the error object, so a handler can log it
without letting the program crash:

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

### Check How the Compiler Read Your Code

When an expression doesn't behave the way you read it, the bytecode will
tell you how the compiler read it:

```zuri
import zuri

echo zuri.compile('var a = -2 ** 2').map(@(i) => i.op)
```

```console
[LoadConst, LoadConst, Pow, Neg, SetGlobal, LoadNil, Return]
```

Look at the order: the power is calculated **before** the negation. `**`
binds more tightly than unary minus, so `-2 ** 2` means `-(2 ** 2)`, which
is `-4`, not the `4` you'd get from `(-2) ** 2`. One line of bytecode
answers the question. [Chapter 21](ch21-00-metaprogramming.md) covers
`zuri.compile()` and `zuri.parse()` in detail.

### Narrow Down the Problem With `assert`

An `assert` states something you believe is true, and you can leave it in
the code:

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

Without it, `average([])` would have returned `NaN`, and the problem would
have shown up somewhere else entirely, as a value that looks like a number.

### Watch the Garbage Collector

`ZURI_GC_LOG=1` reports what the garbage collector is doing. Use it when the
problem is memory rather than logic:

```console
$ ZURI_GC_LOG=1 zuri run main.zu
```

[Chapter 22](ch22-00-performance.md) lists the runtime's other diagnostic
settings, along with what each one measures.

## Common Pitfalls

These behaviors produce a *wrong result* rather than an error, which makes
them much harder to track down.

**Zero is falsy.** `var n = count or 10` turns a real `0` into `10`, and
`if index` is false for the first position but true for `-1`, which means
"not found". Compare values explicitly.

**`to_number()` returns `0` for text it can't parse.** `'eighty'`, `''` and
`'12abc'` all become `0`, and so does `' 7 '` with its spaces. There's no
`NaN` and no error to catch, so invalid input silently becomes a valid zero.
Validate the text before you convert it.

**`[]` and `{}` are truthy.** `if items` is true for an empty list. Use
`is_empty()` instead.

**`sort()` changes the list, but `reverse()` doesn't.**
`var s = items.sort()` sorts `items` as well. If you need both orders, clone
the list first.

**The result of a string method was thrown away.** `name.trim()` on its own
does nothing. Strings are immutable, so you have to assign the result.

**A nested `def` is local.** A function declared inside another function is
scoped to it, like a `var`, and nothing outside that scope can call it.

**`x++` evaluates to the new value**, unlike in C and JavaScript.

## Writing Code That's Easier to Debug

Three habits pay off the first time something breaks.

**Keep decisions separate from side effects.** A function that reads a file,
parses it and makes a decision based on it is really three functions. Split
them up, and you can test the parsing and the decision without needing a
file system.

**Pass dependencies in.** A function that calls `time()` behaves differently
every second, while a function that takes a timestamp as an argument behaves
exactly the same every time you call it with the same number. That means you
can reproduce a failure instead of waiting for it to happen again.

**Return values instead of printing them.** An `echo` inside a function is
invisible to the code that called it, and useless to anything that wants to
check the result. Return the string, and let the caller decide what to do
with it.
