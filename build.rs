// build.rs
use chrono::Utc;
use copy_to_output::copy_to_output;
use std::env;
use std::fs;
use std::path::Path;
use std::process::{Command, Stdio};

fn main() {
  let now = Utc::now().format("%Y-%m-%d %H:%M:%S UTC").to_string();

  let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
  let toml_path = format!("{}/Cargo.toml", manifest_dir);
  let toml_content = fs::read_to_string(&toml_path).expect("Failed to read Cargo.toml");

  let parsed_toml: toml::Value = toml::from_str(&toml_content).expect("Failed to parse Cargo.toml");

  let zuri_version = parsed_toml
    .get("package")
    .and_then(|p| p.get("metadata"))
    .and_then(|m| m.get("zuri"))
    .and_then(|c| c.get("version"))
    .and_then(|v| v.as_str())
    .expect("Missing package.metadata.zuri.version");

  let vm_version = parsed_toml
    .get("package")
    .and_then(|p| p.get("metadata"))
    .and_then(|m| m.get("zuri"))
    .and_then(|c| c.get("vm_version"))
    .and_then(|v| v.as_str())
    .expect("Missing package.metadata.zuri.vm_version");

  let history_size = parsed_toml
    .get("package")
    .and_then(|p| p.get("metadata"))
    .and_then(|m| m.get("zuri"))
    .and_then(|c| c.get("history_size"))
    .and_then(|v| v.as_integer())
    .unwrap_or(1000);

  println!("cargo:rustc-env=ZURI_VERSION={}", zuri_version);
  println!("cargo:rustc-env=ZVM_VERSION={}", vm_version);
  println!("cargo:rustc-env=ZURI_BUILD_TIME={}", now);
  println!("cargo:rustc-env=ZURI_HISTORY_SIZE={}", history_size);

  // Tell Cargo to re-run this script if Cargo.toml changes
  println!("cargo:rerun-if-changed=Cargo.toml");

  let libs_dir = Path::new(&manifest_dir).join("libs");
  // Cleared first, because `copy_to_output` only ever writes files; it
  // never removes one that has since been deleted from `libs/`. Without
  // this, splitting `libs/log.zu` into `libs/log/` leaves the old flat
  // file sitting in the output beside the new directory, and module
  // resolution finds the stale copy. That is a silent and very
  // confusing failure, because `libs/` on disk looks entirely correct
  // and only the build output disagrees.
  let profile = env::var("PROFILE").unwrap();
  let out_libs = copy_output_dir(&profile).join("libs");
  if out_libs.exists() {
    fs::remove_dir_all(&out_libs).expect("Could not clear the copied libs directory");
  }
  copy_to_output("libs", &profile).expect("Could not copy libs");
  println!("cargo:rerun-if-changed={}", libs_dir.display());

  copy_to_output("LICENSE", &env::var("PROFILE").unwrap()).expect("Could not copy license file");
  println!("cargo:rerun-if-changed=LICENSE");

  generate_zu_conformance_tests(&manifest_dir);
  build_docs(&manifest_dir);
}

/// Renders both books whenever mdBook is installed and either has
/// actually changed.
fn build_docs(manifest_dir: &str) {
  for (name, stamp, generated) in [
    ("book", "book.stamp", false),
    ("reference", "reference.stamp", true),
  ] {
    build_book(manifest_dir, name, stamp, generated);
  }
}

