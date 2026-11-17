# Operators

## Arithmetic

| Operator | Meaning | Example |
| --- | --- | --- |
| `+` | Addition | `2 + 3` is `5` |
| `-` | Subtraction | `5 - 2` is `3` |
| `*` | Multiplication | `4 * 3` is `12` |
| `/` | Division (always floating point) | `7 / 2` is `3.5` |
| `//` | Floor division | `7 // 2` is `3` |
| `%` | Remainder | `7 % 3` is `1` |
| `**` | Exponentiation | `2 ** 10` is `1024` |

`/` never truncates. `7 / 2` is `3.5` even though both operands look like
integers, because Zuri has only one number type. If you want an integer
result, use `//`.

`//` rounds **toward negative infinity**, and the result of `%` has the same
sign as the left operand:

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

`%` also works with numbers that aren't whole: `7.5 % 2` is `1.5`.

## Comparison

| Operator | Meaning |
| --- | --- |
| `==` | Equal |
| `!=` | Not equal |
| `<` `<=` `>` `>=` | Ordering (numbers only) |

`==` compares numbers, strings, lists and dictionaries by value, and
everything else by identity. A class can change how its instances are
compared by defining [`@eq`](ch06-03-decorated-methods.md#equality).

```zuri
echo 1 == 1.0
echo [1, 2] == [1, 2]
echo { a: 1 } == { a: 1 }
echo '1' == 1
```

```console
true
true
true
false
```

`==` never converts types, so a string is never equal to a number.

The ordering operators only work with numbers. Comparing two strings with
`<` raises a `TypeError`. Use `compare()` instead, which returns `-1`, `0`
or `1`:

```zuri
echo 'abc'.compare('abd')
```

```console
-1
```

## Logical Operators

Zuri uses words for these:

| Operator | Meaning |
| --- | --- |
| `and` | True when both sides are truthy |
| `or` | True when either side is truthy |
| `!` | Negation |

`and` and `or` short-circuit, and they return **one of their operands**
rather than a boolean:

```zuri
echo 1 and 2
echo nil or 'fallback'
```

```console
2
fallback
```

That's why `var name = given or 'anonymous'` works. Keep in mind that `0`,
`NaN`, `''` and `false` are all falsy, so only use this idiom when none of
those are valid values.

## The Conditional Operator

```zuri
echo true ? 'yes' : 'no'
```

```console
yes
```

`cond ? a : b` evaluates `cond`, and then evaluates only one of the two
branches.

## Bitwise Operators

| Operator | Meaning |
| --- | --- |
| `&` | And |
| `\|` | Or |
| `^` | Exclusive or |
| `~` | Not |
| `<<` | Left shift |
| `>>` | Arithmetic right shift (keeps the sign) |
| `>>>` | Logical right shift (fills with zeros) |

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

When one side of `+` is a string, `+` concatenates, and a number on the
other side is converted to a string:

```zuri
echo 'n=' + 5
```

```console
n=5
```

With lists, `+` concatenates and `*` repeats:

```zuri
echo [1, 2] + [3]
echo [1, 2] * 2
```

```console
[1, 2, 3]
[1, 2, 1, 2]
```

Dictionaries don't support `+`. Use `extend()` instead.

If you use an operator with types it doesn't support, you get a `TypeError`
that shows the exact combination of types:

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

Classes can define what `+` and the other operators do for their instances.
That's covered in [Chapter 6](ch06-03-decorated-methods.md).

## Member Access, Indexing and Slicing

| Syntax | Meaning |
| --- | --- |
| `x.name` | A member of an object, dictionary or module |
| `x[key]` | Index with a computed key |
| `x[a, b]` | Slice from `a` up to but not including `b` |
| `x[, b]` | Slice from the start |
| `x[a, ]` | Slice to the end |
| `a..b` | A range |

Negative indexes count back from the end, for both strings and lists.

## Precedence

The table lists operators from highest to lowest precedence. Operators on
the same row have equal precedence and associate from left to right, except
for `**`, which associates from right to left.

| Level | Operators |
| --- | --- |
| 1 | Literals, `(...)`, `[...]`, `{...}`, `self`, `parent` |
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
| 17 | `=` and the compound assignment operators |

**`**` follows the usual mathematical convention.** It binds more tightly
than multiplication and more tightly than a unary operator in front of it,
and it groups from the right:

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

`2 * 3 ** 2` means `2 * (3 ** 2)`, `2 ** 3 ** 2` means `2 ** (3 ** 2)`, and
`-2 ** 2` means `-(2 ** 2)`. The exponent can have its own sign, so
`2 ** -1` doesn't need parentheses. To raise a negative number to a power,
write `(-2) ** 2`.

**`..` binds very tightly**, to primary expressions only. `1 + 2..5` is
parsed as `1 + (2..5)`, so put parentheses around a range whose endpoints
are expressions.

## Operators Zuri Doesn't Have

There's no `in` operator for checking membership, so use `contains()`
instead. There's no optional chaining with `?.` and no null coalescing with
`??`, but `or` handles the most common case. There's also no comma operator,
and you can't use `+=` on a member that doesn't exist yet.
