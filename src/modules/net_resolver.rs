//! The nameservers this machine is configured to use.
//!
//! Everything else about DNS lives in `libs/net/resolver.zu`: building a
//! query, parsing an answer, following a truncated response onto TCP.
//! None of that needs the operating system's help. Finding out *which*
//! server to send the query to does, and the answer is somewhere
//! different on every platform, which is why this one small piece is
//! here rather than there.
//!
//! On unix that place is `/etc/resolv.conf`. On Windows there is no
//! file at all; the configuration lives in the network stack and comes
//! back from `GetAdaptersAddresses`.

use crate::enforce_arg_count;
use crate::modules::{BuiltinModuleDef, native};
use crate::vm::object::ZuriContext;
use crate::vm::value::Value;
use crate::vm::vm::VM;

pub static MODULE: BuiltinModuleDef = BuiltinModuleDef {
  name: "_net_resolver",
  build,
};

fn build(vm: &mut VM) -> Vec<(&'static str, Value)> {
  vec![
    (
      "resolver_system_servers",
      native(vm, "system_servers", 0, false, resolver_system_servers),
    ),
    (
      "resolver_system_search",
      native(vm, "system_search", 0, false, resolver_system_search),
    ),
  ]
}

fn string_list(ctx: &mut ZuriContext, items: Vec<String>) -> Value {
  let values: Vec<Value> = items
    .into_iter()
    .map(|item| ctx.heap().alloc_string(item))
    .collect();

  ctx.heap().alloc_list(values)
}

/// The addresses of the nameservers to send queries to, in the order
/// the system lists them.
///
/// Only plain IP literals come back. An address that carries a zone
/// index (`fe80::1%eth0`, and the Windows equivalent) is left out
/// rather than returned in a form nothing downstream can connect to.
fn resolver_system_servers(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 0);

  let servers = platform::servers();

  Ok(string_list(ctx, servers))
}

/// The domain suffixes to append to a name that has no dots in it, in
/// the order they should be tried. Usually empty on a machine that is
/// not part of a managed network.
fn resolver_system_search(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 0);

  let search = platform::search();

  Ok(string_list(ctx, search))
}

/// Keeps a candidate only if it parses as a bare IP address, so a zone
/// index or a stray word in a config file cannot reach the Zuri side.
fn keep_plain_ip(candidate: &str) -> Option<String> {
  candidate
    .parse::<std::net::IpAddr>()
    .ok()
    .map(|address| address.to_string())
}

#[cfg(not(windows))]
mod platform {
  use super::keep_plain_ip;

  const RESOLV_CONF: &str = "/etc/resolv.conf";

  /// resolv.conf is line oriented: a keyword, then its arguments,
  /// separated by whitespace. Anything after `#` or `;` is a comment,
  /// and a line whose keyword is not one we care about is skipped
  /// whole.
  fn directives(keyword: &str) -> Vec<Vec<String>> {
    let text = match std::fs::read_to_string(RESOLV_CONF) {
      Ok(text) => text,
      Err(_) => return Vec::new(),
    };

    let mut found = Vec::new();

    for line in text.lines() {
      let line = match line.find(['#', ';']) {
        Some(at) => &line[..at],
        None => line,
      };

      let mut words = line.split_whitespace();

      if words.next() != Some(keyword) {
        continue;
      }

      found.push(words.map(String::from).collect());
    }

    found
  }

  pub fn servers() -> Vec<String> {
    directives("nameserver")
      .into_iter()
      .filter_map(|words| words.into_iter().next())
      .filter_map(|word| keep_plain_ip(&word))
      .collect()
  }

  /// `search` takes a list and `domain` takes one name, and the last of
  /// either wins. The two are alternatives rather than additions, so a
  /// file carrying both is read the way the resolver library reads it.
  pub fn search() -> Vec<String> {
    if let Some(words) = directives("search").pop() {
      return words;
    }

    directives("domain")
      .pop()
      .and_then(|words| words.into_iter().next())
      .map(|name| vec![name])
      .unwrap_or_default()
  }
}

#[cfg(windows)]
mod platform {
  use super::keep_plain_ip;

  use windows_sys::Win32::Foundation::{ERROR_BUFFER_OVERFLOW, ERROR_SUCCESS};
  use windows_sys::Win32::NetworkManagement::IpHelper::{
    GAA_FLAG_SKIP_ANYCAST, GAA_FLAG_SKIP_FRIENDLY_NAME, GAA_FLAG_SKIP_MULTICAST,
    GetAdaptersAddresses, IP_ADAPTER_ADDRESSES_LH,
  };
  use windows_sys::Win32::NetworkManagement::Ndis::IfOperStatusUp;
  use windows_sys::Win32::Networking::WinSock::{AF_INET, AF_INET6, AF_UNSPEC, SOCKADDR};

