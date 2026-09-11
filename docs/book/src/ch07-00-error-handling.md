# Error Handling

Things go wrong. A file is not there, a number arrives as text, a network
peer stops answering. Zuri has one mechanism for all of it: an `Error`
object, raised with `raise` and intercepted with `catch`. There is no
`try`, no `finally`, and no checked exceptions.

This chapter covers raising, the three shapes of `catch`, what an error
carries, the built-in hierarchy, writing your own, and the patterns that
replace `finally`.

## Raising

```zuri
def withdraw(balance, amount) {
  if amount > balance {
    raise ValueError('cannot withdraw ${amount} from ${balance}')
  }
  return balance - amount
}
```

`raise` takes an instance of `Error` or any subclass, and nothing else. A
bare string or number is a `TypeError` in its own right:

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

That rule is worth the small inconvenience: every value that travels
through the error system has a `type`, a `message` and a stack trace,
because there is no way to put anything else in.

An error that nobody catches ends the program with a message, a source
excerpt and a stack trace:

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

## Catching

```zuri
catch {
  risky()
} as e {
  echo '${e.type}: ${e.message}'
}
```

The `catch` block runs. If anything inside it raises, execution jumps to
the handler with the error bound to `e`. If nothing raises, the handler
never runs.

There are three shapes, and each does something different.

**`catch { ... }`** with nothing after it swallows the error and carries
on:

```zuri
catch {
  raise Error('silent')
}
echo 'swallowed'
```

```console
swallowed
```

Use this when failure genuinely does not matter, and nowhere else.

**`catch { ... } as e`** with no handler block binds the error to a
variable that survives the statement. `e` is `nil` when nothing went
wrong:

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

This is the shape to use when the recovery does not belong inside a
handler, for example when you want to check several things in a row.

**`catch { ... } as e { ... }`** is the full form, and the one you will
write most.

## What an Error Carries

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

- `message` is the text.
- `type` is the class name, as a string.
- `stacktrace` is a list of frames, innermost first, each naming a file, a
  line and a function.

The stack trace is captured where the error was **raised**, not where it
was caught, so it points at the origin no matter how many frames it
travelled through:

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

Three frames: `inner`, `middle`, and the script's top level. Printing them
is often the fastest way to answer "how did we get here?" in code you did
not write.

## The Built-in Errors

Every one of these is a class, and every one inherits from `Error`:

| Class | Raised when |
| --- | --- |
| `Error` | the base; a general failure |
| `TypeError` | an operation got the wrong type |
| `ValueError` | the type was right, the value was not |
| `NumericError` | an arithmetic operation failed |
| `ArgumentError` | wrong number of arguments |
| `NotImplementedError` | a method that must be overridden was not |
| `RangeError` | an index or bound was out of range |
| `AccessError` | a permission or access check failed |
| `AssertError` | an `assert` failed |
| `PropertyError` | a member that does not exist was read |
| `UndefinedError` | an undefined name was read |
| `ModuleNotFoundError` | an `import` could not be resolved |

Because they all inherit from `Error`, catching `Error` catches everything,
and a parameter annotated `Error` accepts any of them.

## Custom Errors

Subclass `Error` and carry whatever the caller needs:

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

Two things make this work well. Call `parent(message)` so the base
constructor sets `message` and captures the stack trace. Set `self.type` so
the class name shows up in logs and in the uncaught-error banner.

## Deciding Which Error to Catch

`catch` catches everything inside its block. To handle one kind and let the
others through, test and re-raise:

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

`instance_of()` walks the inheritance chain, so a test against `Error`
matches everything and a test against `HttpError` matches only that branch.

## There Is No `finally`

Code after the `catch` statement runs whether the block raised or not,
because the handler either recovers or re-raises:

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

That covers the common case. When the handler re-raises, or when the block
contains a `return`, the trailing code is skipped, so a resource that must
be released either way goes in the handler as well:

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

Closing twice is safe, so the simpler form is usually fine:

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

## `return` Inside `catch`

A `return` from inside a `catch` block or its handler leaves the enclosing
function, exactly as it would anywhere else:

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

## Nesting

Handlers can raise, and an outer `catch` will see it:

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

## `assert` Versus `raise`

```zuri,ignore
assert items.length() > 0, 'caller must pass a non-empty list'
```

`assert` raises an `AssertError` when its condition is falsy. The message
is optional.

Draw the line like this. **`raise` is for things that happen**: a file that
is not there, a number that does not parse, a network that times out. The
caller is expected to handle it. **`assert` is for things that cannot
happen**: an invariant the code itself is responsible for maintaining. A
failed assertion means the program has a bug, not that the world was
uncooperative.

## Style

Keep the `catch` block small. The block should contain the operation that
can fail and nothing else, so the handler is not accidentally catching a
mistake somewhere further down:

```zuri
# Too wide: a failure inside render() gets reported as a parse failure.
def show(text) {
  catch {
    var data = json.decode(text)
    render(data)
  } as e {
    echo 'bad json'
  }
}

# Right.
def show(text) {
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

Note the `var data` outside the block. A `catch` block is a scope like any
other, so a variable declared inside it is gone by the time the next
statement runs.

Write messages that name the value:

```zuri,ignore
raise ValueError('port must be between 1 and 65535, got ${port}')
```

The person reading that message is trying to work out what went wrong from
one line of a log file. Give them the number.

## Catching Inside a Loop

A `catch` inside a loop body handles one iteration and lets the rest carry
on. This is the shape for processing a batch where individual items are
allowed to fail:

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

Put the `catch` **outside** the loop instead, and the first failure ends
the whole loop — which is the right choice when one bad item makes the
rest meaningless, and the wrong one when it does not. The placement of the
block is the decision; there is no flag to set.

## Nesting and Re-raising

A `catch` inside a handler works like any other, which is how you translate
a low-level failure into one your caller understands:

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

The caller now gets an error in its own vocabulary. Include the original
message, as above, so the detail is not lost on the way up.

Re-raising the **same** error, rather than a new one, keeps the original
stack trace pointing at the original line:

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
  only_handle_missing(@() { raise ValueError('not mine') })
} as e {
  echo '${e.type}: ${e.message}'
}
```

```console
fine
ValueError: not mine
```

That is the pattern for "handle one kind and let everything else through",
and it is worth reaching for whenever a handler would otherwise swallow a
bug along with the failure it meant to catch.

## A Worked Example

Here is the whole chapter in one function: a loader that validates its
input, distinguishes the failures a caller can act on from the ones it
cannot, and closes what it opened on every path.

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

Four things in there are worth naming.

Every failure the caller might reasonably handle arrives as one error type,
`StoreError`, so `catch` on the calling side needs one branch rather than
three.

The message always names the path. A log line saying "file is corrupt"
with no filename costs someone an hour.

`handle.close()` appears on both paths — once in the handler before the
re-raise, once after the block. There is no `finally` to do it for you, and
forgetting the one in the handler is the most common resource leak in Zuri
code.

And the shape check at the end is a `raise`, not an `assert`. A file on
disk containing the wrong thing is the world being uncooperative, not a bug
in this function.
