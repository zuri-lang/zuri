# Installation

Zuri installs with one command, which downloads the release for your
machine, checks it, and puts `zuri` on your `PATH`. Releases cover:

- Linux on x86-64 and ARM with the GNU C library, version 2.28 or later
  (`x86_64-unknown-linux-gnu`, `aarch64-unknown-linux-gnu`). That is
  Debian 10, Ubuntu 20.04, RHEL 8, Amazon Linux 2023 and anything newer.
- Linux on x86-64 and ARM with musl, as on Alpine
  (`x86_64-unknown-linux-musl`, `aarch64-unknown-linux-musl`)
- macOS on Intel and Apple silicon (`x86_64-apple-darwin`,
  `aarch64-apple-darwin`)
- Windows on x86-64 (`x86_64-pc-windows-msvc`)

Zuri also builds from source with Cargo, which the end of this page
covers.

## Linux and macOS

```console
$ curl -fsSL https://zuri-lang.github.io/zuri/install.sh | sh
Downloading zuri 0.1.0 for x86_64-unknown-linux-gnu
Installed zuri 0.1.0 in /home/ada/.zuri/runtime
Added /home/ada/.zuri/bin to PATH in /home/ada/.bashrc

Open a new terminal, or run this one to use zuri straight away:

  export PATH="$HOME/.zuri/bin:$PATH"

Get started with:

  zuri --help
```

The installer downloads the archive for your machine and checks it
against the checksum published beside it. It then runs the new `zuri`
once to confirm the version it reports, and only after that moves it
into `~/.zuri/runtime`, so a failed or tampered download never replaces
a working installation. `zuri` is linked into `~/.zuri/bin`, the same
directory that globally installed tools put their launchers in, and
that one directory goes on your `PATH` through your shell's startup
file: `.zshrc` for zsh, `.bashrc` for bash (`.bash_profile` on macOS),
`config.fish` for fish, and `.profile` for anything else.

On a Linux that uses musl, the installer picks the musl build, which runs
on a bare Alpine with nothing added. It loads shared libraries the way
Alpine's own programs do, which is what lets [`ffi`](ch26-00-ffi.md) open
them, and why it needs the musl loader on the machine. It does not run on
a Linux that uses the GNU C library.

Options go after `sh -s --`:

```console
$ curl -fsSL https://zuri-lang.github.io/zuri/install.sh | sh -s -- --version 0.2.0
```

| Option | What it does |
| --- | --- |
| `--version <version>` | installs that release rather than the newest |
| `--prerelease` | considers pre-releases when picking the newest |
| `--no-modify-path` | leaves your shell's startup files alone |

## Windows

In PowerShell:

```console
> powershell -c "irm https://zuri-lang.github.io/zuri/install.ps1 | iex"
```

The Windows installer makes the same checks and installs into
`%USERPROFILE%\.zuri\runtime`. That directory goes on your user `PATH`,
and so does `%USERPROFILE%\.zuri\bin`, where globally installed tools
put their launchers. A terminal opened afterwards has both; the one the
installer ran in has them already.

The same options are parameters, given by running the installer as a
script block:

```console
> & ([scriptblock]::Create((irm https://zuri-lang.github.io/zuri/install.ps1))) -Version 0.2.0
```

`-Version`, `-Prerelease` and `-NoModifyPath` match the options above.

## Checking the Install

```console
$ zuri
Zuri 0.1.0 (running on ZuriVM 0.1.0), REPL/Interactive mode = ON
Build No. => 2026-09-10 23:19:10 UTC
Type ".exit" to quit, ".help" for help or ".credits" for more information
%>
```

That `%>` is the Zuri prompt. Type `.exit` to leave.

`zuri --version` reports the same build without opening a session, which
is the one to reach for from a script or a CI job, and `zuri --help`
adds everything the runtime can run from here:

```console
$ zuri --version
Zuri 0.1.0 (running on ZuriVM 0.1.0)
Build No. => 2026-09-10 23:19:10 UTC
```

## Where It Goes

Both installers keep everything under `ZURI_HOME`, which is `.zuri` in
your home directory unless the variable says otherwise:

```text
~/.zuri/
  runtime/        zuri, with libs and cmds beside it
  bin/            the zuri link, and globally installed tools
```

`runtime` holds the executable with copies of the `libs` and `cmds`
directories beside it. That pairing matters: the standard library is
written mostly in Zuri itself, and the runtime finds it by looking for a
`libs` directory beside the executable. `cmds` is the same arrangement
for the commands the runtime ships.

Running an installer again installs over the top. `zuri upgrade` does
the same from inside an installation, and
[Bundles and Upgrades](ch27-05-bundles.md#upgrading-zuri) covers it.
Removing Zuri is deleting `~/.zuri/runtime` and the `zuri` link in
`~/.zuri/bin`, along with the line the installer added to your shell's
startup file.

`ZURI_RELEASES_URL` points both installers, and `zuri upgrade`, at
another listing of releases in the shape GitHub publishes them, such as
a mirror. `GITHUB_TOKEN`, when set, is sent to GitHub's API, where it
raises the rate limit.

## Downloading by Hand

Every release on the
[releases page](https://github.com/zuri-lang/zuri/releases) carries an
archive per platform, named `zuri-<version>-<target>.tar.gz`, or `.zip`
for Windows, with a `.sha256` beside it. Check the archive against it,
unpack it, and put the `zuri` it holds on your `PATH`, keeping the
`libs` and `cmds` directories beside it.

## Building From Source

Building needs Rust's package manager, Cargo. If you do not have Rust
installed, get it from [rustup.rs](https://rustup.rs); it takes a minute
and installs `cargo` for you.

```console
$ git clone https://github.com/zuri-lang/zuri
$ cd zuri
$ cargo patch-crates
$ cargo build --release
```

`cargo patch-crates` prepares the few dependencies Zuri builds with
changes of its own. It runs once per clone, and again only when one of
those changes is updated; the build says when that is.

The first build compiles a large set of native dependencies, so it takes
a few minutes. Builds after that are incremental and quick.

When it finishes you have an executable at `target/release/zuri`, and,
sitting right next to it, copies of the `libs/` and `cmds/` directories,
in the same arrangement a release has.

### Putting a Build on Your PATH

The simplest arrangement is to keep the binary and its two directories
together and symlink to the binary:

```console
$ sudo ln -s "$PWD/target/release/zuri" /usr/local/bin/zuri
```

A symlink is resolved before the runtime looks for `libs`, so this works.
Copying just the binary does not.

If you want them somewhere else entirely, set `ZURI_ROOT` to the
directory that *contains* them:

```console
$ export ZURI_ROOT=/opt/zuri
$ ls /opt/zuri
cmds  libs
```

`ZURI_ROOT` wins over the executable-adjacent lookup, which makes it handy
when you are hacking on the standard library itself and want a build to
pick up your edits immediately:

```console
$ ZURI_ROOT=$PWD ./target/debug/zuri run myscript.zu
```

### A Debug Build, and Why You Might Want One

Plain `cargo build` produces `target/debug/zuri`. It keeps the runtime
assertions that the optimised build strips out, which makes it the build to
reach for when a program is doing something you did not expect. It runs
more slowly in exchange. Either build runs everything in this book.
