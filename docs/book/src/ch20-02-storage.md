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

<span class="filename">Filename: storage/index.zu</span>

```zuri
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

  all() {
    return self.tasks
  }

  in_column(column: string) {
    return self.tasks.filter(@(task) => task.column == column)
  }

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

The whole board is held in memory and written out in full after every
change. For a board a team can read on one screen that is the right trade,
and it is the first assumption to revisit if this ever has to hold a
hundred thousand tasks.

Three things are deliberate.

**`_save()` is private.** Callers change the board through `add`, `update`
and `remove`, each of which saves. There is no way to mutate without
persisting, because there is no method that does one without the other.

**`get()` raises rather than returning `nil`.** Every caller of `get()`
either has a task or has an error, so no handler has to check. That
`NotFoundError` is what the middleware turns into a 404.

**`columns()` returns views, not tasks.** `to_view()` adds the formatted
date and the `is_done` flag the template wants, so the template never
computes anything.

## Concurrency

`Board` holds the whole file in memory. Two processes writing the same
`board.json` would each overwrite the other's changes, and the atomic
rename does not fix that: it guarantees the file is never half-written, not
that two writers agree.

This application runs one server process, so the question does not arise.
[Running It for Real](ch20-07-running-it.md) is where it does.
