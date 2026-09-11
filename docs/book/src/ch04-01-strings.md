# Strings

A Zuri string is UTF-8 text. Indexing, slicing and `length()` work in
characters, not bytes, so a string of emoji behaves the way a reader
expects.

Text is the type you will write most, so this section starts with how you
write one down — quoting, escapes and interpolation — before moving on to
what you can do with it.

## Writing a String

A string literal is written between single or double quotes:

```zuri
echo 'single quotes'
echo "double quotes"
```

```console
single quotes
double quotes
```

**The two styles are identical.** They are not the two different things
they are in some other languages. Both process escape sequences, both
interpolate expressions, and both may span several lines. Nothing at all
changes except which quote character ends the string:

```zuri
var name = 'Zuri'

echo 'escape \t and interpolate ${name}'
echo "escape \t and interpolate ${name}"
```

```console
escape 	 and interpolate Zuri
escape 	 and interpolate Zuri
```

Pick whichever avoids escaping. A string containing an apostrophe is
easiest in double quotes; a string containing a quotation mark is easiest
in single quotes:

```zuri
echo "it's clearer this way"
echo 'she said "hello"'
```

```console
it's clearer this way
she said "hello"
```

Most Zuri code, and all of the standard library, uses single quotes by
default and switches to double quotes when the content calls for it.

### Multi-line Strings

A literal may simply contain newlines. There is no separate triple-quoted
form and no continuation character:

```zuri
var message = 'Dear reader,

Thank you for the strings.'

echo message
```

```console
Dear reader,

Thank you for the strings.
```

Everything between the quotes is part of the string, indentation included.
When a long string is indented inside a function, the leading spaces on
each line are in the string too, which is why it is advisable that code
that builds indented text should assemble it with `join()` rather than
writing one long literal.

## Escape Sequences

A backslash starts an escape. The full set is:

| Escape | Produces |
| --- | --- |
| `\0` | the null character, U+0000 |
| `\a` | the alert (bell) character, U+0007 |
| `\b` | backspace, U+0008 |
| `\t` | horizontal tab, U+0009 |
| `\n` | line feed, U+000A |
| `\v` | vertical tab, U+000B |
| `\f` | form feed, U+000C |
| `\r` | carriage return, U+000D |
| `\\` | a single backslash |
| `\'` | a single quote |
| `\"` | a double quote |
| `\xNN` | the byte with hexadecimal value `NN` |
| `\uNNNN` | the code point `U+NNNN` |
| `\UNNNNNNNN` | the code point `U+NNNNNNNN` |

The three numeric forms take exactly the number of hex digits shown — two,
four and eight — and the digits may be upper or lower case:

```zuri
echo 'hex byte: \x41'
echo 'code point: \u00e9'
echo 'astral: \U0001F600'
```

```console
hex byte: A
code point: é
astral: 😀
```

`\u` covers everything up to U+FFFF. Anything above that — emoji, most
historic scripts, mathematical alphanumerics — needs `\U` and its eight
digits. Zero-pad to fill them.

### Unknown Escapes Are Left Alone

A backslash followed by anything not in the table above is **not** an
error, and it is not silently dropped either. The backslash and the
character both survive:

```zuri
echo 'a regex: \d+ and \w+'
echo 'a path: C:\temp\notes'
```

```console
a regex: \d+ and \w+
a path: C:	emp
otes
```

The first line came through untouched: `\d` and `\w` are not escapes, so
both backslashes survived, which is exactly why regular expressions can be
written as plain strings.

The second line did not. `\t` *is* an escape, so `C:\temp` became `C:`
followed by a tab and then `emp`; `\n` is an escape too, so `\notes` became
a newline followed by `otes`. One path, two silent substitutions. The rule
is easy to state and easy to forget: unknown escapes pass through, known
ones do not, and nothing in the way you write them tells you which is
which.

For Windows paths and regular expressions — the two places this bites —
double the backslashes:

```zuri
echo 'a path: C:\\temp\\notes'
```

```console
a path: C:\temp\notes
```

### Escaping the Quote You Chose

You only need to escape the quote character that would end the string:

```zuri
echo 'it\'s escaped'
echo "she said \"hi\""
```

```console
it's escaped
she said "hi"
```

