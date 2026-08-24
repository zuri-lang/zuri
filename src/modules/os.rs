//! `_os` builtin module; native backing for `libs/os.zu`.

use std::fs;
use std::path::Path;
use std::time::Duration;

use crate::builtins::enforce::ArgType;
use crate::modules::{BuiltinModuleDef, native};
use crate::vm::object::ZuriContext;
use crate::vm::value::Value;
use crate::vm::vm::VM;
use crate::{enforce_arg_count, enforce_arg_type};

pub static MODULE: BuiltinModuleDef = BuiltinModuleDef { name: "_os", build };

fn build(vm: &mut VM) -> Vec<(&'static str, Value)> {
  let mut members: Vec<(&'static str, Value)> = Vec::new();

  // functions
  members.push(("exec", native(vm, "exec", 1, false, exec)));
  members.push(("info", native(vm, "info", 0, false, info)));
  members.push(("sleep", native(vm, "sleep", 1, false, sleep_fn)));
  members.push(("getenv", native(vm, "getenv", 1, false, getenv)));
  members.push(("setenv", native(vm, "setenv", 3, false, setenv)));
  members.push(("createdir", native(vm, "createdir", 3, false, createdir)));
  members.push(("readdir", native(vm, "readdir", 2, false, readdir)));
  members.push(("chmod", native(vm, "chmod", 2, false, chmod_fn)));
  members.push(("isdir", native(vm, "isdir", 1, false, isdir)));
  members.push(("removedir", native(vm, "removedir", 2, false, removedir)));
  members.push(("cwd", native(vm, "cwd", 0, false, cwd_fn)));
  members.push(("chdir", native(vm, "chdir", 1, false, chdir_fn)));
  members.push(("exists", native(vm, "exists", 1, false, exists_fn)));
  members.push(("exit", native(vm, "exit", 1, false, exit_fn)));
  members.push(("realpath", native(vm, "realpath", 1, false, realpath_fn)));
  members.push(("dirname", native(vm, "dirname", 1, false, dirname_fn)));
  members.push(("basename", native(vm, "basename", 1, false, basename_fn)));
  members.push(("rename", native(vm, "rename", 2, false, rename_fn)));
  members.push(("homedir", native(vm, "homedir", 0, false, homedir_fn)));

  // constants
  let version_val = vm.heap_mut().alloc_string(env!("ZURI_VERSION"));
  members.push(("version", version_val));

  let vm_version_val = vm.heap_mut().alloc_string(env!("ZVM_VERSION"));
  members.push(("vm_version", vm_version_val));

  let platform_val = vm.heap_mut().alloc_string(std::env::consts::OS);
  members.push(("platform", platform_val));

  // The current CLI only ever accepts a single (script path) argument
  //: see `zuri.rs`'s `args.len() > 2` guard; so there is never
  // anything past the executable and the script path itself to skip;
  // kept as a real (if today always-empty) skip(2) so this keeps
  // working the moment extra-argument support is added there.
  let args: Vec<Value> = std::env::args()
    // .skip(2)
    .map(|a| vm.heap_mut().alloc_string(a))
    .collect();
  let args_val = vm.heap_mut().alloc_list(args);
  members.push(("args", args_val));

  let sep_val = vm
    .heap_mut()
    .alloc_string(std::path::MAIN_SEPARATOR.to_string());
  members.push(("path_separator", sep_val));

  let exe_path_str = std::env::current_exe()
    .map(|p| p.display().to_string())
    .unwrap_or_default();
  let exe_path_val = vm.heap_mut().alloc_string(exe_path_str);
  members.push(("exe_path", exe_path_val));

  // dirent `d_type` constants (POSIX values; -1 where genuinely
  // unsupported by the platform, per spec).
  members.push(("DT_UNKNOWN", Value::number(0.0)));
  members.push(("DT_FIFO", Value::number(1.0)));
  members.push(("DT_CHR", Value::number(2.0)));
  members.push(("DT_DIR", Value::number(4.0)));
  members.push(("DT_BLK", Value::number(6.0)));
  members.push(("DT_REG", Value::number(8.0)));
  members.push(("DT_LNK", Value::number(10.0)));
  members.push(("DT_SOCK", Value::number(12.0)));
  #[cfg(any(
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd",
    target_os = "macos"
  ))]
  members.push(("DT_WHT", Value::number(14.0)));
  #[cfg(not(any(
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd",
    target_os = "macos"
  )))]
  members.push(("DT_WHT", Value::number(-1.0)));

  members
}

// exec / info / sleep

