# Editor Support

Zuri includes a language server. `zuri lsp` implements the Language Server
Protocol, so any editor that supports the protocol gets completion,
navigation, diagnostics as you type, refactoring, formatting and test
running for Zuri, from the same server that comes with the runtime. The
server is written in Zuri and uses the `zuri` module's own parser and
compiler, so what it reports matches what the runtime would do with the same
code.

There's an extension for Visual Studio Code that sets everything up for you.
Neovim, Helix, Sublime Text and Emacs each need a few lines of
configuration, which are shown at the end of this chapter.

- [Features](#features)
  - [Writing Code](#writing-code)
  - [Navigating Code](#navigating-code)
  - [Diagnostics](#diagnostics)
  - [Changing Code](#changing-code)
  - [Running Tests](#running-tests)
  - [Highlighting](#highlighting)
- [How the Server Reads a Project](#how-the-server-reads-a-project)
- [Settings](#settings)
- [Starting the Server](#starting-the-server)
- [Visual Studio Code](#visual-studio-code)
- [Neovim](#neovim)
- [Helix](#helix)
- [Sublime Text](#sublime-text)
- [Emacs](#emacs)
- [Troubleshooting](#troubleshooting)

## Features

### Writing Code

**Completion** suggests what you can write at the cursor:

- After a `.`, the members of the receiver's type. That includes an
  instance's fields and methods, including inherited ones and ones added by
  extensions, a class's static members, a module's exports, a built-in
  type's methods, and the keys of a dictionary written out where its
  variable was declared.
- Anywhere else, the names in scope, the built-in globals, and the keywords
  that can start what you're writing.
- After `import`, module paths, and between an import's braces, what the
  module exports.
- At the start of a class member, `@new` and the operator decorators the
  class doesn't have yet, as method skeletons.
- After the `:` of a type hint, the type names.
- Inside a doc comment, its tags after `@`, and the names of the parameters
  it documents after `@param`.

A name exported by another module is offered along with the import it needs,
either as `import json` and `json.encode` for a standard library module or a
package, or as `import .shapes { Point }` for a module in the project.

**Signature help** follows a call while you write its arguments. It
highlights the parameter at the cursor and shows its documentation, follows
a constructor call to its `@new`, and stays on a variadic parameter for
every argument it receives.

**Hover** shows a declaration as a line of Zuri, its documentation formatted
in the same way as the standard library reference, the type inferred for a
variable, the value of a constant, and an overview of a module.

**Inlay hints** show the name of the parameter each literal argument is
passed to, and, if you turn them on, the type inferred for each variable.

### Navigating Code

Go to definition follows an imported name back to the module that declares
it, and a built-in name to the stub that documents it. Go to declaration
stops at the import that brings a name into the file, and go to type
definition goes to the class of the value a name holds. Go to implementation
lists the methods in subclasses that override a method, or the subclasses of
a class.

Find references and the highlighting of the current name use what each name
resolves to, never its spelling, so a `distance` method in one class is
never confused with another class's `distance` method.

The call hierarchy shows what calls a function and what it calls, and the
type hierarchy shows the classes above and below a class. Each file has an
outline, and the workspace symbol search finds declarations whose names
contain the letters you type in order, so `hrq` finds `HttpRequest`. Each
import links to the file it loads, and each class, function and method shows
how many times it's used.

### Diagnostics

Every error the server reports is either an error the compiler reports, or a
failure that the runtime is certain to raise when the code runs:

- A syntax error. The rest of the file is still checked after recovering
  from it.
- An error from the compiler, such as `break` outside a loop.
- An import of a module that doesn't exist, or of a name its module doesn't
  have.
- `module.name` for a name the module doesn't have.
- A name that nothing declares.
- `self.name = value` outside `@new`, for a field that no class in the
  inheritance chain declares.
- A literal argument of the wrong type, or a missing argument, for a typed
  parameter, with the runtime's own message.

A member that an instance of a fully known class doesn't have is reported as
a warning, because an extension the server hasn't read could add it. Unused
imports and declarations, and code after a `return`, `raise`, `break` or
`continue`, are shown faded. Uses of anything documented as `@deprecated`
are shown with a strikethrough.

The server never guesses. A value whose type can't be determined, a module
whose wildcard imports lead to modules it hasn't read, and a class whose
superclass can't be found never cause a diagnostic. Argument counts aren't
checked either, because Zuri doesn't enforce them.

Quick fixes can add the import a missing name needs, correct a misspelled
name or member, remove an unused import, and declare a field that's assigned
outside `@new`.

### Changing Code

**Rename** changes a declaration and every use of it throughout the
workspace. A `{ name }` dictionary entry keeps its key and becomes
`{ name: renamed }`. A rename is refused, with an explanation, when the new
name is a keyword, when it's already declared somewhere the declaration or
one of its uses would see it, when it would make something private that's
used from outside, and for anything declared by the standard library or a
package. A member used through a receiver whose type can't be determined is
left unchanged, and the editor is told where.

**Extract and inline** refactorings are only offered where they don't change
what the program does:

- *Extract variable* moves an expression into a `var` just before its
  statement. It never moves an expression out of a loop's condition, the
  right side of `and` or `or`, a branch of `?:`, a `when` label or an
  `else if` condition, and never past something with a side effect that runs
  first. When the expression has no side effects, every identical copy of it
  in the block can share the variable.
- *Extract function* turns complete statements from one block into a
  function. The outer local variables they use become its parameters, and
  any values they set that are read afterwards are returned, either directly
  for a single value or in a dictionary for several. Statements that use
  `self` or `parent` become a private method.
- *Inline variable* replaces each use of a `var` that's assigned once and
  never changed with its value.
- *Inline function* replaces a call to a function whose body is a single
  `return` with that expression, and removes the function once every call
  has been inlined.

**Formatting** runs `zuri fmt` on a file or a selection, and only sends back
the lines that changed. A file with a syntax error is left as it is.

Renaming or moving a file updates the relative imports that refer to it,
both in the files that import it and in the moved file itself.

### Running Tests

The server finds the tests that every file in the workspace declares with
the `test` module, meaning each `describe` and `it`, along with their
`_only`, `_skip`, `_todo` and `_each` forms, whose names are written as
strings. A code lens above each one runs it, and one above the first test
runs the whole file.

A test runs in the same way `zuri test` would run it: with `zuri run` on its
file, in a separate process, from the root of the project. Each result is
sent to the editor as soon as it's available, and each failed assertion is
shown on the line where it failed, until the file changes or its tests run
again.

### Highlighting

Semantic highlighting colors each name according to what it resolves to,
whether that's a class, a function, a method, a parameter, a variable, a
field, a module or a decorator, and marks anything defined by the standard
library. It works with any editor theme that supports semantic tokens.

## How the Server Reads a Project

The server reads every `.zu` file in the workspace folders the editor opens.
It does this in the background, on its own isolates, so it can respond while
it's reading. Inside a git working tree, it reads the files git tracks or
would track, so `.gitignore` is respected. Anything under `.git`, `.zuri` or
`node_modules` is skipped, along with anything `zuri.exclude` lists.

Imports are resolved in exactly the same way the runtime resolves them: a
relative import against the importing file, then the project's `.zuri/libs`,
then the standard library, then the native modules, and finally the packages
installed for the user in `ZURI_HOME/libs`. The standard library is read
from the Zuri installation the server belongs to, or from `zuri.root`, or
from `ZURI_ROOT`. A module imported by the workspace is read the first time
it's needed.

## Settings

The server reads its settings from the `zuri` section of the editor's
configuration, either as nested objects or as dotted names, and picks up
changes as soon as they're made.

| Setting | Default | What it does |
| --- | --- | --- |
| `root` | `''` | The installation whose standard library is read. If empty, the server uses `ZURI_ROOT`, or the installation that `zuri` belongs to. |
| `exclude` | `[]` | Glob patterns, relative to a workspace folder, for files that are never read |
| `index.workers` | `2` | The number of isolates that read the workspace in the background |
| `diagnostics.enable` | `true` | Whether problems are reported at all |
| `diagnostics.scope` | `'openFiles'` | `'openFiles'`, or `'workspace'` to check every file in the workspace |
| `diagnostics.delay` | `300` | How many milliseconds to wait after a change before checking a file |
| `diagnostics.unusedHints` | `true` | Whether unused declarations are shown faded |
| `inlayHints.parameterNames` | `'literals'` | Which arguments are labeled with parameter names: `'none'`, `'literals'` or `'all'` |
| `inlayHints.variableTypes` | `false` | Whether inferred variable types are shown |
| `codeLens.references` | `true` | Whether declarations show how many times they're used |
| `completion.autoImport` | `true` | Whether names from modules that haven't been imported yet are suggested |
| `completion.callParentheses` | `false` | Whether completing a function also adds its parentheses |
| `testing.codeLens` | `true` | Whether suites and tests show code lenses that run them |

## Starting the Server

Editors start the server themselves, and communicate with it over its
standard input and output:

```sh
zuri lsp [--stdio] [--log <path>] [--log-level <level>]
```

`--stdio` names the only way the server communicates, and it's accepted
because editors pass it. `--log` writes the server's log to a file, as well
as sending it to the editor. `--log-level` sets how much is logged: `error`,
`warn`, `info` (the default), `debug` or `trace`, which also writes every
message exchanged between the editor and the server to the log file.

The server exits with status `0` when the editor shuts it down properly, and
with `1` if the connection ends without that, or if the editor that started
it has exited.

## Visual Studio Code

Install the **Zuri** extension from the marketplace, or from a downloaded
package:

```sh
code --install-extension zuri-vscode-0.2.0.vsix
```

The extension starts the server for every workspace that contains a `.zu`
file. Its settings are the server's settings, under `zuri.`, plus two of its
own: `zuri.path`, the `zuri` executable to run, and `zuri.trace.server`,
which records the messages between the editor and the server in the **Zuri
Language Server** output panel. Tests appear in the Test Explorer, and the
commands **Zuri: Restart Language Server**, **Zuri: Show Language Server
Output** and **Zuri: Check Zuri Installation** are available in the command
palette.

## Neovim

Neovim 0.11 has built-in support for configuring language servers:

```lua
vim.filetype.add({ extension = { zu = 'zuri' } })

vim.lsp.config('zuri', {
  cmd = { 'zuri', 'lsp' },
  filetypes = { 'zuri' },
  root_markers = { 'project.toml', '.git' },
  settings = {
    zuri = {
      inlayHints = { variableTypes = true },
    },
  },
})

vim.lsp.enable('zuri')
```

To show inlay hints, use `vim.lsp.inlay_hint.enable()`, and to run a code
lens, use `vim.lsp.codelens.run()`.

## Helix

Add the language and its server to `languages.toml`:

```toml
[[language]]
name = "zuri"
scope = "source.zuri"
file-types = ["zu"]
roots = ["project.toml"]
comment-token = "#"
block-comment-tokens = { start = "/*", end = "*/" }
indent = { tab-width = 2, unit = "  " }
language-servers = ["zuri-lsp"]

[language-server.zuri-lsp]
command = "zuri"
args = ["lsp"]

[language-server.zuri-lsp.config.zuri]
inlayHints = { parameterNames = "literals" }
```

## Sublime Text

With the LSP package installed, add a client to its settings:

```json
{
  "clients": {
    "zuri": {
      "enabled": true,
      "command": ["zuri", "lsp"],
      "selector": "source.zuri",
      "settings": {
        "zuri.diagnostics.scope": "openFiles"
      }
    }
  }
}
```

The selector names the syntax that Zuri files are opened with. The Visual
Studio Code extension's grammar, `zuri.tmLanguage.json`, is a TextMate
grammar with the scope `source.zuri`, and the Convert command in PackageDev
turns it into a `.tmLanguage` file for Sublime Text.

## Emacs

Eglot, which is built into Emacs 29, needs a mode for Zuri files and the
command that starts the server:

```elisp
(define-derived-mode zuri-mode prog-mode "Zuri"
  "A major mode for Zuri source."
  (setq-local comment-start "# "))

(add-to-list 'auto-mode-alist '("\\.zu\\'" . zuri-mode))

(with-eval-after-load 'eglot
  (add-to-list 'eglot-server-programs '(zuri-mode "zuri" "lsp")))

(add-hook 'zuri-mode-hook #'eglot-ensure)
```

Settings go in `eglot-workspace-configuration`:

```elisp
(setq-default eglot-workspace-configuration
              '(:zuri (:inlayHints (:variableTypes t))))
```

## Troubleshooting

The server sends whatever it logs at its `--log-level` and above, which is
`info` by default, to the editor's log for language servers. While the
editor requests a trace, it also sends everything down to `debug`. Starting
it with `--log` and `--log-level debug` writes all of this to a file, and
`--log-level trace` also includes every message exchanged.

If nothing works at all, the editor probably can't start `zuri`. Run
`zuri --version` in a terminal to check whether it's on your `PATH`. If your
editor lets you set a path, as `zuri.path` does in Visual Studio Code, you
can give it the location of the executable directly.

If the standard library can't be found, the server reports it in its log. To
fix it, set `zuri.root`, or `ZURI_ROOT` for the editor, to the Zuri
installation.
