use std::io::{Cursor, Read, Write};

use lz4_flex::{compress_prepend_size, decompress_size_prepended};
use zlib_rs::{
  DeflateConfig, DeflateError, Inflate, InflateConfig, InflateError, InflateFlush, ReturnCode,
  Status, Strategy, adler32::adler32, compress_bound, compress_slice, crc32::crc32,
};

use crate::{
  builtins::enforce::{
    ArgType, enforce_method_arg_count, enforce_method_arg_type, enforce_method_arg_type_any_of,
  },
  enforce_arg_count, enforce_arg_range, enforce_arg_type, enforce_arg_type_any_of,
  modules::{
    BuiltinModuleDef, compress_util::DeflateDecoderError, native, optional_bool, optional_number,
  },
  vm::{object::ZuriContext, value::Value, vm::VM},
};

pub static MODULE: BuiltinModuleDef = BuiltinModuleDef {
  name: "_compress",
  build,
};

use super::compress_util::{DeflateDecoder, DeflateEncoder};

fn build(vm: &mut VM) -> Vec<(&'static str, Value)> {
  vec![
    ("crc32_checksum", native(vm, "crc32", 1, true, crc32_fn)),
    (
      "adler32_checksum",
      native(vm, "adler32", 1, true, adler32_fn),
    ),
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
      "gzip_new_encoder",
      native(vm, "@new", 0, true, gzip_new_encoder),
    ),
    (
      "gzip_encoder_finish",
      native(vm, "finish", 1, false, gzip_encoder_finish),
    ),
    (
      "gzip_encoder_reset",
      native(vm, "reset", 1, false, gzip_encoder_reset),
    ),
    (
      "gzip_encoder_write",
      native(vm, "write", 2, false, gzip_encoder_write),
    ),
    (
      "gzip_encoder_flush",
      native(vm, "flush", 1, false, gzip_encoder_flush),
    ),
    (
      "gzip_encoder_available",
      native(vm, "available", 1, false, gzip_encoder_available),
    ),
    (
      "gzip_encoder_finished",
      native(vm, "finished", 1, false, gzip_encoder_finished),
    ),
    (
      "gzip_encoder_total_in",
      native(vm, "total_in", 1, false, gzip_encoder_total_in),
    ),
    (
      "gzip_encoder_total_out",
      native(vm, "total_out", 1, false, gzip_encoder_total_out),
    ),
    (
      "gzip_new_decoder",
      native(vm, "@new", 1, true, gzip_new_decoder),
    ),
    (
      "gzip_decoder_reset",
      native(vm, "reset", 1, false, gzip_decoder_reset),
    ),
    (
      "gzip_decoder_read",
      native(vm, "read", 2, false, gzip_decoder_read),
    ),
    (
      "gzip_decoder_read_exact",
      native(vm, "read_exact", 2, false, gzip_decoder_read_exact),
    ),
    (
      "gzip_decoder_read_all",
      native(vm, "read_all", 1, false, gzip_decoder_read_all),
    ),
    (
      "gzip_decoder_read_as_string",
      native(vm, "read_as_string", 1, false, gzip_decoder_read_as_string),
    ),
    (
      "gzip_decoder_available",
      native(vm, "available", 1, false, gzip_decoder_available),
    ),
    (
      "gzip_decoder_finished",
      native(vm, "finished", 1, false, gzip_decoder_finished),
    ),
    (
      "gzip_decoder_total_in",
      native(vm, "total_in", 1, false, gzip_decoder_total_in),
    ),
    (
      "gzip_decoder_total_out",
      native(vm, "total_out", 1, false, gzip_decoder_total_out),
    ),
    (
      "zstd_compress",
      native(vm, "compress", 1, true, zstd_compress),
    ),
    (
      "zstd_decompress",
      native(vm, "decompress", 1, false, zstd_decompress),
    ),
    (
      "zstd_new_encoder",
      native(vm, "@new", 0, true, zstd_new_encoder),
    ),
    (
      "zstd_encoder_finish",
      native(vm, "finish", 1, false, zstd_encoder_finish),
    ),
    (
      "zstd_encoder_reset",
      native(vm, "reset", 1, false, zstd_encoder_reset),
    ),
    (
      "zstd_encoder_write",
      native(vm, "write", 2, false, zstd_encoder_write),
    ),
    (
      "zstd_encoder_write_all",
      native(vm, "write_all", 2, false, zstd_encoder_write_all),
    ),
    (
      "zstd_encoder_flush",
      native(vm, "flush", 1, false, zstd_encoder_flush),
    ),
    (
      "zstd_new_decoder",
      native(vm, "@new", 1, false, zstd_new_decoder),
    ),
    (
      "zstd_decoder_reset",
      native(vm, "reset", 1, false, zstd_decoder_reset),
    ),
    (
      "zstd_decoder_read",
      native(vm, "read", 2, false, zstd_decoder_read),
    ),
    (
      "zstd_decoder_read_exact",
      native(vm, "zread_exact", 2, true, zstd_decoder_read_exact),
    ),
    (
      "zstd_decoder_read_all",
      native(vm, "read_all", 1, false, zstd_decoder_read_all),
    ),
    (
      "zstd_decoder_read_as_string",
      native(vm, "read_as_string", 1, false, zstd_decoder_read_as_string),
    ),
    (
      "lz4_compress",
      native(vm, "compress", 1, true, lz4_compress),
    ),
    (
      "lz4_decompress",
      native(vm, "decompress", 1, false, lz4_decompress),
    ),
    (
      "lz4_new_encoder",
      native(vm, "@new", 0, true, lz4_new_encoder),
    ),
    (
      "lz4_encoder_finish",
      native(vm, "finish", 1, false, lz4_encoder_finish),
    ),
    (
      "lz4_encoder_write",
      native(vm, "write", 2, false, lz4_encoder_write),
    ),
    (
      "lz4_encoder_write_all",
      native(vm, "write_all", 2, false, lz4_encoder_write_all),
    ),
    (
      "lz4_encoder_flush",
      native(vm, "flush", 1, false, lz4_encoder_flush),
    ),
    (
      "lz4_new_decoder",
      native(vm, "@new", 1, false, lz4_new_decoder),
    ),
    (
      "lz4_decoder_read",
      native(vm, "read", 2, false, lz4_decoder_read),
    ),
    (
      "lz4_decoder_read_exact",
      native(vm, "zread_exact", 2, true, lz4_decoder_read_exact),
    ),
    (
      "lz4_decoder_read_all",
      native(vm, "read_all", 1, false, lz4_decoder_read_all),
    ),
    (
      "lz4_decoder_read_as_string",
      native(vm, "read_as_string", 1, false, lz4_decoder_read_as_string),
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

fn parse_zlib_inflate_error(code: InflateError) -> &'static str {
  match code {
    InflateError::NeedDict { .. } => "Decompressing this input requires a dictionary.",
    InflateError::StreamError => {
      "Inflate is in an inconsistent state, most likely due to an invalid configuration parameter."
    },
    InflateError::DataError => "The input is not a valid deflate stream.",
    InflateError::MemError => "A memory allocation failed.",
  }
}

fn parse_zlib_deflate_error(code: DeflateError) -> &'static str {
  match code {
    DeflateError::StreamError => {
      "The [`Deflate`] is in an inconsistent state, most likely due to an invalid configuration parameter."
    },
    DeflateError::DataError => "The input is not a valid deflate stream.",
    DeflateError::MemError => "A memory allocation failed.",
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

fn parse_lz4_decompress_error(code: lz4_flex::block::DecompressError) -> &'static str {
  match code {
    lz4_flex::block::DecompressError::OutputTooSmall { .. } => "The provided output is too small",
    lz4_flex::block::DecompressError::LiteralOutOfBounds => "Literal is out of bounds of the input",
    lz4_flex::block::DecompressError::ExpectedAnotherByte => {
      "Expected another byte, but none found."
    },
    lz4_flex::block::DecompressError::OffsetZero => "Match offset is 0",
    lz4_flex::block::DecompressError::OffsetOutOfBounds => {
      "Deduplication offset out of bounds (not in buffer)."
    },
    _ => "Unknown error",
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

// ZLib

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

  let decompressed_data =
    decompress_data(&data, window_bits).map_err(|x| parse_zlib_inflate_error(x))?;

  Ok(ctx.heap().alloc_bytes(decompressed_data))
}

// Deflate

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
    decompress_data(&data, -config.window_bits).map_err(|x| parse_zlib_inflate_error(x))?;

  Ok(ctx.heap().alloc_bytes(decompressed_data))
}

// GZIP

const GZIP_ENCODER_NAME: &str = "zuri::compress::gzip::encoder";
const GZIP_DECODER_NAME: &str = "zuri::compress::gzip::decoder";

fn gzip_compress(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 1);
  enforce_arg_type_any_of!(ctx, 0, [ArgType::String, ArgType::Bytes]);

  let data = get_data(ctx.args);
  let mut config = DeflateConfig::default();
  config.window_bits = config.window_bits | 16; // raw gzip

  let mut output = vec![0u8; compress_bound(data.len()) + 18]; // 10 byte header + 8 byte footer workaround.
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

  let decompressed_data =
    decompress_data(&data, config.window_bits).map_err(|x| parse_zlib_inflate_error(x))?;

  Ok(ctx.heap().alloc_bytes(decompressed_data))
}

