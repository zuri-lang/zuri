//! What a `zuri` command line means, worked out before anything is
//! read or compiled.
//!
//! The executable has three shapes. With nothing after it, it is the
//! REPL. With `run`, it launches a script, a package directory, or the
//! working directory's own entrypoint. With anything else, that word
//! names a command, and commands live in a `cmds` directory beside the
//! installed runtime or in the project's `.zuri` directory.
//!
//! `--version` sits outside all three: it reports what the runtime is
//! and runs nothing.

use std::path::{Component, Path, PathBuf};
use std::sync::OnceLock;

/// The file a directory is launched through.
pub const ENTRY_FILE: &str = "index.zu";

/// The extension a launchable script carries.
pub const SCRIPT_EXT: &str = "zu";

/// The directory commands are looked up in, both beside the runtime
/// and under a project's `.zuri`.
pub const COMMAND_DIR: &str = "cmds";

/// The subcommand that launches a script rather than naming a command.
const RUN: &str = "run";

/// The flag that reports what the runtime is instead of running
/// anything.
const VERSION_FLAG: &str = "--version";

/// A resolved command line.
pub enum Launch {
  /// Nothing to run: drop the user into the interactive prompt.
  Repl,
  /// Report what the runtime is and stop.
  Version,
  /// A script to execute, with whatever was meant for it.
  Script(Script),
}

/// Everything the runtime needs to launch one script.
pub struct Script {
  /// The file to compile and run.
  pub path: PathBuf,
  /// What the user typed, which is what diagnostics should name.
  pub name: String,
  /// The arguments meant for the script, with none of zuri's own left
  /// among them.
  pub args: Vec<String>,
}

/// A command line that names nothing runnable.
pub struct LaunchError {
  /// The path or command the user asked for.
  pub name: String,
  /// Why it could not be launched.
  pub reason: String,
}

impl LaunchError {
  fn new(name: impl Into<String>, reason: &str) -> Self {
    LaunchError {
      name: name.into(),
      reason: reason.to_string(),
    }
  }
}

/// Works out what `args` asks for. `args` is the command line with the
/// executable's own name already dropped.
pub fn resolve(args: &[String]) -> Result<Launch, LaunchError> {
  match args.first() {
    None => Ok(Launch::Repl),
    Some(first) if first == VERSION_FLAG => Ok(Launch::Version),
    Some(first) if first == RUN => resolve_run(&args[1..]),
    Some(first) => resolve_command(first, &args[1..]),
  }
}

/// `zuri run [path] [args...]`.
///
/// A first argument starting with a dash is an option for the script
/// rather than a path, so `zuri run --verbose` still launches the
/// working directory and forwards the flag.
fn resolve_run(rest: &[String]) -> Result<Launch, LaunchError> {
  let (target, args) = match rest.first() {
    Some(first) if !first.starts_with('-') => (Some(first.as_str()), &rest[1..]),
    _ => (None, rest),
  };

  let (path, name) = match target {
    None => {
      let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
      let name = cwd.display().to_string();
      (entrypoint_of(&cwd, &name)?, name)
    },
    Some(given) => (resolve_run_path(given)?, given.to_string()),
  };

  Ok(Launch::Script(Script {
    path,
    name,
    args: args.to_vec(),
  }))
}

/// The file `zuri run <path>` launches: the script itself, or the
/// entrypoint of the directory it names.
fn resolve_run_path(given: &str) -> Result<PathBuf, LaunchError> {
  let path = Path::new(given);

  if path.is_dir() {
    return entrypoint_of(path, given);
  }

  if !path.is_file() {
    return Err(LaunchError::new(given, "No such file or directory"));
  }

  // The spec launches Zuri scripts, and a file without the extension
  // is not one. Reading it anyway would report a parse error on a file
  // nobody claimed was Zuri in the first place.
  match path.extension().and_then(|e| e.to_str()) {
    Some(SCRIPT_EXT) => Ok(path.to_path_buf()),
    _ => Err(LaunchError::new(given, "Not a Zuri script")),
  }
}

