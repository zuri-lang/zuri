# Accounts and tokens

An account is what you publish with. Installing needs none.

## Creating an account

Create one on the [sign-up page](/signup), or from the command line:

```sh
zuri account create
```

You are given a recovery key when the account is made, and shown it
only once. It is the only way back in if the password is lost, so keep
it somewhere safe.

## Signing the command line in

```sh
zuri account login
zuri account whoami
```

`login` asks for your username and password and stores a token,
readable by you alone. The token is what every later command sends;
the password is never stored.

A token made on your [account page](/account) signs in without a
password, which is how a build server does it. Read it from standard
input so it never lands in the shell history:

```sh
zuri account login --token-stdin < token.txt
```

Or set it in the environment, where it takes precedence over anything
stored:

```sh
ZURI_TOKEN=nys_... zuri publish
```

## Tokens and scopes

```sh
zuri account tokens
zuri account token ci --scopes publish --days 90
zuri account revoke <id>
```

A token carries scopes that bound what it can do:

| Scope | Allows |
| --- | --- |
| `publish` | publishing new versions |
| `yank` | yanking and restoring versions |
| `owners` | adding and removing a package's owners |
| `account` | issuing and revoking tokens |

Give a build server a token with `publish` alone. A token lasts 365
days unless `--days` says otherwise, and is shown once, when it is
issued.

## Signing out

```sh
zuri account logout
```

That revokes the token on the registry and forgets it on your machine.

## Another registry

Every command above works with the default registry. For any other,
this one included when it is not your default, add `--registry` with
its address:

```sh
zuri account create --registry {{url}}
zuri account login --registry {{url}}
zuri account logout --registry {{url}}
```

An alias saves typing the address. Name it in `config.toml` in your
`ZURI_HOME`, which is `~/.zuri` unless you moved it:

```toml
[registries]
company = "{{url}}"
```

```sh
zuri account login --registry company
```

A token for one registry never goes to another. In the environment,
`ZURI_TOKEN_<ALIAS>` holds the token for the registry with that alias,
the alias in capitals with anything but letters and digits made `_`,
and wins over `ZURI_TOKEN`:

```sh
ZURI_TOKEN_COMPANY=nys_... zuri publish --registry company
```

To make this registry the one every command uses when none is named,
set `default` under `[registries]`, or `ZURI_REGISTRY` for one shell
or one build:

```sh
export ZURI_REGISTRY={{url}}
```
