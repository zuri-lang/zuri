//! `io` builtin module: `stdin`/`stdout`/`stderr` file objects
//! wrapping the process's own standard streams, `readline(...)`, etc.

#[cfg(unix)]
use std::fs::File;
use std::io::{self, BufRead, Read, Write};

use crate::modules::{BuiltinModuleDef, native};
use crate::vm::object::{FileHandle, ModuleNamespace, ObjModule, ZuriContext, write_barrier};
use crate::vm::value::Value;
use crate::vm::vm::VM;

pub static MODULE: BuiltinModuleDef = BuiltinModuleDef { name: "_io", build };

fn build(vm: &mut VM) -> Vec<(&'static str, Value)> {
  let stdin_val = vm.heap_mut().alloc_file(std_file(0, "<stdin>", "rb"));
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
    ("flags", native(vm, "flags", 0, false, tty::flags)),
  ];
  // Only Windows gets this one. A console's mode is set wholesale
  // rather than as termios flag words, so `TTY.set_raw()` cannot build
  // it out of the pieces it uses everywhere else and calls this
  // instead; on Unix it has no reason to exist.
  #[cfg(windows)]
  let members = {
    let mut members = members;
    members.push(("set_raw", native(vm, "set_raw", 1, false, tty::set_raw)));
    members
  };

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
///
/// `binary` comes off the mode string exactly as it does for a
/// user-built `file(path, mode)`, so the two agree on what `b` means.
/// stdin is opened `"rb"`: whatever is piped in is arbitrary bytes,
/// and decoding it as text before the script has said it wants text
/// can only lose information. stdout/stderr stay `"w"` because a write
/// accepts a string or a bytes either way, so the flag would not
/// change anything for them.
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
    binary: mode.to_lowercase().contains('b'),
    is_stream: true,
    handle,
  }
}

/// The Windows half, reaching the same three streams through
/// `GetStdHandle` rather than through descriptor numbers.
///
/// Duplicated for the reason the Unix side calls `dup`: the
/// `FileHandle` owns whatever it is given and closes it when dropped,
/// and closing the process's real stdout would take the stream away
/// from everything else still writing to it.
#[cfg(windows)]
fn std_file(fd: i32, path: &str, mode: &str) -> FileHandle {
  use std::os::windows::io::FromRawHandle;
  use windows_sys::Win32::Foundation::{
    DUPLICATE_SAME_ACCESS, DuplicateHandle, INVALID_HANDLE_VALUE,
  };
  use windows_sys::Win32::System::Console::{
    GetStdHandle, STD_ERROR_HANDLE, STD_INPUT_HANDLE, STD_OUTPUT_HANDLE,
  };
  use windows_sys::Win32::System::Threading::GetCurrentProcess;

  let stream = match fd {
    0 => STD_INPUT_HANDLE,
    1 => STD_OUTPUT_HANDLE,
    _ => STD_ERROR_HANDLE,
  };

  let handle = unsafe {
    let original = GetStdHandle(stream);
    // A process with no console at all, or one started with a stream
    // closed, has nothing to hand back here.
    if original.is_null() || original == INVALID_HANDLE_VALUE {
      None
    } else {
      let process = GetCurrentProcess();
      let mut copy = std::ptr::null_mut();
      let duplicated = DuplicateHandle(
        process,
        original,
        process,
        &mut copy,
        0,
        0,
        DUPLICATE_SAME_ACCESS,
      );

      match duplicated {
        0 => None,
        _ => Some(File::from_raw_handle(copy as _)),
      }
    }
  };

  FileHandle {
    path: path.to_string(),
    mode: mode.to_string(),
    binary: mode.to_lowercase().contains('b'),
    is_stream: true,
    handle,
  }
}

