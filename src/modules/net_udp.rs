use std::io::Error;
use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr, UdpSocket};
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
  name: "_net_udp",
  build,
};

fn build(vm: &mut VM) -> Vec<(&'static str, Value)> {
  vec![
    ("udp_new", native(vm, "@new", 0, false, udp_new)),
    ("udp_connect", native(vm, "connect", 1, true, udp_connect)),
    ("udp_bind", native(vm, "bind", 2, false, udp_bind)),
    (
      "udp_peer_address",
      native(vm, "peer_address", 1, false, udp_peer_address),
    ),
    (
      "udp_local_address",
      native(vm, "local_address", 1, false, udp_local_address),
    ),
    (
      "udp_set_read_timeout",
      native(vm, "set_read_timeout", 2, false, udp_set_read_timeout),
    ),
    (
      "udp_set_write_timeout",
      native(vm, "set_write_timeout", 2, false, udp_set_write_timeout),
    ),
    (
      "udp_get_read_timeout",
      native(vm, "get_read_timeout", 1, false, udp_get_read_timeout),
    ),
    (
      "udp_get_write_timeout",
      native(vm, "get_write_timeout", 1, false, udp_get_write_timeout),
    ),
    ("udp_peek", native(vm, "peek", 1, true, udp_peek)),
    (
      "udp_peek_from",
      native(vm, "peek_from", 1, true, udp_peek_from),
    ),
    ("udp_set_ttl", native(vm, "set_ttl", 2, false, udp_set_ttl)),
    ("udp_get_ttl", native(vm, "get_ttl", 1, false, udp_get_ttl)),
    (
      "udp_get_error",
      native(vm, "get_error", 1, false, udp_get_error),
    ),
    (
      "udp_set_non_blocking",
      native(vm, "set_non_blocking", 2, false, udp_set_non_blocking),
    ),
    (
      "udp_set_broadcast",
      native(vm, "set_broadcast", 2, false, udp_set_broadcast),
    ),
    (
      "udp_get_broadcast",
      native(vm, "get_broadcast", 1, false, udp_get_broadcast),
    ),
    (
      "udp_set_multicast_loop_v4",
      native(
        vm,
        "set_multicast_loop_v4",
        2,
        false,
        udp_set_multicast_loop_v4,
      ),
    ),
    (
      "udp_get_multicast_loop_v4",
      native(
        vm,
        "get_multicast_loop_v4",
        1,
        false,
        udp_get_multicast_loop_v4,
      ),
    ),
    (
      "udp_set_multicast_ttl_v4",
      native(
        vm,
        "set_multicastttl_v4",
        2,
        false,
        udp_set_multicast_ttl_v4,
      ),
    ),
    (
      "udp_get_multicast_ttl_v4",
      native(
        vm,
        "get_multicast_ttl_v4",
        1,
        false,
        udp_get_multicast_ttl_v4,
      ),
    ),
    (
      "udp_set_multicast_loop_v6",
      native(
        vm,
        "set_multicast_loop_v6",
        2,
        false,
        udp_set_multicast_loop_v6,
      ),
    ),
    (
      "udp_get_multicast_loop_v6",
      native(
        vm,
        "get_multicast_loop_v6",
        1,
        false,
        udp_get_multicast_loop_v6,
      ),
    ),
    ("udp_receive", native(vm, "receive", 2, false, udp_receive)),
    (
      "udp_receive_from",
      native(vm, "receive_from", 2, true, udp_receive_from),
    ),
    ("udp_send", native(vm, "send", 2, false, udp_send)),
    ("udp_send_to", native(vm, "send_to", 3, false, udp_send_to)),
    (
      "udp_join_multicast_v4",
      native(vm, "join_multicast_v4", 3, false, udp_join_multicast_v4),
    ),
    (
      "udp_join_multicast_v6",
      native(vm, "join_multicast_v6", 3, false, udp_join_multicast_v6),
    ),
    (
      "udp_leave_multicast_v4",
      native(vm, "leave_multicast_v4", 3, false, udp_leave_multicast_v4),
    ),
    (
      "udp_leave_multicast_v6",
      native(vm, "leave_multicast_v6", 3, false, udp_leave_multicast_v6),
    ),
    ("udp_close", native(vm, "close", 1, false, udp_close)),
  ]
}

