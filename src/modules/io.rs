//! `io` builtin module: `stdin`/`stdout`/`stderr` file objects
//! wrapping the process's own standard streams, `readline(...)`, etc.

use std::fs::File;
use std::io::{self, BufRead, Read, Write};

use crate::modules::{BuiltinModuleDef, native};
use crate::vm::object::{FileHandle, ModuleNamespace, ObjModule, ZuriContext, write_barrier};
use crate::vm::value::Value;
use crate::vm::vm::VM;

pub static MODULE: BuiltinModuleDef = BuiltinModuleDef { name: "_io", build };

fn build(vm: &mut VM) -> Vec<(&'static str, Value)> {
  let stdin_val = vm.heap_mut().alloc_file(std_file(0, "<stdin>", "r"));
  let stdout_val = vm.heap_mut().alloc_file(std_file(1, "<stdout>", "w"));
  let stderr_val = vm.heap_mut().alloc_file(std_file(2, "<stderr>", "w"));

  vec![
    ("stdin", stdin_val),
    ("stdout", stdout_val),
    ("stderr", stderr_val),
    ("isrepl", Value::bool(vm.is_repl)),
    ("readline", native(vm, "readline", 0, true, readline)),
    ("getch", native(vm, "getch", 0, true, getch)),
    ("TTY", build_tty_submodule(vm)),
  ]
}

/// A nested "module" purely as a namespacing device: `libs/io/tty.zu`
/// calls these as `_io.TTY.tcgetattr(...)`, and a real (if tiny)
/// `Obj::Module` is what makes dotted-call syntax work for a group of
/// natives the same way it already does for `_io.readline(...)`
/// itself; a plain dict wouldn't (`Instr::Invoke` needs a method
/// table or a module namespace behind `.name(...)`, not a dict
/// lookup).
fn build_tty_submodule(vm: &mut VM) -> Value {
  let module_val = vm.heap_mut().alloc_module(ObjModule {
    name: "TTY".to_string(),
    path: "<builtin:_io.TTY>".to_string(),
    namespace: ModuleNamespace::new(),
    loaded: true,
  });

  let members: Vec<(&'static str, Value)> = vec![
    (
      "tcgetattr",
      native(vm, "tcgetattr", 1, true, tty::tcgetattr),
    ),
    (
      "tcsetattr",
      native(vm, "tcsetattr", 3, false, tty::tcsetattr),
    ),
    ("exit_raw", native(vm, "exit_raw", 1, false, tty::exit_raw)),
    ("getsize", native(vm, "getsize", 1, false, tty::getsize)),
    ("flush", native(vm, "flush", 1, false, tty::flush)),
  ];
  for (name, value) in members {
    module_val.as_module_mut().namespace.set(name, value);
    write_barrier(module_val.as_obj());
  }

  module_val
}

/// Wraps standard-stream fd `fd` as a `FileHandle`. On Unix this
/// DUPLICATES the fd first, so a Zuri-side `.close()`; or this
/// object simply being GC'd and dropped; closes only the
/// duplicate, never the process's real stdin/stdout/stderr.
#[cfg(unix)]
fn std_file(fd: i32, path: &str, mode: &str) -> FileHandle {
  use std::os::unix::io::FromRawFd;
  let dup_fd = unsafe { libc::dup(fd) };
  let handle = if dup_fd >= 0 {
    Some(unsafe { File::from_raw_fd(dup_fd) })
  } else {
    None
  };
  FileHandle {
    path: path.to_string(),
    mode: mode.to_string(),
    binary: false,
    is_stream: true,
    handle,
  }
}

#[cfg(not(unix))]
fn std_file(_fd: i32, path: &str, mode: &str) -> FileHandle {
  FileHandle {
    path: path.to_string(),
    mode: mode.to_string(),
    binary: false,
    is_stream: true,
    handle: None,
  }
}

