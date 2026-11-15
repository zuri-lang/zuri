# Concurrency with Isolates

Zuri's concurrency model rests on one rule: **nothing is shared.**

An isolate is an operating system thread that runs its own complete VM. It
has its own heap, garbage collector, register stack and module namespace.
Two isolates never hold a pointer to the same object, so there are no data
races to think about, no locks to take, and no pauses in one thread caused
by memory allocation in another.

Values are **copied** when they're passed between isolates. The copy is
exact: cycles are preserved, objects that appear more than once in the same
value are still shared within the copy, and class instances arrive as
instances of the same class.

## Spawning an Isolate

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

`spawn(fn, ...args)` queues the call and immediately returns an `Isolate`
handle. `join()` waits for the isolate to finish and returns its result. You
can call `join()` more than once, and it returns the same value each time.

### What You Can Spawn

You can spawn any function: a top-level `def`, a helper declared inside
another function, an anonymous function or a bound method. Anything the
function captures goes with it, including the modules it uses:

```zuri
import isolate
import json

def encode_all(items) {
  return items.map(@(item) => json.encode(item)).length()
}

echo isolate.spawn(encode_all, [
  { a: 1 },
  { b: 2 },
]).join()
```

```console
2
```

Modules aren't copied the way lists are. Instead, the isolate loads the same
module itself and uses its own copy. That works because the top level of a
module consists of declarations that run once, and the result is cached. Be
careful with a module that depends on live state in the current process,
since the isolate gets a fresh copy of the module, not yours.

There are still two things you can't spawn. You can't use **a native
function or a class** as the spawn target directly, so wrap it in a `def`.
And **a resource handle**, such as an open socket or file, is *moved* rather
than copied, so the side that passed it no longer has it.

Once a worker grows beyond a few lines, it's better to put it in a module of
its own:

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

That's the structure the rest of this book uses. A worker referred to by
name is looked up in the isolate's own namespace, so nothing about it needs
to be reconstructed.

An `import` inside the function body also works, and runs inside the
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

Arguments are copied, not shared. If an isolate changes a list it was given,
it changes its own copy:

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

The isolate saw two elements, while the caller still has one. That's the
whole concurrency model in a single example: you can't write a data race,
because nothing is shared.

## Running Several Isolates at Once

Spawn a list of isolates, then join them:

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

`isolate.map()` does the same thing in a single call:

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

`wait_all(tasks, timeout)` waits for all of the tasks to finish, and
`wait_any(tasks, timeout)` returns as soon as one of them finishes.

## Task Status

A spawned task is in one of two states, and there are several methods that
let you check on it without blocking.

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

| Method | Returns | Blocks? |
| --- | --- | --- |
| `join(timeout)` | The result | Yes |
| `try_join()` | The result, or `nil` if it isn't ready | No |
| `is_done()` | Whether the task has finished | No |
| `status()` | `'pending'` or `'done'` | No |
| `name()` | The task's name, or `nil` | No |

If a task's own result could be `nil`, a `nil` from `try_join()` is
ambiguous, so check `is_done()` as well when it matters.

You can call `join()` more than once. The second call returns the same value
immediately.

### Naming Tasks

`spawn()` creates an unnamed task. `spawn_named()` gives the task a name,
which appears in diagnostics:

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

Give a name to any task that runs for a long time. When a program gets
stuck, it's much easier to find the problem if each task can tell you what
it is.

## Timeouts Are in Seconds

**All timeouts in the `isolate` module are measured in seconds**, and
fractions are allowed:

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
`wait_all()` and `shutdown()`.

This is worth emphasizing, because **the `net` module uses milliseconds**
for its timeouts. `socket.set_read_timeout(5000)` is five seconds, but
`channel.recv(5000)` is an hour and twenty minutes. It's common to use both
modules in the same program, and mixing them up doesn't produce an error. A
timeout that never fires just looks like the program has hung.

If you leave out the timeout, the call waits forever. That's a good default
when the other side is code you control, and a bad one when it isn't.

## Cancelling a Task

`cancel()` **asks** a task to stop, but it doesn't force it to.

What happens next depends on what the task is doing:

**If it's blocked in a channel operation, a `join()`, a `wait_*` call or a
`select()`**, the call is interrupted within about 50 ms and raises an
`IsolateCancelledError`.

**If it's running ordinary code**, nothing is interrupted. The task's
function has to check for cancellation and return by itself:

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

