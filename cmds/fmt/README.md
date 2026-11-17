# fmt

Formats the Zuri files in a path, or in the current directory, in the
project's code style. That means two spaces per indentation level, one
statement per line, braces around every block, consistent spacing between
tokens, and long lines broken at the points where their expressions are
least tightly bound.

## Usage

```sh
zuri fmt [path]
```

`path` is either a single `.zu` file, which is formatted in place, or a
directory, in which case every `.zu` file in it and in its subdirectories
is formatted. If you don't give a path, the current directory is
formatted.

```sh
# format one file
zuri fmt libs/date.zu

# format a whole directory tree
zuri fmt libs

# format the current directory
zuri fmt

# report what would change without writing anything (exits with 1 if anything would)
zuri fmt --dry-run libs
```

Use `--dry-run` (or `-n`) in a commit hook or a CI job. It lists the files
that aren't formatted yet, and exits with a non-zero status if there are
any.

## How it formats code

The formatter reads the program from its parse tree, the same way the
compiler does, and lays out every statement, declaration and expression in
the project's style. Because it knows what each piece of code is, it never
mistakes a sign for an operator, a type's `:` for a dictionary's, or a
subscript for a list.

Long lines are broken following the rules of the Dart formatter, as
closely as Zuri's grammar allows. The layout depends only on the code. It
doesn't matter where the source broke a line inside an expression, so the
same code is always formatted the same way.

- **Indentation.** Each level of nesting is indented two spaces. The body
  of a block, and the items of a list that's split across lines, are
  indented one level. An expression that continues onto another line is
  indented two levels past the line it continues, so a long `if`
  condition never lines up with the body below it.
- **Blocks.** Each statement inside a block goes on its own line, and a
  `;` between two statements becomes a line break. An empty block is
  written as `{}`. `else`, a `while` after a `do`, and an `as` after a
  `catch` go on the same line as the `}` before them.
- **Braces.** `if x y = 1` gets braces around its body, and so does the
  body of an `else`, a `while`, a `for` and a `do`. A body that declares a
  name, such as `if x var y = 1`, is left as it is, because braces would
  give the name a scope of its own. A `when` or `default` arm keeps an
  unbraced body on its line as long as the line is at most 60 characters
  wide, and gets braces otherwise.
- **Blank lines around declarations.** A function, method or class that
  spans several lines gets a blank line before and after it, placed above
  any comments that introduce it. A blank line that the source left
  between two statements is kept, but several are reduced to one.
- **Spacing.** Spaces depend on what each token is: around binary
  operators, after commas, none around `.` or inside a call's
  parentheses, none around `|` in a type, and spaces inside the braces of
  `{ a: 1 }`. An empty `iter` clause keeps the space that shows it's
  there: `iter ; i < n; i++`.
- **Lists of items.** Arguments, parameters, list and dictionary literals,
  and an import's names are kept on one line if they fit. If they don't,
  each item goes on a line of its own, with a trailing comma after the
  last one and the closing bracket back at the start of the line where the
  list opened. Once a list is split across lines, a blank line that the
  source left between two items is kept.
- **Nested collections.** A list or dictionary that contains another list
  or dictionary is always split across lines, at any depth and through any
  call, even if it would fit on one line, so a tree of data is laid out
  as a tree. For example, `[[3, 4], [5, 6]]` puts each row on its own
  line, and so does `[Window('Main', { size: Size(720, 420) })]`. An
  empty `[]` or `{}` inside doesn't count.
- **A single argument split on its own.** When a call's arguments include
  a single function with a body, or otherwise a single list or
  dictionary, and at most one argument comes after it, only that argument
  is split across lines, while the others stay on the call's line, as in
  `each(items, @{` or `Text(label, {`. If the arguments before it don't
  fit on that line, every argument is split onto its own line instead. A
  call is never the argument that gets split this way, so a call inside a
  call that doesn't fit moves to a line of its own under `f(`, instead of
  being written as `f(g(`.
