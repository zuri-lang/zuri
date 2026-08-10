use std::cell::Cell;

use rustc_hash::FxHashMap;
use zlib_rs::{
  DeflateConfig, InflateConfig, ReturnCode, Strategy, adler32::adler32, compress_bound,
  compress_slice, crc32::crc32, decompress_slice,
};

use crate::{
  builtins::enforce::ArgType,
  enforce_arg_count, enforce_arg_range, enforce_arg_type_any_of,
  modules::{BuiltinModuleDef, native, optional_number},
  vm::{
    object::{ModuleNamespace, ObjModule, ZuriContext},
    value::Value,
    vm::VM,
  },
};

pub static MODULE: BuiltinModuleDef = BuiltinModuleDef {
  name: "compress",
  build,
};

/// Re-exports the already globally-registered `sum` native under the
/// `math` namespace -- preserves `import math; math.sum(...)` exactly
/// as it worked before this registry existed.
fn build(vm: &mut VM) -> Vec<(&'static str, Value)> {
  /* Checksum  */
  let mut checksum_mod_entries = FxHashMap::default();
  checksum_mod_entries.insert("crc32".to_string(), 0);
  checksum_mod_entries.insert("adler32".to_string(), 1);

  let crc32 = native(vm, "crc32", 1, true, crc32_fn);
  let adler32 = native(vm, "adler32", 1, true, adler32_fn);

  let common_module = vm.heap_mut().alloc_module(ObjModule {
    name: "deflate".into(),
    path: "<native::mod::compress::deflate>".into(),
    namespace: ModuleNamespace {
      slots: vec![Cell::new(crc32), Cell::new(adler32)],
      names: checksum_mod_entries,
    },
    loaded: true,
  });

  /* Deflate  */
  let mut deflate_mod_entries = FxHashMap::default();
  deflate_mod_entries.insert("encode".to_string(), 0);
  deflate_mod_entries.insert("decode".to_string(), 1);

  let deflate_encode = native(vm, "encode", 1, false, deflate);
  let deflate_decode = native(vm, "decode", 1, false, undeflate);

  let deflate_module = vm.heap_mut().alloc_module(ObjModule {
    name: "deflate".into(),
    path: "<native::mod::compress::deflate>".into(),
    namespace: ModuleNamespace {
      slots: vec![Cell::new(deflate_encode), Cell::new(deflate_decode)],
      names: deflate_mod_entries,
    },
    loaded: true,
  });

  /* GZip  */
  let mut gzip_mod_entries = FxHashMap::default();
  gzip_mod_entries.insert("encode".to_string(), 0);
  gzip_mod_entries.insert("decode".to_string(), 1);

  let gzip_encode = native(vm, "encode", 1, false, gzip);
  let gzip_decode = native(vm, "decode", 1, false, ungzip);

  let gzip_module = vm.heap_mut().alloc_module(ObjModule {
    name: "deflate".into(),
    path: "<native::mod::compress::gzip>".into(),
    namespace: ModuleNamespace {
      slots: vec![Cell::new(gzip_encode), Cell::new(gzip_decode)],
      names: gzip_mod_entries,
    },
    loaded: true,
  });

  vec![
    // Top level constants
    ("NO_COMPRESSION", Value::number(0.0)),
    ("BEST_SPEED", Value::number(1.0)),
    ("BEST_COMPRESSION", Value::number(9.0)),
    ("DEFAULT_COMPRESSION", Value::number(-1.0)),
    ("FILTERED", Value::number(1.0)),
    ("HUFFMAN_ONLY", Value::number(2.0)),
    ("RLE", Value::number(3.0)),
    ("FIXED", Value::number(4.0)),
    ("DEFAULT_STRATEGY", Value::number(0.0)),
    ("DEFAULT_MEMORY_LEVEL", Value::number(8.0)),
    ("MAX_WBITS", Value::number(15.0)),
    // Top level functions
    ("compress", native(vm, "compress", 1, true, compress)),
    ("uncompress", native(vm, "uncompress", 1, true, uncompress)),
    // Sub modules
    ("checksum", common_module),
    ("deflate", deflate_module),
    ("gzip", gzip_module),
  ]
}

fn parse_result(code: ReturnCode) -> Result<(), &'static str> {
  match code {
    ReturnCode::Ok => Ok(()),
    ReturnCode::BufError => Err("Buffer error during compression"),
    ReturnCode::StreamError => Err("Stream error during compression"),
    ReturnCode::DataError => Err("Data error during compression"),
    ReturnCode::MemError => Err("Memory error during compression"),
    ReturnCode::VersionError => Err("Version error during compression"),
    ReturnCode::NeedDict => Err("Dictionary needed during compression"),
    ReturnCode::StreamEnd => Err("Stream ended unexpectedly during compression"),
    ReturnCode::ErrNo => Err("Unknown error during compression"),
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

  let mut compressed_data = vec![0u8; compress_bound(data.len())];
  let (_, rc) = compress_slice(&mut compressed_data, &data, config);

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

  let config = InflateConfig {
    window_bits: optional_number(ctx, 1, default_config.window_bits as f64)? as i32,
  };

  let mut decompressed_data = vec![0u8; compress_bound(data.len())];
  let (_, rc) = decompress_slice(&mut decompressed_data, &data, config);

  if let Err(msg) = parse_result(rc) {
    return Err(msg.to_string());
  }

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

  let mut compressed_data = vec![0u8; compress_bound(data.len())];
  let (_, rc) = compress_slice(&mut compressed_data, &data, config);

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

  let mut config = InflateConfig::default();
  config.window_bits = -config.window_bits; // raw inflate

  let mut decompressed_data = vec![0u8; compress_bound(data.len())];
  let (_, rc) = decompress_slice(&mut decompressed_data, &data, config);

  if let Err(msg) = parse_result(rc) {
    return Err(msg.to_string());
  }

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

  let mut compressed_data = vec![0u8; compress_bound(data.len())];
  let (_, rc) = compress_slice(&mut compressed_data, &data, config);

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

  let mut decompressed_data = vec![0u8; compress_bound(data.len())];
  let (_, rc) = decompress_slice(&mut decompressed_data, &data, config);

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
