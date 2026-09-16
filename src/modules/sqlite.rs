//! `_sqlite`: the native half of `sql.sqlite`.
//!
//! Everything here is a thin, faithful wrapper over SQLite's C API.
//! Policy lives in Zuri: this module does not decide what a NULL means,
//! how a date is stored, or which errors deserve which class. It opens
//! handles, binds values, steps statements and reports result codes,
//! and leaves the rest to `libs/sql/sqlite/`.
//!
//! Handles reach Zuri as `Ptr`s tagged with the constants in
//! `sqlite_util`. Closing one retags it, so a call on a closed handle
//! fails with a type error rather than reaching a freed pointer.

use std::os::raw::{c_char, c_int, c_void};

use libsqlite3_sys as ffi;

use crate::builtins::enforce::{
  ArgType, enforce_method_arg_count, enforce_method_arg_range, enforce_method_arg_type,
};
use crate::modules::sqlite_util::{
  self as util, BACKUP, BACKUP_CLOSED, BLOB, BLOB_CLOSED, CONNECTION, CONNECTION_CLOSED, STATEMENT,
  STATEMENT_CLOSED, SqliteBackup, SqliteBlob, SqliteConn, SqliteStmt, callback, value,
};
use crate::modules::{BuiltinModuleDef, native, optional_bool, optional_number};
use crate::vm::object::ZuriContext;
use crate::vm::value::Value;
use crate::vm::vm::VM;
use crate::{enforce_arg_count, enforce_arg_range, enforce_arg_type};

pub static MODULE: BuiltinModuleDef = BuiltinModuleDef {
  name: "_sqlite",
  build,
};

fn build(vm: &mut VM) -> Vec<(&'static str, Value)> {
  vec![
    // Library information.
    ("libversion", native(vm, "libversion", 0, false, libversion)),
    ("sourceid", native(vm, "sourceid", 0, false, sourceid)),
    ("threadsafe", native(vm, "threadsafe", 0, false, threadsafe)),
    ("errstr", native(vm, "errstr", 1, false, errstr)),
    // Connections.
    ("open", native(vm, "open", 1, true, open)),
    ("close", native(vm, "close", 1, false, close)),
    (
      "busy_timeout",
      native(vm, "busy_timeout", 2, false, busy_timeout),
    ),
    ("interrupt", native(vm, "interrupt", 1, false, interrupt)),
    ("exec", native(vm, "exec", 2, false, exec)),
    ("changes", native(vm, "changes", 1, false, changes)),
    (
      "total_changes",
      native(vm, "total_changes", 1, false, total_changes),
    ),
    (
      "last_insert_rowid",
      native(vm, "last_insert_rowid", 1, false, last_insert_rowid),
    ),
    ("autocommit", native(vm, "autocommit", 1, false, autocommit)),
    ("errcode", native(vm, "errcode", 1, false, errcode)),
    ("errmsg", native(vm, "errmsg", 1, false, errmsg)),
    // Statements.
    ("prepare", native(vm, "prepare", 2, false, prepare)),
    ("finalize", native(vm, "finalize", 1, false, finalize)),
    ("reset", native(vm, "reset", 1, false, reset)),
    (
      "clear_bindings",
      native(vm, "clear_bindings", 1, false, clear_bindings),
    ),
    ("step", native(vm, "step", 1, false, step)),
    ("bind", native(vm, "bind", 3, false, bind)),
    (
      "bind_parameter_count",
      native(vm, "bind_parameter_count", 1, false, bind_parameter_count),
    ),
    (
      "bind_parameter_index",
      native(vm, "bind_parameter_index", 2, false, bind_parameter_index),
    ),
    (
      "bind_parameter_name",
      native(vm, "bind_parameter_name", 2, false, bind_parameter_name),
    ),
    (
      "column_count",
      native(vm, "column_count", 1, false, column_count),
    ),
    (
      "column_names",
      native(vm, "column_names", 1, false, column_names),
    ),
    (
      "column_decltypes",
      native(vm, "column_decltypes", 1, false, column_decltypes),
    ),
    (
      "column_type",
      native(vm, "column_type", 2, false, column_type),
    ),
    (
      "column_value",
      native(vm, "column_value", 2, false, column_value),
    ),
    ("row", native(vm, "row", 1, false, row)),
    (
      "column_origin",
      native(vm, "column_origin", 2, false, column_origin),
    ),
    ("data_count", native(vm, "data_count", 1, false, data_count)),
    ("sql", native(vm, "sql", 1, false, stmt_sql)),
    (
      "expanded_sql",
      native(vm, "expanded_sql", 1, false, expanded_sql),
    ),
    (
      "stmt_readonly",
      native(vm, "stmt_readonly", 1, false, stmt_readonly),
    ),
    ("stmt_busy", native(vm, "stmt_busy", 1, false, stmt_busy)),
    // Incremental blob access.
    ("blob_open", native(vm, "blob_open", 5, true, blob_open)),
    ("blob_bytes", native(vm, "blob_bytes", 1, false, blob_bytes)),
    ("blob_read", native(vm, "blob_read", 3, false, blob_read)),
    ("blob_write", native(vm, "blob_write", 3, false, blob_write)),
    (
      "blob_reopen",
      native(vm, "blob_reopen", 2, false, blob_reopen),
    ),
    ("blob_close", native(vm, "blob_close", 1, false, blob_close)),
    // Online backup.
    (
      "backup_init",
      native(vm, "backup_init", 4, false, backup_init),
    ),
    (
      "backup_step",
      native(vm, "backup_step", 2, false, backup_step),
    ),
    (
      "backup_remaining",
      native(vm, "backup_remaining", 1, false, backup_remaining),
    ),
    (
      "backup_pagecount",
      native(vm, "backup_pagecount", 1, false, backup_pagecount),
    ),
    (
      "backup_finish",
      native(vm, "backup_finish", 1, false, backup_finish),
    ),
    // Extending the engine from Zuri.
    (
      "create_function",
      native(vm, "create_function", 5, false, create_function),
    ),
    (
      "create_aggregate",
      native(vm, "create_aggregate", 5, false, create_aggregate),
    ),
    (
      "delete_function",
      native(vm, "delete_function", 3, false, delete_function),
    ),
    (
      "create_collation",
      native(vm, "create_collation", 3, false, create_collation),
    ),
    (
      "set_update_hook",
      native(vm, "set_update_hook", 2, false, set_update_hook),
    ),
    (
      "set_commit_hook",
      native(vm, "set_commit_hook", 2, false, set_commit_hook),
    ),
    (
      "set_rollback_hook",
      native(vm, "set_rollback_hook", 2, false, set_rollback_hook),
    ),
    (
      "set_authorizer",
      native(vm, "set_authorizer", 2, false, set_authorizer),
    ),
    (
      "set_progress_handler",
      native(vm, "set_progress_handler", 3, false, set_progress_handler),
    ),
  ]
}

