# Running Nyssa

Nyssa is Zuri's package registry, and any Zuri installation can run one. It
provides the registry API that every package command talks to, and a website
where you can find packages, read their READMEs and the API references
generated from their doc comments, read the standard library reference, and
manage your account and its tokens.

```console
$ zuri serve
Nyssa is serving http://127.0.0.1:3000
listening on 127.0.0.1:3000, storage in /home/ada/.zuri/nyssa
```

That's a working registry, without any setup. Point the package commands at
it:

```sh
zuri account create --registry http://127.0.0.1:3000
zuri publish --registry http://127.0.0.1:3000
```

## Settings

Every setting can be given as a flag, as a `NYSSA_` environment variable, or
as a line in `nyssa.toml` in the storage directory, in that order of
priority:

| Flag | Variable | Default |
| --- | --- | --- |
| `--host` | `NYSSA_HOST` | `127.0.0.1` |
| `--port` | `NYSSA_PORT` | `3000` |
| `--storage` | `NYSSA_STORAGE` | `$ZURI_HOME/nyssa` |
| `--config` | `NYSSA_CONFIG` | `nyssa.toml` in the storage directory |
| `--public-url` | `NYSSA_PUBLIC_URL` | `http://<host>:<port>` |
| `--workers` | `NYSSA_WORKERS` | One per CPU core, up to 8 |
| `--database` | `NYSSA_DATABASE` | SQLite in the storage directory |
| `--signup` | `NYSSA_SIGNUP` | `open` |
| `--max-archive-size` | `NYSSA_MAX_ARCHIVE_SIZE` | `10485760` bytes |
| `--trust-proxy` | `NYSSA_TRUST_PROXY` | Off |
| `--read-only` | `NYSSA_READ_ONLY` | Off |
| `--tls-cert`, `--tls-key` | `NYSSA_TLS_CERT`, `NYSSA_TLS_KEY` | None |
| `--guides-url` | `NYSSA_GUIDES_URL` | None, in which case packages' guides aren't built or served |
| | `NYSSA_NAME` | `Nyssa`, the name the site shows |
| | `NYSSA_MAIL_URL`, `NYSSA_MAIL_USERNAME`, `NYSSA_MAIL_PASSWORD`, `NYSSA_MAIL_FROM` | None |

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

`public_url` is the address people use to reach the registry. It's used in
links and emails, and package commands refer to the registry by it.
Passwords belong in the environment, never in a flag, where they'd be
visible in the process list. That means `NYSSA_DATABASE` for a connection
string that contains a password, and `NYSSA_MAIL_PASSWORD`.

## The Database

SQLite in the storage directory doesn't need any setup, and it can easily
serve a registry for a team or a company. PostgreSQL, MySQL and MariaDB can
serve the same site, using the same connection strings the `sql` module
accepts:

```sh
NYSSA_DATABASE=postgres://nyssa:secret@db.internal/nyssa zuri serve
NYSSA_DATABASE=mysql://nyssa:secret@db.internal:3306/nyssa zuri serve
```

The schema is kept up to date by [migrations](ch17-02-migrations.md), each
written once for every database engine and applied when the registry starts,
so starting a newer version of Zuri against an older database updates it.
`zuri serve migrate` does the same and then exits, so you can prepare a
database before a deployment. On PostgreSQL and SQLite, each migration runs
in a transaction. MySQL and MariaDB commit on every schema change, so on
those databases each migration runs outside a transaction and is recorded
once it has finished. If several registries start at the same time against
the same database, each migration is still only applied once.

Published archives are stored by checksum under the storage directory, so a
backup needs to include the database and that directory.

## Serving It Securely

Passwords and tokens are sent over the network whenever someone signs in or
publishes, so a registry that can be reached from other machines must be
served over HTTPS, in one of two ways:

```sh
zuri serve --host 0.0.0.0 --tls-cert fullchain.pem --tls-key privkey.pem
zuri serve --trust-proxy
```

The second way is for a registry behind a proxy that terminates TLS, such as
nginx or a load balancer, and tells the registry to trust the client
addresses that the proxy forwards. If the registry listens beyond the local
machine without either, it prints a warning.

## Packages' Guides

A package can include a guide, which is documentation built by `zuri doc`,
in its `docs` directory ([Chapter 30](ch30-00-documentation.md)). If you
give the registry a host to serve guides on, it builds each version's guide
when the version is published, and links to it from the version's page:

```sh
zuri serve --guides-url https://guides.packages.example.com
```