- **Operator chains.** A chain of operators is broken after every
  operator of the lowest precedence in it (`or` before `and`, before `==`,
  before `+`), and the higher-precedence parts stay on their lines as long
  as they fit. The operator ends the line, which also means a line never
  starts with `-`, which would begin a new statement.
- **Conditionals.** A conditional expression is broken before its `?` and
  its `:`, with each branch on a line of its own. A conditional in the
  last branch continues the same chain, so a series of them reads as a
  list of cases.
- **Method chains.** A method chain is broken before each call, with one
  call per line, and any property reads at the start of the chain stay
  next to the value they start from. Before that happens, two simpler
  layouts are tried. If the calls before the last one take no arguments,
  or the last call takes a function or a collection that can be split on
  its own, only that call's arguments are split. If the chain starts from
  a call or a collection, that is split instead, as long as the rest of
  the chain fits after its closing bracket. A call on a capitalized name,
  such as `Color.of(x)`, is treated as part of the value it's called on.
- **Dictionary values.** If a dictionary entry doesn't fit, and its value
  is made of operators, the value moves to a line of its own after the
  `:`. Nothing else is broken after a `:`, and nothing is broken after an
  `=` or a `=>`, because the language doesn't allow it.
- **Lists of numbers.** A list of numbers, which can include `nil`, is
  filled with as many items per line as fit, instead of one per line.
- **Commented rows.** If a list or dictionary has a `#` comment between
  its items, each row is laid out like a list of its own, so a table stays
  a table even when one of its lines has a comment.
- **Subscripts.** Commas in a subscript are never added or removed.
  `s[1]`, `s[1,]`, `s[,3]` and `s[1,3]` mean four different things, and
  they're written as `s[1]`, `s[1,]`, `s[,3]` and `s[1, 3]`.
- **Comments.** Comments stay where they were written. A comment at the
  end of a line of code stays at the end of that line, a comment on its
  own line stays on its own line, and a doc block moves with the code it
  documents. A `#` comment inside a list or a call splits it across
  lines, because the comment ends its line.
- **Strings, numbers and imports.** Strings and numbers are written
  exactly as they are in the source, and so are the dots in import paths:
  `.. ..shared` and `....shared` both stay as they are.
- **Rewrites.** Inside a dictionary, `name: name` is rewritten as `name`,
  the shorthand the project prefers. Two other things that the language
  allows but the project never uses are removed: parentheses around an
  `iter` clause, and `def` before a method name.

## Safety

The formatter never changes what a program does. Before it writes a file,
it parses its own output and checks that the tree is the same one it
started with, and that every comment is still there, in the same order.
The only differences it accepts are its own rewrites: the dictionary
shorthand, and the braces it adds around a body. If anything else is
different, the file is left unchanged and reported as skipped. This makes
it safe to run on a whole directory tree.

A file that doesn't parse is skipped too, and the line and column of the
first syntax error are reported, because there's no tree to lay out.

## Source files

- `source.zu`: the tokens of the source, for the things the parse tree
  doesn't keep: the blank lines the author left, every comment, and the
  operators and commas between two nodes.
- `doc.zu`: the layout document, made of text and the places where it
  can break.
- `layout.zu`: converts the parse tree into a layout document, in source
  order, so that every comment is placed where it was written.
- `printer.zu`: converts the layout document into lines. Each group is
  kept on one line if it fits, and when several layouts are possible for a
  piece of code, the first one that fits is used.
- `verify.zu`: compares two trees and their comments.
- `engine.zu`: `format(source)` and the checks around it.
- `index.zu`: the command itself, which `zuri fmt` runs.

The formatter uses Zuri's own parser, through `zuri.parse` and
`zuri.tokenize`, so there's no second parser to keep in sync with the
language.

## Tests

Each source file has a test file in `tests/`, written
with the `test` module. To run them all:

```sh
zuri test cmds/fmt/tests
```

Or, from this directory, just run `zuri test`.
