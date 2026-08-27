//! `_crypto` builtin module; native primitives backing `libs/crypto.zu`.
//!
//! `libs/crypto.zu` is the user-facing `crypto` module: it defines
//! `CryptoError`, the `aes_gcm`/`aes_cbc`/`chacha20`/`rsa`/`ecdsa`/
//! `ed25519`/`x25519`/`argon2` namespace objects, and `hkdf`/`random_bytes`,
//! all as thin Zuri wrappers that immediately delegate to `_crypto.*`
//! natives (see that file's own doc comments for the exact call shape
//! each wrapper uses). This module is that native backing.
//!
//! ## Key encoding
//!
//! Every asymmetric key is exchanged with Zuri code as a PEM string:
//! PKCS#8 (`-----BEGIN PRIVATE KEY-----`) for private keys, SubjectPublicKeyInfo
//! (`-----BEGIN PUBLIC KEY-----`) for public keys; the same formats
//! OpenSSL produces and reads by default, so keys generated here are
//! usable with any other standard tool. X25519 has no crate-level PKCS8/
//! SPKI support in `x25519-dalek`, so this module hand-encodes the (fixed-
//! size, RFC 8410) DER for that one case: see `x25519_der`.

use aes_gcm::aead::{Aead, KeyInit, Payload};
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::{
  Algorithm as Argon2Algorithm, Argon2, Params as Argon2Params, Version as Argon2Version,
};
use cbc::cipher::block_padding::Pkcs7;
use cbc::cipher::{BlockDecryptMut, BlockEncryptMut, KeyIvInit};
use ecdsa::signature::{Signer as EcdsaSigner, Verifier as EcdsaVerifier};
use ed25519_dalek::pkcs8::{
  DecodePrivateKey as EdDecodePrivateKey, DecodePublicKey as EdDecodePublicKey,
  EncodePrivateKey as EdEncodePrivateKey, EncodePublicKey as EdEncodePublicKey,
};
use hkdf::Hkdf;
use p256::pkcs8::LineEnding;
use rand::rngs::OsRng;
use rand_core::RngCore;
use rsa::signature::{RandomizedSigner, SignatureEncoding, Verifier as RsaVerifierTrait};
use sha2::Sha256;

use crate::builtins::enforce::ArgType;
use crate::modules::{BuiltinModuleDef, native};
use crate::vm::object::ZuriContext;
use crate::vm::value::Value;
use crate::vm::vm::VM;
use crate::{enforce_arg_count, enforce_arg_range, enforce_arg_type, enforce_arg_type_opt};

pub static MODULE: BuiltinModuleDef = BuiltinModuleDef {
  name: "_crypto",
  build,
};

fn build(vm: &mut VM) -> Vec<(&'static str, Value)> {
  vec![
    (
      "random_bytes",
      native(vm, "random_bytes", 1, false, random_bytes_fn),
    ),
    // AES-GCM
    (
      "aes_gcm_encrypt",
      native(vm, "aes_gcm_encrypt", 3, true, aes_gcm_encrypt_fn),
    ),
    (
      "aes_gcm_decrypt",
      native(vm, "aes_gcm_decrypt", 3, true, aes_gcm_decrypt_fn),
    ),
    // AES-CBC
    (
      "aes_cbc_encrypt",
      native(vm, "aes_cbc_encrypt", 3, false, aes_cbc_encrypt_fn),
    ),
    (
      "aes_cbc_decrypt",
      native(vm, "aes_cbc_decrypt", 3, false, aes_cbc_decrypt_fn),
    ),
    // ChaCha20-Poly1305
    (
      "chacha20_encrypt",
      native(vm, "chacha20_encrypt", 3, true, chacha20_encrypt_fn),
    ),
    (
      "chacha20_decrypt",
      native(vm, "chacha20_decrypt", 3, true, chacha20_decrypt_fn),
    ),
    // RSA
    (
      "rsa_generate",
      native(vm, "rsa_generate", 1, false, rsa_generate_fn),
    ),
    (
      "rsa_encrypt",
      native(vm, "rsa_encrypt", 2, false, rsa_encrypt_fn),
    ),
    (
      "rsa_decrypt",
      native(vm, "rsa_decrypt", 2, false, rsa_decrypt_fn),
    ),
    ("rsa_sign", native(vm, "rsa_sign", 2, false, rsa_sign_fn)),
    (
      "rsa_verify",
      native(vm, "rsa_verify", 3, false, rsa_verify_fn),
    ),
    // ECDSA
    (
      "ecdsa_generate",
      native(vm, "ecdsa_generate", 1, false, ecdsa_generate_fn),
    ),
    (
      "ecdsa_sign",
      native(vm, "ecdsa_sign", 2, false, ecdsa_sign_fn),
    ),
    (
      "ecdsa_verify",
      native(vm, "ecdsa_verify", 3, false, ecdsa_verify_fn),
    ),
    // Ed25519
    (
      "ed25519_generate",
      native(vm, "ed25519_generate", 0, false, ed25519_generate_fn),
    ),
    (
      "ed25519_sign",
      native(vm, "ed25519_sign", 2, false, ed25519_sign_fn),
    ),
    (
      "ed25519_verify",
      native(vm, "ed25519_verify", 3, false, ed25519_verify_fn),
    ),
    // X25519
    (
      "x25519_generate",
      native(vm, "x25519_generate", 0, false, x25519_generate_fn),
    ),
    (
      "x25519_exchange",
      native(vm, "x25519_exchange", 2, false, x25519_exchange_fn),
    ),
    // Argon2id
    (
      "argon2_hash",
      native(vm, "argon2_hash", 2, true, argon2_hash_fn),
    ),
    (
      "argon2_verify",
      native(vm, "argon2_verify", 2, false, argon2_verify_fn),
    ),
    // HKDF-SHA256
    ("hkdf", native(vm, "hkdf", 4, false, hkdf_fn)),
  ]
}

