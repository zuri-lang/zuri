use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::crypto::{CryptoProvider, verify_tls12_signature, verify_tls13_signature};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, ServerName, UnixTime};
use rustls::{
  ClientConfig, ClientConnection, DigitallySignedStruct, RootCertStore, ServerConfig,
  ServerConnection, SignatureScheme, StreamOwned,
};
use x509_cert::der::Decode;
use x509_cert::ext::pkix::SubjectAltName;
use x509_cert::ext::pkix::name::GeneralName;

use crate::builtins::enforce::{
  ArgType, enforce_method_arg_count, enforce_method_arg_type, enforce_method_arg_type_any_of,
};
use crate::enforce_arg_count;
use crate::modules::net_tcp::{TCP_STREAM, TCP_STREAM_UPGRADED, ZuriTcp};
use crate::modules::{BuiltinModuleDef, native};
use crate::vm::object::ZuriContext;
use crate::vm::value::Value;
use crate::vm::vm::VM;

pub static MODULE: BuiltinModuleDef = BuiltinModuleDef {
  name: "_net_tls",
  build,
};

fn build(vm: &mut VM) -> Vec<(&'static str, Value)> {
  vec![
    (
      "tls_config_new",
      native(vm, "@new", 0, false, tls_config_new),
    ),
    (
      "tls_config_set_root_store",
      native(vm, "set_root_store", 2, false, tls_config_set_root_store),
    ),
    (
      "tls_config_add_ca_pem",
      native(vm, "add_ca_pem", 2, false, tls_config_add_ca_pem),
    ),
    (
      "tls_config_set_cert_chain",
      native(vm, "set_cert_chain", 3, false, tls_config_set_cert_chain),
    ),
    (
      "tls_config_require_client_cert",
      native(
        vm,
        "require_client_cert",
        2,
        false,
        tls_config_require_client_cert,
      ),
    ),
    (
      "tls_config_set_alpn",
      native(vm, "set_alpn", 2, false, tls_config_set_alpn),
    ),
    (
      "tls_config_set_versions",
      native(vm, "set_versions", 3, false, tls_config_set_versions),
    ),
    (
      "tls_config_set_insecure",
      native(vm, "set_insecure", 2, false, tls_config_set_insecure),
    ),
    (
      "tls_wrap_client",
      native(vm, "wrap_client", 3, false, tls_wrap_client),
    ),
    (
      "tls_wrap_server",
      native(vm, "wrap_server", 2, false, tls_wrap_server),
    ),
    ("tls_read", native(vm, "read", 2, false, tls_read)),
    (
      "tls_read_exact",
      native(vm, "read_exact", 2, false, tls_read_exact),
    ),
    (
      "tls_read_all",
      native(vm, "read_all", 1, false, tls_read_all),
    ),
    (
      "tls_read_as_string",
      native(vm, "read_as_string", 1, false, tls_read_as_string),
    ),
    ("tls_write", native(vm, "write", 2, true, tls_write)),
    (
      "tls_write_all",
      native(vm, "write_all", 2, true, tls_write_all),
    ),
    ("tls_flush", native(vm, "flush", 1, false, tls_flush)),
    (
      "tls_shutdown",
      native(vm, "shutdown", 1, false, tls_shutdown),
    ),
    ("tls_close", native(vm, "close", 1, false, tls_close)),
    (
      "tls_alpn_protocol",
      native(vm, "alpn_protocol", 1, false, tls_alpn_protocol),
    ),
    (
      "tls_peer_certificate_der",
      native(
        vm,
        "peer_certificate_der",
        1,
        false,
        tls_peer_certificate_der,
      ),
    ),
    (
      "tls_peer_certificate_subject",
      native(
        vm,
        "peer_certificate_subject",
        1,
        false,
        tls_peer_certificate_subject,
      ),
    ),
    (
      "tls_peer_certificate_issuer",
      native(
        vm,
        "peer_certificate_issuer",
        1,
        false,
        tls_peer_certificate_issuer,
      ),
    ),
    (
      "tls_peer_certificate_sans",
      native(
        vm,
        "peer_certificate_sans",
        1,
        false,
        tls_peer_certificate_sans,
      ),
    ),
    (
      "tls_peer_certificate_not_before",
      native(
        vm,
        "peer_certificate_not_before",
        1,
        false,
        tls_peer_certificate_not_before,
      ),
    ),
    (
      "tls_peer_certificate_not_after",
      native(
        vm,
        "peer_certificate_not_after",
        1,
        false,
        tls_peer_certificate_not_after,
      ),
    ),
  ]
}

