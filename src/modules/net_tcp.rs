use std::io::{Error, Read, Write};
use std::net::{Shutdown, SocketAddr, TcpListener, TcpStream, ToSocketAddrs};
use std::time::Duration;

use crate::builtins::enforce::{
  ArgType, enforce_method_arg_count, enforce_method_arg_range, enforce_method_arg_type,
  enforce_method_arg_type_any_of,
};
use crate::enforce_arg_count;
use crate::modules::{BuiltinModuleDef, native, optional_number};
use crate::vm::object::ZuriContext;
use crate::vm::value::Value;
use crate::vm::vm::VM;

pub static MODULE: BuiltinModuleDef = BuiltinModuleDef {
  name: "_net_tcp",
  build,
};

fn build(vm: &mut VM) -> Vec<(&'static str, Value)> {
  vec![
    ("tcp_new", native(vm, "@new", 0, false, tcp_new)),
    ("tcp_resolve", native(vm, "resolve", 1, false, tcp_resolve)),
    ("tcp_connect", native(vm, "connect", 1, true, tcp_connect)),
    ("tcp_bind", native(vm, "bind", 2, false, tcp_bind)),
    ("tcp_accept", native(vm, "accept", 1, false, tcp_accept)),
    (
      "tcp_peer_address",
      native(vm, "peer_address", 1, false, tcp_peer_address),
    ),
    (
      "tcp_local_address",
      native(vm, "local_address", 1, false, tcp_local_address),
    ),
    (
      "tcp_shutdown",
      native(vm, "shutdown", 1, true, tcp_shutdown),
    ),
    (
      "tcp_set_read_timeout",
      native(vm, "set_read_timeout", 2, false, tcp_set_read_timeout),
    ),
    (
      "tcp_set_write_timeout",
      native(vm, "set_write_timeout", 2, false, tcp_set_write_timeout),
    ),
    (
      "tcp_get_read_timeout",
      native(vm, "get_read_timeout", 1, false, tcp_get_read_timeout),
    ),
    (
      "tcp_get_write_timeout",
      native(vm, "get_write_timeout", 1, false, tcp_get_write_timeout),
    ),
    ("tcp_peek", native(vm, "peek", 1, true, tcp_peek)),
    (
      "tcp_set_nodelay",
      native(vm, "set_nodelay", 2, false, tcp_set_nodelay),
    ),
    (
      "tcp_get_nodelay",
      native(vm, "get_nodelay", 1, false, tcp_get_nodelay),
    ),
    ("tcp_set_ttl", native(vm, "set_ttl", 2, false, tcp_set_ttl)),
    ("tcp_get_ttl", native(vm, "get_ttl", 1, false, tcp_get_ttl)),
    (
      "tcp_get_error",
      native(vm, "get_error", 1, false, tcp_get_error),
    ),
    (
      "tcp_set_non_blocking",
      native(vm, "set_non_blocking", 2, false, tcp_set_non_blocking),
    ),
    ("tcp_read", native(vm, "read", 2, false, tcp_read)),
    (
      "tcp_read_exact",
      native(vm, "read_exact", 2, true, tcp_read_exact),
    ),
    (
      "tcp_read_all",
      native(vm, "read_all", 1, false, tcp_read_all),
    ),
    (
      "tcp_read_as_string",
      native(vm, "read_as_string", 1, false, tcp_read_as_string),
    ),
    ("tcp_write", native(vm, "write", 2, false, tcp_write)),
    (
      "tcp_write_all",
      native(vm, "write_all", 2, false, tcp_write_all),
    ),
    ("tcp_flush", native(vm, "flush", 1, false, tcp_flush)),
    (
      "tcp_is_connected",
      native(vm, "is_connected", 1, false, tcp_is_connected),
    ),
    (
      "tcp_is_bound",
      native(vm, "is_bound", 1, false, tcp_is_bound),
    ),
    ("tcp_close", native(vm, "close", 1, false, tcp_close)),
  ]
}

pub(crate) const TCP_STREAM: &str = "zuri::net::TcpStream";

/// Tag a `TcpStream` is switched to once its underlying socket has been
/// handed off to a `TlsStream` for a STARTTLS-style upgrade, same idea as
/// `tcp_close`'s own invalidation below: the Zuri-level instance is dead
/// and any further method call on it should fail loudly instead of
/// silently operating on a socket that TLS now owns.
pub(crate) const TCP_STREAM_UPGRADED: &str = "zuri::net::TcpStream::__upgraded__";

