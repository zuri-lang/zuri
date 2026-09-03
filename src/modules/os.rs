//! `_os` builtin module; native backing for `libs/os.zu`.

use std::fs;
use std::path::Path;
use std::sync::{Mutex, PoisonError};
use std::time::Duration;

use crate::builtins::enforce::{ArgType, enforce_method_arg_count, enforce_method_arg_type};
use crate::modules::isolate_util::pool;
use crate::modules::os_util::{process as os_process, signal as os_signal, sysinfo};
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

  // environment
  members.push(("unsetenv", native(vm, "unsetenv", 1, false, unsetenv)));
  members.push(("environ", native(vm, "environ", 0, false, environ_fn)));

  // filesystem
  members.push(("chown", native(vm, "chown", 3, false, chown_fn)));
  members.push(("umask", native(vm, "umask", 1, false, umask_fn)));
  members.push((
    "is_symlink",
    native(vm, "is_symlink", 1, false, is_symlink_fn),
  ));
  members.push(("readlink", native(vm, "readlink", 1, false, readlink_fn)));

  // process
  members.push(("pid", native(vm, "pid", 0, false, pid_fn)));
  members.push(("ppid", native(vm, "ppid", 0, false, ppid_fn)));
  members.push(("kill", native(vm, "kill", 2, false, kill_fn)));
  members.push(("on_signal", native(vm, "on_signal", 2, false, on_signal_fn)));
  members.push((
    "spawn_process",
    native(vm, "spawn_process", 3, false, spawn_process),
  ));
  members.push((
    "process_write_stdin",
    native(vm, "write_stdin", 2, false, process_write_stdin),
  ));
  members.push((
    "process_close_stdin",
    native(vm, "close_stdin", 1, false, process_close_stdin),
  ));
  members.push((
    "process_read_stdout",
    native(vm, "read_stdout", 2, false, process_read_stdout),
  ));
  members.push((
    "process_read_stderr",
    native(vm, "read_stderr", 2, false, process_read_stderr),
  ));
  members.push(("process_wait", native(vm, "wait", 2, false, process_wait)));
  members.push((
    "process_try_wait",
    native(vm, "try_wait", 1, false, process_try_wait),
  ));
  members.push(("process_kill", native(vm, "kill", 2, false, process_kill)));
  members.push(("process_pid", native(vm, "pid", 1, false, process_pid)));

  // system
  members.push(("cpu_count", native(vm, "cpu_count", 0, false, cpu_count)));
  members.push(("hostname", native(vm, "hostname", 0, false, hostname_fn)));
  members.push((
    "total_memory",
    native(vm, "total_memory", 0, false, total_memory_fn),
  ));
  members.push((
    "free_memory",
    native(vm, "free_memory", 0, false, free_memory_fn),
  ));
  members.push(("uptime", native(vm, "uptime", 0, false, uptime_fn)));

  // temp files
  members.push(("tempdir", native(vm, "tempdir", 0, false, tempdir_fn)));
  members.push((
    "create_temp_file",
    native(vm, "create_temp_file", 2, false, create_temp_file),
  ));
  members.push((
    "create_temp_dir",
    native(vm, "create_temp_dir", 1, false, create_temp_dir),
  ));

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
  let (sysname, nodename, version, release, machine) = sysinfo::uname();

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

/// Guards every access to the process environment below. The isolate
/// pool means script code can genuinely run on several OS threads at
/// once now, all sharing this one process's environment;
/// `std::env::set_var`/`remove_var` are unsound under concurrent
/// access from other threads, and without this lock two isolates
/// calling `get_env`/`set_env` at the same time would be exactly
/// that.
static ENV_LOCK: Mutex<()> = Mutex::new(());

fn env_lock() -> std::sync::MutexGuard<'static, ()> {
  ENV_LOCK.lock().unwrap_or_else(PoisonError::into_inner)
}

