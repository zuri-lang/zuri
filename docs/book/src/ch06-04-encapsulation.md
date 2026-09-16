# Encapsulation and Class Immutability

## Private Members

A field or method whose name starts with `_` is private. It can be reached
through `self` or `parent`, and nowhere else:

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

Reaching in from outside does not compile:

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

This is a compile-time check, not a runtime one, so it costs nothing and
cannot be worked around by computing the name.

A subclass can reach its parent's private members, through `self` for
fields and `parent` for methods:

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

The same underscore convention governs modules: a module member whose name
starts with `_` cannot be imported by name. [Chapter 8](ch08-00-modules.md)
covers that side of it.

## Classes Are Sealed

Once a class is declared, its shape is final. You cannot add a field or a
method to it, and you cannot add one to an instance:

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

The practical consequence is that a misspelled field name is an error at
the point you write it, rather than a new field that silently shadows the
one you meant. Every field a class has is declared in one place, and that
place is `@new`.

Static field **values** are mutable; the set of static fields is not.

## Reflective Access

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

`setprop(obj, name, value)` writes; `delprop` resets the slot to `nil`.
Both return `false` when the field does not exist on the class, because
neither can create one.

These bypass the underscore rule, which is deliberate: they exist for
serialisers, debuggers and test helpers, where reaching into an object is
the entire point. Regular code should not use them.

The `zuri` module goes further, with a full reflection API over classes,
functions and modules. [Chapter 18](ch18-00-metaprogramming.md) covers it.

## Designing With Sealed Classes

Two habits follow from sealing.

**Declare every field on the class, even the ones the constructor fills
in.** `self.x = value` inside `@new` will declare one for you, but a `var`
line at the top of the body is documentation the next reader gets for free,
and it is required the moment a helper method does the assigning instead.

**Model optional state as a field holding `nil`, not as an absent field.**
There is no such thing as an absent field, so a `var cached_result` that
starts `nil` is the shape you want.

## A Worked Example

Privacy earns its place when a class has an invariant to protect. Here is a
bounded history buffer: it keeps the last `n` entries and nothing else, and
there is no way for a caller to break that from outside.

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

Four decisions are doing the work.

**The list is private**, so nothing outside can append to it and skip the
trimming. `history._entries.append('x')` does not compile.

**`entries()` returns a clone.** Without it, the caller would hold the
real list and could grow it past the limit — which is exactly what the
fourth output line shows *not* happening. Handing out a private mutable
collection is the most common way encapsulation leaks.

**The invariant is established in the constructor.** `_limit` is validated
once, so `record()` never has to wonder whether it is sensible.

**The class is sealed**, so `history.limit = 999` is an error rather than a
second, ignored field sitting alongside `_limit`.
