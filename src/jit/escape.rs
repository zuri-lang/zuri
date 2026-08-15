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

use crate::jit::typeflow;
use crate::vm::chunk::Instr;
use crate::vm::object::{ObjFunction, UpvalueDescriptor};

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
pub fn analyze_one(proto: &ObjFunction, alloc_ip: usize) -> EscapeResult {
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

    for reg in escaping_reads(instr) {
      if entry[ip].get(reg) {
        escaped = true;
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
      source_path: Rc::from("test"),
      globals_module: None,
      jit: JitInfo::new(code_len),
    }
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
    assert!(!analyze_one(&f, 0).escapes);
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
    assert!(analyze_one(&f, 0).escapes);
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
    assert!(analyze_one(&f, 0).escapes);
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
    assert!(analyze_one(&f, 0).escapes);
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
    assert!(analyze_one(&f, 0).escapes);
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
    assert!(!analyze_one(&f, 0).escapes);
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
    assert!(!analyze_one(&f, 0).escapes);
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
    assert!(analyze_one(&f, 0).escapes);
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
    assert!(!analyze_one(&f, 0).escapes, "r1 is never read by the second Call above -- sanity check");

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
    assert!(analyze_one(&f2, 0).escapes);
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
    assert!(!analyze_one(&f, 0).escapes);
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
    assert!(!analyze_one(&f, 0).escapes);
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
    assert!(analyze_one(&f, 0).escapes);
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
    assert!(analyze_one(&f, 0).escapes);
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
    assert!(!analyze_one(&f, 0).escapes);
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
    assert!(analyze_one(&f, 0).escapes);
  }
}
