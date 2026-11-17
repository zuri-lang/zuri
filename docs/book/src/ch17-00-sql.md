# Databases

The `sql` module is how a Zuri program works with a relational database.
There's a single module rather than one for each database engine, and that's
the core of its design. `sql` defines what a database adapter has to
provide, supplies everything that works the same way regardless of the
engine, and chooses the adapter based on the connection string.

It comes with three adapters that support four major database engines:
SQLite, PostgreSQL, MySQL and MariaDB. SQLite is a file-based database with
no server to run and nothing to configure, which makes it a good fit for
applications that ship with their own data and for test suites that need a
real database for each run. PostgreSQL, MySQL and MariaDB are database
servers for anything that outgrows a single file. The MySQL adapter works
with MariaDB as well.

To switch from one database to another, you change the connection string,
plus any SQL that the two engines really do write differently.

- [Following Along](#following-along)
- [Introduction](#introduction)
- [Connecting to a Database](#connecting-to-a-database)
- [Running Queries](#running-queries)
- [Parameters](#parameters)
- [Rows, Columns and Types](#rows-columns-and-types)
- [CRUD Helpers](#crud-helpers)
- [Transactions](#transactions)
- [Streaming Large Results](#streaming-large-results)
- [Prepared Statements](#prepared-statements)
- [Connection Pools](#connection-pools)
- [Errors](#errors)
- [Schema Introspection](#schema-introspection)
- [Switching Databases](#switching-databases)
- [SQLite Features](#sqlite-features)
- [PostgreSQL Features](#postgresql-features)
- [MySQL Features](#mysql-features)
- [Writing Your Own Adapter](#writing-your-own-adapter)
- [Design Limits](#design-limits)
- [Module Reference](#module-reference)

> Code blocks in this chapter that list several calls together are
> **reference listings**, not programs. They show the form of each call, not
> a sequence to run. Anything shown as a complete program runs as written.
> The PostgreSQL and MySQL examples need a database server, so they're shown
> but not run.

## Following Along

Most of the examples below use a small database of posts and their authors.
This creates it:

```zuri
import sql

var db = sql.open('sqlite://guide.db')

db.exec_script(
  "
  drop table if exists posts;
  drop table if exists authors;

  create table authors (
    id integer primary key,
    name text not null unique
  );

  create table posts (
    id integer primary key,
    author_id integer not null references authors(id),
    title text not null,
    views integer not null default 0,
    published boolean not null default 0
  );
",
)

var ada = db.insert('authors', { name: 'Ada Lovelace' })
var grace = db.insert('authors', { name: 'Grace Hopper' })

db.insert_many('posts', [
  { author_id: ada, title: 'On Engines', views: 412, published: true },
  { author_id: ada, title: 'On Looms', views: 87, published: false },
  { author_id: grace, title: 'On Bugs', views: 1290, published: true },
  { author_id: grace, title: 'On Compilers', views: 640, published: true },
])

echo db.count('posts')
db.close()
```

```console
4
```

From here on, the examples assume that this file exists in the working
directory.

## Introduction

A database library usually ties a program to a single engine. Its functions
are named after that engine, its placeholders follow that engine's syntax,
and its errors use that engine's error numbers, so switching to another
database means rewriting every call.

`sql` keeps the parts that are the same for every engine in one place, so
you write a statement once:

```zuri
import sql

var db = sql.open('sqlite://guide.db')

for post in db.query('select title, views from posts where published = ?', [
  true,
]) {
  echo '${post.title}: ${post.views}'
}

db.close()
```

```console
On Engines: 412
On Bugs: 1290
On Compilers: 640
```

If you change the first line to `postgres://localhost/app` or
`mysql://localhost/app`, the rest runs unchanged. On PostgreSQL, the `?`
becomes `$1`, because that's what PostgreSQL expects, and on MySQL it stays
`?`, because that's what MySQL expects. An insert that asks for its new ID
gets a `RETURNING` clause on PostgreSQL, because PostgreSQL has no "last
insert ID" and the other two do. A duplicate key is reported as a
`UniqueViolation` by all three.

What `sql` doesn't do is pretend that the engines are the same. They differ
on auto-incrementing keys, on which functions are available and on a lot of
other SQL. `sql` translates what it can and is explicit about the rest.

## Connecting to a Database

`open()` takes a connection string and returns a `Connection`.

```zuri,ignore
sql.open('sqlite://./app.db') # a file
sql.open(':memory:') # a private database in memory
sql.open('./app.db') # a bare path is SQLite
sql.open('postgres://localhost/app')
sql.open('postgres://alice:secret@db.internal:5432/shop?sslmode=require')
sql.open('host=localhost dbname=app user=alice')
sql.open('mysql://localhost/app')
sql.open('mysql://alice:secret@db.internal:3306/shop?sslmode=verify')
sql.open('mariadb://localhost/app')
```

The scheme selects the adapter. A string without a scheme is treated as a
SQLite file path, since it can't be anything else. MySQL and MariaDB use the
same protocol, so either scheme can connect to either server, and the
connection is described by the adapter for whichever server it found.

You can also pass options as a dictionary, which is how you set anything an
adapter supports beyond what fits in the connection string:

```zuri,ignore
sql.open({
  driver: 'sqlite',
  path: './app.db',
  journal_mode: 'wal',
  busy_timeout: 10000,
})
```

Or you can pass them alongside a connection string, in which case they're
merged with, and override, the values in the string:

```zuri,ignore
sql.open('sqlite://./app.db', { journal_mode: 'wal' })
```

Close a connection when you're done with it. A `catch` block makes sure that
happens, even if the code in between raises an error:

```zuri
import sql

var db = sql.open(':memory:')

catch {
  db.exec('create table t (n integer)')
  db.exec('insert into t values (?)', [1])

  echo db.fetch_value('select n from t')
} as error {
  echo 'failed: ${error.message}'
}

db.close()
```

```console
1
```

A connection belongs to the isolate that opened it and can't be passed to
another isolate. An isolate that needs to use the database opens its own
connection or its own pool.

## Running Queries

`query()` runs a statement and reads its entire result:

```zuri
import sql

var db = sql.open('sqlite://guide.db')
var result = db.query('select title, views from posts order by views desc')

echo result.length()
echo result.first().title
echo result.column('title')

db.close()
```

```console
4
On Bugs
[On Bugs, On Compilers, On Engines, On Looms]
```

A result is iterable, so in the most common case, that's all you need:

```zuri,ignore
for post in db.query('select * from posts') {
  echo post.title
}
```

There are shorter forms for the most common kinds of result:

```zuri,ignore
db.fetch_one(sql, params) # the first row, or nil
db.fetch_all(sql, params) # every row, as dictionaries
db.fetch_value(sql, params, fallback) # the first column of the first row
db.fetch_column(sql, params, column) # one column's values, as a list
```

```zuri
import sql

var db = sql.open('sqlite://guide.db')

echo db.fetch_value('select count(*) from posts')
echo db.fetch_one('select title from posts where views > ?', [1000]).title
echo db.fetch_column('select title from posts order by title limit 2')
echo db.fetch_value('select title from posts where views > ?', [99999], 'none')

db.close()
```

```console
4
On Bugs
[On Bugs, On Compilers]
none
```

`exec()` is for statements you run for their effect rather than for rows:

```zuri
import sql

var db = sql.open(':memory:')

db.exec('create table t (id integer primary key, n integer)')
var result = db.exec('insert into t (n) values (?)', [7])

echo result.rows_affected
echo result.last_insert_id

db.close()
```

```console
1
1
```

And `exec_script()` runs several statements at once, which is what a schema
file contains:

```zuri,ignore
db.exec_script(file('schema.sql').read())
```

## Parameters

Values are bound by the database engine and never inserted into the text of
the statement. That's what keeps a value from being treated as SQL, and it's
your entire defense against SQL injection.

Use `?` for positional parameters:

```zuri
import sql

var db = sql.open('sqlite://guide.db')

echo db.fetch_column(
  'select title from posts where author_id = ? and published = ?',
  [1, true],
)

db.close()
```

```console
[On Engines]
```

Or use `:name` for named parameters, where the order doesn't matter and you
can use the same name more than once:

```zuri
import sql

var db = sql.open('sqlite://guide.db')

echo db.fetch_column(
  'select title from posts where views > :floor and views < :ceiling',
  { floor: 100, ceiling: 1000 },
)

db.close()
```

```console
[On Engines, On Compilers]
```

`sql` rewrites these into whatever form the adapter expects: `$1` and `$2`
for PostgreSQL, `?1` and `?2` for SQLite, and `?` unchanged for MySQL, which
already uses that form. The scanner recognizes when a `?` or a `:` isn't a
placeholder, so none of these are changed:

```zuri,ignore
"a ? inside a string"      -- a string
"a ? in an identifier"     -- a quoted identifier
-- a ? in a line comment
/* a ? in a block comment */
$tag$ a ? in a dollar-quoted body $tag$
value::text                -- a PostgreSQL cast
arr[1:3]                   -- an array slice
```

PostgreSQL uses `?` as a JSON operator, so to write a literal one, use `??`:

```zuri,ignore
db.query('select * from docs where data ?? ?', ['key'])
```

> Never build a statement by joining strings around a value, even when the
> value looks safe. The two forms above cover every case where a **value**
> varies. When an **identifier** varies, quote it through the driver instead
> of interpolating it:
>
> ```zuri,ignore
> var column = db.driver().quote_identifier(name)
> db.query('select ${column} from posts')
> ```

## Rows, Columns and Types

A row is a dictionary keyed by column name, which is why you can write
`post.title`.

```zuri
import sql

var db = sql.open('sqlite://guide.db')
var result = db.query('select id, title, views from posts order by id limit 1')

echo result.first()
echo result.columns
echo result.tuples()

db.close()
```

```console
{id: 1, title: On Engines, views: 412}
[{name: id, type: integer}, {name: title, type: text}, {name: views, type: integer}]
[[1, On Engines, 412]]
```

Two columns with the same name collapse into one entry in a dictionary, so
keep that in mind before you select `id` from both sides of a join. Give one
of them an alias to fix it, or use `tuples()` if you can't.

Values are converted like this:

| Zuri | Database |
| --- | --- |
| `nil` | NULL |
| `bool` | A boolean, where the engine has one, and 0 or 1 otherwise |
| `number` | An integer if the value is a whole number, and a float otherwise |
| `bigint` | A 64-bit integer, for values too large for a double |
| `string` | Text |
| `bytes` | A blob |
| `date.Date` | A timestamp |
| `sql.Time` | A `TIME` interval, on engines that have one |
| `sql.Decimal` | An exact decimal |
| `list`, `dict` | JSON |

Not every engine has all of these types. PostgreSQL has a real boolean type
and MySQL doesn't, so a `true` written to MySQL is read back as `1`. Use
`db.supports()` to find out about this kind of difference instead of
guessing. Each engine's section below describes where it differs.

An integer that's too large for a double is returned as a `bigint` instead
of being silently rounded:

```zuri
import sql

var db = sql.open(':memory:')
db.exec('create table t (n integer)')
db.exec('insert into t values (9223372036854775807)')

var n = db.fetch_value('select n from t')
echo typeof(n)
echo n

db.close()
```

```console
bigint
9223372036854775807n
```

SQLite stores five kinds of value and doesn't record what you meant, so a
boolean is stored as 1 and would be read back as the number 1. What SQLite
does keep is the type each column was *declared* with, and the adapter uses
that to convert values when it reads them:

```zuri
import sql

var db = sql.open(':memory:')
db.exec('create table t (ok boolean, at datetime, doc json)')
db.exec('insert into t values (?, ?, ?)', [
  true,
  '2026-09-16T14:30:00.000000+00:00',
  '{"a":1}',
])

var row = db.fetch_one('select * from t')

echo typeof(row.ok)
echo typeof(row.at)
echo row.doc

db.close()
```

```console
bool
Date
{a: 1}
```

A column without a declared type is an expression, and its value is returned
exactly as it was stored.

### Exact Decimals

A `number` is a double, which can only store `0.1` approximately. That isn't
good enough for money, so `sql.Decimal` stores a value exactly:

```zuri
import sql { Decimal }

var price = Decimal('19.99')
var tax = price.multiply(Decimal('0.20'))

echo price.to_string()
echo tax.to_string()
echo price.add(tax).to_string()
echo Decimal('0.1').add(Decimal('0.2')).to_string()
```

```console
19.99
3.9980
23.9880
0.3
```

PostgreSQL's `numeric`, MySQL's `DECIMAL`, and SQLite's `DECIMAL` and
`NUMERIC` columns are read and written as `Decimal` values. SQLite stores a
decimal as a double, which holds 15 significant digits exactly, so a decimal
column in SQLite can hold at most 15 digits, and `table.decimal()` rejects a
larger precision on SQLite. An aggregate such as `sum()` doesn't have a
declared type, so its result is returned as a number.

## CRUD Helpers

Four methods on a connection let you work with rows without writing any SQL.
`insert()`, `update()`, `delete()` and `find()` take a table name and
dictionaries, build the statement for whichever engine you're connected to,
and bind every value as a parameter.

They exist because the statements they replace are the ones least worth
writing by hand. Those statements are repetitive, they differ between
dialects in small ways that only show up in production, and building them by
joining strings is how SQL injection gets in. Anything more complex than a
simple list of conditions is still written in SQL, and you can mix the two
freely on the same connection.

### Inserting Rows and Getting Their IDs

`insert()` takes a table name and a dictionary, and returns the ID of the
new row:

```zuri
import sql

var db = sql.open(':memory:')
db.exec('create table posts (id integer primary key, title text)')

echo db.insert('posts', { title: 'Hello' })
echo db.insert('posts', { title: 'World' })

db.close()
```

```console
1
2
```

This method hides the biggest difference between the engines. SQLite and
MySQL report the ID of the row that was just inserted, but PostgreSQL
doesn't, so an insert that needs the ID has to ask for it with a `RETURNING`
clause that names the primary key. MariaDB supports both and uses
`RETURNING`. `insert()` does whichever applies, and finds the primary key by
asking the schema rather than assuming it's called `id`.

If you want a column other than the key to be returned, name it:

```zuri,ignore
db.insert('events', { name: 'started' }, { returning: 'uuid' })
```

### Inserting Several Rows at Once

Several rows can be inserted in a single statement, which is split up so
that no statement binds more parameters than the engine allows:

```zuri
import sql

var db = sql.open(':memory:')
db.exec('create table points (x integer, y integer)')

echo db.insert_many('points', [
  { x: 1, y: 2 },
  { x: 3, y: 4 },
])

db.close()
```

```console
2
```

Every row must have the same columns. A row with a different set of columns
raises an error instead of being filled in with nulls, because a missing
column and a null value mean different things.

### Finding Rows

`find()` returns a `ResultSet`, `count()` returns a number, and `find_one()`
returns the first matching row as a dictionary:

```zuri
import sql

var db = sql.open('sqlite://guide.db')

echo db.count('posts')
echo db.count('posts', { published: true })
echo db.count('posts', {
  author_id: [1, 2],
})
echo db.count('posts', { author_id: [] })

echo db
    .find(
      'posts',
      { published: true },
      {
        columns: ['title'],
        order: ['views desc'],
        limit: 2,
      },
    )
    .column('title')

echo db.find_one('posts', { title: 'On Bugs' }).views

db.close()
```

```console
4
3
4
0
[On Bugs, On Compilers]
1290
```

If `find_one()` doesn't find a match, it returns `nil`. That's a valid
answer rather than a failure, so no error is raised:

```zuri
import sql

var db = sql.open('sqlite://guide.db')

echo db.find_one('posts', { title: 'On Bugs' }).views
echo db.find_one('posts', { title: 'Never Written' })

db.close()
```

```console
1290
nil
```

### Writing Filters

The type of a condition's value determines what it means. A plain value
means equality, a list means `IN`, an empty list matches nothing, and `nil`
means `IS NULL` rather than `= NULL`, which no row would ever match.

Several conditions in the same dictionary are combined with `AND`. There's
no way to express `OR`, so that's usually the first thing you'll need to
write in SQL:

```zuri
import sql

var db = sql.open('sqlite://guide.db')

echo db.count('posts', { published: true, author_id: 1 })
echo db.count('posts', {
  published: true,
  views: [87, 1290],
})

db.close()
```

```console
1
1
```

Passing `nil` as the whole filter matches every row. You have to ask for
that explicitly, rather than it happening because a dictionary turned out to
be empty. That's what prevents a filter built from user input from turning
into an `UPDATE` without a `WHERE`.

### Ordering, Limits and Pages

`order` is a list of column names, each optionally followed by ` asc` or
` desc`, and they're applied in the order given:

```zuri
import sql

var db = sql.open('sqlite://guide.db')

echo db
    .find('posts', nil, {
      order: ['views desc'],
    })
    .column('title')
echo db
    .find('posts', nil, {
      order: ['author_id asc', 'views desc'],
    })
    .column('title')

db.close()
```

```console
[On Bugs, On Compilers, On Engines, On Looms]
[On Engines, On Looms, On Bugs, On Compilers]
```

`limit` and `offset` are numbers of rows, and together they define a page. A
page beyond the end of the results is empty rather than an error:

```zuri
import sql

var db = sql.open('sqlite://guide.db')

def page(number, size) {
  return db
      .find('posts', nil, {
        columns: ['title'],
        order: ['views desc'],
        limit: size,
        offset: (number - 1) * size,
      })
      .column('title')
}

echo page(1, 2)
echo page(2, 2)
echo page(3, 2)

db.close()
```

```console
[On Bugs, On Compilers]
[On Engines, On Looms]
[]
```

The limit and the offset are written into the text of the statement instead
of being bound as parameters, because not every engine allows a parameter in
those positions. That makes them the only part of a generated statement that
isn't a parameter, so each one is validated before it's added:

```zuri
import sql

var db = sql.open('sqlite://guide.db')

catch {
  db.find('posts', nil, { limit: 2.5 })
} as error {
  echo error.message
}

db.close()
```

```console
a limit is a whole number of rows, not 2.5
```

### Updating and Deleting Rows

`update()` and `delete()` return the number of rows they changed:

```zuri
import sql

var db = sql.open(':memory:')
db.exec('create table t (id integer primary key, n integer)')
db.insert_many('t', [
  { n: 1 },
  { n: 2 },
  { n: 3 },
])

echo db.update(
  't',
  { n: 0 },
  {
    n: [1, 2],
  },
)
echo db.delete('t', { n: 0 })
echo db.count('t')

db.close()
```

```console
2
2
1
```

They interpret a filter in exactly the same way as `find()`, including
`nil`, which means that passing `nil` updates or deletes every row.

### Raw SQL Fragments

When you need to use something that isn't a plain value, `sql.raw()` marks a
fragment of SQL to be used exactly as written:

```zuri
import sql

var db = sql.open(':memory:')
db.exec('create table t (id integer primary key, views integer)')
db.insert('t', { views: 10 })

db.update('t', { views: sql.raw('views + 1') }, { id: 1 })
echo db.fetch_value('select views from t')

db.close()
```

```console
11
```

A `Raw` value is accepted anywhere a column or a value is, which lets you
use an aggregate or a subquery without giving up the helpers:

```zuri
import sql

var db = sql.open('sqlite://guide.db')

echo db.find('posts', nil, {
  columns: [sql.raw('count(*) as n'), sql.raw('sum(views) as total')],
}).first()

echo db.count('posts', {
  author_id: sql.raw("(select id from authors where name = 'Ada Lovelace')"),
})

db.close()
```

```console
{n: 4, total: 2429}
2
```

> `raw()` is exactly as dangerous as it sounds. A fragment built from
> anything a user provided is a SQL injection vulnerability. Only build
> fragments from literals, and keep values in parameters, where they belong.

### Using the Helpers in a Transaction

A `Transaction` has all of the same methods, and they work the same way. A
read inside a transaction sees what that transaction has written, even
though nobody else has committed it yet:

```zuri
import sql

var db = sql.open(':memory:')
db.exec('create table t (id integer primary key, n integer)')

db.transaction(@(tx) {
  tx.insert_many('t', [
    { n: 1 },
    { n: 2 },
    { n: 3 },
  ])

  echo tx.count('t')
  echo tx
      .find('t', nil, {
        order: ['n desc'],
      })
      .column('n')

  tx.update('t', { n: 9 }, { n: 1 })
  tx.delete('t', { n: 3 })
})

echo db
    .find('t', nil, {
      order: ['n asc'],
    })
    .column('n')

db.close()
```

```console
3
[3, 2, 1]
[2, 9]
```

Rolling back undoes those changes, and a transaction that has already been
committed rejects these calls, just as it rejects everything else.

### Limitations of the Helpers

The helpers only support a simple list of equality conditions. There are no
joins, no `OR` and no expression trees, because SQL is a better language for
those than any chain of method calls. For a query that needs one, use
`query()` or `fetch_all()` on the same connection, alongside the helpers.

## Transactions

With the closure form, the transaction is committed when the body returns
and rolled back when it raises an error, so there's no way to leave a
transaction open by accident:

```zuri
import sql

var db = sql.open(':memory:')
db.exec('create table accounts (id integer primary key, balance integer)')
db.insert_many('accounts', [
  { balance: 500 },
  { balance: 0 },
])

db.transaction(@(tx) {
  tx.update('accounts', { balance: sql.raw('balance - 100') }, { id: 1 })
  tx.update('accounts', { balance: sql.raw('balance + 100') }, { id: 2 })
})

echo db.fetch_column('select balance from accounts order by id')

db.close()
```

```console
[400, 100]
```

A failure undoes everything in the transaction:

```zuri
import sql

var db = sql.open(':memory:')
db.exec('create table t (n integer)')

catch {
  db.transaction(@(tx) {
    tx.exec('insert into t values (1)')
    raise Error('something went wrong')
  })
} as error {
  echo 'rolled back: ${error.message}'
}

echo db.count('t')
db.close()
```

```console
rolled back: something went wrong
0
```

A `transaction()` called inside another one becomes a savepoint, so a helper
that opens a transaction works the same way whether it's called on its own
or as part of a larger piece of work. Only the outermost transaction
commits, and a failure inside an inner one only undoes that inner piece:

```zuri
import sql

var db = sql.open(':memory:')
db.exec('create table t (name text)')

db.transaction(@(outer) {
  outer.insert('t', { name: 'outer' })

  catch {
    db.transaction(@(inner) {
      inner.insert('t', { name: 'inner' })
      raise Error('inner failed')
    })
  } as _error {
    echo 'inner undone'
  }
})

echo db.fetch_column('select name from t')
db.close()
```

```console
inner undone
[outer]
```

On SQLite and PostgreSQL, schema changes are part of the transaction, so if
a series of `create table` statements fails halfway through, nothing is left
behind. MySQL and MariaDB commit any open transaction on every `create`,
`alter` and `drop`, so on those engines, a `transaction()` whose body
changes the schema raises a `TransactionError` when it tries to commit.
`db.supports('transactional_ddl')` tells you which behavior applies, so a
program that migrates its own schema can run each change inside a
transaction where that works, and on its own where it doesn't.

You can request an isolation level. If an engine can't provide the level you
ask for, it says so instead of quietly giving you a weaker one:

```zuri,ignore
db.transaction(@(tx) { ... }, sql.SERIALIZABLE)
```

| Level | Support |
| --- | --- |
| `sql.READ_UNCOMMITTED` | MySQL supports it. PostgreSQL accepts it, but gives you read committed. |
| `sql.READ_COMMITTED` | Supported everywhere |
| `sql.REPEATABLE_READ` | Supported everywhere, and the default on MySQL |
| `sql.SERIALIZABLE` | Supported everywhere |

`begin()` starts a transaction that you commit or roll back yourself, for a
transaction that doesn't fit inside a single block. The closure form is
safer, so use it when you can.

## Streaming Large Results

`query()` builds the entire result in memory. That's fine for the hundreds
of rows most queries return, but not for the millions that some return.
`stream()` reads the same result one row at a time:

```zuri
import sql

var db = sql.open('sqlite://guide.db')
var cursor = db.stream('select title from posts order by title')

for post in cursor {
  echo post.title
}

db.close()
```

```console
On Bugs
On Compilers
On Engines
On Looms
```

Reading to the end closes the cursor. If a loop stops early, the cursor
isn't closed, so any loop that might `break` needs to close it:

```zuri,ignore
var cursor = db.stream('select * from events')

for event in cursor {
  if done(event) {
    break
  }
}

cursor.close()
```

`take(n)` reads a batch of rows at a time, for work that naturally happens
in batches:

```zuri
import sql

var db = sql.open('sqlite://guide.db')
var cursor = db.stream('select title from posts order by title')

echo cursor.take(2).map(@(post) => post.title)
echo cursor.take(2).map(@(post) => post.title)
echo cursor.take(2)

db.close()
```

```console
[On Bugs, On Compilers]
[On Engines, On Looms]
[]
```

On PostgreSQL, this uses a server-side portal, and on MySQL a server-side
cursor. Both fetch rows in batches, whose size you can set with
`{ batch: n }`. On SQLite, the engine itself computes rows as they're
requested, so only the current row is held in memory.

A server-side cursor keeps its connection busy while it's open, because the
server is partway through sending the result. If you run another statement
on that connection before the cursor has been read to the end or closed, an
error is raised, so the two exchanges can't get mixed up.

## Prepared Statements

Compiling a statement is the expensive part of running it, so a statement
that runs in a loop should only be compiled once:

```zuri
import sql

var db = sql.open(':memory:')
db.exec('create table points (x integer, y integer)')

var insert = db.prepare('insert into points (x, y) values (?, ?)')

for i in 0..(1000) {
  insert.exec([i, i * 2])
}

insert.close()

echo db.count('points')
db.close()
```

```console
1000
```

The placeholders are translated once, when the statement is prepared, so the
loop doesn't do any string processing at all. A prepared statement has the
same query and fetch methods as a connection.

A prepared statement belongs to the connection that prepared it. Closing the
connection also closes the statement and any cursor still open on it, and
using either of them afterwards raises a `ClosedError`.

## Connection Pools

Opening a connection is expensive. For PostgreSQL and MySQL, it involves a
TCP connection, a TLS handshake and an authentication exchange, all before
the first statement runs. A server that opened a new connection for every
request would spend most of its time connecting.

```zuri
import sql

var db = sql.pool('sqlite://guide.db', { max: 4 })

echo db.fetch_value('select count(*) from posts')
echo db.stats()

db.close()
```

```console
4
{size: 1, idle: 1, in_use: 0, max: 4}
```

Used like this, the pool takes a connection, runs the statement and returns
the connection to the pool. When several statements have to run on the same
connection, as they do in a transaction, `with_connection()` holds onto one
connection and releases it however the block ends:

```zuri,ignore
db.with_connection(@(connection) {
  connection.transaction(@(tx) {
    tx.exec('...')
  })
})
```

`transaction()` on the pool does the same thing in a single call.

| Setting | Meaning |
| --- | --- |
| `max` | The maximum number of connections to open. The default is ten. |
| `min` | The number of connections to open in advance. The default is none. |
| `idle_timeout` | How long an idle connection is kept |
| `max_lifetime` | How long a connection is kept before it's replaced |
| `validate_on_acquire` | Whether to check that a connection still works before handing it out |
| `on_connect` | A function to run on each new connection |

Keep a pool as small as your workload allows. Connections have a cost on the
database side too, and a pool that's larger than the database can usefully
handle just moves a queue from your application into the database, where
it's harder to see.

A pool belongs to the isolate that created it, which has two consequences.
An isolate that needs database access creates its own pool. And a call never
waits when the pool is empty, because nothing else could release a
connection while the call is running, so running out of connections is
reported as an error rather than waited out.

## Errors

Every error is a `sql.SqlError`. Catch that to handle anything that can go
wrong with a database, and use the subclasses to handle particular kinds of
error differently.

```text
Error
└── SqlError
    ├── ConnectionError
    │   ├── AuthenticationError
    │   ├── TimeoutError
    │   └── ProtocolError
    ├── QueryError
    ├── IntegrityError
    │   ├── UniqueViolation
    │   ├── ForeignKeyViolation
    │   ├── NotNullViolation
    │   └── CheckViolation
    ├── TransactionError
    │   ├── SerializationError
    │   └── DeadlockError
    ├── PoolError
    │   └── PoolExhaustedError
    ├── NotSupportedError
    └── ClosedError
```

The error classes are the same on every engine, and that's the point. A
unique constraint violation is SQLSTATE `23505` on PostgreSQL, error number
`1062` on MySQL and extended result code `2067` on SQLite. None of these
numbers mean anything to the other engines, so code that checked for any of
them would stop working as soon as you changed adapters.

```zuri
import sql

var db = sql.open(':memory:')
db.exec('create table users (id integer primary key, email text unique)')
db.insert('users', { email: 'ada@example.com' })

catch {
  db.insert('users', { email: 'ada@example.com' })
} as error {
  echo instance_of(error, sql.UniqueViolation)
  echo instance_of(error, sql.IntegrityError)
  echo instance_of(error, sql.SqlError)
  echo error.driver
}

db.close()
```

```console
true
true
true
sqlite
```

The engine's own information is kept rather than thrown away. `code` holds
the code the engine reported, `sqlstate` holds the five-character SQLSTATE
code where there is one, and `query` holds the statement that failed:

```zuri
import sql

var db = sql.open(':memory:')
db.exec('create table users (id integer primary key, email text unique)')
db.insert('users', { email: 'ada@example.com' })

catch {
  db.insert('users', { email: 'ada@example.com' })
} as error {
  echo error.code
  echo error.sqlstate
  echo error.type
}

db.close()
```

```console
2067
nil
UniqueViolation
```

`SerializationError` and `DeadlockError` are the two errors worth retrying.
Both mean that the engine abandoned a transaction to keep its guarantees,
and running the whole transaction again usually succeeds.

## Schema Introspection

`db.schema` tells you what's in the database. The queries it runs are
completely different for each engine, but the results have the same
structure.

Every method takes an optional schema name. PostgreSQL has schemas inside
each database, and finds the default through `search_path`. MySQL calls a
database a schema and has no level above it, so naming a schema there names
a database. SQLite has neither, and ignores the argument.

```zuri
import sql

var db = sql.open('sqlite://guide.db')

echo db.schema.tables()
echo db.schema.column_names('posts')
echo db.schema.primary_key('posts')
echo db.schema.has_column('posts', 'views')

db.close()
```

```console
[authors, posts]
[id, author_id, title, views, published]
id
true
```

Each column is described in the same way, whichever engine you're using:

```zuri
import sql

var db = sql.open('sqlite://guide.db')

for column in db.schema.columns('posts') {
  echo '${column.name} ${column.type} ${column.nullable ? "null" : "not null"}'
}

db.close()
```

```console
id integer null
author_id integer not null
title text not null
views integer not null
published boolean not null
```

`indexes()` and `foreign_keys()` describe the rest of the table:

```zuri
import sql

var db = sql.open('sqlite://guide.db')

echo db.schema.foreign_keys('posts')

db.close()
```

```console
[{columns: [author_id], references_table: authors, references_columns: [id], on_update: NO ACTION, on_delete: NO ACTION}]
```

This is also what makes `insert()` portable. On an engine without a "last
insert ID", the insert needs a `RETURNING` clause, and the column to return
is the table's primary key, which is looked up here.

One thing to watch for when moving a schema between engines: MySQL parses a
`references` clause on a column and then ignores it, so a foreign key
declared that way exists on the other two engines but not on MySQL. If you
declare it at the table level instead, it exists on all three.

Creating, changing and dropping tables, using one description that works for
every engine, is covered in [Changing the
Schema](ch17-01-schema-changes.md).

## Switching Databases

This module promises that a program can switch between engines by changing
its connection string. Here's a program that shows it:

```zuri
import sql

def report(db) {
  db.exec_script('drop table if exists tally')
  db.exec_script(
    'create table tally (id ' +
        key_type(db) +
        ' primary key, word text not null)',
  )

  db.insert_many('tally', [
    { word: 'alpha' },
    { word: 'beta' },
  ])

  var id = db.insert('tally', { word: 'gamma' })
  var found = db.fetch_column('select word from tally order by word')

  db.exec_script('drop table tally')

  return '${db.driver_name()}: ${found} (last id ${id})'
}

def key_type(db) {
  using db.driver_name() {
    when 'postgres' return 'serial'
    when 'mysql' return 'integer auto_increment'
    when 'mariadb' return 'integer auto_increment'
  }

  return 'integer'
}

var sqlite = sql.open(':memory:')

echo report(sqlite)
sqlite.close()
```

```console
sqlite: [alpha, beta, gamma] (last id 3)
```

The same `report()` function runs against PostgreSQL or MySQL if you open a
different connection, and prints the same list.

Three things had to be written separately for each engine, and they're the
three things that really are different:

- **The connection string**, which is the whole idea.
- **An auto-incrementing key**, which isn't part of standard SQL. PostgreSQL
  uses `serial`, MySQL uses `auto_increment`, and SQLite uses
  `integer primary key`.
- **Any SQL that only one engine supports.** `sql` doesn't parse or rewrite
  statements beyond their placeholders, so a PostgreSQL array operator, a
  MySQL `on duplicate key update` or a SQLite `json_extract` is passed
  through unchanged.

`db.supports()` lets a program ask about a feature instead of assuming:

```zuri
import sql

var db = sql.open(':memory:')

echo db.driver_name()
echo db.supports('returning')
echo db.supports('arrays')
echo db.supports('concurrent_writers')
echo db.capabilities().placeholder_style

db.close()
```

```console
sqlite
true
false
false
indexed
```

## SQLite Features

`db.native()` gives you the adapter underneath, which is where the engine's
own features are.

A SQLite database is a file, except for `:memory:`, which isn't. An
in-memory database belongs to the connection that opened it, so a second
connection to `:memory:` opens a second, empty database, not another handle
to the same data.

Foreign keys are enforced. By default, SQLite doesn't enforce them,
separately for each connection, for compatibility with databases created
before it supported foreign keys. A database that declares foreign keys
almost certainly wants them enforced, so the adapter turns enforcement on.
Pass `foreign_keys: false` to turn it off.

SQLite doesn't have a timestamp type, so a `date.Date` is stored as ISO 8601
text in UTC, whatever time zone offset it had. Text in that format sorts and
compares in chronological order, so `order by` and `<` on such a column
agree with the clock. When a date is read back, it represents the same
moment, in UTC.

### Functions Written in Zuri

A function you register on a connection runs inside the database engine,
once for each row, and you can use it anywhere an expression is allowed:

```zuri
import sql

var db = sql.open('sqlite://guide.db')

db.native().create_function('initials', 1, @(name) {
  return ''.join(name.split(' ').map(@(part) => part[0, 1]))
})

echo db.fetch_column('select initials(name) from authors order by name')

db.close()
```

```console
[AL, GH]
```

An aggregate is written as a fold. `step` is called once for each row, with
the accumulator and the row's arguments, and returns the next accumulator.
`finish` turns the final accumulator into the value for the group.

```zuri
import sql

var db = sql.open('sqlite://guide.db')

db.native().create_aggregate('longest', 1, @(longest, title) {
  if longest == nil or title.length() > longest.length() {
    return title
  }

  return longest
}, @(longest) => longest)

echo db.fetch_value('select longest(title) from posts')

db.close()
```

```console
On Compilers
```

A collation defines an ordering for text, and you use it with `collate`:

```zuri
import sql

var db = sql.open('sqlite://guide.db')

db.native().create_collation('bylength', @(a, b) => a.length() - b.length())

echo db.fetch_column('select title from posts order by title collate bylength')

db.close()
```

```console
[On Bugs, On Looms, On Engines, On Compilers]
```

> A function must not use the connection it was registered on. SQLite is in
> the middle of running a statement when it calls the function, and using
> the connection again would cause a deadlock. If the function raises an
> error, the error is passed back out and raised again, with its own class,
> once the statement has finished.

### Blobs, Backups and Hooks

A large value read with a `select` arrives all at once. A blob handle opens
a single cell and lets you read parts of it:

```zuri,ignore
var blob = db.native().blob('main', 'files', 'content', id, false)

var at = 0
while at < blob.length() {
  out.write(blob.read(at, 65536.min(blob.length() - at)))
  at += 65536
}

blob.close()
```

The cell must already exist and already be the right size, because SQLite
can't make a blob larger this way. That's what inserting `zeroblob(n)` is
for.

Copying the database file with file system tools is only safe when nothing
is writing to it. SQLite's own backup copies the database page by page and
notices when the source changes during the copy:

```zuri,ignore
db.native().backup_to('./snapshot.db')
```

The hooks report what the engine is doing:

```zuri,ignore
db.native().on_change(@(operation, database, table, rowid) { ... })
db.native().on_commit(@() => true)
db.native().on_rollback(@() { ... })
db.native().set_authorizer(@(action, first, second, database, trigger) { ... })
db.native().on_progress(1000, @() => keep_going())
```

## PostgreSQL Features

The adapter uses version 3 of the PostgreSQL wire protocol, over TCP or a
Unix domain socket, with optional TLS.

A `host` that starts with a slash is a socket directory, as it is in libpq,
and the socket inside it is named after the port. So `/var/run/postgresql`
with port 5432 means `/var/run/postgresql/.s.PGSQL.5432`. A path that
already names the socket file is used as it is. TLS isn't used over a
socket, because there's nothing in between to protect against, so `sslmode`
is ignored in that case.

```zuri,ignore
sql.open('postgres:///app?host=/var/run/postgresql')
sql.open({ driver: 'postgres', host: '/var/run/postgresql', database: 'app' })
```

`sslmode` controls how TLS is used: `disable` never uses it, `prefer` uses
it when the server supports it, and `require` always uses it and fails if
the server refuses. Only `require` verifies the server's identity.

Statements are sent using the extended query protocol, which compiles them
on the server and binds values rather than substituting them into the text.
Values are sent in the server's own binary format whenever the adapter has a
codec for the type, and as text otherwise, so even a type it doesn't know
about arrives in readable form instead of as raw bytes.

`numeric` columns are read as `sql.Decimal` values, arrays as lists, `jsonb`
as dictionaries, and timestamps as `date.Date` values.

### Listening for Notifications

`LISTEN` and `NOTIFY` are PostgreSQL's built-in publish and subscribe
mechanism. A connection that has run `LISTEN channel` receives a message
whenever anyone runs `NOTIFY channel`.

```zuri,ignore
var listener = db.native().listener()

listener.listen('jobs')

while true {
  for message in listener.wait(nil) {
    handle(message.channel, message.payload)
  }
}
```

Notifications arrive in between other messages, so a connection that's busy
running statements collects them along the way, and `poll()` returns
whatever has built up.

### Adding Support for a Type

If your database defines its own types, you can tell the adapter how to
handle them:

```zuri,ignore
import sql.postgres { DEFAULT_REGISTRY }

DEFAULT_REGISTRY.register(oid, decoder, encoder)
```

Until you do, columns of that type are read as text, which is correct, if
not especially convenient.

## MySQL Features

The adapter implements the MySQL client/server protocol directly, over TCP
or a Unix domain socket, with optional TLS, and doesn't depend on any client
library. MariaDB uses the same protocol, and the same adapter works with it.

A `socket` option names a socket, and so does a `host` that starts with a
slash. Unlike with PostgreSQL, the path names the socket file itself, not
the directory it's in:

```zuri,ignore
sql.open({
  driver: 'mysql',
  socket: '/var/run/mysqld/mysqld.sock',
  user: 'app',
})
sql.open('socket=/var/run/mysqld/mysqld.sock user=app database=shop')
sql.open('mysql://app@localhost/shop?socket=/var/run/mysqld/mysqld.sock')
```

TLS isn't used over a socket, because there's nothing in between to protect
against. The connection is treated as private, which lets
`caching_sha2_password` and `sha256_password` send the password directly
instead of encrypting it with the server's public key.

A statement without values is sent as text, which takes one round trip. A
statement with values is prepared, so the values are sent in the server's
binary format instead of being written into the SQL, and quoting is never an
issue. Compiled statements are kept and reused, so a query that runs in a
loop is only compiled once.

```zuri
import sql

var mysql = sql.driver('mysql')

echo mysql.capabilities().placeholder_style
echo mysql.capabilities().last_insert_id
echo mysql.capabilities().returning
echo mysql.quote_identifier('order by')
echo sql.driver('mariadb').capabilities().returning
```

```console
question
true
false
`order by`
true
```

MySQL leaves `?` as it's written, and it reports the ID of an inserted row
instead of returning it, so `insert()` reads the reported ID instead of
adding a `RETURNING` clause. MariaDB differs in two ways that affect the SQL
generated by the layers above: it supports `RETURNING`, and it doesn't have
a JSON type. Which behavior applies depends on the server, not the scheme,
so for a MariaDB server reached through `mysql://`, `db.driver_name()`
returns `'mariadb'` and `db.supports('json')` returns false.

### How Column Types Are Read

`DECIMAL` columns are read as `sql.Decimal` values, `JSON` as lists and
dictionaries, `DATE`, `DATETIME` and `TIMESTAMP` as `date.Date` values, and
`TIME` as `sql.Time` values.

MySQL doesn't have a boolean type. `BOOLEAN` is another name for
`TINYINT(1)`, so a value written as `true` is read back as `1`, and
`db.supports('booleans')` returns false to reflect that.

`BLOB` and `TEXT` have the same type in the protocol, and the only way to
tell them apart is the column's collation, which the adapter checks. A
binary column is read as `bytes` and a text column as a string. `BIT` and
`GEOMETRY` are read as `bytes`, because Zuri has no natural way to represent
them. `SET` is read as the comma-separated text that the server stores, not
as a list, because a list written back would be encoded as JSON and the
column would silently stop matching.

MySQL writes a missing date as zeros, but `0000-00-00` isn't a date in any
calendar, so it's read as `nil`.

### TIME Values Are Durations

A `TIME` column ranges from `-838:59:59.999999` to `838:59:59.999999`. It's
a duration rather than a time of day, so it can be negative and can be
longer than 24 hours, and neither of those would survive being read into a
`date.Date`. `sql.Time` holds it instead:

```zuri
import sql

var shift = sql.time(9, 30, 0)
var late = sql.time(0, 0, 30, 0, true)

echo shift.to_string()
echo shift.total_seconds()
echo late.to_string()
echo late.total_seconds()

# Two spans of the same length are equal however each was built.
echo sql.time(1, 30).equals(sql.time(0, 90))

# And the parts are kept as they were written.
echo sql.time(0, 90).minutes

echo sql.parse_time('-838:59:59.999999').to_string()
echo sql.time_from_seconds(-5400).to_string()
```

```console
09:30:00
34200
-00:00:30
-30
true
90
-838:59:59.999999
-01:30:00
```

### Time zones

A `DATETIME` doesn't store a time zone, and a `TIMESTAMP` is converted to
and from the session's time zone. That means the session's time zone
determines what a timestamp means, and if it were left to the server's
configuration, the same database could give different results on two
different machines.

The adapter sets the session's time zone to UTC when it connects. Every
timestamp is then read in UTC, and a `date.Date` that's written back is
converted from whatever offset it has. To keep the server's own setting,
pass `nil` as `time_zone`, or pass the name of the time zone you want to
use:

```zuri,ignore
sql.open('mysql://localhost/app', { time_zone: nil })
sql.open('mysql://localhost/app', { time_zone: '+01:00' })
```

### Authentication

The supported methods are `caching_sha2_password`, which is the default from
MySQL 8.0 onward, `mysql_native_password`, `sha256_password`,
`mysql_clear_password` and MariaDB's `client_ed25519`.

Two of these need the actual password, rather than proof of it, the first
time an account authenticates. Over TLS, the password is sent as is. Over an
unencrypted connection, it's encrypted with a public key that the server
sends first, so it can never be read in transit. `mysql_clear_password` has
no such fallback, so it's rejected on any connection that isn't private,
because it sends the password without any protection.

### TLS

`sslmode` controls how TLS is used:

| Mode | Meaning |
| --- | --- |
| `disable` | Never use TLS. |
| `prefer` | Use TLS when the server supports it, without verifying the server's identity. This is the default. |
| `require` | Always use TLS, without verifying the server's identity. |
| `verify` | Always use TLS, and verify the server's certificate and host name. |

MySQL's own names are accepted too. `DISABLED`, `PREFERRED` and `REQUIRED`
mean what they say, and `VERIFY_CA` and `VERIFY_IDENTITY` both become
`verify`. That makes `VERIFY_CA` stricter here than in the MySQL
command-line client, which checks the certificate but not the host name. The
difference can only ever cause a connection to be refused, never cause a
server to be wrongly trusted.

`ssl_ca` names a certificate authority to trust, and `ssl_cert` and
`ssl_key` provide a client certificate.

### Compression

The protocol can compress everything after the handshake:

```zuri,ignore
sql.open('mysql://localhost/app', { compression: 'zlib' })
sql.open('mysql://localhost/app', { compression: 'zstd' })
```

Compression is worthwhile for large results over a slow network, but costs
more than it saves over a local socket, so it's off unless you turn it on.
Packets that are too small to benefit are always sent uncompressed, which
the protocol allows.

### Sending a File to the Server

With `LOAD DATA LOCAL INFILE`, the server names a file and the client sends
it. The file is read from the machine the program is running on, but the
server chooses which file, so a malicious or compromised server could ask
for any file the process can read. For that reason, it's refused unless you
provide a reader:

```zuri,ignore
import os

var db = sql.open('mysql://localhost/app', {
  local_infile: @(name) {
    return os.file(name).read()
  },
})
```

The reader decides what can be sent, which is where that decision belongs.
When a request is refused, the server still gets an answer, so the
connection can still be used afterwards instead of waiting for a file that
will never arrive.

### Statements With Several Results

A stored procedure sends one result set for each `select` inside it,
followed by an OK packet. `query()` returns the first result set and reads
the others, because leaving them unread wouldn't discard them. They would be
returned to the next statement, which would then get this statement's rows.

```zuri,ignore
var first = db.query('call two_results()')
```

The body of a stored routine contains semicolons, and `exec_script()` splits
a script at every semicolon. So send a `create procedure` statement through
`exec()` as a single statement, not through `exec_script()`.

### Accessing the Adapter

```zuri,ignore
db.native().flavor() # 'mysql' or 'mariadb'
db.native().connection_id() # what KILL names
db.native().parameters() # what the server said about itself
db.native().reset() # back to a fresh session
```

## Writing Your Own Adapter

An adapter for another engine implements four classes: a `Driver` that reads
a connection string and opens a connection, a `DriverConnection` that runs
statements, a `DriverStatement` and a `DriverCursor`.

```zuri,ignore
import sql

class DuckDbDriver < sql.Driver {
  name() { return 'duckdb' }
  schemes() { return ['duckdb'] }

  capabilities() {
    var caps = sql.default_capabilities()
    caps.set('placeholder_style', sql.QUESTION)
    caps.set('returning', true)

    return caps
  }

  parse_dsn(dsn) { ... }
  connect(options) { ... }
}

sql.register(DuckDbDriver())

var db = sql.open('duckdb://./app.duckdb')
```

Everything above the adapter comes for free, including connection pools,
transactions and savepoints, the CRUD helpers, placeholder translation,
cursors, the error hierarchy and models. The adapter only has to provide
access to the engine.

Schema changes and migrations need one more thing: how the engine writes
table definitions. An adapter provides this by subclassing `sql.DdlAdapter`,
which already writes standard SQL, and overriding whatever the engine does
differently, such as `type_name()` for each column type and
`key_definition()` for an `id()` column. It then returns an instance of that
class from its driver's `ddl()` method. A driver that doesn't provide one
rejects schema changes with a `NotSupportedError`.

The base classes raise `NotImplementedError` for anything that hasn't been
implemented, so an unfinished adapter fails exactly where the gap is. An
engine that really can't support a feature raises `NotSupportedError`
instead. The two errors are deliberately different: the first means the
adapter is unfinished, and the second means the engine has a genuine
limitation.

## Design Limits

**Rows are dictionaries.** A connection returns rows, never objects. Models
are provided by [`sql.model`](ch17-03-models.md), which is built on top of
`sql` and only loaded by programs that use it.

**It doesn't write joins for you.** The CRUD helpers support a simple list
of equality conditions, and models support their own columns and
associations. Anything beyond that is written in SQL.

**It doesn't translate SQL.** Placeholders are rewritten, but statements
aren't parsed, so a function that only one engine supports still only works
on that engine.

**It doesn't hide differences by guessing.** When an engine can't do what
was asked, you get a `NotSupportedError` that names the adapter and the
feature, instead of an approximation you didn't ask for.

## Module Reference

The standard library reference documents every class and method. Here's an
overview of the module:

| Function | Description |
| --- | --- |
| `sql.open(dsn, options)` | Opens a connection |
| `sql.pool(dsn, options)` | Opens a connection pool |
| `sql.register(driver)` | Registers an adapter |
| `sql.drivers()` | Lists the registered adapters |
| `sql.raw(fragment)` | Marks SQL to be used exactly as written |
| `sql.Decimal(text)` | Creates an exact decimal |
| `sql.time(h, m, s)` | Creates a signed duration, like a value in a `TIME` column |
| `sql.parse_time(text)` | Reads a duration from text |
| `sql.blueprint` | Table definitions that work for every engine |
| `sql.migrate` | Versioned schema changes, and the `Migrator` that applies them |
| `sql.model` | Models: `Model`, `Query` and the errors they raise |

Methods of a `Connection`:

| Method | Purpose |
| --- | --- |
| `query`, `exec`, `exec_script` | Run statements |
| `fetch_one`, `fetch_all`, `fetch_value`, `fetch_column` | Read results |
| `stream`, `prepare` | Create a cursor or a compiled statement |
| `insert`, `insert_many`, `update`, `delete`, `find`, `find_one`, `count` | Run the common statements without writing SQL |
| `transaction`, `begin`, `in_transaction` | Work with transactions |
| `schema` | Introspection, plus `create_table`, `alter_table`, `rename_table` and `drop_table` |
| `model` | Query a model's rows |
| `driver`, `driver_name`, `capabilities`, `supports` | Describe the engine |
| `native` | The adapter underneath |
| `ping`, `close`, `is_closed` | Manage the connection itself |

Methods of a `Transaction`:

| Method | Purpose |
| --- | --- |
| `query`, `exec` | Run statements |
| `fetch_one`, `fetch_all`, `fetch_value` | Read results |
| `insert`, `insert_many`, `update`, `delete`, `find`, `find_one`, `count` | The same helpers a connection has |
| `schema`, `model` | The connection's schema and models, inside the transaction |
| `commit`, `rollback`, `is_finished` | End the transaction |
| `connection`, `nested` | The connection it's running on, and whether it's a savepoint |
