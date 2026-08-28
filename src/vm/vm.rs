use std::borrow::Cow;
use std::cell::Cell;
use std::io::IsTerminal;
use std::ops::{Neg, Shl, Shr};
use std::rc::Rc;
use std::sync::LazyLock;
use std::sync::atomic::Ordering;

use nu_ansi_term::{Color, Style};
use num_bigint::BigInt;
use num_traits::ToPrimitive;
use rustc_hash::FxHashMap;

use crate::builtins;
use crate::jit::{CompileFacts, EntryFn, background, escape, typeflow};
use crate::vm::chunk::{Instr, JumpKey, ParamType};
use crate::vm::natives;
use crate::vm::object::{
  Heap, ListStorage, NativeFunction, Obj, ObjClass, ObjClosure, ObjFunction, ObjModuleBinding,
  UpvalueDescriptor, UpvalueState, ZuriContext, write_barrier,
};
use crate::vm::value::Value;

/// Interpreted recursion never touches the native stack, only heap-bounded
/// register windows, but a call into compiled code is a real native call.
/// Past this depth we stop handing out compiled entry points and let the
/// interpreter take over, so deep recursion just gets slower instead of
/// blowing the OS stack.
const MAX_JIT_CALL_DEPTH: u32 = 1024;

/// Bounds nested `resolve_possible_deopt` calls, not lifetime deopt count;
/// a function that deopts constantly but never while a previous deopt is
/// still unwinding is fine and stays compiled. What's dangerous is a
/// polymorphic call site nesting deopt-inside-deopt with no bound, each
/// level far more expensive than a normal compiled frame. Past this bound we
/// permanently drop compiled code for the innermost function, which breaks
/// the nesting since it can't be re-entered from there.
const MAX_DEOPT_REENTRANCY: u32 = 64;

static ZURI_LOG_GC: LazyLock<bool> = LazyLock::new(|| std::env::var_os("ZURI_GC_LOG").is_some());
static ZURI_JIT_ENABLED: LazyLock<bool> = LazyLock::new(|| {
  !matches!(
    std::env::var("ZURI_JIT").as_deref(),
    Ok("0") | Ok("off") | Ok("false")
  )
});
static ZURI_JIT_NO_SPECIALIZATION: LazyLock<bool> =
  LazyLock::new(|| std::env::var_os("ZURI_JIT_NO_SPECIALIZATION").is_some());

/// The `--> path:line` locator plus (if `source` is available) a
/// snippet: up to 2 lines of context on either side of `error_line`,
/// with that line marked by a `>` in the gutter instead of the usual
/// blank space. There's no column tracking at runtime; only a
/// per-instruction *line* table (`Chunk::lines`); so a gutter marker
/// on the whole line is the most precise pointer available here, unlike
/// the parser's own single-column caret.
fn render_error_site(path: &str, error_line: u32, source: Option<&str>, use_color: bool) -> String {
  let locator_style = if use_color {
    Style::new().fg(Color::Cyan)
  } else {
    Style::new()
  };
  let mut out = format!("  {} {}:{}\n", locator_style.paint("-->"), path, error_line);

  let Some(source) = source else {
    return out;
  };
  let lines: Vec<&str> = source.lines().collect();
  if error_line == 0 || error_line as usize > lines.len() {
    return out;
  }

  let error_idx = (error_line - 1) as usize;
  let start = error_idx.saturating_sub(2);
  let end = (error_idx + 2).min(lines.len() - 1);
  let gutter_width = (end + 1).to_string().len();

  let marker_style = if use_color {
    Style::new().fg(Color::Red).bold()
  } else {
    Style::new()
  };
  let dim_style = if use_color {
    Style::new().dimmed()
  } else {
    Style::new()
  };

  out.push('\n');
  for (i, line) in lines.iter().enumerate().take(end + 1).skip(start) {
    let n = i + 1;
    if i == error_idx {
      out.push_str(&format!(
        "{} {:>width$} | {}\n",
        marker_style.paint(">"),
        n,
        line,
        width = gutter_width
      ));
    } else {
      out.push_str(&format!(
        "{}\n",
        dim_style.paint(format!("  {:>width$} | {}", n, line, width = gutter_width))
      ));
    }
  }

  out
}

/// "Stack trace (most recent call last):" plus one line per frame,
/// each showing the function name and its `path:line`. Deep recursion
/// can produce hundreds of frames that are all noise past the first
/// handful, so anything beyond a small head+tail collapses into a
/// single "N more frames" marker; the same idea behind Node's
/// `Error.stackTraceLimit` and Python's recursion-trimmed tracebacks.
fn render_stacktrace(locations: &[(Rc<str>, u32, String)], use_color: bool) -> String {
  // Program entry first, working down to the exact line that raised --
  // `frame_locations` (and `e.stacktrace`, its Zuri-visible sibling)
  // stay innermost-first internally, but displaying it that way reads
  // backwards: it restates the error site as the very first line, right
  // after the header/snippet above already pointed at it, then works
  // outward to entry. This instead reads as a narrative; "started
  // here, called this, called this, and broke on the line highlighted
  // above"; ending exactly where the snippet already landed.
  let ordered: Vec<(Rc<str>, u32, String)> = locations.iter().cloned().collect();

  // Give more of the truncation budget to the frames nearest the error
  // (now at the END of `ordered`) than the ones nearest entry, since
  // those are the ones actually useful for a deep call chain; the
  // reverse split from before the reorder.
  const HEAD: usize = 3;
  const TAIL: usize = 10;

  let header_style = if use_color {
    Style::new().bold()
  } else {
    Style::new()
  };
  let name_style = if use_color {
    Style::new().fg(Color::Yellow)
  } else {
    Style::new()
  };
  let loc_style = if use_color {
    Style::new().dimmed()
  } else {
    Style::new()
  };

  let render_frame = |(path, line, name): &(Rc<str>, u32, String)| -> String {
    format!(
      "  at {} {}\n",
      name_style.paint(format!("{}()", name)),
      loc_style.paint(format!("{}:{}", path, line))
    )
  };

  let mut out = format!(
    "{}\n",
    header_style.paint("Stack trace (most recent call last):")
  );

  if ordered.len() <= HEAD + TAIL + 1 {
    for loc in &ordered {
      out.push_str(&render_frame(loc));
    }
  } else {
    for loc in &ordered[..HEAD] {
      out.push_str(&render_frame(loc));
    }
    let omitted = ordered.len() - HEAD - TAIL;
    out.push_str(&format!(
      "  {}\n",
      loc_style.paint(format!("... {} more frames ...", omitted))
    ));
    for loc in &ordered[ordered.len() - TAIL..] {
      out.push_str(&render_frame(loc));
    }
  }

  out
}

/// Inline slots for a call's argument list before spilling to a `Vec`.
/// Covers the overwhelming majority of native/constructor calls with zero
/// heap allocation; kept small because every slot is zero-initialized
/// regardless of actual arg count.
const INLINE_ARGS: usize = 8;

pub(crate) enum CallArgs {
  Inline([Value; INLINE_ARGS], usize),
  Spilled(Vec<Value>),
}

impl CallArgs {
  #[inline]
  pub(crate) fn new() -> Self {
    CallArgs::Inline([Value::nil(); INLINE_ARGS], 0)
  }

  #[inline]
  pub(crate) fn push(&mut self, v: Value) {
    match self {
      CallArgs::Inline(buf, len) if *len < INLINE_ARGS => {
        buf[*len] = v;
        *len += 1;
      },
      CallArgs::Inline(buf, len) => {
        let mut spilled = buf[..*len].to_vec();
        spilled.push(v);
        *self = CallArgs::Spilled(spilled);
      },
      CallArgs::Spilled(vec) => vec.push(v),
    }
  }

  #[inline]
  fn extend_from_slice(&mut self, vs: &[Value]) {
    for &v in vs {
      self.push(v);
    }
  }

  #[inline]
  pub(crate) fn as_slice(&self) -> &[Value] {
    match self {
      CallArgs::Inline(buf, len) => &buf[..*len],
      CallArgs::Spilled(vec) => vec.as_slice(),
    }
  }
}

/// A growable `CallFrame` stack, hand-rolled instead of `Vec<CallFrame>`
/// for the same reason `vm::list::ListStorage` exists instead of
/// `Vec<Value>`: `jit::codegen`'s inline call/return fast path
/// (`emit_self_call`/`emit_known_call`) needs to push and pop a frame
/// with no `jit::runtime` call at all for the common case, and `Vec`'s
/// own field layout isn't something generated code should assume.
/// Unlike `ListStorage` this doesn't need `#[repr(C)]` or hand-picked
/// offsets; `CallFrame` and `FrameStack` are plain structs read only
/// from within this same build, so `std::mem::offset_of!` (see
/// `VM_FRAMES_*_OFFSET` below) already gives generated code the real,
/// compiler-chosen layout directly, the same way `VM::regs_ptr_cache`/
/// `VM::jit_ip` already are.
///
/// `CallFrame` is `Copy`-safe (every field is a raw pointer, a `Value`,
/// or a plain integer; nothing here owns a destructor), so growing is
/// a realloc-and-memcpy and dropping is a single `dealloc`, exactly
/// like `ListStorage`.
struct FrameStack {
  ptr: *mut CallFrame,
  len: usize,
  cap: usize,
}

impl FrameStack {
  const fn new() -> FrameStack {
    FrameStack {
      ptr: std::ptr::null_mut(),
      len: 0,
      cap: 0,
    }
  }

  #[inline]
  fn len(&self) -> usize {
    self.len
  }

  #[allow(unused)]
  #[inline]
  fn is_empty(&self) -> bool {
    self.len == 0
  }

  fn layout_for(cap: usize) -> std::alloc::Layout {
    std::alloc::Layout::array::<CallFrame>(cap)
      .expect("zuri: call-frame stack capacity overflowed the address space")
  }

  /// Doubling growth with a floor of 64; deep recursion should reach
  /// its steady-state depth within its first handful of reallocations,
  /// not one frame at a time.
  #[cold]
  #[inline(never)]
  fn grow(&mut self) {
    let new_cap = if self.cap == 0 { 64 } else { self.cap * 2 };
    let new_layout = FrameStack::layout_for(new_cap);
    let new_ptr = if self.ptr.is_null() {
      unsafe { std::alloc::alloc(new_layout) }
    } else {
      let old_layout = FrameStack::layout_for(self.cap);
      unsafe { std::alloc::realloc(self.ptr as *mut u8, old_layout, new_layout.size()) }
    };
    if new_ptr.is_null() {
      std::alloc::handle_alloc_error(new_layout);
    }
    self.ptr = new_ptr as *mut CallFrame;
    self.cap = new_cap;
  }

  #[inline]
  fn push(&mut self, frame: CallFrame) {
    if self.len == self.cap {
      self.grow();
    }
    unsafe { self.ptr.add(self.len).write(frame) };
    self.len += 1;
  }

  #[inline]
  fn pop(&mut self) -> Option<CallFrame> {
    if self.len == 0 {
      return None;
    }
    self.len -= 1;
    Some(unsafe { self.ptr.add(self.len).read() })
  }

  #[inline]
  fn last(&self) -> Option<&CallFrame> {
    if self.len == 0 {
      return None;
    }
    Some(unsafe { &*self.ptr.add(self.len - 1) })
  }

  #[inline]
  fn last_mut(&mut self) -> Option<&mut CallFrame> {
    if self.len == 0 {
      return None;
    }
    Some(unsafe { &mut *self.ptr.add(self.len - 1) })
  }

  #[inline]
  fn get(&self, idx: usize) -> Option<&CallFrame> {
    if idx >= self.len {
      return None;
    }
    Some(unsafe { &*self.ptr.add(idx) })
  }

  #[inline]
  fn get_mut(&mut self, idx: usize) -> Option<&mut CallFrame> {
    if idx >= self.len {
      return None;
    }
    Some(unsafe { &mut *self.ptr.add(idx) })
  }

  #[inline]
  fn truncate(&mut self, len: usize) {
    if len < self.len {
      self.len = len;
    }
  }

  #[inline]
  fn clear(&mut self) {
    self.len = 0;
  }

  fn iter(&self) -> std::slice::Iter<'_, CallFrame> {
    self.as_slice().iter()
  }

  fn iter_mut(&mut self) -> std::slice::IterMut<'_, CallFrame> {
    self.as_slice_mut().iter_mut()
  }

  fn as_slice(&self) -> &[CallFrame] {
    if self.ptr.is_null() {
      &[]
    } else {
      unsafe { std::slice::from_raw_parts(self.ptr, self.len) }
    }
  }

  fn as_slice_mut(&mut self) -> &mut [CallFrame] {
    if self.ptr.is_null() {
      &mut []
    } else {
      unsafe { std::slice::from_raw_parts_mut(self.ptr, self.len) }
    }
  }
}

impl std::ops::Index<usize> for FrameStack {
  type Output = CallFrame;
  #[inline]
  fn index(&self, idx: usize) -> &CallFrame {
    self.get(idx).expect("zuri: call-frame index out of bounds")
  }
}

impl std::ops::IndexMut<usize> for FrameStack {
  #[inline]
  fn index_mut(&mut self, idx: usize) -> &mut CallFrame {
    self
      .get_mut(idx)
      .expect("zuri: call-frame index out of bounds")
  }
}

impl<'a> IntoIterator for &'a FrameStack {
  type Item = &'a CallFrame;
  type IntoIter = std::slice::Iter<'a, CallFrame>;
  fn into_iter(self) -> Self::IntoIter {
    self.iter()
  }
}

impl<'a> IntoIterator for &'a mut FrameStack {
  type Item = &'a mut CallFrame;
  type IntoIter = std::slice::IterMut<'a, CallFrame>;
  fn into_iter(self) -> Self::IntoIter {
    self.iter_mut()
  }
}

impl Drop for FrameStack {
  fn drop(&mut self) {
    if !self.ptr.is_null() {
      unsafe { std::alloc::dealloc(self.ptr as *mut u8, FrameStack::layout_for(self.cap)) };
    }
  }
}

/// `#[repr(C)]`-free by design: see `FrameStack`'s own docs on why a
/// plain struct read only from within this build doesn't need one.
/// Field offsets `jit::codegen`'s inline call/return path reads or
/// writes directly are exposed below as `CALL_FRAME_*_OFFSET` consts,
/// verified against real values by `frame_stack_tests`.
struct CallFrame {
  function: *const ObjFunction,
  /// The closure instance this frame is executing, for GetUpval/SetUpval/
  /// Closure. Distinct from `function` (the shared prototype) the same way
  /// `ObjClosure` differs from `ObjFunction`.
  closure: *const ObjClosure,
  /// Same closure as `closure` but as the tagged `Value` the GC root scan
  /// walks. `function`/`closure` stay raw pointers so hot dispatch skips
  /// the tag check on every fetch.
  closure_val: Value,
  ip: usize,
  /// Index into `VM::registers` where this frame's register window starts.
  base: usize,
  /// Register in the *caller's* window that the return value should be
  /// written to. Unused for the outermost frame.
  dst_in_caller: u8,
  /// `VM::jit_scalar_roots.len()` at the moment this frame was pushed.
  /// Every frame-removal path truncates `jit_scalar_roots` back to this,
  /// so a scalar-replaced allocation stops being a GC root exactly when its
  /// native frame goes away, on any exit path.
  scalar_roots_mark: usize,
  /// True while this frame is running compiled code rather than
  /// `run_until`'s interpreter loop. An interpreted frame's `ip` is always
  /// current; compiled code only publishes its position to `VM::jit_ip`
  /// before a helper call that could raise or push a frame (see
  /// `jit::codegen::FuncCompiler::call_helper`), so this flag tells
  /// `build_stacktrace`/`setup_closure_call` which source to trust.
  compiled: bool,
}

/// Byte offsets of every `CallFrame` field `jit::codegen`'s inline
/// call/return fast path constructs or reads directly; one
/// `offset_of!` per field, real for this build regardless of `repr`
/// (see `FrameStack`'s own docs). `size_of::<CallFrame>()` (exposed as
/// `CALL_FRAME_SIZE`) is what the same code multiplies a frame-stack
/// index by to get a byte address.
pub(crate) const CALL_FRAME_FUNCTION_OFFSET: usize = std::mem::offset_of!(CallFrame, function);
pub(crate) const CALL_FRAME_CLOSURE_OFFSET: usize = std::mem::offset_of!(CallFrame, closure);
pub(crate) const CALL_FRAME_CLOSURE_VAL_OFFSET: usize =
  std::mem::offset_of!(CallFrame, closure_val);
pub(crate) const CALL_FRAME_IP_OFFSET: usize = std::mem::offset_of!(CallFrame, ip);
pub(crate) const CALL_FRAME_BASE_OFFSET: usize = std::mem::offset_of!(CallFrame, base);
pub(crate) const CALL_FRAME_DST_IN_CALLER_OFFSET: usize =
  std::mem::offset_of!(CallFrame, dst_in_caller);
pub(crate) const CALL_FRAME_SCALAR_ROOTS_MARK_OFFSET: usize =
  std::mem::offset_of!(CallFrame, scalar_roots_mark);
pub(crate) const CALL_FRAME_COMPILED_OFFSET: usize = std::mem::offset_of!(CallFrame, compiled);
pub(crate) const CALL_FRAME_SIZE: usize = std::mem::size_of::<CallFrame>();

#[cfg(test)]
mod frame_stack_tests {
  use super::*;

  /// Cross-checks every `CALL_FRAME_*_OFFSET`/`CALL_FRAME_SIZE` against
  /// a real `CallFrame`, the same way `obj_repr_tests` verifies
  /// `object.rs`'s own offset claims; `jit::codegen`'s inline
  /// call/return path reads and writes through these exact numbers with
  /// no further check at runtime, so a silent drift here would be a
  /// memory-safety bug, not a wrong answer.
  #[test]
  fn call_frame_offsets_match_real_frame() {
    let frame = CallFrame {
      function: 0x1000 as *const ObjFunction,
      closure: 0x2000 as *const ObjClosure,
      closure_val: Value::number(7.0),
      ip: 11,
      base: 22,
      dst_in_caller: 33,
      scalar_roots_mark: 44,
      compiled: true,
    };
    let base_addr = &frame as *const CallFrame as usize;
    unsafe {
      assert_eq!(
        *((base_addr + CALL_FRAME_FUNCTION_OFFSET) as *const *const ObjFunction),
        frame.function
      );
      assert_eq!(
        *((base_addr + CALL_FRAME_CLOSURE_OFFSET) as *const *const ObjClosure),
        frame.closure
      );
      assert_eq!(
        (*((base_addr + CALL_FRAME_CLOSURE_VAL_OFFSET) as *const Value)).as_number(),
        7.0
      );
      assert_eq!(*((base_addr + CALL_FRAME_IP_OFFSET) as *const usize), 11);
      assert_eq!(*((base_addr + CALL_FRAME_BASE_OFFSET) as *const usize), 22);
      assert_eq!(
        *((base_addr + CALL_FRAME_DST_IN_CALLER_OFFSET) as *const u8),
        33
      );
      assert_eq!(
        *((base_addr + CALL_FRAME_SCALAR_ROOTS_MARK_OFFSET) as *const usize),
        44
      );
      assert_eq!(
        *((base_addr + CALL_FRAME_COMPILED_OFFSET) as *const bool),
        true
      );
    }
  }

  /// Confirms `FrameStack::push` and a real `Vec<CallFrame>` agree on
  /// ordering/content across a grow-triggering run, and that
  /// `pop`/`truncate`/indexing behave the same as their `Vec`
  /// counterparts; `jit::codegen`'s inline fast path only ever pushes
  /// one at a time and never triggers `grow` itself (it falls back to
  /// the slow path instead), but every OTHER frame push in this VM
  /// (interpreted calls, the JIT's own slow-path helpers) goes through
  /// this same `push`, so it has to be correct at real scale, not just
  /// for a handful of frames.
  #[test]
  fn frame_stack_matches_vec_semantics() {
    let mut stack = FrameStack::new();
    let mut model: Vec<usize> = Vec::new();
    for i in 0..300usize {
      let frame = CallFrame {
        function: std::ptr::null(),
        closure: std::ptr::null(),
        closure_val: Value::number(i as f64),
        ip: i,
        base: i,
        dst_in_caller: 0,
        scalar_roots_mark: i,
        compiled: false,
      };
      stack.push(frame);
      model.push(i);
      assert_eq!(stack.len(), model.len());
      assert_eq!(stack[i].ip, i);
      assert_eq!(stack.last().unwrap().ip, i);
    }
    for i in (0..300usize).rev() {
      assert_eq!(stack.last().unwrap().ip, i);
      let popped = stack.pop().unwrap();
      assert_eq!(popped.ip, i);
    }
    assert!(stack.is_empty());
    assert_eq!(stack.pop().map(|f| f.ip), None);
  }
}

/// One active `catch` statement's unwind target.
struct CatchHandler {
  /// `self.frames.len()` when `PushCatch` executed; the frame containing
  /// the `catch` itself is `frames[frame_depth-1]`.
  frame_depth: usize,
  /// Absolute instruction index (within that frame) to resume at.
  resume_ip: usize,
  var_reg: Option<u8>,
}

/// What the `#[cold]` error path hands back to `run_until` so it can
/// refresh its cached frame-state locals, or propagate, without that logic
/// living in the cold path itself.
enum ErrorOutcome {
  Handled {
    frame_idx: usize,
    base: usize,
    func_ptr: *const ObjFunction,
    closure_ptr: *const ObjClosure,
    ip: usize,
  },
  Propagate(Value),
}

