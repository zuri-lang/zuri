# Type Annotations

Zuri is dynamically typed, but you can annotate parameters with types. An
annotated parameter is **checked on every call**, and if the argument
doesn't match, Zuri raises a `TypeError` that names the parameter and its
position, and says what was actually passed.

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

## Type Names

| Name | Accepts |
| --- | --- |
| `any` | Anything, including `nil` |
| `bool` | `true` or `false` |
| `number` | Any number |
| `int` | A number with no fractional part |
| `bigint` | An arbitrary-precision integer |
| `string` | A string |
| `bytes` | A byte buffer |
| `list` | A list |
| `dict` | A dictionary |
| `range` | A range |
| `file` | A file handle |
| `function` | A named or anonymous function |
| `callable` | Anything that can be called, including classes |
| `iterable` | Anything `for` can loop over |
| `type` | A class |
| *ClassName* | An instance of that class or one of its subclasses |

Any other name is treated as the name of a class.

`number` and `bigint` are separate types in annotations, just as they are
everywhere else. A parameter annotated as `bigint` rejects `10`, and one
annotated as `number` rejects `10n`:

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

If a function really does accept either one, write `bigint|number`, and
convert the value inside the function with `to_bigint()`.

## Annotated Parameters Are Required

A plain annotation makes the argument **required**. Leaving it out raises a
`TypeError`, because the parameter receives `nil`:

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

That's how you get argument checking in a language that otherwise lets you
call any function with any arguments.

## Optional Parameters

To allow `nil`, put a `?` in front of the type:

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

`?any` is the same as having no annotation at all, which is what unannotated
parameters get.

## Union Types

To accept several types, separate them with `|`:

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

The `?` goes at the front and applies to the whole union, as in
`?string|number`.

## Classes and Subclasses

A class name accepts instances of that class and of any class that inherits
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

The name means the same as it would inside the function body, so the class
can be declared in the same file, imported, or stored in a variable that the
function can see:

```zuri
import date { Date, from_time }

def year_of(moment: Date) {
  return moment.year
}

echo year_of(from_time(0))
```

```console
1970
```

This makes error handling easier to read, since all of the built-in errors
are subclasses of `Error`:

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

## Where You Can Use Annotations

You can annotate the parameters of a `def`, a method or an anonymous
function.

```zuri
class Report {
  render(rows: list, title: ?string) {
    # ...
  }
}

var f = @(n: int) => n * 2
```

The same syntax also works on `var` declarations and class fields:

```zuri
var count: number = 0
var name: ?string

class Report {
  var rows: list = []
  var title: ?string
}
```

An annotation on a variable describes what you intend to store in it, but it
isn't checked at run time, so nothing stops you from putting a string in
`count`. What it does is document the declaration and give tools something
to work with. A linter, an editor's completion engine or a type checker can
read the annotation, know what belongs in the variable, and flag an
assignment that you'd otherwise have to spot by eye.

Annotate declarations for people and tools, and annotate parameters for the
runtime.

## When to Use Annotations

Annotations are optional, and a program that doesn't use them at all is
perfectly normal Zuri. The real question is where they're most useful.

**Annotate at the boundaries of your code.** A function that receives data
from outside your program, such as a request handler, a file parser or a
public function in a module that other people import, is where a value of
the wrong type first shows up. An annotation there replaces a confusing
failure deep in the call stack with a clear error at the point where the bad
value came in:

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

Notice how the work is divided. The annotation catches the *wrong type*, and
the explicit check catches the *wrong value*. An annotation can never do the
second job, because `70000` is a perfectly valid number.

**Use annotations instead of manual type checks.** If a function starts with
`if !is_string(x) { raise TypeError(...) }`, it's doing by hand what an
annotation does in one word, and the annotation gives a better error
message:

```zuri
def shout(text: string) {
  return text.upper() + '!'
}

echo shout('hello')
```

```console
HELLO!
```

**Annotating internal helpers is up to you.** A private function that's only
called from three places in the same file, all of which you can see, gains
less from annotations. Add them if they document something that isn't
obvious, and leave them out if they don't.

Keep in mind that an annotation isn't a replacement for validation.
`text: string` guarantees that you have a string, but not that the string is
a valid email address, a correctly formatted date or a non-empty name.
That's what the `validate` module is for, which is introduced in [Chapter
13](ch13-00-stdlib-tour.md).
