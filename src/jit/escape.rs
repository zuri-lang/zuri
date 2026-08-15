//! Escape analysis -- PHASE 1 of a multi-phase effort (see this
//! project's own perf-investigation notes for the full roadmap).
//!
//! This phase answers ONE narrow, purely INTRAPROCEDURAL question,
//! soundly: for a value defined at bytecode position `alloc_ip` within
//! a single function, can every use of it (and everything it might be
//! copied into, within THIS SAME function) be proven to never let it
//! outlive this function's own execution? If yes, the value is safe to
//! allocate somewhere cheaper than the GC heap (a stack slot, or
//! eventually scalar-replaced registers entirely) -- see `jit::codegen`
//! for how that's eventually used; THIS module only ever proves facts,
//! never changes codegen by itself.
//!
//! # What this phase deliberately does NOT do yet
//!
//! - No interprocedural reasoning at all: a tracked value passed as an
//!   argument to ANY `Call`/`Invoke`/`InvokeSuper`/`CallSuperCtor`, or
//!   used as the receiver of one, is unconditionally treated as
//!   escaping -- there is no call-graph, no per-function summary, no
//!   attempt to prove a CALLEE doesn't retain what it's handed. Real
//!   patterns like `tree_with(depth).count()` (binary-tree's own hot
//!   path) will NOT be proven non-escaping by this phase alone, since
//!   `tree_with`'s return crosses a call boundary into its caller, and
//!   `count()` is itself a call. That needs Phase 2 (interprocedural
//!   summaries over a call graph) at minimum.
//! - No transitive containment: storing a tracked value into ANOTHER
//!   object's field (`SetField`/`SetFieldInit`/`SetMethod`/
//!   `DeclareStatic`/`SetIndex`'s `src` operand, or folding it into a
//!   `MakeList`/`MakeDict`) is unconditionally treated as escaping,
//!   even if that container is ITSELF later proven non-escaping. A
//!   future phase could propagate "container doesn't escape" down to
//!   its contents; this one doesn't attempt it.
//! - No scalar replacement -- proving non-escape doesn't yet DO
//!   anything; wiring a proof into actual stack allocation is a
//!   separate, later, codegen-side change.
//!
//! # Why the safe-use allowlist is so narrow
//!
//! Zuri lets user code hook into what look like "plain" operations:
//! `GetField` on a name that resolves to a METHOD (not a field)
//! implicitly wraps the receiver in a freshly-allocated `BoundMethod`
//! -- which STORES the receiver into a new heap object, a real escape
//! path a naive reading of "GetField just reads a field" would miss
//! entirely. Arithmetic/bitwise/concat ops dispatch to user-defined
//! operator overloads (`@add`, `@sub`, ...) for non-numeric operands.
//! Given how easy it is to miss one of these, this module is built
//! default-DENY: an instruction's use of a possibly-tracked register is
//! escaping UNLESS that exact instruction is explicitly verified safe
//! below, by reading its real runtime handler, not assumed from its
//! name. A false "doesn't escape" here is a genuine memory-safety bug
//! once Phase 2/3 acts on it (a dangling pointer once the stack frame
//! that "owned" a value the GC never knew about returns); a false
//! "escapes" only ever costs a missed optimization. This trade is
//! deliberate and non-negotiable for this module.
//!
//! (`Print` was considered for the escaping side on the theory that it
//! might dispatch to a user-overridden `to_string()` -- checked against
//! the actual handler, `jit::runtime::zuri_jit_print`, which just calls
//! `println!("{}", v)`, i.e. Rust's own `Display for Value`. For
//! `Obj::Instance` that's a hardcoded `write!(f, "<instance of {}>",
//! i.class.as_class().name)` (see `value.rs`) -- reads the class's OWN
//! name, never retains or dispatches on the instance itself. No
//! user-overridable `to_string()` exists in this codebase as of this
//! writing, so `Print` is verified safe below; if/when that spec
//! feature is implemented, THIS is the exact justification that stops
//! holding and `Print` needs to move to the escaping side.)
//!
//! Verified-safe operations (see each match arm below for the specific
//! code path that justifies it):
//! - `Move` (a plain register copy -- propagates tracking, never
//!   escapes).
//! - `Eq`/`Neq`/`EqImm`/`NeqImm` (`Value::equals` on two `Obj::
//!   Instance`s is `std::ptr::eq`, unconditionally -- see
//!   `value.rs`'s own `equals` -- never a user-code dispatch).
//! - `JmpIfFalse`/`JmpIfTrue` (`Value::is_falsey` treats every heap
//!   kind except `Str`/`Bytes`/`BigInt` as unconditionally not falsey,
//!   decided from the tag byte alone -- see `codegen::emit_is_falsey`'s
//!   own docs -- never a payload read or user-code dispatch for an
//!   `Instance`).
//! - `Print` (see the paragraph above).
//! - Being the CONTAINER (never the stored value) of `SetField`/
//!   `SetFieldInit`/`SetMethod`/`DeclareStatic`/`SetIndex` -- writing
//!   into your OWN field/slot doesn't hand your identity to anything
//!   new.
//! - Being the container/index-key operand (never the stored value) of
//!   `GetIndex`/`SetIndex` -- Dict key lookup is also `Value::equals`,
//!   same `ptr::eq` guarantee as above.
//!
//! Everything else that reads a tracked register -- INCLUDING
//! `GetField`'s `obj` (the `BoundMethod`-wrapping risk above),
//! `Return`, `SetGlobal`/`AssignGlobal`, `SetUpval`, `Closure`
//! capturing it, any `Call`/`Invoke`/`InvokeSuper`/`CallSuperCtor`
//! operand, `Raise`, arithmetic/bitwise/concat/unary ops -- is
//! escaping.

use rustc_hash::FxHashSet;

use crate::jit::typeflow;
use crate::vm::chunk::Instr;
use crate::vm::object::{ObjClass, ObjFunction, UpvalueDescriptor};

