#![allow(unused)]

use std::io::{Read, Seek, Write};
use std::sync::LazyLock;

use crate::{
  builtins::{
    MethodTable, build,
    enforce::{
      ArgType, enforce_method_arg_count, enforce_method_arg_range, enforce_method_arg_type,
      enforce_method_arg_type_any_of, enforce_method_arg_type_opt,
    },
    method, method_n, method_opt, to_string,
  },
  vm::{
    object::{FileHandle, ZuriContext},
    value::Value,
  },
};

pub static FILE_METHODS: LazyLock<MethodTable> = LazyLock::new(|| {
  build(vec![
    method("to_string", to_string),
    method("exists", exists),
    method("close", close),
    method("open", open),
    method_opt("read", 0, read),
    method_opt("gets", 0, gets),
    method_n("write", 1, write),
    method_n("puts", 1, puts),
    method("number", number),
    method("is_tty", is_tty),
    method("is_open", is_open),
    method("is_closed", is_closed),
    method("flush", flush),
    method("stats", stats),
    method_n("symlink", 1, symlink),
    method("delete", delete),
    method_n("rename", 1, rename),
    method("path", path),
    method("abs_path", abs_path),
    method_n("copy", 1, copy),
    method_opt("truncate", 0, truncate),
    method_n("chmod", 1, chmod),
    method_n("set_times", 2, set_times),
    method_n("seek", 2, seek),
    method("tell", tell),
    method("mode", mode),
    method("name", name),
  ])
});

fn with_file_mut<F, R>(v: Value, f: F) -> R
where
  F: FnOnce(&mut FileHandle) -> R,
{
  f(&mut v.as_file_cell().borrow_mut())
}

/// Opens `path` per a Zuri mode string (`r`, `w`, `a`, `r+`, `w+`,
/// `a+`, any of those with a trailing/embedded `b`, or a mixed form
/// like `r+w`). Shared by the `file(...)` constructor native
/// (`natives.rs`) and this file's own `.open()` method, so a file
/// re-opened after being closed gets identical semantics to its first
/// open. `w+` deliberately does NOT truncate an existing file, per
/// spec; only bare `w` does.
pub(crate) fn open_with_mode(path: &str, mode: &str) -> Result<std::fs::File, String> {
  let base = mode.replace('b', "");
  let has_plus = base.contains('+');
  let mut opts = std::fs::OpenOptions::new();

  if base.starts_with('a') {
    opts.append(true).create(true);
    if has_plus {
      opts.read(true);
    }
  } else if base.starts_with('w') {
    opts.write(true).create(true);
    if has_plus {
      opts.read(true);
    } else {
      opts.truncate(true);
    }
  } else if base.starts_with('r') || base.is_empty() {
    opts.read(true);
    if has_plus {
      opts.write(true);
    }
  } else {
    // A mixed form like "r+w"; union whatever letters are present.
    if base.contains('r') {
      opts.read(true);
    }
    if base.contains('w') {
      opts.write(true).create(true);
    }
    if base.contains('a') {
      opts.append(true).create(true);
    }
  }

  opts
    .open(path)
    .map_err(|e| format!("could not open '{}' in mode '{}': {}", path, mode, e))
}

fn exists(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  let exists = with_file_mut(ctx.args[0], |fh| std::path::Path::new(&fh.path).exists());
  Ok(Value::bool(exists))
}

fn close(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  with_file_mut(ctx.args[0], |fh| fh.handle = None);
  Ok(Value::nil())
}

fn open(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  let (path, mode) = with_file_mut(ctx.args[0], |fh| (fh.path.clone(), fh.mode.clone()));
  let handle = open_with_mode(&path, &mode)?;
  with_file_mut(ctx.args[0], |fh| fh.handle = Some(handle));
  Ok(ctx.args[0])
}

