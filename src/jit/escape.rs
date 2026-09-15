//! Escape analysis; phase 1 of a multi-phase effort (see this
//! project's own perf-investigation notes for the full roadmap).
//!
//! This phase answers one narrow, purely intraprocedural question,
//! soundly: for a value defined at bytecode position `alloc_ip` within
//! a single function, can every use of it (and everything it might be
//! copied into, within that same function) be proven to never let it
//! outlive this function's own execution? If yes, the value is safe to
//! allocate somewhere cheaper than the GC heap (a stack slot, or
//! eventually scalar-replaced registers entirely): see `jit::codegen`
//! for how that's eventually used; this module only ever proves facts,
//! never changes codegen by itself.
//!
//! # What this phase deliberately doesn't do yet
//!
//! - No interprocedural reasoning at all: a tracked value passed as an
//!   argument to any `Call`/`Invoke`/`InvokeSuper`/`CallSuperCtor`, or
//!   used as the receiver of one, is unconditionally treated as
//!   escaping; there's no call-graph, no per-function summary, no
//!   attempt to prove a callee doesn't retain what it's handed. Real
//!   patterns like `tree_with(depth).count()` (binary-tree's own hot
//!   path) won't be proven non-escaping by this phase alone, since
//!   `tree_with`'s return crosses a call boundary into its caller, and
//!   `count()` is itself a call. That needs Phase 2 (interprocedural
//!   summaries over a call graph) at minimum.
//! - No transitive containment: storing a tracked value into another
//!   object's field (`SetField`/`SetFieldInit`/`SetMethod`/
//!   `DeclareStatic`/`SetIndex`'s `src` operand, or folding it into a
//!   `MakeList`/`MakeDict`) is unconditionally treated as escaping,
//!   even if that container is itself later proven non-escaping. A
//!   future phase could propagate "container doesn't escape" down to
//!   its contents; this one doesn't attempt it.
//! - No scalar replacement; proving non-escape doesn't yet do
//!   anything; wiring a proof into actual stack allocation is a
//!   separate, later, codegen-side change.
//!
//! # Why the safe-use allowlist is so narrow
//!
//! Zuri lets user code hook into what look like "plain" operations:
//! `GetField` on a name that resolves to a method (not a field)
//! implicitly wraps the receiver in a freshly-allocated `BoundMethod`
//!; which stores the receiver into a new heap object, a real escape
//! path a naive reading of "GetField just reads a field" would miss
//! entirely. Arithmetic/bitwise/concat ops dispatch to user-defined
//! operator overloads (`@add`, `@sub`, ...) for non-numeric operands.
//! Given how easy it is to miss one of these, this module is built
//! default-deny: an instruction's use of a possibly-tracked register is
//! escaping unless that exact instruction is explicitly verified safe
//! below, by reading its real runtime handler, not assumed from its
//! name. A false "doesn't escape" here is a genuine memory-safety bug
//! once Phase 2/3 acts on it (a dangling pointer once the stack frame
//! that "owned" a value the GC never knew about returns); a false
//! "escapes" only ever costs a missed optimization. This trade is
//! deliberate and non-negotiable for this module.
//!
//! (`Print` was considered for the escaping side on the theory that it
//! might dispatch to a user-overridden `to_string()`; checked against
//! the actual handler, `jit::runtime::zuri_jit_print`, which just calls
//! `println!("{}", v)`, i.e. Rust's own `Display for Value`. For
//! `Obj::Instance` that's a hardcoded `write!(f, "<instance of {}>",
//! i.class.as_class().name)` (see `value.rs`); reads the class's own
//! name, never retains or dispatches on the instance itself. No
//! user-overridable `to_string()` exists in this codebase as of this
//! writing, so `Print` is verified safe below; if/when that spec
//! feature is implemented, this is the exact justification that stops
//! holding and `Print` needs to move to the escaping side.)
//!
//! Verified-safe operations (see each match arm below for the specific
//! code path that justifies it):
//! - `Move` (a plain register copy; propagates tracking, never
//!   escapes).
//! - `Eq`/`Neq`/`EqImm`/`NeqImm` (`Value::equals` on two `Obj::
//!   Instance`s is `std::ptr::eq`, unconditionally: see
//!   `value.rs`'s own `equals`; never a user-code dispatch).
//! - `JmpIfFalse`/`JmpIfTrue` (`Value::is_falsey` treats every heap
//!   kind except `Str`/`Bytes`/`BigInt` as unconditionally not falsey,
//!   decided from the tag byte alone: see `codegen::emit_is_falsey`'s
//!   own docs; never a payload read or user-code dispatch for an
//!   `Instance`).
//! - `Print` (see the paragraph above).
//! - Being the container (never the stored value) of `SetField`/
//!   `SetFieldInit`/`SetMethod`/`DeclareStatic`/`SetIndex`; writing
//!   into your own field/slot doesn't hand your identity to anything
//!   new.
//! - Being the container/index-key operand (never the stored value) of
//!   `GetIndex`/`SetIndex`; Dict key lookup is also `Value::equals`,
//!   same `ptr::eq` guarantee as above.
//!
//! Everything else that reads a tracked register; including
//! `GetField`'s `obj` (the `BoundMethod`-wrapping risk above),
//! `Return`, `SetGlobal`/`AssignGlobal`, `SetUpval`, `Closure`
//! capturing it, any `Call`/`Invoke`/`InvokeSuper`/`CallSuperCtor`
//! operand, `Raise`, arithmetic/bitwise/concat/unary ops; is
//! escaping.

use rustc_hash::FxHashSet;
use smallvec::{SmallVec, smallvec};

