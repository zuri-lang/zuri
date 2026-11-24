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
//! thread can own thousands of connections and service whichever ones
//! actually have work.
//!
//! # What is remembered, and what is not
//!
//! A `Value` cannot be cached in a native payload, because a young
//! object relocates the first time it survives a minor collection. So
//! the sockets stay in `net.Poller`, in Zuri, where the collector can
//! see them, and every call here is handed them.
//!
//! What is remembered is each token's descriptor, in a kernel event
//! queue, so that a wait costs time in proportion to the sockets that
//! are ready. A remembered descriptor is only safe while the socket it
//! came from still owns it. A socket stops owning it when it is
//! closed, upgraded into a `TlsStream` or moved to another isolate, and
//! the operating system can then hand the number to something else.
//! Each of those happens in a native function on the socket's own
//! thread, and each one calls `note_handle_change()`, as does anything
//! that gives a socket a new descriptor. Before a queue does anything,
//! it checks whether that has happened since it last looked, and if so
//! resolves every token's descriptor again from its socket.
//!
//! A token whose socket can no longer supply a descriptor is taken out
//! of the kernel queue and reported as an error under its own token on
//! every wait until it is removed. A kernel queue only drops a
//! descriptor by itself when the descriptor is closed, which is why one
//! that is still open, after an upgrade or a move, is taken out here.
//! A descriptor is only ever taken out by the token that put it in, so
//! a number that another token's socket has since been given is never
//! touched.
//!
//! # TLS
//!
//! A `TlsStream` can hold decrypted data that is no longer in its
//! socket, which the socket's readiness says nothing about. Every TLS
//! operation calls `note_held_plaintext()` to say whether that is so
//! afterwards, and a wait reports those streams as readable at once.
//!
//! # Isolates
//!
//! Nothing here crosses an isolate boundary: the sockets are the
//! calling isolate's own, and the results are plain numbers. Several
//! worker isolates each keep their own queue over their own
//! connections, which is exactly the shared-nothing model.
//!
//! # Implementation
//!
//! `epoll` on Linux and Android, and `kqueue` on macOS, iOS and FreeBSD.
//! Elsewhere, Windows included, there is no queue to keep, and a wait
//! hands every remembered descriptor to `poll(2)` or `WSAPoll`, which
//! costs time in proportion to all of them.

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};

use crate::builtins::enforce::ArgType;
use crate::modules::{BuiltinModuleDef, native};
use crate::vm::object::ZuriContext;
use crate::vm::value::Value;
use crate::vm::vm::VM;
use crate::{enforce_arg_count, enforce_arg_type};

pub static MODULE: BuiltinModuleDef = BuiltinModuleDef {
  name: "_net_poll",
  build,
};