const TLS_CONFIG: &str = "zuri::net::TlsConfig";
const TLS_STREAM: &str = "zuri::net::TlsStream";

/// The descriptor of the socket underneath a `TlsStream`, or `None`
/// when `value` is not one or has been closed. Readiness is a property
/// of that socket, not of the TLS session on top of it.
///
/// @see `net_tcp::descriptor_of` for why this is resolved on demand.
pub(crate) fn descriptor_of(value: Value) -> Option<crate::modules::net_tcp::Descriptor> {
  if !value.is_ptr_type(TLS_STREAM) {
    return None;
  }

  let cell = value.as_ptr_cell().borrow();
  let stream = cell.downcast_ref::<ZuriTlsStream>()?;

  #[cfg(unix)]
  use std::os::unix::io::AsRawFd;
  #[cfg(windows)]
  use std::os::windows::io::AsRawSocket;

  match stream {
    #[cfg(unix)]
    ZuriTlsStream::Client(s) => Some(s.sock.as_raw_fd()),
    #[cfg(unix)]
    ZuriTlsStream::Server(s) => Some(s.sock.as_raw_fd()),
    #[cfg(windows)]
    ZuriTlsStream::Client(s) => Some(s.sock.as_raw_socket()),
    #[cfg(windows)]
    ZuriTlsStream::Server(s) => Some(s.sock.as_raw_socket()),
    ZuriTlsStream::Closed => None,
  }
}
const TLS_STREAM_CLOSED: &str = "zuri::net::TlsStream::__closed__";

// ---------------------------------------------------------------------------
// TlsConfig: an accumulator for the handful of choices a handshake needs,
// finalized into an actual rustls ClientConfig/ServerConfig lazily, the
// first time it's asked to wrap a connection. Both sides are cached once
// built so a single TlsConfig can be reused for many connections without
// re-parsing certificates or rebuilding a verifier each time.
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
enum RootStoreMode {
  Bundled,
  Native,
}

struct ZuriTlsConfig {
  root_store_mode: RootStoreMode,
  extra_ca_pems: Vec<String>,
  cert_chain_pem: Option<String>,
  key_pem: Option<String>,
  require_client_cert: bool,
  alpn: Vec<Vec<u8>>,
  min_version: Option<u16>,
  max_version: Option<u16>,
  insecure: bool,
  client_config: Option<Arc<ClientConfig>>,
  server_config: Option<Arc<ServerConfig>>,
}

impl ZuriTlsConfig {
  fn new() -> Self {
    ZuriTlsConfig {
      root_store_mode: RootStoreMode::Bundled,
      extra_ca_pems: Vec::new(),
      cert_chain_pem: None,
      key_pem: None,
      require_client_cert: false,
      alpn: Vec::new(),
      min_version: None,
      max_version: None,
      insecure: false,
      client_config: None,
      server_config: None,
    }
  }

  /// Anything that changes what a handshake would negotiate invalidates
  /// whichever finalized configs are already cached, so the next
  /// connect/accept picks the new settings up instead of silently reusing
  /// a stale one.
  fn invalidate(&mut self) {
    self.client_config = None;
    self.server_config = None;
  }