pub struct VM {
  pub(crate) is_repl: bool,
  /// One flat register stack shared by every call frame; each frame claims
  /// a slice of it (its "window"), like Lua's VM.
  registers: Vec<Value>,
  /// Mirrors `registers.as_mut_ptr()`, updated wherever `registers` can
  /// reallocate (see `sync_regs_ptr_cache`). Lets compiled code re-fetch
  /// the registers pointer with a direct load at a baked offset
  /// (`VM_REGS_PTR_CACHE_OFFSET`) instead of an FFI call, which matters
  /// since it's refetched at every helper-call site.
  regs_ptr_cache: Cell<*mut Value>,
  /// Mirrors `registers.len()`, updated in lockstep with `regs_ptr_cache`
  /// (see `sync_regs_ptr_cache`). Lets `jit::codegen`'s inline call fast
  /// path check "does the callee's register window already fit" with a
  /// direct load instead of a helper call; the callee's own required
  /// window size is a compile-time constant there (`ObjFunction::
  /// num_registers`, known statically for a `CallTarget::Known`/self-
  /// recursive callee), so this one runtime value is all that's missing.
  regs_len_cache: Cell<usize>,
  /// Upvalues still Open, as (absolute register index, the Obj::Upvalue
  /// Value there). A new closure capturing a local reuses an already-open
  /// entry for that register instead of duplicating it, which is what lets
  /// two closures over the same variable see each other's writes.
  open_upvalues: Vec<(usize, Value)>,
  /// Mirrors `!open_upvalues.is_empty()`, updated at `open_upvalues`'s
  /// only two mutation sites (`capture_upvalue`, `close_upvalues_from
  /// _slow`). `jit::codegen`'s inline call/return fast path reads this
  /// to skip `close_upvalues_from` entirely for the overwhelmingly
  /// common case; a function whose returning frame never had any of
  /// its locals captured; with no helper call.
  has_open_upvalues: Cell<bool>,
  frames: FrameStack,
  /// Backing storage for every global, indexed by slot. Slots are assigned
  /// lazily on first resolution (`get_or_create_global_slot`) and never
  /// reused, so `Chunk::global_cache` can cache a slot index permanently.
  global_slots: Vec<Cell<Value>>,
  /// Mirrors `global_slots.as_ptr()`, same pattern as `regs_ptr_cache`: lets
  /// compiled `GetGlobal` fast paths index current storage with a direct
  /// load instead of a helper call.
  global_slots_ptr_cache: Cell<*const Cell<Value>>,
  /// name -> slot, consulted only on a `global_cache` miss, i.e. the first
  /// time a given Get/Set/AssignGlobal instruction ever executes. Later
  /// executions go straight through the cache.
  global_names: FxHashMap<String, u32>,
  /// Extra GC roots for values internal VM code needs alive across a call
  /// that might itself collect (e.g. `instantiate` running several field
  /// initializers). A Value sitting only in a local Rust variable is
  /// invisible to the normal root scan; push it here for as long as it
  /// needs to survive, then truncate back off.
  gc_pins: Vec<Value>,
  /// `(base pointer, element count)` for every scalar-replaced allocation a
  /// JIT-compiled function currently has live in its own Cranelift stack
  /// frame. Retired in lockstep with the owning frame via
  /// `CallFrame::scalar_roots_mark`.
  ///
  /// Each entry is `count` plain `Value` slots, not a heap `Obj` — there's
  /// no `GcBox` header to recover, so a fake-`Obj` representation would
  /// hand the ordinary root-scanning machinery a pointer into garbage stack
  /// bytes. `collect_minor`/`collect_garbage` instead treat each entry like
  /// an extra `gc_pins` run: `forward_slot`/`mark_root` applied directly to
  /// each slot.
  jit_scalar_roots: Vec<(*mut Value, usize)>,
  /// Mirrors `jit_scalar_roots.len()`, updated at every one of its
  /// mutation sites (`push_scalar_root`, `pop_frame_inner`'s truncate,
  /// the catch-handler unwind truncate, `clear_frames`). `jit::codegen`'s
  /// inline call fast path reads this directly to fill a freshly pushed
  /// `CallFrame::scalar_roots_mark` with no helper call.
  jit_scalar_roots_len: Cell<usize>,
  /// Active `catch` handlers, innermost last.
  catch_stack: Vec<CatchHandler>,
  /// Handle to the single background compiler thread, lazily spawned
  /// alongside `jit_engine`. `None` until the first function crosses its
  /// warmup threshold.
  jit_compiler: Option<background::JitCompilerHandle>,
  /// For isolate VMs: a shared background compiler job sender and reply channel.
  shared_compiler: Option<(
    std::sync::mpsc::Sender<background::CompileJob>,
    std::sync::mpsc::Receiver<background::CompileResult>,
    std::sync::mpsc::Sender<background::CompileResult>,
    std::sync::Arc<std::sync::atomic::AtomicBool>,
  )>,
  /// GC roots for every function with a background compile enqueued or in
  /// flight, pinned from job submission until `drain_jit_results` — the
  /// compiled-code round trip crosses a thread boundary the GC can't
  /// otherwise see into.
  pending_jit_compiles: Vec<Value>,
  /// Side channel a `jit::runtime` helper sets to a non-nil error when
  /// it needs to propagate a failure out of executing compiled code, which
  /// has no unwinder of its own. `VM::invoke_compiled` checks this right
  /// after a compiled call returns to decide `Ok`/`Err`. Always nil outside
  /// that brief window.
  pub(crate) jit_pending_error: Cell<Value>,
  /// Side channel a `jit::runtime` deopt helper sets to the bytecode `ip`
  /// compiled code should resume interpreting at. Unlike
  /// `jit_pending_error` this means "nothing went wrong, just stop
  /// speculating" — VM registers already hold the correct state since
  /// compiled code keeps them live throughout, so resuming is just handing
  /// control to the interpreter at this `ip`. Checked before the error
  /// channel since a deopt isn't an error. Always `-1` outside the brief
  /// window between a helper setting it and `invoke_compiled` clearing
  /// it; `i64`, not `Option<usize>`, specifically so `jit::codegen`'s
  /// inline call/return fast path can check "no deopt pending" with one
  /// direct load and a compare against `-1` instead of a helper call;
  /// `Option<usize>` has no spare niche to read that way from generated
  /// code (see `UpvalueState`'s own docs for the general reasoning).
  pub(crate) pending_deopt_ip: Cell<i64>,
  /// Master JIT on/off switch, read once from `ZURI_JIT` at startup — a
  /// benchmarking/debugging escape hatch. Programs behave identically
  /// either way, just slower with it off.
  jit_enabled: bool,
  /// How many compiled-function calls are nested on the native call stack
  /// right now: see `MAX_JIT_CALL_DEPTH`.
  jit_call_depth: Cell<u32>,
  /// How many `resolve_possible_deopt` calls are nested on the native call
  /// stack right now: see `MAX_DEOPT_REENTRANCY`.
  deopt_reentrancy_depth: Cell<u32>,
  /// Cached by name after `prelude::install` runs, so `VM::raise` gets O(1)
  /// lookup instead of a globals hashmap hit on every internal error.
  pub(crate) builtin_errors: FxHashMap<&'static str, Value>,
  /// One shared, immortal `Value` per ASCII character, built on first use —
  /// what `s[i]` returns instead of allocating a fresh one-character string
  /// every time.
  ///
  /// Zuri strings are immutable and compare by content with no identity
  /// operator, so sharing is unobservable; it just removes an allocation
  /// (and the GC pressure it caused — a measurable chunk of
  /// `benchmarks/fasta.zu`'s runtime).
  ///
  /// Empty until the first ASCII character is indexed.
  pub(crate) interned_ascii: [Value; 128],
  /// Every module loaded so far, keyed by canonical path (or
  /// `"builtin:NAME"`). Makes re-importing a no-op and is what breaks
  /// circular imports (see `vm::modules::load_from_candidate`). Also a GC
  /// root: a cached module must stay alive for a later `import` to find it.
  pub(crate) modules: FxHashMap<String, Value>,
  /// The application's entry-file path, set by `set_root_path` — becomes
  /// every module's `__root__`. `None` in REPL mode, per spec.
  pub(crate) root_path: Option<String>,
  pub heap: Heap,
  log_gc: bool,
  no_jit_specialization: bool,
  /// Per-opcode execution counts, gathered only under ZURI_OPCODE_PROFILE.
  #[cfg(feature = "opcode-profile")]
  opcode_counts: FxHashMap<&'static str, u64>,
  /// Counts of consecutive opcode pairs, for spotting instruction-fusion
  /// candidates that are actually adjacent in real bytecode.
  #[cfg(feature = "opcode-profile")]
  opcode_bigrams: FxHashMap<(&'static str, &'static str), u64>,
  #[cfg(feature = "opcode-profile")]
  last_opcode: Option<&'static str>,
  /// Bumped by every `SetMethod` execution, never decremented. Invalidates
  /// `emit_self_invoke`'s baked "receiver's class == this compiled method's
  /// owning class" guard, which only holds as long as the class's method
  /// table hasn't been monkey-patched since the compile that baked it
  /// (extension classes can legally call `SetMethod` on an already-live
  /// class at any point). `resolve_self_class` snapshots this counter at
  /// compile time; `emit_self_invoke`'s guard compares the current counter
  /// against that snapshot and falls back to the general resolver on a
  /// mismatch.
  ///
  /// A single global counter, not per-class, since `SetMethod` is rare in
  /// ordinary programs; the coarser invalidation costs nothing in
  /// practice.
  method_table_generation: Cell<u64>,
  /// Where compiled code currently is, as a bytecode index ONE PAST the
  /// instruction being executed; the same convention `CallFrame::ip`
  /// uses, so `build_stacktrace`'s `saturating_sub(1)` applies
  /// unchanged to either.
  ///
  /// Written by generated code (at `VM_JIT_IP_OFFSET`) before every
  /// helper call, which is exactly the set of points a compiled frame
  /// can raise or become a CALLER of a new frame; the only two ways
  /// its position ever becomes observable. Only meaningful for a frame
  /// whose `CallFrame::compiled` is set: see that field's own docs.
  jit_ip: usize,
}

/// Byte offset of `VM::heap` within `VM`; combined in `crate::jit` with
/// `object::HEAP_BYTES_ALLOCATED_OFFSET`/`HEAP_NEXT_GC_OFFSET` so compiled
/// code can inline `Heap::needs_gc()`'s check (a plain integer compare)
/// as two direct loads instead of an unconditional FFI call at every
/// safepoint. Sound within this compilation: `offset_of!` asks the
/// compiler for VM's actual layout rather than assuming one.
pub(crate) const VM_HEAP_OFFSET: usize = std::mem::offset_of!(VM, heap);
/// Byte offset of `VM::regs_ptr_cache`: see that field's own docs.
pub(crate) const VM_REGS_PTR_CACHE_OFFSET: usize = std::mem::offset_of!(VM, regs_ptr_cache);
/// Byte offset of `VM::jit_ip`: see that field's own docs.
pub(crate) const VM_JIT_IP_OFFSET: usize = std::mem::offset_of!(VM, jit_ip);
/// Byte offset of `VM::global_slots_ptr_cache`: see that field's own docs.
pub(crate) const VM_INTERNED_ASCII_OFFSET: usize = std::mem::offset_of!(VM, interned_ascii);
pub(crate) const VM_GLOBAL_SLOTS_PTR_CACHE_OFFSET: usize =
  std::mem::offset_of!(VM, global_slots_ptr_cache);
/// Byte offset of `VM::method_table_generation`: see that field's own
/// docs.
pub(crate) const VM_METHOD_TABLE_GENERATION_OFFSET: usize =
  std::mem::offset_of!(VM, method_table_generation);

/// Byte offset of `VM::frames` itself. Combined in `jit::codegen` with
/// `CALL_FRAME_*_OFFSET`/`FRAMESTACK_*_OFFSET` (below) so the inline
/// call/return fast path can read/write the frame stack's `ptr`/`len`
/// and construct a whole `CallFrame` in place, with no `jit::runtime`
/// call for the common case: see `FrameStack`'s own docs.
pub(crate) const VM_FRAMES_OFFSET: usize = std::mem::offset_of!(VM, frames);
/// Byte offset of `FrameStack::ptr` within `FrameStack` itself; added
/// to `VM_FRAMES_OFFSET` by `jit::codegen`, not usable alone.
pub(crate) const FRAMESTACK_PTR_OFFSET: usize = std::mem::offset_of!(FrameStack, ptr);
/// Byte offset of `FrameStack::len` within `FrameStack` itself; same
/// deal as `FRAMESTACK_PTR_OFFSET`.
pub(crate) const FRAMESTACK_LEN_OFFSET: usize = std::mem::offset_of!(FrameStack, len);
/// Byte offset of `FrameStack::cap` within `FrameStack` itself; same
/// deal as `FRAMESTACK_PTR_OFFSET`.
pub(crate) const FRAMESTACK_CAP_OFFSET: usize = std::mem::offset_of!(FrameStack, cap);
/// Byte offset of `VM::regs_len_cache`: see that field's own docs.
pub(crate) const VM_REGS_LEN_CACHE_OFFSET: usize = std::mem::offset_of!(VM, regs_len_cache);
/// Byte offset of `VM::jit_scalar_roots_len`: see that field's own
/// docs.
pub(crate) const VM_JIT_SCALAR_ROOTS_LEN_OFFSET: usize =
  std::mem::offset_of!(VM, jit_scalar_roots_len);
/// Byte offset of `VM::has_open_upvalues`: see that field's own docs.
pub(crate) const VM_HAS_OPEN_UPVALUES_OFFSET: usize = std::mem::offset_of!(VM, has_open_upvalues);
/// Byte offset of `VM::pending_deopt_ip`: see that field's own docs.
pub(crate) const VM_PENDING_DEOPT_IP_OFFSET: usize = std::mem::offset_of!(VM, pending_deopt_ip);
/// Byte offset of `VM::jit_pending_error`: see that field's own
/// docs.
pub(crate) const VM_JIT_PENDING_EXCEPTION_OFFSET: usize =
  std::mem::offset_of!(VM, jit_pending_error);
/// Byte offset of `VM::jit_call_depth`: see that field's own docs.
pub(crate) const VM_JIT_CALL_DEPTH_OFFSET: usize = std::mem::offset_of!(VM, jit_call_depth);
/// `MAX_JIT_CALL_DEPTH` itself, re-exported so `jit::codegen`'s inline
/// depth check compares against the exact same bound `jit_depth_ok`
/// does, with no risk of the two drifting apart.
pub(crate) const JIT_MAX_CALL_DEPTH: u32 = MAX_JIT_CALL_DEPTH;

type RunResult<T> = Result<T, Value>;

impl VM {
  pub fn new(heap: Heap) -> Self {
    let mut vm = VM {
      is_repl: false,
      registers: Vec::new(),
      regs_ptr_cache: Cell::new(std::ptr::null_mut()),
      regs_len_cache: Cell::new(0),
      global_slots_ptr_cache: Cell::new(std::ptr::null()),
      frames: FrameStack::new(),
      jit_ip: 0,
      open_upvalues: Vec::new(),
      has_open_upvalues: Cell::new(false),
      gc_pins: Vec::new(),
      jit_scalar_roots: Vec::new(),
      jit_scalar_roots_len: Cell::new(0),
      catch_stack: Vec::new(),
      jit_compiler: None,
      shared_compiler: None,
      pending_jit_compiles: Vec::new(),
      jit_pending_error: Cell::new(Value::nil()),
      pending_deopt_ip: Cell::new(-1),
      jit_enabled: *ZURI_JIT_ENABLED,
      no_jit_specialization: *ZURI_JIT_NO_SPECIALIZATION,
      jit_call_depth: Cell::new(0),
      deopt_reentrancy_depth: Cell::new(0),
      builtin_errors: FxHashMap::default(),
      interned_ascii: [Value::nil(); 128],
      global_slots: Vec::new(),
      global_names: FxHashMap::default(),
      modules: FxHashMap::default(),
      root_path: None,
      #[cfg(feature = "opcode-profile")]
      opcode_counts: FxHashMap::default(),
      #[cfg(feature = "opcode-profile")]
      opcode_bigrams: FxHashMap::default(),
      #[cfg(feature = "opcode-profile")]
      last_opcode: None,
      log_gc: *ZURI_LOG_GC,
      heap,
      method_table_generation: Cell::new(0),
    };
    for b in 0u8..128 {
      vm.interned_ascii[b as usize] = vm.heap.alloc_old(Obj::Str(String::from(b as char)));
    }
    vm
  }

  pub fn init(&mut self) {
    natives::install(self);
    crate::vm::prelude::install(self);
  }

  #[inline]
  pub fn heap_mut(&mut self) -> &mut Heap {
    &mut self.heap
  }

  pub fn set_repl_mode(&mut self) {
    self.is_repl = true;
  }

  /// Bind a value directly, useful for wiring up a top-level function
  /// (e.g. "fib") before `run` starts executing.
  pub fn define_global(&mut self, name: impl Into<String>, v: Value) {
    let slot = self.get_or_create_global_slot(name.into());
    self.global_slots[slot as usize].set(v);
  }

  /// Get `name`'s slot, allocating a fresh one (initialized to nil)
  /// if it doesn't have one yet.
  fn get_or_create_global_slot(&mut self, name: String) -> u32 {
    if let Some(&slot) = self.global_names.get(&name) {
      return slot;
    }
    let slot = self.global_slots.len() as u32;
    self.global_slots.push(Cell::new(Value::nil()));
    self.sync_global_slots_ptr_cache();
    self.global_names.insert(name, slot);
    slot
  }

  #[inline]
  pub fn lookup_global(&self, name: &str) -> Option<Value> {
    let &slot = self.global_names.get(name)?;
    Some(self.global_slots[slot as usize].get())
  }

  /// Resolve `name` to a slot in whichever globals table `module` names
  /// (`None` = the VM's root table), growing only that table if `name`
  /// hasn't been seen there. Keeps modules with colliding names from
  /// stepping on each other.
  pub(crate) fn get_or_create_slot_in(&mut self, module: Option<Value>, name: String) -> u32 {
    match module {
      None => self.get_or_create_global_slot(name),
      Some(m) => m.as_module_mut().namespace.get_or_create_slot(&name),
    }
  }

  pub(crate) fn lookup_slot_in(&self, module: Option<Value>, name: &str) -> Option<u32> {
    match module {
      None => self.global_names.get(name).copied(),
      Some(m) => m.as_module().namespace.names.get(name).copied(),
    }
  }

  #[inline]
  #[allow(unused)]
  fn read_slot_in(&self, module: Option<Value>, slot: u32) -> Value {
    match module {
      None => self.global_slots[slot as usize].get(),
      Some(m) => m.as_module().namespace.slots[slot as usize].get(),
    }
  }

  #[inline]
  pub(crate) fn write_slot_in(&self, module: Option<Value>, slot: u32, v: Value) {
    match module {
      None => self.global_slots[slot as usize].set(v),
      Some(m) => {
        m.as_module().namespace.slots[slot as usize].set(v);
        write_barrier(m.as_obj());
      },
    }
  }

  /// Resolve `name` against `module`'s own namespace first, falling back to
  /// the shared root table. Lets module code see builtins and the prelude's
  /// Error hierarchy while still letting a module shadow those names.
  pub(crate) fn resolve_global(&self, module: Option<Value>, name: &str) -> Option<(bool, u32)> {
    match module {
      None => self.global_names.get(name).copied().map(|s| (true, s)),
      Some(m) => {
        if let Some(&s) = m.as_module().namespace.names.get(name) {
          return Some((false, s));
        }
        self.global_names.get(name).copied().map(|s| (true, s))
      },
    }
  }

  #[inline]
  pub(crate) fn read_resolved(&self, module: Option<Value>, is_root: bool, slot: u32) -> Value {
    if is_root {
      self.global_slots[slot as usize].get()
    } else {
      module.unwrap().as_module().namespace.slots[slot as usize].get()
    }
  }

  #[inline]
  pub(crate) fn write_resolved(&self, module: Option<Value>, is_root: bool, slot: u32, v: Value) {
    if is_root {
      self.global_slots[slot as usize].set(v);
    } else {
      let m = module.unwrap();
      m.as_module().namespace.slots[slot as usize].set(v);
      write_barrier(m.as_obj());
    }
  }

  /// Records the entry-file path; every module loaded afterward gets this
  /// as its own `__root__`. Call before `run`; not used in REPL mode.
  pub fn set_root_path(&mut self, path: impl Into<String>) {
    self.root_path = Some(path.into());
  }

  /// Seeds `__file__`/`__root__` into the VM's root global table, making
  /// them visible to the main script as if it were a module. `import`ed
  /// modules get their own copies via `vm::modules::seed_module_vars`
  /// instead. Not called in REPL mode, per the "not defined in REPL"
  /// spec for `__root__`.
  pub fn init_entry_globals(&mut self, file_path: &str) {
    let file_val = self.heap.alloc_string(file_path.to_string());
    self.define_global("__file__", file_val);
    if let Some(root) = self.root_path.clone() {
      let root_val = self.heap.alloc_string(root);
      self.define_global("__root__", root_val);
    }
  }

  /// Constructs a builtin error instance directly by field slot,
  /// bypassing the normal constructor path since these are always known
  /// prelude classes. Every internal VM error site calls this instead of
  /// returning a bare Rust string.
  pub(crate) fn raise(&mut self, class_name: &'static str, message: impl Into<String>) -> Value {
    let message_str = message.into();
    let class_val = *self.builtin_errors.get(class_name).unwrap_or_else(|| {
      panic!(
        "internal error: unknown builtin error class '{}' (prelude not installed?)",
        class_name
      )
    });

    let field_count = class_val.as_class().field_count;
    let instance = self.heap.alloc_instance(class_val, field_count as usize);
    let message_val = self.heap.alloc_string(message_str);
    let type_val = self.heap.alloc_string(class_name.to_string());

    {
      let class = class_val.as_class();
      let inst = instance.as_instance();
      if let Some(&idx) = class.field_slots.get("message") {
        inst.fields[idx as usize].set(message_val);
      }
      if let Some(&idx) = class.field_slots.get("type") {
        inst.fields[idx as usize].set(type_val);
      }
    }

    self.attach_stacktrace(instance)
  }

  /// Is `v` an instance of `Error` or a subclass? What `Instr::Raise`
  /// checks before letting a value propagate as an error.
  fn is_error_value(&self, v: Value) -> bool {
    if !v.is_instance() {
      return false;
    }
    let Some(&error_class) = self.builtin_errors.get("Error") else {
      return false;
    };
    let mut cur = Some(v.as_instance().class);
    while let Some(c) = cur {
      if c.equals(&error_class) {
        return true;
      }
      cur = c.as_class().superclass;
    }
    false
  }

  /// Does `v` satisfy one member of an `Instr::CheckParamType`'s type
  /// list: see `ParamType`'s own docs for what each variant means.
  /// Every variant but `Instance` is a pure `Value`/`Obj` tag test that
  /// can't fail; `Instance` needs a global lookup for the named class
  /// (cached in `func.chunk.global_cache`, keyed by `instr_ip`, exactly
  /// like an ordinary `Instr::GetGlobal` at that position would be), so
  /// it can fail with an `UndefinedError` if the name was never bound.
  pub(crate) fn param_type_matches(
    &mut self,
    v: Value,
    t: ParamType,
    func: &ObjFunction,
    instr_ip: usize,
  ) -> Result<bool, Value> {
    Ok(match t {
      ParamType::Bool => v.is_bool(),
      ParamType::Int => v.is_number() && v.as_number().fract() == 0.0,
      ParamType::Number => v.is_number(),
      ParamType::BigInt => v.is_bigint(),
      ParamType::String => v.is_string(),
      ParamType::Bytes => v.is_bytes(),
      ParamType::List => v.is_list(),
      ParamType::Dict => v.is_dict(),
      ParamType::Range => v.is_range(),
      ParamType::File => v.is_file(),
      // Narrower than `is_callable`: excludes a class (that's `Class`).
      ParamType::Function => v.is_closure() || v.is_native() || v.is_bound_method(),
      ParamType::Class => v.is_class(),
      ParamType::Callable => v.is_callable(),
      // Matches this VM's actual iteration protocol (`@key`/`@value`),
      // same check `natives::is_iterable` makes.
      ParamType::Iterable => {
        v.is_list()
          || v.is_dict()
          || v.is_string()
          || v.is_bytes()
          || v.is_range()
          || (v.is_instance() && {
            let class = v.as_instance().class.as_class();
            class.methods.contains_key("@key") && class.methods.contains_key("@value")
          })
      },
      ParamType::Instance(name_const) => {
        if !v.is_instance() {
          false
        } else {
          let gmod = func.globals_module;
          let (is_root, slot) =
            if let Some(&cached) = func.chunk.global_cache.borrow().get(&instr_ip) {
              cached
            } else {
              let name_val = func.chunk.constants[name_const as usize];
              let resolved = match self.resolve_global(gmod, name_val.as_str()) {
                Some(r) => r,
                None => {
                  let msg = format!("undefined global '{}'", name_val.as_str());
                  return Err(self.raise("UndefinedError", msg));
                },
              };
              func
                .chunk
                .global_cache
                .borrow_mut()
                .insert(instr_ip, resolved);
              resolved
            };
          let target_class = self.read_resolved(gmod, is_root, slot);
          target_class.is_class() && {
            let mut cur = Some(v.as_instance().class);
            let mut matched = false;
            while let Some(c) = cur {
              if c.equals(&target_class) {
                matched = true;
                break;
              }
              cur = c.as_class().superclass;
            }
            matched
          }
        }
      },
    })
  }

  /// Raw (path, line, function-name) per live frame, innermost first --
  /// shared by `build_stacktrace` (which formats these into the
  /// "path:line -> name()" strings Zuri-level `catch` handlers see on
  /// `e.stacktrace`) and `format_uncaught`'s CLI rendering, which needs
  /// the pieces unformatted so it can pull matching source lines for a
  /// snippet.
  fn frame_locations(&self) -> Vec<(Rc<str>, u32, String)> {
    let mut out = Vec::with_capacity(self.frames.len());

    let innermost = self.frames.len().saturating_sub(1);
    for (idx, frame) in self.frames.iter().enumerate().rev() {
      let func = unsafe { &*frame.function };
      // Only the innermost frame needs `jit_ip`: every compiled frame
      // further out already had its position committed to its own `ip`
      // by `setup_closure_call`, at the moment it became a caller.
      let ip = if frame.compiled && idx == innermost {
        self.jit_ip
      } else {
        frame.ip
      };
      let line = func
        .chunk
        .lines
        .get(ip.saturating_sub(1))
        .copied()
        .unwrap_or(0);
      out.push((func.source_path.clone(), line, func.name.clone()));
    }

    out
  }

  /// Frame names, innermost first, as a Zuri list of strings, attached to
  /// every error's `stacktrace` field.
  fn build_stacktrace(&mut self) -> Value {
    let locations = self.frame_locations();
    let mut lines = Vec::with_capacity(locations.len());

    for (path, line, name) in locations {
      let entry = format!("{}:{} -> {}()", path, line, name);
      lines.push(self.heap.alloc_string(entry));
    }

    self.heap.alloc_list(lines)
  }

  fn attach_stacktrace(&mut self, instance: Value) -> Value {
    let trace = self.build_stacktrace();
    if instance.is_instance() {
      let inst = instance.as_instance();
      let idx = inst.class.as_class().field_slots.get("stacktrace").copied();
      if let Some(idx) = idx {
        inst.fields[idx as usize].set(trace);
      }
    }
    instance
  }

  /// "TYPE: message" for top-level reporting, falling back gracefully if
  /// `exc` isn't an instance.
  pub fn describe_error(&self, exc: Value) -> String {
    if !exc.is_instance() {
      return format!("{}", exc);
    }
    let inst = exc.as_instance();
    let class = inst.class.as_class();
    let message = class
      .field_slots
      .get("message")
      .map(|&idx| inst.fields[idx as usize].get().to_string())
      .unwrap_or_else(|| "An unexpected error has occurred".to_string());
    let type_name = class
      .field_slots
      .get("type")
      .map(|&idx| inst.fields[idx as usize].get().to_string())
      .unwrap_or_else(|| class.name.clone());
    format!("{}: {}", type_name, message)
  }

  /// Formats an error that occurred inside an isolate with its
  /// source snippet and stack trace.
  pub fn format_isolate_error(&self, exc: Value) -> String {
    let summary = self.describe_error(exc);
    if !exc.is_instance() {
      return summary;
    }
    let inst = exc.as_instance();
    let class = inst.class.as_class();
    let Some(&idx) = class.field_slots.get("stacktrace") else {
      return summary;
    };
    let trace_val = inst.fields[idx as usize].get();
    if !trace_val.is_list() {
      return summary;
    }
    let trace_list = trace_val.as_list();
    if trace_list.is_empty() {
      return summary;
    }

    let mut locations: Vec<(Rc<str>, u32, String)> = Vec::with_capacity(trace_list.len());
    for item in trace_list {
      if item.is_string() {
        let s = item.as_str();
        if let Some((loc, name_part)) = s.split_once(" -> ") {
          if let Some((path, line_str)) = loc.rsplit_once(':') {
            if let Ok(line) = line_str.parse::<u32>() {
              let name = name_part.strip_suffix("()").unwrap_or(name_part);
              locations.push((Rc::<str>::from(path), line, name.to_string()));
            }
          }
        }
      }
    }

    if locations.is_empty() {
      return summary;
    }

    let mut out = format!("{}\n", summary);
    if let Some((path, line, _)) = locations.first() {
      let source = std::fs::read_to_string(path.as_ref()).ok();
      out.push_str(&render_error_site(path, *line, source.as_deref(), false));
    }
    out.push('\n');
    out.push_str(&render_stacktrace(&locations, false));
    out
  }