fn getch(ctx: &mut ZuriContext) -> Result<Value, String> {
  let mut stdin = io::stdin().lock();
  let mut first_byte = [0u8; 1];

  let b = loop {
    if stdin.read_exact(&mut first_byte).is_err() {
      return Ok(Value::nil()); // EOF
    }

    let current_byte = first_byte[0];
    if current_byte != b'\n' && current_byte != b'\r' {
      break current_byte;
    }
  };

  // UTF-8 leading-byte pattern determines how many continuation bytes follow.
  let len = if b & 0x80 == 0 {
    1
  } else if b & 0xE0 == 0xC0 {
    2
  } else if b & 0xF0 == 0xE0 {
    3
  } else if b & 0xF8 == 0xF0 {
    4
  } else {
    return Err("Invalid UTF-8 start byte".into());
  };

  let mut buf = vec![b; 1];
  if len > 1 {
    let mut remaining = vec![0u8; len - 1];
    let x = stdin
      .read_exact(&mut remaining)
      .map_err(|_| "Failed to read character".to_string());

    if x.is_err() {
      return Err(x.unwrap_err());
    }

    buf.extend(remaining);
  }

  // Discard whatever else is buffered on the line so the next getch()
  // starts clean rather than replaying leftover input.
  let mut garbage = Vec::new();
  let _ = stdin.read_until(b'\n', &mut garbage);

  let s = std::str::from_utf8(&buf).map_err(|e| e.to_string())?;

  Ok(ctx.heap().alloc_string(String::from(s)))
}

/// `readline([message[, secure[, obscure_text]]])`.
fn readline(ctx: &mut ZuriContext) -> Result<Value, String> {
  if ctx.args.len() > 3 {
    return Err(format!(
      "readline() expects at most 3 arguments, got {}",
      ctx.args.len()
    ));
  }

  let message = match ctx.args.get(0) {
    None => None,
    Some(v) if v.is_nil() => None,
    Some(v) if v.is_string() => Some(v.as_str().to_string()),
    Some(v) => {
      return Err(format!(
        "readline() expects argument 1 to be a string, got {}",
        v.type_name()
      ));
    },
  };
  let secure = ctx.args.get(1).map(|v| !v.is_falsey()).unwrap_or(false);
  let obscure_text = match ctx.args.get(2) {
    Some(v) if v.is_string() => v.as_str().to_string(),
    Some(v) if !v.is_nil() => {
      return Err(format!(
        "readline() expects argument 3 to be a string, got {}",
        v.type_name()
      ));
    },
    _ => "*".to_string(),
  };

  if let Some(msg) = &message {
    print!("{}", msg);
    let _ = io::stdout().flush();
  }

  let line = if secure {
    read_secure_line(&obscure_text).map_err(|e| e.to_string())?
  } else {
    let mut buf = String::new();
    io::stdin().read_line(&mut buf).map_err(|e| e.to_string())?;
    while buf.ends_with('\n') || buf.ends_with('\r') {
      buf.pop();
    }
    buf
  };

  Ok(ctx.heap().alloc_string(line))
}

/// No-echo, char-at-a-time read: disables ICANON/ECHO via termios,
/// prints `obscure_text` per keystroke (honoring backspace/delete),
/// restores the terminal's original settings before returning.
#[cfg(unix)]
fn read_secure_line(obscure_text: &str) -> io::Result<String> {
  use std::os::unix::io::AsRawFd;

  let stdin = io::stdin();
  let fd = stdin.as_raw_fd();

  let mut original: libc::termios = unsafe { std::mem::zeroed() };
  let have_tty = unsafe { libc::tcgetattr(fd, &mut original) } == 0;

  if have_tty {
    let mut raw = original;
    raw.c_lflag &= !(libc::ECHO | libc::ICANON);
    unsafe { libc::tcsetattr(fd, libc::TCSANOW, &raw) };
  }

  let result = (|| -> io::Result<String> {
    let mut locked = stdin.lock();
    let mut buf: Vec<u8> = Vec::new();
    let mut byte = [0u8; 1];
    loop {
      if locked.read(&mut byte)? == 0 {
        break; // EOF
      }
      match byte[0] {
        b'\n' | b'\r' => break,
        0x7f | 0x08 => {
          if buf.pop().is_some() {
            print!("\u{8} \u{8}");
            let _ = io::stdout().flush();
          }
        },
        c => {
          buf.push(c);
          print!("{}", obscure_text);
          let _ = io::stdout().flush();
        },
      }
    }
    Ok(String::from_utf8_lossy(&buf).into_owned())
  })();

  if have_tty {
    unsafe { libc::tcsetattr(fd, libc::TCSANOW, &original) };
  }
  println!();

  result
}