  fn protocol_versions(&self) -> Vec<&'static rustls::SupportedProtocolVersion> {
    let mut versions = Vec::with_capacity(2);
    let want = |v: u16| {
      let above_min = self.min_version.is_none_or(|min| v >= min);
      let below_max = self.max_version.is_none_or(|max| v <= max);
      above_min && below_max
    };
    if want(13) {
      versions.push(&rustls::version::TLS13);
    }
    if want(12) {
      versions.push(&rustls::version::TLS12);
    }
    versions
  }

  fn build_root_store(&self) -> Result<RootCertStore, String> {
    let mut store = match self.root_store_mode {
      RootStoreMode::Bundled => RootCertStore {
        roots: webpki_roots::TLS_SERVER_ROOTS.to_vec(),
      },
      RootStoreMode::Native => {
        let result = rustls_native_certs::load_native_certs();
        let mut store = RootCertStore::empty();
        let (added, _) = store.add_parsable_certificates(result.certs);
        if added == 0 {
          return Err("tls: no usable certificates found in the OS-native trust store".to_string());
        }
        store
      },
    };
    for pem in &self.extra_ca_pems {
      let certs = parse_cert_chain_pem(pem)?;
      let (_, invalid) = store.add_parsable_certificates(certs);
      if invalid > 0 {
        return Err(format!(
          "tls: {invalid} certificate(s) in a custom CA bundle could not be parsed"
        ));
      }
    }
    Ok(store)
  }

  fn build_client_config(&mut self) -> Result<Arc<ClientConfig>, String> {
    if let Some(config) = &self.client_config {
      return Ok(config.clone());
    }

    let versions = self.protocol_versions();
    let builder = ClientConfig::builder_with_protocol_versions(&versions);

    let builder = if self.insecure {
      builder
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(NoServerCertVerification::new()))
    } else {
      let root_store = self.build_root_store()?;
      builder.with_root_certificates(root_store)
    };

    let mut config = match (&self.cert_chain_pem, &self.key_pem) {
      (Some(chain_pem), Some(key_pem)) => {
        let chain = parse_cert_chain_pem(chain_pem)?;
        let key = parse_private_key_pem(key_pem)?;
        builder
          .with_client_auth_cert(chain, key)
          .map_err(|e| format!("tls: invalid client certificate/key: {e}"))?
      },
      _ => builder.with_no_client_auth(),
    };
    config.alpn_protocols = self.alpn.clone();

    let config = Arc::new(config);
    self.client_config = Some(config.clone());
    Ok(config)
  }

  fn build_server_config(&mut self) -> Result<Arc<ServerConfig>, String> {
    if let Some(config) = &self.server_config {
      return Ok(config.clone());
    }

    let (chain_pem, key_pem) = self
      .cert_chain_pem
      .as_ref()
      .zip(self.key_pem.as_ref())
      .ok_or_else(|| {
        "tls: a server needs set_cert_chain() called before it can accept connections".to_string()
      })?;
    let chain = parse_cert_chain_pem(chain_pem)?;
    let key = parse_private_key_pem(key_pem)?;

    let versions = self.protocol_versions();
    let builder = ServerConfig::builder_with_protocol_versions(&versions);

    let builder = if self.require_client_cert {
      let root_store = self.build_root_store()?;
      let verifier = rustls::server::WebPkiClientVerifier::builder(Arc::new(root_store))
        .build()
        .map_err(|e| format!("tls: could not build a client certificate verifier: {e}"))?;
      builder.with_client_cert_verifier(verifier)
    } else {
      builder.with_no_client_auth()
    };

    let mut config = builder
      .with_single_cert(chain, key)
      .map_err(|e| format!("tls: invalid server certificate/key: {e}"))?;
    config.alpn_protocols = self.alpn.clone();

    let config = Arc::new(config);
    self.server_config = Some(config.clone());
    Ok(config)
  }
}

pub(crate) fn parse_cert_chain_pem(pem: &str) -> Result<Vec<CertificateDer<'static>>, String> {
  let mut reader = pem.as_bytes();
  let certs: Result<Vec<_>, _> = rustls_pemfile::certs(&mut reader).collect();
  let certs = certs.map_err(|e| format!("tls: could not parse certificate PEM: {e}"))?;
  if certs.is_empty() {
    return Err("tls: no certificates found in PEM input".to_string());
  }
  Ok(certs)
}

pub(crate) fn parse_private_key_pem(pem: &str) -> Result<PrivateKeyDer<'static>, String> {
  let mut reader = pem.as_bytes();
  rustls_pemfile::private_key(&mut reader)
    .map_err(|e| format!("tls: could not parse private key PEM: {e}"))?
    .ok_or_else(|| "tls: no private key found in PEM input".to_string())
}

/// A `ServerCertVerifier` that skips chain-of-trust and hostname checks
/// entirely. Still cryptographically verifies handshake signatures against
/// the certificate presented, so it isn't a total no-op, just an untrusted
/// one; wired up only when a `TlsConfig` is explicitly marked insecure,
/// which `tls.zu` documents as a development/testing-only escape hatch.
#[derive(Debug)]
struct NoServerCertVerification {
  provider: Arc<CryptoProvider>,
}

impl NoServerCertVerification {
  fn new() -> Self {
    NoServerCertVerification {
      provider: Arc::new(rustls::crypto::aws_lc_rs::default_provider()),
    }
  }
}

