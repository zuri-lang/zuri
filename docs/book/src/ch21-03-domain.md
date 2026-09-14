# Validation and the Domain Model

Every rule about what a task is lives in one class. Nothing above it
re-checks a title, and nothing below it stores a task that broke a rule.

<span class="filename">Filename: models/task.zu</span>

```zuri,ignore
import date
import uuid

import ..config

/**
 * Raised when a task cannot be built from the data given.
 */
class TaskError < Error {
  @new(message, field) {
    parent(message)

    self.type = 'TaskError'
    self.field = field
  }
}
```

`TaskError` carries a `field` as well as a message. That one extra value is
what lets the API answer `{"error": "...", "field": "title"}`, which is
what lets a form highlight the input that is wrong. A custom error class
exists precisely so it can carry more than a string.

## The Class

```zuri,ignore
/**
 * A single task on the board.
 */
class Task {

  /** The task's stable identifier, a UUID v7 so ids sort by age. */
  var id

  /** One line describing the work. Required, at most 120 characters. */
  var title

  /** Free-form detail. Optional, defaults to an empty string. */
  var notes = ''

  /** Which column the task sits in. One of `config.COLUMNS`. */
  var column = 'todo'

  /** Unix timestamp, in seconds, of when the task was created. */
  var created_at

  /**
   * @param string title
   * @param ?dict options: `notes`, `column`, `id`, `created_at`
   * @throws TaskError if the title is empty, too long, or the column
   *    is not one this board has.
   */
  @new(title, options) {
    options = options or {}

    self.id = options.get('id', nil) or uuid.v7()
    self.title = _clean_title(title)
    self.notes = options.get('notes', nil) or ''
    self.column = _clean_column(options.get('column', nil) or 'todo')
    self.created_at = options.get('created_at', nil) or time()
  }
```

Every field is declared with `var` and documented, even the ones the
constructor fills in. Classes are sealed, so the list is the whole truth
about what a task holds, and writing it out means the next reader gets that
truth without reading the constructor.

The constructor takes a required `title` and an options dictionary for
everything else. That shape is worth copying: a positional argument for the
thing that is always there, and named options for the rest, so a call site
never reads `Task('x', nil, nil, 'todo', nil)`.

`uuid.v7()` rather than `v4()`, because v7 embeds a timestamp, so ids sort
by age. When your identifier is going to end up as a key in something
ordered, that is free value.

## Mutation

```zuri,ignore
  /**
   * Moves the task to another column.
   *
   * @param string column
   * @returns Task: this task, so calls chain.
   * @throws TaskError if the column is not one this board has.
   */
  move_to(column) {
    self.column = _clean_column(column)
    return self
  }

  /**
   * Applies a partial update. Only the keys present in `changes` are
   * touched; anything absent keeps its current value.
   *
   * @param dict changes: any of `title`, `notes`, `column`
   * @returns Task: this task, so calls chain.
   * @throws TaskError on an invalid title or column.
   */
  update(changes) {
    if changes.contains('title') {
      self.title = _clean_title(changes.title)
    }

    if changes.contains('notes') {
      self.notes = changes.notes or ''
    }

    if changes.contains('column') {
      self.column = _clean_column(changes.column)
    }

    return self
  }
```

`update()` uses `contains()` rather than truthiness. That is the whole
difference between a partial update that works and one that does not: a
`PATCH` body of `{"notes": ""}` means "clear the notes", and
`if changes.notes` would read that as "no change requested".

Both return `self`, so `board.get(id).move_to('done')` reads as one thought.

## Conversion

```zuri,ignore
  /**
   * The task as a plain dictionary, which is what the store writes
   * and what `from_dict()` reads back.
   */
  to_dict() {
    return {
      id: self.id,
      title: self.title,
      notes: self.notes,
      column: self.column,
      created_at: self.created_at,
    }
  }

  /**
   * The same dictionary, plus the derived fields a template or an API
   * client wants and should not have to compute.
   */
  to_view() {
    var view = self.to_dict()

    view.set('created_on', date.from_time(self.created_at).format('M j, Y'))
    view.set('is_done', self.column == 'done')

    return view
  }

  /**
   * What `json.encode()` uses, so a Task can be handed straight to a
   * JSON response with no conversion at the call site.
   */
  @to_json() {
    return self.to_dict()
  }

  to_string() {
    return 'Task(${self.id}, ${self.column}, ${self.title})'
  }
}
```

