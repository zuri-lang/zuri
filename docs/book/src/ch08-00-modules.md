# The Module System

A module is a `.zu` file. It runs at most once per program, it has its own
global namespace, and nothing it declares is visible anywhere else until
another file imports it.

That's all there is to the model. The rest of this chapter covers the syntax
and the rules for finding modules.

## Importing a Module

```zuri
import math
import os

echo math.PI
echo os.cwd()
```

`import name` makes the module available under that name. Use a dot to
access what's inside it.

### Importing Specific Members

```zuri
import math { PI, E }

echo PI
```

The members you name are bound directly, and the module name itself isn't
bound.

`{ * }` imports every public member:

```zuri
import math { * }

echo PI
echo ROOT_2
```

### Renaming a Module

```zuri
import http.websocket as ws
```

This is useful when the module's name is long, or when it would clash with
another name.

`as` renames the **module**, not a member. You can't rename individual names
in a member list, so `import math { PI as pi }` doesn't parse. If you want
to use a different name for something you've imported, assign it yourself:

```zuri
import math { PI }

var pi = PI

echo pi
```

```console
3.141592653589793
```

### Calling a Module Directly

If a module contains a function with the same name as the module, you can
call the module itself. `set` declares a function called `set`, so calling
the module calls that function:

```zuri
import set

echo set([1, 2, 2]).length()
echo set.set([1, 2, 2]).length()
```

```console
2
2
```

The short form is usually preferred. `wire()`, `date()`, `log()`, `hash()`,
`enum()` and `compress()` work the same way, and so do your own modules:
once you've imported a `greet.zu` that contains `def greet(name)`, you can
call it as `greet('Ada')`. A class with the same name as its module can be
called in the same way.

The call relies on the name the module is bound to, so it stops working if
you rename the module. The function is still available as a member:

```zuri
import set as collection

catch {
  collection([1])
} as e {
  echo e.message
}

echo collection.set([1, 2]).length()
```

```console
module 'collection' is not callable
2
```

The name still refers to a module, not a function. `typeof()` confirms this,
and a method that expects a function needs the member itself:

```zuri
import hash

echo typeof(hash)
echo ['md5', 'sha1'].map(@(name) => hash(name, 'zuri').length())
```

```console
module
[32, 40]
```

Calling a module that doesn't have a matching function raises a `TypeError`.

## Relative Imports

A path that starts with `.` or `..` is resolved relative to the directory of
the file that contains the import, and Zuri never looks for it anywhere
else.

```zuri,ignore
import .helpers # next to this file
import .models.user # models/ next to this file, then user
import ..shared.config # up one directory, then shared/, then config
import .. ..shared.config # up two directories, then shared/, then config
```

Each `..` goes up one directory, so `.. ..` goes up two, and `.. .. ..` goes
up three. You can also write the dots together, as in `....shared.config`,
which means the same thing: every two dots in a row go up one directory.
`zuri fmt` keeps whichever form you used.

Each part of the path is resolved the same way as a bare name: Zuri tries
`name.zu` first, then `name/index.zu`. So `import .helpers` finds either
`helpers.zu` or `helpers/index.zu`, and `import .models.user` finds
`models/user.zu` or `models/user/index.zu`, where `models` is a directory in
both cases.

You can't tell from the import which of the two it found, and that's what
allows a module to grow into a package. If you split `helpers.zu` into
`helpers/index.zu` and a few other files, every `import .helpers` keeps
working without changes.

Inside a package, always import a sibling module with `import .sibling`, not
with the full path from the project root. If you write
`import myapp.models.user` inside `myapp/models/`, Zuri searches the library
path for it and won't find anything.

## Re-exporting Imports

**Imports are private by default.** If `a.zu` imports `b.zu`, a third file
that imports `a` can't see what's in `b` through it.

To re-export a module, put `@` in front of its path:

```zuri,ignore
import @.util { * }
```

Everything that `util.zu` exports is now part of this module's public API as
well. All three forms of import accept the prefix:

```zuri,ignore
import @.module # the module itself is re-exported
import @.module { item } # just that item
import @.module { * } # everything
```

This is how a package's `index.zu` builds a public API from several private
files:

<span class="filename">Filename: pkg/index.zu</span>

```zuri,ignore
import @.util { * }
import .sub.deep { deep_slug }

def hello() {
  return 'hello from pkg ' + VERSION
}
```

`util`'s members are re-exported, while `deep_slug` is imported for this
file's own use and stays private.

## Private Members

A member whose name starts with `_` is private to its module and can't be
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

`{ * }` skips private members as well. If something is an implementation
detail, give it a leading underscore and the module system will keep it
private.

## Packages

A directory that contains an `index.zu` is a package. Importing the
directory runs its `index.zu`:

```text
pkg/
  index.zu
  util.zu
  sub/
    deep.zu
```

```zuri,ignore
import .pkg # runs pkg/index.zu
import .pkg.util # runs pkg/util.zu
import .pkg.sub.deep # runs pkg/sub/deep.zu
```

The same rule applies to `zuri run pkg` on the command line, which is why a
package can be run as well as imported.

## Resolving Bare Names

For an import with no leading dot, such as `import http`, Zuri looks in
these places, in order:

1. `.zuri/libs/http` in the **project**
2. `$ZURI_ROOT/libs/http`, or `libs/http` next to the `zuri` executable,
   which is the standard library
