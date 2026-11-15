# Models

`sql` works with rows, and a row is a dictionary. That's the right shape for
a report or a script, but not for the core of an application, where a post
is something with rules: it must have a title, it belongs to an author, and
its slug is derived from its title. `sql.model` gives each table a class.
Instances of the class, called records, can save, validate and delete
themselves, and can load the records they're associated with.

- [A First Model](#a-first-model)
- [Declaring a Model](#declaring-a-model)
- [Records](#records)
- [Assigning Input](#assigning-input)
- [Validation](#validation)
- [Hooks](#hooks)
- [Queries](#queries)
- [Associations](#associations)
- [Transactions](#transactions)
- [Casts](#casts)
- [Records as Output](#records-as-output)
- [Design Limits](#design-limits)

## A First Model

A model is a class that extends `Model`. Its fields are the columns of its
table:

```zuri
import sql
import sql { Blueprint }
import sql.model { Model }

class Post < Model {
  var id
  var title
  var views = 0
}

var db = sql.open(':memory:')

db.schema.create_table('posts', @(table: Blueprint) {
  table.id()
  table.string('title', 200)
  table.integer('views').default_value(0)
})

var post = Post({ title: 'On Engines' })
post.save(db)

echo post.id

post.views = 412
post.save(db)

var found = db.model(Post).find(post.id)
echo '${found.title}: ${found.views}'

db.close()
```

```console
1
On Engines: 412
```

`db.model(Post)` starts a query over the model's rows. Methods that use the
database, such as `save(db)` and `delete(db)`, take the connection to use as
an argument. A record never holds onto a connection itself, so it's saved
inside whichever transaction the calling code opened, and a record that's
kept after its connection has gone back to a pool can't write through a
connection that now belongs to someone else.

## Declaring a Model

Every field the class declares, apart from those that start with `_`, is a
column with the same name. Everything else about the model goes in a static
`define()` method, which receives a definition object to describe the model
on:

```zuri,ignore
class Post < Model {
  var id
  var author_id
  var title
  var published = false
  var created_at
  var updated_at

  var author # holds the association, once loaded

  static define(model: Definition) {
    model.table('articles') # the plural of the class otherwise
    model.primary_key('id') # id otherwise
    model.timestamps() # created_at and updated_at
    model.cast('published', 'bool')
    model.belongs_to('author', Author)
    model.validate('title', validate.required().string().max_length(200))
    model.fillable(['title', 'published'])
    model.hidden(['internal_notes'])
    model.transient(['preview']) # fields that are not columns
    model.scope('published', @(query, _) => query.where({ published: true }))
  }
}
```

A model's table name is its class name in snake case, made plural using the
regular rules of English, so `Post` becomes `posts`, `BlogPost` becomes
`blog_posts`, `Category` becomes `categories` and `Address` becomes
`addresses`. If a word has an irregular plural, set the table name with
`table()`.

`define()` runs once, the first time the model is used, not when the class
is declared, so it can refer to models declared later in the same file. Its
description is checked against the class at that point. An association
without a field to load into, or a cast on a field that isn't a column,
raises a `ModelError` the first time the model is used. A model's primary
key is a single column.

Records loaded from the database are created without any arguments, so a
model's own `@new` mustn't require any. If it accepts attributes, it should
pass them on with `parent(attributes)`.

## Records

Calling the class creates a record, using a dictionary of attributes if you
pass one. `create()` on a query creates a record and saves it:

```zuri,ignore
var draft = Post({ title: 'On Looms' })
var saved = db.model(Post).create({ title: 'On Bugs' })
```

Saving a new record inserts it. Any column that's still `nil` is left out of
the insert, so the database fills in its default, and those columns are then
read back, so the record holds what was actually stored. Saving a record
that's already stored only writes the columns that have changed since it was
loaded, and writes nothing if none have. That way, a column that another
process changed in the meantime isn't overwritten when you save a different
column.

```zuri
import sql
import sql { Blueprint }
import sql.model { Model }

class Post < Model {
  var id
  var title
  var views
  var tags = []
}

var db = sql.open(':memory:')

db.schema.create_table('posts', @(table: Blueprint) {
  table.id()
  table.string('title', 200)
  table.integer('views').default_value(0)
  table.json('tags').default_value([])
})

var post = db.model(Post).create({ title: 'On Engines' })
echo post.views

post.title = 'On Analytical Engines'
post.tags.append('history')

echo post.changes()
echo post.is_dirty('views')
echo post.original('title')

post.save(db)
echo post.is_dirty(nil)

post.title = 'not saved'
post.reload(db)
echo post.title

post.delete(db)
echo post.is_persisted()

db.close()
```

```console
0
{title: On Analytical Engines, tags: [history]}
false
On Engines
false
On Analytical Engines
false
```

A list or dictionary that's changed in place counts as a change. A deleted
record keeps its values, and saving it again inserts it as a new row.
`key()` returns the value of the primary key, whatever the column is called.

## Assigning Input

`fill()` assigns attributes from a dictionary, such as one provided by a
form or a request body. It only assigns fillable columns, which are the
columns the model lists with `fillable()`, or if it doesn't list any, every
column except the primary key and the timestamps. A key that isn't fillable
raises a `ModelError`, so a request can't set a record's primary key, or any
other column it was never meant to change, just by sending an extra field.

The second argument lists the keys to use and ignores the rest. Use it with
input that contains more than the record should accept:

```zuri,ignore
post.fill(request.form(), ['title', 'body'])
```

## Validation

A model validates itself before every save, using `validate` rules,
uniqueness checks and its own `validate()` method. A record that fails
validation isn't saved, and `save()` raises a `RecordInvalid` error:

```zuri
import sql
import sql { Blueprint }
import sql.model { Definition, Model, RecordInvalid }
import validate

class Author < Model {
  var id
  var name
  var born
  var died

  static define(model: Definition) {
    model.validate('name', validate.required().string().max_length(80))
    model.unique('name')
  }

  validate(errors) {
    if self.born != nil and self.died != nil and self.died < self.born {
      errors.add('died', 'An author dies after being born.')
    }
  }
}

var db = sql.open(':memory:')

db.schema.create_table('authors', @(table: Blueprint) {
  table.id()
  table.string('name', 80).unique()
  table.integer('born').nullable()
  table.integer('died').nullable()
})

db.model(Author).create({ name: 'Ada', born: 1815, died: 1852 })

for attempt in [
  { name: 'Ada' },
  { name: 'Grace', born: 1906, died: 1900 },
  { name: '' },
] {
  catch {
    db.model(Author).create(attempt)
  } as error {
    echo error.errors[0].message
  }
}

echo Author({ name: 'Grace' }).is_valid(db)

db.close()
```

```console
The name field has already been taken.
An author dies after being born.
The name field is required.
true
```

`RecordInvalid` is a `validate.ValidationError`, and its `errors` are the
same `{ field, message }` list that form validation reports, so the code
that responds to an invalid form with a 422 can respond to an invalid record
in the same way. `is_valid(db)` runs the checks without saving, and
`errors()` returns what the last check found.

A uniqueness check reports a value that's already taken along with the other
failures. However, only a unique index in the database can stop two records
that are saved at the same moment from getting the same value, so a unique
column should have a unique index in the database as well.

## Hooks

A model can override any of these methods to run code at that point in a
record's life cycle. Each one receives the connection or transaction the
record is using:

| Hook | Runs |
| --- | --- |
| `before_validate(db)` | Before the record is validated |
| `validate(errors)` | While it's validated, to add failures of the model's own |
| `after_validate(db)` | After it's validated, whatever the result |
| `before_save(db)` | Before it's written, whether it's new or not |
| `before_create(db)` | Before a new record is inserted |
| `after_create(db)` | After it's inserted, once it has its key |
| `before_update(db)` | Before a stored record's changes are written |
| `after_update(db)` | After the changes are written |
| `after_save(db)` | After it's written, whether it's new or not |
| `before_delete(db)` | Before it's deleted |
| `after_delete(db)` | After it's deleted |
| `after_load()` | When a record is created from a row |

If a model defines any hooks, saving and deleting run inside a transaction,
or a savepoint if a transaction is already open, so anything a hook writes
is undone along with everything else if something fails. A hook can raise an
error to stop the save or the delete:

```zuri
import sql
import sql { Blueprint }
import sql.model { Model }

class Post < Model {
  var id
  var title
  var slug

  before_save(db) {
    self.slug = self.title.lower().replace(' ', '-')
  }

  after_create(db) {
    db.insert('audit', { what: 'created ${self.slug}' })

    if self.title == 'Refused' {
      raise Error('not this one')
    }
  }
}

var db = sql.open(':memory:')

db.schema.create_table('posts', @(table: Blueprint) {
  table.id()
  table.string('title', 200)
  table.string('slug', 200)
})

db.schema.create_table('audit', @(table: Blueprint) {
  table.text('what')
})

echo db.model(Post).create({ title: 'On Engines' }).slug

catch {
  db.model(Post).create({ title: 'Refused' })
} as _error {}

echo db.model(Post).count()
echo db.fetch_column('select what from audit', nil, nil)

db.close()
```

```console
on-engines
1
[created on-engines]
```

A model's methods share a namespace with the hooks and with every method a
record has, so a model shouldn't declare a method with one of those names
for any other purpose. A field can have the same name as any of them.

## Queries

`db.model(Post)` returns a `Query`. Every method that narrows a query
returns a new query and leaves the original unchanged, so you can keep a
query and build on it in several different directions:

```zuri
import sql
import sql { Blueprint }
import sql.model { Definition, Model }

class Post < Model {
  var id
  var title
  var views
  var published

  static define(model: Definition) {
    model.cast('published', 'bool')
    model.scope(
      'popular',
      @(query, least) => query.where('views >= ?', [least]),
    )
  }
}

var db = sql.open(':memory:')

db.schema.create_table('posts', @(table: Blueprint) {
  table.id()
  table.string('title', 200)
  table.integer('views')
  table.boolean('published')
})

db.insert_many('posts', [
  { title: 'On Engines', views: 412, published: true },
  { title: 'On Looms', views: 87, published: false },
  { title: 'On Bugs', views: 1290, published: true },
  { title: 'On Compilers', views: 640, published: true },
])

var published = db.model(Post).where({ published: true })

echo published.count()
echo published.order('views desc').limit(2).pluck('title')
echo published.scope('popular', 500).pluck('title')
echo db.model(Post).where({
  title: ['On Looms', 'On Bugs'],
}).count()
echo db.model(Post).where_not({ published: true }).first().title
echo db.model(Post).sum('views')
echo db.model(Post).order('id').page(2, 3).records.map(@(post) => post.title)

db.close()
```

```console
3
[On Bugs, On Compilers]
[On Bugs, On Compilers]
2
On Looms
2429
[On Compilers]
```

`where()` with a dictionary works the same way as the conditions for the
CRUD helpers: a value matches itself, `nil` matches null, a list matches any
of its values, and an empty list matches nothing. Anything other than a
dictionary is a fragment of SQL, with a `?` for each value. The fragment is
wrapped in parentheses when it's added, so an `or` inside it stays inside
it. All conditions must be true. `where_not()` excludes matching rows, and a
row whose column is null counts as excluded by any value, since null isn't
equal to anything.

Here's what a query can do with the rows it matches:

```zuri,ignore
query.all()               # every record
query.first()             # the first, by primary key when unordered, or nil
query.find(7)             # by primary key, or RecordNotFound
query.find_by({ slug: 'on-engines' })   # the first matching, or nil
query.exists()            # whether any row matches
query.count()             # how many, whatever the order, limit and offset
query.pluck('title')      # one column, cast as the model casts it
query.sum('views')        # and min, max and average
query.page(2, 20)         # { records, total, page, size, pages }
query.stream()            # records one at a time, for more than fit in memory
query.create({ title: 'x' })            # builds and saves one
query.update({ views: sql.raw('views + 1') })   # every matching row, at once
query.delete()            # every matching row, at once
query.from_sql('select posts.* from posts join ...', [...])
query.to_sql()            # { sql, values }, as all() would run it
```

A query is iterable, so `for post in query` reads every record. `update()`
and `delete()` change every matching row in a single statement without
creating records, so no validation or hooks run. `update()` sets the
`updated` timestamp unless you provide a value for it.

A scope is a reusable part of a query. You declare it with `scope()` and
apply it with `query.scope(name, argument)`.

## Associations

There are four kinds of association, which cover the ways tables can refer
to each other:

| Declared as | Means | The key is |
| --- | --- | --- |
| `belongs_to('author', Author)` | This record holds another record's key | `author_id` in this table |
| `has_one('profile', Profile)` | Another record holds this one's key, and there's at most one | `author_id` in the other table |
| `has_many('posts', Post)` | Other records hold this one's key | `author_id` in the other table |
| `belongs_to_many('tags', Tag)` | A join table holds both keys | `post_id` and `tag_id` in `post_tag` |

Each name is also the field the association is loaded into, which the class
has to declare. Keys are named after the class name in snake case, followed
by `_id`, and the join table is named after both class names in alphabetical
order. You can use options to choose different names:

```zuri,ignore
model.belongs_to('writer', Author, {
  foreign_key: 'written_by',
  owner_key: 'id',
})
model.has_many('posts', Post, { foreign_key: 'written_by', local_key: 'id' })
model.belongs_to_many('tags', Tag, {
  through: 'taggings',
  foreign_key: 'post_id',
  other_key: 'tag_id',
})
```

Associations are only loaded when you ask for them. `include()` loads them
along with the records, using one query per association however many records
there are, and a dotted name loads an association of an association:

```zuri
import sql
import sql { Blueprint }
import sql.model { Definition, Model }

class Author < Model {
  var id
  var name
  var posts

  static define(model: Definition) {
    model.has_many('posts', Post)
  }
}

class Post < Model {
  var id
  var author_id
  var title
  var author
  var tags

  static define(model: Definition) {
    model.belongs_to('author', Author)
    model.belongs_to_many('tags', Tag)
  }
}

class Tag < Model {
  var id
  var name
}

var db = sql.open(':memory:')

db.schema.create_table('authors', @(table: Blueprint) {
  table.id()
  table.string('name', 80)
})

db.schema.create_table('posts', @(table: Blueprint) {
  table.id()
  table.references('author_id', 'authors')
  table.string('title', 200)
})

db.schema.create_table('tags', @(table: Blueprint) {
  table.id()
  table.string('name', 40)
})

db.schema.create_table('post_tag', @(table: Blueprint) {
  table.references('post_id', 'posts')
  table.references('tag_id', 'tags')
  table.primary(['post_id', 'tag_id'])
})

var ada = db.model(Author).create({ name: 'Ada' })

var engines = Post({ title: 'On Engines' })
engines.author = ada
engines.save(db)

db.model(Post).create({ title: 'On Looms', author_id: ada.id })

var history = db.model(Tag).create({ name: 'history' })
var machines = db.model(Tag).create({ name: 'machines' })

engines.attach(db, 'tags', [history, machines])

var authors = db.model(Author).include('posts.tags').all()

for post in authors[0].posts {
  echo '${post.title}: ${post.tags.map(@(tag) => tag.name)}'
}

echo engines.related(db, 'tags').where({ name: 'history' }).count()
echo engines.sync(db, 'tags', [machines])
echo db.model(Post).find(engines.id).load(db, 'author').name

db.close()
```

```console
On Engines: [history, machines]
On Looms: []
1
{attached: [], detached: [1]}
Ada
```

A field that holds an association is `nil` until the association is loaded.
`load()` loads an association onto a record you already have and returns the
record, and `query.load(records, names)` loads associations onto a list of
records, using one query per association. `related()` returns a query over
the records an association refers to, so you can narrow it further instead
of loading all of them.

When you assign a record to a `belongs_to` field, the key that refers to it
is set when the record holding it is saved, and the assigned record has to
be saved first. `attach()`, `detach()` and `sync()` link records through a
`belongs_to_many` association's join table, and accept either records or
keys.

## Transactions

`tx.model(Post)` runs a query inside a transaction, and a record saved with
the transaction is saved inside it:

```zuri,ignore
db.transaction(@(tx) {
  var post = tx.model(Post).find(7)

  post.views += 1
  post.save(tx)

  tx.model(Audit).create({ what: 'viewed ${post.id}' })
})
```

## Casts

Most values are read back as the type they were stored as. A cast makes sure
a field has the same type on every engine, even where the engines differ, by
converting it as it's read:

| Cast | The field holds |
| --- | --- |
| `bool` | `true` or `false`, converted from a bool, a number, `'0'`, `'1'`, `'true'` or `'false'` |
| `integer` | A whole number |
| `number` | A number |
| `decimal` | A `sql.Decimal` |
| `string` | Text |
| `json` | A list or dictionary, decoded from JSON text if necessary |
| `date` | A `date.Date`, converted from ISO 8601 text if necessary |

MySQL doesn't have a boolean type and returns `1`, and MariaDB stores JSON
as text, so a model that runs on either of them should cast those columns,
for example with `cast('published', 'bool')` and `cast('settings', 'json')`.
Null stays `nil` under every cast, and a value that can't be converted is
left as it was read instead of being lost.

## Records as Output

`to_dict()` returns a record's columns, except for the ones listed with
`hidden()`, along with every association that has been loaded, converted to
dictionaries as well. `json.encode()` writes a record as its `to_dict()`, so
a record passed to a response doesn't include something like a password
hash:

```zuri
import json
import sql
import sql { Blueprint }
import sql.model { Definition, Model }

class User < Model {
  var id
  var name
  var password_hash

  static define(model: Definition) {
    model.hidden(['password_hash'])
  }
}

var db = sql.open(':memory:')

db.schema.create_table('users', @(table: Blueprint) {
  table.id()
  table.string('name', 80)
  table.string('password_hash', 120)
})

var user = db.model(User).create({ name: 'Ada', password_hash: 'x' })
var again = db.model(User).find(user.id)

echo json.encode(user)
echo user == again
echo user.to_string()

db.close()
```

```console
{"id":1,"name":"Ada"}
true
<User 1>
```

Two records are equal if they belong to the same model and have the same
primary key. A record that hasn't been saved yet is only equal to itself.

## Design Limits

**Associations are loaded explicitly.** An association is only loaded by
`include()` or `load()`, never just by reading its field, so a loop over
records never silently runs a query for each record.

**Each query creates its own records.** If two queries read the same row,
you get two separate records for it, which are equal (`==`) to each other.
Nothing is cached between queries, so a record always reflects what the
database held when it was read.

**Joins are written in SQL.** Conditions cover a model's own columns, and
`related()` and `include()` cover associations. For a join, a union or a
window function, write the SQL yourself and pass it to `from_sql()`, which
creates records from whatever the query selects.

**Tables are created by migrations.** A model maps an existing table.
[Migrations](ch17-02-migrations.md) and [Changing the
Schema](ch17-01-schema-changes.md) are how you create and change tables.