Two representations, on purpose. `to_dict()` is what gets stored, and it
holds exactly what `from_dict()` needs to rebuild the task. `to_view()` is
what gets displayed, and it adds things that are derived rather than
stored: a formatted date, a boolean the template can branch on.

Keeping them apart means a change to the display format never changes the
file format.

`@to_json()` means a `Task` can be passed straight to `response.json()`.
`to_string()` means `echo` through it during debugging shows something
useful.

## Rebuilding From Storage

```zuri,ignore
/**
 * Rebuilds a task from the dictionary `to_dict()` produced.
 *
 * @param dict data
 * @returns Task
 * @throws TaskError if the stored data is not a valid task.
 */
def from_dict(data: dict) {
  return Task(data.get('title', nil), {
    id: data.get('id', nil),
    notes: data.get('notes', nil),
    column: data.get('column', nil),
    created_at: data.get('created_at', nil),
  })
}
```

This is four lines with one important property: **it goes through the same
constructor as everything else**.

It would have been shorter to assign the fields directly. Doing it this way
means a hand-edited `board.json` containing an unknown column, or a task
with no title, is rejected when the board loads rather than becoming a task
that nothing can render and nothing can fix.

`data.get(key, nil)` rather than `data.key` throughout, because a stored
record written by an older version of the program may be missing a key
entirely. `get()` with a fallback turns that into `nil`, which the
constructor's own defaults then handle; `data.column` would raise
`undefined key`.

It is also the exact inverse of `to_dict()`. The two are a matched pair,
and a field added to `Task` has to appear in both or it will not survive a
restart — the kind of bug that only shows up the second time you run the
program.

## Where the Rules Live

```zuri,ignore
def _clean_title(title) {
  if !is_string(title) {
    raise TaskError('a task needs a title', 'title')
  }

  var cleaned = title.trim()

  if cleaned.is_empty() {
    raise TaskError('a task needs a title', 'title')
  }

  if cleaned.length() > 120 {
    raise TaskError('a title must be 120 characters or fewer', 'title')
  }

  return cleaned
}
```

Read the order of those three checks, because it is the whole function.

**The type check comes first.** A `PATCH` body of `{"title": 42}` reaches
this function as a number, and `42.trim()` would be a `TypeError` about
methods rather than a `TaskError` about titles. Checking first means the
caller gets an error in the application's own vocabulary.

**The trim happens before the emptiness check**, so `'   '` is rejected. A
title of three spaces is not a title, and checking `title.is_empty()` on
the raw input would have accepted it.

**The length check happens after the trim**, so trailing whitespace does
not count against the limit.

And the function **returns the cleaned value**. It is not a validator that
answers yes or no; it is a normaliser that either produces a good value or
raises. That is why the constructor can write `self.title =
_clean_title(title)` with nothing around it.

```zuri,ignore
def _clean_column(column) {
  if !config.COLUMNS.contains(column) {
    raise TaskError(
      'unknown column "${column}", expected one of ' + ', '.join(config.COLUMNS),
      'column'
    )
  }

  return column
}
```

The message names both what was wrong **and** what would have been right.
`unknown column "backlog", expected one of todo, doing, done` tells the
caller how to fix it; `invalid column` does not.

Note that the valid set comes from `config.COLUMNS` rather than being
written out here. Adding a column to the board is a one-line change in one
file, and this check, `columns()`, and `summary()` all follow it.

### Two Call Sites Each

Both helpers are module-private — the leading underscore means no other
file can reach them — and each is called from exactly two places: the
constructor and `update()`.

That is the property the whole chapter is built on. **There is no path into
a `Task` that skips them.** Not `from_dict()`, which goes through the
constructor. Not `move_to()`, which calls `_clean_column()` itself. Not a
route handler, which cannot reach the private function at all.

Every layer above can therefore assume a `Task` it is holding is valid,
which is why no route handler in
[The JSON API](ch21-04-api.md) re-checks a title.

## Why Not the `validate` Module?

Zuri has a schema validator, and for a form with fifteen fields it is
exactly right:

```zuri
import validate

var schema = validate.schema({
  title: validate.required().string().max_length(120),
  column: validate.required().string(),
})
```

Here the rules are three lines of Zuri that also normalise (the `trim()`),
produce a domain error with a `field` on it, and live next to the data they
constrain. A schema would be a second place to look.

Use `validate` when the shape of the input is the problem. Use methods on
the class when the rules are part of what the thing *is*.
