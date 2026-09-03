//! The `Process` handle behind `os.spawn()`.
//!
//! A `std::process::Child`'s stdout/stderr are plain blocking
//! readers, which is fine for `os.exec()`'s "run it, wait, hand back
//! everything" model but not for a handle a script holds onto and
//! reads from incrementally: a script that writes a large stdin
//! payload while never reading stdout could deadlock against the
//! child's own stdout pipe filling up and blocking IT on a write. So
//! each pipe gets its own background thread draining it into a
//! shared buffer as fast as the child produces output, and
//! `read_stdout`/`read_stderr` just pull from that buffer — the
//! script's own read timing can never back-pressure the child.

use std::io::{Read, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

#[derive(Clone, Copy)]
pub enum StdioMode {
  Pipe,
  Inherit,
  Null,
}

impl StdioMode {
  pub fn parse(name: &str) -> Result<StdioMode, String> {
    match name {
      "pipe" => Ok(StdioMode::Pipe),
      "inherit" => Ok(StdioMode::Inherit),
      "null" => Ok(StdioMode::Null),
      other => Err(format!(
        "unknown stdio mode '{}' (expected 'pipe', 'inherit', or 'null')",
        other
      )),
    }
  }

  fn to_stdio(self) -> Stdio {
    match self {
      StdioMode::Pipe => Stdio::piped(),
      StdioMode::Inherit => Stdio::inherit(),
      StdioMode::Null => Stdio::null(),
    }
  }
}

pub struct SpawnOptions {
  pub cwd: Option<String>,
  pub env: Vec<(String, String)>,
  pub env_replace: bool,
  pub stdin_mode: StdioMode,
  pub stdout_mode: StdioMode,
  pub stderr_mode: StdioMode,
}

impl Default for SpawnOptions {
  fn default() -> Self {
    SpawnOptions {
      cwd: None,
      env: Vec::new(),
      env_replace: false,
      stdin_mode: StdioMode::Inherit,
      stdout_mode: StdioMode::Pipe,
      stderr_mode: StdioMode::Pipe,
    }
  }
}

struct DrainState {
  data: Vec<u8>,
  /// Set once the reader thread hits EOF or an error; distinguishes
  /// "nothing buffered yet, keep waiting" from "nothing buffered and
  /// nothing ever will be again".
  finished: bool,
}

struct Drain {
  state: Arc<(Mutex<DrainState>, Condvar)>,
  // Not read again after spawn; kept only so the JoinHandle's own
  // Drop runs as part of this struct's, rather than being detached.
  #[allow(dead_code)]
  handle: JoinHandle<()>,
}

fn spawn_drain(mut reader: impl Read + Send + 'static) -> Drain {
  let state = Arc::new((
    Mutex::new(DrainState {
      data: Vec::new(),
      finished: false,
    }),
    Condvar::new(),
  ));
  let state2 = Arc::clone(&state);

  let handle = std::thread::spawn(move || {
    let mut chunk = [0u8; 8192];
    loop {
      match reader.read(&mut chunk) {
        Ok(0) | Err(_) => break,
        Ok(n) => {
          let (lock, cvar) = &*state2;
          let mut guard = lock.lock().unwrap_or_else(|e| e.into_inner());
          guard.data.extend_from_slice(&chunk[..n]);
          cvar.notify_all();
        },
      }
    }
    let (lock, cvar) = &*state2;
    let mut guard = lock.lock().unwrap_or_else(|e| e.into_inner());
    guard.finished = true;
    cvar.notify_all();
  });

  Drain { state, handle }
}

impl Drain {
  /// Blocks until at least one byte is available or the stream has
  /// hit EOF, then drains up to `length` bytes (everything currently
  /// buffered, if `length` is `None`). Returns an empty `Vec` only
  /// once EOF has genuinely been reached with nothing left buffered.
  fn read(&self, length: Option<usize>) -> Vec<u8> {
    let (lock, cvar) = &*self.state;
    let mut guard = lock.lock().unwrap_or_else(|e| e.into_inner());
    while guard.data.is_empty() && !guard.finished {
      guard = cvar.wait(guard).unwrap_or_else(|e| e.into_inner());
    }
    let take = length.unwrap_or(guard.data.len()).min(guard.data.len());
    guard.data.drain(..take).collect()
  }
}

pub struct Process {
  child: Child,
  stdin: Option<ChildStdin>,
  stdout: Option<Drain>,
  stderr: Option<Drain>,
}

impl Process {
  pub fn spawn(program: &str, args: &[String], opts: &SpawnOptions) -> Result<Process, String> {
    let mut cmd = Command::new(program);
    cmd.args(args);

    if let Some(cwd) = &opts.cwd {
      cmd.current_dir(cwd);
    }
    if opts.env_replace {
      cmd.env_clear();
    }
    for (k, v) in &opts.env {
      cmd.env(k, v);
    }

    cmd.stdin(opts.stdin_mode.to_stdio());
    cmd.stdout(opts.stdout_mode.to_stdio());
    cmd.stderr(opts.stderr_mode.to_stdio());

    let mut child = cmd
      .spawn()
      .map_err(|e| format!("could not spawn '{}': {}", program, e))?;

    let stdin = child.stdin.take();
    let stdout = child.stdout.take().map(spawn_drain);
    let stderr = child.stderr.take().map(spawn_drain);

    Ok(Process {
      child,
      stdin,
      stdout,
      stderr,
    })
  }

  pub fn write_stdin(&mut self, data: &[u8]) -> Result<(), String> {
    match &mut self.stdin {
      Some(w) => w
        .write_all(data)
        .and_then(|_| w.flush())
        .map_err(|e| e.to_string()),
      None => Err("this process's stdin was not piped (or has already been closed)".to_string()),
    }
  }

  /// Drops the write half of stdin so the child sees EOF on it, the
  /// same way it would if the writing end of a real pipe were
  /// closed. Needed for any child that reads its own stdin until
  /// EOF: `Child`'s stdin pipe only actually closes when this
  /// `ChildStdin` value is dropped, and nothing else does that for
  /// the caller automatically.
  pub fn close_stdin(&mut self) {
    self.stdin = None;
  }

  pub fn read_stdout(&self, length: Option<usize>) -> Result<Vec<u8>, String> {
    self
      .stdout
      .as_ref()
      .map(|d| d.read(length))
      .ok_or_else(|| "this process's stdout was not piped".to_string())
  }

  pub fn read_stderr(&self, length: Option<usize>) -> Result<Vec<u8>, String> {
    self
      .stderr
      .as_ref()
      .map(|d| d.read(length))
      .ok_or_else(|| "this process's stderr was not piped".to_string())
  }

  pub fn try_wait(&mut self) -> Result<Option<i32>, String> {
    match self.child.try_wait() {
      Ok(Some(status)) => Ok(Some(status.code().unwrap_or(-1))),
      Ok(None) => Ok(None),
      Err(e) => Err(e.to_string()),
    }
  }

  /// `timeout_ms == None` waits indefinitely (like `Child::wait`);
  /// otherwise polls `try_wait` at a short interval up to the
  /// deadline. `std::process::Child` has no native timed wait, so a
  /// bounded poll loop is the standard dependency-free way to bolt
  /// one on.
  pub fn wait(&mut self, timeout_ms: Option<u64>) -> Result<Option<i32>, String> {
    let Some(ms) = timeout_ms else {
      let status = self.child.wait().map_err(|e| e.to_string())?;
      return Ok(Some(status.code().unwrap_or(-1)));
    };

    let deadline = Instant::now() + Duration::from_millis(ms);
    loop {
      if let Some(code) = self.try_wait()? {
        return Ok(Some(code));
      }
      if Instant::now() >= deadline {
        return Ok(None);
      }
      std::thread::sleep(Duration::from_millis(5));
    }
  }

  #[cfg(unix)]
  pub fn kill(&mut self, signal: Option<i32>) -> Result<(), String> {
    let sig = signal.unwrap_or(libc::SIGTERM);
    let ret = unsafe { libc::kill(self.child.id() as libc::pid_t, sig) };
    if ret != 0 {
      return Err("could not signal the process".to_string());
    }
    Ok(())
  }

  #[cfg(not(unix))]
  pub fn kill(&mut self, signal: Option<i32>) -> Result<(), String> {
    // No selective signal delivery outside Unix; any requested signal
    // number is ignored and the process is just terminated outright.
    let _ = signal;
    self.child.kill().map_err(|e| e.to_string())
  }

  pub fn pid(&self) -> u32 {
    self.child.id()
  }
}
