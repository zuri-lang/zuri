use crate::builtins::enforce::ArgType;
use crate::vm::object::{FileHandle, NativeFn, NativeFunction, ZuriContext, write_barrier};
use crate::vm::value::Value;
use crate::vm::vm::VM;
use crate::{
  enforce_arg_count, enforce_arg_range, enforce_arg_type, enforce_arg_type_any_of,
  enforce_arg_type_opt,
};
use std::cell::Cell;
use std::io::Write;
use std::time::{SystemTime, UNIX_EPOCH};

pub fn install(vm: &mut VM) {
  register(vm, "time", 0, false, time);
  register(vm, "sum", 1, true, sum);
  register(vm, "bytes", 1, false, bytes);
  register(vm, "file", 1, true, file);
  register(vm, "instance_of", 2, false, instance_of);
  register(vm, "typeof", 1, false, typeof_fn);

  register(vm, "delprop", 2, false, delprop);
  register(vm, "getprop", 2, false, getprop);
  register(vm, "hasprop", 2, false, hasprop);
  register(vm, "setprop", 3, false, setprop);

  register(vm, "id", 1, false, id_fn);
  register(vm, "print", 0, true, print_fn);
  register(vm, "rand", 0, true, rand_fn);

  register(vm, "is_bigint", 1, false, is_bigint);
  register(vm, "is_bool", 1, false, is_bool);
  register(vm, "is_bytes", 1, false, is_bytes);
  register(vm, "is_callable", 1, false, is_callable);
  register(vm, "is_class", 1, false, is_class);
  register(vm, "is_dict", 1, false, is_dict);
  register(vm, "is_file", 1, false, is_file);
  register(vm, "is_function", 1, false, is_function);
  register(vm, "is_instance", 1, false, is_instance);
  register(vm, "is_int", 1, false, is_int);
  register(vm, "is_iterable", 1, false, is_iterable);
  register(vm, "is_list", 1, false, is_list);
  register(vm, "is_number", 1, false, is_number);
  register(vm, "is_object", 1, false, is_object);
  register(vm, "is_string", 1, false, is_string);
}

fn register(vm: &mut VM, name: &'static str, min_arity: u8, variadic: bool, func: NativeFn) {
  let native = NativeFunction {
    is_method: false,
    name,
    min_arity,
    variadic,
    func,
  };
  let value = vm.heap.alloc_native(native);
  vm.define_global(name, value);
}

fn time(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 0);

  let now = SystemTime::now().duration_since(UNIX_EPOCH).map_err(|e| {
    format!(
      "time() failed: system clock is set before the Unix epoch: {}",
      e
    )
  })?;
  Ok(Value::number(now.as_secs_f64()))
}

fn sum(ctx: &mut ZuriContext) -> Result<Value, String> {
  let mut total = 0.0;
  for (i, v) in ctx.args.iter().enumerate() {
    if !v.is_number() {
      return Err(format!(
        "sum() expects numbers, argument {} is a {}",
        i + 1,
        v.type_name()
      ));
    }
    total += v.as_number();
  }
  Ok(Value::number(total))
}

fn instance_of(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 2);
  enforce_arg_type!(ctx, 1, ArgType::Class);

  let value = ctx.args[0];
  let target_class = ctx.args[1];

  if !value.is_instance() {
    return Ok(Value::bool(false));
  }

  let mut cur = Some(value.as_instance().class);
  while let Some(c) = cur {
    if c.equals(&target_class) {
      return Ok(Value::bool(true));
    }
    cur = c.as_class().superclass;
  }

  Ok(Value::bool(false))
}

fn bytes(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 1);
  enforce_arg_type_any_of!(ctx, 0, [ArgType::Number, ArgType::List]);

  let v = ctx.args[0];
  if v.is_number() {
    let bytes = ctx.heap().alloc_bytes(vec![0; v.as_number() as usize]);
    return Ok(bytes);
  } else if v.is_list() {
    let is_valid_list = v
      .as_list()
      .iter()
      .all(|f| f.is_number() && 0.0 <= f.as_number() && f.as_number() <= 255.0);

    if !is_valid_list {
      return Err(format!(
        "bytes() expects a list of numbers, got {}",
        v.type_name()
      ));
    }

    let bytes = ctx.heap().alloc_bytes(
      v.as_list()
        .iter()
        .map(|f| f.as_number() as u8)
        .collect::<Vec<_>>(),
    );

    return Ok(bytes);
  }

  return Err(format!(
    "bytes() expects a number or list, got {}",
    v.type_name()
  ));
}

