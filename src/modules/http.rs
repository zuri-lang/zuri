//! `_http` builtin module: high-performance native HTTP request head parsing.
//!
//! Powered by `httparse` with SIMD-accelerated zero-copy parsing of HTTP/1.x
//! request lines and headers directly from socket byte buffers.

use smallvec::{SmallVec, smallvec};
use std::borrow::Cow;
use std::sync::LazyLock;

use crate::builtins::enforce::ArgType;
use crate::modules::{BuiltinModuleDef, native};
use crate::vm::object::ZuriContext;
use crate::vm::value::Value;
use crate::vm::vm::VM;
use crate::{enforce_arg_count, enforce_arg_type, enforce_arg_type_any_of};

pub static MODULE: BuiltinModuleDef = BuiltinModuleDef {
  name: "_http",
  build,
};

fn build(vm: &mut VM) -> Vec<(&'static str, Value)> {
  vec![
    (
      "parse_request_head",
      native(vm, "parse_request_head", 5, false, parse_request_head_fn),
    ),
    (
      "is_valid_field_name",
      native(vm, "is_valid_field_name", 1, false, is_valid_field_name_fn),
    ),
    (
      "is_valid_field_value",
      native(
        vm,
        "is_valid_field_value",
        1,
        false,
        is_valid_field_value_fn,
      ),
    ),
    (
      "append_head",
      native(vm, "append_head", 6, false, append_head_fn),
    ),
    (
      "huffman_decode",
      native(vm, "huffman_decode", 3, false, huffman_decode_fn),
    ),
    (
      "huffman_encode",
      native(vm, "huffman_encode", 1, false, huffman_encode_fn),
    ),
    (
      "huffman_encoded_len",
      native(vm, "huffman_encoded_len", 1, false, huffman_encoded_len_fn),
    ),
    (
      "hpack_decode_integer",
      native(
        vm,
        "hpack_decode_integer",
        3,
        false,
        hpack_decode_integer_fn,
      ),
    ),
    (
      "hpack_encode_integer",
      native(
        vm,
        "hpack_encode_integer",
        4,
        false,
        hpack_encode_integer_fn,
      ),
    ),
    (
      "hpack_decode_string",
      native(vm, "hpack_decode_string", 3, false, hpack_decode_string_fn),
    ),
    (
      "hpack_encode_string",
      native(vm, "hpack_encode_string", 2, false, hpack_encode_string_fn),
    ),
    (
      "hpack_encode_block",
      native(vm, "hpack_encode_block", 1, false, hpack_encode_block_fn),
    ),
    (
      "hpack_decode_block",
      native(vm, "hpack_decode_block", 6, false, hpack_decode_block_fn),
    ),
  ]
}

#[inline]
fn is_singleton(name: &str) -> bool {
  name.eq_ignore_ascii_case("content-length")
    || name.eq_ignore_ascii_case("content-type")
    || name.eq_ignore_ascii_case("host")
    || name.eq_ignore_ascii_case("authorization")
    || name.eq_ignore_ascii_case("proxy-authorization")
    || name.eq_ignore_ascii_case("from")
    || name.eq_ignore_ascii_case("location")
    || name.eq_ignore_ascii_case("max-forwards")
    || name.eq_ignore_ascii_case("referer")
    || name.eq_ignore_ascii_case("user-agent")
    || name.eq_ignore_ascii_case("if-modified-since")
    || name.eq_ignore_ascii_case("if-unmodified-since")
    || name.eq_ignore_ascii_case("age")
    || name.eq_ignore_ascii_case("expires")
    || name.eq_ignore_ascii_case("date")
    || name.eq_ignore_ascii_case("retry-after")
    || name.eq_ignore_ascii_case("last-modified")
    || name.eq_ignore_ascii_case("server")
}

#[inline]
fn canonical_lower(name: &str) -> Cow<'_, str> {
  if name.eq_ignore_ascii_case("host") {
    Cow::Borrowed("host")
  } else if name.eq_ignore_ascii_case("user-agent") {
    Cow::Borrowed("user-agent")
  } else if name.eq_ignore_ascii_case("accept") {
    Cow::Borrowed("accept")
  } else if name.eq_ignore_ascii_case("content-length") {
    Cow::Borrowed("content-length")
  } else if name.eq_ignore_ascii_case("content-type") {
    Cow::Borrowed("content-type")
  } else if name.eq_ignore_ascii_case("connection") {
    Cow::Borrowed("connection")
  } else if name.eq_ignore_ascii_case("accept-encoding") {
    Cow::Borrowed("accept-encoding")
  } else if name.eq_ignore_ascii_case("authorization") {
    Cow::Borrowed("authorization")
  } else if name.eq_ignore_ascii_case("cookie") {
    Cow::Borrowed("cookie")
  } else if name.bytes().any(|b| b.is_ascii_uppercase()) {
    Cow::Owned(name.to_ascii_lowercase())
  } else {
    Cow::Borrowed(name)
  }
}

struct HeaderEntry<'a> {
  orig: &'a str,
  values: SmallVec<[&'a str; 2]>,
}