fn build(vm: &mut VM) -> Vec<(&'static str, Value)> {
  vec![
    ("queue_open", native(vm, "open", 0, false, queue_open)),
    ("queue_watch", native(vm, "watch", 4, false, queue_watch)),
    ("queue_forget", native(vm, "forget", 3, false, queue_forget)),
    ("queue_clear", native(vm, "clear", 1, false, queue_clear)),
    ("queue_wait", native(vm, "wait", 3, false, queue_wait)),
  ]
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
pub(crate) type Descriptor = std::os::unix::io::RawFd;
#[cfg(windows)]
pub(crate) type Descriptor = std::os::windows::io::RawSocket;

const POLL_QUEUE: &str = "zuri::net::PollQueue";

thread_local! {
  /// Counts the times a socket handle on this thread may have stopped
  /// supplying the descriptor it did. A queue that last looked at a
  /// different count resolves its descriptors again.
  static HANDLE_CHANGES: Cell<u64> = const { Cell::new(0) };

  /// The descriptors of the TLS streams on this thread that hold
  /// decrypted data their sockets no longer show.
  static HELD_PLAINTEXT: RefCell<HashSet<Descriptor>> = RefCell::new(HashSet::new());
}

/// Notes that a socket handle on this thread may no longer supply the
/// descriptor it did: it was closed, upgraded into a `TlsStream` or
/// moved to another isolate, or it was given a new one by `connect()`
/// or `bind()`. Every queue on the thread resolves its descriptors
/// again before its next call.
pub(crate) fn note_handle_change() {
  HANDLE_CHANGES.with(|changes| changes.set(changes.get().wrapping_add(1)));
}

fn handle_changes() -> u64 {
  HANDLE_CHANGES.with(Cell::get)
}

/// Notes whether the TLS stream on `descriptor` holds decrypted data
/// that its socket no longer shows, which a wait then reports as
/// readable.
pub(crate) fn note_held_plaintext(descriptor: Descriptor, held: bool) {
  HELD_PLAINTEXT.with(|set| {
    let mut set = set.borrow_mut();

    if held {
      set.insert(descriptor);
    } else if !set.is_empty() {
      set.remove(&descriptor);
    }
  });
}

/// The descriptor behind whichever kind of socket handle this is, or
/// `None` when the handle cannot supply one - it was closed, consumed
/// by a TLS upgrade, or moved to another isolate. A `TcpStream`, a
/// `UnixStream`, and a `TlsStream` wrapping a socket are all pollable;
/// for TLS this is the socket underneath.
fn descriptor_of(value: Value) -> Option<Descriptor> {
  if let Some(fd) = crate::modules::net_tcp::descriptor_of(value) {
    return Some(fd);
  }
  if let Some(fd) = crate::modules::net_unix::descriptor_of(value) {
    return Some(fd);
  }
  crate::modules::net_tls::descriptor_of(value)
}

/// A token's place in a queue.
struct Registration {
  /// The descriptor the token is in the kernel queue under, or `None`
  /// when its socket could not supply one, which is reported as an
  /// error.
  descriptor: Option<Descriptor>,
  interest: u8,
}

/// The registrations of one `net.Poller`, and the kernel queue they
/// are kept in. Tokens are keyed by the bits of their number.
struct PollQueue {
  kernel: kernel::Queue,
  registrations: HashMap<u64, Registration>,

  /// Which token each descriptor in the kernel queue belongs to.
  owners: HashMap<Descriptor, u64>,

  /// The tokens whose sockets cannot supply a descriptor.
  broken: HashSet<u64>,

  /// `HANDLE_CHANGES` when the descriptors were last resolved.
  seen: u64,
}

fn handle_of(sockets: Value, token: u64) -> Option<Value> {
  sockets.dict_get(&Value::number(f64::from_bits(token)))
}

impl PollQueue {
  /// Resolves every token's descriptor again, when some socket handle
  /// on this thread has changed since the last time.
  fn refresh(&mut self, sockets: Value) {
    let changes = handle_changes();

    if changes == self.seen {
      return;
    }

    self.seen = changes;

    let tokens: Vec<u64> = self.registrations.keys().copied().collect();

    for token in tokens {
      let current = handle_of(sockets, token).and_then(descriptor_of);
      let Some(registration) = self.registrations.get(&token) else {
        continue;
      };

      if registration.descriptor == current {
        continue;
      }

      let interest = registration.interest;

      if let Some(old) = registration.descriptor {
        self.release(old, token, interest);
      }

      self.settle(token, current, interest);
    }
  }

  /// Puts `descriptor`, when there is one, in the kernel queue for
  /// `token`, and records where the token now stands.
  fn settle(&mut self, token: u64, descriptor: Option<Descriptor>, interest: u8) {
    let known = descriptor.is_some_and(|d| self.owners.get(&d) == Some(&token));
    let attached = descriptor.filter(|&d| self.attach(d, token, interest, known));

    if attached.is_some() {
      self.broken.remove(&token);
    } else {
      self.broken.insert(token);
    }

    self.registrations.insert(
      token,
      Registration {
        descriptor: attached,
        interest,
      },
    );
  }

  /// Puts `descriptor` in the kernel queue for `token`. Returns whether
  /// it is there, which it is not when another token already holds it
  /// or the kernel refused it.
  fn attach(&mut self, descriptor: Descriptor, token: u64, interest: u8, known: bool) -> bool {
    if let Some(&owner) = self.owners.get(&descriptor)
      && owner != token
    {
      return false;
    }

    if self
      .kernel
      .watch(descriptor, interest, token, known)
      .is_err()
    {
      return false;
    }

    self.owners.insert(descriptor, token);
    true
  }

  /// Takes `descriptor` out of the kernel queue, provided `token` is
  /// the one that put it there.
  fn release(&mut self, descriptor: Descriptor, token: u64, interest: u8) {
    if self.owners.get(&descriptor) == Some(&token) {
      self.owners.remove(&descriptor);
      self.kernel.forget(descriptor, interest);
    }
  }

  fn watch(&mut self, sockets: Value, token: u64, interest: u8) -> Result<(), String> {
    self.refresh(sockets);

    let current = handle_of(sockets, token).and_then(descriptor_of);

    if let Some(descriptor) = current
      && let Some(&owner) = self.owners.get(&descriptor)
      && owner != token
    {
      return Err("poll: this socket is already watched under another token".to_string());
    }

    if let Some(registration) = self.registrations.get(&token)
      && let Some(old) = registration.descriptor
      && Some(old) != current
    {
      let old_interest = registration.interest;
      self.release(old, token, old_interest);
    }

    self.settle(token, current, interest);
    Ok(())
  }

  fn forget(&mut self, sockets: Value, token: u64) {
    self.refresh(sockets);
    self.broken.remove(&token);

    if let Some(registration) = self.registrations.remove(&token)
      && let Some(descriptor) = registration.descriptor
    {
      self.release(descriptor, token, registration.interest);
    }
  }

  fn clear(&mut self) {
    let registrations: Vec<(u64, Registration)> = self.registrations.drain().collect();

    for (token, registration) in registrations {
      if let Some(descriptor) = registration.descriptor {
        self.release(descriptor, token, registration.interest);
      }
    }

    self.broken.clear();
  }

  /// The tokens whose TLS streams hold decrypted data, which are
  /// readable whatever their sockets say.
  fn held(&mut self, sockets: Value) -> Vec<u64> {
    let candidates: Vec<(u64, Descriptor)> = HELD_PLAINTEXT.with(|set| {
      let set = set.borrow();

      if set.is_empty() {
        return Vec::new();
      }

      set
        .iter()
        .filter_map(|descriptor| {
          let token = *self.owners.get(descriptor)?;
          let registration = self.registrations.get(&token)?;
          (registration.interest & READABLE != 0).then_some((token, *descriptor))
        })
        .collect()
    });

    let mut held = Vec::with_capacity(candidates.len());

    for (token, descriptor) in candidates {
      let holding =
        handle_of(sockets, token).is_some_and(crate::modules::net_tls::has_pending_plaintext);

      if holding {
        held.push(token);
      } else {
        note_held_plaintext(descriptor, false);
      }
    }

    held
  }

  fn wait(&mut self, sockets: Value, timeout: f64) -> Result<Vec<(u64, u8)>, String> {
    self.refresh(sockets);

    if self.registrations.is_empty() {
      return Ok(Vec::new());
    }

    let mut events: Vec<(u64, u8)> = self.broken.iter().map(|&token| (token, ERROR)).collect();
    let held = self.held(sockets);

    // Anything already known to be ready or unusable is reported at
    // once; there is no point sleeping for a timeout when the caller
    // has work.
    let effective = if events.is_empty() && held.is_empty() {
      timeout
    } else {
      0.0
    };

    let mut ready = Vec::new();
    self
      .kernel
      .wait(&self.registrations, effective, &mut ready)?;

    for (token, flags) in ready {
      // An event for a token that has since let go of its descriptor
      // is the kernel catching up, not news about the token.
      let current = self
        .registrations
        .get(&token)
        .is_some_and(|registration| registration.descriptor.is_some());

      if current {
        merge(&mut events, token, flags);
      }
    }

    for token in held {
      merge(&mut events, token, READABLE);
    }

    Ok(events)
  }
}

/// Adds `flags` to `token`'s event, so a token is reported once however
/// many of its conditions are ready.
fn merge(events: &mut Vec<(u64, u8)>, token: u64, flags: u8) {
  match events.iter_mut().find(|(other, _)| *other == token) {
    Some(event) => event.1 |= flags,
    None => events.push((token, flags)),
  }
}

fn with_queue<T>(
  ctx: &mut ZuriContext,
  f: impl FnOnce(&mut PollQueue) -> Result<T, String>,
) -> Result<T, String> {
  let mut cell = ctx.args[0].as_ptr_cell().borrow_mut();
  let queue = cell
    .downcast_mut::<PollQueue>()
    .ok_or_else(|| "poll: expected a poller".to_string())?;
  f(queue)
}

fn interest_of(value: Value) -> Result<u8, String> {
  let interest = value.as_number() as u8;

  if interest & (READABLE | WRITABLE) == 0 {
    return Err("poll: each interest must be READABLE, WRITABLE, or both".to_string());
  }

  Ok(interest)
}

fn queue_open(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 0);

  let queue = PollQueue {
    kernel: kernel::Queue::open()?,
    registrations: HashMap::new(),
    owners: HashMap::new(),
    broken: HashSet::new(),
    seen: handle_changes(),
  };

  Ok(ctx.heap().alloc_ptr(POLL_QUEUE, queue))
}

