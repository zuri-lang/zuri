# Dictionaries

A dictionary maps keys to values and remembers the order you inserted
them.

```zuri
var user = { name: 'Ada', age: 36 }
var empty = {}
```

A bare word key is a string, so `{ name: ... }` and `{ 'name': ... }` are
the same. Keys may also be numbers:

```zuri
echo { 1: 'one', 2.5: 'two-point-five' }
```

```console
{1: one, 2.5: two-point-five}
```

A key that is a bare identifier is always taken as that literal name. To
compute a key, make it an expression the parser cannot mistake for a name,
which in practice means wrapping it in parentheses:

```zuri
var key = 'dynamic'

echo { (key): 1 }
echo { 'a' + 'b': 2 }
```

```console
{dynamic: 1}
{ab: 2}
```

Assigning through brackets is usually clearer:

```zuri
var d = {}
d[key] = 3
echo d
```

```console
{dynamic: 3}
```

### Shorthand Keys

When a key and the variable holding its value have the same name, write it
once:

```zuri
var name = 'Ada'
var age = 36

echo { name, age }
```

```console
{name: Ada, age: 36}
```

`{ name, age }` means exactly `{ name: name, age: age }`. This comes up
constantly in functions that build a result out of locals they have just
computed, and Zuri code uses the shorthand whenever the names line up.

### Nesting

A value may be another dictionary, or a list, to any depth. Access chains:

```zuri
var config = {
  server: { host: 'localhost', port: 8080 },
  tags: ['web', 'internal'],
}

echo config.server.host
echo config['server']['port']
echo config.tags[0]
```

```console
localhost
8080
web
```

Dot and bracket access mix freely, because they are the same operation.
Use the dot when the key is a fixed identifier and brackets when it is
computed, contains punctuation, or is a number.

## Reading

Dot and bracket access are the same operation:

```zuri
var user = { name: 'Ada', age: 36 }

echo user.name
echo user['name']
echo user.length()
echo user.is_empty()
echo user.keys()
echo user.values()
```

```console
Ada
Ada
2
false
[name, age]
[Ada, 36]
```

Reading a key that is not there raises a `PropertyError`. `get()` is the
safe form:

```zuri
echo user.get('name')
echo user.get('nope', 'default')
echo user.contains('age')
```

```console
Ada
default
true
```

With no fallback, `get()` on a missing key returns `nil`.

## Writing

```zuri
var user = { name: 'Ada' }

user.set('city', 'London')
user.age = 36
user['country'] = 'UK'

echo user
```

```console
{name: Ada, city: London, age: 36, country: UK}
```

Assignment through a dot, a bracket or `set()` all do the same thing, and
all three create the key if it does not exist. `add()` is a synonym for
`set()`.

To take a key back out:

```zuri
echo user.remove('city')
echo user.contains('city')
```

```console
London
false
```

`remove()` returns the value that was there.

`clear()` empties the dictionary.

## Merging

```zuri
var defaults = { host: 'localhost', port: 8080 }
var given = { port: 9000 }

defaults.extend(given)
echo defaults
```

```console
{host: localhost, port: 9000}
```

`extend()` mutates the receiver and the right-hand side wins on conflicts.
There is no `+` on dictionaries.

## Transforming

```zuri
var user = { name: 'Ada', age: 36, city: nil }

echo user.compact()
echo user.filter(@(value, key) => key == 'name')
echo user.some(@(value, key) => value == 36)
echo user.every(@(value, key) => value != nil)
echo user.reduce(@(acc, value, key) => acc + 1, 0)
```

```console
{name: Ada, age: 36}
{name: Ada}
true
false
3
```

`compact()` drops entries whose value is `nil`.

Every callback here takes **value, then key**, the same order lists use.

## Walking

Every form below visits the pairs in **insertion order**, which is the
order they were first added, not the order they were last written to.

### `for` With Two Variables, the Usual Form

```zuri
var user = { name: 'Ada', age: 36 }

for key, value in user {
  echo '${key} = ${value}'
}
```

```console
name = Ada
age = 36
```

Key first, value second. This is what you want almost every time.

### `for` With One Variable Gives You the Values

```zuri
var user = { name: 'Ada', age: 36 }

for value in user {
  echo value
}
```

```console
Ada
36
```

