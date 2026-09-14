# Server-Rendered Pages with Wire

The browser side is two templates and four routes. Wire's full reference is
[its own chapter](ch14-00-wire.md); this section uses the parts a real page
needs.

## The Layout

<span class="filename">Filename: templates/layout.html</span>

```html
<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <title x-text="title">Task Board</title>
    <link rel="stylesheet" href="/static/app.css">
  </head>
  <body>
    <header class="masthead">
      <h1>Task Board</h1>
      <p class="tagline" x-text="tagline"></p>
    </header>

    <main>
      <template x-slot="content">
        <p>Nothing here yet.</p>
      </template>
    </main>

    <footer>
      <p>Served by Zuri.</p>
    </footer>
  </body>
</html>
```

This is a valid HTML5 document. Open it in a browser and you get a page
with a heading and a paragraph. That is the whole point of Wire's design:
the directives are attributes, so the template is still the thing it
renders.

`x-slot="content"` declares a region an extending template may replace. The
content inside it is the default, used when nothing replaces it.

## The Page

<span class="filename">Filename: templates/board.html</span>

```html
<extend base="layout.html">
  <define name="content">
    <form class="new-task" method="post" action="/tasks">
      <input name="title" placeholder="What needs doing?" maxlength="120" required>
      <select name="column">
        <option x-for="columns" x-value="column"
                x-attr="{ value: column.name }" x-text="column.name"></option>
      </select>
      <button type="submit">Add</button>
    </form>

    <p class="error" x-if="error" x-text="error"></p>

    <div class="board">
      <section class="column" x-for="columns" x-value="column">
        <h2>
          <span x-text="column.name"></span>
          <span class="count">{{ column.tasks|length }}</span>
        </h2>

        <p class="empty" x-not="column.tasks">Nothing here.</p>

        <article class="task" x-for="column.tasks" x-value="task">
          <h3 x-text="task.title"></h3>
          <p class="notes" x-if="task.notes" x-text="task.notes"></p>
          <p class="meta">Added <span x-text="task.created_on"></span></p>

          <form method="post" x-attr="{ action: '/tasks/' + task.id + '/move' }">
            <select name="column">
              <option x-for="columns" x-value="target"
                      x-attr="{ value: target.name }" x-text="target.name"></option>
            </select>
            <button type="submit">Move</button>
          </form>

          <form method="post" x-attr="{ action: '/tasks/' + task.id + '/delete' }">
            <button type="submit" class="danger">Delete</button>
          </form>
        </article>
      </section>
    </div>
  </define>
</extend>
```

Six directives carry the whole page.

**`x-for` with `x-value`** repeats an element once per entry, binding each
one to a name. The nested `x-for="column.tasks"` inside
`x-for="columns"` is an ordinary nested loop, and the inner one can still
see `columns` from the outer scope, which is how the move dropdown lists
every column.

**`x-text`** replaces an element's children with escaped text. Nothing a
task's title contains can become markup.

**`x-attr`** takes a dictionary and spreads it onto the element, which is
how a form's `action` gets built from a task id.

**`x-if` / `x-not`** render an element conditionally. `x-not="column.tasks"`
shows the "Nothing here." line when the list is empty, because in Wire an
empty list is falsy. That is not Zuri's rule, where `[]` is truthy; Wire
uses its own, friendlier one for template authoring.

**`{{ column.tasks|length }}`** is an interpolation through a filter.
`length` is one of Wire's built-in filters, and it works on strings, lists,
dictionaries and bytes.

## The Routes

`routes/pages.zu` registers four routes: one that renders the board, and
three that handle the forms on it. Here it is a piece at a time.

### The Shape of the File

<span class="filename">Filename: routes/pages.zu</span>

```zuri,ignore
import ..config

/**
 * Registers the page and form routes on `server`.
 *
 * @param HttpServer server
 * @param Board board
 * @param Wire view: the template engine to render through
 */
def register(server, board, view) {
```

The whole file is one `register()` function that takes its dependencies as
arguments. Nothing here reaches for a global board or a global template
engine, which is what makes the routes testable: hand `register()` a board
backed by a temporary directory and the same code runs against it.

This is the "pass dependencies in" habit from
[Debugging](ch20-00-debugging.md), applied at the layer where it costs
nothing and buys the most.

### Rendering the Board

```zuri,ignore
  server.get('/', @(request, response) {
    response.html(view.render('board.html', {
      title: 'Task Board',
      tagline: _tagline(board),
      columns: board.columns(),
      error: request.query_param('error', nil),
    }))
  })
```

One route, one call, no logic. Everything it hands the template is either a
constant or a method call on the board — there is no loop, no formatting
and no branching in this handler, because all of that already happened
somewhere better suited to it.

`board.columns()` does the grouping. `_tagline()` does the counting.
`to_view()`, back in the domain model, did the date formatting. By the time
the template runs, every value it needs is sitting in front of it.

`request.query_param('error', nil)` is the other half of the redirect
pattern below: a failed form redirects with `?error=...`, and this is where
that message comes back in to be rendered.

### Handling a Form

