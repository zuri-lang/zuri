//! Finding shared libraries and loading them.
//!
//! A name with a directory in it, or with a library suffix, is opened as
//! it stands. A bare name goes through the platform's naming convention
//! first: `sqlite3` is `libsqlite3.so` on Linux, `libsqlite3.dylib` on
//! macOS and `sqlite3.dll` on Windows. On Linux an unversioned `.so` is
//! usually only present when the development package is installed, so
//! the loader's own cache is consulted for the versioned name the
//! runtime package ships (`libsqlite3.so.0`).

use std::ffi::CString;
use std::path::Path;
#[cfg(unix)]
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

pub struct Library {
  pub handle: usize,
  /// What was opened: a path, or `<process>` for the running program.
  pub path: String,
  closed: AtomicBool,
  /// The process handle is never unloaded.
  unload: bool,
}

// SAFETY: a library handle is an opaque token the loader hands out and
// accepts from any thread.
unsafe impl Send for Library {}
unsafe impl Sync for Library {}

/// How the loader opens a library. Windows' loader has neither choice:
/// it binds every import at load, and a DLL's exports are reached only
/// through its own handle.
#[derive(Clone, Copy)]
#[cfg_attr(windows, allow(dead_code))]
pub struct OpenFlags {
  /// Resolve symbols as they are first used rather than all at load.
  pub lazy: bool,
  /// Make this library's symbols available to libraries loaded later.
  pub global: bool,
}

impl Library {
  pub fn is_closed(&self) -> bool {
    self.closed.load(Ordering::Acquire)
  }

  /// Marks the library closed. It is unloaded once nothing refers to it
  /// any longer, which includes every function bound from it.
  pub fn close(&self) {
    self.closed.store(true, Ordering::Release);
  }

  pub fn is_process(&self) -> bool {
    !self.unload
  }

  /// Opens exactly `path`, or the running process when `path` is `None`.
  pub fn open_exact(path: Option<&str>, flags: OpenFlags) -> Result<Library, String> {
    let handle = sys::open(path, flags)?;

    Ok(Library {
      handle,
      path: path
        .map(str::to_string)
        .unwrap_or_else(|| "<process>".into()),
      closed: AtomicBool::new(false),
      unload: path.is_some(),
    })
  }

  /// Opens `name`, searching `paths` first and then the platform's own
  /// search order.
  pub fn open(name: &str, paths: &[String], flags: OpenFlags) -> Result<Library, String> {
    let mut failures = Vec::new();

    for candidate in candidates(name, paths) {
      match Library::open_exact(Some(&candidate), flags) {
        Ok(lib) => return Ok(lib),
        Err(e) => failures.push(e),
      }
    }

    let detail = failures
      .into_iter()
      .find(|e| {
        !e.contains("No such file") && !e.contains("not found") && !e.contains("cannot find")
      })
      .map(|e| format!(": {e}"))
      .unwrap_or_default();

    Err(format!("cannot load '{name}'{detail}"))
  }

  /// The address of `name`, or `None` when the library does not export
  /// it. `version` picks a versioned symbol where the platform has them.
  pub fn symbol(&self, name: &str, version: Option<&str>) -> Result<Option<usize>, String> {
    // Only the process handle is never unloaded, and only it stands for
    // every module loaded into the process.
    sys::symbol(self.handle, !self.unload, name, version)
  }
}

impl Drop for Library {
  fn drop(&mut self) {
    if self.unload {
      sys::close(self.handle);
    }
  }
}

/// Whether `name` already names a file rather than a library.
fn is_explicit(name: &str) -> bool {
  name.contains('/')
    || name.contains('\\')
    || name.ends_with(".so")
    || name.contains(".so.")
    || name.ends_with(".dylib")
    || name.ends_with(".dll")
    || name.ends_with(".DLL")
    || name.contains(".framework/")
}

/// File names `name` could be, most likely first.
fn spellings(name: &str) -> Vec<String> {
  if is_explicit(name) {
    return vec![name.to_string()];
  }

  let bare = name.strip_prefix("lib").unwrap_or(name);
  let mut out = Vec::new();

  if cfg!(windows) {
    out.push(format!("{name}.dll"));
    out.push(format!("lib{bare}.dll"));
  } else if cfg!(target_vendor = "apple") {
    out.push(format!("lib{bare}.dylib"));
    out.push(format!("{name}.dylib"));
    out.push(format!("{name}.framework/{name}"));
  } else {
    out.push(format!("lib{bare}.so"));
    if let Some(versioned) = sys::cached_soname(bare) {
      out.push(versioned);
    }
    out.push(format!("{name}.so"));
  }

  out
}

