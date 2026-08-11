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
    ("decompress", native(vm, "decompress", 1, true, decompress)),
    (
      "deflate_compress",
      native(vm, "compress", 1, false, deflate_compress),
    ),
    (
      "deflate_decompress",
      native(vm, "decompress", 1, false, deflate_decompress),
    ),
    (
      "gzip_compress",
      native(vm, "compress", 1, false, gzip_compress),
    ),
    (
      "gzip_decompress",
      native(vm, "decompress", 1, false, gzip_decompress),
    ),
    (
      "zstd_compress",
      native(vm, "compress", 1, true, zstd_compress),
    ),
    (
      "zstd_decompress",
      native(vm, "decompress", 1, false, zstd_decompress),
    ),
    ("crc32_checksum", native(vm, "crc32", 1, true, crc32_fn)),
    (
      "adler32_checksum",
      native(vm, "adler32", 1, true, adler32_fn),
    ),
  ]
}

fn parse_zlib_result(code: ReturnCode) -> Result<(), &'static str> {
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

fn parse_zlib_error(code: InflateError) -> &'static str {
  match code {
    InflateError::NeedDict { .. } => "Decompressing this input requires a dictionary.",
    InflateError::StreamError => {
      "Inflate is in an inconsistent state, most likely due to an invalid configuration parameter."
    },
    InflateError::DataError => "The input is not a valid deflate stream.",
    InflateError::MemError => "A memory allocation failed.",
  }
}

fn parse_zrip_compress_error(code: zrip::CompressError) -> &'static str {
  match code {
    zrip::CompressError::OutputTooSmall => "Output buffer is too small.",
    zrip::CompressError::InvalidLevel(_) => {
      "Compression level is outside the supported range (-7..=4)."
    },
    zrip::CompressError::InvalidDictionary => "Dictionary bytes failed to parse.",
  }
}

fn parse_zrip_decompress_error(code: zrip::DecompressError) -> &'static str {
  match code {
    zrip::DecompressError::BadMagic => "Frame magic number is not `0xFD2F_B528`.",
    zrip::DecompressError::BadFrameHeader => "Frame descriptor or field sizes are invalid.",
    zrip::DecompressError::BadBlockHeader => "Block header contains invalid values.",
    zrip::DecompressError::BadBlockType => "Block type field is reserved/unknown.",
    zrip::DecompressError::CorruptLiterals => "Literals section is malformed or truncated.",
    zrip::DecompressError::CorruptSequences => "Sequences section is malformed.",
    zrip::DecompressError::BlockTooLarge => "Raw or RLE block exceeds MAX_BLOCK_SIZE.",
    zrip::DecompressError::InvalidOffset => {
      "Sequence offset is invalid (zero or beyond available history)."
    },
    zrip::DecompressError::FrameSizeMismatch => {
      "Decoded output size does not match the frame content size field."
    },
    zrip::DecompressError::BadFseTable => "FSE table description is invalid.",
    zrip::DecompressError::BadHuffmanWeights => "Huffman weight table is malformed.",
    zrip::DecompressError::BadHuffmanStream => "Huffman bitstream decoding failed.",
    zrip::DecompressError::WindowTooLarge { .. } => {
      "Requested window size exceeds the implementation limit."
    },
    zrip::DecompressError::OutputTooSmall => {
      "Decompressed output would exceed the configured size limit."
    },
    zrip::DecompressError::ChecksumMismatch { .. } => {
      "Content checksum does not match the decompressed data."
    },
    zrip::DecompressError::DictMismatch { .. } => {
      "Frame requires dictionary ID `expected`, but `got` was provided."
    },
    zrip::DecompressError::DictRequired => "Frame requires a dictionary but none was provided.",
    zrip::DecompressError::InvalidDictionary => "Dictionary bytes failed to parse.",
    zrip::DecompressError::InputExhausted => "Input ended before the frame was complete.",
    zrip::DecompressError::ExtraBytes => "Unexpected trailing bytes after a valid frame.",
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

fn get_data(args: &[Value]) -> Vec<u8> {
  let value = args[0];
  if value.is_string() {
    value.as_str().as_bytes().to_vec()
  } else {
    value.as_bytes().to_vec()
  }
}

fn compress(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_range!(ctx, 1, 5);
  enforce_arg_type_any_of!(ctx, 0, [ArgType::String, ArgType::Bytes]);

  let data = get_data(ctx.args);
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

  if let Err(msg) = parse_zlib_result(rc) {
    return Err(msg.to_string());
  }

  Ok(ctx.heap().alloc_bytes(compressed_data))
}

fn decompress(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_range!(ctx, 1, 2);
  enforce_arg_type_any_of!(ctx, 0, [ArgType::String, ArgType::Bytes]);

  let data = get_data(ctx.args);
  let default_config: InflateConfig = InflateConfig::default();
  let window_bits = optional_number(ctx, 1, default_config.window_bits as f64)? as i32;

  let decompressed_data = decompress_data(&data, window_bits).map_err(|x| parse_zlib_error(x))?;

  Ok(ctx.heap().alloc_bytes(decompressed_data))
}

fn deflate_compress(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 1);
  enforce_arg_type_any_of!(ctx, 0, [ArgType::String, ArgType::Bytes]);

  let data = get_data(ctx.args);
  let mut config = DeflateConfig::default();
  config.window_bits = -config.window_bits; // raw deflate

  let mut output = vec![0u8; compress_bound(data.len())];
  let (compressed_data, rc) = compress_slice(&mut output, &data, config);

  if let Err(msg) = parse_zlib_result(rc) {
    return Err(msg.to_string());
  }

  Ok(ctx.heap().alloc_bytes(compressed_data))
}