fn gzip_new_encoder(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_range!(ctx, 1, 2);

  let zlib_header = optional_bool(ctx, 0, false)?;
  let level = optional_number(ctx, 1, 1.0)? as i32;

  let encoder = DeflateEncoder::new(level, zlib_header);
  Ok(ctx.heap().alloc_ptr(GZIP_ENCODER_NAME, encoder))
}

fn gzip_encoder_finish(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(GZIP_ENCODER_NAME));

  let mut ptr = ctx.args[0].as_ptr_cell().borrow_mut();
  let encoder = ptr.downcast_mut::<DeflateEncoder>().unwrap();

  let output = encoder.finish().map_err(parse_zlib_deflate_error)?;

  Ok(ctx.heap().alloc_bytes(output))
}

fn gzip_encoder_reset(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(GZIP_ENCODER_NAME));

  let mut ptr = ctx.args[0].as_ptr_cell().borrow_mut();
  let encoder = ptr.downcast_mut::<DeflateEncoder>().unwrap();

  let output = encoder.reset().map_err(parse_zlib_deflate_error)?;

  Ok(ctx.heap().alloc_bytes(output))
}

fn gzip_encoder_write(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(GZIP_ENCODER_NAME));
  enforce_method_arg_type_any_of!(ctx, 1, [ArgType::Bytes, ArgType::String]);

  let mut ptr = ctx.args[0].as_ptr_cell().borrow_mut();
  let encoder = ptr.downcast_mut::<DeflateEncoder>().unwrap();

  let data = get_data(&ctx.args[1..]);

  Ok(Value::number(
    encoder
      .write(data.as_slice())
      .map_err(parse_zlib_deflate_error)? as f64,
  ))
}