fn get_data(args: &[Value]) -> Vec<u8> {
  let value = args[0];
  if value.is_string() {
    value.as_str().as_bytes().to_vec()
  } else {
    value.with_bytes(|b| b.to_vec())
  }
}

pub(crate) struct ZuriTcp {
  pub stream: Option<TcpStream>,
  pub listener: Option<TcpListener>,
}

static INVALID_STREAM_ERR: &str = "Invalid stream state";

impl ZuriTcp {
  fn new() -> Self {
    ZuriTcp {
      stream: None,
      listener: None,
    }
  }

  fn set_stream(&mut self, stream: TcpStream) {
    self.stream = Some(stream);
  }

  fn set_listener(&mut self, listener: TcpListener) {
    self.listener = Some(listener);
  }

  fn is_connected(&self) -> bool {
    self.stream.is_some()
  }

  fn is_bound(&self) -> bool {
    self.listener.is_some()
  }

  /// Hands the connected socket out to a caller that's about to wrap it in
  /// TLS. Used for STARTTLS-style upgrades, where the connection starts out
  /// plaintext and only becomes encrypted partway through, on the same
  /// underlying socket, rather than by reconnecting.
  pub(crate) fn take_stream(&mut self) -> Result<TcpStream, String> {
    self
      .stream
      .take()
      .ok_or_else(|| INVALID_STREAM_ERR.to_string())
  }

  fn peer_addr(&self) -> Result<SocketAddr, String> {
    if let Some(stream) = &self.stream {
      stream.peer_addr().map_err(|e| e.to_string())
    } else {
      Err(INVALID_STREAM_ERR.to_string())
    }
  }

  fn local_addr(&self) -> Result<SocketAddr, String> {
    if let Some(stream) = &self.stream {
      stream.local_addr().map_err(|e| e.to_string())
    } else if let Some(listener) = &self.listener {
      listener.local_addr().map_err(|e| e.to_string())
    } else {
      Err(INVALID_STREAM_ERR.to_string())
    }
  }

  fn shutdown(&self, how: Shutdown) -> Result<(), String> {
    if let Some(stream) = &self.stream {
      stream.shutdown(how).map_err(|e| e.to_string())
    } else {
      Err(INVALID_STREAM_ERR.to_string())
    }
  }

  fn set_read_timeout(&self, dur: Option<Duration>) -> Result<(), String> {
    if let Some(stream) = &self.stream {
      stream.set_read_timeout(dur).map_err(|e| e.to_string())
    } else {
      Err(INVALID_STREAM_ERR.to_string())
    }
  }

  fn set_write_timeout(&self, dur: Option<Duration>) -> Result<(), String> {
    if let Some(stream) = &self.stream {
      stream.set_write_timeout(dur).map_err(|e| e.to_string())
    } else {
      Err(INVALID_STREAM_ERR.to_string())
    }
  }

  fn read_timeout(&self) -> Result<Option<Duration>, String> {
    if let Some(stream) = &self.stream {
      stream.read_timeout().map_err(|e| e.to_string())
    } else {
      Err(INVALID_STREAM_ERR.to_string())
    }
  }

  fn write_timeout(&self) -> Result<Option<Duration>, String> {
    if let Some(stream) = &self.stream {
      stream.write_timeout().map_err(|e| e.to_string())
    } else {
      Err(INVALID_STREAM_ERR.to_string())
    }
  }

  fn peek(&self, buf: &mut [u8]) -> Result<usize, String> {
    if let Some(stream) = &self.stream {
      stream.peek(buf).map_err(|e| e.to_string())
    } else {
      Err(INVALID_STREAM_ERR.to_string())
    }
  }

  fn set_nodelay(&self, nodelay: bool) -> Result<(), String> {
    if let Some(stream) = &self.stream {
      stream.set_nodelay(nodelay).map_err(|e| e.to_string())
    } else {
      Err(INVALID_STREAM_ERR.to_string())
    }
  }

  fn nodelay(&self) -> Result<bool, String> {
    if let Some(stream) = &self.stream {
      stream.nodelay().map_err(|e| e.to_string())
    } else {
      Err(INVALID_STREAM_ERR.to_string())
    }
  }

