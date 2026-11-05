//! Carries a panic across a C library that called back into Zuri.
//!
//! A panic cannot unwind through C, so a callback stops it at the edge
//! and C sees an ordinary failure. It cannot end there: the VM the
//! callback ran on is torn, and anything else running on it would see
//! that. So the panic waits here, and carries on unwinding as soon as
//! Rust is back in charge, which ends with the VM discarded as any other
//! panic does.

use std::any::Any;
use std::cell::Cell;
use std::panic::resume_unwind;

type Payload = Box<dyn Any + Send>;

thread_local! {
  static PARKED: Cell<Option<Payload>> = const { Cell::new(None) };
}

/// Keeps `payload` until `resume`, unless a panic is already waiting.
pub fn park(payload: Payload) {
  PARKED.with(|slot| {
    let first = slot.take().unwrap_or(payload);
    slot.set(Some(first));
  });
}

/// Whether a panic is waiting on this thread, in which case no further
/// Zuri code may run until it has resumed.
pub fn parked() -> bool {
  PARKED.with(|slot| {
    let payload = slot.take();
    let parked = payload.is_some();
    slot.set(payload);
    parked
  })
}

/// Continues the panic waiting on this thread, if there is one.
pub fn resume() {
  if let Some(payload) = PARKED.with(Cell::take) {
    resume_unwind(payload);
  }
}