/// The raw `sqlite3*` behind argument zero, which every connection
/// native takes as its receiver.
fn db_of(ctx: &ZuriContext) -> *mut ffi::sqlite3 {
  let cell = ctx.args[0].as_ptr_cell().borrow();
  cell.downcast_ref::<SqliteConn>().unwrap().db()
}

/// The raw `sqlite3_stmt*` behind argument zero.
///
/// Copied out rather than borrowed across the call that uses it: a
/// statement native can re-enter Zuri through a user-defined function,
/// and a collection running in there relocates the `Ptr` object this
/// borrow points into. The C pointer itself is SQLite's, outside the
/// Zuri heap, so it stays valid across anything the VM does.
fn stmt_of(ctx: &ZuriContext) -> *mut ffi::sqlite3_stmt {
  let cell = ctx.args[0].as_ptr_cell().borrow();
  cell.downcast_ref::<SqliteStmt>().unwrap().stmt
}

/// The connection state a statement belongs to, for reading back an
/// error a callback parked mid step.
fn stmt_state(ctx: &ZuriContext) -> *mut util::ConnState {
  let cell = ctx.args[0].as_ptr_cell().borrow();
  cell.downcast_ref::<SqliteStmt>().unwrap().state
}

/// The connection's own state pointer, for the natives that drive
/// SQLite through the connection rather than through a statement.
fn conn_state(ctx: &ZuriContext) -> *mut util::ConnState {
  let mut cell = ctx.args[0].as_ptr_cell().borrow_mut();
  cell.downcast_mut::<SqliteConn>().unwrap().state_ptr()
}

/// Re-raises whatever a Zuri callback parked on `state` during the call
/// that just finished, if anything.
///
/// This is where an error raised inside a user-defined function finally
/// surfaces, with the class the program actually raised: the value was
/// rooted when it was trapped, and `rethrow` hands it to `call_native`
/// to re-raise untouched.
fn take_trapped(vm: &mut VM, state: *mut util::ConnState) -> Option<String> {
  if state.is_null() {
    return None;
  }

  // SAFETY: `state` addresses the boxed `ConnState` of a connection
  // that is still open, since the handle that supplied it has not been
  // closed.
  let state = unsafe { &mut *state };
  let trapped = state.trapped.take()?;

  let error = vm.native_root(trapped.slot);
  vm.release_native_root(trapped.slot);

  if error.is_nil() {
    return Some(trapped.message);
  }

  Some(vm.rethrow(error))
}

fn libversion(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 0);

  let text = unsafe { util::cstr_to_string(ffi::sqlite3_libversion()) };
  Ok(ctx.heap().alloc_string(text))
}

fn sourceid(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 0);

  let text = unsafe { util::cstr_to_string(ffi::sqlite3_sourceid()) };
  Ok(ctx.heap().alloc_string(text))
}

/// SQLite's compile-time threading mode: 0 single thread, 1 serialized,
/// 2 multi thread. The bundled build reports 1, which is what makes a
/// connection legal to move between isolates.
fn threadsafe(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 0);

  Ok(Value::number(unsafe { ffi::sqlite3_threadsafe() } as f64))
}

/// SQLite's own description of a result code, for turning a code back
/// into text without a connection to ask.
fn errstr(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 1);
  enforce_arg_type!(ctx, 0, ArgType::Number);

  let text = util::result_code_text(ctx.args[0].as_number() as c_int);
  Ok(ctx.heap().alloc_string(text))
}

/// `open(path, flags, vfs)`.
///
/// `flags` is passed straight to `sqlite3_open_v2`; `sql.sqlite` builds
/// it from the options on a DSN. A nil `vfs` uses the default.
fn open(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_range!(ctx, 1, 3);
  enforce_arg_type!(ctx, 0, ArgType::String);

  let path = util::to_cstring("a database path", ctx.args[0].as_str())?;

  let default_flags = (ffi::SQLITE_OPEN_READWRITE | ffi::SQLITE_OPEN_CREATE) as f64;
  let flags = optional_number(ctx, 1, default_flags)? as c_int;

  let vfs = match ctx.args.get(2) {
    Some(v) if v.is_string() => Some(util::to_cstring("a vfs name", v.as_str())?),
    _ => None,
  };

  let mut db: *mut ffi::sqlite3 = std::ptr::null_mut();

  let code = unsafe {
    ffi::sqlite3_open_v2(
      path.as_ptr(),
      &mut db,
      flags,
      match &vfs {
        Some(name) => name.as_ptr(),
        None => std::ptr::null(),
      },
    )
  };

  if code != ffi::SQLITE_OK {
    // sqlite3_open_v2 hands back a handle even when it fails, purely so
    // the error can be read off it, and that handle still has to be
    // closed.
    let message = if db.is_null() {
      util::format_error(code, &util::result_code_text(code))
    } else {
      let message = util::last_error(db);
      util::close_db(db);
      message
    };

    return Err(message);
  }

  // Extended result codes carry the distinctions that matter for
  // mapping an error onto a class: which constraint failed, which kind
  // of I/O error it was. On from the start so no caller has to ask.
  unsafe { ffi::sqlite3_extended_result_codes(db, 1) };

  Ok(ctx.heap().alloc_ptr(CONNECTION, SqliteConn::new(db)))
}

fn close(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(CONNECTION));

  // The borrow has to end before the VM is touched, and closing
  // releases the connection's callback slots, so the handle is taken
  // out of the `Ptr` first and closed after.
  let mut connection = {
    let mut cell = ctx.args[0].as_ptr_cell().borrow_mut();
    cell.type_name = CONNECTION_CLOSED;

    match cell.take().downcast::<SqliteConn>() {
      Ok(connection) => connection,
      // Already closed, or moved to another isolate; either way there
      // is nothing left here to close.
      Err(_) => return Ok(Value::nil()),
    }
  };

  connection.close(ctx.vm);

  Ok(Value::nil())
}

