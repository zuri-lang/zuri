//! A purely static, compile-time-only dataflow pass proving which
//! bytecode registers are DEFINITELY numeric at each instruction --
//! not a guess, a proof: wherever it says a register is numeric, EVERY
//! possible runtime execution path reaching that point put a number
//! there. `jit::codegen` uses this to skip the `is_number()` guard (and
//! the slow-path helper fallback entirely) for arithmetic whose
//! operands are already proven, turning what would be a branchy
//! guard-and-fallback into unconditional straight-line float math.
//!
//! # Why this needs no runtime guard, no profiling, no deoptimization
//!
//! This is a "must" analysis (a fact holds only if it holds on EVERY
//! incoming path, so a merge point intersects rather than unions its
//! predecessors' facts), computed once per function at JIT-compile
//! time, purely from the bytecode's own shape -- constants, and chains
//! of arithmetic whose own operands are already proven. Nothing here
//! is ever "probably true" or "true so far" -- if the proof holds, the
//! guard is REDUNDANT by construction, so eliding it can't ever be
//! wrong. Anything not staticly provable (function parameters, a
//! `GetField`/`Call`/`GetIndex` result, ...) is just conservatively
//! left unproven, and `codegen` falls back to exactly today's guarded
//! fast/slow codegen for it -- this pass only ever REMOVES already-
//! redundant checks, never adds risk.
//!
//! # Why a `Call`/`Invoke` doesn't invalidate other registers' proofs
//!
//! This VM's calling convention gives a callee a fresh register window
//! starting at `func + 1` (or `obj + 1` for `Invoke`) in the CALLER's
//! own register file -- the callee can only ever write within its own
//! window, never back into the caller's lower-numbered registers
//! (`setup_closure_call`'s whole contract). So a call only changes the
//! TYPE of its own `dst` register; every other register's value (and
//! therefore its proven-numeric status) provably survives a call
//! unchanged, the same way it survives a GC safepoint (non-moving GC,
//! so a pointer's identity and target never change under it either).
//! This is what makes chains like `for i in .. { sum = sum + f(i) }`
//! still get SOME benefit even with a call in the loop: `i`'s own
//! proof survives the call intact, even though `sum`'s doesn't (it's
//! reassigned from the call's own result, which isn't proven).

use rustc_hash::FxHashMap;

use crate::vm::chunk::Instr;
use crate::vm::object::ObjFunction;

/// A bitset over bytecode register indices (0..=255), one word per 64
/// registers. Cheap to clone/AND/compare -- functions rarely use more
/// than a handful of words' worth of registers.
#[derive(Clone, PartialEq, Eq)]
pub struct RegSet {
  words: Vec<u64>,
}

impl RegSet {
  fn word_count(num_registers: usize) -> usize {
    num_registers.div_ceil(64).max(1)
  }

  /// Nothing proven -- the correct starting fact for a function's own
  /// entry (register 0's caller-supplied argument is never staticly
  /// known to be a number, nor is anything else, before any code has
  /// run).
  fn empty(num_registers: usize) -> Self {
    RegSet {
      words: vec![0u64; Self::word_count(num_registers)],
    }
  }

  /// Everything (optimistically) proven -- the correct starting point
  /// for every OTHER block in a forward "must" analysis with an
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
  /// returning whether anything actually changed -- what drives the
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
}

/// The result of analyzing one function: `entry[ip]` is exactly the
/// set of registers PROVEN numeric on every path reaching bytecode
/// position `ip`, i.e. before `ip`'s own instruction executes -- what
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

  /// Every register (among the first 64 -- the same bound
  /// `speculative_params` itself is already subject to) proven numeric
  /// AT bytecode position `ip`, as a bitmask. At `ip == 0` this is
  /// exactly whatever seed `analyze` was given (if any) -- unchanged
  /// from before this existed. At any OTHER `ip`, this is the SOUND
  /// thing to re-validate a runtime guard against when jumping
  /// directly into that bytecode position (e.g. on-stack replacement):
  /// exactly the claim the code at `ip` is about to rely on, not a
  /// proxy for it. Checking whether the ORIGINAL entry-time seed
  /// registers are STILL numeric at `ip` is NOT equivalent to this and
  /// is NOT sound in general -- a seed register can be reassigned
  /// (even to another number) between entry and `ip` in a way that
  /// invalidates an EARLIER computation's proof without that
  /// reassignment itself being visible in a "is register R numeric
  /// right now" check. Querying `entry[ip]` directly sidesteps that
  /// entirely: it's already the fixed point of the SAME dataflow proof
  /// used everywhere else in this file, which tracks every
  /// reassignment precisely.
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

