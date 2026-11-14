# fmt

Formats all Zuri files in the given path, or the current directory, in
the project's own style: two spaces to a level, one statement to a line,
explicit blocks, the spacing a reader expects between tokens, and long
lines broken where their expressions hold together least.

## Running it

```sh
zuri fmt [path]
```

`path` is either a single `.zu` file, which is formatted in place, or a
directory, in which case every `.zu` file inside it is formatted,
subdirectories included. If no path is given, the current directory is
formatted.

```sh
# format one file
zuri fmt libs/date.zu

# format a whole tree
zuri fmt libs

# format the current directory
zuri fmt

# report what would change without writing anything (exit 1 if any would)
zuri fmt --dry-run libs
```

`--dry-run` (or `-n`) is what a commit hook or CI wants: it names the
files that are not already formatted and exits non-zero if there are any.

## What it does

It reads the program the way the compiler does, from its parse tree,
and lays every statement, declaration and expression out in the
project's style. It knows what each piece of code is, so a sign is never
mistaken for an operator, a type's `:` for a dictionary's, or a
subscript for a list.

Where a long line breaks follows the rules of the Dart formatter, as
closely as Zuri's grammar allows. The layout depends on the code alone:
where the source broke a line inside an expression makes no difference,
so the same code always comes out the same way.

- **Indents** two spaces per level of nesting. A block's body, and the
  items of a list that opens out, sit one level in. An expression
  continued onto another line sits two levels past the line it
  continues, so a broken `if` condition never lines up with the body
  under it.
- **Opens every block.** The statements inside a block each get their
  own line, and `;` between two statements becomes a line break. An
  empty block closes up to `{}`. `else`, `while` after a `do`, and `as`
  after a `catch` follow the `}` before them on its line.
- **Braces every body.** `if x y = 1` becomes a block, and so does the
  body of an `else`, a `while`, a `for` and a `do`. A body that declares
  a name, `if x var y = 1`, is left as it is, since braces would give the
  name a scope of its own. A `when` or `default` arm keeps an unbraced
  body on its line while the line is at most 60 characters wide, and
  braces it otherwise.
- **Sets declarations apart.** A function, method or class that spans
  several lines has a blank line on either side of it, ahead of the
  comments that lead into it. A blank line the source left between two
  statements is kept, one at most.
- **Spaces** by what each token is: around binary operators, after
  commas, none around `.` or inside a call's parentheses, none around `|`
  in a type, `{ a: 1 }` with spaces inside its braces. An empty `iter`
  clause keeps the space that shows it is there: `iter ; i < n; i++`.
- **Keeps a list on one line, or opens it one item to a line.** This
  holds for arguments, parameters, list and dictionary literals and an
  import's names. One that does not fit opens out with each item on a
  line of its own and a trailing comma after the last, its closer back
  on the line it opened on. A blank line the source left between two
  items is kept once the list opens out.
- **Opens a list or dictionary that holds another**, at any depth and
  through any call, even when it would fit on one line, so a tree of
  data reads as a tree: `[[3, 4], [5, 6]]` takes a row to a line, and so
  does `[Window('Main', { size: Size(720, 420) })]`. An empty `[]` or
  `{}` inside does not count.
- **Lets one argument open out on its own.** When a call's arguments
  hold a single function with a body, or failing that a single list or
  dictionary, and at most one argument follows it, that argument opens
  out while the rest stay on the call's line: `each(items, @{`,
  `Text(label, {`. When the arguments before it do not fit on that line,
  every argument opens out instead. A call is never the argument that
  opens out, so a call inside a call that does not fit moves to a line
  of its own under `f(`, rather than opening as `f(g(`.
- **Breaks an operator chain after every operator** of the loosest
  precedence in it, `or` before `and` before `==` before `+`, leaving
  the tighter ones on their lines while they fit. The operator ends its
  line, which also keeps a line from opening with `-`, the start of a
  statement of its own.
- **Breaks a conditional before its `?` and `:`**, each branch on a
  line of its own. A conditional in the last branch joins the chain, so
  a run of them reads as a list of cases.
- **Breaks a method chain before each call**, one call to a line, the
  property reads it opens with kept beside the value it starts from.
  Before that, a chain whose calls before the last take no arguments,
  or whose last call takes a function or collection that opens out on
  its own, opens only that call's arguments; and a chain that starts
  from a call or a collection opens that instead when the rest of the
  chain fits after its closing bracket. A call on a capitalized name,
  `Color.of(x)`, counts as part of the value it is made on.
- **Moves a value made of operators onto a line of its own** after the
  `:` of a dictionary entry that does not fit. Nothing else breaks
  after a `:`, and nothing breaks after an `=` or a `=>`, which the
  language does not allow.
- **Fills a list of numbers**, with or without `nil` among them, as
  many to a line as fit, rather than one to a line.
- **Keeps the rows of a list or dictionary holding a `#` comment**
  between its items, each row laid out like a list of its own, so a
  table stays a table when one of its lines carries a comment.
- **Never adds or drops a subscript's commas.** `s[1]`, `s[1,]`, `s[,3]`
  and `s[1,3]` are four different reads, written `s[1]`, `s[1,]`,
  `s[,3]` and `s[1, 3]`.
- **Keeps comments where they were written**: one that trails code stays
  at the end of its line, one on a line of its own keeps that line, and
  a doc block moves along with the code it documents. A `#` comment
  inside a list or a call opens it out, since the comment ends its line.
- **Writes strings and numbers exactly as the source does**, and import
  paths with their dots as written: `.. ..shared` and `....shared` both
  stay.
- **Rewrites `name: name` to `name`** inside a dictionary, the shorthand
  the project prefers, and drops what the language allows but the
  project never writes: the parentheses an `iter` may be written with,
  and `def` before a method name.

## What it will not do

It never changes what a program does. Before writing a file, it parses
its own result and checks that the tree is the one it started from and
that every comment is still there, in order. The only differences it
accepts are its own rewrites: the dictionary shorthand, and a body it
braced. If anything else differs, that file is left as it was and
reported as skipped. This is why it can be run across a whole tree
without fear.

A file that does not parse is skipped too, with the line and column
of the first syntax error: there is no tree to lay out.

## How it is built

- `source.zu`: the tokens of the source, for what the tree does not
  keep: the blank lines the author left, every comment, and the
  operators and commas between two nodes.
- `doc.zu`: the layout document, text and the places it may break.
- `layout.zu`: the parse tree turned into a layout document, in source
  order so every comment is placed where it was written.
- `printer.zu`: the document turned into lines, each group kept on one
  line when it fits, and of the layouts offered for one piece of code,
  the first that fits taken.
- `verify.zu`: the comparison of two trees and their comments.
- `engine.zu`: `format(source)` and the checks around it.
- `index.zu`: the command itself, the entry point `zuri fmt` runs.

The parser is Zuri's own (`zuri.parse` and `zuri.tokenize`), not a
second one to keep in step with the language.

## Tests

A test file sits beside each source file under `tests/`, written with
the `test` module. Run the lot with:

```sh
zuri test cmds/fmt/tests
```

or, from this directory, just `zuri test`.
