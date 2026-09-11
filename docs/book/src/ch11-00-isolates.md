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
import .work

var task = isolate.spawn(work.square, 12)
echo task.join()
```

```console
144
```

`spawn(fn, ...args)` queues the call and returns an `Isolate` handle
immediately. `join()` waits for it and gives you the result. Calling
`join()` again returns the same value; it is not a one-shot.

Note the `import .work` and `work.square`. **The function you spawn has to
be resolvable by module binding.** A plain `def` in the file doing the
spawning works, as does an inline anonymous function. What does not work is
a closure that captured an imported module namespace, because a module
cannot cross the boundary. In practice, that means worker functions live in
their own module, which is good structure anyway.

## Running Many at Once

Spawn a list, join a list:

```zuri
var tasks = [1, 2, 3, 4].map(@(n) => isolate.spawn(work.square, n))
echo tasks.map(@(task) => task.join())
```

```console
[1, 4, 9, 16]
```

`isolate.map()` does exactly that, in one call:

```zuri
echo isolate.map(work.square, [1, 2, 3, 4, 5])
```

```console
[1, 4, 9, 16, 25]
```

`wait_all(tasks, timeout)` waits for every task; `wait_any(tasks,
timeout)` returns as soon as one finishes.

## Checking Without Waiting

```zuri
var task = isolate.spawn(work.slow_sum, 100)

echo task.status()
echo task.join()
echo task.status()
```

```console
pending
4950
done
```

`is_done()` is the boolean form. `try_join()` returns the result if it is
ready and `nil` if it is not. `cancel()` requests cancellation, and
`is_cancelled()` is what a long-running worker should poll so it can stop
cooperatively.

## Errors Cross the Boundary

An uncaught error inside an isolate surfaces at the `join()` as an
`IsolateError`, carrying the original error's message and the stack trace
from inside the worker:

```zuri
catch {
  isolate.spawn(work.boom).join()
} as e {
  echo e.type
  echo e.message
}
```

```console
IsolateError
ValueError: worker failed
  --> /path/to/work.zu:14
...
```

Nothing is lost. You get the failure where you can do something about it,
with enough information to find it.

## Channels

A channel is a bounded queue that crosses isolate boundaries:

```zuri
import isolate
import .work

var channel = isolate.channel(4)
var producer = isolate.spawn(work.producer, channel, 3)

while true {
  var value = channel.recv()

  if value == nil and channel.is_closed() {
    break
  }

  echo value
}

producer.join()
```

<span class="filename">Filename: work.zu</span>

```zuri
def producer(channel, count) {
  iter var i = 0; i < count; i++ {
    channel.send(i)
  }

  channel.close()
}
```

```console
0
1
2
```

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

Sending on a closed channel raises.

## Selecting Across Channels

`select()` waits on several channels and tells you which one produced:

```zuri
var picked = isolate.select([c1, c2], 1000)
echo picked
```

```console
[<instance of Channel>, from c2]
```

It returns `[channel, value]`, so you know where the value came from. The
second argument is a timeout in milliseconds.

## Broadcast

A channel delivers each value to exactly one receiver. A broadcast delivers
each value to **every** subscriber:

```zuri
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

`subscribe()` hands back a `Channel`, so everything above applies.
`unsubscribe(channel)` drops one.

## Scopes

A scope owns its children and will not return until they are all finished:

```zuri
var result = isolate.scope(@(s) {
  s.spawn(work.square, 3)
  s.spawn(work.square, 4)

  return s.children().map(@(child) => child.join())
})

echo result
```

```console
[9, 16]
```

If any child fails, the scope raises rather than letting a failure vanish
into a task nobody joined. This is the shape to reach for whenever a piece
of work fans out and has to be complete before you move on.

## The Pool

Isolates run on a fixed pool of OS threads, sized to the machine's core
count by default:

```zuri
echo isolate.cpu_count()
echo isolate.pool_size()
```

`configure(threads)` changes the size, and it has to be called **before the
first spawn**; the pool is fixed once it exists:

```zuri
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
The fixed cost of one is a few hundred bytes, so queueing a hundred thousand
tasks is a reasonable thing to do; what scales with the work is whatever the
arguments themselves have to carry across the boundary.

## Choosing a Shape

**Fan out, collect results.** `isolate.map()`, or `scope()` when you need
the failure semantics.

**A pipeline.** Channels between stages, each stage an isolate, each channel
bounded so backpressure propagates.

**An event fan-out.** A broadcast, one subscriber per consumer.

**A server.** `http.serve()` builds the whole pattern for you: one isolate
per worker, a bounded backlog, connections handed out as they arrive.
[Chapter 12](ch12-00-networking.md) covers it.
