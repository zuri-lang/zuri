# Defining a Class

```zuri
class Account {
  var balance = 0
  var owner

  @new(owner, balance) {
    self.owner = owner
    self.balance = balance or 0
  }

  deposit(amount) {
    self.balance += amount
    return self
  }
}
```

- `class Name { ... }` declares it. `PascalCase` is the convention.
- `var` inside the body declares a **field**, with an optional default. A
  field with no default starts as `nil`.
- A bare `name(params) { ... }` is a **method**. There is no `def` keyword
  on methods.
- `@new` is the constructor.
- `self` is the current instance.

## Creating an Instance

Call the class. There is no `new` keyword:

```zuri
var a = Account('Ada', 50)

echo a.owner
echo a.balance
```

```console
Ada
50
```

If the class has no `@new`, calling it with no arguments gives you an
instance with every field at its default.

## Methods

```zuri
a.deposit(25).deposit(25)
echo a.balance
```

```console
100
```

`deposit` returns `self`, which is what makes the chain work. Returning
`self` from a mutator is a common Zuri idiom.

Inside a method, `self` is required to reach a field or another method.
There is no implicit receiver:

```zuri
class Greeter {
  var name = 'world'

  greet() {
    return 'hello ' + self.name
  }
}
```

Writing `name` there would look for a local or a global, not a field.

## Fields Are Declared, Not Discovered

The set of fields is fixed when the class is declared. Two things follow
from that.

First, assigning to a field that was never declared is an error:

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

Second, `self.x = value` inside `@new` **does** declare a field, as a
convenience so that constructors do not have to repeat themselves:

```zuri
class Point {
  @new(x, y) {
    self.x = x
    self.y = y
  }
}

echo Point(3, 4).x
```

```console
3
```

That convenience applies to `@new`'s own body and nowhere else. A
constructor that calls a helper to do its initialisation must declare those
fields with `var`:

```zuri
class A {
  var from_helper          # required

  @new() {
    self.setup()
  }

  setup() {
    self.from_helper = 2
  }
}
```

Without the `var` line, the assignment inside `setup()` raises
`undefined field 'from_helper'`.

## Static Members

`static` puts a field or method on the class rather than on each instance:

```zuri
class Account {
  static var count = 0

  @new(owner) {
    self.owner = owner
    Account.count++
  }

  static open(owner) {
    return Account(owner)
  }
}

Account('Ada')
Account('Bob')

echo Account.count
echo Account.open('Carol').owner
```

```console
2
Carol
```

A static method has no `self`:

```console
SyntaxError: 'self' used outside of a method
```

Reach the class by name instead.

Static fields are the one mutable part of a class. The set of members is
sealed; the values of static fields are not.

## Duplicate Members Are Errors

Declaring the same method twice in one class does not silently keep the
last one:

```zuri,ignore
class B {
  m() {}
  m() {}
}
```

```console
SyntaxError: multiple declaration for method 'm' found in class 'B'
```

The same applies to declaring the same class name twice in one module.

## Instances and Dictionaries

| | `dict` | `class` |
| --- | --- | --- |
| keys | any, added at any time | fixed at declaration |
| access | `d.key` or `d['key']` | `obj.field` only |
| missing member | `get()` returns a fallback | error |
| behaviour | none | methods |
| cost of a read | hash lookup | array index |

Use a dictionary for data whose shape you learn at runtime: parsed JSON,
HTTP headers, a config file. Use a class when the shape is known and there
is behaviour to attach.