// Shared helpers

fn crypto_err(context: &str, e: impl std::fmt::Display) -> String {
  format!("crypto: {}: {}", context, e)
}

fn make_keypair_dict(ctx: &mut ZuriContext, private_pem: String, public_pem: String) -> Value {
  let priv_key = ctx.heap().alloc_string("private_pem");
  let priv_val = ctx.heap().alloc_string(private_pem);
  let pub_key = ctx.heap().alloc_string("public_pem");
  let pub_val = ctx.heap().alloc_string(public_pem);
  ctx
    .heap()
    .alloc_dict(vec![(priv_key, priv_val), (pub_key, pub_val)])
}

// random_bytes

fn random_bytes_fn(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 1);
  enforce_arg_type!(ctx, 0, ArgType::Int);

  let n = ctx.args[0].as_int();
  if !(1..=65536).contains(&n) {
    return Err("random_bytes(): n must be an integer between 1 and 65536".to_string());
  }

  let mut buf = vec![0u8; n as usize];
  OsRng.fill_bytes(&mut buf);
  Ok(ctx.heap().alloc_bytes(buf))
}

// AES-GCM

fn aes_gcm_seal(key: &[u8], iv: &[u8], pt: &[u8], aad: &[u8]) -> Result<Vec<u8>, String> {
  use aes_gcm::aead::generic_array::typenum::U12;
  use aes_gcm::{Aes128Gcm, Aes256Gcm, AesGcm, Nonce};
  type Aes192Gcm = AesGcm<aes::Aes192, U12>;

  if iv.len() != 12 {
    return Err("aes_gcm: iv must be exactly 12 bytes".to_string());
  }
  let nonce = Nonce::from_slice(iv);
  let payload = Payload { msg: pt, aad };

  match key.len() {
    16 => Aes128Gcm::new_from_slice(key)
      .map_err(|e| crypto_err("invalid key", e))?
      .encrypt(nonce, payload)
      .map_err(|_| "aes_gcm: encryption failed".to_string()),
    24 => Aes192Gcm::new_from_slice(key)
      .map_err(|e| crypto_err("invalid key", e))?
      .encrypt(nonce, payload)
      .map_err(|_| "aes_gcm: encryption failed".to_string()),
    32 => Aes256Gcm::new_from_slice(key)
      .map_err(|e| crypto_err("invalid key", e))?
      .encrypt(nonce, payload)
      .map_err(|_| "aes_gcm: encryption failed".to_string()),
    n => Err(format!(
      "aes_gcm: key must be 16, 24, or 32 bytes, got {}",
      n
    )),
  }
}

fn aes_gcm_open(key: &[u8], iv: &[u8], ct: &[u8], aad: &[u8]) -> Result<Vec<u8>, String> {
  use aes_gcm::aead::generic_array::typenum::U12;
  use aes_gcm::{Aes128Gcm, Aes256Gcm, AesGcm, Nonce};
  type Aes192Gcm = AesGcm<aes::Aes192, U12>;

  if iv.len() != 12 {
    return Err("aes_gcm: iv must be exactly 12 bytes".to_string());
  }
  let nonce = Nonce::from_slice(iv);
  let payload = Payload { msg: ct, aad };
  let fail =
    || "aes_gcm: authentication failed (wrong key/iv/aad or corrupt ciphertext)".to_string();

  match key.len() {
    16 => Aes128Gcm::new_from_slice(key)
      .map_err(|e| crypto_err("invalid key", e))?
      .decrypt(nonce, payload)
      .map_err(|_| fail()),
    24 => Aes192Gcm::new_from_slice(key)
      .map_err(|e| crypto_err("invalid key", e))?
      .decrypt(nonce, payload)
      .map_err(|_| fail()),
    32 => Aes256Gcm::new_from_slice(key)
      .map_err(|e| crypto_err("invalid key", e))?
      .decrypt(nonce, payload)
      .map_err(|_| fail()),
    n => Err(format!(
      "aes_gcm: key must be 16, 24, or 32 bytes, got {}",
      n
    )),
  }
}