fn parse_data(
  ctx: &mut ZuriContext,
  data: &[u8],
  pos: usize,
  max_line: usize,
  max_header_size: usize,
  max_header_count: usize,
) -> Result<Value, String> {
  if pos >= data.len() {
    return Ok(Value::nil());
  }

  let raw = &data[pos..];

  const STACK_HEADERS: usize = 128;
  let mut stack_headers = [httparse::EMPTY_HEADER; STACK_HEADERS];
  let mut heap_headers;
  let headers_slice: &mut [httparse::Header] = if max_header_count <= STACK_HEADERS {
    &mut stack_headers[..max_header_count]
  } else {
    heap_headers = vec![httparse::EMPTY_HEADER; max_header_count];
    &mut heap_headers[..]
  };

  let mut req = httparse::Request::new(headers_slice);
  let status = match req.parse(raw) {
    Ok(s) => s,
    Err(httparse::Error::TooManyHeaders) => {
      let err_tag = ctx.heap().intern_str("error");
      let msg = ctx
        .heap()
        .alloc_string(format!("more than {max_header_count} header fields"));
      return Ok(ctx.heap().alloc_list(vec![err_tag, msg]));
    },
    Err(err) => {
      let err_tag = ctx.heap().intern_str("error");
      let msg = ctx
        .heap()
        .alloc_string(format!("malformed HTTP request: {err}"));
      return Ok(ctx.heap().alloc_list(vec![err_tag, msg]));
    },
  };

  if status.is_partial() {
    if let Some(p) = raw.iter().position(|&b| b == b'\n') {
      let line_len = if p > 0 && raw[p - 1] == b'\r' {
        p - 1
      } else {
        p
      };
      if line_len > max_line {
        let err_tag = ctx.heap().intern_str("error");
        let msg = ctx
          .heap()
          .alloc_string(format!("line exceeds the {max_line} byte limit"));
        return Ok(ctx.heap().alloc_list(vec![err_tag, msg]));
      }
    } else if raw.len() > max_line {
      let err_tag = ctx.heap().intern_str("error");
      let msg = ctx
        .heap()
        .alloc_string(format!("line exceeds the {max_line} byte limit"));
      return Ok(ctx.heap().alloc_list(vec![err_tag, msg]));
    }
    if raw.len() > max_line + max_header_size {
      let err_tag = ctx.heap().intern_str("error");
      let msg = ctx
        .heap()
        .alloc_string(format!("header section exceeds {max_header_size} bytes"));
      return Ok(ctx.heap().alloc_list(vec![err_tag, msg]));
    }
    return Ok(Value::nil());
  }

  let amt = status.unwrap();

  let first_lf = raw[..amt].iter().position(|&b| b == b'\n').unwrap();
  let req_line_len = if first_lf > 0 && raw[first_lf - 1] == b'\r' {
    first_lf - 1
  } else {
    first_lf
  };
  if req_line_len > max_line {
    let err_tag = ctx.heap().intern_str("error");
    let msg = ctx
      .heap()
      .alloc_string(format!("line exceeds the {max_line} byte limit"));
    return Ok(ctx.heap().alloc_list(vec![err_tag, msg]));
  }

  let headers_raw_len = amt - (first_lf + 1);
  if headers_raw_len > max_header_size {
    let err_tag = ctx.heap().intern_str("error");
    let msg = ctx
      .heap()
      .alloc_string(format!("header section exceeds {max_header_size} bytes"));
    return Ok(ctx.heap().alloc_list(vec![err_tag, msg]));
  }

  let method = req.method.unwrap();
  let target = req.path.unwrap();
  let version = match req.version.unwrap() {
    1 => "1.1",
    0 => "1.0",
    _ => {
      let err_tag = ctx.heap().intern_str("error");
      let msg = ctx.heap().alloc_string("unsupported HTTP version");
      return Ok(ctx.heap().alloc_list(vec![err_tag, msg]));
    },
  };

  let mut host_count = 0;
  let mut authority: Option<&str> = None;
  let mut cookie: Option<String> = None;
  let mut wire_size: usize = 0;

  let mut headers: SmallVec<[HeaderEntry; 8]> = SmallVec::new();

  for h in req.headers.iter() {
    let name = h.name;
    let val_str = match std::str::from_utf8(h.value) {
      Ok(s) => s,
      Err(_) => {
        let err_tag = ctx.heap().intern_str("error");
        let msg = ctx.heap().alloc_string("invalid header field value");
        return Ok(ctx.heap().alloc_list(vec![err_tag, msg]));
      },
    };

    if name.eq_ignore_ascii_case("host") {
      host_count += 1;
      if authority.is_none() {
        authority = Some(val_str);
      }
      if host_count > 1 {
        let err_tag = ctx.heap().intern_str("error");
        let msg = ctx
          .heap()
          .alloc_string("request has more than one Host header");
        return Ok(ctx.heap().alloc_list(vec![err_tag, msg]));
      }
    } else if name.eq_ignore_ascii_case("cookie") {
      match &mut cookie {
        None => cookie = Some(val_str.to_string()),
        Some(existing) => {
          existing.push_str("; ");
          existing.push_str(val_str);
        },
      }
    }

    if is_singleton(name) {
      if let Some(existing) = headers.iter().find(|e| e.orig.eq_ignore_ascii_case(name)) {
        if let Some(&first_val) = existing.values.first() {
          if first_val != val_str {
            let err_tag = ctx.heap().intern_str("error");
            let msg = ctx
              .heap()
              .alloc_string(format!("conflicting duplicate {name} header"));
            return Ok(ctx.heap().alloc_list(vec![err_tag, msg]));
          }
          continue;
        }
      }
    }

    wire_size += name.len() + val_str.len() + 4;

    if let Some(existing) = headers
      .iter_mut()
      .find(|e| e.orig.eq_ignore_ascii_case(name))
    {
      existing.values.push(val_str);
    } else {
      headers.push(HeaderEntry {
        orig: name,
        values: smallvec![val_str],
      });
    }
  }

  if version == "1.1" && host_count == 0 {
    let err_tag = ctx.heap().intern_str("error");
    let msg = ctx
      .heap()
      .alloc_string("HTTP/1.1 request has no Host header");
    return Ok(ctx.heap().alloc_list(vec![err_tag, msg]));
  }
  if host_count > 1 {
    let err_tag = ctx.heap().intern_str("error");
    let msg = ctx
      .heap()
      .alloc_string("request has more than one Host header");
    return Ok(ctx.heap().alloc_list(vec![err_tag, msg]));
  }

  let method_val = ctx.heap().intern_str(method);
  let target_val = ctx.heap().alloc_string(target);
  let version_val = ctx.heap().intern_str(version);

  let mut pairs = Vec::with_capacity(headers.len());
  for item in headers {
    let lower_str = canonical_lower(item.orig);
    let k_val = ctx.heap().intern_str(lower_str.as_ref());
    let orig_val = ctx.heap().intern_str(item.orig);

    let val_list = if item.values.len() == 1 {
      let v_val = ctx.heap().alloc_string(item.values[0]);
      ctx.heap().alloc_list(vec![v_val])
    } else {
      let mut val_items = Vec::with_capacity(item.values.len());
      for v in item.values {
        val_items.push(ctx.heap().alloc_string(v));
      }
      ctx.heap().alloc_list(val_items)
    };

    let entry = ctx.heap().alloc_list(vec![orig_val, val_list]);
    pairs.push((k_val, entry));
  }

  let fields_dict = ctx.heap().alloc_dict(pairs);

  let authority_val = match authority {
    Some(s) => ctx.heap().intern_str(s),
    None => Value::nil(),
  };

  let cookie_val = match cookie {
    Some(s) => ctx.heap().alloc_string(s),
    None => Value::nil(),
  };

  let result_list = ctx.heap().alloc_list(vec![
    method_val,
    target_val,
    version_val,
    fields_dict,
    Value::number(wire_size as f64),
    Value::number(amt as f64),
    authority_val,
    cookie_val,
  ]);

  Ok(result_list)
}

