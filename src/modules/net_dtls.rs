use std::collections::{HashMap, HashSet, VecDeque};
use std::io::ErrorKind;
use std::net::{SocketAddr, ToSocketAddrs, UdpSocket};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use bytes::BytesMut;
use rtc_dtls::config::{ClientAuthType, ConfigBuilder, HandshakeConfig, VerifyPeerCertificateFn};
use rtc_dtls::crypto::Certificate as DtlsCertificate;
use rtc_dtls::endpoint::{Endpoint, EndpointEvent};
use rtc_shared::{EcnCodepoint, TransportProtocol};
use rustls::RootCertStore;
use rustls::pki_types::{CertificateDer, PrivateKeyDer, ServerName, UnixTime};
use rustls::server::ParsedCertificate;
use x509_cert::der::Decode;
use x509_cert::ext::pkix::SubjectAltName;
use x509_cert::ext::pkix::name::GeneralName;

use crate::builtins::enforce::{
  ArgType, enforce_method_arg_count, enforce_method_arg_type, enforce_method_arg_type_any_of,
};
use crate::enforce_arg_count;
use crate::modules::net_tls::{parse_cert_chain_pem, parse_private_key_pem};
use crate::modules::{BuiltinModuleDef, native, optional_number};
use crate::vm::object::ZuriContext;
use crate::vm::value::Value;
use crate::vm::vm::VM;

pub static MODULE: BuiltinModuleDef = BuiltinModuleDef {
  name: "_net_dtls",
  build,
};

fn build(vm: &mut VM) -> Vec<(&'static str, Value)> {
  vec![
    (
      "dtls_config_new",
      native(vm, "@new", 0, false, dtls_config_new),
    ),
    (
      "dtls_config_set_root_store",
      native(vm, "set_root_store", 2, false, dtls_config_set_root_store),
    ),
    (
      "dtls_config_add_ca_pem",
      native(vm, "add_ca_pem", 2, false, dtls_config_add_ca_pem),
    ),
    (
      "dtls_config_set_cert_chain",
      native(vm, "set_cert_chain", 3, false, dtls_config_set_cert_chain),
    ),
    (
      "dtls_config_require_client_cert",
      native(
        vm,
        "require_client_cert",
        2,
        false,
        dtls_config_require_client_cert,
      ),
    ),
    (
      "dtls_config_set_insecure",
      native(vm, "set_insecure", 2, false, dtls_config_set_insecure),
    ),
    ("dtls_new", native(vm, "@new", 0, false, dtls_new)),
    ("dtls_bind", native(vm, "bind", 2, false, dtls_bind)),
    ("dtls_connect", native(vm, "connect", 3, true, dtls_connect)),
    ("dtls_accept", native(vm, "accept", 2, false, dtls_accept)),
    ("dtls_read", native(vm, "read", 2, false, dtls_read)),
    ("dtls_write", native(vm, "write", 2, true, dtls_write)),
    (
      "dtls_local_address",
      native(vm, "local_address", 1, false, dtls_local_address),
    ),
    (
      "dtls_peer_address",
      native(vm, "peer_address", 1, false, dtls_peer_address),
    ),
    (
      "dtls_set_read_timeout",
      native(vm, "set_read_timeout", 2, false, dtls_set_read_timeout),
    ),
    ("dtls_close", native(vm, "close", 1, false, dtls_close)),
    (
      "dtls_peer_certificate_der",
      native(
        vm,
        "peer_certificate_der",
        1,
        false,
        dtls_peer_certificate_der,
      ),
    ),
    (
      "dtls_peer_certificate_subject",
      native(
        vm,
        "peer_certificate_subject",
        1,
        false,
        dtls_peer_certificate_subject,
      ),
    ),
    (
      "dtls_peer_certificate_issuer",
      native(
        vm,
        "peer_certificate_issuer",
        1,
        false,
        dtls_peer_certificate_issuer,
      ),
    ),
    (
      "dtls_peer_certificate_sans",
      native(
        vm,
        "peer_certificate_sans",
        1,
        false,
        dtls_peer_certificate_sans,
      ),
    ),
    (
      "dtls_peer_certificate_not_before",
      native(
        vm,
        "peer_certificate_not_before",
        1,
        false,
        dtls_peer_certificate_not_before,
      ),
    ),
    (
      "dtls_peer_certificate_not_after",
      native(
        vm,
        "peer_certificate_not_after",
        1,
        false,
        dtls_peer_certificate_not_after,
      ),
    ),
  ]
}

