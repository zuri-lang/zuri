# bundle

Packages the project with a runtime, into something that runs where
zuri is not installed.

```sh
zuri bundle [--format <format>] [--target <target...>] [options]
```

A bundle holds the runtime renamed after the program, the standard
library it was built with, and the project with the packages it
installed for production. Running it runs the project's `index.zu`,
whatever directory it is started from, and arguments reach the program
as `os.args`.

## Formats

| Format | What it makes |
| --- | --- |
| `archive` | a `.tar.gz`, or a `.zip` for Windows, of the directory. The default. |
| `dir` | the directory itself |
| `exe` | one file that unpacks itself into the user's cache the first time it runs |
| `app` | a macOS application |

Names follow `<name>-<version>-<target>`, in `dist` in the project
unless `--output` says otherwise.

## Other platforms

`--target` builds for other platforms with the release of this same
zuri version for each, downloaded once, checked against its published
checksum, and kept under `ZURI_HOME/runtimes`. `--target all` builds
for every platform zuri is released for. `--runtime` names an unpacked
runtime to use in place of a release.

A single-file bundle for macOS carries its payload inside the
executable's image and is signed ad hoc as it is built, on any
machine, so it runs on Apple silicon as it comes. To distribute it,
sign it again with a Developer ID; `codesign --force` replaces the ad
hoc signature.

## What it checks

The lockfile has to match `project.toml`, so a bundle never ships
packages nobody resolved, and every package it names apart from
development dependencies has to be installed at its locked version;
`zuri restore` puts either right. Only those packages are copied in, so
development dependencies and anything else in `.zuri/libs` stay
behind. A project with no dependencies needs no lockfile at all.

## Flags

| Flag | What it does |
| --- | --- |
| `-f, --format <format>` | `dir`, `archive`, `exe` or `app`. Default `archive`. |
| `-t, --target <target...>` | `host`, `all`, or platforms such as `aarch64-apple-darwin`. Default `host`. |
| `-o, --output <dir>` | Where bundles are written. Default `dist`. |
| `--name <name>` | What the program is called. Default the project name. |
| `--runtime <dir>` | An unpacked runtime to build with. |
| `--force` | Replace a bundle that is already there. |
| `--json` | Print what was built as JSON. |
