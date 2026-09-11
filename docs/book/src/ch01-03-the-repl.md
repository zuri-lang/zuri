# The REPL

Run `zuri` with no arguments and you get an interactive prompt:

```console
$ zuri
Zuri 0.1.0 (running on ZuriVM 0.1.0), REPL/Interactive mode = ON
Build No. => 2026-09-10 23:19:10 UTC
Type ".exit" to quit, ".help" for help or ".credits" for more information
%>
```

REPL stands for read-eval-print loop, and the "print" part is the reason
you want it. At the top level of the REPL, any expression whose value is
not `nil` gets printed automatically, so you do not need `echo` to look at
something:

```zuri
%> 1 + 2
3
%> var name = 'Zuri'
%> name.upper()
ZURI
%> [1, 2, 3].map(@(n) { return n * n })
[1, 4, 9]
```

Notice that `var name = 'Zuri'` printed nothing. Declarations are not
expressions, and anything that evaluates to `nil` stays quiet. That rule
is what keeps `list.append(4)` from spamming your screen.

The auto-print applies only to the outermost level. Type a loop and the
body does not print once per iteration:

```zuri
%> iter var i = 0; i < 3; i++ {
..   i * 10
.. }
%>
```

Use `echo` inside a block when you want to see something.

## Multi-line Input

The prompt changes from `%>` to `..` when a statement is incomplete. Press
Enter at the end of a line with an unclosed brace and keep typing:

```zuri
%> def double(n) {
..   return n * 2
.. }
%> double(21)
42
```

## Things Worth Knowing

**Tab completes keywords.** Press Tab and you get a menu of the language's
reserved words.

**History persists.** Up and down arrows walk previous lines, and the
history is saved to a `history.txt` file next to the `zuri` executable, so
it survives between sessions. Ctrl+R searches it backwards.

**Definitions stick around.** Variables, functions and classes you declare
at the REPL's top level stay defined for the rest of the session. The REPL
shares one namespace across every line you type.

**Ctrl+C and Ctrl+D both exit**, as does typing `.exit`.

**`.credits`** opens the project's licence in a pager. `.help` reminds you
about Tab completion.

## What the REPL Is Not For

The REPL compiles and runs each line independently. It is perfect for
checking what a method returns, what a piece of syntax parses to, or
whether your mental model of truthiness is right. It is a poor place to
develop a program. Once you are writing more than a few lines, put them in
a `.zu` file and run it, which is exactly what Chapter 2 is about.
