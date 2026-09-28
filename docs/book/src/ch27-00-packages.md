# Packages and Nyssa

A package is a Zuri project other projects can use. Zuri installs,
publishes and serves them itself: the commands ship with the runtime,
and so does Nyssa, the repository they talk to. There is nothing else
to install.

```console
$ zuri install http-extra
Resolving dependencies
Installing http-extra 1.5.0
Installing json-schema 1.2.0
  + http-extra 1.5.0
  + json-schema 1.2.0
Installed 2 packages.
```

```zuri,ignore
import http_extra
```

This chapter covers the whole of it:

1. [Projects and Versions](ch27-01-projects.md): what `project.toml`
   says, how packages are named, and how version ranges read.
2. [Installing Packages](ch27-02-installing.md): adding, updating and
   removing dependencies, the lockfile, and where packages can come
   from.
3. [Publishing Packages](ch27-03-publishing.md): accounts, tokens, and
   putting a version on a registry.
4. [Commands From Packages](ch27-04-commands.md): packages that add
   `zuri` commands, and installing tools for your user.
5. [Bundles and Upgrades](ch27-05-bundles.md): shipping a program to
   machines without Zuri, and keeping Zuri itself up to date.
6. [Running Nyssa](ch27-06-nyssa.md): hosting a repository for a team,
   a company, or the public.

## The Commands

| Command | What it does |
| --- | --- |
| `zuri init` | starts a project |
| `zuri install` | adds packages, or installs everything a project declares |
| `zuri uninstall` | removes packages and whatever only they needed |
| `zuri update` | moves packages to newer versions |
| `zuri restore` | installs exactly what the lockfile names |
| `zuri info` | describes the project's packages, or one on a registry |
| `zuri search` | finds packages on a registry |
| `zuri account` | signs in, and manages tokens |
| `zuri publish` | publishes a version |
| `zuri yank` | stops a version from being chosen |
| `zuri owner` | manages who may publish a package |
| `zuri clean` | frees the space downloads and installs take |
| `zuri bundle` | packages a program with a runtime |
| `zuri upgrade` | replaces this Zuri with a newer release |
| `zuri serve` | runs a Nyssa repository |

Every one of them answers `--help`, and every one that changes a
project answers `--dry-run` with exactly what it would do.

## What Holds It Together

- **A project is a directory with a `project.toml`.** Every command
  works on the project around the directory it runs in, found by
  looking upwards, the same way imports find it.
- **Packages install into the project.** They land in `.zuri/libs`,
  which `import` searches before the standard library. Two projects on
  one machine never share, or fight over, an installed package.
- **One version of each package.** Every requirement in the project is
  satisfied at once or the install stops and says which requirements
  clash. Nothing is installed twice at two versions.
- **The lockfile is the record.** `project.lock` pins every package to
  an exact version and the checksum of what was downloaded, and
  `zuri restore` reproduces it anywhere.
- **Nothing half done.** An install is staged beside `.zuri/libs` and
  swapped in whole, so a failure or an interrupted command leaves the
  project as it was.
