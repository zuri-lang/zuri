# Installing Packages

## Adding a Dependency

```console
$ zuri install http-extra
Resolving dependencies
Installing http-extra 1.5.0
Installing json-schema 1.2.0
  + http-extra 1.5.0
  + json-schema 1.2.0
Installed 2 packages.
```

The newest version is taken and recorded in `project.toml` as a caret
range, `http-extra = "^1.5.0"`. Name a range to choose otherwise, pass
`--exact` to record `=1.5.0`, or `--dev` to add a development
dependency:

```sh
zuri install http-extra@^1.4
zuri install http-extra --exact
zuri install fixtures --dev
```

Everything the package needs is installed with it, and everything
lands in `.zuri/libs` under its import name:

```text
weather/
├── project.toml
├── project.lock
└── .zuri/
    ├── installed.toml
    └── libs/
        ├── http_extra/
        └── json_schema/
```

`zuri init` ignores `.zuri` in git, apart from `.zuri/cmds`, so
installed packages are never committed. `project.toml` and
`project.lock` are, and together they reproduce the directory.

## The Lockfile

`project.lock` records the exact version of every package the project
was resolved to, direct or not, where it came from, and the checksum of
what was downloaded:

```toml
version = 1

[[package]]
name = "http-extra"
version = "1.5.0"
source = "registry+https://pub.zurilang.org"
checksum = "sha256:c5b2e20f517379696623b2bb9af8b9f5459a81f62f1aaf3273d9eebbbca3a8af"
dependencies = ["json-schema"]

[[package]]
name = "json-schema"
version = "1.2.0"
source = "registry+https://pub.zurilang.org"
checksum = "sha256:c92464109e86305502e8acaca519d1f7cb2f515a3ab12f3aa26bd1ec67e66b55"
dependencies = []
```

`zuri restore` installs exactly that, and is what a fresh checkout, a
teammate or a build server runs:

```sh
zuri restore
zuri restore --frozen
zuri restore --production
```

A package whose download does not match its recorded checksum is
refused. `--frozen` refuses to go on when the lockfile no longer
matches `project.toml` instead of resolving again, which is what a
build that must install exactly what was reviewed wants.
`--production` leaves out what only development dependencies need.

`zuri install` with no package named does the same as `zuri restore`
after a change to `project.toml`: it resolves only what changed and
keeps every other package at its locked version.

## When Requirements Clash

Every package gets exactly one version, chosen so that every
requirement in the project holds at once. When no choice does, the
install stops, changes nothing, and explains the clash step by step:

```console
$ zuri install report-kit
Resolving dependencies
install: no set of versions satisfies every requirement:

(1) Because http-extra 1.5.0 depends on json-schema ^1.2 and http-extra 1.4.2 depends on json-schema ^1.2, http-extra requires json-schema ^1.2.
(2) Because report-kit 1.0.0 depends on json-schema ^2.0 and http-extra requires json-schema ^1.2 (1), report-kit 1.0.0 cannot be used with http-extra.
(3) Because report-kit 1.0.0 cannot be used with http-extra (2) and weather depends on http-extra 1.4, report-kit 1.0.0 cannot be used.
    Because report-kit 1.0.0 cannot be used (3) and weather depends on report-kit *, version solving failed.
```

Read from the bottom: `report-kit` needs `json-schema` 2, `http-extra`
needs `json-schema` 1, and a project cannot have both. The way out is
a newer `http-extra` that accepts `json-schema` 2, when there is one,
or doing without one of the two.

## Updating

```console
$ zuri update --dry-run
Resolving dependencies
  package      current  wanted  latest
  json-schema  1.2.0    1.2.0   2.0.0 (breaking)
```

`current` is what is installed, `wanted` the newest the declared range
allows, and `latest` the newest there is, marked when moving to it can
break code written against the current one.

```sh
zuri update                        # everything, within its range
zuri update http-extra             # one package; the rest stay locked
zuri update json-schema --latest   # move the range itself
zuri update --check                # exit 1 when anything is out of date
```

`--latest` rewrites the range in `project.toml` to the newest release,
keeping an exact range exact, and says so for every move to a new major
version.

## Removing

```console
$ zuri uninstall http-extra
Resolving dependencies
  - http-extra 1.5.0
Removed 1 package.
```

Whatever was installed only because the package needed it goes too. A
package something else still needs stays, and `zuri uninstall` says
what needs it.

## Looking Around

