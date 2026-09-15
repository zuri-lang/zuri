use num_bigint::BigInt;
use rustc_hash::FxHashMap;
use smallvec::SmallVec;
use std::any::Any;
use std::borrow::Cow;
use std::cell::{Cell, RefCell};
use std::fs::File;
use std::hash::{Hash, Hasher};
use std::rc::Rc;

use crate::vm::chunk::Chunk;
use crate::vm::value::Value;
use crate::vm::vm::VM;

/// Number of object slots per chunk. Chosen so a chunk is a few hundred
/// KB; large enough to amortize the cost of growing, small enough that
/// early GC cycles don't need to wait for one huge chunk to fill before
/// the free list has anything to give back.
const CHUNK_SIZE: usize = 8192;

/// Backing storage for `Obj::List`; re-exported from `vm::list`,
/// which explains why it is a hand-rolled `#[repr(C)]` vector rather
/// than `Vec` or a `SmallVec`.
pub use crate::vm::list::ListStorage;

/// Everything a Value's pointer tag can point at.
///
/// `#[repr(C, u8)]`, with an explicit discriminant on every variant;
/// NOT the default (niche-optimized, otherwise-unspecified) Rust enum
/// layout every OTHER enum in this codebase gets to use for free. This
/// costs real precision: the compiler is no longer free to pick
/// whatever layout it likes, and every variant's payload now lives at
/// the SAME fixed byte offset (a C-style tagged union, tag first, then
/// one shared payload region sized/aligned to the largest variant):
/// see `OBJ_TAG_*`/`OBJ_PAYLOAD_OFFSET` in this module, which exist
/// ONLY because this repr makes them sound to write down as fixed
/// numbers. The payoff is that `jit::codegen` can (in a LATER,
/// separate piece of work: see that module's own docs on why this
/// alone isn't sufficient yet) check a Value's actual `Obj` kind and
/// reach into specific variants' payloads directly from generated
/// machine code, with no helper-function call, the same way it already
/// does for `VM::global_slots`.
///
/// Costs nothing extra in practice: a C-tagged-union layout is sized
/// the same way the default Rust layout already would be (tag plus a
/// payload region sized to the largest variant), so this repr doesn't
/// grow every heap object the way it could for an enum with more size
/// variance across variants. Every large-relative-to-the-rest variant
/// is boxed (see `Func`/`Class`/`Module`'s own doc comments) precisely
/// to keep that shared payload region small, since it sets every OTHER
/// variant's size too; re-verify with `size_of::<Obj>()` if a new
/// variant is ever added that's meaningfully larger than the rest.
#[repr(C, u8)]
pub enum Obj {
  /// A string, plus a lazily-computed note on whether it is pure
  /// ASCII: `ASCII_UNKNOWN` until something asks, then `ASCII_YES` or
  /// `ASCII_NO`.
  ///
  /// Indexing a Zuri string is by codepoint, but the bytes are UTF-8,
  /// so byte offset `i` only holds codepoint `i` when every byte before
  /// it is single-byte. Compiled code needs that fact in constant time
  /// to index a string without a scan, and a scan is exactly what it is
  /// trying to avoid; hence caching it here rather than recomputing.
  ///
  /// Deliberately lazy rather than filled in at allocation: building a
  /// string by repeated concatenation allocates a fresh one each time,
  /// and scanning every intermediate result would turn that loop
  /// quadratic. Only code that actually indexes a string pays for it.
  ///
  /// The flag cannot go stale. `Obj::Str` holds no `RefCell`, so safe
  /// code can never mutate one in place, and the single unsafe path
  /// that does (`jit::codegen`'s in-place append) only ever appends a
  /// one-BYTE string; a one-byte string is necessarily ASCII, since
  /// every non-ASCII codepoint is two bytes or more in UTF-8, so an
  /// ASCII string stays ASCII and a non-ASCII one stays non-ASCII.
  Str(String, Cell<u8>) = 0,
  Bytes(RefCell<Vec<u8>>) = 1,
  BigInt(BigInt) = 2,
  /// A dynamically-sized list.
  List(RefCell<ListStorage>) = 3,
  /// A dict literal's storage. Boxed for the same reason `Module` is
  /// (see that variant's own doc comment): `DictStorage` (an
  /// insertion-order `Vec` plus an `FxHashMap` index) is large relative
  /// to the common case, and no current benchmark or hot path indexes
  /// through a `Dict` anywhere near as often as it does a `List` or
  /// `Instance`; unlike `List`, there's no small-N inline case worth
  /// preserving here.
  Dict(Box<RefCell<DictStorage>>) = 4,
  /// A function PROTOTYPE; the static, compiled-once result of one
  /// `function` declaration or literal. Shared by every closure ever
  /// created from it; holds no per-call-site state itself.
  Func(Box<ObjFunction>) = 5,
  BoundMethod(ObjBoundMethod) = 6,
  /// A function VALUE at runtime; a prototype plus the specific
  /// upvalues captured at the moment this particular closure was
  /// created. Every callable Value is one of these, even a top-level
  /// function that captures nothing (its `upvalues` is just empty).
  Closure(ObjClosure) = 7,
  /// A captured variable. Starts Open, pointing at a live register in
  /// some still-executing frame; reads/writes through the upvalue and
  /// through the original local are the same memory. Closed when that
  /// frame's register would otherwise become invalid (block exit or
  /// function return): the current value is copied out, and the upvalue
  /// owns it from then on.
  Upvalue(Cell<UpvalueState>) = 8,
  /// A native (Rust-implemented) function; callable through the exact
  /// same Instr::Call path as a Closure, but with no CallFrame, no
  /// register-window setup, and no heap allocation on the call path:
  /// its arguments are a zero-copy slice straight into the caller's own
  /// registers. See vm/natives.rs.
  Native(NativeFunction) = 9,
  /// See `ObjClass`'s own doc comment. Wrapped in a `RefCell` (unlike
  /// every other heap object here, which is immutable-after-creation
  /// except through a `Cell`-wrapped field) because a class's tables
  /// keep growing throughout its own declaration; `RefCell` is the
  /// safe way to do that through the same shared `*const Obj` pointer
  /// every other Value already uses, rather than reaching for unsafe
  /// mutation.
  Class(Box<RefCell<ObjClass>>) = 10,
  Instance(ObjInstance) = 11,
  /// A `lower..upper` range value; valid in either direction (see
  /// Expr::Range's compilation in compiler.rs). Stored as raw f64s, not
  /// Values, specifically so this is a GC leaf: nothing here is ever a
  /// heap pointer, so the mark phase's worklist never needs to descend
  /// into one (see VM::collect_garbage's Obj::Range arm).
  Range {
    lower: f64,
    upper: f64,
    /// Iteration step, defaulting to `1.0`, set via `.step(size)`.
    /// Only consulted by the `@key`/`@value` iterable protocol (see
    /// `builtins::range`); `Instr::MakeRange` always allocates with
    /// the default.
    step: Cell<f64>,
  } = 12,
  /// A `file(...)` object: see `FileHandle`. Boxed: a file handle is
  /// created once per `file(...)` call, not once per value the program
  /// manipulates; same rarity argument as `Module`.
  File(Box<RefCell<FileHandle>>) = 13,
  /// See `ObjModule`'s own doc comment. Boxed like `Func`/`Class` --
  /// `ObjModule` (two `String`s plus a `ModuleNamespace`, itself a
  /// `Vec` and an `FxHashMap`) is the largest variant in this enum,
  /// which sets `Obj`'s size for EVERY variant, including tiny,
  /// extremely common ones like `List`. A module is created once per
  /// imported file, not once per value the program manipulates --
  /// paying one extra pointer indirection on that rare path to shrink
  /// every common allocation is a clear win.
  Module(Box<RefCell<ObjModule>>) = 14,
  /// See `ObjModuleBinding`'s own doc comment. Boxed: created once per
  /// `import` site, not once per value the program manipulates; same
  /// rarity argument as `Module`.
  ModuleBinding(Box<ObjModuleBinding>) = 15,
  /// See `ObjPtr`'s own doc comment.
  Ptr(RefCell<ObjPtr>) = 16,
}

/// Fixed numeric tags matching `Obj`'s own explicit discriminants one
/// for one; kept as named constants (rather than requiring every
/// caller to write the literal number) so `jit::codegen`'s later
/// dereference-and-compare fast paths read as "is this an Instance"
/// instead of "does this byte equal 11". A `#[test]` below cross-checks
/// every one of these against the enum definition itself, so a
/// constant silently drifting out of sync with an edited discriminant
/// is a hard test failure, not a subtle miscompile.
pub const OBJ_TAG_STR: u8 = 0;
pub const OBJ_TAG_BYTES: u8 = 1;
pub const OBJ_TAG_BIGINT: u8 = 2;
pub const OBJ_TAG_LIST: u8 = 3;
pub const OBJ_TAG_DICT: u8 = 4;
pub const OBJ_TAG_FUNC: u8 = 5;
pub const OBJ_TAG_BOUND_METHOD: u8 = 6;
pub const OBJ_TAG_CLOSURE: u8 = 7;
pub const OBJ_TAG_UPVALUE: u8 = 8;
pub const OBJ_TAG_NATIVE: u8 = 9;
pub const OBJ_TAG_CLASS: u8 = 10;
pub const OBJ_TAG_INSTANCE: u8 = 11;
pub const OBJ_TAG_RANGE: u8 = 12;
pub const OBJ_TAG_FILE: u8 = 13;
pub const OBJ_TAG_MODULE: u8 = 14;
pub const OBJ_TAG_MODULE_BINDING: u8 = 15;
pub const OBJ_TAG_PTR: u8 = 16;

impl Obj {
  /// Reads the tag byte directly rather than matching; exists so a
  /// test can cross-check it against `OBJ_TAG_*` (and so any FUTURE
  /// Rust-side code that wants the numeric tag doesn't need to
  /// duplicate a 17-arm match). NOT what `jit::codegen`'s eventual fast
  /// path will use; that reads the SAME byte directly out of raw
  /// memory at a compile-time-baked offset, with no function call at
  /// all; this exists purely for verification from safe Rust.
  #[inline]
  pub fn tag(&self) -> u8 {
    // SAFETY: `#[repr(C, u8)]` guarantees the discriminant is stored
    // as a `u8` at the very start of the type.
    unsafe { *(self as *const Obj as *const u8) }
  }
}

/// Byte offset from an `Obj`'s own address to where its payload
/// actually starts; the same for EVERY variant, since `#[repr(C,
/// u8)]` lays every variant's payload at one shared, tag-sized-and-
/// aligned offset (a C-style tagged union), never a per-variant one.
///
/// Deliberately NOT a hand-derived constant (e.g. "1 byte tag, rounded
/// up to 8-byte alignment"). `std::mem::offset_of!` has no way to name
/// a field inside an enum variant's payload, so instead of asserting
/// what the layout SHOULD be, this measures what it ACTUALLY is: builds
/// one real `Obj::Instance`, matches it back apart to get a genuine
/// `&ObjInstance` pointer, and takes the byte difference from the
/// enclosing `Obj`'s own address; the same technique
/// `obj_repr_tests`/`field_storage_tests` already use to verify claims
/// about this repr instead of trusting them. Computed once and cached,
/// since it never changes at runtime (the layout is fixed at compile
/// time; only the observation of it happens lazily here); every JIT
/// compile that needs it (see `jit::codegen`'s field-access fast path)
/// reads the same cached value.
pub fn obj_payload_offset() -> usize {
  static OFFSET: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
  *OFFSET.get_or_init(|| {
    let probe = Obj::Instance(ObjInstance {
      class: Value::nil(),
      fields: FieldStorage::new(0),
    });
    let obj_addr = &probe as *const Obj as usize;
    let payload_addr = match &probe {
      Obj::Instance(inst) => inst as *const ObjInstance as usize,
      _ => unreachable!(),
    };
    payload_addr - obj_addr
  })
}

/// Byte offset from a `*const Obj` known (via `obj_tag`) to be
/// `Obj::Instance` to that instance's `fields` slice base pointer;
/// `obj_payload_offset()` (tag -> `ObjInstance` start) plus
/// `ObjInstance`/`FieldStorage`'s own `#[repr(C)]`-guaranteed field
/// offsets, unlike `obj_payload_offset()` itself these don't need a
/// runtime probe: `ObjInstance`/`FieldStorage` are plain `#[repr(C)]`
/// structs (not a tagged union), so `offset_of!` on them is already a
/// real compile-time constant. Consumed by `jit::codegen`'s
/// self-field-access fast path to reach a proven-safe field directly,
/// with no `zuri_jit_get_field`/`zuri_jit_set_field` call at all.
pub fn obj_instance_fields_ptr_offset() -> usize {
  obj_payload_offset()
    + std::mem::offset_of!(ObjInstance, fields)
    + std::mem::offset_of!(FieldStorage, ptr)
}

/// Byte offset from a `*const Obj` known (via `obj_tag`) to be
/// `Obj::Instance` to that instance's own `class` field; same
/// reasoning as `obj_instance_fields_ptr_offset`, just one field over.
/// Consumed by `jit::codegen`'s self-invoke fast path
/// (`emit_self_invoke`) to read a receiver's class directly for its
/// own guard, with no `zuri_jit_invoke_prepare` call at all on the
/// guard check itself.
pub fn obj_instance_fields_offset() -> usize {
  obj_payload_offset()
    + std::mem::offset_of!(ObjInstance, fields)
    + std::mem::offset_of!(FieldStorage, ptr)
}

pub fn obj_instance_fields_inline_offset() -> usize {
  obj_payload_offset()
    + std::mem::offset_of!(ObjInstance, fields)
    + std::mem::offset_of!(FieldStorage, inline)
}

pub fn obj_instance_class_offset() -> usize {
  obj_payload_offset() + std::mem::offset_of!(ObjInstance, class)
}

#[cfg(test)]
mod obj_repr_tests {
  use super::*;

  /// Confirms `Cell<Option<EntryFn>>` (a function pointer, which can
  /// never validly be null) actually gets Rust's documented null-
  /// pointer niche optimization; i.e. is exactly pointer-sized, no
  /// separate discriminant byte; which is the entire soundness
  /// argument behind `obj_function_jit_entry_offset()` reading it as a
  /// plain `u64` from generated code. A future Rust version changing
  /// this (extremely unlikely, since it's a documented guarantee, not
  /// an incidental optimization) would fail this loudly instead of
  /// silently misreading the tag as part of the pointer.
  #[test]
  fn entry_fn_option_is_pointer_sized() {
    assert_eq!(
      std::mem::size_of::<Cell<Option<crate::jit::EntryFn>>>(),
      std::mem::size_of::<usize>()
    );
    let empty: Cell<Option<crate::jit::EntryFn>> = Cell::new(None);
    let empty_bits = unsafe { *(&empty as *const _ as *const u64) };
    assert_eq!(empty_bits, 0);
  }

  /// Cross-checks every `OBJ_TAG_*` constant against the enum's own
  /// explicit discriminants, via `Obj::tag()` on one real instance of
  /// each variant; catches the failure mode this whole scheme exists
  /// to avoid: a constant silently drifting out of sync after someone
  /// edits `= N` on a variant without updating the matching constant.
  #[test]
  fn tags_match_discriminants() {
    assert_eq!(
      Obj::Str(String::new(), Cell::new(ASCII_UNKNOWN)).tag(),
      OBJ_TAG_STR
    );
    assert_eq!(Obj::Bytes(RefCell::new(Vec::new())).tag(), OBJ_TAG_BYTES);
    assert_eq!(Obj::BigInt(BigInt::from(0)).tag(), OBJ_TAG_BIGINT);
    assert_eq!(
      Obj::List(RefCell::new(ListStorage::new())).tag(),
      OBJ_TAG_LIST
    );
    assert_eq!(
      Obj::Dict(Box::new(RefCell::new(DictStorage::new()))).tag(),
      OBJ_TAG_DICT
    );
    assert_eq!(
      Obj::Range {
        lower: 0.0,
        upper: 0.0,
        step: Cell::new(1.0),
      }
      .tag(),
      OBJ_TAG_RANGE
    );
  }

  /// Confirms the memory-cost claim in `Obj`'s own doc comment stays
  /// true; fails loudly (rather than silently regressing every heap
  /// object's size) if a future variant grows past what
  /// `RefCell<ListStorage>` (the largest currently-unboxed variant)
  /// costs today.
  #[test]
  fn size_unchanged_from_baseline() {
    assert_eq!(std::mem::size_of::<Obj>(), 48);
    assert_eq!(std::mem::size_of::<GcBox>(), 64);
  }

  /// `obj_payload_offset()` measures where `Obj::Instance`'s payload
  /// starts; this checks a SECOND, independently-constructed
  /// `Obj::Instance` lands its `class`/`fields` at exactly
  /// `obj_payload_offset() + offset_of!(ObjInstance, ...)` from the
  /// ENCLOSING `Obj`'s own address; i.e. that the measurement isn't
  /// somehow specific to the one probe value used to compute it.
  #[test]
  fn payload_offset_matches_real_instance_fields() {
    let offset = obj_payload_offset();
    let obj = Obj::Instance(ObjInstance {
      class: Value::number(123.0),
      fields: FieldStorage::new(2),
    });
    let obj_addr = &obj as *const Obj as usize;
    let class_offset = offset + std::mem::offset_of!(ObjInstance, class);
    let fields_offset = offset + std::mem::offset_of!(ObjInstance, fields);
    let class_ptr = (obj_addr + class_offset) as *const Value;
    let fields_ptr = (obj_addr + fields_offset) as *const FieldStorage;
    unsafe {
      assert_eq!((*class_ptr).as_number(), 123.0);
      assert_eq!((&*fields_ptr).len(), 2);
    }
  }

  /// Cross-checks `UPVALUE_STATE_TAG_*` against `UpvalueState`'s own
  /// explicit discriminants; same failure mode `tags_match_discriminants`
  /// guards against, one type over.
  #[test]
  fn upvalue_state_tags_match_discriminants() {
    assert_eq!(UpvalueState::Open(0).tag(), UPVALUE_STATE_TAG_OPEN);
    assert_eq!(
      UpvalueState::Closed(Value::nil()).tag(),
      UPVALUE_STATE_TAG_CLOSED
    );
  }

