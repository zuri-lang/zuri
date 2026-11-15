<h1 align="center">
  <img src="docs/site/zuri-butterfly-large.svg" alt="Zuri" width="420">
</h1>

<p align="center">
  <strong>One language, one binary, the entire development lifecycle.</strong>
</p>

<p align="center">
  A dynamic language for building web applications, with the standard
  library and the tools you need included in the install.
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

Every Zuri installation comes packed with the runtime and its JIT
compiler, a standard library that covers everything from HTTP to
databases to image processing, and the tools you would normally install
one by one: a test runner, a formatter, a language server, a package
manager, a package registry and a bundler for shipping your program to
machines that don't have Zuri installed.

## Install

On Linux and macOS:

```sh
curl -fsSL https://zuri-lang.github.io/zuri/install.sh | sh
```

On Windows, in PowerShell:

```powershell
powershell -c "irm https://zuri-lang.github.io/zuri/install.ps1 | iex"
```

Prebuilt releases are available for Linux and macOS on x86-64 and ARM,
and for Windows on x86-64. There are two Linux builds: one for glibc 2.28
or later, and one for musl-based systems such as Alpine.

Run `zuri upgrade` to update Zuri itself. The
[installation guide](docs/book/src/ch01-01-installation.md) explains how
to install a specific version or build Zuri from source.

## A Quick Tour

Here is the life of a project, from creating it to shipping it, using
nothing but the `zuri` command:

```console
$ zuri init weather --yes       # create a project with a test and a README
$ cd weather
$ zuri install thermo           # add a dependency
$ zuri run                      # run the project
$ zuri run -w                   # run it again whenever a file changes
$ zuri test                     # run the tests
$ zuri fmt                      # format the code
$ zuri bundle --format exe      # build a standalone executable
$ zuri publish                  # publish the package to a registry
```

## Why Zuri?

In most languages, the language is only the start. Before you can serve
a web page or save a record to a database, you have to choose a package
manager, an HTTP framework, an ORM, a validation library, a template
engine and a test runner. Each one comes from a different author, has
its own conventions and its own documentation, and you spend more time
learning how they fit together than learning the language.

Zuri is built the other way around. The runtime, the HTTP server, the
database layer and the tooling were designed together. They follow the
same naming conventions, report errors the same way, and are documented
in one book and one reference.

## Example

The following program is a small web service that validates sign-ups.
Everything it imports is part of the standard library, so you can run it
straight after installing Zuri.

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

When you do need code from outside the standard library, package
management is built into the language and the `zuri` command, along with
support for vendoring, so you don't need any other tools for that either.

## The Standard Library

| Area | Modules |
| --- | --- |
| **Web** | `http` (server, client, HTTP/2, WebSocket), `wire` (templates), `html` (HTML5 parser, CSS selectors), `url`, `mime` |
| **Mail** | `mail` (SMTP, IMAP and POP3 clients and servers) |
| **Networking** | `net` (TCP, UDP, TLS, DTLS, addresses, polling), `rpc` (JSON-RPC 2.0) |
| **Data** | `sql` (SQLite, PostgreSQL, MySQL, MariaDB), `json`, `yaml`, `toml`, `csv`, `struct`, `base64`, `convert` |
| **Validation and types** | `validate`, `types`, `enum` |
| **Security** | `crypto`, `hash`, `bcrypt`, `jwt`, `uuid` |
| **System** | `os`, `env`, `io`, `args`, `log`, `stat`, `date` (with IANA time zones) |
| **Concurrency** | `isolate` (OS threads with separate heaps and message passing) |
| **Numbers and collections** | `math`, `array` (typed arrays), `set` |
| **Compression** | `compress` (gzip, zlib, deflate, bzip2, brotli, zstd, lz4, tar, zip) |
| **Images and color** | `imagine` (decoding, drawing, filters, encoding), `colors` (color spaces, terminal colors) |
| **Interop** | `ffi` (C and Rust libraries, callbacks) |
| **Testing** | `test` (suites, expectations, mocks and spies) |
| **Metaprogramming** | `zuri` (lexer, parser, compiler, reflection) |

