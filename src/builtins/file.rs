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
  enforce_method_arg_type_opt!(ctx, 1, ArgType::Int);

  let length = ctx.args.get(1).map(|v| v.as_int() as usize);
  let binary = ctx.args[0].as_file_cell().borrow().binary;

  let mut is_stream = false;
  let bytes_read: Vec<u8> = {
    let cell = ctx.args[0].as_file_cell();
    let mut fh = cell.borrow_mut();
    is_stream = fh.is_stream;

    if auto_close && !is_stream {
      fh.handle = Some(open_with_mode(&fh.path, &fh.mode)?);
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

  if length.is_none() && auto_close && !is_stream {
    with_file_mut(ctx.args[0], |fh| fh.handle = None);
  }

  if binary {
    Ok(ctx.vm.heap_mut().alloc_bytes(bytes_read))
  } else {
    let s = String::from_utf8_lossy(&bytes_read).into_owned();
    Ok(ctx.vm.heap_mut().alloc_string(s))
  }
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
    ctx.args[1].as_bytes()
  };

  let mut is_stream = false;
  {
    let cell = ctx.args[0].as_file_cell();
    let mut fh = cell.borrow_mut();
    is_stream = fh.is_stream;

    if auto_close && !is_stream {
      fh.handle = Some(open_with_mode(&fh.path, &fh.mode)?);
    }

    let file = fh
      .handle
      .as_mut()
      .ok_or_else(|| "cannot write: file is not open".to_string())?;
    file.write_all(&data).map_err(|e| e.to_string())?;
    file.flush().map_err(|e| e.to_string())?;
  }

  if auto_close && !is_stream {
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
  #[cfg(unix)]
  {
    use std::os::unix::fs::MetadataExt;
    let path = with_file_mut(ctx.args[0], |fh| fh.path.clone());
    let link_meta = std::fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
    let is_symbolic = link_meta.file_type().is_symlink();
    let meta = std::fs::metadata(&path).map_err(|e| e.to_string())?;
    let mode = meta.mode();

    let entries: Vec<(&str, Value)> = vec![
      ("is_readable", Value::bool(mode & 0o444 != 0)),
      ("is_writable", Value::bool(mode & 0o222 != 0)),
      ("is_executable", Value::bool(mode & 0o111 != 0)),
      ("is_symbolic", Value::bool(is_symbolic)),
      ("size", Value::number(meta.size() as f64)),
      ("mode", Value::number(mode as f64)),
      ("dev", Value::number(meta.dev() as f64)),
      ("ino", Value::number(meta.ino() as f64)),
      ("nlink", Value::number(meta.nlink() as f64)),
      ("uid", Value::number(meta.uid() as f64)),
      ("gid", Value::number(meta.gid() as f64)),
      ("mtime", Value::number(meta.mtime() as f64)),
      ("atime", Value::number(meta.atime() as f64)),
      ("ctime", Value::number(meta.ctime() as f64)),
      ("blocks", Value::number(meta.blocks() as f64)),
      ("blksize", Value::number(meta.blksize() as f64)),
    ];

    let pairs: Vec<(Value, Value)> = entries
      .into_iter()
      .map(|(k, v)| (ctx.vm.heap_mut().alloc_string(k), v))
      .collect();

    Ok(ctx.vm.heap_mut().alloc_dict(pairs))
  }
  #[cfg(not(unix))]
  {
    Err("stats() is only supported on Unix platforms".to_string())
  }
}

fn symlink(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  enforce_method_arg_type!(ctx, 1, ArgType::String);
  #[cfg(unix)]
  {
    let original = with_file_mut(ctx.args[0], |fh| fh.path.clone());
    let target = ctx.args[1].as_str().to_string();
    std::os::unix::fs::symlink(&original, &target).map_err(|e| e.to_string())?;
    Ok(Value::bool(true))
  }
  #[cfg(not(unix))]
  {
    Err("symlink() is only supported on Unix platforms".to_string())
  }
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
  let abs = std::fs::canonicalize(&p).map_err(|e| e.to_string())?;
  Ok(ctx.vm.heap_mut().alloc_string(abs.display().to_string()))
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
  enforce_method_arg_type_opt!(ctx, 1, ArgType::Int);
  let length = ctx.args.get(1).map(|v| v.as_int() as u64).unwrap_or(0);

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
  enforce_method_arg_type!(ctx, 1, ArgType::Int);
  #[cfg(unix)]
  {
    use std::os::unix::fs::PermissionsExt;
    let mode = ctx.args[1].as_int() as u32;
    let p = with_file_mut(ctx.args[0], |fh| fh.path.clone());
    let perms = std::fs::Permissions::from_mode(mode);
    std::fs::set_permissions(&p, perms).map_err(|e| e.to_string())?;
    Ok(Value::bool(true))
  }
  #[cfg(not(unix))]
  {
    Err("chmod() is only supported on Unix platforms".to_string())
  }
}

/// `-1` for either argument means "leave that timestamp unchanged",
/// per spec.
fn set_times(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 2);
  enforce_method_arg_type!(ctx, 1, ArgType::Int);
  enforce_method_arg_type!(ctx, 2, ArgType::Int);

  let atime = ctx.args[1].as_int();
  let mtime = ctx.args[2].as_int();
  let p = with_file_mut(ctx.args[0], |fh| fh.path.clone());

  let mut times = std::fs::FileTimes::new();
  if atime >= 0 {
    times = times.set_accessed(
      std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(atime as u64),
    );
  }
  if mtime >= 0 {
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
  enforce_method_arg_type!(ctx, 1, ArgType::Int);
  enforce_method_arg_type!(ctx, 2, ArgType::Int);

  let position = ctx.args[1].as_int();
  let kind = ctx.args[2].as_int();

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
