use zlib_rs::{
  DeflateConfig, Inflate, InflateConfig, InflateError, InflateFlush, ReturnCode, Status, Strategy,
  adler32::adler32, compress_bound, compress_slice, crc32::crc32, decompress_slice,
};

use crate::{
  builtins::enforce::ArgType,
  enforce_arg_count, enforce_arg_range, enforce_arg_type_any_of,
  modules::{BuiltinModuleDef, native, optional_number},
  vm::{object::ZuriContext, value::Value, vm::VM},
};

pub static MODULE: BuiltinModuleDef = BuiltinModuleDef {
  name: "_compress",
  build,
};

/// Re-exports the already globally-registered `sum` native under the
/// `math` namespace -- preserves `import math; math.sum(...)` exactly
/// as it worked before this registry existed.
fn build(vm: &mut VM) -> Vec<(&'static str, Value)> {
  vec![
    ("compress", native(vm, "compress", 1, true, compress)),
    ("uncompress", native(vm, "uncompress", 1, true, uncompress)),
    ("deflate_encode", native(vm, "encode", 1, false, deflate)),
    ("deflate_decode", native(vm, "decode", 1, false, undeflate)),
    ("gzip_encode", native(vm, "encode", 1, false, gzip)),
    ("gzip_decode", native(vm, "decode", 1, false, ungzip)),
    ("checksum_crc32", native(vm, "crc32", 1, true, crc32_fn)),
    (
      "checksum_adler32",
      native(vm, "adler32", 1, true, adler32_fn),
    ),
  ]
}

fn parse_result(code: ReturnCode) -> Result<(), &'static str> {
  match code {
    ReturnCode::Ok => Ok(()),
    ReturnCode::BufError => Err("Buffer error"),
    ReturnCode::StreamError => Err("Stream error"),
    ReturnCode::DataError => Err("Data error"),
    ReturnCode::MemError => Err("Memory error"),
    ReturnCode::VersionError => Err("Version error"),
    ReturnCode::NeedDict => Err("Dictionary needed"),
    ReturnCode::StreamEnd => Err("Stream ended unexpectedly"),
    ReturnCode::ErrNo => Err("Unknown error"),
  }
}

fn parse_error(code: InflateError) -> &'static str {
  match code {
    InflateError::DataError => "Data error",
    InflateError::MemError => "Memory error",
    InflateError::StreamError => "Stream error",
    InflateError::NeedDict { .. } => "Dictionary needed",
  }
}

fn decompress_data(input: &[u8], window_bits: i32) -> Result<Vec<u8>, InflateError> {
  let mut inflate = Inflate::new(
    if window_bits >= 0 { true } else { false },
    window_bits.abs() as u8,
  );

  let mut output = Vec::new();
  let mut buf = vec![0u8; 8192];
  let mut input_pos = 0;

  while input_pos < input.len() {
    let total_in_before = inflate.total_in();
    let total_out_before = inflate.total_out();

    let status = inflate.decompress(&input[input_pos..], &mut buf, InflateFlush::NoFlush)?;

    let consumed = (inflate.total_in() - total_in_before) as usize;
    let produced = (inflate.total_out() - total_out_before) as usize;

    output.extend_from_slice(&buf[..produced]);
    input_pos += consumed;

    match status {
      Status::StreamEnd => return Ok(output),
      Status::BufError if produced == 0 && consumed == 0 => {
        // No forward progress: grow and retry same input
        buf.resize(buf.len().saturating_mul(2), 0);
      },
      Status::BufError => {
        // Made some progress but ran out of output space; grow and continue
        buf.resize(buf.len().saturating_mul(2), 0);
      },
      Status::Ok => {},
    }
  }

  // Flush
  loop {
    let total_out_before = inflate.total_out();
    match inflate.decompress(&[], &mut buf, InflateFlush::Finish)? {
      Status::StreamEnd => {
        let produced = (inflate.total_out() - total_out_before) as usize;
        output.extend_from_slice(&buf[..produced]);
        return Ok(output);
      },
      Status::BufError => buf.resize(buf.len().saturating_mul(2), 0),
      Status::Ok => {
        let produced = (inflate.total_out() - total_out_before) as usize;
        output.extend_from_slice(&buf[..produced]);
      },
    }
  }
}

fn compress(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_range!(ctx, 1, 5);
  enforce_arg_type_any_of!(ctx, 0, [ArgType::String, ArgType::Bytes]);

  let value = ctx.args[0];
  let data = if value.is_string() {
    value.as_str().as_bytes().to_vec()
  } else {
    value.as_bytes().to_vec()
  };

  let default_config = DeflateConfig::default();

  let config = DeflateConfig {
    level: optional_number(ctx, 1, default_config.level as f64)? as i32,
    strategy: Strategy::try_from(optional_number(ctx, 2, Strategy::Default as i32 as f64)? as i32)
      .map_err(|_| "Invalid strategy".to_string())?,
    window_bits: optional_number(ctx, 3, default_config.window_bits as f64)? as i32,
    mem_level: optional_number(ctx, 4, default_config.mem_level as f64)? as i32,
    method: default_config.method,
  };

  let mut output = vec![0u8; compress_bound(data.len())];
  let (compressed_data, rc) = compress_slice(&mut output, &data, config);

  if let Err(msg) = parse_result(rc) {
    return Err(msg.to_string());
  }

  Ok(ctx.heap().alloc_bytes(compressed_data))
}

