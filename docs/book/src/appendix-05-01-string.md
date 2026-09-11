# String Methods

Every method on the built-in `string` type, with its signature, what it
returns, and the cases where it does something other than the obvious
thing.

| Method | Returns | Summary |
| --- | --- | --- |
| [`length()`](#length) | `number` | Returns the length of a string. |
| [`upper()`](#upper) | `string` | Returns a copy of the string with all the cased characters converted to uppercase. |
| [`lower()`](#lower) | `string` | Return a copy of the string with all the cased characters converted to lowercase. |
| [`is_alpha()`](#is_alpha) | `boolean` | Returns `true` if all the characters in the string are all alphabets and the string is not empty., otherwise returns `false`. |
| [`is_alnum()`](#is_alnum) | `boolean` | Returns `true` if all the characters in the string are either alphabets or numbers and the string is not empty, otherwise returns `false`. |
| [`is_number()`](#is_number) |  | Returns `true` if all the characters in the string are all digits and the string is not empty, otherwise returns `false`. |
| [`is_lower()`](#is_lower) | `boolean` | Returns `true` if at least one character in the string is cased, all cased characters are lower cased and the string is not empty. |
| [`is_upper()`](#is_upper) | `boolean` | Returns `true` if at least one character in the string is cased, all cased characters are upper cased and the string is not empty. |
| [`is_space()`](#is_space) | `boolean` | Returns `true` if there are only whitespace characters in the string and the string is not empty. |
| [`ord()`](#ord) | `number` | Returns the Unicode code point of the string, which must be exactly one character long. |
| [`trim(chr: ?string)`](#trim) | `string` | Returns a copy of the string with the given character (_`chr`_) removed if it appears at the start or end of the string. |
| [`ltrim(chr: ?string)`](#ltrim) | `string` | Similar to the `trim()` method, except that this method only removes characters at the beginning of the string. |
| [`rtrim(chr: ?string)`](#rtrim) | `string` | Similar to the `trim()` method, except that this method only removes characters at the end of the string. |
| [`join(string: string)`](#join) | `string` | Returns a string which is a concatenation of the items in the iterable using the _string_ as the separator. |
| [`split(delimiter: string)`](#split) | `list` | Returns a list of words or characters in a string after separating the content of the string at every point where the _delimiter_ is found. |
| [`index_of(str: string, start_index: ?number)`](#index_of) | `number` | Returns the index position of the first occurrence of the string _`str`_ in the string _`string`_. |
| [`starts_with(str: string)`](#starts_with) | `boolean` | Returns `true` if the string begins with the string or character specified in _str_, otherwise it returns `false`. |
| [`ends_with(str: string)`](#ends_with) | `boolean` | Returns `true` if the string ends with the string or character specified in _str_, otherwise it returns `false`. |
| [`count(str: string)`](#count) | `number` | Returns the number of non-overlapping occurrences of the substring _str_ in the string. |
| [`to_number()`](#to_number) | `number` | Returns the first numeric value contained in the string if any exists or `0` if the string contains no numeric value. |
| [`to_list()`](#to_list) | `list` | Returns a list whose elements consists of every character contained in the string in order of appearance. |
| [`to_bytes()`](#to_bytes) | `bytes` | Returns the content of the string as a stream of `bytes`. |
| [`lpad(width: number, fill: ?string)`](#lpad) | `string` | Returns the string left justified in a string of length _width_. |
| [`rpad(width: number, fill: ?string)`](#rpad) | `string` | Returns the string right justified in a string of length _width_. |
| [`match(str: string)`](#match) | `boolean\|dictionary` | If the string _str_ is a regular string, this method returns `true` if the _string_ contains a substring _str_. |
| [`matches(reg: string)`](#matches) | `dictionary` | Returns a dictionary containing every match of the given regular expression _reg_ in the source string. |
| [`replace(str: string, replacement: string, use_regex: ?bool)`](#replace) | `string` | Returns a copy of the string with all occurrences or matches of _str_ replaced by the _replacement_ string. |
| [`replace_with(regex: string, callback: function)`](#replace_with) | `string` | Returns a copy of the string with all occurrences or matches of *regex* replaced with the result of the function *callback* which is invoked only if and after a match has occurred. |
| [`ascii()`](#ascii) | `string` | Reinterprets the string as a raw byte view: each byte of its UTF-8 encoding becomes its own character (a codepoint between `0` and `255`, i.e. |
| [`case_fold()`](#case_fold) | `string` | Returns a copy of the string case-folded for case-insensitive comparison, using full Unicode case folding rather than plain lowercasing. |
| [`compare(other: string)`](#compare) | `number` | Compares the string with another string. |
| [`is_empty()`](#is_empty) | `boolean` | Returns true if the string is empty, false otherwise. |
| [`contains(str: string)`](#contains) | `boolean` | Returns true if the string contains the specified substring, false otherwise. |
| [`lines()`](#lines) | `list` | Returns the lines of the string as an list as it would be if split on newline characters. |
| [`each_line(callback: function)`](#each_line) |  | Iterates over each line of the string, calling the provided callback function with the line and its index. |
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

```zuri,ignore
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

```zuri,ignore
%> 'zuri'.upper()
'ZURI'
```

**Returns** `string`

## `lower()`

```zuri,ignore
lower() -> string
```

Return a copy of the string with all the cased characters converted to
lowercase.<br>

For example:

```zuri,ignore
%> 'Zuri Is Bae'.lower()
'zuri is bae'
```

**Returns** `string`

## `is_alpha()`

```zuri,ignore
is_alpha() -> boolean
```

Returns `true` if all the characters in the string are all alphabets and
the string is not empty., otherwise returns `false`.

For example:

```zuri,ignore
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

Returns `true` if all the characters in the string are either alphabets
or numbers and the string is not empty, otherwise returns `false`. This
method is the same as `string.is_alpha() or string.is_number()`.

For example:

```zuri,ignore
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

Returns `true` if all the characters in the string are all digits and
the string is not empty, otherwise returns `false`.

For example:

```zuri,ignore
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

Returns `true` if at least one character in the string is cased, all
cased characters are lower cased and the string is not empty. Otherwise,
it returns `false`.

For example:

```zuri,ignore
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

Returns `true` if at least one character in the string is cased, all
cased characters are upper cased and the string is not empty. Otherwise,
it returns `false`.

For example:

```zuri,ignore
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

Returns `true` if there are only whitespace characters in the string and
the string is not empty. Otherwise, it returns empty.

For example:

```zuri,ignore
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

```zuri,ignore
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
trim(chr: ?string) -> string
```

Returns a copy of the string with the given character (_`chr`_) removed
if it appears at the start or end of the string. If _`chr`_ is not
given, it defaults to a space (`' '`). All matching leading and trailing
characters are removed until a character that doesn't match is
encountered. If no match is found, a copy of the original string is
returned.

> The square brackets (`[]`) around the _`chr: char`_ in the
> method definition indicates that the parameter is optional
> and does not mean you have to type the square brackets.

For example:

```zuri,ignore
%> '  example  '.trim()
'example'
%> '  example  '.trim('e')
'  example  '
%> 'example'.trim('e')
'xampl'
```

**Parameters**

- `chr` (`?char`) — The character to trim (Default = ' ').

**Returns** `string`

## `ltrim()`

```zuri,ignore
ltrim(chr: ?string) -> string
```

Similar to the `trim()` method, except that this method only removes
characters at the beginning of the string.

For example:

```zuri,ignore
%> '  example  '.ltrim()
'example  '
%> 'example'.ltrim('e')
'xample'
```

**Parameters**

- `chr` (`?char`) — The character to trim (Default = ' ').

**Returns** `string`

## `rtrim()`

```zuri,ignore
rtrim(chr: ?string) -> string
```

Similar to the `trim()` method, except that this method only removes
characters at the end of the string.

For example:

```zuri,ignore
%> '  example  '.rtrim()
'  example'
%> 'example'.rtrim('e')
'exampl'
```

**Parameters**

- `chr` (`?char`) — The character to trim (Default = ' ').

**Returns** `string`

## `join()`

```zuri,ignore
join(string: string) -> string
```

Returns a string which is a concatenation of the items in the iterable
using the _string_ as the separator. If the iterable contains just one
item or the _string_ is empty, the original element is returned. If the
_iterable_ contains non-string items, the items are converted to their
string representation before joining.

`Bytes` are the only non supported iterables.

For example:

```zuri,ignore
%> ','.join(['ok', 1, true])
'ok,1,true'
%> '--'.join('name')
'n--a--m--e'
%> ','.join('a')
'a'
```

**Parameters**

- `string` (`string`) — The string to join the items in the iterable.

**Returns** `string`

## `split()`

```zuri,ignore
split(delimiter: string) -> list
```

Returns a list of words or characters in a string after separating the
content of the string at every point where the _delimiter_ is found.

If the _delimiter_ is an empty string, the resultant list will contain
the individual characters of the string in the order in which they
appear in the original string. Consecutive delimiters are not grouped
together and are deemed to delimit empty strings. Splitting an empty
string with a specified separator returns an empty list.

This method has full UTF-8 support.

For example:

```zuri,ignore
%> 'name'.split('')
[n, a, m, e]
%> '1<>2<>3'.split('<>')
[1, , 2, , 3]
%> '1,2,3'.split(',')
[1, 2, 3]
%> ''.split(',')
[]
%> '地点'.split('')
[地, 点]
%> 'who is in the garden'.split('/\s/')
[who, is, in, the, garden]
```

**Parameters**

- `delimiter` (`string`) — The delimiter to use the split the string.

**Returns** `list`

## `index_of()`

```zuri,ignore
index_of(str: string, start_index: ?number) -> number
```

Returns the index position of the first occurrence of the string _`str`_
in the string _`string`_. If the _str_ cannot be found anywhere in
_string_, it returns -1. If the `start_index` parameter is given, it
will start scanning from the given index.

For example:

```zuri,ignore
%> 'hello, world'.index_of(' ')
6
%> 'hello, world'.index_of('e')
1
%> 'hello, world'.index_of('q')
-1
%> 'hello, world'.index_of('o')
4
%> 'hello, world'.index_of('o', 5)  # next index of `o` starting from index 5.
8
```

**Parameters**

- `str` (`string`) — The string to search for.
- `start_index` (`?number`) — The index to start the search from.

**Returns** `number`

## `starts_with()`

```zuri,ignore
starts_with(str: string) -> boolean
```

Returns `true` if the string begins with the string or character
specified in _str_, otherwise it returns `false`.

For example:

```zuri,ignore
%> 'hello, world'.starts_with('hello')
true
%> 'hello, world'.starts_with('hellios')
false
```

**Parameters**

- `str` (`string`) — The string to search for.

**Returns** `boolean`

## `ends_with()`

```zuri,ignore
ends_with(str: string) -> boolean
```

Returns `true` if the string ends with the string or character specified
in _str_, otherwise it returns `false`.

For example:

```zuri,ignore
%> 'gumtree'.ends_with('tree')
true
%> 'gumtree'.ends_with('mree')
false
```

**Parameters**

- `str` (`string`) — The string to search for.

**Returns** `boolean`

## `count()`

```zuri,ignore
count(str: string) -> number
```

Returns the number of non-overlapping occurrences of the substring _str_
in the string.

_For those coming from Python who may consider this method similar to
Python's own, this method differs in that it does not allow specifying a
start and end region for the operation. Zuri considers this unnecessary
as the same can be accomplished by slicing the string._

For example:

```zuri,ignore
%> 'Hallelujah'.count('l')
3
%> 'ding dong'.count('ng')
2
%> 'ding dong'[2,7].count('ng') # setting region to search for counts - 'ng do'
1
```

**Parameters**

- `str` (`string`) — The string to search for.

**Returns** `number`

## `to_number()`

```zuri,ignore
to_number() -> number
```

Returns the first numeric value contained in the string if any exists or
`0` if the string contains no numeric value. Floating numbers that have
the same value as their integer counterparts will return the integer
value.

For example:

```zuri,ignore
%> '123.0 hell'.to_number()
123
%> '427 and 12'.to_number()
427
%> '96.3 of 31'.to_number()
96.3
%> 'error'.to_number()
0
```

**Returns** `number`

## `to_list()`

```zuri,ignore
to_list() -> list
```

Returns a list whose elements consists of every character contained in
the string in order of appearance. Characters that repeat in the string
will have different entries in the same index as they appear in the
string.<br>

For example:

```zuri,ignore
%> 'Zuri'.to_list()
[B, l, a, d, e]
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

```zuri,ignore
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

Returns the string left justified in a string of length _width_. Padding
is done using the specified character _fill_ if given of a space (`' '`)
if a _fill_ is not specified. The original string is returned if width
is less than _`string.length()`_.

For example:

```zuri,ignore
%> 'cat'.lpad(5)
'  cat'
%> 'cat'.lpad(5, '-')
'--cat'
%> 'cat'.lpad(2, '-')
'cat'
```

**Parameters**

- `width` (`number`) — The length of the string after padding.
- `fill` (`?string`) — The character to use for padding.

**Returns** `string`

## `rpad()`

```zuri,ignore
rpad(width: number, fill: ?string) -> string
```

Returns the string right justified in a string of length _width_.
Padding is done using the specified character _fill_ if given of a space
(`' '`) if a _fill_ is not specified. The original string is returned if
width is less than _`string.length()`_.

For example:

```zuri,ignore
%> 'Hmm'.rpad(6)
'Hmm   '
%> 'Hmm'.rpad(6, '.')
'Hmm...'
%> 'Hmm'.rpad(3, '.')
'Hmm'
```

**Parameters**

- `width` (`number`) — The length of the string after padding.
- `fill` (`?string`) — The character to use for padding.

**Returns** `string`

## `match()`

```zuri,ignore
match(str: string) -> boolean|dictionary
```

If the string _str_ is a regular string, this method returns `true` if
the _string_ contains a substring _str_. Otherwise, it returns `false`.

If the string _str_ contains a valid [regular
expression](#regular-expressions) (we'll get to that shortly below), it
returns `false` if a match for the regex _str_ cannot be found in the
string. Otherwise, it returns a [dictionary](./dictionaries) containing
all first matching substring.

If the _offset_ argument is specified, it becomes the offset in the
_string_ at which to start matching.

For example:

```zuri,ignore
%> 'gorilla'.match('go')      # regular string match
true
%> 'gorilla'.match('gox')     # regular string non-match
false
%> 'gorilla'.match('/?gox/')  # regular expression match
{0: go}
%> 'gorilla'.match('/gox\d/') # regular expression non-match
false
```

**Parameters**

- `str` (`string`) — The string to match.

**Returns** `boolean|dictionary`

## `matches()`

```zuri,ignore
matches(reg: string) -> dictionary
```

Returns a dictionary containing every match of the given regular
expression _reg_ in the source string. If no match is found, an empty
[dictionary](./dictionaries) is returned.

If the _offset_ argument is specified, it becomes the offset in the
_string_ at which to start matching.

For example:

```zuri,ignore
%> '123 dollars'.matches('/[a-z]+|\d+/')
{0: [123, dollars]}
%> 'who is in the garden'.matches('/\w+/')
{0: [who, is, in, the, garden]}
```

**Parameters**

- `reg` (`string`) — The regular expression to match.

**Returns** `dictionary`

## `replace()`

```zuri,ignore
replace(str: string, replacement: string, use_regex: ?bool) -> string
```

Returns a copy of the string with all occurrences or matches of _str_
replaced by the _replacement_ string.

In the _replacement_ string, if _str_ is a regular expression, then
capture groups can be referenced using the syntax `$index`. Taking as an
example, capture group `0` contains the entire match and can be used in
the _replacement_ string as `$0`.

> To escape the `$` sign in the _replacement_ string, use
> the double backslashes (`\\`).

For example:

```zuri,ignore
%> 'lady friend'.replace('d', 'z')  # non-regex
'lazy frienz'
%> 'John is 26 years old'.replace('/(\d+)/', '1$1') # regex example
'John is 126 years old'
%> 'John is 26 years old'.replace('/(\d+)/', '1\\$2')
'John is 1$2 years old'
```

**Parameters**

- `str` (`string`) — The string to match.
- `replacement` (`string`) — The replacement string.
- `use_regex` (`?bool`) — Whether to use the regular expression or the
  string string as the match string (default = true).

**Returns** `string`

> **Note:** When the third parameter _`use_regex`_ is set to false, _str_
> will never be treated as a regular expression even if it contains a
> valid regular expression.

## `replace_with()`

```zuri,ignore
replace_with(regex: string, callback: function) -> string
```

Returns a copy of the string with all occurrences or matches of *regex*
replaced with the result of the function *callback* which is invoked
only if and after a match has occurred.

The callback function is defined as follows:

```zuri,ignore
def replacer(match, p1, p2, /* …, */ pN, offset, string) {
  return replacement
}
```

The arguments to the function are as follows:

- `match`: The matched substring. (Corresponds to `$0`.)

- `p1, p2, …, pN`: The nth string found by a capture group
(including named capturing groups) corresponds to `$1`, `$2`, etc. For
example, if the pattern is `/(\a+)(\b+)/`, then `p1` is the match for
`\a+`, and `p2` is the match for `\b+`. If the group is part of a
disjunction (e.g. `"abc".replace_with('/(a)|(b)/', replacer)`), the
unmatched alternative will be `nil`.

- `offset`: The offset of the matched substring within the
whole string being examined. For example, if the whole string was
`'abcd'`, and the matched substring was `'bc'`, then this argument will
be `1`.

- `string`: The whole string being examined.

The exact number of arguments depends on how many capture groups are
contained in the regex.

For example:

```zuri,ignore
%> echo 'name'.replace_with('/m/', @(match, offset) {
..   return match + '-'
.. })
'nam-e'
```

Below is another example that uses a capture group:

```zuri,ignore
%> var text = 'all is well'
%> 
%> echo text.replace_with('/([a-z]+)/', @(match, val) {
..   if val == 'is' return 'is not'
..   return 'will be'
.. })
'will be is not will be'
```

**Parameters**

- `regex` (`string`) — The regular expression to match.
- `callback` (`function`) — The callback function to invoke for each
  match.

**Returns** `string`

## `ascii()`

```zuri,ignore
ascii() -> string
```

Reinterprets the string as a raw byte view: each byte of its UTF-8
encoding becomes its own character (a codepoint between `0` and `255`,
i.e. a Latin-1-style one-byte-per-character mapping), rather than the
decoded sequence of Unicode characters that `length()`, `each()`, and
indexing otherwise operate on.

The result is still a valid string (every codepoint between `0` and
`255` is valid UTF-8), so it can be used anywhere a normal string can.
It just no longer round-trips back through the original multi-byte
characters if the string had any, and its `length()` now reports the
original BYTE count of the string rather than its original CHARACTER
count.

This is meant for the rare case where code needs to walk a string
byte-for-byte instead of character-by-character, e.g. one that
originated from a byte stream where the bytes were never meant to be
decoded as Unicode at all.

```zuri,ignore
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
comparison, using full Unicode case folding rather than plain
lowercasing. This matters for characters whose fold is not just their
lowercase form: for example, the German `ß` folds to `ss`.

Two strings that are considered equal ignoring case will always produce
identical output from `case_fold()`, which makes it the correct method
to use for case-insensitive comparisons; `lower()` is not a substitute
for it.

```zuri,ignore
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

- `other` (`string`) — The other string to compare with.

**Returns** `number` — - A negative number if the string is less than
the other string. - Zero if the strings are equal. - A positive number
if the string is greater than the other string.

**Raises** `Error` if the other string is not a string.

## `is_empty()`

```zuri,ignore
is_empty() -> boolean
```

Returns true if the string is empty, false otherwise.

**Returns** `boolean`

## `contains()`

```zuri,ignore
contains(str: string) -> boolean
```

Returns true if the string contains the specified substring, false
otherwise.

**Parameters**

- `str` (`string`) — The substring to search for.

**Returns** `boolean`

## `lines()`

```zuri,ignore
lines() -> list
```

Returns the lines of the string as an list as it would be if split on
newline characters.

**Returns** `list`

## `each_line()`

```zuri,ignore
each_line(callback: function)
```

Iterates over each line of the string, calling the provided callback
function with the line and its index.

**Parameters**

- `callback` (`function`) — A function that takes two arguments: the
  line and its index.

**Returns** — void

**Raises** `Error` if the callback is not a function.

## `each()`

```zuri,ignore
each(callback: function) -> void
```

Iterates over each character of the string, calling the provided
callback function with the character and its index.

**Parameters**

- `callback` (`function`) — A function that takes two arguments: the
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