/// Resolves `GetField`'s `BoundMethod`-wrapping risk (see this
/// module's top-level docs) for ONE specific, already-known class:
/// which of ITS OWN field names are safe to read via `GetField`
/// because NO method of that same name would ever shadow them.
/// Computed once, from a live `&ObjClass`, by whatever caller has VM
/// access to resolve one (this module itself deliberately never
/// touches the VM -- see its own docs) -- sound as a PERMANENT fact,
/// not a one-shot snapshot, because Zuri classes are immutable after
/// construction (NOTES.md: "new fields and methods cannot be added at
/// runtime"; see `ObjFunction::owning_class_name`'s own docs for the
/// same reasoning). Currently only consulted for a method's own
/// `self` (register 0) -- see `analyze_one`/`compute_param_summary`'s
/// own docs on why that's the one case resolvable without further
/// receiver-type inference.
pub struct ClassFieldSafety {
  safe_field_names: FxHashSet<String>,
}

impl ClassFieldSafety {
  /// `field_slots`/`methods` are pre-merged with every ancestor class
  /// already (see `ObjClass`'s own field docs), so this automatically
  /// accounts for inherited collisions too -- a subclass overriding
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
/// MAY currently hold a reference to the ONE allocation this analysis
/// run is tracking -- the "may" (union-at-merge) counterpart to
/// `typeflow::RegSet`'s "must" (intersect-at-merge) semantics. Built as
/// its own small type, deliberately not sharing `RegSet` itself,
/// specifically so this newer, less-proven analysis can never
/// accidentally perturb `typeflow`'s own already-trusted "must"
/// fixed-points -- the two need different merge operators (union here,
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

  /// Nothing aliases yet -- the correct seed for every block EXCEPT the
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

  /// Union merge -- a "may" analysis's fixed point grows monotonically
  /// via union at every merge point (if EITHER predecessor path could
  /// have left the allocation in this register, the merged state must
  /// say so too), the exact mirror of `RegSet::and_assign`'s
  /// intersection for a "must" analysis.
  fn or_assign(&mut self, other: &AliasSet) {
    for (a, b) in self.words.iter_mut().zip(other.words.iter()) {
      *a |= b;
    }
  }
}

/// A bitset over registers for a "must" (intersect-at-merge) fact --
/// used below by `self_reference_facts` to prove a register DEFINITELY
/// (on every path, not just possibly) still holds an unmodified
/// self-reference. A separate type from `AliasSet`, on purpose -- see
/// that type's own docs on why mixing "may" and "must" merge operators
/// under one type invites a silent logic bug instead of a compile
/// error.
#[derive(Clone, PartialEq, Eq)]
struct MustSet {
  words: Vec<u64>,
}

impl MustSet {
  fn word_count(num_registers: usize) -> usize {
    num_registers.div_ceil(64).max(1)
  }

  /// Nothing proven -- the correct seed for the entry block (register
  /// 0's caller-supplied argument is never statically a self-
  /// reference, nor is anything else, before any code has run).
  fn empty(num_registers: usize) -> Self {
    MustSet {
      words: vec![0u64; Self::word_count(num_registers)],
    }
  }