fn parse_request_head_fn(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 5);
  enforce_arg_type_any_of!(ctx, 0, [ArgType::Bytes, ArgType::String]);
  enforce_arg_type!(ctx, 1, ArgType::Number);
  enforce_arg_type!(ctx, 2, ArgType::Number);
  enforce_arg_type!(ctx, 3, ArgType::Number);
  enforce_arg_type!(ctx, 4, ArgType::Number);

  let pos = ctx.args[1].as_number() as usize;
  let max_line = ctx.args[2].as_number() as usize;
  let max_header_size = ctx.args[3].as_number() as usize;
  let max_header_count = ctx.args[4].as_number() as usize;

  let val = ctx.args[0];
  if val.is_bytes() {
    val.with_bytes(|data| parse_data(ctx, data, pos, max_line, max_header_size, max_header_count))
  } else {
    let s = val.as_str().to_string();
    parse_data(
      ctx,
      s.as_bytes(),
      pos,
      max_line,
      max_header_size,
      max_header_count,
    )
  }
}

/// A `tchar` of RFC 9110 §5.6.2, which is what a field name is made of.
fn is_token_byte(byte: u8) -> bool {
  byte.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&byte)
}

fn is_valid_field_name_fn(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 1);
  enforce_arg_type!(ctx, 0, ArgType::String);

  let name = ctx.args[0].as_str().as_bytes();

  Ok(Value::bool(
    !name.is_empty() && name.iter().all(|&byte| is_token_byte(byte)),
  ))
}

fn is_valid_field_value_fn(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 1);
  enforce_arg_type!(ctx, 0, ArgType::String);

  let value = ctx.args[0].as_str().as_bytes();

  Ok(Value::bool(
    !value.iter().any(|byte| matches!(byte, b'\r' | b'\n' | 0)),
  ))
}

fn append_field(out: &mut Vec<u8>, name: &str, value: Value) {
  out.extend_from_slice(name.as_bytes());
  out.extend_from_slice(b": ");

  if value.is_string() {
    out.extend_from_slice(value.as_str().as_bytes());
  } else {
    out.extend_from_slice(format!("{value}").as_bytes());
  }

  out.extend_from_slice(b"\r\n");
}

/// Appends a whole response head to a buffer: the status line, the
/// fields of a `Headers`, one `Set-Cookie` per cookie, and the blank
/// line that ends it.
fn append_head_fn(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 6);
  enforce_arg_type!(ctx, 0, ArgType::Bytes);
  enforce_arg_type!(ctx, 1, ArgType::String);
  enforce_arg_type!(ctx, 2, ArgType::Number);
  enforce_arg_type!(ctx, 3, ArgType::String);
  enforce_arg_type!(ctx, 4, ArgType::Dict);
  enforce_arg_type!(ctx, 5, ArgType::List);

  let version = ctx.args[1].as_str();
  let status = ctx.args[2].as_number() as u32;
  let reason = ctx.args[3].as_str();
  let fields = ctx.args[4];
  let cookies = ctx.args[5];

  ctx.args[0].with_bytes_mut(|out| {
    out.extend_from_slice(b"HTTP/");
    out.extend_from_slice(version.as_bytes());
    out.extend_from_slice(format!(" {status} ").as_bytes());
    out.extend_from_slice(reason.as_bytes());
    out.extend_from_slice(b"\r\n");

    // Each field is stored as `[name as spelled, [values]]`.
    fields.with_dict(|storage| {
      for (_, entry) in &storage.entries {
        entry.with_list(|field| {
          let name = field[0].as_str();

          field[1].with_list(|values| {
            for value in values {
              append_field(out, name, *value);
            }
          });
        });
      }
    });

    cookies.with_list(|items| {
      for cookie in items {
        append_field(out, "Set-Cookie", *cookie);
      }
    });

    out.extend_from_slice(b"\r\n");
  });

  Ok(Value::nil())
}

// ---------------------------------------------------------------------------
// HTTP/2 HPACK & Huffman Native Acceleration (RFC 7541)
// ---------------------------------------------------------------------------