const UDP_STREAM: &str = "zuri::net::UdpStream";

fn get_data(args: &[Value]) -> Vec<u8> {
  let value = args[0];
  if value.is_string() {
    value.as_str().as_bytes().to_vec()
  } else {
    value.with_bytes(|b| b.to_vec())
  }
}

struct ZuriUdp {
  pub socket: Option<UdpSocket>,
}

static INVALID_STREAM_ERR: &str = "Invalid stream state";

impl ZuriUdp {
  fn new() -> Self {
    ZuriUdp { socket: None }
  }

  fn set_socket(&mut self, stream: UdpSocket) {
    self.socket = Some(stream);
  }

  fn peer_addr(&self) -> Result<SocketAddr, String> {
    if let Some(stream) = &self.socket {
      stream.peer_addr().map_err(|e| e.to_string())
    } else {
      Err(INVALID_STREAM_ERR.to_string())
    }
  }

  fn local_addr(&self) -> Result<SocketAddr, String> {
    if let Some(stream) = &self.socket {
      stream.local_addr().map_err(|e| e.to_string())
    } else {
      Err(INVALID_STREAM_ERR.to_string())
    }
  }

  fn set_read_timeout(&self, dur: Option<Duration>) -> Result<(), String> {
    if let Some(stream) = &self.socket {
      stream.set_read_timeout(dur).map_err(|e| e.to_string())
    } else {
      Err(INVALID_STREAM_ERR.to_string())
    }
  }

  fn set_write_timeout(&self, dur: Option<Duration>) -> Result<(), String> {
    if let Some(stream) = &self.socket {
      stream.set_write_timeout(dur).map_err(|e| e.to_string())
    } else {
      Err(INVALID_STREAM_ERR.to_string())
    }
  }

  fn read_timeout(&self) -> Result<Option<Duration>, String> {
    if let Some(stream) = &self.socket {
      stream.read_timeout().map_err(|e| e.to_string())
    } else {
      Err(INVALID_STREAM_ERR.to_string())
    }
  }

  fn write_timeout(&self) -> Result<Option<Duration>, String> {
    if let Some(stream) = &self.socket {
      stream.write_timeout().map_err(|e| e.to_string())
    } else {
      Err(INVALID_STREAM_ERR.to_string())
    }
  }

  fn peek(&self, buf: &mut [u8]) -> Result<usize, String> {
    if let Some(stream) = &self.socket {
      stream.peek(buf).map_err(|e| e.to_string())
    } else {
      Err(INVALID_STREAM_ERR.to_string())
    }
  }

  fn peek_from(&self, buf: &mut [u8]) -> Result<(usize, SocketAddr), String> {
    if let Some(stream) = &self.socket {
      stream.peek_from(buf).map_err(|e| e.to_string())
    } else {
      Err(INVALID_STREAM_ERR.to_string())
    }
  }

  fn set_ttl(&self, ttl: u32) -> Result<(), String> {
    if let Some(stream) = &self.socket {
      stream.set_ttl(ttl).map_err(|e| e.to_string())
    } else {
      Err(INVALID_STREAM_ERR.to_string())
    }
  }

  fn ttl(&self) -> Result<u32, String> {
    if let Some(stream) = &self.socket {
      stream.ttl().map_err(|e| e.to_string())
    } else {
      Err(INVALID_STREAM_ERR.to_string())
    }
  }

  fn take_error(&self) -> Result<Option<Error>, String> {
    if let Some(stream) = &self.socket {
      stream.take_error().map_err(|e| e.to_string())
    } else {
      Err(INVALID_STREAM_ERR.to_string())
    }
  }

  fn set_nonblocking(&self, nonblocking: bool) -> Result<(), String> {
    if let Some(stream) = &self.socket {
      stream
        .set_nonblocking(nonblocking)
        .map_err(|e| e.to_string())
    } else {
      Err(INVALID_STREAM_ERR.to_string())
    }
  }