const DTLS_CONFIG: &str = "zuri::net::DtlsConfig";
const DTLS_SOCKET: &str = "zuri::net::DtlsSocket";

// ---------------------------------------------------------------------------
// DtlsConfig
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
enum RootStoreMode {
  Bundled,
  Native,
}

/// Re-encodes a private key as PKCS#8 DER if it isn't already.
///
/// rtc-dtls's `KeyPair -> CryptoPrivateKey` conversion always calls
/// `EcdsaKeyPair::from_pkcs8`/`RsaKeyPair::from_pkcs8` on whatever bytes
/// `rcgen::KeyPair::serialize_der()` hands back - but for a key that rcgen
/// built from SEC1 or PKCS#1 input, `serialize_der()` just echoes those
/// original bytes rather than re-encoding them, so aws-lc-rs ends up being
/// asked to parse SEC1/PKCS#1 bytes as PKCS#8 and fails. Rather than rely on
/// rcgen to convert, this does it directly: exactly the same "try P-256,
/// then P-384" curve-detection `crypto.rs` already uses for the same
/// PEM-doesn't-say-which-curve problem, plus the RSA PKCS#1 case.
fn normalize_to_pkcs8(key_der: PrivateKeyDer<'static>) -> Result<PrivateKeyDer<'static>, String> {
  use p256::pkcs8::EncodePrivateKey as _;
  use rsa::pkcs1::DecodeRsaPrivateKey as _;

  match key_der {
    PrivateKeyDer::Pkcs8(_) => Ok(key_der),
    PrivateKeyDer::Sec1(sec1) => {
      let der = sec1.secret_sec1_der();
      if let Ok(key) = p256::SecretKey::from_sec1_der(der) {
        let doc = key
          .to_pkcs8_der()
          .map_err(|e| format!("dtls: could not re-encode private key: {e}"))?;
        return Ok(PrivateKeyDer::Pkcs8(doc.as_bytes().to_vec().into()));
      }
      if let Ok(key) = p384::SecretKey::from_sec1_der(der) {
        let doc = key
          .to_pkcs8_der()
          .map_err(|e| format!("dtls: could not re-encode private key: {e}"))?;
        return Ok(PrivateKeyDer::Pkcs8(doc.as_bytes().to_vec().into()));
      }
      Err(
        "dtls: unsupported EC curve in private key (only P-256 and P-384 are supported)"
          .to_string(),
      )
    },
    PrivateKeyDer::Pkcs1(pkcs1) => {
      let key = rsa::RsaPrivateKey::from_pkcs1_der(pkcs1.secret_pkcs1_der())
        .map_err(|e| format!("dtls: invalid RSA private key: {e}"))?;
      let doc = key
        .to_pkcs8_der()
        .map_err(|e| format!("dtls: could not re-encode private key: {e}"))?;
      Ok(PrivateKeyDer::Pkcs8(doc.as_bytes().to_vec().into()))
    },
    _ => Err("dtls: unsupported private key encoding".to_string()),
  }
}

struct ZuriDtlsConfig {
  root_store_mode: RootStoreMode,
  extra_ca_pems: Vec<String>,
  cert_chain_pem: Option<String>,
  key_pem: Option<String>,
  require_client_cert: bool,
  insecure: bool,
}

impl ZuriDtlsConfig {
  fn new() -> Self {
    ZuriDtlsConfig {
      root_store_mode: RootStoreMode::Bundled,
      extra_ca_pems: Vec::new(),
      cert_chain_pem: None,
      key_pem: None,
      require_client_cert: false,
      insecure: false,
    }
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
          return Err(
            "dtls: no usable certificates found in the OS-native trust store".to_string(),
          );
        }
        store
      },
    };
    for pem in &self.extra_ca_pems {
      let certs = parse_cert_chain_pem(pem)?;
      let (_, invalid) = store.add_parsable_certificates(certs);
      if invalid > 0 {
        return Err(format!(
          "dtls: {invalid} certificate(s) in a custom CA bundle could not be parsed"
        ));
      }
    }
    Ok(store)
  }

  /// Builds a handshake config for one specific connection.
  ///
  /// rtc-dtls's own `ConfigBuilder::build()` hardcodes `server_cert_verifier`
  /// to a throwaway self-signed root, and always leaves `client_cert_verifier`
  /// unset - both are WebRTC's fingerprint-based trust model (verify via SDP,
  /// not a CA chain), not general-purpose PKI verification. So real chain and
  /// hostname verification is bypassed here (`insecure_skip_verify: true`,
  /// which suppresses that broken built-in check) and re-implemented properly
  /// through `verify_peer_certificate`, using the same webpki verification
  /// primitives `net_tls.rs` uses, unless the config is explicitly insecure.
  fn build_handshake_config(
    &self,
    is_client: bool,
    server_name: Option<String>,
    remote_addr: Option<SocketAddr>,
  ) -> Result<Arc<HandshakeConfig>, String> {
    let mut builder = ConfigBuilder::default().with_insecure_skip_verify(true);

    if let (Some(chain_pem), Some(key_pem)) = (&self.cert_chain_pem, &self.key_pem) {
      let chain = parse_cert_chain_pem(chain_pem)?;
      let key_der = normalize_to_pkcs8(parse_private_key_pem(key_pem)?)?;
      // rtc-dtls's own Certificate::from_pem() rejects a standard
      // "-----BEGIN PRIVATE KEY-----" PEM block (it checks the tag against
      // the literal "PRIVATE_KEY", which nothing produces), so the private
      // key is bridged through rcgen instead, the same conversion path
      // Certificate::from_pem itself would use if its tag check worked.
      let key_pair = rcgen::KeyPair::try_from(&key_der)
        .map_err(|e| format!("dtls: invalid private key: {e}"))?;
      let private_key = rtc_dtls::crypto::CryptoPrivateKey::try_from(&key_pair)
        .map_err(|e| format!("dtls: invalid private key: {e}"))?;
      builder = builder.with_certificates(vec![DtlsCertificate {
        certificate: chain,
        private_key,
      }]);
    }

    if !is_client {
      builder = builder.with_client_auth(if self.require_client_cert {
        ClientAuthType::RequireAnyClientCert
      } else {
        ClientAuthType::NoClientCert
      });
    }

    if !self.insecure {
      let root_store = Arc::new(self.build_root_store()?);
      let verifier = if is_client {
        let expected_name = server_name
          .map(|s| ServerName::try_from(s).map_err(|e| format!("dtls: invalid server name: {e}")))
          .transpose()?;
        make_server_cert_verifier(root_store, expected_name)
      } else {
        make_client_cert_verifier(root_store)?
      };
      builder = builder.with_verify_peer_certificate(Some(verifier));
    }

    let config = builder
      .build(is_client, remote_addr)
      .map_err(|e| format!("dtls: invalid configuration: {e}"))?;
    Ok(Arc::new(config))
  }
}

