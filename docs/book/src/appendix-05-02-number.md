# `number`

43 methods. See [Numbers](ch04-02-numbers.md) for the guided introduction.

| Method | Returns |
| --- | --- |
| [`to_string()`](#to_string) | `string` |
| [`to_bool()`](#to_bool) | `boolean` |
| [`to_bigint()`](#to_bigint) | `bigint` |
| [`abs()`](#abs) | `number` |
| [`chr()`](#chr) | `string` |
| [`bin()`](#bin) | `string` |
| [`hex()`](#hex) | `string` |
| [`oct()`](#oct) | `string` |
| [`int()`](#int) | `number` |
| [`max(other: number)`](#max) | `number` |
| [`min(other: number)`](#min) | `number` |
| [`factorial()`](#factorial) | `number` |
| [`sin()`](#sin) | `number` |
| [`cos()`](#cos) | `number` |
| [`tan()`](#tan) | `number` |
| [`sinh()`](#sinh) | `number` |
| [`cosh()`](#cosh) | `number` |
| [`tanh()`](#tanh) | `number` |
| [`asin()`](#asin) | `number` |
| [`acos()`](#acos) | `number` |
| [`atan()`](#atan) | `number` |
| [`atan2(x: number)`](#atan2) | `number` |
| [`asinh()`](#asinh) | `number` |
| [`acosh()`](#acosh) | `number` |
| [`atanh()`](#atanh) | `number` |
| [`exp()`](#exp) | `number` |
| [`expm1()`](#expm1) | `number` |
| [`log()`](#log) | `number` |
| [`log2()`](#log2) | `number` |
| [`log10()`](#log10) | `number` |
| [`log1p()`](#log1p) | `number` |
| [`cbrt()`](#cbrt) | `number` |
| [`sqrt()`](#sqrt) | `number` |
| [`sign()`](#sign) | `number` |
| [`ceil()`](#ceil) | `number` |
| [`round()`](#round) | `number` |
| [`floor()`](#floor) | `number` |
| [`is_nan()`](#is_nan) | `boolean` |
| [`is_inf()`](#is_inf) | `boolean` |
| [`is_finite()`](#is_finite) | `boolean` |
| [`trunc()`](#trunc) | `number` |
| [`fraction()`](#fraction) | `number` |
| [`fixed(n)`](#fixed) |  |

## `to_string()`

Returns the string representation of the number.

```zuri
%> 5.to_string()
'5'
```

- **Returns** `string`

## `to_bool()`

Converts the number to a boolean. A number is considered
truthy (`true`) when it is greater than or equal to zero,
and falsy (`false`) when it is negative.

```zuri
%> 5.to_bool()
true
%> 0.to_bool()
true
%> (-5).to_bool()
false
```

- **Returns** `boolean`

## `to_bigint()`

Converts the number to a `bigint`, the counterpart to
`bigint.to_number()`.

```zuri
%> (12345).to_bigint()
12345n
%> (2).to_bigint() ** (100).to_bigint()
1267650600228229401496703205376n
```

   fractional number, `Infinity` and `NaN` all raise rather
   than being rounded or clamped. Numbers above `2^53` are
   already imprecise as doubles, so converting one yields the
   exact integer the double holds, not the decimal literal it
   was written as.

- **Note** Only an exact integer has a `bigint` form, so a
- **Returns** `bigint`
- **Raises** `RangeError` if the number is not an exact integer.

## `abs()`

Returns the absolute value of the number.

```zuri
%> (-5).abs()
5
%> 5.abs()
5
```

- **Returns** `number`

## `chr()`

Returns the Unicode character whose code point is equal to
the number.

```zuri
%> 65.chr()
'A'
```

- **Returns** `string`

## `bin()`

Converts the number to its binary string representation. The
number is truncated to an integer first.

```zuri
%> 10.bin()
'1010'
```

   signed or two's-complement form.

- **Note** A negative number always returns `'0'`; there is no
- **Returns** `string`

## `hex()`

Converts the number to its hexadecimal string
representation. The number is truncated to an integer first.

```zuri
%> 255.hex()
'ff'
```

   signed or two's-complement form.

- **Note** A negative number always returns `'0'`; there is no
- **Returns** `string`

## `oct()`

Converts the number to its octal string representation. The
number is truncated to an integer first.

```zuri
%> 8.oct()
'10'
```

   signed or two's-complement form.

- **Note** A negative number always returns `'0'`; there is no
- **Returns** `string`

## `int()`

Truncates the number down to its integer part, discarding
anything after the decimal point. Unlike `floor()`, this
rounds towards zero rather than towards negative infinity,
so the result for a negative number differs from `floor()`.

```zuri
%> 3.9.int()
3
%> (-3.9).int()
-3
```

- **Returns** `number`

## `max(other: number)`

Returns the larger of the number and _other_.

```zuri
%> 5.max(9)
9
%> 9.max(5)
9
```

- **Parameter** `number` other The number to compare against.
- **Returns** `number`

## `min(other: number)`

Returns the smaller of the number and _other_.

```zuri
%> 5.min(9)
5
%> 9.min(5)
5
```

- **Parameter** `number` other The number to compare against.
- **Returns** `number`

## `factorial()`

Returns the factorial of the number, i.e. the product of
every positive integer less than or equal to it.
`0.factorial()` is `1`, matching the standard mathematical
definition.

```zuri
%> 5.factorial()
120
%> 0.factorial()
1
```

   number.

- **Raises** `Error` if the number is negative or not a whole
- **Returns** `number`

## `sin()`

Returns the sine of the number, taken to be in radians.

- **Returns** `number`

## `cos()`

Returns the cosine of the number, taken to be in radians.

- **Returns** `number`

## `tan()`

Returns the tangent of the number, taken to be in radians.

- **Returns** `number`

## `sinh()`

Returns the hyperbolic sine of the number.

- **Returns** `number`

## `cosh()`

Returns the hyperbolic cosine of the number.

- **Returns** `number`

## `tanh()`

Returns the hyperbolic tangent of the number.

- **Returns** `number`

## `asin()`

Returns the arcsine (inverse sine) of the number, in
radians.

   inclusive; outside that range, this returns `NaN` rather
   than raising an error.

- **Note** Only defined for a receiver between `-1` and `1`
- **Returns** `number`

## `acos()`

Returns the arccosine (inverse cosine) of the number, in
radians.

   inclusive; outside that range, this returns `NaN` rather
   than raising an error.

- **Note** Only defined for a receiver between `-1` and `1`
- **Returns** `number`

## `atan()`

Returns the arctangent (inverse tangent) of the number, in
radians.

- **Returns** `number`

## `atan2(x: number)`

Returns the four-quadrant arctangent of the number and _x_,
in radians. The receiver is treated as the y-coordinate and
_x_ as the x-coordinate, matching the conventional `atan2(y,
x)` signature: `y.atan2(x)`.

```zuri
%> 1.0.atan2(1.0)
0.7853981633974483
```

- **Parameter** `number` x The x-coordinate.
- **Returns** `number`

## `asinh()`

Returns the inverse hyperbolic sine of the number.

- **Returns** `number`

## `acosh()`

Returns the inverse hyperbolic cosine of the number.

   `1`; below that, this returns `NaN` rather than raising
   an error.

- **Note** Only defined for a receiver greater than or equal to
- **Returns** `number`

## `atanh()`

Returns the inverse hyperbolic tangent of the number.

   exclusive; outside that range, this returns `NaN` rather
   than raising an error.

- **Note** Only defined for a receiver between `-1` and `1`
- **Returns** `number`

## `exp()`

Returns _e_ (Euler's number) raised to the power of the
number.

- **Returns** `number`

## `expm1()`

Returns _e_ raised to the power of the number, minus `1`.
For a number close to zero, this is more numerically
accurate than computing `n.exp() - 1` directly.

- **Returns** `number`

## `log()`

Returns the natural logarithm (base _e_) of the number.

```zuri
%> 1.0.log()
0
```

- **Returns** `number`

## `log2()`

Returns the base-2 logarithm of the number.

```zuri
%> 8.0.log2()
3
```

- **Returns** `number`

## `log10()`

Returns the base-10 logarithm of the number.

```zuri
%> 100.0.log10()
2
```

- **Returns** `number`

## `log1p()`

Returns the natural logarithm of `1` plus the number. For a
number close to zero, this is more numerically accurate than
computing `(1 + n).log()` directly.

- **Returns** `number`

## `cbrt()`

Returns the cube root of the number.

```zuri
%> 27.0.cbrt()
3
```

- **Returns** `number`

## `sqrt()`

Returns the square root of the number.

```zuri
%> 16.sqrt()
4
```

   raising an error; there is no `bigint`-style promotion
   into complex numbers. Check `is_nan()` on the result, or
   the sign of the receiver beforehand, if that distinction
   matters to the caller.

- **Note** For a negative number this returns `NaN` rather than
- **Returns** `number`

## `sign()`

Returns the sign of the number: `1` if it is positive, `-1`
if it is negative, and `0` (with its own original sign
preserved) if it is zero.

```zuri
%> 7.sign()
1
%> (-7).sign()
-1
%> 0.sign()
0
```

- **Returns** `number`

## `ceil()`

Returns the smallest whole number greater than or equal to
the number.

```zuri
%> 3.14159.ceil()
4
```

- **Returns** `number`

## `round()`

Rounds the number to the nearest whole number. A value
exactly halfway between two whole numbers rounds away from
zero.

```zuri
%> 3.14159.round()
3
%> 3.6.round()
4
```

- **Returns** `number`

## `floor()`

Returns the largest whole number less than or equal to the
number.

```zuri
%> 3.14159.floor()
3
```

- **Returns** `number`

## `is_nan()`

Returns `true` if the number is NaN (not a number, e.g. the
result of `0/0`), `false` otherwise.

- **Returns** `boolean`

## `is_inf()`

Returns `true` if the number is positive or negative
infinity, `false` otherwise.

- **Returns** `boolean`

## `is_finite()`

Returns `true` if the number is neither infinite nor NaN,
`false` otherwise.

- **Returns** `boolean`

## `trunc()`

Truncates the number towards zero, discarding anything after
the decimal point. For values that fit in a 64-bit integer
this matches `int()`; unlike `int()`, `trunc()` stays a
floating-point result rather than going through an integer
cast, so it does not overflow for numbers larger than a
64-bit integer can hold.

```zuri
%> (-3.9).trunc()
-3
```

- **Returns** `number`

## `fraction()`

Returns the digits after the number's decimal point, read as
a whole number rather than a fraction. Note that this is NOT
the same as `(n - n.int())`: `1.92.fraction()` is `92`, not
`0.92`.

```zuri
%> 1.92.fraction()
92
%> 1.5.fraction()
5
%> 5.fraction()
0
```

- **Returns** `number`

## `fixed(n)`

Returns the number rounded to _n_ decimal places, with a half
rounding away from zero the same way `round()` does.

A number already shorter than _n_ places is returned unchanged, and
so are `NaN` and the infinities. Beyond 17 places an `f64` has no
digits left to round, so a larger _n_ behaves as 17.

```zuri
%> 1.554576852757686786786.fixed(9)
1.554576853
%> 1.554576852757686786786.fixed(8)
1.55457685
%> 1.554576852757686786786.fixed(1)
1.6
%> 1.554576852757686786786.fixed(0)
2
%> (-2.5).fixed(0)
-3
```
