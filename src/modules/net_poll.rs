//! Readiness polling over a set of sockets.
//!
//! # Why this exists
//!
//! Every other read/write path in `net` blocks: a call waits until its
//! own socket is ready, and the thread does nothing else meanwhile.
//! That is the right shape for a client and the wrong one for a
//! server, where a thread that commits to one connection cannot serve
//! another until that connection releases it. A server built only from
//! blocking calls holds exactly as many concurrent connections as it
//! has threads.
//!
//! Polling breaks that: given a set of sockets it answers "which of
//! these can be read or written right now, without blocking", so one
//! thread can own hundreds of connections and service whichever ones
//! actually have work.
//!
//! # This layer holds no state, on purpose
//!
//! The obvious design is a poller object that remembers a set of
//! descriptors. It is also unsound here. A `Value` cannot be cached in
//! a native payload, because a young object relocates the first time
//! it survives a minor collection; and a raw descriptor cannot be
//! cached either, because the socket it came from may be closed (or
//! moved to another isolate, which invalidates the sending side's
//! handle) while the cached number stays behind. The operating system
//! then reuses that number for an unrelated file, and the poller
//! silently reports readiness for something else entirely.
//!
//! So nothing is remembered here. `wait` is handed the live socket
//! handles on every call, resolves each to a descriptor, uses them
//! within that one call, and forgets them. A handle that has been
//! closed or moved cannot resolve, and is reported as an error against
//! its own index rather than polled. The registry - which socket has
//! which token, and what it is waited on for - lives in `net.Poller`,
//! in Zuri, where the collector can see it.
//!
//! # Isolates
//!
//! Nothing here crosses an isolate boundary: the sockets are the
//! calling isolate's own, and the results are plain numbers. Several
//! worker isolates each keep their own registry over their own
//! connections, which is exactly the shared-nothing model.
//!
//! # Implementation
//!
//! `poll(2)` on Unix and `WSAPoll` on Windows, which take the same
//! descriptor-set shape. Both are O(n) per call in the set size, which
//! is adequate at the scale one isolate can serve; nothing in the Zuri
//! API describes how readiness is discovered, so an `epoll` or
//! `kqueue` backend could replace this without changing a line of it.

use crate::builtins::enforce::ArgType;
use crate::modules::{BuiltinModuleDef, native};
use crate::{enforce_arg_count, enforce_arg_type};
use crate::vm::object::ZuriContext;
use crate::vm::value::Value;
use crate::vm::vm::VM;

pub static MODULE: BuiltinModuleDef = BuiltinModuleDef {
  name: "_net_poll",
  build,
};

fn build(vm: &mut VM) -> Vec<(&'static str, Value)> {
  vec![("poll_wait", native(vm, "wait", 3, false, poll_wait))]
}

/// Interest and readiness bits as Zuri sees them. Deliberately not the
/// platform's own `POLLIN`/`POLLOUT` values, which differ between
/// platforms; a Zuri program should never see a number whose meaning
/// depends on where it is running.
const READABLE: u8 = 1;
const WRITABLE: u8 = 2;
const ERROR: u8 = 4;
const HANGUP: u8 = 8;

#[cfg(unix)]
type Descriptor = std::os::unix::io::RawFd;
#[cfg(windows)]
type Descriptor = std::os::windows::io::RawSocket;

/// The descriptor behind whichever kind of socket handle this is, or
/// `None` when the handle cannot supply one - it was closed, consumed
/// by a TLS upgrade, or moved to another isolate. Both a `TcpStream`
/// and a `TlsStream` wrapping one are pollable; for TLS this is the
/// socket underneath, which is what readiness actually describes.
fn descriptor_of(value: Value) -> Option<Descriptor> {
  if let Some(fd) = crate::modules::net_tcp::descriptor_of(value) {
    return Some(fd);
  }
  crate::modules::net_tls::descriptor_of(value)
}

fn poll_wait(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 3);
  enforce_arg_type!(ctx, 0, ArgType::List);
  enforce_arg_type!(ctx, 1, ArgType::List);
  enforce_arg_type!(ctx, 2, ArgType::Number);

  let timeout = ctx.args[2].as_number();

  let sockets: Vec<Value> = ctx.args[0].as_list().to_vec();
  let interests: Vec<Value> = ctx.args[1].as_list().to_vec();

  if sockets.len() != interests.len() {
    return Err("poll: wait() needs one interest per socket".to_string());
  }

  // Resolved fresh every call: see this module's own docs on why
  // nothing here is remembered between calls.
  let mut descriptors: Vec<PlatformPollFd> = Vec::with_capacity(sockets.len());
  let mut origin: Vec<usize> = Vec::with_capacity(sockets.len());
  let mut unusable: Vec<usize> = Vec::new();

  for (index, socket) in sockets.iter().enumerate() {
    match descriptor_of(*socket) {
      Some(descriptor) => {
        let interest = interests[index].as_number() as u8;
        if interest & (READABLE | WRITABLE) == 0 {
          return Err("poll: each interest must be READABLE, WRITABLE, or both".to_string());
        }
        descriptors.push(make_pollfd(descriptor, interest));
        origin.push(index);
      },
      // A handle that cannot give up a descriptor is reported against
      // its own index instead of being polled, so the caller learns to
      // drop it rather than waiting on it forever.
      None => unusable.push(index),
    }
  }

  let mut events: Vec<(usize, u8)> = unusable.into_iter().map(|i| (i, ERROR)).collect();

  if !descriptors.is_empty() {
    // Anything already known to be unusable is reported at once; there
    // is no point sleeping for a timeout when the caller has work.
    let effective = if events.is_empty() { timeout } else { 0.0 };
    let ready = platform_poll(&mut descriptors, effective)?;

    if ready > 0 {
      for (slot, entry) in descriptors.iter().enumerate() {
        let flags = translate_revents(entry.revents);
        if flags != 0 {
          events.push((origin[slot], flags));
        }
      }
    }
  }

  let mut rows = Vec::with_capacity(events.len());
  for (index, flags) in events {
    let pair = vec![Value::number(index as f64), Value::number(flags as f64)];
    rows.push(ctx.heap().alloc_list(pair));
  }

  Ok(ctx.heap().alloc_list(rows))
}

