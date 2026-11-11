# Models

`sql` deals in rows, and a row is a dictionary. That is the right shape
for a report or a script, and the wrong one for the heart of an
application, where a post is something with rules: a title it must
have, an author it belongs to, a slug that follows from its title.
`sql.model` gives each table a class. Its records save, check and delete
themselves, and reach the records they are associated with.

- [A First Model](#a-first-model)
- [Declaring a Model](#declaring-a-model)
- [Records](#records)
- [Filling from Input](#filling-from-input)
- [Validation](#validation)
- [Hooks](#hooks)
- [Queries](#queries)
- [Associations](#associations)
- [Transactions](#transactions)
- [Casts](#casts)
- [Records as Output](#records-as-output)
- [What It Leaves to You](#what-it-leaves-to-you)

## A First Model

A model is a class extending `Model`. Its fields are its table's
columns:

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

`db.model(Post)` starts a query over the model's rows. Records take the
connection to use on every call that touches the database: `save(db)`,
`delete(db)`. A record never holds a connection of its own, so it saves
inside whichever transaction the code saving it opened, and one kept
after its connection went back to a pool cannot write through a
connection that is now somebody else's.

## Declaring a Model

Every field the class declares that does not start with `_` is a column
of the same name. Everything else about the model goes in a static
`define()`, which is handed a definition to describe it on:

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

A model's table is its class name in snake case, made plural by the
regular rules of English: `Post` is `posts`, `BlogPost` is
`blog_posts`, `Category` is `categories`, `Address` is `addresses`. A
word whose plural is irregular names its table with `table()`.

`define()` runs once, the first time the model is used, not when its
class is declared, so it names models declared after it in the same
file. What it describes is checked against the class then: an
association with no field to load into, or a cast on a field that is
not a column, raises `ModelError` at that first use. A model's primary
key is one column.

Records loaded from the database are built with no arguments, so a
model's own `@new` takes none that are required, and passes attributes
on with `parent(attributes)` when it takes them.

## Records

Calling the class builds a record, from a dictionary of attributes when
one is given. `create()` on a query builds one and saves it:

```zuri,ignore
var draft = Post({ title: 'On Looms' })
var saved = db.model(Post).create({ title: 'On Bugs' })
```

Saving a new record inserts it. Every column still nil is left out of
the insert, so the database's default fills it, and those columns are
read back: the record holds what was stored. Saving a record that is
already stored writes the columns that changed since it was loaded, and
nothing at all when none did, so a column another process changed
meanwhile is not overwritten by a save of a different one.

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

A list or dictionary changed in place counts as a change. A deleted
record keeps its values, and saving it again inserts it anew.
`key()` is the primary key's value, whatever the column is called.

## Filling from Input

`fill()` assigns attributes from a dictionary, as a form or a request
body supplies them, and it assigns only fillable columns: those the
model lists with `fillable()`, or every column but the primary key and
the timestamps when it lists none. A key that is not fillable raises
`ModelError`, so a request cannot set a record's key, or a column it was
never meant to reach, by sending one more field.

The second argument takes the keys asked for and leaves the rest, which
is the form to use with input that carries more than the record should
take:

```zuri,ignore
post.fill(request.form(), ['title', 'body'])
```

## Validation

A model checks itself before every save, with `validate` chains, with
unique checks, and with a `validate()` of its own. A record that fails
is not saved, and `save()` raises `RecordInvalid`:

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
same `{ field, message }` list a form's validation reports, so the code
answering a bad form with a 422 answers a bad record the same way.
`is_valid(db)` runs the checks without saving, and `errors()` returns
what the last one found.

A unique check reports a taken value with the other failures. Only a
unique index in the database stops two records saved at the same moment
from taking the same value, so a unique column keeps one there too.

## Hooks

A model overrides any of these methods to run code at that point of a
record's life. Each is handed the connection or transaction the record
is going through:

| Hook | Runs |
| --- | --- |
| `before_validate(db)` | before the record is checked |
| `validate(errors)` | as it is checked, to add failures of the model's own |
| `after_validate(db)` | after it is checked, whatever was found |
| `before_save(db)` | before it is written, new or not |
| `before_create(db)` | before a new record is inserted |
| `after_create(db)` | after it is inserted, with its key |
| `before_update(db)` | before a stored record's changes are written |
| `after_update(db)` | after they are written |
| `after_save(db)` | after it is written, new or not |
| `before_delete(db)` | before it is deleted |
| `after_delete(db)` | after it is deleted |
| `after_load()` | as a record is built from a row |

When a model has any of them, saving and deleting run in a transaction,
or a savepoint inside one already open, so what a hook writes is undone
with the rest when anything fails. A hook raises to stop the save or
the delete:

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

A model's methods share a namespace with the hooks and with every
method a record has, so a model does not declare one of the same name
for anything else. A field may share a name with any of them.

## Queries

`db.model(Post)` is a `Query`. Every method that narrows it returns a
new query and leaves the one it was called on as it was, so a query is
kept and built on in more than one direction:

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

`where()` with a dictionary reads as the CRUD helpers' conditions do: a
value matches itself, `nil` matches null, a list matches any of its
values, and an empty list matches nothing. Anything else is a fragment
of SQL, with `?` for each value, bracketed as it is added, so an `or`
inside one stays inside it. Every condition must hold. `where_not()`
excludes, and a row whose column is null is among the rows a value
excludes, since null is not equal to anything.

What a query does with what it matches:

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

A query is iterable: `for post in query` reads every record. `update()`
and `delete()` change every matching row in one statement, without
building records, so no validation runs and no hook is called;
`update()` sets the `updated` timestamp unless it is given.

A scope is a reusable piece of a query, declared with `scope()` and
applied with `query.scope(name, argument)`.

## Associations

Four kinds of association cover how tables refer to each other:

| Declared as | Means | The key is |
| --- | --- | --- |
| `belongs_to('author', Author)` | this record holds another's key | `author_id` here |
| `has_one('profile', Profile)` | another holds this one's key; at most one | `author_id` there |
| `has_many('posts', Post)` | others hold this one's key | `author_id` there |
| `belongs_to_many('tags', Tag)` | a table joins both keys | `post_id` and `tag_id` in `post_tag` |

Each name is also the field the association loads into, which the class
declares. The keys are named for the class name in snake case and
`_id`, and the join table for both class names in alphabetical order;
options name them otherwise:

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

Associations are loaded when asked for. `include()` loads them along
with the records, one query per association however many records there
are, and a dotted name reaches what an association loads:

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

A field holding an association is nil until it is loaded. `load()`
loads one onto a record in hand and returns it; `query.load(records,
names)` loads onto a list of them in one query per association.
`related()` is a query over what an association reaches, to narrow
further rather than load whole.

A record placed in a `belongs_to` field sets the key referring to it
when the record holding it is saved, and has to be saved itself first.
`attach()`, `detach()` and `sync()` join records through a
`belongs_to_many` association's table, taking records or keys.

## Transactions

`tx.model(Post)` queries inside a transaction, and a record saved with
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

Most values arrive as the type they were stored as. A cast makes a
field the same type on every engine where they differ, as it is read:

| Cast | The field holds |
| --- | --- |
| `bool` | `true` or `false`, from a bool, a number, `'0'`, `'1'`, `'true'` or `'false'` |
| `integer` | a whole number |
| `number` | a number |
| `decimal` | a `sql.Decimal` |
| `string` | text |
| `json` | a list or dictionary, from JSON text or one already decoded |
| `date` | a `date.Date`, from one or from ISO 8601 text |

MySQL has no boolean and returns `1`, and MariaDB keeps JSON as text, so
a model running on either casts those columns: `cast('published',
'bool')`, `cast('settings', 'json')`. Null stays `nil` under every cast,
and a value that will not convert is left as it was read rather than
lost.

## Records as Output

`to_dict()` is a record's columns, but for the ones `hidden()` names,
and every association that has been loaded, as dictionaries in turn.
`json.encode()` writes a record as its `to_dict()`, so a record handed
to a response leaves out a password's hash:

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

Two records are equal when they are of the same model and hold the
same primary key; a record not yet saved equals only itself.

## What It Leaves to You

**Loading is explicit.** An association is loaded by `include()` or
`load()`, never by reading its field, so a loop over records never
turns into a query per record behind the code's back.

**Each query builds its own records.** Two queries reaching the same
row build two records of it, which are `==` to each other. Nothing is
cached between queries, so a record read is what the database held when
it was read.

**Joins are SQL.** Conditions cover a model's own columns, and
`related()` and `include()` cover associations. A join, a union or a
window function is written as SQL and handed to `from_sql()`, which
builds records from whatever it selects.

**Tables come from migrations.** A model maps a table that exists;
[Migrations](ch17-02-migrations.md) and
[Changing the Schema](ch17-01-schema-changes.md) are what create and
change it.