  fn set_ttl(&self, ttl: u32) -> Result<(), String> {
    if let Some(stream) = &self.stream {
      stream.set_ttl(ttl).map_err(|e| e.to_string())
    } else if let Some(listener) = &self.listener {
      listener.set_ttl(ttl).map_err(|e| e.to_string())
    } else {
      Err(INVALID_STREAM_ERR.to_string())
    }
  }

  fn ttl(&self) -> Result<u32, String> {
    if let Some(stream) = &self.stream {
      stream.ttl().map_err(|e| e.to_string())
    } else if let Some(listener) = &self.listener {
      listener.ttl().map_err(|e| e.to_string())
    } else {
      Err(INVALID_STREAM_ERR.to_string())
    }
  }

  fn take_error(&self) -> Result<Option<Error>, String> {
    if let Some(stream) = &self.stream {
      stream.take_error().map_err(|e| e.to_string())
    } else if let Some(listener) = &self.listener {
      listener.take_error().map_err(|e| e.to_string())
    } else {
      Err(INVALID_STREAM_ERR.to_string())
    }
  }

  fn set_nonblocking(&self, nonblocking: bool) -> Result<(), String> {
    if let Some(stream) = &self.stream {
      stream
        .set_nonblocking(nonblocking)
        .map_err(|e| e.to_string())
    } else if let Some(listener) = &self.listener {
      listener
        .set_nonblocking(nonblocking)
        .map_err(|e| e.to_string())
    } else {
      Err(INVALID_STREAM_ERR.to_string())
    }
  }

  fn read(&mut self, buf: &mut [u8]) -> Result<usize, String> {
    if let Some(stream) = &mut self.stream {
      stream.read(buf).map_err(|e| e.to_string())
    } else {
      Err(INVALID_STREAM_ERR.to_string())
    }
  }

  fn read_to_end(&mut self, buf: &mut Vec<u8>) -> Result<usize, String> {
    if let Some(stream) = &mut self.stream {
      stream.read_to_end(buf).map_err(|e| e.to_string())
    } else {
      Err(INVALID_STREAM_ERR.to_string())
    }
  }

  fn read_to_string(&mut self, buf: &mut String) -> Result<usize, String> {
    if let Some(stream) = &mut self.stream {
      stream.read_to_string(buf).map_err(|e| e.to_string())
    } else {
      Err(INVALID_STREAM_ERR.to_string())
    }
  }

  fn read_exact(&mut self, buf: &mut [u8]) -> Result<(), String> {
    if let Some(stream) = &mut self.stream {
      stream.read_exact(buf).map_err(|e| e.to_string())
    } else {
      Err(INVALID_STREAM_ERR.to_string())
    }
  }

  fn write(&mut self, buf: &[u8]) -> Result<usize, String> {
    if let Some(stream) = &mut self.stream {
      stream.write(buf).map_err(|e| e.to_string())
    } else {
      Err(INVALID_STREAM_ERR.to_string())
    }
  }

  fn write_all(&mut self, buf: &[u8]) -> Result<(), String> {
    if let Some(stream) = &mut self.stream {
      stream.write_all(buf).map_err(|e| e.to_string())
    } else {
      Err(INVALID_STREAM_ERR.to_string())
    }
  }

  fn flush(&mut self) -> Result<(), String> {
    if let Some(stream) = &mut self.stream {
      stream.flush().map_err(|e| e.to_string())
    } else {
      Err(INVALID_STREAM_ERR.to_string())
    }
  }

  fn accept(&self) -> Result<(TcpStream, SocketAddr), String> {
    if let Some(listener) = &self.listener {
      listener.accept().map_err(|e| e.to_string())
    } else {
      Err("".to_string())
    }
  }
}

fn tcp_new(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 0);
  Ok(ctx.heap().alloc_ptr(TCP_STREAM, ZuriTcp::new()))
}

