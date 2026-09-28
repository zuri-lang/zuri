# clean

Frees the space downloads, logs and installed packages take up.

```sh
zuri clean [--cache] [--runtimes] [--logs] [--libs] [--all] [options]
```

With nothing chosen, the download cache and the logs are cleaned.
Everything it removes is downloaded again, or put back by `zuri restore`, when it is
next needed.

| What | Where |
| --- | --- |
| `--cache` | downloaded archives, registry indexes and git checkouts |
| `--runtimes` | the runtimes `zuri bundle` downloaded |
| `--logs` | what install and uninstall scripts printed |
| `--libs` | the project's installed packages |

`--libs` removes only packages zuri installed. Code copied into
`.zuri/libs` by hand stays, and `.zuri/cmds` is never touched. A
command installing at the same moment is waited for, never cut short.

`--older-than 30d` keeps anything changed more recently. The units
are `s`, `m`, `h`, `d` and `w`.

## Flags

| Flag | What it does |
| --- | --- |
| `-c, --cache` | Remove the download cache. |
| `--runtimes` | Remove downloaded runtimes. |
| `-l, --logs` | Remove script logs. |
| `--libs` | Remove the installed packages. |
| `-a, --all` | All of the above. |
| `--older-than <age>` | Only what has not changed for this long. |
| `-g, --global` | With `--libs`, this user's packages. |
| `-n, --dry-run` | Say what would go and remove nothing. |