fn file(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_range!(ctx, 1, 2);
  enforce_arg_type!(ctx, 0, ArgType::String);
  enforce_arg_type_opt!(ctx, 1, ArgType::String);

  let path = ctx.args[0].as_str().to_string();
  let mode = if let Some(v) = ctx.args.get(1) {
    v.as_str()
  } else {
    "r"
  }
  .to_string();

  let binary = mode.to_lowercase().contains("b");

  let v = ctx.heap().alloc_file(FileHandle {
    path,
    mode,
    binary,
    handle: None,
    is_stream: false,
    fd: -1,
  });

  Ok(v)
}

fn typeof_fn(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 1);
  let v = ctx.args[0].argument_type_name();
  Ok(ctx.heap().alloc_string(v))
}

// delprop / getprop / hasprop / setprop: instances here have a fixed,
// slot-indexed field layout resolved once at class-declaration time
// (see `ObjClass::field_slots`), not an open/dynamic property bag --
// so none of these can introduce a field that wasn't already declared
// (via `var`, `const`, or an implicit `self.x = ...` in a constructor).
// `setprop`/`delprop` report that via a `false` return, matching the
// documented "if the property already exists" contract; `delprop`
// "deletes" by resetting the slot to `nil` rather than shrinking the
// instance's layout.

fn field_slot(ctx: &ZuriContext, idx: usize, name_idx: usize) -> Option<u16> {
  let inst = ctx.args[idx].as_instance();
  let name = ctx.args[name_idx].as_str();
  inst.class.as_class().field_slots.get(name).copied()
}

fn getprop(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 2);
  enforce_arg_type!(ctx, 0, ArgType::Instance);
  enforce_arg_type!(ctx, 1, ArgType::String);

  match field_slot(ctx, 0, 1) {
    Some(idx) => Ok(ctx.args[0].as_instance().fields[idx as usize].get()),
    None => Ok(Value::nil()),
  }
}

fn hasprop(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 2);
  enforce_arg_type!(ctx, 0, ArgType::Instance);
  enforce_arg_type!(ctx, 1, ArgType::String);

  Ok(Value::bool(field_slot(ctx, 0, 1).is_some()))
}

fn setprop(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 3);
  enforce_arg_type!(ctx, 0, ArgType::Instance);
  enforce_arg_type!(ctx, 1, ArgType::String);

  match field_slot(ctx, 0, 1) {
    Some(idx) => {
      let value = ctx.args[2];
      ctx.args[0].as_instance().fields[idx as usize].set(value);
      write_barrier(ctx.args[0].as_obj());
      Ok(Value::bool(true))
    },
    None => Ok(Value::bool(false)),
  }
}

fn delprop(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 2);
  enforce_arg_type!(ctx, 0, ArgType::Instance);
  enforce_arg_type!(ctx, 1, ArgType::String);

  match field_slot(ctx, 0, 1) {
    Some(idx) => {
      ctx.args[0].as_instance().fields[idx as usize].set(Value::nil());
      write_barrier(ctx.args[0].as_obj());
      Ok(Value::bool(true))
    },
    None => Ok(Value::bool(false)),
  }
}

/// For a heap object, the object's own CURRENT address. Stable for as
/// long as it's alive AND has already been promoted to the old
/// generation (old-gen objects never move again, see `object::Heap`'s
/// own docs); but NOT guaranteed stable across a garbage collection
/// for an object that's still young: a minor collection can relocate
/// it, changing what this returns for the exact same logical object.
/// `id(x) == id(x)` still always holds for two calls with no
/// collection in between, and in practice most objects an id is ever
/// taken of are either short-lived (the comparison never outlives the
/// collection anyway) or already old by the time anyone calls this on
/// them; but it's no longer the unconditional, permanent guarantee
/// this once was, and callers relying on an id surviving indefinitely
/// (e.g. as a long-lived cache key) should be aware. Primitives
/// (number/bool/nil) have no heap identity, so a deterministic numeric
/// encoding of their own value stands in instead; good enough for
/// "is this the same value", just not a real memory address, and
/// entirely unaffected by any of the above.
fn id_fn(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 1);
  let v = ctx.args[0];

  let n = if v.is_obj() {
    v.as_obj() as usize as f64
  } else if v.is_number() {
    v.as_number().to_bits() as f64
  } else if v.is_bool() {
    if v.as_bool() { 1.0 } else { 0.0 }
  } else {
    // nil
    -1.0
  };

  Ok(Value::number(n))
}

