# String Methods

Every method of the built-in `string` type, with its signature, what it
returns, and any behavior you might not expect.

| Method | Returns | Summary |
| --- | --- | --- |
| [`length()`](#length) | `number` | Returns the length of a string. |
| [`upper()`](#upper) | `string` | Returns a copy of the string with all the cased characters converted to uppercase. |
| [`lower()`](#lower) | `string` | Returns a copy of the string with every cased character converted to lowercase. |
| [`is_alpha()`](#is_alpha) | `boolean` | Returns `true` if the string isn't empty and every character in it is a letter, and `false` otherwise. |
| [`is_alnum()`](#is_alnum) | `boolean` | Returns `true` if the string isn't empty and every character in it is a letter or a digit, and `false` otherwise. |
| [`is_number()`](#is_number) |  | Returns `true` if the string isn't empty and every character in it is a digit, and `false` otherwise. |
| [`is_lower()`](#is_lower) | `boolean` | Returns `true` if the string contains at least one cased character and all its cased characters are lowercase, and `false` otherwise. |
| [`is_upper()`](#is_upper) | `boolean` | Returns `true` if the string contains at least one cased character and all its cased characters are uppercase, and `false` otherwise. |
| [`is_space()`](#is_space) | `boolean` | Returns `true` if the string isn't empty and contains only whitespace, and `false` otherwise. |
| [`ord()`](#ord) | `number` | Returns the Unicode code point of the string, which must be exactly one character long. |
| [`trim(chars: ?string)`](#trim) | `string` | Returns a copy of the string with characters removed from both ends. |
| [`ltrim(chars: ?string)`](#ltrim) | `string` | Returns a copy of the string with characters removed from its start only. |
| [`rtrim(chars: ?string)`](#rtrim) | `string` | Returns a copy of the string with characters removed from its end only. |
| [`join(items: list\|dict\|string)`](#join) | `string` | Joins the items of a list, the values of a dictionary or the characters of a string into one string, with this string as the separator between them. |
| [`split(delimiter: string)`](#split) | `list` | Splits the string at every occurrence of _delimiter_, and returns a list of the parts. |
| [`index_of(str: string, start_index: ?number)`](#index_of) | `number` | Returns the index of the first occurrence of _`str`_ in the string, or -1 if it doesn't occur. |
| [`last_index_of(str: string, end_index: ?number)`](#last_index_of) | `number` | Returns the index of the last occurrence of _`str`_ in the string, searching from the end, or -1 if it doesn't occur. |
| [`starts_with(str: string)`](#starts_with) | `boolean` | Returns `true` if the string starts with _str_, and `false` otherwise. |
| [`ends_with(str: string)`](#ends_with) | `boolean` | Returns `true` if the string ends with _str_, and `false` otherwise. |
| [`count(str: string)`](#count) | `number` | Returns the number of non-overlapping occurrences of _str_ in the string. |
| [`to_number(base)`](#to_number) | `number` | Returns the first number in the string, or `0` if the string doesn't contain one. |
| [`to_bigint(base)`](#to_bigint) | `bigint` | Returns the integer in the string as a `bigint`, or `0n` if the string doesn't contain one. |
| [`to_list()`](#to_list) | `list` | Returns a list of the characters in the string, in order. |
| [`to_bytes()`](#to_bytes) | `bytes` | Returns the content of the string as a stream of `bytes`. |
| [`lpad(width: number, fill: ?string)`](#lpad) | `string` | Returns the string padded on the left to _width_ characters, so that it's aligned to the right. |
| [`rpad(width: number, fill: ?string)`](#rpad) | `string` | Returns the string padded on the right to _width_ characters, so that it's aligned to the left. |
| [`match(str: string)`](#match) | `boolean\|dictionary` | If _str_ is a plain string, this returns `true` if the string contains _str_, and `false` otherwise. |
| [`matches(reg: string)`](#matches) | `dictionary` | Returns a dictionary with every match of the regular expression _reg_ in the string. |
| [`replace(str: string, replacement: string, use_regex: ?bool)`](#replace) | `string` | Returns a copy of the string with every occurrence or match of _str_ replaced by _replacement_. |
| [`replace_with(regex: string, callback: function)`](#replace_with) | `string` | Returns a copy of the string with every match of _regex_ replaced by what _callback_ returns. |
| [`ascii()`](#ascii) | `string` | Reinterprets the string as raw bytes: each byte of its UTF-8 encoding becomes a separate character, with a code point from `0` to `255`, as in Latin-1. |
| [`case_fold()`](#case_fold) | `string` | Returns a copy of the string case-folded for case-insensitive comparison, using full Unicode case folding instead of plain conversion to lowercase. |
| [`compare(other: string)`](#compare) | `number` | Compares the string with another string. |
| [`is_empty()`](#is_empty) | `boolean` | Returns `true` if the string is empty, and `false` otherwise. |
| [`contains(str: string)`](#contains) | `boolean` | Returns `true` if the string contains _str_, and `false` otherwise. |
| [`lines()`](#lines) | `list` | Returns the lines of the string as a list, as if the string were split at each line break. |
| [`each_line(callback: function)`](#each_line) | `void` | Iterates over each line of the string, calling the provided callback function with the line and its index. |
| [`each(callback: function)`](#each) | `void` | Iterates over each character of the string, calling the provided callback function with the character and its index. |
| [`capitalize()`](#capitalize) | `string` | Returns a new string with the first character capitalized and the rest in lowercase. |
| [`title()`](#title) | `string` | Returns a new string with each word capitalized. |
| [`to_string()`](#to_string) | `string` | Returns the string itself. |

## `length()`

```zuri,ignore
length() -> number
```

Returns the length of a string. Note that this method is UTF-8
compatible and will return the UTF-8 length for the string if the string
contains UTF-8 characters whether written directly or via the `\u` or
`\U` escapes.

For example:

```zuri-repl
%> 'This is a pretty long string'.length()
28
%> 'उनका एक समय'.length()
11
%> 'This text mixes English and 粵語'.length()
30
```

**Returns** `number`

## `upper()`

```zuri,ignore
upper() -> string
```

Returns a copy of the string with all the cased characters converted to
uppercase. Note that the result of this method may return `false` when
tested with `is_upper()` of the _string_ contains Unicode characters
that are not case folded.

For example:

```zuri-repl
%> 'zuri'.upper()
'ZURI'
```

**Returns** `string`

## `lower()`

```zuri,ignore
lower() -> string
```

Returns a copy of the string with every cased character converted to
lowercase.

For example:

```zuri-repl
%> 'Zuri Is Bae'.lower()
'zuri is bae'
```

**Returns** `string`

## `is_alpha()`

```zuri,ignore
is_alpha() -> boolean
```

Returns `true` if the string isn't empty and every character in it is a
letter, and `false` otherwise.

For example:

```zuri-repl
%> 'abracadabra'.is_alpha()
true
%> 'my tooth aches'.is_alpha()
false
%> ''.is_alpha()
false
```

**Returns** `boolean`

## `is_alnum()`

```zuri,ignore
is_alnum() -> boolean
```

Returns `true` if the string isn't empty and every character in it is a
letter or a digit, and `false` otherwise.

For example:

```zuri-repl
%> '3Idiots'.is_alnum()
true
%> 'Three Idiots'.is_alnum()
false
%> '3 Idiots'.is_alnum()
false
%> '3'.is_alnum()
true
%> 'idiots'.is_alnum()
true
%> ''.is_alnum()
false
```

**Returns** `boolean`

## `is_number()`

```zuri,ignore
is_number()
```

Returns `true` if the string isn't empty and every character in it is a
digit, and `false` otherwise.

For example:

```zuri-repl
%> '123.5'.is_number()
false
%> '1970'.is_number()
true
%> '1980s'.is_number()
false
```

## `is_lower()`

```zuri,ignore
is_lower() -> boolean
```

Returns `true` if the string contains at least one cased character and
all its cased characters are lowercase, and `false` otherwise.

For example:

```zuri-repl
%> 'all'.is_lower()
true
%> 'all...123'.is_lower()
true
%> 'All...123'.is_lower()
false
%> ''.is_lower()
false
```

**Returns** `boolean`

## `is_upper()`

```zuri,ignore
is_upper() -> boolean
```

Returns `true` if the string contains at least one cased character and
all its cased characters are uppercase, and `false` otherwise.

For example:

```zuri-repl
%> 'ALL'.is_upper()
true
%> 'ALL...123'.is_upper()
true
%> 'All...123'.is_upper()
false
%> ''.is_upper()
false
```

**Returns** `boolean`

## `is_space()`

```zuri,ignore
is_space() -> boolean
```

Returns `true` if the string isn't empty and contains only whitespace,
and `false` otherwise.

For example:

```zuri-repl
%> '.     '.is_space()
false
%> '\r\n'.is_space()
true
%> '\t  '.is_space()
true
```

**Returns** `boolean`

## `ord()`

```zuri,ignore
ord() -> number
```

Returns the Unicode code point of the string, which must be exactly one
character long.

```zuri-repl
%> 'A'.ord()
65
%> 'AB'.ord()
Unhandled Error: ord() must be called on a single character, got AB
StackTrace:
  <repl>:1 -> @.script()
```

**Returns** `number`

**Raises** `Error` if the string is not exactly one character long.

## `trim()`

```zuri,ignore
trim(chars: ?string) -> string
```

Returns a copy of the string with characters removed from both ends.

Without an argument, whitespace is removed: spaces, tabs (`\t`), line
feeds (`\n`), vertical tabs, form feeds and carriage returns (`\r`).
Other Unicode spaces, such as the no-break space, are kept.

With _`chars`_, every character in it is removed instead, in any order
and any number of times, until a character that isn't in _`chars`_ is
reached at each end. _`chars`_ is a set of characters, not a prefix or
suffix, so `'xyax'.trim('xy')` is `'a'`. An empty _`chars`_ removes
nothing.

The original string is never changed. If there's nothing to remove, the
result is an equal copy.

For example:

```zuri-repl
%> '  example  '.trim()
'example'
%> '\t example \r\n'.trim()
'example'
%> '  example  '.trim('e')
'  example  '
%> 'example'.trim('e')
'xampl'
%> '--==example==--'.trim('-=')
'example'
```

**Parameters**

- `chars` (`?string`): The characters to remove. Defaults to whitespace.

**Returns** `string`

## `ltrim()`

```zuri,ignore
ltrim(chars: ?string) -> string
```

Returns a copy of the string with characters removed from its start
only. The characters removed are chosen in the same way as for `trim()`:
whitespace if _`chars`_ isn't given, or every character in _`chars`_ if
it is.

For example:

```zuri-repl
%> '  example  '.ltrim()
'example  '
%> 'example'.ltrim('e')
'xample'
%> '0012'.ltrim('0')
'12'
```

**Parameters**

- `chars` (`?string`): The characters to remove. Defaults to whitespace.

**Returns** `string`

## `rtrim()`

```zuri,ignore
rtrim(chars: ?string) -> string
```

Returns a copy of the string with characters removed from its end only.
The characters removed are chosen in the same way as for `trim()`:
whitespace if _`chars`_ isn't given, or every character in _`chars`_ if
it is.

For example:

```zuri-repl
%> '  example  '.rtrim()
'  example'
%> 'example'.rtrim('e')
'exampl'
%> 'line\r\n'.rtrim('\r\n')
'line'
```

**Parameters**

- `chars` (`?string`): The characters to remove. Defaults to whitespace.

**Returns** `string`

## `join()`

```zuri,ignore
join(items: list|dict|string) -> string
```

Joins the items of a list, the values of a dictionary or the characters
of a string into one string, with this string as the separator between
them. Items that aren't strings are converted to strings first. Any
other argument, such as a range or `bytes`, raises an error.

For example:

```zuri-repl
%> ','.join(['ok', 1, true])
'ok,1,true'
%> '--'.join('name')
'n--a--m--e'
%> ','.join('a')
'a'
%> ''.join(['a', 'b', 'c'])
'abc'
%> '-'.join({ a: 1, b: 2 })
'1-2'
```

**Parameters**

- `items` (`list|dict|string`): The items to join.

**Returns** `string`

## `split()`

```zuri,ignore
split(delimiter: string) -> list
```

Splits the string at every occurrence of _delimiter_, and returns a list
of the parts.

If _delimiter_ is an empty string, the result is a list of the string's
characters, in order. Consecutive delimiters aren't treated as one, so
they produce empty strings between them. Splitting an empty string
returns an empty list.

This works correctly with any UTF-8 text.

For example:

```zuri-repl
%> 'name'.split('')
[n, a, m, e]
%> '1<>2<>3'.split('<>')
[1, 2, 3]
%> '1,,2'.split(',')
[1, , 2]
%> ''.split(',')
[]
%> '地点'.split('')
[地, 点]
%> 'who is in the garden'.split('/\s/')
[who, is, in, the, garden]
```

**Parameters**

- `delimiter` (`string`): The delimiter to split the string at.

**Returns** `list`

## `index_of()`

```zuri,ignore
index_of(str: string, start_index: ?number) -> number
```

Returns the index of the first occurrence of _`str`_ in the string, or
-1 if it doesn't occur. If `start_index` is given, the search starts at
that index.

For example:

```zuri-repl
%> 'hello, world'.index_of(' ')
6
%> 'hello, world'.index_of('e')
1
%> 'hello, world'.index_of('q')
-1
%> 'hello, world'.index_of('o')
4
%> 'hello, world'.index_of('o', 5)  # the next `o` from index 5
8
```

**Parameters**

- `str` (`string`): The string to search for.
- `start_index` (`?number`): The index to start searching from.

**Returns** `number`

## `last_index_of()`

```zuri,ignore
last_index_of(str: string, end_index: ?number) -> number
```

Returns the index of the last occurrence of _`str`_ in the string,
searching from the end, or -1 if it doesn't occur.

If `end_index` is given, only a match that _starts_ at or before that
index counts. That's the same position that the second parameter of
`index_of()` uses, so for any index `n`, `index_of(str, n)` and
`last_index_of(str, n)` give the first and last matches in the two
halves that `n` splits the string into.

An empty _str_ returns -1, as it does for `index_of()`.

For example:

```zuri-repl
%> 'hello, world'.last_index_of('o')
8
%> 'hello, world'.last_index_of('l')
10
%> 'hello, world'.last_index_of('q')
-1
%> 'hello, world'.last_index_of('o', 7)  # the last `o` at or before index 7
4
```

The most common use is to split a path at its last separator:

```zuri-repl
%> var path = 'a/b/c'
%> path.last_index_of('/')
3
%> path[path.last_index_of('/') + 1, path.length()]
'c'
```

**Parameters**

- `str` (`string`): The string to search for.
- `end_index` (`?number`): The highest index a match can start at.

**Returns** `number`

## `starts_with()`

```zuri,ignore
starts_with(str: string) -> boolean
```

Returns `true` if the string starts with _str_, and `false` otherwise.

For example:

```zuri-repl
%> 'hello, world'.starts_with('hello')
true
%> 'hello, world'.starts_with('hellios')
false
```

**Parameters**

- `str` (`string`): The string to look for.

**Returns** `boolean`

## `ends_with()`

```zuri,ignore
ends_with(str: string) -> boolean
```

Returns `true` if the string ends with _str_, and `false` otherwise.

For example:

```zuri-repl
%> 'gumtree'.ends_with('tree')
true
%> 'gumtree'.ends_with('mree')
false
```

**Parameters**

- `str` (`string`): The string to look for.

**Returns** `boolean`

## `count()`

```zuri,ignore
count(str: string) -> number
```

Returns the number of non-overlapping occurrences of _str_ in the
string.

Unlike Python's `count()`, this doesn't take a start and end position.
To count within part of a string, slice it first.

For example:

```zuri-repl
%> 'Hallelujah'.count('l')
3
%> 'ding dong'.count('ng')
2
%> 'ding dong'[2,7].count('ng') # count only in 'ng do'
1
```

**Parameters**

- `str` (`string`): The string to count.

**Returns** `number`

## `to_number()`

```zuri,ignore
to_number(base) -> number
```

Returns the first number in the string, or `0` if the string doesn't
contain one. A fractional number with the same value as an integer is
returned as the integer.

For example:

```zuri-repl
%> '123.0 hell'.to_number()
123
%> '427 and 12'.to_number()
427
%> '96.3 of 31'.to_number()
96.3
%> 'error'.to_number()
0
```

**Parameters**

- `base` (`number`): The base of the digits, from 2 to 36. Defaults to
  `10`. A fractional part is only read in base 10, because other bases
  have no way of writing one.

**Returns** `number`

**Raises** `RangeError` if `base` is outside 2 to 36.

## `to_bigint()`

```zuri,ignore
to_bigint(base) -> bigint
```

Returns the integer in the string as a `bigint`, or `0n` if the string
doesn't contain one.

This is `to_number()` for integers too large for a `number`. A number is
only exact up to 2^53, and beyond that, digits are lost. An ID or a
`BIGINT UNSIGNED` value read from a database often goes beyond it. Here,
every digit is kept, however many there are.

```zuri-repl
%> '9007199254740993'.to_bigint()
9007199254740993n
%> '9007199254740993'.to_number()   # rounded down by one
9007199254740992
%> '-42'.to_bigint()
-42n
%> 'ff'.to_bigint(16)
255n
%> 'row 427 of 12'.to_bigint()
427n
%> 'error'.to_bigint()
0n
```

The number is found in the same way as for `to_number()`: it's the first
number in the string, and any text around it is ignored. No fractional
part is read, because the result is an integer, so `'12.5'.to_bigint()`
is `12n`.

**Parameters**

- `base` (`number`): The base of the digits, from 2 to 36. Defaults to
  `10`.

**Returns** `bigint`

**Raises** `RangeError` if `base` is outside 2 to 36.

## `to_list()`

```zuri,ignore
to_list() -> list
```

Returns a list of the characters in the string, in order.

For example:

```zuri-repl
%> 'Zuri'.to_list()
[Z, u, r, i]
%> 'Plantation'.to_list()
[P, l, a, n, t, a, t, i, o, n]
```

**Returns** `list`

## `to_bytes()`

```zuri,ignore
to_bytes() -> bytes
```

Returns the content of the string as a stream of `bytes`.

> The Zuri REPL _may_ truncate long bytes data when printing
> to console/terminal.

For example:

```zuri-repl
%> 'Zuri'.to_bytes()
(42 6c 61 64 65)
%> 'Plantation'.to_bytes()
(50 6c 61 6e 74 61 74 69 6f 6e)
```

**Returns** `bytes`

## `lpad()`

```zuri,ignore
lpad(width: number, fill: ?string) -> string
```

Returns the string padded on the left to _width_ characters, so that
it's aligned to the right. The padding is _fill_ if it's given, or
spaces otherwise. If _width_ is less than the length of the string, the
string is returned unchanged.

For example:

```zuri-repl
%> 'cat'.lpad(5)
'  cat'
%> 'cat'.lpad(5, '-')
'--cat'
%> 'cat'.lpad(2, '-')
'cat'
```

**Parameters**

- `width` (`number`): The length of the string after padding.
- `fill` (`?string`): The character to pad with.

**Returns** `string`

## `rpad()`

```zuri,ignore
rpad(width: number, fill: ?string) -> string
```

Returns the string padded on the right to _width_ characters, so that
it's aligned to the left. The padding is _fill_ if it's given, or spaces
otherwise. If _width_ is less than the length of the string, the string
is returned unchanged.

For example:

```zuri-repl
%> 'Hmm'.rpad(6)
'Hmm   '
%> 'Hmm'.rpad(6, '.')
'Hmm...'
%> 'Hmm'.rpad(3, '.')
'Hmm'
```

**Parameters**

- `width` (`number`): The length of the string after padding.
- `fill` (`?string`): The character to pad with.

**Returns** `string`

## `match()`

```zuri,ignore
match(str: string) -> boolean|dictionary
```

If _str_ is a plain string, this returns `true` if the string contains
_str_, and `false` otherwise.

If _str_ is a regular expression, this returns `false` if there's no
match. Otherwise, it returns a dictionary describing the first match:
the whole match under the key `0`, each capture group under its number,
and each named group under its name as well. A group that isn't part of
the match is `nil`. When several groups share a name, which the `J`
modifier allows, the name refers to the one that's part of the match.

If _offset_ is given, matching starts at that position in the string.

For example:

```zuri-repl
%> 'gorilla'.match('go')      # plain string, found
true
%> 'gorilla'.match('gox')     # plain string, not found
false
%> 'gorilla'.match('/?gox/')  # regular expression, matched
{0: go}
%> 'gorilla'.match('/gox\d/') # regular expression, not matched
false
%> '2024-01'.match('/(?<year>\d+)-(\d+)/')
{0: 2024-01, 1: 2024, 2: 01, year: 2024}
```

**Parameters**

- `str` (`string`): The string or regular expression to match.

**Returns** `boolean|dictionary`

## `matches()`

```zuri,ignore
matches(reg: string) -> dictionary
```

Returns a dictionary with every match of the regular expression _reg_ in
the string. If there are no matches, each key holds an empty list, so a
pattern without groups gives `{0: []}`.

If _offset_ is given, matching starts at that position in the string.

For example:

```zuri-repl
%> '123 dollars'.matches('/[a-z]+|\d+/')
{0: [123, dollars]}
%> 'who is in the garden'.matches('/\w+/')
{0: [who, is, in, the, garden]}
```

**Parameters**

- `reg` (`string`): The regular expression to match.

**Returns** `dictionary`

## `replace()`

```zuri,ignore
replace(str: string, replacement: string, use_regex: ?bool) -> string
```

Returns a copy of the string with every occurrence or match of _str_
replaced by _replacement_.

If _str_ is a regular expression, _replacement_ can refer to capture
groups as `$index`. Group `0` is the whole match, so `$0` inserts the
whole match.

> To write a literal `$` in _replacement_, put two backslashes
> (`\\`) before it.

For example:

```zuri-repl
%> 'lady friend'.replace('d', 'z')  # plain string
'lazy frienz'
%> 'John is 26 years old'.replace('/(\d+)/', '1$1') # regular expression
'John is 126 years old'
%> 'John is 26 years old'.replace('/(\d+)/', '1\\$2')
'John is 1$2 years old'
```

**Parameters**

- `str` (`string`): The string or regular expression to replace.
- `replacement` (`string`): The replacement string.
- `use_regex` (`?bool`): Whether _str_ can be treated as a regular
  expression. Defaults to true.

**Returns** `string`

> **Note:** When the third parameter, _`use_regex`_, is false, _str_ is
> never treated as a regular expression, even if it looks like one.

## `replace_with()`

```zuri,ignore
replace_with(regex: string, callback: function) -> string
```

Returns a copy of the string with every match of _regex_ replaced by
what _callback_ returns. The callback is only called when there's a
match.

The callback looks like this:

```zuri,ignore
def replacer(match, p1, p2, /* …, */ pN, offset, string) {
  return replacement
}
```

Its arguments are:

- `match`: the matched text, which is the same as `$0`.

- `p1, p2, …, pN`: the text matched by each capture group,
  including named groups, which is the same as `$1`, `$2` and so
  on. For example, if the pattern is `/(\a+)(\b+)/`, `p1` is the
  match for `\a+`, and `p2` is the match for `\b+`. If a group is
  one side of an alternation, as in
  `"abc".replace_with('/(a)|(b)/', replacer)`, the side that didn't
  match is `nil`.

- `offset`: the position of the match in the whole string. For
  example, if the whole string is `'abcd'` and the match is `'bc'`,
  this is `1`.

- `string`: the whole string being searched.

The number of arguments depends on the number of capture groups in the
regular expression.

For example:

```zuri-repl
%> echo 'name'.replace_with('/m/', @(match, offset) {
..   return match + '-'
.. })
nam-e
```

Here's an example with a capture group:

```zuri-repl
%> var text = 'all is well'
%> echo text.replace_with('/([a-z]+)/', @(match, val) {
..   if val == 'is' return 'is not'
..   return 'will be'
.. })
will be is not will be
```

**Parameters**

- `regex` (`string`): The regular expression to match.
- `callback` (`function`): The function to call for each match.

**Returns** `string`

## `ascii()`

```zuri,ignore
ascii() -> string
```

Reinterprets the string as raw bytes: each byte of its UTF-8 encoding
becomes a separate character, with a code point from `0` to `255`, as in
Latin-1. Normally, `length()`, `each()` and indexing work on the decoded
Unicode characters instead.

The result is still a valid string, because every code point from `0` to
`255` is valid UTF-8, so it can be used anywhere a normal string can.
But if the string contained multi-byte characters, they can't be
recovered from the result, and its `length()` is the original number of
bytes, not the original number of characters.

This is for the rare case where code needs to go through a string one
byte at a time, instead of one character at a time, for example for a
string that came from a byte stream that was never meant to be decoded
as Unicode.

```zuri-repl
%> 'café'.length()
4
%> 'café'.ascii().length()
5
```

**Returns** `string`

## `case_fold()`

```zuri,ignore
case_fold() -> string
```

Returns a copy of the string case-folded for case-insensitive
comparison, using full Unicode case folding instead of plain conversion
to lowercase. This matters for characters whose folded form isn't just
their lowercase form. For example, the German `ß` folds to `ss`.

Two strings that are equal when case is ignored always give the same
result from `case_fold()`, so it's the right method for case-insensitive
comparison. `lower()` isn't a substitute for it.

```zuri-repl
%> 'HELLO World'.case_fold()
'hello world'
%> 'Straße'.case_fold()
'strasse'
```

**Returns** `string`

## `compare()`

```zuri,ignore
compare(other: string) -> number
```

Compares the string with another string.

**Parameters**

- `other` (`string`): The string to compare with.

**Returns** `number`: A negative number if this string comes before
`other`, zero if the two are equal, or a positive number if this string
comes after `other`.

**Raises** `Error` if `other` isn't a string.

## `is_empty()`

```zuri,ignore
is_empty() -> boolean
```

Returns `true` if the string is empty, and `false` otherwise.

**Returns** `boolean`

## `contains()`

```zuri,ignore
contains(str: string) -> boolean
```

Returns `true` if the string contains _str_, and `false` otherwise.

**Parameters**

- `str` (`string`): The string to look for.

**Returns** `boolean`

## `lines()`

```zuri,ignore
lines() -> list
```

Returns the lines of the string as a list, as if the string were split
at each line break.

**Returns** `list`

## `each_line()`

```zuri,ignore
each_line(callback: function) -> void
```

Iterates over each line of the string, calling the provided callback
function with the line and its index.

**Parameters**

- `callback` (`function`): A function that takes two arguments: the line
  and its index.

**Returns** `void`

**Raises** `Error` if the callback is not a function.

## `each()`

```zuri,ignore
each(callback: function) -> void
```

Iterates over each character of the string, calling the provided
callback function with the character and its index.

**Parameters**

- `callback` (`function`): A function that takes two arguments: the
  character and its index.

**Returns** `void`

**Raises** `Error` if the callback is not a function.

## `capitalize()`

```zuri,ignore
capitalize() -> string
```

Returns a new string with the first character capitalized and the rest
in lowercase.

**Returns** `string`

## `title()`

```zuri,ignore
title() -> string
```

Returns a new string with each word capitalized.

**Returns** `string`

## `to_string()`

```zuri,ignore
to_string() -> string
```

Returns the string itself.

**Returns** `string`
