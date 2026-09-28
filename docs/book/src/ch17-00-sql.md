# Databases

The `sql` module is how a Zuri program talks to a relational database.
It is one module rather than one per engine, and that is the whole
design: `sql` defines what a database adapter has to provide, supplies
everything that is the same whichever engine answers, and picks the
adapter from the connection string.

Three adapters ship with it to support four major database engines
&mdash; SQLite, PostgreSQL, MySQL and MariaDB. SQLite is a file-based
database with no server to run and nothing to configure, which makes it
the right choice for an application that ships with its data and for a
test suite that wants a real database per run. PostgreSQL, MySQL, and
MariaDB are servers, for everything that outgrows a file, and the MySQL 
adapter is the same adapter that drives MariaDB as well.

Changing from one to another means changing the connection string, and
whatever SQL they genuinely spell differently.

- [Following Along](#following-along)
- [Introduction](#introduction)
- [Connecting](#connecting)
- [Querying and Fetching](#querying-and-fetching)
- [Parameters](#parameters)
- [Rows, Columns and Types](#rows-columns-and-types)
- [The CRUD Helpers](#the-crud-helpers)
- [Transactions](#transactions)
- [Streaming Large Results](#streaming-large-results)
- [Prepared Statements](#prepared-statements)
- [Connection Pools](#connection-pools)
- [Errors](#errors)
- [Schema Introspection](#schema-introspection)
- [Switching Databases](#switching-databases)
- [SQLite Specifics](#sqlite-specifics)
- [PostgreSQL Specifics](#postgresql-specifics)
- [MySQL Specifics](#mysql-specifics)
- [Writing Your Own Adapter](#writing-your-own-adapter)
- [What the Module Refuses](#what-the-module-refuses)
- [Module Reference](#module-reference)

> Blocks on this page that list several calls together are **reference
> listings**, not programs: they show the shape of each call rather
> than a sequence to run. Anything presented as a complete program runs
> as written. The PostgreSQL and MySQL examples need a server, so they
> are shown rather than run.

## Following Along

Most examples below use a small database of posts and their authors.
This builds it:

```zuri
import sql

var db = sql.open('sqlite://guide.db')

db.exec_script("
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
")

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

Everything from here on assumes that file exists in the working
directory.

## Introduction

A database library usually ties a program to one engine. The calls are
named after that engine, the placeholders are spelled its way, and the
errors are its own numbers, so moving to another means rewriting every
call site.

`sql` puts the parts that are the same in one place. A statement is
written once:

```zuri
import sql

var db = sql.open('sqlite://guide.db')

for post in db.query('select title, views from posts where published = ?', [true]) {
  echo '${post.title}: ${post.views}'
}

db.close()
```

```console
On Engines: 412
On Bugs: 1290
On Compilers: 640
```

Point the first line at `postgres://localhost/app` or
`mysql://localhost/app` and the rest runs unchanged. The `?` becomes
`$1` on PostgreSQL because that is what PostgreSQL wants, and stays `?`
on MySQL because that is what MySQL wants. An insert asking for its new
id gets a `RETURNING` clause on PostgreSQL, because PostgreSQL has no
last insert id and the other two do. A duplicate key arrives as
`UniqueViolation` from all three.

What it does not do is pretend the engines are the same. They disagree
about auto-incrementing keys, about which functions exist, about a
great deal of SQL. `sql` translates what can be translated and is
explicit about the rest.

## Connecting

`open()` takes a connection string and returns a `Connection`.

```zuri,ignore
sql.open('sqlite://./app.db')          # a file
sql.open(':memory:')                   # a private database in memory
sql.open('./app.db')                   # a bare path is SQLite
sql.open('postgres://localhost/app')
sql.open('postgres://alice:secret@db.internal:5432/shop?sslmode=require')
sql.open('host=localhost dbname=app user=alice')
sql.open('mysql://localhost/app')
sql.open('mysql://alice:secret@db.internal:3306/shop?sslmode=verify')
sql.open('mariadb://localhost/app')
```

The scheme picks the adapter. A string with no scheme at all is taken
as a SQLite path, since nothing else it could be.

Options can also be given as a dictionary, which is how anything an
adapter accepts beyond the connection string is passed:

```zuri,ignore
sql.open({
  driver: 'sqlite',
  path: './app.db',
  journal_mode: 'wal',
  busy_timeout: 10000,
})
```

or alongside a string, where they are merged over whatever it carried:

```zuri,ignore
sql.open('sqlite://./app.db', { journal_mode: 'wal' })
```

A connection should be closed when it is finished with, and a `catch`
block makes that certain even when the work between raises:

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

A connection belongs to the isolate that opened it and cannot be handed
to another. An isolate that needs the database opens its own connection
or its own pool.

## Querying and Fetching

`query()` runs a statement and reads its whole result:

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

A result is iterable, so the common case needs nothing else:

```zuri,ignore
for post in db.query('select * from posts') {
  echo post.title
}
```

For the shapes that come up constantly there are shorter forms:

```zuri,ignore
db.fetch_one(sql, params)             # the first row, or nil
db.fetch_all(sql, params)             # every row, as dictionaries
db.fetch_value(sql, params, fallback) # the first column of the first row
db.fetch_column(sql, params, column)  # one column's values, as a list
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

`exec()` is for a statement run for its effect rather than its rows:

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

And `exec_script()` runs several statements at once, which is what a
schema file is:

```zuri,ignore
db.exec_script(file('schema.sql').read())
```

## Parameters

Values are bound by the engine, never pasted into the statement. That
is what makes a value a value rather than a piece of SQL, and it is the
whole of the defence against injection.

Write `?` for positional parameters:

```zuri
import sql

var db = sql.open('sqlite://guide.db')

echo db.fetch_column(
  'select title from posts where author_id = ? and published = ?',
  [1, true]
)

db.close()
```

```console
[On Engines]
```

or `:name` for named ones, where order stops mattering and a name may
be used more than once:

```zuri
import sql

var db = sql.open('sqlite://guide.db')

echo db.fetch_column(
  'select title from posts where views > :floor and views < :ceiling',
  { floor: 100, ceiling: 1000 }
)

db.close()
```

```console
[On Engines, On Compilers]
```

`sql` rewrites these into whatever the adapter wants: `$1` and `$2` for
PostgreSQL, `?1` and `?2` for SQLite, and `?` unchanged for MySQL, which
already spells them that way. The scanner knows when a `?` or a `:` is
not a placeholder, so none of these are touched:

```zuri,ignore
"a ? inside a string"      -- a string
"a ? in an identifier"     -- a quoted identifier
-- a ? in a line comment
/* a ? in a block comment */
$tag$ a ? in a dollar-quoted body $tag$
value::text                -- a PostgreSQL cast
arr[1:3]                   -- an array slice
```

PostgreSQL uses `?` as a JSON operator, so a literal one is written
`??`:

```zuri,ignore
db.query('select * from docs where data ?? ?', ['key'])
```

> Never build a statement by joining strings around a value, even when
> the value looks safe. The two forms above cover every case where a
> **value** varies. Where an **identifier** varies, quote it through
> the driver rather than interpolating it:
>
> ```zuri,ignore
> var column = db.driver().quote_identifier(name)
> db.query('select ${column} from posts')
> ```

## Rows, Columns and Types

A row is a dictionary keyed by column name, which is what lets
`post.title` read the way it does.

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

Two columns of the same name collapse in a dictionary, which is worth
knowing before selecting `id` from both sides of a join. Aliasing one
of them is the fix; `tuples()` is the escape hatch when it cannot be.

Values correspond like this:

| Zuri | Database |
| --- | --- |
| `nil` | NULL |
| `bool` | a boolean where the engine has one, otherwise 0 and 1 |
| `number` | an integer where the value is whole, otherwise a float |
| `bigint` | a 64 bit integer, for values past what a double holds |
| `string` | text |
| `bytes` | a blob |
| `date.Date` | a timestamp |
| `sql.Time` | a `TIME` interval, on an engine that has one |
| `list`, `dict` | JSON |

Not every engine has every one of those. PostgreSQL has a real boolean
and MySQL does not, so a `true` written to MySQL comes back as `1`.
`db.supports()` answers that sort of question without guessing, and
each engine's own section below says where it differs.

An integer too large for a double comes back as a `bigint` rather than
silently rounded:

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

SQLite stores five things and remembers nothing about intent, so a
boolean goes in as 1 and would come back as the number 1. What it does
keep is the type each column was *declared* with, and that is what the
adapter reads it back by:

```zuri
import sql

var db = sql.open(':memory:')
db.exec('create table t (ok boolean, at datetime, doc json)')
db.exec('insert into t values (?, ?, ?)', [
  true, '2026-09-16T14:30:00.000000+00:00', '{"a":1}',
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

A column with no declared type is an expression, and comes back exactly
as it was stored.

### Exact decimals

A `number` is a double, which holds `0.1` only approximately. For money
that is not good enough, so `sql.Decimal` holds a value exactly:

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

PostgreSQL's `numeric` and MySQL's `DECIMAL` columns read and write as
`Decimal` automatically. SQLite has no exact decimal type, so store one
as text or as an integer count of the smallest unit.

## The CRUD Helpers

Four calls on a connection write no SQL at all. `insert()`, `update()`,
`delete()` and `find()` take a table name and dictionaries, build the
statement for whichever engine is on the other end, and bind every
value as a parameter.

They are here because the statements they replace are the ones least
worth writing by hand. They are mechanical, they differ between
dialects in small ways that only show up in production, and a string
built by concatenation is where an injection gets in. Anything harder
than a flat list of conditions is still written as SQL, and the two
mix freely on the same connection.

### Inserting and ids

`insert()` takes a table and a dictionary, and returns the new row's
id:

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

This is the call that hides the largest difference between the engines.
SQLite and MySQL report the id of the row just inserted; PostgreSQL does
not, and an insert that wants one has to ask with a `RETURNING` clause
naming the primary key. MariaDB has both and uses `RETURNING`.
`insert()` does whichever applies, and finds the key by asking the
schema rather than assuming it is called `id`.

Where the key is not the column to return, name it:

```zuri,ignore
db.insert('events', { name: 'started' }, { returning: 'uuid' })
```

### Several rows at once

Several rows go in one statement, split so that no single statement
binds more parameters than the engine allows:

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

Every row has to name the same columns. A row naming a different set
raises rather than being padded with nulls, because a missing column
and a null column mean different things.

### Finding rows

`find()` answers with a `ResultSet`, `count()` with a number, and
`find_one()` with the first row as a dictionary:

```zuri
import sql

var db = sql.open('sqlite://guide.db')

echo db.count('posts')
echo db.count('posts', { published: true })
echo db.count('posts', { author_id: [1, 2] })
echo db.count('posts', { author_id: [] })

echo db.find('posts', { published: true }, {
  columns: ['title'],
  order: ['views desc'],
  limit: 2,
}).column('title')

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

A `find_one()` that matches nothing answers `nil`. That is the answer,
not a failure, so nothing is raised for it:

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

### What a filter can say

A condition's value decides what it means. A plain value is equality, a
list is `IN`, an empty list matches nothing, and `nil` is `IS NULL`
rather than `= NULL`, which no row ever satisfies.

Several conditions in one dictionary are joined with `AND`. There is no
`OR`, which is the first thing to write as SQL:

```zuri
import sql

var db = sql.open('sqlite://guide.db')

echo db.count('posts', { published: true, author_id: 1 })
echo db.count('posts', { published: true, views: [87, 1290] })

db.close()
```

```console
1
1
```

Passing `nil` as the whole filter matches every row. That has to be
asked for rather than happening because a dictionary came out empty,
which is what stands between a filter built from user input and an
`UPDATE` with no `WHERE`.

### Ordering, limits and pages

`order` is a list of column names, each optionally followed by ` asc`
or ` desc`, applied in the order given:

```zuri
import sql

var db = sql.open('sqlite://guide.db')

echo db.find('posts', nil, { order: ['views desc'] }).column('title')
echo db.find('posts', nil, { order: ['author_id asc', 'views desc'] }).column('title')

db.close()
```

```console
[On Bugs, On Compilers, On Engines, On Looms]
[On Engines, On Looms, On Bugs, On Compilers]
```

`limit` and `offset` are whole numbers of rows, and together they are a
page. A page past the end is empty rather than an error:

```zuri
import sql

var db = sql.open('sqlite://guide.db')

def page(number, size) {
  return db.find('posts', nil, {
    columns: ['title'],
    order: ['views desc'],
    limit: size,
    offset: (number - 1) * size,
  }).column('title')
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

A limit and an offset go into the statement text rather than being
bound, because not every engine allows a parameter in either place.
That leaves them as the one part of a built statement that is not a
parameter, so each is checked before it goes in:

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

### Updating and deleting

`update()` and `delete()` return how many rows changed:

```zuri
import sql

var db = sql.open(':memory:')
db.exec('create table t (id integer primary key, n integer)')
db.insert_many('t', [{ n: 1 }, { n: 2 }, { n: 3 }])

echo db.update('t', { n: 0 }, { n: [1, 2] })
echo db.delete('t', { n: 0 })
echo db.count('t')

db.close()
```

```console
2
2
1
```

They read a filter exactly as `find()` does, `nil` included: passing it
changes or deletes every row.

### Values that are not values

Where a value is not a value, `sql.raw()` marks a fragment to be used
as written:

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

A `Raw` is accepted everywhere a column or a value is, which is what
makes an aggregate or a subquery reachable without leaving the
helpers:

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
> anything a user supplied is a SQL injection. Build them from
> literals, and keep values in the parameters where they belong.

### Inside a transaction

A `Transaction` carries all of them, and they mean the same thing
there. A read inside one sees what that transaction has written and
nobody else has committed yet:

```zuri
import sql

var db = sql.open(':memory:')
db.exec('create table t (id integer primary key, n integer)')

db.transaction(@(tx) {
  tx.insert_many('t', [{ n: 1 }, { n: 2 }, { n: 3 }])

  echo tx.count('t')
  echo tx.find('t', nil, { order: ['n desc'] }).column('n')

  tx.update('t', { n: 9 }, { n: 1 })
  tx.delete('t', { n: 3 })
})

echo db.find('t', nil, { order: ['n asc'] }).column('n')

db.close()
```

```console
3
[3, 2, 1]
[2, 9]
```

A rollback takes those rows with it, the reads included, and a
transaction that has already committed refuses them the way it refuses
everything else.

### Where they stop

These calls cover a flat list of equality conditions and stop there.
There is no join, no `OR` and no expression tree, because SQL is a
better language for those than any chain of method calls. A query that
wants one is a `query()` or a `fetch_all()` on the same connection,
next to the helpers rather than instead of them.

## Transactions

The closure form commits when the body returns and rolls back when it
raises, and there is no path through it that leaves a transaction open:

```zuri
import sql

var db = sql.open(':memory:')
db.exec('create table accounts (id integer primary key, balance integer)')
db.insert_many('accounts', [{ balance: 500 }, { balance: 0 }])

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

A failure undoes the whole thing:

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

A `transaction()` called inside another becomes a savepoint, so a
helper that opens one works the same whether it was called on its own
or from inside a larger piece of work. Only the outermost commits, and
a failure inside undoes just that inner piece:

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

Schema changes are part of the transaction on SQLite and PostgreSQL,
so a set of `create table` statements that fails half way leaves
nothing behind. MySQL and MariaDB commit the open transaction at every
`create`, `alter` and `drop`, and a `transaction()` whose body changes
the schema there raises `TransactionError` when it comes to commit.
`db.supports('transactional_ddl')` tells the two apart, so a program
that migrates its own schema can run each change inside a transaction
where that holds and on its own where it does not.

An isolation level can be asked for. Where an engine cannot provide one
it says so rather than quietly giving something weaker:

```zuri,ignore
db.transaction(@(tx) { ... }, sql.SERIALIZABLE)
```

| | |
| --- | --- |
| `sql.READ_UNCOMMITTED` | MySQL honours it; PostgreSQL accepts it and gives read committed |
| `sql.READ_COMMITTED` | everywhere |
| `sql.REPEATABLE_READ` | everywhere, and MySQL's default |
| `sql.SERIALIZABLE` | everywhere |

`begin()` opens one to be committed or rolled back by hand, for a
transaction whose lifetime is not a block. The closure form is safer
and should be preferred.

## Streaming Large Results

`query()` builds the whole result in memory, which is right for the
hundreds of rows most queries return and wrong for the millions some
do. `stream()` reads the same result a row at a time:

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

Running to the end closes the cursor. A loop that stops early does not,
so anything that might `break` has to close it:

```zuri,ignore
var cursor = db.stream('select * from events')

for event in cursor {
  if done(event) {
    break
  }
}

cursor.close()
```

`take(n)` reads a batch at a time, for work that batches naturally:

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

On PostgreSQL this is a server-side portal and on MySQL a server-side
cursor, both fetched in batches that `{ batch: n }` sizes. On SQLite it
is the engine's own behaviour: rows are computed as they are asked for,
so nothing is held but the current one.

A server-side cursor holds its connection while it is open, since the
server is part way through answering. Running another statement on that
connection before the cursor is read to the end or closed raises rather
than letting the exchange fall out of step.

## Prepared Statements

Compiling a statement is the expensive half of running one, and a
statement run in a loop should pay for it once:

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

The placeholders are translated once, when the statement is prepared,
so the loop does no string work at all. A prepared statement carries
the same query and fetch methods a connection does.

## Connection Pools

Opening a connection is expensive: for PostgreSQL and MySQL it is a TCP
connection, a TLS handshake and an authentication exchange before the
first statement runs. A server that opened one per request would spend
most of its time connecting.

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

Used that way the pool takes a connection, runs the statement and gives
it back. Where several statements have to run on the same connection,
which a transaction requires, `with_connection()` holds one and
releases it whatever happens:

```zuri,ignore
db.with_connection(@(connection) {
  connection.transaction(@(tx) {
    tx.exec('...')
  })
})
```

`transaction()` on the pool does the same in one call.

| Setting | |
| --- | --- |
| `max` | most connections to open. Ten by default |
| `min` | how many to open up front. None by default |
| `idle_timeout` | how long an idle connection is kept |
| `max_lifetime` | how long any connection is kept before being replaced |
| `validate_on_acquire` | check a connection is alive before lending it |
| `on_connect` | run something on each new connection |

A pool wants to be as small as the work allows. Connections are not
free at the other end either, and a pool larger than the database can
usefully serve turns a queue in the application into a queue in the
database, where it is harder to see.

A pool belongs to the isolate that made it, which has two consequences.
An isolate that needs database access makes its own. And there is no
waiting when a pool is empty: nothing else can release a connection
while the call is running, so running out is reported rather than
waited on.

## Errors

Every error is a `sql.SqlError`. Catch that to catch anything a
database can do; the subclasses separate what is worth handling
differently.

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

The classes are the same on every engine, which is the point. A unique
constraint is SQLSTATE `23505` on PostgreSQL, error number `1062` on
MySQL, and extended result code `2067` on SQLite; no one of those means
anything to the others, and code matching on any of them would stop
working the moment the adapter changed.

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

The engine's own account is kept rather than thrown away. `code` holds
what the engine said, `sqlstate` the five character code where there is
one, and `query` the statement that failed:

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

`SerializationError` and `DeadlockError` are the two worth retrying:
both mean the engine gave up on a transaction to keep its promises, and
running the whole transaction again usually succeeds.

## Schema Introspection

`db.schema` answers questions about what is in the database. The
queries underneath are entirely different per engine, and the answers
are the same shape.

Every method takes an optional schema to look in. PostgreSQL has
schemas inside a database and resolves the default through
`search_path`; MySQL calls a database a schema and has no layer above
it, so naming one there names a database; SQLite has neither and
ignores the argument.

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

Each column is described the same way whichever engine answered:

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

`indexes()` and `foreign_keys()` describe the rest:

```zuri
import sql

var db = sql.open('sqlite://guide.db')

echo db.schema.foreign_keys('posts')

db.close()
```

```console
[{columns: [author_id], references_table: authors, references_columns: [id], on_update: NO ACTION, on_delete: NO ACTION}]
```

This is also what makes `insert()` portable: on an engine with no last
insert id the insert needs a `RETURNING` clause, and the column to
return is the table's primary key, which is asked for here.

One caution when moving a schema between engines: MySQL parses a
column level `references` clause and then ignores it, so a foreign key
declared that way exists on the other two and not there. Declared at
table level it exists on all three.

## Switching Databases

The claim this module makes is that a program moves between engines by
changing its connection string. Here is that claim, as a program:

```zuri
import sql

def report(db) {
  db.exec_script('drop table if exists tally')
  db.exec_script(
    'create table tally (id ' + key_type(db) + ' primary key, word text not null)'
  )

  db.insert_many('tally', [{ word: 'alpha' }, { word: 'beta' }])

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

The same `report()` runs against PostgreSQL or MySQL by opening a
different connection, and prints the same list.

Three things did have to be written twice, and they are the three that
are genuinely different:

- **The connection string**, which is the point.
- **An auto-incrementing key**, which is not standard SQL. PostgreSQL
  spells it `serial`, MySQL `auto_increment`, SQLite `integer primary
  key`.
- **Any SQL only one of them has.** `sql` does not parse or rewrite
  statements beyond their placeholders, so a PostgreSQL array operator,
  a MySQL `on duplicate key update` or a SQLite `json_extract` stays
  what it is.

`db.supports()` is how a program asks rather than assumes:

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

## SQLite Specifics

`db.native()` reaches the adapter underneath, where the engine's own
features live.

A SQLite database is a file, and `:memory:` is one that is not. An
in-memory database belongs to the connection that opened it, so a
second connection to `:memory:` is a second, empty database rather than
another handle on the same data.

Foreign keys are enforced. SQLite leaves them unenforced by default,
per connection, for compatibility with databases written before it had
them; a database that declares them almost certainly means them, so the
adapter turns them on. Pass `foreign_keys: false` to turn them back
off.

### Functions written in Zuri

A function registered on a connection runs inside the engine, once per
row, and can be used anywhere an expression can:

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

An aggregate is written as a fold: `step` is called once per row with
the accumulator and the row's arguments and returns the next
accumulator, and `finish` turns the last accumulator into the group's
value.

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

A collation is an ordering for text, reached with `collate`:

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

> A function must not touch the connection it was registered on.
> SQLite is in the middle of a statement when it calls, and reentering
> would deadlock. An error raised inside one is carried out and
> re-raised with its own class intact once the statement has finished.

### Blobs, backups and hooks

A large value read with a `select` arrives all at once. A blob handle
opens one cell and reads windows of it:

```zuri,ignore
var blob = db.native().blob('main', 'files', 'content', id, false)

var at = 0
while at < blob.length() {
  out.write(blob.read(at, 65536.min(blob.length() - at)))
  at += 65536
}

blob.close()
```

The cell has to exist and already be the right size: SQLite cannot grow
a blob this way, which is what inserting `zeroblob(n)` is for.

Copying the file with the filesystem is only safe when nothing is
writing. SQLite's own backup copies page by page and notices when the
source changes underneath:

```zuri,ignore
db.native().backup_to('./snapshot.db')
```

And the hooks report what the engine is doing:

```zuri,ignore
db.native().on_change(@(operation, database, table, rowid) { ... })
db.native().on_commit(@() => true)
db.native().on_rollback(@() { ... })
db.native().set_authorizer(@(action, first, second, database, trigger) { ... })
db.native().on_progress(1000, @() => keep_going())
```

## PostgreSQL Specifics

The adapter speaks version 3 of the wire protocol, over TCP or over a
unix domain socket, optionally under TLS.

A `host` beginning with a slash is a socket directory, which is how
libpq spells it, and the socket inside is named after the port:
`/var/run/postgresql` with port 5432 means
`/var/run/postgresql/.s.PGSQL.5432`. A path that already names the
socket is taken as given. TLS is neither offered nor wanted over a
socket, since nothing sits in between, so `sslmode` is ignored there.

```zuri,ignore
sql.open('postgres:///app?host=/var/run/postgresql')
sql.open({ driver: 'postgres', host: '/var/run/postgresql', database: 'app' })
```

`sslmode` chooses how TLS is used: `disable` never, `prefer` when the
server offers it, and `require` always, failing when the server
refuses. Only `require` verifies who is on the other end.

Statements go through the extended protocol, which compiles them on the
server and binds values rather than substituting them. Values travel in
the server's own binary form wherever the adapter has a codec for the
type, and as text otherwise, so a type it has never heard of still
arrives readable rather than as bytes.

`numeric` columns arrive as `sql.Decimal`, arrays as lists, `jsonb` as
dictionaries, and timestamps as `date.Date`.

### Listening for notifications

`LISTEN` and `NOTIFY` are PostgreSQL's own publish and subscribe. A
connection that has run `LISTEN channel` receives a message whenever
anything anywhere runs `NOTIFY channel`.

```zuri,ignore
var listener = db.native().listener()

listener.listen('jobs')

while true {
  for message in listener.wait(nil) {
    handle(message.channel, message.payload)
  }
}
```

Notifications arrive between other messages, so a connection busy
running statements collects them as it goes and `poll()` hands over
whatever has accumulated.

### Teaching it a type

A database that defines its own types can teach the adapter about them:

```zuri,ignore
import sql.postgres { DEFAULT_REGISTRY }

DEFAULT_REGISTRY.register(oid, decoder, encoder)
```

Until it is taught, such a column arrives as text, which is correct if
unexciting.

## MySQL Specifics

The adapter speaks the client/server protocol directly, over TCP or
over a unix domain socket, optionally under TLS, with no client library
underneath it. MariaDB speaks the same protocol and the same adapter
drives it.

A `socket` option names a socket, and so does a `host` beginning with a
slash. Unlike PostgreSQL the path names the socket itself rather than
the directory holding it:

```zuri,ignore
sql.open({ driver: 'mysql', socket: '/var/run/mysqld/mysqld.sock', user: 'app' })
sql.open('socket=/var/run/mysqld/mysqld.sock user=app database=shop')
sql.open('mysql://app@localhost/shop?socket=/var/run/mysqld/mysqld.sock')
```

TLS is neither offered nor wanted there, since nothing sits in between.
The connection does count as private, which is what lets
`caching_sha2_password` and `sha256_password` send the password itself
rather than encrypting it to the server's public key.

A statement with no values is sent as text, which is one round trip. A
statement with values is prepared, so the values travel in the server's
binary encoding instead of being written into the SQL, and the question
of quoting never arises. Compiled statements are kept and reused, so a
query run in a loop is compiled once.

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

MySQL keeps `?` as it is written, and reports the id of an inserted row
rather than returning it, so `insert()` reads the reported id instead of
adding a `RETURNING` clause. MariaDB differs in two ways that change
what the layer above generates, which is why `mariadb://` is a scheme
of its own: it has `RETURNING`, and it has no JSON type.

### What arrives from where

`DECIMAL` columns arrive as `sql.Decimal`, `JSON` as lists and
dictionaries, `DATE` and `DATETIME` and `TIMESTAMP` as `date.Date`, and
`TIME` as `sql.Time`.

MySQL has no boolean. `BOOLEAN` is another name for `TINYINT(1)`, so a
value written as `true` comes back as `1`, and `db.supports('booleans')`
is false to say so.

`BLOB` and `TEXT` share a type on the wire and are told apart only by
the column's collation, which the adapter reads: a binary column
arrives as `bytes` and a text one as a string. `BIT` and `GEOMETRY`
arrive as `bytes`, having no shape Zuri could represent without
inventing one. `SET` arrives as the comma separated text the server
stores rather than as a list, because a list written back would be
encoded as JSON and the column would quietly stop matching.

A date MySQL considers absent is written as zeros, and `0000-00-00` is
not a date any calendar has. It arrives as `nil`.

### A TIME is a span, not a clock

A `TIME` column runs from `-838:59:59.999999` to `838:59:59.999999`. It
is a duration rather than a point in the day, so it can be negative and
can exceed twenty four hours, and neither of those would survive being
read into a `date.Date`. `sql.Time` holds it:

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

A `DATETIME` carries no zone, and a `TIMESTAMP` is converted to and
from whatever zone the session is in. So the session's zone decides
what a timestamp means, and leaving it to the server's configuration
would make the same database read differently from two machines.

The adapter sets the session to UTC when it connects. Every timestamp
then arrives as UTC, and a `date.Date` written back is converted from
whatever offset it carries. To leave the server's own setting alone,
pass `time_zone` as `nil`, or name a zone to use instead:

```zuri,ignore
sql.open('mysql://localhost/app', { time_zone: nil })
sql.open('mysql://localhost/app', { time_zone: '+01:00' })
```

### Authentication

`caching_sha2_password`, which is the default from MySQL 8.0 onwards,
`mysql_native_password`, `sha256_password`, `mysql_clear_password`, and
MariaDB's `client_ed25519`.

Two of those need the password itself rather than a proof of it, the
first time an account authenticates. Over TLS the password is sent as
it is. Over a plain connection it is encrypted to a public key the
server hands over first, so it is never readable in transit.
`mysql_clear_password` has no such fallback and is refused outright on
a connection that is not private, since it sends the password with
nothing protecting it.

### TLS

`sslmode` chooses how TLS is used:

| Mode | Meaning |
| --- | --- |
| `disable` | Never. |
| `prefer` | When the server offers it, without checking who the server is. The default. |
| `require` | Always, without checking who the server is. |
| `verify` | Always, checking the server's certificate and host name. |

MySQL's own spellings are accepted too. `DISABLED`, `PREFERRED` and
`REQUIRED` mean what they say, and both `VERIFY_CA` and
`VERIFY_IDENTITY` become `verify`. That makes `VERIFY_CA` stricter here
than on the command line, where it checks the certificate but not the
name: the difference can only cause a connection to be refused, never
one to be wrongly trusted.

`ssl_ca` names a certificate authority to trust, and `ssl_cert` with
`ssl_key` present a client certificate.

### Compression

The protocol can compress everything after the handshake:

```zuri,ignore
sql.open('mysql://localhost/app', { compression: 'zlib' })
sql.open('mysql://localhost/app', { compression: 'zstd' })
```

It is worth it for large results over a slow link and costs more than
it saves on a local socket, so it is off unless asked for. A packet too
small to benefit is sent uncompressed regardless, which the protocol
allows for.

### Sending a file

`LOAD DATA LOCAL INFILE` has the server name a file and the client send
it. The file is read from the machine the program runs on, chosen by
the server, so a hostile or compromised server could ask for anything
the process can read. It is refused unless a reader is supplied:

```zuri,ignore
import os

var db = sql.open('mysql://localhost/app', {
  local_infile: @(name) {
    return os.file(name).read()
  },
})
```

The reader decides what may be sent, which is where that decision
belongs. Refusing still answers the server, so the connection stays
usable afterwards rather than waiting for a file that will never come.

### Statements that answer more than once

A stored procedure sends one result set per `select` inside it, then an
OK packet. `query()` hands back the first and reads the rest, because
leaving them unread would not lose them: it would give them to the next
statement, which would then answer with this one's rows.

```zuri,ignore
var first = db.query('call two_results()')
```

A routine's body has semicolons in it, and `exec_script()` splits a
script on semicolons. So a `create procedure` goes through `exec()` as
a single statement rather than through `exec_script()`.

### Reaching the adapter

```zuri,ignore
db.native().flavor()          # 'mysql' or 'mariadb'
db.native().connection_id()   # what KILL names
db.native().parameters()      # what the server said about itself
db.native().reset()           # back to a fresh session
```

## Writing Your Own Adapter

An adapter for another engine implements four things: a `Driver` that
reads a connection string and opens a connection, a `DriverConnection`
that runs statements, a `DriverStatement`, and a `DriverCursor`.

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

Everything above the adapter comes for free: pooling, transactions and
savepoints, the CRUD helpers, placeholder translation, cursors, and the
error hierarchy. What the adapter supplies is the engine.

The base classes raise `NotImplementedError` for anything left out, so
an unfinished adapter fails where the gap is. An engine that genuinely
cannot do something raises `NotSupportedError` instead, which reads
differently on purpose: the first is an unfinished adapter, the second
is an honest limit.

## What the Module Refuses

**It is not an ORM.** There is no model class, no lazy loading and no
identity map. Rows are dictionaries.

**It does not write your joins.** The CRUD helpers cover a flat list of
equality conditions. Anything past that is SQL.

**It does not migrate schemas.** `exec_script()` runs a schema file and
`db.schema` reports what is there; deciding what to change and in what
order is a separate problem.

**It does not translate SQL.** Placeholders are rewritten. Statements
are not parsed, and a function only one engine has stays a function
only one engine has.

**It does not hide a difference by guessing.** Where an engine cannot
do what was asked, the answer is `NotSupportedError` naming the adapter
and the feature, rather than an approximation nobody asked for.

## Module Reference

The standard library reference documents every class and method. The
shape of the module:

| | |
| --- | --- |
| `sql.open(dsn, options)` | opens a connection |
| `sql.pool(dsn, options)` | opens a pool of them |
| `sql.register(driver)` | adds an adapter |
| `sql.drivers()` | what is registered |
| `sql.raw(fragment)` | marks SQL to be used as written |
| `sql.Decimal(text)` | an exact decimal |
| `sql.time(h, m, s)` | a signed span, as a `TIME` column holds one |
| `sql.parse_time(text)` | the same, read from text |

On a `Connection`:

| | |
| --- | --- |
| `query`, `exec`, `exec_script` | run a statement |
| `fetch_one`, `fetch_all`, `fetch_value`, `fetch_column` | read a result |
| `stream`, `prepare` | a cursor, and a compiled statement |
| `insert`, `insert_many`, `update`, `delete`, `find`, `find_one`, `count` | the four statements that are always the same |
| `transaction`, `begin`, `in_transaction` | transactions |
| `schema` | introspection |
| `driver`, `driver_name`, `capabilities`, `supports` | what this engine is |
| `native` | the adapter underneath |
| `ping`, `close`, `is_closed` | the connection itself |

On a `Transaction`:

| | |
| --- | --- |
| `query`, `exec` | run a statement |
| `fetch_one`, `fetch_all`, `fetch_value` | read a result |
| `insert`, `insert_many`, `update`, `delete`, `find`, `find_one`, `count` | the same helpers a connection has |
| `commit`, `rollback`, `is_finished` | ending it |
| `connection`, `nested` | what it is running on, and whether it is a savepoint |