`isolate.is_cancelled()` is the module-level function that a worker calls to
check its own status. `task.is_cancelled()` is the method that the code that
*spawned* the task calls to find out whether cancellation was requested. It
returns `true` from the moment `cancel()` is called, whether or not the task
noticed:

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

That output isn't a contradiction. The worker in this example doesn't check
for cancellation, so it finished normally. `is_cancelled()` tells you that
cancellation was requested, not what the task did about it. A loop that
never checks for cancellation can't be stopped.

Put the check at the point where the loop starts its next pass, and keep it
cheap. Checking once per pass of an outer loop is usually enough, and
checking inside the innermost calculation isn't worth the cost.

## Passing Values Between Isolates

Arguments, results and messages sent over channels all cross between
isolates in the same way: **everything is copied**. Nothing is shared, and
no reference survives the trip.

### Copies Are Exact

A copy isn't a shallow snapshot. Cycles are kept, objects that appear more
than once in a value are still shared within the copy, and class instances
arrive as instances of the same class, with their methods:

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

Look at the last line. `pair` contained the same list twice, and on the
other side it still does: appending through `back[0]` is visible through
`back[1]`. The copy kept the *structure* of the sharing as well as the
values.

### What Can't Be Passed

A few kinds of value are tied to the isolate that created them. Trying to
send one raises an error instead of quietly producing something broken:

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

The error message lists what *can* be passed, which is the more useful part.
The case you'll run into most is an open file. A file handle refers to a
position in a file descriptor owned by the current process, and there's no
way to copy that. Send the path instead, and let the worker open the file
itself.

### Costs

Spawning is cheap. A task is a small record of the function and a copy of
its arguments, pushed onto a queue that the thread pool works through, so
queueing a hundred thousand tasks is perfectly reasonable.

What isn't free is the copying. Passing a large list to an isolate copies
the whole list, once for every spawn. When a worker needs a lot of data,
give it a *description* of the work instead, such as a path, a range of
indexes or a query, and let it read the data itself:

```zuri,ignore
# Copies the file's contents into every worker.
isolate.map(work.process, files.map(@(p) => file(p).read()))

# Copies a short path into every worker instead.
isolate.map(work.read_and_process, files)
```

## Errors in Isolates

An uncaught error inside an isolate is raised at the `join()` call as an
`IsolateError`, which carries the original error's message and the stack
trace from inside the worker:

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

Nothing is lost. You get the failure in a place where you can deal with it,
along with enough information to track it down. Notice the form of the
message: the `IsolateError` wraps the worker's own error instead of
replacing it, so `e.message` still names the `ValueError` and the line it
came from.

If nobody joins a task, an error in that task has nowhere to go. `scope()`,
described below, exists to prevent that.

## Channels

A channel is a queue with a fixed capacity that can be shared between
isolates. One side sends values and the other receives them, and like
everything else, the values are copied on the way:

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

Read the loop condition carefully, because it's easy to get wrong. `recv()`
returns `nil` both when someone sent a `nil` and when the channel is closed
and empty, so the end of the stream is when you get `nil` **and**
`is_closed()` is true. If you only checked for `nil`, the loop would stop
early when a real `nil` was sent. If you only checked `is_closed()`, it
would stop before the queue was empty.

`channel(capacity)` limits the size of the queue, which gives you
backpressure automatically. A producer that sends faster than its consumer
can receive blocks on `send()`, instead of letting the queue grow without
limit.

| Method | Behavior |
| --- | --- |
| `send(value, timeout)` | Blocks while the channel is full |
| `recv(timeout)` | Blocks while the channel is empty |
| `try_recv()` | Returns immediately, with `nil` if the channel is empty |
| `close()` | Stops further sends. Pending values can still be received, after which `recv()` returns `nil` |
| `is_closed()` | Whether the channel has been closed |
| `length()` | How many values are waiting |

You can see all of these behaviors from a single isolate, which makes it
easy to understand how a channel works before you add a second one:

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

Look at the last four lines together. After `close()`, the queue still
**drains**: `recv()` returned the buffered `b` before it started returning
`nil`. Once a closed channel is empty, it keeps returning `nil` instead of
raising an error. Only `send()` raises.

Closing a channel twice does no harm. `try_recv()` on an empty channel
returns `nil` immediately and never blocks.

## Waiting on Several Channels

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