fn gzip_encoder_flush(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(GZIP_ENCODER_NAME));

  let mut ptr = ctx.args[0].as_ptr_cell().borrow_mut();
  let encoder = ptr.downcast_mut::<DeflateEncoder>().unwrap();

  let data = encoder.flush().map_err(parse_zlib_deflate_error)?;

  Ok(ctx.heap().alloc_bytes(data))
}

fn gzip_encoder_available(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(GZIP_ENCODER_NAME));

  let mut ptr = ctx.args[0].as_ptr_cell().borrow_mut();
  let encoder = ptr.downcast_mut::<DeflateEncoder>().unwrap();

  Ok(Value::number(encoder.available() as f64))
}

fn gzip_encoder_finished(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(GZIP_ENCODER_NAME));

  let mut ptr = ctx.args[0].as_ptr_cell().borrow_mut();
  let encoder = ptr.downcast_mut::<DeflateEncoder>().unwrap();

  Ok(Value::bool(encoder.is_finished()))
}

fn gzip_encoder_total_in(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(GZIP_ENCODER_NAME));

  let mut ptr = ctx.args[0].as_ptr_cell().borrow_mut();
  let encoder = ptr.downcast_mut::<DeflateEncoder>().unwrap();

  Ok(Value::number(encoder.total_in() as f64))
}

fn gzip_encoder_total_out(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(GZIP_ENCODER_NAME));

  let mut ptr = ctx.args[0].as_ptr_cell().borrow_mut();
  let encoder = ptr.downcast_mut::<DeflateEncoder>().unwrap();

  Ok(Value::number(encoder.total_out() as f64))
}

fn gzip_new_decoder(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_range!(ctx, 1, 2);
  enforce_arg_type!(ctx, 1, ArgType::Bytes);

  let data = ctx.args[1].as_bytes();
  let zlib_header = optional_bool(ctx, 0, false)?;

  let decoder = DeflateDecoder::new(data, zlib_header);
  Ok(ctx.heap().alloc_ptr(GZIP_DECODER_NAME, decoder))
}

