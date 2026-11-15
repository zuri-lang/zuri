# Installation

You can install Zuri with a single command. The installer downloads the
right release for your machine, verifies it, and adds `zuri` to your `PATH`.
Releases are available for:

- Linux on x86-64 and ARM with the GNU C library 2.28 or later
  (`x86_64-unknown-linux-gnu`, `aarch64-unknown-linux-gnu`). This includes
  Debian 10, Ubuntu 20.04, RHEL 8, Amazon Linux 2023 and newer versions of
  each.
- Linux on x86-64 and ARM with musl, such as Alpine
  (`x86_64-unknown-linux-musl`, `aarch64-unknown-linux-musl`)
- macOS on Intel and Apple silicon (`x86_64-apple-darwin`,
  `aarch64-apple-darwin`)
- Windows on x86-64 (`x86_64-pc-windows-msvc`)

You can also build Zuri from source with Cargo, as described at the end of
this page.

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

The installer downloads the archive for your machine and checks it against
the checksum published with it. Next, it runs the new `zuri` once to make
sure it reports the expected version. Only then does it move the files into
`~/.zuri/runtime`, so a failed or tampered download can't replace a working
installation. The installer links `zuri` into `~/.zuri/bin`, which is also
where globally installed tools put their launchers, and adds that directory
to your `PATH` in your shell's startup file. That's `.zshrc` for zsh,
`.bashrc` for bash (`.bash_profile` on macOS), `config.fish` for fish, and
`.profile` for any other shell.

On Linux distributions that use musl, the installer picks the musl build,
which runs on a plain Alpine system without any extra packages. It loads
shared libraries the same way Alpine's own programs do. That's what allows
[`ffi`](ch26-00-ffi.md) to open them, and it's also why the build needs the
musl loader to be present. The musl build doesn't run on Linux systems that
use the GNU C library.

To pass options to the installer, put them after `sh -s --`:

```console
$ curl -fsSL https://zuri-lang.github.io/zuri/install.sh | sh -s -- --version 0.2.0
```

| Option | What it does |
| --- | --- |
| `--version <version>` | Installs the given release instead of the newest one |
| `--prerelease` | Includes pre-releases when looking for the newest release |
| `--no-modify-path` | Leaves your shell's startup files unchanged |

## Windows

In PowerShell:

```console
> powershell -c "irm https://zuri-lang.github.io/zuri/install.ps1 | iex"
```

The Windows installer performs the same checks and installs Zuri into
`%USERPROFILE%\.zuri\runtime`. It adds that directory to your user `PATH`,
along with `%USERPROFILE%\.zuri\bin`, where globally installed tools put
their launchers. Any terminal you open afterwards will have both, and so
will the terminal the installer ran in.

On Windows, the options are parameters. To use them, run the installer as a
script block:

```console
> & ([scriptblock]::Create((irm https://zuri-lang.github.io/zuri/install.ps1))) -Version 0.2.0
```

`-Version`, `-Prerelease` and `-NoModifyPath` work like the options
described above.

## Checking the Installation

```console
$ zuri
Zuri 0.1.0 (running on ZuriVM 0.1.0), REPL/Interactive mode = ON
Build No. => 2026-09-10 23:19:10 UTC
Type ".exit" to quit, ".help" for help or ".credits" for more information
%>
```

The `%>` is the Zuri prompt. Type `.exit` and press the `ENTER` key to 
leave it.

`zuri --version` prints the same build information without starting a
session, which makes it the better choice in scripts and CI jobs.
`zuri --help` also lists every command that the runtime can run from the
current directory:

```console
$ zuri --version
Zuri 0.1.0 (running on ZuriVM 0.1.0)
Build No. => 2026-09-10 23:19:10 UTC
```

## Where Zuri Is Installed

Both installers put everything under `ZURI_HOME`. Unless you set that
variable, it's the `.zuri` directory in your home directory:

```text
~/.zuri/
  runtime/        zuri, with libs and cmds beside it
  bin/            the zuri link, and globally installed tools
```

`runtime` contains the executable, with copies of the `libs` and `cmds`
directories next to it. The two need to stay together. Most of the standard
library is written in Zuri, and the runtime finds it by looking for a `libs`
directory next to the executable. `cmds` works the same way for the commands
that come with the runtime.

Running an installer again installs over the existing copy. `zuri upgrade`
does the same thing from inside an installation, as described in [Bundles
and Upgrades](ch27-05-bundles.md#upgrading-zuri). To remove Zuri, delete
`~/.zuri/runtime` and the `zuri` link in `~/.zuri/bin`, and remove the line
the installer added to your shell's startup file.

If you set `ZURI_RELEASES_URL`, both installers and `zuri upgrade` read the
list of releases from that address instead, for example from a mirror. The
listing has to use the same format that GitHub does. If `GITHUB_TOKEN` is
set, it's sent along with requests to GitHub's API, which gives you a higher
rate limit.

## Downloading a Release Manually

Each release on the [releases
page](https://github.com/zuri-lang/zuri/releases) has one archive per
platform, named `zuri-<version>-<target>.tar.gz` (or `.zip` on Windows), and
a `.sha256` file next to it. Verify the archive against the checksum, unpack
it, and add the `zuri` executable it contains to your `PATH`. Keep the
`libs` and `cmds` directories next to the executable.

## Building From Source

To build Zuri, you need Cargo, Rust's package manager. If you don't have
Rust installed, you can get it from [rustup.rs](https://rustup.rs). It only
takes a minute, and it installs `cargo` for you.

```console
$ git clone https://github.com/zuri-lang/zuri
$ cd zuri
$ cargo patch-crates
$ cargo build --release
```

`cargo patch-crates` prepares the few dependencies that Zuri builds with its
own changes. You need to run it once after cloning the repository, and again
only when one of those changes is updated. The build tells you when that
happens.

The first build compiles a large number of native dependencies, so expect it
to take a few minutes. Later builds are incremental and much faster.

When the build finishes, you'll have an executable at `target/release/zuri`,
with copies of the `libs/` and `cmds/` directories next to it, arranged the
same way as in a release.

### Adding a Build to Your PATH

The simplest approach is to leave the binary and its two directories where
they are and create a symlink to the binary:

```console
$ sudo ln -s "$PWD/target/release/zuri" /usr/local/bin/zuri
```

This works because the runtime resolves symlinks before it looks for `libs`.
Copying only the binary somewhere else doesn't work.

If you want to keep the directories somewhere else, set `ZURI_ROOT` to the
directory that *contains* them:

```console
$ export ZURI_ROOT=/opt/zuri
$ ls /opt/zuri
cmds  libs
```

`ZURI_ROOT` takes priority over the directory next to the executable. This
is useful when you're working on the standard library itself and want a
build to pick up your changes right away:

```console
$ ZURI_ROOT=$PWD ./target/debug/zuri run myscript.zu
```

### Debug Builds

A plain `cargo build` produces `target/debug/zuri`. This build keeps the
runtime assertions that the optimized build removes, which makes it useful
when a program does something you didn't expect. The trade-off is that it
runs more slowly. Both builds can run everything in this book.
