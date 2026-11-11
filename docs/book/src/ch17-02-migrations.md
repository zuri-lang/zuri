# Migrations

A database's schema changes as the program around it does, and every
copy of the database, a developer's, the test suite's, production's,
has to make the same changes in the same order. A migration is one such
change, kept as a file, applied to each database once and recorded
there, so any copy can be brought up to date from wherever it is.

`sql.migrate` applies them from a program, and `zuri migrate` from the
command line. Both read the same files and keep the same record.

- [Writing a Migration](#writing-a-migration)
- [Running Them](#running-them)
- [Batches and Rolling Back](#batches-and-rolling-back)
- [What Is Applied](#what-is-applied)
- [How a Migration Runs](#how-a-migration-runs)
- [When One Fails](#when-one-fails)
- [Several Processes at Once](#several-processes-at-once)
- [Migrating as a Program Starts](#migrating-as-a-program-starts)
- [The Directory](#the-directory)

## Writing a Migration

A migration is a Zuri file in a directory of them. Its name starts with
a version, a run of digits, and says what it does in letters, digits
and underscores. `zuri migrate new` writes one, with the time as its
version:

```sh
zuri migrate new create_posts
```

```text
Created migrations/20261011143000_create_posts.zu
```

Each name given writes a migration of its own, in the order given, so
`zuri migrate new create_posts add_slug_to_posts` writes two.

It exports `up(db)`, which makes the change, and `down(db)`, which
undoes it. `db` is the `Transaction` the migration runs in, or the
`Connection` itself where it runs outside one: always on MySQL and
MariaDB, and anywhere for a migration that asks to. Its type is
`Transaction|Connection`, and the change is described with `db.schema`
and its blueprint, once, for every engine the program runs against:

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

With the types written, an editor completes `db.` with what a
transaction and a connection both have, and `table.` with the
blueprint's methods. `zuri migrate new` writes them in.

A name of the form `create_<table>` starts the file creating that
table, as above; any other name starts it with empty `up()` and
`down()`. A migration can also move data, with the same calls a
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

## Running Them

`zuri migrate` finds the project, the nearest directory above the
working directory with a `project.toml`, and its `migrations`
directory, and the database from `--database`, or from `DATABASE_URL`
in the environment or in the project's `.env`:

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

From a program, `Migrator` does the same:

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

`Migrator` takes `directory`, `migrations` by default, and `table`, the
table the record is kept in, `zuri_migrations` by default. The table is
created the first time it is needed.

## Batches and Rolling Back

Every migration one run applies shares a batch number. Rolling back
with no options undoes the latest batch whole, however many migrations
that was, newest first:

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

Both directions take a limit. `steps` counts migrations; `to` names
one:

```zuri,ignore
migrator.up({ steps: 1 }) # the next one
migrator.up({ to: '20261012091500_add_slug' }) # up to and including it
migrator.down({ steps: 2 }) # the two most recent
migrator.down({ to: '20261011143000_create' }) # leave it the latest
```

Every migration about to be rolled back is checked before the first
one is: each has to have its file, with a `down()`, or nothing is
rolled back at all.

## What Is Applied

`status()` lists every migration, from the files and from the record,
with its state:

| State | Meaning |
| --- | --- |
| `applied` | recorded, and the file is as it was when it ran |
| `changed` | recorded, and the file has changed since it ran |
| `missing` | recorded, and there is no file for it any more |
| `pending` | a file the database has not had |

A changed migration has already run as it was then, and editing it
changes nothing in a database that ran it. The database has what the
old file said; a further change goes in a new migration. `zuri migrate
status` says so when it finds one.

## How a Migration Runs

Each migration runs in a transaction of its own, together with the row
recording it. On SQLite and PostgreSQL, a migration that fails part
way leaves nothing behind and is not recorded. MySQL and MariaDB commit
at every statement that changes a table's definition: there a migration
runs on its own and is recorded once it has finished, and one that
fails part way leaves the statements before the failure in place.

On SQLite, foreign key enforcement is switched off while a migration
runs, so a change that builds a table again cannot take the rows
referring to it along, and every foreign key is checked before the
migration commits.

A migration that has to run outside a transaction, as PostgreSQL's
`create index concurrently` does, says so:

```zuri,ignore
import sql { Connection, Transaction }

var transaction = false

def up(db: Transaction|Connection) {
  db.exec('create index concurrently posts_title on posts (title)', nil)
}
```

## When One Fails

A failure stops the run where it is. The migrations before it stay
applied, and it and those after it stay pending, ready for the next
run once it is fixed:

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

Two processes migrating one database at the same moment, as two copies
of a server starting together do, apply each migration once. On
PostgreSQL, MySQL and MariaDB a run holds the engine's own lock, and a
second run waits for the first to finish before it looks for pending
migrations. On SQLite each migration takes the write lock as it starts.
Either way, every migration checks the record again once it holds the
lock, so one another process applied meanwhile is passed over.

## Migrating as a Program Starts

A program that keeps its own database brings it up to date as it
starts. Nyssa, the package repository `zuri serve` runs, does exactly
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

A migration directory found from `__file__` travels with the program
wherever it is installed.

## The Directory

Every `.zu` file whose name starts with a version and an underscore is
a migration, and they run by version, as numbers, then by name: `9`
before `10`, and a version two files share is refused. A `.zu` file
whose name starts with `_` is not a migration; it holds code several
migrations share, which they import as `import ._tables`. Files that are
not Zuri are left alone.
