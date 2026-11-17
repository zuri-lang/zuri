# restore

Installs the exact package versions recorded in the project's lockfile,
`project.lock`.

```sh
zuri restore [options]
```

Run it after cloning a project, or on a build server. Each package is
installed at the version and from the source that the lockfile records,
and checked against the checksum recorded there. Packages that are already
installed at the right version are left alone, and installed packages that
the lockfile no longer lists are removed.

If the lockfile no longer matches `project.toml`, the project is resolved
again and the lockfile is rewritten. With `--frozen`, the command stops
with an error instead and lists every difference. Use `--frozen` in a
build that has to install exactly what was reviewed.

## Options

| Option | Description |
| --- | --- |
| `--frozen` | Stop with an error instead of resolving again if `project.lock` is out of date. |
| `-P, --production` | Leave out packages that only development dependencies need. |
| `--verify` | Check whether any installed package has changed since it was installed, and reinstall any that has. |
| `-g, --global` | Restore the packages installed for your user. |
| `--offline` | Only use what's already downloaded. |
| `-n, --dry-run` | Show what would change without changing anything. |
| `--force` | Replace directories in `.zuri/libs` that weren't installed by zuri. |
| `--no-hooks` | Don't run any install scripts. |
| `--allow-hooks <package...>` | Allow these packages to run their scripts this time. |
| `--insecure` | Allow sending a token over plain HTTP to another machine. |
| `-q, --quiet` | Only print the result. |