/// Real CA-chain and hostname verification for the server's certificate,
/// as seen from the client side, wired in through rtc-dtls's
/// `verify_peer_certificate` hook since its own built-in verifier isn't
/// meant for this. Ignores the fingerprint list rtc-dtls passes in -
/// that's WebRTC's out-of-band SDP verification path, which nothing
/// here uses.
///
/// This specifically checks for the server-authentication extended key
/// usage, which is right for a server's certificate but wrong for a
/// client's - `make_client_cert_verifier` below is the server-side
/// counterpart, checking client-authentication EKU instead, for
/// verifying what the *client* presented during mutual DTLS.
fn make_server_cert_verifier(
  root_store: Arc<RootCertStore>,
  expected_name: Option<ServerName<'static>>,
) -> VerifyPeerCertificateFn {
  let provider = Arc::new(rustls::crypto::aws_lc_rs::default_provider());
  // `verify_peer_certificate`'s second parameter is the chain webpki's OWN
  // verifier already validated - always empty here, since this config
  // always sets `insecure_skip_verify: true` to bypass that (WebRTC-
  // fingerprint-oriented) verifier in the first place. The real peer
  // certificates are the raw DER in the first parameter.
  Arc::new(
    move |peer_certs: &[Vec<u8>], _already_verified_chain: &[CertificateDer<'static>]| {
      let chain: Vec<CertificateDer<'static>> = peer_certs
        .iter()
        .map(|der| CertificateDer::from(der.clone()))
        .collect();
      let (end_entity, intermediates) = chain
        .split_first()
        .ok_or_else(|| dtls_error("no certificate presented"))?;
      let parsed =
        ParsedCertificate::try_from(end_entity).map_err(|e| dtls_error(&e.to_string()))?;
      rustls::client::verify_server_cert_signed_by_trust_anchor(
        &parsed,
        &root_store,
        intermediates,
        UnixTime::now(),
        provider.signature_verification_algorithms.all,
      )
      .map_err(|e| dtls_error(&e.to_string()))?;
      if let Some(name) = &expected_name {
        rustls::client::verify_server_name(&parsed, name)
          .map_err(|e| dtls_error(&e.to_string()))?;
      }
      Ok(())
    },
  )
}

