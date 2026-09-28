use mimalloc::MiMalloc;
use minus::{self, Pager};
use std::io::{self, ErrorKind};
use std::path::Path;
use std::rc::Rc;
use std::{env, fs, process};
use zuri::cli::{self, Launch, Script};
use zuri::compiler::parser::ParserError;
use zuri::compiler::token::KEYWORD_TOKENS;
use zuri::term::Palette;
use zuri::vm::modules::install_root_libs;

use itertools::Itertools;
use zuri::compiler::{compiler::Compiler, lexer::Lexer, parser::Parser};
use zuri::vm::vm::VM;
use zuri::vm::{chunk::Chunk, object::Heap};

use crate::shared::repl::Repl;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

mod shared;

fn print_repl_help() {
  println!("Press <tab> for autocomplete suggestions");
}

/// What the runtime is. `--version` prints it on its own; the REPL
/// prints the same thing with a note that it is interactive, so the two
/// report the same build in the same shape.
fn print_version(suffix: &str) {
  let palette = Palette::for_stream(&io::stdout());

  println!(
    "{} {}",
    palette
      .accent
      .paint(format!("Zuri {}", env!("ZURI_VERSION"))),
    palette.muted.paint(format!(
      "(running on ZuriVM {}){suffix}",
      env!("ZVM_VERSION")
    ))
  );
  println!(
    "{}",
    palette
      .muted
      .paint(format!("Build No. => {}", env!("ZURI_BUILD_TIME")))
  );
}

fn format_parse_errors(errors: &[ParserError], path: &str, source: &str) -> String {
  errors.iter().map(|e| e.render(path, source)).join("\n\n")
}

fn print_credits() -> Result<(), String> {
  if let Some(libs_root) = install_root_libs() {
    if let Some(license_file) = libs_root.parent().map(|p| p.join("LICENSE")) {
      if let Ok(content) = fs::read_to_string(license_file) {
        let pager = Pager::new();

        #[allow(deprecated)]
        {
          _ = pager.set_exit_strategy(minus::ExitStrategy::PagerQuit);
        }

        if let Ok(()) = pager.push_str(content) {
          let _ = minus::page_all(pager);
          return Ok(());
        }
      }
    }
  }

  Err("Could not load LICENSE file".to_string())
}

fn run_repl(vm: &mut VM) {
  vm.set_repl_mode();
  vm.set_root_path("@.repl.root".to_string());
  vm.init_entry_globals("@.repl");

  let mut repl = Repl::new(
    KEYWORD_TOKENS
      .iter()
      .map(|f| f.to_string().to_lowercase())
      .collect::<Vec<_>>(),
  );

  print_version(", REPL/Interactive mode = ON");
  println!("Type \".exit\" to quit, \".help\" for help or \".credits\" for more information");

  // let stdin = io::stdin();
  // let mut input = String::new();

  repl.run(vm, |vm, buffer| {
    if !buffer.is_empty() {
      if buffer.eq(".exit") {
        return Err(());
      } else if buffer.eq(".help") {
        print_repl_help();
        return Ok(());
      } else if buffer.eq(".credits") {
        if let Err(message) = print_credits() {
          eprintln!("{}", message);
        }
        return Ok(());
      }

      if let Err(e) = evaluate_line(&buffer, vm) {
        eprintln!("{}", e);
      }
    }

    Ok(())
  });
}

fn evaluate_line(line: &str, vm: &mut VM) -> Result<(), String> {
  let mut lex = Lexer::new(line);
  let mut parser = Parser::new(&mut lex);
  if let Ok(decls) = parser.parse() {
    let chunk = Box::new(Chunk::new());
    let mut compiler = Compiler::new(decls, chunk, &mut vm.heap, Rc::from("<repl>"));
    compiler.enable_repl_mode();

    let res = match compiler.compile() {
      Ok(fn_obj) => {
        let closure = vm.heap.alloc_plain_closure(fn_obj);
        if let Err(e) = vm.run(closure) {
          eprintln!("{}", vm.format_uncaught(e, "<repl>", line));
        }
      },
      Err(errors) => eprintln!("{}", format_parse_errors(&errors, "<repl>", line)),
    };

    parser.errors.clear();
    vm.clear_frames();

    res
  } else {
    let errors = parser.errors.clone();
    parser.errors.clear();
    return Err(format_parse_errors(&errors, "<repl>", line));
  }
  Ok(())
}

/// Matches the original C implementation's launch-failure format exactly:
/// ```text
/// (Zuri):
///   Launch aborted for test.zu
///   Reason: No such file or directory
/// ```
fn abort_launch(name: &str, reason: &str) -> ! {
  eprintln!("(Zuri):\n  Launch aborted for {name}\n  Reason: {reason}");
  process::exit(1);
}

