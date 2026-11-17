# Testing

A test is a program that runs your program and tells you whether it did the
right thing. The `test` module provides everything you need for this: a way
to name and group tests, a way to state what you expect, and a report that
tells you which expectation failed and where.

There's nothing to install. Just write `import test` and you can start.

## A First Test

```zuri
import test { * }

def subtotal(items) {
  return items.reduce(@(total, item) {
    return total + item.price * item.quantity
  }, 0)
}

describe('subtotal', @{

  it('is zero for an empty cart', @{
    expect(subtotal([])).to_be(0)
  })

  it('multiplies price by quantity', @{
    expect(
      subtotal([
        { price: 250, quantity: 3 },
      ]),
    ).to_be(750)
  })

  it('adds every line together', @{
    expect(
      subtotal([
        { price: 250, quantity: 3 },
        { price: 100, quantity: 1 },
      ]),
    ).to_be(850)
  })
})
```

Run it the same way you run anything else:

```console
$ zuri run cart.zu

  subtotal
    ✓ is zero for an empty cart
    ✓ multiplies price by quantity
    ✓ adds every line together

   PASS

  3 passed  •  3 total
  suites 1   time 1ms
```

There are three things to notice right away.

**`import test { * }`.** A test file needs `describe`, `it`, `expect` and
the rest available directly, not behind a module name. `import test` on its
own also works, and gives you `test.describe`, `test.expect` and so on. Use
that form in code that isn't itself a test file.

