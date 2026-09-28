//! Running a program that `zuri bundle` packaged.
//!
//! A bundle is the runtime renamed after the application, with its
//! standard library and the application beside it:
//!
//! ```text
//! myapp/
//!   myapp          the runtime, renamed
//!   bundle.toml    marks the directory as a bundle
//!   libs/          the standard library it was built with
//!   app/           the application, index.zu first
//! ```
//!
//! A macOS application keeps the same files under `Contents/Resources`
//! with the executable in `Contents/MacOS`.
//!
//! A single-file bundle is the runtime with those files appended to it
//! as a payload, followed by a fixed-size trailer. The first run checks
//! the payload against the digest in the trailer and unpacks it into
//! the user's cache under that digest, and every later run finds it
//! there. Unpacking goes into a private directory that is renamed into
//! place, so two copies starting at once never see half a bundle.
//!
//! The payload is zlib-compressed. Decompressed, it is a run of
//! entries, each one:
//!
//! | Field | Size | Meaning |
//! | --- | --- | --- |
//! | kind | 1 | `0` for a file, `1` for a directory |
//! | mode | 4 | permission bits, little-endian |
//! | path length | 4 | little-endian |
//! | path | path length | UTF-8, `/`-separated, relative |
//! | size | 8 | files only, little-endian |
//! | data | size | files only |
//!
//! The trailer is the payload's SHA-256, its length as a little-endian
//! `u64`, then the eight bytes `ZURIBND1`.

use std::fs;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Component, Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::modules::compress_util::DeflateDecoder;

/// The file whose presence beside the executable makes a run a bundle.
pub const MARKER: &str = "bundle.toml";

/// Where the application sits inside a bundle.
pub const APP_DIR: &str = "app";

/// The file the application is started through.
pub const ENTRY: &str = "index.zu";

/// The last eight bytes of a single-file bundle.
pub const MAGIC: &[u8; 8] = b"ZURIBND1";

/// The fixed size of the trailer: digest, length, magic.
pub const TRAILER_LEN: u64 = 32 + 8 + 8;

const KIND_FILE: u8 = 0;
const KIND_DIR: u8 = 1;

/// A bundle found around the running executable.
pub struct Bundle {
  /// The directory holding `bundle.toml`, `libs` and `app`.
  pub root: PathBuf,
}

impl Bundle {
  /// The file the application starts from.
  pub fn entry(&self) -> PathBuf {
    self.root.join(APP_DIR).join(ENTRY)
  }

  /// The application's own directory.
  pub fn app_dir(&self) -> PathBuf {
    self.root.join(APP_DIR)
  }
}

/// The bundle this executable belongs to, if it belongs to one.
///
/// A payload appended to the executable wins, then a `bundle.toml`
/// beside it, then one in `../Resources` for a macOS application. A
/// plain runtime has none of the three and gets `Ok(None)` after one
/// short read of its own last bytes.
pub fn detect() -> Result<Option<Bundle>, String> {
  let Ok(exe) = std::env::current_exe() else {
    return Ok(None);
  };

  if let Some(root) = unpack_payload(&exe)? {
    return Ok(Some(Bundle { root }));
  }

  let Some(dir) = exe.parent() else {
    return Ok(None);
  };

  let candidates = [dir.to_path_buf(), dir.join("..").join("Resources")];

  for candidate in candidates {
    if candidate.join(MARKER).is_file() {
      let root = candidate.canonicalize().unwrap_or(candidate);
      return Ok(Some(Bundle { root }));
    }
  }

  Ok(None)
}

/// Reads the trailer of `exe`, and when there is a payload, makes sure
/// it is unpacked and returns where.
fn unpack_payload(exe: &Path) -> Result<Option<PathBuf>, String> {
  let Ok(mut file) = fs::File::open(exe) else {
    return Ok(None);
  };

  let Ok(size) = file.seek(SeekFrom::End(0)) else {
    return Ok(None);
  };

  if size < TRAILER_LEN {
    return Ok(None);
  }

  let mut trailer = [0u8; TRAILER_LEN as usize];

  if file.seek(SeekFrom::Start(size - TRAILER_LEN)).is_err() || file.read_exact(&mut trailer).is_err()
  {
    return Ok(None);
  }

  if &trailer[40..48] != MAGIC {
    return Ok(None);
  }

  let digest: [u8; 32] = trailer[0..32].try_into().expect("32 bytes");
  let length = u64::from_le_bytes(trailer[32..40].try_into().expect("8 bytes"));

  if length > size - TRAILER_LEN {
    return Err("the bundle is damaged: its payload is longer than the file".into());
  }

  let hex = hex_of(&digest);
  let target = crate::project::user_cache_dir().join("bundles").join(&hex);

  if target.join(MARKER).is_file() {
    return Ok(Some(target));
  }

  let mut payload = vec![0u8; length as usize];

  file
    .seek(SeekFrom::Start(size - TRAILER_LEN - length))
    .and_then(|_| file.read_exact(&mut payload))
    .map_err(|e| format!("could not read the bundle's payload: {e}"))?;

  if Sha256::digest(&payload).as_slice() != digest {
    return Err("the bundle is damaged: its payload does not match its digest".into());
  }

  let mut decoder = DeflateDecoder::new(payload, true, 15);
  let mut stream = Vec::new();

  decoder
    .read_to_end(&mut stream)
    .map_err(|e| format!("the bundle is damaged: {e:?}"))?;

  install(&stream, &target)?;

  Ok(Some(target))
}