/// `watch(queue, sockets, token, interest)`: puts `sockets[token]` in
/// the queue, or changes what it is waited on for.
fn queue_watch(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 4);
  enforce_arg_type!(ctx, 0, ArgType::PtrOf(POLL_QUEUE));
  enforce_arg_type!(ctx, 1, ArgType::Dict);
  enforce_arg_type!(ctx, 2, ArgType::Number);
  enforce_arg_type!(ctx, 3, ArgType::Number);

  let sockets = ctx.args[1];
  let token = ctx.args[2].as_number().to_bits();
  let interest = interest_of(ctx.args[3])?;

  with_queue(ctx, |queue| queue.watch(sockets, token, interest))?;

  Ok(Value::nil())
}

/// `forget(queue, sockets, token)`: takes `token` out of the queue,
/// before its socket leaves `sockets`.
fn queue_forget(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 3);
  enforce_arg_type!(ctx, 0, ArgType::PtrOf(POLL_QUEUE));
  enforce_arg_type!(ctx, 1, ArgType::Dict);
  enforce_arg_type!(ctx, 2, ArgType::Number);

  let sockets = ctx.args[1];
  let token = ctx.args[2].as_number().to_bits();

  with_queue(ctx, |queue| {
    queue.forget(sockets, token);
    Ok(())
  })?;

  Ok(Value::nil())
}

