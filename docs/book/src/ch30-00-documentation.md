# Documenting a Project

`zuri doc` turns a directory of Markdown files into a website, with a page
for each chapter and a table of contents next to it, a search box, a page
that prints everything at once, light and dark themes, and syntax
highlighting for every code block. It can also generate an API reference
from the doc comments in a project's code. This book is built with it, and
so is the standard library reference.

The output is a directory of plain files. Any web server can serve it,
GitHub Pages can publish it as it is, and a browser can open it straight
from disk.

- [Starting Documentation](#starting-documentation)
- [The Table of Contents](#the-table-of-contents)
- [Writing Pages](#writing-pages)
  - [Links and Files](#links-and-files)
  - [Code](#code)
  - [Including Files](#including-files)
- [Building and Serving](#building-and-serving)
- [Generating an API Reference](#generating-an-api-reference)
- [Nested Documentation](#nested-documentation)
- [Settings](#settings)
- [Customizing the Appearance](#customizing-the-appearance)
- [Publishing](#publishing)

## Starting Documentation

Inside a project, `zuri doc init` starts documentation in its `docs`
directory:

```console
$ zuri doc init
Started weather in docs
  doc.toml
  .gitignore
  src/SUMMARY.md
  src/introduction.md
  src/getting-started.md

Build it with zuri doc build, or read it as you write with zuri doc serve.
```

The documentation uses the project's name as its title, unless you give it a
different one with `--title`. Outside a project, it's created in the current
directory, and if you name a directory, it's created there instead, as in
`zuri doc init manual`.

Documentation is organized like this:

```text
docs/
  doc.toml              the settings
  src/
    SUMMARY.md          the contents
    introduction.md
    getting-started.md
  site/                 the built site, which .gitignore leaves out
```

Every command finds the documentation in the same way. It uses the directory
named on the command line, or otherwise the current directory if it contains
a `doc.toml`, or otherwise the `docs` directory of the project you're in, or
failing that its `docs/reference`. That means `zuri doc build` works from
anywhere inside the project.

## The Table of Contents

`src/SUMMARY.md` lists the pages in reading order. A page that isn't listed
there isn't part of the documentation.

```markdown
# Summary

[Introduction](introduction.md)

# Using It

- [Getting Started](getting-started.md)
  - [Installing](getting-started/installing.md)
  - [Configuring](getting-started/configuring.md)
- [Forecasts](forecasts.md)
- [Alerts]()

# Reference {.unnumbered}

- [Settings](settings.md)

---

[Changelog](changelog.md)
```

- The first heading is the title of the summary itself, and isn't shown.
  Every other heading starts a part, which is a titled group of chapters
  that the table of contents marks with its own color.
- A link in a list is a numbered chapter. A list nested inside its item
  contains its sections, which are numbered beneath it as 1.1, 1.2 and so
  on.
- A link outside any list is an unnumbered chapter, such as an introduction
  before the numbered chapters or a changelog after them.
- A link without an address, like `[Alerts]()`, is a draft. It's listed,
  grayed out, until it has a page.
- `---` draws a line across the table of contents.
- `{.unnumbered}` after a part's heading leaves that part's chapters
  unnumbered, which suits appendices. After a single link, as in
  `[Settings](settings.md){.unnumbered}`, it does the same for that chapter
  and its sections.

Each page is written to the same place as its Markdown file, with `.html`
instead of `.md`, so `getting-started/installing.md` becomes
`getting-started/installing.html`. A `README.md` becomes its directory's
`index.html`. The front page, `index.html`, is the first chapter.

## Writing Pages

Pages are written in CommonMark, with GitHub's extensions: tables, task
lists, strikethrough, footnotes, automatic links, emoji shortcodes such as
`:rocket:`, and alerts written as `> [!NOTE]`, `> [!TIP]`, `> [!IMPORTANT]`,
`> [!WARNING]` or `> [!CAUTION]`. You can also write HTML directly in a
page.

Every heading gets an ID: the heading in lowercase, with each space replaced
by a dash and other punctuation removed, so `## Running in Production`
becomes `#running-in-production`, the same ID GitHub gives it. A heading
that repeats an earlier one gets `-1` added to it, then `-2`, and so on.
Braces after a heading let you give it an ID of your own choosing, and
classes:

```markdown
## Running in Production {#production .advanced}
```

When you hover over a heading, a `§` appears that links to it, and the
headings on a page are listed next to it under "On this page".

The `[markdown]` table in `doc.toml` changes how pages are read. Its keys
are the options of the standard library's `markdown` module, so
`smart = true` gives you curly quotes and proper dashes, and `math = true`
reads `$...$` as mathematics and typesets it on the page. The `markdown`
page of the library reference lists all of the options.

### Links and Files

To link to another page, link to its Markdown file, as you would on GitHub:
`[the units](settings.md#units)`. The link is updated to point to the page
when the site is built, so it works both on GitHub and on the site.

Images, and any other files in `src` that aren't Markdown, are copied into
the site next to the pages, at the same paths, so an image written as
`![a chart](images/rain.png)` appears in both places too.

Building the site checks every link within the documentation, and reports
any link that leads to a page not listed in `SUMMARY.md`, to a heading that
doesn't exist on the page, or to a file that doesn't exist, along with the
line it's on.

### Code

Code blocks are highlighted when the site is built, so a page looks the same
with JavaScript turned off. The first word of the fence names the language:

| Language | Fence |
| --- | --- |
| Zuri | `zuri` |
| A REPL session | `zuri-repl` |
| HTML and Wire | `html`, `wire` |
| XML | `xml`, `svg` |
| JSON | `json` |
| Shell scripts | `sh`, `bash`, `shell` |
| Terminal sessions | `console`, `terminal` |
| TOML | `toml` |
| INI and `.env` files | `ini`, `env` |
| JavaScript and CSS | `js`, `css` |

Anything after the first word is ignored, so a block fenced as `zuri,ignore`
is highlighted as Zuri. A block in any other language is shown as plain
text, labeled with the name given in its fence.

In a `console` block, a line starting with a `$ ` prompt is a command, and
is highlighted as one. Every other line is the output of the commands. A
`console` block without any prompt is a program's output on its own, and
when it comes directly after a code block, the two are shown joined
together, as the code and the output it printed. Every example in this book
is written this way, as a `zuri` block followed by a `console` block:

```zuri
echo 'hello'
```

```console
hello
```

A paragraph that only contains
`<span class="filename">Filename: main.zu</span>`, written just before a
code block, becomes a caption on the block that names the file it shows.

Every code block has a button that copies it. On a terminal session, it only
copies the commands, without their prompts or their output.

### Including Files

`\{{#include path}}` is replaced by the contents of the file at _path_,
relative to the page. When it's the only line inside a code block, the block
shows that file, so an example always matches the file it comes from, and
can't get out of sync with code that's tested elsewhere:

```markdown
\{{#include ../../examples/forecast.zu}}
```

A colon after the path selects part of the file: `forecast.zu:12` is line 12
on its own, `forecast.zu:5:20` is lines 5 to 20, `forecast.zu:5:` is line 5
to the end, and `forecast.zu::20` is up to line 20. A name instead of
numbers, as in `forecast.zu:today`, selects the lines between two comments
in the file that mark them, so the file can still be a complete program that
runs:

```zuri
# ANCHOR: today
var today = 'Rain, then sun'
# ANCHOR_END: today

echo today
```

```console
Rain, then sun
```

The marker lines themselves are never included, whichever part of the file
you select.

`\{{#title text}}` sets the title shown in the browser tab for a page,
instead of the one the table of contents gives it. To write either of these
literally, put a backslash in front, as in `\\{{#include x}}`.

## Building and Serving

`zuri doc build` builds the site:

```console
$ zuri doc build
weather: 5 pages in 0.04s
  docs/site
```

When a link within the documentation doesn't lead anywhere, it tells you
where the link is:

```console
$ zuri doc build --strict
warning: docs/src/getting-started.md:10: introduction.md has no heading or id #installing
warning: docs/src/getting-started.md:10: links to forecasts.md, which SUMMARY.md does not list
weather: 5 pages in 0.03s
  docs/site
doc: 2 links inside the documentation lead nowhere
```

Without `--strict`, the warnings are printed and the build succeeds. With
it, the command fails, which is how a CI job can make sure the links stay
valid. `--out` writes the site somewhere other than the location in
`doc.toml`, and `--open` opens it in a browser once it's built.

`--external` also requests every `http` and `https` address the pages link
to, once for each address, however many pages link to it, and warns about
any address that responds with an error or can't be reached, after trying
once more. A response asking it to slow down counts as a successful
response. Addresses are requested a few at a time, so it takes seconds
rather than minutes, but it needs network access, so it only runs when you
ask for it rather than on every build:

```console
$ zuri doc build --external
warning: docs/src/getting-started.md:6: https://example.com/old-page answered 404
weather: 5 pages in 0.04s
  docs/site
```

With `--strict` as well, a broken address also fails the build.

While you're writing, `zuri doc serve` builds the site, serves it on your
machine, and rebuilds it every time the documentation changes:

```console
$ zuri doc serve
Built 5 pages in 0.03s

  weather at http://127.0.0.1:3000/

  Changes to the documentation rebuild it, and open pages reload. Ctrl+C stops.
```

A page that's open in a browser reloads itself after every rebuild. If a
build fails, it tells you why and keeps the last working version of the
site, and the next save tries again. `--port` serves the site on a port
other than 3000, `--host 0.0.0.0` lets other machines access it, and
`--open` opens it in a browser.

`zuri doc clean` removes the built site.

## Generating an API Reference

`zuri doc generate` creates a reference for the project from its doc
comments, with a page for every public module, showing each function,
constant and class it contains, what they accept and what they return, with
links between them.

```console
$ zuri doc generate
Started weather API Reference in docs/reference
  doc.toml
  .gitignore

weather API Reference: 2 modules, 2 names documented
  docs/reference/src

Build it with zuri doc build docs/reference, or read it with zuri doc serve docs/reference.
```

The reference is documentation like any other, with its own `doc.toml`, in
`docs/reference` unless you name a different directory. Its pages are
Markdown files in `src`, which are written again from scratch on every run,
so its `.gitignore` excludes them along with the site. `zuri doc build` and
`zuri doc serve` regenerate them before building, and `serve` rebuilds
whenever a module changes, so the reference is never out of date with the
code. Edit the doc comments, never the generated pages.

Every `.zu` file in the project is treated as a module, except:

- A file with any part of its path starting with an underscore, which the
  compiler keeps private.
- Tests, meaning `*.test.zu` files, and the `tests`, `docs`, `examples`,
  `benchmarks` and `cmds` directories at the top level of the project.
- Anything that `[project] include` and `exclude` in `project.toml` leave
  out of the package, and anything that `.gitignore` ignores.

A module is named the way it's imported. The project's `index.zu` is the
package itself, such as `weather`, `app/index.zu` is `weather.app`, and
`app/forecast.zu` would be `weather.app.forecast`. Each name is documented
on the page for the file that declares it, and written with the path a
reader would use to reach it. When `index.zu` re-exports `app` with
`import @.app { * }`, the `greet` function in `app` is documented as
`weather.greet`. A package's own page also lists every public name that can
be reached through it, wherever it's declared.

A module's description is its doc comment tagged with `@module`, or if there
isn't one, the doc comment at the top of the file, if an import comes after
it. The tags in a declaration's doc comment become the sections of its
entry: `@param`, `@returns`, `@throws`, `@note`, `@deprecated` and `@see`. A
name in braces in the text, such as `{Forecast}` or `{greet()}`, links to
where that name is documented, and `@internal` leaves a declaration out of
the reference completely.

The `[reference]` table in `doc.toml` is what makes a set of documentation a
reference:

| Key | Default | Description |
| --- | --- | --- |
| `modules` | The project containing `doc.toml` | The directory of modules to document |
| `package` | The project's name, with each `-` replaced by `_` | The name the modules are imported under, or `""` for none |
| `exclude` | `[]` | Patterns for files to leave out, relative to `modules` |
| `intro` | None | A Markdown file to show at the start of the front page, instead of the description |
| `groups` | None | Parts of the table of contents, each with a `title`, a `description` and its `modules` |

Without groups, all the modules are shown in a single list, each one under
the module it belongs to. With groups, each group becomes a part of the
table of contents:

```toml
[[reference.groups]]
title = "Forecasts"
description = "Asking about the weather."
modules = ["weather", "weather.app"]
```

A module that isn't in any group is listed at the end, under Other Modules,
with a warning, and `--strict` fails the build because of it, so a new
module can't silently go missing from the table of contents. The standard
library reference is generated this way.

## Nested Documentation

A directory next to `src` that contains its own `doc.toml` is also
documentation, and it's built along with the documentation around it, into
the directory of the site with the same name. The most common case is the
reference that `zuri doc generate` writes to `docs/reference`. If there's
also a guide in `docs`, `zuri doc build` builds both,

```console
$ zuri doc build
weather: 5 pages in 0.39s
  docs/site
weather API Reference: 5 pages in 0.36s
  docs/site/reference
```

and `zuri doc serve` serves both, rebuilding whenever a page or a module
changes.

The top of every page then links to each set of documentation, and
highlights the one the page belongs to. The outer documentation is labeled
Docs, a reference is labeled API Reference, and anything else is labeled
with its title. You can set a different label with `label` in a site's own
`[site]` table:

```toml
[site]
label = "Guide"
```

Any of the logo, favicon, repository, `[site.home]`, `[[site.links]]`, `css`
and `js` that a nested site doesn't set itself are taken from the site
around it, so the whole thing reads as a single site, and its `url` is the
outer `url` with the directory's name added. If you build it on its own, as
in `zuri doc build docs/reference`, it's a separate site and doesn't inherit
any of these settings.

A link from one set of documentation to another is written the same way it
would be on GitHub, as the path from one Markdown file to the other. From
`docs/src/guide.md`, `[greet](../reference/src/weather-app.md#greet)` links
to the heading for `greet()` on the reference page for `weather.app`. The
link is updated to point to the page that file becomes, and the build checks
it along with all the others, naming the documentation it leads into if it
doesn't lead anywhere.

## Settings

`doc.toml` has five tables, and every key in them is optional. A key that a
table doesn't support, or a value of the wrong type, stops the build with an
error that names the key.

```toml
[documentation]
title = "Weather"
description = "Forecasts in your terminal."
authors = ["Ada"]
language = "en"
source = "src"
output = "site"

[site]
url = "https://example.com/weather/"
repository = "https://github.com/ada/weather"
edit = "https://github.com/ada/weather/edit/main/docs/src/{path}"
logo = "logo.svg"
fold = 1

[site.home]
url = "../"
label = "Home"

[[site.links]]
label = "API"
url = "../api/"

[markdown]
smart = true

[redirects]
"setup.html" = "getting-started/installing.html"
```

`[documentation]` describes the documentation:

| Key | Default | Description |
| --- | --- | --- |
| `title` | The directory's name | The title, shown at the top of every page |
| `description` | None | A one-sentence description, for search engines |
| `authors` | None | The authors' names |
| `language` | `"en"` | The language it's written in |
| `source` | `"src"` | The directory that contains the pages |
| `output` | `"site"` | Where the site is written |
| `create-missing` | `false` | Whether to create an empty page for a chapter whose file is missing, instead of stopping |

`[site]` describes how the site looks and where it's published:

| Key | Default | Description |
| --- | --- | --- |
| `url` | None | The address the site is published at |
| `repository` | None | The source code repository, linked from every page |
| `edit` | None | An address for editing a page, with `{path}` standing for the page's file |
| `logo` | None | An image shown next to the title |
| `favicon` | None | The icon shown in the browser tab |
| `theme` | `"system"` | How a page first looks: `"system"`, `"light"` or `"dark"` |
| `fold` | `false` | How many levels of the table of contents are expanded at first. `false` expands every level. |
| `numbers` | `true`, or `false` for a reference | Whether chapters are numbered |
| `search` | `true` | Whether the site can be searched |
| `print` | `true` | Whether the site has a page that prints everything at once |
| `css` | `[]` | Stylesheets added to every page |
| `js` | `[]` | Scripts added to every page |
| `cname` | None | The domain that GitHub Pages serves the site at |
| `label` | `"Docs"`, `"API Reference"` or the title | What the links at the top call this site when it's built with others |

The table of contents is always expanded as far as the current page,
whatever `fold` is set to. `[site.home]` adds a link back to a parent site
next to the title, and each `[[site.links]]` entry adds a link to the top of
every page. An address that isn't absolute is relative to the front page.
`[markdown]` is described in [Writing Pages](#writing-pages), `[reference]`
in [Generating an API Reference](#generating-an-api-reference), and
`[redirects]` keeps an old address working by redirecting it to a new one.

Every path in the file is relative to the directory that contains it.

## Customizing the Appearance

Readers can choose light or dark mode, or follow their system setting, from
the menu at the top of every page, and their choice is remembered. The
`theme` setting only decides how a page looks before they've chosen.

Every page is rendered from a single Wire template, `page.html`, styled by
`doc.css`, with its behavior provided by `doc.js`. A directory called
`theme` next to `doc.toml` can replace any of these three files with a file
of the same name. The versions that `zuri doc` uses are in `cmds/doc/theme`
next to the `zuri` executable, and a copy of one of them is a good place to
start. The `css` and `js` settings add to them instead of replacing them,
which is usually all you need:

```css
:root {
  --measure: 50rem;
}
```

## Publishing

The site is a directory of plain files without anything to run, so
publishing it just means copying it somewhere a web server can read it.

GitHub Pages can serve it as it is. `zuri doc` writes the `.nojekyll` file
that stops GitHub from leaving out files it doesn't expect, and a `CNAME`
file when `cname` is set. Set `url` as well. It gives every page its
canonical address, generates a `sitemap.xml` for search engines, and lets
`404.html`, the page shown for a missing address, work at any depth of the
site, not only at the top level.

You can use every site without a mouse. `/`, `s`, or Ctrl+K (⌘K on a Mac)
opens the search box, the up and down arrow keys move through the results,
and the left and right arrow keys go to the previous and next chapter.
