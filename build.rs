// build.rs
use chrono::Utc;
use std::env;
use std::fs;

fn main() {
  let now = Utc::now().format("%Y-%m-%d %H:%M:%S UTC").to_string();

  // 1. Locate and read the Cargo.toml file
  let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
  let toml_path = format!("{}/Cargo.toml", manifest_dir);
  let toml_content = fs::read_to_string(&toml_path).expect("Failed to read Cargo.toml");

  // 2. Parse the TOML content
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

  let debug_opcodes = parsed_toml
    .get("package")
    .and_then(|p| p.get("metadata"))
    .and_then(|m| m.get("zuri"))
    .and_then(|c| c.get("debug_opcodes"))
    .and_then(|v| v.as_bool())
    .unwrap_or(false);

  // 4. Pass it to the main application via cargo environment variables
  println!("cargo:rustc-env=ZURI_VERSION={}", zuri_version);
  println!("cargo:rustc-env=ZVM_VERSION={}", vm_version);
  println!("cargo:rustc-env=ZURI_BUILD_TIME={}", now);
  println!("cargo:rustc-env=ZURI_DEBUG_OPCODES={}", debug_opcodes);

  // Tell Cargo to re-run this script only if Cargo.toml changes
  println!("cargo:rerun-if-changed=Cargo.toml");
}