/// Shared body for `read`/`gets`; `close_after_full` is the only
/// difference: `read()` auto-closes after reading to EOF with no
/// length given; `gets()` never opens or closes automatically.
fn do_read(ctx: &mut ZuriContext, auto_close: bool) -> Result<Value, String> {
  enforce_method_arg_range!(ctx, 0, 1);
  enforce_method_arg_type_opt!(ctx, 1, ArgType::Number);

  let length = ctx.args.get(1).map(|v| v.as_number() as usize);
  let binary = ctx.args[0].as_file_cell().borrow().binary;

  let mut is_stream = false;
  let mut opened_here = false;
  let bytes_read: Vec<u8> = {
    let cell = ctx.args[0].as_file_cell();
    let mut fh = cell.borrow_mut();
    is_stream = fh.is_stream;

    // Auto-open, but only when the file is not already open.
    // Reopening unconditionally would discard whatever position the
    // handle is at, so an explicit `open()`/`seek()` would be undone
    // by the very `read()` it was performed for, and reading a large
    // file a chunk at a time would return the first chunk forever.
    if auto_close && !is_stream && fh.handle.is_none() {
      fh.handle = Some(open_with_mode(&fh.path, &fh.mode)?);
      opened_here = true;
    }

    let file = fh
      .handle
      .as_mut()
      .ok_or_else(|| "cannot read: file is not open".to_string())?;

    match length {
      Some(n) => {
        let mut buf = vec![0u8; n];
        let read = file.read(&mut buf).map_err(|e| e.to_string())?;
        buf.truncate(read);
        buf
      },
      None => {
        let mut buf = Vec::new();
        file.read_to_end(&mut buf).map_err(|e| e.to_string())?;
        buf
      },
    }
  };

  // A read-everything call closes again, but only if it was the one
  // that opened the handle; closing a file the caller opened would
  // pull it out from under them.
  if length.is_none() && auto_close && !is_stream && opened_here {
    with_file_mut(ctx.args[0], |fh| fh.handle = None);
  }

  if binary {
    return Ok(ctx.vm.heap_mut().alloc_bytes(bytes_read));
  }

  // Text mode decodes strictly. The old lossy decode turned every
  // undecodable byte into U+FFFD without saying so, which silently
  // corrupted any non-UTF-8 file read through the default mode and
  // left no way to tell that from a file that genuinely contained
  // replacement characters.
  let path = ctx.args[0].as_file_cell().borrow().path.clone();
  let text = String::from_utf8(bytes_read).map_err(|e| {
    format!(
      "cannot read '{}' as text: invalid UTF-8 at byte {}; open it in binary mode ('rb') to read the raw bytes",
      path,
      e.utf8_error().valid_up_to()
    )
  })?;

  Ok(ctx.vm.heap_mut().alloc_string(text))
}

fn read(ctx: &mut ZuriContext) -> Result<Value, String> {
  do_read(ctx, true)
}

fn gets(ctx: &mut ZuriContext) -> Result<Value, String> {
  do_read(ctx, false)
}

/// Shared body for `write`/`puts`.
fn do_write(ctx: &mut ZuriContext, auto_close: bool) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  enforce_method_arg_type_any_of!(ctx, 1, [ArgType::String, ArgType::Bytes]);

  let data: Vec<u8> = if ctx.args[1].is_string() {
    ctx.args[1].as_str().as_bytes().to_vec()
  } else {
    ctx.args[1].with_bytes(|b| b.to_vec())
  };

  let mut is_stream = false;
  let mut opened_here = false;
  {
    let cell = ctx.args[0].as_file_cell();
    let mut fh = cell.borrow_mut();
    is_stream = fh.is_stream;

    // Same rule as `do_read`: an already-open handle is written to
    // where it stands, since reopening it would rewind (and, in `w`
    // mode, truncate away) everything written so far.
    if auto_close && !is_stream && fh.handle.is_none() {
      fh.handle = Some(open_with_mode(&fh.path, &fh.mode)?);
      opened_here = true;
    }

    let file = fh
      .handle
      .as_mut()
      .ok_or_else(|| "cannot write: file is not open".to_string())?;
    file.write_all(&data).map_err(|e| e.to_string())?;
    file.flush().map_err(|e| e.to_string())?;
  }

  if auto_close && !is_stream && opened_here {
    with_file_mut(ctx.args[0], |fh| fh.handle = None);
  }

  Ok(Value::bool(true))
}

