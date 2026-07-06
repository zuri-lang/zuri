use std::fs;

use zuri::{lexer, parser};

fn main() {
  let contents = fs::read_to_string("ast_gen.zu").expect("Should have been able to read the file");

  let mut lex = lexer::Lexer::new(&contents);

  for error in lex.clone().errors {
    println!("LexError: {}", error);
  }

  let mut parser = parser::Parser::new(&mut lex);
  if let Ok(tokens) = parser.parse() {
    for el in tokens.iter() {
      println!("{:?}", el);
    }
  } else {
    for error in parser.lex_errors {
      println!("LexerError: {}", error);
    }
    for error in parser.errors {
      println!("ParseError: {}", error);
    }
  }
}
