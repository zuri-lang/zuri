# init

Creates a new Zuri project: the directory layout, a manifest that
describes the project, an application that runs, and a test that passes.
The files it creates already follow the project's code style and are
already tested, so you can start adding to them right away.

## Usage

```sh
zuri init [path]
```

`path` is where the project is created. If the directory doesn't exist,
it's created. Without a path, the current directory becomes the project:

```sh
zuri init myproject   # creates myproject/
zuri init             # uses the current directory
```

Then:

```sh
cd myproject
zuri run
zuri test
```

## What it creates

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

`index.zu` re-exports everything in `app`, so importing the project gives
you all of it. It only starts the application when it's the file that was
run:

```zuri,ignore
import @.app { * }

if __root__ == __file__ {
  main()
}
```

This means that `zuri run` starts the project, while another project that
imports it gets its exports without starting it.

`app/index.zu` is the application. Whatever it exports, the project
exports. `tests/app.test.zu` tests it, and `zuri test` runs that test.

An existing file is never overwritten. If you run `init` in a directory
that already has a README, a license and a git history, it adds the files
that are missing and leaves everything else as it is.

## The manifest

`project.toml` describes the project to the tools that read it:

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
written, so the file shows every field a project normally has.
`keywords`, `homepage` and `repository` are only written when they have a
value.

The name defaults to the name of the directory, in lowercase, with words
joined by hyphens. For example, a directory called `My Project (v2)` gives
`my-project-v2`. A name can contain lowercase letters, digits, hyphens and
underscores, has to start and end with a letter or digit, and can be at
most 64 characters long. The version uses semantic versioning, including
pre-release and build metadata. The author is read from `git config`.

## Questions and defaults

When it runs in a terminal, `init` asks for each of these values in turn,
and suggests a default. Press Enter to accept it. The manifest is shown
before anything is written. If you answer `n` at that point, nothing is
written and the command exits with status 1.

When it doesn't run in a terminal, such as in a pipe, a script or a CI
job, it can't ask any questions, so it uses the defaults and creates the
project right away. `--yes` does the same in a terminal.

An option answers its question, so that question isn't asked:

```sh
zuri init myproject --name web-server --license Apache-2.0
```

## Options

| Option | Description |
| --- | --- |
| `-n, --name <name>` | The project's name. Defaults to the directory's name. |
| `--version <version>` | The project's starting version, as `major.minor.patch`. Defaults to `0.1.0`. |
| `-d, --description <text>` | A one-line description of the project. |
| `-a, --author <author...>` | The project's authors, as `"Name <email>"`. Defaults to the name and email in `git config`. |
| `-l, --license <license>` | The project's license, as an SPDX identifier. Defaults to `MIT`. |
| `-k, --keywords <keyword...>` | Keywords that help people find the project. |
| `--homepage <url>` | The address of the project's homepage or documentation. |
| `--repository <url>` | The address of the project's source code repository. |
| `-y, --yes` | Accept all the defaults without asking any questions. |
| `--vcs <git\|none>` | Whether to create a git repository. Defaults to `git`, unless git isn't installed or the directory is already in a repository. |
| `--force` | Rewrite `project.toml` even if there's already a project here. |
| `--dry-run` | Show what would be written without writing anything. |

`--author` and `--keywords` take every word that follows them, so put the
path before them, or end them with `--`:

```sh
zuri init myproject --keywords cli tools
zuri init --keywords cli tools -- myproject
```

When you answer the questions instead, separate several authors or
keywords with commas.

## Running it twice

`init` won't overwrite an existing project:

```console
$ zuri init
init: a project already exists in '/home/ada/myproject'. Use --force to
rewrite its project.toml without changing any other files
```

With `--force`, the manifest is written again from your answers, and
every other file is left as it is.

## Source files

- `plan.zu`: the argument parser, and the rules for what a project can be
  called, where it's created, and what goes in its manifest.
- `template.zu`: the text of each file that a new project starts with.
- `scaffold.zu`: the layout, and writing it without changing existing
  files.
- `index.zu`: the command itself, the questions and the report.

## Tests

```sh
zuri test cmds/init/tests
```

The tests in `tests/zuri.rs` under `init_command` cover the whole command,
including checking that a new project runs and passes its own tests.
