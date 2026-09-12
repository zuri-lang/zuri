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

/// The number to report for a finished child.
///
/// A process killed by a signal has no exit code of its own: on Unix
/// `ExitStatus::code()` is `None` for it, and the convention every
/// shell follows is 128 plus the signal number. Reporting `-1` instead
/// throws that away, and makes a child killed by SIGINT look identical
/// to one that could not be run at all.
///
/// It matters through a shell too, not only for a direct child.
/// `os.exec()` goes through `sh -c`, and the shells disagree: dash
/// translates a signal death into 128 + signum and exits normally,
/// while bash re-raises the signal and dies of it, so the same command
/// yields a code on Linux and nothing on macOS. Reading the signal
/// here makes both report the same thing.
pub fn exit_code_of(status: std::process::ExitStatus) -> i32 {
  if let Some(code) = status.code() {
    return code;
  }

  #[cfg(unix)]
  {
    use std::os::unix::process::ExitStatusExt;

    if let Some(signal) = status.signal() {
      return 128 + signal;
    }
  }

  -1
}

/// What `kill()` sends when the caller doesn't say. `SIGTERM`, which
/// is what the `kill(1)` command defaults to as well.
pub const DEFAULT_SIGNAL: i32 = 15;

/// The two things a signal number can mean on a platform that has no
/// signals.
///
/// Windows can do exactly two of the things `kill(2)` does: report
/// whether a process exists and can be opened, and end it. What it has
/// no counterpart for is the part that makes a signal a signal, namely
/// the target receiving a number and deciding for itself what to do
/// about it. The only delivery mechanism Windows offers,
/// `GenerateConsoleCtrlEvent`, addresses a console process group rather
/// than a process, and refuses to target a specific group with
/// `CTRL_C_EVENT` at all, so there is nothing to build a faithful
/// `kill(pid, SIGINT)` out of.
///
/// So the numbers divide into the ones that can be honoured exactly and
/// the ones that cannot be honoured at all, and the second kind raises
/// rather than quietly terminating instead. Asking to interrupt a
/// process means wanting it to get the chance to clean up; killing it
/// outright is a different outcome, not a near-enough one, and a caller
/// who would rather have the kill can ask for that by number.
#[cfg(windows)]
pub enum WindowsAction {
  /// Signal 0, the "does this exist and may I touch it" probe. Ends
  /// nothing.
  Probe,
  /// `SIGKILL` or `SIGTERM`: end the process, with the exit status a
  /// Unix caller would have seen for it.
  Terminate(u32),
}

/// Maps a Unix signal number onto what Windows can actually do about
/// it, or explains why it can't.
#[cfg(windows)]
pub fn windows_action(signal: i32) -> Result<WindowsAction, String> {
  const SIGKILL: i32 = 9;

  match signal {
    0 => Ok(WindowsAction::Probe),
    // `128 + signal` is the status a Unix shell reports for a process
    // killed by that signal, and a child that really was signalled
    // reports it here too. Using it means a script reading an exit code
    // sees one number rather than one per platform.
    SIGKILL | DEFAULT_SIGNAL => Ok(WindowsAction::Terminate(128 + signal as u32)),
    other => Err(format!(
      "signal {} cannot be delivered on Windows, which has no way to signal a process by id. \
       Only 0 (test that the process exists), 9 and 15 (terminate it) are supported here",
      other
    )),
  }
}

/// Sends `signal` to the process with id `pid`.
///
/// On Windows only two numbers mean anything, and every other one is
/// refused rather than quietly turned into a kill.
pub fn kill_pid(pid: i64, signal: i32) -> Result<(), String> {
  #[cfg(unix)]
  {
    let ret = unsafe { libc::kill(pid as libc::pid_t, signal) };
    if ret != 0 {
      return Err(format!(
        "could not signal process {}: {}",
        pid,
        std::io::Error::last_os_error()
      ));
    }
    Ok(())
  }

  #[cfg(windows)]
  {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Threading::{
      OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_TERMINATE, TerminateProcess,
    };

    let action = windows_action(signal)?;

    // Asked for no more rights than the action needs: a probe that
    // demanded `PROCESS_TERMINATE` would report "no such process" for
    // any process this one is merely not allowed to kill.
    let access = match action {
      WindowsAction::Probe => PROCESS_QUERY_LIMITED_INFORMATION,
      WindowsAction::Terminate(_) => PROCESS_TERMINATE,
    };

    unsafe {
      let handle = OpenProcess(access, 0, pid as u32);
      if handle.is_null() {
        return Err(format!(
          "could not signal process {}: {}",
          pid,
          std::io::Error::last_os_error()
        ));
      }

      // Formatted before `CloseHandle`, which would otherwise be the
      // last call to set the thread's error code.
      let result = match action {
        WindowsAction::Probe => Ok(()),
        WindowsAction::Terminate(status) if TerminateProcess(handle, status) == 0 => Err(format!(
          "could not terminate process {}: {}",
          pid,
          std::io::Error::last_os_error()
        )),
        WindowsAction::Terminate(_) => Ok(()),
      };

      CloseHandle(handle);
      result
    }
  }

  #[cfg(not(any(unix, windows)))]
  {
    let _ = (pid, signal);
    Err("kill() is not supported on this platform".to_string())
  }
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
      Ok(Some(status)) => Ok(Some(exit_code_of(status))),
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
      return Ok(Some(exit_code_of(status)));
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
    let sig = signal.unwrap_or(DEFAULT_SIGNAL);
    let ret = unsafe { libc::kill(self.child.id() as libc::pid_t, sig) };
    if ret != 0 {
      return Err(format!(
        "could not signal the process: {}",
        std::io::Error::last_os_error()
      ));
    }
    Ok(())
  }

  #[cfg(windows)]
  pub fn kill(&mut self, signal: Option<i32>) -> Result<(), String> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::System::Threading::TerminateProcess;

    match windows_action(signal.unwrap_or(DEFAULT_SIGNAL))? {
      // Holding a `Child` is itself the answer: its handle keeps the
      // process record alive even after the process exits, so there is
      // nothing to go and ask.
      WindowsAction::Probe => Ok(()),
      WindowsAction::Terminate(status) => {
        // Through the child's own handle rather than its id. A pid can
        // be reused the moment the process behind it goes away; a
        // handle we are holding cannot.
        let handle = self.child.as_raw_handle() as _;
        if unsafe { TerminateProcess(handle, status) } == 0 {
          return Err(format!(
            "could not terminate the process: {}",
            std::io::Error::last_os_error()
          ));
        }
        Ok(())
      },
    }
  }

  #[cfg(not(any(unix, windows)))]
  pub fn kill(&mut self, signal: Option<i32>) -> Result<(), String> {
    let _ = signal;
    self.child.kill().map_err(|e| e.to_string())
  }

  pub fn pid(&self) -> u32 {
    self.child.id()
  }
}
