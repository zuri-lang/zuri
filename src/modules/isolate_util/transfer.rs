//! Heap-independent snapshot of a Zuri value; the only thing that
//! ever crosses an isolate spawn/join or channel send/recv.
//!
//! Every isolate (isolate thread) owns its own private `VM`/`Heap`,
//! and this VM's object model was never built to be touched from more
//! than one thread: `Value`s are raw `*const Obj` pointers, `Chunk`s
//! carry non-atomic inline caches, and the GC's remembered set is a
//! `thread_local!`. So no `Value` and no compiled `Chunk` ever crosses
//! a thread boundary directly; `capture` walks a source heap once,
//! producing a plain, `Send`, heap-independent tree; `materialize`
//! walks that tree once more, allocating a fresh, equivalent value
//! into a (possibly different) destination heap.
//!
//! A plain, non-capturing function or method that's currently bound to
//! a module-level name is captured cheaply, as a `Named` reference
//! (its defining home plus its own name) and RE-RESOLVED against a
//! freshly-loaded copy of that home on the destination side, exactly
//! the way `import` already loads a module once and caches it: see
//! `Home`. Everything else callable; a closure that captures a local
//! variable, or a lambda that was never bound to a name at all; gets
//! a full STRUCTURAL transplant instead (see `CapturedFunction`): its
//! compiled bytecode and constant pool travel across directly (bare
//! data; a `Chunk`'s instructions/constants carry no heap pointers of
//! their own once its `Value` constants are captured the same
//! recursive way as everything else), and its captured variables cross
//! as an independent snapshot, not shared state.
//!
//! A `Ptr`-wrapped native resource (see `ObjPtr`) is the one error
//! to "always copy, never share, source stays valid": most such
//! resources (a socket, a database connection) are only sound to use
//! from one thread at a time, so crossing has to be a MOVE. `capture`
//! takes the payload out of the source `ObjPtr` (leaving it tagged
//! `"<moved>"`, unusable on the source side from then on) and wraps it
//! in a `PtrSlot`; an `Arc<Mutex<Option<...>>>`; rather than moving
//! it in directly, specifically so `TransferValue`/`TransferGraph` can
//! stay plain `Clone` (needed so an isolate's `.join()` result stays
//! freely re-readable, same as any other value). Cloning a `PtrSlot`
//! only clones the `Arc`; the payload underneath is still consumed at
//! most once, by whichever `materialize` call reaches it first --
//! anything after that gets a clear error instead of silently
//! duplicating a resource that can't be soundly duplicated.

use std::any::Any;
use std::path::Path;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use num_bigint::BigInt;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::vm::chunk::{Chunk, Instr, JumpKey, ParamTypeCheck};
use crate::vm::object::{
  JitInfo, NativeFn, NativeFunction, ObjClosure, ObjFunction, ObjModuleBinding, UpvalueDescriptor,
  UpvalueState, write_barrier,
};
use crate::vm::value::Value;
use crate::vm::vm::VM;

use super::pool;

/// A moved `Ptr` payload: see this module's own top-level docs.
type PtrSlot = Arc<Mutex<Option<(&'static str, Box<dyn Any + Send>)>>>;

/// Where a `Named` function/class/method lives, so a destination
/// isolate can load the exact same source before looking it up.
///
/// Deliberately only ever a `.zu` MODULE, never the program's own
/// entry script; this restriction applies to the `Named` (by-
/// binding) resolution strategy specifically, NOT to isolates in
/// general (see `CapturedFunction` for the other strategy, which has
/// no such restriction). A module's top level is expected to be
/// side-effect-light (declarations, mostly) and is only ever run once
/// and cached; exactly what re-resolving it on an isolate isolate
/// needs. The entry script has no such expectation: it's the
/// program's own real, imperative top-level logic, which commonly
/// includes the very `isolate.spawn`/`.join()` calls that would
/// trigger this resolution in the first place. Bootstrapping an isolate
/// by re-running it would re-run those calls too; recursively
/// spawning more work and, for a script that blocks on `.join()` at
/// its own top level (extremely common), deadlocking the isolate
/// against itself. Python's `multiprocessing` hits the identical
/// hazard with its `spawn` start method and resolves it the same way:
/// a spawn target must be importable from a module, never defined in
/// `__main__`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Home {
  /// Canonical, on-disk path; the same string `vm::modules` caches
  /// loaded modules under.
  path: String,
}

