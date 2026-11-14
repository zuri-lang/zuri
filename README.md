<h1 align="center">
  <img src="docs/site/zuri-butterfly-large.svg" alt="Zuri" width="420">
</h1>

<p align="center">
  <strong>One language, one binary, the entire development lifecycle.</strong>
</p>

<p align="center">
  Stop learning an entire constellation of third-party tools just to
  build on the web.
</p>

<p align="center">
  <a href="https://zuri-lang.github.io/zuri/">Website</a>
  &nbsp;&middot;&nbsp;
  <a href="#install">Install</a>
  &nbsp;&middot;&nbsp;
  <a href="https://zuri-lang.github.io/zuri/book">The Book</a>
  &nbsp;&middot;&nbsp;
  <a href="https://zuri-lang.github.io/zuri/reference">Standard Library</a>
  &nbsp;&middot;&nbsp;
  <a href="LICENSE">BSD 2-Clause</a>
</p>

<p align="center">
  The runtime, a standard library for the whole stack, a JIT compiler,
  a test runner, a formatter, a language server, a package manager, a
  package registry and a bundler that ships your program to machines
  without Zuri. One download.
</p>

## Install

```sh
curl -fsSL https://zuri-lang.github.io/zuri/install.sh | sh
```

On Windows, in PowerShell:

```powershell
powershell -c "irm https://zuri-lang.github.io/zuri/install.ps1 | iex"
```

Releases cover Linux and macOS on x86-64 and ARM, and Windows on
x86-64. On Linux there is a build for glibc 2.28 or later and one for
musl, as on Alpine. `zuri upgrade` keeps an installation current, and
[Installation](docs/book/src/ch01-01-installation.md) covers choosing
a version and building from source.

## From `zuri init` to a Shipped Executable

The whole lifecycle of a project, with nothing installed but Zuri:

```console
$ zuri init weather --yes       # a project, with tests and a README
$ cd weather
$ zuri install thermo           # a dependency, resolved into a lockfile
$ zuri run                      # run it
$ zuri run -w                   # and again on every change
$ zuri test                     # every test file, each in its own process
$ zuri fmt                      # one house style
$ zuri bundle --format exe      # one file that runs where Zuri is not installed
$ zuri publish                  # share it
```

## Philosophy

Software creation has become buried beneath endless layers of glue code,
external configs and shifting package ecosystems. Zuri brings the focus
back to writing software: one cohesive platform, one documentation site,
and zero framework fatigue.

## The Ecosystem Tax

When you pick up a modern language, you do not just learn that language.

To serve a single web page or persist a record, you must research,
evaluate and configure dozens of disconnected third-party libraries. You
learn a package manager. You choose an HTTP framework, select an ORM,
find a validation library, wire up a template engine, pick a test
runner. Every piece comes from a different author, follows conflicting
conventions, and keeps its own documentation in its own shape.

You spend twenty percent of your time learning core concepts and eighty
percent wrestling with arbitrary tool churn.

**Zuri removes that tax.** The runtime, the server, the data layer and
the utility toolchain were designed together and speak the same
conceptual dialect. When you learn Zuri, you already know the stack.

## Everything Speaks the Same Dialect

This is a whole application. Every import below is in the box: nothing
was installed, nothing was resolved, nothing was configured.

```zuri
import http
import validate
import log

var signups = validate.schema({
  name: validate.required().string().max_length(60),
  email: validate.required().string().email(),
})

var server = http.server(3000)

server.post('/signup', @(request, response) {
  var body = request.json_body() or {}
  var checked = signups.check(body)

  if !checked.valid {
    return response.json({ errors: checked.errors }, 422)
  }

  log.info('signed up ${body.email}')
  response.json({ ok: true }, 201)
})

server.listen()
```

There is one binary and the library that ships beside it. Zuri has
first-class language-level support for package management and vendoring,
so the code you bring in from outside needs no third-party tooling
either.

Every module in it was built by the same hands:

| | |
| --- | --- |
| **The web** | `http` (server, client, HTTP/2, WebSocket), `wire` (templates), `html` (HTML5 parser, CSS selectors), `url`, `mime` |
| **Mail** | `mail` (SMTP, IMAP and POP3, clients and servers) |
| **Networking** | `net` (TCP, UDP, TLS, DTLS, addresses, polling), `rpc` (JSON-RPC 2.0) |
| **Data** | `sql` (SQLite, PostgreSQL, MySQL, MariaDB), `json`, `yaml`, `toml`, `csv`, `struct`, `base64`, `convert` |
| **Correctness** | `validate`, `types`, `enum` |
| **Security** | `crypto`, `hash`, `bcrypt`, `jwt`, `uuid` |
| **The machine** | `os`, `env`, `io`, `args`, `log`, `stat`, `date` (with the IANA time zones) |
| **Concurrency** | `isolate` (real OS threads, separate heaps, message passing) |
| **Numbers** | `math`, `array` (typed arrays), `set` |
| **Archives** | `compress` (gzip, zlib, deflate, bzip2, brotli, zstd, lz4, tar, zip) |
| **Images and colour** | `imagine` (decode, draw, filter, encode), `colors` (colour spaces, terminal colour) |
| **Interop** | `ffi` (C and Rust libraries, callbacks) |
| **Testing** | `test` (suites, expectations, mocks and spies) |
| **Metaprogramming** | `zuri` (lexer, parser, compiler, reflection) |

Because every one of them was designed together, moving between a
database query, a hash and an HTTP response costs you no mental
friction. The naming is the same. The error types are the same. The
documentation is one book.

## The Toolchain Is in the Binary

Every tool below ships inside `zuri`, was designed alongside the
language it serves, and is documented in the same book.

| Command | What it gives you |
| --- | --- |
| `zuri` | the REPL |
| `zuri run` | runs a script, a package, or a whole directory; with `-w`, runs it again whenever a file it uses changes |
| `zuri init` | starts a project: a manifest, an app module, a test and a README |
| `zuri test` | runs the tests, every file in a process of its own, as many at once as `--jobs` allows |
| `zuri fmt` | lays out Zuri source in one house style; `--dry-run` is the check a commit hook or CI job runs |
| `zuri lsp` | a language server: completion, navigation, diagnostics, refactoring and test running in any editor |
| `zuri install`, `update`, `restore`, `uninstall` | dependencies from version ranges, resolved into a lockfile with one version of every package |
| `zuri publish`, `yank`, `owner`, `account` | sharing your own packages on a registry |
| `zuri serve` | Nyssa, a complete package repository with its own website |
| `zuri bundle` | your program and the runtime, as a directory, an archive, a single executable or a macOS app |
| `zuri upgrade` | moves the installation to a newer release, checked against its published checksum |
| `zuri <yours>` | commands of your own, from a project's `.zuri/cmds` or from a package |

Every one of them answers `--help`, and every package command that
changes a project shows exactly what it would do with `--dry-run`.

## Ship It Anywhere

`zuri bundle` packs your program, the packages it uses, the standard
library and the runtime into one thing you can hand to anyone. It runs
the same on every machine it lands on, with nothing to install first.

```console
$ zuri bundle --format exe
Built /home/ada/weather/dist/weather-0.1.0-x86_64-unknown-linux-gnu
$ ./dist/weather-0.1.0-x86_64-unknown-linux-gnu
Lagos: 31°C, 87.8°F
```

| Format | What it makes |
| --- | --- |
| `archive` | a `.tar.gz`, or a `.zip` for Windows |
| `dir` | the bundle directory, ready for a container image |
| `exe` | a single executable |
| `app` | a macOS application |

`zuri bundle --target all` builds for Linux and macOS on x86-64 and
ARM, and Windows on x86-64, from whichever of them you are on.
Single-file macOS bundles come out signed ad hoc, which is all an Apple
silicon Mac needs to run them, and
[Bundles and Upgrades](docs/book/src/ch27-05-bundles.md) walks through
Developer ID signing and notarization.

## Packages and Nyssa

When you share code, the package manager is already installed, and so
is the registry it talks to.

```console
$ zuri install thermo
Resolving dependencies
Installing thermo 0.1.0
  + thermo 0.1.0
Installed 1 package.
```

**Nyssa** is the package repository in every installation: the API the
package commands talk to, and a website for finding packages, reading
their documentation and managing accounts and tokens. `zuri serve`
starts one for a team, a company or the public, on SQLite with nothing
to set up, or on PostgreSQL, MySQL or MariaDB, with HTTPS, closed
sign-up, a read-only mode and live backups.

