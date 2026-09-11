# Hello, World!

Make a directory, put one file in it, and run it.

```console
$ mkdir hello
$ cd hello
```

Create a file called `main.zu`. The `.zu` extension is what the runtime
looks for when resolving imports, so get in the habit early.

<span class="filename">Filename: main.zu</span>

```zuri
echo 'Hello, world!'
```

Run it:

```console
$ zuri main.zu
Hello, world!
```

That is the whole program. No `main` function, no imports, no boilerplate.
A Zuri file is a script, and its top level is code that runs.

## Anatomy of One Line

```zuri
echo 'Hello, world!'
```

`echo` is a **keyword**, not a function. You do not write `echo(...)`, you
write `echo` followed by an expression. It prints the value and adds a
newline.

Strings are written between single or double quotes, and the two are
identical in meaning. Most Zuri code uses single quotes and saves double
quotes for strings that contain an apostrophe.

There is no semicolon. Zuri ends a statement at the end of the line. You
*can* write a semicolon if you want two statements on one line, but almost
nobody does.

## `echo` and `print`

There is also a `print()` function, and the difference is worth learning
now because it bites people later.

```zuri
echo 'first'
echo 'second'

print('third')
print('fourth\n')
```

```console
first
second
thirdfourth
```

`echo` appends a newline; `print()` does not, and it takes any number of
arguments. Use `echo` for output meant for a human reading a terminal, and
`print()` when you are assembling output character by character.

## Running a Directory

If you point `zuri` at a directory instead of a file, it looks for
`index.zu` inside it and runs that:

```console
$ zuri hello
Hello, world!
```

If there is no `index.zu`, you get told so plainly:

```console
$ zuri hello
(Zuri):
  Launch aborted for hello
  Reason: No entrypoint found in the directory
```

This is the same rule the module system uses for packages, which we will
get to in [Chapter 8](ch08-00-modules.md). A directory with an `index.zu`
is a unit you can run *or* import.