  /// Confirms `obj_upvalue_state_tag_offset()`/
  /// `obj_upvalue_state_payload_offset()` land where a real
  /// `Obj::Upvalue(Cell<UpvalueState>)` actually puts its tag and
  /// payload for BOTH variants; `jit::codegen`'s inline `GetUpval`/
  /// `SetUpval` fast path reads either through these same two offsets,
  /// branching only on the tag byte.
  #[test]
  fn upvalue_state_offsets_match_real_obj() {
    let closed = Obj::Upvalue(Cell::new(UpvalueState::Closed(Value::number(42.0))));
    let closed_addr = &closed as *const Obj as usize;
    let tag_ptr = (closed_addr + obj_upvalue_state_tag_offset()) as *const u8;
    let payload_ptr = (closed_addr + obj_upvalue_state_payload_offset()) as *const Value;
    unsafe {
      assert_eq!(*tag_ptr, UPVALUE_STATE_TAG_CLOSED);
      assert_eq!((*payload_ptr).as_number(), 42.0);
    }

    let open = Obj::Upvalue(Cell::new(UpvalueState::Open(7)));
    let open_addr = &open as *const Obj as usize;
    let tag_ptr = (open_addr + obj_upvalue_state_tag_offset()) as *const u8;
    let payload_ptr = (open_addr + obj_upvalue_state_payload_offset()) as *const usize;
    unsafe {
      assert_eq!(*tag_ptr, UPVALUE_STATE_TAG_OPEN);
      assert_eq!(*payload_ptr, 7);
    }
  }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum UpvalueDescriptor {
  /// Capture the immediately enclosing function's local register `n`.
  Local(u8),
  /// Capture the immediately enclosing function's OWN upvalue `n` --
  /// used when a function captures a variable from an outer scope that
  /// isn't its *direct* parent (the variable passes through, unchanged,
  /// via the intermediate function's own upvalue list).
  Upvalue(u8),
}

/// `#[repr(C, u8)]` for the same reason `Obj` itself is: a C-style
/// tagged union puts BOTH variants' payload at one shared, fixed byte
/// offset from this type's own address (each is one 8-byte word;
/// `usize` and `Value` are the same size), which is what lets
/// `jit::codegen`'s inline `GetUpval`/`SetUpval` fast path
/// (`emit_upvalue_obj_ptr` and friends) read or write EITHER an open
/// register index or a closed-over `Value` directly through a raw
/// `Cell<UpvalueState>` pointer, no helper call for either case.
#[derive(Clone, Copy)]
#[repr(C, u8)]
pub enum UpvalueState {
  /// Points at an absolute index into `VM::registers` (i.e. already
  /// includes some frame's `base`).
  Open(usize) = 0,
  Closed(Value) = 1,
}

pub const UPVALUE_STATE_TAG_OPEN: u8 = 0;
pub const UPVALUE_STATE_TAG_CLOSED: u8 = 1;

impl UpvalueState {
  /// Reads the tag byte directly: see `Obj::tag()`'s identical
  /// reasoning; exists for verification, not for `jit::codegen` to call.
  #[inline]
  pub fn tag(&self) -> u8 {
    // SAFETY: `#[repr(C, u8)]` guarantees the discriminant is stored
    // as a `u8` at the very start of the type.
    unsafe { *(self as *const UpvalueState as *const u8) }
  }
}

/// Byte offset from a `Cell<UpvalueState>`'s (or a bare `UpvalueState`'s
///; `Cell<T>` shares `T`'s layout) own address to where its payload
/// word lives; the SAME offset for `Open`'s `usize` and `Closed`'s
/// `Value`, since a C-style tagged union gives every variant one shared
/// payload region (see `UpvalueState`'s own docs). Measured the same
/// runtime-probe way `obj_payload_offset()` is, for the same reason:
/// `offset_of!` can't name a field inside an enum variant. Probed via
/// `Closed` specifically, but `upvalue_state_offsets_match_real_obj`
/// (below) confirms `Open` lands at the identical offset.
pub fn upvalue_state_payload_offset() -> usize {
  static OFFSET: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
  *OFFSET.get_or_init(|| {
    let probe = UpvalueState::Closed(Value::nil());
    let base = &probe as *const UpvalueState as usize;
    let payload = match &probe {
      UpvalueState::Closed(v) => v as *const Value as usize,
      UpvalueState::Open(_) => unreachable!(),
    };
    payload - base
  })
}

/// Byte offset from a `*const Obj` known (via `obj_tag`) to be
/// `Obj::Upvalue` to its `UpvalueState` tag byte; always
/// `obj_payload_offset()` itself, since the `Cell<UpvalueState>` payload
/// starts there and a `UpvalueState`'s own tag sits at ITS offset 0.
/// Named separately from `obj_payload_offset()` purely so call sites
/// read as what they mean.
pub fn obj_upvalue_state_tag_offset() -> usize {
  obj_payload_offset()
}

/// Byte offset from a `*const Obj` known to be `Obj::Upvalue` to its
/// payload word; an open register index OR a closed-over `Value`,
/// whichever the tag byte at `obj_upvalue_state_tag_offset()` says this
/// particular instance holds. Consulted by `jit::codegen`'s inline
/// `GetUpval`/`SetUpval` fast path only after that tag check.
pub fn obj_upvalue_state_payload_offset() -> usize {
  obj_payload_offset() + upvalue_state_payload_offset()
}

pub struct ObjFunction {
  pub name: String,
  pub variadic: bool,
  pub chunk: Chunk,

  /// Total declared parameter count, INCLUDING the variadic parameter
  /// itself if `variadic` is set.
  pub arity: u8,

  /// How many registers this function's window needs. The caller reserves
  /// this many registers starting at the call's `first_arg` register.
  pub num_registers: u8,

  /// Static, compile-time list of what this function's own upvalues
  /// need to capture from ITS enclosing function, in order. Every time
  /// a closure is created from this prototype (`Instr::Closure`), the
  /// VM walks this list once to build that instance's actual upvalues.
  pub upvalues: Vec<UpvalueDescriptor>,

  /// True for a method (instance OR static) compiled via
  /// `Compiler::compile_method_prototype`; register 0 of its frame is
  /// reserved for an implicit receiver even when unused (a static
  /// method's own body never reads it), which is what lets the fused
  /// Invoke/InvokeSuper instructions use one uniform calling convention
  /// regardless of what a receiver expression turns out to be at
  /// runtime. `GetField` consults this flag to decide whether a
  /// class-level (static) closure fetched as a bare value needs
  /// wrapping in an `ObjBoundMethod` with a dummy nil receiver, so that
  /// register still gets filled when it's later called as a plain
  /// value. Ordinary functions/closures/anonymous functions leave this
  /// false.
  pub is_method: bool,

  /// The name of the class this method was declared inside, if any --
  /// `None` for every ordinary function/closure/anonymous function,
  /// and for a method compiled outside a NAMED top-level class
  /// declaration (this project doesn't have those today, but nothing
  /// stops a future local/nested class from producing one). A NAME,
  /// not a `*const ObjClass`, because the class object itself doesn't
  /// exist yet at compile time; it's built at RUNTIME when the
  /// `class Foo { ... }` declaration statement actually executes (see
  /// `Instr::MakeClass`/`FinalizeClass`). Resolving this to the real,
  /// live `ObjClass` (to inspect its `field_slots`/`methods` for a
  /// name collision: see `jit::escape`'s GetField-safety docs) is
  /// the JIT compiler's own job, done once at JIT-compile time (main-
  /// thread, real VM access available) by looking up the CURRENT
  /// global bound to this name; sound because Zuri classes are
  /// immutable after construction (NOTES.md: "new fields and methods
  /// cannot be added at runtime"), so whatever `field_slots`/`methods`
  /// that lookup finds are permanent facts, not a one-shot snapshot
  /// that could go stale.
  pub owning_class_name: Option<String>,

  /// The file this function was compiled from, shared (via `Rc`, not
  /// cloned) by every function compiled in the same `Compiler` run --
  /// see `Compiler::new`. Read per-frame by `VM::build_stacktrace`,
  /// which is what lets a trace correctly attribute each frame to its
  /// OWN file.
  pub source_path: Rc<str>,

  /// Which module's global namespace this function's own
  /// `GetGlobal`/`SetGlobal`/`AssignGlobal` instructions resolve
  /// against. `None` means the VM's shared ROOT table; the main
  /// script and the REPL, exactly the pre-module-system behavior, so
  /// every existing non-module program is completely unaffected. Set
  /// once, uniformly, for every function a `Compiler` instance
  /// produces, via `Compiler::set_current_module`: see
  /// `vm::modules::run_module_source`.
  pub globals_module: Option<Value>,

  /// Tiering/JIT state for this prototype: see `crate::jit`. Lives
  /// here (not on `ObjClosure`) because every closure created from the
  /// same prototype shares the same compiled machine code; a closure
  /// only contributes its own upvalues on top.
  pub jit: JitInfo,
}

impl ObjFunction {
  /// `Class.method` for anything that belongs to a class, the plain
  /// name for everything else. Worth the formatting: a JIT log line
  /// naming `@new` could mean any constructor the program has loaded,
  /// and a program of any size has plenty.
  pub fn display_name(&self) -> Cow<'_, str> {
    match &self.owning_class_name {
      Some(class) => Cow::Owned(format!("{class}.{}", self.name)),
      None => Cow::Borrowed(&self.name),
    }
  }
}

/// Per-function tiering state consulted by both the interpreter (to
/// decide when to compile, and whether a call/loop should route into
/// already-compiled code) and the JIT compiler itself (see
/// `crate::jit::codegen`). Every field here uses interior mutability
/// because `ObjFunction` is reached only through a shared `*const
/// ObjFunction` / `&ObjFunction` for its whole life; exactly like
/// `Chunk::global_cache` already does for its own inline cache.
pub struct JitInfo {
  /// Real invocations of this prototype (via `Instr::Call`, `Invoke`,
  /// `call_value`, ...) since the VM started; NOT bytecode
  /// instructions executed. Compared against `call_threshold`.
  pub call_count: Cell<u32>,
  /// This function's own whole-function warmup threshold, precomputed
  /// once at construction time from its bytecode length: see
  /// `crate::jit::warmup::call_threshold`. Larger functions amortize
  /// the fixed cost of compilation over fewer calls, so they warm up
  /// sooner than tiny ones.
  pub call_threshold: u32,
  /// This function's threshold for a single loop's own back-edge count
  /// to trigger on-stack replacement: see
  /// `crate::jit::warmup::osr_threshold`. Deliberately smaller than
  /// `call_threshold` scaled the same way, so a function that's called
  /// exactly once (e.g. a `main`) but immediately enters a long loop
  /// still gets compiled without waiting for a second call that may
  /// never come.
  pub osr_threshold: u32,
  /// Set once a compilation attempt has run and PRODUCED code; the
  /// single, allocation-free "is this compiled, and if so what do I
  /// call" check consulted on EVERY `Instr::Call`/`Invoke`/... site,
  /// deliberately a bare `Cell` of a `Copy` function pointer rather
  /// than hidden behind a `RefCell<Option<Rc<...>>>`. A borrow-flag
  /// check plus a refcount bump on every single hot call turned out to
  /// cost real, measurable time once call-heavy code (recursive
  /// fibonacci, tree traversal, ...) got compiled; exactly the kind
  /// of code a JIT most needs to win on; so this field is the
  /// leanest thing that can answer "do I have compiled code, and where
  /// is its entry point" with nothing more than a single memory load.
  pub entry: Cell<Option<crate::jit::EntryFn>>,
  /// The bytecode-ip -> osr-id map for this function's own loop
  /// headers, populated at the same time `entry` is. Kept OFF the hot
  /// call path on purpose; unlike `entry`, this is only ever
  /// consulted from `VM::maybe_osr`, itself only reached from a
  /// backward `Instr::Jmp` (cold relative to a call site in
  /// call-dominated code), so the `RefCell`'s cost doesn't matter here
  /// the way it does for `entry`.
  pub osr_ids: RefCell<Option<FxHashMap<usize, i32>>>,
  /// Set once a compilation attempt has run and FAILED, or the
  /// function was found ineligible up front (contains `Raise`/
  /// `PushCatch`/`PopCatch`: see the `crate::jit` module docs for why
  /// error-handling bytecode is never compiled). Sticky: later warm
  /// call sites see this and stop trying, rather than re-attempting a
  /// doomed compilation on every single call.
  pub ineligible: Cell<bool>,
  /// Bytecode positions where this function's compiled code gave up and
  /// handed control back to the interpreter. A speculation that misses
  /// here missed on real input, so the next compilation skips it and
  /// leaves that site on its ordinary inline cache.
  ///
  /// Written only from `VM::resolve_deopt_slow` and read only when a
  /// compile job is built, both on the VM's thread, and those two can
  /// never overlap for one prototype: a deopt means compiled code is
  /// running, which means `entry` is set, which is exactly what stops a
  /// compile from being enqueued. `RefCell` costs nothing here; deopting
  /// is a cold path and the set is empty for almost every function.
  pub deopt_sites: RefCell<rustc_hash::FxHashSet<usize>>,
  /// How many times compiled code for this prototype has been thrown
  /// away after a deopt. Capped, so a function whose deopts come from
  /// something the next compilation can't avoid settles down instead of
  /// recompiling forever.
  pub invalidations: Cell<u32>,
  /// Set on the last invalidation a function is allowed: compile it
  /// once more with every field-class bet in it dropped, rather than
  /// leaving code that gives up on the same instruction for the rest of
  /// the program's life.
  ///
  /// Site-by-site feedback settles the ordinary cases in one or two
  /// rounds. Reaching the cap means it hasn't, so the remaining choice
  /// is between guessing again and not guessing at all, and code that
  /// runs to completion beats code that bails out every call.
  pub field_speculation_off: Cell<bool>,
  /// Per-loop-header back-edge hit counts, keyed by the bytecode `ip`
  /// the loop's `Instr::Jmp` back-edge targets; consulted only by
  /// that instruction's own handler in `vm.rs` to decide when a
  /// specific loop is hot enough to trigger (or use, if compilation
  /// already happened) on-stack replacement.
  pub osr_counts: RefCell<FxHashMap<usize, u32>>,
  /// True from the moment a compile job for this function is handed
  /// to the background compiler thread (see `VM::enqueue_or_ready`)
  /// until its result is drained (`VM::drain_jit_results`); prevents
  /// enqueueing a second, redundant compile for the same prototype
  /// while one is already in flight. Main-thread-only, like every
  /// other `JitInfo` field: the background thread never reads or
  /// writes `JitInfo` at all (see `jit::background`'s module docs).
  pub compiling: Cell<bool>,
  /// Argument-type feedback accumulated, via bitwise AND, across EVERY
  /// call recorded by `VM::record_call_feedback` since this function
  /// started warming up; bit `i` survives only if parameter `i` was
  /// observed numeric on every single call seen so far, exactly the
  /// polymorphic-inline-cache pattern of "keep believing the guess
  /// until a call actually contradicts it". Starts at `!0` (every bit
  /// "unfalsified") rather than `0`, so the first real sample fully
  /// determines the mask instead of an empty AND collapsing everything
  /// to non-numeric; `feedback_samples` is what distinguishes "no
  /// evidence yet" from "confirmed by evidence" for a caller that only
  /// has this field to look at. Read at compile-enqueue time instead of
  /// a one-shot single-call sample: see `VM::combined_param_feedback`.
  pub numeric_feedback: Cell<u64>,
  /// Argument-type feedback accumulated across calls for List parameters;
  /// bit `i` is set if parameter `i` was observed to be an `Obj::List`.
  pub list_feedback: Cell<u64>,
  /// Argument-type feedback accumulated across calls for whole-number (Int) parameters;
  /// bit `i` is set if parameter `i` was observed to be a whole number.
  pub int_feedback: Cell<u64>,
  /// Bit `i` is set if parameter `i` was a list whose elements all
  /// looked like whole numbers on every call sampled so far. A hint
  /// only, and deliberately a cheap one: `VM::sample_param_elem_types`
  /// looks at a bounded prefix of each list rather than walking a
  /// million elements on every warm-up call. What makes the bet SOUND
  /// is the full scan `codegen` emits once at entry, which sends a
  /// list that fails it to the general body.
  pub int_list_feedback: Cell<u64>,
  /// `int_list_feedback` widened to any number, fractions included;
  /// what a list of floats gets proven with. Sampled and guarded the
  /// same way.
  pub num_list_feedback: Cell<u64>,
  /// Number of calls that have contributed to `numeric_feedback` so
  /// far. Needed because `numeric_feedback` alone can't distinguish
  /// "every call observed had numeric args" from "no call has been
  /// observed yet"; both read as `!0`.
  pub feedback_samples: Cell<u32>,
  /// Per-`GetGlobal`-instruction inline cache for compiled code ONLY
  /// (the interpreter has its own, separate `Chunk::global_cache` --
  /// see that field's docs): `global_slot_cache[ip]` is the resolved
  /// slot for the `GetGlobal` at that bytecode position, or `-1` if
  /// never resolved (or resolved against a QUALIFIED module namespace
  /// rather than the root globals table: see
  /// `jit::runtime::zuri_jit_get_global`'s own docs on why only a root
  /// resolution ever gets cached here). Unlike `Chunk::global_cache`
  /// (a `RefCell<HashMap>`, fine for the interpreter's own Rust-side
  /// lookups but not something generated machine code can probe), this
  /// is a flat, fixed-size array living at a STABLE address for
  /// exactly as long as this `ObjFunction` does (old-generation,
  /// non-moving: see `Heap::alloc_old`'s own docs); safe to bake as
  /// a compile-time-constant base pointer and index straight into with
  /// two Cranelift `load`s and a compare, no helper call at all on a
  /// cache hit. Sized once, at construction, to this function's own
  /// bytecode length, since every valid `ip` is already known then.
  pub global_slot_cache: Box<[Cell<i64>]>,
}

impl JitInfo {
  pub fn new(code_len: usize) -> JitInfo {
    JitInfo {
      call_count: Cell::new(0),
      call_threshold: crate::jit::warmup::call_threshold(code_len),
      osr_threshold: crate::jit::warmup::osr_threshold(code_len),
      entry: Cell::new(None),
      osr_ids: RefCell::new(None),
      ineligible: Cell::new(false),
      deopt_sites: RefCell::new(rustc_hash::FxHashSet::default()),
      invalidations: Cell::new(0),
      field_speculation_off: Cell::new(false),
      osr_counts: RefCell::new(FxHashMap::default()),
      compiling: Cell::new(false),
      numeric_feedback: Cell::new(!0u64),
      list_feedback: Cell::new(!0u64),
      int_feedback: Cell::new(!0u64),
      int_list_feedback: Cell::new(!0u64),
      num_list_feedback: Cell::new(!0u64),
      feedback_samples: Cell::new(0),
      global_slot_cache: vec![Cell::new(-1i64); code_len].into_boxed_slice(),
    }
  }
}

/// A method value bound to a specific receiver; produced only when a
/// method is accessed WITHOUT being called immediately (`var f =
/// obj.method`), so it can be stored, passed around, and invoked later
/// on its own. Direct call-site method calls (`obj.method(args)`,
/// `self.method(args)`, `parent.method(args)`) never go through this;
/// they're compiled straight to Invoke/InvokeSuper, which look up and
/// call in one step with no heap allocation. This exists purely for the
/// "method as a first-class value" case.
pub struct ObjBoundMethod {
  pub receiver: Value,
  pub method: Value,
}

/// `#[repr(C)]` for the same reason `ObjInstance` is: `jit::codegen`
/// reads `function` by fixed offset (see
/// `obj_closure_function_offset`), so field order here is part of the
/// generated code's contract rather than an implementation detail.
#[repr(C)]
pub struct ObjClosure {
  /// The prototype this closure was created from, as the tagged `Value`
  /// that points at its `Obj::Func`; not a raw pointer; so the GC's
  /// mark phase can trace it like any other reference and keep the
  /// prototype (and everything in its constant pool) alive for exactly
  /// as long as some closure still needs it.
  pub function: Value,
  /// One entry per `function.upvalues` descriptor, in the same order.
  /// Each Value here points at an `Obj::Upvalue`.
  pub upvalues: SmallVec<[Value; 2]>,
}

/// A module's own global namespace; structurally identical to
/// `VM::global_slots`/`global_names` (a growable slot Vec plus a
/// name->slot map), just scoped to one module instead of the whole
/// VM. This is what gives every module a truly separate set of
/// `def`/`var`/`class` bindings: `Instr::GetGlobal`/`SetGlobal`/
/// `AssignGlobal` resolve against THIS table instead of the VM's root
/// one whenever the currently executing function's
/// `ObjFunction::globals_module` says so (see `vm.rs`).
pub struct ModuleNamespace {
  pub slots: Vec<Cell<Value>>,
  /// Index-aligned with `slots`. `true` (the default for every newly
  /// created slot) means this name is part of the module's own public
  /// surface, reachable via `GetField`/`Invoke` from outside; `false`
  /// means it only exists for this module's OWN code to resolve by
  /// name (see `Instr::GetGlobal`), e.g. a name pulled in by a plain
  /// (non-`@`) wildcard import. Per spec (`NOTES.md`), imports are
  /// local by default and `@` is what actually re-exports them, and a
  /// wildcard import has no compile-time-known name list to turn into
  /// genuine lexical locals the way a plain named import can, so this
  /// bit is the runtime stand-in for that same "not re-exported"
  /// contract.
  pub public: Vec<Cell<bool>>,
  pub names: FxHashMap<String, u32>,
}

impl ModuleNamespace {
  pub fn new() -> Self {
    ModuleNamespace {
      slots: Vec::new(),
      public: Vec::new(),
      names: FxHashMap::default(),
    }
  }

  /// Find-or-create `name`'s slot, growing the table with a fresh
  /// nil-initialized, public-by-default slot the first time this
  /// specific module sees that name; mirrors `VM::get_or_create_global_slot`
  /// exactly.
  pub fn get_or_create_slot(&mut self, name: &str) -> u32 {
    if let Some(&s) = self.names.get(name) {
      return s;
    }
    let s = self.slots.len() as u32;
    self.slots.push(Cell::new(Value::nil()));
    self.public.push(Cell::new(true));
    self.names.insert(name.to_string(), s);
    s
  }

  pub fn get(&self, name: &str) -> Option<Value> {
    self.names.get(name).map(|&s| self.slots[s as usize].get())
  }

  /// Like `get`, but returns `None` for a slot that exists but isn't
  /// part of this module's public surface (see `public` above);
  /// what `GetField`/`Invoke` on a module value from OUTSIDE this
  /// module's own code must use instead of `get`, so a plain wildcard
  /// import's names stay invisible to external dot-access exactly like
  /// a plain named import's already-local bindings are.
  pub fn get_public(&self, name: &str) -> Option<Value> {
    let &s = self.names.get(name)?;
    if !self.public[s as usize].get() {
      return None;
    }
    Some(self.slots[s as usize].get())
  }

  /// Marks an existing slot as not part of this module's public
  /// surface. Called only when a plain (non-`@`) wildcard import
  /// writes an entry; see `public`'s own docs.
  pub fn mark_private(&self, slot: u32) {
    self.public[slot as usize].set(false);
  }

