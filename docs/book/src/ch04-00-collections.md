# Text, Numbers and Collections

Chapter 3 introduced the built-in types and showed what the operators do to
them. This chapter is the working manual for the five you will reach for
every day: strings, numbers, lists, dictionaries and ranges.

One idea shapes all five. **Zuri puts behaviour on the value itself.**
There is no `len(x)` and no `str.upper(x)`; there is `x.length()` and
`x.upper()`. That holds for numbers too, which carry the whole of the
mathematics library as methods:

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

Because the methods live on the value, they chain, and a chain reads left
to right in the order the work happens:

```zuri
var line = '  Ada, Grace , Alan  '

echo line.trim().split(',').map(@(name) => name.trim()).length()
```

```console
3
```

Each section here covers the methods worth knowing by name, the ones with a
behaviour you would not guess, and the mistakes that come up most often.
[Appendix E](appendix-05-type-methods.md) is the complete list, generated
from the same documentation the runtime ships.
