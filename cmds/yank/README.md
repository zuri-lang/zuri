# yank

Stops a published version from being chosen for new installs.

```sh
zuri yank <package@version> [--undo]
```

A yanked version stays on the registry. Every project whose lockfile
already names it keeps installing it, so yanking never breaks a build;
it is only left out when versions are chosen anew. `--undo` lets it be
chosen again. Only the package's owners may yank it.

| Flag | What it does |
| --- | --- |
| `-u, --undo` | Let the version be chosen again. |
| `-r, --registry <registry>` | The registry the package is on. |
| `--json` | Print the result as JSON. |