  fn set_broadcast(&self, broadcast: bool) -> Result<(), String> {
    if let Some(stream) = &self.socket {
      stream.set_broadcast(broadcast).map_err(|e| e.to_string())
    } else {
      Err(INVALID_STREAM_ERR.to_string())
    }
  }

  fn broadcast(&self) -> Result<bool, String> {
    if let Some(stream) = &self.socket {
      stream.broadcast().map_err(|e| e.to_string())
    } else {
      Err(INVALID_STREAM_ERR.to_string())
    }
  }

  fn set_multicast_loop_v4(&self, multicast_loop_v4: bool) -> Result<(), String> {
    if let Some(stream) = &self.socket {
      stream
        .set_multicast_loop_v4(multicast_loop_v4)
        .map_err(|e| e.to_string())
    } else {
      Err(INVALID_STREAM_ERR.to_string())
    }
  }

  fn multicast_loop_v4(&self) -> Result<bool, String> {
    if let Some(stream) = &self.socket {
      stream.multicast_loop_v4().map_err(|e| e.to_string())
    } else {
      Err(INVALID_STREAM_ERR.to_string())
    }
  }

  fn set_multicast_loop_v6(&self, multicast_loop_v6: bool) -> Result<(), String> {
    if let Some(stream) = &self.socket {
      stream
        .set_multicast_loop_v6(multicast_loop_v6)
        .map_err(|e| e.to_string())
    } else {
      Err(INVALID_STREAM_ERR.to_string())
    }
  }

  fn multicast_loop_v6(&self) -> Result<bool, String> {
    if let Some(stream) = &self.socket {
      stream.multicast_loop_v6().map_err(|e| e.to_string())
    } else {
      Err(INVALID_STREAM_ERR.to_string())
    }
  }

  fn set_multicast_ttl_v4(&self, multicast_ttl_v4: u32) -> Result<(), String> {
    if let Some(stream) = &self.socket {
      stream
        .set_multicast_ttl_v4(multicast_ttl_v4)
        .map_err(|e| e.to_string())
    } else {
      Err(INVALID_STREAM_ERR.to_string())
    }
  }

  fn multicast_ttl_v4(&self) -> Result<u32, String> {
    if let Some(stream) = &self.socket {
      stream.multicast_ttl_v4().map_err(|e| e.to_string())
    } else {
      Err(INVALID_STREAM_ERR.to_string())
    }
  }

  fn join_multicast_v4(&self, multiaddr: &Ipv4Addr, interface: &Ipv4Addr) -> Result<(), String> {
    if let Some(stream) = &self.socket {
      stream
        .join_multicast_v4(multiaddr, interface)
        .map_err(|e| e.to_string())
    } else {
      Err(INVALID_STREAM_ERR.to_string())
    }
  }

  fn leave_multicast_v4(&self, multiaddr: &Ipv4Addr, interface: &Ipv4Addr) -> Result<(), String> {
    if let Some(stream) = &self.socket {
      stream
        .leave_multicast_v4(multiaddr, interface)
        .map_err(|e| e.to_string())
    } else {
      Err(INVALID_STREAM_ERR.to_string())
    }
  }

  fn join_multicast_v6(&self, multiaddr: &Ipv6Addr, interface: u32) -> Result<(), String> {
    if let Some(stream) = &self.socket {
      stream
        .join_multicast_v6(multiaddr, interface)
        .map_err(|e| e.to_string())
    } else {
      Err(INVALID_STREAM_ERR.to_string())
    }
  }

  fn leave_multicast_v6(&self, multiaddr: &Ipv6Addr, interface: u32) -> Result<(), String> {
    if let Some(stream) = &self.socket {
      stream
        .leave_multicast_v6(multiaddr, interface)
        .map_err(|e| e.to_string())
    } else {
      Err(INVALID_STREAM_ERR.to_string())
    }
  }

