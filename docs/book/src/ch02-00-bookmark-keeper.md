# Programming a Bookmark Keeper

We are going to build something small and real: a command-line bookmark
keeper. It asks you for a title and a URL, stores what you give it in a
JSON file, and lets you list, search and delete entries later.

By the end of this chapter you will have used variables, functions, lists,
dictionaries, loops, branching, string methods, file handling, JSON and
error handling. We will not explain any of them thoroughly. That is what
Chapters 3 to 8 are for. The goal here is to see a whole program, working,
before we take it apart.

Follow along by typing the code rather than copying it. Mistakes are the
fastest way to learn what the error messages mean.

## Setting Up

```console
$ mkdir bookmarks
$ cd bookmarks
```

Create `main.zu` and start with the modules we will need:

<span class="filename">Filename: main.zu</span>

```zuri
import io
import json

var STORE = 'bookmarks.json'

echo 'Bookmark keeper.'
```

```console
$ zuri run main.zu
Bookmark keeper.
```

Three things to notice. `import` brings in a module and binds it to a name
you then reach through with a dot. `var` declares a variable. A `.zu` file's
top level is just code, running from top to bottom.

## Asking a Question

The `io` module has `readline()`, which prints a prompt and waits for the
user to type a line:

```zuri,ignore
var title = io.readline('Title: ')
echo 'You typed: ' + title
```

```console
$ zuri run main.zu
Title: Zuri docs
You typed: Zuri docs
```

The value that comes back includes whatever the user typed, whitespace
included, so we almost always follow it with `.trim()`:

```zuri,ignore
var title = io.readline('Title: ').trim()
```

`trim()` is a **method** on strings. Every value in Zuri has methods,
numbers, booleans and `nil` included, and you call them with a dot.

## Storing One Bookmark

A bookmark has a title, a URL and some tags. The natural shape for that is
a **dictionary**: a set of keys with values attached.

```zuri
var bookmark = {
  title: 'Zuri docs',
  url: 'https://zuri.dev',
  tags: ['lang', 'docs'],
}
```

Square brackets make a **list**, an ordered sequence. Curly braces make a
dictionary. You read a dictionary's values with a dot, the same way you
reach into a module:

```zuri,ignore
echo bookmark.title
echo bookmark.tags
```

```console
Zuri docs
[lang, docs]
```

Zuri has a shorthand you will use constantly. When the key you want and the
variable holding its value have the same name, write the name once:

```zuri
var title = 'Zuri docs'
var url = 'https://zuri.dev'

var bookmark = { title, url }
```

That is exactly the same dictionary as `{ title: title, url: url }`, with
half the noise.

## Many Bookmarks

One bookmark is a dictionary. Many bookmarks is a list of them:

```zuri
var bookmarks = []

bookmarks.append({ title: 'Zuri docs', url: 'https://zuri.dev' })
bookmarks.append({ title: 'Cranelift', url: 'https://cranelift.dev' })

echo bookmarks.length()
```

```console
2
```

## Saving to Disk

The `json` module turns Zuri values into text and back. `file()` opens a
file; `'w'` means open it for writing.

```zuri
def save(bookmarks) {
  var handle = file(STORE, 'w')
  handle.write(json.encode(bookmarks, false))
  handle.close()
}
```

`def` declares a function. The parameter list needs no types (though it can
have them; see [Chapter 5](ch05-03-type-annotations.md)). The body is a
block, and blocks in Zuri always use braces.

That second argument to `json.encode()` is `compact`. It defaults to `true`,
which gives you one dense line. Passing `false` asks for the indented form,
which is what you want in a file a human might open:

```json
[
  {
    "title": "Cranelift",
    "url": "https://cranelift.dev",
    "tags": [
      "compilers"
    ]
  }
]
```

## Loading From Disk, Carefully

Reading is where a real program has to start thinking about what can go
wrong. The file might not exist yet. It might exist and contain garbage
because someone edited it by hand. Neither should stop the program.

```zuri
def load() {
  var handle = file(STORE)

  if !handle.exists() {
    return []
  }

  catch {
    return json.decode(handle.read())
  } as error {
    echo 'Could not read ${STORE}: ${error.message}'
    return []
  }
}
```

Two new pieces here.

The first is `${...}` inside a string. That is **interpolation**: the
expression between the braces is evaluated and its value spliced into the
string. It works in both single- and double-quoted strings.

