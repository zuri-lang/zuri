# Concurrency with Isolates

Zuri's concurrency model has one rule: **nothing is shared.**

An isolate is a real operating-system thread running a complete, private
VM. It has its own heap, its own garbage collector, its own register stack
and its own module namespace. Two isolates never hold a pointer to the same
object, which means there are no data races to reason about, no locks to
take, and no stop-the-world pause in one thread caused by allocation in
another.

Values cross between isolates by being **copied**. The copy is a faithful
one: cycles are preserved, shared identity inside one payload is preserved,
and class instances arrive as instances of the same class.

## Spawning

```zuri
import isolate

def square(n) {
  return n * n
}

var task = isolate.spawn(square, 12)

echo task.join()
```

```console
144
```

`spawn(fn, ...args)` queues the call and returns an `Isolate` handle
immediately. `join()` waits for it and gives you the result. Calling
`join()` again returns the same value; it is not a one-shot.

### What Can Be Spawned

Any function: a top-level `def`, a helper declared inside another function,
a lambda, a bound method. What it captures travels with it, including the
modules it uses:

```zuri
import isolate
import json

def encode_all(items) {
  return items.map(@(item) => json.encode(item)).length()
}

echo isolate.spawn(encode_all, [{ a: 1 }, { b: 2 }]).join()
```

```console
2
```

A module is not copied the way a list is. The isolate loads the same module
for itself and uses its own copy, which is why this works at all: module
top levels are declarations, they run once, and the result is cached. A
module built out of live per-process state is the case to think twice
about, since the isolate gets a fresh one rather than yours.

Two things still cannot be spawned. **A native function or a class** as the
spawn target itself; wrap it in a `def`. And **a resource handle** such as
an open socket or file is *moved* rather than copied, so the side that
handed it over no longer has it.

Putting the worker in its own module stays the better structure once it
grows past a few lines:

<span class="filename">Filename: work.zu</span>

```zuri,ignore
import net

def serve(channel) {
  var listener = net.TcpStream()
  # ...
}
```

<span class="filename">Filename: main.zu</span>

```zuri,ignore
import isolate
import .work

isolate.spawn(work.serve, channel)
```

That is the shape the rest of this book uses. A worker reached by name is
resolved in the isolate's own namespace, so nothing about it has to be
reconstructed.

An `import` written inside the function body works too, and runs in the
isolate:

```zuri
import isolate

def make_id() {
  import uuid

  return uuid.v4().to_string().length()
}

echo isolate.spawn(make_id).join()
```

```console
36
```

The arguments are copied, not shared. An isolate that mutates a list it was
handed is mutating its own copy:

```zuri
import isolate

def append_to(items) {
  items.append('from the isolate')
  return items.length()
}

var original = ['a']

echo isolate.spawn(append_to, original).join()
echo original
```

```console
2
[a]
```

The isolate saw two elements. The caller still has one. That is the whole
concurrency model in one example: there is no way to write a data race,
because there is nothing shared to race over.

## Running Many at Once

Spawn a list, join a list:

```zuri
import isolate

def square(n) {
  return n * n
}

var tasks = [1, 2, 3, 4].map(@(n) => isolate.spawn(square, n))

echo tasks.map(@(task) => task.join())
```

```console
[1, 4, 9, 16]
```

`isolate.map()` does exactly that in one call:

```zuri
import isolate

def square(n) {
  return n * n
}

echo isolate.map(square, [1, 2, 3, 4, 5])
```

```console
[1, 4, 9, 16, 25]
```

`wait_all(tasks, timeout)` waits for every task; `wait_any(tasks, timeout)`
returns as soon as one finishes.

## The Lifecycle of a Task

A spawned task moves through exactly two states, and four methods let you
ask about it without blocking.

```zuri,ignore
import isolate
import .work

var task = isolate.spawn(work.slow, 3)

echo task.try_join()
echo task.is_done()
echo task.status()

echo task.join()
echo task.status()
```

```console
nil
false
pending
finished
done
```

| Method | Answers | Blocks? |
| --- | --- | --- |
| `join(timeout)` | the result | yes |
| `try_join()` | the result, or `nil` if not ready | no |
| `is_done()` | whether it has finished | no |
| `status()` | `'pending'` or `'done'` | no |
| `name()` | the name it was given, or `nil` | no |