fn gzip_decoder_reset(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(GZIP_DECODER_NAME));
  enforce_method_arg_type!(ctx, 1, ArgType::Bytes);

  let mut ptr = ctx.args[0].as_ptr_cell().borrow_mut();
  let decoder = ptr.downcast_mut::<DeflateDecoder>().unwrap();

  decoder.reset(ctx.args[1].as_bytes());

  Ok(Value::nil())
}

fn gzip_decoder_read(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(GZIP_DECODER_NAME));
  enforce_method_arg_type!(ctx, 1, ArgType::Number);

  let mut ptr = ctx.args[0].as_ptr_cell().borrow_mut();
  let decoder = ptr.downcast_mut::<DeflateDecoder>().unwrap();

  let length = ctx.args[1].as_number() as usize;

  let mut buffer = Vec::new();
  let bytes_read = decoder
    .read(&mut buffer, length)
    .map_err(parse_zlib_inflate_error)?;

  if bytes_read > 0 {
    return Ok(ctx.heap().alloc_bytes(&buffer[0..bytes_read]));
  }

  Ok(ctx.heap().alloc_bytes(Vec::new()))
}

fn gzip_decoder_read_exact(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(GZIP_DECODER_NAME));
  enforce_method_arg_type!(ctx, 1, ArgType::Number);

  let mut ptr = ctx.args[0].as_ptr_cell().borrow_mut();
  let decoder = ptr.downcast_mut::<DeflateDecoder>().unwrap();

  let length = ctx.args[1].as_number() as usize;

  let mut buffer = vec![0u8; length];
  decoder
    .read_exact(&mut buffer, length)
    .map_err(parse_zlib_inflate_error)?;

  Ok(ctx.heap().alloc_bytes(buffer))
}

fn gzip_decoder_read_all(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(GZIP_DECODER_NAME));

  let mut ptr = ctx.args[0].as_ptr_cell().borrow_mut();
  let decoder = ptr.downcast_mut::<DeflateDecoder>().unwrap();

  let mut buffer = Vec::new();
  decoder
    .read_to_end(&mut buffer)
    .map_err(parse_zlib_inflate_error)?;

  Ok(ctx.heap().alloc_bytes(buffer))
}

fn gzip_decoder_read_as_string(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(GZIP_DECODER_NAME));

  let mut ptr = ctx.args[0].as_ptr_cell().borrow_mut();
  let decoder = ptr.downcast_mut::<DeflateDecoder>().unwrap();

  let mut buffer = String::new();
  decoder.read_to_string(&mut buffer).map_err(|x| match x {
    DeflateDecoderError::Inflate(code) => parse_zlib_inflate_error(code).to_string(),
    DeflateDecoderError::Utf8(e) => e.to_string(),
  })?;

  Ok(ctx.heap().alloc_string(buffer))
}

fn gzip_decoder_available(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(GZIP_DECODER_NAME));

  let mut ptr = ctx.args[0].as_ptr_cell().borrow_mut();
  let encoder = ptr.downcast_mut::<DeflateDecoder>().unwrap();

  Ok(Value::number(encoder.available() as f64))
}

fn gzip_decoder_finished(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(GZIP_DECODER_NAME));

  let mut ptr = ctx.args[0].as_ptr_cell().borrow_mut();
  let encoder = ptr.downcast_mut::<DeflateDecoder>().unwrap();

  Ok(Value::bool(encoder.is_finished()))
}

fn gzip_decoder_total_in(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(GZIP_DECODER_NAME));

  let mut ptr = ctx.args[0].as_ptr_cell().borrow_mut();
  let encoder = ptr.downcast_mut::<DeflateDecoder>().unwrap();

  Ok(Value::number(encoder.total_in() as f64))
}

fn gzip_decoder_total_out(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(GZIP_DECODER_NAME));

  let mut ptr = ctx.args[0].as_ptr_cell().borrow_mut();
  let encoder = ptr.downcast_mut::<DeflateDecoder>().unwrap();

  Ok(Value::number(encoder.total_out() as f64))
}

// Checksum

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

// ZSTD

const ZSTD_ENCODER_NAME: &str = "zuri::compress::zstd::encoder";
const ZSTD_DECODER_NAME: &str = "zuri::compress::zstd::decoder";

