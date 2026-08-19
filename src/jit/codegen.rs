//! Bytecode -> Cranelift IR translation. See `crate::jit`'s module docs
//! for the overall design; the short version this file leans on
//! throughout:
//!
//! - Every VM register is memory (`VM::registers`), addressed through a
//!   pointer this function refreshes after any call that could resize
//!   it -- never a Cranelift SSA value/`Variable` of its own. This is
//!   what makes on-stack replacement, GC safepoints, and mixed-mode
//!   calls all trivial instead of needing real deoptimization
//!   machinery (see `jit::runtime`'s module docs for the full
//!   reasoning).
//! - One Cranelift `Block` per bytecode instruction index, so a
//!   backward/forward `Instr::Jmp`-family target is always just "the
//!   block at that index" -- no separate control-flow-graph
//!   reconstruction needed.
//! - Only the NaN-boxing tag bits documented as stable in `value.rs`
//!   (`QNAN`, `SIGN_BIT`, the nil/true/false tags) are ever hand-
//!   encoded here. Anything that needs to look INSIDE a heap object
//!   (`Obj`'s layout is not, and must never be treated as, stable
//!   across compiler versions) always calls back into `jit::runtime`.

use cranelift_codegen::ir::condcodes::{FloatCC, IntCC};
use cranelift_codegen::ir::{
  AbiParam, Block, InstBuilder, SigRef, StackSlot, StackSlotData, StackSlotKind, Value as IrValue,
  types,
};
use cranelift_frontend::{FunctionBuilder, Variable};
use cranelift_jit::JITModule;
use cranelift_module::{FuncId, Module};
use rustc_hash::FxHashMap;

use crate::jit::{CallTarget, CompileFacts, escape, typeflow};
use crate::vm::chunk::Instr;
use crate::vm::object::{self, ObjFunction};
use crate::vm::value::{self};
use crate::vm::vm;

/// Byte offset (from a `*mut VM`) of the cached registers pointer --
/// see `vm::VM::regs_ptr_cache`'s docs. Read directly by compiled code
/// (entry-block init and `refresh_regs`) instead of calling into Rust,
/// since this is re-fetched at essentially every helper-call site.
const REGS_PTR_CACHE_OFFSET: i32 = vm::VM_REGS_PTR_CACHE_OFFSET as i32;
/// Where `publish_ip` writes this compiled frame's current bytecode
/// position -- see `VM::jit_ip`.
const JIT_IP_OFFSET: i32 = vm::VM_JIT_IP_OFFSET as i32;
/// Byte offset of `VM::global_slots_ptr_cache` -- see that field's own
/// docs and `emit_get_global`'s use of it.
const GLOBAL_SLOTS_PTR_CACHE_OFFSET: i32 = vm::VM_GLOBAL_SLOTS_PTR_CACHE_OFFSET as i32;
/// Byte offset of `VM::method_table_generation` -- see that field's own
/// docs and `emit_self_invoke`'s use of it.
const METHOD_TABLE_GENERATION_OFFSET: i32 = vm::VM_METHOD_TABLE_GENERATION_OFFSET as i32;
/// Byte offsets (from a `*mut VM`) of `Heap::bytes_allocated`/`next_gc`
/// (major) and `young_bytes_allocated` (minor) -- lets `emit_safepoint`
/// inline both `Heap::needs_major_gc()`/`needs_minor_gc()` checks
/// (three loads + two compares -- the young threshold itself is a
/// compile-time immediate, see `Heap::YOUNG_NEXT_GC`) instead of an
/// unconditional FFI call on every loop back-edge and call site, only
/// actually calling into Rust on the rare branch where a collection
/// (of either kind) is really about to happen.
const HEAP_BYTES_ALLOCATED_OFFSET: i32 =
  (vm::VM_HEAP_OFFSET + object::HEAP_BYTES_ALLOCATED_OFFSET) as i32;
const HEAP_NEXT_GC_OFFSET: i32 = (vm::VM_HEAP_OFFSET + object::HEAP_NEXT_GC_OFFSET) as i32;
const HEAP_YOUNG_BYTES_ALLOCATED_OFFSET: i32 =
  (vm::VM_HEAP_OFFSET + object::HEAP_YOUNG_BYTES_ALLOCATED_OFFSET) as i32;

/// Compiles `proto`'s bytecode into `fb`'s function body. Returns the
/// bytecode-ip -> osr-id map (`CompiledFunction::osr_ids`) on success,
/// or a human-readable ineligibility reason on failure -- the latter is
/// ALWAYS a permanent, sticky "never try this prototype again" signal
/// (see `VM::try_compile`), never a transient error.
///
/// `speculative_params` (bit `r` = fixed-arity parameter register `r`)
/// is a ONE-SHOT type sample of the actual call that triggered this
/// compilation (see `VM::try_compile`'s own docs) -- when non-empty, a
/// SECOND, specialized copy of the whole function body is compiled
/// alongside the always-present general one, with those specific
/// parameters treated as proven-numeric from entry (see
/// `jit::typeflow`). One runtime guard at ordinary (non-OSR) entry
/// checks the bet is still good and picks a body; OSR always targets
/// the general body (see `FuncCompiler::emit_entry_dispatch`). `None`
/// or an all-zero mask compiles exactly one body, identical to before
/// this parameter existed.
pub fn compile(
  fb: &mut FunctionBuilder,
  module: &mut JITModule,
  helpers: &FxHashMap<&'static str, FuncId>,
  proto: &ObjFunction,
  own_func_id: FuncId,
  speculative_params: Option<u64>,
  speculative_regs: Option<typeflow::SpeculativeRegs>,
  facts: CompileFacts,
) -> Result<FxHashMap<usize, i32>, String> {
  // Exception-handling bytecode is never compiled -- see this crate's
  // `jit` module docs on why "bail to the interpreter" is implemented
  // as "never enter compiled code for this function at all" rather
  // than generated unwind logic.
  for instr in &proto.chunk.code {
    if matches!(
      instr,
      Instr::Raise { .. } | Instr::PushCatch { .. } | Instr::PopCatch
    ) {
      return Err("contains exception-handling bytecode (raise/catch)".to_string());
    }
  }

  let code_len = proto.chunk.code.len();
  if code_len == 0 {
    return Err("empty function body".to_string());
  }
  // A function's own register window is addressed with a `u8` offset
  // throughout the bytecode (`Instr`'s fields), and this compiler bakes
  // register indices as plain `iconst` immediates -- nothing here
  // relies on `num_registers` fitting some OTHER bound, this check just
  // documents the actual limit already implied by the bytecode format.
  if code_len > u32::MAX as usize {
    return Err("function too large to compile".to_string());
  }

  let speculative_params = speculative_params.filter(|&m| m != 0);
  let speculative_regs = speculative_regs.filter(|&m| m != 0);
  let mut fc = FuncCompiler::new(
    fb,
    module,
    helpers,
    proto,
    own_func_id,
    code_len,
    speculative_params,
    speculative_regs,
    facts,
  );
  fc.run()
}

/// A bytecode register's compile-time cache state, tracking whether its
/// Cranelift `Variable` (see `FuncCompiler::reg_vars`) can be trusted
/// as-is or needs a real memory operation before its next use:
///
/// - `Clean`: the `Variable`'s current value is known to match
///   `VM::registers` exactly (nothing has diverged either direction).
/// - `Dirty`: the `Variable` has been written (via `store_reg`) since
///   the last time it matched memory -- a genuine `store_reg`-to-memory
///   is owed before any point that needs memory to be authoritative
///   (a call, a GC safepoint, a deopt, a return).
/// - `Stale`: memory may have changed since the `Variable` was last
///   established (a call/safepoint just ran, and this register was
///   live through it) -- the `Variable`'s value must NOT be trusted;
///   the next `load_reg` for it must issue a real memory load.
///
/// `Clean` and `Dirty` are collapsed into ONE "trust the `Variable`"
/// branch in `load_reg` -- they only differ in whether `flush_live`
/// still owes a write, never in whether a READ can trust the cache.
/// Which floating-point operation an inlined body's arithmetic
/// instruction maps to -- see `FuncCompiler::inline_arith`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum InlineArith {
  FAdd,
  FSub,
  FMul,
  FDiv,
}
use InlineArith::{FAdd, FDiv, FMul, FSub};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum RegCache {
  Clean,
  Dirty,
  Stale,
}

