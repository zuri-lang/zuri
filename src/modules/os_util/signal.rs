//! Trapping OS signals (`SIGINT`/`SIGTERM`/...) for graceful
//! shutdown.
//!
//! The real OS-level handler does the one thing that's actually safe
//! to do inside a signal handler: flip an atomic flag. Nothing
//! allocates, locks, or touches the VM/Heap there. Delivery of the
//! registered Zuri callback happens later, cooperatively, from the
//! main VM's own per-instruction safepoint in `run_until`
//! (`src/vm/vm.rs`) — the same point that already interrupts
//! bytecode dispatch to run a GC collection — so the callback always
//! runs on the VM's own owning thread, never from signal-handler or
//! background-thread context, and never touches another isolate's
//! independent VM/Heap.

use std::sync::atomic::{AtomicBool, Ordering};

/// Every signal name this module knows how to trap. Order fixes each
/// name's flag/pin-slot index everywhere else in this file (and in
/// `vm::vm::VM::ensure_signal_pins`, which reserves exactly
/// `NAMES.len()` pinned GC roots).
pub const NAMES: &[&str] = &["INT", "TERM", "HUP", "BREAK"];

pub fn index_of(name: &str) -> Option<usize> {
  NAMES.iter().position(|&n| n == name)
}

/// One flag per entry in `NAMES`; set by the real signal handler,
/// cleared by `take_pending`.
static PENDING: [AtomicBool; 4] = [
  AtomicBool::new(false),
  AtomicBool::new(false),
  AtomicBool::new(false),
  AtomicBool::new(false),
];

/// Whether the real OS handler for each entry in `NAMES` has been
/// installed yet; installed lazily, once per name, the first time a
/// script calls `on_signal()` for it.
static INSTALLED: [AtomicBool; 4] = [
  AtomicBool::new(false),
  AtomicBool::new(false),
  AtomicBool::new(false),
  AtomicBool::new(false),
];

/// Cheap fast-path check for the per-instruction safepoint: is
/// anything pending at all? Four relaxed loads, no swap, no shared
/// aggregate flag to fall out of sync with the per-signal ones.
#[inline]
pub fn any_pending() -> bool {
  PENDING.iter().any(|f| f.load(Ordering::Relaxed))
}

/// Consumes and returns the index of one pending signal, if any.
/// Call in a loop (from the safepoint) until it returns `None` to
/// drain everything that arrived since the last check.
pub fn take_pending() -> Option<usize> {
  for (idx, flag) in PENDING.iter().enumerate() {
    if flag.swap(false, Ordering::SeqCst) {
      return Some(idx);
    }
  }
  None
}

/// Installs the real OS handler for `name`, if it isn't already.
/// Returns the pin-slot index `on_signal()`'s caller should store the
/// Zuri callback at.
pub fn install(name: &str) -> Result<usize, String> {
  let idx = index_of(name).ok_or_else(|| format!("unknown signal name '{}'", name))?;
  if !INSTALLED[idx].swap(true, Ordering::SeqCst) {
    if let Err(e) = install_platform(name, idx) {
      INSTALLED[idx].store(false, Ordering::SeqCst);
      return Err(e);
    }
  }
  Ok(idx)
}

#[cfg(unix)]
extern "C" fn unix_handler(sig: libc::c_int) {
  if let Some(idx) = unix_signal_index(sig) {
    PENDING[idx].store(true, Ordering::SeqCst);
  }
}

#[cfg(unix)]
fn unix_signal_index(sig: libc::c_int) -> Option<usize> {
  match sig {
    libc::SIGINT => Some(0),
    libc::SIGTERM => Some(1),
    libc::SIGHUP => Some(2),
    _ => None,
  }
}

#[cfg(unix)]
fn unix_signal_number(name: &str) -> Option<libc::c_int> {
  match name {
    "INT" => Some(libc::SIGINT),
    "TERM" => Some(libc::SIGTERM),
    "HUP" => Some(libc::SIGHUP),
    _ => None,
  }
}

#[cfg(unix)]
fn install_platform(name: &str, _idx: usize) -> Result<(), String> {
  let sig = unix_signal_number(name)
    .ok_or_else(|| format!("signal '{}' is not supported on this platform", name))?;
  let prev = unsafe { libc::signal(sig, unix_handler as *const () as usize) };
  if prev == libc::SIG_ERR {
    return Err(format!("could not install a handler for signal '{}'", name));
  }
  Ok(())
}

#[cfg(windows)]
static WINDOWS_HANDLER_INSTALLED: AtomicBool = AtomicBool::new(false);

#[cfg(windows)]
fn windows_ctrl_type(name: &str) -> Option<u32> {
  use windows_sys::Win32::System::Console::{CTRL_BREAK_EVENT, CTRL_C_EVENT, CTRL_CLOSE_EVENT};
  match name {
    "INT" => Some(CTRL_C_EVENT),
    "BREAK" => Some(CTRL_BREAK_EVENT),
    // No exact SIGTERM analogue on Windows; a console close request
    // is the closest "please shut down gracefully" signal there is.
    "TERM" => Some(CTRL_CLOSE_EVENT),
    _ => None,
  }
}

#[cfg(windows)]
unsafe extern "system" fn windows_handler(ctrl_type: u32) -> windows_sys::Win32::Foundation::BOOL {
  use windows_sys::Win32::System::Console::{CTRL_BREAK_EVENT, CTRL_C_EVENT, CTRL_CLOSE_EVENT};
  let idx = match ctrl_type {
    CTRL_C_EVENT => Some(0),
    CTRL_CLOSE_EVENT => Some(1),
    CTRL_BREAK_EVENT => Some(3),
    _ => None,
  };
  match idx {
    Some(idx) => {
      PENDING[idx].store(true, Ordering::SeqCst);
      1
    },
    None => 0,
  }
}

#[cfg(windows)]
fn install_platform(name: &str, _idx: usize) -> Result<(), String> {
  windows_ctrl_type(name)
    .ok_or_else(|| format!("signal '{}' is not supported on this platform", name))?;
  if !WINDOWS_HANDLER_INSTALLED.swap(true, Ordering::SeqCst) {
    use windows_sys::Win32::System::Console::SetConsoleCtrlHandler;
    let ok = unsafe { SetConsoleCtrlHandler(Some(windows_handler), 1) };
    if ok == 0 {
      WINDOWS_HANDLER_INSTALLED.store(false, Ordering::SeqCst);
      return Err("could not install a console control handler".to_string());
    }
  }
  Ok(())
}

#[cfg(not(any(unix, windows)))]
fn install_platform(name: &str, _idx: usize) -> Result<(), String> {
  Err(format!(
    "on_signal() is not supported on this platform (requested '{}')",
    name
  ))
}