fn uncompress(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_range!(ctx, 1, 2);
  enforce_arg_type_any_of!(ctx, 0, [ArgType::String, ArgType::Bytes]);

  let value = ctx.args[0];
  let data = if value.is_string() {
    value.as_str().as_bytes().to_vec()
  } else {
    value.as_bytes().to_vec()
  };

  let default_config: InflateConfig = InflateConfig::default();
  let window_bits = optional_number(ctx, 1, default_config.window_bits as f64)? as i32;

  let decompressed_data = decompress_data(&data, window_bits).map_err(|x| parse_error(x))?;

  Ok(ctx.heap().alloc_bytes(decompressed_data))
}

fn deflate(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 1);
  enforce_arg_type_any_of!(ctx, 0, [ArgType::String, ArgType::Bytes]);

  let value = ctx.args[0];
  let data = if value.is_string() {
    value.as_str().as_bytes().to_vec()
  } else {
    value.as_bytes().to_vec()
  };

  let mut config = DeflateConfig::default();
  config.window_bits = -config.window_bits; // raw deflate

  let mut output = vec![0u8; compress_bound(data.len())];
  let (compressed_data, rc) = compress_slice(&mut output, &data, config);

  if let Err(msg) = parse_result(rc) {
    return Err(msg.to_string());
  }

  Ok(ctx.heap().alloc_bytes(compressed_data))
}

fn undeflate(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 1);
  enforce_arg_type_any_of!(ctx, 0, [ArgType::String, ArgType::Bytes]);

  let value = ctx.args[0];
  let data = if value.is_string() {
    value.as_str().as_bytes().to_vec()
  } else {
    value.as_bytes().to_vec()
  };

  let config = InflateConfig::default();

  let decompressed_data =
    decompress_data(&data, -config.window_bits).map_err(|x| parse_error(x))?;

  Ok(ctx.heap().alloc_bytes(decompressed_data))
}

fn gzip(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 1);
  enforce_arg_type_any_of!(ctx, 0, [ArgType::String, ArgType::Bytes]);

  let value = ctx.args[0];
  let data = if value.is_string() {
    value.as_str().as_bytes().to_vec()
  } else {
    value.as_bytes().to_vec()
  };

  let mut config = DeflateConfig::default();
  config.window_bits = config.window_bits | 16; // raw gzip

  let mut output = vec![0u8; compress_bound(data.len())];
  let (compressed_data, rc) = compress_slice(&mut output, &data, config);

  if let Err(msg) = parse_result(rc) {
    return Err(msg.to_string());
  }

  Ok(ctx.heap().alloc_bytes(compressed_data))
}

fn ungzip(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 1);
  enforce_arg_type_any_of!(ctx, 0, [ArgType::String, ArgType::Bytes]);

  let value = ctx.args[0];
  let data = if value.is_string() {
    value.as_str().as_bytes().to_vec()
  } else {
    value.as_bytes().to_vec()
  };

  let mut config = InflateConfig::default();
  config.window_bits = config.window_bits | 16; // raw gzip

  let mut decompressed_data = Vec::new();
  let (_, rc) = decompress_slice(decompressed_data.as_mut_slice(), &data, config);

  if let Err(msg) = parse_result(rc) {
    return Err(msg.to_string());
  }

  Ok(ctx.heap().alloc_bytes(decompressed_data))
}

fn adler32_fn(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_range!(ctx, 1, 2);
  enforce_arg_type_any_of!(ctx, 0, [ArgType::String, ArgType::Bytes]);

  let value = ctx.args[0];
  let data = if value.is_string() {
    value.as_str().as_bytes().to_vec()
  } else {
    value.as_bytes().to_vec()
  };

  let checksum = adler32(optional_number(ctx, 1, 0.0)? as u32, &data);

  Ok(Value::number(checksum as f64))
}

fn crc32_fn(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_range!(ctx, 1, 2);
  enforce_arg_type_any_of!(ctx, 0, [ArgType::String, ArgType::Bytes]);

  let value = ctx.args[0];
  let data = if value.is_string() {
    value.as_str().as_bytes().to_vec()
  } else {
    value.as_bytes().to_vec()
  };

  let checksum = crc32(optional_number(ctx, 1, 0.0)? as u32, &data);

  Ok(Value::number(checksum as f64))
}
