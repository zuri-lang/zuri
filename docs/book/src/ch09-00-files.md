# Files and the Filesystem

Two things handle most of the work with files. The built-in `file()`
function gives you a handle to a single file, and the `os` module covers
everything else: directories, paths, globs, temporary files and environment
variables.

## Opening a File

```zuri
var handle = file('notes.txt')
var writer = file('notes.txt', 'w')
```

`file(path)` opens a file for reading by default. The second argument sets
the mode:

| Mode | Meaning |
| --- | --- |
| `r` | Read. The file must exist. |
| `w` | Write. Creates the file, or truncates it if it exists. |
| `a` | Append. Every write goes to the end. Creates the file if needed. |
| `r+` | Read and update. The file must exist. |
| `w+` | Read and update. Creates the file, but does **not** truncate it. |
| `a+` | Read and append. Creates the file if needed. |
| `x` | Write. Creates the file, and fails if the path already exists. |
| `x+` | Read and write. Creates the file, and fails if the path already exists. |

Add `b` to any mode to open the file in binary mode, as in `'rb'`, `'wb'` or
`'ab'`.

Use `x` when two programs might try to create the same file at the same
time. Checking whether the file exists and creating it happen in a single
step, so exactly one program succeeds and all the others fail. That's what a
lock file needs.

Creating a handle doesn't touch the disk. Nothing happens until you read,
write or call `open()`.

## Reading an Entire File

```zuri
file('notes.txt', 'w').write('It works!')

echo file('notes.txt').read()
```

```console
It works!
```

`read()` without an argument opens the file, reads all of it and closes it
again. It's the one-line way to get a file's contents, and it's what you'll
want most of the time.

In text mode, you get a string, decoded as strict UTF-8. In binary mode, you
get `bytes`.

### Reading in Chunks

`read(length)` reads up to that many bytes and leaves the handle open, so
you can call it again. This is how you process a file that's too large to
fit in memory all at once:

```zuri
file('notes.txt', 'w').write('alpha\nbeta\ngamma\n')

var handle = file('notes.txt')
handle.open()

var chunks = 0

while true {
  var chunk = handle.read(6)

  if chunk.is_empty() {
    break
  }

  chunks++
}

handle.close()

echo chunks
```

```console
3
```

Reading six bytes at a time is only for demonstration, and in real code
you'd read tens of kilobytes at a time. What matters is the pattern: open
the file once, read until you get an empty result, then close it once.

Note that chunks end wherever the byte count runs out, not at the end of a
line. If a chunked reader needs complete lines, it has to keep the end of
each chunk and join it to the start of the next one.

### Reading Lines

To process text one line at a time, the simplest approach is to read the
whole file and split it:

```zuri
file('notes.txt', 'w').write('alpha\nbeta\ngamma\n')

for line in file('notes.txt').read().lines() {
  echo '[${line}]'
}
```

```console
[alpha]
[beta]
[gamma]
```

`lines()` handles both `\n` and `\r\n`, and it drops the empty string that a
trailing newline would otherwise produce at the end.

## Writing to a File

```zuri
file('notes.txt', 'w').write('It works!')
```

Like `read()`, `write()` opens the handle if it's closed, writes the data,
flushes it and closes the handle again. One call writes one complete file.

Closing automatically has an important consequence. **If you call `write()`
twice on a closed handle in `w` mode, each call truncates the file, so only
the second write is kept.** When you need to write more than once, open the
handle yourself:

```zuri
var handle = file('notes.txt', 'w')
handle.open()

handle.write('first line\n')
handle.write('second line\n')

handle.close()

echo file('notes.txt').read()
```

```console
first line
second line

```

The explicit `open()` call keeps the handle open for both writes. Without
it, each `write()` would open the file, truncate it, write to it and close
it, and only `second line` would be left.

If the handle is already open, `write()` writes at the current position, so
the code above does what it looks like it does.

`puts()` writes without opening or closing the handle at all. The handle has
to be open already, and `puts()` is the method to use inside a loop.

## Reading and Writing at a Specific Position

```zuri
var handle = file('notes.txt')
handle.open()

handle.seek(6, 0)
echo handle.tell()
echo handle.read(4)

handle.close()
```

`seek(offset, whence)` takes `0` for the start of the file, `1` for the
current position and `2` for the end. The `io` module has names for these
values:

```zuri,ignore
import io

handle.seek(0, io.SEEK_SET)
handle.seek(-10, io.SEEK_END)
```

`tell()` returns the current position.

## Getting Information About a File

```zuri
var handle = file('notes.txt')

echo handle.exists()
echo handle.path()
echo handle.abs_path()
echo handle.name()
echo handle.mode()
echo handle.is_open()
echo handle.is_closed()
```

`stats()` returns a dictionary of information about the file on disk:

```zuri
file('notes.txt', 'w').write('alpha\nbeta\n')

var info = file('notes.txt').stats()

echo info.size
echo info.is_readable
echo info.keys()
```

```console
11
true
[is_readable, is_writable, is_executable, is_symbolic, size, mode, dev, ino, nlink, uid, gid, mtime, atime, ctime, blocks, blksize]
```

