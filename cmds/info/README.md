# info

Describes the project and its packages, or one package on a registry.

```sh
zuri info [--tree] [--verify]
zuri info <package[@version]> [--readme]
```

About the project: what `project.toml` says, and for every package the
range declared, the version locked and the version installed. A package
whose three do not agree is marked:

| Mark | Meaning |
| --- | --- |
| `missing` | locked but not installed |
| `drifted` | installed at another version or from another source |
| `modified` | its files changed since it was installed, with `--verify` |
| `unmanaged` | its directory is there, but zuri did not install it |
| `unlocked` | declared, but not in the lockfile yet |
| `extraneous` | installed, but the lockfile no longer names it |

`--tree` shows what depends on what, as deep as `--depth` allows.

About a package: its description, license, owners, links, what the
version depends on, and its versions with the yanked ones marked.
`--readme` prints its README.

| Flag | What it does |
| --- | --- |
| `-t, --tree` | Show the dependency tree. |
| `--depth <levels>` | How deep the tree goes. |
| `--verify` | Check installed files against what was installed. |
| `--readme` | Print the package's README. |
| `-g, --global` | Describe the packages installed for this user. |
| `-r, --registry <registry>` | The registry to ask about a package. |
| `--json` | Print the result as JSON. |
