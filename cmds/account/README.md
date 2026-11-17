# account

Signs in to a registry, and manages your account and its API tokens.

```sh
zuri account create
zuri account login [--username <name>] [--token-stdin]
zuri account logout
zuri account whoami
zuri account tokens
zuri account token [name] [--scopes <scope...>] [--days <days>]
zuri account revoke <id>
```

When you sign in, the registry exchanges your password for a token, and
every later command sends that token. Your password is never stored. Tokens
are saved in `ZURI_HOME/credentials.toml`, one for each registry, and only
your user can read the file.

`create`, and `login` with a password, need a terminal. On a build server,
sign in with a token instead, either from standard input or from the
environment:

```sh
echo "$NYSSA_TOKEN" | zuri account login --token-stdin
ZURI_TOKEN_PRIVATE=nys_... zuri publish --registry private
```

`ZURI_TOKEN_<ALIAS>` holds the token for the registry with that alias, and
`ZURI_TOKEN` holds a token for any registry. Both take priority over the
credentials file.

## Tokens

`token` creates a new token and prints it. It's only shown this once,
because the registry only keeps a digest of it. Use `--scopes` to limit
what the token can do:

| Scope | Allows |
| --- | --- |
| `publish` | Publishing new versions. |
| `yank` | Yanking versions, and undoing a yank. |
| `owners` | Changing a package's owners. |
| `account` | Creating and revoking tokens. |

A token lasts 365 days unless you set `--days`. `tokens` lists your tokens
by ID, and `revoke` revokes one. `logout` revokes the saved token on the
registry and removes it from this machine.

## Options

| Option | Description |
| --- | --- |
| `-r, --registry <registry>` | The registry to use, as an alias or an address. |
| `--insecure` | Allow sending a token over plain HTTP to another machine. |
| `--json` | Print the result as JSON. |
