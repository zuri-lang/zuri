# doc

Builds a documentation site from a book written in Markdown.

```sh
zuri doc [build|serve|init|clean] [dir] [options]
```

A book is a directory holding a `doc.toml` and a `src` directory of
pages, with `src/SUMMARY.md` listing them in reading order. The site has a
page for each chapter with the contents beside it, a search box, a page
that prints the whole book, light and dark themes, and every code block
coloured as it is built. Chapter 30 of the book covers writing one.

With no command, the book is built. The book is the directory named, or
the current directory when it holds a `doc.toml`, or the `docs` directory
of the project.

| Command | What it does |
| --- | --- |
| `build` | writes the site, and reports links inside the book that lead nowhere |
| `serve` | builds the site, serves it, and rebuilds it as the book changes |
| `init` | starts a book: in the project's `docs`, or the directory named |
| `clean` | removes the built site |

## Flags

| Flag | What it does |
| --- | --- |
| `-o, --out <dir>` | Writes the site here instead of where `doc.toml` says. |
| `--open` | Opens the site in a browser once it is built. |
| `--strict` | With `build`, fails when a link inside the book leads nowhere. |
| `-p, --port <port>` | With `serve`, the port to listen on; 3000 by default. |
| `--host <address>` | With `serve`, the address to listen on; this machine only by default. |
| `--title <text>` | With `init`, the book's title. |

## Layout

| File | What it holds |
| --- | --- |
| `index.zu` | the command itself |
| `plan.zu` | the parser, and where the book is |
| `config.zu` | `doc.toml`, read and checked |
| `summary.zu` | `SUMMARY.md`, read into the contents |
| `book.zu` | the chapters: numbered, placed, and read with their includes |
| `render.zu` | a chapter's Markdown as its page's HTML |
| `site.zu` | the site: every page, the search index, the print page |
| `serve.zu` | the local server and the rebuilding |
| `scaffold.zu` | the files `init` writes |
| `highlight/` | colouring each language, at build time |
| `theme/` | the page template, the stylesheet, the script and the fonts |

The fonts in `theme/fonts` are under the SIL Open Font License; see
`theme/fonts/LICENSE.txt`.
