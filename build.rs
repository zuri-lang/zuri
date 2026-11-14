// build.rs
use chrono::Utc;
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

  // The test harness compiles the ffi suites' C fixture for the same
  // target the runtime is built for, and needs to know which that is.
  println!(
    "cargo:rustc-env=ZURI_BUILD_TARGET={}",
    env::var("TARGET").unwrap_or_default()
  );
  println!(
    "cargo:rustc-env=ZURI_BUILD_HOST={}",
    env::var("HOST").unwrap_or_default()
  );

  // Tell Cargo to re-run this script if Cargo.toml changes
  println!("cargo:rerun-if-changed=Cargo.toml");

  // The standard library, the commands the runtime ships, and the
  // licence it prints: everything it looks for beside its own
  // executable. `cargo sync` puts the same bytes in the same place
  // without a build, which is what `sync.rs` is included for.
  let profile = env::var("PROFILE").unwrap();
  let root = Path::new(&manifest_dir);

  sync_payload(root, &copy_output_dir(&profile)).expect("Could not copy the runtime payload");

  for name in PAYLOAD {
    println!("cargo:rerun-if-changed={}", root.join(name).display());
  }

  println!("cargo:rerun-if-changed=xtask/src/sync.rs");

  check_patched_crates(root);
  link_unwinder_statically();
  generate_zu_conformance_tests(&manifest_dir);
}

/// Links libgcc's unwinder into a dynamically linked musl build, so the
/// program needs nothing on the machine it runs on but musl itself.
///
/// `.cargo/config.toml` links musl dynamically, since a statically linked
/// musl program cannot load a shared library and `ffi` has to. The
/// standard library then asks for its unwinder as `-lgcc_s`, the shared
/// libgcc, which a bare Alpine does not have. A `libgcc_s.a` holding
/// libgcc's static unwinder, put first on the search path, answers that
/// request instead. It is libgcc's unwinder either way, which is the one
/// `jit::unwind` registers compiled code with on Linux. `-lgcc` goes last
/// because the unwinder calls helpers in it, such as ARM's atomics.
fn link_unwinder_statically() {
  let musl = env::var("CARGO_CFG_TARGET_ENV").is_ok_and(|env| env == "musl");
  let static_crt = env::var("CARGO_CFG_TARGET_FEATURE")
    .is_ok_and(|features| features.split(',').any(|feature| feature == "crt-static"));

  if !musl || static_crt {
    return;
  }

  let target = env::var("TARGET").unwrap_or_default();
  let names = [
    format!("CC_{}", target.replace('-', "_")),
    format!("CC_{target}"),
    "TARGET_CC".to_string(),
    "CC".to_string(),
  ];

  for name in &names {
    println!("cargo:rerun-if-env-changed={name}");
  }

  let compiler = names
    .iter()
    .find_map(|name| env::var(name).ok())
    .unwrap_or_else(|| "cc".to_string());
  let mut words = compiler.split_whitespace();
  let program = words.next().unwrap_or("cc");

  let found = Command::new(program)
    .args(words)
    .arg("-print-file-name=libgcc_eh.a")
    .stderr(Stdio::null())
    .output()
    .ok()
    .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
    .map(std::path::PathBuf::from)
    .filter(|path| path.is_absolute() && path.is_file());

  let Some(unwinder) = found else {
    println!(
      "cargo:warning={compiler} has no libgcc_eh.a, so this build needs libgcc's shared library where it runs"
    );
    return;
  };

  let dir = Path::new(&env::var("OUT_DIR").unwrap()).join("static-unwind");
  fs::create_dir_all(&dir).expect("Could not create the static unwinder's directory");
  fs::copy(&unwinder, dir.join("libgcc_s.a")).expect("Could not copy libgcc's static unwinder");

  println!("cargo:rustc-link-search=native={}", dir.display());
  println!("cargo:rustc-link-arg=-static-libgcc");
  println!("cargo:rustc-link-arg=-lgcc");
}

/// The profile directory that holds the built binaries, which is what
/// the runtime payload is copied beside.
///
/// Derived by walking up from `OUT_DIR` rather than by rebuilding
/// `target/[{triple}/]{profile}` from parts, so a cross-compiled build's
/// extra target-triple component is handled without having to detect it.
/// The directory is recognised by the `build` directory inside it, not
/// by `PROFILE`: Cargo sets that to `release` or `debug` for the profile
/// a custom one inherits from, while the directory carries the custom
/// profile's own name, as `production` does. Cargo 1.99 and earlier lay
/// `OUT_DIR` out as `<profile>/build/<pkg>-<hash>/out` and later ones as
/// `<profile>/build/<pkg>/<hash>/out`, so the nearest `build` within
/// three levels is taken. Falls back to the conventional relative path
/// if `OUT_DIR` is ever missing or shaped unexpectedly.
fn copy_output_dir(profile: &str) -> std::path::PathBuf {
  if let Some(out_dir) = env::var_os("OUT_DIR") {
    let build_dir = Path::new(&out_dir)
      .ancestors()
      .skip(1)
      .take(3)
      .find(|dir| dir.file_name() == Some(std::ffi::OsStr::new("build")));

    if let Some(profile_dir) = build_dir.and_then(Path::parent) {
      return profile_dir.to_path_buf();
    }
  }

  Path::new("target").join(profile)
}

/// Generates one `#[test]` function per Zuri fixture under `tests/` that
/// has a matching `.out` file, so `cargo test` names each fixture that
/// fails rather than reporting one aggregate count. The conformance
/// test target includes the generated file and provides the
/// `run_fixture` each generated test calls.
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
    // Fixtures in a subdirectory, such as the library suites under
    // `tests/libs`, get their parent folder folded into the test name, so `cargo test`'s
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

include!("xtask/src/sync.rs");

/// Refuses to build against a copy in `vendor/crates` that was not built
/// from the patch now in `vendor/patches`. Cargo compiles a stale copy
/// without complaint, so a pull that changes a patch would otherwise
/// build the old code.
fn check_patched_crates(root: &Path) {
  let vendor = root.join("vendor");
  let patches = vendor.join("patches");

  println!("cargo:rerun-if-changed={}", patches.display());

  let Ok(entries) = fs::read_dir(&patches) else {
    return;
  };

  for entry in entries.flatten() {
    let patch = entry.path();
    let Some(stem) = patch
      .file_name()
      .and_then(|n| n.to_str())
      .and_then(|n| n.strip_suffix(".patch"))
    else {
      continue;
    };

    let Some((name, _)) = split_patch_name(stem) else {
      continue;
    };

    let stamp = stamp_path(&vendor, name);
    println!("cargo:rerun-if-changed={}", stamp.display());

    let want =
      stamp_text(&patch).unwrap_or_else(|e| panic!("could not read {}: {e}", patch.display()));
    let have = fs::read_to_string(&stamp).unwrap_or_default();

    if have != want {
      panic!(
        "vendor/crates/{name} was not built from vendor/patches/{}; run `cargo patch-crates`",
        patch.file_name().unwrap_or_default().to_string_lossy()
      );
    }
  }
}

include!("vendor/tool/src/stamp.rs");
