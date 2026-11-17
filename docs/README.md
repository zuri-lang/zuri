# Zuri Documentation

This directory contains the book, the standard library reference, and
the Zuri programs that generate and check them.

| | |
| --- | --- |
| [`book/`](book) | **The Zuri Programming Language**, the main text. |
| [`reference/`](reference) | **The Zuri Standard Library**. Only its `doc.toml` and introduction are here. The rest is generated from `libs/`. |
| [`site/`](site) | The landing page that the book and the reference are published under. The installers it serves are in the repository root. |
| [`tools/`](tools) | The site build, the appendix generator, the doc block audit and the example verifier. |

## The Book

[**The Zuri Programming Language**](book/src/SUMMARY.md) is the main
text. It covers everything from installation to building a full-stack
web application, and then publishing and shipping it. Its appendices
cover keywords, operators, decorated methods, every built-in function,
every method of every built-in type, the standard library index, the
error hierarchy, and writing your own commands.

The book is written in Markdown under [`book/`](book), so it can be read
on GitHub as it is. `zuri doc` builds it into a searchable site, and
`cargo run-docs` builds it and serves it locally.

Start with [the introduction](book/src/ch00-00-introduction.md), or go
straight to what you need:

| If you want to | Read |
| --- | --- |
| install Zuri and run something | [Getting Started](book/src/ch01-00-getting-started.md) |
| see a whole program before the details | [Programming a Bookmark Keeper](book/src/ch02-00-bookmark-keeper.md) |
| look up syntax | [Common Programming Concepts](book/src/ch03-00-common-concepts.md) |
| look up a method | [Appendix E](book/src/appendix-05-type-methods.md) |
| find a module | [Appendix F](book/src/appendix-06-stdlib-index.md) |
| understand the JIT | [Performance and the JIT](book/src/ch22-00-performance.md) |
| adjust habits from another language | [Appendix H](book/src/appendix-08-coming-from.md) |

## The Standard Library Reference

**The Zuri Standard Library** is the API reference. It lists every
module that ships with Zuri, every public name in each module, and what
each one takes and returns. To read it locally, run
`cargo run-docs -- reference`.

The reference is generated, and none of its pages are kept in the
repository. `docs/reference` only contains a `doc.toml`, which asks for an
API reference of `libs/`, and `intro.md`, the text of its front page. The
pages are written to `target/reference/src` and rendered into
`target/reference/html`. Both are build output, and `cargo clean-docs`
removes them. Because a page only exists once it's generated from its
doc block, the two can't disagree.

Every page comes from a doc block in [`libs/`](../libs), read with the
`zuri` module's own parser. The reference is generated the same way as
any project's API reference, by `zuri doc generate`, which Chapter 30 of
the book describes. Its code is in
[`cmds/doc/reference`](../cmds/doc/reference).

`zuri doc build` and `zuri doc serve` regenerate the pages before they
render them, so `cargo build-docs` and `cargo run-docs -- reference`
don't need any manual steps. To write the pages without rendering them:

```console
$ cargo docs generate
... files checked, no problems found
The Zuri Standard Library: ... modules, ... names documented
  target/reference/src
```

Or run `zuri doc` directly:

```console
$ zuri doc generate docs/reference
```

Don't edit the pages under `target/reference/src`, because the next run
overwrites them. Edit the doc block in the library instead.

Three rules decide what appears in the reference:

- A name that starts with an underscore is private. The compiler doesn't
  allow importing one from another module, so private names never appear
  in the reference. The same applies to a file or directory whose name
  starts with an underscore.
- A member tagged `@internal` is left out, even though its name is
  public.
- Every other module gets its own page, nested under its package. The
  first doc block with an `@module` tag becomes the description at the
  top of that page.

The only part written by hand is the list of `[[reference.groups]]` in
[`reference/doc.toml`](reference/doc.toml). It decides which modules are
grouped together, and in what order, because a doc block can't say that
`hash` belongs next to `crypto` and not next to `math`. A module that's
added to `libs/` but left out of the groups is listed under Other Modules
with a warning. CI builds the reference with `--strict`, which turns that
warning into an error, so a new module can't silently go missing from the
table of contents.

## Reading the Documentation Locally

```console
$ cargo run-docs
```

This builds the book and serves it at <http://localhost:3000> with live
reload, so the page refreshes when you edit a chapter. To serve the
reference instead, name it. The page then refreshes when you edit a doc
block in `libs/`:

```console
$ cargo run-docs -- reference
```

If port 3000 is in use, pass a different one:

```console
$ cargo run-docs -- --port 4000
```

To render the HTML without serving it:

```console
$ cargo build-docs
```

The output goes to `target/book` and `target/reference/html`. Then
`build-docs` assembles the whole website in `target/site`:

```text
target/site/index.html     the landing page, from docs/site
target/site/install.sh     the installers, from the repository root
target/site/install.ps1
target/site/book/          the book
target/site/reference/     the standard library reference
```

That directory is the website as it's published, so you can open it
locally to check it first. The landing page is plain HTML and CSS in
[`site/`](site), and you edit it like any other file here. To assemble
the site again after changing it, without rendering the book or the
reference:

```console
$ cargo docs site
```

