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

**The function you spawn must not capture a module.** A module is one of
the few things that cannot cross the boundary, and a function that
references an imported name captures the module it came from.

`square` above is fine: it touches nothing but its own argument. This is
not:

```zuri,ignore
import isolate
import net

def serve(channel) {
  var listener = net.TcpStream()
  # ...
}

isolate.spawn(serve, channel)
```

```console
Unhandled IsolateError: cannot send a module across isolates
```

`serve` refers to `net`, which is a module-level binding in *this* file, so
spawning it tries to send `net` along with it.

There are two ways round it, and both are worth knowing.

**Put the worker in its own module.** Inside `work.zu`, `net` is resolved
when the worker runs, in the isolate's own namespace, rather than captured
from yours:

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

This is the shape the rest of this book uses, and it is good structure
anyway: worker code tends to want its own file.

**Or import inside the function.** An `import` in the body runs in the
isolate, so nothing is captured:

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

That is the compact option for a short worker. Use the separate module once
the worker is more than a few lines.

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

## Checking Without Waiting

```zuri
import isolate

def slow_sum(n) {
  var total = 0

  iter var i = 0; i < n; i++ {
    total += i
  }

  return total
}

var task = isolate.spawn(slow_sum, 100)

echo task.join()
echo task.status()
echo task.is_done()
```

```console
4950
done
true
```

`status()` is `pending` before the work finishes and `done` after.
`try_join()` returns the result if it is ready and `nil` if it is not, so a
loop can do something else while waiting. `cancel()` requests cancellation,
and `is_cancelled()` is what a long-running worker polls so it can stop
cooperatively — nothing is killed from outside.

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

Sending on a closed channel raises. Closing one twice does not.

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
argument is a timeout in milliseconds; on a timeout you get `nil`.

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

`subscribe()` hands back an ordinary `Channel`, so everything in the table
above applies to it. `unsubscribe(channel)` drops one, and a subscriber
that stops reading applies backpressure to the whole bus once its own
buffer fills.

Use a channel when the work should be done once by whoever is free. Use a
broadcast when every consumer needs to see every event.

## Scopes

A scope owns its children and will not return until all of them have
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

If any child fails, the scope raises rather than letting the failure vanish
into a task nobody joined. That is the real reason to use it: a bare
`spawn()` whose result is never joined will swallow its own error in
silence.

Reach for `scope()` whenever a piece of work fans out and must be complete
before the next step begins.

## The Pool

Isolates run on a fixed pool of OS threads, sized to the machine's core
count by default:

```zuri
import isolate

echo isolate.cpu_count() > 0
echo isolate.pool_size() > 0
```

```console
true
true
```

`configure(threads)` changes the size, and it has to be called **before the
first spawn**; the pool is fixed once it exists:

```zuri,ignore
isolate.configure(16)
```

That matters more than it sounds. If your program runs servers, worker
pools and clients that talk to each other inside one process, a pool sized
to your core count can deadlock: every thread ends up blocked waiting on
work that has no free thread to run on. A starved pool looks like a hang,
not an error. Size the pool for the number of tasks that will be **blocked
at once**, not for the number of cores.

`active_count()`, `queued_count()`, `is_shutdown()` and `shutdown(timeout)`
manage it at runtime.

## Task Density

A spawned task is not a thread. It is a small descriptor holding the callee
and a snapshot of its arguments, pushed onto a queue that the pool drains.
Queueing a hundred thousand tasks is a reasonable thing to do; what grows
with the work is whatever the arguments themselves have to carry across the
boundary, since every one of them is copied.

That is the cost model to keep in mind. Spawning is cheap. Handing an
isolate a large list is not, because the list is copied. When a worker needs
a lot of data, it is usually better to hand it a *description* of the work —
a path, a range of indices, a query — and let it do its own reading.

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
