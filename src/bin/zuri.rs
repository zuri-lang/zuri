use std::collections::HashMap;
use std::{env, fs, process};
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
        eprintln!("Error: {}", e);
      }
    }

    Ok(())
  });
}

fn evaluate_line(line: &str, vm: &mut VM) -> Result<(), String> {
  let mut lex = Lexer::new(line);
  let mut parser = Parser::new(&mut lex);
  if let Ok(tokens) = parser.parse() {
    let chunk = Box::new(Chunk::new());

    let heap = vm.heap_mut();
    let mut compiler = Compiler::new(tokens, chunk, heap);
    compiler.enable_repl_mode();

    let fn_obj = compiler.compile();
    let closure = heap.alloc_plain_closure(fn_obj);

    if let Err(e) = vm.run(closure) {
      eprintln!("runtime error: {}", vm.describe_exception(e));
      process::exit(1);
    }
  } else {
    return Err(format!(
      "parse error: {}",
      parser.errors.iter().map(|f| f.to_string()).join("\n  ")
    ));
  }
  Ok(())
}

fn run_file(vm: &mut VM, file: &str) {
  let content = fs::read_to_string(file).expect("Should have been able to read the file");

  let mut lex = Lexer::new(&content);
  let mut parser = Parser::new(&mut lex);
  if let Ok(tokens) = parser.parse() {
    // println!("{:?}", tokens);
    let mut heap = Heap::new();
    let chunk = Box::new(Chunk::new());
    let compiler = Compiler::new(tokens, chunk, &mut heap);

    let fn_obj = compiler.compile();
    let closure = heap.alloc_plain_closure(fn_obj);

    if let Err(e) = vm.run(closure) {
      eprintln!("runtime error: {}", vm.describe_exception(e));
      process::exit(1);
    }
  } else {
    eprintln!(
      "ParseError: {}",
      parser.errors.iter().map(|f| f.to_string()).join("\n  ")
    );
    process::exit(1);
  }
}

fn main() {
  // zuri::compiler::compiler_test::run_test();
  // zuri::vm::vm_test::run_test();

  let args = env::args().collect::<Vec<_>>();

  if args.len() > 2 {
    println!("Usage: zuri <script>");
    process::exit(1);
  } else {
    let heap = Heap::new();
    let globals = HashMap::new();

    let mut vm = VM::new(heap, globals);
    vm.init();

    if args.len() == 2 {
      run_file(&mut vm, &args[1]);
    } else {
      run_repl(&mut vm);
    }
  }
}