`try_join()` returning `nil` is ambiguous when the task's own result could
be `nil`; pair it with `is_done()` when that matters.

`join()` may be called more than once. It is not a one-shot: the second
call returns the same value immediately.

### Naming a Task

`spawn()` leaves a task anonymous. `spawn_named()` gives it a name that
shows up in diagnostics:

```zuri,ignore
var named = isolate.spawn_named('importer', work.quick, 5)
var plain = isolate.spawn(work.quick, 5)

echo named.name()
echo plain.name()
```

```console
importer
nil
```

Name anything long-running. A stuck program is far easier to diagnose when
the task can say what it is.

## Timeouts Are in Seconds

**Every timeout in the `isolate` module is measured in seconds**, and it
takes a fraction:

```zuri
import isolate

var empty = isolate.channel(1)
var start = time()

catch {
  empty.recv(0.3)
} as e {
  echo '${e.type} after about ${((time() - start) * 10).round() * 100}ms'
}
```

```console
IsolateTimeoutError after about 300ms
```

This applies to `join()`, `send()`, `recv()`, `select()`, `wait_any()`,
`wait_all()` and `shutdown()` alike.

It is worth stating loudly because **the `net` module uses milliseconds**
for its own timeouts. `socket.set_read_timeout(5000)` is five seconds;
`channel.recv(5000)` is an hour and twenty minutes. The two modules are
easy to use in one program, and the mistake is silent — a timeout that
never fires simply looks like a hang.

Omitting the timeout means "wait forever", which is the right default when
the other side is code you control and the wrong one when it is not.

## Cancellation Is Cooperative

`cancel()` **requests** that a task stop. It does not kill anything.

What happens next depends on what the task is doing:

**Blocked in a channel operation, a `join()`, a `wait_*` or a `select()`** —
the call is interrupted within roughly 50ms and raises
`IsolateCancelledError`.

**Running ordinary code** — nothing is interrupted. The task's own function
must notice and return:

```zuri,ignore
import isolate

def slow(seconds) {
  var start = time()

  while time() - start < seconds {
    if isolate.is_cancelled() {
      return 'stopped early'
    }
  }

  return 'ran to completion'
}
```

`isolate.is_cancelled()` is the module-level function a worker calls about
itself. `task.is_cancelled()` is the method the *spawner* calls to ask
whether it requested cancellation — and it answers `true` from the moment
`cancel()` was called, whether or not the task noticed:

```zuri,ignore
var task = isolate.spawn(work.slow, 3)

task.cancel()

echo task.join()
echo task.is_cancelled()
```

```console
ran to completion
true
```

That output is not a contradiction. The worker in this example does not
poll, so it finished normally; `is_cancelled()` reports the request, not
the outcome. A loop with no cancellation check is a loop that cannot be
stopped.

Put the check where the loop turns over, and make it cheap. Checking once
per iteration of an outer loop is usually enough; checking inside the
innermost arithmetic is not worth it.

## What Can Cross, and What It Costs

Arguments in, results out, and channel traffic all cross the same way:
**everything is copied**. There is no sharing and no reference that
survives the boundary.

### The Copy Is Faithful

A copy is not a shallow snapshot. Cycles survive, shared identity inside
one payload survives, and a class instance arrives as an instance of the
same class with its methods intact:

```zuri
import isolate

class Point {

  @new(x) {
    self.x = x
  }

  doubled() {
    return self.x * 2
  }
}

def identity(value) {
  return value
}

# A list that contains itself.
var cyclic = [1]
cyclic.append(cyclic)

# Two slots holding one list.
var shared = [1]
var pair = [shared, shared]

echo isolate.spawn(identity, Point(3)).join().doubled()
echo typeof(isolate.spawn(identity, cyclic).join())

var back = isolate.spawn(identity, pair).join()
back[0].append(2)

echo back[1].length()
```

```console
6
list
2
```

The last line is the one to notice. `pair` held the same list twice, and on
the other side it still does: appending through `back[0]` is visible
through `back[1]`. The copy preserved the *shape* of the sharing, not just
the values.

