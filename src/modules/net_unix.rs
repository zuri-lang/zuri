//! Unix domain sockets: the same stream interface as `net_tcp`, over a
//! path on the filesystem rather than an address on the network.
//!
//! A unix socket is the usual way to reach a database, a message broker
//! or anything else running on the same machine. It skips the network
//! stack entirely, and because the socket is a file, the filesystem's
//! own permissions decide who may connect, which is a stronger and
//! simpler answer than binding to loopback and hoping.
//!
//! Windows has AF_UNIX these days, but Rust's standard library exposes
//! it only on unix targets. Rather than leave the module missing there,
//! every function is present and refuses, so a program importing this
//! fails where it tries to connect, with a sentence saying why, rather
//! than at import with a missing name.

#[cfg(unix)]
use std::io::{Read, Write};
#[cfg(unix)]
use std::os::unix::net::{UnixListener, UnixStream};
#[cfg(unix)]
use std::time::Duration;

#[cfg(unix)]
use crate::builtins::enforce::{
  ArgType, enforce_method_arg_count, enforce_method_arg_type, enforce_method_arg_type_any_of,
};
#[cfg(unix)]
use crate::enforce_arg_count;
use crate::modules::{BuiltinModuleDef, native};
use crate::vm::object::ZuriContext;
use crate::vm::value::Value;
use crate::vm::vm::VM;

pub static MODULE: BuiltinModuleDef = BuiltinModuleDef {
  name: "_net_unix",
  build,
};

/// The name each export is reached by, and the method name it reports
/// when refusing. Only the platform that refuses needs the table; the
/// one that implements them registers each with its own function.
#[cfg(not(unix))]
const EXPORTS: &[(&str, &str)] = &[
  ("unix_new", "@new"),
  ("unix_pair", "pair"),
  ("unix_connect", "connect"),
  ("unix_bind", "bind"),
  ("unix_accept", "accept"),
  ("unix_peer_address", "peer_address"),
  ("unix_local_address", "local_address"),
  ("unix_shutdown", "shutdown"),
  ("unix_set_read_timeout", "set_read_timeout"),
  ("unix_set_write_timeout", "set_write_timeout"),
  ("unix_get_read_timeout", "get_read_timeout"),
  ("unix_get_write_timeout", "get_write_timeout"),
  ("unix_set_non_blocking", "set_non_blocking"),
  ("unix_read", "read"),
  ("unix_read_exact", "read_exact"),
  ("unix_read_all", "read_all"),
  ("unix_read_as_string", "read_as_string"),
  ("unix_write", "write"),
  ("unix_write_all", "write_all"),
  ("unix_flush", "flush"),
  ("unix_is_connected", "is_connected"),
  ("unix_is_bound", "is_bound"),
  ("unix_is_supported", "is_supported"),
  ("unix_close", "close"),
];

pub(crate) const UNIX_STREAM: &str = "zuri::net::UnixStream";

#[cfg(not(unix))]
fn build(vm: &mut VM) -> Vec<(&'static str, Value)> {
  EXPORTS
    .iter()
    .map(|(key, name)| {
      let function = if *key == "unix_is_supported" {
        native(vm, name, 0, true, unsupported_probe)
      } else {
        native(vm, name, 0, true, unsupported)
      };

      (*key, function)
    })
    .collect()
}

#[cfg(not(unix))]
fn unsupported(_ctx: &mut ZuriContext) -> Result<Value, String> {
  Err(String::from(
    "unix domain sockets are not available on this platform",
  ))
}

#[cfg(not(unix))]
fn unsupported_probe(_ctx: &mut ZuriContext) -> Result<Value, String> {
  Ok(Value::bool(false))
}

