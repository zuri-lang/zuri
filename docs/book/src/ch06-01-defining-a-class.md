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

- `class Name { ... }` declares a class. By convention, class names use
  `PascalCase`.
- `var` inside the class body declares a **field**, with an optional default
  value. A field without a default starts out as `nil`.
- A plain `name(params) { ... }` declares a **method**. Methods don't use
  the `def` keyword.
- `@new` is the constructor.
- `self` refers to the current instance.

## Creating an Instance

To create an instance, call the class. There's no `new` keyword:

```zuri
var a = Account('Ada', 50)

echo a.owner
echo a.balance
```

```console
Ada
50
```

If the class doesn't have an `@new` method, calling it without arguments
gives you an instance with every field set to its default value.

## The Constructor

`@new` is the constructor. It runs once, when the class is called, and its
job is to put the new instance into a usable state, as `Account`'s
constructor did above.

### Constructors Are Optional

A class without an `@new` method is created with no arguments, and each
field gets its declared default:

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

If you pass arguments to a class that has no `@new`, they're ignored rather
than rejected, as they would be for a function.

### Constructors Can Declare Fields

`self.x = value` inside `@new` **declares** a field. This is the only place
in the language where an assignment creates a member, and it exists so that
a constructor doesn't have to repeat every field as a `var` line above it:

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

`distance` was never declared with `var`, but it's still a real field.

This only works in the **body of `@new` itself**. If a constructor delegates
its setup to a helper method, the fields that helper assigns have to be
declared:

```zuri
class Delayed {
  var ready # required: setup() cannot declare it

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

If you remove the `var ready` line, `setup()` raises
`undefined field 'ready'`.

### Constructor Arguments Aren't Checked by Default

`@new` is a function, so the usual rules apply: missing arguments are `nil`,
and extra arguments are ignored. A constructor that assumes it received an
argument fails later, somewhere confusing:

```zuri,ignore
class Ctor {

  @new(x) {
    self.derived = x * 2
  }
}

Ctor()
```

```console
Unhandled TypeError: operator '*' not defined for call signature (nil, number)
```

If you annotate the parameter, the error is raised at the call instead, and
it tells you what's wrong:

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

Constructors are the most valuable place in a program to add annotations,
because an object that's built incorrectly usually fails somewhere else
entirely.

### Validating in the Constructor

A constructor can raise an error. When it does, nothing is returned to the
caller, so an object that can't be valid never exists:

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

This is worth doing whenever a class has an invariant, a condition that
should always be true for its instances. The other methods can then rely on
it instead of checking it again.

### `@new` Can't Return a Value

Calling a class always gives you an instance of that class. A `return` in
`@new` ends the constructor early, but it doesn't change what the caller
receives:

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

If you need a call that might return something else, such as a cached
instance, a subclass or `nil` for invalid input, write a `static` factory
method and call that instead.

## Methods

A method is a `name(params) { ... }` declaration inside the class body.
Methods don't use the `def` keyword:

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

`deposit` returns `self`, which is what makes the chained calls work.
Returning `self` from a method that modifies the object is a common pattern
in Zuri.

Inside a method, you have to use `self` to access a field or call another
method. There's no implicit receiver:

```zuri
class Greeter {
  var name = 'world'

  greet() {
    return 'hello ' + self.name
  }
}
```

Writing just `name` there would look for a local or global variable, not a
field.

## Fields Must Be Declared

A class's fields are fixed when the class is declared, which has two
consequences.

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

Second, as described above, `self.x = value` inside `@new` **does** declare
a field, so that constructors don't have to repeat themselves:

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

Again, this only applies in the body of `@new` itself. If a constructor
calls a helper to do its initialization, the helper's fields have to be
declared with `var`:

```zuri
class A {
  var from_helper # required

  @new() {
    self.setup()
  }

  setup() {
    self.from_helper = 2
  }
}
```

Without the `var` line, the assignment in `setup()` raises
`undefined field 'from_helper'`.

## Static Members

`static` attaches a field or method to the class itself instead of to each
instance:

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

Static methods don't have a `self`:

```console
SyntaxError: 'self' used outside of a method
```

Use the class name to refer to the class instead.

Static fields are the only part of a class that can change. The set of
members is sealed, but the values of static fields aren't.

## Duplicate Members

If you declare the same method twice in one class, Zuri reports an error
instead of quietly keeping the second one:

```zuri,ignore
class B {
  m() {}
  m() {}
}
```

```console
SyntaxError: multiple declaration for method 'm' found in class 'B'
```

The same is true if you declare two classes with the same name in one
module.

## Classes or Dictionaries?

| | `dict` | `class` |
| --- | --- | --- |
| Keys | Any, added at any time | Fixed when the class is declared |
| Access | `d.key` or `d['key']` | `obj.field` only |
| Missing member | `get()` returns a fallback | Error |
| Behavior | None | Methods |
| Cost of reading a member | Hash lookup | Array index |

Use a dictionary for data whose shape you only find out at run time, such as
parsed JSON, HTTP headers or a configuration file. Use a class when you know
the shape in advance and want to attach behavior to it.