  pub fn set(&mut self, name: &str, v: Value) {
    let s = self.get_or_create_slot(name);
    self.slots[s as usize].set(v);
  }
}

/// A loaded `.zu` file (or package `index.zu`), as produced by
/// `vm::modules::import`. Wrapped in a `RefCell` (like `ObjClass`)
/// because its namespace keeps growing throughout its own top-level
/// execution.
pub struct ObjModule {
  /// Display/error-message name; the file's own stem, or the
  /// enclosing directory's name for a package's `index.zu`. NOT
  /// necessarily the name any particular importer bound it to (see
  /// `ObjModuleBinding`).
  pub name: String,
  /// Canonical, absolute source path; this module's own `__file__`,
  /// and the cache key `vm::modules` dedupes on.
  pub path: String,
  pub namespace: ModuleNamespace,
  /// False for the entire duration this module's own top-level code
  /// is executing; lets a circular import observe a partially
  /// populated module instead of recursing forever: see
  /// `vm::modules::load_from_candidate`.
  pub loaded: bool,
}

/// What `import PATH [as NAME]` (the default, non-selective,
/// non-`{*}` form) actually binds NAME to, instead of the raw
/// `Obj::Module`: see `Instr::MakePromoted`. Promotion has to be
/// resolved PER IMPORT SITE rather than fixed on the module object
/// itself: the exact same cached module can be promoted under
/// different names by different importers (`import jump` promotes to
/// `jump`'s own `jump()`; `import jump as slow` promotes to `slow()`
/// instead, per the "function promotion" docs).
pub struct ObjModuleBinding {
  pub module: Value,
  /// The module member matching this binding's own name, if it
  /// exists AND is callable; what makes `NAME(...)` work directly.
  /// `None` means this binding just forwards `.field` access; calling
  /// it directly is a TypeError.
  pub promoted: Option<Value>,
  /// The LOCAL name this was bound under (NAME, or the last import
  /// path segment); purely for Display / error messages, matching
  /// the documented REPL rendering `<module m at ...>`.
  pub bind_name: String,
}

/// `#[repr(C)]` so `jit::codegen` can read `func` by fixed offset (see
/// `obj_native_func_offset`); field order here is part of the
/// generated code's contract, not an implementation detail.
#[repr(C)]
pub struct NativeFunction {
  pub name: &'static str,
  /// Minimum number of arguments required.
  pub min_arity: u8,
  /// If true, min_arity is a floor ("one or more"); if false, arg count
  /// must equal min_arity exactly.
  pub variadic: bool,
  /// Was this native registered as a method (`builtins::method`/
  /// `method_n`/`method_opt`), i.e. does `VM::call_native` splice an
  /// implicit receiver into `args[0]` before this runs? Free functions
  /// (`natives.rs`) leave this `false`. Consulted ONLY by
  /// `VM::call_native`'s own arity-mismatch message: see that call
  /// site's doc comment for why the count a user sees has to have the
  /// receiver subtracted back out.
  pub is_method: bool,
  pub func: NativeFn,
}

/// A class value: the shared, heap-allocated "shape" every instance of
/// it points back at. Method/field-slot tables are fully merged with
/// the superclass chain once, at declaration time (see `Instr::MakeClass`)
///; so a call site never needs to walk ancestors to find an instance
/// method or a field's slot index, just one hashmap lookup. Statics are
/// the deliberate error: `static_slots`/`statics` hold ONLY this
/// class's own declarations, and a lookup that misses walks `superclass`
/// live (see `lookup_static` in vm.rs); so an inherited static genuinely
/// shares storage with wherever it's actually declared, rather than being
/// copied.
pub struct ObjClass {
  pub name: String,
  pub superclass: Option<Value>,
  /// Own + inherited instance methods, pre-merged (own overrides
  /// inherited of the same name). Also where a self-named method (this
  /// class's constructor, if it declares one) lives: see
  /// `Instr::FinalizeClass`.
  pub methods: FxHashMap<String, Value>,
  /// Name -> slot index for OWN + inherited instance fields, pre-merged
  /// the same way; `field_count` is the total flat layout size every
  /// `ObjInstance.fields` of this class is allocated with.
  pub field_slots: FxHashMap<String, u16>,
  pub field_count: u16,
  /// This class's OWN declared instance fields' initializer (arity 1:
  /// self); None if it declares no instance fields itself. Ancestors'
  /// own initializers are invoked separately (root to leaf) at
  /// construction time: see `VM::instantiate`; so this is
  /// deliberately NOT chained to call the superclass's initializer.
  pub own_field_initializer: Option<Value>,
  /// Resolved once, when the class is declared (see
  /// `Instr::FinalizeClass`): this class's own same-named method if it
  /// declares one, else inherited from the nearest ancestor that does,
  /// else None (no constructor anywhere in the chain; valid, just
  /// means "run field inits and stop").
  pub constructor: Option<Value>,
  /// Own-only static fields AND static methods, unified in one
  /// namespace (a static method is just a Value that happens to be a
  /// Closure); deliberately not merged with the superclass at
  /// declaration time: see the struct-level doc comment above.
  pub static_slots: FxHashMap<String, u16>,
  pub statics: Vec<Cell<Value>>,
  /// Which globals table the class declaration itself lives in; `None`
  /// for the main script, `Some(module)` otherwise. Exactly
  /// `ObjFunction::globals_module` for whichever function was executing
  /// the `class` declaration; stamped once at `Instr::MakeClass` and
  /// never changed afterward. Every class reaches `Instr::FinalizeClass`
  /// (which already rejects a duplicate name in this same scope), so
  /// this is always resolvable back to a real `(home, name)` pair --
  /// see `modules::isolate_util` for the one consumer that actually
  /// needs it: re-finding a class by name in a freshly-loaded copy of
  /// its own home module when a value crosses an isolate boundary.
  pub globals_module: Option<Value>,
}

/// A `#[repr(C)]`-guaranteed-layout owning slice of `Cell<Value>`;
/// `ObjInstance`'s own field storage, in place of a plain
/// `Vec<Cell<Value>>`. Exists purely so `jit::codegen`'s field-access
/// fast path (a LATER piece of work: see `Obj`'s own docs on the
/// two-layer plan this is part of) can compute `ptr + slot * 8`
/// directly from generated machine code against a real, documented
/// layout guarantee. `Vec<T>`/`Box<[T]>`'s own internal representation
/// is NOT an official stability guarantee the way a `#[repr(C)]`
/// struct's field layout is; every Rust compiler today happens to lay
/// out a fat pointer as `{data, len}`, but nothing in the language
/// promises that stays true, which is exactly the gap this closes.
///
/// Never resized after construction (see `ObjInstance`'s own docs: a
/// fixed-size, flat, slot-indexed array, sized once at allocation
/// time); so this only ever needs to support "allocate once, read/
/// write elements through `Cell`, free once," never growth.
///
/// Derefs to `[Cell<Value>]` specifically so every existing
/// `.get()`/`.set()`/indexing/`.iter()`/`.iter_mut()`/`.len()` call
/// site (GC marking, field access, `Display`, ...) keeps working
/// completely unchanged; this type is a drop-in replacement for
/// `Vec<Cell<Value>>` at every current use site, not a new API surface
/// callers need to learn.
pub const INLINE_FIELDS: usize = 2;

#[repr(C)]
pub struct FieldStorage {
  ptr: *mut Cell<Value>,
  len: u32,
  inline: [Cell<Value>; INLINE_FIELDS],
}

unsafe impl Send for FieldStorage {}

impl FieldStorage {
  pub fn new(len: usize) -> FieldStorage {
    if len <= INLINE_FIELDS {
      FieldStorage {
        ptr: std::ptr::null_mut(),
        len: len as u32,
        inline: [Cell::new(Value::nil()), Cell::new(Value::nil())],
      }
    } else {
      let boxed: Box<[Cell<Value>]> = vec![Cell::new(Value::nil()); len].into_boxed_slice();
      FieldStorage {
        ptr: Box::into_raw(boxed) as *mut Cell<Value>,
        len: len as u32,
        inline: [Cell::new(Value::nil()), Cell::new(Value::nil())],
      }
    }
  }

  pub fn into_raw_parts(self) -> (*mut Cell<Value>, usize) {
    let this = std::mem::ManuallyDrop::new(self);
    (this.ptr, this.len as usize)
  }

  pub unsafe fn from_raw_parts(ptr: *mut Cell<Value>, len: usize) -> FieldStorage {
    FieldStorage {
      ptr,
      len: len as u32,
      inline: [Cell::new(Value::nil()), Cell::new(Value::nil())],
    }
  }

  #[inline]
  pub fn is_inline(&self) -> bool {
    self.ptr.is_null()
  }

  pub fn as_fields_ptr(&self) -> *const Cell<Value> {
    if self.ptr.is_null() {
      self.inline.as_ptr()
    } else {
      self.ptr
    }
  }

  pub fn as_fields_mut_ptr(&mut self) -> *mut Cell<Value> {
    if self.ptr.is_null() {
      self.inline.as_mut_ptr()
    } else {
      self.ptr
    }
  }
}

impl std::ops::Deref for FieldStorage {
  type Target = [Cell<Value>];
  #[inline(always)]
  fn deref(&self) -> &[Cell<Value>] {
    if self.ptr.is_null() {
      &self.inline[..self.len as usize]
    } else {
      unsafe { std::slice::from_raw_parts(self.ptr, self.len as usize) }
    }
  }
}

impl std::ops::DerefMut for FieldStorage {
  #[inline(always)]
  fn deref_mut(&mut self) -> &mut [Cell<Value>] {
    if self.ptr.is_null() {
      &mut self.inline[..self.len as usize]
    } else {
      unsafe { std::slice::from_raw_parts_mut(self.ptr, self.len as usize) }
    }
  }
}

impl Drop for FieldStorage {
  fn drop(&mut self) {
    if !self.ptr.is_null() {
      unsafe {
        drop(Box::from_raw(std::slice::from_raw_parts_mut(
          self.ptr,
          self.len as usize,
        )));
      }
    }
  }
}

/// An instance value. `fields` is a fixed-size, flat, slot-indexed
/// array sized to `class.field_count` at allocation time; there is no
/// dynamic/open field set; a name not present in `class.field_slots` is
/// a runtime error (see `Instr::GetField`/`SetField` in vm.rs).
///
/// `#[repr(C)]` for the same reason `Obj` itself is (see that type's
/// own docs); `jit::codegen`'s field-access fast path needs to reach
/// `class`/`fields` at fixed, guaranteed offsets from a raw `*const
/// ObjInstance`, not offsets the compiler is otherwise free to
/// rearrange.
#[repr(C)]
pub struct ObjInstance {
  pub class: Value,
  pub fields: FieldStorage,
}

/// Wrapper around `Value` that implements `Hash`/`Eq` in terms of
/// `Value::equals` (content-equality for strings/numbers/nil/bool,
/// pointer identity for lists/dicts/closures/instances/etc.), since
/// `Value` itself has no `Hash` impl and its raw NaN-boxed bits don't
/// hash/compare consistently on their own. This is what lets
/// `DictStorage::index` be a real `FxHashMap`.
#[derive(Clone, Copy)]
pub struct DictKey(pub Value);

impl PartialEq for DictKey {
  fn eq(&self, other: &Self) -> bool {
    self.0.equals(&other.0)
  }
}
impl Eq for DictKey {}

impl Hash for DictKey {
  fn hash<H: Hasher>(&self, state: &mut H) {
    if self.0.is_number() {
      0u8.hash(state);
      let n = self.0.as_number();
      // Integer keys are the common case: indices, ids, packed codes.
      // Their f64 bit patterns all carry zeroes low down, because the
      // value sits up in the exponent and the top of the mantissa, and
      // FxHash's multiply only carries entropy upward. hashbrown picks
      // its bucket from the low bits, so a table keyed on small
      // integers collapses into a handful of buckets and every lookup
      // walks a long probe chain comparing keys. Hashing the integer
      // value puts the entropy where the table reads it.
      //
      // Equal numbers are the same f64 and so always take the same
      // branch, which is what keeps this consistent with `DictKey`'s
      // `PartialEq`. Non-integral values, NaN and the infinities all
      // fall through to the bits, where they were already fine.
      if n.fract() == 0.0 && n >= i64::MIN as f64 && n <= i64::MAX as f64 {
        (n as i64).hash(state);
      } else {
        n.to_bits().hash(state);
      }
    } else if self.0.is_nil() {
      1u8.hash(state);
    } else if self.0.is_bool() {
      2u8.hash(state);
      self.0.as_bool().hash(state);
    } else if self.0.is_string() {
      3u8.hash(state);
      self.0.as_str().hash(state);
    } else if self.0.is_obj() {
      // Every other heap-backed kind (list, dict, instance, closure,
      // ...) is compared by pointer identity in Value::equals, so
      // hash on that same pointer. Tradeoff: two distinct-but-equal
      // lists used as dict keys hash differently.
      4u8.hash(state);
      (self.0.as_obj() as usize).hash(state);
    } else {
      5u8.hash(state);
    }
  }
}

/// Backing storage for a Dict value. `entries` preserves insertion
/// order (needed for iteration, `@key`/`@value`, and Display);
/// `index` gives `get`/`set` O(1) average instead of the old O(n)
/// linear scan. Kept in sync because every mutation goes through
/// `set`, never touching `entries` directly from outside.
pub struct DictStorage {
  pub entries: Vec<(Value, Value)>,
  index: FxHashMap<DictKey, usize>,
}

impl DictStorage {
  pub fn new() -> Self {
    DictStorage {
      entries: Vec::new(),
      index: FxHashMap::default(),
    }
  }

  /// Builds storage from raw pairs, de-duplicating by VALUE equality
  /// (via `set`'s own last-write-wins rule); same semantics the old
  /// `Heap::alloc_dict` had.
  pub fn from_pairs(pairs: Vec<(Value, Value)>) -> Self {
    let mut storage = DictStorage::new();
    for (k, v) in pairs {
      storage.set(k, v);
    }
    storage
  }

  pub fn len(&self) -> usize {
    self.entries.len()
  }

  pub fn get(&self, key: &Value) -> Option<Value> {
    self.index.get(&DictKey(*key)).map(|&i| self.entries[i].1)
  }

  pub fn set(&mut self, key: Value, value: Value) {
    if let Some(&i) = self.index.get(&DictKey(key)) {
      self.entries[i].1 = value;
    } else {
      let i = self.entries.len();
      self.entries.push((key, value));
      self.index.insert(DictKey(key), i);
    }
  }

  pub fn index_of(&self, key: &Value) -> Option<usize> {
    self.index.get(&DictKey(*key)).copied()
  }

  /// Rebuilds `index` from scratch against `entries`' CURRENT key
  /// bits. Needed after anything rewrites a key's `Value` in place
  /// without going through `set`; specifically, a minor collection
  /// relocating a reference-type key (`DictKey`'s `Hash` impl hashes
  /// the raw pointer for exactly those, see its own docs), which
  /// changes that key's hash without `index`'s stored bucket knowing
  /// to move with it. See `VM::walk_children_mut`'s `Obj::Dict` case.
  pub(crate) fn reindex(&mut self) {
    self.index.clear();
    for (i, (k, _)) in self.entries.iter().enumerate() {
      self.index.insert(DictKey(*k), i);
    }
  }
}

/// Runtime state behind a `file(...)` object: see `Obj::File`.
/// `handle` is `None` exactly when the file is currently closed
/// (never successfully opened, closed via `.close()`, or auto-closed
/// after a full, unlengthed `.read()`); every method needing real I/O
/// checks this and raises rather than panicking when it's absent.
pub struct FileHandle {
  pub path: String,
  /// The mode string exactly as given to `file(path, mode)` (defaults
  /// to `"r"`), reported back verbatim by `.mode()`.
  pub mode: String,
  /// Was `b` present in `mode`? Decides whether `.read()`/`.gets()`
  /// yield a string or a bytes object.
  pub binary: bool,
  pub handle: Option<File>,
  /// True only for the `io.stdin`/`io.stdout`/`io.stderr` objects
  /// built by `modules::io::std_file`; these wrap a DUPLICATED
  /// standard-stream file descriptor and have no real path on disk to
  /// reopen (`path` is a display-only sentinel like `"<stdout>"`).
  /// `.read()`/`.write()` normally reopen-then-close around each call
  /// (see `builtins::file::do_read`/`do_write`) so a bare `file(...)`
  /// object works as a one-shot convenience call; that reopen would
  /// simply fail for a stream, so this flag makes `.read()`/`.write()`
  /// behave like `.gets()`/`.puts()` instead: use (and keep open)
  /// whatever handle is already there.
  pub is_stream: bool,
  /// The file descriptor this handle stands for, or `-1` when it is
  /// not known.
  ///
  /// For the three standard streams it is the descriptor they are
  /// named by (0, 1, 2), not the private duplicate `modules::io`
  /// actually holds, so anything that cares WHICH stream it has can
  /// just compare the number. `io.capture()` is the current caller;
  /// redirecting stderr, or reading stdin from somewhere else, would
  /// use the same field.
  ///
  /// For an ordinary file it starts at `-1` and `file.number()` fills
  /// it in from the live handle the first time it is asked, on the
  /// platforms that have descriptors at all. `close()` and `open()`
  /// put it back, since the next handle is free to land on a
  /// different descriptor.
  pub fd: i32,
}

/// A type-erased handle to an arbitrary Rust value, letting a native
/// module wrap an external resource; a SQLite connection, a libgd
/// image buffer, a TLS context, a compiled regex, anything; as an
/// ordinary Zuri `Value` that can be stored in variables, lists,
/// dicts, and instance fields, and passed to/from native functions
/// just like any other value.
///
/// Cleanup needs no separate finalizer callback:
/// `value` is a real, owned Rust value, so when this `ObjPtr` is
/// dropped (by `Heap::sweep`, the same as every other dead object)
/// its own `Drop` impl runs; closing a connection, freeing a
/// buffer, whatever the wrapped type does when it goes out of scope
/// in ordinary Rust code.
pub struct ObjPtr {
  /// A short, stable identifier for what's wrapped; e.g.
  /// `"sqlite3_connection"`, `"gd_image"`, `"openssl_ctx"`. Checked
  /// by natives via `Value::ptr_type_name()`/`Value::is_ptr_type()`
  /// before downcasting: `Any::downcast` alone matches on `TypeId`,
  /// which is precise but gives an opaque failure mode when a native
  /// is handed the wrong kind of pointer by mistake (e.g. a user
  /// passing a `gd_image` where a db connection was expected) --
  /// checking `type_name` first lets that surface as an ordinary,
  /// readable Zuri `TypeError` instead.
  pub type_name: &'static str,
  /// The wrapped value itself, type-erased. Boxed so `Obj` doesn't
  /// need to know the size of every possible wrapped type up front,
  /// and so it participates in this arena's normal alloc/sweep/drop
  /// lifecycle like everything else.
  ///
  /// `+ Send`, not just `Any`: what makes it sound for
  /// `modules::isolate_util::transfer` to MOVE a wrapped resource
  /// (a socket, a future db connection, ...) to a destination
  /// isolate's own thread: see `ObjPtr::take`. Costs nothing to every
  /// existing user of this type: every native module that currently
  /// wraps something in a `Ptr` already wraps a plain, self-contained
  /// resource with no thread-affinity of its own (no `Rc`/`RefCell`
  /// inside it), so this was already true, just not yet required by
  /// the type system.
  pub value: Box<dyn Any + Send>,
}

impl ObjPtr {
  #[inline]
  pub fn downcast_ref<T: 'static>(&self) -> Option<&T> {
    self.value.downcast_ref::<T>()
  }

  #[inline]
  pub fn downcast_mut<T: 'static>(&mut self) -> Option<&mut T> {
    self.value.downcast_mut::<T>()
  }

  /// Takes the wrapped value out, leaving this `Ptr` in the same
  /// "consumed" state a `.finish()`-style operation already leaves
  /// one in elsewhere in this codebase (see e.g. `modules::compress`'s
  /// own use of this exact `mem::replace`-to-`Box::new(())` idiom) --
  /// `type_name`/`downcast_*` reflect nothing useful on this `ObjPtr`
  /// afterward. Used when a resource genuinely MOVES to another
  /// isolate rather than being copied; the one case in the whole
  /// isolate-transfer system where the source side can't stay valid
  /// afterward (see `modules::isolate_util::transfer`'s own docs on
  /// why a `Ptr` is the sole error to "always copy, never share,
  /// source stays valid"). Callers are responsible for also updating
  /// `type_name` if they want `ptr_type_name()` to say something more
  /// specific than "still tagged, but empty"; this only swaps the
  /// payload.
  pub fn take(&mut self) -> Box<dyn Any + Send> {
    std::mem::replace(&mut self.value, Box::new(()))
  }
}