fn read_aad(ctx: &ZuriContext, idx: usize) -> Result<Vec<u8>, String> {
  match ctx.args.get(idx) {
    None => Ok(Vec::new()),
    Some(v) if v.is_nil() => Ok(Vec::new()),
    Some(v) if v.is_bytes() => Ok(v.as_bytes()),
    Some(v) => Err(format!("aad must be bytes or nil, got {}", v.type_name())),
  }
}

fn aes_gcm_encrypt_fn(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_range!(ctx, 3, 4);
  enforce_arg_type!(ctx, 0, ArgType::Bytes);
  enforce_arg_type!(ctx, 1, ArgType::Bytes);
  enforce_arg_type!(ctx, 2, ArgType::Bytes);

  let key = ctx.args[0].as_bytes();
  let iv = ctx.args[1].as_bytes();
  let pt = ctx.args[2].as_bytes();
  let aad = read_aad(ctx, 3)?;

  let ct = aes_gcm_seal(&key, &iv, &pt, &aad)?;
  Ok(ctx.heap().alloc_bytes(ct))
}

fn aes_gcm_decrypt_fn(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_range!(ctx, 3, 4);
  enforce_arg_type!(ctx, 0, ArgType::Bytes);
  enforce_arg_type!(ctx, 1, ArgType::Bytes);
  enforce_arg_type!(ctx, 2, ArgType::Bytes);

  let key = ctx.args[0].as_bytes();
  let iv = ctx.args[1].as_bytes();
  let ct = ctx.args[2].as_bytes();
  let aad = read_aad(ctx, 3)?;

  let pt = aes_gcm_open(&key, &iv, &ct, &aad)?;
  Ok(ctx.heap().alloc_bytes(pt))
}

// AES-CBC (PKCS#7)

/// `cipher` 0.4's `BlockEncryptMut`/`BlockDecryptMut` only give a
/// BUFFER-based padded API (`encrypt_padded_mut`/`decrypt_padded_mut`,
/// operating in place on a caller-supplied `&mut [u8]` and handing back a
/// sub-slice of it); there is no `_vec_mut` convenience method on
/// `cbc::Encryptor`/`Decryptor` themselves. This builds that buffer by
/// hand: encryption needs room for up to one extra PKCS#7 block
/// (`pt.len()` is never itself a valid final length once padded, so at
/// least one full block of padding is always added), decryption can
/// reuse the ciphertext's own length as an upper bound (padding removal
/// only ever shrinks it).
fn aes_cbc_encrypt_bytes(key: &[u8], iv: &[u8], pt: &[u8]) -> Result<Vec<u8>, String> {
  use aes::{Aes128, Aes192, Aes256};
  use cbc::Encryptor;

  const BLOCK_SIZE: usize = 16;
  if iv.len() != 16 {
    return Err("aes_cbc: iv must be exactly 16 bytes".to_string());
  }

  let mut buf = vec![0u8; pt.len() + BLOCK_SIZE];
  buf[..pt.len()].copy_from_slice(pt);
  let pad_err = || "aes_cbc: padding error".to_string();

  let ct_len = match key.len() {
    16 => Encryptor::<Aes128>::new_from_slices(key, iv)
      .map_err(|e| crypto_err("invalid key/iv", e))?
      .encrypt_padded_mut::<Pkcs7>(&mut buf, pt.len())
      .map_err(|_| pad_err())?
      .len(),
    24 => Encryptor::<Aes192>::new_from_slices(key, iv)
      .map_err(|e| crypto_err("invalid key/iv", e))?
      .encrypt_padded_mut::<Pkcs7>(&mut buf, pt.len())
      .map_err(|_| pad_err())?
      .len(),
    32 => Encryptor::<Aes256>::new_from_slices(key, iv)
      .map_err(|e| crypto_err("invalid key/iv", e))?
      .encrypt_padded_mut::<Pkcs7>(&mut buf, pt.len())
      .map_err(|_| pad_err())?
      .len(),
    n => {
      return Err(format!(
        "aes_cbc: key must be 16, 24, or 32 bytes, got {}",
        n
      ));
    },
  };
  buf.truncate(ct_len);
  Ok(buf)
}

fn aes_cbc_decrypt_bytes(key: &[u8], iv: &[u8], ct: &[u8]) -> Result<Vec<u8>, String> {
  use aes::{Aes128, Aes192, Aes256};
  use cbc::Decryptor;

  if iv.len() != 16 {
    return Err("aes_cbc: iv must be exactly 16 bytes".to_string());
  }
  let fail = || "aes_cbc: decryption failed (wrong key/iv or corrupt padding)".to_string();

  let mut buf = ct.to_vec();
  let pt_len = match key.len() {
    16 => Decryptor::<Aes128>::new_from_slices(key, iv)
      .map_err(|e| crypto_err("invalid key/iv", e))?
      .decrypt_padded_mut::<Pkcs7>(&mut buf)
      .map_err(|_| fail())?
      .len(),
    24 => Decryptor::<Aes192>::new_from_slices(key, iv)
      .map_err(|e| crypto_err("invalid key/iv", e))?
      .decrypt_padded_mut::<Pkcs7>(&mut buf)
      .map_err(|_| fail())?
      .len(),
    32 => Decryptor::<Aes256>::new_from_slices(key, iv)
      .map_err(|e| crypto_err("invalid key/iv", e))?
      .decrypt_padded_mut::<Pkcs7>(&mut buf)
      .map_err(|_| fail())?
      .len(),
    n => {
      return Err(format!(
        "aes_cbc: key must be 16, 24, or 32 bytes, got {}",
        n
      ));
    },
  };
  buf.truncate(pt_len);
  Ok(buf)
}