/// A `Number` builtin the JIT emits directly instead of dispatching
/// to -- see `FuncCompiler::emit_number_intrinsic` for why every one of
/// these is an IDENTITY with what `builtins::number` computes, never an
/// approximation of it.
///
/// Three shapes, by what the method actually is:
///
/// - `Inline`: the whole method is one Cranelift instruction with
///   exactly matching IEEE-754 semantics. Compiles to one machine
///   instruction and nothing else.
/// - `InlinePredicate`/`Sign`/`Int`: still no call, but a short fixed
///   instruction sequence rather than a single opcode -- a comparison
///   producing a `Value::bool`, or a `select` chain.
/// - `Call`: no machine instruction computes it (every transcendental),
///   so this calls the SAME `f64` method `builtins::number` calls, via
///   a dedicated `jit::runtime` helper. The win here is not a faster
///   `sin` -- it is the same `sin` -- it is skipping method resolution,
///   `builtins::lookup`'s string hash and `memcmp`, the per-call
///   argument `Vec`, and `VM::call_native` around it.
///
/// Deliberately NOT here: anything that allocates (`to_string`, `chr`,
/// `bin`/`hex`/`oct`, `fraction`) or can raise (`factorial`), which
/// would need the full frame/GC-root machinery an intrinsic exists to
/// avoid; and any hand-rolled fast approximation of a transcendental,
/// which would be faster still but would make compiled and interpreted
/// code disagree on results.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum NumberIntrinsic {
  /// One Cranelift float instruction, semantics identical to the `f64`
  /// method of the same name.
  Inline(InlineOp),
  /// A comparison whose result becomes a `Value::bool`.
  InlinePredicate(PredicateOp),
  /// `n.sign()` -- `-1`/`0`/`1` with a zero's own sign preserved, and
  /// `-1` for NaN (both comparisons below are false for NaN), matching
  /// `builtins::number::sign` exactly, which is deliberately NOT
  /// `f64::signum`.
  Sign,
  /// `n.int()` -- Rust's `as i64` cast is saturating with NaN mapping
  /// to zero, which is precisely `fcvt_to_sint_sat`'s own definition.
  Int,
  /// A direct call to the named `jit::runtime` helper: `(vm, bits)` for
  /// `arity` 0, `(vm, recv_bits, arg_bits)` for `arity` 1.
  Call {
    helper: &'static str,
    arity: u8,
  },
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum InlineOp {
  Sqrt,
  Abs,
  Floor,
  Ceil,
  Trunc,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum PredicateOp {
  /// `x != x` -- true for exactly the NaNs.
  IsNan,
  /// `|x| == inf`.
  IsInf,
  /// `|x| < inf` -- false for both infinities AND for NaN, matching
  /// `f64::is_finite`.
  IsFinite,
  /// `builtins::number::to_bool`'s own rule: `n >= 0.0` (so NaN is
  /// false, exactly as Rust's `>=` gives).
  NonNegative,
}

impl NumberIntrinsic {
  /// How many arguments the call site must supply for `name` to be this
  /// intrinsic -- checked by the caller before anything else, so e.g. a
  /// stray `x.sqrt(1)` falls through to the ordinary dispatch and gets
  /// the real arity error.
  fn arity(self) -> u8 {
    match self {
      NumberIntrinsic::Call { arity, .. } => arity,
      _ => 0,
    }
  }

  fn of(name: &str) -> Option<NumberIntrinsic> {
    use InlineOp::*;
    use NumberIntrinsic::*;
    use PredicateOp::*;
    Some(match name {
      "sqrt" => Inline(Sqrt),
      "abs" => Inline(Abs),
      "floor" => Inline(Floor),
      "ceil" => Inline(Ceil),
      "trunc" => Inline(Trunc),

      "is_nan" => InlinePredicate(IsNan),
      "is_inf" => InlinePredicate(IsInf),
      "is_finite" => InlinePredicate(IsFinite),
      "to_bool" => InlinePredicate(NonNegative),

      "sign" => Sign,
      "int" => Int,

      "sin" => Call { helper: "zuri_jit_num_sin", arity: 0 },
      "cos" => Call { helper: "zuri_jit_num_cos", arity: 0 },
      "tan" => Call { helper: "zuri_jit_num_tan", arity: 0 },
      "sinh" => Call { helper: "zuri_jit_num_sinh", arity: 0 },
      "cosh" => Call { helper: "zuri_jit_num_cosh", arity: 0 },
      "tanh" => Call { helper: "zuri_jit_num_tanh", arity: 0 },
      "asin" => Call { helper: "zuri_jit_num_asin", arity: 0 },
      "acos" => Call { helper: "zuri_jit_num_acos", arity: 0 },
      "atan" => Call { helper: "zuri_jit_num_atan", arity: 0 },
      "asinh" => Call { helper: "zuri_jit_num_asinh", arity: 0 },
      "acosh" => Call { helper: "zuri_jit_num_acosh", arity: 0 },
      "atanh" => Call { helper: "zuri_jit_num_atanh", arity: 0 },
      "exp" => Call { helper: "zuri_jit_num_exp", arity: 0 },
      "expm1" => Call { helper: "zuri_jit_num_expm1", arity: 0 },
      "log" => Call { helper: "zuri_jit_num_log", arity: 0 },
      "log2" => Call { helper: "zuri_jit_num_log2", arity: 0 },
      "log10" => Call { helper: "zuri_jit_num_log10", arity: 0 },
      "log1p" => Call { helper: "zuri_jit_num_log1p", arity: 0 },
      "cbrt" => Call { helper: "zuri_jit_num_cbrt", arity: 0 },
      "round" => Call { helper: "zuri_jit_num_round", arity: 0 },

      "max" => Call { helper: "zuri_jit_num_max", arity: 1 },
      "min" => Call { helper: "zuri_jit_num_min", arity: 1 },
      "atan2" => Call { helper: "zuri_jit_num_atan2", arity: 1 },

      _ => return None,
    })
  }
}

impl InlineOp {
  fn emit(self, fb: &mut FunctionBuilder, v: IrValue) -> IrValue {
    match self {
      InlineOp::Sqrt => fb.ins().sqrt(v),
      InlineOp::Abs => fb.ins().fabs(v),
      InlineOp::Floor => fb.ins().floor(v),
      InlineOp::Ceil => fb.ins().ceil(v),
      InlineOp::Trunc => fb.ins().trunc(v),
    }
  }
}

struct FuncCompiler<'a, 'b> {
  fb: &'a mut FunctionBuilder<'b>,
  module: &'a mut JITModule,
  helpers: &'a FxHashMap<&'static str, FuncId>,
  proto: &'a ObjFunction,
  /// One block per bytecode instruction index -- `blocks[ip]` is where
  /// that instruction's own codegen begins, and the only valid jump
  /// target for anything (a `Jmp`-family instruction, or the OSR
  /// dispatch below) that wants to reach bytecode position `ip`.
  blocks: Vec<Block>,
  /// Holds the CURRENT `VM::registers` base pointer -- see this file's
  /// module docs. Refreshed (via `refresh_regs`) after every call to a
  /// `jit::runtime` helper, since any of them can transitively push a
  /// deeper call frame and reallocate `VM::registers`.
  regs_var: Variable,
  vm_param: IrValue,
  /// This frame's absolute register-window start, as the RAW INDEX
  /// (not multiplied by 8) -- what every helper call passes as `base`.
  base_param: IrValue,
  /// `base_param * 8`, computed once in the entry block (which
  /// dominates every other block, so this `Value` stays valid
  /// everywhere) -- the byte offset added to the current `regs_var`
  /// pointer to address this frame's own registers directly.
  base_bytes: IrValue,
  closure_param: IrValue,
  osr_ids: FxHashMap<usize, i32>,
  /// Cached `SigRef` for `jit::EntryFn`'s own shape
  /// (`(i64,i64,i64,i32)->i64`) -- imported at most once per compiled
  /// function (see `entry_sig_ref`), then reused by every
  /// `call_indirect` this function's own fast, inline-cache-style call
  /// sites need (`emit_fast_call`).
  entry_sig: Option<SigRef>,
  /// An 8-byte scratch stack slot, allocated at most once per compiled
  /// function and reused by every fast-call site
  /// (`closure_out_addr`/`emit_fast_call`) -- the out-parameter
  /// `zuri_jit_call_prepare`/`zuri_jit_invoke_prepare` write the
  /// resolved callee closure's `Value` bits into, since (unlike
  /// `Instr::Call`, where the callee already sits in an ordinary
  /// register) `Instr::Invoke`'s resolved METHOD closure exists only
  /// inside the helper's own class-method-table lookup, with no
  /// register holding it for generated code to read back directly.
  closure_out_slot: Option<StackSlot>,
  /// Same lazy-per-function pattern as `closure_out_slot`, for the
  /// list-index fast path's `zuri_jit_list_data` out-parameter (the
  /// resolved list's current length -- see that function's own docs).
  list_len_slot: Option<StackSlot>,
  /// Which registers are PROVEN numeric at each bytecode position, for
  /// WHICHEVER body (general or specialized) is currently being
  /// populated -- see `jit::typeflow`'s own docs and `run`'s two-pass
  /// structure. Consulted before emitting any guarded arithmetic op:
  /// when every operand is proven, the guard and its slow-path
  /// fallback are skipped entirely (they'd never be taken), leaving
  /// unconditional straight-line float math.
  type_facts: typeflow::TypeFacts,
  /// A ONE-SHOT type sample of the call that triggered this
  /// compilation (bit `r` = fixed-arity parameter register `r` held a
  /// number) -- `None` after filtering out an all-zero sample. See
  /// `compile`'s own docs and `emit_entry_dispatch`.
  speculative_params: Option<u64>,
  /// A ONE-SHOT, WHOLE-FRAME type sample taken at the same moment as
  /// `speculative_params`, but covering every register in the
  /// triggering frame rather than only the fixed-arity parameters --
  /// see `jit::typeflow::SpeculativeRegs`'s own docs. `None` after
  /// filtering out an all-zero sample.
  speculative_regs: Option<typeflow::SpeculativeRegs>,
  /// One persistent Cranelift `Variable` per bytecode register, live
  /// for the WHOLE compiled function (both the general and, if present,
  /// specialized body -- see `run`'s own docs on why sharing them
  /// across both is sound). Declared and eagerly initialized once, in
  /// `run`, right after `base_bytes` is available -- see `load_reg`/
  /// `store_reg` for how these replace the old "every register access
  /// is a real memory op" design, and `flush_live`/`mark_stale_live`
  /// for how a call/GC-safepoint/deopt/return still gets a fully
  /// memory-authoritative view exactly where one is actually needed.
  reg_vars: Vec<Variable>,
  /// Parallel to `reg_vars` -- see `RegCache`'s own docs.
  reg_cache: Vec<RegCache>,
  /// The bytecode position `emit_instruction` is CURRENTLY translating
  /// -- set once at the top of every `emit_instruction` call, read by
  /// `flush_live`/`mark_stale_live` (via `call_helper`'s own automatic
  /// wrapping) so every nested helper/`call_indirect` this one bytecode
  /// instruction's codegen might issue consults the SAME liveness
  /// query, regardless of how many separate Cranelift-level calls that
  /// translation happens to need (see `emit_fast_call`, which issues
  /// two).
  current_ip: usize,
  /// Which registers are live (per the standard `uses(ip) ∪
  /// (live_out(ip) - defs(ip))` equation -- see `typeflow::liveness`'s
  /// own docs) at every bytecode position, for THIS prototype's
  /// bytecode. Unlike `type_facts`, this doesn't depend on which body
  /// (general/specialized) is currently being populated -- it's a pure
  /// property of the bytecode's own shape -- so it's computed once and
  /// never swapped.
  liveness: typeflow::LivenessFacts,
  /// `true` for every bytecode position that's a genuine CFG join
  /// point: reachable from more than one distinct predecessor (a loop
  /// header via both its forward entry and its own back-edge; an
  /// if/else merge), OR an OSR target (an EXTRA, synthetic predecessor
  /// `typeflow::predecessor_counts` can't see, since OSR dispatch lives
  /// entirely in `emit_entry_dispatch`, outside the ordinary bytecode
  /// CFG). `emit_instruction` forces every live register `Stale` right
  /// before translating such an instruction -- see `reg_cache`'s own
  /// docs for why a single linear compile-time walk cannot otherwise
  /// know which of several predecessors' cache states is actually true
  /// at a join point, and why treating it as untrustworthy there is the
  /// sound, conservative resolution. Computed once, in `run`, after
  /// `osr_ids` is known (needs it) and before either body is populated
  /// (both need it, unchanged).
  merge_points: Vec<bool>,
  /// Field name -> slot index, for every field on `self`'s (register
  /// 0's) class that's safe to read/write directly, with no
  /// `BoundMethod`-wrapping risk -- resolved once, before compilation
  /// starts, by `VM::resolve_self_field_slots` (which has the VM access
  /// this module deliberately never touches). Empty (not `None`) for a
  /// plain function or when resolution couldn't prove anything safe --
  /// `Instr::GetField`/`SetField` on `self` just falls through to the
  /// general helper path in that case, identical to before this field
  /// existed.
  self_field_slots: FxHashMap<String, u16>,
  /// This compiled function's OWN `FuncId` in `module` -- known before
  /// codegen starts (the caller, `JitEngine::build_ir`, always declares
  /// it first). Lets `emit_call_instr`'s self-recursive case emit a
  /// genuine relocation-resolved direct `call` (via
  /// `Module::declare_func_in_func` on this SAME id) rather than an
  /// indirect call through a runtime-loaded pointer.
  own_func_id: FuncId,
  /// `(self`'s own class as `Value` bits, the `VM::
  /// method_table_generation` snapshot taken alongside it)` -- see
  /// `jit::CompileFacts::self_class_bits`'s own docs. `None` for a
  /// plain function or when `VM::resolve_self_class` couldn't prove
  /// anything.
  self_class_bits: Option<(u64, u64)>,
  /// `Instr::Call` bytecode position -> statically-resolved callee --
  /// see `jit::CallTarget`'s own docs. Consulted by `emit_call_instr`
  /// before falling back to `emit_fast_call`'s general resolver path.
  call_targets: FxHashMap<usize, CallTarget>,
  /// Bytecode register -> (its backing stack slot, element count) for
  /// every `Instr::MakeList` this compile has scalar-replaced (proven
  /// non-escaping via `jit::escape::analyze_one`, with no `Move` ever
  /// reading it -- see `scalar_replace_eligible`'s own docs for exactly
  /// what that buys). Consulted by `Instr::GetIndex`/`SetIndex` to
  /// route to `emit_scalar_list_get`/`emit_scalar_list_set` instead of
  /// the general, real-`Obj::List`-assuming fast path. A register is
  /// NEVER removed from this map once scalar-replaced (the eligibility
  /// check's "no `Move` ever reads it" requirement means nothing else
  /// could ever need to reuse this register for something unrelated
  /// that would make a STALE entry here observably wrong).
  scalar_lists: FxHashMap<u8, (StackSlot, u8)>,
  /// Per-construction-site class facts -- see
  /// `jit::CompileFacts::construct_info`.
  construct_info: FxHashMap<usize, crate::jit::ConstructInfo>,
  /// Registers holding a SCALAR-REPLACED instance: the object was
  /// never allocated, and its fields live in a Cranelift stack slot.
  /// Maps the register to that slot and to the construction site whose
  /// `construct_info` resolves field names to slot indices.
  ///
  /// Same discipline as `scalar_lists`: an entry is only ever added
  /// for a register `escape::analyze_one` already proved never leaves
  /// this function, and any ordinary write to that register removes it.
  scalar_instances: FxHashMap<u8, (StackSlot, usize)>,
  /// Can any upvalue ever be OPEN over this frame's own registers?
  ///
  /// Only `Instr::Closure` opens one (it is the sole caller of
  /// `VM::capture_upvalue`, both interpreted and via
  /// `jit::runtime::zuri_jit_make_closure`), and the index it captures
  /// is always inside the frame that executes it. So a function whose
  /// bytecode contains no `Instr::Closure` at all cannot have a single
  /// open upvalue pointing into its window, and its `Instr::Return`
  /// has nothing to close -- a callee's own upvalues live at strictly
  /// higher indices and are already closed by that callee's return.
  ///
  /// Worth proving statically rather than letting the helper discover
  /// it at runtime: the close is emitted on EVERY return, and going
  /// through `call_checked` costs an ABI call, an error-status branch,
  /// and a full `refresh_regs` reload, all to walk an empty list. The
  /// overwhelming majority of functions -- every one that never builds
  /// a closure -- pay that on every single call for nothing.
  frame_can_open_upvalues: bool,
}

impl<'a, 'b> FuncCompiler<'a, 'b> {
  fn new(
    fb: &'a mut FunctionBuilder<'b>,
    module: &'a mut JITModule,
    helpers: &'a FxHashMap<&'static str, FuncId>,
    proto: &'a ObjFunction,
    own_func_id: FuncId,
    code_len: usize,
    speculative_params: Option<u64>,
    speculative_regs: Option<typeflow::SpeculativeRegs>,
    facts: CompileFacts,
  ) -> Self {
    let blocks = (0..code_len).map(|_| fb.create_block()).collect();
    let type_facts = typeflow::analyze(proto, None, None);
    let liveness = typeflow::liveness(proto);
    FuncCompiler {
      fb,
      module,
      helpers,
      proto,
      blocks,
      regs_var: Variable::from_u32(0),
      vm_param: IrValue::from_u32(0), // placeholder, set in `run` before first use
      base_param: IrValue::from_u32(0),
      base_bytes: IrValue::from_u32(0),
      closure_param: IrValue::from_u32(0),
      osr_ids: FxHashMap::default(),
      entry_sig: None,
      closure_out_slot: None,
      list_len_slot: None,
      type_facts,
      speculative_params,
      speculative_regs,
      // Populated in `run`, once `base_bytes` is available -- empty
      // placeholders here are never actually read before that, since
      // `run` always executes before any `emit_instruction` call.
      reg_vars: Vec::new(),
      reg_cache: Vec::new(),
      current_ip: 0,
      liveness,
      merge_points: Vec::new(),
      self_field_slots: facts.self_field_slots,
      own_func_id,
      self_class_bits: facts.self_class_bits,
      call_targets: facts.call_targets,
      construct_info: facts.construct_info,
      scalar_instances: FxHashMap::default(),
      scalar_lists: FxHashMap::default(),
      frame_can_open_upvalues: proto
        .chunk
        .code
        .iter()
        .any(|i| matches!(i, Instr::Closure { .. })),
    }
  }

  #[inline]
  fn proven_numeric(&self, ip: usize, r: u8) -> bool {
    self.type_facts.is_numeric(ip, r)
  }

  #[inline]
  fn both_proven_numeric(&self, ip: usize, a: u8, b: u8) -> bool {
    self.proven_numeric(ip, a) && self.proven_numeric(ip, b)
  }

  fn run(&mut self) -> Result<FxHashMap<usize, i32>, String> {
    // Discover every loop header (the target of a BACKWARD Instr::Jmp)
    // and assign it a small dense integer id -- what `EntryFn`'s
    // `osr_id` parameter selects among.
    for (ip, instr) in self.proto.chunk.code.iter().enumerate() {
      if let Instr::Jmp { offset } = instr
        && *offset < 0
      {
        let target = (ip as isize + 1 + *offset as isize) as usize;
        if target >= self.proto.chunk.code.len() {
          return Err("malformed jump target".to_string());
        }
        if !self.osr_ids.contains_key(&target) {
          let id = self.osr_ids.len() as i32;
          self.osr_ids.insert(target, id);
        }
      }
    }

    // See `merge_points`'s own docs. Needs `osr_ids` (just computed
    // above) as well as the ordinary bytecode CFG's own predecessor
    // counts.
    let pred_counts = typeflow::predecessor_counts(self.proto);
    self.merge_points = (0..pred_counts.len())
      .map(|ip| pred_counts[ip] > 1 || self.osr_ids.contains_key(&ip))
      .collect();

    let entry_block = self.fb.create_block();
    self.fb.append_block_params_for_function_params(entry_block);
    self.fb.switch_to_block(entry_block);
    let params = self.fb.block_params(entry_block).to_vec();
    self.vm_param = params[0];
    self.base_param = params[1];
    self.closure_param = params[2];
    let osr_param = params[3];

    self.regs_var = self.fb.declare_var(types::I64);
    let initial_regs = self.load_regs_ptr_cache();
    self.fb.def_var(self.regs_var, initial_regs);

    let eight = self.fb.ins().iconst(types::I64, 8);
    self.base_bytes = self.fb.ins().imul(self.base_param, eight);

    // One `Variable` per bytecode register, declared here (entry_block
    // dominates every other block in the function, general body AND
    // specialized body alike, so a `Variable` declared/initialized here
    // is valid to `use_var` anywhere downstream) and eagerly loaded
    // from whatever's ALREADY in `VM::registers` right now -- correct
    // regardless of whether a given register is a real parameter
    // (holds the caller's actual argument) or a not-yet-defined local
    // (holds leftover bits from a previous frame that occupied this
    // slot -- never observed by well-formed bytecode, since the
    // compiler only ever emits a read of a register that's already
    // been written on every path reaching it), and regardless of
    // whether this is an ordinary call OR an OSR jump into the middle
    // of an already-executing loop (the interpreter has been running
    // up to this exact point, so `VM::registers` already holds the
    // real, current values `load_reg`'s `Stale` branch below picks up
    // fresh -- see its own docs). `load_reg` initializes each one via
    // its own existing `Stale` handling, so there's no separate
    // "eager load" code path to keep in sync with it.
    let num_regs = self.proto.num_registers as usize;
    self.reg_vars = (0..num_regs)
      .map(|_| self.fb.declare_var(types::I64))
      .collect();
    self.reg_cache = vec![RegCache::Stale; num_regs];
    for r in 0..num_regs {
      self.load_reg(r as u8);
    }

    // If speculating on EITHER function parameters or a mid-function
    // value (`speculative_regs` -- see `jit::typeflow::SpeculativeRegs`),
    // allocate a SECOND set of blocks now (before the entry dispatch,
    // which needs `specialized_blocks[0]` as a jump target) -- populated
    // in a second pass below, after the general body. Creating a block
    // doesn't require switching into it, so this doesn't disturb
    // `entry_block`'s own not-yet-terminated state. Its type-facts are
    // computed HERE too (not lazily during the second pass, as before)
    // -- `emit_entry_dispatch` needs them NOW to build a SOUND per-OSR-
    // target guard (see its own docs).
    let specialized: Option<(Vec<Block>, typeflow::TypeFacts)> =
      if self.speculative_params.is_some() || self.speculative_regs.is_some() {
        let blocks = (0..self.blocks.len())
          .map(|_| self.fb.create_block())
          .collect();
        let facts = typeflow::analyze(self.proto, self.speculative_params, self.speculative_regs);
        Some((blocks, facts))
      } else {
        None
      };

    self.emit_entry_dispatch(
      osr_param,
      specialized.as_ref().map(|(b, f)| (b.as_slice(), f)),
    );

    // Pass 1: the general body, exactly as before this function ever
    // had a `speculative_params` concept -- `type_facts` was already
    // computed conservatively (seeded with nothing) in `new`.
    for ip in 0..self.blocks.len() {
      self.fb.switch_to_block(self.blocks[ip]);
      let instr = self.proto.chunk.code[ip];
      let terminated = self.emit_instruction(ip, instr);
      if !terminated {
        let next_ip = ip + 1;
        let next = self.blocks.get(next_ip).copied().unwrap_or(self.blocks[ip]);
        if next_ip < self.blocks.len() {
          self.flush_before_jump(next_ip);
        }
        self.fb.ins().jump(next, &[]);
      }
    }

    // Pass 2: the specialized body, if any -- same instruction-
    // emission logic, just re-run against a SEPARATE block array and
    // the type-facts computed above. `emit_instruction`/every
    // `emit_*` helper only ever reference `self.blocks`/
    // `self.type_facts` generically, so swapping both fields and re-
    // running the identical loop is sufficient -- no separate codegen
    // path needed.
    if let Some((spec_blocks, spec_facts)) = specialized {
      self.type_facts = spec_facts;
      // `specialized_blocks[0]` (and every OSR route into the
      // specialized body) is reached directly from `entry_block`'s own
      // dispatch guards, NEVER from any block pass 1 just finished
      // populating -- pass 1's blocks are a sibling subgraph, not an
      // ancestor, so Cranelift's own SSA dominance already resolves a
      // `use_var` at the start of the specialized body straight back
      // to entry_block's eager initialization above, regardless of
      // whatever pass 1 did to `reg_cache`. Resetting to `Clean` here
      // makes this compiler's OWN bookkeeping match that same fact
      // (matching memory, not stale) -- see `RegCache`'s own docs.
      self.reg_cache = vec![RegCache::Clean; self.reg_vars.len()];
      let general_blocks = std::mem::replace(&mut self.blocks, spec_blocks);
      let code_len = self.blocks.len();
      for ip in 0..code_len {
        self.fb.switch_to_block(self.blocks[ip]);
        let instr = self.proto.chunk.code[ip];
        let terminated = self.emit_instruction(ip, instr);
        if terminated {
          continue;
        }
        let spec_next = self.blocks.get(ip + 1).copied().unwrap_or(self.blocks[ip]);
        // Mid-function speculation: if this instruction's destination
        // is one `speculative_regs` bet on AND the dataflow proof
        // confirms that bet is still live heading into `ip + 1` (not
        // immediately merged away by some other, unrelated predecessor
        // edge into `ip + 1` -- see `emit_speculative_guard`'s own
        // docs), plant a real runtime guard here instead of an
        // unconditional jump: re-validate the ACTUAL value this
        // instruction just computed, continue in the specialized body
        // on a match, or deoptimize to the interpreter at `ip + 1` on
        // a mismatch (see `emit_deopt`) -- exactly the entry-guard
        // pattern already used for parameters/OSR, just triggered at
        // an ordinary mid-function definition site instead of an
        // external entry point, and bailing out to the interpreter
        // instead of a compiled fallback body.
        if !self.emit_speculative_guard(ip, instr, spec_next) {
          if ip + 1 < code_len {
            self.flush_before_jump(ip + 1);
          }
          self.fb.ins().jump(spec_next, &[]);
        }
      }
      // `osr_ids` (returned to the caller) indexes into the GENERAL
      // block set by construction (computed before either pass ran,
      // from `self.blocks` as it was BEFORE this swap) -- restore it
      // so nothing downstream of `run` needs to know a swap ever
      // happened.
      self.blocks = general_blocks;
    }

    // Every block's predecessors are now fully known (every `jump`/
    // `brif`/`call_indirect`-adjacent branch this function will ever
    // emit, including every loop's own back-edge, has already been
    // added above) -- only NOW is it sound to seal them all at once.
    // This matters specifically because of `reg_vars`: unlike
    // `regs_var` (always freshly `def_var`'d within the SAME block as
    // any `use_var` of it, so it never actually depended on cross-
    // block SSA resolution), a bytecode register's `Variable` is
    // genuinely defined in one block and read in another -- including
    // across a loop back-edge, where the LATER-compiled block (the
    // back-edge's own source) adds a NEW predecessor edge to a block
    // (the loop header) that was already fully populated earlier in
    // this same pass. Before this edge is added, Cranelift's SSA
    // construction cannot know the loop header has more than one
    // predecessor, and sealing it too early would permanently bake in
    // an incomplete/incorrect resolution for any `use_var` inside the
    // loop that (transitively) depends on the value carried across
    // that back-edge.
    self.fb.seal_all_blocks();

    Ok(std::mem::take(&mut self.osr_ids))
  }

  /// `osr_param == -1` -> ordinary entry; `osr_param == id` -> jump
  /// straight into `blocks[ip]` for whichever `ip` that `id` was
  /// assigned to in `run`. A linear compare chain (not a `br_table`)
  /// -- the number of loop headers in one function is always small,
  /// and this avoids depending on `JumpTableData`'s exact API for
  /// what's a cold, one-time-per-call dispatch anyway.
  ///
  /// If `specialized` is `Some((blocks, facts))`, EVERY entry point
  /// (ordinary AND each OSR target) gets its own guard picking between
  /// the specialized and general body, built from `facts.numeric_mask_at`
  /// AT THAT SPECIFIC bytecode position -- not from a single fixed
  /// mask re-checked everywhere. This is the sound way to validate an
  /// OSR jump straight into the MIDDLE of the specialized body: at
  /// `ip == 0` `numeric_mask_at` is exactly the original speculated
  /// parameter mask (so ordinary entry is unaffected by this
  /// generalization), but at any OTHER `ip` it's whatever the SAME
  /// dataflow proof actually established is live and provably numeric
  /// AT THAT POINT -- precisely the claim the code there is about to
  /// rely on, re-validated against real, current register values. See
  /// `typeflow::TypeFacts::numeric_mask_at`'s own docs for why
  /// re-checking the ORIGINAL entry mask at a later `ip` instead would
  /// NOT be sound (a speculated register can be reassigned between
  /// entry and that point in a way a same-register recheck can't see).
  /// An OSR target where NOTHING is provably numeric (the mask is
  /// empty -- the loop never touches the speculated value at all)
  /// skips the guard and routes straight to the general body: the
  /// specialized block there would be behaviorally identical anyway.
  fn emit_entry_dispatch(
    &mut self,
    osr_param: IrValue,
    specialized: Option<(&[Block], &typeflow::TypeFacts)>,
  ) {
    let neg1 = self.fb.ins().iconst(types::I32, -1);
    let is_normal = self.fb.ins().icmp(IntCC::Equal, osr_param, neg1);

    let normal_route = specialized.map(|_| self.fb.create_block());
    let normal_target = normal_route.unwrap_or(self.blocks[0]);

    let mut next_check = self.fb.create_block();
    self
      .fb
      .ins()
      .brif(is_normal, normal_target, &[], next_check, &[]);

    let mut targets: Vec<(i32, usize)> = self.osr_ids.iter().map(|(&ip, &id)| (id, ip)).collect();
    targets.sort_by_key(|&(id, _)| id);

    // (route_block, target_ip) pairs to populate AFTER the compare
    // chain is fully laid out -- keeps every `switch_to_block` call
    // for the chain itself contiguous, matching the original
    // structure, with route-block bodies filled in afterward.
    let mut routes: Vec<(Block, usize)> = Vec::new();
    if let Some(route) = normal_route {
      routes.push((route, 0));
    }

    for (id, ip) in targets {
      self.fb.switch_to_block(next_check);
      let id_const = self.fb.ins().iconst(types::I32, id as i64);
      let is_this = self.fb.ins().icmp(IntCC::Equal, osr_param, id_const);
      let after = self.fb.create_block();
      if specialized.is_some() {
        let route = self.fb.create_block();
        self.fb.ins().brif(is_this, route, &[], after, &[]);
        routes.push((route, ip));
      } else {
        self
          .fb
          .ins()
          .brif(is_this, self.blocks[ip], &[], after, &[]);
      }
      next_check = after;
    }

    // Defensive fallback for an `osr_id` that matches none of the
    // known loop headers -- unreachable in practice (`VM::maybe_osr`
    // only ever passes an id it read out of THIS SAME function's own
    // `osr_ids` map), but falling through to the ordinary entry (`ip
    // 0`, general OR specialized per that route's own guard) is a
    // safe, well-defined default rather than leaving the block
    // unterminated.
    self.fb.switch_to_block(next_check);
    self.fb.ins().jump(normal_target, &[]);

    if let Some((spec_blocks, spec_facts)) = specialized {
      for (route_block, ip) in routes {
        self.fb.switch_to_block(route_block);
        let mask = spec_facts.numeric_mask_at(ip);
        if mask == 0 {
          if self.speculative_regs.is_some() {
            // Nothing is proven AT this exact entry point, but the
            // specialized body may still contain its own, INDEPENDENT
            // mid-function speculative guards further along (see
            // `emit_speculative_guard`) -- route into it unconditionally
            // rather than skipping straight to general, so ordinary
            // (non-OSR) execution still reaches them. When only
            // parameter speculation is in play (`speculative_regs` is
            // `None`), `spec_blocks[ip]` onward is behaviorally
            // identical to `self.blocks[ip]` in this case -- exactly
            // the reasoning that already justified the unconditional
            // general jump below, still applies whenever there's no
            // OTHER kind of speculation that could benefit downstream.
            self.fb.ins().jump(spec_blocks[ip], &[]);
          } else {
            self.fb.ins().jump(self.blocks[ip], &[]);
          }
          continue;
        }
        let mut guard: Option<IrValue> = None;
        for bit in 0..64u8 {
          if mask & (1u64 << bit) != 0 {
            let v = self.load_reg(bit);
            let is_num = self.is_number(v);
            guard = Some(match guard {
              None => is_num,
              Some(g) => self.fb.ins().band(g, is_num),
            });
          }
        }
        let guard = guard.expect("mask != 0 always sets at least one bit");
        self
          .fb
          .ins()
          .brif(guard, spec_blocks[ip], &[], self.blocks[ip], &[]);
      }
    }
  }

  /// Mid-function counterpart to `emit_entry_dispatch`'s guards: called
  /// right after translating `instr` at bytecode position `ip`, while
  /// populating the SPECIALIZED body (never during the general pass --
  /// callers only reach this from `run`'s pass-2 loop). If `instr`'s
  /// destination register is one the profiling sample bet on
  /// (`speculative_regs`) AND the dataflow proof confirms that bet is
  /// still live heading into `ip + 1` (`self.type_facts`, computed with
  /// `speculative_regs` folded in -- see `jit::typeflow::SpeculativeRegs`),
  /// re-validates the ACTUAL value `instr` just computed and either
  /// continues into `spec_next` (the specialized body's own block for
  /// `ip + 1`) on a match, or deoptimizes to the interpreter at `ip + 1`
  /// (see `emit_deopt`) on a mismatch. Returns `true` iff it terminated
  /// the current block this way -- the caller emits its own
  /// unconditional jump to `spec_next` when this returns `false`
  /// (nothing to guard here).
  ///
  /// Used to cross-jump into the general body's own block for this same
  /// `ip + 1` instead, back when this guard predates real
  /// deoptimization -- that was ALSO sound (neither body ever carries
  /// state across an instruction boundary as a Cranelift SSA value, so
  /// resuming general-body translation needed no reconciliation
  /// either), but strictly less general: it only ever worked because a
  /// compiled fallback happened to already exist. Deopting to the
  /// interpreter needs no fallback body to exist at all.
  fn emit_speculative_guard(&mut self, ip: usize, instr: Instr, spec_next: Block) -> bool {
    let Some(dst) = typeflow::conservative_dst(&instr) else {
      return false;
    };
    if ip + 1 >= self.proto.chunk.code.len() || !self.type_facts.is_numeric(ip + 1, dst) {
      return false;
    }
    let v = self.load_reg(dst);
    let is_num = self.is_number(v);
    let deopt_block = self.fb.create_block();
    // `emit_deopt` below flushes the deopt edge itself; the fast edge
    // to `spec_next` (a possible merge point, e.g. a loop header) needs
    // the same predecessor-side flush every other forward branch into
    // a merge point gets -- see `flush_before_jump`'s own docs.
    self.flush_before_jump(ip + 1);
    self.fb.ins().brif(is_num, spec_next, &[], deopt_block, &[]);
    self.fb.switch_to_block(deopt_block);
    self.emit_deopt(ip + 1);
    true
  }

  /// Real deoptimization: flushes every register live at the RESUME
  /// point (`ip` -- NOT `self.current_ip`, which is wherever the guard
  /// that triggered this deopt happens to sit; `ip` is where the
  /// interpreter picks up from, e.g. `emit_speculative_guard` deopts to
  /// `ip + 1`) to real `VM::registers` memory, calls `zuri_jit_deopt` to
  /// record `ip` itself, then immediately returns from the WHOLE
  /// compiled function -- never falls through to more translated
  /// instructions afterward, so there's no need to mark anything
  /// `Stale` afterward the way `call_helper` does. The returned value is
  /// never observed (`VM::invoke_compiled` checks `pending_deopt_ip`
  /// before it would ever look at the real return bits), so the junk
  /// `0` here costs nothing.
  ///
  /// Goes through `call_helper_raw`, NOT the auto-flushing
  /// `call_helper` -- that wrapper flushes against `self.current_ip`,
  /// which is the WRONG bytecode position for a deopt (the guard site,
  /// not the resume site); flushing here is explicit, against the
  /// correct `ip`, instead.
  ///
  /// Leaves `reg_cache` EXACTLY as it found it. This block always ends
  /// in a `return_`, so nothing downstream is a successor of it -- yet
  /// `flush_live` above is a real mutation of this compiler's own
  /// compile-time bookkeeping, marking every register it wrote `Clean`
  /// ("memory already agrees"). Translation continues afterwards on the
  /// SIBLING edge -- the guard's fall-through, which never executed any
  /// of those stores -- so letting that `Clean` escape tells the rest of
  /// the function memory holds values it does not. The register then
  /// gets skipped by a later `flush_live` (its write is silently
  /// dropped) or, worse, downgraded to `Stale` by a later
  /// `mark_stale_live` and RE-READ from memory that was never written,
  /// resurrecting a value several instructions stale.
  ///
  /// This is the same bug class `restore_dirty_from_snapshot` closes
  /// for the guarded-arithmetic fast/slow split -- a branch that may
  /// never run at runtime mutating state shared with one that does --
  /// and it reproduced concretely: with a `GetField` inline cache
  /// removing the redundant helper-call flush that used to mask it,
  /// `bodies[j].x` inside a speculatively-compiled loop re-read its
  /// receiver register from memory and got the value from BEFORE the
  /// enclosing `GetIndex`, raising a `TypeError` naming the list itself
  /// as the receiver. Handled here rather than at the call site so it
  /// stays closed for any future guard that deopts from inside one arm
  /// of a branch.
  fn emit_deopt(&mut self, ip: usize) {
    let snapshot = self.snapshot_reg_cache();
    self.flush_live(ip);
    let vm = self.vm_param;
    let ip_c = self.u64c(ip as u64);
    self.call_helper_raw("zuri_jit_deopt", &[vm, ip_c]);
    let junk = self.i64c(0);
    self.fb.ins().return_(&[junk]);
    self.reg_cache = snapshot;
  }

  // ---------------------------------------------------------------
  // Register / constant access
  // ---------------------------------------------------------------

  fn reg_addr(&mut self, r: u8) -> IrValue {
    let regs = self.fb.use_var(self.regs_var);
    let with_base = self.fb.ins().iadd(regs, self.base_bytes);
    if r == 0 {
      with_base
    } else {
      self.fb.ins().iadd_imm_s(with_base, (r as i64) * 8)
    }
  }

  /// The REAL memory read this used to be the whole implementation of
  /// -- now used only by `load_reg`'s own `Stale` branch (a genuine
  /// reload is owed) and nowhere else. See this module's docs / the JIT
  /// SSA plan for why every OTHER register access goes through the
  /// `Variable`-backed `load_reg`/`store_reg` instead.
  fn load_reg_mem(&mut self, r: u8) -> IrValue {
    let addr = self.reg_addr(r);
    self.fb.ins().load(
      types::I64,
      cranelift_codegen::ir::MemFlagsData::trusted(),
      addr,
      0,
    )
  }

  /// The REAL memory write this used to be the whole implementation of
  /// -- now used only by `flush_live`, which owes memory a write for
  /// every live `Dirty` register right before a genuine sync point (a
  /// call, a GC safepoint, a deopt, a return).
  fn store_reg_mem(&mut self, r: u8, v: IrValue) {
    let addr = self.reg_addr(r);
    self
      .fb
      .ins()
      .store(cranelift_codegen::ir::MemFlagsData::trusted(), v, addr, 0);
  }

  /// Reads bytecode register `r`'s CURRENT value -- a plain `use_var`
  /// against its persistent `Variable` (see `reg_vars`) the overwhelming
  /// majority of the time, letting Cranelift's own optimizer/register
  /// allocator treat it as a real SSA value with no memory traffic at
  /// all. Only actually touches memory when `reg_cache[r]` says
  /// `Stale` -- a call/GC-safepoint/deopt this register was live
  /// through just ran, so the `Variable`'s last-known value can no
  /// longer be trusted (a moving collection could have relocated a
  /// pointer stored here, for instance) and a fresh read is owed before
  /// this register is used again. See `RegCache`'s own docs for the
  /// full state machine.
  fn load_reg(&mut self, r: u8) -> IrValue {
    match self.reg_cache[r as usize] {
      RegCache::Clean | RegCache::Dirty => self.fb.use_var(self.reg_vars[r as usize]),
      RegCache::Stale => {
        let v = self.load_reg_mem(r);
        self.fb.def_var(self.reg_vars[r as usize], v);
        self.reg_cache[r as usize] = RegCache::Clean;
        v
      },
    }
  }

  /// Writes bytecode register `r`'s CURRENT value -- a plain `def_var`
  /// against its persistent `Variable`, no memory traffic. Marks the
  /// register `Dirty`: memory doesn't reflect this write yet, and won't
  /// until `flush_live` runs (right before the next genuine sync
  /// point this register happens to still be live at).
  fn store_reg(&mut self, r: u8, v: IrValue) {
    self.fb.def_var(self.reg_vars[r as usize], v);
    self.reg_cache[r as usize] = RegCache::Dirty;
    // A register the bytecode compiler reused for a DIFFERENT, later
    // local variable is no longer the scalar-replaced allocation
    // `scalar_lists` might still be tracking it as -- see that field's
    // own docs. `scalar_replace_eligible`'s "no `Move` ever reads it"
    // requirement means nothing SAFE could have relied on this entry
    // surviving past its own last real use anyway, so removing it
    // unconditionally on any ordinary write is always correct, not
    // just defensive.
    self.scalar_lists.remove(&r);
    // Same reasoning: this register is being redefined, so any
    // scalar-replaced instance it used to name is no longer there.
    self.scalar_instances.remove(&r);
  }

  /// Writes real `VM::registers` memory for every register that's both
  /// (a) live at bytecode position `ip` and (b) currently `Dirty`.
  /// Called automatically by `call_helper` (and explicitly wherever a
  /// sync point doesn't go through it -- see `emit_deopt`, and the
  /// `call_indirect` fast-call site in `emit_fast_call`) immediately
  /// BEFORE the actual call/branch, so whatever Rust/native code, the
  /// interpreter, or a GC root scan is about to run sees a fully
  /// correct, current view of every register it could touch.
  ///
  /// NOTE: an "always flush every register, ignore liveness entirely"
  /// variant of this function was tried and measured against the real
  /// bug this is chasing (see git history) -- it did NOT fix it, which
  /// is itself real evidence: the bug is NOT about which registers get
  /// selected for flushing, so this stays liveness-scoped rather than
  /// paying an unnecessary, unjustified performance cost for a fix that
  /// doesn't fix anything.
  fn flush_live(&mut self, ip: usize) {
    let live: Vec<u8> = self.liveness.live_regs_at(ip).collect();
    for r in live {
      if self.reg_cache[r as usize] == RegCache::Dirty {
        let v = self.fb.use_var(self.reg_vars[r as usize]);
        self.store_reg_mem(r, v);
        self.reg_cache[r as usize] = RegCache::Clean;
      }
    }
  }

  /// Marks every register live at bytecode position `ip` `Stale` --
  /// called automatically by `call_helper` immediately AFTER the actual
  /// call/branch, so the NEXT `load_reg` for any of them is forced to
  /// re-read memory rather than trust a `Variable` value that may have
  /// been invalidated by whatever just ran (a GC safepoint relocating a
  /// pointer, a callee/native function, the interpreter during a
  /// deopt-and-resume). See `flush_live`'s own docs on why this stays
  /// liveness-scoped.
  fn mark_stale_live(&mut self, ip: usize) {
    for r in self.liveness.live_regs_at(ip) {
      self.reg_cache[r as usize] = RegCache::Stale;
    }
  }

  /// Flushes, in the CURRENT block, every register that's both Dirty
  /// and live-into `target_ip` -- called right before emitting a
  /// FORWARD branch/fallthrough into a genuine merge point (`Instr::
  /// Jmp`'s forward case, `JmpIfFalse`/`JmpIfTrue`'s targets, and the
  /// automatic fallthrough `run`'s own driver loop appends for a non-
  /// terminated instruction). This is the ONLY place a merge point's
  /// "the fall-through/forward edge might carry an unflushed Dirty
  /// value" case gets handled -- deliberately NOT inside the merge
  /// block itself (see `emit_instruction`'s own docs on why that's
  /// actively unsound for any merge point that's also a loop header:
  /// `use_var` there would resolve, on the BACK edge specifically, to
  /// whatever Cranelift's own SSA construction is holding in a machine
  /// register/spill slot for that `Variable` -- a location GC has zero
  /// visibility into and cannot fix up -- and writing THAT back over
  /// memory can clobber a relocation `emit_safepoint`'s own call just
  /// performed moments earlier, using the CORRECT, already-flushed
  /// value). The back edge needs no separate handling here: it always
  /// runs through `emit_safepoint` first, which flushes (and marks
  /// stale) against ITS OWN block's `current_ip` before ever branching,
  /// so by the time control reaches the merge point via that edge,
  /// memory is already correct and this function has nothing to do for
  /// it (`flush_live` only touches registers still `Dirty`).
  fn flush_before_jump(&mut self, target_ip: usize) {
    if self.merge_points[target_ip] {
      self.flush_live(target_ip);
    }
  }

  /// Snapshots `reg_cache` before a guarded instruction's own internal
  /// fast/slow branch split (see `restore_dirty_from_snapshot`'s own
  /// docs for why this pairing exists and what bug it closes).
  fn snapshot_reg_cache(&self) -> Vec<RegCache> {
    self.reg_cache.clone()
  }

  /// Undoes a specific, confirmed-real bug class: the slow path of a
  /// guarded instruction (`emit_binary_numeric_guarded` and its
  /// siblings) calls its helper through `call_checked`/`call_helper`,
  /// which -- entirely correctly FOR THAT CALL SITE -- flushes and
  /// stale-marks every register live at `self.current_ip` (this WHOLE
  /// instruction's own bytecode position), not just `a`/`b`/`dst`. That
  /// breadth is fine for a call that ALWAYS executes; it's wrong here,
  /// because the slow path is one arm of a runtime branch the fast path
  /// (which never flushes anything) might take instead. Cranelift
  /// compiles BOTH arms unconditionally, so this compiler's OWN
  /// `reg_cache` bookkeeping -- despite being purely compile-time state
  /// -- got mutated by code that may never execute at runtime, for
  /// registers this instruction has no business touching at all (e.g.
  /// a completely unrelated call argument two instructions away that
  /// merely happened to share a live range with this one). Confirmed by
  /// direct reproduction (`tmp/osr_speculation_stress.zu`, a recursive
  /// call whose OWN argument register got silently marked flushed by a
  /// sibling `depth - 1` guarded-arithmetic slow path that never ran,
  /// so the real flush at the call site was skipped and the callee read
  /// nil) and by per-register write-forcing bisection pinpointing that
  /// exact register before this fix existed.
  ///
  /// The fix: after both arms rejoin at `done_block`, restore every
  /// register OTHER than this instruction's own `dst` (already handled
  /// correctly and independently by `resync_dst_from_memory`/explicit
  /// `Stale`-marking) back to `Dirty` if it was `Dirty` in the snapshot
  /// and is anything else now. This is always safe to do even when the
  /// slow path DID run at runtime: `Dirty` only means "the next
  /// sync point must flush this before trusting memory," never "this
  /// value is wrong" -- `reg_vars[r]` itself was never touched by
  /// either arm, so `use_var` still returns the correct value either
  /// way, just possibly triggering one harmless redundant future flush.
  fn restore_dirty_from_snapshot(&mut self, before: &[RegCache], dst: u8) {
    self.restore_dirty_from_snapshot_except(before, Some(dst));
  }

  /// `restore_dirty_from_snapshot` for a guarded instruction that
  /// defines NO register at all (`emit_ic_set_field`), so there is
  /// nothing to hold back from the restore.
  fn restore_dirty_from_snapshot_all(&mut self, before: &[RegCache]) {
    self.restore_dirty_from_snapshot_except(before, None);
  }

  fn restore_dirty_from_snapshot_except(&mut self, before: &[RegCache], skip: Option<u8>) {
    for (r, &prev) in before.iter().enumerate() {
      if skip == Some(r as u8) {
        continue;
      }
      if prev == RegCache::Dirty && self.reg_cache[r] != RegCache::Dirty {
        self.reg_cache[r] = RegCache::Dirty;
      }
    }
  }

  /// Direct load of `VM::regs_ptr_cache` at its compile-time-baked
  /// offset -- no FFI call. Sound as long as every reallocation of
  /// `VM::registers` keeps that cache in sync, which is `VM`'s own
  /// invariant (see `VM::sync_regs_ptr_cache`), not something this
  /// compiler needs to re-establish.
  fn load_regs_ptr_cache(&mut self) -> IrValue {
    self.fb.ins().load(
      types::I64,
      cranelift_codegen::ir::MemFlagsData::trusted(),
      self.vm_param,
      REGS_PTR_CACHE_OFFSET,
    )
  }

  fn refresh_regs(&mut self) {
    let fresh = self.load_regs_ptr_cache();
    self.fb.def_var(self.regs_var, fresh);
  }

  fn idx(&mut self, i: u8) -> IrValue {
    self.fb.ins().iconst(types::I64, i as i64)
  }

  fn i64c(&mut self, v: i64) -> IrValue {
    self.fb.ins().iconst(types::I64, v)
  }

  fn u64c(&mut self, v: u64) -> IrValue {
    self.fb.ins().iconst(types::I64, v as i64)
  }

  /// Bakes `proto.chunk.constants[idx]`'s raw `Value` bits as an
  /// immediate -- see this module's docs on why this never needs a
  /// runtime `chunk.constants[idx]` load. Sound specifically because
  /// `Compiler` allocates every object-typed constant (strings,
  /// bigints, nested function prototypes) via `Heap::alloc_old`/
  /// `alloc_function`, NEVER the young-generation nursery a minor
  /// collection can relocate out from under an already-baked
  /// immediate with no way to fix it back up -- see those functions'
  /// own docs. A plain number/nil/bool constant has no address to go
  /// stale in the first place.
  fn bake_const(&mut self, idx: u16) -> IrValue {
    let v = self.proto.chunk.constants[idx as usize];
    self.u64c(v.to_bits())
  }

  fn bake_f64_bits(&mut self, idx: u16) -> IrValue {
    let v = self.proto.chunk.constants[idx as usize];
    debug_assert!(v.is_number());
    self.u64c(v.as_number().to_bits())
  }

  /// A stable pointer to the CURRENTLY COMPILING `ObjFunction` itself,
  /// baked as an immediate -- sound because `Heap::alloc_function`
  /// always allocates directly into old-generation storage (see its
  /// own docs on why a moving young generation makes that necessary),
  /// so `proto`'s address is fixed for its whole life, and `proto`
  /// itself outlives this compiled function (it owns the very
  /// bytecode this IS the compilation of).
  fn func_ptr_const(&mut self) -> IrValue {
    self.u64c(self.proto as *const ObjFunction as u64)
  }

  // ---------------------------------------------------------------
  // Helper calls
  // ---------------------------------------------------------------

  /// The REAL call instruction -- no register-cache bookkeeping at all.
  /// Used directly ONLY by `call_helper` (below) and `emit_deopt`
  /// (which needs to flush against the DEOPT TARGET ip, not
  /// `self.current_ip`, so it can't go through the automatic wrapper --
  /// see its own docs). Every other call site in this file goes through
  /// `call_helper` instead.
  fn call_helper_raw(&mut self, name: &str, args: &[IrValue]) -> IrValue {
    let func_id = *self
      .helpers
      .get(name)
      .unwrap_or_else(|| panic!("zuri: unregistered JIT helper '{name}'"));
    let func_ref = self.module.declare_func_in_func(func_id, self.fb.func);
    let call = self.fb.ins().call(func_ref, args);
    self.fb.inst_results(call)[0]
  }

  /// `call_helper_raw`, automatically bracketed with `flush_live`/
  /// `mark_stale_live` against `self.current_ip` -- the register-cache
  /// half of "every helper call is conservatively treated as a genuine
  /// sync point" (the OTHER half, `refresh_regs`'s pointer refresh, is
  /// unchanged and still each call site's own responsibility, exactly
  /// as before this cache existed). Centralizing this here, rather than
  /// in each of `call_checked`/`emit_fast_call`/`emit_is_falsey`/
  /// `emit_safepoint`/`UsingJump`'s own codegen, means every one of
  /// this file's ~40 call sites gets correct treatment automatically,
  /// keyed off whichever bytecode instruction is currently being
  /// translated -- see `current_ip`'s own docs.
  fn call_helper(&mut self, name: &str, args: &[IrValue]) -> IrValue {
    self.publish_ip();
    self.flush_live(self.current_ip);
    let result = self.call_helper_raw(name, args);
    self.mark_stale_live(self.current_ip);
    // `live_regs_at` is `live_in`, which by definition EXCLUDES the
    // current instruction's own destination register (a `def` gets
    // subtracted, never added, by the standard liveness equation --
    // see `typeflow::liveness`'s own docs). That's correct for
    // ordinary register writes going through `store_reg`, but MANY of
    // the helpers this call reaches (`zuri_jit_get_global`,
    // `zuri_jit_call_finish`, `zuri_jit_get_field`, ...) write their
    // result DIRECTLY to `VM::registers[base+dst]` on the Rust side,
    // completely bypassing `store_reg`/`reg_cache` -- meaning the
    // JIT-generated Cranelift `Variable` for `dst` (whatever it held
    // BEFORE this call, possibly stale garbage from a previous
    // definition or even function entry) would otherwise never get
    // invalidated, and a LATER `load_reg(dst)` would wrongly trust it
    // instead of the fresh value the helper actually wrote to memory.
    // Explicitly staling `dst` too (regardless of whether it happens
    // to ALSO be a genuine "use" at this `ip`, which is what would
    // otherwise be needed for `live_in` to include it) closes that gap
    // for every helper-backed instruction uniformly.
    if let Some(dst) = typeflow::any_dst(&self.proto.chunk.code[self.current_ip]) {
      self.reg_cache[dst as usize] = RegCache::Stale;
      // Same reasoning as `store_reg`'s identical line: this register
      // is being freshly (re)defined, by a DIFFERENT mechanism than
      // `store_reg` but just as much a real write -- any stale
      // `scalar_lists` entry for it needs to go.
      self.scalar_lists.remove(&dst);
      self.scalar_instances.remove(&dst);
    }
    result
  }

  /// Publishes this frame's current bytecode position to `VM::jit_ip`,
  /// so a stack trace built from here names the right source line.
  ///
  /// The interpreter keeps `CallFrame::ip` current by storing it on
  /// EVERY instruction, precisely because any instruction can raise.
  /// Compiled code cannot afford that, and does not need it: the only
  /// ways a compiled frame's position ever becomes observable are
  /// raising an exception and becoming the caller of a new frame, and
  /// BOTH happen inside a `jit::runtime` helper. So publishing once per
  /// helper call -- a single store of an immediate to a fixed `VM`
  /// offset, on a path that is already paying for an FFI call -- covers
  /// every observable case at a small fraction of the interpreter's
  /// cost. `emit_safepoint`'s own `call_helper_raw` is deliberately not
  /// included: a collection raises nothing and pushes no frame.
  ///
  /// Stores `current_ip + 1`, matching `CallFrame::ip`'s "one past the
  /// instruction being executed" convention exactly, so
  /// `build_stacktrace` can subtract one from either source
  /// indifferently.
  fn publish_ip(&mut self) {
    let ip = self.u64c(self.current_ip as u64 + 1);
    let vm = self.vm_param;
    self.fb.ins().store(
      cranelift_codegen::ir::MemFlagsData::trusted(),
      ip,
      vm,
      JIT_IP_OFFSET,
    );
  }

  /// Calls a `jit::runtime` helper that follows the OK(0)/ERR(1) status
  /// convention (see that module's docs): on `ERR`, immediately returns
  /// from the WHOLE compiled function (the exception is already sitting
  /// in `VM::jit_pending_exception`, ready for `VM::invoke_compiled` to
  /// pick up) rather than continuing this instruction's own codegen.
  /// Always refreshes the registers pointer afterward -- see this
  /// module's docs on why every helper call is conservatively treated
  /// as potentially frame-pushing.
  fn call_checked(&mut self, name: &str, args: &[IrValue]) {
    let status = self.call_helper(name, args);
    let zero = self.i64c(0);
    let is_err = self.fb.ins().icmp(IntCC::NotEqual, status, zero);
    let err_block = self.fb.create_block();
    let ok_block = self.fb.create_block();
    self.fb.ins().brif(is_err, err_block, &[], ok_block, &[]);

    self.fb.switch_to_block(err_block);
    let junk = self.i64c(0);
    self.fb.ins().return_(&[junk]);

    self.fb.switch_to_block(ok_block);
    self.refresh_regs();
  }

  /// `SigRef` for `jit::EntryFn`'s own call shape -- what every fast-
  /// path `call_indirect` in `emit_fast_call` targets. Imported at most
  /// once per compiled function and cached, since it's the exact same
  /// shape at every call site.
  fn entry_sig_ref(&mut self) -> SigRef {
    if let Some(sig) = self.entry_sig {
      return sig;
    }
    let mut sig = self.module.make_signature();
    sig.params.push(AbiParam::new(types::I64)); // vm
    sig.params.push(AbiParam::new(types::I64)); // base
    sig.params.push(AbiParam::new(types::I64)); // closure
    sig.params.push(AbiParam::new(types::I32)); // osr_id
    sig.returns.push(AbiParam::new(types::I64));
    let sig_ref = self.fb.import_signature(sig);
    self.entry_sig = Some(sig_ref);
    sig_ref
  }

  /// The 8-byte scratch stack slot `prepare` helpers write the resolved
  /// callee closure's `Value` bits into -- see `closure_out_slot`'s own
  /// docs. Allocated at most once per compiled function.
  fn closure_out_slot(&mut self) -> StackSlot {
    if let Some(slot) = self.closure_out_slot {
      return slot;
    }
    let slot =
      self
        .fb
        .create_sized_stack_slot(StackSlotData::new(StackSlotKind::ExplicitSlot, 8, 3));
    self.closure_out_slot = Some(slot);
    slot
  }

  /// The 8-byte scratch stack slot `zuri_jit_list_data` writes the
  /// resolved list's current length into -- see `list_len_slot`'s own
  /// docs. Allocated at most once per compiled function.
  fn list_len_slot(&mut self) -> StackSlot {
    if let Some(slot) = self.list_len_slot {
      return slot;
    }
    let slot =
      self
        .fb
        .create_sized_stack_slot(StackSlotData::new(StackSlotKind::ExplicitSlot, 8, 3));
    self.list_len_slot = Some(slot);
    slot
  }

  /// The fast, inline-cache-style direct-call pattern shared by
  /// `Instr::Call` and `Instr::Invoke` -- see `jit::runtime`'s "Fast,
  /// inline-cache-style direct calls" docs for the full protocol this
  /// implements. `prepare_helper` is `zuri_jit_call_prepare` or
  /// `zuri_jit_invoke_prepare`, called with `prepare_args` PLUS the
  /// address of the scratch closure-out slot (appended here, not by the
  /// caller); `new_base` is the callee's frame base (already computable
  /// at compile time as `base + reg + 1`, so there's no need for
  /// `prepare` to report it back). `slow_helper` (an ordinary
  /// `call_checked` target -- `zuri_jit_call`/`zuri_jit_invoke`) is the
  /// fully general fallback for a `0` (not-yet-compiled, or not even a
  /// closure) result from `prepare`.
  fn emit_fast_call(
    &mut self,
    prepare_helper: &'static str,
    prepare_args: &[IrValue],
    new_base: IrValue,
    dst: u8,
    slow_helper: &'static str,
    slow_args: &[IrValue],
  ) {
    let closure_out_addr = {
      let slot = self.closure_out_slot();
      self.fb.ins().stack_addr(types::I64, slot, 0)
    };
    let mut args = prepare_args.to_vec();
    args.push(closure_out_addr);
    let prepare = self.call_helper(prepare_helper, &args);
    self.refresh_regs();
    let zero = self.i64c(0);
    let is_fast = self.fb.ins().icmp(IntCC::NotEqual, prepare, zero);

    let fast_block = self.fb.create_block();
    let slow_block = self.fb.create_block();
    let done_block = self.fb.create_block();
    self
      .fb
      .ins()
      .brif(is_fast, fast_block, &[], slow_block, &[]);

    self.fb.switch_to_block(fast_block);
    let closure_bits = {
      let slot = self.closure_out_slot();
      self.fb.ins().stack_load(types::I64, types::I64, slot, 0)
    };
    let neg1 = self.fb.ins().iconst(types::I32, -1);
    let sig = self.entry_sig_ref();
    // A genuine nested call into another compiled Zuri function's own
    // entry point -- NOT routed through `call_helper` (this is a
    // `call_indirect` to JIT-compiled code, not a `jit::runtime`
    // helper), so the flush/stale-mark bracketing it gets automatically
    // there has to be done explicitly here instead. The callee is free
    // to allocate, trigger a GC safepoint, or recurse arbitrarily
    // deep -- exactly the kind of call this cache exists to stay
    // correct across.
    self.flush_live(self.current_ip);
    let call =
      self
        .fb
        .ins()
        .call_indirect(sig, prepare, &[self.vm_param, new_base, closure_bits, neg1]);
    let ret_bits = self.fb.inst_results(call)[0];
    self.mark_stale_live(self.current_ip);
    self.refresh_regs();
    let base = self.base_param;
    let dst_i = self.idx(dst);
    self.call_checked(
      "zuri_jit_call_finish",
      &[self.vm_param, base, dst_i, new_base, ret_bits],
    );
    self.fb.ins().jump(done_block, &[]);

    self.fb.switch_to_block(slow_block);
    self.call_checked(slow_helper, slow_args);
    self.fb.ins().jump(done_block, &[]);

    self.fb.switch_to_block(done_block);
  }

  /// Can this construction site skip allocating entirely, keeping the
  /// instance's fields in a Cranelift stack slot instead?
  ///
  /// Mirrors `scalar_replace_eligible`'s conditions, for the same
  /// reasons -- see its docs for why `Move` and the speculative-body
  /// gate are ruled out wholesale rather than reasoned about:
  /// - `escape::analyze_one` proves the constructed value never leaves
  ///   this function (consulting this site's own class field-shadowing
  ///   safety, without which reading `d.x` alone would count as an
  ///   escape).
  /// - The constructor is simple enough to reproduce inline, i.e. it
  ///   only copies parameters into fields -- see
  ///   `jit::ConstructInfo::simple_ctor_param_slots`.
  /// - No `Instr::Move` anywhere reads the destination register, so
  ///   `scalar_instances` only ever answers for the exact register
  ///   `analyze_one` reasoned about.
  ///
  /// Deliberately does NOT inherit `scalar_replace_eligible`'s
  /// "no specialized/speculative body" gate. That gate exists for a
  /// specific, reproduced speculation bug involving a list read through
  /// a VARIABLE index (see its docs) -- and a scalar-replaced instance
  /// has no variable index anywhere: every field resolves to a
  /// compile-time-constant slot, emitted as a fixed-offset
  /// `stack_load`/`stack_store`. Keeping the gate here would disable
  /// this optimization on essentially every real numeric workload,
  /// since those are exactly the functions that get a specialized
  /// body.
  fn scalar_construct_eligible(&self, ip: usize, dst: u8) -> bool {
    let Some(info) = self.construct_info.get(&ip) else {
      return false;
    };
    if info.simple_ctor_param_slots.is_none() || info.field_count == 0 {
      return false;
    }
    for instr in &self.proto.chunk.code {
      if let Instr::Move { src, .. } = instr
        && *src == dst
      {
        return false;
      }
    }
    !escape::analyze_one(self.proto, ip, None, Some(&info.safety)).escapes
  }

  /// Builds a proven-non-escaping instance with NO heap allocation:
  /// the constructor's parameter-to-field copies are replayed straight
  /// into a stack slot, and no object, no `@new` call, and no GC work
  /// happen at all.
  ///
  /// Every slot is initialized -- the ones the constructor writes from
  /// its arguments, and any remaining declared field to nil -- before
  /// the slot is registered as a GC root, for the same reason
  /// `emit_scalar_make_list` populates first: an uninitialized slot is
  /// not a valid `Value` for a root scan to walk.
  fn emit_scalar_construct(&mut self, ip: usize, dst: u8, func: u8, num_args: u8) {
    let (field_count, param_slots) = {
      let info = &self.construct_info[&ip];
      (
        info.field_count,
        info
          .simple_ctor_param_slots
          .clone()
          .expect("eligibility checked simple_ctor_param_slots"),
      )
    };

    let slot = self.fb.create_sized_stack_slot(StackSlotData::new(
      StackSlotKind::ExplicitSlot,
      field_count as u32 * 8,
      3,
    ));

    let nil = self.u64c(crate::vm::value::Value::nil().to_bits());
    for i in 0..field_count {
      self
        .fb
        .ins()
        .stack_store(types::I64, nil, slot, (i as i32) * 8);
    }

    // Arguments sit at `func + 1 ..= func + num_args`, exactly as the
    // ordinary call convention leaves them.
    for (param, &field_slot) in param_slots.iter().enumerate() {
      if param >= num_args as usize {
        break;
      }
      let v = self.load_reg(func + 1 + param as u8);
      self
        .fb
        .ins()
        .stack_store(types::I64, v, slot, (field_slot as i32) * 8);
    }

    let addr = self.fb.ins().stack_addr(types::I64, slot, 0);
    let count_c = self.u64c(field_count as u64);
    self.call_checked("zuri_jit_push_scalar_root", &[self.vm_param, addr, count_c]);
    self.scalar_instances.insert(dst, (slot, ip));
  }

  /// Resolves `name_const` to a field slot on a scalar-replaced
  /// instance held in `obj`, or `None` if `obj` isn't one (or the name
  /// isn't a field of its class, which would mean a method read and
  /// must go the general way).
  fn scalar_instance_slot(&self, obj: u8, name_const: u16) -> Option<(StackSlot, u16)> {
    let (slot, site) = *self.scalar_instances.get(&obj)?;
    let info = self.construct_info.get(&site)?;
    let name = self.proto.chunk.constants.get(name_const as usize)?;
    if !name.is_string() {
      return None;
    }
    let field = *info.field_slots.get(name.as_str())?;
    Some((slot, field))
  }

  /// `Instr::Call`'s PROVEN constructor shape (`jit::CallTarget
  /// ::ConstructKnown`): the class -> constructor -> prototype chain
  /// `emit_construct_call` re-walks per instance was settled at
  /// compile time, leaving one guard and a helper that does no
  /// resolution -- see `VM::resolve_construct_target` for the proof
  /// and `zuri_jit_construct_prepare` for what the guard licenses.
  ///
  /// The guard is class IDENTITY plus a `method_table_generation`
  /// match, the same pair (and the same reasoning) as
  /// `emit_self_invoke`'s. It only works because classes are
  /// old-generation allocations and therefore never relocate -- see
  /// `Heap::alloc_class`. On a miss (a global rebound to a different
  /// class, or a class monkey-patched after this function compiled)
  /// control falls into the ordinary dynamic construction path, which
  /// re-resolves everything itself and is still correct.
  fn emit_construct_known(
    &mut self,
    dst: u8,
    func: u8,
    num_args: u8,
    guard_bits: u64,
    generation: u64,
    field_count: u16,
    ctor_bits: u64,
    proto_ptr: usize,
  ) {
    let base = self.base_param;
    let vm_p = self.vm_param;
    let callee = self.load_reg(func);
    let target_class = self.u64c(guard_bits);
    let class_hit = self.fb.ins().icmp(IntCC::Equal, callee, target_class);
    let snapshot = self.snapshot_reg_cache();

    // Every operand either arm needs is materialized HERE, in the
    // block that dominates all of them. A value defined inside one arm
    // and used from another is exactly the dominance error this file
    // already tripped over once -- and it surfaces as a silent
    // Cranelift verifier failure that makes the whole function
    // JIT-ineligible, which reads as a performance regression rather
    // than as the bug it is. Check `ZURI_JIT_LOG=1` for `ineligible:`
    // after touching this.
    // `func`, NOT `func + 1`: this path's callee window deliberately
    // starts at the callee register itself so the constructor's
    // arguments need no shifting -- see
    // `VM::prepare_known_construction`, which must agree with this
    // exactly. The dynamic path below (`emit_construct_call`) still
    // uses the ordinary `func + 1` convention with a real shift.
    let new_base = self.fb.ins().iadd_imm_s(base, func as i64);
    let func_i = self.idx(func);
    let num_args_i = self.idx(num_args);
    let dst_i = self.idx(dst);
    let ctor_v = self.u64c(ctor_bits);
    let proto_v = self.u64c(proto_ptr as u64);
    let field_count_v = self.i64c(field_count as i64);
    let closure_out_addr = {
      let slot = self.closure_out_slot();
      self.fb.ins().stack_addr(types::I64, slot, 0)
    };

    let gen_check_block = self.fb.create_block();
    let dynamic_block = self.fb.create_block();
    let done_block = self.fb.create_block();
    self
      .fb
      .ins()
      .brif(class_hit, gen_check_block, &[], dynamic_block, &[]);

    self.fb.switch_to_block(gen_check_block);
    let cur_gen = self.fb.ins().load(
      types::I64,
      cranelift_codegen::ir::MemFlagsData::trusted(),
      vm_p,
      METHOD_TABLE_GENERATION_OFFSET,
    );
    let target_gen = self.u64c(generation);
    let gen_hit = self.fb.ins().icmp(IntCC::Equal, cur_gen, target_gen);
    let lean_block = self.fb.create_block();
    self
      .fb
      .ins()
      .brif(gen_hit, lean_block, &[], dynamic_block, &[]);

    self.fb.switch_to_block(lean_block);
    let prepare = self.call_helper(
      "zuri_jit_construct_prepare",
      &[
        vm_p,
        base,
        func_i,
        num_args_i,
        dst_i,
        ctor_v,
        proto_v,
        field_count_v,
        closure_out_addr,
      ],
    );
    self.refresh_regs();
    let zero = self.i64c(0);
    let is_fast = self.fb.ins().icmp(IntCC::NotEqual, prepare, zero);
    let fast_block = self.fb.create_block();
    self
      .fb
      .ins()
      .brif(is_fast, fast_block, &[], dynamic_block, &[]);

    self.fb.switch_to_block(fast_block);
    let closure_bits = {
      let slot = self.closure_out_slot();
      self.fb.ins().stack_load(types::I64, types::I64, slot, 0)
    };
    let neg1 = self.fb.ins().iconst(types::I32, -1);
    let sig = self.entry_sig_ref();
    // Identical bracketing requirement to `emit_fast_call`'s own
    // `call_indirect` -- see the note there.
    self.flush_live(self.current_ip);
    self
      .fb
      .ins()
      .call_indirect(sig, prepare, &[vm_p, new_base, closure_bits, neg1]);
    self.mark_stale_live(self.current_ip);
    self.refresh_regs();
    self.call_checked("zuri_jit_new_finish", &[vm_p, base, dst_i, new_base]);
    self.fb.ins().jump(done_block, &[]);

    self.reg_cache = snapshot.clone();
    self.fb.switch_to_block(dynamic_block);
    self.emit_construct_call(dst, func, num_args);
    self.fb.ins().jump(done_block, &[]);

    self.fb.switch_to_block(done_block);
    self.restore_dirty_from_snapshot(&snapshot, dst);
  }

  /// `Instr::Call`'s CONSTRUCTOR shape (`jit::CallTarget::Construct`):
  /// same `prepare` / `call_indirect` / `finish` protocol as
  /// `emit_fast_call`, differing only in that the value delivered to
  /// `dst` is the newly built instance rather than the callee's return
  /// value -- hence `zuri_jit_new_finish`, which takes no return-value
  /// operand at all, in place of `zuri_jit_call_finish`.
  ///
  /// The miss path is deliberately the ORDINARY fast call rather than
  /// the general slow helper: `CallTarget::Construct` is only a
  /// statically-resolved hint about which shape this site most likely
  /// needs, and a global reassigned from a class to a plain function
  /// between compile time and run time should degrade to the normal
  /// compiled-to-compiled call path, not all the way to
  /// `dispatch_call_sync`.
  fn emit_construct_call(&mut self, dst: u8, func: u8, num_args: u8) {
    let base = self.base_param;
    let vm_p = self.vm_param;
    let func_i = self.idx(func);
    let num_args_i = self.idx(num_args);
    let dst_i = self.idx(dst);
    let new_base = self.fb.ins().iadd_imm_s(base, func as i64 + 1);
    let closure_out_addr = {
      let slot = self.closure_out_slot();
      self.fb.ins().stack_addr(types::I64, slot, 0)
    };

    let prepare = self.call_helper(
      "zuri_jit_new_prepare",
      &[vm_p, base, func_i, num_args_i, dst_i, closure_out_addr],
    );
    self.refresh_regs();
    let zero = self.i64c(0);
    let is_fast = self.fb.ins().icmp(IntCC::NotEqual, prepare, zero);

    let fast_block = self.fb.create_block();
    let slow_block = self.fb.create_block();
    let done_block = self.fb.create_block();
    self
      .fb
      .ins()
      .brif(is_fast, fast_block, &[], slow_block, &[]);

    self.fb.switch_to_block(fast_block);
    let closure_bits = {
      let slot = self.closure_out_slot();
      self.fb.ins().stack_load(types::I64, types::I64, slot, 0)
    };
    let neg1 = self.fb.ins().iconst(types::I32, -1);
    let sig = self.entry_sig_ref();
    // Identical bracketing requirement to `emit_fast_call`'s own
    // `call_indirect` -- see the note there.
    self.flush_live(self.current_ip);
    self
      .fb
      .ins()
      .call_indirect(sig, prepare, &[self.vm_param, new_base, closure_bits, neg1]);
    self.mark_stale_live(self.current_ip);
    self.refresh_regs();
    // Reuses the operands materialized before the branch rather than
    // re-emitting them here: anything defined in THIS block would not
    // dominate `slow_block` below, and Cranelift's verifier rejects
    // that outright.
    self.call_checked("zuri_jit_new_finish", &[vm_p, base, dst_i, new_base]);
    self.fb.ins().jump(done_block, &[]);

    self.fb.switch_to_block(slow_block);
    self.emit_fast_call(
      "zuri_jit_call_prepare",
      &[vm_p, base, func_i, num_args_i, dst_i],
      new_base,
      dst,
      "zuri_jit_call",
      &[vm_p, base, func_i, num_args_i, dst_i],
    );
    self.fb.ins().jump(done_block, &[]);

    self.fb.switch_to_block(done_block);
  }

  /// `Instr::Call`'s PROVEN self-recursive fast path (`jit::CallTarget
  /// ::SelfRecursive`, from `VM::resolve_call_targets`'s use of
  /// `escape::self_reference_facts`): no runtime guard at all, since
  /// there is nothing left to misidentify -- the callee register's
  /// VALUE is never even read here, only its bytecode INDEX (needed for
  /// `new_base`'s frame-layout math). Reuses `self.closure_param` (this
  /// invocation's own closure, already held stable for as long as it's
  /// running -- see `VM::ensure_stable_for_compiled_entry`'s own docs on
  /// why that stability guarantee needs no re-establishing for a value
  /// that's already the CURRENT frame's own closure) as the callee, and
  /// a genuine relocation-resolved direct `call` to `own_func_id` --
  /// NOT `call_indirect` on a runtime-loaded pointer -- as the actual
  /// call instruction: `cranelift_module` resolves the target address
  /// at link time, so there's no pointer load, no indirect-branch
  /// misprediction risk, and no `zuri_jit_call_prepare` FFI round trip
  /// at all on the common (native call stack not exhausted) path.
  ///
  /// Still needs `zuri_jit_direct_call_prepare` for the frame-setup
  /// work itself (`VM::setup_closure_call`, which can resize/reallocate
  /// `VM::registers` and therefore genuinely needs the same flush/
  /// refresh bracketing as any other helper call) and the depth check
  /// (native call-stack depth is a real resource `Instr::Call`
  /// recursion has to respect even when the target is statically
  /// known) -- this skips the RESOLVER, not frame setup.
  fn emit_self_call(&mut self, dst: u8, func: u8, num_args: u8) {
    let base = self.base_param;
    let vm_p = self.vm_param;
    let new_base = self.fb.ins().iadd_imm_s(base, func as i64 + 1);
    let num_args_i = self.idx(num_args);
    let dst_i = self.idx(dst);
    let closure_bits = self.closure_param;

    let ok = self.call_helper(
      "zuri_jit_direct_call_prepare",
      &[vm_p, closure_bits, new_base, num_args_i, dst_i],
    );
    self.refresh_regs();
    let zero = self.i64c(0);
    let is_ok = self.fb.ins().icmp(IntCC::NotEqual, ok, zero);

    let fast_block = self.fb.create_block();
    let slow_block = self.fb.create_block();
    let done_block = self.fb.create_block();
    self.fb.ins().brif(is_ok, fast_block, &[], slow_block, &[]);

    // No snapshot/restore needed around this split (unlike
    // `emit_known_call`/`emit_self_invoke`, both of which guard BEFORE
    // their first helper call): the ONE helper call above runs
    // UNCONDITIONALLY, before either branch, so it's already flushed
    // whatever was live-and-dirty on BOTH paths -- exactly
    // `emit_fast_call`'s own proven-safe shape, just with a leaner
    // prepare helper. See `restore_dirty_from_snapshot`'s own docs for
    // the bug class this reasoning has to hold up against.
    self.fb.switch_to_block(fast_block);
    let func_ref = self
      .module
      .declare_func_in_func(self.own_func_id, self.fb.func);
    let neg1 = self.fb.ins().iconst(types::I32, -1);
    self.flush_live(self.current_ip);
    let call = self
      .fb
      .ins()
      .call(func_ref, &[vm_p, new_base, closure_bits, neg1]);
    let ret_bits = self.fb.inst_results(call)[0];
    self.mark_stale_live(self.current_ip);
    self.refresh_regs();
    self.call_checked(
      "zuri_jit_call_finish",
      &[vm_p, base, dst_i, new_base, ret_bits],
    );
    self.fb.ins().jump(done_block, &[]);

    self.fb.switch_to_block(slow_block);
    let func_i = self.idx(func);
    self.call_checked("zuri_jit_call", &[vm_p, base, func_i, num_args_i, dst_i]);
    self.fb.ins().jump(done_block, &[]);

    self.fb.switch_to_block(done_block);
  }

  /// `Instr::Call`'s PROVEN-but-reassignable fast path (`jit::CallTarget
  /// ::Known`, from `VM::resolve_call_targets`'s use of `escape::
  /// global_ref_facts`): the callee register is proven to hold an
  /// UNMODIFIED read of some OTHER global name that, at THIS function's
  /// OWN compile time, already resolved to a different, already-
  /// compiled function -- but unlike self-recursion, that global
  /// binding could still be reassigned before this exact call site
  /// actually runs, so a cheap value-identity guard against the
  /// CURRENT register contents comes first. `entry` is baked as a raw
  /// address constant -- sound because `CompiledFunction`'s own docs
  /// guarantee compiled code is never unloaded or recompiled once
  /// produced, so this address stays valid for the rest of the process.
  /// On a guard miss (or the same depth-exhausted case `emit_self_call`
  /// handles), falls all the way back to the fully general
  /// `zuri_jit_call` slow helper -- exactly `emit_fast_call`'s own slow
  /// path, since a miss here means "let the general path re-resolve
  /// whatever this actually is right now," not "try again with stale
  /// information."
  ///
  /// Guards BEFORE its first helper call (unlike `emit_self_call`), so
  /// -- per `restore_dirty_from_snapshot`'s own docs -- this needs the
  /// snapshot/reset discipline `emit_self_get_field`/`emit_list_get_index`
  /// already established: two INDEPENDENT call sites
  /// (`zuri_jit_direct_call_prepare` on the guard-pass path,
  /// `zuri_jit_call` on the guard-fail path) sit in mutually exclusive
  /// branches, so `self.reg_cache` is reset to the pre-branch snapshot
  /// before generating the slow path, or its `flush_live` would
  /// silently skip a store the OTHER (never-taken-at-runtime-for-this-
  /// compile) branch's bookkeeping already "used up."
  /// Upper bound on an inline candidate's bytecode length. Small on
  /// purpose: the whole value of inlining here is erasing a call
  /// protocol that costs far more than the callee's own arithmetic, and
  /// that ratio only holds for genuinely tiny helpers. A larger budget
  /// would trade a shrinking win for real code growth at every site.
  const MAX_INLINE_OPS: usize = 24;

  /// Inlines a call outright when the callee is a small, straight-line,
  /// arithmetic-only leaf -- no call protocol, no frame, no register
  /// flush, no reload. This is what removes the ~17ns per call that
  /// dominates helper-heavy numeric code like
  /// `benchmarks/spectral-norm.zu`, where the callee's own work is a
  /// handful of flops.
  ///
  /// Returns whether it inlined; `false` leaves the caller to emit an
  /// ordinary call, unchanged.
  ///
  /// # Why this needs no GC roots, no frame, and no safepoint
  ///
  /// This is the entire reason the eligibility rules are as narrow as
  /// they are, so it is worth stating precisely.
  ///
  /// Every value an inlined body produces lives in a Cranelift SSA
  /// value, NOT in `VM::registers` -- so nothing a garbage collection
  /// would need to see or relocate is reachable from it. That is only
  /// sound if a collection cannot happen while those values are live,
  /// which `inline_plan` guarantees structurally rather than hopefully:
  /// the body is proven to contain no call, no allocation, and no
  /// helper that could do either. Note this is NOT implied by the
  /// opcode whitelist alone -- `a + b` on non-numeric operands calls
  /// `zuri_jit_add_slow`, which can allocate a string or a bigint --
  /// which is why every operand must ALSO be proven numeric, making
  /// those slow paths unreachable rather than merely unlikely.
  ///
  /// For the same reason there is no safepoint: an inlined body cannot
  /// allocate, so it cannot advance the heap toward a collection, and
  /// the enclosing loop's own back edge still carries one.
  ///
  /// # Why the callee cannot have changed underneath this
  ///
  /// `guard_bits` is the exact closure `VM::resolve_call_targets` saw.
  /// A global binding can be reassigned at runtime, so the inlined body
  /// runs only when the callee register still holds that identical
  /// closure; anything else falls through to the ordinary call, which
  /// resolves whatever is actually there. `proto_ptr` itself is only
  /// ever read HERE, at compile time -- generated code never touches
  /// it -- so a later reassignment cannot leave it dangling behind.
  fn try_emit_inlined_call(
    &mut self,
    ip: usize,
    dst: u8,
    func: u8,
    num_args: u8,
    guard_bits: u64,
    proto_ptr: usize,
  ) -> bool {
    // SAFETY: see this function's own docs -- `proto_ptr` names a live,
    // old-generation `ObjFunction`, read only during compilation.
    let callee = unsafe { &*(proto_ptr as *const ObjFunction) };
    let Some(plan) = self.inline_plan(ip, callee, func, num_args) else {
      return false;
    };

    if crate::jit::log_enabled() {
      eprintln!(
        "[jit] inlined '{}' ({} ops) into '{}' at ip {}",
        callee.name,
        plan.len(),
        self.proto.name,
        ip
      );
    }

    let callee_val = self.load_reg(func);
    let snapshot = self.snapshot_reg_cache();

    let slow_block = self.fb.create_block();
    let done_block = self.fb.create_block();
    self.emit_callee_proto_guard(callee_val, guard_bits, slow_block);

    let result = self.emit_inlined_body(callee, &plan, func);
    self.store_reg(dst, result);
    self.fb.ins().jump(done_block, &[]);

    self.fb.switch_to_block(slow_block);
    self.emit_safepoint();
    self.emit_generic_call(dst, func, num_args);
    // The two arms disagree about `dst` the same way every other
    // guarded instruction's do -- the inlined arm defines it in its
    // `Variable` and writes no memory, the call arm writes memory and
    // stale-marks it. See `resync_receiver_from_memory` for the bug
    // that leaving that disagreement in place produces.
    self.resync_dst_from_memory(dst);
    self.fb.ins().jump(done_block, &[]);

    self.fb.switch_to_block(done_block);
    self.restore_dirty_from_snapshot(&snapshot, dst);
    true
  }

  /// Decides whether `callee` can be inlined at this site, returning the
  /// prefix of its bytecode to emit (everything up to and including its
  /// first `Return`) when it can.
  ///
  /// Every rule here exists to hold up one of the two guarantees
  /// `try_emit_inlined_call` depends on -- "cannot allocate or call"
  /// and "is straight-line" -- so none of them is merely conservative
  /// tidiness:
  ///
  /// - **Straight-line.** No jump of any kind may appear before the
  ///   `Return`. That makes the body a pure expression tree, so callee
  ///   registers can be plain SSA values with no `Variable`, no merge
  ///   blocks, and no liveness analysis of their own. It also rules out
  ///   loops, which would otherwise need a safepoint this deliberately
  ///   does not emit.
  /// - **Arithmetic only.** The opcode must be one whose proven-numeric
  ///   form emits pure floating-point arithmetic and nothing else.
  /// - **Everything proven numeric.** Tracked forward from the
  ///   arguments: a parameter counts as numeric only if the CALLER
  ///   already proved that argument register numeric, and every other
  ///   register only once something numeric has been written to it.
  ///   This is what makes the arithmetic slow paths (which can
  ///   allocate) unreachable.
  /// - **No upvalues, exact arity, non-variadic.** Anything else means
  ///   the callee's parameters are not simply the argument registers.
  fn inline_plan(
    &self,
    ip: usize,
    callee: &ObjFunction,
    func: u8,
    num_args: u8,
  ) -> Option<Vec<Instr>> {
    if callee.variadic || callee.arity != num_args || !callee.upvalues.is_empty() {
      return None;
    }
    // The callee's registers are addressed as caller registers
    // `func + 1 + r` by the ordinary calling convention; an inlined body
    // never materializes them, but the argument mapping below still has
    // to stay inside a `u8`.
    if (func as usize) + 1 + (callee.num_registers as usize) > u8::MAX as usize {
      return None;
    }
    let code = &callee.chunk.code;
    if code.len() > Self::MAX_INLINE_OPS {
      return None;
    }

    let mut numeric = vec![false; callee.num_registers as usize];
    for i in 0..num_args {
      // A parameter is numeric exactly when the caller already proved
      // the argument it is passed is.
      if !self.proven_numeric(ip, func + 1 + i) {
        return None;
      }
      *numeric.get_mut(i as usize)? = true;
    }

    let numeric_const = |idx: u16| -> bool {
      callee
        .chunk
        .constants
        .get(idx as usize)
        .is_some_and(|c| c.is_number())
    };
    let is_num = |numeric: &Vec<bool>, r: u8| numeric.get(r as usize).copied().unwrap_or(false);

    for (i, instr) in code.iter().enumerate() {
      match *instr {
        Instr::Return { src } => {
          if !is_num(&numeric, src) {
            return None;
          }
          return Some(code[..=i].to_vec());
        },
        Instr::LoadConst { dst, const_idx } => {
          if !numeric_const(const_idx) {
            return None;
          }
          *numeric.get_mut(dst as usize)? = true;
        },
        Instr::Move { dst, src } => {
          let v = is_num(&numeric, src);
          *numeric.get_mut(dst as usize)? = v;
          if !v {
            return None;
          }
        },
        Instr::Add { dst, a, b }
        | Instr::Sub { dst, a, b }
        | Instr::Mul { dst, a, b }
        | Instr::Div { dst, a, b } => {
          if !is_num(&numeric, a) || !is_num(&numeric, b) {
            return None;
          }
          *numeric.get_mut(dst as usize)? = true;
        },
        Instr::AddImm { dst, a, imm_const }
        | Instr::SubImm { dst, a, imm_const }
        | Instr::MulImm { dst, a, imm_const } => {
          if !is_num(&numeric, a) || !numeric_const(imm_const) {
            return None;
          }
          *numeric.get_mut(dst as usize)? = true;
        },
        Instr::Neg { dst, src } => {
          if !is_num(&numeric, src) {
            return None;
          }
          *numeric.get_mut(dst as usize)? = true;
        },
        // Everything else -- jumps, calls, field/index access, anything
        // that can allocate or raise -- disqualifies the whole callee.
        _ => return None,
      }
    }
    None
  }

  /// Emits an `inline_plan`-approved body, returning the `Value` bits
  /// its `Return` produced.
  ///
  /// Callee registers are tracked as plain SSA values rather than
  /// `Variable`s: the plan guarantees the body is straight-line, so
  /// every register has exactly one definition reaching every use and
  /// there is nothing for Cranelift's SSA construction to merge.
  fn emit_inlined_body(&mut self, callee: &ObjFunction, plan: &[Instr], func: u8) -> IrValue {
    let mut regs: Vec<IrValue> = Vec::with_capacity(callee.num_registers as usize);
    let zero = self.i64c(0);
    regs.resize(callee.num_registers as usize, zero);

    for i in 0..callee.arity {
      regs[i as usize] = self.load_reg(func + 1 + i);
    }

    let bake = |fc: &mut Self, idx: u16| -> IrValue {
      let v = callee.chunk.constants[idx as usize];
      fc.u64c(v.to_bits())
    };

    for instr in plan {
      match *instr {
        Instr::Return { src } => return regs[src as usize],
        Instr::LoadConst { dst, const_idx } => regs[dst as usize] = bake(self, const_idx),
        Instr::Move { dst, src } => regs[dst as usize] = regs[src as usize],
        Instr::Add { dst, a, b } => {
          regs[dst as usize] = self.inline_arith(regs[a as usize], regs[b as usize], FAdd)
        },
        Instr::Sub { dst, a, b } => {
          regs[dst as usize] = self.inline_arith(regs[a as usize], regs[b as usize], FSub)
        },
        Instr::Mul { dst, a, b } => {
          regs[dst as usize] = self.inline_arith(regs[a as usize], regs[b as usize], FMul)
        },
        Instr::Div { dst, a, b } => {
          regs[dst as usize] = self.inline_arith(regs[a as usize], regs[b as usize], FDiv)
        },
        Instr::AddImm { dst, a, imm_const } => {
          let b = bake(self, imm_const);
          regs[dst as usize] = self.inline_arith(regs[a as usize], b, FAdd)
        },
        Instr::SubImm { dst, a, imm_const } => {
          let b = bake(self, imm_const);
          regs[dst as usize] = self.inline_arith(regs[a as usize], b, FSub)
        },
        Instr::MulImm { dst, a, imm_const } => {
          let b = bake(self, imm_const);
          regs[dst as usize] = self.inline_arith(regs[a as usize], b, FMul)
        },
        Instr::Neg { dst, src } => {
          let f = self.to_f64(regs[src as usize]);
          let n = self.fb.ins().fneg(f);
          regs[dst as usize] = self.from_f64(n);
        },
        // Unreachable: `inline_plan` returns `None` for anything else,
        // and is the only producer of `plan`.
        _ => unreachable!("inline_plan admitted a non-inlinable instruction"),
      }
    }
    unreachable!("inline_plan always ends its plan with a Return")
  }

  /// One unguarded floating-point operation on two operands already
  /// proven numeric -- the inlined-body counterpart of
  /// `emit_binary_numeric_proven`, differing only in that it threads
  /// SSA values instead of bytecode registers.
  fn inline_arith(&mut self, a: IrValue, b: IrValue, op: InlineArith) -> IrValue {
    let fa = self.to_f64(a);
    let fb_ = self.to_f64(b);
    let r = match op {
      FAdd => self.fb.ins().fadd(fa, fb_),
      FSub => self.fb.ins().fsub(fa, fb_),
      FMul => self.fb.ins().fmul(fa, fb_),
      FDiv => self.fb.ins().fdiv(fa, fb_),
    };
    self.from_f64(r)
  }

  /// Branches to `fail_block` unless the callee register holds a
  /// closure over the prototype `guard_bits` names, leaving the
  /// success path as the current block.
  ///
  /// Three separate branches rather than one fused boolean, for the
  /// same reason `emit_ic_guard` needs them: masking a non-object
  /// `Value`'s bits into a "pointer" and loading through it would
  /// fault, and reading a non-closure `Obj`'s bytes at
  /// `ObjClosure::function`'s offset would read a different union arm.
  ///
  /// See `CallTarget::Known::guard_bits` for why this guards the
  /// PROTOTYPE and not the closure -- guarding the closure's own
  /// address silently stops matching the first time a minor collection
  /// relocates it.
  fn emit_callee_proto_guard(&mut self, callee_val: IrValue, guard_bits: u64, fail_block: Block) {
    let is_obj = self.is_obj(callee_val);
    let obj_block = self.fb.create_block();
    self.fb.ins().brif(is_obj, obj_block, &[], fail_block, &[]);

    self.fb.switch_to_block(obj_block);
    let ptr = self.obj_ptr(callee_val);
    let tag = self.obj_tag(ptr);
    let tag_closure = self.i64c(object::OBJ_TAG_CLOSURE as i64);
    let is_closure = self.fb.ins().icmp(IntCC::Equal, tag, tag_closure);
    let proto_block = self.fb.create_block();
    self
      .fb
      .ins()
      .brif(is_closure, proto_block, &[], fail_block, &[]);

    self.fb.switch_to_block(proto_block);
    let func_off = object::obj_closure_function_offset() as i32;
    let proto = self.fb.ins().load(
      types::I64,
      cranelift_codegen::ir::MemFlagsData::trusted(),
      ptr,
      func_off,
    );
    let want = self.u64c(guard_bits);
    let is_hit = self.fb.ins().icmp(IntCC::Equal, proto, want);
    let hit_block = self.fb.create_block();
    self.fb.ins().brif(is_hit, hit_block, &[], fail_block, &[]);
    self.fb.switch_to_block(hit_block);
  }

  /// `Instr::Call`'s fully general codegen -- the resolver-driven
  /// `zuri_jit_call_prepare` fast call, used whenever nothing stronger
  /// was proven about the callee.
  fn emit_generic_call(&mut self, dst: u8, func: u8, num_args: u8) {
    let base = self.base_param;
    let vm_p = self.vm_param;
    let func_i = self.idx(func);
    let num_args_i = self.idx(num_args);
    let dst_i = self.idx(dst);
    let new_base = self.fb.ins().iadd_imm_s(base, func as i64 + 1);
    self.emit_fast_call(
      "zuri_jit_call_prepare",
      &[vm_p, base, func_i, num_args_i, dst_i],
      new_base,
      dst,
      "zuri_jit_call",
      &[vm_p, base, func_i, num_args_i, dst_i],
    );
  }

  fn emit_known_call(&mut self, dst: u8, func: u8, num_args: u8, entry: usize, guard_bits: u64) {
    let base = self.base_param;
    let vm_p = self.vm_param;
    let callee_val = self.load_reg(func);
    let snapshot = self.snapshot_reg_cache();

    let new_base = self.fb.ins().iadd_imm_s(base, func as i64 + 1);
    let num_args_i = self.idx(num_args);
    let dst_i = self.idx(dst);

    let slow_block = self.fb.create_block();
    let fast_block = self.fb.create_block();
    let done_block = self.fb.create_block();
    self.emit_callee_proto_guard(callee_val, guard_bits, slow_block);

    let ok = self.call_helper(
      "zuri_jit_direct_call_prepare",
      &[vm_p, callee_val, new_base, num_args_i, dst_i],
    );
    self.refresh_regs();
    let zero = self.i64c(0);
    let is_ok = self.fb.ins().icmp(IntCC::NotEqual, ok, zero);
    self.fb.ins().brif(is_ok, fast_block, &[], slow_block, &[]);

    self.fb.switch_to_block(fast_block);
    let entry_addr = self.u64c(entry as u64);
    let sig = self.entry_sig_ref();
    let neg1 = self.fb.ins().iconst(types::I32, -1);
    self.flush_live(self.current_ip);
    let call = self
      .fb
      .ins()
      .call_indirect(sig, entry_addr, &[vm_p, new_base, callee_val, neg1]);
    let ret_bits = self.fb.inst_results(call)[0];
    self.mark_stale_live(self.current_ip);
    self.refresh_regs();
    self.call_checked(
      "zuri_jit_call_finish",
      &[vm_p, base, dst_i, new_base, ret_bits],
    );
    self.fb.ins().jump(done_block, &[]);

    self.reg_cache = snapshot.clone();
    self.fb.switch_to_block(slow_block);
    let func_i = self.idx(func);
    self.call_checked("zuri_jit_call", &[vm_p, base, func_i, num_args_i, dst_i]);
    self.fb.ins().jump(done_block, &[]);

    self.fb.switch_to_block(done_block);
    self.restore_dirty_from_snapshot(&snapshot, dst);
  }

  /// `Instr::Invoke`'s eligibility check for `emit_self_invoke`: the
  /// invoked method's name must equal THIS function's own name AND
  /// `VM::resolve_self_class` must have proven `proto` owns that method
  /// on its own class (`self.self_class_bits`, resolved once ahead of
  /// compilation). Resolves the name via `proto.chunk.constants`
  /// directly -- a compile-time lookup, like `self_field_slot`'s own --
  /// never hands the name to generated code.
  fn self_invoke_target(&self, method_const: u16) -> Option<(u64, u64)> {
    let (class_bits, generation) = self.self_class_bits?;
    let name = self.proto.chunk.constants[method_const as usize];
    if name.is_string() && name.as_str() == self.proto.name {
      Some((class_bits, generation))
    } else {
      None
    }
  }

  /// `Instr::Invoke`'s PROVEN-same-method fast path (see
  /// `self_invoke_target`/`jit::CompileFacts::self_class_bits`). The
  /// runtime guard is a receiver CLASS check plus a method-table-
  /// generation check, NOT a value-identity check (unlike
  /// `emit_known_call`): ANY receiver whose class is bit-identical to
  /// the baked class, REGARDLESS of which register/expression it came
  /// from (`self`, `self.left`, a local, ...), is guaranteed by
  /// `resolve_self_class`'s proof to resolve `method_const` to this
  /// exact compiled function -- PROVIDED the class's method table
  /// hasn't been monkey-patched since that proof was taken (see
  /// `VM::method_table_generation`'s own docs for why that second check
  /// is load-bearing, not defensive-programming boilerplate). On a
  /// guard hit, this is genuine self-recursion (same `own_func_id`, a
  /// real relocation-resolved direct `call`), not a `Known`-style guess
  /// at some OTHER already-compiled function.
  ///
  /// Same two-stage `is_obj` -> tag-check discipline as
  /// `emit_self_get_field` (a non-`Obj::Instance` `obj` falls straight
  /// to the general path rather than being assumed away), and the same
  /// snapshot/reset discipline as `emit_known_call` (the guard runs
  /// before any helper call, so the guard-fail path's `zuri_jit_invoke`
  /// and the guard-pass path's `zuri_jit_direct_call_prepare` are two
  /// independent call sites in mutually exclusive branches).
  fn emit_self_invoke(
    &mut self,
    ip: usize,
    dst: u8,
    obj: u8,
    method_const: u16,
    num_args: u8,
    class_bits: u64,
    generation: u64,
  ) {
    let base = self.base_param;
    let vm_p = self.vm_param;
    let receiver = self.load_reg(obj);
    let is_obj = self.is_obj(receiver);
    let snapshot = self.snapshot_reg_cache();
    // Computed here, in the single entry block every later block is
    // dominated by -- NOT inside `try_direct_block` (which is already
    // unreachable-via-fallthrough by the time these would otherwise be
    // needed, since Cranelift requires switching blocks before emitting
    // further instructions once one is terminated).
    let new_base = self.fb.ins().iadd_imm_s(base, obj as i64 + 1);
    let num_args_i = self.idx(num_args);
    // `1 + num_args`: the receiver the bytecode compiler already
    // duplicated into `obj + 1` occupies the callee's own register 0
    // ("self") -- see `Instr::Invoke`'s own doc comment in chunk.rs and
    // `zuri_jit_invoke_prepare`'s identical `1 + num_args` convention.
    // Passing bare `num_args` here would make `VM::setup_closure_call`
    // treat register 0 as a MISSING positional argument and overwrite
    // it with `nil` whenever `num_args < arity` -- exactly the "self.left
    // on a nil" corruption this comment is here to prevent regressing.
    let direct_num_args_i = self.i64c(num_args as i64 + 1);
    let dst_i = self.idx(dst);
    let closure_bits = self.closure_param;

    let obj_block = self.fb.create_block();
    let slow_block = self.fb.create_block();
    let done_block = self.fb.create_block();
    self.fb.ins().brif(is_obj, obj_block, &[], slow_block, &[]);

    self.fb.switch_to_block(obj_block);
    let ptr = self.obj_ptr(receiver);
    let tag = self.obj_tag(ptr);
    let tag_instance = self.i64c(object::OBJ_TAG_INSTANCE as i64);
    let is_instance = self.fb.ins().icmp(IntCC::Equal, tag, tag_instance);
    let class_check_block = self.fb.create_block();
    self
      .fb
      .ins()
      .brif(is_instance, class_check_block, &[], slow_block, &[]);

    self.fb.switch_to_block(class_check_block);
    let class_off = object::obj_instance_class_offset() as i32;
    let class_val = self.fb.ins().load(
      types::I64,
      cranelift_codegen::ir::MemFlagsData::trusted(),
      ptr,
      class_off,
    );
    let target_class = self.u64c(class_bits);
    let class_hit = self.fb.ins().icmp(IntCC::Equal, class_val, target_class);
    let gen_check_block = self.fb.create_block();
    self
      .fb
      .ins()
      .brif(class_hit, gen_check_block, &[], slow_block, &[]);

    self.fb.switch_to_block(gen_check_block);
    let cur_gen = self.fb.ins().load(
      types::I64,
      cranelift_codegen::ir::MemFlagsData::trusted(),
      vm_p,
      METHOD_TABLE_GENERATION_OFFSET,
    );
    let target_gen = self.u64c(generation);
    let gen_hit = self.fb.ins().icmp(IntCC::Equal, cur_gen, target_gen);
    let try_direct_block = self.fb.create_block();
    self
      .fb
      .ins()
      .brif(gen_hit, try_direct_block, &[], slow_block, &[]);

    self.fb.switch_to_block(try_direct_block);
    let ok = self.call_helper(
      "zuri_jit_direct_call_prepare",
      &[vm_p, closure_bits, new_base, direct_num_args_i, dst_i],
    );
    self.refresh_regs();
    let zero = self.i64c(0);
    let is_ok = self.fb.ins().icmp(IntCC::NotEqual, ok, zero);
    let fast_block = self.fb.create_block();
    self.fb.ins().brif(is_ok, fast_block, &[], slow_block, &[]);

    self.fb.switch_to_block(fast_block);
    let func_ref = self
      .module
      .declare_func_in_func(self.own_func_id, self.fb.func);
    let neg1 = self.fb.ins().iconst(types::I32, -1);
    self.flush_live(ip);
    let call = self
      .fb
      .ins()
      .call(func_ref, &[vm_p, new_base, closure_bits, neg1]);
    let ret_bits = self.fb.inst_results(call)[0];
    self.mark_stale_live(ip);
    self.refresh_regs();
    self.call_checked(
      "zuri_jit_call_finish",
      &[vm_p, base, dst_i, new_base, ret_bits],
    );
    self.fb.ins().jump(done_block, &[]);

    self.reg_cache = snapshot.clone();
    self.fb.switch_to_block(slow_block);
    let obj_i = self.idx(obj);
    let name = self.bake_const(method_const);
    self.call_checked(
      "zuri_jit_invoke",
      &[vm_p, base, obj_i, num_args_i, dst_i, name],
    );
    self.fb.ins().jump(done_block, &[]);

    self.fb.switch_to_block(done_block);
    self.restore_dirty_from_snapshot(&snapshot, dst);
  }

  /// `Instr::GetGlobal`'s inline-cache-style fast path: once this exact
  /// instruction has resolved its name to a ROOT global slot once (see
  /// `JitInfo::global_slot_cache`'s own docs -- a qualified-module
  /// resolution never populates this cache, so those always take the
  /// helper path below), every later execution reads the slot straight
  /// out of `VM::global_slots` with two loads and an add, no helper
  /// call, no name lookup at all -- this is what makes a self-recursive
  /// top-level call (`fib` referencing itself, the single most common
  /// hot pattern in real recursive code) cost the same as reading an
  /// already-resolved local instead of paying a full FFI call on every
  /// single reference.
  ///
  /// `flush_live`/`mark_stale_live` follow the exact same unconditional,
  /// outside-the-branch discipline `emit_safepoint`/`emit_is_falsey` use
  /// and for the identical reason: the helper call in `miss_block` is
  /// only actually reached the FIRST time this instruction ever runs,
  /// but compile-time bookkeeping can't know that in advance, so the
  /// conservative flush has to happen unconditionally before the branch,
  /// not bundled inside the block that happens to call the helper. Uses
  /// `call_helper_raw` (not `call_checked`) for exactly the same reason
  /// `emit_safepoint`'s `gc_block` does -- avoiding a second, redundant
  /// flush/stale-mark from `call_helper`'s own automatic wrapping.
  fn emit_get_global(&mut self, ip: usize, dst: u8, name_const: u16) {
    let cache_ptr = self.proto.jit.global_slot_cache.as_ptr() as i64;
    let cache_base = self.i64c(cache_ptr);
    let cached = self.fb.ins().load(
      types::I64,
      cranelift_codegen::ir::MemFlagsData::trusted(),
      cache_base,
      (ip as i32) * 8,
    );
    let neg1 = self.i64c(-1);
    let is_hit = self.fb.ins().icmp(IntCC::NotEqual, cached, neg1);

    self.flush_live(ip);

    let hit_block = self.fb.create_block();
    let miss_block = self.fb.create_block();
    let done_block = self.fb.create_block();
    self.fb.ins().brif(is_hit, hit_block, &[], miss_block, &[]);

    self.fb.switch_to_block(hit_block);
    let slots_ptr = self.fb.ins().load(
      types::I64,
      cranelift_codegen::ir::MemFlagsData::trusted(),
      self.vm_param,
      GLOBAL_SLOTS_PTR_CACHE_OFFSET,
    );
    let byte_off = self.fb.ins().imul_imm_s(cached, 8);
    let addr = self.fb.ins().iadd(slots_ptr, byte_off);
    let v = self.fb.ins().load(
      types::I64,
      cranelift_codegen::ir::MemFlagsData::trusted(),
      addr,
      0,
    );
    self.store_reg(dst, v);
    self.fb.ins().jump(done_block, &[]);

    self.fb.switch_to_block(miss_block);
    let base = self.base_param;
    let dst_i = self.idx(dst);
    let func_ptr = self.func_ptr_const();
    let name = self.bake_const(name_const);
    let ip_c = self.u64c(ip as u64);
    let status = self.call_helper_raw(
      "zuri_jit_get_global",
      &[self.vm_param, base, dst_i, func_ptr, name, ip_c],
    );
    let zero = self.i64c(0);
    let is_err = self.fb.ins().icmp(IntCC::NotEqual, status, zero);
    let err_block = self.fb.create_block();
    let ok_block = self.fb.create_block();
    self.fb.ins().brif(is_err, err_block, &[], ok_block, &[]);

    self.fb.switch_to_block(err_block);
    let junk = self.i64c(0);
    self.fb.ins().return_(&[junk]);

    self.fb.switch_to_block(ok_block);
    self.refresh_regs();
    self.resync_dst_from_memory(dst);
    self.fb.ins().jump(done_block, &[]);

    self.fb.switch_to_block(done_block);
    self.mark_stale_live(ip);
  }

  /// `Instr::GetField`/`SetField`'s self-field fast-path eligibility
  /// check: `obj` must be register 0 (`self`) AND the field name must
  /// be one `VM::resolve_self_field_slots` proved safe on this method's
  /// own class (see that function's own docs for exactly what's
  /// proven). Resolves the name via `proto.chunk.constants` directly --
  /// a compile-time lookup, not `bake_const`'s runtime-immediate one --
  /// since this only needs to know the STRING to check the map, never
  /// hands the name itself to generated code.
  fn self_field_slot(&self, obj: u8, name_const: u16) -> Option<u16> {
    if obj != 0 {
      return None;
    }
    let name = self.proto.chunk.constants[name_const as usize];
    self.self_field_slots.get(name.as_str()).copied()
  }

  /// `self.field` read fast path for a field PROVEN (see
  /// `self_field_slot`) to live at a fixed slot on `self`'s own class,
  /// with no `BoundMethod`-wrapping risk. No helper call, no `RefCell`
  /// borrow, no hashmap probe on the common path: a direct load at
  /// `object::obj_instance_fields_ptr_offset()` (fixed since
  /// `ObjInstance`/`FieldStorage` are `#[repr(C)]`) plus `slot * 8`.
  /// Falls back to the ordinary `zuri_jit_get_field` helper on the
  /// defensive (should be unreachable in practice -- a method's `self`
  /// is always the instance it was invoked on -- but checked rather
  /// than assumed) case that register 0 doesn't actually hold an
  /// `Obj::Instance` right now.
  ///
  /// Follows `emit_binary_numeric_guarded`'s exact snapshot/restore
  /// discipline around the fast/slow split -- see
  /// `restore_dirty_from_snapshot`'s own docs for the real bug class
  /// that protects against (Cranelift compiles both arms unconditionally,
  /// so the slow arm's `call_checked` would otherwise corrupt this
  /// compiler's OWN compile-time liveness bookkeeping for registers this
  /// instruction never touches, even when the slow arm never runs at
  /// runtime).
  fn emit_self_get_field(&mut self, ip: usize, dst: u8, obj: u8, name_const: u16, slot: u16) {
    let self_val = self.load_reg(obj);
    let is_obj = self.is_obj(self_val);
    let snapshot = self.snapshot_reg_cache();

    let obj_block = self.fb.create_block();
    let slow_block = self.fb.create_block();
    let done_block = self.fb.create_block();
    self.fb.ins().brif(is_obj, obj_block, &[], slow_block, &[]);

    // `ptr`/`tag` are only ever computed/dereferenced INSIDE this
    // block, proven reachable only when `is_obj` was true -- see
    // `is_obj`'s own docs. Computing either unconditionally (e.g. via
    // a plain boolean AND instead of a real branch) would mean
    // dereferencing a masked-bits "pointer" for a nil/bool/number
    // value, which is NOT a valid address and can fault.
    self.fb.switch_to_block(obj_block);
    let ptr = self.obj_ptr(self_val);
    let tag = self.obj_tag(ptr);
    let tag_instance = self.i64c(object::OBJ_TAG_INSTANCE as i64);
    let is_instance = self.fb.ins().icmp(IntCC::Equal, tag, tag_instance);
    let fast_block = self.fb.create_block();
    self
      .fb
      .ins()
      .brif(is_instance, fast_block, &[], slow_block, &[]);

    self.fb.switch_to_block(fast_block);
    let fields_ptr = self.load_instance_fields_ptr(ptr);
    let v = self.fb.ins().load(
      types::I64,
      cranelift_codegen::ir::MemFlagsData::trusted(),
      fields_ptr,
      (slot as i32) * 8,
    );
    self.store_reg(dst, v);
    self.fb.ins().jump(done_block, &[]);

    self.fb.switch_to_block(slow_block);
    let base = self.base_param;
    let dst_i = self.idx(dst);
    let obj_i = self.idx(obj);
    let name = self.bake_const(name_const);
    let func_ptr = self.func_ptr_const();
    let ip_c = self.u64c(ip as u64);
    self.call_checked(
      "zuri_jit_get_field",
      &[self.vm_param, base, dst_i, obj_i, name, func_ptr, ip_c],
    );
    self.resync_dst_from_memory(dst);
    self.fb.ins().jump(done_block, &[]);

    self.fb.switch_to_block(done_block);
    self.restore_dirty_from_snapshot(&snapshot, dst);
  }

  /// `self.field = ...` write fast path -- the write-side counterpart
  /// of `emit_self_get_field`. Defines no VM register at all (only
  /// reads `src`), so nothing is held back from the merged restore; the
  /// receiver is reconciled across the two arms by
  /// `resync_receiver_from_memory` instead, exactly as in
  /// `emit_ic_set_field` -- see its docs for the disagreement that
  /// closes. Only one `call_helper` site exists in this function (the
  /// slow path), so the OTHER real bug class this file's
  /// snapshot/restore machinery guards against -- two INDEPENDENT
  /// `call_helper` sites in different branches, see
  /// `emit_list_set_index`'s own docs -- doesn't apply here.
  fn emit_self_set_field(&mut self, ip: usize, obj: u8, name_const: u16, src: u8, slot: u16) {
    let self_val = self.load_reg(obj);
    let src_val = self.load_reg(src);
    let is_obj = self.is_obj(self_val);
    let snapshot = self.snapshot_reg_cache();

    let obj_block = self.fb.create_block();
    let slow_block = self.fb.create_block();
    let done_block = self.fb.create_block();
    self.fb.ins().brif(is_obj, obj_block, &[], slow_block, &[]);

    // See `emit_self_get_field`'s identical two-stage branch for why
    // `ptr`/`tag` must only ever be computed inside a block already
    // proven reachable only when `is_obj` was true.
    self.fb.switch_to_block(obj_block);
    let ptr = self.obj_ptr(self_val);
    let tag = self.obj_tag(ptr);
    let tag_instance = self.i64c(object::OBJ_TAG_INSTANCE as i64);
    let is_instance = self.fb.ins().icmp(IntCC::Equal, tag, tag_instance);
    let fast_block = self.fb.create_block();
    self
      .fb
      .ins()
      .brif(is_instance, fast_block, &[], slow_block, &[]);

    self.fb.switch_to_block(fast_block);
    let fields_ptr = self.load_instance_fields_ptr(ptr);
    self.fb.ins().store(
      cranelift_codegen::ir::MemFlagsData::trusted(),
      src_val,
      fields_ptr,
      (slot as i32) * 8,
    );
    self.emit_write_barrier(ptr);
    self.fb.ins().jump(done_block, &[]);

    self.fb.switch_to_block(slow_block);
    let base = self.base_param;
    let obj_i = self.idx(obj);
    let name = self.bake_const(name_const);
    let src_i = self.idx(src);
    let func_ptr = self.func_ptr_const();
    let ip_c = self.u64c(ip as u64);
    self.call_checked(
      "zuri_jit_set_field",
      &[self.vm_param, base, obj_i, name, src_i, func_ptr, ip_c],
    );
    self.resync_receiver_from_memory(obj);
    self.fb.ins().jump(done_block, &[]);

    self.fb.switch_to_block(done_block);
    self.restore_dirty_from_snapshot_all(&snapshot);
  }

  /// The address of this instruction's own `chunk::FieldCacheCell`,
  /// baked as an immediate -- `None` when the chunk has no cell for
  /// this position (see `Chunk::field_cache_cell`), in which case the
  /// caller emits the plain helper call instead.
  ///
  /// Sound for the same reason `func_ptr_const` is: the cell lives in a
  /// `Box<[FieldCacheCell]>` the chunk allocates exactly once and never
  /// resizes, owned by the `ObjFunction` this IS the compilation of --
  /// so the address is fixed for at least as long as the code being
  /// generated here can run.
  fn field_cache_addr(&mut self, ip: usize) -> Option<IrValue> {
    let addr = self.proto.chunk.field_cache_cell(ip)? as *const _ as u64;
    Some(self.u64c(addr))
  }

  /// Emits the shared front half of both inline-cache field fast paths:
  /// branch to `slow_block` unless `recv` is an `Obj::Instance` whose
  /// class bit-matches this site's cached class. Returns the raw
  /// `*const Obj` and the cached byte offset, valid only in the block
  /// that is current on return (the cache-hit block).
  ///
  /// The three checks are three SEPARATE branches, not one fused
  /// boolean: each stage may only be evaluated once the previous one
  /// has proven it safe to. Masking a nil/number/bool `Value`'s bits
  /// into a "pointer" and loading its tag would fault, and loading a
  /// non-instance `Obj`'s bytes as an `ObjInstance` would read the
  /// wrong union arm -- see `emit_self_get_field`'s identical two-stage
  /// discipline, which this extends by one stage.
  fn emit_ic_guard(
    &mut self,
    recv: IrValue,
    cache_addr: IrValue,
    slow_block: Block,
  ) -> (IrValue, IrValue) {
    let is_obj = self.is_obj(recv);
    let obj_block = self.fb.create_block();
    self.fb.ins().brif(is_obj, obj_block, &[], slow_block, &[]);

    self.fb.switch_to_block(obj_block);
    let ptr = self.obj_ptr(recv);
    let tag = self.obj_tag(ptr);
    let tag_instance = self.i64c(object::OBJ_TAG_INSTANCE as i64);
    let is_instance = self.fb.ins().icmp(IntCC::Equal, tag, tag_instance);
    let class_block = self.fb.create_block();
    self
      .fb
      .ins()
      .brif(is_instance, class_block, &[], slow_block, &[]);

    self.fb.switch_to_block(class_block);
    let flags = cranelift_codegen::ir::MemFlagsData::trusted();
    let class_off = object::obj_instance_class_offset() as i32;
    let class_bits = self.fb.ins().load(types::I64, flags, ptr, class_off);
    let cached_class = self.fb.ins().load(types::I64, flags, cache_addr, 0);
    let same_class = self.fb.ins().icmp(IntCC::Equal, class_bits, cached_class);
    let hit_block = self.fb.create_block();
    self
      .fb
      .ins()
      .brif(same_class, hit_block, &[], slow_block, &[]);

    self.fb.switch_to_block(hit_block);
    let byte_offset = self.fb.ins().load(types::I64, flags, cache_addr, 8);
    (ptr, byte_offset)
  }

  /// `obj.field` read fast path for an ARBITRARY receiver register,
  /// backed by this site's own monomorphic inline cache (see
  /// `Chunk::field_cache`). The generalization of `emit_self_get_field`,
  /// which handles the one case -- `self.field` inside a method of the
  /// owning class -- where the slot is provable at compile time and
  /// needs no cache or class guard at all; that path stays separate and
  /// is always preferred, since it is strictly cheaper.
  ///
  /// This is what takes field-heavy numeric code off the
  /// `zuri_jit_get_field` helper entirely: on a hit it is three
  /// dependent loads and three not-taken branches, with no call, no
  /// `RefCell` borrow, no hash probe, and -- crucially -- no
  /// `flush_live`/`mark_stale_live` round trip forcing every live
  /// register back through memory.
  ///
  /// Every non-instance receiver (a class's statics, a module member, a
  /// dict key, a method being read as a bound method) and every cache
  /// miss falls through to the unchanged helper, which is still the
  /// only implementation of those cases and is also what FILLS the
  /// cache for the next time round.
  fn emit_ic_get_field(&mut self, ip: usize, dst: u8, obj: u8, name_const: u16, cache: IrValue) {
    let recv = self.load_reg(obj);
    let snapshot = self.snapshot_reg_cache();

    let slow_block = self.fb.create_block();
    let done_block = self.fb.create_block();

    let (ptr, byte_offset) = self.emit_ic_guard(recv, cache, slow_block);
    let fields_ptr = self.load_instance_fields_ptr(ptr);
    let addr = self.fb.ins().iadd(fields_ptr, byte_offset);
    let v = self.fb.ins().load(
      types::I64,
      cranelift_codegen::ir::MemFlagsData::trusted(),
      addr,
      0,
    );
    self.store_reg(dst, v);
    self.fb.ins().jump(done_block, &[]);

    self.fb.switch_to_block(slow_block);
    let base = self.base_param;
    let dst_i = self.idx(dst);
    let obj_i = self.idx(obj);
    let name = self.bake_const(name_const);
    let func_ptr = self.func_ptr_const();
    let ip_c = self.u64c(ip as u64);
    self.call_checked(
      "zuri_jit_get_field",
      &[self.vm_param, base, dst_i, obj_i, name, func_ptr, ip_c],
    );
    self.resync_dst_from_memory(dst);
    self.resync_receiver_from_memory(obj);
    self.fb.ins().jump(done_block, &[]);

    self.fb.switch_to_block(done_block);
    self.restore_dirty_from_snapshot(&snapshot, dst);
  }

  /// Re-reads a fast-path field access's RECEIVER register from memory
  /// into its `Variable`, on the slow arm only, and marks it `Dirty` --
  /// what lets both arms of these instructions agree on one honest
  /// compile-time cache state for `obj`.
  ///
  /// Without it the two arms disagree irreconcilably. The slow arm's
  /// helper call marks every live register `Stale` ("memory is
  /// authoritative, re-read it"), which is true THERE because the call
  /// flushed first; the fast arm never flushes anything, so for it
  /// `Stale` is a lie and the next `load_reg(obj)` reads whatever
  /// happened to be in that register slot several instructions ago.
  /// That is not hypothetical: `bodies[i].y` in `benchmarks/nbody.zu`
  /// read back the `bodies` LIST -- the value the `GetGlobal` feeding
  /// the `GetIndex` had left in memory -- and raised a `TypeError`
  /// naming it.
  ///
  /// Resyncing here makes the `Variable` authoritative on BOTH arms
  /// (fast: never invalidated; slow: freshly re-read, so it also picks
  /// up any relocation a collection inside the helper performed), so
  /// `Dirty` is the correct merged state and `restore_dirty_from_snapshot`
  /// can treat the receiver like any other register. Costs one load,
  /// on the slow path only.
  fn resync_receiver_from_memory(&mut self, obj: u8) {
    let v = self.load_reg_mem(obj);
    self.store_reg(obj, v);
  }

  /// `obj.field = ...` write fast path -- `emit_ic_get_field`'s
  /// counterpart, with the same guard chain plus the write barrier
  /// every field mutation owes (see `emit_write_barrier`).
  ///
  /// Defines no register, so nothing is held back from the merged
  /// restore -- the receiver included, since `resync_receiver_from_memory`
  /// has already made its `Variable` authoritative on both arms.
  fn emit_ic_set_field(&mut self, ip: usize, obj: u8, name_const: u16, src: u8, cache: IrValue) {
    let recv = self.load_reg(obj);
    let src_val = self.load_reg(src);
    let snapshot = self.snapshot_reg_cache();

    let slow_block = self.fb.create_block();
    let done_block = self.fb.create_block();

    let (ptr, byte_offset) = self.emit_ic_guard(recv, cache, slow_block);
    let fields_ptr = self.load_instance_fields_ptr(ptr);
    let addr = self.fb.ins().iadd(fields_ptr, byte_offset);
    self.fb.ins().store(
      cranelift_codegen::ir::MemFlagsData::trusted(),
      src_val,
      addr,
      0,
    );
    self.emit_write_barrier(ptr);
    self.fb.ins().jump(done_block, &[]);

    self.fb.switch_to_block(slow_block);
    let base = self.base_param;
    let obj_i = self.idx(obj);
    let name = self.bake_const(name_const);
    let src_i = self.idx(src);
    let func_ptr = self.func_ptr_const();
    let ip_c = self.u64c(ip as u64);
    self.call_checked(
      "zuri_jit_set_field",
      &[self.vm_param, base, obj_i, name, src_i, func_ptr, ip_c],
    );
    self.resync_receiver_from_memory(obj);
    self.fb.ins().jump(done_block, &[]);

    self.fb.switch_to_block(done_block);
    self.restore_dirty_from_snapshot_all(&snapshot);
  }

  /// `Instr::Invoke`'s ordinary codegen: the compiled-method fast call
  /// when `self_invoke_target` proves the receiver's class resolves
  /// this name to the method being compiled, otherwise the general
  /// inline-cache-style `zuri_jit_invoke_prepare` path. Extracted so
  /// `emit_number_intrinsic` can reuse it verbatim as its own guard's
  /// slow arm.
  ///
  /// Does NOT emit the safepoint the `Instr::Invoke` arm owes -- callers
  /// place that themselves, since the intrinsic path deliberately skips
  /// it (see `emit_number_intrinsic`).
  fn emit_generic_invoke(&mut self, ip: usize, dst: u8, obj: u8, method_const: u16, num_args: u8) {
    if let Some((class_bits, generation)) = self.self_invoke_target(method_const) {
      self.emit_self_invoke(ip, dst, obj, method_const, num_args, class_bits, generation);
      return;
    }
    let base = self.base_param;
    let vm_p = self.vm_param;
    let obj_i = self.idx(obj);
    let num_args_i = self.idx(num_args);
    let dst_i = self.idx(dst);
    let name = self.bake_const(method_const);
    let func_ptr = self.func_ptr_const();
    let ip_c = self.u64c(ip as u64);
    let new_base = self.fb.ins().iadd_imm_s(base, obj as i64 + 1);
    self.emit_fast_call(
      "zuri_jit_invoke_prepare",
      &[vm_p, base, obj_i, num_args_i, dst_i, name, func_ptr, ip_c],
      new_base,
      dst,
      "zuri_jit_invoke",
      &[vm_p, base, obj_i, num_args_i, dst_i, name],
    );
  }

  /// The method name an `Instr::Invoke` names, read straight out of the
  /// constant table at compile time -- a compile-time lookup only, like
  /// `self_field_slot`'s, never handed to generated code.
  fn method_name(&self, method_const: u16) -> &str {
    self.proto.chunk.constants[method_const as usize].as_str()
  }

  /// `n.sqrt()` and friends, compiled to the single machine instruction
  /// they are, instead of a method dispatch.
  ///
  /// Nothing about this is speculative. A `Value` that `is_number()`
  /// has no class, no fields and no user-reachable method table: its
  /// `Instr::Invoke` resolution always ends at
  /// `builtins::lookup` -> `NUMBER_METHODS`, a `LazyLock` static with
  /// no mutation API anywhere in the language, so on a receiver proven
  /// (or guarded) numeric, `sqrt` IS `f64::sqrt` and cannot be anything
  /// else. Each intrinsic below is the IEEE-754 operation its
  /// `builtins::number` counterpart calls, so results are bit-identical
  /// to the interpreter's, not merely close.
  ///
  /// `round` is deliberately absent: Rust's `f64::round` breaks ties
  /// away from zero, while Cranelift's `nearest` is IEEE
  /// round-half-to-even. They disagree on exact halves, so it is not
  /// the same function and does not belong here.
  fn emit_number_intrinsic(
    &mut self,
    ip: usize,
    dst: u8,
    obj: u8,
    method_const: u16,
    num_args: u8,
    op: NumberIntrinsic,
  ) {
    // No `emit_safepoint` on this path, unlike every other `Invoke`:
    // nothing an intrinsic emits can allocate, so there is nothing for
    // a collection to be owed here. Every loop's own back edge still
    // carries a safepoint (see `Instr::Jmp`/`JmpIfFalse`/`JmpIfTrue`),
    // so GC progress in a loop whose only call is an intrinsified one
    // is still guaranteed.
    let recv = self.load_reg(obj);
    // A one-argument intrinsic's argument sits at `obj + 2` -- `obj + 1`
    // holds the duplicated receiver the closure-call convention needs,
    // which an intrinsic bypasses. Matches `runtime::invoke_native_args`
    // exactly.
    let arg_reg = obj + 2;
    let arg = (num_args == 1).then(|| self.load_reg(arg_reg));

    // Both the receiver AND (for the binary forms) the argument must be
    // numbers before any of this is the right answer: `builtins::number`
    // enforces the argument's type and RAISES on a mismatch, so a
    // non-numeric argument has to reach the real dispatch to get the
    // real error.
    let mut guard_needed = !self.proven_numeric(ip, obj);
    if num_args == 1 && !self.proven_numeric(ip, arg_reg) {
      guard_needed = true;
    }

    if !guard_needed {
      let v = self.emit_intrinsic_value(op, recv, arg);
      self.store_reg(dst, v);
      return;
    }

    let mut guard = self.is_number(recv);
    if let Some(a) = arg {
      let arg_is_num = self.is_number(a);
      guard = self.fb.ins().band(guard, arg_is_num);
    }

    let snapshot = self.snapshot_reg_cache();
    let fast_block = self.fb.create_block();
    let slow_block = self.fb.create_block();
    let done_block = self.fb.create_block();
    self.fb.ins().brif(guard, fast_block, &[], slow_block, &[]);

    self.fb.switch_to_block(fast_block);
    let v = self.emit_intrinsic_value(op, recv, arg);
    self.store_reg(dst, v);
    self.fb.ins().jump(done_block, &[]);

    // The full ordinary dispatch, safepoint included, for a receiver
    // that turned out not to be a number after all -- a string, a list,
    // an instance whose class happens to declare a method by this name.
    self.fb.switch_to_block(slow_block);
    self.emit_safepoint();
    self.emit_generic_invoke(ip, dst, obj, method_const, num_args);
    // Both arms must leave the same story behind: the fast arm defines
    // `dst` in its `Variable` and writes no memory, while everything on
    // the slow arm flushed and stale-marked both `dst` and the
    // receiver. Re-reading them here makes the `Variable` authoritative
    // either way, so the merged `Dirty` state below is honest -- see
    // `resync_receiver_from_memory`'s own docs for the bug the
    // alternative produces.
    self.resync_dst_from_memory(dst);
    self.resync_receiver_from_memory(obj);
    self.fb.ins().jump(done_block, &[]);

    self.fb.switch_to_block(done_block);
    self.restore_dirty_from_snapshot(&snapshot, dst);
  }

  /// The intrinsic itself, on operands already known to be numbers --
  /// no branching, no register bookkeeping, just the value.
  fn emit_intrinsic_value(
    &mut self,
    op: NumberIntrinsic,
    recv: IrValue,
    arg: Option<IrValue>,
  ) -> IrValue {
    match op {
      NumberIntrinsic::Inline(inline) => {
        let f = self.to_f64(recv);
        let r = inline.emit(&mut self.fb, f);
        self.from_f64(r)
      },
      NumberIntrinsic::InlinePredicate(pred) => {
        let f = self.to_f64(recv);
        let cond = match pred {
          PredicateOp::IsNan => self.fb.ins().fcmp(FloatCC::NotEqual, f, f),
          PredicateOp::IsInf => {
            let mag = self.fb.ins().fabs(f);
            let inf = self.fb.ins().f64const(f64::INFINITY);
            self.fb.ins().fcmp(FloatCC::Equal, mag, inf)
          },
          PredicateOp::IsFinite => {
            let mag = self.fb.ins().fabs(f);
            let inf = self.fb.ins().f64const(f64::INFINITY);
            self.fb.ins().fcmp(FloatCC::LessThan, mag, inf)
          },
          PredicateOp::NonNegative => {
            let zero = self.fb.ins().f64const(0.0);
            self.fb.ins().fcmp(FloatCC::GreaterThanOrEqual, f, zero)
          },
        };
        self.bool_value(cond)
      },
      NumberIntrinsic::Sign => {
        let f = self.to_f64(recv);
        let zero = self.fb.ins().f64const(0.0);
        let one = self.fb.ins().f64const(1.0);
        let minus_one = self.fb.ins().f64const(-1.0);
        let is_zero = self.fb.ins().fcmp(FloatCC::Equal, f, zero);
        let is_pos = self.fb.ins().fcmp(FloatCC::GreaterThan, f, zero);
        // `f` itself, not a fresh `0.0`, for the zero case: that is
        // what preserves `-0.0`'s sign, which is the whole reason
        // `builtins::number::sign` is not `f64::signum`.
        let nonzero = self.fb.ins().select(is_pos, one, minus_one);
        let r = self.fb.ins().select(is_zero, f, nonzero);
        self.from_f64(r)
      },
      NumberIntrinsic::Int => {
        let f = self.to_f64(recv);
        let i = self.fb.ins().fcvt_to_sint_sat(types::I64, f);
        let r = self.fb.ins().fcvt_from_sint(types::F64, i);
        self.from_f64(r)
      },
      NumberIntrinsic::Call { helper, .. } => {
        let vm = self.vm_param;
        match arg {
          Some(a) => self.call_helper_raw(helper, &[vm, recv, a]),
          None => self.call_helper_raw(helper, &[vm, recv]),
        }
      },
    }
  }

  /// `object::write_barrier`'s own guard, inlined -- owed after EVERY
  /// write into an already-live instance's field storage, since a
  /// generational minor collection finds old->young pointers only
  /// through the remembered set this maintains (see `write_barrier`'s
  /// own docs). `obj_ptr` is the raw `*const Obj` the write went
  /// through, already proven to be an `Obj::Instance` by the caller's
  /// own tag check.
  ///
  /// The guard is two byte loads and a branch rather than an
  /// unconditional call because the answer at a hot mutation site is
  /// almost always "nothing owed": either the object is still young
  /// (young objects are rescanned wholesale every minor collection, so
  /// no remembered-set entry is needed at all), or it is old and some
  /// earlier write this cycle already queued it. Only the genuinely
  /// rare first-write-to-an-old-object case calls out.
  ///
  /// Reached through `call_helper_raw`, NOT `call_helper`: this helper
  /// touches no VM register, cannot allocate, collect, or raise, so
  /// bracketing it with `flush_live`/`mark_stale_live` would invalidate
  /// this compiler's whole register cache on every field write for no
  /// reason -- exactly the cost the inline fast path exists to avoid.
  fn emit_write_barrier(&mut self, obj_ptr: IrValue) {
    let flags = cranelift_codegen::ir::MemFlagsData::trusted();
    let gen_off = object::obj_to_gcbox_generation_offset();
    let rem_off = object::obj_to_gcbox_remembered_offset();
    let generation = self.fb.ins().load(types::I8, flags, obj_ptr, gen_off);
    let remembered = self.fb.ins().load(types::I8, flags, obj_ptr, rem_off);
    let old = self
      .fb
      .ins()
      .iconst(types::I8, object::GENERATION_OLD_BYTE as i64);
    let is_old = self.fb.ins().icmp(IntCC::Equal, generation, old);
    let zero = self.fb.ins().iconst(types::I8, 0);
    let fresh = self.fb.ins().icmp(IntCC::Equal, remembered, zero);
    let owed = self.fb.ins().band(is_old, fresh);

    let barrier_block = self.fb.create_block();
    let done_block = self.fb.create_block();
    self
      .fb
      .ins()
      .brif(owed, barrier_block, &[], done_block, &[]);

    self.fb.switch_to_block(barrier_block);
    let vm = self.vm_param;
    self.call_helper_raw("zuri_jit_write_barrier", &[vm, obj_ptr]);
    self.fb.ins().jump(done_block, &[]);

    self.fb.switch_to_block(done_block);
  }

  /// Loads an `Obj::Instance`'s `fields` slice base pointer, given a
  /// raw `*const Obj` already proven (by a runtime tag check against
  /// `OBJ_TAG_INSTANCE`, in a block only reachable when `is_obj` was
  /// ALSO already proven true -- see `emit_self_get_field`'s two-stage
  /// branch) to actually be one -- see
  /// `object::obj_instance_fields_ptr_offset()`'s own docs for why this
  /// fixed offset is sound.
  fn load_instance_fields_ptr(&mut self, obj_ptr: IrValue) -> IrValue {
    let off = object::obj_instance_fields_ptr_offset() as i32;
    self.fb.ins().load(
      types::I64,
      cranelift_codegen::ir::MemFlagsData::trusted(),
      obj_ptr,
      off,
    )
  }

  /// `Instr::GetIndex`'s fast path for `list[i]` with a plain, already
  /// in-bounds integer index. Falls back to the general
  /// `zuri_jit_get_index` helper for everything else it doesn't prove
  /// inline: a non-list receiver (Bytes/String/Dict indexing all still
  /// go through the general path), a non-numeric or non-integer index,
  /// or a genuinely out-of-bounds index (needs the general path's real
  /// `RangeError` message). See `zuri_jit_list_data`'s own docs for why
  /// resolving the list's data pointer still costs one small helper
  /// call while the bounds check and element load are real inline
  /// Cranelift code either way.
  fn emit_list_get_index(&mut self, dst: u8, obj: u8, iidx: u8) {
    let obj_val = self.load_reg(obj);
    let idx_val = self.load_reg(iidx);
    // Both safe to compute unconditionally regardless of the other's
    // truth value -- neither dereferences memory, see `is_obj`/
    // `is_number`'s own docs.
    let is_obj = self.is_obj(obj_val);
    let is_num = self.is_number(idx_val);
    let cheap_guard = self.fb.ins().band(is_obj, is_num);
    let snapshot = self.snapshot_reg_cache();

    let checked_block = self.fb.create_block();
    let slow_block = self.fb.create_block();
    let done_block = self.fb.create_block();
    self
      .fb
      .ins()
      .brif(cheap_guard, checked_block, &[], slow_block, &[]);

    // `ptr`/`tag` (dereferences memory) and the float round-trip check
    // (pure arithmetic, but only MEANINGFUL once `is_num` is known
    // true) are both only computed here, in a block reachable only
    // when `cheap_guard` -- and therefore `is_obj` -- was already
    // proven true. Same discipline `emit_self_get_field` uses for its
    // own tag check.
    self.fb.switch_to_block(checked_block);
    let ptr = self.obj_ptr(obj_val);
    let tag = self.obj_tag(ptr);
    let tag_list = self.i64c(object::OBJ_TAG_LIST as i64);
    let is_list = self.fb.ins().icmp(IntCC::Equal, tag, tag_list);

    let f = self.to_f64(idx_val);
    let as_int = self.fb.ins().fcvt_to_sint_sat(types::I64, f);
    let roundtrip = self.fb.ins().fcvt_from_sint(types::F64, as_int);
    let is_int = self.fb.ins().fcmp(
      cranelift_codegen::ir::condcodes::FloatCC::Equal,
      f,
      roundtrip,
    );
    let list_and_int = self.fb.ins().band(is_list, is_int);

    let resolve_block = self.fb.create_block();
    self
      .fb
      .ins()
      .brif(list_and_int, resolve_block, &[], slow_block, &[]);

    self.fb.switch_to_block(resolve_block);
    let len_slot = self.list_len_slot();
    let len_addr = self.fb.ins().stack_addr(types::I64, len_slot, 0);
    let data_ptr = self.call_helper("zuri_jit_list_data", &[self.vm_param, ptr, len_addr]);
    let len = self.fb.ins().load(
      types::I64,
      cranelift_codegen::ir::MemFlagsData::trusted(),
      len_addr,
      0,
    );
    // Negative-index wraparound (`list[-1]` == last element), matching
    // `VM::coerce_index`'s own semantics exactly.
    let zero = self.fb.ins().iconst(types::I64, 0);
    let is_neg = self.fb.ins().icmp(IntCC::SignedLessThan, as_int, zero);
    let wrapped = self.fb.ins().iadd(as_int, len);
    let i_adj = self.fb.ins().select(is_neg, wrapped, as_int);

    let ge_zero = self
      .fb
      .ins()
      .icmp(IntCC::SignedGreaterThanOrEqual, i_adj, zero);
    let lt_len = self.fb.ins().icmp(IntCC::SignedLessThan, i_adj, len);
    let in_bounds = self.fb.ins().band(ge_zero, lt_len);

    let fast_block = self.fb.create_block();
    self
      .fb
      .ins()
      .brif(in_bounds, fast_block, &[], slow_block, &[]);

    self.fb.switch_to_block(fast_block);
    let byte_off = self.fb.ins().imul_imm_s(i_adj, 8);
    let elem_addr = self.fb.ins().iadd(data_ptr, byte_off);
    let v = self.fb.ins().load(
      types::I64,
      cranelift_codegen::ir::MemFlagsData::trusted(),
      elem_addr,
      0,
    );
    self.store_reg(dst, v);
    self.fb.ins().jump(done_block, &[]);

    // See `emit_list_set_index`'s own docs on why this reset (not just
    // the `restore_dirty_from_snapshot` at `done_block`) is needed:
    // `resolve_block`'s own `call_helper` above already mutated
    // `reg_cache` as a side effect of being GENERATED, regardless of
    // whether it ever runs at runtime -- without resetting back to the
    // true pre-instruction state here, `slow_block`'s own
    // `call_checked` would see nothing left to flush and silently emit
    // no flush instructions at all, even on the (here, only) runtime
    // path where IT is the one that actually needs to.
    self.reg_cache = snapshot.clone();
    self.fb.switch_to_block(slow_block);
    let base = self.base_param;
    let dst_i = self.idx(dst);
    let obj_i = self.idx(obj);
    let idx_i = self.idx(iidx);
    self.call_checked(
      "zuri_jit_get_index",
      &[self.vm_param, base, dst_i, obj_i, idx_i],
    );
    self.resync_dst_from_memory(dst);
    self.fb.ins().jump(done_block, &[]);

    self.fb.switch_to_block(done_block);
    self.restore_dirty_from_snapshot(&snapshot, dst);
  }

  /// `Instr::SetIndex`'s fast path -- the write-side counterpart of
  /// `emit_list_get_index`; see its own docs for the shared reasoning,
  /// including why `reg_cache` needs a hard reset before `slow_block`
  /// (two independent `call_helper` sites: `zuri_jit_list_data` here,
  /// `zuri_jit_set_index` there).
  fn emit_list_set_index(&mut self, obj: u8, iidx: u8, src: u8) {
    let obj_val = self.load_reg(obj);
    let idx_val = self.load_reg(iidx);
    let src_val = self.load_reg(src);
    let is_obj = self.is_obj(obj_val);
    let is_num = self.is_number(idx_val);
    let cheap_guard = self.fb.ins().band(is_obj, is_num);
    let snapshot = self.snapshot_reg_cache();

    let checked_block = self.fb.create_block();
    let slow_block = self.fb.create_block();
    let done_block = self.fb.create_block();
    self
      .fb
      .ins()
      .brif(cheap_guard, checked_block, &[], slow_block, &[]);

    self.fb.switch_to_block(checked_block);
    let ptr = self.obj_ptr(obj_val);
    let tag = self.obj_tag(ptr);
    let tag_list = self.i64c(object::OBJ_TAG_LIST as i64);
    let is_list = self.fb.ins().icmp(IntCC::Equal, tag, tag_list);

    let f = self.to_f64(idx_val);
    let as_int = self.fb.ins().fcvt_to_sint_sat(types::I64, f);
    let roundtrip = self.fb.ins().fcvt_from_sint(types::F64, as_int);
    let is_int = self.fb.ins().fcmp(
      cranelift_codegen::ir::condcodes::FloatCC::Equal,
      f,
      roundtrip,
    );
    let list_and_int = self.fb.ins().band(is_list, is_int);
    let resolve_block = self.fb.create_block();
    self
      .fb
      .ins()
      .brif(list_and_int, resolve_block, &[], slow_block, &[]);

    self.fb.switch_to_block(resolve_block);
    let len_slot = self.list_len_slot();
    let len_addr = self.fb.ins().stack_addr(types::I64, len_slot, 0);
    let data_ptr = self.call_helper("zuri_jit_list_data", &[self.vm_param, ptr, len_addr]);
    let len = self.fb.ins().load(
      types::I64,
      cranelift_codegen::ir::MemFlagsData::trusted(),
      len_addr,
      0,
    );
    let zero = self.fb.ins().iconst(types::I64, 0);
    let is_neg = self.fb.ins().icmp(IntCC::SignedLessThan, as_int, zero);
    let wrapped = self.fb.ins().iadd(as_int, len);
    let i_adj = self.fb.ins().select(is_neg, wrapped, as_int);

    let ge_zero = self
      .fb
      .ins()
      .icmp(IntCC::SignedGreaterThanOrEqual, i_adj, zero);
    let lt_len = self.fb.ins().icmp(IntCC::SignedLessThan, i_adj, len);
    let in_bounds = self.fb.ins().band(ge_zero, lt_len);

    let fast_block = self.fb.create_block();
    self
      .fb
      .ins()
      .brif(in_bounds, fast_block, &[], slow_block, &[]);

    self.fb.switch_to_block(fast_block);
    let byte_off = self.fb.ins().imul_imm_s(i_adj, 8);
    let elem_addr = self.fb.ins().iadd(data_ptr, byte_off);
    self.fb.ins().store(
      cranelift_codegen::ir::MemFlagsData::trusted(),
      src_val,
      elem_addr,
      0,
    );
    self.fb.ins().jump(done_block, &[]);

    // See `restore_dirty_from_snapshot`'s own docs for the established
    // half of this bug class; this is the OTHER half, specific to
    // having more than one independent `call_helper` site across
    // mutually-exclusive branches: `call_helper` mutates `reg_cache`
    // (Dirty -> Stale) as a compile-time SIDE EFFECT of GENERATING its
    // block's code, regardless of whether that block ever runs at
    // runtime. With two such sites, the FIRST one generated "uses up"
    // the Dirty flag at compile time, so the SECOND site's own
    // `flush_live` sees nothing left to flush and emits no store
    // instruction at all -- even when, at runtime, only the SECOND
    // block ever actually executes and the first's flush never ran.
    // Resetting back to the snapshot before generating EACH
    // independent branch (not just restoring once at the very end)
    // means every such branch's own `call_helper` sees the TRUE
    // pre-instruction state and emits exactly the flush it actually
    // needs, independent of what any sibling branch's codegen did.
    self.reg_cache = snapshot.clone();
    self.fb.switch_to_block(slow_block);
    let base = self.base_param;
    let obj_i = self.idx(obj);
    let idx_i = self.idx(iidx);
    let src_i = self.idx(src);
    self.call_checked(
      "zuri_jit_set_index",
      &[self.vm_param, base, obj_i, idx_i, src_i],
    );
    self.fb.ins().jump(done_block, &[]);

    self.fb.switch_to_block(done_block);
    self.restore_dirty_from_snapshot(&snapshot, obj);
  }

  /// Bounded so a single scalar-replaced allocation can't blow up this
  /// compiled function's own native stack usage for a pathological
  /// literal -- matches the spirit (not the letter) of similar caps
  /// elsewhere in this file. Ordinary code overwhelmingly writes small,
  /// fixed-size list literals; anything bigger just falls through to
  /// the general, heap-allocating path unchanged.
  const MAX_SCALAR_LIST_LEN: u8 = 16;

  /// Is `Instr::MakeList{dst, start: _, count}` at `alloc_ip` safe to
  /// scalar-replace -- keep its `count` elements as ordinary `Value`s
  /// in a Cranelift stack slot (see `emit_scalar_make_list`) instead of
  /// a real heap `Obj::List`, with `dst` never materialized as a
  /// tagged `Value` at all?
  ///
  /// Three independent conditions, all required:
  /// - `jit::escape::analyze_one` proves `dst` never escapes this
  ///   function (see that module's docs for exactly what's whitelisted
  ///   -- `GetIndex`/`SetIndex`'s container position already is, with
  ///   no extension needed here).
  /// - No `Instr::Move` ANYWHERE in this function ever reads `dst`.
  ///   `analyze_one`'s own dataflow WOULD correctly follow a `Move`
  ///   (propagating the "doesn't escape" proof to whatever register it
  ///   copies into), but this compiler's OWN codegen-time tracking
  ///   (`scalar_lists`) does NOT independently replicate that
  ///   propagation -- deliberately: reimplementing the same
  ///   reachability/aliasing logic a second time, in a completely
  ///   separate piece of code, is exactly the kind of two-sources-of-
  ///   truth setup that has ALREADY produced one real, silent-
  ///   corruption bug this session (see `emit_list_get_index`'s own
  ///   commit history). Ruling out `Move` entirely, unconditionally
  ///   (not just on paths reachable from `alloc_ip`), is a safe, cheap
  ///   over-approximation instead: `scalar_lists` then only ever needs
  ///   to answer for the EXACT register `analyze_one` already reasoned
  ///   about, with no second analysis to keep in sync. Real code
  ///   essentially never copies a freshly-built temporary list into
  ///   another register before indexing it, so this costs nothing in
  ///   practice.
  /// - `self.speculative_params`/`self.speculative_regs` are BOTH
  ///   `None`, i.e. this compile has no specialized/speculative body at
  ///   all. This is a confirmed-necessary, temporary safety gate, NOT
  ///   a property scalar replacement itself needs: a real, reproduced
  ///   data-corruption bug exists in `emit_speculative_guard`'s (or
  ///   `emit_entry_dispatch`'s) interaction with a `GetIndex` whose
  ///   index isn't a compile-time constant -- a value read via a
  ///   variable list index, once returned from a function that later
  ///   gets a specialized body, silently freezes at a stale value on
  ///   every subsequent call. Reproduced identically with scalar
  ///   replacement disabled entirely (a `Move`-forced real `Obj::List`
  ///   hits the exact same corruption), so this is a PRE-EXISTING bug
  ///   in the speculation machinery itself, not in this feature -- but
  ///   until it's root-caused and fixed there, scalar-replacing a list
  ///   whose reads could feed a speculatively-guarded register would
  ///   inherit the same hazard. Revisit removing this condition once
  ///   that bug is fixed.
  fn scalar_replace_eligible(&self, alloc_ip: usize, dst: u8, count: u8) -> bool {
    if self.speculative_params.is_some() || self.speculative_regs.is_some() {
      return false;
    }
    if count == 0 || count > Self::MAX_SCALAR_LIST_LEN {
      return false;
    }
    for instr in &self.proto.chunk.code {
      if let Instr::Move { src, .. } = instr
        && *src == dst
      {
        return false;
      }
    }
    !escape::analyze_one(self.proto, alloc_ip, None, None).escapes
  }

  /// `Instr::MakeList{dst, start, count}`'s scalar-replaced fast path
  /// (see `scalar_replace_eligible`): copies the `count` source
  /// registers into a fresh Cranelift stack slot -- ordinary `Value`s,
  /// never wrapped in a heap `Obj::List` -- records `dst -> (slot,
  /// count)` in `scalar_lists` for `Instr::GetIndex`/`SetIndex` to
  /// consult, and registers the slot as a GC root.
  ///
  /// Registration happens LAST, strictly after every element slot has
  /// already been written: `VM::jit_scalar_roots`'s whole soundness
  /// argument depends on every slot a GC walk might visit already
  /// holding a valid `Value` (see that field's own docs) -- an
  /// uninitialized stack slot is neither `nil` nor any other valid tag
  /// pattern, and a GC safepoint CAN fire between two ordinary
  /// instructions (a nested call inside one of the source expressions,
  /// for instance), so there is a real window here to get right, not a
  /// theoretical one.
  fn emit_scalar_make_list(&mut self, dst: u8, start: u8, count: u8) {
    let slot = self.fb.create_sized_stack_slot(StackSlotData::new(
      StackSlotKind::ExplicitSlot,
      count as u32 * 8,
      3,
    ));
    for i in 0..count {
      let v = self.load_reg(start + i);
      self
        .fb
        .ins()
        .stack_store(types::I64, v, slot, (i as i32) * 8);
    }
    let addr = self.fb.ins().stack_addr(types::I64, slot, 0);
    let count_c = self.u64c(count as u64);
    self.call_checked("zuri_jit_push_scalar_root", &[self.vm_param, addr, count_c]);
    self.scalar_lists.insert(dst, (slot, count));
  }

  /// `Instr::GetIndex`'s fast path when `obj` is a scalar-replaced
  /// list (`self.scalar_lists`) -- the SAME bounds-check shape as
  /// `emit_list_get_index` (negative-index wraparound, integer-value
  /// round-trip check), just against a COMPILE-TIME-KNOWN `count` and
  /// a directly-addressable stack slot instead of a runtime-resolved
  /// `Obj::List`: no `is_obj`/tag check at all, since `obj` being a key
  /// in `scalar_lists` already proves what it is.
  ///
  /// Still needs `snapshot_reg_cache`/`restore_dirty_from_snapshot`
  /// even though there's only ONE `call_helper` site here (unlike
  /// `emit_list_get_index`'s two) -- the relevant condition for needing
  /// this isn't "how many call_helper sites in this instruction," it's
  /// "does this instruction have a call_helper site that ONLY runs on
  /// a CONDITIONAL branch." `slow_block`'s `flush_live` mutates
  /// `reg_cache` (Dirty -> Clean) the moment its code is GENERATED,
  /// regardless of whether the runtime path taken is fast or slow; if
  /// left unrestored, a LATER instruction's own `flush_live` -- even a
  /// completely unrelated one several instructions later -- would see
  /// a register as already-Clean and skip flushing it, even on a
  /// runtime execution where THIS instruction actually took its fast
  /// path (which never flushes anything) and that register genuinely
  /// is still Dirty. Confirmed by direct reproduction: `tmp[0][0] +
  /// tmp[1][1] + tmp[2][0]` (three separate scalar-list `GetIndex`
  /// sites reading the same `tmp` in one expression) silently returned
  /// a stale value from an EARLIER site's fast-path store once a
  /// LATER site's slow-block flush was skipped this way.
  fn emit_scalar_list_get(&mut self, dst: u8, slot: StackSlot, count: u8, iidx: u8) {
    let idx_val = self.load_reg(iidx);
    let is_num = self.is_number(idx_val);
    let addr = self.fb.ins().stack_addr(types::I64, slot, 0);
    let snapshot = self.snapshot_reg_cache();

    let checked_block = self.fb.create_block();
    let slow_block = self.fb.create_block();
    let done_block = self.fb.create_block();
    self
      .fb
      .ins()
      .brif(is_num, checked_block, &[], slow_block, &[]);

    self.fb.switch_to_block(checked_block);
    let f = self.to_f64(idx_val);
    let as_int = self.fb.ins().fcvt_to_sint_sat(types::I64, f);
    let roundtrip = self.fb.ins().fcvt_from_sint(types::F64, as_int);
    let is_int = self.fb.ins().fcmp(
      cranelift_codegen::ir::condcodes::FloatCC::Equal,
      f,
      roundtrip,
    );

    let len = self.i64c(count as i64);
    let zero = self.fb.ins().iconst(types::I64, 0);
    let is_neg = self.fb.ins().icmp(IntCC::SignedLessThan, as_int, zero);
    let wrapped = self.fb.ins().iadd(as_int, len);
    let i_adj = self.fb.ins().select(is_neg, wrapped, as_int);
    let ge_zero = self
      .fb
      .ins()
      .icmp(IntCC::SignedGreaterThanOrEqual, i_adj, zero);
    let lt_len = self.fb.ins().icmp(IntCC::SignedLessThan, i_adj, len);
    let in_bounds = self.fb.ins().band(ge_zero, lt_len);
    let ok = self.fb.ins().band(is_int, in_bounds);

    let fast_block = self.fb.create_block();
    self.fb.ins().brif(ok, fast_block, &[], slow_block, &[]);

    self.fb.switch_to_block(fast_block);
    let byte_off = self.fb.ins().imul_imm_s(i_adj, 8);
    let elem_addr = self.fb.ins().iadd(addr, byte_off);
    let v = self.fb.ins().load(
      types::I64,
      cranelift_codegen::ir::MemFlagsData::trusted(),
      elem_addr,
      0,
    );
    self.store_reg(dst, v);
    self.fb.ins().jump(done_block, &[]);

    // See this function's own docs: undoes `checked_block`'s (never
    // actually flushing anything, so a no-op here) and, more
    // importantly, whatever an EARLIER instruction's own conditional
    // `call_helper` site left behind, before `slow_block`'s OWN
    // `flush_live` runs -- otherwise it could see a register as
    // already-Clean from a branch that never actually executed.
    self.reg_cache = snapshot.clone();
    self.fb.switch_to_block(slow_block);
    let base = self.base_param;
    let dst_i = self.idx(dst);
    let count_c = self.u64c(count as u64);
    let idx_i = self.idx(iidx);
    self.call_checked(
      "zuri_jit_scalar_get_index",
      &[self.vm_param, base, dst_i, addr, count_c, idx_i],
    );
    // `zuri_jit_scalar_get_index` writes `dst` DIRECTLY to `VM::
    // registers`, bypassing `store_reg`/`reg_vars` entirely -- matches
    // `emit_list_get_index`'s identical slow path, and for the exact
    // same reason: `call_checked`'s automatic `Stale` mark alone isn't
    // enough here, since `fast_block` DID call `store_reg` (a real
    // `def_var`), so without giving THIS block its own `def_var` too,
    // Cranelift's SSA merge at `done_block` would resolve `dst`'s
    // `Variable` to whatever dominating definition existed BEFORE this
    // instruction on this path -- silently stale, not merely absent.
    self.resync_dst_from_memory(dst);
    self.fb.ins().jump(done_block, &[]);

    self.fb.switch_to_block(done_block);
    self.restore_dirty_from_snapshot(&snapshot, dst);
  }

  /// `Instr::SetIndex`'s scalar-replaced fast path -- the write-side
  /// counterpart of `emit_scalar_list_get`; see its own docs for the
  /// shared bounds-check shape and the general reason a snapshot/
  /// restore is needed around ANY conditionally-reached `call_helper`
  /// site, not just ones with two or more sites in one instruction.
  ///
  /// `restore_dirty_from_snapshot`'s excluded register is normally
  /// "this instruction's own freshly-redefined `dst`" (e.g.
  /// `emit_list_set_index` excludes `obj`, since a SET's container
  /// register is the one at risk of caching a since-relocated pointer
  /// that specifically needs to stay `Stale`, not be trusted). There is
  /// no analogous register here at all: `obj`/the list itself is never
  /// a real `Value` in any register in the first place (it's scalar-
  /// replaced), so `src` is passed purely to satisfy the signature --
  /// its own value was already captured into `src_val` before either
  /// branch, so which way its `reg_cache` entry ends up doesn't affect
  /// correctness, only whether a later read of it costs one redundant
  /// reload.
  fn emit_scalar_list_set(&mut self, slot: StackSlot, count: u8, iidx: u8, src: u8) {
    let idx_val = self.load_reg(iidx);
    let src_val = self.load_reg(src);
    let is_num = self.is_number(idx_val);
    let addr = self.fb.ins().stack_addr(types::I64, slot, 0);
    let snapshot = self.snapshot_reg_cache();

    let checked_block = self.fb.create_block();
    let slow_block = self.fb.create_block();
    let done_block = self.fb.create_block();
    self
      .fb
      .ins()
      .brif(is_num, checked_block, &[], slow_block, &[]);

    self.fb.switch_to_block(checked_block);
    let f = self.to_f64(idx_val);
    let as_int = self.fb.ins().fcvt_to_sint_sat(types::I64, f);
    let roundtrip = self.fb.ins().fcvt_from_sint(types::F64, as_int);
    let is_int = self.fb.ins().fcmp(
      cranelift_codegen::ir::condcodes::FloatCC::Equal,
      f,
      roundtrip,
    );

    let len = self.i64c(count as i64);
    let zero = self.fb.ins().iconst(types::I64, 0);
    let is_neg = self.fb.ins().icmp(IntCC::SignedLessThan, as_int, zero);
    let wrapped = self.fb.ins().iadd(as_int, len);
    let i_adj = self.fb.ins().select(is_neg, wrapped, as_int);
    let ge_zero = self
      .fb
      .ins()
      .icmp(IntCC::SignedGreaterThanOrEqual, i_adj, zero);
    let lt_len = self.fb.ins().icmp(IntCC::SignedLessThan, i_adj, len);
    let in_bounds = self.fb.ins().band(ge_zero, lt_len);
    let ok = self.fb.ins().band(is_int, in_bounds);

    let fast_block = self.fb.create_block();
    self.fb.ins().brif(ok, fast_block, &[], slow_block, &[]);

    self.fb.switch_to_block(fast_block);
    let byte_off = self.fb.ins().imul_imm_s(i_adj, 8);
    let elem_addr = self.fb.ins().iadd(addr, byte_off);
    self.fb.ins().store(
      cranelift_codegen::ir::MemFlagsData::trusted(),
      src_val,
      elem_addr,
      0,
    );
    self.fb.ins().jump(done_block, &[]);

    self.reg_cache = snapshot.clone();
    self.fb.switch_to_block(slow_block);
    let base = self.base_param;
    let count_c = self.u64c(count as u64);
    let idx_i = self.idx(iidx);
    let src_i = self.idx(src);
    self.call_checked(
      "zuri_jit_scalar_set_index",
      &[self.vm_param, base, addr, count_c, idx_i, src_i],
    );
    self.fb.ins().jump(done_block, &[]);

    self.fb.switch_to_block(done_block);
    self.restore_dirty_from_snapshot(&snapshot, src);
  }

  /// `Instr::SetGlobal`/`Instr::AssignGlobal`'s inline-cache-style fast
  /// path -- the write-side counterpart of `emit_get_global`, sharing
  /// its cache array (`JitInfo::global_slot_cache`) and the same
  /// unconditional flush/stale-mark discipline around the branch (see
  /// that method's own docs for why). No `resync_dst_from_memory` call
  /// is needed on the miss path here the way `emit_get_global` needs
  /// one for its `dst` -- neither instruction defines a register at
  /// all, only reads `src` and writes to `VM::global_slots`, so there's
  /// no register-`Variable` SSA merge at `done_block` to keep
  /// consistent between the two paths.
  fn emit_set_global(&mut self, ip: usize, src: u8, name_const: u16, slow_helper: &'static str) {
    let cache_ptr = self.proto.jit.global_slot_cache.as_ptr() as i64;
    let cache_base = self.i64c(cache_ptr);
    let cached = self.fb.ins().load(
      types::I64,
      cranelift_codegen::ir::MemFlagsData::trusted(),
      cache_base,
      (ip as i32) * 8,
    );
    let neg1 = self.i64c(-1);
    let is_hit = self.fb.ins().icmp(IntCC::NotEqual, cached, neg1);

    self.flush_live(ip);

    let hit_block = self.fb.create_block();
    let miss_block = self.fb.create_block();
    let done_block = self.fb.create_block();
    self.fb.ins().brif(is_hit, hit_block, &[], miss_block, &[]);

    self.fb.switch_to_block(hit_block);
    let slots_ptr = self.fb.ins().load(
      types::I64,
      cranelift_codegen::ir::MemFlagsData::trusted(),
      self.vm_param,
      GLOBAL_SLOTS_PTR_CACHE_OFFSET,
    );
    let byte_off = self.fb.ins().imul_imm_s(cached, 8);
    let addr = self.fb.ins().iadd(slots_ptr, byte_off);
    let v = self.load_reg(src);
    self
      .fb
      .ins()
      .store(cranelift_codegen::ir::MemFlagsData::trusted(), v, addr, 0);
    self.fb.ins().jump(done_block, &[]);

    self.fb.switch_to_block(miss_block);
    let base = self.base_param;
    let src_i = self.idx(src);
    let func_ptr = self.func_ptr_const();
    let name = self.bake_const(name_const);
    let ip_c = self.u64c(ip as u64);
    let status = self.call_helper_raw(
      slow_helper,
      &[self.vm_param, base, src_i, func_ptr, name, ip_c],
    );
    let zero = self.i64c(0);
    let is_err = self.fb.ins().icmp(IntCC::NotEqual, status, zero);
    let err_block = self.fb.create_block();
    let ok_block = self.fb.create_block();
    self.fb.ins().brif(is_err, err_block, &[], ok_block, &[]);

    self.fb.switch_to_block(err_block);
    let junk = self.i64c(0);
    self.fb.ins().return_(&[junk]);

    self.fb.switch_to_block(ok_block);
    self.refresh_regs();
    self.fb.ins().jump(done_block, &[]);

    self.fb.switch_to_block(done_block);
    self.mark_stale_live(ip);
  }

  // HISTORICAL NOTE, kept so the dead end is not re-explored: a
  // `GetField`/`SetField` inline fast path was built and reverted twice
  // before, both times blamed on a run-to-run FLOATING-POINT result
  // divergence on `benchmarks/nbody.zu` attributed to Cranelift
  // reordering/reassociating the surrounding float arithmetic.
  //
  // That attribution was WRONG -- Cranelift never reassociates floating
  // point, and has no fast-math mode to do it under. The divergence was
  // a stale-register READ, from two independent causes, both fixed and
  // documented at their own sites: `emit_deopt` leaking its `flush_live`
  // bookkeeping out of a branch arm that never runs, and a guarded field
  // access's two arms disagreeing about the receiver register (see
  // `resync_receiver_from_memory`). Both were masked by the old
  // helper-call path's redundant flush on every single field access,
  // which is why removing that flush is what exposed them.
  //
  // The fast path now ships -- see `emit_ic_get_field`/`emit_ic_set_field`
  // for the general receiver and `emit_self_get_field`/`emit_self_set_field`
  // for the compile-time-resolvable `self.field` case.

  // ---------------------------------------------------------------
  // Guards
  // ---------------------------------------------------------------

  /// `(bits & QNAN) != QNAN` -- `Value::is_number()`'s exact bit test
  /// (see `value.rs`), safe to inline because it only ever inspects the
  /// tagged `u64` itself, never a heap object's contents.
  fn is_number(&mut self, v: IrValue) -> IrValue {
    let qnan = self.u64c(value::QNAN);
    let masked = self.fb.ins().band(v, qnan);
    self.fb.ins().icmp(IntCC::NotEqual, masked, qnan)
  }

  fn both_numbers(&mut self, va: IrValue, vb: IrValue) -> IrValue {
    let na = self.is_number(va);
    let nb = self.is_number(vb);
    self.fb.ins().band(na, nb)
  }

  /// `(bits & (QNAN|SIGN_BIT)) == (QNAN|SIGN_BIT)` -- `Value::is_obj()`'s
  /// exact bit test (see `value.rs`). Safe to inline because it only
  /// inspects the tagged `u64` itself, never dereferences anything.
  /// Telling WHICH heap type a confirmed object is needs a real
  /// dereference -- see `obj_ptr`/`obj_tag` for the (now sound, since
  /// `Obj` is `#[repr(C, u8)]`) way to do that inline too.
  fn is_obj(&mut self, v: IrValue) -> IrValue {
    let mask = self.u64c(value::QNAN | value::SIGN_BIT);
    let masked = self.fb.ins().band(v, mask);
    self.fb.ins().icmp(IntCC::Equal, masked, mask)
  }

  /// Recovers the raw `*const Obj` pointer from a tagged `Value` known
  /// (by an already-checked `is_obj`) to actually hold one -- the exact
  /// inverse of `Value::obj`'s own tagging (`SIGN_BIT | QNAN | ptr`),
  /// masking the tag bits back off. Callers must not call this on a
  /// `Value` that hasn't already been proven `is_obj` -- the result is
  /// garbage (though not unsound to COMPUTE; it's only unsound to
  /// DEREFERENCE) otherwise.
  fn obj_ptr(&mut self, v: IrValue) -> IrValue {
    let mask = self.u64c(value::PTR_MASK);
    self.fb.ins().band(v, mask)
  }

  /// Reads `Obj`'s own tag byte straight out of memory -- sound only
  /// because `Obj` is `#[repr(C, u8)]` with an explicit discriminant on
  /// every variant (see that type's own docs), which is what makes
  /// "the tag is a `u8` at offset 0" a real, load-bearing guarantee
  /// instead of an assumption about a layout the compiler is otherwise
  /// free to change. Compare against `object::OBJ_TAG_*` constants, the
  /// SAME ones `Obj::tag()` and this type's own `#[cfg(test)]` module
  /// cross-check against the enum's actual discriminants.
  fn obj_tag(&mut self, ptr: IrValue) -> IrValue {
    let tag8 = self.fb.ins().load(
      types::I8,
      cranelift_codegen::ir::MemFlagsData::trusted(),
      ptr,
      0,
    );
    self.fb.ins().uextend(types::I64, tag8)
  }

  fn both_obj(&mut self, va: IrValue, vb: IrValue) -> IrValue {
    let oa = self.is_obj(va);
    let ob = self.is_obj(vb);
    self.fb.ins().band(oa, ob)
  }

  fn to_f64(&mut self, bits: IrValue) -> IrValue {
    self
      .fb
      .ins()
      .bitcast(types::F64, cranelift_codegen::ir::MemFlagsData::new(), bits)
  }

  fn from_f64(&mut self, f: IrValue) -> IrValue {
    self
      .fb
      .ins()
      .bitcast(types::I64, cranelift_codegen::ir::MemFlagsData::new(), f)
  }

  /// Wraps a boolean condition into a Zuri `Value` bit pattern (`nil`/
  /// `true`/`false` share the same `QNAN`-tagged layout -- see
  /// `value.rs`): `cond` selects between the baked `TRUE_VAL`/
  /// `FALSE_VAL` constants directly, with no branch at all.
  fn bool_value(&mut self, cond: IrValue) -> IrValue {
    let t = self.u64c(value::TRUE_VAL);
    let f = self.u64c(value::FALSE_VAL);
    self.fb.ins().select(cond, t, f)
  }

  /// `Value::is_falsey()` -- fully inlined, no helper call at all,
  /// UNLESS `cond`'s register holds a heap object at runtime (a
  /// bigint/string/bytes might be empty-and-therefore-falsey; any other
  /// heap type never is -- see `value.rs`'s own `is_falsey` doc
  /// comment). `Value`'s tag space is exactly {number, nil, true,
  /// false, object}, and only the "object" case needs a real
  /// dereference to resolve -- nil/bool/number are each decidable from
  /// the bit pattern alone: `nil` and `false` are exact bit-pattern
  /// matches, and a number is falsey iff it's `<= 0.0` (real IEEE-754
  /// comparison, not a bit compare, to get `-0.0`/NaN right). This is
  /// the single hottest check in the whole VM (every loop condition and
  /// `if` goes through it), so avoiding a real function call for the
  /// overwhelming majority of cases (a loop counter, a comparison
  /// result, ...) matters far more here than for most other ops.
  fn emit_is_falsey(&mut self, cond: u8) -> IrValue {
    let v = self.load_reg(cond);
    let is_obj = self.is_obj(v);
    let result_var = self.fb.declare_var(types::I64);

    // See `emit_safepoint`'s own docs on why the flush has to be
    // unconditional, in shared code, rather than living inside
    // `call_helper`'s automatic wrapping bundled into `obj_block`
    // below: `obj_block` only actually runs at runtime when `cond`
    // holds a heap object, which is FAR from every call (a loop
    // condition or `if` on a bool/number never takes it) -- the
    // register cache can't know in advance which way THIS particular
    // check will go, so it has to assume the conservative case (a
    // helper call, and therefore a real safepoint, COULD happen here)
    // unconditionally.
    self.flush_live(self.current_ip);

    let obj_block = self.fb.create_block();
    let nonobj_block = self.fb.create_block();
    let merge_block = self.fb.create_block();
    self
      .fb
      .ins()
      .brif(is_obj, obj_block, &[], nonobj_block, &[]);

    self.fb.switch_to_block(obj_block);
    // `Value::is_falsey`'s own definition (value.rs) never treats ANY
    // object as falsey except an empty Str/Bytes or a non-positive
    // BigInt -- every other heap kind (List, Dict, Instance, Closure,
    // ...) is unconditionally NOT falsey, decidable from the tag byte
    // alone with no payload inspection. So: check the tag first, and
    // only actually call the helper (to inspect the payload) for the
    // three kinds where the answer can vary.
    let ptr = self.obj_ptr(v);
    let tag = self.obj_tag(ptr);
    let tag_str = self.i64c(object::OBJ_TAG_STR as i64);
    let tag_bytes = self.i64c(object::OBJ_TAG_BYTES as i64);
    let tag_bigint = self.i64c(object::OBJ_TAG_BIGINT as i64);
    let is_str = self.fb.ins().icmp(IntCC::Equal, tag, tag_str);
    let is_bytes = self.fb.ins().icmp(IntCC::Equal, tag, tag_bytes);
    let is_bigint = self.fb.ins().icmp(IntCC::Equal, tag, tag_bigint);
    let is_str_or_bytes = self.fb.ins().bor(is_str, is_bytes);
    let maybe_falsey = self.fb.ins().bor(is_str_or_bytes, is_bigint);

    let payload_block = self.fb.create_block();
    let never_falsey_block = self.fb.create_block();
    self
      .fb
      .ins()
      .brif(maybe_falsey, payload_block, &[], never_falsey_block, &[]);

    self.fb.switch_to_block(payload_block);
    let base = self.base_param;
    let vm_p = self.vm_param;
    let cond_i = self.idx(cond);
    let falsey = self.call_helper_raw("zuri_jit_is_falsey", &[vm_p, base, cond_i]);
    self.fb.def_var(result_var, falsey);
    self.fb.ins().jump(merge_block, &[]);

    self.fb.switch_to_block(never_falsey_block);
    let not_falsey = self.i64c(0);
    self.fb.def_var(result_var, not_falsey);
    self.fb.ins().jump(merge_block, &[]);

    self.fb.switch_to_block(nonobj_block);
    let nil_val = self.u64c(value::NIL_VAL);
    let false_val = self.u64c(value::FALSE_VAL);
    let is_nil = self.fb.ins().icmp(IntCC::Equal, v, nil_val);
    let is_false = self.fb.ins().icmp(IntCC::Equal, v, false_val);
    let is_num = self.is_number(v);
    let fv = self.to_f64(v);
    let zero_f = self.fb.ins().f64const(0.0);
    let le_zero = self.fb.ins().fcmp(
      cranelift_codegen::ir::condcodes::FloatCC::LessThanOrEqual,
      fv,
      zero_f,
    );
    let num_falsey = self.fb.ins().band(is_num, le_zero);
    let nil_or_false = self.fb.ins().bor(is_nil, is_false);
    let falsey_bool = self.fb.ins().bor(nil_or_false, num_falsey);
    let falsey_i64 = self.fb.ins().uextend(types::I64, falsey_bool);
    self.fb.def_var(result_var, falsey_i64);
    self.fb.ins().jump(merge_block, &[]);

    self.fb.switch_to_block(merge_block);
    // Mirrors the unconditional flush above -- `obj_block`'s helper
    // call, if it ran, could have relocated/invalidated other live
    // registers, and there's no way to tell from here which branch
    // was actually taken.
    self.mark_stale_live(self.current_ip);
    self.fb.use_var(result_var)
  }

  // ---------------------------------------------------------------
  // Per-instruction codegen. Returns `true` if the instruction's own
  // codegen already ends in a terminator (so `run`'s driver loop must
  // NOT append an automatic fallthrough jump), `false` otherwise.
  // ---------------------------------------------------------------

  fn emit_instruction(&mut self, ip: usize, instr: Instr) -> bool {
    // See `current_ip`'s own docs -- every nested helper/`call_indirect`
    // this instruction's own codegen issues (there can be more than
    // one, e.g. `emit_fast_call`'s prepare call plus its fast-path
    // `call_indirect`) consults this SAME value.
    self.current_ip = ip;
    // See `merge_points`'s own docs: a genuine CFG join point can't
    // trust whichever single predecessor's `reg_cache` state happened
    // to be active when THIS compile-time walk last touched it --
    // force a fresh memory read for every live register's next use
    // instead, regardless of which predecessor is ACTUALLY taken at
    // runtime. Every predecessor edge is responsible for its OWN
    // flush before branching HERE (see `flush_before_jump` for forward/
    // fall-through edges, `emit_safepoint` for back edges) -- this
    // block itself must NOT also flush via `use_var`: on the back
    // edge specifically, that would read whatever Cranelift's own SSA
    // construction is holding for the `Variable` in a machine
    // register/spill slot, a location GC cannot see or fix up, and
    // writing it back over memory can clobber a relocation that
    // already ran (see `flush_before_jump`'s own docs for the full
    // reasoning -- this was a real, confirmed bug, not a hypothetical
    // one). By the time control reaches here via ANY edge, memory is
    // already correct; all that's needed is invalidating this
    // compiler's OWN bookkeeping so later code in/after this block
    // re-reads it instead of trusting a stale `Variable`.
    if self.merge_points[ip] {
      self.mark_stale_live(ip);
    }
    match instr {
      Instr::LoadConst { dst, const_idx } => {
        let v = self.bake_const(const_idx);
        self.store_reg(dst, v);
        false
      },
      Instr::LoadNil { dst } => {
        let v = self.u64c(value::NIL_VAL);
        self.store_reg(dst, v);
        false
      },
      Instr::LoadBool { dst, val } => {
        let v = self.u64c(if val {
          value::TRUE_VAL
        } else {
          value::FALSE_VAL
        });
        self.store_reg(dst, v);
        false
      },
      Instr::Move { dst, src } => {
        let v = self.load_reg(src);
        self.store_reg(dst, v);
        false
      },

      Instr::Add { dst, a, b } => {
        if self.both_proven_numeric(ip, a, b) {
          self.emit_binary_numeric_proven(dst, a, b, |fc, fa, fb| fc.fb.ins().fadd(fa, fb));
        } else {
          self.emit_binary_numeric_guarded(dst, a, b, "zuri_jit_add_slow", |fc, fa, fb| {
            fc.fb.ins().fadd(fa, fb)
          });
        }
        false
      },
      Instr::Sub { dst, a, b } => {
        if self.both_proven_numeric(ip, a, b) {
          self.emit_binary_numeric_proven(dst, a, b, |fc, fa, fb| fc.fb.ins().fsub(fa, fb));
        } else {
          self.emit_binary_numeric_guarded(dst, a, b, "zuri_jit_sub_slow", |fc, fa, fb| {
            fc.fb.ins().fsub(fa, fb)
          });
        }
        false
      },
      Instr::Mul { dst, a, b } => {
        if self.both_proven_numeric(ip, a, b) {
          self.emit_binary_numeric_proven(dst, a, b, |fc, fa, fb| fc.fb.ins().fmul(fa, fb));
        } else {
          self.emit_binary_numeric_guarded(dst, a, b, "zuri_jit_mul_slow", |fc, fa, fb| {
            fc.fb.ins().fmul(fa, fb)
          });
        }
        false
      },
      Instr::Div { dst, a, b } => {
        if self.both_proven_numeric(ip, a, b) {
          self.emit_binary_numeric_proven(dst, a, b, |fc, fa, fb| fc.fb.ins().fdiv(fa, fb));
        } else {
          self.emit_binary_numeric_guarded(dst, a, b, "zuri_jit_div_slow", |fc, fa, fb| {
            fc.fb.ins().fdiv(fa, fb)
          });
        }
        false
      },
      Instr::Pow { dst, a, b } => {
        self.emit_always_helper("zuri_jit_pow", dst, a, b);
        false
      },
      Instr::Floor { dst, a, b } => {
        self.emit_always_helper("zuri_jit_floordiv", dst, a, b);
        false
      },
      Instr::Mod { dst, a, b } => {
        self.emit_always_helper("zuri_jit_mod", dst, a, b);
        false
      },

      Instr::BitAnd { dst, a, b } => {
        if self.both_proven_numeric(ip, a, b) {
          self.emit_bitwise_proven(dst, a, b, |fb, ia, ib| fb.ins().band(ia, ib));
        } else {
          self.emit_bitwise_guarded(dst, a, b, "zuri_jit_bitand_slow", |fb, ia, ib| {
            fb.ins().band(ia, ib)
          });
        }
        false
      },
      Instr::BitOr { dst, a, b } => {
        if self.both_proven_numeric(ip, a, b) {
          self.emit_bitwise_proven(dst, a, b, |fb, ia, ib| fb.ins().bor(ia, ib));
        } else {
          self.emit_bitwise_guarded(dst, a, b, "zuri_jit_bitor_slow", |fb, ia, ib| {
            fb.ins().bor(ia, ib)
          });
        }
        false
      },
      Instr::BitXor { dst, a, b } => {
        if self.both_proven_numeric(ip, a, b) {
          self.emit_bitwise_proven(dst, a, b, |fb, ia, ib| fb.ins().bxor(ia, ib));
        } else {
          self.emit_bitwise_guarded(dst, a, b, "zuri_jit_bitxor_slow", |fb, ia, ib| {
            fb.ins().bxor(ia, ib)
          });
        }
        false
      },
      Instr::BitShl { dst, a, b } => {
        self.emit_always_helper("zuri_jit_bitshl", dst, a, b);
        false
      },
      Instr::BitShr { dst, a, b } => {
        self.emit_always_helper("zuri_jit_bitshr", dst, a, b);
        false
      },
      Instr::BitUshr { dst, a, b } => {
        self.emit_always_helper("zuri_jit_bitushr", dst, a, b);
        false
      },
      Instr::BitNot { dst, src } => {
        if self.proven_numeric(ip, src) {
          let v = self.load_reg(src);
          let f = self.to_f64(v);
          let i = self.fb.ins().fcvt_to_sint_sat(types::I64, f);
          let inv = self.fb.ins().bnot(i);
          let r = self.fb.ins().fcvt_from_sint(types::F64, inv);
          let bits = self.from_f64(r);
          self.store_reg(dst, bits);
        } else {
          let v = self.load_reg(src);
          let is_num = self.is_number(v);
          let snapshot = self.snapshot_reg_cache();
          let fast_block = self.fb.create_block();
          let slow_block = self.fb.create_block();
          let done_block = self.fb.create_block();
          self.fb.ins().brif(is_num, fast_block, &[], slow_block, &[]);

          self.fb.switch_to_block(fast_block);
          let f = self.to_f64(v);
          let i = self.fb.ins().fcvt_to_sint_sat(types::I64, f);
          let inv = self.fb.ins().bnot(i);
          let r = self.fb.ins().fcvt_from_sint(types::F64, inv);
          let bits = self.from_f64(r);
          self.store_reg(dst, bits);
          self.fb.ins().jump(done_block, &[]);

          self.fb.switch_to_block(slow_block);
          let base = self.base_param;
          let dst_i = self.idx(dst);
          let src_i = self.idx(src);
          self.call_checked("zuri_jit_bitnot_slow", &[self.vm_param, base, dst_i, src_i]);
          self.resync_dst_from_memory(dst);
          self.fb.ins().jump(done_block, &[]);

          self.fb.switch_to_block(done_block);
          self.restore_dirty_from_snapshot(&snapshot, dst);
        }
        false
      },
      Instr::Neg { dst, src } => {
        if self.proven_numeric(ip, src) {
          let v = self.load_reg(src);
          let f = self.to_f64(v);
          let neg = self.fb.ins().fneg(f);
          let bits = self.from_f64(neg);
          self.store_reg(dst, bits);
        } else {
          let v = self.load_reg(src);
          let is_num = self.is_number(v);
          let snapshot = self.snapshot_reg_cache();
          let fast_block = self.fb.create_block();
          let slow_block = self.fb.create_block();
          let done_block = self.fb.create_block();
          self.fb.ins().brif(is_num, fast_block, &[], slow_block, &[]);

          self.fb.switch_to_block(fast_block);
          let f = self.to_f64(v);
          let neg = self.fb.ins().fneg(f);
          let bits = self.from_f64(neg);
          self.store_reg(dst, bits);
          self.fb.ins().jump(done_block, &[]);

          self.fb.switch_to_block(slow_block);
          let base = self.base_param;
          let dst_i = self.idx(dst);
          let src_i = self.idx(src);
          self.call_checked("zuri_jit_neg_slow", &[self.vm_param, base, dst_i, src_i]);
          self.resync_dst_from_memory(dst);
          self.fb.ins().jump(done_block, &[]);

          self.fb.switch_to_block(done_block);
          self.restore_dirty_from_snapshot(&snapshot, dst);
        }
        false
      },
      Instr::Not { dst, src } => {
        let base = self.base_param;
        let dst_i = self.idx(dst);
        let src_i = self.idx(src);
        self.call_checked("zuri_jit_logical_not", &[self.vm_param, base, dst_i, src_i]);
        false
      },
      Instr::Concat { dst, a, b } => {
        self.emit_always_helper("zuri_jit_concat", dst, a, b);
        false
      },

      Instr::Eq { dst, a, b } => {
        if self.both_proven_numeric(ip, a, b) {
          self.emit_compare_proven_numeric(dst, a, b, IntCC::Equal);
        } else {
          self.emit_compare_guarded(dst, a, b, "zuri_jit_eq_slow", IntCC::Equal);
        }
        false
      },
      Instr::Neq { dst, a, b } => {
        if self.both_proven_numeric(ip, a, b) {
          self.emit_compare_proven_numeric(dst, a, b, IntCC::NotEqual);
        } else {
          self.emit_compare_guarded(dst, a, b, "zuri_jit_neq_slow", IntCC::NotEqual);
        }
        false
      },
      Instr::Lt { dst, a, b } => {
        if self.both_proven_numeric(ip, a, b) {
          self.emit_fcompare_proven(
            dst,
            a,
            b,
            cranelift_codegen::ir::condcodes::FloatCC::LessThan,
          );
        } else {
          self.emit_fcompare_guarded(
            dst,
            a,
            b,
            "zuri_jit_lt_slow",
            cranelift_codegen::ir::condcodes::FloatCC::LessThan,
          );
        }
        false
      },
      Instr::Le { dst, a, b } => {
        if self.both_proven_numeric(ip, a, b) {
          self.emit_fcompare_proven(
            dst,
            a,
            b,
            cranelift_codegen::ir::condcodes::FloatCC::LessThanOrEqual,
          );
        } else {
          self.emit_fcompare_guarded(
            dst,
            a,
            b,
            "zuri_jit_le_slow",
            cranelift_codegen::ir::condcodes::FloatCC::LessThanOrEqual,
          );
        }
        false
      },
      Instr::Gt { dst, a, b } => {
        if self.both_proven_numeric(ip, a, b) {
          self.emit_fcompare_proven(
            dst,
            a,
            b,
            cranelift_codegen::ir::condcodes::FloatCC::GreaterThan,
          );
        } else {
          self.emit_fcompare_guarded(
            dst,
            a,
            b,
            "zuri_jit_gt_slow",
            cranelift_codegen::ir::condcodes::FloatCC::GreaterThan,
          );
        }
        false
      },
      Instr::Ge { dst, a, b } => {
        if self.both_proven_numeric(ip, a, b) {
          self.emit_fcompare_proven(
            dst,
            a,
            b,
            cranelift_codegen::ir::condcodes::FloatCC::GreaterThanOrEqual,
          );
        } else {
          self.emit_fcompare_guarded(
            dst,
            a,
            b,
            "zuri_jit_ge_slow",
            cranelift_codegen::ir::condcodes::FloatCC::GreaterThanOrEqual,
          );
        }
        false
      },

      Instr::Jmp { offset } => {
        let target_ip = (ip as isize + 1 + offset as isize) as usize;
        if offset < 0 {
          self.emit_safepoint();
        } else {
          self.flush_before_jump(target_ip);
        }
        self.fb.ins().jump(self.blocks[target_ip], &[]);
        true
      },
      Instr::JmpIfFalse { cond, offset } => {
        let target_ip = (ip as isize + 1 + offset as isize) as usize;
        let falsey = self.emit_is_falsey(cond);
        let zero = self.i64c(0);
        let is_falsey = self.fb.ins().icmp(IntCC::NotEqual, falsey, zero);
        if offset < 0 {
          self.emit_safepoint();
        } else {
          self.flush_before_jump(target_ip);
        }
        self.flush_before_jump(ip + 1);
        self.fb.ins().brif(
          is_falsey,
          self.blocks[target_ip],
          &[],
          self.blocks[ip + 1],
          &[],
        );
        true
      },
      Instr::JmpIfTrue { cond, offset } => {
        let target_ip = (ip as isize + 1 + offset as isize) as usize;
        let truthy = self.emit_is_falsey(cond);
        let zero = self.i64c(0);
        let is_truthy = self.fb.ins().icmp(IntCC::Equal, truthy, zero);
        if offset < 0 {
          self.emit_safepoint();
        } else {
          self.flush_before_jump(target_ip);
        }
        self.flush_before_jump(ip + 1);
        self.fb.ins().brif(
          is_truthy,
          self.blocks[target_ip],
          &[],
          self.blocks[ip + 1],
          &[],
        );
        true
      },

      Instr::Call {
        dst,
        func,
        num_args,
      } => {
        // Inlining is checked BEFORE the safepoint, because an inlined
        // body emits no call, no allocation and no frame push -- there
        // is nothing for a collection to be owed at such a site, and
        // the enclosing loop's own back edge still carries one.
        if let Some(CallTarget::Known {
          guard_bits,
          proto_ptr,
          ..
        }) = self.call_targets.get(&ip).copied()
          && self.try_emit_inlined_call(ip, dst, func, num_args, guard_bits, proto_ptr)
        {
          return false;
        }
        self.emit_safepoint();
        match self.call_targets.get(&ip).copied() {
          Some(CallTarget::SelfRecursive) => self.emit_self_call(dst, func, num_args),
          // `entry == 0` means "resolved, but not compiled yet" (see
          // `CallTarget::Known::entry`) -- there is no address to jump
          // to, so this falls through to the ordinary resolver exactly
          // as an unresolved site would.
          Some(CallTarget::Known {
            entry, guard_bits, ..
          }) if entry != 0 => self.emit_known_call(dst, func, num_args, entry, guard_bits),
          Some(CallTarget::Known { .. }) => self.emit_generic_call(dst, func, num_args),
          Some(CallTarget::Construct) => self.emit_construct_call(dst, func, num_args),
          Some(CallTarget::ConstructKnown {
            guard_bits,
            generation,
            field_count,
            ctor_bits,
            proto_ptr,
          }) => {
            if self.scalar_construct_eligible(ip, dst) {
              self.emit_scalar_construct(ip, dst, func, num_args);
              return false;
            }
            self.emit_construct_known(
              dst,
              func,
              num_args,
              guard_bits,
              generation,
              field_count,
              ctor_bits,
              proto_ptr,
            )
          },
          None => self.emit_generic_call(dst, func, num_args),
        }
        false
      },
      Instr::Return { src } => {
        if self.frame_can_open_upvalues {
          let base = self.base_param;
          let zero = self.i64c(0);
          self.call_checked("zuri_jit_close_upvalues", &[self.vm_param, base, zero]);
        }
        let v = self.load_reg(src);
        self.fb.ins().return_(&[v]);
        true
      },

      Instr::Print { src } => {
        let base = self.base_param;
        let src_i = self.idx(src);
        self.call_checked("zuri_jit_print", &[self.vm_param, base, src_i]);
        false
      },

      Instr::GetGlobal { dst, name_const } => {
        self.emit_get_global(ip, dst, name_const);
        false
      },
      Instr::SetGlobal { name_const, src } => {
        self.emit_set_global(ip, src, name_const, "zuri_jit_set_global");
        false
      },
      Instr::AssignGlobal { name_const, src } => {
        self.emit_set_global(ip, src, name_const, "zuri_jit_assign_global");
        false
      },

      Instr::Closure { dst, proto_const } => {
        let base = self.base_param;
        let dst_i = self.idx(dst);
        let proto_v = self.bake_const(proto_const);
        self.call_checked(
          "zuri_jit_make_closure",
          &[self.vm_param, base, dst_i, proto_v, self.closure_param],
        );
        false
      },
      Instr::GetUpval { dst, idx: uidx } => {
        let base = self.base_param;
        let dst_i = self.idx(dst);
        let uidx_i = self.idx(uidx);
        self.call_checked(
          "zuri_jit_get_upval",
          &[self.vm_param, base, dst_i, uidx_i, self.closure_param],
        );
        false
      },
      Instr::SetUpval { idx: uidx, src } => {
        let base = self.base_param;
        let src_i = self.idx(src);
        let uidx_i = self.idx(uidx);
        self.call_checked(
          "zuri_jit_set_upval",
          &[self.vm_param, base, src_i, uidx_i, self.closure_param],
        );
        false
      },
      Instr::CloseUpvalues { from } => {
        let base = self.base_param;
        let from_i = self.idx(from);
        self.call_checked("zuri_jit_close_upvalues", &[self.vm_param, base, from_i]);
        false
      },

      Instr::MakeList { dst, start, count } => {
        if self.scalar_replace_eligible(ip, dst, count) {
          self.emit_scalar_make_list(dst, start, count);
          return false;
        }
        let base = self.base_param;
        let dst_i = self.idx(dst);
        let start_i = self.idx(start);
        let count_i = self.idx(count);
        self.call_checked(
          "zuri_jit_make_list",
          &[self.vm_param, base, dst_i, start_i, count_i],
        );
        false
      },
      Instr::MakeDict { dst, start, count } => {
        let base = self.base_param;
        let dst_i = self.idx(dst);
        let start_i = self.idx(start);
        let count_i = self.idx(count);
        self.call_checked(
          "zuri_jit_make_dict",
          &[self.vm_param, base, dst_i, start_i, count_i],
        );
        false
      },

      Instr::MakeClass {
        dst,
        name_const,
        superclass,
      } => {
        let base = self.base_param;
        let dst_i = self.idx(dst);
        let name = self.bake_const(name_const);
        let (has_super, super_reg) = match superclass {
          Some(r) => (self.i64c(1), self.idx(r)),
          None => (self.i64c(0), self.i64c(0)),
        };
        self.call_checked(
          "zuri_jit_make_class",
          &[self.vm_param, base, dst_i, name, has_super, super_reg],
        );
        false
      },
      Instr::DeclareField { class, name_const } => {
        let base = self.base_param;
        let class_i = self.idx(class);
        let name = self.bake_const(name_const);
        self.call_checked(
          "zuri_jit_declare_field",
          &[self.vm_param, base, class_i, name],
        );
        false
      },
      Instr::SetFieldInit { class, src } => {
        let base = self.base_param;
        let class_i = self.idx(class);
        let src_i = self.idx(src);
        self.call_checked(
          "zuri_jit_set_field_init",
          &[self.vm_param, base, class_i, src_i],
        );
        false
      },
      Instr::SetMethod {
        class,
        name_const,
        src,
      } => {
        let base = self.base_param;
        let class_i = self.idx(class);
        let name = self.bake_const(name_const);
        let src_i = self.idx(src);
        self.call_checked(
          "zuri_jit_set_method",
          &[self.vm_param, base, class_i, name, src_i],
        );
        false
      },
      Instr::DeclareStatic {
        class,
        name_const,
        src,
      } => {
        let base = self.base_param;
        let class_i = self.idx(class);
        let name = self.bake_const(name_const);
        let src_i = self.idx(src);
        self.call_checked(
          "zuri_jit_declare_static",
          &[self.vm_param, base, class_i, name, src_i],
        );
        false
      },
      Instr::FinalizeClass { class } => {
        let base = self.base_param;
        let class_i = self.idx(class);
        let func_ptr = self.func_ptr_const();
        self.call_checked(
          "zuri_jit_finalize_class",
          &[self.vm_param, base, class_i, func_ptr],
        );
        false
      },
      Instr::GetField {
        dst,
        obj,
        name_const,
      } => {
        // A scalar-replaced instance has no object to read from -- the
        // field IS the stack slot, so this becomes a plain load.
        if let Some((slot, field)) = self.scalar_instance_slot(obj, name_const) {
          let v = self
            .fb
            .ins()
            .stack_load(types::I64, types::I64, slot, (field as i32) * 8);
          self.store_reg(dst, v);
          return false;
        }
        if let Some(slot) = self.self_field_slot(obj, name_const) {
          self.emit_self_get_field(ip, dst, obj, name_const, slot);
          return false;
        }
        if let Some(cache) = self.field_cache_addr(ip) {
          self.emit_ic_get_field(ip, dst, obj, name_const, cache);
          return false;
        }
        let base = self.base_param;
        let dst_i = self.idx(dst);
        let obj_i = self.idx(obj);
        let name = self.bake_const(name_const);
        let func_ptr = self.func_ptr_const();
        let ip_c = self.u64c(ip as u64);
        self.call_checked(
          "zuri_jit_get_field",
          &[self.vm_param, base, dst_i, obj_i, name, func_ptr, ip_c],
        );
        false
      },
      Instr::SetField {
        obj,
        name_const,
        src,
      } => {
        // Mirror of the `GetField` case above: a plain store into the
        // stack slot standing in for the never-allocated instance.
        if let Some((slot, field)) = self.scalar_instance_slot(obj, name_const) {
          let v = self.load_reg(src);
          self
            .fb
            .ins()
            .stack_store(types::I64, v, slot, (field as i32) * 8);
          return false;
        }
        if let Some(slot) = self.self_field_slot(obj, name_const) {
          self.emit_self_set_field(ip, obj, name_const, src, slot);
          return false;
        }
        if let Some(cache) = self.field_cache_addr(ip) {
          self.emit_ic_set_field(ip, obj, name_const, src, cache);
          return false;
        }
        let base = self.base_param;
        let obj_i = self.idx(obj);
        let name = self.bake_const(name_const);
        let src_i = self.idx(src);
        let func_ptr = self.func_ptr_const();
        let ip_c = self.u64c(ip as u64);
        self.call_checked(
          "zuri_jit_set_field",
          &[self.vm_param, base, obj_i, name, src_i, func_ptr, ip_c],
        );
        false
      },

      Instr::Invoke {
        dst,
        obj,
        method_const,
        num_args,
      } => {
        if let Some(op) = NumberIntrinsic::of(self.method_name(method_const))
          && op.arity() == num_args
        {
          self.emit_number_intrinsic(ip, dst, obj, method_const, num_args, op);
          return false;
        }
        self.emit_safepoint();
        self.emit_generic_invoke(ip, dst, obj, method_const, num_args);
        false
      },
      Instr::InvokeSuper {
        dst,
        superclass,
        method_const,
        num_args,
      } => {
        self.emit_safepoint();
        let base = self.base_param;
        let super_i = self.idx(superclass);
        let num_args_i = self.idx(num_args);
        let dst_i = self.idx(dst);
        let name = self.bake_const(method_const);
        self.call_checked(
          "zuri_jit_invoke_super",
          &[self.vm_param, base, super_i, num_args_i, dst_i, name],
        );
        false
      },
      Instr::CallSuperCtor {
        dst,
        superclass,
        num_args,
      } => {
        self.emit_safepoint();
        let base = self.base_param;
        let super_i = self.idx(superclass);
        let num_args_i = self.idx(num_args);
        let dst_i = self.idx(dst);
        self.call_checked(
          "zuri_jit_call_super_ctor",
          &[self.vm_param, base, super_i, num_args_i, dst_i],
        );
        false
      },

      Instr::Import {
        dst,
        path_const,
        importer_const,
      } => {
        self.emit_safepoint();
        let base = self.base_param;
        let dst_i = self.idx(dst);
        let path = self.bake_const(path_const);
        let importer = self.bake_const(importer_const);
        self.call_checked(
          "zuri_jit_import",
          &[self.vm_param, base, dst_i, path, importer],
        );
        false
      },
      Instr::ImportAll { module } => {
        let base = self.base_param;
        let module_i = self.idx(module);
        let func_ptr = self.func_ptr_const();
        self.call_checked(
          "zuri_jit_import_all",
          &[self.vm_param, base, module_i, func_ptr],
        );
        false
      },
      Instr::MakePromoted {
        dst,
        module,
        name_const,
      } => {
        let base = self.base_param;
        let dst_i = self.idx(dst);
        let module_i = self.idx(module);
        let name = self.bake_const(name_const);
        self.call_checked(
          "zuri_jit_make_promoted",
          &[self.vm_param, base, dst_i, module_i, name],
        );
        false
      },

      Instr::GetIndex {
        dst,
        obj,
        idx: iidx,
      } => {
        if let Some(&(slot, count)) = self.scalar_lists.get(&obj) {
          self.emit_scalar_list_get(dst, slot, count, iidx);
          return false;
        }
        self.emit_list_get_index(dst, obj, iidx);
        false
      },
      Instr::SetIndex {
        obj,
        idx: iidx,
        src,
      } => {
        if let Some(&(slot, count)) = self.scalar_lists.get(&obj) {
          self.emit_scalar_list_set(slot, count, iidx, src);
          return false;
        }
        self.emit_list_set_index(obj, iidx, src);
        false
      },
      Instr::GetSlice { dst, obj, lo, hi } => {
        let base = self.base_param;
        let dst_i = self.idx(dst);
        let obj_i = self.idx(obj);
        let lo_i = self.idx(lo);
        let hi_i = self.idx(hi);
        self.call_checked(
          "zuri_jit_get_slice",
          &[self.vm_param, base, dst_i, obj_i, lo_i, hi_i],
        );
        false
      },

      Instr::MakeRange { dst, lower, upper } => {
        let base = self.base_param;
        let dst_i = self.idx(dst);
        let lower_i = self.idx(lower);
        let upper_i = self.idx(upper);
        self.call_checked(
          "zuri_jit_make_range",
          &[self.vm_param, base, dst_i, lower_i, upper_i],
        );
        false
      },

      Instr::UsingJump { subject, table_idx } => {
        let base = self.base_param;
        let subject_i = self.idx(subject);
        let func_ptr = self.func_ptr_const();
        let table_i = self.i64c(table_idx as i64);
        let target = self.call_helper(
          "zuri_jit_using_jump",
          &[self.vm_param, base, subject_i, func_ptr, table_i],
        );
        let no_match = self.u64c(runtime_using_no_match());
        let matched = self.fb.ins().icmp(IntCC::NotEqual, target, no_match);

        // A hit sets `ip` directly to an arbitrary absolute bytecode
        // position -- unlike every other jump, the destination isn't
        // known until runtime, so this can't be a direct `Block`
        // branch. Route through a tiny indirect trampoline instead:
        // `br_table`-free by construction (avoids that API's exact
        // shape entirely) -- a chain comparing the returned target ip
        // against every jump-table VALUE this specific `UsingJump`
        // could have produced, each already known at compile time from
        // `chunk.jump_tables[table_idx]`.
        let miss_block = self.fb.create_block();
        let mut next_check = self.fb.create_block();
        self
          .fb
          .ins()
          .brif(matched, next_check, &[], miss_block, &[]);

        let mut targets: Vec<usize> = self.proto.chunk.jump_tables[table_idx as usize]
          .values()
          .copied()
          .collect();
        targets.sort_unstable();
        targets.dedup();
        for target_ip in targets {
          self.fb.switch_to_block(next_check);
          let want = self.u64c(target_ip as u64);
          let is_this = self.fb.ins().icmp(IntCC::Equal, target, want);
          let after = self.fb.create_block();
          self
            .fb
            .ins()
            .brif(is_this, self.blocks[target_ip], &[], after, &[]);
          next_check = after;
        }
        // Exhausted every known constant target without a match --
        // unreachable in practice (the helper only ever returns a
        // value it read out of this exact table), but fall through to
        // the miss path rather than leaving a block unterminated.
        self.fb.switch_to_block(next_check);
        self.fb.ins().jump(miss_block, &[]);

        self.fb.switch_to_block(miss_block);
        false
      },

      Instr::Raise { .. } | Instr::PushCatch { .. } | Instr::PopCatch => {
        unreachable!("excluded by the eligibility scan in `compile`")
      },

      Instr::AddImm { dst, a, imm_const } => {
        if self.proven_numeric(ip, a) {
          self.emit_addimm_proven(dst, a, imm_const);
        } else {
          self.emit_addimm(dst, a, imm_const);
        }
        false
      },
      Instr::SubImm { dst, a, imm_const } => {
        if self.proven_numeric(ip, a) {
          self
            .emit_imm_numeric_proven(dst, a, imm_const, |fc, fa, fimm| fc.fb.ins().fsub(fa, fimm));
        } else {
          self.emit_imm_numeric_guarded(
            dst,
            a,
            imm_const,
            "zuri_jit_subimm_slow",
            |fc, fa, fimm| fc.fb.ins().fsub(fa, fimm),
          );
        }
        false
      },
      Instr::MulImm { dst, a, imm_const } => {
        // No inline fast path beyond the numeric guard -- the
        // non-numeric fallback (string/list repeat) is common enough
        // (and cheap enough to check for) that `zuri_jit_mulimm_slow`
        // handles the WHOLE non-fast-path case uniformly; see its docs.
        if self.proven_numeric(ip, a) {
          self
            .emit_imm_numeric_proven(dst, a, imm_const, |fc, fa, fimm| fc.fb.ins().fmul(fa, fimm));
        } else {
          self.emit_imm_numeric_guarded(
            dst,
            a,
            imm_const,
            "zuri_jit_mulimm_slow",
            |fc, fa, fimm| fc.fb.ins().fmul(fa, fimm),
          );
        }
        false
      },
      Instr::LtImm { dst, a, imm_const } => {
        if self.proven_numeric(ip, a) {
          self.emit_imm_compare_proven(
            dst,
            a,
            imm_const,
            cranelift_codegen::ir::condcodes::FloatCC::LessThan,
          );
        } else {
          self.emit_imm_compare_guarded(
            dst,
            a,
            imm_const,
            "zuri_jit_ltimm_slow",
            cranelift_codegen::ir::condcodes::FloatCC::LessThan,
          );
        }
        false
      },
      Instr::LeImm { dst, a, imm_const } => {
        if self.proven_numeric(ip, a) {
          self.emit_imm_compare_proven(
            dst,
            a,
            imm_const,
            cranelift_codegen::ir::condcodes::FloatCC::LessThanOrEqual,
          );
        } else {
          self.emit_imm_compare_guarded(
            dst,
            a,
            imm_const,
            "zuri_jit_leimm_slow",
            cranelift_codegen::ir::condcodes::FloatCC::LessThanOrEqual,
          );
        }
        false
      },
      Instr::GtImm { dst, a, imm_const } => {
        if self.proven_numeric(ip, a) {
          self.emit_imm_compare_proven(
            dst,
            a,
            imm_const,
            cranelift_codegen::ir::condcodes::FloatCC::GreaterThan,
          );
        } else {
          self.emit_imm_compare_guarded(
            dst,
            a,
            imm_const,
            "zuri_jit_gtimm_slow",
            cranelift_codegen::ir::condcodes::FloatCC::GreaterThan,
          );
        }
        false
      },
      Instr::GeImm { dst, a, imm_const } => {
        if self.proven_numeric(ip, a) {
          self.emit_imm_compare_proven(
            dst,
            a,
            imm_const,
            cranelift_codegen::ir::condcodes::FloatCC::GreaterThanOrEqual,
          );
        } else {
          self.emit_imm_compare_guarded(
            dst,
            a,
            imm_const,
            "zuri_jit_geimm_slow",
            cranelift_codegen::ir::condcodes::FloatCC::GreaterThanOrEqual,
          );
        }
        false
      },
      Instr::EqImm { dst, a, imm_const } => {
        if self.proven_numeric(ip, a) {
          self.emit_imm_eq_proven(dst, a, imm_const, true);
        } else {
          self.emit_imm_eq(dst, a, imm_const, true);
        }
        false
      },
      Instr::NeqImm { dst, a, imm_const } => {
        if self.proven_numeric(ip, a) {
          self.emit_imm_eq_proven(dst, a, imm_const, false);
        } else {
          self.emit_imm_eq(dst, a, imm_const, false);
        }
        false
      },
    }
  }

  /// GC safepoint -- see `runtime::zuri_jit_gc_safepoint`'s own docs.
  /// Emitted at every loop back-edge and function/method call site,
  /// matching the standard "safepoints at back-edges and calls"
  /// baseline-JIT policy this project's design calls for.
  ///
  /// `Heap::needs_major_gc()`/`needs_minor_gc()` are each just a
  /// threshold compare -- inlined here (four loads + two compares, one
  /// pair per generation) so the overwhelmingly common case (neither
  /// generation near its threshold) costs that instead of an
  /// unconditional FFI call at EVERY loop iteration and call site. The
  /// real `zuri_jit_gc_safepoint` helper is only actually invoked on
  /// the rare branch where a collection of some kind is about to
  /// happen; it re-checks both thresholds itself too, so a stale read
  /// here (never possible mid-single-threaded-execution anyway)
  /// couldn't cause an incorrect collection either way.
  fn emit_safepoint(&mut self) {
    let bytes = self.fb.ins().load(
      types::I64,
      cranelift_codegen::ir::MemFlagsData::trusted(),
      self.vm_param,
      HEAP_BYTES_ALLOCATED_OFFSET,
    );
    let next_gc = self.fb.ins().load(
      types::I64,
      cranelift_codegen::ir::MemFlagsData::trusted(),
      self.vm_param,
      HEAP_NEXT_GC_OFFSET,
    );
    let young_bytes = self.fb.ins().load(
      types::I64,
      cranelift_codegen::ir::MemFlagsData::trusted(),
      self.vm_param,
      HEAP_YOUNG_BYTES_ALLOCATED_OFFSET,
    );
    // Must stay EXACTLY `Heap::needs_major_gc`'s own comparison, not a
    // conservative approximation of it: this test gates a real helper
    // call, so a version that over-fires would pay a full FFI round
    // trip at every safepoint for as long as the two disagreed, and
    // the helper would decline to collect each time. Hence the
    // subtraction here rather than reusing `bytes` directly -- see
    // `Heap::old_bytes_allocated` for why it cannot underflow.
    let old_bytes = self.fb.ins().isub(bytes, young_bytes);
    let needs_major = self
      .fb
      .ins()
      .icmp(IntCC::UnsignedGreaterThan, old_bytes, next_gc);

    // `YOUNG_NEXT_GC` is the one threshold in this collector that's
    // truly fixed (never grows the way `next_gc` does), so it's baked
    // in as a compile-time immediate instead of a third runtime load.
    let young_next_gc = self.i64c(object::Heap::YOUNG_NEXT_GC as i64);
    let needs_minor = self
      .fb
      .ins()
      .icmp(IntCC::UnsignedGreaterThan, young_bytes, young_next_gc);

    let needs_some_gc = self.fb.ins().bor(needs_major, needs_minor);

    // `flush_live`/`mark_stale_live` MUST run unconditionally, in
    // shared code BEFORE this branch -- not inside `gc_block` (which
    // is only entered at runtime if `needs_some_gc` is actually true).
    // This is a genuine safepoint regardless of whether a collection
    // ends up running THIS time: at compile time we can't know which
    // way `needs_some_gc` will go on any given call, so the register
    // cache has to assume the conservative case (a collection COULD
    // happen right here) every single time. Bundling this inside
    // `call_helper`'s automatic wrapping (as every OTHER call site in
    // this file correctly does) would silently condition it on
    // `gc_block` actually being entered -- on the overwhelmingly
    // common "no collection needed this time" path, that call (and
    // therefore the flush) would never actually execute, while every
    // instruction compiled AFTER this one would wrongly believe
    // memory was already made current. Using `call_helper_raw` inside
    // `gc_block` avoids double-flushing/double-staling against the
    // unconditional calls below.
    self.flush_live(self.current_ip);

    let gc_block = self.fb.create_block();
    let done_block = self.fb.create_block();
    self
      .fb
      .ins()
      .brif(needs_some_gc, gc_block, &[], done_block, &[]);

    self.fb.switch_to_block(gc_block);
    self.call_helper_raw("zuri_jit_gc_safepoint", &[self.vm_param]);
    // Neither collection touches `VM::registers`'s backing buffer
    // (they only read register contents for root-marking, and free
    // `Obj` storage on `heap`), so no `refresh_regs()` is needed here.
    self.fb.ins().jump(done_block, &[]);

    self.fb.switch_to_block(done_block);
    self.mark_stale_live(self.current_ip);
  }

  /// `emit_binary_numeric_guarded`'s fast-path body with the guard,
  /// branch, and slow-path fallback removed entirely -- valid only
  /// when the caller has already confirmed (via `type_facts`) that `a`
  /// and `b` are PROVEN numeric at this instruction, so the slow path
  /// could never be reached anyway.
  fn emit_binary_numeric_proven(
    &mut self,
    dst: u8,
    a: u8,
    b: u8,
    fast: impl FnOnce(&mut Self, IrValue, IrValue) -> IrValue,
  ) {
    let va = self.load_reg(a);
    let vb = self.load_reg(b);
    let fa = self.to_f64(va);
    let fb_ = self.to_f64(vb);
    let fr = fast(self, fa, fb_);
    let bits = self.from_f64(fr);
    self.store_reg(dst, bits);
  }

  /// Re-establishes `reg_vars[dst]` (via a real memory read, then a
  /// normal `store_reg`) immediately after a helper call that just
  /// wrote `dst`'s result DIRECTLY to memory, bypassing `store_reg`
  /// entirely. Needed specifically when this happens on ONE internal
  /// branch of a fast/slow split whose OTHER branch (the fast path)
  /// defines the SAME `reg_vars[dst]` via an ordinary `store_reg` call:
  /// without this, Cranelift's own SSA construction never sees a
  /// `def_var` for `dst` on the slow side at all, so its merge at the
  /// branches' join point would resolve a later `use_var` to whatever
  /// pre-instruction value dominated the slow edge -- silently
  /// discarding the slow path's real result if that's the path actually
  /// taken at runtime. Simply marking `dst` `Stale` (what `call_helper`'s
  /// generic `any_dst` handling already does for every helper-written
  /// destination) is NOT sufficient here specifically: `Stale` means
  /// "trust memory on the next read," but the FAST path's own result is
  /// deliberately never written to memory at all (that's the entire
  /// point of `store_reg`'s laziness) -- a stale-triggered reload would
  /// silently return the OLD, pre-instruction value if the fast path is
  /// the one that actually ran. Calling this right after the slow
  /// helper makes both branches leave `dst` in the exact same kind of
  /// state (a real `Variable` definition), so Cranelift's own merge
  /// handles the rest correctly regardless of which path is taken.
  fn resync_dst_from_memory(&mut self, dst: u8) {
    let v = self.load_reg_mem(dst);
    self.store_reg(dst, v);
  }

  fn emit_binary_numeric_guarded(
    &mut self,
    dst: u8,
    a: u8,
    b: u8,
    slow_helper: &'static str,
    fast: impl FnOnce(&mut Self, IrValue, IrValue) -> IrValue,
  ) {
    let va = self.load_reg(a);
    let vb = self.load_reg(b);
    let guard = self.both_numbers(va, vb);
    let snapshot = self.snapshot_reg_cache();
    let fast_block = self.fb.create_block();
    let slow_block = self.fb.create_block();
    let done_block = self.fb.create_block();
    self.fb.ins().brif(guard, fast_block, &[], slow_block, &[]);

    self.fb.switch_to_block(fast_block);
    let fa = self.to_f64(va);
    let fb_ = self.to_f64(vb);
    let fr = fast(self, fa, fb_);
    let bits = self.from_f64(fr);
    self.store_reg(dst, bits);
    self.fb.ins().jump(done_block, &[]);

    self.fb.switch_to_block(slow_block);
    let base = self.base_param;
    let dst_i = self.idx(dst);
    let a_i = self.idx(a);
    let b_i = self.idx(b);
    self.call_checked(slow_helper, &[self.vm_param, base, dst_i, a_i, b_i]);
    self.resync_dst_from_memory(dst);
    self.fb.ins().jump(done_block, &[]);

    self.fb.switch_to_block(done_block);
    self.restore_dirty_from_snapshot(&snapshot, dst);
  }

  /// `emit_bitwise_guarded`'s fast path, unguarded -- see
  /// `emit_binary_numeric_proven`'s docs.
  fn emit_bitwise_proven(
    &mut self,
    dst: u8,
    a: u8,
    b: u8,
    fast: impl FnOnce(&mut FunctionBuilder, IrValue, IrValue) -> IrValue,
  ) {
    let va = self.load_reg(a);
    let vb = self.load_reg(b);
    let fa = self.to_f64(va);
    let fb_ = self.to_f64(vb);
    let ia = self.fb.ins().fcvt_to_sint_sat(types::I64, fa);
    let ib = self.fb.ins().fcvt_to_sint_sat(types::I64, fb_);
    let ir = fast(self.fb, ia, ib);
    let fr = self.fb.ins().fcvt_from_sint(types::F64, ir);
    let bits = self.from_f64(fr);
    self.store_reg(dst, bits);
  }

  fn emit_bitwise_guarded(
    &mut self,
    dst: u8,
    a: u8,
    b: u8,
    slow_helper: &'static str,
    fast: impl FnOnce(&mut FunctionBuilder, IrValue, IrValue) -> IrValue,
  ) {
    let va = self.load_reg(a);
    let vb = self.load_reg(b);
    let guard = self.both_numbers(va, vb);
    let snapshot = self.snapshot_reg_cache();
    let fast_block = self.fb.create_block();
    let slow_block = self.fb.create_block();
    let done_block = self.fb.create_block();
    self.fb.ins().brif(guard, fast_block, &[], slow_block, &[]);

    self.fb.switch_to_block(fast_block);
    let fa = self.to_f64(va);
    let fb_ = self.to_f64(vb);
    let ia = self.fb.ins().fcvt_to_sint_sat(types::I64, fa);
    let ib = self.fb.ins().fcvt_to_sint_sat(types::I64, fb_);
    let ir = fast(self.fb, ia, ib);
    let fr = self.fb.ins().fcvt_from_sint(types::F64, ir);
    let bits = self.from_f64(fr);
    self.store_reg(dst, bits);
    self.fb.ins().jump(done_block, &[]);

    self.fb.switch_to_block(slow_block);
    let base = self.base_param;
    let dst_i = self.idx(dst);
    let a_i = self.idx(a);
    let b_i = self.idx(b);
    self.call_checked(slow_helper, &[self.vm_param, base, dst_i, a_i, b_i]);
    self.resync_dst_from_memory(dst);
    self.fb.ins().jump(done_block, &[]);

    self.fb.switch_to_block(done_block);
    self.restore_dirty_from_snapshot(&snapshot, dst);
  }

  fn emit_always_helper(&mut self, helper: &'static str, dst: u8, a: u8, b: u8) {
    let base = self.base_param;
    let dst_i = self.idx(dst);
    let a_i = self.idx(a);
    let b_i = self.idx(b);
    self.call_checked(helper, &[self.vm_param, base, dst_i, a_i, b_i]);
  }

  /// `Instr::Eq`/`Instr::Neq` -- fully inlined except when BOTH
  /// operands are heap objects (needing `Obj`-aware content comparison
  /// -- list/dict structural equality, or pointer identity for
  /// everything else -- which always needs a real dereference). Eq/Neq
  /// never consult an operator override (matches the interpreter's own
  /// handler exactly -- see `vm.rs`).
  ///
  /// `Value::equals`'s own logic is: real IEEE-754 compare when both
  /// are numbers; recursive/pointer compare when both are objects;
  /// otherwise a RAW BIT COMPARE of the two `u64`s (see `value.rs`) --
  /// which is exactly right for nil/bool/number-vs-anything-mismatched,
  /// since NaN-boxing guarantees no two DIFFERENT tag categories ever
  /// share a bit pattern. So the only case genuinely needing a helper
  /// is "both objects"; every other combination (extremely common --
  /// `x == nil` chains through linked structures, `flag == true`, a
  /// number compared against a non-number, ...) is just one more
  /// branch away from the number fast path, no helper call at all.
  /// `emit_compare_guarded`'s `num_block` path directly -- valid only
  /// when `a`/`b` are PROVEN numeric, so the object/raw-bits cases can
  /// never apply.
  fn emit_compare_proven_numeric(&mut self, dst: u8, a: u8, b: u8, cc: IntCC) {
    let va = self.load_reg(a);
    let vb = self.load_reg(b);
    let fa = self.to_f64(va);
    let fb_ = self.to_f64(vb);
    let cmp = self.fb.ins().fcmp(to_float_cc(cc), fa, fb_);
    let bits = self.bool_value(cmp);
    self.store_reg(dst, bits);
  }

  fn emit_compare_guarded(&mut self, dst: u8, a: u8, b: u8, slow_helper: &'static str, cc: IntCC) {
    let va = self.load_reg(a);
    let vb = self.load_reg(b);
    let both_num = self.both_numbers(va, vb);
    let both_obj = self.both_obj(va, vb);
    let snapshot = self.snapshot_reg_cache();

    let num_block = self.fb.create_block();
    let check_obj_block = self.fb.create_block();
    let obj_block = self.fb.create_block();
    let bits_block = self.fb.create_block();
    let done_block = self.fb.create_block();
    self
      .fb
      .ins()
      .brif(both_num, num_block, &[], check_obj_block, &[]);

    self.fb.switch_to_block(num_block);
    let fa = self.to_f64(va);
    let fb_ = self.to_f64(vb);
    let cmp = self.fb.ins().fcmp(to_float_cc(cc), fa, fb_);
    let bits = self.bool_value(cmp);
    self.store_reg(dst, bits);
    self.fb.ins().jump(done_block, &[]);

    self.fb.switch_to_block(check_obj_block);
    self
      .fb
      .ins()
      .brif(both_obj, obj_block, &[], bits_block, &[]);

    self.fb.switch_to_block(obj_block);
    let base = self.base_param;
    let dst_i = self.idx(dst);
    let a_i = self.idx(a);
    let b_i = self.idx(b);
    self.call_checked(slow_helper, &[self.vm_param, base, dst_i, a_i, b_i]);
    self.resync_dst_from_memory(dst);
    self.fb.ins().jump(done_block, &[]);

    self.fb.switch_to_block(bits_block);
    let raw_cmp = self.fb.ins().icmp(cc, va, vb);
    let raw_bits = self.bool_value(raw_cmp);
    self.store_reg(dst, raw_bits);
    self.fb.ins().jump(done_block, &[]);

    self.fb.switch_to_block(done_block);
    self.restore_dirty_from_snapshot(&snapshot, dst);
  }

  /// `emit_fcompare_guarded`'s fast path, unguarded -- see
  /// `emit_binary_numeric_proven`'s docs.
  fn emit_fcompare_proven(
    &mut self,
    dst: u8,
    a: u8,
    b: u8,
    cc: cranelift_codegen::ir::condcodes::FloatCC,
  ) {
    let va = self.load_reg(a);
    let vb = self.load_reg(b);
    let fa = self.to_f64(va);
    let fb_ = self.to_f64(vb);
    let cmp = self.fb.ins().fcmp(cc, fa, fb_);
    let bits = self.bool_value(cmp);
    self.store_reg(dst, bits);
  }

  fn emit_fcompare_guarded(
    &mut self,
    dst: u8,
    a: u8,
    b: u8,
    slow_helper: &'static str,
    cc: cranelift_codegen::ir::condcodes::FloatCC,
  ) {
    let va = self.load_reg(a);
    let vb = self.load_reg(b);
    let guard = self.both_numbers(va, vb);
    let snapshot = self.snapshot_reg_cache();
    let fast_block = self.fb.create_block();
    let slow_block = self.fb.create_block();
    let done_block = self.fb.create_block();
    self.fb.ins().brif(guard, fast_block, &[], slow_block, &[]);

    self.fb.switch_to_block(fast_block);
    let fa = self.to_f64(va);
    let fb_ = self.to_f64(vb);
    let cmp = self.fb.ins().fcmp(cc, fa, fb_);
    let bits = self.bool_value(cmp);
    self.store_reg(dst, bits);
    self.fb.ins().jump(done_block, &[]);

    self.fb.switch_to_block(slow_block);
    let base = self.base_param;
    let dst_i = self.idx(dst);
    let a_i = self.idx(a);
    let b_i = self.idx(b);
    self.call_checked(slow_helper, &[self.vm_param, base, dst_i, a_i, b_i]);
    self.resync_dst_from_memory(dst);
    self.fb.ins().jump(done_block, &[]);

    self.fb.switch_to_block(done_block);
    self.restore_dirty_from_snapshot(&snapshot, dst);
  }

  /// `emit_addimm`'s fast path, unguarded -- see
  /// `emit_binary_numeric_proven`'s docs.
  fn emit_addimm_proven(&mut self, dst: u8, a: u8, imm_const: u16) {
    let va = self.load_reg(a);
    let fa = self.to_f64(va);
    let fimm_bits = self.bake_f64_bits(imm_const);
    let fimm = self.to_f64(fimm_bits);
    let fr = self.fb.ins().fadd(fa, fimm);
    let bits = self.from_f64(fr);
    self.store_reg(dst, bits);
  }

  fn emit_addimm(&mut self, dst: u8, a: u8, imm_const: u16) {
    let va = self.load_reg(a);
    let guard = self.is_number(va);
    let snapshot = self.snapshot_reg_cache();
    let fast_block = self.fb.create_block();
    let slow_block = self.fb.create_block();
    let done_block = self.fb.create_block();
    self.fb.ins().brif(guard, fast_block, &[], slow_block, &[]);

    self.fb.switch_to_block(fast_block);
    let fa = self.to_f64(va);
    let fimm_bits = self.bake_f64_bits(imm_const);
    let fimm = self.to_f64(fimm_bits);
    let fr = self.fb.ins().fadd(fa, fimm);
    let bits = self.from_f64(fr);
    self.store_reg(dst, bits);
    self.fb.ins().jump(done_block, &[]);

    self.fb.switch_to_block(slow_block);
    let base = self.base_param;
    let dst_i = self.idx(dst);
    let a_i = self.idx(a);
    let imm_bits = self.bake_f64_bits(imm_const);
    self.call_checked(
      "zuri_jit_addimm_slow",
      &[self.vm_param, base, dst_i, a_i, imm_bits],
    );
    self.resync_dst_from_memory(dst);
    self.fb.ins().jump(done_block, &[]);

    self.fb.switch_to_block(done_block);
    self.restore_dirty_from_snapshot(&snapshot, dst);
  }

  /// `emit_imm_numeric_guarded`'s fast path, unguarded -- see
  /// `emit_binary_numeric_proven`'s docs.
  fn emit_imm_numeric_proven(
    &mut self,
    dst: u8,
    a: u8,
    imm_const: u16,
    fast: impl FnOnce(&mut Self, IrValue, IrValue) -> IrValue,
  ) {
    let va = self.load_reg(a);
    let fa = self.to_f64(va);
    let fimm_bits = self.bake_f64_bits(imm_const);
    let fimm = self.to_f64(fimm_bits);
    let fr = fast(self, fa, fimm);
    let bits = self.from_f64(fr);
    self.store_reg(dst, bits);
  }

  fn emit_imm_numeric_guarded(
    &mut self,
    dst: u8,
    a: u8,
    imm_const: u16,
    slow_helper: &'static str,
    fast: impl FnOnce(&mut Self, IrValue, IrValue) -> IrValue,
  ) {
    let va = self.load_reg(a);
    let guard = self.is_number(va);
    let snapshot = self.snapshot_reg_cache();
    let fast_block = self.fb.create_block();
    let slow_block = self.fb.create_block();
    let done_block = self.fb.create_block();
    self.fb.ins().brif(guard, fast_block, &[], slow_block, &[]);

    self.fb.switch_to_block(fast_block);
    let fa = self.to_f64(va);
    let fimm_bits = self.bake_f64_bits(imm_const);
    let fimm = self.to_f64(fimm_bits);
    let fr = fast(self, fa, fimm);
    let bits = self.from_f64(fr);
    self.store_reg(dst, bits);
    self.fb.ins().jump(done_block, &[]);

    self.fb.switch_to_block(slow_block);
    let base = self.base_param;
    let dst_i = self.idx(dst);
    let a_i = self.idx(a);
    let imm_bits = self.bake_f64_bits(imm_const);
    self.call_checked(slow_helper, &[self.vm_param, base, dst_i, a_i, imm_bits]);
    self.resync_dst_from_memory(dst);
    self.fb.ins().jump(done_block, &[]);

    self.fb.switch_to_block(done_block);
    self.restore_dirty_from_snapshot(&snapshot, dst);
  }

  /// `emit_imm_compare_guarded`'s fast path, unguarded -- see
  /// `emit_binary_numeric_proven`'s docs.
  fn emit_imm_compare_proven(
    &mut self,
    dst: u8,
    a: u8,
    imm_const: u16,
    cc: cranelift_codegen::ir::condcodes::FloatCC,
  ) {
    let va = self.load_reg(a);
    let fa = self.to_f64(va);
    let fimm_bits = self.bake_f64_bits(imm_const);
    let fimm = self.to_f64(fimm_bits);
    let cmp = self.fb.ins().fcmp(cc, fa, fimm);
    let bits = self.bool_value(cmp);
    self.store_reg(dst, bits);
  }

  fn emit_imm_compare_guarded(
    &mut self,
    dst: u8,
    a: u8,
    imm_const: u16,
    slow_helper: &'static str,
    cc: cranelift_codegen::ir::condcodes::FloatCC,
  ) {
    let va = self.load_reg(a);
    let guard = self.is_number(va);
    let snapshot = self.snapshot_reg_cache();
    let fast_block = self.fb.create_block();
    let slow_block = self.fb.create_block();
    let done_block = self.fb.create_block();
    self.fb.ins().brif(guard, fast_block, &[], slow_block, &[]);

    self.fb.switch_to_block(fast_block);
    let fa = self.to_f64(va);
    let fimm_bits = self.bake_f64_bits(imm_const);
    let fimm = self.to_f64(fimm_bits);
    let cmp = self.fb.ins().fcmp(cc, fa, fimm);
    let bits = self.bool_value(cmp);
    self.store_reg(dst, bits);
    self.fb.ins().jump(done_block, &[]);

    self.fb.switch_to_block(slow_block);
    let base = self.base_param;
    let dst_i = self.idx(dst);
    let a_i = self.idx(a);
    let imm_bits = self.bake_f64_bits(imm_const);
    self.call_checked(slow_helper, &[self.vm_param, base, dst_i, a_i, imm_bits]);
    self.resync_dst_from_memory(dst);
    self.fb.ins().jump(done_block, &[]);

    self.fb.switch_to_block(done_block);
    self.restore_dirty_from_snapshot(&snapshot, dst);
  }

  /// `EqImm`/`NeqImm` -- like the interpreter's own handler, this is
  /// pure `Value::equals` against a KNOWN-numeric constant, no operator
  /// override lookup ever. Fully inlinable with no helper fallback at
  /// all: when `a` is itself a number, real IEEE-754 equality decides
  /// it; otherwise `Value::equals` falls through to a raw bit compare
  /// (see `value.rs`), which is exactly what comparing the two `u64`s
  /// directly already gives here.
  /// `emit_imm_eq`'s `num_block` path directly -- valid only when `a`
  /// is PROVEN numeric.
  fn emit_imm_eq_proven(&mut self, dst: u8, a: u8, imm_const: u16, want_eq: bool) {
    let va = self.load_reg(a);
    let fa = self.to_f64(va);
    let imm_bits = self.bake_f64_bits(imm_const);
    let fimm = self.to_f64(imm_bits);
    let float_cc = if want_eq {
      cranelift_codegen::ir::condcodes::FloatCC::Equal
    } else {
      cranelift_codegen::ir::condcodes::FloatCC::NotEqual
    };
    let cmp_num = self.fb.ins().fcmp(float_cc, fa, fimm);
    let bits_num = self.bool_value(cmp_num);
    self.store_reg(dst, bits_num);
  }

  fn emit_imm_eq(&mut self, dst: u8, a: u8, imm_const: u16, want_eq: bool) {
    let va = self.load_reg(a);
    let is_num = self.is_number(va);
    let imm_bits = self.bake_f64_bits(imm_const);

    let num_block = self.fb.create_block();
    let raw_block = self.fb.create_block();
    let merge_block = self.fb.create_block();
    self.fb.ins().brif(is_num, num_block, &[], raw_block, &[]);

    self.fb.switch_to_block(num_block);
    let fa = self.to_f64(va);
    let fimm = self.to_f64(imm_bits);
    let float_cc = if want_eq {
      cranelift_codegen::ir::condcodes::FloatCC::Equal
    } else {
      cranelift_codegen::ir::condcodes::FloatCC::NotEqual
    };
    let cmp_num = self.fb.ins().fcmp(float_cc, fa, fimm);
    let bits_num = self.bool_value(cmp_num);
    self.store_reg(dst, bits_num);
    self.fb.ins().jump(merge_block, &[]);

    self.fb.switch_to_block(raw_block);
    let int_cc = if want_eq {
      IntCC::Equal
    } else {
      IntCC::NotEqual
    };
    let cmp_raw = self.fb.ins().icmp(int_cc, va, imm_bits);
    let bits_raw = self.bool_value(cmp_raw);
    self.store_reg(dst, bits_raw);
    self.fb.ins().jump(merge_block, &[]);

    self.fb.switch_to_block(merge_block);
  }
}

fn to_float_cc(cc: IntCC) -> cranelift_codegen::ir::condcodes::FloatCC {
  use cranelift_codegen::ir::condcodes::FloatCC;
  match cc {
    IntCC::Equal => FloatCC::Equal,
    IntCC::NotEqual => FloatCC::NotEqual,
    _ => unreachable!("emit_compare_guarded only ever passes Equal/NotEqual"),
  }
}

fn runtime_using_no_match() -> u64 {
  crate::jit::runtime::USING_NO_MATCH
}
