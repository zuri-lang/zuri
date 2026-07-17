use num_bigint::BigInt;
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};

use crate::vm::chunk::Chunk;
use crate::vm::value::Value;
use crate::vm::vm::VM;

/// Everything a Value's pointer tag can point at.
pub enum Obj {
  Str(String),
  Bytes(RefCell<Vec<u8>>),
  BigInt(BigInt),
  /// A dynamically-sized list -- used to collect a variadic function's
  /// trailing arguments. No bytecode support yet for indexing into or
  /// iterating one from Zuri code -- this is just the storage.
  List(Vec<Cell<Value>>),
  /// A dict literal's storage. `Vec<(Value, Value)>` rather than a real
  /// hash map, deliberately -- a proper HashMap needs Value to have
  /// Hash/Eq that matches Value::equals' semantics (content-equality
  /// for strings and numbers, identity for closures/lists), which is a
  /// design decision worth making once Expr::Index/lookup exists and
  /// performance is the thing being optimized for. This is correct and
  /// simple; it's O(n) lookup, which is the honest tradeoff for now.
  Dict(RefCell<Vec<(Value, Value)>>),
  /// A function PROTOTYPE -- the static, compiled-once result of one
  /// `function` declaration or literal. Shared by every closure ever
  /// created from it; holds no per-call-site state itself.
  Func(ObjFunction),
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
  Class(RefCell<ObjClass>),
  Instance(ObjInstance),
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

/// A plain Rust function pointer -- not `Box<dyn Fn>`. No vtable, no
/// heap-allocated closure environment; calling one is a single indirect
/// call through a fn pointer, as cheap as native dispatch gets. Takes
/// `&mut Heap` (not `&mut VM`) specifically so the caller can hand it a
/// zero-copy slice of `VM::registers` at the same time -- see the
/// disjoint-field-borrow note in `Instr::Call`'s handling.
/// Everything a native function body gets handed. `args` is an OWNED
/// copy of the call's arguments, not a borrow into VM::registers -- it
/// has to be, because `vm` is a live &mut VM at the same time, and a
/// slice into the VM's own register array would alias with that. This is
/// the real cost of letting natives call back into Zuri code via
/// `vm.call_value(...)`.
pub struct ZuriContext<'a> {
  pub vm: &'a mut VM,
  pub args: &'a [Value],
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

/// Owns every heap object for the lifetime of the VM. Values only ever hold
/// *const Obj pointers into this arena, never real ownership, which is what
/// lets a Value stay a plain Copy u64.
#[derive(Default)]
pub struct Heap {
  objects: Vec<Box<Obj>>,
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
}

impl Heap {
  /// Floor for `next_gc` -- keeps a small/short-lived program from
  /// triggering a collection after every third allocation.
  const MIN_NEXT_GC: usize = 256 * 1024;
  /// After a sweep, the next collection is scheduled at this multiple of
  /// the heap's current live size.
  const GC_HEAP_GROW_FACTOR: usize = 2;

  pub fn new() -> Self {
    Heap {
      objects: Vec::new(),
      bytes_allocated: 0,
      next_gc: Self::MIN_NEXT_GC,
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
    self.objects.len()
  }

  /// Has the heap grown enough since the last collection that the VM
  /// should pause and run one before allocating further?
  #[inline]
  pub fn needs_gc(&self) -> bool {
    self.bytes_allocated > self.next_gc
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
        Obj::BigInt(_) => 0,
        Obj::List(items) => items.len() * size_of::<Cell<Value>>(),
        Obj::Dict(pairs) => pairs.borrow().len() * size_of::<(Value, Value)>(),
        Obj::Func(f) => {
          f.chunk.code.len() * size_of::<crate::vm::chunk::Instr>()
            + f.chunk.constants.len() * size_of::<Value>()
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
        },
        Obj::Instance(i) => i.fields.len() * size_of::<Cell<Value>>(),
      }
  }

  fn alloc(&mut self, obj: Obj) -> Value {
    self.bytes_allocated += Self::approx_size(&obj);
    self.objects.push(Box::new(obj));
    let ptr: *const Obj = self.objects.last().unwrap().as_ref();
    Value::obj(ptr)
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
    let cells = list.into().into_iter().map(Cell::new).collect();
    self.alloc(Obj::List(cells))
  }

  /// Builds a Dict from raw (key, value) pairs, de-duplicating by
  /// VALUE equality (not pointer identity -- two distinct string
  /// objects with the same text collide, matching every other
  /// language's dict-literal semantics), keeping the LAST occurrence
  /// of any repeated key.
  pub fn alloc_dict(&mut self, pairs: Vec<(Value, Value)>) -> Value {
    let mut deduped: Vec<(Value, Value)> = Vec::with_capacity(pairs.len());
    for (key, value) in pairs {
      if let Some(existing) = deduped.iter_mut().find(|(k, _)| k.equals(&key)) {
        existing.1 = value;
      } else {
        deduped.push((key, value));
      }
    }
    self.alloc(Obj::Dict(RefCell::new(deduped)))
  }

  pub fn alloc_function(&mut self, f: ObjFunction) -> Value {
    self.alloc(Obj::Func(f))
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
    self.alloc(Obj::Class(RefCell::new(class)))
  }

  pub fn alloc_instance(&mut self, class: Value, field_count: usize) -> Value {
    self.alloc(Obj::Instance(ObjInstance {
      class,
      fields: vec![Cell::new(Value::nil()); field_count],
    }))
  }

  /// Drop every object whose address isn't in `reachable`, then
  /// recompute `bytes_allocated` and re-arm `next_gc` off the resulting
  /// live size. Returns how many objects were freed.
  ///
  /// This only performs the sweep half of mark-and-sweep -- `reachable`
  /// must already be the complete, transitively-closed set of live
  /// objects (see `VM::collect_garbage`), or anything missing from it
  /// gets freed out from under whatever still references it.
  pub fn sweep(&mut self, reachable: &HashSet<*const Obj>) -> usize {
    let before = self.objects.len();
    self
      .objects
      .retain(|obj| reachable.contains(&(obj.as_ref() as *const Obj)));
    let freed = before - self.objects.len();

    self.bytes_allocated = self.objects.iter().map(|o| Self::approx_size(o)).sum();
    self.next_gc = (self.bytes_allocated * Self::GC_HEAP_GROW_FACTOR).max(Self::MIN_NEXT_GC);

    freed
  }
}
