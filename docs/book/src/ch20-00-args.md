# Parsing the Command Line

A program started from a shell is handed a list of strings and nothing
else. Everything a user meant by `-v`, `--output report.csv` or
`commit -m "fix the thing"` has to be recovered from that list, and the
recovering is where command-line programs quietly rot. The first
version reads `os.args` and checks a few positions. The second adds a
flag, and the checks become a chain of conditions. By the fourth nobody
can say what `-vo` does without running it, the help text has drifted
from the code that reads the flags, and a typo silently means something
else instead of saying so.

The `args` module takes the declaration instead. You say which options
exist, what type each one carries and which are required; it works out
what the user typed, converts it, validates it, writes the help text
from the same declarations, and refuses anything that does not fit.

- [The First Parser](#the-first-parser)
- [What `parse()` Returns](#what-parse-returns)
- [Options](#options)
  - [Attaching a Value](#attaching-a-value)
  - [Types](#types)
  - [Defaults and Absence](#defaults-and-absence)
  - [Requiring an Option](#requiring-an-option)
  - [Restricting the Value](#restricting-the-value)
  - [Collecting Repeats](#collecting-repeats)
  - [Retiring an Option](#retiring-an-option)
- [Positional Arguments](#positional-arguments)
- [Sub-commands](#sub-commands)
  - [A Value of Its Own](#a-value-of-its-own)
  - [Running Something Directly](#running-something-directly)
- [The Shapes a Command Line Can Take](#the-shapes-a-command-line-can-take)
  - [Bundling](#bundling)
  - [Abbreviation](#abbreviation)
  - [End of Options](#end-of-options)
  - [Arguments From a File](#arguments-from-a-file)
- [Help](#help)
- [When It Does Not Fit](#when-it-does-not-fit)
- [Testing a Command Line](#testing-a-command-line)
- [Module Reference](#module-reference)

## The First Parser

Three lines of declaration and a call:

```zuri
import args

var parser = args.Parser('greet')
parser.add_option('name', 'Who to greet', { short_name: 'n', type: args.STRING })

var parsed = parser.parse(['--name', 'Ada'])

echo parsed.options.name
```

```console
Ada
```

`parse()` with no argument reads the real command line, which is what a
program does. Everywhere in this chapter it is given an explicit list
instead, because that is also how you test one, and because a book
example cannot rely on how you invoked it.

## What `parse()` Returns

One dictionary with three keys, always:

```zuri
import args

var parser = args.Parser('tool')
parser.add_option('verbose', 'Say more', { short_name: 'v' })
parser.add_index('source', 'The file to read')
parser.add_command('check', 'Check it over')

echo parser.parse(['-v', 'check', 'notes.txt'])
```

```console
{options: {verbose: true}, command: {name: check, value: nil}, indexes: [notes.txt]}
```

`options` holds every option that was supplied or has a default.
`command` is `nil` or the sub-command that was named. `indexes` holds
the positional arguments in the order they were declared. The three are
independent: a program can use one of them and ignore the rest.

A declared positional that nobody filled and that has no default keeps
its place in `indexes` as a `nil`, so the argument after it is still
found at the index it was declared at rather than sliding down one.

## Options

An option is declared by its long name. The short name is optional, and
so is everything else:

```zuri
import args

var parser = args.Parser('tool')
parser.add_option('output', 'Where to write', { short_name: 'o', type: args.STRING })
parser.add_option('force', 'Overwrite without asking', { short_name: 'f' })

echo parser.parse(['-o', 'out.csv', '-f']).options
```

```console
{output: out.csv, force: true}
```

`--output` and `-o` are the same option.

### Attaching a Value

A long option takes its value either as the next argument or attached
with `=`. The two spellings mean the same thing:

```zuri
import args

def parser() {
  var p = args.Parser('tool')
  p.add_option('output', 'Where to write', { short_name: 'o', type: args.STRING })
  return p
}

echo parser().parse(['--output', 'out.csv']).options.output
echo parser().parse(['--output=out.csv']).options.output
```

```console
out.csv
out.csv
```

The split is on the first `=`, so `--output=a=b` writes to `a=b` rather
than losing half the name, and `--output=` is an explicit empty string
rather than a missing value. The attached form is also the only way to
pass a value that begins with a dash, because in `--output -5` the
parser reads `-5` as an option:

```zuri
import args

var parser = args.Parser('seek')
parser.add_option('offset', 'Where to start', { type: args.INT })

echo parser.parse(['--offset=-5']).options.offset
```

```console
-5
```

A short option takes its value as the next argument only: `-o out.csv`,
never `-oout.csv` and never `-o=out.csv`. A short token is a bundle of
single-character flags, and neither `=` nor the letters of a value are
flags.

### Types

The type decides what the string becomes and whether a value is taken
at all.

| Constant | The option takes | Becomes |
| --- | --- | --- |
| `args.NONE` | nothing | `true` when present |
| `args.STRING` | a value | the string itself |
| `args.INT` | a value | a whole number |
| `args.NUMBER` | a value | a number, fractions allowed |
| `args.BOOL` | a value | a boolean |
| `args.LIST` | a value, repeatable | a list of strings |
| `args.CHOICE` | a value from a fixed set | the string, or what it maps to |
| `args.OPTIONAL` | a value, if one is there | the string, or `true` |

`NONE` is the default and is the plain flag. `BOOL` is the one that
takes an explicit answer, and it accepts every spelling a user is
likely to reach for:

```zuri
import args

def parser() {
  var p = args.Parser('tool')
  p.add_option('colour', 'Use colour', { short_name: 'c', type: args.BOOL })
  return p
}

echo parser().parse(['-c', 'yes']).options.colour
echo parser().parse(['-c', 'off']).options.colour
echo parser().parse(['-c', '0']).options.colour
```

```console
true
false
false
```

`1`, `true`, `yes`, `y` and `on` all mean true; `0`, `false`, `no`, `n`
and `off` all mean false.

### Defaults and Absence

`value` is what the option is worth when nobody supplies it:

```zuri
import args

var parser = args.Parser('tool')
parser.add_option('count', 'How many', { short_name: 'c', type: args.INT, value: 1 })
parser.add_option('verbose', 'Say more', { short_name: 'v' })

echo parser.parse([]).options
echo parser.parse(['-c', '5', '-v']).options
```

```console
{count: 1}
{count: 5, verbose: true}
```

An option with no default and no value on the command line is not in
the dictionary at all. That is the difference between a flag that was
left off and one that was set to false, and it is why `verbose` is
missing from the first line rather than sitting there as `false`. Use
`options.contains('verbose')` to ask, or give the option a default and
stop having to.

### Requiring an Option

```zuri
import args

var parser = args.Parser('tool')
parser.add_option('output', 'Where to write', {
  short_name: 'o',
  type: args.STRING,
  required: true,
})

echo parser.parse(['-o', 'out.csv']).options.output
```

```console
out.csv
```

Leave it off and the program stops with `error: required option
--output is missing`, the usage line, and exit status 1. Nothing else
runs.

### Restricting the Value

`CHOICE` with a list accepts only what is in the list:

```zuri
import args

var parser = args.Parser('tool')
parser.add_option('level', 'How loud', {
  type: args.CHOICE,
  choices: ['quiet', 'normal', 'loud'],
})

echo parser.parse(['--level', 'loud']).options.level
```

```console
loud
```

Anything else stops the program with `error: --level expects one of
{'quiet', 'normal', 'loud'}, got "shouty"`, which names the offender
and lists the alternatives without you writing either.

Give it a dictionary instead and the user types the key while the
program receives the value. This is how a short spelling on the command
line becomes a meaningful value inside:

```zuri
import args

var parser = args.Parser('tool')
parser.add_option('mode', 'What to do', {
  short_name: 'm',
  type: args.CHOICE,
  choices: { r: 'read', w: 'write', rw: 'read-write' },
})

echo parser.parse(['-m', 'rw']).options.mode
```

```console
read-write
```

### Collecting Repeats

`LIST` accumulates every time the option appears:

```zuri
import args

var parser = args.Parser('tool')
parser.add_option('tag', 'Tag to apply', { short_name: 't', type: args.LIST })

echo parser.parse(['-t', 'urgent', '-t', 'docs', '--tag', 'review']).options.tag
```

```console
[urgent, docs, review]
```

### Retiring an Option

An option you no longer want but cannot remove yet keeps working and
says so on stderr:

```zuri
import args

var parser = args.Parser('tool')
parser.add_option('old-name', 'Use --name instead', {
  type: args.STRING,
  deprecated: true,
})

echo parser.parse(['--old-name', 'Ada']).options['old-name']
```

```console
Ada
```

The warning goes to stderr, so it reaches the person running the
program without contaminating output that something else is reading.

## Positional Arguments

A positional argument is declared with `add_index`, and they fill in
the order they were declared:

```zuri
import args

var parser = args.Parser('copy')
parser.add_index('source', 'The file to read', { required: true })
parser.add_index('dest', 'Where to write it', { value: 'out.txt' })

echo parser.parse(['in.csv']).indexes
echo parser.parse(['in.csv', 'report.csv']).indexes
```

```console
[in.csv, out.txt]
[in.csv, report.csv]
```

They take `type`, `choices`, `value`, `required` and `metavar`, the
same as options do. A positional that is neither declared nor expected
is an error rather than something quietly ignored, which is what makes
a mistyped flag fail loudly instead of being swallowed as a filename.

## Sub-commands

A program with several jobs gives each one a name and its own options:

```zuri
import args

var parser = args.Parser('notes')
parser.add_option('verbose', 'Say more', { short_name: 'v' })

parser.add_command('list', 'Show every note')
parser.add_command('remove', 'Delete a note').
  add_option('force', 'Do not ask first', { short_name: 'f' })

echo parser.parse(['-v', 'list'])
echo parser.parse(['remove', '-f'])
```

```console
{options: {verbose: true}, command: {name: list, value: nil}, indexes: []}
{options: {force: true}, command: {name: remove, value: nil}, indexes: []}
```

`add_command` returns the command, so its options chain straight off
it. A command's own options land in the same `options` dictionary as
the global ones; `command.name` is what tells you which job was asked
for.

Order matters, and it is the order every tool of this shape uses: the
parser's own options come before the command name, and the command's
options come after it. `notes -v list` works and `notes list -v` does
not, because after `list` the parser is reading `list`'s options and
`-v` is not one of them.

### A Value of Its Own

A command can take one value directly, the way `git commit -m` takes a
message:

```zuri
import args

var parser = args.Parser('notes')
parser.add_command('add', 'Write a note', {
  type: args.STRING,
  metavar: 'text',
}).add_option('pin', 'Keep it at the top', { short_name: 'p' })

echo parser.parse(['add', 'buy milk', '-p'])
```

```console
{options: {pin: true}, command: {name: add, value: buy milk}, indexes: []}
```

The value comes immediately after the command name, before the
command's own options. `metavar` is the word the help text shows in
place of the value, so the usage line reads `notes add <text>` rather
than `notes add <value>`.

### Running Something Directly

A command can carry the function that implements it, called after
parsing with the options and the command's value:

```zuri
import args

var parser = args.Parser('notes')

parser.add_command('add', 'Write a note', {
  type: args.STRING,
  metavar: 'text',
  action: @(options, value) {
    var prefix = options.contains('pin') ? '[pinned] ' : ''
    echo prefix + value
  },
}).add_option('pin', 'Keep it at the top', { short_name: 'p' })

parser.parse(['add', 'buy milk', '-p'])
```

```console
[pinned] buy milk
```

This is worth reaching for once there are more than two or three
commands, because the alternative is a chain of comparisons on
`command.name` that has to be kept in step with the declarations by
hand.

## The Shapes a Command Line Can Take

Users type things the parser was not asked about directly. These are
the conventions it honours.

### Bundling

Several flags behind one dash:

```zuri
import args

var parser = args.Parser('tool')
parser.add_option('verbose', 'Say more', { short_name: 'v' })
parser.add_option('quiet', 'Say less', { short_name: 'q' })

echo parser.parse(['-vq']).options
```

```console
{verbose: true, quiet: true}
```

Every character in a bundle has to name an option. `-vqz` is an error
rather than `-vq` with the `z` quietly dropped, which is the difference
between a typo you find now and one you find after the run.

### Abbreviation

A long option may be shortened to any prefix that is still
unambiguous:

```zuri
import args

var parser = args.Parser('tool')
parser.add_option('verbose', 'Say more', { short_name: 'v' })

echo parser.parse(['--verb']).options
```

```console
{verbose: true}
```

Set `parser.allow_abbrev = false` to turn this off, which is worth
doing for a program whose option set is still growing: a prefix that is
unambiguous today stops being so the day somebody adds `--version`, and
a script written against the short form breaks.

### End of Options

A bare `--` ends option parsing. Everything after it is positional,
even when it starts with a dash:

```zuri
import args

var parser = args.Parser('run')
parser.add_option('verbose', 'Say more', { short_name: 'v' })
parser.add_index('command', 'What to run')
parser.add_index('argument', 'What to pass it')

echo parser.parse(['-v', '--', 'grep', '-i']).indexes
```

```console
[grep, -i]
```

This is how a program passes arguments through to another one without
having to know what they mean.

### Arguments From a File

A token beginning with `@` is replaced by the contents of that file,
one argument per line:

```zuri
import args

file('greet.args', 'w').write('--name\nAda\n--count\n3\n')

var parser = args.Parser('greet')
parser.add_option('name', 'Who to greet', { type: args.STRING })
parser.add_option('count', 'How many times', { type: args.INT, value: 1 })

echo parser.parse(['@greet.args']).options
echo parser.parse(['@greet.args', '--name', 'Grace']).options
```

```console
{name: Ada, count: 3}
{name: Grace, count: 3}
```

The file is expanded in place, so anything after it still wins. This is
the escape hatch for a command line that has outgrown what a shell will
comfortably hold, and for arguments a program would rather not put
where `ps` can read them. Set `parser.allow_atfile = false` to turn it
off for a program that should treat a leading `@` as ordinary text.

## Help

The help text is written from the declarations, so it cannot drift
away from what the program accepts:

```zuri
import args

var parser = args.Parser('greet')
parser.set_terminal_width(68)
parser.description = 'Greet somebody, once or several times.'
parser.epilog = 'Set NO_COLOR to turn off colour.'

parser.add_option('name', 'Who to greet', { short_name: 'n', type: args.STRING })
parser.add_option('count', 'How many times', { short_name: 'c', type: args.INT, value: 1 })
parser.add_index('output', 'Where to write the greeting')

parser.add_command('history', 'Show past greetings')

parser.help()
```

```console
Usage: greet [OPTIONS] [COMMAND] [output]

  Greet somebody, once or several times.

POSITIONAL ARGUMENTS:
  [output]  Where to write the greeting

OPTIONS:
  -h, --help           Show this help message and exit
  -n, --name <name>    Who to greet
  -c, --count <count>  How many times (default: 1)

COMMANDS:
  history  Show past greetings

Set NO_COLOR to turn off colour.

Run "greet --help [COMMAND]" for help on a specific command.

```

`-h` and `--help` are declared for you and handled wherever they
appear, including after a command, where they describe that command
instead of the whole program. `help()` is the same thing called
directly; both print and exit 0.

Colour is used when stdout is a terminal and `NO_COLOR` is unset, so a
run whose output is piped or redirected gets plain text rather than
escape codes. `set_terminal_width()` fixes the wrapping width, which is
what the example above does so the output is the same on every
terminal; left alone, the parser asks the terminal and falls back to
`COLUMNS`, then to 80.

A parser whose command line matched nothing at all prints its help and
carries on. An option with a default counts as a match, so a parser
that defaults anything never does this; pass `false` as the second
argument to `args.Parser` to turn the behaviour off outright.

## When It Does Not Fit

Every failure follows the same shape: a line on stderr beginning
`error:`, the usage text, and exit status 1.

| What happened | What it says |
| --- | --- |
| an option nobody declared | `unknown option: --colour` |
| a value-taking option at the end | `option --name expects <name>` |
| a required option left off | `required option --output is missing` |
| a value outside `choices` | `--level expects one of {'quiet', 'loud'}, got "shouty"` |
| a required positional left off | `required positional argument <source> is missing` |
| an argument that fits nothing | `unexpected argument: report.csv` |

None of these raise. A command-line program that has been handed
something it cannot use has nothing useful left to do, and unwinding a
stack trace into a user's terminal tells them less than one line does.
The errors that *do* raise are the ones in the declarations — a
duplicate option name, a short name already taken, a `choices` that is
neither a list nor a dictionary — because those are bugs in the program
rather than mistakes by its user, and they raise `ArgsError` or a
`TypeError` at the `add_option` that caused them.

## Testing a Command Line

`parse()` takes an explicit list, and that is the seam:

```zuri
import args

def build() {
  var parser = args.Parser('greet')
  parser.add_option('name', 'Who to greet', { short_name: 'n', type: args.STRING })
  parser.add_option('count', 'How many times', { short_name: 'c', type: args.INT, value: 1 })
  return parser
}

var parsed = build().parse(['-n', 'Ada', '-c', '3'])

assert parsed.options.name == 'Ada', 'the name is read'
assert parsed.options.count == 3, 'the count is coerced to a number'
assert build().parse([]).options.count == 1, 'the default applies'

echo 'all good'
```

```console
all good
```

Build the parser in a function so each test gets a clean one; a parser
carries the results of the last parse, and sharing one between tests
makes them depend on their order. The paths that end in `os.exit()` —
help, and every error above — need a real subprocess to observe, which
is what `os.exec()` is for; [Chapter 23](ch23-00-testing.md) covers the
rest of testing.

## Module Reference

Building a parser:

| | |
| --- | --- |
| `Parser(name, default_help)` | a new parser |
| `add_option(name, help, opts)` | an option, global or on a command |
| `add_command(name, help, opts)` | a sub-command, returned for chaining |
| `add_index(name, help, opts)` | a positional argument |
| `parse(custom_args)` | read the command line, or a list |
| `help()` | print the help text and exit 0 |
| `set_terminal_width(width)` | fix the wrapping width |

Properties you can set after construction:

| | |
| --- | --- |
| `description`, `epilog` | prose above and below the options |
| `allow_abbrev` | unambiguous long-option prefixes, default `true` |
| `allow_atfile` | `@file` expansion, default `true` |
| `terminal_width` | the wrapping width directly |

Keys `opts` understands:

| | |
| --- | --- |
| `short_name` | the single-character form |
| `type` | one of the type constants |
| `value` | what it is worth when absent |
| `choices` | a list of allowed values, or a map from key to value |
| `required` | refuse to run without it |
| `metavar` | the word help shows in place of the value |
| `deprecated` | warn on stderr when it is used |
| `action` | on a command, the function to run after parsing |

The type constants:

| | |
| --- | --- |
| `NONE`, `STRING`, `INT`, `NUMBER` | a flag, and the three plain values |
| `BOOL`, `LIST`, `CHOICE`, `OPTIONAL` | an answer, a repeat, a fixed set, a maybe |