  fn connect(&mut self, addr: &str) -> Result<(), String> {
    if let Some(stream) = &mut self.socket {
      stream.connect(addr).map_err(|e| e.to_string())
    } else {
      Err(INVALID_STREAM_ERR.to_string())
    }
  }

  fn receive(&mut self, buf: &mut [u8]) -> Result<usize, String> {
    if let Some(stream) = &mut self.socket {
      stream.recv(buf).map_err(|e| e.to_string())
    } else {
      Err(INVALID_STREAM_ERR.to_string())
    }
  }

  fn receive_from(&mut self, buf: &mut [u8]) -> Result<(usize, SocketAddr), String> {
    if let Some(stream) = &mut self.socket {
      stream.recv_from(buf).map_err(|e| e.to_string())
    } else {
      Err(INVALID_STREAM_ERR.to_string())
    }
  }

  fn send(&mut self, buf: &[u8]) -> Result<usize, String> {
    if let Some(stream) = &mut self.socket {
      stream.send(buf).map_err(|e| e.to_string())
    } else {
      Err(INVALID_STREAM_ERR.to_string())
    }
  }

  fn send_to(&self, buf: &[u8], addr: &str) -> Result<usize, String> {
    if let Some(stream) = &self.socket {
      stream.send_to(buf, addr).map_err(|e| e.to_string())
    } else {
      Err(INVALID_STREAM_ERR.to_string())
    }
  }
}

fn udp_new(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 0);
  Ok(ctx.heap().alloc_ptr(UDP_STREAM, ZuriUdp::new()))
}

fn udp_connect(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_range!(ctx, 1, 2);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(UDP_STREAM));
  enforce_method_arg_type!(ctx, 1, ArgType::String);

  let mut ptr = ctx.args[0].as_ptr_cell().borrow_mut();
  let udp = ptr.downcast_mut::<ZuriUdp>().unwrap();

  let address = ctx.args[1].as_str();

  udp.connect(address)?;

  Ok(Value::nil())
}

fn udp_peer_address(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(UDP_STREAM));

  let ptr = ctx.args[0].as_ptr_cell().borrow();
  let udp = ptr.downcast_ref::<ZuriUdp>().unwrap();

  let addr = udp.peer_addr()?;

  Ok(ctx.heap().alloc_string(addr.to_string()))
}

fn udp_local_address(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(UDP_STREAM));

  let ptr = ctx.args[0].as_ptr_cell().borrow();
  let udp = ptr.downcast_ref::<ZuriUdp>().unwrap();

  let addr = udp.local_addr()?;

  Ok(ctx.heap().alloc_string(addr.to_string()))
}

fn udp_set_read_timeout(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(UDP_STREAM));
  enforce_method_arg_type!(ctx, 1, ArgType::Number);

  let ptr = ctx.args[0].as_ptr_cell().borrow();
  let udp = ptr.downcast_ref::<ZuriUdp>().unwrap();
  let timeout = ctx.args[1].as_number();

  let duration = if timeout > -1.0 {
    Some(Duration::from_millis(timeout as u64))
  } else {
    None
  };

  udp.set_read_timeout(duration)?;

  Ok(Value::nil())
}

fn udp_get_read_timeout(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(UDP_STREAM));

  let ptr = ctx.args[0].as_ptr_cell().borrow();
  let udp = ptr.downcast_ref::<ZuriUdp>().unwrap();

  if let Some(timeout) = udp.read_timeout()? {
    Ok(Value::number(timeout.as_millis() as f64))
  } else {
    Ok(Value::number(-1.0))
  }
}

fn udp_set_write_timeout(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(UDP_STREAM));
  enforce_method_arg_type!(ctx, 1, ArgType::Number);

  let ptr = ctx.args[0].as_ptr_cell().borrow();
  let udp = ptr.downcast_ref::<ZuriUdp>().unwrap();
  let timeout = ctx.args[1].as_number();

  let duration = if timeout > -1.0 {
    Some(Duration::from_millis(timeout as u64))
  } else {
    None
  };

  udp.set_write_timeout(duration).map_err(|e| e.to_string())?;

  Ok(Value::nil())
}

