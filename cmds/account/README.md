# account

Signs in to a registry, and manages the account and its tokens there.

```sh
zuri account create
zuri account login [--username <name>] [--token-stdin]
zuri account logout
zuri account whoami
zuri account tokens
zuri account token [name] [--scopes <scope...>] [--days <days>]
zuri account revoke <id>
```

Signing in exchanges the password for a token, and the token is what
every later command sends. The password is never stored. Tokens are
kept in `ZURI_HOME/credentials.toml`, readable by its owner alone, one
per registry.

`create` and a password `login` need a terminal. A build server signs
in with a token instead, from standard input or the environment:

```sh
echo "$NYSSA_TOKEN" | zuri account login --token-stdin
ZURI_TOKEN_PRIVATE=nys_... zuri publish --registry private
```

`ZURI_TOKEN_<ALIAS>` is the token for the registry with that alias,
and `ZURI_TOKEN` the token for any registry. Both win over the
credentials file.

## Tokens

`token` issues a new one and prints it once; the registry keeps only
its digest. `--scopes` limits what it may do:

| Scope | Allows |
| --- | --- |
| `publish` | publishing new versions |
| `yank` | yanking and unyanking |
| `owners` | changing a package's owners |
| `account` | issuing and revoking tokens |

A token lasts 365 days unless `--days` says otherwise. `tokens` lists
them by id, and `revoke` ends one. `logout` revokes the saved token on
the registry and forgets it here.

## Flags

| Flag | What it does |
| --- | --- |
| `-r, --registry <registry>` | The registry, by alias or address. |
| `--json` | Print the result as JSON. |