/// `_os.exec(cmd)` -> `[exit_code, output]`. Runs `cmd` through the
/// platform shell (`cmd /C` on Windows, `sh -c` elsewhere); matches
/// `os.zu`'s own doc comment ("Executes the given shell (or command
/// prompt for Windows) commands").
fn exec(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 1);
  enforce_arg_type!(ctx, 0, ArgType::String);
  let cmd = ctx.args[0].as_str().to_string();

  let output = if cfg!(windows) {
    std::process::Command::new("cmd")
      .args(["/C", &cmd])
      .output()
  } else {
    std::process::Command::new("sh").args(["-c", &cmd]).output()
  };

  match output {
    Ok(out) => {
      let code = out.status.code().unwrap_or(-1) as f64;
      let mut text = String::from_utf8_lossy(&out.stdout).into_owned();
      if !out.stderr.is_empty() {
        if !text.is_empty() {
          text.push('\n');
        }
        text.push_str(&String::from_utf8_lossy(&out.stderr));
      }
      let out_val = ctx.vm.heap_mut().alloc_string(text);
      let items = vec![Value::number(code), out_val];
      Ok(ctx.vm.heap_mut().alloc_list(items))
    },
    Err(e) => Err(format!("failed to execute command: {}", e)),
  }
}

/// `_os.info()` -> `{sysname, nodename, version, release, machine}`.
fn info(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 0);
  let (sysname, nodename, version, release, machine) = gather_uname();

  let sysname_val = ctx.vm.heap_mut().alloc_string(sysname);
  let nodename_val = ctx.vm.heap_mut().alloc_string(nodename);
  let version_val = ctx.vm.heap_mut().alloc_string(version);
  let release_val = ctx.vm.heap_mut().alloc_string(release);
  let machine_val = ctx.vm.heap_mut().alloc_string(machine);

  let key_sysname = ctx.vm.heap_mut().alloc_string("sysname");
  let key_nodename = ctx.vm.heap_mut().alloc_string("nodename");
  let key_version = ctx.vm.heap_mut().alloc_string("version");
  let key_release = ctx.vm.heap_mut().alloc_string("release");
  let key_machine = ctx.vm.heap_mut().alloc_string("machine");

  let pairs = vec![
    (key_sysname, sysname_val),
    (key_nodename, nodename_val),
    (key_version, version_val),
    (key_release, release_val),
    (key_machine, machine_val),
  ];
  Ok(ctx.vm.heap_mut().alloc_dict(pairs))
}

#[cfg(unix)]
fn uname_field(flag: &str) -> String {
  std::process::Command::new("uname")
    .arg(flag)
    .output()
    .ok()
    .and_then(|o| String::from_utf8(o.stdout).ok())
    .map(|s| s.trim().to_string())
    .filter(|s| !s.is_empty())
    .unwrap_or_else(|| "unknown".to_string())
}

#[cfg(unix)]
fn gather_uname() -> (String, String, String, String, String) {
  (
    uname_field("-s"),
    uname_field("-n"),
    uname_field("-v"),
    uname_field("-r"),
    uname_field("-m"),
  )
}

/// Best-effort fallback where `uname` isn't a meaningful thing to shell
/// out to (Windows); built from Rust's own compile-time platform
/// constants rather than left blank.
#[cfg(not(unix))]
fn gather_uname() -> (String, String, String, String, String) {
  let sysname = std::env::consts::OS.to_string();
  let nodename = std::env::var("COMPUTERNAME").unwrap_or_else(|_| "unknown".to_string());
  let version = "unknown".to_string();
  let release = "unknown".to_string();
  let machine = std::env::consts::ARCH.to_string();
  (sysname, nodename, version, release, machine)
}

fn sleep_fn(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 1);
  enforce_arg_type!(ctx, 0, ArgType::Number);
  let secs = ctx.args[0].as_number();
  if secs > 0.0 {
    std::thread::sleep(Duration::from_secs_f64(secs));
  }
  Ok(Value::nil())
}

// Environment variables

fn getenv(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 1);
  enforce_arg_type!(ctx, 0, ArgType::String);
  let name = ctx.args[0].as_str().to_string();
  match std::env::var(&name) {
    Ok(v) => Ok(ctx.vm.heap_mut().alloc_string(v)),
    Err(_) => Ok(Value::nil()),
  }
}

/// Only overwrites an existing variable when `overwrite` is true, per
/// `os.zu`'s own documented default-false behavior.
fn setenv(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 3);
  enforce_arg_type!(ctx, 0, ArgType::String);
  enforce_arg_type!(ctx, 1, ArgType::String);
  enforce_arg_type!(ctx, 2, ArgType::Bool);

  let name = ctx.args[0].as_str().to_string();
  let value = ctx.args[1].as_str().to_string();
  let overwrite = ctx.args[2].as_bool();

  if !overwrite && std::env::var(&name).is_ok() {
    return Ok(Value::bool(false));
  }

  // SAFETY: Zuri is single-threaded at the point any script code runs
  // (no native concurrency primitive exists in this VM), so there is
  // no other thread that could be concurrently reading the process
  // environment while this write happens.
  unsafe {
    std::env::set_var(&name, &value);
  }
  Ok(Value::bool(true))
}