fn queue_clear(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 1);
  enforce_arg_type!(ctx, 0, ArgType::PtrOf(POLL_QUEUE));

  with_queue(ctx, |queue| {
    queue.clear();
    Ok(())
  })?;

  Ok(Value::nil())
}

/// `wait(queue, sockets, timeout)`: waits up to `timeout` milliseconds,
/// a negative one meaning for good, and returns `[token, flags]` for
/// each token that is ready.
fn queue_wait(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 3);
  enforce_arg_type!(ctx, 0, ArgType::PtrOf(POLL_QUEUE));
  enforce_arg_type!(ctx, 1, ArgType::Dict);
  enforce_arg_type!(ctx, 2, ArgType::Number);

  let sockets = ctx.args[1];
  let timeout = ctx.args[2].as_number();

  let events = with_queue(ctx, |queue| queue.wait(sockets, timeout))?;

  let mut rows = Vec::with_capacity(events.len());
  for (token, flags) in events {
    let pair = vec![
      Value::number(f64::from_bits(token)),
      Value::number(flags as f64),
    ];
    rows.push(ctx.heap().alloc_list(pair));
  }

  Ok(ctx.heap().alloc_list(rows))
}

// ---------------------------------------------------------------------------
// Kernel queues
// ---------------------------------------------------------------------------