impl ServerCertVerifier for NoServerCertVerification {
  fn verify_server_cert(
    &self,
    _end_entity: &CertificateDer<'_>,
    _intermediates: &[CertificateDer<'_>],
    _server_name: &ServerName<'_>,
    _ocsp_response: &[u8],
    _now: UnixTime,
  ) -> Result<ServerCertVerified, rustls::Error> {
    Ok(ServerCertVerified::assertion())
  }

  fn verify_tls12_signature(
    &self,
    message: &[u8],
    cert: &CertificateDer<'_>,
    dss: &DigitallySignedStruct,
  ) -> Result<HandshakeSignatureValid, rustls::Error> {
    verify_tls12_signature(
      message,
      cert,
      dss,
      &self.provider.signature_verification_algorithms,
    )
  }

  fn verify_tls13_signature(
    &self,
    message: &[u8],
    cert: &CertificateDer<'_>,
    dss: &DigitallySignedStruct,
  ) -> Result<HandshakeSignatureValid, rustls::Error> {
    verify_tls13_signature(
      message,
      cert,
      dss,
      &self.provider.signature_verification_algorithms,
    )
  }

  fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
    self
      .provider
      .signature_verification_algorithms
      .supported_schemes()
  }
}

// ---------------------------------------------------------------------------
// TlsStream: an established (or dead) TLS connection over an owned TcpStream.
// ---------------------------------------------------------------------------

enum ZuriTlsStream {
  Client(StreamOwned<ClientConnection, TcpStream>),
  Server(StreamOwned<ServerConnection, TcpStream>),
  Closed,
}

impl ZuriTlsStream {
  fn read(&mut self, buf: &mut [u8]) -> Result<usize, String> {
    match self {
      ZuriTlsStream::Client(s) => s.read(buf).map_err(|e| e.to_string()),
      ZuriTlsStream::Server(s) => s.read(buf).map_err(|e| e.to_string()),
      ZuriTlsStream::Closed => Err("tls: stream is closed".to_string()),
    }
  }

  fn read_exact(&mut self, buf: &mut [u8]) -> Result<(), String> {
    match self {
      ZuriTlsStream::Client(s) => s.read_exact(buf).map_err(|e| e.to_string()),
      ZuriTlsStream::Server(s) => s.read_exact(buf).map_err(|e| e.to_string()),
      ZuriTlsStream::Closed => Err("tls: stream is closed".to_string()),
    }
  }

  fn read_to_end(&mut self, buf: &mut Vec<u8>) -> Result<usize, String> {
    match self {
      ZuriTlsStream::Client(s) => s.read_to_end(buf).map_err(|e| e.to_string()),
      ZuriTlsStream::Server(s) => s.read_to_end(buf).map_err(|e| e.to_string()),
      ZuriTlsStream::Closed => Err("tls: stream is closed".to_string()),
    }
  }

  fn read_to_string(&mut self, buf: &mut String) -> Result<usize, String> {
    match self {
      ZuriTlsStream::Client(s) => s.read_to_string(buf).map_err(|e| e.to_string()),
      ZuriTlsStream::Server(s) => s.read_to_string(buf).map_err(|e| e.to_string()),
      ZuriTlsStream::Closed => Err("tls: stream is closed".to_string()),
    }
  }

  fn write(&mut self, buf: &[u8]) -> Result<usize, String> {
    match self {
      ZuriTlsStream::Client(s) => s.write(buf).map_err(|e| e.to_string()),
      ZuriTlsStream::Server(s) => s.write(buf).map_err(|e| e.to_string()),
      ZuriTlsStream::Closed => Err("tls: stream is closed".to_string()),
    }
  }

  fn write_all(&mut self, buf: &[u8]) -> Result<(), String> {
    match self {
      ZuriTlsStream::Client(s) => s.write_all(buf).map_err(|e| e.to_string()),
      ZuriTlsStream::Server(s) => s.write_all(buf).map_err(|e| e.to_string()),
      ZuriTlsStream::Closed => Err("tls: stream is closed".to_string()),
    }
  }

  fn flush(&mut self) -> Result<(), String> {
    match self {
      ZuriTlsStream::Client(s) => s.flush().map_err(|e| e.to_string()),
      ZuriTlsStream::Server(s) => s.flush().map_err(|e| e.to_string()),
      ZuriTlsStream::Closed => Err("tls: stream is closed".to_string()),
    }
  }

  fn shutdown(&mut self) -> Result<(), String> {
    match self {
      ZuriTlsStream::Client(s) => {
        s.conn.send_close_notify();
        s.conn.complete_io(&mut s.sock).map_err(|e| e.to_string())?;
        Ok(())
      },
      ZuriTlsStream::Server(s) => {
        s.conn.send_close_notify();
        s.conn.complete_io(&mut s.sock).map_err(|e| e.to_string())?;
        Ok(())
      },
      ZuriTlsStream::Closed => Err("tls: stream is closed".to_string()),
    }
  }