// Directories

fn createdir(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 3);
  enforce_arg_type!(ctx, 0, ArgType::String);
  enforce_arg_type!(ctx, 1, ArgType::Number);
  enforce_arg_type!(ctx, 2, ArgType::Bool);

  let path = ctx.args[0].as_str();
  let permission = ctx.args[1].as_number() as u32;
  let recursive = ctx.args[2].as_bool();

  // Per spec: "if the directory already exists, it returns false".
  if Path::new(path).exists() {
    return Ok(Value::bool(false));
  }

  let result = if recursive {
    fs::create_dir_all(path)
  } else {
    fs::create_dir(path)
  };

  match result {
    Ok(()) => {
      #[cfg(unix)]
      {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(path, fs::Permissions::from_mode(permission));
      }
      #[cfg(not(unix))]
      {
        let _ = permission;
      }
      Ok(Value::bool(true))
    },
    Err(e) => Err(format!("could not create directory '{}': {}", path, e)),
  }
}

/// Flat listing (plus the synthetic `.`/`..` entries the doc example
/// shows); when `recursive` is set, nested entries are appended
/// depth-first after their own containing directory's siblings.
fn readdir(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 2);
  enforce_arg_type!(ctx, 0, ArgType::String);
  enforce_arg_type!(ctx, 1, ArgType::Bool);

  let path = ctx.args[0].as_str().to_string();
  let recursive = ctx.args[1].as_bool();

  let mut names: Vec<String> = vec![".".to_string(), "..".to_string()];
  collect_dir_entries(Path::new(&path), recursive, &mut names)
    .map_err(|e| format!("could not read directory '{}': {}", path, e))?;

  let items: Vec<Value> = names
    .into_iter()
    .map(|n| ctx.vm.heap_mut().alloc_string(n))
    .collect();
  Ok(ctx.vm.heap_mut().alloc_list(items))
}

fn collect_dir_entries(dir: &Path, recursive: bool, out: &mut Vec<String>) -> std::io::Result<()> {
  // Start with an empty prefix so root entries are just their own names
  collect_dir_entries_inner(dir, Path::new(""), recursive, out)
}

fn collect_dir_entries_inner(
  dir: &Path,
  prefix: &Path,
  recursive: bool,
  out: &mut Vec<String>,
) -> std::io::Result<()> {
  for entry in fs::read_dir(dir)? {
    let entry = entry?;
    let name = entry.file_name();

    // Build the relative path from the original directory
    let rel_path = prefix.join(&name);
    out.push(rel_path.to_string_lossy().into_owned());

    if recursive && entry.file_type()?.is_dir() {
      // Guard against infinite recursion into . and ..
      if name != "." && name != ".." {
        collect_dir_entries_inner(&entry.path(), &rel_path, recursive, out)?;
      }
    }
  }
  Ok(())
}

fn chmod_fn(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 2);
  enforce_arg_type!(ctx, 0, ArgType::String);
  enforce_arg_type!(ctx, 1, ArgType::Number);

  let path = ctx.args[0].as_str().to_string();
  let mode = ctx.args[1].as_number() as u32;

  #[cfg(unix)]
  {
    use std::os::unix::fs::PermissionsExt;
    match fs::set_permissions(&path, fs::Permissions::from_mode(mode)) {
      Ok(()) => Ok(Value::bool(true)),
      Err(e) => Err(format!("could not chmod '{}': {}", path, e)),
    }
  }
  #[cfg(windows)]
  {
    // Windows has no Unix-style permission bits. Best-effort: map the
    // owner-write bit (0o200) to the read-only attribute.
    let readonly = (mode & 0o200) == 0;
    match fs::metadata(&path) {
      Ok(metadata) => {
        let mut permissions = metadata.permissions();
        permissions.set_readonly(readonly);
        match fs::set_permissions(&path, permissions) {
          Ok(()) => Ok(Value::bool(true)),
          Err(e) => Err(format!("could not chmod '{}': {}", path, e)),
        }
      },
      Err(e) => Err(format!("could not chmod '{}': {}", path, e)),
    }
  }
  #[cfg(not(any(unix, windows)))]
  {
    let _ = mode;
    Err("chmod() is only supported on Unix and Windows platforms".to_string())
  }
}

fn isdir(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 1);
  enforce_arg_type!(ctx, 0, ArgType::String);
  let path = ctx.args[0].as_str();
  Ok(Value::bool(Path::new(path).is_dir()))
}

