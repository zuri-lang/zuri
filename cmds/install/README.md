# install

Adds packages to a project, or installs everything it declares.

## Running it

```sh
zuri install [package[@range]...] [options]
```

```sh
zuri install http-extra             # the newest release, recorded as ^x.y.z
zuri install http-extra@^1.4        # the newest 1.x from 1.4.0 up
zuri install http-extra --exact     # recorded as =x.y.z instead
zuri install fixtures --dev         # for working on the project only
zuri install                        # everything project.toml declares
```

A package comes from the default registry unless `--registry` names
another, by alias or by address. It installs into `.zuri/libs` under
its import name, every hyphen made an underscore, so `http-extra` is
`import http_extra`.

With nothing named, every declared dependency is installed at the
version `project.lock` pins. Only what `project.toml` changed since the
lockfile was written is resolved again.

## Git and paths

```sh
zuri install --tag v1.2.0 --git https://example.com/tools.git
zuri install --git git@example.com:acme/tools.git --branch main
zuri install --path ../shared
```

The package's name comes from its own `project.toml`. A git dependency
is pinned to the commit its tag, branch or revision resolved to, and to
the checksum of what that commit packs to, so `zuri restore` installs
the same files a year later. A path dependency is copied again every
time the project is installed.

## What it guarantees

- **One version of each package.** The resolver finds versions that
  satisfy every requirement at once, preferring what is already locked
  and then the newest, and says exactly which requirements clash when
  none do.
- **Nothing half installed.** New packages are unpacked beside
  `.zuri/libs` and swapped in together. A failure anywhere, a script
  included, leaves the installed packages as they were, and a command
  killed part way is put right by the next one.
- **Nothing it did not put there.** A directory in `.zuri/libs` that
  zuri did not install is never replaced unless `--force` says to.
- **Checked downloads.** Every archive must match the checksum the
  registry published with it.
- **One command at a time.** Two commands working on the same project
  take turns.

## Scripts

A package may name a script to run after it is installed. The
project's own runs; a dependency's runs only when `project.toml` allows
it by name:

```toml
[install]
allow-hooks = ["native-sqlite"]
```

`--allow-hooks` allows more for one run, and `--no-hooks` runs none.

## For your user

`--global` installs into `ZURI_HOME` rather than a project. A package
that provides commands gets a launcher for each in `ZURI_HOME/bin`,
which is worth adding to your `PATH`.

## The environment

| Variable | What it does |
| --- | --- |
| `ZURI_REGISTRY` | The default registry, by alias or address, in place of the one `ZURI_HOME/config.toml` names. A project that names its own default keeps it. |
| `ZURI_TOKEN_<ALIAS>`, `ZURI_TOKEN` | The token for a registry, before the credentials file. |
| `ZURI_HOME` | Where global packages, settings and tokens live. Default `~/.zuri`. |
| `ZURI_CACHE` | Where downloads are kept. Default `ZURI_HOME/cache`. |

## Flags

| Flag | What it does |
| --- | --- |
| `-r, --registry <registry>` | The registry, by alias or address. |
| `-D, --dev` | Add as a development dependency. |
| `-g, --global` | Install for this user rather than the project. |
| `-E, --exact` | Record the exact version rather than a caret range. |
| `--git <url>` | Install from a git repository. |
| `--tag`, `--branch`, `--rev` | Pin the git repository. |
| `--path <dir>` | Install from a directory on this machine. |
| `--offline` | Use only what is already downloaded. |
| `-n, --dry-run` | Show what would change. |
| `--force` | Replace directories zuri did not install. |
| `--no-hooks` | Run no scripts. |
| `--allow-hooks <package...>` | Let these packages run their scripts. |
| `-q, --quiet` | Print only the result. |
| `--insecure` | Allow a token over plain http to another machine. |
