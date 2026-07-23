use num_bigint::BigInt;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::rc::Rc;

use crate::vm::chunk::Chunk;
use crate::vm::value::Value;
use crate::vm::vm::VM;

/// Number of object slots per chunk. Chosen so a chunk is a few hundred
/// KB -- large enough to amortize the cost of growing, small enough that
/// early GC cycles don't need to wait for one huge chunk to fill before
/// the free list has anything to give back.
const CHUNK_SIZE: usize = 8192;

/// Everything a Value's pointer tag can point at.
pub enum Obj {
  Str(String),
  Bytes(RefCell<Vec<u8>>),
  BigInt(BigInt),
  /// A dynamically-sized list.
  List(RefCell<Vec<Value>>),
  /// A dict literal's storage.
  Dict(RefCell<DictStorage>),
  /// A function PROTOTYPE -- the static, compiled-once result of one
  /// `function` declaration or literal. Shared by every closure ever
  /// created from it; holds no per-call-site state itself.
  Func(Box<ObjFunction>),
  BoundMethod(ObjBoundMethod),
  /// A function VALUE at runtime -- a prototype plus the specific
  /// upvalues captured at the moment this particular closure was
  /// created. Every callable Value is one of these, even a top-level
  /// function that captures nothing (its `upvalues` is just empty).
  Closure(ObjClosure),
  /// A captured variable. Starts Open, pointing at a live register in
  /// some still-executing frame -- reads/writes through the upvalue and
  /// through the original local are the same memory. Closed when that
  /// frame's register would otherwise become invalid (block exit or
  /// function return): the current value is copied out, and the upvalue
  /// owns it from then on.
  Upvalue(Cell<UpvalueState>),
  /// A native (Rust-implemented) function -- callable through the exact
  /// same Instr::Call path as a Closure, but with no CallFrame, no
  /// register-window setup, and no heap allocation on the call path:
  /// its arguments are a zero-copy slice straight into the caller's own
  /// registers. See vm/natives.rs.
  Native(NativeFunction),
  /// See `ObjClass`'s own doc comment. Wrapped in a `RefCell` (unlike
  /// every other heap object here, which is immutable-after-creation
  /// except through a `Cell`-wrapped field) because a class's tables
  /// keep growing throughout its own declaration -- `RefCell` is the
  /// safe way to do that through the same shared `*const Obj` pointer
  /// every other Value already uses, rather than reaching for unsafe
  /// mutation.
  Class(Box<RefCell<ObjClass>>),
  Instance(ObjInstance),
  /// A `lower..upper` range value -- valid in either direction (see
  /// Expr::Range's compilation in compiler.rs). Stored as raw f64s, not
  /// Values, specifically so this is a GC leaf: nothing here is ever a
  /// heap pointer, so the mark phase's worklist never needs to descend
  /// into one (see VM::collect_garbage's Obj::Range arm).
  Range {
    lower: f64,
    upper: f64,
  },
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

#[derive(Clone, Copy)]
pub enum UpvalueState {
  /// Points at an absolute index into `VM::registers` (i.e. already
  /// includes some frame's `base`).
  Open(usize),
  Closed(Value),
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
  /// `Compiler::compile_method_prototype` -- register 0 of its frame is
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

  /// The file this function was compiled from, shared (via `Rc`, not
  /// cloned) by every function compiled in the same `Compiler` run --
  /// see `Compiler::new`. Read per-frame by `VM::build_stacktrace`,
  /// which is what lets a trace correctly attribute each frame to its
  /// OWN file.
  pub source_path: Rc<str>,
}

/// A method value bound to a specific receiver -- produced only when a
/// method is accessed WITHOUT being called immediately (`var f =
/// obj.method`), so it can be stored, passed around, and invoked later
/// on its own. Direct call-site method calls (`obj.method(args)`,
/// `self.method(args)`, `parent.method(args)`) never go through this --
/// they're compiled straight to Invoke/InvokeSuper, which look up and
/// call in one step with no heap allocation. This exists purely for the
/// "method as a first-class value" case.
pub struct ObjBoundMethod {
  pub receiver: Value,
  pub method: Value,
}

pub struct ObjClosure {
  /// The prototype this closure was created from, as the tagged `Value`
  /// that points at its `Obj::Func` -- not a raw pointer -- so the GC's
  /// mark phase can trace it like any other reference and keep the
  /// prototype (and everything in its constant pool) alive for exactly
  /// as long as some closure still needs it.
  pub function: Value,
  /// One entry per `function.upvalues` descriptor, in the same order.
  /// Each Value here points at an `Obj::Upvalue`.
  pub upvalues: Vec<Value>,
}

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
  /// `VM::call_native`'s own arity-mismatch message -- see that call
  /// site's doc comment for why the count a user sees has to have the
  /// receiver subtracted back out.
  pub is_method: bool,
  pub func: NativeFn,
}

