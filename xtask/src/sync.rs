// The payload the runtime looks for beside its own executable, and the
// copy that puts it there.
//
// `build.rs` includes this file so that a build and `cargo sync` lay
// the same bytes out the same way. Nothing here names a `use`, because
// an included file shares the scope it lands in.

/// What gets copied next to the binary, in the order it is reported.
pub const PAYLOAD: [&str; 3] = ["libs", "cmds", "LICENSE"];

/// Copies every entry of `PAYLOAD` from `root` into `dest`, and returns
/// how many files each one took.
///
/// A directory is mirrored rather than merged: whatever is already
/// there is cleared first. Merging leaves a file that has since been
/// deleted from the source sitting in the output, and module resolution
/// then finds the stale copy — a silent failure, since the source tree
/// on disk looks entirely correct and only the output disagrees.
pub fn sync_payload(
  root: &std::path::Path,
  dest: &std::path::Path,
) -> std::io::Result<Vec<(&'static str, usize)>> {
  let mut counts = Vec::new();

  for name in PAYLOAD {
    let from = root.join(name);

    if !from.exists() {
      continue;
    }

    let to = dest.join(name);

    let copied = match from.is_dir() {
      true => mirror_dir(&from, &to)?,
      false => {
        copy_file(&from, &to)?;

        1
      },
    };

    counts.push((name, copied));
  }

  Ok(counts)
}

/// Replaces `to` with a copy of `from`, and returns how many files it
/// holds.
pub fn mirror_dir(from: &std::path::Path, to: &std::path::Path) -> std::io::Result<usize> {
  if to.exists() {
    std::fs::remove_dir_all(to)?;
  }

  copy_dir(from, to)
}

fn copy_dir(from: &std::path::Path, to: &std::path::Path) -> std::io::Result<usize> {
  std::fs::create_dir_all(to)?;

  let mut copied = 0;

  for entry in std::fs::read_dir(from)? {
    let entry = entry?;
    let source = entry.path();
    let target = to.join(entry.file_name());

    copied += match entry.file_type()?.is_dir() {
      true => copy_dir(&source, &target)?,
      false => {
        copy_file(&source, &target)?;

        1
      },
    };
  }

  Ok(copied)
}

fn copy_file(from: &std::path::Path, to: &std::path::Path) -> std::io::Result<()> {
  if let Some(parent) = to.parent() {
    std::fs::create_dir_all(parent)?;
  }

  std::fs::copy(from, to)?;

  Ok(())
}

/// Every path under `from` whose copy in `to` is missing or different,
/// written relative to `from`.
pub fn stale_paths(
  from: &std::path::Path,
  to: &std::path::Path,
  prefix: &std::path::Path,
  out: &mut Vec<std::path::PathBuf>,
) -> std::io::Result<()> {
  if !from.is_dir() {
    if !same_file(from, to) {
      out.push(prefix.to_path_buf());
    }

    return Ok(());
  }

  if !to.is_dir() {
    out.push(prefix.to_path_buf());

    return Ok(());
  }

  for entry in std::fs::read_dir(from)? {
    let entry = entry?;

    stale_paths(
      &entry.path(),
      &to.join(entry.file_name()),
      &prefix.join(entry.file_name()),
      out,
    )?;
  }

  // A file the source no longer has is as stale as one it changed: the
  // runtime would still load it.
  if to.is_dir() {
    for entry in std::fs::read_dir(to)? {
      let entry = entry?;

      if !from.join(entry.file_name()).exists() {
        out.push(prefix.join(entry.file_name()));
      }
    }
  }

  Ok(())
}

fn same_file(left: &std::path::Path, right: &std::path::Path) -> bool {
  match (std::fs::read(left), std::fs::read(right)) {
    (Ok(left), Ok(right)) => left == right,
    _ => false,
  }
}
