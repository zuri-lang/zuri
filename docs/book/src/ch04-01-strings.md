# Strings

A Zuri string is UTF-8 text. Indexing, slicing and `length()` all work in
characters rather than bytes, so a string of emoji behaves the way you'd
expect.

You'll probably work with strings more than any other type, so this section
starts with how to write them (quoting, escape sequences and interpolation)
before moving on to what you can do with them.

## String Literals

A string literal is written in single or double quotes:

```zuri
echo 'single quotes'
echo "double quotes"
```

```console
single quotes
double quotes
```

**The two styles are identical.** Unlike in some other languages, they don't
mean different things. Both process escape sequences, both support
interpolation, and both can span several lines. The only difference is which
quote character ends the string:

```zuri
var name = 'Zuri'

echo 'escape \t and interpolate ${name}'
echo "escape \t and interpolate ${name}"
```

```console
escape 	 and interpolate Zuri
escape 	 and interpolate Zuri
```

Choose whichever one saves you from escaping. A string that contains an
apostrophe is easiest to write in double quotes, and one that contains a
double quote is easiest in single quotes:

```zuri
echo "it's clearer this way"
echo 'she said "hello"'
```

```console
it's clearer this way
she said "hello"
```

Most Zuri code, including the whole standard library, uses single quotes by
default and switches to double quotes when the content needs it.

### Multi-line Strings

A string literal can contain newlines directly. There's no special
triple-quoted syntax and no line continuation character:

```zuri
var message = 'Dear reader,

Thank you for the strings.'

echo message
```

```console
Dear reader,

Thank you for the strings.
```

Everything between the quotes is part of the string, including indentation.
If you indent a long string inside a function, the leading spaces on each
line end up in the string too. For that reason, code that builds indented
text is usually better off assembling it with `join()` than writing one long
literal.

## Escape Sequences

A backslash starts an escape sequence. These are all of them:

| Escape | Produces |
| --- | --- |
| `\0` | The null character, U+0000 |
| `\a` | The alert (bell) character, U+0007 |
| `\b` | Backspace, U+0008 |
| `\t` | Horizontal tab, U+0009 |
| `\n` | Line feed, U+000A |
| `\v` | Vertical tab, U+000B |
| `\f` | Form feed, U+000C |
| `\r` | Carriage return, U+000D |
| `\\` | A single backslash |
| `\'` | A single quote |
| `\"` | A double quote |
| `\${` | A literal `${`, with no interpolation |
| `\xNN` | The byte with the hexadecimal value `NN` |
| `\uNNNN` | The code point `U+NNNN` |
| `\UNNNNNNNN` | The code point `U+NNNNNNNN` |

The three numeric forms take exactly the number of hex digits shown (two,
four and eight), and the digits can be uppercase or lowercase:

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

`\u` covers code points up to U+FFFF. Anything higher, such as emoji, most
historic scripts and mathematical alphanumerics, needs `\U` with eight
digits. Pad the value with leading zeros to fill them.

### Unknown Escape Sequences

A backslash followed by a character that isn't in the table above is **not**
an error, and it isn't silently removed either. Both the backslash and the
character stay in the string:

```zuri
echo 'a regex: \d+ and \w+'
echo 'a path: C:\temp\notes'
```

```console
a regex: \d+ and \w+
a path: C:	emp
otes
```

The first line comes through unchanged. `\d` and `\w` aren't escape
sequences, so both backslashes are kept, and that's why you can write
regular expressions as ordinary strings.

The second line doesn't. `\t` *is* an escape sequence, so `C:\temp` becomes
`C:` followed by a tab and `emp`. `\n` is an escape sequence too, so
`\notes` becomes a newline followed by `otes`. That's two silent
substitutions in a single path. The rule is simple but easy to forget:
unknown escape sequences are left alone and known ones aren't, and nothing
in the way you write them tells you which is which.

Windows paths and regular expressions are where this usually causes trouble.
In both, double the backslashes:

```zuri
echo 'a path: C:\\temp\\notes'
```

```console
a path: C:\temp\notes
```

### Escaping Quotes

You only need to escape the quote character that would otherwise end the
string:

```zuri
echo 'it\'s escaped'
echo "she said \"hi\""
```

```console
it's escaped
she said "hi"
```

Escaping the *other* quote character doesn't do anything useful. Since it
isn't a known escape sequence, the backslash stays in the result:

```zuri
echo 'this \" keeps its backslash'
```

```console
this \" keeps its backslash
```

That's another reason to pick the quote style that lets you write the text
without escaping anything.

