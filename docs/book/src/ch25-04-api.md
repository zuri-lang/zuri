# The JSON API

The API has six routes. Each one reads from or writes to the board and
returns JSON, and none of them handle errors.

## The Routes

### The Structure of the File

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

`register(server, board)` takes the server and the board as arguments
instead of importing them. That's what lets the same routes work with a
different board, such as a temporary one in a test or one filled with sample
data for a demo, and it keeps any decision about where the data is stored
out of this file.

### Listing Tasks, With an Optional Filter

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

A single route answers two questions: what are all the tasks, and what are
all the tasks in a particular column?

`query_param('column', nil)` provides the fallback, and the check is
`column == nil` instead of `if column`. That distinction matters, because a
query string of `?column=` produces an empty string, which is falsy but
*present*, and an empty column name should be rejected by `in_column()`, not
silently treated as "no filter".

The response includes `summary` alongside `tasks`, because a client that
displays a board needs both, and making it ask separately would mean two
requests for one screen.

### Fetching a Single Task

```zuri,ignore
server.get('/api/tasks/:id', @(request, response) {
  response.json(board.get(request.param('id')).to_view())
})
```

This is a single line, and it can fail. `board.get()` raises a
`NotFoundError` for an unknown ID, and this handler does nothing about it,
for reasons explained in the section below.

### Creating a Task

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

`request.json_body()` returns `nil` for a request with no body, or with a
body that isn't JSON. `or {}` turns that into an empty dictionary, so
`body.get('title', nil)` works either way, and a missing title becomes the
*model's* problem, which is where the message about it already lives.

The `201` is the second argument to `response.json()`. A newly created
resource shouldn't get a `200`, and getting it right only takes one
argument.

The handler returns `task.to_view()` rather than `task`, for the reason
explained below.

### Updating and Deleting Tasks

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
without filtering it. That's safe, because `update()` only looks at the
three keys it knows about, `title`, `notes` and `column`, using
`contains()`. So a body containing `{"id": "hacked"}` doesn't change
anything. The list of allowed keys is kept in the model, once, instead of in
every route that accepts a body.

`DELETE` returns a body instead of an empty `204`, so that a client that
parses every response as JSON doesn't need a special case for one route.

## Why the Handlers Don't Handle Errors

This is the main point of this section, and it's easiest to see if you
count: there are six handlers, and not one `catch` block.

`board.get()` raises a `NotFoundError` for an unknown ID. `board.add()` and
`board.update()` raise a `TaskError` for an invalid title or an unknown
column. None of the handlers catch either error, and that's why each one is
only one to five lines long.

The middleware in [the next section](ch25-06-middleware.md) catches both
errors and turns them into a 404 and a 422. **Only one place in this
application knows which domain error corresponds to which status code**, and
it isn't a route.

Think about the alternative for a moment. Six handlers would each wrap their
call to the board in a `catch`, choose a status code and format an error
response. That's thirty or so lines of duplicated code, and a seventh route
added next month would probably get one of those details slightly wrong.

Notice also the two imports at the top of the file. `TaskError` and
`NotFoundError` are imported but never used again in this file. They're
there because the module that catches them needs them to be re-exported
through `routes`. That's the module system doing its job, and [Chapter
8](ch08-00-modules.md) explains why the `@` matters there.

## Using `to_view()` for Responses

Every response sends the result of `to_view()`, never the `Task` itself.

That one habit means the structure of the API is decided by `Task`, in a
single method, instead of depending on however the object happens to be laid
out internally. If you add a private field to `Task` tomorrow, the API still
returns exactly what it returned yesterday.

It's also why the responses include `created_on` and `is_done`, which aren't
stored anywhere. `to_view()` calculates them, so every client gets a
formatted date and a boolean without having to work them out.

## Trying the API

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

The last two requests are the interesting ones. You get a 422 with a
`field`, and a 404 with a message, from handlers that never mention status
codes at all.
