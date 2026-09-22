# init

Starts a Zuri project: the directory layout, a manifest describing it,
an application that runs, and a test that passes. What it writes is
already in the project style and already under test, so the first
thing anyone does with a new project is add to it rather than fix it.

## Running it

```sh
zuri init [path]
```

`path` is where the project goes, created if it is not there yet. Left
off, the directory you are standing in becomes the project:

```sh
zuri init myproject   # myproject/, created
zuri init             # here
```

Then:

```sh
cd myproject
zuri run
zuri test
```

## What it writes

```
myproject/
├── project.toml
├── index.zu
├── app/
│   └── index.zu
├── tests/
│   └── app.test.zu
├── README.md
├── .gitignore
└── .gitattributes
```

`index.zu` re-exports everything under `app`, so importing the project
gets the whole of it. It starts the application only when it is what
was run:

```zuri,ignore
import @.app { * }

if __root__ == __file__ {
  main()
}
```

Which means `zuri run` starts the project, and another project that
imports it gets its exports without setting it going.

`app/index.zu` is the application. Whatever it exports, the project
exports. `tests/app.test.zu` tests it, and `zuri test` runs it.

A file that is already there is never written over. Initializing
inside a directory that has a README, a licence and a git history adds
what is missing and disturbs nothing else.

## The manifest

`project.toml` is what the project is, for anything that reads it:

```toml
[project]
name = "myproject"
version = "0.1.0"
description = "Does one thing."
authors = ["Ada Lovelace <ada@example.com>"]
license = "MIT"
keywords = ["cli"]
homepage = "https://example.com"

[dependencies]
```

`name`, `version`, `description`, `authors` and `license` are always
written, so the file states the whole shape a project has. `keywords`,
`homepage` and `repository` appear once they have something to say.

The name defaults to the directory's own, lower-cased and joined up: a
directory called `My Project (v2)` suggests `my-project-v2`. A name is
lowercase letters, digits, hyphens and underscores, beginning and
ending on a letter or digit, and at most 64 characters. The version is
semantic versioning, pre-release and build metadata included. The
author is read from `git config`.

## Answering the questions

Run in a terminal, `init` asks for each of those in turn and offers
what it worked out. Press enter to take it. The manifest is shown
before anything is written, and `n` at that point writes nothing and
exits `1`.

Run anywhere else — a pipe, a script, a CI job — there is nobody to
ask, so the defaults stand and the project is written straight away.
`--yes` says so outright.

A flag settles its question, and the question is not asked:

```sh
zuri init myproject --name web-server --license Apache-2.0
```

## Flags

| Flag | What it does |
| --- | --- |
| `-n, --name <name>` | The project name. Default: the directory's own. |
| `--version <version>` | The version it starts at. Default `0.1.0`. |
| `-d, --description <text>` | One line on what the project is. |
| `-a, --author <author...>` | Who wrote it. Default: `git config`. |
| `-l, --license <license>` | An SPDX identifier. Default `MIT`. |
| `-k, --keywords <keyword...>` | Words it should be found by. |
| `--homepage <url>` | Where it is documented. |
| `--repository <url>` | Where its source lives. |
| `-y, --yes` | Take every default without asking. |
| `--vcs <git\|none>` | Whether to create a git repository. Default: `git`, unless git is missing or the directory is already in a repository. |
| `--force` | Write `project.toml` again over a project that is already here. |
| `--dry-run` | Report what would be written and write nothing. |

`--author` and `--keywords` take as many words as follow them, so put
the path ahead of them, or close them with `--`:

```sh
zuri init myproject --keywords cli tools
zuri init --keywords cli tools -- myproject
```

At the prompt, both are separated by commas instead.

## Initializing twice

`init` refuses where a project already is, rather than writing over
one:

```console
$ zuri init
init: a project already exists in '/home/ada/myproject'; use --force to
rewrite its project.toml and leave everything else alone
```

`--force` does exactly that much: the manifest is written again from
the answers, and every other file is left as it stands.

## How it is built

- `plan.zu` — the argument parser, and every rule about what a project
  may be called, where it lands, and what ends up in its manifest.
- `template.zu` — the text of each file a project starts with.
- `scaffold.zu` — the layout, and writing it out without disturbing
  what was already there.
- `index.zu` — the command itself, the questions, and the report.

## Tests

```sh
zuri test cmds/init/tests
```

The command end to end, including that what it writes runs and passes
its own tests, is covered in `tests/zuri.rs` under `init_command`.
