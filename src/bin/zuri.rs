use mimalloc::MiMalloc;
use minus::{self, Pager};
use std::io::ErrorKind;
use std::path::Path;
use std::rc::Rc;
use std::{env, fs, process};
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

  println!(
    "Zuri {} (running on ZuriVM {}), REPL/Interactive mode = ON",
    env!("ZURI_VERSION"),
    env!("ZVM_VERSION")
  );
  println!("Build No. => {}", env!("ZURI_BUILD_TIME"));
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

/// Resolves the launch target exactly like the original C implementation:
/// a directory runs its own `index.zu` if present, otherwise aborts with
/// "No entrypoint found in the directory"; anything else is read as-is,
/// with a real file-system error also aborting via `abort_launch` rather
/// than panicking.
fn run_file(vm: &mut VM, file: &str) {
  let path = Path::new(file);
  let resolved = if path.is_dir() {
    let entry = path.join("index.zu");
    if !entry.is_file() {
      abort_launch(file, "No entrypoint found in the directory");
    }
    entry
  } else {
    path.to_path_buf()
  };

  let content = match fs::read_to_string(&resolved) {
    Ok(content) => content,
    Err(e) => abort_launch(file, &io_error_reason(&e)),
  };

  // Canonicalize so stack traces show a full, unambiguous path,
  // matching the target format; falls back to the given (possibly
  // relative) path if that fails for any reason.
  let display_path: Rc<str> = Rc::from(
    zuri::builtins::file::canonical_path(&resolved.to_string_lossy())
      .unwrap_or_else(|_| resolved.display().to_string()),
  );

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

fn main() {
  // zuri::compiler::compiler_test::run_test();
  // zuri::vm::vm_test::run_test();

  let args = env::args().collect::<Vec<_>>();

  let heap = Heap::new();
  let mut vm = VM::new(heap);
  vm.init();

  if args.len() > 1 {
    run_file(&mut vm, &args[1]);
  } else {
    run_repl(&mut vm);
  }
}
