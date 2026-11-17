# Error Handling

Things go wrong. A file is missing, a number arrives as text, or a server on
the network stops responding. Zuri handles all of these with one mechanism:
an `Error` object that you raise with `raise` and handle with `catch`.
There's no `try`, no `finally` and no checked exceptions.

This chapter covers raising errors, the three forms of `catch`, what an
error contains, the built-in error classes, writing your own, and what to
use instead of `finally`.

## Raising Errors

```zuri
def withdraw(balance, amount) {
  if amount > balance {
    raise ValueError('cannot withdraw ${amount} from ${balance}')
  }
  return balance - amount
}
```

`raise` only accepts an instance of `Error` or one of its subclasses.
Raising a plain string or number is itself a `TypeError`:

```zuri
catch {
  raise 'something went wrong'
} as e {
  echo e.message
}
```

```console
can only raise an Error or subclass, got a string
```

This is a small inconvenience, but it means that every error has a `type`, a
`message` and a stack trace, because there's no way to raise anything else.

If an error isn't caught, the program ends and prints a message, an excerpt
of the source code and a stack trace:

```console
Unhandled ValueError: cannot withdraw 100 from 50
  --> /path/to/main.zu:3

   1 | def withdraw(balance, amount) {
   2 |   if amount > balance {
>  3 |     raise ValueError('cannot withdraw ${amount} from ${balance}')
   4 |   }
   5 |   return balance - amount

Stack trace (most recent call last):
  at withdraw() /path/to/main.zu:3
  at @.script() /path/to/main.zu:8
```

## Catching Errors

```zuri
catch {
  risky()
} as e {
  echo '${e.type}: ${e.message}'
}
```

The code in the `catch` block runs. If anything inside it raises an error,
execution jumps to the handler, with the error assigned to `e`. If nothing
raises an error, the handler doesn't run.

There are three forms of `catch`, and each one behaves differently.

**`catch { ... }`**, with nothing after it, discards the error and
continues:

```zuri
catch {
  raise Error('silent')
}
echo 'swallowed'
```

```console
swallowed
```

Only use this when the failure really doesn't matter.

**`catch { ... } as e`**, without a handler block, assigns the error to a
variable that's still available after the statement. If nothing went wrong,
`e` is `nil`:

```zuri
catch {
  raise Error('boom')
} as e

if e {
  echo e.message
}
```

```console
boom
```

Use this form when the recovery code doesn't belong inside a handler, for
example when you want to check several things one after another.

**`catch { ... } as e { ... }`** is the full form, and the one you'll use
most often.

## What an Error Contains

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

- `message` is the error message.
- `type` is the name of the error's class, as a string.
- `stacktrace` is a list of stack frames, starting with the innermost one.
  Each frame names a file, a line and a function.

The stack trace is recorded where the error was **raised**, not where it was
caught, so it always points to where the error started, however many frames
it passed through:

```zuri
def inner() {
  raise ValueError('deep')
}

def middle() {
  inner()
}

catch {
  middle()
} as e {
  echo e.stacktrace.length()
}
```

```console
3
```

There are three frames: `inner`, `middle` and the top level of the script.
Printing the stack trace is often the quickest way to find out how the
program got to a certain point, especially in code you didn't write.

## Built-in Error Classes

Each of these is a class, and they all inherit from `Error`:

| Class | Raised when |
| --- | --- |
| `Error` | The base class, for general failures |
| `TypeError` | An operation received a value of the wrong type |
| `ValueError` | The type was right, but the value wasn't |
| `NumericError` | An arithmetic operation failed |
| `ArgumentError` | A function received the wrong number of arguments |
| `NotImplementedError` | A method that should have been overridden wasn't |
| `RangeError` | An index or bound was out of range |
| `AccessError` | A permission or access check failed |
| `AssertError` | An `assert` failed |
| `PropertyError` | Code read a member that doesn't exist |
| `UndefinedError` | Code read a name that isn't defined |
| `ModuleNotFoundError` | An `import` couldn't be resolved |

Since they all inherit from `Error`, catching `Error` catches all of them,
and a parameter annotated as `Error` accepts any of them.

## Defining Your Own Errors

Create a subclass of `Error` and give it whatever information the caller
needs:

```zuri
class HttpError < Error {
  @new(message, status) {
    parent(message)
    self.type = 'HttpError'
    self.status = status
  }
}

catch {
  raise HttpError('not found', 404)
} as e {
  echo '${e.type} ${e.status}: ${e.message}'
  echo instance_of(e, Error)
}
```

