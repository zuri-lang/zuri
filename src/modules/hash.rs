//! `_hash` builtin module; native primitives backing `libs/hash.zu`.
//!
//! `libs/hash.zu` (the user-facing `hash` module) implements everything
//! documented in its own docblocks; `md5()`, `sha256()`, `hmac_*()`,
//! `pbkdf2()`, hex encoding, argument validation, etc; as ordinary Zuri
//! source, the same way `libs/os.zu` is a thin Zuri wrapper around the
//! `_os` native module (see `src/modules/os.rs`). This module is that
//! native backing for `_hash`: it does only the parts that genuinely
//! require native code (the actual digest algorithms), and nothing else.
//! `hash.zu`'s `hash(algorithm, data, as_bytes)` dispatcher calls straight
//! into `_hash.hash(algorithm, data)` for every algorithm EXCEPT the FNV
//! family and GOST, which it special-cases to `_hash.fnv1(data)` /
//! `_hash.fnv1_64(data)` / `_hash.fnv1a(data)` / `_hash.fnv1a_64(data)` /
//! `_hash.gost(data)` directly: see `hash.zu`'s own `hash()` body.
//!
//! Every native here returns the RAW DIGEST as a `bytes` value; hex
//! encoding (the default, most common presentation) is entirely `hash.zu`'s
//! job via `convert.bytes_to_hex`, matching the documented
//! `{string|bytes}` return type and the `as_bytes` flag's semantics.

use digest::Digest;

use crate::builtins::enforce::ArgType;
use crate::modules::{BuiltinModuleDef, native};
use crate::vm::object::{DictKey, ZuriContext};
use crate::vm::value::Value;
use crate::vm::vm::VM;
use crate::{enforce_arg_count, enforce_arg_type};

pub static MODULE: BuiltinModuleDef = BuiltinModuleDef {
  name: "_hash",
  build,
};

fn build(vm: &mut VM) -> Vec<(&'static str, Value)> {
  vec![
    ("id", native(vm, "id", 1, false, id_fn)),
    ("hash", native(vm, "hash", 2, false, hash_fn)),
    ("fnv1", native(vm, "fnv1", 1, false, fnv1_fn)),
    ("fnv1_64", native(vm, "fnv1_64", 1, false, fnv1_64_fn)),
    ("fnv1a", native(vm, "fnv1a", 1, false, fnv1a_fn)),
    ("fnv1a_64", native(vm, "fnv1a_64", 1, false, fnv1a_64_fn)),
    ("gost", native(vm, "gost", 1, false, gost_fn)),
  ]
}

// Helpers

/// `hash.zu` accepts `{string|bytes}` for every data argument; strings
/// are hashed over their UTF-8 bytes, matching `string.to_bytes()`
/// elsewhere in this VM.
fn data_bytes(v: Value) -> Result<Vec<u8>, String> {
  if v.is_string() {
    Ok(v.as_str().as_bytes().to_vec())
  } else if v.is_bytes() {
    Ok(v.as_bytes())
  } else {
    Err(format!(
      "data must be of type bytes or string, got {}",
      v.type_name()
    ))
  }
}

/// Every algorithm `hash.zu`'s `hash()` dispatcher forwards to
/// `_hash.hash()` (i.e. everything documented EXCEPT the FNV family and
/// GOST, which get their own dedicated natives below). Names are matched
/// lowercase; `hash.zu` already lowercases before calling in.
fn digest_bytes(algorithm: &str, data: &[u8]) -> Result<Vec<u8>, String> {
  use blake2::{Blake2b512, Blake2s256};
  use md2::Md2;
  use md4::Md4;
  use md5::Md5;
  use ripemd::Ripemd160;
  use sha1::Sha1;
  use sha2::{Sha224, Sha256, Sha384, Sha512, Sha512_224, Sha512_256};
  use sha3::digest::{ExtendableOutput, Update, XofReader};
  use sha3::{Sha3_224, Sha3_256, Sha3_384, Sha3_512, Shake128, Shake256};
  use sm3::Sm3;
  use whirlpool::Whirlpool;

  Ok(match algorithm {
    "md2" => Md2::digest(data).to_vec(),
    "md4" => Md4::digest(data).to_vec(),
    "md5" => Md5::digest(data).to_vec(),
    "sha" | "sha1" => Sha1::digest(data).to_vec(),
    "sha224" => Sha224::digest(data).to_vec(),
    "sha256" => Sha256::digest(data).to_vec(),
    "sha384" => Sha384::digest(data).to_vec(),
    "sha512" => Sha512::digest(data).to_vec(),
    "sha512-224" => Sha512_224::digest(data).to_vec(),
    "sha512-256" => Sha512_256::digest(data).to_vec(),
    // OpenSSL's legacy md5-sha1 "digest" (TLS 1.0/1.1 handshake hash) is
    // simply the two digests concatenated, MD5 first.
    "md5-sha1" => {
      let mut v = Md5::digest(data).to_vec();
      v.extend(Sha1::digest(data));
      v
    },
    "sha3-224" => Sha3_224::digest(data).to_vec(),
    "sha3-256" => Sha3_256::digest(data).to_vec(),
    "sha3-384" => Sha3_384::digest(data).to_vec(),
    "sha3-512" => Sha3_512::digest(data).to_vec(),
    // SHAKE is an XOF (arbitrary-length output); this module picks the
    // conventional fixed output length used when SHAKE stands in for a
    // regular digest; 32 bytes for SHAKE128, 64 for SHAKE256 (twice
    // their security level in bytes, matching common usage e.g. in
    // Ethereum/Keccak-family tooling).
    "shake128" => {
      let mut h = Shake128::default();
      h.update(data);
      let mut out = vec![0u8; 32];
      h.finalize_xof().read(&mut out);
      out
    },
    "shake256" => {
      let mut h = Shake256::default();
      h.update(data);
      let mut out = vec![0u8; 64];
      h.finalize_xof().read(&mut out);
      out
    },
    "ripemd160" => Ripemd160::digest(data).to_vec(),
    "whirlpool" => Whirlpool::digest(data).to_vec(),
    "blake2s256" => Blake2s256::digest(data).to_vec(),
    "blake2b512" => Blake2b512::digest(data).to_vec(),
    "sm3" => Sm3::digest(data).to_vec(),
    other => {
      return Err(format!(
        "hash(): unsupported or unrecognized algorithm '{}'",
        other
      ));
    },
  })
}

