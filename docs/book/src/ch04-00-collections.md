# Text, Numbers and Collections

Chapter 3 introduced the built-in types and showed how the operators work
with them. This chapter goes into detail on the five types you'll use every
day: strings, numbers, lists, dictionaries and ranges.

All five have one thing in common. **In Zuri, behavior belongs to the value
itself.** Instead of `len(x)` or `str.upper(x)`, you write `x.length()` and
`x.upper()`. This is true for numbers as well, and the math functions are
all available as methods:

```zuri
echo 'zuri'.upper()
echo [3, 1, 2].sort()
echo 16.sqrt()
echo 255.hex()
```

```console
ZURI
[1, 2, 3]
4
ff
```

Because the methods are called on the value, you can chain them, and a chain
reads from left to right in the order the work happens:

```zuri
var line = '  Ada, Grace , Alan  '

echo line.trim().split(',').map(@(name) => name.trim()).length()
```

```console
3
```

Each section in this chapter covers the methods you should know by name, the
ones whose behavior might surprise you, and the most common mistakes.
[Appendix E](appendix-05-type-methods.md) has the complete list, generated
from the same documentation that ships with the runtime.
