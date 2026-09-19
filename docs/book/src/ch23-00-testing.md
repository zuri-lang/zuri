# Testing

A test is a program that runs your program and says whether it did the
right thing. The `test` module gives you the pieces: a way to name and
group tests, a way to state what you expected, and a report that tells
you which expectation failed and where.

Nothing needs installing. `import test` and you have it.

## Your First Test

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
    expect(subtotal([{ price: 250, quantity: 3 }])).to_be(750)
  })

  it('adds every line together', @{
    expect(subtotal([
      { price: 250, quantity: 3 },
      { price: 100, quantity: 1 },
    ])).to_be(850)
  })

})
```

Run it the way you run anything else:

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

Three things are worth noticing straight away.

**`import test { * }`.** A test file wants `describe`, `it`, `expect`
and the rest in scope, not behind a module name. `import test` on its
own works too, and gives you `test.describe`, `test.expect` and so on;
use that from code that is not itself a test file.

**Nothing says "now run".** `describe` and `it` do not run anything;
they build a tree, and the tree is run once the file has finished
declaring it, through `os.at_exit()`. That separation is what lets the
framework count the tests before the first one starts, focus on one of
them, filter by name, and run them in a random order. `run()` exists
for when you want to pass options or read the result, and
[Controlling a Run](#controlling-a-run) comes back to it.

**`@{ ... }` is a function.** `it('name', @{ ... })` hands the
framework a body to call later, which is why it can decide not to.

## When It Fails

Change `850` to `900` and run it again:

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

The message, the two values, the line, and the code around it. The
process exits with status `1`, which is what a CI system reads.

## Grouping

`describe` nests as deeply as the thing you are describing:

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

The nesting shows up in the report and in the name a failure is filed
under: `Cart > add() > merges a duplicate SKU`. Choose names that read
as a sentence when joined like that, because that is how you will read
them.

## Expectations

`expect(value)` gives you an object with every matcher on it.

```zuri,ignore
expect(total).to_be(850)
expect(cart).to_have_length(2)
expect(user).to_match_object({ name: 'Ada' })
expect(items).not.to_be_empty()
```

**`.not` negates any matcher.** There is one implementation behind both
directions, so `to_contain` and `not.to_contain` can never disagree
about what containing means.

**Matchers chain**, because each returns the object it was called on:

```zuri,ignore
expect(port).to_be_int().to_be_between(1024, 65535)
```

**A second argument names the value**, which earns its keep the moment
the number alone would not say which number it was:

```zuri,ignore
expect(response.status, 'status').to_be(200)
```

```console
Expected status (404) to be 200
```

### Choosing Between `to_be` and `to_equal`

`to_be` is Zuri's `==`. That already compares lists, dictionaries and
bytes by their contents, so it is the right matcher for most things:

```zuri,ignore
expect([1, 2]).to_be([1, 2])          # passes
expect({ a: 1 }).to_be({ a: 1 })      # passes
```

It compares two **instances** by identity, though, because that is what
`==` does:

```zuri,ignore
expect(Point(1, 2)).to_be(Point(1, 2))      # fails: two objects
expect(Point(1, 2)).to_equal(Point(1, 2))   # passes: same contents
```

`to_equal` is the structural one. It walks an instance field by field,
treats `NaN` as equal to itself, and survives a value that contains
itself. Reach for it whenever the objects are built fresh on both sides
of the comparison.

### The Matchers

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

Each one's exact behaviour, and what it refuses, is in its doc block.

A few are worth calling out.

**`to_be_close_to`** is how you compare anything that has been through
floating-point arithmetic. `expect(0.1 + 0.2).to_be(0.3)` fails; the
sum is `0.30000000000000004`.

```zuri,ignore
expect(0.1 + 0.2).to_be_close_to(0.3)
```

**`to_raise*`** takes a function, not a value, because the framework has
to be the one to call it:

```zuri,ignore
expect(@{ parse('') }).to_raise_instance_of(ValueError)
expect(@{ parse('{}') }).to_not_raise()
```

**`to_have_property`** walks a dotted path through dictionaries,
instances and list indices alike, which is what a decoded response
usually is all the way down:

```zuri,ignore
expect(payload).to_have_property('data.items.0.id', 7)
```

**`to_satisfy`** is the escape hatch when nothing else fits:

```zuri,ignore
expect(port).to_satisfy(@(n) { return n % 2 == 0 }, 'to be an even port')
```

### Failing on Purpose

```zuri,ignore
using response.status {
  when 200 handle_ok()
  when 404 handle_missing()
  default fail('unexpected status ${response.status}')
}
```

And when the assertions live inside a callback that might never run,
say how many you expect:

```zuri,ignore
it('reports every error', @{
  assertions(2)

  validate(bad_input, @(error) {
    expect(error.field).to_be_string()
  })
})
```

If the callback ran once instead of twice, the test fails with
`Expected 2 assertions but 1 ran` rather than passing on a technicality.
`has_assertions()` is the looser form: at least one.

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

The rules:

- `before_all` runs once, immediately before the first test in its suite
  that is actually going to run. A suite everything was filtered out of
  never connects to anything.
- `after_all` runs after the last one, and only if `before_all` ran.
- `before_each` runs outermost suite first, `after_each` innermost
  first, so teardown undoes setup in the order it was done.
- `after_each` runs even when the test failed, which is exactly when you
  need it to.

A hook that raises is reported as a hook failure against the test it was
preparing, rather than taking the run down. A `before_all` that raises
fails every test in its suite, because none of them got the setup they
were written against.

## Focusing, Skipping and Todo

While you are working on one thing:

```zuri,ignore
it_only('the one I am fixing', @{ ... })
describe_only('the area I am in', @{ ... })
```

As soon as anything is marked `only`, everything else is skipped and
still listed, so you cannot forget it is on.

```zuri,ignore
it_skip('known broken, see #412', @{ ... })
describe_skip('the old API', @{ ... })