/// Unpacks `stream` into a private directory beside `target`, then
/// renames it into place. Losing the race to another copy of the same
/// bundle is fine: whichever rename lands first is the one used.
fn install(stream: &[u8], target: &Path) -> Result<(), String> {
  let parent = target
    .parent()
    .ok_or_else(|| "the bundle cache has no parent directory".to_string())?;

  fs::create_dir_all(parent).map_err(|e| format!("could not create '{}': {e}", parent.display()))?;

  let staging = parent.join(format!(
    ".{}.{}",
    target.file_name().and_then(|n| n.to_str()).unwrap_or("bundle"),
    std::process::id()
  ));

  let _ = fs::remove_dir_all(&staging);

  let unpacked = fs::create_dir_all(&staging)
    .map_err(|e| format!("could not create '{}': {e}", staging.display()))
    .and_then(|_| unpack(stream, &staging));

  if let Err(e) = unpacked {
    let _ = fs::remove_dir_all(&staging);
    return Err(e);
  }

  if fs::rename(&staging, target).is_err() {
    let _ = fs::remove_dir_all(&staging);

    if !target.join(MARKER).is_file() {
      return Err(format!("could not unpack the bundle into '{}'", target.display()));
    }
  }

  Ok(())
}

/// A cursor over the decompressed payload that refuses to read past
/// its end.
struct Entries<'a> {
  stream: &'a [u8],
  at: usize,
}

impl<'a> Entries<'a> {
  fn take(&mut self, n: usize) -> Result<&'a [u8], String> {
    let end = self
      .at
      .checked_add(n)
      .filter(|end| *end <= self.stream.len())
      .ok_or_else(|| "the bundle is damaged: an entry runs past the end".to_string())?;

    let slice = &self.stream[self.at..end];
    self.at = end;

    Ok(slice)
  }

  fn u32(&mut self) -> Result<u32, String> {
    Ok(u32::from_le_bytes(self.take(4)?.try_into().expect("4 bytes")))
  }

  fn u64(&mut self) -> Result<u64, String> {
    Ok(u64::from_le_bytes(self.take(8)?.try_into().expect("8 bytes")))
  }

  fn done(&self) -> bool {
    self.at >= self.stream.len()
  }
}

/// Writes every entry of `stream` under `root`.
fn unpack(stream: &[u8], root: &Path) -> Result<(), String> {
  let mut entries = Entries { stream, at: 0 };

  while !entries.done() {
    let kind = entries.take(1)?[0];
    let mode = entries.u32()?;
    let path_len = entries.u32()? as usize;
    let raw = std::str::from_utf8(entries.take(path_len)?)
      .map_err(|_| "the bundle is damaged: an entry's path is not UTF-8".to_string())?;

    let path = root.join(safe_relative(raw)?);

    match kind {
      KIND_DIR => {
        fs::create_dir_all(&path).map_err(|e| format!("could not create '{}': {e}", path.display()))?;
      },
      KIND_FILE => {
        let size = entries.u64()? as usize;
        let data = entries.take(size)?;

        if let Some(dir) = path.parent() {
          fs::create_dir_all(dir).map_err(|e| format!("could not create '{}': {e}", dir.display()))?;
        }

        fs::write(&path, data).map_err(|e| format!("could not write '{}': {e}", path.display()))?;
        set_mode(&path, mode);
      },
      other => return Err(format!("the bundle is damaged: unknown entry kind {other}")),
    }
  }

  Ok(())
}

/// `raw` as a path that stays inside the directory it is joined to.
fn safe_relative(raw: &str) -> Result<PathBuf, String> {
  let mut path = PathBuf::new();

  for part in raw.split('/') {
    if part.is_empty() || part == "." {
      continue;
    }

    let piece = Path::new(part);
    let mut components = piece.components();

    match (components.next(), components.next()) {
      (Some(Component::Normal(_)), None) => path.push(part),
      _ => return Err(format!("the bundle is damaged: '{raw}' leaves the bundle")),
    }
  }

  if path.as_os_str().is_empty() {
    return Err("the bundle is damaged: an entry has an empty path".into());
  }

  Ok(path)
}

#[cfg(unix)]
fn set_mode(path: &Path, mode: u32) {
  use std::os::unix::fs::PermissionsExt;

  if mode != 0 {
    let _ = fs::set_permissions(path, fs::Permissions::from_mode(mode & 0o777));
  }
}

#[cfg(not(unix))]
fn set_mode(_path: &Path, _mode: u32) {}

fn hex_of(bytes: &[u8]) -> String {
  bytes.iter().map(|b| format!("{b:02x}")).collect()
}
