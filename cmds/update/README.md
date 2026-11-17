# update

Updates packages to the newest versions that their ranges allow.

```sh
zuri update [package...] [options]
```

If you don't name any packages, every package can be updated. If you do,
only the packages you name are updated, and every other package stays at
its locked version.

```sh
zuri update --dry-run      # show what's out of date
zuri update --check        # the same, but exit with status 1 if anything is
zuri update                # update within the declared ranges
zuri update http-extra -L  # change the range itself to the newest release
```

`--dry-run` lists each package with the version that's installed, the
newest version its range allows, and the newest release. If the newest
release could break code written for the installed version, it's marked.

`--latest` changes the ranges in `project.toml` to the newest release. An
exact range stays exact, and you get a warning for every package that moves
to a new major version.

## Options

| Option | Description |
| --- | --- |
| `-L, --latest` | Change each range in `project.toml` to the newest release. |
| `-n, --dry-run` | Show what's out of date without changing anything. |
| `--check` | Like `--dry-run`, but exit with status 1 if anything is out of date. |
| `-g, --global` | Update the packages installed for your user. |
| `--offline` | Only use what's already downloaded. |
| `--no-hooks` | Don't run any install scripts. |
| `--allow-hooks <package...>` | Allow these packages to run their scripts this time. |
| `--insecure` | Allow sending a token over plain HTTP to another machine. |
| `-q, --quiet` | Only print the result. |
| `--json` | Print the result as JSON. |