it_todo('handle an empty payload')
```

A todo is a test with no body. `it('name')` with nothing after it means
the same thing. It counts towards nothing and shows up in the report as
a reminder that survives being committed.

Two options change what a failure means:

```zuri,ignore
it('reproduces issue 412', @{ ... }, { failing: true })
it('talks to a flaky endpoint', @{ ... }, { retries: 2 })
```

`failing: true` passes when the body fails, and fails when the body
passes, so the day someone fixes the bug the test tells you. `retries`
runs the body again on failure, and a test that passes on a later
attempt is reported as flaky rather than quietly green.

## One Test, Many Inputs

```zuri
import test { * }

def slug(title) {
  return title.lower().replace('/[^a-z0-9]+/', '-').trim('-')
}

it_each([
  ['Hello World', 'hello-world'],
  ['  Spaced  Out  ', 'spaced-out'],
  ['Zuri 1.0!', 'zuri-1-0'],
], 'turns $0 into $1', @(title, expected) {
  expect(slug(title)).to_be(expected)
})
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

Each row becomes a separate test with its own name and its own place in
the report, so one bad row does not hide the others. `$0`, `$1` and so
on stand for the row's values and `$#` for the row number.
`describe_each` does the same for whole suites, which is how you run one
set of tests against several implementations of the same interface.

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

    expect(@{ retry(operation.fn, 3) }).to_raise_with_message('connection reset')
    expect(operation).to_have_been_called_times(3)
  })

})
```

`mock()` returns a `Mock`, and `mock.fn` is the plain function you hand
to the code under test. The matchers accept either, so
`expect(operation)` and `expect(operation.fn)` mean the same thing.

The behaviour is scripted in front-to-back order: everything queued with
a `_once` suffix runs first, one call each, and then the standing
behaviour takes over.

| Method | Effect |
| --- | --- |
| `returns(v)` / `returns_once(v)` | return `v` |
| `raises(e)` / `raises_once(e)` | raise `e` |
| `implements(fn)` / `implements_once(fn)` | run `fn` with the real arguments |
| `reset()` | forget the calls |
| `clear_behaviour()` | forget the behaviour |

### Spying on Something That Already Exists

`spy_on` swaps a function out in place and gives you a `Mock` that both
records and stands in for it:

```zuri
import test { * }

def checkout(cart, gateway) {
  var total = cart.reduce(@(sum, line) { return sum + line }, 0)
  return gateway['charge'](total)
}

