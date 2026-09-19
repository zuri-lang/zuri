# format

Lays out Zuri source in the project's own style: two spaces to a level,
one statement to a line, the spacing a reader expects between tokens, and
long lines broken across the brackets and operators that carry them.

## Running it

```sh
zuri apps/format/main.zu <path>
```

`path` is a single `.zu` file, formatted in place, or a directory, whose
`.zu` files are all formatted, its subdirectories included.

```sh
# format one file
zuri apps/format/main.zu libs/date.zu

# format a whole tree
zuri apps/format/main.zu libs

# report what would change without writing anything (exit 1 if any would)
zuri apps/format/main.zu --dry-run libs
```

`--dry-run` (or `-n`) is what a commit hook or CI wants: it names the
files that are not already formatted and exits non-zero if there are any.

## What it does

- **Indents** two spaces per level of nesting, driven by the blocks.
- **Spaces** tokens by the rules the language reads by: around binary
  operators, after commas, none around `.` or inside a call's
  parentheses, none around `|` in a type, and so on.
- **Breaks long lines** at their `and`/`or`, or across the brackets of a
  call or a literal, filling each line to the width. A broken collection
  keeps a trailing comma so a later edit touches one line, not two.
- **Leaves a subscript alone.** `s[1]`, `s[1,]`, `s[,3]` and `s[1,3]` are
  four different reads, so the commas inside a `[` that indexes
  something are never added, dropped, or filled across lines, and they
  close up: `s[2,5]`, not `s[2, 5]`. A `[` that opens a list is not a
  subscript and keeps the spacing and the trailing-comma rule above.
- **Keeps blocks open**: the statements inside a block each get their own
  line, always.
- **Rewrites `name: name` to `name`** inside a dictionary, the shorthand
  the project prefers.

## What it will not do

It never changes what a program does. Before writing a file, it checks
the result holds exactly the same tokens as the input; if a bug ever
produced anything else, that file is left as it was and reported as
skipped. This is why it can be run across a whole tree without fear.

## How it is built

- `atoms.zu` — the token stream, with each string (interpolation and all)
  reassembled whole from the source it came from.
- `tree.zu` — the tokens grouped by their brackets, each `{` decided to be
  a block or a dictionary.
- `render.zu` — one-line rendering and the spacing rules.
- `layout.zu` — the line breaking: blocks, closures, comments, wrapping.
- `index.zu` — `format(source)` and the safety check.
- `main.zu` — the command itself.

The parser is Zuri's own (`zuri.tokenize`), not a second one to keep in
step with the language.

## Tests

A test file sits beside each source file under `tests/`, written with
the `test` module. Run the lot with:

```sh
zuri apps/format/tests
```
