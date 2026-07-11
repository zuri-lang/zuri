use std::fs;

use zuri::vm::{chunk::Chunk, object::Obj};

fn main() {
  // zuri::compiler::compiler_test::run_test();
  // zuri::vm::vm_test::run_test();

  let content = fs::read_to_string("sample.zu").expect("Should have been able to read the file");

  let mut lex = zuri::compiler::lexer::Lexer::new(&content);
  let mut parser = zuri::compiler::parser::Parser::new(&mut lex);
  if let Ok(tokens) = parser.parse() {
    // println!("{:?}", tokens);
    let heap = Box::new(zuri::vm::object::Heap::new());
    let chunk = Box::new(Chunk::new());
    let compiler = zuri::compiler::compiler::Compiler::new(tokens, chunk, heap);

    let (fn_obj, mut heap) = compiler.compile();
    let closure = heap.alloc_plain_closure(fn_obj);

    let mut vm = zuri::vm::vm::VM::new(heap);
    let main_ptr = match unsafe { &*closure.as_obj() } {
      Obj::Closure(c) => c as *const _,
      _ => unreachable!(),
    };

    if let Err(e) = vm.run(main_ptr) {
      eprintln!("runtime error: {}", e);
      std::process::exit(1);
    }
  } else {
    for error in parser.errors {
      println!("ParseError: {}", error);
    }
  }
}
