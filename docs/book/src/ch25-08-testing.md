# Testing the Board

The application works. This section is about keeping it working, and it's
the final benefit of the layered design from [Laying Out the
Project](ch25-01-project-layout.md).

Here's the structure again:

```text
index.zu  ->  app.zu  ->  routes/  ->  storage/  ->  models/
```

That's also the order to test the layers in. `models` only depends on
`config`, so its tests don't need anything. `storage` depends on `models`
and a file, so its tests need a directory. `routes` depends on `storage` and
a server, and takes both as arguments, so its tests need neither a socket
nor a real board.

Each layer is tested against the real layer below it, and without the layer
above it at all.

## Where the Tests Go

```text
taskboard/
  index.zu
  app.zu
  config.zu
  models/
  storage/
  routes/
  tests/
    task.zu
    board.zu
    api.zu
```

```console
$ zuri test
```

That's all there is to it. You run the application with `zuri run taskboard`
and its tests with `zuri test`, and you don't need to remember any file
names. The command exits with status `1` if anything fails, which is all a
CI system needs.

Each test file is an ordinary script that declares its tests and stops. Each
one runs in its own process, which matters here for a specific reason: each
of these files creates a `Board`, and a `Board` is a file on disk. Because
each file runs in its own process, nothing one file leaves behind can affect
another.

## Testing the Model

`models/task.zu` doesn't depend on anything outside itself. There's no file,
no network, and no clock that matters. Its tests are the fastest, and
they're the ones to write first, because every rule about what a task is
lives in this file, and nothing above it checks any of them again.

<span class="filename">Filename: tests/task.zu</span>

```zuri,ignore
import test { * }

import ..models { Task, TaskError, from_dict }

describe('Task', @{

  describe('construction', @{

    it('needs a title', @{
      expect(@{
        Task(nil)
      }).to_raise_instance_of(TaskError)
      expect(@{
        Task('')
      }).to_raise_instance_of(TaskError)
      expect(@{
        Task('   ')
      }).to_raise_instance_of(TaskError)
    })

    it('says which field was wrong', @{
      catch {
        Task(nil)
      } as error {
        expect(error.field).to_be('title')
      }
    })

    it('refuses a title over 120 characters', @{
      expect(@{
        Task('x' * 121)
      }).to_raise_instance_of(TaskError)
      expect(@{
        Task('x' * 120)
      }).to_not_raise()
    })

    it('defaults everything but the title', @{
      var task = Task('Write the tests')

      expect(task.notes).to_be('')
      expect(task.column).to_be('todo')
      expect(task.id).to_be_string()
      expect(task.created_at).to_be_number()
    })

    it('sorts by id, because v7 embeds the time', @{
      var first = Task('one')
      var second = Task('two')

      expect([first.id, second.id]).to_be_sorted()
    })
  })

  describe('update()', @{

    it('touches only the keys it was given', @{
      var task = Task('Write the tests', { notes: 'keep me' })

      task.update({ column: 'doing' })

      expect(task.column).to_be('doing')
      expect(task.notes).to_be('keep me')
      expect(task.title).to_be('Write the tests')
    })

    it('clears a value when the key is present and empty', @{
      var task = Task('Write the tests', { notes: 'remove me' })

      task.update({ notes: '' })

      expect(task.notes).to_be('')
    })

    it('refuses a column the board does not have', @{
      var task = Task('Write the tests')

      expect(@{
        task.update({ column: 'sideways' })
      }).to_raise_instance_of(TaskError)
      expect(task.column).to_be('todo')
    })
  })

  describe('round-tripping', @{

    it('rebuilds a task from what it stored', @{
      var original = Task('Write the tests', {
        notes: 'a note',
        column: 'doing',
      })

      expect(from_dict(original.to_dict())).to_equal(original)
    })

    it('rejects a stored record that broke a rule', @{
      expect(@{
        from_dict({ id: 'x', title: '', column: 'todo' })
      }).to_raise_instance_of(TaskError)
    })

    it('survives a record written before a field existed', @{
      var task = from_dict({ title: 'Write the tests' })

      expect(task.column).to_be('todo')
      expect(task.notes).to_be('')
    })
  })
})
```

Four of these are worth a closer look.

**`expect(@{ task.update(...) }).to_raise_instance_of(TaskError)`, followed
by `expect(task.column).to_be('todo')`.** There are two assertions because
there are two claims: that the update was rejected, and that it was rejected
*before anything was changed*. A validator that raises its error after
assigning the value is a bug you'll only find by checking the second claim.

**`to_equal`, not `to_be`, for the round trip.** `from_dict()` creates a new
`Task`, so the identities will never match. What matters is whether the
contents survived.

