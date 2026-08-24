//! A purely static, compile-time-only dataflow pass proving which
//! bytecode registers are definitely numeric at each instruction --
//! not a guess, a proof: wherever it says a register is numeric, every
//! possible runtime execution path reaching that point put a number
//! there. `jit::codegen` uses this to skip the `is_number()` guard (and
//! the slow-path helper fallback entirely) for arithmetic whose
//! operands are already proven, turning what would be a branchy
//! guard-and-fallback into unconditional straight-line float math.
//!
//! # Why this needs no runtime guard, no profiling, no deoptimization
//!
//! This is a "must" analysis (a fact holds only if it holds on every
//! incoming path, so a merge point intersects rather than unions its
//! predecessors' facts), computed once per function at JIT-compile
//! time, purely from the bytecode's own shape; constants, and chains
//! of arithmetic whose own operands are already proven. Nothing here
//! is ever "probably true" or "true so far": if the proof holds, the
//! guard is redundant by construction, so eliding it can't ever be
//! wrong. Anything not statically provable (function parameters, a
//! `GetField`/`Call`/`GetIndex` result, ...) is just conservatively
//! left unproven, and `codegen` falls back to exactly today's guarded
//! fast/slow codegen for it; this pass only ever removes already-
//! redundant checks, never adds risk.
//!
//! # Why a `Call`/`Invoke` doesn't invalidate other registers' proofs
//!
//! This VM's calling convention gives a callee a fresh register window
//! starting at `func + 1` (or `obj + 1` for `Invoke`) in the caller's
//! own register file; the callee can only ever write within its own
//! window, never back into the caller's lower-numbered registers
//! (`setup_closure_call`'s whole contract). So a call only changes the
//! type of its own `dst` register; every other register's value (and
//! therefore its proven-numeric status) provably survives a call
//! unchanged, the same way it survives a GC safepoint (non-moving GC,
//! so a pointer's identity and target never change under it either).
//! This is what makes chains like `for i in .. { sum = sum + f(i) }`
//! still get some benefit even with a call in the loop: `i`'s own
//! proof survives the call intact, even though `sum`'s doesn't (it's
//! reassigned from the call's own result, which isn't proven).

use rustc_hash::FxHashMap;

use crate::vm::chunk::{Instr, ParamType};
use crate::vm::object::ObjFunction;

/// A bitset over bytecode register indices (0..=255), one word per 64
/// registers. Cheap to clone/AND/compare; functions rarely use more
/// than a handful of words' worth of registers.
#[derive(Clone, PartialEq, Eq)]
pub struct RegSet {
  words: Vec<u64>,
}

impl RegSet {
  fn word_count(num_registers: usize) -> usize {
    num_registers.div_ceil(64).max(1)
  }

  /// Nothing proven; the correct starting fact for a function's own
  /// entry (register 0's caller-supplied argument is never statically
  /// known to be a number, nor is anything else, before any code has
  /// run).
  fn empty(num_registers: usize) -> Self {
    RegSet {
      words: vec![0u64; Self::word_count(num_registers)],
    }
  }

  /// Everything (optimistically) proven; the correct starting point
  /// for every other block in a forward "must" analysis with an
  /// intersecting merge: each real predecessor's facts can only ever
  /// narrow this down via `and_assign`, never widen it, so seeding
  /// with "everything" and letting real edges intersect it down is
  /// what makes the fixed-point iteration converge to the tightest
  /// correct answer rather than getting stuck too conservative.
  fn full(num_registers: usize) -> Self {
    let words = Self::word_count(num_registers);
    let mut v = vec![u64::MAX; words];
    // Clear any bits past `num_registers` in the last word so equality
    // comparisons between two `full()`-seeded sets behave sanely (not
    // load-bearing for correctness, just keeps the representation
    // canonical).
    let extra_bits = words * 64 - num_registers;
    if extra_bits > 0
      && extra_bits < 64
      && let Some(last) = v.last_mut()
    {
      *last &= u64::MAX >> extra_bits;
    }
    RegSet { words: v }
  }

  #[inline]
  pub fn get(&self, r: u8) -> bool {
    let idx = r as usize / 64;
    let bit = r as usize % 64;
    self
      .words
      .get(idx)
      .map(|w| (w >> bit) & 1 != 0)
      .unwrap_or(false)
  }

  #[inline]
  fn set(&mut self, r: u8, v: bool) {
    let idx = r as usize / 64;
    let bit = r as usize % 64;
    if idx >= self.words.len() {
      return;
    }
    if v {
      self.words[idx] |= 1u64 << bit;
    } else {
      self.words[idx] &= !(1u64 << bit);
    }
  }

  /// Intersects `self` with `other` (a predecessor's outgoing facts),
  /// returning whether anything actually changed; what drives the
  /// worklist's fixed-point termination.
  fn and_assign(&mut self, other: &RegSet) -> bool {
    let mut changed = false;
    for (a, b) in self.words.iter_mut().zip(other.words.iter()) {
      let merged = *a & *b;
      if merged != *a {
        changed = true;
      }
      *a = merged;
    }
    changed
  }

  /// Unions `self` with `other`, returning whether anything actually
  /// changed; the merge operator `liveness`'s "may" fixed point uses
  /// (a register is live if it's needed on ANY path forward, unlike
  /// `and_assign`'s "must hold on every path" used by the numeric-facts
  /// analysis above).
  fn or_assign(&mut self, other: &RegSet) -> bool {
    let mut changed = false;
    for (a, b) in self.words.iter_mut().zip(other.words.iter()) {
      let merged = *a | *b;
      if merged != *a {
        changed = true;
      }
      *a = merged;
    }
    changed
  }

  /// Sets every register in `start..start+count` (saturating at the
  /// representable range); used for instructions whose operands are a
  /// whole contiguous register window rather than a fixed handful of
  /// named fields (`Call`'s argument window, `MakeList`/`MakeDict`'s
  /// element run, `Invoke`'s receiver+self+argument window, ...).
  fn set_range(&mut self, start: u8, count: usize) {
    let mut r = start as usize;
    for _ in 0..count {
      if r > u8::MAX as usize {
        break;
      }
      self.set(r as u8, true);
      r += 1;
    }
  }

  /// Every register index currently set, low to high; what a caller
  /// that needs to actually enumerate (not just test) the live set at a
  /// program point iterates over (e.g. `jit::codegen`'s spill-site
  /// emission, driven by exactly this at every sync point).
  pub fn iter_set(&self) -> impl Iterator<Item = u8> + '_ {
    self.words.iter().enumerate().flat_map(|(word_idx, &w)| {
      (0..64u32).filter_map(move |bit| {
        if (w >> bit) & 1 != 0 {
          Some((word_idx * 64 + bit as usize) as u8)
        } else {
          None
        }
      })
    })
  }
}

/// The result of analyzing one function: `entry[ip]` is exactly the
/// set of registers proven numeric on every path reaching bytecode
/// position `ip`, i.e. before `ip`'s own instruction executes; what
/// `codegen::FuncCompiler` consults when deciding whether an
/// arithmetic op's operands need a runtime guard at all.
pub struct TypeFacts {
  entry: Vec<RegSet>,
}

impl TypeFacts {
  #[inline]
  pub fn is_numeric(&self, ip: usize, r: u8) -> bool {
    self.entry[ip].get(r)
  }

  /// Every register (among the first 64; the same bound
  /// `speculative_params` itself is already subject to) proven numeric
  /// at bytecode position `ip`, as a bitmask. At `ip == 0` this is
  /// exactly whatever seed `analyze` was given, if any. At any other
  /// `ip`, this is the sound thing to re-validate a runtime guard
  /// against when jumping directly into that bytecode position (e.g.
  /// on-stack replacement): exactly the claim the code at `ip` is
  /// about to rely on, not a proxy for it.
  ///
  /// Checking whether the original entry-time seed registers are
  /// still numeric at `ip` is not equivalent and not sound in general:
  /// a seed register can be reassigned (even to another number)
  /// between entry and `ip` in a way that invalidates an earlier
  /// proof without that reassignment being visible in a plain "is
  /// register R numeric right now" check. Querying `entry[ip]`
  /// directly sidesteps that; it's already the fixed point of the
  /// same dataflow proof used everywhere else in this file, which
  /// tracks every reassignment precisely.
  pub fn numeric_mask_at(&self, ip: usize) -> u64 {
    let mut mask = 0u64;
    for bit in 0..64u8 {
      if self.entry[ip].get(bit) {
        mask |= 1u64 << bit;
      }
    }
    mask
  }
}

/// Every register (among the first 64) that a whole-frame profile
/// sample observed holding a number at the moment compilation
/// triggered; consulted by `transfer` wherever an instruction's
/// result would otherwise be unconditionally treated as unproven
/// (`GetField`, `Call`, `Invoke`, `GetGlobal`, `GetUpval`, `GetIndex`,
/// ...: anything whose value depends on something this pass can't see
/// statically). Unlike `speculative_params` (which seeds function
/// entry, `ip == 0`, since a parameter genuinely holds its real value
/// before any code runs), this has no single seed point: it changes
/// what `transfer` computes for the out set at whatever ip the
/// matching instruction actually executes at, and the existing
/// fixed-point worklist propagates that forward like any other fact;
/// no changes needed to the merge/iteration logic itself.
///
/// Deliberately not restricted to a hardcoded instruction allowlist:
/// any instruction whose result is semantically never a number (a
/// `Closure`, a `List`, ...) simply never samples as numeric in the
/// first place, so the profiling itself keeps this self-limited to
/// instructions where speculating is actually meaningful, rather than
/// a second, separately-maintained list that could drift out of sync
/// with `transfer`'s own instruction match.
///
/// Soundness comes from the same place it always does in this JIT:
/// `codegen` never trusts this seed's claim without a real runtime
/// guard planted exactly at the instruction's own definition site
/// (checking the actual value just computed, not a proxy for it):
/// see `jit::codegen::FuncCompiler`'s own docs on the mid-function
/// guard-and-fork this drives. A wrong guess here costs a fallback
/// jump into the general body's continuation, never a wrong answer.
pub type SpeculativeRegs = u64;