/// How many events one wait takes from the kernel. Anything beyond is
/// still ready on the next wait, since every queue here reports
/// readiness for as long as it lasts.
#[cfg(any(
  target_os = "linux",
  target_os = "android",
  target_os = "macos",
  target_os = "ios",
  target_os = "freebsd"
))]
const BATCH: usize = 256;

#[cfg(any(target_os = "linux", target_os = "android"))]
mod kernel {
  use std::collections::HashMap;
  use std::mem::MaybeUninit;
  use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};

  use super::{BATCH, Descriptor, ERROR, HANGUP, READABLE, Registration, WRITABLE};

  pub struct Queue {
    fd: OwnedFd,
  }

  fn mask(interest: u8) -> u32 {
    let mut events = 0;

    if interest & READABLE != 0 {
      events |= libc::EPOLLIN as u32;
    }
    if interest & WRITABLE != 0 {
      events |= libc::EPOLLOUT as u32;
    }

    events
  }

  impl Queue {
    pub fn open() -> Result<Queue, String> {
      let fd = unsafe { libc::epoll_create1(libc::EPOLL_CLOEXEC) };

      if fd < 0 {
        return Err(format!("poll: {}", std::io::Error::last_os_error()));
      }

      Ok(Queue {
        fd: unsafe { OwnedFd::from_raw_fd(fd) },
      })
    }

    fn control(
      &self,
      op: i32,
      descriptor: Descriptor,
      interest: u8,
      token: u64,
    ) -> std::io::Result<()> {
      let mut event = libc::epoll_event {
        events: mask(interest),
        u64: token,
      };

      let rc = unsafe { libc::epoll_ctl(self.fd.as_raw_fd(), op, descriptor, &mut event) };

      if rc < 0 {
        Err(std::io::Error::last_os_error())
      } else {
        Ok(())
      }
    }

    /// Adds `descriptor`, or changes its interest when `known` says it
    /// is already in. Either guess can be wrong, after a descriptor was
    /// closed and given out again, so each falls back on the other.
    pub fn watch(
      &mut self,
      descriptor: Descriptor,
      interest: u8,
      token: u64,
      known: bool,
    ) -> Result<(), String> {
      let (first, second, retry) = if known {
        (libc::EPOLL_CTL_MOD, libc::EPOLL_CTL_ADD, libc::ENOENT)
      } else {
        (libc::EPOLL_CTL_ADD, libc::EPOLL_CTL_MOD, libc::EEXIST)
      };

      match self.control(first, descriptor, interest, token) {
        Ok(()) => Ok(()),
        Err(error) if error.raw_os_error() == Some(retry) => self
          .control(second, descriptor, interest, token)
          .map_err(|error| format!("poll: {error}")),
        Err(error) => Err(format!("poll: {error}")),
      }
    }

    /// Takes `descriptor` out. One that was closed has gone already,
    /// so a failure here is nothing to report.
    pub fn forget(&mut self, descriptor: Descriptor, interest: u8) {
      let _ = self.control(libc::EPOLL_CTL_DEL, descriptor, interest, 0);
    }

    pub fn wait(
      &mut self,
      _registrations: &HashMap<u64, Registration>,
      timeout: f64,
      ready: &mut Vec<(u64, u8)>,
    ) -> Result<(), String> {
      let millis = if timeout < 0.0 { -1 } else { timeout as i32 };
      let mut events: [MaybeUninit<libc::epoll_event>; BATCH] =
        [const { MaybeUninit::uninit() }; BATCH];

      let count = loop {
        let rc = unsafe {
          libc::epoll_wait(
            self.fd.as_raw_fd(),
            events.as_mut_ptr().cast(),
            BATCH as i32,
            millis,
          )
        };

        if rc >= 0 {
          break rc as usize;
        }

        let error = std::io::Error::last_os_error();

        // A signal arriving mid-wait is not a failure: the caller asked
        // to be told about readiness, not about signals.
        if error.kind() != std::io::ErrorKind::Interrupted {
          return Err(format!("poll: {error}"));
        }
      };

      for event in &events[..count] {
        let event = unsafe { event.assume_init() };
        let bits = event.events;
        let mut flags = 0;

        if bits & libc::EPOLLIN as u32 != 0 {
          flags |= READABLE;
        }
        if bits & libc::EPOLLOUT as u32 != 0 {
          flags |= WRITABLE;
        }
        if bits & libc::EPOLLERR as u32 != 0 {
          flags |= ERROR;
        }
        if bits & libc::EPOLLHUP as u32 != 0 {
          flags |= HANGUP;
        }

        ready.push((event.u64, flags));
      }

      Ok(())
    }
  }
}