fn get_stdout_buffer_capacity() -> usize {
  std::env::var("ZURI_STDOUT_BUFFER_SIZE")
    .ok()
    .and_then(|s| s.parse::<usize>().ok())
    .unwrap_or(65536)
}

thread_local! {
  pub(crate) static STDOUT_BUFFER: std::cell::RefCell<std::io::BufWriter<std::io::Stdout>> =
    std::cell::RefCell::new(std::io::BufWriter::with_capacity(
      get_stdout_buffer_capacity(),
      std::io::stdout(),
    ));

  /// Open `io.capture()` frames, innermost last. While any frame is
  /// open, everything Zuri writes to stdout lands in the innermost
  /// one's buffer instead of the terminal.
  ///
  /// Thread-local, like the stdout buffer itself, so an isolate
  /// capturing its own output never redirects another isolate's.
  static CAPTURE_FRAMES: std::cell::RefCell<Vec<Vec<u8>>> =
    const { std::cell::RefCell::new(Vec::new()) };

  /// `CAPTURE_FRAMES.len()`, mirrored so the test on every single
  /// `echo` is one read rather than a `RefCell` borrow.
  static CAPTURE_DEPTH: Cell<usize> = const { Cell::new(0) };
}

/// How many `io.capture()` frames are open right now. Anything
/// writing to stdout checks this first; zero is the ordinary case and
/// means write straight to the terminal.
#[inline]
pub fn capture_depth() -> usize {
  CAPTURE_DEPTH.with(Cell::get)
}

/// Opens a capture frame. Output written from here until the matching
/// `capture_end()` is buffered instead of printed.
///
/// The real stdout is flushed first so anything already buffered
/// belongs to the terminal, not to the capture.
pub fn capture_begin() {
  flush_stdout();
  CAPTURE_FRAMES.with(|frames| frames.borrow_mut().push(Vec::new()));
  CAPTURE_DEPTH.with(|d| d.set(d.get() + 1));
}

/// Closes the innermost capture frame and hands back what it caught,
/// or `None` when nothing was open.
pub fn capture_end() -> Option<Vec<u8>> {
  let frame = CAPTURE_FRAMES.with(|frames| frames.borrow_mut().pop());
  if frame.is_some() {
    CAPTURE_DEPTH.with(|d| d.set(d.get() - 1));
  }
  frame
}

/// Appends to the innermost open capture frame. Callers check
/// `capture_depth()` first; with no frame open this drops the bytes,
/// which is why it is never the only path to stdout.
pub fn capture_write(bytes: &[u8]) {
  CAPTURE_FRAMES.with(|frames| {
    if let Some(frame) = frames.borrow_mut().last_mut() {
      frame.extend_from_slice(bytes);
    }
  });
}

/// Closes every still-open capture frame, printing what each caught.
///
/// A script that calls `io.capture_begin()` and never reaches its
/// `capture_end()` (it raised, or it just forgot) would otherwise have
/// that output vanish with no trace at all. Called once where the
/// program actually ends.
pub fn capture_unwind_all() {
  let frames = CAPTURE_FRAMES.with(|frames| std::mem::take(&mut *frames.borrow_mut()));
  CAPTURE_DEPTH.with(|d| d.set(0));

  if frames.is_empty() {
    return;
  }

  STDOUT_BUFFER.with(|buf_cell| {
    let mut stdout = buf_cell.borrow_mut();
    for frame in frames {
      let _ = stdout.write_all(&frame);
    }
    let _ = stdout.flush();
  });
}

#[inline]
pub fn flush_stdout() {
  STDOUT_BUFFER.with(|buf| {
    let _ = buf.borrow_mut().flush();
  });
}

/// Emits `echo`'s value followed by a newline into the shared stdout buffer and flushes.
/// `VM::echo` sends here only a value with no `@to_string()` in it.
#[inline]
pub fn echo_value(v: Value) {
  if capture_depth() > 0 {
    capture_write(format!("{}\n", v).as_bytes());
    return;
  }

  STDOUT_BUFFER.with(|buf_cell| {
    let mut stdout = buf_cell.borrow_mut();
    let _ = writeln!(stdout, "{}", v);
    let _ = stdout.flush();
  });
}

