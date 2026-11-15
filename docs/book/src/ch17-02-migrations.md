# Migrations

A database's schema changes along with the program that uses it, and every
copy of the database, whether it belongs to a developer, the test suite or
production, has to make the same changes in the same order. A migration is
one of those changes, stored as a file. It's applied to each database once
and recorded there, so any copy of the database can be brought up to date
from wherever it is.

`sql.migrate` applies migrations from a program, and `zuri migrate` applies
them from the command line. Both read the same files and keep the same
record.

- [Writing a Migration](#writing-a-migration)
- [Running Migrations](#running-migrations)
- [Batches and Rolling Back](#batches-and-rolling-back)
- [Checking What's Applied](#checking-whats-applied)
- [How a Migration Runs](#how-a-migration-runs)
- [When a Migration Fails](#when-a-migration-fails)
- [Several Processes at Once](#several-processes-at-once)
- [Migrating When a Program Starts](#migrating-when-a-program-starts)
- [The Migrations Directory](#the-migrations-directory)

## Writing a Migration

A migration is a Zuri file in a directory of migrations. Its name starts
with a version, which is a sequence of digits, followed by a description
made of letters, digits and underscores. `zuri migrate new` creates one,
using the current time as its version:

```sh
zuri migrate new create_posts
```

```text
Created migrations/20261011143000_create_posts.zu
```

Each name you give creates a separate migration, in the order you give them,
so `zuri migrate new create_posts add_slug_to_posts` creates two.

A migration exports `up(db)`, which makes the change, and `down(db)`, which
undoes it. `db` is the `Transaction` the migration runs in, or the
`Connection` itself when it runs outside a transaction, which always happens
on MySQL and MariaDB, and can happen anywhere for a migration that asks for
it. Its type is `Transaction|Connection`, and you describe the change with
`db.schema` and its blueprint, once, for every engine the program runs
against:

```zuri,ignore
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

With the types written out, your editor can complete `db.` with the methods
that transactions and connections share, and `table.` with the blueprint's
methods. `zuri migrate new` includes the types for you.

If the name has the form `create_<table>`, the new file starts out creating
that table, as above. Any other name starts with empty `up()` and `down()`
functions. A migration can also change data, using the same methods a
connection has:

```zuri,ignore
import sql { Blueprint, Connection, Transaction }

def up(db: Transaction|Connection) {
  db.schema.alter_table('posts', @(table: Blueprint) {
    table.string('slug', 220).nullable().unique()
  })

  for post in db.fetch_all('select id, title from posts', nil) {
    db.update('posts', { slug: slugify(post.title) }, { id: post.id })
  }
}
```

## Running Migrations

`zuri migrate` finds the project by looking for the nearest directory above
the working directory that contains a `project.toml`, and uses its
`migrations` directory. It gets the database from `--database`, or from
`DATABASE_URL` in the environment or the project's `.env` file:

```sh
zuri migrate                  # apply every pending migration
zuri migrate status           # what has run, and what has not
zuri migrate down             # roll back the latest batch
zuri migrate reset            # roll back everything
```

```text
Applied 2 migrations:
  20261011143000_create_posts
  20261012091500_add_slug_to_posts
```

In a program, `Migrator` does the same thing:

```zuri
import os
import sql
import sql.migrate { Migrator, create }

var directory = os.join_paths(os.create_temp_dir('guide-'), 'migrations')

create(directory, 'create_posts')

var db = sql.open(':memory:')
var migrator = Migrator(db, { directory })

echo migrator.pending().length()
echo migrator.up().length()
echo db.schema.column_names('posts')

db.close()
```

```console
1
1
[id, created_at, updated_at]
```

`Migrator` accepts a `directory`, which is `migrations` by default, and a
`table` to keep the record in, which is `zuri_migrations` by default. The
table is created the first time it's needed.

## Batches and Rolling Back

All the migrations applied in a single run share a batch number. Rolling
back without any options undoes the whole of the latest batch, however many
migrations that is, starting with the newest:

```zuri
import os
import sql
import sql.migrate { Migrator }

var directory = os.create_temp_dir('guide-')

def write(name, table) {
  file(os.join_paths(directory, name), 'w').write(
    'import sql { Blueprint, Connection, Transaction }\n\n' +
        'def up(db: Transaction|Connection) {\n' +
        "  db.schema.create_table('${table}', @(table: Blueprint) {\n" +
        '    table.id()\n  })\n}\n\n' +
        'def down(db: Transaction|Connection) {\n' +
        "  db.schema.drop_table('${table}')\n}\n",
  )
}

write('1_create_authors.zu', 'authors')
write('2_create_posts.zu', 'posts')

var db = sql.open(':memory:')
var migrator = Migrator(db, { directory })

migrator.up({ steps: 1 })
migrator.up()

echo migrator.applied().map(@(row) => row.batch)
echo migrator.down()
echo db.schema.tables()

db.close()
```

```console
[1, 2]
[2_create_posts]
[authors, zuri_migrations]
```

Both directions accept a limit. `steps` counts migrations, and `to` names a
specific migration:

```zuri,ignore
migrator.up({ steps: 1 }) # the next one
migrator.up({ to: '20261012091500_add_slug' }) # up to and including it
migrator.down({ steps: 2 }) # the two most recent
migrator.down({ to: '20261011143000_create' }) # leave it the latest
```

Before the first migration is rolled back, every migration that's about to
be rolled back is checked. Each one must still have its file, with a
`down()` function, or nothing is rolled back at all.

## Checking What's Applied

`status()` lists every migration, from both the files and the record, along
with its state:

| State | Meaning |
| --- | --- |
| `applied` | Recorded, and the file is unchanged since it ran |
| `changed` | Recorded, but the file has changed since it ran |
| `missing` | Recorded, but the file no longer exists |
| `pending` | A file that hasn't been applied to this database yet |

A changed migration has already run in its earlier form, and editing it
doesn't change anything in a database where it has run. That database has
whatever the old file did. To make a further change, write a new migration.
`zuri migrate status` reminds you of this when it finds a changed migration.

## How a Migration Runs

Each migration runs in its own transaction, together with the row that
records it. On SQLite and PostgreSQL, a migration that fails partway through
leaves nothing behind and isn't recorded. MySQL and MariaDB commit every
statement that changes a table's definition, so on those engines, a
migration runs outside a transaction and is recorded once it has finished,
and a migration that fails partway through leaves the statements before the
failure in place.

On SQLite, foreign key enforcement is turned off while a migration runs, so
that a change that rebuilds a table can't delete the rows that refer to it.
Every foreign key is checked before the migration commits.

A migration that has to run outside a transaction, as PostgreSQL's
`create index concurrently` does, can say so:

```zuri,ignore
import sql { Connection, Transaction }

var transaction = false

def up(db: Transaction|Connection) {
  db.exec('create index concurrently posts_title on posts (title)', nil)
}
```

## When a Migration Fails

A failure stops the run at that point. The migrations before it stay
applied, and the failed one and those after it stay pending, ready for the
next run once you've fixed the problem:

```zuri
import os
import sql
import sql.migrate { Migrator }

var directory = os.create_temp_dir('guide-')

var head = 'import sql { Blueprint, Connection, Transaction }\n\n' +
    'def up(db: Transaction|Connection) {\n'

file(os.join_paths(directory, '1_create_posts.zu'), 'w').write(
  head +
      "  db.schema.create_table('posts', @(table: Blueprint) {\n" +
      '    table.id()\n  })\n}\n',
)

file(os.join_paths(directory, '2_broken.zu'), 'w').write(
  head +
      "  db.schema.create_table('half', @(table: Blueprint) {\n" +
      '    table.id()\n  })\n' +
      "  db.exec('this is not sql', nil)\n}\n",
)

var db = sql.open(':memory:')
var migrator = Migrator(db, { directory })

catch {
  migrator.up()
} as error {
  echo typeof(error)
}

echo migrator.status().map(@(entry) => entry.state)
echo db.schema.has_table('half')

db.close()
```

```console
QueryError
[applied, pending]
false
```

## Several Processes at Once

If two processes migrate the same database at the same time, as happens when
two copies of a server start together, each migration is still applied only
once. On PostgreSQL, MySQL and MariaDB, a run holds a lock provided by the
engine, and a second run waits for the first one to finish before it looks
for pending migrations. On SQLite, each migration takes the write lock when
it starts. Either way, each migration checks the record again once it holds
the lock, so a migration that another process applied in the meantime is
skipped.

## Migrating When a Program Starts

A program that manages its own database can bring it up to date when it
starts. Nyssa, the package registry that `zuri serve` runs, does exactly
this:

```zuri,ignore
import env
import os
import sql
import sql.migrate { Migrator }

var db = sql.open(env.require('DATABASE_URL'))

Migrator(
  db,
  { directory: os.join_paths(os.dir_name(__file__), 'migrations') },
).up()
```

Because the migrations directory is found relative to `__file__`, it moves
with the program wherever it's installed.

## The Migrations Directory

Every `.zu` file whose name starts with a version followed by an underscore
is a migration. Migrations run in order of version, compared as numbers, and
then by name, so `9` comes before `10`. Two files with the same version are
rejected. A `.zu` file whose name starts with `_` isn't a migration. It
holds code shared by several migrations, which they import with
`import ._tables`. Files that aren't Zuri files are ignored.