`size` is in bytes. `mtime`, `atime` and `ctime` are times in seconds since
the epoch, which you can pass straight to the `date` module. `mode` is the
raw word that holds the permissions and the type of entry, and the `stat`
module helps you make sense of it:

```zuri
import stat

file('notes.txt').chmod(0c644)

var info = file('notes.txt').stats()

echo stat.S_ISREG(info.mode)
echo stat.S_ISDIR(info.mode)
echo stat.file_mode(info.mode)
```

```console
true
false
-rw-r--r--
```

`S_ISREG`, `S_ISDIR`, `S_ISLNK` and the other functions like them each check
for one kind of entry. `file_mode()` formats the permission bits the same
way `ls -l` does.

## Managing Files

```zuri
file('notes.txt').copy('backup.txt')
file('backup.txt').rename('archive.txt')
file('archive.txt').delete()
```

File handles also have `truncate(length)`, `chmod(mode)`,
`set_times(access, modify)` and `symlink(target)`.

## Closing Files

`read()` and `write()` clean up after themselves, but you're responsible for
closing anything you opened with `open()`. The cleanest way to make sure it
happens is a `catch` that closes the file on the way out:

```zuri,ignore
var handle = file(path, 'w')
handle.open()

catch {
  write_everything(handle)
} as e

handle.close()

if e {
  raise e
}
```

For cleanup that has to happen however the program ends, rather than however
a single block ends, register a handler with `os.at_exit()`:

```zuri
import os

var scratch = file('scratch.txt', 'w')
scratch.open()

os.at_exit(@{
  scratch.close()
  scratch.delete()
})

scratch.write('working notes')
echo scratch.is_open()
```

```console
true
```

Handlers run in reverse order of registration. They run whether the program
reached the end of its script, called `os.exit()` or stopped because of an
uncaught error. If a handler raises an error, the error is reported on
standard error and the remaining handlers still run, so one failed cleanup
can't stop the others.

## Directories

```zuri
import os

os.create_dir('sub/deep', nil, true)
```

The three arguments are the path, the permission bits, and whether to create
any missing parent directories. It returns `false` if the directory already
existed.

```zuri
echo os.dir_exists('sub')
echo os.is_dir('sub')
echo os.remove_dir('sub', true)
```

If the second argument to `remove_dir` is true, it removes the directory and
everything in it.

## Listing Files and Globbing

The examples in this section work on a real directory tree, so let's create
one first:

```zuri
import os

os.create_dir('tree/sub', nil, true)

file('tree/top.txt', 'w').write('a')
file('tree/sub/nested.txt', 'w').write('b')
file('tree/sub/notes.md', 'w').write('c')
```

`read_dir()` lists the contents of a directory:

```zuri
echo os.read_dir('tree')
echo os.read_dir('tree', true)
```

```console
[., .., sub, top.txt]
[., .., sub, sub/nested.txt, sub/notes.md, top.txt]
```

There are three things to notice here. First, `.` and `..` are included, so
a loop over the result usually needs to skip them. Second, the recursive
form returns nested entries as paths **relative to the directory you
listed**, not as bare names, so you can use them directly. Third, entries
are sorted by name, and each directory's contents come immediately after the
directory itself, so the listing is the same on every machine and file
system.

`glob()` is usually what you really want:

```zuri
echo os.glob('*.txt', 'tree')
echo os.glob('**/*.txt', 'tree')
```

```console
[top.txt]
[sub/nested.txt]
```

`*` matches within a single part of a path, while `**` matches across parts.
The second argument is the base directory, and the results are relative to
it.

Compare those two results carefully, because the difference often catches
people out. `*.txt` found the file at the top level but not the nested one.
`**/*.txt` found the nested file but **not** the top-level one, because
`**/` means "in a subdirectory". Neither pattern finds both.

To match files at any depth, glob for `**` and filter the results:

```zuri
echo os.glob('**', 'tree')
echo os.glob('**', 'tree').filter(@(p) => p.ends_with('.txt'))
```

```console
[sub, sub/nested.txt, sub/notes.md, top.txt]
[sub/nested.txt, top.txt]
```

`**` on its own matches every entry at every depth, including directories,
which is why the filter on the second line is needed. `glob()` uses
`read_dir()` internally, so its matches come back in the same sorted order.

Clean up when you're done:

```zuri
echo os.remove_dir('tree', true)
```

```console
true
```

## Paths

All of the path functions just manipulate strings, except where noted:

```zuri
echo os.join_paths('sub', 'a.txt')
echo os.base_name('sub/a.txt')
echo os.dir_name('sub/a.txt')
echo os.real_path('sub')
echo os.relative_path(os.cwd(), '/full/path/to/sub')
```

```console
sub/a.txt
a.txt
sub
/home/you/project/sub
sub
```

`real_path()` resolves symbolic links, and the path has to exist.
`abs_path()` doesn't require that. `expand_user()` replaces a leading `~`
with the home directory. `path_contains(base, candidate)` checks whether one
path is inside another, which is the check you need before serving a file
that a user asked for by name. On Windows, both `path_contains()` and
`relative_path()` ignore case, the same way Windows does when it looks up
names.

