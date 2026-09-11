# Performance and the JIT

Zuri runs your bytecode in an interpreter until a piece of it gets hot,
then compiles that piece to machine code with Cranelift and runs it there
instead. The tiering happens on its own, on a background thread, with no
annotations and no flags.

Here is what that is worth, on a plain recursive Fibonacci:

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
$ zuri fib.zu
196418
took 96ms

$ ZURI_JIT=0 zuri fib.zu
196418
took 772ms
```

Eight times, for a function that does nothing but add and compare. You do
not have to do anything to get that. The rest of this chapter is about the
handful of cases where what you write decides whether you get it.

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
$ ZURI_JIT_LOG=1 zuri fib.zu
[jit] compiled 'fib' (11 bytecode ops, 0 osr point(s), speculative_params=0x1, speculative_regs=0x0)
196418
```

And you can see what did and did not make it:

```console
$ ZURI_JIT_COVERAGE=1 zuri fib.zu
=== jit coverage ===
status	calls	threshold	ops	osr	name
compiled	62009	169	11	0	fib
cold	0	56	100	0	@.script
...
```

`calls` against `threshold` tells you how close something came.

## The One Rule That Matters

**A function containing a `catch` block is never compiled.**

`catch` maintains unwind state the interpreter owns, and the JIT generates
no unwind logic, so a function with `PushCatch` in its bytecode stays
interpreted forever. Not the `catch` block. The whole function.

Measure it:

```zuri
def with_catch(n) {
  var total = 0

  catch {
    iter var i = 0; i < n; i++ {
      total += i
    }
  } as e

  return total
}

def without_catch(n) {
  var total = 0

  iter var i = 0; i < n; i++ {
    total += i
  }

  return total
}

def guarded(n) {
  catch {
    return without_catch(n)
  } as e {
    return 0
  }
}
```

Two thousand rounds of a thousand iterations each:

```console
catch inside  : 1245ms
catch outside : 18ms
no catch      : 3ms
```

Seventy times, for moving one `catch` up a level. The fix is always the
same: **put the loop in its own function and wrap the call, not the loop.**

`raise` on its own is fine. A `raise` compiles to a bail-out back to the
interpreter at that exact instruction, so a guard clause that never fires
costs the compiled path nothing. It is `catch` that disqualifies a
function.

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

```console
typed  : 212ms
untyped: 310ms
```

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

Four rules, learned the hard way.

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

## The Environment Variables

These exist for measurement and debugging. Ordinary programs need none of
them.

| Variable | Effect |
| --- | --- |
| `ZURI_JIT=0` | disable the JIT entirely |
| `ZURI_JIT_LOG=1` | one line per compilation attempt |
| `ZURI_JIT_LOG_IR=1` | dump the Cranelift IR |
| `ZURI_JIT_COVERAGE=1` | a table of what compiled, at exit |
| `ZURI_JIT_NO_SPECIALIZATION=1` | compile, but do not speculate on types |
| `ZURI_JIT_THREADS=n` | background compiler threads |
| `ZURI_JIT_CALL_K`, `ZURI_JIT_OSR_K` | the warm-up curve constants |
| `ZURI_GC_LOG=1` | garbage collector activity |
| `ZURI_OPCODE_PROFILE=1` | interpreter opcode histogram |

`ZURI_JIT=0` is the useful one. Running a benchmark with and without it
tells you immediately whether your hot path is being compiled at all, which
is the first question to ask when something is slower than it should be.

## When to Stop

The interpreter is fast and the JIT is automatic. Most Zuri code needs no
performance work at all, and the code that does usually needs exactly one
of the three things in this chapter: move a `catch` out of a hot function,
annotate a parameter, or stop putting two types in one variable.

Reach for anything more exotic only after `ZURI_JIT_COVERAGE` has told you
which function is actually the problem.