fn udp_get_write_timeout(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(UDP_STREAM));

  let ptr = ctx.args[0].as_ptr_cell().borrow();
  let udp = ptr.downcast_ref::<ZuriUdp>().unwrap();

  if let Some(timeout) = udp.write_timeout()? {
    Ok(Value::number(timeout.as_millis() as f64))
  } else {
    Ok(Value::number(-1.0))
  }
}

fn udp_set_ttl(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(UDP_STREAM));
  enforce_method_arg_type!(ctx, 1, ArgType::Number);

  let ptr = ctx.args[0].as_ptr_cell().borrow();
  let udp = ptr.downcast_ref::<ZuriUdp>().unwrap();

  udp.set_ttl(ctx.args[1].as_number() as u32)?;

  Ok(Value::nil())
}

fn udp_get_ttl(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(UDP_STREAM));

  let ptr = ctx.args[0].as_ptr_cell().borrow();
  let udp = ptr.downcast_ref::<ZuriUdp>().unwrap();

  Ok(Value::number(udp.ttl().unwrap_or(0) as f64))
}

fn udp_set_multicast_ttl_v4(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(UDP_STREAM));
  enforce_method_arg_type!(ctx, 1, ArgType::Number);

  let ptr = ctx.args[0].as_ptr_cell().borrow();
  let udp = ptr.downcast_ref::<ZuriUdp>().unwrap();

  udp.set_multicast_ttl_v4(ctx.args[1].as_number() as u32)?;

  Ok(Value::nil())
}

fn udp_get_multicast_ttl_v4(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(UDP_STREAM));

  let ptr = ctx.args[0].as_ptr_cell().borrow();
  let udp = ptr.downcast_ref::<ZuriUdp>().unwrap();

  Ok(Value::number(udp.multicast_ttl_v4().unwrap_or(0) as f64))
}

fn udp_set_broadcast(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(UDP_STREAM));
  enforce_method_arg_type!(ctx, 1, ArgType::Bool);

  let ptr = ctx.args[0].as_ptr_cell().borrow();
  let udp = ptr.downcast_ref::<ZuriUdp>().unwrap();

  udp.set_broadcast(ctx.args[1].as_bool())?;

  Ok(Value::nil())
}

fn udp_get_broadcast(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(UDP_STREAM));

  let ptr = ctx.args[0].as_ptr_cell().borrow();
  let udp = ptr.downcast_ref::<ZuriUdp>().unwrap();

  Ok(Value::bool(udp.broadcast().unwrap_or(false)))
}

fn udp_set_multicast_loop_v4(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(UDP_STREAM));
  enforce_method_arg_type!(ctx, 1, ArgType::Bool);

  let ptr = ctx.args[0].as_ptr_cell().borrow();
  let udp = ptr.downcast_ref::<ZuriUdp>().unwrap();

  udp.set_multicast_loop_v4(ctx.args[1].as_bool())?;

  Ok(Value::nil())
}

fn udp_get_multicast_loop_v4(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(UDP_STREAM));

  let ptr = ctx.args[0].as_ptr_cell().borrow();
  let udp = ptr.downcast_ref::<ZuriUdp>().unwrap();

  Ok(Value::bool(udp.multicast_loop_v4().unwrap_or(false)))
}

fn udp_set_multicast_loop_v6(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(UDP_STREAM));
  enforce_method_arg_type!(ctx, 1, ArgType::Bool);

  let ptr = ctx.args[0].as_ptr_cell().borrow();
  let udp = ptr.downcast_ref::<ZuriUdp>().unwrap();

  udp.set_multicast_loop_v6(ctx.args[1].as_bool())?;

  Ok(Value::nil())
}

fn udp_get_multicast_loop_v6(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(UDP_STREAM));

  let ptr = ctx.args[0].as_ptr_cell().borrow();
  let udp = ptr.downcast_ref::<ZuriUdp>().unwrap();

  Ok(Value::bool(udp.multicast_loop_v6().unwrap_or(false)))
}

