# Data Types

Zuri has ten built-in types. `typeof()` names the one you are holding:

```zuri
var values = [1, 1.5, 'text', true, nil, [1], {a: 1}, 1..3, 10n, bytes(2)]

for v in values {
  echo typeof(v)
}
```

```console
number
number
string
bool
nil
list
dict
range
bigint
bytes
```

On top of those there are `function`, `class`, `instance`, `module`, `file`
and `ptr`, which you get from declaring things rather than from a literal.

## `number`

A number is an IEEE-754 double. There is no separate integer type, which
means `1` and `1.0` are the same value:

```zuri
echo 1 == 1.0
```

```console
true
```

`is_int()` asks whether a number's value is integral, not whether it was
written without a decimal point:

```zuri
echo is_int(1)
echo is_int(1.0)
echo is_int(1.5)
```

```console
true
true
false
```

Doubles hold integers exactly up to 2^53. Past that, precision goes:

```zuri
echo 9007199254740993
```

```console
9007199254740992
```

Numeric literals come in five forms:

```zuri
var decimal = 1_000_000
var binary = 0b1010          # 10
var octal = 0c17             # 15
var hexadecimal = 0xff       # 255
var scientific = 6.02e23

echo [decimal, binary, octal, hexadecimal, scientific]
```

```console
[1000000, 10, 15, 255, 602000000000000000000000]
```

Underscores are digit separators, and they are accepted in decimal literals
only. `1_000_000` and `1_0.5_5` are fine; `0xdead_beef` and `0b1010_1010`
are not. [Numbers](ch04-02-numbers.md) has the full rules.

The special values behave the way the standard says:

```zuri
echo 1 / 0
echo -1 / 0
echo 0 / 0
```

```console
inf
-inf
NaN
```

Every number carries methods, so mathematics reads left to right:

```zuri
echo 2.sqrt()
echo 16.log2()
echo (-3).abs()
echo 3.7.round()
echo 3.14159.fixed(2)
echo 255.hex()
```

```console
1.4142135623730951
4
3
4
3.14
ff
```

A literal takes a method directly; the parentheses around `(-3)` are there
because a method call binds tighter than the minus sign, so `-3.abs()`
would negate the result instead of the operand.
[Numbers](ch04-02-numbers.md) covers the rule in full.

## `bigint`

When 2^53 is not enough, suffix the literal with `n` and you get an
arbitrary-precision integer:

```zuri
echo 9007199254740993n + 1n
```

```console
9007199254740994n
```

Bigints never lose precision and never overflow. They are slower than
numbers and they do not silently mix with them, so convert explicitly with
`to_bigint()` and `to_number()`. [Chapter 4](ch04-02-numbers.md) covers
them properly.

## `bool`

`true` and `false`. Every value in Zuri is either **truthy** or **falsy**,
and that determines what `if`, `while`, `and`, `or`, `!` and `? :` do with
it. The falsy values are:

| Value | Why |
| --- | --- |
| `false` | itself |
| `nil` | absence of a value |
| `0`, `0.0`, `-0.0` | zero |
| any negative number | see below |
| `0n` | the bigint zero |
| `''` | the empty string |
| `bytes(0)` | an empty byte buffer |

Everything else is truthy, including `[]`, `{}`, `'0'` and `NaN`.

Two of those rows deserve a second look.

**Negative numbers are falsy.** `-1` is falsy, and so is `-42`. That makes
`if list.index_of(x) { ... }` read as "if it was found", because
`index_of()` returns `-1` when it was not. It also means the idiom
`var n = maybe or fallback` silently replaces any negative value with the
fallback. When a negative number is a legitimate result, compare it
explicitly:

```zuri,ignore
var position = haystack.index_of(needle)

if position == -1 {
  echo 'not found'
}
```

**An empty list is truthy.** `[]` and `{}` are objects, and objects are
truthy. Use `is_empty()`:

```zuri,ignore
if items.is_empty() {
  echo 'nothing here'
}
```

## `nil`

`nil` is the absence of a value. An uninitialised `var` is `nil`, a
function that falls off the end returns `nil`, and a missing dictionary key
read through `get()` gives `nil`.

`nil` is falsy, and it still has a `to_string()`:

```zuri
echo nil.to_string()
```

```console
nil
```

Calling any other method on `nil` raises a `TypeError`, which is usually
exactly the error you wanted.

## `string`

