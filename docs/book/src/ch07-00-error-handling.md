# Error Handling

Zuri has one error mechanism: an `Error` object, raised with `raise` and
intercepted with `catch`. There is no `try`, no `finally`, and no checked
exceptions.

## Raising

```zuri
def withdraw(balance, amount) {
  if amount > balance {
    raise ValueError('cannot withdraw ${amount} from ${balance}')
  }
  return balance - amount
}
```

`raise` takes an instance of `Error` or any subclass. An error that nobody
catches ends the program with a message, a source excerpt and a stack
trace:

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

```zuri
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

```zuri
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

```zuri
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

```zuri
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

```zuri
raise ValueError('port must be between 1 and 65535, got ${port}')
```

The person reading that message is trying to work out what went wrong from
one line of a log file. Give them the number.