/// Real CA-chain verification for the client's certificate, as seen
/// from the server side during mutual DTLS. Built on rustls's own
/// `WebPkiClientVerifier` - the same one `net_tls.rs` uses for mutual
/// TLS - rather than hand-rolling chain validation a second time; it
/// correctly checks for client-authentication EKU (no hostname check,
/// since a client certificate isn't verified against a name the way a
/// server's is).
fn make_client_cert_verifier(
  root_store: Arc<RootCertStore>,
) -> Result<VerifyPeerCertificateFn, String> {
  let verifier = rustls::server::WebPkiClientVerifier::builder(root_store)
    .build()
    .map_err(|e| format!("dtls: could not build a client certificate verifier: {e}"))?;
  Ok(Arc::new(
    move |peer_certs: &[Vec<u8>], _already_verified_chain: &[CertificateDer<'static>]| {
      let chain: Vec<CertificateDer<'static>> = peer_certs
        .iter()
        .map(|der| CertificateDer::from(der.clone()))
        .collect();
      let (end_entity, intermediates) = chain
        .split_first()
        .ok_or_else(|| dtls_error("no certificate presented"))?;
      verifier
        .verify_client_cert(end_entity, intermediates, UnixTime::now())
        .map_err(|e| dtls_error(&e.to_string()))?;
      Ok(())
    },
  ))
}

fn dtls_error(msg: &str) -> rtc_shared::error::Error {
  rtc_shared::error::Error::OtherDtlsErr(msg.to_string())
}

// ---------------------------------------------------------------------------
// DtlsSocket
// ---------------------------------------------------------------------------

struct SharedEndpoint {
  socket: UdpSocket,
  endpoint: Endpoint,
  /// Decrypted application-data messages that arrived for a remote other
  /// than whichever one was actively being read for at the time - DTLS
  /// multiplexes many peer associations over one bound socket, so a single
  /// `recv` can surface data meant for any of them.
  pending: HashMap<SocketAddr, VecDeque<BytesMut>>,
  /// Remotes whose handshake has completed but that `accept()` hasn't
  /// handed back to Zuri code yet.
  completed_unaccepted: VecDeque<SocketAddr>,
  /// Remotes already handed back by `accept()`, so a retransmitted final
  /// handshake flight doesn't get accepted twice.
  accepted: HashSet<SocketAddr>,
}

impl SharedEndpoint {
  fn pump_transmits(&mut self) -> Result<(), String> {
    while let Some(msg) = self.endpoint.poll_transmit() {
      self
        .socket
        .send_to(&msg.message, msg.transport.peer_addr)
        .map_err(|e| e.to_string())?;
    }
    Ok(())
  }

  /// Waits for at most `timeout` for one inbound datagram, feeds it to the
  /// endpoint, files away whatever it produced, and drives every
  /// in-progress handshake's retransmission timer. A `None` timeout means
  /// block indefinitely, matching `TcpStream`'s own default.
  fn drive_once(&mut self, timeout: Option<Duration>) -> Result<(), String> {
    self
      .socket
      .set_read_timeout(timeout)
      .map_err(|e| e.to_string())?;
    let mut buf = vec![0u8; 4096];
    match self.socket.recv_from(&mut buf) {
      Ok((n, from)) => {
        buf.truncate(n);
        let events = self
          .endpoint
          .read(
            Instant::now(),
            from,
            None::<EcnCodepoint>,
            BytesMut::from(&buf[..]),
          )
          .map_err(|e| e.to_string())?;
        for event in events {
          match event {
            EndpointEvent::ApplicationData(data) => {
              self.pending.entry(from).or_default().push_back(data);
            },
            EndpointEvent::HandshakeComplete => {
              if !self.accepted.contains(&from) {
                self.completed_unaccepted.push_back(from);
              }
            },
          }
        }
        self.pump_transmits()?;
      },
      Err(e) if e.kind() == ErrorKind::WouldBlock || e.kind() == ErrorKind::TimedOut => {
        let remotes: Vec<SocketAddr> = self.endpoint.get_connections_keys().copied().collect();
        for remote in remotes {
          // A connection that has exhausted its retransmissions reports an
          // error here; that's expected for an abandoned handshake and
          // shouldn't take the whole socket down with it.
          let _ = self.endpoint.handle_timeout(remote, Instant::now());
        }
        self.pump_transmits()?;
      },
      Err(e) => return Err(e.to_string()),
    }
    Ok(())
  }
}