## Interpolation

`${...}` inside a string evaluates the expression between the braces and
inserts its text into the string:

```zuri
var name = 'Ada'
var year = 1843

echo 'In ${year}, ${name} wrote the first program.'
```

```console
In 1843, Ada wrote the first program.
```

Interpolation works with both quote styles, and it's how most Zuri code
builds strings. Two sections from now, we'll look at `+`, which does the
same job with more punctuation.

### Using Any Expression

The braces can contain any expression, not just a variable name. Arithmetic,
method calls, indexing, comparisons and conditional expressions all work:

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

Look at the third line. The quotes inside `${...}` are the **same** quote
character that started the string, and it still works. That's because the
expression inside the braces is read as code, not as part of the text. You
don't need to switch quote styles or escape anything to put a string literal
inside an interpolation.

### Nested Interpolation

A string literal inside `${...}` is an ordinary string literal, so it can
contain interpolations of its own, nested as deeply as you like:

```zuri
var n = 1

echo 'outer ${ 'inner ${n + 1}' }'
echo '${ '${ '${2 ** 3}' }' }'
```

```console
outer inner 2
8
```

This is more useful than it might sound. It lets a conditional inside a
string produce formatted text instead of a single word, which you'll need
often in messages meant for people to read:

```zuri
var user = { name: 'ada', unread: 3 }

echo 'Hi ${user.name.capitalize()}, you have ${user.unread > 0 ? '${user.unread} new ${user.unread == 1 ? 'message' : 'messages'}' : 'nothing new'}.'
```

```console
Hi Ada, you have 3 new messages.
```

There are three levels of interpolation here: the outer message, the branch
that chooses between a count and "nothing new", and the branch that chooses
between singular and plural. Each level is an ordinary string literal in an
ordinary expression, and each one uses single quotes. The quotes never
clash, because the expression inside `${...}` is read as code.

Nesting is also handy inside a callback, where the inner string builds one
piece of a larger result:

```zuri
var items = ['a', 'b']

echo 'list: ${', '.join(items.map(@(i) => '<${i}>'))}'
```

```console
list: <a>, <b>
```

Deep nesting gets hard to read quickly. When a line like the message above
becomes difficult to follow, move the inner parts into local variables on
the lines before it. Both versions work the same way.

### How Values Are Displayed

Interpolation displays a value the same way `echo` does:

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

You can't write a dictionary **literal** directly inside `${...}`, because
its closing brace would end the interpolation:

```zuri,ignore
echo 'dict: ${{ a: 1 }}'
```

```console
SyntaxError: Expected '}' after dictionary
```

Assign it to a variable first, as in the example above. List literals don't
have this problem, because `]` and `}` are different characters.

There's one important exception that catches almost everyone at least once.
**Interpolating a class instance doesn't call its `to_string()` method:**

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

You have to call the method yourself. The same applies to `'text ' + p`.
Neither interpolation nor `+` will call a method for you to get the text. If
you want `to_string()` to run, call it. `echo` is the exception, through
`@to_string()`, which is covered in [Decorated
Methods](ch06-03-decorated-methods.md).

### Writing a Literal `${`

A backslash in front of `${` prevents interpolation, and the backslash
itself is removed:

```zuri
var x = 7
echo 'literal: \${x}'
echo 'value: ${x}'
```

```console
literal: ${x}
value: 7
```

The escape sequence is a backslash, a dollar sign and a `{`. `\\` is a
separate escape sequence, so `\\${x}` gives you a backslash followed by the
value of `x`. Anywhere else, `\$` is an unknown escape sequence and keeps
its backslash, which is how a regular expression can match a literal dollar
sign:

```zuri
var x = 7
echo '\\${x}'
echo 'price: $100'.match('/(?<=\$)\d+/')
```

```console
\7
{0: 100}
```

A `$` on its own has no special meaning, so you only need to escape the
exact sequence `${`:

```zuri
echo 'price: $5.00'
echo 'total: $${12 + 8}'
```

```console
price: $5.00
total: $20
```

## Combining Strings

### Concatenation with `+`

`+` joins two strings together:

```zuri
echo 'Hello, ' + 'world'
echo 'a' + 'b' + 'c'
```

```console
Hello, world
abc
```

When one side is a string, the other side is converted to text first. This
works whichever side the string is on, and for all built-in types:

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

Look at the second line again. `5 + 'n'` gives `'5n'`. It's not an error,
and it's not arithmetic. A string on *either* side turns the whole
expression into concatenation. Keep this in mind when a value comes from a
source you don't control:

```zuri
var quantity = '2'

echo quantity + 3
echo quantity.to_number() + 3
```

```console
23
5
```

The first line is string concatenation and the second is arithmetic, and
nothing in the expression tells you which one you'll get. When a value needs
to be a number, convert it as soon as it enters your program.

The one type that **doesn't** convert in a useful way is a class instance:

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

Concatenating the instance directly would give you `<instance of Point>`,
for the same reason as with interpolation: Zuri doesn't call `to_string()`
for you.

### Concatenation or Interpolation?

Both produce the same string, so choose whichever is easier to read:

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

Interpolation is usually better when there's more than one value to insert.
The literal text stays in one piece, you don't end up with lots of quotes,
and you can't accidentally lose a separator between two `+` signs. Use `+`
when you're joining just two pieces, or when one of them is a variable that
already holds a complete string.

### Repetition with `*`

`*` repeats a string a given number of times:

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

The second line shows why this operator is useful. Separators, horizontal
rules, indentation and padding each become one short expression instead of a
loop.

Here are the exact rules for the count:

- A count of `1` returns the string unchanged.
- A count of `0` returns an empty string.
- A **negative** count also returns an empty string instead of raising an
  error.
- A **fractional** count is truncated toward zero, so `* 2.7` repeats the
  string twice.

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

Watch out for the negative case. `'-' * (width - label.length())` quietly
produces nothing when the label is longer than the width, and there's no
error to tell you. When the count is calculated, clamp it:

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

Unlike `+`, the order matters for repetition. The string has to be on the
left:

```zuri,ignore
echo 3 * 'ab'
```

```console
Unhandled TypeError: operator '*' not defined for call signature (number, string)
```

### Building a String From Many Pieces

When you have more than a few pieces, neither operator is a good fit.
Collect the pieces in a list and call `join()`:

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

`join()` is a method of the **separator**, not of the list. It converts each
element to text as it goes, so a list of numbers can be joined just as
easily as a list of strings:

```zuri
echo '-'.join([1, 2, 3])
```

```console
1-2-3
```

`join()` also puts the separator only *between* elements, never at the
beginning or end. That's exactly the part that hand-written loops tend to
get wrong for the last item.

## Inspecting Strings

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

`index_of()` returns `-1` when there's no match. It accepts an optional
second argument that sets where the search starts.

`last_index_of()` searches from the end instead, which is what you want when
the separator you care about is the last one:

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

Both methods take a second argument, and in both cases it limits where a
*match can start*. Together, they split a string at an index: for any `n`,
`index_of(str, n)` finds the first match at or after `n`, and
`last_index_of(str, n)` finds the last match at or before `n`.

There's also a set of methods that test character classes. Each one returns
true when **every** character in the string matches:

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

The full set is `is_alpha`, `is_alnum`, `is_number`, `is_lower`, `is_upper`
and `is_space`, along with `is_empty`.

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

`capitalize()` converts the first character to uppercase and the rest to
lowercase. `title()` does the same for every word.