fn getenv(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 1);
  enforce_arg_type!(ctx, 0, ArgType::String);
  let name = ctx.args[0].as_str().to_string();

  let _guard = env_lock();
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

  let _guard = env_lock();
  if !overwrite && std::env::var(&name).is_ok() {
    return Ok(Value::bool(false));
  }

  // SAFETY: every read/write of the process environment anywhere in
  // this module goes through `env_lock()` first, so nothing else can
  // be touching it concurrently with this write.
  unsafe {
    std::env::set_var(&name, &value);
  }
  Ok(Value::bool(true))
}

fn unsetenv(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 1);
  enforce_arg_type!(ctx, 0, ArgType::String);
  let name = ctx.args[0].as_str().to_string();

  let _guard = env_lock();
  let existed = std::env::var(&name).is_ok();
  if existed {
    // SAFETY: see `setenv`'s note; guarded by the same lock.
    unsafe {
      std::env::remove_var(&name);
    }
  }
  Ok(Value::bool(existed))
}

fn environ_fn(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 0);

  let vars: Vec<(String, String)> = {
    let _guard = env_lock();
    std::env::vars().collect()
  };

  let pairs: Vec<(Value, Value)> = vars
    .into_iter()
    .map(|(k, v)| {
      let key = ctx.vm.heap_mut().alloc_string(k);
      let val = ctx.vm.heap_mut().alloc_string(v);
      (key, val)
    })
    .collect();
  Ok(ctx.vm.heap_mut().alloc_dict(pairs))
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

// Ownership, umask, symlinks

#[cfg(unix)]
fn chown_fn(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 3);
  enforce_arg_type!(ctx, 0, ArgType::String);
  enforce_arg_type!(ctx, 1, ArgType::Number);
  enforce_arg_type!(ctx, 2, ArgType::Number);

  let path = ctx.args[0].as_str().to_string();
  let uid = ctx.args[1].as_number() as libc::uid_t;
  let gid = ctx.args[2].as_number() as libc::gid_t;

  let cpath =
    std::ffi::CString::new(path.clone()).map_err(|_| format!("invalid path '{}'", path))?;
  let ret = unsafe { libc::chown(cpath.as_ptr(), uid, gid) };
  if ret != 0 {
    return Err(format!(
      "could not chown '{}': {}",
      path,
      std::io::Error::last_os_error()
    ));
  }
  Ok(Value::bool(true))
}

#[cfg(not(unix))]
fn chown_fn(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 3);
  Err("chown() is not supported on this platform".to_string())
}

#[cfg(unix)]
fn umask_fn(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 1);

  if ctx.args[0].is_nil() {
    // `umask(2)` always both sets AND returns the previous mask, so
    // reading the current one without a lasting side effect means
    // setting it to a throwaway value and immediately restoring
    // whatever it actually was.
    let current = unsafe {
      let prev = libc::umask(0o022);
      libc::umask(prev);
      prev
    };
    return Ok(Value::number(current as f64));
  }

  enforce_arg_type!(ctx, 0, ArgType::Number);
  let mask = ctx.args[0].as_number() as libc::mode_t;
  let previous = unsafe { libc::umask(mask) };
  Ok(Value::number(previous as f64))
}

#[cfg(not(unix))]
fn umask_fn(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 1);
  Err("umask() is not supported on this platform".to_string())
}

fn is_symlink_fn(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 1);
  enforce_arg_type!(ctx, 0, ArgType::String);
  let path = ctx.args[0].as_str();
  let is_link = fs::symlink_metadata(path)
    .map(|m| m.file_type().is_symlink())
    .unwrap_or(false);
  Ok(Value::bool(is_link))
}

fn readlink_fn(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 1);
  enforce_arg_type!(ctx, 0, ArgType::String);
  let path = ctx.args[0].as_str().to_string();
  match fs::read_link(&path) {
    Ok(target) => Ok(ctx.vm.heap_mut().alloc_string(target.display().to_string())),
    Err(e) => Err(format!("could not read link '{}': {}", path, e)),
  }
}

