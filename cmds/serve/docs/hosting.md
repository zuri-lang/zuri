# Hosting a repository

This site is Nyssa, and every Zuri installation can run one:

```sh
zuri serve
```

It listens on `127.0.0.1:3000` and keeps everything under `.zuri/nyssa`
in your home directory: the database, the published archives, and the
secret the session cookies are signed with.

## Settings

Every setting is a flag, an environment variable, or a line in
`nyssa.toml` in the storage directory, in that order of precedence.
Each variable is `NYSSA_` and the setting's name in capitals:
`NYSSA_STORAGE`, `NYSSA_PORT`, `NYSSA_PUBLIC_URL`, `NYSSA_DATABASE`,
and so on for every setting below. `NYSSA_STORAGE` moves everything,
and `NYSSA_CONFIG` names a configuration file elsewhere.

```toml
[server]
host = "0.0.0.0"
port = 8080
workers = 4
public_url = "https://packages.example.com"
trust_proxy = true

[registry]
name = "Example Packages"
signup = "closed"
max_archive_size = 20971520
```

`public_url` is the address people reach the repository at, and what
links and emails use. Set `signup = "closed"` for a private repository
and create accounts yourself:

```sh
zuri serve admin create ada
zuri serve admin promote ada
```

`workers` is how many requests are served at once. It is one per CPU
core, up to 8, unless set.

## The database

The database is SQLite in the storage directory unless `database`
names another. PostgreSQL, MySQL and MariaDB serve the same site:

```toml
[database]
url = "postgres://nyssa@localhost/nyssa"
```

A connection string that carries a password belongs in
`NYSSA_DATABASE` rather than the file. `zuri serve migrate` brings the
database up to date and exits, which is how to prepare one before the
first start; `zuri serve` does the same on its own when it starts.

## Serving it safely

Passwords and tokens cross the network whenever anyone signs in or
publishes, so serve HTTPS: give it a certificate with `--tls-cert` and
`--tls-key`, or put it behind a proxy that terminates TLS and pass
`--trust-proxy` so it believes the addresses the proxy forwards.
Listening on a public address without either prints a warning.

## Mail

With mail set up, new accounts confirm their address before
publishing, and a lost password can be reset by email:

```toml
[mail]
url = "smtp://mail.example.com"
username = "packages"
from = "Example Packages <packages@example.com>"
```

The password goes in `NYSSA_MAIL_PASSWORD`, never a flag.

## Looking after it

```sh
zuri serve backup /backups/nyssa.db
zuri serve check
zuri serve migrate
```

`backup` copies the database while the repository runs. `check`
confirms every archive still matches the checksum it was published
with. `migrate` brings the database up to date, which starting the
repository also does.