Because these modules were written together, switching from a database
query to a hash to an HTTP response doesn't mean switching styles. The
names, the error types and the documentation all follow the same
patterns.

## The Toolchain

The whole toolchain is built into the `zuri` binary, so there's nothing
extra to install.

| Command | Description |
| --- | --- |
| `zuri` | Starts the interactive REPL. |
| `zuri run` | Runs a script, a package or a directory. With `-w`, runs it again whenever a file it uses changes. |
| `zuri init` | Creates a new project with a manifest, an app module, a test and a README. |
| `zuri test` | Runs the tests. Each test file runs in its own process, and `--jobs` sets how many run at once. |
| `zuri fmt` | Formats Zuri source code. Use `--dry-run` in a commit hook or CI job to check formatting without changing anything. |
| `zuri lsp` | Starts the language server, which provides completion, navigation, diagnostics, refactoring and test running. |
| `zuri doc` | Builds a documentation site from Markdown and an API reference from doc comments. |
| `zuri migrate` | Applies and rolls back database migrations. |
| `zuri install`, `update`, `restore`, `uninstall` | Manage dependencies. Version ranges are resolved into a lockfile that pins one version of each package. |
| `zuri publish`, `yank`, `owner`, `account` | Publish and manage your packages on a registry. |
| `zuri serve` | Runs Nyssa, a package registry with its own website. |
| `zuri bundle` | Packages your program and the runtime as a directory, an archive, a single executable or a macOS app. |
| `zuri upgrade` | Upgrades Zuri to a newer release, after checking it against the published checksum. |
| `zuri <name>` | Runs a custom command from the project's `.zuri/cmds` directory or from an installed package. |

Every command accepts `--help`. The package commands that modify a
project also accept `--dry-run`, which prints what would change without
changing it.

## Shipping Your Program

`zuri bundle` packages your program together with its dependencies, the
standard library and the runtime. The result runs on machines that don't
have Zuri installed.

```console
$ zuri bundle --format exe
Built /home/ada/weather/dist/weather-0.1.0-x86_64-unknown-linux-gnu
$ ./dist/weather-0.1.0-x86_64-unknown-linux-gnu
Lagos: 31°C, 87.8°F
```

| Format | Output |
| --- | --- |
| `archive` | A `.tar.gz` file, or a `.zip` file for Windows |
| `dir` | A plain directory, useful for container images |
| `exe` | A single executable |
| `app` | A macOS application |

You can build for any supported platform from any other with
`zuri bundle --target`, and `--target all` builds for all of them at
once. Single-file macOS bundles are signed ad hoc, which is enough for
them to run on Apple silicon. For Developer ID signing and notarization,
see [Bundles and Upgrades](docs/book/src/ch27-05-bundles.md).

## Packages and Nyssa

The package manager is part of the `zuri` command:

```console
$ zuri install thermo
Resolving dependencies
Installing thermo 0.1.0
  + thermo 0.1.0
Installed 1 package.
```

The registry it talks to is part of every installation too. **Nyssa**
serves the API that the package commands use, along with a website where
people can browse packages, read their documentation and manage their
accounts and API tokens. Run `zuri serve` to host your own, whether for a
team, a company or the public:

```console
$ zuri serve
Nyssa is serving http://127.0.0.1:3000
listening on 127.0.0.1:3000, storage in /home/ada/.zuri/nyssa
```

Nyssa uses SQLite by default, so there's nothing to set up before you
start it. It can also run on PostgreSQL, MySQL or MariaDB, and it
supports HTTPS, closing public sign-up, a read-only mode and backups of
a live database. See [Packages and Nyssa](docs/book/src/ch27-00-packages.md)
for the details.

## The Language

Zuri's syntax will look familiar if you've used JavaScript, Python or
Ruby. It's dynamically typed, but the compiler checks several things
that dynamic languages usually leave to discipline:

- Classes are sealed. Assigning to a field the class doesn't declare is
  an error, so a misspelled field name can't silently create a new one.