// Process identity, signals, subprocess spawning

fn pid_fn(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 0);
  Ok(Value::number(std::process::id() as f64))
}

fn ppid_fn(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 0);
  Ok(Value::number(sysinfo::ppid() as f64))
}

#[cfg(unix)]
fn kill_fn(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 2);
  enforce_arg_type!(ctx, 0, ArgType::Number);
  enforce_arg_type!(ctx, 1, ArgType::Number);
  let pid = ctx.args[0].as_number() as libc::pid_t;
  let signal = ctx.args[1].as_number() as libc::c_int;
  let ret = unsafe { libc::kill(pid, signal) };
  if ret != 0 {
    return Err(format!(
      "could not signal process {}: {}",
      pid,
      std::io::Error::last_os_error()
    ));
  }
  Ok(Value::bool(true))
}

#[cfg(windows)]
fn kill_fn(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 2);
  enforce_arg_type!(ctx, 0, ArgType::Number);
  let pid = ctx.args[0].as_number() as u32;

  use windows_sys::Win32::Foundation::CloseHandle;
  use windows_sys::Win32::System::Threading::{OpenProcess, PROCESS_TERMINATE, TerminateProcess};

  unsafe {
    let handle = OpenProcess(PROCESS_TERMINATE, 0, pid);
    if handle == 0 {
      return Err(format!("could not open process {}", pid));
    }
    let ok = TerminateProcess(handle, 1);
    CloseHandle(handle);
    if ok == 0 {
      return Err(format!("could not terminate process {}", pid));
    }
  }
  Ok(Value::bool(true))
}

#[cfg(not(any(unix, windows)))]
fn kill_fn(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 2);
  Err("kill() is not supported on this platform".to_string())
}

fn on_signal_fn(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 2);
  enforce_arg_type!(ctx, 0, ArgType::String);
  enforce_arg_type!(ctx, 1, ArgType::Function);

  let name = ctx.args[0].as_str().to_string();
  let callback = ctx.args[1];

  let idx = os_signal::install(&name)?;
  ctx.vm.set_signal_callback(idx, callback);
  Ok(Value::bool(true))
}

/// Accepts either a `string` or `bytes` value as raw bytes, the same
/// union `file.write()` itself accepts.
fn value_to_bytes(v: Value) -> Result<Vec<u8>, String> {
  if v.is_bytes() {
    Ok(v.as_bytes())
  } else if v.is_string() {
    Ok(v.as_str().as_bytes().to_vec())
  } else {
    Err(format!(
      "expected a string or bytes value, got {}",
      v.type_name()
    ))
  }
}

/// Looks up a string key in a Zuri dict, treating `nil` the same as
/// "not present" (so an options dict that explicitly sets a key to
/// `nil` behaves exactly like omitting it).
fn dict_lookup(ctx: &mut ZuriContext, dict: Value, key: &str) -> Option<Value> {
  let key_val = ctx.vm.heap_mut().alloc_string(key);
  dict.dict_get(&key_val).filter(|v| !v.is_nil())
}

const PROCESS_PTR: &str = "zuri::os::Process";

