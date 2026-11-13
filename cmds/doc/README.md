# doc

Builds a documentation site from pages written in Markdown, or from the
doc blocks in a project.

```sh
zuri doc [build|serve|init|generate|clean] [dir] [options]
```

Documentation is a directory holding a `doc.toml` and a `src` directory
of pages, with `src/SUMMARY.md` listing them in reading order. The site
has a page for each chapter with the contents beside it, a search box, a
page that prints all of it at once, light and dark themes, and every code
block coloured as it is built. Chapter 30 of *The Zuri Programming
Language* covers writing it.

An API reference from `generate` is documentation too, with a
`[reference]` table in its `doc.toml`, and `build` and `serve` write its
pages again from the doc blocks before building it.

A directory beside the pages that holds a `doc.toml` of its own, such as
`docs/reference`, is built along with the documentation around it, into
the site directory of the same name, and every page links to each site
from its top bar: Docs, API Reference, or whatever `[site] label` says.
A link from one to another is written as the path between the two
Markdown files, and is checked like any other.

With no command, the documentation is built. It is the directory named,
or the current directory when it holds a `doc.toml`, or the `docs`
directory of the project, or failing that its `docs/reference`.

| Command | What it does |
| --- | --- |
| `build` | writes the site, and reports links inside the documentation that lead nowhere |
| `serve` | builds the site, serves it, and rebuilds it as the documentation changes |
| `init` | starts documentation: in the project's `docs`, or the directory named |
| `generate` | writes an API reference from the project's doc blocks, in `docs/reference` or the directory named |
| `clean` | removes the built site |

## Flags

| Flag | What it does |
| --- | --- |
| `-o, --out <dir>` | Writes the site here instead of where `doc.toml` says. |
| `--open` | Opens the site in a browser once it is built. |
| `--strict` | With `build` or `generate`, fails over a warning, such as a link inside the documentation that leads nowhere. |
| `-p, --port <port>` | With `serve`, the port to listen on; 3000 by default. |
| `--host <address>` | With `serve`, the address to listen on; this machine only by default. |
| `--title <text>` | With `init` or `generate`, the new documentation's title. |

## Layout

| File | What it holds |
| --- | --- |
| `index.zu` | the command itself |
| `plan.zu` | the parser, and where the documentation is |
| `build.zu` | building documentation with the documentation inside it, and the links between them |
| `config.zu` | `doc.toml`, read and checked |
| `summary.zu` | `SUMMARY.md`, read into the contents |
| `documentation.zu` | the chapters: numbered, placed, and read with their includes |
| `render.zu` | a chapter's Markdown as its page's HTML |
| `site.zu` | the site: every page, the search index, the print page |
| `serve.zu` | the local server and the rebuilding |
| `scaffold.zu` | the files `init` writes, and those `generate` starts a reference with |
| `reference/` | the API reference: which files are modules, their pages, the contents and front page |
| `highlight/` | colouring each language, at build time |
| `theme/` | the page template, the stylesheet, the script and the fonts |

The fonts in `theme/fonts` are under the SIL Open Font License; see
`theme/fonts/LICENSE.txt`.
