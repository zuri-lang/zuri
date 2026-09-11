# Numbers

There is one numeric type, `number`, and it is a 64-bit IEEE-754 double.
Integers and fractions are the same type, and the distinction you care
about is whether a particular value happens to be integral.

## Methods, Not Functions

Everything you would reach into a math library for is a method on the
number:

```zuri
echo (2).sqrt()
echo (8).log2()
echo (100).log10()
echo (100).log()
echo (1).exp()
echo (2).cbrt()
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
echo (1).atan2(1)
```

```console
0.7853981633974483
```

## Parentheses Around Literals

`2.sqrt()` does not parse. The lexer sees `2.` and starts reading a
floating-point number. Wrap the literal:

```zuri
echo (2).sqrt()
```

A variable needs no parentheses, and neither does a literal that already
has a decimal point:

```zuri
var n = 2
echo n.sqrt()
echo 3.7.round()
```

## Rounding

```zuri
echo (2.5).ceil()
echo (2.5).floor()
echo (2.5).trunc()
echo (2.5).int()
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
echo (2.5).round()
echo (3.5).round()
echo (-2.5).round()
```

```console
3
4
-3
```

`fixed()` rounds to a number of decimal places and gives you a number back:

```zuri
echo (2.567).fixed(2)
```

```console
2.57
```

`fraction()` gives the digits after the decimal point as a whole number:

```zuri
echo (2.567).fraction()
```

```console
567
```

## Sign, Magnitude and Comparison

```zuri
echo (-5).sign()
echo (-3).abs()
echo (5).max(9)
echo (5).min(9)
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
echo (255).bin()
echo (255).oct()
echo (255).hex()
echo (65).chr()
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
echo (1).is_finite()
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
echo (5).factorial()
echo (17).to_bigint()
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
echo (5).to_bigint() * 2n
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
echo (100n).gcd(75n)
echo (100n).lcm(75n)
echo (2n).modpow(10n, 1000n)
echo (3n).modinv(11n)
echo (144n).sqrt()
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

Bigints are slower than numbers by a wide margin, because every operation
allocates. Use them where precision is the point, and numbers everywhere
else.
