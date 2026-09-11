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

<span class="filename">Filename: routes/pages.zu</span>

```zuri
import ..config

/**
 * Registers the page and form routes on `server`.
 *
 * @param HttpServer server
 * @param Board board
 * @param Wire view: the template engine to render through
 */
def register(server, board, view) {

  server.get('/', @(request, response) {
    response.html(view.render('board.html', {
      title: 'Task Board',
      tagline: _tagline(board),
      columns: board.columns(),
      error: request.query_param('error', nil),
    }))
  })

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

def _tagline(board) {
  var counts = board.summary()

  return '${counts.todo} to do, ${counts.doing} in progress, ${counts.done} done'
}

def _escape(message) {
  return message.replace('/[^a-zA-Z0-9 .,-]/', '').replace(' ', '+', false)
}
```

## Redirect After Post

Every form handler ends in a redirect rather than rendering a page. That is
the post/redirect/get pattern, and it exists because a browser that
rendered a page in response to a `POST` will re-submit that `POST` when the
user presses refresh. Adding a task twice because someone hit F5 is not a
bug you want to explain.

The failure path redirects too, carrying the message as a query parameter
that the next `GET` renders into the error banner.

## Why These Handlers Do Catch

The API handlers let errors propagate, because the middleware turns them
into status codes. These catch, because a browser filling in a form does
not want a 422 page; it wants the board back with a message on it.

Same domain errors, two presentations, chosen at the layer that knows which
kind of client it is talking to.

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
