# The Module System

A module is a `.zu` file. It runs at most once per program, it gets its own
global namespace, and nothing it declares is visible anywhere else until
someone imports it.

That is the whole model. The rest of this chapter is the syntax and the
resolution rules.

## Importing

```zuri
import math
import os

echo math.PI
echo os.cwd()
```

`import name` binds the module under that name. Reach into it with a dot.

### Picking Out Members

```zuri
import math { PI, E }

echo PI
```

The named members are bound directly, and the module name itself is not.

`{ * }` brings in everything public:

```zuri
import math { * }

echo PI
echo ROOT_2
```

### Renaming

```zuri
import http.websocket as ws
```

Use this when the natural name is long, or when it would collide.

## Relative Imports

A path starting with `.` or `..` is resolved against the directory of the
file doing the importing, and is never searched for anywhere else.

```zuri,ignore
import .helpers          # helpers.zu, next to this file
import .models.user      # models/user.zu
import ..shared.config   # up one directory, then shared/config.zu
```

Inside a package, a sibling is always `import .sibling`, never the full
path from the project root. Writing `import myapp.models.user` from inside
`myapp/models/` sends the resolver out to the library search path and it
will not find anything.

## Exporting

**Imports are local by default.** If `a.zu` imports `b.zu`, a third file
that imports `a` does not see `b`'s contents through it.

Prefix the path with `@` to re-export:

```zuri,ignore
import @.util { * }
```

Now everything `util.zu` exposed is part of this module's public surface
too. All three forms take the prefix:

```zuri,ignore
import @.module            # the module itself is re-exported
import @.module { item }   # just that item
import @.module { * }      # everything
```

This is how a package's `index.zu` assembles a public API out of several
private files:

<span class="filename">Filename: pkg/index.zu</span>

```zuri,ignore
import @.util { * }
import .sub.deep { deep_slug }

def hello() {
  return 'hello from pkg ' + VERSION
}
```

`util`'s members are re-exported. `deep_slug` is imported for this file's
own use and stays private.

## Privacy

A member whose name starts with `_` is private to its module and cannot be
imported by name:

```zuri,ignore
import .pkg.util { _secret }
```

```console
SyntaxError: Cannot import private items from module
  --> /path/to/bad.zu:1:20
  |
1 | import .pkg.util { _secret }
  |                    ^
```

`{ * }` skips private members too. Prefix anything that is an
implementation detail and the module system will keep it that way.

## Packages

A directory with an `index.zu` is a package, and importing the directory
runs its `index.zu`:

```text
pkg/
  index.zu
  util.zu
  sub/
    deep.zu
```

```zuri,ignore
import .pkg            # runs pkg/index.zu
import .pkg.util       # runs pkg/util.zu
import .pkg.sub.deep   # runs pkg/sub/deep.zu
```

The same rule applies to `zuri pkg` on the command line, which is what
makes a package runnable as well as importable.

## How a Bare Name Is Resolved

`import http`, with no leading dot, is searched for in this order:

1. `./.zuri/libs/http` relative to the **current working directory**
2. `$ZURI_ROOT/libs/http`, or `libs/http` beside the `zuri` executable
3. a built-in native module named `http`

At each step, `http.zu` is tried first and then `http/index.zu`.

Step one is what makes vendoring work. Dropping a file into `.zuri/libs/`
in your project directory shadows a standard library module of the same
name:

```console
$ cat .zuri/libs/mylib.zu
def hi() { return 'from user libs' }

$ cat uses.zu
import mylib
echo mylib.hi()

$ zuri uses.zu
from user libs
```

## Modules Run Once

A module's top level executes the first time it is imported, and never
again. Every later import of the same file gets the same module object:

```zuri,ignore
import .once
import .once as again
import .once
```

```console
side effect ran
```

One line of output, three imports. Identity is by canonical filesystem
path, so two different relative paths to the same file are the same module.

This makes a module's top level the natural place for setup that must
happen exactly once: opening a connection pool, reading a config file,
registering handlers.

## Circular Imports

A circular import works. A module is registered before its body runs, so
when `b.zu` imports `a.zu` while `a.zu` is still loading, it gets the
partially built module rather than looping forever:

<span class="filename">Filename: a.zu</span>

```zuri,ignore
import .b

def from_a() {
  return 'a'
}

echo 'a loaded'
```

<span class="filename">Filename: b.zu</span>

```zuri,ignore
import .a

echo 'b loaded'
```

```console
b loaded
a loaded
a
```

The catch is visible in that output: `b` finished loading before `a` did,
so anything `b` reads from `a` **at its top level** is not there yet.
Reading it from inside a function is fine, because by then `a` has
finished.

If a module fails while loading, it is dropped from the cache rather than
left behind half built, so a later import genuinely retries.

## Module Variables

Every module gets two names for free:

```zuri
echo __file__
echo __root__
```

`__file__` is this module's own canonical path. `__root__` is the entry
file the program was started from, and it is the same in every module. Use
them to locate files relative to your source rather than relative to
whatever directory the user happened to run from:

```zuri
import os

var templates = os.join_paths(os.dir_name(__file__), 'templates')
```

Neither is defined in the REPL.

### Running as a Program, Importing as a Module

Because `__root__` is the entry file and `__file__` is this one, comparing
them tells a module whether it is the program being run or something being
imported:

<span class="filename">Filename: tool.zu</span>

```zuri
def add(a, b) {
  return a + b
}

if __root__ == __file__ {
  echo 'running as a program: ' + add(2, 3)
}
```

```console
$ zuri tool.zu
running as a program: 5
```

```zuri,ignore
import .tool
echo tool.add(10, 20)
```

```console
30
```

The same file is a clean, side-effect-free library when imported and a
runnable command-line program when launched directly. Put the argument
parsing and the entry point behind that check, and everything else above
it.

These two names describe *which file this is*, so they are not exports. A
wildcard import copies every public name out of the module it names, and
`__file__` and `__root__` are deliberately excluded from that:

```zuri
import math { * }

echo __file__ == __root__
```

```console
true
```

That guarantee is what makes the `__root__ == __file__` check above
reliable in every file, including one that wildcard-imports a sibling.

## Structuring a Project

A small program is one file. Past that, the shape that works is a package
per area of responsibility, each with an `index.zu` that re-exports what is
public:

```text
myapp/
  index.zu          # import @.routes, import @.models, then start
  config.zu
  models/
    index.zu        # import @.user { * }, import @.task { * }
    user.zu
    task.zu
  routes/
    index.zu
    api.zu
    pages.zu
  storage/
    index.zu
    _json_store.zu  # private: the leading underscore says so
```

Run it with `zuri myapp`. The capstone in [Chapter 20](ch20-00-task-board.md)
is laid out exactly this way.
