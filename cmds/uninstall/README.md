# uninstall

Removes packages from a project, along with any dependencies that only
they need.

```sh
zuri uninstall <package...> [options]
```

Each package is removed from `project.toml`, and the project is resolved
again with every other package kept at its locked version. Anything that's
no longer needed is then removed from `.zuri/libs`. If another dependency
still needs one of the packages, that package stays installed, and the
command tells you which dependency needs it.

You can only uninstall a package that's declared in `project.toml`. If you
name one that was installed because another package depends on it, the
error tells you which packages those are.

If a package has a `pre-uninstall` script, it runs before the package is
removed, under the same rules as install scripts.

## Options

| Option | Description |
| --- | --- |
| `-g, --global` | Remove the packages from your user instead of the project. |
| `-n, --dry-run` | Show what would change without changing anything. |
| `--no-hooks` | Don't run any uninstall scripts. |
| `--allow-hooks <package...>` | Allow these packages to run their scripts this time. |
| `-q, --quiet` | Only print the result. |