/// How long a statement waits on a locked database before giving up
/// with SQLITE_BUSY. Zero disables the wait.
fn busy_timeout(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(CONNECTION));
  enforce_method_arg_type!(ctx, 1, ArgType::Number);

  let db = db_of(ctx);
  let milliseconds = ctx.args[1].as_number();

  if !milliseconds.is_finite() || milliseconds < 0.0 {
    return Err(String::from(
      "a busy timeout must be zero or more milliseconds",
    ));
  }

  let code = unsafe { ffi::sqlite3_busy_timeout(db, milliseconds as c_int) };
  util::check(db, code)?;

  Ok(Value::nil())
}

/// Asks any statement currently running on this connection to abort.
/// The statement fails with SQLITE_INTERRUPT rather than being killed.
fn interrupt(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(CONNECTION));

  unsafe { ffi::sqlite3_interrupt(db_of(ctx)) };

  Ok(Value::nil())
}

/// Runs one or more statements for their effect, with no parameters and
/// no results. This is the path for scripts: schema files, pragmas, and
/// the BEGIN and COMMIT that bracket a transaction.
fn exec(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(CONNECTION));
  enforce_method_arg_type!(ctx, 1, ArgType::String);

  let db = db_of(ctx);
  let sql = util::to_cstring("a sql string", ctx.args[1].as_str())?;
  let state = conn_state(ctx);

  let code = {
    // Published for the duration because exec can reach a trigger, and
    // a trigger can reach a user-defined function.
    let _guard = callback::enter_vm(ctx.vm);

    unsafe {
      ffi::sqlite3_exec(
        db,
        sql.as_ptr(),
        None,
        std::ptr::null_mut(),
        std::ptr::null_mut(),
      )
    }
  };

  if let Some(message) = take_trapped(ctx.vm, state) {
    return Err(message);
  }

  util::check(db, code)?;

  Ok(Value::nil())
}

fn changes(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(CONNECTION));

  let n = unsafe { ffi::sqlite3_changes64(db_of(ctx)) };
  Ok(value::integer_value(ctx, n))
}

fn total_changes(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(CONNECTION));

  let n = unsafe { ffi::sqlite3_total_changes64(db_of(ctx)) };
  Ok(value::integer_value(ctx, n))
}

fn last_insert_rowid(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(CONNECTION));

  let n = unsafe { ffi::sqlite3_last_insert_rowid(db_of(ctx)) };
  Ok(value::integer_value(ctx, n))
}

/// False while a transaction is open, which is how the Zuri side knows
/// whether a BEGIN it did not issue is already in effect.
fn autocommit(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(CONNECTION));

  Ok(Value::bool(
    unsafe { ffi::sqlite3_get_autocommit(db_of(ctx)) } != 0,
  ))
}

fn errcode(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(CONNECTION));

  Ok(Value::number(
    unsafe { ffi::sqlite3_extended_errcode(db_of(ctx)) } as f64,
  ))
}

fn errmsg(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(CONNECTION));

  let text = unsafe { util::cstr_to_string(ffi::sqlite3_errmsg(db_of(ctx))) };
  Ok(ctx.heap().alloc_string(text))
}

/// `prepare(db, sql)`, returning `{ statement, tail }`.
///
/// `tail` is whatever followed the statement that was compiled, so a
/// caller holding a script can prepare it one statement at a time
/// rather than guessing where the boundaries are. It is the empty
/// string once nothing is left.
///
/// A `sql` that is entirely whitespace or a comment compiles to no
/// statement at all, in which case `statement` is nil.
fn prepare(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(CONNECTION));
  enforce_method_arg_type!(ctx, 1, ArgType::String);

  let db = db_of(ctx);
  let sql = ctx.args[1].as_str().to_string();
  let state = conn_state(ctx);

  let mut stmt: *mut ffi::sqlite3_stmt = std::ptr::null_mut();
  let mut tail: *const c_char = std::ptr::null();

  let code = {
    // An authorizer runs during preparation, not execution, so this
    // needs the VM published too.
    let _guard = callback::enter_vm(ctx.vm);

    unsafe {
      ffi::sqlite3_prepare_v2(
        db,
        sql.as_ptr() as *const c_char,
        sql.len() as c_int,
        &mut stmt,
        &mut tail,
      )
    }
  };

  if let Some(message) = take_trapped(ctx.vm, state) {
    return Err(message);
  }

  util::check(db, code)?;

  // `tail` points into the buffer that was passed in, so the remainder
  // is measured as an offset rather than read as a C string.
  let remaining = if tail.is_null() {
    String::new()
  } else {
    let consumed = tail as usize - sql.as_ptr() as usize;
    sql.get(consumed..).unwrap_or("").to_string()
  };

  let statement = if stmt.is_null() {
    Value::nil()
  } else {
    ctx.heap().alloc_ptr(STATEMENT, SqliteStmt { stmt, state })
  };

  let tail_value = ctx.heap().alloc_string(remaining);
  let statement_key = ctx.heap().alloc_string("statement");
  let tail_key = ctx.heap().alloc_string("tail");

  Ok(
    ctx
      .heap()
      .alloc_dict(vec![(statement_key, statement), (tail_key, tail_value)]),
  )
}

fn finalize(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(STATEMENT));

  let mut cell = ctx.args[0].as_ptr_cell().borrow_mut();

  if let Some(statement) = cell.downcast_mut::<SqliteStmt>() {
    statement.finalize();
  }

  cell.type_name = STATEMENT_CLOSED;

  Ok(Value::nil())
}

/// Rewinds a statement so it can be stepped again. Bindings survive;
/// `clear_bindings` is what drops those.
fn reset(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(STATEMENT));

  // sqlite3_reset reports the error of the statement's last run rather
  // than an error of its own, and the caller has already seen that one
  // from step. Deliberately not checked here.
  unsafe { ffi::sqlite3_reset(stmt_of(ctx)) };

  Ok(Value::nil())
}

