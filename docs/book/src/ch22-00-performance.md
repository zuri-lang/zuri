# Performance and the JIT

Zuri runs your bytecode in an interpreter until part of it becomes hot. It
then compiles that part to machine code and runs the machine code instead.
There are two compilers, a fast baseline compiler and a slower optimizing
compiler. By default, hot functions are compiled by the optimizing compiler,
and the baseline compiler handles the ones it turns down. All of this
happens automatically, on background threads, without any annotations or
flags.

Here's the difference it makes for a simple recursive Fibonacci function,
measured on an idle machine:

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

That's more than three times faster, for a function that does nothing but
add and compare, and you don't have to write anything to get it. The rest of
this chapter explains how the compilers work, and covers the few cases where
the way you write code determines whether you get that speedup.

## The Two Compilers

**Kebbi** is the baseline compiler. It turns a function's bytecode into
machine code one instruction at a time, keeps values in machine registers,
and works out what it can about types before it compiles. For example, it
knows that a parameter annotated as `list` is a list, and that a counter
that starts at 0 and goes up by 1 is a whole number. Kebbi compiles quickly,
and the code it produces runs several times faster than the interpreter.

**Bayelsa** is the optimizing compiler. It represents the whole function as
a graph of typed values, and optimizes that graph before generating any
machine code. It applies a fact it has proven once to every later use, moves
checks that can't change out of loops, keeps loop counters as plain
integers, inlines small functions into the functions that call them, and
wherever nothing proves a type, speculates based on what the function has
done so far. Bayelsa takes longer to compile, and the code it produces is
the fastest that Zuri can generate.

Both compilers use Cranelift, and both run on background threads. A program
never waits for a compilation. It keeps running in the interpreter, or in
the code it already has, until the new code is ready.

### How a Function Is Compiled

1. **The interpreter.** Every function starts here. The interpreter counts
   how many times the function is called and how many times its loops
   repeat, and records what kinds of values each operation sees.
2. **Bayelsa (the default).** Once a function is warm, Bayelsa compiles it
   directly, using the information the interpreter recorded. Small helper
   functions are inlined, checks on constants are moved out of loops, and
   variables are kept in CPU registers.
3. **Kebbi as a fallback.** If Bayelsa turns down a function, for example
   because it contains a `catch` block, Kebbi compiles it instead.

If you set `ZURI_JIT_TIERED=1`, Zuri can also use a multi-tier pipeline. A
warm function is first compiled by Kebbi with profiling instrumentation, and
once that profiling code has done enough work, Bayelsa compiles the function
using the information it gathered.

### When Speculation Fails

Bayelsa compiles the code based on what it has seen. A loop that has only
ever added whole numbers uses integer arithmetic. A module constant that
held `10` is compared as the integer 10. An index operation that has only
ever seen `bytes` reads a byte. Each assumption is checked at the point
where it's made, and if a check fails, the frame is handed back to the
interpreter at that exact instruction, with every variable as the compiled
code left it. This is called **deoptimization**.

The place where the check failed is remembered, and the next compilation of
the function doesn't make that assumption there. So a function is
deoptimized at a particular place only once, not every time it gets there.

### Functions That Stay in Kebbi

Bayelsa leaves a function to Kebbi in two cases:

- The function contains a `catch`. Kebbi compiles it, including its
  handlers.
- The function's hot operations are ones that Kebbi handles inline but
  Bayelsa would pass to the runtime, such as arithmetic on values that
  weren't always numbers, method calls on strings, and indexing lists and
  strings whose type can't be determined. In those cases, Kebbi's code is
  faster.

A function that's left in Kebbi is recompiled without its profiling code, so
it runs at Kebbi's full speed.

## How Functions Become Hot

Every function counts its own calls, and every loop counts how many times it
repeats. When a counter reaches its threshold, a compilation job is sent to
a background thread. When the machine code is ready, later calls use it.

The threshold isn't a fixed number. It depends on the size of the function:

```text
threshold = K / sqrt(instruction_count)
```

A large function gets a low threshold, because it does more work per call,
so compiling it pays off sooner. A getter with three instructions gets a
high threshold, because compiling it is a gamble, and most tiny functions
never run often enough to make it worthwhile.

Loops get their own, lower threshold, through **on-stack replacement**. A
loop inside a function that's only ever called once still gets compiled, and
execution jumps from the interpreter into the middle of the compiled version
without unwinding anything. That's what makes a script that runs once fast.
A loop's threshold follows the same curve, based on the length of the loop
itself, so a short loop in a long function, such as setup code that goes
through a small table, has to repeat many times before the function around
it is compiled.

You can see it happen:

```console
$ ZURI_JIT_LOG=1 zuri run fib.zu
[jit] 'fib' goes straight to Bayelsa
[jit] compiled 'fib' in Bayelsa (13 bytecode ops, 0 osr point(s))
196418
```

`fib` goes straight to Bayelsa, because by default, warm functions are
compiled directly by the optimizing compiler. With Bayelsa turned off
(`ZURI_JIT_KEBBI=1`), Kebbi compiles it instead, and the output lists what
Kebbi was able to prove:

```console
$ ZURI_JIT_KEBBI=1 ZURI_JIT_LOG=1 zuri run fib.zu
[jit] compiled 'fib' in Kebbi (13 bytecode ops, 0 osr point(s), speculative_params=0x1, speculative_regs=0x0, speculative_lists=0x0, speculative_ints=0x1)
196418
```

When Bayelsa leaves a function to Kebbi, the output includes a line
explaining why:

```text
[jit] 'report' stays in Kebbi: catch at ip 5
```

You can also see what was and wasn't compiled:

```console
$ ZURI_JIT_COVERAGE=1 zuri run fib.zu
=== jit coverage ===
status	calls	threshold	ops	osr	name
compiled	62009	169	11	0	fib
cold	0	56	100	0	@.script
...
```

Comparing `calls` with `threshold` tells you how close a function came to
being compiled.

## Use Errors Only for Errors

A function with a `catch` is compiled like any other. As long as no error is
raised, a `catch` costs almost nothing: the handler is registered and
removed around the body, and the body runs as compiled code.

What does cost something is an error that's actually raised. A `raise` hands
the frame back to the interpreter at that instruction, and a caught error
continues at its handler in the interpreter too. The function returns to
compiled code the next time its loop repeats, or the next time it's called.
That's a reasonable cost for a real failure, but not for an outcome that a
loop runs into every few iterations:

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

Here are two hundred rounds of a thousand iterations, with one in ten
failing:

| Variant | Time |
| --- | --- |
| `raise`, caught inside the loop | 73ms |
| `nil` returned and checked | 11ms |

When failure is an expected outcome, such as input that might not parse or a
key that might be missing, return a value that indicates it and check that
value. Save `raise` for situations the caller can't reasonably recover from.
A `raise` that never runs, such as a guard clause at the top of a function,
doesn't slow down the compiled code.

## Type Annotations Improve Performance

The JIT speculates based on the types it has seen. When you tell it the type
instead, it doesn't need to guess, and the check disappears.

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

Here are twenty thousand rounds over a list of a thousand elements:

| Variant | Time |
| --- | --- |
| `values: list` | 85ms |
| No annotation | 150ms |

Running the two variants in both orders gives the same result, which
confirms that the difference comes from the annotation and not from warm-up.

Annotate the parameters of any function on a hot path. It costs one word per
parameter, and it documents the function at the same time.

## Keep Types Stable

Speculation works by assuming the future will look like the past. If a
variable holds a number for ten thousand iterations and then a string on the
next one, the code is **deoptimized**: the compiled code falls back to the
interpreter, and the function might be recompiled with a weaker assumption.

A single deoptimization is cheap. But a loop that's deoptimized on every
iteration is slower than if it had never been compiled at all.

In practice, this means:

- Use each variable for one kind of value.
- Use a list of numbers, not a list of numbers that sometimes contains
  strings.
- A field that starts as `nil` and later holds a number has two types. Start
  it at `0` instead.

The last one is the most common case. A class field declared as `var count`
is `nil` until the constructor runs, so every read of it in compiled code
has to handle both types. `var count = 0` doesn't have this problem.

## Where to Put a Hot Loop

A loop that processes every element belongs in a **standalone function with
typed parameters**, not in a method that reads a field:

This is slower, because every iteration reads `self.pixels` through a field
check:

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

This is faster, because the loop only uses local variables with declared
types:

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
the difference for a loop over every pixel is measured in multiples, not
percentages.

## Reading the Bytecode

When you want to know what the compiler actually did, ask it:

```zuri
import zuri

echo zuri.compile('var a = 1 + 2').map(@(i) => i.op)
```

```console
[LoadConst, AddImm, SetGlobal, LoadNil, Return]
```

The `AddImm` instruction, rather than a separate load, tells you that the
constant was included in the instruction itself. This is the quickest way to
check whether a change to your code did what you hoped, and it's always
accurate.

## Measuring Performance Reliably

Every number in this chapter is a single measurement on a single machine.
Treat them as ratios rather than numbers to reproduce, because your results
will vary with your processor, your build and whatever else the machine is
doing. These five rules make the difference between a measurement and a
guess.

**Warm up before you start timing.** The first few hundred iterations run in
the interpreter, and if your benchmark is short, that's all you've measured.

