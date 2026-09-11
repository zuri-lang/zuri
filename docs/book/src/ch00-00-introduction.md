# Introduction

Welcome. Grab a coffee, open a terminal, and let's talk about Zuri.

Every language is an argument about what matters. C argues that the machine
matters. Python argues that the reader matters. Go argues that the team
matters. Zuri's argument is narrower and easier to defend: **the code you
write in a hurry should still be the code you want to read a year later.**

You can see that argument in what the language refuses to let you do. You
cannot write `if x > 5 do_something()` on one line, because `if` requires a
block. You cannot add a field to a class at runtime, because a class is
frozen the moment it is created. You cannot import a module and have its
contents leak out through your own module's namespace, because imports are
local unless you mark them for export. Those are not three unrelated rules.
They are one decision applied three times: make the surprising thing
impossible, and the obvious thing short.

## Who This Book Is For

Chapters 1 through 6 assume nothing except that you can open a terminal. If
you have never written a line of code, start at the beginning and take your
time. Every concept is introduced before it is used.

From Chapter 7 onward the pace picks up. By then you know the language, and
the question changes from "what does this syntax mean?" to "how do I build
the thing I came here to build?" If you already program in Python,
JavaScript, Ruby or Go, you can skim Chapters 3 to 5 in an afternoon.
[Appendix H](appendix-08-coming-from.md) collects the places where Zuri
will trip you up precisely *because* it looks familiar.

## How to Read This Book

Read it with a terminal open. Zuri has a REPL, every example in this book
is short enough to paste into it, and nothing cements a language faster
than breaking one of its examples on purpose to see what the error says.

The book has five parts.

1. **Getting started** (Chapters 1 and 2). Install it, run it, then build a
   small but genuinely useful command-line application, so you have seen
   the shape of a real Zuri program before we start dissecting it.
2. **The language** (Chapters 3 to 8). Variables, types, control flow,
   functions, classes, errors and modules. This is the reference-quality
   core.
3. **The world outside** (Chapters 9 to 13). Files, binary data,
   concurrency, networking, and a guided tour of the standard library so
   you stop reinventing it.
4. **The big modules** (Chapters 14 to 16). Wire, HTTP and Imagine each get
   a chapter, because each is large enough to be a library in its own
   right.
5. **Depth and a capstone** (Chapters 17 to 20). Reflection, the JIT,
   debugging, and one full-stack web application that uses nearly
   everything the book has taught.

The appendices are the part you will keep coming back to: keywords,
operator precedence, the decorated-method table, and an index of every
standard library module.

## Conventions

Code you type into a shell looks like this, with the output underneath:

```console
$ zuri main.zu
Hello, world!
```

Code in a file looks like this, with the filename above it when it matters:

<span class="filename">Filename: main.zu</span>

```zuri
echo 'Hello, world!'
```

Anything the book claims about the language is a claim about how the
implementation behaves, and every one was checked by running it. Where a
rule has an exception, the exception is stated in the same breath as the
rule.

Let's get started.
