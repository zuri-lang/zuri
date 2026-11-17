# install

Adds packages to a project, or installs every package the project
declares.

## Usage

```sh
zuri install [package[@range]...] [options]
```

```sh
zuri install http-extra             # the newest release, recorded as ^x.y.z
zuri install http-extra@^1.4        # the newest 1.x release, from 1.4.0 up
zuri install http-extra --exact     # recorded as =x.y.z instead
zuri install fixtures --dev         # only for working on the project
zuri install                        # everything project.toml declares
```

Packages come from the default registry unless `--registry` names another
one, as an alias or an address. Each package is installed in `.zuri/libs`
under its import name, which is its name with each hyphen replaced by an
underscore. For example, `http-extra` is imported with `import http_extra`.

If you don't name any packages, every declared dependency is installed at
the version that `project.lock` records. Only the parts of `project.toml`
that changed since the lockfile was written are resolved again.

## Git repositories and local directories

```sh
zuri install --tag v1.2.0 --git https://example.com/tools.git
zuri install --git git@example.com:acme/tools.git --branch main
zuri install --path ../shared
```

The package's name is read from its own `project.toml`. A git dependency
is pinned to the commit that its tag, branch or revision resolved to, and
to the checksum of the files at that commit, so `zuri restore` installs
the same files a year later. A path dependency is copied again every time
the project is installed.

## Guarantees

- **One version of each package.** The resolver looks for versions that
  satisfy every requirement at once, preferring the locked versions and
  then the newest ones. If there aren't any, it tells you exactly which
  requirements conflict.
- **No partial installs.** New packages are unpacked next to `.zuri/libs`
  and moved into place together. If anything fails, including a script,
  the installed packages are left as they were. If the command is killed
  partway through, the next command cleans up.
- **Your own files are left alone.** A directory in `.zuri/libs` that
  zuri didn't install is never replaced unless you pass `--force`.
- **Verified downloads.** Every archive has to match the checksum that the
  registry published with it.
- **One command at a time.** If two commands work on the same project at
  the same time, the second waits for the first.

## Install scripts

A package can name a script to run after it's installed. The project's own
script runs. A dependency's script only runs if `project.toml`
allows it by name:

```toml
[install]
allow-hooks = ["native-sqlite"]
```

`--allow-hooks` allows more packages for a single run, and `--no-hooks`
runs no scripts at all.

## Installing for your user

`--global` installs packages in `ZURI_HOME` instead of a project. If a
package provides commands, a launcher for each one is created in
`ZURI_HOME/bin`. Add that directory to your `PATH` to run them by name.

## Environment variables

| Variable | Description |
| --- | --- |
| `ZURI_REGISTRY` | The default registry, as an alias or an address, in place of the one named in `ZURI_HOME/config.toml`. A project that sets its own default registry still uses it. |
| `ZURI_TOKEN_<ALIAS>`, `ZURI_TOKEN` | The token for a registry. These take priority over the credentials file. |
| `ZURI_HOME` | Where global packages, settings and tokens are kept. Defaults to `~/.zuri`. |
| `ZURI_CACHE` | Where downloads are kept. Defaults to `ZURI_HOME/cache`. |

## Options

| Option | Description |
| --- | --- |
| `-r, --registry <registry>` | The registry to use, as an alias or an address. |
| `--insecure` | Allow sending a token over plain HTTP to another machine. |
| `-D, --dev` | Add the packages as development dependencies. |
| `-g, --global` | Install the packages for your user instead of the project. |
| `-E, --exact` | Record the exact version chosen instead of a caret range. |
| `--git <url>` | Install from a git repository instead of a registry. |
| `--tag <tag>` | The git tag to install. |
| `--branch <branch>` | The git branch to follow. |
| `--rev <commit>` | The git commit to install. |
| `--path <dir>` | Install from a directory on this machine. |
| `--offline` | Only use what's already downloaded. |
| `-n, --dry-run` | Show what would change without changing anything. |
| `--force` | Replace directories in `.zuri/libs` that weren't installed by zuri. |
| `--no-hooks` | Don't run any install or uninstall scripts. |
| `--allow-hooks <package...>` | Allow these packages to run their scripts this time. |
| `-q, --quiet` | Only print the result. |
