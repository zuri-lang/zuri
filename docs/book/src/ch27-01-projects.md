# Projects and Versions

## Starting a Project

```console
$ zuri init weather
$ cd weather
$ zuri run
Hello, world!
```

`zuri init` writes a project that runs and a test that passes, with
`project.toml` describing it. `zuri init --help` lists what it asks and
what it can be told up front.

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
| `name` | what the package is called, and what it is imported as |
| `version` | the version publishing sends, a semantic version |
| `description` | one sentence, shown in search results |
| `authors`, `license`, `readme`, `homepage`, `repository`, `keywords` | shown on the package's page; `license` is an SPDX expression |
| `zuri` | the versions of Zuri the package works with, as a range |
| `include`, `exclude` | which files publishing sends, as globs |

`[dependencies]` is what the project needs to run. `[dev-dependencies]`
is what working on it needs, such as test fixtures, and is never
required of a project that depends on this one. Both are managed by the
package commands, which keep every comment and blank line the file
already has.

Four more sections come up later in the chapter: `[registries]` in
[Installing Packages](ch27-02-installing.md#other-registries),
`[install]` and `[hooks]` in
[Install Scripts](ch27-02-installing.md#install-scripts), and
`[bundle]` in [Bundles and Upgrades](ch27-05-bundles.md).

## Names

A package name is lowercase letters, digits, hyphens and underscores,
starting with a letter and ending with a letter or digit, at most 64
characters. It is imported with every hyphen made an underscore:

| Package | Import |
| --- | --- |
| `http-extra` | `import http_extra` |
| `json-schema` | `import json_schema` |
| `orm_lite` | `import orm_lite` |

Because of that, two names that differ only in hyphens and underscores
are the same package, and a registry refuses the second. A name the
standard library already uses, such as `json` or `http`, cannot be a
package, since the package could never be imported past the standard
library module.

## Versions

A version is `major.minor.patch`, as
[Semantic Versioning](https://semver.org) lays it out:

- **patch** for fixes that change nothing anyone relies on,
- **minor** for additions that break nothing,
- **major** for anything that can break code written against the
  version before.

A pre-release, such as `2.0.0-rc.1`, sorts below its release, and
build metadata after a `+` is ignored when comparing.

## Ranges

A dependency says which versions it accepts:

| Range | Accepts |
| --- | --- |
| `1.4.2` or `^1.4.2` | `>=1.4.2, <2.0.0`: anything compatible |
| `^0.4.2` | `>=0.4.2, <0.5.0`: below `1.0`, a minor change may break |
| `^0.0.4` | exactly `0.0.4` |
| `~1.4.2` | `>=1.4.2, <1.5.0`: patches only |
| `=1.4.2` | exactly `1.4.2` |
| `1.4` | `>=1.4.0, <2.0.0`, a caret range like any bare version |
| `1.4.*`, `1.4.x` | `>=1.4.0, <1.5.0` |
| `*` | any release |
| `>=1.2, <1.8` | both at once; a comma or a space joins comparators |
| `^1.2 \|\| ^2.1` | either |

A bare version is a caret range, so the common case takes compatible
updates without saying so. `=` pins.

A pre-release is only chosen for a range that names a pre-release of
the same `major.minor.patch` itself: `^2.0.0-rc.1` accepts
`2.0.0-rc.3`, and `^1.4` accepts no pre-release at all. Nobody who
asked for releases is handed an unfinished version.