fn zstd_compress(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_range!(ctx, 1, 2);
  enforce_arg_type_any_of!(ctx, 0, [ArgType::String, ArgType::Bytes]);

  let data = get_data(ctx.args);
  let level = optional_number(ctx, 1, 1.0)? as i32;

  let compressed_data = zrip::compress(&data, level).map_err(parse_zrip_compress_error)?;

  Ok(ctx.heap().alloc_bytes(compressed_data))
}

fn zstd_decompress(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 1);
  enforce_arg_type_any_of!(ctx, 0, [ArgType::String, ArgType::Bytes]);

  let data = get_data(ctx.args);

  let decompressed_data = zrip::decompress(&data).map_err(parse_zrip_decompress_error)?;

  Ok(ctx.heap().alloc_bytes(decompressed_data))
}

fn zstd_new_encoder(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_range!(ctx, 0, 3);

  let level = optional_number(ctx, 0, 1.0)? as i32;
  let window_log = optional_number(ctx, 1, 10.0)? as u32;
  let ldm = optional_bool(ctx, 2, false)?;

  let option = zrip::Options::default().window_log(window_log).ldm(ldm);

  let encoder = zrip::FrameEncoder::with_options(Vec::new(), level, &option)
    .map_err(parse_zrip_compress_error)?;
  Ok(ctx.heap().alloc_ptr(ZSTD_ENCODER_NAME, encoder))
}

fn zstd_encoder_finish(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(ZSTD_ENCODER_NAME));

  let mut ptr = ctx.args[0].as_ptr_cell().borrow_mut();

  let boxed = std::mem::replace(&mut ptr.value, Box::new(()));
  ptr.type_name = "zuri::compress::__finished__";

  let encoder = boxed.downcast::<zrip::FrameEncoder<Vec<u8>>>().unwrap();

  let result = encoder.finish().map_err(|x| x.to_string())?;

  Ok(ctx.heap().alloc_bytes(result))
}

fn zstd_encoder_reset(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(ZSTD_ENCODER_NAME));

  let mut ptr = ctx.args[0].as_ptr_cell().borrow_mut();
  let encoder = ptr.downcast_mut::<zrip::FrameEncoder<Vec<u8>>>().unwrap();

  let result = encoder.reset(Vec::new()).map_err(|x| x.to_string())?;

  Ok(ctx.heap().alloc_bytes(result))
}

fn zstd_encoder_write(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(ZSTD_ENCODER_NAME));
  enforce_method_arg_type_any_of!(ctx, 1, [ArgType::Bytes, ArgType::String]);

  let mut ptr = ctx.args[0].as_ptr_cell().borrow_mut();
  let encoder = ptr.downcast_mut::<zrip::FrameEncoder<Vec<u8>>>().unwrap();

  let data = get_data(&ctx.args[1..]);

  Ok(Value::number(
    encoder.write(data.as_slice()).map_err(|x| x.to_string())? as f64,
  ))
}

fn zstd_encoder_write_all(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(ZSTD_ENCODER_NAME));
  enforce_method_arg_type_any_of!(ctx, 1, [ArgType::Bytes, ArgType::String]);

  let mut ptr = ctx.args[0].as_ptr_cell().borrow_mut();
  let encoder = ptr.downcast_mut::<zrip::FrameEncoder<Vec<u8>>>().unwrap();

  let data = get_data(&ctx.args[1..]);
  encoder
    .write_all(data.as_slice())
    .map_err(|x| x.to_string())?;

  Ok(Value::nil())
}

fn zstd_encoder_flush(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(ZSTD_ENCODER_NAME));

  let mut ptr = ctx.args[0].as_ptr_cell().borrow_mut();
  let encoder = ptr.downcast_mut::<zrip::FrameEncoder<Vec<u8>>>().unwrap();

  encoder.flush().map_err(|x| x.to_string())?;

  Ok(Value::nil())
}

fn zstd_new_decoder(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 1);
  enforce_arg_type!(ctx, 0, ArgType::Bytes);

  let data = ctx.args[0].as_bytes();
  let cursor = Cursor::new(data);

  let decoder = zrip::FrameDecoder::new(cursor);
  Ok(ctx.heap().alloc_ptr(ZSTD_DECODER_NAME, decoder))
}

fn zstd_decoder_reset(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(ZSTD_DECODER_NAME));
  enforce_method_arg_type!(ctx, 1, ArgType::Bytes);

  let mut ptr = ctx.args[0].as_ptr_cell().borrow_mut();
  let decoder = ptr
    .downcast_mut::<zrip::FrameDecoder<Cursor<Vec<u8>>>>()
    .unwrap();

  let data = ctx.args[1].as_bytes();
  let cursor = Cursor::new(data);

  decoder.reset(cursor);

  Ok(Value::nil())
}