This is worth stating plainly, because the equivalent loop in several other
languages hands you the keys. In Zuri, one variable is always the value,
whatever you are iterating.

### `iter` Over the Keys, When You Need an Index

```zuri
var user = { name: 'Ada', age: 36 }
var keys = user.keys()

iter var i = 0; i < keys.length(); i++ {
  echo '${i}. ${keys[i]} = ${user[keys[i]]}'
}
```

```console
0. name = Ada
1. age = 36
```

A dictionary has no positional index of its own, so `iter` walks the key
list instead. Reach for it when you need to number the output, or to look
ahead to the next key.

### `each()`, With a Function

```zuri
var user = { name: 'Ada', age: 36 }

user.each(@(value, key) {
  echo '${key} -> ${value}'
})
```

```console
name -> Ada
age -> 36
```

The callback receives the **value first and the key second** — the reverse
of `for`, matching every other `each()` in the language.

### Walking Just One Side

```zuri
var user = { name: 'Ada', age: 36 }

for key in user.keys() {
  echo key
}

echo user.values()
```

```console
name
age
[Ada, 36]
```

`keys()` and `values()` each return a plain list, so everything from
[Lists](ch04-03-lists.md) applies: sort them, filter them, reduce them.
Sorting `keys()` is how you get output in a stable order regardless of
insertion.

## Converting

```zuri
var user = { name: 'Ada', age: 36 }

echo user.to_list()
echo user.find_key('Ada')
echo user.clone()
```

```console
[[name, age], [Ada, 36]]
name
{name: Ada, age: 36}
```

`to_list()` gives you two parallel lists, keys then values, not a list of
pairs. `find_key()` searches by value and returns the first key holding
it.

## Dictionaries of Functions

A dictionary value can be a function, and that turns a dictionary into a
**dispatch table**: a set of named behaviours you can choose between at
runtime by looking a name up. It is the structure that replaces `eval()`,
and it is worth knowing well.

```zuri
def plus(a, b) {
  return a + b
}

var ops = {
  plus: plus,
  minus: @(a, b) => a - b,
  times: @(a, b) { return a * b },
}
```

All three forms are equivalent: a named function by name, an arrow
function, and an anonymous function with a body.

### Calling One

This is the part that surprises people, so it comes first. **You cannot
call one with a dot in a single step:**

```zuri,ignore
echo ops.plus(1, 2)
```

```console
Unhandled TypeError: object of type dict does not define method 'plus'
```

A dot followed by a call looks for a **method on the dictionary itself** —
`length()`, `keys()`, `get()` and the rest — not for a value stored under
that key. Dictionaries have no method called `plus`, so the call fails.

There are two forms that do work. Read the function out first:

```zuri
def plus(a, b) {
  return a + b
}

var ops = { plus: plus, minus: @(a, b) => a - b }

var chosen = ops.plus

echo chosen(1, 2)
```

```console
3
```

Or index with brackets and call the result:

```zuri
var ops = { plus: @(a, b) => a + b, minus: @(a, b) => a - b }

echo ops['minus'](5, 2)
```

```console
3
```

The bracket form is the one to reach for when the key is computed, which is
the whole point of a dispatch table:

```zuri
var ops = {
  plus: @(a, b) => a + b,
  minus: @(a, b) => a - b,
  times: @(a, b) => a * b,
}

for name in ['plus', 'minus', 'times'] {
  echo '${name}: ${ops[name](6, 3)}'
}
```

```console
plus: 9
minus: 3
times: 18
```

### Beware of Names That Are Already Methods

A dictionary's own methods take priority over its keys when you use a dot
call. Storing a function under `add`, `keys`, `get`, `length` or any other
method name produces a call that succeeds and does the **wrong thing**:

```zuri
def plus(a, b) {
  return a + b
}

var d = { add: plus }

echo d.add(1, 2)
echo d.keys()
```

```console
nil
[add, 1]
```

`d.add(1, 2)` called the dictionary's own `add()`, which is a synonym for
`set()` — so it stored the key `1` with the value `2` and returned `nil`.
Nothing raised. The only sign is the extra key in the output.

Two habits avoid this entirely: use bracket calls for dispatch tables, and
avoid method names as keys. [Appendix E](appendix-05-06-dict.md) lists
every name a dictionary already uses.

### A Dispatch Table in Practice

The pattern is a table of handlers, a lookup with a fallback, and a call:

```zuri
def _unknown(args) {
  return 'unknown command: ${args[0]}'
}

var commands = {
  greet: @(args) => 'hello, ${args[1]}',
  add: @(args) => args[1].to_number() + args[2].to_number(),
  version: @(args) => '1.0.0',
}

def run(line) {
  var args = line.split(' ')
  var handler = commands.get(args[0], _unknown)

  return handler(args)
}

echo run('greet ada')
echo run('add 2 40')
echo run('version')
echo run('explode')
```

```console
hello, ada
42
1.0.0
unknown command: explode
```

`commands.get(args[0], _unknown)` is doing the work. A key that exists
gives you its handler; one that does not gives you the fallback, so there
is no separate `contains()` check and no branch for the error case.

The important property is that **only the handlers in the table can ever
run**. A user typing anything at all selects one of four functions you
wrote, or the fallback. That is the difference between a program that
handles input and one that executes it, and it is why Zuri has no
`eval()`. [Chapter 18](ch18-00-metaprogramming.md) covers the reasoning.

### Storing Methods

`zuri.reflect.bind_method()` puts an instance's method in a table with the
instance still attached:

```zuri
import zuri

class Counter {

  @new() {
    self.n = 0
  }

  increment() {
    self.n++
    return self.n
  }
}

var counter = Counter()

var actions = { up: zuri.reflect.bind_method(counter, 'increment') }

echo actions['up']()
echo actions['up']()
echo counter.n
```

```console
1
2
2
```

The bound method keeps its receiver, so calling it through the table
changes the counter it came from.

## Comparing Dictionaries

`==` compares dictionaries by value, and **order is not part of the
comparison**:

```zuri
echo { a: 1 } == { a: 1 }
echo { a: 1 } == { a: 2 }
echo { a: 1, b: 2 } == { b: 2, a: 1 }
```

```console
true
false
true
```

A dictionary remembers its insertion order for iteration, but two
dictionaries with the same pairs in different orders are equal. That is the
opposite of lists, where order is the whole point.

## Dictionaries Are References

Like lists, assigning a dictionary shares it. `clone()` gives you a
shallow copy.

## Dictionaries and Objects

A dictionary is the right shape for data with a variable set of keys:
configuration, a parsed JSON document, a set of HTTP headers. When the
keys are fixed and there is behaviour attached to them, that is a class.
[Chapter 6](ch06-00-classes.md) covers the difference.

## A Worked Example

Dictionaries are the natural shape for counting, grouping and indexing.
This example does all three over a list of log lines: it counts how often
each level appears, groups the messages under their level, and builds an
index from a request id back to the line that mentioned it.

```zuri
var lines = [
  'INFO  req=a1 started',
  'WARN  req=a1 slow upstream',
  'INFO  req=b2 started',
  'ERROR req=b2 upstream refused',
  'INFO  req=a1 finished',
]

var counts = {}
var grouped = {}
var by_request = {}

for line in lines {
  var parts = line.split(' ').filter(@(p) => p != '')
  var level = parts[0]
  var request = parts[1].replace('req=', '', false)
  var message = ' '.join(parts[2, parts.length()])

  counts[level] = counts.get(level, 0) + 1

  if !grouped.contains(level) {
    grouped[level] = []
  }

  grouped[level].append(message)

  if !by_request.contains(request) {
    by_request[request] = []
  }

  by_request[request].append(level)
}

echo counts
echo grouped.ERROR
echo by_request.a1
echo by_request.get('zz', ['no such request'])
```

```console
{INFO: 3, WARN: 1, ERROR: 1}
[upstream refused]
[INFO, WARN, INFO]
[no such request]
```

Four habits in there are worth taking away.

`counts.get(level, 0) + 1` is the counting idiom. `get()` with a fallback
means you never have to check whether the key exists before adding to it.

`if !grouped.contains(level) { grouped[level] = [] }` is the grouping
idiom, and it has to be spelled out because `get()` would hand back a fresh
default list each time rather than one you can keep appending to.

`grouped.ERROR` and `by_request.a1` read keys with a dot, which works
because both are valid identifiers. `by_request['b2']` would be needed if
the key were computed or awkward.

And the whole thing preserves order. `counts` came out `INFO`, `WARN`,
`ERROR` — the order those levels were first seen, not alphabetical and not
arbitrary.
