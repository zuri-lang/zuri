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

```zuri
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

This is not a restriction the language apologises for. It is what lets
every instance have a flat array of fields with a compile-time-known index
per name, which is why a field read is an array index instead of a hash
lookup, and why the JIT can inline one.

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
functions and modules. [Chapter 17](ch17-00-metaprogramming.md) covers it.

## Designing With Sealed Classes

Two habits follow from sealing.

**Declare every field on the class, even the ones the constructor fills
in.** `self.x = value` inside `@new` will declare one for you, but a `var`
line at the top of the body is documentation the next reader gets for free,
and it is required the moment a helper method does the assigning instead.

**Model optional state as a field holding `nil`, not as an absent field.**
There is no such thing as an absent field, so a `var cached_result` that
starts `nil` is the shape you want.