/// A class value: the shared, heap-allocated "shape" every instance of
/// it points back at. Method/field-slot tables are fully merged with
/// the superclass chain once, at declaration time (see `Instr::MakeClass`)
/// -- so a call site never needs to walk ancestors to find an instance
/// method or a field's slot index, just one hashmap lookup. Statics are
/// the deliberate exception: `static_slots`/`statics` hold ONLY this
/// class's own declarations, and a lookup that misses walks `superclass`
/// live (see `lookup_static` in vm.rs) -- so an inherited static genuinely
/// shares storage with wherever it's actually declared, rather than being
/// copied.
pub struct ObjClass {
  pub name: String,
  pub superclass: Option<Value>,
  /// Own + inherited instance methods, pre-merged (own overrides
  /// inherited of the same name). Also where a self-named method (this
  /// class's constructor, if it declares one) lives -- see
  /// `Instr::FinalizeClass`.
  pub methods: HashMap<String, Value>,
  /// Name -> slot index for OWN + inherited instance fields, pre-merged
  /// the same way; `field_count` is the total flat layout size every
  /// `ObjInstance.fields` of this class is allocated with.
  pub field_slots: HashMap<String, u16>,
  pub field_count: u16,
  /// This class's OWN declared instance fields' initializer (arity 1:
  /// self) -- None if it declares no instance fields itself. Ancestors'
  /// own initializers are invoked separately (root to leaf) at
  /// construction time -- see `VM::instantiate` -- so this is
  /// deliberately NOT chained to call the superclass's initializer.
  pub own_field_initializer: Option<Value>,
  /// Resolved once, when the class is declared (see
  /// `Instr::FinalizeClass`): this class's own same-named method if it
  /// declares one, else inherited from the nearest ancestor that does,
  /// else None (no constructor anywhere in the chain -- valid, just
  /// means "run field inits and stop").
  pub constructor: Option<Value>,
  /// Own-only static fields AND static methods, unified in one
  /// namespace (a static method is just a Value that happens to be a
  /// Closure) -- deliberately not merged with the superclass at
  /// declaration time; see the struct-level doc comment above.
  pub static_slots: HashMap<String, u16>,
  pub statics: Vec<Cell<Value>>,
}

/// An instance value. `fields` is a fixed-size, flat, slot-indexed
/// array sized to `class.field_count` at allocation time -- there is no
/// dynamic/open field set; a name not present in `class.field_slots` is
/// a runtime error (see `Instr::GetField`/`SetField` in vm.rs).
pub struct ObjInstance {
  pub class: Value,
  pub fields: Vec<Cell<Value>>,
}

/// Wrapper around `Value` that implements `Hash`/`Eq` in terms of
/// `Value::equals` (content-equality for strings/numbers/nil/bool,
/// pointer identity for lists/dicts/closures/instances/etc.), since
/// `Value` itself has no `Hash` impl and its raw NaN-boxed bits don't
/// hash/compare consistently on their own. This is what lets
/// `DictStorage::index` be a real `HashMap`.
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
      self.0.as_number().to_bits().hash(state);
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
  index: HashMap<DictKey, usize>,
}

impl DictStorage {
  pub fn new() -> Self {
    DictStorage {
      entries: Vec::new(),
      index: HashMap::new(),
    }
  }

  /// Builds storage from raw pairs, de-duplicating by VALUE equality
  /// (via `set`'s own last-write-wins rule) -- same semantics the old
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
}

/// Everything a native function body gets handed. `args` is an OWNED
/// copy of the call's arguments, not a borrow into VM::registers -- it
/// has to be, because `vm` is a live &mut VM at the same time, and a
/// slice into the VM's own register array would alias with that. This
/// is the real cost of letting natives call back into Zuri code via
/// `vm.call_value(...)`.
///
/// `name` is the SAME string as `NativeFunction::name` this call was
/// dispatched through -- carried here specifically so the
/// `enforce_arg_*!` family (see `builtins::enforce`) can generate
/// "'foo' expects ..." messages without every native having to spell
/// its own name out by hand at every call site.
pub struct ZuriContext<'a> {
  pub vm: &'a mut VM,
  pub args: &'a [Value],
  pub name: &'static str,
}

impl<'a> ZuriContext<'a> {
  /// Equivalent to `ctx.vm.heap`, spelled out because `heap` used to be
  /// its own field before this took `&mut VM` instead.
  pub fn heap(&mut self) -> &mut Heap {
    &mut self.vm.heap
  }
}

