# Performance and the JIT

Zuri runs your bytecode in an interpreter until a piece of it gets hot,
then compiles that piece to machine code and runs it there instead. Two
compilers share the work, one quick and one thorough, and a hot function
passes through both. The tiering happens on its own, on background
threads, with no annotations and no flags.

Here is what that is worth on a plain recursive Fibonacci, measured on one
idle machine:

```zuri
def fib(n) {
  if n < 2 {
    return n
  }
  return fib(n - 1) + fib(n - 2)
}

var start = time()
echo fib(27)
echo 'took ${((time() - start) * 1000).round()}ms'
```

```console
$ zuri run fib.zu
196418
took 14ms

$ ZURI_JIT=0 zuri run fib.zu
196418
took 47ms
```

Over three times, for a function that does nothing but add and compare, and
you write nothing to get it. The rest of this chapter explains how the
compilers work and covers the handful of cases where what you write decides
whether you get it.

## The Two Compilers

**Kebbi** compiles first. It turns a function's bytecode into machine code
one instruction at a time, keeps values in machine registers, and proves
what it can about types before it compiles: a parameter annotated `list` is
a list, and a counter that starts at 0 and steps by 1 is a whole number.
Kebbi compiles quickly, and its code runs several times faster than the
interpreter.

**Bayelsa** compiles second. It builds the whole function as a graph of
typed values, then optimizes that graph before it generates any machine
code. It carries a fact proven once to every later use, moves checks that
cannot change out of loops, keeps loop counters as plain integers, builds
small functions into the functions that call them, and bets on what a
function has done so far wherever nothing proves it. Bayelsa takes longer
to compile, and its code is the fastest Zuri produces.

Both compile with Cranelift, and both run on background threads. A program
never waits for a compile: it carries on in the interpreter, or in the code
it already has, until the new code is ready.

### How a Function Moves Between Them

1. **The interpreter.** Every function starts here. It counts its calls and
   its loop turns, and records the kind of value each operation meets.
2. **Kebbi, profiling.** Once a function is warm, Kebbi compiles it with
   the counting and recording built in. The function is fast from here on,
   and still watching itself.
3. **Bayelsa.** Once the profiling code has done enough work, Bayelsa
   compiles the function from what it recorded. A loop running at that
   moment moves into the new code at its next turn.

A function every operation of which has already run in the interpreter
skips the profiling step and goes straight to Bayelsa, since profiling
would only record what the interpreter already knows.

### When a Bet Fails

Bayelsa compiles what it has seen. A loop that only ever added whole
numbers does integer arithmetic; a module constant that held `10` is
compared as the integer 10; an index that only ever met `bytes` reads a
byte. Each bet is checked where it is made, and a failed check hands the
frame back to the interpreter at that exact instruction, with every
variable as the compiled code left it. This is a **deoptimisation**.

The place that failed is remembered. The next compile of the function makes
no bet there, so a function deoptimises at a given place once, not every
time round.

### What Stays in Kebbi

Bayelsa leaves a function to Kebbi in two cases:

- The function contains a `catch`. Kebbi compiles it, handlers included.
- The function's hot operations are ones Kebbi handles inline and Bayelsa
  would hand to the runtime: arithmetic on values that were not always
  numbers, method calls on strings, and indexing lists and strings whose
  kind nothing settles. Kebbi's code is the faster of the two there.

A function left in Kebbi is rebuilt without its profiling and runs at
Kebbi's full speed.

## How Tiering Works

Every function counts its own calls, and every loop counts its own
back-edges. When a counter crosses that function's threshold, a compile job
goes to a background thread; when the machine code comes back, later calls
use it.

The threshold is not a fixed number. It scales with the function's size:

```text
threshold = K / sqrt(instruction_count)
```

A large function gets a low threshold, because it is doing more work per
call and the compilation pays for itself sooner. A three-instruction getter
gets a high one, because compiling it is speculative and most tiny
functions never run enough to earn it back.

Loops get their own, lower threshold through **on-stack replacement**. A
loop inside a function that is only ever called once still compiles, and
execution jumps from the interpreter into the middle of the compiled
version without unwinding anything. That is what makes a one-shot batch
script fast.

You can watch it happen:

```console
$ ZURI_JIT_LOG=1 zuri run fib.zu
[jit] 'fib' goes straight to Bayelsa
[jit] compiled 'fib' in Bayelsa (13 bytecode ops, 0 osr point(s))
196418
```

`fib` goes straight to Bayelsa because every one of its operations ran in
the interpreter before it warmed up. With Bayelsa off, Kebbi compiles it
instead, and the line lists what Kebbi proved:

```console
$ ZURI_JIT_BAYELSA=0 ZURI_JIT_LOG=1 zuri run fib.zu
[jit] compiled 'fib' in Kebbi (13 bytecode ops, 0 osr point(s), speculative_params=0x1, speculative_regs=0x0, speculative_lists=0x0, speculative_ints=0x1)
196418
```