use crate::jit::typeflow;
use crate::vm::chunk::Instr;
use crate::vm::object::{ObjClass, ObjFunction, UpvalueDescriptor};

/// Resolves `GetField`'s `BoundMethod`-wrapping risk (see this
/// module's top-level docs) for one specific, already-known class:
/// which of its own field names are safe to read via `GetField`
/// because no method of that same name would ever shadow them.
/// Computed once, from a live `&ObjClass`, by whatever caller has VM
/// access to resolve one (this module itself deliberately never
/// touches the VM: see its own docs); sound as a permanent fact,
/// not a one-shot snapshot, because Zuri classes are immutable after
/// construction (NOTES.md: "new fields and methods cannot be added at
/// runtime": see `ObjFunction::owning_class_name`'s own docs for the
/// same reasoning). Currently only consulted for a method's own
/// `self` (register 0): see `analyze_one`/`compute_param_summary`'s
/// own docs on why that's the one case resolvable without further
/// receiver-type inference.
#[derive(Clone)]
pub struct ClassFieldSafety {
  safe_field_names: FxHashSet<String>,
}

impl ClassFieldSafety {
  /// `field_slots`/`methods` are pre-merged with every ancestor class
  /// already (see `ObjClass`'s own field docs), so this automatically
  /// accounts for inherited collisions too; a subclass overriding
  /// an inherited field with a same-named method (or vice versa) is
  /// exactly as unsafe as a same-class collision, and already shows
  /// up here without any extra superclass-walking.
  pub fn from_class(class: &ObjClass) -> Self {
    let safe_field_names = class
      .field_slots
      .keys()
      .filter(|name| !class.methods.contains_key(*name))
      .cloned()
      .collect();
    ClassFieldSafety { safe_field_names }
  }

  fn is_field_safe(&self, name: &str) -> bool {
    self.safe_field_names.contains(name)
  }
}

/// A bitset over bytecode register indices, tracking which registers
/// may currently hold a reference to the one allocation this analysis
/// run is tracking; the "may" (union-at-merge) counterpart to
/// `typeflow::RegSet`'s "must" (intersect-at-merge) semantics. Built as
/// its own small type, deliberately not sharing `RegSet` itself,
/// specifically so this newer, less-proven analysis can never
/// accidentally perturb `typeflow`'s own already-trusted "must"
/// fixed-points; the two need different merge operators (union here,
/// intersection there), and keeping them as distinct types makes it a
/// compile error to accidentally call the wrong one, rather than a
/// silent logic bug from calling `and_assign` where `or_assign`
/// belongs.
#[derive(Clone, PartialEq, Eq)]
struct AliasSet {
  words: Vec<u64>,
}

impl AliasSet {
  fn word_count(num_registers: usize) -> usize {
    num_registers.div_ceil(64).max(1)
  }

  /// Nothing aliases yet; the correct seed for every block except the
  /// one immediately following the allocation site itself (a "may"
  /// analysis starts empty and only grows via union, the mirror image
  /// of `RegSet::full`'s role in a "must" analysis).
  fn empty(num_registers: usize) -> Self {
    AliasSet {
      words: vec![0u64; Self::word_count(num_registers)],
    }
  }

  fn get(&self, r: u8) -> bool {
    let r = r as usize;
    let word = r / 64;
    if word >= self.words.len() {
      return false;
    }
    (self.words[word] >> (r % 64)) & 1 != 0
  }

  fn set(&mut self, r: u8, v: bool) {
    let r = r as usize;
    let word = r / 64;
    if word >= self.words.len() {
      return;
    }
    if v {
      self.words[word] |= 1 << (r % 64);
    } else {
      self.words[word] &= !(1 << (r % 64));
    }
  }

  /// Union merge; a "may" analysis's fixed point grows monotonically
  /// via union at every merge point (if either predecessor path could
  /// have left the allocation in this register, the merged state must
  /// say so too), the exact mirror of `RegSet::and_assign`'s
  /// intersection for a "must" analysis.
  fn or_assign(&mut self, other: &AliasSet) {
    for (a, b) in self.words.iter_mut().zip(other.words.iter()) {
      *a |= b;
    }
  }
}

/// A bitset over registers for a "must" (intersect-at-merge) fact;
/// used below by `self_reference_facts` to prove a register definitely
/// (on every path, not just possibly) still holds an unmodified
/// self-reference. A separate type from `AliasSet`, on purpose: see
/// that type's own docs on why mixing "may" and "must" merge operators
/// under one type invites a silent logic bug instead of a compile
/// error.
#[derive(Clone, PartialEq, Eq)]
pub struct MustSet {
  words: SmallVec<[u64; 4]>,
}

impl MustSet {
  fn word_count(num_registers: usize) -> usize {
    num_registers.div_ceil(64).max(1)
  }

  /// Nothing proven; the correct seed for the entry block (register
  /// 0's caller-supplied argument is never statically a self-
  /// reference, nor is anything else, before any code has run).
  fn empty(num_registers: usize) -> Self {
    MustSet {
      words: smallvec![0u64; Self::word_count(num_registers)],
    }
  }

  /// Everything (optimistically) proven; the correct seed for every
  /// other block, so a real predecessor's facts only ever narrow it
  /// down via `and_assign`, never widen it (the same reasoning
  /// `typeflow::RegSet::full` documents for its own analogous role).
  fn full(num_registers: usize) -> Self {
    let words = Self::word_count(num_registers);
    let mut v = smallvec![u64::MAX; words];
    let extra_bits = words * 64 - num_registers;
    if extra_bits > 0
      && let Some(last) = v.last_mut()
    {
      *last >>= extra_bits;
    }
    MustSet { words: v }
  }