```console
HttpError 404: not found
true
```

There are two things to get right. Call `parent(message)` so that the base
constructor sets `message` and records the stack trace. And set `self.type`,
so that the class name appears in logs and in the message for an uncaught
error.

## Catching Specific Errors

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
`Error` matches every error, while checking against `HttpError` only matches
that branch of the hierarchy.

## Replacing `finally`

Code after the `catch` statement runs whether or not the block raised an
error, because the handler either recovers from the error or raises it
again:

```zuri
def cleanup_demo() {
  catch {
    raise Error('mid')
  } as e {
    echo 'caught'
  }

  echo 'always runs'
}

cleanup_demo()
```

```console
caught
always runs
```

That covers the most common case. But if the handler raises the error again,
or the block contains a `return`, the code after the statement is skipped.
So if a resource has to be released either way, release it in the handler as
well:

```zuri,ignore
var handle = file(path, 'w')

catch {
  write_everything(handle)
} as e {
  handle.close()
  raise e
}

handle.close()
```

### Using `as e` Without a Handler

The version above calls `handle.close()` twice, once in the handler and once
after the statement, because those are two different ways out. The third
form of `catch`, **`as e` without a handler block**, combines them into one:

```zuri,ignore
var handle = file(path, 'w')

catch {
  write_everything(handle)
} as e

handle.close()

if e {
  raise e
}
```

The only difference is the missing `{ ... }` after `as e`, but it changes
the control flow, not just the layout. Without a handler, there's nothing to
jump to, so an error raised inside the block is **stored in `e`, and
execution continues on the next line**. Both paths, the one that raised an
error and the one that didn't, now run the same code afterwards:

```console
  fallthrough: closing
  fallthrough: re-raising
  caught: disk full
```

This way, `close()` is written once and always runs, and then you decide
explicitly whether to raise the error again. It's the nearest thing Zuri has
to `finally`, built from ordinary statements rather than a separate
construct:

| Line | Purpose |
| --- | --- |
| `catch { ... } as e` | Runs the block and records any error, without jumping |
| `handle.close()` | Cleans up, whichever path was taken |
| `if e { raise e }` | Passes the error on, unchanged |

There are two things to keep in mind.

**`e` is `nil` when nothing went wrong**, so `if e { raise e }` is all you
need to decide what to do. When there's no error, the cleanup runs and the
function continues as normal.