The book and the reference are both built by `zuri doc`, using the
`zuri` binary that each `cargo` alias builds first. That binary reads
`libs/` and `cmds/` directly from this checkout, so changes to either show
up in the output without rebuilding Zuri. You can also run `zuri doc`
directly, which is faster while you're writing:

```console
$ zuri doc serve docs/book
```

To remove the rendered output:

```console
$ cargo clean-docs
```

## Writing for the Book

Chapters are Markdown files in [`book/src`](book/src), listed in
[`SUMMARY.md`](book/src/SUMMARY.md). A file that isn't listed in
`SUMMARY.md` isn't part of the book.

The existing text follows a few rules.

**Every example is run before it's written down.** The output in a
`console` block is what the program actually printed, not what it should
print. When you change how something behaves, run the examples that
cover it again.

A block tagged ```` ```zuri,ignore ```` isn't run. There are five reasons
to tag a block this way:

- It's meant to show an error.
- It's one file of a program made of several files.
- It never finishes, such as a server or a prompt.
- It needs something that isn't available and can't be created, such as
  a remote host, an uploaded file or a licensed font. A block that only
  needs a local file doesn't count. Create the file in an earlier block on
  the same page instead, as the Imagine chapter does with `photo.jpg`.
- It's a catalog: several alternative calls listed together to show a
  family of methods. That isn't a program and couldn't run as one. The
  reference chapters have many of these, and the text before each one
  says so.

Every other example has to run. The tag means that an example can't be
checked. It isn't a way to avoid checking one.

This command runs every example in the book, and compares what each one
prints with the `console` block below it:

```console
$ zuri run docs/tools/verify.zu
```

It reports how many examples it ran. If that number goes down after a
change, a block was probably tagged instead of fixed.

To check part of the book while you're working on it, name the book and
part of a file name:

```console
$ zuri run docs/tools/verify.zu book ch04
```

**Limits are stated as rules, not as apologies.** For example, write
"The `const` keyword is enforced in local scopes", not a note apologizing
for what it doesn't do.

**Appendices D and E are generated.** They come from the doc blocks in
`libs/_*.stub.zu`, read with the `zuri` module's own parser, so the
appendices always match the documentation that ships with the runtime:

```console
$ zuri run docs/tools/book/generate.zu
```

This rewrites `appendix-04-builtins.md`, `appendix-05-type-methods.md`
and every `appendix-05-NN-*.md` page. To change one of them, edit the
stub, and then run the generator again. The exception is
`appendix-05-10-function.md`. The methods of functions are defined in the
runtime, not in a stub, so that page is written by hand.

## The Tools

The book and the reference are checked by Zuri programs in
[`tools/`](tools), and the book's appendices are generated by one of
them. The reference is generated by `zuri doc generate`. All of these
read the library's doc blocks with the standard library's
[`zuri.doc`](../libs/zuri/doc.zu) module, which defines the doc block
syntax: prose, `@tag` lines, and the wrapping rule that decides where a
tag ends.

| | |
| --- | --- |
| [`audit.zu`](tools/audit.zu) | Checks every doc block in `libs/` and `cmds/`. |
| [`verify.zu`](tools/verify.zu) | Runs every example and compares its output. |
| [`blocks.zu`](tools/blocks.zu) | Extracts each runnable example from a page, along with the output it claims, for `verify.zu`. |
| [`site.zu`](tools/site.zu) | Builds the book and the reference with `zuri doc`, and assembles the website, for the `cargo` aliases. |

[`tools/book`](tools/book) generates the book's appendices. It writes
its Markdown with the same helpers as the reference, in
[`cmds/doc/reference/text.zu`](../cmds/doc/reference/text.zu), which
handle reflowing, wrapping, heading levels and escaping.

A mistake in a doc block ends up in the book or the reference, so the
appendix generator and `site.zu` both run the audit first, and report
anything it finds:

```console
$ zuri run docs/tools/audit.zu
... files checked, no problems found
```

The audit checks for three problems:

- A doc block that's never closed.
- A code fence that's opened and never closed. It would swallow the
  block's own `@tag` lines and, once rendered, every heading after it on
  the page. Zuri block comments can be nested, so the audit counts the
  nesting depth instead of stopping at the first `*/`. One of the string
  stubs needs this, because it documents a callback whose example
  contains a comment of its own.
- A ```` ```zuri ```` example that doesn't parse as Zuri.

The last check matters because running every example in `libs/` is a
separate job, with costs of its own: some need a socket or a file, and
some wait on a prompt that never returns. Parsing them is cheap, though,
and it catches an example that stopped being valid when the syntax it
used changed, such as a `//` comment, a string in triple quotes, or a `??`
operator, none of which Zuri has.

The pages are still written when the audit finds a problem, because the
renderers can produce a usable page anyway, and an out-of-date page
helps nobody. The problem is still reported, so it isn't missed: the
appendix generator exits with a non-zero status, and `site.zu` prints a
warning.

`zuri doc` checks links itself as it builds, against the pages and
heading IDs it actually wrote. CI builds with `--strict`, which turns a
link that leads nowhere into an error:

```console
$ zuri doc build docs/book --strict
```

The images in the Imagine chapter are created by
[`book/src/imagine/figures.zu`](book/src/imagine/figures.zu). Run it
from the repository root to regenerate them, so they always show what
the module actually does:

```console
$ zuri run docs/book/src/imagine/figures.zu
```
