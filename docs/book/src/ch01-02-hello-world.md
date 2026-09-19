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