/// A directory's `index.zu`, or the no-entrypoint failure.
fn entrypoint_of(dir: &Path, name: &str) -> Result<PathBuf, LaunchError> {
  let entry = dir.join(ENTRY_FILE);

  match entry.is_file() {
    true => Ok(entry),
    false => Err(LaunchError::new(name, "No entrypoint found in the directory")),
  }
}

/// `zuri <command> [args...]`.
///
/// The command directory beside the runtime is searched first, then
/// the project's. Everything after the command name belongs to it,
/// flags included.
fn resolve_command(name: &str, rest: &[String]) -> Result<Launch, LaunchError> {
  if !is_command_name(name) {
    return Err(LaunchError::new(name, "Unknown command"));
  }

  for dir in command_dirs() {
    if let Some(path) = command_in(&dir, name) {
      return Ok(Launch::Script(Script {
        path,
        name: name.to_string(),
        args: rest.to_vec(),
      }));
    }
  }

  Err(LaunchError::new(name, "Unknown command"))
}

/// The command a directory holds under `name`, if it holds one at all.
///
/// A `<name>` subdirectory with an entrypoint is the command. Without
/// one it is not a command at all, so a plain `<name>.zu` beside it
/// still answers, and a directory that is neither leaves the name to
/// the next place commands are looked for.
fn command_in(dir: &Path, name: &str) -> Option<PathBuf> {
  let entry = dir.join(name).join(ENTRY_FILE);

  if entry.is_file() {
    return Some(entry);
  }

  let script = dir.join(format!("{name}.{SCRIPT_EXT}"));

  match script.is_file() {
    true => Some(script),
    false => None,
  }
}

/// Where commands are looked for, in the order they win.
///
/// The installed set comes first so that a project cannot shadow a
/// command the runtime ships, and the project's `.zuri/cmds` follows
/// for the commands a checkout carries of its own.
pub fn command_dirs() -> Vec<PathBuf> {
  let mut dirs = Vec::new();

  if let Some(installed) = install_root_cmds() {
    dirs.push(installed);
  }

  if let Ok(cwd) = std::env::current_dir() {
    dirs.push(cwd.join(".zuri").join(COMMAND_DIR));
  }

  dirs
}

/// `$ZURI_ROOT/cmds`, falling back to a `cmds` directory beside the
/// running executable. The same pair of locations the standard library
/// resolves through, so a checkout and an install behave alike.
fn install_root_cmds() -> Option<PathBuf> {
  if let Ok(root) = std::env::var("ZURI_ROOT") {
    return Some(PathBuf::from(root).join(COMMAND_DIR));
  }

  std::env::current_exe()
    .ok()
    .and_then(|p| p.parent().map(|p| p.join(COMMAND_DIR)))
}

/// Whether `name` can name a command at all.
///
/// A command is one ordinary path segment. Anything carrying a
/// separator, a drive prefix or a `..` is a path the user typed where a
/// command was expected, and resolving it would reach outside the
/// command directory entirely.
fn is_command_name(name: &str) -> bool {
  let mut parts = Path::new(name).components();

  matches!(parts.next(), Some(Component::Normal(_))) && parts.next().is_none()
}

static SCRIPT_ARGS: OnceLock<Vec<String>> = OnceLock::new();

/// Publishes the argument list `os.args` reports.
///
/// It keeps the shape the executable has always handed the language:
/// the runtime, then the script, then the script's own arguments. A
/// program reading `os.args[2,]` sees exactly what was meant for it,
/// whether it was launched through `run` or as a command.
pub fn set_script_args(args: Vec<String>) {
  let _ = SCRIPT_ARGS.set(args);
}

/// What `os.args` reports.
///
/// Falls back to the raw process arguments for anything embedding the
/// runtime without going through the executable's own launch path.
pub fn script_args() -> Vec<String> {
  match SCRIPT_ARGS.get() {
    Some(args) => args.clone(),
    None => std::env::args().collect(),
  }
}