fn clear_bindings(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(STATEMENT));

  unsafe { ffi::sqlite3_clear_bindings(stmt_of(ctx)) };

  Ok(Value::nil())
}

/// Advances the statement, returning `'row'` when one is available and
/// `'done'` when the statement has finished.
fn step(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(STATEMENT));

  let stmt = stmt_of(ctx);
  let state = stmt_state(ctx);

  let code = {
    let _guard = callback::enter_vm(ctx.vm);

    unsafe { ffi::sqlite3_step(stmt) }
  };

  if let Some(message) = take_trapped(ctx.vm, state) {
    return Err(message);
  }

  match code {
    ffi::SQLITE_ROW => Ok(ctx.heap().alloc_string("row")),
    ffi::SQLITE_DONE => Ok(ctx.heap().alloc_string("done")),
    _ => {
      let db = unsafe { ffi::sqlite3_db_handle(stmt) };
      Err(util::last_error(db))
    },
  }
}

/// Binds one parameter, addressed by its 1 based index.
fn bind(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 2);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(STATEMENT));
  enforce_method_arg_type!(ctx, 1, ArgType::Number);

  let stmt = stmt_of(ctx);
  let index = ctx.args[1].as_number();

  if !index.is_finite() || index < 1.0 {
    return Err(String::from("a parameter index starts at 1"));
  }

  let code = value::bind(stmt, index as c_int, ctx.args[2])?;

  if code != ffi::SQLITE_OK {
    let db = unsafe { ffi::sqlite3_db_handle(stmt) };
    return Err(util::last_error(db));
  }

  Ok(Value::nil())
}

fn bind_parameter_count(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(STATEMENT));

  Ok(Value::number(
    unsafe { ffi::sqlite3_bind_parameter_count(stmt_of(ctx)) } as f64,
  ))
}

/// The index of a named parameter, or 0 when the statement has no such
/// parameter. The name includes its sigil, as it appears in the SQL.
fn bind_parameter_index(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(STATEMENT));
  enforce_method_arg_type!(ctx, 1, ArgType::String);

  let stmt = stmt_of(ctx);
  let name = util::to_cstring("a parameter name", ctx.args[1].as_str())?;

  let index = unsafe { ffi::sqlite3_bind_parameter_index(stmt, name.as_ptr()) };

  Ok(Value::number(index as f64))
}

/// The name of parameter `index`, or nil for a positional one, which
/// has no name.
fn bind_parameter_name(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(STATEMENT));
  enforce_method_arg_type!(ctx, 1, ArgType::Number);

  let stmt = stmt_of(ctx);
  let index = ctx.args[1].as_number() as c_int;

  let name = unsafe { ffi::sqlite3_bind_parameter_name(stmt, index) };

  if name.is_null() {
    return Ok(Value::nil());
  }

  let text = unsafe { util::cstr_to_string(name) };
  Ok(ctx.heap().alloc_string(text))
}

fn column_count(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(STATEMENT));

  Ok(Value::number(
    unsafe { ffi::sqlite3_column_count(stmt_of(ctx)) } as f64,
  ))
}

/// Every column's name, in order.
///
/// These are the names as the result set reports them, which for an
/// expression is the expression's own text unless the query gave it an
/// alias. Duplicates are possible and are left as they are.
fn column_names(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(STATEMENT));

  let stmt = stmt_of(ctx);
  let count = unsafe { ffi::sqlite3_column_count(stmt) };

  let mut names = Vec::with_capacity(count as usize);

  for index in 0..count {
    let text = unsafe { util::cstr_to_string(ffi::sqlite3_column_name(stmt, index)) };
    names.push(ctx.heap().alloc_string(text));
  }

  Ok(ctx.heap().alloc_list(names))
}

/// Every column's declared type, or nil where there is none.
///
/// A column that is an expression rather than a table column has no
/// declared type. `sql.sqlite` uses these to decide how to read a value
/// back, since SQLite itself stores no such intent.
fn column_decltypes(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(STATEMENT));

  let stmt = stmt_of(ctx);
  let count = unsafe { ffi::sqlite3_column_count(stmt) };

  let mut types = Vec::with_capacity(count as usize);

  for index in 0..count {
    let declared = unsafe { ffi::sqlite3_column_decltype(stmt, index) };

    types.push(if declared.is_null() {
      Value::nil()
    } else {
      let text = unsafe { util::cstr_to_string(declared) };
      ctx.heap().alloc_string(text)
    });
  }

  Ok(ctx.heap().alloc_list(types))
}

/// The storage class of a column in the current row: one of
/// `'integer'`, `'float'`, `'text'`, `'blob'` or `'null'`.
fn column_type(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(STATEMENT));
  enforce_method_arg_type!(ctx, 1, ArgType::Number);

  let stmt = stmt_of(ctx);
  let index = column_index(ctx, stmt, 1)?;

  let code = unsafe { ffi::sqlite3_column_type(stmt, index) };

  Ok(ctx.heap().alloc_string(value::type_name(code)))
}

fn column_value(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(STATEMENT));
  enforce_method_arg_type!(ctx, 1, ArgType::Number);

  let stmt = stmt_of(ctx);
  let index = column_index(ctx, stmt, 1)?;

  Ok(value::column(ctx, stmt, index))
}

/// The whole current row as a list, which is one call instead of one
/// per column. Reading rows is the hottest thing this module does, so
/// it is worth not crossing the boundary once per cell.
fn row(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(STATEMENT));

  let stmt = stmt_of(ctx);
  let count = unsafe { ffi::sqlite3_data_count(stmt) };

  let mut values = Vec::with_capacity(count as usize);

  for index in 0..count {
    values.push(value::column(ctx, stmt, index));
  }

  Ok(ctx.heap().alloc_list(values))
}

/// Where a result column actually came from: `{ database, table,
/// column }`, each nil for a column that is an expression rather than a
/// table column.
fn column_origin(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(STATEMENT));
  enforce_method_arg_type!(ctx, 1, ArgType::Number);

  let stmt = stmt_of(ctx);
  let index = column_index(ctx, stmt, 1)?;

  let database = unsafe { ffi::sqlite3_column_database_name(stmt, index) };
  let table = unsafe { ffi::sqlite3_column_table_name(stmt, index) };
  let column = unsafe { ffi::sqlite3_column_origin_name(stmt, index) };

  let mut pairs = Vec::with_capacity(3);

  for (name, ptr) in [("database", database), ("table", table), ("column", column)] {
    let key = ctx.heap().alloc_string(name);

    let value = if ptr.is_null() {
      Value::nil()
    } else {
      let text = unsafe { util::cstr_to_string(ptr) };
      ctx.heap().alloc_string(text)
    };

    pairs.push((key, value));
  }

  Ok(ctx.heap().alloc_dict(pairs))
}

