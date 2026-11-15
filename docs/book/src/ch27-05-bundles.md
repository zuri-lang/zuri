# Bundles and Upgrades

## Bundling a Program

`zuri bundle` packages a project together with a runtime, so that it can run
on a machine that doesn't have Zuri installed:

```console
$ zuri bundle --format exe
$ ./dist/weather-0.1.0-x86_64-unknown-linux-gnu
Hello, world!
```

A bundle contains the runtime, renamed after the program, the standard
library it was built with, and the project along with the packages it needs
in production. Running the bundle starts the project's `index.zu`, whatever
directory it's started from, and its arguments are passed to the program in
`os.args`. It doesn't use anything installed on the machine it runs on, such
as a `ZURI_ROOT` or anything in a `ZURI_HOME`, so it behaves the same
everywhere.

| Format | What it creates |
| --- | --- |
| `archive` | A `.tar.gz` of the bundle directory, or a `.zip` for Windows. This is the default. |
| `dir` | The bundle directory itself |
| `exe` | A single file, which unpacks itself into the user's cache the first time it runs and starts from there afterwards |
| `app` | A macOS application |

Bundles are written to `dist` in the project, and named
`<name>-<version>-<platform>`, unless you use `--output` and `--name` to
choose otherwise.

## What Goes Into a Bundle

The lockfile has to match `project.toml`, and every package it lists, except
development dependencies, has to be installed at its locked version, so a
bundle never includes packages that weren't resolved. `zuri restore` fixes
either problem. A project without any dependencies doesn't need a lockfile
at all. The project's files are selected in the same way as for publishing,
so `include` and `exclude` apply here too.

## Building for Other Platforms

```sh
zuri bundle --target aarch64-apple-darwin --target x86_64-pc-windows-msvc
zuri bundle --target all
```

A bundle for another platform is built using the release of the same Zuri
version for that platform. It's downloaded once, checked against its
published checksum and kept in `$ZURI_HOME/runtimes`. You can use
`--runtime` to name an unpacked runtime to use instead. These are the
supported platforms:

- `x86_64-unknown-linux-gnu`
- `aarch64-unknown-linux-gnu`
- `x86_64-unknown-linux-musl`
- `aarch64-unknown-linux-musl`
- `x86_64-apple-darwin`
- `aarch64-apple-darwin`
- `x86_64-pc-windows-msvc`

A bundle runs anywhere its runtime does: a GNU bundle on glibc 2.28 or
later, and a musl bundle on any musl system, including a bare Alpine
installation.

You can build a single-file bundle for macOS on any machine. Its payload is
stored inside the executable's image rather than after it, and the result is
signed ad hoc, which is all an Apple silicon Mac needs to run it.

## Signing for Distribution

A program downloaded onto a Mac only runs if it's signed with a Developer ID
and notarized, and notarization requires the hardened runtime. With the
hardened runtime, Zuri's JIT needs the `com.apple.security.cs.allow-jit`
entitlement to create executable memory, so sign it with an entitlements
file that grants it:

```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>com.apple.security.cs.allow-jit</key>
  <true/>
</dict>
</plist>
```

```console
$ codesign --force --options runtime --entitlements entitlements.plist \
    --sign "Developer ID Application: Example Ltd" dist/weather-0.1.0-aarch64-apple-darwin
```

`--force` replaces the ad hoc signature. Sign an `app` bundle the same way,
passing the `.app` directory instead of the executable.

## macOS Applications

```toml
[bundle]
name = "Weather"
identifier = "com.example.weather"
icon = "assets/weather.icns"
```

An `app` bundle needs an `identifier`, written in reverse domain name form.
`name` is the name Finder shows, and `icon` is an `.icns` file in the
project.

## Upgrading Zuri

```console
$ zuri upgrade --check
$ zuri upgrade
```

`--check` tells you whether a newer release is available, and exits with
status `1` if there is one. `zuri upgrade` downloads the release for the
current platform, checks it against its published checksum, unpacks it next
to the installation and runs it to confirm the version it reports. Only then
are the executable, the standard library and the included commands all
replaced at once, and if any part of that fails, everything is put back.

A release without a published checksum is rejected, and so is an
installation you don't have permission to write to, before anything is
downloaded. `--version` chooses a particular release, `--prerelease`
includes pre-releases, and installing an older release requires
`--allow-downgrade`. Releases come from the project's GitHub releases.
`ZURI_RELEASES_URL` points to another list of releases in the same format,
such as a mirror, and `GITHUB_TOKEN` is sent along if it's set.