  pub(crate) fn get(&self, r: u8) -> bool {
    let r = r as usize;
    let word = r / 64;
    if word >= self.words.len() {
      return false;
    }
    (self.words[word] >> (r % 64)) & 1 != 0
  }

  fn set(&mut self, r: u8, v: bool) {
    let r = r as usize;
    let word = r / 64;
    if word >= self.words.len() {
      return;
    }
    if v {
      self.words[word] |= 1 << (r % 64);
    } else {
      self.words[word] &= !(1 << (r % 64));
    }
  }

  /// Intersect merge; a "must" analysis's fixed point only ever
  /// narrows at a merge point (a fact holds after the merge only if
  /// every predecessor path already proved it).
  fn and_assign(&mut self, other: &MustSet) {
    for (a, b) in self.words.iter_mut().zip(other.words.iter()) {
      *a &= b;
    }
  }
}

/// For every bytecode position in `proto`, which registers definitely
/// (on every path reaching that position) still hold an unmodified
/// self-reference; the result of a `GetGlobal` whose name matches
/// `proto`'s own name, never redefined since. This is what lets
/// `Call`/`Invoke` sites be recognized as provably self-recursive
/// (calling this exact function, not some other value that merely
/// happens to occupy the same register) without needing live access to
/// the actual runtime global table: see the module-level "Phase 2"
/// docs for why self-recursion specifically is the one call-target
/// case this analysis can resolve without that.
///
/// A "must" analysis, the same shape as `typeflow::analyze` (optimistic
/// `full()` seed at every non-entry block, narrowed by intersection at
/// merges): a register only counts as a proven self-reference if every
/// path agrees, and anything not proven here is conservatively treated
/// as "might not be self"; the safe direction to be wrong in, since
/// a false "is definitely self" would misapply this function's own
/// (possibly still-escaping) parameter summary to what's actually a
/// call to something else entirely.
pub fn self_reference_facts(proto: &ObjFunction) -> Vec<MustSet> {
  let preds = typeflow::build_predecessors(proto);
  self_reference_facts_with_preds(proto, &preds)
}

pub fn self_reference_facts_with_preds(proto: &ObjFunction, preds: &[Vec<usize>]) -> Vec<MustSet> {
  // Matching the name only identifies the function itself when the
  // function is what that name is bound to. A method is not: it lives
  // in its class's method table, and calls itself through `self.name()`,
  // an `Invoke`. A bare `name(...)` inside a method is therefore always
  // something else, and a method that shares its name with a global
  // (`file`, `print`, anything in the builtins) would otherwise have
  // that global's calls compiled as calls to the method; the receiver
  // the method's calling convention expects in register 0 is not there,
  // so every argument lands one parameter late.
  if proto.is_method {
    let code_len = proto.chunk.code.len();
    let num_registers = proto.num_registers as usize;
    return (0..code_len)
      .map(|_| MustSet::empty(num_registers))
      .collect();
  }
  let is_self_name = |name_const: u16| -> bool {
    match proto.chunk.constants.get(name_const as usize) {
      Some(v) if v.is_string() => v.as_str() == proto.name,
      _ => false,
    }
  };
  global_ref_facts_for(proto, preds, is_self_name)
}

/// Generalization of `self_reference_facts`: same "must" analysis,
/// same soundness argument, but proving a register definitely holds an
/// unmodified read of `target_name` specifically, rather than always
/// `proto`'s own name. `self_reference_facts` is the `target_name ==
/// proto.name` special case (kept separate since self-recursion needs
/// no runtime guard at all once proven, whereas a call to some other
/// named global: see `global_ref_facts`; still needs a value-
/// identity guard, since unlike a function's own name, an arbitrary
/// global binding can be reassigned).
fn global_ref_facts_for(
  proto: &ObjFunction,
  preds: &[Vec<usize>],
  is_target_name: impl Fn(u16) -> bool,
) -> Vec<MustSet> {
  let code = &proto.chunk.code;
  let code_len = code.len();
  let num_registers = proto.num_registers as usize;

  let mut entry: Vec<MustSet> = (0..code_len)
    .map(|ip| {
      if ip == 0 {
        MustSet::empty(num_registers)
      } else {
        MustSet::full(num_registers)
      }
    })
    .collect();

  let transfer = |in_set: &MustSet, instr: &Instr| -> MustSet {
    let mut out = in_set.clone();
    match *instr {
      Instr::GetGlobal { dst, name_const } => {
        out.set(dst, is_target_name(name_const));
      },
      Instr::Move { dst, src } => {
        out.set(dst, in_set.get(src));
      },
      _ => {
        if let Some(dst) = typeflow::any_dst(instr) {
          out.set(dst, false);
        }
      },
    }
    out
  };

  let mut worklist: Vec<usize> = (0..code_len).collect();
  let mut in_worklist = vec![true; code_len];
  let mut out: Vec<MustSet> = (0..code_len)
    .map(|ip| transfer(&entry[ip], &code[ip]))
    .collect();

  while let Some(ip) = worklist.pop() {
    in_worklist[ip] = false;

    let mut new_in = MustSet::full(num_registers);
    let mut any_pred = false;
    for &p in &preds[ip] {
      new_in.and_assign(&out[p]);
      any_pred = true;
    }
    if ip == 0 {
      new_in = MustSet::empty(num_registers);
    } else if !any_pred {
      // Unreachable code; vacuously "everything proven" is safe,
      // same reasoning as `typeflow::analyze`'s own identical case.
      new_in = MustSet::full(num_registers);
    }

    if new_in != entry[ip] {
      entry[ip] = new_in;
      out[ip] = transfer(&entry[ip], &code[ip]);
      for &s in &typeflow::successors(ip, &code[ip], proto) {
        if s < code_len && !in_worklist[s] {
          in_worklist[s] = true;
          worklist.push(s);
        }
      }
    }
  }

  entry
}