/// Resolves a `host:port` string to every socket address it names.
///
/// This is the only way Zuri code can turn a hostname into a concrete
/// address, which `TcpStream.connect()` needs before it can accept a
/// connect timeout (`TcpStream::connect_timeout` takes a parsed
/// `SocketAddr`, not a name).
fn tcp_resolve(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 1);
  enforce_method_arg_type!(ctx, 0, ArgType::String);

  let address = ctx.args[0].as_str().to_string();
  let resolved = address.to_socket_addrs().map_err(|e| e.to_string())?;

  let strings: Vec<String> = resolved.map(|addr| addr.to_string()).collect();

  if strings.is_empty() {
    return Err(format!("could not resolve {address}"));
  }

  let values: Vec<Value> = strings
    .into_iter()
    .map(|s| ctx.heap().alloc_string(s))
    .collect();

  Ok(ctx.heap().alloc_list(values))
}

fn tcp_connect(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_range!(ctx, 1, 2);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(TCP_STREAM));
  enforce_method_arg_type!(ctx, 1, ArgType::String);

  let mut ptr = ctx.args[0].as_ptr_cell().borrow_mut();
  let tcp = ptr.downcast_mut::<ZuriTcp>().unwrap();

  if tcp.is_bound() {
    return Err(String::from("Cannot connect to bound socket"));
  } else if tcp.is_connected() {
    return Err(String::from("Socket already connected to another stream"));
  }

  let address = ctx.args[1].as_str();
  let timeout = optional_number(ctx, 2, 0.0)?;

  let stream = if timeout == 0.0 {
    TcpStream::connect(address)
  } else {
    let addr: SocketAddr = address.parse::<SocketAddr>().map_err(|e| e.to_string())?;
    TcpStream::connect_timeout(&addr, Duration::from_millis(timeout as u64))
  }
  .map_err(|e| e.to_string())?;

  tcp.set_stream(stream);

  Ok(Value::nil())
}

fn tcp_peer_address(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(TCP_STREAM));

  let ptr = ctx.args[0].as_ptr_cell().borrow();
  let tcp = ptr.downcast_ref::<ZuriTcp>().unwrap();

  let addr = tcp.peer_addr()?;

  Ok(ctx.heap().alloc_string(addr.to_string()))
}

fn tcp_local_address(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(TCP_STREAM));

  let ptr = ctx.args[0].as_ptr_cell().borrow();
  let tcp = ptr.downcast_ref::<ZuriTcp>().unwrap();

  let addr = tcp.local_addr()?;

  Ok(ctx.heap().alloc_string(addr.to_string()))
}

fn tcp_shutdown(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_range!(ctx, 1, 2);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(TCP_STREAM));

  let ptr = ctx.args[0].as_ptr_cell().borrow();
  let tcp = ptr.downcast_ref::<ZuriTcp>().unwrap();

  let how_type = optional_number(ctx, 1, 0.0)?;
  let how = match how_type {
    0.0 => Ok(Shutdown::Both),
    1.0 => Ok(Shutdown::Read),
    2.0 => Ok(Shutdown::Write),
    _ => Err("Unknown shutdown kind"),
  }?;

  tcp.shutdown(how)?;

  Ok(Value::nil())
}

fn tcp_set_read_timeout(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(TCP_STREAM));
  enforce_method_arg_type!(ctx, 1, ArgType::Number);

  let ptr = ctx.args[0].as_ptr_cell().borrow();
  let tcp = ptr.downcast_ref::<ZuriTcp>().unwrap();
  let timeout = ctx.args[1].as_number();

  let duration = if timeout > 0.0 {
    Some(Duration::from_millis(timeout as u64))
  } else {
    None
  };

  tcp.set_read_timeout(duration)?;

  Ok(Value::nil())
}

fn tcp_get_read_timeout(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(TCP_STREAM));

  let ptr = ctx.args[0].as_ptr_cell().borrow();
  let tcp = ptr.downcast_ref::<ZuriTcp>().unwrap();

  if let Some(timeout) = tcp.read_timeout()? {
    Ok(Value::number(timeout.as_millis() as f64))
  } else {
    Ok(Value::number(-1.0))
  }
}

fn tcp_set_write_timeout(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(TCP_STREAM));
  enforce_method_arg_type!(ctx, 1, ArgType::Number);

  let ptr = ctx.args[0].as_ptr_cell().borrow();
  let tcp = ptr.downcast_ref::<ZuriTcp>().unwrap();
  let timeout = ctx.args[1].as_number();

  let duration = if timeout > -1.0 {
    Some(Duration::from_millis(timeout as u64))
  } else {
    None
  };

  tcp.set_write_timeout(duration).map_err(|e| e.to_string())?;

  Ok(Value::nil())
}