The second is `catch`. Zuri does not have `try`. You write `catch { ... }`
around the code that might fail, and `as error { ... }` to handle it. If
nothing goes wrong, the handler never runs. If something does, `error` is
an object with a `message`, a `type` and a `stacktrace`.

Notice there is no `finally`. Zuri does not have one. Code after the
`catch` statement runs either way, which covers most of what `finally` is
used for.

## Adding a Bookmark

Now we can put the pieces together:

```zuri
def add(bookmarks) {
  var title = io.readline('Title: ').trim()
  var url = io.readline('URL:   ').trim()

  if title.is_empty() or url.is_empty() {
    echo 'Both a title and a URL are required.'
    return
  }

  var tags = io.readline('Tags (comma separated): ').trim()

  bookmarks.append({
    title,
    url,
    tags: tags.is_empty() ? [] : tags.split(',').map(@(t) { return t.trim() }),
  })

  save(bookmarks)
  echo 'Saved "${title}".'
}
```

The tags line packs in three ideas.

`cond ? a : b` is the conditional operator: if `cond` is truthy the whole
expression is `a`, otherwise `b`.

`split(',')` cuts a string into a list at every comma.

`map()` takes a function and applies it to every element, giving back a new
list. `@(t) { return t.trim() }` is an **anonymous function**: `@` followed
by a parameter list and a body. You can also spell it `def(t) { ... }`; `@`
is the shorthand and it is what most Zuri code uses for a one-liner.

There is a shorter form still. When the body is a single expression that
you want returned, `=>` replaces the braces and the `return`:

```zuri,ignore
tags.split(',').map(@(t) => t.trim())
```

That is the same function written three ways.
[Chapter 5](ch05-02-closures.md) covers every spelling, and when each one
reads best.

Also notice `or` rather than `||`. Zuri spells its logical operators `and`,
`or` and `!`.

## Printing a Bookmark

```zuri
def show(bookmark, index) {
  echo '${index + 1}. ${bookmark.title}'
  echo '   ${bookmark.url}'

  if !bookmark.tags.is_empty() {
    echo '   [' + ', '.join(bookmark.tags) + ']'
  }
}
```

`join` lives on the string, not on the list, and it reads exactly as it
works: take this separator, and stitch that list together with it.

## Listing and Searching

```zuri
def list_all(bookmarks) {
  if bookmarks.is_empty() {
    echo 'Nothing saved yet.'
    return
  }

  bookmarks.each(@(bookmark, index) {
    show(bookmark, index)
  })
}
```

`each()` calls your function once per element, handing it the **value
first** and the **index second**. That order catches people out; it is the
same for lists, dictionaries and strings.

Search is a filter plus a test:

```zuri
def find(bookmarks) {
  var needle = io.readline('Search: ').trim().lower()

  var hits = bookmarks.filter(@(bookmark) {
    if bookmark.title.lower().contains(needle) {
      return true
    }
    return bookmark.tags.some(@(tag) { return tag.lower() == needle })
  })

  if hits.is_empty() {
    echo 'No match for "${needle}".'
    return
  }

  hits.each(@(bookmark, index) {
    show(bookmark, index)
  })
}
```

`filter()` keeps the elements your function returns `true` for. `some()`
answers "is this true of at least one element?" and stops at the first one
that matches.

## Removing a Bookmark

```zuri
def remove(bookmarks) {
  list_all(bookmarks)

  if bookmarks.is_empty() {
    return
  }

  var answer = io.readline('Remove which number? ').trim()
  var position = answer.to_number() - 1

  if position < 0 or position >= bookmarks.length() {
    echo 'There is no bookmark ${answer}.'
    return
  }

  var gone = bookmarks[position]
  bookmarks.remove_at(position)
  save(bookmarks)
  echo 'Removed "${gone.title}".'
}
```

`to_number()` converts a string; `bookmarks[position]` indexes a list;
`remove_at()` deletes by position and shifts the rest down.

The bounds check is not optional politeness. Indexing a list past its end
raises an error, and an unhandled error ends the program.

## The Main Loop

The last piece is a loop that reads a command and dispatches on it:

```zuri,ignore
def main() {
  var bookmarks = load()

  echo 'Bookmark keeper. ${bookmarks.length()} saved.'

  while true {
    var command = io.readline('\n(a)dd (l)ist (f)ind (r)emove (q)uit > ').trim().lower()

    using command {
      when 'a' add(bookmarks)
      when 'l' list_all(bookmarks)
      when 'f' find(bookmarks)
      when 'r' remove(bookmarks)
      when 'q' {
        echo 'Bye.'
        return
      }
      default {
        echo 'Unknown command "${command}".'
      }
    }
  }
}

main()
```

