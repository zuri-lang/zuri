use std::sync::LazyLock;

use crate::{builtins::to_string, vm::object::NativeFunction};

/// Fallback for a receiver with no primitive `Kind` at all (in
/// practice: an instance) whose class doesn't declare its own
/// override. Kept as one static rather than duplicated in every table.
pub static OBJECT_TO_STRING: LazyLock<NativeFunction> = LazyLock::new(|| NativeFunction {
  name: "to_string",
  min_arity: 1,
  variadic: false,
  is_method: true,
  func: to_string,
});
