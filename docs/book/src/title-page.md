# The Zuri Programming Language

*by the Zuri Project*

Zuri is a language for building whole applications, and it ships with
everything that takes. The runtime, the web server, the templating engine,
the data formats, the cryptography and the toolchain were designed
together and are delivered as one binary, so starting a real program does
not begin with assembling an ecosystem out of third-party parts. Zuri has
first-class language-level support for package management and vendoring, 
so the code you bring in from outside needs no third-party tooling either.

The standard library ships with the language, and it is large. Templating,
HTTP/1.1 and HTTP/2, WebSockets, TLS, JSON, YAML, CSV, compression,
cryptography, image decoding and drawing, HTML parsing, date arithmetic
with the IANA time zone database, and OS-thread concurrency are all part
of the installation. Every one of them is reachable with a bare `import`.

Zuri is dynamically typed, and it declines to be vague. A declared
parameter type is enforced at the call, classes are sealed, a function
cannot be quietly redefined, and a name beginning with an underscore stays
inside the module that declared it. Your code runs as bytecode until a
piece of it gets hot, and a JIT compiles that piece to machine code while
the program is still running.

This book teaches the language from the first line of code to a complete
web application. It is written against the Rust implementation of Zuri.
Every program printed in these pages was run against that implementation,
and the output shown underneath each one is the output it produced.
