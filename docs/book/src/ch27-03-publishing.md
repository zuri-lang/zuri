# Publishing Packages

## An Account

Publishing needs an account on the registry. Installing needs none.

```sh
zuri account create
```

That asks for a username, an email address and a password, creates the
account, and signs the command line in. It also prints a recovery key,
once: the only way back into the account if the password is lost. An
account can be made on the registry's website just as well, and signed
in to afterwards:

```console
$ zuri account login
$ zuri account whoami
ada on https://pub.zurilang.org
```

Signing in exchanges the password for a token, and the token is what
every later command sends. The password is never stored. Tokens are
kept one per registry in `$ZURI_HOME/credentials.toml`, readable by you
alone.

## Tokens

```console
$ zuri account token ci --scopes publish --days 90
$ zuri account tokens
  id                name                 scopes                          expires
  74fed0927f53cc20  ci                   publish                         2026-10-28
  544af66ae6171536  zuri account create  publish, yank, owners, account  2027-09-28
$ zuri account revoke 74fed0927f53cc20
```

A token carries scopes that bound what it can do:

| Scope | Allows |
| --- | --- |
| `publish` | publishing new versions |
| `yank` | yanking and restoring versions |
| `owners` | adding and removing a package's owners |
| `account` | issuing and revoking tokens |

A token is shown once, when it is issued, and the registry keeps only a
digest of it. It lasts 365 days unless `--days` says otherwise, and
`zuri account logout` revokes the saved one and forgets it.

A build server signs in with a token rather than a password, read from
standard input so it never lands in the shell history or the process
list, or taken from the environment:

```sh
echo "$PUBLISH_TOKEN" | zuri account login --token-stdin
ZURI_TOKEN="$PUBLISH_TOKEN" zuri publish
```

Give it a token with the `publish` scope alone, and nothing it leaks
can yank, change owners or issue more tokens.

## What Goes In

A package is the project's files, packed. Inside a git repository that
is what git tracks or would track; outside one, every file below the
project. `.git` and `.zuri` never go. `include` and `exclude` in
`[project]` narrow or widen that with globs, `*` within one directory
and `**` across any number:

```toml
[project]
exclude = ["docs/drafts/**", "*.log"]
```

Files that look like they hold secrets, such as `.env`, private keys
and credential files, are left out, and the publish stops and names
them. A file that is meant to go is named in `include`.

See exactly what would be sent first. This project leaves its tests
out with `exclude = ["tests"]`:

```console
$ zuri publish --dry-run
Would publish weather 0.1.0

  53 B   .gitattributes
  217 B  .gitignore
  461 B  README.md
  541 B  app/index.zu
  376 B  index.zu
  206 B  project.toml

6 files, 1.8 KiB unpacked, 1.6 KiB packed
sha256:0a6b340e81682bde7e4364e2209333ba32c1f3852ff10558bf513550e2758b30
```

The archive is built the same way every time: sorted entries, fixed
times and owners, normalised permissions. The same files always make
the same bytes and the same checksum, on any machine.

## Publishing a Version

```sh
zuri publish
```

Before anything is sent, the project must have a valid name and
version, git must have no uncommitted changes (`--allow-dirty` goes
ahead anyway), and the package must hold at most 20,000 files that
unpack to at most 256 MiB. A license that does not read as an SPDX
expression is a warning. The registry then checks the archive again
for itself, and refuses anything unsafe to unpack.

A published version never changes. Publishing the same files again is
reported as already published; publishing different files under a
version that exists is refused. To fix a version, publish the next one.

The first account to publish a name owns it.

## Yanking

```sh
zuri yank http-extra@1.4.1
zuri yank http-extra@1.4.1 --undo
```

A yanked version stays on the registry, and every project whose
lockfile already names it keeps installing it, so yanking never breaks
a build. It is only left out when versions are chosen anew. Yank a
version with a serious bug; do not yank it to hide that it existed.

## Owners

```sh
zuri owner list http-extra
zuri owner add http-extra grace
zuri owner remove http-extra ada
```

Every owner may publish, yank and change the owners. A package always
keeps at least one.

## Another Registry

`publish`, `yank`, `owner` and `account` work with the default registry
unless `--registry` names another, by address or by alias:

```sh
zuri account login --registry company
zuri publish --registry company
```

A dependency from another registry must say which in `project.toml`,
so that whoever installs the package finds it. A dependency naming no
registry comes from the registry the package itself is published on.