Escaping the *other* quote is harmless but pointless, and because an
unnecessary escape is an unknown escape, the backslash stays in the result:

```zuri
echo 'this \" keeps its backslash'
```

```console
this \" keeps its backslash
```

That is a good reason to choose the quote style that lets you write the
text plainly and escape nothing at all.

## Interpolation

`${...}` inside a string evaluates the expression between the braces and
splices its text into the result:

```zuri
var name = 'Ada'
var year = 1843

echo 'In ${year}, ${name} wrote the first program.'
```

```console
In 1843, Ada wrote the first program.
```

This works in both quote styles, and it is the way most Zuri code builds a
string. The section after next covers `+`, which does the same job with
more punctuation.

### Any Expression Fits

The braces take a whole expression, not just a variable name. Arithmetic,
method calls, indexing, comparisons and conditionals all work:

```zuri
var items = ['a', 'b', 'c']
var user = { name: 'ada', admin: true }

echo 'count: ${items.length()}'
echo 'first: ${items[0].upper()}'
echo 'math: ${(2 ** 10) - 24}'
echo 'role: ${user.admin ? 'admin' : 'user'}'
echo 'name: ${user.name.capitalize()}'
```

```console
count: 3
first: A
math: 1000
role: admin
name: Ada
```

Notice the third line. The quotes inside `${...}` are the **same** quote
character that opened the string, and it still parses correctly, because
the expression inside the braces is lexed as its own piece of code rather
than as text. You do not need to switch quote styles or escape anything to
put a string literal inside an interpolation.

### Interpolations Nest

A string literal inside `${...}` is a string literal like any other, which
means it may contain interpolations of its own, to any depth:

```zuri
var n = 1

echo 'outer ${ 'inner ${n + 1}' }'
echo '${ '${ '${2 ** 3}' }' }'
```

```console
outer inner 2
8
```

This is not a curiosity. It is what makes a conditional inside a string
able to produce formatted text rather than a bare word, which comes up
constantly in messages meant for a person to read:

```zuri
var user = { name: 'ada', unread: 3 }

echo 'Hi ${user.name.capitalize()}, you have ${user.unread > 0 ? '${user.unread} new ${user.unread == 1 ? 'message' : 'messages'}' : 'nothing new'}.'
```

```console
Hi Ada, you have 3 new messages.
```

Three levels of interpolation are at work there: the outer message, the
branch that chooses between a count and "nothing new", and the branch that
picks the singular or the plural. Each one is an ordinary string literal in
an ordinary expression, and each one uses single quotes, because the
expression inside `${...}` is lexed as code and its quotes never collide
with the ones around it.

Nesting is equally useful inside a callback, where the inner string is
building one element of a larger result:

```zuri
var items = ['a', 'b']

echo 'list: ${', '.join(items.map(@(i) => '<${i}>'))}'
```

```console
list: <a>, <b>
```

Depth costs readability quickly. When a line like the message above stops
being scannable, lift the inner pieces into local variables on the lines
before it; the language is happy either way.

### What Each Type Looks Like

Interpolation renders a value the same way `echo` does:

```zuri
var settings = { a: 1 }

echo 'number: ${1 / 3}'
echo 'list: ${[1, 2]}'
echo 'dict: ${settings}'
echo 'nil: ${nil}'
echo 'bool: ${true}'
```

```console
number: 0.3333333333333333
list: [1, 2]
dict: {a: 1}
nil: nil
bool: true
```

A dictionary **literal** written directly inside `${...}` does not parse,
because its closing brace runs into the interpolation's closing brace:

```zuri,ignore
echo 'dict: ${{ a: 1 }}'
```

```console
SyntaxError: Expected '}' after dictionary
```

Put it in a variable first, as above. A list literal has no such problem,
since `]` and `}` are different characters.

There is one important exception, and it catches everyone once.
**Interpolating a class instance does not call its `to_string()` method:**

```zuri
class Point {

  @new(x, y) {
    self.x = x
    self.y = y
  }

  to_string() {
    return '(${self.x}, ${self.y})'
  }
}

var p = Point(3, 4)

echo 'implicit: ${p}'
echo 'explicit: ${p.to_string()}'
```

```console
implicit: <instance of Point>
explicit: (3, 4)
```

