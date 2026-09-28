//! Where a program's project is, and where Zuri keeps its per-user
//! state.
//!
//! A project is the nearest directory holding a `project.toml`. Its
//! `.zuri/libs` is where installed and vendored modules live, and its
//! `.zuri/cmds` holds the commands it carries. Both are found from the
//! project root rather than from wherever the program happened to be
//! started, so `zuri run app/tool.zu` from inside `app/` sees the same
//! modules as `zuri run` from the root.
//!
//! The search starts from an anchor the executable settles before
//! anything runs: the directory of the script for `zuri run`, and the
//! working directory for a command or the REPL, because a command
//! works on the directory it was started in. Without a project above
//! the anchor, the working directory stands in for the root, which is
//! how vendoring into `.zuri/libs` works for a script with no manifest
//! at all.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// The file whose presence makes a directory a project root.
pub const MANIFEST: &str = "project.toml";

/// The directory, under a project root or under `ZURI_HOME`, that holds
/// installed modules.
pub const LIBS_DIR: &str = "libs";

/// The per-project directory Zuri keeps its state in.
pub const STATE_DIR: &str = ".zuri";

static ANCHOR: OnceLock<PathBuf> = OnceLock::new();

static BUNDLE: OnceLock<PathBuf> = OnceLock::new();

static ROOT: OnceLock<Option<PathBuf>> = OnceLock::new();

/// Fixes the directory the project search starts from. Only the first
/// call counts: the answer has to be the same for every module and
/// every isolate for the whole run.
pub fn set_anchor(dir: impl Into<PathBuf>) {
  let _ = ANCHOR.set(dir.into());
}

/// Marks this run as a bundled application whose runtime files sit in
/// `dir`. From then on the standard library is read from `dir/libs`
/// whatever `ZURI_ROOT` says, and nothing installed for the user is
/// consulted: a bundle runs the code it was built with.
pub fn set_bundle_root(dir: impl Into<PathBuf>) {
  let _ = BUNDLE.set(dir.into());
}

/// The directory of the bundle this run belongs to, if it is one.
pub fn bundle_root() -> Option<PathBuf> {
  BUNDLE.get().cloned()
}

/// The nearest directory at or above `start` that holds a
/// `project.toml`.
pub fn find_root(start: &Path) -> Option<PathBuf> {
  let absolute = match start.is_absolute() {
    true => start.to_path_buf(),
    false => std::env::current_dir().ok()?.join(start),
  };

  absolute
    .ancestors()
    .find(|dir| dir.join(MANIFEST).is_file())
    .map(Path::to_path_buf)
}

/// The project root this run belongs to, or `None` when no
/// `project.toml` sits above the anchor.
///
/// Worked out once, the first time anything asks. A program that
/// changes directory halfway through keeps importing from the project
/// it started in.
pub fn root() -> Option<PathBuf> {
  if let Some(anchor) = ANCHOR.get() {
    return ROOT.get_or_init(|| find_root(anchor)).clone();
  }

  // Nothing fixed an anchor, which happens when the runtime is embedded
  // rather than launched. The working directory is the only guide, and
  // it is read fresh because nothing promised it would stay put.
  std::env::current_dir().ok().and_then(|cwd| find_root(&cwd))
}

/// The directory a project's `.zuri` hangs off: the project root, or
/// the working directory when there is no project.
pub fn base_dir() -> Option<PathBuf> {
  root().or_else(|| std::env::current_dir().ok())
}

/// `<project>/.zuri/<name>`, resolved against `base_dir()`.
pub fn state_path(name: &str) -> Option<PathBuf> {
  base_dir().map(|base| base.join(STATE_DIR).join(name))
}

/// Where Zuri keeps what belongs to the user rather than to a project:
/// globally installed packages, the download cache, credentials.
/// `$ZURI_HOME`, or `.zuri` in the home directory.
pub fn zuri_home() -> Option<PathBuf> {
  if let Some(home) = std::env::var_os("ZURI_HOME").filter(|v| !v.is_empty()) {
    return Some(PathBuf::from(home));
  }

  home_dir().map(|home| home.join(STATE_DIR))
}

/// `$ZURI_HOME/libs`, where globally installed packages live. `None`
/// inside a bundle, which carries everything it imports.
pub fn global_libs() -> Option<PathBuf> {
  if BUNDLE.get().is_some() {
    return None;
  }

  zuri_home().map(|home| home.join(LIBS_DIR))
}

/// The project's `.zuri/libs`, unless it is the very same directory as
/// the global one. That happens when a script with no project runs from
/// the home directory, and the global packages must not be promoted
/// ahead of the standard library just because of where somebody stood.
pub fn project_libs() -> Option<PathBuf> {
  let libs = state_path(LIBS_DIR)?;

  match global_libs() {
    Some(global) if same_dir(&global, &libs) => None,
    _ => Some(libs),
  }
}

fn same_dir(a: &Path, b: &Path) -> bool {
  match (a.canonicalize(), b.canonicalize()) {
    (Ok(a), Ok(b)) => a == b,
    _ => a == b,
  }
}

/// The per-user cache directory Zuri keeps regenerable files under:
/// `%LOCALAPPDATA%\zuri` on Windows, `~/Library/Caches/zuri` on macOS,
/// and `$XDG_CACHE_HOME/zuri` (or `~/.cache/zuri`) elsewhere, falling
/// back to the system temporary directory when none of those can be
/// worked out.
pub fn user_cache_dir() -> PathBuf {
  let base = if cfg!(windows) {
    std::env::var_os("LOCALAPPDATA").map(PathBuf::from)
  } else if cfg!(target_vendor = "apple") {
    std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Library").join("Caches"))
  } else {
    std::env::var_os("XDG_CACHE_HOME")
      .map(PathBuf::from)
      .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache")))
  };

  base.unwrap_or_else(std::env::temp_dir).join("zuri")
}

#[cfg(not(windows))]
fn home_dir() -> Option<PathBuf> {
  std::env::var_os("HOME")
    .filter(|v| !v.is_empty())
    .map(PathBuf::from)
}

#[cfg(windows)]
fn home_dir() -> Option<PathBuf> {
  std::env::var_os("USERPROFILE")
    .filter(|v| !v.is_empty())
    .map(PathBuf::from)
}
