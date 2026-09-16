# Comments and Doc Blocks

## Line Comments

`#` runs to the end of the line:

```zuri
# Rates are quoted per thousand, not per unit.
var rate = 0.0125  # 1.25%
```

## Block Comments

`/* ... */` spans as many lines as you like, and **nests**:

```zuri
/* This whole section is off.
   /* Including this inner comment. */
   Still off. */
```

Nesting is worth knowing about, because it means commenting out a region
that already contains a block comment works the way you expect, and it also
means an unbalanced `/*` inside a comment swallows the rest of your file.

## Doc Blocks

A block comment that opens with `/**` is a **doc block**. The parser keeps
doc blocks in the syntax tree rather than discarding them, which is what
lets the `zuri` module read a file's documentation without a separate
parser. Put one directly above the thing it documents:

```zuri
/**
 * Converts a duration in seconds to a human-readable string.
 *
 * Rounds to the nearest whole second. Durations below one second
 * render as `'0s'`.
 *
 * @param number seconds
 * @returns string
 */
def humanize(seconds) {
  # ...
}
```

The tag vocabulary used across the standard library is:

| Tag | Meaning |
| --- | --- |
| `@param {type} name: description` | one parameter |
| `@returns type` | what the function gives back |
| `@throws ErrorClass` | an error it can raise |
| `@note` | something the caller must know |
| `@default` | the default a parameter falls back to |

Two conventions from the standard library are worth copying. State the
default of every optional parameter, and state what happens at the edges:
an empty input, a zero length, a value out of range. Anything a caller
would otherwise have to discover by experiment belongs in the doc block.

Doc blocks are not only for readers. Because the parser keeps them, a
program can read them: `zuri.parse()` returns each one as a `DocBlock` node
sitting immediately before the declaration it documents, which is enough to
build a documentation generator in a few dozen lines.
[Chapter 18](ch18-00-metaprogramming.md) shows how.

## Commenting Style

A comment earns its place by explaining **why**, not **what**. The code
already says what it does:

```zuri,ignore
# Bad: restates the code.
# Add one to the counter.
counter++

# Good: explains a decision the code cannot.
# Servers count from one, and the wire protocol has no zero frame.
counter++
```
