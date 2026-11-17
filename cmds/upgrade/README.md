# upgrade

Upgrades Zuri to a newer release.

```sh
zuri upgrade [--version <version>] [--check] [options]
```

The release for your platform is downloaded, checked against its published
checksum, unpacked next to the current installation, and run to confirm the
version it reports. Only then are the executable, the standard library and
the bundled commands replaced, all at the same time. If any part of the
replacement fails, everything is put back the way it was.

A release that has no published checksum isn't installed. If you don't have
permission to write to the installation, the command stops before it
downloads anything and tells you why. To install a release older than the
current one, pass `--allow-downgrade`.

`--check` only reports whether a newer release exists. It exits with status
1 if one does, so a script can test for it.

Releases are downloaded from the project's GitHub releases. To use another
release listing, such as a mirror, set `ZURI_RELEASES_URL`. If
`GITHUB_TOKEN` is set, it's sent with each request to GitHub, which raises
the rate limit.

## Options

| Option | Description |
| --- | --- |
| `-V, --version <version>` | The release to install. Defaults to the newest. |
| `--check` | Check whether a newer release exists, and exit with status 1 if one does. |
| `--prerelease` | Include pre-releases. |
| `--allow-downgrade` | Allow installing a release older than the current one. |
| `-n, --dry-run` | Download and check the release without installing it. |
| `--json` | Print the result as JSON. |
