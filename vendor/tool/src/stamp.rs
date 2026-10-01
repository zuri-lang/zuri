// What marks a copy in `vendor/crates` as built from a given patch.
//
// `build.rs` includes this file to refuse a copy built from any other
// patch, so the tool and the build agree on what up to date means.
// Nothing here names a `use`, because an included file shares the scope
// it lands in.

/// Splits a patch's file stem, such as `cranelift-codegen-0.136.1`, into
/// the crate and its version. The version starts after the last hyphen
/// that a digit follows, which keeps both `foo-2d-1.0.0` and
/// `foo-1.0.0-rc.1` whole.
pub fn split_patch_name(stem: &str) -> Option<(&str, &str)> {
  let (at, _) = stem
    .rmatch_indices('-')
    .find(|(at, _)| stem[at + 1..].starts_with(|c: char| c.is_ascii_digit()))?;

  Some((&stem[..at], &stem[at + 1..]))
}

/// The stamp beside a crate's copy.
pub fn stamp_path(vendor: &std::path::Path, name: &str) -> std::path::PathBuf {
  vendor.join("crates").join(format!("{name}.applied"))
}

/// What the stamp holds once `patch` has been applied: the patch's file
/// name, then every byte of it. A patch edited in place changes the
/// stamp as surely as one renamed for a new version.
pub fn stamp_text(patch: &std::path::Path) -> std::io::Result<String> {
  let name = patch.file_name().unwrap_or_default().to_string_lossy();
  let body = std::fs::read_to_string(patch)?;

  Ok(format!("{name}\n{body}"))
}