### What Cannot Cross

A handful of things are tied to the isolate that made them, and sending one
raises rather than silently producing something broken:

```zuri
import isolate

def identity(value) {
  return value
}

catch {
  isolate.spawn(identity, file('notes.txt'))
} as e {
  echo e.message.lines()[0]
}
```

```console
cannot send a file across isolates; only nil, bool, number, string, bytes, bigint, range, list, dict, instance, class, bound method, function, module, and native-pointer values can cross
```

The message lists what *can* cross, which is the more useful half. An open
file is the one you will meet: it is a handle onto a position in a
descriptor this process owns, and there is no copy of that to hand over.
Send the path instead and let the worker open it.

### The Cost Model

Spawning is cheap. A task is a small descriptor holding the callee and a
snapshot of its arguments, pushed onto a queue the pool drains, so queueing
a hundred thousand of them is reasonable.

What is not free is the copy. Handing an isolate a large list copies the
whole list, once per spawn. When a worker needs a lot of data, give it a
*description* of the work — a path, a range of indices, a query — and let
it do its own reading:

```zuri,ignore
# Copies the file's contents into every worker.
isolate.map(work.process, files.map(@(p) => file(p).read()))

# Copies a short path into every worker instead.
isolate.map(work.read_and_process, files)
```

## Errors Cross the Boundary

An uncaught error inside an isolate surfaces at the `join()` as an
`IsolateError`, carrying the original error's message and the stack trace
from inside the worker:

```zuri
import isolate

def boom() {
  raise ValueError('worker failed')
}

catch {
  isolate.spawn(boom).join()
} as e {
  echo e.type
  echo e.message.lines()[0]
}
```

```console
IsolateError
ValueError: worker failed
```

Nothing is lost. You get the failure where you can do something about it,
with enough information to find it. Note the shape of the message: the
`IsolateError` wraps the worker's own error rather than replacing it, so
`e.message` still names `ValueError` and the line it came from.

An error in a task nobody joins has nowhere to surface. That is what
`scope()`, further down, exists to prevent.

## Channels

A channel is a bounded queue that crosses isolate boundaries. One side
sends, the other receives, and the values are copied on the way across like
everything else:

```zuri
import isolate

def producer(channel, count) {
  iter var i = 0; i < count; i++ {
    channel.send(i)
  }

  channel.close()
}

var channel = isolate.channel(4)
var task = isolate.spawn(producer, channel, 3)

while true {
  var value = channel.recv()

  if value == nil and channel.is_closed() {
    break
  }

  echo value
}

task.join()
```

```console
0
1
2
```

Read the loop condition carefully, because it is the part that is easy to
get wrong. `recv()` returns `nil` both for "a `nil` was sent" and for "the
channel is closed and drained", so the test for the end of the stream is
`nil` **and** `is_closed()`. Checking only for `nil` would stop early on a
legitimate `nil`; checking only `is_closed()` would stop before the queue
had drained.

`channel(capacity)` bounds the queue, which gives you backpressure for
free: a producer that outruns its consumer blocks on `send()` rather than
growing the queue without limit.

| Method | Behaviour |
| --- | --- |
| `send(value, timeout)` | blocks while the channel is full |
| `recv(timeout)` | blocks while the channel is empty |
| `try_recv()` | returns immediately, `nil` when empty |
| `close()` | no more sends; pending receives drain, then return `nil` |
| `is_closed()` | whether it has been closed |
| `length()` | how many values are waiting |

Every one of those behaviours is observable from a single isolate, which
makes a channel easy to reason about before you introduce a second one:

```zuri
import isolate

var c = isolate.channel(2)

c.send('a')
c.send('b')

echo c.length()
echo c.try_recv()

c.close()

echo c.is_closed()
echo c.recv()
echo c.recv()

catch {
  c.send('z')
} as e {
  echo '${e.type}: ${e.message}'
}
```

```console
2
a
true
b
nil
IsolateError: cannot send on a closed channel
```

Read the last four lines together. After `close()`, the queue still
**drains**: `recv()` returned the buffered `b` before it started returning
`nil`. A closed, drained channel returns `nil` forever rather than raising,
and only `send()` raises.