A guide is the package's own HTML and scripts, so it's only served on the
guides host, never on the registry's own host. There, its scripts run
without access to anything that belongs to the registry, such as a session
or a signed-in page. That's why the guides host must be different from the
registry's host, even if the port is different, because a cookie applies to
every port on its host. Point the guides host at the registry in the same
way as the registry's own address, through the same proxy if there is one,
and the registry responds to each request according to the host name it was
sent to. A guide is available at `/<package>/<version>/` on that host, and
`/<package>/` shows the guide for the newest version.

A guide is built using only the package's own files. Everything its
`doc.toml` names and its pages include has to be inside the package, or the
guide isn't built, and the package's own page template is never used,
because a template runs code. Guides for versions published before guides
were turned on are built in the background the next time the registry
starts.

## Accounts

With `signup = "open"`, anyone can create an account. A private registry
turns off sign-up and creates accounts itself:

```console
$ zuri serve admin create grace
Email: grace@example.com
Password:
Password again:
Created grace. Their recovery key, shown only this once:
```

| Command | What it does |
| --- | --- |
| `zuri serve admin create <username>` | Creates an account, whether or not sign-up is open |
| `zuri serve admin promote <username>` | Makes the account an administrator |
| `zuri serve admin demote <username>` | Makes it an ordinary account again |
| `zuri serve admin suspend <username>` | Suspends the account and revokes all of its tokens |
| `zuri serve admin restore <username>` | Lifts a suspension |

An administrator can yank any version and change the owners of any package,
which is how a malicious release is stopped or an abandoned package is
handed over to someone else. Only a package's owners can publish new
versions. Every change is recorded in the audit log, along with who made it
and where it came from.

If mail is configured, a new account has to confirm its email address before
it can publish, and a lost password can be reset by email. Without mail, the
account's recovery key is the only way back in.

## Maintenance

```sh
zuri serve check             # every stored archive against its checksum
zuri serve backup nyssa.db   # a SQLite database, while it is in use
zuri serve --read-only       # browse and install, nothing published
```

Back up a PostgreSQL or MySQL database with its own tools, such as `pg_dump`
or `mysqldump`. Read-only mode keeps a registry running during maintenance,
or while a migration happens somewhere else.

## Running It as a Service

On Linux, systemd can keep it running:

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

`secrets.env` contains `NYSSA_DATABASE` and `NYSSA_MAIL_PASSWORD`, and only
the service's user can read it. When the service is stopped, every request
that's already running is allowed to finish first.

## Security Measures

- Passwords are hashed with Argon2id. Tokens and recovery keys are only
  stored as digests, and every token has scopes and an expiry date.
- After five failed sign-ins, an account is locked for fifteen minutes. The
  failures are counted in the database, so every worker sees them. Each
  worker also allows an address 30 attempts a minute to sign in, sign up or
  recover an account.
- Every form includes a token that only the site's own pages have, pages are
  sent with a content security policy that doesn't allow scripts to run, and
  HTML in READMEs is sanitized before it's displayed.
- Every archive is checked before it's stored, including its checksum, its
  size, every path in it, and whether its `project.toml` names the package
  and version it was published as. Anything that couldn't be unpacked safely
  is rejected.
- A name that only differs from a published name in case, hyphens or
  underscores is rejected, so that no package can pretend to be another.
- When something fails, the visitor sees a plain error page, and the full
  error, including the stack trace, goes to standard error for whoever runs
  the registry.

## The API

The package commands use version 1 of a JSON API, which any client can use.
Every error response has the form `{ "error": { "code", "message" } }`, with
a status code that describes the error, and the `code` values don't change.

| Request | What it does |
| --- | --- |
| `GET /api/v1/config` | Describes the registry and what it accepts |
| `GET /api/v1/index/:name` | Lists every version of a package, with checksums and dependencies |
| `GET /api/v1/packages/:name` | Returns a package as its page shows it |
| `GET /api/v1/packages/:name/:version/archive` | Downloads an archive |
| `GET /api/v1/search?q=` | Lists packages that match a query, with `sort`, `page` and `per_page` |
| `PUT /api/v1/packages` | Publishes a version, as a multipart upload |
| `POST`, `DELETE /api/v1/packages/:name/:version/yank` | Yanks a version, or restores it |
| `GET`, `PUT /api/v1/packages/:name/owners` | Lists the owners, or adds one |
| `DELETE /api/v1/packages/:name/owners/:username` | Removes an owner |
| `POST /api/v1/accounts` | Creates an account |
| `POST`, `GET /api/v1/tokens` | Signs in or issues a token, or lists tokens |
| `DELETE /api/v1/tokens/:id` | Revokes a token. Use `current` for the token sent with the request. |
| `GET /api/v1/me` | Returns the account that the token belongs to |
| `GET /healthz` | Responds with `200` while the registry is running |

A request that changes anything sends its token in the header
`Authorization: Bearer nys_...`.
