# Validation and the Domain Model

Every rule about what a task is lives in a single class. Nothing above it
checks a title again, and nothing below it stores a task that breaks a rule.

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

`TaskError` has a `field` as well as a message. That one extra value is what
lets the API respond with `{"error": "...", "field": "title"}`, which in
turn lets a form highlight the input that's wrong. Being able to carry more
than a string is exactly why you'd write a custom error class.

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
constructor sets. Classes are sealed, so this list is everything a task
holds, and writing it out means the next person reading the code can see
that without reading the constructor.

The constructor takes a required `title`, and an options dictionary for
everything else. That's a pattern worth copying: a positional argument for
the value that's always there, and named options for the rest, so you never
see a call like `Task('x', nil, nil, 'todo', nil)`.

It uses `uuid.v7()` instead of `v4()`, because version 7 includes a
timestamp, so IDs sort by age. When an identifier is going to be used as a
key in something ordered, you get that for free.

## Changing a Task

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

`update()` uses `contains()` instead of checking for truthiness. That's the
difference between a partial update that works and one that doesn't. A
`PATCH` body of `{"notes": ""}` means "clear the notes", and
`if changes.notes` would read it as "no change".

Both methods return `self`, so `board.get(id).move_to('done')` reads as a
single action.

## Converting a Task

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

  @to_string() {
    return 'Task(${self.id}, ${self.column}, ${self.title})'
  }
}
```

There are two representations, and that's intentional. `to_dict()` is what
gets stored, and it contains exactly what `from_dict()` needs to rebuild the
task. `to_view()` is what gets displayed, and it adds values that are
calculated rather than stored, such as a formatted date and a boolean that
the template can check.

Keeping them separate means that changing how a task is displayed never
changes the file format.

`@to_json()` lets you pass a `Task` straight to `response.json()`, and
`@to_string()` makes `echo` show something useful when you look at a task
while debugging.

## Loading a Task From Storage

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

These four lines have one important property: **they use the same
constructor as everything else**.

It would have been shorter to assign the fields directly. But because the
constructor is used, a hand-edited `board.json` that contains an unknown
column, or a task without a title, is rejected when the board loads, instead
of becoming a task that nothing can display or fix.

The code uses `data.get(key, nil)` instead of `data.key` throughout, because
a record saved by an older version of the program might not have every key.
`get()` with a fallback turns a missing key into `nil`, which the
constructor's defaults then handle, while `data.column` would raise an
`undefined key` error.

It's also the exact opposite of `to_dict()`. The two methods go together, so
a field added to `Task` has to appear in both, or it won't survive a
restart. That's the kind of bug that only shows up the second time you run
the program.

## Where the Rules Are Defined

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

Look at the order of those three checks, because the order is the whole
point of the function.

**The type check comes first.** A `PATCH` body of `{"title": 42}` reaches
this function as a number, and `42.trim()` would raise a `TypeError` about
methods instead of a `TaskError` about titles. Checking the type first means
the caller gets an error in the application's own terms.

**The value is trimmed before checking whether it's empty**, so `' '` is
rejected. A title made of three spaces isn't a title, and calling
`title.is_empty()` on the raw input would have accepted it.

**The length is checked after trimming**, so trailing whitespace doesn't
count toward the limit.

And the function **returns the cleaned-up value**. It isn't a validator that
answers yes or no. It either returns a valid, normalized value or raises an
error. That's why the constructor can simply write
`self.title = _clean_title(title)`.

```zuri,ignore
def _clean_column(column) {
  if !config.COLUMNS.contains(column) {
    raise TaskError(
      'unknown column "${column}", expected one of ' +
          ', '.join(config.COLUMNS),
      'column',
    )
  }

  return column
}
```

The message says what was wrong **and** what would have been right.
`unknown column "backlog", expected one of todo, doing, done` tells the
caller how to fix the problem, while `invalid column` doesn't.

Notice that the set of valid columns comes from `config.COLUMNS` instead of
being written out here. Adding a column to the board is a one-line change in
one file, and this check, `columns()` and `summary()` all follow it.

### Each Helper Is Called From Two Places

Both helpers are private to the module, because of their leading underscore,
so no other file can use them. Each one is called from exactly two places:
the constructor and `update()`.

That's the property this whole chapter is built on. **There's no way to
create or change a `Task` without going through them.** `from_dict()` uses
the constructor, `move_to()` calls `_clean_column()` itself, and a route
handler can't use the private functions at all.

That means every layer above can assume that any `Task` it has is valid,
which is why none of the route handlers in [The JSON API](ch25-04-api.md)
check a title again.

## Why Not Use the `validate` Module?

Zuri has a schema validator, and for a form with fifteen fields, it's
exactly the right tool:

```zuri
import validate

var schema = validate.schema({
  title: validate.required().string().max_length(120),
  column: validate.required().string(),
})
```

Here, though, the rules are three lines of Zuri that also normalize the
value (with `trim()`), produce a domain error that includes a `field`, and
live right next to the data they apply to. A schema would just be one more
place to look.

Use `validate` when the problem is the shape of the input. Use methods on
the class when the rules are part of what the object *is*.
