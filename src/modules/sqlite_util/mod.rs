//! Shared plumbing for the `_sqlite` native module: the handle types
//! Zuri sees as `Ptr`s, the conversions between SQLite's value model
//! and Zuri's, and the trampolines SQLite calls back through.
//!
//! The module talks to SQLite's C API directly rather than through a
//! safe wrapper. A Zuri cursor holds a live `sqlite3_stmt` and steps it
//! one row at a time across separate calls from the VM, which a
//! lifetime-bound statement type cannot express without tying itself
//! back to the connection it borrowed from. Raw handles behind
//! `Obj::Ptr` are also what every other external resource in this
//! codebase already uses.

pub mod callback;
pub mod value;

use std::ffi::{CStr, CString};
use std::os::raw::c_char;

use libsqlite3_sys as ffi;

use crate::vm::value::Value;
use crate::vm::vm::VM;

/// `Ptr` tags. Every native here checks the tag before it downcasts,
/// so handing a blob handle to a statement function fails with a type
/// error naming both rather than a bad cast deeper in.
pub const CONNECTION: &str = "zuri::sql::sqlite3::connection";
pub const STATEMENT: &str = "zuri::sql::sqlite3::statement";
pub const BLOB: &str = "zuri::sql::sqlite3::blob";
pub const BACKUP: &str = "zuri::sql::sqlite3::backup";

/// What a handle's tag becomes once it has been closed, so a call on a
/// closed connection fails loudly instead of reaching a dangling
/// `sqlite3*`. Mirrors how `net_tcp` invalidates a closed socket.
pub const CONNECTION_CLOSED: &str = "zuri::sql::sqlite3::connection::__closed__";
pub const STATEMENT_CLOSED: &str = "zuri::sql::sqlite3::statement::__closed__";
pub const BLOB_CLOSED: &str = "zuri::sql::sqlite3::blob::__closed__";
pub const BACKUP_CLOSED: &str = "zuri::sql::sqlite3::backup::__closed__";

// `sqlite3_close_v2` is compiled into the library but `libsqlite3-sys`
// does not declare it: its pre-generated bindings are cut from a
// curated header set that stops short of this one.
//
// It is worth declaring by hand rather than settling for
// `sqlite3_close`. The plain version refuses to close a connection that
// still has an unfinalized statement, and answering that refusal by
// doing nothing would hold the database file open for the life of the
// process. `close_v2` instead marks the connection as a zombie and
// reclaims it once the last statement goes, which is the behaviour a
// garbage-collected language wants: a program is not required to have
// finalized every cursor before it closes a connection.
unsafe extern "C" {
  fn sqlite3_close_v2(db: *mut ffi::sqlite3) -> std::os::raw::c_int;
}

/// Closes `db` and returns its result code, tolerating a null handle so
/// callers can close idempotently.
pub fn close_db(db: *mut ffi::sqlite3) -> i32 {
  if db.is_null() {
    return ffi::SQLITE_OK;
  }

  unsafe { sqlite3_close_v2(db) }
}

/// A Zuri error raised inside a callback while SQLite owned the stack.
///
/// It cannot be thrown from there: the callback was invoked from C, and
/// unwinding a Rust panic through a C frame is undefined behaviour, so
/// the error is parked here instead and re-raised by whichever native
/// was driving SQLite once control is back on the Rust side.
pub struct Trapped {
  /// The slot in `VM::native_roots` holding the raised value, kept
  /// rooted because a collection can run between the callback and the
  /// point the native gets to re-raise it.
  pub slot: usize,
  /// The error's text, used only if the value behind `slot` has gone,
  /// which nothing ordinary should be able to cause.
  pub message: String,
}