/// How many columns the current row has. Zero before the first step and
/// after the last, which is how a caller tells a query apart from a
/// statement that returns nothing.
fn data_count(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(STATEMENT));

  Ok(Value::number(
    unsafe { ffi::sqlite3_data_count(stmt_of(ctx)) } as f64,
  ))
}

fn stmt_sql(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(STATEMENT));

  let text = unsafe { util::cstr_to_string(ffi::sqlite3_sql(stmt_of(ctx))) };
  Ok(ctx.heap().alloc_string(text))
}

/// The statement's SQL with its bound parameters substituted in.
///
/// For logging and diagnostics. Never for building a statement to run:
/// the result is a rendering of what was bound, not an escaping
/// routine.
fn expanded_sql(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(STATEMENT));

  let expanded = unsafe { ffi::sqlite3_expanded_sql(stmt_of(ctx)) };

  if expanded.is_null() {
    return Ok(Value::nil());
  }

  let text = unsafe { util::cstr_to_string(expanded) };

  // SQLite allocated this one and expects it back.
  unsafe { ffi::sqlite3_free(expanded as *mut c_void) };

  Ok(ctx.heap().alloc_string(text))
}

/// Whether the statement only reads. A pool uses this to know that a
/// statement is safe to run on a read-only connection.
fn stmt_readonly(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(STATEMENT));

  Ok(Value::bool(
    unsafe { ffi::sqlite3_stmt_readonly(stmt_of(ctx)) } != 0,
  ))
}

/// Whether the statement has been stepped and not yet reset, which is
/// what makes re-binding it an error.
fn stmt_busy(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(STATEMENT));

  Ok(Value::bool(
    unsafe { ffi::sqlite3_stmt_busy(stmt_of(ctx)) } != 0,
  ))
}

/// Reads and range checks a column index argument.
///
/// SQLite reads an out-of-range index as NULL rather than complaining,
/// which turns a loop that runs one column too far into a column of
/// nils instead of an error.
fn column_index(
  ctx: &ZuriContext,
  stmt: *mut ffi::sqlite3_stmt,
  arg: usize,
) -> Result<c_int, String> {
  let index = ctx.args[arg].as_number();
  let count = unsafe { ffi::sqlite3_column_count(stmt) };

  if !index.is_finite() || index < 0.0 || index >= count as f64 {
    return Err(format!(
      "column index {index} is out of range for a result with {count} columns"
    ));
  }

  Ok(index as c_int)
}

/// `blob_open(db, database, table, column, rowid, writable)`.
///
/// Opens one blob-valued cell for incremental reading and writing,
/// which is how a large value is moved without ever holding all of it
/// in memory. The row must already exist and the cell must already hold
/// a blob of the final size; SQLite cannot grow one through this
/// interface, which is what `zeroblob(n)` in an INSERT is for.
fn blob_open(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_range!(ctx, 5, 6);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(CONNECTION));
  enforce_method_arg_type!(ctx, 1, ArgType::String);
  enforce_method_arg_type!(ctx, 2, ArgType::String);
  enforce_method_arg_type!(ctx, 3, ArgType::String);
  enforce_method_arg_type!(ctx, 4, ArgType::Number);

  let db = db_of(ctx);
  let database = util::to_cstring("a database name", ctx.args[1].as_str())?;
  let table = util::to_cstring("a table name", ctx.args[2].as_str())?;
  let column = util::to_cstring("a column name", ctx.args[3].as_str())?;
  let rowid = whole(ctx.args[4].as_number(), "a rowid")?;
  let writable = optional_bool(ctx, 5, false)?;

  let mut blob: *mut ffi::sqlite3_blob = std::ptr::null_mut();

  let code = unsafe {
    ffi::sqlite3_blob_open(
      db,
      database.as_ptr(),
      table.as_ptr(),
      column.as_ptr(),
      rowid,
      if writable { 1 } else { 0 },
      &mut blob,
    )
  };

  util::check(db, code)?;

  Ok(ctx.heap().alloc_ptr(BLOB, SqliteBlob { blob }))
}

/// The size of the open blob, which is fixed for its lifetime.
fn blob_bytes(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(BLOB));

  Ok(Value::number(
    unsafe { ffi::sqlite3_blob_bytes(blob_of(ctx)) } as f64,
  ))
}

/// Reads `length` bytes from `offset`. Both are checked against the
/// blob's size here, because SQLite's own answer to a read past the end
/// is a bare error that says nothing about which bound was missed.
fn blob_read(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 2);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(BLOB));
  enforce_method_arg_type!(ctx, 1, ArgType::Number);
  enforce_method_arg_type!(ctx, 2, ArgType::Number);

  let blob = blob_of(ctx);
  let size = unsafe { ffi::sqlite3_blob_bytes(blob) } as i64;

  let offset = whole(ctx.args[1].as_number(), "an offset")?;
  let length = whole(ctx.args[2].as_number(), "a length")?;

  if offset < 0 || length < 0 || offset + length > size {
    return Err(format!(
      "cannot read {length} bytes at offset {offset} from a blob of {size} bytes"
    ));
  }

  let mut buffer = vec![0u8; length as usize];

  let code = unsafe {
    ffi::sqlite3_blob_read(
      blob,
      buffer.as_mut_ptr() as *mut c_void,
      length as c_int,
      offset as c_int,
    )
  };

  if code != ffi::SQLITE_OK {
    return Err(util::format_error(code, &util::result_code_text(code)));
  }

  Ok(ctx.heap().alloc_bytes(buffer))
}

