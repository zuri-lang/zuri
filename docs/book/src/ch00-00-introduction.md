# Introduction

Welcome. Open a terminal, keep it next to this page, and let's begin.

This book teaches Zuri. It starts with installing the language and printing
a line of text, and it ends with a task-board web application that stores
data on disk, renders HTML from templates, serves a JSON API, and runs
behind a stack of middleware. Everything in between is the road from one to
the other.

## What Zuri Looks Like

Here is a complete program. You do not need to understand all of it yet;
read it the way you would read a paragraph in a language you are learning,
and see how much comes through.

```zuri
class Account {

  @new(owner: string, balance: number) {
    self.owner = owner
    self.balance = balance
  }

  deposit(amount: number) {
    if amount <= 0 {
      raise ValueError('deposit must be positive')
    }

    self.balance += amount
    return self.balance
  }

  to_string() {
    return '${self.owner}: ${self.balance}'
  }
}

var accounts = [
  Account('ada', 100),
  Account('grace', 250),
]

for account in accounts {
  account.deposit(50)
  echo account.to_string()
}
```

```console
ada: 150
grace: 300
```

Most of that will be familiar if you have written code before. The pieces
worth pointing at now, because they come up on the very first page of
Chapter 3:

- `var` declares a variable, and `class` declares a class.
- A constructor is called `@new`. Methods whose names begin with `@` hook
  into the language's own syntax, and Chapter 6 covers the full set.
- `self` refers to the instance, and reading a field always goes through
  it: `self.balance`, never a bare `balance`.
- `'${...}'` interpolates an expression into a string.
- `echo` prints a value and a newline.
- `raise` signals an error; Chapter 7 shows how to catch one.
- Every block is written with braces, and every control-flow statement
  requires one.

## Who This Book Is For

Chapters 1 through 6 assume you can open a terminal and nothing else. If
this is your first programming language, start at the beginning and go
slowly. Every idea is introduced before it is used, and every example is
short enough to type out.

If you already write Python, JavaScript, Ruby, Go or Java, you can move
through Chapters 3 to 5 quickly. Read [Appendix H](appendix-08-coming-from.md)
first: it lists the places where Zuri does something different from what
the same syntax does in the language you already know, which is where the
hours get lost.

## How This Book Is Organised

**Getting started**, Chapters 1 and 2. Install the language, run something,
then build a small command-line application end to end so you have seen the
shape of a real program before we take one apart.

**The language**, Chapters 3 to 8. Variables, types, operators, control
flow, strings, numbers, collections, functions, closures, type
annotations, classes, inheritance, decorated methods, errors and modules.
Read these in order.

**The world outside your program**, Chapters 9 to 13. Files, binary data
and byte streams, isolates and concurrency, sockets and networking, and a
tour of the standard library.

**Four large modules**, Chapters 14 to 17. Wire for templating, HTTP for
clients and servers, Imagine for images, and SQL for databases. Each is
big enough to need a chapter of its own, and each is a reference you will
come back to.

**Depth**, Chapters 18 to 21. Reflection and the compiler API, how Zuri
executes your code and how to make it faster, how to test it, and how to
debug a program that is doing something you did not expect.

**The capstone**, Chapter 22. One application, built in seven steps, using
almost everything the book has covered.

**Appendices.** Keywords, operators and precedence, decorated methods,
built-in functions, every method on every built-in type, the standard
library index, the error hierarchy, and the notes for readers arriving from
another language. These are reference material; the rest of the book is
prose.

## How to Read It

Read it with the interpreter running. Zuri has a REPL, every short example
in this book can be pasted straight into it, and the fastest way to
understand a rule is to break it on purpose and read the error.

Chapters build on each other. When a chapter needs something from later in
the book, it says so and links to it, and you can carry on without
following the link. When a chapter introduces something you will need
again, it says that too.

## Conventions

Commands you type in a shell appear with a `$` prompt, and the output
follows underneath:

```console
$ zuri main.zu
Hello, world!
```

Code that belongs in a file appears with the filename above it when the
filename matters:

<span class="filename">Filename: main.zu</span>

```zuri
echo 'Hello, world!'
```

Sessions in the interactive prompt use `%>` for the first line of an input
and `..` for its continuations, which is exactly what the REPL itself
prints:

```zuri
%> var name = 'zuri'
%> name.upper()
'ZURI'
```

Where a rule has an exception, the exception is stated in the same
paragraph as the rule. Where a limit exists, it is stated as a limit.
Nothing in this book is a guess about how the language behaves; every claim
was checked by running it.

Let's get started.