When you compare text entered by users, use `case_fold()` instead of
`lower()`. Case folding is the Unicode operation designed for
case-insensitive matching, and it handles cases that simple lowercasing gets
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
echo '-=hi=-'.trim('-=')
```

```console
[pad]
[pad  ]
[  pad]
hi
hi
```

Called without an argument, all three methods remove whitespace: spaces,
tabs, newlines, carriage returns, vertical tabs and form feeds. If you pass
a string, they remove any of the characters in it, in any order, so
`'-=hi=-'.trim('-=')` gives `hi`.

```zuri
echo 'x'.lpad(5, '.')
echo 'x'.rpad(5, '.')
```

```console
....x
x....
```

`lpad` and `rpad` take a target width and an optional fill character, which
defaults to a space. If the string is already as wide as the target or
wider, it's returned unchanged.

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

`join` is a method of the **separator**, not of the list. `to_list()` splits
a string into single characters. `lines()` splits on newlines, and handles
both `\n` and `\r\n`.

## Replacing Text

```zuri
echo 'Hello, Zuri'.replace('Zuri', 'World')
```

```console
Hello, World
```

`replace()` replaces every occurrence. The pattern can be a regular
expression, and the replacement can refer to capture groups with `$1`, `$2`
and so on:

```zuri
echo 'John Smith'.replace('/(\w+) (\w+)/', '$2, $1')
```

```console
Smith, John
```

The optional third argument turns off regular expression handling, so a
pattern that would otherwise be treated as a regular expression is matched
literally, delimiters and all:

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

Zuri uses **PCRE2** for regular expressions, the same engine used by PHP, R
and many other tools. It's the full library, not a subset, so named groups,
backreferences, lookahead, lookbehind, atomic groups, possessive
quantifiers, conditionals and recursion all work as the PCRE2 documentation
describes.

This section explains how to write a pattern in Zuri, which methods accept
patterns, what each modifier does, and the few places where Zuri behaves
differently from what you might expect. It doesn't teach regular expression
syntax itself, but any PCRE or Perl reference applies as it is.

### Writing a Pattern

A pattern is an ordinary string. Its **first character must be a non-word
character, and the same character closes the pattern**. Any modifier letters
go after the closing delimiter:

```text
'/[a-z]+/'
'/[a-z]+/i'
'#\d{3}#'
'!https?://\S+!i'
```

A non-word character is anything other than a letter, a digit or an
underscore. A forward slash is the usual choice, and it's what every example
in this book uses, but you can pick a different delimiter. That's useful
when the pattern itself contains a lot of slashes:

```zuri
echo 'see https://example.com/docs now'.match('!https?://\S+!')
```

```console
{0: https://example.com/docs}
```

Because a pattern is a string, Zuri's own escape rules are applied to its
backslashes first. In practice, this rarely causes problems, because `\d`,
`\w`, `\s`, `\b` and `\S` aren't escape sequences in strings and are passed
through unchanged. The exceptions are the letters that *are* escape
sequences: `\t`, `\n`, `\r`, `\f`, `\v`, `\a`, `\b`, `\0`, `\x` and `\u`.
`\t` and `\n` mean the same thing in both strings and patterns, so they're
harmless. The one that will catch you out is `\b`, which means a word
boundary in a pattern but a backspace in a string:

```zuri
echo 'one two'.match('/\btwo\b/')
echo 'one two'.match('/\\btwo\\b/')
```

```console
false
{0: two}
```

Whenever a pattern needs `\b`, write it with a double backslash.

### Plain Strings Are Matched Literally

All of the methods in this section also accept a plain string, which they
treat as a **literal substring** rather than a pattern:

```zuri
echo 'a.b'.replace('.', 'X')
echo 'a.b'.replace('/./', 'X')
```

```console
aXb
XXX
```

The first call replaced the single period. The second call compiled `.` as a
pattern, where it means "any character", so it replaced all three
characters.

The only thing that decides this is whether the string looks like a
delimited pattern. If you're working with text from elsewhere that might
happen to look like one, pass `false` as the third argument to `replace()`
to turn off pattern handling:

```zuri
echo 'a/b/c'.replace('/b/', 'X')
echo 'a/b/c'.replace('/b/', 'X', false)
```

```console
a/X/c
aXc
```

With `false`, the delimiters are matched as ordinary characters.

### Methods That Accept Patterns

There are exactly five:

| Method | What it does with the pattern |
| --- | --- |
| `match(pattern)` | Returns the first match and its groups, or `false` |
| `matches(pattern)` | Returns every match, grouped by capture group |
| `replace(pattern, with, use_regex)` | Replaces every match |
| `replace_with(pattern, callback)` | Replaces every match with the result of a function call |
| `split(pattern)` | Splits the string at every match |

All the other methods, such as `contains()`, `index_of()`, `count()`,
`starts_with()`, `ends_with()` and `trim()`, only accept **literal**
strings. If you pass them something that looks like a regular expression,
they search for those exact characters:

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

All three calls searched for the six-character string `/[0-9]/`, which
doesn't appear in `a1b`. If you need a pattern in a case like this, use
`match()` instead.

### `match()` and `matches()`

`match()` finds the **first** match and returns it in a dictionary under the
key `0`. When nothing matches, it returns `false`, not `nil`:

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

Both `false` and `nil` are falsy, so `if s.match(p) { ... }` works as
expected. Comparing the result with `nil` doesn't, and that's the most
common regular expression mistake in Zuri code.

The dictionary also contains the pattern's capture groups, each under its
number, and named groups under their names as well. A group that doesn't
take part in the match is `nil`:

```zuri
echo '2024-01'.match('/(?<year>\d+)-(\d+)/')
echo 'ab'.match('/([a-z]+)(\d+)?/')
```

```console
{0: 2024-01, 1: 2024, 2: 01, year: 2024}
{0: ab, 1: ab, 2: nil}
```

`matches()` finds **every** match and groups the results by capture group.
Key `0` holds the full matches, key `1` holds the matches for the first
group, and so on. Each value is a list, and the lists line up with each
other:

```zuri
echo 'a1b2c3'.matches('/[0-9]/')
echo 'a1b2'.matches('/([a-z])([0-9])/')
```

```console
{0: [1, 2, 3]}
{0: [a1, b2], 1: [a, b], 2: [1, 2]}
```

In the second result, the first match is `a1`, with groups `a` and `1`, and
the second match is `b2`, with groups `b` and `2`. Read down each list, not
across them.

When nothing matches, `matches()` returns `{0: []}` instead of `false`.
Since the two methods behave differently here, check the result of
`matches()` by calling `is_empty()` on key zero:

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

In `replace()`, you refer to a capture group with `$1`, `$2` and so on:

```zuri
echo 'John Smith'.replace('/(\w+) (\w+)/', '$2, $1')
```

```console
Smith, John
```

**Don't write `${1}`.** The braced form is Zuri's own string interpolation,
and it's evaluated before `replace()` sees the string. The expression `2`
evaluates to the number two, so by the time the replacement string reaches
`replace()`, it's already `'2, 1'`:

```zuri
echo 'John Smith'.replace('/(\w+) (\w+)/', '${2}, ${1}')
```

```console
2, 1
```

Always use the plain `$1` form. This is one of the few places where Zuri's
interpolation syntax clashes with another syntax, and when it goes wrong,
there's no error to warn you.

### Named Groups

Named groups are supported in **patterns**, because PCRE2 supports them:

```zuri
echo 'John Smith'.matches('/(?<first>\w+) (?<last>\w+)/')
```

```console
{0: [John Smith], 1: [John], 2: [Smith]}
```

However, the results are keyed by **position**, not by name. A group's name
documents what it's for and lets a backreference such as `\k<first>` refer
to it, but it doesn't change the keys in the result. To find the group you
want, count the groups.

### Computing the Replacement

When the replacement depends on what was matched, `replace_with()` calls a
function for each match:

```zuri
echo 'a1b2'.replace_with('/[0-9]/', @(m) => '<' + m + '>')
```

```console
a<1>b<2>
```

The function receives the full match first, then one argument for each
capture group, then the position of the match, and finally the whole string
being searched. You only need to declare the arguments you use, because
extra arguments are ignored:

```zuri
echo 'aXbXc'.replace_with('/X/', @(match, offset) => '[${offset}]')
```

```console
a[1]b[3]c
```

### Splitting on a Pattern

`split()` accepts a pattern, which lets you split on "one or more of
something" instead of an exact separator:

```zuri
echo 'a1b22c'.split('/[0-9]+/')
echo 'a, b,c ,  d'.split('/\s*,\s*/')
```

```console
[a, b, c]
[a, b, c, d]
```

### Modifiers

Modifier letters go after the closing delimiter, in any order and
combination. For example, `/[a-z]+/mi` is both multi-line and
case-insensitive.

| Modifier | Effect |
| --- | --- |
| `i` | Case-insensitive matching. |
| `m` | Multi-line mode: `^` and `$` match at the start and end of every line, not just the start and end of the string. |
| `s` | Dot-all mode: `.` also matches newlines. |
| `x` | Extended mode: whitespace in the pattern is ignored unless escaped, and `#` starts a comment that runs to the end of the line. |
| `u` | Unicode mode: `\d`, `\w`, `\s` and similar classes match Unicode characters instead of only ASCII. |
| `U` | Ungreedy mode: quantifiers are lazy by default, and a `?` after one makes it greedy. |
| `A` | Anchored mode: a match is only accepted if it starts exactly where the search started. |
| `J` | Allows two capture groups in the same pattern to have the same name. |

Here's each of them in use:

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

Compare the last two results. The pattern matched when the text was at the
start of the string and failed when it wasn't, which is what `A` is for.
Without `A`, the second one would have matched at position one.

`u` deserves a closer look too. Without it, `\w` only matches ASCII, so `é`
isn't a word character. With it, Unicode properties apply, and `é` is a word
character. Matching is always done per character rather than per byte, so
positions are correct on text with multi-byte characters, with or without
`u`.

### Differences From PCRE2

**PCRE2's `D` (dollar end only) modifier has no effect in Zuri.** `$` always
matches before a trailing newline as well as at the very end of the string,
and you can't change that. The `D` letter is accepted rather than rejected,
so a pattern that uses it still compiles and runs. It just doesn't do
anything.

The same goes for any other modifier letter that Zuri doesn't recognize.
It's accepted and ignored instead of raising an error. That means a typo in
a modifier goes unnoticed, so keep it in mind if a pattern behaves as though
one of its flags isn't set.

All the other modifiers in the table are supported and behave as the PCRE2
documentation describes.

### Invalid Patterns

If PCRE2 can't compile a pattern, Zuri raises an error that includes the
pattern and PCRE2's own explanation:

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

Patterns are compiled once and then reused, so using the same pattern
repeatedly in a loop has no extra cost after the first time.

## Converting Strings

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

`to_number()` accepts an optional base. `ord()` takes a string of exactly
one character and returns its code point, and `chr()` converts a number back
into a character.

### `to_number()` Never Fails

This is the one conversion rule you should memorize. `to_number()` never
raises an error, and it never returns `NaN`. It reads the first number in
the string, and if the string doesn't contain a number at all, the result is
**zero**:

```zuri
echo '42'.to_number()
echo '3.5'.to_number()
echo '-5'.to_number()
echo 'eighty'.to_number()
echo ''.to_number()
echo '12abc'.to_number()
echo '  7  '.to_number()
echo '96.3 of 31'.to_number()
```

```console
42
3.5
-5
0
0
12
7
96.3
```

Look at the last three lines. Anything around the number is ignored, so
`'12abc'` gives `12`, the spaces in `' 7 '` don't matter, and only the first
number is read, however many follow it.

This can work for you or against you. `'3 apples'` giving `3` is usually
what you want, but `'2026-09-16'` giving `2026` usually isn't. A number
starts at a digit, or at a sign or decimal point immediately before a digit,
so `'-5'` is negative five, while `'a - 42'`, where the sign is separated
from the digits, is `42`.

Since there's no error to catch and no special value to check for, an empty
form field and a field containing `'0'` give you the same number, and so do
`'eighty'` and `'0'`. When the difference matters, check the text before you
convert it:

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

`is_number()` is stricter than its name suggests. It's only true for a
string made entirely of digits, so `'3.5'` and `'-5'` both fail. For
anything other than unsigned integers, a regular expression is the more
reliable check.

## Iterating Over a String

Strings are iterable, one character at a time, and you can use the same four
approaches that work for lists.

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

This is the approach to use by default. Each `ch` is a one-character string,
not a code point number. Use `ord()` if you need the number.

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

The key comes first and the value second. For a string, the key is the
position of the character.

### `iter`, When You Need Control Over the Position

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

`s[i]` indexes by character rather than by byte, so this works correctly on
text that isn't ASCII. Use `iter` when you need to skip ahead, go backward,
or look at `s[i + 1]` inside the loop.

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

As with the other types, the function receives the **value first and the
index second**. Note that this is the opposite of the order used by `for`.

### `each_line()` and `lines()`, for Line-Based Text

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

`each_line()` calls a function once for each line, and `lines()` gives you
all of the lines as a list, so you can loop over them, filter them or count
them. Both split on `\n` and `\r\n`, so they work with files written on any
platform.

## Comparing Strings

`==` compares two strings by value, which is nearly always what you want:

```zuri
echo 'abc' == 'abc'
echo 'abc' == 'ABC'
```

```console
true
false
```

The ordering operators are different. `<`, `<=`, `>` and `>=` only work with
numbers, and using one to compare two strings raises a `TypeError`. Use
`compare()` instead, which returns `-1`, `0` or `1`:

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

A three-way result like this is what a sort comparison function needs.
`compare()` compares strings by code point, so the result is stable and
doesn't depend on the locale.

## Strings Are Immutable

Every method in this section returns a new string. None of them change the
string in place:

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

It's important to keep this in mind, because it leads to the most common
string mistake of all:

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

The first `trim()` call created a trimmed string and then threw it away.
Assign the result to a variable, or chain another call onto it.

Because strings are immutable, you never need to copy one before passing it
somewhere. Nothing you pass a string to can change the string you still
have.

## Example: Parsing a Configuration File

Here's a small parser that uses most of what this section has covered. It
takes `key = value` configuration text and turns it into a dictionary,
skipping blank lines and comments and accepting any spacing around the `=`.

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

A few things in this code are worth pointing out. `lines()` handles both
`\n` and `\r\n`, so the same code can read a file written on Windows. The
result of `index_of()` is compared with `-1` explicitly rather than tested
for truthiness, because `0` is falsy and `0` is a valid position. And every
value starts out as a string, because the input is text. `to_number()` is
how you turn it into a number.
