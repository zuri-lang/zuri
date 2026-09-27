//! Turning static libraries into something that can be loaded.
//!
//! A static library is object code waiting for a linker, and nothing at
//! run time can load it as it stands. So the platform's own linker links
//! every object in it into a shared library, which is then opened like
//! any other. The result is cached under a name derived from the
//! archives' contents and the options, so the linker runs once per
//! distinct input rather than once per program start.
//!
//! A Rust `staticlib` carries the standard library with it, and that in
//! turn needs a handful of system libraries. An archive holding Rust
//! code is recognised and gets them without being asked.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use sha2::{Digest, Sha256};

#[derive(Default)]
pub struct LinkOptions {
  /// Extra libraries to link against, by name: `m`, `ssl`.
  pub libraries: Vec<String>,
  /// Directories searched for those libraries.
  pub search_paths: Vec<String>,
  /// On Windows, the symbols the DLL exports. Defaults to every symbol
  /// the archives define with an unmangled name.
  pub exports: Option<Vec<String>>,
  /// The linker, or the compiler driver that runs it.
  pub linker: Option<String>,
  /// Further arguments, passed to the linker as they are.
  pub flags: Vec<String>,
  /// Where linked libraries are kept.
  pub cache: Option<String>,
  /// Whether the archives hold Rust code. Detected when not given.
  pub rust: Option<bool>,
}

fn extension() -> &'static str {
  if cfg!(windows) {
    "dll"
  } else if cfg!(target_vendor = "apple") {
    "dylib"
  } else {
    "so"
  }
}

/// The default cache: the platform's per-user cache directory.
pub fn default_cache() -> PathBuf {
  let base = if cfg!(windows) {
    std::env::var_os("LOCALAPPDATA").map(PathBuf::from)
  } else if cfg!(target_vendor = "apple") {
    std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Library").join("Caches"))
  } else {
    std::env::var_os("XDG_CACHE_HOME")
      .map(PathBuf::from)
      .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache")))
  };

  base
    .unwrap_or_else(std::env::temp_dir)
    .join("zuri")
    .join("ffi")
}

/// Links `archives` into a shared library and returns its path.
pub fn link(archives: &[String], options: &LinkOptions) -> Result<String, String> {
  if archives.is_empty() {
    return Err("give at least one static library to link".into());
  }

  let mut hasher = Sha256::new();
  let mut contents = Vec::with_capacity(archives.len());

  for path in archives {
    let bytes = fs::read(path).map_err(|e| format!("cannot read '{path}': {e}"))?;
    if !bytes.starts_with(b"!<arch>\n") {
      return Err(format!("'{path}' is not a static library"));
    }
    hasher.update((bytes.len() as u64).to_le_bytes());
    hasher.update(&bytes);
    contents.push(bytes);
  }

  let rust = options
    .rust
    .unwrap_or_else(|| contents.iter().any(|b| holds_rust(b)));

  for part in [
    options.libraries.join("\0"),
    options.search_paths.join("\0"),
    options
      .exports
      .as_ref()
      .map(|e| e.join("\0"))
      .unwrap_or_default(),
    options.linker.clone().unwrap_or_default(),
    options.flags.join("\0"),
    rust.to_string(),
    std::env::consts::OS.to_string(),
    std::env::consts::ARCH.to_string(),
  ] {
    hasher.update(part.as_bytes());
    hasher.update([0xff]);
  }

  let digest = hasher.finalize();
  let key: String = digest[..12].iter().map(|b| format!("{b:02x}")).collect();

  let cache = options
    .cache
    .as_ref()
    .map(PathBuf::from)
    .unwrap_or_else(default_cache);
  fs::create_dir_all(&cache)
    .map_err(|e| format!("cannot create the cache '{}': {e}", cache.display()))?;

  let output = cache.join(format!("link-{key}.{}", extension()));

  if output.exists() {
    return Ok(output.to_string_lossy().into_owned());
  }

  // Linked under a temporary name and renamed into place, so a program
  // never opens a half-written library another process is producing.
  let partial = cache.join(format!(
    "link-{key}.{}.{}.partial",
    std::process::id(),
    extension()
  ));

  let absolute: Vec<PathBuf> = archives
    .iter()
    .map(|a| fs::canonicalize(a).unwrap_or_else(|_| PathBuf::from(a)))
    .collect();

  let result = run_linker(&absolute, &contents, &partial, options, rust);

  if let Err(e) = result {
    let _ = fs::remove_file(&partial);
    return Err(e);
  }

  if let Err(e) = fs::rename(&partial, &output) {
    let _ = fs::remove_file(&partial);
    if !output.exists() {
      return Err(format!(
        "cannot move the linked library into the cache: {e}"
      ));
    }
  }

  Ok(output.to_string_lossy().into_owned())
}

/// Whether an archive holds Rust code: every Rust staticlib carries the
/// personality routine its unwinding needs.
fn holds_rust(bytes: &[u8]) -> bool {
  let needles: [&[u8]; 2] = [b"rust_eh_personality", b"__rust_alloc"];
  needles
    .iter()
    .any(|n| bytes.windows(n.len()).any(|w| w == *n))
}

fn run_output(mut command: Command) -> Result<(), String> {
  let shown = format!("{command:?}");
  let out = command
    .output()
    .map_err(|e| format!("cannot run the linker ({shown}): {e}"))?;

  if !out.status.success() {
    let mut text = String::from_utf8_lossy(&out.stderr).into_owned();
    text.push_str(&String::from_utf8_lossy(&out.stdout));
    return Err(format!(
      "the linker failed ({}):\n{}",
      out.status,
      text.trim()
    ));
  }

  Ok(())
}