fn udp_peek(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_range!(ctx, 0, 1);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(UDP_STREAM));

  let ptr = ctx.args[0].as_ptr_cell().borrow();
  let udp = ptr.downcast_ref::<ZuriUdp>().unwrap();
  let length = optional_number(ctx, 1, 1.0)? as usize;

  let mut buffer = vec![0u8; length];
  let len = udp.peek(&mut buffer)?;

  if len > 0 {
    Ok(ctx.heap().alloc_bytes(&buffer[0..len]))
  } else {
    Ok(Value::nil())
  }
}

fn udp_peek_from(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_range!(ctx, 0, 1);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(UDP_STREAM));

  let ptr = ctx.args[0].as_ptr_cell().borrow();
  let udp = ptr.downcast_ref::<ZuriUdp>().unwrap();
  let length = optional_number(ctx, 1, 1.0)? as usize;

  let mut buffer = vec![0u8; length];
  let (received, addr) = udp.peek_from(&mut buffer)?;
  buffer.truncate(received);

  let data_key = ctx.heap().alloc_string("data");
  let data_value = ctx.heap().alloc_bytes(buffer);
  let address_key = ctx.heap().alloc_string("address");
  let address_value = ctx.heap().alloc_string(addr.to_string());

  Ok(
    ctx
      .heap()
      .alloc_dict(vec![(data_key, data_value), (address_key, address_value)]),
  )
}

fn udp_get_error(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(UDP_STREAM));

  let ptr = ctx.args[0].as_ptr_cell().borrow();
  let udp = ptr.downcast_ref::<ZuriUdp>().unwrap();

  if let Some(error) = udp.take_error()? {
    Ok(ctx.heap().alloc_string(error.to_string()))
  } else {
    Ok(Value::nil())
  }
}

fn udp_set_non_blocking(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(UDP_STREAM));
  enforce_method_arg_type!(ctx, 1, ArgType::Bool);

  let ptr = ctx.args[0].as_ptr_cell().borrow();
  let udp = ptr.downcast_ref::<ZuriUdp>().unwrap();

  udp.set_nonblocking(ctx.args[1].as_bool())?;

  Ok(Value::nil())
}

fn udp_receive(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(UDP_STREAM));
  enforce_method_arg_type!(ctx, 1, ArgType::Number);

  let mut ptr = ctx.args[0].as_ptr_cell().borrow_mut();
  let udp = ptr.downcast_mut::<ZuriUdp>().unwrap();

  let length = ctx.args[1].as_number() as usize;

  let mut buffer = vec![0u8; length];
  let bytes_read = udp.receive(buffer.as_mut_slice())?;

  if bytes_read > 0 {
    return Ok(ctx.heap().alloc_bytes(&buffer[0..bytes_read]));
  }

  Ok(ctx.heap().alloc_bytes(Vec::new()))
}

fn udp_receive_from(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(UDP_STREAM));
  enforce_method_arg_type!(ctx, 1, ArgType::Number);

  let mut ptr = ctx.args[0].as_ptr_cell().borrow_mut();
  let udp = ptr.downcast_mut::<ZuriUdp>().unwrap();

  let length = ctx.args[1].as_number() as usize;

  let mut buffer = vec![0u8; length];
  let (bytes_read, _) = udp.receive_from(buffer.as_mut_slice())?;

  Ok(ctx.heap().alloc_bytes(&buffer[0..bytes_read]))
}

fn udp_send(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(UDP_STREAM));
  enforce_method_arg_type_any_of!(ctx, 1, [ArgType::Bytes, ArgType::String]);

  let mut ptr = ctx.args[0].as_ptr_cell().borrow_mut();
  let udp = ptr.downcast_mut::<ZuriUdp>().unwrap();

  let data = get_data(&ctx.args[1..]);

  Ok(Value::number(udp.send(data.as_slice())? as f64))
}