enum ZuriDtlsRole {
  Unbound,
  Listener(Arc<Mutex<SharedEndpoint>>),
  Connected {
    shared: Arc<Mutex<SharedEndpoint>>,
    remote: SocketAddr,
  },
  Closed,
}

struct ZuriDtls {
  role: ZuriDtlsRole,
  read_timeout: Option<Duration>,
}

const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(30);
const POLL_INTERVAL: Duration = Duration::from_millis(500);

impl ZuriDtls {
  fn new() -> Self {
    ZuriDtls {
      role: ZuriDtlsRole::Unbound,
      read_timeout: None,
    }
  }

  fn bind_socket(address: &str) -> Result<UdpSocket, String> {
    let socket = UdpSocket::bind(address).map_err(|e| e.to_string())?;
    Ok(socket)
  }

  fn bind(&mut self, address: &str) -> Result<(), String> {
    if !matches!(self.role, ZuriDtlsRole::Unbound) {
      return Err("dtls: socket is already bound or connected".to_string());
    }
    let socket = Self::bind_socket(address)?;
    let local_addr = socket.local_addr().map_err(|e| e.to_string())?;
    let endpoint = Endpoint::new(local_addr, TransportProtocol::UDP, None);
    self.role = ZuriDtlsRole::Listener(Arc::new(Mutex::new(SharedEndpoint {
      socket,
      endpoint,
      pending: HashMap::new(),
      completed_unaccepted: VecDeque::new(),
      accepted: HashSet::new(),
    })));
    Ok(())
  }

  fn connect(
    &mut self,
    config: &ZuriDtlsConfig,
    address: &str,
    server_name: Option<String>,
  ) -> Result<(), String> {
    if !matches!(self.role, ZuriDtlsRole::Unbound) {
      return Err("dtls: socket is already bound or connected".to_string());
    }
    let remote: SocketAddr = address
      .to_socket_addrs()
      .map_err(|e| e.to_string())?
      .next()
      .ok_or_else(|| format!("dtls: could not resolve '{address}'"))?;
    let local_bind = if remote.is_ipv4() {
      "0.0.0.0:0"
    } else {
      "[::]:0"
    };
    let socket = Self::bind_socket(local_bind)?;
    let local_addr = socket.local_addr().map_err(|e| e.to_string())?;

    let handshake_config = config.build_handshake_config(true, server_name, Some(remote))?;
    let mut endpoint = Endpoint::new(local_addr, TransportProtocol::UDP, None);
    endpoint
      .connect(remote, handshake_config, None)
      .map_err(|e| e.to_string())?;

    let mut shared = SharedEndpoint {
      socket,
      endpoint,
      pending: HashMap::new(),
      completed_unaccepted: VecDeque::new(),
      accepted: HashSet::new(),
    };
    shared.pump_transmits()?;

    let deadline = Instant::now() + HANDSHAKE_TIMEOUT;
    loop {
      if Instant::now() >= deadline {
        return Err("dtls: handshake timed out".to_string());
      }
      shared.drive_once(Some(POLL_INTERVAL))?;
      if let Some(pos) = shared
        .completed_unaccepted
        .iter()
        .position(|r| *r == remote)
      {
        shared.completed_unaccepted.remove(pos);
        break;
      }
    }

    self.role = ZuriDtlsRole::Connected {
      shared: Arc::new(Mutex::new(shared)),
      remote,
    };
    Ok(())
  }