/// Public entry point for `global_ref_facts_for`: for every bytecode
/// position, which registers definitely still hold an unmodified read
/// of the global named `target_name`. Used by
/// `VM::resolve_call_targets` to prove a `Call` site's callee register
/// is a specific, statically-known top-level function; unlike
/// `self_reference_facts`, the proven identity here still needs a
/// runtime value-identity guard before a direct call is safe (an
/// arbitrary global binding, unlike a function's own name, can be
/// reassigned), so the caller pairs this with the resolved `Value`'s
/// bits for that guard.
#[allow(dead_code)]
pub(crate) fn global_ref_facts(proto: &ObjFunction, target_name: &str) -> Vec<MustSet> {
  let preds = typeflow::build_predecessors(proto);
  global_ref_facts_with_preds(proto, &preds, target_name)
}

pub(crate) fn global_ref_facts_with_preds(
  proto: &ObjFunction,
  preds: &[Vec<usize>],
  target_name: &str,
) -> Vec<MustSet> {
  global_ref_facts_for(proto, preds, |name_const| {
    match proto.chunk.constants.get(name_const as usize) {
      Some(v) if v.is_string() => v.as_str() == target_name,
      _ => false,
    }
  })
}

/// Does this instruction's normal (non-tracked-register) behavior
/// write some other, unrelated value into a register; i.e. should
/// that register be killed from the alias set (it no longer holds
/// whatever it held before this instruction ran)? `Move` is handled
/// separately (it's the one case that propagates tracking instead of
/// killing it), so this only needs to cover every other register-
/// defining instruction; exactly `typeflow::any_dst`, reused as-is
/// rather than re-deriving the same exhaustive match a second time.
fn kill_target(instr: &Instr) -> Option<u8> {
  if matches!(instr, Instr::Move { .. }) {
    return None;
  }
  typeflow::any_dst(instr)
}

/// Is `GetField{obj: 0, name_const, ..}` (i.e. `self.NAME` inside a
/// method) proven safe; name resolves to a field, never a method, on
/// `proto`'s own class, so no `BoundMethod` wrapping can happen? Needs
/// both: `proto` actually knows which class it belongs to
/// (`owning_class_name`, set for every method: see that field's own
/// docs), and the caller supplied that class's already-resolved
/// `ClassFieldSafety` (this module never touches the VM itself to
/// resolve the name -> live `ObjClass` step: see `ClassFieldSafety`'s
/// own docs). Absent either one, conservatively unsafe.
fn self_getfield_is_safe(
  proto: &ObjFunction,
  name_const: u16,
  self_class_safety: Option<&ClassFieldSafety>,
) -> bool {
  let (Some(safety), Some(_)) = (self_class_safety, &proto.owning_class_name) else {
    return false;
  };
  let Some(name_val) = proto.chunk.constants.get(name_const as usize) else {
    return false;
  };
  if !name_val.is_string() {
    return false;
  }
  safety.is_field_safe(name_val.as_str())
}

/// `self_getfield_is_safe`'s counterpart for the tracked allocation
/// itself rather than for `self`.
///
/// Same `BoundMethod`-shadowing question, asked about a different
/// object: reading `d.x` off a freshly built `Vec3` is only a plain
/// field read if `Vec3` has no method also called `x`; otherwise it
/// materializes a `BoundMethod` capturing `d`, which genuinely leaks
/// it. The difference is only where the class comes from: `self`'s is
/// `proto.owning_class_name`, while an allocation's is known to
/// whoever proved the construction site's target class (see
/// `vm::vm::VM::resolve_construct_target`), so there's no
/// `owning_class_name` precondition here.
///
/// Conservatively unsafe with no safety information supplied.
fn tracked_getfield_is_safe(
  proto: &ObjFunction,
  name_const: u16,
  alloc_class_safety: Option<&ClassFieldSafety>,
) -> bool {
  let Some(safety) = alloc_class_safety else {
    return false;
  };
  let Some(name_val) = proto.chunk.constants.get(name_const as usize) else {
    return false;
  };
  if !name_val.is_string() {
    return false;
  }
  safety.is_field_safe(name_val.as_str())
}