fn removedir(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 2);
  enforce_arg_type!(ctx, 0, ArgType::String);
  enforce_arg_type!(ctx, 1, ArgType::Bool);

  let path = ctx.args[0].as_str().to_string();
  let recursive = ctx.args[1].as_bool();

  let result = if recursive {
    fs::remove_dir_all(&path)
  } else {
    fs::remove_dir(&path)
  };

  match result {
    Ok(()) => Ok(Value::bool(true)),
    Err(e) => Err(format!("could not remove directory '{}': {}", path, e)),
  }
}

fn cwd_fn(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 0);
  match std::env::current_dir() {
    Ok(p) => Ok(ctx.vm.heap_mut().alloc_string(p.display().to_string())),
    Err(e) => Err(format!("could not get current working directory: {}", e)),
  }
}

fn chdir_fn(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 1);
  enforce_arg_type!(ctx, 0, ArgType::String);
  let path = ctx.args[0].as_str().to_string();
  match std::env::set_current_dir(&path) {
    Ok(()) => Ok(Value::bool(true)),
    Err(_) => Ok(Value::bool(false)),
  }
}

fn exists_fn(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 1);
  enforce_arg_type!(ctx, 0, ArgType::String);
  let path = ctx.args[0].as_str();
  Ok(Value::bool(Path::new(path).exists()))
}

// Process control

fn exit_fn(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 1);
  enforce_arg_type!(ctx, 0, ArgType::Number);
  let code = ctx.args[0].as_number() as i32;
  std::process::exit(code);
}

// Path helpers

fn realpath_fn(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 1);
  enforce_arg_type!(ctx, 0, ArgType::String);
  let path = ctx.args[0].as_str().to_string();
  let resolved = fs::canonicalize(&path)
    .map(|p| p.display().to_string())
    .unwrap_or(path);
  Ok(ctx.vm.heap_mut().alloc_string(resolved))
}

/// Mirrors POSIX `dirname(3)`: no `/` in `path` (or an empty `path`)
/// yields `"."`; an all-slash `path` yields `"/"`.
fn dirname_fn(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 1);
  enforce_arg_type!(ctx, 0, ArgType::String);
  let path = ctx.args[0].as_str();

  let result = if path.is_empty() {
    ".".to_string()
  } else {
    let trimmed = path.trim_end_matches(is_sep);
    if trimmed.is_empty() {
      // path was made up entirely of separators
      std::path::MAIN_SEPARATOR.to_string()
    } else {
      match trimmed.rfind(is_sep) {
        Some(0) => std::path::MAIN_SEPARATOR.to_string(),
        Some(idx) => trimmed[..idx].to_string(),
        None => ".".to_string(),
      }
    }
  };
  Ok(ctx.vm.heap_mut().alloc_string(result))
}

/// Mirrors POSIX `basename(3)`: an all-slash `path` yields `"/"`; an
/// empty `path` yields `"."`.
fn basename_fn(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 1);
  enforce_arg_type!(ctx, 0, ArgType::String);
  let path = ctx.args[0].as_str();

  let result = if path.is_empty() {
    ".".to_string()
  } else {
    let trimmed = path.trim_end_matches(is_sep);
    if trimmed.is_empty() {
      std::path::MAIN_SEPARATOR.to_string()
    } else {
      match trimmed.rfind(is_sep) {
        Some(idx) => trimmed[idx + 1..].to_string(),
        None => trimmed.to_string(),
      }
    }
  };
  Ok(ctx.vm.heap_mut().alloc_string(result))
}

fn is_sep(c: char) -> bool {
  c == '/' || c == std::path::MAIN_SEPARATOR
}

fn rename_fn(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 2);
  enforce_arg_type!(ctx, 0, ArgType::String);
  enforce_arg_type!(ctx, 1, ArgType::String);
  let old_name = ctx.args[0].as_str().to_string();
  let new_name = ctx.args[1].as_str().to_string();
  match fs::rename(&old_name, &new_name) {
    Ok(()) => Ok(Value::bool(true)),
    Err(e) => Err(format!(
      "could not rename '{}' to '{}': {}",
      old_name, new_name, e
    )),
  }
}

fn homedir_fn(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 0);
  match home_dir_str() {
    Some(h) => Ok(ctx.vm.heap_mut().alloc_string(h)),
    None => Ok(Value::nil()),
  }
}

#[cfg(unix)]
fn home_dir_str() -> Option<String> {
  std::env::var("HOME").ok()
}

#[cfg(windows)]
fn home_dir_str() -> Option<String> {
  std::env::var("USERPROFILE").ok()
}

#[cfg(not(any(unix, windows)))]
fn home_dir_str() -> Option<String> {
  None
}
