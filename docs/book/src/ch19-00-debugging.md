# Debugging

## Reading an Error

An uncaught error prints three things:

```console
Unhandled ValueError: bottomed out
  --> /path/to/main.zu:3

  1 | def recurse(n) {
  2 |   if n <= 0 {
> 3 |     raise ValueError("bottomed out")
  4 |   }
  5 |   recurse(n - 1)

Stack trace (most recent call last):
  at recurse() /path/to/main.zu:3
  at recurse() /path/to/main.zu:5
  ... 19 more frames ...
  at recurse() /path/to/main.zu:5
```

The type and message, the source around the failure with the line marked,
and the call stack innermost first. A deep stack is truncated in the
middle, because the top and the bottom are the parts that tell you
anything.

The process exits `1`.

## What the Common Errors Mean

| Message | Cause |
| --- | --- |
| `undefined global 'x'` | the name was never declared, or is declared later in the file |
| `undefined field 'x' on instance of 'C'` | the class has no such field; classes are sealed |
| `undefined key 'x' in dict` | bracket or dot read of a key that is not there; use `get()` |
| `undefined member 'x' on module m` | the module did not export it, or the export needs `@` |
| `object of type nil does not define method 'f'` | something upstream returned `nil` |
| `operator '+' not defined for call signature (nil, number)` | same, one step earlier |
| `'x' is private and can only be accessed via 'self' or 'parent'` | leading underscore, reached from outside |
| `'x' is already declared in this scope` | two `var`s of the same name in one block |
| `cannot assign to constant 'x'` | writing to a local `const` |
| `module 'x' could not be found` | check for a missing leading `.` on a relative import |
| `Unexpected token "?"` | a conditional expression split across lines; keep `? :` on one line |

The two nil-related messages are worth internalising. Zuri does not tell
you where the `nil` came from, because by the time it matters the value has
already been passed along. When you see one, look at what produced the
value, not at the line that failed.

## Techniques

**Echo the value and its type.** Dynamic typing means the surprise is
usually that something is not what you assumed:

```zuri
echo typeof(value)
echo value
```

**Print an instance through `to_string()`.** `echo` prints
`<instance of Thing>` for an instance. Define `to_string()` on any class
you expect to look at, and call it.

**Use `json.encode(x, false)` for nested data.** A deep dictionary printed
by `echo` is one long line. Encoded with `compact` off, it is readable:

```zuri
import json

echo json.encode(request_body, false)
```

**Catch and re-raise to add context.** An error thrown deep in a call
chain says what failed, not what you were doing:

```zuri
catch {
  parse_config(path)
} as e {
  raise ValueError('while reading ${path}: ' + e.message)
}
```

**Check the bytecode when the behaviour makes no sense:**

```zuri
import zuri

echo zuri.compile(source).map(@(i) => i.op)
```

**Narrow the tier.** If something works in one run and not another, take
the JIT out of the picture:

```console
$ ZURI_JIT=0 zuri main.zu
```

If that fixes it, the problem is in the compiled path, and
`ZURI_JIT_LOG=1` tells you which function got compiled.

**Watch the collector** with `ZURI_GC_LOG=1` when memory is the question,
and **the opcode histogram** with `ZURI_OPCODE_PROFILE=1` when you want to
know what the interpreter is actually spending its time on.

## Structuring Code You Can Reason About

Three habits pay for themselves the first time something breaks.

**Separate the decision from the effect.** A function that reads a file,
parses it and decides something is three functions. Split them, and the
parse and the decision can be exercised without a filesystem in the way.

**Pass dependencies in.** A function that calls `time()` behaves
differently every second. One that takes a timestamp behaves the same way
every time you call it with the same number.

**Return values instead of printing.** `echo` inside a function is
invisible to a caller. Return the string and let the caller decide what to
do with it.
