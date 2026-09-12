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

use std::{
  io::{Write, stderr, stdout},
  process,
  sync::atomic::{AtomicBool, Ordering},
};

#[cfg(windows)]
use windows_sys::Win32::{
  Foundation::BOOL,
  System::Console::{CTRL_BREAK_EVENT, CTRL_C_EVENT, CTRL_CLOSE_EVENT, SetConsoleCtrlHandler},
};

#[cfg(unix)]
use libc::{SIG_DFL, SIG_ERR, SIGHUP, SIGINT, SIGTERM, c_int, raise, signal};

#[cfg(windows)]
static WINDOWS_HANDLER_INSTALLED: AtomicBool = AtomicBool::new(false);

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

/// A single byte, at a fixed process-wide address, that says "one of
/// the `PENDING` flags might be set".
///
/// The interpreter can afford `any_pending()`'s four loads on every
/// instruction; compiled code cannot, and it has no way to inline a
/// four-element scan into its safepoint anyway. So it tests this one
/// byte instead, reaching it through an address baked into the machine
/// code (`pending_hint_addr`).
///
/// Deliberately only a HINT, never the authority. `PENDING` and
/// `take_pending()` remain the only things that decide whether a
/// signal really arrived, so this being spuriously set costs one
/// wasted safepoint helper call and nothing else. What it must never
/// be is spuriously CLEAR, since that is the direction that would lose
/// a signal; `unix_handler` sets it only after the per-signal flag,
/// and `take_pending` clears it only before rescanning them.
pub static PENDING_HINT: AtomicBool = AtomicBool::new(false);

/// Address of `PENDING_HINT`, for `jit::codegen::emit_safepoint` to
/// bake into compiled code as an immediate. A `static`'s address is
/// fixed for the life of the process, which is what makes baking it
/// sound.
pub fn pending_hint_addr() -> usize {
  &raw const PENDING_HINT as usize
}

/// Whether this process has ever installed a signal handler.
///
/// Read at COMPILE time by `jit::codegen`, to decide whether a loop
/// that provably cannot allocate needs a safepoint on its back edge at
/// all. A program that never calls `on_signal()` is the overwhelmingly
/// common case, and it gets exactly the code it always got: nothing on
/// that edge. Only a program that actually asks to trap signals pays
/// for the poll.
static ARMED: AtomicBool = AtomicBool::new(false);

/// Whether any signal handler has been installed yet.
#[inline]
pub fn armed() -> bool {
  ARMED.load(Ordering::Relaxed)
}

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
  // Cleared BEFORE the scan below, not after it. A signal that lands
  // while this function is running stores its own `PENDING` flag first
  // and the hint second, so clearing first means the worst case is a
  // hint left set with nothing behind it (one wasted helper call on the
  // next safepoint). Clearing after the scan would invert that into the
  // one outcome that actually matters: a set `PENDING` flag with the
  // hint cleared out from under it, which compiled code would never
  // look at again.
  PENDING_HINT.store(false, Ordering::SeqCst);

  for (idx, flag) in PENDING.iter().enumerate() {
    if flag.swap(false, Ordering::SeqCst) {
      return Some(idx);
    }
  }
  None
}

/// Does what the OS would have done for signal `idx` if no handler had
/// been registered: terminates the process.
///
/// Reached when a registered callback returns a falsy value, which is
/// how a handler says "I looked at it, but I am not taking
/// responsibility for it". Trapping a signal otherwise means owning
/// termination completely, and a handler that only logs would leave
/// Ctrl+C unable to stop anything.
///
/// On Unix this restores `SIG_DFL` and re-raises, rather than calling
/// `exit()`, so the process really does die *of the signal*: the exit
/// status is the conventional 128 + signum and the parent sees
/// `WIFSIGNALED`, exactly as it would have with no handler at all.
/// The `exit()` below it is unreachable for the terminating signals
/// this module traps, and is there for the ones a future platform
/// might not terminate on.
#[cfg_attr(not(unix), allow(unused_variables))]
pub fn perform_default_action(idx: usize) -> ! {
  // Buffered output written by the callback that just declined would
  // otherwise be lost, since neither `raise` nor `exit` unwinds.
  let _ = stdout().flush();
  let _ = stderr().flush();

  #[cfg(unix)]
  {
    if let Some(sig) = NAMES.get(idx).and_then(|n| unix_signal_number(n)) {
      unsafe {
        signal(sig, SIG_DFL);
        raise(sig);
      }
      process::exit(128 + sig);
    }
  }

  process::exit(1);
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

  // From here on, functions compiled by the JIT poll for signals on
  // back edges they would otherwise skip. Set after the platform
  // handler is really in place, so a failed install never arms
  // anything.
  ARMED.store(true, Ordering::SeqCst);
  Ok(idx)
}

#[cfg(unix)]
extern "C" fn unix_handler(sig: c_int) {
  if let Some(idx) = unix_signal_index(sig) {
    PENDING[idx].store(true, Ordering::SeqCst);
    // Strictly after the flag above: see `PENDING_HINT`'s own docs on
    // why this order is the one that cannot lose a signal.
    PENDING_HINT.store(true, Ordering::SeqCst);
  }
}

#[cfg(unix)]
fn unix_signal_index(sig: c_int) -> Option<usize> {
  match sig {
    SIGINT => Some(0),
    SIGTERM => Some(1),
    SIGHUP => Some(2),
    _ => None,
  }
}

#[cfg(unix)]
fn unix_signal_number(name: &str) -> Option<c_int> {
  match name {
    "INT" => Some(SIGINT),
    "TERM" => Some(SIGTERM),
    "HUP" => Some(SIGHUP),
    _ => None,
  }
}

#[cfg(unix)]
fn install_platform(name: &str, _idx: usize) -> Result<(), String> {
  let sig = unix_signal_number(name)
    .ok_or_else(|| format!("signal '{}' is not supported on this platform", name))?;
  let prev = unsafe { signal(sig, unix_handler as *const () as usize) };
  if prev == SIG_ERR {
    return Err(format!("could not install a handler for signal '{}'", name));
  }
  Ok(())
}

#[cfg(windows)]
fn windows_ctrl_type(name: &str) -> Option<u32> {
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
unsafe extern "system" fn windows_handler(ctrl_type: u32) -> BOOL {
  let idx = match ctrl_type {
    CTRL_C_EVENT => Some(0),
    CTRL_CLOSE_EVENT => Some(1),
    CTRL_BREAK_EVENT => Some(3),
    _ => None,
  };
  match idx {
    Some(idx) => {
      PENDING[idx].store(true, Ordering::SeqCst);
      // Strictly after the flag above, for the same reason the unix
      // handler does it in that order.
      PENDING_HINT.store(true, Ordering::SeqCst);
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
