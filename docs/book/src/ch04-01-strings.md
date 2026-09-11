# Strings

A Zuri string is UTF-8 text. Indexing, slicing and `length()` work in
characters, not bytes, so a string of emoji behaves the way a reader
expects.

## Inspecting

```zuri
var s = 'Hello, Zuri'

echo s.length()
echo s.is_empty()
echo s.index_of('Zuri')
echo s.count('l')
echo s.starts_with('Hello')
echo s.ends_with('Zuri')
echo s.contains('Zuri')
```

```console
11
false
7
2
true
true
true
```

`index_of()` returns `-1` when there is no match, and takes an optional
second argument to start the search from.

There is a family of character-class predicates, each true when **every**
character in the string qualifies:

```zuri
echo 'abc'.is_alpha()
echo '123'.is_number()
echo 'ABC'.is_upper()
echo '  '.is_space()
```

```console
true
true
true
true
```

The full set is `is_alpha`, `is_alnum`, `is_number`, `is_lower`,
`is_upper`, `is_space`, plus `is_empty`.

## Changing Case

```zuri
var s = 'Hello, Zuri'

echo s.upper()
echo s.lower()
echo s.capitalize()
echo s.title()
```

```console
HELLO, ZURI
hello, zuri
Hello, zuri
Hello, Zuri
```

`capitalize()` uppercases the first character and lowercases the rest.
`title()` does that to every word.

For comparing text from users, reach for `case_fold()` rather than
`lower()`. Case folding is the Unicode operation defined for
case-insensitive matching, and it handles cases simple lowercasing gets
wrong:

```zuri
echo 'Straße'.case_fold()
```

```console
strasse
```

## Trimming and Padding

```zuri
echo '[' + '  pad  '.trim() + ']'
echo '[' + '  pad  '.ltrim() + ']'
echo '[' + '  pad  '.rtrim() + ']'
echo 'xxhixx'.trim('x')
```

```console
[pad]
[pad  ]
[  pad]
hi
```

All three take an optional argument naming the characters to strip. With
no argument they strip whitespace.

```zuri
echo 'x'.lpad(5, '.')
echo 'x'.rpad(5, '.')
```

```console
....x
x....
```

`lpad` and `rpad` take a target width and an optional fill character that
defaults to a space. A string already at or over the width comes back
unchanged.

## Splitting and Joining

```zuri
echo 'Hello, Zuri'.split(', ')
echo ', '.join(['a', 'b', 'c'])
echo 'abc'.to_list()
echo 'line1\nline2'.lines()
```

```console
[Hello, Zuri]
a, b, c
[a, b, c]
[line1, line2]
```

`join` lives on the **separator**, not on the list. `to_list()` splits into
single characters. `lines()` splits on newlines and handles both `\n` and
`\r\n`.

## Replacing

```zuri
echo 'Hello, Zuri'.replace('Zuri', 'World')
```

```console
Hello, World
```

Every occurrence is replaced. The pattern may be a regular expression, and
the replacement can refer back to capture groups with `$1`, `$2` or
`${1}`:

```zuri
echo 'John Smith'.replace('/(\w+) (\w+)/', '$2, $1')
```

```console
Smith, John
```

The optional third argument turns regex handling off, so a pattern that
would otherwise be read as a regex is matched literally, delimiters
included:

```zuri
echo 'a/b/c'.replace('/b/', 'X')
echo 'a/b/c'.replace('/b/', 'X', false)
```

```console
a/X/c
aXc
```

When the replacement depends on what was matched, use `replace_with()`,
which calls a function for each match:

```zuri
echo 'a1b2'.replace_with('/[0-9]/', @(m) => '<' + m + '>')
```

```console
a<1>b<2>
```

## Regular Expressions

Zuri's regular expressions are real PCRE2, which means named groups,
backreferences and lookaround all work. A pattern is written as a string
whose first character is a non-word delimiter, repeated to close, with
modifier letters after:

```text
'/[a-z]+/i'
'#\d{3}#'
```

The delimiter is whatever you make it, which saves escaping when your
pattern is full of slashes.

Any method that accepts a pattern also accepts a plain string, and treats
it as a literal substring. `'a.c'` matches the three characters; `'/a.c/'`
matches `abc`.

`match()` returns `{0: <the matched text>}`, or `false` when nothing
matched:

```zuri
echo 'one two'.match('/(\w+) (\w+)/')
echo 'nope'.match('/zzz/')
```

```console
{0: one two}
false
```

Note that "no match" is `false`, not `nil`. Both are falsy, so `if
s.match(p) { ... }` reads correctly, but an equality test against `nil`
does not.

`matches()` returns every match, grouped by capture group. Key `0` is the
whole match, key `1` the first group, and so on:

```zuri
echo 'a1b2c3'.matches('/[0-9]/')
```

```console
{0: [1, 2, 3]}
```

The common modifiers are `i` (case-insensitive), `m` (multi-line, so `^`
and `$` match at line boundaries), `s` (dot matches newline), `x` (ignore
whitespace in the pattern) and `A` (anchor the match at the start
position).

## Conversion

```zuri
echo '5'.to_number() + 1
echo 'ff'.to_number(16)
echo 'H'.ord()
echo 72.chr()
echo 'Hi'.to_bytes()
```

```console
6
255
72
H
(48 69)
```

`to_number()` takes an optional base. `ord()` requires a single-character
string and gives its code point; `chr()` on a number goes the other way.

## Walking a String

```zuri
var s = 'Hi'
s.each(@(c, i) { print('${i}:${c} ') })
print('\n')
```

```console
0:H 1:i 
```

`each_line()` does the same over lines. Both hand the callback the value
first and the index second.

Strings are also iterable with `for`, which is usually what you want:

```zuri
for ch in 'hey' {
  echo ch
}
```

## Strings Are Immutable

Every method here returns a new string. Nothing mutates in place, so you
can pass a string anywhere without copying it defensively.

Slicing is cheap. `s[a, b]` does not rebuild the string character by
character, which means building up output by repeated slicing stays linear
rather than quadratic.