  fn accept(&mut self, config: &ZuriDtlsConfig) -> Result<ZuriDtls, String> {
    let shared = match &self.role {
      ZuriDtlsRole::Listener(shared) => shared.clone(),
      _ => return Err("dtls: accept() is only valid on a bound socket".to_string()),
    };

    let handshake_config = config.build_handshake_config(false, None, None)?;
    {
      let mut shared = shared
        .lock()
        .map_err(|_| "dtls: lock poisoned".to_string())?;
      shared.endpoint.set_server_config(Some(handshake_config));
    }

    loop {
      let mut guard = shared
        .lock()
        .map_err(|_| "dtls: lock poisoned".to_string())?;
      if let Some(remote) = guard.completed_unaccepted.pop_front() {
        if guard.accepted.insert(remote) {
          drop(guard);
          return Ok(ZuriDtls {
            role: ZuriDtlsRole::Connected {
              shared: shared.clone(),
              remote,
            },
            read_timeout: None,
          });
        }
        continue;
      }
      guard.drive_once(self.read_timeout.or(Some(POLL_INTERVAL)))?;
    }
  }

  fn read(&mut self, max_len: usize) -> Result<Vec<u8>, String> {
    let (shared, remote) = match &self.role {
      ZuriDtlsRole::Connected { shared, remote } => (shared.clone(), *remote),
      _ => return Err("dtls: not a connected socket".to_string()),
    };
    loop {
      let mut guard = shared
        .lock()
        .map_err(|_| "dtls: lock poisoned".to_string())?;
      if let Some(queue) = guard.pending.get_mut(&remote)
        && let Some(mut message) = queue.pop_front()
      {
        message.truncate(max_len);
        return Ok(message.to_vec());
      }
      let timeout = self.read_timeout.or(Some(POLL_INTERVAL));
      guard.drive_once(timeout)?;
      if self.read_timeout.is_some() {
        // A real caller-supplied timeout, not our internal poll cadence:
        // one attempt is all they asked for.
        if let Some(queue) = guard.pending.get_mut(&remote)
          && let Some(mut message) = queue.pop_front()
        {
          message.truncate(max_len);
          return Ok(message.to_vec());
        }
        return Err("dtls: read timed out".to_string());
      }
    }
  }

  fn write(&mut self, data: &[u8]) -> Result<usize, String> {
    let (shared, remote) = match &self.role {
      ZuriDtlsRole::Connected { shared, remote } => (shared.clone(), *remote),
      _ => return Err("dtls: not a connected socket".to_string()),
    };
    let mut guard = shared
      .lock()
      .map_err(|_| "dtls: lock poisoned".to_string())?;
    guard
      .endpoint
      .write(remote, data)
      .map_err(|e| e.to_string())?;
    guard.pump_transmits()?;
    Ok(data.len())
  }

  fn local_address(&self) -> Result<SocketAddr, String> {
    match &self.role {
      ZuriDtlsRole::Listener(shared) | ZuriDtlsRole::Connected { shared, .. } => {
        let guard = shared
          .lock()
          .map_err(|_| "dtls: lock poisoned".to_string())?;
        guard.socket.local_addr().map_err(|e| e.to_string())
      },
      _ => Err("dtls: socket is not bound".to_string()),
    }
  }

  fn peer_address(&self) -> Result<SocketAddr, String> {
    match &self.role {
      ZuriDtlsRole::Connected { remote, .. } => Ok(*remote),
      _ => Err("dtls: not a connected socket".to_string()),
    }
  }

  fn peer_certificate(&self) -> Result<Option<Vec<u8>>, String> {
    match &self.role {
      ZuriDtlsRole::Connected { shared, remote } => {
        let guard = shared
          .lock()
          .map_err(|_| "dtls: lock poisoned".to_string())?;
        Ok(
          guard
            .endpoint
            .get_connection_state(*remote)
            .and_then(|state| state.peer_certificates.first().cloned()),
        )
      },
      _ => Ok(None),
    }
  }

  fn close(&mut self) {
    self.role = ZuriDtlsRole::Closed;
  }
}

// ---------------------------------------------------------------------------
// DtlsConfig natives
// ---------------------------------------------------------------------------

fn dtls_config_new(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 0);
  Ok(ctx.heap().alloc_ptr(DTLS_CONFIG, ZuriDtlsConfig::new()))
}

fn with_dtls_config<F, T>(ctx: &mut ZuriContext, f: F) -> Result<T, String>
where
  F: FnOnce(&mut ZuriDtlsConfig) -> Result<T, String>,
{
  let mut ptr = ctx.args[0].as_ptr_cell().borrow_mut();
  let config = ptr
    .downcast_mut::<ZuriDtlsConfig>()
    .ok_or_else(|| "dtls: expected a DtlsConfig".to_string())?;
  f(config)
}

