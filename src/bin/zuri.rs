use mimalloc::MiMalloc;
use minus::{self, Pager};
use std::io::ErrorKind;
use std::path::Path;
use std::rc::Rc;
use std::{env, fs, process};
use zuri::cli::{self, Launch, Script};
use zuri::compiler::parser::ParserError;
use zuri::compiler::token::KEYWORD_TOKENS;
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
  println!(
    "Zuri {} (running on ZuriVM {}){suffix}",
    env!("ZURI_VERSION"),
    env!("ZVM_VERSION")
  );
  println!("Build No. => {}", env!("ZURI_BUILD_TIME"));
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

        if let Some(code) = vm.take_exit_code() {
          process::exit(code);
        }
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

  let launch = match cli::resolve(argv.get(1..).unwrap_or_default()) {
    Ok(launch) => launch,
    Err(e) => abort_launch(&e.name, &e.reason),
  };

  // Settled before the VM exists, because building it is already
  // enough to have a module ask what `os.args` holds.
  let script = match launch {
    Launch::Version => {
      print_version("");
      return;
    },
    Launch::Repl => None,
    Launch::Script(script) => {
      let display_path = display_path_of(&script.path);
      cli::set_script_args(os_args(&argv, &display_path, &script));

      Some((script, display_path))
    },
  };

  let heap = Heap::new();
  let mut vm = VM::new(heap);
  vm.init();

  match script {
    Some((script, display_path)) => {
      run_script(&mut vm, &script.path, &script.name, display_path)
    },
    None => run_repl(&mut vm),
  }
}

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