fn write(ctx: &mut ZuriContext) -> Result<Value, String> {
  do_write(ctx, true)
}

fn puts(ctx: &mut ZuriContext) -> Result<Value, String> {
  do_write(ctx, false)
}

fn number(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  #[cfg(unix)]
  {
    use std::os::unix::io::AsRawFd;
    let fd = with_file_mut(ctx.args[0], |fh| fh.handle.as_ref().map(|f| f.as_raw_fd()));
    Ok(Value::number(fd.unwrap_or(-1) as f64))
  }
  #[cfg(not(unix))]
  {
    Ok(Value::number(-1.0))
  }
}

fn is_tty(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  use std::io::IsTerminal;
  let is_tty = with_file_mut(ctx.args[0], |fh| {
    fh.handle.as_ref().map(|f| f.is_terminal()).unwrap_or(false)
  });
  Ok(Value::bool(is_tty))
}

fn is_open(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  Ok(Value::bool(with_file_mut(ctx.args[0], |fh| {
    fh.handle.is_some()
  })))
}

fn is_closed(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  Ok(Value::bool(with_file_mut(ctx.args[0], |fh| {
    fh.handle.is_none()
  })))
}

fn flush(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  with_file_mut(ctx.args[0], |fh| {
    if let Some(f) = fh.handle.as_mut() {
      let _ = f.flush();
    }
  });
  Ok(Value::nil())
}

fn stats(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  let path = with_file_mut(ctx.args[0], |fh| fh.path.clone());
  let stats = stats_of(&path)?;

  let entries: Vec<(&str, Value)> = vec![
    ("is_readable", Value::bool(stats.is_readable)),
    ("is_writable", Value::bool(stats.is_writable)),
    ("is_executable", Value::bool(stats.is_executable)),
    ("is_symbolic", Value::bool(stats.is_symbolic)),
    ("size", Value::number(stats.size as f64)),
    ("mode", Value::number(stats.mode as f64)),
    ("dev", Value::number(stats.dev as f64)),
    ("ino", Value::number(stats.ino as f64)),
    ("nlink", Value::number(stats.nlink as f64)),
    ("uid", Value::number(stats.uid as f64)),
    ("gid", Value::number(stats.gid as f64)),
    ("mtime", Value::number(stats.mtime as f64)),
    ("atime", Value::number(stats.atime as f64)),
    ("ctime", Value::number(stats.ctime as f64)),
    ("blocks", Value::number(stats.blocks as f64)),
    ("blksize", Value::number(stats.blksize as f64)),
  ];

  let pairs: Vec<(Value, Value)> = entries
    .into_iter()
    .map(|(k, v)| (ctx.vm.heap_mut().alloc_string(k), v))
    .collect();

  Ok(ctx.vm.heap_mut().alloc_dict(pairs))
}

fn symlink(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  enforce_method_arg_type!(ctx, 1, ArgType::String);

  let original = with_file_mut(ctx.args[0], |fh| fh.path.clone());
  let target = ctx.args[1].as_str().to_string();

  create_symlink(&original, &target)?;
  Ok(Value::bool(true))
}

fn delete(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  let path = with_file_mut(ctx.args[0], |fh| fh.path.clone());
  with_file_mut(ctx.args[0], |fh| fh.handle = None);
  std::fs::remove_file(&path).map_err(|e| e.to_string())?;
  Ok(Value::bool(true))
}

fn rename(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  enforce_method_arg_type!(ctx, 1, ArgType::String);
  let new_name = ctx.args[1].as_str().to_string();
  if new_name.is_empty() {
    return Err("rename() new name cannot be empty".to_string());
  }
  let old_path = with_file_mut(ctx.args[0], |fh| fh.path.clone());
  std::fs::rename(&old_path, &new_name).map_err(|e| e.to_string())?;
  with_file_mut(ctx.args[0], |fh| fh.path = new_name);
  Ok(Value::bool(true))
}