/// Every register whose use by `instr`, if it currently aliases the
/// tracked allocation, proves the allocation escapes: see this
/// module's own docs for the verified-safe allowlist this is the
/// complement of. Deliberately structured as an exhaustive match over
/// every `Instr` variant (mirroring `typeflow::mark_uses`'s own
/// exhaustive coverage) rather than a catch-all default, so adding a
/// new instruction variant to the language is a compile error here
/// (forces an explicit, deliberate decision about its escape
/// implications) instead of silently falling into either an
/// over-conservative or; far worse; an unsound default.
pub(crate) fn escaping_reads(instr: &Instr) -> Vec<u8> {
  match *instr {
    Instr::LoadConst { .. } | Instr::LoadNil { .. } | Instr::LoadBool { .. } => vec![],

    // Verified safe: see module docs.
    Instr::Move { .. } => vec![],
    Instr::JmpIfFalse { .. } | Instr::JmpIfTrue { .. } => vec![],
    Instr::Eq { .. } | Instr::Neq { .. } => vec![],
    Instr::EqImm { .. } | Instr::NeqImm { .. } => vec![],
    Instr::Print { .. } => vec![],

    // Arithmetic/bitwise/concat/unary; all dispatch to user-defined
    // operator overloads for non-numeric operands (see
    // `jit::runtime`'s `*_slow` helpers), which could do anything with
    // the operand, including storing it somewhere long-lived.
    Instr::Add { a, b, .. }
    | Instr::Sub { a, b, .. }
    | Instr::Mul { a, b, .. }
    | Instr::Div { a, b, .. }
    | Instr::Pow { a, b, .. }
    | Instr::Floor { a, b, .. }
    | Instr::Mod { a, b, .. }
    | Instr::Concat { a, b, .. }
    | Instr::BitAnd { a, b, .. }
    | Instr::BitOr { a, b, .. }
    | Instr::BitXor { a, b, .. }
    | Instr::BitShl { a, b, .. }
    | Instr::BitShr { a, b, .. }
    | Instr::BitUshr { a, b, .. }
    | Instr::Lt { a, b, .. }
    | Instr::Le { a, b, .. }
    | Instr::Gt { a, b, .. }
    | Instr::Ge { a, b, .. } => vec![a, b],
    Instr::AddImm { a, .. }
    | Instr::SubImm { a, .. }
    | Instr::MulImm { a, .. }
    | Instr::LtImm { a, .. }
    | Instr::LeImm { a, .. }
    | Instr::GtImm { a, .. }
    | Instr::GeImm { a, .. } => vec![a],
    Instr::Neg { src, .. } | Instr::Not { src, .. } | Instr::BitNot { src, .. } => vec![src],

    Instr::Jmp { .. } => vec![],

    // Every argument and the callee/receiver register itself; no
    // interprocedural summaries yet (see module docs), so any of these
    // escapes unconditionally.
    Instr::Call { func, num_args, .. } => (func..=func.saturating_add(num_args)).collect(),
    Instr::Invoke { obj, num_args, .. } => {
      (obj..=obj.saturating_add(num_args.saturating_add(1))).collect()
    },
    Instr::InvokeSuper {
      superclass,
      num_args,
      ..
    }
    | Instr::CallSuperCtor {
      superclass,
      num_args,
      ..
    } => (superclass..=superclass.saturating_add(num_args.saturating_add(1))).collect(),

    Instr::Return { src } | Instr::Raise { src } => vec![src],

    Instr::GetGlobal { .. } => vec![],
    Instr::SetGlobal { src, .. } | Instr::AssignGlobal { src, .. } => vec![src],

    // Capturing a local register as an upvalue lets a closure that may
    // outlive this frame reach it; an escape. Only `Local(n)`
    // descriptors read a current register at all; `Upvalue(n)` passes
    // an already-captured value through, not a fresh register read.
    Instr::Closure { .. } => {
      // Resolving the capture list needs `proto.chunk.constants`, not
      // available from `instr` alone; handled by the caller
      // (`analyze_one`), which has `proto` in scope. This arm exists
      // only so the match stays exhaustive: see `analyze_one`'s own
      // Closure handling for the real logic.
      vec![]
    },
    Instr::GetUpval { .. } => vec![],
    Instr::SetUpval { src, .. } => vec![src],
    Instr::CloseUpvalues { .. } => vec![],

    // Folded into a new List/Dict this analysis doesn't track; no
    // transitive containment yet (see module docs), so unconditionally
    // escaping.
    Instr::MakeList { start, count, .. } => (start..start.saturating_add(count)).collect(),
    Instr::MakeDict { start, count, .. } => {
      (start..start.saturating_add(count.saturating_mul(2))).collect()
    },

    Instr::MakeClass { superclass, .. } => superclass.into_iter().collect(),
    Instr::DeclareField { .. } | Instr::FinalizeClass { .. } => vec![],
    // The container (`class`) is safe; writing into your own
    // slot/method table doesn't hand your identity to anything new.
    // The stored value (`src`) escapes: see module docs on why this
    // phase doesn't yet try to prove transitive containment.
    Instr::SetFieldInit { src, .. }
    | Instr::SetMethod { src, .. }
    | Instr::DeclareStatic { src, .. } => vec![src],

    // `obj`'s use here is not safe, despite reading like a plain field
    // access: see module docs on `GetField`'s implicit `BoundMethod`
    // wrapping when the name resolves to a method instead of a field.
    Instr::GetField { obj, .. } => vec![obj],
    Instr::SetField { obj: _, src, .. } => vec![src],

    Instr::Import { .. } => vec![],
    Instr::ImportAll { module, .. } => vec![module],
    Instr::MakePromoted { module, .. } => vec![module],

    // `obj` (container) and `idx` (compared via `Value::equals`, same
    // `ptr::eq` guarantee as `Eq`/`Neq` above) are safe; `src` escapes.
    Instr::GetIndex { .. } => vec![],
    Instr::SetIndex {
      obj: _,
      idx: _,
      src,
    } => vec![src],
    Instr::GetSlice { obj, lo, hi, .. } => vec![obj, lo, hi],
    Instr::MakeRange { lower, upper, .. } => vec![lower, upper],

    Instr::UsingJump { .. } => vec![],

    // Reads only the tagged `Value`'s own bits/class pointer to compare
    // against a type descriptor, and either falls through or raises --
    // never stores the value anywhere, never hands it to user code.
    Instr::CheckParamType { .. } => vec![],

    Instr::PushCatch { .. } | Instr::PopCatch => {
      unreachable!("excluded from compilation before this analysis ever runs")
    },
  }
}

/// One function's worth of escape facts, computed on demand per
/// allocation site rather than eagerly for every `Call` in the
/// function; `analyze_one` is cheap enough (one small fixed point
/// over the function's own instruction count) that callers needing
/// only a handful of sites checked don't pay for the rest.
pub struct EscapeResult {
  pub escapes: bool,
}