#[cfg(not(unix))]
fn read_secure_line(_obscure_text: &str) -> io::Result<String> {
  // No portable no-echo terminal API here; fall back to a visible
  // read rather than failing outright.
  let mut buf = String::new();
  io::stdin().read_line(&mut buf)?;
  while buf.ends_with('\n') || buf.ends_with('\r') {
    buf.pop();
  }
  Ok(buf)
}

/// Backs `libs/io/tty.zu`'s `TTY` class: raw termios access, terminal
/// size, and a TTY-level flush, all keyed off a real file descriptor
/// (`stdin`/`stdout`/`stderr`, or any other file the caller opened
/// against a terminal device). Unix-only, matching this file's own
/// `read_secure_line` split -- there's no portable termios/ioctl
/// equivalent to fall back to on other platforms, so every function
/// here just reports "not supported" there instead of pretending to
/// work.
mod tty {
  use crate::vm::object::ZuriContext;
  use crate::vm::value::Value;
  use crate::{enforce_arg_count, enforce_arg_range};

  #[cfg(unix)]
  use std::cell::RefCell;
  #[cfg(unix)]
  use std::os::unix::io::AsRawFd;

  #[cfg(unix)]
  thread_local! {
    /// The termios each fd had the FIRST time `tcsetattr` touched it
    /// (see `tcsetattr`'s own docs), consulted by `exit_raw` to put a
    /// terminal back exactly how it found it. Keyed on the raw fd
    /// rather than the Zuri file `Value`: `set_raw()`/`exit_raw()`
    /// are always called on the very same OS descriptor (`self.std`
    /// never changes underneath a `TTY` instance), and a plain `i32`
    /// key sidesteps needing the heap object to still be alive (or
    /// even the same Value bit pattern, across a GC move) at
    /// `exit_raw` time.
    static ORIGINAL: RefCell<rustc_hash::FxHashMap<i32, libc::termios>> =
      RefCell::new(rustc_hash::FxHashMap::default());
  }

  #[cfg(unix)]
  fn fd_of(ctx: &ZuriContext, idx: usize) -> Result<i32, String> {
    let v = *ctx
      .args
      .get(idx)
      .ok_or_else(|| "expected a file argument".to_string())?;
    if !v.is_file() {
      return Err(format!("expected a file, got {}", v.type_name()));
    }
    let handle = v.as_file_cell().borrow();
    let file = handle
      .handle
      .as_ref()
      .ok_or_else(|| "file is closed".to_string())?;
    Ok(file.as_raw_fd())
  }

  #[cfg(unix)]
  pub fn tcgetattr(ctx: &mut ZuriContext) -> Result<Value, String> {
    // A second argument is accepted (`TTY.set_raw()` passes one) but
    // deliberately ignored: with a real termios read on every call,
    // there is no "normalized vs. raw" distinction left to make; the
    // one and only value this ever returns already IS the exact
    // kernel-reported state.
    enforce_arg_range!(ctx, 1, 2);
    let fd = fd_of(ctx, 0)?;

    let mut termios: libc::termios = unsafe { std::mem::zeroed() };
    if unsafe { libc::tcgetattr(fd, &mut termios) } != 0 {
      return Err(format!(
        "tcgetattr failed: {}",
        std::io::Error::last_os_error()
      ));
    }

    let ispeed = unsafe { libc::cfgetispeed(&termios) };
    let ospeed = unsafe { libc::cfgetospeed(&termios) };
    let pairs = vec![
      (Value::number(0.0), Value::number(termios.c_iflag as f64)),
      (Value::number(1.0), Value::number(termios.c_oflag as f64)),
      (Value::number(2.0), Value::number(termios.c_cflag as f64)),
      (Value::number(3.0), Value::number(termios.c_lflag as f64)),
      (Value::number(4.0), Value::number(ispeed as f64)),
      (Value::number(5.0), Value::number(ospeed as f64)),
    ];
    Ok(ctx.heap().alloc_dict(pairs))
  }

