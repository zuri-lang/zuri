# File Methods

Every method of the built-in `file` type, with its signature, what it
returns, and any behavior you might not expect.

| Method | Returns | Summary |
| --- | --- | --- |
| [`exists()`](#exists) | `boolean` | Returns `true` if a file exists or `false` otherwise. |
| [`close()`](#close) | `void` | Closes the stream to an opened file. |
| [`open()`](#open) | `void` | Opens the stream to a file for the operation originally specified on the file object during creation. |
| [`read(length: ?int)`](#read) | `string\|bytes` | Reads from the open file, up to `length` bytes, and returns a string, or bytes if the file was opened in a binary mode. |
| [`gets(length: ?int)`](#gets) | `string\|bytes` | Same as `read()`, but doesn't open or close the file automatically. |
| [`write(data: string\|bytes)`](#write) | `string\|bytes` | Writes a string or bytes to an opened file at the current insertion point. |
| [`puts(data: string\|bytes)`](#puts) | `string\|bytes` | Same as `write()`, but doesn't open or close the file automatically. |
| [`number()`](#number) | `int` | Returns the file descriptor that the operating system uses for this file. |
| [`is_tty()`](#is_tty) | `boolean` | Returns `true` if the file is connected to a TTY like device or `false` otherwise. |
| [`is_open()`](#is_open) | `boolean` | Returns `true` if the file is open for reading or writing and `false` otherwise. |
| [`is_closed()`](#is_closed) | `boolean` | Returns `true` if the file is closed for reading or writing and `false` otherwise. |
| [`flush()`](#flush) | `void` | Flushes the buffer held by a file. |
| [`stats()`](#stats) | `dict` | Returns the statistics or details of a file. |
| [`symlink()`](#symlink) | `boolean` | Creates a symbolic link to this file at the given path. |
| [`delete()`](#delete) | `boolean` | Deletes a file. |
| [`rename(new_name: string)`](#rename) | `boolean` | Renames the file to `new_name`. |
| [`path()`](#path) | `string` | Returns the path to the file. |
| [`abs_path()`](#abs_path) | `string` | Returns the absolute path to the file. |
| [`copy(path: string)`](#copy) | `boolean` | Copies the file to the given path. |
| [`truncate(length: ?number)`](#truncate) | `boolean` | Truncates the entire file if length is not given or truncates the file such that only length number of bytes is left in it. |
| [`chmod(mode: int)`](#chmod) | `boolean` | Changes the permission on the file to the one specified in the number given. |
| [`set_times(atime: number, mtime: number)`](#set_times) | `boolean` | Sets the last access time and last modified time of the file. |
| [`seek(offset: number, seek_type: int)`](#seek) | `boolean` | Sets the position of a file reader or writer in a file. |
| [`tell()`](#tell) | `number` | Returns the current position of the reader/writer in a file. |
| [`mode()`](#mode) | `string` | Returns the mode in which the current file was opened. |
| [`name()`](#name) | `string` | Returns the name of the current file. |
| [`to_string()`](#to_string) | `string` | Returns the file handle as a string, naming its path and the mode it was opened in. |

## `exists()`

```zuri,ignore
exists() -> boolean
```

Returns `true` if a file exists or `false` otherwise.

For example:

```zuri-repl
%> file('sample.txt').exists()
true
```

**Returns** `boolean`

## `close()`

```zuri,ignore
close() -> void
```

Closes the stream to an opened file. You'll rarely ever need to call
this method yourself in most use cases.

For example:

```zuri-repl
%> var f = file('sample.txt')
%> f.close()
```

**Returns** `void`

## `open()`

```zuri,ignore
open() -> void
```

Opens the stream to a file for the operation originally specified on the
file object during creation. You may need to call this method after a
call to read() if the length isn't specified or write() if you wish to
read or write again as the file will already be closed.

For example:

```zuri-repl
%> f.open()
```

**Returns** `void`

## `read()`

```zuri,ignore
read(length: ?int) -> string|bytes
```

Reads from the open file, up to `length` bytes, and returns a string, or
bytes if the file was opened in a binary mode. Without a length, it
reads to the end of the file.

In text mode, the data must be valid UTF-8, and anything else raises an
error, instead of being silently replaced. To read arbitrary bytes, open
the file in a binary mode, such as `'rb'`. `io.stdin` is already binary.

The file must be open in a mode that allows reading, which includes the
default mode. If you read the whole file, it's closed for you.
Otherwise, call `close()` when you've finished.

**Parameters**

- `length` (`?int`)

**Returns** `string|bytes`

## `gets()`

```zuri,ignore
gets(length: ?int) -> string|bytes
```

Same as `read()`, but doesn't open or close the file automatically.

**Parameters**

- `length` (`?int`)

**Returns** `string|bytes`

## `write()`

```zuri,ignore
write(data: string|bytes) -> string|bytes
```

Writes a string or bytes to an opened file at the current insertion
point. When the file is opened with the `a` mode enabled, write will
always start from the end of the file. If the `seek()` method has been
previously called, write will begin from the seeked position, otherwise
it will start at the beginning of the file.

_An example has been given above._

**Parameters**

- `data` (`string|bytes`)

**Returns** `string|bytes`

## `puts()`

```zuri,ignore
puts(data: string|bytes) -> string|bytes
```

Same as `write()`, but doesn't open or close the file automatically.

**Parameters**

- `data` (`string|bytes`)

**Returns** `string|bytes`

## `number()`

```zuri,ignore
number() -> int
```

Returns the file descriptor that the operating system uses for this
file. This is useful for low-level interfaces that work with file
descriptors.

For example:

```zuri-repl
%> file('sample.txt').number()
6
```

A standard stream reports the descriptor it's known by, `0`, `1` or `2`,
not the private duplicate that the runtime keeps open for it, so
`number()` tells `io.stdout` apart from `io.stderr`.

**Returns** `int`

> **Note:** It's `-1` on a platform without file descriptors, and for any
> handle that isn't currently open.

## `is_tty()`

```zuri,ignore
is_tty() -> boolean
```

Returns `true` if the file is connected to a TTY like device or `false`
otherwise.

For example:

```zuri-repl
%> file('sample.txt').is_tty()
false
%> import io
%> io.stdout.is_tty()   # io.stdin is a file...
true
```

**Returns** `boolean`

## `is_open()`

```zuri,ignore
is_open() -> boolean
```

Returns `true` if the file is open for reading or writing and `false`
otherwise.

> **_@note:_** `std` files are always open.

For example:

```zuri-repl
%> file('sample.txt').is_open()
true
```

**Returns** `boolean`

## `is_closed()`

```zuri,ignore
is_closed() -> boolean
```

Returns `true` if the file is closed for reading or writing and `false`
otherwise.

For example:

```zuri-repl
%> file('sample.txt').is_closed()
false
```

**Returns** `boolean`

## `flush()`

```zuri,ignore
flush() -> void
```

Flushes the buffer held by a file. This could be useful for writable
files as file writes are buffered.

For example:

```zuri-repl
%> w.flush()
```

**Returns** `void`

## `stats()`

```zuri,ignore
stats() -> dict
```

Returns the statistics or details of a file.

For example:

```zuri-repl
%> file('sample.txt').stats()
{is_readable: true, is_writable: true, is_executable: false,
is_symbolic: false, size: 72, mode: 33188, dev: 16777230,
ino: 4865113, nlink: 1, uid: 501, gid: 20, mtime: 1631395239,
atime: 1631395271, ctime: 1631395239, blocks: 8, blksize: 4096}
```

Every key above is present on every platform, so reading `size` or
`mtime` needs no check of which one you are on.

**Returns** `dict`

> **Note:** Windows keeps a different set of facts about a file, and the
> ones it has no answer for read as `0`: `dev`, `ino`, `uid` and `gid`,
> with `nlink` always `1`. `mode` is assembled from the file's type and
> its read-only attribute, so it carries the right file-type bits and
> either `0o444` or `0o666`, widened by `0o111` for a directory or a name
> `PATHEXT` says the shell would run. `ctime` is the file's creation time
> there, Windows having no equivalent of a Unix inode-change time.

## `symlink()`

```zuri,ignore
symlink() -> boolean
```

Creates a symbolic link to this file at the given path.

For example:

```zuri-repl
%> file('sample.txt').symlink('sample2.txt')
true
```

**Returns** `boolean`

> **Note:** Windows decides when a link is created whether it's a link to
> a file or a directory, so the original is checked first. A link to
> something that doesn't exist yet is created as a file link. Creating any
> symbolic link on Windows requires privileges, and it fails unless the
> machine is in developer mode or the process is elevated.

## `delete()`

```zuri,ignore
delete() -> boolean
```

Deletes a file.

For example:

```zuri-repl
%> file('test-2.zu').delete()
true
```

**Returns** `boolean`

> **Note:** If the file is opened by one or more processes or threads
> outside of the current process or thread, the file will not be deleted
> until the last process frees it.

> **Note:** This method throws Error on failure.

## `rename()`

```zuri,ignore
rename(new_name: string) -> boolean
```

Renames the file to `new_name`. The new name can be a full path to
another location, in which case the file is moved.

For example:

```zuri-repl
%> file('sample copy.txt').rename('sample-2.txt')
true
```

**Parameters**

- `new_name` (`string`)

**Returns** `boolean`

> **Note:** The new name can't be empty.

> **Note:** This method raises an error if it fails.

## `path()`

```zuri,ignore
path() -> string
```

Returns the path to the file.

For example:

```zuri-repl
%> file('sample.txt').path()
'sample.txt'
```

**Returns** `string`

## `abs_path()`

```zuri,ignore
abs_path() -> string
```

Returns the absolute path to the file.

For example:

```zuri-repl
%> file('sample.txt').abs_path()
'C:\Users\username\zuri-docs\sample.txt'
```

**Returns** `string`

## `copy()`

```zuri,ignore
copy(path: string) -> boolean
```

Copies the file to the given path.

For example:

```zuri-repl
%> file('./sample.txt').copy('samp.txt')
true
```

**Parameters**

- `path` (`string`)

**Returns** `boolean`

## `truncate()`

```zuri,ignore
truncate(length: ?number) -> boolean
```

Truncates the entire file if length is not given or truncates the file
such that only length number of bytes is left in it.

For example:

```zuri-repl
%> file('./samp.txt').truncate()
true
```

**Parameters**

- `length` (`?number`)

**Returns** `boolean`

## `chmod()`

```zuri,ignore
chmod(mode: int) -> boolean
```

Changes the permission on the file to the one specified in the number
given.

> **_@note:_** The number is required to be an octal number.
> e.g. 0c755

For example:

```zuri-repl
%> file('sample.txt').chmod(0c755)
true
```

**Parameters**

- `mode` (`int`)

**Returns** `boolean`

> **Note:** Windows stores one read-only attribute where Unix stores nine
> permission bits, so the owner-write bit decides it and the rest are
> dropped: `0o755` and `0o700` are the same instruction there. A mode with
> no owner-write bit marks the file read-only.

## `set_times()`

```zuri,ignore
set_times(atime: number, mtime: number) -> boolean
```

Sets the last access time and last modified time of the file.

> **_@note:_** Time is expected in UTC seconds<br>
> **_@note:_** set argument -1 to leave the current value.

For example:

```zuri-repl
%> file('sample.txt').set_times(time(), time())
true
%> file('sample.txt').stats()
{is_readable: true, is_writable: true, is_executable: true,
is_symbolic: false, size: 72, mode: 33261, dev: 16777230,
ino: 4865113, nlink: 1, uid: 501, gid: 20, mtime: 1631477099,
atime: 1631477100, ctime: 1631477099, blocks: 8, blksize: 4096}
```

**Parameters**

- `atime` (`number`)
- `mtime` (`number`)

**Returns** `boolean`

## `seek()`

```zuri,ignore
seek(offset: number, seek_type: int) -> boolean
```

Sets the position of a file reader or writer in a file. The position
must be within the range of the file size. _seek_type_ must be on of
`SEEK_SET`, `SEEK_CUR` or `SEEK_END` from the `io` package.

For example:

```zuri-repl
%> f.seek(5, io.SEEK_SET)
true
```

**Parameters**

- `offset` (`number`)
- `seek_type` (`int`)

**Returns** `boolean`

## `tell()`

```zuri,ignore
tell() -> number
```

Returns the current position of the reader/writer in a file.

For example:

```zuri-repl
%> import io
%> var f = file('sample.txt')
%> f.seek(5, io.SEEK_SET)
true
%> f.tell()
5
```

**Returns** `number`

## `mode()`

```zuri,ignore
mode() -> string
```

Returns the mode in which the current file was opened.<br>

For example:

```zuri-repl
%> file('sample.txt').mode()
'r'
```

**Returns** `string`

## `name()`

```zuri,ignore
name() -> string
```

Returns the name of the current file.<br>

For example:

```zuri-repl
%> file('./sample.txt').name()
'sample.txt'
```

**Returns** `string`

## `to_string()`

```zuri,ignore
to_string() -> string
```

Returns the file handle as a string, naming its path and the mode it was
opened in.

```zuri-repl
%> file('sample.txt', 'w').to_string()
'<file at sample.txt in mode w>'
```

**Returns** `string`
