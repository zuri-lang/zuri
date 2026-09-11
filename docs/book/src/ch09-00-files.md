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
echo file('notes.txt').read()
```

`read()` with no argument opens the file, reads all of it, and closes it
again. That is the one-liner for "give me this file's contents", and it is
what you want most of the time.

In text mode you get a string, decoded strictly as UTF-8. In binary mode
you get `bytes`.

`read(length)` reads at most that many bytes and leaves the handle open, so
you can call it again:

```zuri
var handle = file('big.bin', 'rb')
handle.open()

while true {
  var chunk = handle.read(65536)

  if chunk.is_empty() {
    break
  }

  process(chunk)
}

handle.close()
```

`gets(length)` is the same as `read()` except that it never opens or closes
anything for you. Use it when you are managing the handle yourself.

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
```

```console
first line
second line
```

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

```zuri
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

`stats()` returns a dictionary of metadata:

```zuri
var info = file('notes.txt').stats()

echo info.size
echo info.is_file
```

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

```zuri
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

```zuri
echo os.read_dir('r')
echo os.read_dir('r', true)
```

```console
[., .., top.txt, inner]
[., .., top.txt, inner, inner/nested.txt]
```

`read_dir()` includes `.` and `..`, and the recursive form returns nested
entries as paths relative to the directory you asked about.

`glob()` is usually what you actually want:

```zuri
echo os.glob('*.txt', 'sub')
echo os.glob('**/*.txt', 'r')
```

```console
[a.txt]
[inner/nested.txt]
```

`*` matches within one path segment and `**` matches across segments. The
second argument is the base directory, and results come back relative to
it.

`**/*.txt` means "in a subdirectory, a `.txt` file", so it does not match a
file sitting directly in the base directory:

```zuri
echo os.glob('*.txt', 'wc')
echo os.glob('**/*.txt', 'wc')
echo os.glob('**', 'wc')
```

```console
[a.txt]
[sub/b.txt]
[a.txt, sub, sub/b.txt]
```

To walk a whole tree, glob `**` and filter:

```zuri
os.glob('**', root).filter(@(p) => p.ends_with('.txt'))
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

Counting words across every text file in a directory tree:

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

echo word_count('wc')
```

```console
{hello: 2, world: 2, of: 1, zuri: 1}
```

Three things in that function are worth pointing at. `glob()` gives paths
relative to the root, so they have to be joined back onto it.
`split('/\W+/')` uses a regular expression, which is why punctuation does
not end up in the keys. And `counts.get(word, 0)` supplies the default that
makes the increment work on a key that is not there yet.