const HUFFMAN_CODE_LENGTHS: [u8; 257] = [
  13, 23, 28, 28, 28, 28, 28, 28, 28, 24, 30, 28, 28, 30, 28, 28, 28, 28, 28, 28, 28, 28, 30, 28,
  28, 28, 28, 28, 28, 28, 28, 28, 6, 10, 10, 12, 13, 6, 8, 11, 10, 10, 8, 11, 8, 6, 6, 6, 5, 5, 5,
  6, 6, 6, 6, 6, 6, 6, 7, 8, 15, 6, 12, 10, 13, 6, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7,
  7, 7, 7, 7, 7, 7, 8, 7, 8, 13, 19, 13, 14, 6, 15, 5, 6, 5, 6, 5, 6, 6, 6, 5, 7, 7, 6, 6, 6, 5, 6,
  7, 6, 5, 5, 6, 7, 7, 7, 7, 7, 15, 11, 14, 13, 28, 20, 22, 20, 20, 22, 22, 22, 23, 22, 23, 23, 23,
  23, 23, 24, 23, 24, 24, 22, 23, 24, 23, 23, 23, 23, 21, 22, 23, 22, 23, 23, 24, 22, 21, 20, 22,
  22, 23, 23, 21, 23, 22, 22, 24, 21, 22, 23, 23, 21, 21, 22, 21, 23, 22, 23, 23, 20, 22, 22, 22,
  23, 22, 22, 23, 26, 26, 20, 19, 22, 23, 22, 25, 26, 26, 26, 27, 27, 26, 24, 25, 19, 21, 26, 27,
  27, 26, 27, 24, 21, 21, 26, 26, 28, 27, 27, 27, 20, 24, 20, 21, 22, 21, 21, 23, 22, 22, 25, 25,
  24, 24, 26, 23, 26, 27, 26, 26, 27, 27, 27, 27, 27, 28, 27, 27, 27, 27, 27, 26, 30,
];

#[derive(Clone, Copy)]
struct HuffmanNode {
  left: i16,
  right: i16,
  symbol: i16,
}

struct HuffmanTable {
  codes: [(u32, u8); 257],
  nodes: Vec<HuffmanNode>,
}

fn build_huffman_table() -> HuffmanTable {
  let mut codes = [(0u32, 0u8); 257];
  let mut by_length: [Vec<usize>; 31] = Default::default();

  for symbol in 0..257 {
    let len = HUFFMAN_CODE_LENGTHS[symbol] as usize;
    by_length[len].push(symbol);
  }

  let mut code = 0u32;
  let mut previous = 0usize;

  for len in 1..=30 {
    let symbols = &by_length[len];
    if symbols.is_empty() {
      continue;
    }
    code <<= len - previous;
    previous = len;
    for &sym in symbols {
      codes[sym] = (code, len as u8);
      code += 1;
    }
  }

  let mut nodes = Vec::with_capacity(514);
  nodes.push(HuffmanNode {
    left: -1,
    right: -1,
    symbol: -1,
  });

  for symbol in 0..257 {
    let (code_val, len) = codes[symbol];
    let mut node_idx = 0usize;

    for bit_idx in (0..len).rev() {
      let bit = (code_val >> bit_idx) & 1;
      let next = if bit == 1 {
        nodes[node_idx].right
      } else {
        nodes[node_idx].left
      };

      let child_idx = if next == -1 {
        let new_idx = nodes.len() as i16;
        nodes.push(HuffmanNode {
          left: -1,
          right: -1,
          symbol: -1,
        });
        if bit == 1 {
          nodes[node_idx].right = new_idx;
        } else {
          nodes[node_idx].left = new_idx;
        }
        new_idx as usize
      } else {
        next as usize
      };

      node_idx = child_idx;
    }

    nodes[node_idx].symbol = symbol as i16;
  }

  HuffmanTable { codes, nodes }
}

static HUFFMAN: LazyLock<HuffmanTable> = LazyLock::new(build_huffman_table);

fn huffman_decode(data: &[u8]) -> Result<Vec<u8>, u8> {
  let table = &*HUFFMAN;
  let mut out = Vec::with_capacity(data.len());
  let mut node_idx = 0usize;
  let mut bits_since_leaf = 0usize;
  let mut all_ones = true;

  for &byte in data {
    for m in (0..8).rev() {
      let bit = (byte >> m) & 1;
      let next = if bit == 1 {
        table.nodes[node_idx].right
      } else {
        all_ones = false;
        table.nodes[node_idx].left
      };

      if next == -1 {
        return Err(1);
      }

      node_idx = next as usize;
      bits_since_leaf += 1;

      let sym = table.nodes[node_idx].symbol;
      if sym >= 0 {
        if sym == 256 {
          return Err(2);
        }
        out.push(sym as u8);
        node_idx = 0;
        bits_since_leaf = 0;
        all_ones = true;
      }
    }
  }

  if bits_since_leaf > 7 || !all_ones {
    return Err(3);
  }

  Ok(out)
}

fn huffman_encode(data: &[u8]) -> Vec<u8> {
  let table = &*HUFFMAN;
  let mut out = Vec::with_capacity(data.len());
  let mut accumulator = 0u64;
  let mut bits = 0u8;

  for &byte in data {
    let (code, len) = table.codes[byte as usize];
    accumulator = (accumulator << len) | (code as u64);
    bits += len;

    while bits >= 8 {
      bits -= 8;
      out.push((accumulator >> bits) as u8);
    }
  }

  if bits > 0 {
    let padding = 8 - bits;
    let padded = (accumulator << padding) | ((1u64 << padding) - 1);
    out.push(padded as u8);
  }

  out
}

#[inline]
fn huffman_encoded_len(data: &[u8]) -> usize {
  let mut bits = 0usize;
  for &byte in data {
    bits += HUFFMAN_CODE_LENGTHS[byte as usize] as usize;
  }
  (bits + 7) / 8
}

