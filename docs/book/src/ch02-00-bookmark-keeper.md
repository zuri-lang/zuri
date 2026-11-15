# Programming a Bookmark Keeper

In this chapter we'll build a small but useful program: a command-line
bookmark keeper. It asks for a title and a URL, saves them to a JSON file,
and lets you list, search and delete bookmarks later.

Along the way, you'll use variables, functions, lists, dictionaries, loops,
branches, string methods, files, JSON and error handling. We won't explain
any of them in depth here, because that's what Chapters 3 to 8 are for. The
goal of this chapter is to show you a complete, working program before we
look at each part in detail.

Try typing the code in yourself instead of copying it. Making mistakes is
the quickest way to learn what the error messages mean.

## Setting Up

```console
$ mkdir bookmarks
$ cd bookmarks
```

Create `main.zu` and start by importing the modules we'll need:

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

There are three things to notice here. `import` loads a module and binds it
to a name, and you use a dot to reach what's inside it. `var` declares a
variable. And the top level of a `.zu` file is ordinary code that runs from
top to bottom.

## Asking for Input

The `io` module has a `readline()` function, which prints a prompt and waits
for the user to type a line:

```zuri,ignore
var title = io.readline('Title: ')
echo 'You typed: ' + title
```

```console
$ zuri run main.zu
Title: Zuri docs
You typed: Zuri docs
```

The returned value contains exactly what the user typed, including any
spaces, so you'll almost always call `.trim()` on it:

```zuri,ignore
var title = io.readline('Title: ').trim()
```

`trim()` is a **method** of strings. In Zuri, every value has methods,
including numbers, booleans and `nil`, and you call them with a dot.

## Storing One Bookmark

A bookmark has a title, a URL and some tags. The natural way to represent
that is a **dictionary**, which is a set of keys, each with a value.

```zuri
var bookmark = {
  title: 'Zuri docs',
  url: 'https://zuri.dev',
  tags: ['lang', 'docs'],
}
```

Square brackets create a **list**, which is an ordered sequence of values.
Curly braces create a dictionary. You read a value from a dictionary with a
dot, just as you do with a module:

```zuri,ignore
echo bookmark.title
echo bookmark.tags
```

```console
Zuri docs
[lang, docs]
```

Zuri has a shorthand that you'll use all the time. When a key has the same
name as the variable that holds its value, you only need to write the name
once:

```zuri
var title = 'Zuri docs'
var url = 'https://zuri.dev'

var bookmark = { title, url }
```

This creates the same dictionary as `{ title: title, url: url }`, but it's
shorter and easier to read.

## Many Bookmarks

A single bookmark is a dictionary, so a collection of bookmarks is a list of
dictionaries:

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

The `json` module converts Zuri values to text and back. `file()` opens a
file, and `'w'` opens it for writing.

```zuri
def save(bookmarks) {
  var handle = file(STORE, 'w')
  handle.write(json.encode(bookmarks, false))
  handle.close()
}
```

`def` declares a function. The parameters don't need types, although you can
add them (see [Chapter 5](ch05-03-type-annotations.md)). The body is a
block, and in Zuri, blocks always use braces.

The second argument to `json.encode()` is called `compact`. It defaults to
`true`, which puts everything on one dense line. Passing `false` gives you
indented output instead, which is easier to read if someone opens the file:

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

## Loading the File Safely

Reading the file is where a real program has to think about what might go
wrong. The file might not exist yet. Or it might exist but contain invalid
JSON because someone edited it by hand. Neither case should crash the
program.

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

This code introduces two new things.

The first is `${...}` inside a string. This is called **interpolation**: the
expression inside the braces is evaluated, and its value is inserted into
the string. Interpolation works in both single-quoted and double-quoted
strings.

The second is `catch`. Zuri doesn't have `try`. Instead, you wrap the code
that might fail in `catch { ... }` and handle the error in
`as error { ... }`. If nothing goes wrong, the handler doesn't run. If
something does, `error` is an object with `message`, `type` and `stacktrace`
properties.