  fn alpn_protocol(&self) -> Option<Vec<u8>> {
    match self {
      ZuriTlsStream::Client(s) => s.conn.alpn_protocol().map(|p| p.to_vec()),
      ZuriTlsStream::Server(s) => s.conn.alpn_protocol().map(|p| p.to_vec()),
      ZuriTlsStream::Closed => None,
    }
  }

  fn peer_certificate(&self) -> Option<CertificateDer<'static>> {
    match self {
      ZuriTlsStream::Client(s) => s.conn.peer_certificates().and_then(|c| c.first().cloned()),
      ZuriTlsStream::Server(s) => s.conn.peer_certificates().and_then(|c| c.first().cloned()),
      ZuriTlsStream::Closed => None,
    }
  }
}

// ---------------------------------------------------------------------------
// TlsConfig natives
// ---------------------------------------------------------------------------

fn tls_config_new(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 0);
  Ok(ctx.heap().alloc_ptr(TLS_CONFIG, ZuriTlsConfig::new()))
}

fn with_config_at<F, T>(ctx: &mut ZuriContext, idx: usize, f: F) -> Result<T, String>
where
  F: FnOnce(&mut ZuriTlsConfig) -> Result<T, String>,
{
  let mut ptr = ctx.args[idx].as_ptr_cell().borrow_mut();
  let config = ptr
    .downcast_mut::<ZuriTlsConfig>()
    .ok_or_else(|| "tls: expected a TlsConfig".to_string())?;
  f(config)
}

fn tls_config_set_root_store(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(TLS_CONFIG));
  enforce_method_arg_type!(ctx, 1, ArgType::String);

  let mode = match ctx.args[1].as_str() {
    "bundled" => RootStoreMode::Bundled,
    "native" => RootStoreMode::Native,
    other => {
      return Err(format!(
        "tls: unknown root store mode '{other}', expected 'bundled' or 'native'"
      ));
    },
  };
  with_config_at(ctx, 0, |config| {
    config.root_store_mode = mode;
    config.invalidate();
    Ok(Value::nil())
  })
}

fn tls_config_add_ca_pem(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(TLS_CONFIG));
  enforce_method_arg_type!(ctx, 1, ArgType::String);

  let pem = ctx.args[1].as_str().to_string();
  with_config_at(ctx, 0, |config| {
    config.extra_ca_pems.push(pem);
    config.invalidate();
    Ok(Value::nil())
  })
}

fn tls_config_set_cert_chain(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 2);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(TLS_CONFIG));
  enforce_method_arg_type!(ctx, 1, ArgType::String);
  enforce_method_arg_type!(ctx, 2, ArgType::String);

  let chain_pem = ctx.args[1].as_str().to_string();
  let key_pem = ctx.args[2].as_str().to_string();
  with_config_at(ctx, 0, |config| {
    config.cert_chain_pem = Some(chain_pem);
    config.key_pem = Some(key_pem);
    config.invalidate();
    Ok(Value::nil())
  })
}

fn tls_config_require_client_cert(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(TLS_CONFIG));
  enforce_method_arg_type!(ctx, 1, ArgType::Bool);

  let required = ctx.args[1].as_bool();
  with_config_at(ctx, 0, |config| {
    config.require_client_cert = required;
    config.invalidate();
    Ok(Value::nil())
  })
}

fn tls_config_set_alpn(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(TLS_CONFIG));
  enforce_method_arg_type!(ctx, 1, ArgType::List);

  let mut protocols = Vec::new();
  for item in ctx.args[1].as_list() {
    if !item.is_string() {
      return Err("tls: set_alpn() expects a list of strings".to_string());
    }
    protocols.push(item.as_str().as_bytes().to_vec());
  }
  with_config_at(ctx, 0, |config| {
    config.alpn = protocols;
    config.invalidate();
    Ok(Value::nil())
  })
}

fn version_number(ctx: &ZuriContext, idx: usize) -> Result<Option<u16>, String> {
  match ctx.args.get(idx) {
    None => Ok(None),
    Some(v) if v.is_nil() => Ok(None),
    Some(v) if v.is_string() => match v.as_str() {
      "1.2" => Ok(Some(12)),
      "1.3" => Ok(Some(13)),
      other => Err(format!(
        "tls: unknown protocol version '{other}', expected '1.2' or '1.3'"
      )),
    },
    Some(v) => Err(format!(
      "set_versions() expects a string or nil, got {}",
      v.type_name()
    )),
  }
}

