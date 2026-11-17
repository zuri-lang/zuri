# test

Runs a project's test files, each in its own process, and reports the
results together. It's a command-line front end for `test.conduct()`, so
a project doesn't need a test runner of its own.

## Usage

```sh
zuri test [target]
```

With no target, it runs the tests in the `tests` directory:

```sh
zuri test
```

With a target, it runs a single file, with or without the `.zu`
extension, or every file in a directory:

```sh
zuri test mytest            # tests/mytest.zu
zuri test mytest.zu         # the same file
zuri test tests/api.zu      # the same file, by its path
zuri test deep              # every file in tests/deep
zuri test cmds/fmt/tests    # a directory anywhere in the project
```

A name is looked up in `tests` first, so a test can be run by its name
even if something else in the project has the same name. If nothing in
`tests` matches the name as written, it's looked up again by file name
alone, anywhere under `tests`, so a file in a subdirectory can be run by
its name too. If two files have that name, you get an error instead of a
guess.

The command exits with status 1 if any test failed, so a CI job doesn't
need any extra checks.

## Options

| Option | Description |
| --- | --- |
| `-j, --jobs <count>` | How many files to run at once, or `auto` for one per CPU. Defaults to `1`. |
| `-t, --timeout <duration>` | How long a single file can run before it's stopped, such as `500ms`, `30s`, `2m` or `1h`, or a number of milliseconds. There's no limit by default. |
| `-b, --bail [count]` | Stop after this many files fail, or after the first failure if no count is given. By default, every file runs. |
| `-m, --match <pattern...>` | File name patterns to run, instead of `*.zu`. |
| `-i, --ignore <pattern...>` | File name patterns to skip, instead of `_*`, `.*` and `index.zu`. |
| `--no-recursive` | Only run the files directly in the directory, not in subdirectories. |
| `-e, --env <assignment...>` | Extra environment variables for every test process, as `KEY=VALUE`. |
| `-l, --list` | Print the files that would run, one per line, and stop. |

```sh
zuri test --jobs auto --timeout 30s
zuri test --bail
zuri test --match '*_test.zu' '*_spec.zu'
zuri test --list | xargs wc -l
```

`--bail` and the two pattern options take every word that follows them,
so put the target before them, or end them with `--`:

```sh
zuri test mytest --bail
zuri test --match '*_test.zu' -- mytest
```

For the same reason, a count for `--bail` has to be attached to it, as in
`--bail=3`. A count written as a separate word can't be told apart from a
target.

`--match` and `--ignore` choose which files in a directory to run, so they
can't be combined with a target that names a single file.

## Writing tests

A test file doesn't need anything special. It's an ordinary Zuri script
that declares its tests with the `test` module, and you can also run it by
itself with `zuri run`:

```zuri,ignore
import test { * }

describe('parse', @{
  it('reads a header', @{
    expect(parse('a: 1')).to_equal({ a: '1' })
  })
})
```

## Source files

- `plan.zu`: the argument parser, the rules that turn a target into a
  directory and a file, and the rules that turn options into `conduct()`
  options.
- `index.zu`: the command itself, which `zuri test` runs.

The tests are run by `test.conduct()`, unchanged. Each file runs in its own
process, and the files are reported in the order they were found. The
directory's `index.zu` and the conductor's own script aren't run as tests.

## Tests

```sh
zuri test cmds/test/tests
```
