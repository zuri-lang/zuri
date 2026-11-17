# serve

Runs Nyssa, a package registry, for a team, a company or the public.

```sh
zuri serve [options]
zuri serve migrate | check | backup <file> | admin <action> <username>
```

Without a subcommand, the registry starts and runs until it's stopped. It
serves both the API that the package commands use and a website where
people can find packages, read their documentation, and manage their
account and its tokens.

## Settings

Each setting can be given as an option, as a `NYSSA_` environment
variable, or in `nyssa.toml` in the storage directory. An option takes
priority over an environment variable, and an environment variable takes
priority over the file.

| Option | Variable | Default |
| --- | --- | --- |
| `-n, --host <address>` | `NYSSA_HOST` | `127.0.0.1` |
| `-p, --port <port>` | `NYSSA_PORT` | `3000` |
| `-s, --storage <dir>` | `NYSSA_STORAGE` | `ZURI_HOME/nyssa` |
| `-c, --config <file>` | `NYSSA_CONFIG` | `nyssa.toml` in the storage directory |
| `--public-url <url>` | `NYSSA_PUBLIC_URL` | `http://<host>:<port>`, or `https` with a certificate |
| `-w, --workers <count>` | `NYSSA_WORKERS` | One per CPU core, up to 8 |
| `-d, --database <url>` | `NYSSA_DATABASE` | SQLite in the storage directory |
| `--signup <mode>` | `NYSSA_SIGNUP` | `open`. Use `closed` for a private registry. |
| `--max-archive-size <bytes>` | `NYSSA_MAX_ARCHIVE_SIZE` | `10485760` |
| `--guides-url <url>` | `NYSSA_GUIDES_URL` | None. Without it, package guides aren't built or served. |
| `--trust-proxy` | `NYSSA_TRUST_PROXY` | Off |
| `--read-only` | `NYSSA_READ_ONLY` | Off |
| `--tls-cert <file>` | `NYSSA_TLS_CERT` | None |
| `--tls-key <file>` | `NYSSA_TLS_KEY` | None |
| | `NYSSA_NAME` | `Nyssa`. The name that the site shows. |
| | `NYSSA_MAIL_URL` | None. The SMTP server to send mail through. |
| | `NYSSA_MAIL_USERNAME` | None |
| | `NYSSA_MAIL_PASSWORD` | None |
| | `NYSSA_MAIL_FROM` | None. The address to send mail from. |

To turn on a setting with an environment variable, set it to `1`, `true`,
`yes` or `on`. To turn it off, set it to `0`, `false`, `no`, `off` or an
empty value.

The database is SQLite unless `--database` names a PostgreSQL, MySQL or
MariaDB database. The site works the same way on each of them. The schema
is brought up to date every time the registry starts. Put a database
password in `NYSSA_DATABASE` instead of on the command line, and the mail
password in `NYSSA_MAIL_PASSWORD`.

## Subcommands

| Subcommand | What it does |
| --- | --- |
| `migrate` | Brings the database schema up to date, and exits. |
| `check` | Checks every stored archive against its checksum. |
| `backup <file>` | Copies a SQLite database to a file while it's in use. |
| `admin create <username>` | Creates an account, for a registry with closed sign-up. |
| `admin promote <username>`, `admin demote <username>` | Makes an account an administrator, or removes that role. An administrator can yank any version and change the owners of any package. |
| `admin suspend <username>`, `admin restore <username>` | Suspends an account and revokes its tokens, or restores it. |

## Security

- Passwords are hashed with Argon2id. Tokens and recovery keys are only
  stored as digests, and every token has scopes and an expiry date.
- After five failed sign-ins, an account is locked for fifteen minutes.
  The failures are counted in the database, so every worker sees them.
  Each worker also limits an address to 30 attempts a minute to sign in,
  sign up or recover an account.
- Every form is protected against cross-site request forgery, pages are
  sent with a content security policy that allows no scripts, and HTML in
  a README is sanitized before it's shown.
- An archive is checked before it's stored: its checksum, its size, every
  path in it, and whether its `project.toml` matches the name and version
  it was published as.
- A package's guide is built only from the package's own files, never
  with a page template of its own, and served only from the guides host,
  which must be different from the registry's host. That way, scripts in a
  guide never run alongside the registry's pages or have access to its
  session.
- If the registry listens on an address other than this machine without
  a certificate, it prints a warning, because passwords and tokens would
  cross the network unencrypted. Give it a certificate, or put it behind a
  proxy that terminates TLS and pass `--trust-proxy`.

The site's own documentation, under `/docs`, covers publishing,
installing, accounts and hosting in detail.