fn tls_config_set_versions(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 2);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(TLS_CONFIG));

  let min = version_number(ctx, 1)?;
  let max = version_number(ctx, 2)?;
  with_config_at(ctx, 0, |config| {
    config.min_version = min;
    config.max_version = max;
    config.invalidate();
    Ok(Value::nil())
  })
}

fn tls_config_set_insecure(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(TLS_CONFIG));
  enforce_method_arg_type!(ctx, 1, ArgType::Bool);

  let insecure = ctx.args[1].as_bool();
  with_config_at(ctx, 0, |config| {
    config.insecure = insecure;
    config.invalidate();
    Ok(Value::nil())
  })
}

// ---------------------------------------------------------------------------
// Handshake natives
// ---------------------------------------------------------------------------

/// Takes ownership of a connected `TcpStream` out of a `ZuriTcp` instance
/// for a TLS handshake, and invalidates the plain `TcpStream` the same way
/// `close()` does. Used by both `tls_wrap_client` and `tls_wrap_server` -
/// this is the one operation STARTTLS-style upgrades and plain
/// connect-then-handshake TLS both boil down to.
fn take_tcp_stream(ctx: &mut ZuriContext, idx: usize) -> Result<TcpStream, String> {
  let mut ptr = ctx.args[idx].as_ptr_cell().borrow_mut();
  let tcp = ptr
    .downcast_mut::<ZuriTcp>()
    .ok_or_else(|| "tls: expected a TcpStream".to_string())?;
  let stream = tcp.take_stream()?;
  ptr.type_name = TCP_STREAM_UPGRADED;
  Ok(stream)
}

/// Drives a handshake to completion, absorbing `WouldBlock` on a
/// non-blocking socket by retrying internally rather than surfacing it
/// to the caller.
///
/// A TLS handshake is a multi-flight exchange, not a single request:
/// abandoning a partially-sent ClientHello (or a partially-processed
/// ServerHello) and having the *caller* start over from scratch, the
/// way retrying a plain `TcpStream.connect()` call works, would leave
/// stray handshake bytes on the wire and desync the connection - the
/// second attempt's fresh ClientHello lands right after the first
/// attempt's partial one, and the peer can't make sense of either. So
/// unlike a plain socket op, this has to own the retry loop itself,
/// keeping the same `ClientConnection`/`ServerConnection` across every
/// `WouldBlock` until the handshake genuinely finishes or fails for a
/// real reason. A read/write *after* the handshake has none of this
/// problem - each call is independent at the TCP-byte level - so those
/// keep the normal "caller retries the same call" contract.
fn drive_handshake<C, T, S>(conn: &mut C, sock: &mut T) -> Result<(), String>
where
  C: std::ops::DerefMut + std::ops::Deref<Target = rustls::ConnectionCommon<S>>,
  T: Read + Write,
  S: rustls::SideData,
{
  loop {
    match conn.complete_io(sock) {
      Ok(_) => return Ok(()),
      Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
        std::thread::sleep(std::time::Duration::from_millis(1));
      },
      Err(e) => return Err(e.to_string()),
    }
  }
}

fn tls_wrap_client(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 2);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(TCP_STREAM));
  enforce_method_arg_type!(ctx, 1, ArgType::PtrOf(TLS_CONFIG));
  enforce_method_arg_type!(ctx, 2, ArgType::String);

  let server_name = ctx.args[2].as_str().to_string();
  let name =
    ServerName::try_from(server_name).map_err(|e| format!("tls: invalid server name: {e}"))?;

  let config = with_config_at(ctx, 1, |config| config.build_client_config())?;
  let tcp_stream = take_tcp_stream(ctx, 0)?;

  let conn = ClientConnection::new(config, name).map_err(|e| e.to_string())?;
  let mut stream = StreamOwned::new(conn, tcp_stream);
  // Force the handshake to run to completion now, rather than lazily on
  // the first real read/write, so a bad certificate or refused handshake
  // surfaces from `connect()`/`upgrade()` itself instead of from whatever
  // unrelated call happens to trigger it later.
  drive_handshake(&mut stream.conn, &mut stream.sock)?;

  Ok(
    ctx
      .heap()
      .alloc_ptr(TLS_STREAM, ZuriTlsStream::Client(stream)),
  )
}

