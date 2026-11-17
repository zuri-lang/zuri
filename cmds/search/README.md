# search

Searches a registry for packages by name, description or keyword.

```sh
zuri search <word...> [options]
```

## Options

| Option | Description |
| --- | --- |
| `-r, --registry <registry>` | The registry to search, as an alias or an address. |
| `--insecure` | Allow sending a token over plain HTTP to another machine. |
| `-p, --page <page>` | The page of results to show. Defaults to `1`. |
| `-l, --limit <count>` | The number of results per page, up to 100. Defaults to `20`. |
| `--sort <order>` | How to sort the results: `relevance`, `downloads`, `updated` or `name`. Defaults to `relevance`. |
| `--json` | Print the results as JSON. |