/// Everything `open` tries for `name`, in order.
pub fn candidates(name: &str, paths: &[String]) -> Vec<String> {
  let names = spellings(name);
  let mut out = Vec::new();

  for dir in paths {
    for n in &names {
      let path = Path::new(dir).join(n);
      if path.exists() {
        out.push(path.to_string_lossy().into_owned());
      }
    }
  }

  if name.contains('/') || name.contains('\\') {
    out.push(name.to_string());
    return out;
  }

  for n in names {
    out.push(n);
  }

  out
}

/// The path `name` resolves to, without loading it, or `None` when it
/// cannot be found.
pub fn find(name: &str, paths: &[String]) -> Option<String> {
  for dir in paths {
    for n in spellings(name) {
      let path = Path::new(dir).join(&n);
      if path.exists() {
        return Some(path.to_string_lossy().into_owned());
      }
    }
  }

  if name.contains('/') || name.contains('\\') {
    return Path::new(name).exists().then(|| name.to_string());
  }

  for n in spellings(name) {
    if let Some(found) = sys::search(&n) {
      return Some(found);
    }
  }

  None
}

/// The C runtime's own library, as `open` should be given it.
pub fn libc_name() -> &'static str {
  if cfg!(windows) {
    "ucrtbase.dll"
  } else if cfg!(target_vendor = "apple") {
    "/usr/lib/libSystem.B.dylib"
  } else if cfg!(target_env = "musl") {
    "libc.so"
  } else {
    "libc.so.6"
  }
}

/// The C maths library.
pub fn libm_name() -> &'static str {
  if cfg!(windows) {
    "ucrtbase.dll"
  } else if cfg!(target_vendor = "apple") {
    "/usr/lib/libSystem.B.dylib"
  } else if cfg!(target_env = "musl") {
    "libc.so"
  } else {
    "libm.so.6"
  }
}

#[cfg(unix)]
fn standard_dirs() -> Vec<PathBuf> {
  let mut dirs = Vec::new();

  let var = if cfg!(target_vendor = "apple") {
    "DYLD_LIBRARY_PATH"
  } else {
    "LD_LIBRARY_PATH"
  };

  if let Ok(value) = std::env::var(var) {
    dirs.extend(std::env::split_paths(&value));
  }

  for d in [
    "/lib",
    "/usr/lib",
    "/usr/local/lib",
    "/lib64",
    "/usr/lib64",
    "/opt/homebrew/lib",
    "/lib/x86_64-linux-gnu",
    "/usr/lib/x86_64-linux-gnu",
    "/lib/aarch64-linux-gnu",
    "/usr/lib/aarch64-linux-gnu",
    "/Library/Frameworks",
    "/System/Library/Frameworks",
  ] {
    dirs.push(PathBuf::from(d));
  }

  dirs
}

#[cfg(unix)]
mod sys {
  use super::*;

  fn last_error() -> String {
    // SAFETY: dlerror returns either null or a NUL-terminated string
    // owned by the loader, valid until the next dl* call on this thread.
    let text = unsafe { libc::dlerror() };
    if text.is_null() {
      return "unknown error".into();
    }
    unsafe { std::ffi::CStr::from_ptr(text) }
      .to_string_lossy()
      .into_owned()
  }

  pub fn open(path: Option<&str>, flags: super::OpenFlags) -> Result<usize, String> {
    let mut mode = if flags.lazy {
      libc::RTLD_LAZY
    } else {
      libc::RTLD_NOW
    };
    mode |= if flags.global {
      libc::RTLD_GLOBAL
    } else {
      libc::RTLD_LOCAL
    };

    let c_path = match path {
      Some(p) => Some(CString::new(p).map_err(|_| format!("'{p}' contains a NUL byte"))?),
      None => None,
    };

    // SAFETY: the path is NUL-terminated, or null for the process.
    let handle = unsafe {
      libc::dlopen(
        c_path.as_ref().map_or(std::ptr::null(), |c| c.as_ptr()),
        mode,
      )
    };

    if handle.is_null() {
      return Err(last_error());
    }

    Ok(handle as usize)
  }

  pub fn close(handle: usize) {
    // SAFETY: the handle came from dlopen and is closed once, from Drop.
    unsafe { libc::dlclose(handle as *mut libc::c_void) };
  }

  pub fn symbol(
    handle: usize,
    _process: bool,
    name: &str,
    version: Option<&str>,
  ) -> Result<Option<usize>, String> {
    let c_name = CString::new(name).map_err(|_| format!("'{name}' contains a NUL byte"))?;

    // SAFETY: clearing any stale error so the one after dlsym is ours.
    unsafe { libc::dlerror() };

    let address = match version {
      None => unsafe { libc::dlsym(handle as *mut libc::c_void, c_name.as_ptr()) },
      Some(v) => versioned(handle, &c_name, v)?,
    };

    // A weak symbol nothing defined resolves to null without an error.
    // There is nothing at that address to call or read, so it counts as
    // absent either way.
    if address.is_null() {
      return Ok(None);
    }

    Ok(Some(address as usize))
  }

