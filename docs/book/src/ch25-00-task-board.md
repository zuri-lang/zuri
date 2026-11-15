# A Full-Stack Task Board

In this chapter, we'll build a real web application: a shared task board
with three columns, a browser interface and a JSON API over the same data.

It's about four hundred lines of Zuri, and it uses almost everything this
book has covered: classes and inheritance for the domain model, custom
errors for validation, the module system for structure, files and JSON for
storage, the HTTP server for routing, middleware and static files, Wire for
HTML rendered on the server, `os` for configuration and signals, and `log`
for output.

There are no dependencies and nothing to install. Run `zuri run taskboard`
and it starts.

## What the Application Does

- It has three columns: `todo`, `doing` and `done`.
- A page at `/` shows the board, with forms for adding, moving and deleting
  tasks.
- A JSON API under `/api` does the same things, for clients other than
  browsers.
- A `board.json` file stores the state, and is written atomically.
- A single error handler turns domain errors into the right status code, for
  both the HTML pages and the JSON API.

## How This Chapter Is Organized

Each section builds one layer, starting from the inside:

1. [Laying Out the Project](ch25-01-project-layout.md): the directory
   structure, and the reasons for it.
2. [The Storage Layer](ch25-02-storage.md): the JSON file, and the board
   built on top of it.
3. [Validation and the Domain Model](ch25-03-domain.md): the `Task` class,
   which owns every rule about what a task is.
4. [The JSON API](ch25-04-api.md): six routes for working with the board.
5. [Server-Rendered Pages with Wire](ch25-05-pages.md): the templates, the
   forms and the redirect-after-post pattern.
6. [Middleware, Logging and Errors](ch25-06-middleware.md): the two pieces
   of shared behavior that every request goes through.
7. [Running It for Real](ch25-07-running-it.md): configuration, signals, and
   what changes when you want to use more than one core.
8. [Testing the Board](ch25-08-testing.md): a test suite covering all three
   layers, and what the layered design makes possible.

Read the sections in order, because each one builds on the one before it.
