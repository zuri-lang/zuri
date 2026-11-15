# Encapsulation and Class Immutability

## Private Members

A field or method whose name starts with `_` is private. You can access it
through `self` or `parent`, but nowhere else:

```zuri
class Box {
  var _items = []

  add(value) {
    self._items.append(value)
    return self
  }

  count() {
    return self._items.length()
  }
}

echo Box().add(1).add(2).count()
```

```console
2
```

Code outside the class that tries to access it doesn't compile:

```zuri,ignore
var b = Box()
echo b._items
```

```console
SyntaxError: '_items' is private and can only be accessed via 'self' or 'parent'
  --> /path/to/main.zu:2:8
   |
 2 | echo b._items
   |        ^
```

The check happens at compile time rather than at run time, so it has no
cost, and you can't get around it by building the name dynamically.

A subclass can access its parent's private members, through `self` for
fields and through `parent` for methods:

```zuri
class Base {
  var _secret = 'base secret'

  _internal() {
    return 'base internal'
  }
}

class Child < Base {
  reveal() {
    return self._secret + '/' + parent._internal()
  }
}

echo Child().reveal()
```

```console
base secret/base internal
```

Modules follow the same underscore convention: a module member whose name
starts with `_` can't be imported by name. [Chapter 8](ch08-00-modules.md)
covers modules.

## Classes Are Sealed

Once a class has been declared, its structure can't change. You can't add a
field or a method to the class, and you can't add one to an instance either:

```zuri
class Account {
  var balance = 0
}

var a = Account()

catch {
  a.nickname = 'rainy day'
} as e {
  echo e.message
}
```

```console
undefined field 'nickname' on instance of 'Account'
```

In practice, this means a misspelled field name is reported as an error
where you wrote it, instead of creating a new field that quietly hides the
one you meant. Every field of a class is declared in one place, which is
`@new`.

The **values** of static fields can change, but the set of static fields
can't.

## Accessing Fields by Name

Four built-in functions read and write fields by name:

```zuri
var b = Box()

echo hasprop(b, '_items')
echo getprop(b, '_items')
echo delprop(b, '_items')
echo getprop(b, '_items')
```

```console
true
[]
true
nil
```

`setprop(obj, name, value)` writes a field, and `delprop` resets it to
`nil`. Both return `false` if the class doesn't have the field, because
neither function can create one.

These functions ignore the underscore rule on purpose. They exist for
serializers, debuggers and test helpers, whose whole job is to look inside
objects. Regular code shouldn't use them.

The `zuri` module goes further and provides a complete reflection API for
classes, functions and modules. [Chapter 21](ch21-00-metaprogramming.md)
covers it.

## Designing With Sealed Classes

Sealed classes lead to two good habits.

**Declare every field in the class, even the ones the constructor sets.**
`self.x = value` inside `@new` declares a field for you, but a `var` line at
the top of the class body documents it for the next person who reads the
code. It's also required as soon as a helper method does the assigning
instead.

**Represent optional state as a field that holds `nil`, not as a missing
field.** Fields can't be missing, so a `var cached_result` that starts out
as `nil` is the way to do it.

## Example: A Bounded History

Privacy is most useful when a class has an invariant to protect. Here's a
bounded history buffer. It keeps the last `n` entries and nothing more, and
a caller has no way of breaking that from outside.

```zuri
class History {
  var _entries = []
  var _limit = 0

  @new(limit: number) {
    if limit < 1 {
      raise ValueError('limit must be at least 1, got ${limit}')
    }

    self._limit = limit
  }

  record(entry) {
    self._entries.append(entry)

    if self._entries.length() > self._limit {
      self._entries.shift()
    }

    return self
  }

  # A copy, so a caller cannot append through the value we hand back.
  entries() {
    return self._entries.clone()
  }

  length() {
    return self._entries.length()
  }
}

var history = History(3)

history.record('a').record('b').record('c').record('d')

echo history.entries()
echo history.length()

var taken = history.entries()
taken.append('e')

echo history.entries()

catch {
  History(0)
} as e {
  echo '${e.type}: ${e.message}'
}
```

```console
[b, c, d]
3
[b, c, d]
ValueError: limit must be at least 1, got 0
```

Four design decisions make this work.

**The list is private**, so nothing outside the class can append to it and
skip the trimming. `history._entries.append('x')` doesn't compile.

**`entries()` returns a copy.** Otherwise, the caller would have the real
list and could grow it past the limit. The fourth line of output shows that
this *doesn't* happen. Returning a private, mutable collection is the most
common way encapsulation breaks down.

**The invariant is set up in the constructor.** `_limit` is validated once,
so `record()` never has to check whether it's sensible.

**The class is sealed**, so `history.limit = 999` is an error, rather than a
second field that's ignored while `_limit` keeps working.
