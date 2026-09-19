# The Storage Layer

Two files. One knows about the disk and nothing about tasks. The other
knows about tasks and nothing about the disk.

## The File

<span class="filename">Filename: storage/_json_store.zu</span>

```zuri
import json
import os

/**
 * Reads every stored task dictionary.
 *
 * A missing file is an empty board, not an error: a fresh install has
 * nothing saved yet and should start cleanly. A file that exists but
 * cannot be parsed IS an error, because silently discarding someone's
 * data is worse than refusing to start.
 *
 * @param string path
 * @returns list[dict]
 * @throws Error if the file exists and does not contain a JSON list.
 */
def read_all(path: string) {
  var handle = file(path)

  if !handle.exists() {
    return []
  }

  var decoded

  catch {
    decoded = json.decode(handle.read())
  } as error {
    raise Error('${path} is not readable as JSON: ' + error.message)
  }

  if !is_list(decoded) {
    raise Error('${path} should contain a JSON list, found ' + typeof(decoded))
  }

  return decoded
}

/**
 * Replaces the stored board with `records`.
 *
 * Writes to a temporary file beside the target and renames it into
 * place, so a write interrupted halfway leaves the previous board
 * intact rather than a truncated file.
 *
 * @param string path
 * @param list records
 */
def write_all(path: string, records: list) {
  var directory = os.dir_name(path)

  if !os.dir_exists(directory) {
    os.create_dir(directory, nil, true)
  }

  var temporary = path + '.tmp'
  var handle = file(temporary, 'w')

  handle.open()
  handle.write(json.encode(records, false))
  handle.close()

  file(temporary).rename(path)
}
```

Four decisions in fifty lines.

**A missing file is not an error.** A first run has nothing saved, and the
program should start. A file that exists but cannot be parsed *is* an
error, because the alternative is overwriting whatever was in there with an
empty board.

**The error message names the path.** The person reading it is looking at
one line of a log.

**The write is atomic.** Writing into a temporary file and renaming it into
place means a process killed mid-write leaves the previous board intact,
because a rename within a directory either happens or does not.

**The handle is opened explicitly.** `write()` on a closed handle opens,
writes and closes again, so two `write()` calls in `w` mode would each
truncate the file. `open()` first, then write, then `close()`. That is the
rule from [Chapter 9](ch09-00-files.md), and this is exactly where it
bites.

## The Board

`storage/index.zu` is the layer above. It holds `Task` objects, answers
questions about them, and persists through `_json_store` — and it is the
only thing in the application that knows a file is involved at all.

Rather than read it as one hundred lines, here it is a piece at a time.

### The Error It Raises

<span class="filename">Filename: storage/index.zu</span>

```zuri,ignore
import os

import ..config
import ..models { Task, from_dict }
import ._json_store

/**
 * Raised when a task id does not name a task on the board.
 */
class NotFoundError < Error {

  @new(message) {
    parent(message)
    self.type = 'NotFoundError'
  }
}
```

A custom error class, four lines, and it earns them. Every layer above can
say `instance_of(error, NotFoundError)` instead of matching on a message
string, and the middleware in
[Middleware, Logging and Errors](ch25-06-middleware.md) turns exactly this
class into a 404. Had `get()` raised a plain `Error('not found')`, that
mapping would be a substring search.

Note the two lines inside `@new`. `parent(message)` lets the base
constructor set `message` and capture the stack trace; `self.type` is what
makes the class name show up in logs and in the uncaught-error banner.
Both are the pattern from [Chapter 7](ch07-00-error-handling.md).

### The Fields and the Constructor

```zuri,ignore
/**
 * A board backed by a JSON file.
 */
class Board {

  /** Every task, newest last. */
  var tasks = []

  /** The file this board reads from and writes to. */
  var path

  /**
   * @param ?string directory: defaults to `config.DATA_DIR`
   */
  @new(directory) {
    self.path = os.join_paths(directory or config.DATA_DIR, 'board.json')
    self.tasks = _json_store.read_all(self.path).map(@(record) => from_dict(record))
  }
```

Two fields, declared with `var` even though `@new` assigns both. The
constructor could have declared them implicitly; writing them out means the
class's shape is visible at the top without reading the constructor, and it
is required the moment any other method assigns to them.

The constructor does two things and no more. It works out **where** the
file is, and it loads **what** is in it.

`directory or config.DATA_DIR` is the optional-parameter idiom from
[Chapter 5](ch05-01-defining-functions.md). It is safe here precisely
because a directory is never legitimately `''` or `0` — the falsy-default
trap does not apply to paths.

The `map` on the second line is the boundary between two worlds.
`read_all()` returns plain dictionaries, because that is what JSON is;
`from_dict()` turns each into a real `Task`. **Everything above this line
deals in objects, everything below it deals in dictionaries**, and this is
the single place they meet.

