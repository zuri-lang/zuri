//! `io` builtin module: `stdin`/`stdout`/`stderr` file objects
//! wrapping the process's own standard streams, `readline(...)`, etc.

use std::fs::File;
use std::io::{self, Read, Write};

use crate::modules::{BuiltinModuleDef, native};
use crate::vm::object::{FileHandle, ZuriContext};
use crate::vm::value::Value;
use crate::vm::vm::VM;

pub static MODULE: BuiltinModuleDef = BuiltinModuleDef { name: "io", build };

fn build(vm: &mut VM) -> Vec<(&'static str, Value)> {
  let stdin_val = vm.heap_mut().alloc_file(std_file(0, "<stdin>", "r"));
  let stdout_val = vm.heap_mut().alloc_file(std_file(1, "<stdout>", "w"));
  let stderr_val = vm.heap_mut().alloc_file(std_file(2, "<stderr>", "w"));
  let readline_val = native(vm, "readline", 0, true, readline);

  vec![
    ("stdin", stdin_val),
    ("stdout", stdout_val),
    ("stderr", stderr_val),
    ("readline", readline_val),
  ]
}

/// Wraps standard-stream fd `fd` as a `FileHandle`. On Unix this
/// DUPLICATES the fd first, so a Zuri-side `.close()` -- or this
/// object simply being GC'd and dropped -- closes only the
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
    handle,
  }
}

#[cfg(not(unix))]
fn std_file(_fd: i32, path: &str, mode: &str) -> FileHandle {
  FileHandle {
    path: path.to_string(),
    mode: mode.to_string(),
    binary: false,
    handle: None,
  }
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
  // No portable no-echo terminal API here -- fall back to a visible
  // read rather than failing outright.
  let mut buf = String::new();
  io::stdin().read_line(&mut buf)?;
  while buf.ends_with('\n') || buf.ends_with('\r') {
    buf.pop();
  }
  Ok(buf)
}
