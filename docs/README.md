# Zuri Documentation

Everything written about the language lives here, as two books and the
Zuri programs that generate and check them.

| | |
| --- | --- |
| [`book/`](book) | **The Zuri Programming Language**, the narrative text |
| [`reference/`](reference) | **The Zuri Standard Library**: one `book.toml`, generated from `libs/` |
| [`site/`](site) | the landing page the two books sit under |
| [`tools/`](tools) | the generators, the audit, the verifier, the link checker |

## The Book

[**The Zuri Programming Language**](book/src/SUMMARY.md) is the main text:
twenty-two chapters from installation to a full-stack web application,
plus eight appendices covering keywords, operators, decorated methods,
every built-in function, every method on every built-in type, the
standard library index, and the error hierarchy.

It is written as mdBook source under [`book/`](book), so it reads fine as
plain markdown on GitHub and renders to a searchable site with
`cargo run-docs`.

Start at [the introduction](book/src/ch00-00-introduction.md), or jump to
what you need:

| If you want to | Read |
| --- | --- |
| install it and run something | [Getting Started](book/src/ch01-00-getting-started.md) |
| see a whole program before the theory | [Programming a Bookmark Keeper](book/src/ch02-00-bookmark-keeper.md) |
| look up syntax | [Common Programming Concepts](book/src/ch03-00-common-concepts.md) |
| look up a method | [Appendix E](book/src/appendix-05-type-methods.md) |
| find a module | [Appendix F](book/src/appendix-06-stdlib-index.md) |
| understand the JIT | [Performance and the JIT](book/src/ch22-00-performance.md) |
| port habits from another language | [Appendix H](book/src/appendix-08-coming-from.md) |

## The Standard Library Reference

**The Zuri Standard Library** is the other book: every module Zuri
ships with, every public name in each one, and what each takes and
returns. Read it with `cargo run-docs -- reference`.

It is **generated**, not written, and **none of it is kept in the
repository**. `docs/reference` holds a `book.toml` and nothing else:
the pages are written to `target/reference/src` and rendered into
`target/reference/html`, both build output, both thrown away by
`cargo clean-docs`. A page and the doc block it came from therefore
cannot disagree, because there is no page until one is generated.

Every page comes from a doc block in [`libs/`](../libs), read with the
`zuri` module's own parser.

`cargo build-docs` and `cargo run-docs -- reference` both generate it
before rendering, so there is usually nothing to do by hand. To write
the pages without rendering anything:

```console
$ cargo docs generate reference
generating reference from libs/
34 modules, 148 pages, 2952 documented names
```

Or run the generator directly:

```console
$ zuri docs/tools/reference/generate.zu
```

Editing a page under `reference/src` is pointless; the next run
overwrites it. Edit the library's doc block instead.

A plain `cargo build` renders whichever books already have pages. On a
fresh clone that is the book alone, and it says so:

```console
warning: the reference has no pages yet; run `cargo build-docs` to generate them
```

Three rules decide what appears:

- A name beginning with an underscore is private. The compiler refuses
  to import one across a module boundary, so none reach the reference.
  The same goes for a whole file or directory whose name starts with
  one.
- A member tagged `@internal` is left out, even though it is publicly
  named.
- Every other module gets its own page, nested under its package. The
  first unattached doc block carrying `@module` is that page's own
  description.

The one hand-written part is
[`tools/reference/catalogue.zu`](tools/reference/catalogue.zu), which
decides which modules are grouped together and in what order, because
no doc block can say that `hash` belongs beside `crypto` rather than
beside `math`. A module added to `libs/` and not catalogued there stops
the build rather than quietly vanishing from the contents.

## Reading It Locally

```console
$ cargo run-docs
```

That builds the book and serves it at <http://localhost:3000> with live
reload, so an edit to a chapter refreshes the page. Name the other book
to serve that instead:

```console
$ cargo run-docs -- reference
```

Pass a port if 3000 is taken:

```console
$ cargo run-docs -- --port 4000
```

To render the HTML without serving it:

```console
$ cargo build-docs
```

Output lands in `target/book` and `target/reference/html`, and
`build-docs` then assembles the whole website under `target/site`:

```text
target/site/index.html     the landing page, from docs/site
target/site/book/          the book
target/site/reference/     the standard library reference
```

That directory is the site exactly as it is served, so it can be opened
locally before it is published. The landing page is ordinary HTML and
CSS in [`site/`](site), edited like any other file here; to lay the site
out again after changing it, without re-rendering either book:

```console
$ cargo docs site
```

