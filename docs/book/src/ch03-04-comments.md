# Comments and Doc Blocks

## Line Comments

A `#` starts a comment that runs to the end of the line:

```zuri
# Rates are quoted per thousand, not per unit.
var rate = 0.0125 # 1.25%
```

## Block Comments

`/* ... */` can span as many lines as you like, and block comments **nest**:

```zuri
/* This whole section is off.
   /* Including this inner comment. */
   Still off. */
```

Nesting matters for two reasons. You can comment out a section of code that
already contains a block comment, and it works as you'd expect. But it also
means that an unmatched `/*` inside a comment turns the rest of the file
into a comment.

## Doc Blocks

A block comment that starts with `/**` is a **doc block**. The parser keeps
doc blocks in the syntax tree instead of throwing them away, which is how
the `zuri` module can read a file's documentation without needing a separate
parser. Put a doc block directly above the thing it documents:

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

The standard library uses these tags:

| Tag | Meaning |
| --- | --- |
| `@param {type} name: description` | Describes one parameter |
| `@returns type` | Describes the return value |
| `@throws ErrorClass` | Names an error the function can raise |
| `@note` | Something the caller needs to know |
| `@default` | The default value of a parameter |

Two habits from the standard library are worth copying. Document the default
value of every optional parameter, and describe what happens in edge cases,
such as an empty input, a length of zero or a value that's out of range. If
a caller would otherwise have to find something out by trial and error, put
it in the doc block.

Doc blocks aren't only for people reading the source. Because the parser
keeps them, programs can read them too. `zuri.parse()` returns each doc
block as a `DocBlock` node placed just before the declaration it documents,
which is all you need to build a simple documentation generator in a few
dozen lines. [Chapter 21](ch21-00-metaprogramming.md) shows how.

## Writing Good Comments

A good comment explains **why** the code does something. The code itself
already shows **what** it does:

```zuri,ignore
# Bad: restates the code.
# Add one to the counter.
counter++

# Good: explains a decision the code cannot.
# Servers count from one, and the wire protocol has no zero frame.
counter++
```