/// Every register (among the first 64) that a WHOLE-FRAME profile
/// sample observed holding a number at the moment compilation
/// triggered -- consulted by `transfer` wherever an instruction's
/// result would otherwise be unconditionally treated as unproven
/// (`GetField`, `Call`, `Invoke`, `GetGlobal`, `GetUpval`, `GetIndex`,
/// ...: anything whose value depends on something this pass can't see
/// statically). Unlike `speculative_params` (which seeds function
/// ENTRY, `ip == 0`, since a parameter genuinely holds its real value
/// before any code runs), this has no single seed point: it changes
/// what `transfer` computes for the OUT set at whatever ip the
/// matching instruction actually executes at, and the existing
/// fixed-point worklist propagates that forward exactly like any other
/// fact -- no changes needed to the merge/iteration logic itself.
///
/// Deliberately NOT restricted to a hardcoded instruction allowlist:
/// any instruction whose result is semantically NEVER a number (a
/// `Closure`, a `List`, ...) simply never samples as numeric in the
/// first place, so the profiling itself is what keeps this
/// self-limited to instructions where speculating is actually
/// meaningful, rather than a second, separately-maintained list that
/// could drift out of sync with `transfer`'s own instruction match.
///
/// Soundness comes from the same place it always does in this JIT:
/// `codegen` never trusts this seed's claim without a real runtime
/// guard planted exactly at the instruction's own definition site
/// (checking the ACTUAL value just computed, not a proxy for it) --
/// see `jit::codegen::FuncCompiler`'s own docs on the mid-function
/// guard-and-fork this drives. A wrong guess here costs a fallback
/// jump into the general body's continuation, never a wrong answer.
pub type SpeculativeRegs = u64;

/// Runs the analysis. `speculative_params`, if given, seeds
/// register-0-based parameter slots as ALREADY proven numeric at
/// function entry instead of starting from nothing -- this is the hook
/// `jit::engine`'s profile-guided specialization (a SEPARATE, second
/// compiled copy of the function body, guarded by one runtime check at
/// entry -- see `codegen`'s own docs) uses to get the exact same
/// guard-elision benefit for a speculatively-numeric parameter as a
/// staticly-provable constant gets for free. `None` (or an empty set)
/// reproduces the fully conservative baseline (nothing assumed at
/// entry) -- always sound on its own, used for both ordinary
/// compilation and every OSR entry point.
///
/// `speculative_params` is a bitmask (bit `r` = register `r`) of
/// FIXED-arity parameter registers to seed as already-proven at
/// function entry, rather than a `RegSet` directly -- keeps `RegSet`
/// itself a purely internal representation, with only a plain integer
/// crossing this module's boundary. Bits at or past register 64 (or
/// past `proto.num_registers`) are simply never representable/used --
/// a real function needing more than 64 SPECULATED parameters doesn't
/// lose correctness, just the ability to speculate on the overflow
/// ones (`codegen`'s own entry guard is built from the same mask, so
/// the two always agree on which registers are actually being bet on).
///
/// `speculative_regs` is the SAME kind of bitmask, but for values
/// beyond function parameters -- see `SpeculativeRegs`'s own docs.
pub fn analyze(
  proto: &ObjFunction,
  speculative_params: Option<u64>,
  speculative_regs: Option<SpeculativeRegs>,
) -> TypeFacts {
  let code = &proto.chunk.code;
  let code_len = code.len();
  let num_registers = proto.num_registers as usize;

  let preds = build_predecessors(proto);

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

  let spec_regs = speculative_regs.unwrap_or(0);

  let mut worklist: Vec<usize> = (0..code_len).collect();
  let mut in_worklist = vec![true; code_len];
  // Seed every OUT from its (possibly still-`full()`, not-yet-
  // converged) IN, so the worklist loop below has a real starting
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
      // block) -- vacuously "everything proven" is safe: nothing ever
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

/// What a single bytecode instruction proves/invalidates about
/// register numeric-ness, given what was proven on entry to it.
/// `speculative_regs` overrides the "conservative, always unproven"
/// destinations below to numeric where the caller's profiling sample
/// says so -- see `SpeculativeRegs`'s own docs.
fn transfer(in_set: &RegSet, instr: &Instr, proto: &ObjFunction, speculative_regs: u64) -> RegSet {
  let mut out = in_set.clone();
  match *instr {
    Instr::LoadConst { dst, const_idx } => {
      // Constants never change after compilation, so whether the
      // loaded value is numeric is itself a static fact -- common for
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
    // -- conservatively not proven, UNLESS the caller's profiling
    // sample observed this exact destination register holding a
    // number at the moment compilation triggered (`speculative_regs`)
    // -- see `SpeculativeRegs`'s own docs. `codegen` is what actually
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

    // No destination register written at all -- facts pass through
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

    Instr::Raise { .. } | Instr::PushCatch { .. } | Instr::PopCatch => {
      unreachable!("excluded from compilation before this analysis ever runs")
    },
  }
  out
}

/// The destination register of `instr`, if it's one of `transfer`'s
/// "conservative, always unproven unless speculated" instructions --
/// exactly the SAME instruction list as that match arm above (kept as
/// a single source of truth would require restructuring `transfer`
/// itself; until then, the two must be kept in sync by hand, the same
/// way `zuri_jit_invoke_prepare`'s own doc comment already flags its
/// parameter order needing to match `emit_fast_call`'s calling
/// convention by hand). `codegen::FuncCompiler` calls this once per
/// instruction, right after emitting it in the specialized body, to
/// decide whether a mid-function guard-and-fork belongs there -- see
/// its own docs.
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

/// Every bytecode position `ip`'s instruction can transfer control to,
/// including the implicit fallthrough to `ip + 1` where applicable --
/// the forward edges the fixed-point worklist propagates facts along.
fn successors(ip: usize, instr: &Instr, proto: &ObjFunction) -> Vec<usize> {
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
/// worklist pop.
fn build_predecessors(proto: &ObjFunction) -> Vec<Vec<usize>> {
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