/// Runs the analysis. `speculative_params`, if given, seeds
/// register-0-based parameter slots as already proven numeric at
/// function entry instead of starting from nothing; this is the hook
/// `jit::engine`'s profile-guided specialization (a separate, second
/// compiled copy of the function body, guarded by one runtime check at
/// entry: see `codegen`'s own docs) uses to get the same
/// guard-elision benefit for a speculatively-numeric parameter as a
/// statically-provable constant gets for free. `None` (or an empty
/// set) reproduces the fully conservative baseline (nothing assumed at
/// entry); always sound on its own, used for both ordinary
/// compilation and every OSR entry point.
///
/// `speculative_params` is a bitmask (bit `r` = register `r`) of
/// fixed-arity parameter registers to seed as already-proven at
/// function entry, rather than a `RegSet` directly; keeps `RegSet`
/// itself a purely internal representation, with only a plain integer
/// crossing this module's boundary. Bits at or past register 64 (or
/// past `proto.num_registers`) are simply never representable or used
///; a real function needing more than 64 speculated parameters
/// doesn't lose correctness, just the ability to speculate on the
/// overflow ones (`codegen`'s own entry guard is built from the same
/// mask, so the two always agree on which registers are being bet on).
///
/// `speculative_regs` is the same kind of bitmask, but for values
/// beyond function parameters: see `SpeculativeRegs`'s own docs.
///
/// `preds` is `build_predecessors(proto)`, computed by the CALLER and
/// passed in rather than recomputed here: it depends only on `proto`'s
/// own control flow, never on anything a particular analysis is
/// proving, so `analyze_int`/`analyze_list`/`liveness` (which take it
/// the same way) all share one computation instead of each redoing
/// the identical predecessor-graph walk for every function compiled.
pub fn analyze(
  proto: &ObjFunction,
  preds: &[Vec<usize>],
  speculative_params: Option<u64>,
  speculative_regs: Option<SpeculativeRegs>,
) -> TypeFacts {
  let code = &proto.chunk.code;
  let code_len = code.len();
  let num_registers = proto.num_registers as usize;

  let seed: Option<RegSet> = speculative_params.map(|mask| {
    let mut s = RegSet::empty(num_registers);
    for bit in 0..64u8 {
      if mask & (1u64 << bit) != 0 {
        s.set(bit, true);
      }
    }
    s
  });

  let mut entry: Vec<RegSet> = (0..code_len)
    .map(|ip| {
      if ip == 0 {
        RegSet::empty(num_registers)
      } else {
        RegSet::full(num_registers)
      }
    })
    .collect();
  if let Some(seed) = &seed
    && code_len > 0
  {
    entry[0] = seed.clone();
  }

  // `speculative_regs` is a single register-indexed bitmask, sampled
  // as a one-shot runtime snapshot of "whatever's currently in this
  // register" (see `VM::sample_all_reg_types`); it carries no memory
  // of which instruction produced that value. `transfer` applies a
  // set bit to every unprovable instruction (`Call`, `GetGlobal`,
  // `GetField`, ...) that happens to write that same register number,
  // anywhere in the function. That's unsound whenever the bytecode
  // compiler's register allocator reuses one register slot for two
  // different unprovable definitions; the single most common case
  // being a call's own callee slot getting reused, in place, for the
  // call's result (`GetGlobal dst=r` to load the callee, immediately
  // followed by `Call dst=r, func=r`): the snapshot naturally observes
  // the result (often numeric), but the same bit then also claims the
  // callee load itself is numeric; which a closure/function value
  // never is, so that guard would fail on every single invocation,
  // not occasionally. Strip any register written by more than one
  // distinct speculatable instruction before it ever reaches
  // `transfer`, so a seed only ever attaches to the one definition
  // site it was actually sampled from.
  let spec_regs = speculative_regs
    .map(|mask| mask & !ambiguous_speculative_regs(code))
    .unwrap_or(0);

  let mut worklist: Vec<usize> = (0..code_len).collect();
  let mut in_worklist = vec![true; code_len];
  // Seed every out set from its (possibly still-`full()`, not-yet-
  // converged) in set, so the worklist loop below has a real starting
  // point to compare against.
  let mut out: Vec<RegSet> = (0..code_len)
    .map(|ip| transfer(&entry[ip], &code[ip], proto, spec_regs))
    .collect();

  while let Some(ip) = worklist.pop() {
    in_worklist[ip] = false;

    let mut new_in = RegSet::full(num_registers);
    let mut any_pred = false;
    for &p in &preds[ip] {
      new_in.and_assign(&out[p]);
      any_pred = true;
    }
    if !any_pred {
      // Unreachable code (no predecessor at all, and not the entry
      // block); vacuously "everything proven" is safe: nothing ever
      // actually executes this instruction, so whatever `codegen`
      // does with an over-optimistic fact here can never run.
      new_in = RegSet::full(num_registers);
    }
    if ip == 0
      && let Some(seed) = &seed
    {
      new_in = seed.clone();
    } else if ip == 0 {
      new_in = RegSet::empty(num_registers);
    }

    if new_in != entry[ip] {
      entry[ip] = new_in;
      out[ip] = transfer(&entry[ip], &code[ip], proto, spec_regs);
      for &s in &successors(ip, &code[ip], proto) {
        if s < code_len && !in_worklist[s] {
          in_worklist[s] = true;
          worklist.push(s);
        }
      }
    }
  }

  TypeFacts { entry }
}

//-----------------------------------------------------------------------------------
// Integer-valued analysis
//-----------------------------------------------------------------------------------

/// The result of analyzing one function: `entry[ip]` is exactly the set
/// of registers proven to hold a WHOLE-NUMBER-valued `f64` (`fract() ==
/// 0.0`, not just numeric) on every path reaching bytecode position
/// `ip`. `jit::codegen` uses this to skip the float-roundtrip "is this
/// actually an integer, not e.g. `3.5`" check `emit_list_get_index`/
/// `emit_list_set_index` would otherwise redo on every single indexed
/// access, even for a plain loop counter.
///
/// Same "must" analysis shape as `TypeFacts`, and safe to build on the
/// same closure argument: IEEE-754 round-to-nearest-even addition,
/// subtraction, and multiplication of two whole-number-valued doubles
/// ALWAYS produces another whole-number-valued double, at every
/// magnitude, with no errors; not an approximation someone could
/// find a counterexample to. The reasoning: the true mathematical
/// result of integer-plus-integer (or times/minus) is itself a whole
/// number, and every double whose representable granularity is >= 1
/// (any magnitude at or past roughly 2^52) is ALREADY constrained to
/// only ever represent whole numbers at all; so rounding a whole-
/// number result to the nearest representable double, at ANY
/// magnitude, can only ever land on another whole number, never on a
/// fractional one. (What genuinely isn't preserved past that
/// magnitude is which EXACT whole number; true precision, a
/// different property this analysis was never asked to prove.) This
/// is why, unlike a naive first read of "arithmetic can lose
/// precision" might suggest, no overflow-checked arithmetic or
/// runtime fallback is needed anywhere here for soundness.
pub struct IntFacts {
  entry: Vec<RegSet>,
}

impl IntFacts {
  #[inline]
  pub fn is_int(&self, ip: usize, r: u8) -> bool {
    self.entry[ip].get(r)
  }
}

fn transfer_int(in_set: &RegSet, instr: &Instr, proto: &ObjFunction) -> RegSet {
  let mut out = in_set.clone();
  match *instr {
    Instr::LoadConst { dst, const_idx } => {
      let c = &proto.chunk.constants[const_idx as usize];
      out.set(dst, c.is_number() && c.as_number().fract() == 0.0);
    },
    Instr::Move { dst, src } => out.set(dst, in_set.get(src)),
    Instr::Neg { dst, src } => out.set(dst, in_set.get(src)),

    // See `IntFacts`'s own docs: sound at every magnitude, no overflow
    // check needed.
    Instr::Add { dst, a, b } | Instr::Sub { dst, a, b } | Instr::Mul { dst, a, b } => {
      out.set(dst, in_set.get(a) && in_set.get(b));
    },
    Instr::AddImm { dst, a, imm_const }
    | Instr::SubImm { dst, a, imm_const }
    | Instr::MulImm { dst, a, imm_const } => {
      let imm_is_int = proto.chunk.constants[imm_const as usize]
        .as_number()
        .fract()
        == 0.0;
      out.set(dst, imm_is_int && in_set.get(a));
    },

    // A bitwise op's result is a whole number BY DEFINITION of what
    // the operation means, regardless of whether its operands were
    // already proven int; exactly the same "it just raised
    // otherwise, so this holds given execution reached here at all"
    // reasoning `CheckParamType` relies on below, just for a
    // different kind of guard.
    Instr::BitAnd { dst, .. }
    | Instr::BitOr { dst, .. }
    | Instr::BitXor { dst, .. }
    | Instr::BitShl { dst, .. }
    | Instr::BitShr { dst, .. }
    | Instr::BitUshr { dst, .. }
    | Instr::BitNot { dst, .. } => out.set(dst, true),

    // A parameter checked as EXACTLY `int` (not the wider `number`,
    // which also admits fractional values) is provably whole on every
    // path past this instruction.
    Instr::CheckParamType { reg, check_idx } => {
      let check = &proto.chunk.param_checks[check_idx as usize];
      let all_int =
        !check.nullable && check.types.len() == 1 && matches!(check.types[0], ParamType::Int);
      out.set(reg, all_int);
    },

    // Everything else that writes a register either never produces a
    // number at all (comparisons, `Concat`, `LoadNil`/`LoadBool`, ...)
    // or isn't provably whole even when it IS numeric (`Div`/`Pow`/
    // `Floor`/`Mod`, a `Call`/`GetField`/`GetIndex` result, ...); same
    // conservative treatment `transfer`'s own numeric analysis gives
    // these, just narrower since "numeric" doesn't imply "whole".
    //
    // `any_dst`, not `conservative_dst`/`comparison_or_never_numeric_dst`
    //; those two are each a curated SUBSET (originally written to
    // cover only the arms this match already handles explicitly above),
    // so any instruction outside both lists AND outside this match's own
    // explicit arms (e.g. `Div`, or a plain `LoadConst` of a fractional
    // number) fell through doing nothing at all, leaving `out[dst]`
    // holding whatever it was BEFORE this instruction overwrote the
    // register; a real "must" analysis unsoundness whenever the
    // register allocator reuses a whole-number-proven register for one
    // of these. `any_dst` is a genuine superset covering every `Instr`
    // that writes a register at all, so nothing can slip through here.
    _ => {
      if let Some(dst) = any_dst(instr) {
        out.set(dst, false);
      }
    },
  }
  out
}

