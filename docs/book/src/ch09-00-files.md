# Files and the Filesystem

Two things do the work here. The built-in `file()` function gives you a
handle to one file. The `os` module covers everything else: directories,
paths, globs, temporary files and the environment.

## Opening a File

```zuri
var handle = file('notes.txt')
var writer = file('notes.txt', 'w')
```

`file(path)` defaults to read mode. The second argument is the mode:

| Mode | Meaning |
| --- | --- |
| `r` | read; the file must exist |
| `w` | write; creates the file, truncates an existing one |
| `a` | append; writes always go to the end, creates the file |
| `r+` | read and update; the file must exist |
| `w+` | read and update; creates the file, does **not** truncate |
| `a+` | read and append; creates the file |

Append `b` to any of them for binary mode: `'rb'`, `'wb'`, `'ab'`.

Creating a handle does not touch the disk. Nothing happens until you read,
write or call `open()`.

## Reading a Whole File

```zuri
file('notes.txt', 'w').write('It works!')

echo file('notes.txt').read()
```

```console
It works!
```

`read()` with no argument opens the file, reads all of it, and closes it
again. That is the one-liner for "give me this file's contents", and it is
what you want most of the time.

In text mode you get a string, decoded strictly as UTF-8. In binary mode
you get `bytes`.

### Reading in Chunks

`read(length)` reads at most that many bytes and leaves the handle open, so
you can call it again. This is how you process a file too large to hold in
memory at once:

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

Six bytes at a time is a demonstration; in real code the chunk is tens of
kilobytes. The shape is what matters: open once, read until you get an
empty result, close once.

Note that chunks fall wherever the byte count lands, not on line
boundaries. A chunked reader that needs whole lines has to keep the tail of
each chunk and join it to the front of the next.

### Reading Lines

For text you want line by line, the simplest form reads the file and splits
it:

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

`lines()` handles both `\n` and `\r\n`, and drops the trailing empty
piece a final newline would otherwise produce.

## Writing

```zuri
file('notes.txt', 'w').write('It works!')
```

Like `read()`, `write()` opens the handle if it is closed, writes, flushes,
and closes it again. One call, one complete file.

That auto-close has a consequence worth being precise about. **Two
consecutive `write()` calls on a closed handle in `w` mode each truncate
the file, so only the last one survives.** When you are writing more than
once, open the handle yourself:

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

The explicit `open()` is what keeps the handle open across both writes.
Without it, each `write()` would open, truncate, write and close, and only
`second line` would survive.

An already-open handle is written to where it stands, so the sequence above
does exactly what it reads like.

`puts()` writes without ever opening or closing. It requires an open
handle, and it is the method to reach for inside a loop.

## Reading and Writing at a Position

```zuri
var handle = file('notes.txt')
handle.open()

handle.seek(6, 0)
echo handle.tell()
echo handle.read(4)

handle.close()
```

`seek(offset, whence)` takes `0` for the start of the file, `1` for the
current position and `2` for the end. The `io` module names them:

```zuri,ignore
import io

handle.seek(0, io.SEEK_SET)
handle.seek(-10, io.SEEK_END)
```

`tell()` reports the current offset.

## Asking About a File

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

`stats()` returns a dictionary of metadata about the file on disk:

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

`size` is in bytes. `mtime`, `atime` and `ctime` are epoch seconds, ready
to hand to the `date` module. `mode` is the raw permission-and-type word,
and the `stat` module is what turns it into an answer:

```zuri
import stat

var info = file('notes.txt').stats()

echo stat.S_ISREG(info.mode)
echo stat.S_ISDIR(info.mode)
echo stat.file_mode(info.mode)
```

```console
true
false
-rw-rw-r--
```

`S_ISREG`, `S_ISDIR`, `S_ISLNK` and the rest of the family each answer one
question about the kind of entry. `file_mode()` renders the permission bits
the way `ls -l` does.

## Managing Files

```zuri
file('notes.txt').copy('backup.txt')
file('backup.txt').rename('archive.txt')
file('archive.txt').delete()
```

There is also `truncate(length)`, `chmod(mode)`, `set_times(access,
modify)` and `symlink(target)`.

## Always Close What You Opened

`read()` and `write()` clean up after themselves. Anything you opened with
`open()` is yours to close, and the cleanest way to guarantee it is a
`catch` that closes on the way out:

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

