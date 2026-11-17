# Accounts and tokens

You need an account to publish packages. You don't need one to install
them.

## Creating an account

You can create an account on the [sign-up page](/signup), or from the
command line:

```sh
zuri account create
```

When the account is created, you're given a recovery key, which is only
shown once. It's the only way to get back into your account if you lose
your password, so keep it somewhere safe.

## Signing in from the command line

```sh
zuri account login
zuri account whoami
```

`login` asks for your username and password, and saves a token that only
your user can read. Every later command sends that token. Your password
is never stored.

A token created on your [account page](/account) lets you sign in without
a password, which is how a build server should sign in. Read the token
from standard input, so that it doesn't end up in your shell history:

```sh
zuri account login --token-stdin < token.txt
```

Or set it in the environment, where it takes priority over a saved
token:

```sh
ZURI_TOKEN=nys_... zuri publish
```

## Tokens and scopes

```sh
zuri account tokens
zuri account token ci --scopes publish --days 90
zuri account revoke <id>
```

A token's scopes limit what it can do:

| Scope | Allows |
| --- | --- |
| `publish` | Publishing new versions. |
| `yank` | Yanking versions, and undoing a yank. |
| `owners` | Adding and removing a package's owners. |
| `account` | Creating and revoking tokens. |

Give a build server a token with only the `publish` scope. A token lasts
365 days unless you set `--days`, and it's only shown once, when it's
created.

## Signing out

```sh
zuri account logout
```

This revokes the token on the registry, and removes it from your machine.

## Using another registry

Every command on this page uses your default registry. For any other
registry, including this one if it isn't your default, add `--registry`
with its address:

```sh
zuri account create --registry {{url}}
zuri account login --registry {{url}}
zuri account logout --registry {{url}}
```

An alias saves you from typing the address. Define it in `config.toml`
in your `ZURI_HOME`, which is `~/.zuri` unless you've moved it:

```toml
[registries]
company = "{{url}}"
```

```sh
zuri account login --registry company
```

A token for one registry is never sent to another. In the environment,
`ZURI_TOKEN_<ALIAS>` holds the token for the registry with that alias,
and takes priority over `ZURI_TOKEN`. The alias is written in capitals,
with every character that isn't a letter or a digit replaced by `_`:

```sh
ZURI_TOKEN_COMPANY=nys_... zuri publish --registry company
```

To make this registry the one that every command uses when no registry
is named, set `default` under `[registries]` in the same file. To do the
same for a single shell or build, set `ZURI_REGISTRY`:

```sh
export ZURI_REGISTRY={{url}}
```