/// Per-connection state that callbacks reach without touching the Zuri
/// heap at all.
///
/// Boxed by `SqliteConn` and addressed by raw pointer from every
/// registered callback. That indirection is the point: moving the
/// handle (to another isolate, or just around in memory as the `Ptr`
/// payload is passed along) moves the `Box`, never what it points at,
/// so a pointer SQLite is holding stays valid across the move.
pub struct ConnState {
  pub db: *mut ffi::sqlite3,
  /// The first error a callback raised on this connection, if any.
  ///
  /// Only the first: once this is set, every further callback on the
  /// connection returns an error to SQLite without re-entering Zuri.
  /// A statement that has already failed is being torn down, and
  /// running more Zuri code during that teardown would both obscure
  /// the original error and re-enter the VM at a point the erroring
  /// callback has no reason to expect.
  pub trapped: Option<Trapped>,
  /// Everything registered against this connection. Held so it can be
  /// torn down with the connection, and boxed so the addresses handed
  /// to SQLite as user data stay put as the vector grows.
  pub registrations: Vec<Box<callback::Registration>>,
}

impl ConnState {
  /// Parks `error` as the trapped error, unless one is already parked.
  ///
  /// `message` is only a fallback for display; the value itself is what
  /// gets re-raised, so the class the program actually raised survives
  /// the trip through C.
  pub fn trap(&mut self, vm: &mut VM, error: Value, message: String) {
    if self.trapped.is_some() {
      return;
    }

    let slot = vm.retain_native_root(error);
    self.trapped = Some(Trapped { slot, message });
  }

  /// True once a callback has failed, which is every later callback's
  /// cue to fail immediately rather than run more Zuri code.
  pub fn poisoned(&self) -> bool {
    self.trapped.is_some()
  }
}

/// A database connection, as Zuri sees it.
pub struct SqliteConn {
  pub state: Box<ConnState>,
}

// SAFETY: the bundled amalgamation is built in serialized threading
// mode (SQLITE_THREADSAFE=1), which is what makes a connection legal to
// use from a thread other than the one that opened it. A handle is only
// ever moved between isolates, never shared: `ObjPtr::take` leaves the
// source tombstoned, so two isolates cannot hold the same `sqlite3*`.
// The raw pointers in `ConnState` are what SQLite handed back and are
// not tied to the opening thread.
unsafe impl Send for SqliteConn {}

impl SqliteConn {
  pub fn new(db: *mut ffi::sqlite3) -> Self {
    SqliteConn {
      state: Box::new(ConnState {
        db,
        trapped: None,
        registrations: Vec::new(),
      }),
    }
  }

  pub fn db(&self) -> *mut ffi::sqlite3 {
    self.state.db
  }

  /// Raw pointer to the boxed state, for handing to SQLite as the user
  /// data of a callback.
  pub fn state_ptr(&mut self) -> *mut ConnState {
    self.state.as_mut() as *mut ConnState
  }

  /// Closes the connection and releases every callback slot it held.
  ///
  /// `sqlite3_close_v2` rather than `sqlite3_close` so an unfinalized
  /// statement leaves the handle to be reclaimed once the statement
  /// goes, instead of refusing to close and leaking the database file
  /// handle for the life of the process.
  pub fn close(&mut self, vm: &mut VM) {
    for registration in self.state.registrations.drain(..) {
      registration.release(vm);
    }

    if let Some(trapped) = self.state.trapped.take() {
      vm.release_native_root(trapped.slot);
    }

    if !self.state.db.is_null() {
      close_db(self.state.db);
      self.state.db = std::ptr::null_mut();
    }
  }
}

impl Drop for SqliteConn {
  /// Last-resort cleanup for a connection dropped without `close()`,
  /// which is what happens when one is simply garbage collected.
  ///
  /// This releases the database handle, which is the part that holds an
  /// operating-system resource. It cannot release the connection's
  /// callback slots, because reaching `VM::release_native_root` needs a
  /// VM and there is none here; those slots stay occupied until the VM
  /// ends. Closing a connection explicitly is what frees them.
  fn drop(&mut self) {
    if !self.state.db.is_null() {
      close_db(self.state.db);
      self.state.db = std::ptr::null_mut();
    }
  }
}

/// A prepared statement.
///
/// It keeps the connection's state pointer so a native driving this
/// statement can find a trapped error after `sqlite3_step` returns
/// without having to be handed the connection as well.
pub struct SqliteStmt {
  pub stmt: *mut ffi::sqlite3_stmt,
  pub state: *mut ConnState,
}

