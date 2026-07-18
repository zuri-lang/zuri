//! Built-in Exception class hierarchy, defined as ordinary Zuri source
//! and compiled + run once at VM startup (see `install`) rather than
//! hand-built in Rust -- reuses the exact same class/inheritance
//! machinery every user-defined class goes through, so Exception and
//! its subclasses behave identically to anything a Zuri program could
//! write itself, including being subclassable (see
//! tests/custom_exception.zu's `class Error < Exception`).

use std::rc::Rc;

use crate::compiler::{compiler::Compiler, lexer::Lexer, parser::Parser};
use crate::vm::chunk::Chunk;
use crate::vm::value::Value;
use crate::vm::vm::VM;

const PRELUDE_SOURCE: &str = r#"
class Exception {
  var message = 'An unexpected error has occurred'
  var stacktrace = []
  var type = 'Exception'

  Exception(message) {
    self.message = message or self.message
  }
}

class TypeError < Exception {
  TypeError(message) {
    self.message = message or self.message
    self.type = 'TypeError'
  }
}

class ValueError < Exception {
  ValueError(message) {
    self.message = message or self.message
    self.type = 'ValueError'
  }
}

class NumericError < Exception {
  NumericError(message) {
    self.message = message or self.message
    self.type = 'NumericError'
  }
}

class ArgumentError < Exception {
  ArgumentError(message) {
    self.message = message or self.message
    self.type = 'ArgumentError'
  }
}

class NotImplementedError < Exception {
  NotImplementedError(message) {
    self.message = message or self.message
    self.type = 'NotImplementedError'
  }
}

class RangeError < Exception {
  RangeError(message) {
    self.message = message or self.message
    self.type = 'RangeError'
  }
}

class AccessError < Exception {
  AccessError(message) {
    self.message = message or self.message
    self.type = 'AccessError'
  }
}

class AssertError < Exception {
  AssertError(message) {
    self.message = message or self.message
    self.type = 'AssertError'
  }
}

class PropertyError < Exception {
  PropertyError(message) {
    self.message = message or self.message
    self.type = 'PropertyError'
  }
}

class UndefinedError < Exception {
  UndefinedError(message) {
    self.message = message or self.message
    self.type = 'UndefinedError'
  }
}
"#;

/// Names of every builtin exception class, in declaration order (each
/// subclasses `Exception`, so it has to already be bound as a global by
/// the time its subclasses compile -- matching `PRELUDE_SOURCE`'s own
/// ordering).
pub const EXCEPTION_CLASS_NAMES: &[&str] = &[
  "Exception",
  "TypeError",
  "ValueError",
  "NumericError",
  "ArgumentError",
  "NotImplementedError",
  "RangeError",
  "AccessError",
  "AssertError",
  "PropertyError",
  "UndefinedError",
];

/// Compile and run `PRELUDE_SOURCE` against `vm`, then cache each
/// resulting class Value by name in `vm.builtin_exceptions` for
/// `VM::raise`'s fast lookup. Panics on any failure -- a broken prelude
/// is an internal bug, not a user-facing error, so there is no
/// meaningful way to recover from it (and no user code has run yet to
/// have anything at stake).
pub fn install(vm: &mut VM) {
  let mut lexer = Lexer::new(PRELUDE_SOURCE);
  let mut parser = Parser::new(&mut lexer);
  let decls = parser
    .parse()
    .unwrap_or_else(|errors| panic!("internal error: prelude failed to parse: {:?}", errors));

  let chunk = Box::new(Chunk::new());
  let compiler = Compiler::new(decls, chunk, &mut vm.heap, Rc::from("<prelude>"));
  let fn_obj = compiler.compile();
  let closure = vm.heap.alloc_plain_closure(fn_obj);

  if let Err(e) = vm.run(closure) {
    panic!(
      "internal error: prelude raised an exception: {}",
      vm.describe_exception(e)
    );
  }

  for &name in EXCEPTION_CLASS_NAMES {
    let class_val: Value = vm
      .lookup_global(name)
      .unwrap_or_else(|| panic!("internal error: prelude did not define '{}'", name));
    vm.builtin_exceptions.insert(name, class_val);
  }
}
