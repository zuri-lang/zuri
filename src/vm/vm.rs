use std::cell::Cell;
use std::ops::{Neg, Shl, Shr};

use num_bigint::BigInt;
use num_traits::ToPrimitive;
use rustc_hash::FxHashMap;

use crate::builtins;
use crate::jit::{EntryFn, JitEngine, background, typeflow};
use crate::vm::chunk::{Instr, JumpKey};
use crate::vm::natives;
use crate::vm::object::{
  Heap, ListStorage, Obj, ObjClass, ObjClosure, ObjFunction, ObjModuleBinding, UpvalueDescriptor,
  UpvalueState, ZuriContext, write_barrier,
};
use crate::vm::value::Value;

/// Ceiling on how many compiled-function calls may be nested on the
/// REAL Rust/OS call stack at once. Pure interpreted recursion never
/// touches the native stack (see `run_until`'s frame-stack design), so
/// it's bounded only by heap memory; a call INTO compiled machine code
/// is an ordinary native call, so it IS bounded by the thread's stack.
/// Once nesting hits this depth, `VM::tiered_entry`/`maybe_osr` simply
/// stop offering compiled entry points and let the (stack-safe)
/// interpreter take over for the rest of the recursion, exactly the
/// same fallback already used for a function that isn't warm/compiled
/// yet -- so a pathologically deep recursion degrades to "slower" NOT
/// to "the process crashes with a native stack overflow".
const MAX_JIT_CALL_DEPTH: u32 = 1024;

/// How many `VM::resolve_possible_deopt` calls may be nested on the
/// REAL native call stack at once (see `VM::deopt_reentrancy_depth`)
/// before the currently-deopting function gets permanently disabled.
///
/// This is deliberately a REENTRANCY-DEPTH bound, not a lifetime
/// total: a guard that fails once, in isolation, is normal and cheap
/// (see `VM::invoke_compiled`) no matter how many times that happens
/// over a function's lifetime, AS LONG AS each occurrence resolves
/// and returns before the next one starts -- e.g. a hot loop that
/// calls the same function 500,000 times and mispredicts a handful of
/// times, scattered and non-overlapping, never grows the native stack
/// at all, since each deopt's nested interpreter call fully unwinds
/// before the next TOP-LEVEL call even begins. A per-function
/// lifetime counter would (and, in an earlier version of this
/// mechanism, DID) misfire on exactly that harmless case, permanently
/// falling back to pure interpretation for a function that was
/// otherwise an excellent compile target.
///
/// The real danger is NESTED deopts: a compiled call that's still
/// on-stack (hasn't returned) when a callee it invoked -- directly or
/// via further recursion -- ALSO deopts, resuming via a NEW nested
/// interpreter call from inside `resolve_possible_deopt` rather than
/// a true non-recursive return. A call site that's genuinely
/// polymorphic (e.g. alternating types every invocation, never
/// settling on the speculative guess) can keep this nesting growing
/// with no bound, and each level costs much more native stack than an
/// ordinary compiled call frame (a whole `invoke_compiled` +
/// `resolve_possible_deopt` + interpreter-dispatch frame, not just
/// one), so this is bounded far more tightly than the general
/// `MAX_JIT_CALL_DEPTH`. Past this bound, `resolve_possible_deopt`
/// gives up on compiled code for the function AT THE DEEPEST NESTED
/// LEVEL permanently (mirrors the existing sticky `ineligible`
/// pattern), which is what actually breaks the recursion: once
/// `entry` is cleared, `tiered_entry`/`maybe_osr` never offer compiled
/// code for that function again, so nothing re-enters
/// `resolve_possible_deopt` from inside the interpreter run this
/// deopt just started.
const MAX_DEOPT_REENTRANCY: u32 = 64;

/// Inline capacity for a native/operator-override/constructor call's
/// argument list. `Value` is a plain Copy u64, so this is a handful of
/// stack bytes -- covers the overwhelming majority of real calls (few
/// natives or constructors take more than a handful of arguments) with
/// zero heap allocation. A call that genuinely needs more spills into
/// a Vec exactly once, the same cost the old always-Vec version paid
/// on every call.
const INLINE_ARGS: usize = 255;

enum CallArgs {
  Inline([Value; INLINE_ARGS], usize),
  Spilled(Vec<Value>),
}

impl CallArgs {
  #[inline]
  fn new() -> Self {
    CallArgs::Inline([Value::nil(); INLINE_ARGS], 0)
  }