fn encode_integer(out: &mut Vec<u8>, value: usize, prefix_bits: usize, first: u8) {
  let limit = (1usize << prefix_bits) - 1;
  if value < limit {
    out.push(first | (value as u8));
    return;
  }
  out.push(first | (limit as u8));
  let mut rem = value - limit;
  while rem >= 128 {
    out.push(((rem % 128) as u8) + 128);
    rem /= 128;
  }
  out.push(rem as u8);
}

fn decode_integer(
  data: &[u8],
  offset: usize,
  prefix_bits: usize,
) -> Result<(usize, usize), &'static str> {
  if offset >= data.len() {
    return Err("truncated HPACK integer");
  }
  let limit = (1usize << prefix_bits) - 1;
  let mut value = (data[offset] as usize) & limit;
  let mut at = offset + 1;
  if value < limit {
    return Ok((value, at));
  }
  let mut shift = 0;
  loop {
    if at >= data.len() {
      return Err("truncated HPACK integer");
    }
    let byte = data[at] as usize;
    at += 1;
    value += (byte & 127) << shift;
    shift += 7;
    if byte < 128 {
      break;
    }
    if shift > 28 {
      return Err("HPACK integer is too large");
    }
  }
  Ok((value, at))
}

fn encode_string(out: &mut Vec<u8>, text: &[u8]) {
  let enc_len = huffman_encoded_len(text);
  if enc_len < text.len() {
    let encoded = huffman_encode(text);
    encode_integer(out, encoded.len(), 7, 128);
    out.extend_from_slice(&encoded);
  } else {
    encode_integer(out, text.len(), 7, 0);
    out.extend_from_slice(text);
  }
}

fn decode_string(data: &[u8], offset: usize, max_length: usize) -> Result<(String, usize), String> {
  if offset >= data.len() {
    return Err("truncated HPACK string".to_string());
  }
  let is_huffman = (data[offset] & 128) != 0;
  let (length, start) = match decode_integer(data, offset, 7) {
    Ok(v) => v,
    Err(e) => return Err(e.to_string()),
  };
  if length > max_length {
    return Err(format!("HPACK string of {length} bytes exceeds the limit"));
  }
  if start + length > data.len() {
    return Err("truncated HPACK string".to_string());
  }
  let text = if is_huffman {
    let decoded = match huffman_decode(&data[start..start + length]) {
      Ok(bytes) => bytes,
      Err(1) => return Err("invalid Huffman code in a header block".to_string()),
      Err(2) => return Err("EOS appears in a Huffman-encoded header".to_string()),
      Err(_) => return Err("invalid Huffman padding in a header block".to_string()),
    };
    match String::from_utf8(decoded) {
      Ok(s) => s,
      Err(e) => String::from_utf8_lossy(e.as_bytes()).into_owned(),
    }
  } else {
    let slice = &data[start..start + length];
    match std::str::from_utf8(slice) {
      Ok(s) => s.to_string(),
      Err(_) => String::from_utf8_lossy(slice).into_owned(),
    }
  };
  Ok((text, start + length))
}

static STATIC_TABLE: [(&str, &str); 62] = [
  ("", ""),
  (":authority", ""),
  (":method", "GET"),
  (":method", "POST"),
  (":path", "/"),
  (":path", "/index.html"),
  (":scheme", "http"),
  (":scheme", "https"),
  (":status", "200"),
  (":status", "204"),
  (":status", "206"),
  (":status", "304"),
  (":status", "400"),
  (":status", "404"),
  (":status", "500"),
  ("accept-charset", ""),
  ("accept-encoding", "gzip, deflate"),
  ("accept-language", ""),
  ("accept-ranges", ""),
  ("accept", ""),
  ("access-control-allow-origin", ""),
  ("age", ""),
  ("allow", ""),
  ("authorization", ""),
  ("cache-control", ""),
  ("content-disposition", ""),
  ("content-encoding", ""),
  ("content-language", ""),
  ("content-length", ""),
  ("content-location", ""),
  ("content-range", ""),
  ("content-type", ""),
  ("cookie", ""),
  ("date", ""),
  ("etag", ""),
  ("expect", ""),
  ("expires", ""),
  ("from", ""),
  ("host", ""),
  ("if-match", ""),
  ("if-modified-since", ""),
  ("if-none-match", ""),
  ("if-range", ""),
  ("if-unmodified-since", ""),
  ("last-modified", ""),
  ("link", ""),
  ("location", ""),
  ("max-forwards", ""),
  ("proxy-authenticate", ""),
  ("proxy-authorization", ""),
  ("range", ""),
  ("referer", ""),
  ("refresh", ""),
  ("retry-after", ""),
  ("server", ""),
  ("set-cookie", ""),
  ("strict-transport-security", ""),
  ("transfer-encoding", ""),
  ("user-agent", ""),
  ("vary", ""),
  ("via", ""),
  ("www-authenticate", ""),
];

fn static_pair_lookup(name: &str, value: &str) -> Option<usize> {
  match name {
    ":method" => match value {
      "GET" => Some(2),
      "POST" => Some(3),
      _ => None,
    },
    ":path" => match value {
      "/" => Some(4),
      "/index.html" => Some(5),
      _ => None,
    },
    ":scheme" => match value {
      "http" => Some(6),
      "https" => Some(7),
      _ => None,
    },
    ":status" => match value {
      "200" => Some(8),
      "204" => Some(9),
      "206" => Some(10),
      "304" => Some(11),
      "400" => Some(12),
      "404" => Some(13),
      "500" => Some(14),
      _ => None,
    },
    "accept-encoding" => match value {
      "gzip, deflate" => Some(16),
      _ => None,
    },
    _ => None,
  }
}