fn tls_wrap_server(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(TCP_STREAM));
  enforce_method_arg_type!(ctx, 1, ArgType::PtrOf(TLS_CONFIG));

  let config = with_config_at(ctx, 1, |config| config.build_server_config())?;
  let tcp_stream = take_tcp_stream(ctx, 0)?;

  let conn = ServerConnection::new(config).map_err(|e| e.to_string())?;
  let mut stream = StreamOwned::new(conn, tcp_stream);
  drive_handshake(&mut stream.conn, &mut stream.sock)?;

  Ok(
    ctx
      .heap()
      .alloc_ptr(TLS_STREAM, ZuriTlsStream::Server(stream)),
  )
}

// ---------------------------------------------------------------------------
// TlsStream I/O natives
// ---------------------------------------------------------------------------

fn get_data(args: &[Value]) -> Vec<u8> {
  let value = args[0];
  if value.is_string() {
    value.as_str().as_bytes().to_vec()
  } else {
    value.as_bytes().to_vec()
  }
}

fn with_stream<F, T>(ctx: &mut ZuriContext, f: F) -> Result<T, String>
where
  F: FnOnce(&mut ZuriTlsStream) -> Result<T, String>,
{
  let mut ptr = ctx.args[0].as_ptr_cell().borrow_mut();
  let stream = ptr
    .downcast_mut::<ZuriTlsStream>()
    .ok_or_else(|| "tls: expected a TlsStream".to_string())?;
  f(stream)
}

fn tls_read(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(TLS_STREAM));
  enforce_method_arg_type!(ctx, 1, ArgType::Number);

  let length = ctx.args[1].as_number() as usize;
  let mut buffer = vec![0u8; length];
  let bytes_read = with_stream(ctx, |s| s.read(&mut buffer))?;
  Ok(ctx.heap().alloc_bytes(&buffer[0..bytes_read]))
}

fn tls_read_exact(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(TLS_STREAM));
  enforce_method_arg_type!(ctx, 1, ArgType::Number);

  let length = ctx.args[1].as_number() as usize;
  let mut buffer = vec![0u8; length];
  with_stream(ctx, |s| s.read_exact(&mut buffer))?;
  Ok(ctx.heap().alloc_bytes(buffer))
}

fn tls_read_all(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(TLS_STREAM));

  let mut buffer = Vec::new();
  with_stream(ctx, |s| s.read_to_end(&mut buffer))?;
  Ok(ctx.heap().alloc_bytes(buffer))
}

fn tls_read_as_string(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(TLS_STREAM));

  let mut buffer = String::new();
  with_stream(ctx, |s| s.read_to_string(&mut buffer))?;
  Ok(ctx.heap().alloc_string(buffer))
}

fn tls_write(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(TLS_STREAM));
  enforce_method_arg_type_any_of!(ctx, 1, [ArgType::Bytes, ArgType::String]);

  let data = get_data(&ctx.args[1..]);
  let written = with_stream(ctx, |s| s.write(&data))?;
  Ok(Value::number(written as f64))
}

fn tls_write_all(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(TLS_STREAM));
  enforce_method_arg_type_any_of!(ctx, 1, [ArgType::Bytes, ArgType::String]);

  let data = get_data(&ctx.args[1..]);
  with_stream(ctx, |s| s.write_all(&data))?;
  Ok(Value::nil())
}

fn tls_flush(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(TLS_STREAM));

  with_stream(ctx, |s| s.flush())?;
  Ok(Value::nil())
}

fn tls_shutdown(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(TLS_STREAM));

  with_stream(ctx, |s| s.shutdown())?;
  Ok(Value::nil())
}

fn tls_close(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(TLS_STREAM));

  let mut ptr = ctx.args[0].as_ptr_cell().borrow_mut();
  if let Some(stream) = ptr.downcast_mut::<ZuriTlsStream>() {
    // Best-effort close_notify; a peer that's already gone shouldn't stop
    // us from tearing the connection down on our side.
    let _ = stream.shutdown();
    *stream = ZuriTlsStream::Closed;
  }
  ptr.type_name = TLS_STREAM_CLOSED;

  Ok(Value::nil())
}

fn tls_alpn_protocol(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(TLS_STREAM));

  let protocol = with_stream(ctx, |s| Ok(s.alpn_protocol()))?;
  match protocol {
    Some(bytes) => Ok(
      ctx
        .heap()
        .alloc_string(String::from_utf8_lossy(&bytes).into_owned()),
    ),
    None => Ok(Value::nil()),
  }
}

