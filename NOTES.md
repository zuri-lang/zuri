# Notes on the new state of the Zuri programming language.

- Numbers and Booleans are now objects with methods. Numbers carry methods such as `log`, `sin`, `abs`, etc.
- All objects and classes now carry a default `to_string()` method that can be overridden by their implementations.
- Builtin functions such as `to_string`, `to_number`, etc have been made redundant new features and have been removed.
- Anonymous functions now have an alternate syntax that uses the `def` keyword directly instead of using the `@` sign which has now been marked as a shorthand. i.e. We can now have `def() { ... }` instead of `@(){ ... }` and `def {}` instead of `@{}` where the `@` notation now becomes the shorthand.
- The class constructor has been replaced by the `@new` decorated method instead of a method with the same name. 
- Classes are now partially-immutable. Once created, new fields and methods cannot be added at runtime. However, their static properties can continue to be mutated.
- Class operator override have now been replaced and no longer have their own distinct syntax. Now, they reuse the decorated methods such as `@add`, `@sub`, `@mul`, `@div`, `@lshift` etc.
- Imports are now local by default and you'll have to explicity specify that you intend to export them out of the importing module by prefixing the import path with the `@` symbol. E.g. `import @.module`, `import @.module { item }`, or `import @.module { * }` where module, item, and all items become exported by the current module respectively.
- `Exception` has been renamed to `Error`.
- The `zlib` module has been dropped in favor of the `compress` module which includes support for `deflate`, `gzip`, and `zlib` format that are currently supported and introduces support for `zstd`.
- The `zip` module has been moved under the `compress` module.
- The `ast` and the `reflect` module has been collapsed into a single module called `zuri`.
- `template` module has been renamed and moved to `wire` module.
- The `socket` module and `ssl` module has been dropped in favor of the `net` module.
- The `postgres` and `sqlite` module have been replaced with the respective submodules of the new `sql` module. The `sql`.
- The `curl` module has been removed from Zuri.
- The `env` module has been added to the standard library. It reads a `.env` file into the process environment and reads values back out converted to the type the program wants.
- The executable no longer runs a bare path. `zuri` alone is still the REPL, `zuri run [path]` runs a script, a package directory, or the working directory's own `index.zu`, and any other first word names a command. Commands are a directory with an `index.zu` or a `.zu` file, looked up in that order, resolved from the `cmds` directory beside the runtime first and a project's `.zuri/cmds` second. Everything after the path or the command name is forwarded to the program, so `os.args` reads the same in both cases. `zuri --version` reports the build and `zuri --help` adds the commands available from here, both taking a short spelling (`-v`, `-h`) and running nothing. A command names and describes itself with `@command` and `@description` in its doc block, which is what the listing reads. The banner, the usage lines and the listing headings are coloured the same way a command's own `args` help is, and everything the runtime prints goes plain when `NO_COLOR` is set or the stream is not a terminal.
- The `http` module now has server-side sessions. `http.session.session()` is middleware: it finds the session named by a cookie, hands it to the handler through `request.session()`, and writes it back when the response goes out. A request that never touches its session reads nothing, writes nothing and sets no cookie; the record exists from the first write. Sessions carry values, a one-request flash, `regenerate()` for the identifier rotation a sign-in needs, and `destroy()` for signing out. They expire on two clocks, a rolling idle timeout and an absolute lifetime. Storage is a `SessionStore`: `FileStore` (the default, in a private `0700` directory under the platform's temporary directory), `MemoryStore` for tests, and `SqlStore` from `http.session.sql`, which is a separate import so the `sql` module is not loaded by a server that does not want it. The store is keyed by the SHA-256 of the identifier rather than the identifier, so its contents cannot be replayed as cookies, and setting a `secret` signs the cookie so a forged one is refused before the store is touched.
- The runtime ships two commands. `zuri format` lays out Zuri source in the project's style, and `zuri test` runs a project's test files, each in its own process. `zuri test` is `test.conduct()` behind a command line: with no argument it runs the `tests` directory it was called from, with one it runs a single file (`zuri test mytest` and `zuri test mytest.zu` both reach `tests/mytest.zu`) or a whole directory anywhere in the project, and every option `conduct()` takes is a flag. A project no longer writes a `tests/index.zu` to get a suite.


# CHANGELOGS

- [12-09-2026]
  - Change repository visibility from Private to Public.
- [15-09-2026]
  - Changed repository visibility back to Private. Not ready to become public until Nyssa or at least a prototype of it is ready.