**`expect([first.id, second.id]).to_be_sorted()`** is the simplest way to
check the reason `uuid.v7()` was chosen. If someone ever changes that line
to `v4()`, this test will tell them.

**The record without a `column` and `notes`.** This is the case of a record
saved by an older version of the program, which is why `data.get(key, nil)`
is used. The test stops the next person from simplifying `get()` to
`.column` and only finding out it was a mistake when it breaks someone's
real `board.json`.

## Testing the Storage Layer

A `Board` is a file, so its tests need a directory of their own. Each test
gets one, created and removed by the hooks:

<span class="filename">Filename: tests/board.zu</span>

```zuri,ignore
import test { * }
import os

import ..models { Task }
import ..storage { Board, NotFoundError }

describe('Board', @{

  var directory = nil
  var board = nil

  before_each(@{
    directory = os.create_temp_dir('taskboard-test')
    board = Board(directory)
  })

  after_each(@{
    os.remove_dir(directory, true)
  })

  it('starts empty', @{
    expect(board.all()).to_be_empty()
    expect(board.summary()).to_match_object({ todo: 0, doing: 0, done: 0 })
  })

  it('keeps what it was given', @{
    board.add('Write the tests')

    expect(board.all()).to_have_length(1)
    expect(board.all()[0].title).to_be('Write the tests')
  })

  it('survives a restart', @{
    var created = board.add('Write the tests', { notes: 'a note' })

    expect(Board(directory).get(created.id)).to_equal(created)
  })

  it('raises for an id it does not have', @{
    expect(@{
      board.get('no-such-id')
    }).to_raise_instance_of(NotFoundError)
  })

  it('filters by column', @{
    board.add('one')
    var moved = board.add('two')
    board.update(moved.id, { column: 'doing' })

    expect(board.in_column('todo')).to_have_length(1)
    expect(board.in_column('doing')).to_have_length(1)
    expect(board.in_column('done')).to_be_empty()
  })

  it('counts what it holds', @{
    board.add('one')
    board.add('two')

    expect(board.summary()).to_match_object({ todo: 2 })
  })

  it('removes', @{
    var created = board.add('Write the tests')

    board.remove(created.id)

    expect(board.all()).to_be_empty()
    expect(@{
      board.get(created.id)
    }).to_raise_instance_of(NotFoundError)
  })
})
```

The most valuable test here is **`survives a restart`**. It creates a
*second* `Board` using the same directory, and asks it for the task the
first one created. It's the only test that uses `_json_store`, `to_dict()`
and `from_dict()` together, and it's the test that fails when someone adds a
field to `Task` and forgets to update one of the two methods.

`after_each` removes the directory whether or not the test passed, so a
failed test doesn't leave anything behind for the next one to trip over.
That's an important property, because teardown that only runs when a test
succeeds isn't teardown you can rely on.

Notice that none of these tests read `board.json` directly. They ask the
`Board` what it contains. The file format is the storage layer's concern,
and a test that parsed the file would fail whenever the format changed, for
reasons that have nothing to do with what the test was checking.

## Testing the Routes

`register(server, board)` takes its two collaborators as arguments. [The
JSON API](ch25-04-api.md) explained that this lets you run the routes with a
board filled with sample data. Here's the other benefit.

A handler needs three things: something to register routes on, a request and
a response. You can provide stand-ins for all three:

<span class="filename">Filename: tests/api.zu</span>

```zuri,ignore
import test { * }
import os

import ..routes { register_api }
import ..storage { Board }

class FakeServer {
  var routes = {}

  @new() {
    self.routes = {}
  }

  get(path, handler) {
    self.routes.set('GET ${path}', handler)
  }

  post(path, handler) {
    self.routes.set('POST ${path}', handler)
  }

  patch(path, handler) {
    self.routes.set('PATCH ${path}', handler)
  }

  delete(path, handler) {
    self.routes.set('DELETE ${path}', handler)
  }
}

class FakeRequest {
  var params = {}
  var query = {}
  var body = nil

  @new(options) {
    options = options or {}

    self.params = options.get('params', {})
    self.query = options.get('query', {})
    self.body = options.get('body', nil)
  }

  param(name) {
    return self.params.get(name, nil)
  }

  query_param(name, fallback) {
    return self.query.get(name, fallback)
  }

  json_body() {
    return self.body
  }
}

class FakeResponse {
  var body = nil
  var status = 200

  json(body, status) {
    self.body = body
    self.status = status or 200
  }
}
```

With three small classes, the routes become ordinary functions:

```zuri,ignore
describe('the JSON API', @{

  var directory = nil
  var server = nil
  var board = nil

  before_each(@{
    directory = os.create_temp_dir('taskboard-test')
    board = Board(directory)
    server = FakeServer()

    register_api(server, board)
  })

  after_each(@{
    os.remove_dir(directory, true)
  })

  def call(route, options) {
    var response = FakeResponse()
    server.routes[route](FakeRequest(options), response)

    return response
  }

  it('registers every route', @{
    expect(server.routes).to_have_keys([
      'GET /api/tasks',
      'GET /api/tasks/:id',
      'POST /api/tasks',
      'PATCH /api/tasks/:id',
      'DELETE /api/tasks/:id',
      'GET /api/summary',
    ])
  })

  it('answers 201 when it creates a task', @{
    var response = call('POST /api/tasks', {
      body: { title: 'Write the tests' },
    })

    expect(response.status).to_be(201)
    expect(response.body)
        .to_match_object({ title: 'Write the tests', column: 'todo' })
    expect(board.all()).to_have_length(1)
  })

  it('lets the model reject a bad body', @{
    expect(@{
      call('POST /api/tasks', { body: {} })
    }).to_raise_with_message('title')
  })

  it('treats a missing body as an empty one', @{
    expect(@{
      call('POST /api/tasks', {})
    }).to_raise_with_message('title')
  })

  it('lists everything, with a summary', @{
    board.add('one')

    var response = call('GET /api/tasks', {})

    expect(response.body).to_have_keys(['tasks', 'summary'])
    expect(response.body.tasks).to_have_length(1)
  })

  it('filters by column when asked', @{
    board.add('one')

    expect(
      call('GET /api/tasks', {
        query: { column: 'done' },
      }).body.tasks,
    ).to_be_empty()
  })

  it('sends the view, not the stored task', @{
    var created = board.add('Write the tests')

    var response = call('GET /api/tasks/:id', {
      params: { id: created.id },
    })

    expect(response.body).to_have_keys(['created_on', 'is_done'])
  })

  it('ignores a key the model does not own', @{
    var created = board.add('Write the tests')

    call('PATCH /api/tasks/:id', {
      params: { id: created.id },
      body: { id: 'hacked', column: 'doing' },
    })

    expect(board.get(created.id).column).to_be('doing')
    expect(board.get(created.id).id).to_be(created.id)
  })
})
```

There are several things to say about this.

**The board is real.** Only the HTTP machinery is fake. A fake board would
mean writing down what `board.add()` returns, and the test would keep
passing from then on, whatever `board.add()` actually did. Fakes are for
things that are slow, remote or awkward to set up, and a temporary file is
none of those.

**`lets the model reject a bad body`.** As described in [Why the Handlers
Don't Handle Errors](ch25-04-api.md#why-the-handlers-dont-handle-errors),
the handler doesn't catch anything, so an invalid title comes out of the
handler as a `TaskError`. The middleware depends on that behavior, and this
test checks it directly instead of through the middleware.

**`ignores a key the model does not own`** tests the list of allowed keys in
`update()`, at exactly the point an attacker would target. It's one line of
test code for a property the code gets for free from `contains()`, and if
someone ever "simplifies" that method, the test will fail.

**Use a class, not a dictionary, for a fake.** A dictionary already has its
own `get`, `add`, `set` and `keys` methods, so `fake.get('id')` would call
the dictionary's method instead of yours. A small class gives you exactly
the names you intended.

## What to Test Next

Two layers are intentionally left untested above.

**The pages.** `routes/pages.zu` renders templates. You can use the same
fake server, and check the rendered HTML with
`expect(response.body).to_contain(...)`, or with `to_match_snapshot()`,
which records the whole page once and compares against it from then on:

```zuri,ignore
it('renders the board', @{
  expect(call('GET /', {}).body).to_match_snapshot()
})
```

That stores the page in `tests/__snapshots__/pages.zu.snap`. The point is to
review the changes when it changes. If a template edit changes more of the
page than you meant it to, that's where you'll see it.

**The whole application.** You can start the real server on port `0`, send
real requests with the `http` client, and shut it down in `after_all`. That
gives you a test that covers routing, middleware, templates and storage
together. Write a few of these, not a hundred. They're your slowest tests,
and they're the ones most likely to break for reasons that have nothing to
do with what they're checking.

## The Result

Run the tests:

```console
$ zuri test

  zuri test  3 files in tests

   PASS   api.zu  312ms  8 tests
   PASS   board.zu  198ms  7 tests
   PASS   task.zu  164ms  10 tests

  3 files
  25 passed  •  25 total
  time 674ms
```

That's twenty-five tests in under a second, without a server or a network.
It's a direct result of the layered design: every dependency points in the
same direction, and every layer takes its collaborators as arguments instead
of importing them.

[Laying Out the Project](ch25-01-project-layout.md) justified the layers by
saying they make each layer readable on its own. This is the other half of
that argument, and it's the half you'll notice every day.