/// Renders one book in `docs/` whenever mdBook is installed and that
/// book has actually changed.
///
/// Two things keep this off the critical path of ordinary work. The
/// `newest_mtime` check below means a day spent in `src/` never spends
/// a second rendering markdown, and a machine without mdBook gets one
/// warning rather than a failed build, because the book reads perfectly
/// well as the markdown it already is.
///
/// The stamp file is needed because this script re-runs on every single
/// build regardless: `generate_zu_conformance_tests` writes into
/// `tests/generated` while `tests` is itself a rerun trigger, so Cargo
/// always sees the directory as dirty. Leaning on `rerun-if-changed`
/// alone would rebuild the book every time.
///
/// Output goes to `target/<name>` (set in each `book.toml`), deliberately not
/// under `docs/`; writing into a directory this script also watches
/// would have every build trigger the next one.
fn build_book(manifest_dir: &str, name: &str, stamp_name: &str, generated: bool) {
  let book_dir = Path::new(manifest_dir).join("docs").join(name);
  let manifest = book_dir.join("book.toml");

  // The book keeps its pages beside `book.toml`; the reference's are
  // generated under `target/`, which is where its `src` setting points.
  let src_dir = match generated {
    true => Path::new(manifest_dir)
      .join("target")
      .join(name)
      .join("src"),
    false => book_dir.join("src"),
  };

  println!("cargo:rerun-if-changed={}", src_dir.display());
  println!("cargo:rerun-if-changed={}", manifest.display());

  if !manifest.is_file() {
    return;
  }

  // Both books declare `theme/zuri-highlight.js` in their `book.toml`;
  // the file itself has one source in `docs/tools`. Copying it here as
  // well as in the docs tool keeps a plain `cargo build` able to render
  // a book that someone has never run `cargo build-docs` on.
  let highlighter = Path::new(manifest_dir)
    .join("docs")
    .join("tools")
    .join("zuri-highlight.js");
  let theme = book_dir.join("theme");

  if highlighter.is_file() && fs::create_dir_all(&theme).is_ok() {
    let _ = fs::copy(&highlighter, theme.join("zuri-highlight.js"));
  }

  println!("cargo:rerun-if-changed={}", highlighter.display());

  // The reference's pages are generated rather than committed, so a
  // fresh clone has none until `cargo build-docs` writes them. Running
  // the generator from here is not an option: it needs the `zuri`
  // binary, which this script runs before linking.
  if !src_dir.join("SUMMARY.md").is_file() {
    println!("cargo:warning=the {name} has no pages yet; run `cargo build-docs` to generate them");
    return;
  }

  let newest = newest_mtime(&src_dir).max(file_mtime(&manifest));
  let stamp = Path::new(&env::var("OUT_DIR").unwrap()).join(stamp_name);
  let rendered = match generated {
    true => Path::new(manifest_dir)
      .join("target")
      .join(name)
      .join("html"),
    false => Path::new(manifest_dir).join("target").join(name),
  };

  if rendered.is_dir() && file_mtime(&stamp) >= newest {
    return;
  }

  if !mdbook_available() {
    println!(
      "cargo:warning=mdbook not found, skipping the {name}. \
       Install it with `cargo install mdbook`, or read docs/{name}/src directly."
    );
    return;
  }

  let status = Command::new("mdbook").arg("build").arg(&book_dir).status();

  match status {
    Ok(status) if status.success() => {
      // Only stamped on success, so a build that failed for a fixable
      // reason (a bad `book.toml`, a missing chapter) is retried next
      // time instead of being remembered as done.
      let _ = fs::write(&stamp, "");
    },
    Ok(status) => println!("cargo:warning=mdbook build exited with {status}"),
    Err(e) => println!("cargo:warning=could not run mdbook: {e}"),
  }
}

/// The most recently modified file anywhere under `dir`, as seconds
/// since the epoch. Zero for a directory that does not exist, which
/// makes a missing book look older than any stamp and skip cleanly.
fn newest_mtime(dir: &Path) -> u64 {
  let Ok(entries) = fs::read_dir(dir) else {
    return 0;
  };

  entries
    .flatten()
    .map(|entry| {
      let path = entry.path();
      if path.is_dir() {
        newest_mtime(&path)
      } else {
        file_mtime(&path)
      }
    })
    .max()
    .unwrap_or(0)
}

fn file_mtime(path: &Path) -> u64 {
  fs::metadata(path)
    .and_then(|m| m.modified())
    .ok()
    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
    .map(|d| d.as_secs())
    .unwrap_or(0)
}

fn mdbook_available() -> bool {
  Command::new("mdbook")
    .arg("--version")
    .stdout(Stdio::null())
    .stderr(Stdio::null())
    .status()
    .map(|status| status.success())
    .unwrap_or(false)
}