fn tcp_get_write_timeout(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(TCP_STREAM));

  let ptr = ctx.args[0].as_ptr_cell().borrow();
  let tcp = ptr.downcast_ref::<ZuriTcp>().unwrap();

  if let Some(timeout) = tcp.write_timeout()? {
    Ok(Value::number(timeout.as_millis() as f64))
  } else {
    Ok(Value::number(-1.0))
  }
}

fn tcp_set_nodelay(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(TCP_STREAM));
  enforce_method_arg_type!(ctx, 1, ArgType::Bool);

  let ptr = ctx.args[0].as_ptr_cell().borrow();
  let tcp = ptr.downcast_ref::<ZuriTcp>().unwrap();

  tcp.set_nodelay(ctx.args[1].as_bool())?;

  Ok(Value::nil())
}

fn tcp_get_nodelay(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(TCP_STREAM));

  let ptr = ctx.args[0].as_ptr_cell().borrow();
  let tcp = ptr.downcast_ref::<ZuriTcp>().unwrap();

  Ok(Value::bool(tcp.nodelay().unwrap_or(false)))
}

fn tcp_set_ttl(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(TCP_STREAM));
  enforce_method_arg_type!(ctx, 1, ArgType::Number);

  let ptr = ctx.args[0].as_ptr_cell().borrow();
  let tcp = ptr.downcast_ref::<ZuriTcp>().unwrap();

  tcp.set_ttl(ctx.args[1].as_number() as u32)?;

  Ok(Value::nil())
}

fn tcp_get_ttl(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(TCP_STREAM));

  let ptr = ctx.args[0].as_ptr_cell().borrow();
  let tcp = ptr.downcast_ref::<ZuriTcp>().unwrap();

  Ok(Value::number(tcp.ttl().unwrap_or(0) as f64))
}

fn tcp_peek(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_range!(ctx, 0, 1);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(TCP_STREAM));

  let ptr = ctx.args[0].as_ptr_cell().borrow();
  let tcp = ptr.downcast_ref::<ZuriTcp>().unwrap();
  let length = optional_number(ctx, 1, 1.0)? as usize;

  let mut buffer = vec![0u8; length];
  let len = tcp.peek(&mut buffer)?;

  if len > 0 {
    Ok(ctx.heap().alloc_bytes(buffer))
  } else {
    Ok(Value::nil())
  }
}

fn tcp_get_error(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(TCP_STREAM));

  let ptr = ctx.args[0].as_ptr_cell().borrow();
  let tcp = ptr.downcast_ref::<ZuriTcp>().unwrap();

  if let Some(error) = tcp.take_error()? {
    Ok(ctx.heap().alloc_string(error.to_string()))
  } else {
    Ok(Value::nil())
  }
}

fn tcp_set_non_blocking(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(TCP_STREAM));
  enforce_method_arg_type!(ctx, 1, ArgType::Bool);

  let ptr = ctx.args[0].as_ptr_cell().borrow();
  let tcp = ptr.downcast_ref::<ZuriTcp>().unwrap();

  tcp.set_nonblocking(ctx.args[1].as_bool())?;

  Ok(Value::nil())
}

fn tcp_read(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(TCP_STREAM));
  enforce_method_arg_type!(ctx, 1, ArgType::Number);

  let mut ptr = ctx.args[0].as_ptr_cell().borrow_mut();
  let tcp = ptr.downcast_mut::<ZuriTcp>().unwrap();

  let length = ctx.args[1].as_number() as usize;

  let mut buffer = vec![0u8; length];
  let bytes_read = tcp.read(buffer.as_mut_slice())?;

  if bytes_read > 0 {
    return Ok(ctx.heap().alloc_bytes(&buffer[0..bytes_read]));
  }

  Ok(ctx.heap().alloc_bytes(Vec::new()))
}

fn tcp_read_exact(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(TCP_STREAM));
  enforce_method_arg_type!(ctx, 1, ArgType::Number);

  let mut ptr = ctx.args[0].as_ptr_cell().borrow_mut();
  let tcp = ptr.downcast_mut::<ZuriTcp>().unwrap();

  let length = ctx.args[1].as_number() as usize;

  let mut buffer = vec![0u8; length];
  tcp.read_exact(buffer.as_mut_slice())?;

  Ok(ctx.heap().alloc_bytes(buffer))
}