#[cfg(unix)]
fn build(vm: &mut VM) -> Vec<(&'static str, Value)> {
  vec![
    ("unix_new", native(vm, "@new", 0, false, unix_new)),
    ("unix_pair", native(vm, "pair", 0, false, unix_pair)),
    (
      "unix_connect",
      native(vm, "connect", 2, false, unix_connect),
    ),
    ("unix_bind", native(vm, "bind", 2, false, unix_bind)),
    ("unix_accept", native(vm, "accept", 1, false, unix_accept)),
    (
      "unix_peer_address",
      native(vm, "peer_address", 1, false, unix_peer_address),
    ),
    (
      "unix_local_address",
      native(vm, "local_address", 1, false, unix_local_address),
    ),
    (
      "unix_shutdown",
      native(vm, "shutdown", 1, true, unix_shutdown),
    ),
    (
      "unix_set_read_timeout",
      native(vm, "set_read_timeout", 2, false, unix_set_read_timeout),
    ),
    (
      "unix_set_write_timeout",
      native(vm, "set_write_timeout", 2, false, unix_set_write_timeout),
    ),
    (
      "unix_get_read_timeout",
      native(vm, "get_read_timeout", 1, false, unix_get_read_timeout),
    ),
    (
      "unix_get_write_timeout",
      native(vm, "get_write_timeout", 1, false, unix_get_write_timeout),
    ),
    (
      "unix_set_non_blocking",
      native(vm, "set_non_blocking", 2, false, unix_set_non_blocking),
    ),
    ("unix_read", native(vm, "read", 2, false, unix_read)),
    (
      "unix_read_exact",
      native(vm, "read_exact", 2, false, unix_read_exact),
    ),
    (
      "unix_read_all",
      native(vm, "read_all", 1, false, unix_read_all),
    ),
    (
      "unix_read_as_string",
      native(vm, "read_as_string", 1, false, unix_read_as_string),
    ),
    ("unix_write", native(vm, "write", 2, false, unix_write)),
    (
      "unix_write_all",
      native(vm, "write_all", 2, false, unix_write_all),
    ),
    ("unix_flush", native(vm, "flush", 1, false, unix_flush)),
    (
      "unix_is_connected",
      native(vm, "is_connected", 1, false, unix_is_connected),
    ),
    (
      "unix_is_bound",
      native(vm, "is_bound", 1, false, unix_is_bound),
    ),
    (
      "unix_is_supported",
      native(vm, "is_supported", 0, false, unix_is_supported),
    ),
    ("unix_close", native(vm, "close", 1, false, unix_close)),
  ]
}

/// Tag a handle is switched to once closed, so a later call fails
/// loudly instead of operating on a socket that is gone. Same idea as
/// `net_tcp`'s own invalidation.
#[cfg(unix)]
const UNIX_STREAM_CLOSED: &str = "zuri::net::UnixStream::__closed__";

#[cfg(unix)]
static INVALID_STREAM_ERR: &str = "Invalid stream state";

/// The descriptor behind a `UnixStream` handle, for `net.poll`.
///
/// Resolved on demand and never cached, for the reason `net_poll`'s own
/// docs give: a stored descriptor outlives the socket it names.
#[cfg(unix)]
pub(crate) fn descriptor_of(value: Value) -> Option<std::os::unix::io::RawFd> {
  use std::os::unix::io::AsRawFd;

  if !value.is_ptr_type(UNIX_STREAM) {
    return None;
  }

  let cell = value.as_ptr_cell().borrow();
  let socket = cell.downcast_ref::<ZuriUnix>()?;

  if let Some(stream) = &socket.stream {
    return Some(stream.as_raw_fd());
  }

  socket.listener.as_ref().map(|l| l.as_raw_fd())
}

#[cfg(not(unix))]
pub(crate) fn descriptor_of(_value: Value) -> Option<std::os::windows::io::RawSocket> {
  None
}

#[cfg(unix)]
pub(crate) struct ZuriUnix {
  pub stream: Option<UnixStream>,
  pub listener: Option<UnixListener>,
}

#[cfg(unix)]
impl ZuriUnix {
  fn new() -> Self {
    ZuriUnix {
      stream: None,
      listener: None,
    }
  }

  fn with_stream(stream: UnixStream) -> Self {
    ZuriUnix {
      stream: Some(stream),
      listener: None,
    }
  }

  fn stream(&self) -> Result<&UnixStream, String> {
    self.stream.as_ref().ok_or(INVALID_STREAM_ERR.to_string())
  }

  fn stream_mut(&mut self) -> Result<&mut UnixStream, String> {
    self.stream.as_mut().ok_or(INVALID_STREAM_ERR.to_string())
  }
}

#[cfg(unix)]
fn get_data(value: Value) -> Vec<u8> {
  if value.is_string() {
    value.as_str().as_bytes().to_vec()
  } else {
    value.with_bytes(|b| b.to_vec())
  }
}