**Raising `e` itself again keeps the original stack trace**, which points to
the line that actually failed rather than to your `raise` statement. Only
wrap it in a new error when you have something to add, as shown in [Nesting
and Re-raising](#nesting-and-re-raising).

Use the form with a handler when you need to handle the error, and this form
when you only need to clean up afterwards.

## `return` Inside `catch`

A `return` inside a `catch` block or its handler returns from the enclosing
function, just as it would anywhere else:

```zuri
def find(items, needle) {
  catch {
    var i = items.index_of(needle)

    if i == -1 {
      raise ValueError('${needle} not in list')
    }

    return items[i]
  } as e {
    echo 'handled: ' + e.message
    return nil
  }
}

echo find([1, 2], 2)
echo find([1, 2], 9)
```

```console
2
handled: 9 not in list
nil
```

## Errors Raised by Handlers

A handler can raise an error, and an outer `catch` will see it:

```zuri
catch {
  catch {
    raise Error('inner')
  } as inner {
    raise Error('outer: ' + inner.message)
  }
} as outer {
  echo outer.message
}
```

```console
outer: inner
```

## `assert` or `raise`?

```zuri,ignore
assert items.length() > 0, 'caller must pass a non-empty list'
```

`assert` raises an `AssertError` when its condition is falsy. The message is
optional.

Here's how to decide between them. **Use `raise` for things that can
happen**, such as a missing file, a number that doesn't parse or a network
timeout. The caller is expected to deal with these. **Use `assert` for
things that should never happen**, such as an invariant that your own code
is responsible for keeping. If an assertion fails, the program has a bug. It
doesn't mean something outside the program went wrong.

## Good Practices

Keep the `catch` block small. It should contain only the operation that
might fail, so that the handler doesn't accidentally catch a mistake
somewhere else:

This block is too wide. A failure inside `render()` would be reported as a
parsing failure:

```zuri,ignore
def show_wide(text) {
  catch {
    var data = json.decode(text)
    render(data)
  } as e {
    echo 'bad json'
  }
}
```

This is better. Only the call that might fail is inside the block:

```zuri,ignore
def show_narrow(text) {
  var data

  catch {
    data = json.decode(text)
  } as e {
    echo 'bad json: ' + e.message
    return
  }

  render(data)
}
```

Notice that `var data` is declared outside the block. A `catch` block is a
scope like any other, so a variable declared inside it no longer exists once
the statement ends.

Write error messages that include the value that caused the problem:

```zuri,ignore
raise ValueError('port must be between 1 and 65535, got ${port}')
```

Whoever reads that message is trying to work out what went wrong from a
single line in a log file. Give them the number.

## Catching Inside a Loop

A `catch` inside a loop body handles a single iteration and lets the rest of
the loop continue. Use this when you're processing a batch of items and some
of them are allowed to fail:

```zuri
def parse_positive(text) {
  if !text.match('/^\d+$/') {
    raise ValueError('not a number: ${text}')
  }

  var n = text.to_number()

  if n <= 0 {
    raise ValueError('must be positive: ${text}')
  }

  return n
}

var inputs = ['12', 'not a number', '30']
var total = 0
var rejected = []

for raw in inputs {
  catch {
    total += parse_positive(raw)
  } as e {
    rejected.append(raw)
  }
}

echo total
echo rejected
```

```console
42
[not a number]
```

If you put the `catch` **outside** the loop instead, the first failure ends
the whole loop. That's the right choice when one bad item makes the rest
pointless, and the wrong choice when it doesn't. Where you put the block is
how you make that decision, since there's no flag for it.

## Nesting and Re-raising

A `catch` inside a handler works like any other `catch`. You can use this to
turn a low-level failure into an error that makes sense to your caller:

```zuri
import json

class ConfigError < Error {

  @new(message) {
    parent(message)
    self.type = 'ConfigError'
  }
}

def load(text) {
  catch {
    return json.decode(text)
  } as e {
    raise ConfigError('config is not valid JSON: ${e.message}')
  }
}

catch {
  load('{ broken')
} as e {
  echo '${e.type}: ${e.message}'
}
```

The caller now gets an error that's in terms it understands. Include the
original message, as shown above, so the details aren't lost along the way.

Raising the **same** error again, instead of a new one, keeps the original
stack trace, which points to the line where the error first happened:

```zuri
def only_handle_missing(work) {
  catch {
    return work()
  } as e {
    if !instance_of(e, ModuleNotFoundError) {
      raise e
    }

    return 'defaulted'
  }
}

echo only_handle_missing(@() => 'fine')

catch {
  only_handle_missing(@() {
    raise ValueError('not mine')
  })
} as e {
  echo '${e.type}: ${e.message}'
}
```

```console
fine
ValueError: not mine
```

This is the pattern for handling one kind of error and letting all the
others through. Use it whenever a handler might otherwise hide a bug along
with the failure it was meant to catch.

## Example: Loading a File

Here's everything in this chapter in a single function: a loader that
validates its input, separates the failures a caller can do something about
from the ones it can't, and closes the file it opened whatever happens.

```zuri
import json

class StoreError < Error {

  @new(message) {
    parent(message)
    self.type = 'StoreError'
  }
}

def read_records(path) {
  var handle = file(path)

  if !handle.exists() {
    raise StoreError('no store at ${path}')
  }

  var records

  catch {
    records = json.decode(handle.read())
  } as e {
    handle.close()
    raise StoreError('${path} is corrupt: ${e.message}')
  }

  handle.close()

  if !is_list(records) {
    raise StoreError('${path} should hold a list, found ${typeof(records)}')
  }

  return records
}

file('records.json', 'w').write('[{"id": 1}]')
echo read_records('records.json').length()

file('records.json', 'w').write('{ not json')

catch {
  read_records('records.json')
} as e {
  echo e.type
}

file('records.json').delete()

catch {
  read_records('records.json')
} as e {
  echo '${e.type}: ${e.message}'
}
```

```console
1
StoreError
StoreError: no store at records.json
```

There are four things worth pointing out.

Every failure the caller might reasonably handle is reported as a single
error type, `StoreError`, so the `catch` on the calling side needs one
branch instead of three.

The message always includes the path. A log line that says "file is corrupt"
without saying which file can cost someone an hour.

`handle.close()` is called on both paths: once in the handler before the
error is raised again, and once after the block. There's no `finally` to do
this for you, and forgetting the call in the handler is the most common way
Zuri code leaks resources.

Finally, the check on the file's contents at the end uses `raise`, not
`assert`. A file on disk with the wrong contents is a problem with the
outside world, not a bug in this function.