fn dtls_config_set_root_store(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(DTLS_CONFIG));
  enforce_method_arg_type!(ctx, 1, ArgType::String);

  let mode = match ctx.args[1].as_str() {
    "bundled" => RootStoreMode::Bundled,
    "native" => RootStoreMode::Native,
    other => {
      return Err(format!(
        "dtls: unknown root store mode '{other}', expected 'bundled' or 'native'"
      ));
    },
  };
  with_dtls_config(ctx, |config| {
    config.root_store_mode = mode;
    Ok(Value::nil())
  })
}

fn dtls_config_add_ca_pem(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(DTLS_CONFIG));
  enforce_method_arg_type!(ctx, 1, ArgType::String);

  let pem = ctx.args[1].as_str().to_string();
  with_dtls_config(ctx, |config| {
    config.extra_ca_pems.push(pem);
    Ok(Value::nil())
  })
}

fn dtls_config_set_cert_chain(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 2);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(DTLS_CONFIG));
  enforce_method_arg_type!(ctx, 1, ArgType::String);
  enforce_method_arg_type!(ctx, 2, ArgType::String);

  let chain_pem = ctx.args[1].as_str().to_string();
  let key_pem = ctx.args[2].as_str().to_string();
  with_dtls_config(ctx, |config| {
    config.cert_chain_pem = Some(chain_pem);
    config.key_pem = Some(key_pem);
    Ok(Value::nil())
  })
}

fn dtls_config_require_client_cert(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(DTLS_CONFIG));
  enforce_method_arg_type!(ctx, 1, ArgType::Bool);

  let required = ctx.args[1].as_bool();
  with_dtls_config(ctx, |config| {
    config.require_client_cert = required;
    Ok(Value::nil())
  })
}

fn dtls_config_set_insecure(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(DTLS_CONFIG));
  enforce_method_arg_type!(ctx, 1, ArgType::Bool);

  let insecure = ctx.args[1].as_bool();
  with_dtls_config(ctx, |config| {
    config.insecure = insecure;
    Ok(Value::nil())
  })
}

// ---------------------------------------------------------------------------
// DtlsSocket natives
// ---------------------------------------------------------------------------

fn dtls_new(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 0);
  Ok(ctx.heap().alloc_ptr(DTLS_SOCKET, ZuriDtls::new()))
}

fn with_socket<F, T>(ctx: &mut ZuriContext, f: F) -> Result<T, String>
where
  F: FnOnce(&mut ZuriDtls) -> Result<T, String>,
{
  let mut ptr = ctx.args[0].as_ptr_cell().borrow_mut();
  let socket = ptr
    .downcast_mut::<ZuriDtls>()
    .ok_or_else(|| "dtls: expected a DtlsSocket".to_string())?;
  f(socket)
}

fn dtls_bind(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(DTLS_SOCKET));
  enforce_method_arg_type!(ctx, 1, ArgType::String);

  let address = ctx.args[1].as_str().to_string();
  with_socket(ctx, |socket| socket.bind(&address))?;
  Ok(Value::nil())
}

fn dtls_connect(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(DTLS_SOCKET));
  enforce_method_arg_type!(ctx, 1, ArgType::PtrOf(DTLS_CONFIG));
  enforce_method_arg_type!(ctx, 2, ArgType::String);

  let address = ctx.args[2].as_str().to_string();
  let server_name = match ctx.args.get(3) {
    Some(v) if v.is_string() => Some(v.as_str().to_string()),
    _ => None,
  };

  let mut config_ptr = ctx.args[1].as_ptr_cell().borrow_mut();
  let config = config_ptr
    .downcast_mut::<ZuriDtlsConfig>()
    .ok_or_else(|| "dtls: expected a DtlsConfig".to_string())?;

  with_socket(ctx, |socket| socket.connect(config, &address, server_name))?;
  Ok(Value::nil())
}

fn dtls_accept(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(DTLS_SOCKET));
  enforce_method_arg_type!(ctx, 1, ArgType::PtrOf(DTLS_CONFIG));

  let mut config_ptr = ctx.args[1].as_ptr_cell().borrow_mut();
  let config = config_ptr
    .downcast_mut::<ZuriDtlsConfig>()
    .ok_or_else(|| "dtls: expected a DtlsConfig".to_string())?;

  let accepted = with_socket(ctx, |socket| socket.accept(config))?;
  Ok(ctx.heap().alloc_ptr(DTLS_SOCKET, accepted))
}

