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

Or with `for`, which gives you key first:

```zuri
for key, value in user {
  echo '${key} = ${value}'
}
```

Both walk in insertion order.

## Converting

```zuri
var user = { name: 'Ada', age: 36 }

echo user.to_list()
echo user.find_key('Ada')
echo user.clone()
```

```console
[[name, age], [Ada, 36]]
Ada
{name: Ada, age: 36}
```

`to_list()` gives you two parallel lists, keys then values, not a list of
pairs. `find_key()` searches by value and returns the first key holding
it.

## Dictionaries Are References

Like lists, assigning a dictionary shares it. `clone()` gives you a
shallow copy.

## Dictionaries and Objects

A dictionary is the right shape for data with a variable set of keys:
configuration, a parsed JSON document, a set of HTTP headers. When the
keys are fixed and there is behaviour attached to them, that is a class.
[Chapter 6](ch06-00-classes.md) covers the difference.