fn static_name_lookup(name: &str) -> usize {
  match name {
    ":authority" => 1,
    ":method" => 2,
    ":path" => 4,
    ":scheme" => 6,
    ":status" => 8,
    "accept-charset" => 15,
    "accept-encoding" => 16,
    "accept-language" => 17,
    "accept-ranges" => 18,
    "accept" => 19,
    "access-control-allow-origin" => 20,
    "age" => 21,
    "allow" => 22,
    "authorization" => 23,
    "cache-control" => 24,
    "content-disposition" => 25,
    "content-encoding" => 26,
    "content-language" => 27,
    "content-length" => 28,
    "content-location" => 29,
    "content-range" => 30,
    "content-type" => 31,
    "cookie" => 32,
    "date" => 33,
    "etag" => 34,
    "expect" => 35,
    "expires" => 36,
    "from" => 37,
    "host" => 38,
    "if-match" => 39,
    "if-modified-since" => 40,
    "if-none-match" => 41,
    "if-range" => 42,
    "if-unmodified-since" => 43,
    "last-modified" => 44,
    "link" => 45,
    "location" => 46,
    "max-forwards" => 47,
    "proxy-authenticate" => 48,
    "proxy-authorization" => 49,
    "range" => 50,
    "referer" => 51,
    "refresh" => 52,
    "retry-after" => 53,
    "server" => 54,
    "set-cookie" => 55,
    "strict-transport-security" => 56,
    "transfer-encoding" => 57,
    "user-agent" => 58,
    "vary" => 59,
    "via" => 60,
    "www-authenticate" => 61,
    _ => 0,
  }
}

#[inline]
fn is_never_indexed(name: &str) -> bool {
  name == "authorization"
    || name == "proxy-authorization"
    || name == "cookie"
    || name == "set-cookie"
}

#[inline]
fn entry_size(name: &str, value: &str) -> usize {
  name.len() + value.len() + 32
}

fn check_name(name: &str) -> Result<(), String> {
  if name.is_empty() {
    return Err("empty header field name".to_string());
  }
  for (i, &b) in name.as_bytes().iter().enumerate() {
    if b.is_ascii_uppercase() {
      return Err(format!("header field name {name} is not lowercase"));
    }
    if b <= 32 || (b == b':' && i > 0) || b == 127 {
      return Err(format!("invalid character in header field name: {name}"));
    }
  }
  Ok(())
}

fn make_hpack_error(ctx: &mut ZuriContext, msg: &str) -> Value {
  let err_tag = ctx.heap().intern_str("error");
  let msg_val = ctx.heap().alloc_string(msg);
  ctx.heap().alloc_list(vec![err_tag, msg_val])
}

fn huffman_decode_fn(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 3);
  enforce_arg_type_any_of!(ctx, 0, [ArgType::Bytes, ArgType::String]);
  enforce_arg_type!(ctx, 1, ArgType::Number);
  enforce_arg_type!(ctx, 2, ArgType::Number);

  let start = ctx.args[1].as_number() as usize;
  let end = ctx.args[2].as_number() as usize;

  let val = ctx.args[0];
  let res = if val.is_bytes() {
    val.with_bytes(|data| {
      if start > end || end > data.len() {
        return Err(3u8);
      }
      huffman_decode(&data[start..end])
    })
  } else {
    let s = val.as_str();
    let data = s.as_bytes();
    if start > end || end > data.len() {
      return Ok(Value::number(3.0));
    }
    huffman_decode(&data[start..end])
  };

  match res {
    Ok(decoded) => Ok(ctx.heap().alloc_bytes(decoded)),
    Err(code) => Ok(Value::number(code as f64)),
  }
}

fn huffman_encode_fn(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 1);
  enforce_arg_type_any_of!(ctx, 0, [ArgType::Bytes, ArgType::String]);

  let val = ctx.args[0];
  let encoded = if val.is_bytes() {
    val.with_bytes(|data| huffman_encode(data))
  } else {
    huffman_encode(val.as_str().as_bytes())
  };

  Ok(ctx.heap().alloc_bytes(encoded))
}

fn huffman_encoded_len_fn(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 1);
  enforce_arg_type_any_of!(ctx, 0, [ArgType::Bytes, ArgType::String]);

  let val = ctx.args[0];
  let len = if val.is_bytes() {
    val.with_bytes(|data| huffman_encoded_len(data))
  } else {
    huffman_encoded_len(val.as_str().as_bytes())
  };

  Ok(Value::number(len as f64))
}

fn hpack_decode_integer_fn(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 3);
  enforce_arg_type_any_of!(ctx, 0, [ArgType::Bytes, ArgType::String]);
  enforce_arg_type!(ctx, 1, ArgType::Number);
  enforce_arg_type!(ctx, 2, ArgType::Number);

  let offset = ctx.args[1].as_number() as usize;
  let prefix_bits = ctx.args[2].as_number() as usize;

  let val = ctx.args[0];
  let res = if val.is_bytes() {
    val.with_bytes(|data| decode_integer(data, offset, prefix_bits))
  } else {
    decode_integer(val.as_str().as_bytes(), offset, prefix_bits)
  };

  match res {
    Ok((value, next_at)) => {
      let v_val = Value::number(value as f64);
      let at_val = Value::number(next_at as f64);
      Ok(ctx.heap().alloc_list(vec![v_val, at_val]))
    },
    Err("truncated HPACK integer") => Ok(Value::number(1.0)),
    Err(_) => Ok(Value::number(2.0)),
  }
}

