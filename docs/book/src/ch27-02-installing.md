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

The newest version is installed and recorded in `project.toml` as a caret
range, such as `http-extra = "^1.5.0"`. To choose a different version, give
a range. Pass `--exact` to record `=1.5.0` instead, or `--dev` to add a
development dependency:

```sh
zuri install http-extra@^1.4
zuri install http-extra --exact
zuri install fixtures --dev
```

Everything the package needs is installed along with it, and every package
goes in `.zuri/libs` under its import name:

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

`zuri init` tells git to ignore `.zuri`, except for `.zuri/cmds`, so
installed packages are never committed. `project.toml` and `project.lock`
are committed, and together they're enough to recreate the directory.

## The Lockfile

`project.lock` records the exact version that every package was resolved to,
whether it's a direct dependency or not, along with where it came from and
the checksum of what was downloaded:

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

`zuri restore` installs exactly what the lockfile lists. It's what you run
after a fresh checkout, and what a teammate or a build server runs:

```sh
zuri restore
zuri restore --frozen
zuri restore --production
```

A package whose download doesn't match its recorded checksum is rejected.
`--frozen` stops with an error if the lockfile no longer matches
`project.toml`, instead of resolving the dependencies again, which is what
you want for a build that must install exactly what was reviewed.
`--production` leaves out anything that only development dependencies need.

`zuri install` without a package name does the same as `zuri restore` after
you've changed `project.toml`. It only resolves what changed, and keeps
every other package at its locked version.

## Conflicting Requirements

Every package gets exactly one version, chosen so that every requirement in
the project is satisfied at the same time. When that's not possible, the
install stops without changing anything, and explains the conflict step by
step:

```console
$ zuri install report-kit
Resolving dependencies
install: no set of versions satisfies every requirement:

(1) Because http-extra 1.5.0 depends on json-schema ^1.2 and http-extra 1.4.2 depends on json-schema ^1.2, http-extra requires json-schema ^1.2.
(2) Because report-kit 1.0.0 depends on json-schema ^2.0 and http-extra requires json-schema ^1.2 (1), report-kit 1.0.0 cannot be used with http-extra.
(3) Because report-kit 1.0.0 cannot be used with http-extra (2) and weather depends on http-extra 1.4, report-kit 1.0.0 cannot be used.
    Because report-kit 1.0.0 cannot be used (3) and weather depends on report-kit *, version solving failed.
```

Read it from the bottom up: `report-kit` needs version 2 of `json-schema`,
`http-extra` needs version 1, and a project can't have both. To fix it, use
a newer version of `http-extra` that accepts version 2 of `json-schema`, if
one exists, or stop using one of the two packages.

## Updating Packages

```console
$ zuri update --dry-run
Resolving dependencies
  package      current  wanted  latest
  json-schema  1.2.0    1.2.0   2.0.0 (breaking)
```

`current` is the installed version, `wanted` is the newest version the
declared range allows, and `latest` is the newest version available, marked
when updating to it could break code written against the current version.

```sh
zuri update                        # everything, within its range
zuri update http-extra             # one package; the rest stay locked
zuri update json-schema --latest   # move the range itself
zuri update --check                # exit 1 when anything is out of date
```

`--latest` changes the range in `project.toml` to the newest release,
keeping an exact range exact, and points out every update to a new major
version.

## Removing Packages

```console
$ zuri uninstall http-extra
Resolving dependencies
  - http-extra 1.5.0
Removed 1 package.
```

Anything that was only installed because the package needed it is removed as
well. A package that something else still needs is kept, and
`zuri uninstall` tells you what needs it.

## Getting Information About Packages

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

A package whose declared range, locked version and installed version don't
agree is marked with one of these states: `missing`, `drifted`, `unlocked`
or `extraneous`. With `--verify`, a package whose files have changed since
it was installed is marked as `modified`.

You can also ask the registry for information:

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

A package doesn't have to come from a registry:

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

The package's name is taken from its own `project.toml`. A git dependency is
locked to the commit that its tag, branch or revision pointed to, and to the
checksum of that commit's packaged files, so `zuri restore` installs the
same files even after the branch has moved on. `zuri update` updates a
branch dependency to its newest commit. A path dependency is copied again
every time the project is installed, which suits a package that you're
working on alongside the project that uses it.

A published package can only depend on packages from a registry, because a
git or path dependency wouldn't mean anything on the machine of whoever
installs it.

## Other Registries

Packages are installed from the default registry,
`https://pub.zurilang.org`, unless you say otherwise:

```sh
zuri install internal-tools --registry https://packages.example.com
```

The dependency records which registry it came from, so everyone who installs
the project gets it from the same place. A project that uses a registry
often can give it a name once:

```toml
[registries]
company = "https://packages.example.com"

[dependencies]
internal-tools = { version = "^2", registry = "company" }
```

You can also define aliases in `$ZURI_HOME/config.toml`, for your own use in
every project. Setting `default` there, or setting `ZURI_REGISTRY` in the
environment, changes the registry that's used when none is named, although a
project that defines its own `default` keeps using it.

A package is never installed from a different registry as a fallback. A
package requested from one registry is only ever installed from that
registry, so a package with the same name on another registry can never be
substituted for it.

## Install Scripts

A package can specify scripts to run before and after it's installed:

```toml
[hooks]
post-install = "scripts/setup.zu"
pre-uninstall = "scripts/teardown.zu"
```

A script can do anything that the person running `zuri install` can do, so a
dependency's scripts only run if the project allows that package by name:

```toml
[install]
allow-hooks = ["native-sqlite"]
```

The project's own scripts always run. `--allow-hooks` allows more packages
for a single run, and `--no-hooks` doesn't run any scripts. A script runs
with `zuri run`, from the package's directory, without a shell in between,
and it has to finish within ten minutes with status `0`. Its output goes to
a log file under `$ZURI_HOME/logs`, which is named in the error if the
script fails. `ZURI_PACKAGE_NAME`, `ZURI_PACKAGE_VERSION`,
`ZURI_PACKAGE_DIR` and `ZURI_PROJECT_DIR` tell the script where it's
running. If a script fails, the whole install is undone.

## Working Offline, Caches and Disk Space

Downloads are kept in a cache, checked against their checksums, and shared
by every project on the machine. `--offline` installs packages from the
cache only, and if something's missing, it fails with an error that names
what's missing instead of using the network.

```sh
zuri clean                  # the cache and the script logs
zuri clean --libs           # this project's installed packages
zuri clean --older-than 30d
```

Anything that `zuri clean` removes comes back automatically the next time
it's needed, either downloaded again or restored from the lockfile.

## Environment Variables

| Variable | What it does |
| --- | --- |
| `ZURI_HOME` | Where your packages, settings and tokens are stored. The default is `~/.zuri`. |
| `ZURI_CACHE` | Where downloads are cached. The default is `$ZURI_HOME/cache`. |
| `ZURI_REGISTRY` | The default registry, as an alias or an address |
| `ZURI_TOKEN`, `ZURI_TOKEN_<ALIAS>` | A registry token, which takes priority over the saved one |
| `HTTPS_PROXY`, `HTTP_PROXY`, `NO_PROXY` | The proxy used for every download |
| `NO_COLOR` | Turns off colored output |