/// `echo_value` for text already rendered by `VM::render_printable`.
pub fn echo_text(text: &str) {
  if capture_depth() > 0 {
    capture_write(format!("{}\n", text).as_bytes());
    return;
  }

  STDOUT_BUFFER.with(|buf_cell| {
    let mut stdout = buf_cell.borrow_mut();
    let _ = writeln!(stdout, "{}", text);
    let _ = stdout.flush();
  });
}

/// Whether showing `v` runs Zuri code: it is, or holds somewhere inside
/// its lists and dictionaries, an instance whose class defines
/// `@to_string()`. Everything else prints straight from `Display`.
pub fn shows_through_to_string(v: Value) -> bool {
  if !v.is_obj() {
    return false;
  }
  if v.is_instance() {
    return v
      .as_instance()
      .class
      .as_class()
      .methods
      .contains_key("@to_string");
  }
  if v.is_list() {
    return v.with_list(|items| items.iter().any(|&item| shows_through_to_string(item)));
  }
  if v.is_dict() {
    return v.with_dict(|storage| {
      storage
        .entries
        .iter()
        .any(|&(k, item)| shows_through_to_string(k) || shows_through_to_string(item))
    });
  }
  false
}

/// Unlike `echo` (which always appends a newline and only ever prints
/// one value), `print()` writes every argument back-to-back with no
/// separator and no trailing newline; and, critically, writes a
/// `bytes` object as RAW bytes rather than its `Display` text. That
/// raw-byte path is what lets a script stream binary output (e.g. a
/// PBM/PNG image body one scanline at a time).
///
/// An instance is shown through its `@to_string()` exactly as `echo`
/// shows it. Every argument is rendered before anything is written, so
/// a `@to_string()` that raises leaves no partial output behind.
fn print_fn(ctx: &mut ZuriContext) -> Result<Value, String> {
  if ctx.args.iter().any(|&v| shows_through_to_string(v)) {
    let rendered = render_print_args(ctx)?;
    if capture_depth() > 0 {
      capture_write(&rendered);
      return Ok(Value::nil());
    }
    return STDOUT_BUFFER.with(|buf_cell| {
      let mut stdout = buf_cell.borrow_mut();
      stdout.write_all(&rendered).map_err(|e| e.to_string())?;
      Ok(Value::nil())
    });
  }

  if capture_depth() > 0 {
    let mut buf: Vec<u8> = Vec::new();
    write_print_args(&mut buf, ctx.args)?;
    capture_write(&buf);
    return Ok(Value::nil());
  }

  STDOUT_BUFFER.with(|buf_cell| -> Result<Value, String> {
    let mut stdout = buf_cell.borrow_mut();
    write_print_args(&mut *stdout, ctx.args)?;
    Ok(Value::nil())
  })
}

/// `print()`'s arguments when at least one of them runs a `@to_string()`.
/// The arguments are pinned for the whole render, since any one of
/// those calls can collect and move the others.
fn render_print_args(ctx: &mut ZuriContext) -> Result<Vec<u8>, String> {
  let count = ctx.args.len();
  let mark = ctx.vm.pin_values(ctx.args.iter().copied());
  let mut out: Vec<u8> = Vec::new();

  for i in 0..count {
    let v = ctx.vm.pinned(mark + i);
    if v.is_bytes() {
      v.with_bytes(|raw| out.extend_from_slice(raw));
    } else if v.is_string() {
      out.extend_from_slice(v.as_str().as_bytes());
    } else {
      match ctx.vm.render_printable(v) {
        Ok(text) => out.extend_from_slice(text.as_bytes()),
        Err(e) => {
          ctx.vm.unpin(mark);
          return Err(ctx.vm.rethrow(e));
        },
      }
    }
  }

  ctx.vm.unpin(mark);
  Ok(out)
}

/// `print()`'s formatting rules, factored out so the capture path and
/// the terminal path cannot drift apart on what a bytes or a string
/// argument turns into.
fn write_print_args(out: &mut impl Write, args: &[Value]) -> Result<(), String> {
  for v in args.iter() {
    if v.is_bytes() {
      v.with_bytes(|raw| out.write_all(raw))
        .map_err(|e| e.to_string())?;
    } else if v.is_string() {
      let s = v.as_str();
      out.write_all(s.as_bytes()).map_err(|e| e.to_string())?;
    } else {
      write!(out, "{}", v).map_err(|e| e.to_string())?;
    }
  }
  Ok(())
}

thread_local! {
  /// Self-seeded xorshift64 state; no external RNG crate needed.
  /// Seeded once per thread from the system clock; reseeding on every
  /// call would make consecutive `rand()`s within the same nanosecond
  /// (entirely possible on a fast loop) return identical values.
  static RNG_STATE: Cell<u64> = Cell::new(seed_rng());
}

