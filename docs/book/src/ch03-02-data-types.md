# Data Types

Zuri has ten built-in types. `typeof()` tells you which one a value is:

```zuri
var values = [
  1,
  1.5,
  'text',
  true,
  nil,
  [1],
  { a: 1 },
  1..3,
  10n,
  bytes(2),
]

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

In addition, there are `function`, `class`, `instance`, `module`, `file` and
`ptr` values, which you get by declaring or opening things rather than by
writing a literal.

## `number`

A number is an IEEE 754 double-precision value. There's no separate integer
type, so `1` and `1.0` are the same value:

```zuri
echo 1 == 1.0
```

```console
true
```

`is_int()` checks whether a number's value is a whole number. It doesn't
matter whether it was written with a decimal point:

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

Doubles can represent integers exactly up to 2^53. Beyond that, precision is
lost:

```zuri
echo 9007199254740993
```

```console
9007199254740992
```

There are five ways to write a number literal:

```zuri
var decimal = 1_000_000
var binary = 0b1010 # 10
var octal = 0c17 # 15
var hexadecimal = 0xff # 255
var scientific = 6.02e23

echo [decimal, binary, octal, hexadecimal, scientific]
```

```console
[1000000, 10, 15, 255, 602000000000000000000000]
```

Underscores can be used as digit separators, but only in decimal literals.
`1_000_000` and `1_0.5_5` are fine, while `0xdead_beef` and `0b1010_1010`
aren't. See [Numbers](ch04-02-numbers.md) for the full rules.

The special values behave as the IEEE standard specifies:

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

Numbers have methods, so math can be written from left to right:

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

You can call a method directly on a literal. The parentheses around `(-3)`
are needed because a method call binds more tightly than the minus sign, so
`-3.abs()` would negate the result instead of the number.
[Numbers](ch04-02-numbers.md) explains this rule in full.

## `bigint`

When 2^53 isn't big enough, add an `n` suffix to the literal to get an
arbitrary-precision integer:

```zuri
echo 9007199254740993n + 1n
```

```console
9007199254740994n
```

Bigints never lose precision and never overflow. They're slower than
numbers, and the two types don't mix automatically, so you convert between
them explicitly with `to_bigint()` and `to_number()`. [Chapter
4](ch04-02-numbers.md) covers them in detail.

## `bool`

A `bool` is either `true` or `false`. Every value in Zuri is either
**truthy** or **falsy**, which determines how `if`, `while`, `and`, `or`,
`!` and `? :` treat it. These are the falsy values:

| Value | Why it's falsy |
| --- | --- |
| `false` | It's false |
| `nil` | No value |
| `0`, `0.0`, `-0.0` | Zero |
| `NaN` | Not a number |
| `0n` | Bigint zero |
| `''` | Empty string |
| `bytes(0)` | Empty byte buffer |

Everything else is truthy, including negative numbers, `[]`, `{}` and `'0'`.

This has two consequences that are easy to miss.

**A position of `0` is falsy.** `index_of()` returns `-1` when it doesn't
find anything, which is truthy, and `0` when the item is first, which is
falsy. So `if list.index_of(x) { ... }` gets both cases wrong. Compare the
result explicitly instead:

```zuri
var haystack = ['a', 'b', 'c']
var position = haystack.index_of('z')

if position == -1 {
  echo 'not found'
}
```

```console
not found
```

**An empty list is truthy.** `[]` and `{}` are objects, and all objects are
truthy. Use `is_empty()` instead:

```zuri
var items = []

if items.is_empty() {
  echo 'nothing here'
}
```

```console
nothing here
```

## `nil`

`nil` represents the absence of a value. An uninitialized `var` holds `nil`,
a function that ends without returning anything returns `nil`, and reading a
missing dictionary key with `get()` gives you `nil`.

`nil` is falsy, but it still has a `to_string()` method:

```zuri
echo nil.to_string()
```

```console
nil
```

Calling any other method on `nil` raises a `TypeError`, which is usually the
error you want to see in that situation.

## `string`

This section is a quick overview. [Strings](ch04-01-strings.md) covers
quoting, escape sequences, interpolation, concatenation, repetition and
regular expressions in detail.

Strings can be written in single or double quotes, and both mean the same
thing:

```zuri
echo 'single'
echo "double"
```

Either kind can span several lines:

```zuri
echo 'multi
line'
```

```console
multi
line
```

The escape sequences are `\0`, `\a`, `\b`, `\f`, `\n`, `\r`, `\t`, `\v`,
`\\`, `\'`, `\"` and `\${` (for a literal `${`), along with `\xNN` for a
byte, `\uNNNN` for a code point, and `\UNNNNNNNN` for a code point outside
the Basic Multilingual Plane:

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

A backslash followed by any other character is left as it is, backslash
included.

### Interpolation

`${...}` inside a string evaluates the expression and inserts the result. It
works with both kinds of quotes, and the expression can be anything:

```zuri
var name = 'Zuri'
echo 'Hi ${name}, ${1 + 2} and ${name.upper()}'
```

```console
Hi Zuri, 3 and ZURI
```

To write a literal `${`, put a backslash in front of it:

```zuri
echo 'B: \${x}'
```

```console
B: ${x}
```

### Strings as Sequences

Indexing a string gives you a one-character string. Negative indexes count
from the end:

```zuri
var s = 'hello world'
echo s[0]
echo s[0, 5]
echo s[-5,]
```

```console
h
hello
world
```

`s[a,b]` is a **slice**, from index `a` up to but not including `b`. You can
leave out either side, so `s[,3]` is the first three characters and `s[3,]`
is everything from index three onward.

`+` concatenates strings and `*` repeats them:

```zuri
echo 'ab' + 'cd'
echo 'ab' * 3
```

```console
abcd
ababab
```

## `list`

A list is an ordered sequence that can grow, written in square brackets. Its
elements can be of any type, including other lists:

```zuri
var mixed = [
  1,
  'two',
  [3],
  { four: 4 },
]
echo mixed.length()
```

```console
4
```

Lists support indexing and slicing just like strings, including negative
indexes. [Chapter 4](ch04-03-lists.md) covers their forty methods.

## `dict`

A dictionary maps keys to values and remembers the order in which keys were
inserted:

```zuri
var config = { host: 'localhost', port: 8080, debug: true }
```

A key written as a bare word is treated as a string, so `{ host: ... }` and
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

Dot access and bracket access do the same thing. Use a dot when the key is a
fixed name, and brackets when the key is computed or isn't a valid
identifier.

When a key has the same name as the variable that holds its value, you can
write the name once:

```zuri
var host = 'localhost'
var port = 8080

var config = { host, port }
```

## `range`

`a..b` represents the integers from `a` up to but not including `b`:

```zuri
echo 1..5
echo 1..5.to_list()
```

```console
1..5
[1, 2, 3, 4]
```

A range is a value in its own right, not just loop syntax. You can store it
in a variable, pass it to a function, and call methods on it:

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

A `bytes` value is a fixed-size buffer of 8-bit values, used for binary
data. `bytes(n)` creates a buffer of `n` zero bytes, and `bytes(list)`
creates one from a list of numbers between 0 and 255:

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

Bytes are printed as hexadecimal values in parentheses, so you can always
tell them apart from lists. [Chapter 10](ch10-00-binary-data.md) covers them
in full.

## Checking Types

`typeof()` gives you the name of a type. The `is_*` functions give you a
boolean instead, and there are fifteen of them:

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

To check whether a value is an instance of a class, use
`instance_of(value, SomeClass)`, which also checks parent classes. [Chapter
6](ch06-00-classes.md) covers this.
