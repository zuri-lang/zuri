# Running Nyssa

Nyssa is the package repository, and every Zuri installation can run
one: the registry every package command talks to, and a website for
finding packages, reading their documentation, and managing an account
and its tokens.

```console
$ zuri serve
Nyssa is serving http://127.0.0.1:3000
listening on 127.0.0.1:3000, storage in /home/ada/.zuri/nyssa
```

That is a working repository, with nothing to set up first. Point the
package commands at it:

```sh
zuri account create --registry http://127.0.0.1:3000
zuri publish --registry http://127.0.0.1:3000
```

## Settings

Every setting is a flag, a `NYSSA_` environment variable, or a line in
`nyssa.toml` in the storage directory, in that order of precedence:

| Flag | Variable | Default |
| --- | --- | --- |
| `--host` | `NYSSA_HOST` | `127.0.0.1` |
| `--port` | `NYSSA_PORT` | `3000` |
| `--storage` | `NYSSA_STORAGE` | `$ZURI_HOME/nyssa` |
| `--config` | `NYSSA_CONFIG` | `nyssa.toml` in the storage directory |
| `--public-url` | `NYSSA_PUBLIC_URL` | `http://<host>:<port>` |
| `--workers` | `NYSSA_WORKERS` | one per CPU core, up to 8 |
| `--database` | `NYSSA_DATABASE` | SQLite in the storage directory |
| `--signup` | `NYSSA_SIGNUP` | `open` |
| `--max-archive-size` | `NYSSA_MAX_ARCHIVE_SIZE` | `10485760` bytes |
| `--trust-proxy` | `NYSSA_TRUST_PROXY` | off |
| `--read-only` | `NYSSA_READ_ONLY` | off |
| `--tls-cert`, `--tls-key` | `NYSSA_TLS_CERT`, `NYSSA_TLS_KEY` | none |
| | `NYSSA_NAME` | `Nyssa`, the name the site shows |
| | `NYSSA_MAIL_URL`, `NYSSA_MAIL_USERNAME`, `NYSSA_MAIL_PASSWORD`, `NYSSA_MAIL_FROM` | none |

```toml
[server]
host = "0.0.0.0"
port = 8080
public_url = "https://packages.example.com"
workers = 4
trust_proxy = true

[database]
url = "postgres://nyssa@db.internal/nyssa"

[registry]
name = "Example Packages"
signup = "closed"
max_archive_size = 20971520

[mail]
url = "smtp://mail.example.com:587"
username = "nyssa"
from = "Example Packages <packages@example.com>"
```

`public_url` is the address people reach the repository at, which links
and emails use and which package commands name it by. Passwords belong
in the environment, never in a flag, where they would show in the
process list: `NYSSA_DATABASE` for a connection string that carries
one, and `NYSSA_MAIL_PASSWORD`.

## The Database

SQLite in the storage directory needs nothing set up, and serves a
repository for a team or a company comfortably. PostgreSQL, MySQL and
MariaDB serve the same site through the same connection strings the
`sql` module opens:

```sh
NYSSA_DATABASE=postgres://nyssa:secret@db.internal/nyssa zuri serve
NYSSA_DATABASE=mysql://nyssa:secret@db.internal:3306/nyssa zuri serve
```

The schema is kept up to date by numbered migrations, applied when the
repository starts, so starting a newer Zuri against an older database
brings it up to date. `zuri serve migrate` does the same and exits, for
preparing a database ahead of a deployment. On PostgreSQL and SQLite
each migration runs in a transaction; MySQL and MariaDB commit at every
schema change, so there each migration runs on its own and is recorded
once it has finished.

Published archives are stored under the storage directory by checksum,
so the database and that directory are what a backup has to hold.

## Serving It Safely

Passwords and tokens cross the network whenever anyone signs in or
publishes, so a repository reachable beyond one machine is served over
HTTPS, one of two ways:

```sh
zuri serve --host 0.0.0.0 --tls-cert fullchain.pem --tls-key privkey.pem
zuri serve --trust-proxy
```

The second is for a repository behind a proxy that terminates TLS,
such as nginx or a load balancer, and makes it believe the client
addresses the proxy forwards. Listening beyond this machine with
neither prints a warning.

## Accounts

With `signup = "open"`, anyone may create an account. A private
repository closes sign-up and creates accounts itself:

```console
$ zuri serve admin create grace
Email: grace@example.com
Password:
Password again:
Created grace. Their recovery key, shown only this once:
```

| Command | What it does |
| --- | --- |
| `zuri serve admin create <username>` | creates an account, whether or not sign-up is open |
| `zuri serve admin promote <username>` | makes the account an administrator |
| `zuri serve admin demote <username>` | makes it an ordinary publisher again |
| `zuri serve admin suspend <username>` | suspends it and revokes every token it holds |
| `zuri serve admin restore <username>` | lifts a suspension |

An administrator may yank any version and change the owners of any
package, which is how a malicious release is stopped or an abandoned
package handed on. Publishing a new version stays with the package's
owners. Every change anyone makes is recorded in the audit log, with
who made it and from where.

With mail configured, a new account confirms its email address before
it can publish, and a lost password can be reset by email. Without
mail, the recovery key an account is given is the way back in.

## Looking After It

```sh
zuri serve check             # every stored archive against its checksum
zuri serve backup nyssa.db   # a SQLite database, while it is in use
zuri serve --read-only       # browse and install, nothing published
```

A PostgreSQL or MySQL database is backed up with its own tools, such as
`pg_dump` or `mysqldump`. Read-only mode keeps a repository serving
during maintenance or a migration elsewhere.

## Running It as a Service

On Linux, systemd keeps it running:

```ini
[Unit]
Description=Nyssa package repository
After=network.target

[Service]
User=nyssa
Environment=NYSSA_STORAGE=/var/lib/nyssa
Environment=NYSSA_PUBLIC_URL=https://packages.example.com
EnvironmentFile=/etc/nyssa/secrets.env
ExecStart=/usr/local/bin/zuri serve --host 127.0.0.1 --port 3000 --trust-proxy
Restart=on-failure

[Install]
WantedBy=multi-user.target
```

`secrets.env` holds `NYSSA_DATABASE` and `NYSSA_MAIL_PASSWORD`,
readable by the service's user alone. Stopping the service lets every
request already running finish first.

## How It Protects Itself

- Passwords are hashed with Argon2id. Tokens and recovery keys are kept
  only as digests, and every token carries scopes and an expiry.
- Five failed sign-ins lock an account for fifteen minutes, counted in
  the database so every worker sees them, and each worker allows an
  address 30 attempts a minute to sign in, sign up or recover.
- Every form carries a token only the site's own pages have, pages are
  sent with a content security policy that runs no scripts, and README
  HTML is sanitised before it is shown.
- An archive is checked before it is stored: its checksum, its size,
  every path in it, and that its `project.toml` names the package and
  version it was published as. Anything that could not be unpacked
  safely is refused.
- A name that differs from a published one only by case, hyphens or
  underscores is refused, so no package can pass for another.
- A failure answers the visitor with a plain error page, and the whole
  of it, with its stack, goes to standard error for whoever runs the
  repository.

## The API

The package commands speak version 1 of a JSON API, which any client
can use. Every failure is `{ "error": { "code", "message" } }` with a
status that describes it, and the `code` is stable.

| Request | What it does |
| --- | --- |
| `GET /api/v1/config` | what the repository is and accepts |
| `GET /api/v1/index/:name` | every version of a package, with checksums and dependencies |
| `GET /api/v1/packages/:name` | a package as its page shows it |
| `GET /api/v1/packages/:name/:version/archive` | one archive |
| `GET /api/v1/search?q=` | packages matching a query, with `sort`, `page` and `per_page` |
| `PUT /api/v1/packages` | publishes a version, as a multipart upload |
| `POST`, `DELETE /api/v1/packages/:name/:version/yank` | yanks a version, and restores it |
| `GET`, `PUT /api/v1/packages/:name/owners` | the owners, and adding one |
| `DELETE /api/v1/packages/:name/owners/:username` | removes an owner |
| `POST /api/v1/accounts` | creates an account |
| `POST`, `GET /api/v1/tokens` | signs in or issues a token, and lists tokens |
| `DELETE /api/v1/tokens/:id` | revokes a token, `current` being the one sent |
| `GET /api/v1/me` | the account behind the token |
| `GET /healthz` | answers `200` while the repository is up |

A request that changes anything sends its token as
`Authorization: Bearer nys_...`.