fn aes_cbc_encrypt_fn(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 3);
  enforce_arg_type!(ctx, 0, ArgType::Bytes);
  enforce_arg_type!(ctx, 1, ArgType::Bytes);
  enforce_arg_type!(ctx, 2, ArgType::Bytes);

  let key = ctx.args[0].as_bytes();
  let iv = ctx.args[1].as_bytes();
  let pt = ctx.args[2].as_bytes();
  let ct = aes_cbc_encrypt_bytes(&key, &iv, &pt)?;
  Ok(ctx.heap().alloc_bytes(ct))
}

fn aes_cbc_decrypt_fn(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 3);
  enforce_arg_type!(ctx, 0, ArgType::Bytes);
  enforce_arg_type!(ctx, 1, ArgType::Bytes);
  enforce_arg_type!(ctx, 2, ArgType::Bytes);

  let key = ctx.args[0].as_bytes();
  let iv = ctx.args[1].as_bytes();
  let ct = ctx.args[2].as_bytes();
  let pt = aes_cbc_decrypt_bytes(&key, &iv, &ct)?;
  Ok(ctx.heap().alloc_bytes(pt))
}

// ChaCha20-Poly1305

fn chacha20_encrypt_fn(ctx: &mut ZuriContext) -> Result<Value, String> {
  use chacha20poly1305::{ChaCha20Poly1305, KeyInit as ChaKeyInit, Nonce};

  enforce_arg_range!(ctx, 3, 4);
  enforce_arg_type!(ctx, 0, ArgType::Bytes);
  enforce_arg_type!(ctx, 1, ArgType::Bytes);
  enforce_arg_type!(ctx, 2, ArgType::Bytes);

  let key = ctx.args[0].as_bytes();
  let iv = ctx.args[1].as_bytes();
  let pt = ctx.args[2].as_bytes();
  let aad = read_aad(ctx, 3)?;

  if key.len() != 32 {
    return Err(format!(
      "chacha20: key must be exactly 32 bytes, got {}",
      key.len()
    ));
  }
  if iv.len() != 12 {
    return Err(format!(
      "chacha20: iv must be exactly 12 bytes, got {}",
      iv.len()
    ));
  }

  let cipher = ChaCha20Poly1305::new_from_slice(&key).map_err(|e| crypto_err("invalid key", e))?;
  let nonce = Nonce::from_slice(&iv);
  let ct = cipher
    .encrypt(
      nonce,
      Payload {
        msg: &pt,
        aad: &aad,
      },
    )
    .map_err(|_| "chacha20: encryption failed".to_string())?;
  Ok(ctx.heap().alloc_bytes(ct))
}

fn chacha20_decrypt_fn(ctx: &mut ZuriContext) -> Result<Value, String> {
  use chacha20poly1305::{ChaCha20Poly1305, KeyInit as ChaKeyInit, Nonce};

  enforce_arg_range!(ctx, 3, 4);
  enforce_arg_type!(ctx, 0, ArgType::Bytes);
  enforce_arg_type!(ctx, 1, ArgType::Bytes);
  enforce_arg_type!(ctx, 2, ArgType::Bytes);

  let key = ctx.args[0].as_bytes();
  let iv = ctx.args[1].as_bytes();
  let ct = ctx.args[2].as_bytes();
  let aad = read_aad(ctx, 3)?;

  if key.len() != 32 {
    return Err(format!(
      "chacha20: key must be exactly 32 bytes, got {}",
      key.len()
    ));
  }
  if iv.len() != 12 {
    return Err(format!(
      "chacha20: iv must be exactly 12 bytes, got {}",
      iv.len()
    ));
  }

  let cipher = ChaCha20Poly1305::new_from_slice(&key).map_err(|e| crypto_err("invalid key", e))?;
  let nonce = Nonce::from_slice(&iv);
  let pt = cipher
    .decrypt(
      nonce,
      Payload {
        msg: &ct,
        aad: &aad,
      },
    )
    .map_err(|_| {
      "chacha20: authentication failed (wrong key/iv/aad or corrupt ciphertext)".to_string()
    })?;
  Ok(ctx.heap().alloc_bytes(pt))
}

// RSA (OAEP encryption, PSS signatures)

