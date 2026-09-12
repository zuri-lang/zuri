# Zuri

**One language, one binary, the entire development lifecycle.**

Stop learning an entire constellation of third-party tools just to build
on the web.

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

There is no `package.json`, no lockfile, no vendor directory. There is
one binary and the library that ships beside it.

Every module in it was built by the same hands:

| | |
| --- | --- |
| **The web** | `http` (server, client, HTTP/2, WebSocket), `wire` (templates), `html`, `url`, `mime` |
| **Networking** | `net` (TCP, UDP, TLS, DTLS, addresses, polling) |
| **Data** | `json`, `yaml`, `csv`, `struct`, `base64`, `convert` |
| **Correctness** | `validate`, `types`, `enum` |
| **Security** | `crypto`, `hash`, `bcrypt`, `jwt`, `uuid` |
| **The machine** | `os`, `io`, `args`, `log`, `date` |
| **Concurrency** | `isolate` (real OS threads, separate heaps, message passing) |
| **Numbers** | `math`, `stat`, `array`, `set` |
| **Archives** | `compress` (gzip, zlib, deflate, bzip2, brotli, zstd, lz4, tar, zip) |
| **Images** | `imagine` (decode, draw, filter, encode) |
| **Itself** | `zuri` (the lexer, parser, compiler and reflection, as a library) |

Because every one of them was designed together, moving between a
database query, a hash and an HTTP response costs you no mental
friction. The naming is the same. The error types are the same. The
documentation is one book.

## Language Design

Zuri takes the expressive, familiar syntax of modern dynamic languages
and adds deliberate structural controls. It is dynamically typed, and it
declines to be vague.

**A function cannot be quietly redefined.**

```console
SyntaxError: multiple declaration for function 'greet' found
```

**A declared parameter type is enforced at the call**, not documented
and hoped for.

```console
TypeError: twice() expects parameter 'n' (argument 1) to be a number, got string
```

**Classes are sealed.** The fields a class declares are the fields it
has, so a typo is an error rather than a new attribute.

```console
PropertyError: undefined field 'y' on instance of 'Point'
```

**A leading underscore is private, and the compiler enforces it** across
a module boundary. Not a convention, not a linting rule: an attempt does
not compile.

```console
SyntaxError: Cannot import private items from module
```

The point of each is the same. The things that are hard to see when
reading code are the things the language refuses to let you get wrong.

## Under the Hood

A register-based virtual machine, a Cranelift JIT with on-stack
replacement, and a generational garbage collector.

## Documentation

[`docs/`](docs) holds everything written about the language.

- [**The Zuri Programming Language**](docs/book/src/SUMMARY.md) is the
  main text: twenty chapters from installing it to a full-stack web
  application, plus appendices covering the keywords, the operators and
  every method on every built-in type.
- **The Zuri Standard Library** is the reference: every module, every
  public name, generated from the library's own doc blocks so a page and
  the code it describes can never disagree.

Read the book on GitHub as it is, or render either locally:

```console
$ cargo run-docs                # the book
$ cargo run-docs -- reference   # the standard library
```

## Editor Support

Syntax highlighting, from the same hands as everything else.

- **[Open VSX](https://open-vsx.org/extension/zuri-lang/zuri-vscode)** —
  the current build. This is the one to install: VS Codium, Cursor,
  Windsurf, Gitpod and Eclipse Theia all pull from here.
- **[VS Code Marketplace](https://marketplace.visualstudio.com/items?itemName=zuri-lang.zuri-vscode)**
  — the same extension for VS Code itself, and it trails the Open VSX
  build.

## Roadmap

On the way, in the order it matters:

- [ ] **A data layer**, so that persisting a record never means reaching
      outside the language.
- [ ] **A package manager**, for the code that is genuinely
      third-party. A complete standard library is not an argument
      against sharing.
- [ ] **A self-hosted repository server** to serve it.
- [ ] **A test runner.** `assert` is a built-in; a runner that finds,
      groups and reports on tests is not.
- [ ] **HTTP/3.** The `http` module speaks HTTP/1.1 and HTTP/2 today.
- [ ] **C and Rust compatible FFI interop.**

Already here:

- [x] The language, complete and self-hosting enough to parse, compile
      and reflect on itself
- [x] A Just-In-Time compiler
- [x] A standard library covering the whole stack bar the data layer
- [x] Two books, one of them generated from the library itself

## AI Involvement

AI wrote code in this repository, and how much, where, and how well it
went are documented rather than glossed over. **[AI.md](AI.md)** is that
account, and it ends in a rule that contributors are held to.

## License

BSD 2-Clause. See [LICENSE](LICENSE).
