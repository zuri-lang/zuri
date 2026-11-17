# Hello, World!

Create a directory, put one file in it, and run it.

```console
$ mkdir hello
$ cd hello
```

Create a file called `main.zu`. The runtime looks for the `.zu` extension
when it resolves imports, so it's a good habit to use it from the start.

<span class="filename">Filename: main.zu</span>

```zuri
echo 'Hello, world!'
```

Then run it:

```console
$ zuri run main.zu
Hello, world!
```

That's the entire program. You don't need a `main` function, imports or any
boilerplate. A Zuri file is a script, and the code at its top level runs
when you run the file.

## A Closer Look

```zuri
echo 'Hello, world!'
```

`echo` is a **keyword**, not a function. Instead of `echo(...)`, you write
`echo` followed by an expression. It prints the value followed by a newline.

Strings can be written in single or double quotes, and both mean the same
thing. Most Zuri code uses single quotes, and double quotes for strings that
contain an apostrophe.

There's no semicolon at the end. A statement ends at the end of the line.
You *can* use a semicolon to put two statements on one line, but that's
rarely done.

## `echo` and `print`

There's also a `print()` function. It's worth knowing how it differs from
`echo` now, because the difference can catch you out later.

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

`echo` adds a newline and `print()` doesn't, and `print()` accepts any
number of arguments. Use `echo` for output that someone will read in a
terminal, and `print()` when you're building up output piece by piece.

## Running a Directory

`run` accepts a directory as well as a file. When you give it a directory,
it looks for an `index.zu` file inside and runs that:

```console
$ zuri run hello
Hello, world!
```

If there's no `index.zu`, it tells you:

```console
$ zuri run hello
(Zuri):
  Launch aborted for hello
  Reason: No entrypoint found in the directory
```

The module system uses the same rule for packages, which we'll look at in
[Chapter 8](ch08-00-modules.md). A directory with an `index.zu` can be
either run *or* imported.

If you leave out the path, `run` runs the current directory. This is the
usual way to start a project:

```console
$ cd hello
$ zuri run
Hello, world!
```

Any arguments after the path are passed to your program, not to `zuri`, so a
script can read its own flags the same way it would anywhere else:

```console
$ zuri run main.zu --name Ada --verbose
```

[Chapter 20](ch20-00-args.md) explains how to read them.

## Rerunning on Every Change

While you're working on a program, the `--watch` flag (or `-w` for short)
keeps it running and restarts it from the beginning whenever one of its
files changes:

```console
$ zuri run -w
watching index.zu and the files it uses, Ctrl+C to stop
Hello, world!
index.zu finished, waiting for a change
```

The watched files are the entry file, every module it imports, the
[Wire](ch14-00-wire.md) templates it renders, and the project's
`project.toml` and `.env` files. Zuri discovers modules and templates as the
program runs, so a module that's only imported partway through is watched
from the moment it's loaded. Files the program writes itself, such as logs,
databases and uploads, don't trigger a restart.

Watch mode writes its own messages to standard error, so you can still pipe
the program's output somewhere else. If the program exits, watch mode waits
for the next change. If it fails to start because of a mistake in the code,
it's started again as soon as you fix the mistake. Press Ctrl+C to stop both
the program and watch mode.

The flag has to come right after `run`. Anything after the path is passed to
the program, so `zuri run . -w` passes `-w` to your program instead.

## Starting a Project

Everyone starts with a single file, but once you have a second one you'll
want some structure. `zuri init` creates the layout for a new project:

```console
$ zuri init myproject
```

When you run it in a terminal, it asks for the project's name, its starting
version and its author, and suggests an answer for each. Press Enter to
accept the suggestion. When there's nobody to answer, such as in a script or
a CI job, it uses the suggested answers automatically. You can also ask for
that explicitly with `--yes`.

Here's what it creates:

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

The project runs, and its tests pass, before you've written any code:

```console
$ cd myproject
$ zuri run
Hello, world!
$ zuri test

  zuri test  1 file in tests

   PASS   app.test.zu  36ms  2 tests

  1 file
  2 passed  •  2 total
```

The root `index.zu` does two jobs:

<span class="filename">Filename: index.zu</span>

```zuri,ignore
import @.app { * }

if __root__ == __file__ {
  main()
}
```

The import re-exports everything that `app` declares, so another project can
`import myproject` and use all of it. The `if` statement starts the
application, but only when this file is the one being run. `__root__` is the
file that `zuri` was started with, and `__file__` is the file that contains
the line. When the project is imported, `main()` isn't called. This is how
Zuri writes a main guard, and you'll see it in every runnable package.