  /// Full multi-line "Unhandled ..." block the CLI/REPL print at the top
  /// level: a header, a snippet of source around the exact line that
  /// raised (2 lines of context on each side, that line marked), and a
  /// stack trace. `describe_error` stays the short summary, still
  /// used by the prelude's internal panic path.
  ///
  /// `entry_path`/`entry_source` are the file the caller actually ran --
  /// used to render the innermost frame's snippet without re-reading it
  /// off disk. Any OTHER frame's file (reached via `import`, say) gets
  /// read fresh from disk, since the VM doesn't keep source text around
  /// once a file's compiled; a source that can't be found (a deleted
  /// file, `<repl>` for an outer frame) just falls back to the bare
  /// `path:line` locator with no snippet.
  pub fn format_uncaught(&self, exc: Value, entry_path: &str, entry_source: &str) -> String {
    let summary = self.describe_error(exc);
    let use_color = std::io::stderr().is_terminal();
    let err_style = if use_color {
      Style::new().fg(Color::Red).bold()
    } else {
      Style::new()
    };

    if !exc.is_instance() {
      return format!("{}", err_style.paint(format!("Unhandled {}", summary)));
    }

    let locations = self.frame_locations();
    let mut out = format!("{}\n", err_style.paint(format!("Unhandled {}", summary)));

    if let Some((path, line, _)) = locations.first() {
      let source = if path.as_ref() == entry_path {
        Some(Cow::Borrowed(entry_source))
      } else {
        std::fs::read_to_string(path.as_ref()).ok().map(Cow::Owned)
      };
      out.push_str(&render_error_site(
        path,
        *line,
        source.as_deref(),
        use_color,
      ));
    }

    out.push('\n');
    out.push_str(&render_stacktrace(&locations, use_color));
    out
  }

  /// Run `main` (a top-level closure, typically zero-upvalue, taking no
  /// arguments) to completion.
  pub fn run(&mut self, main: Value) -> RunResult<()> {
    let closure = main.as_closure();
    let proto = closure.function.as_func();
    let num_registers = proto.num_registers as usize;
    self.registers.resize(num_registers, Value::nil());
    self.sync_regs_ptr_cache();
    self.frames.push(CallFrame {
      function: proto as *const ObjFunction,
      closure: closure as *const ObjClosure,
      closure_val: main,
      ip: 0,
      base: 0,
      dst_in_caller: 0,
      scalar_roots_mark: self.jit_scalar_roots.len(),
      compiled: false,
    });
    let res = self.run_until(0);
    crate::vm::natives::flush_stdout();
    res?;
    Ok(())
  }

  /// Invoke any callable Value (closure or native) with already-evaluated
  /// arguments and run it to completion. Lets a native call back into Zuri
  /// code, e.g. a `map(list, fn)` calling `fn` once per element.
  pub fn call_value(&mut self, callee: Value, args: &[Value]) -> RunResult<Value> {
    if callee.is_native() {
      let native = callee.as_native();
      return self.call_native(native, args);
    }
    if !callee.is_closure() {
      let msg = format!("cannot call object of type {}", callee.type_name());
      return Err(self.raise("TypeError", msg));
    }

    let closure = callee.as_closure();
    let proto = closure.function.as_func();
    let required = if proto.variadic {
      proto.arity - 1
    } else {
      proto.arity
    };

    // Same convention as dispatch_call's Instr::Call: place the new frame
    // right after whatever's currently executing, not at registers.len().
    // Bounds growth by max call depth instead of growing once per
    // call_value invocation; no truncate-on-return needed since later
    // calls at the same depth just reuse the already-grown capacity.
    let new_base = self
      .frames
      .last()
      .map(|f| f.base + unsafe { &*f.function }.num_registers as usize)
      .unwrap_or(0);
    let needed = new_base + proto.num_registers as usize;
    if self.registers.len() < needed {
      self.registers.resize(needed, Value::nil());
      self.sync_regs_ptr_cache();
    }

    for i in 0..required as usize {
      self.registers[new_base + i] = args.get(i).copied().unwrap_or(Value::nil());
    }
    if proto.variadic {
      let extra: Vec<Value> = args.iter().skip(required as usize).copied().collect();
      let list_val = self.heap.alloc_list(extra);
      self.registers[new_base + required as usize] = list_val;
    }

    let stop_depth = self.frames.len();
    self.frames.push(CallFrame {
      function: proto as *const ObjFunction,
      closure: closure as *const ObjClosure,
      closure_val: callee,
      ip: 0,
      base: new_base,
      dst_in_caller: 0,
      scalar_roots_mark: self.jit_scalar_roots.len(),
      compiled: false,
    });
    self.run_frame(stop_depth, proto, callee)
  }

  // JIT tiering: see `crate::jit` for the compiled-code side. Every
  // entry point below assumes the caller already pushed the CallFrame
  // being executed; these methods only decide interpret-vs-compiled, never
  // frame setup.

  /// Must be called immediately after every `registers.resize(..)` with no
  /// errors; compiled code trusts this cache with a raw load and no
  /// staleness check of its own. Syncs `regs_len_cache` in the same
  /// breath, for the same reason.
  #[inline]
  fn sync_regs_ptr_cache(&mut self) {
    self.regs_ptr_cache.set(self.registers.as_mut_ptr());
    self.regs_len_cache.set(self.registers.len());
  }

  fn sync_global_slots_ptr_cache(&mut self) {
    self.global_slots_ptr_cache.set(self.global_slots.as_ptr());
  }

  /// Called by every `SetMethod` execution, interpreted or compiled
  /// (`jit::runtime::zuri_jit_set_method`).
  pub(crate) fn bump_method_table_generation(&self) {
    self
      .method_table_generation
      .set(self.method_table_generation.get() + 1);
  }

  /// Does `proto` have a compiled entry point ready right now? Never
  /// blocks: disabled JIT, an ineligible prototype, call-depth already at
  /// `MAX_JIT_CALL_DEPTH`, or not warm yet all just return `None`. Once
  /// warm, this enqueues a background compile (or finds one already in
  /// flight) but still returns `None` for the current call; the
  /// interpreter keeps running until `drain_jit_results` installs the
  /// finished entry point and later calls pick it up transparently.
  ///
  /// Hit on every `Call`/`Invoke`/`InvokeSuper`/`CallSuperCtor`, so the
  /// common already-compiled case is just an enabled-flag read, a
  /// depth-counter read, and one `Cell::get()`.
  ///
  /// `proto_value` must be the exact `Value` (`Obj::Func`-tagged) that
  /// owns `proto`; used to pin it as a GC root if this call ends up
  /// enqueueing a new compile (see `enqueue_compile`).
  #[inline]
  pub(crate) fn tiered_entry(
    &mut self,
    proto: &ObjFunction,
    proto_value: Value,
  ) -> Option<EntryFn> {
    if !self.jit_enabled || self.jit_call_depth.get() >= MAX_JIT_CALL_DEPTH {
      return None;
    }
    // Checked BEFORE draining, not after: once `proto` has an entry
    // point installed there is nothing a pending compile result could
    // change about the answer, and this is by far the common case on
    // any hot call site. Draining first would put a channel poll on
    // literally every call in the program to serve the handful of
    // calls that actually witness a compile landing.
    if let Some(entry) = proto.jit.entry.get() {
      return Some(entry);
    }
    self.drain_jit_results();
    if let Some(entry) = proto.jit.entry.get() {
      return Some(entry);
    }
    if proto.jit.ineligible.get()
      || proto.jit.compiling.get()
      || proto.jit.call_count.get() < proto.jit.call_threshold
    {
      return None;
    }
    self.enqueue_compile(proto, proto_value);
    None
  }

  pub fn set_shared_jit_compiler(
    &mut self,
    job_tx: std::sync::mpsc::Sender<background::CompileJob>,
  ) {
    if let Some((current_job_tx, _, _, _)) = &mut self.shared_compiler {
      *current_job_tx = job_tx;
    } else {
      let (reply_tx, reply_rx) = std::sync::mpsc::channel();
      let pending = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
      self.shared_compiler = Some((job_tx, reply_rx, reply_tx, pending));
    }
  }

  fn jit_compiler(&mut self) -> &mut background::JitCompilerHandle {
    if self.jit_compiler.is_none() {
      self.jit_compiler = Some(background::spawn());
    }
    self.jit_compiler.as_mut().unwrap()
  }

  /// Samples whichever instance happens to be `self` at the moment a
  /// method is queued for JIT compilation and hands back the set of its
  /// fields that hold a number right then. This is a ONE-SHOT SAMPLE of
  /// a single instance, not a proof, i.e. a different instance, or this
  /// same one after a later reassignment, can easily hold something
  /// else in the same field. `jit::codegen` may only ever fold this
  /// into `type_facts` on the SPECULATIVE side (the specialized body,
  /// guarded per-access by `emit_speculative_guard`, which re-validates
  /// the real value and deopts on a mismatch); it must never reach the
  /// general/shared body's facts, since that body has no guard
  /// mechanism at all and would treat the bet as an unconditional truth
  /// instead of the guess it actually is.
  fn resolve_self_numeric_fields(&self, proto: &ObjFunction) -> rustc_hash::FxHashSet<String> {
    let mut numeric_fields = rustc_hash::FxHashSet::default();
    let Some(frame) = self.frames.last() else {
      return numeric_fields;
    };
    if !std::ptr::eq(frame.function, proto as *const ObjFunction) {
      return numeric_fields;
    }
    let Some(self_val) = self.registers.get(frame.base) else {
      return numeric_fields;
    };
    if !self_val.is_instance() {
      return numeric_fields;
    }
    let inst = self_val.as_instance();
    let class = inst.class.as_class();
    for (name, &slot) in &class.field_slots {
      if let Some(cell) = inst.fields.get(slot as usize) {
        if cell.get().is_number() {
          numeric_fields.insert(name.clone());
        }
      }
    }
    numeric_fields
  }

  /// Resolves the class a method's `self` is guaranteed to be an instance
  /// of, then maps every field name safe to read/write on it directly (no
  /// `BoundMethod`-wrapping risk) to its slot index. `None` for a
  /// non-method function or unprovable resolution.
  ///
  /// Field slots are stable across inheritance; `Instr::Class` clones the
  /// superclass's `field_slots` wholesale and `DeclareField` only appends,
  /// never renumbers; so a slot resolved from the method's declaring
  /// class stays correct for any subclass `self` actually is at runtime.
  ///
  /// Resolving by name (`owning_class_name`) rather than back-pointer needs
  /// one extra check: the global binding, unlike the class object itself,
  /// can be reassigned after declaration. If the name no longer resolves to
  /// a class that actually owns `proto`, we return `None` and fall back to
  /// the general path.
  fn resolve_self_field_slots(&self, proto: &ObjFunction) -> Option<FxHashMap<String, u16>> {
    let class_name = proto.owning_class_name.as_ref()?;
    let (is_root, slot) = self.resolve_global(proto.globals_module, class_name)?;
    let class_val = self.read_resolved(proto.globals_module, is_root, slot);
    if !class_val.is_class() {
      return None;
    }
    let class = class_val.as_class();
    let proto_ptr = proto as *const ObjFunction;
    let owns_proto = class
      .methods
      .values()
      .any(|m| m.is_closure() && std::ptr::eq(m.as_closure().function.as_func(), proto_ptr));
    if !owns_proto {
      return None;
    }
    Some(
      class
        .field_slots
        .iter()
        .filter(|(name, _)| !class.methods.contains_key(*name))
        .map(|(name, &slot)| (name.clone(), slot))
        .collect(),
    )
  }

  /// `resolve_self_field_slots`'s counterpart for an ORDINARY (non-
  /// `self`) parameter: every `Instr::CheckParamType` whose declared
  /// type is a single, non-nullable, resolvable class, with its
  /// register never written again anywhere in the function (the same
  /// whole-bytecode over-approximation `jit::codegen::FuncCompiler::
  /// compute_proven_param_shapes` uses for the cheaper is_obj/tag-only
  /// cascade; this is its bigger sibling, resolving the FULL field
  /// layout instead of just the shape), maps to that class's own
  /// field-name -> slot table.
  ///
  /// Unlike `resolve_self_field_slots`, there's no "does this class's
  /// method table map back to `proto`" check to make; a parameter
  /// isn't `proto`'s own receiver, just some value the caller is
  /// contractually required (by `Instr::CheckParamType` already having
  /// raised otherwise) to have handed in as an instance of exactly this
  /// class. Field slots are stable across inheritance the same way
  /// `resolve_self_field_slots` relies on, so this is sound for any
  /// subclass too.
  /// Returns, per proven register, `(that class's own Value bits, its
  /// field-name -> slot table)`. The bits are what let `jit::codegen`
  /// inline `Instr::CheckParamType` ITSELF down to a plain class-bits
  /// compare (no helper call at all) for the overwhelmingly common
  /// exact-class-match case; a monomorphic hot function with an
  /// object-typed parameter otherwise pays a full opaque helper call on
  /// EVERY invocation just to verify the type, which can easily cost
  /// more than the field-access savings downstream ever recover
  /// (confirmed: `interact(bi: Body, bj: Body, dt: number)` called ~30M
  /// times measured SLOWER overall without this, ~6.6s -> ~8.3s,
  /// despite every field access on `bi`/`bj` getting cheaper). A
  /// receiver whose class is a genuine SUBCLASS of the declared one
  /// still falls to the helper (an exact-bits compare can't see
  /// inheritance); correct, just not the fast path: see
  /// `jit::codegen::emit_check_param_type`'s own docs for how the two
  /// halves (this exact-match fast path, the helper's subclass-aware
  /// slow path) fit together.
  fn resolve_param_field_slots(
    &self,
    proto: &ObjFunction,
  ) -> FxHashMap<u8, (u64, FxHashMap<String, u16>)> {
    let mut out = FxHashMap::default();
    for instr in &proto.chunk.code {
      let Instr::CheckParamType { reg, check_idx } = *instr else {
        continue;
      };
      let check = &proto.chunk.param_checks[check_idx as usize];
      if check.nullable || check.types.len() != 1 {
        continue;
      }
      let ParamType::Instance(name_const) = check.types[0] else {
        continue;
      };
      let rewritten = proto
        .chunk
        .code
        .iter()
        .any(|i| crate::jit::typeflow::any_dst(i) == Some(reg));
      if rewritten {
        continue;
      }
      let name_val = proto.chunk.constants[name_const as usize];
      let Some((is_root, slot)) = self.resolve_global(proto.globals_module, name_val.as_str())
      else {
        continue;
      };
      let class_val = self.read_resolved(proto.globals_module, is_root, slot);
      if !class_val.is_class() {
        continue;
      }
      let class = class_val.as_class();
      let slots: FxHashMap<String, u16> = class
        .field_slots
        .iter()
        .filter(|(name, _)| !class.methods.contains_key(*name))
        .map(|(name, &slot)| (name.clone(), slot))
        .collect();
      out.insert(reg, (class_val.to_bits(), slots));
    }
    out
  }

  /// `resolve_self_field_slots`'s sibling: returns `self`'s own class as
  /// `Value` bits when `proto`'s owning class's method table maps back to
  /// `proto` itself. Codegen uses this so any `Invoke` of this method,
  /// wherever the receiver register came from, can guard on the receiver's
  /// actual class matching this bit pattern at the call site.
  fn resolve_self_class(&self, proto: &ObjFunction) -> Option<(u64, u64)> {
    let class_name = proto.owning_class_name.as_ref()?;
    let (is_root, slot) = self.resolve_global(proto.globals_module, class_name)?;
    let class_val = self.read_resolved(proto.globals_module, is_root, slot);
    if !class_val.is_class() {
      return None;
    }
    let class = class_val.as_class();
    let proto_ptr = proto as *const ObjFunction;
    let owns_proto = class
      .methods
      .values()
      .any(|m| m.is_closure() && std::ptr::eq(m.as_closure().function.as_func(), proto_ptr));
    if !owns_proto {
      return None;
    }
    Some((class_val.to_bits(), self.method_table_generation.get()))
  }

  /// Resolves every `Call` site whose callee register is proven to hold an
  /// unmodified global read into a `CallTarget`, so codegen can skip
  /// `zuri_jit_call_prepare`'s resolver entirely. `SelfRecursive` needs no
  /// runtime guard; `Known` still needs a value-identity guard since an
  /// arbitrary global binding (unlike a function's own name) can be
  /// reassigned.
  ///
  /// Candidate names are collected first so the dataflow runs once per
  /// distinct name referenced, not once per instruction. Worth doing
  /// because unproven construction is a five-deep chase of dependent loads
  /// (register -> class -> constructor -> closure -> function -> variadic)
  /// per instance built, and every link is a static fact about the class.
  ///
  /// Falls back to `None` unless no class in the ancestor chain has its own
  /// field initializer, and the constructor exists as a non-variadic
  /// closure.
  ///
  /// The constructor closure's own bits are deliberately not baked in --
  /// closures are young allocations that relocate, so we read `constructor`
  /// back off the guarded class at call time instead. `proto_ptr` is baked
  /// since `ObjFunction` never moves.
  ///
  /// The paired `method_table_generation` snapshot covers a dead class's
  /// address getting recycled by a new one, same as `resolve_self_class`.
  ///
  /// `simple_ctor_param_slots` below recognizes a constructor that does
  /// nothing but copy each parameter into a field. Accepts only the exact
  /// shape `@new(x, y, z) { self.x = x; self.y = y; self.z = z }` compiles
  /// to; a run of `SetField` from parameter registers, then `LoadNil` +
  /// `Return`. Anything else (computed value, branch, call, double write)
  /// returns `None`, since a syntactic match is the only way to be certain.
  fn simple_ctor_param_slots(ctor: &ObjFunction, class: &ObjClass) -> Option<Vec<u16>> {
    let code = &ctor.chunk.code;
    if ctor.variadic || code.len() < 2 {
      return None;
    }
    // arity counts the implicit `self`, so `@new(x, y, z)` reports 4;
    // real parameters are registers 1..=params.
    let params = (ctor.arity as usize).checked_sub(1)?;
    // slots[i] is where parameter i is stored.
    let mut slots: Vec<Option<u16>> = vec![None; params];

    let mut ip = 0;
    while ip < code.len() {
      match code[ip] {
        Instr::SetField {
          obj: 0,
          name_const,
          src,
        } => {
          if src == 0 || (src as usize) > params {
            return None;
          }
          let name = ctor.chunk.constants.get(name_const as usize)?;
          if !name.is_string() {
            return None;
          }
          let slot = *class.field_slots.get(name.as_str())?;
          let param = src as usize - 1;
          // One store per parameter, no field written twice; either
          // would make the slot assignment ambiguous.
          if slots[param].is_some() || slots.iter().any(|s| *s == Some(slot)) {
            return None;
          }
          slots[param] = Some(slot);
          ip += 1;
        },
        Instr::LoadNil { dst } => {
          // Must be the trailing `LoadNil` + `Return` pair and nothing
          // more.
          if ip + 2 != code.len() {
            return None;
          }
          return match code[ip + 1] {
            Instr::Return { src } if src == dst => slots.into_iter().collect::<Option<Vec<u16>>>(),
            _ => None,
          };
        },
        _ => return None,
      }
    }
    None
  }

  fn snapshot_class_target(&self, class_val: Value) -> Option<crate::jit::ResolvedGlobal> {
    let (field_count, ctor, superclass) = {
      let class = class_val.as_class();
      if class.own_field_initializer.is_some() {
        return None;
      }
      (class.field_count, class.constructor?, class.superclass)
    };

    // An inherited field initializer disqualifies just as much as an own
    // one; `instantiate` runs every ancestor's, root to leaf.
    let mut cur = superclass;
    while let Some(c) = cur {
      let next = {
        let cobj = c.as_class();
        if cobj.own_field_initializer.is_some() {
          return None;
        }
        cobj.superclass
      };
      cur = next;
    }

    if !ctor.is_closure() {
      return None;
    }
    // Class methods are allocated straight into the old (mark-sweep,
    // non-relocating) generation, so this should never actually fail --
    // kept as a real check so a future allocation-policy change falls back
    // to the dynamic path instead of handing generated code a stale
    // pointer.
    if Heap::is_young(ctor.as_obj()) {
      return None;
    }
    let ctor_proto = ctor.as_closure().function.as_func();
    if ctor_proto.variadic {
      return None;
    }

    let class = class_val.as_class();
    Some(crate::jit::ResolvedGlobal::Class {
      guard_bits: class_val.to_bits(),
      generation: self.method_table_generation.get(),
      field_count,
      ctor_bits: ctor.to_bits(),
      proto_ptr: ctor_proto as *const ObjFunction as usize,
      safety: escape::ClassFieldSafety::from_class(&class),
      field_slots: class.field_slots.clone(),
      simple_ctor_param_slots: Self::simple_ctor_param_slots(ctor_proto, &class),
    })
  }

  fn snapshot_globals(&self, proto: &ObjFunction) -> FxHashMap<String, crate::jit::ResolvedGlobal> {
    let mut out = FxHashMap::default();
    let mut candidate_names: Vec<String> = Vec::new();
    for instr in &proto.chunk.code {
      if let Instr::GetGlobal { name_const, .. } = instr
        && let Some(v) = proto.chunk.constants.get(*name_const as usize)
        && v.is_string()
      {
        let s = v.as_str();
        if !candidate_names.iter().any(|n| n == s) {
          candidate_names.push(s.to_string());
        }
      }
    }
    for name in candidate_names {
      let Some((is_root, slot)) = self.resolve_global(proto.globals_module, &name) else {
        continue;
      };
      let resolved = self.read_resolved(proto.globals_module, is_root, slot);
      if resolved.is_class() {
        if let Some(target) = self.snapshot_class_target(resolved) {
          out.insert(name, target);
        }
      } else if resolved.is_native() {
        let n = resolved.as_native();
        out.insert(
          name,
          crate::jit::ResolvedGlobal::Native {
            guard_fn: n.func as usize as u64,
            native_ptr: n as *const NativeFunction as usize,
          },
        );
      } else if resolved.is_closure() {
        let callee_proto = resolved.as_closure().function.as_func();
        for (cip, cinstr) in callee_proto.chunk.code.iter().enumerate() {
          let name_const = match cinstr {
            crate::vm::chunk::Instr::GetGlobal { name_const, .. }
            | crate::vm::chunk::Instr::SetGlobal { name_const, .. }
            | crate::vm::chunk::Instr::AssignGlobal { name_const, .. } => *name_const,
            _ => continue,
          };
          if let Some(name_val) = callee_proto.chunk.constants.get(name_const as usize)
            && name_val.is_string()
          {
            let name = name_val.as_str();
            if let Some((is_root, slot)) = self.resolve_global(callee_proto.globals_module, name) {
              if is_root {
                callee_proto.jit.global_slot_cache[cip].set(slot as i64);
              }
            }
          }
        }
        out.insert(
          name,
          crate::jit::ResolvedGlobal::Closure {
            entry: callee_proto.jit.entry.get().map_or(0, |e| e as usize),
            guard_bits: resolved.as_closure().function.to_bits(),
            proto_ptr: callee_proto as *const ObjFunction as usize,
          },
        );
      }
    }
    out
  }

  /// Builds a snapshot of facts resolved about `proto` in $O(1)$ without
  /// performing any whole-function dataflow analyses on the main VM thread,
  /// then hands the job to the background compiler thread.
  fn enqueue_compile(&mut self, proto: &ObjFunction, proto_value: Value) {
    let (speculative_params, speculative_regs) = if self.no_jit_specialization {
      (None, None)
    } else {
      (self.combined_param_feedback(proto), None)
    };
    for (ip, instr) in proto.chunk.code.iter().enumerate() {
      let name_const = match instr {
        crate::vm::chunk::Instr::GetGlobal { name_const, .. }
        | crate::vm::chunk::Instr::SetGlobal { name_const, .. }
        | crate::vm::chunk::Instr::AssignGlobal { name_const, .. } => *name_const,
        _ => continue,
      };
      if let Some(name_val) = proto.chunk.constants.get(name_const as usize)
        && name_val.is_obj()
      {
        let name = name_val.as_str();
        if let Some((is_root, slot)) = self.resolve_global(proto.globals_module, name) {
          if is_root {
            proto.jit.global_slot_cache[ip].set(slot as i64);
          }
        }
      }
    }
    let facts = CompileFacts {
      self_field_slots: self.resolve_self_field_slots(proto).unwrap_or_default(),
      self_numeric_fields: self.resolve_self_numeric_fields(proto),
      param_field_slots: self.resolve_param_field_slots(proto),
      self_class_bits: self.resolve_self_class(proto),
      globals_snapshot: self.snapshot_globals(proto),
    };

    proto.jit.compiling.set(true);
    self.pending_jit_compiles.push(proto_value);
    let sent = if let Some((job_tx, _, reply_tx, pending)) = &self.shared_compiler {
      let job = background::CompileJob {
        proto: background::SendPtr(proto as *const ObjFunction),
        speculative_params,
        speculative_regs,
        facts,
        reply_to: Some((reply_tx.clone(), std::sync::Arc::clone(pending))),
      };
      job_tx.send(job).is_ok()
    } else {
      let job = background::CompileJob {
        proto: background::SendPtr(proto as *const ObjFunction),
        speculative_params,
        speculative_regs,
        facts,
        reply_to: None,
      };
      if let Some(tx) = self.jit_compiler().job_tx.as_ref() {
        tx.send(job).is_ok()
      } else {
        false
      }
    };
    if !sent {
      // The background thread is gone; shouldn't happen (it lives
      // for the whole process), but if it did, undo the pin/flag so
      // proto just stays interpreted forever rather than wedged in a
      // permanent "compiling" state no result will ever clear.
      proto.jit.compiling.set(false);
      self.pending_jit_compiles.pop();
    }
  }

