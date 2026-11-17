# Configuration and the Environment

Any program that works with other services needs to be told where they are:
a database address, an API key, a port to listen on, or a flag that turns on
verbose logging for an afternoon. None of that belongs in the source code.
It changes between your laptop and the server, and some of it should never
be committed to a repository at all.

The industry's standard answer is the process environment. Orchestrators, CI
runners and shells all set environment variables, and every language can
read them. What none of them handle is the first machine in the chain, which
is yours. There's no orchestrator on your laptop, and nobody wants to type
eight variable assignments in front of every command.

The `env` module fills that gap. It reads a `.env` file into the process
environment when the program starts, and it reads values back out already
converted to the number, boolean or list your program needs. The file is a
convenience for development, while the environment is the real interface. A
program written with `env` doesn't know or care where a value came from,
which is what lets it run unchanged in both places.

- [Loading the File](#loading-the-file)
- [Writing a `.env` File](#writing-a-env-file)
  - [Quoting](#quoting)
  - [Comments](#comments)
  - [Values That Span Lines](#values-that-span-lines)
- [Existing Variables Take Priority](#existing-variables-take-priority)
  - [Overriding](#overriding)
  - [Empty Means Unset](#empty-means-unset)
- [References Between Values](#references-between-values)
  - [Fallbacks and Required Values](#fallbacks-and-required-values)
  - [Literal Dollar Signs](#literal-dollar-signs)
- [Reading Values](#reading-values)
- [Configuring a Load](#configuring-a-load)
  - [Layering Files](#layering-files)
  - [Loading From Text](#loading-from-text)
  - [Requiring the File](#requiring-the-file)
  - [Reading Without Loading](#reading-without-loading)
- [Writing a File](#writing-a-file)
- [Errors](#errors)
- [Keeping Secrets Out of Git](#keeping-secrets-out-of-git)
- [Module Reference](#module-reference)

## Loading the File

It takes one call, which should go as early in the program as possible:

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

`env.load()` without an argument reads `.env` in the current working
directory, which is what almost every program wants. The examples in this
chapter write their file first and pass its name, so that you can run each
one as it is.

The file is optional. If it doesn't exist, `load()` leaves the environment
exactly as it was, and the program continues with whatever the shell already
provided. That's intentional, so the same program runs on a laptop with a
`.env` file and on a server without one.

## Writing a `.env` File

Each line has a name, an `=` and a value:

```dosini
HOST=127.0.0.1
PORT=8080
DATABASE_URL=postgres://app@localhost/app
```

A name is a letter or underscore followed by letters, digits and
underscores, which is the same rule a shell uses. Anything else stops the
load with a `ParseError` instead of being silently ignored, because a name
that no shell could export is always a typo.

A leading `export` is allowed and ignored, so you can also load the same
file into your shell with `source` when you want the variables there too.

### Quoting

An unquoted value is everything up to the end of the line, with whitespace
removed from both ends. If that's not what you want, put the value in
quotes:

```zuri
import env

var values = env.parse(
  'BARE=  plain text  \n' +
      'COMMENTED=value # not part of it\n' +
      'PASSWORD=hunter#2\n' +
      "RAW='no \\n escape, no expansion'\n" +
      'COOKED="caf\\u00e9"\n',
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

Each quoting style differs from the others in exactly one way:

| Written | Means |
| --- | --- |
| `KEY=value` | Trimmed, and ends at a comment or the end of the line |
| `KEY='value'` | Every character exactly as written, with nothing resolved |
| `` KEY=`value` `` | The same, for values that contain both kinds of quotes |
| `KEY="value"` | Backslash escapes are resolved, and `$` references are expanded |

Inside double quotes, `\n`, `\r`, `\t`, `\f`, `\v`, `\b`, `\a`, `\e` and
`\0` mean the same as they do in Zuri, and `\xHH`, `\uHHHH` and `\u{H...}`
specify a code point. A backslash before any other character keeps both
characters, so `"C:\Users\me"` gives you the path you meant, and you don't
need to think about escaping.

Use single quotes for any value that's a secret. A generated password that
contains `$` or `\` is kept exactly as it is, and nothing in it can
accidentally refer to another variable.

### Comments

A `#` starts a comment, either on a line of its own or after a value. Inside
an unquoted value, it only starts a comment when it's at the start of the
value or follows a space, which is why `PASSWORD=hunter#2` above kept its
`#`. If you'd rather not rely on that rule, quote the value, and the
question doesn't come up.

### Values That Span Lines

A quoted value continues until its closing quote, including any newlines.
This is how you put a private key in the file:

```dosini
SIGNING_KEY="-----BEGIN PRIVATE KEY-----
MIIEvQIBADANBgkqhkiG9w0BAQEFAASCBKcwggSjAgEAAoIBAQC7...
-----END PRIVATE KEY-----"
```

Single quotes work the same way and don't resolve anything, which usually
makes them the better choice for keys.

## Existing Variables Take Priority

If a name is already set in the process environment, it's left as it is, and
the file only fills in the rest:

```zuri
import env
import os

os.set_env('DATABASE_URL', 'postgres://production/app', true)

file(
  '.env.demo',
  'w',
).write('DATABASE_URL=postgres://localhost/app\n' + 'CACHE_TTL=300\n')

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

This is the reason for the default. The file contains what a developer needs
to run the program at all. In production, the real values are set through
the shell, the orchestrator or the CI runner, and the file doesn't need to
know anything about it.

`load()` returns a `Result` that accounts for every name and every file, so
a program never has to guess whether its configuration was loaded. `applied`
contains the variables the load set, `skipped` contains the ones that were
already set, and `values` contains everything the file defined, either way.
`files` lists the paths that were read, and `missing` lists the ones that
didn't exist.

### Overriding

When the file really should take priority, say so:

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

### Empty Means Unset

Throughout the module, a variable set to an empty string is treated as
unset. `FLAG=` in a file, an empty shell variable and a variable that was
never set all mean the same thing, because that's what people mean by all
three. A load fills in an empty variable, `has()` returns false for it,
`require()` raises an error for it, and every typed reader returns its
default value.

If you need to tell an empty value from a missing one, which is rare, use
`os.get_env()`, which doesn't treat them the same.

## References Between Values

A value can refer to another value, or to the surrounding environment, using
the same syntax as a shell:

```dosini
HOST=localhost
PUBLIC_URL=http://${HOST}:${PORT:-8080}
```

```zuri
import env

file('.env.demo', 'w').write(
  'HOST=localhost\n' + 'PUBLIC_URL=http://$' + '{HOST}:$' + '{PORT:-8080}\n',
)

var loaded = env.load('.env.demo')

echo loaded.values.PUBLIC_URL
```

```console
http://localhost:8080
```

The split string in that example is because of Zuri, not `.env` files. A
`${` written inside a Zuri string literal is an interpolation, so when you
build `.env` text in source code, you have to keep the `$` and the `{`
apart. A file you write in an editor doesn't have this problem, as the
`dosini` block above shows.

A reference resolves to the value the variable will have *once the load has
finished*. That rule is what makes `PORT=${PORT:-8080}` mean what it looks
like: the value from the environment if there is one, or 8080 if there
isn't. This is true whichever line of the file the reference is on, and it
changes with `override()` in the same way that priority does.

A definition can also refer to the value it's replacing, which is how you
extend a path instead of overwriting it:

```dosini
PATH=${PATH}:/opt/app/bin
```

### Fallbacks and Required Values

The three modifiers are the ones POSIX defines. Adding a colon in front of
each one changes the test from "is it set" to "is it set to something":

| Written | Means |
| --- | --- |
| `${NAME:-fallback}` | The value, or `fallback` if it's unset or empty |
| `${NAME-fallback}` | The value, or `fallback` if it's unset |
| `${NAME:+instead}` | `instead` if the value is set and not empty, and nothing otherwise |
| `${NAME+instead}` | `instead` if the value is set, and nothing otherwise |
| `${NAME:?reason}` | The value, or a `MissingVariable` error with `reason` as its message |
| `${NAME?reason}` | The same, but an empty value counts as set |

The text after a modifier is also expanded, so a fallback can have its own
fallback:

```dosini
CACHE_DIR=${XDG_CACHE_HOME:-${HOME}/.cache}/app
```

`:?` is particularly useful. It lets the file itself document what a
deployment requires, and if something is missing, the program fails when it
starts, with the reason included:

```dosini
SESSION_SECRET=${SESSION_SECRET:?the deploy must supply a session secret}
```

### Literal Dollar Signs

`\$` is a dollar sign and nothing else. A `$` that isn't followed by a name,
as in `PRICE=$5.00`, already stands for itself. Values in single quotes or
backticks are never expanded, so you don't need to worry about a secret that
contains `${`:

```dosini
API_KEY='sk_live_${not_a_reference}'
```

## Reading Values

An environment variable is always text, but programs rarely want text. The
typed readers convert the value, and reject anything that isn't what it
claims to be:

```zuri
import env

file('.env.demo', 'w').write(
  'PORT=8080\n' +
      'DEBUG=yes\n' +
      'REQUEST_TIMEOUT=2.5\n' +
      'CORS_ORIGINS= https://a.example , https://b.example \n' +
      'SENTRY_DSN=\n',
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

| Function | Returns |
| --- | --- |
| `get(name, default)` | The text, or the default |
| `require(name, reason)` | The text, or raises a `MissingVariable` error |
| `has(name)` | Whether the variable is set at all |
| `int(name, default)` | A decimal integer, which can have a sign |
| `float(name, default)` | A number, including one with an exponent |
| `bool(name, default)` | True for `1`, `true`, `yes`, `y` and `on`, and false for their opposites |
| `list(name, separator, default)` | The value split into a list, with each item trimmed and empty items removed |

A value that's set but can't be converted raises a `ValueError` that names
the variable, instead of silently falling back to the default. `PORT=eighty`
is a configuration mistake, and the right time to find out about it is when
the program starts, not on the first request that needs a port.

These functions read the process environment, not the file. They give the
same answer whether a value came from `.env` or from the shell, which is
what lets one program run in both places without any special cases.

Most programs should use `require()` for at least one setting. For a setting
that the code can't make up a default for, the program should stop right at
the start, with the setting's name in the error message:

```zuri,ignore
var secret = env.require('SESSION_SECRET', 'set SESSION_SECRET in .env')
```

## Configuring a Load

When the one-line form isn't enough, `env.loader()` returns a `Loader`.
Every setting returns the loader, so you can write a whole configuration as
a single expression. Running a loader doesn't change it, so you can keep one
and use it again.

### Layering Files

Sources are read in the order they're added, and the last source to define a
name is the one whose value is used:

```zuri
import env

file('.env.demo', 'w').write('HOST=localhost\nPORT=8080\n')
file('.env.local.demo', 'w').write('HOST=127.0.0.1\n')

var result = env
    .loader()
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

This combination is the one you'll use most often. `.env` contains what the
whole team shares, and is worth committing as `.env.example`, while
`.env.local` contains what's different on one particular machine, and isn't
committed. A path can start with `~`, which is expanded to the home
directory.

### Loading From Text

`source()` adds text instead of a path, so configuration that came from the
network or a secret store goes through exactly the same parsing, expansion
and priority rules as a file on disk:

```zuri,ignore
env.loader().path('.env').source(vault.fetch('app/production')).load()
```

### Requiring the File

By default, a missing file isn't an error, and its path is recorded in
`Result.missing`. When the file really is part of the deployment, mark it as
required, so the load fails with a clear error if it's missing:

```zuri,ignore
env.loader().path('/etc/app/env').required().load()
```

### Reading Without Loading

`read()` does everything `load()` does except the final step. Nothing is
written to the process environment, but `$` references are still resolved as
they would have been, so what you get back is exactly what `load()` would
have set. It's how a tool can inspect a file, and how a test can check one
without changing the environment of the process it's running in.

`expand(false)` turns off expansion completely, for a file of secrets where
every value should be used exactly as written.

## Writing a File

`stringify()` is the opposite of `parse()`, for a program that generates a
`.env` file instead of reading one:

```zuri
import env

print(
  env.stringify({
    HOST: '127.0.0.1',
    PORT: 8080,
    GREETING: 'hello there',
    DEBUG: false,
  }),
)
```

```console
HOST=127.0.0.1
PORT=8080
GREETING="hello there"
DEBUG=false
```

Values are written without quotes where that's unambiguous, and in double
quotes where it isn't, and a `$` inside a value is escaped so that loading
the file again doesn't expand it. The output round-trips: passing it to
`parse()` returns the same names and values.

Numbers, bigints, booleans and bytes are converted to text. Anything else
raises a `TypeError`, because guessing how a list should look in an
environment file is how configuration silently goes wrong.

## Errors

Every error the module raises is an `EnvError`, and each subclass includes
details that the message alone can't:

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

| Class | Raised when | Includes |
| --- | --- | --- |
| `ParseError` | A source isn't a valid environment file | `line`, `column` |
| `MissingFile` | A required file doesn't exist | `path` |
| `MissingVariable` | A required variable isn't set | `name` |

Catching `EnvError` catches all three. A `ValueError` from a typed reader is
an ordinary `ValueError`, so you can handle it in the same place as the rest
of your validation errors.

## Keeping Secrets Out of Git

A `.env` file holds the values that differ from one deployment to the next,
which means it holds your secrets. Three lines in your `.gitignore` take
care of that once and for all:

```console
$ echo '.env' >> .gitignore
$ echo '.env.local' >> .gitignore
$ cp .env .env.example    # then replace every real value
```

Commit a `.env.example` that lists every variable but contains no real
values. It's the only documentation of what the program needs that can't get
out of date, because as soon as someone adds a setting without adding it
there, a fresh checkout stops working.

Avoid committing a `.env.production` alongside a `.env.staging`.
Configuration varies by deployment, not by environment name, and as soon as
there are two files, someone will edit the wrong one. Use one file per
machine, and let `required()` and `:?` make it clear what that machine is
still missing.

## Module Reference

Here's everything the module provides:

| Function | Description |
| --- | --- |
| `load(path)` | Reads a file into the process environment |
| `loader()` | Returns a `Loader`, for loads that need more configuration |
| `parse(source)` | Converts text into names and values, without changing the environment |
| `stringify(values)` | Converts names and values back into text |
| `is_name(name)` | Checks whether a name is a valid variable name |

Reading values:

| Functions | Description |
| --- | --- |
| `get`, `require`, `has` | Read the text, or check whether it's missing |
| `int`, `float`, `bool`, `list` | Read the text and convert it, or raise a `ValueError` |

The classes:

| Class | Members |
| --- | --- |
| `Loader` | `path`, `paths`, `source`, `override`, `expand`, `required`, `read`, `load` |
| `Result` | `values`, `applied`, `skipped`, `files`, `missing` |
| `EnvError` | `ParseError`, `MissingFile`, `MissingVariable` |

And the lower-level module, for programs that need it directly:

| Module | Contents |
| --- | --- |
| `env.expand` | `expand(value, resolve)`, the `$` reference syntax on its own |