fn path(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  let p = with_file_mut(ctx.args[0], |fh| fh.path.clone());
  Ok(ctx.vm.heap_mut().alloc_string(p))
}

fn abs_path(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  let p = with_file_mut(ctx.args[0], |fh| fh.path.clone());
  let abs = canonical_path(&p).map_err(|e| e.to_string())?;
  Ok(ctx.vm.heap_mut().alloc_string(abs))
}

fn copy(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  enforce_method_arg_type!(ctx, 1, ArgType::String);
  let src = with_file_mut(ctx.args[0], |fh| fh.path.clone());
  let dest = ctx.args[1].as_str().to_string();
  std::fs::copy(&src, &dest).map_err(|e| e.to_string())?;
  Ok(Value::bool(true))
}

/// Filesystem-level, so this uses a fresh handle rather than the
/// object's own `fh.handle`; it should work even while the file
/// object's own stream is currently closed.
fn truncate(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_range!(ctx, 0, 1);
  enforce_method_arg_type_opt!(ctx, 1, ArgType::Number);
  let length = ctx.args.get(1).map(|v| v.as_number() as u64).unwrap_or(0);

  let p = with_file_mut(ctx.args[0], |fh| fh.path.clone());
  let file = std::fs::OpenOptions::new()
    .write(true)
    .open(&p)
    .map_err(|e| e.to_string())?;
  file.set_len(length).map_err(|e| e.to_string())?;
  Ok(Value::bool(true))
}

fn chmod(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  enforce_method_arg_type!(ctx, 1, ArgType::Number);

  let mode = ctx.args[1].as_number() as u32;
  let path = with_file_mut(ctx.args[0], |fh| fh.path.clone());

  set_mode(&path, mode)?;
  Ok(Value::bool(true))
}

/// `-1` for either argument means "leave that timestamp unchanged",
/// per spec.
fn set_times(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 2);
  enforce_method_arg_type!(ctx, 1, ArgType::Number);
  enforce_method_arg_type!(ctx, 2, ArgType::Number);

  let atime = ctx.args[1].as_number();
  let mtime = ctx.args[2].as_number();
  let p = with_file_mut(ctx.args[0], |fh| fh.path.clone());

  let mut times = std::fs::FileTimes::new();
  if atime >= 0.0 {
    times = times.set_accessed(
      std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(atime as u64),
    );
  }
  if mtime >= 0.0 {
    times = times.set_modified(
      std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(mtime as u64),
    );
  }

  let file = std::fs::OpenOptions::new()
    .write(true)
    .open(&p)
    .map_err(|e| e.to_string())?;
  file.set_times(times).map_err(|e| e.to_string())?;
  Ok(Value::bool(true))
}

/// `seek_type` is the raw `0`/`1`/`2` (Start/Current/End) convention
///: see this file's module-level note about the not-yet-visible
/// `io` module for `io.SEEK_*`.
fn seek(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 2);
  enforce_method_arg_type!(ctx, 1, ArgType::Number);
  enforce_method_arg_type!(ctx, 2, ArgType::Number);

  let position = ctx.args[1].as_number() as i64;
  let kind = ctx.args[2].as_number() as i64;

  let from = match kind {
    0 => std::io::SeekFrom::Start(position.max(0) as u64),
    1 => std::io::SeekFrom::Current(position),
    2 => std::io::SeekFrom::End(position),
    _ => return Err(format!("'{}' unknown seek_type {}", ctx.name, kind)),
  };

  let cell = ctx.args[0].as_file_cell();
  let mut fh = cell.borrow_mut();
  let file = fh
    .handle
    .as_mut()
    .ok_or_else(|| "cannot seek: file is not open".to_string())?;
  file.seek(from).map_err(|e| e.to_string())?;
  Ok(Value::bool(true))
}

fn tell(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  let cell = ctx.args[0].as_file_cell();
  let mut fh = cell.borrow_mut();
  let file = fh
    .handle
    .as_mut()
    .ok_or_else(|| "cannot tell: file is not open".to_string())?;
  let pos = file.stream_position().map_err(|e| e.to_string())?;
  Ok(Value::number(pos as f64))
}