/// Writes bytes at `offset`. A blob opened read-only, or a write that
/// would run past the end, fails rather than being truncated to fit.
fn blob_write(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 2);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(BLOB));
  enforce_method_arg_type!(ctx, 1, ArgType::Number);
  enforce_method_arg_type!(ctx, 2, ArgType::Bytes);

  let blob = blob_of(ctx);
  let size = unsafe { ffi::sqlite3_blob_bytes(blob) } as i64;
  let offset = whole(ctx.args[1].as_number(), "an offset")?;
  let data = ctx.args[2].with_bytes(|b| b.to_vec());

  if offset < 0 || offset + data.len() as i64 > size {
    return Err(format!(
      "cannot write {} bytes at offset {offset} to a blob of {size} bytes",
      data.len()
    ));
  }

  let code = unsafe {
    ffi::sqlite3_blob_write(
      blob,
      data.as_ptr() as *const c_void,
      data.len() as c_int,
      offset as c_int,
    )
  };

  if code != ffi::SQLITE_OK {
    return Err(util::format_error(code, &util::result_code_text(code)));
  }

  Ok(Value::nil())
}

/// Points the same handle at the same column of a different row.
///
/// Cheaper than closing and reopening when walking many rows, which is
/// the whole reason SQLite offers it.
fn blob_reopen(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(BLOB));
  enforce_method_arg_type!(ctx, 1, ArgType::Number);

  let blob = blob_of(ctx);
  let rowid = whole(ctx.args[1].as_number(), "a rowid")?;

  let code = unsafe { ffi::sqlite3_blob_reopen(blob, rowid) };

  if code != ffi::SQLITE_OK {
    return Err(util::format_error(code, &util::result_code_text(code)));
  }

  Ok(Value::nil())
}

fn blob_close(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(BLOB));

  let mut cell = ctx.args[0].as_ptr_cell().borrow_mut();

  if let Some(blob) = cell.downcast_mut::<SqliteBlob>() {
    blob.close();
  }

  cell.type_name = BLOB_CLOSED;

  Ok(Value::nil())
}

/// `backup_init(destination, destination_name, source, source_name)`.
///
/// Starts an online backup, which copies a live database without
/// requiring it to be idle. The two connections have to be different;
/// one used as both is rejected here rather than deadlocking later.
fn backup_init(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 3);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(CONNECTION));
  enforce_method_arg_type!(ctx, 1, ArgType::String);
  enforce_method_arg_type!(ctx, 2, ArgType::PtrOf(CONNECTION));
  enforce_method_arg_type!(ctx, 3, ArgType::String);

  let destination = db_of(ctx);
  let destination_name = util::to_cstring("a database name", ctx.args[1].as_str())?;

  let source = {
    let cell = ctx.args[2].as_ptr_cell().borrow();
    cell.downcast_ref::<SqliteConn>().unwrap().db()
  };

  let source_name = util::to_cstring("a database name", ctx.args[3].as_str())?;

  if std::ptr::eq(destination, source) {
    return Err(String::from(
      "a backup needs two different connections, not one used as both source and destination",
    ));
  }

  let backup = unsafe {
    ffi::sqlite3_backup_init(
      destination,
      destination_name.as_ptr(),
      source,
      source_name.as_ptr(),
    )
  };

  if backup.is_null() {
    // A failed init leaves its explanation on the destination.
    return Err(util::last_error(destination));
  }

  Ok(ctx.heap().alloc_ptr(BACKUP, SqliteBackup { backup }))
}

/// Copies up to `pages` pages, or all of them when `pages` is negative.
///
/// Returns `'done'` when the copy is complete, `'ok'` when there is
/// more to do, and `'busy'` or `'locked'` when the source moved under
/// the backup and the step should be retried. Those two are outcomes
/// rather than errors: a backup of a database being written to is
/// expected to hit them and carry on.
fn backup_step(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(BACKUP));
  enforce_method_arg_type!(ctx, 1, ArgType::Number);

  let backup = backup_of(ctx);
  let pages = whole(ctx.args[1].as_number(), "a page count")?;

  let code = unsafe { ffi::sqlite3_backup_step(backup, pages as c_int) };

  let outcome = match code {
    ffi::SQLITE_DONE => "done",
    ffi::SQLITE_OK => "ok",
    ffi::SQLITE_BUSY => "busy",
    ffi::SQLITE_LOCKED => "locked",
    _ => return Err(util::format_error(code, &util::result_code_text(code))),
  };

  Ok(ctx.heap().alloc_string(outcome))
}

/// Pages still to copy, as of the last step.
fn backup_remaining(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(BACKUP));

  Ok(Value::number(
    unsafe { ffi::sqlite3_backup_remaining(backup_of(ctx)) } as f64,
  ))
}

/// Pages in the source database, as of the last step.
fn backup_pagecount(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(BACKUP));

  Ok(Value::number(
    unsafe { ffi::sqlite3_backup_pagecount(backup_of(ctx)) } as f64,
  ))
}

/// Ends the backup, reporting any error the copy accumulated.
///
/// Finishing an incomplete backup is legitimate: it abandons the copy
/// and leaves the destination as it was.
fn backup_finish(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(BACKUP));

  let code = {
    let mut cell = ctx.args[0].as_ptr_cell().borrow_mut();

    let code = match cell.downcast_mut::<SqliteBackup>() {
      Some(backup) => backup.finish(),
      None => ffi::SQLITE_OK,
    };

    cell.type_name = BACKUP_CLOSED;
    code
  };

  if code != ffi::SQLITE_OK {
    return Err(util::format_error(code, &util::result_code_text(code)));
  }

  Ok(Value::nil())
}

/// A whole-number argument, rejecting fractions and infinities that
/// would otherwise be silently truncated on the way into C.
fn whole(n: f64, what: &str) -> Result<i64, String> {
  if !n.is_finite() || n.fract() != 0.0 {
    return Err(format!("{what} must be a whole number, got {n}"));
  }

  Ok(n as i64)
}

fn blob_of(ctx: &ZuriContext) -> *mut ffi::sqlite3_blob {
  let cell = ctx.args[0].as_ptr_cell().borrow();
  cell.downcast_ref::<SqliteBlob>().unwrap().blob
}

fn backup_of(ctx: &ZuriContext) -> *mut ffi::sqlite3_backup {
  let cell = ctx.args[0].as_ptr_cell().borrow();
  cell.downcast_ref::<SqliteBackup>().unwrap().backup
}