/// Proves (or fails to prove) that the value defined at `alloc_ip`
/// never escapes `proto`'s own execution: see this module's own docs
/// for exactly what "escapes" covers and doesn't yet cover.
///
/// `alloc_ip` must name an instruction with a real destination
/// register (checked via `typeflow::any_dst`); the analysis tracks
/// that register (and whatever it's copied into via `Move`) forward
/// from `alloc_ip`'s own successor(s) to the end of the function.
///
/// Consults `proto`'s own self-recursive parameter summary (Phase 2:
/// see that section's own docs) for any `Call` provably targeting
/// `proto` itself: an allocation passed as an argument to such a call,
/// in a position mapping onto a parameter Phase 2 already proved
/// doesn't escape, is no longer conservatively flagged just because
/// some call touched it; e.g. `TreeNode(...)` built once and then
/// threaded unchanged through further recursive calls of the same
/// function that never store it anywhere. Every other call target
/// (anything not provably self) is still fully conservative, exactly
/// as Phase 1 alone treats it.
///
/// `self_class_safety`, if supplied, also resolves `GetField` on
/// `self` for a name proven collision-free on `proto`'s own class
/// (see `ClassFieldSafety`'s own docs and `self_getfield_is_safe`);
/// `None` reproduces Phase 1's original, fully conservative GetField
/// treatment exactly.
pub fn analyze_one(
  proto: &ObjFunction,
  alloc_ip: usize,
  self_class_safety: Option<&ClassFieldSafety>,
  alloc_class_safety: Option<&ClassFieldSafety>,
) -> EscapeResult {
  let preds = typeflow::build_predecessors(proto);
  let self_ref = self_reference_facts_with_preds(proto, &preds);
  let self_summary = compute_param_summary_with_facts(proto, self_class_safety, &preds, &self_ref);
  analyze_one_with_facts(
    proto,
    alloc_ip,
    self_class_safety,
    alloc_class_safety,
    &preds,
    &self_ref,
    &self_summary.param_escapes,
  )
}

pub fn analyze_one_with_facts(
  proto: &ObjFunction,
  alloc_ip: usize,
  self_class_safety: Option<&ClassFieldSafety>,
  alloc_class_safety: Option<&ClassFieldSafety>,
  preds: &[Vec<usize>],
  self_ref: &[MustSet],
  self_summary: &[bool],
) -> EscapeResult {
  let code = &proto.chunk.code;
  let code_len = code.len();
  let num_registers = proto.num_registers as usize;

  let Some(alloc_reg) = typeflow::any_dst(&code[alloc_ip]) else {
    // Not a register-defining instruction at all; nothing to track,
    // vacuously "escapes" (there's no allocation here to prove
    // anything about, so callers must not act on this as a green
    // light).
    return EscapeResult { escapes: true };
  };

  // `entry[ip]`/`out[ip]` are meaningless (left empty, never read) for
  // `alloc_ip` itself; its role in this analysis is exactly one
  // fixed fact ("right after this instruction, `alloc_reg` holds the
  // tracked allocation"), asserted once below via `seeded_alloc_out`,
  // never recomputed by the generic per-instruction transfer this loop
  // otherwise applies to every other instruction. Folding `alloc_ip`
  // into the same generic path is wrong: this instruction's own
  // operands (e.g. a constructor call's arguments) aren't reads of the
  // value this analysis run is tracking (which doesn't exist until
  // this instruction finishes), and recomputing its `out` generically
  // from `entry[alloc_ip]` (always empty; nothing reaches the
  // allocation site already aliasing itself) would silently overwrite
  // the seed with an empty set.
  let mut entry: Vec<AliasSet> = vec![AliasSet::empty(num_registers); code_len];
  let mut out: Vec<AliasSet> = vec![AliasSet::empty(num_registers); code_len];

  let mut escaped = false;

  let mut seeded_alloc_out = AliasSet::empty(num_registers);
  seeded_alloc_out.set(alloc_reg, true);

  let mut worklist: Vec<usize> = Vec::new();
  let mut in_worklist = vec![false; code_len];
  for &s in &typeflow::successors(alloc_ip, &code[alloc_ip], proto) {
    if s != alloc_ip && s < code_len && !in_worklist[s] {
      in_worklist[s] = true;
      worklist.push(s);
    }
  }

  while let Some(ip) = worklist.pop() {
    in_worklist[ip] = false;
    debug_assert!(ip != alloc_ip, "alloc_ip is never pushed onto the worklist");

    let mut new_in = AliasSet::empty(num_registers);
    for &p in &preds[ip] {
      if p == alloc_ip {
        new_in.or_assign(&seeded_alloc_out);
      } else {
        new_in.or_assign(&out[p]);
      }
    }

    if new_in == entry[ip] {
      continue;
    }
    entry[ip] = new_in;

    let instr = &code[ip];
    let mut new_out = entry[ip].clone();

    // `Closure` needs `proto` to resolve its capture list; handled
    // here rather than in `escaping_reads`, which only sees `instr`.
    if let Instr::Closure { proto_const, .. } = *instr {
      let nested = proto.chunk.constants[proto_const as usize].as_func();
      for desc in &nested.upvalues {
        if let UpvalueDescriptor::Local(n) = *desc
          && entry[ip].get(n)
        {
          escaped = true;
        }
      }
    }

    // See `analyze_one`'s own docs on the Phase 2 refinement: a
    // provably self-recursive `Call`'s arguments are checked against
    // `proto`'s own parameter summary instead of unconditionally
    // escaping; a `GetField` reading `self` (register 0, only in a
    // method) for a name proven collision-free on `proto`'s own
    // class isn't escaping at all, replacing (not supplementing)
    // `escaping_reads`' normal `vec![obj]` for this one instruction
    // shape. Everything else falls through to Phase 1's plain
    // `escaping_reads`.
    if let Instr::Call { func, num_args, .. } = *instr
      && self_ref[ip].get(func)
    {
      for k in 1..=num_args {
        let arg_reg = func.saturating_add(k);
        if !entry[ip].get(arg_reg) {
          continue;
        }
        let param_idx = (k - 1) as usize;
        let escapes_here = self_summary.get(param_idx).copied().unwrap_or(true);
        if escapes_here {
          escaped = true;
        }
      }
    } else if let Instr::GetField {
      obj, name_const, ..
    } = *instr
    {
      // `GetField` reads exactly one register (`obj`), so this arm
      // fully replaces `escaping_reads`' `vec![obj]` for it. The
      // receiver only matters when it currently aliases the tracked
      // allocation; when it does, the read is harmless precisely when
      // the name cannot resolve to a method on the receiver's class.
      // `self` and a tracked allocation differ only in where that
      // class is known from: see the two `*_getfield_is_safe`
      // helpers.
      if entry[ip].get(obj) {
        let safe = if obj == 0 && proto.is_method {
          self_getfield_is_safe(proto, name_const, self_class_safety)
        } else {
          tracked_getfield_is_safe(proto, name_const, alloc_class_safety)
        };
        if !safe {
          escaped = true;
        }
      }
    } else {
      for reg in escaping_reads(instr) {
        if entry[ip].get(reg) {
          escaped = true;
        }
      }
    }

    if let Instr::Move { dst, src } = *instr {
      new_out.set(dst, entry[ip].get(src));
    } else if let Some(dst) = kill_target(instr) {
      new_out.set(dst, false);
    }

    if new_out != out[ip] {
      out[ip] = new_out;
      // `alloc_ip` is never re-queued here, no matter how it's
      // reached: it's a real, ordinary successor of anything whose
      // control flow loops back around to it (an allocation site
      // inside a loop being reached again via the loop's own back
      // edge is the everyday case, not an exotic one), but its
      // `entry`/`out` are the fixed seed (`seeded_alloc_out`), never
      // recomputed generically: see this function's own docs on why
      // processing it through the ordinary transfer function would
      // silently overwrite that seed. Any other ip whose predecessor
      // is `alloc_ip` already gets the seed correctly via the `p ==
      // alloc_ip` special case above; `alloc_ip` itself simply never
      // needs a turn as `ip` in this loop.
      for &s in &typeflow::successors(ip, instr, proto) {
        if s != alloc_ip && s < code_len && !in_worklist[s] {
          in_worklist[s] = true;
          worklist.push(s);
        }
      }
    }
  }

  EscapeResult { escapes: escaped }
}

