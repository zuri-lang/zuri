# Type Annotations

Zuri is dynamically typed, and parameters may be annotated. An annotated
parameter is **checked at every call**, and a mismatch raises a `TypeError`
naming the parameter, its position and what actually arrived.

```zuri
def repeat(text: string, times: number) {
  return text * times
}

echo repeat('ab', 3)

catch {
  repeat(42, 3)
} as e {
  echo e.message
}
```

```console
ababab
repeat() expects parameter 'text' (argument 1) to be a string, got number
```

## The Type Names

| Name | Accepts |
| --- | --- |
| `any` | anything, including `nil` |
| `bool` | `true` or `false` |
| `number` | any number |
| `int` | a number with no fractional part |
| `bigint` | an arbitrary-precision integer |
| `string` | a string |
| `bytes` | a byte buffer |
| `list` | a list |
| `dict` | a dictionary |
| `range` | a range |
| `file` | a file handle |
| `function` | a function or an anonymous function |
| `callable` | anything callable, classes included |
| `iterable` | anything `for` can walk |
| `type` | a class |
| *ClassName* | an instance of that class or a subclass |

Anything that is not one of those names is read as a class name.

`number` and `bigint` are separate, in annotations exactly as they are
everywhere else. A parameter typed `bigint` rejects `10`, and one typed
`number` rejects `10n`:

```zuri
def factorial(n: bigint) {
  if n <= 1n {
    return 1n
  }

  return n * factorial(n - 1n)
}

echo factorial(25n)

catch {
  factorial(25)
} as e {
  echo e.message
}
```

```console
15511210043330985984000000n
factorial() expects parameter 'n' (argument 1) to be a bigint, got number
```

Write `bigint|number` when a function genuinely takes either, and convert
inside it with `to_bigint()`.

## Required by Default

A plain annotation makes the argument **required**. Omitting it is a
`TypeError`, because the parameter arrives as `nil`:

```zuri
def needs_number(x: number) {}

catch {
  needs_number()
} as e {
  echo e.message
}
```

```console
needs_number() expects parameter 'x' (argument 1) to be a number, got nil
```

That is how you get argument checking out of a language that otherwise
lets you call anything with anything.

## Optional Parameters

Prefix the type with `?` to allow `nil`:

```zuri
def connect(host: string, port: ?number) {
  port = port or 8080
  echo '${host}:${port}'
}

connect('localhost')
connect('localhost', 9000)
```

```console
localhost:8080
localhost:9000
```

`?any` is the same as no annotation at all, which is what an unannotated
parameter gets.

## Union Types

Separate alternatives with `|`:

```zuri
def render(value: string|number|list) {
  echo value
}

render('text')
render(42)
render([1])

catch {
  render({ a: 1 })
} as e {
  echo e.message
}
```

```console
text
42
[1]
render() expects parameter 'value' (argument 1) to be a string, a number, or a list, got dict
```

The `?` goes at the front and applies to the whole union: `?string|number`.

## Classes and Subclasses

A class name accepts instances of that class and of anything that inherits
from it:

```zuri
class Point {
  @new(x, y) {
    self.x = x
    self.y = y
  }
}

def distance_from_origin(p: Point) {
  return (p.x * p.x + p.y * p.y).sqrt()
}

echo distance_from_origin(Point(3, 4))
```

```console
5
```

This is what makes error handling readable, since every built-in error
subclasses `Error`:

```zuri
def report(e: Error) {
  echo '${e.type}: ${e.message}'
}

catch {
  raise ValueError('bad value')
} as e {
  report(e)
}
```

```console
ValueError: bad value
```

## Where Annotations Work

Annotations go on parameters: in a `def`, in a method, and in an anonymous
function.

```zuri
class Report {
  render(rows: list, title: ?string) {
    # ...
  }
}

var f = @(n: int) => n * 2
```

The same syntax goes on `var` declarations and on class fields:

```zuri
var count: number = 0
var name: ?string

class Report {
  var rows: list = []
  var title: ?string
}
```

A variable annotation is a statement of intent. It is not checked at
runtime, so the runtime will not stop you from putting a string in
`count`. What it does is make the declaration self-documenting, and give
static analysis tools something to work with: an annotated declaration
tells a linter, an editor's completion engine or a type checker exactly
what belongs there, and lets them flag the assignment your eyes would
otherwise have to catch.

Annotate declarations for the reader and the tooling. Annotate parameters
for the runtime.

## When to Annotate

Annotations are optional, and a program with none of them is perfectly
ordinary Zuri. The question is where they earn their place.

**Annotate a boundary.** A function that receives data from outside your
program — a request handler, a file parser, a public function in a module
other people import — is where a wrong type first arrives. An annotation
there turns a confusing failure deep in the call stack into a clear one at
the door:

```zuri
def parse_port(raw: string) {
  var port = raw.to_number()

  if port < 1 or port > 65535 {
    raise ValueError('port out of range: ${raw}')
  }

  return port
}

catch {
  parse_port(8080)
} as e {
  echo e.message
}

echo parse_port('8080')
```

```console
parse_port() expects parameter 'raw' (argument 1) to be a string, got number
8080
```

Note the division of labour there. The annotation handles *wrong type*, and
the explicit check handles *wrong value*. An annotation can never do the
second job, because `70000` is a perfectly good number.

**Annotate to replace a manual check.** Any function that opens with
`if !is_string(x) { raise TypeError(...) }` is spelling out by hand what an
annotation says in one word, and the annotation produces a better message:

```zuri
def shout(text: string) {
  return text.upper() + '!'
}

echo shout('hello')
```

```console
HELLO!
```

**Leave internal helpers alone if you prefer.** A private function called
from three places in the same file, all of which you can see, gains less.
Annotate it if it documents something non-obvious; skip it if it does not.

One thing an annotation is not: a substitute for validation. `text: string`
guarantees you have a string, not that the string is a valid email address,
a well-formed date or a non-empty name. The `validate` module covers that
job, and [Chapter 13](ch13-00-stdlib-tour.md) introduces it.