fn rsa_generate_fn(ctx: &mut ZuriContext) -> Result<Value, String> {
  use rsa::pkcs8::{EncodePrivateKey, EncodePublicKey};
  use rsa::{RsaPrivateKey, RsaPublicKey};

  enforce_arg_count!(ctx, 1);
  enforce_arg_type!(ctx, 0, ArgType::Int);

  let bits = ctx.args[0].as_int();
  if bits != 2048 && bits != 4096 {
    return Err("rsa_generate(): bits must be 2048 or 4096".to_string());
  }

  let priv_key = RsaPrivateKey::new(&mut OsRng, bits as usize)
    .map_err(|e| crypto_err("key generation failed", e))?;
  let pub_key = RsaPublicKey::from(&priv_key);

  let priv_pem = priv_key
    .to_pkcs8_pem(LineEnding::LF)
    .map_err(|e| crypto_err("could not encode private key", e))?
    .to_string();
  let pub_pem = pub_key
    .to_public_key_pem(LineEnding::LF)
    .map_err(|e| crypto_err("could not encode public key", e))?;

  Ok(make_keypair_dict(ctx, priv_pem, pub_pem))
}

fn rsa_encrypt_fn(ctx: &mut ZuriContext) -> Result<Value, String> {
  use rsa::pkcs8::DecodePublicKey;
  use rsa::{Oaep, RsaPublicKey};

  enforce_arg_count!(ctx, 2);
  enforce_arg_type!(ctx, 0, ArgType::String);
  enforce_arg_type!(ctx, 1, ArgType::Bytes);

  let pub_key = RsaPublicKey::from_public_key_pem(ctx.args[0].as_str())
    .map_err(|e| crypto_err("invalid public key", e))?;
  let pt = ctx.args[1].as_bytes();

  let ct = pub_key
    .encrypt(&mut OsRng, Oaep::new::<Sha256>(), &pt)
    .map_err(|e| crypto_err("encryption failed", e))?;
  Ok(ctx.heap().alloc_bytes(ct))
}

fn rsa_decrypt_fn(ctx: &mut ZuriContext) -> Result<Value, String> {
  use rsa::pkcs8::DecodePrivateKey;
  use rsa::{Oaep, RsaPrivateKey};

  enforce_arg_count!(ctx, 2);
  enforce_arg_type!(ctx, 0, ArgType::String);
  enforce_arg_type!(ctx, 1, ArgType::Bytes);

  let priv_key = RsaPrivateKey::from_pkcs8_pem(ctx.args[0].as_str())
    .map_err(|e| crypto_err("invalid private key", e))?;
  let ct = ctx.args[1].as_bytes();

  let pt = priv_key
    .decrypt(Oaep::new::<Sha256>(), &ct)
    .map_err(|e| crypto_err("decryption failed", e))?;
  Ok(ctx.heap().alloc_bytes(pt))
}

fn rsa_sign_fn(ctx: &mut ZuriContext) -> Result<Value, String> {
  use rsa::RsaPrivateKey;
  use rsa::pkcs8::DecodePrivateKey;
  use rsa::pss::SigningKey;

  enforce_arg_count!(ctx, 2);
  enforce_arg_type!(ctx, 0, ArgType::String);
  enforce_arg_type!(ctx, 1, ArgType::Bytes);

  let priv_key = RsaPrivateKey::from_pkcs8_pem(ctx.args[0].as_str())
    .map_err(|e| crypto_err("invalid private key", e))?;
  let message = ctx.args[1].as_bytes();

  let signing_key = SigningKey::<Sha256>::new(priv_key);
  let sig = signing_key.sign_with_rng(&mut OsRng, &message);
  Ok(ctx.heap().alloc_bytes(sig.to_vec()))
}

fn rsa_verify_fn(ctx: &mut ZuriContext) -> Result<Value, String> {
  use rsa::RsaPublicKey;
  use rsa::pkcs8::DecodePublicKey;
  use rsa::pss::{Signature as PssSignature, VerifyingKey};

  enforce_arg_count!(ctx, 3);
  enforce_arg_type!(ctx, 0, ArgType::String);
  enforce_arg_type!(ctx, 1, ArgType::Bytes);
  enforce_arg_type!(ctx, 2, ArgType::Bytes);

  let pub_key = RsaPublicKey::from_public_key_pem(ctx.args[0].as_str())
    .map_err(|e| crypto_err("invalid public key", e))?;
  let message = ctx.args[1].as_bytes();
  let sig_bytes = ctx.args[2].as_bytes();

  let verifying_key = VerifyingKey::<Sha256>::new(pub_key);
  let ok = match PssSignature::try_from(sig_bytes.as_slice()) {
    Ok(sig) => RsaVerifierTrait::verify(&verifying_key, &message, &sig).is_ok(),
    Err(_) => false,
  };
  Ok(Value::bool(ok))
}

// ECDSA (P-256 / P-384)

