# Server-Rendered Pages with Wire

The browser side of the application consists of two templates and four
routes. Wire has [its own chapter](ch14-00-wire.md) with the full reference,
and this section uses the parts a real page needs.

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

This is a valid HTML5 document. If you open it in a browser, you get a page
with a heading and a paragraph. That's the whole idea behind Wire's design:
the directives are attributes, so the template still looks like the page it
renders.

`x-slot="content"` declares a region that an extending template can replace.
The content inside it is the default, which is used when nothing replaces
it.

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

Six directives make up the whole page.

**`x-for` with `x-value`** repeats an element once for each item and gives
each item a name. The `x-for="column.tasks"` nested inside `x-for="columns"`
is an ordinary nested loop, and the inner loop can still see `columns` from
the outer one, which is how the dropdown for moving a task lists every
column.

**`x-text`** replaces an element's children with escaped text, so nothing in
a task's title can turn into markup.

**`x-attr`** takes a dictionary and adds its entries to the element as
attributes, which is how a form's `action` is built from a task's ID.

**`x-if` and `x-not`** render an element only under a condition.
`x-not="column.tasks"` shows the "Nothing here." line when the list is
empty, because an empty list is falsy in Wire. That's not Zuri's rule, since
`[]` is truthy in Zuri, but Wire uses its own rule, which is more convenient
for templates.

**`{{ column.tasks|length }}`** is an interpolation with a filter. `length`
is one of Wire's built-in filters, and it works on strings, lists,
dictionaries and bytes.

## The Routes

`routes/pages.zu` registers four routes: one that renders the board, and
three that handle its forms. Let's go through it one piece at a time.

### The Structure of the File

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

The whole file is a single `register()` function that takes its dependencies
as arguments. Nothing here uses a global board or a global template engine,
which is what makes the routes testable. Pass `register()` a board that uses
a temporary directory, and the same code runs against it.

This is the "pass dependencies in" habit from
[Debugging](ch24-00-debugging.md), applied to the layer where it costs
nothing and helps the most.

### Rendering the Board

```zuri,ignore
server.get('/', @(request, response) {
  response.html(
    view.render('board.html', {
      title: 'Task Board',
      tagline: _tagline(board),
      columns: board.columns(),
      error: request.query_param('error', nil),
    }),
  )
})
```

This route makes one call and contains no logic. Everything it passes to the
template is either a constant or the result of a method call on the board.
There's no loop, no formatting and no branching in this handler, because all
of that has already been done somewhere better suited to it.

`board.columns()` does the grouping and `_tagline()` does the counting,
while `to_view()`, in the domain model, has already formatted the dates. By
the time the template runs, every value it needs is ready.

`request.query_param('error', nil)` is the other half of the redirect
pattern described below. When a form fails, it redirects with `?error=...`,
and this is where the message comes back to be displayed.

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

All three form handlers follow exactly this structure, so it's worth reading
carefully once.

`request.form()` parses a URL-encoded body into a dictionary. The code uses
`form.get('title', nil)` instead of `form.title`, because a browser can send
a body with any fields at all, or none, and `form.title` would raise an
`undefined key` error for a request that simply left it out. With `get()`, a
missing field becomes `nil`, and `_clean_title()` turns that into a proper
`TaskError`.

The `catch` wraps **only** the call to the board. `response.redirect('/')`,
on the success path, is outside it, so a mistake in the redirect isn't
reported as a validation failure. That's the "keep the catch block small"
rule from [Chapter 7](ch07-00-error-handling.md).

The `return` inside the handler stops execution from continuing to the
redirect on the success path. Without it, a failed add would send two
redirects.

### The Other Two Form Handlers

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

`:id` in the path is a route parameter, which `request.param('id')` reads.

Notice what these handlers **don't** do. Neither one checks that the ID
belongs to a real task, because `board.get()` raises a `NotFoundError` and
the `catch` handles it. Neither one validates the column, because
`_clean_column()` does that. And neither one checks that the task exists
before deleting it, because `remove()` calls `get()` first.

That's the benefit of putting the rules in the domain model. Each route
handler is only four lines long, because there's nothing else left for it to
do.

### The Two Helper Functions

```zuri,ignore
def _tagline(board) {
  var counts = board.summary()

  return '${counts.todo} to do, ${counts.doing} in progress, ${counts.done} done'
}

def _escape(message) {
  return message.replace('/[^a-zA-Z0-9 .,-]/', '').replace(' ', '+', false)
}
```

`_tagline()` turns the board's counts into the line shown under the heading.
It's defined here rather than in `Board` because it's a presentation
decision. A different front end would word it differently, and the board
shouldn't care.

`_escape()` is more careful than it looks. The message is about to be put
into a URL, so it removes everything except letters, digits, spaces and
basic punctuation, and then replaces spaces with `+`.

There are two details in that one line. The first `replace()` uses a
**regular expression**, and the second passes `false` as its third argument
to turn pattern matching **off**, so the single space is matched literally
instead of as a pattern. And removing characters instead of percent-encoding
them is a deliberate choice. This is a message the program generated, not
user input being sent back, so a conservative list of allowed characters is
simpler than encoding, and it can't produce an invalid URL.

## Redirecting After a Post

Every form handler ends with a redirect instead of rendering a page. This is
the **post/redirect/get** pattern. It exists because if a browser displays a
page in response to a `POST`, it submits the `POST` again when the user
refreshes the page. Adding a task twice because someone pressed F5 isn't a
bug you want to have to explain.

The failure path redirects too, passing the message as a query parameter,
which the next `GET` displays in the error banner. Either way, the browser
ends up on a plain `GET /`, which is safe to refresh, bookmark or go back
to.

## Why These Handlers Catch Errors and the API's Handlers Don't

The API handlers in [The JSON API](ch25-04-api.md) let errors propagate,
because the middleware turns them into status codes. These handlers catch
them, because a browser submitting a form doesn't want a 422 page. It wants
the board back, with a message on it.

The same domain errors are presented in two different ways, and the choice
is made by the layer that knows what kind of client it's talking to. Neither
`Board` nor `Task` needs to know that a browser is involved.

## The Stylesheet

`server.serve_files('/static', config.STATIC_DIR)` serves the directory. The
static file handler takes care of content types, `ETag` headers, conditional
requests and range requests by itself, so `app.css` is just a plain file:

```console
$ curl -s -o /dev/null -w '%{http_code} %{content_type}\n' localhost:8000/static/app.css
200 text/css
```

## The Result

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

The result is HTML rendered on the server, without any JavaScript, and with
every value escaped by the template engine rather than by whoever wrote the
template.
