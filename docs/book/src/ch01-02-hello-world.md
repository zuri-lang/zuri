# Hello, World!

Make a directory, put one file in it, and run it.

```console
$ mkdir hello
$ cd hello
```

Create a file called `main.zu`. The `.zu` extension is what the runtime
looks for when resolving imports, so get in the habit early.

<span class="filename">Filename: main.zu</span>

```zuri
echo 'Hello, world!'
```

Run it:

```console
$ zuri run main.zu
Hello, world!
```

That is the whole program. No `main` function, no imports, no boilerplate.
A Zuri file is a script, and its top level is code that runs.

## Anatomy of One Line

```zuri
echo 'Hello, world!'
```

`echo` is a **keyword**, not a function. You do not write `echo(...)`, you
write `echo` followed by an expression. It prints the value and adds a
newline.

Strings are written between single or double quotes, and the two are
identical in meaning. Most Zuri code uses single quotes and saves double
quotes for strings that contain an apostrophe.

There is no semicolon. Zuri ends a statement at the end of the line. You
*can* write a semicolon if you want two statements on one line, but almost
nobody does.

## `echo` and `print`

There is also a `print()` function, and the difference is worth learning
now because it bites people later.

```zuri
echo 'first'
echo 'second'

print('third')
print('fourth\n')
```

```console
first
second
thirdfourth
```

`echo` appends a newline; `print()` does not, and it takes any number of
arguments. Use `echo` for output meant for a human reading a terminal, and
`print()` when you are assembling output character by character.

## Running a Directory

`run` takes a directory as readily as a file. Point it at one and it
looks for `index.zu` inside and runs that:

```console
$ zuri run hello
Hello, world!
```

If there is no `index.zu`, you get told so plainly:

```console
$ zuri run hello
(Zuri):
  Launch aborted for hello
  Reason: No entrypoint found in the directory
```

This is the same rule the module system uses for packages, which we will
get to in [Chapter 8](ch08-00-modules.md). A directory with an `index.zu`
is a unit you can run *or* import.

Leave the path off and `run` launches the directory you are standing in,
which is how a project is usually started:

```console
$ cd hello
$ zuri run
Hello, world!
```

Everything after the path belongs to the program rather than to `zuri`,
so a script reads its own flags exactly as it would anywhere else:

```console
$ zuri run main.zu --name Ada --verbose
```

[Chapter 20](ch20-00-args.md) covers reading them.

## Starting a Project

One file is where everybody starts, and it stops being enough about as
soon as there are two of them. `zuri init` writes the layout a project
grows into:

```console
$ zuri init myproject
```

Run in a terminal, it asks what the project is called, what version it
starts at and who wrote it, offering an answer to each. Press enter to
take what it offers. Run anywhere there is nobody to ask — a script, a
CI job — it takes those answers itself, which `--yes` also says
outright.

What it leaves behind:

```text
myproject/
  project.toml          what the project is
  index.zu              the entry point
  app/
    index.zu            the application
  tests/
    app.test.zu         its tests
  README.md
  .gitignore
  .gitattributes
```

It runs, and its tests pass, before you have written anything:

```console
$ cd myproject
$ zuri run
Hello, world!
$ zuri test

  zuri test  1 file in tests

   PASS   app.test.zu  36ms  2 tests

  1 files
  2 passed  •  2 total
```

The root `index.zu` is two things at once:

<span class="filename">Filename: index.zu</span>

```zuri,ignore
import @.app { * }

if __root__ == __file__ {
  main()
}
```

The import re-exports everything `app` declares, so another project can
`import myproject` and reach all of it. The `if` starts the application,
but only when this file is the one that was run: `__root__` is the file
`zuri` was pointed at, and `__file__` is the file the line is written
in. Importing the project leaves `main()` alone. That is the Zuri
spelling of a main guard, and it is worth knowing early because every
runnable package uses it.

`app/index.zu` is the application itself. Whatever it exports, the
project exports:

<span class="filename">Filename: app/index.zu</span>

```zuri,ignore
def greet(name: ?string) {
  return 'Hello, ${name or "world"}!'
}

def main() {
  echo greet()
}
```

`project.toml` is what the project is, for anything that reads it:

```toml
[project]
name = "myproject"
version = "0.1.0"
description = ""
authors = ["Ada Lovelace <ada@example.com>"]
license = "MIT"

[dependencies]
```

The name comes from the directory, the author from `git config`, and a
git repository is created unless there is already one above. `zuri init
--help` has the rest, and every question it asks has a flag that answers
it:

```console
$ zuri init myproject --name web-server --license Apache-2.0 --yes
```

Run inside a directory that already has files, `init` adds what is
missing and writes over nothing. Run where a project already is, it
refuses rather than overwrite one.

[Chapter 8](ch08-00-modules.md) is where packages and re-exports are
covered properly, and [Chapter 25](ch25-00-task-board.md) grows this
layout into a full application.

## Commands

A first word that is not `run` names a command instead of a path:

```console
$ zuri greet Ada
Hello, Ada!
```

A command is a `.zu` file, or a directory with an `index.zu`, sitting in
a `cmds` directory. A project keeps its own in `.zuri/cmds`, so either of
these answers to `zuri greet`:

```text
.zuri/cmds/greet.zu           # a command in one file
.zuri/cmds/greet/index.zu     # a command with room to grow
```

The directory wins if both are there, so a command that has outgrown one
file takes over the name as soon as its `index.zu` lands. Until then the
directory is not a command, and the file goes on answering.

The commands the runtime itself ships live in the `cmds` directory beside
the executable, and those win over a project's. Everything after the name
is forwarded to the command untouched, flags included.

A name that matches nothing is refused rather than guessed at:

```console
$ zuri gret Ada
(Zuri):
  Launch aborted for gret
  Reason: Unknown command
```

A command says what it is in its own doc block, with two tags. `zuri
--help` lists every command it can reach by exactly those:

<span class="filename">Filename: .zuri/cmds/greet.zu</span>

```zuri,ignore
/**
 * @command greet
 * @description Say hello to somebody by name.
 */

import os

echo 'Hello, ${os.args[2]}!'
```

```console
$ zuri --help
Zuri 0.1.0 (running on ZuriVM 0.1.0)
Build No. => 2026-09-10 23:19:10 UTC

Usage: zuri                    start the interactive REPL
       zuri run [PATH]         run a script, a package, or this directory
       zuri <command> [ARGS]   run a command

OPTIONS:
  -h, --help     Show this help message and exit
  -v, --version  Show version information and exit

COMMANDS:
  format  Lay out Zuri source in the project style.
  init    Scaffold a new Zuri project.
  test    Run a project's test files, each in its own process.

PROJECT COMMANDS:
  greet   Say hello to somebody by name.

Run "zuri <command> --help" for help on a specific command.
```

A description too long for one line carries on below it, indented:

```zuri,ignore
/**
 * @command deploy
 * @description Ship the current build to staging, then wait for
 *    the health check to come back green.
 */
```

Two flags sit outside all of this and run nothing. `zuri --help` is the
listing above, and `zuri --version` reports just the build:

```console
$ zuri --version
Zuri 0.1.0 (running on ZuriVM 0.1.0)
Build No. => 2026-09-10 23:19:10 UTC
```

Both take the short spelling too, `-h` and `-v`.
