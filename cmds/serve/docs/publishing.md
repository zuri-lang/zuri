# Publishing packages

A package is a project whose `project.toml` describes it:

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

## Choosing a name

A package name can contain lowercase letters, digits, hyphens and
underscores. It has to start with a letter and end with a letter or a
digit. The package is imported by its name with each hyphen replaced by an
underscore, so `http-extra` is imported with `import http_extra`. For the
same reason, two names that differ only in hyphens and underscores count
as the same package. You can't use a name that the standard library
already uses, or a reserved name such as `zuri` or `std`.

The first account to publish a package under a name becomes its owner,
and can add other owners.

## Which files are included

If the project is in a git repository, the package contains the files
that git tracks or would track. Otherwise, it contains every file in the
project. Some files are always left out: `.git` and `.zuri` directories,
a `tmp` directory at the root, log files, and `project.lock`.

To control what's included, use `include` and `exclude` in the
`[project]` table. When `include` is set, the package only contains the
files it names, plus `project.toml`. `exclude` removes the files it names.

Files that look like secrets, such as `.env` or a private key, are left
out, and the publish stops so you can check them. If one of those files
really belongs in the package, name it in `include`. To see exactly what
would be sent before you publish, run:

```sh
zuri publish --dry-run
```

## Publishing a version

```sh
zuri publish
```

A published version can never be changed. To fix a mistake, publish a
new version. All of a version's dependencies have to come from a
registry, because a git or path dependency only exists on your machine.

## The API reference

Each version's page has an API tab next to its README. It has a page for
each public module, listing every function, constant and class the
module declares, taken from the doc blocks in the published files. The
reference is created when the version is published, so you don't have to
build or upload anything. The registry reads the code, but never runs
it.

Every `.zu` file counts as a module, except for:

- test files, and files in `tests`, `docs`, `examples`, `benchmarks` or
  `cmds`
- files with a part of their path that starts with an underscore, which
  the compiler treats as private

Each version's page shows how many of its public names have a doc
block. `zuri publish` reports the same count before it sends anything,
and lists the first few names that don't have one.

Every name in the reference links to the line that declares it, on the
version's Source tab. That tab shows every file in the version, with line
numbers and syntax highlighting for Zuri code.

## The package guide

Documentation in the package's `docs` directory, built with `zuri doc`,
is the package's guide. If the registry serves guides, the guide is built
when the version is published, and linked from the version's page. It's
built only from the package's own files, so every file that its
`doc.toml` refers to, and every file its pages include, has to be in the
package.

A name in braces that the package's own pages don't document, such as
`{http.Request}` or `{weather_kit.forecast()}`, links to where it's
documented. That's either the standard library reference, which this
registry serves at [/std](/std) for the version of Zuri it runs on, or
the API reference of a package on this registry that the package depends
on.

## Yanking a version

```sh
zuri yank http-extra@1.4.1
zuri yank http-extra@1.4.1 --undo
```

Projects that already have a yanked version in their lockfile can still
download it, but it's never chosen for new installs.

## Owners

```sh
zuri owner list http-extra
zuri owner add http-extra grace
zuri owner remove http-extra ada
```

Every owner can publish versions, yank versions, and add or remove
owners. A package always has at least one owner.

## Using another registry

`publish`, `yank` and `owner` use your default registry unless you name
another one. To publish here when this isn't your default registry,
sign in to it, and then name it in each command:

```sh
zuri account login --registry {{url}}
zuri publish --registry {{url}}
zuri yank http-extra@1.4.1 --registry {{url}}
```

A dependency from a different registry has to name that registry in
`project.toml`, so that everyone who installs the package can find it:

```toml
[dependencies]
internal-tools = { version = "^2", registry = "{{url}}" }
```

A dependency without a registry comes from the registry that the package
itself is published on.