fn deflate_decompress(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 1);
  enforce_arg_type_any_of!(ctx, 0, [ArgType::String, ArgType::Bytes]);

  let data = get_data(ctx.args);
  let config = InflateConfig::default();

  let decompressed_data =
    decompress_data(&data, -config.window_bits).map_err(|x| parse_zlib_error(x))?;

  Ok(ctx.heap().alloc_bytes(decompressed_data))
}

fn gzip_compress(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 1);
  enforce_arg_type_any_of!(ctx, 0, [ArgType::String, ArgType::Bytes]);

  let data = get_data(ctx.args);
  let mut config = DeflateConfig::default();
  config.window_bits = config.window_bits | 16; // raw gzip

  let mut output = vec![0u8; compress_bound(data.len())];
  let (compressed_data, rc) = compress_slice(&mut output, &data, config);

  if let Err(msg) = parse_zlib_result(rc) {
    return Err(msg.to_string());
  }

  Ok(ctx.heap().alloc_bytes(compressed_data))
}

fn gzip_decompress(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 1);
  enforce_arg_type_any_of!(ctx, 0, [ArgType::String, ArgType::Bytes]);

  let data = get_data(ctx.args);
  let mut config = InflateConfig::default();
  config.window_bits = config.window_bits | 16; // raw gzip

  let mut decompressed_data = Vec::new();
  let (_, rc) = decompress_slice(decompressed_data.as_mut_slice(), &data, config);

  if let Err(msg) = parse_zlib_result(rc) {
    return Err(msg.to_string());
  }

  Ok(ctx.heap().alloc_bytes(decompressed_data))
}

fn adler32_fn(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_range!(ctx, 1, 2);
  enforce_arg_type_any_of!(ctx, 0, [ArgType::String, ArgType::Bytes]);

  let data = get_data(ctx.args);
  let checksum = adler32(optional_number(ctx, 1, 0.0)? as u32, &data);

  Ok(Value::number(checksum as f64))
}

fn crc32_fn(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_range!(ctx, 1, 2);
  enforce_arg_type_any_of!(ctx, 0, [ArgType::String, ArgType::Bytes]);

  let data = get_data(ctx.args);
  let checksum = crc32(optional_number(ctx, 1, 0.0)? as u32, &data);

  Ok(Value::number(checksum as f64))
}

fn zstd_compress(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_range!(ctx, 1, 2);
  enforce_arg_type_any_of!(ctx, 0, [ArgType::String, ArgType::Bytes]);

  let data = get_data(ctx.args);
  let level = optional_number(ctx, 1, 1.0)? as i32;

  let compressed_data = zrip::compress(&data, level).map_err(|x| parse_zrip_compress_error(x))?;

  Ok(ctx.heap().alloc_bytes(compressed_data))
}

fn zstd_decompress(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 1);
  enforce_arg_type_any_of!(ctx, 0, [ArgType::String, ArgType::Bytes]);

  let data = get_data(ctx.args);

  let compressed_data = zrip::decompress(&data).map_err(|x| parse_zrip_decompress_error(x))?;

  Ok(ctx.heap().alloc_bytes(compressed_data))
}