It returns `[channel, value]`, so you can find out where the value came from
by comparing the first element with the channels you passed in. The second
argument is a timeout in **seconds**, like every other timeout in this
module. See [Timeouts Are in Seconds](#timeouts-are-in-seconds), because the
`net` module does the opposite.

Use this for a consumer that gets input from more than one producer, such as
work on one channel and shutdown signals on another, without polling either
one.

## Broadcasting

A channel delivers each value to a single receiver. A broadcast delivers
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

`subscribe()` returns an ordinary `Channel`, so everything in the channel
table applies to it, including `recv()`, `try_recv()`, `length()` and the
same backpressure.

### Subscribers Only See Later Values

Values aren't replayed. A subscriber that joins late misses everything that
was sent before it subscribed:

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

This matters when subscribers are being set up at the same time as the
producer starts. Either subscribe everything *before* anything is sent, or
accept that a consumer that starts later will begin partway through the
stream.

### Unsubscribing, and Sending With No Subscribers

`unsubscribe(channel)` removes one subscriber. Sending when there are no
subscribers isn't an error, and the value is simply thrown away:

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

A removed subscriber stops receiving new values right away, but its channel
keeps any values that were already queued:

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

So unsubscribing doesn't throw away what a consumer hasn't read yet. It only
stops new values from arriving.

### Closing a Broadcast

`close()` closes the broadcast and every channel it handed out. Subscribers
receive what's left in their channels and then get `nil`, and `send()`
raises an error:

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

### Backpressure Applies to Each Subscriber

The capacity you give `broadcast()` is the capacity of *each* subscriber's
channel. If a subscriber stops reading, its own buffer fills up, and once
it's full, `send()` blocks. So one slow consumer holds up the producer, and
therefore every other subscriber too.

If you can't let a slow consumer do that, give the broadcast a larger
capacity, or have the slow consumer copy values into its own queue so that
only it falls behind.

### Channel or Broadcast?

Use a **channel** when each piece of work should be done once, by whichever
worker is free. Use a **broadcast** when every consumer needs to see every
event. Shutdown signals, configuration changes and progress updates are
broadcasts, and jobs are channels.

## Scopes

A scope owns the tasks spawned through it, and doesn't return until all of
them have finished:

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

`scope(body)` calls `body` with a scope object, waits for everything spawned
through that object, and returns whatever the body returned.

### Scopes Wait Whether or Not You Join

Joining inside the body is how you collect results, but it isn't what makes
the scope wait. The scope waits either way:

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

Neither task was joined, and the scope still didn't return until both had
finished. `children()` returns the `Isolate` objects in the order they were
spawned, and `s.spawn_named()` works just like the module-level function.

### One Failure Stops the Whole Group

This is the main reason to use a scope. If you never join the result of a
plain `spawn()`, any error it raises is silently lost. A scope raises it
instead:

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

When a task fails, the body's return value is discarded and the error is
raised by `scope()` itself. Use a scope whenever work is split across
several tasks and all of them have to finish, and succeed, before the next
step can start.

## Waiting on Several Tasks

There are three helper functions for waiting on tasks, and they return
different things:

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

**`wait_all(tasks, timeout)`** returns the **results**, in the same order as
the tasks you passed, not the order in which they finished.

**`wait_any(tasks, timeout)`** returns the **`Isolate`** that finished
first, not its result, so you still need to call `join()` on it. That way
you can tell *which* task finished first.

**`map(fn, items, timeout)`** spawns one task for each item and collects the
results. It's `wait_all` with the spawning done for you.

All three raise an `IsolateTimeoutError` if the timeout expires, and all
three pass on a failure from any of the workers:

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

The whole call fails as soon as one element fails. If it's acceptable for
individual items to fail, spawn and join the tasks yourself and wrap each
`join()` in a `catch`, as the example at the end of this chapter does.

## Spawning From Inside an Isolate

There's no restriction on nesting. A worker can spawn its own tasks and join
them, and they run on the same thread pool:

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

This is mostly worth knowing as a warning, which the next section explains:
nested tasks that wait on each other are the quickest way to run out of
threads.

## The Thread Pool

Each isolate doesn't get a thread of its own. Isolates run on a fixed pool
of operating system threads, which by default has one thread per CPU core:

```zuri
import isolate

echo isolate.cpu_count() > 0
echo isolate.pool_size() == isolate.cpu_count()
```

```console
true
true
```

### Setting the Pool Size

`configure(threads)` changes the size of the pool, but it only works
**before the pool has been created**, which means before the first spawn. It
returns whether it changed anything:

```zuri
import isolate

echo isolate.configure(7)
echo isolate.pool_size()
```

```console
true
7
```

If you call it after a spawn, it returns `false` and does nothing. It
doesn't raise an error, so a `configure()` call placed after some
initialization code that already spawned a task won't have any effect. Put
it at the very top of your entry file.

The size is a maximum, not a fixed number of threads. A new thread only
starts when an isolate is waiting to run and every existing thread is busy,
so a generous pool size for occasional bursts costs nothing when there's no
burst. `started_count()` tells you how many threads the pool has started so
far:

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

### Why the Pool Size Matters

A pool with one thread per core can **deadlock**, and when it does, the
program just hangs without reporting an error.

Here's how it happens. A task that blocks, whether on `join()`, `recv()` or
a socket read, keeps its thread busy while it waits. If every thread is held
by a task that's waiting for work that has no free thread to run on, nothing
can make progress.

This is most likely with these three patterns:

- A server and a client that talk to each other in the same process
- Nested spawns where the parent task joins its child
- A pipeline with more stages than there are threads

**Size the pool for the number of tasks that might be blocked at the same
time, not for the number of cores.** Blocked threads don't compete for the
CPU, so having too many costs very little, while having too few can stop the
program completely.

### Monitoring the Pool

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

`active_count()` returns how many tasks are running, and `queued_count()`
returns how many are waiting for a thread. If the queued count keeps
growing, the pool doesn't have enough threads.

`shutdown(timeout)` lets the pool finish its work and then stops it. Most
programs never need to call it. It's there for long-running processes that
want to release their threads without exiting.

### Memory Use While Waiting

A pool thread keeps its isolate's heap from one task to the next, and
garbage collection runs as the program allocates memory. A thread that's
waiting doesn't allocate anything, so waiting threads collect garbage on
their own. Once a thread has waited for one second, whether for its next
task or inside `recv()`, `select()`, `join()`, `wait_any()` or `wait_all()`,
it collects its garbage, and the memory is returned to the operating system.
The main thread does the same in those calls. As a result, a task that built
and then discarded a large data structure leaves the process at its normal
working size rather than at its peak.

A wait of one second or less returns before that happens, and a heap that
has grown by less than 4 MB since its last collection of this kind is left
alone, so a loop that makes many short waits doesn't pay for it.

## Error Types

This module raises three types of error, and each one means something
different:

| Error | Raised when |
| --- | --- |
| `IsolateError` | A worker raised an error or panicked, or an operation was invalid, such as sending on a closed channel. A panic that escapes from a Rust library called through `ffi` ends the process instead; see [Rust in Depth](ch26-00-ffi.md#rust-in-depth). |
| `IsolateTimeoutError` | A timeout expired before the operation finished |
| `IsolateCancelledError` | A blocking call was interrupted by `cancel()` |

All three inherit from `Error`, so a plain `catch` catches all of them, and
you can use `instance_of()` to tell them apart.

The difference matters because each one calls for a different response. A
timeout usually means you should retry or give up. A cancellation means you
should shut down quietly. And an `IsolateError` that carries a worker's
failure means something went wrong in your own code, and the message tells
you what.

## Example: Processing a Batch

This example spreads a computation across several tasks, collects the
results, and reports a failure without losing the other results:

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

Three things make this work.

Every task is spawned **before** any of them are joined. Spawning all of
them in one loop and joining them in another is what lets the work run in
parallel. If you joined inside the first loop, the tasks would run one after
another and you'd gain nothing.

Each number is stored alongside its task. The pool can finish the tasks in
any order, and a plain list of results wouldn't tell you which input each
result belongs to.

And the `catch` surrounds a single `join()`, so the one bad input produces
one `failed` entry instead of ending the whole batch. If you moved it
outside the loop, the first failure would take the other four results with
it.

## Choosing an Approach

**Splitting work and collecting the results:** use `isolate.map()`, or
`scope()` when one failure should stop the whole group.

**A pipeline:** connect the stages with channels, run each stage in its own
isolate, and give every channel a capacity so that backpressure reaches all
the way back to the source.

**Sending events to several consumers:** use a broadcast, with one
subscriber for each consumer.

**A server:** `http.serve()` sets all of this up for you, with one isolate
per worker, a limited backlog, and connections handed out as they arrive.
[Chapter 15](ch15-00-http.md) covers it.

**No concurrency at all:** concurrency costs a copy every time a value
crosses between isolates, and a lot of care every time you join. If a
single-threaded loop finishes in time, it's the better program.