fn seed_rng() -> u64 {
  let nanos = SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .map(|d| d.as_nanos() as u64)
    .unwrap_or(0x853c49e6748fea9b);
  // xorshift64 can't start at 0; fold in a fixed odd constant so a
  // clock read of exactly 0 (or any degenerate value) still seeds a
  // usable, non-zero state.
  (nanos ^ 0x2545_f491_4f6c_dd1d) | 1
}

fn next_u64() -> u64 {
  RNG_STATE.with(|state| {
    let mut x = state.get();
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    state.set(x);
    x
  })
}

/// Uniform double in `[0, 1)`, built from the top 53 bits of a
/// xorshift64 draw; the standard "enough bits for an f64 mantissa"
/// trick.
fn next_unit_f64() -> f64 {
  (next_u64() >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
}

/// `rand()` -> `[0, 1)`; `rand(x)` -> `[0, x)`; `rand(x, y)` -> a
/// value between `x` and `y` regardless of which is larger (matching
/// range-direction conventions, e.g. `range.within`).
fn rand_fn(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_range!(ctx, 0, 2);
  enforce_arg_type_opt!(ctx, 0, ArgType::Number);
  enforce_arg_type_opt!(ctx, 1, ArgType::Number);

  match ctx.args.len() {
    0 => Ok(Value::number(next_unit_f64())),
    1 => {
      let x = ctx.args[0].as_number();
      Ok(Value::number(next_unit_f64() * x))
    },
    _ => {
      let x = ctx.args[0].as_number();
      let y = ctx.args[1].as_number();
      let (lo, hi) = if x <= y { (x, y) } else { (y, x) };
      Ok(Value::number(lo + next_unit_f64() * (hi - lo)))
    },
  }
}

// is_* type predicates

fn is_bigint(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 1);
  Ok(Value::bool(ctx.args[0].is_bigint()))
}

fn is_bool(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 1);
  Ok(Value::bool(ctx.args[0].is_bool()))
}

fn is_bytes(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 1);
  Ok(Value::bool(ctx.args[0].is_bytes()))
}

/// Classes, closures, natives, and bound methods are all callable via
/// `Instr::Call`; matches `Value::is_callable()` exactly.
fn is_callable(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 1);
  Ok(Value::bool(ctx.args[0].is_callable()))
}

fn is_class(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 1);
  Ok(Value::bool(ctx.args[0].is_class()))
}

fn is_dict(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 1);
  Ok(Value::bool(ctx.args[0].is_dict()))
}

fn is_file(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 1);
  Ok(Value::bool(ctx.args[0].is_file()))
}

/// Narrower than `is_callable`: a function/closure/native/bound
/// method, but NOT a class; matches every one of `Value::type_name`'s
/// "function" cases.
fn is_function(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 1);
  let v = ctx.args[0];
  Ok(Value::bool(
    v.is_closure() || v.is_native() || v.is_bound_method(),
  ))
}

fn is_instance(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 1);
  Ok(Value::bool(ctx.args[0].is_instance()))
}

fn is_int(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 1);
  let v = ctx.args[0];
  Ok(Value::bool(v.is_number() && v.as_number().fract() == 0.0))
}

/// Matches this VM's actual iteration protocol; `@key`/`@value`
/// (see `for`-loop desugaring in the parser, and
/// `range.rs`/`list.rs`/etc.'s own `@key`/`@value` natives).
fn is_iterable(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 1);
  let v = ctx.args[0];
  let result = v.is_list()
    || v.is_dict()
    || v.is_string()
    || v.is_bytes()
    || v.is_range()
    || (v.is_instance() && {
      let class = v.as_instance().class.as_class();
      class.methods.contains_key("@key") && class.methods.contains_key("@value")
    });
  Ok(Value::bool(result))
}

fn is_list(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 1);
  Ok(Value::bool(ctx.args[0].is_list()))
}

fn is_number(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 1);
  Ok(Value::bool(ctx.args[0].is_number()))
}

/// Any heap-allocated value; strings, lists, dicts, bytes, ranges,
/// instances, classes, functions, files, bigints. Excludes only the
/// three NaN-boxed non-pointer singletons: number, bool, nil.
fn is_object(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 1);
  Ok(Value::bool(ctx.args[0].is_obj()))
}

fn is_string(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 1);
  Ok(Value::bool(ctx.args[0].is_string()))
}