fn mode(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  let m = with_file_mut(ctx.args[0], |fh| fh.mode.clone());
  Ok(ctx.vm.heap_mut().alloc_string(m))
}

fn name(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  let p = with_file_mut(ctx.args[0], |fh| fh.path.clone());
  let n = std::path::Path::new(&p)
    .file_name()
    .map(|s| s.to_string_lossy().into_owned())
    .unwrap_or_else(|| p.clone());
  Ok(ctx.vm.heap_mut().alloc_string(n))
}

// Platform-shaped filesystem facts
//
// Zuri's model of a file is the Unix one, because that is what the
// language exposes: `stats()` hands back a mode, `chmod()` takes one,
// and library code reads permission bits out of both. Windows keeps a
// different set of facts about a file, so the job below is to answer
// the Unix questions from the Windows answers, once, in a way every
// caller shares.
//
// The mapping belongs here and not in Zuri. `compress`, `http`, `log`,
// `wire` and `os.fs` all read a file's stats, and none of them want to
// know what OS they are on; an operation that refused to answer here
// would put that question in all five.

/// A file's metadata, in the shape `file.stats()` reports it.
///
/// Every field is present on every platform. The ones Windows has no
/// answer for are zero rather than absent, so a caller reading
/// `stats.size` or `stats.mtime` needs no platform check to do it.
pub struct Stats {
  pub is_readable: bool,
  pub is_writable: bool,
  pub is_executable: bool,
  pub is_symbolic: bool,
  pub size: u64,
  pub mode: u32,
  pub dev: u64,
  pub ino: u64,
  pub nlink: u64,
  pub uid: u32,
  pub gid: u32,
  pub mtime: i64,
  pub atime: i64,
  pub ctime: i64,
  pub blocks: u64,
  pub blksize: u64,
}

/// The file-type bits of a Unix mode, which callers do read: the ZIP
/// writer in `compress` puts the whole mode into an entry's external
/// attributes, and a Unix extractor that finds no type bits there
/// declines to restore permissions at all.
///
/// Only needed where a mode has to be assembled by hand; a real Unix
/// mode arrives with these already in it. `S_IFLNK` is deliberately
/// absent: nothing here ever resolves to a link, so no mode carries it.
#[cfg(not(unix))]
const S_IFREG: u32 = 0o100000;
#[cfg(not(unix))]
const S_IFDIR: u32 = 0o040000;

#[cfg(unix)]
pub fn stats_of(path: &str) -> Result<Stats, String> {
  use std::os::unix::fs::MetadataExt;

  let link_meta = std::fs::symlink_metadata(path).map_err(|e| e.to_string())?;
  let meta = std::fs::metadata(path).map_err(|e| e.to_string())?;
  let mode = meta.mode();

  Ok(Stats {
    is_readable: mode & 0o444 != 0,
    is_writable: mode & 0o222 != 0,
    is_executable: mode & 0o111 != 0,
    is_symbolic: link_meta.file_type().is_symlink(),
    size: meta.size(),
    mode,
    dev: meta.dev(),
    ino: meta.ino(),
    nlink: meta.nlink(),
    uid: meta.uid(),
    gid: meta.gid(),
    mtime: meta.mtime(),
    atime: meta.atime(),
    ctime: meta.ctime(),
    blocks: meta.blocks(),
    blksize: meta.blksize(),
  })
}

/// Windows records a file's times as 100-nanosecond ticks since the
/// start of 1601. Everything above this layer speaks Unix seconds.
#[cfg(windows)]
fn filetime_to_unix(ticks: u64) -> i64 {
  /// Seconds between 1601-01-01 and 1970-01-01.
  const EPOCH_DIFFERENCE: i64 = 11_644_473_600;

  (ticks / 10_000_000) as i64 - EPOCH_DIFFERENCE
}

