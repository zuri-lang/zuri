# test

Runs a project's test files, each in a process of its own, and reports
them together. It is `test.conduct()` behind a command line, so a
project gets a test runner without writing one.

## Running it

```sh
zuri test [target]
```

With no target it runs the `tests` directory here:

```sh
zuri test
```

With one it runs a single file, whose `.zu` ending is optional, or a
whole directory:

```sh
zuri test mytest            # tests/mytest.zu
zuri test mytest.zu         # the same file
zuri test tests/api.zu      # the same file, by its path
zuri test deep              # tests/deep, whole
zuri test cmds/format/tests # a directory anywhere in the project
```

A name is looked for under `tests` first, so a test keeps its own name
even when something else in the project shares it. If nothing there
matches the name as written, it is looked for once more by filename
alone anywhere under `tests`, so a file in a subdirectory answers to its
own name. Two files of that name is reported rather than guessed at.

The run exits `1` when anything failed, so a CI job needs nothing added
to it.

## Flags

| Flag | What it does |
| --- | --- |
| `-j, --jobs <count>` | How many files to run at once. `auto` is one per CPU. Default `1`. |
| `-t, --timeout <duration>` | How long to give a single file before killing it. `500ms`, `30s`, `2m`, `1h`, or a bare number of milliseconds. Default: no limit. |
| `-b, --bail [count]` | Stop after this many failing files. On its own, stop at the first. Default: never stop early. |
| `-m, --match <pattern...>` | Filename patterns to run, instead of `*.zu`. |
| `-i, --ignore <pattern...>` | Filename patterns to skip, instead of `_*`, `.*` and `index.zu`. |
| `--no-recursive` | Only the files directly in the directory. |
| `-e, --env <assignment...>` | Extra environment for every test process, as `KEY=VALUE`. |
| `-l, --list` | Print the files that would run, one to a line, and stop. |

```sh
zuri test --jobs auto --timeout 30s
zuri test --bail
zuri test --match '*_test.zu' '*_spec.zu'
zuri test --list | xargs wc -l
```

`--bail` and the two pattern flags take as many words as follow them,
so put the target ahead of them, or close them with `--`:

```sh
zuri test mytest --bail
zuri test --match '*_test.zu' -- mytest
```

A count for `--bail` has to be attached, `--bail=3`, for the same
reason: a count written as a separate word is indistinguishable from
the target.

`--match` and `--ignore` choose among the files in a directory. Naming
one file has already chosen, so the two together are refused rather
than quietly resolved.

## Writing the tests

Nothing here is required of a test file. It is an ordinary Zuri script
that declares its tests with the `test` module, equally runnable on its
own with `zuri run`:

```zuri,ignore
import test { * }

describe('parse', @{
  it('reads a header', @{
    expect(parse('a: 1')).to_equal({ a: '1' })
  })
})
```

## How it is built

- `plan.zu` — the argument parser, the rules that turn a target into a
  directory and a file, and the ones that turn flags into `conduct()`
  options.
- `index.zu` — the command itself, the entry point `zuri test` runs.

The running is `test.conduct()`, unchanged: one process per file,
reported in discovery order, with `index.zu` and the conductor's own
script left out of what it finds.

## Tests

```sh
zuri test cmds/test/tests
```