fn ecdsa_generate_fn(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 1);
  enforce_arg_type!(ctx, 0, ArgType::String);

  let (priv_pem, pub_pem) = match ctx.args[0].as_str() {
    "P-256" => {
      use p256::ecdsa::{SigningKey, VerifyingKey};
      use p256::pkcs8::{EncodePrivateKey, EncodePublicKey};

      let sk = SigningKey::random(&mut OsRng);
      let vk = VerifyingKey::from(&sk);
      let priv_pem = sk
        .to_pkcs8_pem(LineEnding::LF)
        .map_err(|e| crypto_err("could not encode private key", e))?
        .to_string();
      let pub_pem = vk
        .to_public_key_pem(LineEnding::LF)
        .map_err(|e| crypto_err("could not encode public key", e))?;
      (priv_pem, pub_pem)
    },
    "P-384" => {
      use p384::ecdsa::{SigningKey, VerifyingKey};
      use p384::pkcs8::{EncodePrivateKey, EncodePublicKey};

      let sk = SigningKey::random(&mut OsRng);
      let vk = VerifyingKey::from(&sk);
      let priv_pem = sk
        .to_pkcs8_pem(LineEnding::LF)
        .map_err(|e| crypto_err("could not encode private key", e))?
        .to_string();
      let pub_pem = vk
        .to_public_key_pem(LineEnding::LF)
        .map_err(|e| crypto_err("could not encode public key", e))?;
      (priv_pem, pub_pem)
    },
    other => {
      return Err(format!(
        "ecdsa_generate(): curve must be \"P-256\" or \"P-384\", got \"{}\"",
        other
      ));
    },
  };

  Ok(make_keypair_dict(ctx, priv_pem, pub_pem))
}

/// Tries P-256 first, then P-384; the PEM's own embedded curve OID
/// makes the "wrong" curve's parse simply fail, so this is an unambiguous
/// way to recover which curve a given private key PEM belongs to without
/// requiring the caller to say so separately.
fn ecdsa_sign_fn(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 2);
  enforce_arg_type!(ctx, 0, ArgType::String);
  enforce_arg_type!(ctx, 1, ArgType::Bytes);

  let pem = ctx.args[0].as_str();
  let message = ctx.args[1].as_bytes();

  {
    use p256::ecdsa::{Signature, SigningKey};
    use p256::pkcs8::DecodePrivateKey;
    if let Ok(sk) = SigningKey::from_pkcs8_pem(pem) {
      let sig: Signature = sk.sign(&message);
      return Ok(ctx.heap().alloc_bytes(sig.to_der().as_bytes().to_vec()));
    }
  }
  {
    use p384::ecdsa::{Signature, SigningKey};
    use p384::pkcs8::DecodePrivateKey;
    if let Ok(sk) = SigningKey::from_pkcs8_pem(pem) {
      let sig: Signature = sk.sign(&message);
      return Ok(ctx.heap().alloc_bytes(sig.to_der().as_bytes().to_vec()));
    }
  }

  Err("ecdsa_sign(): invalid private key (expected a P-256 or P-384 PKCS#8 PEM)".to_string())
}

fn ecdsa_verify_fn(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 3);
  enforce_arg_type!(ctx, 0, ArgType::String);
  enforce_arg_type!(ctx, 1, ArgType::Bytes);
  enforce_arg_type!(ctx, 2, ArgType::Bytes);

  let pem = ctx.args[0].as_str();
  let message = ctx.args[1].as_bytes();
  let sig_bytes = ctx.args[2].as_bytes();

  {
    use p256::ecdsa::{Signature, VerifyingKey};
    use p256::pkcs8::DecodePublicKey;
    if let Ok(vk) = VerifyingKey::from_public_key_pem(pem) {
      let ok = Signature::from_der(&sig_bytes)
        .map(|sig| EcdsaVerifier::verify(&vk, &message, &sig).is_ok())
        .unwrap_or(false);
      return Ok(Value::bool(ok));
    }
  }
  {
    use p384::ecdsa::{Signature, VerifyingKey};
    use p384::pkcs8::DecodePublicKey;
    if let Ok(vk) = VerifyingKey::from_public_key_pem(pem) {
      let ok = Signature::from_der(&sig_bytes)
        .map(|sig| EcdsaVerifier::verify(&vk, &message, &sig).is_ok())
        .unwrap_or(false);
      return Ok(Value::bool(ok));
    }
  }

  Err("ecdsa_verify(): invalid public key (expected a P-256 or P-384 SPKI PEM)".to_string())
}

// Ed25519

fn ed25519_generate_fn(ctx: &mut ZuriContext) -> Result<Value, String> {
  use ed25519_dalek::SigningKey;

  enforce_arg_count!(ctx, 0);

  let signing_key = SigningKey::generate(&mut OsRng);
  let priv_pem = signing_key
    .to_pkcs8_pem(LineEnding::LF)
    .map_err(|e| crypto_err("could not encode private key", e))?
    .to_string();
  let pub_pem = signing_key
    .verifying_key()
    .to_public_key_pem(LineEnding::LF)
    .map_err(|e| crypto_err("could not encode public key", e))?;

  Ok(make_keypair_dict(ctx, priv_pem, pub_pem))
}

fn ed25519_sign_fn(ctx: &mut ZuriContext) -> Result<Value, String> {
  use ed25519_dalek::SigningKey;

  enforce_arg_count!(ctx, 2);
  enforce_arg_type!(ctx, 0, ArgType::String);
  enforce_arg_type!(ctx, 1, ArgType::Bytes);

  let signing_key = SigningKey::from_pkcs8_pem(ctx.args[0].as_str())
    .map_err(|e| crypto_err("invalid private key", e))?;
  let message = ctx.args[1].as_bytes();

  let sig = signing_key.sign(&message);
  Ok(ctx.heap().alloc_bytes(sig.to_bytes().to_vec()))
}