  #[cfg(all(target_os = "linux", target_env = "gnu"))]
  fn versioned(handle: usize, name: &CString, version: &str) -> Result<*mut libc::c_void, String> {
    let c_version =
      CString::new(version).map_err(|_| format!("'{version}' contains a NUL byte"))?;
    Ok(unsafe {
      libc::dlvsym(
        handle as *mut libc::c_void,
        name.as_ptr(),
        c_version.as_ptr(),
      )
    })
  }

  #[cfg(not(all(target_os = "linux", target_env = "gnu")))]
  fn versioned(
    _handle: usize,
    _name: &CString,
    _version: &str,
  ) -> Result<*mut libc::c_void, String> {
    Err("versioned symbols exist only with the GNU C library".into())
  }

  /// Where the dynamic loader would find `file`, without loading it.
  pub fn search(file: &str) -> Option<String> {
    for dir in standard_dirs() {
      let path = dir.join(file);
      if path.exists() {
        return Some(path.to_string_lossy().into_owned());
      }
    }

    if let Some(found) = cache_lookup(file) {
      return Some(found);
    }

    apple_preflight(file)
  }

  #[cfg(target_vendor = "apple")]
  fn apple_preflight(file: &str) -> Option<String> {
    // System libraries live in the dyld shared cache and have no file
    // on disk, so ask dyld whether it could load the name.
    unsafe extern "C" {
      fn dlopen_preflight(path: *const libc::c_char) -> bool;
    }

    for candidate in [file.to_string(), format!("/usr/lib/{file}")] {
      let c = CString::new(candidate.clone()).ok()?;
      if unsafe { dlopen_preflight(c.as_ptr()) } {
        return Some(candidate);
      }
    }

    None
  }

  #[cfg(not(target_vendor = "apple"))]
  fn apple_preflight(_file: &str) -> Option<String> {
    None
  }

  /// The versioned file name the loader cache lists for `lib<name>.so`.
  pub fn cached_soname(name: &str) -> Option<String> {
    let prefix = format!("lib{name}.so.");
    let entries = read_cache()?;

    // The highest version wins, which is what the development symlink
    // would have pointed at.
    entries
      .into_iter()
      .filter(|(soname, _)| soname.starts_with(&prefix))
      .max_by(|a, b| version_key(&a.0).cmp(&version_key(&b.0)))
      .map(|(soname, _)| soname)
  }

  fn cache_lookup(file: &str) -> Option<String> {
    read_cache()?
      .into_iter()
      .find(|(soname, _)| soname == file)
      .map(|(_, path)| path)
  }

  fn version_key(soname: &str) -> Vec<u64> {
    soname
      .split(".so.")
      .nth(1)
      .unwrap_or("")
      .split('.')
      .map(|p| p.parse().unwrap_or(0))
      .collect()
  }

  /// `(soname, path)` for every library `/etc/ld.so.cache` lists for
  /// this architecture. Only the format glibc has written since 2.32 is
  /// read, including when it follows the older one in the same file.
  #[cfg(target_os = "linux")]
  fn read_cache() -> Option<Vec<(String, String)>> {
    const MAGIC: &[u8] = b"glibc-ld.so.cache1.1";
    let data = std::fs::read("/etc/ld.so.cache").ok()?;
    let start = data.windows(MAGIC.len()).position(|w| w == MAGIC)?;

    let read_u32 = |at: usize| -> Option<u32> {
      data
        .get(at..at + 4)
        .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
    };

    let count = read_u32(start + MAGIC.len())? as usize;
    // Header: magic, nlibs, len_strings, flags(1) + padding(3),
    // extension offset, three unused words.
    let entries_at = start + MAGIC.len() + 4 + 4 + 4 + 4 + 12;

    let wanted: i32 = if cfg!(target_arch = "x86_64") {
      0x0300
    } else if cfg!(target_arch = "aarch64") {
      0x0a00
    } else {
      0
    };

    let cstr_at = |offset: usize| -> Option<String> {
      let at = start + offset;
      let end = data.get(at..)?.iter().position(|b| *b == 0)?;
      Some(String::from_utf8_lossy(&data[at..at + end]).into_owned())
    };

    let mut out = Vec::with_capacity(count);
    for i in 0..count {
      let at = entries_at + i * 24;
      let flags = read_u32(at)? as i32;
      let key = read_u32(at + 4)? as usize;
      let value = read_u32(at + 8)? as usize;

      if flags & 0x00ff != 0x0003 || flags & 0xff00 != wanted {
        continue;
      }

      if let (Some(soname), Some(path)) = (cstr_at(key), cstr_at(value)) {
        out.push((soname, path));
      }
    }

    Some(out)
  }