/// A socket address as text. An unnamed socket, which is what both ends
/// of a `pair()` are, has no path at all and reports nil rather than an
/// invented one.
#[cfg(unix)]
fn address_text(ctx: &mut ZuriContext, address: &std::os::unix::net::SocketAddr) -> Value {
  match address.as_pathname() {
    Some(path) => ctx.heap().alloc_string(path.to_string_lossy().to_string()),
    None => Value::nil(),
  }
}

#[cfg(unix)]
fn unix_new(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 0);

  Ok(ctx.heap().alloc_ptr(UNIX_STREAM, ZuriUnix::new()))
}

/// Two connected sockets with no path, for talking between threads or
/// to a child process without putting anything on the filesystem.
#[cfg(unix)]
fn unix_pair(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 0);

  let (left, right) = UnixStream::pair().map_err(|e| e.to_string())?;
  let first = ctx
    .heap()
    .alloc_ptr(UNIX_STREAM, ZuriUnix::with_stream(left));
  let second = ctx
    .heap()
    .alloc_ptr(UNIX_STREAM, ZuriUnix::with_stream(right));

  Ok(ctx.heap().alloc_list(vec![first, second]))
}

#[cfg(unix)]
fn unix_connect(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(UNIX_STREAM));
  enforce_method_arg_type!(ctx, 1, ArgType::String);

  let path = ctx.args[1].as_str().to_string();
  let mut ptr = ctx.args[0].as_ptr_cell().borrow_mut();
  let socket = ptr.downcast_mut::<ZuriUnix>().unwrap();

  if socket.listener.is_some() {
    return Err(String::from("Cannot connect to bound socket"));
  }

  if socket.stream.is_some() {
    return Err(String::from("Socket already connected to another stream"));
  }

  socket.stream = Some(UnixStream::connect(&path).map_err(|e| e.to_string())?);

  Ok(Value::nil())
}

#[cfg(unix)]
fn unix_bind(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(UNIX_STREAM));
  enforce_method_arg_type!(ctx, 1, ArgType::String);

  let path = ctx.args[1].as_str().to_string();
  let mut ptr = ctx.args[0].as_ptr_cell().borrow_mut();
  let socket = ptr.downcast_mut::<ZuriUnix>().unwrap();

  if socket.stream.is_some() {
    return Err(String::from("Cannot bind a connected socket"));
  }

  socket.listener = Some(UnixListener::bind(&path).map_err(|e| e.to_string())?);

  Ok(Value::nil())
}

#[cfg(unix)]
fn unix_accept(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(UNIX_STREAM));

  let stream = {
    let ptr = ctx.args[0].as_ptr_cell().borrow();
    let socket = ptr.downcast_ref::<ZuriUnix>().unwrap();
    let listener = socket
      .listener
      .as_ref()
      .ok_or("Socket is not bound to a path")?;

    listener.accept().map_err(|e| e.to_string())?.0
  };

  Ok(
    ctx
      .heap()
      .alloc_ptr(UNIX_STREAM, ZuriUnix::with_stream(stream)),
  )
}

#[cfg(unix)]
fn unix_peer_address(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(UNIX_STREAM));

  let address = {
    let ptr = ctx.args[0].as_ptr_cell().borrow();
    let socket = ptr.downcast_ref::<ZuriUnix>().unwrap();

    socket.stream()?.peer_addr().map_err(|e| e.to_string())?
  };

  Ok(address_text(ctx, &address))
}

#[cfg(unix)]
fn unix_local_address(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(UNIX_STREAM));

  let address = {
    let ptr = ctx.args[0].as_ptr_cell().borrow();
    let socket = ptr.downcast_ref::<ZuriUnix>().unwrap();

    match (&socket.stream, &socket.listener) {
      (Some(stream), _) => stream.local_addr().map_err(|e| e.to_string())?,
      (None, Some(listener)) => listener.local_addr().map_err(|e| e.to_string())?,
      _ => return Err(INVALID_STREAM_ERR.to_string()),
    }
  };

  Ok(address_text(ctx, &address))
}