fn ed25519_verify_fn(ctx: &mut ZuriContext) -> Result<Value, String> {
  use ed25519_dalek::{Signature, VerifyingKey};

  enforce_arg_count!(ctx, 3);
  enforce_arg_type!(ctx, 0, ArgType::String);
  enforce_arg_type!(ctx, 1, ArgType::Bytes);
  enforce_arg_type!(ctx, 2, ArgType::Bytes);

  let verifying_key = VerifyingKey::from_public_key_pem(ctx.args[0].as_str())
    .map_err(|e| crypto_err("invalid public key", e))?;
  let message = ctx.args[1].as_bytes();
  let sig_bytes = ctx.args[2].as_bytes();

  let ok = Signature::from_slice(&sig_bytes)
    .map(|sig| verifying_key.verify(&message, &sig).is_ok())
    .unwrap_or(false);
  Ok(Value::bool(ok))
}

// X25519; hand-rolled RFC 8410 PKCS#8/SPKI DER (x25519-dalek has no
// pkcs8/spki support of its own).

mod x25519_der {
  //! Fixed-size (X25519 keys are always exactly 32 bytes) DER templates
  //! per RFC 8410; the OID for X25519 is `1.3.101.110`, which DER-encodes
  //! (as an AlgorithmIdentifier with no parameters) to the constant 7-byte
  //! `30 05 06 03 2B 65 6E` sequence embedded in both templates below.

  /// `PrivateKeyInfo { version=0, algorithm=id-X25519, privateKey=OCTET
  /// STRING(OCTET STRING(raw)) }`; 48 bytes total for a 32-byte key.
  const PKCS8_PREFIX: [u8; 16] = [
    0x30, 0x2e, 0x02, 0x01, 0x00, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x6e, 0x04, 0x22, 0x04, 0x20,
  ];

  /// `SubjectPublicKeyInfo { algorithm=id-X25519, subjectPublicKey=BIT
  /// STRING(raw) }`; 44 bytes total for a 32-byte key.
  const SPKI_PREFIX: [u8; 12] = [
    0x30, 0x2a, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x6e, 0x03, 0x21, 0x00,
  ];