/// Runs the whole-number analysis: see `IntFacts`'s own docs. No
/// speculative-profiling hook, unlike `analyze`: proving "whole
/// number" needs an actual definition site to reason about (a
/// `LoadConst`, a chain of proven-int arithmetic, ...), so there's no
/// sound way to seed it from a bare runtime snapshot the way
/// `speculative_regs` seeds `TypeFacts` (that hook works there only
/// because `codegen` re-verifies the ACTUAL value at the definition
/// site regardless; a whole-number claim would need the identical
/// re-verification this whole analysis exists to avoid paying for).
/// `preds`: see `analyze`'s own docs on why this takes it as a
/// parameter instead of computing it fresh.
///
/// Short-circuits before the worklist the same way `analyze_list`
/// does, and for the identical reason: if the function has no
/// whole-number `LoadConst`, no `CheckParamType(Int)`, and no bitwise
/// op anywhere (the only instructions `transfer_int` ever seeds
/// `true` from; `Add`/`Sub`/`Mul`/`*Imm`/`Move` only ever PROPAGATE
/// an existing proof, never originate one), no register can ever be
/// proven whole on any path, so the real fixed point is trivially
/// "nothing, anywhere". Less likely to fire than `analyze_list`'s own
/// skip; a bare integer literal is common; but real for method
/// bodies that are pure dispatch/field access with no arithmetic of
/// their own at all.
pub fn analyze_int(proto: &ObjFunction, preds: &[Vec<usize>]) -> IntFacts {
  let code = &proto.chunk.code;
  let code_len = code.len();

  let has_int_source = code.iter().any(|i| match i {
    Instr::LoadConst { const_idx, .. } => {
      let c = &proto.chunk.constants[*const_idx as usize];
      c.is_number() && c.as_number().fract() == 0.0
    },
    Instr::AddImm { imm_const, .. }
    | Instr::SubImm { imm_const, .. }
    | Instr::MulImm { imm_const, .. } => {
      proto.chunk.constants[*imm_const as usize]
        .as_number()
        .fract()
        == 0.0
    },
    Instr::CheckParamType { check_idx, .. } => {
      let check = &proto.chunk.param_checks[*check_idx as usize];
      !check.nullable && check.types.len() == 1 && matches!(check.types[0], ParamType::Int)
    },
    Instr::BitAnd { .. }
    | Instr::BitOr { .. }
    | Instr::BitXor { .. }
    | Instr::BitShl { .. }
    | Instr::BitShr { .. }
    | Instr::BitUshr { .. }
    | Instr::BitNot { .. } => true,
    _ => false,
  });
  if !has_int_source {
    return IntFacts {
      entry: vec![RegSet::empty(proto.num_registers as usize); code_len],
    };
  }

  let num_registers = proto.num_registers as usize;

  let mut entry: Vec<RegSet> = (0..code_len)
    .map(|ip| {
      if ip == 0 {
        RegSet::empty(num_registers)
      } else {
        RegSet::full(num_registers)
      }
    })
    .collect();

  let mut worklist: Vec<usize> = (0..code_len).collect();
  let mut in_worklist = vec![true; code_len];
  let mut out: Vec<RegSet> = (0..code_len)
    .map(|ip| transfer_int(&entry[ip], &code[ip], proto))
    .collect();

  while let Some(ip) = worklist.pop() {
    in_worklist[ip] = false;

    let mut new_in = RegSet::full(num_registers);
    let mut any_pred = false;
    for &p in &preds[ip] {
      new_in.and_assign(&out[p]);
      any_pred = true;
    }
    if !any_pred {
      new_in = RegSet::full(num_registers);
    }
    if ip == 0 {
      new_in = RegSet::empty(num_registers);
    }

    if new_in != entry[ip] {
      entry[ip] = new_in;
      out[ip] = transfer_int(&entry[ip], &code[ip], proto);
      for &s in &successors(ip, &code[ip], proto) {
        if s < code_len && !in_worklist[s] {
          in_worklist[s] = true;
          worklist.push(s);
        }
      }
    }
  }

  IntFacts { entry }
}

//-----------------------------------------------------------------------------------
// List-shape analysis
//-----------------------------------------------------------------------------------

/// The result of analyzing one function: `entry[ip]` is exactly the
/// set of registers proven to hold an `Obj::List` on every path
/// reaching bytecode position `ip`. `jit::codegen` uses this to skip
/// the `is_obj` + tag-load + tag-compare guard `emit_list_get_index`/
/// `emit_list_set_index` would otherwise redo on every single indexed
/// access, and to skip real method-name lookup for a handful of List
/// methods simple enough to inline directly (see
/// `codegen::ListIntrinsic`).
///
/// Same "must" analysis shape as `TypeFacts`/`IntFacts`; optimistic
/// `full()` seed at every non-entry block, narrowed by intersection at
/// merges, so a register only counts as proven here when EVERY
/// incoming path agrees, including both sides of a branch (`if flag {
/// x = [1] } else { x = [2] }` proves `x` a list after the join,
/// `if flag { x = [1] } else { x = "s" }` proves neither) and every
/// loop back-edge (a loop that reassigns its own list variable to
/// itself, or to a fresh list, keeps proving it a list on the very
/// next iteration too; the fixed-point worklist below converges on
/// that the same way it already does for `TypeFacts`' numeric facts,
/// no special-casing needed).
pub struct ListFacts {
  entry: Vec<RegSet>,
}

impl ListFacts {
  #[inline]
  pub fn is_list(&self, ip: usize, r: u8) -> bool {
    self.entry[ip].get(r)
  }
}

fn transfer_list(in_set: &RegSet, instr: &Instr, proto: &ObjFunction) -> RegSet {
  let mut out = in_set.clone();
  match *instr {
    Instr::MakeList { dst, .. } => out.set(dst, true),
    Instr::Move { dst, src } => out.set(dst, in_set.get(src)),

    // `[x] * n` (list-repeat); the ONLY other instruction that can
    // produce a list, and only when its left operand already is one;
    // `binary_mult`'s own list-repeat semantics never turn a NON-list
    // `a` into a list result, so `false` is exactly right when `a`
    // isn't already proven.
    Instr::Mul { dst, a, .. } => out.set(dst, in_set.get(a)),

    // A parameter checked as EXACTLY `list` (not a union) is provably
    // a list on every path past this instruction; it just raised
    // otherwise, same "must" reasoning `IntFacts`'s own
    // `CheckParamType` arm uses.
    Instr::CheckParamType { reg, check_idx } => {
      let check = &proto.chunk.param_checks[check_idx as usize];
      let all_list =
        !check.nullable && check.types.len() == 1 && matches!(check.types[0], ParamType::List);
      out.set(reg, all_list);
    },

    // Everything else that writes a register is either never a list
    // (arithmetic, comparisons, `LoadNil`/`LoadBool`, `Concat`, ...)
    // or not PROVABLY one even when it might be at runtime (a
    // `Call`/`GetField`/`GetIndex`/`Invoke` result; e.g. `list.map`
    // returns a new list, but that's a fact about ONE specific method
    // on a receiver already proven list, not something this
    // instruction-shape-only pass can see).
    //
    // `any_dst`: see `transfer_int`'s identical catch-all for why the
    // narrower `conservative_dst`/`comparison_or_never_numeric_dst`
    // pair is NOT enough here: `x = [1]; x = 1 + 2` reuses `x`'s
    // register for an `Add`, which is in neither list, so the stale
    // "list" fact from the `MakeList` would otherwise survive the
    // reassignment; a real, reproduced memory-safety bug (`emit_list_
    // get_index`'s proven-list fast path would then mask a plain
    // number's bits as if they were a list pointer and dereference
    // whatever that lands on).
    _ => {
      if let Some(dst) = any_dst(instr) {
        out.set(dst, false);
      }
    },
  }
  out
}

/// Runs the list-shape analysis: see `ListFacts`'s own docs.
///
/// Short-circuits before touching the worklist at all when the
/// function has no `MakeList` and no list-typed parameter check
/// anywhere in it: those are the ONLY two instructions `transfer_list`
/// ever seeds `true` from, so with neither present, no register can
/// EVER be proven a list on any path, and the real fixed point is
/// trivially "nothing, anywhere"; computing that via one cheap
/// linear scan instead of the full predecessor-graph/worklist
/// machinery matters because this analysis now runs for EVERY
/// compiled function, including the overwhelming majority (most
/// polymorphic-dispatch/arithmetic-heavy code) that never touches a
/// list at all. `preds`: see `analyze`'s own docs on why this takes
/// it as a parameter instead of computing it fresh (the short-circuit
/// above means this particular analysis often doesn't even need it).
pub fn analyze_list(proto: &ObjFunction, preds: &[Vec<usize>]) -> ListFacts {
  let code = &proto.chunk.code;
  let code_len = code.len();

  let has_list_source = code.iter().any(|i| match i {
    Instr::MakeList { .. } => true,
    Instr::CheckParamType { check_idx, .. } => {
      let check = &proto.chunk.param_checks[*check_idx as usize];
      !check.nullable && check.types.len() == 1 && matches!(check.types[0], ParamType::List)
    },
    _ => false,
  });
  if !has_list_source {
    return ListFacts {
      entry: vec![RegSet::empty(proto.num_registers as usize); code_len],
    };
  }

  let num_registers = proto.num_registers as usize;

  let mut entry: Vec<RegSet> = (0..code_len)
    .map(|ip| {
      if ip == 0 {
        RegSet::empty(num_registers)
      } else {
        RegSet::full(num_registers)
      }
    })
    .collect();

  let mut worklist: Vec<usize> = (0..code_len).collect();
  let mut in_worklist = vec![true; code_len];
  let mut out: Vec<RegSet> = (0..code_len)
    .map(|ip| transfer_list(&entry[ip], &code[ip], proto))
    .collect();

  while let Some(ip) = worklist.pop() {
    in_worklist[ip] = false;

    let mut new_in = RegSet::full(num_registers);
    let mut any_pred = false;
    for &p in &preds[ip] {
      new_in.and_assign(&out[p]);
      any_pred = true;
    }
    if !any_pred {
      new_in = RegSet::full(num_registers);
    }
    if ip == 0 {
      new_in = RegSet::empty(num_registers);
    }

    if new_in != entry[ip] {
      entry[ip] = new_in;
      out[ip] = transfer_list(&entry[ip], &code[ip], proto);
      for &s in &successors(ip, &code[ip], proto) {
        if s < code_len && !in_worklist[s] {
          in_worklist[s] = true;
          worklist.push(s);
        }
      }
    }
  }

  ListFacts { entry }
}

/// `ListFacts`' counterpart for `Value::String`; same "must" shape,
/// same worklist, same short-circuit; but seeded from a different set
/// of instructions: a `LoadConst` whose constant is itself a string
/// (string LITERALS are baked straight into the constant table, no
/// `MakeList`-style builder opcode involved), `Instr::Add` when BOTH
/// operands are already proven strings (`..` is Zuri's RANGE operator,
/// not concatenation; `s1 + s2` is how source actually concatenates
/// two strings, compiling to a plain `Instr::Add`; `VM::binary_add_
/// values`' `is_string() && is_string()` arm always allocates a fresh
/// string for that case, no operator-override detour possible since
/// `try_operator_override` only ever fires for an Instance receiver,
/// which a register this analysis already proved a String can never
/// be), and `Instr::Concat`, which; though no current source syntax
/// actually emits it; `VM::run`'s own handler always resolves to a
/// string too, so it's included for the same reason `Mul`'s list-repeat
/// arm is in `transfer_list`: correct and free to keep, even if this
/// particular producer turns out unreachable from today's grammar.
/// Consulted by `codegen::FuncCompiler::emit_string_invoke` to skip
/// straight to `zuri_jit_invoke_string`, bypassing the wasted
/// `zuri_jit_invoke_prepare` attempt (that helper's very first check is
/// `receiver.is_instance()`, which a String can never be).
pub struct StringFacts {
  entry: Vec<RegSet>,
}

impl StringFacts {
  #[inline]
  pub fn is_string(&self, ip: usize, r: u8) -> bool {
    self.entry[ip].get(r)
  }
}