A function Bayelsa leaves to Kebbi gets a line saying why:

```text
[jit] 'report' stays in Kebbi: catch at ip 5
```

And you can see what did and did not make it:

```console
$ ZURI_JIT_COVERAGE=1 zuri run fib.zu
=== jit coverage ===
status	calls	threshold	ops	osr	name
compiled	62009	169	11	0	fib
cold	0	56	100	0	@.script
...
```

`calls` against `threshold` tells you how close something came.

## Keep Errors for Errors

A function with a `catch` compiles like any other. While no error comes
through, a `catch` costs next to nothing: the handler is registered and
dropped around the body, and the body runs as compiled code.

What costs is an error that is actually raised. A `raise` hands the frame
back to the interpreter at that instruction, and a caught error resumes at
its handler in the interpreter too. The function returns to compiled code
the next time its loop comes round, or the next time it is called. For a
genuine failure that is the right trade. For an outcome a loop meets every
few iterations, it is not:

```zuri
def parse_or_raise(i) {
  if i % 10 == 0 {
    raise ValueError('not a digit')
  }
  return i % 10
}

def parse_or_nil(i) {
  if i % 10 == 0 {
    return nil
  }
  return i % 10
}

def with_raise(n) {
  var total = 0
  iter var i = 0; i < n; i++ {
    catch {
      total += parse_or_raise(i)
    } as e {
      total += 0
    }
  }
  return total
}

def with_nil(n) {
  var total = 0
  iter var i = 0; i < n; i++ {
    var digit = parse_or_nil(i)
    if digit != nil {
      total += digit
    }
  }
  return total
}

def timed(label, work) {
  var start = time()
  work()
  echo '${label}: ${((time() - start) * 1000).round()}ms'
}

timed('raise and catch', @() {
  iter var r = 0; r < 200; r++ {
    with_raise(1000)
  }
})

timed('return nil     ', @() {
  iter var r = 0; r < 200; r++ {
    with_nil(1000)
  }
})
```

Two hundred rounds of a thousand iterations, one in ten of them failing:

| Variant | Time |
| --- | --- |
| `raise`, caught in the loop | 73ms |
| `nil` returned and checked | 11ms |

When failing is an expected outcome, input that might not parse or a key
that might be missing, return a value that says so and test it. Keep
`raise` for what the caller cannot reasonably carry on from. A `raise` that
never fires, a guard clause at the top of a function, costs the compiled
path nothing.

## Type Annotations Are a Performance Feature

The JIT speculates on the types it has seen. When it has been told instead,
it does not need to guess, and the guard disappears.

```zuri
def sum_untyped(values) {
  var total = 0
  iter var i = 0; i < values.length(); i++ {
    total += values[i]
  }
  return total
}

def sum_typed(values: list) {
  var total = 0
  iter var i = 0; i < values.length(); i++ {
    total += values[i]
  }
  return total
}
```

Twenty thousand rounds over a thousand-element list:

| Variant | Time |
| --- | --- |
| `values: list` | 85ms |
| no annotation | 150ms |

Running them in both orders gives the same answer, which is the check that
tells you it is the annotation and not the warm-up.

Annotate the parameters of any function on a hot path. It costs one word
per parameter and it documents the function at the same time.

## Keep Types Stable

Speculation works by assuming the future looks like the past. A variable
that holds a number on ten thousand iterations and a string on the ten
thousand and first causes a **deoptimisation**: the compiled code bails to
the interpreter, and the function may be recompiled with a weaker
assumption.

One deopt is cheap. A loop that deopts every iteration is slower than never
compiling at all.

In practice this means:

- One variable, one kind of value.
- A list of numbers, not a list of numbers-and-sometimes-strings.
- A field that starts `nil` and later holds a number is two types. Start it
  at `0`.

That last one is the common case. `var count` on a class field is a `nil`
until the constructor runs, and every read of it in compiled code has to
handle both. `var count = 0` does not.

## Where to Put a Hot Loop

A per-element loop belongs in a **typed free function**, not in a method
reading a field:

Slower, because every iteration reads `self.pixels` back through a field
guard:

```zuri
class Canvas {
  var pixels = bytes(0)

  brighten(amount) {
    iter var i = 0; i < self.pixels.length(); i++ {
      self.pixels[i] = self.pixels[i] + amount
    }
  }
}
```

Faster, because the loop sees plain locals whose types are declared:

```zuri
def _brighten(pixels: bytes, amount: number) {
  iter var i = 0; i < pixels.length(); i++ {
    pixels[i] = pixels[i] + amount
  }
}

class Canvas {
  var pixels = bytes(0)

  brighten(amount) {
    _brighten(self.pixels, amount)
  }
}
```