/// Where `copy_to_output` puts things: the profile directory that holds
/// the built binaries, which is what `libs/` is copied beside.
///
/// Derived by walking up from `OUT_DIR`
/// (`<profile>/build/<pkg>-<hash>/out`) rather than by rebuilding
/// `target/[{triple}/]{profile}` from parts, so a cross-compiled build's
/// extra target-triple component is handled without having to detect it.
/// Falls back to the relative path `copy_to_output` itself would use if
/// `OUT_DIR` is ever missing or shaped unexpectedly.
fn copy_output_dir(profile: &str) -> std::path::PathBuf {
  if let Some(out_dir) = env::var_os("OUT_DIR") {
    let path = Path::new(&out_dir);
    // out -> <pkg>-<hash> -> build -> <profile>
    if let Some(profile_dir) = path.ancestors().nth(3)
      && profile_dir.file_name() == Some(std::ffi::OsStr::new(profile))
    {
      return profile_dir.to_path_buf();
    }
  }

  Path::new("target").join(profile)
}

/// Generates one `#[test]` function per `tests/*.zu` fixture that has
/// a matching `tests/*.out` file, so `cargo test` reports each fixture
/// independently (which one failed, not just an aggregate count)
/// (see `tests/zu_conformance.rs`), which `include!`s the generated file
/// and provides the actual `run_fixture` implementation each generated
/// test calls.
fn generate_zu_conformance_tests(manifest_dir: &str) {
  let tests_dir = Path::new(manifest_dir).join("tests");
  // Written into a SUBDIRECTORY of `tests/`, not `tests/` itself and
  // not `OUT_DIR`: a `.rs` file directly under `tests/` would be
  // auto-discovered by Cargo as its OWN separate integration-test
  // binary (it's just a pile of `#[test] fn ...` calling `run_fixture`,
  // which only exists in `zu_conformance.rs`; compiled standalone it
  // wouldn't build), while `OUT_DIR` is per-build-script-process and
  // not visible to other targets' `env!()` without extra
  // `cargo:rustc-env` forwarding. A subdirectory is invisible to
  // Cargo's top-level test auto-discovery AND has a fixed, predictable
  // path computable from `CARGO_MANIFEST_DIR` alone, needing no
  // cross-target env var at all.
  let generated_dir = tests_dir.join("generated");
  fs::create_dir_all(&generated_dir).expect("failed to create tests/generated/");
  let dest = generated_dir.join("zu_conformance_generated.rs");

  let mut zu_files = Vec::new();
  collect_zu_files(&tests_dir, &generated_dir, &mut zu_files);
  zu_files.sort();

  let mut seen_names = std::collections::HashSet::new();
  let mut code = String::new();
  for zu_path in &zu_files {
    // Subdirectory fixtures (e.g. `tests/libs/isolate.zu`) get their
    // parent folder folded into the test name, so `cargo test`'s
    // output tells you which suite a failure came from at a glance
    // instead of every subdirectory's fixtures being indistinguishable
    // underscore-suffixed siblings of the top-level ones.
    let rel = zu_path
      .strip_prefix(&tests_dir)
      .unwrap_or(zu_path)
      .with_extension("");
    let name_source = rel
      .components()
      .map(|c| c.as_os_str().to_str().unwrap_or("_"))
      .collect::<Vec<_>>()
      .join("_");
    let mut test_name = escape_ident(&name_source);
    // Two fixture stems could collide after sanitization (e.g. names
    // differing only by a character `sanitize_ident` folds away). So we
    // disambiguate rather than silently generating two functions with
    // the same name (a hard compile error) or silently dropping one.
    while !seen_names.insert(test_name.clone()) {
      test_name.push('_');
    }
    // A top-level fixture is checked by exact output match; one that
    // lives in a subdirectory owns its own pass/fail logic (see
    // `run_exit_code_fixture`'s own docs) and is checked by exit code
    // alone.
    let runner = if rel.components().count() > 1 {
      "run_exit_code_fixture"
    } else {
      "run_fixture"
    };
    code.push_str(&format!(
      "{}#[test]\nfn {test_name}() {{ {runner}({:?}); }}\n\n",
      platform_gate(zu_path),
      zu_path.display().to_string()
    ));
  }

  fs::write(&dest, code).expect("failed to write generated test file");

  // Cargo walks a watched directory, so this covers both a fixture
  // being added or removed and an existing one having its `@platform`
  // line changed. Nothing else about a fixture's contents matters
  // here: `run_fixture` reads the `.zu` and `.out` fresh at test-run
  // time, not build time.
  println!("cargo:rerun-if-changed={}", tests_dir.display());
}