Call the method yourself. This applies to `echo p` too, and to
`'text ' + p`. The rule is that Zuri never calls a method on your behalf to
produce text; if you want `to_string()` to run, write it.

### Writing a Literal `${`

A backslash does not escape an interpolation. `\${x}` is an unknown escape,
so the backslash survives and the interpolation still does not happen —
which is rarely what anyone wants:

```zuri
echo 'literal attempt: \${x}'
```

```console
literal attempt: \${x}
```

To produce the two characters `${` followed by text, split the string so
the `$` and the `{` are never adjacent inside one literal:

```zuri
echo 'shell syntax: $' + '{HOME}'
```

```console
shell syntax: ${HOME}
```

A lone `$` is not special, so only the exact sequence `${` needs this
treatment:

```zuri
echo 'price: $5.00'
echo 'total: $${12 + 8}'
```

```console
price: $5.00
total: $20
```

## Joining Strings Together

### Concatenation with `+`

`+` joins two strings end to end:

```zuri
echo 'Hello, ' + 'world'
echo 'a' + 'b' + 'c'
```

```console
Hello, world
abc
```

When one side is a string, the other side is converted to its text form
first. This works in both directions, and for every built-in type:

```zuri
echo 'n=' + 5
echo 5 + 'n'
echo 'pi is about ' + 3.14
echo 'flag: ' + true
echo 'missing: ' + nil
echo 'items: ' + [1, 2]
```

```console
n=5
5n
pi is about 3.14
flag: true
missing: nil
items: [1, 2]
```

Read the second line again: `5 + 'n'` gives `'5n'`, not an error and not
arithmetic. A string on *either* side turns the whole expression into
concatenation. That is worth knowing when a value arrives from somewhere
you do not control:

```zuri
var quantity = '2'

echo quantity + 3
echo quantity.to_number() + 3
```

```console
23
5
```

The first line is text, the second is arithmetic, and nothing in the
expression tells you which you are getting. When a number must be a number,
convert it at the point it enters your program rather than hoping.

The one type that does **not** convert usefully is a class instance:

```zuri
class Point {

  @new(x, y) {
    self.x = x
    self.y = y
  }

  to_string() {
    return '(${self.x}, ${self.y})'
  }
}

echo 'at ' + Point(1, 2).to_string()
```

```console
at (1, 2)
```

Concatenating the instance directly would produce `<instance of Point>`,
for the same reason interpolation does: Zuri does not call `to_string()`
for you.

### Concatenation Versus Interpolation

Both produce the same string, so choose on readability:

```zuri
var host = 'localhost'
var port = 8080

echo 'connecting to ' + host + ':' + port + '/health'
echo 'connecting to ${host}:${port}/health'
```

```console
connecting to localhost:8080/health
connecting to localhost:8080/health
```

Interpolation wins whenever there is more than one hole to fill: the
literal text stays in one piece, the quotes do not multiply, and there is
no chance of losing a separator between two `+` signs. Reach for `+` when
you are gluing exactly two pieces together, or when one of them is already
a variable holding a complete string.

### Repetition with `*`

`*` repeats a string a whole number of times:

```zuri
echo 'ab' * 3
echo '-' * 20
echo '  ' * 2 + 'indented'
```

```console
ababab
--------------------
    indented
```

That second line is the reason this operator earns its place. Separators,
rules, indentation and padding are all one short expression instead of a
loop.

The rules for the count are worth stating exactly:

- A count of `1` returns the string unchanged.
- A count of `0` returns the empty string.
- A **negative** count also returns the empty string, rather than raising.
- A **fractional** count is truncated toward zero, so `* 2.7` repeats
  twice.

```zuri
echo '[' + 'ab' * 1 + ']'
echo '[' + 'ab' * 0 + ']'
echo '[' + 'ab' * -1 + ']'
echo '[' + 'ab' * 2.7 + ']'
```

```console
[ab]
[]
[]
[abab]
```

The negative case is the one to watch. `'-' * (width - label.length())`
silently produces nothing when the label is longer than the width, and no
error tells you so. When the count is computed, clamp it:

```zuri
def rule(width, label) {
  var padding = (width - label.length()).max(0)

  return label + '-' * padding
}

echo rule(10, 'ab')
echo rule(10, 'a much longer label')
```

```console
ab--------
a much longer label
```

Unlike `+`, repetition does not commute. The string has to be on the left:

```zuri,ignore
echo 3 * 'ab'
```

```console
Unhandled TypeError: operator '*' not defined for call signature (number, string)
```

### Building a String from Many Pieces

For more than a handful of pieces, neither operator is the right tool.
Collect them in a list and `join()`:

```zuri
var names = ['ada', 'grace', 'alan']

echo ', '.join(names)
echo '\n'.join(names.map(@(n) => '- ' + n.capitalize()))
```

```console
ada, grace, alan
- Ada
- Grace
- Alan
```

`join()` lives on the **separator**, not on the list, and it converts each
element to text on the way — so a list of numbers joins as readily as a
list of strings:

```zuri
echo '-'.join([1, 2, 3])
```

```console
1-2-3
```

Note also that `join()` puts the separator *between* elements, never at
either end, which is exactly the behaviour a hand-written loop usually gets
wrong on the last item.

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

`last_index_of()` searches from the other end, which is what you want
whenever the interesting separator is the final one:

```zuri
var path = 'src/vm/value.rs'

echo path.index_of('/')
echo path.last_index_of('/')
echo path[path.last_index_of('/') + 1, path.length()]
echo path.last_index_of('\\')
```

```console
3
6
value.rs
-1
```

Both take a second argument, and in both it bounds where a *match may
begin*. That makes the pair split a string at one index: for any `n`,
`index_of(str, n)` finds the first match at or after `n` and
`last_index_of(str, n)` the last match at or before it.

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

Zuri's regular expressions are **PCRE2**, the same engine that powers
regular expressions in PHP, R and a long list of other tools. It is the
real library rather than a subset of it, so named groups, backreferences,
lookahead, lookbehind, atomic groups, possessive quantifiers, conditionals
and recursion all work exactly as PCRE2 documents them.

This section covers how a pattern is written in Zuri, which methods accept
one, what each modifier does, and the handful of places where Zuri's
surface differs from what you may expect. It does not teach regular
expression syntax itself; any PCRE or Perl reference applies unchanged.

### Writing a Pattern

A pattern is an ordinary string whose **first character is a non-word
character, repeated to close the pattern**, with any modifier letters
after the closing delimiter:

```text
'/[a-z]+/'
'/[a-z]+/i'
'#\d{3}#'
'!https?://\S+!i'
```

A "non-word character" is anything that is not a letter, a digit or an
underscore. Forward slash is the conventional choice, and every example in
this book uses it, but the delimiter is yours to pick — which is worth
doing when the pattern itself is full of slashes:

```zuri
echo 'see https://example.com/docs now'.match('!https?://\S+!')
```

```console
{0: https://example.com/docs}
```

Because the pattern is a string, the backslashes in it are subject to
Zuri's own escape rules first. This is almost never a problem, because
`\d`, `\w`, `\s`, `\b` and `\S` are not escape sequences and therefore pass
through untouched. The exceptions are the letters that *are* escapes —
`\t`, `\n`, `\r`, `\f`, `\v`, `\a`, `\b`, `\0`, `\x` and `\u`. Of those,
`\t` and `\n` mean the same thing to both, so they are harmless; `\b`
(word boundary in a pattern, backspace in a string) is the one that will
catch you:

```zuri
echo 'one two'.match('/\btwo\b/')
echo 'one two'.match('/\\btwo\\b/')
```

```console
false
{0: two}
```

Double the backslash whenever the pattern needs `\b`.

### A Plain String Is a Literal

Every method in this section also accepts a plain string, and treats it as
a **literal substring** rather than a pattern:

```zuri
echo 'a.b'.replace('.', 'X')
echo 'a.b'.replace('/./', 'X')
```

```console
aXb
XXX
```

The first call replaced the single full stop. The second compiled `.` as a
pattern, where it means "any character", and replaced all three.

That distinction is decided purely by whether the string looks like a
delimited pattern. When you have text from elsewhere that might
accidentally look like one, `replace()` takes a third argument that turns
pattern handling off:

```zuri
echo 'a/b/c'.replace('/b/', 'X')
echo 'a/b/c'.replace('/b/', 'X', false)
```

```console
a/X/c
aXc
```

With `false`, the delimiters are matched as characters like anything else.

### Which Methods Take a Pattern

Five, and only five:

| Method | What it does with the pattern |
| --- | --- |
| `match(pattern)` | the first match, or `false` |
| `matches(pattern)` | every match, grouped by capture group |
| `replace(pattern, with, use_regex)` | replaces every match |
| `replace_with(pattern, callback)` | replaces every match with a call |
| `split(pattern)` | splits on every match |

Everything else — `contains()`, `index_of()`, `count()`, `starts_with()`,
`ends_with()`, `trim()` — takes a **literal** string only. A
regex-looking argument passed to one of those is matched as the literal
characters it contains:

```zuri
echo 'a1b'.contains('/[0-9]/')
echo 'a1b'.index_of('/[0-9]/')
echo 'a1b'.count('/[0-9]/')
```

```console
false
-1
0
```

All three answered about the six-character string `/[0-9]/`, which is not
in `a1b`. When you need a pattern there, reach for `match()` instead.

### `match()` and `matches()`

`match()` finds the **first** match and returns it as a dictionary under
key `0`. It returns `false` — not `nil` — when nothing matches:

```zuri
echo 'one two'.match('/\w+/')
echo 'nope'.match('/zzz/')
echo typeof('nope'.match('/zzz/'))
```

```console
{0: one}
false
bool
```

Both are falsy, so `if s.match(p) { ... }` reads correctly. An equality
test against `nil` does not, and this is the single most common regex
mistake in Zuri code.

`matches()` finds **every** match and groups the results by capture group.
Key `0` holds the whole matches, key `1` the first group's, and so on —
each as a list, parallel across the keys:

```zuri
echo 'a1b2c3'.matches('/[0-9]/')
echo 'a1b2'.matches('/([a-z])([0-9])/')
```

```console
{0: [1, 2, 3]}
{0: [a1, b2], 1: [a, b], 2: [1, 2]}
```

In the second result, match zero is `a1` with groups `a` and `1`, and
match one is `b2` with groups `b` and `2`. Read down the lists, not
across.

When nothing matches, `matches()` returns `{0: []}` rather than `false`.
The two methods differ here, so test `matches()` with `is_empty()` on key
zero:

```zuri
var found = 'nope'.matches('/zzz/')

echo found
echo found[0].is_empty()
```

```console
{0: []}
true
```

### Capture Groups in a Replacement

`replace()` refers to a capture group with `$1`, `$2` and so on:

```zuri
echo 'John Smith'.replace('/(\w+) (\w+)/', '$2, $1')
```

```console
Smith, John
```

**Do not write `${1}`.** The braced form is Zuri's own string
interpolation, and it is consumed before `replace()` ever sees it. The
expression `2` evaluates to the number two, so the replacement string is
already `'2, 1'` by the time it arrives:

```zuri
echo 'John Smith'.replace('/(\w+) (\w+)/', '${2}, ${1}')
```

```console
2, 1
```

Use the bare `$1` form throughout. This is one of the few places where
Zuri's interpolation and another language's syntax collide, and it fails
quietly rather than loudly.

### Named Groups

Named groups are supported in the **pattern**, because PCRE2 supports
them:

```zuri
echo 'John Smith'.matches('/(?<first>\w+) (?<last>\w+)/')
```

```console
{0: [John Smith], 1: [John], 2: [Smith]}
```

The results are keyed by **position**, not by name. A name in a pattern
documents the group and lets a backreference such as `\k<first>` refer to
it; it does not change how the result is keyed. Count the groups to find
the one you want.

### Computing the Replacement

When the replacement depends on what matched, `replace_with()` calls a
function for each match:

```zuri
echo 'a1b2'.replace_with('/[0-9]/', @(m) => '<' + m + '>')
```

```console
a<1>b<2>
```

The callback receives the whole match first, then one argument per capture
group, then the offset of the match, then the whole subject string. Take
only the arguments you need, since extra arguments are dropped:

```zuri
echo 'aXbXc'.replace_with('/X/', @(match, offset) => '[${offset}]')
```

```console
a[1]b[3]c
```

### Splitting on a Pattern

`split()` accepts a pattern, which is how you split on "one or more of
something" rather than on an exact separator:

```zuri
echo 'a1b22c'.split('/[0-9]+/')
echo 'a, b,c ,  d'.split('/\s*,\s*/')
```

```console
[a, b, c]
[a, b, c, d]
```

