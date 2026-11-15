# Introduction

Welcome! Open a terminal next to this page and let's get started.

This book teaches you Zuri. It starts with installing the language and
printing a line of text, and it ends with a task-board web application that
stores its data on disk, renders HTML from templates, serves a JSON API and
runs behind several layers of middleware. The chapters in between cover
everything you need to get from the first program to the last.

## What Zuri Looks Like

Here is a complete program. Don't worry about understanding all of it yet.
Read it the way you'd read a paragraph in a language you're learning, and
see how much of it makes sense.

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

var accounts = [Account('ada', 100), Account('grace', 250)]

for account in accounts {
  account.deposit(50)
  echo account.to_string()
}
```

```console
ada: 150
grace: 300
```

If you've programmed before, most of this will look familiar. A few things
are worth pointing out now, because you'll see them again as soon as Chapter
3:

- `var` declares a variable, and `class` declares a class.
- The constructor is named `@new`. Methods whose names start with `@` hook
  into the language's own syntax. Chapter 6 lists all of them.
- `self` refers to the current instance, and you always read a field through
  it. You write `self.balance`, never just `balance`.
- `'${...}'` inserts the value of an expression into a string.
- `echo` prints a value followed by a newline.
- `raise` raises an error. Chapter 7 explains how to catch one.
- Blocks are written with braces, and every control flow statement needs
  them.

## What Comes With Zuri

When you install Zuri, you get its whole toolchain as well. Each tool is a
subcommand of `zuri`, and each one is covered somewhere in this book:

| Command | What it does | Where |
| --- | --- | --- |
| `zuri` | Starts the interactive REPL | [Chapter 1](ch01-03-the-repl.md) |
| `zuri run` | Runs a script, a package or a directory | [Chapter 1](ch01-02-hello-world.md) |
| `zuri init` | Creates a new project | [Chapter 1](ch01-02-hello-world.md#starting-a-project) |
| `zuri fmt` | Formats Zuri source code | [Chapter 1](ch01-02-hello-world.md#formatting) |
| `zuri test` | Runs a project's tests | [Chapter 23](ch23-00-testing.md) |
| `zuri migrate` | Applies and rolls back database migrations | [Chapter 17](ch17-02-migrations.md) |
| `zuri lsp` | Runs the language server that your editor talks to | [Chapter 29](ch29-00-editors.md) |
| `zuri doc` | Builds a documentation site (like this book) from Markdown, and an API reference from doc comments | [Chapter 30](ch30-00-documentation.md) |
| `zuri install` | Adds packages to a project and records them in a lockfile | [Chapter 27](ch27-02-installing.md) |
| `zuri publish` | Publishes a package to a registry | [Chapter 27](ch27-03-publishing.md) |
| `zuri serve` | Runs Nyssa, your own package registry | [Chapter 27](ch27-06-nyssa.md) |
| `zuri bundle` | Packages a program to run on machines without Zuri | [Chapter 27](ch27-05-bundles.md) |
| `zuri upgrade` | Updates Zuri to a newer release | [Chapter 27](ch27-05-bundles.md#upgrading-zuri) |

Projects and packages can add their own commands too. [Appendix
I](appendix-09-commands.md) shows how.

## Who This Book Is For

Chapters 1 through 6 assume only that you know how to open a terminal. If
Zuri is your first programming language, start at the beginning and take
your time. Each idea is explained before it's used, and the examples are
short enough to type in yourself.

If you already know Python, JavaScript, Ruby, Go or Java, you can skim
Chapters 3 to 5. Read [Appendix H](appendix-08-coming-from.md) first,
though. It lists the places where Zuri behaves differently from the language
you know, even when the syntax looks the same, and those are the places
where you're most likely to get stuck.

## How This Book Is Organised

**Getting started**, Chapters 1 and 2. You'll install the language, run some
code, create and format a project, and then build a small command-line
application from start to finish, so you've seen what a real program looks
like before we go through the language piece by piece.

**The language**, Chapters 3 to 8. Variables, types, operators, control
flow, strings, numbers, collections, functions, closures, type annotations,
classes, inheritance, decorated methods, error handling and modules. These
chapters are best read in order.

**The world outside your program**, Chapters 9 to 13. Files, binary data
and byte streams, isolates and concurrency, sockets and networking, and a
tour of the standard library.

**The large modules**, Chapters 14 to 20. Wire for templates, HTTP for
clients and servers, Imagine for images, SQL for databases, Mail for sending
and receiving email, env for configuration, and args for parsing the command
line. Each of these modules is big enough for a chapter of its own, and
you'll probably come back to them as a reference.

**Going deeper**, Chapters 21 to 24. Reflection and the compiler API, how
Zuri runs your code and how to make it faster, how to test it, and how to
debug a program that isn't doing what you expected.

**The capstone project**, Chapter 25. A single application, built in seven
steps, that uses most of what the book has covered.

**Interoperability**, Chapter 26. Calling C and Rust libraries, and letting
them call back into Zuri.

**Sharing and shipping**, Chapter 27. Installing and publishing packages,
the lockfile, bundling a program so it runs where Zuri isn't installed, and
running Nyssa, the package registry, for your team or for the public.

**Talking to other programs**, Chapter 28. JSON-RPC, for calling methods in
another program and handling its calls over any kind of connection.

**Tools**, Chapters 29 and 30. The language server and how to set up your
editor to use it, and building documentation with `zuri doc`.

**Appendices.** Keywords, operators and precedence, decorated methods,
built-in functions, the methods of every built-in type, an index of the
standard library, the error hierarchy, notes for readers coming from other
languages, and how to write your own commands. The appendices are meant for
looking things up, while the chapters are meant to be read.

## How to Read It

Keep the REPL open while you read. You can paste any of the short examples
straight into it, and one of the quickest ways to understand a rule is to
break it deliberately and read the error message.

The chapters build on each other. When a chapter mentions something that's
covered later, it links to it, but you can keep reading without following
the link. When a chapter introduces something you'll need again later, it
tells you.

## Conventions

Commands that you type in a shell are shown with a `$` prompt, followed by
their output:

```console
$ zuri run main.zu
Hello, world!
```

When the file name matters, code that belongs in a file is shown with the
name above it:

<span class="filename">Filename: main.zu</span>

```zuri
echo 'Hello, world!'
```

Sessions in the REPL use `%>` for the first line of input and `..` for each
continuation line, which is what the REPL itself prints:

```zuri
%> var name = 'zuri'
%> name.upper()
'ZURI'
```

All of the examples in this book were run to check that they behave as
described.

Let's get started.