  /// Installs every background compile result that's ready right now
  /// (non-blocking). Called at the top of `tiered_entry`/`maybe_osr` so
  /// results get installed lazily, whenever something asks, with no
  /// separate polling thread needed.
  fn drain_jit_results(&mut self) {
    let mut results = Vec::new();
    if let Some((_, reply_rx, _, pending)) = &self.shared_compiler {
      if pending.load(Ordering::Acquire) {
        pending.store(false, Ordering::Relaxed);
        while let Ok(result) = reply_rx.try_recv() {
          results.push(result);
        }
      }
    } else if let Some(handle) = self.jit_compiler.as_ref() {
      if handle.results_pending.load(Ordering::Acquire) {
        handle.results_pending.store(false, Ordering::Relaxed);
        while let Ok(result) = handle.result_rx.try_recv() {
          results.push(result);
        }
      }
    } else {
      return;
    }
    for result in results {
      // SAFETY: result.proto was pinned in pending_jit_compiles from the
      // moment its job was enqueued until right here.
      let proto = unsafe { &*result.proto.0 };
      match result.outcome {
        Ok(entry) => {
          if crate::jit::log_enabled() {
            eprintln!(
              "[jit] compiled '{}' ({} bytecode ops, {} osr point(s), speculative_params={:#x}, speculative_regs={:#x})",
              proto.name,
              proto.chunk.code.len(),
              result.osr_ids.len(),
              result.speculative_params.unwrap_or(0),
              result.speculative_regs.unwrap_or(0),
            );
          }
          *proto.jit.osr_ids.borrow_mut() = Some(result.osr_ids);
          proto.jit.entry.set(Some(entry));
        },
        Err(reason) => {
          if crate::jit::log_enabled() {
            eprintln!("[jit] '{}' ineligible: {}", proto.name, reason);
          }
          proto.jit.ineligible.set(true);
        },
      }
      proto.jit.compiling.set(false);
      if let Some(pos) = self
        .pending_jit_compiles
        .iter()
        .position(|v| std::ptr::eq(v.as_func(), proto))
      {
        self.pending_jit_compiles.swap_remove(pos);
      }
    }
  }

  /// A single-call type sample of `proto`'s fixed-arity parameters, read
  /// from the current top frame (always a frame for `proto` by the time
  /// this is called). Betting on real observed values here is sound
  /// because the resulting mask only ever seeds a guard that re-validates
  /// the registers for real before any later call trusts them.
  fn sample_param_types(&self, proto: &ObjFunction) -> Option<u64> {
    let frame = self.frames.last()?;
    if !std::ptr::eq(frame.function, proto as *const ObjFunction) {
      return None;
    }
    let required = if proto.variadic {
      proto.arity.saturating_sub(1)
    } else {
      proto.arity
    };
    let base = frame.base;
    let mut mask: u64 = 0;
    for i in 0..(required as usize).min(64) {
      let Some(v) = self.registers.get(base + i) else {
        break;
      };
      if v.is_number() {
        mask |= 1u64 << i;
      }
    }
    Some(mask)
  }

  /// A one-shot type sample of every register in `proto`'s frame (not just
  /// parameters; compare `sample_param_types`). Feeds
  /// `typeflow::SpeculativeRegs`: a register that happens to be numeric
  /// right now gets a real runtime guard planted at its definition site in
  /// the specialized body, so nothing downstream trusts this sample
  /// directly.
  ///
  /// Not accumulated across calls like parameter feedback is; a register
  /// beyond the parameter range doesn't have a stable call-independent
  /// identity, it can be a different bytecode-level variable on different
  /// calls, so continuous accumulation isn't a natural fit here.
  #[allow(dead_code)]
  fn sample_all_reg_types(&self, proto: &ObjFunction) -> Option<typeflow::SpeculativeRegs> {
    let frame = self.frames.last()?;
    if !std::ptr::eq(frame.function, proto as *const ObjFunction) {
      return None;
    }
    let base = frame.base;
    let mut mask: u64 = 0;
    for i in 0..(proto.num_registers as usize).min(64) {
      let Some(v) = self.registers.get(base + i) else {
        break;
      };
      if v.is_number() {
        mask |= 1u64 << i;
      }
    }
    Some(mask)
  }

  /// Folds one call's argument types into `proto`'s running
  /// `numeric_feedback` accumulator via bitwise AND, so a parameter's bit
  /// only survives to compile time if it was numeric on every call seen so
  /// far; the same "keep believing until contradicted" pattern as a
  /// polymorphic inline cache. Skipped once `proto` is compiled, enqueued,
  /// or ineligible, since feedback can no longer inform a decision by then.
  #[inline]
  fn record_call_feedback(&self, proto: &ObjFunction) {
    if proto.jit.entry.get().is_some() || proto.jit.compiling.get() || proto.jit.ineligible.get() {
      return;
    }
    let Some(mask) = self.sample_param_types(proto) else {
      return;
    };
    proto
      .jit
      .numeric_feedback
      .set(proto.jit.numeric_feedback.get() & mask);
    proto
      .jit
      .feedback_samples
      .set(proto.jit.feedback_samples.get().saturating_add(1));
  }

  /// The type-feedback mask consulted when `proto` is enqueued for
  /// compilation: the accumulated result from `record_call_feedback`, or a
  /// fresh one-shot sample if no call was ever recorded that way (e.g. a
  /// top-level script's OSR-triggered compile, whose outermost frame is
  /// never pushed via a "call").
  fn combined_param_feedback(&self, proto: &ObjFunction) -> Option<u64> {
    if proto.jit.feedback_samples.get() > 0 {
      Some(proto.jit.numeric_feedback.get())
    } else {
      self.sample_param_types(proto)
    }
  }

  /// Runs a just-pushed frame for `proto`/`closure_val` either interpreted
  /// or, once warm, as compiled code. Used by `call_value` so natives
  /// calling back into Zuri code benefit from tiering like ordinary
  /// `Instr::Call` does.
  fn run_frame(
    &mut self,
    stop_depth: usize,
    proto: &ObjFunction,
    closure_val: Value,
  ) -> RunResult<Value> {
    proto
      .jit
      .call_count
      .set(proto.jit.call_count.get().saturating_add(1));
    self.record_call_feedback(proto);
    let proto_value = closure_val.as_closure().function;
    if let Some(entry) = self.tiered_entry(proto, proto_value) {
      return self.invoke_compiled(entry, closure_val, -1);
    }
    self.run_until(stop_depth)
  }

  /// Pins every value in `values` into `gc_pins` for as long as the caller
  /// needs them to survive a re-entrant `call_value` (which runs arbitrary
  /// Zuri code and can trigger a collection). Returns the index the first
  /// value landed at; value `i` is at `mark + i`, and `mark` is what `unpin`
  /// needs to release them.
  ///
  /// A `Value` sitting only in a plain Rust local has no way to get
  /// rewritten if the object it names is relocated by a collection running
  /// mid-loop inside an earlier iteration's `call_value`. `gc_pins` is a
  /// real GC root, so a pinned value stays correctly updated across further
  /// re-entrant calls; as long as every read goes back through
  /// `self.gc_pins[idx]` fresh, never a copy taken before an intervening
  /// call.
  pub(crate) fn pin_values(&mut self, values: impl IntoIterator<Item = Value>) -> usize {
    let mark = self.gc_pins.len();
    self.gc_pins.extend(values);
    mark
  }

  pub(crate) fn unpin(&mut self, mark: usize) {
    self.gc_pins.truncate(mark);
  }

  /// Reads back a value pinned by `pin_values`, fresh; reflects any
  /// relocation a collection made since the pin.
  #[inline]
  pub(crate) fn pinned(&self, idx: usize) -> Value {
    self.gc_pins[idx]
  }

  /// Guarantees `closure_val` isn't `Young` before handing it to compiled
  /// code, relocating it now if it is. Codegen's `closure_param` is a
  /// Cranelift SSA value loaded once at entry and reused for the whole
  /// invocation, with no GC-scannable memory location to write a relocated
  /// address back into if the object moved mid-invocation; so instead it
  /// must never be free to move while compiled code holds it.
  ///
  /// Cheap in the common case: a closure invoked through a warm call site
  /// has almost always already survived a minor collection, costing one
  /// generation read. Only a still-young closure pays for a real collection.
  #[inline]
  pub(crate) fn ensure_stable_for_compiled_entry(&mut self, closure_val: Value) -> Value {
    // Split so the common "already old" answer inlines into the per-call
    // helpers as a single generation read, instead of a real call that
    // almost always returns immediately.
    if !Heap::is_young(closure_val.as_obj()) {
      return closure_val;
    }
    self.relocate_for_compiled_entry(closure_val)
  }

  #[cold]
  #[inline(never)]
  fn relocate_for_compiled_entry(&mut self, closure_val: Value) -> Value {
    // Must pin before collecting, not resolve through the stale pointer
    // afterward: a minor collection relocates every reachable young
    // object for real (see `Heap::forward_or_promote`), so a pointer
    // captured before one runs is stale the moment it returns. Pinning
    // first lets the same collection's own root scan update it the
    // ordinary way.
    let mark = self.pin_values([closure_val]);
    // A real, full minor collection, not an isolated relocation of just
    // this object; the closure is also reachable from wherever
    // closure_val came from (a register, a method table, a field), and
    // relocating it alone would leave those other references pointing at a
    // slot the collection has since reused.
    self.collect_minor();
    let new_val = self.pinned(mark);
    self.unpin(mark);
    new_val
  }

  /// Runs the current top frame as compiled machine code from `osr_id`
  /// (`-1` for an ordinary entry at ip 0; a non-negative id from
  /// `JitInfo::osr_ids` jumps straight into a loop header: see
  /// `maybe_osr`).
  ///
  /// On success, pops the frame and returns `Ok(value)`, same as
  /// `Instr::Return`. On failure, the frame is left in place, matching
  /// `run_until`'s "an uncaught error leaves every frame up to the
  /// catching ancestor, truncated in one shot by `handle_error`"
  /// behavior.
  fn invoke_compiled(
    &mut self,
    entry: EntryFn,
    closure_val: Value,
    osr_id: i32,
  ) -> RunResult<Value> {
    let base = self
      .frames
      .last()
      .expect("invoke_compiled: no active frame")
      .base;
    let closure_val = self.ensure_stable_for_compiled_entry(closure_val);

    self.jit_call_depth.set(self.jit_call_depth.get() + 1);
    // Restored rather than left set on the way out, since this same frame
    // can keep running interpreted afterwards, most obviously after a
    // deopt.
    let entered_idx = self.frames.len() - 1;
    let was_compiled = std::mem::replace(&mut self.frames[entered_idx].compiled, true);
    let a0 = self
      .registers
      .get(base + 0)
      .copied()
      .unwrap_or(Value::nil())
      .to_bits();
    let a1 = self
      .registers
      .get(base + 1)
      .copied()
      .unwrap_or(Value::nil())
      .to_bits();
    let a2 = self
      .registers
      .get(base + 2)
      .copied()
      .unwrap_or(Value::nil())
      .to_bits();
    let a3 = self
      .registers
      .get(base + 3)
      .copied()
      .unwrap_or(Value::nil())
      .to_bits();
    // SAFETY: entry was produced by JitEngine::compile_function for this
    // exact prototype; base is this frame's own register-window start,
    // matching the compiled calling convention.
    let result_bits = unsafe {
      entry(
        self as *mut VM,
        base as u64,
        closure_val.to_bits(),
        osr_id,
        a0,
        a1,
        a2,
        a3,
      )
    };
    self.jit_call_depth.set(self.jit_call_depth.get() - 1);
    if let Some(frame) = self.frames.get_mut(entered_idx) {
      frame.compiled = was_compiled;
    }

    // A real deopt takes priority over everything else. This check is also
    // needed by zuri_jit_call_finish, the other place compiled code's
    // return value gets processed; the compiled-to-compiled fast path
    // never goes through this function, so a deopt there would otherwise go
    // unnoticed. Both call sites must resolve a pending deopt before doing
    // anything else with compiled code's return value.
    if let Some(result) = self.resolve_possible_deopt() {
      return result;
    }

    let pending = self.jit_pending_error.get();
    if !pending.is_nil() {
      self.jit_pending_error.set(Value::nil());
      return Err(pending);
    }

    self.close_upvalues_from(base);
    self.pop_frame_inner();
    Ok(Value::from_bits(result_bits))
  }

  /// Checks (and clears) `pending_deopt_ip`. If compiled code just bailed
  /// out, the current top frame is still exactly as valid as it always
  /// was; the only thing wrong is compiled code gave up on it partway
  /// through. Point its `ip` at the deopt target and hand it to a fresh,
  /// depth-bounded interpreter run; both callers see an ordinary
  /// `RunResult<Value>` either way, deopt is fully invisible above this
  /// function.
  ///
  /// Tiny and always-inlined: the common case (no deopt pending) is one
  /// `Cell` read, kept small so it disappears into callers' hot paths
  /// instead of costing a real call every time. The rare resolution logic
  /// is kept out of line in `resolve_deopt_slow`.
  #[inline(always)]
  pub(crate) fn resolve_possible_deopt(&mut self) -> Option<RunResult<Value>> {
    let deopt_ip = self.pending_deopt_ip.replace(-1);
    if deopt_ip < 0 {
      return None;
    }
    Some(self.resolve_deopt_slow(deopt_ip as usize))
  }

  #[cold]
  #[inline(never)]
  fn resolve_deopt_slow(&mut self, deopt_ip: usize) -> RunResult<Value> {
    let frame_idx = self.frames.len() - 1;
    // SAFETY: this frame's function has been a valid, live ObjFunction for
    // as long as the frame has existed, same pointer every other unsafe
    // deref of frame.function in this file already trusts.
    let deopting_fn = unsafe { &*self.frames[frame_idx].function };
    let depth = self.deopt_reentrancy_depth.get() + 1;
    self.deopt_reentrancy_depth.set(depth);
    if depth > MAX_DEOPT_REENTRANCY {
      // This nested deopt chain is deep enough that letting it grow
      // further risks a real native stack overflow; give up on compiled
      // code for the function at this deepest level for good. Clearing
      // entry (not just ineligible) matters: tiered_entry checks entry
      // first.
      deopting_fn.jit.entry.set(None);
      deopting_fn.jit.ineligible.set(true);
      if crate::jit::log_enabled() {
        eprintln!(
          "[jit] '{}' permanently deoptimized: nested deopt depth reached {}",
          deopting_fn.name, depth
        );
      }
    }
    self.frames[frame_idx].ip = deopt_ip;
    // The interpreter takes this frame over and syncs ip on every
    // instruction from here, so jit_ip stops being the truthful source.
    self.frames[frame_idx].compiled = false;
    let stop_depth = frame_idx;
    let result = self.run_until(stop_depth);
    self
      .deopt_reentrancy_depth
      .set(self.deopt_reentrancy_depth.get() - 1);
    result
  }

  /// Checked by `run_until`'s `Instr::Jmp` handler on every backward jump
  /// (loop back-edge); `target_ip` is the loop header it lands on. `None`
  /// means keep interpreting normally; `Some(outcome)` means OSR just ran
  /// the current frame to completion and the caller must treat it like
  /// `Instr::Return`/an unhandled error firing, not resume
  /// interpreting.
  pub(crate) fn maybe_osr(
    &mut self,
    func: &ObjFunction,
    target_ip: usize,
  ) -> Option<RunResult<Value>> {
    if !self.jit_enabled
      || func.jit.ineligible.get()
      || self.jit_call_depth.get() >= MAX_JIT_CALL_DEPTH
    {
      return None;
    }
    self.drain_jit_results();

    if let Some(entry) = func.jit.entry.get() {
      let osr_id = *func.jit.osr_ids.borrow().as_ref()?.get(&target_ip)?;
      let closure_val = self.frames.last().unwrap().closure_val;
      return Some(self.invoke_compiled(entry, closure_val, osr_id));
    }
    if func.jit.compiling.get() {
      return None;
    }

    let hot = {
      let mut counts = func.jit.osr_counts.borrow_mut();
      let count = counts.entry(target_ip).or_insert(0);
      *count += 1;
      *count >= func.jit.osr_threshold
    };
    if !hot {
      return None;
    }

    // self.frames.last() is func's own currently-executing frame, since
    // this is only ever reached from a backward jump inside it.
    let closure_val = self.frames.last().unwrap().closure_val;
    let proto_value = closure_val.as_closure().function;
    self.enqueue_compile(func, proto_value);
    None
  }

  /// The one choke point every single-frame removal funnels through.
  /// Truncates `jit_scalar_roots` back to what it held before this frame
  /// was pushed, so a scalar-replaced allocation stops being a GC root the
  /// moment its native frame goes away. Multi-frame removal sites (catch
  /// unwind, `clear_frames`) do the same truncation inline instead, since
  /// they remove more than one frame at once.
  ///
  /// Also discards any `catch_stack` entry left behind by the frame
  /// that's going away without ever reaching its own `Instr::PopCatch`
  /// -- a `return` (or any other early exit) straight out of a `catch {
  /// ... }` body compiles no such cleanup of its own (see
  /// `Compiler::compile_catch`/`compile_statement`'s `Stmt::Return`
  /// arm), so without this, the handler's `CatchHandler::frame_depth`
  /// keeps pointing at a frame that no longer exists. A LATER, entirely
  /// unrelated error raised anywhere shallower than that stale depth
  /// would then find it via `handle_error`'s `frame_depth > stop_depth`
  /// check, "handle" it by resuming at `resume_ip` (an instruction
  /// index into the departed frame's OWN chunk) inside whatever frame
  /// is now current, and start executing that frame's bytecode from a
  /// meaningless offset thereby corrupting execution in a way that can
  /// look like a register holding the wrong value, a function being
  /// called twice, or a `TypeError` calling something that was never
  /// callable.
  /// A function containing `Instr::PushCatch` is never JIT-compiled
  /// (`codegen::is_eligible`), so the frame that could leave a stale
  /// handler here is always interpreted, and its `Instr::Return` always
  /// calls this function directly; there's no separate JIT-inlined
  /// frame-pop path that also needs this same cleanup.
  fn pop_frame_inner(&mut self) -> CallFrame {
    let frame = self.frames.pop().expect("pop_frame_inner: no frame to pop");
    self.jit_scalar_roots.truncate(frame.scalar_roots_mark);
    self.jit_scalar_roots_len.set(self.jit_scalar_roots.len());
    let new_len = self.frames.len();
    while matches!(self.catch_stack.last(), Some(h) if h.frame_depth > new_len) {
      self.catch_stack.pop();
    }
    frame
  }

  /// Pops the top frame with no upvalue-closing/return-value bookkeeping --
  /// used only by `zuri_jit_call_finish` and its `Invoke` counterpart,
  /// which need `frames` (private to this module) popped from outside
  /// `vm.rs`.
  pub(crate) fn pop_frame(&mut self) {
    self.pop_frame_inner();
  }

  /// Registers a scalar-replaced allocation's backing memory as a GC root.
  /// Called by `zuri_jit_push_scalar_root` only after every slot is
  /// populated; an uninitialized slot isn't a valid `Value` a GC walk
  /// could safely inspect. Retired automatically via
  /// `CallFrame::scalar_roots_mark`.
  pub(crate) fn push_scalar_root(&mut self, ptr: *mut Value, count: usize) {
    self.jit_scalar_roots.push((ptr, count));
    self.jit_scalar_roots_len.set(self.jit_scalar_roots.len());
  }

  /// Is the native call stack shallow enough for one more nested compiled
  /// call? Exposed as its own side-effect-free check, separate from
  /// `tiered_entry`, for `zuri_jit_call_prepare`/`zuri_jit_invoke_prepare`'s
  /// pure peek at whether the fast direct-call path applies; they never
  /// trigger compilation or touch `call_count` themselves.
  #[inline]
  pub(crate) fn jit_depth_ok(&self) -> bool {
    self.jit_enabled && self.jit_call_depth.get() < MAX_JIT_CALL_DEPTH
  }

  #[inline]
  pub(crate) fn jit_depth_enter(&self) {
    self.jit_call_depth.set(self.jit_call_depth.get() + 1);
  }

  /// Marks the frame just pushed by a `jit::runtime` prepare helper as
  /// running compiled code. The prepare/call_indirect/finish protocol never
  /// goes through `invoke_compiled`, so it's the one path into compiled
  /// execution that has to say so explicitly.
  #[inline]
  pub(crate) fn mark_top_frame_compiled(&mut self) {
    if let Some(frame) = self.frames.last_mut() {
      frame.compiled = true;
    }
  }

  #[inline]
  pub(crate) fn jit_depth_exit(&self) {
    self.jit_call_depth.set(self.jit_call_depth.get() - 1);
  }

  /// Sets up a new register window and pushes a `CallFrame` for calling
  /// `closure` with `num_args` argument slots already sitting at
  /// `new_base..new_base+num_args`. Shared by the interpreter's Closure
  /// call, `invoke_prebound`, and the JIT's fast direct-call path so they
  /// can't drift apart. Fills missing fixed parameters with nil and
  /// collects extra variadic arguments into a list.
  pub(crate) fn setup_closure_call(
    &mut self,
    closure_val: Value,
    closure: &ObjClosure,
    proto: &ObjFunction,
    new_base: usize,
    num_args: u8,
    dst_in_caller: u8,
  ) {
    // The frame about to become a caller stops being the innermost one, so
    // if it's compiled this is the moment its position must be committed
    // somewhere a later stack trace can find it; jit_ip only describes
    // the innermost compiled frame. An interpreted caller needs nothing
    // here since run_until syncs its ip on every instruction.
    if let Some(caller) = self.frames.last_mut()
      && caller.compiled
    {
      caller.ip = self.jit_ip;
    }

    // Split so the common shape; non-variadic callee, called at its
    // declared arity, into a register window with room to spare; is a
    // straight frame push with nothing else. The variadic branch below
    // builds a Vec and allocates a list; its mere presence gave this
    // function a 168-byte stack frame and a four-register prologue every
    // ordinary call paid for. That prologue alone measured ~13% of this
    // function's time on constructor-heavy code before the split.
    if !proto.variadic
      && num_args == proto.arity
      && self.registers.len() >= new_base + proto.num_registers as usize
    {
      self.push_frame_fast(closure_val, closure, proto, new_base, dst_in_caller);
      return;
    }
    self.setup_closure_call_slow(
      closure_val,
      closure,
      proto,
      new_base,
      num_args,
      dst_in_caller,
    );
  }

  /// `setup_closure_call`'s fast path: arguments are already in place and
  /// the window is already big enough, so setup is one `CallFrame` push.
  #[inline]
  fn push_frame_fast(
    &mut self,
    closure_val: Value,
    closure: &ObjClosure,
    proto: &ObjFunction,
    new_base: usize,
    dst_in_caller: u8,
  ) {
    self.frames.push(CallFrame {
      function: proto as *const ObjFunction,
      closure: closure as *const ObjClosure,
      closure_val,
      ip: 0,
      base: new_base,
      dst_in_caller,
      scalar_roots_mark: self.jit_scalar_roots.len(),
      compiled: false,
    });
  }

  /// `setup_closure_call`'s general path: grows the register window if
  /// needed, nil-fills any missing fixed parameter, and collects extra
  /// arguments into a variadic list.
  #[cold]
  #[inline(never)]
  fn setup_closure_call_slow(
    &mut self,
    closure_val: Value,
    closure: &ObjClosure,
    proto: &ObjFunction,
    new_base: usize,
    num_args: u8,
    dst_in_caller: u8,
  ) {
    let required = if proto.variadic {
      proto.arity - 1
    } else {
      proto.arity
    };
    let needed = new_base + proto.num_registers as usize;
    if self.registers.len() < needed {
      self.registers.resize(needed, Value::nil());
      self.sync_regs_ptr_cache();
    }
    for i in num_args..required {
      self.registers[new_base + i as usize] = Value::nil();
    }
    if proto.variadic {
      let extra_count = num_args.saturating_sub(required);
      let mut items = Vec::with_capacity(extra_count as usize);
      for i in 0..extra_count {
        items.push(self.registers[new_base + required as usize + i as usize]);
      }
      let list_val = self.heap.alloc_list(items);
      self.registers[new_base + required as usize] = list_val;
    }

    self.push_frame_fast(closure_val, closure, proto, new_base, dst_in_caller);
  }

  pub(crate) fn call_native(
    &mut self,
    native: &crate::vm::object::NativeFunction,
    args: &[Value],
  ) -> RunResult<Value> {
    let ok_arity = if native.variadic {
      args.len() as u8 >= native.min_arity
    } else {
      args.len() as u8 == native.min_arity
    };

    if !ok_arity {
      let msg = if native.is_method {
        // min_arity/args.len() both count the implicit receiver spliced
        // into args[0], so a user calling x.abs(1) wrote one argument, not
        // two; subtract the receiver back out before showing either
        // number.
        let expected = native.min_arity.saturating_sub(1);
        let got = (args.len() as u8).saturating_sub(1);
        format!(
          "'{}' expects {}{} argument{}, got {}",
          native.name,
          if native.variadic { "at least " } else { "" },
          expected,
          if expected == 1 { "" } else { "s" },
          got
        )
      } else {
        format!(
          "{}() expects {}{} argument{}, got {}",
          native.name,
          if native.variadic { "at least " } else { "" },
          native.min_arity,
          if native.min_arity == 1 { "" } else { "s" },
          args.len()
        )
      };
      return Err(self.raise("ArgumentError", msg));
    }

    // Safety net for gc_pins: a native that pins values to survive its own
    // re-entrant call_values is expected to unpin before returning, but an
    // early return via `?` is easy to miss that for. Truncating back to the
    // pre-call length regardless of how the native returns means a missed
    // unpin costs nothing worse than holding pins a bit longer; gc_pins
    // is a real GC root, so a genuine leak there would be a correctness bug,
    // not just wasted memory.
    let pin_mark = self.gc_pins.len();
    let mut ctx = ZuriContext {
      vm: self,
      args,
      name: native.name,
    };
    let result = (native.func)(&mut ctx);
    self.gc_pins.truncate(pin_mark);
    result.map_err(|msg| self.raise("Error", msg))
  }

