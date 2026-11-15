# Appendix G: The Error Hierarchy

Every error in Zuri is an instance of a class, and all of them inherit from
`Error`. They're declared in ordinary Zuri and use the same class machinery
as your own code, which is why subclassing one works exactly like
subclassing anything else.

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

Every error has three fields:

| Field | What it holds |
| --- | --- |
| `message` | The error message. The default is `'An unexpected error has occurred'`. |
| `type` | The name of the error's class, as a string |
| `stacktrace` | A list of stack frames, starting with the innermost one |

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

## When Each Error Is Raised

| Class | Raised when |
| --- | --- |
| `Error` | The base class, for general failures with nothing more specific to report |
| `TypeError` | An operation received a value of the wrong type, such as an operator used with unsupported types, a method called on `nil`, or the wrong kind of value passed to an annotated parameter |
| `ValueError` | The type was right, but the value wasn't |
| `NumericError` | An arithmetic operation failed |
| `ArgumentError` | A call passed the wrong number of arguments to a native function |
| `NotImplementedError` | A method that should have been overridden wasn't |
| `RangeError` | An index or bound was outside what the value allows |
| `AccessError` | A permission or access check failed |
| `AssertError` | An `assert` condition was falsy |
| `PropertyError` | Code read a member that doesn't exist, such as a missing dictionary key, an undeclared field or a module member that wasn't exported |
| `UndefinedError` | Code read an undefined global name |
| `ModuleNotFoundError` | An `import` couldn't be resolved |

## Catching Errors

`catch` catches every error raised inside its block. To handle one kind of
error and let the others through, check the error and raise it again if it
isn't the kind you want:

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

`instance_of()` checks the whole inheritance chain, so checking against
`Error` matches every error.

A parameter annotated as `Error` accepts any error, which makes a handler
easy to read:

```zuri
def report(e: Error) {
  echo '${e.type}: ${e.message}'
}
```

## Defining Your Own Errors

```zuri
class HttpError < Error {
  @new(message, status) {
    parent(message)

    self.type = 'HttpError'
    self.status = status
  }
}
```

There are two things to get right. Call `parent(message)` so that the base
constructor sets `message` and records the stack trace. And set `self.type`,
so that the class name appears in logs and in the message for an uncaught
error.

Give the error whatever information its handler needs. Being able to carry
more than a string is the reason to write an error class at all. The
capstone project's `TaskError` includes the name of the field that failed
validation, which is what lets its API respond with
`{"error": "...", "field": "title"}`.

## Uncaught Errors

An error that nothing catches prints its type, its message, the source code
around the failure and the stack trace, and the program then exits with
status `1`:

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

A deep stack trace is shortened in the middle, because the top and the
bottom are the useful parts.

## There's No `finally`

Code after a `catch` statement runs whether or not the block raised an
error, because the handler either recovers from the error or raises it
again. [Error Handling](ch07-00-error-handling.md) describes the patterns to
use instead of `finally`.
