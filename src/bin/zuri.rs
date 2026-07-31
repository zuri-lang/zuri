use std::rc::Rc;
use std::{env, fs, process};
use zuri::compiler::parser::ParserError;
use zuri::compiler::token::KEYWORD_TOKENS;

use itertools::Itertools;
use zuri::compiler::{compiler::Compiler, lexer::Lexer, parser::Parser};
use zuri::vm::vm::VM;
use zuri::vm::{chunk::Chunk, object::Heap};

use crate::shared::repl::Repl;

mod shared;

fn print_repl_help() {
  println!("Press <tab> for autocomplete suggestions");
}

fn format_parse_errors(errors: &[ParserError], path: &str) -> String {
  errors
    .iter()
    .map(|e| format!("{}\n  {}:{}", e, path, e.line_number))
    .join("\n")
}

fn run_repl(vm: &mut VM) {
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
      }

      // Evaluate the line using the persistent VM and heap references
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
          eprintln!("{}", vm.format_uncaught(e));
        }
      },
      Err(errors) => eprintln!("{}", format_parse_errors(&errors, "<repl>")),
    };

    parser.errors.clear();
    vm.clear_frames();

    res
  } else {
    let errors = parser.errors.clone();
    parser.errors.clear();
    return Err(format_parse_errors(&errors, "<repl>"));
  }
  Ok(())
}

fn run_file(vm: &mut VM, file: &str) {
  let content = fs::read_to_string(file).expect("Should have been able to read the file");

  // Canonicalize so stack traces show a full, unambiguous path,
  // matching the target format -- falls back to the given (possibly
  // relative) path if that fails for any reason.
  let display_path: Rc<str> = Rc::from(
    fs::canonicalize(file)
      .map(|p| p.display().to_string())
      .unwrap_or_else(|_| file.to_string()),
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

        if let Err(e) = result {
          eprintln!("{}", vm.format_uncaught(e));
          process::exit(1);
        }
      },
      Err(errors) => {
        eprintln!("{}", format_parse_errors(&errors, &display_path));
        process::exit(1);
      },
    }
  } else {
    eprintln!("{}", format_parse_errors(&parser.errors, &display_path));
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
