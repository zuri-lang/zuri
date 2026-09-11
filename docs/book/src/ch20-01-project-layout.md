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
```

Four decisions are worth explaining, because they are the ones that make
the rest of the code short.

## One Package per Responsibility

`models`, `storage` and `routes` are packages: directories with an
`index.zu`. The `index.zu` decides what is public:

<span class="filename">Filename: models/index.zu</span>

```zuri
import @.task { * }
```

That one line re-exports everything `task.zu` declares, so the rest of the
application writes `import .models { Task }` and never needs to know there
is a `task.zu` at all. Splitting `task.zu` into two files later changes
that one line and nothing else.

## The Underscore Is Load-Bearing

`storage/_json_store.zu` starts with an underscore, so no module outside
`storage` can import it. The compiler enforces that:

```console
SyntaxError: Cannot import private items from module
```

The point is not secrecy. It is that swapping the JSON file for a database
means rewriting one file, and the compiler guarantees nothing else reached
past the `Board` to touch it.

## `app.zu` and `index.zu` Are Separate

`app.zu` builds a fully configured server and returns it. `index.zu`
starts one:

<span class="filename">Filename: index.zu</span>

```zuri
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

`zuri taskboard` runs the directory, finds `index.zu`, and starts serving.
`import .taskboard` from somewhere else gets `app.build()` and starts
nothing, because `__root__` is that other program's entry file and
`__file__` is this one. One file, both jobs.

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

Two things to notice.

Every environment lookup has a fallback, so the application runs with no
configuration at all.

`HERE` is derived from `__file__`, not from `os.cwd()`. The templates and
the stylesheet live next to the source, and the person running the program
is not required to be standing in the right directory.

`COLUMNS` is here because it is the one piece of knowledge the model, the
storage layer and the templates all share. Everything downstream reads it
rather than repeating the list.