There's no `finally` either. Code after the `catch` statement runs whether
or not there was an error, which covers most of the things `finally` is
normally used for.

## Adding a Bookmark

Now we can put these pieces together:

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
    tags: tags.is_empty()
        ? []
        : tags.split(',').map(@(t) {
            return t.trim()
          }),
  })

  save(bookmarks)
  echo 'Saved "${title}".'
}
```

The line that builds the tags uses three new ideas.

`cond ? a : b` is the conditional operator. If `cond` is truthy, the result
is `a`. Otherwise, it's `b`.

`split(',')` splits a string into a list at each comma.

`map()` calls a function on every element of a list and returns a new list
of the results. `@(t) { return t.trim() }` is an **anonymous function**,
written as `@` followed by a parameter list and a body. You can also write
it as `def(t) { ... }`, but `@` is the shorthand, and it's what most Zuri
code uses for short functions.

There's an even shorter form. When the body is a single expression whose
value you want to return, you can use `=>` instead of the braces and the
`return`:

```zuri,ignore
tags.split(',').map(@(t) => t.trim())
```

These are three ways of writing the same function. [Chapter
5](ch05-02-closures.md) covers all of the forms and when to use each one.

Notice also that the code uses `or` instead of `||`. Zuri's logical
operators are `and`, `or` and `!`.

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

`join` is a method of the string, not of the list. It reads naturally: take
this separator and use it to join the items in that list.

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

`each()` calls your function once for every element, passing the **value
first** and the **index second**. This order surprises some people. It's the
same for lists, dictionaries and strings.

Searching is a filter combined with a test:

```zuri
def find(bookmarks) {
  var needle = io.readline('Search: ').trim().lower()

  var hits = bookmarks.filter(@(bookmark) {
    if bookmark.title.lower().contains(needle) {
      return true
    }
    return bookmark.tags.some(@(tag) {
      return tag.lower() == needle
    })
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

`filter()` keeps the elements for which your function returns `true`.
`some()` checks whether a condition is true for at least one element, and it
stops as soon as it finds one.

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

`to_number()` converts a string to a number, `bookmarks[position]` gets an
element of a list by its index, and `remove_at()` removes the element at a
given position and moves the remaining elements down.

The bounds check is necessary. Reading past the end of a list raises an
error, and an error that isn't handled ends the program.

## The Main Loop

The last piece is a loop that reads a command and decides what to do with
it:

```zuri,ignore
def main() {
  var bookmarks = load()

  echo 'Bookmark keeper. ${bookmarks.length()} saved.'

  while true {
    var command = io
        .readline('\n(a)dd (l)ist (f)ind (r)emove (q)uit > ')
        .trim()
        .lower()

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

`using` is Zuri's multi-way branch statement, and each branch starts with
`when`. It compares the value against each `when` and runs the first branch
that matches. The `default` branch handles everything else. When a branch's
body is a single short statement, it can stay on the same line, which keeps
a dispatch table like this one easy to read. Longer bodies need a block.

Unlike `switch` in C, there's no fall-through, so you don't need `break`.
One branch runs, and then the statement is finished.

## The Complete Program

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
    tags: tags.is_empty()
        ? []
        : tags.split(',').map(@(t) {
            return t.trim()
          }),
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
    return bookmark.tags.some(@(tag) {
      return tag.lower() == needle
    })
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
    var command = io
        .readline('\n(a)dd (l)ist (f)ind (r)emove (q)uit > ')
        .trim()
        .lower()

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

Here's what a session looks like:

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

## Summary

You've just written a program with persistent state, user input, error
handling and a command loop, in about a hundred lines, using only what comes
with Zuri.

Along the way, you've seen `var`, `def`, `if`/`else`, `while`,
`using`/`when`, `catch`/`as`, `return`, lists, dictionaries, the dictionary
shorthand, string interpolation, the conditional operator, anonymous
functions and a number of built-in methods.

The next six chapters explain each of these in detail.