```console
$ zuri info
weather 0.1.0
Shows the weather

  package      declared  locked  installed
  http-extra   1.4       1.5.0   1.5.0
  json-schema  -         1.2.0   1.2.0

$ zuri info --tree
weather
└── http-extra 1.5.0
    └── json-schema 1.2.0
```

A package whose declared range, locked version and installed version do
not agree is marked: `missing`, `drifted`, `unlocked`, `extraneous`,
or, with `--verify`, `modified` when its files changed since it was
installed.

The registry answers questions too:

```console
$ zuri search json
  json-schema  2.0.0  Validates data against JSON Schema drafts 4 to 2020-12.
  report-kit   1.0.0  Builds reports from JSON data.

2 packages on https://pub.zurilang.org, page 1 of 1

$ zuri info http-extra
http-extra 1.5.0
Helpers for building HTTP services: routing, sessions and rate limits.

  license     MIT
  owners      ada
  downloads   0
  depends on  json-schema ^1.2

versions: 1.5.0, 1.4.2, 1.0.0 (yanked)
```

## Git and Local Packages

A package does not have to be on a registry:

```sh
zuri install --tag v1.2.0 --git https://example.com/tools.git
zuri install --branch main --git git@example.com:acme/tools.git
zuri install --path ../shared
```

```toml
[dependencies]
tools = { git = "https://example.com/tools.git", tag = "v1.2.0" }
shared = { path = "../shared" }
```

The package's name comes from its own `project.toml`. A git dependency
is locked to the commit its tag, branch or revision pointed at and to
the checksum of what that commit packs to, so `zuri restore` installs
the same files long after the branch has moved. `zuri update` follows a
branch to its newest commit. A path dependency is copied afresh each
time the project is installed, which suits a package being worked on
beside the project that uses it.

A published package can only depend on registry packages, since a git
or path dependency means nothing on the machine of whoever installs it.

## Other Registries

Packages come from the default registry, `https://pub.zurilang.org`,
unless something says otherwise:

```sh
zuri install internal-tools --registry https://packages.example.com
```

The dependency records the registry it came from, so everyone who
installs the project gets it from the same place. A project that uses a
registry often names it once:

```toml
[registries]
company = "https://packages.example.com"

[dependencies]
internal-tools = { version = "^2", registry = "company" }
```

Aliases can also live in `$ZURI_HOME/config.toml` for your own use in
every project. `default` there, or `ZURI_REGISTRY` in the environment,
changes the registry used when nothing names one; a project that names
its own `default` keeps it.

A package never falls back from one registry to another. A package
asked for from one registry is only ever installed from that registry,
so a package with the same name elsewhere can never stand in for it.

## Install Scripts

A package may name scripts to run around its installation:

```toml
[hooks]
post-install = "scripts/setup.zu"
pre-uninstall = "scripts/teardown.zu"
```

A script can do anything the person running `zuri install` can, so a
dependency's scripts run only when the project allows that package by
name:

```toml
[install]
allow-hooks = ["native-sqlite"]
```

The project's own scripts always run. `--allow-hooks` allows more for
one run, and `--no-hooks` runs none. A script runs with `zuri run`,
from the package's directory, with no shell in between, and must finish
within ten minutes with status `0`; its output goes to a log under
`$ZURI_HOME/logs`, which a failure names. `ZURI_PACKAGE_NAME`,
`ZURI_PACKAGE_VERSION`, `ZURI_PACKAGE_DIR` and `ZURI_PROJECT_DIR` tell
it where it is. A failing script undoes the whole install.

## Offline, Caches and Space

Downloads are kept in a cache, checked by checksum, and shared by every
project on the machine. `--offline` installs from the cache alone, and
fails naming what is missing rather than reaching the network.

```sh
zuri clean                  # the cache and the script logs
zuri clean --libs           # this project's installed packages
zuri clean --older-than 30d
```

Everything `zuri clean` removes comes back by itself, downloaded again
or restored from the lockfile, the next time it is needed.

## The Environment

| Variable | What it does |
| --- | --- |
| `ZURI_HOME` | where your packages, settings and tokens live; `~/.zuri` by default |
| `ZURI_CACHE` | where downloads are kept; `$ZURI_HOME/cache` by default |
| `ZURI_REGISTRY` | the default registry, by alias or address |
| `ZURI_TOKEN`, `ZURI_TOKEN_<ALIAS>` | a registry token, before the saved one |
| `HTTPS_PROXY`, `HTTP_PROXY`, `NO_PROXY` | the proxy every download goes through |
| `NO_COLOR` | plain output |
