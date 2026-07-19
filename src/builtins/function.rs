#![allow(unused)]

use std::sync::LazyLock;

use crate::{
  builtins::{MethodTable, build, method, to_string},
  vm::{object::ZuriContext, value::Value},
};

pub static FUNCTION_METHODS: LazyLock<MethodTable> = LazyLock::new(|| {
  build(vec![
    // methods
    method("to_string", to_string),
  ])
});
