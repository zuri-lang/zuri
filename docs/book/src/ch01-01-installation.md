# Installation

Zuri is built from source with Rust's package manager, Cargo. If you do not
have Rust installed, get it from [rustup.rs](https://rustup.rs); it takes a
minute and installs `cargo` for you.

## Building

```console
$ git clone https://github.com/zuri-lang/zuri-rs
$ cd zuri-rs
$ cargo build --release
```

The first build compiles a large set of native dependencies, so it takes
a few minutes. Builds after that are incremental and quick.

When it finishes you have an executable at `target/release/zuri`, and,
sitting right next to it, copies of the `libs/` and `cmds/` directories.
That pairing matters: the standard library is written mostly in Zuri
itself, and the runtime finds it by looking for a `libs` directory beside
the executable. `cmds` is the same arrangement for the commands the
runtime ships. Move the binary somewhere without them, and `import os`
will stop working.

## Putting `zuri` on Your PATH

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
is the one to reach for from a script or a CI job:

```console
$ zuri --version
Zuri 0.1.0 (running on ZuriVM 0.1.0)
Build No. => 2026-09-10 23:19:10 UTC
```

## A Debug Build, and Why You Might Want One

Plain `cargo build` produces `target/debug/zuri`. It keeps the runtime
assertions that the optimised build strips out, which makes it the build to
reach for when a program is doing something you did not expect. It runs
more slowly in exchange. Either build runs everything in this book.
