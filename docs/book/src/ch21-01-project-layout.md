# Laying Out the Project

```text
taskboard/
  index.zu              entry point
  app.zu                wiring: board + templates + server
  config.zu             every setting, with defaults
  models/
    index.zu
    task.zu             the Task class and its rules
  storage/
    index.zu            the Board
    _json_store.zu      private: reading and writing the file
  routes/
    index.zu
    api.zu              the JSON routes
    pages.zu            the HTML routes
  templates/
    layout.html
    board.html
  static/
    app.css
  tests/
    index.zu            `zuri tests` runs the rest
    task.zu             one file per layer
    board.zu
    api.zu
```

Four decisions are worth explaining, because they are the ones that make
the rest of the code short.

Before them, the shape itself. Every arrow in this application points one
way:

```text
index.zu  ->  app.zu  ->  routes/  ->  storage/  ->  models/
```

`routes` knows about `storage`; `storage` knows about `models`; `models`
knows about nothing but `config`. Nothing points back the other way, which
is what makes each layer readable on its own and testable without the ones
above it. [Testing the Board](ch21-08-testing.md) is where that second
half is cashed in.

`config.zu` sits outside that chain — everything may read it, and it reads
nothing. That is the one module allowed to be depended on from anywhere,
and it earns the exemption by containing no behaviour at all.

## One Package per Responsibility

`models`, `storage` and `routes` are packages: directories with an
`index.zu`. The `index.zu` decides what is public:

<span class="filename">Filename: models/index.zu</span>

```zuri,ignore
import @.task { * }
```

That one line re-exports everything `task.zu` declares, so the rest of the
application writes `import .models { Task }` and never needs to know there
is a `task.zu` at all. Splitting `task.zu` into two files later changes
that one line and nothing else.

The `@` is what makes it a re-export rather than a private import. Without
it, `models/index.zu` could use `Task` itself but nothing outside could
reach it through `models` — the import would be local, and
`import .models { Task }` elsewhere would fail. That distinction is the
subject of [Exporting](ch08-00-modules.md#exporting), and this is the
single most common place to get it wrong.

The `index.zu` is therefore a deliberate, editable list of what a package
offers, rather than an accident of which files happen to exist.

## The Underscore Is Load-Bearing

`storage/_json_store.zu` starts with an underscore, so no module outside
`storage` can import it. The compiler enforces that:

```console
SyntaxError: Cannot import private items from module
```

The point is not secrecy. It is that swapping the JSON file for a database
means rewriting one file, and the compiler **guarantees** nothing else
reached past the `Board` to touch it. Not "we checked and nothing does" —
nothing can, and an attempt does not compile.

That guarantee is what turns a convention into a boundary. A comment saying
"internal, do not use" is advice; a leading underscore is enforced.

## `app.zu` and `index.zu` Are Separate

`app.zu` builds a fully configured server and returns it. `index.zu`
starts one:

<span class="filename">Filename: index.zu</span>

```zuri,ignore
import log
import os

import @.app
import .config

if __root__ == __file__ {
  var server = app.build()

  os.on_signal('INT', @() {
    log.info('shutting down')
    os.exit(0)
  })

  log.info('task board on http://${config.HOST}:${config.PORT}')
  server.listen()
}
```

Three things are happening in that short file.

**`if __root__ == __file__` is the whole trick.** `zuri taskboard` runs the
directory, finds `index.zu`, and the two are equal, so the body runs and
the server starts. `import .taskboard` from another program leaves them
different — `__root__` is that program's entry file — so nothing starts,
and the importer gets `app.build()` to use however it likes. One file, both
jobs, and [Chapter 8](ch08-00-modules.md) covers the idiom.

**The signal handler is installed before serving.** `listen()` does not
return, so anything that needs to happen on the way out has to be arranged
first. `os.on_signal('INT', ...)` is what turns Ctrl+C into an orderly exit
rather than a killed process.

**The log line comes before `listen()`**, so the address is printed the
moment the process is ready rather than after it stops. It is one line, and
it is the difference between "did it start?" and knowing.

Note what `index.zu` does *not* do. It builds nothing, configures nothing
and knows nothing about boards, templates or routes. Every one of those
decisions is in `app.zu`, which is why the whole of
[Middleware, Logging and Errors](ch21-06-middleware.md) can walk through
one function and cover the entire assembly.

## Configuration Has Defaults

<span class="filename">Filename: config.zu</span>

```zuri
import os

var HERE = os.dir_name(__file__)

var PORT = os.get_env('PORT', '8000').to_number()
var HOST = os.get_env('HOST', '127.0.0.1')
var DATA_DIR = os.get_env('DATA_DIR', os.join_paths(HERE, 'data'))

var TEMPLATE_DIR = os.join_paths(HERE, 'templates')
var STATIC_DIR = os.join_paths(HERE, 'static')

var COLUMNS = ['todo', 'doing', 'done']
```

Four things to notice.

**Every environment lookup has a fallback**, so the application runs with
no configuration at all. `git clone`, `zuri taskboard`, and it works. A
program that requires six environment variables before it will start is a
program nobody tries.

**`PORT` is converted with `to_number()`.** Environment variables are
always strings, and `'8000'` is not a port a socket will accept. The
conversion is here, once, rather than at the place the port is used.

**`HERE` is derived from `__file__`, not from `os.cwd()`.** The templates
and the stylesheet live next to the source, so the person running the
program is not required to be standing in the right directory. This is the
rule from [Chapter 9](ch09-00-files.md), and a web application is where
ignoring it hurts most: the program starts, serves a page, and fails to
find a template only once someone requests it.

**`COLUMNS` is here** because it is the one piece of knowledge the model,
the storage layer and the templates all share. `_clean_column()` validates
against it, `Board.columns()` and `Board.summary()` iterate it, and the
template renders one section per entry. Adding a fourth column is a
one-line change in this file, and every one of those follows.

That is the test for whether something belongs in `config.zu`: not "is it a
setting", but **"would changing it otherwise mean editing several files
consistently?"**