fn udp_send_to(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 2);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(UDP_STREAM));
  enforce_method_arg_type_any_of!(ctx, 1, [ArgType::Bytes, ArgType::String]);
  enforce_method_arg_type!(ctx, 2, ArgType::String);

  let mut ptr = ctx.args[0].as_ptr_cell().borrow_mut();
  let udp = ptr.downcast_mut::<ZuriUdp>().unwrap();

  let data = get_data(&ctx.args[1..]);

  Ok(Value::number(
    udp.send_to(data.as_slice(), ctx.args[2].as_str())? as f64,
  ))
}

fn udp_bind(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(UDP_STREAM));
  enforce_method_arg_type!(ctx, 1, ArgType::String);

  let mut ptr = ctx.args[0].as_ptr_cell().borrow_mut();
  let udp = ptr.downcast_mut::<ZuriUdp>().unwrap();

  udp.set_socket(UdpSocket::bind(ctx.args[1].as_str()).map_err(|e| e.to_string())?);

  Ok(Value::nil())
}

fn udp_join_multicast_v4(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 2);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(UDP_STREAM));
  enforce_method_arg_type!(ctx, 1, ArgType::String);
  enforce_method_arg_type!(ctx, 2, ArgType::String);

  let mut ptr = ctx.args[0].as_ptr_cell().borrow_mut();
  let udp = ptr.downcast_mut::<ZuriUdp>().unwrap();

  let addr = ctx.args[1]
    .as_str()
    .parse::<Ipv4Addr>()
    .map_err(|e| e.to_string())?;

  let interface = ctx.args[2]
    .as_str()
    .parse::<Ipv4Addr>()
    .map_err(|e| e.to_string())?;

  udp.join_multicast_v4(&addr, &interface)?;

  Ok(Value::nil())
}

fn udp_leave_multicast_v4(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 2);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(UDP_STREAM));
  enforce_method_arg_type!(ctx, 1, ArgType::String);
  enforce_method_arg_type!(ctx, 2, ArgType::String);

  let mut ptr = ctx.args[0].as_ptr_cell().borrow_mut();
  let udp = ptr.downcast_mut::<ZuriUdp>().unwrap();

  let addr = ctx.args[1]
    .as_str()
    .parse::<Ipv4Addr>()
    .map_err(|e| e.to_string())?;

  let interface = ctx.args[2]
    .as_str()
    .parse::<Ipv4Addr>()
    .map_err(|e| e.to_string())?;

  udp.leave_multicast_v4(&addr, &interface)?;

  Ok(Value::nil())
}

fn udp_join_multicast_v6(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 2);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(UDP_STREAM));
  enforce_method_arg_type!(ctx, 1, ArgType::String);
  enforce_method_arg_type!(ctx, 2, ArgType::Number);

  let mut ptr = ctx.args[0].as_ptr_cell().borrow_mut();
  let udp = ptr.downcast_mut::<ZuriUdp>().unwrap();

  let addr = ctx.args[1]
    .as_str()
    .parse::<Ipv6Addr>()
    .map_err(|e| e.to_string())?;

  udp.join_multicast_v6(&addr, ctx.args[2].as_number() as u32)?;

  Ok(Value::nil())
}

fn udp_leave_multicast_v6(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 2);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(UDP_STREAM));
  enforce_method_arg_type!(ctx, 1, ArgType::String);
  enforce_method_arg_type!(ctx, 2, ArgType::Number);

  let mut ptr = ctx.args[0].as_ptr_cell().borrow_mut();
  let udp = ptr.downcast_mut::<ZuriUdp>().unwrap();

  let addr = ctx.args[1]
    .as_str()
    .parse::<Ipv6Addr>()
    .map_err(|e| e.to_string())?;

  udp.leave_multicast_v6(&addr, ctx.args[2].as_number() as u32)?;

  Ok(Value::nil())
}

fn udp_close(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(UDP_STREAM));

  let mut ptr = ctx.args[0].as_ptr_cell().borrow_mut();
  let udp = ptr.downcast_mut::<ZuriUdp>().unwrap();

  udp.socket = None;

  // Make pointer invalid so that the stream cannot be reused.
  ptr.type_name = "zuri::net::UdpStream::__closed__";

  Ok(Value::nil())
}
