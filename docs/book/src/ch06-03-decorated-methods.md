# Decorated Methods

A method whose name starts with `@` is a **decorated method**. The runtime
calls it for you when a particular piece of syntax is used with an instance
of your class. This is how a class can support construction, arithmetic,
comparison and iteration without any special syntax of its own.

Apart from that, decorated methods are ordinary methods. They can be
inherited, overridden and called by name.

## `@new`: Construction

You've already seen this one. It runs when the class is called and receives
the arguments, and it's the only place where `self.x = value` can declare a
new field.

## Arithmetic

Define a method for the operator you want, and the operator works with your
instances:

```zuri
class Vector {
  @new(x, y) {
    self.x = x
    self.y = y
  }

  @add(other) {
    return Vector(self.x + other.x, self.y + other.y)
  }

  @sub(other) {
    return Vector(self.x - other.x, self.y - other.y)
  }

  @mul(k) {
    return Vector(self.x * k, self.y * k)
  }

  @neg() {
    return Vector(-self.x, -self.y)
  }

  to_string() {
    return '(${self.x}, ${self.y})'
  }
}

var a = Vector(1, 2)
var b = Vector(3, 4)

echo (a + b).to_string()
echo (b - a).to_string()
echo (a * 3).to_string()
echo (-a).to_string()
```

```console
(4, 6)
(2, 2)
(3, 6)
(-1, -2)
```

Here's the full set of arithmetic decorators:

| Decorator | Operator |
| --- | --- |
| `@add` | `+` |
| `@sub` | `-` (binary) |
| `@mul` | `*` |
| `@div` | `/` |
| `@floordiv` | `//` |
| `@mod` | `%` |
| `@pow` | `**` |
| `@neg` | `-` (unary) |

## Bitwise and Logical Operators

| Decorator | Operator |
| --- | --- |
| `@and` | `&` |
| `@or` | `\|` |
| `@xor` | `^` |
| `@lshift` | `<<` |
| `@rshift` | `>>` |
| `@urshift` | `>>>` |
| `@not` | `~` |

```zuri
class Flags {
  @new(bits) {
    self.bits = bits
  }

  @and(mask) {
    return self.bits & mask
  }

  @not() {
    return Flags(~self.bits)
  }
}

echo Flags(5) & 4
echo (~Flags(5)).bits
```

```console
4
-6
```

`@not` is bound to `~`, the bitwise complement. `!` is logical negation, and
you can't override it. An instance is always truthy, so `!instance` is
always `false`.

## Comparison

| Decorator | Operator |
| --- | --- |
| `@eq` | `==` and `!=` |
| `@lt` | `<` |
| `@lte` | `<=` |
| `@gt` | `>` |
| `@gte` | `>=` |

```zuri
class Version {
  @new(major, minor) {
    self.major = major
    self.minor = minor
  }

  @lt(other) {
    if self.major != other.major {
      return self.major < other.major
    }
    return self.minor < other.minor
  }

  @gt(other) {
    return other < self
  }
}

echo Version(1, 2) < Version(1, 10)
echo Version(2, 0) > Version(1, 10)
```

```console
true
true
```

### Each Operator Is Defined Separately

Operators aren't derived from each other. Defining `@lt` **doesn't** give
you `>`, and defining `@add` doesn't give you `+=` with your instance on the
other side:

```zuri
class Price {

  @new(cents) {
    self.cents = cents
  }

  @lt(other) {
    return self.cents < other.cents
  }
}

echo Price(250) < Price(500)

catch {
  echo Price(500) > Price(250)
} as e {
  echo e.message
}
```

```console
true
operator '>' not defined for Price and Price
```

Define every operator you want to support. `@gt` is usually a single line,
as the `Version` example above shows.

### The Other Operand Isn't Checked

A decorated method is an ordinary method, and its parameter is an ordinary
parameter, so nothing guarantees that the other operand is an instance of
the same class:

```zuri
class Amount {

  @new(cents) {
    self.cents = cents
  }

  @add(other) {
    return Amount(self.cents + other.cents)
  }
}

catch {
  echo Amount(500) + 5
} as e {
  echo '${e.type}: ${e.message}'
}

catch {
  echo 5 + Amount(500)
} as e {
  echo '${e.type}: ${e.message}'
}
```

```console
TypeError: cannot read property 'cents' on a number
TypeError: operator '+' not defined for call signature (number, Amount)
```

