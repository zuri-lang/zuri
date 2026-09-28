# Installing packages

Every package command works on the project around the directory it
runs in: the nearest directory above it holding a `project.toml`.

## Adding a dependency

```sh
zuri install http-extra
zuri install http-extra@^1.4
zuri install fixtures --dev
```

With no version, the newest is taken and recorded as a caret range,
`^1.4.2`, which accepts compatible updates. Pass `--exact` to record
`=1.4.2` instead. A development dependency is installed for working on
the project and is never required of anyone who depends on it.

The package lands in `.zuri/libs` and is imported by its name, with
every hyphen made an underscore:

```zuri
import http_extra
```

A dependency can also come from a git repository, or from a directory
on this machine:

```sh
zuri install --tag v1.2.0 --git https://example.com/tools.git
zuri install --path ../shared
```

## Versions and the lockfile

`project.toml` says which versions are acceptable. `project.lock` says
which exact version of every package, direct or not, the project was
resolved to, with the checksum of what was downloaded. Commit both.

`zuri restore` installs exactly what the lockfile names, and is what a
fresh checkout or a build server runs:

```sh
zuri restore --frozen
```

`--frozen` fails, rather than resolving again, when the lockfile and
`project.toml` disagree.

## Updating

```sh
zuri update --dry-run
zuri update
zuri update http-extra --latest
```

The first lists what is out of date. The second moves every package
to the newest version its range allows. The third moves a range itself
to the newest version there is, which can mean a major version with
breaking changes; those are called out when they happen.

## Removing

```sh
zuri uninstall http-extra
```

Anything that was installed only because the package needed it goes
too.

## For your user

`--global` installs into your own `ZURI_HOME` rather than a project.
Commands the package provides become programs in `ZURI_HOME/bin`, which
is worth adding to your `PATH`.

## Scripts that run on install

A package may ask to run a script after it is installed, or before it
is removed. A dependency's scripts only run when the project allows
that package by name:

```toml
[install]
allow-hooks = ["native-sqlite"]
```

## Another registry

Packages come from the default registry unless something says
otherwise. To install from another, this one included when it is not
your default, give its address:

```sh
zuri install http-extra --registry {{url}}
```

The dependency is recorded with the registry it came from, so everyone
who installs the project gets it from the same place:

```toml
[dependencies]
http-extra = { version = "^1.4", registry = "{{url}}" }
```

A project that uses a registry often gives it an alias in
`project.toml`, so every dependency and every teammate agrees on it:

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

An alias can also live in `config.toml` in your `ZURI_HOME`, for your
own use across projects. To make this registry the one used whenever
none is named, set `default` under `[registries]` there, or
`ZURI_REGISTRY` for one shell or one build:

```sh
ZURI_REGISTRY={{url}} zuri install http-extra
```

A project that names its own `default` keeps it, whatever the
environment says, since that is part of the project.