- Parameters can declare a type, and the type is checked when the
  function is called.
- Methods, functions and module members whose names start with an
  underscore are private, and code outside can't reach them.

Code starts out as bytecode. When part of a program runs often, a tiered
JIT compiler built on Cranelift (the code generator used by Wasmtime)
compiles it to machine code in the background, specializing it for the
types it actually sees. You don't need annotations or flags for any of
this.

## Documentation

- [**The Zuri Programming Language**](https://zuri-lang.github.io/zuri/book)
  is the main guide. It starts with installation and goes all the way to
  publishing packages, and its appendices cover the keywords, the
  operators and every method of the built-in types.
- [**The Zuri Standard Library**](https://zuri-lang.github.io/zuri/reference)
  is the API reference for every module. It's generated from the doc
  comments in the library source, so it always matches the code.

Both live in [`docs/`](docs) and are published on the
[website](https://zuri-lang.github.io/zuri/). To build and browse them
locally, you need `cargo`:

```console
$ cargo run-docs                # the book
$ cargo run-docs -- reference   # the standard library reference
```

## Editor Support

`zuri lsp` is a complete language server. It's written in Zuri and uses
the compiler's own parser, so the errors it shows in your editor are the
same ones the runtime would report. It supports completion (adding
imports for you), diagnostics as you type, go to definition, find
references, call and type hierarchies, rename across the workspace,
extract and inline refactorings, hover documentation, formatting, and
running a test from the line it's on.

- **[Open VSX](https://open-vsx.org/extension/zuri-lang/zuri-vscode)**:
  the recommended way to get the extension, which sets everything up for
  you. VSCodium, Cursor, Windsurf, Gitpod and Eclipse Theia all install
  extensions from Open VSX.
- **[VS Code Marketplace](https://marketplace.visualstudio.com/items?itemName=zuri-lang.zuri-vscode)**:
  the same extension for Visual Studio Code. New versions may appear here
  a little later than on Open VSX.
- **Neovim, Helix, Sublime Text and Emacs** each need a few lines of
  configuration, described in
  [Editor Support](docs/book/src/ch29-00-editors.md).

## Roadmap

Done:

- [x] The language, including a `zuri` module that can parse, compile
      and inspect Zuri code.
- [x] A tiered JIT compiler.
- [x] A standard library that covers the full web stack.
- [x] Two books: a guide to the language, and a standard library
      reference generated from the source.
- [x] **Databases**: SQLite, PostgreSQL, MySQL and MariaDB through one API.
- [x] **A test runner** and **a formatter**.
- [x] **A language server** for any editor that supports LSP.
- [x] **FFI** for calling C and Rust libraries.
- [x] **Package management** built into the runtime: `zuri install`,
      `publish`, `update`, `restore` and related commands, with a
      lockfile that pins one version of each package.
- [x] **Nyssa**, a self-hosted package registry included in every
      installation (`zuri serve`), for public or private use.
- [x] **Bundles** for running programs on machines without Zuri, as a
      directory, an archive or a single executable.
- [x] **An ORM** for saving and loading models.

Planned, roughly in order of priority:

- [ ] **HTTP/3.** The `http` module currently supports HTTP/1.1 and HTTP/2.
- [ ] **A public package registry** at pub.zurilang.org.
- [ ] **A debugger** with breakpoints and stepping, for editors that support DAP.
- [ ] **A profiler** for finding where a program spends its time and memory.
- [ ] **OAuth 2** in the standard library, for signing in with other services.
- [ ] **Telemetry**: exporting traces and metrics over OpenTelemetry.
- [ ] **Job scheduling** for running tasks at set times or intervals.
- [ ] **A GitHub Action** for installing Zuri in CI workflows.

## Use of AI

Parts of this repository were written with the help of AI tools.
[AI.md](AI.md) describes where and how much, what worked and what
didn't, and the rule that contributors are expected to follow as a
result.

## License

Zuri is released under the BSD 2-Clause license. See [LICENSE](LICENSE).