#[cfg(any(target_os = "macos", target_os = "ios", target_os = "freebsd"))]
mod kernel {
  use std::collections::HashMap;
  use std::mem::MaybeUninit;
  use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};

  use super::{BATCH, Descriptor, ERROR, HANGUP, READABLE, Registration, WRITABLE};

  pub struct Queue {
    fd: OwnedFd,
  }

  fn change(descriptor: Descriptor, filter: i16, flags: u16, token: u64) -> libc::kevent {
    let mut event: libc::kevent = unsafe { std::mem::zeroed() };

    event.ident = descriptor as libc::uintptr_t;
    event.filter = filter;
    // With a receipt, each change reports its own result instead of the
    // first failure failing the whole call.
    event.flags = flags | libc::EV_RECEIPT;
    event.udata = token as usize as _;

    event
  }

  impl Queue {
    pub fn open() -> Result<Queue, String> {
      let fd = unsafe { libc::kqueue() };

      if fd < 0 {
        return Err(format!("poll: {}", std::io::Error::last_os_error()));
      }

      Ok(Queue {
        fd: unsafe { OwnedFd::from_raw_fd(fd) },
      })
    }

    /// Applies a read change and a write change. Taking out a filter
    /// that is not there is not a failure.
    fn submit(&self, changes: &[libc::kevent; 2]) -> Result<(), String> {
      let mut results: [MaybeUninit<libc::kevent>; 2] = [const { MaybeUninit::uninit() }; 2];

      loop {
        let count = unsafe {
          libc::kevent(
            self.fd.as_raw_fd(),
            changes.as_ptr(),
            2,
            results.as_mut_ptr().cast(),
            2,
            std::ptr::null(),
          )
        };

        if count >= 0 {
          for result in &results[..count as usize] {
            let result = unsafe { result.assume_init_ref() };
            let errno = result.data as i32;

            if result.flags & libc::EV_ERROR != 0 && errno != 0 && errno != libc::ENOENT {
              return Err(format!(
                "poll: {}",
                std::io::Error::from_raw_os_error(errno)
              ));
            }
          }

          return Ok(());
        }

        let error = std::io::Error::last_os_error();

        if error.kind() != std::io::ErrorKind::Interrupted {
          return Err(format!("poll: {error}"));
        }
      }
    }

    pub fn watch(
      &mut self,
      descriptor: Descriptor,
      interest: u8,
      token: u64,
      _known: bool,
    ) -> Result<(), String> {
      let read = if interest & READABLE != 0 {
        libc::EV_ADD
      } else {
        libc::EV_DELETE
      };
      let write = if interest & WRITABLE != 0 {
        libc::EV_ADD
      } else {
        libc::EV_DELETE
      };

      self.submit(&[
        change(descriptor, libc::EVFILT_READ, read, token),
        change(descriptor, libc::EVFILT_WRITE, write, token),
      ])
    }

    /// Takes `descriptor` out. One that was closed has gone already,
    /// so a failure here is nothing to report.
    pub fn forget(&mut self, descriptor: Descriptor, _interest: u8) {
      let _ = self.submit(&[
        change(descriptor, libc::EVFILT_READ, libc::EV_DELETE, 0),
        change(descriptor, libc::EVFILT_WRITE, libc::EV_DELETE, 0),
      ]);
    }

    pub fn wait(
      &mut self,
      _registrations: &HashMap<u64, Registration>,
      timeout: f64,
      ready: &mut Vec<(u64, u8)>,
    ) -> Result<(), String> {
      let millis = if timeout < 0.0 { -1 } else { timeout as i64 };
      let span = libc::timespec {
        tv_sec: (millis.max(0) / 1000) as libc::time_t,
        tv_nsec: ((millis.max(0) % 1000) * 1_000_000) as libc::c_long,
      };
      let limit: *const libc::timespec = if millis < 0 { std::ptr::null() } else { &span };
      let mut events: [MaybeUninit<libc::kevent>; BATCH] = [const { MaybeUninit::uninit() }; BATCH];

      let count = loop {
        let rc = unsafe {
          libc::kevent(
            self.fd.as_raw_fd(),
            std::ptr::null(),
            0,
            events.as_mut_ptr().cast(),
            BATCH as i32,
            limit,
          )
        };

        if rc >= 0 {
          break rc as usize;
        }

        let error = std::io::Error::last_os_error();

        // A signal arriving mid-wait is not a failure: the caller asked
        // to be told about readiness, not about signals.
        if error.kind() != std::io::ErrorKind::Interrupted {
          return Err(format!("poll: {error}"));
        }
      };

      for event in &events[..count] {
        let event = unsafe { event.assume_init_ref() };
        let mut flags = 0;

        if event.filter == libc::EVFILT_READ {
          flags |= READABLE;
        }
        if event.filter == libc::EVFILT_WRITE {
          flags |= WRITABLE;
        }
        // The peer has finished. A socket error comes with it in
        // `fflags`, and is the error poll(2) would have reported.
        if event.flags & libc::EV_EOF != 0 {
          flags |= HANGUP;

          if event.fflags != 0 {
            flags |= ERROR;
          }
        }
        if event.flags & libc::EV_ERROR != 0 {
          flags |= ERROR;
        }

        ready.push((event.udata as usize as u64, flags));
      }

      Ok(())
    }
  }
}