**Nothing tells the tests to run.** `describe` and `it` don't run anything.
They build a tree, which runs once the file has finished declaring it,
through `os.at_exit()`. That separation is what lets the framework count the
tests before the first one starts, focus on a single test, filter tests by
name and run them in random order. `run()` exists for when you want to pass
options or read the result, which [Controlling a Run](#controlling-a-run)
covers.

**`@{ ... }` is a function.** `it('name', @{ ... })` gives the framework a
body to call later, which is why the framework can also choose not to call
it.

## When a Test Fails

Change `850` to `900` and run the test again:

```console
  subtotal
    ✓ is zero for an empty cart
    ✓ multiplies price by quantity
    ✗ adds every line together

  Failures

  1) subtotal › adds every line together

     Expected 850 to be 900

     + expected  900
     - received  850

     at cart.zu:23 in @anon3

       21 |       { price: 250, quantity: 3 },
       22 |       { price: 100, quantity: 1 },
     > 23 |     ])).to_be(900)
       24 |   })
       25 |

   FAIL

  1 failed  •  2 passed  •  3 total
  suites 1   time 4ms
```

You get the message, the two values, the line number and the code around it.
The process exits with status `1`, which is what a CI system checks.

## Grouping Tests

You can nest `describe` blocks as deeply as the thing you're describing
needs:

```zuri,ignore
describe('Cart', @{

  describe('subtotal()', @{
    it('is zero when empty', @{ ... })
  })

  describe('add()', @{
    it('appends a line', @{ ... })
    it('merges a duplicate SKU', @{ ... })
  })

})
```

The nesting appears in the report and in the name that a failure is reported
under, such as `Cart > add() > merges a duplicate SKU`. Choose names that
read as a sentence when they're joined together like that, because that's
how you'll read them.

## Expectations

`expect(value)` returns an object that has every matcher on it.

```zuri,ignore
expect(total).to_be(850)
expect(cart).to_have_length(2)
expect(user).to_match_object({ name: 'Ada' })
expect(items).not.to_be_empty()
```

**`.not` negates any matcher.** The same implementation handles both
directions, so `to_contain` and `not.to_contain` can never disagree about
what "contains" means.

**Matchers can be chained**, because each one returns the object it was
called on:

```zuri,ignore
expect(port).to_be_int().to_be_between(1024, 65535)
```

**A second argument gives the value a name**, which is useful as soon as the
number on its own wouldn't tell you which number it was:

```zuri,ignore
expect(response.status, 'status').to_be(200)
```

```console
Expected status (404) to be 200
```

### Choosing Between `to_be` and `to_equal`

`to_be` uses Zuri's `==` operator. That already compares lists, dictionaries
and bytes by their contents, so it's the right matcher for most values:

```zuri,ignore
expect([1, 2]).to_be([1, 2]) # passes
expect({ a: 1 }).to_be({ a: 1 }) # passes
```

However, it compares two **instances** by identity, unless their class
defines `@eq`, because that's what `==` does:

```zuri,ignore
expect(Point(1, 2)).to_be(Point(1, 2)) # fails without @eq: two objects
expect(Point(1, 2)).to_equal(Point(1, 2)) # passes: same contents
```

`to_equal` compares structures. It goes through an instance field by field,
treats `NaN` as equal to itself, and can handle a value that contains
itself. Use it whenever the objects on both sides of the comparison are
created separately.

### Available Matchers

| Group | Matchers |
| --- | --- |
| Equality | `to_be`, `to_equal`, `to_be_same_as`, `to_be_close_to`, `to_be_within`, `to_match_object` |
| Truthiness | `to_be_true`, `to_be_false`, `to_be_truthy`, `to_be_falsy`, `to_be_nil`, `to_be_defined` |
| Types | `to_be_a`, `to_be_string`, `to_be_number`, `to_be_int`, `to_be_float`, `to_be_bigint`, `to_be_bool`, `to_be_list`, `to_be_dict`, `to_be_bytes`, `to_be_function`, `to_be_callable`, `to_be_iterable`, `to_be_class`, `to_be_instance`, `to_be_instance_of` |
| Numbers | `to_be_greater_than`, `to_be_greater_than_or_equal`, `to_be_less_than`, `to_be_less_than_or_equal`, `to_be_between`, `to_be_positive`, `to_be_negative`, `to_be_zero`, `to_be_divisible_by`, `to_be_even`, `to_be_odd`, `to_be_nan`, `to_be_finite`, `to_be_infinite` |
| Text | `to_contain`, `to_contain_ignoring_case`, `to_equal_ignoring_case`, `to_start_with`, `to_end_with`, `to_match`, `to_be_blank` |
| Collections | `to_have_length`, `to_be_empty`, `to_contain_equal`, `to_contain_all`, `to_contain_any`, `to_contain_none`, `to_contain_exactly`, `to_have_key`, `to_have_keys`, `to_have_value`, `to_have_property`, `to_be_sorted`, `to_have_unique_items` |
| Errors | `to_raise`, `to_raise_instance_of`, `to_raise_with_message`, `to_not_raise` |
| Mocks | `to_have_been_called`, `to_have_been_called_times`, `to_have_been_called_with`, `to_have_been_last_called_with`, `to_have_been_nth_called_with`, `to_have_returned`, `to_have_returned_with`, `to_have_raised` |
| Output | `to_print`, `to_print_exactly`, `to_print_nothing` |
| Snapshots | `to_match_snapshot` |
| Anything else | `to_satisfy` |

Each matcher's doc comment describes exactly how it behaves and what it
rejects.

A few of them are worth pointing out.

**`to_be_close_to`** is how you compare anything that has gone through
floating-point arithmetic. `expect(0.1 + 0.2).to_be(0.3)` fails, because the
sum is `0.30000000000000004`.

```zuri,ignore
expect(0.1 + 0.2).to_be_close_to(0.3)
```

**`to_raise`** and the matchers like it take a function rather than a value,
because the framework has to be the one that calls it:

```zuri,ignore
expect(@{
  parse('')
}).to_raise_instance_of(ValueError)
expect(@{
  parse('{}')
}).to_not_raise()
```

**`to_have_property`** follows a dotted path through dictionaries, instances
and list indexes alike, which is what a decoded response usually consists
of:

```zuri,ignore
expect(payload).to_have_property('data.items.0.id', 7)
```

**`to_satisfy`** is for when nothing else fits:

```zuri,ignore
expect(port).to_satisfy(@(n) {
  return n % 2 == 0
}, 'to be an even port')
```

### Failing a Test Explicitly

```zuri,ignore
using response.status {
  when 200 handle_ok()
  when 404 handle_missing()
  default fail('unexpected status ${response.status}')
}
```

When the assertions are inside a callback that might never run, state how
many assertions you expect:

```zuri,ignore
it('reports every error', @{
  assertions(2)

  validate(bad_input, @(error) {
    expect(error.field).to_be_string()
  })
})
```

If the callback ran once instead of twice, the test fails with
`Expected 2 assertions but 1 ran`, instead of passing on a technicality.
`has_assertions()` is a less strict version that requires at least one
assertion.

## Setup and Teardown

```zuri
import test { * }

describe('Session', @{

  var store = nil

  before_all(@{
    echo 'connecting'
  })

  after_all(@{
    echo 'disconnecting'
  })

  before_each(@{
    store = { rows: [] }
  })

  it('starts empty', @{
    expect(store.rows).to_be_empty()
  })

  it('is a fresh store every time', @{
    store.rows.append('a')
    expect(store.rows).to_have_length(1)
  })
})
```

```console
$ zuri run session.zu
connecting
  Session
    ✓ starts empty
    ✓ is a fresh store every time
disconnecting

   PASS

  2 passed  •  2 total
  suites 1   time 2ms
```

These are the rules:

- `before_all` runs once, just before the first test in its suite that's
  actually going to run. If every test in a suite was filtered out,
  `before_all` never runs, so nothing connects to anything.
- `after_all` runs after the last test, and only if `before_all` ran. It
  also runs if `before_all` raised an error, because a setup that failed
  partway through might already have opened a connection or started a
  server. Write it so that it can cope with however far the setup got.
- `before_each` hooks run starting with the outermost suite, and
  `after_each` hooks run starting with the innermost, so teardown undoes
  setup in the reverse order.
- `after_each` runs even when the test failed, which is exactly when you
  need it.

A hook that raises an error is reported as a hook failure for the test it
was preparing, instead of stopping the whole run. If a `before_all` raises
an error, every test in its suite fails, because none of them got the setup
they depend on.

## Focusing, Skipping and To-Do Tests

When you're working on one particular thing:

```zuri,ignore
it_only('the one I am fixing', @{ ... })
describe_only('the area I am in', @{ ... })
```

As soon as anything is marked with `only`, everything else is skipped, but
still listed, so you can't forget that it's turned on.

```zuri,ignore
it_skip('known broken, see #412', @{ ... })
describe_skip('the old API', @{ ... })

it_todo('handle an empty payload')
```

A to-do is a test without a body, and `it('name')` with nothing after it
means the same thing. It doesn't count towards anything, and appears in the
report as a reminder that stays there after you commit it.

Two options change what a failure means:

```zuri,ignore
it('reproduces issue 412', @{ ... }, { failing: true })
it('talks to a flaky endpoint', @{ ... }, { retries: 2 })
```

`failing: true` makes the test pass when the body fails and fail when the
body passes, so on the day someone fixes the bug, the test tells you.
`retries` runs the body again if it fails, and a test that passes on a later
attempt is reported as flaky instead of being quietly counted as a pass.

## Running a Test With Several Inputs

```zuri
import test { * }

def slug(title) {
  return title.lower().replace('/[^a-z0-9]+/', '-').trim('-')
}

it_each(
  [
    ['Hello World', 'hello-world'],
    ['  Spaced  Out  ', 'spaced-out'],
    ['Zuri 1.0!', 'zuri-1-0'],
  ],
  'turns $0 into $1',
  @(title, expected) {
    expect(slug(title)).to_be(expected)
  },
)
```

```console
$ zuri run slug.zu
  ✓ turns 'Hello World' into 'hello-world'
  ✓ turns '  Spaced  Out  ' into 'spaced-out'
  ✓ turns 'Zuri 1.0!' into 'zuri-1-0'

   PASS

  3 passed  •  3 total
  suites 0   time 3ms
```

Each row becomes a separate test, with its own name and its own entry in the
report, so one bad row doesn't hide the others. `$0`, `$1` and so on stand
for the row's values, and `$#` for the row number. `describe_each` does the
same for whole suites, which is how you run one set of tests against several
implementations of the same interface.

## Test Doubles

`mock()` gives you a function that records how it was called and does
whatever you tell it to.

```zuri
import test { * }

def retry(operation, attempts) {
  iter var attempt = 1; attempt <= attempts; attempt++ {
    catch {
      return operation()
    } as error {
      if attempt == attempts {
        raise error
      }
    }
  }
}

describe('retry', @{

  it('returns the first success', @{
    var operation = mock()
    operation.returns('ok')

    expect(retry(operation.fn, 3)).to_be('ok')
    expect(operation).to_have_been_called_times(1)
  })

  it('tries again after a failure', @{
    var operation = mock()
    operation.raises_once(Error('connection reset'))
    operation.returns('ok')

    expect(retry(operation.fn, 3)).to_be('ok')
    expect(operation).to_have_been_called_times(2)
  })

  it('gives up eventually', @{
    var operation = mock()
    operation.raises(Error('connection reset'))

    expect(@{
      retry(operation.fn, 3)
    }).to_raise_with_message('connection reset')
    expect(operation).to_have_been_called_times(3)
  })
})
```

`mock()` returns a `Mock`, and `mock.fn` is the plain function that you pass
to the code under test. The matchers accept either one, so
`expect(operation)` and `expect(operation.fn)` mean the same thing.

Behavior is used in order. Everything you queue with a `_once` method runs
first, one call each, and after that the standing behavior takes over.

| Method | Effect |
| --- | --- |
| `returns(v)` / `returns_once(v)` | Returns `v` |
| `raises(e)` / `raises_once(e)` | Raises `e` |
| `implements(fn)` / `implements_once(fn)` | Runs `fn` with the actual arguments |
| `reset()` | Forgets the recorded calls |
| `clear_behaviour()` | Forgets the configured behavior |

### Spying on an Existing Function

`spy_on` replaces a function in place and gives you a `Mock` that both
records calls and stands in for the original:

```zuri
import test { * }

def checkout(cart, gateway) {
  var total = cart.reduce(@(sum, line) {
    return sum + line
  }, 0)
  return gateway['charge'](total)
}

it('charges the cart total once', @{
  var gateway = {
    charge: @(amount) {
      return 'live-charge'
    },
  }
  var charge = spy_on(gateway, 'charge')
  charge.returns('receipt-1')

  expect(checkout([250, 100], gateway)).to_be('receipt-1')
  expect(charge).to_have_been_called_times(1)
  expect(charge).to_have_been_called_with(350)
})
```

A spy calls the original function unless you tell it otherwise, and it's
restored automatically after the test that installed it, even if the test
failed partway through. You don't need to undo anything yourself.

It works on a dictionary entry, and on an instance property that holds a
function. It doesn't work on a class method or a module function, because
Zuri classes can't be changed after they're declared, and a module's members
can't be assigned from outside the module. If you want to be able to
substitute something in tests, have the code take its collaborators as
arguments or store them in properties, which is a good design anyway.

## Snapshots

For a value that's too large to write out by hand, record it once and
compare against the recording from then on:

```zuri,ignore
def invoice(customer, lines) {
  return {
    customer,
    lines,
    total: lines.reduce(@(sum, line) {
      return sum + line.amount
    }, 0),
  }
}

it('builds the document', @{
  expect(
    invoice('Ada', [
      { label: 'Design', amount: 4200 },
    ]),
  ).to_match_snapshot()
})
```

The first run writes the file and passes:

```console
$ zuri run invoice.zu

  invoice
    ✓ builds the document

   PASS

  1 passed  •  1 total
  suites 1   time 2ms
  snapshots 1 written
```

The file is saved next to the test file, in `__snapshots__`:

```text
# Zuri snapshot file v1

=== invoice > builds the document 1 ===
  {
    customer: 'Ada',
    lines: [
      {
        amount: 4200,
        label: 'Design'
      }
    ],
    total: 4200
  }
```

Commit it. Reviewing changes to that file in a pull request is the whole
point of snapshot testing, because an unexplained change there is exactly
what you want to notice.

When the value changes, you see the difference:

```console
  1) invoice › builds the document

     Snapshot 'invoice > builds the document 1' no longer matches

       + expected  - received

       {
         customer: 'Ada',
         lines: [
           {
     -       amount: 5200,
     +       amount: 4200,
             label: 'Design'
           }
         ],
     -   total: 5200
     +   total: 4200
       }
```

If the new value is correct, update the snapshots:

```console
$ ZURI_UPDATE_SNAPSHOTS=1 zuri run invoice.zu
```

This also removes entries that are no longer used. In CI, where the `CI`
environment variable is set, writing a *new* snapshot counts as a failure
instead of quietly passing, because a snapshot that nobody has looked at
doesn't test anything.

## Testing What Something Prints

```zuri,ignore
expect(@{
  greet('Ada')
}).to_print('Hello, Ada')
expect(@{
  quiet_mode()
}).to_print_nothing()
```

You can also capture the output and make your own assertions about it:

```zuri,ignore
var printed = capture_output(@{
  report(rows)
})
expect(printed.lines()).to_have_length(4)
```

This works because the runtime can redirect everything Zuri writes to
standard output. The same mechanism keeps output from passing tests out of
the report. It's captured and only shown when a test fails.
`run({ verbose: true })` shows it either way, and `run({ capture: false })`
turns capturing off completely.

## Running Several Files

A real project has a whole directory of test files:

```text
project/
  tests/
    cart.zu
    pricing.zu
```

`zuri test` runs all of them:

```console
$ zuri test

  zuri test  2 files in tests

   PASS   cart.zu  188ms  2 tests

   FAIL   pricing.zu  281ms  1 test
        ✗ pricing > applies the discount
          Expected 90 to be 100
          at tests/pricing.zu:4

  2 files  •  1 with failures
  1 failed  •  2 passed  •  3 total
  time 476ms
```

You don't need to write anything for this. The command finds the `tests`
directory where you ran it, and exits with status `1` if anything failed, so
a CI job doesn't need any extra setup either.

Each file runs in a separate process. That's more than an implementation
detail, because it gives you several guarantees:

- A file that loops forever is stopped, and the other files still run. Use
  `--timeout 30s` to set how long is too long.
- A file that crashes, or calls `os.exit()` partway through, is reported as
  a file that didn't report back, instead of bringing down the whole run.
- Global state, modules loaded for their side effects and changes to the
  working directory never leak from one file to the next.

Files are reported in the order they were found, whatever order they finish
in, so `--jobs 4` makes a large suite faster without the report jumping
around.

A test file doesn't need anything special to run this way. It declares its
tests, and you can still run it on its own.

### Running a Single File

Give its name, with or without the `.zu` extension:

```console
$ zuri test pricing
$ zuri test pricing.zu
$ zuri test tests/pricing.zu
```

All three run `tests/pricing.zu`. A name is looked for under `tests` first,
so a test keeps its own name even if something else in the project has the
same name. If the name isn't found directly in that directory, Zuri looks
for a file with that name anywhere under it, so a file in a subdirectory can
still be found by its own name.

A directory works too, wherever it is:

```console
$ zuri test tests/api
$ zuri test packages/store/tests
```

### Command-Line Flags

| Flag | What it does |
| --- | --- |
| `-j, --jobs <count>` | The number of files to run at once. `auto` runs one per CPU. The default is `1`. |
| `-t, --timeout <duration>` | How long a single file can run before it's stopped, such as `500ms`, `30s`, `2m` or `1h`, or a plain number of milliseconds. By default, there's no limit. |
| `-b, --bail [count]` | Stops after this many files have failed. Without a count, stops at the first failure. |
| `-m, --match <pattern...>` | File name patterns to run, instead of `*.zu` |
| `-i, --ignore <pattern...>` | File name patterns to skip, instead of `_*`, `.*` and `index.zu` |
| `--no-recursive` | Only runs the files directly inside the directory |
| `-e, --env <assignment...>` | Sets extra environment variables for every test process, written as `KEY=VALUE` |
| `-l, --list` | Prints the files that would run, one per line, and stops |

```console
$ zuri test --jobs auto --timeout 30s
$ zuri test --bail
$ zuri test --match '*_test.zu' '*_spec.zu'
```

`--bail` and the two pattern flags take every word that follows them, so put
the file name before them, or end the list with `--`:

```console
$ zuri test pricing --bail
$ zuri test --match '*_test.zu' -- pricing
```

For the same reason, a count for `--bail` has to be attached, as in
`--bail=3`, because a count written as a separate word can't be told apart
from a file name.

### Running a Suite Yourself

`conduct()` is the function that `zuri test` is built on, and a project that
wants to control the run itself can call it directly. When you pass a
directory to `zuri run`, it runs that directory's `index.zu`, so a single
file lets you run the suite with `zuri run tests`:

<span class="filename">Filename: tests/index.zu</span>

```zuri,ignore
import os
import test

test.conduct(os.dir_name(__file__), { jobs: 4, timeout: 30000 })
```

It accepts the same settings as the command-line flags, as a dictionary, and
returns the run's results instead of only reporting them. `conduct` leaves
`index.zu` out when it looks for test files, and never runs the script that
called it, so the index can't end up running itself.

## Controlling a Run

Call `run()` yourself when you want to change how the run behaves, or to
read the results. It takes a dictionary of options, and calling it replaces
the automatic run, so nothing runs twice.

These are the options you're most likely to use:

| Option | What it does |
| --- | --- |
| `filter` | Only runs tests whose full name contains this text, or matches it as a regular expression |
| `bail` | Stops after this many failures |
| `shuffle` / `seed` | Runs the tests in random order, and lets you reproduce that order later |
| `reporter` | One of `spec`, `dot`, `tap`, `junit`, `json`, `ndjson` or `silent` |
| `exit` | Exits the process with the run's status when it finishes |

```zuri,ignore
run({ filter: 'subtotal', bail: 1 })
```

You can also set `filter` and `reporter` with `ZURI_TEST_FILTER` and
`ZURI_TEST_REPORTER`, which lets you point the same command at a single test
without editing the file.

Shuffling is worth turning on deliberately. Tests that only pass because an
earlier test left something behind are a real and common problem, and
running them in a different order every time is how you find them:

```zuri,ignore
run({ shuffle: true })
```

The seed is printed along with the summary, and passing it back reproduces
exactly the same order.

### Running Tests in CI

You don't need to add anything. A test file exits with `0` when everything
passed and `1` when something didn't, whether or not it called `run()`.

If your CI system needs a machine-readable report, `{ reporter: 'junit' }`
writes JUnit XML to standard output, and `{ reporter: 'tap' }` writes TAP
version 14.

## Writing Your Own Reporter

The runner doesn't know anything about output. It walks the tree of tests
and calls methods on a reporter, and every method has a default that does
nothing:

```zuri
import test { * }
import test.reporter { Reporter }

class Quiet < Reporter {
  test_finished(one) {
    if one.status == 'failed' {
      echo one.full_name()
    }
  }
}

describe('a suite', @{
  it('passes quietly', @{
    expect(1).to_be(1)
  })
  it('fails loudly', @{
    expect(1).to_be(2)
  })
})

run({ reporter: Quiet(), exit: false })
```

The objects passed to the reporter are the same ones the built-in reporters
receive: `Case`, `Suite`, `Failure` and `Summary`, which are documented in
`test.result`.

## Two Things to Keep in Mind

**A test's `timeout` is checked after the body returns.** Zuri runs
synchronously, so a test runs to completion and then its time is checked.
That catches a test that has become too slow, but it can't stop one that
hangs. Handling a hang needs a process boundary, which is exactly what
`conduct` puts around each file, and why its `timeout` is the one that's
enforced.

**All the tests in a file share the same interpreter.** Reset whatever you
change in `before_each`, and use `shuffle` to find out whether you've missed
anything.

## Further Reading

The module's own documentation includes the full list of matchers, with the
edge cases of each one, every option that `run()` and `conduct()` accept,
and the format of snapshot files. [Appendix F](appendix-06-stdlib-index.md)
lists the submodules, and [Chapter 24](ch24-00-debugging.md) explains what
to do once a test has told you something is wrong.