fn transfer_string(in_set: &RegSet, instr: &Instr, proto: &ObjFunction) -> RegSet {
  let mut out = in_set.clone();
  match *instr {
    Instr::LoadConst { dst, const_idx } => {
      let c = &proto.chunk.constants[const_idx as usize];
      out.set(dst, c.is_string());
    },
    Instr::Move { dst, src } => out.set(dst, in_set.get(src)),
    Instr::Concat { dst, .. } => out.set(dst, true),

    // `a + b` where BOTH sides are already proven strings: see this
    // struct's own docs for why that's unconditionally a fresh string,
    // no operator-override or numeric-add branch reachable. Only when
    // BOTH operands are proven, mirroring `transfer_list`'s `Mul` arm:
    // `"x" + 1` is a real `TypeError`-or-`format!`-fallback case this
    // pass has no business claiming as a string.
    Instr::Add { dst, a, b } => out.set(dst, in_set.get(a) && in_set.get(b)),

    // A parameter checked as EXACTLY `string` (not a union) is provably
    // a string on every path past this instruction; same reasoning
    // `transfer_list`'s own `CheckParamType` arm uses.
    Instr::CheckParamType { reg, check_idx } => {
      let check = &proto.chunk.param_checks[check_idx as usize];
      let all_string =
        !check.nullable && check.types.len() == 1 && matches!(check.types[0], ParamType::String);
      out.set(reg, all_string);
    },

    // Everything else that writes a register is either never a string
    // (arithmetic, comparisons, `MakeList`, ...) or not PROVABLY one
    // even when it might be at runtime (a `Call`/`GetField`/`GetIndex`/
    // `Invoke` result; e.g. `s.upper()` returns a new string, but
    // that's a fact about ONE specific method on an already-proven-
    // string receiver, not something this instruction-shape-only pass
    // can see).
    //
    // `any_dst`, not the narrower `conservative_dst`/`comparison_or_
    // never_numeric_dst` pair: see `transfer_list`'s identical
    // catch-all for why: those two lists don't cover every register-
    // writing instruction (`LoadConst` of a non-string constant,
    // arithmetic, ...), so a register reused for one of them after
    // being proven a string would otherwise keep that stale proof.
    _ => {
      if let Some(dst) = any_dst(instr) {
        out.set(dst, false);
      }
    },
  }
  out
}

/// Runs the string-shape analysis: see `StringFacts`'s own docs.
///
/// Short-circuits the same way `analyze_list` does: with no string-
/// valued `LoadConst` and no `Concat` anywhere in the function (the
/// only two instructions `transfer_string` ever seeds `true` from), no
/// register can ever be proven a string on any path, so the fixed
/// point is trivially empty and the worklist is skipped entirely.
pub fn analyze_string(proto: &ObjFunction, preds: &[Vec<usize>]) -> StringFacts {
  let code = &proto.chunk.code;
  let code_len = code.len();

  let has_string_source = code.iter().any(|i| match i {
    Instr::LoadConst { const_idx, .. } => proto.chunk.constants[*const_idx as usize].is_string(),
    Instr::Concat { .. } => true,
    _ => false,
  });
  if !has_string_source {
    return StringFacts {
      entry: vec![RegSet::empty(proto.num_registers as usize); code_len],
    };
  }

  let num_registers = proto.num_registers as usize;

  let mut entry: Vec<RegSet> = (0..code_len)
    .map(|ip| {
      if ip == 0 {
        RegSet::empty(num_registers)
      } else {
        RegSet::full(num_registers)
      }
    })
    .collect();

  let mut worklist: Vec<usize> = (0..code_len).collect();
  let mut in_worklist = vec![true; code_len];
  let mut out: Vec<RegSet> = (0..code_len)
    .map(|ip| transfer_string(&entry[ip], &code[ip], proto))
    .collect();

  while let Some(ip) = worklist.pop() {
    in_worklist[ip] = false;

    let mut new_in = RegSet::full(num_registers);
    let mut any_pred = false;
    for &p in &preds[ip] {
      new_in.and_assign(&out[p]);
      any_pred = true;
    }
    if !any_pred {
      new_in = RegSet::full(num_registers);
    }
    if ip == 0 {
      new_in = RegSet::empty(num_registers);
    }

    if new_in != entry[ip] {
      entry[ip] = new_in;
      out[ip] = transfer_string(&entry[ip], &code[ip], proto);
      for &s in &successors(ip, &code[ip], proto) {
        if s < code_len && !in_worklist[s] {
          in_worklist[s] = true;
          worklist.push(s);
        }
      }
    }
  }

  StringFacts { entry }
}

/// One register's constant-propagation state at one bytecode position
///; a genuine 3-level lattice, not a boolean like `RegSet`'s: TWO
/// DIFFERENT known constants meeting at a merge point must collapse
/// straight to `Bottom` ("not provably any single value"), there's no
/// partial answer between them the way "not proven a list" already
/// covers every non-list case.
///
/// Compared by raw bit pattern, not `f64`'s own `PartialEq` (`NaN !=
/// NaN` would make `Exact(NaN) == Exact(NaN)` false even for the
/// identical constant loaded from the identical source, which is not
/// the question this asks): see `ConstFacts`'s own docs for the one
/// place this actually matters.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum ConstFact {
  /// The optimistic seed: no real predecessor has narrowed this yet.
  /// Never observable outside this file; `analyze_const`'s worklist
  /// always resolves every reachable position to one of the other two
  /// before returning.
  Top,
  /// Every path reaching this position defines this register as
  /// EXACTLY this `f64` bit pattern.
  Exact(u64),
  /// Proven NOT any single constant; either it genuinely varies
  /// across paths, or it never was a compile-time-known value at all
  /// (a parameter, a call result, arithmetic on a non-constant, ...).
  Bottom,
}

impl ConstFact {
  /// The lattice's meet (⊓) operation, same role `RegSet::and_assign`
  /// plays for the boolean analyses: `Top` is the identity (a not-yet-
  /// visited predecessor contributes nothing), two agreeing `Exact`
  /// values stay agreed, anything else; including two DIFFERENT
  /// `Exact` values; collapses to `Bottom`.
  fn meet(self, other: ConstFact) -> ConstFact {
    match (self, other) {
      (ConstFact::Top, x) | (x, ConstFact::Top) => x,
      (ConstFact::Exact(a), ConstFact::Exact(b)) if a == b => ConstFact::Exact(a),
      _ => ConstFact::Bottom,
    }
  }
}

/// The result of analyzing one function: `entry[ip][r]` is `Some(c)`
/// exactly when EVERY path reaching bytecode position `ip` defines
/// register `r` as the identical literal `f64` value `c`; a real
/// constant-propagation "must" analysis, same fixed-point shape as
/// `IntFacts`/`ListFacts`/`StringFacts` (optimistic seed, narrowed by
/// intersection at merges, monotonically converges), just over a flat
/// per-register lattice (`ConstFact`) instead of a boolean bitset.
///
/// Seeded from a numeric `LoadConst` and propagated through `Move`;
/// deliberately NOT through arithmetic (`Add`/`Sub`/`Mul`/`Neg` on two
/// already-known constants COULD be folded further, and would be sound
/// to, but that is real constant-FOLDING, a distinct, larger feature
/// this analysis doesn't attempt; this one only tracks values that are
/// ALREADY sitting in a register as a literal, however far from their
/// own `LoadConst` that register travels unmodified). Currently
/// consulted only by `codegen::FuncCompiler::div_by_pow2_reciprocal`,
/// which is what makes it able to see PAST a constant hoisted into a
/// local variable and reused across many divisions (e.g. inside a
/// loop); not just the single immediately-preceding-`LoadConst`
/// shape a purely local, dataflow-free check could ever catch.
pub struct ConstFacts {
  entry: Vec<Vec<ConstFact>>,
}

impl ConstFacts {
  #[inline]
  pub fn const_value(&self, ip: usize, r: u8) -> Option<f64> {
    match self.entry[ip][r as usize] {
      ConstFact::Exact(bits) => Some(f64::from_bits(bits)),
      ConstFact::Top | ConstFact::Bottom => None,
    }
  }
}

fn transfer_const(in_facts: &[ConstFact], instr: &Instr, proto: &ObjFunction) -> Vec<ConstFact> {
  let mut out = in_facts.to_vec();
  match *instr {
    Instr::LoadConst { dst, const_idx } => {
      let c = &proto.chunk.constants[const_idx as usize];
      out[dst as usize] = if c.is_number() {
        ConstFact::Exact(c.as_number().to_bits())
      } else {
        ConstFact::Bottom
      };
    },
    Instr::Move { dst, src } => out[dst as usize] = in_facts[src as usize],

    // Everything else that writes a register either isn't a number at
    // all or isn't PROVABLY the same one every time (arithmetic on a
    // non-constant, a `Call`/`GetField`/`GetIndex` result, ...): see
    // `any_dst`'s own docs, and `transfer_list`'s identical reasoning
    // for why this must be the genuinely complete instruction list,
    // not a curated subset.
    _ => {
      if let Some(dst) = any_dst(instr) {
        out[dst as usize] = ConstFact::Bottom;
      }
    },
  }
  out
}

/// Runs the constant-propagation analysis: see `ConstFacts`'s own
/// docs. Short-circuits the same way `analyze_list`/`analyze_string`
/// do: with no numeric `LoadConst` anywhere in the function (the only
/// instruction `transfer_const` ever seeds `Exact` from), no register
/// can ever be proven a constant on any path.
pub fn analyze_const(proto: &ObjFunction, preds: &[Vec<usize>]) -> ConstFacts {
  let code = &proto.chunk.code;
  let code_len = code.len();
  let num_registers = proto.num_registers as usize;

  let has_numeric_const = code.iter().any(|i| match i {
    Instr::LoadConst { const_idx, .. } => proto.chunk.constants[*const_idx as usize].is_number(),
    _ => false,
  });
  if !has_numeric_const {
    return ConstFacts {
      entry: vec![vec![ConstFact::Bottom; num_registers]; code_len],
    };
  }

  let mut entry: Vec<Vec<ConstFact>> = (0..code_len)
    .map(|ip| {
      if ip == 0 {
        vec![ConstFact::Bottom; num_registers]
      } else {
        vec![ConstFact::Top; num_registers]
      }
    })
    .collect();

  let mut worklist: Vec<usize> = (0..code_len).collect();
  let mut in_worklist = vec![true; code_len];
  let mut out: Vec<Vec<ConstFact>> = (0..code_len)
    .map(|ip| transfer_const(&entry[ip], &code[ip], proto))
    .collect();

  while let Some(ip) = worklist.pop() {
    in_worklist[ip] = false;

    let mut new_in = vec![ConstFact::Top; num_registers];
    let mut any_pred = false;
    for &p in &preds[ip] {
      for r in 0..num_registers {
        new_in[r] = new_in[r].meet(out[p][r]);
      }
      any_pred = true;
    }
    if !any_pred {
      // Unreachable code; vacuously "everything proven" (`Top`) is
      // safe, same reasoning `RegSet::full()` gets for this case in
      // every other analysis here: nothing ever actually executes this
      // instruction, so whatever `codegen` does with an over-
      // optimistic fact here can never run. `Top` is genuinely never
      // returned by `const_value` (only `Exact`/`Bottom` are), so
      // this can't leak an unsound answer to a caller even for
      // unreachable code.
      new_in = vec![ConstFact::Top; num_registers];
    }
    if ip == 0 {
      new_in = vec![ConstFact::Bottom; num_registers];
    }

    if new_in != entry[ip] {
      entry[ip] = new_in;
      out[ip] = transfer_const(&entry[ip], &code[ip], proto);
      for &s in &successors(ip, &code[ip], proto) {
        if s < code_len && !in_worklist[s] {
          in_worklist[s] = true;
          worklist.push(s);
        }
      }
    }
  }

  ConstFacts { entry }
}

