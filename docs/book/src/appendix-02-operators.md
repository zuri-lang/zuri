# Appendix B: Operators and Precedence

## Precedence

Tightest first. Operators on the same row bind equally and associate left
to right, except `**`, which associates right to left.

| Level | Operators | Notes |
| --- | --- | --- |
| 1 | literals, `(...)`, `[...]`, `{...}`, `self`, `parent` | |
| 2 | `..` | binds primaries only |
| 3 | `.` `()` `[]` | member, call, index and slice |
| 4 | `++` `--` | postfix only |
| 5 | `**` | right-associative; the exponent may carry a unary operator |
| 6 | `!` `-` `~` | unary |
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
| 17 | `=` and every compound assignment | |

Consequences worth remembering:

- `2 ** 3 ** 2` is `512`. `**` is right-associative.
- `2 * 3 ** 2` is `18`. `**` outranks `*`.
- `-2 ** 2` is `-4`. `**` binds tighter than unary minus; write `(-2) ** 2`
  to raise a negative number.
- `2 ** -1` is `0.5`. The exponent may carry its own sign.
- `1 + 2..5` is `1 + (2..5)`. Parenthesise ranges with computed endpoints.

## Arithmetic

| Operator | Meaning | Decorator |
| --- | --- | --- |
| `+` | addition; string and list concatenation | `@add` |
| `-` | subtraction | `@sub` |
| `*` | multiplication; string and list repetition | `@mul` |
| `/` | division, always floating point | `@div` |
| `//` | floor division, rounds toward negative infinity | `@floordiv` |
| `%` | remainder, keeps the sign of the left operand | `@mod` |
| `**` | exponentiation | `@pow` |
| `-` (unary) | negation | `@neg` |

## Comparison

| Operator | Meaning | Decorator |
| --- | --- | --- |
| `==` | equal, by value for numbers, strings, lists, dicts; by identity otherwise | none |
| `!=` | not equal | none |
| `<` | less than, numbers only | `@lt` |
| `<=` | less than or equal | `@lte` |
| `>` | greater than | `@gt` |
| `>=` | greater than or equal | `@gte` |

`==` is not overridable. For value equality on your own class, write an
`equals()` method.

## Logic

| Operator | Meaning |
| --- | --- |
| `and` | both truthy; returns the operand, not a bool |
| `or` | either truthy; returns the operand, not a bool |
| `!` | logical negation, always a bool |
| `? :` | conditional expression |

## Bitwise

| Operator | Meaning | Decorator |
| --- | --- | --- |
| `&` | and | `@and` |
| `\|` | or | `@or` |
| `^` | xor | `@xor` |
| `~` | complement | `@not` |
| `<<` | left shift | `@lshift` |
| `>>` | arithmetic right shift | `@rshift` |
| `>>>` | logical right shift, zero filling | `@urshift` |

## Assignment

| Operator | Equivalent to |
| --- | --- |
| `=` | assignment |
| `+=` `-=` `*=` `/=` `//=` `**=` `%=` | `x = x op y` |
| `&=` `\|=` `^=` `~=` `<<=` `>>=` `>>>=` | `x = x op y` |
| `++` `--` | increment, decrement; postfix only, evaluates to the **new** value |

## Access

| Syntax | Meaning |
| --- | --- |
| `x.name` | member of an object, dictionary or module |
| `x[key]` | index with a computed key |
| `x[a, b]` | slice, from `a` up to but not including `b` |
| `x[, b]` | slice from the start |
| `x[a, ]` | slice to the end |
| `a..b` | a range value |
| `...x` | variadic parameter, in a parameter list only |

Negative indices count back from the end, for strings, lists and bytes.

## Truthiness

Falsy: `false`, `nil`, `0`, `0.0`, `-0.0`, **any negative number**, `0n`,
`''`, and `bytes(0)`.

Truthy: everything else, including `[]`, `{}`, `'0'` and `NaN`.

## Operators Zuri Does Not Have

No `in` for membership; use `contains()`. No `?.` optional chaining. No
`??` null coalescing; `or` covers it, with the truthiness caveat above. No
comma operator. No prefix `++`/`--`.
