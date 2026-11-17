# migrate

Applies, rolls back and lists database migrations.

```sh
zuri migrate [up | status | down | reset | new <name>...] [options]
```

A migration is a Zuri file in the project's `migrations` directory, named
with a version and a description of the change, such as
`20261011143000_create_posts.zu`. It exports two functions: `up(db)`,
which makes the change, and `down(db)`, which undoes it. Both are written
with `db.schema`, so the same file works on SQLite, PostgreSQL, MySQL and
MariaDB. `db` is the migration's `Transaction`, or a `Connection` when the
migration runs outside a transaction, as it always does on MySQL and
MariaDB:

```zuri
import sql { Blueprint, Connection, Transaction }

def up(db: Transaction|Connection) {
  db.schema.create_table('posts', @(table: Blueprint) {
    table.id()
    table.string('title', 200)
    table.timestamps()
  })
}

def down(db: Transaction|Connection) {
  db.schema.drop_table('posts')
}
```

Each migration runs once, in order, and is recorded in the
`zuri_migrations` table. On engines that can roll back a schema change,
each migration runs in a transaction of its own. If two processes migrate
the same database at the same time, each migration is still applied only
once. `sql.migrate` provides the same thing as a library, for a program
that migrates its own database when it starts.

The project is the nearest directory, starting from the working directory,
that contains a `project.toml`. If there isn't one, the working directory
is used. The database is taken from `--database`, or from `DATABASE_URL`
in the environment or in the project's `.env` file.

| Command | What it does |
| --- | --- |
| `zuri migrate` | Applies every pending migration. |
| `zuri migrate up` | The same. `--steps` and `--to` stop it sooner. |
| `zuri migrate status` | Lists every migration and whether it has been applied. |
| `zuri migrate down` | Rolls back the latest batch. `--steps` and `--to` roll back a different number. |
| `zuri migrate reset` | Rolls back every migration. |
| `zuri migrate new <name>...` | Creates a new, empty migration for each name. |

A batch is the set of migrations that one `up` applied. `status` marks a
migration whose file changed after it was applied, and one that was
applied but whose file no longer exists.

A migration name can contain letters, digits and underscores. A name of
the form `create_<table>`, such as `create_posts`, creates a migration that
creates and drops that table. Any other name creates a migration with
empty `up` and `down` functions.
Either way, the functions start with the type annotations shown above.
`new create_posts add_slug_to_posts` creates two migrations, in that order.

## Options

| Option | Description |
| --- | --- |
| `-d, --database <url>` | The database connection string. |
| `--dir <path>` | The directory containing the migrations. Defaults to `migrations` in the project. |
| `--table <name>` | The table that records applied migrations. Defaults to `zuri_migrations`. |
| `-s, --steps <count>` | With `up` or `down`, the maximum number of migrations to apply or roll back. |
| `--to <name>` | With `up`, the last migration to apply. With `down`, the migration to roll back to. |