//-----------------------------------------------------------------------------------
// Reference classification
//-----------------------------------------------------------------------------------

/// The result of analyzing one function: `entry[ip]` is exactly the set
/// of registers proven to never hold a GC-managed reference (a String,
/// BigInt, List, Dict, Func, Closure, Class, Instance, Range, Module,
/// ...) on every path reaching bytecode position `ip`; i.e. always
/// one of the three non-reference NaN-boxed types (number, bool, nil)
/// there. This is a "must" analysis with the same shape as `TypeFacts`
/// (optimistic `full()` seed at every non-entry block, narrowed by
/// intersection at merges) for the same reason: a register only counts
/// as proven non-reference if every path agrees, and a register never
/// proven here is conservatively treated as "might be a reference";
/// the safe direction to be wrong in, since this feeds a GC safepoint's
/// decision about which registers need to be spilled and scanned as
/// roots (see the JIT SSA plan's Stage 4). Getting this backwards
/// (falsely proving "never a reference") would be a genuine
/// memory-safety bug, not just a missed optimization, so unlike
/// `TypeFacts`'s `speculative_regs` hook, this analysis has no
/// profiling-based speculation escape hatch; every fact here is a
/// real proof or nothing.
pub struct RefFacts {
  entry: Vec<RegSet>,
}

impl RefFacts {
  #[inline]
  pub fn is_never_ref(&self, ip: usize, r: u8) -> bool {
    self.entry[ip].get(r)
  }
}

/// Runs the reference-classification analysis, given `type_facts` (the
/// result of `analyze` on the same function) as an auxiliary input.
/// Several instructions here (`Add`, `Sub`, `Mul`, `Lt`, `BitAnd`, ...)
/// can each produce either a plain number or a genuine reference at
/// runtime depending on their operands' types; e.g. `Add` allocates a
/// new `BigInt`/`String`/`List` when its operands call for one (see
/// `VM::binary_add_values`), and `Lt`/`Le`/`Gt`/`Ge` fall through to a
/// user-defined `try_operator_override` (which can return literally
/// anything) whenever their operands aren't both provably numeric;
/// but the moment both operands are proven numeric by `type_facts`, the
/// interpreter's own plain-number fast path is the only branch that can
/// possibly fire (every other branch requires an operand that isn't a
/// number), so the result is provably a plain number too. This is
/// exactly why `type_facts`; not a redundant, independently-computed
/// copy of the same fact; is threaded in as a parameter: reusing the
/// same proof `codegen` already relies on for guard elision keeps the
/// two analyses from ever silently drifting apart.
///
/// `Eq`/`Neq`/`EqImm`/`NeqImm` are the one case that's unconditionally
/// non-reference regardless of operand types: they call `Value::equals`
/// directly with no operator-override hook whatsoever (unlike every
/// other comparison), so their result is provably a bool no matter
/// what `a`/`b` are.
pub fn classify_refs(proto: &ObjFunction, type_facts: &TypeFacts) -> RefFacts {
  let code = &proto.chunk.code;
  let code_len = code.len();
  let num_registers = proto.num_registers as usize;

  let preds = build_predecessors(proto);

  let mut entry: Vec<RegSet> = (0..code_len)
    .map(|ip| {
      if ip == 0 {
        RegSet::empty(num_registers)
      } else {
        RegSet::full(num_registers)
      }
    })
    .collect();

  let mut worklist: Vec<usize> = (0..code_len).collect();
  let mut in_worklist = vec![true; code_len];
  let mut out: Vec<RegSet> = (0..code_len)
    .map(|ip| ref_transfer(&entry[ip], ip, &code[ip], proto, type_facts))
    .collect();

  while let Some(ip) = worklist.pop() {
    in_worklist[ip] = false;

    let mut new_in = RegSet::full(num_registers);
    let mut any_pred = false;
    for &p in &preds[ip] {
      new_in.and_assign(&out[p]);
      any_pred = true;
    }
    if !any_pred {
      // Unreachable code; vacuously "everything proven" is safe, same
      // reasoning as `analyze`'s identical case.
      new_in = RegSet::full(num_registers);
    }
    if ip == 0 {
      new_in = RegSet::empty(num_registers);
    }

    if new_in != entry[ip] {
      entry[ip] = new_in;
      out[ip] = ref_transfer(&entry[ip], ip, &code[ip], proto, type_facts);
      for &s in &successors(ip, &code[ip], proto) {
        if s < code_len && !in_worklist[s] {
          in_worklist[s] = true;
          worklist.push(s);
        }
      }
    }
  }

  RefFacts { entry }
}

/// What a single bytecode instruction proves/invalidates about
/// register reference-freedom, given what was proven on entry to it
/// (`in_set`) and the numeric proof already established for the same
/// `ip` by `analyze` (`type_facts`). See `classify_refs`'s own docs for
/// why arithmetic/comparison instructions consult `type_facts` rather
/// than re-deriving numeric-ness independently, and for the verified,
/// source-checked justification behind every branch below; this
/// isn't inferred from instruction names, it was confirmed against
/// each instruction's actual `VM` handler in `vm.rs`.
fn ref_transfer(
  in_set: &RegSet,
  ip: usize,
  instr: &Instr,
  proto: &ObjFunction,
  type_facts: &TypeFacts,
) -> RegSet {
  let mut out = in_set.clone();
  let both_numeric = |a: u8, b: u8| type_facts.is_numeric(ip, a) && type_facts.is_numeric(ip, b);

  match *instr {
    // Never a reference, unconditionally.
    Instr::LoadNil { dst } | Instr::LoadBool { dst, .. } | Instr::Not { dst, .. } => {
      out.set(dst, true)
    },
    // `Eq`/`Neq` call `Value::equals` directly; no operator-override
    // hook at all, unlike every other comparison; so the result is
    // provably a bool regardless of operand types.
    Instr::Eq { dst, .. }
    | Instr::Neq { dst, .. }
    | Instr::EqImm { dst, .. }
    | Instr::NeqImm { dst, .. } => out.set(dst, true),

    // A compile-time constant's own reference-ness is a fixed, static
    // fact; `!is_obj()` covers all three non-reference NaN-boxed
    // types at once (number, bool, nil), unlike `transfer`'s own
    // `is_number()`-only check for its narrower numeric-provenance
    // purpose.
    Instr::LoadConst { dst, const_idx } => {
      let is_obj = proto.chunk.constants[const_idx as usize].is_obj();
      out.set(dst, !is_obj);
    },
    Instr::Move { dst, src } => out.set(dst, in_set.get(src)),

    // Provably non-reference only when both operands are provably
    // numeric; that's exactly what forces the interpreter down its
    // plain-number fast path, bypassing every bigint/string/list/
    // operator-override branch that could otherwise produce a
    // reference. See `VM::binary_numeric`/`binary_add_values`/
    // `binary_mult`/`bitwise_numeric`/`compare`.
    Instr::Add { dst, a, b }
    | Instr::Sub { dst, a, b }
    | Instr::Mul { dst, a, b }
    | Instr::Div { dst, a, b }
    | Instr::Pow { dst, a, b }
    | Instr::Floor { dst, a, b }
    | Instr::Mod { dst, a, b }
    | Instr::BitAnd { dst, a, b }
    | Instr::BitOr { dst, a, b }
    | Instr::BitXor { dst, a, b }
    | Instr::BitShl { dst, a, b }
    | Instr::BitShr { dst, a, b }
    | Instr::BitUshr { dst, a, b }
    | Instr::Lt { dst, a, b }
    | Instr::Le { dst, a, b }
    | Instr::Gt { dst, a, b }
    | Instr::Ge { dst, a, b } => out.set(dst, both_numeric(a, b)),
    Instr::Neg { dst, src } | Instr::BitNot { dst, src } => {
      out.set(dst, type_facts.is_numeric(ip, src))
    },
    // The immediate operand is always a numeric constant by
    // construction (same fact `analyze`'s own `AddImm`/`SubImm`/
    // `MulImm` case relies on); only `a` needs checking.
    Instr::AddImm { dst, a, .. }
    | Instr::SubImm { dst, a, .. }
    | Instr::MulImm { dst, a, .. }
    | Instr::LtImm { dst, a, .. }
    | Instr::LeImm { dst, a, .. }
    | Instr::GtImm { dst, a, .. }
    | Instr::GeImm { dst, a, .. } => out.set(dst, type_facts.is_numeric(ip, a)),

    // Always a reference, unconditionally, on every successful path --
    // see `classify_refs`'s own docs / this pass's verification notes
    // for the source citations behind each of these.
    Instr::Concat { dst, .. }
    | Instr::MakeRange { dst, .. }
    | Instr::MakeList { dst, .. }
    | Instr::MakeDict { dst, .. }
    | Instr::MakeClass { dst, .. }
    | Instr::Closure { dst, .. }
    | Instr::Import { dst, .. }
    | Instr::MakePromoted { dst, .. }
    | Instr::GetSlice { dst, .. } => out.set(dst, false),

    // Never statically provable either way; depends on arbitrary
    // runtime container contents, field values, or user/native code.
    Instr::Call { dst, .. }
    | Instr::GetGlobal { dst, .. }
    | Instr::GetUpval { dst, .. }
    | Instr::GetField { dst, .. }
    | Instr::Invoke { dst, .. }
    | Instr::InvokeSuper { dst, .. }
    | Instr::CallSuperCtor { dst, .. }
    | Instr::GetIndex { dst, .. } => out.set(dst, false),

    // No destination register written at all; facts pass through
    // unchanged, same instruction list `transfer` uses for the same
    // reason.
    Instr::SetGlobal { .. }
    | Instr::AssignGlobal { .. }
    | Instr::SetUpval { .. }
    | Instr::CloseUpvalues { .. }
    | Instr::DeclareField { .. }
    | Instr::SetFieldInit { .. }
    | Instr::SetMethod { .. }
    | Instr::DeclareStatic { .. }
    | Instr::FinalizeClass { .. }
    | Instr::SetField { .. }
    | Instr::ImportAll { .. }
    | Instr::SetIndex { .. }
    | Instr::UsingJump { .. }
    | Instr::Print { .. }
    | Instr::Return { .. }
    | Instr::Jmp { .. }
    | Instr::JmpIfFalse { .. }
    | Instr::JmpIfTrue { .. }
    | Instr::CheckParamType { .. } => {},

    // A raise writes no register, so it proves and invalidates nothing
    //; exactly like `Return` above. It compiles to a deopt rather than
    // real unwind logic (see `codegen::compile`'s eligibility scan), and
    // `successors` deliberately still gives it a fallthrough edge so no
    // block becomes unreachable.
    Instr::Raise { .. } => {},

    Instr::PushCatch { .. } | Instr::PopCatch => {
      unreachable!("excluded from compilation before this analysis ever runs")
    },
  }
  out
}

