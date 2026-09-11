# Appendix E: Built-in Type Methods

Every method on every built-in type, with its signature, what it returns,
and its edge cases. This is the reference; the chapters in
[Text, Numbers and Collections](ch04-00-collections.md) are the
introduction.

Methods are called with a dot, on the value itself:

```zuri
echo 'zuri'.upper()
echo (255).hex()
echo [3, 1, 2].sort()
```

| Type | Methods | Page |
| --- | --- | --- |
| `string` | 39 | [Strings](appendix-05-01-string.md) |
| `number` | 43 | [Numbers](appendix-05-02-number.md) |
| `bigint` | 26 | [Bigints](appendix-05-03-bigint.md) |
| `bool` | 1 | [Booleans](appendix-05-04-bool.md) |
| `list` | 39 | [Lists](appendix-05-05-list.md) |
| `dict` | 21 | [Dictionaries](appendix-05-06-dict.md) |
| `range` | 8 | [Ranges](appendix-05-07-range.md) |
| `bytes` | 24 | [Bytes](appendix-05-08-bytes.md) |
| `file` | 27 | [Files](appendix-05-09-file.md) |
| `function` | 5 | [Functions](appendix-05-10-function.md) |

## What Is Not Listed Here

**`@key` and `@value`** exist on every iterable built-in type, which is
what makes `for ... in` work on them. They are documented as a protocol in
[Appendix C](appendix-03-decorators.md) rather than repeated per type.

**`to_string()`** exists on every value, `nil` included.

**Class instances** carry whatever their class declares, plus
`to_string()`. See [Classes and Objects](ch06-00-classes.md).

**Module members** are not methods. See
[Appendix F](appendix-06-stdlib-index.md).

## Conventions in These Pages

A parameter written `name: ?type` is optional. A parameter written
`...name` is variadic. Anything marked **Default** states what an omitted
argument falls back to.

A method that **mutates** says so. Where a type has both, the distinction
matters: `list.sort()` mutates and returns the list, while
`list.reverse()` returns a new list and leaves the original alone.