// ---------------------------------------------------------------------------
// Platform layer
// ---------------------------------------------------------------------------

#[cfg(unix)]
type PlatformPollFd = libc::pollfd;

#[cfg(unix)]
fn make_pollfd(descriptor: Descriptor, interest: u8) -> PlatformPollFd {
  let mut events: libc::c_short = 0;
  if interest & READABLE != 0 {
    events |= libc::POLLIN;
  }
  if interest & WRITABLE != 0 {
    events |= libc::POLLOUT;
  }

  libc::pollfd {
    fd: descriptor,
    events,
    revents: 0,
  }
}

#[cfg(unix)]
fn platform_poll(fds: &mut [PlatformPollFd], timeout: f64) -> Result<usize, String> {
  // A negative timeout waits indefinitely, which is what poll(2)
  // itself means by -1.
  let millis = if timeout < 0.0 { -1 } else { timeout as i32 };

  loop {
    let rc = unsafe { libc::poll(fds.as_mut_ptr(), fds.len() as libc::nfds_t, millis) };

    if rc >= 0 {
      return Ok(rc as usize);
    }

    let err = std::io::Error::last_os_error();

    // A signal arriving mid-wait is not a failure: the caller asked to
    // be told about readiness, not about signals.
    if err.kind() == std::io::ErrorKind::Interrupted {
      continue;
    }

    return Err(format!("poll: {err}"));
  }
}

#[cfg(unix)]
fn translate_revents(revents: libc::c_short) -> u8 {
  let mut flags = 0;

  if revents & libc::POLLIN != 0 {
    flags |= READABLE;
  }
  if revents & libc::POLLOUT != 0 {
    flags |= WRITABLE;
  }
  // POLLNVAL means the descriptor was closed out from under the poll.
  // Surfacing it as an error rather than ignoring it is what lets a
  // caller notice and deregister instead of spinning on a dead entry.
  if revents & (libc::POLLERR | libc::POLLNVAL) != 0 {
    flags |= ERROR;
  }
  if revents & libc::POLLHUP != 0 {
    flags |= HANGUP;
  }

  flags
}

#[cfg(windows)]
type PlatformPollFd = windows_sys::Win32::Networking::WinSock::WSAPOLLFD;

#[cfg(windows)]
fn make_pollfd(descriptor: Descriptor, interest: u8) -> PlatformPollFd {
  use windows_sys::Win32::Networking::WinSock::{POLLRDNORM, POLLWRNORM, WSAPOLLFD};

  let mut events: i16 = 0;
  if interest & READABLE != 0 {
    events |= POLLRDNORM;
  }
  if interest & WRITABLE != 0 {
    events |= POLLWRNORM;
  }

  WSAPOLLFD {
    fd: descriptor as usize,
    events,
    revents: 0,
  }
}

#[cfg(windows)]
fn platform_poll(fds: &mut [PlatformPollFd], timeout: f64) -> Result<usize, String> {
  use windows_sys::Win32::Networking::WinSock::WSAPoll;

  let millis = if timeout < 0.0 { -1 } else { timeout as i32 };
  let rc = unsafe { WSAPoll(fds.as_mut_ptr(), fds.len() as u32, millis) };

  if rc < 0 {
    return Err(format!("poll: {}", std::io::Error::last_os_error()));
  }

  Ok(rc as usize)
}

#[cfg(windows)]
fn translate_revents(revents: i16) -> u8 {
  use windows_sys::Win32::Networking::WinSock::{
    POLLERR, POLLHUP, POLLNVAL, POLLRDNORM, POLLWRNORM,
  };

  let mut flags = 0;

  if revents & POLLRDNORM != 0 {
    flags |= READABLE;
  }
  if revents & POLLWRNORM != 0 {
    flags |= WRITABLE;
  }
  if revents & (POLLERR | POLLNVAL) != 0 {
    flags |= ERROR;
  }
  if revents & POLLHUP != 0 {
    flags |= HANGUP;
  }

  flags
}