/// A plain fn pointer, not a boxed closure. Takes &mut Heap (not &mut VM)
/// specifically so it can be called while a slice of VM::registers is
/// still borrowed -- see the disjoint-field-borrow note in Instr::Call.
pub type NativeFn = fn(&mut ZuriContext) -> Result<Value, String>;

/// Wraps every heap object with an inline GC mark bit so marking is a
/// pointer dereference instead of a HashSet insert.
#[repr(C)]
struct GcBox {
  live: Cell<bool>,
  marked: Cell<bool>,
  size: usize,
  obj: Obj,
}

/// One fixed-capacity block of GcBox storage. Never grows past
/// `CHUNK_SIZE` (enforced in `Heap::alloc`), so its backing buffer never
/// reallocates -- every pointer handed out into a chunk stays valid for
/// the chunk's whole lifetime, which is the whole lifetime of `Heap`.
/// Moving the `Chunk` struct itself (e.g. when the outer `chunks: Vec`
/// grows) only moves the Vec's (ptr, len, cap) header, never the buffer
/// it points at -- so that's safe too.
struct GcChunk {
  slots: Vec<GcBox>,
  /// Dead slots within THIS chunk, ready for reuse with no allocator
  /// call. Kept local (not a heap-wide free list) specifically so that
  /// when `live_count` hits zero, dropping the whole `Chunk` also drops
  /// this list -- nothing outside needs to be purged or cross-referenced.
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
  /// `None` marks a chunk that's been reclaimed -- kept as a hole
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
  live_count: usize,
}

impl Heap {
  /// Floor for `next_gc` -- keeps a small/short-lived program from
  /// triggering a collection after every third allocation.
  const MIN_NEXT_GC: usize = 10 * 1024 * 1024;
  /// After a sweep, the next collection is scheduled at this multiple of
  /// the heap's current live size.
  const GC_HEAP_GROW_FACTOR: f32 = 1.5;

  pub fn new() -> Self {
    Heap {
      chunks: Vec::new(),
      candidates: Vec::new(),
      bytes_allocated: 0,
      next_gc: Self::MIN_NEXT_GC,
      live_count: 0,
    }
  }

  #[inline]
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

  /// Has the heap grown enough since the last collection that the VM
  /// should pause and run one before allocating further?
  #[inline]
  pub fn needs_gc(&self) -> bool {
    self.bytes_allocated > self.next_gc
  }

  #[inline]
  fn gcbox_of(ptr: *const Obj) -> *const GcBox {
    let offset = std::mem::offset_of!(GcBox, obj);
    // SAFETY: every `*const Obj` reachable from a Value was produced by
    // `alloc`, above, from the `obj` field of a real `GcBox`.
    unsafe { (ptr as *const u8).sub(offset) as *const GcBox }
  }

  /// Mark the object behind `ptr` reachable this cycle. Returns true
  /// the FIRST time (caller should walk its children), false on repeat
  /// visits -- same contract the old HashSet-based version had.
  pub(crate) fn mark_object(ptr: *const Obj) -> bool {
    let gcbox = unsafe { &*Self::gcbox_of(ptr) };
    debug_assert!(gcbox.live.get(), "marking a supposedly-dead object");
    !gcbox.marked.replace(true)
  }

  /// Rough size in bytes attributed to one heap object, used only to
  /// decide *when* to collect -- not an exact accounting (e.g. a
  /// `BigInt`'s own heap limbs aren't sized individually), just enough
  /// to make `next_gc` track real memory pressure instead of raw object
  /// count.
  fn approx_size(obj: &Obj) -> usize {
    use std::mem::size_of;
    size_of::<Obj>()
      + match obj {
        Obj::Str(s) => s.len(),
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
        Obj::Range { .. } => 0,
      }
  }

