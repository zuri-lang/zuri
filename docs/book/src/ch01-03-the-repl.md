# The REPL

If you run `zuri` without any arguments, you get an interactive prompt:

```console
$ zuri
Zuri 0.1.0 (running on ZuriVM 0.1.0), REPL/Interactive mode = ON
Build No. => 2026-09-10 23:19:10 UTC
Type ".exit" to quit, ".help" for help or ".credits" for more information
%>
```

REPL stands for read-eval-print loop, and the *print* part is what makes it
so handy. At the top level of the REPL, the value of any expression that
isn't `nil` is printed for you, so you don't need `echo` just to look at a
value:

```zuri
%> 1 + 2
3
%> var name = 'Zuri'
%> name.upper()
ZURI
%> [1, 2, 3].map(@(n) { return n * n })
[1, 4, 9]
```

Notice that `var name = 'Zuri'` didn't print anything. A declaration isn't
an expression, and nothing is printed for values that are `nil`. That's why
`list.append(4)` and `echo x` don't print their output twice.

Values are only printed automatically at the top level. If you type a loop,
its body doesn't print anything on each iteration:

```zuri
%> iter var i = 0; i < 3; i++ {
..   i * 10
.. }
%>
```

Use `echo` inside a block when you want to see a value.

## Multi-line Input

When what you've typed isn't a complete statement yet, the prompt changes
from `%>` to `..`. If a line ends with an unclosed brace, press Enter and
keep typing:

```zuri
%> def double(n) {
..   return n * 2
.. }
%> double(21)
42
```

The same goes for an unclosed bracket or parenthesis, an unfinished string,
or a block comment. The REPL reads your input the same way the compiler
does, so a bracket inside a string or a quote inside a comment doesn't
count. The REPL works out when your input is complete, so you never have to
tell it.

## Definitions Persist

Variables, functions and classes that you declare at the top level stay
defined for the rest of the session, because the whole session shares one
namespace:

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

If you declare a name again at the top level, the new declaration replaces
the old one instead of raising an error. That way, you can keep retyping a
function until you get it right.

## Leaving the REPL

**Type `.exit`.** In a terminal, this is the only way to end the session.

**Ctrl+C and Ctrl+D don't exit.** Both print an interrupt message and give
you a new prompt:

```console
%> <KeyboardInterrupt [CtrlC]>
Type '.exit' to exit the REPL session
%>
```

Use Ctrl+C to throw away a line you've only partly typed. The line is
discarded and you get a fresh prompt.

As far as the rest of the language is concerned, the whole session is a
single program. Handlers registered with `os.at_exit()` run when the session
ends, and `os.set_exit_code()` sets its exit status.

## Piped Input

When standard input isn't a terminal, `zuri` reads the session from standard
input instead. It doesn't show a banner or a prompt, only the session's
output, and it stops when the input ends:

```console
$ printf '1 + 2\nvar n = 4\nn * 2\n' | zuri
3
8
```

Otherwise, it behaves as though you'd typed the input yourself. Expressions
are printed, declarations stay defined, statements can span several lines,
and `.exit` ends the session early. When a line fails, its error is reported
and the session continues, so the exit status is still `0`. If you want a
program to stop at its first error, use `zuri run` instead.

## Syntax Highlighting

In a terminal, the REPL highlights your input as you type. Keywords are
colored and everything else is left plain, so a misspelling such as `retrun`
or `whiel` is easy to spot before you press Enter, because it doesn't change
color.

The highlighter knows about string literals. A keyword inside quotes is just
text, so it isn't colored:

```zuri
%> var note = 'return it later'
```

Here, `return` stays plain because it's part of the string.

The REPL also shows a grayed-out suggestion to the right of the cursor when
what you've typed matches an earlier line in your history. Press the Right
Arrow key to accept it.

Highlighting and suggestions only appear in a terminal. If you pipe input
into `zuri` or redirect its output, you get plain text, so a session saved
to a file contains no escape codes.

## Other Dot Commands

**`.help`** reminds you that you can press Tab for completions.

**`.credits`** opens the project's license in a pager. Press `q` to return
to the prompt. In a piped session, the license is printed instead.

Tab completes both commands, as well as every keyword in the language.

## History

Use the Up and Down arrow keys to move through previous lines, and Ctrl+R to
search backward through them. History is saved to a `history.txt` file next
to the `zuri` executable, so it's kept between sessions.

## What the REPL Is Good For

Each line is compiled and run separately, which makes the REPL a good fit
for quick questions like these:

- What does this method return? `'a,b,,c'.split(',')`
- Is this value truthy? `!!(-1)`
- What type is this value? `typeof(x)`
- Does this syntax parse the way I expect? Type it in and see.

It's not a good place to *write* a program, since you can't go back and edit
line four. Once you have more than a few lines, put them in a `.zu` file and
run that instead. That's what the next chapter does.
