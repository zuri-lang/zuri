# owner

Lists, adds and removes the publishers who own a package.

```sh
zuri owner list <package>
zuri owner add <package> <username>
zuri owner remove <package> <username>
```

Every owner may publish new versions, yank them, and change the owners.
The first account to publish a name owns it. A package always keeps at
least one owner, so removing the last is refused.

| Flag | What it does |
| --- | --- |
| `-r, --registry <registry>` | The registry the package is on. |
| `--json` | Print the owners as JSON. |