#[cfg(not(any(
  target_os = "linux",
  target_os = "android",
  target_os = "macos",
  target_os = "ios",
  target_os = "freebsd"
)))]
mod kernel {
  use std::collections::HashMap;

  use super::{Descriptor, ERROR, HANGUP, READABLE, Registration, WRITABLE};

  /// Nothing kept in the kernel: each wait hands every descriptor to
  /// `poll(2)` or `WSAPoll`.
  pub struct Queue;

  impl Queue {
    pub fn open() -> Result<Queue, String> {
      Ok(Queue)
    }

    pub fn watch(
      &mut self,
      _descriptor: Descriptor,
      _interest: u8,
      _token: u64,
      _known: bool,
    ) -> Result<(), String> {
      Ok(())
    }

    pub fn forget(&mut self, _descriptor: Descriptor, _interest: u8) {}

    pub fn wait(
      &mut self,
      registrations: &HashMap<u64, Registration>,
      timeout: f64,
      ready: &mut Vec<(u64, u8)>,
    ) -> Result<(), String> {
      let mut descriptors = Vec::with_capacity(registrations.len());
      let mut tokens = Vec::with_capacity(registrations.len());

      for (token, registration) in registrations {
        if let Some(descriptor) = registration.descriptor {
          descriptors.push(make_pollfd(descriptor, registration.interest));
          tokens.push(*token);
        }
      }

      if descriptors.is_empty() {
        return Ok(());
      }

      if platform_poll(&mut descriptors, timeout)? > 0 {
        for (slot, entry) in descriptors.iter().enumerate() {
          let flags = translate_revents(entry.revents);
          if flags != 0 {
            ready.push((tokens[slot], flags));
          }
        }
      }

      Ok(())
    }
  }

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
}