// ---------------------------------------------------------------------
// PHASE 2: self-recursive parameter-escape summaries.
// ---------------------------------------------------------------------
//
// Extends Phase 1 with exactly one interprocedural case: a function
// calling itself (detected via `self_reference_facts`, above; the
// one call-target case resolvable without live access to the runtime
// global table: see this module's top-level docs). For a self-
// recursive call, an argument that maps onto one of this function's
// own parameters no longer escapes unconditionally; it escapes only
// if that same parameter is (from the rest of this computation)
// already known to escape, which is exactly the parameter-escape
// summary this section computes, via a small Kleene/Tarski fixed-point
// iteration: seed every parameter optimistically as `Local`, recompute
// the summary using that guess for self-recursive call sites, and
// repeat until it stops changing.
//
// This is monotonic (a parameter can only ever flip from `Local` to
// `Escapes`, never back), so the iteration is bounded by `arity` steps
// and converges to the least fixed point; the most precise summary
// that's still fully sound, the same Kleene-iteration shape any
// recursive dataflow summary computation uses.
//
// Still not enough, on its own, to prove real recursive-return patterns
// like `tree_with(depth).count()` non-escaping; two gaps remain,
// deliberately left for a later phase rather than rushed here:
// - `Return` is still treated as an unconditional escape (inherited
//   from `escaping_reads`), not "escapes only if the caller's use of
//   the return value escapes". Modeling that needs a three-state
//   lattice (`Local` / `EscapesViaReturn` / `Escapes`), not attempted
//   here.
// - `GetField`'s receiver is still conservatively escaping (the
//   `BoundMethod`-wrapping risk: see module docs), which is exactly
//   what `count()`'s `self.left`/`self.right` reads hit. Resolving
//   that needs proving a specific `GetField` site resolves to a field,
//   never a method, which needs class-shape knowledge this analysis
//   doesn't have.
// - Method calls (`Invoke`) are not resolved for self-recursion at
//   all; `count()` recurses via `self.left.count()`, an `Invoke`,
//   not a `Call`; only direct `GetGlobal`-based self-calls (like
//   `tree_with`'s own recursion) are handled here.

/// One function's parameter-escape summary: `param_escapes[i]` is
/// whether register `i` (parameter `i`, for `i < proto.arity`)
/// escapes this function's own body: see this section's own docs on
/// exactly what's (and isn't) accounted for.
pub struct FuncEscapeSummary {
  pub param_escapes: Vec<bool>,
}

