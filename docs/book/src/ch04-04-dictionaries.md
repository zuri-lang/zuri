# Dictionaries

A dictionary maps keys to values and remembers the order in which the keys
were added.

```zuri
var user = { name: 'Ada', age: 36 }
var empty = {}
```

A key written as a bare word is a string, so `{ name: ... }` and
`{ 'name': ... }` are the same. Keys can also be numbers:

```zuri
echo { 1: 'one', 2.5: 'two-point-five' }
```

```console
{1: one, 2.5: two-point-five}
```

A key written as a bare identifier is always taken literally, as that name.
To compute a key, write it as an expression that the parser can't mistake
for a name. In practice, that means wrapping it in parentheses:

```zuri
var key = 'dynamic'

echo { (key): 1 }
echo { 'a' + 'b': 2 }
```

```console
{dynamic: 1}
{ab: 2}
```

Assigning with brackets is often clearer:

```zuri
var d = {}
d[key] = 3
echo d
```

```console
{dynamic: 3}
```

### Shorthand Keys

When a key has the same name as the variable that holds its value, you only
need to write it once:

```zuri
var name = 'Ada'
var age = 36

echo { name, age }
```

```console
{name: Ada, age: 36}
```

`{ name, age }` means exactly the same as `{ name: name, age: age }`. This
is especially common in functions that build a result from local variables
they've just computed, and Zuri code uses the shorthand whenever the names
match.

### Nesting

A value can be another dictionary or a list, nested to any depth. You can
chain accesses to reach nested values:

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

You can mix dot and bracket access freely, because they do the same thing.
Use a dot when the key is a fixed identifier, and brackets when it's
computed, contains punctuation or is a number.

## Reading Values

Dot access and bracket access do the same thing:

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

Reading a key that doesn't exist raises a `PropertyError`. Use `get()` if
the key might be missing:

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

Without a fallback value, `get()` returns `nil` for a missing key.

## Writing Values

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

Assigning with a dot, with brackets or with `set()` all do the same thing,
and all three create the key if it doesn't exist yet. `add()` is another
name for `set()`.

To remove a key:

```zuri
echo user.remove('city')
echo user.contains('city')
```

```console
London
false
```

`remove()` returns the value that was stored under the key.

`clear()` removes every key.

## Merging Dictionaries

```zuri
var defaults = { host: 'localhost', port: 8080 }
var given = { port: 9000 }

defaults.extend(given)
echo defaults
```

```console
{host: localhost, port: 9000}
```

`extend()` changes the dictionary it's called on, and when both dictionaries
have the same key, the value from the argument wins. Dictionaries don't
support `+`.

## Transforming Dictionaries

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

`compact()` removes entries whose value is `nil`.

All of these callbacks receive the **value first and the key second**, the
same order that list callbacks use.

## Iterating Over a Dictionary

All of the approaches below visit the entries in **insertion order**, which
is the order in which keys were first added, not the order in which they
were last updated.

### `for` With Two Variables

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

You get the key first and the value second. This is what you'll want nearly
every time.

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

This is worth pointing out, because in several other languages the
equivalent loop gives you the keys. In Zuri, a single variable always
receives the values, whatever you're iterating over.

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

A dictionary doesn't have positional indexes of its own, so `iter` loops
over the list of keys instead. Use this when you need to number the output
or look ahead to the next key.

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

The callback receives the **value first and the key second**. That's the
opposite of `for`, but it's the same as every other `each()` in the
language.

### Iterating Over Only the Keys or the Values

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

`keys()` and `values()` both return an ordinary list, so everything in
[Lists](ch04-03-lists.md) applies. You can sort, filter or reduce them.
Sorting `keys()` is how you get output in a predictable order, whatever
order the keys were added in.

## Converting Dictionaries

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

`to_list()` returns two parallel lists, one of keys and one of values,
rather than a list of pairs. `find_key()` searches by value and returns the
first key that holds it.

## Dictionaries of Functions

A dictionary value can be a function, which turns the dictionary into a
**dispatch table**: a set of named behaviors that you can choose between at
run time by looking up a name. This is what you use in Zuri instead of
`eval()`, so it's worth getting to know well.

```zuri
def plus(a, b) {
  return a + b
}

var ops = {
  plus,
  minus: @(a, b) => a - b,
  times: @(a, b) {
    return a * b
  },
}
```