/// Everything a native function body gets handed. `args` is an OWNED
/// copy of the call's arguments, not a borrow into VM::registers; it
/// has to be, because `vm` is a live &mut VM at the same time, and a
/// slice into the VM's own register array would alias with that. This
/// is the real cost of letting natives call back into Zuri code via
/// `vm.call_value(...)`.
///
/// `name` is the SAME string as `NativeFunction::name` this call was
/// dispatched through; carried here specifically so the
/// `enforce_arg_*!` family (see `builtins::enforce`) can generate
/// "'foo' expects ..." messages without every native having to spell
/// its own name out by hand at every call site.
pub struct ZuriContext<'a> {
  pub vm: &'a mut VM,
  pub args: &'a [Value],
  pub name: &'static str,
}

impl<'a> ZuriContext<'a> {
  /// Shorthand for `ctx.vm.heap`, for natives that only need the heap.
  pub fn heap(&mut self) -> &mut Heap {
    &mut self.vm.heap
  }
}

/// A plain fn pointer, not a boxed closure. Takes &mut Heap (not &mut VM)
/// specifically so it can be called while a slice of VM::registers is
/// still borrowed: see the disjoint-field-borrow note in Instr::Call.
pub type NativeFn = fn(&mut ZuriContext) -> Result<Value, String>;

/// Which generation a `GcBox` currently belongs to. Every object is
/// born `Young`, living in the nursery; the first time it survives a
/// minor collection, it's promoted; copied for real into old-
/// generation chunk storage (see `Heap::forward_or_promote`), never
/// moved again after that.
#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
enum Generation {
  Young = 0,
  Old = 1,
}

/// `Generation::Old`'s own discriminant, as the raw byte a
/// `#[repr(u8)]` enum actually stores; what `jit::codegen`'s inlined
/// write barrier compares the byte it loads from `GcBox::generation`
/// against, with no way to name the Rust enum itself. Cross-checked
/// against the real discriminant by `gcbox_layout_tests`.
pub const GENERATION_OLD_BYTE: u8 = 1;

thread_local! {
  /// Head of the remembered set; every `Old` object that's been
  /// mutated since the last collection, threaded through
  /// `GcBox::list_next` (see that field's docs). A minor collection
  /// treats each of these as an extra root (specifically: walks its
  /// children looking for young objects to keep alive), since an old
  /// object mutated after its last full scan is the only way an
  /// old->young pointer can exist: see `write_barrier`.
  ///
  /// Thread-local rather than a `Heap` field because several of this
  /// GC's mutation choke points (`Value::list_set`, `Value::dict_set`,
  /// `ModuleNamespace::set`, ...) only ever have a bare `*const Obj` in
  /// hand, never a `&Heap`; there's only ever one `VM`/`Heap`
  /// instantiated per process (confirmed: `VM::new` has exactly one
  /// call site, in `bin/zuri.rs`), so a thread-local is sound and
  /// avoids threading a `&Heap` through every one of those call sites.
  static REMEMBERED_HEAD: Cell<*const GcBox> = const { Cell::new(std::ptr::null()) };
}

/// Write barrier; call after mutating any field of an ALREADY-LIVE
/// heap object (never needed for a value being written at
/// construction time, since a just-allocated object is always Young
/// by construction and Young objects are always fully rescanned by
/// the very next minor collection regardless).
///
/// Deliberately coarse: it doesn't inspect what was actually written,
/// only whether `container` itself is `Old`. Any mutation of an old
/// object; regardless of whether the new value happens to be a
/// young pointer, an old pointer, or not a pointer at all; adds it
/// to the remembered set (idempotently; `remembered` guards against
/// queuing the same box twice before the next collection drains it).
/// A field-precise barrier would need to know the specific value being
/// written at every call site and reason correctly about its
/// generation; this coarse version only ever needs the one pointer
/// already at hand, which is what makes it tractable to get right at
/// every one of the many mutation sites in the interpreter, natives,
/// and JIT runtime helpers. The cost of the extra conservatism is
/// bounded and small: mutation of an already-old object is rare
/// relative to allocation in the workloads this collector targets
/// (tree/graph builders that construct once and rarely mutate after),
/// and a stale remembered entry only costs a few wasted pointer reads
/// during the next minor collection, never a correctness problem.
pub(crate) fn write_barrier(container: *const Obj) {
  let gcbox = unsafe { &*Heap::gcbox_of(container) };
  if gcbox.generation.get() == Generation::Old && !gcbox.remembered.get() {
    gcbox.remembered.set(true);
    REMEMBERED_HEAD.with(|head| {
      gcbox.list_next.set(head.get());
      head.set(gcbox as *const GcBox);
    });
  }
}

/// Byte offset from a `*const Obj` BACK to its owning `GcBox`'s
/// `generation` byte; negative, since the `obj` payload is the last
/// field of the header (see `GcBox`'s own deliberate field ordering).
///
/// Exists so `jit::codegen` can inline `write_barrier`'s own guard
/// (`generation == Old && !remembered`) as two byte loads and a branch,
/// instead of paying a real call on every single field write; the
/// overwhelmingly common answer at a hot mutation site is "no barrier
/// needed" (either the object is still young, or it's old and was
/// already remembered earlier this cycle), and that answer costs
/// nothing to reach inline. Only the rare true case calls out to
/// `jit::runtime::zuri_jit_write_barrier`, which just re-runs the real
/// `write_barrier` in full.
///
/// Derived from `offset_of!` rather than hand-written, and
/// cross-checked against a real allocation by `gcbox_layout_tests`, for
/// exactly the reason `obj_payload_offset` measures instead of
/// asserting: a future reordering of `GcBox`'s fields must not silently
/// turn generated code into a wild read.
pub fn obj_to_gcbox_generation_offset() -> i32 {
  std::mem::offset_of!(GcBox, generation) as i32 - std::mem::offset_of!(GcBox, obj) as i32
}

/// Byte offset from a `*const Obj` known (via `obj_tag`) to be
/// `Obj::Native` to that native's `func` pointer.
///
/// Exists so a compiled call site can guard on WHICH NATIVE a callee
/// register holds without baking the object's address, which a minor
/// collection would invalidate the first time it promoted it; the
/// same hazard `obj_closure_function_offset` exists for, and the reason
/// `alloc_native` deliberately stays a young allocation.
///
/// A `NativeFn` is a `'static` function pointer, so it is stable for
/// the life of the program and unique per registered builtin.
pub fn obj_native_func_offset() -> usize {
  obj_payload_offset() + std::mem::offset_of!(NativeFunction, func)
}

/// Byte offset from a `*const Obj` known (via `obj_tag`) to be
/// `Obj::Closure` to that closure's `function` field; the tagged
/// `Value` naming its prototype.
///
/// Exists so a compiled call site can guard on WHICH FUNCTION a callee
/// register holds rather than on the closure's own address. That
/// distinction is load-bearing: `alloc_closure` only forces class
/// methods into the non-moving old generation, so an ordinary
/// top-level function's closure is born in the nursery and RELOCATES
/// the first time it survives a minor collection. Any guard that baked
/// such a closure's address would silently stop matching from that
/// point on, disabling whatever fast path it protects for the rest of
/// the run. A prototype never moves (`alloc_function` is always
/// `alloc_old`), so guarding on it stays true for the life of the
/// program.
pub fn obj_closure_function_offset() -> usize {
  obj_payload_offset() + std::mem::offset_of!(ObjClosure, function)
}

/// Byte offset from a `*const ObjFunction` to its own compiled-entry
/// cell (`JitInfo::entry`). Read directly by `jit::codegen`'s inline
/// construct fast path (`emit_inline_construct`) instead of a helper
/// call, for the same reason `emit_construct_known`'s existing
/// `zuri_jit_construct_prepare` re-reads this fresh on every call
/// rather than trusting a value baked at the CALLER's own compile
/// time: a constructor's own compile can (and often does, since
/// constructors tend to be small and simple) finish AFTER the caller
/// that constructs it already has, and re-reading live is what lets
/// the caller start using it the moment that happens, instead of
/// being stuck on a stale `entry == 0` for the rest of the run.
///
/// Sound to read as a plain `u64` with no further interpretation:
/// `Cell<Option<EntryFn>>` has a null-pointer niche (a function pointer
/// is never validly null), so `0` and "the entry address" are the
/// type's own only two representations; no separate `is_some` tag
/// byte to also read.
pub const fn obj_function_jit_entry_offset() -> usize {
  std::mem::offset_of!(ObjFunction, jit) + std::mem::offset_of!(JitInfo, entry)
}

pub fn obj_func_proto_offset() -> usize {
  obj_payload_offset()
}

pub const fn obj_function_arity_offset() -> usize {
  std::mem::offset_of!(ObjFunction, arity)
}

pub const fn obj_function_variadic_offset() -> usize {
  std::mem::offset_of!(ObjFunction, variadic)
}

pub const fn obj_function_num_registers_offset() -> usize {
  std::mem::offset_of!(ObjFunction, num_registers)
}

pub const fn obj_function_is_method_offset() -> usize {
  std::mem::offset_of!(ObjFunction, is_method)
}

/// Byte offset from a `*const Obj` known (via `obj_tag`) to be
/// `Obj::List` to the start of its `ListStorage`.
///
/// Measured from a real probe object rather than derived, for the same
/// reason `obj_payload_offset` is: `RefCell`'s own field order is not
/// guaranteed, so the only honest way to learn where its value sits is
/// to ask a live one via `as_ptr()`. Everything past that point IS
/// guaranteed; `ListStorage` is `#[repr(C)]` precisely so compiled
/// code can read its `ptr`/`len` directly (see `vm::list`).
pub fn obj_list_storage_offset() -> usize {
  static OFFSET: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
  *OFFSET.get_or_init(|| {
    let probe = Obj::List(RefCell::new(ListStorage::new()));
    let obj_addr = &probe as *const Obj as usize;
    let storage_addr = match &probe {
      Obj::List(cell) => cell.as_ptr() as usize,
      _ => unreachable!(),
    };
    storage_addr - obj_addr
  })
}

/// Byte offset from a proven-`Obj::List` pointer to the element data
/// pointer; what `jit::codegen` loads instead of calling back into
/// Rust for it.
pub fn obj_list_ptr_offset() -> i32 {
  (obj_list_storage_offset() + crate::vm::list::LIST_PTR_OFFSET as usize) as i32
}

/// `obj_list_ptr_offset`'s sibling for the element count.
pub fn obj_list_len_offset() -> i32 {
  (obj_list_storage_offset() + crate::vm::list::LIST_LEN_OFFSET as usize) as i32
}

pub fn obj_list_cap_offset() -> i32 {
  (obj_list_storage_offset() + crate::vm::list::LIST_CAP_OFFSET as usize) as i32
}

/// `obj_list_ptr_offset`'s sibling for the inline element buffer, which
/// is where the elements live whenever the data pointer is null: see
/// `vm::list::ListStorage`.
pub fn obj_list_inline_offset() -> i32 {
  (obj_list_storage_offset() + crate::vm::list::LIST_INLINE_OFFSET as usize) as i32
}

/// `obj_to_gcbox_generation_offset`'s sibling for the `remembered`
/// flag: see its docs.
pub fn obj_to_gcbox_remembered_offset() -> i32 {
  std::mem::offset_of!(GcBox, remembered) as i32 - std::mem::offset_of!(GcBox, obj) as i32
}

/// `Obj::Str`'s ASCII flag: nobody has asked yet.
pub const ASCII_UNKNOWN: u8 = 0;

/// `Obj::Str`'s ASCII flag: every byte is below 0x80, so a codepoint
/// index is also a byte index.
pub const ASCII_YES: u8 = 1;

/// `Obj::Str`'s ASCII flag: at least one multi-byte codepoint, so a
/// codepoint index says nothing about a byte offset.
pub const ASCII_NO: u8 = 2;

/// Byte offset from a proven-`Obj::Str` pointer to its cached ASCII
/// flag, for compiled code to read directly.
///
/// Measured against a live probe rather than assumed, for the same
/// reason `obj_str_data_offsets` measures: where the flag lands inside
/// the variant depends on how `String` itself is laid out, which is not
/// a language guarantee. Taken as the difference between the field's
/// own address and the enum's, so it stays right whatever that layout
/// turns out to be.
pub fn obj_str_ascii_offset() -> i32 {
  static OFFSET: std::sync::OnceLock<i32> = std::sync::OnceLock::new();
  *OFFSET.get_or_init(|| {
    let probe = Obj::Str(String::from("probe"), Cell::new(ASCII_UNKNOWN));
    let base = &probe as *const Obj as usize;

    match &probe {
      Obj::Str(_, flag) => (flag as *const Cell<u8> as usize - base) as i32,
      _ => unreachable!("the probe was just built as an Obj::Str"),
    }
  })
}

/// Byte offsets from a proven-`Obj::Str` pointer to that `String`'s
/// heap data pointer and UTF-8 BYTE length (not the codepoint count
/// `StringIntrinsic::Length` computes; that's a scan over these same
/// bytes); what `jit::codegen` reads directly for `StringIntrinsic::
/// IsEmpty`/`Length` instead of calling back into Rust for either.
///
/// Unlike `obj_list_storage_offset`, there's no `#[repr(C)]` type of
/// ours to lean on here: `Obj::Str` wraps a plain `std::String`, whose
/// internal field layout (order, or even which fields exist) is not
/// part of any language guarantee. What IS guaranteed is that the
/// standard library picks exactly ONE layout for `String` per compiled
/// build, the same one for every `String` value that build ever
/// creates; so measuring it once against a live probe and caching
/// the result is sound for the same reason `obj_list_storage_offset`'s
/// own `RefCell` probe is: whatever offsets are found here are the
/// offsets every OTHER `String` in this exact binary was laid out
/// with too, not just this one probe.
///
/// The probe deliberately over-allocates capacity (`with_capacity(64)`
/// then `push_str` a short probe text) so its length and capacity are
/// GUARANTEED distinct values; `String::from(short_literal)` alone
/// would allocate exact-fit, making `len == capacity`, which would
/// make the two indistinguishable by value when scanning raw bytes
/// below.
fn obj_str_data_offsets() -> (i32, i32, i32) {
  static OFFSETS: std::sync::OnceLock<(i32, i32, i32)> = std::sync::OnceLock::new();
  *OFFSETS.get_or_init(|| {
    let mut probe_string = String::with_capacity(64);
    probe_string.push_str("zuri-string-offset-probe");
    let want_ptr = probe_string.as_ptr() as usize;
    let want_len = probe_string.len();
    let want_cap = probe_string.capacity();
    debug_assert_ne!(want_len, want_cap);

    let probe = Obj::Str(probe_string, Cell::new(ASCII_UNKNOWN));
    let obj_bytes = unsafe {
      std::slice::from_raw_parts(
        &probe as *const Obj as *const u8,
        std::mem::size_of::<Obj>(),
      )
    };

    let word = std::mem::size_of::<usize>();
    let mut ptr_off = None;
    let mut len_off = None;
    let mut cap_off = None;
    for i in (0..=obj_bytes.len() - word).step_by(word) {
      let value = usize::from_ne_bytes(obj_bytes[i..i + word].try_into().unwrap());
      if value == want_ptr {
        ptr_off = Some(i as i32);
      } else if value == want_len {
        len_off = Some(i as i32);
      } else if value == want_cap {
        cap_off = Some(i as i32);
      }
    }
    (
      ptr_off.expect("String's data pointer not found anywhere in Obj::Str's raw bytes"),
      len_off.expect("String's byte length not found anywhere in Obj::Str's raw bytes"),
      cap_off.expect("String's capacity not found anywhere in Obj::Str's raw bytes"),
    )
  })
}

/// Byte offset from a proven-`Obj::Str` pointer to the string's raw
/// UTF-8 byte data: see `obj_str_data_offsets`'s own docs.
pub fn obj_str_ptr_offset() -> i32 {
  obj_str_data_offsets().0
}

/// `obj_str_ptr_offset`'s sibling for the BYTE length (not codepoint
/// count): see `obj_str_data_offsets`'s own docs.
pub fn obj_str_len_offset() -> i32 {
  obj_str_data_offsets().1
}

/// Byte offset from a proven-`Obj::Str` pointer to the string's capacity.
pub fn obj_str_cap_offset() -> i32 {
  obj_str_data_offsets().2
}

fn obj_bytes_data_offsets() -> (i32, i32) {
  static OFFSETS: std::sync::OnceLock<(i32, i32)> = std::sync::OnceLock::new();
  *OFFSETS.get_or_init(|| {
    let mut probe_vec: Vec<u8> = Vec::with_capacity(64);
    probe_vec.extend_from_slice(b"zuri-bytes-offset-probe");
    let want_ptr = probe_vec.as_ptr() as usize;
    let want_len = probe_vec.len();
    debug_assert_ne!(want_len, probe_vec.capacity());

    let probe = Obj::Bytes(std::cell::RefCell::new(probe_vec));
    let obj_bytes = unsafe {
      std::slice::from_raw_parts(
        &probe as *const Obj as *const u8,
        std::mem::size_of::<Obj>(),
      )
    };

    let word = std::mem::size_of::<usize>();
    let mut ptr_off = None;
    let mut len_off = None;
    for i in (0..=obj_bytes.len() - word).step_by(word) {
      let value = usize::from_ne_bytes(obj_bytes[i..i + word].try_into().unwrap());
      if value == want_ptr {
        ptr_off = Some(i as i32);
      } else if value == want_len {
        len_off = Some(i as i32);
      }
    }
    (
      ptr_off.expect("Bytes' data pointer not found anywhere in Obj::Bytes' raw bytes"),
      len_off.expect("Bytes' byte length not found anywhere in Obj::Bytes' raw bytes"),
    )
  })
}

/// Byte offset from a proven-`Obj::Bytes` pointer to the bytes data buffer.
pub fn obj_bytes_ptr_offset() -> i32 {
  obj_bytes_data_offsets().0
}

/// Byte offset from a proven-`Obj::Bytes` pointer to the bytes length.
pub fn obj_bytes_len_offset() -> i32 {
  obj_bytes_data_offsets().1
}

fn obj_range_data_offsets() -> (i32, i32, i32) {
  static OFFSETS: std::sync::OnceLock<(i32, i32, i32)> = std::sync::OnceLock::new();
  *OFFSETS.get_or_init(|| {
    let probe = Obj::Range {
      lower: 1.0,
      upper: 2.0,
      step: std::cell::Cell::new(3.0),
    };
    let obj_addr = &probe as *const Obj as usize;
    match &probe {
      Obj::Range { lower, upper, step } => {
        let lo_off = (lower as *const f64 as usize) - obj_addr;
        let hi_off = (upper as *const f64 as usize) - obj_addr;
        let step_off = (step.as_ptr() as usize) - obj_addr;
        (lo_off as i32, hi_off as i32, step_off as i32)
      },
      _ => unreachable!(),
    }
  })
}

/// Byte offset from a proven-`Obj::Range` pointer to its lower bound (f64).
pub fn obj_range_lower_offset() -> i32 {
  obj_range_data_offsets().0
}

/// Byte offset from a proven-`Obj::Range` pointer to its upper bound (f64).
pub fn obj_range_upper_offset() -> i32 {
  obj_range_data_offsets().1
}

/// Byte offset from a proven-`Obj::Range` pointer to its step cell (f64).
pub fn obj_range_step_offset() -> i32 {
  obj_range_data_offsets().2
}

#[cfg(test)]
mod gcbox_layout_tests {
  use super::*;

  /// The two offsets `jit::codegen`'s inlined write barrier reads
  /// through, checked against a REAL allocated object rather than
  /// against `offset_of!` again; i.e. that applying them to a
  /// `*const Obj` handed out by `Heap::alloc` actually lands on that
  /// object's own header bytes, and that those bytes read back as the
  /// values `write_barrier` itself would have inspected.
  #[test]
  fn barrier_offsets_land_on_real_header_bytes() {
    assert_eq!(Generation::Old as u8, GENERATION_OLD_BYTE);

    let mut heap = Heap::default();
    let v = heap.alloc(Obj::Str(String::from("probe"), Cell::new(ASCII_UNKNOWN)));
    let obj = v.as_obj();
    let gen_addr = unsafe { (obj as *const u8).offset(obj_to_gcbox_generation_offset() as isize) };
    let rem_addr = unsafe { (obj as *const u8).offset(obj_to_gcbox_remembered_offset() as isize) };

    // Freshly allocated: young, never remembered.
    assert_eq!(unsafe { *gen_addr }, Generation::Young as u8);
    assert_eq!(unsafe { *rem_addr }, 0);

    // Forcing the box old and running the real barrier must be visible
    // through those same two addresses.
    let gcbox = unsafe { &*Heap::gcbox_of(obj) };
    gcbox.generation.set(Generation::Old);
    assert_eq!(unsafe { *gen_addr }, GENERATION_OLD_BYTE);
    write_barrier(obj);
    assert_eq!(unsafe { *rem_addr }, 1);
  }