/// What a single bytecode instruction proves/invalidates about
/// register numeric-ness, given what was proven on entry to it.
/// `speculative_regs` overrides the "conservative, always unproven"
/// destinations below to numeric where the caller's profiling sample
/// says so: see `SpeculativeRegs`'s own docs.
fn transfer(in_set: &RegSet, instr: &Instr, proto: &ObjFunction, speculative_regs: u64) -> RegSet {
  let mut out = in_set.clone();
  match *instr {
    Instr::LoadConst { dst, const_idx } => {
      // Constants never change after compilation, so whether the
      // loaded value is numeric is itself a static fact; common for
      // loop-carried accumulators/counters seeded with e.g. `0`.
      let is_numeric_const = proto.chunk.constants[const_idx as usize].is_number();
      out.set(dst, is_numeric_const);
    },
    Instr::LoadNil { dst } | Instr::LoadBool { dst, .. } => out.set(dst, false),
    Instr::Move { dst, src } => out.set(dst, in_set.get(src)),

    Instr::Add { dst, a, b }
    | Instr::Sub { dst, a, b }
    | Instr::Mul { dst, a, b }
    | Instr::Div { dst, a, b }
    | Instr::BitAnd { dst, a, b }
    | Instr::BitOr { dst, a, b }
    | Instr::BitXor { dst, a, b }
    | Instr::BitShl { dst, a, b }
    | Instr::BitShr { dst, a, b }
    | Instr::BitUshr { dst, a, b } => {
      out.set(dst, in_set.get(a) && in_set.get(b));
    },
    Instr::Neg { dst, src } | Instr::BitNot { dst, src } => out.set(dst, in_set.get(src)),
    Instr::AddImm { dst, a, .. } | Instr::SubImm { dst, a, .. } | Instr::MulImm { dst, a, .. } => {
      // The immediate operand is always a numeric constant by
      // construction (that's what makes it foldable into an `*Imm`
      // instruction in the first place).
      out.set(dst, in_set.get(a));
    },

    // Every comparison and `Not` produces a bool, never a number.
    Instr::Eq { dst, .. }
    | Instr::Neq { dst, .. }
    | Instr::Lt { dst, .. }
    | Instr::Le { dst, .. }
    | Instr::Gt { dst, .. }
    | Instr::Ge { dst, .. }
    | Instr::Not { dst, .. }
    | Instr::LtImm { dst, .. }
    | Instr::LeImm { dst, .. }
    | Instr::GtImm { dst, .. }
    | Instr::GeImm { dst, .. }
    | Instr::EqImm { dst, .. }
    | Instr::NeqImm { dst, .. } => out.set(dst, false),

    // Never numeric results (string, or otherwise never-a-number).
    Instr::Concat { dst, .. } => out.set(dst, false),
    Instr::Pow { dst, .. } | Instr::Floor { dst, .. } | Instr::Mod { dst, .. } => {
      out.set(dst, false)
    },

    // Any instruction whose result depends on something this pass
    // can't see statically (heap contents, globals, call results, ...)
    //; conservatively not proven, UNLESS the caller's profiling
    // sample observed this exact destination register holding a
    // number at the moment compilation triggered (`speculative_regs`)
    //: see `SpeculativeRegs`'s own docs. `codegen` is what actually
    // makes this safe: it never emits code that trusts this without a
    // real runtime guard planted right here, at this instruction's own
    // definition site.
    Instr::Call { dst, .. }
    | Instr::GetGlobal { dst, .. }
    | Instr::Closure { dst, .. }
    | Instr::GetUpval { dst, .. }
    | Instr::MakeList { dst, .. }
    | Instr::MakeDict { dst, .. }
    | Instr::MakeClass { dst, .. }
    | Instr::GetField { dst, .. }
    | Instr::Invoke { dst, .. }
    | Instr::InvokeSuper { dst, .. }
    | Instr::CallSuperCtor { dst, .. }
    | Instr::Import { dst, .. }
    | Instr::MakePromoted { dst, .. }
    | Instr::GetIndex { dst, .. }
    | Instr::GetSlice { dst, .. }
    | Instr::MakeRange { dst, .. } => {
      let speculated = dst < 64 && (speculative_regs >> dst) & 1 != 0;
      out.set(dst, speculated);
    },

    // A parameter whose declared type is exactly (a union of only)
    // `number`/`int`, with no `?nullable`, is PROVABLY numeric on every
    // path past this instruction; it just raised otherwise, and this
    // is a "must" analysis over facts that hold given execution reached
    // here at all. Anything else (a union that also allows a
    // non-numeric type, or a nullable one nil could slip through) stays
    // unproven, same as any other guard this pass doesn't specifically
    // recognize.
    Instr::CheckParamType { reg, check_idx } => {
      let check = &proto.chunk.param_checks[check_idx as usize];
      let all_numeric = !check.nullable
        && check
          .types
          .iter()
          .all(|t| matches!(t, ParamType::Number | ParamType::Int));
      out.set(reg, all_numeric);
    },

    // No destination register written at all; facts pass through
    // unchanged.
    Instr::SetGlobal { .. }
    | Instr::AssignGlobal { .. }
    | Instr::SetUpval { .. }
    | Instr::CloseUpvalues { .. }
    | Instr::DeclareField { .. }
    | Instr::SetFieldInit { .. }
    | Instr::SetMethod { .. }
    | Instr::DeclareStatic { .. }
    | Instr::FinalizeClass { .. }
    | Instr::SetField { .. }
    | Instr::ImportAll { .. }
    | Instr::SetIndex { .. }
    | Instr::UsingJump { .. }
    | Instr::Print { .. }
    | Instr::Return { .. }
    | Instr::Jmp { .. }
    | Instr::JmpIfFalse { .. }
    | Instr::JmpIfTrue { .. } => {},

    // A raise writes no register, so it proves and invalidates nothing
    //; exactly like `Return` above. It compiles to a deopt rather than
    // real unwind logic (see `codegen::compile`'s eligibility scan), and
    // `successors` deliberately still gives it a fallthrough edge so no
    // block becomes unreachable.
    Instr::Raise { .. } => {},

    Instr::PushCatch { .. } | Instr::PopCatch => {
      unreachable!("excluded from compilation before this analysis ever runs")
    },
  }
  out
}

/// The destination register of `instr`, if it's one of `transfer`'s
/// "conservative, always unproven unless speculated" instructions;
/// exactly the same instruction list as that match arm above (making
/// this a single source of truth would require restructuring
/// `transfer` itself; until then, the two must be kept in sync by
/// hand, the same way `zuri_jit_invoke_prepare`'s own doc comment
/// already flags its parameter order needing to match
/// `emit_fast_call`'s calling convention by hand). `codegen::
/// FuncCompiler` calls this once per instruction, right after emitting
/// it in the specialized body, to decide whether a mid-function
/// guard-and-fork belongs there: see its own docs.
pub fn conservative_dst(instr: &Instr) -> Option<u8> {
  match *instr {
    Instr::Call { dst, .. }
    | Instr::GetGlobal { dst, .. }
    | Instr::Closure { dst, .. }
    | Instr::GetUpval { dst, .. }
    | Instr::MakeList { dst, .. }
    | Instr::MakeDict { dst, .. }
    | Instr::MakeClass { dst, .. }
    | Instr::GetField { dst, .. }
    | Instr::Invoke { dst, .. }
    | Instr::InvokeSuper { dst, .. }
    | Instr::CallSuperCtor { dst, .. }
    | Instr::Import { dst, .. }
    | Instr::MakePromoted { dst, .. }
    | Instr::GetIndex { dst, .. }
    | Instr::GetSlice { dst, .. }
    | Instr::MakeRange { dst, .. } => Some(dst),
    _ => None,
  }
}

/// The destination register any instruction writes, if it writes one
/// at all; a strict superset of `conservative_dst` (which only
/// covers the "unprovable, needs a runtime guard to speculate on"
/// subset). Used by `ambiguous_speculative_regs` to see every
/// definition of a register, not just the speculatable ones; kept
/// as its own function, deliberately not folded into `conservative_dst`
/// itself, since callers that only care about "which registers might
/// need a runtime guard" (`codegen::FuncCompiler::emit_speculative_guard`)
/// would otherwise have to filter this broader set back down by hand.
/// `pub(crate)` (not just used internally) so `codegen::FuncCompiler::
/// call_helper` can also use it: see its own docs on why a helper-
/// backed instruction's `dst` needs staling even when it's not part of
/// `live_in` at that `ip`.
pub(crate) fn any_dst(instr: &Instr) -> Option<u8> {
  match *instr {
    Instr::LoadConst { dst, .. }
    | Instr::LoadNil { dst }
    | Instr::LoadBool { dst, .. }
    | Instr::Move { dst, .. }
    | Instr::Add { dst, .. }
    | Instr::Sub { dst, .. }
    | Instr::Mul { dst, .. }
    | Instr::Div { dst, .. }
    | Instr::Pow { dst, .. }
    | Instr::Floor { dst, .. }
    | Instr::Mod { dst, .. }
    | Instr::Neg { dst, .. }
    | Instr::Not { dst, .. }
    | Instr::Concat { dst, .. }
    | Instr::BitAnd { dst, .. }
    | Instr::BitOr { dst, .. }
    | Instr::BitXor { dst, .. }
    | Instr::BitShl { dst, .. }
    | Instr::BitShr { dst, .. }
    | Instr::BitUshr { dst, .. }
    | Instr::BitNot { dst, .. }
    | Instr::Eq { dst, .. }
    | Instr::Neq { dst, .. }
    | Instr::Lt { dst, .. }
    | Instr::Le { dst, .. }
    | Instr::Gt { dst, .. }
    | Instr::Ge { dst, .. }
    | Instr::Call { dst, .. }
    | Instr::GetGlobal { dst, .. }
    | Instr::Closure { dst, .. }
    | Instr::GetUpval { dst, .. }
    | Instr::MakeList { dst, .. }
    | Instr::MakeDict { dst, .. }
    | Instr::MakeClass { dst, .. }
    | Instr::GetField { dst, .. }
    | Instr::Invoke { dst, .. }
    | Instr::InvokeSuper { dst, .. }
    | Instr::CallSuperCtor { dst, .. }
    | Instr::Import { dst, .. }
    | Instr::MakePromoted { dst, .. }
    | Instr::GetIndex { dst, .. }
    | Instr::GetSlice { dst, .. }
    | Instr::MakeRange { dst, .. }
    | Instr::AddImm { dst, .. }
    | Instr::SubImm { dst, .. }
    | Instr::MulImm { dst, .. }
    | Instr::LtImm { dst, .. }
    | Instr::LeImm { dst, .. }
    | Instr::GtImm { dst, .. }
    | Instr::GeImm { dst, .. }
    | Instr::EqImm { dst, .. }
    | Instr::NeqImm { dst, .. } => Some(dst),
    _ => None,
  }
}

