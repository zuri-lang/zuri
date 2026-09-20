//! Repository chores that are not a build.
//!
//! The one that exists so far is `sync`, which puts `libs/` and `cmds/`
//! beside an already-built binary. Editing a `.zu` file otherwise means
//! a full `cargo build` to see the change, and that re-runs the whole
//! build script and relinks a binary whose Rust never moved.

use std::env;
use std::path::{Path, PathBuf};
use std::process;

include!("sync.rs");

const PROFILES: [&str; 2] = ["debug", "release"];

fn main() {
  let args: Vec<String> = env::args().skip(1).collect();
  let words: Vec<&str> = args.iter().map(String::as_str).collect();

  match words.first().copied() {
    Some("sync") => sync(&words[1..]),
    Some("help") | Some("--help") | Some("-h") | None => usage(),
    Some(other) => {
      eprintln!("xtask: unknown task '{other}'");
      usage();
      process::exit(1);
    },
  }
}

fn usage() {
  println!("Usage: cargo sync [PROFILE] [--check]");
  println!();
  println!("Copies libs/ and cmds/ beside a built binary, so a change to");
  println!("either takes effect without a rebuild.");
  println!();
  println!("  PROFILE   debug or release; both by default, whichever exist");
  println!("  --check   report what is stale, change nothing, exit 1 if any");
}

fn sync(args: &[&str]) {
  let check = args.contains(&"--check");
  let root = repo_root();

  let wanted: Vec<&str> = args
    .iter()
    .copied()
    .filter(|word| !word.starts_with('-'))
    .collect();

  for word in &wanted {
    if !PROFILES.contains(word) {
      eprintln!("cargo sync: '{word}' is not a profile; expected debug or release");
      process::exit(1);
    }
  }

  // A profile nobody has built yet has no binary to sit beside, so it
  // is skipped rather than conjured up.
  let targets: Vec<PathBuf> = PROFILES
    .iter()
    .filter(|profile| wanted.is_empty() || wanted.contains(profile))
    .flat_map(|profile| profile_dirs(&root, profile))
    .collect();

  if targets.is_empty() {
    println!("cargo sync: nothing built yet, so nothing to sync");

    return;
  }

  let mut stale = 0;

  for target in &targets {
    match check {
      true => stale += report_stale(&root, target),
      false => copy_into(&root, target),
    }
  }

  if check && stale > 0 {
    process::exit(1);
  }
}

/// Every built profile directory of that name: `target/<profile>`, and
/// the `target/<triple>/<profile>` a cross-compiled build writes.
fn profile_dirs(root: &Path, profile: &str) -> Vec<PathBuf> {
  let target = match env::var_os("CARGO_TARGET_DIR") {
    Some(dir) => PathBuf::from(dir),
    None => root.join("target"),
  };

  let mut found = Vec::new();
  let plain = target.join(profile);

  if plain.is_dir() {
    found.push(plain);
  }

  if let Ok(entries) = std::fs::read_dir(&target) {
    for entry in entries.flatten() {
      if !entry.path().is_dir() || PROFILES.contains(&entry.file_name().to_string_lossy().as_ref())
      {
        continue;
      }

      let nested = entry.path().join(profile);

      if nested.is_dir() {
        found.push(nested);
      }
    }
  }

  found
}

fn copy_into(root: &Path, target: &Path) {
  match sync_payload(root, target) {
    Ok(counts) => {
      let summary = counts
        .iter()
        .map(|(name, files)| format!("{name} ({files})"))
        .collect::<Vec<_>>()
        .join(", ");

      println!("synced {summary} into {}", display(root, target));
    },
    Err(e) => {
      eprintln!("cargo sync: could not sync into {}: {e}", display(root, target));
      process::exit(1);
    },
  }
}

fn report_stale(root: &Path, target: &Path) -> usize {
  let mut stale = Vec::new();

  for name in PAYLOAD {
    let from = root.join(name);

    if !from.exists() {
      continue;
    }

    if let Err(e) = stale_paths(&from, &target.join(name), Path::new(name), &mut stale) {
      eprintln!("cargo sync: could not read {}: {e}", from.display());
      process::exit(1);
    }
  }

  let where_ = display(root, target);

  if stale.is_empty() {
    println!("{where_} is up to date");

    return 0;
  }

  println!("{where_} is stale in {} places:", stale.len());

  for path in &stale {
    println!("  {}", path.display());
  }

  stale.len()
}

/// A target directory written relative to the repository when it sits
/// inside it, which is the shape a reader recognises.
fn display(root: &Path, target: &Path) -> String {
  target
    .strip_prefix(root)
    .unwrap_or(target)
    .display()
    .to_string()
}

fn repo_root() -> PathBuf {
  // The xtask lives one directory down from the repository it serves.
  Path::new(env!("CARGO_MANIFEST_DIR"))
    .parent()
    .expect("the xtask package has no parent directory")
    .to_path_buf()
}