/// Whether Windows would run this path as a program.
///
/// There is no execute permission to read, so the question is decided
/// the way the shell decides it: by extension, against `PATHEXT`. The
/// machine's own setting is what answers it, rather than a list kept
/// here, because that variable is precisely the list of extensions the
/// shell will run and it is the user's to change.
///
/// Worth knowing what this excludes. An `.msi` launches when opened,
/// but it is a package that `msiexec` reads rather than a program the
/// shell runs, so it is absent from `PATHEXT` and reads as
/// non-executable here. The same goes for `.ps1`: PowerShell runs one,
/// `cmd` does not. Both become executable the moment a machine adds
/// them to its own `PATHEXT`, which is the point of reading it.
#[cfg(windows)]
fn is_executable_name(path: &str) -> bool {
  /// What `cmd` falls back to with no `PATHEXT` set, which is a
  /// shorter list than the value Windows normally puts in the
  /// environment. The longer one is irrelevant here: if the variable
  /// exists at all, it is read instead of this.
  const DEFAULT_PATHEXT: &str = ".COM;.EXE;.BAT;.CMD";

  let Some(extension) = std::path::Path::new(path)
    .extension()
    .and_then(|e| e.to_str())
  else {
    return false;
  };

  std::env::var("PATHEXT")
    .unwrap_or_else(|_| DEFAULT_PATHEXT.to_string())
    .split(';')
    .any(|listed| {
      listed
        .trim()
        .trim_start_matches('.')
        .eq_ignore_ascii_case(extension)
    })
}

#[cfg(windows)]
pub fn stats_of(path: &str) -> Result<Stats, String> {
  use std::os::windows::fs::MetadataExt;

  let link_meta = std::fs::symlink_metadata(path).map_err(|e| e.to_string())?;
  let meta = std::fs::metadata(path).map_err(|e| e.to_string())?;

  let is_symbolic = link_meta.file_type().is_symlink();
  let writable = !meta.permissions().readonly();
  let executable = meta.is_dir() || is_executable_name(path);

  // Windows grants read access to anything it will hand you metadata
  // for, so the only permission that really varies is write. The mode
  // is assembled from that plus the file's type, which is as close to a
  // Unix mode as the platform's own facts reach.
  let mut mode = 0o444;
  if writable {
    mode |= 0o222;
  }
  if executable {
    mode |= 0o111;
  }
  // The type bits describe what the path resolves to, not whether it
  // was reached through a link, because `metadata` followed the link to
  // get here and Unix reports it the same way: a symlink to a file
  // stats as a regular file, and `is_symbolic` alone carries the fact
  // that a link was involved. `S_IFLNK` never appears in a mode from
  // here, on either platform.
  mode |= if meta.is_dir() { S_IFDIR } else { S_IFREG };

  let size = meta.file_size();

  Ok(Stats {
    is_readable: true,
    is_writable: writable,
    is_executable: executable,
    is_symbolic,
    size,
    mode,
    // No stable way to read a volume serial or file index through
    // `Metadata`, and Windows has no owner or group in the Unix sense.
    dev: 0,
    ino: 0,
    nlink: 1,
    uid: 0,
    gid: 0,
    mtime: filetime_to_unix(meta.last_write_time()),
    atime: filetime_to_unix(meta.last_access_time()),
    // Unix `ctime` is the inode-change time and Windows records a
    // creation time instead. Creation is the nearer of the two to what
    // a caller reading `ctime` is usually after.
    ctime: filetime_to_unix(meta.creation_time()),
    // Reported the way a Unix filesystem would for a file this size,
    // rather than left at zero: callers multiply `blocks` by 512 to
    // estimate space used, and zero makes every file look empty.
    blocks: size.div_ceil(512),
    blksize: 4096,
  })
}

#[cfg(not(any(unix, windows)))]
pub fn stats_of(path: &str) -> Result<Stats, String> {
  let meta = std::fs::metadata(path).map_err(|e| e.to_string())?;
  let writable = !meta.permissions().readonly();

  let mut mode = 0o444;
  if writable {
    mode |= 0o222;
  }
  mode |= if meta.is_dir() { S_IFDIR } else { S_IFREG };

  Ok(Stats {
    is_readable: true,
    is_writable: writable,
    is_executable: meta.is_dir(),
    is_symbolic: false,
    size: meta.len(),
    mode,
    dev: 0,
    ino: 0,
    nlink: 1,
    uid: 0,
    gid: 0,
    mtime: 0,
    atime: 0,
    ctime: 0,
    blocks: meta.len().div_ceil(512),
    blksize: 4096,
  })
}

