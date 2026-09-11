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

## The Constructor

`@new` is the constructor. It runs once, when the class is called, and its
job is to put the instance into a usable state — which is what
`Account`'s did above.

### It Is Optional

A class with no `@new` is constructed with no arguments, and every field
takes its declared default:

```zuri
class Settings {
  var theme = 'dark'
  var retries
}

var s = Settings()

echo s.theme
echo s.retries
```

```console
dark
nil
```

Extra arguments to a class with no `@new` are ignored rather than
rejected, exactly as they are for a function.

### It Declares Fields

`self.x = value` inside `@new` **declares** a field. This is the one place
in the language where assignment creates a member, and it exists so a
constructor does not have to repeat every field as a `var` line above it:

```zuri
class Point {

  @new(x, y) {
    self.x = x
    self.y = y
    self.distance = (x * x + y * y).sqrt()
  }
}

var p = Point(3, 4)

echo p.distance
```

```console
5
```

`distance` was never declared with `var`, and it is a real field.

That power belongs to `@new`'s **own body** and nowhere else. A constructor
that delegates its setup to a helper must declare those fields:

```zuri
class Delayed {
  var ready          # required: setup() cannot declare it

  @new() {
    self.setup()
  }

  setup() {
    self.ready = true
  }
}

echo Delayed().ready
```

```console
true
```

Remove the `var ready` line and `setup()` raises
`undefined field 'ready'`.

### Arguments Are Not Checked Unless You Ask

`@new` is a function, so the usual rules apply: missing arguments arrive as
`nil`, extra ones are dropped. A constructor that assumes it got something
fails later, in a confusing place:

```zuri,ignore
class Ctor {

  @new(x) {
    self.derived = x * 2
  }
}

Ctor()
```

```console
Unhandled TypeError: operator '*' not defined for call signature (nil, float)
```

Annotate the parameter and the failure moves to the call, where it names
the problem:

```zuri
class Ctor {

  @new(x: number) {
    self.derived = x * 2
  }
}

echo Ctor(5).derived

catch {
  Ctor('five')
} as e {
  echo e.message
}
```

```console
10
@new() expects parameter 'x' (argument 1) to be a number, got string
```

Constructors are the highest-value place in a program to annotate, because
a badly built object goes wrong somewhere else entirely.

### Validating in the Constructor

A constructor may raise. Nothing is returned to the caller, so an object
that cannot be valid never exists:

```zuri
class Port {

  @new(number: number) {
    if number < 1 or number > 65535 {
      raise ValueError('port out of range: ${number}')
    }

    self.number = number
  }
}

echo Port(8080).number

catch {
  Port(99999)
} as e {
  echo '${e.type}: ${e.message}'
}
```

```console
8080
ValueError: port out of range: 99999
```

This is worth doing whenever a class has an invariant. Every other method
can then assume it holds, instead of re-checking.

### `@new` Cannot Return a Value

Calling a class always produces an instance of that class. A `return` in
`@new` ends the constructor early; it does not change what the caller
gets:

```zuri
class Returns {

  @new() {
    self.v = 1

    return 'this is discarded'
  }
}

echo typeof(Returns())
```

```console
Returns
```

If you want a call that may hand back something else — a cached instance, a
subclass, `nil` on bad input — write a `static` factory method and call
that instead.

## Methods

A method is a `name(params) { ... }` declaration inside the class body.
There is no `def` keyword on it:

```zuri
class Account {

  @new(owner, balance) {
    self.owner = owner
    self.balance = balance
  }

  deposit(amount) {
    self.balance += amount
    return self
  }
}

var a = Account('Ada', 50)

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