```zuri
echo os.cwd()
echo os.home_dir()
os.change_dir('/some/where')
```

## Temporary Files

```zuri
echo os.temp_dir()

var path = os.create_temp_file('report-', '.csv')
var dir = os.create_temp_dir('build-')
```

Both functions create the file or directory and return its path. You're
responsible for removing them when you're done.

## Environment Variables

```zuri
echo os.get_env('HOME')
echo os.get_env('NOPE', 'fallback')

os.set_env('ZURI_BOOK', '1')
os.unset_env('ZURI_BOOK')

echo os.environ()
echo os.expand_vars('$HOME/projects')
```

`get_env()` accepts a fallback value. `environ()` returns all of the
environment variables as a dictionary.

## Finding Files Relative to Your Code

The current working directory is wherever the user ran `zuri` from, which
isn't necessarily where your source code is. Use `__file__` instead:

```zuri
import os

var HERE = os.dir_name(__file__)
var templates = os.join_paths(HERE, 'templates')
```

Do this in every module that reads a file stored next to it. Otherwise, your
program will only work when it's run from the project root.

## Watching for File Changes

An `os.Watcher` is notified by the operating system as files change. It
doesn't poll the files repeatedly. Instead, it uses each platform's own
notification mechanism: inotify on Linux, FSEvents on macOS,
`ReadDirectoryChangesW` on Windows, and kqueue on the BSDs. FSEvents only
allows about one watched directory for every twelve files a process can have
open, so on macOS, the first watcher raises the process's soft limit on open
files as far as the hard limit allows.

```zuri
import os

var dir = os.create_temp_dir('notes-')
var watcher = os.Watcher().add(dir)

file(os.join_paths(dir, 'todo.txt'), 'w').write('buy milk')

var event = watcher.next(5000)

echo '${event.kind} ${os.base_name(event.path)}'

watcher.close()
os.remove_dir(dir, true)
```

```console
create todo.txt
```

`add()` accepts either a directory, which is watched along with everything
inside it, or a single file. `next()` waits for the next event. It takes a
timeout in milliseconds and returns `nil` if nothing happened in that time.
Each event's `kind` is one of the following:

- `'create'`: something now exists at `path`.
- `'modify'`: the file at `path` was changed in place.
- `'remove'`: nothing exists at `path` anymore.
- `'rescan'`: the operating system dropped some events, so anything might
  have changed.

A rename is reported as a `'remove'` of the old path and a `'create'` of the
new one. Reading a file is never reported.

A watcher can also follow a child process. When the process exits, the
watcher reports it alongside the file changes:

```zuri
import os

var child = os.spawn(os.exe_path, ['--version'], { stdout: 'null' })
var watcher = os.Watcher().follow(child)

var event = watcher.next(5000)

echo event.kind
echo event.process.wait()

watcher.close()
```

```console
exit
0
```

A `for` loop over a watcher keeps waiting for events until the watcher is
closed. Signal handlers still run while it waits, so pressing Ctrl+C is the
usual way to stop one:

```zuri,ignore
import os

var watcher = os.Watcher().add('templates')

os.on_signal('INT', @() {
  watcher.close()
  return true
})

for event in watcher {
  echo '${event.kind} ${event.path}'
}

echo 'stopped watching'
```

`zuri run --watch` is built on top of this.

## Example: Counting Words

Here's a complete function that counts the words in every text file in a
directory tree:

```zuri
import os

def text_files(root) {
  return os.glob('**', root).filter(@(p) => p.ends_with('.txt'))
}

def word_count(root) {
  var counts = {}

  for path in text_files(root) {
    var text = file(os.join_paths(root, path)).read()

    for word in text.lower().split('/\W+/') {
      if word.is_empty() {
        continue
      }

      counts.set(word, counts.get(word, 0) + 1)
    }
  }

  return counts
}

os.create_dir('wc/sub', nil, true)

file('wc/a.txt', 'w').write('Hello world, hello!')
file('wc/sub/b.txt', 'w').write('World of Zuri.')
file('wc/sub/skip.md', 'w').write('not counted')

echo word_count('wc')

os.remove_dir('wc', true)
```

```console
{hello: 2, world: 2, of: 1, zuri: 1}
```

There are four things in that function worth pointing out.

`glob('**', root)` returns paths **relative to the root**, so you have to
join each one to the root before you can open it. Forgetting to do that is
the most common mistake in code that uses globs.

The `.md` file doesn't appear in the result, because `text_files()` filtered
it out. That's the filtering that `**` can't do by itself.

`split('/\W+/')` splits on a regular expression, which is why no punctuation
ends up in the keys: `world,` and `hello!` became `world` and `hello`. A
plain `split(' ')` would have kept the punctuation.

And `counts.get(word, 0) + 1` provides a starting value for keys that don't
exist yet. Without the fallback, the first time each word appeared would
raise an error.

## Further Reading

Binary files, byte streams and the `io` module's in-memory files are covered
in [Chapter 10](ch10-00-binary-data.md). Reading files over the network is
covered in [Chapter 12](ch12-00-networking.md). [Appendix
E](appendix-05-09-file.md) lists every method of a file handle.
