# info

Shows information about the project and its packages, or about a package
on a registry.

```sh
zuri info [--tree] [--verify]
zuri info <package[@version]> [--readme]
```

## The project

Without a package name, `zuri info` shows what `project.toml` declares,
and for each package, the declared range, the locked version and the
installed version. If those don't agree, the package is marked:

| Mark | Meaning |
| --- | --- |
| `missing` | It's locked, but not installed. |
| `drifted` | It's installed at a different version, or from a different source. |
| `modified` | Its files have changed since it was installed. This is only checked with `--verify`. |
| `unmanaged` | Its directory exists, but zuri didn't install it. |
| `unlocked` | It's declared, but isn't in the lockfile yet. |
| `extraneous` | It's installed, but the lockfile no longer lists it. |

`--tree` shows the dependency tree. Use `--depth` to limit how many levels
are shown.

## A package on a registry

With a package name, `zuri info` shows the package's description, license,
owners and links, the dependencies of the version, and the list of
versions, with yanked versions marked. `--readme` prints the package's
README.

## Options

| Option | Description |
| --- | --- |
| `-t, --tree` | Show the dependency tree. |
| `--depth <levels>` | How many levels of the tree to show. Defaults to all of them. |
| `--verify` | Check whether any installed package has changed since it was installed. |
| `--readme` | Print the package's README. |
| `-g, --global` | Show the packages installed for your user. |
| `-r, --registry <registry>` | The registry to use, as an alias or an address. |
| `--insecure` | Allow sending a token over plain HTTP to another machine. |
| `--json` | Print the result as JSON. |