  /// Everything (optimistically) proven -- the correct seed for every
  /// OTHER block, so a real predecessor's facts only ever narrow it
  /// down via `and_assign`, never widen it (the same reasoning
  /// `typeflow::RegSet::full` documents for its own analogous role).
  fn full(num_registers: usize) -> Self {
    let words = Self::word_count(num_registers);
    let mut v = vec![u64::MAX; words];
    let extra_bits = words * 64 - num_registers;
    if extra_bits > 0
      && let Some(last) = v.last_mut()
    {
      *last >>= extra_bits;
    }
    MustSet { words: v }
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

  /// Intersect merge -- a "must" analysis's fixed point only ever
  /// narrows at a merge point (a fact holds after the merge only if
  /// EVERY predecessor path already proved it).
  fn and_assign(&mut self, other: &MustSet) {
    for (a, b) in self.words.iter_mut().zip(other.words.iter()) {
      *a &= b;
    }
  }
}

/// For every bytecode position in `proto`, which registers are
/// DEFINITELY (on every path reaching that position) still holding an
/// unmodified self-reference -- the result of a `GetGlobal` whose name
/// matches `proto`'s own name, never redefined since. This is what
/// lets `Call`/`Invoke` sites be recognized as PROVABLY self-recursive
/// (calling this exact function, not some other value that merely
/// happens to occupy the same register) without needing live access to
/// the actual runtime global table -- see the module-level "Phase 2"
/// docs for why self-recursion specifically is the one call-target
/// case this analysis can resolve without that.
///
/// A "must" analysis, the same shape as `typeflow::analyze` (optimistic
/// `full()` seed at every non-entry block, narrowed by intersection at
/// merges): a register only counts as a proven self-reference if EVERY
/// path agrees, and anything not proven here is conservatively treated
/// as "might not be self" -- the safe direction to be wrong in, since
/// a false "is definitely self" would misapply this function's OWN
/// (possibly still-escaping) parameter summary to what's actually a
/// call to something else entirely.
fn self_reference_facts(proto: &ObjFunction) -> Vec<MustSet> {
  let code = &proto.chunk.code;
  let code_len = code.len();
  let num_registers = proto.num_registers as usize;
  let preds = typeflow::build_predecessors(proto);

  let is_self_name = |name_const: u16| -> bool {
    match proto.chunk.constants.get(name_const as usize) {
      Some(v) if v.is_string() => v.as_str() == proto.name,
      _ => false,
    }
  };

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
        out.set(dst, is_self_name(name_const));
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
      // Unreachable code -- vacuously "everything proven" is safe,
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

/// Does this instruction's normal (non-tracked-register) behavior
/// write some OTHER, unrelated value into a register -- i.e. should
/// that register be KILLED from the alias set (it no longer holds
/// whatever it held before THIS instruction ran)? `Move` is handled
/// separately (it's the one case that PROPAGATES tracking instead of
/// killing it), so this only needs to cover every OTHER register-
/// defining instruction -- exactly `typeflow::any_dst`, reused as-is
/// rather than re-deriving the same exhaustive match a second time.
fn kill_target(instr: &Instr) -> Option<u8> {
  if matches!(instr, Instr::Move { .. }) {
    return None;
  }
  typeflow::any_dst(instr)
}

/// Is `GetField{obj: 0, name_const, ..}` (i.e. `self.NAME` inside a
/// method) proven safe -- name resolves to a field, never a method, on
/// `proto`'s OWN class, so no `BoundMethod` wrapping can happen? Needs
/// BOTH: `proto` actually knows which class it belongs to
/// (`owning_class_name`, set for every method -- see that field's own
/// docs), AND the caller supplied that class's ALREADY-RESOLVED
/// `ClassFieldSafety` (this module never touches the VM itself to
/// resolve the name -> live `ObjClass` step -- see `ClassFieldSafety`'s
/// own docs). Absent either one, conservatively unsafe -- exactly
/// Phase 1's original behavior, never worse.
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

/// Every register whose use by `instr`, IF it currently aliases the
/// tracked allocation, proves the allocation escapes -- see this
/// module's own docs for the verified-safe allowlist this is the
/// complement of. Deliberately structured as an exhaustive match over
/// every `Instr` variant (mirroring `typeflow::mark_uses`'s own
/// exhaustive coverage) rather than a catch-all default, so adding a
/// new instruction variant to the language is a compile error here
/// (forces an explicit, deliberate decision about its escape
/// implications) instead of silently falling into either an
/// over-conservative or -- far worse -- an UNSOUND default.
fn escaping_reads(instr: &Instr) -> Vec<u8> {
  match *instr {
    Instr::LoadConst { .. } | Instr::LoadNil { .. } | Instr::LoadBool { .. } => vec![],

    // Verified safe -- see module docs.
    Instr::Move { .. } => vec![],
    Instr::JmpIfFalse { .. } | Instr::JmpIfTrue { .. } => vec![],
    Instr::Eq { .. } | Instr::Neq { .. } => vec![],
    Instr::EqImm { .. } | Instr::NeqImm { .. } => vec![],
    Instr::Print { .. } => vec![],

    // Arithmetic/bitwise/concat/unary -- all dispatch to user-defined
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

    // Every argument AND the callee/receiver register itself -- no
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

    // Capturing a LOCAL register as an upvalue lets a closure that may
    // outlive this frame reach it -- an escape. Only `Local(n)`
    // descriptors read a CURRENT register at all; `Upvalue(n)` passes
    // an ALREADY-captured value through, not a fresh register read.
    Instr::Closure { .. } => {
      // NOTE: resolving the capture list needs `proto.chunk.constants`,
      // not available from `instr` alone -- handled by the caller
      // (`analyze_one`), which has `proto` in scope. This arm exists
      // only so the match stays exhaustive; see `analyze_one`'s own
      // Closure handling for the real logic.
      vec![]
    },
    Instr::GetUpval { .. } => vec![],
    Instr::SetUpval { src, .. } => vec![src],
    Instr::CloseUpvalues { .. } => vec![],

    // Folded into a NEW List/Dict this analysis doesn't track -- no
    // transitive containment yet (see module docs), so unconditionally
    // escaping.
    Instr::MakeList { start, count, .. } => (start..start.saturating_add(count)).collect(),
    Instr::MakeDict { start, count, .. } => {
      (start..start.saturating_add(count.saturating_mul(2))).collect()
    },

    Instr::MakeClass { superclass, .. } => superclass.into_iter().collect(),
    Instr::DeclareField { .. } | Instr::FinalizeClass { .. } => vec![],
    // The container (`class`) is safe -- writing into your own
    // slot/method table doesn't hand your identity to anything new.
    // The stored VALUE (`src`) escapes -- see module docs on why this
    // phase doesn't yet try to prove transitive containment.
    Instr::SetFieldInit { src, .. }
    | Instr::SetMethod { src, .. }
    | Instr::DeclareStatic { src, .. } => vec![src],

    // `obj`'s use here is NOT safe, despite reading like a plain field
    // access -- see module docs on `GetField`'s implicit `BoundMethod`
    // wrapping when the name resolves to a method instead of a field.
    Instr::GetField { obj, .. } => vec![obj],
    Instr::SetField { obj: _, src, .. } => vec![src],

    Instr::Import { .. } => vec![],
    Instr::ImportAll { module } => vec![module],
    Instr::MakePromoted { module, .. } => vec![module],

    // `obj` (container) and `idx` (compared via `Value::equals`, same
    // `ptr::eq` guarantee as `Eq`/`Neq` above) are safe; `src` escapes.
    Instr::GetIndex { .. } => vec![],
    Instr::SetIndex { obj: _, idx: _, src } => vec![src],
    Instr::GetSlice { obj, lo, hi, .. } => vec![obj, lo, hi],
    Instr::MakeRange { lower, upper, .. } => vec![lower, upper],

    Instr::UsingJump { .. } => vec![],

    Instr::PushCatch { .. } | Instr::PopCatch => {
      unreachable!("excluded from compilation before this analysis ever runs")
    },
  }
}

/// One function's worth of escape facts, computed on demand per
/// allocation site rather than eagerly for every `Call` in the
/// function -- `analyze_one` is cheap enough (one small fixed point
/// over the function's own instruction count) that callers needing
/// only a handful of sites checked don't pay for the rest.
pub struct EscapeResult {
  pub escapes: bool,
}

/// Proves (or fails to prove) that the value defined at `alloc_ip`
/// never escapes `proto`'s own execution -- see this module's own docs
/// for exactly what "escapes" covers and doesn't yet cover.
///
/// `alloc_ip` must name an instruction with a real destination
/// register (checked via `typeflow::any_dst`); the analysis tracks
/// THAT register (and whatever it's copied into via `Move`) forward
/// from `alloc_ip`'s own successor(s) to the end of the function.
///
/// Consults `proto`'s OWN self-recursive parameter summary (Phase 2 --
/// see that section's own docs) for any `Call` PROVABLY targeting
/// `proto` itself: an allocation passed as an argument to such a call,
/// in a position mapping onto a parameter Phase 2 already proved
/// doesn't escape, is no longer conservatively flagged just because
/// SOME call touched it -- e.g. `TreeNode(...)` built once and then
/// threaded unchanged through further recursive calls of the SAME
/// function that never store it anywhere. Every OTHER call target
/// (anything not provably self) is still fully conservative, exactly
/// as Phase 1 alone treats it.
///
/// `self_class_safety`, if supplied, ALSO resolves `GetField` on
/// `self` for a name proven collision-free on `proto`'s own class
/// (see `ClassFieldSafety`'s own docs and `self_getfield_is_safe`) --
/// `None` reproduces Phase 1's original, fully conservative GetField
/// treatment exactly.
pub fn analyze_one(
  proto: &ObjFunction,
  alloc_ip: usize,
  self_class_safety: Option<&ClassFieldSafety>,
) -> EscapeResult {
  let code = &proto.chunk.code;
  let code_len = code.len();
  let num_registers = proto.num_registers as usize;

  let Some(alloc_reg) = typeflow::any_dst(&code[alloc_ip]) else {
    // Not a register-defining instruction at all -- nothing to track,
    // vacuously "escapes" (there's no allocation here to prove
    // anything about, so callers must not act on this as a green
    // light).
    return EscapeResult { escapes: true };
  };

  let self_ref = self_reference_facts(proto);
  let self_summary = compute_param_summary(proto, self_class_safety);

  let preds = typeflow::build_predecessors(proto);

  // `entry[ip]`/`out[ip]` are meaningless (left empty, never read) for
  // `alloc_ip` itself -- its role in this analysis is exactly ONE
  // fixed fact ("right after this instruction, `alloc_reg` holds the
  // tracked allocation"), asserted once below via `seeded_alloc_out`,
  // never recomputed by the generic per-instruction transfer this loop
  // otherwise applies to every OTHER instruction. Folding `alloc_ip`
  // into the same generic path was tried and is exactly wrong: this
  // instruction's OWN operands (e.g. a constructor call's arguments)
  // aren't reads of the value THIS analysis run is tracking (which
  // doesn't exist until this instruction finishes), and recomputing
  // its `out` generically from `entry[alloc_ip]` (always empty --
  // nothing reaches the allocation site already aliasing itself)
  // would silently overwrite the seed with an empty set.
  let mut entry: Vec<AliasSet> = vec![AliasSet::empty(num_registers); code_len];
  let mut out: Vec<AliasSet> = vec![AliasSet::empty(num_registers); code_len];

  let mut escaped = false;

  let mut seeded_alloc_out = AliasSet::empty(num_registers);
  seeded_alloc_out.set(alloc_reg, true);

  let mut worklist: Vec<usize> = Vec::new();
  let mut in_worklist = vec![false; code_len];
  for &s in &typeflow::successors(alloc_ip, &code[alloc_ip], proto) {
    if s < code_len && !in_worklist[s] {
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

    // `Closure` needs `proto` to resolve its capture list -- handled
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
    // method) for a name proven collision-free on `proto`'s OWN
    // class is not escaping at all, REPLACING (not supplementing)
    // `escaping_reads`' normal `vec![obj]` for this one instruction
    // shape. Everything else falls through to Phase 1's plain
    // `escaping_reads`.
    let is_self_get_field = matches!(instr, Instr::GetField { obj, .. } if *obj == 0)
      && proto.is_method;
    if let Instr::Call { func, num_args, .. } = *instr
      && self_ref[ip].get(func)
    {
      for k in 1..=num_args {
        let arg_reg = func.saturating_add(k);
        if !entry[ip].get(arg_reg) {
          continue;
        }
        let param_idx = (k - 1) as usize;
        let escapes_here = self_summary
          .param_escapes
          .get(param_idx)
          .copied()
          .unwrap_or(true);
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

    if new_out != out[ip] {
      out[ip] = new_out;
      for &s in &typeflow::successors(ip, instr, proto) {
        if s < code_len && !in_worklist[s] {
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
// Extends Phase 1 with exactly ONE interprocedural case: a function
// calling ITSELF (detected via `self_reference_facts`, above -- the
// one call-target case resolvable without live access to the runtime
// global table; see this module's top-level docs). For a self-
// recursive call, an argument that maps onto one of THIS function's
// own parameters no longer escapes unconditionally -- it escapes only
// if that SAME parameter is (from the rest of this computation)
// already known to escape, which is exactly the parameter-escape
// summary this section computes, via a small Kleene/Tarski fixed-point
// iteration: seed every parameter optimistically as `Local`, recompute
// the summary using that guess for self-recursive call sites, and
// repeat until it stops changing.
//
// This is monotonic (a parameter can only ever flip from `Local` to
// `Escapes`, never back), so the iteration is bounded by `arity` steps
// and converges to the LEAST fixed point -- the most PRECISE summary
// that is still fully sound, the same Kleene-iteration shape any
// recursive dataflow summary computation uses.
//
// STILL NOT ENOUGH, on its own, to prove real recursive-return patterns
// like `tree_with(depth).count()` non-escaping -- two gaps remain,
// deliberately left for a later phase rather than rushed here:
// - `Return` is still treated as an unconditional escape (inherited
//   from `escaping_reads`), not "escapes only if the CALLER'S use of
//   the return value escapes". Modeling that needs a three-state
//   lattice (`Local` / `EscapesViaReturn` / `Escapes`), not attempted
//   here.
// - `GetField`'s receiver is still conservatively escaping (the
//   `BoundMethod`-wrapping risk -- see module docs), which is exactly
//   what `count()`'s `self.left`/`self.right` reads hit. Resolving
//   that needs proving a specific `GetField` site resolves to a FIELD,
//   never a method, which needs class-shape knowledge this analysis
//   doesn't have.
// - Method calls (`Invoke`) are not resolved for self-recursion at
//   all -- `count()` recurses via `self.left.count()`, an `Invoke`,
//   not a `Call`; only direct `GetGlobal`-based self-calls (like
//   `tree_with`'s own recursion) are handled here.

/// One function's parameter-escape summary: `param_escapes[i]` is
/// whether register `i` (parameter `i`, for `i < proto.arity`)
/// escapes this function's own body -- see this section's own docs on
/// exactly what's (and isn't) accounted for.
pub struct FuncEscapeSummary {
  pub param_escapes: Vec<bool>,
}

/// Same core walk as `analyze_one`, but seeded at a PARAMETER register
/// from the function's entry (`ip = 0`) instead of an allocation
/// site's own destination, and -- the one real difference -- consults
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
) -> bool {
  let code = &proto.chunk.code;
  let code_len = code.len();
  let num_registers = proto.num_registers as usize;
  let preds = typeflow::build_predecessors(proto);

  if code_len == 0 {
    return false;
  }

  let mut entry: Vec<AliasSet> = vec![AliasSet::empty(num_registers); code_len];
  let mut out: Vec<AliasSet> = vec![AliasSet::empty(num_registers); code_len];
  let mut escaped = false;

  // The seed: `param_reg` holds the tracked parameter from the very
  // first instruction onward (unlike `analyze_one`'s `alloc_ip`, whose
  // OWN operands aren't reads of the not-yet-existing tracked
  // allocation, a parameter is live from instruction 0 itself -- `ip =
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

    // The one real difference from `analyze_one`'s OWN Call handling:
    // a PROVABLY self-recursive `Call` (see `self_reference_facts`)
    // maps each argument register onto the callee's (= this same
    // function's) parameter at the matching position, and consults
    // `guess` for THAT parameter instead of unconditionally escaping.
    // Any argument register beyond `proto.arity` (an arity mismatch,
    // or a variadic tail) has no corresponding parameter to consult
    // -- conservatively escapes, same as an ordinary unresolved call.
    // `GetField` on `self` is handled exactly like `analyze_one`'s own
    // -- see `self_getfield_is_safe`'s own docs.
    let is_self_get_field = matches!(instr, Instr::GetField { obj, .. } if *obj == 0)
      && proto.is_method;
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

/// Computes `proto`'s own parameter-escape summary -- see this
/// section's own docs for the fixed-point shape and its known limits.
pub fn compute_param_summary(
  proto: &ObjFunction,
  self_class_safety: Option<&ClassFieldSafety>,
) -> FuncEscapeSummary {
  let arity = proto.arity as usize;
  if arity == 0 {
    return FuncEscapeSummary {
      param_escapes: Vec::new(),
    };
  }

  let self_ref = self_reference_facts(proto);
  let mut guess = vec![false; arity];

  loop {
    let mut next = guess.clone();
    let mut changed = false;
    for i in 0..arity {
      if guess[i] {
        continue; // already escaping -- monotonic, can't un-escape
      }
      if analyze_param_escape(proto, i as u8, &self_ref, &guess, self_class_safety) {
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

#[cfg(test)]
mod tests {
  use std::rc::Rc;

  use super::*;
  use crate::vm::chunk::Chunk;
  use crate::vm::object::JitInfo;
  use crate::vm::value::Value;

  fn make_func(code: Vec<Instr>, constants: Vec<Value>, num_registers: u8) -> ObjFunction {
    let mut chunk = Chunk::new();
    chunk.code = code;
    chunk.constants = constants;
    let code_len = chunk.code.len();
    ObjFunction {
      name: "test".to_string(),
      variadic: false,
      chunk,
      arity: 0,
      num_registers,
      upvalues: Vec::new(),
      is_method: false,
      owning_class_name: None,
      source_path: Rc::from("test"),
      globals_module: None,
      jit: JitInfo::new(code_len),
    }
  }

  /// A heap-allocated (deliberately leaked -- this is test-only code)
  /// `Obj::Str`, wrapped as a `Value` -- constants that name a global
  /// (`GetGlobal`/`SetGlobal`'s `name_const`) need a real string Value,
  /// not just a placeholder, since `self_reference_facts` compares
  /// their actual text against `proto.name`.
  fn test_str(s: &str) -> Value {
    let boxed = Box::new(crate::vm::object::Obj::Str(s.to_string()));
    Value::obj(Box::leak(boxed))
  }

  /// Never read, never stored anywhere -- the simplest possible
  /// non-escaping case.
  #[test]
  fn never_used_does_not_escape() {
    let code = vec![
      Instr::Call {
        dst: 1,
        func: 0,
        num_args: 0,
      },
      Instr::LoadNil { dst: 2 },
      Instr::Return { src: 2 },
    ];
    let f = make_func(code, vec![], 3);
    assert!(!analyze_one(&f, 0, None).escapes);
  }

  #[test]
  fn escapes_via_return() {
    let code = vec![
      Instr::Call {
        dst: 1,
        func: 0,
        num_args: 0,
      },
      Instr::Return { src: 1 },
    ];
    let f = make_func(code, vec![], 2);
    assert!(analyze_one(&f, 0, None).escapes);
  }

  #[test]
  fn escapes_via_set_global() {
    let code = vec![
      Instr::Call {
        dst: 1,
        func: 0,
        num_args: 0,
      },
      Instr::SetGlobal {
        name_const: 0,
        src: 1,
      },
      Instr::Return { src: 0 },
    ];
    let f = make_func(code, vec![Value::nil()], 2);
    assert!(analyze_one(&f, 0, None).escapes);
  }

  #[test]
  fn escapes_via_set_upval() {
    let code = vec![
      Instr::Call {
        dst: 1,
        func: 0,
        num_args: 0,
      },
      Instr::SetUpval { idx: 0, src: 1 },
      Instr::Return { src: 0 },
    ];
    let f = make_func(code, vec![], 2);
    assert!(analyze_one(&f, 0, None).escapes);
  }

  /// A `Move` chain must still propagate tracking -- an allocation
  /// copied into a different register, then returned FROM there,
  /// still escapes.
  #[test]
  fn move_chain_propagates_escape() {
    let code = vec![
      Instr::Call {
        dst: 1,
        func: 0,
        num_args: 0,
      },
      Instr::Move { dst: 2, src: 1 },
      Instr::Return { src: 2 },
    ];
    let f = make_func(code, vec![], 3);
    assert!(analyze_one(&f, 0, None).escapes);
  }

  /// The register that held the allocation gets overwritten with
  /// something unrelated before the escaping use -- the allocation
  /// itself was already discarded (never stored anywhere else), so
  /// this must NOT be flagged as escaping just because the SAME
  /// register number later holds something that does.
  #[test]
  fn overwritten_register_does_not_falsely_escape() {
    let code = vec![
      Instr::Call {
        dst: 1,
        func: 0,
        num_args: 0,
      },
      Instr::LoadNil { dst: 1 }, // r1 now holds nil, not the allocation
      Instr::Return { src: 1 },  // returns nil, not the tracked allocation
    ];
    let f = make_func(code, vec![], 2);
    assert!(!analyze_one(&f, 0, None).escapes);
  }

  /// Writing INTO the allocation's own field (`obj` is the container)
  /// must not count as an escape.
  #[test]
  fn writing_own_field_does_not_escape() {
    let code = vec![
      Instr::Call {
        dst: 1,
        func: 0,
        num_args: 0,
      },
      Instr::LoadNil { dst: 2 },
      Instr::SetField {
        obj: 1,
        name_const: 0,
        src: 2,
      },
      Instr::Return { src: 0 },
    ];
    let f = make_func(code, vec![Value::nil()], 3);
    assert!(!analyze_one(&f, 0, None).escapes);
  }

  /// Storing the TRACKED allocation as a field's VALUE (into some
  /// OTHER object) must escape -- Phase 1 doesn't try to prove the
  /// container is also local (see module docs).
  #[test]
  fn storing_as_field_value_escapes() {
    let code = vec![
      Instr::Call {
        dst: 1,
        func: 0,
        num_args: 0,
      }, // tracked allocation
      Instr::Call {
        dst: 2,
        func: 0,
        num_args: 0,
      }, // a different object (the container)
      Instr::SetField {
        obj: 2,
        name_const: 0,
        src: 1,
      }, // stores r1 (tracked) as a field of r2
      Instr::Return { src: 0 },
    ];
    let f = make_func(code, vec![Value::nil()], 3);
    assert!(analyze_one(&f, 0, None).escapes);
  }

  /// Passing the tracked allocation as a call argument escapes --
  /// Phase 1 has no interprocedural summaries yet (see module docs).
  #[test]
  fn passed_as_call_argument_escapes() {
    let code = vec![
      Instr::Call {
        dst: 1,
        func: 0,
        num_args: 0,
      }, // tracked allocation in r1
      Instr::Call {
        dst: 5,
        func: 3,
        num_args: 1,
      }, // reads r3 (func) and r4 (arg) -- not r1 yet
      Instr::Return { src: 0 },
    ];
    let f = make_func(code, vec![], 6);
    assert!(!analyze_one(&f, 0, None).escapes, "r1 is never read by the second Call above -- sanity check");

    let code2 = vec![
      Instr::Call {
        dst: 1,
        func: 0,
        num_args: 0,
      },
      Instr::Move { dst: 4, src: 1 }, // r4 (an arg slot) now aliases the tracked allocation
      Instr::Call {
        dst: 5,
        func: 3,
        num_args: 1,
      }, // reads r3, r4 -- r4 aliases our allocation
      Instr::Return { src: 0 },
    ];
    let f2 = make_func(code2, vec![], 6);
    assert!(analyze_one(&f2, 0, None).escapes);
  }

  /// Identity comparison (`Eq`/`Neq`) is verified safe -- see module
  /// docs (`Value::equals` on two Instances is `ptr::eq`, never a
  /// user-code dispatch).
  #[test]
  fn identity_comparison_does_not_escape() {
    let code = vec![
      Instr::Call {
        dst: 1,
        func: 0,
        num_args: 0,
      },
      Instr::LoadNil { dst: 2 },
      Instr::Eq { dst: 3, a: 1, b: 2 },
      Instr::Return { src: 3 },
    ];
    let f = make_func(code, vec![], 4);
    assert!(!analyze_one(&f, 0, None).escapes);
  }

  /// `Print` is verified safe -- see module docs (no user-overridable
  /// `to_string()` exists yet; `Display for Value` on an `Instance`
  /// only reads the class's own name).
  #[test]
  fn print_does_not_escape() {
    let code = vec![
      Instr::Call {
        dst: 1,
        func: 0,
        num_args: 0,
      },
      Instr::Print { src: 1 },
      Instr::Return { src: 0 },
    ];
    let f = make_func(code, vec![], 2);
    assert!(!analyze_one(&f, 0, None).escapes);
  }

  /// `GetField`'s `obj` operand is treated as escaping (the implicit
  /// `BoundMethod`-wrapping risk -- see module docs), even though it
  /// reads like a plain, obviously-safe field access.
  #[test]
  fn get_field_receiver_is_conservatively_escaping() {
    let code = vec![
      Instr::Call {
        dst: 1,
        func: 0,
        num_args: 0,
      },
      Instr::GetField {
        dst: 2,
        obj: 1,
        name_const: 0,
      },
      Instr::Return { src: 0 },
    ];
    let f = make_func(code, vec![Value::nil()], 3);
    assert!(analyze_one(&f, 0, None).escapes);
  }

  /// A branch merge must UNION, not intersect: an escape reachable
  /// down only ONE of two paths still counts, even though the other
  /// path never touches the tracked register at all.
  #[test]
  fn escape_on_one_branch_is_still_detected() {
    let code = vec![
      Instr::Call {
        dst: 1,
        func: 0,
        num_args: 0,
      }, // ip 0: allocation
      Instr::JmpIfFalse {
        cond: 9,
        offset: 1,
      }, // ip 1: false -> ip 3, true (fallthrough) -> ip 2
      Instr::Return { src: 1 }, // ip 2: escaping path
      Instr::Return { src: 9 }, // ip 3: non-escaping path
    ];
    let f = make_func(code, vec![], 10);
    assert!(analyze_one(&f, 0, None).escapes);
  }

  /// The mirror of the above: if NEITHER branch ever touches the
  /// tracked register, the merge must not spuriously invent an
  /// escape.
  #[test]
  fn no_escape_on_either_branch_stays_unescaped() {
    let code = vec![
      Instr::Call {
        dst: 1,
        func: 0,
        num_args: 0,
      }, // ip 0: allocation
      Instr::JmpIfFalse {
        cond: 9,
        offset: 1,
      }, // ip 1
      Instr::Return { src: 9 }, // ip 2
      Instr::Return { src: 9 }, // ip 3
    ];
    let f = make_func(code, vec![], 10);
    assert!(!analyze_one(&f, 0, None).escapes);
  }

  /// A loop back-edge (the allocation is live across a jump backward)
  /// must reach a fixed point rather than looping forever, and must
  /// still correctly detect an escape that only happens after several
  /// iterations' worth of propagation.
  #[test]
  fn loop_back_edge_reaches_fixed_point() {
    let code = vec![
      Instr::Call {
        dst: 1,
        func: 0,
        num_args: 0,
      }, // ip 0: allocation
      Instr::JmpIfFalse {
        cond: 9,
        offset: 2,
      }, // ip 1: false -> ip 4 (exit), true -> ip 2 (loop body)
      Instr::LoadNil { dst: 2 }, // ip 2: loop body, doesn't touch r1
      Instr::Jmp { offset: -3 }, // ip 3: back-edge to ip 1
      Instr::Return { src: 1 }, // ip 4: exit path -- escapes
    ];
    let f = make_func(code, vec![], 10);
    assert!(analyze_one(&f, 0, None).escapes);
  }

  // -----------------------------------------------------------------
  // PHASE 2: self-recursion detection and parameter-escape summaries.
  // -----------------------------------------------------------------

  fn make_named_func(
    name: &str,
    code: Vec<Instr>,
    constants: Vec<Value>,
    num_registers: u8,
    arity: u8,
  ) -> ObjFunction {
    let mut f = make_func(code, constants, num_registers);
    f.name = name.to_string();
    f.arity = arity;
    f
  }

  #[test]
  fn self_reference_facts_recognizes_own_name_only() {
    let code = vec![
      Instr::GetGlobal {
        dst: 0,
        name_const: 0,
      }, // "f" -- self
      Instr::GetGlobal {
        dst: 1,
        name_const: 1,
      }, // "g" -- NOT self
      Instr::Return { src: 0 },
    ];
    let f = make_named_func(
      "f",
      code,
      vec![test_str("f"), test_str("g")],
      2,
      0,
    );
    let facts = self_reference_facts(&f);
    // After ip=1 (both GetGlobals have executed), r0 is definitely
    // self, r1 is definitely not.
    assert!(facts[2].get(0));
    assert!(!facts[2].get(1));
  }

  #[test]
  fn self_reference_killed_by_redefinition() {
    let code = vec![
      Instr::GetGlobal {
        dst: 0,
        name_const: 0,
      }, // "f" -- self
      Instr::LoadNil { dst: 0 }, // overwritten -- no longer self
      Instr::Return { src: 0 },
    ];
    let f = make_named_func("f", code, vec![test_str("f")], 1, 0);
    let facts = self_reference_facts(&f);
    assert!(!facts[2].get(0));
  }

  /// A parameter passed through a self-recursive call's SAME argument
  /// position, and never otherwise touched, must be proven non-
  /// escaping -- the whole point of Phase 2 over Phase 1.
  #[test]
  fn param_passed_through_self_recursion_does_not_escape() {
    // def f(n, obj) {
    //   if n == 0 { return 0 }
    //   return f(n - 1, obj)   // `obj` passed through unchanged
    // }
    let code = vec![
      Instr::LoadConst {
        dst: 2,
        const_idx: 1,
      }, // ip0: 0
      Instr::Eq { dst: 3, a: 0, b: 2 }, // ip1: n == 0
      Instr::JmpIfFalse {
        cond: 3,
        offset: 2,
      }, // ip2: false -> ip5, true -> ip3
      Instr::LoadConst {
        dst: 4,
        const_idx: 1,
      }, // ip3: 0
      Instr::Return { src: 4 }, // ip4: base case, doesn't touch obj
      Instr::GetGlobal {
        dst: 5,
        name_const: 0,
      }, // ip5: "f" (self)
      Instr::SubImm {
        dst: 6,
        a: 0,
        imm_const: 1,
      }, // ip6: n - 1
      Instr::Move { dst: 7, src: 1 }, // ip7: obj -> arg slot (func+2)
      Instr::Call {
        dst: 8,
        func: 5,
        num_args: 2,
      }, // ip8: f(n-1, obj)
      Instr::Return { src: 8 }, // ip9: returns the recursive result, not obj
    ];
    let f = make_named_func(
      "f",
      code,
      vec![test_str("f"), Value::number(0.0)],
      9,
      2,
    );
    let summary = compute_param_summary(&f, None);
    assert_eq!(summary.param_escapes.len(), 2);
    assert!(
      !summary.param_escapes[1],
      "obj (param 1) is only ever passed through the self-recursive \
       call in its own argument position, and never read/stored/\
       returned directly -- must be proven non-escaping"
    );
  }

  /// The same shape as above, but `obj` is ALSO stored to a global
  /// inside the function -- the summary must correctly flag it as
  /// escaping, proving the analysis isn't just unconditionally
  /// optimistic about self-recursive parameters.
  #[test]
  fn param_escapes_despite_self_recursion_if_also_stored_globally() {
    let code = vec![
      Instr::LoadConst {
        dst: 2,
        const_idx: 1,
      }, // ip0: 0
      Instr::Eq { dst: 3, a: 0, b: 2 }, // ip1: n == 0
      Instr::JmpIfFalse {
        cond: 3,
        offset: 2,
      }, // ip2
      Instr::LoadConst {
        dst: 4,
        const_idx: 1,
      }, // ip3
      Instr::Return { src: 4 }, // ip4: base case
      Instr::GetGlobal {
        dst: 5,
        name_const: 0,
      }, // ip5: "f"
      Instr::SubImm {
        dst: 6,
        a: 0,
        imm_const: 1,
      }, // ip6: n - 1
      Instr::Move { dst: 7, src: 1 }, // ip7: obj -> arg slot
      Instr::SetGlobal {
        name_const: 2,
        src: 1,
      }, // ip8: ALSO stash obj in a global
      Instr::Call {
        dst: 8,
        func: 5,
        num_args: 2,
      }, // ip9: f(n-1, obj)
      Instr::Return { src: 8 }, // ip10
    ];
    let f = make_named_func(
      "f",
      code,
      vec![
        test_str("f"),
        Value::number(0.0),
        test_str("leaked"),
      ],
      9,
      2,
    );
    let summary = compute_param_summary(&f, None);
    assert!(summary.param_escapes[1]);
  }

  /// A call to a DIFFERENT (non-self) global is not resolved by Phase
  /// 2 at all -- its arguments must remain conservatively escaping,
  /// exactly like Phase 1 alone would treat them.
  #[test]
  fn non_self_call_argument_stays_conservatively_escaping() {
    let code = vec![
      Instr::GetGlobal {
        dst: 1,
        name_const: 0,
      }, // ip0: "other", NOT self
      Instr::Move { dst: 2, src: 0 }, // ip1: param -> arg slot
      Instr::Call {
        dst: 3,
        func: 1,
        num_args: 1,
      }, // ip2: other(param)
      Instr::Return { src: 3 }, // ip3
    ];
    let f = make_named_func("f", code, vec![test_str("other")], 4, 1);
    let summary = compute_param_summary(&f, None);
    assert!(summary.param_escapes[0]);
  }

  /// End-to-end: `analyze_one` (Phase 1's OWN allocation tracking),
  /// not just a bare parameter summary, benefits from Phase 2 -- an
  /// object allocated once and threaded unchanged through further
  /// self-recursive calls, never otherwise touched, is proven
  /// non-escaping.
  #[test]
  fn allocation_threaded_through_self_recursion_does_not_escape() {
    // def f(n) {                 -- n (r0) itself is never touched
    //   var obj = alloc()        -- ip0: the tracked allocation
    //   return f(obj)            -- self-recursive, obj threaded as
    //                                the argument in n's OWN position
    // }
    let code = vec![
      Instr::Call {
        dst: 1,
        func: 5,
        num_args: 0,
      }, // ip0: allocation
      Instr::GetGlobal {
        dst: 2,
        name_const: 0,
      }, // ip1: "f" -- self
      Instr::Move { dst: 3, src: 1 }, // ip2: obj -> arg slot
      Instr::Call {
        dst: 4,
        func: 2,
        num_args: 1,
      }, // ip3: f(obj) -- self-recursive
      Instr::Return { src: 4 }, // ip4: returns the recursive result, not obj
    ];
    let f = make_named_func("f", code, vec![test_str("f")], 6, 1);
    assert!(!analyze_one(&f, 0, None).escapes);
  }

  // -----------------------------------------------------------------
  // GetField-on-self resolution (ClassFieldSafety).
  // -----------------------------------------------------------------

  fn make_class(
    field_names: &[&str],
    method_names: &[&str],
  ) -> crate::vm::object::ObjClass {
    let mut field_slots = rustc_hash::FxHashMap::default();
    for (i, name) in field_names.iter().enumerate() {
      field_slots.insert(name.to_string(), i as u16);
    }
    let mut methods = rustc_hash::FxHashMap::default();
    for name in method_names {
      methods.insert(name.to_string(), Value::nil());
    }
    crate::vm::object::ObjClass {
      name: "TestClass".to_string(),
      superclass: None,
      methods,
      field_slots,
      field_count: field_names.len() as u16,
      own_field_initializer: None,
      constructor: None,
      static_slots: rustc_hash::FxHashMap::default(),
      statics: Vec::new(),
    }
  }

  #[test]
  fn class_field_safety_excludes_name_collisions() {
    // A (pathological, but the collision this whole mechanism exists
    // to catch) class with a field AND a method both named "count".
    let class = make_class(&["left", "right", "count"], &["count"]);
    let safety = ClassFieldSafety::from_class(&class);
    assert!(safety.is_field_safe("left"));
    assert!(safety.is_field_safe("right"));
    assert!(
      !safety.is_field_safe("count"),
      "a field/method name collision must never be reported safe"
    );
    assert!(!safety.is_field_safe("nonexistent"));
  }

  fn make_method_func(
    class_name: &str,
    code: Vec<Instr>,
    constants: Vec<Value>,
    num_registers: u8,
  ) -> ObjFunction {
    let mut f = make_named_func("someMethod", code, constants, num_registers, 1);
    f.is_method = true;
    f.owning_class_name = Some(class_name.to_string());
    f
  }

  /// `self.left` (a name proven collision-free on the method's own
  /// class) must NOT be treated as escaping when `self_class_safety`
  /// is supplied -- the whole point of this section.
  #[test]
  fn self_getfield_on_safe_field_does_not_escape_param() {
    let code = vec![
      Instr::GetField {
        dst: 1,
        obj: 0,
        name_const: 0,
      }, // ip0: self.left
      Instr::Return { src: 1 }, // ip1: returns the FIELD's value, not self
    ];
    let f = make_method_func("TreeNode", code, vec![test_str("left")], 2);
    let class = make_class(&["left", "right"], &["count"]);
    let safety = ClassFieldSafety::from_class(&class);

    let summary = compute_param_summary(&f, Some(&safety));
    assert!(
      !summary.param_escapes[0],
      "self.left is proven safe and never otherwise used -- self must not escape"
    );
  }

  /// The exact same code, WITHOUT `self_class_safety` supplied, must
  /// stay fully conservative -- confirms this is additive (an opt-in
  /// refinement), never a behavior change for callers that don't have
  /// a resolved class on hand.
  #[test]
  fn self_getfield_without_safety_info_stays_conservative() {
    let code = vec![
      Instr::GetField {
        dst: 1,
        obj: 0,
        name_const: 0,
      },
      Instr::Return { src: 1 },
    ];
    let f = make_method_func("TreeNode", code, vec![test_str("left")], 2);

    let summary = compute_param_summary(&f, None);
    assert!(summary.param_escapes[0]);
  }

  /// `self.count` where `count` collides with a method name on the
  /// SAME class must still escape, even with `self_class_safety`
  /// supplied -- the safety check itself must correctly say "unsafe"
  /// for a real collision, not just default to "safe whenever
  /// provided".
  #[test]
  fn self_getfield_on_colliding_field_still_escapes() {
    let code = vec![
      Instr::GetField {
        dst: 1,
        obj: 0,
        name_const: 0,
      }, // ip0: self.count -- collides with a method
      Instr::Return { src: 1 },
    ];
    let f = make_method_func("Weird", code, vec![test_str("count")], 2);
    let class = make_class(&["count"], &["count"]);
    let safety = ClassFieldSafety::from_class(&class);

    let summary = compute_param_summary(&f, Some(&safety));
    assert!(summary.param_escapes[0]);
  }

  /// End-to-end: `analyze_one` tracking a CONSTRUCTED allocation
  /// (not just a bare parameter) also benefits -- a `TreeNode` built,
  /// read via safe `self`-shaped field access is a DIFFERENT scenario
  /// than this test (the tracked value here is `self` itself, via a
  /// method's own parameter tracking through `compute_param_summary`,
  /// which `analyze_one` already delegates to for self-recursive
  /// calls -- see `analyze_one`'s own Call handling). This test
  /// instead confirms `analyze_one`'s OWN direct GetField dispatch
  /// path (not just via a nested `compute_param_summary` call) honors
  /// `self_class_safety` when the TRACKED allocation itself is read
  /// back via `self.field` inside the SAME method that constructed it.
  #[test]
  fn analyze_one_honors_self_class_safety_directly() {
    let code = vec![
      Instr::Call {
        dst: 1,
        func: 5,
        num_args: 0,
      }, // ip0: allocation, held in r1 (NOT self/r0)
      Instr::GetField {
        dst: 2,
        obj: 0,
        name_const: 0,
      }, // ip1: self.left -- safe, unrelated to r1's tracking
      Instr::Return { src: 2 }, // ip2: returns the field read, not r1
    ];
    let f = make_method_func("TreeNode", code, vec![test_str("left")], 6);
    let class = make_class(&["left", "right"], &["count"]);
    let safety = ClassFieldSafety::from_class(&class);

    // r1 (the allocation) is never touched by the GetField at all --
    // this mainly confirms `self_class_safety` threads through
    // `analyze_one` without breaking its unrelated-allocation tracking.
    assert!(!analyze_one(&f, 0, Some(&safety)).escapes);
  }
}