// FNV; non-cryptographic, but part of the documented algorithm family

fn fnv1_32(data: &[u8]) -> u32 {
  let mut h: u32 = 0x811c_9dc5;
  for &b in data {
    h = h.wrapping_mul(0x0100_0193);
    h ^= b as u32;
  }
  h
}

fn fnv1a_32(data: &[u8]) -> u32 {
  let mut h: u32 = 0x811c_9dc5;
  for &b in data {
    h ^= b as u32;
    h = h.wrapping_mul(0x0100_0193);
  }
  h
}

fn fnv1_64(data: &[u8]) -> u64 {
  let mut h: u64 = 0xcbf2_9ce4_8422_2325;
  for &b in data {
    h = h.wrapping_mul(0x0000_0100_0000_01b3);
    h ^= b as u64;
  }
  h
}

fn fnv1a_64(data: &[u8]) -> u64 {
  let mut h: u64 = 0xcbf2_9ce4_8422_2325;
  for &b in data {
    h ^= b as u64;
    h = h.wrapping_mul(0x0000_0100_0000_01b3);
  }
  h
}

// Natives

/// `_hash.id(value)`. A class can override the result via a `to_hash`
/// decorator method (per `hash.zu`'s own doc comment); everything else
/// falls back to the exact same content-equality hash this VM's dict
/// implementation itself uses (`vm::object::DictKey`), so
/// `hash.id(a) == hash.id(b)` iff `a` and `b` would collide as dict keys.
fn id_fn(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 1);
  use std::hash::{Hash, Hasher};

  let v = ctx.args[0];

  if v.is_instance() {
    let method = {
      let class = v.as_instance().class.as_class();
      class.methods.get("to_hash").copied()
    };
    if let Some(m) = method {
      return ctx
        .vm
        .call_value(m, &[v])
        .map_err(|e| ctx.vm.describe_error(e));
    }
  }

  let mut hasher = rustc_hash::FxHasher::default();
  DictKey(v).hash(&mut hasher);
  Ok(Value::number(hasher.finish() as f64))
}

/// `_hash.hash(algorithm, data)` -> raw digest `bytes`. See
/// `digest_bytes`'s own doc comment for exactly which algorithm names are
/// handled here versus by a dedicated native.
fn hash_fn(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 2);
  enforce_arg_type!(ctx, 0, ArgType::String);

  let algorithm = ctx.args[0].as_str().to_lowercase();
  let data = data_bytes(ctx.args[1])?;
  let out = digest_bytes(&algorithm, &data)?;
  Ok(ctx.heap().alloc_bytes(out))
}

fn fnv1_fn(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 1);
  let data = data_bytes(ctx.args[0])?;
  let out = fnv1_32(&data).to_be_bytes().to_vec();
  Ok(ctx.heap().alloc_bytes(out))
}

fn fnv1_64_fn(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 1);
  let data = data_bytes(ctx.args[0])?;
  let out = fnv1_64(&data).to_be_bytes().to_vec();
  Ok(ctx.heap().alloc_bytes(out))
}

fn fnv1a_fn(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 1);
  let data = data_bytes(ctx.args[0])?;
  let out = fnv1a_32(&data).to_be_bytes().to_vec();
  Ok(ctx.heap().alloc_bytes(out))
}

fn fnv1a_64_fn(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 1);
  let data = data_bytes(ctx.args[0])?;
  let out = fnv1a_64(&data).to_be_bytes().to_vec();
  Ok(ctx.heap().alloc_bytes(out))
}

/// `_hash.gost(data)`; GOST R 34.11-94 using the CryptoPro S-box
/// parameter set, the most common real-world GOST94 variant.
fn gost_fn(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 1);
  use gost94::Gost94CryptoPro;

  let data = data_bytes(ctx.args[0])?;
  let out = Gost94CryptoPro::digest(&data).to_vec();
  Ok(ctx.heap().alloc_bytes(out))
}
