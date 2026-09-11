# The JSON API

Six routes. Every one of them reads or writes through the board and returns
JSON, and not one of them handles an error.

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

  server.get('/api/tasks', @(request, response) {
    var column = request.query_param('column', nil)
    var tasks = column == nil ? board.all() : board.in_column(column)

    response.json({
      tasks: tasks.map(@(task) => task.to_view()),
      summary: board.summary(),
    })
  })

  server.get('/api/tasks/:id', @(request, response) {
    response.json(board.get(request.param('id')).to_view())
  })

  server.post('/api/tasks', @(request, response) {
    var body = request.json_body() or {}

    var task = board.add(body.get('title', nil), {
      notes: body.get('notes', nil),
      column: body.get('column', nil),
    })

    response.json(task.to_view(), 201)
  })

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

## A Function, Not a Module Body

`register(server, board)` takes the server and the board rather than
importing them. That is what lets the same routes run against a different
board, and it is what keeps `routes/api.zu` free of any decision about
where the data lives.

## Handlers Do Not Handle Errors

`board.get()` raises `NotFoundError` for an unknown id. `board.add()`
raises `TaskError` for a bad title. Neither handler catches either, and
both are one line as a result.

The middleware in [the next section but one](ch20-06-middleware.md) catches
both and turns them into a 404 and a 422. There is exactly one place in the
application that knows which domain error means which status code, and it
is not in a route.

## `json_body()`, and the `or {}`

`request.json_body()` returns `nil` for a request with no body or a body
that is not JSON. `or {}` turns that into an empty dictionary, so
`body.get('title', nil)` works either way and the missing title becomes the
model's problem, which is where the message about it lives.

## `to_view()` at the Boundary

Every response sends `to_view()`, not the `Task`. The API's shape is
therefore a decision `Task` makes in one method, rather than something that
leaks out of however the object happens to be laid out. Adding a private
field to `Task` tomorrow does not change what the API returns.

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
