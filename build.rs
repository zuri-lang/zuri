// build.rs
use chrono::Utc;
use copy_to_output::copy_to_output;
use std::env;
use std::fs;
use std::path::Path;

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
  copy_to_output("libs", &env::var("PROFILE").unwrap()).expect("Could not copy libs");
  println!("cargo:rerun-if-changed={}", libs_dir.display());

  copy_to_output("LICENSE", &env::var("PROFILE").unwrap()).expect("Could not copy license file");
  println!("cargo:rerun-if-changed=LICENSE");

  generate_zu_conformance_tests(&manifest_dir);
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

  let mut zu_files: Vec<_> = fs::read_dir(&tests_dir)
    .expect("tests/ directory should exist")
    .filter_map(|entry| entry.ok())
    .map(|entry| entry.path())
    .filter(|p| p.extension().is_some_and(|ext| ext == "zu"))
    .filter(|p| p.with_extension("out").exists())
    .collect();
  zu_files.sort();

  let mut seen_names = std::collections::HashSet::new();
  let mut code = String::new();
  for zu_path in &zu_files {
    let stem = zu_path
      .file_stem()
      .and_then(|s| s.to_str())
      .unwrap_or_else(|| panic!("non-UTF8 fixture name: {}", zu_path.display()));
    let mut test_name = escape_ident(stem);
    // Two fixture stems could collide after sanitization (e.g. names
    // differing only by a character `sanitize_ident` folds away). So we
    // disambiguate rather than silently generating two functions with
    // the same name (a hard compile error) or silently dropping one.
    while !seen_names.insert(test_name.clone()) {
      test_name.push('_');
    }
    code.push_str(&format!(
      "#[test]\nfn {test_name}() {{ run_fixture({:?}); }}\n\n",
      zu_path.display().to_string()
    ));
  }

  fs::write(&dest, code).expect("failed to write generated test file");

  // Structural changes (a fixture added or removed) need the test list
  // regenerated; content changes to an existing .zu/.out don't, since
  // `run_fixture` reads both fresh at test-run time, not build time.
  println!("cargo:rerun-if-changed={}", tests_dir.display());
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
