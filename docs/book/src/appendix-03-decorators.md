# Appendix C: Decorated Methods

A method whose name begins with `@` is called by the runtime when a piece
of syntax is applied to an instance of its class. They are ordinary
methods otherwise: inherited, overridable, and callable by name.

See [Decorated Methods](ch06-03-decorated-methods.md) for the guided
treatment, and [Class Extensions](ch06-06-class-extensions.md) for adding
one to a class you did not write.

## Construction

| Decorator | Called by | Signature |
| --- | --- | --- |
| `@new` | `ClassName(...)` | `@new(...args)` |

`@new` is the only place `self.x = value` may declare a field that was not
declared with `var`.

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

`@not` is bound to `~`, the bitwise complement. `!` is logical negation and
is not overridable: an instance is always truthy, so `!instance` is always
`false`.

## Comparison

| Decorator | Operator | Signature |
| --- | --- | --- |
| `@lt` | `a < b` | `@lt(other)` |
| `@lte` | `a <= b` | `@lte(other)` |
| `@gt` | `a > b` | `@gt(other)` |
| `@gte` | `a >= b` | `@gte(other)` |

There is no `@eq`. `==` on instances compares identity and cannot be
overridden. Write a plain `equals()` method instead; that is the
convention the standard library follows.

## Iteration

| Decorator | Called by | Signature |
| --- | --- | --- |
| `@key` | `for ... in` | `@key(previous)` |
| `@value` | `for ... in` | `@value(key)` |

`@key(previous)` receives the previous key, starting from `nil`, and
returns the next one or `nil` when the sequence is finished. `@value(key)`
returns what is stored at that key.

Defining both is what makes `is_iterable()` return `true` for the class.

## Serialisation

| Decorator | Called by | Signature |
| --- | --- | --- |
| `@to_json` | `json.encode()` | `@to_json()` |

Returns whatever should be encoded in the instance's place, which is where
you decide what does and does not cross the wire.

## Not a Decorator

`to_string()` has no `@`. It is a real method every value already carries,
and a class may override it.

`echo` does **not** call it; it prints `<instance of ClassName>`. String
interpolation and `+` do go through it.

## Resolution

The left operand decides. `a + b` looks for `@add` on `a`'s class only; if
`a` is a number and `b` is your instance, the operation is a `TypeError`
rather than a call to `b`'s `@add`.

An operator with no matching decorator raises a `TypeError` naming the
exact signature:

```console
operator '+' not defined for call signature (nil, number)
```