## Directories

```zuri
import os

os.create_dir('sub/deep', nil, true)
```

The three arguments are the path, the permission bits, and whether to
create intermediate directories. It returns `false` when the directory
already existed.

```zuri
echo os.dir_exists('sub')
echo os.is_dir('sub')
echo os.remove_dir('sub', true)
```

`remove_dir`'s second argument makes it recursive.

## Listing and Globbing

Everything in this section works on a real tree, so build one first:

```zuri
import os

os.create_dir('tree/sub', nil, true)

file('tree/top.txt', 'w').write('a')
file('tree/sub/nested.txt', 'w').write('b')
file('tree/sub/notes.md', 'w').write('c')
```

`read_dir()` lists one directory:

```zuri
echo os.read_dir('tree')
echo os.read_dir('tree', true)
```

```console
[., .., top.txt, sub]
[., .., top.txt, sub, sub/nested.txt, sub/notes.md]
```

Two things to notice. `.` and `..` are included, so a loop over the result
almost always wants to skip them. And the recursive form returns nested
entries as paths **relative to the directory you asked about**, not as bare
names — which is what makes them usable directly.

`glob()` is usually what you actually want:

```zuri
echo os.glob('*.txt', 'tree')
echo os.glob('**/*.txt', 'tree')
```

```console
[top.txt]
[sub/nested.txt]
```

`*` matches within one path segment; `**` matches across segments. The
second argument is the base directory, and results come back relative to
it.

Read those two results together, because the distinction catches people
out. `*.txt` found the file at the top and not the nested one. `**/*.txt`
found the nested one and **not** the top-level one, because `**/` means
"in a subdirectory". Neither pattern finds both.

To match at every depth, glob `**` and filter:

```zuri
echo os.glob('**', 'tree')
echo os.glob('**', 'tree').filter(@(p) => p.ends_with('.txt'))
```

```console
[top.txt, sub, sub/nested.txt, sub/notes.md]
[top.txt, sub/nested.txt]
```

`**` on its own matches every entry at every depth, directories included,
which is why the filter is doing real work in the second line.

Clean up when you are done:

```zuri
echo os.remove_dir('tree', true)
```

```console
true
```

## Paths

Every path function is pure string manipulation except where noted:

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

`real_path()` resolves symlinks and requires the path to exist.
`abs_path()` does not. `expand_user()` turns a leading `~` into the home
directory. `path_contains(base, candidate)` answers whether one path is
inside another, which is the check you need before serving a file a user
named.

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

Both create the thing and hand you its path. Clean them up yourself when
you are done.

## Environment Variables

```zuri
echo os.get_env('HOME')
echo os.get_env('NOPE', 'fallback')

os.set_env('ZURI_BOOK', '1')
os.unset_env('ZURI_BOOK')

echo os.environ()
echo os.expand_vars('$HOME/projects')
```

`get_env()` takes a fallback. `environ()` gives the whole set as a
dictionary.

## Locating Files Relative to Your Code

The current working directory is wherever the user ran `zuri` from, which
is not where your source lives. Use `__file__`:

```zuri
import os

var HERE = os.dir_name(__file__)
var templates = os.join_paths(HERE, 'templates')
```

Doing this in every module that reads a file next to itself is the
difference between a program that works and one that works only from the
project root.

## A Worked Example

Counting words across every text file in a directory tree, start to finish:

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

Four things in that function are worth pointing at.

`glob('**', root)` returns paths **relative to the root**, so they have to
be joined back onto it before they can be opened. Forgetting that join is
the most common mistake in code that globs.

The `.md` file is absent from the result because `text_files()` filtered it
out, which is the filter doing the job `**` alone cannot.

`split('/\W+/')` is a regular expression, which is why punctuation does not
end up in the keys — `world,` and `hello!` became `world` and `hello`. A
plain `split(' ')` would have kept both.

And `counts.get(word, 0) + 1` supplies the starting value for a key that
does not exist yet. Without the fallback, the first sighting of every word
would raise.

## Where to Go Next

Binary files, byte streams and the `io` module's in-memory files are
[Chapter 10](ch10-00-binary-data.md). Reading a file over the network is
[Chapter 12](ch12-00-networking.md). The complete list of methods a file
handle carries is [Appendix E](appendix-05-09-file.md).