/// Canonicalises `path`, spelled the way the rest of the system spells
/// it.
///
/// Windows' canonical form carries a `\\?\` verbatim prefix that no
/// other API there produces. A path that has been through canonicalise
/// therefore stops comparing equal to the same path that has not, which
/// breaks every containment check written as a string prefix, and it
/// reaches people unaltered in error messages and stack traces. The
/// prefix is stripped back off here so nothing above this layer has to
/// know it exists. On Unix there is nothing to strip.
pub fn canonical_path(path: &str) -> std::io::Result<String> {
  let resolved = std::fs::canonicalize(path)?;

  #[cfg(windows)]
  return Ok(strip_verbatim(resolved.display().to_string()));

  #[cfg(not(windows))]
  Ok(resolved.display().to_string())
}

/// Puts a verbatim Windows path back into its ordinary spelling.
#[cfg(windows)]
fn strip_verbatim(path: String) -> String {
  // `\\?\UNC\server\share` is the verbatim spelling of
  // `\\server\share`.
  if let Some(rest) = path.strip_prefix(r"\\?\UNC\") {
    return format!(r"\\{}", rest);
  }

  // Only a plain drive path has a shorter spelling to go back to.
  // Anything else needs the prefix in order to still mean what it says.
  match path.strip_prefix(r"\\?\") {
    Some(rest) if rest.as_bytes().get(1) == Some(&b':') => rest.to_string(),
    _ => path,
  }
}

/// Applies a Unix mode to `path`.
///
/// Windows has one writable bit where Unix has nine permission bits, so
/// the owner-write bit decides it and the rest are dropped. That loses
/// information, but it is the only part of a mode the platform can
/// actually store, and refusing the call outright would break every
/// archive extractor that restores permissions as it writes.
pub fn set_mode(path: &str, mode: u32) -> Result<(), String> {
  #[cfg(unix)]
  {
    use std::os::unix::fs::PermissionsExt;

    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode))
      .map_err(|e| format!("could not chmod '{}': {}", path, e))
  }

  #[cfg(not(unix))]
  {
    let metadata =
      std::fs::metadata(path).map_err(|e| format!("could not chmod '{}': {}", path, e))?;
    let mut permissions = metadata.permissions();
    permissions.set_readonly(mode & 0o200 == 0);

    std::fs::set_permissions(path, permissions)
      .map_err(|e| format!("could not chmod '{}': {}", path, e))
  }
}

/// Creates a symbolic link at `target` pointing to `original`.
///
/// Windows needs to know at creation time whether the link stands for
/// a file or a directory, so `original` is inspected first; a link to
/// something that isn't there yet is made as a file link, which is the
/// commoner case by a distance.
pub fn create_symlink(original: &str, target: &str) -> Result<(), String> {
  #[cfg(unix)]
  {
    std::os::unix::fs::symlink(original, target).map_err(|e| e.to_string())
  }

  #[cfg(windows)]
  {
    let directory = std::fs::metadata(original)
      .map(|m| m.is_dir())
      .unwrap_or(false);

    let result = match directory {
      true => std::os::windows::fs::symlink_dir(original, target),
      false => std::os::windows::fs::symlink_file(original, target),
    };

    result.map_err(|e| {
      // Creating one is a privileged operation unless the machine is in
      // developer mode, and the bare OS message doesn't say so.
      format!(
        "could not create a symbolic link at '{}': {}. Creating symbolic links on Windows \
         requires developer mode or an elevated process",
        target, e
      )
    })
  }

  #[cfg(not(any(unix, windows)))]
  {
    let _ = (original, target);
    Err("symlink() is not supported on this platform".to_string())
  }
}
