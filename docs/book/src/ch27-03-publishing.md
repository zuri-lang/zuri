# Publishing Packages

## Creating an Account

To publish packages, you need an account on the registry. You don't need one
to install packages.

```sh
zuri account create
```

This command asks for a username, an email address and a password, creates
the account and signs in the command line. It also prints a recovery key,
which is only shown once and is the only way back into the account if you
lose the password. You can also create an account on the registry's website
and sign in afterwards:

```console
$ zuri account login
$ zuri account whoami
ada on https://pub.zurilang.org
```

Signing in exchanges your password for a token, and every later command
sends the token instead. The password itself is never stored. Tokens are
stored in `$ZURI_HOME/credentials.toml`, one for each registry, in a file
that only you can read.

## Tokens

```console
$ zuri account token ci --scopes publish --days 90
$ zuri account tokens
  id                name                 scopes                          expires
  74fed0927f53cc20  ci                   publish                         2026-10-28
  544af66ae6171536  zuri account create  publish, yank, owners, account  2027-09-28
$ zuri account revoke 74fed0927f53cc20
```

A token has scopes that limit what it can do:

| Scope | Allows |
| --- | --- |
| `publish` | Publishing new versions |
| `yank` | Yanking and restoring versions |
| `owners` | Adding and removing a package's owners |
| `account` | Issuing and revoking tokens |

A token is only shown once, when it's issued, and the registry only stores a
digest of it. It's valid for 365 days unless you set a different number with
`--days`. `zuri account logout` revokes the saved token and removes it.

A build server should sign in with a token instead of a password. It can
read the token from standard input, so that it never appears in the shell
history or the process list, or take it from the environment:

```sh
echo "$PUBLISH_TOKEN" | zuri account login --token-stdin
ZURI_TOKEN="$PUBLISH_TOKEN" zuri publish
```

Give it a token that only has the `publish` scope, so that even if the token
leaks, it can't be used to yank versions, change owners or issue more
tokens.

## What Gets Published

A package consists of the project's files, packed into an archive. Inside a
git repository, that means the files git tracks, or would track. Outside
one, it means every file under the project. `.git` and `.zuri` are never
included. `include` and `exclude` in `[project]` narrow or widen that
selection with glob patterns, where `*` matches within one directory and
`**` matches across any number of directories:

```toml
[project]
exclude = ["docs/drafts/**", "*.log"]
```

Files that look like they contain secrets, such as `.env`, private keys and
credential files, are left out, and the publish stops and lists them. If one
of those files really is meant to be published, add it to `include`.

Check exactly what would be sent first. This project leaves out its tests
with `exclude = ["tests"]`:

```console
$ zuri publish --dry-run
Would publish weather 0.1.0

  53 B   .gitattributes
  250 B  .gitignore
  472 B  README.md
  541 B  app/index.zu
  387 B  index.zu
  177 B  project.toml

6 files, 1.8 KiB unpacked, 1.6 KiB packed
sha256:f617dec6d820dd15409fdfa4954d53d6e2809b969a5ab23c997b7476456b7e19
Documented: 2 of 2 public names (100%)
```

The last line counts how many of the public names in the package's modules
have a doc comment, in the same way the registry counts them for the
version's page, and lists the first few that don't have one. Publishing
shows the same information before it sends anything.

The archive is built the same way every time, with sorted entries, fixed
times and owners, and normalized permissions. The same files always produce
the same bytes and the same checksum, on any machine.

## Publishing a Version

```sh
zuri publish
```

Before anything is sent, the project must have a valid name and version, git
must have no uncommitted changes (you can use `--allow-dirty` to publish
anyway), and the package can contain at most 20,000 files, which can unpack
to at most 256 MiB. A license that isn't a valid SPDX expression produces a
warning. The registry then checks the archive again itself, and rejects
anything that isn't safe to unpack.

A published version never changes. Publishing the same files again is
reported as already published, and publishing different files under an
existing version is rejected. To fix a version, publish the next one.

The first account to publish a package name owns it.

Each version's page on the registry has an API tab next to its README. It
shows the package's API reference, generated from the doc comments in the
published files in the same way `zuri doc generate` generates one, with a
page for each public module ([Chapter
30](ch30-00-documentation.md#generating-an-api-reference)). You don't need
to build or upload anything for it, because the registry reads the code
without ever running it. Each name links to the line that declares it on the
Source tab, where you can read every file in the version, and searching the
registry's site for a name finds it in the newest version of each package. A
name in braces that the package doesn't document itself, such as
`{http.Request}`, links to the standard library reference, which the
registry serves for the version of Zuri it runs on, or to the API of a
package it depends on from the same registry.

Documentation in `docs`, built with `zuri doc`, is the package's guide. A
registry that serves guides builds the guide when the version is published
and links to it from the version's page. It only uses the package's own
files, so everything the guide's `doc.toml` names and its pages include has
to be part of the package ([Packages'
Guides](ch27-06-nyssa.md#packages-guides)).

## Yanking a Version

```sh
zuri yank http-extra@1.4.1
zuri yank http-extra@1.4.1 --undo
```

A yanked version stays on the registry, and any project whose lockfile
already lists it keeps installing it, so yanking never breaks an existing
build. It's only left out when versions are chosen again. Yank a version
that has a serious bug, but don't yank a version just to hide that it
existed.

## Package Owners

```sh
zuri owner list http-extra
zuri owner add http-extra grace
zuri owner remove http-extra ada
```

Every owner can publish, yank and change the owners of a package. A package
always has at least one owner.

## Using Another Registry

`publish`, `yank`, `owner` and `account` use the default registry unless
`--registry` names a different one, either by address or by alias:

```sh
zuri account login --registry company
zuri publish --registry company
```

A dependency from another registry has to name that registry in
`project.toml`, so that whoever installs the package can find it. A
dependency that doesn't name a registry comes from the registry the package
itself is published on.