  /// Sets termios attributes on `file`, merged (not replaced) onto
  /// whatever the terminal's real current state already is: any of
  /// the six `TTY_*` keys the caller's dict leaves out simply keeps
  /// its live value, matching `TTY.set_attr()`'s own documented
  /// contract. The very first time this is called for a given fd,
  /// the pre-change state is snapshotted into `ORIGINAL` so
  /// `exit_raw` has something to restore later.
  #[cfg(unix)]
  pub fn tcsetattr(ctx: &mut ZuriContext) -> Result<Value, String> {
    enforce_arg_count!(ctx, 3);
    let fd = fd_of(ctx, 0)?;

    let option = ctx.args[1];
    if !option.is_number() {
      return Err("tcsetattr() expects a numeric option".to_string());
    }
    let opt = match option.as_number() as i64 {
      0 => libc::TCSANOW,
      1 => libc::TCSADRAIN,
      2 => libc::TCSAFLUSH,
      other => return Err(format!("invalid tcsetattr() option {other}")),
    };

    let attrs = ctx.args[2];
    if !attrs.is_dict() {
      return Err("tcsetattr() expects a dict of attributes".to_string());
    }

    let mut current: libc::termios = unsafe { std::mem::zeroed() };
    if unsafe { libc::tcgetattr(fd, &mut current) } != 0 {
      return Err(format!(
        "tcgetattr failed: {}",
        std::io::Error::last_os_error()
      ));
    }
    ORIGINAL.with(|cache| {
      cache.borrow_mut().entry(fd).or_insert(current);
    });

    let mut next = current;
    for (key, value) in attrs.as_dict() {
      if !key.is_number() || !value.is_number() {
        continue;
      }
      let raw = value.as_number() as u32;
      match key.as_number() as i64 {
        0 => next.c_iflag = raw as libc::tcflag_t,
        1 => next.c_oflag = raw as libc::tcflag_t,
        2 => next.c_cflag = raw as libc::tcflag_t,
        3 => next.c_lflag = raw as libc::tcflag_t,
        4 => unsafe {
          libc::cfsetispeed(&mut next, raw as libc::speed_t);
        },
        5 => unsafe {
          libc::cfsetospeed(&mut next, raw as libc::speed_t);
        },
        _ => {},
      }
    }

    let ok = unsafe { libc::tcsetattr(fd, opt, &next) } == 0;
    Ok(Value::bool(ok))
  }

  /// Restores whatever `tcsetattr` snapshotted for this fd before its
  /// own first change (see that function's docs); a no-op returning
  /// `false` if `tcsetattr` was never called on this fd at all, since
  /// there is nothing recorded to put back.
  #[cfg(unix)]
  pub fn exit_raw(ctx: &mut ZuriContext) -> Result<Value, String> {
    enforce_arg_count!(ctx, 1);
    let fd = fd_of(ctx, 0)?;

    let saved = ORIGINAL.with(|cache| cache.borrow_mut().remove(&fd));
    match saved {
      Some(original) => {
        let ok = unsafe { libc::tcsetattr(fd, libc::TCSAFLUSH, &original) } == 0;
        Ok(Value::bool(ok))
      },
      None => Ok(Value::bool(false)),
    }
  }