  /// `obj_native_func_offset` must land on a real native's own `func`
  /// pointer, checked against an actual allocation.
  #[test]
  fn native_func_offset_lands_on_fn_pointer() {
    fn probe_fn(_ctx: &mut ZuriContext) -> Result<Value, String> {
      Ok(Value::nil())
    }
    let mut heap = Heap::default();
    let v = heap.alloc_native(NativeFunction {
      name: "probe",
      min_arity: 0,
      variadic: false,
      is_method: false,
      func: probe_fn,
    });
    let obj = v.as_obj();
    assert_eq!(unsafe { (*obj).tag() }, OBJ_TAG_NATIVE);
    let slot = unsafe { (obj as *const u8).add(obj_native_func_offset()) as *const usize };
    assert_eq!(unsafe { *slot }, probe_fn as *const () as usize);
  }

  /// `obj_str_ptr_offset`/`obj_str_len_offset` must land on a REAL
  /// allocated string's actual data pointer and byte length; checked
  /// against a fresh allocation distinct from the probe
  /// `obj_str_data_offsets` measures against internally, and with a
  /// length that (unlike the internal probe) is NOT ASCII, to also
  /// confirm the length these offsets expose is bytes, not codepoints.
  #[test]
  fn str_offsets_land_on_real_data_and_len() {
    let mut heap = Heap::default();
    let text = "héllo wörld"; // 11 codepoints, 13 UTF-8 bytes
    let v = heap.alloc_string(text.to_string());
    let obj = v.as_obj();
    assert_eq!(unsafe { (*obj).tag() }, OBJ_TAG_STR);

    let ptr_slot =
      unsafe { (obj as *const u8).offset(obj_str_ptr_offset() as isize) as *const usize };
    let len_slot =
      unsafe { (obj as *const u8).offset(obj_str_len_offset() as isize) as *const usize };
    let data_ptr = unsafe { *ptr_slot } as *const u8;
    let byte_len = unsafe { *len_slot };

    assert_eq!(byte_len, text.len());
    assert_ne!(byte_len, text.chars().count());
    let bytes = unsafe { std::slice::from_raw_parts(data_ptr, byte_len) };
    assert_eq!(bytes, text.as_bytes());
  }

  /// `obj_bytes_ptr_offset`/`obj_bytes_len_offset` must land on a REAL
  /// allocated Bytes object's actual data pointer and byte length.
  #[test]
  fn bytes_offsets_land_on_real_data_and_len() {
    let mut heap = Heap::default();
    let raw = vec![0x12u8, 0x34, 0x56, 0x78];
    let v = heap.alloc_bytes(raw.clone());
    let obj = v.as_obj();
    assert_eq!(unsafe { (*obj).tag() }, OBJ_TAG_BYTES);

    let ptr_slot =
      unsafe { (obj as *const u8).offset(obj_bytes_ptr_offset() as isize) as *const usize };
    let len_slot =
      unsafe { (obj as *const u8).offset(obj_bytes_len_offset() as isize) as *const usize };
    let data_ptr = unsafe { *ptr_slot } as *const u8;
    let byte_len = unsafe { *len_slot };

    assert_eq!(byte_len, raw.len());
    let bytes = unsafe { std::slice::from_raw_parts(data_ptr, byte_len) };
    assert_eq!(bytes, raw.as_slice());
  }

  /// `obj_closure_function_offset` must land on a real closure's own
  /// `function` field, checked against an actual allocation rather than
  /// against `offset_of!` a second time.
  #[test]
  fn closure_function_offset_lands_on_prototype() {
    let mut heap = Heap::default();
    let proto = heap.alloc_function(ObjFunction {
      name: String::from("probe"),
      variadic: false,
      chunk: Chunk::new(),
      arity: 0,
      num_registers: 1,
      upvalues: Vec::new(),
      is_method: false,
      owning_class_name: None,
      source_path: std::rc::Rc::from("<probe>"),
      globals_module: None,
      jit: JitInfo::new(0),
    });
    let closure = heap.alloc_closure(ObjClosure {
      function: proto,
      upvalues: SmallVec::new(),
    });
    let obj = closure.as_obj();
    assert_eq!(unsafe { (*obj).tag() }, OBJ_TAG_CLOSURE);
    let slot = unsafe { (obj as *const u8).add(obj_closure_function_offset()) as *const Value };
    assert_eq!(unsafe { *slot }.to_bits(), proto.to_bits());

    let proto_obj = proto.as_obj();
    assert_eq!(unsafe { (*proto_obj).tag() }, OBJ_TAG_FUNC);
    let raw_proto_ptr = unsafe {
      *((proto_obj as *const u8).add(obj_func_proto_offset()) as *const *const ObjFunction)
    };
    assert_eq!(raw_proto_ptr, proto.as_func() as *const ObjFunction);
    let arity = unsafe { *((raw_proto_ptr as *const u8).add(obj_function_arity_offset())) };
    assert_eq!(arity, 0);
    let num_regs =
      unsafe { *((raw_proto_ptr as *const u8).add(obj_function_num_registers_offset())) };
    assert_eq!(num_regs, 1);
  }

  #[test]
  fn inline_list_layout_matches_alloc_list() {
    assert_eq!(std::mem::size_of::<GcBox>(), 64);
    assert_eq!(std::mem::offset_of!(GcBox, obj), 16);
    assert_eq!(obj_list_storage_offset(), 16);
    assert_eq!(obj_list_ptr_offset(), 16);
    assert_eq!(obj_list_len_offset(), 24);
    assert_eq!(obj_list_cap_offset(), 28);
    assert_eq!(obj_list_inline_offset(), 32);

    let mut heap = Heap::default();
    let v0 = Value::number(100.0);
    let v1 = Value::number(200.0);
    let list_val = heap.alloc_list(vec![v0, v1]);
    let obj = list_val.as_obj();
    let gcbox_ptr = Heap::gcbox_of(obj) as *const u64;

    // Word 0 (bytes 0..7): live=1, marked=0, gen=Young(0), remembered=0, chunk_idx=0
    assert_eq!(unsafe { *gcbox_ptr.add(0) }, 1u64);
    // Word 1 (bytes 8..15): list_next = null
    assert_eq!(unsafe { *gcbox_ptr.add(1) }, 0u64);
    // Word 2 (bytes 16..23): Obj tag = OBJ_TAG_LIST (3)
    assert_eq!(unsafe { (*obj).tag() }, OBJ_TAG_LIST);
    // Word 3 (bytes 24..31): RefCell borrow counter = 0
    assert_eq!(unsafe { *gcbox_ptr.add(3) }, 0u64);
    // Word 4 (bytes 32..39): ListStorage::ptr = null (inline elements)
    assert_eq!(unsafe { *gcbox_ptr.add(4) }, 0u64);
    // Word 5 (bytes 40..47): len=2 (u32), cap=0 (u32)
    assert_eq!(unsafe { *gcbox_ptr.add(5) }, 2u64);
    // Word 6 (bytes 48..55): inline[0] = v0
    assert_eq!(unsafe { *gcbox_ptr.add(6) }, v0.to_bits());
    // Word 7 (bytes 56..63): inline[1] = v1
    assert_eq!(unsafe { *gcbox_ptr.add(7) }, v1.to_bits());
  }

  #[test]
  fn range_layout_matches_offsets() {
    let mut heap = Heap::default();
    let v = heap.alloc(Obj::Range {
      lower: 12.5,
      upper: 99.5,
      step: Cell::new(2.5),
    });
    let obj = v.as_obj();
    assert_eq!(unsafe { (*obj).tag() }, OBJ_TAG_RANGE);
    let lo_ptr =
      unsafe { (obj as *const u8).offset(obj_range_lower_offset() as isize) as *const f64 };
    let hi_ptr =
      unsafe { (obj as *const u8).offset(obj_range_upper_offset() as isize) as *const f64 };
    let step_ptr =
      unsafe { (obj as *const u8).offset(obj_range_step_offset() as isize) as *const f64 };
    assert_eq!(unsafe { *lo_ptr }, 12.5);
    assert_eq!(unsafe { *hi_ptr }, 99.5);
    assert_eq!(unsafe { *step_ptr }, 2.5);
  }
}

/// Wraps every heap object with an inline GC mark bit so marking is a
/// pointer dereference instead of a HashSet insert.
///
/// Also carries the generational-GC bookkeeping (`generation`,
/// `remembered`/`list_next`, `chunk_idx`). The OLD generation is
/// non-moving: once a `GcBox` lives in `Heap::chunks`, its address
/// never changes for the rest of its life, which is what lets a
/// `Value` stay a bare `Copy` pointer with no indirection for
/// anything that's ever been promoted. The YOUNG generation is NOT
/// non-moving, though: an object born into the nursery (see
/// `Heap::alloc`) gets relocated for real, via an actual copy, the
/// first time it survives a minor collection (see
/// `Heap::forward_or_promote`/`VM::collect_minor`); "promotion" is
/// a genuine address change, not a bit-flip. `Value` still stays a
/// bare pointer safely, but the invariant that makes it sound is
/// broader now: every reference to a young object gets found and
/// rewritten as part of that same collection, not that addresses
/// never change at all. `GcBox`'s own layout is shared identically by
/// both generations specifically so that fixed-offset trick
/// (`gcbox_of`) doesn't need to know or care which one a given
/// pointer currently belongs to.
/// Field order here is deliberate, not incidental: `#[repr(C)]` lays
/// fields out in DECLARATION order with natural alignment padding, no
/// reordering, so the four single-byte flags plus `chunk_idx` (a
/// `u32`, not `usize`; chunks never remotely approach 4 billion) are
/// grouped first to pack into exactly 8 bytes with zero padding,
/// before the two 8-byte-aligned fields. The "obvious" declaration
/// order (bools, then `size`, then the pointer fields) leaves multiple
/// 6-byte alignment gaps instead; 24 extra bytes per object instead
/// of 8, which is real allocation throughput lost on every single
/// object in an allocation-heavy workload.
#[repr(C)]
struct GcBox {
  live: Cell<bool>,
  marked: Cell<bool>,
  generation: Cell<Generation>,
  /// Set once this box has been pushed onto the remembered-set list
  /// (see `write_barrier`), so a second write to the same old object
  /// before the next minor collection doesn't push it again.
  remembered: Cell<bool>,
  /// Index into `Heap::chunks` of the chunk that owns this box.
  /// Needed so a minor collection's sweep; which finds dead young
  /// objects via the intrusive young-list, not by iterating chunks --
  /// still knows which chunk's own free-list/live-count to update,
  /// exactly as `Heap::sweep`'s chunk-major iteration does today for a
  /// full collection.
  chunk_idx: u32,
  /// Intrusive singly-linked list, threaded through `GcBox` itself,
  /// used for BOTH the remembered set and the young-generation set --
  /// never both at once for the same box (a box is only ever in one of
  /// these lists at a time), so sharing the one field costs nothing.
  /// `null` when not linked into either list.
  list_next: Cell<*const GcBox>,
  obj: Obj,
}

/// One fixed-capacity block of GcBox storage. Never grows past
/// `CHUNK_SIZE` (enforced in `Heap::alloc`), so its backing buffer never
/// reallocates; every pointer handed out into a chunk stays valid for
/// the chunk's whole lifetime, which is the whole lifetime of `Heap`.
/// Moving the `Chunk` struct itself (e.g. when the outer `chunks: Vec`
/// grows) only moves the Vec's (ptr, len, cap) header, never the buffer
/// it points at; so that's safe too.
struct GcChunk {
  slots: Vec<GcBox>,
  /// Dead slots within THIS chunk, ready for reuse with no allocator
  /// call. Kept local (not a heap-wide free list) specifically so that
  /// when `live_count` hits zero, dropping the whole `Chunk` also drops
  /// this list; nothing outside needs to be purged or cross-referenced.
  free: Vec<*mut GcBox>,
  /// How many slots in this chunk are currently live. When this hits
  /// zero, every slot in the chunk is garbage and the whole chunk (and
  /// its backing allocation) can be dropped.
  live_count: usize,
}

/// Owns every heap object for the lifetime of the VM. Values only ever hold
/// *const Obj pointers into this arena, never real ownership, which is what
/// lets a Value stay a plain Copy u64.
#[derive(Default)]
pub struct Heap {
  /// `None` marks a chunk that's been reclaimed; kept as a hole
  /// rather than removed, so no other chunk's index ever shifts.
  chunks: Vec<Option<GcChunk>>,
  /// Stack of chunk indices known to have spare capacity (a free slot,
  /// or room left to bump-allocate into). Checked before creating a new
  /// chunk; entries are popped once exhausted or found reclaimed.
  candidates: Vec<usize>,
  /// Running total of `approx_size()` across every live object --
  /// compared against `next_gc` to decide when the VM should pause and
  /// collect. Recomputed from scratch on every sweep rather than
  /// incrementally decremented on free, so it can never drift out of
  /// sync with what's actually still on the heap.
  bytes_allocated: usize,
  /// The `bytes_allocated` threshold that triggers the next collection.
  /// Grows with live heap size after each sweep (see `sweep`), the same
  /// self-tuning strategy clox uses, so steady-state programs settle
  /// into collecting roughly every time the heap doubles rather than
  /// thrashing on a fixed budget.
  next_gc: usize,
  /// How much this heap lets its young generation grow before a minor
  /// collection; `YOUNG_NEXT_GC`, or `ISOLATE_YOUNG_NEXT_GC` for an
  /// isolate's own heap. Fixed for the heap's whole life, which is what
  /// lets `jit::codegen` keep baking it in as an immediate.
  young_next_gc: usize,
  live_count: usize,
  /// Bytes allocated into the young generation since the last minor
  /// (or major) collection; deliberately tracked separately from
  /// `bytes_allocated`, which is the whole-heap total major collection
  /// already keys off. This is what lets a minor collection trigger
  /// far more often, on a far smaller budget.
  young_bytes_allocated: usize,
  /// Mirror of needs_minor_gc() || needs_major_gc() for JIT safepoints
  pub jit_gc_needed: bool,
  /// The young generation's own storage: bump-allocated, chunked
  /// EXACTLY like `chunks` is for the old generation, and for the
  /// same reason; each individual `NurseryChunk`'s own buffer is
  /// reserved at a fixed capacity and never reallocates once
  /// created, so a `Value` pointing into one stays valid for as long
  /// as that chunk exists, but the OUTER `Vec` here is free to grow
  /// without bound: growing it only ever moves lightweight `(ptr,
  /// len, cap)` headers, never the `GcBox` storage a chunk's own
  /// `slots` buffer owns (identical reasoning to `chunks`' own doc
  /// comment on `GcChunk`). This is what lets young allocation grow
  /// completely unbounded between minor collections with no capacity
  /// to guess, overflow, or synchronously collect against mid-alloc.
  ///
  /// Every object here lives at wherever `alloc`'s bump-push left it
  /// until the next minor collection either copies it out to old-gen
  /// storage (it survived) or drops it in place (it didn't): see
  /// `VM::collect_minor` and `reset_nursery`.
  nursery_chunks: Vec<NurseryChunk>,
  /// Index into `nursery_chunks` of the chunk `alloc` is currently
  /// bump-allocating into. Reset to 0 by `reset_nursery`, since every
  /// retained chunk starts that next cycle empty and ready for reuse
  /// in order: see `MAX_RETAINED_NURSERY_CHUNKS`'s own docs for why
  /// `alloc` walks forward through already-allocated chunks instead of
  /// just always using `nursery_chunks.last()` (which would skip past
  /// every retained-but-not-yet-touched chunk straight to allocating a
  /// brand new one, defeating the whole point of retaining them).
  nursery_fill_idx: usize,
  /// Bump-allocation cursor into the ACTIVE nursery chunk
  /// (`nursery_chunks[nursery_fill_idx]`): the address of the next free
  /// `GcBox` slot. `nursery_end` is one past that chunk's last usable
  /// slot, so `alloc`'s entire fast path is "compare, bump, write".
  ///
  /// Both are null before the first allocation of a cycle, which the
  /// same `cur == end` comparison already routes to `refill_nursery` --
  /// no separate initialization check.
  ///
  /// **While a chunk is active, this cursor; NOT that chunk's
  /// `slots.len()`; is the source of truth for how many slots are in
  /// use.** `Vec::push` cannot be the allocation step if generated code
  /// is ever to perform it inline (see `jit::codegen`'s allocation fast
  /// path and `Obj`'s own docs on the `#[repr(C, u8)]` layout that
  /// exists for exactly this), so the length is written back lazily,
  /// by `sync_active_chunk_len`, which MUST run before anything
  /// iterates `nursery_chunks`.
  nursery_cur: *mut GcBox,
  /// One past the last usable slot of the active chunk: see
  /// `nursery_cur`.
  nursery_end: *mut GcBox,
  /// Compile-time string constants, deduplicated by content: see
  /// `alloc_string_old`. Entries live for the whole program (they are a
  /// constant pool, bounded by the program's own text), and are marked
  /// as roots by `VM::collect_garbage`.
  interned_strings: FxHashMap<Box<str>, Value>,
  /// Recycled `FieldStorage` buffers, keyed by their exact field count
  /// (a class's `field_count` is fixed for its whole lifetime, so a
  /// buffer freed for one instance of a class is immediately valid for
  /// the NEXT instance of any class with that same field count; no
  /// bug-prone "close enough" resizing). Populated by `reset_nursery`
  /// when a dead `Obj::Instance` is reclaimed (its `FieldStorage`'s
  /// backing allocation is pulled out via `into_raw_parts` instead of
  /// being freed) and drained by `alloc_instance`, turning what used to
  /// be a real `malloc`+`free` pair on every short-lived instance into
  /// a plain `Vec::pop`/`push` most of the time. Bounded by total
  /// retained bytes (see `FIELD_STORAGE_POOL_BUDGET_FACTOR`) so a
  /// one-off burst of a rarely-used field count doesn't hold memory
  /// forever; exactly the same "retain some, drop the rest" tradeoff
  /// `MAX_RETAINED_NURSERY_CHUNKS` already makes for nursery chunks.
  field_storage_pool: FieldStoragePool,
}

/// One fixed-capacity block of nursery `GcBox` storage; the young
/// generation's counterpart to `GcChunk`, deliberately a separate,
/// leaner type: nursery objects are never individually freed or
/// reused mid-cycle (every survivor is copied OUT, every non-survivor
/// dies with the whole chunk reset at once: see `reset_nursery`),
/// so there's no need for `GcChunk`'s own `free`-list/`live_count`
/// bookkeeping here at all.
struct NurseryChunk {
  slots: Vec<GcBox>,
}

/// Ceiling on the total bytes `Heap::field_storage_pool` keeps alive,
/// as a multiple of the young generation's own budget.
///
/// What the pool has to cover is one collection cycle's worth of dead
/// instances, and a cycle cannot free more than it allocated, so the
/// young budget is the natural bound: at one times the budget the pool
/// absorbs a whole cycle's churn of a single shape without ever
/// reaching `malloc`. Two times rather than one so a cycle whose
/// survivors and casualties overlap in time still lands inside it.
///
/// Bounding the retained BYTES rather than a buffer count per size
/// class is what ties the ceiling to the workload: a program churning
/// millions of four-field instances per cycle and one churning a
/// handful of thirty-field ones both want the same answer, and only a
/// byte total gives it to them. Past the ceiling a freed buffer is
/// dropped for real rather than hoarded, the same "retain some, drop
/// the rest" trade `MAX_RETAINED_NURSERY_CHUNKS` makes for nursery
/// chunks.
const FIELD_STORAGE_POOL_BUDGET_FACTOR: usize = 2;

/// How many field counts `FieldStoragePool` serves from its direct,
/// index-addressed free lists (`0..FIELD_STORAGE_DIRECT_CLASSES`);
/// anything larger falls back to `FieldStoragePool::overflow`. Covers
/// essentially every real class; a type with more than 32 instance
/// fields is rare, and the fallback is only slower, never wrong.
const FIELD_STORAGE_DIRECT_CLASSES: usize = 33;

