# The Zuri Programming Language

*by the Zuri Project*

Zuri is a general-purpose programming language for writing programs that
other people read. It is dynamically typed, object-oriented, and built
around a small set of statements that behave the same way everywhere they
appear.

A Zuri program is a file. Run it and the file's top level is the program;
import it and the same file is a module. There is no build step, no
project manifest, and no dependency to resolve before the first line runs.

The standard library ships with the language, and it is large. Templating,
HTTP/1.1 and HTTP/2, TLS, JSON, YAML, CSV, compression, cryptography,
image decoding and drawing, HTML parsing, date arithmetic with the IANA
time zone database, and OS-thread concurrency are all part of the
installation. Every one of them is reachable with a bare `import`.

This book teaches the language from the first line of code to a complete
web application. It is written against the Rust implementation of Zuri.
Every program printed in these pages was run against that implementation,
and the output shown underneath each one is the output it produced.
