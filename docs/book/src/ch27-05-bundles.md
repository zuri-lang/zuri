# Bundles and Upgrades

## Bundling a Program

`zuri bundle` packages a project with a runtime into something that
runs on a machine where Zuri is not installed:

```console
$ zuri bundle --format exe
$ ./dist/weather-0.1.0-x86_64-unknown-linux-gnu
Hello, world!
```

A bundle holds the runtime renamed after the program, the standard
library it was built with, and the project with the packages it needs
in production. Running it starts the project's `index.zu`, whatever
directory it is started from, and its arguments reach the program in
`os.args`. It uses nothing installed on the machine it runs on, neither
a `ZURI_ROOT` nor anything in a `ZURI_HOME`, so it behaves the same
everywhere.

| Format | What it makes |
| --- | --- |
| `archive` | a `.tar.gz` of the bundle directory, or a `.zip` for Windows; the default |
| `dir` | the bundle directory itself |
| `exe` | one file, which unpacks itself into the user's cache the first time it runs and starts from there afterwards |
| `app` | a macOS application |

Bundles go in `dist` in the project, named
`<name>-<version>-<platform>`, unless `--output` and `--name` say
otherwise.

## What Goes In

The lockfile has to match `project.toml`, and every package it names,
apart from development dependencies, has to be installed at its locked
version, so a bundle never ships packages nobody resolved. `zuri
restore` puts either right. The project's files are chosen the same way
publishing chooses them, so `include` and `exclude` apply here too.

## Other Platforms

```sh
zuri bundle --target aarch64-apple-darwin --target x86_64-pc-windows-msvc
zuri bundle --target all
```

A bundle for another platform is built with the release of this same
Zuri version for that platform, downloaded once, checked against its
published checksum, and kept in `$ZURI_HOME/runtimes`. `--runtime`
names an unpacked runtime to use instead. The platforms are:

- `x86_64-unknown-linux-gnu`
- `aarch64-unknown-linux-gnu`
- `x86_64-apple-darwin`
- `aarch64-apple-darwin`
- `x86_64-pc-windows-msvc`

A single-file bundle for macOS has to be signed again once built, since
appending to an executable breaks its signature, so it is only built on
a Mac.

## macOS Applications

```toml
[bundle]
name = "Weather"
identifier = "com.example.weather"
icon = "assets/weather.icns"
```

An `app` bundle needs `identifier`, in reverse domain form. `name` is
what Finder shows, and `icon` an `.icns` file in the project.

## Upgrading Zuri

```console
$ zuri upgrade --check
$ zuri upgrade
```

`--check` says whether a newer release exists, and exits `1` when one
does. `zuri upgrade` downloads the release for this platform, checks it
against its published checksum, unpacks it beside the installation, and
runs it to confirm the version it reports. Only then are the
executable, the standard library and the shipped commands swapped in
together, and if any part of that fails, all of it is put back.

A release with no published checksum is refused, and so is an
installation you cannot write to, before anything is downloaded.
`--version` picks a release, `--prerelease` considers pre-releases, and
moving to an older release needs `--allow-downgrade`. Releases come
from the project's GitHub releases; `ZURI_RELEASES_URL` points at
another listing in the same shape, such as a mirror, and `GITHUB_TOKEN`
is sent when it is set.