```zuri,ignore
  server.post('/tasks', @(request, response) {
    var form = request.form()

    catch {
      board.add(form.get('title', nil), { column: form.get('column', nil) })
    } as error {
      response.redirect('/?error=' + _escape(error.message))
      return
    }

    response.redirect('/')
  })
```

All three form handlers follow this exact shape, so it is worth reading
once carefully.

`request.form()` parses a URL-encoded body into a dictionary.
`form.get('title', nil)` rather than `form.title`, because a browser can
post a body with any fields at all — or none — and `form.title` would raise
`undefined key` on a request that simply omitted it. With `get()`, a
missing field arrives as `nil`, and `_clean_title()` turns that into a
proper `TaskError`.

The `catch` wraps **only** the board call. `response.redirect('/')` on the
success path sits outside it, so a mistake in the redirect is not reported
as a validation failure. That is the "keep the catch block small" rule from
[Chapter 7](ch07-00-error-handling.md).

The `return` inside the handler is what stops execution falling through to
the success redirect. Without it, a failed add would issue two redirects.

### The Other Two

```zuri,ignore
  server.post('/tasks/:id/move', @(request, response) {
    catch {
      board.update(request.param('id'), {
        column: request.form().get('column', nil),
      })
    } as error {
      response.redirect('/?error=' + _escape(error.message))
      return
    }

    response.redirect('/')
  })

  server.post('/tasks/:id/delete', @(request, response) {
    catch {
      board.remove(request.param('id'))
    } as error {
      response.redirect('/?error=' + _escape(error.message))
      return
    }

    response.redirect('/')
  })
}
```

`:id` in the path is a route parameter, and `request.param('id')` reads it.

Notice what is **not** in these handlers. Neither checks that the id names
a real task — `board.get()` raises `NotFoundError` and the `catch` picks it
up. Neither validates the column — `_clean_column()` does. Neither checks
that the task exists before deleting it — `remove()` calls `get()` first.

That is the payoff for putting the rules in the domain model. A route
handler is four lines because there is nothing left for it to do.

### The Two Helpers

```zuri,ignore
def _tagline(board) {
  var counts = board.summary()

  return '${counts.todo} to do, ${counts.doing} in progress, ${counts.done} done'
}

def _escape(message) {
  return message.replace('/[^a-zA-Z0-9 .,-]/', '').replace(' ', '+', false)
}
```

`_tagline()` turns the board's counts into the line under the heading. It
lives here rather than in `Board` because it is a presentation decision:
another front end would word it differently, and the board should not have
an opinion.

`_escape()` is doing something more careful than it looks. The message is
about to be put into a URL, so it strips everything that is not a letter,
digit, space or basic punctuation, then turns spaces into `+`.

Two details in that one line. The first `replace()` uses a **regular
expression**; the second passes `false` as the third argument to turn
pattern handling **off**, so the single space is matched literally rather
than as a pattern. And stripping rather than percent-encoding is the
deliberate choice: this is a message we generated, not user input echoed
back, so a conservative allow-list is simpler than encoding and cannot
produce a malformed URL.

## Redirect After Post

Every form handler ends in a redirect rather than rendering a page. That is
the **post/redirect/get** pattern, and it exists because a browser that
rendered a page in response to a `POST` will re-submit that `POST` when the
user presses refresh. Adding a task twice because someone hit F5 is not a
bug you want to explain.

The failure path redirects too, carrying the message as a query parameter
that the next `GET` renders into the error banner. So both outcomes leave
the browser sitting on a plain `GET /`, which is refreshable, bookmarkable
and safe to go back to.

## Why These Handlers Catch and the API's Do Not

The API handlers in [The JSON API](ch21-04-api.md) let errors propagate,
because the middleware turns them into status codes. These catch, because a
browser submitting a form does not want a 422 page — it wants the board
back with a message on it.

Same domain errors, two presentations, and the choice is made at the layer
that knows which kind of client it is talking to. Neither the `Board` nor
the `Task` has to know that a browser is involved.

## The Stylesheet

`server.serve_files('/static', config.STATIC_DIR)` mounts the directory.
The static-file handler deals with content types, `ETag`s, conditional
requests and range requests on its own, so `app.css` is a plain file with
nothing around it:

```console
$ curl -s -o /dev/null -w '%{http_code} %{content_type}\n' localhost:8000/static/app.css
200 text/css
```

## What It Renders

```html
<!DOCTYPE html><html lang="en"><head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <title>Task Board</title>
    <link rel="stylesheet" href="/static/app.css">
  </head>
  <body>
    <header class="masthead">
      <h1>Task Board</h1>
      <p class="tagline">1 to do, 0 in progress, 0 done</p>
    </header>
    ...
    <div class="board">
      <section class="column">
        <h2>
          <span>todo</span>
          <span class="count">1</span>
        </h2>

        <article class="task">
          <h3>write the capstone</h3>
          <p class="notes">chapter 17</p>
          <p class="meta">Added <span>Sep 11, 2026</span></p>

          <form method="post" action="/tasks/01a08e15-e758-741d-b368-d5a988cb90cd/move">
            ...
```

Server-rendered HTML, no JavaScript, and every value escaped by the engine
rather than by the person who wrote the template.
