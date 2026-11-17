# owner

Lists, adds and removes the owners of a package.

```sh
zuri owner list <package>
zuri owner add <package> <username>
zuri owner remove <package> <username>
```

Every owner can publish new versions, yank versions, and add or remove
owners. The account that first publishes a package becomes its owner. A
package must always have at least one owner, so you can't remove the last
one.

## Options

| Option | Description |
| --- | --- |
| `-r, --registry <registry>` | The registry the package is on, as an alias or an address. |
| `--insecure` | Allow sending a token over plain HTTP to another machine. |
| `--json` | Print the result as JSON. |