**Run each variant separately.** Two variants in the same process share
warm-up state and a heap. Run them in both orders, and if the result
changes, you've measured the order.

**Keep an eye on the machine.** A laptop under sustained load slows itself
down, so a second run isn't comparable to the first. Check the load just
before the timed run, not before the build that comes before it.

**Change one thing at a time.** If a fix changes three places, there are
three possible explanations for its effect, and one of them is often a
regression hidden by the other two.

**Measure the real program.** A microbenchmark of a single function tells
you about that function on its own, with a warm cache and no other
allocations competing with it. Running the real workload with
`ZURI_JIT_COVERAGE=1` tells you which function to look at in the first
place, which is nearly always more useful.

### Timing Code Yourself

`time()` returns the time in seconds since the epoch, with microsecond
resolution, so a timing harness only takes four lines:

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

The elapsed time will be different on every run, but the length won't. Print
both, so that if the length ever changes, you know the harness is broken,
rather than thinking the code has become faster.

## Choosing a Mode

By default, Bayelsa compiles functions directly, and falls back to Kebbi
whenever it turns a function down. `ZURI_JIT_KEBBI=1` (or
`ZURI_JIT_BAYELSA=0`) turns off Bayelsa, so every hot function stays in
Kebbi. `ZURI_JIT_TIERED=1` turns on the multi-tier pipeline, where Kebbi
profiles a function before Bayelsa compiles it.

Keep the default for anything that runs long enough for its speed to matter,
such as servers, batch jobs, numeric work, and anything whose hot loops run
for more than a moment. That's where Bayelsa's code pays back the cost of
compiling it many times over.

Turn off Bayelsa (`ZURI_JIT_KEBBI=1`) when:

- **The program is short.** A script that finishes in a fraction of a second
  spends a significant part of its run time compiling, and the faster code
  arrives too late to pay for itself.
- **CPU cores are limited.** Bayelsa's compilations use more processor time
  than Kebbi's. On a machine or container limited to one core, or when every
  core is busy running isolates, that time comes out of the program's own.
- **Startup time is what matters.** A command-line tool that's run over and
  over again, for a few milliseconds each time, gains nothing from code
  that's faster on its thousandth iteration.
- **Timings need to be consistent from the first run.** Kebbi reaches its
  full speed sooner and stays there.

Measure both. The setting is a single environment variable, and running your
real workload both ways answers the question for that workload.

## Environment Variables

These variables are for measuring and debugging. Ordinary programs don't
need any of them.

| Variable | Effect |
| --- | --- |
| `ZURI_JIT=0` | Turns off the JIT completely |
| `ZURI_JIT_KEBBI=1` | Compiles with Kebbi only (the same as `ZURI_JIT_BAYELSA=0`) |
| `ZURI_JIT_TIERED=1` | Turns on multi-tier mode, where Kebbi profiles before Bayelsa compiles |
| `ZURI_JIT_LOG=1` | Prints one line for each compilation attempt |
| `ZURI_JIT_LOG_IR=1` | Prints the Cranelift IR, and Bayelsa's own IR before it |
| `ZURI_JIT_COVERAGE=1` | Prints a table of what was compiled when the program exits |
| `ZURI_JIT_NO_SPECIALIZATION=1` | Compiles functions, but without speculating on types |
| `ZURI_JIT_THREADS=n` | Sets the number of background compiler threads |
| `ZURI_JIT_CALL_K`, `ZURI_JIT_OSR_K` | Set the constants for the warm-up curve |
| `ZURI_JIT_TIERUP_K` | Sets how much profiling work comes before Bayelsa, in tiered mode |
| `ZURI_GC_LOG=1` | Prints garbage collector activity |
| `ZURI_GC_NURSERY_MB=n` | Sets the starting and minimum size of the young generation, in megabytes |
| `ZURI_OPCODE_PROFILE=1` | Prints a histogram of the interpreter's opcodes |
| `MIMALLOC_PURGE_DELAY=n` | Sets how many milliseconds freed memory waits before it's returned to the operating system. The default is 10. |

`ZURI_JIT=0` is the most useful one. Running a benchmark with and without it
tells you straight away whether your hot path is being compiled at all,
which is the first thing to check when something is slower than it should
be.

## Knowing When to Stop

The interpreter is fast, and the JIT works automatically. Most Zuri code
doesn't need any performance work at all, and code that does usually needs
just one of the three changes described in this chapter: return a value
instead of raising an error for an expected outcome, annotate a parameter,
or stop storing two types in one variable.

Only try anything more unusual after `ZURI_JIT_COVERAGE` has shown you which
function is actually causing the problem.
