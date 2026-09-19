# Testing the Board

The application works. This section is about keeping it working, and it is
the last thing the layering from
[Laying Out the Project](ch24-01-project-layout.md) pays for.

Recall the shape:

```text
index.zu  ->  app.zu  ->  routes/  ->  storage/  ->  models/
```

That is also the order to test in. `models` depends on nothing but
`config`, so its tests need nothing. `storage` depends on `models` and a
file, so its tests need a directory. `routes` depends on `storage` and a
server, and takes both as arguments, so its tests need neither a socket nor
a real board.

Each layer is tested against the layer below it as it really is, and
against the layer above it not at all.

## Where the Tests Live

```text
taskboard/
  index.zu
  app.zu
  config.zu
  models/
  storage/
  routes/
  tests/
    index.zu            runs the rest
    task.zu
    board.zu
    api.zu
```

<span class="filename">Filename: tests/index.zu</span>

```zuri,ignore
import os
import test

test.conduct(os.dir_name(__file__))
```

```console
$ zuri tests
```

`tests/` gets an `index.zu` for the same reason `models/` and `storage/`
do: a directory handed to `zuri` runs its `index.zu`. The application is
`zuri taskboard`, so its tests are `zuri tests`, and neither needs a file
name remembering.

`conduct` leaves `index.zu` out of discovery, and never runs the script
that called it either, so the index cannot end up running itself. It exits
`1` when anything failed, which is all CI needs.

Each test file is an ordinary script: it declares its tests and stops.
`conduct` runs each one in a process of its own, which matters here for
one concrete reason: every one of these files is going to create a
`Board`, and a `Board` is a file on disk. One process per file means one
file's leftovers can never reach another's.

## The Model

`models/task.zu` is pure. No file, no clock you care about, no network. Its
tests are the fastest and the ones worth writing first, because every rule
about what a task is lives there and nothing above re-checks any of them.

<span class="filename">Filename: tests/task.zu</span>

```zuri,ignore
import test { * }

import ..models { Task, TaskError, from_dict }

describe('Task', @{

  describe('construction', @{

    it('needs a title', @{
      expect(@{ Task(nil) }).to_raise_instance_of(TaskError)
      expect(@{ Task('') }).to_raise_instance_of(TaskError)
      expect(@{ Task('   ') }).to_raise_instance_of(TaskError)
    })

    it('says which field was wrong', @{
      catch {
        Task(nil)
      } as error {
        expect(error.field).to_be('title')
      }
    })

    it('refuses a title over 120 characters', @{
      expect(@{ Task('x' * 121) }).to_raise_instance_of(TaskError)
      expect(@{ Task('x' * 120) }).to_not_raise()
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

      expect(@{ task.update({ column: 'sideways' }) }).to_raise_instance_of(TaskError)
      expect(task.column).to_be('todo')
    })

  })

  describe('round-tripping', @{

    it('rebuilds a task from what it stored', @{
      var original = Task('Write the tests', { notes: 'a note', column: 'doing' })

      expect(from_dict(original.to_dict())).to_equal(original)
    })

    it('rejects a stored record that broke a rule', @{
      expect(@{ from_dict({ id: 'x', title: '', column: 'todo' }) })
        .to_raise_instance_of(TaskError)
    })

    it('survives a record written before a field existed', @{
      var task = from_dict({ title: 'Write the tests' })

      expect(task.column).to_be('todo')
      expect(task.notes).to_be('')
    })

  })

})
```

Four of those are worth pointing at.

**`expect(@{ task.update(...) }).to_raise_instance_of(TaskError)` followed
by `expect(task.column).to_be('todo')`.** Two assertions, because there are
two claims: that it refused, and that it refused *before changing
anything*. A validator that raises after assigning is a bug you only find
by checking the second one.

**`to_equal`, not `to_be`, for the round trip.** `from_dict()` builds a new
`Task`, so identity is never going to match. The whole question is whether
the contents survived.

**`expect([first.id, second.id]).to_be_sorted()`** is the cheapest possible
way to state what `uuid.v7()` was chosen for. If someone ever changes that
line to `v4()`, this is the test that says so.

**The record with no `column` and no `notes`.** That is the
older-version-of-the-program case that `data.get(key, nil)` exists to
handle. Writing the test is what stops the next person from simplifying
`get()` into `.column` and finding out on someone's real `board.json`.

## The Storage Layer