fn dtls_read(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(DTLS_SOCKET));
  enforce_method_arg_type!(ctx, 1, ArgType::Number);

  let length = ctx.args[1].as_number() as usize;
  let bytes = with_socket(ctx, |socket| socket.read(length))?;
  Ok(ctx.heap().alloc_bytes(bytes))
}

fn get_data(args: &[Value]) -> Vec<u8> {
  let value = args[0];
  if value.is_string() {
    value.as_str().as_bytes().to_vec()
  } else {
    value.as_bytes().to_vec()
  }
}

fn dtls_write(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(DTLS_SOCKET));
  enforce_method_arg_type_any_of!(ctx, 1, [ArgType::Bytes, ArgType::String]);

  let data = get_data(&ctx.args[1..]);
  let written = with_socket(ctx, |socket| socket.write(&data))?;
  Ok(Value::number(written as f64))
}

fn dtls_local_address(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(DTLS_SOCKET));

  let addr = with_socket(ctx, |socket| socket.local_address())?;
  Ok(ctx.heap().alloc_string(addr.to_string()))
}

fn dtls_peer_address(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(DTLS_SOCKET));

  let addr = with_socket(ctx, |socket| socket.peer_address())?;
  Ok(ctx.heap().alloc_string(addr.to_string()))
}

fn dtls_set_read_timeout(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(DTLS_SOCKET));
  enforce_method_arg_type!(ctx, 1, ArgType::Number);

  let timeout = optional_number(ctx, 1, 0.0)?;
  let duration = if timeout > 0.0 {
    Some(Duration::from_millis(timeout as u64))
  } else {
    None
  };
  with_socket(ctx, |socket| {
    socket.read_timeout = duration;
    Ok(())
  })?;
  Ok(Value::nil())
}

fn dtls_close(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(DTLS_SOCKET));

  with_socket(ctx, |socket| {
    socket.close();
    Ok(())
  })?;
  Ok(Value::nil())
}

// ---------------------------------------------------------------------------
// Peer certificate introspection - same shape as net_tls.rs's, over the raw
// DER `rtc_dtls::state::State::peer_certificates` hands back.
// ---------------------------------------------------------------------------

fn parsed_peer_certificate(
  ctx: &mut ZuriContext,
) -> Result<Option<x509_cert::Certificate>, String> {
  enforce_method_arg_count!(ctx, 0);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(DTLS_SOCKET));

  let der = with_socket(ctx, |socket| socket.peer_certificate())?;
  match der {
    Some(der) => {
      let cert = x509_cert::Certificate::from_der(&der)
        .map_err(|e| format!("dtls: could not parse peer certificate: {e}"))?;
      Ok(Some(cert))
    },
    None => Ok(None),
  }
}

fn dtls_peer_certificate_der(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(DTLS_SOCKET));

  let der = with_socket(ctx, |socket| socket.peer_certificate())?;
  match der {
    Some(der) => Ok(ctx.heap().alloc_bytes(der)),
    None => Ok(Value::nil()),
  }
}

fn dtls_peer_certificate_subject(ctx: &mut ZuriContext) -> Result<Value, String> {
  match parsed_peer_certificate(ctx)? {
    Some(cert) => Ok(
      ctx
        .heap()
        .alloc_string(cert.tbs_certificate().subject().to_string()),
    ),
    None => Ok(Value::nil()),
  }
}

fn dtls_peer_certificate_issuer(ctx: &mut ZuriContext) -> Result<Value, String> {
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

fn dtls_peer_certificate_sans(ctx: &mut ZuriContext) -> Result<Value, String> {
  match parsed_peer_certificate(ctx)? {
    Some(cert) => {
      let sans = cert
        .tbs_certificate()
        .get_extension::<SubjectAltName>()
        .map_err(|e| format!("dtls: could not parse subjectAltName extension: {e}"))?;
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

fn dtls_peer_certificate_not_before(ctx: &mut ZuriContext) -> Result<Value, String> {
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

fn dtls_peer_certificate_not_after(ctx: &mut ZuriContext) -> Result<Value, String> {
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