/// Recycled `FieldStorage` buffers, ready to hand straight back to the
/// next instance of any class with the same field count.
///
/// The direct classes are **intrusive singly-linked free lists**: a
/// buffer sitting in the pool is dead memory, so the link to the next
/// free buffer is stored in its own first cell rather than in any
/// side table. Popping one is a load, a store, and a decrement, with
/// no hashing and no `Vec`; which is the point. This is on the path
/// of every single instance allocation, and it is meant to be
/// reproducible directly as generated machine code (`#[repr(C)]`, and
/// `heads` first, so a head slot is reachable at a compile-time-known
/// offset plus `field_count * 8`).
///
/// `pooled_bytes` exists only to enforce the retention ceiling, which
/// an intrusive list cannot answer by itself.
///
/// Field count 0 is never pooled: `FieldStorage::new(0)` allocates
/// nothing at all (an empty `Box<[T]>` is a dangling pointer), so
/// there is no buffer to recycle and nowhere to put a link.
#[repr(C)]
struct FieldStoragePool {
  heads: [*mut Cell<Value>; FIELD_STORAGE_DIRECT_CLASSES],
  /// Field counts at or beyond `FIELD_STORAGE_DIRECT_CLASSES` fall back
  /// to a hash map; not worth a dedicated array slot for shapes this
  /// large and rare.
  overflow: FxHashMap<usize, Vec<*mut Cell<Value>>>,
  /// Total bytes of buffer currently held across every size class.
  pooled_bytes: usize,
}

impl Default for FieldStoragePool {
  fn default() -> Self {
    Self::new()
  }
}

impl FieldStoragePool {
  fn new() -> Self {
    FieldStoragePool {
      heads: [std::ptr::null_mut(); FIELD_STORAGE_DIRECT_CLASSES],
      overflow: FxHashMap::default(),
      pooled_bytes: 0,
    }
  }

  /// Pops a recycled buffer of exactly `len` cells, or `None`.
  /// Every cell of the returned buffer is `Value::nil()`, upholding
  /// `FieldStorage::new`'s postcondition; including the first, which
  /// `give` overwrote with the free-list link.
  fn take(&mut self, len: usize) -> Option<*mut Cell<Value>> {
    if len <= INLINE_FIELDS {
      return None;
    }
    if len < FIELD_STORAGE_DIRECT_CLASSES {
      let head = self.heads[len];
      if head.is_null() {
        return None;
      }
      // SAFETY: `give` stored the next-free pointer in cell 0 of this
      // very buffer, which it owns exclusively while pooled.
      let next = unsafe { *(head as *const *mut Cell<Value>) };
      self.heads[len] = next;
      self.pooled_bytes = self.pooled_bytes.saturating_sub(Self::buffer_bytes(len));
      unsafe { (*head).set(Value::nil()) };
      return Some(head);
    }
    let list = self.overflow.get_mut(&len)?;
    let ptr = list.pop()?;
    self.pooled_bytes = self.pooled_bytes.saturating_sub(Self::buffer_bytes(len));
    Some(ptr)
  }

  /// What one buffer of `len` cells actually costs to hold.
  #[inline]
  fn buffer_bytes(len: usize) -> usize {
    len * std::mem::size_of::<Cell<Value>>()
  }

  /// Returns a dead instance's buffer to the pool. `false` means the
  /// pool declined it (size class full, or not poolable) and the
  /// caller still owns the allocation.
  ///
  /// Every cell must already be `Value::nil()` on entry; cell 0 is
  /// then repurposed as the free-list link, and `take` restores it.
  fn give(&mut self, ptr: *mut Cell<Value>, len: usize, budget: usize) -> bool {
    if ptr.is_null() || len <= INLINE_FIELDS {
      return false;
    }
    let bytes = Self::buffer_bytes(len);
    if self.pooled_bytes + bytes > budget {
      return false;
    }
    if len < FIELD_STORAGE_DIRECT_CLASSES {
      // SAFETY: this buffer is dead and exclusively owned from here
      // until `take` hands it back out, so its first cell is free
      // storage. `Cell<Value>` is 8 bytes, exactly a pointer.
      unsafe { *(ptr as *mut *mut Cell<Value>) = self.heads[len] };
      self.heads[len] = ptr;
      self.pooled_bytes += bytes;
      return true;
    }
    self.overflow.entry(len).or_default().push(ptr);
    self.pooled_bytes += bytes;
    true
  }
}

/// Byte offsets of `Heap::bytes_allocated`/`next_gc`; combined with
/// `vm::VM_HEAP_OFFSET` in `crate::jit` so compiled code can inline
/// `needs_major_gc()`'s check directly instead of an FFI call at every
/// safepoint. See `vm::VM_HEAP_OFFSET`'s own docs for why this is sound.
pub(crate) const HEAP_JIT_GC_NEEDED_OFFSET: usize = std::mem::offset_of!(Heap, jit_gc_needed);
pub(crate) const HEAP_NURSERY_CUR_OFFSET: usize = std::mem::offset_of!(Heap, nursery_cur);
pub(crate) const HEAP_NURSERY_END_OFFSET: usize = std::mem::offset_of!(Heap, nursery_end);
pub(crate) const HEAP_YOUNG_BYTES_ALLOCATED_OFFSET: usize =
  std::mem::offset_of!(Heap, young_bytes_allocated);
pub(crate) const HEAP_BYTES_ALLOCATED_OFFSET: usize = std::mem::offset_of!(Heap, bytes_allocated);
pub(crate) const HEAP_LIVE_COUNT_OFFSET: usize = std::mem::offset_of!(Heap, live_count);

/// Frees every buffer still sitting in `field_storage_pool` when the
/// `Heap` itself is torn down (process exit; there's exactly one
/// `Heap` for the VM's whole life, so this runs once, not a hot path).
/// Without this, every pooled `*mut Cell<Value>` is just a raw pointer
/// with no owning Rust value anywhere; `Vec<*mut T>`'s own `Drop`
/// only frees the `Vec`'s OWN backing array, never what its pointers
/// point AT, so skipping this would be a genuine leak.
impl Drop for Heap {
  fn drop(&mut self) {
    // Direct classes: walk each intrusive free list, reading each
    // buffer's link out of its own first cell BEFORE that buffer is
    // handed to `FieldStorage`'s own `Drop`, which frees it.
    for len in 1..FIELD_STORAGE_DIRECT_CLASSES {
      let mut ptr = self.field_storage_pool.heads[len];
      while !ptr.is_null() {
        // SAFETY: cell 0 holds the next-free pointer (see
        // `FieldStoragePool::give`); read it before the free below.
        let next = unsafe { *(ptr as *const *mut Cell<Value>) };
        // SAFETY: produced by `into_raw_parts` on a `FieldStorage` of
        // exactly `len` cells, linked in at most once, and unreachable
        // from anywhere else by the time the `Heap` is being torn down
        //; so this is its one and only free.
        drop(unsafe { FieldStorage::from_raw_parts(ptr, len) });
        ptr = next;
      }
      self.field_storage_pool.heads[len] = std::ptr::null_mut();
    }
    for (&len, ptrs) in self.field_storage_pool.overflow.iter() {
      for &ptr in ptrs.iter() {
        // SAFETY: same argument as the direct classes above.
        drop(unsafe { FieldStorage::from_raw_parts(ptr, len) });
      }
    }
  }
}

impl Heap {
  /// Floor for `next_gc`; keeps a small/short-lived program from
  /// triggering a collection after every third allocation.
  const MIN_NEXT_GC: usize = 16 * 1024 * 1024;
  /// After a sweep, the next major collection is scheduled at this
  /// multiple of the OLD generation's surviving size (see
  /// `needs_major_gc`); so it directly sets how much dead-but-
  /// promoted garbage the old generation may accumulate before being
  /// swept again, and therefore trades peak RSS against major-GC
  /// frequency.
  ///
  /// 1.25 rather than a looser 1.5: once `needs_major_gc` stopped
  /// firing on every cycle, that extra slack bought little wall-clock
  /// for a real jump in peak RSS; not worth it.
  const GC_HEAP_GROW_FACTOR: f32 = 1.25;
  /// Fixed (not growing) budget for the young generation; kept
  /// small and constant, unlike `next_gc`, specifically so minor
  /// collections stay cheap and frequent for the whole run instead of
  /// the young budget creeping up alongside the live heap. Exposed as
  /// `pub(crate)` (not just used internally) so `jit::codegen` can
  /// bake it into compiled code as a compile-time immediate instead
  /// of a runtime load; sound specifically because it's the one
  /// threshold in this collector that's truly constant.
  /// 128MB, not the 32MB this originally shipped with: measured
  /// directly on `benchmarks/binary-tree.zu` (`ZURI_GC_LOG=1`), 32MB
  /// meant EVERY single minor collection promoted 100% of what it
  /// scanned; `[gc-minor] promoted/freed across N -> N objects` with
  /// N unchanged on both sides, every single time, because one
  /// mid-sized recursive tree construction alone (a depth-20 binary
  /// tree is ~144MB of `TreeNode`s) already outlives a 32MB nursery
  /// cycle while still fully reachable from the in-progress recursion.
  /// Old-generation then only reclaims that same garbage later, via
  /// the strictly more expensive mark-sweep major collection; exactly
  /// the cost a young generation exists to avoid paying. 128MB gives
  /// real headroom for that same recursive pattern's smaller, genuinely
  /// short-lived calls to die for free.
  pub(crate) const YOUNG_NEXT_GC: usize = 128 * 1024 * 1024;

  /// The same budget for a heap belonging to an isolate rather than to
  /// the main VM.
  ///
  /// Every isolate carries its own nursery, so the figure above is not
  /// a ceiling on one program's young generation but on each of them:
  /// a server running eight workers reaches eight times it before
  /// anything is collected, and resident memory follows. Isolates are
  /// there to run work concurrently, which is the shape of program the
  /// larger budget buys the least for, so they get the smaller one.
  ///
  /// Deep recursion inside an isolate pays for this the way the main VM
  /// did before the budget was raised: a live set that outlives a cycle
  /// gets promoted wholesale instead of dying young.
  pub(crate) const ISOLATE_YOUNG_NEXT_GC: usize = 32 * 1024 * 1024;

  /// Upper bound on how many nursery chunk buffers `reset_nursery`
  /// keeps allocated (emptied, not dropped) between cycles for
  /// immediate reuse. Sized to comfortably cover one full
  /// `YOUNG_NEXT_GC` budget's worth of chunks with some headroom for a
  /// burst that slightly overruns before the next safepoint check
  /// catches it: see `reset_nursery`'s own docs for why retaining
  /// these (instead of freeing every cycle down to one) matters:
  /// truncating to a single chunk every cycle drives thousands of
  /// ~1.2MB alloc/free calls through the allocator on a long-running,
  /// allocation-heavy program, which is exactly the pattern that
  /// pushes glibc's malloc into retaining fragmented,
  /// never-returned-to-the-OS memory; inflating RSS well past what
  /// the GC's own live-byte accounting would justify.
  const MAX_RETAINED_NURSERY_CHUNKS: usize =
    (Self::YOUNG_NEXT_GC / (CHUNK_SIZE * std::mem::size_of::<GcBox>())) + 4;

  pub fn new() -> Self {
    Self::with_young_budget(Self::YOUNG_NEXT_GC)
  }

  /// A heap for an isolate, which collects its young generation on a
  /// smaller budget: see `ISOLATE_YOUNG_NEXT_GC`.
  pub fn new_for_isolate() -> Self {
    Self::with_young_budget(Self::ISOLATE_YOUNG_NEXT_GC)
  }

  fn with_young_budget(young_next_gc: usize) -> Self {
    Heap {
      chunks: Vec::new(),
      candidates: Vec::new(),
      bytes_allocated: 0,
      next_gc: Self::MIN_NEXT_GC,
      young_next_gc,
      live_count: 0,
      young_bytes_allocated: 0,
      jit_gc_needed: false,
      nursery_chunks: Vec::new(),
      nursery_fill_idx: 0,
      nursery_cur: std::ptr::null_mut(),
      nursery_end: std::ptr::null_mut(),
      field_storage_pool: FieldStoragePool::new(),
      interned_strings: FxHashMap::default(),
    }
  }

  #[inline]
  /// Calls `visit` once for every live function prototype on the heap,
  /// in both generations.
  ///
  /// Only ever used by the JIT coverage dump (`ZURI_JIT_COVERAGE`),
  /// which runs once at exit; nothing on any hot path walks the heap
  /// like this.
  pub fn for_each_function(&self, mut visit: impl FnMut(&ObjFunction)) {
    for chunk in self.chunks.iter().flatten() {
      for gcbox in chunk.slots.iter() {
        if !gcbox.live.get() {
          continue;
        }
        if let Obj::Func(proto) = &gcbox.obj {
          visit(proto);
        }
      }
    }

    for chunk in self.nursery_chunks.iter() {
      for gcbox in chunk.slots.iter() {
        if !gcbox.live.get() {
          continue;
        }
        if let Obj::Func(proto) = &gcbox.obj {
          visit(proto);
        }
      }
    }
  }

  pub fn bytes_allocated(&self) -> usize {
    self.bytes_allocated
  }

  #[inline]
  pub fn next_gc(&self) -> usize {
    self.next_gc
  }

  #[inline]
  pub fn object_count(&self) -> usize {
    self.live_count
  }

  /// Live bytes outside the nursery. `young_bytes_allocated` is only
  /// ever bumped alongside `bytes_allocated` (see `alloc`), and zeroed
  /// together with the nursery it accounts for (see `reset_nursery`),
  /// so this can never underflow.
  #[inline]
  pub fn old_bytes_allocated(&self) -> usize {
    self.bytes_allocated - self.young_bytes_allocated
  }

  /// Has the OLD generation grown enough since the last collection that
  /// the VM should pause and run a full (major) collection before
  /// allocating further?
  ///
  /// Deliberately measured against `old_bytes_allocated` rather than
  /// total `bytes_allocated`, which is what makes this collector
  /// generational in practice and not just in structure. Nursery
  /// allocation walks the total up continuously, so a total-based
  /// threshold is really a threshold on ALLOCATION RATE; and since
  /// `run_until`'s safepoint checks this before `needs_minor_gc`, the
  /// major collection would win every race, running a full mark and
  /// sweep of the whole old generation on a schedule that has nothing
  /// to do with whether the old generation actually grew. On an
  /// allocation-heavy workload that means nearly every collection ends
  /// up major, sweeping a huge live old generation to free almost
  /// nothing.
  ///
  /// Against the old generation instead, the two thresholds finally
  /// describe two different things: `YOUNG_NEXT_GC` bounds how much
  /// garbage the nursery accumulates between cheap minor cycles, and
  /// this bounds how far the genuinely long-lived set may grow between
  /// expensive full ones.
  #[inline]
  pub fn needs_major_gc(&self) -> bool {
    self.old_bytes_allocated() > self.next_gc
  }

  /// Has the young generation grown enough that a cheap minor
  /// collection is worth running? Checked far more often than
  /// `needs_major_gc`: see `YOUNG_NEXT_GC`.
  #[inline]
  pub fn update_jit_gc_needed(&mut self) {
    self.jit_gc_needed = self.needs_minor_gc() || self.needs_major_gc();
  }

  #[inline]
  pub fn needs_minor_gc(&self) -> bool {
    self.young_bytes_allocated > self.young_next_gc
  }

  /// This heap's young-generation budget, for `jit::codegen` to bake
  /// into the safepoint check it emits.
  #[inline]
  pub fn young_budget(&self) -> usize {
    self.young_next_gc
  }

  #[inline]
  fn gcbox_of(ptr: *const Obj) -> *const GcBox {
    let offset = std::mem::offset_of!(GcBox, obj);
    // SAFETY: every `*const Obj` reachable from a Value was produced by
    // `alloc`, above, from the `obj` field of a real `GcBox`.
    unsafe { (ptr as *const u8).sub(offset) as *const GcBox }
  }

  /// Is the object behind `ptr` currently in the young generation?
  /// A cheap, read-only peek; unlike `forward_or_promote`, this
  /// never relocates anything, just answers the question. Used by
  /// `VM::ensure_stable_for_compiled_entry` to decide, before paying
  /// for a full minor collection, whether one is even needed.
  #[inline]
  pub(crate) fn is_young(ptr: *const Obj) -> bool {
    let gcbox = unsafe { &*Self::gcbox_of(ptr) };
    gcbox.generation.get() == Generation::Young
  }

  /// Mark the object behind `ptr` reachable this cycle. Returns true
  /// the FIRST time (caller should walk its children), false on repeat
  /// visits; same contract the old HashSet-based version had.
  pub(crate) fn mark_object(ptr: *const Obj) -> bool {
    let gcbox = unsafe { &*Self::gcbox_of(ptr) };
    // Already reclaimed, so there is nothing here to keep alive and, more
    // to the point, nothing safe to read: returning true would send the
    // caller off to walk a freed `Obj`'s children and push whatever those
    // bytes happen to look like onto the mark worklist. See
    // `forward_or_promote`'s matching guard for how a dead pointer
    // reaches a collector in the first place.
    if !gcbox.live.get() {
      return false;
    }
    !gcbox.marked.replace(true)
  }

  /// Rough size in bytes attributed to one heap object, used only to
  /// decide *when* to collect; not an exact accounting (e.g. a
  /// `BigInt`'s own heap limbs aren't sized individually), just enough
  /// to make `next_gc` track real memory pressure instead of raw object
  /// count.
  fn approx_size(obj: &Obj) -> usize {
    use std::mem::size_of;
    size_of::<Obj>()
      + match obj {
        Obj::Str(s, _) => s.len(),
        Obj::Bytes(b) => b.borrow().len(),
        Obj::BigInt(x) => size_of::<BigInt>() + x.bits() as usize,
        Obj::List(items) => items.borrow().len() * size_of::<Value>(),
        Obj::Dict(storage) => {
          let s = storage.borrow();
          s.entries.len() * (size_of::<(Value, Value)>() + size_of::<usize>() * 2)
        },
        Obj::Func(f) => {
          size_of::<ObjFunction>()
            + f.chunk.code.len() * size_of::<crate::vm::chunk::Instr>()
            + f.chunk.constants.len() * size_of::<Value>()
            + f.upvalues.len() * size_of::<Value>()
        },
        Obj::BoundMethod(_) => size_of::<ObjBoundMethod>(),
        Obj::Closure(c) => c.upvalues.len() * size_of::<Value>(),
        Obj::Upvalue(_) => 0,
        Obj::Native(_) => 0,
        Obj::Class(c) => {
          let c = c.borrow();
          size_of::<ObjClass>()
            + c.methods.len() * 64
            + c.field_slots.len() * 32
            + c.static_slots.len() * 32
            + c.statics.len() * size_of::<Cell<Value>>()
        },
        Obj::Instance(i) => i.fields.len() * size_of::<Cell<Value>>(),
        Obj::Module(m) => {
          let m = m.borrow();
          size_of::<ObjModule>() + m.namespace.slots.len() * (size_of::<Cell<Value>>() + 32)
        },
        Obj::ModuleBinding(_) => size_of::<ObjModuleBinding>(),
        Obj::Range { .. } => 0,
        Obj::File(_) => size_of::<FileHandle>(),
        Obj::Ptr(_) => size_of::<ObjPtr>(),
      }
  }

  /// Every new object is born here: a plain bump-push into the
  /// nursery's current chunk (a fresh one appended whenever the last
  /// one is full: see `NurseryChunk`'s own docs on why this never
  /// needs to reallocate an EXISTING chunk's buffer, so every `Value`
  /// already handed out stays valid no matter how much more gets
  /// allocated afterward). Old-generation allocation only happens
  /// indirectly, via `promote_into_old`, when a minor collection finds
  /// this object still reachable.
  fn alloc(&mut self, obj: Obj) -> Value {
    let size = Self::approx_size(&obj);
    self.alloc_sized(obj, size)
  }

  /// `alloc` for a caller that ALREADY knows the object's accounted
  /// size, skipping `approx_size`'s 17-arm match entirely.
  ///
  /// `size` must equal what `approx_size` would return for `obj` --
  /// it feeds `bytes_allocated`/`young_bytes_allocated` and therefore
  /// GC scheduling, so a wrong value skews collection timing (it is
  /// not a memory-safety input). Worth having because instance
  /// allocation, the hottest allocation site on object-oriented code,
  /// knows the answer as a plain multiply from `field_count`, while
  /// `approx_size` would re-derive it by matching the variant and
  /// re-reading the field slice's length.
  fn alloc_sized(&mut self, obj: Obj, size: usize) -> Value {
    if self.nursery_cur == self.nursery_end {
      self.refill_nursery();
    }

    // SAFETY: `refill_nursery` above guarantees `nursery_cur` now
    // points at a real, uninitialized, in-bounds slot of the active
    // chunk's buffer, and that buffer never reallocates for the
    // chunk's whole lifetime (see `NurseryChunk`'s own docs), so this
    // write cannot invalidate any pointer already handed out.
    let slot = self.nursery_cur;
    self.nursery_cur = unsafe { slot.add(1) };
    self.bytes_allocated += size;
    self.young_bytes_allocated += size;
    self.live_count += 1;
    self.update_jit_gc_needed();

    unsafe {
      std::ptr::write(
        slot,
        GcBox {
          live: Cell::new(true),
          marked: Cell::new(false),
          obj,
          generation: Cell::new(Generation::Young),
          remembered: Cell::new(false),
          list_next: Cell::new(std::ptr::null()),
          // Unused for nursery objects: see `GcBox::chunk_idx`'s own
          // docs; nothing ever looks this up for a `Young` box, since
          // nursery chunks are never individually freed/reused
          // mid-cycle.
          chunk_idx: 0,
        },
      );
      Value::obj(&(*slot).obj as *const Obj)
    }
  }

