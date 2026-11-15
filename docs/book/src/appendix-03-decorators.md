# Appendix C: Decorated Methods

A method whose name starts with `@` is called by the runtime when a
particular piece of syntax is used with an instance of its class. Apart from
that, decorated methods are ordinary methods: they can be inherited,
overridden and called by name.

See [Decorated Methods](ch06-03-decorated-methods.md) for a full
explanation, and [Class Extensions](ch06-06-class-extensions.md) for adding
one to a class you didn't write.

## Construction

| Decorator | Called by | Signature |
| --- | --- | --- |
| `@new` | `ClassName(...)` | `@new(...args)` |

`@new` is the only place where `self.x = value` can declare a field that
wasn't declared with `var`.

## Arithmetic

| Decorator | Operator | Signature |
| --- | --- | --- |
| `@add` | `a + b` | `@add(other)` |
| `@sub` | `a - b` | `@sub(other)` |
| `@mul` | `a * b` | `@mul(other)` |
| `@div` | `a / b` | `@div(other)` |
| `@floordiv` | `a // b` | `@floordiv(other)` |
| `@mod` | `a % b` | `@mod(other)` |
| `@pow` | `a ** b` | `@pow(other)` |
| `@neg` | `-a` | `@neg()` |

## Bitwise

| Decorator | Operator | Signature |
| --- | --- | --- |
| `@and` | `a & b` | `@and(other)` |
| `@or` | `a \| b` | `@or(other)` |
| `@xor` | `a ^ b` | `@xor(other)` |
| `@not` | `~a` | `@not()` |
| `@lshift` | `a << b` | `@lshift(other)` |
| `@rshift` | `a >> b` | `@rshift(other)` |
| `@urshift` | `a >>> b` | `@urshift(other)` |

`@not` is bound to `~`, the bitwise complement. `!` is logical negation, and
it can't be overridden. An instance is always truthy, so `!instance` is
always `false`.

## Comparison

| Decorator | Operator | Signature |
| --- | --- | --- |
| `@eq` | `a == b`, `a != b` | `@eq(other)` |
| `@lt` | `a < b` | `@lt(other)` |
| `@lte` | `a <= b` | `@lte(other)` |
| `@gt` | `a > b` | `@gt(other)` |
| `@gte` | `a >= b` | `@gte(other)` |

`!=` is the opposite of `@eq`. `@eq` only runs when the right operand is
also an object, so `x == nil` never calls it, and it must return a bool.
Lists, dictionaries, `contains()` and `index_of()` compare instances by
identity, whether or not their class defines `@eq`.

## Iteration

| Decorator | Called by | Signature |
| --- | --- | --- |
| `@key` | `for ... in` | `@key(previous)` |
| `@value` | `for ... in` | `@value(key)` |

`@key(previous)` receives the previous key, starting with `nil`, and returns
the next key, or `nil` when there are no more. `@value(key)` returns the
value stored under that key.

Defining both methods makes `is_iterable()` return `true` for the class.

## Display

| Decorator | Called by | Signature |
| --- | --- | --- |
| `@to_string` | `echo`, `print()` | `@to_string()` |

`@to_string()` returns the text displayed for the instance, including when
it appears inside a list or dictionary that's being displayed, as a key or
as a value. It must return a string, and returning anything else raises a
`TypeError`. Without it, an instance is displayed as
`<instance of ClassName>`.

## Serialization

| Decorator | Called by | Signature |
| --- | --- | --- |
| `@to_json` | `json.encode()` | `@to_json()` |

Returns whatever should be encoded in place of the instance, which lets you
decide exactly what gets sent.

## `to_string()` Isn't a Decorated Method

`to_string()` doesn't have an `@`. It's a regular method that every value
already has, and a class can override it.

Nothing calls it automatically. String interpolation and `+` display an
instance as `<instance of ClassName>`, and `echo` and `print()` use
`@to_string()`.

## Which Method Is Called

The left operand decides. `a + b` only looks for `@add` on the class of `a`.
If `a` is a number and `b` is an instance of your class, the operation
raises a `TypeError`, rather than calling `b`'s `@add`.

An operator without a matching decorated method raises a `TypeError` that
shows the exact combination of types:

```console
operator '+' not defined for call signature (nil, number)
```