Closing twice is harmless. `try_recv()` on an empty channel returns `nil`
immediately and never blocks.

## Selecting Across Channels

`select()` waits on several channels at once and tells you which one
produced a value:

```zuri
import isolate

var c1 = isolate.channel(1)
var c2 = isolate.channel(1)

c2.send('from c2')

var picked = isolate.select([c1, c2], 1000)

echo picked[1]
```

```console
from c2
```

It returns `[channel, value]`, so you can tell where the value came from by
comparing the first element against the channels you passed in. The second
argument is a timeout in **seconds**, like every other timeout in this
module — see [Timeouts Are in Seconds](#timeouts-are-in-seconds), because
it is the opposite of what the `net` module does.

This is the shape for a consumer fed by more than one producer — work on
one channel, shutdown signals on another — without polling either.

## Broadcast

A channel delivers each value to exactly one receiver. A broadcast delivers
each value to **every** subscriber:

```zuri
import isolate

var bus = isolate.broadcast(4)

var a = bus.subscribe()
var b = bus.subscribe()

bus.send('tick')

echo a.recv()
echo b.recv()
echo bus.subscriber_count()

bus.close()
```

```console
tick
tick
2
```

`subscribe()` hands back an ordinary `Channel`, so everything in the
channel table applies to it: `recv()`, `try_recv()`, `length()`, and the
same backpressure.

### A Subscriber Only Sees What Comes After It

There is no replay. A subscriber that arrives late has missed everything
sent before it subscribed:

```zuri
import isolate

var bus = isolate.broadcast(4)
var early = bus.subscribe()

bus.send('first')

var late = bus.subscribe()

bus.send('second')

echo 'early: ${early.recv()}, ${early.recv()}'
echo 'late: ${late.try_recv()}'
```

```console
early: first, second
late: second
```

This matters when subscribers are set up concurrently with the producer.
Subscribe everything *before* anything is sent, or accept that a consumer
starting later begins mid-stream.

### Unsubscribing, and Sending Into the Void

`unsubscribe(channel)` drops one subscriber. Sending with none at all is
not an error — the value is simply discarded:

```zuri
import isolate

var bus = isolate.broadcast(4)
var sub = bus.subscribe()

bus.unsubscribe(sub)

echo bus.subscriber_count()
echo bus.send('nobody is listening')
echo sub.try_recv()
```

```console
0
nil
nil
```

A dropped subscriber stops receiving new values immediately, and its
channel keeps whatever was already queued in it:

```zuri
import isolate

var bus = isolate.broadcast(4)
var sub = bus.subscribe()

bus.send('queued before unsubscribe')
bus.unsubscribe(sub)

echo sub.try_recv()
```

```console
queued before unsubscribe
```

So unsubscribing is not a way to discard what a consumer has not read yet;
it only stops the flow.

### Closing

`close()` closes the bus and every channel it handed out. Subscribers drain
what they have and then receive `nil`; `send()` raises:

```zuri
import isolate

var bus = isolate.broadcast(4)
var sub = bus.subscribe()

bus.close()

echo bus.is_closed()
echo sub.try_recv()

catch {
  bus.send('too late')
} as e {
  echo e.type
}
```

```console
true
nil
IsolateError
```

### Backpressure Applies Per Subscriber

The capacity you give `broadcast()` is the capacity of *each* subscriber's
channel. A subscriber that stops reading fills its own buffer, and once it
is full the bus blocks on `send()` — one slow consumer holds up the
producer and therefore everyone.

When a slow consumer must not be allowed to do that, give the bus a larger
capacity, or have that consumer read into its own queue and fall behind on
its own time.

### Channel or Broadcast?

Use a **channel** when the work should be done once, by whichever worker is
free. Use a **broadcast** when every consumer needs to see every event.
Shutdown signals, configuration changes and progress events are broadcasts;
jobs are channels.

## Scopes

A scope owns its children and does not return until all of them have
finished:

```zuri
import isolate

def square(n) {
  return n * n
}

var result = isolate.scope(@(s) {
  s.spawn(square, 3)
  s.spawn(square, 4)

  return s.children().map(@(child) => child.join())
})

echo result
```

```console
[9, 16]
```

`scope(body)` calls `body` with a scope object, waits for everything
spawned through it, and returns whatever the body returned.

### It Waits Whether or Not You Join

Joining inside the body is how you collect results. It is not how the
waiting happens — that is the scope's job either way:

```zuri
import isolate

def square(n) {
  return n * n
}

echo isolate.scope(@(s) {
  s.spawn(square, 3)
  s.spawn_named('four', square, 4)

  return s.children().length()
})
```

```console
2
```

Neither child was joined, and the scope still did not return until both had
finished. `children()` gives you the `Isolate` objects in spawn order, and
`s.spawn_named()` works exactly like the module-level one.

### A Failure Stops the Group

This is the real reason to use a scope. A bare `spawn()` whose result is
never joined swallows its own error in silence; a scope raises:

```zuri
import isolate

def ok(n) {
  return n
}

def boom() {
  raise ValueError('worker failed')
}

catch {
  isolate.scope(@(s) {
    s.spawn(ok, 1)
    s.spawn(boom)

    return 'never returned'
  })
} as e {
  echo '${e.type}: ${e.message.lines()[0]}'
}
```

```console
IsolateError: ValueError: worker failed
```

The body's return value is discarded when a child failed, and the failure
comes out of `scope()` itself. Reach for a scope whenever a piece of work
fans out and must be complete — and correct — before the next step begins.

## Waiting on Several Tasks

Three helpers cover the shapes that come up, and they return different
things:

```zuri
import isolate

def square(n) {
  return n * n
}

var tasks = [1, 2, 3].map(@(n) => isolate.spawn(square, n))

echo isolate.wait_all(tasks)
echo typeof(isolate.wait_any([isolate.spawn(square, 9)]))
echo isolate.map(square, [4, 5])
```

```console
[1, 4, 9]
Isolate
[16, 25]
```

**`wait_all(tasks, timeout)`** returns the **results**, in the order the
tasks were given, not the order they finished.

**`wait_any(tasks, timeout)`** returns the **`Isolate`** that finished
first, not its result — you still call `join()` on it. That is what lets
you tell *which* one won.

**`map(fn, items, timeout)`** spawns one task per item and collects the
results, which is `wait_all` with the spawning done for you.

All three raise `IsolateTimeoutError` if the timeout passes, and all three
propagate a worker's failure:

```zuri
import isolate

def halve(n) {
  if n == 0 {
    raise ValueError('cannot halve zero')
  }

  return n / 2
}

echo isolate.map(halve, [2, 4, 6])

catch {
  isolate.map(halve, [2, 0, 6])
} as e {
  echo '${e.type}: ${e.message.lines()[0]}'
}
```

```console
[1, 2, 3]
IsolateError: ValueError: cannot halve zero
```

The whole call fails on the first failing element. When individual failures
are acceptable, spawn and join yourself with a `catch` around each `join()`,
as the worked example below does.

## Isolates Can Spawn Isolates

There is no restriction on nesting. A worker may spawn its own tasks and
join them, and they run on the same pool:

<span class="filename">Filename: work.zu</span>

```zuri,ignore
import isolate

def double(n) {
  return n * 2
}

def nested(n) {
  return isolate.spawn(double, n).join()
}
```

```console
$ zuri run main.zu
6
```

This is worth knowing mostly as a warning, which the next section covers:
nested tasks that block on each other are the fastest way to exhaust the
pool.

## The Pool

Isolates do not each get a thread of their own. They run on a fixed pool of
OS threads, sized to the machine's core count by default:

```zuri
import isolate

echo isolate.cpu_count() > 0
echo isolate.pool_size() == isolate.cpu_count()
```

```console
true
true
```

### Sizing It

`configure(threads)` changes the size, and it only works **before the pool
exists** — which is to say, before the first spawn. It reports whether it
did anything:

```zuri
import isolate

echo isolate.configure(7)
echo isolate.pool_size()
```

```console
true
7
```

Call it after a spawn and it returns `false` and changes nothing. It does
not raise, so a `configure()` buried below some initialisation that already
spawned will silently do nothing — put it at the very top of the entry
file.

The size is a ceiling, not a head count. A thread is started only when an
isolate is waiting and every thread already started is busy, so sizing the
pool generously for a burst costs nothing while the burst is not happening.
`started_count()` says how many threads the pool has started so far:

```zuri
import isolate

isolate.configure(8)

echo isolate.started_count()
echo isolate.spawn(@() => 21 * 2).join()
echo isolate.started_count()
```

```console
0
42
1
```

### Why the Size Matters More Than It Looks

A pool sized to your core count can **deadlock**, and the failure looks
like a hang rather than an error.

The mechanism: a task that blocks — on `join()`, on `recv()`, on a socket
read — occupies its thread while it waits. If every thread is occupied by a
task waiting for work that has no free thread to run on, nothing can ever
progress.

That happens most easily with three patterns:

- a server and a client that talk to each other in one process;
- nested spawns where the parent joins the child;
- a pipeline with more stages than threads.

**Size the pool for the number of tasks that will be blocked at once, not
for the number of cores.** Threads that are blocked are not competing for
CPU, so over-provisioning costs little; under-provisioning costs
everything.

### Watching It

```zuri
import isolate

echo isolate.active_count() >= 0
echo isolate.queued_count() >= 0
echo isolate.is_shutdown()
```

```console
true
true
false
```

`active_count()` is how many tasks are running; `queued_count()` is how
many are waiting for a thread. A queued count that only grows is the
signature of a starved pool.

`shutdown(timeout)` drains the pool and stops it. Most programs never call
it; it is there for a long-lived process that wants to release its threads
without exiting.

## The Errors

Three error types come out of this module, and they mean different things:

| Error | Raised when |
| --- | --- |
| `IsolateError` | a worker raised, or an operation is invalid — sending on a closed channel |
| `IsolateTimeoutError` | a timeout passed before the operation completed |
| `IsolateCancelledError` | a blocking call was interrupted by `cancel()` |

All three inherit from `Error`, so `catch` on its own catches every one and
`instance_of()` sorts them.

The distinction matters because they call for different responses. A
timeout usually means retry or give up. A cancellation means shut down
quietly. An `IsolateError` carrying a worker's failure means something in
your own code went wrong, and the message names it.

## A Worked Example

Fan out a computation, collect the results, and report a failure without
losing the rest:

```zuri
import isolate

def classify(n) {
  if n < 0 {
    raise ValueError('negative input: ${n}')
  }

  if n < 2 {
    return 'small'
  }

  var divisor = 2

  while divisor * divisor <= n {
    if n % divisor == 0 {
      return 'composite'
    }

    divisor++
  }

  return 'prime'
}

def classify_all(numbers) {
  var tasks = numbers.map(@(n) => [n, isolate.spawn(classify, n)])
  var results = {}

  for pair in tasks {
    catch {
      results[pair[0]] = pair[1].join()
    } as e {
      results[pair[0]] = 'failed'
    }
  }

  return results
}

echo classify_all([1, 7, 9, -3, 97])
```

```console
{1: small, 7: prime, 9: composite, -3: failed, 97: prime}
```

Three things are doing the work there.

Every task is spawned **before** any is joined. Spawning in one pass and
joining in another is what makes the work overlap; joining inside the first
loop would run them one after another and buy nothing.

The number is carried alongside its task, because the results come back in
whatever order the pool produces them and a bare list of results would lose
which input each belonged to.

And the `catch` is around one `join()`, so the one bad input becomes one
`failed` entry rather than ending the batch. Move it outside the loop and
the first failure takes the other four with it.

## Choosing a Shape

**Fan out, collect results.** `isolate.map()`, or `scope()` when a failure
must stop the group.

**A pipeline.** Channels between stages, each stage an isolate, each channel
bounded so backpressure propagates all the way back to the source.

**An event fan-out.** A broadcast, one subscriber per consumer.

**A server.** `http.serve()` builds the whole pattern for you: one isolate
per worker, a bounded backlog, connections handed out as they arrive.
[Chapter 15](ch15-00-http.md) covers it.

**Nothing at all.** Concurrency costs a copy at every boundary and a great
deal of care at every join. A single-threaded loop that finishes in time is
the better program.
