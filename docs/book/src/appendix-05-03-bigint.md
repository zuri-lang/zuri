# `bigint`

26 methods. See [Bigints](ch04-02-numbers.md) for the guided introduction.

| Method | Returns |
| --- | --- |
| [`to_string(radix)`](#to_string) | `string` |
| [`to_number()`](#to_number) | `number` |
| [`to_bool()`](#to_bool) | `boolean` |
| [`to_bytes(order)`](#to_bytes) | `bytes` |
| [`bin()`](#bin) | `string` |
| [`hex()`](#hex) | `string` |
| [`oct()`](#oct) | `string` |
| [`abs()`](#abs) | `bigint` |
| [`sign()`](#sign) | `number` |
| [`max(other)`](#max) | `bigint` |
| [`min(other)`](#min) | `bigint` |
| [`pow(exponent)`](#pow) | `bigint` |
| [`sqrt()`](#sqrt) | `bigint` |
| [`cbrt()`](#cbrt) | `bigint` |
| [`nth_root(n)`](#nth_root) | `bigint` |
| [`gcd(other)`](#gcd) | `bigint` |
| [`lcm(other)`](#lcm) | `bigint` |
| [`modpow(exponent, modulus)`](#modpow) | `bigint` |
| [`modinv(modulus)`](#modinv) | `bigint\|nil` |
| [`bits()`](#bits) | `number` |
| [`bit(index)`](#bit) | `boolean` |
| [`set_bit(index, value)`](#set_bit) | `bigint` |
| [`trailing_zeros()`](#trailing_zeros) | `number\|nil` |
| [`is_zero()`](#is_zero) | `boolean` |
| [`is_even()`](#is_even) | `boolean` |
| [`is_odd()`](#is_odd) | `boolean` |

## `to_string(radix)`

Returns the decimal digits of the bigint, with a leading `-`
when it is negative and no trailing `n`. Pass `radix` to
render in another base instead, using lowercase letters for
digit values above nine.

```zuri
%> (255n).to_string()
'255'
%> (255n).to_string(16)
'ff'
%> (-255n).to_string(16)
'-ff'
```

   part of the repr, not the conversion; `to_string()` never
   includes it.
   Defaults to 10.

- **Note** The `n` that `echo` and string interpolation show is
- **Parameter** `number` radix - base to render in, from 2 to 36.
- **Returns** `string`
- **Raises** `RangeError` if `radix` is outside 2 to 36.

## `to_number()`

Converts the bigint to a `number`.

```zuri
%> (6n).to_number()
6
```

   the nearest double, and a value past the double range
   becomes `inf` or `-inf` rather than wrapping or reading as
   zero. Check `bits()` beforehand when that matters.

- **Note** This is lossy for anything past `2^53`: the result is
- **Returns** `number`

## `to_bool()`

Converts the bigint to a boolean, following the same rule as
`number.to_bool()`: zero and up are truthy, negatives are
falsy.

```zuri
%> (5n).to_bool()
true
%> (0n).to_bool()
true
%> (-5n).to_bool()
false
```

- **Returns** `boolean`

## `to_bytes(order)`

Returns the two's-complement byte representation, which
carries the sign and so round-trips back to the same value.
The result is the shortest byte string that can hold it, and
is never empty: zero is a single `0x00` byte.

```zuri
%> (258n).to_bytes()
(01 02)
%> (258n).to_bytes('little')
(02 01)
%> (-1n).to_bytes()
(ff)
```

   `'big'`.
   `'little'`.

- **Parameter** `string` order - `'big'` or `'little'`. Defaults to
- **Returns** `bytes`
- **Raises** `RangeError` if `order` is neither `'big'` nor

## `bin()`

Returns the base-2 digits, equivalent to `to_string(2)`.

```zuri
%> (255n).bin()
'11111111'
```

   bigint comes back with a leading `-` rather than as two's
   complement. Use `to_bytes()` for the two's-complement
   view.

- **Note** This is the sign-and-magnitude form, so a negative
- **Returns** `string`

## `hex()`

Returns the base-16 digits in lowercase, equivalent to
`to_string(16)`.

```zuri
%> (255n).hex()
'ff'
```

- **Returns** `string`

## `oct()`

Returns the base-8 digits, equivalent to `to_string(8)`.

```zuri
%> (255n).oct()
'377'
```

- **Returns** `string`

## `abs()`

Returns the absolute value.

```zuri
%> (-5n).abs()
5n
```

- **Returns** `bigint`

## `sign()`

Returns the sign as a plain `number`: `1` when positive,
`-1` when negative and `0` for zero.

```zuri
%> (-9n).sign()
-1
%> (0n).sign()
0
```

- **Returns** `number`

## `max(other)`

Returns the larger of the two bigints.

```zuri
%> (3n).max(7n)
7n
```

- **Parameter** `bigint` other
- **Returns** `bigint`
- **Raises** `TypeError` if `other` is not a bigint.

## `min(other)`

Returns the smaller of the two bigints.

```zuri
%> (3n).min(7n)
3n
```

- **Parameter** `bigint` other
- **Returns** `bigint`
- **Raises** `TypeError` if `other` is not a bigint.

## `pow(exponent)`

Raises the bigint to `exponent`, the method form of `**`.

```zuri
%> (2n).pow(100)
1267650600228229401496703205376n
```

   2^32 - 1. Negative exponents have no integral answer and
   are rejected rather than truncated to zero.
   too large.

- **Parameter** `number|bigint` exponent - a integer from 0 to
- **Returns** `bigint`
- **Raises** `RangeError` if `exponent` is negative, fractional or

## `sqrt()`

Returns the integer square root, truncated towards zero, so
`145n.sqrt()` is `12n` rather than `12.04...`.

```zuri
%> (144n).sqrt()
12n
%> (145n).sqrt()
12n
```

- **Returns** `bigint`
- **Raises** `RangeError` if the bigint is negative.

## `cbrt()`

Returns the integer cube root, truncated towards zero.
Negatives are fine here, unlike `sqrt()`.

```zuri
%> (-27n).cbrt()
-3n
```

- **Returns** `bigint`

## `nth_root(n)`

Returns the integer `n`th root, truncated towards zero.

```zuri
%> (1000000n).nth_root(3)
100n
```

   2^32 - 1.
   too large, or if `n` is even and the bigint is negative.

- **Parameter** `number|bigint` n - a integer from 1 to
- **Returns** `bigint`
- **Raises** `RangeError` if `n` is zero, negative, fractional or

## `gcd(other)`

Returns the greatest common divisor of the two bigints. The
result is always non-negative regardless of either sign, and
`0n.gcd(0n)` is `0n`.

```zuri
%> (48n).gcd(18n)
6n
```

- **Parameter** `bigint` other
- **Returns** `bigint`
- **Raises** `TypeError` if `other` is not a bigint.

## `lcm(other)`

Returns the least common multiple of the two bigints. The
result is always non-negative, and is `0n` when either side
is zero.

```zuri
%> (48n).lcm(18n)
144n
```

- **Parameter** `bigint` other
- **Returns** `bigint`
- **Raises** `TypeError` if `other` is not a bigint.

## `modpow(exponent, modulus)`

Returns `(self ** exponent) % modulus` without ever building
the full power, which is what makes it usable for the huge
exponents cryptography needs.

```zuri
%> (4n).modpow(13n, 497n)
445n
```

   result carries the sign of `modulus`, not of the receiver.
   A negative `exponent` is allowed only when the receiver is
   invertible modulo `modulus`.
   is negative and no modular inverse exists.

- **Note** The remainder is floored rather than truncated, so the
- **Parameter** `bigint` exponent
- **Parameter** `bigint` modulus - must not be zero.
- **Returns** `bigint`
- **Raises** `TypeError` if either argument is not a bigint.
- **Raises** `RangeError` if `modulus` is zero, or if `exponent`

## `modinv(modulus)`

Returns the modular multiplicative inverse: the `x` solving
`self * x == 1 (mod modulus)`.

```zuri
%> (3n).modinv(11n)
4n
%> (4n).modinv(8n)
nil
```

   `modulus` are not coprime, since having no inverse is an
   ordinary answer and not a caller mistake. The result
   carries the sign of `modulus`.

- **Note** Returns `nil` rather than raising when the receiver and
- **Parameter** `bigint` modulus - must not be zero.
- **Returns** `bigint|nil`
- **Raises** `TypeError` if `modulus` is not a bigint.
- **Raises** `RangeError` if `modulus` is zero.

## `bits()`

Returns how many bits the magnitude occupies, ignoring the
sign. Zero occupies none.

```zuri
%> (255n).bits()
8
%> (0n).bits()
0
```

- **Returns** `number`

## `bit(index)`

Returns whether the bit at `index` is set, counting from the
least significant bit at index 0.

```zuri
%> (5n).bit(0)
true
%> (5n).bit(1)
false
```

   receiver reports `true` for every index above its
   magnitude rather than running out of bits.

- **Note** The bigint is read as two's complement, so a negative
- **Parameter** `number` index - a non-negative integer.
- **Returns** `boolean`
- **Raises** `RangeError` if `index` is negative or fractional.

## `set_bit(index, value)`

Returns a new bigint with the bit at `index` set or cleared.
The receiver is left untouched.

```zuri
%> (5n).set_bit(1, true)
7n
```

- **Parameter** `number` index - a non-negative integer.
- **Parameter** `boolean` value
- **Returns** `bigint`
- **Raises** `RangeError` if `index` is negative or fractional.

## `trailing_zeros()`

Returns the count of least-significant zero bits, which is
the largest power of two dividing the bigint.

```zuri
%> (40n).trailing_zeros()
3
%> (0n).trailing_zeros()
nil
```

   and would otherwise have to report an arbitrary number.

- **Note** Returns `nil` for zero, which has no largest such power
- **Returns** `number|nil`

## `is_zero()`

Returns whether the bigint is zero.

```zuri
%> (0n).is_zero()
true
```

- **Returns** `boolean`

## `is_even()`

Returns whether the bigint is even. Zero is even.

```zuri
%> (4n).is_even()
true
```

- **Returns** `boolean`

## `is_odd()`

Returns whether the bigint is odd.

```zuri
%> (5n).is_odd()
true
```

- **Returns** `boolean`
