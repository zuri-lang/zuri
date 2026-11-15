# Numbers

Zuri has a single number type, `number`, which is a 64-bit IEEE 754 double.
Whole numbers and fractions are the same type. What usually matters is
whether a particular value happens to be a whole number.

## Number Literals

There are five ways to write a number literal, and they all produce the same
type:

```zuri
echo 42
echo 3.5
echo 0b1011
echo 0c17
echo 0xff
echo 6.02e23
echo 1.5e-3
```

```console
42
3.5
11
15
255
602000000000000000000000
0.0015
```

`0b` is binary, `0c` is octal and `0x` is hexadecimal. The letters in a
hexadecimal literal can be uppercase or lowercase, so `0xff` and `0xFF` are
the same number. Note that octal uses `0c`, not the `0o` that some other
languages use.

The exponent form uses `e` or `E`, and the exponent can be negative. `1e3`
is `1000`, and the part before the `e` doesn't need a decimal point. The
exponent is only a way of *writing* the literal, though. The value doesn't
remember it, so `6.02e23` prints in full decimal form.

### Digit Separators

Underscores between digits are ignored, which makes long numbers easier to
read:

```zuri
echo 1_000_000
echo 1_0.5_5
```

```console
1000000
10.55
```

**Separators only work in decimal literals.** `0xdead_beef` and
`0b1010_1010` are syntax errors.

### Literals That Aren't Allowed

A literal needs a digit on both sides of the decimal point, so neither of
these parses:

```zuri,ignore
echo .5
echo 5.
```

Write `0.5` and `5.0` instead. The second case matters more than you might
think, because `5.` is also how a method call on a literal begins, as you'll
see two sections from now.

## Math Methods

Everything you'd normally find in a math library is available as a method on
the number:

```zuri
echo 2.sqrt()
echo 8.log2()
echo 100.log10()
echo 100.log()
echo 1.exp()
echo 2.cbrt()
```

```console
1.4142135623730951
3
2
4.605170185988092
2.718281828459045
1.2599210498948732
```

`log()` is the natural logarithm. There are also `log1p()` and `expm1()`,
which are more precise for values close to zero.

All of the trigonometric functions are available: `sin`, `cos`, `tan`,
`asin`, `acos`, `atan` and `atan2`, along with the hyperbolic `sinh`,
`cosh`, `tanh`, `asinh`, `acosh` and `atanh`.

```zuri
echo 1.atan2(1)
```

```console
0.7853981633974483
```

## Calling a Method on a Literal

You can call a method directly on a number literal, without parentheses or a
temporary variable. This works in every base, with or without a decimal
point:

```zuri
echo 2.sqrt()
echo 3.7.round()
echo 255.hex()
echo 0xff.bin()
echo 1e3.int()
echo 2n.bits()
```

```console
1.4142135623730951
4
ff
11111111
1000
2
```

The one case that needs parentheses is a **negative** literal. A method call
binds more tightly than unary minus, so the minus applies to the result of
the call instead of the number:

```zuri
echo -3.abs()
echo (-3).abs()
```

```console
-3
3
```

`-3.abs()` means `-(3.abs())`, which is `-3`. When the number is negative,
put it in parentheses, or assign it to a variable first so the question
doesn't come up:

```zuri
var n = -3

echo n.abs()
```

```console
3
```

The same goes for any expression you want to call a method on. Wrap it in
parentheses, because otherwise the call only applies to the last part of the
expression.

```zuri
echo (1 / 0).is_inf()
echo (2 ** 10).hex()
```

```console
true
400
```

## Rounding

```zuri
echo 2.5.ceil()
echo 2.5.floor()
echo 2.5.trunc()
echo 2.5.int()
echo (-2.5).int()
```

```console
3
2
2
2
-2
```

`trunc()` and `int()` both remove the fractional part, rounding toward zero.

`round()` rounds halves away from zero:

```zuri
echo 2.5.round()
echo 3.5.round()
echo (-2.5).round()
```

```console
3
4
-3
```

`fixed()` rounds to a given number of decimal places and returns a number:

```zuri
echo 2.567.fixed(2)
```

```console
2.57
```

`fraction()` returns the digits after the decimal point as a whole number:

```zuri
echo 2.567.fraction()
```

```console
567
```

## Sign, Magnitude and Comparison

```zuri
echo (-5).sign()
echo (-3).abs()
echo 5.max(9)
echo 5.min(9)
```

```console
-1
3
9
5
```

`sign()` returns `-1`, `0` or `1`.

## Bases and Characters

```zuri
echo 255.bin()
echo 255.oct()
echo 255.hex()
echo 65.chr()
```

```console
11111111
377
ff
A
```

`chr()` converts a code point into a one-character string, and `'A'.ord()`
does the reverse.

## Special Values

```zuri
echo (0 / 0).is_nan()
echo (1 / 0).is_inf()
echo 1.is_finite()
```

```console
true
true
true
```

`NaN` is **falsy**, like zero, so `var x = a / b or fallback` replaces the
`NaN` from `0 / 0` with the fallback. Use `is_nan()` when you need to tell
`NaN` and zero apart.

As the IEEE standard requires, `NaN` isn't equal to itself, so `x == x` is
false only when `x` is `NaN`.

## Other Methods

`factorial()` works on small integers, and `to_string()`, `to_bool()` and
`to_bigint()` convert numbers to other types.

```zuri
echo 5.factorial()
echo 17.to_bigint()
```

```console
120
17n
```

## The `math` Module

Constants are defined in the `math` module, since a constant doesn't belong
to any particular number:

```zuri
import math

echo math.PI
echo math.E
echo math.Infinity
echo math.NaN
```

```console
3.141592653589793
2.718281828459045
inf
NaN
```

The module also defines `LOG_2`, `LOG_10`, `LOG_2_E`, `LOG_10_E`, `ROOT_2`,
`ROOT_3` and `ROOT_HALF`.

## Bigints

When 2^53 isn't big enough, use a bigint. To write one, add an `n` suffix:

```zuri
var a = 2n ** 100n
echo a
echo a.bits()
```

```console
1267650600228229401496703205376n
101
```

Bigints have arbitrary precision. They never overflow, and they never lose a
digit.

They also never mix with numbers automatically:

```zuri
catch {
  echo 5n + 3
} as e {
  echo e.message
}
```

```console
operator '+' not defined for call signature (bigint, number)
```

Convert between them explicitly, in whichever direction you need:

```zuri
echo 5.to_bigint() * 2n
echo (2n ** 100n).to_number()
```

```console
10n
1267650600228229400000000000000
```

Converting to `number` loses precision once you go past 2^53, which is
exactly why bigints exist. Converting the other way is always exact.

Type annotations keep the two types separate as well. `bigint` is a type
name alongside `number` and `int`, and it only accepts bigints:

```zuri
def scale(n: bigint, factor: number) {
  return n * factor.to_bigint()
}

echo scale(2n, 50)

catch {
  scale(2, 50)
} as e {
  echo e.message
}
```

```console
100n
scale() expects parameter 'n' (argument 1) to be a bigint, got number
```

The number theory methods are where bigints really earn their keep:

```zuri
echo 100n.gcd(75n)
echo 100n.lcm(75n)
echo 2n.modpow(10n, 1000n)
echo 3n.modinv(11n)
echo 144n.sqrt()
```

```console
25n
300n
24n
4n
12n
```

`modpow(exponent, modulus)` is the operation that public-key cryptography is
built on, and it's computed without ever calculating the full power.
`modinv` returns the modular multiplicative inverse.

There are also `nth_root()`, `cbrt()`, `bit()`, `set_bit()`,
`trailing_zeros()`, `is_zero()`, `is_even()`, `is_odd()`, `abs()`, `sign()`,
`max()`, `min()` and `pow()`, as well as `bin()`, `oct()` and `hex()`.

Use a bigint when you need exact values beyond 2^53, for example in
cryptography, for money stored in cents, for factorials, or for identifiers
that have to survive a round trip unchanged. For everything else, such as
measurements, coordinates, counters and ratios, use a `number`.