3. A built-in native module named `http`
4. `$ZURI_HOME/libs/http`, the packages installed for your user, which are
   in `~/.zuri/libs` unless you set `ZURI_HOME` to something else

At each step, `http.zu` is tried first, and then `http/index.zu`.

The project is the nearest directory containing a `project.toml`, found by
searching upward from the script given to `zuri run`, or from the working
directory for a command or the REPL. A program run from anywhere inside a
project imports that project's packages, and a program that isn't in a
project uses `.zuri/libs` in the working directory. The project is found
once, when the program starts, so changing directory while it runs doesn't
change where imports come from.

`zuri install` puts packages in `.zuri/libs`, and [Packages and
Nyssa](ch27-00-packages.md) covers it in full. The first step in the search
is also what makes vendoring possible. A file you put in `.zuri/libs/` takes
priority over a standard library module with the same name:

```console
$ cat .zuri/libs/mylib.zu
def hi() { return 'from user libs' }

$ cat uses.zu
import mylib
echo mylib.hi()

$ zuri run uses.zu
from user libs
```

## Modules Run Only Once

A module's top-level code runs the first time the module is imported, and
never again. Every later import of the same file gets the same module
object:

```zuri,ignore
import .once
import .once as again
import .once
```

```console
side effect ran
```

There's one line of output for three imports. Modules are identified by
their canonical file path, so two different relative paths to the same file
refer to the same module.

That makes a module's top level the natural place for setup that should
happen exactly once, such as opening a connection pool, reading a
configuration file or registering handlers.

## Circular Imports

Circular imports work. A module is registered before its body runs, so when
`b.zu` imports `a.zu` while `a.zu` is still loading, `b` gets the partly
loaded module instead of looping forever:

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

The output shows the catch: `b` finished loading before `a` did, so anything
`b` reads from `a` **at its top level** doesn't exist yet. Reading it inside
a function is fine, because by the time the function runs, `a` has finished
loading.

If a module fails while it's loading, it's removed from the cache instead of
being left half loaded, so a later import really does try again.

## Module Variables

Every module automatically has two variables:

```zuri
echo __file__
echo __root__
```

`__file__` is the canonical path of the module itself. `__root__` is the
entry file the program was started with, and it's the same in every module.
Use them to find files relative to your source code, rather than relative to
whatever directory the user ran the program from:

```zuri
import os

var templates = os.join_paths(os.dir_name(__file__), 'templates')
```

Both are also defined in the REPL, with placeholder values in place of a
real file:

```zuri
%> __file__
@.repl
%> __root__
@.repl.root
```

In the REPL, they're different from each other, so the
`__root__ == __file__` check below is `false` at the prompt. A REPL session
is never the entry point of a program.

### Running a Module as a Program

Since `__root__` is the entry file and `__file__` is the current one,
comparing them tells a module whether it's being run as the program or
imported by something else:

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
$ zuri run tool.zu
running as a program: 5
```

```zuri,ignore
import .tool
echo tool.add(10, 20)
```

```console
30
```

When imported, the same file is a library with no side effects, and when run
directly, it's a command-line program. Put the argument parsing and the
entry point behind that check, and everything else above it.

These two variables describe *which file this is*, so they aren't exported.
A wildcard import copies every public name from the module it imports, but
`__file__` and `__root__` are always left out:

```zuri
import math { * }

echo __file__ == __root__
```

```console
true
```

That's what makes the `__root__ == __file__` check reliable in every file,
even in a file that uses a wildcard import of a sibling module.

## Organizing a Project

A small program can be a single file. Beyond that, a good structure is one
package for each area of responsibility, each with an `index.zu` that
re-exports its public members:

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

Run it with `zuri run myapp`. The capstone project in [Chapter
25](ch25-00-task-board.md) uses exactly this layout.

### A Complete Package

Here's the smallest complete example of that structure: three files, one
package and one public entry point.

<span class="filename">Filename: greet/english.zu</span>

```zuri,ignore
def hello(name) {
  return 'Hello, ${name}'
}

def _shout(text) {
  return text.upper()
}
```

<span class="filename">Filename: greet/french.zu</span>

```zuri,ignore
def hello(name) {
  return 'Bonjour, ${name}'
}
```

<span class="filename">Filename: greet/index.zu</span>

```zuri,ignore
import @.english
import @.french

def greet(name, language) {
  return language == 'fr' ? french.hello(name) : english.hello(name)
}
```

<span class="filename">Filename: index.zu</span>

```zuri,ignore
import .greet

echo greet('Ada', 'en')
echo greet('Ada', 'fr')
echo greet.english.hello('Grace')
```

```console
$ zuri run
Hello, Ada
Bonjour, Ada
Hello, Grace
```

There are four things to notice.

**`greet/index.zu` is what `import .greet` loads.** A directory with an
`index.zu` is a package, and importing the directory runs that file.

**The `@` in `import @.english` is what re-exports the module.** Without it,
`greet.english` wouldn't be accessible outside `greet/index.zu`, although
`greet()` itself would still work, which is often what you want.

**Both submodules define `hello`, and they don't clash.** Each one lives in
its own namespace and is accessed through its own module name. That's the
reason to use `import @.english` here rather than `import @.english { * }`.

**`_shout` can't be accessed from outside.** Writing
`greet.english._shout(x)` anywhere else is a compile-time error, not a
run-time one:

```console
SyntaxError: '_shout' is private and can only be accessed via 'self' or 'parent'
```

The leading underscore is the only way to declare something private, and
it's checked before the program runs.