  pub fn priv_to_der(raw: &[u8; 32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(48);
    out.extend_from_slice(&PKCS8_PREFIX);
    out.extend_from_slice(raw);
    out
  }

  pub fn priv_from_der(der: &[u8]) -> Result<[u8; 32], String> {
    if der.len() != 48 || der[..16] != PKCS8_PREFIX {
      return Err("invalid X25519 PKCS#8 private key".to_string());
    }
    let mut raw = [0u8; 32];
    raw.copy_from_slice(&der[16..48]);
    Ok(raw)
  }

  pub fn pub_to_der(raw: &[u8; 32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(44);
    out.extend_from_slice(&SPKI_PREFIX);
    out.extend_from_slice(raw);
    out
  }

  pub fn pub_from_der(der: &[u8]) -> Result<[u8; 32], String> {
    if der.len() != 44 || der[..12] != SPKI_PREFIX {
      return Err("invalid X25519 SPKI public key".to_string());
    }
    let mut raw = [0u8; 32];
    raw.copy_from_slice(&der[12..44]);
    Ok(raw)
  }

  pub fn pem_encode(label: &str, der: &[u8]) -> String {
    use base64::Engine;
    let b64 = base64::engine::general_purpose::STANDARD.encode(der);
    let mut out = format!("-----BEGIN {}-----\n", label);
    for chunk in b64.as_bytes().chunks(64) {
      out.push_str(std::str::from_utf8(chunk).unwrap());
      out.push('\n');
    }
    out.push_str(&format!("-----END {}-----\n", label));
    out
  }

  pub fn pem_decode(pem: &str, label: &str) -> Result<Vec<u8>, String> {
    use base64::Engine;
    let begin = format!("-----BEGIN {}-----", label);
    let end = format!("-----END {}-----", label);
    let start = pem
      .find(&begin)
      .ok_or_else(|| format!("expected a PEM block labeled '{}'", label))?
      + begin.len();
    let stop = pem
      .find(&end)
      .ok_or_else(|| "malformed PEM: missing END marker".to_string())?;
    let b64: String = pem[start..stop]
      .chars()
      .filter(|c| !c.is_whitespace())
      .collect();
    base64::engine::general_purpose::STANDARD
      .decode(b64)
      .map_err(|e| format!("malformed PEM body: {}", e))
  }
}

fn x25519_generate_fn(ctx: &mut ZuriContext) -> Result<Value, String> {
  use x25519_dalek::{PublicKey, StaticSecret};

  enforce_arg_count!(ctx, 0);

  let secret = StaticSecret::random_from_rng(OsRng);
  let public = PublicKey::from(&secret);

  let priv_pem =
    x25519_der::pem_encode("PRIVATE KEY", &x25519_der::priv_to_der(&secret.to_bytes()));
  let pub_pem = x25519_der::pem_encode("PUBLIC KEY", &x25519_der::pub_to_der(public.as_bytes()));

  Ok(make_keypair_dict(ctx, priv_pem, pub_pem))
}

fn x25519_exchange_fn(ctx: &mut ZuriContext) -> Result<Value, String> {
  use x25519_dalek::{PublicKey, StaticSecret};

  enforce_arg_count!(ctx, 2);
  enforce_arg_type!(ctx, 0, ArgType::String);
  enforce_arg_type!(ctx, 1, ArgType::String);

  let priv_der = x25519_der::pem_decode(ctx.args[0].as_str(), "PRIVATE KEY")?;
  let raw_priv = x25519_der::priv_from_der(&priv_der)?;
  let secret = StaticSecret::from(raw_priv);

  let pub_der = x25519_der::pem_decode(ctx.args[1].as_str(), "PUBLIC KEY")?;
  let raw_pub = x25519_der::pub_from_der(&pub_der)?;
  let peer_public = PublicKey::from(raw_pub);

  let shared = secret.diffie_hellman(&peer_public);
  Ok(ctx.heap().alloc_bytes(shared.to_bytes().to_vec()))
}

// Argon2id

fn argon2_hash_fn(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_range!(ctx, 2, 4);
  enforce_arg_type!(ctx, 0, ArgType::String);
  enforce_arg_type!(ctx, 1, ArgType::Bytes);
  enforce_arg_type_opt!(ctx, 3, ArgType::Bool);

  let password = ctx.args[0].as_str().to_string();
  let salt_bytes = ctx.args[1].as_bytes();
  if salt_bytes.len() < 8 {
    return Err("argon2_hash(): salt must be at least 8 bytes".to_string());
  }

  let mut t_cost: u32 = 3;
  let mut m_cost: u32 = 65536;
  let mut threads: u32 = 4;
  let mut hash_len: usize = 32;

  if let Some(opts) = ctx.args.get(2) {
    if !opts.is_nil() {
      if !opts.is_dict() {
        return Err(format!(
          "argon2_hash(): options must be a dict, got {}",
          opts.type_name()
        ));
      }
      for (k, v) in opts.as_dict() {
        if !k.is_string() || !v.is_int() {
          continue;
        }
        match k.as_str() {
          "t_cost" => t_cost = v.as_int() as u32,
          "m_cost" => m_cost = v.as_int() as u32,
          "threads" => threads = v.as_int() as u32,
          "hash_len" => hash_len = v.as_int() as usize,
          _ => {},
        }
      }
    }
  }

  let return_bytes = ctx.args.get(3).map(|v| !v.is_falsey()).unwrap_or(false);

  let params = Argon2Params::new(m_cost, t_cost, threads, Some(hash_len))
    .map_err(|e| crypto_err("invalid argon2 parameters", e))?;
  let argon2 = Argon2::new(Argon2Algorithm::Argon2id, Argon2Version::V0x13, params);

  let salt = SaltString::encode_b64(&salt_bytes).map_err(|e| crypto_err("invalid salt", e))?;
  let hash = argon2
    .hash_password(password.as_bytes(), &salt)
    .map_err(|e| crypto_err("hashing failed", e))?;

  if return_bytes {
    let raw = hash
      .hash
      .ok_or_else(|| "argon2_hash(): hasher produced no output".to_string())?;
    Ok(ctx.heap().alloc_bytes(raw.as_bytes().to_vec()))
  } else {
    Ok(ctx.heap().alloc_string(hash.to_string()))
  }
}

fn argon2_verify_fn(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 2);
  enforce_arg_type!(ctx, 0, ArgType::String);
  enforce_arg_type!(ctx, 1, ArgType::String);

  let encoded = ctx.args[0].as_str();
  let password = ctx.args[1].as_str();

  let parsed = PasswordHash::new(encoded).map_err(|e| crypto_err("invalid encoded hash", e))?;
  let ok = Argon2::default()
    .verify_password(password.as_bytes(), &parsed)
    .is_ok();
  Ok(Value::bool(ok))
}

// HKDF-SHA256

fn hkdf_fn(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 4);
  enforce_arg_type!(ctx, 0, ArgType::Bytes);
  enforce_arg_type!(ctx, 1, ArgType::Bytes);
  enforce_arg_type!(ctx, 2, ArgType::Bytes);
  enforce_arg_type!(ctx, 3, ArgType::Int);

  let ikm = ctx.args[0].as_bytes();
  let salt = ctx.args[1].as_bytes();
  let info = ctx.args[2].as_bytes();
  let length = ctx.args[3].as_int();

  if !(1..=8160).contains(&length) {
    return Err("hkdf(): length must be an integer between 1 and 8160".to_string());
  }

  let hk = Hkdf::<Sha256>::new(Some(&salt), &ikm);
  let mut okm = vec![0u8; length as usize];
  hk.expand(&info, &mut okm)
    .map_err(|_| "hkdf(): requested length is too large for this PRF".to_string())?;
  Ok(ctx.heap().alloc_bytes(okm))
}
