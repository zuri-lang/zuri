# Appendix I: Custom Commands

`zuri fmt`, `zuri init` and `zuri test` are Zuri programs. The runtime
includes them in a `cmds` directory next to itself, and when the first word
after `zuri` isn't `run`, it looks for that word there and runs the script
it finds. A project can add its own commands in the same way, in its
`.zuri/cmds` directory, and they're run, listed and documented exactly like
the commands that come with Zuri.

This is the place for scripts that a project keeps running by hand, such as
a release checklist, a data import or a code generator. As a command, each
one can be run by name from anywhere in the project, appears in
`zuri --help` with a line describing what it does, and parses its own
arguments with the same `args` module as everything else.

## Where Commands Are Found

`zuri <name>` looks in these places, in order, and runs the first match:

| Where | What it contains |
| --- | --- |
| `$ZURI_ROOT/cmds`, or `cmds` next to the executable if `ZURI_ROOT` isn't set | The commands that come with the runtime |
| `.zuri/cmds` in the project | The project's own commands |
| `cmds` in each package in the project's `.zuri/libs` | Commands provided by the project's packages |
| `cmds` in each package in `$ZURI_HOME/libs` | Commands provided by the packages installed for your user |

The project is the nearest directory above the working directory that
contains a `project.toml`, so a project's commands can be run from anywhere
inside it. If there's no project, `.zuri` in the working directory is used.

The commands that come with Zuri are checked first, so a project can't
replace one of them. A project command named `test` is never run, and
`zuri --help` leaves it out of the list. A project's own commands come
before any provided by packages, and the project's packages come before your
user's packages.

If two packages in the same place provide the same command, Zuri reports an
error instead of choosing one at random. `zuri <name>` names both packages
and runs neither, and `zuri --help` marks the command as provided twice.
`zuri install` won't let you create the conflict in the first place.

`zuri init` writes a `.gitignore` that ignores everything under `.zuri`
except `.zuri/cmds`, so a project's commands are committed along with the
rest of its code, while the tools that store state in `.zuri` don't leave it
in the repository.

## Writing a Command

A command is either a single `.zu` file or a directory with an `index.zu`:

```text
.zuri/
  cmds/
    greet.zu
    release/
      index.zu
      notes.zu
      tests/
```

`zuri greet` runs `greet.zu`, and `zuri release` runs `release/index.zu`. If
a directory and a file have the same name, the directory is used. The
directory form is for a command that has grown too big for a single file.
Its `index.zu` imports the other files with relative imports, as any package
does, such as `import .notes`.

The name of a command is its file or directory name. It's always a single
name, never a path, so `zuri tools/greet` is rejected as an unknown command
before anything is looked up.

A name starting with `_` is private, just as an identifier starting with one
is. `_notes.zu` and `_shared/index.zu` are never treated as commands, so
`zuri --help` leaves them out, and `zuri _notes` is an unknown command.
That's where to put code shared by several commands, which each of them
imports with a relative import, such as `import .._shared.notes` from
`release/index.zu`.

## Naming and Describing a Command

The first doc comment in the file describes the command. `@command` gives
the name it's run by, and `@description` is the line that `zuri --help`
shows next to it:

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

The doc comment has to start within the first 16 KiB of the file, which in
practice means at the top. A description that's longer than a line continues
on the indented lines below the tag, and ends at a blank line or the next
tag. A command without a description is still listed, with just its name.
`@command` is the first thing someone reading the file sees, and it names
the file's own command. The runtime lists and runs a command by its file or
directory name, so keep the two the same.

## Arguments and the Exit Status

Everything after the command's name, including `--help`, belongs to the
command, which reads it with the [`args`](ch20-00-args.md) module. Create a
parser named after the command and call `parse()` without arguments. It
reads the real command line, converts and validates each value, responds to
`--help` based on the declarations, and rejects anything that isn't declared
with a message and exit status 1. That's what gives every command, whether
it comes with Zuri or not, the same flags, the same help layout and the same
error messages:

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
  p.add_option('times', 'How many greetings', {
    short_name: 't',
    type: args.INT,
    value: 1,
  })

  return p
}

var parsed = parser().parse()
var name = parsed.indexes[0]

iter var i = 0; i < parsed.options.times; i++ {
  echo 'Hello, ${name}!'
}
```

The raw list is also available. `os.args` has the same structure for a
command as for `zuri run`: the executable, the command's own file and then
whatever the user typed, so the command's arguments are `os.args[2,]`. But
reading that list by hand is how argument checks and help text get out of
sync, which is exactly what the parser is designed to prevent, so a command
should use `args` rather than `os.args`.

A command exits with status 0 when it runs to the end. An uncaught error
prints its stack trace and exits with status 1, and `os.exit()` exits with
any other status. A command that checks something, as `zuri fmt --dry-run`
checks formatting, should exit with a non-zero status when the check fails,
so that a shell script or a CI job can act on it.

## The Working Directory

A command runs in the directory that `zuri` was started in, so `os.cwd()` is
the user's directory, and for a project command, that's the project's root.
`__file__` is the command's own file, which is how a command can find files
stored next to it:

```zuri,ignore
import os

var template = os.join_paths(os.dir_name(__file__), 'notes.template')
```

## Trying It Out

This example creates a temporary project with a single command, runs it the
way a user would, asks it for its help message, and reads the list that
`zuri --help` shows:

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
      'echo "Hello, " + parser.parse().indexes[0] + "!"\n',
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
  [name]    Who to greet [default: world]

OPTIONS:
  -h, --help  Show this help message and exit
PROJECT COMMANDS:
  greet      Greets whoever is named.
```

## Testing a Command

Create the parser in a function, as the `greet` command above does, and you
can test the argument handling by passing a list to `parse()`, as described
in [Chapter 20](ch20-00-args.md#testing-command-line-parsing). Everything
else is ordinary Zuri, so keep the command's logic in functions and files
next to `index.zu`, and test those.

The commands that come with Zuri keep their tests in a `tests` directory
inside the command, and a project command can do the same. `zuri test` can
run a directory anywhere in the project:

```sh
zuri test .zuri/cmds/release/tests
```
