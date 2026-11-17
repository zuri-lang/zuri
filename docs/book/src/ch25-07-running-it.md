# Running It for Real

```console
$ zuri run taskboard
2026-09-11T02:29:55+01:00 INFO [taskboard]: task board on http://127.0.0.1:8000
```

Open `http://127.0.0.1:8000`, and the board is there. Add a task, move it
and delete it. Stop the server with Ctrl+C and start it again, and
everything is still there.

## The Entry Point

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

`import @.app` re-exports `app`, so another program can write
`import .taskboard { app }` and build its own server. Because of the
`__root__ == __file__` check, importing this package doesn't start anything.

`os.on_signal('INT', ...)` handles Ctrl+C. This application saves after
every change, so there's nothing left to write when it stops, but the
handler is here because once the hook exists, it's the obvious place to
finish whatever the program was doing.

## Configuration

Every setting comes from the environment, and has a default:

```console
$ PORT=9000 HOST=0.0.0.0 DATA_DIR=/var/lib/taskboard zuri run taskboard
```

Running it without any environment variables also works, which makes the
first run easy.

## Trying It Out

```console
$ curl -s localhost:8000/health
{"ok":true,"tasks":0}

$ curl -s -X POST -H 'content-type: application/json' \
    -d '{"title":"write the capstone","notes":"chapter 17"}' \
    localhost:8000/api/tasks
{"id":"01a08e15-...","title":"write the capstone","notes":"chapter 17",
 "column":"todo","created_at":1789090195.9,"created_on":"Sep 11, 2026",
 "is_done":false}

$ curl -s -X POST -d 'title=review+it&column=doing' localhost:8000/tasks -o /dev/null -w '%{http_code} -> %{redirect_url}\n'
302 -> http://localhost:8000/

$ curl -s -X POST -d 'title=&column=todo' localhost:8000/tasks -o /dev/null -w '%{redirect_url}\n'
http://localhost:8000/?error=a+task+needs+a+title

$ curl -s localhost:8000/api/summary
{"todo":0,"doing":1,"done":1}
```

The HTML form and the JSON API work with the same board, through the same
validation, and each reports failures in the form its own client
understands.

## What Gets Stored

```console
$ cat taskboard/data/board.json
[
  {
    "id": "01a08e15-e758-741d-b368-d5a988cb90cd",
    "title": "write the capstone",
    "notes": "chapter 17",
    "column": "todo",
    "created_at": 1789090195.9
  }
]
```

The file is easy to read, edit and search. If you type an invalid column
into it by hand, it's rejected the next time the board loads, because
`from_dict()` uses the same constructor as everything else.

## Running on a Single Process

`server.listen()` serves connections on a single isolate. For a team's task
board, that's more than enough. The slowest part of any request is Wire
compiling the templates for the first time, and every request after that
takes a few milliseconds.

Zuri can serve requests on several cores, using `http.serve()`. It takes a
setup function, calls it once inside each worker with that worker's own
server, and runs the pool:

```zuri,ignore
import http
import .app

# In app.zu, alongside build():
#
#   def setup(server) {
#     ...register the same routes and middleware here...
#   }

http.serve(app.setup, { port: 8000, workers: 4 })
```
Doing that here would change one important thing: **each isolate has its own
heap**, so each worker would have its own `Board`, its own copy of every
task, and its own idea of what `board.json` should contain. If two workers
saved at the same time, each would write a complete file, and one of them
would win.

The board would have to stop being kept in memory. These are the usual
options:

- **A database.** Replace `_json_store.zu` with a real store that supports
  transactions. Nothing above it needs to change, which is why it's a single
  private file.
- **A single owner.** Keep the board in one isolate, and have the workers
  send it messages over a channel. Every change becomes a message, and that
  one isolate processes them one at a time.
- **A lock.** Protect the file with an advisory lock, and read it again
  before every write. This is the easiest to add, but every request then has
  to pay the cost of reading the file.

Which one is right depends on how many people use the board, and none of
them is worth doing until the board can't keep up. The design already makes
room for each of them, and that's the part worth getting right early.

## What the Application Used

Almost everything.

| Chapter | What the application uses it for |
| --- | --- |
| 3, 4 | Every line |
| 5 | Anonymous handlers, closures over `board`, typed parameters |
| 6 | `Task`, `Board`, custom errors, `@to_json`, `to_string` |
| 7 | `TaskError`, `NotFoundError`, and a `catch` where forms are handled |
| 8 | Packages, `index.zu`, re-exporting with `@`, privacy with `_`, `__file__` |
| 9 | `file()`, atomic renames, `os.join_paths`, `os.get_env` |
| 13 | `json`, `uuid`, `date`, `log` |
| 14 | Wire: layouts, slots, loops, filters and escaping |
| 15 | The HTTP server, routing, middleware and static files |
| 23 | The test suite in [Testing the Board](ch25-08-testing.md) |
| 24 | The request log, and error responses that make it easy to read |

What it didn't use is just as telling. It doesn't use isolates, because one
process is enough. It doesn't use `validate`, because the rules belong in
the model. And it doesn't use binary data, compression or reflection. All of
those are available, but using them here would have made the application
longer without making it better.

## Next Steps

These are the natural next steps, roughly in order of how much they teach
you:

**Authentication.** Use `bcrypt` to hash passwords, a signed cookie for the
session, and a middleware that rejects any request that isn't authenticated.
There's already a place in the middleware chain for it.

**Live updates.** Use `http.sse` to send an event to the browser whenever
the board changes, so that two people looking at the board see each other's
changes.

**A board for each user.** Store one `board.json` per user, cache `Board`
objects by user ID, and make `path` a parameter instead of a default.

**A database.** Replace `_json_store.zu`, without changing anything else,
and see how much the underscore helps.

**Search and filtering.** Use `filter()` on the board, exposed through query
parameters, with the same code serving both the HTML page and the API.

Each of these is a change to a single layer. That's what the layout was
designed for, and [Testing the Board](ch25-08-testing.md) shows how to make
any of these changes with confidence.
