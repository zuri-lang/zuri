# Appendix B: Operators and Precedence

## Precedence

The table lists operators from highest to lowest precedence. Operators on
the same row have equal precedence and associate from left to right, except
for `**`, which associates from right to left.

| Level | Operators | Notes |
| --- | --- | --- |
| 1 | Literals, `(...)`, `[...]`, `{...}`, `self`, `parent` | |
| 2 | `..` | Only binds primary expressions |
| 3 | `.` `()` `[]` | Member access, calls, indexing and slicing |
| 4 | `++` `--` | Postfix only |
| 5 | `**` | Right-associative. The exponent can have a unary operator. |
| 6 | `!` `-` `~` | Unary operators |
| 7 | `*` `/` `//` `%` | |
| 8 | `+` `-` | |
| 9 | `<<` `>>` `>>>` | |
| 10 | `&` | |
| 11 | `^` | |
| 12 | `\|` | |
| 13 | `<` `<=` `>` `>=` `==` `!=` | |
| 14 | `and` | |
| 15 | `or` | |
| 16 | `? :` | |
| 17 | `=` and the compound assignment operators | |

Some consequences to keep in mind:

- `2 ** 3 ** 2` is `512`, because `**` is right-associative.
- `2 * 3 ** 2` is `18`, because `**` has higher precedence than `*`.
- `-2 ** 2` is `-4`, because `**` binds more tightly than unary minus. Write
  `(-2) ** 2` to raise a negative number to a power.
- `2 ** -1` is `0.5`, because the exponent can have its own sign.
- `1 + 2..5` means `1 + (2..5)`, so put parentheses around ranges whose
  endpoints are calculated.

## Arithmetic Operators

| Operator | Meaning | Decorator |
| --- | --- | --- |
| `+` | Addition, and string and list concatenation | `@add` |
| `-` | Subtraction | `@sub` |
| `*` | Multiplication, and string and list repetition | `@mul` |
| `/` | Division, always floating point | `@div` |
| `//` | Floor division, which rounds toward negative infinity | `@floordiv` |
| `%` | Remainder, with the sign of the left operand | `@mod` |
| `**` | Exponentiation | `@pow` |
| `-` (unary) | Negation | `@neg` |

## Comparison Operators

| Operator | Meaning | Decorator |
| --- | --- | --- |
| `==` | Equal. Compares numbers, strings, lists and dictionaries by value, and everything else by identity. | `@eq` |
| `!=` | Not equal, the opposite of `==` | `@eq` |
| `<` | Less than, for numbers only | `@lt` |
| `<=` | Less than or equal to | `@lte` |
| `>` | Greater than | `@gt` |
| `>=` | Greater than or equal to | `@gte` |

`@eq` only runs when both operands are objects, so `x == nil` and `x == 5`
never call it.

## Logical Operators

| Operator | Meaning |
| --- | --- |
| `and` | True if both sides are truthy. Returns one of the operands, not a bool. |
| `or` | True if either side is truthy. Returns one of the operands, not a bool. |
| `!` | Logical negation, which always returns a bool |
| `? :` | The conditional expression |

## Bitwise Operators

| Operator | Meaning | Decorator |
| --- | --- | --- |
| `&` | And | `@and` |
| `\|` | Or | `@or` |
| `^` | Exclusive or | `@xor` |
| `~` | Complement | `@not` |
| `<<` | Left shift | `@lshift` |
| `>>` | Arithmetic right shift | `@rshift` |
| `>>>` | Logical right shift, which fills with zeros | `@urshift` |

## Assignment Operators

| Operator | Equivalent to |
| --- | --- |
| `=` | Assignment |
| `+=` `-=` `*=` `/=` `//=` `**=` `%=` | `x = x op y` |
| `&=` `\|=` `^=` `~=` `<<=` `>>=` `>>>=` | `x = x op y` |
| `++` `--` | Increment and decrement. Postfix only, and evaluates to the **new** value. |

## Access Operators

| Syntax | Meaning |
| --- | --- |
| `x.name` | A member of an object, dictionary or module |
| `x[key]` | Index with a computed key |
| `x[a, b]` | Slice from `a` up to but not including `b` |
| `x[, b]` | Slice from the start |
| `x[a, ]` | Slice to the end |
| `a..b` | A range |
| `...x` | A variadic parameter, only in a parameter list |

Negative indexes count back from the end, for strings, lists and bytes.

## Truthiness

These values are falsy: `false`, `nil`, `0`, `0.0`, `-0.0`, `NaN`, `0n`,
`''` and `bytes(0)`.

Everything else is truthy, including negative numbers, `[]`, `{}` and `'0'`.

## Operators Zuri Doesn't Have

There's no `in` operator for checking membership, so use `contains()`
instead. There's no optional chaining with `?.`, and no null coalescing with
`??`, but `or` covers that, with the truthiness caveat described above.
There's no comma operator, and no prefix `++` or `--`.