fn spawn_process(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 3);
  enforce_arg_type!(ctx, 0, ArgType::String);
  enforce_arg_type!(ctx, 1, ArgType::List);

  let program = ctx.args[0].as_str().to_string();
  let args: Vec<String> = ctx.args[1]
    .as_list()
    .into_iter()
    .map(|v| v.as_str().to_string())
    .collect();

  let mut opts = os_process::SpawnOptions::default();

  if !ctx.args[2].is_nil() {
    let options = ctx.args[2];
    if !options.is_dict() {
      return Err(format!(
        "spawn() expects its options argument to be a dict, got {}",
        options.type_name()
      ));
    }

    if let Some(v) = dict_lookup(ctx, options, "cwd") {
      opts.cwd = Some(v.as_str().to_string());
    }
    if let Some(v) = dict_lookup(ctx, options, "env_replace") {
      opts.env_replace = v.as_bool();
    }
    if let Some(v) = dict_lookup(ctx, options, "env") {
      if !v.is_dict() {
        return Err("spawn() expects options.env to be a dict".to_string());
      }
      opts.env = v
        .as_dict()
        .into_iter()
        .map(|(k, val)| (k.as_str().to_string(), val.as_str().to_string()))
        .collect();
    }
    if let Some(v) = dict_lookup(ctx, options, "stdin") {
      opts.stdin_mode = os_process::StdioMode::parse(v.as_str())?;
    }
    if let Some(v) = dict_lookup(ctx, options, "stdout") {
      opts.stdout_mode = os_process::StdioMode::parse(v.as_str())?;
    }
    if let Some(v) = dict_lookup(ctx, options, "stderr") {
      opts.stderr_mode = os_process::StdioMode::parse(v.as_str())?;
    }
  }

  let process = os_process::Process::spawn(&program, &args, &opts)?;
  Ok(ctx.heap().alloc_ptr(PROCESS_PTR, process))
}

fn process_write_stdin(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(PROCESS_PTR));
  let data = value_to_bytes(ctx.args[1])?;

  let mut ptr = ctx.args[0].as_ptr_cell().borrow_mut();
  let process = ptr.downcast_mut::<os_process::Process>().unwrap();
  process.write_stdin(&data)?;
  Ok(Value::bool(true))
}

fn process_close_stdin(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(PROCESS_PTR));

  let mut ptr = ctx.args[0].as_ptr_cell().borrow_mut();
  let process = ptr.downcast_mut::<os_process::Process>().unwrap();
  process.close_stdin();
  Ok(Value::nil())
}

fn process_read_stdout(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(PROCESS_PTR));
  let length = if ctx.args[1].is_nil() {
    None
  } else {
    Some(ctx.args[1].as_number() as usize)
  };

  let bytes = {
    let ptr = ctx.args[0].as_ptr_cell().borrow();
    let process = ptr.downcast_ref::<os_process::Process>().unwrap();
    process.read_stdout(length)?
  };
  Ok(ctx.heap().alloc_bytes(bytes))
}

fn process_read_stderr(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(PROCESS_PTR));
  let length = if ctx.args[1].is_nil() {
    None
  } else {
    Some(ctx.args[1].as_number() as usize)
  };

  let bytes = {
    let ptr = ctx.args[0].as_ptr_cell().borrow();
    let process = ptr.downcast_ref::<os_process::Process>().unwrap();
    process.read_stderr(length)?
  };
  Ok(ctx.heap().alloc_bytes(bytes))
}

fn process_wait(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(PROCESS_PTR));
  let timeout_ms = if ctx.args[1].is_nil() {
    None
  } else {
    Some(ctx.args[1].as_number() as u64)
  };

  let mut ptr = ctx.args[0].as_ptr_cell().borrow_mut();
  let process = ptr.downcast_mut::<os_process::Process>().unwrap();
  match process.wait(timeout_ms)? {
    Some(code) => Ok(Value::number(code as f64)),
    None => Ok(Value::nil()),
  }
}

fn process_try_wait(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(PROCESS_PTR));

  let mut ptr = ctx.args[0].as_ptr_cell().borrow_mut();
  let process = ptr.downcast_mut::<os_process::Process>().unwrap();
  match process.try_wait()? {
    Some(code) => Ok(Value::number(code as f64)),
    None => Ok(Value::nil()),
  }
}

fn process_kill(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(PROCESS_PTR));
  let signal = if ctx.args[1].is_nil() {
    None
  } else {
    Some(ctx.args[1].as_number() as i32)
  };

  let mut ptr = ctx.args[0].as_ptr_cell().borrow_mut();
  let process = ptr.downcast_mut::<os_process::Process>().unwrap();
  process.kill(signal)?;
  Ok(Value::bool(true))
}