/// `std::io::Error`'s own `Display` renders `NotFound` as "No such file
/// or directory (os error 2)" on Linux; strip the OS-specific suffix
/// so it matches the C implementation's message exactly.
fn io_error_reason(e: &std::io::Error) -> String {
  match e.kind() {
    ErrorKind::NotFound => "No such file or directory".to_string(),
    ErrorKind::PermissionDenied => "Permission denied".to_string(),
    _ => e.to_string(),
  }
}

/// Compiles and runs one already-resolved script. Everything about
/// which file that is was settled before the VM was built; all that is
/// left here is a read that can still fail on permissions, and the
/// compile and run themselves.
///
/// `name` is what the user typed, so a failure names the path or the
/// command they used rather than whatever it resolved to.
fn run_script(vm: &mut VM, path: &Path, name: &str, display_path: Rc<str>) {
  let content = match fs::read_to_string(path) {
    Ok(content) => content,
    Err(e) => abort_launch(name, &io_error_reason(&e)),
  };

  vm.set_root_path(display_path.to_string());
  vm.init_entry_globals(&display_path);

  let mut lex = Lexer::new(&content);
  let mut parser = Parser::new(&mut lex);
  if let Ok(decls) = parser.parse() {
    let chunk = Box::new(Chunk::new());
    let compiler = Compiler::new(decls, chunk, &mut vm.heap, display_path.clone());

    match compiler.compile() {
      Ok(fn_obj) => {
        let closure = vm.heap.alloc_plain_closure(fn_obj);
        let result = vm.run(closure);

        #[cfg(feature = "opcode-profile")]
        if std::env::var_os("ZURI_OPCODE_PROFILE").is_some() {
          vm.dump_opcode_profile();
        }

        if std::env::var_os("ZURI_JIT_COVERAGE").is_some() {
          vm.dump_jit_coverage();
        }

        if let Err(e) = result {
          eprintln!("{}", vm.format_uncaught(e, &display_path, &content));
          process::exit(1);
        }

        // The program is over: its exit handlers have run and its output
        // is flushed. Dropping the VM from here would only free memory
        // the process is about to give back anyway, and would first wait
        // for any compile still reading the heap, which can outlast the
        // program by any amount. Exiting here also takes down the
        // compiler threads and isolate workers where they stand.
        process::exit(vm.take_exit_code().unwrap_or(0));
      },
      Err(errors) => {
        eprintln!("{}", format_parse_errors(&errors, &display_path, &content));
        process::exit(1);
      },
    }
  } else {
    eprintln!(
      "{}",
      format_parse_errors(&parser.errors, &display_path, &content)
    );
    process::exit(1);
  }
}

/// The top-level help: what this is, how to invoke it, and every
/// command it can reach from where it was run.
///
/// Laid out the way `args.Parser` lays out a command's own help, so the
/// two read as the same program talking.
fn print_help() {
  let palette = Palette::for_stream(&io::stdout());

  print_version("");

  println!();
  println!(
    "{} {}                    start the interactive REPL",
    palette.heading.paint("Usage:"),
    palette.accent.paint("zuri")
  );
  println!(
    "       {} run [PATH]         run a script, a package, or this directory",
    palette.accent.paint("zuri")
  );
  println!(
    "       {} <command> [ARGS]   run a command",
    palette.accent.paint("zuri")
  );
  println!();
  println!("{}", palette.heading.paint("OPTIONS:"));
  println!("  -h, --help     Show this help message and exit");
  println!("  -v, --version  Show version information and exit");

  let commands = cli::list_commands();
  let width = commands.name_width();

  print_commands("COMMANDS", &commands.global, width, &palette);
  print_commands("PROJECT COMMANDS", &commands.local, width, &palette);
  print_commands("PACKAGE COMMANDS", &commands.packaged, width, &palette);

  if !commands.is_empty() {
    println!();
    println!(
      "{}",
      palette
        .muted
        .paint("Run \"zuri <command> --help\" for help on a specific command.")
    );
  }
}

/// One headed run of commands, left out entirely when there are none
/// to put under it.
fn print_commands(title: &str, commands: &[cli::Command], width: usize, palette: &Palette) {
  if commands.is_empty() {
    return;
  }

  println!();
  println!("{}", palette.heading.paint(format!("{title}:")));

  for command in commands {
    let origin = match command.packages.len() {
      0 => String::new(),
      1 => format!(" (from {})", command.packages[0]),
      _ => format!(" (claimed by {})", command.packages.join(", ")),
    };

    let origin = palette.muted.paint(origin);

    match command.description.is_empty() {
      true => println!("  {}{origin}", command.name),
      false => println!("  {:width$}  {}{origin}", command.name, command.description),
    }
  }
}

/// The path a stack trace shows: full and unambiguous, falling back to
/// the path as given if it cannot be canonicalized.
fn display_path_of(path: &Path) -> Rc<str> {
  Rc::from(
    zuri::builtins::file::canonical_path(&path.to_string_lossy())
      .unwrap_or_else(|_| path.display().to_string()),
  )
}