This is the short version. [Strings](ch04-01-strings.md) covers quoting,
escapes, interpolation, concatenation, repetition and regular expressions
in full.

Strings are written in single or double quotes, with no difference in
meaning:

```zuri
echo 'single'
echo "double"
```

Both kinds may span multiple lines:

```zuri
echo 'multi
line'
```

```console
multi
line
```

The escape sequences are `\0`, `\a`, `\b`, `\f`, `\n`, `\r`, `\t`, `\v`,
`\\`, `\'`, `\"`, plus `\xNN` for a byte, `\uNNNN` for a code point and
`\UNNNNNNNN` for one outside the basic plane:

```zuri
echo "tab:\there"
echo "hex: \x41"
echo "emoji: \U0001F600"
```

```console
tab:	here
hex: A
emoji: 😀
```

A backslash followed by anything else is left alone, backslash and all.

### Interpolation

`${...}` inside a string evaluates the expression and splices the result
in. It works in both quote styles and the expression can be anything:

```zuri
var name = 'Zuri'
echo 'Hi ${name}, ${1 + 2} and ${name.upper()}'
```

```console
Hi Zuri, 3 and ZURI
```

To produce a literal `${`, build it by concatenation:

```zuri
echo 'B: $' + '{x}'
```

```console
B: ${x}
```

### Strings Are Sequences

Indexing gives you a one-character string, and negative indices count from
the end:

```zuri
var s = 'hello world'
echo s[0]
echo s[0,5]
echo s[-5,]
```

```console
h
hello
world
```

`s[a,b]` is a **slice**: from `a` up to but not including `b`. Either side
may be left out, so `s[,3]` is the first three characters and `s[3,]` is
everything from index three onward.

`+` concatenates and `*` repeats:

```zuri
echo 'ab' + 'cd'
echo 'ab' * 3
```

```console
abcd
ababab
```

## `list`

An ordered, growable sequence, written in square brackets. Elements may be
of any type, including other lists:

```zuri
var mixed = [1, 'two', [3], { four: 4 }]
echo mixed.length()
```

```console
4
```

Lists index and slice exactly like strings, negative indices included.
[Chapter 4](ch04-03-lists.md) covers the forty methods they carry.

## `dict`

An insertion-ordered mapping from keys to values:

```zuri
var config = { host: 'localhost', port: 8080, debug: true }
```

A bare word key is taken as a string, so `{ host: ... }` and
`{ 'host': ... }` are the same dictionary. Keys can also be numbers, and
they can be computed:

```zuri
var d = { name: 'Ada', 'age': 36, 3: 'three' }
echo d['name']
echo d.name
echo d[3]
```

```console
Ada
Ada
three
```

Dot access and bracket access are the same operation. Use the dot when the
key is a fixed name, brackets when it is computed or not a valid
identifier.

When a key and the variable holding its value share a name, write it once:

```zuri
var host = 'localhost'
var port = 8080

var config = { host, port }
```

## `range`

`a..b` describes the integers from `a` up to but not including `b`:

```zuri
echo 1..5
echo (1..5).to_list()
```

```console
1..5
[1, 2, 3, 4]
```

A range is a real value, not loop syntax. You can store one, pass it
around, and ask it questions:

```zuri
var r = 0..10
echo r.lower()
echo r.upper()
echo r.within(7)
```

```console
0
10
true
```

## `bytes`

A fixed-size buffer of 8-bit values, for binary data. `bytes(n)` allocates
`n` zero bytes; `bytes(list)` builds one from numbers in `0..256`:

```zuri
var b = bytes([72, 101, 108, 108, 111])
echo b
echo b.to_string()
echo b[0]
```

```console
(48 65 6c 6c 6f)
Hello
72
```

Bytes print as hexadecimal in parentheses, which is how you can always tell
one from a list at a glance. [Chapter 10](ch10-00-binary-data.md) is the
full treatment.

## Checking Types

`typeof()` gives you a name. The `is_*` family gives you a boolean, and
there are fifteen of them:

```text
is_bigint   is_bool     is_bytes    is_callable  is_class
is_dict     is_file     is_function is_instance  is_int
is_iterable is_list     is_number   is_object    is_string
```

```zuri
echo is_string('x')
echo is_callable(print)
echo is_iterable([1, 2])
```

```console
true
true
true
```

Use `instance_of(value, SomeClass)` for classes, which walks the
inheritance chain. [Chapter 6](ch06-00-classes.md) covers that.
