# Bigint Methods

Every method of the built-in `bigint` type, with its signature, what it
returns, and any behavior you might not expect.

| Method | Returns | Summary |
| --- | --- | --- |
| [`to_string(radix)`](#to_string) | `string` | Returns the decimal digits of the bigint, with a leading `-` if it's negative and without the trailing `n`. |
| [`to_number()`](#to_number) | `number` | Converts the bigint to a `number`. |
| [`to_bool()`](#to_bool) | `boolean` | Converts the bigint to a boolean, by the same rule `if` uses. |
| [`to_bytes(order)`](#to_bytes) | `bytes` | Returns the bigint as bytes in two's complement, which includes the sign, so the bytes convert back to the same value. |
| [`bin()`](#bin) | `string` | Returns the binary digits of the bigint. |
| [`hex()`](#hex) | `string` | Returns the base-16 digits in lowercase, equivalent to `to_string(16)`. |
| [`oct()`](#oct) | `string` | Returns the base-8 digits, equivalent to `to_string(8)`. |
| [`abs()`](#abs) | `bigint` | Returns the absolute value. |
| [`sign()`](#sign) | `number` | Returns the sign as a plain `number`: `1` when positive, `-1` when negative and `0` for zero. |
| [`max(other)`](#max) | `bigint` | Returns the larger of the two bigints. |
| [`min(other)`](#min) | `bigint` | Returns the smaller of the two bigints. |
| [`pow(exponent)`](#pow) | `bigint` | Raises the bigint to the power `exponent`. |
| [`sqrt()`](#sqrt) | `bigint` | Returns the integer square root, truncated toward zero, so `145n.sqrt()` is `12n`, not `12.04...`. |
| [`cbrt()`](#cbrt) | `bigint` | Returns the integer cube root, truncated towards zero. |
| [`nth_root(n)`](#nth_root) | `bigint` | Returns the integer `n`th root, truncated toward zero. |
| [`gcd(other)`](#gcd) | `bigint` | Returns the greatest common divisor of the two bigints. |
| [`lcm(other)`](#lcm) | `bigint` | Returns the least common multiple of the two bigints. |
| [`modpow(exponent, modulus)`](#modpow) | `bigint` | Returns `(self ** exponent) % modulus` without computing the full power, so it works with the very large exponents that cryptography uses. |
| [`modinv(modulus)`](#modinv) | `bigint\|nil` | Returns the modular multiplicative inverse, which is the `x` for which `self * x == 1 (mod modulus)`. |
| [`bits()`](#bits) | `number` | Returns how many bits the magnitude occupies, ignoring the sign. |
| [`bit(index)`](#bit) | `boolean` | Returns whether the bit at `index` is set, counting from the least significant bit, which is at index 0. |
| [`set_bit(index, value)`](#set_bit) | `bigint` | Returns a new bigint with the bit at `index` set or cleared. |
| [`trailing_zeros()`](#trailing_zeros) | `number\|nil` | Returns the count of least-significant zero bits, which is the largest power of two dividing the bigint. |
| [`is_zero()`](#is_zero) | `boolean` | Returns whether the bigint is zero. |
| [`is_even()`](#is_even) | `boolean` | Returns whether the bigint is even. |
| [`is_odd()`](#is_odd) | `boolean` | Returns whether the bigint is odd. |

## `to_string()`

```zuri,ignore
to_string(radix) -> string
```

Returns the decimal digits of the bigint, with a leading `-` if it's
negative and without the trailing `n`. Pass `radix` to use another base,
with lowercase letters for digits above nine.

```zuri-repl
%> 255n.to_string()
'255'
%> 255n.to_string(16)
'ff'
%> (-255n).to_string(16)
'-ff'
```

**Parameters**

- `radix` (`number`): the base to use, from 2 to 36. Defaults to 10.

**Returns** `string`

**Raises** `RangeError` if `radix` is outside 2 to 36.

> **Note:** The `n` that `echo` and string interpolation show is part of
> how a bigint is displayed, not part of the conversion. `to_string()`
> never includes it.

## `to_number()`

```zuri,ignore
to_number() -> number
```

Converts the bigint to a `number`.

```zuri-repl
%> 6n.to_number()
6
```

**Returns** `number`

> **Note:** This loses precision for anything above `2^53`, because the
> result is the nearest double. A value beyond the range of a double
> becomes `inf` or `-inf`, instead of wrapping around or becoming zero.
> Check `bits()` first if that matters.

## `to_bool()`

```zuri,ignore
to_bool() -> boolean
```

Converts the bigint to a boolean, by the same rule `if` uses. `0n` is
`false`, and every other bigint, negative ones included, is `true`.

```zuri-repl
%> 5n.to_bool()
true
%> 0n.to_bool()
false
%> (-5n).to_bool()
true
```

**Returns** `boolean`

## `to_bytes()`

```zuri,ignore
to_bytes(order) -> bytes
```

Returns the bigint as bytes in two's complement, which includes the
sign, so the bytes convert back to the same value. The result is the
shortest byte string that can hold the value, and it's never empty: zero
is a single `0x00` byte.

```zuri-repl
%> 258n.to_bytes()
(01 02)
%> 258n.to_bytes('little')
(02 01)
%> (-1n).to_bytes()
(ff)
```

**Parameters**

- `order` (`string`): `'big'` or `'little'`. Defaults to `'big'`.

**Returns** `bytes`

**Raises** `RangeError` if `order` is neither `'big'` nor `'little'`.

## `bin()`

```zuri,ignore
bin() -> string
```

Returns the binary digits of the bigint. This is the same as
`to_string(2)`.

```zuri-repl
%> 255n.bin()
'11111111'
```

**Returns** `string`

> **Note:** This uses sign and magnitude, so a negative bigint is returned
> with a leading `-`, not in two's complement. Use `to_bytes()` for two's
> complement.

## `hex()`

```zuri,ignore
hex() -> string
```

Returns the base-16 digits in lowercase, equivalent to `to_string(16)`.

```zuri-repl
%> 255n.hex()
'ff'
```

**Returns** `string`

## `oct()`

```zuri,ignore
oct() -> string
```

Returns the base-8 digits, equivalent to `to_string(8)`.

```zuri-repl
%> 255n.oct()
'377'
```

**Returns** `string`

## `abs()`

```zuri,ignore
abs() -> bigint
```

Returns the absolute value.

```zuri-repl
%> (-5n).abs()
5n
```

**Returns** `bigint`

## `sign()`

```zuri,ignore
sign() -> number
```

Returns the sign as a plain `number`: `1` when positive, `-1` when
negative and `0` for zero.

```zuri-repl
%> (-9n).sign()
-1
%> 0n.sign()
0
```

**Returns** `number`

## `max()`

```zuri,ignore
max(other) -> bigint
```

Returns the larger of the two bigints.

```zuri-repl
%> 3n.max(7n)
7n
```

**Parameters**

- `other` (`bigint`)

**Returns** `bigint`

**Raises** `TypeError` if `other` is not a bigint.

## `min()`

```zuri,ignore
min(other) -> bigint
```

Returns the smaller of the two bigints.

```zuri-repl
%> 3n.min(7n)
3n
```

**Parameters**

- `other` (`bigint`)

**Returns** `bigint`

**Raises** `TypeError` if `other` is not a bigint.

## `pow()`

```zuri,ignore
pow(exponent) -> bigint
```

Raises the bigint to the power `exponent`. This is the method form of
`**`.

```zuri-repl
%> 2n.pow(100)
1267650600228229401496703205376n
```

**Parameters**

- `exponent` (`number|bigint`): an integer from 0 to 2^32 - 1. A
  negative exponent has no integer result, so it's rejected instead of
  being truncated to zero.

**Returns** `bigint`

**Raises** `RangeError` if `exponent` is negative, fractional or too
large.

## `sqrt()`

```zuri,ignore
sqrt() -> bigint
```

Returns the integer square root, truncated toward zero, so `145n.sqrt()`
is `12n`, not `12.04...`.

```zuri-repl
%> 144n.sqrt()
12n
%> 145n.sqrt()
12n
```

**Returns** `bigint`

**Raises** `RangeError` if the bigint is negative.

## `cbrt()`

```zuri,ignore
cbrt() -> bigint
```

Returns the integer cube root, truncated towards zero. Negatives are
fine here, unlike `sqrt()`.

```zuri-repl
%> (-27n).cbrt()
-3n
```

**Returns** `bigint`

## `nth_root()`

```zuri,ignore
nth_root(n) -> bigint
```

Returns the integer `n`th root, truncated toward zero.

```zuri-repl
%> 1000000n.nth_root(3)
100n
```

**Parameters**

- `n` (`number|bigint`): an integer from 1 to 2^32 - 1.

**Returns** `bigint`

**Raises** `RangeError` if `n` is zero, negative, fractional or too
large, or if `n` is even and the bigint is negative.

## `gcd()`

```zuri,ignore
gcd(other) -> bigint
```

Returns the greatest common divisor of the two bigints. The result is
always non-negative regardless of either sign, and `0n.gcd(0n)` is `0n`.

```zuri-repl
%> 48n.gcd(18n)
6n
```

**Parameters**

- `other` (`bigint`)

**Returns** `bigint`

**Raises** `TypeError` if `other` is not a bigint.

## `lcm()`

```zuri,ignore
lcm(other) -> bigint
```

Returns the least common multiple of the two bigints. The result is
always non-negative, and is `0n` when either side is zero.

```zuri-repl
%> 48n.lcm(18n)
144n
```

**Parameters**

- `other` (`bigint`)

**Returns** `bigint`

**Raises** `TypeError` if `other` is not a bigint.

## `modpow()`

```zuri,ignore
modpow(exponent, modulus) -> bigint
```

Returns `(self ** exponent) % modulus` without computing the full power,
so it works with the very large exponents that cryptography uses.

```zuri-repl
%> 4n.modpow(13n, 497n)
445n
```

**Parameters**

- `exponent` (`bigint`)
- `modulus` (`bigint`): must not be zero.

**Returns** `bigint`

**Raises** `TypeError` if either argument isn't a bigint.

**Raises** `RangeError` if `modulus` is zero, or if `exponent` is
negative and there's no modular inverse.

> **Note:** The remainder is floored, not truncated, so the result has the
> sign of `modulus`, not of the receiver. A negative `exponent` is only
> allowed if the receiver has an inverse modulo `modulus`.

## `modinv()`

```zuri,ignore
modinv(modulus) -> bigint|nil
```

Returns the modular multiplicative inverse, which is the `x` for which
`self * x == 1 (mod modulus)`.

```zuri-repl
%> 3n.modinv(11n)
4n
%> 4n.modinv(8n)
nil
```

**Parameters**

- `modulus` (`bigint`): must not be zero.

**Returns** `bigint|nil`

**Raises** `TypeError` if `modulus` isn't a bigint.

**Raises** `RangeError` if `modulus` is zero.

> **Note:** If the receiver and `modulus` aren't coprime, this returns
> `nil` instead of raising an error, because having no inverse is an
> ordinary result, not a mistake by the caller. The result has the sign of
> `modulus`.

## `bits()`

```zuri,ignore
bits() -> number
```

Returns how many bits the magnitude occupies, ignoring the sign. Zero
occupies none.

```zuri-repl
%> 255n.bits()
8
%> 0n.bits()
0
```

**Returns** `number`

## `bit()`

```zuri,ignore
bit(index) -> boolean
```

Returns whether the bit at `index` is set, counting from the least
significant bit, which is at index 0.

```zuri-repl
%> 5n.bit(0)
true
%> 5n.bit(1)
false
```

**Parameters**

- `index` (`number`): a non-negative integer.

**Returns** `boolean`

**Raises** `RangeError` if `index` is negative or fractional.

> **Note:** The bigint is read as two's complement, so a negative bigint
> reports `true` for every index above its magnitude, instead of running
> out of bits.

## `set_bit()`

```zuri,ignore
set_bit(index, value) -> bigint
```

Returns a new bigint with the bit at `index` set or cleared. The
original bigint isn't changed.

```zuri-repl
%> 5n.set_bit(1, true)
7n
```

**Parameters**

- `index` (`number`): a non-negative integer.
- `value` (`boolean`)

**Returns** `bigint`

**Raises** `RangeError` if `index` is negative or fractional.

## `trailing_zeros()`

```zuri,ignore
trailing_zeros() -> number|nil
```

Returns the count of least-significant zero bits, which is the largest
power of two dividing the bigint.

```zuri-repl
%> 40n.trailing_zeros()
3
%> 0n.trailing_zeros()
nil
```

**Returns** `number|nil`

> **Note:** Returns `nil` for zero, which has no largest such power and
> would otherwise have to report an arbitrary number.

## `is_zero()`

```zuri,ignore
is_zero() -> boolean
```

Returns whether the bigint is zero.

```zuri-repl
%> 0n.is_zero()
true
```

**Returns** `boolean`

## `is_even()`

```zuri,ignore
is_even() -> boolean
```

Returns whether the bigint is even. Zero is even.

```zuri-repl
%> 4n.is_even()
true
```

**Returns** `boolean`

## `is_odd()`

```zuri,ignore
is_odd() -> boolean
```

Returns whether the bigint is odd.

```zuri-repl
%> 5n.is_odd()
true
```

**Returns** `boolean`