/// Registers `registration` on the connection behind argument zero,
/// dropping whatever it replaces.
///
/// The box is kept by the connection so the address SQLite holds as
/// user data stays valid, and so everything is torn down together when
/// the connection closes.
fn install(
  ctx: &mut ZuriContext,
  kind: callback::Kind,
  name: &str,
  arity: i32,
  slot: usize,
  final_slot: Option<usize>,
) -> *mut callback::Registration {
  let displaced = {
    let mut cell = ctx.args[0].as_ptr_cell().borrow_mut();
    let connection = cell.downcast_mut::<SqliteConn>().unwrap();
    let state = connection.state_ptr();

    let previous = connection
      .state
      .registrations
      .iter()
      .position(|r| r.same_target(kind, name, arity));

    let displaced = previous.map(|index| connection.state.registrations.remove(index));

    connection
      .state
      .registrations
      .push(Box::new(callback::Registration {
        state,
        kind,
        slot,
        final_slot,
        name: name.to_string(),
        arity,
      }));

    displaced
  };

  // Released after the borrow ends, since giving the slots back needs
  // the VM.
  if let Some(displaced) = displaced {
    displaced.release(ctx.vm);
  }

  let mut cell = ctx.args[0].as_ptr_cell().borrow_mut();
  let connection = cell.downcast_mut::<SqliteConn>().unwrap();
  let last = connection.state.registrations.len() - 1;

  connection.state.registrations[last].as_mut() as *mut callback::Registration
}

/// Drops the registration matching `kind`, `name` and `arity`, if there
/// is one. Used when a hook is cleared by passing nil.
fn uninstall(ctx: &mut ZuriContext, kind: callback::Kind, name: &str, arity: i32) {
  let displaced = {
    let mut cell = ctx.args[0].as_ptr_cell().borrow_mut();
    let connection = cell.downcast_mut::<SqliteConn>().unwrap();

    let previous = connection
      .state
      .registrations
      .iter()
      .position(|r| r.same_target(kind, name, arity));

    previous.map(|index| connection.state.registrations.remove(index))
  };

  if let Some(displaced) = displaced {
    displaced.release(ctx.vm);
  }
}

/// A function's text-encoding-and-flags argument.
///
/// UTF8 is the only encoding this module uses; `deterministic` is worth
/// setting for a function whose result depends only on its arguments,
/// since it lets SQLite use the function in an index or a partial index
/// and hoist it out of a loop.
fn function_flags(deterministic: bool) -> c_int {
  let mut flags = ffi::SQLITE_UTF8;

  if deterministic {
    flags |= ffi::SQLITE_DETERMINISTIC;
  }

  flags
}

/// `create_function(db, name, arity, deterministic, callable)`.
///
/// An arity of -1 accepts any number of arguments. The callable is
/// invoked with the argument values and returns the result.
fn create_function(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 4);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(CONNECTION));
  enforce_method_arg_type!(ctx, 1, ArgType::String);
  enforce_method_arg_type!(ctx, 2, ArgType::Number);
  enforce_method_arg_type!(ctx, 3, ArgType::Bool);
  enforce_method_arg_type!(ctx, 4, ArgType::Function);

  let db = db_of(ctx);
  let name_text = ctx.args[1].as_str().to_string();
  let name = util::to_cstring("a function name", &name_text)?;
  let arity = whole(ctx.args[2].as_number(), "a function arity")? as i32;
  let flags = function_flags(ctx.args[3].as_bool());

  let slot = ctx.vm.retain_native_root(ctx.args[4]);
  let registration = install(ctx, callback::Kind::Scalar, &name_text, arity, slot, None);

  let code = unsafe {
    ffi::sqlite3_create_function_v2(
      db,
      name.as_ptr(),
      arity,
      flags,
      registration as *mut c_void,
      Some(callback::scalar),
      None,
      None,
      None,
    )
  };

  if code != ffi::SQLITE_OK {
    uninstall(ctx, callback::Kind::Scalar, &name_text, arity);
    return Err(util::last_error(db));
  }

  Ok(Value::nil())
}

/// `create_aggregate(db, name, arity, step, finish)`.
///
/// The Zuri side is a fold. `step` is called once per row with the
/// accumulator followed by the row's arguments and returns the next
/// accumulator; `finish` is called once per group with the final
/// accumulator and returns the group's value. The accumulator starts as
/// nil, which is also what `finish` sees for a group with no rows.
fn create_aggregate(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 4);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(CONNECTION));
  enforce_method_arg_type!(ctx, 1, ArgType::String);
  enforce_method_arg_type!(ctx, 2, ArgType::Number);
  enforce_method_arg_type!(ctx, 3, ArgType::Function);
  enforce_method_arg_type!(ctx, 4, ArgType::Function);

  let db = db_of(ctx);
  let name_text = ctx.args[1].as_str().to_string();
  let name = util::to_cstring("an aggregate name", &name_text)?;
  let arity = whole(ctx.args[2].as_number(), "an aggregate arity")? as i32;

  let step_slot = ctx.vm.retain_native_root(ctx.args[3]);
  let final_slot = ctx.vm.retain_native_root(ctx.args[4]);

  let registration = install(
    ctx,
    callback::Kind::Aggregate,
    &name_text,
    arity,
    step_slot,
    Some(final_slot),
  );

  let code = unsafe {
    ffi::sqlite3_create_function_v2(
      db,
      name.as_ptr(),
      arity,
      ffi::SQLITE_UTF8,
      registration as *mut c_void,
      None,
      Some(callback::aggregate_step),
      Some(callback::aggregate_final),
      None,
    )
  };

  if code != ffi::SQLITE_OK {
    uninstall(ctx, callback::Kind::Aggregate, &name_text, arity);
    return Err(util::last_error(db));
  }

  Ok(Value::nil())
}

/// Removes a function or aggregate previously registered under this
/// name and arity.
fn delete_function(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 2);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(CONNECTION));
  enforce_method_arg_type!(ctx, 1, ArgType::String);
  enforce_method_arg_type!(ctx, 2, ArgType::Number);

  let db = db_of(ctx);
  let name_text = ctx.args[1].as_str().to_string();
  let name = util::to_cstring("a function name", &name_text)?;
  let arity = whole(ctx.args[2].as_number(), "a function arity")? as i32;

  let code = unsafe {
    ffi::sqlite3_create_function_v2(
      db,
      name.as_ptr(),
      arity,
      ffi::SQLITE_UTF8,
      std::ptr::null_mut(),
      None,
      None,
      None,
      None,
    )
  };

  util::check(db, code)?;
  uninstall(ctx, callback::Kind::Scalar, &name_text, arity);

  Ok(Value::nil())
}