  #[inline]
  fn push(&mut self, v: Value) {
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
  fn as_slice(&self) -> &[Value] {
    match self {
      CallArgs::Inline(buf, len) => &buf[..*len],
      CallArgs::Spilled(vec) => vec.as_slice(),
    }
  }
}

struct CallFrame {
  function: *const ObjFunction,
  /// The specific closure instance this frame is executing -- needed
  /// whenever GetUpval/SetUpval/Closure look at "my own captured
  /// upvalues". Distinct from `function` (the shared, static prototype)
  /// the same way `ObjClosure` is distinct from `ObjFunction`.
  closure: *const ObjClosure,
  /// The same closure as `closure`, but as the tagged `Value` it was
  /// called through rather than a raw pointer. `function`/`closure`
  /// stay raw pointers purely so the hot instruction-dispatch loop
  /// doesn't pay for a tag check on every fetch; this field is what the
  /// GC's root scan actually walks to keep those raw pointers valid --
  /// see `VM::collect_garbage`.
  closure_val: Value,
  ip: usize,
  /// Index into `VM::registers` where this frame's register window starts.
  base: usize,
  /// Register (in the *caller's* window) that the return value should be
  /// written to. Unused for the outermost frame.
  dst_in_caller: u8,
}

/// One active `catch` statement's unwind target -- see the module-level
/// design note in this diff's accompanying explanation for the full
/// reasoning behind `frame_depth`'s role in distinguishing "this
/// exception is mine to catch" from "belongs to an ancestor
/// `run_until` invocation, possibly across a `call_value` boundary".
struct CatchHandler {
  /// `self.frames.len()` at the moment `PushCatch` executed -- the
  /// frame containing the `catch` statement itself is `frames[frame_depth-1]`.
  frame_depth: usize,
  /// Absolute instruction index (within that same frame) to resume at,
  /// whichever path is taken -- normal completion or an unwind.
  resume_ip: usize,
  var_reg: Option<u8>,
}

/// Outcome of handling a propagated exception -- what the `#[cold]`
/// exception path hands back to `run_until` so it can refresh its
/// cached frame-state locals (or propagate) without that refresh
/// logic itself needing to live in the cold path.
enum ExceptionOutcome {
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
  /// One flat register stack shared by every call frame; each frame just
  /// claims a slice of it (its "window"), exactly like Lua's VM.
  registers: Vec<Value>,
  /// Mirrors `registers.as_mut_ptr()`, updated at the 3 sites that can
  /// reallocate `registers` (see `sync_regs_ptr_cache`). Exists so
  /// compiled code (see `jit::codegen`) can re-fetch the current
  /// registers pointer with a single direct memory load at a
  /// compile-time-baked offset (`VM_REGS_PTR_CACHE_OFFSET`) instead of
  /// an FFI call into `registers_ptr()` -- this is refetched at every
  /// helper-call site, so replacing a real function call with a load
  /// there matters a lot for call-heavy compiled code.
  regs_ptr_cache: Cell<*mut Value>,
  /// Upvalues that are still Open, as (absolute register index, the
  /// Obj::Upvalue Value at that index). Consulted whenever a new closure
  /// captures a local -- if one's already open for that exact register,
  /// it's reused rather than duplicated, which is what makes two
  /// closures over the same variable see each other's writes.
  open_upvalues: Vec<(usize, Value)>,
  frames: Vec<CallFrame>,
  /// Backing storage for every global variable, indexed by slot.
  /// Slots are assigned lazily the first time a name is resolved (see
  /// `get_or_create_global_slot`) and never reused or removed -- what
  /// lets `Chunk::global_cache` cache a slot index permanently with no
  /// invalidation logic needed.
  global_slots: Vec<Cell<Value>>,
  /// Mirrors `global_slots.as_ptr()`, updated at the one site that can
  /// reallocate `global_slots` (see `get_or_create_global_slot`) --
  /// same purpose and pattern as `regs_ptr_cache`, letting compiled
  /// code's `GetGlobal` fast path (see `jit::codegen`'s own docs on
  /// `JitInfo::global_slot_cache`) index straight into current storage
  /// with a direct load at a compile-time-baked offset
  /// (`VM_GLOBAL_SLOTS_PTR_CACHE_OFFSET`) instead of a helper call.
  global_slots_ptr_cache: Cell<*const Cell<Value>>,
  /// name -> slot, consulted only on a `global_cache` MISS -- i.e. the
  /// very first time a particular Get/Set/AssignGlobal instruction
  /// executes, ever. Every later execution of that instruction goes
  /// straight through the cache and never touches this map again.
  global_names: FxHashMap<String, u32>,
  /// Explicit extra GC roots for values internal (non-bytecode) VM code
  /// needs to keep alive across a call that might itself trigger a
  /// collection -- e.g. `instantiate` invoking several field
  /// initializers in sequence. A Value sitting only in a local Rust
  /// variable, with nothing in any register/global/frame pointing at
  /// it, is invisible to the normal root scan; push it here for exactly
  /// as long as it needs to survive, then truncate back off.
  gc_pins: Vec<Value>,
  /// Active `catch` handlers, innermost (most recently pushed) last --
  /// see `CatchHandler`'s own doc comment.
  catch_stack: Vec<CatchHandler>,
  /// Owns the Cranelift `JITModule` and drives whole-function
  /// compilation -- see `crate::jit`. Lazily constructed (see
  /// `VM::jit_engine`) on the FIRST actual compilation attempt, not at
  /// `VM::new()` time -- setting up an ISA, a `JITModule`, and every
  /// `jit::runtime` helper's signature is real, measurable work
  /// (target/CPU-feature probing plus dozens of signature
  /// declarations) that a short-lived script -- or any run with
  /// `ZURI_JIT=0`, or one where nothing ever gets hot enough to
  /// compile -- has no reason to pay at startup. Once built, it lives
  /// for the rest of the process; compiled machine code is never
  /// unloaded or recompiled.
  jit_engine: Option<JitEngine>,
  /// Handle to the single background compiler thread (see
  /// `jit::background`) -- lazily spawned alongside `jit_engine`, on
  /// the same "don't pay for it until actually needed" principle.
  /// `None` until the first function crosses its warmup threshold.
  jit_compiler: Option<background::JitCompilerHandle>,
  /// GC roots for every function with a background compile currently
  /// enqueued or in flight -- pinned here from the moment a job is
  /// sent to `jit_compiler` until its result is drained
  /// (`drain_jit_results`), since the compiled-code round trip crosses
  /// a thread boundary the GC can't otherwise see into. Scanned by
  /// `collect_garbage` exactly like `gc_pins`. See `jit::background`'s
  /// module docs for the full safety argument.
  pending_jit_compiles: Vec<Value>,
  /// Side channel a `crate::jit::runtime` helper sets to a non-nil
  /// exception `Value` exactly when it needs to propagate a failure
  /// out of currently-executing compiled code. Compiled code has no
  /// unwinder of its own (see the `jit` module's top-level docs on why
  /// exceptions always cause a bailout rather than being handled
  /// in-place) -- this is what `VM::invoke_compiled` checks immediately
  /// after a compiled call returns to decide `Ok(value)` vs
  /// `Err(exception)`. Always nil outside the brief window between a
  /// helper setting it and `invoke_compiled` observing + clearing it.
  pub(crate) jit_pending_exception: Cell<Value>,
  /// Side channel a `crate::jit::runtime` deopt helper sets to the
  /// bytecode `ip` compiled code should resume interpreting at, right
  /// before returning early out of the currently-executing compiled
  /// function. Unlike `jit_pending_exception`, this is a genuine
  /// "give up on compiled code for this invocation, but nothing went
  /// wrong" signal -- it's what backs real deoptimization: a
  /// speculative guard that turns out wrong bails all the way out to
  /// the interpreter with `VM::registers` already holding the correct
  /// state (every VM register lives there for the whole time compiled
  /// code runs, never only in a native machine register -- see
  /// `jit::codegen`'s module docs), so "resuming" is just "let the
  /// interpreter's own dispatch loop take over at this `ip`", no
  /// state reconstruction needed. `VM::invoke_compiled` checks this
  /// FIRST (before the exception channel above), since a deopt is
  /// orthogonal to a real error. Always `None` outside the brief
  /// window between a deopt helper setting it and `invoke_compiled`
  /// observing + clearing it.
  pub(crate) pending_deopt_ip: Cell<Option<usize>>,
  /// Master on/off switch for tiering up at all, read once from
  /// `ZURI_JIT` at startup (`"0"`/`"off"`/`"false"` disables it) --
  /// purely a benchmarking/debugging escape hatch. Every program
  /// behaves identically either way, just slower with it off (always
  /// interpreted, exactly like this VM before this tier existed).
  jit_enabled: bool,
  /// How many compiled-function calls are currently nested on the REAL
  /// native call stack -- see `MAX_JIT_CALL_DEPTH`.
  jit_call_depth: Cell<u32>,
  /// How many `resolve_possible_deopt` calls are currently nested on
  /// the REAL native call stack -- see `MAX_DEOPT_REENTRANCY`.
  deopt_reentrancy_depth: Cell<u32>,
  /// Cached by name after `prelude::install` runs, for O(1) lookup from
  /// `VM::raise` rather than a `self.globals` hashmap hit on every
  /// internal error.
  pub(crate) builtin_exceptions: FxHashMap<&'static str, Value>,
  /// Every module loaded so far this run, keyed by its canonical
  /// filesystem path (or `"builtin:NAME"` for a synthetic builtin
  /// module) -- what makes re-importing the same module a no-op instead
  /// of re-executing it, and what breaks circular imports (see
  /// `vm::modules::load_from_candidate`). Also a GC root: a cached
  /// module must stay alive for the rest of the run even if nothing
  /// else currently references it, since a LATER `import` of the same
  /// path must find it again.
  pub(crate) modules: FxHashMap<String, Value>,
  /// The application's entry-file path, as set by `set_root_path` --
  /// becomes every module's `__root__`. `None` in REPL mode, per spec.
  pub(crate) root_path: Option<String>,
  pub heap: Heap,
  /// Per-opcode execution counts, gathered only under
  /// ZURI_OPCODE_PROFILE -- checked once per instruction (a single
  /// bool read via LazyLock, same cost every other debug flag here
  /// already pays), and otherwise entirely inert.
  #[cfg(feature = "opcode-profile")]
  opcode_counts: FxHashMap<&'static str, u64>,
  /// Counts of CONSECUTIVE opcode pairs -- what actually identifies a
  /// good instruction-fusion candidate, since fusing two opcodes only
  /// helps if they're frequently adjacent in real bytecode, not just
  /// individually common.
  #[cfg(feature = "opcode-profile")]
  opcode_bigrams: FxHashMap<(&'static str, &'static str), u64>,
  #[cfg(feature = "opcode-profile")]
  last_opcode: Option<&'static str>,
}

/// Byte offset of `VM::heap` within `VM` -- combined in `crate::jit` with
/// `object::HEAP_BYTES_ALLOCATED_OFFSET`/`HEAP_NEXT_GC_OFFSET` so compiled
/// code can inline `Heap::needs_gc()`'s check (a plain integer compare)
/// as two direct loads instead of an unconditional FFI call at every
/// safepoint. Sound within this compilation: `offset_of!` asks the
/// compiler for VM's actual layout rather than assuming one.
pub(crate) const VM_HEAP_OFFSET: usize = std::mem::offset_of!(VM, heap);
/// Byte offset of `VM::regs_ptr_cache` -- see that field's own docs.
pub(crate) const VM_REGS_PTR_CACHE_OFFSET: usize = std::mem::offset_of!(VM, regs_ptr_cache);
/// Byte offset of `VM::global_slots_ptr_cache` -- see that field's own docs.
pub(crate) const VM_GLOBAL_SLOTS_PTR_CACHE_OFFSET: usize =
  std::mem::offset_of!(VM, global_slots_ptr_cache);

type RunResult<T> = Result<T, Value>;

impl VM {
  pub fn new(heap: Heap) -> Self {
    VM {
      is_repl: false,
      registers: Vec::new(),
      regs_ptr_cache: Cell::new(std::ptr::null_mut()),
      global_slots_ptr_cache: Cell::new(std::ptr::null()),
      frames: Vec::new(),
      open_upvalues: Vec::new(),
      gc_pins: Vec::new(),
      catch_stack: Vec::new(),
      jit_engine: None,
      jit_compiler: None,
      pending_jit_compiles: Vec::new(),
      jit_pending_exception: Cell::new(Value::nil()),
      pending_deopt_ip: Cell::new(None),
      jit_enabled: !matches!(
        std::env::var("ZURI_JIT").as_deref(),
        Ok("0") | Ok("off") | Ok("false")
      ),
      jit_call_depth: Cell::new(0),
      deopt_reentrancy_depth: Cell::new(0),
      builtin_exceptions: FxHashMap::default(),
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
      heap,
    }
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

  /// Resolve `name` to a slot in whichever globals table `module`
  /// names (`None` = the VM's own root table), growing that SPECIFIC
  /// table with a fresh nil slot if `name` hasn't been seen there
  /// before. Never touches any OTHER table -- this is what keeps two
  /// modules (or a module and the root script) that happen to declare
  /// the same name from colliding with each other.
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

  /// Resolve `name` against `module`'s own namespace first, then --
  /// if it isn't declared there -- fall back to the VM's shared root
  /// table. This is what lets module code see built-in natives
  /// (`bytes()`, `print()`, `is_string()`, ...) and the prelude's
  /// Error hierarchy, both of which live only in the root table,
  /// while still letting a module shadow any of those names with its
  /// own declaration. `None` (main script/REPL) has nothing to fall
  /// back FROM -- it just resolves against root directly.
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

  /// Records the application's entry-file path -- every module loaded
  /// afterward gets this as its own `__root__`. Call before `run`; not
  /// meaningful (and not called) in REPL mode.
  pub fn set_root_path(&mut self, path: impl Into<String>) {
    self.root_path = Some(path.into());
  }

  /// Seeds `__file__` (and, if `set_root_path` was called, `__root__`)
  /// into the VM's own ROOT global table -- what makes them visible to
  /// the MAIN script itself, exactly as if it were a module. Every
  /// module loaded via `import` gets the same two variables seeded into
  /// its own separate namespace instead -- see
  /// `vm::modules::seed_module_vars`. Not called for the REPL, matching
  /// the documented "not defined in REPL mode" behavior for `__root__`
  /// (and there's no meaningful `__file__` for a REPL line either).
  pub fn init_entry_globals(&mut self, file_path: &str) {
    let file_val = self.heap.alloc_string(file_path.to_string());
    self.define_global("__file__", file_val);
    if let Some(root) = self.root_path.clone() {
      let root_val = self.heap.alloc_string(root);
      self.define_global("__root__", root_val);
    }
  }

  /// Construct a fresh instance of the builtin exception class named
  /// `class_name` (from `prelude::EXCEPTION_CLASS_NAMES`), with
  /// `message`/`type` set directly by field-slot name (bypassing the
  /// normal constructor-call path entirely, since these are ALWAYS the
  /// prelude's own known classes -- no user override to worry about),
  /// and its `stacktrace` attached. This is what every internal VM
  /// error site calls instead of returning a bare Rust string.
  pub(crate) fn raise(&mut self, class_name: &'static str, message: impl Into<String>) -> Value {
    let message_str = message.into();
    let class_val = *self.builtin_exceptions.get(class_name).unwrap_or_else(|| {
      panic!(
        "internal error: unknown builtin exception class '{}' (prelude not installed?)",
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

  /// Is `v` an instance of `Error` or one of its subclasses? What
  /// `Instr::Raise` checks before allowing a value to propagate as an
  /// error, and what `raise`'s own output always satisfies trivially.
  fn is_exception_value(&self, v: Value) -> bool {
    if !v.is_instance() {
      return false;
    }
    let Some(&exception_class) = self.builtin_exceptions.get("Error") else {
      return false;
    };
    let mut cur = Some(v.as_instance().class);
    while let Some(c) = cur {
      if c.equals(&exception_class) {
        return true;
      }
      cur = c.as_class().superclass;
    }
    false
  }

  /// Frame names, innermost first, as a Zuri list of strings -- what
  /// gets attached to every exception's `stacktrace` field. No line
  /// numbers yet (bytecode doesn't carry source positions), just the
  /// call chain by function name.
  fn build_stacktrace(&mut self) -> Value {
    let mut lines = Vec::with_capacity(self.frames.len());

    for frame in self.frames.iter().rev() {
      let func = unsafe { &*frame.function };
      let line = func
        .chunk
        .lines
        .get(frame.ip.saturating_sub(1))
        .copied()
        .unwrap_or(0);
      let entry = format!("{}:{} -> {}()", func.source_path, line, func.name);
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

  /// Formats an uncaught exception for top-level reporting (see
  /// zuri.rs) -- "TYPE: message", falling back gracefully if `exc`
  /// somehow isn't an instance at all.
  pub fn describe_exception(&self, exc: Value) -> String {
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

  /// Full multi-line "Unhandled ..." block for an uncaught exception --
  /// what the CLI/REPL print at the top level (see zuri.rs).
  /// `describe_exception` stays the short "TYPE: message" summary,
  /// still used e.g. by the prelude's own internal panic path.
  pub fn format_uncaught(&self, exc: Value) -> String {
    let summary = self.describe_exception(exc);
    if !exc.is_instance() {
      return format!("Unhandled {}", summary);
    }
    let inst = exc.as_instance();
    let trace_idx = inst.class.as_class().field_slots.get("stacktrace").copied();
    let mut out = format!("Unhandled {}\n  StackTrace:", summary);
    if let Some(idx) = trace_idx {
      let trace_val = inst.fields[idx as usize].get();
      if trace_val.is_list() {
        for line in trace_val.as_list() {
          out.push_str(&format!("\n    {}", line));
        }
      }
    }
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
    });
    self.run_until(0)?;
    Ok(())
  }

  /// Invoke any callable Value -- a closure OR a native -- with the
  /// given (already-evaluated, owned) arguments, run it to completion,
  /// and return its result. This is what lets a native function call
  /// BACK into Zuri code: a future `map(list, fn)` plugin would call
  /// this once per element with `fn` as the callee.
  pub fn call_value(&mut self, callee: Value, args: &[Value]) -> RunResult<Value> {
    if callee.is_native() {
      let native = callee.as_native();
      return self.call_native(native, args);
    }
    if !callee.is_closure() {
      let msg = format!("cannot call a {}", callee.type_name());
      return Err(self.raise("TypeError", msg));
    }

    let closure = callee.as_closure();
    let proto = closure.function.as_func();
    let required = if proto.variadic {
      proto.arity - 1
    } else {
      proto.arity
    };

    // Same convention dispatch_call already uses for Instr::Call: place
    // the new frame right after whatever frame is CURRENTLY executing,
    // instead of always appending at self.registers.len(). This bounds
    // growth by max simultaneous call depth -- exactly like ordinary
    // bytecode recursion already is -- rather than growing once per
    // call_value invocation forever. No truncate-on-return needed: like
    // dispatch_call's own recursion, later calls at the same depth just
    // reuse the already-grown capacity (resize only fires when
    // `needed > self.registers.len()`), so this is self-bounding on its
    // own without shrinking anything mid-flight.
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
    });
    self.run_frame(stop_depth, proto, callee)
  }

  //-----------------------------------------------------------------------------------
  // JIT tiering -- see `crate::jit` for the compiled-code side of all of
  // this. Every entry point below assumes its caller has ALREADY pushed
  // the `CallFrame` this call/loop is executing (matching `run_until`'s
  // own precondition) -- these methods only ever decide "run the
  // already-active top frame interpreted, or hand it to compiled code",
  // never frame setup itself.
  //-----------------------------------------------------------------------------------

  /// Refreshes `regs_ptr_cache` to match `registers`' current backing
  /// buffer -- MUST be called immediately after every `self.registers
  /// .resize(..)`, with no exceptions, since compiled code trusts this
  /// cache implicitly (a single direct memory load, no bounds/staleness
  /// check of its own -- see `VM_REGS_PTR_CACHE_OFFSET`).
  #[inline]
  fn sync_regs_ptr_cache(&mut self) {
    self.regs_ptr_cache.set(self.registers.as_mut_ptr());
  }

  fn sync_global_slots_ptr_cache(&mut self) {
    self.global_slots_ptr_cache.set(self.global_slots.as_ptr());
  }

  /// Does `proto` have a compiled entry point ready to use RIGHT NOW?
  /// Never blocks: because the JIT is disabled, this exact prototype
  /// was found ineligible (contains `Raise`/`PushCatch`/`PopCatch` --
  /// see the `jit` module's docs), the real call stack is already deep
  /// enough that handing it another native call risks overflowing it
  /// (see `MAX_JIT_CALL_DEPTH`), or it isn't warm enough yet, this
  /// returns `None` immediately. Once `proto` IS warm, this either
  /// finds a background compile already in flight (does nothing
  /// further) or enqueues one (see `jit::background`) -- EITHER WAY it
  /// still returns `None` for this exact call, so the interpreter
  /// keeps running `proto` interpreted for as many further calls as it
  /// takes the background thread to finish, then transparently
  /// switches over once `drain_jit_results` installs the result.
  ///
  /// This is THE hot-path check -- reached on every single
  /// `Instr::Call`/`Invoke`/`InvokeSuper`/`CallSuperCtor`, so the
  /// common case (already compiled) is nothing more than an enabled-
  /// flag read, a depth-counter read, a non-blocking channel drain,
  /// and one `Cell::get()` on `proto.jit.entry`. See
  /// `object::JitInfo::entry`'s own docs for why this is deliberately
  /// NOT behind a `RefCell<Option<Rc<...>>>`.
  ///
  /// `proto_value` must be the exact `Value` (`Obj::Func`-tagged) that
  /// owns `proto` -- used to pin it as a GC root if this call ends up
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

  /// The Cranelift engine, built on first use -- see `jit_engine`
  /// field's own docs for why this is lazy rather than built in
  /// `VM::new()`.
  fn jit_engine(&mut self) -> &mut JitEngine {
    self.jit_engine.get_or_insert_with(JitEngine::new)
  }

  /// The background compiler thread's channel handle, lazily spawned
  /// on first use -- see `jit_compiler` field's own docs.
  fn jit_compiler(&mut self) -> &mut background::JitCompilerHandle {
    if self.jit_compiler.is_none() {
      let isa = self.jit_engine().isa_handle();
      self.jit_compiler = Some(background::spawn(isa));
    }
    self.jit_compiler.as_mut().unwrap()
  }

  /// Builds `proto`'s IR right now (synchronously -- the only stage
  /// that touches `proto`, see `jit::background`'s module docs) and
  /// hands the result to the background compiler thread for the
  /// expensive part, pinning `proto_value` as a GC root for the round
  /// trip. If IR-building itself fails, `proto` is marked permanently
  /// ineligible immediately (no point enqueueing anything) -- exactly
  /// mirroring the old synchronous `try_compile`'s failure handling,
  /// just without the (now background-only) backend-compile step ever
  /// getting a chance to also fail here.
  fn enqueue_compile(&mut self, proto: &ObjFunction, proto_value: Value) {
    let speculative_params = self.combined_param_feedback(proto);
    let speculative_regs = self.sample_all_reg_types(proto);
    let __diag_start = std::time::Instant::now();
    let pending = match self
      .jit_engine()
      .build_ir(proto, speculative_params, speculative_regs)
    {
      Ok(pending) => pending,
      Err(reason) => {
        if crate::jit::log_enabled() {
          eprintln!("[jit] '{}' ineligible: {}", proto.name, reason);
        }
        proto.jit.ineligible.set(true);
        return;
      },
    };
    if std::env::var_os("ZURI_DIAG_CALLS").is_some() {
      eprintln!(
        "[diag] synchronous build_ir for '{}' took {:?}",
        proto.name,
        __diag_start.elapsed()
      );
    }

    proto.jit.compiling.set(true);
    self.pending_jit_compiles.push(proto_value);
    let job = background::CompileJob {
      ctx: pending.ctx,
      func_id: pending.func_id,
      osr_ids: pending.osr_ids,
      proto: background::SendPtr(proto as *const ObjFunction),
      speculative_params,
      speculative_regs,
    };
    if self.jit_compiler().job_tx.send(job).is_err() {
      // The background thread is gone -- shouldn't happen (it lives
      // for the whole process), but if it did, undo the pin/flag so
      // `proto` just stays interpreted forever rather than wedged in
      // a permanent "compiling" state no result will ever clear.
      proto.jit.compiling.set(false);
      self.pending_jit_compiles.pop();
    }
  }

  /// Installs every background compile result that's ready RIGHT NOW
  /// (non-blocking) into its prototype's `JitInfo`, and un-pins it.
  /// Called at the top of both `tiered_entry` and `maybe_osr` -- the
  /// only two places that ever check "is this ready yet" -- so results
  /// get installed lazily, exactly when something asks, with no
  /// separate polling thread/timer needed.
  fn drain_jit_results(&mut self) {
    if self.jit_compiler.is_none() {
      return;
    }
    // Collect into an owned `Vec` first rather than looping directly
    // on `try_recv` while also calling `self.jit_engine()` for each --
    // avoids overlapping the channel's borrow of `self.jit_compiler`
    // with the `&mut self` each result's installation needs.
    let mut results = Vec::new();
    while let Ok(result) = self.jit_compiler.as_ref().unwrap().result_rx.try_recv() {
      results.push(result);
    }
    for result in results {
      // SAFETY: `result.proto` was pinned in `pending_jit_compiles`
      // from the moment its job was enqueued until right here -- see
      // `jit::background`'s module docs.
      let proto = unsafe { &*result.proto.0 };
      let install_outcome = result.outcome.and_then(|(bytes, alignment, relocs)| {
        self
          .jit_engine()
          .install_compiled(result.func_id, alignment, &bytes, &relocs)
      });
      match install_outcome {
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

  /// A single-call type sample of `proto`'s FIXED-arity parameters,
  /// read from THE CURRENT TOP FRAME (by the time this is ever called,
  /// `self.frames.last()` is ALWAYS a frame for `proto` -- either just
  /// pushed with real argument values already placed at its own `base`
  /// by `setup_closure_call`/`call_value`, for an ordinary call, or the
  /// currently-executing frame itself, for an OSR trigger, whose
  /// parameter registers still hold whatever this same invocation's
  /// arguments evolved into by now). Two callers: `record_call_feedback`
  /// (one sample per call, folded into a running multi-call AND) and
  /// `combined_param_feedback`'s fallback (no accumulated evidence
  /// exists yet, so bet on this one call same as before that existed).
  /// Either way, betting on real observed values here is sound because
  /// whatever mask a caller ends up passing to `codegen::compile` only
  /// ever seeds a GUARD that re-validates the same registers for real
  /// before ever trusting them on any later call -- see
  /// `jit::codegen::compile`'s own docs.
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

  /// A ONE-SHOT type sample of EVERY register in `proto`'s currently
  /// executing frame (not just its fixed-arity parameters -- compare
  /// `sample_param_types`), taken at the same moment and under the
  /// same precondition (`self.frames.last()` is `proto`'s own frame).
  /// Feeds `jit::typeflow::SpeculativeRegs`: a register whose value
  /// came from a `GetField`/`Call`/`GetIndex`/... result that happens
  /// to be a number RIGHT NOW gets a real runtime guard planted at that
  /// instruction's own definition site in the specialized body (see
  /// `codegen::FuncCompiler::emit_speculative_guard`), which is what
  /// makes betting on it here sound: nothing downstream ever trusts
  /// this sample directly, only whatever guard it results in re-
  /// validating the ACTUAL value on every future execution.
  ///
  /// Deliberately a single one-shot sample, not accumulated across
  /// calls the way `record_call_feedback` accumulates parameter
  /// feedback -- a register beyond the parameter range doesn't have a
  /// stable, call-independent "value at this call" the way a parameter
  /// does (it might be a totally different bytecode-level variable at
  /// different points across different calls), so continuous
  /// accumulation isn't the natural fit here the way it was for
  /// parameters. A future increment could add per-definition-site
  /// accumulation if the one-shot version proves too noisy in
  /// practice.
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

  /// Folds ONE call's argument types into `proto`'s running
  /// `numeric_feedback` accumulator (bitwise AND) -- called at every
  /// site that increments `call_count`, right after `proto`'s own
  /// frame has been pushed, so `sample_param_types` always samples
  /// THIS call. Unlike a one-shot bet on whichever single call happens
  /// to tip the warm-up threshold, this observes EVERY call along the
  /// way: a parameter's bit only survives to compile time if it was
  /// numeric on every call seen so far, exactly the "keep believing
  /// the guess until a real call contradicts it" pattern of a
  /// polymorphic inline cache. Skipped once `proto` is compiled,
  /// already enqueued, or ineligible -- feedback stops mattering (and
  /// costing anything) the moment it can no longer inform a
  /// not-yet-made decision.
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

  /// The type-feedback mask actually consulted at the moment `proto`
  /// is enqueued for compilation: the AND-accumulated result of every
  /// call `record_call_feedback` has recorded since `proto` started
  /// warming up (which, by construction, already includes this exact
  /// triggering call -- it's recorded at the very same call sites that
  /// increment `call_count`, before `tiered_entry`/`maybe_osr` can ever
  /// decide to compile). Falls back to a fresh one-shot
  /// `sample_param_types` only if NO call was ever recorded through
  /// that path -- e.g. a top-level script's own OSR-triggered compile,
  /// whose outermost frame is never pushed via a "call" at all -- so
  /// this is never any worse than the single-sample behavior it
  /// replaces, only better-informed when real accumulated evidence
  /// exists.
  fn combined_param_feedback(&self, proto: &ObjFunction) -> Option<u64> {
    if proto.jit.feedback_samples.get() > 0 {
      Some(proto.jit.numeric_feedback.get())
    } else {
      self.sample_param_types(proto)
    }
  }

  /// Given a frame that was JUST pushed for `proto`/`closure_val` (so
  /// `stop_depth` is exactly what `run_until` needs to know when to
  /// stop), either interpret it (today's unbounded-depth behavior,
  /// unchanged) or -- once it's warm enough, compiling it right now if
  /// needed -- run it as compiled machine code instead. Used by
  /// `call_value` (natives/builtins calling back into Zuri code) so
  /// that path benefits from tiering exactly like ordinary bytecode
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

  /// Pins every value in `values` into `gc_pins` for the duration the
  /// caller needs them to survive a re-entrant `call_value` (which
  /// runs arbitrary Zuri code and can trigger a collection), and
  /// returns the index the FIRST one landed at -- every value is at
  /// `mark + i` for its position `i` in the iterator, and also
  /// exactly the mark `unpin` needs to release them all again.
  ///
  /// This exists because a `Value` sitting only in a plain Rust local
  /// (or a `Vec` cloned out of a list/dict's own storage, or a
  /// borrowed argument slice like `ZuriContext::args`) has NO way to
  /// be found and rewritten if the object it names gets relocated by
  /// a collection that runs mid-loop, inside some earlier iteration's
  /// own `call_value`. `gc_pins` is a real GC root (scanned by both
  /// `collect_garbage` and `collect_minor`), so a value pinned here
  /// stays correctly address-updated across any number of further
  /// re-entrant calls -- AS LONG AS every subsequent read goes back
  /// to `self.gc_pins[idx]` fresh each time, rather than trusting a
  /// copy taken before an intervening call. See `VM::instantiate` for
  /// the pattern this generalizes (and the bug -- a `Box(...)`
  /// constructor call intermittently reading a relocated-and-
  /// neutralized slot back as `Obj::Range` -- that motivated it).
  pub(crate) fn pin_values(&mut self, values: impl IntoIterator<Item = Value>) -> usize {
    let mark = self.gc_pins.len();
    self.gc_pins.extend(values);
    mark
  }

  /// Releases every pin taken since `mark` (a value previously
  /// returned by `pin_values`) -- see its own docs.
  pub(crate) fn unpin(&mut self, mark: usize) {
    self.gc_pins.truncate(mark);
  }

  /// Reads back a value pinned by `pin_values`, fresh -- the whole
  /// point being that this reflects any relocation a collection made
  /// since the pin, unlike whatever local variable/slice the caller
  /// originally had it in.
  #[inline]
  pub(crate) fn pinned(&self, idx: usize) -> Value {
    self.gc_pins[idx]
  }

  /// Guarantees `closure_val` (the closure a compiled function is
  /// about to be ENTERED through) is not currently `Young` before
  /// handing it to compiled code, relocating it right now if it is.
  /// `jit::codegen`'s `closure_param` is a plain Cranelift SSA value,
  /// loaded once at function entry and reused for the WHOLE compiled
  /// invocation -- every `Instr::Closure`/`GetUpval`/`SetUpval` in
  /// the function body reuses that exact same value, never reloading
  /// it from `VM::registers` or anywhere else GC-scannable. Unlike an
  /// ordinary register, there is NO memory location `VM::collect_minor`
  /// could write a relocated address back into if this object moved
  /// partway through the invocation (say, at a loop back-edge
  /// safepoint) -- so instead, it must simply never be free to move
  /// at all for as long as compiled code might still be holding it.
  ///
  /// Cheap in the overwhelmingly common case: a closure invoked
  /// repeatedly through a warm call site has almost always long since
  /// survived a minor collection already, costing one `generation`
  /// read and nothing else. Only a genuinely still-young closure pays
  /// for a real, full minor collection here.
  pub(crate) fn ensure_stable_for_compiled_entry(&mut self, closure_val: Value) -> Value {
    let ptr = closure_val.as_obj();
    if !Heap::is_young(ptr) {
      return closure_val;
    }
    // Pinned BEFORE the collection, not read back afterward via its
    // original (pre-collection) pointer -- an earlier version of this
    // function did the latter, re-resolving through
    // `Heap::forward_or_promote`'s "already forwarded" branch, which
    // needs the OLD nursery slot's own memory to still be valid to
    // read from. That's true right up until `collect_minor` finishes
    // -- but `reset_nursery` (its very last step) doesn't just wipe
    // the young generation's CONTENTS, it deallocates every nursery
    // chunk beyond the first ENTIRELY (see `Heap::reset_nursery`'s
    // own docs), so if this closure happened to live in a chunk
    // beyond the first -- plausible under real allocation pressure,
    // not an exotic corner case -- the "old slot" the stale pointer
    // named was already freed by the time this looked it up: a
    // genuine use-after-free, caught by this project's own testing as
    // an intermittent segfault inside `Cell::get` reading a class's
    // `generation` field, deep in `string.each`'s own callback
    // invocation. Pinning first sidesteps the whole problem: a real
    // `gc_pins` root gets correctly updated by the SAME collection's
    // own root scan, the ordinary way, before `reset_nursery` ever
    // runs.
    let mark = self.pin_values([closure_val]);
    // A REAL, full minor collection -- not a one-off relocation of
    // just this object -- and deliberately so: this closure is
    // necessarily ALSO reachable from wherever `closure_val` itself
    // came from (a register, a class's `methods` table, an instance
    // field, ...), and relocating it in isolation would fix up only
    // the pinned copy, leaving every OTHER reference to the same
    // object pointing at a slot the collection has since reused or
    // neutralized -- exactly the bug an even earlier version of this
    // function had (caught by `tests/inheritance.zu` reading a
    // leftover placeholder as "cannot call a range"). A full cycle's
    // comprehensive root/child scan is what finds and rewrites every
    // one of those together, the same way it always does.
    self.collect_minor();
    let new_val = self.pinned(mark);
    self.unpin(mark);
    new_val
  }

  /// Runs the CURRENT top frame (already pushed, `self.frames.last()`)
  /// as compiled machine code from `osr_id` (`-1` for an ordinary
  /// entry starting at bytecode ip 0; a non-negative id from
  /// `JitInfo::osr_ids` to jump straight into a specific loop header
  /// instead -- see `maybe_osr`).
  ///
  /// On success, pops the frame and returns `Ok(value)` -- exactly
  /// `Instr::Return`'s own effect. On failure, the frame is left in
  /// place, matching `run_until`'s existing "an uncaught exception
  /// leaves every frame between here and whichever ancestor `catch`
  /// eventually claims it, to be truncated in one shot by
  /// `handle_exception`" behavior -- compiled code never pops on error,
  /// only on success, for exactly that reason.
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
    // SAFETY: `entry` was produced by `jit::engine::JitEngine::compile_function`
    // for THIS exact prototype; `base` is this (already-pushed) frame's
    // own register-window start, matching every other caller of this
    // machine code's calling convention (see `jit::EntryFn`'s docs).
    let result_bits = unsafe { entry(self as *mut VM, base as u64, closure_val.to_bits(), osr_id) };
    self.jit_call_depth.set(self.jit_call_depth.get() - 1);

    // A real deopt takes priority over everything else -- see
    // `resolve_possible_deopt`'s own docs. This exact check (and the
    // recursive-interpreter resolution behind it) is ALSO needed by
    // `jit::runtime::zuri_jit_call_finish`, the OTHER place compiled
    // code's return value gets processed: the direct compiled-to-
    // compiled fast path (`emit_fast_call`'s own `call_indirect`,
    // used for e.g. self-recursive calls) never goes through THIS
    // function at all, so a deopt happening there would otherwise go
    // completely unnoticed -- exactly the bug that shipped first and
    // got caught by `tests/inheritance.zu`/`osr_speculation_stress.zu`
    // regressing. Both call sites MUST resolve a pending deopt before
    // doing anything else with compiled code's return value.
    if let Some(result) = self.resolve_possible_deopt() {
      return result;
    }

    let pending = self.jit_pending_exception.get();
    if !pending.is_nil() {
      self.jit_pending_exception.set(Value::nil());
      return Err(pending);
    }

    self.close_upvalues_from(base);
    self.frames.pop();
    Ok(Value::from_bits(result_bits))
  }

  /// Checks (and clears) `pending_deopt_ip`. If compiled code just
  /// bailed out, the CURRENT top frame -- whichever one that is; this
  /// is called from both `invoke_compiled` (a freshly-pushed frame on
  /// a regular call, or an already-active frame OSR'd into mid-loop)
  /// and `jit::runtime::zuri_jit_call_finish` (the direct compiled-
  /// to-compiled fast path's own callee frame) -- is still exactly as
  /// valid as it always was; the only thing wrong is compiled code
  /// gave up on it partway through. Point its own `ip` at the deopt
  /// target and hand it to a fresh, depth-bounded interpreter run --
  /// `run_until` seeds its `ip`/`base`/etc. straight from
  /// `self.frames[frame_idx]` (see its own top), so this is the
  /// entire fix-up needed, and this exact "recurse into the
  /// interpreter for one bounded frame" shape is already proven sound
  /// by `run_frame`'s native-callback path. Both callers see an
  /// ordinary `RunResult<Value>` either way -- deopt is fully
  /// invisible above this function.
  /// Tiny, always-inlined fast-path check -- the overwhelming common
  /// case (no deopt pending) is just one `Cell<Option<usize>>` read,
  /// kept as small as possible so it disappears into its callers'
  /// own hot paths (`invoke_compiled`, `zuri_jit_call_finish`) rather
  /// than costing a real out-of-line call on every single compiled
  /// call, deopt or not. The actual (rare) resolution logic is kept
  /// OUT of line in `resolve_deopt_slow`, both so it doesn't bloat
  /// the hot path's icache footprint and so its own locals/borrows
  /// don't fight this function's inlining eligibility.
  #[inline(always)]
  pub(crate) fn resolve_possible_deopt(&mut self) -> Option<RunResult<Value>> {
    let deopt_ip = self.pending_deopt_ip.take()?;
    Some(self.resolve_deopt_slow(deopt_ip))
  }

  /// The actual (rare) deopt-resolution logic -- see
  /// `resolve_possible_deopt`'s own docs for why this is split out.
  #[cold]
  #[inline(never)]
  fn resolve_deopt_slow(&mut self, deopt_ip: usize) -> RunResult<Value> {
    let frame_idx = self.frames.len() - 1;
    // SAFETY: this frame's `function` has been a valid, live
    // `ObjFunction` for as long as the frame itself has existed --
    // same pointer every other `unsafe { &*frame.function }` site in
    // this file already trusts.
    let deopting_fn = unsafe { &*self.frames[frame_idx].function };
    let depth = self.deopt_reentrancy_depth.get() + 1;
    self.deopt_reentrancy_depth.set(depth);
    if depth > MAX_DEOPT_REENTRANCY {
      // See `MAX_DEOPT_REENTRANCY`'s own docs: this nested deopt chain
      // has grown deep enough that continuing to let it grow risks a
      // real native stack overflow -- give up on compiled code for
      // the function at THIS deepest level FOR GOOD, which is what
      // actually stops the chain from growing on the very next nested
      // call. Clearing `entry` (not just `ineligible`) matters:
      // `tiered_entry` checks `entry` first.
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
    let stop_depth = frame_idx;
    let result = self.run_until(stop_depth);
    self
      .deopt_reentrancy_depth
      .set(self.deopt_reentrancy_depth.get() - 1);
    result
  }

  /// Checked by `run_until`'s own `Instr::Jmp` handler on every
  /// BACKWARD jump (a loop back-edge) -- `target_ip` is where that
  /// back-edge lands (the loop header). Returns `None` to mean "keep
  /// interpreting this loop normally" (not hot yet, ineligible, or the
  /// native call stack is already too deep); `Some(outcome)` means
  /// on-stack replacement into compiled code just ran the CURRENT
  /// frame to completion, and the caller must treat that exactly like
  /// `Instr::Return` (`Ok`) or an unhandled exception (`Err`) firing
  /// for this same frame -- NOT resume interpreting it.
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

    // `self.frames.last()` is `func`'s own currently-executing frame
    // (this is only ever reached from a backward jump INSIDE `func`'s
    // own interpreted execution) -- its closure's `function` field is
    // exactly the `Value` `enqueue_compile` needs to pin.
    let closure_val = self.frames.last().unwrap().closure_val;
    let proto_value = closure_val.as_closure().function;
    self.enqueue_compile(func, proto_value);
    None
  }

  /// Pops the current top frame with no upvalue-closing/return-value
  /// bookkeeping -- used only by `jit::runtime::zuri_jit_call_finish`
  /// (and its `Invoke` counterpart), which need `VM::frames` itself
  /// (private to this module) popped from OUTSIDE `vm.rs` after a
  /// direct, inline-cached compiled-to-compiled call completes. Every
  /// other frame-pop site in this file already has direct field access
  /// and doesn't need this wrapper.
  pub(crate) fn pop_frame(&mut self) {
    self.frames.pop();
  }

  /// Is the real native call stack shallow enough to safely add one
  /// more nested compiled call? See `MAX_JIT_CALL_DEPTH`'s own docs.
  /// Exposed as its own cheap, side-effect-free check (rather than
  /// folded into `tiered_entry`) specifically for
  /// `jit::runtime::zuri_jit_call_prepare`/`zuri_jit_invoke_prepare`'s
  /// PURE peek at whether the fast, inline-cache-style direct-call path
  /// applies -- see those functions' own docs on why they never
  /// trigger compilation or touch `call_count` themselves.
  #[inline]
  pub(crate) fn jit_depth_ok(&self) -> bool {
    self.jit_enabled && self.jit_call_depth.get() < MAX_JIT_CALL_DEPTH
  }

  #[inline]
  pub(crate) fn jit_depth_enter(&self) {
    self.jit_call_depth.set(self.jit_call_depth.get() + 1);
  }

  #[inline]
  pub(crate) fn jit_depth_exit(&self) {
    self.jit_call_depth.set(self.jit_call_depth.get() - 1);
  }

  /// Sets up a new register window and pushes a `CallFrame` for
  /// calling `closure` (whose prototype is `proto`) with `num_args`
  /// argument slots ALREADY sitting at `new_base .. new_base+num_args`
  /// -- the exact frame-setup both `dispatch_call`'s Closure arm,
  /// `invoke_prebound`, and the JIT's own fast, inline-cache-style
  /// direct-call path (`jit::runtime::zuri_jit_call_prepare`/
  /// `zuri_jit_invoke_prepare`) all need, kept in exactly one place so
  /// they can never drift apart. Fills any missing fixed parameters
  /// with nil and collects any extra variadic arguments into a list,
  /// matching this VM's established calling convention.
  pub(crate) fn setup_closure_call(
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

    self.frames.push(CallFrame {
      function: proto as *const ObjFunction,
      closure: closure as *const ObjClosure,
      closure_val,
      ip: 0,
      base: new_base,
      dst_in_caller,
    });
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
        // `min_arity`/`args.len()` both count the implicit receiver
        // spliced into `args[0]` (see `builtins::method`/`method_n`/
        // `method_opt`) -- a user calling `x.abs(1)` wrote ONE
        // argument, not two, so both numbers need the receiver
        // subtracted back out before they're shown. This is the SAME
        // check `enforce_method_arg_count!` does inside a native's own
        // body, just running earlier -- for an exact-arity (non-
        // variadic) method, a count mismatch is caught HERE, before
        // the native body (and any `enforce_method_arg_*!` calls in
        // it) ever runs at all.
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

    // Safety net for `gc_pins`: a native that pins values (see
    // `pin_values`) to survive its own re-entrant `call_value`s is
    // expected to `unpin` them again before returning, but an early
    // return via `?` on an error path is easy to miss doing that for
    // (a caught exception from a user callback is an entirely normal
    // outcome here, not a rare edge case). Truncating back to
    // whatever `gc_pins` looked like right before this call, no
    // matter how the native returns, means a missed `unpin` costs
    // nothing worse than holding its pins a little longer than
    // strictly necessary -- never a permanent leak (which would ALSO
    // be a correctness bug, not just wasted memory: `gc_pins` is a
    // real GC root, so anything stuck in it stays uncollectable
    // forever).
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

  /// Construct a new instance of `class_val`: allocate storage sized to
  /// its (already-merged) field layout, run every ancestor's OWN field
  /// initializer root-to-leaf, then call the resolved constructor (if
  /// any) with `args`.
  ///
  /// This is the one place internal VM code makes several SEQUENTIAL
  /// re-entrant calls (`call_value`, which can itself trigger a
  /// collection) while depending on Values that live only in local Rust
  /// variables in between -- the class itself, its ancestors' field
  /// initializers, the constructor, and the caller's own `args`. None of
  /// those are reachable through any register/global/frame during that
  /// window, so each is explicitly pinned for the duration (see
  /// `gc_pins`) rather than trusting the normal root scan to find them.
  fn instantiate(&mut self, class_val: Value, args: &[Value]) -> RunResult<Value> {
    let constructor = class_val.as_class().constructor;
    let field_count = class_val.as_class().field_count;

    // Fast path: does ANY ancestor declare an own field initializer at
    // all? A class whose instance fields are all assigned directly in
    // its own constructor body (no top-level `var name = default`
    // declarations) -- an extremely common shape, e.g. any simple data
    // class -- never needs the `field_inits` list below at all. Worth
    // checking explicitly: building that list, even when every entry in
    // it turns out to be `None`, means a REAL heap allocation (the
    // `Vec` itself) on every single instantiation otherwise -- pure
    // overhead for the common case.
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

    // Every one of these gets pinned, and -- crucially -- EVERY use of
    // one from here on re-reads it from its pinned slot instead of
    // whatever local variable it started in. `call_value` below can
    // trigger a collection (a field initializer or constructor body
    // is arbitrary Zuri code, free to allocate), and unlike a register
    // or global, a plain Rust local has no way to be found and
    // rewritten if the object it names gets relocated -- `gc_pins`
    // only gives it one AS LONG AS every subsequent read goes back to
    // the pinned slot. A local variable captured before the pin (like
    // this function's old `instance_val`/`constructor`/`args` reads
    // used to be, straight through several back-to-back `call_value`s)
    // stays frozen at whatever address it had at THAT moment even
    // after the pinned copy gets moved -- exactly the bug caught by
    // `tmp/gc_write_barrier_stress.zu` intermittently reading a
    // relocated-and-neutralized slot back as `Obj::Range`.
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

  /// Shared "call whatever's in register `func_reg`" logic -- the exact
  /// dispatch `Instr::Call` performs, factored out so Invoke/InvokeSuper's
  /// field-fallback (a field that happens to hold a callable, e.g. `var
  /// _print = @(g) { ... }`) can reach it too, rather than duplicating
  /// native/class/bound-method/closure dispatch a second time. `func_reg`
  /// and `dst` are relative to `base`; arguments must already sit at
  /// `func_reg+1 ..= func_reg+num_args` -- ordinary data-call convention,
  /// arity does NOT include any implicit receiver.
  pub(crate) fn dispatch_call(
    &mut self,
    base: usize,
    func_reg: u8,
    num_args: u8,
    dst: u8,
  ) -> RunResult<()> {
    self.dispatch_call_inner(base, func_reg, num_args, dst, false)
  }

  /// Same dispatch as `dispatch_call`, but for a call site that has NO
  /// flat interpreter loop waiting to pick up a merely-pushed frame --
  /// i.e. a call issued from within already-COMPILED code (see
  /// `jit::runtime::zuri_jit_call`). A `Closure` callee therefore
  /// always runs to full completion synchronously here (through
  /// `run_frame`, exactly like `call_value` already does for a native
  /// calling back into Zuri) rather than being left on `self.frames`
  /// for a caller's own dispatch loop to continue -- there is no such
  /// loop to hand it to.
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
      let msg = format!("cannot call a {}", callee.type_name());
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
        let user_args: Vec<Value> = self.registers[args_start..args_end].to_vec();
        let instance = self.instantiate(callee, &user_args)?;
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
          // No flat interpreter loop is waiting for this frame -- run
          // it to completion right now, interpreted or compiled
          // (`run_frame` decides), exactly like `call_value` already
          // does for a native calling back into Zuri.
          let stop_depth = self.frames.len() - 1;
          let ret = self.run_frame(stop_depth, callee_fn, callee)?;
          self.set_reg(base, dst, ret);
        } else {
          // Mixed-mode dispatch: if `callee_fn` is already warm/
          // compiled (or this exact call is the one that tips it over
          // its own warm-up threshold), run it as compiled machine
          // code RIGHT NOW instead of leaving the frame for the
          // interpreter loop to pick up next iteration. If it's still
          // cold, this falls straight through to `Ok(())` and the
          // existing push-and-continue behavior is completely
          // unchanged.
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
            // Overwrite the callee's own register with the promoted
            // function and recurse -- dispatch_call re-reads `func_reg`
            // fresh at the top, so this reuses every existing dispatch
            // path (closure/native/etc.) for free instead of duplicating
            // it here.
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
        let msg = format!("cannot call a {}", callee.type_name());
        Err(self.raise("TypeError", msg))
      },
    }
  }

  /// Call a CLOSURE whose implicit receiver has ALREADY been placed by
  /// the compiler at `recv_reg + 1` -- the convention behind a genuine
  /// method call (`is_method` reserved that slot at compile time; see
  /// `ObjFunction::is_method`'s doc comment). Unlike `dispatch_call`,
  /// `callee` itself is never written into any register here -- it's
  /// consulted only for its function pointers, since the receiver
  /// occupying what would otherwise be the callee's register is exactly
  /// the point of the fused Invoke/InvokeSuper instructions.
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
  /// interpreter loop waiting -- see `dispatch_call_sync`'s doc
  /// comment, the exact same reasoning applies here for
  /// `Invoke`/`InvokeSuper`/`CallSuperCtor`'s own compiled call sites.
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
      let msg = format!("cannot call a {}", callee.type_name());
      return Err(self.raise("TypeError", msg));
    }

    let callee_closure = callee.as_closure();
    let callee_fn = callee_closure.function.as_func();
    let new_base = base + recv_reg as usize + 1;
    // `1 + num_args`: the receiver the compiler already duplicated
    // into `recv_reg + 1` occupies the callee's own register 0 ("self"),
    // ahead of the `num_args` user arguments -- see `Instr::Invoke`'s
    // own doc comment in chunk.rs.
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
      // Same mixed-mode tiering as `dispatch_call`'s Closure arm -- see
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
      let chars_len = receiver.as_str().chars().count();
      let i = self.coerce_index(index, chars_len)?;
      let c = receiver.as_str().chars().nth(i).unwrap();
      Ok(self.heap.alloc_string(c.to_string()))
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
    // Early-exit out of the labeled 'step block below with an Err,
    // exactly like `?` would from inside an ordinary function. A bare
    // `?` here would target run_until's OWN return type directly and
    // skip the catch_stack check entirely -- every fallible call in
    // this loop goes through this instead. Label is passed explicitly
    // (rather than hardcoded as 'step inside the macro body) to avoid
    // any ambiguity from macro hygiene around labels.
    macro_rules! tri {
      ($e:expr, $label:lifetime) => {
        match $e {
          Ok(v) => v,
          Err(e) => break $label Err(e),
        }
      };
    }

    // Cached "which frame/function/closure am I currently executing"
    // state. Previously this was re-derived from self.frames on EVERY
    // instruction (two bounds-checked Vec accesses: one read to
    // destructure it, one write to bump ip) even though the vast
    // majority of instructions never change which frame is active.
    // Now it's refreshed only at the specific points that actually
    // change it: Call/Invoke/InvokeSuper/CallSuperCtor push a frame,
    // Return pops one, a caught exception truncates several.
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
        // `func_ptr` never needs this: `ObjFunction` always allocates
        // directly into old-generation storage (see
        // `Heap::alloc_function`'s own docs) specifically so it, like
        // this cached pointer to it, never moves. `closure_ptr` has no
        // such guarantee -- an `ObjClosure` is an ordinary young
        // allocation, and unlike `func_ptr`/`base` (a plain index,
        // unaffected by anything a collection relocates),
        // `run_until`'s own local cache of it is exactly the same
        // hazard `VM::ensure_stable_for_compiled_entry` exists to
        // prevent for JIT-compiled code's `closure_param`: a raw
        // pointer held OUTSIDE any GC-scannable location for longer
        // than one instruction. Unlike compiled code, though, the
        // interpreter re-derives this on EVERY safepoint instead of
        // needing the object pinned non-young for a whole invocation
        // -- cheap, and `collect_minor` has already relocated it (via
        // the per-frame loop that keeps `self.frames[..].closure` in
        // sync) by the time this reads it back out.
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
      // Synced back every instruction (not just at frame-change points)
      // because ANY instruction can end up calling self.raise(), which
      // reads every active frame's ip to build a stack trace.
      self.frames[frame_idx].ip = ip;

      // No more IIFE closure wrapping the whole match -- that closure
      // was too large for LLVM to ever inline across, so every
      // instruction paid a real function-call boundary on top of the
      // dispatch itself. This labeled block gives the same
      // "capture-and-inspect the Result before deciding to propagate"
      // behavior the closure gave, with none of the call overhead,
      // and direct mutable access to the frame-state locals above (no
      // capture needed since it's not a closure).
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
              self.binary_numeric(base, dst, a, b, "%", "@mod", |x, y| x % y, |x, y| &x % &y),
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
            // Mirrors binary_mult's string/lista-repeat cases -- only
            // "string/list * number" needs the repeat behavior (not
            // "number * string"), and a literal here can only ever
            // supply the right-hand number, so this covers it fully.
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
            // A backward jump is a loop back-edge -- exactly where a
            // baseline JIT is expected to offer on-stack replacement
            // (see `crate::jit`'s module docs). `maybe_osr` returns
            // `None` the overwhelming majority of the time (loop not
            // hot yet, function ineligible, or JIT disabled), in which
            // case this behaves exactly like the plain jump it always
            // was.
            if offset < 0 {
              // Captured BEFORE `maybe_osr` runs -- on an `Ok` outcome
              // it has ALREADY closed this frame's upvalues and popped
              // it (see `VM::invoke_compiled`), so `self.frames[frame_idx]`
              // itself is no longer valid to read afterward.
              let dst_in_caller = self.frames[frame_idx].dst_in_caller;
              if let Some(outcome) = self.maybe_osr(func, target) {
                match outcome {
                  // Mirrors Instr::Return's own handler exactly --
                  // on-stack replacement just ran the CURRENT frame to
                  // completion, so from here on this is a return, not
                  // a jump. `invoke_compiled` already closed upvalues
                  // and popped the frame; this just refreshes the
                  // dispatch loop's own cached state to the caller's.
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
                  // Feed into the SAME exception machinery any other
                  // failing instruction uses -- compiled code never
                  // pops its own frame on error (see
                  // `VM::invoke_compiled`), so `catch_stack`/
                  // `handle_exception` see this exactly as if an
                  // ordinary interpreted instruction had failed.
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
            // dispatch_call may or may not have pushed a new frame
            // (Closure does, Native/Class/BoundMethod resolve
            // synchronously and don't) -- always refresh, cheap, and
            // only paid on an actual call instruction.
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
            let finished = self.frames.pop().unwrap();
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
            println!("{}", v);
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

            let mut captured = Vec::with_capacity(proto.upvalues.len());
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
            // Collect straight into `ListStorage`, not a `Vec` -- for
            // `count` within the inline capacity (the common case:
            // small literal arrays), this is the whole point of
            // switching `Obj::List`'s storage to `SmallVec` at all.
            // Collecting into a `Vec` first and converting after would
            // still pay for a heap allocation on every list literal.
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
            });
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
              // `dict.key` is sugar for `dict['key']` -- same lookup,
              // same "missing key" error as Instr::GetIndex's own dict
              // arm (see `VM::index_get`), just reached through field
              // syntax instead of a bracketed index.
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
              // `dict.key = value` is sugar for `dict['key'] = value` --
              // insert-or-update, same as Instr::SetIndex's own dict arm
              // (see `VM::index_set`), never an error for a missing key.
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
                    let msg = format!("undefined member '{}' on module", method_name_val.as_str());
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
            // frame; the native/field-fallback paths never do. Always
            // refresh -- only paid on Invoke itself, never on the
            // arithmetic/move instructions that dominate a hot loop.
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
            if !self.is_exception_value(value) {
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
        match self.handle_exception(exc, stop_depth) {
          ExceptionOutcome::Handled {
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
          ExceptionOutcome::Propagate(e) => return Err(e),
        }
      }
    }
  }

  /// Find-or-create an OPEN upvalue for the given absolute register
  /// index. Reusing an existing one (rather than always allocating a new
  /// one) is what makes two closures created from the same enclosing
  /// scope, over the same local, actually share state.
  #[inline]
  pub(crate) fn capture_upvalue(&mut self, abs_index: usize) -> Value {
    if let Some((_, v)) = self.open_upvalues.iter().find(|(idx, _)| *idx == abs_index) {
      return *v;
    }
    let v = self.heap.alloc_upvalue(UpvalueState::Open(abs_index));
    self.open_upvalues.push((abs_index, v));
    v
  }

  /// Close every open upvalue pointing at a register >= `from_abs_index`,
  /// copying the register's current value into the upvalue's own
  /// storage. Called on block exit and on Return.
  pub(crate) fn close_upvalues_from(&mut self, from_abs_index: usize) {
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

  /// Like `get_reg`/`set_reg`, but for an ABSOLUTE register index
  /// rather than one relative to some frame's `base` -- needed only for
  /// open-upvalue access (`UpvalueState::Open` already stores an
  /// absolute index; see `object::UpvalueState`), where the index can
  /// belong to a DIFFERENT, outer frame than the one currently reading/
  /// writing through the upvalue.
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

    // NOTE: `||` here (not `&&`) matches this project's existing Add
    // behavior exactly, including its pre-existing edge case --
    // `.as_list()`/`.as_bytes()` below will panic if only ONE side is
    // actually a list/bytes (e.g. `[1,2] + 5`). That's a latent bug in
    // the ORIGINAL Add path, not introduced here -- preserved as-is
    // deliberately, so a literal RHS behaves identically to a variable
    // RHS holding the same value instead of silently diverging based
    // on whether fusion happened to apply. Worth fixing separately,
    // not folded into this change.
    if va.is_string() || vb.is_string() {
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
        va.as_list().to_vec().repeat(count)
      } else {
        Vec::new()
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

  /// Full mark-and-sweep collection. Roots are: every register within
  /// reach of a currently active frame, every global, the closure each
  /// active call frame is executing, and any upvalue still open. From
  /// there, every `Value` those objects transitively hold is walked
  /// with an explicit work-list (not recursion, so a long chain can't
  /// blow the stack) before anything unreached gets swept.
  ///
  /// Called automatically from `run_until` once the heap has grown past
  /// its threshold; also exposed to native code (see the `gc` native)
  /// for forcing a collection on demand. See `collect_minor` for the
  /// cheaper, far-more-frequent counterpart this collector normally
  /// relies on instead.
  pub(crate) fn collect_garbage(&mut self) {
    // A major collection's own mark-sweep below only ever visits
    // `Heap::chunks` -- flushing the nursery FIRST (promoting
    // everything in it that's still reachable, discarding the rest)
    // means every live object is uniformly chunk-resident by the
    // time that pass runs, so it needs no nursery-awareness of its
    // own at all. See `VM::collect_minor`'s own docs.
    self.collect_minor();

    #[cfg(feature = "gc-log")]
    let before_bytes = self.heap.bytes_allocated();
    #[cfg(feature = "gc-log")]
    let before_count = self.heap.object_count();

    let mut worklist: Vec<*const Obj> = Vec::new();

    // Only the register range actually within reach of a currently
    // active frame can hold live data -- registers past the innermost
    // active frame's window are left over from calls that have already
    // returned (the register stack is never shrunk, purely as a perf
    // tradeoff), so scanning them would just pin down garbage forever.
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
    for v in &self.pending_jit_compiles {
      Self::mark_root(*v, &mut worklist);
    }
    for v in self.modules.values() {
      Self::mark_root(*v, &mut worklist);
    }
    for v in self.builtin_exceptions.values() {
      Self::mark_root(*v, &mut worklist);
    }
    Self::mark_root(self.jit_pending_exception.get(), &mut worklist);

    while let Some(ptr) = worklist.pop() {
      // SAFETY: every pointer on the worklist was pulled out of a Value
      // that was itself still live when we queued it, and nothing is
      // freed until `sweep` runs below -- well after this loop -- so the
      // object behind `ptr` is guaranteed to still be valid here.
      Self::walk_children(ptr, |v| Self::mark_root(v, &mut worklist));
    }

    // A full scan just proved everything currently reachable, old
    // objects included -- every remembered-set entry is now
    // redundant. Drop them (clearing their `remembered` flags) so a
    // future write to any of them properly re-queues it; see
    // `Heap::drain_remembered`'s own docs.
    self.heap.drain_remembered();

    #[cfg(feature = "gc-log")]
    {
      let freed = self.heap.sweep();
      if std::env::var_os("ZURI_GC_LOG").is_some() {
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
    #[cfg(not(feature = "gc-log"))]
    self.heap.sweep();
  }

  /// Minor collection -- the cheap, frequent counterpart to
  /// `collect_garbage` that this collector normally relies on, and
  /// the young generation's whole reason to exist: a REAL, moving,
  /// copying collection rather than mark-sweep. Scans the SAME roots
  /// `collect_garbage` does, but instead of just marking reachable
  /// objects, actively RELOCATES every still-`Young` one it finds
  /// (via `Heap::forward_or_promote`) into old-generation storage,
  /// rewriting every reference to it -- root or child slot -- to
  /// point at the new copy. An `Old` object reached from a root is
  /// left completely alone (`forward_or_promote` returns it
  /// unchanged), since a copying collection never needs to prove an
  /// old object's liveness at all -- unlike mark-sweep, nothing about
  /// this pass depends on visiting every live object, only on
  /// visiting every POINTER TO A YOUNG one.
  ///
  /// The one thing that reasoning alone can't see: an old object that
  /// was MUTATED since its last full scan could now point at a young
  /// object that's otherwise unreachable from any of today's roots.
  /// That's exactly what `write_barrier` and the remembered set exist
  /// to cover -- every remembered old object's direct children are
  /// walked (and relocated in place) here too. See `object.rs`'s
  /// module docs on `write_barrier` for why this is sound: the
  /// barrier fires on EVERY mutation of an old container, so an
  /// old->young edge can only exist via an object currently in the
  /// remembered set.
  ///
  /// Once every root and every live object's children have been
  /// walked, EVERYTHING still in the nursery is, by construction,
  /// unreachable -- `Heap::reset_nursery` reclaims it in one step,
  /// with no per-object free-list bookkeeping needed at all (compare
  /// `collect_garbage`'s `sweep`, which visits every chunk slot).
  pub(crate) fn collect_minor(&mut self) {
    #[cfg(feature = "gc-log")]
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
        // `function`/`closure` are raw pointers CACHED from
        // `closure_val` at frame-push time purely for hot-path speed
        // (see `CallFrame`'s own docs) -- relocating the object
        // `closure_val` points at invalidates them just as much as
        // `closure_val` itself, so they need the exact same
        // re-derivation a fresh frame push would do, every time this
        // branch fires.
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
    for v in self.builtin_exceptions.values_mut() {
      Self::forward_slot(&mut self.heap, v, &mut worklist);
    }
    {
      let mut pending = self.jit_pending_exception.get();
      if Self::forward_slot(&mut self.heap, &mut pending, &mut worklist) {
        self.jit_pending_exception.set(pending);
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
        // SAFETY: `remembered_ptr` is a live old object (see
        // `drain_remembered`'s own docs); `walk_children_mut` only
        // ever yields pointers to genuine `Value` slots owned by it.
        Self::forward_slot(&mut self.heap, unsafe { &mut *slot }, &mut worklist)
      });
    }

    while let Some(ptr) = worklist.pop() {
      Self::walk_children_mut(ptr, |slot| {
        Self::forward_slot(&mut self.heap, unsafe { &mut *slot }, &mut worklist)
      });
    }

    #[cfg(feature = "gc-log")]
    {
      let before_bytes = self.heap.bytes_allocated();
      self.heap.reset_nursery();
      if std::env::var_os("ZURI_GC_LOG").is_some() {
        eprintln!(
          "[gc-minor] promoted/freed across {} -> {} objects, {} -> {} bytes",
          before_count,
          self.heap.object_count(),
          before_bytes,
          self.heap.bytes_allocated(),
        );
      }
    }
    #[cfg(not(feature = "gc-log"))]
    self.heap.reset_nursery();
  }

  /// Resolves ONE slot that might currently hold a pointer to a young
  /// object, relocating it (see `Heap::forward_or_promote`) and
  /// rewriting `*slot` in place if so. Returns whether `slot` was
  /// actually rewritten -- `walk_children_mut`'s `Obj::Dict` case
  /// needs to know this, to decide whether a key's hash may have
  /// changed and its index needs rebuilding (see
  /// `DictStorage::reindex`'s own docs). Free-standing (takes `heap`
  /// explicitly rather than `&mut self`) so callers can borrow it
  /// alongside other, disjoint fields of `self` -- see every root
  /// loop in `collect_minor` for the pattern this enables.
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

  /// Enumerates every `Value` held directly by the object behind
  /// `ptr` -- its immediate children in the object graph -- invoking
  /// `mark` for each. Shared between `collect_garbage` (which marks
  /// unconditionally) and `collect_minor` (which only marks, and
  /// therefore only transitively walks into, objects that are still
  /// `Young`), so this per-`Obj`-variant traversal exists in exactly
  /// one place instead of two copies that could drift apart.
  fn walk_children(ptr: *const Obj, mut mark: impl FnMut(Value)) {
    // SAFETY: see the two call sites' own safety comments -- both only
    // ever call this with a pointer that's still guaranteed live.
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

  /// `walk_children`'s mutable counterpart, used only by
  /// `collect_minor`'s copying pass: instead of handing each child
  /// `Value` to `mark` BY COPY (read-only), hands `relocate` a raw
  /// `*mut Value` pointing at the ACTUAL slot the child lives in --
  /// letting the caller rewrite it in place if it turns out to point
  /// at a young object that just got promoted. Sound via a single
  /// `&mut Obj` cast at the top: collection is always fully
  /// stop-the-world (no interpreter or JIT code runs concurrently
  /// with it), so nothing else can be aliasing `ptr` while this runs,
  /// regardless of whether the object's OWN fields are `Cell`-wrapped
  /// or not.
  ///
  /// Kept as a genuinely separate function from `walk_children`
  /// (rather than one traversal parameterized over both callback
  /// shapes) because about a third of its variants need real
  /// mutable-borrow machinery (`RefCell::get_mut`, `Cell::get_mut`,
  /// `Vec::iter_mut`) that a read-only `mark: impl FnMut(Value)`
  /// has no reason to carry -- see each variant for specifics.
  ///
  /// `Obj::Dict` is the one case that needs MORE than just rewriting
  /// each slot: `DictKey`'s hash is the raw pointer for every
  /// reference-type key (see its own docs), so relocating a KEY
  /// changes its hash out from under `DictStorage::index` -- tracked
  /// here via `relocate`'s own return value and repaired with one
  /// `reindex()` call, only when a key actually moved.
  fn walk_children_mut(ptr: *const Obj, mut relocate: impl FnMut(*mut Value) -> bool) {
    // SAFETY: see this function's own docs -- collection is always
    // stop-the-world, so exclusive access to every reachable object
    // is sound for its whole duration.
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

  fn coerce_index(&mut self, index: Value, len: usize) -> RunResult<usize> {
    let mut i = self.value_as_index(index)?;

    // First attempt to coerce it into the range [0, len) by wrapping negative indices around to the end of the array.
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

  /// Attempt to service an operator via a class- or builtin-table-declared
  /// override method named `deco` (e.g. "@add") on the LEFT operand only --
  /// matching the same "receiver defines the behavior" model every other
  /// method call in this VM already uses (no reflected/right-hand fallback).
  /// `extra_args` is everything after the implicit receiver -- one Value
  /// for a binary op, empty for a unary op.
  ///
  /// Returns `Ok(None)` if `receiver` has no such override at all (caller
  /// falls through to its own type-mismatch error), or the override's
  /// result / propagated exception once it's actually been invoked.
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

    if let Some(native) = builtins::lookup(receiver, deco) {
      let mut args = CallArgs::new();
      args.push(receiver);
      args.extend_from_slice(extra_args);
      return self.call_native(native, args.as_slice()).map(Some);
    }

    Ok(None)
  }

  /// Everything that happens when an instruction propagates an
  /// exception -- factored out of `run_until`'s dispatch loop and
  /// marked `#[cold]`/`#[inline(never)]` purely for CODE LAYOUT: this
  /// makes the exception path an out-of-line function call instead of
  /// inline code sharing icache lines with the hot dispatch loop, and
  /// lets LLVM lay out the loop's straight-line path biased toward
  /// the (overwhelmingly common) success case. This is NOT fixing a
  /// slow per-instruction check -- `if let Err(exc) = step` itself is
  /// a single, essentially-always-not-taken branch a modern predictor
  /// handles for free -- it's purely about keeping the rarely-taken
  /// handling code out of the hot loop's instruction-cache footprint.
  #[cold]
  #[inline(never)]
  fn handle_exception(&mut self, exc: Value, stop_depth: usize) -> ExceptionOutcome {
    let claims_it = matches!(self.catch_stack.last(), Some(h) if h.frame_depth > stop_depth);
    if !claims_it {
      return ExceptionOutcome::Propagate(exc);
    }

    let handler = self.catch_stack.pop().unwrap();
    if let Some(discard_base) = self.frames.get(handler.frame_depth).map(|f| f.base) {
      self.close_upvalues_from(discard_base);
    }
    self.frames.truncate(handler.frame_depth);
    let top = self.frames.last_mut().expect("catch handler left no frame");
    top.ip = handler.resume_ip;
    let top_base = top.base;
    if let Some(reg) = handler.var_reg {
      self.set_reg(top_base, reg, exc);
    }

    let frame_idx = self.frames.len() - 1;
    let f = &self.frames[frame_idx];
    ExceptionOutcome::Handled {
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
  }
}

/// Walk `class_val`'s superclass chain looking for a static member
/// named `name`, checking each class's own (never inherited-in)
/// `static_slots` table -- see `ObjClass`'s doc comment for why statics
/// aren't pre-merged the way methods/fields are.
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
/// space `Instr::UsingJump`'s jump table was built in at compile time
/// (see `expr_as_jump_key` in compiler.rs). `None` for anything that
/// was never eligible to be a constant case label to begin with (list,
/// dict, instance, range, etc.) -- always falls through to the
/// sequential dynamic-label path rather than ever consulting the table.
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
