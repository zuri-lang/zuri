# restore

Installs exactly the packages `project.lock` names.

```sh
zuri restore [options]
```

This is what a fresh checkout, a teammate or a build server runs. Every
package installs at the version, and from the source, the lockfile
pins, checked against the checksum it recorded. Packages already
installed at those versions are left alone, and installed packages the
lockfile no longer names are removed.

When the lockfile no longer matches `project.toml`, the project is
resolved again and the lockfile rewritten. `--frozen` refuses instead,
listing every difference, which is what a build that must install
exactly what was reviewed wants.

| Flag | What it does |
| --- | --- |
| `--frozen` | Refuse, rather than resolve again, when the lockfile is out of date. |
| `-P, --production` | Leave out packages only development dependencies need. |
| `--verify` | Check every installed package's files against what was installed, and reinstall any that changed. |
| `-g, --global` | Restore the packages installed for this user. |
| `--offline` | Use only what is already downloaded. |
| `-n, --dry-run` | Show what would change. |
| `--force` | Replace directories zuri did not install. |
| `--no-hooks` | Run no scripts. |
| `--allow-hooks <package...>` | Let these packages run their scripts. |
| `-q, --quiet` | Print only the result. |