### Reading the Board

```zuri,ignore
  all() {
    return self.tasks
  }

  in_column(column: string) {
    return self.tasks.filter(@(task) => task.column == column)
  }
```

`all()` is a one-liner, and it hands back the real list rather than a copy
— deliberate, because every caller in this application only reads it. If
that changed, this is the line that would need `clone()`.

`in_column()` is `filter()` and nothing else. It is a method rather than a
loop at each call site so that "which column is this task in" is decided in
one place.

### Shaping It for the Page

```zuri,ignore
  /**
   * The board grouped for display: one entry per configured column,
   * in board order, each with its own tasks.
   *
   * @returns list[dict]: `{ name, tasks }`
   */
  columns() {
    return config.COLUMNS.map(@(name) {
      var tasks = self.in_column(name).map(@(task) => task.to_view())

      return { name, tasks }
    })
  }
```

This is the method the HTML template consumes, and three decisions inside
it are worth naming.

**It iterates `config.COLUMNS`, not the tasks.** That means a column with
no tasks still appears, as an empty column — which is what a board should
look like. Grouping by walking the tasks instead would silently drop empty
columns and produce them in whatever order the data happened to be in.

**It returns `to_view()` results, not `Task` objects.** A view carries the
formatted date and the `is_done` flag already computed, so the template
never has to. [Server-Rendered Pages](ch25-05-pages.md) explains why that
matters for templates specifically.

**`{ name, tasks }` uses the shorthand.** Both keys match the variables
holding them, so writing `{ name: name, tasks: tasks }` would be noise.

### Finding One

```zuri,ignore
  /**
   * @throws NotFoundError if no task has that id.
   */
  get(id: string) {
    var found = self.tasks.find(@(task) => task.id == id)

    if found == nil {
      raise NotFoundError('no task with id ${id}')
    }

    return found
  }
```

The important decision is that **`get()` raises rather than returning
`nil`**.

That choice propagates. Every caller either has a real task or has an
error, so no route handler contains `if task == nil`. The alternative —
returning `nil` — would put that check in five places and guarantee one of
them was eventually forgotten, producing a `nil` field access somewhere far
away from the cause.

The message includes the id, because the person reading the log has the
request and nothing else.

### Changing It

```zuri,ignore
  add(title, options) {
    var task = Task(title, options)

    self.tasks.append(task)
    self._save()

    return task
  }

  update(id: string, changes: dict) {
    var task = self.get(id).update(changes)

    self._save()

    return task
  }

  remove(id: string) {
    var task = self.get(id)

    self.tasks.remove_at(self.tasks.index_of(task))
    self._save()

    return task
  }
```

All three follow one shape: **change the in-memory list, then save, then
return what changed**.

`update()` and `remove()` both start by calling `get()`, which means a bad
id raises `NotFoundError` before anything is modified. There is no path
that half-applies a change.

Each returns the affected task rather than nothing, which is what lets the
JSON API answer with the created or updated record without a second
lookup.

`remove_at(index_of(task))` rather than `remove(task)` is deliberate: list
`remove()` compares by value, and two tasks with identical fields would
make that ambiguous. Removing by position removes exactly the object
`get()` found.

### Counting, and Saving

```zuri,ignore
  summary() {
    var counts = {}

    for name in config.COLUMNS {
      counts.set(name, self.in_column(name).length())
    }

    return counts
  }

  _save() {
    _json_store.write_all(self.path, self.tasks.map(@(task) => task.to_dict()))
  }
}
```

`summary()` walks the configured columns for the same reason `columns()`
does: an empty column should report `0`, not be missing.

`_save()` is the other side of the constructor's `map`. Objects go out as
dictionaries via `to_dict()`, exactly as they came in through
`from_dict()`. The two are a matched pair, and a field added to `Task`
needs to appear in both or it will not survive a restart.

**`_save()` is private**, and that is the class's most important property.
There is no public method that changes the board without persisting,
because `add`, `update` and `remove` each call it and nothing else can. A
caller cannot forget to save, because a caller is never given the choice.

### The Trade It Makes

The whole board is held in memory and rewritten in full after every change.

For a board a team can read on one screen, that is the right trade: the
code is simple enough to hold in your head, and a full rewrite of a few
kilobytes is immaterial. It is also the first assumption to revisit if this
ever has to hold a hundred thousand tasks, at which point the answer is
[a real database](ch17-00-sql.md) rather than a cleverer file format.

## Concurrency

`Board` holds the whole file in memory. Two processes writing the same
`board.json` would each overwrite the other's changes, and the atomic
rename does not fix that: it guarantees the file is never half-written, not
that two writers agree.

This application runs one server process, so the question does not arise.
[Running It for Real](ch25-07-running-it.md) is where it does.
