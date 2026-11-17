# The Storage Layer

The storage layer consists of two files. One knows about the disk but
nothing about tasks, and the other knows about tasks but nothing about the
disk.

## The JSON File

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

These fifty lines make four decisions.

**A missing file isn't an error.** On the first run, nothing has been saved
yet, and the program should still start. A file that exists but can't be
parsed *is* an error, because otherwise its contents would be overwritten
with an empty board.

**The error message includes the path.** Whoever reads it is looking at a
single line in a log.

**The write is atomic.** The data is written to a temporary file, which is
then renamed into place. If the process is killed partway through a write,
the previous board is left intact, because a rename within a directory
either happens completely or not at all.

**The file handle is opened explicitly.** `write()` on a closed handle opens
the file, writes to it and closes it again, so two `write()` calls in `w`
mode would each truncate the file. Call `open()` first, then write, and then
call `close()`. This is the rule from [Chapter 9](ch09-00-files.md), and
this is exactly the kind of code where it matters.

## The Board

`storage/index.zu` is the layer above. It holds `Task` objects, answers
questions about them and saves them using `_json_store`, and it's the only
part of the application that knows a file is involved at all.

Instead of reading it as a hundred lines all at once, let's go through it
one piece at a time.

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

This is a custom error class, and it's only four lines long, but it's worth
it. Every layer above can check `instance_of(error, NotFoundError)` instead
of matching against a message string, and the middleware in [Middleware,
Logging and Errors](ch25-06-middleware.md) turns exactly this class into a
404 response. If `get()` raised a plain `Error('not found')`, that check
would have to search the message text.

Notice the two lines inside `@new`. `parent(message)` lets the base
constructor set `message` and record the stack trace, and `self.type` makes
the class name appear in logs and in the message for an uncaught error. Both
follow the pattern from [Chapter 7](ch07-00-error-handling.md).

### Fields and Constructor

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

There are two fields, both declared with `var`, even though `@new` assigns
both of them. The constructor could have declared them itself, but writing
them out means you can see the shape of the class at the top without reading
the constructor, and it becomes necessary as soon as any other method
assigns to them.

The constructor does two things. It works out **where** the file is, and it
loads **what's** in it.

`directory or config.DATA_DIR` is the pattern for optional parameters from
[Chapter 5](ch05-01-defining-functions.md). It's safe here because a
directory can never sensibly be `''` or `0`, so the problem with falsy
defaults doesn't apply to paths.

The `map` on the second line is the boundary between two worlds.
`read_all()` returns plain dictionaries, because that's what JSON contains,
and `from_dict()` turns each one into a real `Task`. **Everything above this
line works with objects, and everything below it works with dictionaries**,
and this is the only place where the two meet.

### Reading From the Board

```zuri,ignore
  all() {
    return self.tasks
  }

  in_column(column: string) {
    return self.tasks.filter(@(task) => task.column == column)
  }
```

`all()` is a single line, and it returns the actual list instead of a copy.
That's intentional, because every caller in this application only reads the
list. If that ever changed, this is the line that would need a `clone()`.

`in_column()` is just a call to `filter()`. It's a method rather than a loop
in each place it's used, so that the question of which column a task is in
is answered in one place.

### Preparing Data for the Page

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

This is the method the HTML template uses, and it makes three decisions
worth pointing out.

**It loops over `config.COLUMNS`, not over the tasks.** That means a column
without any tasks still appears, as an empty column, which is how a board
should look. Grouping by looping over the tasks would silently leave out
empty columns, and would produce the columns in whatever order the data
happened to be in.

**It returns the results of `to_view()`, not `Task` objects.** A view
already contains the formatted date and the `is_done` flag, so the template
doesn't have to calculate them. [Server-Rendered Pages](ch25-05-pages.md)
explains why that's particularly important for templates.

**`{ name, tasks }` uses the shorthand syntax.** Both keys have the same
names as the variables that hold their values, so writing
`{ name: name, tasks: tasks }` would just add noise.

### Finding a Single Task

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

The important decision here is that **`get()` raises an error instead of
returning `nil`**.

That choice affects everything that calls it. Every caller either has a real
task or gets an error, so none of the route handlers need an
`if task == nil` check. If `get()` returned `nil`, that check would have to
appear in five places, and sooner or later one of them would be forgotten,
causing an error when a field is read from `nil` somewhere far away from the
real cause.

The message includes the ID, because whoever reads the log only has the
request to go on.

### Changing the Board

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

All three methods follow the same pattern: **change the list in memory, save
it, and return what changed**.

`update()` and `remove()` both start by calling `get()`, so an invalid ID
raises a `NotFoundError` before anything is changed. There's no way for a
change to be only half applied.

Each method returns the task it affected instead of returning nothing, which
lets the JSON API respond with the created or updated record without looking
it up again.

Using `remove_at(index_of(task))` instead of `remove(task)` is intentional.
The list's `remove()` compares by value, so two tasks with identical fields
would make it ambiguous. Removing by position removes exactly the object
that `get()` found.

### Counting and Saving

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

`summary()` loops over the configured columns for the same reason
`columns()` does: an empty column should be reported as `0`, not left out.

`_save()` is the counterpart to the `map` in the constructor. Objects are
written out as dictionaries using `to_dict()`, just as they were read in
using `from_dict()`. The two methods go together, so if you add a field to
`Task`, it has to appear in both, or it won't survive a restart.

**`_save()` is private**, and that's the most important property of this
class. There's no public method that changes the board without saving it,
because `add`, `update` and `remove` all call `_save()`, and nothing else
can. A caller can't forget to save, because a caller never gets the choice.

### The Trade-Off

The whole board is kept in memory and completely rewritten after every
change.

For a board small enough for a team to read on one screen, that's the right
trade-off. The code is simple enough to keep in your head, and rewriting a
few kilobytes doesn't matter. But it's also the first assumption to revisit
if the board ever needs to hold a hundred thousand tasks, and at that point,
the answer is [a real database](ch17-00-sql.md), not a cleverer file format.

## Concurrency

`Board` keeps the whole file in memory. If two processes wrote to the same
`board.json`, each would overwrite the other's changes, and the atomic
rename doesn't fix that. It guarantees that the file is never half written,
but not that two writers agree.

This application only runs one server process, so the problem doesn't come
up. [Running It for Real](ch25-07-running-it.md) is where it does.
