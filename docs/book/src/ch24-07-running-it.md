# Running It for Real

```console
$ zuri taskboard
2026-09-11T02:29:55+01:00 INFO [taskboard]: task board on http://127.0.0.1:8000
```

Open `http://127.0.0.1:8000` and the board is there. Add a task, move it,
delete it. Stop the server with Ctrl+C and start it again, and everything
is still there.

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

`import @.app` re-exports `app`, so another program can
`import .taskboard { app }` and build a server of its own. The
`__root__ == __file__` check means importing this package starts nothing.

`os.on_signal('INT', ...)` catches Ctrl+C. This application saves after
every change, so there is nothing to flush, and the handler is here because
the place to put "finish what you were doing" is obvious once the hook
exists.

## Configuration

Every setting comes from the environment with a default:

```console
$ PORT=9000 HOST=0.0.0.0 DATA_DIR=/var/lib/taskboard zuri taskboard
```

Running it with no environment at all works too, which is what makes the
first run painless.

## Exercising It

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

The HTML form and the JSON API reach the same board through the same
validation, and each reports failure in the form its own client
understands.

## What Is Stored

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

Readable, editable, and greppable. An invalid column typed in by hand is
rejected when the board next loads, because `from_dict()` goes through the
same constructor everything else does.

## The Single-Process Assumption

`server.listen()` serves one connection at a time on one isolate. For a
team board that is more than enough: the slowest thing in a request is
Wire's first compile, and everything after it is single-digit milliseconds.

Zuri can serve across cores, and `http.serve()` is how. It takes a setup
function, calls it once inside each worker with that worker's own server,
and runs the pool:

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
Doing it here would change one thing that matters: **each isolate has its
own heap**, so each worker would have its own `Board`, its own copy of
every task, and its own idea of what `board.json` should contain. Two
workers saving at once would each write a complete file, and one of them
would win.

The board would need to stop being in-memory state. The choices are the
usual ones:

- **A database.** Move `_json_store.zu` to a real store with transactions.
  Nothing above it changes; that is why it is one private file.
- **One owner.** Keep the board in a single isolate and have the workers
  talk to it over a channel. Every mutation becomes a message, and one
  isolate serialises them.
- **A lock.** Guard the file with an advisory lock and re-read before every
  write. Simplest to add, and it makes every request pay for the file.

Which one is right depends on how many people are using the board, and none
of them is worth doing before the answer is "more than this can handle."
The design already leaves the door open, and that is the part to get right
early.

## What This Application Used

Almost all of it.

| Chapter | What the application uses it for |
| --- | --- |
| 3, 4 | every line |
| 5 | anonymous handlers, closures over `board`, typed parameters |
| 6 | `Task`, `Board`, custom errors, `@to_json`, `to_string` |
| 7 | `TaskError`, `NotFoundError`, `catch` at the form boundary |
| 8 | packages, `index.zu`, `@` re-export, `_` privacy, `__file__` |
| 9 | `file()`, atomic rename, `os.join_paths`, `os.get_env` |
| 12 | the HTTP server, routing, middleware, static files |
| 13 | `json`, `uuid`, `date`, `log` |
| 14 | Wire: layout, slots, loops, filters, escaping |
| 16 | the request log, and the error shapes that make it readable |
| 19 | the suite in [Testing the Board](ch24-08-testing.md) |

What it did not use is as informative. There are no isolates, because one
process is enough. There is no `validate`, because the rules belong to the
model. There is no binary handling, no compression, no reflection. Those
are all available, and reaching for them here would have made the
application longer without making it better.

## Where to Take It

The natural next steps, roughly in order of how much they teach:

**Authentication.** `bcrypt` for password hashing, a signed cookie for the
session, a middleware that rejects anything unauthenticated. The middleware
slot is already there.

**Live updates.** `http.sse` pushes an event to the browser when the board
changes, so two people looking at it see each other's edits.

**Per-user boards.** One `board.json` per user, a `Board` cache keyed by
user id, and `path` becoming a parameter rather than a default.

**A database.** Replace `_json_store.zu`, change nothing else, and watch
the underscore earn its keep.

**Search and filtering.** `filter()` over the board, exposed as query
parameters, with the same code serving the HTML page and the API.

Every one of those is a change to one layer. That is what the layout was
for, and [Testing the Board](ch24-08-testing.md) is how you make any of
them without holding your breath.