#[cfg(not(any(unix, windows)))]
fn std_file(_fd: i32, path: &str, mode: &str) -> FileHandle {
  FileHandle {
    path: path.to_string(),
    mode: mode.to_string(),
    binary: mode.to_lowercase().contains('b'),
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
/// `read_secure_line` split: there's no portable termios/ioctl
/// equivalent to fall back to on other platforms, so every function
/// here just reports "not supported" there instead of pretending to
/// work.
mod tty {
  use crate::enforce_arg_count;
  #[cfg(unix)]
  use crate::enforce_arg_range;
  use crate::vm::object::ZuriContext;
  use crate::vm::value::Value;

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
    let cc_items: Vec<Value> = termios
      .c_cc
      .iter()
      .map(|&b| Value::number(b as f64))
      .collect();
    let cc_list = ctx.heap().alloc_list(cc_items);
    let pairs = vec![
      (Value::number(0.0), Value::number(termios.c_iflag as f64)),
      (Value::number(1.0), Value::number(termios.c_oflag as f64)),
      (Value::number(2.0), Value::number(termios.c_cflag as f64)),
      (Value::number(3.0), Value::number(termios.c_lflag as f64)),
      (Value::number(4.0), Value::number(ispeed as f64)),
      (Value::number(5.0), Value::number(ospeed as f64)),
      (Value::number(6.0), cc_list),
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
      x if x == libc::TCSANOW as i64 => libc::TCSANOW,
      x if x == libc::TCSADRAIN as i64 => libc::TCSADRAIN,
      x if x == libc::TCSAFLUSH as i64 => libc::TCSAFLUSH,
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
      if !key.is_number() {
        continue;
      }
      let key = key.as_number() as i64;

      // key 6 (c_cc) is the one group that isn't a plain flag word:
      // it's the array of special control characters, so it takes a
      // list rather than a number.
      if key == 6 {
        if !value.is_list() {
          continue;
        }
        for (i, item) in value.as_list().iter().enumerate() {
          if i >= next.c_cc.len() {
            break;
          }
          if item.is_number() {
            next.c_cc[i] = item.as_number() as libc::cc_t;
          }
        }
        continue;
      }

      if !value.is_number() {
        continue;
      }
      let raw = value.as_number() as u32;
      match key {
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

  /// `IUTF8` isn't exported by every `libc` target this crate covers
  /// (FreeBSD/NetBSD/OpenBSD genuinely don't have the flag at all; the
  /// glibc target module in this crate version happens to omit it too,
  /// even though real glibc headers do define it). Its value is the
  /// same `0x4000` everywhere it does exist, so this fills the gap by
  /// hand rather than leaving Linux (the one platform we actually
  /// run this codebase on) without it.
  #[cfg(unix)]
  fn iutf8_value() -> libc::tcflag_t {
    if cfg!(any(
      target_os = "linux",
      target_os = "android",
      target_os = "macos",
      target_os = "ios"
    )) {
      0x00004000
    } else {
      0
    }
  }

  /// Every termios bit/index constant `libs/io/tty.zu` exposes as a
  /// `TTY.NAME` class constant, read straight out of this build's own
  /// `libc` crate rather than hand-typed in Zuri. The bit layout of
  /// `c_iflag`/`c_oflag`/`c_cflag`/`c_lflag` (and the `c_cc` control-
  /// character index table) genuinely differs across platform families:
  /// BSD/macOS and Linux/glibc disagree on most of `c_cflag`/
  /// `c_lflag` outright, and Zuri itself has no conditional
  /// compilation to express "pick the right literal for whoever's
  /// running this." Only native code, via `libc`'s own per-target cfg
  /// gates, actually knows which platform it was compiled for, so the
  /// constants have to originate here and get read into Zuri once at
  /// `TTY` class-load time instead.
  #[cfg(unix)]
  pub fn flags(ctx: &mut ZuriContext) -> Result<Value, String> {
    enforce_arg_count!(ctx, 0);

    let pairs: Vec<(&str, i64)> = vec![
      // Wire-protocol group indices `tcgetattr`/`tcsetattr` use for
      // their own dict keys, immediately above. These aren't OS
      // values at all (they're internal to Zuri `io` module), but
      // they're handed out from here too rather than duplicated as
      // separate literals on the Zuri side, so the two ends of that
      // protocol can never quietly drift apart.
      ("TTY_IFLAG", 0),
      ("TTY_OFLAG", 1),
      ("TTY_CFLAG", 2),
      ("TTY_LFLAG", 3),
      ("TTY_ISPEED", 4),
      ("TTY_OSPEED", 5),
      ("TTY_CC", 6),
      // set_attr()'s option argument
      ("TCSANOW", libc::TCSANOW as i64),
      ("TCSADRAIN", libc::TCSADRAIN as i64),
      ("TCSAFLUSH", libc::TCSAFLUSH as i64),
      // input flags
      ("IGNBRK", libc::IGNBRK as i64),
      ("BRKINT", libc::BRKINT as i64),
      ("IGNPAR", libc::IGNPAR as i64),
      ("PARMRK", libc::PARMRK as i64),
      ("INPCK", libc::INPCK as i64),
      ("ISTRIP", libc::ISTRIP as i64),
      ("INLCR", libc::INLCR as i64),
      ("IGNCR", libc::IGNCR as i64),
      ("ICRNL", libc::ICRNL as i64),
      ("IXON", libc::IXON as i64),
      ("IXOFF", libc::IXOFF as i64),
      ("IXANY", libc::IXANY as i64),
      ("IUTF8", iutf8_value() as i64),
      // output flags
      ("OPOST", libc::OPOST as i64),
      ("ONLCR", libc::ONLCR as i64),
      // control flags
      ("CSIZE", libc::CSIZE as i64),
      ("CS5", libc::CS5 as i64),
      ("CS6", libc::CS6 as i64),
      ("CS7", libc::CS7 as i64),
      ("CS8", libc::CS8 as i64),
      ("CSTOPB", libc::CSTOPB as i64),
      ("CREAD", libc::CREAD as i64),
      ("PARENB", libc::PARENB as i64),
      ("PARODD", libc::PARODD as i64),
      ("HUPCL", libc::HUPCL as i64),
      ("CLOCAL", libc::CLOCAL as i64),
      // local flags
      ("ECHOE", libc::ECHOE as i64),
      ("ECHOK", libc::ECHOK as i64),
      ("ECHO", libc::ECHO as i64),
      ("ECHONL", libc::ECHONL as i64),
      ("ISIG", libc::ISIG as i64),
      ("ICANON", libc::ICANON as i64),
      ("IEXTEN", libc::IEXTEN as i64),
      ("TOSTOP", libc::TOSTOP as i64),
      ("NOFLSH", libc::NOFLSH as i64),
      // c_cc indices
      ("VEOF", libc::VEOF as i64),
      ("VEOL", libc::VEOL as i64),
      ("VERASE", libc::VERASE as i64),
      ("VKILL", libc::VKILL as i64),
      ("VINTR", libc::VINTR as i64),
      ("VQUIT", libc::VQUIT as i64),
      ("VSUSP", libc::VSUSP as i64),
      ("VSTART", libc::VSTART as i64),
      ("VSTOP", libc::VSTOP as i64),
      ("VMIN", libc::VMIN as i64),
      ("VTIME", libc::VTIME as i64),
    ];

    let dict_pairs = pairs
      .into_iter()
      .map(|(name, value)| (ctx.heap().alloc_string(name), Value::number(value as f64)))
      .collect();
    Ok(ctx.heap().alloc_dict(dict_pairs))
  }

  #[cfg(not(unix))]
  pub fn tcgetattr(_ctx: &mut ZuriContext) -> Result<Value, String> {
    Err("TTY control is not supported on this platform".to_string())
  }

  #[cfg(not(unix))]
  pub fn tcsetattr(_ctx: &mut ZuriContext) -> Result<Value, String> {
    Err("TTY control is not supported on this platform".to_string())
  }

  #[cfg(windows)]
  thread_local! {
    /// The console mode each stream had before `set_raw` first changed
    /// it, so `exit_raw` can put it back exactly. Keyed on the raw
    /// handle for the same reason the Unix cache keys on the raw fd:
    /// it outlives any particular Zuri `Value` and survives a GC move.
    static ORIGINAL_MODE: RefCell<rustc_hash::FxHashMap<isize, u32>> =
      RefCell::new(rustc_hash::FxHashMap::default());
  }

  #[cfg(windows)]
  fn handle_of(ctx: &ZuriContext, idx: usize) -> Result<isize, String> {
    use std::os::windows::io::AsRawHandle;

    let v = *ctx
      .args
      .get(idx)
      .ok_or_else(|| "expected a file argument".to_string())?;
    if !v.is_file() {
      return Err(format!("expected a file, got {}", v.type_name()));
    }
    let cell = v.as_file_cell();
    let borrowed = cell.borrow();
    let file = borrowed
      .handle
      .as_ref()
      .ok_or_else(|| "file is closed".to_string())?;
    Ok(file.as_raw_handle() as isize)
  }

  /// Puts a Windows console into raw mode.
  ///
  /// The console equivalent of the termios dance `TTY.set_raw()` does
  /// on Unix: clearing `ENABLE_LINE_INPUT` stops the console holding
  /// input back until Return, `ENABLE_ECHO_INPUT` stops it printing
  /// what was typed, and `ENABLE_PROCESSED_INPUT` stops it turning
  /// Ctrl+C into a signal before the program ever sees the keystroke.
  /// A console has all three on when it is created, so the default
  /// state is the cooked one, exactly as on Unix.
  #[cfg(windows)]
  pub fn set_raw(ctx: &mut ZuriContext) -> Result<Value, String> {
    use windows_sys::Win32::System::Console::{
      ENABLE_ECHO_INPUT, ENABLE_LINE_INPUT, ENABLE_PROCESSED_INPUT, GetConsoleMode, SetConsoleMode,
    };

    enforce_arg_count!(ctx, 1);
    let handle = handle_of(ctx, 0)?;

    let mut mode = 0u32;
    if unsafe { GetConsoleMode(handle as _, &mut mode) } == 0 {
      return Err(format!(
        "cannot enter raw mode: this stream is not a console: {}",
        std::io::Error::last_os_error()
      ));
    }

    // Only the FIRST call records anything, so repeated `set_raw()`
    // calls still restore to the state before any of them.
    ORIGINAL_MODE.with(|cache| {
      cache.borrow_mut().entry(handle).or_insert(mode);
    });

    let raw = mode & !(ENABLE_LINE_INPUT | ENABLE_ECHO_INPUT | ENABLE_PROCESSED_INPUT);
    if unsafe { SetConsoleMode(handle as _, raw) } == 0 {
      return Err(format!(
        "cannot enter raw mode: {}",
        std::io::Error::last_os_error()
      ));
    }

    Ok(Value::bool(true))
  }

  /// Puts the console mode back the way `set_raw` found it.
  ///
  /// Returns whether there was anything to restore, and never raises,
  /// so a cleanup path can call it without knowing whether raw mode
  /// was ever entered.
  #[cfg(windows)]
  pub fn exit_raw(ctx: &mut ZuriContext) -> Result<Value, String> {
    use windows_sys::Win32::System::Console::SetConsoleMode;

    enforce_arg_count!(ctx, 1);
    let Ok(handle) = handle_of(ctx, 0) else {
      return Ok(Value::bool(false));
    };

    let saved = ORIGINAL_MODE.with(|cache| cache.borrow_mut().remove(&handle));
    match saved {
      Some(mode) => Ok(Value::bool(
        unsafe { SetConsoleMode(handle as _, mode) } != 0,
      )),
      None => Ok(Value::bool(false)),
    }
  }

  /// Reports that nothing was restored, rather than refusing. Nothing
  /// here can enter raw mode, so nothing ever needs leaving.
  #[cfg(not(any(unix, windows)))]
  pub fn exit_raw(_ctx: &mut ZuriContext) -> Result<Value, String> {
    Ok(Value::bool(false))
  }

  /// Reports that nothing was discarded, rather than refusing.
  ///
  /// Same contract as `exit_raw` above: the Unix side returns whether
  /// `tcflush` worked, `false` for a stream with no terminal behind it,
  /// and never raises. There is no terminal queue to discard here, so
  /// the answer is `false`.
  #[cfg(not(unix))]
  pub fn flush(_ctx: &mut ZuriContext) -> Result<Value, String> {
    Ok(Value::bool(false))
  }

  /// Unlike the other TTY natives, this one can't just return an
  /// error: `libs/io/tty.zu`'s `TTY.NAME` constants call this once,
  /// unconditionally, while the class itself is being defined, and
  /// that has to succeed on every platform (Windows included) simply
  /// to let `import io` finish loading, even though none of these
  /// values do anything real there (`tcgetattr`/`tcsetattr` above
  /// already refuse to run at all on non-unix, before `set_raw()`
  /// ever gets far enough to use them).
  #[cfg(not(unix))]
  pub fn flags(ctx: &mut ZuriContext) -> Result<Value, String> {
    enforce_arg_count!(ctx, 0);

    let dict_pairs = [
      // the wire-protocol group indices are real and meaningful even
      // here, so they carry their true value rather than 0
      ("TTY_IFLAG", 0i64),
      ("TTY_OFLAG", 1),
      ("TTY_CFLAG", 2),
      ("TTY_LFLAG", 3),
      ("TTY_ISPEED", 4),
      ("TTY_OSPEED", 5),
      ("TTY_CC", 6),
      // everything below is a real termios value with no non-unix
      // equivalent: tcgetattr/tcsetattr above already refuse to run
      // at all here, so these never do anything real either way
      ("TCSANOW", 0),
      ("TCSADRAIN", 0),
      ("TCSAFLUSH", 0),
      ("IGNBRK", 0),
      ("BRKINT", 0),
      ("IGNPAR", 0),
      ("PARMRK", 0),
      ("INPCK", 0),
      ("ISTRIP", 0),
      ("INLCR", 0),
      ("IGNCR", 0),
      ("ICRNL", 0),
      ("IXON", 0),
      ("IXOFF", 0),
      ("IXANY", 0),
      ("IUTF8", 0),
      ("OPOST", 0),
      ("ONLCR", 0),
      ("CSIZE", 0),
      ("CS5", 0),
      ("CS6", 0),
      ("CS7", 0),
      ("CS8", 0),
      ("CSTOPB", 0),
      ("CREAD", 0),
      ("PARENB", 0),
      ("PARODD", 0),
      ("HUPCL", 0),
      ("CLOCAL", 0),
      ("ECHOE", 0),
      ("ECHOK", 0),
      ("ECHO", 0),
      ("ECHONL", 0),
      ("ISIG", 0),
      ("ICANON", 0),
      ("IEXTEN", 0),
      ("TOSTOP", 0),
      ("NOFLSH", 0),
      ("VEOF", 0),
      ("VEOL", 0),
      ("VERASE", 0),
      ("VKILL", 0),
      ("VINTR", 0),
      ("VQUIT", 0),
      ("VSUSP", 0),
      ("VSTART", 0),
      ("VSTOP", 0),
      ("VMIN", 0),
      ("VTIME", 0),
    ]
    .into_iter()
    .map(|(name, value)| (ctx.heap().alloc_string(name), Value::number(value as f64)))
    .collect();
    Ok(ctx.heap().alloc_dict(dict_pairs))
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
