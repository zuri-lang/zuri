//! Builds `vendor/crates` from the patches in `vendor/patches`, and turns
//! changes made there back into patches.
//!
//! Each patch is named for the crate and version it applies to, such as
//! `cranelift-codegen-0.136.1.patch`. The crate is taken untouched from
//! crates.io through Cargo's own registry cache, copied to
//! `vendor/crates/<name>`, and patched there. The workspace's
//! `[patch.crates-io]` points at that copy.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{self, Command};

include!("stamp.rs");

type Result<T> = std::result::Result<T, String>;

/// A patch in `vendor/patches`, and the crate it applies to.
struct Patch {
  name: String,
  version: String,
  file: PathBuf,
}

fn main() {
  let args: Vec<String> = env::args().skip(1).collect();
  let words: Vec<&str> = args.iter().map(String::as_str).collect();
  let vendor = vendor_dir();

  let result = match words.as_slice() {
    [] => apply_all(&vendor, false),
    ["--check"] => apply_all(&vendor, true),
    ["save", name] => save(&vendor, name),
    ["upgrade", name, version] => upgrade(&vendor, name, version),
    ["help" | "--help" | "-h"] => {
      usage();

      Ok(())
    },
    _ => {
      usage();
      process::exit(1);
    },
  };

  if let Err(e) = result {
    eprintln!("cargo patch-crates: {e}");
    process::exit(1);
  }
}

fn usage() {
  println!("Usage: cargo patch-crates [--check]");
  println!("       cargo patch-crates save CRATE");
  println!("       cargo patch-crates upgrade CRATE VERSION");
  println!();
  println!("Builds vendor/crates from vendor/patches: each crate is copied");
  println!("untouched from crates.io and its patch applied on top. A copy");
  println!("already built from its current patch is left alone.");
  println!();
  println!("  --check   report copies that are missing or stale, change nothing,");
  println!("            exit 1 if any");
  println!("  save      write the changes made to a crate's copy out as its patch");
  println!("  upgrade   start a crate's copy over at VERSION, with its current");
  println!("            patch applied as far as it goes");
}

/// Brings every copy up to date with its patch, or with `check`, only
/// reports the ones that are not.
fn apply_all(vendor: &Path, check: bool) -> Result<()> {
  let mut stale = Vec::new();

  for patch in patches(vendor)? {
    if !up_to_date(vendor, &patch)? {
      stale.push(patch);
    }
  }

  if stale.is_empty() {
    println!("vendor/crates is up to date");

    return Ok(());
  }

  if check {
    for patch in &stale {
      println!(
        "vendor/crates/{} is not built from {}",
        patch.name,
        display(&patch.file)
      );
    }

    process::exit(1);
  }

  for patch in &stale {
    apply(vendor, patch, false)?;
    println!("patched {} {}", patch.name, patch.version);
  }

  Ok(())
}

/// Writes the difference between a crate's copy and the untouched crate
/// to `vendor/patches/<name>-<version>.patch`, replacing the patch for
/// any other version of it.
fn save(vendor: &Path, name: &str) -> Result<()> {
  let copy = copy_dir(vendor, name);

  if !copy.is_dir() {
    return Err(format!("there is no vendor/crates/{name} to save"));
  }

  let rejects = find_rejects(&copy)?;

  if !rejects.is_empty() {
    return Err(format!(
      "vendor/crates/{name} still has hunks that did not apply; resolve and delete {}",
      rejects.join(", ")
    ));
  }

  let version = package_version(&copy)?;
  let pristine = pristine(vendor, name, &version)?;
  let crates = vendor.join("crates");
  let from = pristine
    .strip_prefix(&crates)
    .map_err(|_| "pristine copy outside vendor/crates")?;

  let output = Command::new("git")
    .current_dir(&crates)
    .args([
      "diff",
      "--no-index",
      "--no-color",
      "--no-ext-diff",
      "--no-textconv",
      "--no-renames",
      "--src-prefix=a/",
      "--dst-prefix=b/",
    ])
    .arg(git_path(from))
    .arg(name)
    .output()
    .map_err(|e| format!("could not run git: {e}"))?;

  // Comparing two directories, git exits 1 when they differ, which is
  // the case being asked for.
  match output.status.code() {
    Some(0) => {
      return Err(format!(
        "vendor/crates/{name} is untouched {name} {version}; nothing to save"
      ));
    },
    Some(1) => {},
    _ => {
      return Err(format!(
        "git diff failed: {}",
        String::from_utf8_lossy(&output.stderr).trim()
      ));
    },
  }

  let diff =
    String::from_utf8(output.stdout).map_err(|_| "git diff wrote something that is not UTF-8")?;
  let from = from.to_string_lossy().replace('\\', "/");
  let body = relative_headers(&diff, &[&from, name]);
  let file = vendor
    .join("patches")
    .join(format!("{name}-{version}.patch"));

  fs::create_dir_all(vendor.join("patches"))
    .map_err(|e| format!("could not create vendor/patches: {e}"))?;
  fs::write(&file, body).map_err(|e| format!("could not write {}: {e}", display(&file)))?;

  for old in patches(vendor)? {
    if old.name == name && old.file != file {
      fs::remove_file(&old.file)
        .map_err(|e| format!("could not remove {}: {e}", display(&old.file)))?;
      println!("removed {}", display(&old.file));
    }
  }

  write_stamp(vendor, name, &file)?;
  println!("saved {}", display(&file));

  Ok(())
}