fn tcp_read_all(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(TCP_STREAM));

  let mut ptr = ctx.args[0].as_ptr_cell().borrow_mut();
  let tcp = ptr.downcast_mut::<ZuriTcp>().unwrap();

  let mut buffer = Vec::new();
  tcp.read_to_end(&mut buffer)?;

  Ok(ctx.heap().alloc_bytes(buffer))
}

fn tcp_read_as_string(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(TCP_STREAM));

  let mut ptr = ctx.args[0].as_ptr_cell().borrow_mut();
  let tcp = ptr.downcast_mut::<ZuriTcp>().unwrap();

  let mut buffer = String::new();
  tcp.read_to_string(&mut buffer)?;

  Ok(ctx.heap().alloc_string(buffer))
}

fn tcp_write(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(TCP_STREAM));
  enforce_method_arg_type_any_of!(ctx, 1, [ArgType::Bytes, ArgType::String]);

  let mut ptr = ctx.args[0].as_ptr_cell().borrow_mut();
  let tcp = ptr.downcast_mut::<ZuriTcp>().unwrap();

  let data = get_data(&ctx.args[1..]);

  Ok(Value::number(tcp.write(data.as_slice())? as f64))
}

fn tcp_write_all(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(TCP_STREAM));
  enforce_method_arg_type_any_of!(ctx, 1, [ArgType::Bytes, ArgType::String]);

  let mut ptr = ctx.args[0].as_ptr_cell().borrow_mut();
  let tcp = ptr.downcast_mut::<ZuriTcp>().unwrap();

  let data = get_data(&ctx.args[1..]);
  tcp.write_all(data.as_slice())?;

  Ok(Value::nil())
}

fn tcp_flush(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(TCP_STREAM));

  let mut ptr = ctx.args[0].as_ptr_cell().borrow_mut();
  let tcp = ptr.downcast_mut::<ZuriTcp>().unwrap();

  tcp.flush()?;

  Ok(Value::nil())
}

fn tcp_bind(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(TCP_STREAM));
  enforce_method_arg_type!(ctx, 1, ArgType::String);

  let mut ptr = ctx.args[0].as_ptr_cell().borrow_mut();
  let tcp = ptr.downcast_mut::<ZuriTcp>().unwrap();

  if tcp.is_connected() {
    return Err(String::from("Cannot bind to connected socket"));
  } else if tcp.is_bound() {
    return Err(String::from("Socket already bound to an address"));
  }

  tcp.set_listener(TcpListener::bind(ctx.args[1].as_str()).map_err(|e| e.to_string())?);

  Ok(Value::nil())
}

fn tcp_accept(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(TCP_STREAM));

  let mut ptr = ctx.args[0].as_ptr_cell().borrow_mut();
  let tcp = ptr.downcast_mut::<ZuriTcp>().unwrap();

  let (stream, _) = tcp.accept()?;

  let mut new_tcp = ZuriTcp::new();
  new_tcp.set_stream(stream);

  Ok(ctx.heap().alloc_ptr(TCP_STREAM, new_tcp))
}

fn tcp_is_connected(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(TCP_STREAM));

  let mut ptr = ctx.args[0].as_ptr_cell().borrow_mut();
  let tcp = ptr.downcast_mut::<ZuriTcp>().unwrap();

  Ok(Value::bool(tcp.stream.is_some()))
}

fn tcp_is_bound(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(TCP_STREAM));

  let mut ptr = ctx.args[0].as_ptr_cell().borrow_mut();
  let tcp = ptr.downcast_mut::<ZuriTcp>().unwrap();

  Ok(Value::bool(tcp.listener.is_some()))
}

fn tcp_close(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(TCP_STREAM));

  let mut ptr = ctx.args[0].as_ptr_cell().borrow_mut();
  let tcp = ptr.downcast_mut::<ZuriTcp>().unwrap();

  if let Some(stream) = &mut tcp.stream {
    _ = stream.flush().map_err(|e| e.to_string());
    _ = stream.shutdown(Shutdown::Both).map_err(|e| e.to_string());
  }

  tcp.stream = None;
  tcp.listener = None;

  // Make pointer invalid so that the stream cannot be reused.
  ptr.type_name = "zuri::net::TcpStream::__closed__";

  Ok(Value::nil())
}
