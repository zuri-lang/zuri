# publish

Packs the project and publishes it to a package registry as a new version.

```sh
zuri publish [--dry-run] [--registry <registry>]
```

The version is the one in `project.toml`. A published version can't be
changed. If you publish the same files again, you're told that the version
is already published, and publishing different files under an existing
version fails.

## Which files are included

Inside a git repository, the package contains the files that git tracks or
would track. Outside one, it contains every file in the project. `.zuri`
and `.git` directories are always left out, at any depth. You can add or
remove files with the `include` and `exclude` globs in the `[project]`
table:

```toml
[project]
exclude = ["docs/drafts/**"]
```

Files that look like they contain secrets, such as `.env`, private keys
and credential files, are left out, and the publish stops and lists them.
If one of those files really belongs in the package, add it to `include`.

The archive is built the same way every time, with sorted entries, fixed
timestamps and owners, and normalized permissions. The same files always
produce the same bytes and the same checksum.

## Checks before anything is sent

- The project has a valid name and version.
- Git has no uncommitted changes, unless you pass `--allow-dirty`.
- The package has at most 20,000 files, unpacks to at most 256 MiB, and
  has no path longer than 240 characters. A registry can set a lower size
  limit of its own.
- The license is a valid SPDX expression. If it isn't, you get a warning.

`--dry-run` lists every file with its size, along with the size and
checksum of the archive, and doesn't send anything.

Both a real publish and a dry run report how many of the package's public
names have a doc block, counted the same way as on the registry's page for
the version, and list the first few that don't.

## Options

| Option | Description |
| --- | --- |
| `-r, --registry <registry>` | The registry to use, as an alias or an address. |
| `--insecure` | Allow sending a token over plain HTTP to another machine. |
| `-n, --dry-run` | Show exactly what would be published without publishing anything. |
| `--allow-dirty` | Publish even if there are uncommitted changes in git. |
| `-q, --quiet` | Only print errors. |
| `--json` | Print the result as JSON. |