/// Replaces a crate's copy with the untouched crate at `version` and
/// applies its current patch as far as it goes. Hunks that no longer fit
/// are left beside their files as `.rej`, for `save` once they are
/// resolved.
fn upgrade(vendor: &Path, name: &str, version: &str) -> Result<()> {
  let current = patches(vendor)?
    .into_iter()
    .find(|patch| patch.name == name)
    .ok_or_else(|| format!("vendor/patches holds no patch for {name}"))?;

  let target = Patch {
    name: name.to_string(),
    version: version.to_string(),
    file: current.file.clone(),
  };

  let clean = apply(vendor, &target, true)?;
  let rejects = find_rejects(&copy_dir(vendor, name))?;

  if !clean && rejects.is_empty() {
    return Err(format!(
      "git could not apply {} at all; run it by hand in vendor/crates/{name} to see why",
      display(&current.file)
    ));
  }

  println!(
    "vendor/crates/{name} is now {name} {version} with {}",
    display(&current.file)
  );

  if clean && rejects.is_empty() {
    println!("every hunk applied; run `cargo patch-crates save {name}` to keep it");
  } else {
    println!("these hunks did not apply and need doing by hand:");

    for reject in &rejects {
      println!("  {reject}");
    }

    println!("delete each .rej once done, then run `cargo patch-crates save {name}`");
  }

  println!("and set {name} to {version} in Cargo.toml");

  Ok(())
}

/// Makes a fresh copy of the crate at the patch's version and applies
/// the patch to it. With `partial`, hunks that fail are written out as
/// `.rej` rather than failing the whole patch, no stamp is written, and
/// the result says whether everything applied.
fn apply(vendor: &Path, patch: &Patch, partial: bool) -> Result<bool> {
  let pristine = pristine(vendor, &patch.name, &patch.version)?;
  let copy = copy_dir(vendor, &patch.name);
  let stamp = stamp_path(vendor, &patch.name);

  // The stamp goes first, so a run cut short leaves the copy stale
  // rather than looking finished.
  if stamp.exists() {
    fs::remove_file(&stamp).map_err(|e| format!("could not remove {}: {e}", display(&stamp)))?;
  }

  if copy.exists() {
    fs::remove_dir_all(&copy).map_err(|e| format!("could not clear {}: {e}", display(&copy)))?;
  }

  copy_tree(&pristine, &copy).map_err(|e| format!("could not copy {}: {e}", display(&pristine)))?;

  let file = std::path::absolute(&patch.file)
    .map_err(|e| format!("could not read {}: {e}", display(&patch.file)))?;
  let mut git = Command::new("git");

  // Inside a repository, `git apply` reads the patch's paths from the
  // repository's top and quietly skips any outside the directory it runs
  // in, which is all of them here. Stopping the search for a repository
  // at `vendor/crates` makes it apply to the copy as plain `patch` would.
  git
    .current_dir(&copy)
    .env("GIT_CEILING_DIRECTORIES", git_path(&vendor.join("crates")))
    .args(["apply", "-p1", "--whitespace=nowarn"]);

  if partial {
    git.arg("--reject");
  }

  let output = git
    .arg(git_path(&file))
    .output()
    .map_err(|e| format!("could not run git: {e}"))?;

  if partial {
    return Ok(output.status.success());
  }

  if !output.status.success() {
    return Err(format!(
      "{} does not apply to {} {}:\n{}",
      display(&patch.file),
      patch.name,
      patch.version,
      String::from_utf8_lossy(&output.stderr).trim()
    ));
  }

  write_stamp(vendor, &patch.name, &patch.file)?;

  Ok(true)
}

/// Whether the crate's copy exists and was built from this patch.
fn up_to_date(vendor: &Path, patch: &Patch) -> Result<bool> {
  if !copy_dir(vendor, &patch.name).join("Cargo.toml").is_file() {
    return Ok(false);
  }

  let want =
    stamp_text(&patch.file).map_err(|e| format!("could not read {}: {e}", display(&patch.file)))?;
  let have = fs::read_to_string(stamp_path(vendor, &patch.name)).unwrap_or_default();

  Ok(have == want)
}

