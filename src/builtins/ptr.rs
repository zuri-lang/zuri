//! Builtin methods for `Obj::Ptr` -- the generic wrapper natives use
//! to expose an external resource (sqlite, libgd, openssl, ...) as a
//! Zuri value. Deliberately minimal: everything beyond identity/
//! type-checking is specific to whatever module actually wraps the
//! resource (e.g. a future `sqlite` module supplies its OWN methods
//! by looking up `Instr::Invoke`'s target through its own dispatch,
//! same as every other native module here).

use std::sync::LazyLock;

use crate::{
  builtins::{MethodTable, build, enforce::enforce_method_arg_count, method},
  vm::{object::ZuriContext, value::Value},
};

pub static PTR_METHODS: LazyLock<MethodTable> = LazyLock::new(|| {
  build(vec![
    method("to_string", to_string_impl),
    method("ptr_type", ptr_type),
  ])
});

fn to_string_impl(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  let s = format!("{}", ctx.args[0]);
  Ok(ctx.heap().alloc_string(s))
}

/// `.ptr_type()` -- lets Zuri code itself sanity-check what a Ptr
/// wraps before handing it to a native that expects a specific kind
/// (e.g. `if conn.ptr_type() != 'sqlite3_connection' raise TypeError(...)`).
fn ptr_type(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  let name = ctx.args[0].ptr_type_name().unwrap_or("ptr");
  Ok(ctx.heap().alloc_string(name))
}