A `Board` is a file, so its tests need a directory of their own. One per
test, created and removed by the hooks:

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
    expect(@{ board.get('no-such-id') }).to_raise_instance_of(NotFoundError)
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
    expect(@{ board.get(created.id) }).to_raise_instance_of(NotFoundError)
  })

})
```

The one that earns its place is **`survives a restart`**. It constructs a
*second* `Board` over the same directory and asks it for the task the first
one created. That is the only test here that exercises `_json_store`,
`to_dict()` and `from_dict()` together, and it is the test that fails the
day someone adds a field to `Task` and forgets one half of the pair.

`after_each` removes the directory whether the test passed or not, so a
failing test leaves nothing behind for the next one to trip over. That is
the property to rely on: teardown that only runs on success is teardown you
cannot trust.

Note that none of these tests read `board.json` themselves. They ask the
`Board` what it holds. The file format is `storage`'s business, and a test
that parsed it would fail the day the format changed for a reason that has
nothing to do with what the test was checking.

## The Routes

`register(server, board)` takes its two collaborators as arguments. That
was presented in [The JSON API](ch24-04-api.md) as being about seeding a
demo board; here is the other half of what it buys.

A handler needs three things: something to register on, a request, and a
response. Stand in for all three:

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

Three small classes and the routes become ordinary functions:

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
    var response = call('POST /api/tasks', { body: { title: 'Write the tests' } })

    expect(response.status).to_be(201)
    expect(response.body).to_match_object({ title: 'Write the tests', column: 'todo' })
    expect(board.all()).to_have_length(1)
  })

  it('lets the model reject a bad body', @{
    expect(@{ call('POST /api/tasks', { body: {} }) }).to_raise_with_message('title')
  })

  it('treats a missing body as an empty one', @{
    expect(@{ call('POST /api/tasks', {}) }).to_raise_with_message('title')
  })

  it('lists everything, with a summary', @{
    board.add('one')

    var response = call('GET /api/tasks', {})

    expect(response.body).to_have_keys(['tasks', 'summary'])
    expect(response.body.tasks).to_have_length(1)
  })

  it('filters by column when asked', @{
    board.add('one')

    expect(call('GET /api/tasks', { query: { column: 'done' } }).body.tasks).to_be_empty()
  })

  it('sends the view, not the stored task', @{
    var created = board.add('Write the tests')

    var response = call('GET /api/tasks/:id', { params: { id: created.id } })

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

Several things worth saying about that.

**The board is real.** Only the HTTP machinery is faked. A fake board would
have meant writing down what `board.add()` returns, and the test would then
pass forever afterwards regardless of what `board.add()` actually did.
Faking is for the things that are slow, remote or awkward, and a temporary
file is none of those.

**`lets the model reject a bad body`.** From
[Handlers Do Not Handle Errors](ch24-04-api.md#handlers-do-not-handle-errors):
the handler does not catch anything, so an invalid title comes back out of
the handler as a `TaskError`. That is the behaviour the middleware relies
on, and this asserts it directly rather than through the middleware.

**`ignores a key the model does not own`** is the whitelist claim from
`update()`, tested where an attacker would aim it. One line of test for a
property the code gets for free from `contains()`, and the day someone
"simplifies" that method it fails.

**Use a class, not a dictionary, for a fake.** A dictionary already has
`get`, `add`, `set` and `keys` of its own, so `fake.get('id')` calls the
dictionary's method rather than yours. A small class gives you the names
you meant.

## What to Test Next

Two layers are deliberately not tested above.

**The pages.** `routes/pages.zu` renders templates. The same fake server
works, and the assertion becomes `expect(response.body).to_contain(...)`
against the rendered HTML, or `to_match_snapshot()`, which records the
whole page once and watches it from then on:

```zuri,ignore
it('renders the board', @{
  expect(call('GET /', {}).body).to_match_snapshot()
})
```

That records the page in `tests/__snapshots__/pages.zu.snap`. Reviewing
the diff when it changes is the point; a template edit that alters more of
the page than you intended shows up there and nowhere else.

**The whole thing.** Starting the real server on port `0`, making real
requests with the `http` client, and shutting it down in `after_all` gives
you one test that covers routing, middleware, templates and storage
together. Write a handful of those, not a hundred: they are the slowest
tests you own and the ones that break for reasons unrelated to what they
were checking.

## What This Bought

Run it:

```console
$ zuri tests

  zuri test  3 files in tests

   PASS   api.zu  312ms  8 tests
   PASS   board.zu  198ms  7 tests
   PASS   task.zu  164ms  10 tests

  3 files
  25 passed  •  25 total
  time 674ms
```

Twenty-five tests, under a second, no server and no network. That is a
direct consequence of the layering: every arrow points one way, and every
layer takes its collaborators as arguments rather than importing them.

The layering was justified in
[Laying Out the Project](ch24-01-project-layout.md) on the grounds that it
makes each layer readable on its own. This is the other half of the claim,
and it is the half you feel every day.