fn write_stamp(vendor: &Path, name: &str, patch: &Path) -> Result<()> {
  let stamp = stamp_path(vendor, name);
  let text = stamp_text(patch).map_err(|e| format!("could not read {}: {e}", display(patch)))?;

  fs::write(&stamp, text).map_err(|e| format!("could not write {}: {e}", display(&stamp)))
}

/// Every patch in `vendor/patches`, by crate name.
fn patches(vendor: &Path) -> Result<Vec<Patch>> {
  let dir = vendor.join("patches");
  let entries = match fs::read_dir(&dir) {
    Ok(entries) => entries,
    Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
    Err(e) => return Err(format!("could not read {}: {e}", display(&dir))),
  };

  let mut found = Vec::new();

  for entry in entries.flatten() {
    let file = entry.path();
    let Some(stem) = file
      .file_name()
      .and_then(|n| n.to_str())
      .and_then(|n| n.strip_suffix(".patch"))
    else {
      continue;
    };

    let (name, version) = split_patch_name(stem)
      .ok_or_else(|| format!("{} is not named <crate>-<version>.patch", display(&file)))?;

    found.push(Patch {
      name: name.to_string(),
      version: version.to_string(),
      file: file.clone(),
    });
  }

  found.sort_by(|a, b| a.name.cmp(&b.name));

  Ok(found)
}

/// The untouched crate, kept in `vendor/crates/.pristine` so `save` can
/// diff against it without going back to the registry.
fn pristine(vendor: &Path, name: &str, version: &str) -> Result<PathBuf> {
  let dir = vendor
    .join("crates")
    .join(".pristine")
    .join(format!("{name}-{version}"));

  if dir.join("Cargo.toml").is_file() {
    return Ok(dir);
  }

  let source = registry_source(vendor, name, version)?;
  let partial = dir.with_extension("partial");

  if partial.exists() {
    fs::remove_dir_all(&partial)
      .map_err(|e| format!("could not clear {}: {e}", display(&partial)))?;
  }

  copy_tree(&source, &partial).map_err(|e| format!("could not copy {}: {e}", source.display()))?;
  fs::rename(&partial, &dir)
    .map_err(|e| format!("could not move {} into place: {e}", display(&dir)))?;

  Ok(dir)
}

/// Where Cargo unpacks the crate in its registry cache, downloading it
/// first if need be. A throwaway package that depends on exactly that
/// version is asked for its metadata, which makes Cargo fetch, verify
/// and unpack it the same way a build would.
fn registry_source(vendor: &Path, name: &str, version: &str) -> Result<PathBuf> {
  let fetch = vendor.join("crates").join(".fetch");
  let manifest = fetch.join("Cargo.toml");

  fs::create_dir_all(fetch.join("src"))
    .map_err(|e| format!("could not create {}: {e}", display(&fetch)))?;
  fs::write(fetch.join("src").join("lib.rs"), "")
    .map_err(|e| format!("could not write {}: {e}", display(&fetch)))?;
  fs::write(
    &manifest,
    format!(
      "[package]\nname = \"fetch\"\nversion = \"0.0.0\"\nedition = \"2024\"\npublish = false\n\n\
       [dependencies]\n{name} = \"={version}\"\n\n[workspace]\n"
    ),
  )
  .map_err(|e| format!("could not write {}: {e}", display(&manifest)))?;

  let cargo = env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
  let output = Command::new(cargo)
    .args(["metadata", "--format-version", "1", "--manifest-path"])
    .arg(&manifest)
    .output()
    .map_err(|e| format!("could not run cargo: {e}"))?;

  if !output.status.success() {
    return Err(format!(
      "cargo could not fetch {name} {version}:\n{}",
      String::from_utf8_lossy(&output.stderr).trim()
    ));
  }

  let json = String::from_utf8_lossy(&output.stdout);
  let wanted = format!("{name}-{version}");

  json_strings_after(&json, "\"manifest_path\":")
    .into_iter()
    .map(PathBuf::from)
    .find(|path| {
      path
        .parent()
        .and_then(Path::file_name)
        .is_some_and(|dir| dir == wanted.as_str())
    })
    .and_then(|path| path.parent().map(Path::to_path_buf))
    .ok_or_else(|| format!("cargo fetched {name} {version} but did not say where it put it"))
}