fn hpack_encode_integer_fn(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 4);
  enforce_arg_type!(ctx, 0, ArgType::Bytes);
  enforce_arg_type!(ctx, 1, ArgType::Number);
  enforce_arg_type!(ctx, 2, ArgType::Number);
  enforce_arg_type!(ctx, 3, ArgType::Number);

  let value = ctx.args[1].as_number() as usize;
  let prefix_bits = ctx.args[2].as_number() as usize;
  let first = ctx.args[3].as_number() as u8;

  ctx.args[0].with_bytes_mut(|out| {
    encode_integer(out, value, prefix_bits, first);
  });

  Ok(Value::nil())
}

fn hpack_decode_string_fn(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 3);
  enforce_arg_type_any_of!(ctx, 0, [ArgType::Bytes, ArgType::String]);
  enforce_arg_type!(ctx, 1, ArgType::Number);
  enforce_arg_type!(ctx, 2, ArgType::Number);

  let offset = ctx.args[1].as_number() as usize;
  let max_len = ctx.args[2].as_number() as usize;

  let val = ctx.args[0];
  let res = if val.is_bytes() {
    val.with_bytes(|data| decode_string(data, offset, max_len))
  } else {
    decode_string(val.as_str().as_bytes(), offset, max_len)
  };

  match res {
    Ok((text, next_at)) => {
      let s_val = ctx.heap().alloc_string(text);
      let at_val = Value::number(next_at as f64);
      Ok(ctx.heap().alloc_list(vec![s_val, at_val]))
    },
    Err(err_msg) => Ok(ctx.heap().alloc_string(err_msg)),
  }
}

fn hpack_encode_string_fn(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 2);
  enforce_arg_type!(ctx, 0, ArgType::Bytes);
  enforce_arg_type_any_of!(ctx, 1, [ArgType::Bytes, ArgType::String]);

  let text_val = ctx.args[1];
  if text_val.is_bytes() {
    text_val.with_bytes(|raw| {
      ctx.args[0].with_bytes_mut(|out| {
        encode_string(out, raw);
      });
    });
  } else {
    let s = text_val.as_str();
    ctx.args[0].with_bytes_mut(|out| {
      encode_string(out, s.as_bytes());
    });
  }

  Ok(Value::nil())
}

fn hpack_encode_block_fn(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 1);
  enforce_arg_type!(ctx, 0, ArgType::List);

  let fields_val = ctx.args[0];
  let mut out = Vec::with_capacity(256);

  let fields_list = fields_val.as_list();
  for field in fields_list {
    if !field.is_list() {
      continue;
    }
    let pair = field.as_list();
    if pair.len() < 2 {
      continue;
    }
    let name_str = pair[0].as_str();
    let val_cow = if pair[1].is_string() {
      Cow::Borrowed(pair[1].as_str())
    } else {
      Cow::Owned(format!("{}", pair[1]))
    };

    if let Some(pair_idx) = static_pair_lookup(name_str, val_cow.as_ref()) {
      encode_integer(&mut out, pair_idx, 7, 128);
      continue;
    }

    let sensitive = is_never_indexed(name_str);
    let flag = if sensitive { 16 } else { 0 };
    let name_idx = static_name_lookup(name_str);

    encode_integer(&mut out, name_idx, 4, flag);
    if name_idx == 0 {
      encode_string(&mut out, name_str.as_bytes());
    }
    encode_string(&mut out, val_cow.as_bytes());
  }

  Ok(ctx.heap().alloc_bytes(out))
}

fn decode_block_internal(
  ctx: &mut ZuriContext,
  block: &[u8],
  mut dyn_entries: Vec<(String, String)>,
  mut capacity: usize,
  mut size: usize,
  max_string_len: usize,
  max_header_list_size: usize,
) -> Result<Value, String> {
  let mut at = 0;
  let mut total = 0;
  let mut fields_pairs: Vec<(String, String)> = Vec::new();
  let mut added_entries: Vec<(String, String)> = Vec::new();

  while at < block.len() {
    let byte = block[at];

    if (byte & 128) != 0 {
      let (idx, next_at) = match decode_integer(block, at, 7) {
        Ok(v) => v,
        Err(e) => return Ok(make_hpack_error(ctx, e)),
      };
      if idx < 1 {
        return Ok(make_hpack_error(ctx, "HPACK index 0 is not valid"));
      }
      let (name, val) = if idx <= 61 {
        (
          STATIC_TABLE[idx].0.to_string(),
          STATIC_TABLE[idx].1.to_string(),
        )
      } else {
        let dyn_idx = idx - 62;
        if dyn_idx >= dyn_entries.len() {
          return Ok(make_hpack_error(
            ctx,
            &format!("HPACK index {idx} is past the end of the table"),
          ));
        }
        dyn_entries[dyn_idx].clone()
      };

      total += entry_size(&name, &val);
      fields_pairs.push((name, val));
      at = next_at;
      continue;
    }

    if (byte & 32) != 0 && (byte & 64) == 0 {
      let (new_cap, next_at) = match decode_integer(block, at, 5) {
        Ok(v) => v,
        Err(e) => return Ok(make_hpack_error(ctx, e)),
      };
      if new_cap > capacity && new_cap > 4096 {
        return Ok(make_hpack_error(
          ctx,
          "HPACK table size update exceeds the agreed maximum",
        ));
      }
      capacity = new_cap;
      while size > capacity && !dyn_entries.is_empty() {
        let evicted = dyn_entries.pop().unwrap();
        size -= entry_size(&evicted.0, &evicted.1);
      }
      at = next_at;
      continue;
    }

    let indexed = (byte & 64) != 0;
    let prefix = if indexed { 6 } else { 4 };

    let (name_idx, next_at) = match decode_integer(block, at, prefix) {
      Ok(v) => v,
      Err(e) => return Ok(make_hpack_error(ctx, e)),
    };
    at = next_at;

    let name = if name_idx == 0 {
      let (s, next_at) = match decode_string(block, at, max_string_len) {
        Ok(v) => v,
        Err(e) => return Ok(make_hpack_error(ctx, &e)),
      };
      at = next_at;
      s
    } else if name_idx <= 61 {
      STATIC_TABLE[name_idx].0.to_string()
    } else {
      let dyn_idx = name_idx - 62;
      if dyn_idx >= dyn_entries.len() {
        return Ok(make_hpack_error(
          ctx,
          &format!("HPACK index {name_idx} is past the end of the table"),
        ));
      }
      dyn_entries[dyn_idx].0.clone()
    };

    let (value, next_at) = match decode_string(block, at, max_string_len) {
      Ok(v) => v,
      Err(e) => return Ok(make_hpack_error(ctx, &e)),
    };
    at = next_at;

    if let Err(e) = check_name(&name) {
      return Ok(make_hpack_error(ctx, &e));
    }

    total += entry_size(&name, &value);
    fields_pairs.push((name.clone(), value.clone()));

    if indexed {
      let cost = entry_size(&name, &value);
      while size + cost > capacity && !dyn_entries.is_empty() {
        let evicted = dyn_entries.pop().unwrap();
        size -= entry_size(&evicted.0, &evicted.1);
      }
      if cost <= capacity {
        dyn_entries.insert(0, (name.clone(), value.clone()));
        size += cost;
        added_entries.push((name, value));
      }
    }

    if total > max_header_list_size {
      return Ok(make_hpack_error(
        ctx,
        &format!("decoded header list exceeds {max_header_list_size} bytes"),
      ));
    }
  }

  let ok_tag = ctx.heap().intern_str("ok");
  let mut fields_val_vec = Vec::with_capacity(fields_pairs.len());
  for (n, v) in fields_pairs {
    let n_val = ctx.heap().intern_str(&n);
    let v_val = ctx.heap().alloc_string(v);
    fields_val_vec.push(ctx.heap().alloc_list(vec![n_val, v_val]));
  }
  let fields_list = ctx.heap().alloc_list(fields_val_vec);

  let mut added_val_vec = Vec::with_capacity(added_entries.len());
  for (n, v) in added_entries {
    let n_val = ctx.heap().intern_str(&n);
    let v_val = ctx.heap().alloc_string(v);
    added_val_vec.push(ctx.heap().alloc_list(vec![n_val, v_val]));
  }
  let added_list = ctx.heap().alloc_list(added_val_vec);

  Ok(ctx.heap().alloc_list(vec![
    ok_tag,
    fields_list,
    Value::number(capacity as f64),
    added_list,
  ]))
}