Both commands need [mdBook](https://rust-lang.github.io/mdBook). If it is
not installed they offer to install it for you, and nothing is installed
without your say-so.

A plain `cargo build` also rebuilds the book, but only when mdBook is
already installed and only when something under `docs/book/src` actually
changed. Without mdBook you get one line of warning and the build carries
on, because the book is markdown that reads perfectly well unrendered.

To throw the rendered output away:

```console
$ cargo clean-docs
```

## Writing for the Book

Chapters are plain markdown in [`book/src`](book/src), listed in
[`SUMMARY.md`](book/src/SUMMARY.md). A file that is not in `SUMMARY.md` is
not in the book.

Two rules the existing text follows:

**Every example is run before it is written down.** The output in a
`console` block is what the program actually printed, not what it ought to
print. When you change behaviour, re-run the examples that cover it.

A block tagged ```` ```zuri,ignore ```` is exempt, and there are only four
reasons to tag one:

- it deliberately shows an error;
- it is one file of a multi-file program;
- it blocks forever — a server, a prompt;
- it needs something that is not there and cannot be: a remote host, an
  uploaded file, a licensed font. A block that only needs a *local* file
  does not qualify — create the file in an earlier block on the same page,
  as the Imagine chapter does with `photo.jpg`;
- it is a **catalogue**: several alternative calls listed together to show
  a family of methods, which is not a program and could not run as one.
  The reference chapters are full of these, and each is introduced by prose
  saying so.

Everything else must run. A tag is a statement that the example cannot be
checked, not a way to avoid checking it.

`zuri docs/tools/verify.zu` reports the count it ran. When that number
falls after a change, a block was tagged rather than fixed.

That rule is enforced, not just stated. This runs every example in the book
and compares what it printed against the `console` block beneath it:

```console
$ zuri docs/tools/verify.zu
```

Name a book, and a fragment of a filename, to check one part of it while
you are working:

```console
$ zuri docs/tools/verify.zu book ch04
```

**Limits are stated as rules, not as caveats.** "The `const` keyword is
enforced in local scopes" rather than a note apologising for it.

**Appendices D and E are generated, not written.** They come from the doc
blocks in `libs/_*.stub.zu`, read with the `zuri` module's own parser, so
the reference can never drift from the documentation the runtime ships:

```console
$ zuri docs/tools/book/generate.zu
```

That rewrites `appendix-04-builtins.md`, `appendix-05-type-methods.md` and
every `appendix-05-NN-*.md` page. Edit the stub, then re-run it. The one
exception is `appendix-05-10-function.md`, whose methods live in the
runtime rather than in a stub and which is maintained by hand.

## The Tools

Both books are generated and checked by Zuri programs in
[`tools/`](tools). Four of them are shared, because both books read the
same doc blocks and emit the same kind of markdown:

| | |
| --- | --- |
| [`docs.zu`](tools/docs.zu) | the doc block grammar: prose, `@tag` lines, and the wrapping rule that decides where a tag ends |
| [`markdown.zu`](tools/markdown.zu) | reflowing, wrapping, heading normalisation, escaping |
| [`audit.zu`](tools/audit.zu) | checks every doc block in `libs/` |
| [`verify.zu`](tools/verify.zu) | runs every example and compares its output |
| [`links.zu`](tools/links.zu) | checks that every internal link and anchor resolves |

The rest is per-book: [`tools/book`](tools/book) generates the
appendices, [`tools/reference`](tools/reference) generates the library
reference.

**A defect in a doc block becomes a defect in a book**, so the audit runs
as part of both generators and reports anything it finds:

```console
$ zuri docs/tools/audit.zu
161 files checked, no problems found
```

It catches three things. A doc block that is never closed. A code fence
that is opened and never closed, which swallows the block's own `@tag`
lines and, once rendered, every heading after it on the page — Zuri block
comments nest, so it counts depth rather than stopping at the first `*/`,
because one of the string stubs documents a callback whose example
contains a comment of its own. And a ```` ```zuri ```` example that does
not parse as Zuri.

That last check is why examples in `libs/` are not merely decorative.
Running them all is a separate job with separate costs — a socket, a
file, a prompt that never returns — but parsing them costs nothing and
catches the example that rotted when the syntax it used moved on. It
found sixteen on its first run, among them a shell command tagged as
Zuri, two `//` comments, a string in triple quotes, and a `??` operator
the language does not have.

Both generators still write their pages when the audit finds something —
the renderers are defensive enough to produce a usable page, and a stale
book helps nobody — but they exit non-zero so the problem is not missed.

Links are checked separately, across both books at once:

```console
$ zuri docs/tools/links.zu
220 pages checked, every link resolves
```

The images in the Imagine chapter are produced by
[`book/src/imagine/figures.zu`](book/src/imagine/figures.zu). Re-run it
from the repository root and the figures follow whatever the module
actually does:

```console
$ zuri docs/book/src/imagine/figures.zu
```