  /// `alloc`'s slow path: the active chunk is full (or there isn't one
  /// yet), so publish its final length and move the cursor to a chunk
  /// with room.
  ///
  /// Walks forward from `nursery_fill_idx` rather than jumping to
  /// `nursery_chunks.last()`: `reset_nursery` retains a batch of
  /// already-allocated, now-empty chunks (up to
  /// `MAX_RETAINED_NURSERY_CHUNKS`) for exactly this loop to bump-
  /// allocate back into with zero new `malloc` calls, and `.last()`
  /// would skip straight past all of them to allocate a brand new one,
  /// defeating the point of retaining them.
  #[cold]
  #[inline(never)]
  fn refill_nursery(&mut self) {
    self.sync_active_chunk_len();
    while self.nursery_fill_idx < self.nursery_chunks.len()
      && self.nursery_chunks[self.nursery_fill_idx].slots.len()
        >= self.nursery_chunks[self.nursery_fill_idx].slots.capacity()
    {
      self.nursery_fill_idx += 1;
    }
    if self.nursery_fill_idx >= self.nursery_chunks.len() {
      self.nursery_chunks.push(NurseryChunk {
        slots: Vec::with_capacity(CHUNK_SIZE),
      });
    }
    // Read AFTER any push above: growing the outer `Vec` relocates the
    // `NurseryChunk` headers, though never any chunk's own buffer.
    let chunk = &mut self.nursery_chunks[self.nursery_fill_idx];
    let base = chunk.slots.as_mut_ptr();
    let len = chunk.slots.len();
    let cap = chunk.slots.capacity();
    self.nursery_cur = unsafe { base.add(len) };
    self.nursery_end = unsafe { base.add(cap) };
  }

  /// Writes the active chunk's bump cursor back into its `Vec`'s own
  /// length, making `slots.len()` correct again.
  ///
  /// Must run before ANYTHING iterates `nursery_chunks`; while a
  /// chunk is active its `slots.len()` is deliberately stale and the
  /// cursor is the truth (see `nursery_cur`). Idempotent, so callers
  /// may run it defensively.
  fn sync_active_chunk_len(&mut self) {
    if self.nursery_cur.is_null() {
      return;
    }
    let chunk = &mut self.nursery_chunks[self.nursery_fill_idx];
    let base = chunk.slots.as_mut_ptr();
    // SAFETY: `nursery_cur` was derived from this same buffer by
    // `refill_nursery` and only ever bumped forward within it.
    let len = unsafe { self.nursery_cur.offset_from(base) } as usize;
    debug_assert!(len <= chunk.slots.capacity());
    unsafe { chunk.slots.set_len(len) };
  }

  /// Allocates directly into OLD-generation storage, never the
  /// nursery; for object kinds that must NEVER move, because
  /// something outside the GC's own reach caches their address as a
  /// raw pointer with no relocation hook of its own. `ObjFunction` is
  /// the one real case today: `jit::codegen` bakes a compiled
  /// function's OWN prototype address as a machine-code IMMEDIATE
  /// (see `FuncCompiler`'s use of `self.proto as *const ObjFunction`)
  ///; once that's baked into installed machine code, there is no
  /// second chance to fix it up the way a `Value` sitting in a
  /// register or object field gets fixed up by `VM::collect_minor`.
  /// Since a function prototype is compiled once (into the enclosing
  /// chunk's constant pool: see `Instr::Closure`) and referenced
  /// for the rest of the program's life, it gains nothing from young-
  /// gen's fast bump-allocate/early-death optimization anyway, so
  /// skipping the nursery entirely costs nothing real.
  ///
  /// Unconditionally write-barriers the object it just created before
  /// returning it; NOT an optional safety margin. The usual "a
  /// freshly allocated object never needs a barrier for its own
  /// construction-time writes" exemption (see `write_barrier`'s own
  /// docs) relies specifically on the object being born Young, so
  /// that ANY of its fields still pointing at other young objects get
  /// found for free by the very next minor collection's normal
  /// root+worklist walk. An object born straight into the old
  /// generation gets NO such free pass: nothing walks an old object's
  /// children again after its own one-time promotion visit unless
  /// it's in the remembered set, and something created here via
  /// `alloc_old` was never promoted at all, so it never had that
  /// visit in the first place. A field set at construction time to a
  /// still-young value (`ObjFunction::globals_module`, pointing at a
  /// module that's often still Young at compile time, is the
  /// confirmed real case) would otherwise never get discovered and
  /// relocated at all.
  /// Queuing it into the remembered set right away is what gives it
  /// the SAME guarantee a young-born object gets automatically: its
  /// children get walked (via the remembered-set scan this time,
  /// instead of the promotion worklist) the very next minor collection.
  pub fn alloc_old(&mut self, obj: Obj) -> Value {
    let size = Self::approx_size(&obj);
    self.bytes_allocated += size;
    self.live_count += 1;
    self.update_jit_gc_needed();
    let gcbox_ptr = self.promote_into_old(obj);
    let obj_ptr = unsafe { &(*gcbox_ptr).obj as *const Obj };
    write_barrier(obj_ptr);
    Value::obj(obj_ptr)
  }

  /// Moves an already-computed `(size, obj)` pair; the payload of a
  /// young object a minor collection just found still reachable --
  /// directly into old-generation chunk storage. Exactly the
  /// candidate/free/new-chunk search `alloc` used to run for every
  /// object back when young and old shared the same storage, just
  /// parameterized on an already-known size/payload instead of
  /// computing them fresh, and tagging the result `Old` immediately
  /// rather than `Young`; a promoted object is never "young" for
  /// even one instant, since it only exists because it already
  /// survived a full minor collection.
  ///
  /// Deliberately does NOT touch `bytes_allocated`/`live_count`: this
  /// object was already counted once, when `alloc` first created it
  /// in the nursery: relocating it during a collection isn't a new
  /// allocation event, so counting it again here would double-count
  /// every survivor.
  ///
  /// Returns the new `GcBox`'s address; for the caller
  /// (`forward_or_promote`) to record as this object's forwarding
  /// target and to walk its children from.
  fn promote_into_old(&mut self, obj: Obj) -> *const GcBox {
    loop {
      let Some(&idx) = self.candidates.last() else {
        break;
      };
      let Some(chunk) = self.chunks[idx].as_mut() else {
        self.candidates.pop();
        continue;
      };

      if let Some(ptr) = chunk.free.pop() {
        chunk.live_count += 1;
        // SAFETY: `ptr` was pushed by `sweep` only after this slot was
        // confirmed unreachable and its old contents dropped; nothing
        // else references it, so exclusive access here is sound.
        unsafe {
          (*ptr).live.set(true);
          (*ptr).marked.set(false);
          (*ptr).obj = obj;
          (*ptr).generation.set(Generation::Old);
          (*ptr).remembered.set(false);
          (*ptr).chunk_idx = idx as u32;
          (*ptr).list_next.set(std::ptr::null());
        }
        return ptr;
      }

      if chunk.slots.len() < chunk.slots.capacity() {
        chunk.slots.push(GcBox {
          live: Cell::new(true),
          marked: Cell::new(false),
          obj,
          generation: Cell::new(Generation::Old),
          remembered: Cell::new(false),
          list_next: Cell::new(std::ptr::null()),
          chunk_idx: idx as u32,
        });
        chunk.live_count += 1;
        return chunk.slots.last().unwrap();
      }

      // Neither a free slot nor spare capacity left; stale candidate.
      self.candidates.pop();
    }

    // No usable candidate; start a fresh chunk.
    let idx = self.chunks.len();
    let mut chunk = GcChunk {
      slots: Vec::with_capacity(CHUNK_SIZE),
      free: Vec::new(),
      live_count: 1,
    };
    chunk.slots.push(GcBox {
      live: Cell::new(true),
      marked: Cell::new(false),
      obj,
      generation: Cell::new(Generation::Old),
      remembered: Cell::new(false),
      list_next: Cell::new(std::ptr::null()),
      chunk_idx: idx as u32,
    });
    self.chunks.push(Some(chunk));
    let gcbox_ptr: *const GcBox = self.chunks[idx].as_ref().unwrap().slots.last().unwrap();
    self.candidates.push(idx);
    gcbox_ptr
  }

  /// Resolves ONE possibly-young pointer during a minor collection's
  /// copy phase: if `ptr` isn't currently `Young`, it's already old
  /// (or was already forwarded earlier THIS SAME cycle: see below)
  /// and needs no relocation, so it's returned unchanged. Otherwise:
  ///
  /// - If this exact object was already forwarded earlier in this
  ///   cycle (`marked`; repurposed here as "already forwarded", not
  ///   its usual mark-sweep meaning: see `GcBox::marked`'s sibling
  ///   docs), `list_next` (repurposed the same way as the forwarding
  ///   target) already holds its new address; every reference to the
  ///   SAME object converges on the SAME new copy this way, which is
  ///   what keeps pointer-identity semantics (`Value::equals`'s
  ///   pointer-equality cases) correct across a collection.
  /// - Otherwise, this is the FIRST reference to it found this cycle:
  ///   move its `Obj` payload out (a real, ownership-transferring
  ///   move via `ptr::read`; copying the bytes would leave the
  ///   nursery slot and the new slot both "owning" e.g. the same
  ///   `String`'s heap buffer, a double-free waiting to happen),
  ///   promote it into old-gen storage, mark the OLD slot forwarded,
  ///   and queue the new copy for `collect_minor`'s own worklist to
  ///   walk its children next.
  pub(crate) fn forward_or_promote(
    &mut self,
    ptr: *const Obj,
    worklist: &mut Vec<*const Obj>,
  ) -> *const Obj {
    let gcbox_ptr = Self::gcbox_of(ptr) as *mut GcBox;
    let gcbox = unsafe { &*gcbox_ptr };
    if gcbox.generation.get() != Generation::Young {
      return ptr;
    }
    // A nursery slot `reset_nursery` has already finished with. Every
    // slot it walks is marked dead, whether its payload was reclaimed
    // (gone) or moved out by a forwarding promotion (a stub whose
    // `list_next` may since have been swept and recycled). Neither is
    // safe to follow, and nothing overwrote the box to say so:
    // `generation` still reads `Young`, so without this the reclaimed
    // case would promote a corpse into the old generation for
    // `Heap::sweep` to drop a second time -- a straight double free --
    // and the forwarded case would hand back a stale target for
    // `walk_children_mut` to read.
    //
    // A register outside the top frame's window is how one gets here.
    // `collect_minor` bounds its root scan to that window, so when a
    // callee whose registers reach past its caller's returns, the slots
    // above the caller's window stop being roots and whatever they name
    // dies; their BITS stay put, and a later, deeper call brings those
    // same slots back inside the window with the dead pointer still in
    // them. Nothing reads such a register (the compiler only ever reads
    // a register it has defined), so leaving the pointer alone is
    // exactly right: it is dead, and the collector's job here is simply
    // not to mistake it for something worth resurrecting.
    if !gcbox.live.get() {
      return ptr;
    }
    if gcbox.marked.get() {
      let new_gcbox = gcbox.list_next.get();
      return unsafe { &(*new_gcbox).obj };
    }
    // SAFETY: not yet forwarded (checked above), so `obj` hasn't been
    // read out yet; this takes ownership exactly once. Every future
    // reference to this SAME nursery slot takes the `marked` branch
    // above instead of reaching this read again.
    let moved = unsafe { std::ptr::read(&gcbox.obj) };
    // `ptr::read` copies the bytes out but does NOT erase the source
    //; without overwriting it right now, this slot would still look
    // like a perfectly valid `Obj` sharing ownership of the SAME
    // Rust-level allocations (a `String`'s buffer, a `Vec<Value>`'s
    // backing store, ...) the new promoted copy now legitimately owns.
    // `reset_nursery` skips `marked` slots when IT runs, but this
    // object might never see another minor collection before the
    // program exits (e.g. `VM::ensure_stable_for_compiled_entry`
    // promoting something out of cycle, in a short-lived program) --
    // at which point `Heap`'s own ordinary drop glue would run over
    // this slot with no idea it's already been moved from, a genuine
    // double-free. Writing a value that owns nothing here is what
    // makes EITHER of those eventual drops safe regardless of when
    // (or whether) they happen.
    // SAFETY: writes through a raw pointer derived straight from
    // `gcbox_ptr`, never through the shared `gcbox` reference above
    // (casting `&T` to `*mut T` and writing through THAT is UB even
    // when nothing else is aliasing it); `gcbox_ptr` itself is sound
    // to write through for the same reason every other GC-internal
    // mutation in this file is: collection-time access to a `GcBox` is
    // always effectively exclusive.
    unsafe {
      std::ptr::write(
        &mut (*gcbox_ptr).obj,
        Obj::Range {
          lower: 0.0,
          upper: 0.0,
          step: Cell::new(1.0),
        },
      );
    }
    let new_gcbox = self.promote_into_old(moved);
    gcbox.marked.set(true);
    gcbox.list_next.set(new_gcbox);
    worklist.push(unsafe { &(*new_gcbox).obj });
    unsafe { &(*new_gcbox).obj }
  }

  /// Shared cleanup for one dead `Obj` found during `reset_nursery`'s
  /// scan. `FieldStorage`'s backing allocation is recycled into `pool`
  /// instead of freed (see `field_storage_pool`'s own docs); every
  /// other `Obj` variant is dropped exactly as `drop_in_place` used to
  /// drop it.
  ///
  /// Free-standing (takes `pool` explicitly rather than `&mut self`)
  /// so `reset_nursery` can call it from inside a loop that's already
  /// borrowing a DIFFERENT field of `self` (`nursery_chunks`); the
  /// same disjoint-field-borrow pattern `VM::forward_slot` uses, and
  /// for the identical reason (see that function's own docs): a
  /// `&mut self` method here would make the borrow checker treat it as
  /// touching all of `self`, conflicting with the loop's own borrow
  /// even though the two never actually overlap.
  fn reclaim_dead_obj(pool: &mut FieldStoragePool, obj: Obj, pool_budget: usize) {
    match obj {
      Obj::Instance(instance) => {
        let (ptr, len) = instance.fields.into_raw_parts();
        if !ptr.is_null() {
          for i in 0..len {
            unsafe { (*ptr.add(i)).set(Value::nil()) };
          }
          if !pool.give(ptr, len, pool_budget) {
            drop(unsafe { FieldStorage::from_raw_parts(ptr, len) });
          }
        }
      },
      other => drop(other),
    }
  }

  /// How many bytes of recycled field storage this heap will hold on
  /// to: see `FIELD_STORAGE_POOL_BUDGET_FACTOR`.
  #[inline]
  fn field_storage_pool_budget(&self) -> usize {
    self.young_next_gc * FIELD_STORAGE_POOL_BUDGET_FACTOR
  }

  /// Reclaims the nursery after a minor collection's copy phase has
  /// fully drained its worklist: by construction, every slot NOT
  /// forwarded this cycle (`marked == false`) is garbage; nothing
  /// still reachable can point at it, since `collect_minor` visited
  /// every root and every live object's children before calling this.
  /// Its `Obj` payload (and whatever it owns; a `String`'s buffer, a
  /// `List`'s backing `SmallVec`, ...) is dropped in place via
  /// `reclaim_dead_obj`, exactly what `sweep`/the old `sweep_young`
  /// used to do for a dead slot. A forwarded slot's `obj` was already
  /// MOVED OUT via `ptr::read` in `forward_or_promote`; dropping it
  /// again here would be a double-free, which is exactly what `marked`
  /// (this cycle's forwarding flag) exists to distinguish.
  ///
  /// After every slot in a chunk is handled, `set_len(0)` reclaims
  /// that chunk's WHOLE buffer for the next cycle's allocations in
  /// one step, without running `Vec`'s own per-element `Drop` glue a
  /// second time over slots this function already handled by hand.
  /// Every chunk up to `MAX_RETAINED_NURSERY_CHUNKS` is kept around
  /// (emptied, not dropped) for the next cycle to bump-allocate into
  /// with zero further allocator calls; unlike the old generation's
  /// `sweep`, which genuinely wants to return a fully-empty chunk's
  /// memory (old objects can live indefinitely, so an idle old chunk
  /// is likely to stay idle), the nursery refills every single minor
  /// collection by design, so a chunk it just emptied is overwhelmingly
  /// likely to be needed again within the next `YOUNG_NEXT_GC` bytes
  /// of allocation. Only genuinely excess chunks (beyond the cap, from
  /// an unusually large one-off burst) get dropped, returning their
  /// memory instead of holding it as permanent inventory forever.
  pub(crate) fn reset_nursery(&mut self) {
    // The active chunk's `slots.len()` is stale by design while it is
    // being bump-allocated into; publish it before iterating, or
    // every slot allocated since the last refill is invisible here and
    // silently leaks its payload instead of being reclaimed.
    self.sync_active_chunk_len();
    let pool_budget = self.field_storage_pool_budget();
    let mut freed_count = 0usize;
    let mut freed_bytes = 0usize;
    for chunk in self.nursery_chunks.iter_mut() {
      for gcbox in chunk.slots.iter_mut() {
        // Dead either way by the time this loop is done with it: a
        // forwarded slot is a stub whose payload now lives in the old
        // generation, and an unmarked one is reclaimed just below.
        // Recording that matters because nothing else does: the slot's
        // BYTES outlive it until something allocates over them, and
        // `generation` goes on reading `Young` the whole time. See
        // `forward_or_promote`'s liveness guard for what reads this.
        gcbox.live.set(false);
        if !gcbox.marked.get() {
          if let Obj::Instance(instance) = &gcbox.obj
            && instance.fields.is_inline()
          {
            freed_bytes += std::mem::size_of::<Obj>()
              + (instance.fields.len as usize) * std::mem::size_of::<Cell<Value>>();
            freed_count += 1;
            continue;
          }
          freed_bytes += Self::approx_size(&gcbox.obj);
          // SAFETY: never forwarded (checked above), so `obj` was
          // never moved out before now; this is its one and only
          // move, mirroring `forward_or_promote`'s own `ptr::read` for
          // the forwarded case. `chunk.slots.set_len(0)` below never
          // runs any destructor over this slot again either way.
          let obj = unsafe { std::ptr::read(&gcbox.obj) };
          Self::reclaim_dead_obj(&mut self.field_storage_pool, obj, pool_budget);
          freed_count += 1;
        }
      }
      // SAFETY: every slot in this chunk has either been moved out
      // (forwarded) or dropped in place (above); none of them owns
      // anything that still needs cleanup, so shrinking the logical
      // length to 0 without running element destructors a second
      // time is exactly right.
      unsafe { chunk.slots.set_len(0) };
    }
    self
      .nursery_chunks
      .truncate(Self::MAX_RETAINED_NURSERY_CHUNKS.max(1));
    self.nursery_fill_idx = 0;
    // Both null so the next `alloc`'s `cur == end` check routes
    // straight to `refill_nursery`, which re-derives them against
    // whichever chunks survived the truncate above. Leaving stale
    // pointers here would bump into a freed chunk's buffer.
    self.nursery_cur = std::ptr::null_mut();
    self.nursery_end = std::ptr::null_mut();
    self.bytes_allocated = self.bytes_allocated.saturating_sub(freed_bytes);
    self.live_count = self.live_count.saturating_sub(freed_count);
    self.young_bytes_allocated = 0;
  }

  pub fn alloc_string(&mut self, s: impl Into<String>) -> Value {
    self.alloc(Obj::Str(s.into(), Cell::new(ASCII_UNKNOWN)))
  }

