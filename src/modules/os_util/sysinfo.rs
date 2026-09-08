//! Per-platform system/machine introspection that has no portable
//! std API: hostname, physical/available memory, uptime, and (on
//! Windows, which has no direct syscall for it) the parent process
//! id. Every function here is a real syscall or a read of the
//! platform's own accounting file, never a shell-out — the same
//! standard `uname` shelled out to a whole subprocess just to read
//! static facts, which is what this replaces it with.

#[cfg(windows)]
use windows_sys::Win32::System::{
  Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS,
  },
  Foundation::{CloseHandle, INVALID_HANDLE_VALUE},
  SystemInformation::{
    ComputerNamePhysicalDnsHostname, GetComputerNameExW, GetTickCount64, GlobalMemoryStatusEx,
    MEMORYSTATUSEX,
  },
};

// --- hostname ---------------------------------------------------------

#[cfg(unix)]
pub fn hostname() -> Result<String, String> {
  let mut buf = vec![0u8; 256];
  let ret = unsafe { libc::gethostname(buf.as_mut_ptr() as *mut libc::c_char, buf.len()) };
  if ret != 0 {
    return Err("could not determine the machine's hostname".to_string());
  }
  let end = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
  Ok(String::from_utf8_lossy(&buf[..end]).into_owned())
}

#[cfg(windows)]
pub fn hostname() -> Result<String, String> {
  // First call with no buffer just asks how big one needs to be.
  let mut len: u32 = 0;
  unsafe {
    GetComputerNameExW(
      ComputerNamePhysicalDnsHostname,
      std::ptr::null_mut(),
      &mut len,
    );
  }
  if len == 0 {
    return Err("could not determine the machine's hostname".to_string());
  }

  let mut buf = vec![0u16; len as usize];
  let ok =
    unsafe { GetComputerNameExW(ComputerNamePhysicalDnsHostname, buf.as_mut_ptr(), &mut len) };
  if ok == 0 {
    return Err("could not determine the machine's hostname".to_string());
  }
  Ok(String::from_utf16_lossy(&buf[..len as usize]))
}

#[cfg(not(any(unix, windows)))]
pub fn hostname() -> Result<String, String> {
  Err("hostname() is not supported on this platform".to_string())
}

// --- uname (sysname/nodename/version/release/machine) -----------------

/// Real `uname(2)` on Unix, replacing what used to be a `uname`
/// subprocess shell-out. `nodename` is filled in from `hostname()`
/// above rather than a second syscall for the same fact.
#[cfg(unix)]
pub fn uname() -> (String, String, String, String, String) {
  unsafe {
    let mut buf: libc::utsname = std::mem::zeroed();
    if libc::uname(&mut buf) != 0 {
      return uname_fallback();
    }
    let sysname = cstr_field(&buf.sysname);
    let version = cstr_field(&buf.version);
    let release = cstr_field(&buf.release);
    let machine = cstr_field(&buf.machine);
    let nodename = hostname().unwrap_or_else(|_| cstr_field(&buf.nodename));
    (sysname, nodename, version, release, machine)
  }
}

#[cfg(unix)]
unsafe fn cstr_field(field: &[std::os::raw::c_char]) -> String {
  let bytes: Vec<u8> = field
    .iter()
    .take_while(|&&c| c != 0)
    .map(|&c| c as u8)
    .collect();
  String::from_utf8_lossy(&bytes).into_owned()
}

#[cfg(unix)]
fn uname_fallback() -> (String, String, String, String, String) {
  (
    "unknown".to_string(),
    hostname().unwrap_or_else(|_| "unknown".to_string()),
    "unknown".to_string(),
    "unknown".to_string(),
    std::env::consts::ARCH.to_string(),
  )
}

/// No `uname(2)` equivalent on Windows; built from Rust's own
/// compile-time platform constants plus the real hostname, same as
/// the previous shell-out-free fallback this replaces.
#[cfg(windows)]
pub fn uname() -> (String, String, String, String, String) {
  let nodename = hostname().unwrap_or_else(|_| "unknown".to_string());
  (
    std::env::consts::OS.to_string(),
    nodename,
    "unknown".to_string(),
    "unknown".to_string(),
    std::env::consts::ARCH.to_string(),
  )
}

#[cfg(not(any(unix, windows)))]
pub fn uname() -> (String, String, String, String, String) {
  (
    std::env::consts::OS.to_string(),
    "unknown".to_string(),
    "unknown".to_string(),
    "unknown".to_string(),
    std::env::consts::ARCH.to_string(),
  )
}

// --- memory -------------------------------------------------------------

/// `(total_bytes, available_bytes)`. "Available" follows `/proc/
/// meminfo`'s own `MemAvailable` on Linux — free memory plus
/// reclaimable caches, the number that actually reflects what a new
/// process could get without swapping, the same figure `free -h`
/// reports rather than the much smaller and less useful raw
/// `MemFree`.
#[cfg(target_os = "linux")]
pub fn memory() -> Result<(u64, u64), String> {
  let content = std::fs::read_to_string("/proc/meminfo")
    .map_err(|e| format!("could not read /proc/meminfo: {}", e))?;

  let mut total_kb = None;
  let mut avail_kb = None;
  for line in content.lines() {
    if let Some(rest) = line.strip_prefix("MemTotal:") {
      total_kb = parse_kb_field(rest);
    } else if let Some(rest) = line.strip_prefix("MemAvailable:") {
      avail_kb = parse_kb_field(rest);
    }
  }

  match (total_kb, avail_kb) {
    (Some(total), Some(avail)) => Ok((total * 1024, avail * 1024)),
    _ => Err("could not find MemTotal/MemAvailable in /proc/meminfo".to_string()),
  }
}

