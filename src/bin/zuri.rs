use std::fs;

fn main() {
  // zuri::compiler::compiler_test::run_test();
  // zuri::vm::vm_test::run_test();

  let content = fs::read_to_string("sample.zu").expect("Should have been able to read the file");

  let mut lex = zuri::compiler::lexer::Lexer::new(&content);
  let mut parser = zuri::compiler::parser::Parser::new(&mut lex);
  if let Ok(tokens) = parser.parse() {
    // println!("{:?}", tokens);
    let mut heap = zuri::vm::object::Heap::new();
    let mut compiler = zuri::compiler::compiler::Compiler::new(tokens, &mut heap);

    let fn_obj = compiler.compile();

    let mut vm = zuri::vm::vm::VM::new(heap);

    if let Err(e) = vm.run(&fn_obj) {
      eprintln!("runtime error: {}", e);
      std::process::exit(1);
    }
  } else {
    for error in parser.errors {
      println!("ParseError: {}", error);
    }
  }
}
