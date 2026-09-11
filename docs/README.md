# Zuri Documentation

Everything written about the language lives here.

## The Book

[**The Zuri Programming Language**](book/src/SUMMARY.md) is the main text:
twenty chapters from installation to a full-stack web application, plus
eight appendices covering keywords, operators, decorated methods, every
built-in function, every method on every built-in type, the standard
library index, and the error hierarchy.

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
| understand the JIT | [Performance and the JIT](book/src/ch18-00-performance.md) |
| port habits from another language | [Appendix H](book/src/appendix-08-coming-from.md) |

## Reading It Locally

```console
$ cargo run-docs
```

That builds the book and serves it at <http://localhost:3000> with live
reload, so an edit to a chapter refreshes the page. Pass a port if 3000 is
taken:

```console
$ cargo run-docs -- --port 4000
```

To render the HTML without serving it:

```console
$ cargo build-docs
```

Output lands in `target/book`.

Both commands need [mdBook](https://rust-lang.github.io/mdBook). If it is
not installed they offer to install it for you, and nothing is installed
without your say-so.

A plain `cargo build` also rebuilds the book, but only when mdBook is
already installed and only when something under `docs/book/src` changed.
Without mdBook you get one line of warning and the build carries on.

## Writing for the Book

Chapters are plain markdown in [`book/src`](book/src), listed in
[`SUMMARY.md`](book/src/SUMMARY.md). A file that is not in `SUMMARY.md` is
not in the book.

Two rules the existing text follows:

**Every example is run before it is written down.** The output in a
`console` block is what the program actually printed, not what it ought to
print. When you change behaviour, re-run the examples that cover it.

**Limits are stated as rules, not as caveats.** "The `const` keyword is
enforced in local scopes" rather than a note apologising for it.

Two appendices are generated rather than hand-written, and are regenerated
from the standard library's own stub documentation:

- Appendix D comes from `libs/_builtins.stub.zu`
- Appendix E comes from `libs/_*.stub.zu`

Edit the stub, not the appendix.

The images in the Imagine chapter are produced by
[`book/src/imagine/figures.zu`](book/src/imagine/figures.zu). Re-run it
from the repository root and the figures follow whatever the module
actually does:

```console
$ zuri docs/book/src/imagine/figures.zu
```
