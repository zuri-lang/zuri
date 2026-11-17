# bundle

Packages the project together with a runtime, so that it can run on
machines without Zuri.

```sh
zuri bundle [--format <format>] [--target <target...>] [options]
```

A bundle contains the runtime, renamed after the program, the standard
library it was built with, and the project with its production packages.
Running the bundle runs the project's `index.zu`, whatever directory it's
started from, and the program receives its arguments in `os.args`.

## Formats

| Format | What it builds |
| --- | --- |
| `archive` | A `.tar.gz` of the bundle directory, or a `.zip` for Windows. This is the default. |
| `dir` | The bundle directory itself. |
| `exe` | A single file that unpacks itself into the user's cache the first time it runs. |
| `app` | A macOS application. |

Bundles are named `<name>-<version>-<target>`, and written to `dist` in the
project unless you pass `--output`.

## Other platforms

`--target` builds for other platforms, using the release of the same Zuri
version for each one. Each release is downloaded once, checked against its
published checksum, and kept in `ZURI_HOME/runtimes`. `--target all` builds
for every platform that Zuri is released for. To build with an unpacked
runtime instead of a release, pass `--runtime`.

A single-file bundle for macOS keeps its payload inside the executable's
image, and it's signed ad hoc as it's built, on any machine, so it runs on
Apple silicon without any further steps. To distribute it, sign it again
with a Developer ID. `codesign --force` replaces the ad hoc signature.

## What it checks

The lockfile has to match `project.toml`, so that a bundle never contains
packages that weren't resolved. Every package in the lockfile, except for
development dependencies, has to be installed at its locked version. If
either check fails, run `zuri restore` to fix it. Only those packages are
copied into the bundle, so development dependencies and anything else in
`.zuri/libs` are left out. A project without dependencies doesn't need a
lockfile.

## Options

| Option | Description |
| --- | --- |
| `-f, --format <format>` | `dir`, `archive`, `exe` or `app`. Defaults to `archive`. |
| `-t, --target <target...>` | `host`, `all`, or platforms such as `aarch64-apple-darwin`. Defaults to `host`. |
| `-o, --output <dir>` | Where to write the bundles. Defaults to `dist` in the project. |
| `--name <name>` | The program's name. Defaults to the project's name. |
| `--runtime <dir>` | An unpacked runtime to build with, instead of a downloaded release. |
| `--force` | Replace a bundle that already exists. |
| `--json` | Print what was built as JSON. |
