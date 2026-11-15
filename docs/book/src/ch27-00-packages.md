# Packages and Nyssa

A package is a Zuri project that other projects can use. Zuri installs,
publishes and serves packages itself. The commands come with the runtime,
and so does Nyssa, the package registry they talk to, so there's nothing
else to install.

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

This chapter covers all of it:

1. [Projects and Versions](ch27-01-projects.md): what goes in
   `project.toml`, how packages are named, and how to read version ranges.
2. [Installing Packages](ch27-02-installing.md): adding, updating and
   removing dependencies, the lockfile, and the places packages can come
   from.
3. [Publishing Packages](ch27-03-publishing.md): accounts, tokens and
   publishing a version to a registry.
4. [Commands From Packages](ch27-04-commands.md): packages that add `zuri`
   commands, and installing tools for your user.
5. [Bundles and Upgrades](ch27-05-bundles.md): shipping a program to
   machines without Zuri, and keeping Zuri itself up to date.
6. [Running Nyssa](ch27-06-nyssa.md): hosting a registry for a team, a
   company or the public.

## The Commands

| Command | What it does |
| --- | --- |
| `zuri init` | Creates a project |
| `zuri install` | Adds packages, or installs everything a project declares |
| `zuri uninstall` | Removes packages, along with anything only they needed |
| `zuri update` | Updates packages to newer versions |
| `zuri restore` | Installs exactly what the lockfile lists |
| `zuri info` | Describes the project's packages, or a package on a registry |
| `zuri search` | Searches for packages on a registry |
| `zuri account` | Signs in and manages tokens |
| `zuri publish` | Publishes a version of a package |
| `zuri yank` | Stops a version from being chosen |
| `zuri owner` | Manages who can publish a package |
| `zuri clean` | Frees the disk space used by downloads and installs |
| `zuri bundle` | Packages a program together with a runtime |
| `zuri upgrade` | Updates Zuri itself to a newer release |
| `zuri serve` | Runs a Nyssa registry |

Every command accepts `--help`, and every command that changes a project
accepts `--dry-run`, which shows exactly what it would do.

## Key Concepts

- **A project is a directory with a `project.toml`.** Every command works on
  the project that contains the current directory, which it finds by
  searching upward, the same way imports do.
- **Packages are installed into the project.** They go in `.zuri/libs`,
  which `import` searches before the standard library. Two projects on the
  same machine never share or conflict over an installed package.
- **Each package has a single version.** Every requirement in the project
  has to be satisfied at the same time. If that's not possible, the install
  stops and tells you which requirements conflict. A package is never
  installed twice in two different versions.
- **The lockfile is the record.** `project.lock` pins every package to an
  exact version, along with the checksum of what was downloaded, and
  `zuri restore` reproduces it anywhere.
- **Nothing is left half done.** An install is prepared next to `.zuri/libs`
  and swapped in all at once, so if it fails or is interrupted, the project
  is left as it was.
