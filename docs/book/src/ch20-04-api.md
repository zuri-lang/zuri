# The JSON API

Six routes. Every one of them reads or writes through the board and returns
JSON, and not one of them handles an error.

## The Routes, One at a Time

### The Shape of the File

<span class="filename">Filename: routes/api.zu</span>

```zuri,ignore
import ..models { TaskError }
import ..storage { NotFoundError }

/**
 * Registers every `/api` route on `server`, reading and writing
 * through `board`.
 *
 * @param HttpServer server
 * @param Board board
 */
def register(server, board) {
```

`register(server, board)` takes the server and the board rather than
importing them. That is what lets the same routes run against a different
board — a temporary one in a test, a seeded one in a demo — and it keeps
this file free of any decision about where the data lives.

### Listing, With an Optional Filter

```zuri,ignore
  server.get('/api/tasks', @(request, response) {
    var column = request.query_param('column', nil)
    var tasks = column == nil ? board.all() : board.in_column(column)

    response.json({
      tasks: tasks.map(@(task) => task.to_view()),
      summary: board.summary(),
    })
  })
```

One route serving two questions: every task, or every task in one column.

`query_param('column', nil)` supplies the fallback, and the test is
`column == nil` rather than `if column`. That distinction matters: a query
string of `?column=` produces an empty string, which is falsy but *present*
— and an empty column name should be rejected by `in_column()`, not
silently treated as "no filter".

The response carries `summary` alongside `tasks` because a client
rendering a board wants both, and making it ask twice would be two requests
for one screen.

### Fetching One

```zuri,ignore
  server.get('/api/tasks/:id', @(request, response) {
    response.json(board.get(request.param('id')).to_view())
  })
```

One line, and it can fail. `board.get()` raises `NotFoundError` for an
unknown id, and this handler does nothing about it — which is the subject
of the section below.

### Creating

```zuri,ignore
  server.post('/api/tasks', @(request, response) {
    var body = request.json_body() or {}

    var task = board.add(body.get('title', nil), {
      notes: body.get('notes', nil),
      column: body.get('column', nil),
    })

    response.json(task.to_view(), 201)
  })
```

`request.json_body()` returns `nil` for a request with no body, or a body
that is not JSON at all. `or {}` turns that into an empty dictionary, so
`body.get('title', nil)` works either way and a missing title becomes the
*model's* problem — which is where the message about it already lives.

The `201` is the second argument to `response.json()`. A created resource
is not a `200`, and saying so is one character of effort.

`task.to_view()` rather than `task`, for the reason below.

### Updating and Deleting

```zuri,ignore
  server.patch('/api/tasks/:id', @(request, response) {
    var body = request.json_body() or {}

    response.json(board.update(request.param('id'), body).to_view())
  })

  server.delete('/api/tasks/:id', @(request, response) {
    board.remove(request.param('id'))
    response.json({ deleted: true })
  })

  server.get('/api/summary', @(request, response) {
    response.json(board.summary())
  })
}
```

The `PATCH` handler passes the decoded body straight to `board.update()`
with no filtering. That is safe because `update()` only looks at the three
keys it knows — `title`, `notes`, `column` — using `contains()`, so a body
containing `{"id": "hacked"}` changes nothing. The whitelist lives in the
model, once, rather than in every route that accepts a body.

`DELETE` returns a body rather than a bare `204`, because a client that
parses every response as JSON should not have to special-case one route.

## Handlers Do Not Handle Errors

This is the chapter's real point, and it is easiest to see by counting: six
handlers, zero `catch` blocks.

`board.get()` raises `NotFoundError` for an unknown id. `board.add()` and
`board.update()` raise `TaskError` for a bad title or an unknown column.
Not one handler catches either, and every one of them is one to five lines
as a direct result.

The middleware in [the next section](ch20-06-middleware.md) catches both
and turns them into a 404 and a 422. **There is exactly one place in this
application that knows which domain error means which status code**, and it
is not in a route.

Consider the alternative for a moment. Six handlers each wrapping their
board call in a `catch`, each deciding on a status, each formatting an
error body — thirty-odd lines of duplication, and a seventh route added
next month that gets one of them subtly wrong.

Note also the two imports at the top of the file. `TaskError` and
`NotFoundError` are imported and never mentioned again in this file; they
are there because the module that catches them needs them re-exported
through `routes`. That is the module system doing its job, and
[Chapter 8](ch08-00-modules.md) covers why the `@` matters there.

## `to_view()` at the Boundary

Every response sends `to_view()`, never the `Task` itself.

That single habit means the API's shape is a decision `Task` makes in one
method, rather than something that leaks out of however the object happens
to be laid out. Add a private field to `Task` tomorrow and the API returns
exactly what it returned yesterday.

It is also why the responses carry `created_on` and `is_done`, which are
not stored anywhere — `to_view()` computes them, so every client gets a
formatted date and a boolean without doing the work itself.

## The Routes in Practice

```console
$ curl -s localhost:8000/api/tasks -H 'content-type: application/json' \
    -d '{"title":"write the capstone","notes":"chapter 17"}'
{"id":"01a08e15-e758-741d-b368-d5a988cb90cd","title":"write the capstone",
 "notes":"chapter 17","column":"todo","created_at":1789090195.9,
 "created_on":"Sep 11, 2026","is_done":false}

$ curl -s localhost:8000/api/tasks
{"tasks":[...],"summary":{"todo":1,"doing":0,"done":0}}

$ curl -s -X PATCH localhost:8000/api/tasks/01a08e15-... \
    -H 'content-type: application/json' -d '{"column":"doing"}'
{"id":"01a08e15-...","column":"doing",...}

$ curl -s localhost:8000/api/tasks?column=doing
{"tasks":[...],"summary":{"todo":0,"doing":1,"done":0}}

$ curl -s localhost:8000/api/tasks -H 'content-type: application/json' -d '{"title":""}'
{"error":"a task needs a title","field":"title"}

$ curl -s localhost:8000/api/tasks/does-not-exist
{"error":"no task with id does-not-exist"}
```

The last two are the interesting ones. A 422 with a `field`, and a 404 with
a message, from handlers that said nothing at all about status codes.