The three entries are equivalent. The first is a named function, the second
is an arrow function, and the third is an anonymous function with a body.

### Calling a Stored Function

This surprises a lot of people, so let's start with it. **You can't call a
stored function with a dot in one step:**

```zuri,ignore
echo ops.plus(1, 2)
```

```console
Unhandled TypeError: object of type dict does not define method 'plus'
```

A dot followed by a call looks for a **method of the dictionary itself**,
such as `length()`, `keys()` or `get()`, not for a value stored under that
key. Dictionaries don't have a method called `plus`, so the call fails.

There are two ways that do work. You can read the function into a variable
first:

```zuri
def plus(a, b) {
  return a + b
}

var ops = { plus, minus: @(a, b) => a - b }

var chosen = ops.plus

echo chosen(1, 2)
```

```console
3
```

Or you can use brackets and call the result:

```zuri
var ops = { plus: @(a, b) => a + b, minus: @(a, b) => a - b }

echo ops['minus'](5, 2)
```

```console
3
```

Use the bracket form when the key is computed, which is the whole point of a
dispatch table:

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

### Avoid Keys That Are Method Names

When you use a dot call, the dictionary's own methods take priority over its
keys. If you store a function under `add`, `keys`, `get`, `length` or the
name of any other method, the call will succeed but do the **wrong thing**:

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

`d.add(1, 2)` called the dictionary's own `add()` method, which is another
name for `set()`. So it stored the key `1` with the value `2` and returned
`nil`. No error was raised, and the only sign that something went wrong is
the extra key in the output.

Two habits will keep you out of trouble: use bracket calls for dispatch
tables, and don't use method names as keys. [Appendix
E](appendix-05-06-dict.md) lists all of the names that dictionaries already
use.

### Example: A Command Dispatcher

A dispatch table usually consists of a table of handlers, a lookup with a
fallback, and a call:

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

`commands.get(args[0], _unknown)` does most of the work. If the key exists,
you get its handler. If it doesn't, you get the fallback. That means you
don't need a separate `contains()` check or a branch for the error case.

The important thing is that **only the handlers in the table can ever run**.
Whatever the user types, it selects one of the four functions you wrote, or
the fallback. That's the difference between a program that handles input and
a program that executes it, and it's why Zuri doesn't have `eval()`.
[Chapter 21](ch21-00-metaprogramming.md) explains the reasoning.

### Storing Methods

`zuri.reflect.bind_method()` stores an instance's method in a table while
keeping the instance attached to it:

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

The bound method remembers its instance, so calling it through the table
updates the counter it came from.

## Comparing Dictionaries

`==` compares dictionaries by value, and **the order of the keys doesn't
matter**:

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

A dictionary remembers its insertion order when you iterate over it, but two
dictionaries with the same entries in a different order are still equal.
This is the opposite of lists, where order is part of what makes two lists
equal.

## Dictionaries Are Passed by Reference

As with lists, assigning a dictionary to another variable shares it rather
than copying it. `clone()` gives you a shallow copy.

## Dictionaries or Classes?

A dictionary is a good fit for data with a variable set of keys, such as
configuration, a parsed JSON document or a set of HTTP headers. When the
keys are fixed and there's behavior attached to them, use a class instead.
[Chapter 6](ch06-00-classes.md) covers the difference.

## Example: Analyzing a Log

Dictionaries are well suited to counting, grouping and indexing. This
example does all three with a list of log lines. It counts how many times
each level appears, groups the messages by level, and builds an index that
maps each request ID to the line that mentions it.

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

There are four habits here that are worth copying.

`counts.get(level, 0) + 1` is the usual way to count. Because `get()` has a
fallback, you never have to check whether the key exists before adding to
it.

`if !grouped.contains(level) { grouped[level] = [] }` is the usual way to
group. You have to write it out, because `get()` would return a new default
list each time instead of one you can keep appending to.

`grouped.ERROR` and `by_request.a1` read keys with a dot, which works
because both keys are valid identifiers. If a key were computed or contained
unusual characters, you'd need brackets, as in `by_request['b2']`.

Finally, the order is preserved throughout. `counts` lists `INFO`, `WARN`
and `ERROR` in the order those levels first appeared, not in alphabetical or
random order.