### The Modifiers

Modifier letters go after the closing delimiter, in any order and any
combination. `/[a-z]+/mi` is both multi-line and case-insensitive.

| Modifier | Effect |
| --- | --- |
| `i` | Case-insensitive matching. |
| `m` | Multi-line: `^` and `$` match at every line boundary, not only at the start and end of the subject. |
| `s` | Dot-all: `.` matches a newline as well as everything else. |
| `x` | Extended: unescaped whitespace in the pattern is ignored, and `#` starts a comment to end of line. |
| `u` | Unicode properties: `\d`, `\w`, `\s` and friends become Unicode-aware instead of ASCII-only. |
| `U` | Ungreedy: quantifiers become lazy by default, and `?` after one makes it greedy. |
| `A` | Anchored: a match is only accepted if it begins exactly where the search started. |
| `J` | Allow two capture groups in one pattern to share a name. |

Each one in use:

```zuri
echo 'HELLO'.match('/hello/i')
echo 'a\nb'.matches('/^./m')
echo 'a\nb'.match('/a.b/s')
echo 'abc'.match('/ a b c /x')
echo 'héllo'.matches('/\w/u')
echo 'aaa'.match('/a+?/U')
echo 'abc'.match('/abc/A')
echo 'xabc'.match('/abc/A')
```

```console
{0: HELLO}
{0: [a, b]}
{0: a
b}
{0: abc}
{0: [h, é, l, l, o]}
{0: aaa}
{0: abc}
false
```

Read the last two together: the pattern matched when it was at the start of
the subject and failed when it was not, which is what `A` is for. Without
it, the second would have matched at offset one.

`u` is worth a second look as well. Without it, `\w` is ASCII-only and
`é` is not a word character; with it, the Unicode properties apply and it
is. Matching itself is always per-character rather than per-byte, so
indexes and offsets are correct on multi-byte text regardless of `u`.

### The One Exception to PCRE2 Compatibility

**PCRE2's `D` (dollar-endonly) modifier has no effect in Zuri.** `$`
always matches before a trailing newline as well as at the absolute end of
the subject, and there is no way to change that. The letter is accepted
rather than rejected, so a pattern carrying it compiles and runs; it simply
does not do anything.

The same is true of any other unrecognised modifier letter: it is accepted
and ignored rather than raising. A typo in a modifier is therefore silent,
which is worth remembering when a pattern behaves as though a flag you
wrote is not set.

Everything else in the table above is supported and behaves exactly as
PCRE2 documents it.

### Invalid Patterns

A pattern that PCRE2 cannot compile raises, naming the pattern and the
engine's own explanation:

```zuri
catch {
  echo 'text'.match('/(unclosed/')
} as e {
  echo e.message
}
```

```console
invalid regular expression '(unclosed': PCRE2: error compiling pattern at offset 9: missing closing parenthesis
```

Patterns are compiled once and reused, so repeating the same pattern in a
loop costs nothing after the first time through.

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

### `to_number()` Never Fails

This is the one conversion behaviour worth memorising. `to_number()` does
not raise, and it does not produce `NaN`. Text it cannot parse becomes
**zero**:

```zuri
echo '42'.to_number()
echo '3.5'.to_number()
echo '-5'.to_number()
echo 'eighty'.to_number()
echo ''.to_number()
echo '12abc'.to_number()
echo '  7  '.to_number()
```

```console
42
3.5
-5
0
0
0
0
```

Look at the last three. `'12abc'` is not read as `12` and stopped; it is
zero. And `'  7  '` is zero as well, because the surrounding spaces are not
trimmed for you.

There is no error to catch and no sentinel to test, so a form field a user
left blank and a form field they filled with `'0'` produce the same number.
When the difference matters, check the text before converting it:

```zuri
def to_count(text) {
  var trimmed = text.trim()

  if !trimmed.match('/^\d+$/') {
    raise ValueError('not a whole number: ${text}')
  }

  return trimmed.to_number()
}

echo to_count('  12  ')

catch {
  to_count('twelve')
} as e {
  echo e.message
}
```

```console
12
not a whole number: twelve
```

`is_number()` is a narrower test than it looks — it is true only for a
string of digits, so `'3.5'` and `'-5'` both fail it. A regular expression
is the reliable check for anything beyond unsigned integers.

