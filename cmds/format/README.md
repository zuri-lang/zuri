# format

Lays out Zuri source in the project's own style: two spaces to a level,
one statement to a line, the spacing a reader expects between tokens, and
long lines broken across the brackets and operators that carry them.

## Running it

```sh
zuri format <path>
```

`path` is a single `.zu` file, formatted in place, or a directory, whose
`.zu` files are all formatted, its subdirectories included. Left off, it
formats the current directory.

```sh
# format one file
zuri format libs/date.zu

# format a whole tree
zuri format libs

# format the current directory
zuri format

# report what would change without writing anything (exit 1 if any would)
zuri format --dry-run libs
```

`--dry-run` (or `-n`) is what a commit hook or CI wants: it names the
files that are not already formatted and exits non-zero if there are any.

## What it does

Its job is indentation, spacing, and the lines that are too long for the
width. Where a line break already is, is not its business.

- **Keeps the lines you wrote.** A list opened out one item to a line, a
  concatenation carried across several, a call whose arguments were
  spread on purpose: each of those stays as it is and is re-indented,
  never packed back together. A collection written on one line stays on
  one line. That holds for the blank line a `class` or a `def` opens on,
  and for an empty block written across two lines.
- **Indents** two spaces per level of nesting, driven by the blocks.
  Everything after a statement's first line sits one level under it,
  however many breaks it took to lay the statement out.
- **Spaces** tokens by the rules the language reads by: around binary
  operators, after commas, none around `.` or inside a call's
  parentheses, none around `|` in a type, and so on. An empty `iter`
  clause keeps the space that shows it is there: `iter ; i < n; i++`.
- **Breaks a long line** at its loosest operator, or across the brackets
  of a call or a literal, filling each line to the width. It breaks only
  where the break resolves the overflow; a line with nowhere useful to
  break is left as it was written. A collection the formatter breaks
  itself keeps a trailing comma so a later edit touches one line, not
  two.
- **Never adds or drops a subscript's commas.** `s[1]`, `s[1,]`, `s[,3]`
  and `s[1,3]` are four different reads, so a `[` that indexes something
  is never filled across lines and its commas are left as they are. They
  are spaced like any other comma, `s[2, 5]`, except the one opening a
  slice from the start, which belongs to the bound it precedes: `s[,3]`.
- **Keeps blocks open**: the statements inside a block each get their own
  line, always. An empty one written as `{}` stays `{}`.
- **Leaves an unbraced body where it was.** `if x y = 1` is one line and
  stays one line; a body written on the next line keeps the indent that
  is the only thing marking it as a body. An `else` joins the line above
  it only when that line closed a block, because `if c x else y` on one
  line is not a statement Zuri reads.
- **Rewrites `name: name` to `name`** inside a dictionary, the shorthand
  the project prefers.

## What it will not do

It never changes what a program does. Before writing a file, it checks
the result twice: that it holds exactly the same tokens as the input,
and that the parser still reads it. If either check fails, that file is
left as it was and reported as skipped. This is why it can be run across
a whole tree without fear.

The second check earns its place. A newline is what ends a statement in
Zuri, so a line break moved to the wrong place turns a program into one
the parser refuses with every token still in it, and no comparison of
tokens can see that.

## How it is built

- `atoms.zu` — the token stream, with each string (interpolation and all)
  reassembled whole from the source it came from.
- `tree.zu` — the tokens grouped by their brackets, each `{` decided to be
  a block or a dictionary.
- `render.zu` — one-line rendering and the spacing rules.
- `layout.zu` — the lines: the breaks the source had, blocks, closures,
  comments, and the wrapping of what is left too long.
- `engine.zu` — `format(source)` and the safety check.
- `index.zu` — the command itself, the entry point `zuri format` runs.

The parser is Zuri's own (`zuri.tokenize`), not a second one to keep in
step with the language.

## Tests

A test file sits beside each source file under `tests/`, written with
the `test` module. Run the lot with:

```sh
zuri test cmds/format/tests
```

or, from this directory, just `zuri test`.
