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
    task.zu             one file per layer
    board.zu
    api.zu
```

Four design decisions are worth explaining, because they're what keep the
rest of the code short.

First, here's the overall structure. Every dependency in this application
points in the same direction:

```text
index.zu  ->  app.zu  ->  routes/  ->  storage/  ->  models/
```

`routes` depends on `storage`, `storage` depends on `models`, and `models`
depends only on `config`. Nothing depends on anything in the other
direction, which means you can read each layer on its own, and test it
without the layers above it. [Testing the Board](ch25-08-testing.md) is
where that second benefit pays off.

`config.zu` is outside that chain. Every module can use it, and it doesn't
use anything itself. It's the only module that can be used from anywhere,
and that's acceptable because it contains no behavior at all.

## One Package for Each Responsibility

`models`, `storage` and `routes` are packages, which means they're
directories with an `index.zu`. The `index.zu` decides what's public:

<span class="filename">Filename: models/index.zu</span>

```zuri,ignore
import @.task { * }
```

That one line re-exports everything `task.zu` declares, so the rest of the
application can write `import .models { Task }` without needing to know that
`task.zu` exists. If you later split `task.zu` into two files, you only need
to change that one line.

The `@` is what makes it a re-export rather than a private import. Without
it, `models/index.zu` could use `Task` itself, but nothing outside could get
to it through `models`. The import would be local, and
`import .models { Task }` elsewhere would fail. [Re-exporting
Imports](ch08-00-modules.md#re-exporting-imports) covers this distinction,
and this is the place where people most often get it wrong.

So the `index.zu` is an intentional list of what a package offers, which you
can edit, rather than an accident of which files happen to exist.

## The Underscore Matters

`storage/_json_store.zu` starts with an underscore, so no module outside
`storage` can import it, and the compiler enforces that:

```console
SyntaxError: Cannot import private items from module
```

The point isn't to keep anything secret. It's that if you replace the JSON
file with a database, you only have to rewrite one file, and the compiler
**guarantees** that nothing else went around the `Board` to use it directly.
It's not that you checked and nothing does. Nothing can, because any attempt
fails to compile.

That guarantee is what turns a convention into a real boundary. A comment
saying "internal, don't use" is only a suggestion, but a leading underscore
is enforced.

## `app.zu` and `index.zu` Are Separate Files

`app.zu` builds a fully configured server and returns it. `index.zu` starts
it:

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

There are three things going on in that short file.

**`if __root__ == __file__` does most of the work.** `zuri run taskboard`
runs the directory, which runs `index.zu`, and the two values are equal, so
the body runs and the server starts. If another program uses
`import .taskboard`, the two values are different, because `__root__` is
that program's entry file. So nothing starts, and the other program can use
`app.build()` however it likes. One file handles both cases, and [Chapter
8](ch08-00-modules.md) explains this pattern.

**The signal handler is installed before the server starts.** `listen()`
doesn't return, so anything that needs to happen when the program stops has
to be set up beforehand. `os.on_signal('INT', ...)` is what makes Ctrl+C
shut the program down cleanly, instead of killing the process.

**The log message comes before `listen()`**, so the address is printed as
soon as the process is ready, not after it stops. It's only one line, but it
means you know the server started instead of having to guess.

Notice what `index.zu` *doesn't* do. It doesn't build or configure anything,
and it doesn't know anything about boards, templates or routes. All of those
decisions are in `app.zu`, which is why [Middleware, Logging and
Errors](ch25-06-middleware.md) can go through a single function and cover
how the whole application is put together.

## Configuration With Defaults

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

There are four things to notice.

**Every environment variable has a fallback**, so the application runs
without any configuration at all. You can run `git clone` and then
`zuri run taskboard`, and it works. Nobody tries a program that needs six
environment variables before it will start.

**`PORT` is converted with `to_number()`.** Environment variables are always
strings, and a socket won't accept `'8000'` as a port. The conversion
happens here, once, instead of wherever the port is used.

**`HERE` is based on `__file__`, not `os.cwd()`.** The templates and the
stylesheet are stored next to the source code, so whoever runs the program
doesn't need to be in the right directory. This is the rule from [Chapter
9](ch09-00-files.md), and ignoring it causes the most trouble in a web
application, where the program starts and serves a page, but only fails to
find a template when someone requests it.

**`COLUMNS` is defined here** because it's the one piece of information that
the model, the storage layer and the templates all share. `_clean_column()`
validates against it, `Board.columns()` and `Board.summary()` loop over it,
and the template renders one section for each entry. Adding a fourth column
takes a one-line change in this file, and everything else follows.

That's how to decide whether something belongs in `config.zu`. Don't ask "is
it a setting?" Ask **"would changing it otherwise mean editing several files
in the same way?"**