/// `create_collation(db, name, callable)`.
///
/// The callable is passed two strings and returns a negative number,
/// zero, or a positive number, the same shape a sort comparator takes
/// everywhere else.
fn create_collation(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 2);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(CONNECTION));
  enforce_method_arg_type!(ctx, 1, ArgType::String);
  enforce_method_arg_type!(ctx, 2, ArgType::Function);

  let db = db_of(ctx);
  let name_text = ctx.args[1].as_str().to_string();
  let name = util::to_cstring("a collation name", &name_text)?;

  let slot = ctx.vm.retain_native_root(ctx.args[2]);
  let registration = install(ctx, callback::Kind::Collation, &name_text, 0, slot, None);

  let code = unsafe {
    ffi::sqlite3_create_collation_v2(
      db,
      name.as_ptr(),
      ffi::SQLITE_UTF8,
      registration as *mut c_void,
      Some(callback::collation),
      None,
    )
  };

  if code != ffi::SQLITE_OK {
    uninstall(ctx, callback::Kind::Collation, &name_text, 0);
    return Err(util::last_error(db));
  }

  Ok(Value::nil())
}

/// Registers or clears one of the connection-wide hooks.
///
/// They all share this shape: one callable per connection, replaced by
/// registering another and cleared by passing nil.
fn set_hook(
  ctx: &mut ZuriContext,
  kind: callback::Kind,
  install_hook: impl FnOnce(*mut ffi::sqlite3, *mut callback::Registration),
  clear_hook: impl FnOnce(*mut ffi::sqlite3),
) -> Result<Value, String> {
  let db = db_of(ctx);

  if ctx.args[1].is_nil() {
    clear_hook(db);
    uninstall(ctx, kind, "", 0);

    return Ok(Value::nil());
  }

  if !ctx.args[1].is_callable() {
    return Err(format!(
      "a hook is a function or nil, got {}",
      ctx.args[1].type_name()
    ));
  }

  let slot = ctx.vm.retain_native_root(ctx.args[1]);
  let registration = install(ctx, kind, "", 0, slot, None);

  install_hook(db, registration);

  Ok(Value::nil())
}

/// Called after each row an INSERT, UPDATE or DELETE changes, with the
/// operation, the database and table names, and the row id.
///
/// The hook sees changes made directly by statements. Changes a trigger
/// or a foreign key action makes do not reach it, and neither does a
/// change rolled back afterwards.
fn set_update_hook(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(CONNECTION));

  set_hook(
    ctx,
    callback::Kind::UpdateHook,
    |db, registration| {
      unsafe {
        ffi::sqlite3_update_hook(db, Some(callback::update_hook), registration as *mut c_void)
      };
    },
    |db| {
      unsafe { ffi::sqlite3_update_hook(db, None, std::ptr::null_mut()) };
    },
  )
}

/// Called just before each commit. Returning `false` turns the commit
/// into a rollback; any other result lets it through.
fn set_commit_hook(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(CONNECTION));

  set_hook(
    ctx,
    callback::Kind::CommitHook,
    |db, registration| {
      unsafe {
        ffi::sqlite3_commit_hook(db, Some(callback::commit_hook), registration as *mut c_void)
      };
    },
    |db| {
      unsafe { ffi::sqlite3_commit_hook(db, None, std::ptr::null_mut()) };
    },
  )
}

/// Called whenever a transaction rolls back, however it was triggered.
fn set_rollback_hook(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(CONNECTION));

  set_hook(
    ctx,
    callback::Kind::RollbackHook,
    |db, registration| {
      unsafe {
        ffi::sqlite3_rollback_hook(
          db,
          Some(callback::rollback_hook),
          registration as *mut c_void,
        )
      };
    },
    |db| {
      unsafe { ffi::sqlite3_rollback_hook(db, None, std::ptr::null_mut()) };
    },
  )
}

/// Consulted for each action a statement wants to take, as it is
/// prepared rather than as it runs.
///
/// The callable is passed the action code and the four strings SQLite
/// supplies about it, and answers `'allow'`, `'deny'` or `'ignore'`.
/// Anything else counts as denial, so a callable that falls off the end
/// without returning fails closed.
fn set_authorizer(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(CONNECTION));

  set_hook(
    ctx,
    callback::Kind::Authorizer,
    |db, registration| {
      unsafe {
        ffi::sqlite3_set_authorizer(db, Some(callback::authorizer), registration as *mut c_void)
      };
    },
    |db| {
      unsafe { ffi::sqlite3_set_authorizer(db, None, std::ptr::null_mut()) };
    },
  )
}

/// `set_progress_handler(db, instructions, callable)`.
///
/// Called every `instructions` virtual machine steps during a long
/// statement. Returning `false` interrupts it. Passing nil for the
/// callable clears the handler.
fn set_progress_handler(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 2);
  enforce_method_arg_type!(ctx, 0, ArgType::PtrOf(CONNECTION));
  enforce_method_arg_type!(ctx, 1, ArgType::Number);

  let db = db_of(ctx);
  let instructions = whole(ctx.args[1].as_number(), "an instruction count")?;

  if ctx.args[2].is_nil() {
    unsafe { ffi::sqlite3_progress_handler(db, 0, None, std::ptr::null_mut()) };
    uninstall(ctx, callback::Kind::Progress, "", 0);

    return Ok(Value::nil());
  }

  if !ctx.args[2].is_callable() {
    return Err(format!(
      "a progress handler is a function or nil, got {}",
      ctx.args[2].type_name()
    ));
  }

  if instructions <= 0 {
    return Err(String::from(
      "a progress handler runs every N instructions, so N has to be positive",
    ));
  }

  let slot = ctx.vm.retain_native_root(ctx.args[2]);
  let registration = install(ctx, callback::Kind::Progress, "", 0, slot, None);

  unsafe {
    ffi::sqlite3_progress_handler(
      db,
      instructions as c_int,
      Some(callback::progress),
      registration as *mut c_void,
    )
  };

  Ok(Value::nil())
}