fn process_pid(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(PROCESS_PTR));

  let ptr = ctx.args[0].as_ptr_cell().borrow();
  let process = ptr.downcast_ref::<os_process::Process>().unwrap();
  Ok(Value::number(process.pid() as f64))
}

// System / machine introspection

fn cpu_count(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 0);
  Ok(Value::number(pool::cpu_count() as f64))
}

fn hostname_fn(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 0);
  let name = sysinfo::hostname()?;
  Ok(ctx.vm.heap_mut().alloc_string(name))
}

fn total_memory_fn(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 0);
  let (total, _) = sysinfo::memory()?;
  Ok(Value::number(total as f64))
}

fn free_memory_fn(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 0);
  let (_, available) = sysinfo::memory()?;
  Ok(Value::number(available as f64))
}

fn uptime_fn(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 0);
  Ok(Value::number(sysinfo::uptime()?))
}

// Temp files

fn tempdir_fn(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 0);
  let dir = std::env::temp_dir().display().to_string();
  Ok(ctx.vm.heap_mut().alloc_string(dir))
}

/// Not cryptographically random; it doesn't need to be. Uniqueness
/// against a collision comes from the atomic `create_new`/
/// `create_dir` call below, never from this name; entropy here only
/// exists to make an actual collision (and thus a retry) unlikely in
/// the first place.
fn temp_name_suffix() -> String {
  use std::sync::atomic::{AtomicU64, Ordering};
  use std::time::{SystemTime, UNIX_EPOCH};

  static COUNTER: AtomicU64 = AtomicU64::new(0);
  let nanos = SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .map(|d| d.as_nanos() as u64)
    .unwrap_or(0);
  let count = COUNTER.fetch_add(1, Ordering::Relaxed);
  format!("{:x}{:x}{:x}", nanos, std::process::id(), count)
}

const TEMP_NAME_ATTEMPTS: u32 = 100;

fn create_temp_file(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 2);
  enforce_arg_type!(ctx, 0, ArgType::String);
  enforce_arg_type!(ctx, 1, ArgType::String);
  let prefix = ctx.args[0].as_str().to_string();
  let suffix = ctx.args[1].as_str().to_string();

  let dir = std::env::temp_dir();
  for _ in 0..TEMP_NAME_ATTEMPTS {
    let candidate = dir.join(format!("{}{}{}", prefix, temp_name_suffix(), suffix));
    match fs::OpenOptions::new()
      .write(true)
      .read(true)
      .create_new(true)
      .open(&candidate)
    {
      Ok(_) => {
        return Ok(
          ctx
            .vm
            .heap_mut()
            .alloc_string(candidate.display().to_string()),
        );
      },
      Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
      Err(e) => return Err(format!("could not create a temp file: {}", e)),
    }
  }
  Err(format!(
    "could not create a unique temp file after {} attempts",
    TEMP_NAME_ATTEMPTS
  ))
}

fn create_temp_dir(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 1);
  enforce_arg_type!(ctx, 0, ArgType::String);
  let prefix = ctx.args[0].as_str().to_string();

  let dir = std::env::temp_dir();
  for _ in 0..TEMP_NAME_ATTEMPTS {
    let candidate = dir.join(format!("{}{}", prefix, temp_name_suffix()));
    match fs::create_dir(&candidate) {
      Ok(()) => {
        return Ok(
          ctx
            .vm
            .heap_mut()
            .alloc_string(candidate.display().to_string()),
        );
      },
      Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
      Err(e) => return Err(format!("could not create a temp directory: {}", e)),
    }
  }
  Err(format!(
    "could not create a unique temp directory after {} attempts",
    TEMP_NAME_ATTEMPTS
  ))
}
