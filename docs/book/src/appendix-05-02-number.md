# Number Methods

Every method on the built-in `number` type, with its signature, what it
returns, and the cases where it does something other than the obvious
thing.

| Method | Returns | Summary |
| --- | --- | --- |
| [`to_string()`](#to_string) | `string` | Returns the string representation of the number. |
| [`to_bool()`](#to_bool) | `boolean` | Converts the number to a boolean. |
| [`to_bigint()`](#to_bigint) | `bigint` | Converts the number to a `bigint`, the counterpart to `bigint.to_number()`. |
| [`abs()`](#abs) | `number` | Returns the absolute value of the number. |
| [`chr()`](#chr) | `string` | Returns the Unicode character whose code point is equal to the number. |
| [`bin()`](#bin) | `string` | Converts the number to its binary string representation. |
| [`hex()`](#hex) | `string` | Converts the number to its hexadecimal string representation. |
| [`oct()`](#oct) | `string` | Converts the number to its octal string representation. |
| [`int()`](#int) | `number` | Truncates the number down to its integer part, discarding anything after the decimal point. |
| [`max(other: number)`](#max) | `number` | Returns the larger of the number and _other_. |
| [`min(other: number)`](#min) | `number` | Returns the smaller of the number and _other_. |
| [`factorial()`](#factorial) | `number` | Returns the factorial of the number, i.e. |
| [`sin()`](#sin) | `number` | Returns the sine of the number, taken to be in radians. |
| [`cos()`](#cos) | `number` | Returns the cosine of the number, taken to be in radians. |
| [`tan()`](#tan) | `number` | Returns the tangent of the number, taken to be in radians. |
| [`sinh()`](#sinh) | `number` | Returns the hyperbolic sine of the number. |
| [`cosh()`](#cosh) | `number` | Returns the hyperbolic cosine of the number. |
| [`tanh()`](#tanh) | `number` | Returns the hyperbolic tangent of the number. |
| [`asin()`](#asin) | `number` | Returns the arcsine (inverse sine) of the number, in radians. |
| [`acos()`](#acos) | `number` | Returns the arccosine (inverse cosine) of the number, in radians. |
| [`atan()`](#atan) | `number` | Returns the arctangent (inverse tangent) of the number, in radians. |
| [`atan2(x: number)`](#atan2) | `number` | Returns the four-quadrant arctangent of the number and _x_, in radians. |
| [`asinh()`](#asinh) | `number` | Returns the inverse hyperbolic sine of the number. |
| [`acosh()`](#acosh) | `number` | Returns the inverse hyperbolic cosine of the number. |
| [`atanh()`](#atanh) | `number` | Returns the inverse hyperbolic tangent of the number. |
| [`exp()`](#exp) | `number` | Returns _e_ (Euler's number) raised to the power of the number. |
| [`expm1()`](#expm1) | `number` | Returns _e_ raised to the power of the number, minus `1`. |
| [`log()`](#log) | `number` | Returns the natural logarithm (base _e_) of the number. |
| [`log2()`](#log2) | `number` | Returns the base-2 logarithm of the number. |
| [`log10()`](#log10) | `number` | Returns the base-10 logarithm of the number. |
| [`log1p()`](#log1p) | `number` | Returns the natural logarithm of `1` plus the number. |
| [`cbrt()`](#cbrt) | `number` | Returns the cube root of the number. |
| [`sqrt()`](#sqrt) | `number` | Returns the square root of the number. |
| [`sign()`](#sign) | `number` | Returns the sign of the number: `1` if it is positive, `-1` if it is negative, and `0` (with its own original sign preserved) if it is zero. |
| [`ceil()`](#ceil) | `number` | Returns the smallest whole number greater than or equal to the number. |
| [`round()`](#round) | `number` | Rounds the number to the nearest whole number. |
| [`floor()`](#floor) | `number` | Returns the largest whole number less than or equal to the number. |
| [`is_nan()`](#is_nan) | `boolean` | Returns `true` if the number is NaN (not a number, e.g. |
| [`is_inf()`](#is_inf) | `boolean` | Returns `true` if the number is positive or negative infinity, `false` otherwise. |
| [`is_finite()`](#is_finite) | `boolean` | Returns `true` if the number is neither infinite nor NaN, `false` otherwise. |
| [`trunc()`](#trunc) | `number` | Truncates the number towards zero, discarding anything after the decimal point. |
| [`fraction()`](#fraction) | `number` | Returns the digits after the number's decimal point, read as a whole number rather than a fraction. |
| [`fixed(n)`](#fixed) |  | Returns the number rounded to _n_ decimal places, with a half rounding away from zero the same way `round()` does. |

## `to_string()`

```zuri,ignore
to_string() -> string
```

Returns the string representation of the number.

```zuri,ignore
%> 5.to_string()
'5'
```

**Returns** `string`

## `to_bool()`

```zuri,ignore
to_bool() -> boolean
```

Converts the number to a boolean. A number is considered truthy (`true`)
when it is greater than or equal to zero, and falsy (`false`) when it is
negative.

```zuri,ignore
%> 5.to_bool()
true
%> 0.to_bool()
true
%> (-5).to_bool()
false
```

**Returns** `boolean`

## `to_bigint()`

```zuri,ignore
to_bigint() -> bigint
```

Converts the number to a `bigint`, the counterpart to
`bigint.to_number()`.

```zuri,ignore
%> (12345).to_bigint()
12345n
%> (2).to_bigint() ** (100).to_bigint()
1267650600228229401496703205376n
```

**Returns** `bigint`

**Raises** `RangeError` if the number is not an exact integer.

> **Note:** Only an exact integer has a `bigint` form, so a fractional
> number, `Infinity` and `NaN` all raise rather than being rounded or
> clamped. Numbers above `2^53` are already imprecise as doubles, so
> converting one yields the exact integer the double holds, not the
> decimal literal it was written as.

## `abs()`

```zuri,ignore
abs() -> number
```

Returns the absolute value of the number.

```zuri,ignore
%> (-5).abs()
5
%> 5.abs()
5
```

**Returns** `number`

## `chr()`

```zuri,ignore
chr() -> string
```

Returns the Unicode character whose code point is equal to the number.

```zuri,ignore
%> 65.chr()
'A'
```

**Returns** `string`

## `bin()`

```zuri,ignore
bin() -> string
```

Converts the number to its binary string representation. The number is
truncated to an integer first.

```zuri,ignore
%> 10.bin()
'1010'
```

**Returns** `string`

> **Note:** A negative number always returns `'0'`; there is no signed or
> two's-complement form.

## `hex()`

```zuri,ignore
hex() -> string
```

Converts the number to its hexadecimal string representation. The number
is truncated to an integer first.

```zuri,ignore
%> 255.hex()
'ff'
```

**Returns** `string`

> **Note:** A negative number always returns `'0'`; there is no signed or
> two's-complement form.

## `oct()`

```zuri,ignore
oct() -> string
```

Converts the number to its octal string representation. The number is
truncated to an integer first.

```zuri,ignore
%> 8.oct()
'10'
```

**Returns** `string`

> **Note:** A negative number always returns `'0'`; there is no signed or
> two's-complement form.

## `int()`

```zuri,ignore
int() -> number
```

Truncates the number down to its integer part, discarding anything after
the decimal point. Unlike `floor()`, this rounds towards zero rather
than towards negative infinity, so the result for a negative number
differs from `floor()`.

```zuri,ignore
%> 3.9.int()
3
%> (-3.9).int()
-3
```

**Returns** `number`

## `max()`

```zuri,ignore
max(other: number) -> number
```

Returns the larger of the number and _other_.

```zuri,ignore
%> 5.max(9)
9
%> 9.max(5)
9
```

**Parameters**

- `other` (`number`) — The number to compare against.

**Returns** `number`

## `min()`

```zuri,ignore
min(other: number) -> number
```

Returns the smaller of the number and _other_.

```zuri,ignore
%> 5.min(9)
5
%> 9.min(5)
5
```

**Parameters**

- `other` (`number`) — The number to compare against.

**Returns** `number`

## `factorial()`

```zuri,ignore
factorial() -> number
```

Returns the factorial of the number, i.e. the product of every positive
integer less than or equal to it. `0.factorial()` is `1`, matching the
standard mathematical definition.

```zuri,ignore
%> 5.factorial()
120
%> 0.factorial()
1
```

**Returns** `number`

**Raises** `Error` if the number is negative or not a whole number.

## `sin()`

```zuri,ignore
sin() -> number
```

Returns the sine of the number, taken to be in radians.

**Returns** `number`

## `cos()`

```zuri,ignore
cos() -> number
```

Returns the cosine of the number, taken to be in radians.

**Returns** `number`

## `tan()`

```zuri,ignore
tan() -> number
```

Returns the tangent of the number, taken to be in radians.

**Returns** `number`

## `sinh()`

```zuri,ignore
sinh() -> number
```

Returns the hyperbolic sine of the number.

**Returns** `number`

## `cosh()`

```zuri,ignore
cosh() -> number
```

Returns the hyperbolic cosine of the number.

**Returns** `number`

## `tanh()`

```zuri,ignore
tanh() -> number
```

Returns the hyperbolic tangent of the number.

**Returns** `number`

## `asin()`

```zuri,ignore
asin() -> number
```

Returns the arcsine (inverse sine) of the number, in radians.

**Returns** `number`

> **Note:** Only defined for a receiver between `-1` and `1` inclusive;
> outside that range, this returns `NaN` rather than raising an error.

## `acos()`

```zuri,ignore
acos() -> number
```

Returns the arccosine (inverse cosine) of the number, in radians.

**Returns** `number`

> **Note:** Only defined for a receiver between `-1` and `1` inclusive;
> outside that range, this returns `NaN` rather than raising an error.

## `atan()`

```zuri,ignore
atan() -> number
```

Returns the arctangent (inverse tangent) of the number, in radians.

**Returns** `number`

## `atan2()`

```zuri,ignore
atan2(x: number) -> number
```

Returns the four-quadrant arctangent of the number and _x_, in radians.
The receiver is treated as the y-coordinate and _x_ as the x-coordinate,
matching the conventional `atan2(y, x)` signature: `y.atan2(x)`.

```zuri,ignore
%> 1.0.atan2(1.0)
0.7853981633974483
```

**Parameters**

- `x` (`number`) — The x-coordinate.

**Returns** `number`

## `asinh()`

```zuri,ignore
asinh() -> number
```

Returns the inverse hyperbolic sine of the number.

**Returns** `number`

## `acosh()`

```zuri,ignore
acosh() -> number
```

Returns the inverse hyperbolic cosine of the number.

**Returns** `number`

> **Note:** Only defined for a receiver greater than or equal to `1`;
> below that, this returns `NaN` rather than raising an error.

## `atanh()`

```zuri,ignore
atanh() -> number
```

Returns the inverse hyperbolic tangent of the number.

**Returns** `number`

> **Note:** Only defined for a receiver between `-1` and `1` exclusive;
> outside that range, this returns `NaN` rather than raising an error.

## `exp()`

```zuri,ignore
exp() -> number
```

Returns _e_ (Euler's number) raised to the power of the number.

**Returns** `number`

## `expm1()`

```zuri,ignore
expm1() -> number
```

Returns _e_ raised to the power of the number, minus `1`. For a number
close to zero, this is more numerically accurate than computing `n.exp()
- 1` directly.

**Returns** `number`

## `log()`

```zuri,ignore
log() -> number
```

Returns the natural logarithm (base _e_) of the number.

```zuri,ignore
%> 1.0.log()
0
```

**Returns** `number`

## `log2()`

```zuri,ignore
log2() -> number
```

Returns the base-2 logarithm of the number.

```zuri,ignore
%> 8.0.log2()
3
```

**Returns** `number`

## `log10()`

```zuri,ignore
log10() -> number
```

Returns the base-10 logarithm of the number.

```zuri,ignore
%> 100.0.log10()
2
```

**Returns** `number`

## `log1p()`

```zuri,ignore
log1p() -> number
```

Returns the natural logarithm of `1` plus the number. For a number close
to zero, this is more numerically accurate than computing `(1 +
n).log()` directly.

**Returns** `number`

## `cbrt()`

```zuri,ignore
cbrt() -> number
```

Returns the cube root of the number.

```zuri,ignore
%> 27.0.cbrt()
3
```

**Returns** `number`

## `sqrt()`

```zuri,ignore
sqrt() -> number
```

Returns the square root of the number.

```zuri,ignore
%> 16.sqrt()
4
```

**Returns** `number`

> **Note:** For a negative number this returns `NaN` rather than raising
> an error; there is no `bigint`-style promotion into complex numbers.
> Check `is_nan()` on the result, or the sign of the receiver beforehand,
> if that distinction matters to the caller.

## `sign()`

```zuri,ignore
sign() -> number
```

Returns the sign of the number: `1` if it is positive, `-1` if it is
negative, and `0` (with its own original sign preserved) if it is zero.

```zuri,ignore
%> 7.sign()
1
%> (-7).sign()
-1
%> 0.sign()
0
```

**Returns** `number`

## `ceil()`

```zuri,ignore
ceil() -> number
```

Returns the smallest whole number greater than or equal to the number.

```zuri,ignore
%> 3.14159.ceil()
4
```

**Returns** `number`

## `round()`

```zuri,ignore
round() -> number
```

Rounds the number to the nearest whole number. A value exactly halfway
between two whole numbers rounds away from zero.

```zuri,ignore
%> 3.14159.round()
3
%> 3.6.round()
4
```

**Returns** `number`

## `floor()`

```zuri,ignore
floor() -> number
```

Returns the largest whole number less than or equal to the number.

```zuri,ignore
%> 3.14159.floor()
3
```

**Returns** `number`

## `is_nan()`

```zuri,ignore
is_nan() -> boolean
```

Returns `true` if the number is NaN (not a number, e.g. the result of
`0/0`), `false` otherwise.

**Returns** `boolean`

## `is_inf()`

```zuri,ignore
is_inf() -> boolean
```

Returns `true` if the number is positive or negative infinity, `false`
otherwise.

**Returns** `boolean`

## `is_finite()`

```zuri,ignore
is_finite() -> boolean
```

Returns `true` if the number is neither infinite nor NaN, `false`
otherwise.

**Returns** `boolean`

## `trunc()`

```zuri,ignore
trunc() -> number
```

Truncates the number towards zero, discarding anything after the decimal
point. For values that fit in a 64-bit integer this matches `int()`;
unlike `int()`, `trunc()` stays a floating-point result rather than
going through an integer cast, so it does not overflow for numbers
larger than a 64-bit integer can hold.

```zuri,ignore
%> (-3.9).trunc()
-3
```

**Returns** `number`

## `fraction()`

```zuri,ignore
fraction() -> number
```

Returns the digits after the number's decimal point, read as a whole
number rather than a fraction. Note that this is NOT the same as `(n -
n.int())`: `1.92.fraction()` is `92`, not `0.92`.

```zuri,ignore
%> 1.92.fraction()
92
%> 1.5.fraction()
5
%> 5.fraction()
0
```

**Returns** `number`

## `fixed()`

```zuri,ignore
fixed(n)
```

Returns the number rounded to _n_ decimal places, with a half rounding
away from zero the same way `round()` does.

A number already shorter than _n_ places is returned unchanged, and so
are `NaN` and the infinities. Beyond 17 places an `f64` has no digits
left to round, so a larger _n_ behaves as 17.

```zuri,ignore
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