The standard library's `imagine` module is written this way throughout, and
the difference on a per-pixel loop is measured in multiples, not percent.

## Reading the Bytecode

When you want to know what the compiler actually did, ask it:

```zuri
import zuri

echo zuri.compile('var a = 1 + 2').map(@(i) => i.op)
```

```console
[LoadConst, AddImm, SetGlobal, LoadNil, Return]
```

`AddImm` rather than a separate load tells you the constant was folded into
the instruction. This is the fastest way to check whether a rewrite did
what you hoped, and it never goes stale.

## Measuring Honestly

Every number in this chapter is one measurement on one machine. Treat them
as ratios rather than as figures to reproduce: yours will differ with your
processor, your build and what else the machine is doing. Five rules make
the difference between a measurement and a guess.

**Warm up before you time.** The first few hundred iterations run
interpreted, and if your benchmark is short, that is all you measured.

**Run each variant on its own.** Two variants in one process share warm-up
state and a heap. Order them both ways; if the answer changes, you measured
the order.

**Watch the machine.** A laptop under sustained load throttles, and a
second run is not comparable to the first. Check the load before the timed
run, not before the build that precedes it.

**Change one thing.** A "fix" that touches three sites has three possible
explanations for its effect, and at least one of them is usually a
regression hiding behind the other two.

**Measure the real program.** A microbenchmark of one function tells you
about that function in isolation, with a warm cache and no competing
allocation. `ZURI_JIT_COVERAGE=1` on the actual workload tells you which
function to look at in the first place, which is nearly always the more
valuable answer.

### Timing Something Yourself

`time()` returns epoch seconds with microsecond resolution, so a timing
harness is four lines:

```zuri
def timed(label, work) {
  var start = time()
  var result = work()

  echo '${label}: ${((time() - start) * 1000).round()}ms'

  return result
}

var squares = timed('build a list', @() {
  var out = []

  iter var i = 0; i < 200000; i++ {
    out.append(i * i)
  }

  return out
})

echo squares.length()
```

The elapsed line will differ every run; the length will not. Print both, so
a change in the second tells you the harness broke rather than the code
getting faster.

## Choosing a Mode

Both compilers run by default. `ZURI_JIT_BAYELSA=0` turns Bayelsa off, and
every hot function then stays in Kebbi.

Keep the default for anything that runs long enough for its speed to
matter: servers, batch jobs, numeric work, anything whose hot loops run for
more than a moment. That is where Bayelsa's code repays its compile many
times over.

Turn Bayelsa off when:

- **The program is short.** A script that finishes in a fraction of a
  second spends a real share of its life compiling a second time, and the
  faster code arrives too late to pay for itself.
- **Cores are scarce.** Bayelsa's compiles take more processor time than
  Kebbi's. On a machine or container held to one core, or with every core
  busy with isolates, that time comes out of the program's own.
- **Start-up is the workload.** A command-line tool run over and over, a
  few milliseconds each time, gains nothing from code that is faster on
  its thousandth iteration.
- **Timings have to hold from the first run.** Kebbi reaches its speed
  sooner and stays there. Under Bayelsa a function speeds up once more,
  part way through a run.

Measure both. The switch is one variable, and running the real workload
each way settles the question for that workload.

## The Environment Variables

These exist for measurement and debugging. Ordinary programs need none of
them.

| Variable | Effect |
| --- | --- |
| `ZURI_JIT=0` | disable the JIT entirely |
| `ZURI_JIT_BAYELSA=0` | compile with Kebbi alone |
| `ZURI_JIT_LOG=1` | one line per compilation attempt |
| `ZURI_JIT_LOG_IR=1` | dump the Cranelift IR, and Bayelsa's own before it |
| `ZURI_JIT_COVERAGE=1` | a table of what compiled, at exit |
| `ZURI_JIT_NO_SPECIALIZATION=1` | compile, but do not speculate on types |
| `ZURI_JIT_THREADS=n` | background compiler threads |
| `ZURI_JIT_CALL_K`, `ZURI_JIT_OSR_K` | the warm-up curve constants |
| `ZURI_JIT_TIERUP_K` | how much profiling work comes before Bayelsa |
| `ZURI_GC_LOG=1` | garbage collector activity |
| `ZURI_OPCODE_PROFILE=1` | interpreter opcode histogram |

`ZURI_JIT=0` is the most useful one. Running a benchmark with and without it
tells you immediately whether your hot path is being compiled at all, which
is the first question to ask when something is slower than it should be.

## When to Stop

The interpreter is fast and the JIT is automatic. Most Zuri code needs no
performance work at all, and the code that does usually needs exactly one
of the three things in this chapter: return a value instead of raising for
an expected outcome, annotate a parameter, or stop putting two types in one
variable.

Reach for anything more exotic only after `ZURI_JIT_COVERAGE` has told you
which function is actually the problem.