fn hpack_decode_block_fn(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 6);
  enforce_arg_type_any_of!(ctx, 0, [ArgType::Bytes, ArgType::String]);
  enforce_arg_type!(ctx, 1, ArgType::List);
  enforce_arg_type!(ctx, 2, ArgType::Number);
  enforce_arg_type!(ctx, 3, ArgType::Number);
  enforce_arg_type!(ctx, 4, ArgType::Number);
  enforce_arg_type!(ctx, 5, ArgType::Number);

  let block_val = ctx.args[0];
  let dynamic_entries_val = ctx.args[1];
  let capacity = ctx.args[2].as_number() as usize;
  let size = ctx.args[3].as_number() as usize;
  let max_string_len = ctx.args[4].as_number() as usize;
  let max_header_list_size = ctx.args[5].as_number() as usize;

  let mut dyn_entries: Vec<(String, String)> = Vec::new();
  for entry in dynamic_entries_val.as_list() {
    if entry.is_list() {
      let p = entry.as_list();
      if p.len() >= 2 && p[0].is_string() && p[1].is_string() {
        dyn_entries.push((p[0].as_str().to_string(), p[1].as_str().to_string()));
      }
    }
  }

  if block_val.is_bytes() {
    block_val.with_bytes(|data| {
      decode_block_internal(
        ctx,
        data,
        dyn_entries,
        capacity,
        size,
        max_string_len,
        max_header_list_size,
      )
    })
  } else {
    let s = block_val.as_str().to_string();
    decode_block_internal(
      ctx,
      s.as_bytes(),
      dyn_entries,
      capacity,
      size,
      max_string_len,
      max_header_list_size,
    )
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
  }

  #[test]
  fn test_huffman_rfc7541_c4() {
    // C.4.1 www.example.com
    let enc1 = huffman_encode(b"www.example.com");
    assert_eq!(to_hex(&enc1), "f1e3c2e5f23a6ba0ab90f4ff");
    assert_eq!(huffman_decode(&enc1).unwrap(), b"www.example.com");

    // C.4.2 no-cache
    let enc2 = huffman_encode(b"no-cache");
    assert_eq!(to_hex(&enc2), "a8eb10649cbf");
    assert_eq!(huffman_decode(&enc2).unwrap(), b"no-cache");

    // C.4.3 custom-key
    let enc3 = huffman_encode(b"custom-key");
    assert_eq!(to_hex(&enc3), "25a849e95ba97d7f");
    assert_eq!(huffman_decode(&enc3).unwrap(), b"custom-key");

    // C.4.4 custom-value
    let enc4 = huffman_encode(b"custom-value");
    assert_eq!(to_hex(&enc4), "25a849e95bb8e8b4bf");
    assert_eq!(huffman_decode(&enc4).unwrap(), b"custom-value");
  }

  #[test]
  fn test_hpack_integers() {
    let mut out = Vec::new();
    encode_integer(&mut out, 10, 5, 0);
    assert_eq!(out, vec![10]);

    out.clear();
    encode_integer(&mut out, 1337, 5, 0);
    assert_eq!(to_hex(&out), "1f9a0a");
    let (val, next) = decode_integer(&out, 0, 5).unwrap();
    assert_eq!(val, 1337);
    assert_eq!(next, 3);
  }
}
