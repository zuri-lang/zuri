# update

Moves packages to the newest versions their ranges allow.

```sh
zuri update [package...] [options]
```

With no names, every package may move; with names, only those do, and
everything else keeps its locked version.

```sh
zuri update --dry-run      # what is out of date
zuri update --check        # the same, exiting 1 when anything is
zuri update                # move within the declared ranges
zuri update http-extra -L  # move the range itself to the newest release
```

`--dry-run` lists, for each package, the version installed, the one its
range allows now, and the newest release there is, marking a newest
release that would break code written against the installed one.
`--latest` rewrites the named dependencies' ranges in `project.toml` to
the newest release, keeping an exact range exact, and warns about every
move to a new major version.

| Flag | What it does |
| --- | --- |
| `-L, --latest` | Move ranges to the newest release there is. |
| `-n, --dry-run` | Show what is out of date and change nothing. |
| `--check` | Like dry run, exiting 1 when anything is out of date. |
| `-g, --global` | Update the packages installed for this user. |
| `--offline` | Use only what is already downloaded. |
| `--no-hooks` | Run no scripts. |
| `--allow-hooks <package...>` | Let these packages run their scripts. |
| `--json` | Print the table as JSON. |
| `-q, --quiet` | Print only the result. |
