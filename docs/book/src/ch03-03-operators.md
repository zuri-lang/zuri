# Operators

## Arithmetic

| Operator | Meaning | Example |
| --- | --- | --- |
| `+` | addition | `2 + 3` is `5` |
| `-` | subtraction | `5 - 2` is `3` |
| `*` | multiplication | `4 * 3` is `12` |
| `/` | division, always floating point | `7 / 2` is `3.5` |
| `//` | floor division | `7 // 2` is `3` |
| `%` | remainder | `7 % 3` is `1` |
| `**` | exponentiation | `2 ** 10` is `1024` |

`/` never truncates. `7 / 2` is `3.5` even though both operands look like
integers, because there is only one numeric type. When you want the
integer, ask for it with `//`.

`//` rounds **towards negative infinity**, while `%` keeps the sign of the
left operand:

```zuri
echo -7 // 2
echo 7 // -2
echo -7 % 3
echo 7 % -3
```

```console
-4
-4
-1
1
```

`%` works on non-integers too: `7.5 % 2` is `1.5`.

## Comparison

| Operator | Meaning |
| --- | --- |
| `==` | equal |
| `!=` | not equal |
| `<` `<=` `>` `>=` | ordering, numbers only |

`==` compares by value for numbers, strings, lists and dictionaries, and by
identity for everything else:

```zuri
echo 1 == 1.0
echo [1, 2] == [1, 2]
echo {a: 1} == {a: 1}
echo '1' == 1
```

```console
true
true
true
false
```

There is no coercion in `==`. A string is never equal to a number.

The ordering operators are for numbers. Comparing two strings with `<`
raises a `TypeError`; use `compare()`, which returns `-1`, `0` or `1`:

```zuri
echo 'abc'.compare('abd')
```

```console
-1
```

## Logic

Zuri spells these as words:

| Operator | Meaning |
| --- | --- |
| `and` | true when both sides are truthy |
| `or` | true when either side is truthy |
| `!` | negation |

`and` and `or` short-circuit, and they return **the operand**, not a
boolean:

```zuri
echo 1 and 2
echo nil or 'fallback'
```

```console
2
fallback
```

That is what makes `var name = given or 'anonymous'` work. Remember that
`0`, `NaN`, `''` and `false` are falsy, so this idiom is only safe when
those are not legitimate values.

## The Conditional Operator

```zuri
echo true ? 'yes' : 'no'
```

```console
yes
```

`cond ? a : b` evaluates `cond`, then exactly one of the branches.

## Bitwise

| Operator | Meaning |
| --- | --- |
| `&` | and |
| `\|` | or |
| `^` | xor |
| `~` | not |
| `<<` | left shift |
| `>>` | arithmetic right shift, sign preserving |
| `>>>` | logical right shift, zero filling |

```zuri
echo ~5
echo 5 >>> 1
echo 1 << 10
```

```console
-6
2
1024
```

Bitwise operators work on the integer value of a number, and on bigints.

## Concatenation and Repetition

`+` on a string concatenates, and a number on either side is converted:

```zuri
echo 'n=' + 5
```

```console
n=5
```

`+` on a list concatenates; `*` repeats:

```zuri
echo [1, 2] + [3]
echo [1, 2] * 2
```

```console
[1, 2, 3]
[1, 2, 1, 2]
```

Dictionaries do not support `+`. Use `extend()`.

Anything the language does not define raises a `TypeError` that names the
exact signature:

```zuri
catch {
  echo nil + 1
} as e {
  echo e.message
}
```

```console
operator '+' not defined for call signature (nil, number)
```

Classes can define what `+` and every other operator mean for their own
instances. That is [Chapter 6](ch06-03-decorated-methods.md).

## Member, Index and Slice

| Syntax | Meaning |
| --- | --- |
| `x.name` | member of an object, dictionary or module |
| `x[key]` | index with a computed key |
| `x[a, b]` | slice from `a` up to but not including `b` |
| `x[, b]` | slice from the start |
| `x[a, ]` | slice to the end |
| `a..b` | a range value |

Negative indices count back from the end, for both strings and lists.

## Precedence

From tightest to loosest. Everything on one row binds equally and
associates left to right, except `**`, which associates right to left.

| Level | Operators |
| --- | --- |
| 1 | literals, `(...)`, `[...]`, `{...}`, `self`, `parent` |
| 2 | `..` |
| 3 | `.` `()` `[]` |
| 4 | `++` `--` |
| 5 | `**` |
| 6 | `!` `-` `~` (unary) |
| 7 | `*` `/` `//` `%` |
| 8 | `+` `-` |
| 9 | `<<` `>>` `>>>` |
| 10 | `&` |
| 11 | `^` |
| 12 | `\|` |
| 13 | `<` `<=` `>` `>=` `==` `!=` |
| 14 | `and` |
| 15 | `or` |
| 16 | `? :` |
| 17 | `=` and every compound assignment |

**`**` follows the mathematical convention.** It binds tighter than
multiplication and tighter than a unary operator written before it, and it
groups from the right:

```zuri
echo 2 * 3 ** 2
echo 2 ** 3 ** 2
echo -2 ** 2
echo 2 ** -1
```

```console
18
512
-4
0.5
```

`2 * 3 ** 2` is `2 * (3 ** 2)`, `2 ** 3 ** 2` is `2 ** (3 ** 2)`, and
`-2 ** 2` is `-(2 ** 2)`. The exponent itself may carry a sign, so
`2 ** -1` needs no parentheses. Write `(-2) ** 2` to raise a negative
number.

**`..` binds very tightly**, to primaries only. `1 + 2..5` parses as
`1 + (2..5)`. Parenthesise any range whose endpoints are expressions.

## Operators Zuri Does Not Have

There is no `in` operator for membership; use `contains()`. There is no
`?.` optional chaining and no `??` null coalescing; `or` covers the common
case. There is no comma operator, and no `+=` on a member that does not
already exist.
