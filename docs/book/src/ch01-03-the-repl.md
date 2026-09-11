# The REPL

Run `zuri` with no arguments and you get an interactive prompt:

```console
$ zuri
Zuri 0.1.0 (running on ZuriVM 0.1.0), REPL/Interactive mode = ON
Build No. => 2026-09-10 23:19:10 UTC
Type ".exit" to quit, ".help" for help or ".credits" for more information
%>
```

REPL stands for read-eval-print loop, and the *print* is the part that
makes it useful. At the top level of the REPL, any expression whose value
is not `nil` is printed for you, so you never need `echo` just to look at
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

Notice that `var name = 'Zuri'` printed nothing. A declaration is not an
expression, and anything that evaluates to `nil` stays quiet. That single
rule is what keeps `list.append(4)` and `echo x` from doubling up on your
screen.

Auto-printing applies only to the outermost level. Type a loop and the body
does not print once per iteration:

```zuri
%> iter var i = 0; i < 3; i++ {
..   i * 10
.. }
%>
```

Use `echo` inside a block when you want to see something.

## Multi-line Input

The prompt changes from `%>` to `..` when what you have typed so far is
not yet a complete statement. Press Enter at the end of a line with an
unclosed brace and keep going:

```zuri
%> def double(n) {
..   return n * 2
.. }
%> double(21)
42
```

The same applies to an unclosed bracket, an unclosed parenthesis, or a
string that has not been closed. The REPL decides when you are finished;
you never have to signal it.

## Everything Stays Defined

Variables, functions and classes you declare at the top level of the REPL
remain defined for the rest of the session, because the whole session
shares one namespace:

```zuri
%> class Point {
..   @new(x, y) {
..     self.x = x
..     self.y = y
..   }
.. }
%> var p = Point(3, 4)
%> (p.x ** 2 + p.y ** 2).sqrt()
5
```

Redeclaring a name at the REPL's top level replaces it rather than raising
an error, so you can retype a function until it is right.

## Getting Out

**Type `.exit`.** That is the only thing that ends the session.

**Ctrl+C and Ctrl+D do not exit.** Both print an interrupt line and hand
the prompt back:

```console
%> <KeyboardInterrupt [CtrlC]>
Type '.exit' to exit the REPL session
%>
```

Ctrl+C is how you abandon a half-typed line: the line is discarded and you
start again on a fresh prompt.

## Syntax Highlighting

On a terminal, the REPL colours what you type as you type it. Keywords are
highlighted and everything else is left plain, so a misspelled `retrun` or
`whiel` stands out before you press Enter — it simply does not change
colour.

The highlighting understands string literals. A keyword inside quotes is
text, not a keyword, and is left uncoloured:

```zuri
%> var note = 'return it later'
```

`return` there stays plain, because it is part of the string.

Alongside it, a greyed-out suggestion appears to the right of the cursor
when what you have typed so far matches something earlier in your history.
Press the right arrow to accept it.

Both are terminal features. Piping input to `zuri` or redirecting its
output produces plain text, so a session captured to a file has no escape
codes in it.

## The Other Dot Commands

**`.help`** reminds you that Tab offers completions.

**`.credits`** opens the project's licence in a pager. Press `q` to come
back.

Both are completed by Tab, along with every keyword in the language.

## History

Up and Down walk through previous lines, and Ctrl+R searches backwards
through them. History is written to a `history.txt` file sitting next to
the `zuri` executable, so it survives between sessions.

## What the REPL Is Good At

Each line is compiled and run on its own, which makes the REPL the right
tool for a specific kind of question:

- What does this method return? `'a,b,,c'.split(',')`
- Is this value truthy? `!!(-1)`
- What type am I actually holding? `typeof(x)`
- Does this syntax parse the way I think? Type it and find out.

It is a poor place to *write* a program, because you cannot go back and
edit line four. Once you are past a few lines, put them in a `.zu` file and
run it — which is what the next chapter does.
