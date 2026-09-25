//! Warm-up threshold formulas.
//!
//! There's deliberately no single fixed "compile after N calls"
//! constant anywhere in this module. A fixed threshold is wrong at both
//! ends of the size spectrum: a tiny 3-instruction getter compiled
//! after (say) 50 calls has paid a real, fixed compilation cost (build
//! Cranelift IR, run its optimizer, emit machine code) for a function
//! that will almost certainly never earn it back, while a huge,
//! expensive-per-call function sitting behind the same threshold keeps
//! the VM stuck in the slow interpreter tier far longer than it needs
//! to before the JIT gets a chance to help.
//!
//! Instead, the threshold scales with the function's own compiled
//! bytecode instruction count (`chunk.code.len()`), computed once (see
//! `JitInfo::new` in `vm::object`) when the function is first compiled
//! to bytecode; well before it's ever executed, let alone JIT'd.
//! Bigger functions get a lower threshold (they're doing more work per
//! call, so compiling them pays for itself sooner, and the interpreter
//! is paying dispatch overhead on more instructions per call in the
//! meantime); tiny functions get a higher one.
//!
//! The chosen curve is `threshold = K / sqrt(instruction_count)`,
//! clamped to a sane floor/ceiling. A couple of worked examples (just
//! illustrating the shape, not literal calibration targets): 100
//! instructions -> ~56 calls; 1000 instructions -> ~18 calls. A
//! 3-instruction one-liner clamps at the ceiling (compiling it at all
//! is speculative; most never get called enough to matter, so the
//! ceiling protects against wasting compilation effort on code that
//! runs a handful of times and never again); a 20,000-instruction
//! monster clamps at the floor (compile it almost immediately; even
//! one call through the interpreter's dispatch loop at that size is
//! expensive enough that the fixed cost of compiling is trivial by
//! comparison).
//!
//! `sqrt`, rather than a straight inverse-linear `K / n`, was chosen
//! deliberately: `K / n` punishes mid-sized functions (a few hundred
//! instructions; extremely common for real functions) far too
//! aggressively relative to tiny ones, while `sqrt` gives a gentler,
//! more evenly-spread curve across the whole realistic size range.
//!
//! Both constants (`K` for whole-function call warm-up, and the
//! smaller one for a single loop's own OSR back-edge warm-up) are
//! overridable via environment variables purely for benchmarking/
//! tuning during development; `ZURI_JIT_CALL_K` / `ZURI_JIT_OSR_K` --
//! read once and cached, never touched by ordinary use.

use std::sync::OnceLock;

/// Numerator for the whole-function call-count warm-up curve;
/// `threshold = CALL_K / sqrt(instruction_count)`.
fn call_k() -> f64 {
  static K: OnceLock<f64> = OnceLock::new();
  *K.get_or_init(|| {
    std::env::var("ZURI_JIT_CALL_K")
      .ok()
      .and_then(|s| s.parse().ok())
      .unwrap_or(560.0)
  })
}

/// Numerator for a single loop's own back-edge OSR warm-up curve.
/// Smaller than `call_k()` so a long-running loop inside a function
/// that's only ever called once or twice (a `main`-style entry point,
/// a one-shot batch job) still gets compiled; OSR is what makes that
/// case possible at all, since whole-function call warm-up alone would
/// never fire for it.
fn osr_k() -> f64 {
  static K: OnceLock<f64> = OnceLock::new();
  *K.get_or_init(|| {
    std::env::var("ZURI_JIT_OSR_K")
      .ok()
      .and_then(|s| s.parse().ok())
      .unwrap_or(140.0)
  })
}

const CALL_WARMUP_MIN: u32 = 8;
const CALL_WARMUP_MAX: u32 = 100_000;
const OSR_WARMUP_MIN: u32 = 4;
const OSR_WARMUP_MAX: u32 = 20_000;

fn curve(k: f64, instruction_count: usize, min: u32, max: u32) -> u32 {
  // A function with (near-)zero instructions can't meaningfully divide
  // by sqrt(n); treat it as size 1 so the formula stays well-defined
  // and just lands at the ceiling, matching the "essentially free to
  // interpret forever" intuition for a trivial body.
  let n = (instruction_count.max(1)) as f64;
  let raw = (k / n.sqrt()).round();
  if !raw.is_finite() {
    return max;
  }
  (raw as i64).clamp(min as i64, max as i64) as u32
}

/// How many real invocations this function needs before the VM
/// compiles it: see the module-level docs for the shape of the curve.
pub fn call_threshold(instruction_count: usize) -> u32 {
  curve(
    call_k(),
    instruction_count,
    CALL_WARMUP_MIN,
    CALL_WARMUP_MAX,
  )
}

/// How many times a single loop's own back-edge must run before it
/// triggers on-stack replacement into (freshly compiled, or already
/// compiled) machine code.
pub fn osr_threshold(instruction_count: usize) -> u32 {
  curve(osr_k(), instruction_count, OSR_WARMUP_MIN, OSR_WARMUP_MAX)
}

/// Numerator for the tier-up curve: how much work a function's profiling
/// tier-1 code does, counted in entries plus loop turns, before tier 2
/// compiles it from the feedback that code has gathered. Larger than the
/// warm-up curves by design: tier 2's bets are only as good as the
/// feedback behind them, and every path a stable function takes should
/// have had its turn by then.
fn tierup_k() -> f64 {
  static K: OnceLock<f64> = OnceLock::new();
  *K.get_or_init(|| {
    std::env::var("ZURI_JIT_TIERUP_K")
      .ok()
      .and_then(|s| s.parse().ok())
      .unwrap_or(20000.0)
  })
}

const TIERUP_MIN: u32 = 64;
const TIERUP_MAX: u32 = 200_000;

/// Entries plus loop turns a function's profiling tier-1 code makes
/// before it asks for tier 2.
pub fn tierup_threshold(instruction_count: usize) -> u32 {
  curve(tierup_k(), instruction_count, TIERUP_MIN, TIERUP_MAX)
}
