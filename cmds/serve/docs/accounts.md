# Accounts and tokens

An account is what you publish with. Create one on the
[sign-up page](/signup), or from the command line:

```sh
zuri account create --registry {{url}}
```

You are given a recovery key when the account is made, and shown it
only once. It is the only way back in if the password is lost.

## Signing the command line in

```sh
zuri account login --registry {{url}}
```

That asks for your username and password and stores a token for this
registry, readable by you alone. The token is what every later command
sends; the password is never stored.

A token made on your [account page](/account) signs in without a
password, which is how a build server does it. Read it from standard
input so it never lands in the shell history:

```sh
zuri account login --registry {{url}} --token-stdin < token.txt
```

Or set it in the environment, where it takes precedence over anything
stored: `ZURI_TOKEN`, or `ZURI_TOKEN_<ALIAS>` for a registry you gave
an alias.

## Scopes

A token carries scopes that bound what it can do: `publish`, `yank`,
`owners` and `account`. Give a build server a token with `publish`
alone.

## Signing out

```sh
zuri account logout --registry {{url}}
```

That revokes the token here and forgets it on your machine.
