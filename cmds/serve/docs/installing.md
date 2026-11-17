# Installing packages

Every package command works on the project that contains the current
directory. That's the nearest directory, starting from the current one
and going up, that contains a `project.toml` file.

## Adding a dependency

```sh
zuri install http-extra
zuri install http-extra@^1.4
zuri install fixtures --dev
```

If you don't give a version, the newest one is installed and recorded as
a caret range, such as `^1.4.2`, which accepts compatible updates. To
record the exact version, `=1.4.2`, pass `--exact`. A development
dependency is only installed for working on the project itself. Projects
that depend on yours don't install it.

The package is installed in `.zuri/libs`, and you import it by its name,
with each hyphen replaced by an underscore:

```zuri
import http_extra
```

A dependency can also come from a git repository, or from a directory on
your machine:

```sh
zuri install --tag v1.2.0 --git https://example.com/tools.git
zuri install --path ../shared
```

## Versions and the lockfile

`project.toml` lists the versions that the project accepts.
`project.lock` records the exact version that was chosen for every
package, including indirect dependencies, along with the checksum of what
was downloaded. Commit both files.

`zuri restore` installs exactly the versions in the lockfile. Run it
after cloning a project, or on a build server:

```sh
zuri restore --frozen
```

With `--frozen`, the command fails if the lockfile and `project.toml`
don't match, instead of resolving the versions again.

## Updating

```sh
zuri update --dry-run
zuri update
zuri update http-extra --latest
```

The first command lists the packages that are out of date. The second
updates every package to the newest version its range allows. The third
changes the range itself to the newest version available. That can be a
new major version with breaking changes, and if it is, you get a warning.

## Removing a package

```sh
zuri uninstall http-extra
```

Any dependencies that were only installed for that package are removed
too.

## Installing for your user

`--global` installs a package in your own `ZURI_HOME` instead of a
project. If the package provides commands, they're installed as programs
in `ZURI_HOME/bin`. Add that directory to your `PATH` to run them by
name.

## Install scripts

A package can ask to run a script after it's installed, or before it's
removed. A dependency's scripts only run if the project allows that
package by name:

```toml
[install]
allow-hooks = ["native-sqlite"]
```

## Using another registry

Packages come from your default registry unless you say otherwise. To
install from a different registry, including this one if it isn't your
default, give its address:

```sh
zuri install http-extra --registry {{url}}
```

The dependency is recorded along with its registry, so everyone who
installs the project gets it from the same place:

```toml
[dependencies]
http-extra = { version = "^1.4", registry = "{{url}}" }
```

A project that uses a registry often gives it an alias in
`project.toml`, so that every dependency and every member of the team
uses the same one:

```toml
[registries]
company = "{{url}}"

[dependencies]
http-extra = { version = "^1.4", registry = "company" }
```

```sh
zuri install internal-tools --registry company
zuri search json --registry company
```

You can also define an alias in `config.toml` in your `ZURI_HOME`, for
your own use in every project. To make this registry the one that's used
when no registry is named, set `default` under `[registries]` in that
file. To do the same for a single shell or build, set `ZURI_REGISTRY`:

```sh
ZURI_REGISTRY={{url}} zuri install http-extra
```

If a project sets its own `default`, that setting always applies to it,
whatever the environment says, because it's part of the project.
