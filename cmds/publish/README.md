# publish

Publishes the project as a new version on a registry.

```sh
zuri publish [--dry-run] [--registry <registry>]
```

The version is the one `project.toml` states, and a published version
never changes: publishing the same files again is reported as already
published, and publishing different files under a version that exists
is refused.

## What goes in

Inside a git repository, the files git tracks or would track, and
outside one, every file below the project. `.zuri` and `.git` never
go, at any depth. `[project] include` and `exclude`
narrow or widen that with globs:

```toml
[project]
exclude = ["docs/drafts/**"]
```

Files that look like they hold secrets, such as `.env`, private keys
and credential files, are left out, and the publish stops and names
them. A file meant to go is named in `include`.

The archive is built the same way every time: sorted entries, fixed
times and owners, and normalised permissions. The same files always
make the same bytes and the same checksum.

## Checks before anything is sent

- The project has a valid name and version.
- Git has no uncommitted changes, unless `--allow-dirty` says to go
  ahead.
- The package holds at most 20,000 files, unpacking to at most 256
  MiB, with no path longer than 240 characters. The registry may set a
  lower size limit of its own.
- The license reads as an SPDX expression; anything else is a warning.

`--dry-run` lists every file with its size, the archive's size and its
checksum, and sends nothing.

## Flags

| Flag | What it does |
| --- | --- |
| `-r, --registry <registry>` | The registry, by alias or address. |
| `-n, --dry-run` | Show exactly what would be published. |
| `--allow-dirty` | Publish even when git has uncommitted changes. |
| `-q, --quiet` | Print nothing but a failure. |
| `--json` | Print the result as JSON. |