/// The `#[cfg]` attribute, if any, that a fixture's `@platform` line
/// asks for.
///
/// A fixture restricts itself with a `# @platform: unix` line in its
/// header, and that becomes a `#[cfg(unix)]` on its generated test.
/// This is for a fixture whose subject genuinely has no counterpart on
/// the other platform, not for one that merely fails there: signal
/// delivery to a pid, for instance, is a Unix concept with no Windows
/// equivalent to test against, so the fixture is skipped rather than
/// rewritten into something that no longer covers what it was written
/// for.
///
/// Only the header is searched, so the marker cannot be matched by
/// accident inside a string literal further down.
fn platform_gate(path: &Path) -> String {
  const HEADER_LINES: usize = 80;

  let Ok(source) = fs::read_to_string(path) else {
    return String::new();
  };

  for line in source.lines().take(HEADER_LINES) {
    let Some(platform) = line.trim().strip_prefix("# @platform:") else {
      continue;
    };
    return match platform.trim() {
      "unix" => "#[cfg(unix)]\n".to_string(),
      "windows" => "#[cfg(windows)]\n".to_string(),
      other => panic!(
        "{}: unknown @platform '{other}'; expected 'unix' or 'windows'",
        path.display()
      ),
    };
  }

  String::new()
}

/// Walks `dir` looking for `.zu` fixtures with a matching `.out`,
/// descending into subdirectories (`tests/libs/`, and whatever else
/// shows up later) so a fixture doesn't have to live directly under
/// `tests/` to be picked up. Skips `skip_dir` (`tests/generated/`,
/// this script's own output) so it never tries to read back its own
/// generated Rust as a candidate `.zu` fixture.
fn collect_zu_files(dir: &Path, skip_dir: &Path, out: &mut Vec<std::path::PathBuf>) {
  let entries =
    fs::read_dir(dir).unwrap_or_else(|e| panic!("failed to read directory {}: {e}", dir.display()));
  for entry in entries.filter_map(|entry| entry.ok()) {
    let path = entry.path();
    if path == skip_dir {
      continue;
    }
    if path.is_dir() {
      collect_zu_files(&path, skip_dir, out);
    } else if path.extension().is_some_and(|ext| ext == "zu") && path.with_extension("out").exists()
    {
      out.push(path);
    }
  }
}

fn sanitize_ident(s: &str) -> String {
  let mut out = String::new();
  for c in s.chars() {
    if c.is_alphanumeric() {
      out.push(c.to_ascii_lowercase());
    } else {
      out.push('_');
    }
  }
  if out.chars().next().is_none_or(|c| c.is_ascii_digit()) {
    out.insert(0, '_');
  }
  out
}

/// Sanitizes, then unconditionally wraps in `r#`; a raw identifier is
/// a pure lexer-level escape with no semantic difference from the
/// plain identifier (`r#foo` and `foo` name the same thing) whether or
/// not the text happens to be a keyword. We handle the one error the
/// language itself carves out; `self`/`Self`/`super`/`crate` can
/// never be raw identifiers, `r#` or not; is narrow enough to just
/// check for directly.
fn escape_ident(s: &str) -> String {
  let sanitized = sanitize_ident(s);
  // `sanitize_ident` already lowercases, so `Self` and `self` collapse
  // to the same string here; only three literal cases to check.
  if matches!(sanitized.as_str(), "self" | "super" | "crate") {
    format!("{sanitized}_")
  } else {
    format!("r#{sanitized}")
  }
}