`using` is Zuri's multi-way branch, and `when` is its branch keyword. It
compares the subject against each `when` value and runs the first match;
`default` catches everything else. A `when` whose body is a single short
statement can stay on one line, which is what makes the dispatch table
above readable. Anything longer gets a block.

Unlike a C `switch`, there is no fall-through and no `break` to remember.
One branch runs, then the statement is over.

## The Whole Program

<span class="filename">Filename: main.zu</span>

```zuri,ignore
import io
import json

var STORE = 'bookmarks.json'

def load() {
  var handle = file(STORE)

  if !handle.exists() {
    return []
  }

  catch {
    return json.decode(handle.read())
  } as error {
    echo 'Could not read ${STORE}: ${error.message}'
    return []
  }
}

def save(bookmarks) {
  var handle = file(STORE, 'w')
  handle.write(json.encode(bookmarks, false))
  handle.close()
}

def add(bookmarks) {
  var title = io.readline('Title: ').trim()
  var url = io.readline('URL:   ').trim()

  if title.is_empty() or url.is_empty() {
    echo 'Both a title and a URL are required.'
    return
  }

  var tags = io.readline('Tags (comma separated): ').trim()

  bookmarks.append({
    title,
    url,
    tags: tags.is_empty() ? [] : tags.split(',').map(@(t) { return t.trim() }),
  })

  save(bookmarks)
  echo 'Saved "${title}".'
}

def show(bookmark, index) {
  echo '${index + 1}. ${bookmark.title}'
  echo '   ${bookmark.url}'

  if !bookmark.tags.is_empty() {
    echo '   [' + ', '.join(bookmark.tags) + ']'
  }
}

def list_all(bookmarks) {
  if bookmarks.is_empty() {
    echo 'Nothing saved yet.'
    return
  }

  bookmarks.each(@(bookmark, index) {
    show(bookmark, index)
  })
}

def find(bookmarks) {
  var needle = io.readline('Search: ').trim().lower()

  var hits = bookmarks.filter(@(bookmark) {
    if bookmark.title.lower().contains(needle) {
      return true
    }
    return bookmark.tags.some(@(tag) { return tag.lower() == needle })
  })

  if hits.is_empty() {
    echo 'No match for "${needle}".'
    return
  }

  hits.each(@(bookmark, index) {
    show(bookmark, index)
  })
}

def remove(bookmarks) {
  list_all(bookmarks)

  if bookmarks.is_empty() {
    return
  }

  var answer = io.readline('Remove which number? ').trim()
  var position = answer.to_number() - 1

  if position < 0 or position >= bookmarks.length() {
    echo 'There is no bookmark ${answer}.'
    return
  }

  var gone = bookmarks[position]
  bookmarks.remove_at(position)
  save(bookmarks)
  echo 'Removed "${gone.title}".'
}

def main() {
  var bookmarks = load()

  echo 'Bookmark keeper. ${bookmarks.length()} saved.'

  while true {
    var command = io.readline('\n(a)dd (l)ist (f)ind (r)emove (q)uit > ').trim().lower()

    using command {
      when 'a' add(bookmarks)
      when 'l' list_all(bookmarks)
      when 'f' find(bookmarks)
      when 'r' remove(bookmarks)
      when 'q' {
        echo 'Bye.'
        return
      }
      default {
        echo 'Unknown command "${command}".'
      }
    }
  }
}

main()
```

A session looks like this:

```console
$ zuri run main.zu
Bookmark keeper. 0 saved.

(a)dd (l)ist (f)ind (r)emove (q)uit > a
Title: Zuri docs
URL:   https://zuri.dev
Tags (comma separated): lang, docs
Saved "Zuri docs".

(a)dd (l)ist (f)ind (r)emove (q)uit > l
1. Zuri docs
   https://zuri.dev
   [lang, docs]

(a)dd (l)ist (f)ind (r)emove (q)uit > q
Bye.
```

## What You Just Learned

You wrote a program with persistent state, user input, error recovery and a
command loop, in about a hundred lines, using nothing that is not in the
box.

Along the way you met: `var`, `def`, `if`/`else`, `while`, `using`/`when`,
`catch`/`as`, `return`, lists, dictionaries, dictionary shorthand, string
interpolation, the conditional operator, anonymous functions, and a handful
of built-in methods.

The next six chapters take every one of those and explain it properly.