// SAFETY: as for `SqliteConn`, and with the same single-owner argument:
// a statement handle travels with the connection that owns it.
unsafe impl Send for SqliteStmt {}

impl SqliteStmt {
  pub fn finalize(&mut self) {
    if !self.stmt.is_null() {
      unsafe { ffi::sqlite3_finalize(self.stmt) };
      self.stmt = std::ptr::null_mut();
    }
  }
}

impl Drop for SqliteStmt {
  fn drop(&mut self) {
    self.finalize();
  }
}

/// An open blob handle, for incremental reads and writes against a
/// single blob-valued cell.
pub struct SqliteBlob {
  pub blob: *mut ffi::sqlite3_blob,
}

// SAFETY: as for `SqliteConn`.
unsafe impl Send for SqliteBlob {}

impl SqliteBlob {
  pub fn close(&mut self) {
    if !self.blob.is_null() {
      unsafe { ffi::sqlite3_blob_close(self.blob) };
      self.blob = std::ptr::null_mut();
    }
  }
}

impl Drop for SqliteBlob {
  fn drop(&mut self) {
    self.close();
  }
}

/// An online backup in progress between two connections.
pub struct SqliteBackup {
  pub backup: *mut ffi::sqlite3_backup,
}

// SAFETY: as for `SqliteConn`.
unsafe impl Send for SqliteBackup {}

impl SqliteBackup {
  pub fn finish(&mut self) -> i32 {
    if self.backup.is_null() {
      return ffi::SQLITE_OK;
    }

    let code = unsafe { ffi::sqlite3_backup_finish(self.backup) };
    self.backup = std::ptr::null_mut();
    code
  }
}

impl Drop for SqliteBackup {
  fn drop(&mut self) {
    self.finish();
  }
}

/// Reads a C string SQLite owns, as a Rust `String`. Empty for null,
/// which is what most of these return when they have nothing to say.
///
/// # Safety
///
/// `ptr` must be null or a valid NUL-terminated string that stays alive
/// for the duration of the call.
pub unsafe fn cstr_to_string(ptr: *const c_char) -> String {
  if ptr.is_null() {
    return String::new();
  }

  unsafe { CStr::from_ptr(ptr) }.to_string_lossy().into_owned()
}

/// Turns a Rust string into one SQLite can take.
///
/// Fails on an interior NUL rather than silently truncating at it,
/// which for a SQL string would mean running a prefix of what the
/// program asked for.
pub fn to_cstring(what: &str, text: &str) -> Result<CString, String> {
  CString::new(text)
    .map_err(|_| format!("{what} cannot contain a null character"))
}

/// The current error on `db`, as a message with its extended result
/// code attached so the Zuri side can map it onto a specific error
/// class. Extended rather than primary because the useful distinctions
/// live there: every constraint failure is `SQLITE_CONSTRAINT`, but
/// only the extended code says which constraint.
pub fn last_error(db: *mut ffi::sqlite3) -> String {
  let code = unsafe { ffi::sqlite3_extended_errcode(db) };
  let message = unsafe { cstr_to_string(ffi::sqlite3_errmsg(db)) };

  format_error(code, &message)
}

/// The message shape every error from this module takes: the extended
/// result code, then the text. `sqlite.zu` parses the code back out to
/// choose an error class, so the prefix is load-bearing and not just
/// decoration.
pub fn format_error(code: i32, message: &str) -> String {
  if message.is_empty() {
    return format!("[{code}] sqlite error {code}");
  }

  format!("[{code}] {message}")
}

/// Turns a non-OK result code into the error the native returns,
/// preferring the connection's own message when there is one.
pub fn check(db: *mut ffi::sqlite3, code: i32) -> Result<(), String> {
  if code == ffi::SQLITE_OK {
    return Ok(());
  }

  if db.is_null() {
    return Err(format_error(code, &result_code_text(code)));
  }

  Err(last_error(db))
}

/// SQLite's own one-line description of a result code, for the cases
/// where there is no connection to ask for something better.
pub fn result_code_text(code: i32) -> String {
  unsafe { cstr_to_string(ffi::sqlite3_errstr(code)) }
}
