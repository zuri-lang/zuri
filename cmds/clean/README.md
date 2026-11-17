# clean

Frees up the disk space used by downloads, logs and installed packages.

```sh
zuri clean [--cache] [--runtimes] [--logs] [--libs] [--all] [options]
```

If you don't choose what to remove, the download cache and the logs are
removed. Anything this command removes is downloaded again, or brought back
by `zuri restore`, the next time it's needed.

| Option | What it removes |
| --- | --- |
| `--cache` | Downloaded archives, registry indexes and git checkouts. |
| `--runtimes` | The runtimes that `zuri bundle` downloaded. |
| `--logs` | The output of install and uninstall scripts. |
| `--libs` | The project's installed packages. |

`--libs` only removes packages that zuri installed. Code that you copied
into `.zuri/libs` yourself is kept, and `.zuri/cmds` is never changed. If
another command is installing packages at the same time, `zuri clean`
waits for it to finish instead of interrupting it.

To keep anything that changed recently, use `--older-than`. For example,
`--older-than 30d` only removes what hasn't changed in the last 30 days.
The units are `s`, `m`, `h`, `d` and `w`.

## Options

| Option | Description |
| --- | --- |
| `-c, --cache` | Remove the download cache. |
| `--runtimes` | Remove the downloaded runtimes. |
| `-l, --logs` | Remove the script logs. |
| `--libs` | Remove the project's installed packages. |
| `-a, --all` | Remove all of the above. |
| `--older-than <age>` | Only remove what hasn't changed for this long. |
| `-g, --global` | With `--libs`, remove your user's packages instead of the project's. |
| `-n, --dry-run` | Show what would be removed without removing anything. |