```console
$ zuri serve
Nyssa is serving http://127.0.0.1:3000
listening on 127.0.0.1:3000, storage in /home/ada/.zuri/nyssa
```

[Packages and Nyssa](docs/book/src/ch27-00-packages.md) covers all of it.

## Language Design

Zuri takes the expressive, familiar syntax of modern dynamic languages
and adds deliberate structural controls. It is dynamically typed, and it
declines to be vague.

Zuri features true access controls with sealed classes, function 
parameter type guards, private methods and modules, all enforced by the 
compiler. The point of each is the same. **The things that are hard to see 
when reading code are the things the language refuses to let you get 
wrong**.

Zuri features a fast Just-In-Time (JIT) compiler driven underneath by Cranelift 
&mdash; The same engine that drives `wasmtime`, with cutting edge 
specialization and optimized to production workloads.

## Documentation

[`docs/`](docs) holds everything written about the language.

- [**The Zuri Programming Language**](https://zuri-lang.github.io/zuri/book) 
  is the main text: it covers everything from installing to publishing,
  plus appendices covering the keywords, the operators and every method
  on every built-in type.
- [**The Zuri Standard Library**](https://zuri-lang.github.io/zuri/reference) 
  is the reference: every module, every public name, generated from the library's own doc blocks so a page and the code it describes can never disagree.

Read the book on [The Zuri website](https://zuri-lang.github.io/zuri/) as it 
is, or render either locally if you have `cargo` installed:

```console
$ cargo run-docs                # the book
$ cargo run-docs -- reference   # the standard library
```

## Editor Support

`zuri lsp` is a full language server, written in Zuri on the compiler's
own parser, so what it reports is what the runtime would do with the
same code. Completion with the import added for you, diagnostics as you
type with the runtime's own messages, go to definition, references, call
and type hierarchies, rename across the workspace, extract and inline
refactorings, hover documentation, formatting, and tests run from the
line they are on.

- **[Open VSX](https://open-vsx.org/extension/zuri-lang/zuri-vscode)** &mdash;
  the current build of the extension, which sets everything up. This is
  the one to install: VS Codium, Cursor, Windsurf, Gitpod and Eclipse
  Theia all pull from here.
- **[VS Code Marketplace](https://marketplace.visualstudio.com/items?itemName=zuri-lang.zuri-vscode)**
  &mdash; the same extension for VS Code itself, and it trails the Open VSX
  build.
- **Neovim, Helix, Sublime Text and Emacs** take a few lines of
  configuration each, given in
  [Editor Support](docs/book/src/ch29-00-editors.md).

## Roadmap

Already here:

- [x] The language, complete and self-hosting enough to parse, compile
      and reflect on itself.
- [x] A Just-In-Time compiler.
- [x] A standard library covering the whole stack.
- [x] Two books, one of them generated from the library itself.
- [x] **Databases**: SQLite, PostgreSQL, MySQL and MariaDB behind one
      interface.
- [x] **A test runner** and **a formatter.**
- [x] **A language server** for every editor that speaks LSP.
- [x] **C and Rust compatible FFI interop.**
- [x] **Package management**, for the code that is genuinely
      third-party: `zuri install`, `publish`, `update`, `restore` and the
      rest ship with the runtime, with a lockfile and one version of
      every package. A complete standard library is not an argument
      against sharing.
- [x] **Nyssa, a self-hosted repository server** in every installation,
      `zuri serve`, so public and private organizations share Zuri
      packages whichever way they like.
- [x] **Bundles** that allows shipping a program to run where Zuri is 
      not installed, as a directory, an archive or a single executable.
- [x] **An ORM**, so that persisting a model never means reaching outside
      the language.

On the way, in the order it matters:

- [ ] **HTTP/3.** The `http` module speaks HTTP/1.1 and HTTP/2 today.

## AI Involvement

AI wrote code in this repository, and how much, where, and how well it
went are documented rather than glossed over. **[AI.md](AI.md)** is that
account, and it ends in a rule that contributors are held to.

## License

BSD 2-Clause. See [LICENSE](LICENSE).