#[cfg(target_os = "linux")]
fn parse_kb_field(s: &str) -> Option<u64> {
  s.split_whitespace().next()?.parse::<u64>().ok()
}

#[cfg(target_os = "macos")]
pub fn memory() -> Result<(u64, u64), String> {
  let total = macos_sysctl_u64(&mut [libc::CTL_HW, libc::HW_MEMSIZE])?;

  // Free + inactive pages are, together, what Activity Monitor itself
  // calls "available" memory: pages that hold nothing that can't
  // simply be dropped or is trivially reclaimable, as opposed to
  // `free_count` alone, which undercounts what a new allocation could
  // actually get without swapping.
  let page_size = unsafe { libc::sysconf(libc::_SC_PAGESIZE) } as u64;
  let mut info: libc::vm_statistics64 = unsafe { std::mem::zeroed() };
  let mut count = (std::mem::size_of::<libc::vm_statistics64>()
    / std::mem::size_of::<libc::integer_t>()) as libc::mach_msg_type_number_t;
  let ret = unsafe {
    libc::host_statistics64(
      libc::mach_host_self(),
      libc::HOST_VM_INFO64,
      &mut info as *mut _ as libc::host_info64_t,
      &mut count,
    )
  };
  if ret != libc::KERN_SUCCESS {
    return Err("host_statistics64() failed".to_string());
  }

  let available = (info.free_count as u64 + info.inactive_count as u64) * page_size;
  Ok((total, available))
}

#[cfg(target_os = "macos")]
fn macos_sysctl_u64(mib: &mut [libc::c_int]) -> Result<u64, String> {
  let mut value: u64 = 0;
  let mut len = std::mem::size_of::<u64>();
  let ret = unsafe {
    libc::sysctl(
      mib.as_mut_ptr(),
      mib.len() as u32,
      &mut value as *mut _ as *mut libc::c_void,
      &mut len,
      std::ptr::null_mut(),
      0,
    )
  };
  if ret != 0 {
    return Err("sysctl() failed".to_string());
  }
  Ok(value)
}

#[cfg(windows)]
pub fn memory() -> Result<(u64, u64), String> {
  let mut status: MEMORYSTATUSEX = unsafe { std::mem::zeroed() };
  status.dwLength = std::mem::size_of::<MEMORYSTATUSEX>() as u32;
  let ok = unsafe { GlobalMemoryStatusEx(&mut status) };
  if ok == 0 {
    return Err("GlobalMemoryStatusEx() failed".to_string());
  }
  Ok((status.ullTotalPhys, status.ullAvailPhys))
}

#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
pub fn memory() -> Result<(u64, u64), String> {
  Err("memory introspection is not supported on this platform".to_string())
}

// --- uptime ---------------------------------------------------------------

/// Seconds since the machine booted.
#[cfg(target_os = "linux")]
pub fn uptime() -> Result<f64, String> {
  let content = std::fs::read_to_string("/proc/uptime")
    .map_err(|e| format!("could not read /proc/uptime: {}", e))?;
  content
    .split_whitespace()
    .next()
    .and_then(|s| s.parse::<f64>().ok())
    .ok_or_else(|| "could not parse /proc/uptime".to_string())
}

#[cfg(target_os = "macos")]
pub fn uptime() -> Result<f64, String> {
  let mut mib = [libc::CTL_KERN, libc::KERN_BOOTTIME];
  let mut boottime: libc::timeval = unsafe { std::mem::zeroed() };
  let mut len = std::mem::size_of::<libc::timeval>();
  let ret = unsafe {
    libc::sysctl(
      mib.as_mut_ptr(),
      mib.len() as u32,
      &mut boottime as *mut _ as *mut libc::c_void,
      &mut len,
      std::ptr::null_mut(),
      0,
    )
  };
  if ret != 0 {
    return Err("sysctl(KERN_BOOTTIME) failed".to_string());
  }

  let now = std::time::SystemTime::now()
    .duration_since(std::time::UNIX_EPOCH)
    .map_err(|e| e.to_string())?;
  let boot_secs = boottime.tv_sec as f64 + boottime.tv_usec as f64 / 1_000_000.0;
  Ok((now.as_secs_f64() - boot_secs).max(0.0))
}

#[cfg(windows)]
pub fn uptime() -> Result<f64, String> {
  Ok(unsafe { GetTickCount64() } as f64 / 1000.0)
}

#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
pub fn uptime() -> Result<f64, String> {
  Err("uptime() is not supported on this platform".to_string())
}

// --- parent process id ------------------------------------------------

#[cfg(unix)]
pub fn ppid() -> u32 {
  unsafe { libc::getppid() as u32 }
}

/// Windows has no direct "ask my parent's pid" syscall; the standard
/// workaround is walking a process snapshot looking for the entry
/// that matches this process, then reading ITS `th32ParentProcessID`.
#[cfg(windows)]
pub fn ppid() -> u32 {
  let current_pid = std::process::id();

  unsafe {
    let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
    if snapshot.is_null() || snapshot == INVALID_HANDLE_VALUE {
      return 0;
    }

    let mut entry: PROCESSENTRY32W = std::mem::zeroed();
    entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;

    let mut found_ppid = 0u32;
    if Process32FirstW(snapshot, &mut entry) != 0 {
      loop {
        if entry.th32ProcessID == current_pid {
          found_ppid = entry.th32ParentProcessID;
          break;
        }
        if Process32NextW(snapshot, &mut entry) == 0 {
          break;
        }
      }
    }

    CloseHandle(snapshot);
    found_ppid
  }
}

#[cfg(not(any(unix, windows)))]
pub fn ppid() -> u32 {
  0
}
