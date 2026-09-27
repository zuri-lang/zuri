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
  <a href="https://zuri-lang.github.io/zuri-rs/">Website</a>
  &nbsp;&middot;&nbsp;
  <a href="https://zuri-lang.github.io/zuri-rs/book">The Book</a>
  &nbsp;&middot;&nbsp;
  <a href="https://zuri-lang.github.io/zuri-rs/reference">Standard Library</a>
  &nbsp;&middot;&nbsp;
  <a href="LICENSE">BSD 2-Clause</a>
</p>

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
| **The web** | `http` (server, client, HTTP/2, WebSocket), `wire` (templates), `html`, `mail` (SMTP, IMAP, POP3), `url`, `mime` |
| **Networking** | `net` (TCP, UDP, TLS, DTLS, addresses, polling) |
| **Data** | `sql` (SQLite, PostgreSQL, MySQL, MariaDB), `json`, `yaml`, `csv`, `struct`, `base64`, `convert` |
| **Correctness** | `validate`, `types`, `enum` |
| **Security** | `crypto`, `hash`, `bcrypt`, `jwt`, `uuid` |
| **The machine** | `os`, `env`, `io`, `args`, `log`, `date` |
| **Concurrency** | `isolate` (real OS threads, separate heaps, message passing) |
| **Numbers** | `math`, `stat`, `array`, `set` |
| **Archives** | `compress` (gzip, zlib, deflate, bzip2, brotli, zstd, lz4, tar, zip) |
| **Images** | `imagine` (decode, draw, filter, encode) |
| **Unit Testing** | `test` |

Because every one of them was designed together, moving between a
database query, a hash and an HTTP response costs you no mental
friction. The naming is the same. The error types are the same. The
documentation is one book.

## Language Design

Zuri takes the expressive, familiar syntax of modern dynamic languages
and adds deliberate structural controls. It is dynamically typed, and it
declines to be vague.

Zuri features true access controls with sealed classes, function 
parameter type guards, private methods and modules, all enforced by the 
compiler. The point of each is the same. The things that are hard to see 
when reading code are the things the language refuses to let you get 
wrong.

Zuri features a fast Just-In-Time (JIT) compiler driven underneath by Cranelift 
&mdash; The same engine that drives `wasmtime`, with cutting edge 
specialization and optimized to production workloads.

## Documentation

[`docs/`](docs) holds everything written about the language.

- [**The Zuri Programming Language**](https://zuri-lang.github.io/zuri-rs/book) 
  is the main text: twenty chapters from installing it to a full-stack web
  application, plus appendices covering the keywords, the operators and
  every method on every built-in type.
- [**The Zuri Standard Library**](https://zuri-lang.github.io/zuri-rs/reference) 
  is the reference: every module, every public name, generated from the library's own doc blocks so a page and the code it describes can never disagree.

Read the book on [The Zuri website](https://zuri-lang.github.io/zuri-rs/) as it 
is, or render either locally if you have `cargo` installed:

```console
$ cargo run-docs                # the book
$ cargo run-docs -- reference   # the standard library
```

## Editor Support

Syntax highlighting, from the same hands as everything else.

- **[Open VSX](https://open-vsx.org/extension/zuri-lang/zuri-vscode)** &mdash;
  the current build. This is the one to install: VS Codium, Cursor,
  Windsurf, Gitpod and Eclipse Theia all pull from here.
- **[VS Code Marketplace](https://marketplace.visualstudio.com/items?itemName=zuri-lang.zuri-vscode)**
  &mdash; the same extension for VS Code itself, and it trails the Open VSX
  build.

## Roadmap

Already here:

- [x] The language, complete and self-hosting enough to parse, compile
      and reflect on itself.
- [x] A Just-In-Time compiler.
- [x] A standard library covering the whole stack bar the data layer.
- [x] Two books, one of them generated from the library itself.
- [x] **Database & ORM**, so that persisting a record never means reaching
      outside the language.
- [x] **A test runner.**
- [x] **C and Rust compatible FFI interop.**

On the way, in the order it matters:

- [ ] **Nyssa package manager**, for the code that is genuinely
      third-party. A complete standard library is not an argument
      against sharing.
- [ ] **A self-hosted repository server** bundled with `Nyssa` that allows 
      public and private organizations to share Zuri packages whichever way 
      they like.
- [ ] **HTTP/3.** The `http` module speaks HTTP/1.1 and HTTP/2 today.

## AI Involvement

AI wrote code in this repository, and how much, where, and how well it
went are documented rather than glossed over. **[AI.md](AI.md)** is that
account, and it ends in a rule that contributors are held to.

## License

BSD 2-Clause. See [LICENSE](LICENSE).