/// Registers written by more than one distinct static definition site
/// in `code` (any instruction that writes a register at all, not just
/// speculatable ones); unsafe to seed a speculative guess onto,
/// since a one-shot runtime snapshot of "what's in this register right
/// now" (see `VM::sample_all_reg_types`) can't say which of the
/// register's multiple, possibly-unrelated definitions it actually
/// observed. Two confirmed real patterns this catches: a call's own
/// callee-load register reused, in place, for the call's result
/// (`GetGlobal dst=r` immediately followed by `Call dst=r, func=r`);
/// the snapshot sees the numeric result and wrongly also credits the
/// callee load, which is never a number; and a receiver register
/// reused for a method call's (non-numeric) return value while also
/// being written elsewhere by something that genuinely is numeric
/// (e.g. sharing a slot with a loop counter across non-overlapping
/// live ranges); the snapshot can catch either moment and wrongly
/// credit the other. Both make an `emit_speculative_guard` check that
/// fails on every invocation, not occasionally: see `analyze`'s own
/// docs at its `spec_regs` computation.
fn ambiguous_speculative_regs(code: &[Instr]) -> u64 {
  let mut seen: u64 = 0;
  let mut ambiguous: u64 = 0;
  for instr in code {
    let Some(dst) = any_dst(instr) else {
      continue;
    };
    if dst >= 64 {
      continue;
    }
    let bit = 1u64 << dst;
    if seen & bit != 0 {
      ambiguous |= bit;
    }
    seen |= bit;
  }
  ambiguous
}

/// Every bytecode position `ip`'s instruction can transfer control to,
/// including the implicit fallthrough to `ip + 1` where applicable;
/// the forward edges the fixed-point worklist propagates facts along.
/// `pub(crate)` (not just used internally) so other fixed-point passes
/// over the same bytecode shape (e.g. `jit::escape`'s may-alias
/// analysis) reuse this exact, already-correct control-flow edge logic
/// instead of each re-deriving their own copy that could drift out of
/// sync with real jump/branch/dispatch semantics.
pub(crate) fn successors(ip: usize, instr: &Instr, proto: &ObjFunction) -> Vec<usize> {
  match *instr {
    Instr::Jmp { offset } => vec![(ip as isize + 1 + offset as isize) as usize],
    Instr::JmpIfFalse { offset, .. } | Instr::JmpIfTrue { offset, .. } => {
      vec![(ip as isize + 1 + offset as isize) as usize, ip + 1]
    },
    Instr::UsingJump { table_idx, .. } => {
      let mut targets: Vec<usize> = proto.chunk.jump_tables[table_idx as usize]
        .values()
        .copied()
        .collect();
      targets.push(ip + 1); // miss falls through
      targets
    },
    Instr::Return { .. } => vec![],
    _ => vec![ip + 1],
  }
}

/// Predecessor list for every bytecode position, built once up front
/// (a single forward scan) rather than inverting `successors` on every
/// worklist pop. `pub(crate)` for the same reason `successors` is:
/// see its own docs.
pub(crate) fn build_predecessors(proto: &ObjFunction) -> Vec<Vec<usize>> {
  let code = &proto.chunk.code;
  let code_len = code.len();
  let mut preds: Vec<Vec<usize>> = vec![Vec::new(); code_len];
  let mut seen: FxHashMap<(usize, usize), ()> = FxHashMap::default();
  for (ip, instr) in code.iter().enumerate() {
    for s in successors(ip, instr, proto) {
      if s < code_len && seen.insert((ip, s), ()).is_none() {
        preds[s].push(ip);
      }
    }
  }
  preds
}

//-----------------------------------------------------------------------------------
// Liveness analysis
//-----------------------------------------------------------------------------------

/// The result of analyzing one function: `live_in[ip]` is exactly the
/// set of registers that might still be needed on some path forward
/// from bytecode position `ip`, including whatever `ip`'s own
/// instruction itself reads; i.e. precisely the registers that must
/// hold a correct, up-to-date value in `VM::registers` at the moment
/// `ip` is about to execute. This is what `jit::codegen` consults at
/// every sync point (a call, a GC safepoint, a deopt/guard branch, a
/// stack-map spill site) to decide exactly which cached register
/// values need a real `store_reg` there; never "everything," never
/// "nothing," just what's actually live. See this module's own
/// `liveness` doc comment for why "live_in" (not "live_out") is the
/// right quantity for that: it already folds in both what `ip` itself
/// is about to read and whatever survives it for later, via the
/// standard equation `live_in[ip] = uses(ip) ∪ (live_out[ip] -
/// defs(ip))`.
pub struct LivenessFacts {
  live_in: Vec<RegSet>,
}

impl LivenessFacts {
  #[inline]
  pub fn is_live(&self, ip: usize, r: u8) -> bool {
    self.live_in[ip].get(r)
  }

  /// Every register live immediately before `ip`'s own instruction
  /// executes, low to high; what a spill-site emitter actually
  /// iterates over to know which cached values need flushing.
  pub fn live_regs_at(&self, ip: usize) -> impl Iterator<Item = u8> + '_ {
    self.live_in[ip].iter_set()
  }
}

/// Runs a standard backward "may" liveness analysis: a register is live
/// at a point if there exists some path forward from there on which its
/// current value is read before being overwritten. Unlike `analyze`'s
/// numeric-facts pass (a "must" analysis, seeded optimistically full and
/// narrowed by intersection at merges, since a fact only holds if every
/// path agrees), this is seeded empty and grows by union at merges,
/// since a register only needs to be considered dead if no path forward
/// needs it; the textbook fixed point for liveness, guaranteed to
/// converge because each `RegSet` only ever grows and is bounded above
/// by "every register."
/// `preds`: see `analyze`'s own docs on why this takes it as a
/// parameter instead of computing it fresh.
pub fn liveness(proto: &ObjFunction, preds: &[Vec<usize>]) -> LivenessFacts {
  let code = &proto.chunk.code;
  let code_len = code.len();
  let num_registers = proto.num_registers as usize;

  let mut live_in: Vec<RegSet> = vec![RegSet::empty(num_registers); code_len];
  let mut live_out: Vec<RegSet> = vec![RegSet::empty(num_registers); code_len];

  let mut worklist: Vec<usize> = (0..code_len).rev().collect();
  let mut in_worklist = vec![true; code_len];

  while let Some(ip) = worklist.pop() {
    in_worklist[ip] = false;

    let mut new_out = RegSet::empty(num_registers);
    for &s in &successors(ip, &code[ip], proto) {
      if s < code_len {
        new_out.or_assign(&live_in[s]);
      }
    }
    let out_changed = new_out != live_out[ip];
    if out_changed {
      live_out[ip] = new_out;
    }

    let mut new_in = live_out[ip].clone();
    if let Some(d) = any_dst(&code[ip]) {
      new_in.set(d, false);
    }
    mark_uses(&code[ip], proto, &mut new_in);

    if new_in != live_in[ip] {
      live_in[ip] = new_in;
      for &p in &preds[ip] {
        if !in_worklist[p] {
          in_worklist[p] = true;
          worklist.push(p);
        }
      }
    }
  }

  LivenessFacts { live_in }
}

/// Marks every register `instr` reads (never what it writes: see
/// `any_dst` for that) into `set`. Kept as its own pass over the same
/// field layout `transfer`/`any_dst` already match on, rather than
/// folding into either: `transfer` cares about numeric-ness of a
/// destination, `any_dst` cares only about the (single, if any)
/// destination, and this cares only about sources; three genuinely
/// different questions asked of the same instruction shape, no single
/// match arm answers all three without being harder to read than three
/// smaller ones.
///
/// A few instructions read a whole contiguous register window rather
/// than a fixed handful of named operands (`Call`'s own callee register
/// plus its `num_args` argument registers immediately after it;
/// `Invoke`/`InvokeSuper`/`CallSuperCtor`'s receiver/superclass register
/// plus the duplicated-`self` and argument registers per their own doc
/// comments in `vm::chunk::Instr`; `MakeList`/`MakeDict`'s element run)
///; `RegSet::set_range` covers those directly from the instruction's
/// own `start`/`count`-style fields, needing no extra bookkeeping beyond
/// what's already encoded in the bytecode.
///
/// `Closure` is the one case that reads registers not named anywhere in
/// the instruction itself: it captures its nested prototype's own
/// `UpvalueDescriptor::Local(n)` entries out of the currently executing
/// (enclosing) function's registers at the moment the closure is
/// created (see `ObjFunction::upvalues`'s own doc comment); missing
/// one of these would let a captured local's register be treated as
/// dead and reused/discarded before the closure actually reads it,
/// silently capturing the wrong value.
fn mark_uses(instr: &Instr, proto: &ObjFunction, set: &mut RegSet) {
  use crate::vm::object::UpvalueDescriptor;

  match *instr {
    Instr::LoadConst { .. } | Instr::LoadNil { .. } | Instr::LoadBool { .. } => {},

    Instr::Move { src, .. }
    | Instr::Neg { src, .. }
    | Instr::BitNot { src, .. }
    | Instr::Not { src, .. } => set.set(src, true),

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
    | Instr::Eq { a, b, .. }
    | Instr::Neq { a, b, .. }
    | Instr::Lt { a, b, .. }
    | Instr::Le { a, b, .. }
    | Instr::Gt { a, b, .. }
    | Instr::Ge { a, b, .. } => {
      set.set(a, true);
      set.set(b, true);
    },
    Instr::AddImm { a, .. }
    | Instr::SubImm { a, .. }
    | Instr::MulImm { a, .. }
    | Instr::LtImm { a, .. }
    | Instr::LeImm { a, .. }
    | Instr::GtImm { a, .. }
    | Instr::GeImm { a, .. }
    | Instr::EqImm { a, .. }
    | Instr::NeqImm { a, .. } => set.set(a, true),

    Instr::Jmp { .. } => {},
    Instr::JmpIfFalse { cond, .. } | Instr::JmpIfTrue { cond, .. } => set.set(cond, true),

    Instr::Call { func, num_args, .. } => set.set_range(func, num_args as usize + 1),
    Instr::Return { src } | Instr::Print { src } | Instr::Raise { src } => set.set(src, true),

    Instr::GetGlobal { .. } => {},
    Instr::SetGlobal { src, .. } | Instr::AssignGlobal { src, .. } => set.set(src, true),

    Instr::Closure { proto_const, .. } => {
      let nested = proto.chunk.constants[proto_const as usize].as_func();
      for desc in &nested.upvalues {
        if let UpvalueDescriptor::Local(n) = *desc {
          set.set(n, true);
        }
      }
    },
    Instr::GetUpval { .. } => {},
    Instr::SetUpval { src, .. } => set.set(src, true),
    Instr::CloseUpvalues { from } => {
      // Conservatively "reads" every register from `from` up to the
      // function's own top; we don't statically know which of them
      // currently has an open upvalue, and this only runs once per
      // block exit, so there's no meaningful cost to being precise-but-
      // safe here rather than plumbing open-upvalue tracking into a
      // purely static pass.
      set.set_range(from, proto.num_registers as usize - from as usize);
    },

    Instr::MakeList { start, count, .. } => set.set_range(start, count as usize),
    Instr::MakeDict { start, count, .. } => set.set_range(start, count as usize * 2),

    Instr::MakeClass { superclass, .. } => {
      if let Some(s) = superclass {
        set.set(s, true);
      }
    },
    Instr::DeclareField { class, .. } | Instr::FinalizeClass { class } => set.set(class, true),
    Instr::SetFieldInit { class, src }
    | Instr::SetMethod { class, src, .. }
    | Instr::DeclareStatic { class, src, .. } => {
      set.set(class, true);
      set.set(src, true);
    },

    Instr::GetField { obj, .. } => set.set(obj, true),
    Instr::SetField { obj, src, .. } => {
      set.set(obj, true);
      set.set(src, true);
    },

    // `obj` itself, plus the compiler-duplicated `self` at `obj + 1`,
    // plus `num_args` more argument registers after that: see these
    // variants' own doc comments in `vm::chunk::Instr`.
    Instr::Invoke { obj, num_args, .. } => set.set_range(obj, num_args as usize + 2),
    Instr::InvokeSuper {
      superclass,
      num_args,
      ..
    }
    | Instr::CallSuperCtor {
      superclass,
      num_args,
      ..
    } => set.set_range(superclass, num_args as usize + 2),

    Instr::Import { .. } => {},
    Instr::ImportAll { module } => set.set(module, true),
    Instr::MakePromoted { module, .. } => set.set(module, true),

    Instr::GetIndex { obj, idx, .. } => {
      set.set(obj, true);
      set.set(idx, true);
    },
    Instr::SetIndex { obj, idx, src } => {
      set.set(obj, true);
      set.set(idx, true);
      set.set(src, true);
    },
    Instr::GetSlice { obj, lo, hi, .. } => {
      set.set(obj, true);
      set.set(lo, true);
      set.set(hi, true);
    },
    Instr::MakeRange { lower, upper, .. } => {
      set.set(lower, true);
      set.set(upper, true);
    },

    Instr::UsingJump { subject, .. } => set.set(subject, true),

    Instr::CheckParamType { reg, .. } => set.set(reg, true),

    Instr::PushCatch { .. } | Instr::PopCatch => {
      unreachable!("excluded from compilation before this analysis ever runs")
    },
  }
}

