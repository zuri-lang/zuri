# serve

Runs Nyssa, a package repository, for a team, a company, or the public.

```sh
zuri serve [options]
zuri serve migrate | check | backup <file> | admin <action> <username>
```

With no subcommand the repository starts and serves until it is
stopped. It is both the registry every package command speaks to and
a website for finding packages, reading their documentation, and
managing an account and its tokens.

## Settings

Every setting is a flag, a `NYSSA_` environment variable, or a line in
`nyssa.toml` in the storage directory, in that order of precedence.

| Flag | Variable | Default |
| --- | --- | --- |
| `-n, --host <address>` | `NYSSA_HOST` | `127.0.0.1` |
| `-p, --port <port>` | `NYSSA_PORT` | `3000` |
| `-s, --storage <dir>` | `NYSSA_STORAGE` | `ZURI_HOME/nyssa` |
| `-c, --config <file>` | `NYSSA_CONFIG` | `nyssa.toml` in the storage directory |
| `--public-url <url>` | `NYSSA_PUBLIC_URL` | `http://<host>:<port>`, or `https` with a certificate |
| `-w, --workers <count>` | `NYSSA_WORKERS` | one per CPU core, up to 8 |
| `-d, --database <url>` | `NYSSA_DATABASE` | SQLite in the storage directory |
| `--signup <mode>` | `NYSSA_SIGNUP` | `open`; `closed` for a private repository |
| `--max-archive-size <bytes>` | `NYSSA_MAX_ARCHIVE_SIZE` | `10485760` |
| `--trust-proxy` | `NYSSA_TRUST_PROXY` | off |
| `--read-only` | `NYSSA_READ_ONLY` | off |
| `--tls-cert <file>` | `NYSSA_TLS_CERT` | none |
| `--tls-key <file>` | `NYSSA_TLS_KEY` | none |
| | `NYSSA_NAME` | `Nyssa`, the name the site shows |
| | `NYSSA_MAIL_URL` | none; the SMTP server mail is sent through |
| | `NYSSA_MAIL_USERNAME` | none |
| | `NYSSA_MAIL_PASSWORD` | none |
| | `NYSSA_MAIL_FROM` | none; the address mail is sent from |

A variable switching something on takes `1`, `true`, `yes` or `on`,
and `0`, `false`, `no`, `off` or nothing to switch it off.

The database is SQLite unless `--database` names PostgreSQL, MySQL or
MariaDB, and the site is the same on each. Every start brings the
schema up to date. A password in a connection string belongs in
`NYSSA_DATABASE`, and the mail password in `NYSSA_MAIL_PASSWORD`.

## Subcommands

| Subcommand | What it does |
| --- | --- |
| `migrate` | brings the database up to date and exits |
| `check` | checks every stored archive against its checksum |
| `backup <file>` | copies a SQLite database to a file while it is in use |
| `admin create <username>` | creates an account, for a closed repository |
| `admin promote`, `demote` `<username>` | makes or unmakes an administrator, who may yank any version and change any package's owners |
| `admin suspend`, `restore` `<username>` | suspends an account, revoking its tokens, or restores it |

## Security

- Passwords are hashed with Argon2id. Tokens and recovery keys are
  kept only as digests, and every token carries scopes and an expiry.
- Five failed sign-ins lock an account for fifteen minutes, counted
  in the database so every worker sees them. Each worker also allows an
  address at most 30 attempts a minute to sign in, sign up or recover.
- Every form is protected against cross-site requests, pages are sent
  with a content security policy that allows no scripts, and README
  HTML is sanitised before it is shown.
- An archive is checked before it is stored: its checksum, its size,
  every path in it, and that its `project.toml` agrees with the name
  and version it was published as.
- Listening beyond this machine without a certificate prints a
  warning, since passwords and tokens would cross the network in the
  clear. Give it a certificate, or put it behind a proxy that
  terminates TLS and pass `--trust-proxy`.

The site's own documentation, under `/docs`, covers publishing,
installing, accounts and hosting in full.
