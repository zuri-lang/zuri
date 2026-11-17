# doc

Builds a documentation site from Markdown pages, or an API reference from
the doc comments in a project.

```sh
zuri doc [build|serve|init|generate|clean] [dir] [options]
```

Documentation is a directory that contains a `doc.toml` file and a `src`
directory of pages, with `src/SUMMARY.md` listing the pages in reading
order. The site has a page for each chapter with the table of contents
beside it, a search box, a page that prints everything at once, and light
and dark themes. Code blocks are highlighted when the site is built.
Chapter 30 of *The Zuri Programming Language*, "Documenting a Project",
describes how to write it.

An API reference created by `generate` is documentation too, with a
`[reference]` table in its `doc.toml`. `build` and `serve` regenerate its
pages from the doc comments before they build it.

If a directory next to the pages has a `doc.toml` of its own, such as
`docs/reference`, it's built along with the documentation around it, into
the site directory of the same name. Every page links to each of these
sites from its top bar, labeled Docs, API Reference, or whatever the
site's `[site] label` setting says. To link from one site to another,
write the relative path between the two Markdown files. These links are
checked like any other.

With no command, the documentation is built. `zuri doc` looks for it in
the directory you name, then in the current directory if it contains a
`doc.toml`, then in the project's `docs` directory, and finally in
`docs/reference`.

| Command | What it does |
| --- | --- |
| `build` | Builds the site, and reports links inside the documentation that lead nowhere. |
| `serve` | Builds and serves the site, and rebuilds it when the documentation changes. |
| `init` | Creates new documentation, in the project's `docs` directory or in the directory you name. |
| `generate` | Generates an API reference from the project's doc comments, in `docs/reference` or in the directory you name. |
| `clean` | Removes the built site. |

## Options

| Option | Description |
| --- | --- |
| `-o, --out <dir>` | Write the site to this directory instead of the one in `doc.toml`. |
| `--open` | Open the site in a browser once it's built. |
| `--strict` | With `build` or `generate`, treat warnings as errors, such as a link inside the documentation that leads nowhere. |
| `--external` | With `build`, also check every `http` and `https` link in the documentation, and warn about any that don't respond. |
| `-p, --port <port>` | With `serve`, the port to listen on. Defaults to 3000. |
| `--host <address>` | With `serve`, the address to listen on. By default, only this machine can connect. |
| `--title <text>` | With `init` or `generate`, the title of the new documentation. |

## Source files

| File | Contents |
| --- | --- |
| `index.zu` | The command itself. |
| `plan.zu` | The argument parser, and the rules for finding the documentation. |
| `build.zu` | Building documentation that contains other documentation, and the links between them. |
| `config.zu` | Reading and checking `doc.toml`. |
| `summary.zu` | Reading `SUMMARY.md` into the table of contents. |
| `documentation.zu` | The chapters: numbering them, placing them, and reading them with their includes. |
| `render.zu` | Converting a chapter's Markdown to its page's HTML. |
| `site.zu` | The site: every page, the search index and the print page. |
| `serve.zu` | The local server and the rebuilding. |
| `scaffold.zu` | The files that `init` creates, and the ones that `generate` starts a reference with. |
| `reference/` | The API reference: which files are modules, their pages, the table of contents and the front page. |
| `highlight/` | Syntax highlighting for each language, at build time. |
| `theme/` | The page template, the stylesheet, the script and the fonts. |

The fonts in `theme/fonts` are licensed under the SIL Open Font License.
See `theme/fonts/LICENSE.txt`.
