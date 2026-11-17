# Hosting a registry

This site runs Nyssa, a package registry that comes with every Zuri
installation. To run one yourself:

```sh
zuri serve
```

By default, it listens on `127.0.0.1:3000`, and keeps all its data in
`.zuri/nyssa` in your home directory. That includes the database, the
published archives, and the secret that session cookies are signed with.

## Settings

Each setting can be given as an option, as an environment variable, or
in `nyssa.toml` in the storage directory. An option takes priority over
an environment variable, and an environment variable takes priority
over the file. Each variable's name is `NYSSA_` followed by the setting's
name in capitals, such as `NYSSA_STORAGE`, `NYSSA_PORT`,
`NYSSA_PUBLIC_URL` and `NYSSA_DATABASE`. `NYSSA_STORAGE` moves all of the
data, and `NYSSA_CONFIG` points to a configuration file somewhere else.

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

`public_url` is the address that people use to reach the registry. It's
also used in links and emails. For a private registry, set
`signup = "closed"`, and create the accounts yourself:

```sh
zuri serve admin create ada
zuri serve admin promote ada
```

`workers` is the number of connections served at the same time. By
default, it's one per CPU core, up to 8.

## The database

By default, the database is SQLite, stored in the storage directory. To
use PostgreSQL, MySQL or MariaDB instead, set `database`. The site works
the same way on each of them:

```toml
[database]
url = "postgres://nyssa@localhost/nyssa"
```

If the connection string contains a password, put it in
`NYSSA_DATABASE` instead of the file. `zuri serve migrate` brings the
database schema up to date and exits, so you can prepare a database
before the first start. `zuri serve` also does this itself every time it
starts.

## Package guides

A package can include a guide, which is documentation built with
`zuri doc`, in its `docs` directory. If you give the registry a host to
serve guides from, it builds each version's guide when the version is
published:

```toml
[guides]
url = "https://guides.packages.example.com"
```

The guides host must be different from the registry's host, even if
only the port would differ. A guide contains its package's own HTML and
scripts, and a cookie is shared by every port of a host. Point the guides
host at the registry in the same way as the registry's own address. The
registry decides what to serve from the host name in each request.

## Serving it securely

Passwords and tokens are sent over the network whenever someone signs in
or publishes a package, so serve the registry over HTTPS. Either give it
a certificate with `--tls-cert` and `--tls-key`, or put it behind a proxy
that terminates TLS and pass `--trust-proxy`, so that it trusts the
client addresses the proxy forwards. If it listens on a public address
without either of these, it prints a warning.

## Mail

When mail is set up, new users have to confirm their email address before
they can publish, and a user who forgets their password can reset it by
email:

```toml
[mail]
url = "smtp://mail.example.com"
username = "packages"
from = "Example Packages <packages@example.com>"
```

Put the mail password in `NYSSA_MAIL_PASSWORD`. There's no option for it.

## Maintenance

```sh
zuri serve backup /backups/nyssa.db
zuri serve check
zuri serve migrate
```

`backup` copies a SQLite database while the registry is running. For
PostgreSQL, MySQL or MariaDB, use the database's own tools, such as
`pg_dump` or `mysqldump`. `check` confirms that every archive still
matches the checksum it was published with. `migrate` brings the database
schema up to date, which the registry also does when it starts.
