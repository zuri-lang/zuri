# The Zuri Programming Language

*by the Zuri Project*

Zuri is a programming language for building complete applications, and it
comes with the tools and libraries those applications need. The runtime, the
web server, the template engine, the data formats, the cryptography and the
toolchain were designed together and ship as a single binary. You don't have
to assemble a stack of third-party packages before you can start on a real
program. When you do want outside code, package management and vendoring are
built into the language, so you don't need extra tools for that either.

The standard library is large. Templates, HTTP/1.1 and HTTP/2, WebSockets,
TLS, JSON, YAML, CSV, compression, cryptography, image decoding and drawing,
HTML parsing, date arithmetic with the IANA time zone database, and
concurrency on OS threads are all included, and each is available with a
plain `import`.

The tools are part of the same binary. `zuri fmt` formats your code,
`zuri test` runs your tests, and `zuri lsp` provides completion, navigation
and diagnostics in your editor. `zuri install` and `zuri publish` manage
packages on Nyssa, a package registry that any Zuri installation can host,
and `zuri bundle` packages a finished program so it can run on machines that
do not have Zuri installed, even as a single executable.

Zuri is dynamically typed, but it checks more than most dynamic languages
do. Parameter types you declare are checked when the function is called,
classes are sealed, a function can't be silently redefined, and names that
start with an underscore are private to the module that declares them. Your
code runs as bytecode at first. When part of it runs often enough, a JIT
compiler turns that part into machine code while the program keeps running.

This book teaches the language, starting with your first line of code and
ending with a complete web application. The output shown below each 
program is the output it actually produced.
