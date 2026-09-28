# uninstall

Removes packages from a project, along with anything only they needed.

```sh
zuri uninstall <package...> [options]
```

Each package is taken out of `project.toml`, the project is resolved
again with every other package held at its locked version, and whatever
nothing needs any more leaves `.zuri/libs`. A package another
dependency still needs stays installed, and the command says which one
needs it. A name that is not declared is refused, with the packages
that pulled it in named when there are any.

A package's `pre-uninstall` script runs before it goes, under the same
rules as install scripts.

| Flag | What it does |
| --- | --- |
| `-g, --global` | Remove from this user rather than the project. |
| `-n, --dry-run` | Show what would change. |
| `--no-hooks` | Run no scripts. |
| `--allow-hooks <package...>` | Let these packages run their scripts. |
| `-q, --quiet` | Print only the result. |