  /// Deliberately `alloc_old`, not `alloc`; for `Compiler`'s own
  /// use building a chunk's CONSTANT POOL (method/field/class names,
  /// string literals, ...) specifically, never for runtime string
  /// creation. See `alloc_old`'s own docs: `jit::codegen::bake_const`
  /// bakes a constant's raw bits as a machine-code immediate, so
  /// anything that can end up in `Chunk::constants` needs the exact
  /// same "never moves" guarantee `ObjFunction` needs, for the exact
  /// same reason; and gains nothing from the nursery either way,
  /// since a constant pool entry lives exactly as long as its chunk.
  /// Allocates a COMPILE-TIME string constant, interned: two identical
  /// literals anywhere in the program (or across separately compiled
  /// modules) become one shared heap object.
  ///
  /// This is the funnel every constant-pool string goes through --
  /// literals, method names, field names, class names; so a name like
  /// `x` used at fifty sites costs one allocation instead of fifty, and
  /// the constant pool's total footprint becomes the program's set of
  /// DISTINCT strings rather than its count of string occurrences.
  ///
  /// Deliberately NOT extended to runtime-created strings. Interning
  /// pays only when strings repeat; for the string-BUILDING pattern
  /// (`b += c` in a loop) every intermediate is unique and short-lived,
  /// so hashing each one to find no match would be pure cost, and the
  /// table would grow without bound.
  ///
  /// Sharing is unobservable through the language's own semantics --
  /// strings are immutable and equality is by content, with no identity
  /// operator. The one place it shows is `natives::id`, which exposes
  /// raw addresses: `id` of two equal literals now matches. That was
  /// never a guarantee to break, since a string's address already
  /// changes when a minor collection promotes it.
  pub fn alloc_string_old(&mut self, s: impl Into<String>) -> Value {
    let s = s.into();
    if let Some(&existing) = self.interned_strings.get(s.as_str()) {
      return existing;
    }
    let key: Box<str> = s.as_str().into();
    let value = self.alloc_old(Obj::Str(s, Cell::new(ASCII_UNKNOWN)));
    self.interned_strings.insert(key, value);
    value
  }

  /// Every interned compile-time string, for the major collector's root
  /// scan: see `alloc_string_old`. Without this the sweep would free
  /// strings the table still points at.
  pub(crate) fn interned_strings(&self) -> impl Iterator<Item = Value> + '_ {
    self.interned_strings.values().copied()
  }

  pub fn alloc_bytes(&mut self, b: impl Into<Vec<u8>>) -> Value {
    self.alloc(Obj::Bytes(RefCell::new(b.into())))
  }

  pub fn alloc_bigint(&mut self, s: impl Into<BigInt>) -> Value {
    self.alloc(Obj::BigInt(s.into()))
  }

  /// `alloc_bigint`'s `alloc_old` counterpart: see
  /// `alloc_string_old`'s own docs; a `BigInt` literal can ALSO end
  /// up baked as a constant-pool immediate the same way a string
  /// literal can.
  pub fn alloc_bigint_old(&mut self, s: impl Into<BigInt>) -> Value {
    self.alloc_old(Obj::BigInt(s.into()))
  }

  pub fn alloc_list(&mut self, list: impl Into<ListStorage>) -> Value {
    self.alloc(Obj::List(RefCell::new(list.into())))
  }

  /// Builds a Dict from raw (key, value) pairs, de-duplicating by
  /// VALUE equality (not pointer identity; two distinct string
  /// objects with the same text collide, matching every other
  /// language's dict-literal semantics), keeping the LAST occurrence
  /// of any repeated key.
  pub fn alloc_dict(&mut self, pairs: Vec<(Value, Value)>) -> Value {
    let storage = DictStorage::from_pairs(pairs);
    self.alloc(Obj::Dict(Box::new(RefCell::new(storage))))
  }

  /// Deliberately `alloc_old`, not `alloc`: see `alloc_old`'s own
  /// docs on why a function prototype must never be young.
  pub fn alloc_function(&mut self, f: ObjFunction) -> Value {
    self.alloc_old(Obj::Func(Box::new(f)))
  }

  /// Allocates a class METHOD's closure straight into the old
  /// generation, and every other closure into the nursery as usual.
  ///
  /// Same argument as `alloc_class`, one level down. A method closure
  /// is created once, by the `Instr::Closure`/`Instr::SetMethod` pair
  /// that runs when its class is declared, and then lives in that
  /// class's method table for as long as the class does; so the
  /// nursery never buys it anything, while its address moving out from
  /// under a compile-time-baked guard costs real performance silently
  /// (see `alloc_class`'s own docs for how that failure mode looks).
  ///
  /// Concretely, this is what makes `jit::CallTarget::ConstructKnown`
  /// able to bake a constructor's `Value` bits at all: with the
  /// closure immovable, `zuri_jit_construct_prepare` needs neither the
  /// `ObjClass` read that finds `constructor` nor the
  /// `ensure_stable_for_compiled_entry` generation probe; two
  /// dependent, cache-missing loads on every single instance built.
  ///
  /// Ordinary closures stay young deliberately: those genuinely are
  /// short-lived (a callback built inside a loop is the common case),
  /// which is exactly what the nursery is for.
  pub fn alloc_closure(&mut self, c: ObjClosure) -> Value {
    if c.function.as_func().owning_class_name.is_some() {
      return self.alloc_old(Obj::Closure(c));
    }
    self.alloc(Obj::Closure(c))
  }

  pub fn alloc_upvalue(&mut self, state: UpvalueState) -> Value {
    self.alloc(Obj::Upvalue(Cell::new(state)))
  }

  pub fn alloc_bound_method(&mut self, receiver: Value, method: Value) -> Value {
    self.alloc(Obj::BoundMethod(ObjBoundMethod { receiver, method }))
  }

  /// Convenience for a function that captures nothing (the common case:
  /// every top-level function, and any nested function that happens not
  /// to reference an enclosing local); allocates the prototype and
  /// wraps it in a trivial empty-upvalue closure in one step.
  pub fn alloc_plain_closure(&mut self, f: ObjFunction) -> Value {
    let proto_val = self.alloc_function(f);
    self.alloc_closure(ObjClosure {
      function: proto_val,
      upvalues: SmallVec::new(),
    })
  }

  /// Ordinary young allocation, deliberately NOT `alloc_old`.
  ///
  /// Moving natives to the old generation looks harmless; they're
  /// registered once at startup and live for the whole program; but
  /// `alloc_old` runs a `write_barrier`, joining every native to the
  /// remembered set immediately. `sweep` frees old boxes (and drops
  /// whole chunks) without unlinking them from that intrusive list
  /// threaded through `GcBox::list_next`, and `promote_into_old` can
  /// then recycle such a slot and truncate the chain; a real
  /// use-after-free, not a theoretical one.
  ///
  /// `jit::codegen` therefore may NOT bake a native's address as a
  /// call-site guard, since a young native relocates on its first minor
  /// collection; it guards on the `NativeFn` pointer instead, which
  /// relocation cannot change. See `obj_native_func_offset`.
  pub fn alloc_native(&mut self, native: NativeFunction) -> Value {
    self.alloc(Obj::Native(native))
  }

  /// Deliberately `alloc_old`, not `alloc`; the same decision, for
  /// the same reason, as `alloc_function` right above.
  ///
  /// A class is a declaration-time entity: one is created per `class`
  /// statement, never in a loop, and it stays reachable from every
  /// instance's own `class` field for as long as any instance lives.
  /// Born in the nursery it would survive every minor collection
  /// anyway, so the only thing the young generation ever did for it
  /// was copy it once; and then RELOCATE it, which is the part that
  /// actively broke things.
  ///
  /// Relocation is what makes this load-bearing rather than a micro-
  /// optimization. `jit::CallTarget` and `jit::CompileFacts
  /// ::self_class_bits` bake a class's `Value` bits into generated
  /// code as a compile-time guard immediate. A function crosses its
  /// warmup threshold long before the first minor collection has had
  /// any reason to run, so those bits were routinely taken while the
  /// class was still young; and the moment it was promoted, every
  /// guard baked against it stopped matching FOREVER. The optimization
  /// did not misbehave, it silently stopped existing, permanently
  /// falling back to the general path with no signal that anything was
  /// wrong. Allocating old makes the address stable for the process's
  /// lifetime, which is what those guards always assumed.
  ///
  /// Safe with respect to the generational invariant because every
  /// mutation that stores a `Value` into a class; `SetFieldInit`,
  /// `SetMethod`, `DeclareStatic`, `FinalizeClass`, in both the
  /// interpreter and `jit::runtime`; already ends in a
  /// `write_barrier` on the class, so old-to-young references out of
  /// one are tracked by the remembered set exactly as they must be.
  /// (`DeclareField` needs none: it stores only a name and a `u16`
  /// slot index, never a `Value`.)
  pub fn alloc_class(&mut self, class: ObjClass) -> Value {
    self.alloc_old(Obj::Class(Box::new(RefCell::new(class))))
  }

  pub fn alloc_instance(&mut self, class: Value, field_count: usize) -> Value {
    let fields = self.take_field_storage(field_count);
    let size = std::mem::size_of::<Obj>() + field_count * size_of::<Cell<Value>>();
    self.alloc_sized(Obj::Instance(ObjInstance { class, fields }), size)
  }

  #[inline(always)]
  pub fn alloc_instance_fast_0(&mut self, class: Value) -> Value {
    if self.nursery_cur == self.nursery_end {
      self.refill_nursery();
    }
    let slot = self.nursery_cur;
    self.nursery_cur = unsafe { slot.add(1) };
    let size = std::mem::size_of::<Obj>();
    self.bytes_allocated += size;
    self.young_bytes_allocated += size;
    self.live_count += 1;
    self.update_jit_gc_needed();

    let fields = FieldStorage {
      ptr: std::ptr::null_mut(),
      len: 0,
      inline: [Cell::new(Value::nil()), Cell::new(Value::nil())],
    };
    unsafe {
      std::ptr::write(
        slot,
        GcBox {
          live: Cell::new(true),
          marked: Cell::new(false),
          obj: Obj::Instance(ObjInstance { class, fields }),
          generation: Cell::new(Generation::Young),
          remembered: Cell::new(false),
          list_next: Cell::new(std::ptr::null()),
          chunk_idx: 0,
        },
      );
      Value::obj(&(*slot).obj as *const Obj)
    }
  }

  #[inline(always)]
  pub fn alloc_instance_fast_1(&mut self, class: Value, f0: Value) -> Value {
    if self.nursery_cur == self.nursery_end {
      self.refill_nursery();
    }
    let slot = self.nursery_cur;
    self.nursery_cur = unsafe { slot.add(1) };
    let size = std::mem::size_of::<Obj>() + std::mem::size_of::<Cell<Value>>();
    self.bytes_allocated += size;
    self.young_bytes_allocated += size;
    self.live_count += 1;
    self.update_jit_gc_needed();

    let fields = FieldStorage {
      ptr: std::ptr::null_mut(),
      len: 1,
      inline: [Cell::new(f0), Cell::new(Value::nil())],
    };
    unsafe {
      std::ptr::write(
        slot,
        GcBox {
          live: Cell::new(true),
          marked: Cell::new(false),
          obj: Obj::Instance(ObjInstance { class, fields }),
          generation: Cell::new(Generation::Young),
          remembered: Cell::new(false),
          list_next: Cell::new(std::ptr::null()),
          chunk_idx: 0,
        },
      );
      Value::obj(&(*slot).obj as *const Obj)
    }
  }

  #[inline(always)]
  pub fn alloc_instance_fast_2(&mut self, class: Value, f0: Value, f1: Value) -> Value {
    if self.nursery_cur == self.nursery_end {
      self.refill_nursery();
    }
    let slot = self.nursery_cur;
    self.nursery_cur = unsafe { slot.add(1) };
    let size = std::mem::size_of::<Obj>() + 2 * std::mem::size_of::<Cell<Value>>();
    self.bytes_allocated += size;
    self.young_bytes_allocated += size;
    self.live_count += 1;
    self.update_jit_gc_needed();

    let fields = FieldStorage {
      ptr: std::ptr::null_mut(),
      len: 2,
      inline: [Cell::new(f0), Cell::new(f1)],
    };
    unsafe {
      std::ptr::write(
        slot,
        GcBox {
          live: Cell::new(true),
          marked: Cell::new(false),
          obj: Obj::Instance(ObjInstance { class, fields }),
          generation: Cell::new(Generation::Young),
          remembered: Cell::new(false),
          list_next: Cell::new(std::ptr::null()),
          chunk_idx: 0,
        },
      );
      Value::obj(&(*slot).obj as *const Obj)
    }
  }

  /// Pops a recycled buffer of exactly `len` cells off
  /// `field_storage_pool` if one's available, otherwise falls back to a
  /// real allocation: see `field_storage_pool`'s own docs. Every
  /// pooled buffer was reset to all-`Value::nil()` before being pushed
  /// (`reset_nursery`), so this upholds `FieldStorage::new`'s exact
  /// postcondition either way.
  fn take_field_storage(&mut self, len: usize) -> FieldStorage {
    if len <= INLINE_FIELDS {
      return FieldStorage::new(len);
    }
    if let Some(ptr) = self.field_storage_pool.take(len) {
      return unsafe { FieldStorage::from_raw_parts(ptr, len) };
    }
    FieldStorage::new(len)
  }

  pub fn alloc_range(&mut self, lower: f64, upper: f64) -> Value {
    self.alloc(Obj::Range {
      lower,
      upper,
      step: Cell::new(1.0),
    })
  }

  pub fn alloc_file(&mut self, fh: FileHandle) -> Value {
    self.alloc(Obj::File(Box::new(RefCell::new(fh))))
  }

  pub fn alloc_module(&mut self, m: ObjModule) -> Value {
    self.alloc(Obj::Module(Box::new(RefCell::new(m))))
  }

  pub fn alloc_module_binding(&mut self, b: ObjModuleBinding) -> Value {
    self.alloc(Obj::ModuleBinding(Box::new(b)))
  }

  pub fn alloc_ptr<T: 'static + Send>(&mut self, type_name: &'static str, value: T) -> Value {
    self.alloc(Obj::Ptr(RefCell::new(ObjPtr {
      type_name,
      value: Box::new(value),
    })))
  }

  /// Like `alloc_ptr`, for a payload that's already been through
  /// `ObjPtr::take` on some OTHER heap (possibly on another thread --
  /// that's the entire point) and is now just a type-erased box with
  /// no concrete `T` left to name. Used only by
  /// `modules::isolate_util::transfer`, rebuilding a moved `Ptr` on
  /// a destination isolate.
  pub fn alloc_ptr_boxed(&mut self, type_name: &'static str, value: Box<dyn Any + Send>) -> Value {
    self.alloc(Obj::Ptr(RefCell::new(ObjPtr { type_name, value })))
  }

  /// Drop every object whose address isn't in `reachable`, then
  /// recompute `bytes_allocated` and re-arm `next_gc` off the resulting
  /// live size. Returns how many objects were freed.
  ///
  /// This only performs the sweep half of mark-and-sweep; `reachable`
  /// must already be the complete, transitively-closed set of live
  /// objects (see `VM::collect_garbage`), or anything missing from it
  /// gets freed out from under whatever still references it.
  ///
  /// Visits every resident chunk. A slot that's live but wasn't marked
  /// this cycle is garbage: its contents are dropped in place and it
  /// goes on its chunk's local free list. A chunk whose live_count hits
  /// zero; every slot in it dead; is dropped ENTIRELY, returning its
  /// backing allocation (and, for a block this size, typically the
  /// underlying pages) to the allocator instead of holding it as
  /// permanent inventory.
  ///
  /// This is a full (major) sweep. `VM::collect_garbage` always flushes
  /// the nursery first (via `VM::collect_minor`), so every live slot
  /// found here is definitionally `Old`.
  pub fn sweep(&mut self) -> usize {
    let pool_budget = self.field_storage_pool_budget();
    let mut freed = 0;

    for idx in 0..self.chunks.len() {
      let live_count_after;
      let became_reusable;

      {
        let Some(chunk) = self.chunks[idx].as_mut() else {
          continue;
        };
        let had_capacity = !chunk.free.is_empty() || chunk.slots.len() < chunk.slots.capacity();

        for gcbox in chunk.slots.iter_mut() {
          if !gcbox.live.get() {
            continue;
          }
          if gcbox.marked.get() {
            gcbox.marked.set(false);
          } else {
            self.bytes_allocated = self
              .bytes_allocated
              .saturating_sub(Self::approx_size(&gcbox.obj));
            // Route through the same FieldStorage-recycling path
            // `reset_nursery` uses instead of a plain assignment-drop:
            // most of a binary-tree-shaped workload's garbage survives
            // long enough to be promoted (see `reclaim_dead_obj`'s own
            // docs), so THIS is where the majority of its FieldStorage
            // actually dies; skipping the pool here left it fed
            // almost exclusively by the rare object that dies still in
            // the nursery.
            let obj = std::mem::replace(
              &mut gcbox.obj,
              Obj::Range {
                lower: 0.0,
                upper: 0.0,
                step: Cell::new(1.0),
              },
            );
            Self::reclaim_dead_obj(&mut self.field_storage_pool, obj, pool_budget);
            gcbox.live.set(false);
            chunk.live_count -= 1;
            chunk.free.push(gcbox as *mut GcBox);
            freed += 1;
          }
        }

        live_count_after = chunk.live_count;
        became_reusable = !had_capacity && !chunk.free.is_empty();
      }

      if live_count_after == 0 {
        self.chunks[idx] = None; // whole chunk reclaimed here
      } else if became_reusable {
        self.candidates.push(idx);
      }
    }

    self.live_count -= freed;
    // Re-armed against the surviving OLD set specifically; the same
    // quantity `needs_major_gc` now tests, so `GC_HEAP_GROW_FACTOR`
    // means what it says: allow the long-lived set to grow by half
    // again before paying for another full collection.
    self.next_gc = ((self.old_bytes_allocated() as f32 * Self::GC_HEAP_GROW_FACTOR) as usize)
      .max(Self::MIN_NEXT_GC);
    freed
  }

  /// Drains the remembered set, clearing each entry's `remembered`
  /// flag (so a future write to the same object properly re-queues
  /// it) and returning the objects that were in it, for a minor
  /// collection to walk as extra mark roots. A major collection also
  /// calls this, discarding the result, purely to reset the flags --
  /// a full scan makes every existing entry redundant, and leaving the
  /// flags set without the list behind them would silently stop a
  /// future write from ever re-queuing that object.
  pub(crate) fn drain_remembered(&self) -> Vec<*const Obj> {
    let mut out = Vec::new();
    REMEMBERED_HEAD.with(|head| {
      let mut ptr = head.replace(std::ptr::null());
      while !ptr.is_null() {
        let gcbox = unsafe { &*ptr };
        gcbox.remembered.set(false);
        out.push(&gcbox.obj as *const Obj);
        ptr = gcbox.list_next.get();
      }
    });
    out
  }
}

#[cfg(test)]
mod field_storage_tests {
  use super::*;

  /// `jit::codegen`'s eventual fast path needs these offsets to be
  /// exactly what `#[repr(C)]` promises; verify it directly rather
  /// than trusting the derivation.
  #[test]
  fn offsets_match_repr_c_expectations() {
    assert_eq!(std::mem::offset_of!(FieldStorage, ptr), 0);
    assert_eq!(std::mem::offset_of!(FieldStorage, len), 8);
    assert_eq!(std::mem::offset_of!(FieldStorage, inline), 16);
  }

  #[test]
  fn read_write_roundtrip_heap() {
    let mut fs = FieldStorage::new(4);
    assert_eq!(fs.len(), 4);
    for c in fs.iter() {
      assert!(c.get().is_nil());
    }
    fs[0].set(Value::number(42.0));
    fs[3].set(Value::number(7.0));
    assert_eq!(fs[0].get().as_number(), 42.0);
    assert!(fs[1].get().is_nil());
    assert!(fs[2].get().is_nil());
    assert_eq!(fs[3].get().as_number(), 7.0);

    let mut count = 0;
    for c in fs.iter_mut() {
      *c.get_mut() = Value::number(count as f64);
      count += 1;
    }
    assert_eq!(fs[2].get().as_number(), 2.0);
  }

  #[test]
  fn read_write_roundtrip_inline() {
    let fs = FieldStorage::new(2);
    assert_eq!(fs.len(), 2);
    for c in fs.iter() {
      assert!(c.get().is_nil());
    }
    fs[0].set(Value::number(10.0));
    fs[1].set(Value::number(20.0));
    assert_eq!(fs[0].get().as_number(), 10.0);
    assert_eq!(fs[1].get().as_number(), 20.0);
  }

  #[test]
  fn zero_length_is_safe() {
    let fs = FieldStorage::new(0);
    assert_eq!(fs.len(), 0);
    assert!(fs.iter().next().is_none());
  }

  /// Repeated alloc/drop in a loop is the closest a unit test gets to
  /// proving the `Drop` impl neither leaks nor double-frees without
  /// reaching for a whole separate tool; ASAN/valgrind is the real
  /// verification, run separately, but this at least exercises the
  /// exact `Box::into_raw`/`Box::from_raw` round trip many times over
  /// varied sizes.
  #[test]
  fn many_alloc_drop_cycles() {
    for n in 0..200 {
      let fs = FieldStorage::new(n % 17);
      drop(fs);
    }
  }
}