  /// Constructs a new instance of `class_val`: allocates storage sized to
  /// its field layout, runs every ancestor's own field initializer
  /// root-to-leaf, then calls the resolved constructor with `args`.
  ///
  /// This makes several sequential re-entrant `call_value`s (each of which
  /// can trigger a collection) while depending on Values that live only in
  /// local Rust variables; none of those are reachable through any
  /// register/global/frame, so each is explicitly pinned via `gc_pins`
  /// rather than trusting the normal root scan.
  fn instantiate(&mut self, class_val: Value, args: &[Value]) -> RunResult<Value> {
    let constructor = class_val.as_class().constructor;
    let field_count = class_val.as_class().field_count;

    // Fast path: does any ancestor declare a field initializer at all? A
    // class whose fields are all assigned directly in its constructor body
    // never needs the field_inits list, and building that list even when
    // every entry is None is a real Vec allocation per instantiation.
    let mut has_field_init = false;
    let mut cur = Some(class_val);
    while let Some(c) = cur {
      let cobj = c.as_class();
      if cobj.own_field_initializer.is_some() {
        has_field_init = true;
        break;
      }
      cur = cobj.superclass;
    }

    let mut field_inits = Vec::new();
    if has_field_init {
      let mut cur = Some(class_val);
      while let Some(c) = cur {
        let cobj = c.as_class();
        field_inits.push(cobj.own_field_initializer);
        cur = cobj.superclass;
      }
      field_inits.reverse(); // root to leaf
    }

    // Every one of these gets pinned, and every use from here on re-reads
    // it from its pinned slot rather than a local variable. call_value
    // below can trigger a collection since a field initializer or
    // constructor body is arbitrary Zuri code, and a plain Rust local has
    // no way to be found and rewritten if the object it names relocates --
    // gc_pins only protects a value as long as every read goes back
    // through the pinned slot.
    let pin_mark = self.gc_pins.len();
    self.gc_pins.push(class_val);
    let class_idx = pin_mark;

    let mut field_init_idxs = Vec::with_capacity(field_inits.len());
    for f in field_inits.into_iter().flatten() {
      self.gc_pins.push(f);
      field_init_idxs.push(self.gc_pins.len() - 1);
    }

    let ctor_idx = constructor.map(|c| {
      self.gc_pins.push(c);
      self.gc_pins.len() - 1
    });

    let args_start = self.gc_pins.len();
    for a in args {
      self.gc_pins.push(*a);
    }
    let args_end = self.gc_pins.len();

    let instance_val = self
      .heap
      .alloc_instance(self.gc_pins[class_idx], field_count as usize);
    self.gc_pins.push(instance_val);
    let instance_idx = self.gc_pins.len() - 1;

    let result: RunResult<()> = (|| {
      for &idx in &field_init_idxs {
        let init = self.gc_pins[idx];
        let instance_now = self.gc_pins[instance_idx];
        self.call_value(init, &[instance_now])?;
      }
      if let Some(idx) = ctor_idx {
        let ctor = self.gc_pins[idx];
        let instance_now = self.gc_pins[instance_idx];
        let mut ctor_args = CallArgs::new();
        ctor_args.push(instance_now);
        ctor_args.extend_from_slice(&self.gc_pins[args_start..args_end]);
        self.call_value(ctor, ctor_args.as_slice())?;
      }
      Ok(())
    })();

    let final_instance = self.gc_pins[instance_idx];
    self.gc_pins.truncate(pin_mark);
    result?;
    Ok(final_instance)
  }

  /// `instantiate`'s fast, inline-cache-style twin for a `Class` callee
  /// reached from already-compiled code, the frame-setup half of
  /// `zuri_jit_new_prepare`.
  ///
  /// `zuri_jit_call_prepare` bails the instant it sees a non-`Closure`
  /// callee, so every `Point(x, y)` in compiled code used to fall all the
  /// way through `dispatch_call_sync` -> `instantiate` -> `call_value` ->
  /// `run_frame` -> `tiered_entry` -> `invoke_compiled`, re-deriving arity,
  /// dispatch kind, pins and frame layout from scratch each time. On
  /// allocation-heavy code that machinery, not the constructor body, ends
  /// up dominating the profile.
  ///
  /// Deliberately narrow, bails to the general path unless: the callee is
  /// a real `Class` (not a module binding), no class in the ancestor chain
  /// declares its own field initializer, and the class has a non-variadic
  /// `Closure` constructor that's already compiled.
  ///
  /// Returns `(entry, constructor_closure)` for generated code to
  /// `call_indirect`, having already placed the instance and arguments in
  /// the constructor's register window and pushed its frame. The instance
  /// stays pinned until `finish_compiled_construction` releases it, so the
  /// pair must always run together.
  pub(crate) fn prepare_compiled_construction(
    &mut self,
    base: usize,
    func_reg: u8,
    num_args: u8,
    dst: u8,
  ) -> Option<(EntryFn, Value)> {
    // num_args + 1 (the implicit self) has to stay a valid u8 register
    // count.
    if !self.jit_depth_ok() || num_args == u8::MAX {
      return None;
    }
    let class_val = self.get_reg(base, func_reg);
    if !class_val.is_class() {
      return None;
    }

    // Everything needed from the class comes out under one borrow.
    // as_class is a real RefCell borrow/release pair, and on a
    // constructor-heavy workload taking it twice measured as a visible
    // cost.
    let (field_count, ctor, superclass) = {
      let class = class_val.as_class();
      if class.own_field_initializer.is_some() {
        return None;
      }
      (
        class.field_count as usize,
        class.constructor?,
        class.superclass,
      )
    };
    if !ctor.is_closure() {
      return None;
    }
    // An inherited field initializer is just as disqualifying as an own
    // one; `instantiate` runs every ancestor's, root to leaf.
    let mut cur = superclass;
    while let Some(c) = cur {
      let next = {
        let cobj = c.as_class();
        if cobj.own_field_initializer.is_some() {
          return None;
        }
        cobj.superclass
      };
      cur = next;
    }

    // Order below is load-bearing: this is the only collection point in
    // the function, so everything it could relocate is either re-read
    // afterwards or (for ctor) made immovable by it.
    let ctor = self.ensure_stable_for_compiled_entry(ctor);
    // Re-read through the register, a real GC root the collection above
    // would have updated, rather than reusing the local.
    let class_val = self.get_reg(base, func_reg);

    let closure = ctor.as_closure();
    let proto = closure.function.as_func();
    if proto.variadic {
      return None;
    }
    let entry = proto.jit.entry.get()?;

    // From here on nothing can collect: Heap::alloc only bump-allocates,
    // collections are driven from run_until's safepoint checks, and the
    // non-variadic check above rules out setup_closure_call's one
    // allocating branch.
    let instance = self.heap.alloc_instance(class_val, field_count);

    // The constructor's window is [self, arg0, ..], but Instr::Call on a
    // class left [class, arg0, ..]; shift arguments up one slot, high to
    // low so a slot is never read after being overwritten. Window must
    // grow before the shift since the topmost argument's new home is past
    // where the call site itself wrote.
    let new_base = base + func_reg as usize + 1;
    let needed = new_base + proto.num_registers as usize;
    if self.registers.len() < needed {
      self.registers.resize(needed, Value::nil());
      self.sync_regs_ptr_cache();
    }
    for i in (0..num_args as usize).rev() {
      self.registers[new_base + i + 1] = self.registers[new_base + i];
    }
    self.registers[new_base] = instance;

    self.setup_closure_call(ctor, closure, proto, new_base, num_args + 1, dst);
    self.jit_depth_enter();
    // Pinned rather than kept in a local: the constructor body is
    // arbitrary Zuri code that may collect, and this is the value the call
    // site's dst ultimately receives; gc_pins is what keeps it correct
    // while nothing else references it.
    self.gc_pins.push(instance);
    Some((entry, ctor))
  }

  /// `prepare_compiled_construction` with every resolution step already
  /// discharged at compile time.
  ///
  /// Callers must hold `resolve_construct_target`'s proof and have had
  /// generated code check its guard immediately beforehand; that's what
  /// licenses trusting `ctor`/`proto`/`field_count` here instead of
  /// re-deriving them from the class.
  pub(crate) fn prepare_known_construction(
    &mut self,
    base: usize,
    func_reg: u8,
    num_args: u8,
    dst: u8,
    ctor: Value,
    proto: &ObjFunction,
    field_count: usize,
  ) {
    let class_val = self.get_reg(base, func_reg);
    let instance = self.heap.alloc_instance(class_val, field_count);

    // The callee's window starts at the callee register itself, one lower
    // than an ordinary call's func_reg + 1; that single offset is what
    // makes the argument shift unnecessary.
    //
    // A constructor's window needs [self, arg0, arg1, ..], while
    // Instr::Call on a class leaves [class, arg0, arg1, ..]. Starting the
    // frame one register earlier lines those up: the fresh instance
    // overwrites the class and becomes register 0 ("self"), and every
    // argument is already where the callee expects it. Sliding arguments
    // up instead costs a load/store per argument per instance built --
    // measured at ~9% of this helper's time.
    //
    // Safe because func_reg is dead to the caller once the call is issued
    //; it exists only to hold the callee. Even when the call site reuses
    // it as dst, nothing is lost: zuri_jit_new_finish writes dst only
    // after popping the callee's frame. While the constructor runs, that
    // slot holds the instance as the callee's own self, exactly the GC
    // root it needs to be.
    let new_base = base + func_reg as usize;
    let needed = new_base + proto.num_registers as usize;
    if self.registers.len() < needed {
      self.registers.resize(needed, Value::nil());
      self.sync_regs_ptr_cache();
    }
    self.registers[new_base] = instance;

    let closure = ctor.as_closure();
    self.setup_closure_call(ctor, closure, proto, new_base, num_args + 1, dst);
    self.jit_depth_enter();
    self.gc_pins.push(instance);
  }

  /// `prepare_known_construction`'s allocation half ONLY; no register-
  /// window growth check, no frame push. Used by `jit::codegen`'s
  /// inline construct fast path (`emit_inline_construct`), which
  /// verifies the window already fits (via `emit_call_checks`, using
  /// the SAME bound `prepare_known_construction`'s own growth check
  /// uses: `new_base + proto.num_registers`) before ever calling this,
  /// specifically so this never needs to grow anything itself: see
  /// `emit_inline_construct`'s own docs for why doing this BEFORE that
  /// check passed would be unsound (an orphaned, permanently-pinned
  /// instance if the frame push it's paired with then fails and falls
  /// back to a path that allocates its own).
  ///
  /// Returns the new instance; generated code still needs its `Value`
  /// bits to build the constructor's own `CallFrame`.
  pub(crate) fn alloc_and_pin_instance(
    &mut self,
    base: usize,
    func_reg: u8,
    field_count: usize,
  ) -> Value {
    let class_val = self.get_reg(base, func_reg);
    let instance = self.heap.alloc_instance(class_val, field_count);
    let new_base = base + func_reg as usize;
    self.registers[new_base] = instance;
    self.gc_pins.push(instance);
    instance
  }

  /// Completes `prepare_compiled_construction`'s bracket: releases the
  /// instance pin and hands back the (possibly relocated) instance, which
  /// is the constructor call's real result; its own return value is
  /// discarded, same as `instantiate`.
  ///
  /// Pairs strictly LIFO: a nested construction inside a constructor body
  /// pushes and pops its own pin entirely within this one's lifetime.
  pub(crate) fn take_constructed_instance(&mut self) -> Value {
    self
      .gc_pins
      .pop()
      .expect("a matching prepare_compiled_construction always pinned one")
  }

  /// Shared "call whatever's in register `func_reg`" logic; the dispatch
  /// `Instr::Call` performs, factored out so Invoke/InvokeSuper's
  /// field-fallback (a field holding a callable) can reach it too instead
  /// of duplicating native/class/bound-method/closure dispatch. `func_reg`
  /// and `dst` are relative to `base`; arguments sit at
  /// `func_reg+1..=func_reg+num_args`, arity excludes any implicit
  /// receiver.
  pub(crate) fn dispatch_call(
    &mut self,
    base: usize,
    func_reg: u8,
    num_args: u8,
    dst: u8,
  ) -> RunResult<()> {
    self.dispatch_call_inner(base, func_reg, num_args, dst, false)
  }

  /// Same dispatch as `dispatch_call`, but for a call site with no flat
  /// interpreter loop waiting to pick up a merely-pushed frame; a call
  /// issued from already-compiled code. A `Closure` callee runs to
  /// completion synchronously here via `run_frame`, like `call_value` does
  /// for a native calling back into Zuri, rather than being left on
  /// `frames` for a loop that doesn't exist.
  pub(crate) fn dispatch_call_sync(
    &mut self,
    base: usize,
    func_reg: u8,
    num_args: u8,
    dst: u8,
  ) -> RunResult<()> {
    self.dispatch_call_inner(base, func_reg, num_args, dst, true)
  }

  fn dispatch_call_inner(
    &mut self,
    base: usize,
    func_reg: u8,
    num_args: u8,
    dst: u8,
    sync: bool,
  ) -> RunResult<()> {
    let callee = self.get_reg(base, func_reg);

    if !callee.is_obj() {
      let msg = format!("cannot call object of type {}", callee.type_name());
      return Err(self.raise("TypeError", msg));
    }

    match unsafe { &*callee.as_obj() } {
      Obj::Native(_) => {
        let args_start = base + func_reg as usize + 1;
        let args_end = args_start + num_args as usize;
        let mut args = CallArgs::new();
        args.extend_from_slice(&self.registers[args_start..args_end]);
        let result = self.call_native(callee.as_native(), args.as_slice())?;
        self.set_reg(base, dst, result);
        Ok(())
      },
      Obj::Class(_) => {
        let args_start = base + func_reg as usize + 1;
        let args_end = args_start + num_args as usize;
        let mut user_args = CallArgs::new();
        user_args.extend_from_slice(&self.registers[args_start..args_end]);
        let instance = self.instantiate(callee, user_args.as_slice())?;
        self.set_reg(base, dst, instance);
        Ok(())
      },
      Obj::BoundMethod(_) => {
        let args_start = base + func_reg as usize + 1;
        let args_end = args_start + num_args as usize;
        let bound = callee.as_bound_method();
        let mut full_args = Vec::with_capacity(num_args as usize + 1);
        full_args.push(bound.receiver);
        full_args.extend_from_slice(&self.registers[args_start..args_end]);
        let result = self.call_value(bound.method, &full_args)?;
        self.set_reg(base, dst, result);
        Ok(())
      },
      Obj::Closure(_) => {
        let callee_closure = callee.as_closure();
        let callee_fn = callee_closure.function.as_func();
        let new_base = base + func_reg as usize + 1;
        self.setup_closure_call(callee, callee_closure, callee_fn, new_base, num_args, dst);
        if sync {
          // No flat interpreter loop is waiting for this frame; run it
          // to completion right now, same as call_value does for a
          // native calling back into Zuri.
          let stop_depth = self.frames.len() - 1;
          let ret = self.run_frame(stop_depth, callee_fn, callee)?;
          self.set_reg(base, dst, ret);
        } else {
          // Mixed-mode dispatch: if callee_fn is warm/compiled (or this
          // call tips it over the warm-up threshold), run it as compiled
          // code right now instead of leaving the frame for the
          // interpreter loop's next iteration. Still cold falls straight
          // through to Ok(()) and ordinary push-and-continue.
          callee_fn
            .jit
            .call_count
            .set(callee_fn.jit.call_count.get().saturating_add(1));
          self.record_call_feedback(callee_fn);
          if let Some(entry) = self.tiered_entry(callee_fn, callee_closure.function) {
            let ret = self.invoke_compiled(entry, callee, -1)?;
            self.set_reg(base, dst, ret);
          }
        }
        Ok(())
      },
      Obj::ModuleBinding(b) => {
        match b.promoted {
          Some(f) => {
            // Overwrite the callee's register with the promoted function
            // and recurse; dispatch_call re-reads func_reg fresh at the
            // top, reusing every existing dispatch path for free.
            self.set_reg(base, func_reg, f);
            self.dispatch_call_inner(base, func_reg, num_args, dst, sync)
          },
          None => {
            let msg = format!("module '{}' is not callable", b.bind_name);
            Err(self.raise("TypeError", msg))
          },
        }
      },
      _ => {
        let msg = format!("cannot call object of type {}", callee.type_name());
        Err(self.raise("TypeError", msg))
      },
    }
  }

  /// Calls a closure whose implicit receiver was already placed by the
  /// compiler at `recv_reg + 1`, the convention behind a genuine method
  /// call. Unlike `dispatch_call`, `callee` itself is never written to a
  /// register here; it's consulted only for its function pointers, since
  /// the receiver occupying what would otherwise be the callee's register
  /// is the whole point of the fused Invoke/InvokeSuper instructions.
  pub(crate) fn invoke_prebound(
    &mut self,
    base: usize,
    recv_reg: u8,
    callee: Value,
    num_args: u8,
    dst: u8,
  ) -> RunResult<()> {
    self.invoke_prebound_inner(base, recv_reg, callee, num_args, dst, false)
  }

  /// `invoke_prebound`'s counterpart for a call site with no flat
  /// interpreter loop waiting; same reasoning as `dispatch_call_sync`,
  /// for Invoke/InvokeSuper/CallSuperCtor's compiled call sites.
  pub(crate) fn invoke_prebound_sync(
    &mut self,
    base: usize,
    recv_reg: u8,
    callee: Value,
    num_args: u8,
    dst: u8,
  ) -> RunResult<()> {
    self.invoke_prebound_inner(base, recv_reg, callee, num_args, dst, true)
  }

  fn invoke_prebound_inner(
    &mut self,
    base: usize,
    recv_reg: u8,
    callee: Value,
    num_args: u8,
    dst: u8,
    sync: bool,
  ) -> RunResult<()> {
    if !callee.is_closure() {
      let msg = format!("cannot call object of type {}", callee.type_name());
      return Err(self.raise("TypeError", msg));
    }

    let callee_closure = callee.as_closure();
    let callee_fn = callee_closure.function.as_func();
    let new_base = base + recv_reg as usize + 1;
    // 1 + num_args: the receiver already duplicated into recv_reg + 1
    // occupies the callee's register 0 ("self"), ahead of the user
    // arguments.
    self.setup_closure_call(
      callee,
      callee_closure,
      callee_fn,
      new_base,
      1 + num_args,
      dst,
    );
    if sync {
      let stop_depth = self.frames.len() - 1;
      let ret = self.run_frame(stop_depth, callee_fn, callee)?;
      self.set_reg(base, dst, ret);
    } else {
      // Same mixed-mode tiering as `dispatch_call`'s Closure arm: see
      // its comment for the full rationale.
      callee_fn
        .jit
        .call_count
        .set(callee_fn.jit.call_count.get().saturating_add(1));
      self.record_call_feedback(callee_fn);
      if let Some(entry) = self.tiered_entry(callee_fn, callee_closure.function) {
        let ret = self.invoke_compiled(entry, callee, -1)?;
        self.set_reg(base, dst, ret);
      }
    }
    Ok(())
  }

  //-----------------------------------------------------------------------------------
  // Indexing and slicing
  //-----------------------------------------------------------------------------------

  pub(crate) fn index_get(&mut self, receiver: Value, index: Value) -> RunResult<Value> {
    if receiver.is_list() {
      let i = self.coerce_index(index, receiver.list_len())?;
      Ok(receiver.list_get(i).unwrap())
    } else if receiver.is_bytes() {
      let i = self.coerce_index(index, receiver.bytes_len())?;
      Ok(Value::number(receiver.bytes_get(i).unwrap() as f64))
    } else if receiver.is_string() {
      let c = self.string_char_at(receiver, index)?;
      Ok(self.interned_char(c))
    } else if receiver.is_dict() {
      match receiver.dict_get(&index) {
        Some(v) => Ok(v),
        None => Err(self.raise(
          "PropertyError",
          format!("undefined key '{}' in dict", index),
        )),
      }
    } else {
      Err(self.raise(
        "TypeError",
        format!("cannot index into a {}", receiver.type_name()),
      ))
    }
  }

  /// The shared `Value` for a single ASCII character, building the whole
  /// table on first use. Non-ASCII characters get an ordinary fresh
  /// allocation; too many to intern, and not the hot case.
  ///
  /// Allocated `alloc_old` so these never move; still marked as roots by
  /// the major collector, which keeps them from being swept.
  fn interned_char(&mut self, c: char) -> Value {
    if !c.is_ascii() {
      return self.heap.alloc_string(c.to_string());
    }
    self.interned_ascii[c as usize]
  }

  /// `s[i]` for a string receiver, resolving `i` (possibly negative,
  /// counting from the end) to the character it names.
  ///
  /// Has an ASCII fast path: the naive implementation walks the string
  /// twice per index (once to bounds-check via `chars().count()`, once for
  /// `chars().nth(i)`), making indexing in a loop quadratic. If every byte
  /// up to and including byte `i` is ASCII, character `i` is byte `i` and
  /// no counting is needed; checking that prefix is a word-at-a-time scan
  /// instead of a per-character UTF-8 decode.
  ///
  /// Anything the fast path can't answer; non-ASCII in range, negative or
  /// out-of-range index; falls through to the original behavior.
  fn string_char_at(&mut self, receiver: Value, index: Value) -> RunResult<char> {
    let raw = self.value_as_index(index)?;
    if raw >= 0 {
      let i = raw as usize;
      let bytes = receiver.as_str().as_bytes();
      if i < bytes.len() && bytes[..=i].is_ascii() {
        return Ok(bytes[i] as char);
      }
    }
    let chars_len = receiver.as_str().chars().count();
    let i = self.coerce_index(index, chars_len)?;
    Ok(receiver.as_str().chars().nth(i).unwrap())
  }

  pub(crate) fn index_set(&mut self, receiver: Value, index: Value, value: Value) -> RunResult<()> {
    if receiver.is_list() {
      let i = self.coerce_index(index, receiver.list_len())?;
      receiver.list_set(i, value);
      Ok(())
    } else if receiver.is_bytes() {
      let i = self.coerce_index(index, receiver.bytes_len())?;
      if !value.is_number() {
        let msg = format!("bytes element must be a number, got {}", value.type_name());
        return Err(self.raise("TypeError", msg));
      }
      let n = value.as_number();
      if n.fract() != 0.0 || !(0.0..=255.0).contains(&n) {
        let msg = format!("bytes element must be an integer in 0..=255, got {}", n);
        return Err(self.raise("NumericError", msg));
      }
      receiver.bytes_set(i, n as u8);
      Ok(())
    } else if receiver.is_dict() {
      receiver.dict_set(index, value);
      Ok(())
    } else if receiver.is_string() {
      Err(self.raise(
        "TypeError",
        "strings are immutable and do not support index assignment",
      ))
    } else {
      Err(self.raise(
        "TypeError",
        format!("cannot assign into a {}", receiver.type_name()),
      ))
    }
  }

  pub(crate) fn index_slice(&mut self, receiver: Value, lo: Value, hi: Value) -> RunResult<Value> {
    if receiver.is_obj() {
      match unsafe { &*receiver.as_obj() } {
        Obj::List(_) => {
          let len = receiver.list_len();
          let bounds = self.resolve_slice_bounds(lo, hi, len)?;
          let items: Vec<Value> = match bounds {
            Some((lo, hi)) => (lo..hi).map(|i| receiver.list_get(i).unwrap()).collect(),
            None => Vec::new(),
          };
          return Ok(self.heap.alloc_list(items));
        },
        Obj::Bytes(_) => {
          let len = receiver.bytes_len();
          let bounds = self.resolve_slice_bounds(lo, hi, len)?;
          let items: Vec<u8> = match bounds {
            Some((lo, hi)) => (lo..hi).map(|i| receiver.bytes_get(i).unwrap()).collect(),
            None => Vec::new(),
          };
          return Ok(self.heap.alloc_bytes(items));
        },
        Obj::Str(_) => {
          let chars: Vec<char> = receiver.as_str().chars().collect();
          let bounds = self.resolve_slice_bounds(lo, hi, chars.len())?;
          let s: String = match bounds {
            Some((lo, hi)) => chars[lo..hi].iter().collect(),
            None => String::new(),
          };
          return Ok(self.heap.alloc_string(s));
        },
        _ => {},
      }
    };

    Err(self.raise(
      "TypeError",
      format!("cannot slice a {}", receiver.type_name()),
    ))
  }