  #[cfg(not(target_os = "linux"))]
  fn read_cache() -> Option<Vec<(String, String)>> {
    None
  }
}

#[cfg(windows)]
mod sys {
  use super::*;
  use std::os::windows::ffi::OsStrExt;
  use windows_sys::Win32::Foundation::{FreeLibrary, GetLastError, HMODULE};
  use windows_sys::Win32::Storage::FileSystem::SearchPathW;
  use windows_sys::Win32::System::Diagnostics::Debug::{
    FORMAT_MESSAGE_FROM_SYSTEM, FORMAT_MESSAGE_IGNORE_INSERTS, FormatMessageW,
  };
  use windows_sys::Win32::System::LibraryLoader::{
    GetModuleHandleW, GetProcAddress, LOAD_LIBRARY_SEARCH_DEFAULT_DIRS,
    LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR, LoadLibraryExW,
  };
  use windows_sys::Win32::System::ProcessStatus::EnumProcessModules;
  use windows_sys::Win32::System::Threading::GetCurrentProcess;

  fn wide(text: &str) -> Vec<u16> {
    std::ffi::OsStr::new(text)
      .encode_wide()
      .chain(Some(0))
      .collect()
  }

  fn last_error() -> String {
    let code = unsafe { GetLastError() };
    let mut buffer = [0u16; 512];
    let n = unsafe {
      FormatMessageW(
        FORMAT_MESSAGE_FROM_SYSTEM | FORMAT_MESSAGE_IGNORE_INSERTS,
        std::ptr::null(),
        code,
        0,
        buffer.as_mut_ptr(),
        buffer.len() as u32,
        std::ptr::null(),
      )
    };
    let text = String::from_utf16_lossy(&buffer[..n as usize]);
    format!("{} (error {code})", text.trim())
  }

  pub fn open(path: Option<&str>, _flags: super::OpenFlags) -> Result<usize, String> {
    let Some(path) = path else {
      let handle = unsafe { GetModuleHandleW(std::ptr::null()) };
      return Ok(handle as usize);
    };

    let name = wide(path);
    let explicit = path.contains('/') || path.contains('\\');
    // With a directory given, the DLL's own directory is searched for
    // its dependencies too, the way a program's own DLLs are found.
    let flags = if explicit {
      LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_DEFAULT_DIRS
    } else {
      0
    };

    let handle = unsafe { LoadLibraryExW(name.as_ptr(), std::ptr::null_mut(), flags) };

    if handle.is_null() {
      return Err(last_error());
    }

    Ok(handle as usize)
  }

  pub fn close(handle: usize) {
    unsafe { FreeLibrary(handle as HMODULE) };
  }

  pub fn symbol(
    handle: usize,
    process: bool,
    name: &str,
    version: Option<&str>,
  ) -> Result<Option<usize>, String> {
    if version.is_some() {
      return Err("versioned symbols exist only with the GNU C library".into());
    }

    let c_name = CString::new(name).map_err(|_| format!("'{name}' contains a NUL byte"))?;

    let found = unsafe { GetProcAddress(handle as HMODULE, c_name.as_ptr() as *const u8) };
    if let Some(f) = found {
      return Ok(Some(f as usize));
    }

    if !process {
      return Ok(None);
    }

    // The process as a whole: every module loaded into it, in load
    // order, which is what dlsym on the process does on Unix.
    let process = unsafe { GetCurrentProcess() };
    let mut modules: Vec<HMODULE> = vec![std::ptr::null_mut(); 1024];
    let mut needed = 0u32;
    let ok = unsafe {
      EnumProcessModules(
        process,
        modules.as_mut_ptr(),
        (modules.len() * size_of::<HMODULE>()) as u32,
        &mut needed,
      )
    };

    if ok == 0 {
      return Ok(None);
    }

    let count = (needed as usize / size_of::<HMODULE>()).min(modules.len());
    for module in &modules[..count] {
      if let Some(f) = unsafe { GetProcAddress(*module, c_name.as_ptr() as *const u8) } {
        return Ok(Some(f as usize));
      }
    }

    Ok(None)
  }

  pub fn search(file: &str) -> Option<String> {
    let name = wide(file);
    let mut buffer = vec![0u16; 1024];
    let n = unsafe {
      SearchPathW(
        std::ptr::null(),
        name.as_ptr(),
        std::ptr::null(),
        buffer.len() as u32,
        buffer.as_mut_ptr(),
        std::ptr::null_mut(),
      )
    };

    if n == 0 || n as usize >= buffer.len() {
      return None;
    }

    Some(String::from_utf16_lossy(&buffer[..n as usize]))
  }

  pub fn cached_soname(_name: &str) -> Option<String> {
    None
  }
}