`app/index.zu` contains the application itself. Whatever it exports, the
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

`project.toml` describes the project for any tool that reads it:

```toml
[project]
name = "myproject"
version = "0.1.0"
description = ""
authors = ["Ada Lovelace <ada@example.com>"]
license = "MIT"

[dependencies]
```

The name is taken from the directory and the author from `git config`. A git
repository is created as well, unless the directory is already inside one.
Run `zuri init --help` to see the other options. Each question that `init`
asks can also be answered with a flag:

```console
$ zuri init myproject --name web-server --license Apache-2.0 --yes
```

If you run `init` in a directory that already contains files, it adds the
files that are missing and doesn't overwrite anything. If the directory
already contains a project, `init` stops without changing it.

[Chapter 8](ch08-00-modules.md) covers packages and re-exports in detail,
and [Chapter 25](ch25-00-task-board.md) builds this layout into a full
application.

## Formatting

`zuri fmt` formats Zuri source code in a single, consistent style: two
spaces per indentation level, one statement per line, explicit blocks where
they belong, conventional spacing, and lines longer than 80 columns broken
at sensible points. Line breaking follows the rules of the Dart formatter. A
list of arguments or items stays on one line if it fits, and otherwise goes
one item per line. A list or dictionary that contains another one is always
expanded, as `rooms` is below. When a call has a single function or
collection among its arguments, only that argument is expanded, as in
`each(items, @{`. The layout depends only on the code, so it doesn't matter
where you broke lines inside an expression. Blank lines between statements
are kept.

Here's a file written in a hurry:

<span class="filename">Filename: rooms.zu</span>

```zuri
def area(width, height) {
  return width * height
}

var rooms = [
  [3, 4],
  [5, 6],
]
for room in rooms {
  echo area(room[0], room[1])
}
```

```console
$ zuri fmt rooms.zu
formatted rooms.zu
Formatted 1 file.
```

<span class="filename">Filename: rooms.zu</span>

```zuri
def area(width, height) {
  return width * height
}

var rooms = [
  [3, 4],
  [5, 6],
]
for room in rooms {
  echo area(room[0], room[1])
}
```

If you give `zuri fmt` a directory, it formats every `.zu` file in it,
except for packages the project has installed. With no path, it formats the
current directory. Formatting never changes what a program does. Before
writing a file, the formatter parses its own output and checks that the code
and all of its comments are unchanged. If it can't format a file that way,
it leaves the file alone.

With `--dry-run`, nothing is written. The formatter lists each file that
would change and exits with status 1 if there are any, which is what you
want in a commit hook or a CI job:

```console
$ zuri fmt --dry-run
would format /home/ada/myproject/app/extra.zu
1 file would change. 3 files were already formatted.
```

Your editor uses the same formatter through the language server, for a whole
file or a selection. [Chapter 29](ch29-00-editors.md) explains how to set
that up.

## Commands

If the first word after `zuri` isn't `run`, it's treated as the name of a
command rather than a path:

```console
$ zuri greet Ada
Hello, Ada!
```

A command is either a `.zu` file or a directory with an `index.zu`, inside a
`cmds` directory. A project keeps its own commands in `.zuri/cmds`, so
either of these will run as `zuri greet`:

```text
.zuri/cmds/greet.zu           # a command in one file
.zuri/cmds/greet/index.zu     # a command with room to grow
```

If both exist, the directory is used. This means that when a command grows
too big for one file, the directory takes over as soon as you add its
`index.zu`. Until then, the directory isn't a command and the file is still
used.

The commands that come with the runtime live in the `cmds` directory next to
the executable, and they take priority over a project's own commands.
Everything after the command name is passed to the command unchanged,
including flags.

If a name doesn't match any command, `zuri` reports an error instead of
guessing:

```console
$ zuri gret Ada
(Zuri):
  Launch aborted for gret
  Reason: Unknown command
```

A command describes itself in its doc comment, using two tags. `zuri --help`
uses them to list every command it can find:

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
       zuri run -w [PATH]      run it again whenever a file it uses changes
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

If a description doesn't fit on one line, continue it on the lines below,
indented:

```zuri,ignore
/**
 * @command deploy
 * @description Ship the current build to staging, then wait for
 *    the health check to come back green.
 */
```

Two flags don't run anything at all. `zuri --help` prints the list shown
above, and `zuri --version` prints the build information:

```console
$ zuri --version
Zuri 0.1.0 (running on ZuriVM 0.1.0)
Build No. => 2026-09-10 23:19:10 UTC
```

Both also have short forms, `-h` and `-v`.