/// Same core walk as `analyze_one`, but seeded at a parameter register
/// from the function's entry (`ip = 0`) instead of an allocation
/// site's own destination, and; the one real difference; consults
/// `guess` (the in-progress summary from the current fixed-point
/// iteration) instead of unconditionally flagging a self-recursive
/// call's argument as escaping.
///
/// Deliberately a near-duplicate of `analyze_one`'s loop rather than a
/// shared refactor: sharing the loop would mean threading the self-
/// recursion-aware Call/Invoke classification through Phase 1's
/// already-tested path too, which is a real risk to something that
/// works today for a code-sharing win that isn't worth that risk here.
fn analyze_param_escape(
  proto: &ObjFunction,
  param_reg: u8,
  self_ref: &[MustSet],
  guess: &[bool],
  self_class_safety: Option<&ClassFieldSafety>,
  preds: &[Vec<usize>],
) -> bool {
  let code = &proto.chunk.code;
  let code_len = code.len();
  let num_registers = proto.num_registers as usize;

  if code_len == 0 {
    return false;
  }

  let mut entry: Vec<AliasSet> = vec![AliasSet::empty(num_registers); code_len];
  let mut out: Vec<AliasSet> = vec![AliasSet::empty(num_registers); code_len];
  let mut escaped = false;

  // The seed: `param_reg` holds the tracked parameter from the very
  // first instruction onward (unlike `analyze_one`'s `alloc_ip`, whose
  // own operands aren't reads of the not-yet-existing tracked
  // allocation, a parameter is live from instruction 0 itself; `ip =
  // 0` is an ordinary instruction here, not a seed-only site, so it
  // goes through the exact same loop body as everything else below).
  entry[0].set(param_reg, true);

  let mut worklist: Vec<usize> = vec![0];
  let mut in_worklist = vec![false; code_len];
  in_worklist[0] = true;

  while let Some(ip) = worklist.pop() {
    in_worklist[ip] = false;

    let new_in = if ip == 0 {
      entry[0].clone()
    } else {
      let mut merged = AliasSet::empty(num_registers);
      for &p in &preds[ip] {
        merged.or_assign(&out[p]);
      }
      merged
    };

    if ip != 0 && new_in == entry[ip] {
      continue;
    }
    entry[ip] = new_in;

    let instr = &code[ip];
    let mut new_out = entry[ip].clone();

    if let Instr::Closure { proto_const, .. } = *instr {
      let nested = proto.chunk.constants[proto_const as usize].as_func();
      for desc in &nested.upvalues {
        if let UpvalueDescriptor::Local(n) = *desc
          && entry[ip].get(n)
        {
          escaped = true;
        }
      }
    }

    // The one real difference from `analyze_one`'s own Call handling:
    // a provably self-recursive `Call` (see `self_reference_facts`)
    // maps each argument register onto the callee's (= this same
    // function's) parameter at the matching position, and consults
    // `guess` for that parameter instead of unconditionally escaping.
    // Any argument register beyond `proto.arity` (an arity mismatch,
    // or a variadic tail) has no corresponding parameter to consult
    //; conservatively escapes, same as an ordinary unresolved call.
    // `GetField` on `self` is handled exactly like `analyze_one`'s own
    //: see `self_getfield_is_safe`'s own docs.
    let is_self_get_field =
      matches!(instr, Instr::GetField { obj, .. } if *obj == 0) && proto.is_method;
    if let Instr::Call { func, num_args, .. } = *instr
      && self_ref[ip].get(func)
    {
      for k in 1..=num_args {
        let arg_reg = func.saturating_add(k);
        if !entry[ip].get(arg_reg) {
          continue;
        }
        let param_idx = (k - 1) as usize;
        let escapes_here = guess.get(param_idx).copied().unwrap_or(true);
        if escapes_here {
          escaped = true;
        }
      }
    } else if is_self_get_field {
      let Instr::GetField { name_const, .. } = *instr else {
        unreachable!("is_self_get_field only true for GetField");
      };
      if entry[ip].get(0) && !self_getfield_is_safe(proto, name_const, self_class_safety) {
        escaped = true;
      }
    } else {
      for reg in escaping_reads(instr) {
        if entry[ip].get(reg) {
          escaped = true;
        }
      }
    }

    if let Instr::Move { dst, src } = *instr {
      new_out.set(dst, entry[ip].get(src));
    } else if let Some(dst) = kill_target(instr) {
      new_out.set(dst, false);
    }

    if new_out != out[ip] || ip == 0 {
      out[ip] = new_out;
      for &s in &typeflow::successors(ip, instr, proto) {
        if s < code_len && !in_worklist[s] {
          in_worklist[s] = true;
          worklist.push(s);
        }
      }
    }
  }

  escaped
}

/// Computes `proto`'s own parameter-escape summary: see this
/// section's own docs for the fixed-point shape and its known limits.
pub fn compute_param_summary(
  proto: &ObjFunction,
  self_class_safety: Option<&ClassFieldSafety>,
) -> FuncEscapeSummary {
  let preds = typeflow::build_predecessors(proto);
  let self_ref = self_reference_facts_with_preds(proto, &preds);
  compute_param_summary_with_facts(proto, self_class_safety, &preds, &self_ref)
}

pub fn compute_param_summary_with_facts(
  proto: &ObjFunction,
  self_class_safety: Option<&ClassFieldSafety>,
  preds: &[Vec<usize>],
  self_ref: &[MustSet],
) -> FuncEscapeSummary {
  let arity = proto.arity as usize;
  if arity == 0 {
    return FuncEscapeSummary {
      param_escapes: Vec::new(),
    };
  }

  let mut guess = vec![false; arity];

  loop {
    let mut next = guess.clone();
    let mut changed = false;
    for i in 0..arity {
      if guess[i] {
        continue; // already escaping; monotonic, can't un-escape
      }
      if analyze_param_escape(proto, i as u8, self_ref, &guess, self_class_safety, preds) {
        next[i] = true;
        changed = true;
      }
    }
    guess = next;
    if !changed {
      break;
    }
  }

  FuncEscapeSummary {
    param_escapes: guess,
  }
}
