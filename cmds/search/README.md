# search

Finds packages on a registry by name, description or keyword.

```sh
zuri search <words...> [options]
```

| Flag | What it does |
| --- | --- |
| `-r, --registry <registry>` | The registry to search. |
| `-p, --page <page>` | Which page of results. Default `1`. |
| `-l, --limit <count>` | Results per page, up to 100. Default `20`. |
| `--sort <order>` | `relevance`, `downloads`, `updated` or `name`. Default `relevance`. |
| `--json` | Print the results as JSON. |
