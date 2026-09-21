//! What a `zuri` command line means, worked out before anything is
//! read or compiled.
//!
//! The executable has three shapes. With nothing after it, it is the
//! REPL. With `run`, it launches a script, a package directory, or the
//! working directory's own entrypoint. With anything else, that word
//! names a command, and commands live in a `cmds` directory beside the
//! installed runtime or in the project's `.zuri` directory.
//!
//! `--version` and `--help` sit outside all three: they report what the
//! runtime is and what it can reach, and run nothing.

use std::collections::BTreeSet;
use std::fs;
use std::io::Read;
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

/// The flags that report what the runtime is instead of running
/// anything.
const VERSION_FLAGS: [&str; 2] = ["--version", "-v"];

/// The flags that list what the runtime can reach instead of running
/// anything.
const HELP_FLAGS: [&str; 2] = ["--help", "-h"];

/// The tag a command's doc block names itself with.
const COMMAND_TAG: &str = "@command";

/// The tag a command's doc block describes itself with.
const DESCRIPTION_TAG: &str = "@description";

/// How much of a command's source is read looking for its doc block. A
/// command declares itself at the top of the file or not at all.
const DOC_SCAN_BYTES: u64 = 16 * 1024;

/// A resolved command line.
pub enum Launch {
  /// Nothing to run: drop the user into the interactive prompt.
  Repl,
  /// Report what the runtime is and stop.
  Version,
  /// List what the runtime can reach and stop.
  Help,
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
    Some(first) if VERSION_FLAGS.contains(&first.as_str()) => Ok(Launch::Version),
    Some(first) if HELP_FLAGS.contains(&first.as_str()) => Ok(Launch::Help),
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
    false => Err(LaunchError::new(
      name,
      "No entrypoint found in the directory",
    )),
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
  [installed_command_dir(), project_command_dir()]
    .into_iter()
    .flatten()
    .collect()
}

/// `$ZURI_ROOT/cmds`, falling back to a `cmds` directory beside the
/// running executable. The same pair of locations the standard library
/// resolves through, so a checkout and an install behave alike.
fn installed_command_dir() -> Option<PathBuf> {
  if let Ok(root) = std::env::var("ZURI_ROOT") {
    return Some(PathBuf::from(root).join(COMMAND_DIR));
  }

  std::env::current_exe()
    .ok()
    .and_then(|p| p.parent().map(|p| p.join(COMMAND_DIR)))
}

/// The `cmds` directory this project carries of its own.
fn project_command_dir() -> Option<PathBuf> {
  std::env::current_dir()
    .ok()
    .map(|cwd| cwd.join(".zuri").join(COMMAND_DIR))
}

/// One command a listing can name.
pub struct Command {
  /// What the user types.
  pub name: String,
  /// The one line it says about itself, empty when it says nothing.
  pub description: String,
}

/// Every command that can be invoked from here, split by where it came
/// from.
pub struct Commands {
  /// The commands the runtime ships.
  pub global: Vec<Command>,
  /// The commands this project carries.
  pub local: Vec<Command>,
}

impl Commands {
  /// Whether there is anything at all to list.
  pub fn is_empty(&self) -> bool {
    self.global.is_empty() && self.local.is_empty()
  }

  /// The width the name column needs to hold every name in both sets.
  pub fn name_width(&self) -> usize {
    self
      .global
      .iter()
      .chain(self.local.iter())
      .map(|command| command.name.len())
      .max()
      .unwrap_or(0)
  }
}

/// Every command available from here, each described by its own doc
/// block.
///
/// A shipped command owns its name outright, so a project command it
/// hides is left out: the listing names what can actually be run.
pub fn list_commands() -> Commands {
  let global = installed_command_dir()
    .map(|dir| commands_in(&dir))
    .unwrap_or_default();

  let mut local = project_command_dir()
    .map(|dir| commands_in(&dir))
    .unwrap_or_default();

  local.retain(|command| !global.iter().any(|shipped| shipped.name == command.name));

  Commands { global, local }
}

/// The commands one directory holds, in the order they are listed.
fn commands_in(dir: &Path) -> Vec<Command> {
  let Ok(entries) = fs::read_dir(dir) else {
    return Vec::new();
  };

  // A name can be spelled by both a directory and a file beside it, so
  // the set collapses the pair; sorted, so two runs list the same way.
  let mut names = BTreeSet::new();

  for entry in entries.flatten() {
    if let Some(name) = command_name_of(&entry.path()) {
      names.insert(name);
    }
  }

  names
    .into_iter()
    .filter_map(|name| {
      // Through the same lookup that dispatch uses, so a listing can
      // never name a command that would not run, or describe one by a
      // file that would not be the one executed.
      let path = command_in(dir, &name)?;
      let description = description_of(&path);

      Some(Command { name, description })
    })
    .collect()
}