## Walking a String

A string is iterable, one character at a time, and the same four forms
available for a list work here.

### `for`, One Character at a Time

```zuri
for ch in 'hey' {
  echo ch
}
```

```console
h
e
y
```

This is the form to reach for by default. Each `ch` is a one-character
string, not a code point number — use `ord()` when you want the number.

### `for` With Two Variables, for the Position

```zuri
for index, ch in 'hey' {
  echo '${index}: ${ch}'
}
```

```console
0: h
1: e
2: y
```

The key comes first and the value second, and for a string the key is the
character position.

### `iter`, When the Position Drives the Walk

```zuri
var s = 'hey'

iter var i = s.length() - 1; i >= 0; i-- {
  echo s[i]
}
```

```console
y
e
h
```

`s[i]` indexes by character, not by byte, so this is correct on text that
is not ASCII. Use `iter` when you need to skip, step backwards, or look at
`s[i + 1]` from inside the body.

### `each()`, With a Function

```zuri
'Hi'.each(@(ch, index) {
  echo '${index}:${ch}'
})
```

```console
0:H
1:i
```

As everywhere else, the callback receives the **value first and the index
second** — the opposite order to `for`.

### `each_line()` and `lines()`, for Text in Lines

```zuri
var doc = 'first\nsecond\nthird'

doc.each_line(@(line, index) {
  echo '${index}: ${line}'
})

echo doc.lines()
```

```console
0: first
1: second
2: third
[first, second, third]
```

`each_line()` calls the function once per line; `lines()` hands you the
whole list so you can `for` over it, filter it, or count it. Both split on
`\n` and `\r\n`, so both read a file written on any platform.

## Ordering and Comparison

`==` compares two strings by value, which is almost always what you want:

```zuri
echo 'abc' == 'abc'
echo 'abc' == 'ABC'
```

```console
true
false
```

The ordering operators are a different story. `<`, `<=`, `>` and `>=` are
defined for numbers only, and comparing two strings with one raises a
`TypeError`. Use `compare()`, which returns `-1`, `0` or `1`:

```zuri
echo 'abc'.compare('abd')
echo 'abc'.compare('abc')
echo 'abd'.compare('abc')
```

```console
-1
0
1
```

That three-way result is exactly the shape a sort comparison wants, and it
compares by code point, so it is stable and locale-independent.

## Strings Are Immutable

Every method in this section returns a new string. Nothing mutates in
place:

```zuri
var original = 'hello'
var shouted = original.upper()

echo original
echo shouted
```

```console
hello
HELLO
```

This is worth internalising, because it is the source of the single most
common string mistake:

```zuri
var name = '  ada  '

name.trim()
echo '[${name}]'

name = name.trim()
echo '[${name}]'
```

```console
[  ada  ]
[ada]
```

The first `trim()` produced a trimmed string and threw it away. Assign the
result, or chain onto it.

Immutability also means you can pass a string anywhere without copying it
defensively. Nothing you hand a string to can change the one you still
hold.

## A Worked Example

Here is a small parser that puts most of this section together. It takes a
block of `key = value` configuration text and produces a dictionary,
skipping blank lines and comments, and tolerating whatever spacing the
author used.

```zuri
def parse_config(text) {
  var config = {}

  for line in text.lines() {
    var trimmed = line.trim()

    if trimmed.is_empty() or trimmed.starts_with('#') {
      continue
    }

    var split_at = trimmed.index_of('=')

    if split_at == -1 {
      continue
    }

    var key = trimmed[0, split_at].trim()
    var value = trimmed[split_at + 1, trimmed.length()].trim()

    config[key] = value
  }

  return config
}

var source = '# server settings
host = localhost
port   =   8080

# empty lines and comments are skipped
name = zuri app
'

var config = parse_config(source)

echo config.host
echo config.port.to_number() + 1
echo config.name
echo config.length()
```

```console
localhost
8081
zuri app
3
```

Three things in there are worth naming. `lines()` handles both `\n` and
`\r\n`, so the same code reads a file written on Windows. `index_of()`
returning `-1` is checked explicitly rather than relied on for truthiness,
because `-1` is falsy and so is `0` — and `0` is a legitimate position.
And every value arrives as a string, because that is what text is;
`to_number()` is how you leave.