it('charges the cart total once', @{
  var gateway = { charge: @(amount) { return 'live-charge' } }
  var charge = spy_on(gateway, 'charge')
  charge.returns('receipt-1')

  expect(checkout([250, 100], gateway)).to_be('receipt-1')
  expect(charge).to_have_been_called_times(1)
  expect(charge).to_have_been_called_with(350)
})
```

A spy calls through to the original unless you tell it otherwise, and is
restored automatically after the test that installed it, even one that
failed halfway through. Nothing has to be undone by hand.

It works on a dictionary entry and on an instance property that holds a
function. It does not work on a class method or a module function: Zuri
classes are immutable once declared, and a module's members cannot be
assigned from outside it. Code you want to substitute takes its
collaborators as arguments or holds them in properties, which is the
shape worth designing for anyway.

## Snapshots

For a value too big to write out by hand, record it once and compare
against the recording from then on:

```zuri,ignore
def invoice(customer, lines) {
  return {
    customer,
    lines,
    total: lines.reduce(@(sum, line) { return sum + line.amount }, 0),
  }
}

it('builds the document', @{
  expect(invoice('Ada', [{ label: 'Design', amount: 4200 }])).to_match_snapshot()
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

It lands beside the test file, in `__snapshots__`:

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

Commit it. Reviewing the change to that file in a pull request is the
entire value of the technique: an unexplained diff there is exactly the
thing worth noticing.

When the value changes, you get the diff:

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

If the new value is right, rewrite the snapshots:

```console
$ ZURI_UPDATE_SNAPSHOTS=1 zuri run invoice.zu
```

That also deletes entries nothing asks for any more. And on CI, where
`CI` is set in the environment, writing a *brand new* snapshot is a
failure rather than a silent pass, because a snapshot nobody has looked
at asserts nothing.

## Testing What Something Prints

```zuri,ignore
expect(@{ greet('Ada') }).to_print('Hello, Ada')
expect(@{ quiet_mode() }).to_print_nothing()
```

Or take the output and assert on it yourself:

```zuri,ignore
var printed = capture_output(@{ report(rows) })
expect(printed.lines()).to_have_length(4)
```

This works because the runtime can redirect everything Zuri writes to
standard output. The same mechanism is why a passing test's output does
not clutter the report: it is captured and shown only when the test
fails. `run({ verbose: true })` shows it either way, and
`run({ capture: false })` turns it off entirely.

## More Than One File

A real project has a directory of them:

```text
project/
  tests/
    cart.zu
    pricing.zu
```

`zuri test` runs the lot:

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

There is nothing to write for this. The command finds the `tests`
directory you ran it from, and ends the process with `1` when anything
failed, so a CI job needs nothing added to it either.

Each file runs in a process of its own. That is not an implementation
detail you can ignore, because it is what you are buying:

- A file that loops forever is killed, and the rest still run.
  `--timeout 30s` says how long is too long.
- A file that crashes, or calls `os.exit()` halfway through, is reported
  as a file that never reported rather than taking the run with it.
- Global state, a module loaded for its side effect, a changed working
  directory: none of it leaks from one file into the next.

Files are reported in the order they were discovered whatever order they
finish in, so `--jobs 4` makes a big suite faster without making the
report move around.

A test file needs nothing special to be conducted. It declares its
tests, and is equally runnable on its own.

### Running One File

Name it, with or without its `.zu`:

```console
$ zuri test pricing
$ zuri test pricing.zu
$ zuri test tests/pricing.zu
```

All three run `tests/pricing.zu`. A name is looked for under `tests`
first, so a test keeps its own name even when something else in the
project shares it, and a name that is nowhere to be found under that
directory is looked for once more by filename alone anywhere beneath it,
so a file in a subdirectory answers to its own name.

A directory works too, wherever it sits:

```console
$ zuri test tests/api
$ zuri test packages/store/tests
```

### The Flags

| Flag | What it does |
| --- | --- |
| `-j, --jobs <count>` | How many files to run at once. `auto` is one per CPU. Default `1`. |
| `-t, --timeout <duration>` | How long one file may run before it is killed. `500ms`, `30s`, `2m`, `1h`, or a bare number of milliseconds. Default: no limit. |
| `-b, --bail [count]` | Stop after this many failing files. On its own, stop at the first. |
| `-m, --match <pattern...>` | Filename patterns to run, in place of `*.zu`. |
| `-i, --ignore <pattern...>` | Filename patterns to skip, in place of `_*`, `.*` and `index.zu`. |
| `--no-recursive` | Only the files directly in the directory. |
| `-e, --env <assignment...>` | Extra environment for every test process, as `KEY=VALUE`. |
| `-l, --list` | Print the files that would run, one to a line, and stop. |

```console
$ zuri test --jobs auto --timeout 30s
$ zuri test --bail
$ zuri test --match '*_test.zu' '*_spec.zu'
```

`--bail` and the two pattern flags take as many words as follow them, so
put the file you are naming ahead of them, or close them with `--`:

```console
$ zuri test pricing --bail
$ zuri test --match '*_test.zu' -- pricing
```

A count for `--bail` has to be attached, `--bail=3`, for the same
reason: a count written as a separate word is indistinguishable from the
file.

### Conducting a Suite Yourself

`conduct()` is the function `zuri test` is built on, and a project that
wants the run under its own control can call it directly. A directory
handed to `zuri run` runs its `index.zu`, so one file makes the suite
`zuri run tests`:

<span class="filename">Filename: tests/index.zu</span>

```zuri,ignore
import os
import test

test.conduct(os.dir_name(__file__), { jobs: 4, timeout: 30000 })
```

It takes the same choices the flags do, as a dictionary, and returns the
run rather than only reporting it. `conduct` leaves `index.zu` out of
discovery, and never runs the script that called it either, so the index
cannot end up running itself.

## Controlling a Run

Call `run()` yourself when you want to change how the run behaves, or
to read the result. It takes an options dictionary, and calling it
takes over from the automatic run, so nothing happens twice.

The options you will reach for:

| Option | What it does |
| --- | --- |
| `filter` | run only tests whose full name contains this, or matches it as a regular expression |
| `bail` | stop after this many failures |
| `shuffle` / `seed` | run in a random order, and reproduce that order later |
| `reporter` | `spec`, `dot`, `tap`, `junit`, `json`, `ndjson`, `silent` |
| `exit` | exit the process with the run's status when it finishes |

```zuri,ignore
run({ filter: 'subtotal', bail: 1 })
```

`filter` and `reporter` can also come from `ZURI_TEST_FILTER` and
`ZURI_TEST_REPORTER`, which is what lets one command be pointed at one
test without editing the file.

Shuffling is the one worth turning on deliberately. Tests that only pass
because an earlier test left something behind are a real and common
problem, and running them in a different order every time is how you
find out:

```zuri,ignore
run({ shuffle: true })
```

The seed is printed with the summary, and passing it back reproduces
that exact order.

### On CI

Nothing to add. A test file exits `0` when everything passed and `1`
when it did not, whether or not it called `run()`.

For a CI system that wants a machine-readable report,
`{ reporter: 'junit' }` writes JUnit XML to standard output and
`{ reporter: 'tap' }` writes TAP version 14.

## Writing Your Own Reporter

The runner knows nothing about output. It walks the tree and calls
methods on a reporter, and every method has a do-nothing default:

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
  it('passes quietly', @{ expect(1).to_be(1) })
  it('fails loudly', @{ expect(1).to_be(2) })
})

run({ reporter: Quiet(), exit: false })
```

The objects handed to it are the same ones the built-in reporters see:
`Case`, `Suite`, `Failure` and `Summary`, documented in
`test.result`.

## Two Things to Know

**A `timeout` on a test is measured after the body returns.** Zuri runs
synchronously, so a test runs to completion and is then timed. That
catches one that got too slow; it does not rescue one that hangs. A hang
needs a process boundary, which is exactly what `conduct` puts around
each file, and why its `timeout` is the enforcing one.

**Everything in one file shares one interpreter.** Reset what you change
in `before_each`, and reach for `shuffle` to find out whether you missed
anything.

## Where to Go Next

The module's own documentation carries the full matcher list with each
one's edge cases, every option `run()` and `conduct()` accept, and the
snapshot file format. [Appendix F](appendix-06-stdlib-index.md) lists
the submodules. [Chapter 24](ch24-00-debugging.md) is what to do once a
test has told you something is wrong.