#[cfg(not(windows))]
fn run_linker(
  archives: &[PathBuf],
  _contents: &[Vec<u8>],
  output: &Path,
  options: &LinkOptions,
  rust: bool,
) -> Result<(), String> {
  let linker = options
    .linker
    .clone()
    .or_else(|| std::env::var("CC").ok())
    .unwrap_or_else(|| "cc".into());

  let mut command = Command::new(&linker);

  if cfg!(target_vendor = "apple") {
    command.arg("-dynamiclib");
    for a in archives {
      command.arg(format!("-Wl,-force_load,{}", a.display()));
    }
  } else {
    command.arg("-shared");
    command.arg("-Wl,--whole-archive");
    for a in archives {
      command.arg(a);
    }
    command.arg("-Wl,--no-whole-archive");
  }

  command.arg("-o").arg(output);

  for dir in &options.search_paths {
    command.arg(format!("-L{dir}"));
  }
  for lib in &options.libraries {
    command.arg(format!("-l{lib}"));
  }

  if rust {
    let system: &[&str] = if cfg!(target_vendor = "apple") {
      &["-lSystem", "-lc", "-lm", "-liconv"]
    } else {
      &[
        "-lgcc_s",
        "-lutil",
        "-lrt",
        "-lpthread",
        "-lm",
        "-ldl",
        "-lc",
      ]
    };
    command.args(system);
  }

  command.args(&options.flags);

  run_output(command)
}

#[cfg(windows)]
fn run_linker(
  archives: &[PathBuf],
  contents: &[Vec<u8>],
  output: &Path,
  options: &LinkOptions,
  rust: bool,
) -> Result<(), String> {
  let mut command = match &options.linker {
    Some(l) => Command::new(l),
    None => find_msvc_tools::find("x86_64", "link.exe").ok_or_else(|| {
      "cannot find link.exe; install the Visual Studio C++ build tools or pass the linker option"
        .to_string()
    })?,
  };

  command
    .arg("/NOLOGO")
    .arg("/DLL")
    .arg(format!("/OUT:{}", output.display()));

  for a in archives {
    command.arg(format!("/WHOLEARCHIVE:{}", a.display()));
    command.arg(a);
  }

  for dir in &options.search_paths {
    command.arg(format!("/LIBPATH:{dir}"));
  }

  for lib in &options.libraries {
    if lib.ends_with(".lib") {
      command.arg(lib);
    } else {
      command.arg(format!("{lib}.lib"));
    }
  }

  if rust {
    command.args([
      "kernel32.lib",
      "advapi32.lib",
      "ntdll.lib",
      "userenv.lib",
      "ws2_32.lib",
      "dbghelp.lib",
      "bcrypt.lib",
      "synchronization.lib",
    ]);
  }

  command.arg("/DEFAULTLIB:msvcrt");

  // A static library's functions are not marked for export, so the DLL
  // is told which to export by name.
  let exports = match &options.exports {
    Some(list) => list.clone(),
    None => {
      let mut all = Vec::new();
      for bytes in contents {
        all.extend(archive_symbols(bytes).into_iter().filter(|s| exportable(s)));
      }
      all.sort();
      all.dedup();
      all
    },
  };

  if exports.is_empty() {
    return Err(
      "the archives define no C symbols to export; name them with the exports option".into(),
    );
  }

  // Thousands of exports overflow a command line, so they go in a
  // response file.
  let response = output.with_extension("exports.rsp");
  let text: String = exports.iter().map(|e| format!("/EXPORT:{e}\n")).collect();
  fs::write(&response, text).map_err(|e| format!("cannot write '{}': {e}", response.display()))?;
  command.arg(format!("@{}", response.display()));

  command.args(&options.flags);

  let result = run_output(command);
  let _ = fs::remove_file(&response);
  let _ = fs::remove_file(output.with_extension("lib"));
  let _ = fs::remove_file(output.with_extension("exp"));
  result
}

/// A symbol a C caller could name: not C++ or Rust mangled, not an
/// import thunk, not a compiler-internal name.
#[cfg(windows)]
fn exportable(symbol: &str) -> bool {
  !(symbol.starts_with('?')
    || symbol.starts_with("_ZN")
    || symbol.starts_with("_R")
    || symbol.starts_with("__imp_")
    || symbol.starts_with("__rust")
    || symbol.starts_with("rust_")
    || symbol.starts_with("__")
    || symbol.starts_with('.')
    || symbol.starts_with('$')
    || symbol.contains('@'))
}

/// The symbols an archive's index says it defines. Reads the GNU and
/// COFF index, which is the first member, named `/`.
#[cfg(windows)]
fn archive_symbols(bytes: &[u8]) -> Vec<String> {
  let mut out = Vec::new();
  let mut at = 8;

  if at + 60 > bytes.len() {
    return out;
  }

  let header = &bytes[at..at + 60];
  let name = String::from_utf8_lossy(&header[..16]).trim().to_string();
  let size: usize = String::from_utf8_lossy(&header[48..58])
    .trim()
    .parse()
    .unwrap_or(0);
  at += 60;

  if name != "/" || at + size > bytes.len() || size < 4 {
    return out;
  }

  let member = &bytes[at..at + size];
  let count = u32::from_be_bytes(member[..4].try_into().unwrap()) as usize;
  let names_at = 4 + count * 4;

  if names_at > member.len() {
    return out;
  }

  for raw in member[names_at..].split(|b| *b == 0).take(count) {
    if !raw.is_empty() {
      out.push(String::from_utf8_lossy(raw).into_owned());
    }
  }

  out
}
