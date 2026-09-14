# A Full-Stack Task Board

We are going to build a real web application: a shared task board with
three columns, a browser interface, and a JSON API over the same data.

It is about four hundred lines of Zuri, and it uses almost everything this
book has covered. Classes and inheritance for the domain model. Custom
errors for validation. The module system for structure. Files and JSON for
persistence. The HTTP server for routing, middleware and static files. Wire
for server-rendered HTML. `os` for configuration and signals. `log` for
output.

No dependencies. Nothing to install. `zuri taskboard` and it runs.

## What It Does

- Three columns: `todo`, `doing`, `done`.
- A page at `/` showing the board, with forms to add, move and delete.
- A JSON API under `/api` doing the same things, for anything that is not
  a browser.
- A `board.json` file holding the state, written atomically.
- One error handler turning domain errors into the right status code, for
  both the HTML and the JSON side.

## How the Chapter Is Organised

Each section builds one layer, from the inside out:

1. [Laying Out the Project](ch21-01-project-layout.md): the directory
   structure and why it is shaped this way.
2. [The Storage Layer](ch21-02-storage.md): the JSON file and the board
   that sits on top of it.
3. [Validation and the Domain Model](ch21-03-domain.md): the `Task` class,
   which owns every rule about what a task is.
4. [The JSON API](ch21-04-api.md): six routes over the board.
5. [Server-Rendered Pages with Wire](ch21-05-pages.md): the templates, the
   forms and the redirect-after-post pattern.
6. [Middleware, Logging and Errors](ch21-06-middleware.md): the two pieces
   of cross-cutting behaviour every request goes through.
7. [Running It for Real](ch21-07-running-it.md): configuration, signals and
   what changes when you want more than one core.
8. [Testing the Board](ch21-08-testing.md): a suite over all three layers,
   and what the layering bought.

Read it in order. Each section assumes the previous one exists.
