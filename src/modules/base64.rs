//! `base64` builtin module; converts between `bytes` values and
//! standard (RFC 4648, padded) base64-encoded strings.
//!
//! Reuses the same `base64` crate already vendored for
//! `src/modules/crypto.rs` (see that module's `x25519_der` helper for
//! an existing call site of the same `Engine` API), so this adds no
//! new dependency.

use base64::Engine;

use crate::builtins::enforce::ArgType;
use crate::modules::{BuiltinModuleDef, native};
use crate::vm::object::ZuriContext;
use crate::vm::value::Value;
use crate::vm::vm::VM;
use crate::{enforce_arg_count, enforce_arg_type};

pub static MODULE: BuiltinModuleDef = BuiltinModuleDef {
  name: "base64",
  build,
};

fn build(vm: &mut VM) -> Vec<(&'static str, Value)> {
  vec![
    ("encode", native(vm, "encode", 1, false, encode_fn)),
    ("decode", native(vm, "decode", 1, false, decode_fn)),
  ]
}

/// `base64.encode(data: bytes) -> string`; standard, padded base64
/// alphabet (`A-Za-z0-9+/=`).
fn encode_fn(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 1);
  enforce_arg_type!(ctx, 0, ArgType::Bytes);

  let data = ctx.args[0].as_bytes();
  let encoded = base64::engine::general_purpose::STANDARD.encode(&data);
  Ok(ctx.heap().alloc_string(encoded))
}

/// `base64.decode(str: string) -> bytes`. Errors (via the ordinary
/// native-function error path, becoming a catchable `Error`) on
/// malformed base64; wrong padding, invalid alphabet characters, or
/// an incomplete final group.
fn decode_fn(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 1);
  enforce_arg_type!(ctx, 0, ArgType::String);

  let s = ctx.args[0].as_str();
  let decoded = base64::engine::general_purpose::STANDARD
    .decode(s)
    .map_err(|e| format!("base64.decode(): invalid base64 string: {}", e))?;
  Ok(ctx.heap().alloc_bytes(decoded))
}
