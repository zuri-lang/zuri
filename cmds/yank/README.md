# yank

Stops a published version of a package from being chosen for new installs.

```sh
zuri yank <name@version> [--undo]
```

A yanked version stays on the registry. Projects whose lockfile already
lists it keep installing it, so yanking a version doesn't break existing
builds. It's only skipped when a version has to be chosen. To let it be
chosen again, run the same command with `--undo`. Only the owners of a
package can yank its versions.

## Options

| Option | Description |
| --- | --- |
| `-u, --undo` | Allow a yanked version to be chosen again. |
| `-r, --registry <registry>` | The registry the package is on, as an alias or an address. |
| `--insecure` | Allow sending a token over plain HTTP to another machine. |
| `--json` | Print the result as JSON. |
