use std::collections::HashMap;
use std::io::Write;
use std::{env, fs, io, process};

use itertools::Itertools;
use zuri::compiler::{compiler::Compiler, lexer::Lexer, parser::Parser};
use zuri::vm::vm::VM;
use zuri::vm::{
  chunk::Chunk,
  object::{Heap, Obj},
};

fn run_repl() {
  println!(
    "Zuri {} (running on ZuriVM {}), REPL/Interactive mode = ON",
    env!("ZURI_VERSION"),
    env!("ZVM_VERSION")
  );
  println!("Build No. => {}", env!("ZURI_BUILD_TIME"));
  println!("Type \".exit\" to quit or \".credits\" for more information");

  let stdin = io::stdin();
  let mut input = String::new();

  let mut heap = Heap::new();
  let globals = HashMap::new();
  let mut vm = VM::new(&mut heap, globals);

  loop {
    print!("> ");
    io::stdout().flush().expect("Failed to flush stdout");

    input.clear();
    if stdin.read_line(&mut input).expect("Failed to read line") == 0 {
      // End of file (Ctrl+D)
      println!();
      break;
    }

    let line = input.trim();
    if line.is_empty() {
      continue;
    }

    if line.eq(".exit") {
      break;
    }

    // 2. Evaluate the line using the persistent VM and heap references
    if let Err(e) = evaluate_line(line, &mut vm) {
      eprintln!("Error: {}", e);
    }
  }
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
    // println!("{}", fn_obj.chunk);
    let closure = heap.alloc_plain_closure(fn_obj);

    let main_ptr = match unsafe { &*closure.as_obj() } {
      Obj::Closure(c) => c as *const _,
      _ => unreachable!(),
    };

    if let Err(e) = vm.run(main_ptr) {
      return Err(format!("runtime error: {}", e));
    }
  } else {
    return Err(format!(
      "parse error: {}",
      parser.errors.iter().map(|f| f.to_string()).join("\n")
    ));
  }
  Ok(())
}

fn run_file(file: &str) {
  let content = fs::read_to_string(file).expect("Should have been able to read the file");

  let globals = HashMap::new();

  let mut lex = Lexer::new(&content);
  let mut parser = Parser::new(&mut lex);
  if let Ok(tokens) = parser.parse() {
    // println!("{:?}", tokens);
    let mut heap = Heap::new();
    let chunk = Box::new(Chunk::new());
    let compiler = Compiler::new(tokens, chunk, &mut heap);

    let fn_obj = compiler.compile();
    let closure = heap.alloc_plain_closure(fn_obj);

    let mut vm = VM::new(&mut heap, globals);
    let main_ptr = match unsafe { &*closure.as_obj() } {
      Obj::Closure(c) => c as *const _,
      _ => unreachable!(),
    };

    if let Err(e) = vm.run(main_ptr) {
      eprintln!("runtime error: {}", e);
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
  } else if args.len() == 2 {
    run_file(&args[1]);
  } else {
    run_repl();
  }
}