fn main() {
  let argv = env::args().collect::<Vec<_>>();

  clear_replaced_executable();
  keep_standard_handles_private();

  let bundle = match zuri::bundle::detect() {
    Ok(bundle) => bundle,
    Err(reason) => abort_launch(&program_name(&argv), &reason),
  };

  let launch = match bundle {
    Some(bundle) => bundled_launch(bundle, &argv),
    None => match cli::resolve(argv.get(1..).unwrap_or_default()) {
      Ok(launch) => launch,
      Err(e) => abort_launch(&e.name, &e.reason),
    },
  };

  // Settled before the VM exists, because building it is already
  // enough to have a module ask what `os.args` holds.
  let script = match launch {
    Launch::Version => {
      print_version("");
      return;
    },
    Launch::Help => {
      print_help();
      return;
    },
    Launch::Repl => {
      if let Ok(cwd) = env::current_dir() {
        zuri::project::set_anchor(cwd);
      }

      None
    },
    Launch::Script(script) => {
      zuri::project::set_anchor(script.anchor.clone());

      let display_path = display_path_of(&script.path);
      cli::set_script_args(os_args(&argv, &display_path, &script));

      Some((script, display_path))
    },
  };

  // Never dropped, on any path out of here, a panic included. Compiler
  // workers read functions straight out of this heap, and nothing waits
  // for them to finish, so the heap has to last as long as the process.
  let mut vm = std::mem::ManuallyDrop::new(VM::new(Heap::new()));
  vm.init();

  match script {
    Some((script, display_path)) => run_script(&mut vm, &script.path, &script.name, display_path),
    None => run_repl(&mut vm),
  }
}

/// What this executable was invoked as, for a failure that happens
/// before there is any script to name.
fn program_name(argv: &[String]) -> String {
  argv
    .first()
    .map(|arg0| {
      Path::new(arg0)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| arg0.clone())
    })
    .unwrap_or_else(|| "zuri".to_string())
}

/// A bundled application: every argument belongs to it, and none of
/// zuri's own dispatch applies. The standard library and the modules
/// come from the bundle alone.
fn bundled_launch(bundle: zuri::bundle::Bundle, argv: &[String]) -> Launch {
  let entry = bundle.entry();
  let name = program_name(argv);

  if !entry.is_file() {
    abort_launch(&name, "The bundle holds no application to run");
  }

  zuri::project::set_bundle_root(bundle.root.clone());

  Launch::Script(Script {
    path: entry,
    anchor: bundle.app_dir(),
    name,
    args: argv.get(1..).unwrap_or_default().to_vec(),
  })
}

/// Removes the executable an upgrade on Windows set aside. A running
/// executable cannot be deleted there, only renamed, so `zuri upgrade`
/// leaves it as `zuri.exe.old` and the next start clears it away.
#[cfg(windows)]
fn clear_replaced_executable() {
  let Ok(exe) = env::current_exe() else {
    return;
  };

  let Some(name) = exe.file_name().map(|n| n.to_string_lossy().into_owned()) else {
    return;
  };

  let old = exe.with_file_name(format!("{name}.old"));

  if old.is_file() {
    let _ = fs::remove_file(old);
  }
}

#[cfg(not(windows))]
fn clear_replaced_executable() {}

/// Keeps the standard handles this process was started with out of the
/// processes it starts.
///
/// Windows hands every inheritable handle a process holds to every
/// child it creates, whatever that child's own streams are. The
/// standard handles are inheritable, since that is how they arrived, so
/// a program started with its output sent elsewhere would still hold
/// this process's output pipe open for as long as it ran, and whoever
/// reads that pipe would wait for it. A child told to inherit a stream
/// is given its own inheritable copy when it is started, so clearing
/// the flag on the originals takes nothing from it.
#[cfg(windows)]
fn keep_standard_handles_private() {
  use windows_sys::Win32::Foundation::{
    HANDLE_FLAG_INHERIT, INVALID_HANDLE_VALUE, SetHandleInformation,
  };
  use windows_sys::Win32::System::Console::{
    GetStdHandle, STD_ERROR_HANDLE, STD_INPUT_HANDLE, STD_OUTPUT_HANDLE,
  };

  for stream in [STD_INPUT_HANDLE, STD_OUTPUT_HANDLE, STD_ERROR_HANDLE] {
    unsafe {
      let handle = GetStdHandle(stream);

      if !handle.is_null() && handle != INVALID_HANDLE_VALUE {
        SetHandleInformation(handle, HANDLE_FLAG_INHERIT, 0);
      }
    }
  }
}

#[cfg(not(windows))]
fn keep_standard_handles_private() {}

/// The list `os.args` reports: the runtime, the script, then the
/// script's own arguments. Nothing of zuri's own dispatch survives
/// into it, so `run` and a command hand a program the same shape.
fn os_args(argv: &[String], display_path: &str, script: &Script) -> Vec<String> {
  let mut args = Vec::with_capacity(script.args.len() + 2);

  args.push(argv.first().cloned().unwrap_or_else(|| "zuri".to_string()));
  args.push(display_path.to_string());
  args.extend(script.args.iter().cloned());

  args
}