fn zstd_decoder_read(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(ZSTD_DECODER_NAME));
  enforce_method_arg_type!(ctx, 1, ArgType::Number);

  let mut ptr = ctx.args[0].as_ptr_cell().borrow_mut();
  let decoder = ptr
    .downcast_mut::<zrip::FrameDecoder<Cursor<Vec<u8>>>>()
    .unwrap();

  let length = ctx.args[1].as_number() as usize;

  let mut buffer = vec![0u8; length];
  let bytes_read = decoder
    .read(buffer.as_mut_slice())
    .map_err(|x| x.to_string())?;

  if bytes_read > 0 {
    return Ok(ctx.heap().alloc_bytes(&buffer[0..bytes_read]));
  }

  Ok(ctx.heap().alloc_bytes(Vec::new()))
}

fn zstd_decoder_read_exact(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(ZSTD_DECODER_NAME));
  enforce_method_arg_type!(ctx, 1, ArgType::Number);

  let mut ptr = ctx.args[0].as_ptr_cell().borrow_mut();
  let decoder = ptr
    .downcast_mut::<zrip::FrameDecoder<Cursor<Vec<u8>>>>()
    .unwrap();

  let length = ctx.args[1].as_number() as usize;

  let mut buffer = vec![0u8; length];
  decoder
    .read_exact(buffer.as_mut_slice())
    .map_err(|x| x.to_string())?;

  Ok(ctx.heap().alloc_bytes(buffer))
}

fn zstd_decoder_read_all(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(ZSTD_DECODER_NAME));

  let mut ptr = ctx.args[0].as_ptr_cell().borrow_mut();
  let decoder = ptr
    .downcast_mut::<zrip::FrameDecoder<Cursor<Vec<u8>>>>()
    .unwrap();

  let mut buffer = Vec::new();
  decoder
    .read_to_end(&mut buffer)
    .map_err(|x| x.to_string())?;

  Ok(ctx.heap().alloc_bytes(buffer))
}

fn zstd_decoder_read_as_string(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(ZSTD_DECODER_NAME));

  let mut ptr = ctx.args[0].as_ptr_cell().borrow_mut();
  let decoder = ptr
    .downcast_mut::<zrip::FrameDecoder<Cursor<Vec<u8>>>>()
    .unwrap();

  let mut buffer = String::new();
  decoder
    .read_to_string(&mut buffer)
    .map_err(|x| x.to_string())?;

  Ok(ctx.heap().alloc_string(buffer))
}

// LZ4

const LZ4_ENCODER_NAME: &str = "zuri::compress::lz4::encoder";
const LZ4_DECODER_NAME: &str = "zuri::compress::lz4::decoder";

fn lz4_compress(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 1);
  enforce_arg_type_any_of!(ctx, 0, [ArgType::String, ArgType::Bytes]);

  let data = get_data(ctx.args);

  let compressed_data = compress_prepend_size(data.as_slice());

  Ok(ctx.heap().alloc_bytes(compressed_data))
}

fn lz4_decompress(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 1);
  enforce_arg_type_any_of!(ctx, 0, [ArgType::String, ArgType::Bytes]);

  let data = get_data(ctx.args);

  let decompressed_data = decompress_size_prepended(&data).map_err(parse_lz4_decompress_error)?;

  Ok(ctx.heap().alloc_bytes(decompressed_data))
}

fn lz4_new_encoder(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 0);

  let encoder = lz4_flex::frame::FrameEncoder::new(Vec::new());
  Ok(ctx.heap().alloc_ptr(LZ4_ENCODER_NAME, encoder))
}

fn lz4_encoder_finish(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(LZ4_ENCODER_NAME));

  let mut ptr = ctx.args[0].as_ptr_cell().borrow_mut();

  let boxed = std::mem::replace(&mut ptr.value, Box::new(()));
  ptr.type_name = "zuri::compress::__finished__";

  let encoder = boxed
    .downcast::<lz4_flex::frame::FrameEncoder<Vec<u8>>>()
    .unwrap();

  let result = encoder.finish().map_err(|x| x.to_string())?;

  Ok(ctx.heap().alloc_bytes(result))
}

