# Parsing the Command Line

A program started from a shell receives a list of strings and nothing else.
Whatever the user meant by `-v`, `--output report.csv` or
`commit -m "fix the thing"` has to be worked out from that list, and that's
where command-line programs tend to slowly fall apart. The first version
reads `os.args` and checks a few positions. The second adds a flag, and the
checks become a chain of conditions. By the fourth version, nobody can say
what `-vo` does without trying it, the help text no longer matches the code
that reads the flags, and a typo quietly does something else instead of
reporting an error.

The `args` module takes a declarative approach instead. You declare which
options exist, what type of value each one takes and which ones are
required. The module works out what the user typed, converts and validates
the values, generates the help text from the same declarations, and rejects
anything that doesn't match.

- [A First Parser](#a-first-parser)
- [What `parse()` Returns](#what-parse-returns)
- [Options](#options)
  - [Passing a Value](#passing-a-value)
  - [Types](#types)
  - [Defaults and Missing Options](#defaults-and-missing-options)
  - [Requiring an Option](#requiring-an-option)
  - [Restricting the Allowed Values](#restricting-the-allowed-values)
  - [Repeated Options](#repeated-options)
  - [Deprecating an Option](#deprecating-an-option)
- [Positional Arguments](#positional-arguments)
- [Subcommands](#subcommands)
  - [Commands That Take a Value](#commands-that-take-a-value)
  - [Attaching an Action](#attaching-an-action)
- [Command-Line Conventions](#command-line-conventions)
  - [Combining Flags](#combining-flags)
  - [Abbreviations](#abbreviations)
  - [End of Options](#end-of-options)
  - [Reading Arguments From a File](#reading-arguments-from-a-file)
- [Help](#help)
- [Errors](#errors)
- [Testing Command-Line Parsing](#testing-command-line-parsing)
- [Module Reference](#module-reference)

## A First Parser

It takes three lines of declarations and one call:

```zuri
import args

var parser = args.Parser('greet')
parser.add_option('name', 'Who to greet', {
  short_name: 'n',
  type: args.STRING,
})

var parsed = parser.parse(['--name', 'Ada'])

echo parsed.options.name
```

```console
Ada
```

`parse()` without an argument reads the real command line, which is what a
program normally does. Throughout this chapter, it's given an explicit list
instead, because that's also how you test a parser, and because an example
in a book can't depend on how you ran it.

## What `parse()` Returns

It always returns a dictionary with three keys:

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

`options` holds every option that was given on the command line or has a
default. `command` is `nil`, or the subcommand that was given. `indexes`
holds the positional arguments, in the order they were declared. The three
are independent, so a program can use one of them and ignore the others.

A declared positional argument that wasn't given and has no default stays in
`indexes` as `nil`, so the argument after it is still at the index where it
was declared, instead of moving down one place.

## Options

An option is declared by its long name. The short name is optional, and so
is everything else:

```zuri
import args

var parser = args.Parser('tool')
parser.add_option('output', 'Where to write', {
  short_name: 'o',
  type: args.STRING,
})
parser.add_option('force', 'Overwrite without asking', { short_name: 'f' })

echo parser.parse(['-o', 'out.csv', '-f']).options
```

```console
{output: out.csv, force: true}
```

`--output` and `-o` are the same option.

### Passing a Value

A long option takes its value either as the next argument or attached with
`=`. The two forms mean the same thing:

```zuri
import args

def parser() {
  var p = args.Parser('tool')
  p.add_option('output', 'Where to write', {
    short_name: 'o',
    type: args.STRING,
  })
  return p
}

echo parser().parse(['--output', 'out.csv']).options.output
echo parser().parse(['--output=out.csv']).options.output
```

```console
out.csv
out.csv
```

The argument is split at the first `=`, so `--output=a=b` writes to `a=b`
instead of losing half of the name, and `--output=` sets the value to an
empty string instead of leaving it missing. The attached form is also the
only way to pass a value that starts with a dash, because in `--output -5`,
the parser reads `-5` as an option:

```zuri
import args

var parser = args.Parser('seek')
parser.add_option('offset', 'Where to start', { type: args.INT })

echo parser.parse(['--offset=-5']).options.offset
```

```console
-5
```

A short option only takes its value as the next argument, as in
`-o out.csv`, never `-oout.csv` or `-o=out.csv`. A short option token is a
group of single-character flags, and neither `=` nor the letters of a value
are flags.

### Types

The type determines what the string is converted to, and whether the option
takes a value at all.

| Constant | The option takes | The result is |
| --- | --- | --- |
| `args.NONE` | Nothing | `true` when the option is present |
| `args.STRING` | A value | The string itself |
| `args.INT` | A value | A whole number |
| `args.NUMBER` | A value | A number, which can have a fractional part |
| `args.BOOL` | A value | A boolean |
| `args.LIST` | A value, and can be repeated | A list of strings |
| `args.CHOICE` | A value from a fixed set | The string, or the value it maps to |
| `args.OPTIONAL` | A value, if there is one | The string, or `true` |

`NONE` is the default, and it's a plain flag. `BOOL` takes an explicit
value, and it accepts every spelling a user is likely to try:

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

`1`, `true`, `yes`, `y` and `on` all mean true, and `0`, `false`, `no`, `n`
and `off` all mean false.

### Defaults and Missing Options

`value` is the option's value when it isn't given on the command line:

```zuri
import args

var parser = args.Parser('tool')
parser.add_option('count', 'How many', {
  short_name: 'c',
  type: args.INT,
  value: 1,
})
parser.add_option('verbose', 'Say more', { short_name: 'v' })

echo parser.parse([]).options
echo parser.parse(['-c', '5', '-v']).options
```

```console
{count: 1}
{count: 5, verbose: true}
```

If an option has no default and isn't given on the command line, it isn't in
the dictionary at all. That's how you tell a flag that was left out from one
that was set to false, and it's why `verbose` is missing from the first line
instead of being `false`. Use `options.contains('verbose')` to check, or
give the option a default so you don't need to.

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

If the option is left out, the program stops with
`error: required option --output is missing`, followed by the usage line,
and exits with status 1. Nothing else runs.

### Restricting the Allowed Values

`CHOICE` with a list only accepts the values in the list:

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

Any other value stops the program with
`error: --level must be one of quiet, normal or loud, not "shouty"`,
which names the invalid value and lists the alternatives, without you having
to write either.

If you pass a dictionary instead, the user types the key and the program
receives the corresponding value. This is how you turn a short name on the
command line into a more meaningful value inside the program:

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

### Repeated Options

`LIST` collects a value every time the option appears:

```zuri
import args

var parser = args.Parser('tool')
parser.add_option('tag', 'Tag to apply', { short_name: 't', type: args.LIST })

echo parser.parse(['-t', 'urgent', '-t', 'docs', '--tag', 'review']).options.tag
```

```console
[urgent, docs, review]
```

### Deprecating an Option

An option that you no longer want, but can't remove yet, keeps working and
prints a warning:

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

The warning goes to standard error, so the person running the program sees
it, but it doesn't end up in output that another program might be reading.

## Positional Arguments

Positional arguments are declared with `add_index`, and they're filled in
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

They accept `type`, `choices`, `value`, `required` and `metavar`, just as
options do. A positional argument that wasn't declared is an error rather
than being silently ignored, which means a mistyped flag fails with an error
instead of being taken as a file name.

## Subcommands

A program that does several different jobs can give each one a name and its
own options:

```zuri
import args

var parser = args.Parser('notes')
parser.add_option('verbose', 'Say more', { short_name: 'v' })

parser.add_command('list', 'Show every note')
parser.add_command('remove', 'Delete a note').add_option(
  'force',
  'Do not ask first',
  { short_name: 'f' },
)

echo parser.parse(['-v', 'list'])
echo parser.parse(['remove', '-f'])
```

```console
{options: {verbose: true}, command: {name: list, value: nil}, indexes: []}
{options: {force: true}, command: {name: remove, value: nil}, indexes: []}
```

`add_command` returns the command, so you can chain its options directly
after it. A command's options are added to the same `options` dictionary as
the global options, and `command.name` tells you which job the user asked
for.

The order matters, and it's the same order every tool like this uses: the
parser's own options come before the command name, and the command's options
come after it. `notes -v list` works, but `notes list -v` doesn't, because
after `list`, the parser is reading `list`'s options, and `-v` isn't one of
them.

### Commands That Take a Value

A command can take a single value directly, in the way that `git commit -m`
takes a message:

```zuri
import args

var parser = args.Parser('notes')
parser
    .add_command('add', 'Write a note', { type: args.STRING, metavar: 'text' })
    .add_option('pin', 'Keep it at the top', { short_name: 'p' })

echo parser.parse(['add', 'buy milk', '-p'])
```

```console
{options: {pin: true}, command: {name: add, value: buy milk}, indexes: []}
```

The value comes straight after the command name, before the command's own
options. `metavar` is the word the help text shows in place of the value, so
the usage line reads `notes add <text>` instead of `notes add <value>`.

### Attaching an Action

A command can include the function that implements it, which is called after
parsing with the options and the command's value:

```zuri
import args

var parser = args.Parser('notes')

parser
    .add_command('add', 'Write a note', {
      type: args.STRING,
      metavar: 'text',
      action: @(options, value) {
        var prefix = options.contains('pin') ? '[pinned] ' : ''
        echo prefix + value
      },
    })
    .add_option('pin', 'Keep it at the top', { short_name: 'p' })

parser.parse(['add', 'buy milk', '-p'])
```

```console
[pinned] buy milk
```

This is worth using once you have more than two or three commands.
Otherwise, you end up with a chain of comparisons on `command.name` that you
have to keep in sync with the declarations yourself.

## Command-Line Conventions

Users type things that the parser wasn't explicitly told about. These are
the conventions it supports.

### Combining Flags

You can put several flags after a single dash:

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

Every character in the group has to be a declared option. `-vqz` is an
error, rather than `-vq` with the `z` silently ignored, so you find out
about the typo now and not after the program has run.

### Abbreviations

A long option can be shortened to any prefix that's still unambiguous:

```zuri
import args

var parser = args.Parser('tool')
parser.add_option('verbose', 'Say more', { short_name: 'v' })

echo parser.parse(['--verb']).options
```

```console
{verbose: true}
```

Set `parser.allow_abbrev = false` to turn this off. That's worth doing for a
program whose set of options is still growing, because a prefix that's
unambiguous today stops being unambiguous as soon as someone adds
`--version`, and any script written with the short form breaks.

### End of Options

A `--` on its own ends option parsing. Everything after it is a positional
argument, even if it starts with a dash:

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

This is how a program can pass arguments on to another program without
needing to understand them.

### Reading Arguments From a File

An argument that starts with `@` is replaced by the contents of that file,
with one argument per line:

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

The file's contents are inserted in place, so any arguments after it still
take priority. This is useful when a command line becomes too long for a
shell to handle comfortably, and for arguments that a program would rather
not put where `ps` can see them. Set `parser.allow_atfile = false` to turn
it off, for a program that should treat a leading `@` as ordinary text.

## Help

The help text is generated from the declarations, so it always matches what
the program accepts:

```zuri
import args

var parser = args.Parser('greet')
parser.set_terminal_width(68)
parser.description = 'Greet somebody, once or several times.'
parser.epilog = 'Set NO_COLOR to turn off colour.'

parser.add_option('name', 'Who to greet', {
  short_name: 'n',
  type: args.STRING,
})
parser.add_option('count', 'How many times', {
  short_name: 'c',
  type: args.INT,
  value: 1,
})
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
  -c, --count <count>  How many times [default: 1]

COMMANDS:
  history  Show past greetings

Set NO_COLOR to turn off colour.

Run "greet --help [COMMAND]" for help on a specific command.

```

`-h` and `--help` are declared for you and work wherever they appear. After
a command, they describe that command instead of the whole program. `help()`
does the same thing when you call it directly. Both print the help text and
exit with status 0.

Colors are used when standard output is a terminal and `NO_COLOR` isn't set,
so output that's piped or redirected is plain text without escape codes.
`set_terminal_width()` sets the width the text wraps at, which is what the
example above does, so that the output is the same in every terminal. If you
don't set it, the parser asks the terminal for its width, then falls back to
`COLUMNS`, and then to 80.

If a parser's command line didn't match anything at all, it prints its help
text and continues. An option with a default counts as a match, so this
never happens for a parser with any defaults. To turn this behavior off
completely, pass `false` as the second argument to `args.Parser`.

## Errors

Every error is reported the same way: a line on standard error starting with
`error:`, followed by the usage text, and an exit status of 1.

| What happened | The message |
| --- | --- |
| An option that wasn't declared | `unknown option: --colour` |
| An option that needs a value was the last argument | `option --name expects <name>` |
| A required option was left out | `required option --output is missing` |
| A value that isn't in `choices` | `--level must be one of quiet or loud, not "shouty"` |
| A required positional argument was left out | `required positional argument <source> is missing` |
| An argument that doesn't match anything | `unexpected argument: report.csv` |

None of these raise an exception. When a command-line program receives
something it can't use, there's nothing useful left for it to do, and a
stack trace in the user's terminal tells them less than a one-line message.
The errors that *do* raise an exception are mistakes in the declarations,
such as a duplicate option name, a short name that's already used, or
`choices` that's neither a list nor a dictionary. Those are bugs in the
program rather than mistakes by the user, so they raise an `ArgsError` or a
`TypeError` from the `add_option` call that caused them.

## Testing Command-Line Parsing

`parse()` accepts an explicit list, which is what makes it testable:

```zuri
import args

def build() {
  var parser = args.Parser('greet')
  parser.add_option('name', 'Who to greet', {
    short_name: 'n',
    type: args.STRING,
  })
  parser.add_option('count', 'How many times', {
    short_name: 'c',
    type: args.INT,
    value: 1,
  })
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

Create the parser in a function, so that each test gets a new one. A parser
keeps the results of its last parse, so sharing one between tests makes them
depend on the order they run in. To test the paths that end in `os.exit()`,
which are help and every error above, you need to run a real subprocess,
which is what `os.exec()` is for. [Chapter 23](ch23-00-testing.md) covers
testing in more detail.

## Module Reference

Creating a parser:

| Method | Description |
| --- | --- |
| `Parser(name, default_help)` | Creates a new parser |
| `add_option(name, help, opts)` | Adds an option, either globally or to a command |
| `add_command(name, help, opts)` | Adds a subcommand and returns it, for chaining |
| `add_index(name, help, opts)` | Adds a positional argument |
| `parse(custom_args)` | Parses the command line, or the given list |
| `help()` | Prints the help text and exits with status 0 |
| `set_terminal_width(width)` | Sets the width that text wraps at |

Properties you can set after creating a parser:

| Property | Description |
| --- | --- |
| `description`, `epilog` | Text shown before and after the options |
| `allow_abbrev` | Whether unambiguous prefixes of long options are accepted. The default is `true`. |
| `allow_atfile` | Whether `@file` arguments are expanded. The default is `true`. |
| `terminal_width` | The width that text wraps at |

Keys you can use in `opts`:

| Key | Description |
| --- | --- |
| `short_name` | The single-character form of the option |
| `type` | One of the type constants |
| `value` | The value when the option isn't given |
| `choices` | A list of allowed values, or a dictionary that maps keys to values |
| `required` | Whether the program refuses to run without it |
| `metavar` | The word the help text shows in place of the value |
| `deprecated` | Prints a warning to standard error when the option is used |
| `action` | For a command, the function to run after parsing |

The type constants:

| Constants | Description |
| --- | --- |
| `NONE`, `STRING`, `INT`, `NUMBER` | A flag, and the three plain value types |
| `BOOL`, `LIST`, `CHOICE`, `OPTIONAL` | A yes or no answer, a repeated value, a value from a fixed set, and an optional value |