  /// What both exported lists are gathered from: one walk of the
  /// adapter table, taking the DNS servers and the DNS suffix off every
  /// adapter that is actually up.
  fn adapters<T>(mut take: impl FnMut(&IP_ADAPTER_ADDRESSES_LH, &mut Vec<T>)) -> Vec<T> {
    let flags = GAA_FLAG_SKIP_ANYCAST | GAA_FLAG_SKIP_MULTICAST | GAA_FLAG_SKIP_FRIENDLY_NAME;
    let mut size: u32 = 16 * 1024;
    let mut collected = Vec::new();

    // The call reports the size it wants when the buffer is too small,
    // and the table can grow between the two calls, so this asks again
    // rather than assuming the second attempt must fit.
    for _ in 0..4 {
      let mut buffer = vec![0u8; size as usize];
      let head = buffer.as_mut_ptr() as *mut IP_ADAPTER_ADDRESSES_LH;

      let status =
        unsafe { GetAdaptersAddresses(AF_UNSPEC as u32, flags, std::ptr::null(), head, &mut size) };

      if status == ERROR_BUFFER_OVERFLOW {
        continue;
      }

      if status != ERROR_SUCCESS {
        return collected;
      }

      let mut adapter = head;

      while !adapter.is_null() {
        let entry = unsafe { &*adapter };

        if entry.OperStatus == IfOperStatusUp {
          take(entry, &mut collected);
        }

        adapter = entry.Next;
      }

      return collected;
    }

    collected
  }

  /// Reads an address out of a `sockaddr` without going through the
  /// per-family structs. The family is the first field either way, and
  /// the address sits at a fixed offset after it: 4 bytes in for IPv4,
  /// 8 for IPv6.
  fn address_text(sockaddr: *const SOCKADDR, length: i32) -> Option<String> {
    if sockaddr.is_null() {
      return None;
    }

    let family = unsafe { (*sockaddr).sa_family };
    let bytes = sockaddr as *const u8;

    if family == AF_INET && length >= 8 {
      let mut octets = [0u8; 4];

      unsafe { std::ptr::copy_nonoverlapping(bytes.add(4), octets.as_mut_ptr(), 4) };

      return Some(std::net::Ipv4Addr::from(octets).to_string());
    }

    if family == AF_INET6 && length >= 24 {
      let mut octets = [0u8; 16];

      unsafe { std::ptr::copy_nonoverlapping(bytes.add(8), octets.as_mut_ptr(), 16) };

      let address = std::net::Ipv6Addr::from(octets);

      // A link-local server is only reachable through the interface it
      // was learned on, which a plain address cannot say.
      if address.segments()[0] & 0xffc0 == 0xfe80 {
        return None;
      }

      return Some(address.to_string());
    }

    None
  }

  fn wide_text(mut cursor: *const u16) -> Option<String> {
    if cursor.is_null() {
      return None;
    }

    let mut units = Vec::new();

    unsafe {
      while *cursor != 0 {
        units.push(*cursor);
        cursor = cursor.add(1);
      }
    }

    if units.is_empty() {
      return None;
    }

    String::from_utf16(&units).ok()
  }

  pub fn servers() -> Vec<String> {
    let mut found = adapters(|adapter, into: &mut Vec<String>| {
      let mut server = adapter.FirstDnsServerAddress;

      while !server.is_null() {
        let entry = unsafe { &*server };

        if let Some(text) = address_text(entry.Address.lpSockaddr, entry.Address.iSockaddrLength) {
          into.push(text);
        }

        server = entry.Next;
      }
    });

    // Several adapters routinely point at the same resolver, and asking
    // one server twice is only a slower way to get the same answer.
    let mut seen = Vec::new();

    found.retain(|address| {
      if keep_plain_ip(address).is_none() {
        return false;
      }

      if seen.contains(address) {
        return false;
      }

      seen.push(address.clone());

      true
    });

    found
  }

  pub fn search() -> Vec<String> {
    let mut found = adapters(|adapter, into: &mut Vec<String>| {
      if let Some(suffix) = wide_text(adapter.DnsSuffix) {
        into.push(suffix);
      }
    });

    let mut seen = Vec::new();

    found.retain(|suffix| {
      if seen.contains(suffix) {
        return false;
      }

      seen.push(suffix.clone());

      true
    });

    found
  }
}