  fn alloc(&mut self, obj: Obj) -> Value {
    let size = Self::approx_size(&obj);
    self.bytes_allocated += size;
    self.live_count += 1;

    // Try the most recently useful chunk(s) first, discarding any that
    // turn out to be exhausted or already reclaimed.
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
          (*ptr).size = size;
          (*ptr).obj = obj;
          return Value::obj(&(*ptr).obj as *const Obj);
        }
      }

      if chunk.slots.len() < chunk.slots.capacity() {
        chunk.slots.push(GcBox {
          live: Cell::new(true),
          marked: Cell::new(false),
          size,
          obj,
        });
        chunk.live_count += 1;
        let gcbox_ptr: *const GcBox = chunk.slots.last().unwrap();
        // SAFETY: just pushed into this chunk's stable (with_capacity'd,
        // never-reallocating) buffer.
        let obj_ptr: *const Obj = unsafe { &(*gcbox_ptr).obj };
        return Value::obj(obj_ptr);
      }

      // Neither a free slot nor spare capacity left -- stale candidate.
      self.candidates.pop();
    }

    // No usable candidate -- start a fresh chunk.
    let mut chunk = GcChunk {
      slots: Vec::with_capacity(CHUNK_SIZE),
      free: Vec::new(),
      live_count: 1,
    };
    chunk.slots.push(GcBox {
      live: Cell::new(true),
      marked: Cell::new(false),
      size,
      obj,
    });
    self.chunks.push(Some(chunk));
    let idx = self.chunks.len() - 1;
    self.candidates.push(idx);
    let gcbox_ptr: *const GcBox = self.chunks[idx].as_ref().unwrap().slots.last().unwrap();
    let obj_ptr: *const Obj = unsafe { &(*gcbox_ptr).obj };
    Value::obj(obj_ptr)
  }

  pub fn alloc_string(&mut self, s: impl Into<String>) -> Value {
    self.alloc(Obj::Str(s.into()))
  }

  pub fn alloc_bytes(&mut self, b: impl Into<Vec<u8>>) -> Value {
    self.alloc(Obj::Bytes(RefCell::new(b.into())))
  }

  pub fn alloc_bigint(&mut self, s: impl Into<BigInt>) -> Value {
    self.alloc(Obj::BigInt(s.into()))
  }

  pub fn alloc_list(&mut self, list: impl Into<Vec<Value>>) -> Value {
    self.alloc(Obj::List(RefCell::new(list.into())))
  }

  /// Builds a Dict from raw (key, value) pairs, de-duplicating by
  /// VALUE equality (not pointer identity -- two distinct string
  /// objects with the same text collide, matching every other
  /// language's dict-literal semantics), keeping the LAST occurrence
  /// of any repeated key.
  pub fn alloc_dict(&mut self, pairs: Vec<(Value, Value)>) -> Value {
    let storage = DictStorage::from_pairs(pairs);
    self.alloc(Obj::Dict(RefCell::new(storage)))
  }

  pub fn alloc_function(&mut self, f: ObjFunction) -> Value {
    self.alloc(Obj::Func(Box::new(f)))
  }

  pub fn alloc_closure(&mut self, c: ObjClosure) -> Value {
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
  /// to reference an enclosing local) -- allocates the prototype and
  /// wraps it in a trivial empty-upvalue closure in one step.
  pub fn alloc_plain_closure(&mut self, f: ObjFunction) -> Value {
    let proto_val = self.alloc_function(f);
    self.alloc_closure(ObjClosure {
      function: proto_val,
      upvalues: Vec::new(),
    })
  }

  pub fn alloc_native(&mut self, native: NativeFunction) -> Value {
    self.alloc(Obj::Native(native))
  }

  pub fn alloc_class(&mut self, class: ObjClass) -> Value {
    self.alloc(Obj::Class(Box::new(RefCell::new(class))))
  }

  pub fn alloc_instance(&mut self, class: Value, field_count: usize) -> Value {
    self.alloc(Obj::Instance(ObjInstance {
      class,
      fields: vec![Cell::new(Value::nil()); field_count],
    }))
  }

  pub fn alloc_range(&mut self, lower: f64, upper: f64) -> Value {
    self.alloc(Obj::Range { lower, upper })
  }

  /// Drop every object whose address isn't in `reachable`, then
  /// recompute `bytes_allocated` and re-arm `next_gc` off the resulting
  /// live size. Returns how many objects were freed.
  ///
  /// This only performs the sweep half of mark-and-sweep -- `reachable`
  /// must already be the complete, transitively-closed set of live
  /// objects (see `VM::collect_garbage`), or anything missing from it
  /// gets freed out from under whatever still references it.
  ///
  /// Sweep every resident chunk. A slot that's live but wasn't marked
  /// this cycle is garbage: its contents are dropped in place and it
  /// goes on its chunk's local free list. A chunk whose live_count hits
  /// zero -- every slot in it dead -- is dropped ENTIRELY, returning its
  /// backing allocation (and, for a block this size, typically the
  /// underlying pages) to the allocator instead of holding it as
  /// permanent inventory.
  pub fn sweep(&mut self) -> usize {
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
            self.bytes_allocated = self.bytes_allocated.saturating_sub(gcbox.size);
            gcbox.obj = Obj::Range {
              lower: 0.0,
              upper: 0.0,
            }; // drops old contents
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
    self.next_gc =
      ((self.bytes_allocated as f32 * Self::GC_HEAP_GROW_FACTOR) as usize).max(Self::MIN_NEXT_GC);
    freed
  }
}