#[cfg(unix)]
fn unix_shutdown(ctx: &mut ZuriContext) -> Result<Value, String> {
  use std::net::Shutdown;

  enforce_method_arg_count!(ctx, 0);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(UNIX_STREAM));

  let ptr = ctx.args[0].as_ptr_cell().borrow();
  let socket = ptr.downcast_ref::<ZuriUnix>().unwrap();

  socket
    .stream()?
    .shutdown(Shutdown::Both)
    .map_err(|e| e.to_string())?;

  Ok(Value::nil())
}

#[cfg(unix)]
fn duration_arg(ctx: &ZuriContext) -> Option<Duration> {
  let millis = ctx.args[1].as_number();

  if millis <= 0.0 {
    None
  } else {
    Some(Duration::from_millis(millis as u64))
  }
}

#[cfg(unix)]
fn unix_set_read_timeout(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(UNIX_STREAM));
  enforce_method_arg_type!(ctx, 1, ArgType::Number);

  let timeout = duration_arg(ctx);
  let ptr = ctx.args[0].as_ptr_cell().borrow();
  let socket = ptr.downcast_ref::<ZuriUnix>().unwrap();

  socket
    .stream()?
    .set_read_timeout(timeout)
    .map_err(|e| e.to_string())?;

  Ok(Value::nil())
}

#[cfg(unix)]
fn unix_set_write_timeout(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(UNIX_STREAM));
  enforce_method_arg_type!(ctx, 1, ArgType::Number);

  let timeout = duration_arg(ctx);
  let ptr = ctx.args[0].as_ptr_cell().borrow();
  let socket = ptr.downcast_ref::<ZuriUnix>().unwrap();

  socket
    .stream()?
    .set_write_timeout(timeout)
    .map_err(|e| e.to_string())?;

  Ok(Value::nil())
}

#[cfg(unix)]
fn millis_of(timeout: Option<Duration>) -> Value {
  match timeout {
    Some(duration) => Value::number(duration.as_millis() as f64),
    None => Value::number(0.0),
  }
}

#[cfg(unix)]
fn unix_get_read_timeout(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(UNIX_STREAM));

  let ptr = ctx.args[0].as_ptr_cell().borrow();
  let socket = ptr.downcast_ref::<ZuriUnix>().unwrap();

  Ok(millis_of(
    socket.stream()?.read_timeout().map_err(|e| e.to_string())?,
  ))
}

#[cfg(unix)]
fn unix_get_write_timeout(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(UNIX_STREAM));

  let ptr = ctx.args[0].as_ptr_cell().borrow();
  let socket = ptr.downcast_ref::<ZuriUnix>().unwrap();

  Ok(millis_of(
    socket
      .stream()?
      .write_timeout()
      .map_err(|e| e.to_string())?,
  ))
}

#[cfg(unix)]
fn unix_set_non_blocking(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(UNIX_STREAM));
  enforce_method_arg_type!(ctx, 1, ArgType::Bool);

  let wanted = ctx.args[1].as_bool();
  let ptr = ctx.args[0].as_ptr_cell().borrow();
  let socket = ptr.downcast_ref::<ZuriUnix>().unwrap();

  match (&socket.stream, &socket.listener) {
    (Some(stream), _) => stream.set_nonblocking(wanted).map_err(|e| e.to_string())?,
    (None, Some(listener)) => listener
      .set_nonblocking(wanted)
      .map_err(|e| e.to_string())?,
    _ => return Err(INVALID_STREAM_ERR.to_string()),
  }

  Ok(Value::nil())
}

#[cfg(unix)]
fn unix_read(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(UNIX_STREAM));
  enforce_method_arg_type!(ctx, 1, ArgType::Number);

  let length = ctx.args[1].as_number() as usize;
  let mut buffer = vec![0u8; length];

  let read = {
    let mut ptr = ctx.args[0].as_ptr_cell().borrow_mut();
    let socket = ptr.downcast_mut::<ZuriUnix>().unwrap();

    socket
      .stream_mut()?
      .read(buffer.as_mut_slice())
      .map_err(|e| e.to_string())?
  };

  buffer.truncate(read);

  Ok(ctx.heap().alloc_bytes(buffer))
}