  fn run_until(&mut self, stop_depth: usize) -> RunResult<Value> {
    // Early-exit out of the labeled 'step block with an Err, like `?`
    // would from inside an ordinary function. A bare `?` here would target
    // run_until's own return type directly and skip the catch_stack check
    // entirely.
    macro_rules! tri {
      ($e:expr, $label:lifetime) => {
        match $e {
          Ok(v) => v,
          Err(e) => break $label Err(e),
        }
      };
    }

    // Cached "which frame/function/closure am I executing" state, refreshed
    // only where it actually changes (Call/Invoke/InvokeSuper/CallSuperCtor
    // push a frame, Return pops one, a caught error truncates several)
    // rather than re-derived from self.frames on every instruction.
    let mut frame_idx = self.frames.len() - 1;
    let mut base = self.frames[frame_idx].base;
    let mut func_ptr = self.frames[frame_idx].function;
    let mut closure_ptr = self.frames[frame_idx].closure;
    let mut ip = self.frames[frame_idx].ip;

    'dispatch: loop {
      if self.heap.needs_major_gc() {
        self.collect_garbage();
        closure_ptr = self.frames[frame_idx].closure;
      } else if self.heap.needs_minor_gc() {
        self.collect_minor();
        // func_ptr never needs this: ObjFunction always allocates old-
        // generation, so it never moves. closure_ptr has no such guarantee
        //; ObjClosure is an ordinary young allocation, the same hazard
        // ensure_stable_for_compiled_entry exists to prevent for compiled
        // code's closure_param. The interpreter just re-derives it on
        // every safepoint instead of needing it pinned for a whole
        // invocation; cheap, and collect_minor already relocated it via
        // the per-frame loop that keeps self.frames[..].closure in sync.
        closure_ptr = self.frames[frame_idx].closure;
      }

      let func = unsafe { &*func_ptr };

      if ip >= func.chunk.code.len() {
        let msg = format!("fell off the end of '{}' without a Return", func.name);
        return Err(self.raise("Error", msg));
      }

      let instr = unsafe { *func.chunk.code.get_unchecked(ip) };

      #[cfg(feature = "opcode-profile")]
      self.record_opcode(crate::vm::chunk::instr_name(&instr));

      ip += 1;
      // Synced every instruction, not just at frame-change points, because
      // any instruction can call self.raise(), which reads every active
      // frame's ip to build a stack trace.
      self.frames[frame_idx].ip = ip;

      // A labeled block instead of an IIFE closure wrapping the match: the
      // closure was too large for LLVM to inline across, so every
      // instruction paid a real call boundary on top of dispatch. This
      // gives the same "inspect the Result before propagating" behavior
      // with direct access to the frame-state locals, no capture needed.
      let step: RunResult<()> = 'step: {
        match instr {
          Instr::LoadConst { dst, const_idx } => {
            let v = func.chunk.constants[const_idx as usize];
            self.set_reg(base, dst, v);
          },
          Instr::LoadNil { dst } => self.set_reg(base, dst, Value::nil()),
          Instr::LoadBool { dst, val } => self.set_reg(base, dst, Value::bool(val)),
          Instr::Move { dst, src } => {
            let v = self.get_reg(base, src);
            self.set_reg(base, dst, v);
          },

          Instr::Add { dst, a, b } => {
            tri!(self.binary_add(base, dst, a, b, "+"), 'step);
          },
          Instr::Sub { dst, a, b } => {
            tri!(
              self.binary_numeric(base, dst, a, b, "-", "@sub", |x, y| x - y, |x, y| &x - &y),
              'step
            );
          },
          Instr::Mul { dst, a, b } => {
            tri!(self.binary_mult(base, dst, a, b, "*"), 'step);
          },
          Instr::Div { dst, a, b } => {
            tri!(
              self.binary_numeric(base, dst, a, b, "/", "@div", |x, y| x / y, |x, y| &x / &y),
              'step
            );
          },
          Instr::Pow { dst, a, b } => {
            tri!(
              self.binary_numeric(
                base, dst, a, b, "**", "@pow",
                |x, y| x.powf(y),
                |x, y| &x * &y,
              ),
              'step
            );
          },
          Instr::Mod { dst, a, b } => {
            tri!(
              self.binary_numeric(base, dst, a, b, "%", "@mod", crate::vm::value::num_rem, |x, y| &x % &y),
              'step
            );
          },
          Instr::Floor { dst, a, b } => {
            tri!(
              self.binary_numeric(
                base, dst, a, b, "//", "@floordiv",
                |x, y| (x / y).floor(),
                |x, y| &x / &y,
              ),
              'step
            );
          },
          Instr::BitAnd { dst, a, b } => {
            tri!(
              self.bitwise_numeric(base, dst, a, b, "&", "@and", |x, y| x & y, |x, y| &x & &y),
              'step
            );
          },
          Instr::BitOr { dst, a, b } => {
            tri!(
              self.bitwise_numeric(base, dst, a, b, "|", "@or", |x, y| x | y, |x, y| &x | &y),
              'step
            );
          },
          Instr::BitXor { dst, a, b } => {
            tri!(
              self.bitwise_numeric(base, dst, a, b, "^", "@xor", |x, y| x ^ y, |x, y| &x ^ &y),
              'step
            );
          },
          Instr::BitShl { dst, a, b } => {
            tri!(
              self.bitwise_numeric(
                base, dst, a, b, "<<", "@lshift",
                |x, y| x.checked_shl(y as u32).unwrap_or(0),
                |x, y| x.shl(y.to_i64().unwrap_or(0)),
              ),
              'step
            );
          },
          Instr::BitShr { dst, a, b } => {
            tri!(
              self.bitwise_numeric(
                base, dst, a, b, ">>", "@rshift",
                |x, y| x.checked_shr(y as u32).unwrap_or(0),
                |x, y| x.shr(y.to_i64().unwrap_or(0)),
              ),
              'step
            );
          },
          Instr::BitUshr { dst, a, b } => {
            tri!(
              self.bitwise_numeric(
                base, dst, a, b, ">>>", "@urshift",
                |x, y| (x as u32).checked_shr(y as u32).unwrap_or(0) as i64,
                |x, y| x.shr(y.to_i64().unwrap_or(0)),
              ),
              'step
            );
          },
          Instr::BitNot { dst, src } => {
            let v = self.get_reg(base, src);
            if v.is_number() {
              self.set_reg(base, dst, Value::number((!(v.as_number() as i64)) as f64));
            } else if let Some(result) = tri!(self.try_operator_override(v, "@not", &[]), 'step) {
              self.set_reg(base, dst, result);
            } else {
              let msg = format!("cannot bitwise not a {}", v.argument_type_name());
              break 'step Err(self.raise("TypeError", msg));
            }
          },
          Instr::Neg { dst, src } => {
            let v = self.get_reg(base, src);
            if v.is_number() {
              self.set_reg(base, dst, Value::number(-v.as_number()));
            } else if v.is_bigint() {
              let nv = self.heap.alloc_bigint(v.as_bigint().neg());
              self.set_reg(base, dst, nv);
            } else if let Some(result) = tri!(self.try_operator_override(v, "@neg", &[]), 'step) {
              self.set_reg(base, dst, result);
            } else {
              let msg = format!("cannot negate a {}", v.argument_type_name());
              break 'step Err(self.raise("TypeError", msg));
            }
          },
          Instr::Not { dst, src } => {
            let v = self.get_reg(base, src);
            self.set_reg(base, dst, Value::bool(v.is_falsey()));
          },
          Instr::AddImm { dst, a, imm_const } => {
            let va = self.get_reg(base, a);
            let vb = func.chunk.constants[imm_const as usize];
            let result = tri!(self.binary_add_values(va, vb, "+"), 'step);
            self.set_reg(base, dst, result);
          },
          Instr::SubImm { dst, a, imm_const } => {
            let imm = func.chunk.constants[imm_const as usize].as_number();
            tri!(self.binary_numeric_imm(base, dst, a, imm, "-", "@sub", |x, y| x - y), 'step);
          },
          Instr::MulImm { dst, a, imm_const } => {
            let va = self.get_reg(base, a);
            let imm = func.chunk.constants[imm_const as usize].as_number();
            // Mirrors binary_mult's string/list-repeat cases; only
            // "string/list * number" needs repeat behavior, and a literal
            // here can only ever be the right-hand number.
            if va.is_string() {
              let count = imm as usize;
              let s = if count < usize::MAX {
                va.as_str().repeat(count)
              } else {
                String::new()
              };
              let v = self.heap.alloc_string(s);
              self.set_reg(base, dst, v);
            } else if va.is_list() {
              let count = imm as usize;
              let value = if count < usize::MAX {
                va.as_list().to_vec().repeat(count)
              } else {
                Vec::new()
              };
              let v = self.heap.alloc_list(value);
              self.set_reg(base, dst, v);
            } else {
              tri!(self.binary_numeric_imm(base, dst, a, imm, "*", "@mul", |x, y| x * y), 'step);
            }
          },
          Instr::LtImm { dst, a, imm_const } => {
            let imm = func.chunk.constants[imm_const as usize].as_number();
            tri!(self.compare_imm(base, dst, a, imm, "<", "@lt", |x, y| x < y), 'step);
          },
          Instr::LeImm { dst, a, imm_const } => {
            let imm = func.chunk.constants[imm_const as usize].as_number();
            tri!(self.compare_imm(base, dst, a, imm, "<=", "@lte", |x, y| x <= y), 'step);
          },
          Instr::GtImm { dst, a, imm_const } => {
            let imm = func.chunk.constants[imm_const as usize].as_number();
            tri!(self.compare_imm(base, dst, a, imm, ">", "@gt", |x, y| x > y), 'step);
          },
          Instr::GeImm { dst, a, imm_const } => {
            let imm = func.chunk.constants[imm_const as usize].as_number();
            tri!(self.compare_imm(base, dst, a, imm, ">=", "@gte", |x, y| x >= y), 'step);
          },
          Instr::EqImm { dst, a, imm_const } => {
            let va = self.get_reg(base, a);
            let vb = func.chunk.constants[imm_const as usize];
            self.set_reg(base, dst, Value::bool(va.equals(&vb)));
          },
          Instr::NeqImm { dst, a, imm_const } => {
            let va = self.get_reg(base, a);
            let vb = func.chunk.constants[imm_const as usize];
            self.set_reg(base, dst, Value::bool(!va.equals(&vb)));
          },
          Instr::Concat { dst, a, b } => {
            let va = self.get_reg(base, a);
            let vb = self.get_reg(base, b);
            let s = format!("{}{}", va, vb);
            let v = self.heap.alloc_string(s);
            self.set_reg(base, dst, v);
          },

          Instr::Eq { dst, a, b } => {
            let va = self.get_reg(base, a);
            let vb = self.get_reg(base, b);
            self.set_reg(base, dst, Value::bool(va.equals(&vb)));
          },
          Instr::Neq { dst, a, b } => {
            let va = self.get_reg(base, a);
            let vb = self.get_reg(base, b);
            self.set_reg(base, dst, Value::bool(!va.equals(&vb)));
          },
          Instr::Lt { dst, a, b } => {
            tri!(self.compare(base, dst, a, b, "<", "@lt", |x, y| x < y, |x, y| &x < &y), 'step);
          },
          Instr::Gt { dst, a, b } => {
            tri!(self.compare(base, dst, a, b, ">", "@gt", |x, y| x > y, |x, y| &x > &y), 'step);
          },
          Instr::Le { dst, a, b } => {
            tri!(
              self.compare(base, dst, a, b, "<=", "@lte", |x, y| x <= y, |x, y| &x <= &y),
              'step
            );
          },
          Instr::Ge { dst, a, b } => {
            tri!(
              self.compare(base, dst, a, b, ">=", "@gte", |x, y| x >= y, |x, y| &x >= &y),
              'step
            );
          },

          Instr::Jmp { offset } => {
            let target = (ip as isize + offset as isize) as usize;
            // A backward jump is a loop back-edge, where a baseline JIT
            // offers on-stack replacement. maybe_osr returns None the vast
            // majority of the time, in which case this is a plain jump.
            if offset < 0 {
              // Captured before maybe_osr runs; on Ok it has already
              // closed this frame's upvalues and popped it, so
              // self.frames[frame_idx] is no longer valid to read after.
              let dst_in_caller = self.frames[frame_idx].dst_in_caller;
              if let Some(outcome) = self.maybe_osr(func, target) {
                match outcome {
                  // Mirrors Instr::Return: OSR ran the current frame to
                  // completion, so from here this is a return, not a jump.
                  Ok(ret) => {
                    if self.frames.len() == stop_depth {
                      return Ok(ret);
                    }
                    frame_idx = self.frames.len() - 1;
                    let caller = &self.frames[frame_idx];
                    base = caller.base;
                    func_ptr = caller.function;
                    closure_ptr = caller.closure;
                    ip = caller.ip;
                    self.set_reg(base, dst_in_caller, ret);
                    continue 'dispatch;
                  },
                  // Same error machinery any other failing instruction
                  // uses; compiled code never pops its own frame on
                  // error, so catch_stack sees this like an ordinary
                  // interpreted instruction failing.
                  Err(exc) => break 'step Err(exc),
                }
              }
            }
            ip = target;
          },
          Instr::JmpIfFalse { cond, offset } => {
            if self.get_reg(base, cond).is_falsey() {
              ip = (ip as isize + offset as isize) as usize;
            }
          },
          Instr::JmpIfTrue { cond, offset } => {
            if !self.get_reg(base, cond).is_falsey() {
              ip = (ip as isize + offset as isize) as usize;
            }
          },

          Instr::Call {
            dst,
            func: func_reg,
            num_args,
          } => {
            tri!(self.dispatch_call(base, func_reg, num_args, dst), 'step);
            // dispatch_call may or may not have pushed a new frame (Closure
            // does, Native/Class/BoundMethod resolve synchronously and
            // don't); always refresh, cheap and only paid on a call.
            frame_idx = self.frames.len() - 1;
            let f = &self.frames[frame_idx];
            base = f.base;
            func_ptr = f.function;
            closure_ptr = f.closure;
            ip = f.ip;
          },
          Instr::Return { src } => {
            let ret = self.get_reg(base, src);
            self.close_upvalues_from(base);
            let finished = self.pop_frame_inner();
            if self.frames.len() == stop_depth {
              return Ok(ret);
            }
            frame_idx = self.frames.len() - 1;
            let caller = &self.frames[frame_idx];
            base = caller.base;
            func_ptr = caller.function;
            closure_ptr = caller.closure;
            ip = caller.ip;
            self.set_reg(base, finished.dst_in_caller, ret);
          },

          Instr::Print { src } => {
            let v = self.get_reg(base, src);
            crate::vm::natives::echo_value(v);
          },

          Instr::CheckParamType { reg, check_idx } => {
            let instr_ip = ip - 1;
            let v = self.get_reg(base, reg);
            let check = &func.chunk.param_checks[check_idx as usize];
            let passes = check.nullable && v.is_nil();
            if !passes {
              let mut matched = false;
              // `check.types` is short (a source-level `|`-union), so a
              // plain loop beats collecting into a Vec first; and
              // `param_type_matches` needs `&mut self` (an `Instance`
              // miss can raise), which a `.any(...)` closure borrowing
              // `check` from `func.chunk` can't coexist with anyway.
              for &t in &check.types {
                match self.param_type_matches(v, t, func, instr_ip) {
                  Ok(true) => {
                    matched = true;
                    break;
                  },
                  Ok(false) => {},
                  Err(e) => break 'step Err(e),
                }
              }
              if !matched {
                let check = &func.chunk.param_checks[check_idx as usize];
                let msg = format!(
                  "{}() expects parameter '{}' (argument {}) to be {}, got {}",
                  func.name,
                  check.param_name,
                  check.position,
                  crate::vm::chunk::describe_param_types(&check.types, &func.chunk),
                  v.type_name(),
                );
                break 'step Err(self.raise("TypeError", msg));
              }
            }
          },

          Instr::GetGlobal { dst, name_const } => {
            let instr_ip = ip - 1;
            let gmod = func.globals_module;
            let (is_root, slot) =
              if let Some(&cached) = func.chunk.global_cache.borrow().get(&instr_ip) {
                cached
              } else {
                let name_val = func.chunk.constants[name_const as usize];
                if !name_val.is_string() {
                  break 'step Err(
                    self.raise("TypeError", "expected a string constant for a global name"),
                  );
                }
                let resolved = match self.resolve_global(gmod, name_val.as_str()) {
                  Some(r) => r,
                  None => {
                    let msg = format!("undefined global '{}'", name_val.as_str());
                    break 'step Err(self.raise("UndefinedError", msg));
                  },
                };
                func
                  .chunk
                  .global_cache
                  .borrow_mut()
                  .insert(instr_ip, resolved);
                resolved
              };
            let v = self.read_resolved(gmod, is_root, slot);
            self.set_reg(base, dst, v);
          },

          Instr::SetGlobal { name_const, src } => {
            let instr_ip = ip - 1;
            let gmod = func.globals_module;
            let slot = if let Some(&(_, s)) = func.chunk.global_cache.borrow().get(&instr_ip) {
              s
            } else {
              let name_val = func.chunk.constants[name_const as usize];
              if !name_val.is_string() {
                break 'step Err(
                  self.raise("TypeError", "expected a string constant for a global name"),
                );
              }
              let s = self.get_or_create_slot_in(gmod, name_val.as_str().to_string());
              func
                .chunk
                .global_cache
                .borrow_mut()
                .insert(instr_ip, (gmod.is_none(), s));
              s
            };
            let v = self.get_reg(base, src);
            self.write_slot_in(gmod, slot, v);
          },

          Instr::AssignGlobal { name_const, src } => {
            let instr_ip = ip - 1;
            let gmod = func.globals_module;
            let (is_root, slot) =
              if let Some(&cached) = func.chunk.global_cache.borrow().get(&instr_ip) {
                cached
              } else {
                let name_val = func.chunk.constants[name_const as usize];
                if !name_val.is_string() {
                  break 'step Err(
                    self.raise("TypeError", "expected a string constant for a global name"),
                  );
                }
                let resolved = match self.resolve_global(gmod, name_val.as_str()) {
                  Some(r) => r,
                  None => {
                    let msg = format!("undefined global '{}'", name_val.as_str());
                    break 'step Err(self.raise("UndefinedError", msg));
                  },
                };
                func
                  .chunk
                  .global_cache
                  .borrow_mut()
                  .insert(instr_ip, resolved);
                resolved
              };
            let v = self.get_reg(base, src);
            self.write_resolved(gmod, is_root, slot, v);
          },

          Instr::Closure { dst, proto_const } => {
            let proto_val = func.chunk.constants[proto_const as usize];
            if !proto_val.is_func() {
              break 'step Err(self.raise("TypeError", "Closure operand is not a function"));
            }
            let proto = proto_val.as_func();

            let mut captured = smallvec::SmallVec::with_capacity(proto.upvalues.len());
            for desc in &proto.upvalues {
              let upval = match *desc {
                UpvalueDescriptor::Local(reg) => {
                  let abs_index = base + reg as usize;
                  self.capture_upvalue(abs_index)
                },
                UpvalueDescriptor::Upvalue(idx) => {
                  let current_closure = unsafe { &*closure_ptr };
                  current_closure.upvalues[idx as usize]
                },
              };
              captured.push(upval);
            }

            let closure_val = self.heap.alloc_closure(ObjClosure {
              function: proto_val,
              upvalues: captured,
            });
            self.set_reg(base, dst, closure_val);
          },
          Instr::GetUpval { dst, idx } => {
            let current_closure = unsafe { &*closure_ptr };
            let upval_val = current_closure.upvalues[idx as usize];
            if !upval_val.is_upvalue() {
              break 'step Err(self.raise("TypeError", "GetUpval operand is not an upvalue"));
            }
            let v = match upval_val.as_upvalue().get() {
              UpvalueState::Open(abs_idx) => self.registers[abs_idx],
              UpvalueState::Closed(v) => v,
            };
            self.set_reg(base, dst, v);
          },
          Instr::SetUpval { idx, src } => {
            let v = self.get_reg(base, src);
            let current_closure = unsafe { &*closure_ptr };
            let upval_val = current_closure.upvalues[idx as usize];
            if !upval_val.is_upvalue() {
              break 'step Err(self.raise("TypeError", "SetUpval operand is not an upvalue"));
            }
            let cell = upval_val.as_upvalue();
            match cell.get() {
              UpvalueState::Open(abs_idx) => self.registers[abs_idx] = v,
              UpvalueState::Closed(_) => {
                cell.set(UpvalueState::Closed(v));
                write_barrier(upval_val.as_obj());
              },
            }
          },
          Instr::CloseUpvalues { from } => {
            self.close_upvalues_from(base + from as usize);
          },
          Instr::MakeList { dst, start, count } => {
            // Collect straight into ListStorage, not a Vec; for count
            // within the inline capacity (small literal arrays, the common
            // case) this avoids a heap allocation per list literal.
            let items: ListStorage = (0..count).map(|i| self.get_reg(base, start + i)).collect();
            let list_val = self.heap.alloc_list(items);
            self.set_reg(base, dst, list_val);
          },
          Instr::MakeDict { dst, start, count } => {
            let pairs: Vec<(Value, Value)> = (0..count)
              .map(|i| {
                (
                  self.get_reg(base, start + i),
                  self.get_reg(base, start + count + i),
                )
              })
              .collect();
            let dict_val = self.heap.alloc_dict(pairs);
            self.set_reg(base, dst, dict_val);
          },

          Instr::MakeClass {
            dst,
            name_const,
            superclass,
          } => {
            let name = tri!(self.const_as_str(func, name_const), 'step);
            let superclass_val = match superclass {
              Some(r) => {
                let v = self.get_reg(base, r);
                if !v.is_class() {
                  let msg = format!(
                    "superclass of '{}' is not a class (got a {})",
                    name,
                    v.type_name()
                  );
                  break 'step Err(self.raise("TypeError", msg));
                }
                Some(v)
              },
              None => None,
            };

            let (methods, field_slots, field_count, constructor) = match superclass_val {
              Some(sup) => {
                let s = sup.as_class();
                (
                  s.methods.clone(),
                  s.field_slots.clone(),
                  s.field_count,
                  s.constructor,
                )
              },
              None => (FxHashMap::default(), FxHashMap::default(), 0, None),
            };

            let class_val = self.heap.alloc_class(ObjClass {
              name,
              superclass: superclass_val,
              methods,
              field_slots,
              field_count,
              own_field_initializer: None,
              constructor,
              static_slots: FxHashMap::default(),
              statics: Vec::new(),
              globals_module: func.globals_module,
            });
            write_barrier(class_val.as_obj());
            self.set_reg(base, dst, class_val);
          },

          Instr::DeclareField { class, name_const } => {
            let class_val = self.get_reg(base, class);
            let name = tri!(self.const_as_str(func, name_const), 'step);
            let mut c = class_val.as_class_mut();

            if !c.field_slots.contains_key(&name) {
              let idx = c.field_count;
              c.field_slots.insert(name, idx);
              c.field_count += 1;
            }
          },

          Instr::SetFieldInit { class, src } => {
            let class_val = self.get_reg(base, class);
            let init = self.get_reg(base, src);
            class_val.as_class_mut().own_field_initializer = Some(init);
            write_barrier(class_val.as_obj());
          },

          Instr::SetMethod {
            class,
            name_const,
            src,
          } => {
            let class_val = self.get_reg(base, class);
            let name = tri!(self.const_as_str(func, name_const), 'step);
            let method = self.get_reg(base, src);
            class_val.as_class_mut().methods.insert(name, method);
            write_barrier(class_val.as_obj());
            self.bump_method_table_generation();
          },

          Instr::DeclareStatic {
            class,
            name_const,
            src,
          } => {
            let class_val = self.get_reg(base, class);
            let name = tri!(self.const_as_str(func, name_const), 'step);
            let value = self.get_reg(base, src);
            let mut c = class_val.as_class_mut();
            let idx = c.statics.len() as u16;
            c.static_slots.insert(name, idx);
            c.statics.push(Cell::new(value));
            drop(c);
            write_barrier(class_val.as_obj());
          },

          Instr::FinalizeClass { class } => {
            let class_val = self.get_reg(base, class);
            let name = tri!(self.const_as_str(func, 0), 'step);
            let mut c = class_val.as_class_mut();

            if self.lookup_slot_in(func.globals_module, &c.name).is_some() {
              break 'step Err(self.raise(
                "Error",
                format!("class '{}' already declared in this scope", c.name),
              ));
            }

            if let Some(ctor) = c.methods.get(&name).copied() {
              c.constructor = Some(ctor);
              drop(c);
              write_barrier(class_val.as_obj());
            }
          },

          Instr::GetField {
            dst,
            obj,
            name_const,
          } => {
            let receiver = self.get_reg(base, obj);
            let name_val = func.chunk.constants[name_const as usize];
            if !name_val.is_string() {
              break 'step Err(
                self.raise("TypeError", "expected a string constant for a global name"),
              );
            }

            let value = if receiver.is_instance() {
              let inst = receiver.as_instance();
              let class = inst.class.as_class();
              if let Some(&idx) = class.field_slots.get(name_val.as_str()) {
                inst.fields[idx as usize].get()
              } else if let Some(method) = class.methods.get(name_val.as_str()).copied() {
                self.heap.alloc_bound_method(receiver, method)
              } else {
                let msg = format!(
                  "undefined property '{}' on instance of '{}'",
                  name_val.as_str(),
                  class.name
                );
                break 'step Err(self.raise("PropertyError", msg));
              }
            } else if receiver.is_class() {
              let raw = tri!(
                lookup_static(receiver, name_val.as_str())
                  .ok_or_else(|| format!(
                    "undefined static member '{}' on class '{}'",
                    name_val.as_str(),
                    receiver.as_class().name
                  ))
                  .map_err(|msg| self.raise("PropertyError", msg)),
                'step
              );
              if raw.is_closure() && raw.as_closure().function.as_func().is_method {
                self.heap.alloc_bound_method(Value::nil(), raw)
              } else {
                raw
              }
            } else if receiver.is_module() {
              let m = receiver.as_module();
              match m.namespace.get(name_val.as_str()) {
                Some(v) => v,
                None => {
                  let msg = format!("module '{}' has no member '{}'", m.name, name_val.as_str());
                  break 'step Err(self.raise("PropertyError", msg));
                },
              }
            } else if receiver.is_module_binding() {
              let module_val = receiver.as_module_binding().module;
              let m = module_val.as_module();
              match m.namespace.get(name_val.as_str()) {
                Some(v) => v,
                None => {
                  let msg = format!("module '{}' has no member '{}'", m.name, name_val.as_str());
                  break 'step Err(self.raise("PropertyError", msg));
                },
              }
            } else if receiver.is_dict() {
              // dict.key is sugar for dict['key']; same lookup and
              // missing-key error as GetIndex's dict arm.
              match receiver.dict_get(&name_val) {
                Some(v) => v,
                None => {
                  let msg = format!("undefined key '{}' in dict", name_val);
                  break 'step Err(self.raise("PropertyError", msg));
                },
              }
            } else {
              let msg = format!(
                "cannot read property '{}' on a {}",
                name_val.as_str(),
                receiver.type_name()
              );
              break 'step Err(self.raise("TypeError", msg));
            };
            self.set_reg(base, dst, value);
          },

          Instr::SetField {
            obj,
            name_const,
            src,
          } => {
            let receiver = self.get_reg(base, obj);
            let value = self.get_reg(base, src);
            let name_val = func.chunk.constants[name_const as usize];
            if !name_val.is_string() {
              break 'step Err(
                self.raise("TypeError", "expected a string constant for a global name"),
              );
            }

            if receiver.is_instance() {
              let inst = receiver.as_instance();
              let class = inst.class.as_class();
              let idx = *tri!(
                class
                  .field_slots
                  .get(name_val.as_str())
                  .ok_or_else(|| format!(
                    "undefined field '{}' on instance of '{}'",
                    name_val.as_str(),
                    class.name
                  ))
                  .map_err(|msg| self.raise("PropertyError", msg)),
                'step
              );
              inst.fields[idx as usize].set(value);
              write_barrier(receiver.as_obj());
            } else if receiver.is_class() {
              tri!(
                set_static(receiver, name_val.as_str(), value)
                  .map_err(|msg| self.raise("PropertyError", msg)),
                'step
              );
            } else if receiver.is_module() || receiver.is_module_binding() {
              let msg = "cannot assign to a module member from outside the module".to_string();
              break 'step Err(self.raise("AccessError", msg));
            } else if receiver.is_dict() {
              // dict.key = value is sugar for dict['key'] = value --
              // insert-or-update, never an error for a missing key.
              receiver.dict_set(name_val, value);
            } else {
              let msg = format!(
                "cannot set property '{}' on a {}",
                name_val.as_str(),
                receiver.type_name()
              );
              break 'step Err(self.raise("TypeError", msg));
            }
          },

          Instr::Invoke {
            dst,
            obj,
            method_const,
            num_args,
          } => {
            let receiver = self.get_reg(base, obj);
            let method_name_val = func.chunk.constants[method_const as usize];
            if !method_name_val.is_string() {
              break 'step Err(
                self.raise("TypeError", "expected a string constant for a global name"),
              );
            }

            if receiver.is_instance() {
              let inst = receiver.as_instance();
              let class_val = inst.class;
              let found = {
                let class = class_val.as_class();
                if let Some(m) = class.methods.get(method_name_val.as_str()).copied() {
                  Some(Ok(m))
                } else if let Some(&idx) = class.field_slots.get(method_name_val.as_str()) {
                  Some(Err(idx))
                } else {
                  None
                }
              };

              match found {
                Some(Ok(method)) => {
                  tri!(self.invoke_prebound(base, obj, method, num_args, dst), 'step);
                },
                Some(Err(idx)) => {
                  let field_value = inst.fields[idx as usize].get();
                  self.set_reg(base, obj + 1, field_value);
                  tri!(self.dispatch_call(base, obj + 1, num_args, dst), 'step);
                },
                None => match builtins::lookup(receiver, method_name_val.as_str()) {
                  Some(native) => {
                    let args_start = base + obj as usize + 2;
                    let args_end = args_start + num_args as usize;
                    let mut call_args = CallArgs::new();
                    call_args.push(receiver);
                    call_args.extend_from_slice(&self.registers[args_start..args_end]);
                    let result = tri!(self.call_native(native, call_args.as_slice()), 'step);
                    self.set_reg(base, dst, result);
                  },
                  None => {
                    let msg = format!(
                      "undefined property '{}' on instance of '{}'",
                      method_name_val.as_str(),
                      class_val.as_class().name
                    );
                    break 'step Err(self.raise("PropertyError", msg));
                  },
                },
              }
            } else if receiver.is_class() {
              let callee = tri!(
                lookup_static(receiver, method_name_val.as_str())
                  .ok_or_else(|| format!(
                    "undefined static member '{}' on class '{}'",
                    method_name_val.as_str(),
                    receiver.as_class().name
                  ))
                  .map_err(|msg| self.raise("PropertyError", msg)),
                'step
              );
              if callee.is_closure() && callee.as_closure().function.as_func().is_method {
                tri!(self.invoke_prebound(base, obj, callee, num_args, dst), 'step);
              } else {
                self.set_reg(base, obj + 1, callee);
                tri!(self.dispatch_call(base, obj + 1, num_args, dst), 'step);
              }
            } else if receiver.is_module() || receiver.is_module_binding() {
              let module_val = if receiver.is_module() {
                receiver
              } else {
                receiver.as_module_binding().module
              };
              let member = {
                let m = module_val.as_module();
                m.namespace.get(method_name_val.as_str())
              };
              match member {
                Some(v) => {
                  self.set_reg(base, obj + 1, v);
                  tri!(self.dispatch_call(base, obj + 1, num_args, dst), 'step);
                },
                None => match builtins::lookup(receiver, method_name_val.as_str()) {
                  Some(native) => {
                    let args_start = base + obj as usize + 2;
                    let args_end = args_start + num_args as usize;
                    let mut call_args = CallArgs::new();
                    call_args.push(receiver);
                    call_args.extend_from_slice(&self.registers[args_start..args_end]);
                    let result = tri!(self.call_native(native, call_args.as_slice()), 'step);
                    self.set_reg(base, dst, result);
                  },
                  None => {
                    let msg = format!(
                      "undefined member '{}' on module {}",
                      method_name_val.as_str(),
                      module_val.as_module().name
                    );
                    break 'step Err(self.raise("PropertyError", msg));
                  },
                },
              }
            } else {
              match builtins::lookup(receiver, method_name_val.as_str()) {
                Some(native) => {
                  let args_start = base + obj as usize + 2;
                  let args_end = args_start + num_args as usize;
                  let mut call_args = CallArgs::new();
                  call_args.push(receiver);
                  call_args.extend_from_slice(&self.registers[args_start..args_end]);
                  let result = tri!(self.call_native(native, call_args.as_slice()), 'step);
                  self.set_reg(base, dst, result);
                },
                None => {
                  let msg = format!(
                    "object of type {} does not define method '{}'",
                    receiver.type_name(),
                    method_name_val.as_str()
                  );
                  break 'step Err(self.raise("TypeError", msg));
                },
              }
            }

            // invoke_prebound/dispatch_call above may have pushed a new
            // frame; native/field-fallback paths never do. Always refresh
            //; only paid on Invoke itself, not the arithmetic/move
            // instructions dominating a hot loop.
            frame_idx = self.frames.len() - 1;
            let f = &self.frames[frame_idx];
            base = f.base;
            func_ptr = f.function;
            closure_ptr = f.closure;
            ip = f.ip;
          },

          Instr::InvokeSuper {
            dst,
            superclass,
            method_const,
            num_args,
          } => {
            let super_val = self.get_reg(base, superclass);
            if !super_val.is_class() {
              let msg = format!(
                "'parent' does not refer to a class (got a {})",
                super_val.type_name()
              );
              break 'step Err(self.raise("TypeError", msg));
            }
            let method_name_val = func.chunk.constants[method_const as usize];
            if !method_name_val.is_string() {
              break 'step Err(
                self.raise("TypeError", "expected a string constant for a global name"),
              );
            }

            let found = {
              let class = super_val.as_class();
              class.methods.get(method_name_val.as_str()).copied()
            };

            if let Some(method) = found {
              tri!(self.invoke_prebound(base, superclass, method, num_args, dst), 'step);
            } else {
              let self_val = self.get_reg(base, superclass + 1);
              if !self_val.is_instance() {
                let msg = format!(
                  "undefined method '{}' on superclass '{}'",
                  method_name_val.as_str(),
                  super_val.as_class().name
                );
                break 'step Err(self.raise("PropertyError", msg));
              }

              let inst = self_val.as_instance();
              let idx = *tri!(
                inst
                  .class
                  .as_class()
                  .field_slots
                  .get(method_name_val.as_str())
                  .ok_or_else(|| format!(
                    "undefined method '{}' on superclass '{}'",
                    method_name_val.as_str(),
                    super_val.as_class().name
                  ))
                  .map_err(|msg| self.raise("PropertyError", msg)),
                'step
              );

              let field_value = inst.fields[idx as usize].get();
              self.set_reg(base, superclass + 1, field_value);
              tri!(self.dispatch_call(base, superclass + 1, num_args, dst), 'step);
            }

            frame_idx = self.frames.len() - 1;
            let f = &self.frames[frame_idx];
            base = f.base;
            func_ptr = f.function;
            closure_ptr = f.closure;
            ip = f.ip;
          },
          Instr::CallSuperCtor {
            dst,
            superclass,
            num_args,
          } => {
            let super_val = self.get_reg(base, superclass);
            if !super_val.is_class() {
              let msg = format!(
                "'parent' does not refer to a class (got a {})",
                super_val.type_name()
              );
              break 'step Err(self.raise("TypeError", msg));
            }
            let ctor = super_val.as_class().constructor;
            match ctor {
              Some(ctor) => {
                tri!(self.invoke_prebound(base, superclass, ctor, num_args, dst), 'step);
              },
              None => {
                let msg = format!(
                  "class '{}' has no constructor to call via parent()",
                  super_val.as_class().name
                );
                break 'step Err(self.raise("AccessError", msg));
              },
            }

            frame_idx = self.frames.len() - 1;
            let f = &self.frames[frame_idx];
            base = f.base;
            func_ptr = f.function;
            closure_ptr = f.closure;
            ip = f.ip;
          },

          Instr::GetIndex { dst, obj, idx } => {
            let ov = self.get_reg(base, obj);
            let iv = self.get_reg(base, idx);
            let result = tri!(self.index_get(ov, iv), 'step);
            self.set_reg(base, dst, result);
          },
          Instr::SetIndex { obj, idx, src } => {
            let ov = self.get_reg(base, obj);
            let iv = self.get_reg(base, idx);
            let sv = self.get_reg(base, src);
            tri!(self.index_set(ov, iv, sv), 'step);
          },
          Instr::GetSlice { dst, obj, lo, hi } => {
            let ov = self.get_reg(base, obj);
            let lov = self.get_reg(base, lo);
            let hiv = self.get_reg(base, hi);
            let result = tri!(self.index_slice(ov, lov, hiv), 'step);
            self.set_reg(base, dst, result);
          },

          Instr::MakeRange { dst, lower, upper } => {
            let lo = self.get_reg(base, lower);
            let hi = self.get_reg(base, upper);
            if !lo.is_number() || !hi.is_number() {
              let msg = format!(
                "range bounds must be numbers, got {} and {}",
                lo.type_name(),
                hi.type_name()
              );
              break 'step Err(self.raise("TypeError", msg));
            }
            let range_val = self.heap.alloc_range(lo.as_number(), hi.as_number());
            self.set_reg(base, dst, range_val);
          },
          Instr::UsingJump { subject, table_idx } => {
            let v = self.get_reg(base, subject);
            if let Some(key) = value_to_jump_key(v) {
              if let Some(&target) = func.chunk.jump_tables[table_idx as usize].get(&key) {
                ip = target;
              }
            }
          },

          Instr::Raise { src } => {
            let value = self.get_reg(base, src);
            if !self.is_error_value(value) {
              let msg = format!(
                "can only raise an Error or subclass, got a {}",
                value.type_name()
              );
              break 'step Err(self.raise("TypeError", msg));
            }
            let value = self.attach_stacktrace(value);
            break 'step Err(value);
          },

          Instr::PushCatch { var_reg, offset } => {
            let resume_ip = (ip as isize + offset as isize) as usize;
            self.catch_stack.push(CatchHandler {
              frame_depth: self.frames.len(),
              resume_ip,
              var_reg,
            });
          },

          Instr::PopCatch => {
            self.catch_stack.pop();
          },

          Instr::Import {
            dst,
            path_const,
            importer_const,
          } => {
            let path_val = func.chunk.constants[path_const as usize];
            let importer_val = func.chunk.constants[importer_const as usize];
            if !path_val.is_string() || !importer_val.is_string() {
              break 'step Err(self.raise("TypeError", "expected string constants for import"));
            }
            let path = path_val.as_str().to_string();
            let importer = importer_val.as_str().to_string();
            let module_val = tri!(crate::vm::modules::import(self, &importer, &path), 'step);
            self.set_reg(base, dst, module_val);
          },

          Instr::ImportAll { module } => {
            let mv = self.get_reg(base, module);
            if !mv.is_module() {
              break 'step Err(self.raise("TypeError", "expected a module for 'import ... { * }'"));
            }
            let entries: Vec<(String, Value)> = {
              let m = mv.as_module();
              m.namespace
                .names
                .iter()
                .map(|(k, &idx)| (k.clone(), m.namespace.slots[idx as usize].get()))
                .collect()
            };
            let target = func.globals_module;
            for (name, val) in entries {
              let slot = self.get_or_create_slot_in(target, name);
              self.write_slot_in(target, slot, val);
            }
          },

          Instr::MakePromoted {
            dst,
            module,
            name_const,
          } => {
            let mv = self.get_reg(base, module);
            let name_val = func.chunk.constants[name_const as usize];
            if !mv.is_module() || !name_val.is_string() {
              break 'step Err(self.raise("TypeError", "invalid module promotion"));
            }
            let promoted = {
              let m = mv.as_module();
              m.namespace
                .get(name_val.as_str())
                .filter(|v| v.is_callable())
            };
            let binding = self.heap.alloc_module_binding(ObjModuleBinding {
              module: mv,
              promoted,
              bind_name: name_val.as_str().to_string(),
            });
            self.set_reg(base, dst, binding);
          },
        }
        Ok(())
      };

      if let Err(exc) = step {
        match self.handle_error(exc, stop_depth) {
          ErrorOutcome::Handled {
            frame_idx: fi,
            base: b,
            func_ptr: fp,
            closure_ptr: cp,
            ip: nip,
          } => {
            frame_idx = fi;
            base = b;
            func_ptr = fp;
            closure_ptr = cp;
            ip = nip;
            continue;
          },
          ErrorOutcome::Propagate(e) => return Err(e),
        }
      }
    }
  }

  /// Find-or-create an open upvalue for the given absolute register index.
  /// Reusing an existing one is what makes two closures over the same
  /// local actually share state.
  #[inline]
  pub(crate) fn capture_upvalue(&mut self, abs_index: usize) -> Value {
    if let Some((_, v)) = self.open_upvalues.iter().find(|(idx, _)| *idx == abs_index) {
      return *v;
    }
    let v = self.heap.alloc_upvalue(UpvalueState::Open(abs_index));
    self.open_upvalues.push((abs_index, v));
    self.has_open_upvalues.set(true);
    v
  }

  /// Close every open upvalue pointing at a register >= `from_abs_index`,
  /// copying the register's current value into the upvalue's own
  /// storage. Called on block exit and on Return.
  #[inline]
  pub(crate) fn close_upvalues_from(&mut self, from_abs_index: usize) {
    // Split so just this check inlines into the callers that run on every
    // return. A program that never captures a local in a closure keeps
    // this list empty for its entire run.
    if self.open_upvalues.is_empty() {
      return;
    }
    self.close_upvalues_from_slow(from_abs_index);
  }

  fn close_upvalues_from_slow(&mut self, from_abs_index: usize) {
    let mut i = 0;
    while i < self.open_upvalues.len() {
      let (idx, v) = self.open_upvalues[i];
      if idx >= from_abs_index {
        let current_val = self.registers[idx];
        v.as_upvalue().set(UpvalueState::Closed(current_val));
        write_barrier(v.as_obj());
        self.open_upvalues.swap_remove(i);
      } else {
        i += 1;
      }
    }
    self.has_open_upvalues.set(!self.open_upvalues.is_empty());
  }

  #[inline]
  fn const_as_str(&mut self, func: &ObjFunction, idx: u16) -> RunResult<String> {
    let v = func.chunk.constants[idx as usize];
    if !v.is_string() {
      return Err(self.raise("TypeError", "expected a string constant for a global name"));
    }
    Ok(v.as_str().to_string())
  }

  #[inline(always)]
  pub(crate) fn get_reg(&self, base: usize, r: u8) -> Value {
    debug_assert!((base + r as usize) < self.registers.len());
    unsafe { *self.registers.get_unchecked(base + r as usize) }
  }

  #[inline(always)]
  pub(crate) fn set_reg(&mut self, base: usize, r: u8, v: Value) {
    debug_assert!((base + r as usize) < self.registers.len());
    unsafe {
      *self.registers.get_unchecked_mut(base + r as usize) = v;
    }
  }

  /// Reads an `Obj::Upvalue`'s current value, whether it's still Open
  /// (pointing at a live register in some still-executing frame) or
  /// already Closed; the same two-way read `Instr::GetUpval` does
  /// inline. `pub(crate)` so code outside the interpreter loop (e.g.
  /// `modules::isolate_util`, which needs to snapshot a closure's
  /// captured values before they can cross an isolate boundary) can read
  /// one without duplicating that match.
  pub(crate) fn read_upvalue(&self, upvalue: Value) -> Value {
    match upvalue.as_upvalue().get() {
      UpvalueState::Open(abs_idx) => self.registers[abs_idx],
      UpvalueState::Closed(v) => v,
    }
  }

  /// Like `get_reg`/`set_reg` but for an absolute register index rather
  /// than one relative to a frame's `base`; needed for open-upvalue
  /// access, where the index can belong to a different, outer frame.
  #[inline(always)]
  pub(crate) fn get_reg_abs(&self, abs: usize) -> Value {
    debug_assert!(abs < self.registers.len());
    unsafe { *self.registers.get_unchecked(abs) }
  }

  #[inline(always)]
  pub(crate) fn set_reg_abs(&mut self, abs: usize, v: Value) {
    debug_assert!(abs < self.registers.len());
    unsafe {
      *self.registers.get_unchecked_mut(abs) = v;
    }
  }

  #[inline(always)]
  pub(crate) fn bitwise_numeric<F, G>(
    &mut self,
    base: usize,
    dst: u8,
    a: u8,
    b: u8,
    op_name: &str,
    deco: &str,
    op: F,
    big_op: G,
  ) -> RunResult<()>
  where
    F: Fn(i64, i64) -> i64,
    G: Fn(BigInt, BigInt) -> BigInt,
  {
    let va = self.get_reg(base, a);
    let vb = self.get_reg(base, b);
    if va.is_number() && vb.is_number() {
      return Ok(self.set_reg(
        base,
        dst,
        Value::number(op(va.as_number() as i64, vb.as_number() as i64) as f64),
      ));
    } else if va.is_bigint() && vb.is_bigint() {
      let v = self
        .heap
        .alloc_bigint(big_op(va.as_bigint().clone(), vb.as_bigint().clone()));
      return Ok(self.set_reg(base, dst, v));
    }

    if let Some(result) = self.try_operator_override(va, deco, &[vb])? {
      self.set_reg(base, dst, result);
      return Ok(());
    }

    let msg = format!(
      "operator '{}' not defined for call signature ({}, {})",
      op_name,
      va.argument_type_name(),
      vb.argument_type_name()
    );
    Err(self.raise("TypeError", msg))
  }

  #[inline(always)]
  pub(crate) fn binary_numeric<F, G>(
    &mut self,
    base: usize,
    dst: u8,
    a: u8,
    b: u8,
    op_name: &str,
    deco: &str,
    op: F,
    big_op: G,
  ) -> RunResult<()>
  where
    F: Fn(f64, f64) -> f64,
    G: Fn(BigInt, BigInt) -> BigInt,
  {
    let va = self.get_reg(base, a);
    let vb = self.get_reg(base, b);

    if va.is_number() && vb.is_number() {
      return Ok(self.set_reg(base, dst, Value::number(op(va.as_number(), vb.as_number()))));
    } else if va.is_bigint() && vb.is_bigint() {
      let v = self
        .heap
        .alloc_bigint(big_op(va.as_bigint().clone(), vb.as_bigint().clone()));
      return Ok(self.set_reg(base, dst, v));
    }

    if let Some(result) = self.try_operator_override(va, deco, &[vb])? {
      return Ok(self.set_reg(base, dst, result));
    }

    let msg = format!(
      "operator '{}' not defined for call signature ({}, {})",
      op_name,
      va.argument_type_name(),
      vb.argument_type_name()
    );
    Err(self.raise("TypeError", msg))
  }

  #[inline]
  pub(crate) fn binary_add_values(
    &mut self,
    va: Value,
    vb: Value,
    op_name: &str,
  ) -> RunResult<Value> {
    if va.is_number() && vb.is_number() {
      return Ok(Value::number(va.as_number() + vb.as_number()));
    } else if va.is_bigint() && vb.is_bigint() {
      return Ok(self.heap.alloc_bigint(va.as_bigint() + vb.as_bigint()));
    }

    if let Some(result) = self.try_operator_override(va, "@add", &[vb])? {
      return Ok(result);
    }

    // `||` here, not `&&`: matches Add's existing behavior including its
    // edge case where `.as_list()`/`.as_bytes()` panics if only one side
    // is actually a list/bytes (e.g. `[1,2] + 5`). Deliberately preserved
    // so a literal RHS behaves identically to a variable RHS holding the
    // same value.
    if va.is_string() && vb.is_string() {
      // Both sides already strings, the common shape format! serves worst.
      // Display for a string Value is its raw contents, so this produces a
      // byte-identical result while skipping core::fmt's dynamic dispatch
      // and String's default growth reallocation; one allocation, two
      // memcpys.
      let (a, b) = (va.as_str(), vb.as_str());
      let mut s = String::with_capacity(a.len() + b.len());
      s.push_str(a);
      s.push_str(b);
      return Ok(self.heap.alloc_string(s));
    } else if va.is_string() || vb.is_string() {
      let s = format!("{}{}", va, vb);
      return Ok(self.heap.alloc_string(s));
    } else if va.is_list() || vb.is_list() {
      let mut value = Vec::new();
      value.extend(va.as_list().iter().cloned());
      value.extend(vb.as_list().iter().cloned());
      return Ok(self.heap.alloc_list(value));
    } else if va.is_bytes() || vb.is_bytes() {
      let mut value = Vec::new();
      value.extend(va.as_bytes().iter().cloned());
      value.extend(vb.as_bytes().iter().cloned());
      return Ok(self.heap.alloc_bytes(value));
    }

    let msg = format!(
      "operator '{}' not defined for call signature ({}, {})",
      op_name,
      va.argument_type_name(),
      vb.argument_type_name()
    );
    Err(self.raise("TypeError", msg))
  }

  #[inline]
  pub(crate) fn binary_add(
    &mut self,
    base: usize,
    dst: u8,
    a: u8,
    b: u8,
    op_name: &str,
  ) -> RunResult<()> {
    let va = self.get_reg(base, a);
    let vb = self.get_reg(base, b);
    let result = self.binary_add_values(va, vb, op_name)?;
    Ok(self.set_reg(base, dst, result))
  }

  #[inline(always)]
  pub(crate) fn binary_mult(
    &mut self,
    base: usize,
    dst: u8,
    a: u8,
    b: u8,
    op_name: &str,
  ) -> RunResult<()> {
    let va = self.get_reg(base, a);
    let vb = self.get_reg(base, b);

    if va.is_number() && vb.is_number() {
      return Ok(self.set_reg(base, dst, Value::number(va.as_number() * vb.as_number())));
    } else if va.is_bigint() && vb.is_bigint() {
      let v = self.heap.alloc_bigint(va.as_bigint() * vb.as_bigint());
      return Ok(self.set_reg(base, dst, v));
    } else if va.is_string() && vb.is_number() {
      let count = vb.as_number() as usize;
      let s = if count < usize::MAX {
        va.as_str().repeat(count)
      } else {
        String::new()
      };
      let v = self.heap.alloc_string(s);
      return Ok(self.set_reg(base, dst, v));
    } else if va.is_list() && vb.is_number() {
      let count = vb.as_number() as usize;
      let value = if count < usize::MAX {
        crate::vm::list::ListStorage::repeat_slice(&va.as_list(), count)
      } else {
        crate::vm::list::ListStorage::new()
      };
      let v = self.heap.alloc_list(value);
      return Ok(self.set_reg(base, dst, v));
    }

    if let Some(result) = self.try_operator_override(va, "@mul", &[vb])? {
      return Ok(self.set_reg(base, dst, result));
    }

    let msg = format!(
      "operator '{}' not defined for call signature ({}, {})",
      op_name,
      va.argument_type_name(),
      vb.argument_type_name()
    );
    Err(self.raise("TypeError", msg))
  }

  #[inline(always)]
  pub(crate) fn compare<F, G>(
    &mut self,
    base: usize,
    dst: u8,
    a: u8,
    b: u8,
    op_name: &str,
    deco: &str,
    op: F,
    big_op: G,
  ) -> RunResult<()>
  where
    F: Fn(f64, f64) -> bool,
    G: Fn(BigInt, BigInt) -> bool,
  {
    let va = self.get_reg(base, a);
    let vb = self.get_reg(base, b);
    if va.is_number() && vb.is_number() {
      return Ok(self.set_reg(base, dst, Value::bool(op(va.as_number(), vb.as_number()))));
    } else if va.is_bigint() && vb.is_bigint() {
      return Ok(self.set_reg(
        base,
        dst,
        Value::bool(big_op(va.as_bigint().clone(), vb.as_bigint().clone())),
      ));
    }

    if let Some(result) = self.try_operator_override(va, deco, &[vb])? {
      return Ok(self.set_reg(base, dst, result));
    }

    let msg = format!(
      "operator '{}' not defined for {} and {}",
      op_name,
      va.argument_type_name(),
      vb.argument_type_name()
    );
    return Err(self.raise("TypeError", msg));
  }

  #[inline]
  pub(crate) fn binary_numeric_imm<F>(
    &mut self,
    base: usize,
    dst: u8,
    a: u8,
    imm: f64,
    op_name: &str,
    deco: &str,
    op: F,
  ) -> RunResult<()>
  where
    F: Fn(f64, f64) -> f64,
  {
    let va = self.get_reg(base, a);
    if va.is_number() {
      return Ok(self.set_reg(base, dst, Value::number(op(va.as_number(), imm))));
    }
    if let Some(result) = self.try_operator_override(va, deco, &[Value::number(imm)])? {
      return Ok(self.set_reg(base, dst, result));
    }
    let msg = format!(
      "operator '{}' not defined for call signature ({}, float)",
      op_name,
      va.argument_type_name(),
    );
    Err(self.raise("TypeError", msg))
  }

  #[inline]
  pub(crate) fn compare_imm<F>(
    &mut self,
    base: usize,
    dst: u8,
    a: u8,
    imm: f64,
    op_name: &str,
    deco: &str,
    op: F,
  ) -> RunResult<()>
  where
    F: Fn(f64, f64) -> bool,
  {
    let va = self.get_reg(base, a);
    if va.is_number() {
      return Ok(self.set_reg(base, dst, Value::bool(op(va.as_number(), imm))));
    }
    if let Some(result) = self.try_operator_override(va, deco, &[Value::number(imm)])? {
      return Ok(self.set_reg(base, dst, result));
    }
    let msg = format!(
      "operator '{}' not defined for {} and float",
      op_name,
      va.argument_type_name(),
    );
    Err(self.raise("TypeError", msg))
  }

  //-----------------------------------------------------------------------------------
  // Garbage collection
  //-----------------------------------------------------------------------------------

  /// Full mark-and-sweep collection. Roots are every register within reach
  /// of an active frame, every global, each frame's executing closure, and
  /// any open upvalue. From there every transitively held `Value` is
  /// walked with an explicit work-list, not recursion, so a long chain
  /// can't blow the stack.
  ///
  /// Called automatically once the heap crosses its threshold; also
  /// exposed to native code via the `gc` native. See `collect_minor` for
  /// the cheaper, far more frequent counterpart this normally relies on.
  pub(crate) fn collect_garbage(&mut self) {
    // Flush the nursery first so every live object is uniformly
    // chunk-resident by the time the mark-sweep pass below runs; it
    // needs no nursery-awareness of its own.
    self.collect_minor();

    let before_bytes = self.heap.bytes_allocated();
    let before_count = self.heap.object_count();

    let mut worklist: Vec<*const Obj> = Vec::new();

    // Only the register range within reach of the active frame can hold
    // live data; registers past its window are leftovers from returned
    // calls (the register stack is never shrunk), so scanning them would
    // just pin down garbage forever.
    let regs_top = self
      .frames
      .last()
      .map(|f| f.base + unsafe { &*f.function }.num_registers as usize)
      .unwrap_or(0)
      .min(self.registers.len());

    for v in &self.registers[..regs_top] {
      Self::mark_root(*v, &mut worklist);
    }
    for cell in &self.global_slots {
      Self::mark_root(cell.get(), &mut worklist);
    }
    for frame in &self.frames {
      Self::mark_root(frame.closure_val, &mut worklist);
    }
    for (_, v) in &self.open_upvalues {
      Self::mark_root(*v, &mut worklist);
    }
    for v in &self.gc_pins {
      Self::mark_root(*v, &mut worklist);
    }
    // Each jit_scalar_roots entry is count ordinary Value slots with no
    // Obj/GcBox layer, so this is the same treatment as gc_pins just
    // above, reading through a raw pointer/count pair instead of a Vec.
    for &(ptr, count) in &self.jit_scalar_roots {
      // SAFETY: every entry is live for as long as its owning CallFrame is
      // still on self.frames, and every frame on self.frames right now is
      // by definition still executing.
      let slice = unsafe { std::slice::from_raw_parts(ptr, count) };
      for &v in slice {
        Self::mark_root(v, &mut worklist);
      }
    }
    for v in &self.pending_jit_compiles {
      Self::mark_root(*v, &mut worklist);
    }
    for v in self.modules.values() {
      Self::mark_root(*v, &mut worklist);
    }
    for v in self.builtin_errors.values() {
      Self::mark_root(*v, &mut worklist);
    }
    for v in &self.interned_ascii {
      Self::mark_root(*v, &mut worklist);
    }
    // The compile-time string constant pool. Collected into a Vec rather
    // than borrowed through the iterator, since marking needs &mut
    // worklist while the heap is also borrowed.
    let interned: Vec<Value> = self.heap.interned_strings().collect();
    for v in interned {
      Self::mark_root(v, &mut worklist);
    }
    Self::mark_root(self.jit_pending_error.get(), &mut worklist);

    while let Some(ptr) = worklist.pop() {
      // SAFETY: every pointer on the worklist came from a Value that was
      // still live when queued, and nothing is freed until sweep runs
      // below, well after this loop.
      Self::walk_children(ptr, |v| Self::mark_root(v, &mut worklist));
    }

    // A full scan just proved everything reachable, old objects included,
    // so every remembered-set entry is now redundant. Drop them so a
    // future write to any of them properly re-queues it.
    self.heap.drain_remembered();

    let freed = self.heap.sweep();
    self.heap.update_jit_gc_needed();
    if self.log_gc {
      eprintln!(
        "[gc-major] freed {}/{} objects, {} -> {} bytes (next collection at {} bytes)",
        freed,
        before_count,
        before_bytes,
        self.heap.bytes_allocated(),
        self.heap.next_gc()
      );
    }
  }

  /// Minor collection: the cheap, frequent counterpart to `collect_garbage`
  /// and the young generation's whole reason to exist; a real, moving,
  /// copying collection rather than mark-sweep. Scans the same roots as
  /// `collect_garbage`, but instead of marking, actively relocates every
  /// still-`Young` object into old-generation storage and rewrites every
  /// reference to it. An `Old` object reached from a root is left alone --
  /// a copying collection never needs to prove an old object's liveness,
  /// only visit every pointer to a young one.
  ///
  /// What that reasoning alone can't see: an old object mutated since its
  /// last full scan could now point at an otherwise-unreachable young
  /// object. That's what `write_barrier` and the remembered set cover --
  /// every remembered old object's children are walked and relocated here
  /// too. Sound because the barrier fires on every mutation of an old
  /// container, so an old->young edge can only exist via the remembered set.
  ///
  /// Once every root and live child has been walked, everything still in
  /// the nursery is unreachable by construction; `reset_nursery`
  /// reclaims it in one step, no per-object free-list bookkeeping needed.
  pub(crate) fn collect_minor(&mut self) {
    let before_count = self.heap.object_count();

    let mut worklist: Vec<*const Obj> = Vec::new();

    let regs_top = self
      .frames
      .last()
      .map(|f| f.base + unsafe { &*f.function }.num_registers as usize)
      .unwrap_or(0)
      .min(self.registers.len());

    for v in &mut self.registers[..regs_top] {
      Self::forward_slot(&mut self.heap, v, &mut worklist);
    }
    for cell in &mut self.global_slots {
      Self::forward_slot(&mut self.heap, cell.get_mut(), &mut worklist);
    }
    for frame in &mut self.frames {
      if Self::forward_slot(&mut self.heap, &mut frame.closure_val, &mut worklist) {
        // function/closure are raw pointers cached from closure_val at
        // frame-push time for hot-path speed; relocating what
        // closure_val points at invalidates them too, so they need the
        // same re-derivation a fresh frame push would do.
        let closure = frame.closure_val.as_closure();
        frame.closure = closure as *const ObjClosure;
        frame.function = closure.function.as_func() as *const ObjFunction;
      }
    }
    for (_, v) in &mut self.open_upvalues {
      Self::forward_slot(&mut self.heap, v, &mut worklist);
    }
    for v in &mut self.gc_pins {
      Self::forward_slot(&mut self.heap, v, &mut worklist);
    }
    // Same treatment as the gc_pins loop above: each entry is count live
    // ordinary Value slots, forwarded in place like any other root.
    for &(ptr, count) in &self.jit_scalar_roots {
      // SAFETY: every entry is live for as long as its owning CallFrame is
      // still on self.frames, and every frame on self.frames right now is
      // by definition still executing.
      let slice = unsafe { std::slice::from_raw_parts_mut(ptr, count) };
      for v in slice {
        Self::forward_slot(&mut self.heap, v, &mut worklist);
      }
    }
    for v in self.builtin_errors.values_mut() {
      Self::forward_slot(&mut self.heap, v, &mut worklist);
    }
    {
      let mut pending = self.jit_pending_error.get();
      if Self::forward_slot(&mut self.heap, &mut pending, &mut worklist) {
        self.jit_pending_error.set(pending);
      }
    }
    for v in &mut self.pending_jit_compiles {
      Self::forward_slot(&mut self.heap, v, &mut worklist);
    }
    for v in self.modules.values_mut() {
      Self::forward_slot(&mut self.heap, v, &mut worklist);
    }

    for remembered_ptr in self.heap.drain_remembered() {
      Self::walk_children_mut(remembered_ptr, |slot| {
        // SAFETY: remembered_ptr is a live old object; walk_children_mut
        // only yields pointers to genuine Value slots it owns.
        Self::forward_slot(&mut self.heap, unsafe { &mut *slot }, &mut worklist)
      });
    }

    while let Some(ptr) = worklist.pop() {
      Self::walk_children_mut(ptr, |slot| {
        Self::forward_slot(&mut self.heap, unsafe { &mut *slot }, &mut worklist)
      });
    }

    let before_bytes = self.heap.bytes_allocated();
    self.heap.reset_nursery();
    if self.log_gc {
      eprintln!(
        "[gc-minor] promoted/freed across {} -> {} objects, {} -> {} bytes",
        before_count,
        self.heap.object_count(),
        before_bytes,
        self.heap.bytes_allocated(),
      );
    }
  }

  /// Resolves one slot that might hold a pointer to a young object,
  /// relocating it and rewriting `*slot` in place if so. Returns whether
  /// it was rewritten; `walk_children_mut`'s `Obj::Dict` case needs this
  /// to decide whether a key's hash changed and its index needs
  /// rebuilding. Takes `heap` explicitly rather than `&mut self` so
  /// callers can borrow it alongside other disjoint fields of `self`.
  #[inline(always)]
  fn forward_slot(heap: &mut Heap, slot: &mut Value, worklist: &mut Vec<*const Obj>) -> bool {
    if !slot.is_obj() {
      return false;
    }
    let old_ptr = slot.as_obj();
    let new_ptr = heap.forward_or_promote(old_ptr, worklist);
    if std::ptr::eq(new_ptr, old_ptr) {
      return false;
    }
    *slot = Value::obj(new_ptr);
    true
  }

  /// Enumerates every `Value` held directly by the object behind `ptr`,
  /// invoking `mark` for each. Shared between `collect_garbage` and
  /// `collect_minor` so this per-`Obj`-variant traversal exists in one
  /// place instead of two copies that could drift apart.
  fn walk_children(ptr: *const Obj, mut mark: impl FnMut(Value)) {
    // SAFETY: both call sites only ever call this with a pointer still
    // guaranteed live.
    match unsafe { &*ptr } {
      Obj::List(items) => {
        for v in items.borrow().iter() {
          mark(*v);
        }
      },
      Obj::Dict(storage) => {
        for (k, v) in storage.borrow().entries.iter() {
          mark(*k);
          mark(*v);
        }
      },
      Obj::Func(f) => {
        for c in &f.chunk.constants {
          mark(*c);
        }
        if let Some(m) = f.globals_module {
          mark(m);
        }
      },
      Obj::Closure(c) => {
        mark(c.function);
        for u in &c.upvalues {
          mark(*u);
        }
      },
      Obj::Upvalue(cell) => {
        if let UpvalueState::Closed(v) = cell.get() {
          mark(v);
        }
      },
      Obj::Class(c) => {
        let class = c.borrow();
        if let Some(sup) = class.superclass {
          mark(sup);
        }
        for m in class.methods.values() {
          mark(*m);
        }
        if let Some(init) = class.own_field_initializer {
          mark(init);
        }
        if let Some(ctor) = class.constructor {
          mark(ctor);
        }
        for cell in &class.statics {
          mark(cell.get());
        }
        if let Some(m) = class.globals_module {
          mark(m);
        }
      },
      Obj::Instance(inst) => {
        mark(inst.class);
        for cell in inst.fields.iter() {
          mark(cell.get());
        }
      },
      Obj::BoundMethod(b) => {
        mark(b.receiver);
        mark(b.method);
      },
      Obj::Module(m) => {
        let m = m.borrow();
        for cell in &m.namespace.slots {
          mark(cell.get());
        }
      },
      Obj::ModuleBinding(b) => {
        mark(b.module);
        if let Some(p) = b.promoted {
          mark(p);
        }
      },
      Obj::Str(_)
      | Obj::Bytes(_)
      | Obj::BigInt(_)
      | Obj::Native(_)
      | Obj::File(_)
      | Obj::Ptr(_)
      | Obj::Range { .. } => {},
    }
  }

  /// `walk_children`'s mutable counterpart, used only by `collect_minor`'s
  /// copying pass: hands `relocate` a raw `*mut Value` pointing at the
  /// actual slot instead of a read-only copy, so the caller can rewrite it
  /// in place if it points at a young object that just got promoted. Sound
  /// via a single `&mut Obj` cast at the top since collection is always
  /// stop-the-world.
  ///
  /// Kept separate from `walk_children` rather than one traversal
  /// parameterized over both callback shapes, since about a third of the
  /// variants need real mutable-borrow machinery a read-only `mark` has no
  /// reason to carry.
  ///
  /// `Obj::Dict` needs more than rewriting each slot: a `DictKey`'s hash is
  /// the raw pointer for reference-type keys, so relocating a key changes
  /// its hash out from under `DictStorage::index`; tracked via
  /// `relocate`'s return value and repaired with one `reindex()` call.
  fn walk_children_mut(ptr: *const Obj, mut relocate: impl FnMut(*mut Value) -> bool) {
    // SAFETY: collection is always stop-the-world, so exclusive access to
    // every reachable object is sound for its whole duration.
    let obj = unsafe { &mut *(ptr as *mut Obj) };
    match obj {
      Obj::List(items) => {
        for v in items.get_mut().iter_mut() {
          relocate(v as *mut Value);
        }
      },
      Obj::Dict(storage) => {
        let storage = storage.get_mut();
        let mut key_moved = false;
        for (k, v) in storage.entries.iter_mut() {
          key_moved |= relocate(k as *mut Value);
          relocate(v as *mut Value);
        }
        if key_moved {
          storage.reindex();
        }
      },
      Obj::Func(f) => {
        for c in f.chunk.constants.iter_mut() {
          relocate(c as *mut Value);
        }
        if let Some(m) = f.globals_module.as_mut() {
          relocate(m as *mut Value);
        }
      },
      Obj::Closure(c) => {
        relocate(&mut c.function as *mut Value);
        for u in c.upvalues.iter_mut() {
          relocate(u as *mut Value);
        }
      },
      Obj::Upvalue(cell) => {
        if let UpvalueState::Closed(v) = cell.get_mut() {
          relocate(v as *mut Value);
        }
      },
      Obj::Class(c) => {
        let class = c.get_mut();
        if let Some(sup) = class.superclass.as_mut() {
          relocate(sup as *mut Value);
        }
        for m in class.methods.values_mut() {
          relocate(m as *mut Value);
        }
        if let Some(init) = class.own_field_initializer.as_mut() {
          relocate(init as *mut Value);
        }
        if let Some(ctor) = class.constructor.as_mut() {
          relocate(ctor as *mut Value);
        }
        for cell in class.statics.iter_mut() {
          relocate(cell.get_mut() as *mut Value);
        }
        if let Some(m) = class.globals_module.as_mut() {
          relocate(m as *mut Value);
        }
      },
      Obj::Instance(inst) => {
        relocate(&mut inst.class as *mut Value);
        for cell in inst.fields.iter_mut() {
          relocate(cell.get_mut() as *mut Value);
        }
      },
      Obj::BoundMethod(b) => {
        relocate(&mut b.receiver as *mut Value);
        relocate(&mut b.method as *mut Value);
      },
      Obj::Module(m) => {
        let m = m.get_mut();
        for cell in m.namespace.slots.iter_mut() {
          relocate(cell.get_mut() as *mut Value);
        }
      },
      Obj::ModuleBinding(b) => {
        relocate(&mut b.module as *mut Value);
        if let Some(p) = b.promoted.as_mut() {
          relocate(p as *mut Value);
        }
      },
      Obj::Str(_)
      | Obj::Bytes(_)
      | Obj::BigInt(_)
      | Obj::Native(_)
      | Obj::File(_)
      | Obj::Ptr(_)
      | Obj::Range { .. } => {},
    }
  }

  /// Add `v` to the reachable set and, the first time it's seen, queue
  /// it so `collect_garbage` walks its children too. A no-op on repeat
  /// visits, which is what makes cycles (e.g. a closure capturing a
  /// variable that in turn points back at the closure) safe to trace.
  #[inline(always)]
  fn mark_root(v: Value, worklist: &mut Vec<*const Obj>) {
    if !v.is_obj() {
      return;
    }
    let ptr = v.as_obj();
    if Heap::mark_object(ptr) {
      worklist.push(ptr);
    }
  }
}

impl VM {
  fn value_as_index(&mut self, index: Value) -> RunResult<i64> {
    if !index.is_number() {
      let msg = format!("index must be a number, got {}", index.type_name());
      return Err(self.raise("TypeError", msg));
    }
    let n = index.as_number();
    let i = n as i64;
    if i as f64 != n {
      return Err(self.raise("TypeError", format!("index must be an integer, got {}", n)));
    }
    Ok(i)
  }

  pub(crate) fn coerce_index(&mut self, index: Value, len: usize) -> RunResult<usize> {
    let mut i = self.value_as_index(index)?;

    // Wrap negative indices around to the end of the array.
    if i < 0 {
      i += len as i64;
    }

    if i < 0 || i as usize >= len {
      let msg = format!("index {} out of bounds (length {})", i, len);
      return Err(self.raise("RangeError", msg));
    }
    Ok(i as usize)
  }

  fn resolve_slice_bounds(
    &mut self,
    lo: Value,
    hi: Value,
    len: usize,
  ) -> RunResult<Option<(usize, usize)>> {
    if len == 0 {
      return Ok(None);
    }

    let lo = if lo.is_nil() {
      0
    } else {
      let mut i = self.value_as_index(lo)?;
      if i < 0 {
        i = (i + len as i64).max(0);
      }
      i as usize
    };

    let hi = if hi.is_nil() {
      len
    } else {
      let mut i = self.value_as_index(hi)?;
      if i < 0 {
        i = (i + len as i64).max(0);
      }

      i as usize
    };

    if lo > len || hi > len {
      let msg = format!("slice bounds {}..{} out of range (length {})", lo, hi, len);
      return Err(self.raise("RangeError", msg));
    }

    if lo > hi || lo == hi {
      return Ok(None);
    }

    Ok(Some((lo, hi)))
  }

  /// Services an operator via a class- or builtin-declared override method
  /// named `deco` (e.g. "@add") on the left operand only; the same
  /// "receiver defines the behavior" model every other method call here
  /// uses, no reflected right-hand fallback. `extra_args` is everything
  /// after the implicit receiver.
  ///
  /// `Ok(None)` if `receiver` has no such override (caller falls through to
  /// its own type-mismatch error), otherwise the override's result.
  pub(crate) fn try_operator_override(
    &mut self,
    receiver: Value,
    deco: &str,
    extra_args: &[Value],
  ) -> RunResult<Option<Value>> {
    if receiver.is_instance() {
      let method = {
        let class = receiver.as_instance().class.as_class();
        class.methods.get(deco).copied()
      };
      if let Some(method) = method {
        let mut args = CallArgs::new();
        args.push(receiver);
        args.extend_from_slice(extra_args);
        return self.call_value(method, args.as_slice()).map(Some);
      }
      return Ok(None);
    }

    if let Some(native) = builtins::lookup_operator(receiver, deco) {
      let mut args = CallArgs::new();
      args.push(receiver);
      args.extend_from_slice(extra_args);
      return self.call_native(native, args.as_slice()).map(Some);
    }

    Ok(None)
  }

  /// Everything that happens when an instruction propagates an error,
  /// factored out and marked `#[cold]`/`#[inline(never)]` purely for code
  /// layout: keeps the rarely-taken handling code out of the hot dispatch
  /// loop's icache footprint. `if let Err(exc) = step` itself is a
  /// branch-predictor-friendly check either way; this is about layout,
  /// not a slow check.
  #[cold]
  #[inline(never)]
  fn handle_error(&mut self, exc: Value, stop_depth: usize) -> ErrorOutcome {
    let claims_it = matches!(self.catch_stack.last(), Some(h) if h.frame_depth > stop_depth);
    if !claims_it {
      return ErrorOutcome::Propagate(exc);
    }

    let handler = self.catch_stack.pop().unwrap();
    if let Some(discard_base) = self.frames.get(handler.frame_depth).map(|f| f.base) {
      self.close_upvalues_from(discard_base);
    }
    // Every frame from handler.frame_depth onward is being discarded in
    // one step; roll jit_scalar_roots back to what it held before the
    // first of them was pushed, same reasoning as pop_frame_inner.
    // unwrap_or covers handler.frame_depth == frames.len() already (the
    // error happened in the frame that pushed this catch), a no-op
    // truncate.
    let scalar_roots_mark = self
      .frames
      .get(handler.frame_depth)
      .map(|f| f.scalar_roots_mark)
      .unwrap_or(self.jit_scalar_roots.len());
    self.frames.truncate(handler.frame_depth);
    self.jit_scalar_roots.truncate(scalar_roots_mark);
    self.jit_scalar_roots_len.set(self.jit_scalar_roots.len());
    let top = self.frames.last_mut().expect("catch handler left no frame");
    top.ip = handler.resume_ip;
    let top_base = top.base;
    if let Some(reg) = handler.var_reg {
      self.set_reg(top_base, reg, exc);
    }

    let frame_idx = self.frames.len() - 1;
    let f = &self.frames[frame_idx];
    ErrorOutcome::Handled {
      frame_idx,
      base: f.base,
      func_ptr: f.function,
      closure_ptr: f.closure,
      ip: f.ip,
    }
  }

  #[cfg(feature = "opcode-profile")]
  #[inline]
  fn record_opcode(&mut self, name: &'static str) {
    *self.opcode_counts.entry(name).or_insert(0) += 1;
    if let Some(prev) = self.last_opcode.replace(name) {
      *self.opcode_bigrams.entry((prev, name)).or_insert(0) += 1;
    }
  }

  #[cfg(feature = "opcode-profile")]
  pub fn dump_opcode_profile(&self) {
    let mut counts: Vec<_> = self.opcode_counts.iter().collect();
    counts.sort_by(|a, b| b.1.cmp(a.1));
    eprintln!("=== opcode counts (top 20) ===");
    for (name, count) in counts.iter().take(20) {
      eprintln!("{:>14}  {}", count, name);
    }

    let mut bigrams: Vec<_> = self.opcode_bigrams.iter().collect();
    bigrams.sort_by(|a, b| b.1.cmp(a.1));
    eprintln!("=== consecutive opcode pairs (top 20) ===");
    for ((a, b), count) in bigrams.iter().take(20) {
      eprintln!("{:>14}  {} -> {}", count, a, b);
    }
  }

  #[inline]
  pub fn clear_frames(&mut self) {
    self.frames.clear();
    // Every remaining frame is wiped unconditionally (REPL error recovery)
    //; their native stack frames are already gone, so any
    // jit_scalar_roots entries they registered would otherwise dangle
    // into freed/reused native stack memory for the next GC to walk.
    self.jit_scalar_roots.clear();
    self.jit_scalar_roots_len.set(0);
  }
}

/// Walk `class_val`'s superclass chain looking for a static member named
/// `name`, checking each class's own (never inherited-in) `static_slots`
/// table.
pub(crate) fn lookup_static(class_val: Value, name: &str) -> Option<Value> {
  let mut cur = Some(class_val);
  while let Some(c) = cur {
    let class = c.as_class();
    if let Some(&idx) = class.static_slots.get(name) {
      return Some(class.statics[idx as usize].get());
    }
    cur = class.superclass;
  }
  None
}

pub(crate) fn set_static(class_val: Value, name: &str, value: Value) -> Result<(), String> {
  let mut cur = Some(class_val);
  while let Some(c) = cur {
    let class = c.as_class();
    if let Some(&idx) = class.static_slots.get(name) {
      class.statics[idx as usize].set(value);
      write_barrier(c.as_obj());
      return Ok(());
    }
    cur = class.superclass;
  }
  Err(format!(
    "undefined static member '{}' on class '{}'",
    name,
    class_val.as_class().name
  ))
}

/// Converts a runtime `using`-subject Value into the same hashable key
/// space `Instr::UsingJump`'s jump table was built in at compile time.
/// `None` for anything that was never eligible to be a constant case label
/// (list, dict, instance, range, etc.), falling through to the sequential
/// dynamic-label path.
fn value_to_jump_key(v: Value) -> Option<JumpKey> {
  if v.is_nil() {
    Some(JumpKey::Nil)
  } else if v.is_bool() {
    Some(JumpKey::Bool(v.as_bool()))
  } else if v.is_number() {
    Some(JumpKey::Number(v.as_number().to_bits()))
  } else if v.is_string() {
    Some(JumpKey::Str(v.as_str().to_string()))
  } else {
    None
  }
}

impl Drop for VM {
  fn drop(&mut self) {
    drop(self.jit_compiler.take());
  }
}
