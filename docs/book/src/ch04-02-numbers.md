# Numbers

There is one numeric type, `number`, and it is a 64-bit IEEE-754 double.
Integers and fractions are the same type, and the distinction you care
about is whether a particular value happens to be integral.

## Writing a Number

There are five ways to write a numeric literal, and they all produce the
same type:

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

`0b` is binary, `0c` is octal, and `0x` is hexadecimal. The letters in a
hexadecimal literal may be either case, so `0xff` and `0xFF` are the same
number. Note that octal is `0c`, not the `0o` some other languages use.

The exponent form takes `e` or `E`, and the exponent may be negative.
`1e3` is `1000`; the mantissa needs no decimal point. Note that the
exponent is a way of *writing* the literal, not a property the value keeps:
`6.02e23` prints as its full decimal expansion, because that is the number
it is.

### Digit Separators

An underscore between digits is ignored, which makes long numbers
readable:

```zuri
echo 1_000_000
echo 1_0.5_5
```

```console
1000000
10.55
```

**Separators work in decimal literals only.** `0xdead_beef` and
`0b1010_1010` are syntax errors, not clever formatting.

### Two Forms That Do Not Exist

A literal needs a digit on both sides of its decimal point. Neither of
these parses:

```zuri,ignore
echo .5
echo 5.
```

Write `0.5` and `5.0`. The second case matters more than it looks, because
`5.` is also how a method call on a literal starts — which is the subject
of the next-but-one section.

## Methods, Not Functions

Everything you would reach into a math library for is a method on the
number:

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

`log()` is the natural logarithm. There is also `log1p()` and `expm1()`
for the precision-sensitive forms near zero.

The full trigonometric set is there: `sin`, `cos`, `tan`, `asin`, `acos`,
`atan`, `atan2`, and the hyperbolic `sinh`, `cosh`, `tanh`, `asinh`,
`acosh`, `atanh`.

```zuri
echo 1.atan2(1)
```

```console
0.7853981633974483
```

## Calling a Method on a Literal

A numeric literal takes a method directly. No parentheses, no temporary
variable, in every base and with a decimal point or without:

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

There is exactly one case where you need parentheses, and it is a
**negative** literal. A method call binds tighter than unary minus, so the
minus applies to the result rather than to the number:

```zuri
echo -3.abs()
echo (-3).abs()
```

```console
-3
3
```

`-3.abs()` is `-(3.abs())`, which is `-3`. When the receiver is negative,
parenthesise it — or put it in a variable, where the question disappears:

```zuri
var n = -3

echo n.abs()
```

```console
3
```

The same rule covers any expression you want to call a method on: wrap it,
because the call would otherwise bind to the last term alone.

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

`trunc()` and `int()` both drop the fractional part, rounding towards zero.

`round()` rounds half away from zero:

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

`fixed()` rounds to a number of decimal places and gives you a number back:

```zuri
echo 2.567.fixed(2)
```

```console
2.57
```

`fraction()` gives the digits after the decimal point as a whole number:

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

`sign()` is `-1`, `0` or `1`.

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

`chr()` turns a code point into a one-character string; `'A'.ord()` goes
back the other way.

## The Special Values

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

`NaN` is **truthy**, which catches people out. `var x = a / b or fallback`
does not protect you from a `0 / 0`. Test with `is_nan()`.

`NaN` is also not equal to itself, as the standard requires, so
`x == x` is a valid way to spot one.

## Other Methods

`factorial()` for small integers, `to_string()`, `to_bool()` and
`to_bigint()` for conversion.

```zuri
echo 5.factorial()
echo 17.to_bigint()
```

```console
120
17n
```

## The `math` Module

Constants live in `math`, because a constant is not a method on anything:

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

It also carries `LOG_2`, `LOG_10`, `LOG_2_E`, `LOG_10_E`, `ROOT_2`,
`ROOT_3` and `ROOT_HALF`.

## Bigints

When 2^53 is not enough, use a bigint. Write one with an `n` suffix:

```zuri
var a = 2n ** 100n
echo a
echo a.bits()
```

```console
1267650600228229401496703205376n
101
```

Bigints are arbitrary precision. They never overflow and never lose a
digit.

They also never mix with numbers implicitly:

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

Convert explicitly, in whichever direction you need:

```zuri
echo 5.to_bigint() * 2n
echo (2n ** 100n).to_number()
```

```console
10n
1267650600228229400000000000000
```

Going to `number` is lossy once you are past 2^53, which is the whole
reason bigints exist. Going the other way is exact.

The same separation holds in type annotations. `bigint` is a type name
alongside `number` and `int`, and it accepts nothing else:

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

The number-theory methods are the reason bigints are worth having:

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

`modpow(exponent, modulus)` is the operation every public-key algorithm is
built on, and it is computed without ever materialising the full power.
`modinv` gives the modular multiplicative inverse.

There is also `nth_root()`, `cbrt()`, `bit()`, `set_bit()`,
`trailing_zeros()`, `is_zero()`, `is_even()`, `is_odd()`, `abs()`,
`sign()`, `max()`, `min()`, `pow()`, and `bin()`/`oct()`/`hex()`.

A bigint is the right choice when exactness past 2^53 is the point:
cryptography, currency in minor units, factorials, identifiers that must
survive a round trip. For everything else — measurements, coordinates,
counters, ratios — a `number` is the type you want.
