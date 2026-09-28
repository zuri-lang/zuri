# upgrade

Replaces this installation of zuri with a newer release.

```sh
zuri upgrade [--version <version>] [--check] [options]
```

The release for this platform is downloaded, checked against its
published checksum, unpacked beside the installation, and run to
confirm the version it reports. Only then are the executable, the
standard library and the shipped commands swapped in together. If any
part of the swap fails, all of it is put back.

A release without a checksum is refused. An installation this user
cannot write to is refused before anything is downloaded, with the
reason. Moving to an older release needs `--allow-downgrade`.

`--check` says whether a newer release exists, and exits 1 when one
does, for a script that wants to know.

Releases come from the project's GitHub releases. `ZURI_RELEASES_URL`
points at another release listing, and `GITHUB_TOKEN` is sent when set.

## Flags

| Flag | What it does |
| --- | --- |
| `-V, --version <version>` | The release to install. Default the newest. |
| `--check` | Say whether a newer release exists. |
| `--prerelease` | Consider pre-releases too. |
| `--allow-downgrade` | Allow installing an older release. |
| `-n, --dry-run` | Download and check the release, and replace nothing. |
| `--json` | Print the result as JSON. |
