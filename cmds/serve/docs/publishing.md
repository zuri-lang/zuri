# Publishing packages

A package is a project whose `project.toml` states what it is:

```toml
[project]
name = "http-extra"
version = "1.4.2"
description = "Helpers for building HTTP services."
license = "MIT"
readme = "README.md"
homepage = "https://example.com/http-extra"
repository = "https://example.com/http-extra.git"
keywords = ["http", "web"]
```

## The name

A name is lowercase letters, digits, hyphens and underscores,
beginning with a letter and ending with a letter or digit. It is
imported with every hyphen made an underscore, so `http-extra` is
`import http_extra`, and two names that differ only in hyphens and
underscores are the same package. A name the standard library already
uses cannot be taken.

The first account to publish a name owns it, and may add other owners.

## What goes in

Everything git tracks or would track, less `.git`, `.zuri` and log
files. `include` and `exclude` in `[project]` narrow that down. Files
that look like secrets, such as `.env` or a private key, are refused
unless `include` names them outright. See exactly what would be sent
first:

```sh
zuri publish --dry-run
```

## Publishing a version

```sh
zuri publish
```

A published version never changes. To fix one, publish the next
version. A version's dependencies must all come from a registry, since
a git or path dependency means nothing on anyone else's machine.

## Yanking a version

```sh
zuri yank http-extra@1.4.1
zuri yank http-extra@1.4.1 --undo
```

A yanked version stays downloadable for projects that already locked
it, and nothing new chooses it.