  /// `[columns, rows]` of the terminal `file` is attached to, via
  /// `TIOCGWINSZ`.
  #[cfg(unix)]
  pub fn getsize(ctx: &mut ZuriContext) -> Result<Value, String> {
    enforce_arg_count!(ctx, 1);
    let fd = fd_of(ctx, 0)?;

    let mut size: libc::winsize = unsafe { std::mem::zeroed() };
    if unsafe { libc::ioctl(fd, libc::TIOCGWINSZ, &mut size) } != 0 {
      return Err(format!(
        "failed to query terminal size: {}",
        std::io::Error::last_os_error()
      ));
    }

    let items = vec![
      Value::number(size.ws_col as f64),
      Value::number(size.ws_row as f64),
    ];
    Ok(ctx.heap().alloc_list(items))
  }

  /// Discards (rather than merely flushing) whatever input and output
  /// the terminal has pending, via `tcflush(..., TCIOFLUSH)`; the
  /// TTY-specific counterpart to an ordinary buffered-stream flush,
  /// for clearing out e.g. stray keystrokes typed ahead of a prompt.
  #[cfg(unix)]
  pub fn flush(ctx: &mut ZuriContext) -> Result<Value, String> {
    enforce_arg_count!(ctx, 1);
    let fd = fd_of(ctx, 0)?;
    let ok = unsafe { libc::tcflush(fd, libc::TCIOFLUSH) } == 0;
    Ok(Value::bool(ok))
  }

  #[cfg(not(unix))]
  pub fn tcgetattr(_ctx: &mut ZuriContext) -> Result<Value, String> {
    Err("TTY control is not supported on this platform".to_string())
  }

  #[cfg(not(unix))]
  pub fn tcsetattr(_ctx: &mut ZuriContext) -> Result<Value, String> {
    Err("TTY control is not supported on this platform".to_string())
  }

  #[cfg(not(unix))]
  pub fn exit_raw(_ctx: &mut ZuriContext) -> Result<Value, String> {
    Err("TTY control is not supported on this platform".to_string())
  }

  #[cfg(not(unix))]
  pub fn flush(_ctx: &mut ZuriContext) -> Result<Value, String> {
    Err("TTY control is not supported on this platform".to_string())
  }

  /// `[columns, rows]` of the console `file` is attached to, via the
  /// Win32 console API. Unlike raw mode / termios attributes (which
  /// have no Windows counterpart at all), console size genuinely does
  /// exist there too, so this gets its own real implementation rather
  /// than the same blanket "not supported" every other function here
  /// falls back to off-Unix.
  #[cfg(windows)]
  pub fn getsize(ctx: &mut ZuriContext) -> Result<Value, String> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::System::Console::{
      CONSOLE_SCREEN_BUFFER_INFO, GetConsoleScreenBufferInfo,
    };

    enforce_arg_count!(ctx, 1);
    let v = ctx.args[0];
    if !v.is_file() {
      return Err(format!("expected a file, got {}", v.type_name()));
    }
    let handle_ref = v.as_file_cell().borrow();
    let file = handle_ref
      .handle
      .as_ref()
      .ok_or_else(|| "file is closed".to_string())?;
    let handle = file.as_raw_handle();

    let mut info: CONSOLE_SCREEN_BUFFER_INFO = unsafe { std::mem::zeroed() };
    if unsafe { GetConsoleScreenBufferInfo(handle, &mut info) } == 0 {
      return Err("failed to query terminal size".to_string());
    }

    // The VISIBLE window, not the (usually much taller, scrollback-
    // including) screen buffer itself: `srWindow` is the same
    // Left/Top/Right/Bottom-inclusive rectangle every other terminal-
    // size library on Windows reads this same way.
    let cols = (info.srWindow.Right - info.srWindow.Left + 1) as f64;
    let rows = (info.srWindow.Bottom - info.srWindow.Top + 1) as f64;
    let items = vec![Value::number(cols), Value::number(rows)];
    Ok(ctx.heap().alloc_list(items))
  }

  #[cfg(not(any(unix, windows)))]
  pub fn getsize(_ctx: &mut ZuriContext) -> Result<Value, String> {
    Err("TTY control is not supported on this platform".to_string())
  }
}
