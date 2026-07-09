use std::fs;

use crate::compiler::{lexer, parser};

pub fn run_test() {
  let contents = fs::read_to_string("sample.zu").expect("Should have been able to read the file");

  let mut lex = lexer::Lexer::new(&contents);
  let mut parser = parser::Parser::new(&mut lex);
  if let Ok(tokens) = parser.parse() {
    for el in tokens.iter() {
      println!("{:?}", el);
    }
  } else {
    for error in parser.errors {
      println!("ParseError: {}", error);
    }
  }
}