// ---------------------------------------------------------------------------
// Peer certificate introspection
// ---------------------------------------------------------------------------

fn peer_certificate(ctx: &mut ZuriContext) -> Result<Option<CertificateDer<'static>>, String> {
  enforce_method_arg_count!(ctx, 0);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(TLS_STREAM));
  with_stream(ctx, |s| Ok(s.peer_certificate()))
}

fn parsed_peer_certificate(
  ctx: &mut ZuriContext,
) -> Result<Option<x509_cert::Certificate>, String> {
  match peer_certificate(ctx)? {
    Some(der) => {
      let cert = x509_cert::Certificate::from_der(der.as_ref())
        .map_err(|e| format!("tls: could not parse peer certificate: {e}"))?;
      Ok(Some(cert))
    },
    None => Ok(None),
  }
}

fn tls_peer_certificate_der(ctx: &mut ZuriContext) -> Result<Value, String> {
  match peer_certificate(ctx)? {
    Some(der) => Ok(ctx.heap().alloc_bytes(der.as_ref().to_vec())),
    None => Ok(Value::nil()),
  }
}

fn tls_peer_certificate_subject(ctx: &mut ZuriContext) -> Result<Value, String> {
  match parsed_peer_certificate(ctx)? {
    Some(cert) => Ok(
      ctx
        .heap()
        .alloc_string(cert.tbs_certificate().subject().to_string()),
    ),
    None => Ok(Value::nil()),
  }
}

fn tls_peer_certificate_issuer(ctx: &mut ZuriContext) -> Result<Value, String> {
  match parsed_peer_certificate(ctx)? {
    Some(cert) => Ok(
      ctx
        .heap()
        .alloc_string(cert.tbs_certificate().issuer().to_string()),
    ),
    None => Ok(Value::nil()),
  }
}

fn general_name_to_string(name: &GeneralName) -> Option<String> {
  match name {
    GeneralName::DnsName(s) => Some(s.as_str().to_string()),
    GeneralName::Rfc822Name(s) => Some(s.as_str().to_string()),
    GeneralName::UniformResourceIdentifier(s) => Some(s.as_str().to_string()),
    GeneralName::DirectoryName(n) => Some(n.to_string()),
    GeneralName::IpAddress(octets) => match octets.as_bytes().len() {
      4 => {
        let b = octets.as_bytes();
        Some(format!("{}.{}.{}.{}", b[0], b[1], b[2], b[3]))
      },
      16 => {
        Some(std::net::Ipv6Addr::from(<[u8; 16]>::try_from(octets.as_bytes()).ok()?).to_string())
      },
      _ => None,
    },
    _ => None,
  }
}

fn tls_peer_certificate_sans(ctx: &mut ZuriContext) -> Result<Value, String> {
  match parsed_peer_certificate(ctx)? {
    Some(cert) => {
      let sans = cert
        .tbs_certificate()
        .get_extension::<SubjectAltName>()
        .map_err(|e| format!("tls: could not parse subjectAltName extension: {e}"))?;
      let names: Vec<Value> = match sans {
        Some((_, SubjectAltName(general_names))) => general_names
          .iter()
          .filter_map(general_name_to_string)
          .map(|s| ctx.heap().alloc_string(s))
          .collect(),
        None => Vec::new(),
      };
      Ok(ctx.heap().alloc_list(names))
    },
    None => Ok(Value::nil()),
  }
}

fn tls_peer_certificate_not_before(ctx: &mut ZuriContext) -> Result<Value, String> {
  match parsed_peer_certificate(ctx)? {
    Some(cert) => Ok(Value::number(
      cert
        .tbs_certificate()
        .validity()
        .not_before
        .to_unix_duration()
        .as_secs() as f64,
    )),
    None => Ok(Value::nil()),
  }
}

fn tls_peer_certificate_not_after(ctx: &mut ZuriContext) -> Result<Value, String> {
  match parsed_peer_certificate(ctx)? {
    Some(cert) => Ok(Value::number(
      cert
        .tbs_certificate()
        .validity()
        .not_after
        .to_unix_duration()
        .as_secs() as f64,
    )),
    None => Ok(Value::nil()),
  }
}

#[allow(dead_code)]
fn now_unix_secs() -> u64 {
  SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .map(|d| d.as_secs())
    .unwrap_or(0)
}
