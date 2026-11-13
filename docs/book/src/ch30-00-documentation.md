# Documenting a Project

`zuri doc` turns a directory of Markdown into a website: a page for each
chapter with the contents beside it, a search box, a page that prints all
of it at once, light and dark themes, and every code block coloured. It
also writes an API reference from the doc blocks in a project's code.
This book is built with it, and so is the standard library reference.

What it writes is a directory of plain files. Any web server can serve
it, GitHub Pages can publish it as it is, and a browser can read it
straight from disk.

- [Starting Documentation](#starting-documentation)
- [The Contents](#the-contents)
- [Writing Pages](#writing-pages)
  - [Links and Files](#links-and-files)
  - [Code](#code)
  - [Including Files](#including-files)
- [Building and Serving](#building-and-serving)
- [Generating an API Reference](#generating-an-api-reference)
- [Documentation Inside Documentation](#documentation-inside-documentation)
- [Settings](#settings)
- [Changing the Look](#changing-the-look)
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

The documentation takes the project's name as its title, and `--title` gives it
another. Outside a project it starts in the current directory, and
naming a directory starts it there instead: `zuri doc init manual`.

Documentation is laid out like this:

```text
docs/
  doc.toml              the settings
  src/
    SUMMARY.md          the contents
    introduction.md
    getting-started.md
  site/                 the built site, which .gitignore leaves out
```

Every command finds the documentation the same way: the directory named
on the command line, or else the current directory when it holds a
`doc.toml`, or else the `docs` directory of the project you are in, or
failing that its `docs/reference`. So `zuri doc build` works from
anywhere inside the project.

## The Contents

`src/SUMMARY.md` lists the pages in the order they are read. A page it
does not list is not in the documentation.

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

- The first heading is the summary's own title, and is not shown. Every
  other heading starts a part: a titled group of chapters, which the
  contents mark with a colour of its own.
- A link in a list is a numbered chapter. A list inside its item holds
  its sections, numbered beneath it as 1.1 and 1.2.
- A link outside any list is an unnumbered chapter, such as an
  introduction before the numbered ones or a changelog after them.
- A link with no address, like `[Alerts]()`, is a draft. It is listed,
  greyed out, until it has a page.
- `---` draws a line across the contents.
- `{.unnumbered}` after a part's heading leaves that part's chapters
  unnumbered, which suits appendices. After a single link, as in
  `[Settings](settings.md){.unnumbered}`, it does the same for that
  chapter and its sections.

Each page is written where its Markdown file is, with `.html` in place of
`.md`, so `getting-started/installing.md` becomes
`getting-started/installing.html`. A `README.md` becomes its directory's
`index.html`. The front page, `index.html`, is the first chapter.

## Writing Pages

Pages are CommonMark, with GitHub's additions: tables, task lists,
strikethrough, footnotes, addresses made into links, emoji shortcodes
such as `:rocket:`, and notes written as `> [!NOTE]`, `> [!TIP]`,
`> [!IMPORTANT]`, `> [!WARNING]` or `> [!CAUTION]`. HTML can be written
into a page as it is.

Every heading gets an id: lower case, with each space made a dash and
other punctuation dropped, so `## Running in Production` is
`#running-in-production`, the same id GitHub gives it. A heading that
repeats an earlier one gets `-1` after it, then `-2`. Braces after a
heading give it an id of your choosing, and classes:

```markdown
## Running in Production {#production .advanced}
```

Hovering over a heading shows a `§` that links to it, and the headings of
a page are listed beside it under "On this page".

The `[markdown]` table in `doc.toml` changes how pages are read. Its keys
are the options of the standard library's `markdown` module, so
`smart = true` gives curly quotes and real dashes, and `math = true`
reads `$...$` as mathematics and typesets it on the page. The `markdown`
page of the library reference lists them all.

### Links and Files

Link to another page by its Markdown file, as you would on GitHub:
`[the units](settings.md#units)`. The link is pointed at the page when the
site is built, so it works on GitHub and on the site alike.

Images, and any other file in `src` that is not Markdown, are copied into
the site beside the pages at the same paths, so an image written as
`![a chart](images/rain.png)` shows in both places too.

Building checks every link that stays inside the documentation, and
reports one that leads to a page `SUMMARY.md` does not list, to a heading
a page does not have, or to a file that is not there, with the line it is
on.

### Code

Code blocks are coloured as the site is built, so a page reads the same
with scripts turned off. The first word of the fence names the language:

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

Anything after that first word is left alone, so a block fenced
`zuri,ignore` is coloured as Zuri. A block in any other language is
shown as it is, labelled with the name its fence gives.

In a `console` block, a line starting with a `$ ` prompt is a command,
and is coloured as one. Every other line is what the commands printed. A
`console` block with no prompt in it at all is a program's output on its
own, and when it follows a code block directly the two are shown joined,
as the code and what it printed. Every example in this book is written
that way, a `zuri` block and then a `console` block:

```zuri
echo 'hello'
```

```console
hello
```

A paragraph holding nothing but `<span class="filename">Filename:
main.zu</span>`, written just before a code block, becomes a caption on
the block naming the file it shows.

Every block of code has a button that copies it. On a terminal session it
copies the commands alone, without their prompts or what they printed.

### Including Files

`\{{#include path}}` is replaced by the file at _path_, relative to the
page. Written as the only line inside a code block, it makes the block
show that file, so an example is always the same as the file it comes
from and cannot drift from code that is tested elsewhere:

```markdown
\{{#include ../../examples/forecast.zu}}
```

A colon after the path takes part of the file: `forecast.zu:12` is line
12 alone, `forecast.zu:5:20` lines 5 to 20, `forecast.zu:5:` line 5 on,
and `forecast.zu::20` up to line 20. A name instead of numbers,
`forecast.zu:today`, takes the lines between two comments in the file
that mark them, so the file stays a whole program that runs:

```zuri
# ANCHOR: today
var today = 'Rain, then sun'
# ANCHOR_END: today

echo today
```

```console
Rain, then sun
```

The marking lines are never included themselves, whichever part of the
file is taken.

`\{{#title text}}` gives a page the title its browser tab shows, in place
of the one the contents give it. A backslash in front, as in
`\\{{#include x}}`, writes either of them as it is.

## Building and Serving

`zuri doc build` writes the site:

```console
$ zuri doc build
weather: 5 pages in 0.04s
  docs/site
```

When a link inside the documentation leads nowhere, it says where:

```console
$ zuri doc build --strict
warning: docs/src/getting-started.md:10: introduction.md has no heading or id #installing
warning: docs/src/getting-started.md:10: links to forecasts.md, which SUMMARY.md does not list
weather: 5 pages in 0.03s
  docs/site
doc: 2 links inside the documentation lead nowhere
```

Without `--strict` the warnings are printed and the build succeeds. With
it the command fails, which is how a CI job keeps the links whole.
`--out` writes the site somewhere other than where `doc.toml` says, and
`--open` opens it in a browser once it is built.

While you write, `zuri doc serve` builds the site, serves it on this
machine, and builds it again each time the documentation changes:

```console
$ zuri doc serve
Built 5 pages in 0.03s

  weather at http://127.0.0.1:3000/

  Changes to the documentation rebuild it, and open pages reload. Ctrl+C stops.
```

A page open in a browser reloads itself after every rebuild. A build that
fails says why and leaves the last good site in place, and the next save
tries again. `--port` serves somewhere other than 3000, `--host 0.0.0.0`
lets other machines read it, and `--open` opens it.

`zuri doc clean` removes the built site.

## Generating an API Reference

`zuri doc generate` writes a reference for the project from its doc
blocks: a page for every public module, with each function, constant and
class in it, what they take and what they return, and links between
them.

```console
$ zuri doc generate
Started weather API Reference in docs/reference
  doc.toml
  .gitignore

weather API Reference: 2 modules, 2 names documented
  docs/reference/src

Build it with zuri doc build docs/reference, or read it with zuri doc serve docs/reference.
```

The reference is documentation like any other, with a `doc.toml` of its
own, in `docs/reference` unless a directory is named. Its pages are
Markdown in `src`, written afresh on every run, so its `.gitignore` leaves
them out along with the site. `zuri doc build` and `zuri doc serve`
generate them again before building, and `serve` rebuilds when a module
changes, so the reference is never older than the code. The doc blocks
are what to edit, never the pages.

Every `.zu` file in the project is a module, except:

- a file with any part of its path starting with an underscore, which
  the compiler keeps private;
- tests, as `*.test.zu` files, and the `tests`, `docs`, `examples`,
  `benchmarks` and `cmds` directories at the top of the project;
- what `[project] include` and `exclude` in `project.toml` leave out of
  the package, and what `.gitignore` ignores.

A module is named the way it is imported. The project's `index.zu` is
the package itself, `weather`, `app/index.zu` is `weather.app`, and
`app/forecast.zu` would be `weather.app.forecast`. A name is documented on
the page of the file that declares it, and written with the path a
reader types to reach it: when `index.zu` lifts `app` with
`import @.app { * }`, `app`'s `greet` is `weather.greet`. A package's own
page also lists every public name reached through it, wherever it is
declared.

A module's description is its doc block tagged `@module`, or failing that
the doc block at the top of the file when an import follows it. The tags
of a declaration's doc block become the sections of its entry:
`@param`, `@returns`, `@throws`, `@note`, `@deprecated` and `@see`. A
name in braces in the prose, as in `{Forecast}` or `{greet()}`, links to
where that name is documented, and `@internal` keeps a declaration out
of the reference altogether.

The `[reference]` table in `doc.toml` is what makes documentation a
reference:

| Key | Default | What it is |
| --- | --- | --- |
| `modules` | the project `doc.toml` is in | the directory of modules to document |
| `package` | the project's name, each `-` made `_` | the name the modules are imported under; `""` for none |
| `exclude` | `[]` | patterns for files to leave out, relative to `modules` |
| `intro` | none | a Markdown file the front page opens with, in place of the description |
| `groups` | none | parts of the contents, each with a `title`, a `description` and its `modules` |

Without groups, every module is listed in one list, each beneath the one
it belongs to. With them, each group is a part of the contents:

```toml
[[reference.groups]]
title = "Forecasts"
description = "Asking about the weather."
modules = ["weather", "weather.app"]
```

A module no group reaches is listed at the end, under Other Modules, with
a warning, and `--strict` fails over it, so a new module cannot quietly go
missing from the contents. The standard library reference is generated
this way.

## Documentation Inside Documentation

A directory beside `src` that holds a `doc.toml` of its own is
documentation too, and it is built along with the documentation around
it, into the directory of the site with the same name. The reference
`zuri doc generate` writes to `docs/reference` is the usual case: with a
guide in `docs` as well, `zuri doc build` builds both,

```console
$ zuri doc build
weather: 5 pages in 0.39s
  docs/site
weather API Reference: 5 pages in 0.36s
  docs/site/reference
```

and `zuri doc serve` serves both, rebuilding when a page or a module
changes.

The top of every page then links to each of them, and marks the one it
belongs to. The documentation around the others is called Docs, a
reference API Reference, and any other by its title. `label` in a
site's own `[site]` table calls it something else:

```toml
[site]
label = "Guide"
```

What a site inside leaves unset of the logo, favicon, repository,
`[site.home]`, `[[site.links]]`, `css` and `js`, it takes from the one
around it, so the whole reads as one site, and its `url` is the outer
`url` with its directory's name added. Built on its own, as
`zuri doc build docs/reference`, it is a site of its own and takes none
of that.

A link from one to another is written the way it would be on GitHub, as
the path from one Markdown file to the other. From `docs/src/guide.md`,
`[greet](../reference/src/weather-app.md#greet)` reaches the heading for
`greet()` on the reference's page for `weather.app`. The site points the
link at the page that file becomes, and the build checks it with the
rest, naming the documentation it leads into when it leads nowhere.

## Settings

`doc.toml` has five tables, and every key in them is optional. A key a
table does not have, or a value of the wrong type, stops the build with
the key named.

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

`[documentation]` says what the documentation is:

| Key | Default | What it is |
| --- | --- | --- |
| `title` | the directory's name | the title, shown at the top of every page |
| `description` | none | one sentence, for search engines |
| `authors` | none | the authors' names |
| `language` | `"en"` | the language it is written in |
| `source` | `"src"` | the directory of pages |
| `output` | `"site"` | where the site is written |
| `create-missing` | `false` | write an empty page for a chapter whose file is missing, rather than stopping |

`[site]` says how the site looks and where it lives:

| Key | Default | What it is |
| --- | --- | --- |
| `url` | none | the address the site is published at |
| `repository` | none | the source repository, linked from every page |
| `edit` | none | an address for editing a page, with `{path}` for the page's file |
| `logo` | none | an image shown beside the title |
| `favicon` | none | the icon a browser tab shows |
| `theme` | `"system"` | how a page first appears: `"system"`, `"light"` or `"dark"` |
| `fold` | `false` | how many levels of the contents start open; `false` opens every level |
| `numbers` | `true`, or `false` for a reference | whether chapters are numbered |
| `search` | `true` | whether the site can be searched |
| `print` | `true` | whether the site has a page that prints all of it at once |
| `css` | `[]` | stylesheets added to every page |
| `js` | `[]` | scripts added to every page |
| `cname` | none | the domain GitHub Pages serves the site at |
| `label` | `"Docs"`, `"API Reference"` or the title | what the links at the top call this site when it is built with others |

The contents always open as far as the page being read, whatever `fold`
says. `[site.home]` puts a link back to a site above this one beside its
title, and each `[[site.links]]` adds a link to the top of every page; an
address that is not absolute is relative to the front page.
`[markdown]` is described under [Writing Pages](#writing-pages),
`[reference]` under [Generating an API Reference](#generating-an-api-reference),
and `[redirects]` keeps an old address working by sending it on to a new
one.

Every path in the file is relative to the directory holding it.

## Changing the Look

A reader picks light or dark, or follows their system, from the menu at
the top of every page, and the choice is remembered. The `theme` setting
decides only how a page looks before they have chosen.

Every page is written through one Wire template, `page.html`, styled by
`doc.css` and given its behaviour by `doc.js`. A directory called `theme`
beside `doc.toml` replaces any of the three with a file of the same name.
The ones `zuri doc` uses are in `cmds/doc/theme` beside the `zuri`
executable, and a copy of one is the place to start from. The `css` and
`js` settings add to them instead, which is usually all it takes:

```css
:root {
  --measure: 50rem;
}
```

## Publishing

The site is a directory of plain files with nothing to run, so publishing
it is copying it somewhere a web server reads.

GitHub Pages serves it as it is. `zuri doc` writes the `.nojekyll` file
that stops GitHub from leaving out files it does not expect, and a
`CNAME` file when `cname` is set. Set `url` as well: it gives every page
its canonical address, writes a `sitemap.xml` for search engines, and
lets `404.html`, the page a missing address is answered with, work at
any depth of the site rather than only at its top.

Every site can be read without a mouse: `/`, `s`, or Ctrl+K and ⌘K on a
Mac open the search box, the arrow keys move through what it finds, and
the left and right arrow keys turn to the previous and next chapter.
