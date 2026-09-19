# Configuration and the Environment

Every program that talks to anything else needs to be told where it is.
A database address, an API key, a port to bind, a flag that turns
verbose logging on for one afternoon. None of that belongs in the
source: it changes between your laptop and the server, and some of it
must never reach a repository at all.

The answer the industry settled on is the process environment.
Orchestrators set it, CI runners set it, shells set it, and every
language can read it. What none of them solve is the first machine in
the chain: yours, where there is no orchestrator and nobody wants to
prefix every run with eight assignments.

The `env` module closes that gap. It reads a `.env` file into the
process environment at startup, and it reads values back out already
converted to the number, boolean or list your program is going to use.
The file is a development convenience; the environment is the
interface. A program written against `env` does not know or care which
one supplied a value, which is exactly what lets it run unchanged in
both places.

- [The First Line](#the-first-line)
- [Writing a `.env` File](#writing-a-env-file)
  - [Quoting](#quoting)
  - [Comments](#comments)
  - [Values That Span Lines](#values-that-span-lines)
- [What Is Already Set Wins](#what-is-already-set-wins)
  - [Overriding](#overriding)
  - [Empty Means Unconfigured](#empty-means-unconfigured)
- [References Between Values](#references-between-values)
  - [Fallbacks and Demands](#fallbacks-and-demands)
  - [Literal Dollar Signs](#literal-dollar-signs)
- [Reading Values Back](#reading-values-back)
- [Building a Load](#building-a-load)
  - [Layering Files](#layering-files)
  - [Text Instead of a File](#text-instead-of-a-file)
  - [Demanding the File](#demanding-the-file)
  - [Looking Without Loading](#looking-without-loading)
- [Writing a File Back Out](#writing-a-file-back-out)
- [When It Goes Wrong](#when-it-goes-wrong)
- [Keeping Secrets Out of Git](#keeping-secrets-out-of-git)
- [Module Reference](#module-reference)

## The First Line

One call, as early in the program as you can put it:

```zuri
import env

file('.env.demo', 'w').write('PORT=8080\nGREETING=hello\n')

env.load('.env.demo')

echo env.get('GREETING')
echo env.int('PORT')
```

```console
hello
8080
```

`env.load()` with no argument reads `.env` in the current working
directory, which is what almost every program wants. The examples in
this chapter write their file first and name it, so that you can run
each one as it stands.

The file is optional. If it is not there, `load()` leaves the
environment exactly as it found it and the program carries on with
whatever the shell already provided. That is deliberate: the same
binary runs on a laptop with a `.env` file and on a server without one.

## Writing a `.env` File

One name per line, a `=`, and a value:

```dosini
HOST=127.0.0.1
PORT=8080
DATABASE_URL=postgres://app@localhost/app
```

A name is a letter or underscore followed by letters, digits and
underscores, which is the same rule a shell applies. Anything else
stops the load with a `ParseError` rather than being silently dropped,
because a name no shell could ever export is a typo every time.

A leading `export` is allowed and ignored, so the same file can be fed
to `source` in a terminal when you want the variables in your shell
too.

### Quoting

An unquoted value is the text up to the end of the line, with the
whitespace trimmed off both ends. Wrap it in quotes when that is not
what you want:

```zuri
import env

var values = env.parse(
  'BARE=  plain text  \n' +
  'COMMENTED=value # not part of it\n' +
  'PASSWORD=hunter#2\n' +
  "RAW='no \\n escape, no expansion'\n" +
  'COOKED="caf\\u00e9"\n'
)

for name, value in values {
  echo '${name} -> [${value}]'
}
```

```console
BARE -> [plain text]
COMMENTED -> [value]
PASSWORD -> [hunter#2]
RAW -> [no \n escape, no expansion]
COOKED -> [café]
```

The three quoting forms differ in exactly one way each:

| Written | Means |
| --- | --- |
| `KEY=value` | trimmed, ends at a comment or the line |
| `KEY='value'` | every character as written, nothing resolved |
| `` KEY=`value` `` | the same, for values containing both other quotes |
| `KEY="value"` | backslash escapes resolved, `$` references expanded |

Inside double quotes, `\n`, `\r`, `\t`, `\f`, `\v`, `\b`, `\a`, `\e`
and `\0` mean what they do in Zuri, and `\xHH`, `\uHHHH` and `\u{H...}`
name a codepoint. A backslash before anything else keeps both
characters, so `"C:\Users\me"` is the path you meant and not a lesson
in escaping.

Reach for single quotes whenever a value is a secret. A generated
password containing `$` or `\` goes through untouched, and nothing in
it can accidentally name another variable.

### Comments

A `#` starts a comment, either on its own line or after a value. Inside
an unquoted value it only does so when it begins the value or follows a
space, which is why `PASSWORD=hunter#2` above kept its `#`. Where that
rule is too subtle to rely on, quote the value and the question does
not arise.

### Values That Span Lines

A quoted value runs until its closing quote, newlines included. This is
how a private key goes in a file:

```dosini
SIGNING_KEY="-----BEGIN PRIVATE KEY-----
MIIEvQIBADANBgkqhkiG9w0BAQEFAASCBKcwggSjAgEAAoIBAQC7...
-----END PRIVATE KEY-----"
```

Single quotes work the same way and resolve nothing, which is usually
the better choice for key material.

## What Is Already Set Wins

A name that is already set in the process environment is left alone.
The file fills in the rest:

```zuri
import env
import os

os.set_env('DATABASE_URL', 'postgres://production/app', true)

file('.env.demo', 'w').write(
  'DATABASE_URL=postgres://localhost/app\n' +
  'CACHE_TTL=300\n'
)

var result = env.load('.env.demo')

echo os.get_env('DATABASE_URL')
echo result.applied
echo result.skipped
```

```console
postgres://production/app
[CACHE_TTL]
[DATABASE_URL]
```

This is the whole point of the default. The file carries what a
developer needs to run the program at all; production sets the real
values through the shell, the orchestrator or the CI runner, and the
file never has to know that it did.

`load()` returns a `Result` that accounts for every name and every
file, so a program never has to guess whether its configuration
arrived. `applied` is what the load set, `skipped` is what was already
set, `values` is everything the file defined either way, and `files`
and `missing` say which paths were read and which were not there.

### Overriding

Where the file genuinely should win, say so:

```zuri
import env
import os

os.set_env('LOG_LEVEL', 'warn', true)

file('.env.demo', 'w').write('LOG_LEVEL=debug\n')

env.loader().path('.env.demo').override().load()

echo os.get_env('LOG_LEVEL')
```

```console
debug
```

### Empty Means Unconfigured

Throughout the module, a variable set to the empty string counts as
unset. `FLAG=` in a file, an empty shell variable, and a name nobody
ever set all mean the same thing, because that is the one thing anybody
means by any of them. A load will fill in an empty name, `has()` is
false for it, `require()` raises on it, and every typed reader returns
its default.

`os.get_env()` is the unfiltered view for the rare case that needs to
tell an empty value apart from an absent one.

## References Between Values

A value may refer to another value, or to the environment around it,
with the syntax a shell uses:

```dosini
HOST=localhost
PUBLIC_URL=http://${HOST}:${PORT:-8080}
```

```zuri
import env

file('.env.demo', 'w').write(
  'HOST=localhost\n' +
  'PUBLIC_URL=http://$' + '{HOST}:$' + '{PORT:-8080}\n'
)

var loaded = env.load('.env.demo')

echo loaded.values.PUBLIC_URL
```

```console
http://localhost:8080
```

The split string in that example is a Zuri detail, not a `.env` one: a
`${` written inside a Zuri literal is an interpolation, so building
`.env` text in source means keeping the `$` and the `{` apart. A file
you type in an editor has no such problem, as the `dosini` block above
shows.

A reference resolves to the value the name will hold *once the load has
finished*. That single rule is what makes `PORT=${PORT:-8080}` read the
way it looks: whatever the environment already says, and 8080 when it
says nothing. It holds no matter which line of the file the reference
sits on, and it changes with `override()` exactly as precedence does.

A definition may also reach past itself to the value it is replacing,
which is how a path gets extended rather than clobbered:

```dosini
PATH=${PATH}:/opt/app/bin
```

### Fallbacks and Demands

The three modifiers are POSIX's, and the leading colon on each widens
the test from "is it set" to "is it set to something":

| Written | Means |
| --- | --- |
| `${NAME:-fallback}` | the value, or `fallback` when it is unset or empty |
| `${NAME-fallback}` | the value, or `fallback` when it is unset |
| `${NAME:+instead}` | `instead` when the value is set and not empty, otherwise nothing |
| `${NAME+instead}` | `instead` when the value is set, otherwise nothing |
| `${NAME:?reason}` | the value, or a `MissingVariable` carrying `reason` |
| `${NAME?reason}` | the same, counting an empty value as set |

The text after a modifier is itself expanded, so a fallback may have a
fallback:

```dosini
CACHE_DIR=${XDG_CACHE_HOME:-${HOME}/.cache}/app
```

`:?` is worth knowing. It turns the file itself into the place where a
deployment's requirements are written down, and the failure arrives at
startup with the reason attached:

```dosini
SESSION_SECRET=${SESSION_SECRET:?the deploy must supply a session secret}
```

### Literal Dollar Signs

`\$` is a dollar sign and nothing else. A `$` that does not name
anything, as in `PRICE=$5.00`, stands for itself already. A value in
single quotes or backticks is never expanded at all, so a secret
containing `${` needs no thought:

```dosini
API_KEY='sk_live_${not_a_reference}'
```

## Reading Values Back

An environment variable is always text, and almost nothing in a program
wants text. The typed readers convert it and refuse anything that is
not what it claims to be:

```zuri
import env

file('.env.demo', 'w').write(
  'PORT=8080\n' +
  'DEBUG=yes\n' +
  'REQUEST_TIMEOUT=2.5\n' +
  'CORS_ORIGINS= https://a.example , https://b.example \n' +
  'SENTRY_DSN=\n'
)

env.load('.env.demo')

echo env.int('PORT', 3000)
echo env.bool('DEBUG', false)
echo env.float('REQUEST_TIMEOUT', 1)
echo env.list('CORS_ORIGINS', ',', [])
echo env.get('SENTRY_DSN', 'not configured')
echo env.has('SENTRY_DSN')
```

```console
8080
true
2.5
[https://a.example, https://b.example]
not configured
false
```

| | |
| --- | --- |
| `get(name, default)` | the text, or the default |
| `require(name, reason)` | the text, or a `MissingVariable` |
| `has(name)` | whether it is configured at all |
| `int(name, default)` | a decimal integer, optionally signed |
| `float(name, default)` | a number, exponents included |
| `bool(name, default)` | `1`, `true`, `yes`, `y`, `on` and their opposites |
| `list(name, separator, default)` | split, trimmed, empty items dropped |

A value that is set but not convertible is a `ValueError` naming the
variable, not a silent fall back to the default. `PORT=eighty` is a
mistake in the configuration, and the moment to hear about it is
startup rather than the first request that needed a port.

These read the process environment, not the file. They answer the same
whether a value came from `.env` or from the shell, which is what lets
one program run in both places without a branch anywhere in it.

`require()` deserves its own line in most programs. A setting the code
cannot invent a default for should stop the program at the top, with
its own name in the message:

```zuri,ignore
var secret = env.require('SESSION_SECRET', 'set SESSION_SECRET in .env')
```

## Building a Load

`env.loader()` returns a `Loader` when the one-line form is not enough.
Every setting returns the loader, so a whole configuration is one
expression, and nothing about the loader changes when it runs, so the
same one can be kept and used again.

### Layering Files

Sources are read in the order they are added, and the last one to
define a name is the one that defines it:

```zuri
import env

file('.env.demo', 'w').write('HOST=localhost\nPORT=8080\n')
file('.env.local.demo', 'w').write('HOST=127.0.0.1\n')

var result = env.loader()
  .path('.env.demo')
  .path('.env.local.demo')
  .path('.env.missing.demo')
  .read()

echo result.values
echo result.files
echo result.missing
```

```console
{HOST: 127.0.0.1, PORT: 8080}
[.env.demo, .env.local.demo]
[.env.missing.demo]
```

That pairing is the useful one: `.env` holds what the team shares and
is worth committing as `.env.example`, and `.env.local` holds what one
machine does differently and is not. A path may begin with `~`, which
expands to the home directory.

### Text Instead of a File

`source()` adds text rather than a path, so configuration that arrived
over the network or out of a secret store goes through exactly the same
parsing, expansion and precedence as a file on disk:

```zuri,ignore
env.loader()
  .path('.env')
  .source(vault.fetch('app/production'))
  .load()
```

### Demanding the File

A file that is not there is not an error by default, and its path is
recorded in `Result.missing`. Where the file is genuinely part of the
deployment, say so and let it fail loudly:

```zuri,ignore
env.loader().path('/etc/app/env').required().load()
```

### Looking Without Loading

`read()` does everything `load()` does except the last step. Nothing is
written to the process environment, and `$` references still resolve
the way they would have, so what comes back is exactly what `load()`
would have set. It is how a tool inspects a file, and how a test checks
one without changing the process it is running in.

`expand(false)` turns expansion off entirely, for a file of opaque
secrets where every value should be taken exactly as written.

## Writing a File Back Out

`stringify()` is `parse()` backwards, for the program that generates a
`.env` file rather than reading one:

```zuri
import env

print(env.stringify({
  HOST: '127.0.0.1',
  PORT: 8080,
  GREETING: 'hello there',
  DEBUG: false,
}))
```

```console
HOST=127.0.0.1
PORT=8080
GREETING="hello there"
DEBUG=false
```

Values are written bare where that is unambiguous and double-quoted
where it is not, and a `$` inside a value is escaped so that loading
the file back does not expand it. The output round-trips: feeding it to
`parse()` returns the same names and the same values.

Numbers, bigints, booleans and bytes are converted to their text.
Anything else is a `TypeError`, because guessing what a list should
look like in an environment file is how configuration goes wrong
quietly.

## When It Goes Wrong

Every error the module raises is an `EnvError`, and each subclass
carries the detail a message alone cannot:

```zuri
import env

catch {
  env.parse('12FACTOR=yes')
} as error {
  echo '${error.type}: ${error.message}'
  echo 'line ${error.line}, column ${error.column}'
}

catch {
  env.loader().path('.env.nowhere').required().load()
} as error {
  echo '${error.type}: ${error.message}'
}

catch {
  env.require('NOTHING_SET_THIS')
} as error {
  echo '${error.type}: ${error.message}'
}
```

```console
ParseError: expected a variable name
line 1, column 1
MissingFile: no environment file at .env.nowhere
MissingVariable: NOTHING_SET_THIS is not set
```

| Class | Raised when | Carries |
| --- | --- | --- |
| `ParseError` | a source is not a valid environment file | `line`, `column` |
| `MissingFile` | a required file is not there | `path` |
| `MissingVariable` | a demanded variable is not configured | `name` |

Catching `EnvError` catches all three, and a `ValueError` from a typed
reader is the ordinary one, so it goes wherever the rest of your
validation failures go.

## Keeping Secrets Out of Git

A `.env` file holds the values that differ between one deployment and
the next, which is to say it holds the secrets. Three lines of
housekeeping and the subject never comes up again:

```console
$ echo '.env' >> .gitignore
$ echo '.env.local' >> .gitignore
$ cp .env .env.example    # then replace every real value
```

Commit `.env.example` with every name in it and no real value. It is
the only documentation of what the program needs that cannot go stale,
because the day someone adds a setting without adding it there is the
day a new checkout stops working.

Resist the pull towards a `.env.production` checked in beside a
`.env.staging`. Configuration varies per deploy, not per named
environment, and the moment there are two files somebody edits the
wrong one. Use one file per machine and let `required()` and `:?` say
out loud what that machine still owes you.

## Module Reference

The whole surface:

| | |
| --- | --- |
| `load(path)` | read a file into the process environment |
| `loader()` | a `Loader`, for a load that needs more |
| `parse(source)` | text to names and values, nothing else |
| `stringify(values)` | names and values back to text |
| `is_name(name)` | whether a name is a legal variable name |

Reading values back:

| | |
| --- | --- |
| `get`, `require`, `has` | text, or the absence of it |
| `int`, `float`, `bool`, `list` | text converted, or a `ValueError` |

The classes:

| | |
| --- | --- |
| `Loader` | `path`, `paths`, `source`, `override`, `expand`, `required`, `read`, `load` |
| `Result` | `values`, `applied`, `skipped`, `files`, `missing` |
| `EnvError` | `ParseError`, `MissingFile`, `MissingVariable` |

And the piece underneath, for a program that needs it directly:

| | |
| --- | --- |
| `env.expand` | `expand(value, resolve)`, the `$` reference syntax on its own |