#[cfg(unix)]
fn unix_read_exact(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(UNIX_STREAM));
  enforce_method_arg_type!(ctx, 1, ArgType::Number);

  let length = ctx.args[1].as_number() as usize;
  let mut buffer = vec![0u8; length];

  {
    let mut ptr = ctx.args[0].as_ptr_cell().borrow_mut();
    let socket = ptr.downcast_mut::<ZuriUnix>().unwrap();

    socket
      .stream_mut()?
      .read_exact(buffer.as_mut_slice())
      .map_err(|e| e.to_string())?;
  }

  Ok(ctx.heap().alloc_bytes(buffer))
}

#[cfg(unix)]
fn unix_read_all(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(UNIX_STREAM));

  let mut buffer = Vec::new();

  {
    let mut ptr = ctx.args[0].as_ptr_cell().borrow_mut();
    let socket = ptr.downcast_mut::<ZuriUnix>().unwrap();

    socket
      .stream_mut()?
      .read_to_end(&mut buffer)
      .map_err(|e| e.to_string())?;
  }

  Ok(ctx.heap().alloc_bytes(buffer))
}

#[cfg(unix)]
fn unix_read_as_string(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(UNIX_STREAM));

  let mut text = String::new();

  {
    let mut ptr = ctx.args[0].as_ptr_cell().borrow_mut();
    let socket = ptr.downcast_mut::<ZuriUnix>().unwrap();

    socket
      .stream_mut()?
      .read_to_string(&mut text)
      .map_err(|e| e.to_string())?;
  }

  Ok(ctx.heap().alloc_string(text))
}

#[cfg(unix)]
fn unix_write(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(UNIX_STREAM));
  enforce_method_arg_type_any_of!(ctx, 1, [ArgType::Bytes, ArgType::String]);

  let data = get_data(ctx.args[1]);
  let mut ptr = ctx.args[0].as_ptr_cell().borrow_mut();
  let socket = ptr.downcast_mut::<ZuriUnix>().unwrap();

  let written = socket
    .stream_mut()?
    .write(data.as_slice())
    .map_err(|e| e.to_string())?;

  Ok(Value::number(written as f64))
}

#[cfg(unix)]
fn unix_write_all(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(UNIX_STREAM));
  enforce_method_arg_type_any_of!(ctx, 1, [ArgType::Bytes, ArgType::String]);

  let data = get_data(ctx.args[1]);
  let mut ptr = ctx.args[0].as_ptr_cell().borrow_mut();
  let socket = ptr.downcast_mut::<ZuriUnix>().unwrap();

  socket
    .stream_mut()?
    .write_all(data.as_slice())
    .map_err(|e| e.to_string())?;

  Ok(Value::nil())
}

#[cfg(unix)]
fn unix_flush(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(UNIX_STREAM));

  let mut ptr = ctx.args[0].as_ptr_cell().borrow_mut();
  let socket = ptr.downcast_mut::<ZuriUnix>().unwrap();

  socket.stream_mut()?.flush().map_err(|e| e.to_string())?;

  Ok(Value::nil())
}

#[cfg(unix)]
fn unix_is_connected(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(UNIX_STREAM));

  let ptr = ctx.args[0].as_ptr_cell().borrow();

  Ok(Value::bool(
    ptr.downcast_ref::<ZuriUnix>().unwrap().stream.is_some(),
  ))
}

#[cfg(unix)]
fn unix_is_bound(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(UNIX_STREAM));

  let ptr = ctx.args[0].as_ptr_cell().borrow();

  Ok(Value::bool(
    ptr.downcast_ref::<ZuriUnix>().unwrap().listener.is_some(),
  ))
}

#[cfg(unix)]
fn unix_is_supported(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 0);

  Ok(Value::bool(true))
}

#[cfg(unix)]
fn unix_close(ctx: &mut ZuriContext) -> Result<Value, String> {
  use std::net::Shutdown;

  enforce_method_arg_count!(ctx, 0);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(UNIX_STREAM));

  let mut ptr = ctx.args[0].as_ptr_cell().borrow_mut();
  let socket = ptr.downcast_mut::<ZuriUnix>().unwrap();

  if let Some(stream) = &mut socket.stream {
    _ = stream.flush();
    _ = stream.shutdown(Shutdown::Both);
  }

  socket.stream = None;

  // The listener is dropped, which closes the descriptor. The path it
  // was bound to stays on the filesystem: removing it is the caller's
  // business, since another process may have replaced it by now.
  socket.listener = None;

  ptr.type_name = UNIX_STREAM_CLOSED;

  Ok(Value::nil())
}
