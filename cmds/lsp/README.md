# lsp

The Zuri language server. It communicates using the Language Server
Protocol over standard input and output, so any editor that supports the
protocol gets completion, navigation, diagnostics as you type,
refactoring, formatting and test running for Zuri. The Editor Support
chapter of the book describes each feature, and explains how to set up
each editor.

## Running it

Editors start the server themselves:

```sh
zuri lsp [--stdio] [--log <path>] [--log-level <level>]
```

The server always communicates over standard input and output. `--stdio`
is accepted because some editors pass it. `--log` appends the log to a
file, in addition to sending it to the editor. `--log-level` is `error`,
`warn`, `info` (the default), `debug` or `trace`. At `trace`, every
message exchanged with the editor is also written to the log file.

The process exits with status 0 after an orderly `shutdown` and `exit`.
It exits with status 1 if the connection ends any other way, or if the
editor that started it exits.

## How it works

A single isolate owns all of the server's state and answers every
message. Other isolates read standard input, index the files that aren't
open in the editor, lay out files for formatting, and run tests. Each one
reports back over a channel, and the owning isolate waits on all of them
and on its own timers at the same time. Before it handles anything, it
takes every message that has arrived. That way, a request that was
canceled while it waited is answered as canceled without being run,
and a request about a document that has changed since is answered as
modified.

A file is analyzed in three steps:

1. `zuri.parse_partial()` reads it into a syntax tree, recovering from
   syntax errors.
2. The walk in `analysis/scope.zu` resolves every name the same way the
   compiler does, and stores the result as a summary made of plain lists
   and dictionaries. Because the summary is plain data, an indexing
   isolate can build one and send it over a channel.
3. The linker in `analysis/linker.zu` answers the questions that no
   single file can: what a module exports, what an imported name refers
   to, what a deferred type turns out to be, and which members a class has
   once its superclasses and extensions are included.

Imports are resolved in the same order the runtime uses (`resolve.zu`).
The built-in globals and types come from the stub files that ship with the
standard library (`analysis/builtins.zu`). The runtime's own tests check
those stubs against what the runtime defines.

## Source files

- `index.zu`: the command, its options and the log setup.
- `server.zu`: the message loop, the lifecycle, documents, diagnostics
  timing, indexing, settings, formatting jobs and test runs.
- `state.zu`: open documents and the settings.
- `text.zu`: line indexes, the three position encodings, incremental
  edits, URIs, and the line diff used to answer formatting requests.
- `workspace.zu`: the files of a workspace folder.
- `resolve.zu`: import resolution.
- `logging.zu`: the server's log, built on the `log` module.
- `indexer.zu`, `formatter.zu`: the isolates that index and format files.
- `analysis/`: the summary of a file (`summary.zu`, `scope.zu` and
  `infer.zu`), the linker, the built-ins, questions about a position in a
  file (`query.zu`), and doc blocks rendered as Markdown (`render.zu`).
- `features/`: one file for each part of the protocol that the server
  supports.

## Tests

The tests in `tests/` drive a real `zuri lsp` process through the `rpc`
module, the way an editor does, against the small project in
`tests/_fixture/`:

```sh
zuri test cmds/lsp/tests
```