#[cfg(test)]
mod liveness_tests {
  use std::rc::Rc;

  use super::*;
  use crate::vm::chunk::Chunk;
  use crate::vm::object::{JitInfo, Obj, UpvalueDescriptor};
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

  #[test]
  fn straight_line_dead_after_last_use() {
    let code = vec![
      Instr::LoadConst {
        dst: 0,
        const_idx: 0,
      },
      Instr::LoadConst {
        dst: 1,
        const_idx: 1,
      },
      Instr::Add { dst: 2, a: 0, b: 1 },
      Instr::Return { src: 2 },
    ];
    let f = make_func(code, vec![Value::number(1.0), Value::number(2.0)], 3);
    let facts = liveness(&f, &build_predecessors(&f));
    assert!(facts.is_live(3, 2), "Return reads r2");
    assert!(!facts.is_live(3, 0));
    assert!(!facts.is_live(3, 1));
    assert!(facts.is_live(2, 0), "Add reads r0");
    assert!(facts.is_live(2, 1), "Add reads r1");
    assert!(!facts.is_live(2, 2), "r2 not yet defined before ip2 runs");
    assert!(facts.is_live(1, 0), "r0 must survive to ip2");
    assert!(!facts.is_live(0, 0), "not live before its own definition");
    assert!(!facts.is_live(0, 1));
  }

  #[test]
  fn loop_back_edge_keeps_register_live_across_iterations() {
    let code = vec![
      Instr::LoadConst {
        dst: 0,
        const_idx: 0,
      }, // ip0: r0 = outer value
      Instr::LoadConst {
        dst: 1,
        const_idx: 1,
      }, // ip1: r1 = acc = 0
      Instr::Add { dst: 1, a: 1, b: 0 }, // ip2: r1 = r1 + r0 (loop body)
      Instr::JmpIfTrue {
        cond: 1,
        offset: -2,
      }, // ip3: back to ip2 if r1 truthy
      Instr::Return { src: 1 },          // ip4
    ];
    let f = make_func(code, vec![Value::number(5.0), Value::number(0.0)], 2);
    let facts = liveness(&f, &build_predecessors(&f));
    assert!(facts.is_live(2, 0), "r0 needed inside loop body");
    assert!(facts.is_live(3, 0), "r0 still needed across the back-edge");
    assert!(facts.is_live(1, 0), "r0 needed before first loop entry");
    assert!(!facts.is_live(4, 0), "r0 dead once the loop has exited");
    assert!(!facts.is_live(0, 0), "not live before its own definition");
    assert!(facts.is_live(4, 1), "Return reads r1");
    assert!(!facts.is_live(0, 1));
  }

  #[test]
  fn call_uses_callee_and_argument_window() {
    let code = vec![
      Instr::LoadConst {
        dst: 5,
        const_idx: 0,
      },
      Instr::Call {
        dst: 5,
        func: 5,
        num_args: 2,
      }, // reads r5(func), r6, r7
      Instr::Return { src: 5 },
    ];
    let f = make_func(code, vec![Value::number(1.0)], 8);
    let facts = liveness(&f, &build_predecessors(&f));
    assert!(facts.is_live(1, 5), "Call reads its own callee register");
    assert!(facts.is_live(1, 6), "Call reads argument 0");
    assert!(facts.is_live(1, 7), "Call reads argument 1");
    assert!(!facts.is_live(1, 8), "one past the argument window");
  }

  #[test]
  fn closure_marks_captured_locals_live() {
    let mut nested = make_func(vec![Instr::Return { src: 0 }], vec![], 1);
    // Captures the enclosing function's local register 3.
    nested.upvalues = vec![UpvalueDescriptor::Local(3)];
    let nested_ptr: &'static Obj = Box::leak(Box::new(Obj::Func(Box::new(nested))));
    let nested_val = Value::obj(nested_ptr as *const Obj);

    let code = vec![
      Instr::LoadConst {
        dst: 3,
        const_idx: 0,
      }, // ip0: define r3
      Instr::Closure {
        dst: 4,
        proto_const: 1,
      }, // ip1: captures r3
      Instr::Return { src: 4 }, // ip2
    ];
    let f = make_func(code, vec![Value::number(9.0), nested_val], 5);
    let facts = liveness(&f, &build_predecessors(&f));
    assert!(
      facts.is_live(1, 3),
      "Closure must keep its captured local live"
    );
    assert!(
      !facts.is_live(2, 3),
      "r3 is dead once the closure has captured it"
    );
  }
}

#[cfg(test)]
mod ref_classify_tests {
  use std::rc::Rc;

  use super::*;
  use crate::vm::chunk::Chunk;
  use crate::vm::object::{JitInfo, Obj};
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

  #[test]
  fn nil_and_bool_never_reference() {
    let code = vec![
      Instr::LoadNil { dst: 0 },
      Instr::LoadBool { dst: 1, val: true },
      Instr::Return { src: 0 },
    ];
    let f = make_func(code, vec![], 2);
    let types = analyze(&f, &build_predecessors(&f), None, None);
    let refs = classify_refs(&f, &types);
    assert!(refs.is_never_ref(2, 0));
    assert!(refs.is_never_ref(2, 1));
  }

  #[test]
  fn load_const_reflects_actual_constant_type() {
    let string_val: &'static Obj = Box::leak(Box::new(Obj::Str("hello".to_string())));
    let string_val = Value::obj(string_val as *const Obj);
    let code = vec![
      Instr::LoadConst {
        dst: 0,
        const_idx: 0,
      }, // numeric constant
      Instr::LoadConst {
        dst: 1,
        const_idx: 1,
      }, // string constant (a reference)
      Instr::Return { src: 0 },
    ];
    let f = make_func(code, vec![Value::number(3.0), string_val], 2);
    let types = analyze(&f, &build_predecessors(&f), None, None);
    let refs = classify_refs(&f, &types);
    assert!(refs.is_never_ref(2, 0), "numeric constant is never a ref");
    assert!(
      !refs.is_never_ref(2, 1),
      "string constant IS a ref; must not be misclassified"
    );
  }

  #[test]
  fn add_is_nonref_only_when_both_operands_proven_numeric() {
    // r0, r1 both proven numeric (constants) -> Add's result is proven
    // non-ref, since that forces the plain-number fast path.
    let code = vec![
      Instr::LoadConst {
        dst: 0,
        const_idx: 0,
      },
      Instr::LoadConst {
        dst: 1,
        const_idx: 1,
      },
      Instr::Add { dst: 2, a: 0, b: 1 },
      Instr::Return { src: 2 },
    ];
    let f = make_func(code, vec![Value::number(1.0), Value::number(2.0)], 3);
    let types = analyze(&f, &build_predecessors(&f), None, None);
    let refs = classify_refs(&f, &types);
    assert!(
      refs.is_never_ref(3, 2),
      "both operands proven numeric -> Add's result is proven non-ref"
    );
  }

  #[test]
  fn add_is_conservative_when_operand_not_proven_numeric() {
    // r0 comes from an unprovable GetGlobal; Add could hit the
    // bigint/string/list/operator-override path, so its result must
    // NOT be proven non-ref.
    let code = vec![
      Instr::GetGlobal {
        dst: 0,
        name_const: 0,
      },
      Instr::LoadConst {
        dst: 1,
        const_idx: 1,
      },
      Instr::Add { dst: 2, a: 0, b: 1 },
      Instr::Return { src: 2 },
    ];
    let f = make_func(code, vec![Value::number(0.0), Value::number(2.0)], 3);
    let types = analyze(&f, &build_predecessors(&f), None, None);
    let refs = classify_refs(&f, &types);
    assert!(
      !refs.is_never_ref(3, 2),
      "unprovable operand -> Add's result must be conservatively 'maybe a ref'"
    );
  }

  #[test]
  fn eq_always_nonref_regardless_of_operand_types() {
    // Eq calls Value::equals directly, no operator-override hook --
    // provably non-ref even though neither operand is proven numeric.
    let code = vec![
      Instr::GetGlobal {
        dst: 0,
        name_const: 0,
      },
      Instr::GetGlobal {
        dst: 1,
        name_const: 1,
      },
      Instr::Eq { dst: 2, a: 0, b: 1 },
      Instr::Return { src: 2 },
    ];
    let f = make_func(code, vec![Value::number(0.0), Value::number(0.0)], 3);
    let types = analyze(&f, &build_predecessors(&f), None, None);
    let refs = classify_refs(&f, &types);
    assert!(
      refs.is_never_ref(3, 2),
      "Eq is always a bool, no matter what its operands are"
    );
  }

  #[test]
  fn always_reference_producing_instructions() {
    let code = vec![
      Instr::MakeList {
        dst: 0,
        start: 0,
        count: 0,
      },
      Instr::Concat { dst: 1, a: 0, b: 0 },
      Instr::Return { src: 0 },
    ];
    let f = make_func(code, vec![], 2);
    let types = analyze(&f, &build_predecessors(&f), None, None);
    let refs = classify_refs(&f, &types);
    assert!(!refs.is_never_ref(2, 0), "MakeList always allocates a ref");
    assert!(!refs.is_never_ref(2, 1), "Concat always allocates a String");
  }

  #[test]
  fn move_propagates_the_fact() {
    let code = vec![
      Instr::LoadConst {
        dst: 0,
        const_idx: 0,
      },
      Instr::Move { dst: 1, src: 0 },
      Instr::Return { src: 1 },
    ];
    let f = make_func(code, vec![Value::number(4.0)], 2);
    let types = analyze(&f, &build_predecessors(&f), None, None);
    let refs = classify_refs(&f, &types);
    assert!(
      refs.is_never_ref(2, 1),
      "Move should propagate non-ref-ness"
    );
  }
}