/// The command a `cmds` entry could spell: a directory under its own
/// name, or a script under its stem. `None` for anything else, and for
/// a name no command line could ever reach.
fn command_name_of(path: &Path) -> Option<String> {
  let name = match path.is_dir() {
    true => path.file_name(),
    false => match path.extension().and_then(|e| e.to_str()) {
      Some(SCRIPT_EXT) => path.file_stem(),
      _ => None,
    },
  };

  name
    .and_then(|raw| raw.to_str())
    .filter(|raw| is_command_name(raw))
    .map(String::from)
}

/// The `@description` a command's doc block carries, empty when it
/// carries none.
fn description_of(path: &Path) -> String {
  let Some(source) = head_of(path) else {
    return String::new();
  };

  let Some(block) = leading_doc_block(&source) else {
    return String::new();
  };

  tag_text(block, DESCRIPTION_TAG).unwrap_or_default()
}

/// The name a command's doc block claims with `@command`.
pub fn declared_command_name(path: &Path) -> Option<String> {
  let source = head_of(path)?;
  let block = leading_doc_block(&source)?;

  tag_text(block, COMMAND_TAG)
}

/// The front of a file, which is as far as a declaration can hide.
fn head_of(path: &Path) -> Option<String> {
  let mut bytes = Vec::new();

  fs::File::open(path)
    .ok()?
    .take(DOC_SCAN_BYTES)
    .read_to_end(&mut bytes)
    .ok()?;

  Some(String::from_utf8_lossy(&bytes).into_owned())
}

/// The inside of the file's first doc block.
///
/// Zuri's block comments nest, so this counts depth rather than
/// stopping at the first `*/`: a block whose example opens a comment of
/// its own would otherwise end early, halfway through what it says.
fn leading_doc_block(source: &str) -> Option<&str> {
  let start = source.find("/**")?;
  let body = &source[start + 3..];
  let bytes = body.as_bytes();

  let mut depth = 1usize;
  let mut i = 0;

  while i + 1 < bytes.len() {
    if bytes[i] == b'/' && bytes[i + 1] == b'*' {
      depth += 1;
      i += 2;
      continue;
    }

    if bytes[i] == b'*' && bytes[i + 1] == b'/' {
      depth -= 1;

      if depth == 0 {
        return Some(&body[..i]);
      }

      i += 2;
      continue;
    }

    i += 1;
  }

  None
}

/// What one `@tag` in a doc block says.
///
/// A tag's text may wrap onto the indented lines below it, which is how
/// anything longer than a line gets written; the next tag, or a blank
/// line, ends it.
fn tag_text(block: &str, tag: &str) -> Option<String> {
  let mut collected: Option<String> = None;

  for line in block.lines() {
    let bare = strip_star(line);
    let text = bare.trim();

    if let Some(rest) = opens_tag(text, tag) {
      collected = Some(rest.to_string());
      continue;
    }

    let Some(text_so_far) = collected.as_mut() else {
      continue;
    };

    if text.is_empty() || text.starts_with('@') || !bare.starts_with(' ') {
      break;
    }

    if !text_so_far.is_empty() {
      text_so_far.push(' ');
    }

    text_so_far.push_str(text);
  }

  collected.map(|text| text.trim().to_string())
}

/// The text after `tag`, when this line opens it rather than merely
/// starting with the same letters.
fn opens_tag<'a>(text: &'a str, tag: &str) -> Option<&'a str> {
  let rest = text.strip_prefix(tag)?;

  match rest.chars().next() {
    None => Some(rest),
    // `@description: text` hangs the colon off the tag name.
    Some(':') => Some(rest[1..].trim_start()),
    Some(c) if c.is_whitespace() => Some(rest.trim_start()),
    Some(_) => None,
  }
}

/// Strips the comment furniture from one line of a doc block, keeping
/// whatever indentation followed it. That indentation is what marks a
/// tag continuation, so it has to survive.
fn strip_star(line: &str) -> &str {
  let text = line.trim_start();

  let Some(rest) = text.strip_prefix('*') else {
    return text;
  };

  rest.strip_prefix(' ').unwrap_or(rest)
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
