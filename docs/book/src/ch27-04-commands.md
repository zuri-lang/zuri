# Commands From Packages

A package can add commands to `zuri`, the way the runtime's own are
written: a `cmds` directory in the package, one command per `.zu` file
or directory, as [Appendix I](appendix-09-commands.md) describes.

```text
lint-tools/
├── project.toml
├── index.zu
└── cmds/
    └── lint/
        └── index.zu
```

Installed into a project, the package's commands work from anywhere
inside it and show up in `zuri --help`, marked with the package they
come from:

```console
$ zuri install lint-tools --dev
$ zuri lint
$ zuri --help
...
PACKAGE COMMANDS:
  lint       Check the project for the mistakes CI rejects. (from lint-tools)
```

The runtime's commands come first and a project's own `.zuri/cmds`
next, so a package can never replace either. Two installed packages
providing the same command is refused when the second is installed.

## Tools for Your User

`--global` installs into `$ZURI_HOME` instead of a project, which is
how a tool you use everywhere is installed:

```sh
zuri install lint-tools --global
zuri uninstall lint-tools --global
zuri update --global
```

A globally installed package's commands work from any directory. Each
one also gets a launcher in `$ZURI_HOME/bin`, so with that directory on
your `PATH`, the command runs on its own:

```sh
export PATH="$HOME/.zuri/bin:$PATH"
lint
```

`zuri install --global` says when the directory is missing from `PATH`.
Globally installed packages are importable from any script too, after
the project's packages and the standard library.