/// The string values that follow every occurrence of `key` in a JSON
/// document. Enough JSON for reading `cargo metadata`'s paths, and no
/// more.
fn json_strings_after(json: &str, key: &str) -> Vec<String> {
  let mut out = Vec::new();

  for (at, _) in json.match_indices(key) {
    let rest = json[at + key.len()..].trim_start();
    let Some(rest) = rest.strip_prefix('"') else {
      continue;
    };

    let mut value = String::new();
    let mut chars = rest.chars();

    while let Some(c) = chars.next() {
      match c {
        '"' => break,
        '\\' => match chars.next() {
          Some('n') => value.push('\n'),
          Some('t') => value.push('\t'),
          Some(other) => value.push(other),
          None => break,
        },
        c => value.push(c),
      }
    }

    out.push(value);
  }

  out
}

/// Comparing two directories, git names each file by the path it was given, so
/// the pristine side reads `a/.pristine/<crate>-<version>/src/lib.rs`.
/// This rewrites the file headers to `a/src/lib.rs`, the form that
/// applies from inside the crate.
fn relative_headers(diff: &str, roots: &[&str]) -> String {
  let mut out = String::with_capacity(diff.len());

  for line in diff.split_inclusive('\n') {
    if !(line.starts_with("diff --git ") || line.starts_with("--- ") || line.starts_with("+++ ")) {
      out.push_str(line);
      continue;
    }

    let mut line = line.to_string();

    for root in roots {
      line = line
        .replace(&format!("a/{root}/"), "a/")
        .replace(&format!("b/{root}/"), "b/");
    }

    out.push_str(&line);
  }

  out
}

/// The `.rej` files git left in a copy for hunks it could not apply,
/// relative to the copy.
fn find_rejects(copy: &Path) -> Result<Vec<String>> {
  let mut found = Vec::new();
  let mut pending = vec![copy.to_path_buf()];

  while let Some(dir) = pending.pop() {
    let entries =
      fs::read_dir(&dir).map_err(|e| format!("could not read {}: {e}", display(&dir)))?;

    for entry in entries.flatten() {
      let path = entry.path();

      if path.is_dir() {
        pending.push(path);
      } else if path.extension().is_some_and(|ext| ext == "rej") {
        let relative = path.strip_prefix(copy).unwrap_or(&path);
        found.push(relative.to_string_lossy().replace('\\', "/"));
      }
    }
  }

  found.sort();

  Ok(found)
}

/// The `version` under `[package]` in a crate's manifest.
fn package_version(copy: &Path) -> Result<String> {
  let manifest = copy.join("Cargo.toml");
  let text = fs::read_to_string(&manifest)
    .map_err(|e| format!("could not read {}: {e}", display(&manifest)))?;
  let mut in_package = false;

  for line in text.lines().map(str::trim) {
    if line.starts_with('[') {
      in_package = line == "[package]";
    } else if in_package
      && let Some(value) = line.strip_prefix("version").map(str::trim_start)
      && let Some(value) = value.strip_prefix('=')
    {
      return Ok(value.trim().trim_matches('"').to_string());
    }
  }

  Err(format!("{} has no package version", display(&manifest)))
}

fn copy_tree(from: &Path, to: &Path) -> std::io::Result<()> {
  fs::create_dir_all(to)?;

  for entry in fs::read_dir(from)? {
    let entry = entry?;
    let target = to.join(entry.file_name());

    match entry.file_type()?.is_dir() {
      true => copy_tree(&entry.path(), &target)?,
      false => {
        fs::copy(entry.path(), &target)?;
      },
    }
  }

  Ok(())
}

fn copy_dir(vendor: &Path, name: &str) -> PathBuf {
  vendor.join("crates").join(name)
}

/// A path the way git takes it. Git for Windows reads forward slashes
/// everywhere, and cannot open a verbatim `\\?\` path at all, so on
/// Windows a path reaches it in plain forward-slash form.
fn git_path(path: &Path) -> String {
  let text = path.to_string_lossy();
  if cfg!(windows) {
    text.replace('\\', "/")
  } else {
    text.into_owned()
  }
}

fn vendor_dir() -> PathBuf {
  // The tool lives one directory down from the `vendor` it serves. Cargo
  // names that directory again when it runs the tool, and that one wins
  // over the directory the binary was built from: Cargo hashes a path
  // package without its location, so checkouts sharing a target directory
  // share this binary too.
  let manifest_dir = env::var_os("CARGO_MANIFEST_DIR")
    .map(PathBuf::from)
    .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")));

  manifest_dir
    .parent()
    .expect("the patch-crates package has no parent directory")
    .to_path_buf()
}

/// A path written relative to the repository, the shape a reader
/// recognises.
fn display(path: &Path) -> String {
  let root = vendor_dir()
    .parent()
    .map(Path::to_path_buf)
    .unwrap_or_default();

  path
    .strip_prefix(&root)
    .unwrap_or(path)
    .to_string_lossy()
    .replace('\\', "/")
}
