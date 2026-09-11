# `file`

27 methods. See [Files](ch09-00-files.md) for the guided introduction.

| Method | Returns |
| --- | --- |
| [`exists()`](#exists) | `boolean` |
| [`close()`](#close) | `void` |
| [`open()`](#open) | `void` |
| [`read(length: ?int)`](#read) | `string\|bytes` |
| [`gets(length: ?int)`](#gets) | `string\|bytes` |
| [`write(data: string\|bytes)`](#write) | `string\|bytes` |
| [`puts(data: string\|bytes)`](#puts) | `string\|bytes` |
| [`number()`](#number) | `int` |
| [`is_tty()`](#is_tty) | `boolean` |
| [`is_open()`](#is_open) | `boolean` |
| [`is_closed()`](#is_closed) | `boolean` |
| [`flush()`](#flush) | `void` |
| [`stats()`](#stats) | `dict` |
| [`symlink()`](#symlink) | `boolean` |
| [`delete()`](#delete) | `boolean` |
| [`rename(new_name: string)`](#rename) | `boolean` |
| [`path()`](#path) | `string` |
| [`abs_path()`](#abs_path) | `string` |
| [`copy(path: string)`](#copy) | `boolean` |
| [`truncate(length: ?number)`](#truncate) | `boolean` |
| [`chmod(mode: int)`](#chmod) | `boolean` |
| [`set_times(atime: number, mtime: number)`](#set_times) | `boolean` |
| [`seek(offset: number, seek_type: int)`](#seek) | `boolean` |
| [`tell()`](#tell) | `number` |
| [`mode()`](#mode) |  |
| [`to_string()`](#to_string) | `string` |
| [`name()`](#name) | `string` |

## `exists()`

Returns `true` if a file exists or `false` otherwise.

For example:

```zuri
%> file('sample.txt').exists()
true
```

- **Returns** `boolean`

## `close()`

Closes the stream to an opened file. You'll rarely ever need
to call this method yourself in most use cases.

For example:

```zuri
%> var f = file('sample.txt')
%> f.close()
```

- **Returns** `void`

## `open()`

Opens the stream to a file for the operation originally
specified on the file object during creation. You may need
to call this method after a call to read() if the length
isn't specified or write() if you wish to read or write
again as the file will already be closed.

For example:

```zuri
%> f.open()
```

- **Returns** `void`

## `read(length: ?int)`

Reads the content of an opened file up to the specified
length and returns it as string or bytes if the file was
opened in the binary mode. If the length is not specified,
the file will be read to the end.

In text mode the bytes read must be valid UTF-8; anything else
raises rather than being silently replaced. Open the file in a
binary mode (`'rb'`) to read arbitrary bytes instead. Note that
`io.stdin` is already binary.

This method requires that the file be opened in the read
mode (default mode) or a mode that supports reading. If you
aren't reading the full length of the file, you'll need to
call the `close()` method to free the file for further
reading, otherwise, the `close()` method will be
automatically called for you.

_An example has been given above._

- **Parameter** `?int` length
- **Returns** `string|bytes`

## `gets(length: ?int)`

Same as `read()`, but doesn't open or close the file
automatically.

- **Parameter** `?int` length
- **Returns** `string|bytes`

## `write(data: string|bytes)`

Writes a string or bytes to an opened file at the current
insertion point. When the file is opened with the `a` mode
enabled, write will always start from the end of the file.
If the `seek()` method has been previously called, write
will begin from the seeked position, otherwise it will start
at the beginning of the file.

_An example has been given above._

- **Parameter** `string|bytes` data
- **Returns** `string|bytes`

## `puts(data: string|bytes)`

Same as `write()`, but doesn't open or close the file
automatically.

- **Parameter** `string|bytes` data
- **Returns** `string|bytes`

## `number()`

Returns the integer file descriptor number that is used by
the underlying implementation to request I/O operations from
the operating system. This can be very useful for low-level
interfaces that uses or act as file descriptors.

For example:

```zuri
%> file('sample.txt').number()
6
```

- **Returns** `int`

## `is_tty()`

Returns `true` if the file is connected to a TTY like device
or `false` otherwise.

For example:

```zuri
%> file('sample.txt').is_tty()
false
%> import io
%> io.stdout.is_tty()   # io.stdin is a file...
true
```

- **Returns** `boolean`

## `is_open()`

Returns `true` if the file is open for reading or writing
and `false` otherwise.

> **_@note:_** `std` files are always open.

For example:

```zuri
%> file('sample.txt').is_open()
true
```

- **Returns** `boolean`

## `is_closed()`

Returns `true` if the file is closed for reading or writing
and `false` otherwise.

For example:

```zuri
%> file('sample.txt').is_closed()
false
```

- **Returns** `boolean`

## `flush()`

Flushes the buffer held by a file. This could be useful for
writable files as file writes are buffered.

For example:

```zuri
%> w.flush()
```

- **Returns** `void`

## `stats()`

Returns the statistics or details of a file.

For example:

```zuri
%> file('sample.txt').stats()
{is_readable: true, is_writable: true, is_executable: false, is_symbolic: false, size: 72, mode: 33188, dev: 16777230,
ino: 4865113, nlink: 1, uid: 501, gid: 20, mtime: 1631395239, atime: 1631395271, ctime: 1631395239, blocks: 8,
blksize: 4096}
```

- **Returns** `dict`

## `symlink()`

Creates a symbolic link for the original file at the
specified path.

For example:

```zuri
%> file('sample.txt').symlink('sample2.txt')
true
```

- **Returns** `boolean`

## `delete()`

Deletes a file.

   threads outside of the current process or thread, the
   file will not be deleted until the last process frees it.
For example:
```zuri
%> file('test-2.zu').delete()
true
```

- **Note** If the file is opened by one or more processes or
- **Note** This method throws Error on failure.
- **Returns** `boolean`

## `rename(new_name: string)`

Renames a file to to `new_name`. The new name can be a full
path in another location in which case the file will be
moved.

For example:
```zuri
%> file('sample copy.txt').rename('sample-2.txt')
true
```

- **Note** The new name cannot be empty
- **Note** This method throws Error on failure.
- **Parameter** `string` new_name
- **Returns** `boolean`

## `path()`

Returns the path to the file.

For example:

```zuri
%> file('sample.txt').path()
'sample.txt'
```

- **Returns** `string`

## `abs_path()`

Returns the absolute path to the file.

For example:

```zuri
%> file('sample.txt').abs_path()
'C:\Users\username\zuri-docs\sample.txt'
```

- **Returns** `string`

## `copy(path: string)`

Copies a file from the path specified in the original file
to the given path.

For example:

```zuri
%> file('./sample.txt').copy('samp.txt')
true
```

- **Parameter** `string` new_name
- **Returns** `boolean`

## `truncate(length: ?number)`

Truncates the entire file if length is not given or
truncates the file such that only length number of bytes is
left in it.

For example:

```zuri
%> file('./samp.txt').truncate()
true
```

- **Parameter** `?number` length
- **Returns** `boolean`

## `chmod(mode: int)`

Changes the permission on the file to the one specified in
the number given.

> **_@note:_** The number is required to be an octal number.
> e.g. 0c755

For example:

```zuri
%> file('sample.txt').chmod(0c755)
true
```

- **Parameter** `int` mode
- **Returns** `boolean`

## `set_times(atime: number, mtime: number)`

Sets the last access time and last modified time of the
file.

> **_@note:_** Time is expected in UTC seconds
> **_@note:_** set argument -1 to leave the current value.

For example:

```zuri
%> file('sample.txt').set_times(time(), time())
true
%> file('sample.txt').stats()
{is_readable: true, is_writable: true, is_executable: true, is_symbolic: false, size: 72, mode: 33261,
dev: 16777230, ino: 4865113, nlink: 1, uid: 501, gid: 20, mtime: 1631477099, atime: 1631477100, ctime:
1631477099, blocks: 8, blksize: 4096}
```

- **Parameter** `number` atime
- **Parameter** `number` mtime
- **Returns** `boolean`

## `seek(offset: number, seek_type: int)`

Sets the position of a file reader or writer in a file. The
position must be within the range of the file size.
_seek_type_ must be on of `SEEK_SET`, `SEEK_CUR` or
`SEEK_END` from the `io` package.

For example:

```zuri
%> f.seek(5, io.SEEK_SET)
true
```

- **Parameter** `number` offset
- **Parameter** `int` seek_type
- **Returns** `boolean`

## `tell()`

Returns the current position of the reader/writer in a file.

For example:

```zuri
%> import io
%> var f = file('sample.txt')
%> f.seek(5, io.SEEK_SET)
true
%> f.tell()
5
```

- **Returns** `number`

## `mode()`

Returns the mode in which the current file was opened.

For example:

```zuri
%> file('sample.txt').mode()
'r'

@return {string}

## `name()`

Returns the name of the current file.

For example:

```zuri
%> file('./sample.txt').name() 'sample.txt'
```

- **Returns** `string`

## `to_string()`

The handle rendered for display, as `<file at PATH in mode MODE>`.

```zuri
%> file('sample.txt')
<file at sample.txt in mode r>
```

- **Returns** `string`
