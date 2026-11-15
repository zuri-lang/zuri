# Projects and Versions

## Starting a Project

```console
$ zuri init weather
$ cd weather
$ zuri run
Hello, world!
```

`zuri init` creates a project that runs, with a test that passes and a
`project.toml` that describes it. `zuri init --help` lists the questions it
asks, and the options you can use to answer them in advance.

## project.toml

```toml
[project]
name = "weather"
version = "0.1.0"
description = "Shows the weather."
authors = ["Ada Lovelace <ada@example.com>"]
license = "MIT"
readme = "README.md"
homepage = "https://example.com/weather"
repository = "https://github.com/example/weather"
keywords = ["weather", "cli"]
zuri = ">=0.1"

[dependencies]
http-extra = "^1.4"

[dev-dependencies]
fixtures = "^0.3"
```

| Key | Meaning |
| --- | --- |
| `name` | The package's name, which is also the name it's imported by |
| `version` | The version that gets published, as a semantic version |
| `description` | A one-sentence description, shown in search results |
| `authors`, `license`, `readme`, `homepage`, `repository`, `keywords` | Shown on the package's page. `license` is an SPDX expression. |
| `zuri` | The versions of Zuri the package works with, as a range |
| `include`, `exclude` | Which files are published, as glob patterns |

`[dependencies]` lists what the project needs to run. `[dev-dependencies]`
lists what you need to work on it, such as test fixtures, and they're never
required by projects that depend on this one. Both sections are managed by
the package commands, which keep any comments and blank lines already in the
file.

Four more sections are described later in this chapter: `[registries]` in
[Installing Packages](ch27-02-installing.md#other-registries), `[install]`
and `[hooks]` in [Install Scripts](ch27-02-installing.md#install-scripts),
and `[bundle]` in [Bundles and Upgrades](ch27-05-bundles.md).

## Package Names

A package name consists of lowercase letters, digits, hyphens and
underscores. It must start with a letter, end with a letter or digit, and be
no longer than 64 characters. When you import a package, every hyphen in its
name becomes an underscore:

| Package | Import |
| --- | --- |
| `http-extra` | `import http_extra` |
| `json-schema` | `import json_schema` |
| `orm_lite` | `import orm_lite` |

Because of this, two names that only differ in their hyphens and underscores
refer to the same package, and a registry rejects the second one. A name
that the standard library already uses, such as `json` or `http`, can't be
used for a package, because the standard library module would always be
imported instead.

## Versions

A version has the form `major.minor.patch`, as described by [Semantic
Versioning](https://semver.org):

- Increase the **patch** version for fixes that don't change anything people
  rely on.
- Increase the **minor** version for additions that don't break anything.
- Increase the **major** version for any change that could break code
  written against the previous version.

A pre-release version, such as `2.0.0-rc.1`, sorts before the release it
leads up to, and build metadata after a `+` is ignored when comparing
versions.

## Version Ranges

A dependency specifies which versions it accepts:

| Range | Accepts |
| --- | --- |
| `1.4.2` or `^1.4.2` | `>=1.4.2, <2.0.0`, meaning any compatible version |
| `^0.4.2` | `>=0.4.2, <0.5.0`, because below `1.0`, a minor version change can break things |
| `^0.0.4` | Exactly `0.0.4` |
| `~1.4.2` | `>=1.4.2, <1.5.0`, meaning patch releases only |
| `=1.4.2` | Exactly `1.4.2` |
| `1.4` | `>=1.4.0, <2.0.0`, which is a caret range, like any plain version |
| `1.4.*`, `1.4.x` | `>=1.4.0, <1.5.0` |
| `*` | Any release |
| `>=1.2, <1.8` | Both conditions at once. Conditions can be separated by a comma or a space. |
| `^1.2 \|\| ^2.1` | Either range |

A plain version is treated as a caret range, so by default you get
compatible updates without having to ask for them. Use `=` to pin an exact
version.

A pre-release is only chosen for a range that itself names a pre-release of
the same `major.minor.patch` version. So `^2.0.0-rc.1` accepts `2.0.0-rc.3`,
but `^1.4` doesn't accept any pre-release at all. If you asked for releases,
you'll never get an unfinished version.