impl Home {
  fn describe(&self) -> String {
    format!("module '{}'", self.path)
  }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NamedKind {
  Function,
  Class,
  /// An instance method, captured e.g. via `obj.method` used as a
  /// value rather than called immediately. `class_name` is the
  /// method's OWN declaring class (`ObjFunction::owning_class_name`),
  /// looked up in `Home` the same way a plain class reference is;
  /// the transferred `name` is the method's name within it.
  Method {
    class_name: String,
  },
}

/// A structurally-captured function prototype: its full compiled
/// bytecode and constant pool, plain data with no live heap pointers
/// of its own (every `Value` constant is itself a `TransferValue`,
/// captured the same recursive way as anything else). Ready to be
/// rebuilt as a fresh `ObjFunction` on a destination heap.
///
/// Used for whatever the cheap `Named`/`Home` path can't handle: a
/// closure that captures a local variable, or a lambda that was never
/// assigned to a module-level name at all. Unlike `Named`, this never
/// needs the destination to already know the closure by name; only,
/// if `home` is `Some`, its DEFINING MODULE, so the closure's own
/// `GetGlobal`/`SetGlobal` instructions (calling a sibling top-level
/// function, reading a module `var`, ...) resolve exactly as they
/// would have on the source side. `home: None` (declared directly in
/// the main script) is FINE here, unlike for `Home` above: nothing in
/// this path ever re-executes anything, so there's no re-entrant
/// `spawn`/`join` hazard to avoid. A `GetGlobal` this closure's
/// bytecode issues for a name the destination doesn't have (there was
/// no module to load, or the module doesn't define it) just fails
/// with an ordinary `UndefinedError` at the point it's actually
/// reached, same as any other unresolved global.
#[derive(Clone)]
pub struct CapturedFunction {
  pub data: Arc<CapturedFunctionData>,
}

pub struct CapturedFunctionData {
  pub id: usize,
  pub name: String,
  pub variadic: bool,
  pub arity: u8,
  pub num_registers: u8,
  pub code: Vec<Instr>,
  pub constants: Vec<TransferValue>,
  pub jump_tables: Vec<FxHashMap<JumpKey, usize>>,
  pub lines: Vec<u32>,
  pub param_checks: Vec<ParamTypeCheck>,
  pub upvalue_descriptors: Vec<UpvalueDescriptor>,
  pub is_method: bool,
  pub owning_class_name: Option<String>,
  pub home: Option<Home>,
  pub root_globals: Vec<(String, TransferValue)>,
}

/// A structurally-captured class: see `capture_class_structural`.
#[derive(Clone)]
pub struct CapturedClass {
  name: String,
  superclass: Option<TransferValue>,
  methods: Vec<(String, TransferValue)>,
  field_slots: Vec<(String, u16)>,
  field_count: u16,
  own_field_initializer: Option<TransferValue>,
  static_slots: Vec<(String, u16)>,
  statics: Vec<TransferValue>,
}

/// One captured value, heap-independent and `Send`. Immediates (nil/
/// bool/number) and small leaves (strings, bytes, bigints, ranges,
/// native functions) are stored directly; anything with internal
/// sharing or cycles (lists, dicts, instances, bound methods,
/// structurally-captured closures) is a `Ref` into the owning
/// `TransferGraph`'s arena instead, so two slots that pointed at the
/// SAME source object still point at the same rebuilt object after
/// `materialize`, and a self-referential structure; including a
/// recursive lambda that captures itself; doesn't recurse forever
/// in either direction.
#[derive(Clone)]
pub enum TransferValue {
  Nil,
  Bool(bool),
  Number(f64),
  BigInt(BigInt),
  Str(String),
  Bytes(Vec<u8>),
  Range {
    lower: f64,
    upper: f64,
    step: f64,
  },
  Native {
    name: &'static str,
    min_arity: u8,
    variadic: bool,
    is_method: bool,
    func: NativeFn,
  },
  Named {
    home: Home,
    name: String,
    kind: NamedKind,
  },
  /// A builtin error class (`Error`, `TypeError`, ...); resolved
  /// by name against the destination's own prelude, already installed
  /// fresh by `VM::init` on every isolate. See `capture_class`'s own
  /// docs on why this can't go through the ordinary `Named`/`Home`
  /// path (there's no module a prelude class is declared in) or the
  /// structural one (a clone wouldn't be recognized by the
  /// destination's own `raise`/`catch`).
  Prelude(String),
  /// A `Channel` handle; cloned, not moved, since `pool::ChannelState`
  /// is already internally synchronized for concurrent access from
  /// both sides. See `capture_value`'s own docs on why this is NOT
  /// treated like an ordinary `Ptr`.
  ChannelHandle(Arc<pool::ChannelState>),
  /// A `Isolate` handle; same reasoning as `ChannelHandle`.
  IsolateHandle(Arc<pool::IsolateState>),
  /// An imported module, carried as the key it is cached under rather
  /// than as anything copied out of it; the destination loads the same
  /// module for itself and gets its own independent copy.
  ///
  /// This is the same bargain `Home` strikes, and it rests on the same
  /// property: a module's top level is declarations, it runs once, and
  /// the result is cached. What it buys is the ordinary case of a
  /// helper that uses an import; `import json` at the top of a file is
  /// a local of that file's own scope, so every function below it that
  /// mentions `json` captures the module as an upvalue and would
  /// otherwise be unable to cross.
  ///
  /// `binding` is set when the source value was the promoted binding
  /// `import PATH [as NAME]` produces rather than the bare module, and
  /// holds the local name it was bound under; promotion is re-derived
  /// on the destination from that name, exactly as the import itself
  /// would have.
  Module {
    key: String,
    binding: Option<String>,
  },
  Ref(u32),
}

#[derive(Clone)]
pub enum TransferNode {
  List(Vec<TransferValue>),
  Dict(Vec<(TransferValue, TransferValue)>),
  Instance {
    class: TransferValue,
    fields: Vec<TransferValue>,
  },
  BoundMethod {
    receiver: TransferValue,
    method: TransferValue,
  },
  Class(CapturedClass),
  /// A moved native-resource payload: see this module's own
  /// top-level docs.
  Ptr(PtrSlot),
  /// A bare, not-yet-closed-over function prototype; either a
  /// nested one (only ever appears inside another `CapturedFunction`'s
  /// own `constants`, mirroring how `Obj::Func` only ever appears
  /// inside a `Chunk`'s constant pool on the source side too), or the
  /// prototype half of a `Closure` entry below. Arena-backed (not a
  /// plain leaf) specifically so a recursive TOP-LEVEL function --
  /// `def fact(n) { ... fact(n - 1) ... }`, calling itself by name via
  /// `GetGlobal`; can be memoized before its own `root_globals` scan
  /// recurses back into capturing itself, the same self-reference
  /// protection `Closure`'s own upvalues get below.
  Proto(CapturedFunction),
  Closure {
    /// Always a `Ref` into this same arena, pointing at a `Proto`
    /// entry.
    proto: TransferValue,
    /// One snapshotted value per prototype's own
    /// `upvalue_descriptors` entry; captured BY VALUE at the moment
    /// this closure crossed the boundary, not shared live state
    /// (there's no such thing as shared mutable state between
    /// isolates: see this module's own top-level docs).
    upvalues: Vec<TransferValue>,
  },
}

/// A fully captured value, ready to move to another thread. `root` is
/// what `materialize` rebuilds and hands back; `arena` backs every
/// `TransferValue::Ref` reachable from it.
#[derive(Clone)]
pub struct TransferGraph {
  pub root: TransferValue,
  arena: Vec<TransferNode>,
}

// ---------------------------------------------------------------------
// capture: source heap -> heap-independent TransferGraph
// ---------------------------------------------------------------------

/// Walks `root`'s object graph on `vm`'s own heap and snapshots it
/// into a `TransferGraph`. Read-only; never allocates on `vm`'s
/// heap, so it can't itself trigger a collection.
pub fn capture(vm: &VM, root: Value) -> Result<TransferGraph, String> {
  if root.is_nil() {
    return Ok(TransferGraph {
      root: TransferValue::Nil,
      arena: Vec::new(),
    });
  }
  if root.is_bool() {
    return Ok(TransferGraph {
      root: TransferValue::Bool(root.as_bool()),
      arena: Vec::new(),
    });
  }
  if root.is_number() {
    return Ok(TransferGraph {
      root: TransferValue::Number(root.as_number()),
      arena: Vec::new(),
    });
  }
  if root.is_string() {
    return Ok(TransferGraph {
      root: TransferValue::Str(root.as_str().to_string()),
      arena: Vec::new(),
    });
  }
  if root.is_bytes() {
    return Ok(TransferGraph {
      root: TransferValue::Bytes(root.as_bytes()),
      arena: Vec::new(),
    });
  }
  if root.is_bigint() {
    return Ok(TransferGraph {
      root: TransferValue::BigInt(root.as_bigint().clone()),
      arena: Vec::new(),
    });
  }
  if root.is_range() {
    let (lower, upper) = root.as_range();
    return Ok(TransferGraph {
      root: TransferValue::Range {
        lower,
        upper,
        step: root.range_step(),
      },
      arena: Vec::new(),
    });
  }
  if root.is_native() {
    let n = root.as_native();
    return Ok(TransferGraph {
      root: TransferValue::Native {
        name: n.name,
        min_arity: n.min_arity,
        variadic: n.variadic,
        is_method: n.is_method,
        func: n.func,
      },
      arena: Vec::new(),
    });
  }
  if root.is_ptr_type(pool::CHANNEL_PTR_TYPE) {
    let handle = root
      .as_ptr_cell()
      .borrow()
      .downcast_ref::<Arc<pool::ChannelState>>()
      .cloned()
      .ok_or_else(|| "internal error: malformed channel handle".to_string())?;
    return Ok(TransferGraph {
      root: TransferValue::ChannelHandle(handle),
      arena: Vec::new(),
    });
  }
  if root.is_ptr_type(pool::ISOLATE_PTR_TYPE) {
    let handle = root
      .as_ptr_cell()
      .borrow()
      .downcast_ref::<Arc<pool::IsolateState>>()
      .cloned()
      .ok_or_else(|| "internal error: malformed isolate handle".to_string())?;
    return Ok(TransferGraph {
      root: TransferValue::IsolateHandle(handle),
      arena: Vec::new(),
    });
  }

  let mut arena = Vec::new();
  let mut memo: FxHashMap<usize, u32> = FxHashMap::default();
  let root = capture_value(vm, root, &mut arena, &mut memo)?;
  Ok(TransferGraph { root, arena })
}

fn capture_value(
  vm: &VM,
  v: Value,
  arena: &mut Vec<TransferNode>,
  memo: &mut FxHashMap<usize, u32>,
) -> Result<TransferValue, String> {
  if v.is_nil() {
    return Ok(TransferValue::Nil);
  }
  if v.is_bool() {
    return Ok(TransferValue::Bool(v.as_bool()));
  }
  if v.is_number() {
    return Ok(TransferValue::Number(v.as_number()));
  }

  // Only List/Dict/Instance/BoundMethod/Closure ever get inserted
  // below, so this only ever hits for a repeat/aliased reference to
  // one of those; a miss here just means "not one of those", not
  // "not captured yet".
  let ptr = v.as_obj() as usize;
  if let Some(&idx) = memo.get(&ptr) {
    return Ok(TransferValue::Ref(idx));
  }

  if v.is_string() {
    return Ok(TransferValue::Str(v.as_str().to_string()));
  }
  if v.is_bytes() {
    return Ok(TransferValue::Bytes(v.as_bytes()));
  }
  if v.is_bigint() {
    return Ok(TransferValue::BigInt(v.as_bigint().clone()));
  }
  if v.is_range() {
    let (lower, upper) = v.as_range();
    return Ok(TransferValue::Range {
      lower,
      upper,
      step: v.range_step(),
    });
  }
  if v.is_native() {
    let n = v.as_native();
    return Ok(TransferValue::Native {
      name: n.name,
      min_arity: n.min_arity,
      variadic: n.variadic,
      is_method: n.is_method,
      func: n.func,
    });
  }
  if v.is_func() {
    // A bare nested prototype; only ever reached from
    // `capture_prototype`'s own constant-pool walk, never as an
    // ordinary Zuri-visible runtime value (every callable Value a
    // script can hold is a Closure, even a capture-nothing one: see
    // `Obj::Closure`'s own docs).
    return capture_prototype(vm, v, arena, memo);
  }
  if v.is_closure() {
    return capture_closure(vm, v, arena, memo);
  }
  if v.is_class() {
    return capture_class(vm, v, arena, memo);
  }
  if v.is_bound_method() {
    let idx = arena.len() as u32;
    arena.push(TransferNode::BoundMethod {
      receiver: TransferValue::Nil,
      method: TransferValue::Nil,
    });
    memo.insert(ptr, idx);
    let bm = v.as_bound_method();
    let receiver = capture_value(vm, bm.receiver, arena, memo)?;
    let method = capture_value(vm, bm.method, arena, memo)?;
    arena[idx as usize] = TransferNode::BoundMethod { receiver, method };
    return Ok(TransferValue::Ref(idx));
  }
  if v.is_ptr_type(pool::CHANNEL_PTR_TYPE) {
    // A `Channel`'s own `_ptr` field, NOT a resource like a socket or
    // an encoder: `pool::ChannelState` is already internally
    // synchronized (`Mutex`+`Condvar`) specifically so it CAN be used
    // concurrently from both sides at once; that's the entire point
    // of a channel. Cloning the `Arc` (never moving/taking it) is what
    // lets the very channel an isolate was just handed still be sent
    // on/received from by the code that spawned it.
    let handle = v
      .as_ptr_cell()
      .borrow()
      .downcast_ref::<Arc<pool::ChannelState>>()
      .cloned()
      .ok_or_else(|| "internal error: malformed channel handle".to_string())?;
    return Ok(TransferValue::ChannelHandle(handle));
  }
  if v.is_ptr_type(pool::ISOLATE_PTR_TYPE) {
    // Same reasoning as `Channel` above: a `Isolate` handle is a
    // synchronized, freely-shareable reference to a result slot, not
    // an exclusive resource; cloning it is what lets a `Isolate`
    // handle be passed into (or returned from) another isolate.
    let handle = v
      .as_ptr_cell()
      .borrow()
      .downcast_ref::<Arc<pool::IsolateState>>()
      .cloned()
      .ok_or_else(|| "internal error: malformed isolate handle".to_string())?;
    return Ok(TransferValue::IsolateHandle(handle));
  }
  if v.is_ptr() {
    // A genuine MOVE, not a copy: see this module's own top-level
    // docs. Tombstones the SOURCE `ObjPtr` (its own value is left
    // empty, its type_name overwritten) so a native accidentally
    // touching it again after the move gets a clean type mismatch
    // through the ordinary `ptr_type_name()`/`is_ptr_type()` checks
    // every native already does, rather than silently reading stale
    // state. Arena-backed and memoized like everything else with
    // identity, so `[conn, conn]`; the same `Ptr` referenced twice
    // in one message; takes the payload only once and both slots
    // resolve to the one shared `PtrSlot`, not a double-take.
    let idx = arena.len() as u32;
    let mut borrowed = v.as_ptr_cell().borrow_mut();
    let type_name = borrowed.type_name;
    let payload = borrowed.take();
    borrowed.type_name = "<moved>";
    drop(borrowed);
    arena.push(TransferNode::Ptr(Arc::new(Mutex::new(Some((
      type_name, payload,
    ))))));
    memo.insert(ptr, idx);
    return Ok(TransferValue::Ref(idx));
  }
  if v.is_list() {
    let idx = arena.len() as u32;
    arena.push(TransferNode::List(Vec::new()));
    memo.insert(ptr, idx);
    let items = v.as_list();
    let mut captured = Vec::with_capacity(items.len());
    for item in items {
      captured.push(capture_value(vm, item, arena, memo)?);
    }
    arena[idx as usize] = TransferNode::List(captured);
    return Ok(TransferValue::Ref(idx));
  }
  if v.is_dict() {
    let idx = arena.len() as u32;
    arena.push(TransferNode::Dict(Vec::new()));
    memo.insert(ptr, idx);
    let pairs = v.as_dict();
    let mut captured = Vec::with_capacity(pairs.len());
    for (k, val) in pairs {
      let k = capture_value(vm, k, arena, memo)?;
      let val = capture_value(vm, val, arena, memo)?;
      captured.push((k, val));
    }
    arena[idx as usize] = TransferNode::Dict(captured);
    return Ok(TransferValue::Ref(idx));
  }
  if v.is_instance() {
    let idx = arena.len() as u32;
    arena.push(TransferNode::Instance {
      class: TransferValue::Nil,
      fields: Vec::new(),
    });
    memo.insert(ptr, idx);
    let inst = v.as_instance();
    let class = capture_value(vm, inst.class, arena, memo)?;
    let mut fields = Vec::with_capacity(inst.fields.len());
    for cell in inst.fields.iter() {
      fields.push(capture_value(vm, cell.get(), arena, memo)?);
    }
    arena[idx as usize] = TransferNode::Instance { class, fields };
    return Ok(TransferValue::Ref(idx));
  }

  if v.is_module() || v.is_module_binding() {
    let (module, binding) = if v.is_module() {
      (v, None)
    } else {
      let b = v.as_module_binding();
      (b.module, Some(b.bind_name.clone()))
    };
    let key = crate::vm::modules::cache_key_of(vm, module).ok_or_else(|| {
      format!(
        "cannot send module '{}' across isolates; it was never loaded from a \
         file or a builtin",
        module.as_module().name
      )
    })?;
    return Ok(TransferValue::Module { key, binding });
  }

  Err(format!(
    "cannot send a {} across isolates; only nil, bool, number, string, \
     bytes, bigint, range, list, dict, instance, class, bound method, \
     function, module, and native-pointer values can cross",
    v.type_name()
  ))
}

fn capture_closure(
  vm: &VM,
  v: Value,
  arena: &mut Vec<TransferNode>,
  memo: &mut FxHashMap<usize, u32>,
) -> Result<TransferValue, String> {
  let closure = v.as_closure();
  let proto = closure.function.as_func();

  // Cheap path first: a plain function/lambda; or a method; that's
  // currently bound to a module-level name (directly, or via its
  // class's own `methods` map) resolves as a `Named` reference: no
  // bytecode needs to travel at all, and the SAME destination Value
  // gets reused across repeated messages.
  //
  // Deliberately NOT gated on `closure.upvalues.is_empty()`: whether
  // this specific closure happens to have captured something is
  // irrelevant to whether the Named path is safe, because the Named
  // path never transplants THIS closure's own upvalues at all; the
  // destination re-loads the home module fresh and gets back
  // whatever closure THAT run independently creates for the same
  // name, with its own independently-recreated upvalues. A module-
  // level `def`/`var` commonly closes over another plain (non-`@`-
  // exported) import in the same file; e.g. `import _isolate` is
  // just a local of the file's own top-level scope, so any nested
  // function referencing it captures it as an upvalue; and a
  // `Module`/`ModuleBinding` value can never itself cross an isolate
  // boundary. Gating this on an empty upvalue list would reject
  // exactly that ordinary case, forcing it down the STRUCTURAL path
  // below where it genuinely does need to move that upvalue and
  // genuinely can't. Only a closure `find_binding_name`/the method
  // lookup can't find by name (a true local; returned from an
  // enclosing function, or never assigned a name at all) needs
  // structural transplant, upvalues and all.
  if let Some(m) = proto.globals_module {
    let home = Home {
      path: m.as_module().path.clone(),
    };
    if proto.is_method {
      if let Some(class_name) = proto.owning_class_name.clone() {
        let matches = lookup_named_source(vm, &home, &class_name)
          .filter(|c| c.is_class())
          .is_some_and(|c| {
            c.as_class()
              .methods
              .get(&proto.name)
              .is_some_and(|m| m.equals(&v))
          });
        if matches {
          return Ok(TransferValue::Named {
            home,
            name: proto.name.clone(),
            kind: NamedKind::Method { class_name },
          });
        }
      }
    } else if let Some(name) = find_binding_name(vm, &home, v) {
      return Ok(TransferValue::Named {
        home,
        name,
        kind: NamedKind::Function,
      });
    }
  }

  // Everything else; a closure that captures a local variable, a
  // lambda that was never bound to a name, or a method whose class
  // lives directly in the main script; gets a full structural
  // transplant. `capture_prototype` carries `is_method`/
  // `owning_class_name` through as plain data either way, so a
  // structurally-captured method still round-trips correctly through
  // `Instr::Invoke`'s calling convention on the other side.
  // `capture_prototype` also handles the closure's own self-reference
  // protection (a recursive lambda closing over the very local it's
  // assigned to needs its PROTOTYPE entry memoized before upvalues
  // are captured); this entry additionally memoizes the CLOSURE value
  // itself, so a second reference to the same closure (not just the
  // same underlying function) also resolves to one shared entry.
  // `closure_idx` MUST be computed AFTER `capture_prototype` returns,
  // not before: that call pushes its own entries (the prototype
  // itself, plus whatever its constants/root-globals recursion adds)
  // onto this SAME arena first, so `arena.len()` at that point; not
  // before; is where THIS closure's own entry will actually land.
  let proto_ref = capture_prototype(vm, closure.function, arena, memo)?;
  let closure_idx = arena.len() as u32;
  arena.push(TransferNode::Closure {
    proto: proto_ref,
    upvalues: Vec::new(),
  });
  memo.insert(v.as_obj() as usize, closure_idx);

  let mut upvalues = Vec::with_capacity(closure.upvalues.len());
  for &uv in &closure.upvalues {
    let current = vm.read_upvalue(uv);
    upvalues.push(capture_value(vm, current, arena, memo)?);
  }
  if let TransferNode::Closure { upvalues: slot, .. } = &mut arena[closure_idx as usize] {
    *slot = upvalues;
  }
  Ok(TransferValue::Ref(closure_idx))
}

/// Captures the function prototype `func_val` (an `Obj::Func`) points
/// at, returning a `Ref` into a `TransferNode::Proto` arena entry.
/// Memoizes `func_val` BEFORE capturing its constants/root-globals;
/// required for a recursive top-level function (`def fact(n) { ...
/// fact(n - 1) ... }`, calling itself by name via `GetGlobal`), whose
/// own root-globals scan would otherwise recurse into capturing
/// itself forever. `func_val` may be a bare `Obj::Func` (a nested
/// prototype constant) or `ObjClosure::function` (the prototype half
/// of a real closure); both are the same `Obj::Func` shape.
fn capture_prototype(
  vm: &VM,
  func_val: Value,
  arena: &mut Vec<TransferNode>,
  memo: &mut FxHashMap<usize, u32>,
) -> Result<TransferValue, String> {
  let ptr = func_val.as_obj() as usize;
  if let Some(&idx) = memo.get(&ptr) {
    return Ok(TransferValue::Ref(idx));
  }

  let proto = func_val.as_func();
  let home = proto.globals_module.map(|m| Home {
    path: m.as_module().path.clone(),
  });

  let idx = arena.len() as u32;
  arena.push(TransferNode::Proto(CapturedFunction {
    data: Arc::new(CapturedFunctionData {
      id: ptr,
      name: proto.name.clone(),
      variadic: proto.variadic,
      arity: proto.arity,
      num_registers: proto.num_registers,
      code: proto.chunk.code.clone(),
      constants: Vec::new(),
      jump_tables: proto.chunk.jump_tables.clone(),
      lines: proto.chunk.lines.clone(),
      param_checks: proto.chunk.param_checks.clone(),
      upvalue_descriptors: proto.upvalues.clone(),
      is_method: proto.is_method,
      owning_class_name: proto.owning_class_name.clone(),
      home: home.clone(),
      root_globals: Vec::new(),
    }),
  }));
  memo.insert(ptr, idx);

  let constants = proto
    .chunk
    .constants
    .iter()
    .map(|&c| capture_value(vm, c, arena, memo))
    .collect::<Result<Vec<_>, _>>()?;
  let root_globals = if home.is_none() {
    capture_root_globals(vm, &proto.chunk, arena, memo)?
  } else {
    Vec::new()
  };

  let full_data = Arc::new(CapturedFunctionData {
    id: ptr,
    name: proto.name.clone(),
    variadic: proto.variadic,
    arity: proto.arity,
    num_registers: proto.num_registers,
    code: proto.chunk.code.clone(),
    constants,
    jump_tables: proto.chunk.jump_tables.clone(),
    lines: proto.chunk.lines.clone(),
    param_checks: proto.chunk.param_checks.clone(),
    upvalue_descriptors: proto.upvalues.clone(),
    is_method: proto.is_method,
    owning_class_name: proto.owning_class_name.clone(),
    home,
    root_globals,
  });

  if let TransferNode::Proto(cf) = &mut arena[idx as usize] {
    cf.data = full_data;
  }
  Ok(TransferValue::Ref(idx))
}

/// Scans `chunk`'s own bytecode (NOT any nested prototype's; each
/// gets its own independent call via `capture_prototype`) for every
/// distinct name a `GetGlobal`/`SetGlobal`/`AssignGlobal` references,
/// and snapshots whichever of those currently resolve against the
/// SOURCE vm's root table. A name that isn't currently defined is
/// simply skipped; exactly like today, it'll fail with an ordinary
/// `UndefinedError` on the destination only if the function actually
/// reaches that instruction, never up front.
fn capture_root_globals(
  vm: &VM,
  chunk: &Chunk,
  arena: &mut Vec<TransferNode>,
  memo: &mut FxHashMap<usize, u32>,
) -> Result<Vec<(String, TransferValue)>, String> {
  let mut seen: FxHashSet<String> = FxHashSet::default();
  let mut out = Vec::new();
  for instr in &chunk.code {
    let name_const = match *instr {
      Instr::GetGlobal { name_const, .. } => name_const,
      Instr::SetGlobal { name_const, .. } => name_const,
      Instr::AssignGlobal { name_const, .. } => name_const,
      _ => continue,
    };
    let Some(&name_val) = chunk.constants.get(name_const as usize) else {
      continue;
    };
    if !name_val.is_string() {
      continue;
    }
    let name = name_val.as_str();
    if !seen.insert(name.to_string()) {
      continue;
    }
    let Some(current) = vm.lookup_global(name) else {
      continue;
    };
    let captured = capture_value(vm, current, arena, memo).map_err(|e| {
      format!(
        "cannot send this function across isolates: global '{}' it \
         depends on can't cross: {}",
        name, e
      )
    })?;
    out.push((name.to_string(), captured));
  }
  Ok(out)
}

fn capture_class(
  vm: &VM,
  v: Value,
  arena: &mut Vec<TransferNode>,
  memo: &mut FxHashMap<usize, u32>,
) -> Result<TransferValue, String> {
  let (name, gmod) = {
    let c = v.as_class();
    (c.name.clone(), c.globals_module)
  };

  // A builtin error class (`Error`, `TypeError`, ...) is installed
  // fresh by `VM::init` on every isolate isolate already; it isn't
  // declared in any module a `Home` could point at, and structurally
  // cloning it would produce a class that LOOKS the same but isn't the
  // exact object the destination's own `VM::raise`/`Instr::Raise`
  // checks against, breaking `raise`/`catch` for anything built from
  // it (a plain `Error('msg')`, most commonly). Resolving it by name
  // against the destination's own prelude instead is both correct and
  // free; no module load, no clone.
  if gmod.is_none()
    && vm
      .builtin_errors
      .get(name.as_str())
      .is_some_and(|c| c.equals(&v))
  {
    return Ok(TransferValue::Prelude(name));
  }

  // Cheap path first, same shape as `capture_closure`'s: a
  // module-scoped class is always bound under its own name (every
  // class reaches `Instr::FinalizeClass`, which already enforces
  // that), so this covers every module-scoped class.
  if let Some(m) = gmod {
    let home = Home {
      path: m.as_module().path.clone(),
    };
    if lookup_named_source(vm, &home, &name).is_some_and(|f| f.equals(&v)) {
      return Ok(TransferValue::Named {
        home,
        name,
        kind: NamedKind::Class,
      });
    }
  }

  // Structural fallback; the only option for a class declared
  // directly in the main script (`gmod: None`, never a valid `Home`
  // for the Named path: see `Home`'s own docs).
  capture_class_structural(vm, v, arena, memo)
}

/// Full structural transplant of a class: its superclass chain,
/// methods, field/static layout, and current static values, all
/// captured the same recursive way as everything else (a method is
/// just another closure; a superclass is just another class). The
/// constructor is deliberately NOT captured as its own field; it's
/// always either `None` or `methods[name]` (see `Instr::
/// FinalizeClass`), so `materialize` just re-derives it from the
/// rebuilt `methods` map instead of risking the two drifting apart.
///
/// Statics are mutable, shared, per-class state on the source side;
/// captured as a one-time SNAPSHOT here, same as an upvalue or a
/// root global. Once a class crosses into an isolate, its statics
/// there are independent: neither side's later mutations are visible
/// to the other. There's no other sound option in a shared-nothing
/// model: see this module's own top-level docs.
fn capture_class_structural(
  vm: &VM,
  v: Value,
  arena: &mut Vec<TransferNode>,
  memo: &mut FxHashMap<usize, u32>,
) -> Result<TransferValue, String> {
  let idx = arena.len() as u32;
  let name = v.as_class().name.clone();
  arena.push(TransferNode::Class(CapturedClass {
    name,
    superclass: None,
    methods: Vec::new(),
    field_slots: Vec::new(),
    field_count: 0,
    own_field_initializer: None,
    static_slots: Vec::new(),
    statics: Vec::new(),
  }));
  memo.insert(v.as_obj() as usize, idx);

  let (
    superclass_src,
    methods_src,
    field_slots,
    field_count,
    own_init_src,
    static_slots,
    statics_src,
  ) = {
    let c = v.as_class();
    (
      c.superclass,
      c.methods.clone(),
      c.field_slots.clone(),
      c.field_count,
      c.own_field_initializer,
      c.static_slots.clone(),
      c.statics.iter().map(|cell| cell.get()).collect::<Vec<_>>(),
    )
  };

  let superclass = match superclass_src {
    Some(s) => Some(capture_value(vm, s, arena, memo)?),
    None => None,
  };
  let mut methods = Vec::with_capacity(methods_src.len());
  for (mname, mval) in methods_src {
    methods.push((mname, capture_value(vm, mval, arena, memo)?));
  }
  let own_field_initializer = match own_init_src {
    Some(f) => Some(capture_value(vm, f, arena, memo)?),
    None => None,
  };
  let mut statics = Vec::with_capacity(statics_src.len());
  for s in statics_src {
    statics.push(capture_value(vm, s, arena, memo)?);
  }

  if let TransferNode::Class(cc) = &mut arena[idx as usize] {
    cc.superclass = superclass;
    cc.methods = methods;
    cc.field_slots = field_slots.into_iter().collect();
    cc.field_count = field_count;
    cc.own_field_initializer = own_field_initializer;
    cc.static_slots = static_slots.into_iter().collect();
    cc.statics = statics;
  }
  Ok(TransferValue::Ref(idx))
}

/// `None` means the class/method was declared straight in the running
/// script rather than through `import`; rejected, since class/
/// method resolution always goes through the `Named`/`Home` path
/// (never the structural one plain functions/lambdas get): see
/// `Home`'s own docs for why that path can't target the main script.
/// Read-only lookup against the SOURCE vm, which; unlike a
/// destination isolate; already has `home` fully loaded (that's
/// where `v` itself came from). Used only to verify a captured
/// class/method round-trips to the exact value it claims to name,
/// before it's ever allowed to cross a thread boundary.
fn lookup_named_source(vm: &VM, home: &Home, name: &str) -> Option<Value> {
  vm.modules
    .get(&home.path)
    .and_then(|&m| m.as_module().namespace.get(name))
}

/// The reverse of `lookup_named_source`: which name (if any) `home`'s
/// namespace currently binds `v` under. Used for plain functions
/// specifically, since a function's OWN declared name
/// (`ObjFunction::name`) can't be trusted to be its binding name: a
/// `def square(n) {}` declaration happens to make both the same, but
/// a lambda literal (`var greet = @(name) { ... }`) carries the
/// parser's own internal placeholder in `name` instead, unrelated to
/// `greet`. Ties (the same closure re-exported under two names)
/// resolve to whichever one iteration happens to see first; either is
/// equally correct to hand back, since both resolve to the exact same
/// value again on the destination side.
fn find_binding_name(vm: &VM, home: &Home, v: Value) -> Option<String> {
  let &module = vm.modules.get(&home.path)?;
  let namespace = &module.as_module().namespace;
  for (name, &slot) in namespace.names.iter() {
    if namespace.slots[slot as usize].get().equals(&v) {
      return Some(name.clone());
    }
  }
  None
}

// ---------------------------------------------------------------------
// materialize: TransferGraph -> destination heap
// ---------------------------------------------------------------------

/// Rebuilds `graph` as a real `Value` on `vm`'s own heap, loading
/// whatever module a `Named` value or a structurally-captured
/// closure's home needs (once per destination isolate; cheap and a
/// no-op on every later call once loaded). May allocate heavily and
/// may itself run arbitrary Zuri top-level code (loading a module),
/// so; unlike `capture`; this needs `&mut VM`.
pub fn materialize(vm: &mut VM, graph: &TransferGraph) -> Result<Value, String> {
  if graph.arena.is_empty() {
    return match &graph.root {
      TransferValue::Nil => Ok(Value::nil()),
      TransferValue::Bool(b) => Ok(Value::bool(*b)),
      TransferValue::Number(n) => Ok(Value::number(*n)),
      TransferValue::BigInt(b) => Ok(vm.heap_mut().alloc_bigint(b.clone())),
      TransferValue::Str(s) => Ok(vm.heap_mut().alloc_string(s.clone())),
      TransferValue::Bytes(b) => Ok(vm.heap_mut().alloc_bytes(b.clone())),
      TransferValue::Range { lower, upper, step } => {
        let v = vm.heap_mut().alloc_range(*lower, *upper);
        v.range_set_step(*step);
        Ok(v)
      },
      TransferValue::Native {
        name,
        min_arity,
        variadic,
        is_method,
        func,
      } => Ok(vm.heap_mut().alloc_native(NativeFunction {
        name,
        min_arity: *min_arity,
        variadic: *variadic,
        is_method: *is_method,
        func: *func,
      })),
      TransferValue::Named { home, name, kind } => resolve_named(vm, home, name, kind),
      TransferValue::Prelude(name) => vm.lookup_global(name).ok_or_else(|| {
        format!(
          "internal error: builtin error class '{}' missing from the \
           destination isolate's own prelude",
          name
        )
      }),
      TransferValue::ChannelHandle(state) => Ok(
        vm.heap_mut()
          .alloc_ptr(pool::CHANNEL_PTR_TYPE, state.clone()),
      ),
      TransferValue::IsolateHandle(state) => Ok(
        vm.heap_mut()
          .alloc_ptr(pool::ISOLATE_PTR_TYPE, state.clone()),
      ),
      TransferValue::Module { key, binding } => resolve_module(vm, key, binding.as_deref()),
      TransferValue::Ref(_) => unreachable!(),
    };
  }

  let outer_mark = vm.pin_values(std::iter::empty());
  let mut node_pin: Vec<Option<usize>> = vec![None; graph.arena.len()];
  let result = materialize_value(vm, &graph.root, &graph.arena, &mut node_pin);
  vm.unpin(outer_mark);
  result
}

fn materialize_value(
  vm: &mut VM,
  tv: &TransferValue,
  arena: &[TransferNode],
  node_pin: &mut Vec<Option<usize>>,
) -> Result<Value, String> {
  match tv {
    TransferValue::Nil => Ok(Value::nil()),
    TransferValue::Bool(b) => Ok(Value::bool(*b)),
    TransferValue::Number(n) => Ok(Value::number(*n)),
    TransferValue::BigInt(b) => Ok(vm.heap_mut().alloc_bigint(b.clone())),
    TransferValue::Str(s) => Ok(vm.heap_mut().alloc_string(s.clone())),
    TransferValue::Bytes(b) => Ok(vm.heap_mut().alloc_bytes(b.clone())),
    TransferValue::Range { lower, upper, step } => {
      let v = vm.heap_mut().alloc_range(*lower, *upper);
      v.range_set_step(*step);
      Ok(v)
    },
    TransferValue::Native {
      name,
      min_arity,
      variadic,
      is_method,
      func,
    } => Ok(vm.heap_mut().alloc_native(NativeFunction {
      name,
      min_arity: *min_arity,
      variadic: *variadic,
      is_method: *is_method,
      func: *func,
    })),
    TransferValue::Named { home, name, kind } => resolve_named(vm, home, name, kind),
    TransferValue::Prelude(name) => vm.lookup_global(name).ok_or_else(|| {
      format!(
        "internal error: builtin error class '{}' missing from the \
         destination isolate's own prelude",
        name
      )
    }),
    TransferValue::ChannelHandle(state) => Ok(
      vm.heap_mut()
        .alloc_ptr(pool::CHANNEL_PTR_TYPE, state.clone()),
    ),
    TransferValue::IsolateHandle(state) => Ok(
      vm.heap_mut()
        .alloc_ptr(pool::ISOLATE_PTR_TYPE, state.clone()),
    ),
    TransferValue::Module { key, binding } => resolve_module(vm, key, binding.as_deref()),
    TransferValue::Ref(idx) => materialize_ref(vm, *idx, arena, node_pin),
  }
}

fn materialize_ref(
  vm: &mut VM,
  idx: u32,
  arena: &[TransferNode],
  node_pin: &mut Vec<Option<usize>>,
) -> Result<Value, String> {
  if let Some(p) = node_pin[idx as usize] {
    return Ok(vm.pinned(p));
  }
  match &arena[idx as usize] {
    TransferNode::List(items) => {
      // Sized (and nil-filled) up front, then filled in place; lets a
      // self-referential/cyclic list resolve to this SAME placeholder
      // instead of recursing forever.
      let placeholder = vm.heap_mut().alloc_list(vec![Value::nil(); items.len()]);
      let p = vm.pin_values([placeholder]);
      node_pin[idx as usize] = Some(p);
      for (i, item_tv) in items.iter().enumerate() {
        let item_val = materialize_value(vm, item_tv, arena, node_pin)?;
        vm.pinned(p).list_set(i, item_val);
      }
      Ok(vm.pinned(p))
    },
    TransferNode::Dict(pairs) => {
      let placeholder = vm.heap_mut().alloc_dict(Vec::new());
      let p = vm.pin_values([placeholder]);
      node_pin[idx as usize] = Some(p);
      for (k_tv, v_tv) in pairs {
        let k_val = materialize_value(vm, k_tv, arena, node_pin)?;
        let kp = vm.pin_values([k_val]);
        let v_val = materialize_value(vm, v_tv, arena, node_pin)?;
        let k_val = vm.pinned(kp);
        vm.pinned(p).dict_set(k_val, v_val);
      }
      Ok(vm.pinned(p))
    },
    TransferNode::Instance { class, fields } => {
      let class_val = materialize_value(vm, class, arena, node_pin)?;
      let cp = vm.pin_values([class_val]);
      let field_count = fields.len();
      let placeholder = {
        let c = vm.pinned(cp);
        vm.heap_mut().alloc_instance(c, field_count)
      };
      let p = vm.pin_values([placeholder]);
      node_pin[idx as usize] = Some(p);
      for (i, f_tv) in fields.iter().enumerate() {
        let f_val = materialize_value(vm, f_tv, arena, node_pin)?;
        let inst_val = vm.pinned(p);
        inst_val.as_instance().fields[i].set(f_val);
        write_barrier(inst_val.as_obj());
      }
      Ok(vm.pinned(p))
    },
    TransferNode::BoundMethod { receiver, method } => {
      let r_val = materialize_value(vm, receiver, arena, node_pin)?;
      let rp = vm.pin_values([r_val]);
      let m_val = materialize_value(vm, method, arena, node_pin)?;
      let r_val = vm.pinned(rp);
      let bound = vm.heap_mut().alloc_bound_method(r_val, m_val);
      let p = vm.pin_values([bound]);
      node_pin[idx as usize] = Some(p);
      Ok(bound)
    },
    TransferNode::Class(cc) => materialize_class(vm, cc, idx, arena, node_pin),
    TransferNode::Ptr(slot) => {
      let taken = slot.lock().unwrap().take();
      let Some((type_name, payload)) = taken else {
        return Err(
          "this native resource was already consumed by an earlier read \
          ; an isolate result or channel message containing a native \
           pointer can only be materialized once, by whichever join()/\
           recv() reaches it first"
            .to_string(),
        );
      };
      let val = vm.heap_mut().alloc_ptr_boxed(type_name, payload);
      let p = vm.pin_values([val]);
      node_pin[idx as usize] = Some(p);
      Ok(val)
    },
    TransferNode::Proto(proto) => materialize_prototype(vm, proto, idx, arena, node_pin),
    TransferNode::Closure { proto, upvalues } => {
      let proto_val = materialize_value(vm, proto, arena, node_pin)?;
      let proto_pin = vm.pin_values([proto_val]);

      // Phase 1: a placeholder (Closed(nil)) Upvalue object per
      // captured slot, and the closure itself built around them,
      // BEFORE materializing what they actually hold; lets a
      // closure that captures ITSELF (a recursive lambda assigned to
      // the very local it closes over) resolve back to this exact
      // closure instead of recursing forever, the same placeholder-
      // first trick `List`'s own arm above uses.
      let mut shell_pins = Vec::with_capacity(upvalues.len());
      for _ in upvalues {
        let shell = vm
          .heap_mut()
          .alloc_upvalue(UpvalueState::Closed(Value::nil()));
        shell_pins.push(vm.pin_values([shell]));
      }
      let proto_val = vm.pinned(proto_pin);
      let shells: smallvec::SmallVec<[Value; 2]> =
        shell_pins.iter().map(|&p| vm.pinned(p)).collect();
      let closure_val = vm.heap_mut().alloc_closure(ObjClosure {
        function: proto_val,
        upvalues: shells,
      });
      write_barrier(closure_val.as_obj());
      let p = vm.pin_values([closure_val]);
      node_pin[idx as usize] = Some(p);

      // Phase 2: materialize what each upvalue actually holds (which
      // may now safely reference the closure above via `node_pin`),
      // and patch the corresponding shell in place; sound because
      // `Obj::Upvalue` is a `Cell`, mutable after creation, and every
      // `ObjClosure.upvalues` entry is a `Value` (a pointer/handle),
      // so patching what it points AT is visible through every copy
      // of that handle, not just this one.
      for (uv_tv, &shell_pin) in upvalues.iter().zip(shell_pins.iter()) {
        let val = materialize_value(vm, uv_tv, arena, node_pin)?;
        let shell = vm.pinned(shell_pin);
        shell.as_upvalue().set(UpvalueState::Closed(val));
        write_barrier(shell.as_obj());
      }

      Ok(vm.pinned(p))
    },
  }
}

/// Rebuilds `cc` (`arena[idx]`, always a `TransferNode::Class`) as a
/// real `Obj::Class` `Value` on `vm`'s own heap.
///
/// Allocates an EMPTY class shell first and memoizes it immediately,
/// then fills it in place via the same mutable `RefCell<ObjClass>`
/// API `Instr::MakeClass`/`SetMethod`/`DeclareStatic`/... already use
/// to build a class up progressively at runtime; unlike a function
/// prototype, `ObjClass` was always designed to be mutated after its
/// own allocation (see that type's own docs), so there's no need for
/// `Closure`'s own two-phase shell trick here. The empty shell being
/// memoized up front is what lets a method body that references its
/// OWN class by name (a static method calling another static method
/// on the same class, say) resolve back to this exact class instead
/// of recursing forever.
fn materialize_class(
  vm: &mut VM,
  cc: &CapturedClass,
  idx: u32,
  arena: &[TransferNode],
  node_pin: &mut Vec<Option<usize>>,
) -> Result<Value, String> {
  if let Some(existing) = vm.lookup_global(&cc.name) {
    if existing.is_class() {
      let p = vm.pin_values([existing]);
      node_pin[idx as usize] = Some(p);
      return Ok(existing);
    }
  }

  let placeholder = vm.heap_mut().alloc_class(crate::vm::object::ObjClass {
    name: cc.name.clone(),
    superclass: None,
    methods: FxHashMap::default(),
    field_slots: FxHashMap::default(),
    field_count: 0,
    own_field_initializer: None,
    constructor: None,
    static_slots: FxHashMap::default(),
    statics: Vec::new(),
    // Structurally-captured classes are always main-script-scoped --
    // a module-scoped one always takes the cheap `Named` path instead
    // (see `capture_class`); so there's no home module to point at.
    globals_module: None,
    display: Default::default(),
    subclassed: Default::default(),
    finalized: Default::default(),
  });
  let p = vm.pin_values([placeholder]);
  node_pin[idx as usize] = Some(p);

  let superclass = match &cc.superclass {
    Some(s) => Some(materialize_value(vm, s, arena, node_pin)?),
    None => None,
  };
  let mut methods = FxHashMap::default();
  for (name, tv) in &cc.methods {
    let m = materialize_value(vm, tv, arena, node_pin)?;
    methods.insert(name.clone(), m);
  }
  let own_field_initializer = match &cc.own_field_initializer {
    Some(f) => Some(materialize_value(vm, f, arena, node_pin)?),
    None => None,
  };
  let mut statics = Vec::with_capacity(cc.statics.len());
  for tv in &cc.statics {
    statics.push(std::cell::Cell::new(materialize_value(
      vm, tv, arena, node_pin,
    )?));
  }
  // Same derivation `Instr::FinalizeClass` uses: the constructor, if
  // any, is the method literally named "@new"; NOT one sharing the
  // class's own name (that was the bug here: every structurally
  // transferred class silently lost its constructor, since no class
  // is ever actually named "@new").
  let constructor = methods.get("@new").copied();

  let class_val = vm.pinned(p);
  {
    let mut c = class_val.as_class_mut();
    c.superclass = superclass;
    c.methods = methods;
    c.field_slots = cc.field_slots.iter().cloned().collect();
    c.field_count = cc.field_count;
    c.own_field_initializer = own_field_initializer;
    c.constructor = constructor;
    c.static_slots = cc.static_slots.iter().cloned().collect();
    c.statics = statics;
  }
  class_val.as_class().fill_display(class_val.to_bits());
  class_val.as_class().finalized.set(true);
  // Which of these methods a class elsewhere overrides is not carried
  // across, so none is taken as the same for a whole family of classes.
  for m in class_val.as_class().methods.values() {
    if m.is_closure() {
      m.as_closure().function.as_func().jit.overridden.set(true);
    }
  }
  write_barrier(class_val.as_obj());
  vm.define_global(cc.name.clone(), class_val);
  Ok(class_val)
}

fn materialize_constant(
  vm: &mut VM,
  tv: &TransferValue,
  arena: &[TransferNode],
  node_pin: &mut Vec<Option<usize>>,
) -> Result<Value, String> {
  match tv {
    TransferValue::Str(s) => Ok(vm.heap_mut().alloc_string_old(s.clone())),
    TransferValue::BigInt(b) => Ok(vm.heap_mut().alloc_bigint_old(b.clone())),
    _ => materialize_value(vm, tv, arena, node_pin),
  }
}

fn materialize_prototype(
  vm: &mut VM,
  proto: &CapturedFunction,
  idx: u32,
  arena: &[TransferNode],
  node_pin: &mut Vec<Option<usize>>,
) -> Result<Value, String> {
  let proto_id = proto.data.id;
  let key = format!("__proto_{}", proto_id);
  if let Some(cached_val) = vm.lookup_global(&key) {
    if cached_val.is_func() {
      let p = vm.pin_values([cached_val]);
      node_pin[idx as usize] = Some(p);
      return Ok(cached_val);
    }
  }

  let globals_module = match &proto.data.home {
    None => None,
    Some(home) => Some(load_module_cached(vm, &home.path)?),
  };

  // One pin per constant, but the pin INDEX has to be remembered rather
  // than counted from a mark: a constant that is itself a nested
  // prototype sends `materialize_prototype` back round, and that pins
  // the function it builds along with every constant of its own, so the
  // n-th constant of this chunk is not the n-th pin past the mark.
  let mut pins = Vec::with_capacity(proto.data.constants.len());
  for c in &proto.data.constants {
    let v = materialize_constant(vm, c, arena, node_pin)?;
    pins.push(vm.pin_values([v]));
  }
  let constants: Vec<Value> = pins.into_iter().map(|p| vm.pinned(p)).collect();

  let mut chunk = Chunk::new();
  chunk.code = proto.data.code.clone();
  chunk.constants = constants;
  chunk.jump_tables = proto.data.jump_tables.clone();
  chunk.lines = proto.data.lines.clone();
  chunk.param_checks = proto.data.param_checks.clone();
  let jit = JitInfo::new(chunk.code.len());

  let source_path: Rc<str> = Rc::from(
    proto
      .data
      .home
      .as_ref()
      .map(|h| h.path.as_str())
      .unwrap_or("<isolate>"),
  );

  let fn_obj = ObjFunction {
    name: proto.data.name.clone(),
    variadic: proto.data.variadic,
    chunk,
    arity: proto.data.arity,
    num_registers: proto.data.num_registers,
    upvalues: proto.data.upvalue_descriptors.clone(),
    is_method: proto.data.is_method,
    owning_class_name: proto.data.owning_class_name.clone(),
    source_path,
    globals_module,
    jit,
  };
  let fn_val = vm.heap_mut().alloc_function(fn_obj);
  write_barrier(fn_val.as_obj());
  let p = vm.pin_values([fn_val]);
  node_pin[idx as usize] = Some(p);

  for (name, tv) in &proto.data.root_globals {
    let val = materialize_value(vm, tv, arena, node_pin)?;
    vm.define_global(name.clone(), val);
  }

  vm.define_global(key, fn_val);

  Ok(vm.pinned(p))
}

/// Loads the module `key` names on this isolate and hands back either
/// the module itself or the promoted binding an `import` of it would
/// have produced, matching whichever shape crossed the boundary.
fn resolve_module(vm: &mut VM, key: &str, binding: Option<&str>) -> Result<Value, String> {
  let module = crate::vm::modules::load_by_cache_key(vm, key)
    .map_err(|e| format!("could not load module '{}' on this isolate: {}", key, e))?;

  let Some(bind_name) = binding else {
    return Ok(module);
  };

  let promoted = {
    let m = module.as_module();
    m.namespace.get(bind_name).filter(|v| v.is_callable())
  };

  Ok(vm.heap_mut().alloc_module_binding(ObjModuleBinding {
    module,
    promoted,
    bind_name: bind_name.to_string(),
  }))
}

fn resolve_named(vm: &mut VM, home: &Home, name: &str, kind: &NamedKind) -> Result<Value, String> {
  let module = load_module_cached(vm, &home.path)?;

  match kind {
    NamedKind::Function => {
      let found = module
        .as_module()
        .namespace
        .get(name)
        .ok_or_else(|| format!("'{}' is not defined in {}", name, home.describe()))?;
      if !found.is_closure() {
        return Err(format!(
          "'{}' in {} is no longer a function",
          name,
          home.describe()
        ));
      }
      Ok(found)
    },
    NamedKind::Class => {
      let found = module
        .as_module()
        .namespace
        .get(name)
        .ok_or_else(|| format!("'{}' is not defined in {}", name, home.describe()))?;
      if !found.is_class() {
        return Err(format!(
          "'{}' in {} is no longer a class",
          name,
          home.describe()
        ));
      }
      Ok(found)
    },
    NamedKind::Method { class_name } => {
      let class_val = module
        .as_module()
        .namespace
        .get(class_name)
        .ok_or_else(|| format!("'{}' is not defined in {}", class_name, home.describe()))?;
      if !class_val.is_class() {
        return Err(format!(
          "'{}' in {} is no longer a class",
          class_name,
          home.describe()
        ));
      }
      let method = class_val.as_class().methods.get(name).copied();
      method.ok_or_else(|| {
        format!(
          "'{}' has no method '{}' in {}",
          class_name,
          name,
          home.describe()
        )
      })
    },
  }
}

fn load_module_cached(vm: &mut VM, path: &str) -> Result<Value, String> {
  if let Some(&m) = vm.modules.get(path) {
    return Ok(m);
  }
  crate::vm::modules::load_from_candidate(vm, Path::new(path), path)
    .map_err(|e| vm.describe_error(e))
}

/// Captures a single value as a 1-element arguments list without
/// allocating a temporary Zuri list on the source heap.
pub fn capture_as_args_list(vm: &VM, item: Value) -> Result<TransferGraph, String> {
  let mut arena = Vec::new();
  let mut memo: FxHashMap<usize, u32> = FxHashMap::default();
  let item_tv = capture_value(vm, item, &mut arena, &mut memo)?;
  let list_idx = arena.len() as u32;
  arena.push(TransferNode::List(vec![item_tv]));
  Ok(TransferGraph {
    root: TransferValue::Ref(list_idx),
    arena,
  })
}