fn lz4_encoder_write(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(LZ4_ENCODER_NAME));
  enforce_method_arg_type_any_of!(ctx, 1, [ArgType::Bytes, ArgType::String]);

  let mut ptr = ctx.args[0].as_ptr_cell().borrow_mut();
  let encoder = ptr
    .downcast_mut::<lz4_flex::frame::FrameEncoder<Vec<u8>>>()
    .unwrap();

  let data = get_data(&ctx.args[1..]);

  Ok(Value::number(
    encoder.write(data.as_slice()).map_err(|x| x.to_string())? as f64,
  ))
}

fn lz4_encoder_write_all(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(LZ4_ENCODER_NAME));
  enforce_method_arg_type_any_of!(ctx, 1, [ArgType::Bytes, ArgType::String]);

  let mut ptr = ctx.args[0].as_ptr_cell().borrow_mut();
  let encoder = ptr
    .downcast_mut::<lz4_flex::frame::FrameEncoder<Vec<u8>>>()
    .unwrap();

  let data = get_data(&ctx.args[1..]);
  encoder
    .write_all(data.as_slice())
    .map_err(|x| x.to_string())?;

  Ok(Value::nil())
}

fn lz4_encoder_flush(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(LZ4_ENCODER_NAME));

  let mut ptr = ctx.args[0].as_ptr_cell().borrow_mut();
  let encoder = ptr
    .downcast_mut::<lz4_flex::frame::FrameEncoder<Vec<u8>>>()
    .unwrap();

  encoder.flush().map_err(|x| x.to_string())?;

  Ok(Value::nil())
}

fn lz4_new_decoder(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 1);
  enforce_arg_type!(ctx, 0, ArgType::Bytes);

  let data = ctx.args[0].as_bytes();
  let cursor = Cursor::new(data);

  let decoder = lz4_flex::frame::FrameDecoder::new(cursor);
  Ok(ctx.heap().alloc_ptr(LZ4_DECODER_NAME, decoder))
}

fn lz4_decoder_read(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(LZ4_DECODER_NAME));
  enforce_method_arg_type!(ctx, 1, ArgType::Number);

  let mut ptr = ctx.args[0].as_ptr_cell().borrow_mut();
  let decoder = ptr
    .downcast_mut::<lz4_flex::frame::FrameDecoder<Cursor<Vec<u8>>>>()
    .unwrap();

  let length = ctx.args[1].as_number() as usize;

  let mut buffer = vec![0u8; length];
  let bytes_read = decoder
    .read(buffer.as_mut_slice())
    .map_err(|x| x.to_string())?;

  if bytes_read > 0 {
    return Ok(ctx.heap().alloc_bytes(&buffer[0..bytes_read]));
  }

  Ok(ctx.heap().alloc_bytes(Vec::new()))
}

fn lz4_decoder_read_exact(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(LZ4_DECODER_NAME));
  enforce_method_arg_type!(ctx, 1, ArgType::Number);

  let mut ptr = ctx.args[0].as_ptr_cell().borrow_mut();
  let decoder = ptr
    .downcast_mut::<lz4_flex::frame::FrameDecoder<Cursor<Vec<u8>>>>()
    .unwrap();

  let length = ctx.args[1].as_number() as usize;

  let mut buffer = vec![0u8; length];
  decoder
    .read_exact(buffer.as_mut_slice())
    .map_err(|x| x.to_string())?;

  Ok(ctx.heap().alloc_bytes(buffer))
}

fn lz4_decoder_read_all(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(LZ4_DECODER_NAME));

  let mut ptr = ctx.args[0].as_ptr_cell().borrow_mut();
  let decoder = ptr
    .downcast_mut::<lz4_flex::frame::FrameDecoder<Cursor<Vec<u8>>>>()
    .unwrap();

  let mut buffer = Vec::new();
  decoder
    .read_to_end(&mut buffer)
    .map_err(|x| x.to_string())?;

  Ok(ctx.heap().alloc_bytes(buffer))
}

fn lz4_decoder_read_as_string(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(LZ4_DECODER_NAME));

  let mut ptr = ctx.args[0].as_ptr_cell().borrow_mut();
  let decoder = ptr
    .downcast_mut::<lz4_flex::frame::FrameDecoder<Cursor<Vec<u8>>>>()
    .unwrap();

  let mut buffer = String::new();
  decoder
    .read_to_string(&mut buffer)
    .map_err(|x| x.to_string())?;

  Ok(ctx.heap().alloc_string(buffer))
}