Compare those two results. With the instance on the **left**, `@add` ran and
failed inside your own method with a confusing message. With the instance on
the **right**, the operator was never passed to your class at all, because a
decorated method only handles the case where its own instance is the left
operand.

Annotate the parameter to fix the first error message, and treat the second
case as a rule of the language:

```zuri
class Sum {

  @new(cents) {
    self.cents = cents
  }

  @add(other: Sum) {
    return Sum(self.cents + other.cents)
  }
}

catch {
  echo Sum(500) + 5
} as e {
  echo e.message
}
```

```console
@add() expects parameter 'other' (argument 1) to be a Sum, got number
```

### Equality

`@eq` defines `==` for your class, and `!=` is always the opposite of `==`:

```zuri
class Point {
  @new(x, y) {
    self.x = x
    self.y = y
  }

  @eq(other) {
    if !instance_of(other, Point) {
      return false
    }

    return self.x == other.x and self.y == other.y
  }
}

var p = Point(1, 2)

echo p == Point(1, 2)
echo p != Point(1, 2)
echo p == Point(2, 1)
echo p == nil
```

```console
true
false
false
false
```

`@eq` only runs when the value on the right is also an object, such as a
string, a list or another instance. `p == nil`, `p == 5` and `p == true` are
compared in the usual way without calling it, so a check for `nil` always
behaves the same, whatever the class defines. `@eq` has to return a bool,
and returning anything else raises a `TypeError`. Without `@eq`, two
instances are only equal if they're the same object.

`using` also uses `@eq` for matching, since it compares values the same way
`==` does.

`@eq` only affects `==`. Lists and dictionaries compare the instances they
contain by identity, and so do `contains()`, `index_of()` and dictionary
keys:

```zuri
class Point {
  @new(x, y) {
    self.x = x
    self.y = y
  }

  @eq(other) {
    if !instance_of(other, Point) {
      return false
    }

    return self.x == other.x and self.y == other.y
  }
}

var p = Point(1, 2)

echo [p] == [Point(1, 2)]
echo [p].contains(Point(1, 2))
echo [p].contains(p)
```

```console
false
false
true
```

## Iteration

Together, `@key` and `@value` make a class work with `for ... in`. They take
the most care to get right of all the decorated methods, so they have their
own section: [Making a Class Iterable](ch06-05-iterable-classes.md).

| Decorator | Called by | Signature |
| --- | --- | --- |
| `@key` | `for ... in` | `@key(previous)` |
| `@value` | `for ... in` | `@value(key)` |

## `@to_json`

`json.encode()` calls `@to_json()` on an instance and encodes whatever it
returns:

```zuri
import json

class User {
  @new(name, password) {
    self.name = name
    self.password = password
  }

  @to_json() {
    return { name: self.name }
  }
}

echo json.encode(User('ada', 'hunter2'))
```

```console
{"name":"ada"}
```

Without `@to_json()`, there's nothing for the encoder to work with. With it,
you decide exactly what gets sent, which makes it the right place to leave
out things like a password.

## `@to_string`

`@to_string()` controls what `echo` and `print()` display for an instance:

```zuri
class Money {
  @new(cents) {
    self.cents = cents
  }

  @to_string() {
    return '$' + (self.cents / 100)
  }
}

var m = Money(500)

echo m
echo [m, Money(250)]
echo { total: m }
```

```console
$5
[$5, $2.5]
{total: $5}
```

It applies wherever the instance appears in the output, including inside
lists and dictionaries, as a key or as a value. Without it, an instance is
displayed as `<instance of Money>`. `@to_string()` has to return a string,
and returning anything else raises a `TypeError`.

## `to_string()`

`to_string()` doesn't have an `@` because it isn't a decorated method. It's
a regular method that every value already has, and a class can override it
to give its instances a plain string form.

Nothing calls it for you. String interpolation and `+` show an instance as
`<instance of Money>`, so call `to_string()` inside the interpolation
yourself. A class that defines both usually implements one in terms of the
other:

```zuri
class Money {
  @new(cents) {
    self.cents = cents
  }

  to_string() {
    return '$' + (self.cents / 100)
  }

  @to_string() {
    return '<Money ${self.to_string()}>'
  }
}

var m = Money(500)

echo m
echo 'cost: ${m.to_string()}'
```

```console
<Money $5>
cost: $5
```

This is the convention the standard library follows: `to_string()` returns
the value as text, and `@to_string()` controls how the instance looks when
you print it.

Keep both methods fast and free of side effects, because error messages,
logging and debugging all rely on them.
