# Appendix I: Custom Commands

`zuri fmt`, `zuri init` and `zuri test` are Zuri programs. The runtime
ships them in a `cmds` directory beside itself, and when the first word
after `zuri` is not `run`, it looks that word up there and runs the
script it finds. A project adds commands of its own the same way, in
its `.zuri/cmds` directory, and they are run, listed and documented
exactly like the ones that ship.

This is the place for the scripts a project keeps running by hand: a
release checklist, a data import, a code generator. As a command, each
one is found by name from anywhere in the checkout's root, shows up in
`zuri --help` with a line saying what it does, and parses its own
arguments with the same `args` module as everything else.

## Where Commands Are Found

`zuri <name>` looks in two places, in this order, and runs the first
match:

| Where | What it holds |
| --- | --- |
| `$ZURI_ROOT/cmds`, or `cmds` beside the executable when `ZURI_ROOT` is unset | the commands the runtime ships |
| `.zuri/cmds` in the working directory | the project's own commands |

The shipped commands come first, so a project cannot replace one: a
project command named `test` is never run, and `zuri --help` leaves it
out of the listing. Only the working directory's `.zuri` is consulted,
not those of the directories above it, so a project's commands are run
from its root.

`zuri init` writes a `.gitignore` that ignores everything under `.zuri`
except `.zuri/cmds`, so a project's commands are committed with the
rest of its code while the tools that keep state in `.zuri` do not
leave it behind in the repository.

## Writing One

A command is a single `.zu` file, or a directory with an `index.zu`:

```text
.zuri/
  cmds/
    greet.zu
    release/
      index.zu
      notes.zu
      tests/
```

`zuri greet` runs `greet.zu`, and `zuri release` runs
`release/index.zu`. When a directory and a file share a name, the
directory wins. The directory form is for a command that has grown past
one file: `index.zu` imports its siblings relatively, as any package
does, with `import .notes`.

The name a command answers to is its file or directory name. It is a
single name, never a path: `zuri tools/greet` is refused as an unknown
command before anything is looked up.

## Naming and Describing It

The first doc block in the file introduces the command. `@command`
states the name it answers to, and `@description` is the one line
`zuri --help` shows beside it:

```zuri,ignore
/**
 * @command release
 * @description Tags a release and writes its notes from the commits
 *    since the last one.
 *
 * Run it from a clean checkout on the main branch:
 *
 *   zuri release 1.4.0
 */

import .notes
```

The block has to open within the first 16 KiB of the file, which in
practice means at the top. A description longer than a line carries on
over the indented lines beneath the tag, and ends at a blank line or
the next tag. A command without a description is still listed, with
its name alone. `@command` is what a reader of the file sees first, and
it names the file's own command; the runtime lists and runs a command
by its file or directory name, so keep the two the same.

## Arguments and the Exit Status

Everything after the command's name belongs to the command, `--help`
included, and a command reads it with the [`args`](ch20-00-args.md)
module. Build a parser named after the command and call `parse()` with
nothing: it reads the real command line, converts and validates each
value, answers `--help` from the declarations, and refuses anything
undeclared with a message and exit status 1. That is what gives every
command, shipped or not, the same flags, the same help layout and the
same errors:

```zuri,ignore
/**
 * @command greet
 * @description Greets whoever is named, as often as asked.
 */

import args

def parser() {
  var p = args.Parser('greet', false)

  p.description = 'Greets whoever is named, as often as asked.'
  p.add_index('name', 'Who to greet', { required: true })
  p.add_option('times', 'How many greetings', { short_name: 't', type: args.INT, value: 1 })

  return p
}

var parsed = parser().parse()
var name = parsed.indexes[0]

iter var i = 0; i < parsed.options.times; i++ {
  echo 'Hello, ${name}!'
}
```

The raw list is there as well. `os.args` has the same shape for a
command as for `zuri run`: the executable, the command's own file, then
what the user typed, so the command's arguments are `os.args[2,]`.
Reading that list by hand is how the checks and the help text drift
apart, which is exactly what the parser exists to prevent, so a command
should reach for `args` and leave `os.args` alone.

A command ends with status 0 when it runs to the end. An uncaught error
prints its trace and ends it with status 1, and `os.exit()` ends it
with any other status. A command that checks something, the way
`zuri fmt --dry-run` checks formatting, shoul exit with a non-zero exit 
code when the check fails, so a shell script or a CI job can act on it.

## Where It Runs

A command runs in the directory `zuri` was started in, so `os.cwd()`
is the user's directory, and for a project command that is the
project's root. `__file__` is the command's own file, which is how a
command reaches something shipped beside it:

```zuri,ignore
import os

var template = os.join_paths(os.dir_name(__file__), 'notes.template')
```

## Seeing It Work

This example builds a throwaway project with one command, runs it the 
way a user would, asks it for its help message, and reads back the 
listing `zuri --help` gives:

```zuri
import os

var project = os.create_temp_dir('zuri-commands-')
var commands = os.join_paths(project, '.zuri', 'cmds')

os.create_dir(commands, 0c755, true)

var source = file(os.join_paths(commands, 'greet.zu'), 'w')

source.write(
  '/**\n' +
  ' * @command greet\n' +
  ' * @description Greets whoever is named.\n' +
  ' */\n' +
  'import args\n' +
  'var parser = args.Parser("greet", false)\n' +
  'parser.description = "Greets whoever is named."\n' +
  'parser.add_index("name", "Who to greet", { value: "world" })\n' +
  'echo "Hello, " + parser.parse().indexes[0] + "!"\n'
)
source.close()

# Runs `zuri` in the project with `arguments`, and returns what it printed.
def zuri(arguments) {
  var child = os.spawn(os.exe_path, arguments, { cwd: project, stdin: 'null' })

  child.wait()

  return child.read_stdout().to_string()
}

echo zuri(['greet', 'Ada']).trim('\n')
echo zuri(['greet', '--help']).trim('\n')

# The part of the listing this project adds.
var listing = zuri(['--help'])
var start = listing.index_of('PROJECT COMMANDS')

echo listing[start, listing.index_of('\n\n', start)]

os.remove_dir(project, true)
```

```console
Hello, Ada!
Usage: greet [OPTIONS] [name]

  Greets whoever is named.

POSITIONAL ARGUMENTS:
  [name]    Who to greet (default: world)

OPTIONS:
  -h, --help  Show this help message and exit
PROJECT COMMANDS:
  greet  Greets whoever is named.
```

## Testing a Command

Build the parser in a function, as the `greet` command above does, and
the argument handling can be tested by handing `parse()` a list, the
seam [Chapter 20](ch20-00-args.md#testing-a-command-line) describes.
Everything else is ordinary Zuri: keep the command's logic in the
functions and files beside `index.zu`, and test those.

The shipped commands keep their tests in a `tests` directory inside the
command, and a project command can do the same. `zuri test` runs a
directory anywhere in the project:

```sh
zuri test .zuri/cmds/release/tests
```
