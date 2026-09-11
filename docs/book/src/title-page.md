# The Zuri Programming Language

*by the Zuri project*

This book targets the Rust implementation of Zuri, the `zuri-rs` project
this repository holds. Every program printed in these pages was run against
that implementation while the chapter was being written, and the output
shown is the output it produced.

Zuri is a dynamically typed language with a strong opinion about
readability. It has classes, closures, a module system that keeps
namespaces honest, and a standard library that covers templating, HTTP/2,
TLS, image manipulation, compression and real OS-thread concurrency without
reaching for a single third-party package.

It also has a JIT compiler, so the loops you write get compiled to machine
code once they run hot enough to be worth it. You will not think about that
for most of this book. Chapter 18 is where we lift the hood.
