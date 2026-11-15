# Commands From Packages

A package can add commands to `zuri`, written the same way as the runtime's
own commands: a `cmds` directory in the package, with one command per `.zu`
file or directory, as described in [Appendix I](appendix-09-commands.md).

```text
lint-tools/
├── project.toml
├── index.zu
└── cmds/
    └── lint/
        └── index.zu
```

When the package is installed in a project, its commands work from anywhere
inside that project and appear in `zuri --help`, marked with the package
they come from:

```console
$ zuri install lint-tools --dev
$ zuri lint
$ zuri --help
...
PACKAGE COMMANDS:
  lint       Check the project for the mistakes CI rejects. (from lint-tools)
```

The runtime's own commands come first, followed by the project's own
commands in `.zuri/cmds`, so a package can never replace either of them. If
two installed packages provide the same command, the second one is rejected
when you install it.

## Installing Tools for Your User

`--global` installs a package into `$ZURI_HOME` instead of into a project,
which is how you install a tool that you use everywhere:

```sh
zuri install lint-tools --global
zuri uninstall lint-tools --global
zuri update --global
```

The commands of a globally installed package work from any directory. Each
one also gets a launcher in `$ZURI_HOME/bin`, so if that directory is on
your `PATH`, you can run the command directly:

```sh
export PATH="$HOME/.zuri/bin:$PATH"
lint
```

`zuri install --global` tells you if that directory isn't on your `PATH`.
Globally installed packages can also be imported from any script, after the
project's packages and the standard library.
