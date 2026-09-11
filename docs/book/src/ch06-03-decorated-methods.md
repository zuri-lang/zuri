# Decorated Methods

A method whose name begins with `@` is a **decorated method**. The runtime
calls it for you when a piece of language syntax is applied to your
instance. That is how a class hooks into construction, arithmetic,
comparison and iteration without any special syntax of its own.

Decorated methods are ordinary methods. They can be inherited, overridden
and called by name.

## `@new`: Construction

Already covered. It runs when the class is called, receives the arguments,
and is the one place `self.x = value` may declare a new field.

## Arithmetic

Define the operator you want and it works on your instances:

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

The full arithmetic set:

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

## Bitwise and Logic

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

`@not` is bound to `~`, the bitwise complement. `!` is logical negation and
it is not overridable: an instance is always truthy, so `!instance` is
always `false`.

## Comparison

| Decorator | Operator |
| --- | --- |
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

There is **no `@eq`**. `==` on instances compares identity, and that is not
overridable. When you need value equality, write a plain `equals()` method
and call it:

```zuri
class Point {
  @new(x, y) {
    self.x = x
    self.y = y
  }

  equals(other) {
    return self.x == other.x and self.y == other.y
  }
}

var p = Point(1, 2)

echo p == Point(1, 2)
echo p.equals(Point(1, 2))
```

```console
false
true
```

That is a convention the standard library follows everywhere, so it reads
as idiomatic rather than as a workaround.

## Iteration

`@key` and `@value` together make a class work with `for ... in`.

`@key(previous)` is handed the previous key, starting with `nil`, and
returns the next one, or `nil` when the sequence is finished.
`@value(key)` returns what is stored at that key.

```zuri
class Countdown {
  @new(from) {
    self.from = from
  }

  @key(previous) {
    if previous == nil {
      return self.from
    }
    if previous <= 1 {
      return nil
    }
    return previous - 1
  }

  @value(key) {
    return key
  }
}

for n in Countdown(3) {
  echo n
}
```

```console
3
2
1
```

Defining both is also what makes `is_iterable()` answer `true` for your
class.

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

Without it, encoding an instance has nothing to work from. With it, you
decide exactly what crosses the wire, which is the right place to leave a
password behind.

## `to_string()`

`to_string()` has no `@` because it is not a decorator; it is a real method
every value already has, and a class may override it:

```zuri
class Money {
  @new(cents) {
    self.cents = cents
  }

  to_string() {
    return '$' + (self.cents / 100)
  }
}

echo Money(500).to_string()
```

```console
$5
```

**`echo` does not call `to_string()`.** `echo` prints a value's built-in
representation, and for an instance that is `<instance of Money>`. Call the
method when you want your own text:

```zuri
var m = Money(500)

echo m
echo m.to_string()
```

```console
<instance of Money>
$5
```

String interpolation and `+` go through `to_string()`, so those do what you
expect:

```zuri
echo 'cost: ${m.to_string()}'
```

Keep `to_string()` cheap and free of side effects. Error messages,
logging and debugging all reach for it.
