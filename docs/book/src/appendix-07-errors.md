# Appendix G: The Error Hierarchy

Every error in Zuri is an instance of a class, and every one of them
inherits from `Error`. They are declared in ordinary Zuri and go through
the same class machinery user code does, which is why subclassing one
behaves exactly like subclassing anything else.

```text
Error
├── TypeError
├── ValueError
├── NumericError
├── ArgumentError
├── NotImplementedError
├── RangeError
├── AccessError
├── AssertError
├── PropertyError
├── UndefinedError
└── ModuleNotFoundError
```

## Fields

Every error carries three:

| Field | What it holds |
| --- | --- |
| `message` | the text, defaulting to `'An unexpected error has occurred'` |
| `type` | the class name, as a string |
| `stacktrace` | a list of frames, innermost first |

```zuri
catch {
  raise ValueError('bad input')
} as e {
  echo e.type
  echo e.message
  echo e.stacktrace
}
```

```console
ValueError
bad input
[/path/to/main.zu:2 -> @.script()]
```

## When Each One Is Raised

| Class | Raised when |
| --- | --- |
| `Error` | the base class; a general failure with nothing more specific to say |
| `TypeError` | an operation received the wrong type: an undefined operator signature, a method on `nil`, an annotated parameter given the wrong thing |
| `ValueError` | the type was right and the value was not |
| `NumericError` | an arithmetic operation failed |
| `ArgumentError` | a call passed the wrong number of arguments to a native function |
| `NotImplementedError` | a method meant to be overridden was not |
| `RangeError` | an index or a bound fell outside what the value allows |
| `AccessError` | a permission or access check failed |
| `AssertError` | an `assert` condition was falsy |
| `PropertyError` | a member that does not exist was read: a missing dictionary key, an undeclared field, a module member that was not exported |
| `UndefinedError` | an undefined global was read |
| `ModuleNotFoundError` | an `import` could not be resolved |

## Catching

`catch` catches everything inside its block. To handle one kind and let the
rest through, test and re-raise:

```zuri,ignore
catch {
  load_config()
} as e {
  if !instance_of(e, ModuleNotFoundError) {
    raise e
  }

  echo 'no config, using defaults'
}
```

`instance_of()` walks the whole chain, so a test against `Error` matches
everything.

A parameter annotated `Error` accepts any of them, which is the readable
way to write a handler:

```zuri
def report(e: Error) {
  echo '${e.type}: ${e.message}'
}
```

## Subclassing

```zuri
class HttpError < Error {
  @new(message, status) {
    parent(message)

    self.type = 'HttpError'
    self.status = status
  }
}
```

Two things make this work well. Call `parent(message)` so the base
constructor sets `message` and the stack trace is captured. Set
`self.type` so the class name appears in logs and in the uncaught-error
banner.

Carry whatever the handler needs. An error class exists precisely so it can
hold more than a string; the capstone's `TaskError` carries the name of the
field that failed validation, and that is what lets an API answer
`{"error": "...", "field": "title"}`.

## Uncaught

An error nobody catches prints its type, its message, the source around the
failure, and the stack trace, then exits `1`:

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

A deep stack is truncated in the middle. The top and the bottom are the
parts that tell you anything.

## There Is No `finally`

Code after a `catch` statement runs whether the block raised or not,
because the handler either recovers or re-raises. See
[Error Handling](ch07-00-error-handling.md) for the patterns that replace
it.
