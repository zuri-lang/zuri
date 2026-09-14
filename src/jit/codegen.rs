//! Bytecode -> Cranelift IR translation. See `crate::jit`'s module docs
//! for the overall design; the short version this file leans on
//! throughout:
//!
//! - Every bytecode register maps to a Cranelift SSA `Variable` (`reg_vars`),
//!   tracked via `use_var`/`def_var` across basic blocks and allocated to
//!   physical CPU hardware registers by Cranelift's backtracking allocator
//!   (`regalloc2`). Memory synchronization against `VM::registers`
//!   (`flush_live`/`reload_live`) occurs strictly at FFI helper calls, GC
//!   safepoints, deoptimizations, and function returns.
//! - One Cranelift `Block` per bytecode instruction index, so a
//!   backward/forward `Instr::Jmp`-family target is always just "the
//!   block at that index"; no separate control-flow-graph
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
use rustc_hash::{FxHashMap, FxHashSet};
use std::sync::atomic::AtomicBool;

use crate::jit::{CallTarget, CompileFacts, escape, typeflow};
use crate::vm::chunk::{Instr, ParamType};
use crate::vm::object::{self, ObjFunction};
use crate::vm::value::{self};
use crate::vm::vm;

/// Byte offset (from a `*mut VM`) of the cached registers pointer
/// (see `vm::VM::regs_ptr_cache`'s docs). Read directly by compiled code
/// (entry-block init and `refresh_regs`) instead of calling into Rust,
/// since this is re-fetched at essentially every helper-call site.
const REGS_PTR_CACHE_OFFSET: i32 = vm::VM_REGS_PTR_CACHE_OFFSET as i32;
/// Where `publish_ip` writes this compiled frame's current bytecode
/// position: see `VM::jit_ip`.
const JIT_IP_OFFSET: i32 = vm::VM_JIT_IP_OFFSET as i32;
/// Byte offset of `VM::global_slots_ptr_cache`: see that field's own
/// docs and `emit_get_global`'s use of it.
const GLOBAL_SLOTS_PTR_CACHE_OFFSET: i32 = vm::VM_GLOBAL_SLOTS_PTR_CACHE_OFFSET as i32;
/// Byte offset of `VM::method_table_generation`: see that field's own
/// docs and `emit_self_invoke`'s use of it.
const METHOD_TABLE_GENERATION_OFFSET: i32 = vm::VM_METHOD_TABLE_GENERATION_OFFSET as i32;
/// Byte offset of `VM::interned_ascii`: see that field's own docs.
const INTERNED_ASCII_OFFSET: i32 = vm::VM_INTERNED_ASCII_OFFSET as i32;
/// Byte offset (from a `*mut VM`) of the frame stack's own data
/// pointer; `VM_FRAMES_OFFSET + FRAMESTACK_PTR_OFFSET`, combined once
/// here so every call site just uses the finished number. See
/// `emit_inline_call`'s own docs for what this backs.
const FRAMES_PTR_OFFSET: i32 = (vm::VM_FRAMES_OFFSET + vm::FRAMESTACK_PTR_OFFSET) as i32;
/// Byte offset (from a `*mut VM`) of the frame stack's current length.
const FRAMES_LEN_OFFSET: i32 = (vm::VM_FRAMES_OFFSET + vm::FRAMESTACK_LEN_OFFSET) as i32;
/// Byte offset (from a `*mut VM`) of the frame stack's current
/// capacity.
const FRAMES_CAP_OFFSET: i32 = (vm::VM_FRAMES_OFFSET + vm::FRAMESTACK_CAP_OFFSET) as i32;
/// Byte offset (from a `*mut VM`) of `VM::regs_len_cache`.
const REGS_LEN_CACHE_OFFSET: i32 = vm::VM_REGS_LEN_CACHE_OFFSET as i32;
/// Byte offset (from a `*mut VM`) of `VM::jit_scalar_roots_len`.
const JIT_SCALAR_ROOTS_LEN_OFFSET: i32 = vm::VM_JIT_SCALAR_ROOTS_LEN_OFFSET as i32;
/// Byte offset (from a `*mut VM`) of `VM::has_open_upvalues`.
const HAS_OPEN_UPVALUES_OFFSET: i32 = vm::VM_HAS_OPEN_UPVALUES_OFFSET as i32;
/// Byte offset (from a `*mut VM`) of `VM::pending_deopt_ip`.
const PENDING_DEOPT_IP_OFFSET: i32 = vm::VM_PENDING_DEOPT_IP_OFFSET as i32;
/// Byte offset (from a `*mut VM`) of `VM::jit_pending_error`.
const JIT_PENDING_EXCEPTION_OFFSET: i32 = vm::VM_JIT_PENDING_EXCEPTION_OFFSET as i32;
/// Byte offset (from a `*mut VM`) of `VM::jit_call_depth`.
const JIT_CALL_DEPTH_OFFSET: i32 = vm::VM_JIT_CALL_DEPTH_OFFSET as i32;
/// Byte offsets of each `CallFrame` field, relative to one frame slot's
/// own address (`frames_ptr + index * CALL_FRAME_SIZE`): see
/// `vm::CALL_FRAME_*_OFFSET`'s own docs.
const CALL_FRAME_FUNCTION_OFFSET: i32 = vm::CALL_FRAME_FUNCTION_OFFSET as i32;
const CALL_FRAME_CLOSURE_VAL_OFFSET: i32 = vm::CALL_FRAME_CLOSURE_VAL_OFFSET as i32;
const CALL_FRAME_IP_OFFSET: i32 = vm::CALL_FRAME_IP_OFFSET as i32;
const CALL_FRAME_BASE_OFFSET: i32 = vm::CALL_FRAME_BASE_OFFSET as i32;
const CALL_FRAME_DST_IN_CALLER_OFFSET: i32 = vm::CALL_FRAME_DST_IN_CALLER_OFFSET as i32;
const CALL_FRAME_SCALAR_ROOTS_MARK_OFFSET: i32 = vm::CALL_FRAME_SCALAR_ROOTS_MARK_OFFSET as i32;
const CALL_FRAME_COMPILED_OFFSET: i32 = vm::CALL_FRAME_COMPILED_OFFSET as i32;
const CALL_FRAME_SIZE: i64 = vm::CALL_FRAME_SIZE as i64;
/// Byte offset (from a `*const ObjFunction`) of its own compiled-entry
/// cell: see `object::obj_function_jit_entry_offset`'s own docs for
/// why `emit_inline_construct` reads this fresh on every call instead
/// of baking it, unlike `emit_known_call`'s `entry`.
const PROTO_JIT_ENTRY_OFFSET: i32 = object::obj_function_jit_entry_offset() as i32;
/// Byte offsets (from a `*mut VM`) of `Heap::bytes_allocated`/`next_gc`
/// (major) and `young_bytes_allocated` (minor); lets `emit_safepoint`
/// inline both `Heap::needs_major_gc()`/`needs_minor_gc()` checks
/// (three loads + two compares; the young threshold itself is a
/// compile-time immediate, see `Heap::YOUNG_NEXT_GC`) instead of an
/// unconditional FFI call on every loop back-edge and call site, only
/// actually calling into Rust on the rare branch where a collection
/// (of either kind) is really about to happen.
const HEAP_JIT_GC_NEEDED_OFFSET: i32 =
  (vm::VM_HEAP_OFFSET + object::HEAP_JIT_GC_NEEDED_OFFSET) as i32;
const HEAP_NURSERY_CUR_OFFSET: i32 = (vm::VM_HEAP_OFFSET + object::HEAP_NURSERY_CUR_OFFSET) as i32;
const HEAP_NURSERY_END_OFFSET: i32 = (vm::VM_HEAP_OFFSET + object::HEAP_NURSERY_END_OFFSET) as i32;
const HEAP_YOUNG_BYTES_ALLOCATED_OFFSET: i32 =
  (vm::VM_HEAP_OFFSET + object::HEAP_YOUNG_BYTES_ALLOCATED_OFFSET) as i32;
const HEAP_BYTES_ALLOCATED_OFFSET: i32 =
  (vm::VM_HEAP_OFFSET + object::HEAP_BYTES_ALLOCATED_OFFSET) as i32;
const HEAP_LIVE_COUNT_OFFSET: i32 = (vm::VM_HEAP_OFFSET + object::HEAP_LIVE_COUNT_OFFSET) as i32;

/// Compiles `proto`'s bytecode into `fb`'s function body. Returns the
/// bytecode-ip -> osr-id map (`CompiledFunction::osr_ids`) on success,
/// or a human-readable ineligibility reason on failure; the latter is
/// ALWAYS a permanent, sticky "never try this prototype again" signal
/// (see `VM::try_compile`), never a transient error.
///
/// `speculative_params` (bit `r` = fixed-arity parameter register `r`)
/// is a ONE-SHOT type sample of the actual call that triggered this
/// compilation (see `VM::try_compile`'s own docs); when non-empty, a
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
  shutdown: Option<&AtomicBool>,
) -> Result<FxHashMap<usize, i32>, String> {
  // A function that establishes a CATCH handler is still never compiled:
  // `PushCatch`/`PopCatch` maintain unwind state the interpreter owns,
  // and this compiler generates no unwind logic (see the `jit` module
  // docs).
  //
  // `Instr::Raise` on its own is different, and treating it the same way
  // was costing real time. A raise is almost always an error path --
  // `raise Error("bad task id")` guarding a lookup that never fails in
  // practice; but its mere presence disqualified the ENTIRE function,
  // including the hot path around it. Richards spent ~30% of its runtime
  // in the interpreter for exactly this reason: one unreachable `raise`
  // inside `findtcb`, which its hottest method calls per packet.
  //
  // So a raise now compiles to a deopt (see `emit_deopt`): bail to the
  // interpreter at that bytecode position and let it do the raising and
  // unwinding it already knows how to do. The cost lands on the path
  // that actually raises, where it belongs, instead of on every call.
  for instr in &proto.chunk.code {
    if matches!(instr, Instr::PushCatch { .. } | Instr::PopCatch) {
      return Err("contains a catch handler (PushCatch/PopCatch)".to_string());
    }
  }

  let code_len = proto.chunk.code.len();
  if code_len == 0 {
    return Err("empty function body".to_string());
  }
  // A function's own register window is addressed with a `u8` offset
  // throughout the bytecode (`Instr`'s fields), and this compiler bakes
  // register indices as plain `iconst` immediates; nothing here
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
    shutdown,
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
///   the last time it matched memory; a genuine `store_reg`-to-memory
///   is owed before any point that needs memory to be authoritative
///   (a call, a GC safepoint, a deopt, a return).
/// - `Stale`: memory may have changed since the `Variable` was last
///   established (a call/safepoint just ran, and this register was
///   live through it); the `Variable`'s value must NOT be trusted;
///   the next `load_reg` for it must issue a real memory load.
///
/// `Clean` and `Dirty` are collapsed into ONE "trust the `Variable`"
/// branch in `load_reg`; they only differ in whether `flush_live`
/// still owes a write, never in whether a READ can trust the cache.
/// A builtin native emitted inline: see
/// `FuncCompiler::native_intrinsic`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum NativeIntrinsic {
  IsNumber,
  IsBool,
  IsObject,
  IsInt,
  /// `is_obj()` AND the object's tag is one of these.
  Tag(&'static [u8]),
}

/// A register's shape, proven for the WHOLE function body by a
/// non-nullable, single-type `Instr::CheckParamType` on it plus a
/// whole-bytecode scan proving nothing ever writes that register again
///: see `FuncCompiler::compute_proven_shapes`'s own docs for exactly
/// what's required. Consulted by `emit_ic_guard` to skip a guard the
/// caller already paid for once, at the parameter check. The `List`
/// equivalent of this used to live here too, but is now
/// `typeflow::ListFacts`; a real per-`ip` dataflow proof, sound
/// across arbitrary reassignment/branches/loops, not just this
/// whole-function-scoped approximation: see its own docs.
#[derive(Clone, Copy, PartialEq, Eq)]
enum ParamShape {
  /// SOME instance; not necessarily of the exact class a given
  /// `GetField`/`SetField` site's own inline cache expects. Only
  /// removes the `is_obj`+tag half of that site's guard; the per-site
  /// class compare still runs exactly as before.
  Instance,
}

/// A `Number` builtin the JIT emits directly instead of dispatching
/// to: see `FuncCompiler::emit_number_intrinsic` for why every one of
/// these is an IDENTITY with what `builtins::number` computes, never an
/// approximation of it.
///
/// Three shapes, by what the method actually is:
///
/// - `Inline`: the whole method is one Cranelift instruction with
///   exactly matching IEEE-754 semantics. Compiles to one machine
///   instruction and nothing else.
/// - `InlinePredicate`/`Sign`/`Int`: still no call, but a short fixed
///   instruction sequence rather than a single opcode; a comparison
///   producing a `Value::bool`, or a `select` chain.
/// - `Call`: no machine instruction computes it (every transcendental),
///   so this calls the SAME `f64` method `builtins::number` calls, via
///   a dedicated `jit::runtime` helper. The win here is not a faster
///   `sin`; it is the same `sin`; it is skipping method resolution,
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
  /// `n.sign()`; `-1`/`0`/`1` with a zero's own sign preserved, and
  /// `-1` for NaN (both comparisons below are false for NaN), matching
  /// `builtins::number::sign` exactly, which is deliberately NOT
  /// `f64::signum`.
  Sign,
  /// `n.int()`; Rust's `as i64` cast is saturating with NaN mapping
  /// to zero, which is precisely `fcvt_to_sint_sat`'s own definition.
  Int,
  /// `a.max(b)` inlined into Cranelift fmax.
  Max,
  /// `a.min(b)` inlined into Cranelift fmin.
  Min,
  /// `n.round()`; round half away from zero, matching `f64::round` exactly.
  Round,
  /// A direct call to the named `jit::runtime` helper: `(vm, bits)` for
  /// `arity` 0, `(vm, recv_bits, arg_bits)` for `arity` 1.
  Call { helper: &'static str, arity: u8 },
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
  /// `x != x`; true for exactly the NaNs.
  IsNan,
  /// `|x| == inf`.
  IsInf,
  /// `|x| < inf`; false for both infinities AND for NaN, matching
  /// `f64::is_finite`.
  IsFinite,
  /// `builtins::number::to_bool`'s own rule: `n >= 0.0` (so NaN is
  /// false, exactly as Rust's `>=` gives).
  NonNegative,
}

impl NumberIntrinsic {
  /// How many arguments the call site must supply for `name` to be this
  /// intrinsic; checked by the caller before anything else, so e.g. a
  /// stray `x.sqrt(1)` falls through to the ordinary dispatch and gets
  /// the real arity error.
  fn arity(self) -> u8 {
    match self {
      NumberIntrinsic::Call { arity, .. } => arity,
      NumberIntrinsic::Max | NumberIntrinsic::Min => 1,
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
      "max" => Max,
      "min" => Min,

      "sin" => Call {
        helper: "zuri_jit_num_sin",
        arity: 0,
      },
      "cos" => Call {
        helper: "zuri_jit_num_cos",
        arity: 0,
      },
      "tan" => Call {
        helper: "zuri_jit_num_tan",
        arity: 0,
      },
      "sinh" => Call {
        helper: "zuri_jit_num_sinh",
        arity: 0,
      },
      "cosh" => Call {
        helper: "zuri_jit_num_cosh",
        arity: 0,
      },
      "tanh" => Call {
        helper: "zuri_jit_num_tanh",
        arity: 0,
      },
      "asin" => Call {
        helper: "zuri_jit_num_asin",
        arity: 0,
      },
      "acos" => Call {
        helper: "zuri_jit_num_acos",
        arity: 0,
      },
      "atan" => Call {
        helper: "zuri_jit_num_atan",
        arity: 0,
      },
      "asinh" => Call {
        helper: "zuri_jit_num_asinh",
        arity: 0,
      },
      "acosh" => Call {
        helper: "zuri_jit_num_acosh",
        arity: 0,
      },
      "atanh" => Call {
        helper: "zuri_jit_num_atanh",
        arity: 0,
      },
      "exp" => Call {
        helper: "zuri_jit_num_exp",
        arity: 0,
      },
      "expm1" => Call {
        helper: "zuri_jit_num_expm1",
        arity: 0,
      },
      "log" => Call {
        helper: "zuri_jit_num_log",
        arity: 0,
      },
      "log2" => Call {
        helper: "zuri_jit_num_log2",
        arity: 0,
      },
      "log10" => Call {
        helper: "zuri_jit_num_log10",
        arity: 0,
      },
      "log1p" => Call {
        helper: "zuri_jit_num_log1p",
        arity: 0,
      },
      "cbrt" => Call {
        helper: "zuri_jit_num_cbrt",
        arity: 0,
      },
      "round" => Round,

      "atan2" => Call {
        helper: "zuri_jit_num_atan2",
        arity: 1,
      },

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

/// A List builtin the JIT emits directly instead of dispatching to;
/// `NumberIntrinsic`'s counterpart, same reasoning: skip real method-
/// name lookup (`builtins::lookup`'s hash + `memcmp`, what
/// `builtins::method_table_key`'s own docs call out `.length()` in a
/// loop as paying on every single iteration) AND the generic invoke
/// call machinery entirely, for a method simple enough to just be a
/// couple of loads off the list header.
///
/// Every variant here reads `Obj::List`'s own header fields (data
/// pointer, length); never allocates, never can raise for a genuine
/// list receiver, so there's nothing here shaped like
/// `NumberIntrinsic::Call`; if a List method ever needs a real helper
/// call (`.append()`, say), it'd need its own variant the same way
/// `NumberIntrinsic::Call` earns its keep for the transcendentals.
/// Dict methods the JIT recognises by name at compile time and
/// dispatches itself, the dict counterpart to `ListIntrinsic`.
///
/// `get` earns its place on measurement: `d.get(k, fallback)` is how
/// any counting loop reads a running total, and taking it through the
/// generic invoke path costs about as much again as the dict lookup
/// itself (~36ns against ~93ns for the equivalent indexing
/// instructions, most of a third of such a loop).
#[derive(Clone, Copy, PartialEq, Eq)]
///
/// `length`/`is_empty` are handled by `emit_list_intrinsic`'s own tag
/// chain instead, because `ListIntrinsic` claims those names earlier
/// in the dispatch order.
enum DictIntrinsic {
  /// `x.get(key)` and `x.get(key, fallback)`. Not dict-only despite
  /// living here: lists and byte streams answer `get` too, so
  /// `emit_dict_intrinsic` tag-chains all three. See its own docs for
  /// how far their meanings diverge.
  Get,
  /// `d.set(key, value)`, and `d.add(key, value)`, which
  /// `builtins::dict` implements as the very same function.
  Set,
  Contains,
}

impl DictIntrinsic {
  /// `get` is legal with or without a fallback, so arity alone does
  /// not settle a match the way it does for the other intrinsics.
  fn accepts_arity(self, num_args: u8) -> bool {
    match self {
      DictIntrinsic::Get => num_args == 1 || num_args == 2,
      DictIntrinsic::Contains => num_args == 1,
      DictIntrinsic::Set => num_args == 2,
    }
  }

  fn of(name: &str) -> Option<DictIntrinsic> {
    Some(match name {
      "get" => DictIntrinsic::Get,
      "set" | "add" => DictIntrinsic::Set,
      "contains" => DictIntrinsic::Contains,
      _ => return None,
    })
  }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum ListIntrinsic {
  Length,
  IsEmpty,
  /// `nil` for an empty list; matches `builtins::list::first`.
  First,
  /// `nil` for an empty list; matches `builtins::list::last`.
  Last,
  Append,
}

impl ListIntrinsic {
  /// Same role as `NumberIntrinsic::arity`.
  fn arity(self) -> u8 {
    match self {
      ListIntrinsic::Length
      | ListIntrinsic::IsEmpty
      | ListIntrinsic::First
      | ListIntrinsic::Last => 0,
      ListIntrinsic::Append => 1,
    }
  }

  fn of(name: &str) -> Option<ListIntrinsic> {
    Some(match name {
      "length" => ListIntrinsic::Length,
      "is_empty" => ListIntrinsic::IsEmpty,
      "first" => ListIntrinsic::First,
      "last" => ListIntrinsic::Last,
      "append" => ListIntrinsic::Append,
      _ => return None,
    })
  }
}

/// `ListIntrinsic`'s counterpart for a String receiver; same
/// reasoning (skip `builtins::lookup` and the generic invoke machinery
/// for a method simple enough to read straight off the string's own
/// header, via `object::obj_str_ptr_offset`/`obj_str_len_offset`).
///
/// Only these two: every OTHER `STRING_METHODS` entry either allocates
/// a new string (`upper`/`lower`/`trim`/`replace`/`split`/`join`/...)
/// or needs real per-byte work beyond a length check (`is_alpha`,
/// regex methods, ...); neither shape belongs here for the same
/// reason `NumberIntrinsic` excludes anything that allocates or can
/// raise. `Length` still isn't a bare header load the way `ListIntrinsic
/// ::Length` is, though: Zuri's `.length()` counts CODEPOINTS, not
/// bytes, so it's the one variant here that compiles to a real loop
/// (see `emit_string_intrinsic_value`) rather than a fixed instruction
/// sequence; still no allocation and no way to raise for a genuine
/// string receiver, so it keeps the same "no safepoint owed" property
/// every other intrinsic here has.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum StringIntrinsic {
  /// Codepoint count, matching `builtins::string::length` exactly
  /// (`s.chars().count()`); NOT the byte length `obj_str_len_offset`
  /// reads directly.
  Length,
  /// Byte-length-zero, which is equivalent to codepoint-count-zero for
  /// any valid UTF-8 string (an empty byte sequence has no codepoints
  /// and vice versa); so unlike `Length`, this needs no loop at all.
  IsEmpty,
  /// The codepoint of a one-character string. Unlike the other two this
  /// one can legitimately fail (`builtins::string::ord` raises unless
  /// the receiver is exactly one character), so it carries its own
  /// guard and slow arm rather than going through
  /// `emit_string_intrinsic_value`.
  Ord,
}

impl StringIntrinsic {
  /// Same role as `ListIntrinsic::arity`.
  fn arity(self) -> u8 {
    0
  }

  fn of(name: &str) -> Option<StringIntrinsic> {
    Some(match name {
      "length" => StringIntrinsic::Length,
      "is_empty" => StringIntrinsic::IsEmpty,
      "ord" => StringIntrinsic::Ord,
      _ => return None,
    })
  }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum IterableIntrinsic {
  Key,
  Value,
}

impl IterableIntrinsic {
  fn of(name: &str) -> Option<IterableIntrinsic> {
    match name {
      "@key" => Some(IterableIntrinsic::Key),
      "@value" => Some(IterableIntrinsic::Value),
      _ => None,
    }
  }
}

struct FuncCompiler<'a, 'b> {
  fb: &'a mut FunctionBuilder<'b>,
  module: &'a mut JITModule,
  helpers: &'a FxHashMap<&'static str, FuncId>,
  proto: &'a ObjFunction,
  /// One block per bytecode instruction index; `blocks[ip]` is where
  /// that instruction's own codegen begins, and the only valid jump
  /// target for anything (a `Jmp`-family instruction, or the OSR
  /// dispatch below) that wants to reach bytecode position `ip`.
  blocks: Vec<Block>,
  /// Holds the CURRENT `VM::registers` base pointer: see this file's
  /// module docs. Refreshed (via `refresh_regs`) after every call to a
  /// `jit::runtime` helper, since any of them can transitively push a
  /// deeper call frame and reallocate `VM::registers`.
  regs_var: Variable,
  vm_param: IrValue,
  /// This frame's absolute register-window start, as the RAW INDEX
  /// (not multiplied by 8); what every helper call passes as `base`.
  base_param: IrValue,
  /// `base_param * 8`, computed once in the entry block (which
  /// dominates every other block, so this `Value` stays valid
  /// everywhere); the byte offset added to the current `regs_var`
  /// pointer to address this frame's own registers directly.
  base_bytes: IrValue,
  closure_param: IrValue,
  osr_ids: FxHashMap<usize, i32>,
  /// Cached `SigRef` for `jit::EntryFn`'s own shape
  /// (`(i64,i64,i64,i32)->i64`); imported at most once per compiled
  /// function (see `entry_sig_ref`), then reused by every
  /// `call_indirect` this function's own fast, inline-cache-style call
  /// sites need (`emit_fast_call`).
  entry_sig: Option<SigRef>,
  /// An 8-byte scratch stack slot, allocated at most once per compiled
  /// function and reused by every fast-call site
  /// (`closure_out_addr`/`emit_fast_call`); the out-parameter
  /// `zuri_jit_call_prepare`/`zuri_jit_invoke_prepare` write the
  /// resolved callee closure's `Value` bits into, since (unlike
  /// `Instr::Call`, where the callee already sits in an ordinary
  /// register) `Instr::Invoke`'s resolved METHOD closure exists only
  /// inside the helper's own class-method-table lookup, with no
  /// register holding it for generated code to read back directly.
  closure_out_slot: Option<StackSlot>,
  /// Same lazy-per-function pattern as `closure_out_slot`, for the
  /// list-index fast path's `zuri_jit_list_data` out-parameter (the
  /// resolved list's current length: see that function's own docs).
  /// Which registers are PROVEN numeric at each bytecode position, for
  /// WHICHEVER body (general or specialized) is currently being
  /// populated: see `jit::typeflow`'s own docs and `run`'s two-pass
  /// structure. Consulted before emitting any guarded arithmetic op:
  /// when every operand is proven, the guard and its slow-path
  /// fallback are skipped entirely (they'd never be taken), leaving
  /// unconditional straight-line float math.
  type_facts: typeflow::TypeFacts,
  /// Which registers are PROVEN to hold a whole-number-valued `f64`
  /// (not just numeric) at each bytecode position: see `jit::
  /// typeflow::IntFacts`'s own docs. Consulted by `emit_list_get_index`/
  /// `emit_list_set_index` to skip the float-roundtrip "is this
  /// actually a whole number" check entirely for a plain loop-counter
  /// index. Computed once per body alongside `type_facts`, same
  /// lifetime and the same reason it isn't shared between the general
  /// and specialized body.
  int_facts: typeflow::IntFacts,
  /// Which registers are PROVEN to hold an `Obj::List` at each
  /// bytecode position: see `jit::typeflow::ListFacts`'s own docs.
  /// Consulted by `emit_list_get_index`/`emit_list_set_index` (skips
  /// the object-shape half of the guard) and `ListIntrinsic` call
  /// sites (skips method-name lookup entirely). Same lifetime/scope
  /// as `type_facts`/`int_facts`.
  list_facts: typeflow::ListFacts,
  /// Which registers are PROVEN to hold a `Value::String` at each
  /// bytecode position: see `jit::typeflow::StringFacts`'s own docs.
  /// Consulted by `Instr::Invoke` to route straight to `emit_string_
  /// invoke`, skipping the wasted `zuri_jit_invoke_prepare` attempt a
  /// String receiver can never satisfy. Same lifetime/scope as
  /// `type_facts`/`int_facts`/`list_facts`.
  string_facts: typeflow::StringFacts,
  /// Which registers hold a `bytes` at each bytecode position; the
  /// bytes sibling of `list_facts`/`string_facts`.
  bytes_facts: typeflow::BytesFacts,
  /// Which registers hold a dict; seeded by `MakeDict` the way
  /// `list_facts` is by `MakeList`.
  dict_facts: typeflow::DictFacts,
  /// Which registers provably hold a whole number in 0..=255, so a
  /// byte store can skip checking.
  byte_value_facts: typeflow::ByteValueFacts,
  bool_facts: typeflow::BoolFacts,
  /// Which registers are PROVEN to hold one exact, statically-known
  /// `f64` constant at each bytecode position: see `jit::typeflow::
  /// ConstFacts`'s own docs. Consulted by `div_by_pow2_reciprocal` to
  /// see PAST a constant hoisted into a local and reused (e.g. across a
  /// loop's own divisions), not just an immediately-preceding
  /// `LoadConst`. Same lifetime/scope as `type_facts`/`int_facts`.
  const_facts: typeflow::ConstFacts,
  /// `typeflow::build_predecessors(proto)`, computed once here and
  /// shared by every dataflow pass that needs it (`type_facts`/
  /// `int_facts`/`list_facts`/`liveness`, and `merge_points`' own
  /// predecessor counts in `run`); it depends only on `proto`'s
  /// control flow, never on what any one of those passes is proving,
  /// so recomputing it per-pass was pure repeated work paid on every
  preds: Vec<Vec<usize>>,
  self_ref_facts: std::cell::OnceCell<Vec<crate::jit::escape::MustSet>>,
  self_summary: std::cell::OnceCell<crate::jit::escape::FuncEscapeSummary>,
  /// A ONE-SHOT type sample of the call that triggered this
  /// compilation (bit `r` = fixed-arity parameter register `r` held a
  /// number); `None` after filtering out an all-zero sample. See
  /// `compile`'s own docs and `emit_entry_dispatch`.
  speculative_params: Option<u64>,
  speculative_lists: Option<u64>,
  speculative_ints: Option<u64>,
  spec_int_facts: Option<typeflow::IntFacts>,
  /// A ONE-SHOT, WHOLE-FRAME type sample taken at the same moment as
  /// `speculative_params`, but covering every register in the
  /// triggering frame rather than only the fixed-arity parameters --
  /// see `jit::typeflow::SpeculativeRegs`'s own docs. `None` after
  /// filtering out an all-zero sample.
  speculative_regs: Option<typeflow::SpeculativeRegs>,
  /// One persistent Cranelift Variable per bytecode register, live
  /// for the whole compiled function.
  reg_vars: Vec<Variable>,
  /// A SECOND persistent Variable per bytecode register, typed `F64`
  /// instead of `I64`, holding the exact same 64 bits. Only allocated
  /// for the registers `f64_tracked` marks (the ones some float
  /// instruction actually reads or writes); the rest get a dummy that
  /// is never defined or used.
  ///
  /// This exists because Cranelift lowers an `I64 <-> F64` `bitcast`
  /// to a real cross-domain `movq` on x86-64 and has no rule anywhere
  /// that cancels a `bitcast(bitcast(x))` pair. Since every bytecode
  /// instruction gets its own Cranelift block here, a float value
  /// living only in an `I64` Variable pays TWO of those moves per
  /// arithmetic op (one to get into an xmm register, one to get back
  /// out) and can never stay resident in the vector register file
  /// across a loop. Keeping a parallel `F64` Variable lets the float
  /// side of a register flow xmm-to-xmm with no move at all.
  ///
  /// Both Variables are defined at every definition site, so either
  /// one may be read at any point; a register's two views can never
  /// disagree. Where the `I64` view turns out to be dead, Cranelift's
  /// own DCE drops its `bitcast` chain and the register becomes purely
  /// float-resident; where it isn't, the cost is exactly what it was
  /// before this existed.
  reg_vars_f64: Vec<Variable>,
  reg_vars_int: Vec<Variable>,
  int_tracked: Vec<bool>,
  /// Registers that some float-path instruction reads or writes, found
  /// by a plain syntactic scan of the bytecode. Purely a budget on how
  /// many `reg_vars_f64` entries are worth materializing: a register
  /// left out of this set just behaves exactly as it did before the
  /// float Variables existed, so a miss here costs speed, never
  /// correctness.
  f64_tracked: Vec<bool>,
  /// Each register's integer value exactly as the entry block
  /// established it, before any body has run.
  ///
  /// `emit_entry_dispatch` needs the raw bits to build its
  /// `is_number` guards from, and going through `load_reg` for that
  /// would rebuild them out of the float view for a canonical
  /// register: the entry block has just moved the value INTO an xmm
  /// register, so the guard would move it straight back out again.
  /// Two `vmovq`s per speculated parameter, on every single call.
  /// These values dominate every dispatch block, so using them
  /// directly is both cheaper and simpler.
  entry_reg_values: Vec<IrValue>,
  /// The subset of `f64_tracked` whose float Variable is the register's
  /// ONLY definition, with the integer view rebuilt on demand at the
  /// few places that still want one.
  ///
  /// Dual-defining is already never worse than the old integer-only
  /// scheme, but it does leave one `bitcast` per definition alive
  /// whenever anything downstream reads the integer view; for a loop
  /// accumulator that read is usually a single `return` outside the
  /// loop, and paying for it on every iteration is exactly backwards.
  /// A register `type_facts` proves numeric everywhere it is live can
  /// invert that trade safely: it never holds a tagged pointer, so its
  /// integer view has no readers other than genuine boundaries (a
  /// safepoint flush, a helper call, a return), and rebuilding it
  /// there costs one `bitcast` on a path that was already leaving the
  /// vector registers anyway.
  ///
  /// Recomputed for the specialized body, which proves strictly more
  /// than the general one does; sound because the two bodies are
  /// disjoint subgraphs that share no edge, and both are dominated by
  /// the entry block, where every register gets both views seeded.
  f64_canonical: Vec<bool>,
  global_vars: FxHashMap<i64, Variable>,
  /// The bytecode position emit_instruction is currently translating.
  current_ip: usize,
  /// Which registers are live at every bytecode position.
  liveness: typeflow::LivenessFacts,
  /// Field name -> slot index, for every field on `self`'s (register
  /// 0's) class that's safe to read/write directly, with no
  /// `BoundMethod`-wrapping risk; resolved once, before compilation
  /// starts, by `VM::resolve_self_field_slots` (which has the VM access
  /// this module deliberately never touches). Empty (not `None`) for a
  /// plain function or when resolution couldn't prove anything safe --
  /// `Instr::GetField`/`SetField` on `self` just falls through to the
  /// general helper path in that case, identical to before this field
  /// existed.
  self_field_slots: FxHashMap<String, u16>,
  self_numeric_fields: FxHashSet<String>,
  /// `self_field_slots`' counterpart for a typed, non-`self` parameter
  /// register: see `jit::CompileFacts::param_field_slots`'s own docs.
  param_field_slots: FxHashMap<u8, (u64, FxHashMap<String, u16>)>,
  /// Register -> (construct ip, class bits, field slots) for an
  /// instance built in this very function, where the construct is the
  /// last write to that register. An instance cannot change class, so
  /// this is as firm a fact as a type annotation; the class is still
  /// checked once at the access, which is what makes it sound on a
  /// path that somehow skipped the construct.
  construct_field_slots: FxHashMap<u8, (usize, u64, FxHashMap<String, u16>)>,
  /// This compiled function's OWN `FuncId` in `module`; known before
  /// codegen starts (the caller, `JitEngine::build_ir`, always declares
  /// it first). Lets `emit_call_instr`'s self-recursive case emit a
  /// genuine relocation-resolved direct `call` (via
  /// `Module::declare_func_in_func` on this SAME id) rather than an
  /// indirect call through a runtime-loaded pointer.
  own_func_id: FuncId,
  /// `(self`'s own class as `Value` bits, the `VM::
  /// method_table_generation` snapshot taken alongside it)`: see
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
  /// reading it: see `scalar_replace_eligible`'s own docs for exactly
  /// what that buys). Consulted by `Instr::GetIndex`/`SetIndex` to
  /// route to `emit_scalar_list_get`/`emit_scalar_list_set` instead of
  /// the general, real-`Obj::List`-assuming fast path. A register is
  /// NEVER removed from this map once scalar-replaced (the eligibility
  /// check's "no `Move` ever reads it" requirement means nothing else
  /// could ever need to reuse this register for something unrelated
  /// that would make a STALE entry here observably wrong).
  scalar_lists: FxHashMap<u8, (StackSlot, u8)>,
  /// Per-construction-site class facts: see
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
  /// The scalar-construct sites decided before any code is emitted,
  /// with their stack slots. Separate from `scalar_instances` because
  /// that one is deliberately invalidated by any write to the register
  /// (`store_reg`), and the construct idiom writes the register once
  /// before the call to load the class. The plan has to outlive that;
  /// the OSR entry paths are built from it.
  planned_scalar_instances: FxHashMap<u8, (StackSlot, usize)>,
  /// Can any upvalue ever be OPEN over this frame's own registers?
  ///
  /// Only `Instr::Closure` opens one (it is the sole caller of
  /// `VM::capture_upvalue`, both interpreted and via
  /// `jit::runtime::zuri_jit_make_closure`), and the index it captures
  /// is always inside the frame that executes it. So a function whose
  /// bytecode contains no `Instr::Closure` at all cannot have a single
  /// open upvalue pointing into its window, and its `Instr::Return`
  /// has nothing to close; a callee's own upvalues live at strictly
  /// higher indices and are already closed by that callee's return.
  ///
  /// Worth proving statically rather than letting the helper discover
  /// it at runtime: the close is emitted on EVERY return, and going
  /// through `call_checked` costs an ABI call, an error-status branch,
  /// and a full `refresh_regs` reload, all to walk an empty list. The
  /// overwhelming majority of functions; every one that never builds
  /// a closure; pay that on every single call for nothing.
  frame_can_open_upvalues: bool,
  /// Registers whose shape is proven for the whole function: see
  /// `ParamShape`'s own docs and `compute_proven_shapes`. Computed
  /// once, in `new`, from the bytecode's own shape alone (not
  /// dependent on `type_facts`/`speculative_*`, unlike most of this
  /// struct's other per-body state), so it's identical for both the
  /// general and specialized body if this compile has one.
  proven_param_shapes: FxHashMap<u8, ParamShape>,
  numeric_fields: FxHashSet<String>,
  global_lists: FxHashSet<String>,
  guarded_instance_vars: FxHashMap<u8, (Variable, Variable, Variable)>,
  active_guarded: FxHashSet<u8>,
  known_classes: FxHashMap<u64, FxHashMap<String, u16>>,
  /// Where an earlier compilation of this function gave up; see
  /// `JitInfo::deopt_sites`. A speculation covering one of these has
  /// already been contradicted by real input, so it isn't made again.
  deopt_sites: FxHashSet<usize>,
  /// Receiver registers named by those sites. A field access that bet
  /// wrong is rarely alone: every field read off the same value is a
  /// separate site that will bet the same way and be wrong the same
  /// way, so giving up on the register rather than the instruction
  /// settles the whole group in one recompilation instead of one per
  /// field.
  deopt_receivers: FxHashSet<u8>,
  /// Compile this function with no field-class bets at all; see
  /// `JitInfo::field_speculation_off`.
  field_speculation_off: bool,
  /// See `jit::CompileFacts::self_method_protos`.
  self_method_protos: FxHashMap<String, usize>,
  is_specialized_pass: bool,
  active_guarded_classes: FxHashMap<u8, u64>,
  shutdown: Option<&'a AtomicBool>,
}

impl<'a, 'b> FuncCompiler<'a, 'b> {
  /// A register is trustworthy for the WHOLE function as `shape` in
  /// two cases, both deliberately simple whole-function checks rather
  /// than a real per-`ip` dataflow pass (see below for why that's
  /// still sound):
  ///
  /// - It's the target of a non-nullable `Instr::CheckParamType` whose
  ///   declared type is EXACTLY one member (a union like `list|dict`
  ///   proves neither alone), and nothing anywhere in the function
  ///   ever writes that register again (`typeflow::any_dst`).
  /// - It's a LOCAL built from a list literal (`Instr::MakeList`) or a
  ///   list-repeat (`Instr::Mul` with an already-List left operand,
  ///   `[0] * n`'s own compiled shape) in the function's straight-line
  ///   PROLOGUE; every instruction from `ip` 0 up to the first
  ///   branch, which by construction is the only entry to any of them,
  ///   so nothing here needs a real dominance check to know they all
  ///   run unconditionally, in order, before anything else; and
  ///   nothing after the prologue ever writes that register again.
  ///   Multiple writes WITHIN the prologue are fine (only the last one
  ///   before it ends matters); this only cares whether anything past
  ///   it reassigns the register.
  ///
  /// Both are the same "cheap, deliberately whole-function over-
  /// approximation" trade `scalar_lists`' own eligibility gate makes
  /// (see its docs): a register being reused for something else
  /// entirely, mid-function, is rare enough in practice that a full
  /// analysis would buy little beyond what this costs to compute.
  ///
  /// Deliberately narrower than `type_facts`' own numeric analysis: a
  /// `number`/`int`-only, non-nullable check ALSO feeds `typeflow::
  /// transfer` directly (a real per-`ip` "must" fact, sound across
  /// merges and loop back-edges); this function only needs to cover
  /// the two shapes that dataflow doesn't, `List` and `Instance`, so it
  /// stays intentionally simple rather than duplicating that machinery.
  fn compute_proven_shapes(proto: &ObjFunction) -> FxHashMap<u8, ParamShape> {
    let mut out = FxHashMap::default();
    for instr in &proto.chunk.code {
      let Instr::CheckParamType { reg, check_idx } = *instr else {
        continue;
      };
      let check = &proto.chunk.param_checks[check_idx as usize];
      if check.nullable || check.types.len() != 1 {
        continue;
      }
      let shape = match check.types[0] {
        ParamType::Instance(_) => ParamShape::Instance,
        _ => continue,
      };
      let rewritten = proto
        .chunk
        .code
        .iter()
        .any(|i| typeflow::any_dst(i) == Some(reg));
      if !rewritten {
        out.insert(reg, shape);
      }
    }
    out
  }

  /// Which registers are worth giving a parallel `F64` Variable to:
  /// exactly those some instruction reads or writes through the float
  /// path in `emit_instruction`, plus anything `Instr::Move` ties to
  /// one of them.
  ///
  /// This mirrors `emit_instruction`'s own dispatch rather than
  /// deriving anything, so it stays a cheap syntactic scan. It is
  /// deliberately an over-approximation on both ends: it doesn't check
  /// `type_facts`, so it also marks registers whose float path turns
  /// out to be the guarded one (where the float view is still used,
  /// just behind a branch), and a register it misses simply keeps the
  /// pre-existing `I64`-only behavior. Neither direction can produce a
  /// wrong answer, only a missed or wasted Variable.
  ///
  /// `Instr::Eq`/`Neq` are absent on purpose: those compare raw value
  /// bits with an integer `icmp`, never `fcmp`, so their operands
  /// genuinely have no float view to keep.
  fn compute_f64_tracked(proto: &ObjFunction) -> Vec<bool> {
    let n = proto.num_registers as usize;
    let mut tracked = vec![false; n];
    let mark = |t: &mut Vec<bool>, r: u8| {
      if let Some(slot) = t.get_mut(r as usize) {
        *slot = true;
      }
    };

    for instr in &proto.chunk.code {
      match *instr {
        Instr::Add { dst, a, b }
        | Instr::Sub { dst, a, b }
        | Instr::Mul { dst, a, b }
        | Instr::Div { dst, a, b }
        | Instr::Pow { dst, a, b }
        | Instr::Floor { dst, a, b }
        | Instr::Mod { dst, a, b } => {
          mark(&mut tracked, dst);
          mark(&mut tracked, a);
          mark(&mut tracked, b);
        },
        Instr::AddImm { dst, a, .. }
        | Instr::SubImm { dst, a, .. }
        | Instr::MulImm { dst, a, .. } => {
          mark(&mut tracked, dst);
          mark(&mut tracked, a);
        },
        Instr::Neg { dst, src } => {
          mark(&mut tracked, dst);
          mark(&mut tracked, src);
        },
        // The destination of a comparison is a boolean, never a
        // number; only the operands go through `fcmp`. `Eq`/`Neq`
        // belong here too: they take an `IntCC` but still lower to an
        // `fcmp` once both sides are known numeric.
        Instr::Lt { a, b, .. }
        | Instr::Le { a, b, .. }
        | Instr::Gt { a, b, .. }
        | Instr::Ge { a, b, .. }
        | Instr::Eq { a, b, .. }
        | Instr::Neq { a, b, .. } => {
          mark(&mut tracked, a);
          mark(&mut tracked, b);
        },
        Instr::LtImm { a, .. }
        | Instr::LeImm { a, .. }
        | Instr::GtImm { a, .. }
        | Instr::GeImm { a, .. }
        | Instr::EqImm { a, .. }
        | Instr::NeqImm { a, .. } => mark(&mut tracked, a),
        // The bitwise ops are integer at the machine level but their
        // operands still arrive as floats and convert through one.
        Instr::BitAnd { dst, a, b }
        | Instr::BitOr { dst, a, b }
        | Instr::BitXor { dst, a, b }
        | Instr::BitShl { dst, a, b }
        | Instr::BitShr { dst, a, b }
        | Instr::BitUshr { dst, a, b } => {
          mark(&mut tracked, dst);
          mark(&mut tracked, a);
          mark(&mut tracked, b);
        },
        Instr::BitNot { dst, src } => {
          mark(&mut tracked, dst);
          mark(&mut tracked, src);
        },
        // A numeric branch condition is truth-tested with an `fcmp`
        // against zero rather than a bit compare.
        Instr::JmpIfFalse { cond, .. } | Instr::JmpIfTrue { cond, .. } => mark(&mut tracked, cond),
        // Indexing converts through `f64` on its way to a machine
        // integer, so the index register has a real float view even
        // though the container doesn't.
        Instr::GetIndex { idx, .. } => mark(&mut tracked, idx),
        Instr::SetIndex { idx, .. } => mark(&mut tracked, idx),
        Instr::LoadConst { dst, const_idx } => {
          if proto.chunk.constants[const_idx as usize].is_number() {
            mark(&mut tracked, dst);
          }
        },
        _ => {},
      }
    }

    // A `Move` between a tracked register and an untracked one would
    // otherwise break the float chain right in the middle of an
    // expression, which is exactly where the compiler emits them.
    // Propagate across moves to a fixed point so both ends agree.
    loop {
      let mut changed = false;
      for instr in &proto.chunk.code {
        if let Instr::Move { dst, src } = *instr {
          let (d, s) = (dst as usize, src as usize);
          if d >= n || s >= n {
            continue;
          }
          if tracked[d] != tracked[s] {
            tracked[d] = true;
            tracked[s] = true;
            changed = true;
          }
        }
      }
      if !changed {
        break;
      }
    }

    tracked
  }

  /// Narrows `f64_tracked` to the registers whose float view can stand
  /// alone: see `f64_canonical`. A register qualifies when `facts`
  /// proves it numeric at every position it is live at, which is what
  /// rules out the ones that only look float-ish, most obviously a
  /// comparison result that `compute_f64_tracked` picked up from a
  /// numeric `JmpIfFalse`.
  fn compute_f64_canonical(&self, facts: &typeflow::TypeFacts) -> Vec<bool> {
    let code_len = self.proto.chunk.code.len();
    let mut canonical = self.f64_tracked.clone();
    for (r, slot) in canonical.iter_mut().enumerate() {
      if !*slot {
        continue;
      }
      let r = r as u8;
      *slot = (0..code_len).all(|ip| {
        !self.liveness.is_live(ip, r) || facts.is_numeric(ip, r) || self.int_facts.is_int(ip, r)
      });
    }
    canonical
  }

  fn resolve_call_targets_from_snapshot(
    proto: &ObjFunction,
    preds: &[Vec<usize>],
    globals: &FxHashMap<String, crate::jit::ResolvedGlobal>,
  ) -> (
    FxHashMap<usize, CallTarget>,
    FxHashMap<usize, crate::jit::ConstructInfo>,
    FxHashMap<u8, (usize, u64, FxHashMap<String, u16>)>,
  ) {
    let mut targets = FxHashMap::default();
    let mut construct_info = FxHashMap::default();
    let mut construct_slots: FxHashMap<u8, (usize, u64, FxHashMap<String, u16>)> =
      FxHashMap::default();

    let self_facts = escape::self_reference_facts_with_preds(proto, preds);
    if globals.is_empty() {
      for (ip, instr) in proto.chunk.code.iter().enumerate() {
        if let Instr::Call { func, .. } = instr
          && self_facts[ip].get(*func)
        {
          targets.insert(ip, CallTarget::SelfRecursive);
        }
      }
      return (targets, construct_info, construct_slots);
    }

    let named_facts: Vec<(&String, Vec<escape::MustSet>)> = globals
      .keys()
      .filter(|name| *name != &proto.name)
      .map(|name| {
        let facts = escape::global_ref_facts_with_preds(proto, preds, name);
        (name, facts)
      })
      .collect();

    let proto_ptr = proto as *const ObjFunction;
    for (ip, instr) in proto.chunk.code.iter().enumerate() {
      let Instr::Call { func, .. } = instr else {
        continue;
      };
      if self_facts[ip].get(*func) {
        targets.insert(ip, CallTarget::SelfRecursive);
        continue;
      }
      for (name, facts) in &named_facts {
        if !facts[ip].get(*func) {
          continue;
        }
        let Some(resolved) = globals.get(*name) else {
          continue;
        };
        match resolved {
          crate::jit::ResolvedGlobal::Class {
            guard_bits,
            generation,
            field_count,
            ctor_bits,
            proto_ptr: ctor_proto_ptr,
            safety,
            field_slots,
            simple_ctor_param_slots,
          } => {
            construct_info.insert(
              ip,
              crate::jit::ConstructInfo {
                safety: safety.clone(),
                field_slots: field_slots.clone(),
                field_count: *field_count,
                simple_ctor_param_slots: simple_ctor_param_slots.clone(),
              },
            );
            // `var p = Point(1, 2)` fixes p's class for good: an
            // instance never changes class, so every later read of
            // this register is a read of a Point. Recorded when the
            // construct is the LAST thing to write the register; the
            // idiom writes it twice (the class global, then the
            // result), so "written once" would never hold.
            if let Instr::Call { dst, .. } = instr {
              let written_later = proto
                .chunk
                .code
                .iter()
                .enumerate()
                .any(|(k, i)| k > ip && crate::jit::typeflow::any_dst(i) == Some(*dst));
              if !written_later {
                construct_slots
                  .entry(*dst)
                  .or_insert_with(|| (ip, *guard_bits, field_slots.clone()));
              }
            }
            targets.insert(
              ip,
              CallTarget::ConstructKnown {
                guard_bits: *guard_bits,
                generation: *generation,
                field_count: *field_count,
                ctor_bits: *ctor_bits,
                proto_ptr: *ctor_proto_ptr,
              },
            );
            break;
          },
          crate::jit::ResolvedGlobal::Native {
            guard_fn,
            native_ptr,
          } => {
            targets.insert(
              ip,
              CallTarget::KnownNative {
                guard_fn: *guard_fn,
                native_ptr: *native_ptr,
              },
            );
            break;
          },
          crate::jit::ResolvedGlobal::Closure {
            entry,
            guard_bits,
            proto_ptr: callee_proto_ptr,
          } => {
            if *callee_proto_ptr == proto_ptr as usize {
              targets.insert(ip, CallTarget::SelfRecursive);
            } else {
              targets.insert(
                ip,
                CallTarget::Known {
                  entry: *entry,
                  guard_bits: *guard_bits,
                  proto_ptr: *callee_proto_ptr,
                },
              );
            }
            break;
          },
        }
      }
    }
    (targets, construct_info, construct_slots)
  }

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
    shutdown: Option<&'a AtomicBool>,
  ) -> Self {
    let blocks = (0..code_len).map(|_| fb.create_block()).collect();
    let preds = typeflow::build_predecessors(proto);
    // The general body gets NO self-field or numeric field facts at all,
    // neither `facts.self_numeric_fields` nor `facts.numeric_fields`:
    // those are speculative samples (see `VM::resolve_self_numeric_fields`
    // and `VM::resolve_all_numeric_fields`), and this body has no guard
    // mechanism to re-validate them. The real sets still reach the
    // specialized body below, through `self.self_numeric_fields` and
    // `self.numeric_fields`, where `emit_speculative_guard` re-checks
    // every bet against the actual value and deopts on a mismatch.
    let no_self_numeric_fields = FxHashSet::default();
    let no_numeric_fields = FxHashSet::default();
    let speculative_ints = facts.speculative_ints.filter(|&m| m != 0);
    let bytes_facts = typeflow::analyze_bytes(proto, &preds);
    let dict_facts = typeflow::analyze_dict(proto, &preds);
    let byte_value_facts = typeflow::analyze_byte_value(proto, &preds, &bytes_facts);
    let int_facts = typeflow::analyze_int(proto, &preds, None, &bytes_facts);
    let spec_int_facts =
      speculative_ints.map(|si| typeflow::analyze_int(proto, &preds, Some(si), &bytes_facts));
    let type_facts = typeflow::analyze(
      proto,
      &preds,
      None,
      None,
      &no_self_numeric_fields,
      &no_numeric_fields,
      Some(&int_facts),
    );
    let list_facts = typeflow::analyze_list(proto, &preds, None, &facts.global_lists);
    let string_facts = typeflow::analyze_string(proto, &preds);
    let bool_facts = typeflow::analyze_bool(proto, &preds);
    let const_facts = typeflow::analyze_const(proto, &preds);
    let liveness = typeflow::liveness(proto, &preds);
    let (call_targets, construct_info, construct_slots) =
      Self::resolve_call_targets_from_snapshot(proto, &preds, &facts.globals_snapshot);
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
      type_facts,
      int_facts,
      list_facts,
      string_facts,
      bytes_facts,
      dict_facts,
      byte_value_facts,
      bool_facts,
      const_facts,
      preds,
      self_ref_facts: std::cell::OnceCell::new(),
      self_summary: std::cell::OnceCell::new(),
      speculative_params,
      speculative_lists: facts.speculative_lists.filter(|&m| m != 0),
      speculative_ints,
      spec_int_facts,
      speculative_regs,
      // Populated in `run`, once `base_bytes` is available; empty
      // placeholders here are never actually read before that, since
      // `run` always executes before any `emit_instruction` call.
      reg_vars: Vec::new(),
      reg_vars_f64: Vec::new(),
      reg_vars_int: Vec::new(),
      int_tracked: Vec::new(),
      f64_tracked: Vec::new(),
      f64_canonical: Vec::new(),
      entry_reg_values: Vec::new(),
      global_vars: FxHashMap::default(),
      current_ip: 0,
      liveness,
      self_field_slots: facts.self_field_slots,
      self_numeric_fields: facts.self_numeric_fields,
      numeric_fields: facts.numeric_fields,
      global_lists: facts.global_lists,
      param_field_slots: facts.param_field_slots,
      construct_field_slots: construct_slots,
      own_func_id,
      self_class_bits: facts.self_class_bits,
      call_targets,
      construct_info,
      scalar_instances: FxHashMap::default(),
      planned_scalar_instances: FxHashMap::default(),
      scalar_lists: FxHashMap::default(),
      frame_can_open_upvalues: proto
        .chunk
        .code
        .iter()
        .any(|i| matches!(i, Instr::Closure { .. })),
      proven_param_shapes: Self::compute_proven_shapes(proto),
      guarded_instance_vars: FxHashMap::default(),
      active_guarded: FxHashSet::default(),
      known_classes: facts.known_classes,
      deopt_receivers: facts
        .deopt_sites
        .iter()
        .filter_map(|&ip| match proto.chunk.code.get(ip) {
          Some(Instr::GetField { obj, .. }) | Some(Instr::SetField { obj, .. }) => Some(*obj),
          _ => None,
        })
        .collect(),
      deopt_sites: facts.deopt_sites,
      field_speculation_off: facts.field_speculation_off,
      self_method_protos: facts.self_method_protos,
      is_specialized_pass: false,
      active_guarded_classes: FxHashMap::default(),
      shutdown,
    }
  }

  #[inline]
  fn proven_numeric(&self, ip: usize, r: u8) -> bool {
    self.type_facts.is_numeric(ip, r) || self.int_facts.is_int(ip, r)
  }

  #[inline]
  fn proven_int(&self, ip: usize, r: u8) -> bool {
    self.int_facts.is_int(ip, r) && self.int_tracked.get(r as usize).copied().unwrap_or(false)
  }

  #[inline]
  fn proven_int_list(&self, ip: usize, r: u8) -> bool {
    self.int_facts.is_int_list(ip, r)
  }

  #[inline]
  fn proven_list(&self, ip: usize, r: u8) -> bool {
    self.list_facts.is_list(ip, r)
  }

  #[inline]
  fn proven_string(&self, ip: usize, r: u8) -> bool {
    self.string_facts.is_string(ip, r)
  }

  #[inline]
  fn proven_bytes(&self, ip: usize, r: u8) -> bool {
    self.bytes_facts.is_bytes(ip, r)
  }

  #[inline]
  fn proven_dict(&self, ip: usize, r: u8) -> bool {
    self.dict_facts.is_dict(ip, r)
  }

  /// Is `r` provably a value a byte stream will accept, so a store
  /// into one needs no check? Either the analysis proved it, or it is
  /// a constant already in range.
  fn proven_byte_value(&self, ip: usize, r: u8) -> bool {
    if self.byte_value_facts.is_byte_value(ip, r) {
      return true;
    }
    self
      .proven_const(ip, r)
      .is_some_and(|c| c.fract() == 0.0 && (0.0..=255.0).contains(&c))
  }

  /// Emits the check `builtins`/`VM::index_set` make before storing
  /// into a byte stream: a number, whole, and within 0..=255. Branches
  /// to `ok_block` when it holds and `slow_block` when it does not, so
  /// the real error stays the interpreter's to raise.
  ///
  /// Skipped entirely when `proven_byte_value` already settles it.
  fn emit_byte_value_guard(
    &mut self,
    ip: usize,
    src: u8,
    src_val: IrValue,
    ok_block: cranelift_codegen::ir::Block,
    slow_block: cranelift_codegen::ir::Block,
  ) {
    use cranelift_codegen::ir::condcodes::FloatCC;
    let f = self.f64_view(src, src_val);
    let as_int = self.fb.ins().fcvt_to_sint_sat(types::I64, f);
    let roundtrip = self.fb.ins().fcvt_from_sint(types::F64, as_int);
    // Catches a fraction and a NaN in one test: NaN never compares
    // equal, so it takes the slow path and raises there.
    let whole = self.fb.ins().fcmp(FloatCC::Equal, f, roundtrip);
    let zero = self.i64c(0);
    let max = self.i64c(255);
    let ge0 = self
      .fb
      .ins()
      .icmp(IntCC::SignedGreaterThanOrEqual, as_int, zero);
    let le255 = self
      .fb
      .ins()
      .icmp(IntCC::SignedLessThanOrEqual, as_int, max);
    let in_range = self.fb.ins().band(ge0, le255);
    let whole_and_range = self.fb.ins().band(whole, in_range);
    let ok = if self.proven_numeric(ip, src) {
      whole_and_range
    } else {
      let is_num = self.is_number(src_val);
      self.fb.ins().band(is_num, whole_and_range)
    };
    self.fb.ins().brif(ok, ok_block, &[], slow_block, &[]);
  }

  #[inline]
  fn proven_const(&self, ip: usize, r: u8) -> Option<f64> {
    self.const_facts.const_value(ip, r)
  }

  #[inline]
  fn both_proven_numeric(&self, ip: usize, a: u8, b: u8) -> bool {
    self.proven_numeric(ip, a) && self.proven_numeric(ip, b)
  }

  #[inline]
  fn both_proven_string(&self, ip: usize, a: u8, b: u8) -> bool {
    self.proven_string(ip, a) && self.proven_string(ip, b)
  }

  #[inline]
  fn both_proven_int(&self, ip: usize, a: u8, b: u8) -> bool {
    self.proven_int(ip, a) && self.proven_int(ip, b)
  }

  fn maybe_clear_guarded_instances(&mut self, ip: usize) {
    if ip == 0 || self.preds[ip].len() != 1 || self.preds[ip][0] != ip - 1 {
      self.active_guarded.clear();
      self.active_guarded_classes.clear();
      return;
    }
    let prev = self.proto.chunk.code[ip - 1];
    if matches!(
      prev,
      Instr::Call { .. } | Instr::InvokeSuper { .. } | Instr::Return { .. }
    ) {
      self.active_guarded.clear();
      self.active_guarded_classes.clear();
      return;
    }
    if let Instr::Invoke { method_const, .. } = prev {
      let name = self.method_name(method_const);
      if NumberIntrinsic::of(name).is_none() {
        self.active_guarded.clear();
        self.active_guarded_classes.clear();
        return;
      }
    }
    if let Some(dst) = typeflow::any_dst(&prev) {
      self.active_guarded.remove(&dst);
      self.active_guarded_classes.remove(&dst);
    }
  }

  /// Which registers get an `i64` Variable maintained alongside their
  /// float one.
  ///
  /// Being an integer somewhere is not reason enough. The integer view
  /// costs a second value to compute and keep live at every write, so it
  /// is only worth maintaining where an instruction genuinely READS an
  /// `i64` operand: the remainder, the bitwise operators, and an index.
  /// An accumulator that is only ever added to and returned wants none
  /// of it, and paying for one there means running the loop's arithmetic
  /// twice, once per domain.
  ///
  /// Demand flows backwards out of those readers through the operations
  /// that merely forward the view, to a fixed point. Missing a reader is
  /// safe: `proven_int` then reads false and the instruction takes its
  /// float path, which is slower but never wrong.
  fn compute_int_tracked(&self) -> Vec<bool> {
    let n = self.proto.num_registers as usize;

    // Nothing outside the analysis's own set can carry an integer view,
    // whatever the demand says.
    let mut ever_int = vec![false; n];
    for set in &self.int_facts.entry {
      for r in 0..n {
        if set.get(r as u8) {
          ever_int[r] = true;
        }
      }
    }
    if let Some(spec) = &self.spec_int_facts {
      for set in &spec.entry {
        for r in 0..n {
          if set.get(r as u8) {
            ever_int[r] = true;
          }
        }
      }
    }

    let mut needed = vec![false; n];
    let mark = |needed: &mut Vec<bool>, r: u8| -> bool {
      let i = r as usize;
      if i < n && !needed[i] {
        needed[i] = true;
        return true;
      }
      false
    };

    for instr in &self.proto.chunk.code {
      match *instr {
        Instr::Mod { a, b, .. }
        | Instr::BitAnd { a, b, .. }
        | Instr::BitOr { a, b, .. }
        | Instr::BitXor { a, b, .. }
        | Instr::BitShl { a, b, .. }
        | Instr::BitShr { a, b, .. }
        | Instr::BitUshr { a, b, .. } => {
          mark(&mut needed, a);
          mark(&mut needed, b);
        },
        Instr::BitNot { src, .. } => {
          mark(&mut needed, src);
        },
        Instr::GetIndex { idx, .. } | Instr::SetIndex { idx, .. } => {
          mark(&mut needed, idx);
        },
        // Comparisons take the integer route when both sides have a
        // view, which is what a loop condition rides on.
        Instr::Eq { a, b, .. }
        | Instr::Neq { a, b, .. }
        | Instr::Lt { a, b, .. }
        | Instr::Le { a, b, .. }
        | Instr::Gt { a, b, .. }
        | Instr::Ge { a, b, .. } => {
          mark(&mut needed, a);
          mark(&mut needed, b);
        },
        Instr::EqImm { a, .. }
        | Instr::NeqImm { a, .. }
        | Instr::LtImm { a, .. }
        | Instr::LeImm { a, .. }
        | Instr::GtImm { a, .. }
        | Instr::GeImm { a, .. } => {
          mark(&mut needed, a);
        },
        Instr::GetSlice { lo, hi, .. } => {
          mark(&mut needed, lo);
          mark(&mut needed, hi);
        },
        // An inlined callee reads the CALLER's integer view for its
        // arguments, and the instruction that consumes it lives in the
        // callee where this scan cannot see it. Arguments therefore
        // count as read. `Invoke` additionally hands its receiver and
        // argument to the integer form of `min`/`max`.
        Instr::Call { func, num_args, .. } => {
          for i in 0..num_args {
            mark(&mut needed, func + 1 + i);
          }
        },
        Instr::Invoke { obj, num_args, .. } => {
          mark(&mut needed, obj);
          for i in 0..num_args {
            mark(&mut needed, obj + 2 + i);
          }
        },
        _ => {},
      }
    }

    loop {
      let mut changed = false;
      for instr in &self.proto.chunk.code {
        match *instr {
          Instr::Add { dst, a, b } | Instr::Sub { dst, a, b } | Instr::Mul { dst, a, b } => {
            if needed[dst as usize] {
              changed |= mark(&mut needed, a);
              changed |= mark(&mut needed, b);
            }
          },
          Instr::AddImm { dst, a, .. }
          | Instr::SubImm { dst, a, .. }
          | Instr::MulImm { dst, a, .. } => {
            if needed[dst as usize] {
              changed |= mark(&mut needed, a);
            }
          },
          Instr::Neg { dst, src } | Instr::Move { dst, src } | Instr::BitNot { dst, src } => {
            if needed[dst as usize] {
              changed |= mark(&mut needed, src);
            }
          },
          _ => {},
        }
      }
      if !changed {
        break;
      }
    }

    for r in 0..n {
      needed[r] = needed[r] && ever_int[r];
    }
    needed
  }

  /// `Instr::Div { dst, a, b }`'s strength-reduction check: is `b`'s
  /// value PROVABLY a compile-time constant that's an exact power of
  /// two, so `x / b` can compile to `x * (1.0/b)` instead of a real
  /// `fdiv`? Sound because multiplying or dividing an IEEE-754 double
  /// by a power of two only ever shifts its exponent; the mantissa
  /// is untouched either way; so this is an EXACT rewrite, bit-for-
  /// bit identical to the division for every possible `x`, unlike the
  /// general "replace division by a multiplication by its reciprocal"
  /// trick (an approximation for an arbitrary constant, which this
  /// codebase does not do). Measured: ~25-30% faster on a division-
  /// heavy microbenchmark (`fdiv` has multi-cycle throughput/latency on
  /// every mainstream x86/ARM core; `fmul` is single-cycle-throughput).
  ///
  /// Backed by `typeflow::ConstFacts`, a real "must be this exact
  /// value" dataflow proof; so this catches a constant hoisted into
  /// a local variable and reused across many divisions (e.g. every
  /// iteration of a loop dividing by the same hoisted constant), or one
  /// reached through a branch merge where every arm happens to load the
  /// identical literal, not just an immediately-preceding `LoadConst`.
  /// An earlier version of this check was exactly that narrower local
  /// check (no dataflow pass, just "is `code[ip-1]` this exact
  /// `LoadConst`"); `ConstFacts` is a strict superset of what it
  /// could prove, so there was nothing left for it to do once this
  /// existed.
  fn div_by_pow2_reciprocal(&self, ip: usize, b: u8) -> Option<f64> {
    let value = self.proven_const(ip, b)?;
    if value == 0.0 || !value.is_finite() {
      return None;
    }
    // An exact power of two has a fully zero mantissa and a "normal"
    // (neither all-zero, which means a subnormal/zero, nor all-one,
    // which means inf/NaN) exponent; a real bit-level test, not a
    // fuzzy `log2` comparison that could be fooled by rounding.
    let bits = value.to_bits();
    let mantissa = bits & 0x000F_FFFF_FFFF_FFFF;
    let exponent = (bits >> 52) & 0x7FF;
    if mantissa != 0 || exponent == 0 || exponent == 0x7FF {
      return None;
    }
    Some(1.0 / value)
  }

  fn run(&mut self) -> Result<FxHashMap<usize, i32>, String> {
    // Discover every loop header (the target of a BACKWARD Instr::Jmp)
    // and assign it a small dense integer id; what `EntryFn`'s
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
    // counts; from `self.preds`, already computed once in `new`,
    // rather than a fresh `predecessor_counts` call recomputing the
    // identical predecessor graph again.
    let entry_block = self.fb.create_block();
    self.fb.append_block_params_for_function_params(entry_block);
    self.fb.switch_to_block(entry_block);
    let params = self.fb.block_params(entry_block).to_vec();
    self.vm_param = params[0];
    self.base_param = params[1];
    self.closure_param = params[2];
    let osr_param = params[3];
    let fast_args = [params[4], params[5], params[6], params[7]];

    self.regs_var = self.fb.declare_var(types::I64);
    let initial_regs = self.load_regs_ptr_cache();
    self.fb.def_var(self.regs_var, initial_regs);

    let eight = self.fb.ins().iconst(types::I64, 8);
    self.base_bytes = self.fb.ins().imul(self.base_param, eight);

    let neg1 = self.fb.ins().iconst(types::I32, -1);
    let is_normal = self.fb.ins().icmp(IntCC::Equal, osr_param, neg1);

    let num_regs = self.proto.num_registers as usize;
    self.reg_vars = (0..num_regs)
      .map(|_| self.fb.declare_var(types::I64))
      .collect();
    self.f64_tracked = Self::compute_f64_tracked(self.proto);
    self.int_tracked = self.compute_int_tracked();
    // Declared for every register, but only ever defined or used for
    // the tracked ones; an untracked entry is inert.
    self.reg_vars_f64 = (0..num_regs)
      .map(|_| self.fb.declare_var(types::F64))
      .collect();
    self.reg_vars_int = (0..num_regs)
      .map(|_| self.fb.declare_var(types::I64))
      .collect();
    let zero = self.fb.ins().iconst(types::I64, 0);
    for r in 0..num_regs {
      let mem_v = self.load_reg_mem(r as u8);
      let v = if r < 4 && r < self.proto.arity as usize {
        self.fb.ins().select(is_normal, fast_args[r], mem_v)
      } else {
        mem_v
      };
      // Seeds BOTH views in the entry block, which dominates every
      // other block in the function (the general body, the specialized
      // body, and every OSR target alike). That's what lets any later
      // block read either view without having to prove some
      // intervening definition reached it, and it's why the two bodies
      // can disagree about which registers are canonical. Deliberately
      // not `def_reg_both`: this one place wants both views
      // unconditionally, canonical or not.
      self.fb.def_var(self.reg_vars[r], v);
      if self.f64_tracked[r] {
        let f = self
          .fb
          .ins()
          .bitcast(types::F64, cranelift_codegen::ir::MemFlagsData::new(), v);
        self.fb.def_var(self.reg_vars_f64[r], f);
      }
      if self.int_tracked[r] {
        let f = self.to_f64(v);
        let iv = self.fb.ins().fcvt_to_sint_sat(types::I64, f);
        self.fb.def_var(self.reg_vars_int[r], iv);
      }
      self.entry_reg_values.push(v);
    }
    for r in 0..num_regs as u8 {
      let ptr_v = self.fb.declare_var(types::I64);
      let f_ptr_v = self.fb.declare_var(types::I64);
      let cls_v = self.fb.declare_var(types::I64);
      self.fb.def_var(ptr_v, zero);
      self.fb.def_var(f_ptr_v, zero);
      self.fb.def_var(cls_v, zero);
      self
        .guarded_instance_vars
        .insert(r, (ptr_v, f_ptr_v, cls_v));
    }
    self.f64_canonical = self.compute_f64_canonical(&self.type_facts);

    let flags_init = cranelift_codegen::ir::MemFlagsData::trusted();
    let slots_ptr_init = self.fb.ins().load(
      types::I64,
      flags_init,
      self.vm_param,
      GLOBAL_SLOTS_PTR_CACHE_OFFSET,
    );
    let mut unique_slots: FxHashSet<i64> = FxHashSet::default();
    for slot_cell in &self.proto.jit.global_slot_cache {
      let slot = slot_cell.get();
      if slot >= 0 {
        unique_slots.insert(slot);
      }
    }
    for &slot in &unique_slots {
      let var = self.fb.declare_var(types::I64);
      let byte_off = (slot * 8) as i32;
      let initial_val = self
        .fb
        .ins()
        .load(types::I64, flags_init, slots_ptr_init, byte_off);
      self.fb.def_var(var, initial_val);
      self.global_vars.insert(slot, var);
    }

    // If speculating on EITHER function parameters or a mid-function
    // value (`speculative_regs`: see `jit::typeflow::SpeculativeRegs`),
    // allocate a SECOND set of blocks now (before the entry dispatch,
    // which needs `specialized_blocks[0]` as a jump target); populated
    // in a second pass below, after the general body. Creating a block
    // doesn't require switching into it, so this doesn't disturb
    // `entry_block`'s own not-yet-terminated state. Its type-facts are
    // computed HERE too (not lazily during the second pass, as before)
    //; `emit_entry_dispatch` needs them NOW to build a SOUND per-OSR-
    // target guard (see its own docs).
    let specialized: Option<(Vec<Block>, typeflow::TypeFacts)> =
      if self.speculative_params.is_some()
        || self.speculative_regs.is_some()
        || self.speculative_lists.is_some()
        || self.speculative_ints.is_some()
        || !self.self_numeric_fields.is_empty()
        || !self.numeric_fields.is_empty()
      {
        let blocks = (0..self.blocks.len())
          .map(|_| self.fb.create_block())
          .collect();
        let int_facts_ref = self.spec_int_facts.as_ref().unwrap_or(&self.int_facts);
        let facts = typeflow::analyze(
          self.proto,
          &self.preds,
          self.speculative_params,
          self.speculative_regs,
          &self.self_numeric_fields,
          &self.numeric_fields,
          Some(int_facts_ref),
        );
        Some((blocks, facts))
      } else {
        None
      };

    for (ip, instr) in self.proto.chunk.code.iter().enumerate() {
      if let Instr::MakeList { dst, count, .. } = *instr {
        if self.scalar_replace_eligible(ip, dst, count) && !self.scalar_lists.contains_key(&dst) {
          let s = self.fb.create_sized_stack_slot(StackSlotData::new(
            StackSlotKind::ExplicitSlot,
            count as u32 * 8,
            3,
          ));
          self.scalar_lists.insert(dst, (s, count));
        }
      }
    }

    // Scalar-replaced instances are registered here, alongside the
    // lists, and NOT where the construct is emitted. `emit_entry_
    // dispatch` below writes the OSR entry paths, and those have to
    // refill every scalar slot from the real object; a slot that only
    // came into existence later in the body would be invisible to
    // them, and entering past the construct would then read an
    // unwritten slot.
    for (ip, instr) in self.proto.chunk.code.iter().enumerate() {
      if let Instr::Call { dst, .. } = *instr
        && self.scalar_construct_eligible(ip, dst)
        && !self.planned_scalar_instances.contains_key(&dst)
        && let Some(info) = self.construct_info.get(&ip)
      {
        let s = self.fb.create_sized_stack_slot(StackSlotData::new(
          StackSlotKind::ExplicitSlot,
          info.field_count as u32 * 8,
          3,
        ));
        self.planned_scalar_instances.insert(dst, (s, ip));
      }
    }

    self.emit_entry_dispatch(
      osr_param,
      specialized.as_ref().map(|(b, f)| (b.as_slice(), f)),
    );

    // Pass 1: the general body.
    self.is_specialized_pass = false;
    self.active_guarded.clear();
    self.active_guarded_classes.clear();
    for ip in 0..self.blocks.len() {
      if let Some(shutdown) = self.shutdown {
        if shutdown.load(std::sync::atomic::Ordering::Relaxed) {
          return Err("compilation aborted: VM shutdown".to_string());
        }
      }
      self.fb.switch_to_block(self.blocks[ip]);
      self.maybe_clear_guarded_instances(ip);
      let instr = self.proto.chunk.code[ip];
      let terminated = self.emit_instruction(ip, instr);
      if !terminated {
        let next_ip = ip + 1;
        let next = self.blocks.get(next_ip).copied().unwrap_or(self.blocks[ip]);
        self.fb.ins().jump(next, &[]);
      }
    }

    // Pass 2: the specialized body, if any.
    if let Some((spec_blocks, spec_facts)) = specialized {
      self.type_facts = spec_facts;
      if let Some(spec_int) = self.spec_int_facts.take() {
        self.int_facts = spec_int;
      }
      self.list_facts = typeflow::analyze_list(
        self.proto,
        &self.preds,
        self.speculative_lists,
        &self.global_lists,
      );
      // The specialized facts prove strictly more than the general
      // ones, so this body typically gets more canonical registers;
      // recompute rather than carrying pass 1's answer over. See
      // `f64_canonical` on why the two bodies are free to disagree.
      self.f64_canonical = self.compute_f64_canonical(&self.type_facts);
      let general_blocks = std::mem::replace(&mut self.blocks, spec_blocks);
      let code_len = self.blocks.len();
      self.is_specialized_pass = true;
      self.active_guarded.clear();
      self.active_guarded_classes.clear();
      for ip in 0..code_len {
        if let Some(shutdown) = self.shutdown {
          if shutdown.load(std::sync::atomic::Ordering::Relaxed) {
            return Err("compilation aborted: VM shutdown".to_string());
          }
        }
        self.fb.switch_to_block(self.blocks[ip]);
        self.maybe_clear_guarded_instances(ip);
        let instr = self.proto.chunk.code[ip];
        let terminated = self.emit_instruction(ip, instr);
        if terminated {
          continue;
        }
        let spec_next = self.blocks.get(ip + 1).copied().unwrap_or(self.blocks[ip]);
        if !self.emit_speculative_guard(ip, instr, spec_next) {
          self.fb.ins().jump(spec_next, &[]);
        }
      }
      // `osr_ids` (returned to the caller) indexes into the GENERAL
      // block set by construction (computed before either pass ran,
      // from `self.blocks` as it was BEFORE this swap); restore it
      // so nothing downstream of `run` needs to know a swap ever
      // happened.
      self.blocks = general_blocks;
    }

    // Every block's predecessors are now fully known (every `jump`/
    // `brif`/`call_indirect`-adjacent branch this function will ever
    // emit, including every loop's own back-edge, has already been
    // added above); only NOW is it sound to seal them all at once.
    // This matters specifically because of `reg_vars`: unlike
    // `regs_var` (always freshly `def_var`'d within the SAME block as
    // any `use_var` of it, so it never actually depended on cross-
    // block SSA resolution), a bytecode register's `Variable` is
    // genuinely defined in one block and read in another; including
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
  ///; the number of loop headers in one function is always small,
  /// and this avoids depending on `JumpTableData`'s exact API for
  /// what's a cold, one-time-per-call dispatch anyway.
  ///
  /// If `specialized` is `Some((blocks, facts))`, EVERY entry point
  /// (ordinary AND each OSR target) gets its own guard picking between
  /// the specialized and general body, built from `facts.numeric_mask_at`
  /// AT THAT SPECIFIC bytecode position; not from a single fixed
  /// mask re-checked everywhere. This is the sound way to validate an
  /// OSR jump straight into the MIDDLE of the specialized body: at
  /// `ip == 0` `numeric_mask_at` is exactly the original speculated
  /// parameter mask (so ordinary entry is unaffected by this
  /// generalization), but at any OTHER `ip` it's whatever the SAME
  /// dataflow proof actually established is live and provably numeric
  /// AT THAT POINT; precisely the claim the code there is about to
  /// rely on, re-validated against real, current register values. See
  /// `typeflow::TypeFacts::numeric_mask_at`'s own docs for why
  /// re-checking the ORIGINAL entry mask at a later `ip` instead would
  /// NOT be sound (a speculated register can be reassigned between
  /// entry and that point in a way a same-register recheck can't see).
  /// An OSR target where NOTHING is provably numeric (the mask is
  /// empty; the loop never touches the speculated value at all)
  /// skips the guard and routes straight to the general body: the
  /// specialized block there would be behaviorally identical anyway.
  fn emit_osr_scalar_list_init(&mut self, ip: usize) {
    if ip == 0 {
      return;
    }
    let lists: Vec<(u8, StackSlot, u8)> = self
      .scalar_lists
      .iter()
      .map(|(&dst, &(slot, count))| (dst, slot, count))
      .collect();
    for (dst, slot, count) in lists {
      let slot_addr = self.fb.ins().stack_addr(types::I64, slot, 0);
      let dst_c = self.u64c(dst as u64);
      let count_c = self.u64c(count as u64);
      self.call_helper(
        "zuri_jit_init_osr_scalar_list",
        &[self.vm_param, self.base_param, dst_c, slot_addr, count_c],
      );
    }

    // Same treatment for scalar-replaced instances. Entering past the
    // construct leaves its stack slot unwritten, so the fields come
    // from the real instance the interpreter already built.
    let instances: Vec<(u8, StackSlot, u16)> = self
      .planned_scalar_instances
      .iter()
      .filter_map(|(&dst, &(slot, site))| {
        let count = self.construct_info.get(&site)?.field_count;
        Some((dst, slot, count))
      })
      .collect();
    for (dst, slot, count) in instances {
      let slot_addr = self.fb.ins().stack_addr(types::I64, slot, 0);
      let dst_c = self.u64c(dst as u64);
      let count_c = self.u64c(count as u64);
      self.call_helper(
        "zuri_jit_init_osr_scalar_instance",
        &[self.vm_param, self.base_param, dst_c, slot_addr, count_c],
      );
    }
  }

  fn emit_entry_dispatch(
    &mut self,
    osr_param: IrValue,
    specialized: Option<(&[Block], &typeflow::TypeFacts)>,
  ) {
    let neg1 = self.fb.ins().iconst(types::I32, -1);
    let is_normal = self.fb.ins().icmp(IntCC::Equal, osr_param, neg1);

    let normal_route = Some(self.fb.create_block());
    let normal_target = normal_route.unwrap();

    let mut next_check = self.fb.create_block();
    self
      .fb
      .ins()
      .brif(is_normal, normal_target, &[], next_check, &[]);

    let mut targets: Vec<(i32, usize)> = self.osr_ids.iter().map(|(&ip, &id)| (id, ip)).collect();
    targets.sort_by_key(|&(id, _)| id);

    // (route_block, target_ip) pairs to populate AFTER the compare
    // chain is fully laid out; keeps every `switch_to_block` call
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
      let route = self.fb.create_block();
      self.fb.ins().brif(is_this, route, &[], after, &[]);
      routes.push((route, ip));
      next_check = after;
    }

    // Defensive fallback for an `osr_id` that matches none of the
    // known loop headers; unreachable in practice (`VM::maybe_osr`
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
        self.emit_osr_scalar_list_init(ip);
        let num_mask = spec_facts.numeric_mask_at(ip);
        let list_mask = if ip == 0 {
          self.speculative_lists.unwrap_or(0)
        } else {
          0
        };
        let int_mask = if ip == 0 {
          self.speculative_ints.unwrap_or(0)
        } else {
          0
        };
        if num_mask == 0 && list_mask == 0 && int_mask == 0 {
          if self.speculative_regs.is_some()
            || self.speculative_ints.is_some()
            || !self.self_numeric_fields.is_empty()
            || !self.numeric_fields.is_empty()
            || !self.known_classes.is_empty()
          {
            self.fb.ins().jump(spec_blocks[ip], &[]);
          } else {
            self.fb.ins().jump(self.blocks[ip], &[]);
          }
          continue;
        }
        let mut guard: Option<IrValue> = None;
        for bit in 0..64u8 {
          if num_mask & (1u64 << bit) != 0 {
            let v = self.entry_reg_values[bit as usize];
            let is_num = self.is_number(v);
            guard = Some(match guard {
              None => is_num,
              Some(g) => self.fb.ins().band(g, is_num),
            });
          }
        }
        if let Some(g) = guard {
          let num_passed_block = self.fb.create_block();
          self
            .fb
            .ins()
            .brif(g, num_passed_block, &[], self.blocks[ip], &[]);
          self.fb.switch_to_block(num_passed_block);
        }
        if list_mask != 0 {
          for bit in 0..64u8 {
            if list_mask & (1u64 << bit) != 0 {
              let v = self.entry_reg_values[bit as usize];
              let is_obj = self.is_obj(v);
              let obj_passed_block = self.fb.create_block();
              self
                .fb
                .ins()
                .brif(is_obj, obj_passed_block, &[], self.blocks[ip], &[]);
              self.fb.switch_to_block(obj_passed_block);

              let ptr = self.obj_ptr(v);
              let tag = self.obj_tag(ptr);
              let tag_list = self.i64c(object::OBJ_TAG_LIST as i64);
              let is_list = self.fb.ins().icmp(IntCC::Equal, tag, tag_list);
              let list_passed_block = self.fb.create_block();
              self
                .fb
                .ins()
                .brif(is_list, list_passed_block, &[], self.blocks[ip], &[]);
              self.fb.switch_to_block(list_passed_block);
            }
          }
        }
        if int_mask != 0 {
          for bit in 0..64u8 {
            if int_mask & (1u64 << bit) != 0 {
              let v = self.entry_reg_values[bit as usize];
              let is_num = self.is_number(v);
              let f = self.to_f64(v);
              let iv = self.fb.ins().fcvt_to_sint_sat(types::I64, f);
              let roundtrip = self.fb.ins().fcvt_from_sint(types::F64, iv);
              let is_whole = self.fb.ins().fcmp(
                cranelift_codegen::ir::condcodes::FloatCC::Equal,
                f,
                roundtrip,
              );
              let is_int = self.fb.ins().band(is_num, is_whole);
              let int_passed_block = self.fb.create_block();
              self
                .fb
                .ins()
                .brif(is_int, int_passed_block, &[], self.blocks[ip], &[]);
              self.fb.switch_to_block(int_passed_block);
            }
          }
        }
        self.fb.ins().jump(spec_blocks[ip], &[]);
      }
    } else {
      for (route_block, ip) in routes {
        self.fb.switch_to_block(route_block);
        self.emit_osr_scalar_list_init(ip);
        self.fb.ins().jump(self.blocks[ip], &[]);
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
  /// `speculative_regs` folded in: see `jit::typeflow::SpeculativeRegs`),
  /// re-validates the ACTUAL value `instr` just computed and either
  /// continues into `spec_next` (the specialized body's own block for
  /// `ip + 1`) on a match, or deoptimizes to the interpreter at `ip + 1`
  /// (see `emit_deopt`) on a mismatch. Returns `true` iff it terminated
  /// the current block this way; the caller emits its own
  /// unconditional jump to `spec_next` when this returns `false`
  /// (nothing to guard here).
  ///
  /// Used to cross-jump into the general body's own block for this same
  /// `ip + 1` instead, back when this guard predates real
  /// deoptimization; that was ALSO sound (neither body ever carries
  /// state across an instruction boundary as a Cranelift SSA value, so
  /// resuming general-body translation needed no reconciliation
  /// either), but strictly less general: it only ever worked because a
  /// compiled fallback happened to already exist. Deopting to the
  /// interpreter needs no fallback body to exist at all.
  fn emit_speculative_guard(&mut self, ip: usize, instr: Instr, spec_next: Block) -> bool {
    let Some(dst) = typeflow::conservative_dst(&instr) else {
      return false;
    };
    // A scalar-replaced list's `dst` is deliberately never materialized
    // as a real `Value` at all; `emit_scalar_make_list` never calls
    // `store_reg` for it, since the whole point is that no tagged
    // `Value` for this register exists anywhere (see its own docs).
    // `load_reg(dst)` below would therefore read whatever stale or
    // uninitialized bits happen to sit in that Cranelift `Variable`,
    // not a genuine value to validate; and if `speculative_regs`
    // happened to (wrongly, but plausibly, since it's a one-shot sample
    // of a DIFFERENT execution of this same register number elsewhere
    // in the function) bet this register numeric, `type_facts` would
    // otherwise optimistically believe it from this exact definition
    // site onward, triggering a guard against garbage. There is
    // nothing to speculate about here regardless: a list literal's
    // `dst` is never a number, scalar-replaced or not, so skipping
    // this unconditionally is always the correct answer, not a
    // special-cased escape hatch.
    if let Instr::MakeList { dst: list_dst, .. } = instr
      && self.scalar_lists.contains_key(&list_dst)
    {
      return false;
    }
    if let Instr::GetIndex { obj, .. } = instr
      && self.int_facts.is_int_list(ip, obj)
    {
      return false;
    }
    if ip + 1 >= self.proto.chunk.code.len() || !self.type_facts.is_numeric(ip + 1, dst) {
      return false;
    }
    let v = self.load_reg(dst);
    let is_num = self.is_number(v);
    let deopt_block = self.fb.create_block();
    self.fb.ins().brif(is_num, spec_next, &[], deopt_block, &[]);
    self.fb.switch_to_block(deopt_block);
    self.emit_deopt(ip + 1);
    true
  }

  fn emit_deopt(&mut self, ip: usize) {
    self.flush_live(ip);
    let vm = self.vm_param;
    let ip_c = self.u64c(ip as u64);
    self.call_helper_raw("zuri_jit_deopt", &[vm, ip_c]);
    let junk = self.i64c(0);
    self.fb.ins().return_(&[junk]);
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

  fn load_reg_mem(&mut self, r: u8) -> IrValue {
    let addr = self.reg_addr(r);
    self.fb.ins().load(
      types::I64,
      cranelift_codegen::ir::MemFlagsData::trusted(),
      addr,
      0,
    )
  }

  fn store_reg_mem(&mut self, r: u8, v: IrValue) {
    let addr = self.reg_addr(r);
    self
      .fb
      .ins()
      .store(cranelift_codegen::ir::MemFlagsData::trusted(), v, addr, 0);
  }

  #[inline]
  fn is_f64_tracked(&self, r: u8) -> bool {
    self.f64_tracked.get(r as usize).copied().unwrap_or(false)
  }

  /// Whether the float Variable is the ONLY definition `r` gets, with
  /// the integer one materialized on demand instead of maintained in
  /// parallel: see `f64_canonical`.
  #[inline]
  fn is_f64_canonical(&self, r: u8) -> bool {
    self.f64_canonical.get(r as usize).copied().unwrap_or(false)
  }

  fn load_reg(&mut self, r: u8) -> IrValue {
    if self.is_f64_canonical(r) {
      let f = self.fb.use_var(self.reg_vars_f64[r as usize]);
      return self.from_f64(f);
    }
    self.fb.use_var(self.reg_vars[r as usize])
  }

  /// The register's bits viewed as an `f64`. For a tracked register
  /// this is the float Variable itself, so a value produced by a float
  /// instruction reaches the next one without ever leaving the vector
  /// register file.
  fn load_reg_f64(&mut self, r: u8) -> IrValue {
    if self.is_f64_tracked(r) {
      return self.fb.use_var(self.reg_vars_f64[r as usize]);
    }
    let v = self.load_reg(r);
    self.to_f64(v)
  }

  /// `load_reg_f64` for the fast arm of a RUNTIME-guarded instruction,
  /// where the caller already loaded the register's `I64` value to
  /// build the guard out of.
  ///
  /// Taking the Variable here rather than bitcasting `v` is worth real
  /// time even though this arm sits behind a runtime guard: a guard
  /// that passes is the common case for genuinely numeric code that
  /// simply isn't statically provable, which is most of a hot loop
  /// whose values come back from calls or element reads.
  /// `spectral-norm.zu` is entirely this shape, and routing it through
  /// a local bitcast instead gives up the whole win there.
  ///
  /// `v` has to still BE `r`'s current value at this point, since for
  /// a tracked register this reads the float Variable as it stands now
  /// and ignores `v` entirely. In practice that means calling this
  /// before the instruction stores its result, which matters whenever
  /// a destination register aliases an operand (`x = x + y`).
  fn f64_view(&mut self, r: u8, v: IrValue) -> IrValue {
    if self.is_f64_tracked(r) {
      return self.fb.use_var(self.reg_vars_f64[r as usize]);
    }
    self.to_f64(v)
  }

  /// Defines `r` from an `I64` value. Every definition site has to go
  /// through here (or `store_reg_f64`) so the register's two views can
  /// never drift apart on some path.
  fn def_reg_both(&mut self, r: u8, v: IrValue) {
    if self.is_f64_tracked(r) {
      let f = self.to_f64(v);
      self.fb.def_var(self.reg_vars_f64[r as usize], f);
      if self.is_f64_canonical(r) {
        return;
      }
    }
    self.fb.def_var(self.reg_vars[r as usize], v);
  }

  fn store_reg(&mut self, r: u8, v: IrValue) {
    self.def_reg_both(r, v);
    self.scalar_lists.remove(&r);
    self.scalar_instances.remove(&r);
  }

  /// The float-producing counterpart of `store_reg`. The `F64` view is
  /// defined from `f` directly, with no bitcast at all; a canonical
  /// register stops right there, and for the rest Cranelift's own DCE
  /// still drops the integer view wherever nothing reads it.
  fn store_reg_f64(&mut self, r: u8, f: IrValue) {
    if self.is_f64_tracked(r) {
      self.fb.def_var(self.reg_vars_f64[r as usize], f);
    }
    if !self.is_f64_canonical(r) {
      let v = self.from_f64(f);
      self.fb.def_var(self.reg_vars[r as usize], v);
    }
    self.scalar_lists.remove(&r);
    self.scalar_instances.remove(&r);
  }

  fn store_reg_int(&mut self, r: u8, i: IrValue) {
    if self.int_tracked[r as usize] {
      self.fb.def_var(self.reg_vars_int[r as usize], i);
    }
    let f = self.fb.ins().fcvt_from_sint(types::F64, i);
    self.store_reg_f64(r, f);
  }

  fn flush_live(&mut self, ip: usize) {
    let live: Vec<u8> = self.liveness.live_regs_at(ip).collect();
    for r in live {
      let v = self.load_reg(r);
      self.store_reg_mem(r, v);
    }
    self.flush_globals();
  }

  fn reload_live(&mut self, ip: usize) {
    self.refresh_regs();
    let live: Vec<u8> = self.liveness.live_regs_at(ip).collect();
    for r in live {
      let v = self.load_reg_mem(r);
      self.def_reg_both(r, v);
      if self.int_tracked[r as usize] && self.int_facts.is_int(ip, r) {
        let f = self.to_f64(v);
        let vi = self.fb.ins().fcvt_to_sint_sat(types::I64, f);
        self.fb.def_var(self.reg_vars_int[r as usize], vi);
      }
    }
    self.reload_globals();
  }

  fn flush_globals(&mut self) {
    if self.global_vars.is_empty() {
      return;
    }
    let flags = cranelift_codegen::ir::MemFlagsData::trusted();
    let slots_ptr = self.fb.ins().load(
      types::I64,
      flags,
      self.vm_param,
      GLOBAL_SLOTS_PTR_CACHE_OFFSET,
    );
    for (&slot, &var) in &self.global_vars {
      let v = self.fb.use_var(var);
      let byte_off = (slot * 8) as i32;
      self.fb.ins().store(flags, v, slots_ptr, byte_off);
    }
  }

  fn reload_globals(&mut self) {
    if self.global_vars.is_empty() {
      return;
    }
    let flags = cranelift_codegen::ir::MemFlagsData::trusted();
    let slots_ptr = self.fb.ins().load(
      types::I64,
      flags,
      self.vm_param,
      GLOBAL_SLOTS_PTR_CACHE_OFFSET,
    );
    for (&slot, &var) in &self.global_vars {
      let byte_off = (slot * 8) as i32;
      let v = self.fb.ins().load(types::I64, flags, slots_ptr, byte_off);
      self.fb.def_var(var, v);
    }
  }

  fn jump_target_block(&self, ip: usize, target_ip: usize) -> Block {
    self
      .blocks
      .get(target_ip)
      .copied()
      .unwrap_or(self.blocks[ip])
  }

  /// Direct load of `VM::regs_ptr_cache` at its compile-time-baked
  /// offset; no FFI call. Sound as long as every reallocation of
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
  /// immediate: see this module's docs on why this never needs a
  /// runtime `chunk.constants[idx]` load. Sound specifically because
  /// `Compiler` allocates every object-typed constant (strings,
  /// bigints, nested function prototypes) via `Heap::alloc_old`/
  /// `alloc_function`, NEVER the young-generation nursery a minor
  /// collection can relocate out from under an already-baked
  /// immediate with no way to fix it back up: see those functions'
  /// own docs. A plain number/nil/bool constant has no address to go
  /// stale in the first place.
  fn bake_const(&mut self, idx: u16) -> IrValue {
    let v = self.proto.chunk.constants[idx as usize];
    self.u64c(v.to_bits())
  }

  /// The numeric constant at `idx` as a real `F64` value. Distinct
  /// from `bake_f64_bits`, which produces the same number as an `I64`
  /// integer immediate: that form is what the slow-path helpers take,
  /// but feeding it to an `fadd`/`fmul` would cost a `movq` across to
  /// the vector side that `f64const` doesn't.
  fn bake_f64(&mut self, r: u8, idx: u16) -> IrValue {
    let v = self.proto.chunk.constants[idx as usize];
    debug_assert!(v.is_number());
    // Only worth the constant pool when `r` genuinely lives in the
    // float domain. Otherwise its own value had to be bitcast into an
    // xmm register to get here anyway, and a RIP-relative load for the
    // constant just adds a second memory access where a `movabs`
    // immediate would have done; the same trap `Instr::LoadConst` fell
    // into, and worth ~5% on `pidigits.zu` between them.
    if self.is_f64_tracked(r) {
      return self.fb.ins().f64const(v.as_number());
    }
    let bits = self.u64c(v.as_number().to_bits());
    self.to_f64(bits)
  }

  fn bake_f64_bits(&mut self, idx: u16) -> IrValue {
    let v = self.proto.chunk.constants[idx as usize];
    debug_assert!(v.is_number());
    self.u64c(v.as_number().to_bits())
  }

  /// A stable pointer to the CURRENTLY COMPILING `ObjFunction` itself,
  /// baked as an immediate; sound because `Heap::alloc_function`
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

  /// The REAL call instruction; no register-cache bookkeeping at all.
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
  /// `mark_stale_live` against `self.current_ip`; the register-cache
  /// half of "every helper call is conservatively treated as a genuine
  /// sync point" (the OTHER half, `refresh_regs`'s pointer refresh, is
  /// unchanged and still each call site's own responsibility, exactly
  /// as before this cache existed). Centralizing this here, rather than
  /// in each of `call_checked`/`emit_fast_call`/`emit_is_falsey`/
  /// `emit_safepoint`/`UsingJump`'s own codegen, means every one of
  /// this file's ~40 call sites gets correct treatment automatically,
  /// keyed off whichever bytecode instruction is currently being
  /// translated: see `current_ip`'s own docs.
  fn call_helper(&mut self, name: &str, args: &[IrValue]) -> IrValue {
    self.publish_ip();
    self.flush_live(self.current_ip);
    let result = self.call_helper_raw(name, args);
    self.reload_live(self.current_ip);
    result
  }

  /// Publishes this frame's current bytecode position to `VM::jit_ip`,
  /// so a stack trace built from here names the right source line.
  ///
  /// The interpreter keeps `CallFrame::ip` current by storing it on
  /// EVERY instruction, precisely because any instruction can raise.
  /// Compiled code cannot afford that, and does not need it: the only
  /// ways a compiled frame's position ever becomes observable are
  /// raising an error and becoming the caller of a new frame, and
  /// BOTH happen inside a `jit::runtime` helper. So publishing once per
  /// helper call; a single store of an immediate to a fixed `VM`
  /// offset, on a path that is already paying for an FFI call; covers
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
  /// from the WHOLE compiled function (the error is already sitting
  /// in `VM::jit_pending_error`, ready for `VM::invoke_compiled` to
  /// pick up) rather than continuing this instruction's own codegen.
  /// Always refreshes the registers pointer afterward: see this
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

  /// `SigRef` for `jit::EntryFn`'s own call shape; what every fast-
  /// path `call_indirect` in `emit_fast_call` targets. Imported at most
  /// once per compiled function and cached, since it's the exact same
  /// shape at every call site.
  fn reg_addr_at_base(&mut self, base: IrValue, r: u8) -> IrValue {
    let regs = self.fb.use_var(self.regs_var);
    let base_bytes = self.fb.ins().imul_imm_s(base, 8);
    let with_base = self.fb.ins().iadd(regs, base_bytes);
    if r == 0 {
      with_base
    } else {
      self.fb.ins().iadd_imm_s(with_base, (r as i64) * 8)
    }
  }

  fn load_reg_mem_at_base(&mut self, base: IrValue, r: u8) -> IrValue {
    let addr = self.reg_addr_at_base(base, r);
    self.fb.ins().load(
      types::I64,
      cranelift_codegen::ir::MemFlagsData::trusted(),
      addr,
      0,
    )
  }

  fn load_call_arg_values(&mut self, first_arg_reg: u8, num_args: u8) -> [IrValue; 4] {
    let nil_c = self.u64c(crate::vm::value::Value::nil().to_bits());
    let mut args = [nil_c, nil_c, nil_c, nil_c];
    for k in 0..4u8 {
      if k < num_args {
        args[k as usize] = self.load_reg(first_arg_reg + k);
      }
    }
    args
  }

  /// Argument marshalling for a construct site whose `prepare` leaves
  /// the arguments exactly where the call site wrote them: the KNOWN
  /// path, whose callee window starts at the callee register itself, so
  /// `VM::prepare_known_construction` drops the instance onto that
  /// register and shifts nothing. The dynamic path does move them, and
  /// has its own loader; see `load_shifted_construct_arg_values`.
  fn load_construct_arg_values(
    &mut self,
    instance: IrValue,
    first_arg_reg: u8,
    num_args: u8,
  ) -> [IrValue; 4] {
    let nil_c = self.u64c(crate::vm::value::Value::nil().to_bits());
    let mut args = [instance, nil_c, nil_c, nil_c];
    for k in 0..3u8 {
      if k < num_args {
        args[(k + 1) as usize] = self.load_reg(first_arg_reg + k);
      }
    }
    args
  }

  /// Argument marshalling for the DYNAMIC construct path, where the
  /// arguments are no longer where the call site put them.
  ///
  /// `Instr::Call` on a class leaves `[class, arg0, ..]`, but a
  /// constructor's window has to read `[self, arg0, ..]`, so
  /// `VM::prepare_compiled_construction` shifts every argument up one
  /// slot and drops the instance into the gap. Reading them back at
  /// their pre-shift registers hands the constructor its own `self` as
  /// the first argument and drops the last one: `Tag('kept')` through a
  /// warmed-up site whose class was swapped comes back with `name`
  /// holding the instance (see `tests/construct-class-swap.zu`).
  ///
  /// Out of memory rather than the caller's Variables, because the
  /// topmost argument's new home sits above the caller's own live set;
  /// nothing tracks a Variable for it, so `reload_live` never refreshed
  /// one.
  fn load_shifted_construct_arg_values(
    &mut self,
    instance: IrValue,
    new_base: IrValue,
    num_args: u8,
  ) -> [IrValue; 4] {
    let nil_c = self.u64c(crate::vm::value::Value::nil().to_bits());
    let mut args = [instance, nil_c, nil_c, nil_c];
    for k in 0..3u8 {
      if k < num_args {
        args[(k + 1) as usize] = self.load_reg_mem_at_base(new_base, k + 1);
      }
    }
    args
  }

  fn entry_sig_ref(&mut self) -> SigRef {
    if let Some(sig) = self.entry_sig {
      return sig;
    }
    let mut sig = self.module.make_signature();
    sig.params.push(AbiParam::new(types::I64)); // vm
    sig.params.push(AbiParam::new(types::I64)); // base
    sig.params.push(AbiParam::new(types::I64)); // closure
    sig.params.push(AbiParam::new(types::I32)); // osr_id
    sig.params.push(AbiParam::new(types::I64)); // a0
    sig.params.push(AbiParam::new(types::I64)); // a1
    sig.params.push(AbiParam::new(types::I64)); // a2
    sig.params.push(AbiParam::new(types::I64)); // a3
    sig.returns.push(AbiParam::new(types::I64));
    let sig_ref = self.fb.import_signature(sig);
    self.entry_sig = Some(sig_ref);
    sig_ref
  }

  /// The 8-byte scratch stack slot `prepare` helpers write the resolved
  /// callee closure's `Value` bits into: see `closure_out_slot`'s own
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

  /// Inlines `zuri_jit_direct_call_prepare`'s job for a callee whose
  /// `arity`/`variadic`/`num_registers` are already compile-time
  /// constants (self-recursion, or a `CallTarget::Known` callee once
  /// its value-identity guard has passed): see this module's own docs
  /// for the profiling that motivated this: `zuri_jit_direct_call_
  /// prepare`/`zuri_jit_call_finish` alone were measured at ~19% of a
  /// call-heavy benchmark's total runtime, almost entirely the fixed
  /// cost of the Rust-call boundary itself, crossed billions of times
  /// for work that's individually a handful of loads and stores.
  ///
  /// Callers must check eligibility THEMSELVES, at compile time, before
  /// calling this: `!callee_variadic && num_args == callee_arity`,
  /// exactly `VM::setup_closure_call`'s own fast-path gate (see
  /// `push_frame_fast`), just decided once here instead of freshly on
  /// every call. There is no runtime check for it; an ineligible
  /// callee must never reach this function at all, and instead keep
  /// using the unaccelerated `zuri_jit_direct_call_prepare` helper,
  /// unchanged.
  ///
  /// Three runtime conditions still gate the fast path, each rare once
  /// a hot call site reaches its steady state and each falling to
  /// `slow_block` (the caller's own, ordinary `zuri_jit_call` fallback
  ///; fully general and always correct, just unaccelerated for this
  /// one call): JIT call depth exhausted, the callee's register window
  /// doesn't already fit `VM::registers`, or the frame stack doesn't
  /// already have spare capacity. None of these three needs a "grow and
  /// continue inline" path; deferring to the ordinary slow call for
  /// the rare call that actually needs to grow something is simpler and
  /// no less correct than reimplementing `Vec`-style growth here too.
  ///
  /// On success, this frame's `compiled` flag is set to `true` directly
  /// as part of constructing it (no separate `mark_top_frame_compiled`
  /// call), `VM::jit_call_depth` is incremented, and `publish_ip` is
  /// called explicitly; both of those happen for free inside
  /// `call_helper`/`call_checked` normally, but this function doesn't
  /// go through either, so the caller must not ALSO call them.
  /// The three runtime preconditions `emit_inline_frame_push` (and the
  /// construct path's own inline fast path) both need verified BEFORE
  /// doing anything irreversible; JIT call depth, the callee's
  /// register window already fitting, and the frame stack already
  /// having spare capacity. Split out specifically so a caller that
  /// ALSO needs to do its own side effect between "these hold" and
  /// "push the frame" (construction's instance allocation + `gc_pins`
  /// push, which must never happen if the push that's supposed to
  /// follow it is about to fail and fall back to a path that allocates
  /// its OWN instance) can check first and only commit once every
  /// precondition is confirmed. See `emit_inline_construct`'s own docs
  /// for exactly why this ordering matters there.
  ///
  /// Returns `(depth, frames_len)`; both already loaded as part of
  /// the checks, and both needed again by the frame construction that
  /// follows, so callers reuse them instead of reloading.
  fn emit_call_checks(
    &mut self,
    new_base: IrValue,
    callee_num_registers: u8,
    slow_block: Block,
  ) -> (IrValue, IrValue) {
    let num_regs = self.i64c(callee_num_registers as i64);
    self.emit_call_checks_dyn(new_base, num_regs, slow_block)
  }

  fn emit_call_checks_dyn(
    &mut self,
    new_base: IrValue,
    callee_num_registers: IrValue,
    slow_block: Block,
  ) -> (IrValue, IrValue) {
    let vm = self.vm_param;
    let flags = cranelift_codegen::ir::MemFlagsData::trusted();

    let depth = self
      .fb
      .ins()
      .load(types::I32, flags, vm, JIT_CALL_DEPTH_OFFSET);
    let max_depth = self
      .fb
      .ins()
      .iconst(types::I32, vm::JIT_MAX_CALL_DEPTH as i64);
    let depth_ok = self
      .fb
      .ins()
      .icmp(IntCC::UnsignedLessThan, depth, max_depth);
    let depth_block = self.fb.create_block();
    self
      .fb
      .ins()
      .brif(depth_ok, depth_block, &[], slow_block, &[]);

    self.fb.switch_to_block(depth_block);
    let regs_len = self
      .fb
      .ins()
      .load(types::I64, flags, vm, REGS_LEN_CACHE_OFFSET);
    let needed = self.fb.ins().iadd(new_base, callee_num_registers);
    let regs_ok = self
      .fb
      .ins()
      .icmp(IntCC::UnsignedGreaterThanOrEqual, regs_len, needed);
    let regs_block = self.fb.create_block();
    self
      .fb
      .ins()
      .brif(regs_ok, regs_block, &[], slow_block, &[]);

    self.fb.switch_to_block(regs_block);
    let frames_len = self.fb.ins().load(types::I64, flags, vm, FRAMES_LEN_OFFSET);
    let frames_cap = self.fb.ins().load(types::I64, flags, vm, FRAMES_CAP_OFFSET);
    let frames_ok = self
      .fb
      .ins()
      .icmp(IntCC::UnsignedLessThan, frames_len, frames_cap);
    let frames_block = self.fb.create_block();
    self
      .fb
      .ins()
      .brif(frames_ok, frames_block, &[], slow_block, &[]);

    self.fb.switch_to_block(frames_block);
    (depth, frames_len)
  }

  /// The closure the frame on top of the stack is running, read fresh
  /// from memory. `closure_param` is a copy taken at entry, so a
  /// collection that relocates the closure mid-invocation leaves it
  /// pointing at a slot the nursery has since reused; the frame field is
  /// a GC root and is rewritten by every collection instead.
  fn emit_current_closure_val(&mut self) -> IrValue {
    let flags = cranelift_codegen::ir::MemFlagsData::trusted();
    let vm = self.vm_param;
    let frames_ptr = self.fb.ins().load(types::I64, flags, vm, FRAMES_PTR_OFFSET);
    let frames_len = self.fb.ins().load(types::I64, flags, vm, FRAMES_LEN_OFFSET);
    let top = self.fb.ins().iadd_imm_s(frames_len, -1);
    let frame_off = self.fb.ins().imul_imm_s(top, CALL_FRAME_SIZE);
    let frame_addr = self.fb.ins().iadd(frames_ptr, frame_off);
    self
      .fb
      .ins()
      .load(types::I64, flags, frame_addr, CALL_FRAME_CLOSURE_VAL_OFFSET)
  }

  fn emit_inline_frame_push(
    &mut self,
    proto_bits: u64,
    callee_num_registers: u8,
    dst: u8,
    new_base: IrValue,
    closure_val: IrValue,
    slow_block: Block,
  ) {
    let (depth, frames_len) = self.emit_call_checks(new_base, callee_num_registers, slow_block);
    let proto_c = self.u64c(proto_bits);
    self.emit_frame_construction(proto_c, dst, new_base, closure_val, depth, frames_len);
  }

  /// The actual `CallFrame` construction + `frames`/`jit_call_depth`
  /// bump, factored out of `emit_inline_frame_push` so
  /// `emit_inline_construct` can reuse it AFTER its own allocation step,
  /// using the `(depth, frames_len)` `emit_call_checks` already
  /// verified and returned; every precondition for this to be safe
  /// was already confirmed by whichever `emit_call_checks` call led
  /// here.
  fn emit_frame_construction(
    &mut self,
    proto_bits: IrValue,
    dst: u8,
    new_base: IrValue,
    closure_val: IrValue,
    depth: IrValue,
    frames_len: IrValue,
  ) {
    let vm = self.vm_param;
    let flags = cranelift_codegen::ir::MemFlagsData::trusted();

    let frames_ptr = self.fb.ins().load(types::I64, flags, vm, FRAMES_PTR_OFFSET);
    let frame_off = self.fb.ins().imul_imm_s(frames_len, CALL_FRAME_SIZE);
    let frame_addr = self.fb.ins().iadd(frames_ptr, frame_off);

    self
      .fb
      .ins()
      .store(flags, proto_bits, frame_addr, CALL_FRAME_FUNCTION_OFFSET);
    self.fb.ins().store(
      flags,
      closure_val,
      frame_addr,
      CALL_FRAME_CLOSURE_VAL_OFFSET,
    );
    let zero = self.i64c(0);
    self
      .fb
      .ins()
      .store(flags, zero, frame_addr, CALL_FRAME_IP_OFFSET);
    self
      .fb
      .ins()
      .store(flags, new_base, frame_addr, CALL_FRAME_BASE_OFFSET);
    let dst_c = self.fb.ins().iconst(types::I8, dst as i64);
    self
      .fb
      .ins()
      .store(flags, dst_c, frame_addr, CALL_FRAME_DST_IN_CALLER_OFFSET);
    let scalar_mark = self
      .fb
      .ins()
      .load(types::I64, flags, vm, JIT_SCALAR_ROOTS_LEN_OFFSET);
    self.fb.ins().store(
      flags,
      scalar_mark,
      frame_addr,
      CALL_FRAME_SCALAR_ROOTS_MARK_OFFSET,
    );
    let true_c = self.fb.ins().iconst(types::I8, 1);
    self
      .fb
      .ins()
      .store(flags, true_c, frame_addr, CALL_FRAME_COMPILED_OFFSET);

    let new_frames_len = self.fb.ins().iadd_imm_s(frames_len, 1);
    self
      .fb
      .ins()
      .store(flags, new_frames_len, vm, FRAMES_LEN_OFFSET);

    let new_depth = self.fb.ins().iadd_imm_s(depth, 1);
    self
      .fb
      .ins()
      .store(flags, new_depth, vm, JIT_CALL_DEPTH_OFFSET);

    self.publish_ip();
  }

  /// `emit_inline_frame_push`'s other half; inlines
  /// `zuri_jit_call_finish`'s job once the callee's own `call_indirect`
  /// has returned. `new_base` must be the exact value passed to the
  /// matching `emit_inline_frame_push` call (needed only for the rare
  /// `close_upvalues_from` case); `ret_bits` is the callee's raw return
  /// value.
  ///
  /// Every exit path here writes `dst` the same way
  /// `zuri_jit_call_finish` itself always did; straight into `VM::
  /// registers` memory, with `reg_cache[dst]` marked `Stale` rather than
  /// going through `store_reg`'s `Variable`. That's deliberate, not an
  /// oversight: this function has multiple internal branches (deopt,
  /// error, ordinary success) that all reach the same `done_block`,
  /// and `dst`'s `Variable` is never defined on the deopt/error
  /// paths at all (they write memory directly, exactly like the helper
  /// calls they replace). A `Dirty` marking on the success path only
  /// would leave a LATER `load_reg(dst)` trusting a `Variable` that was
  /// only ever defined on ONE of several incoming edges; unsound
  /// regardless of which edge actually ran at runtime. Uniform `Stale`
  /// is what `zuri_jit_call_finish`'s own call-based version already
  /// guaranteed for free; this preserves that exactly.
  fn emit_inline_frame_finish(&mut self, dst: u8, new_base: IrValue, ret_bits: IrValue) {
    let vm = self.vm_param;
    let flags = cranelift_codegen::ir::MemFlagsData::trusted();

    let depth = self
      .fb
      .ins()
      .load(types::I32, flags, vm, JIT_CALL_DEPTH_OFFSET);
    let new_depth = self.fb.ins().iadd_imm_s(depth, -1);
    self
      .fb
      .ins()
      .store(flags, new_depth, vm, JIT_CALL_DEPTH_OFFSET);

    let done_block = self.fb.create_block();

    let deopt_ip = self
      .fb
      .ins()
      .load(types::I64, flags, vm, PENDING_DEOPT_IP_OFFSET);
    let neg1 = self.i64c(-1);
    let no_deopt = self.fb.ins().icmp(IntCC::Equal, deopt_ip, neg1);
    let deopt_block = self.fb.create_block();
    let past_deopt_block = self.fb.create_block();
    self
      .fb
      .ins()
      .brif(no_deopt, past_deopt_block, &[], deopt_block, &[]);

    self.fb.switch_to_block(deopt_block);
    let base = self.base_param;
    let dst_i = self.idx(dst);
    self.call_checked("zuri_jit_finish_deopt", &[vm, base, dst_i]);
    self.fb.ins().jump(done_block, &[]);

    self.fb.switch_to_block(past_deopt_block);
    let exc = self
      .fb
      .ins()
      .load(types::I64, flags, vm, JIT_PENDING_EXCEPTION_OFFSET);
    let nil = self.u64c(value::NIL_VAL);
    let no_exc = self.fb.ins().icmp(IntCC::Equal, exc, nil);
    let exc_block = self.fb.create_block();
    let ok_block = self.fb.create_block();
    self.fb.ins().brif(no_exc, ok_block, &[], exc_block, &[]);

    self.fb.switch_to_block(exc_block);
    let junk = self.i64c(0);
    self.fb.ins().return_(&[junk]);

    self.fb.switch_to_block(ok_block);
    let has_open = self
      .fb
      .ins()
      .load(types::I8, flags, vm, HAS_OPEN_UPVALUES_OFFSET);
    let zero8 = self.fb.ins().iconst(types::I8, 0);
    let none_open = self.fb.ins().icmp(IntCC::Equal, has_open, zero8);
    let pop_block = self.fb.create_block();
    let close_block = self.fb.create_block();
    self
      .fb
      .ins()
      .brif(none_open, pop_block, &[], close_block, &[]);

    self.fb.switch_to_block(close_block);
    let zero = self.i64c(0);
    self.call_checked("zuri_jit_close_upvalues", &[vm, new_base, zero]);
    self.fb.ins().jump(pop_block, &[]);

    self.fb.switch_to_block(pop_block);
    self.emit_pop_top_frame();
    self.store_reg_mem(dst, ret_bits);
    self.fb.ins().jump(done_block, &[]);

    self.fb.switch_to_block(done_block);
    let v = self.load_reg_mem(dst);
    self.store_reg(dst, v);
  }

  /// Pops `VM::frames`' own top entry and restores `VM::
  /// jit_scalar_roots_len` to what it held before that frame was
  /// pushed; the exact two-step `VM::pop_frame_inner` does, inlined.
  /// Shared by `emit_inline_frame_finish` and
  /// `emit_inline_construct_finish`, which differ only in what they do
  /// with the frame's own former caller-return-value slot afterward.
  fn emit_pop_top_frame(&mut self) {
    let vm = self.vm_param;
    let flags = cranelift_codegen::ir::MemFlagsData::trusted();
    let frames_len = self.fb.ins().load(types::I64, flags, vm, FRAMES_LEN_OFFSET);
    let top_idx = self.fb.ins().iadd_imm_s(frames_len, -1);
    let frames_ptr = self.fb.ins().load(types::I64, flags, vm, FRAMES_PTR_OFFSET);
    let frame_off = self.fb.ins().imul_imm_s(top_idx, CALL_FRAME_SIZE);
    let frame_addr = self.fb.ins().iadd(frames_ptr, frame_off);
    let mark = self.fb.ins().load(
      types::I64,
      flags,
      frame_addr,
      CALL_FRAME_SCALAR_ROOTS_MARK_OFFSET,
    );
    self
      .fb
      .ins()
      .store(flags, mark, vm, JIT_SCALAR_ROOTS_LEN_OFFSET);
    self.fb.ins().store(flags, top_idx, vm, FRAMES_LEN_OFFSET);
  }

  /// `emit_inline_frame_finish`'s construct-call counterpart; the
  /// same deopt/error/upvalue/pop shape, but discarding the
  /// constructor's own return value in favour of the instance
  /// `emit_inline_construct` pinned, and needing the `gc_pins` release
  /// `zuri_jit_take_constructed_instance` does. See `zuri_jit_new_finish`
  /// 's own docs for why that release happens exactly here (after the
  /// deopt/error checks; unlike the ordinary-call finish, this
  /// one has a real resource to release regardless of which of those
  /// two fire) and `emit_inline_frame_finish`'s own docs for why every
  /// exit path writes `dst` the same uniform way.
  fn emit_inline_construct_finish(&mut self, dst: u8, new_base: IrValue) {
    let vm = self.vm_param;
    let flags = cranelift_codegen::ir::MemFlagsData::trusted();

    let depth = self
      .fb
      .ins()
      .load(types::I32, flags, vm, JIT_CALL_DEPTH_OFFSET);
    let new_depth = self.fb.ins().iadd_imm_s(depth, -1);
    self
      .fb
      .ins()
      .store(flags, new_depth, vm, JIT_CALL_DEPTH_OFFSET);

    let done_block = self.fb.create_block();

    // Deopt is checked BEFORE the `gc_pins` release below, and NOT
    // released on this branch until after `zuri_jit_finish_deopt`
    // returns; matching `zuri_jit_new_finish`'s own strict ordering.
    // Resolving a deopt resumes the constructor through the
    // interpreter, which can run arbitrary Zuri code (a collection
    // included); the instance must stay pinned for every moment that's
    // happening, or a relocation would leave `gc_pins`' own copy
    // correctly updated while a bare local `Value` read out beforehand
    // silently didn't.
    let deopt_ip = self
      .fb
      .ins()
      .load(types::I64, flags, vm, PENDING_DEOPT_IP_OFFSET);
    let neg1 = self.i64c(-1);
    let no_deopt = self.fb.ins().icmp(IntCC::Equal, deopt_ip, neg1);
    let deopt_block = self.fb.create_block();
    let past_deopt_block = self.fb.create_block();
    self
      .fb
      .ins()
      .brif(no_deopt, past_deopt_block, &[], deopt_block, &[]);

    self.fb.switch_to_block(deopt_block);
    let base = self.base_param;
    let dst_i = self.idx(dst);
    // `zuri_jit_finish_deopt` writes ITS OWN resolved value into `dst`
    // on success; fine for the ordinary-call finish (that value IS
    // the answer), wrong here (the answer is always the instance, never
    // whatever the constructor body itself returned). Overwritten
    // unconditionally right after: cheap, and simpler than a construct-
    // specific deopt-finish helper for a path this rare.
    self.call_checked("zuri_jit_finish_deopt", &[vm, base, dst_i]);
    let instance_after_deopt = self.call_helper("zuri_jit_take_constructed_instance", &[vm]);
    self.store_reg_mem(dst, instance_after_deopt);
    self.fb.ins().jump(done_block, &[]);

    self.fb.switch_to_block(past_deopt_block);
    // No deopt was pending, so nothing has run any Zuri code since the
    // check above; safe to release the pin here, still strictly
    // before the error check, matching the original's own order.
    let instance = self.call_helper("zuri_jit_take_constructed_instance", &[vm]);
    let exc = self
      .fb
      .ins()
      .load(types::I64, flags, vm, JIT_PENDING_EXCEPTION_OFFSET);
    let nil = self.u64c(value::NIL_VAL);
    let no_exc = self.fb.ins().icmp(IntCC::Equal, exc, nil);
    let exc_block = self.fb.create_block();
    let ok_block = self.fb.create_block();
    self.fb.ins().brif(no_exc, ok_block, &[], exc_block, &[]);

    self.fb.switch_to_block(exc_block);
    let junk = self.i64c(0);
    self.fb.ins().return_(&[junk]);

    self.fb.switch_to_block(ok_block);
    let has_open = self
      .fb
      .ins()
      .load(types::I8, flags, vm, HAS_OPEN_UPVALUES_OFFSET);
    let zero8 = self.fb.ins().iconst(types::I8, 0);
    let none_open = self.fb.ins().icmp(IntCC::Equal, has_open, zero8);
    let pop_block = self.fb.create_block();
    let close_block = self.fb.create_block();
    self
      .fb
      .ins()
      .brif(none_open, pop_block, &[], close_block, &[]);

    self.fb.switch_to_block(close_block);
    let zero = self.i64c(0);
    self.call_checked("zuri_jit_close_upvalues", &[vm, new_base, zero]);
    self.fb.ins().jump(pop_block, &[]);

    self.fb.switch_to_block(pop_block);
    self.emit_pop_top_frame();
    self.store_reg_mem(dst, instance);
    self.fb.ins().jump(done_block, &[]);

    self.fb.switch_to_block(done_block);
    let v = self.load_reg_mem(dst);
    self.store_reg(dst, v);
  }

  /// Inlines `zuri_jit_construct_prepare`'s job for a `CallTarget::
  /// ConstructKnown` callee once its class-identity/generation guard
  /// has already passed: see `emit_inline_frame_push`'s own docs for
  /// the general reasoning (same profiling motivation, same three
  /// runtime preconditions via `emit_call_checks`). What's different
  /// about construction: allocating the instance is a real,
  /// irreversible side effect (a heap allocation plus a `gc_pins`
  /// push), so `emit_call_checks` runs FIRST and only once it confirms
  /// the frame push that has to follow will actually succeed does this
  /// allocate anything; doing it the other way round risks an
  /// orphaned, permanently-pinned instance if the checks then fail and
  /// `slow_block` allocates its own.
  ///
  /// Unlike `emit_known_call`'s `entry`, the callee's compiled entry is
  /// NOT baked as an immediate here; read fresh via
  /// `PROTO_JIT_ENTRY_OFFSET` instead, matching `zuri_jit_construct_
  /// prepare`'s own behavior exactly (see `object::
  /// obj_function_jit_entry_offset`'s own docs for why: a small, simple
  /// constructor routinely finishes its own compile AFTER the function
  /// that constructs it already has, and re-reading live is what lets
  /// that caller start benefiting the moment it does, rather than
  /// being stuck on a stale miss for the rest of the run).
  ///
  /// Returns the resolved entry address for the caller's own
  /// `call_indirect`.
  fn emit_inline_construct(
    &mut self,
    ctor_bits: u64,
    proto_bits: u64,
    callee_num_registers: u8,
    dst: u8,
    func_reg: u8,
    new_base: IrValue,
    field_count: u16,
    slow_block: Block,
  ) -> IrValue {
    let (depth, frames_len) = self.emit_call_checks(new_base, callee_num_registers, slow_block);

    let vm = self.vm_param;
    let flags = cranelift_codegen::ir::MemFlagsData::trusted();
    let proto_c = self.u64c(proto_bits);
    let entry = self
      .fb
      .ins()
      .load(types::I64, flags, proto_c, PROTO_JIT_ENTRY_OFFSET);
    let zero = self.i64c(0);
    let entry_ok = self.fb.ins().icmp(IntCC::NotEqual, entry, zero);
    let alloc_block = self.fb.create_block();
    self
      .fb
      .ins()
      .brif(entry_ok, alloc_block, &[], slow_block, &[]);

    self.fb.switch_to_block(alloc_block);
    let base = self.base_param;
    let func_i = self.idx(func_reg);
    let field_count_c = self.i64c(field_count as i64);
    self.call_helper(
      "zuri_jit_alloc_and_pin_instance",
      &[vm, base, func_i, field_count_c],
    );
    let closure_c = self.u64c(ctor_bits);
    self.emit_frame_construction(proto_c, dst, new_base, closure_c, depth, frames_len);
    entry
  }

  /// Loads a proven-`Obj::List` receiver's element data pointer and
  /// length, straight out of the object.
  ///
  /// This used to be a real ABI call into `zuri_jit_list_data`, which
  /// sat in the innermost loop of every array-shaped program. The call
  /// itself was the smaller half of the cost: `call_helper` has to
  /// flush every live register to the VM's register file before it and
  /// mark them stale after, and Cranelift cannot move a single load or
  /// store across an opaque call, so the whole surrounding loop body
  /// lost its register allocation once per subscript. `ListStorage` is
  /// `#[repr(C)]` with `ptr` then `len` specifically so that this can
  /// be two plain loads instead (see `vm::list`).
  ///
  /// `readonly` is deliberately NOT set on these: a list's buffer moves
  /// on any growth, so a `ptr` loaded before an `append` must not be
  /// reused after it.
  ///
  /// A null data pointer means the elements are held inline in the
  /// object itself (see `vm::list::ListStorage`), which resolves to a
  /// compare and a select rather than a branch. Computing the inline
  /// address unconditionally is safe; it is arithmetic on a pointer
  /// this site has already proven points at a live `Obj::List`, and
  /// nothing is dereferenced until after the bounds check.
  fn load_list_ptr_len(&mut self, obj_ptr: IrValue) -> (IrValue, IrValue) {
    let flags = cranelift_codegen::ir::MemFlagsData::trusted();
    let heap_ptr = self
      .fb
      .ins()
      .load(types::I64, flags, obj_ptr, object::obj_list_ptr_offset());
    let len32 = self
      .fb
      .ins()
      .load(types::I32, flags, obj_ptr, object::obj_list_len_offset());
    let len = self.fb.ins().uextend(types::I64, len32);
    let inline_ptr = self
      .fb
      .ins()
      .iadd_imm_s(obj_ptr, object::obj_list_inline_offset() as i64);
    let zero = self.i64c(0);
    let is_inline = self.fb.ins().icmp(IntCC::Equal, heap_ptr, zero);
    let data_ptr = self.fb.ins().select(is_inline, inline_ptr, heap_ptr);
    (data_ptr, len)
  }

  /// The fast, inline-cache-style direct-call pattern shared by
  /// `Instr::Call` and `Instr::Invoke`: see `jit::runtime`'s "Fast,
  /// inline-cache-style direct calls" docs for the full protocol this
  /// implements. `prepare_helper` is `zuri_jit_call_prepare` or
  /// `zuri_jit_invoke_prepare`, called with `prepare_args` PLUS the
  /// address of the scratch closure-out slot (appended here, not by the
  /// caller); `new_base` is the callee's frame base (already computable
  /// at compile time as `base + reg + 1`, so there's no need for
  /// `prepare` to report it back). `slow_helper` (an ordinary
  /// `call_checked` target; `zuri_jit_call`/`zuri_jit_invoke`) is the
  /// fully general fallback for a `0` (not-yet-compiled, or not even a
  /// closure) result from `prepare`.
  fn emit_fast_call(
    &mut self,
    prepare_helper: &'static str,
    prepare_args: &[IrValue],
    new_base: IrValue,
    dst: u8,
    first_arg_reg: u8,
    num_args: u8,
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
    let [a0, a1, a2, a3] = self.load_call_arg_values(first_arg_reg, num_args);
    self.flush_live(self.current_ip);
    let call = self.fb.ins().call_indirect(
      sig,
      prepare,
      &[self.vm_param, new_base, closure_bits, neg1, a0, a1, a2, a3],
    );
    let ret_bits = self.fb.inst_results(call)[0];
    self.reload_live(self.current_ip);
    self.refresh_regs();
    let base = self.base_param;
    let dst_i = self.idx(dst);
    self.call_checked(
      "zuri_jit_call_finish",
      &[self.vm_param, base, dst_i, new_base, ret_bits],
    );
    self.resync_dst_from_memory(dst);
    self.fb.ins().jump(done_block, &[]);

    self.fb.switch_to_block(slow_block);
    self.call_checked(slow_helper, slow_args);
    self.resync_dst_from_memory(dst);
    self.fb.ins().jump(done_block, &[]);

    self.fb.switch_to_block(done_block);
  }

  /// Can this construction site skip allocating entirely, keeping the
  /// instance's fields in a Cranelift stack slot instead?
  ///
  /// Mirrors `scalar_replace_eligible`'s conditions, for the same
  /// reasons: see its docs for why `Move` and the speculative-body
  /// gate are ruled out wholesale rather than reasoned about:
  /// - `escape::analyze_one` proves the constructed value never leaves
  ///   this function (consulting this site's own class field-shadowing
  ///   safety, without which reading `d.x` alone would count as an
  ///   escape).
  /// - The constructor is simple enough to reproduce inline, i.e. it
  ///   only copies parameters into fields: see
  ///   `jit::ConstructInfo::simple_ctor_param_slots`.
  /// - No `Instr::Move` anywhere reads the destination register, so
  ///   `scalar_instances` only ever answers for the exact register
  ///   `analyze_one` reasoned about.
  ///
  /// Deliberately does NOT inherit `scalar_replace_eligible`'s
  /// "no specialized/speculative body" gate. That gate exists for a
  /// specific, reproduced speculation bug involving a list read through
  /// a VARIABLE index (see its docs); and a scalar-replaced instance
  /// has no variable index anywhere: every field resolves to a
  /// compile-time-constant slot, emitted as a fixed-offset
  /// `stack_load`/`stack_store`. Keeping the gate here would disable
  /// this optimization on essentially every real numeric workload,
  /// since those are exactly the functions that get a specialized
  /// body.
  fn escape_analyze_one(
    &self,
    alloc_ip: usize,
    self_class_safety: Option<&escape::ClassFieldSafety>,
    alloc_class_safety: Option<&escape::ClassFieldSafety>,
  ) -> escape::EscapeResult {
    let self_ref = self
      .self_ref_facts
      .get_or_init(|| escape::self_reference_facts_with_preds(self.proto, &self.preds));
    let self_summary = self.self_summary.get_or_init(|| {
      escape::compute_param_summary_with_facts(self.proto, self_class_safety, &self.preds, self_ref)
    });
    escape::analyze_one_with_facts(
      self.proto,
      alloc_ip,
      self_class_safety,
      alloc_class_safety,
      &self.preds,
      self_ref,
      &self_summary.param_escapes,
    )
  }

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
    let rewritten = self
      .proto
      .chunk
      .code
      .iter()
      .enumerate()
      .any(|(i, instr)| i > ip && typeflow::any_dst(instr) == Some(dst));
    if rewritten {
      return false;
    }
    !self
      .escape_analyze_one(ip, None, Some(&info.safety))
      .escapes
  }

  /// Builds a proven-non-escaping instance with NO heap allocation:
  /// the constructor's parameter-to-field copies are replayed straight
  /// into a stack slot, and no object, no `@new` call, and no GC work
  /// happen at all.
  ///
  /// Every slot is initialized; the ones the constructor writes from
  /// its arguments, and any remaining declared field to nil; before
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

    // The slot was allocated in the pre-pass before `emit_entry_
    // dispatch`, so the OSR paths already know to refill it. It is
    // always present here: this is only reached when
    // `scalar_construct_eligible` held, and that forbids any other
    // write to `dst`, so a register has at most one such site.
    let &(slot, site) = self
      .planned_scalar_instances
      .get(&dst)
      .expect("scalar construct slot registered in the pre-pass");
    debug_assert_eq!(site, ip, "one scalar construct site per register");

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
    if crate::jit::log_enabled() {
      eprintln!(
        "[sis] {} obj=r{} site=ip{} slot_ok={}",
        self.proto.name,
        obj,
        site,
        self.construct_info.contains_key(&site)
      );
    }
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
  /// resolution: see `VM::resolve_construct_target` for the proof
  /// and `zuri_jit_construct_prepare` for what the guard licenses.
  ///
  /// The guard is class IDENTITY plus a `method_table_generation`
  /// match, the same pair (and the same reasoning) as
  /// `emit_self_invoke`'s. It only works because classes are
  /// old-generation allocations and therefore never relocate: see
  /// `Heap::alloc_class`. On a miss (a global rebound to a different
  /// class, or a class monkey-patched after this function compiled)
  /// control falls into the ordinary dynamic construction path, which
  /// re-resolves everything itself and is still correct.
  fn emit_construct_known(
    &mut self,
    ip: usize,
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

    // Every operand either arm needs is materialized HERE, in the
    // block that dominates all of them. A value defined inside one arm
    // and used from another is exactly the dominance error this file
    // already tripped over once; and it surfaces as a silent
    // Cranelift verifier failure that makes the whole function
    // JIT-ineligible, which reads as a performance regression rather
    // than as the bug it is. Check `ZURI_JIT_LOG=1` for `ineligible:`
    // after touching this.
    // `func`, NOT `func + 1`: this path's callee window deliberately
    // starts at the callee register itself so the constructor's
    // arguments need no shifting: see
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

    // Eligibility, exactly like `emit_self_call`'s/`emit_known_call`'s:
    // the receiver ("self") occupies the constructor's own register 0,
    // so the real argument count to compare against `arity` is
    // `1 + num_args`, matching `prepare_known_construction`'s own
    // `num_args + 1` convention.
    //
    // SAFETY: `proto_ptr` names a live, old-generation `ObjFunction` --
    // see `CallTarget::ConstructKnown::proto_ptr`'s own docs.
    let ctor_proto = unsafe { &*(proto_ptr as *const ObjFunction) };
    let simple_ctor_param_slots = self
      .construct_info
      .get(&ip)
      .and_then(|info| info.simple_ctor_param_slots.clone());
    if ctor_proto.variadic || (num_args as u16 + 1) != ctor_proto.arity as u16 {
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
      // `call_indirect`: see the note there.
      let instance_val = self.load_reg_mem_at_base(new_base, 0);
      let [a0, a1, a2, a3] = self.load_construct_arg_values(instance_val, func + 1, num_args);
      self.flush_live(self.current_ip);
      self.fb.ins().call_indirect(
        sig,
        prepare,
        &[vm_p, new_base, closure_bits, neg1, a0, a1, a2, a3],
      );
      self.reload_live(self.current_ip);
      self.refresh_regs();
      self.call_checked("zuri_jit_new_finish", &[vm_p, base, dst_i, new_base]);
      self.resync_dst_from_memory(dst);
      self.fb.ins().jump(done_block, &[]);
    } else if let Some(param_slots) = simple_ctor_param_slots {
      let instance_val = if field_count == 2
        && param_slots.len() == 2
        && param_slots[0] == 0
        && param_slots[1] == 1
        && num_args == 2
      {
        let a0 = self.load_reg(func + 1);
        let a1 = self.load_reg(func + 2);
        self.call_helper_raw(
          "zuri_jit_alloc_instance_fast_2",
          &[vm_p, target_class, a0, a1],
        )
      } else if field_count == 1 && param_slots.len() == 1 && param_slots[0] == 0 && num_args == 1 {
        let a0 = self.load_reg(func + 1);
        self.call_helper_raw("zuri_jit_alloc_instance_fast_1", &[vm_p, target_class, a0])
      } else if field_count == 0 && num_args == 0 {
        self.call_helper_raw("zuri_jit_alloc_instance_fast_0", &[vm_p, target_class])
      } else {
        let instance_val = self.call_helper_raw(
          "zuri_jit_alloc_instance_fast",
          &[vm_p, target_class, field_count_v],
        );
        let instance_ptr = self.obj_ptr(instance_val);
        let fields_ptr = if field_count <= object::INLINE_FIELDS as u16 {
          self.fb.ins().iadd_imm_s(
            instance_ptr,
            object::obj_instance_fields_inline_offset() as i64,
          )
        } else {
          self.load_instance_fields_ptr(instance_ptr)
        };
        let trusted = cranelift_codegen::ir::MemFlagsData::trusted();
        for (param, &field_slot) in param_slots.iter().enumerate() {
          if param < num_args as usize {
            let arg_val = self.load_reg(func + 1 + param as u8);
            self
              .fb
              .ins()
              .store(trusted, arg_val, fields_ptr, (field_slot as i32) * 8);
          }
        }
        instance_val
      };
      self.store_reg(dst, instance_val);
      self.fb.ins().jump(done_block, &[]);
    } else {
      // Fully-inline path: see `emit_inline_construct`'s own docs.
      let entry = self.emit_inline_construct(
        ctor_bits,
        proto_ptr as u64,
        ctor_proto.num_registers,
        dst,
        func,
        new_base,
        field_count,
        dynamic_block,
      );
      let fast_block = self.fb.create_block();
      self.fb.ins().jump(fast_block, &[]);

      self.fb.switch_to_block(fast_block);
      let neg1 = self.fb.ins().iconst(types::I32, -1);
      let sig = self.entry_sig_ref();
      let closure_bits = self.u64c(ctor_bits);
      let instance_val = self.load_reg_mem_at_base(new_base, 0);
      let [a0, a1, a2, a3] = self.load_construct_arg_values(instance_val, func + 1, num_args);
      self.flush_live(self.current_ip);
      self.fb.ins().call_indirect(
        sig,
        entry,
        &[vm_p, new_base, closure_bits, neg1, a0, a1, a2, a3],
      );
      self.reload_live(self.current_ip);
      self.refresh_regs();
      self.emit_inline_construct_finish(dst, new_base);
      self.fb.ins().jump(done_block, &[]);
    }

    self.fb.switch_to_block(dynamic_block);
    self.emit_construct_call(dst, func, num_args);
    self.fb.ins().jump(done_block, &[]);

    self.fb.switch_to_block(done_block);
  }

  /// `Instr::Call`'s CONSTRUCTOR shape (`jit::CallTarget::Construct`):
  /// same `prepare` / `call_indirect` / `finish` protocol as
  /// `emit_fast_call`, differing only in that the value delivered to
  /// `dst` is the newly built instance rather than the callee's return
  /// value; hence `zuri_jit_new_finish`, which takes no return-value
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
    // `call_indirect`: see the note there.
    let instance_val = self.load_reg_mem_at_base(new_base, 0);
    let [a0, a1, a2, a3] = self.load_shifted_construct_arg_values(instance_val, new_base, num_args);
    self.flush_live(self.current_ip);
    self.fb.ins().call_indirect(
      sig,
      prepare,
      &[self.vm_param, new_base, closure_bits, neg1, a0, a1, a2, a3],
    );
    self.reload_live(self.current_ip);
    self.refresh_regs();
    // Reuses the operands materialized before the branch rather than
    // re-emitting them here: anything defined in THIS block would not
    // dominate `slow_block` below, and Cranelift's verifier rejects
    // that outright.
    self.call_checked("zuri_jit_new_finish", &[vm_p, base, dst_i, new_base]);
    self.resync_dst_from_memory(dst);
    self.fb.ins().jump(done_block, &[]);

    self.fb.switch_to_block(slow_block);
    self.emit_fast_call(
      "zuri_jit_call_prepare",
      &[vm_p, base, func_i, num_args_i, dst_i],
      new_base,
      dst,
      func + 1,
      num_args,
      "zuri_jit_call",
      &[vm_p, base, func_i, num_args_i, dst_i],
    );
    self.fb.ins().jump(done_block, &[]);

    self.fb.switch_to_block(done_block);
  }

  /// `Instr::Call`'s PROVEN self-recursive fast path (`jit::CallTarget
  /// ::SelfRecursive`, from `VM::resolve_call_targets`'s use of
  /// `escape::self_reference_facts`): no runtime guard at all, since
  /// there is nothing left to misidentify; the callee register's
  /// VALUE is never even read here, only its bytecode INDEX (needed for
  /// `new_base`'s frame-layout math). The callee is this invocation's own
  /// closure, read out of the live frame rather than from the entry
  /// parameter, since a collection can relocate it mid-invocation and
  /// only the frame field gets rewritten when it does; that value is
  /// both passed on and stored into the frame this pushes, so a stale
  /// one would leave the collector tracing a dead pointer. Paired with
  /// a genuine relocation-resolved direct `call` to `own_func_id` --
  /// NOT `call_indirect` on a runtime-loaded pointer; as the actual
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
  /// known); this skips the RESOLVER, not frame setup.
  fn emit_self_call(&mut self, dst: u8, func: u8, num_args: u8) {
    let base = self.base_param;
    let vm_p = self.vm_param;
    let new_base = self.fb.ins().iadd_imm_s(base, func as i64 + 1);
    let num_args_i = self.idx(num_args);
    let dst_i = self.idx(dst);
    let closure_bits = self.emit_current_closure_val();

    // Eligibility for the fully-inline path is a compile-time fact for
    // self-recursion (this function's own `arity`/`variadic`; there is
    // no other callee it could possibly be): decided once here, not as
    // a runtime branch. An ineligible callee (variadic, or this call
    // site's `num_args` happens not to match; both fixed by the
    // instruction, never varying call to call) keeps using the
    // original, unaccelerated `zuri_jit_direct_call_prepare` sequence
    // verbatim.
    if self.proto.variadic || num_args != self.proto.arity {
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

      // No snapshot/restore needed around this split: the ONE helper
      // call above runs UNCONDITIONALLY, before either branch, so it's
      // already flushed whatever was live-and-dirty on BOTH paths --
      // see `restore_dirty_from_snapshot`'s own docs for the bug class
      // this reasoning has to hold up against.
      self.fb.switch_to_block(fast_block);
      let func_ref = self
        .module
        .declare_func_in_func(self.own_func_id, self.fb.func);
      let neg1 = self.fb.ins().iconst(types::I32, -1);
      let [a0, a1, a2, a3] = self.load_call_arg_values(func + 1, num_args);
      self.flush_live(self.current_ip);
      let call = self.fb.ins().call(
        func_ref,
        &[vm_p, new_base, closure_bits, neg1, a0, a1, a2, a3],
      );
      let ret_bits = self.fb.inst_results(call)[0];
      self.reload_live(self.current_ip);
      self.refresh_regs();
      self.call_checked(
        "zuri_jit_call_finish",
        &[vm_p, base, dst_i, new_base, ret_bits],
      );
      self.resync_dst_from_memory(dst);
      self.fb.ins().jump(done_block, &[]);

      self.fb.switch_to_block(slow_block);
      let func_i = self.idx(func);
      self.call_checked("zuri_jit_call", &[vm_p, base, func_i, num_args_i, dst_i]);
      self.resync_dst_from_memory(dst);
      self.fb.ins().jump(done_block, &[]);

      self.fb.switch_to_block(done_block);
      return;
    }

    // Fully-inline path: see `emit_inline_frame_push`'s own docs.
    //
    // Unlike the eligibility-false branch above, `emit_inline_frame_push`
    // itself branches to `slow_block` BEFORE any unconditional helper
    // call (its depth/registers/frame-capacity checks are all inline),
    // so this needs the snapshot/restore discipline `emit_known_call`
    // already uses, for exactly the reason its own docs give: two
    // independent call sites (the inline fast path's own `call`, and
    // `slow_block`'s `zuri_jit_call`) now sit in mutually exclusive
    // branches with no unconditional call ahead of the split to have
    // already flushed both.
    let proto_bits = self.proto as *const ObjFunction as u64;
    let slow_block = self.fb.create_block();
    let fast_block = self.fb.create_block();
    let done_block = self.fb.create_block();

    self.emit_inline_frame_push(
      proto_bits,
      self.proto.num_registers,
      dst,
      new_base,
      closure_bits,
      slow_block,
    );
    self.fb.ins().jump(fast_block, &[]);

    self.fb.switch_to_block(fast_block);
    let func_ref = self
      .module
      .declare_func_in_func(self.own_func_id, self.fb.func);
    let neg1 = self.fb.ins().iconst(types::I32, -1);
    let [a0, a1, a2, a3] = self.load_call_arg_values(func + 1, num_args);
    let is_alloc_free = self.is_self_allocation_free();
    if !is_alloc_free {
      self.flush_live(self.current_ip);
    }
    let call = self.fb.ins().call(
      func_ref,
      &[vm_p, new_base, closure_bits, neg1, a0, a1, a2, a3],
    );
    let ret_bits = self.fb.inst_results(call)[0];
    if !is_alloc_free {
      self.reload_live(self.current_ip);
      self.refresh_regs();
    }
    self.emit_inline_frame_finish(dst, new_base, ret_bits);
    self.fb.ins().jump(done_block, &[]);

    self.fb.switch_to_block(slow_block);
    let func_i = self.idx(func);
    self.call_checked("zuri_jit_call", &[vm_p, base, func_i, num_args_i, dst_i]);
    self.resync_dst_from_memory(dst);
    self.fb.ins().jump(done_block, &[]);

    self.fb.switch_to_block(done_block);
  }

  /// `Instr::Call`'s PROVEN-but-reassignable fast path (`jit::CallTarget
  /// ::Known`, from `VM::resolve_call_targets`'s use of `escape::
  /// global_ref_facts`): the callee register is proven to hold an
  /// UNMODIFIED read of some OTHER global name that, at THIS function's
  /// OWN compile time, already resolved to a different, already-
  /// compiled function; but unlike self-recursion, that global
  /// binding could still be reassigned before this exact call site
  /// actually runs, so a cheap value-identity guard against the
  /// CURRENT register contents comes first. `entry` is baked as a raw
  /// address constant; sound because `CompiledFunction`'s own docs
  /// guarantee compiled code is never unloaded or recompiled once
  /// produced, so this address stays valid for the rest of the process.
  /// On a guard miss (or the same depth-exhausted case `emit_self_call`
  /// handles), falls all the way back to the fully general
  /// `zuri_jit_call` slow helper; exactly `emit_fast_call`'s own slow
  /// path, since a miss here means "let the general path re-resolve
  /// whatever this actually is right now," not "try again with stale
  /// information."
  ///
  /// Guards BEFORE its first helper call (unlike `emit_self_call`), so
  ///; per `restore_dirty_from_snapshot`'s own docs; this needs the
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
  /// arithmetic-only leaf; no call protocol, no frame, no register
  /// flush, no reload. Call overhead otherwise dominates numeric code
  /// where the callee's own work is a handful of flops.
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
  /// value, NOT in `VM::registers`; so nothing a garbage collection
  /// would need to see or relocate is reachable from it. That is only
  /// sound if a collection cannot happen while those values are live,
  /// which `inline_plan` guarantees structurally rather than hopefully:
  /// the body is proven to contain no call, no allocation, and no
  /// helper that could do either. Note this is NOT implied by the
  /// opcode whitelist alone; `a + b` on non-numeric operands calls
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
  /// ever read HERE, at compile time; generated code never touches
  /// it; so a later reassignment cannot leave it dangling behind.
  fn try_emit_inlined_call(
    &mut self,
    ip: usize,
    dst: u8,
    func: u8,
    num_args: u8,
    guard_bits: u64,
    proto_ptr: usize,
  ) -> bool {
    // SAFETY: see this function's own docs; `proto_ptr` names a live,
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

    let slow_block = self.fb.create_block();
    let done_block = self.fb.create_block();
    self.emit_callee_proto_guard(callee_val, guard_bits, slow_block);

    let result = self.emit_inlined_body(ip, callee, &plan, func);
    self.store_reg(dst, result);
    self.fb.ins().jump(done_block, &[]);

    self.fb.switch_to_block(slow_block);
    self.emit_safepoint();
    self.emit_generic_call_cold(dst, func, num_args);
    // The two arms disagree about `dst` the same way every other
    // guarded instruction's do; the inlined arm defines it in its
    // `Variable` and writes no memory, the call arm writes memory and
    // stale-marks it. See `resync_receiver_from_memory` for the bug
    // that leaving that disagreement in place produces.
    self.resync_dst_from_memory(dst);
    self.fb.ins().jump(done_block, &[]);

    self.fb.switch_to_block(done_block);
    true
  }

  /// Decides whether `callee` can be inlined at this site, returning the
  /// prefix of its bytecode to emit (everything up to and including its
  /// first `Return`) when it can.
  ///
  /// Every rule here exists to hold up one of the two guarantees
  /// `try_emit_inlined_call` depends on; "cannot allocate or call"
  /// and "is straight-line"; so none of them is merely conservative
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
  ) -> Option<Vec<(usize, Instr)>> {
    if callee.variadic || callee.arity != num_args || !callee.upvalues.is_empty() {
      if crate::jit::log_enabled() {
        eprintln!(
          "[jit] inline '{}' rejected: variadic/arity/upvalues (arity={}, num_args={}, upval={})",
          callee.name,
          callee.arity,
          num_args,
          callee.upvalues.len()
        );
      }
      return None;
    }
    // The callee's registers are addressed as caller registers
    // `func + 1 + r` by the ordinary calling convention; an inlined body
    // never materializes them, but the argument mapping below still has
    // to stay inside a `u8`.
    if (func as usize) + 1 + (callee.num_registers as usize) > u8::MAX as usize {
      if crate::jit::log_enabled() {
        eprintln!("[jit] inline '{}' rejected: reg limit", callee.name);
      }
      return None;
    }
    let code = &callee.chunk.code;
    if code.len() > Self::MAX_INLINE_OPS {
      if crate::jit::log_enabled() {
        eprintln!(
          "[jit] inline '{}' rejected: code len {}",
          callee.name,
          code.len()
        );
      }
      return None;
    }

    let mut numeric = vec![false; callee.num_registers as usize];
    for i in 0..num_args {
      // A parameter is numeric exactly when the caller already proved
      // the argument it is passed is.
      if !self.proven_numeric(ip, func + 1 + i) {
        if crate::jit::log_enabled() {
          eprintln!(
            "[jit] inline '{}' rejected: arg {} (reg {}) not proven numeric at ip {}",
            callee.name,
            i,
            func + 1 + i,
            ip
          );
        }
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
          // `CheckParamType` instructions are dropped from the plan
          // rather than disqualifying it: `emit_inlined_body` has no
          // arm for them (an inlined body is straight-line arithmetic
          // only), but a parameter register already proven numeric by
          // the CALL SITE's own `proven_numeric` check (required above,
          // before this loop even starts) makes a `number`/`int`-only,
          // non-nullable check on that same register a guaranteed
          // no-op: see the `CheckParamType` arm below for the actual
          // proof. Silently keeping it in the plan instead would panic
          // in `emit_inlined_body`; silently disqualifying the whole
          // callee instead (the ORIGINAL behavior here, before typed
          // parameters existed) would mean every typed small leaf
          // function permanently loses inlining; confirmed to cost
          // 2x on `spectral-norm.zu`'s `eval_A`, called ~600M times.
          return Some(
            code[..=i]
              .iter()
              .copied()
              .enumerate()
              .filter(|(_, instr)| !matches!(instr, Instr::CheckParamType { .. }))
              .collect(),
          );
        },
        // A parameter register the CALL SITE already proved numeric
        // (required for every one of `num_args` above) trivially
        // satisfies its own `number`/`int`-only, non-nullable check --
        // that's exactly what `proven_numeric` means. Anything the
        // check ALSO covers beyond that (a wider union, `?nullable`, a
        // non-numeric type on a register the call site did NOT already
        // prove numeric) can't be soundly assumed here, so it
        // disqualifies inlining instead of risking silently skipping a
        // check that could have raised.
        Instr::CheckParamType { reg, check_idx } => {
          let check = &callee.chunk.param_checks[check_idx as usize];
          let numeric_only = !check.nullable
            && check
              .types
              .iter()
              .all(|t| matches!(t, ParamType::Number | ParamType::Int));
          if !numeric_only || !is_num(&numeric, reg) {
            return None;
          }
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
        Instr::Mod { dst, a, b }
        | Instr::BitAnd { dst, a, b }
        | Instr::BitOr { dst, a, b }
        | Instr::BitXor { dst, a, b }
        | Instr::BitShl { dst, a, b }
        | Instr::BitShr { dst, a, b }
        | Instr::BitUshr { dst, a, b } => {
          if !is_num(&numeric, a) || !is_num(&numeric, b) {
            return None;
          }
          *numeric.get_mut(dst as usize)? = true;
        },
        Instr::BitNot { dst, src } => {
          if !is_num(&numeric, src) {
            return None;
          }
          *numeric.get_mut(dst as usize)? = true;
        },
        Instr::GetGlobal { dst, .. } => {
          if callee
            .jit
            .global_slot_cache
            .get(i)
            .map(|c| c.get())
            .unwrap_or(-1)
            < 0
          {
            return None;
          }
          *numeric.get_mut(dst as usize)? = true;
        },
        Instr::SetGlobal { src, .. } | Instr::AssignGlobal { src, .. } => {
          if callee
            .jit
            .global_slot_cache
            .get(i)
            .map(|c| c.get())
            .unwrap_or(-1)
            < 0
            || !is_num(&numeric, src)
          {
            return None;
          }
        },
        _ => {
          if crate::jit::log_enabled() {
            eprintln!(
              "[jit] inline '{}' rejected: unsupported op at {} {:?}",
              callee.name, i, instr
            );
          }
          return None;
        },
      }
    }
    if crate::jit::log_enabled() {
      eprintln!("[jit] inline '{}' rejected: no Return found", callee.name);
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
  fn emit_inlined_body(
    &mut self,
    ip: usize,
    callee: &ObjFunction,
    plan: &[(usize, Instr)],
    func: u8,
  ) -> IrValue {
    let mut regs: Vec<IrValue> = Vec::with_capacity(callee.num_registers as usize);
    let zero = self.fb.ins().f64const(0.0);
    regs.resize(callee.num_registers as usize, zero);

    // Which callee registers hold a value already known to be an
    // integer in `i64` range. Seeded from the CALLER's own
    // `typeflow::IntFacts` for the argument registers, which is the
    // same evidence the out-of-line `Instr::Mod` trusts via
    // `both_proven_int`. Deliberately not propagated through
    // arithmetic: a sum or product of two in-range integers can leave
    // `i64` range, and the range half of the guarantee is exactly what
    // `Instr::Mod`'s round-trip check is there to establish.
    let mut integral = vec![false; callee.num_registers as usize];
    // A stronger property than `integral` for the DIVISOR specifically:
    // a known-positive integer constant makes the divisor guard
    // statically true, which removes the branch rather than just
    // predicting it well. `% 0` and `i64::MIN % -1` are the two cases
    // that guard exists for, and a positive constant rules out both.
    let mut pos_const = vec![false; callee.num_registers as usize];
    // The caller already maintains an `i64` view of any register it
    // tracks as an int (`store_reg_int` defines `reg_vars_int` at every
    // write when `int_tracked` says so), which is the same thing the
    // out-of-line `Instr::Mod` reaches for. Borrowing it here lets an
    // integer op inside the inlined body work on the integer directly
    // instead of round-tripping the f64 through a SATURATING
    // conversion, which is several instructions, not one.
    //
    // Only values that arrive from the caller (or are integral
    // constants, or copies of either) get a view: anything computed by
    // the body itself is f64 arithmetic, and maintaining a parallel
    // integer result for it would cost more than the conversion saves.
    let mut int_view: Vec<Option<IrValue>> = vec![None; callee.num_registers as usize];
    let is_pos_int_const = |n: f64| n.fract() == 0.0 && n > 0.0 && n < 9223372036854775808.0;
    for i in 0..callee.arity {
      let src = func + 1 + i;
      let v = self.load_reg(src);
      regs[i as usize] = self.to_f64(v);
      integral[i as usize] = self.proven_int(ip, src);
      if integral[i as usize] {
        int_view[i as usize] = Some(self.fb.use_var(self.reg_vars_int[src as usize]));
      }
      if let Some(n) = self.const_facts.const_value(ip, src)
        && is_pos_int_const(n)
      {
        integral[i as usize] = true;
        if int_view[i as usize].is_none() {
          int_view[i as usize] = Some(self.i64c(n as i64));
        }
        pos_const[i as usize] = true;
      }
    }

    let bake = |fc: &mut Self, idx: u16| -> IrValue {
      let v = callee.chunk.constants[idx as usize];
      fc.fb.ins().f64const(v.as_number())
    };

    for &(orig_ip, instr) in plan {
      match instr {
        Instr::Return { src } => {
          let f = regs[src as usize];
          return self.from_f64(f);
        },
        Instr::LoadConst { dst, const_idx } => {
          let n = callee.chunk.constants[const_idx as usize].as_number();
          integral[dst as usize] = n.fract() == 0.0 && n.abs() < 9223372036854775808.0;
          pos_const[dst as usize] = is_pos_int_const(n);
          int_view[dst as usize] = if integral[dst as usize] {
            Some(self.i64c(n as i64))
          } else {
            None
          };
          regs[dst as usize] = bake(self, const_idx)
        },
        Instr::Move { dst, src } => {
          integral[dst as usize] = integral[src as usize];
          pos_const[dst as usize] = pos_const[src as usize];
          int_view[dst as usize] = int_view[src as usize];
          regs[dst as usize] = regs[src as usize]
        },
        Instr::Add { dst, a, b } => {
          regs[dst as usize] = self.fb.ins().fadd(regs[a as usize], regs[b as usize])
        },
        Instr::Sub { dst, a, b } => {
          regs[dst as usize] = self.fb.ins().fsub(regs[a as usize], regs[b as usize])
        },
        Instr::Mul { dst, a, b } => {
          regs[dst as usize] = self.fb.ins().fmul(regs[a as usize], regs[b as usize])
        },
        Instr::Div { dst, a, b } => {
          regs[dst as usize] = self.fb.ins().fdiv(regs[a as usize], regs[b as usize])
        },
        Instr::AddImm { dst, a, imm_const } => {
          let b = bake(self, imm_const);
          regs[dst as usize] = self.fb.ins().fadd(regs[a as usize], b)
        },
        Instr::SubImm { dst, a, imm_const } => {
          let b = bake(self, imm_const);
          regs[dst as usize] = self.fb.ins().fsub(regs[a as usize], b)
        },
        Instr::MulImm { dst, a, imm_const } => {
          let b = bake(self, imm_const);
          regs[dst as usize] = self.fb.ins().fmul(regs[a as usize], b)
        },
        // Bitwise operands narrow with a SATURATING conversion, which
        // is what `bitwise_numeric`'s `as i64` does; the result always
        // fits `i64`, so it also seeds the integer view.
        Instr::BitAnd { dst, a, b }
        | Instr::BitOr { dst, a, b }
        | Instr::BitXor { dst, a, b }
        | Instr::BitShl { dst, a, b }
        | Instr::BitShr { dst, a, b }
        | Instr::BitUshr { dst, a, b } => {
          let ia = match int_view[a as usize] {
            Some(v) => v,
            None => self.emit_narrow_to_i64(regs[a as usize]),
          };
          let ib = match int_view[b as usize] {
            Some(v) => v,
            None => self.emit_narrow_to_i64(regs[b as usize]),
          };
          let ir = match instr {
            Instr::BitAnd { .. } => self.fb.ins().band(ia, ib),
            Instr::BitOr { .. } => self.fb.ins().bor(ia, ib),
            Instr::BitXor { .. } => self.fb.ins().bxor(ia, ib),
            Instr::BitShl { .. } => Self::shift_left(self.fb, ia, ib),
            Instr::BitShr { .. } => Self::shift_right(self.fb, ia, ib),
            _ => Self::shift_right_unsigned(self.fb, ia, ib),
          };
          integral[dst as usize] = true;
          pos_const[dst as usize] = false;
          int_view[dst as usize] = Some(ir);
          regs[dst as usize] = self.fb.ins().fcvt_from_sint(types::F64, ir);
        },
        Instr::BitNot { dst, src } => {
          let is = match int_view[src as usize] {
            Some(v) => v,
            None => self.emit_narrow_to_i64(regs[src as usize]),
          };
          let inv = self.fb.ins().bnot(is);
          integral[dst as usize] = true;
          pos_const[dst as usize] = false;
          int_view[dst as usize] = Some(inv);
          regs[dst as usize] = self.fb.ins().fcvt_from_sint(types::F64, inv);
        },
        Instr::Neg { dst, src } => {
          integral[dst as usize] = false;
          pos_const[dst as usize] = false;
          int_view[dst as usize] = None;
          regs[dst as usize] = self.fb.ins().fneg(regs[src as usize]);
        },
        Instr::Mod { dst, a, b } => {
          // Same shape as the out-of-line `Instr::Mod`, which this used
          // to skip entirely: it fed both operands straight to `srem`
          // with no guards, so a fractional operand silently lost its
          // fraction, a negative divisor came back mangled, and `x % 0`
          // took the process down with SIGFPE where the interpreter
          // answers NaN. The integer path is only entered when both
          // operands are integral and the divisor is positive; a
          // positive divisor also rules out the `i64::MIN % -1`
          // overflow trap.
          let fa = regs[a as usize];
          let fb_ = regs[b as usize];
          let ia = match int_view[a as usize] {
            Some(v) => v,
            None => self.fb.ins().fcvt_to_sint_sat(types::I64, fa),
          };
          let ib = match int_view[b as usize] {
            Some(v) => v,
            None => self.fb.ins().fcvt_to_sint_sat(types::I64, fb_),
          };

          // Nothing left to test: an integral dividend and a positive
          // constant divisor satisfy the guard outright, so this drops
          // to the bare divide with no branch and no merge at all.
          if integral[a as usize] && pos_const[b as usize] {
            let rem = self.fb.ins().srem(ia, ib);
            let rem_f = self.fb.ins().fcvt_from_sint(types::F64, rem);
            integral[dst as usize] = true;
            int_view[dst as usize] = Some(rem);
            regs[dst as usize] = self.fb.ins().fcopysign(rem_f, fa);
            continue;
          }

          let zero = self.i64c(0);
          let is_pos_denom = self.fb.ins().icmp(IntCC::SignedGreaterThan, ib, zero);
          // The round-trip test proves BOTH integrality and `i64` range
          // (the conversion saturates), so it is only skippable for an
          // operand the caller already proved an int. A literal divisor
          // then folds `is_pos_denom` away too, leaving the bare `srem`.
          let mut can_fast = is_pos_denom;
          if !integral[a as usize] {
            let fa_rt = self.fb.ins().fcvt_from_sint(types::F64, ia);
            let is_int_a =
              self
                .fb
                .ins()
                .fcmp(cranelift_codegen::ir::condcodes::FloatCC::Equal, fa, fa_rt);
            can_fast = self.fb.ins().band(can_fast, is_int_a);
          }
          if !integral[b as usize] {
            let fb_rt = self.fb.ins().fcvt_from_sint(types::F64, ib);
            let is_int_b =
              self
                .fb
                .ins()
                .fcmp(cranelift_codegen::ir::condcodes::FloatCC::Equal, fb_, fb_rt);
            can_fast = self.fb.ins().band(can_fast, is_int_b);
          }

          let rem_block = self.fb.create_block();
          let fmod_block = self.fb.create_block();
          let join_block = self.fb.create_block();
          self.fb.append_block_param(join_block, types::F64);
          self
            .fb
            .ins()
            .brif(can_fast, rem_block, &[], fmod_block, &[]);

          self.fb.switch_to_block(rem_block);
          let rem = self.fb.ins().srem(ia, ib);
          let rem_f = self.fb.ins().fcvt_from_sint(types::F64, rem);
          let signed = self.fb.ins().fcopysign(rem_f, fa);
          self.fb.ins().jump(join_block, &[signed.into()]);

          self.fb.switch_to_block(fmod_block);
          let fallback = self.call_f64_intrinsic("zuri_jit_num_fmod", fa, fb_);
          self.fb.ins().jump(join_block, &[fallback.into()]);

          self.fb.switch_to_block(join_block);
          integral[dst as usize] = integral[a as usize] && integral[b as usize];
          // The merge can carry an `fmod` result, so there is no integer
          // value that is valid on both edges.
          int_view[dst as usize] = None;
          regs[dst as usize] = self.fb.block_params(join_block)[0];
        },
        Instr::GetGlobal { dst, .. } => {
          let slot = callee.jit.global_slot_cache[orig_ip].get();
          let flags = cranelift_codegen::ir::MemFlagsData::trusted();
          let slots_ptr = self.fb.ins().load(
            types::I64,
            flags,
            self.vm_param,
            GLOBAL_SLOTS_PTR_CACHE_OFFSET,
          );
          let byte_off = (slot * 8) as i32;
          let raw_val = self.fb.ins().load(types::I64, flags, slots_ptr, byte_off);
          regs[dst as usize] = self.to_f64(raw_val);
        },
        Instr::SetGlobal { src, .. } | Instr::AssignGlobal { src, .. } => {
          let slot = callee.jit.global_slot_cache[orig_ip].get();
          let flags = cranelift_codegen::ir::MemFlagsData::trusted();
          let slots_ptr = self.fb.ins().load(
            types::I64,
            flags,
            self.vm_param,
            GLOBAL_SLOTS_PTR_CACHE_OFFSET,
          );
          let byte_off = (slot * 8) as i32;
          let raw_val = self.from_f64(regs[src as usize]);
          self.fb.ins().store(flags, raw_val, slots_ptr, byte_off);
        },
        _ => unreachable!("inline_plan admitted a non-inlinable instruction"),
      }
    }
    unreachable!("inline_plan always ends its plan with a Return")
  }

  /// One unguarded floating-point operation on two operands already
  /// proven numeric; the inlined-body counterpart of
  /// `emit_binary_numeric_proven`, differing only in that it threads
  /// SSA values instead of bytecode registers.

  /// Branches to `fail_block` unless the callee register holds the
  /// builtin native whose `NativeFn` is `guard_fn`, leaving the success
  /// path as the current block.
  ///
  /// Guards the FUNCTION POINTER, not the object's address: see
  /// `CallTarget::KnownNative::guard_fn` for why an address guard would
  /// silently rot. Three separate branches for the same reason
  /// `emit_ic_guard` needs them: each stage may only be evaluated once
  /// the previous proved it safe to dereference.
  fn emit_callee_native_guard(&mut self, callee_val: IrValue, guard_fn: u64, fail_block: Block) {
    let is_obj = self.is_obj(callee_val);
    let obj_block = self.fb.create_block();
    self.fb.ins().brif(is_obj, obj_block, &[], fail_block, &[]);

    self.fb.switch_to_block(obj_block);
    let ptr = self.obj_ptr(callee_val);
    let tag = self.obj_tag(ptr);
    let tag_native = self.i64c(object::OBJ_TAG_NATIVE as i64);
    let is_native = self.fb.ins().icmp(IntCC::Equal, tag, tag_native);
    let fn_block = self.fb.create_block();
    self
      .fb
      .ins()
      .brif(is_native, fn_block, &[], fail_block, &[]);

    self.fb.switch_to_block(fn_block);
    let off = object::obj_native_func_offset() as i32;
    let actual = self.fb.ins().load(
      types::I64,
      cranelift_codegen::ir::MemFlagsData::trusted(),
      ptr,
      off,
    );
    let want = self.u64c(guard_fn);
    let is_hit = self.fb.ins().icmp(IntCC::Equal, actual, want);
    let hit_block = self.fb.create_block();
    self.fb.ins().brif(is_hit, hit_block, &[], fail_block, &[]);
    self.fb.switch_to_block(hit_block);
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
  /// PROTOTYPE and not the closure; guarding the closure's own
  /// address silently stops matching the first time a minor collection
  /// relocates it.
  fn emit_callee_proto_guard(&mut self, callee_val: IrValue, guard_bits: u64, fail_block: Block) {
    let is_obj = self.is_obj(callee_val);
    let ptr = self.obj_ptr(callee_val);
    let flags = cranelift_codegen::ir::MemFlagsData::trusted();
    let func_off = object::obj_closure_function_offset() as i32;
    let proto = self.fb.ins().load(types::I64, flags, ptr, func_off);
    let want = self.u64c(guard_bits);
    let same_proto = self.fb.ins().icmp(IntCC::Equal, proto, want);
    let is_hit = self.fb.ins().band(is_obj, same_proto);
    let hit_block = self.fb.create_block();
    self.fb.ins().brif(is_hit, hit_block, &[], fail_block, &[]);
    self.fb.switch_to_block(hit_block);
  }

  /// A builtin native (`vm::natives`) whose whole body is a `Value` tag
  /// test, emitted inline instead of called.
  ///
  /// These are the cheapest functions in the language and, until now,
  /// among the most expensive to call: a measured ~44ns of resolution,
  /// dispatch and argument marshalling around what is one or two
  /// machine instructions of actual work.
  ///
  /// Every variant reproduces its `vm::natives` counterpart exactly --
  /// the same `Value` predicate, not an equivalent-looking one; so a
  /// compiled answer is always the interpreted answer. Natives whose
  /// body is more than a tag test (`typeof`, `id`, `is_iterable`,
  /// `print`, ...) are deliberately absent; they still get
  /// `KnownNative`'s resolution-free call, just not an inline body.
  fn native_intrinsic(name: &str) -> Option<NativeIntrinsic> {
    use NativeIntrinsic::*;
    Some(match name {
      "is_number" => IsNumber,
      "is_bool" => IsBool,
      "is_object" => IsObject,
      "is_int" => IsInt,
      "is_string" => Tag(&[object::OBJ_TAG_STR]),
      "is_list" => Tag(&[object::OBJ_TAG_LIST]),
      "is_dict" => Tag(&[object::OBJ_TAG_DICT]),
      "is_bytes" => Tag(&[object::OBJ_TAG_BYTES]),
      "is_class" => Tag(&[object::OBJ_TAG_CLASS]),
      "is_instance" => Tag(&[object::OBJ_TAG_INSTANCE]),
      "is_file" => Tag(&[object::OBJ_TAG_FILE]),
      // `natives::is_function` is closure/native/bound-method but NOT
      // class; `is_callable` is those three PLUS class. Keeping them
      // as distinct tag sets is what preserves that difference.
      "is_function" => Tag(&[
        object::OBJ_TAG_CLOSURE,
        object::OBJ_TAG_NATIVE,
        object::OBJ_TAG_BOUND_METHOD,
      ]),
      "is_callable" => Tag(&[
        object::OBJ_TAG_CLOSURE,
        object::OBJ_TAG_NATIVE,
        object::OBJ_TAG_BOUND_METHOD,
        object::OBJ_TAG_CLASS,
      ]),
      _ => return None,
    })
  }

  /// `Instr::Call` on a call site proven to target a specific builtin
  /// native: see `CallTarget::KnownNative`.
  ///
  /// Guards that the callee register still holds that exact native (a
  /// global binding is reassignable, so this is a real check, not a
  /// formality) and then either emits the native's body inline or calls
  /// it directly, skipping resolution and dispatch entirely. A failed
  /// guard falls back to the ordinary resolver, which handles whatever
  /// is actually there.
  ///
  /// No safepoint on the intrinsic path: a tag test cannot allocate, and
  /// the enclosing loop's back edge still carries one.
  fn emit_known_native_call(
    &mut self,
    dst: u8,
    func: u8,
    num_args: u8,
    guard_fn: u64,
    native_ptr: usize,
  ) {
    // SAFETY: `native_ptr` is only ever dereferenced HERE, during this
    // compilation, to read the native's name. `VM::resolve_call_targets`
    // produced it moments ago on this same thread and the native is
    // permanently reachable from a global, so it is live for this read.
    // It is deliberately NOT baked into generated code: natives are
    // young allocations (see `Heap::alloc_native`) and relocate on
    // promotion, so a baked address would dangle at runtime.
    let native = unsafe { &*(native_ptr as *const object::NativeFunction) };
    let intrinsic = (num_args == 1)
      .then(|| Self::native_intrinsic(native.name))
      .flatten();

    let callee_val = self.load_reg(func);

    let slow_block = self.fb.create_block();
    let done_block = self.fb.create_block();
    self.emit_callee_native_guard(callee_val, guard_fn, slow_block);

    match intrinsic {
      Some(op) => {
        let arg = self.load_reg(func + 1);
        let v = self.emit_native_intrinsic(op, arg);
        self.store_reg(dst, v);
      },
      None => {
        self.emit_safepoint();
        let base = self.base_param;
        let vm_p = self.vm_param;
        let func_i = self.idx(func);
        let num_args_i = self.idx(num_args);
        let dst_i = self.idx(dst);
        self.call_checked(
          "zuri_jit_call_native",
          &[vm_p, base, func_i, num_args_i, dst_i],
        );
        self.resync_dst_from_memory(dst);
      },
    }
    self.fb.ins().jump(done_block, &[]);

    self.fb.switch_to_block(slow_block);
    self.emit_safepoint();
    self.emit_generic_call_cold(dst, func, num_args);
    self.resync_dst_from_memory(dst);
    self.fb.ins().jump(done_block, &[]);

    self.fb.switch_to_block(done_block);
  }

  /// The intrinsic body itself, as a `Value`-bits result.
  fn emit_native_intrinsic(&mut self, op: NativeIntrinsic, v: IrValue) -> IrValue {
    match op {
      NativeIntrinsic::IsNumber => {
        let c = self.is_number(v);
        self.bool_value(c)
      },
      NativeIntrinsic::IsObject => {
        let c = self.is_obj(v);
        self.bool_value(c)
      },
      NativeIntrinsic::IsBool => {
        let c = self.emit_is_bool_test(v);
        self.bool_value(c)
      },
      NativeIntrinsic::IsInt => {
        let c = self.emit_is_int_test(v);
        self.bool_value(c)
      },
      NativeIntrinsic::Tag(tags) => {
        let c = self.emit_obj_tag_test(v, tags);
        self.bool_value(c)
      },
    }
  }

  /// `Value::is_bool`'s exact match against the two singleton bit
  /// patterns, not a masked test; raw `i8` `0`/`1`, not a boxed
  /// `Value`. Shared by `emit_native_intrinsic`'s `IsBool` and
  /// `emit_check_param_type`'s inline `ParamType::Bool` case, so the
  /// two can't silently disagree on what "a bool" means.
  fn emit_is_bool_test(&mut self, v: IrValue) -> IrValue {
    let t = self.u64c(value::TRUE_VAL);
    let f = self.u64c(value::FALSE_VAL);
    let is_t = self.fb.ins().icmp(IntCC::Equal, v, t);
    let is_f = self.fb.ins().icmp(IntCC::Equal, v, f);
    self.fb.ins().bor(is_t, is_f)
  }

  /// `natives::is_int` is `is_number() && fract() == 0.0`, and Rust's
  /// `f64::fract` is `self - self.trunc()`; so an infinity yields NaN
  /// here and correctly compares unequal, exactly as the interpreted
  /// version does. Raw `i8` `0`/`1`: see `emit_is_bool_test`'s own docs
  /// on why this is split out from `emit_native_intrinsic`.
  fn emit_is_int_test(&mut self, v: IrValue) -> IrValue {
    let is_num = self.is_number(v);
    let num_block = self.fb.create_block();
    let done_block = self.fb.create_block();
    self.fb.append_block_param(done_block, types::I8);
    let false_v = self.fb.ins().iconst(types::I8, 0);
    self
      .fb
      .ins()
      .brif(is_num, num_block, &[], done_block, &[false_v.into()]);

    self.fb.switch_to_block(num_block);
    let f = self.to_f64(v);
    let t = self.fb.ins().trunc(f);
    let frac = self.fb.ins().fsub(f, t);
    let zero = self.fb.ins().f64const(0.0);
    let is_whole = self.fb.ins().fcmp(FloatCC::Equal, frac, zero);
    self.fb.ins().jump(done_block, &[is_whole.into()]);

    self.fb.switch_to_block(done_block);
    self.fb.block_params(done_block)[0]
  }

  /// Two stages, never fused: the tag lives behind a pointer, so it may
  /// only be read once `is_obj` has proven there is one: see
  /// `emit_ic_guard` for the same discipline. Raw `i8` `0`/`1`: see
  /// `emit_is_bool_test`'s own docs on why this is split out from
  /// `emit_native_intrinsic`.
  fn emit_obj_tag_test(&mut self, v: IrValue, tags: &[u8]) -> IrValue {
    let is_obj = self.is_obj(v);
    let obj_block = self.fb.create_block();
    let done_block = self.fb.create_block();
    self.fb.append_block_param(done_block, types::I8);
    let false_v = self.fb.ins().iconst(types::I8, 0);
    self
      .fb
      .ins()
      .brif(is_obj, obj_block, &[], done_block, &[false_v.into()]);

    self.fb.switch_to_block(obj_block);
    let ptr = self.obj_ptr(v);
    let tag = self.obj_tag(ptr);
    let mut matched = None;
    for &want in tags {
      let w = self.i64c(want as i64);
      let eq = self.fb.ins().icmp(IntCC::Equal, tag, w);
      matched = Some(match matched {
        None => eq,
        Some(prev) => self.fb.ins().bor(prev, eq),
      });
    }
    let matched = matched.expect("emit_obj_tag_test always names at least one tag");
    self.fb.ins().jump(done_block, &[matched.into()]);

    self.fb.switch_to_block(done_block);
    self.fb.block_params(done_block)[0]
  }

  /// Exact-class fast test for `Instr::CheckParamType`'s `Instance`
  /// case, when `param_class_bits` resolved the declared class
  /// statically: `is_obj` + tag==`INSTANCE` + `class_bits==target`, all
  /// in one raw `i8` boolean. A genuine SUBCLASS of `target` fails this
  /// (an exact-bits compare can't see inheritance) and correctly falls
  /// through to the helper's own subclass-aware walk: see
  /// `param_field_slots`' own docs on why the common exact-match case
  /// being this cheap is what makes an object-typed parameter's check
  /// worth inlining at all, instead of paying a full opaque helper call
  /// on every invocation regardless of how the value turns out.
  fn emit_instance_class_test(&mut self, v: IrValue, target_bits: u64) -> IrValue {
    let is_obj = self.is_obj(v);
    let obj_block = self.fb.create_block();
    let done_block = self.fb.create_block();
    self.fb.append_block_param(done_block, types::I8);
    let false_v = self.fb.ins().iconst(types::I8, 0);
    self
      .fb
      .ins()
      .brif(is_obj, obj_block, &[], done_block, &[false_v.into()]);

    self.fb.switch_to_block(obj_block);
    let ptr = self.obj_ptr(v);
    let tag = self.obj_tag(ptr);
    let tag_instance = self.i64c(object::OBJ_TAG_INSTANCE as i64);
    let is_instance = self.fb.ins().icmp(IntCC::Equal, tag, tag_instance);
    let class_block = self.fb.create_block();
    self
      .fb
      .ins()
      .brif(is_instance, class_block, &[], done_block, &[false_v.into()]);

    self.fb.switch_to_block(class_block);
    let flags = cranelift_codegen::ir::MemFlagsData::trusted();
    let class_off = object::obj_instance_class_offset() as i32;
    let class_bits = self.fb.ins().load(types::I64, flags, ptr, class_off);
    let target = self.u64c(target_bits);
    let same_class = self.fb.ins().icmp(IntCC::Equal, class_bits, target);
    self.fb.ins().jump(done_block, &[same_class.into()]);

    self.fb.switch_to_block(done_block);
    self.fb.block_params(done_block)[0]
  }

  /// `Instr::CheckParamType`; inlines every member of the declared
  /// union that's a plain `Value`/`Obj` tag test (everything except
  /// `Iterable`, a method-table probe, and `Instance`, a possibly-
  /// failing global lookup plus a superclass walk), OR'd together with
  /// the same `emit_is_bool_test`/`emit_is_int_test`/`emit_obj_tag_test`
  /// `emit_native_intrinsic` itself uses for the `is_*` builtins; so a
  /// typed parameter and an explicit `is_list(x)` check compile to
  /// identical machine code, not two independently-maintained ideas of
  /// what "a list" means.
  ///
  /// A hit skips the helper entirely; a miss (or a union containing
  /// ONLY `Iterable`/`Instance`, which never sets `cond` at all) falls
  /// back to `zuri_jit_check_param_type`, which re-examines the value
  /// against every member (including the ones already ruled out here --
  /// redundant work only on the raise-or-nullable-nil path, never on
  /// the common hit path this function exists to make cheap).
  fn emit_check_param_type(&mut self, ip: usize, reg: u8, check_idx: u16) {
    let check = &self.proto.chunk.param_checks[check_idx as usize];
    let nullable = check.nullable;
    let types: Vec<ParamType> = check.types.clone();

    let v = self.load_reg(reg);

    let done_block = self.fb.create_block();
    let slow_block = self.fb.create_block();

    if nullable {
      let nil_val = self.u64c(value::NIL_VAL);
      let is_nil = self.fb.ins().icmp(IntCC::Equal, v, nil_val);
      let after_nil = self.fb.create_block();
      self.fb.ins().brif(is_nil, done_block, &[], after_nil, &[]);
      self.fb.switch_to_block(after_nil);
    }

    let mut cond: Option<IrValue> = None;
    let mut obj_tags: Vec<u8> = Vec::new();
    let or_in = |fc: &mut Self, cond: &mut Option<IrValue>, c: IrValue| {
      *cond = Some(match *cond {
        None => c,
        Some(prev) => fc.fb.ins().bor(prev, c),
      });
    };

    for t in types {
      match t {
        ParamType::Bool => {
          let c = self.emit_is_bool_test(v);
          or_in(self, &mut cond, c);
        },
        ParamType::Number => {
          let c = self.is_number(v);
          or_in(self, &mut cond, c);
        },
        ParamType::Int => {
          let c = self.emit_is_int_test(v);
          or_in(self, &mut cond, c);
        },
        ParamType::BigInt => obj_tags.push(object::OBJ_TAG_BIGINT),
        ParamType::String => obj_tags.push(object::OBJ_TAG_STR),
        ParamType::Bytes => obj_tags.push(object::OBJ_TAG_BYTES),
        ParamType::List => obj_tags.push(object::OBJ_TAG_LIST),
        ParamType::Dict => obj_tags.push(object::OBJ_TAG_DICT),
        ParamType::Range => obj_tags.push(object::OBJ_TAG_RANGE),
        ParamType::File => obj_tags.push(object::OBJ_TAG_FILE),
        ParamType::Function => obj_tags.extend_from_slice(&[
          object::OBJ_TAG_CLOSURE,
          object::OBJ_TAG_NATIVE,
          object::OBJ_TAG_BOUND_METHOD,
        ]),
        ParamType::Class => obj_tags.push(object::OBJ_TAG_CLASS),
        ParamType::Callable => obj_tags.extend_from_slice(&[
          object::OBJ_TAG_CLOSURE,
          object::OBJ_TAG_NATIVE,
          object::OBJ_TAG_BOUND_METHOD,
          object::OBJ_TAG_CLASS,
        ]),
        // Inlined only when `param_field_slots` resolved the declared
        // class statically: see `emit_instance_class_test`'s own
        // docs. Otherwise falls to the helper, same as `Iterable`
        // always does.
        ParamType::Instance(_) => {
          if let Some(target_bits) = self.param_class_bits(reg) {
            let c = self.emit_instance_class_test(v, target_bits);
            or_in(self, &mut cond, c);
          }
        },
        ParamType::Iterable => {},
      }
    }

    if !obj_tags.is_empty() {
      let c = self.emit_obj_tag_test(v, &obj_tags);
      or_in(self, &mut cond, c);
    }

    match cond {
      Some(c) => {
        self.fb.ins().brif(c, done_block, &[], slow_block, &[]);
      },
      None => {
        self.fb.ins().jump(slow_block, &[]);
      },
    }

    self.fb.switch_to_block(slow_block);
    let base = self.base_param;
    let reg_i = self.idx(reg);
    let func_ptr = self.func_ptr_const();
    let check_idx_c = self.u64c(check_idx as u64);
    let ip_c = self.u64c(ip as u64);
    self.call_checked(
      "zuri_jit_check_param_type",
      &[self.vm_param, base, reg_i, func_ptr, check_idx_c, ip_c],
    );
    self.fb.ins().jump(done_block, &[]);

    self.fb.switch_to_block(done_block);
    if check.types == [ParamType::Int] && !nullable {
      let f = self.to_f64(v);
      let iv = self.fb.ins().fcvt_to_sint_sat(types::I64, f);
      self.fb.def_var(self.reg_vars_int[reg as usize], iv);
    }
  }

  /// `Instr::Call`'s fully general codegen, for a site where nothing
  /// stronger was proven about the callee and this is therefore the
  /// real dispatch path every call actually takes. Guards inline on a
  /// compiled JIT closure and executes it directly with inline frame
  /// push/pop, falling back to the resolver-driven
  /// `zuri_jit_call_prepare` fast call whenever the callee is not yet
  /// compiled, variadic, or not a plain closure. Worth its size here:
  /// a megamorphic site (a callee read out of a list or a field, so no
  /// resolver fact can pin it down) would otherwise pay a full helper
  /// round trip on every single call.
  fn emit_generic_call(&mut self, dst: u8, func: u8, num_args: u8) {
    self.emit_generic_call_inner(dst, func, num_args, false);
  }

  /// The same call, emitted for a site that only runs when some
  /// enclosing guard has already missed: an inlined callee whose
  /// identity check failed, or a known-native site whose target moved.
  ///
  /// Identical semantics, deliberately smaller code. The inline
  /// dispatch chain above is a dozen-odd blocks of loads, tag checks
  /// and frame construction, and emitting it here buys nothing (this
  /// path is cold by construction) while charging the ENCLOSING
  /// function for it anyway: Cranelift's register allocator reasons
  /// over the whole CFG, so blocks that never execute still shape the
  /// allocation and scheduling decisions made on the hot path beside
  /// them. A guarded fast path exists precisely so its own fallback
  /// stays rare, so the fallback should stay compact and let the
  /// helper do the work.
  fn emit_generic_call_cold(&mut self, dst: u8, func: u8, num_args: u8) {
    self.emit_generic_call_inner(dst, func, num_args, true);
  }

  fn emit_generic_call_inner(&mut self, dst: u8, func: u8, num_args: u8, cold_fallback: bool) {
    let base = self.base_param;
    let vm_p = self.vm_param;
    let func_i = self.idx(func);
    let num_args_i = self.idx(num_args);
    let dst_i = self.idx(dst);
    let new_base = self.fb.ins().iadd_imm_s(base, func as i64 + 1);

    // Over four arguments the inline path cannot pass them in the
    // entry signature's register slots anyway, so it has nothing left
    // to offer; a cold fallback declines it for its own reasons (see
    // `emit_generic_call_cold`).
    if num_args > 4 || cold_fallback {
      self.emit_fast_call(
        "zuri_jit_call_prepare",
        &[vm_p, base, func_i, num_args_i, dst_i],
        new_base,
        dst,
        func + 1,
        num_args,
        "zuri_jit_call",
        &[vm_p, base, func_i, num_args_i, dst_i],
      );
      return;
    }

    let callee_val = self.load_reg(func);
    let slow_block = self.fb.create_block();
    let direct_call_block = self.fb.create_block();
    let join_block = self.fb.create_block();

    let is_obj = self.is_obj(callee_val);
    let obj_block = self.fb.create_block();
    self.fb.ins().brif(is_obj, obj_block, &[], slow_block, &[]);

    self.fb.switch_to_block(obj_block);
    let closure_ptr = self.obj_ptr(callee_val);
    let tag = self.obj_tag(closure_ptr);
    let is_closure = self
      .fb
      .ins()
      .icmp_imm_s(IntCC::Equal, tag, object::OBJ_TAG_CLOSURE as i64);
    let closure_block = self.fb.create_block();
    self
      .fb
      .ins()
      .brif(is_closure, closure_block, &[], slow_block, &[]);

    self.fb.switch_to_block(closure_block);
    let flags = cranelift_codegen::ir::MemFlagsData::trusted();
    let func_val = self.fb.ins().load(
      types::I64,
      flags,
      closure_ptr,
      object::obj_closure_function_offset() as i32,
    );
    let func_is_obj = self.is_obj(func_val);
    let func_obj_block = self.fb.create_block();
    self
      .fb
      .ins()
      .brif(func_is_obj, func_obj_block, &[], slow_block, &[]);

    self.fb.switch_to_block(func_obj_block);
    let func_obj_ptr = self.obj_ptr(func_val);
    let func_tag = self.obj_tag(func_obj_ptr);
    let is_func = self
      .fb
      .ins()
      .icmp_imm_s(IntCC::Equal, func_tag, object::OBJ_TAG_FUNC as i64);
    let check_proto_block = self.fb.create_block();
    self
      .fb
      .ins()
      .brif(is_func, check_proto_block, &[], slow_block, &[]);

    self.fb.switch_to_block(check_proto_block);
    let proto_ptr = self.fb.ins().load(
      types::I64,
      flags,
      func_obj_ptr,
      object::obj_func_proto_offset() as i32,
    );

    let entry_addr = self.fb.ins().load(
      types::I64,
      flags,
      proto_ptr,
      object::obj_function_jit_entry_offset() as i32,
    );
    let zero = self.i64c(0);
    let has_entry = self.fb.ins().icmp(IntCC::NotEqual, entry_addr, zero);
    let entry_block = self.fb.create_block();
    self
      .fb
      .ins()
      .brif(has_entry, entry_block, &[], slow_block, &[]);

    self.fb.switch_to_block(entry_block);
    let arity = self.fb.ins().load(
      types::I8,
      flags,
      proto_ptr,
      object::obj_function_arity_offset() as i32,
    );
    let arity_ok = self
      .fb
      .ins()
      .icmp_imm_s(IntCC::Equal, arity, num_args as i64);

    let variadic = self.fb.ins().load(
      types::I8,
      flags,
      proto_ptr,
      object::obj_function_variadic_offset() as i32,
    );
    let not_variadic = self.fb.ins().icmp_imm_s(IntCC::Equal, variadic, 0);

    let is_method = self.fb.ins().load(
      types::I8,
      flags,
      proto_ptr,
      object::obj_function_is_method_offset() as i32,
    );
    let not_method = self.fb.ins().icmp_imm_s(IntCC::Equal, is_method, 0);

    let sig_part = self.fb.ins().band(arity_ok, not_variadic);
    let sig_ok = self.fb.ins().band(sig_part, not_method);
    let frame_block = self.fb.create_block();
    self
      .fb
      .ins()
      .brif(sig_ok, frame_block, &[], slow_block, &[]);

    self.fb.switch_to_block(frame_block);
    let callee_num_regs8 = self.fb.ins().load(
      types::I8,
      flags,
      proto_ptr,
      object::obj_function_num_registers_offset() as i32,
    );
    let callee_num_regs = self.fb.ins().uextend(types::I64, callee_num_regs8);
    let (depth, frames_len) = self.emit_call_checks_dyn(new_base, callee_num_regs, slow_block);

    self.emit_frame_construction(proto_ptr, dst, new_base, callee_val, depth, frames_len);
    self.fb.ins().jump(direct_call_block, &[]);

    self.fb.switch_to_block(direct_call_block);
    let sig = self.entry_sig_ref();
    let neg1 = self.fb.ins().iconst(types::I32, -1);
    let [a0, a1, a2, a3] = self.load_call_arg_values(func + 1, num_args);
    self.flush_live(self.current_ip);
    let call = self.fb.ins().call_indirect(
      sig,
      entry_addr,
      &[vm_p, new_base, callee_val, neg1, a0, a1, a2, a3],
    );
    let ret_bits = self.fb.inst_results(call)[0];
    self.reload_live(self.current_ip);
    self.refresh_regs();
    self.emit_inline_frame_finish(dst, new_base, ret_bits);
    self.fb.ins().jump(join_block, &[]);

    self.fb.switch_to_block(slow_block);
    self.emit_fast_call(
      "zuri_jit_call_prepare",
      &[vm_p, base, func_i, num_args_i, dst_i],
      new_base,
      dst,
      func + 1,
      num_args,
      "zuri_jit_call",
      &[vm_p, base, func_i, num_args_i, dst_i],
    );
    self.fb.ins().jump(join_block, &[]);

    self.fb.switch_to_block(join_block);
    self.resync_dst_from_memory(dst);
  }

  fn emit_known_call(
    &mut self,
    dst: u8,
    func: u8,
    num_args: u8,
    entry: usize,
    guard_bits: u64,
    proto_ptr: usize,
  ) {
    let base = self.base_param;
    let vm_p = self.vm_param;
    let callee_val = self.load_reg(func);

    let new_base = self.fb.ins().iadd_imm_s(base, func as i64 + 1);
    let num_args_i = self.idx(num_args);
    let dst_i = self.idx(dst);

    let slow_block = self.fb.create_block();
    let fast_block = self.fb.create_block();
    let done_block = self.fb.create_block();
    self.emit_callee_proto_guard(callee_val, guard_bits, slow_block);

    // Eligibility, exactly like `emit_self_call`'s: a compile-time fact
    // about the PROVEN callee (`emit_callee_proto_guard` already
    // guarantees, once past it, that the register holds a closure whose
    // prototype is exactly `proto_ptr`), decided once here rather than
    // as a runtime branch.
    //
    // SAFETY: `proto_ptr` names a live, old-generation `ObjFunction` --
    // see `CallTarget::Known::proto_ptr`'s own docs.
    let callee = unsafe { &*(proto_ptr as *const ObjFunction) };
    if callee.variadic || num_args != callee.arity {
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
      let [a0, a1, a2, a3] = self.load_call_arg_values(func + 1, num_args);
      self.flush_live(self.current_ip);
      let call = self.fb.ins().call_indirect(
        sig,
        entry_addr,
        &[vm_p, new_base, callee_val, neg1, a0, a1, a2, a3],
      );
      let ret_bits = self.fb.inst_results(call)[0];
      self.reload_live(self.current_ip);
      self.refresh_regs();
      self.call_checked(
        "zuri_jit_call_finish",
        &[vm_p, base, dst_i, new_base, ret_bits],
      );
      self.resync_dst_from_memory(dst);
      self.fb.ins().jump(done_block, &[]);

      self.fb.switch_to_block(slow_block);
      let func_i = self.idx(func);
      self.call_checked("zuri_jit_call", &[vm_p, base, func_i, num_args_i, dst_i]);
      self.resync_dst_from_memory(dst);
      self.fb.ins().jump(done_block, &[]);

      self.fb.switch_to_block(done_block);
      return;
    }

    // Fully-inline path: see `emit_inline_frame_push`'s own docs.
    self.emit_inline_frame_push(
      proto_ptr as u64,
      callee.num_registers,
      dst,
      new_base,
      callee_val,
      slow_block,
    );
    self.fb.ins().jump(fast_block, &[]);

    self.fb.switch_to_block(fast_block);
    let entry_addr = self.u64c(entry as u64);
    let sig = self.entry_sig_ref();
    let neg1 = self.fb.ins().iconst(types::I32, -1);
    let [a0, a1, a2, a3] = self.load_call_arg_values(func + 1, num_args);
    self.flush_live(self.current_ip);
    let call = self.fb.ins().call_indirect(
      sig,
      entry_addr,
      &[vm_p, new_base, callee_val, neg1, a0, a1, a2, a3],
    );
    let ret_bits = self.fb.inst_results(call)[0];
    self.reload_live(self.current_ip);
    self.refresh_regs();
    self.emit_inline_frame_finish(dst, new_base, ret_bits);
    self.fb.ins().jump(done_block, &[]);

    self.fb.switch_to_block(slow_block);
    let func_i = self.idx(func);
    self.call_checked("zuri_jit_call", &[vm_p, base, func_i, num_args_i, dst_i]);
    self.resync_dst_from_memory(dst);
    self.fb.ins().jump(done_block, &[]);

    self.fb.switch_to_block(done_block);
  }

  /// `Instr::Invoke`'s eligibility check for `emit_self_invoke`: the
  /// invoked method's name must equal THIS function's own name AND
  /// `VM::resolve_self_class` must have proven `proto` owns that method
  /// on its own class (`self.self_class_bits`, resolved once ahead of
  /// compilation). Resolves the name via `proto.chunk.constants`
  /// directly; a compile-time lookup, like `self_field_slot`'s own --
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
  /// exact compiled function; PROVIDED the class's method table
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
    // Computed here, in the single entry block every later block is
    // dominated by; NOT inside `try_direct_block` (which is already
    // unreachable-via-fallthrough by the time these would otherwise be
    // needed, since Cranelift requires switching blocks before emitting
    // further instructions once one is terminated).
    let new_base = self.fb.ins().iadd_imm_s(base, obj as i64 + 1);
    let num_args_i = self.idx(num_args);
    // `1 + num_args`: the receiver the bytecode compiler already
    // duplicated into `obj + 1` occupies the callee's own register 0
    // ("self"): see `Instr::Invoke`'s own doc comment in chunk.rs and
    // `zuri_jit_invoke_prepare`'s identical `1 + num_args` convention.
    // Passing bare `num_args` here would make `VM::setup_closure_call`
    // treat register 0 as a MISSING positional argument and overwrite
    // it with `nil` whenever `num_args < arity`; exactly the "self.left
    // on a nil" corruption this comment is here to prevent regressing.
    let direct_num_args_i = self.i64c(num_args as i64 + 1);
    let dst_i = self.idx(dst);
    let closure_bits = self.emit_current_closure_val();

    let slow_block = self.fb.create_block();
    let done_block = self.fb.create_block();
    let class_check_block = self.fb.create_block();

    // `obj == 0` means the receiver IS `self`; and `emit_self_invoke`
    // is only ever reached via `self_invoke_target`, which requires
    // `self_class_bits` to be `Some`, which `VM::resolve_self_class`
    // only ever produces when `proto` genuinely IS an instance method
    // that its own class's method table maps back to (see that
    // function's own docs). So whenever THIS specific call is on `self`
    // itself, it's unconditionally an `Obj::Instance` already; the
    // exact same trust `self_field_slot` relies on to skip its own
    // `is_obj`+tag guard for `self.field` access. Only the CLASS check
    // below still needs to run (an override in a subclass can still
    // change which method `self`'s class resolves this name to). Same
    // "exactly one arm ever actually emitted" shape `emit_list_get_
    // index`'s own `proven_list` split uses; `ptr` dominates
    // `class_check_block` either way, just via a shorter chain when
    // `obj == 0`.
    let ptr = if obj == 0 {
      let ptr = self.obj_ptr(receiver);
      self.fb.ins().jump(class_check_block, &[]);
      ptr
    } else {
      let is_obj = self.is_obj(receiver);
      let obj_block = self.fb.create_block();
      self.fb.ins().brif(is_obj, obj_block, &[], slow_block, &[]);

      self.fb.switch_to_block(obj_block);
      let ptr = self.obj_ptr(receiver);
      let tag = self.obj_tag(ptr);
      let tag_instance = self.i64c(object::OBJ_TAG_INSTANCE as i64);
      let is_instance = self.fb.ins().icmp(IntCC::Equal, tag, tag_instance);
      self
        .fb
        .ins()
        .brif(is_instance, class_check_block, &[], slow_block, &[]);
      ptr
    };

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

    // Eligibility, exactly like `emit_self_call`'s: `self_invoke_target`
    // already proved the invoked method IS `self.proto` (this exact
    // compiling function), so its `arity`/`variadic`/`num_registers` are
    // compile-time facts here too. The receiver occupies the callee's
    // own register 0 ("self"), so the real argument count to compare
    // against `arity` is `1 + num_args`, matching `direct_num_args_i`'s
    // own `1 +` convention below.
    if self.proto.variadic || (num_args as u16 + 1) != self.proto.arity as u16 {
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
      let [a0, a1, a2, a3] = self.load_call_arg_values(obj + 1, num_args + 1);
      self.flush_live(ip);
      let call = self.fb.ins().call(
        func_ref,
        &[vm_p, new_base, closure_bits, neg1, a0, a1, a2, a3],
      );
      let ret_bits = self.fb.inst_results(call)[0];
      self.reload_live(ip);
      self.refresh_regs();
      self.call_checked(
        "zuri_jit_call_finish",
        &[vm_p, base, dst_i, new_base, ret_bits],
      );
      self.resync_dst_from_memory(dst);
      self.fb.ins().jump(done_block, &[]);
    } else {
      // Fully-inline path: see `emit_inline_frame_push`'s own docs.
      let proto_bits = self.proto as *const ObjFunction as u64;
      self.emit_inline_frame_push(
        proto_bits,
        self.proto.num_registers,
        dst,
        new_base,
        closure_bits,
        slow_block,
      );
      let fast_block = self.fb.create_block();
      self.fb.ins().jump(fast_block, &[]);

      self.fb.switch_to_block(fast_block);
      let func_ref = self
        .module
        .declare_func_in_func(self.own_func_id, self.fb.func);
      let neg1 = self.fb.ins().iconst(types::I32, -1);
      let [a0, a1, a2, a3] = self.load_call_arg_values(obj + 1, num_args + 1);
      let is_alloc_free = self.is_self_allocation_free();
      if !is_alloc_free {
        self.flush_live(ip);
      }
      let call = self.fb.ins().call(
        func_ref,
        &[vm_p, new_base, closure_bits, neg1, a0, a1, a2, a3],
      );
      let ret_bits = self.fb.inst_results(call)[0];
      if !is_alloc_free {
        self.reload_live(ip);
        self.refresh_regs();
      }
      self.emit_inline_frame_finish(dst, new_base, ret_bits);
      self.fb.ins().jump(done_block, &[]);
    }

    self.fb.switch_to_block(slow_block);
    let obj_i = self.idx(obj);
    let name = self.bake_const(method_const);
    let cache = self.invoke_cache_addr(ip);
    self.call_checked(
      "zuri_jit_invoke",
      &[vm_p, base, obj_i, num_args_i, dst_i, name, cache],
    );
    self.resync_dst_from_memory(dst);
    self.fb.ins().jump(done_block, &[]);

    self.fb.switch_to_block(done_block);
  }

  /// `Instr::GetGlobal`'s inline-cache-style fast path: once this exact
  /// instruction has resolved its name to a ROOT global slot once (see
  /// `JitInfo::global_slot_cache`'s own docs; a qualified-module
  /// resolution never populates this cache, so those always take the
  /// helper path below), every later execution reads the slot straight
  /// out of `VM::global_slots` with two loads and an add, no helper
  /// call, no name lookup at all; this is what makes a self-recursive
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
  /// `emit_safepoint`'s `gc_block` does; avoiding a second, redundant
  /// flush/stale-mark from `call_helper`'s own automatic wrapping.
  fn emit_get_global(&mut self, ip: usize, dst: u8, name_const: u16) {
    self.emit_get_global_load(ip, dst, name_const);
    self.guard_snapshot_list_global(ip, dst, name_const);
  }

  /// The runtime check that makes `transfer_list`'s `GetGlobal` arm safe.
  ///
  /// That arm seeds list-ness straight from `VM::snapshot_global_lists`, a
  /// reading of the heap taken when compilation was requested. Nothing stops
  /// the binding changing afterwards, and `emit_list_get_index`'s proven arm
  /// masks the value and dereferences it with no tag check at all, so
  /// `DATA = [1, 2]` ... `DATA = 7` after the reader is hot turned the number
  /// 7 into a list pointer and segfaulted.
  ///
  /// Only emitted where the fact is actually consumed: if nothing downstream
  /// treats `dst` as a list there is nothing to keep honest, and a global read
  /// in a hot loop should not pay for a fact no one asked for.
  fn guard_snapshot_list_global(&mut self, ip: usize, dst: u8, name_const: u16) {
    if ip + 1 >= self.proto.chunk.code.len() || !self.list_facts.is_list(ip + 1, dst) {
      return;
    }
    let Some(name) = self.proto.chunk.constants.get(name_const as usize) else {
      return;
    };
    if !name.is_string() || !self.global_lists.contains(name.as_str()) {
      return;
    }

    let v = self.load_reg(dst);
    let is_obj = self.is_obj(v);
    let tagged_block = self.fb.create_block();
    let ok_block = self.fb.create_block();
    let deopt_block = self.fb.create_block();
    self
      .fb
      .ins()
      .brif(is_obj, tagged_block, &[], deopt_block, &[]);

    // Tag load waits until `is_obj` holds, the same discipline every other
    // unproven-receiver arm in this file follows.
    self.fb.switch_to_block(tagged_block);
    let ptr = self.obj_ptr(v);
    let tag = self.obj_tag(ptr);
    let tag_list = self.i64c(object::OBJ_TAG_LIST as i64);
    let is_list = self.fb.ins().icmp(IntCC::Equal, tag, tag_list);
    self.fb.ins().brif(is_list, ok_block, &[], deopt_block, &[]);

    self.fb.switch_to_block(deopt_block);
    self.emit_deopt(ip);

    self.fb.switch_to_block(ok_block);
  }

  fn emit_get_global_load(&mut self, ip: usize, dst: u8, name_const: u16) {
    let flags = cranelift_codegen::ir::MemFlagsData::trusted();
    let slot = self.proto.jit.global_slot_cache[ip].get();
    if slot >= 0 {
      if let Some(&var) = self.global_vars.get(&slot) {
        let v = self.fb.use_var(var);
        self.store_reg(dst, v);
        return;
      }
      let slots_ptr = self.fb.ins().load(
        types::I64,
        flags,
        self.vm_param,
        GLOBAL_SLOTS_PTR_CACHE_OFFSET,
      );
      let byte_off = (slot * 8) as i32;
      let v = self.fb.ins().load(types::I64, flags, slots_ptr, byte_off);
      self.store_reg(dst, v);
      return;
    }

    let cache_ptr = self.proto.jit.global_slot_cache.as_ptr() as i64;
    let cache_base = self.i64c(cache_ptr);
    let cached = self
      .fb
      .ins()
      .load(types::I64, flags, cache_base, (ip as i32) * 8);
    let neg1 = self.i64c(-1);
    let is_hit = self.fb.ins().icmp(IntCC::NotEqual, cached, neg1);

    let hit_block = self.fb.create_block();
    let miss_block = self.fb.create_block();
    let done_block = self.fb.create_block();
    self.fb.ins().brif(is_hit, hit_block, &[], miss_block, &[]);

    self.fb.switch_to_block(hit_block);
    let slots_ptr = self.fb.ins().load(
      types::I64,
      flags,
      self.vm_param,
      GLOBAL_SLOTS_PTR_CACHE_OFFSET,
    );
    let byte_off = self.fb.ins().imul_imm_s(cached, 8);
    let addr = self.fb.ins().iadd(slots_ptr, byte_off);
    let v = self.fb.ins().load(types::I64, flags, addr, 0);
    self.store_reg(dst, v);
    self.fb.ins().jump(done_block, &[]);

    self.fb.switch_to_block(miss_block);
    self.publish_ip();
    self.flush_live(ip);
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
    self.reload_live(ip);
    self.refresh_regs();
    self.resync_dst_from_memory(dst);
    self.fb.ins().jump(done_block, &[]);

    self.fb.switch_to_block(done_block);
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

  /// `self_field_slot`'s counterpart for a typed, non-`self` parameter
  ///; `obj` need not be register 0 here, since a plain function's
  /// typed parameters start at register 0 themselves and a method can
  /// have several besides `self`. See `param_field_slots`' own docs.
  fn param_field_slot(&self, obj: u8, name_const: u16) -> Option<u16> {
    let name = self.proto.chunk.constants[name_const as usize];
    self
      .param_field_slots
      .get(&obj)?
      .1
      .get(name.as_str())
      .copied()
  }

  /// A register's resolved class `Value` bits, when it's a proven-
  /// single-class parameter; consulted by `emit_check_param_type` to
  /// inline the CHECK ITSELF (not just downstream field access) down to
  /// one class-bits compare. See `param_field_slots`' own docs on why
  /// this matters: without it, an `Instance`-typed parameter's check
  /// unconditionally calls the general helper, which for a hot function
  /// can cost more than the field-access savings ever recover.
  /// `(class bits, slot)` for `obj.name` when `obj` holds an instance
  /// built by a construct earlier in this function. Only past the
  /// construct: before it the register holds the class global itself.
  fn construct_field_slot(&self, ip: usize, obj: u8, name_const: u16) -> Option<(u64, u16)> {
    if self.field_speculation_off || self.deopt_sites.contains(&ip) {
      return None;
    }
    let (construct_ip, bits, slots) = self.construct_field_slots.get(&obj)?;
    if ip <= *construct_ip {
      return None;
    }
    let name = self.proto.chunk.constants[name_const as usize];
    slots.get(name.as_str()).copied().map(|slot| (*bits, slot))
  }

  /// `obj.field` where the class is known from a construct: one
  /// compare against the class, then a load at a constant slot.
  ///
  /// Cheaper than the inline cache it replaces, which has to load the
  /// cached class AND the cached slot out of memory before it can do
  /// the same load. The class compare stays because the register could
  /// in principle be read on a path that never ran the construct; a
  /// mismatch simply takes the general path.
  fn emit_construct_field(
    &mut self,
    ip: usize,
    dst: Option<u8>,
    obj: u8,
    name_const: u16,
    src: Option<u8>,
    class_bits: u64,
    slot: u16,
  ) {
    let obj_val = self.load_reg(obj);
    let flags = cranelift_codegen::ir::MemFlagsData::trusted();
    let _ = name_const;

    // Deopt rather than a slow-path call, exactly as the inline cache
    // does: the construct that fixed this class is in this very
    // function, so a mismatch means control reached the access without
    // running the construct, and bailing to the interpreter for that
    // is both correct and far less code than a second call site.
    let obj_block = self.fb.create_block();
    let fast_block = self.fb.create_block();
    let deopt_block = self.fb.create_block();
    let is_obj = self.is_obj(obj_val);
    self.fb.ins().brif(is_obj, obj_block, &[], deopt_block, &[]);

    self.fb.switch_to_block(obj_block);
    let ptr = self.obj_ptr(obj_val);
    let cls = self.fb.ins().load(
      types::I64,
      flags,
      ptr,
      object::obj_instance_class_offset() as i32,
    );
    let want = self.u64c(class_bits);
    let same = self.fb.ins().icmp(IntCC::Equal, cls, want);
    self.fb.ins().brif(same, fast_block, &[], deopt_block, &[]);

    self.fb.switch_to_block(deopt_block);
    self.emit_deopt(ip);

    self.fb.switch_to_block(fast_block);
    let fields_ptr = self.load_instance_fields_ptr(ptr);
    match (dst, src) {
      (Some(d), None) => {
        let v = self
          .fb
          .ins()
          .load(types::I64, flags, fields_ptr, (slot as i32) * 8);
        self.store_reg(d, v);
      },
      (None, Some(sr)) => {
        let v = self.load_reg(sr);
        self.fb.ins().store(flags, v, fields_ptr, (slot as i32) * 8);
        self.emit_write_barrier_for_store(ip, sr, v, ptr);
      },
      _ => unreachable!("emit_construct_field takes exactly one of dst/src"),
    }
  }

  fn param_class_bits(&self, obj: u8) -> Option<u64> {
    self.param_field_slots.get(&obj).map(|&(bits, _)| bits)
  }

  fn find_unique_known_class_for_field(&self, name: &str) -> Option<(u64, u16)> {
    let mut found: Option<(u64, u16)> = None;
    for (&class_bits, slots) in &self.known_classes {
      if let Some(&slot) = slots.get(name) {
        if found.is_some() {
          return None;
        }
        found = Some((class_bits, slot));
      }
    }
    found
  }

  /// The class this `GetField`/`SetField` site should guard against,
  /// with its slot, or `None` to leave the site on its ordinary inline
  /// cache.
  ///
  /// `find_unique_known_class_for_field` picks a class from the field
  /// name alone, which says nothing about whether the receiver is even
  /// an instance: a dictionary reaches the same instruction. When that
  /// bet is wrong it is wrong on every call, and a missed guard here
  /// leaves compiled code entirely, so the site is worth far less than
  /// nothing. `deopt_sites` is what a previous compilation learned
  /// about exactly that.
  fn target_class_for_field(&self, ip: usize, obj: u8, name: &str) -> Option<(u64, u16)> {
    if self.field_speculation_off
      || self.deopt_sites.contains(&ip)
      || self.deopt_receivers.contains(&obj)
    {
      return None;
    }
    if let Some(cell) = self.proto.chunk.field_cache_cell(ip) {
      let c_bits = cell.class_bits.get();
      if c_bits != 0 {
        if let Some(slots) = self.known_classes.get(&c_bits) {
          if let Some(&slot) = slots.get(name) {
            return Some((c_bits, slot));
          }
        }
      }
    }
    self.find_unique_known_class_for_field(name)
  }

  /// `self.field` read fast path for a field PROVEN (see
  /// `self_field_slot`/`param_field_slot`) to live at a fixed slot on
  /// the receiver's own class, with no `BoundMethod`-wrapping risk. No
  /// helper call, no `RefCell` borrow, no hashmap probe on the common
  /// path: a direct load at `object::obj_instance_fields_ptr_offset()`
  /// (fixed since `ObjInstance`/`FieldStorage` are `#[repr(C)]`) plus
  /// `slot * 8`.
  ///
  /// `proven` distinguishes the two callers: `self_field_slot`'s case
  /// (`proven = false`) still checks `is_obj`+tag defensively (should
  /// be unreachable; a method's `self` is always the instance it was
  /// invoked on; but checked rather than assumed, since that's an
  /// invariant of the CALLING CONVENTION, never independently
  /// verified). `param_field_slot`'s case (`proven = true`) has a
  /// STRONGER guarantee than that: an explicit, RAISING
  /// `Instr::CheckParamType` already ran on this exact register, and
  /// `resolve_param_field_slots` already proved nothing rewrites it
  /// afterward; so re-deriving the same fact here would be checking
  /// something the bytecode itself already enforces, not defending
  /// against a genuine unknown. `proven = true` skips straight to the
  /// load, no branch, no slow path, no snapshot/restore at all.
  ///
  /// The `proven = false` arm follows `emit_binary_numeric_guarded`'s
  /// exact snapshot/restore discipline around the fast/slow split --
  /// see `restore_dirty_from_snapshot`'s own docs for the real bug
  /// class that protects against (Cranelift compiles both arms
  /// unconditionally, so the slow arm's `call_checked` would otherwise
  /// corrupt this compiler's OWN compile-time liveness bookkeeping for
  /// registers this instruction never touches, even when the slow arm
  /// never runs at runtime).
  fn emit_self_get_field(
    &mut self,
    ip: usize,
    dst: u8,
    obj: u8,
    name_const: u16,
    slot: u16,
    proven: bool,
  ) {
    let self_val = self.load_reg(obj);

    if proven {
      let ptr = self.obj_ptr(self_val);
      let fields_ptr = self.load_instance_fields_ptr(ptr);
      let v = self.fb.ins().load(
        types::I64,
        cranelift_codegen::ir::MemFlagsData::trusted(),
        fields_ptr,
        (slot as i32) * 8,
      );
      self.store_reg(dst, v);
      return;
    }

    let is_obj = self.is_obj(self_val);

    let obj_block = self.fb.create_block();
    let slow_block = self.fb.create_block();
    let done_block = self.fb.create_block();
    self.fb.ins().brif(is_obj, obj_block, &[], slow_block, &[]);

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
  }

  /// `self.field = ...` write fast path; the write-side counterpart
  /// of `emit_self_get_field`: see its own docs on `proven` (`false`
  /// for `self_field_slot`'s defensive-checked case, `true` for
  /// `param_field_slot`'s already-raised-if-wrong case, which skips
  /// straight to the store). Defines no VM register at all (only reads
  /// `src`), so nothing is held back from the merged restore in the
  /// `proven = false` arm; the receiver is reconciled across the two
  /// arms by `resync_receiver_from_memory` instead, exactly as in
  /// `emit_ic_set_field`: see its docs for the disagreement that
  /// closes. Only one `call_helper` site exists in this function (the
  /// slow path), so the OTHER real bug class this file's
  /// snapshot/restore machinery guards against; two INDEPENDENT
  /// `call_helper` sites in different branches, see
  /// `emit_list_set_index`'s own docs; doesn't apply here.
  fn emit_self_set_field(
    &mut self,
    ip: usize,
    obj: u8,
    name_const: u16,
    src: u8,
    slot: u16,
    proven: bool,
  ) {
    let self_val = self.load_reg(obj);
    let src_val = self.load_reg(src);

    if proven {
      let ptr = self.obj_ptr(self_val);
      let fields_ptr = self.load_instance_fields_ptr(ptr);
      self.fb.ins().store(
        cranelift_codegen::ir::MemFlagsData::trusted(),
        src_val,
        fields_ptr,
        (slot as i32) * 8,
      );
      self.emit_write_barrier_for_store(ip, src, src_val, ptr);
      return;
    }

    let is_obj = self.is_obj(self_val);

    let obj_block = self.fb.create_block();
    let slow_block = self.fb.create_block();
    let done_block = self.fb.create_block();
    self.fb.ins().brif(is_obj, obj_block, &[], slow_block, &[]);

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
    self.emit_write_barrier_for_store(ip, src, src_val, ptr);
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
  }

  /// The baked address of this `Instr::Invoke` position's own cache
  /// cell, or a null constant when the chunk has no cell for it (see
  /// `Chunk::invoke_cache_cell`); the helper treats null as "no
  /// cache" and simply resolves every time.
  fn invoke_cache_addr(&mut self, ip: usize) -> IrValue {
    let addr = self
      .proto
      .chunk
      .invoke_cache_cell(ip)
      .map_or(0, |cell| cell as *const _ as u64);
    self.u64c(addr)
  }

  /// The address of this instruction's own `chunk::FieldCacheCell`,
  /// baked as an immediate; `None` when the chunk has no cell for
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
  /// wrong union arm: see `emit_self_get_field`'s identical two-stage
  /// discipline, which this extends by one stage.
  ///
  /// `obj` (the receiver's OWN register, not its already-loaded value)
  /// is consulted against `proven_param_shapes`: a parameter checked
  /// `Instr::CheckParamType`-non-nullable against exactly ONE class has
  /// already paid for "is this an `Obj::Instance`" once, at the check
  ///; skip re-deriving it here and go straight to the class load. The
  /// per-site class MATCH still runs unconditionally regardless (a
  /// proven parameter can still be any subclass, or in practice any
  /// class at all if the check's declared class doesn't match this
  /// site's own field), so a mismatch still correctly falls to
  /// `slow_block`, same as an unproven receiver.
  fn emit_ic_guard(
    &mut self,
    obj: u8,
    recv: IrValue,
    cache_addr: IrValue,
    slow_block: Block,
  ) -> (IrValue, IrValue) {
    let proven_instance = self.proven_param_shapes.get(&obj) == Some(&ParamShape::Instance);

    let ptr = if proven_instance {
      self.obj_ptr(recv)
    } else {
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
      ptr
    };

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
  /// which handles the one case; `self.field` inside a method of the
  /// owning class; where the slot is provable at compile time and
  /// needs no cache or class guard at all; that path stays separate and
  /// is always preferred, since it is strictly cheaper.
  ///
  /// This is what takes field-heavy numeric code off the
  /// `zuri_jit_get_field` helper entirely: on a hit it is three
  /// dependent loads and three not-taken branches, with no call, no
  /// `RefCell` borrow, no hash probe, and; crucially; no
  /// `flush_live`/`mark_stale_live` round trip forcing every live
  /// register back through memory.
  ///
  /// Every non-instance receiver (a class's statics, a module member, a
  /// dict key, a method being read as a bound method) and every cache
  /// miss falls through to the unchanged helper, which is still the
  /// only implementation of those cases and is also what FILLS the
  /// cache for the next time round.
  fn emit_ic_get_field(&mut self, ip: usize, dst: u8, obj: u8, name_const: u16, cache: IrValue) {
    let flags = cranelift_codegen::ir::MemFlagsData::trusted();
    let name_val = self.proto.chunk.constants[name_const as usize];
    let name = name_val.as_str();

    if self.is_specialized_pass {
      if let Some(&guarded_class) = self.active_guarded_classes.get(&obj) {
        if let Some(&slot) = self
          .known_classes
          .get(&guarded_class)
          .and_then(|m| m.get(name))
        {
          let (_ptr_var, fields_ptr_var, _class_var) = self.guarded_instance_vars[&obj];
          let fields_ptr = self.fb.use_var(fields_ptr_var);
          let v = self
            .fb
            .ins()
            .load(types::I64, flags, fields_ptr, (slot as i32) * 8);
          self.store_reg(dst, v);
          if dst == obj {
            self.active_guarded.remove(&obj);
            self.active_guarded_classes.remove(&obj);
          }
          return;
        }
      } else if let Some((target_class_bits, slot)) = self.target_class_for_field(ip, obj, name) {
        let (ptr_var, fields_ptr_var, class_var) = self.guarded_instance_vars[&obj];
        let recv = self.load_reg(obj);

        let deopt_block = self.fb.create_block();
        let hit_block = self.fb.create_block();

        let proven_instance = self.proven_param_shapes.get(&obj) == Some(&ParamShape::Instance);
        let ptr = if proven_instance {
          self.obj_ptr(recv)
        } else {
          let obj_block = self.fb.create_block();
          let inst_block = self.fb.create_block();
          let is_obj = self.is_obj(recv);
          self.fb.ins().brif(is_obj, obj_block, &[], deopt_block, &[]);

          self.fb.switch_to_block(obj_block);
          let ptr = self.obj_ptr(recv);
          let tag = self.obj_tag(ptr);
          let tag_instance = self.i64c(object::OBJ_TAG_INSTANCE as i64);
          let is_instance = self.fb.ins().icmp(IntCC::Equal, tag, tag_instance);
          self
            .fb
            .ins()
            .brif(is_instance, inst_block, &[], deopt_block, &[]);

          self.fb.switch_to_block(inst_block);
          ptr
        };

        let class_off = object::obj_instance_class_offset() as i32;
        let class_bits = self.fb.ins().load(types::I64, flags, ptr, class_off);
        let expected_class = self.u64c(target_class_bits);
        let same_class = self.fb.ins().icmp(IntCC::Equal, class_bits, expected_class);
        self
          .fb
          .ins()
          .brif(same_class, hit_block, &[], deopt_block, &[]);

        self.fb.switch_to_block(deopt_block);
        self.emit_deopt(ip);

        self.fb.switch_to_block(hit_block);
        let fields_ptr = self.load_instance_fields_ptr(ptr);
        self.fb.def_var(ptr_var, ptr);
        self.fb.def_var(fields_ptr_var, fields_ptr);
        self.fb.def_var(class_var, class_bits);
        self.active_guarded.insert(obj);
        self.active_guarded_classes.insert(obj, target_class_bits);

        let v = self
          .fb
          .ins()
          .load(types::I64, flags, fields_ptr, (slot as i32) * 8);
        self.store_reg(dst, v);
        if dst == obj {
          self.active_guarded.remove(&obj);
          self.active_guarded_classes.remove(&obj);
        }
        return;
      }
    }

    if self.active_guarded.contains(&obj) {
      let (_ptr_var, fields_ptr_var, class_var) = self.guarded_instance_vars[&obj];
      let fields_ptr = self.fb.use_var(fields_ptr_var);
      let guarded_class = self.fb.use_var(class_var);
      let cached_class = self.fb.ins().load(types::I64, flags, cache, 0);
      let not_zero = self.fb.ins().icmp_imm_u(IntCC::NotEqual, guarded_class, 0);
      let same_class = self
        .fb
        .ins()
        .icmp(IntCC::Equal, guarded_class, cached_class);
      let guard_ok = self.fb.ins().band(not_zero, same_class);

      let slow_block = self.fb.create_block();
      let done_block = self.fb.create_block();
      let hit_block = self.fb.create_block();
      self
        .fb
        .ins()
        .brif(guard_ok, hit_block, &[], slow_block, &[]);

      self.fb.switch_to_block(hit_block);
      let byte_offset = self.fb.ins().load(types::I64, flags, cache, 8);
      let addr = self.fb.ins().iadd(fields_ptr, byte_offset);
      let v = self.fb.ins().load(types::I64, flags, addr, 0);

      if self.proven_numeric(ip, dst) {
        let is_num = self.is_number(v);
        let num_ok_block = self.fb.create_block();
        self
          .fb
          .ins()
          .brif(is_num, num_ok_block, &[], slow_block, &[]);
        self.fb.switch_to_block(num_ok_block);
      }
      self.store_reg(dst, v);
      self.fb.ins().jump(done_block, &[]);

      self.fb.switch_to_block(slow_block);
      let zero = self.i64c(0);
      self.fb.def_var(class_var, zero);
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
      return;
    }

    let (ptr_var, fields_ptr_var, class_var) = self.guarded_instance_vars[&obj];
    let recv = self.load_reg(obj);

    let slow_block = self.fb.create_block();
    let done_block = self.fb.create_block();

    let (ptr, byte_offset) = self.emit_ic_guard(obj, recv, cache, slow_block);
    let fields_ptr = self.load_instance_fields_ptr(ptr);
    let class_off = object::obj_instance_class_offset() as i32;
    let class_bits = self.fb.ins().load(types::I64, flags, ptr, class_off);
    self.fb.def_var(ptr_var, ptr);
    self.fb.def_var(fields_ptr_var, fields_ptr);
    self.fb.def_var(class_var, class_bits);

    let addr = self.fb.ins().iadd(fields_ptr, byte_offset);
    let v = self.fb.ins().load(types::I64, flags, addr, 0);

    if self.proven_numeric(ip, dst) {
      let is_num = self.is_number(v);
      let num_ok_block = self.fb.create_block();
      self
        .fb
        .ins()
        .brif(is_num, num_ok_block, &[], slow_block, &[]);
      self.fb.switch_to_block(num_ok_block);
    }
    self.store_reg(dst, v);
    self.fb.ins().jump(done_block, &[]);

    self.fb.switch_to_block(slow_block);
    let zero = self.i64c(0);
    self.fb.def_var(class_var, zero);
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
    self.active_guarded.insert(obj);
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

  /// `obj.field = ...` write fast path; `emit_ic_get_field`'s
  /// counterpart, with the same guard chain plus the write barrier
  /// every field mutation owes (see `emit_write_barrier`).
  ///
  /// Defines no register, so nothing is held back from the merged
  /// restore; the receiver included, since `resync_receiver_from_memory`
  /// has already made its `Variable` authoritative on both arms.
  fn emit_ic_set_field(&mut self, ip: usize, obj: u8, name_const: u16, src: u8, cache: IrValue) {
    let flags = cranelift_codegen::ir::MemFlagsData::trusted();
    let src_val = self.load_reg(src);
    let name_val = self.proto.chunk.constants[name_const as usize];
    let name = name_val.as_str();

    if self.is_specialized_pass {
      if let Some(&guarded_class) = self.active_guarded_classes.get(&obj) {
        if let Some(&slot) = self
          .known_classes
          .get(&guarded_class)
          .and_then(|m| m.get(name))
        {
          let (ptr_var, fields_ptr_var, _class_var) = self.guarded_instance_vars[&obj];
          let ptr = self.fb.use_var(ptr_var);
          let fields_ptr = self.fb.use_var(fields_ptr_var);
          self
            .fb
            .ins()
            .store(flags, src_val, fields_ptr, (slot as i32) * 8);
          self.emit_write_barrier_for_store(ip, src, src_val, ptr);
          return;
        }
      } else if let Some((target_class_bits, slot)) = self.target_class_for_field(ip, obj, name) {
        let (ptr_var, fields_ptr_var, class_var) = self.guarded_instance_vars[&obj];
        let recv = self.load_reg(obj);

        let deopt_block = self.fb.create_block();
        let hit_block = self.fb.create_block();

        let proven_instance = self.proven_param_shapes.get(&obj) == Some(&ParamShape::Instance);
        let ptr = if proven_instance {
          self.obj_ptr(recv)
        } else {
          let obj_block = self.fb.create_block();
          let inst_block = self.fb.create_block();
          let is_obj = self.is_obj(recv);
          self.fb.ins().brif(is_obj, obj_block, &[], deopt_block, &[]);

          self.fb.switch_to_block(obj_block);
          let ptr = self.obj_ptr(recv);
          let tag = self.obj_tag(ptr);
          let tag_instance = self.i64c(object::OBJ_TAG_INSTANCE as i64);
          let is_instance = self.fb.ins().icmp(IntCC::Equal, tag, tag_instance);
          self
            .fb
            .ins()
            .brif(is_instance, inst_block, &[], deopt_block, &[]);

          self.fb.switch_to_block(inst_block);
          ptr
        };

        let class_off = object::obj_instance_class_offset() as i32;
        let class_bits = self.fb.ins().load(types::I64, flags, ptr, class_off);
        let expected_class = self.u64c(target_class_bits);
        let same_class = self.fb.ins().icmp(IntCC::Equal, class_bits, expected_class);
        self
          .fb
          .ins()
          .brif(same_class, hit_block, &[], deopt_block, &[]);

        self.fb.switch_to_block(deopt_block);
        self.emit_deopt(ip);

        self.fb.switch_to_block(hit_block);
        let fields_ptr = self.load_instance_fields_ptr(ptr);
        self.fb.def_var(ptr_var, ptr);
        self.fb.def_var(fields_ptr_var, fields_ptr);
        self.fb.def_var(class_var, class_bits);
        self.active_guarded.insert(obj);
        self.active_guarded_classes.insert(obj, target_class_bits);

        self
          .fb
          .ins()
          .store(flags, src_val, fields_ptr, (slot as i32) * 8);
        self.emit_write_barrier_for_store(ip, src, src_val, ptr);
        return;
      }
    }

    if self.active_guarded.contains(&obj) {
      let (ptr_var, fields_ptr_var, class_var) = self.guarded_instance_vars[&obj];
      let ptr = self.fb.use_var(ptr_var);
      let fields_ptr = self.fb.use_var(fields_ptr_var);
      let guarded_class = self.fb.use_var(class_var);
      let cached_class = self.fb.ins().load(types::I64, flags, cache, 0);
      let not_zero = self.fb.ins().icmp_imm_u(IntCC::NotEqual, guarded_class, 0);
      let same_class = self
        .fb
        .ins()
        .icmp(IntCC::Equal, guarded_class, cached_class);
      let guard_ok = self.fb.ins().band(not_zero, same_class);

      let slow_block = self.fb.create_block();
      let done_block = self.fb.create_block();
      let hit_block = self.fb.create_block();
      self
        .fb
        .ins()
        .brif(guard_ok, hit_block, &[], slow_block, &[]);

      self.fb.switch_to_block(hit_block);
      let byte_offset = self.fb.ins().load(types::I64, flags, cache, 8);
      let addr = self.fb.ins().iadd(fields_ptr, byte_offset);
      self.fb.ins().store(flags, src_val, addr, 0);
      if !self.proven_numeric(ip, src) {
        let is_obj = self.is_obj(src_val);
        let barrier_block = self.fb.create_block();
        let pass_block = self.fb.create_block();
        self
          .fb
          .ins()
          .brif(is_obj, barrier_block, &[], pass_block, &[]);
        self.fb.switch_to_block(barrier_block);
        self.emit_write_barrier(ptr);
        self.fb.ins().jump(pass_block, &[]);
        self.fb.switch_to_block(pass_block);
      }
      self.fb.ins().jump(done_block, &[]);

      self.fb.switch_to_block(slow_block);
      let zero = self.i64c(0);
      self.fb.def_var(class_var, zero);
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
      return;
    }

    let (ptr_var, fields_ptr_var, class_var) = self.guarded_instance_vars[&obj];
    let recv = self.load_reg(obj);

    let slow_block = self.fb.create_block();
    let done_block = self.fb.create_block();

    let (ptr, byte_offset) = self.emit_ic_guard(obj, recv, cache, slow_block);
    let fields_ptr = self.load_instance_fields_ptr(ptr);
    let class_off = object::obj_instance_class_offset() as i32;
    let class_bits = self.fb.ins().load(types::I64, flags, ptr, class_off);
    self.fb.def_var(ptr_var, ptr);
    self.fb.def_var(fields_ptr_var, fields_ptr);
    self.fb.def_var(class_var, class_bits);

    let addr = self.fb.ins().iadd(fields_ptr, byte_offset);
    self.fb.ins().store(flags, src_val, addr, 0);
    if !self.proven_numeric(ip, src) {
      let is_obj = self.is_obj(src_val);
      let barrier_block = self.fb.create_block();
      let pass_block = self.fb.create_block();
      self
        .fb
        .ins()
        .brif(is_obj, barrier_block, &[], pass_block, &[]);
      self.fb.switch_to_block(barrier_block);
      self.emit_write_barrier(ptr);
      self.fb.ins().jump(pass_block, &[]);
      self.fb.switch_to_block(pass_block);
    }
    self.fb.ins().jump(done_block, &[]);

    self.fb.switch_to_block(slow_block);
    let zero = self.i64c(0);
    self.fb.def_var(class_var, zero);
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
    self.active_guarded.insert(obj);
  }

  /// `Instr::Invoke`'s ordinary codegen: the compiled-method fast call
  /// when `self_invoke_target` proves the receiver's class resolves
  /// this name to the method being compiled, otherwise the general
  /// inline-cache-style `zuri_jit_invoke_prepare` path. Extracted so
  /// `emit_number_intrinsic` can reuse it verbatim as its own guard's
  /// slow arm.
  ///
  /// Does NOT emit the safepoint the `Instr::Invoke` arm owes; callers
  /// place that themselves, since the intrinsic path deliberately skips
  /// it (see `emit_number_intrinsic`).
  fn emit_generic_invoke(&mut self, ip: usize, dst: u8, obj: u8, method_const: u16, num_args: u8) {
    if let Some((class_bits, generation)) = self.self_invoke_target(method_const) {
      self.emit_self_invoke(ip, dst, obj, method_const, num_args, class_bits, generation);
      return;
    }

    // A small accessor on the compiling function's own class becomes its
    // own body rather than a call: see `invoke_inline_target`.
    if let Some((class_bits, generation)) = self.self_class_bits
      && let Some(callee) = self.invoke_inline_target(method_const, num_args)
    {
      self.emit_invoke_inline(
        ip,
        dst,
        obj,
        method_const,
        num_args,
        class_bits,
        generation,
        callee,
      );
      return;
    }

    if self.emit_invoke_ic(ip, dst, obj, method_const, num_args) {
      return;
    }

    self.emit_invoke_helper(ip, dst, obj, method_const, num_args);
  }

  /// `Instr::Invoke`'s ordinary out-of-line path: the inline-cache-style
  /// `zuri_jit_invoke_prepare` attempt, falling back to the full
  /// `zuri_jit_invoke` resolver. Also the guard-miss arm of
  /// `emit_invoke_inline`.
  /// The whole point of `chunk::InvokeCacheCell` carrying an entry: once a
  /// call site has been prepared once, generated code can take the call
  /// itself instead of going back through `zuri_jit_invoke_prepare` on
  /// every single invoke.
  ///
  /// Emits, inline: receiver is an `Obj::Instance`, its class is bit-equal
  /// to the cell's key, and the cell holds a compiled entry. On a hit the
  /// only call left is `zuri_jit_direct_call_prepare`, which just checks
  /// depth and sets up the frame; the resolution, the shape checks, the
  /// stabilization and the closure/proto/entry chase all disappear. Any
  /// miss falls through to `emit_invoke_helper`, which resolves for real
  /// and repopulates the cell.
  ///
  /// The class-key check is exactly the one `zuri_jit_invoke_prepare`
  /// already performs, so this moves that test rather than weakening it;
  /// `Chunk::invoke_cache`'s own docs cover the relocation hazard the key
  /// carries, unchanged either way. Reading `payload` is safe because
  /// `invoke_prepare` now writes the cell only after
  /// `ensure_stable_for_compiled_entry` has promoted the closure out of
  /// the nursery.
  ///
  /// Returns `false` when the chunk has no cell for this position, leaving
  /// the caller to emit the ordinary path alone.
  fn emit_invoke_ic(
    &mut self,
    ip: usize,
    dst: u8,
    obj: u8,
    method_const: u16,
    num_args: u8,
  ) -> bool {
    if num_args as usize + 1 > 4 {
      return false;
    }
    let Some(cell) = self.proto.chunk.invoke_cache_cell(ip) else {
      return false;
    };
    let cell_addr = cell as *const _ as u64;

    let flags = cranelift_codegen::ir::MemFlagsData::trusted();
    let vm_p = self.vm_param;
    let base = self.base_param;
    // Register `obj`, not `obj + 1`: the guard has to inspect the same
    // receiver `zuri_jit_invoke_prepare` and `emit_self_invoke` do.
    let receiver = self.load_reg(obj);
    let new_base = self.fb.ins().iadd_imm_s(base, obj as i64 + 1);
    let cell_ptr = self.u64c(cell_addr);

    let miss_block = self.fb.create_block();
    let done_block = self.fb.create_block();
    let obj_block = self.fb.create_block();
    let inst_block = self.fb.create_block();
    let key_block = self.fb.create_block();
    let entry_block = self.fb.create_block();

    let is_obj = self.is_obj(receiver);
    self.fb.ins().brif(is_obj, obj_block, &[], miss_block, &[]);

    self.fb.switch_to_block(obj_block);
    let ptr = self.obj_ptr(receiver);
    let tag = self.obj_tag(ptr);
    let tag_instance = self.i64c(object::OBJ_TAG_INSTANCE as i64);
    let is_instance = self.fb.ins().icmp(IntCC::Equal, tag, tag_instance);
    self
      .fb
      .ins()
      .brif(is_instance, inst_block, &[], miss_block, &[]);

    self.fb.switch_to_block(inst_block);
    let class_off = object::obj_instance_class_offset() as i32;
    let class_val = self.fb.ins().load(types::I64, flags, ptr, class_off);
    let key = self.fb.ins().load(types::I64, flags, cell_ptr, 0);
    let key_hit = self.fb.ins().icmp(IntCC::Equal, class_val, key);
    self.fb.ins().brif(key_hit, key_block, &[], miss_block, &[]);

    self.fb.switch_to_block(key_block);
    let entry = self.fb.ins().load(types::I64, flags, cell_ptr, 16);
    let zero = self.i64c(0);
    let has_entry = self.fb.ins().icmp(IntCC::NotEqual, entry, zero);
    self
      .fb
      .ins()
      .brif(has_entry, entry_block, &[], miss_block, &[]);

    // From here the sequence is `emit_generic_call_inner`'s, which takes an
    // `Instr::Call` with no helper call at all. The only differences: the
    // callee arrives from the cache rather than a register, and an INVOKED
    // callee is of course a method, so that path's `not_method` check is
    // deliberately absent.
    self.fb.switch_to_block(entry_block);
    let method_bits = self.fb.ins().load(types::I64, flags, cell_ptr, 8);
    let closure_ptr = self.obj_ptr(method_bits);
    let func_val = self.fb.ins().load(
      types::I64,
      flags,
      closure_ptr,
      object::obj_closure_function_offset() as i32,
    );
    // Two hops, as `emit_generic_call_inner` does: `obj_ptr` reaches the
    // `Obj::Func` wrapper, and the prototype itself lives one load further
    // in. Stopping at the wrapper reads arity and variadic out of the wrong
    // object.
    let func_obj_ptr = self.obj_ptr(func_val);
    let proto_ptr = self.fb.ins().load(
      types::I64,
      flags,
      func_obj_ptr,
      object::obj_func_proto_offset() as i32,
    );

    let arity = self.fb.ins().load(
      types::I8,
      flags,
      proto_ptr,
      object::obj_function_arity_offset() as i32,
    );
    let arity_ok = self
      .fb
      .ins()
      .icmp_imm_s(IntCC::Equal, arity, num_args as i64 + 1);
    let variadic = self.fb.ins().load(
      types::I8,
      flags,
      proto_ptr,
      object::obj_function_variadic_offset() as i32,
    );
    let not_variadic = self.fb.ins().icmp_imm_s(IntCC::Equal, variadic, 0);
    let sig_ok = self.fb.ins().band(arity_ok, not_variadic);
    let frame_block = self.fb.create_block();
    self
      .fb
      .ins()
      .brif(sig_ok, frame_block, &[], miss_block, &[]);

    self.fb.switch_to_block(frame_block);
    let callee_num_regs8 = self.fb.ins().load(
      types::I8,
      flags,
      proto_ptr,
      object::obj_function_num_registers_offset() as i32,
    );
    let callee_num_regs = self.fb.ins().uextend(types::I64, callee_num_regs8);
    let (depth, frames_len) = self.emit_call_checks_dyn(new_base, callee_num_regs, miss_block);
    self.emit_frame_construction(proto_ptr, dst, new_base, method_bits, depth, frames_len);

    let neg1 = self.fb.ins().iconst(types::I32, -1);
    let sig = self.entry_sig_ref();
    let [a0, a1, a2, a3] = self.load_call_arg_values(obj + 1, num_args + 1);
    self.flush_live(ip);
    let call = self.fb.ins().call_indirect(
      sig,
      entry,
      &[vm_p, new_base, method_bits, neg1, a0, a1, a2, a3],
    );
    let ret_bits = self.fb.inst_results(call)[0];
    self.reload_live(ip);
    self.refresh_regs();
    self.emit_inline_frame_finish(dst, new_base, ret_bits);
    self.fb.ins().jump(done_block, &[]);

    // Cold: first execution of this site, a receiver of a different class,
    // or a callee not compiled yet. Resolves for real and refills the cell.
    self.fb.switch_to_block(miss_block);
    self.emit_invoke_helper(ip, dst, obj, method_const, num_args);
    self.fb.ins().jump(done_block, &[]);

    self.fb.switch_to_block(done_block);
    true
  }

  fn emit_invoke_helper(&mut self, ip: usize, dst: u8, obj: u8, method_const: u16, num_args: u8) {
    let base = self.base_param;
    let vm_p = self.vm_param;
    let obj_i = self.idx(obj);
    let num_args_i = self.idx(num_args);
    let dst_i = self.idx(dst);
    let name = self.bake_const(method_const);
    let func_ptr = self.func_ptr_const();
    let ip_c = self.u64c(ip as u64);
    let new_base = self.fb.ins().iadd_imm_s(base, obj as i64 + 1);
    let cache = self.invoke_cache_addr(ip);
    self.emit_fast_call(
      "zuri_jit_invoke_prepare",
      &[vm_p, base, obj_i, num_args_i, dst_i, name, func_ptr, ip_c],
      new_base,
      dst,
      obj + 1,
      num_args + 1,
      "zuri_jit_invoke",
      &[vm_p, base, obj_i, num_args_i, dst_i, name, cache],
    );
  }

  /// How many bytecode ops an invoke-inlined accessor may have, and how
  /// many branch points it may fork on. The body is emitted once per path,
  /// so the fork budget is what bounds code growth.
  const MAX_INVOKE_INLINE_OPS: usize = 16;
  const MAX_INVOKE_INLINE_FORKS: u32 = 3;

  /// Decides whether the method `method_const` names on the compiling
  /// function's own class is a small enough accessor to emit inline at an
  /// `Instr::Invoke` site instead of calling it.
  ///
  /// A deliberately separate judgement from `inline_plan`, which inlines
  /// straight-line NUMERIC expressions and threads callee registers as
  /// `f64` SSA values. An accessor like `get_hash()` is neither: it reads a
  /// field of unknown type, compares it against `nil`, and branches. So
  /// this works entirely in tagged `Value` bits and permits forward
  /// branches, at the cost of admitting no arithmetic at all. Keeping the
  /// two apart is also what stops this from perturbing the numeric
  /// inlining `spectral-norm`'s `eval_A` depends on.
  ///
  /// The rules hold up three guarantees the emitter depends on:
  ///
  /// - **Cannot allocate, cannot call.** Every admitted opcode loads a
  ///   scalar constant, copies a register, reads a field slot the class
  ///   guard already proved, or compares against `nil`. None has a slow
  ///   path, so no safepoint is owed and no collection can happen mid-body.
  /// - **No back edges.** Every jump goes strictly forward, so the body is
  ///   a DAG the emitter walks by cloning register state down each path.
  /// - **Every path returns.** Nothing falls out of the body, which is what
  ///   removes the need for phi nodes over callee registers; the only merge
  ///   is the single result block.
  fn invoke_inline_target(&self, method_const: u16, num_args: u8) -> Option<&'static ObjFunction> {
    let name = self.proto.chunk.constants[method_const as usize];
    if !name.is_string() {
      return None;
    }
    let proto_ptr = *self.self_method_protos.get(name.as_str())?;
    // SAFETY: `resolve_self_method_protos` read this off a live class on
    // the VM thread, and an `ObjFunction` never moves or is freed while a
    // compile referencing it is in flight; the same guarantee
    // `try_emit_inlined_call` relies on for its own `proto_ptr`.
    let callee: &'static ObjFunction = unsafe { &*(proto_ptr as *const ObjFunction) };

    // The receiver occupies the callee's own register 0, so the real
    // argument count is `1 + num_args`.
    if callee.variadic
      || callee.arity as usize != num_args as usize + 1
      || !callee.upvalues.is_empty()
      || callee.chunk.code.len() > Self::MAX_INVOKE_INLINE_OPS
    {
      return None;
    }

    if self.invoke_inline_eligible(callee) {
      Some(callee)
    } else {
      if crate::jit::log_enabled() {
        eprintln!(
          "[jit] invoke-inline '{}' rejected: unsupported body",
          callee.name
        );
      }
      None
    }
  }

  /// The opcode/CFG half of `invoke_inline_target`'s judgement.
  fn invoke_inline_eligible(&self, callee: &ObjFunction) -> bool {
    let code = &callee.chunk.code;
    if !matches!(code.last(), Some(Instr::Return { .. })) {
      return false;
    }

    // Collected first so the nil-comparison rule below can insist its
    // `LoadNil` really does reach the compare, rather than merely sitting
    // in front of it in the listing.
    let mut is_target = vec![false; code.len()];
    for (i, instr) in code.iter().enumerate() {
      let offset = match *instr {
        Instr::Jmp { offset } | Instr::JmpIfFalse { offset, .. } => offset,
        _ => continue,
      };
      // Offsets are signed: a negative one is a loop back edge, which this
      // inliner deliberately does not handle (it emits no safepoint, and
      // its path-cloning walk assumes a DAG). Rejecting it here also keeps
      // the `as usize` below from sign-extending into a huge value.
      if offset < 0 {
        return false;
      }
      let target = i + 1 + offset as usize;
      if target <= i || target >= code.len() {
        return false;
      }
      is_target[target] = true;
    }

    // Registers proven to hold a bool, so a `JmpIfFalse` on one is a plain
    // bit test rather than Zuri's general truthiness (under which a
    // negative number is falsy; getting that wrong would silently change
    // behaviour).
    let mut is_bool = vec![false; callee.num_registers as usize];
    // Registers holding a number known at compile time, which is what lets
    // `Neg` be folded rather than needing a numeric guard of its own.
    // `return -1` compiles to `LoadConst 1; Neg`, so without this every
    // accessor with a negative literal would be turned away.
    let mut const_num = vec![false; callee.num_registers as usize];
    let mut forks = 0u32;

    for (i, instr) in code.iter().enumerate() {
      match *instr {
        Instr::Return { .. } | Instr::Jmp { .. } => {},
        Instr::LoadNil { dst } => {
          let Some(b) = is_bool.get_mut(dst as usize) else {
            return false;
          };
          *b = false;
          const_num[dst as usize] = false;
        },
        Instr::LoadConst { dst, const_idx } => {
          // A heap constant's bits are a pointer into the young
          // generation, which relocates; only scalars are safe to bake.
          let c = callee.chunk.constants[const_idx as usize];
          if c.is_obj() {
            return false;
          }
          let Some(b) = is_bool.get_mut(dst as usize) else {
            return false;
          };
          *b = c.is_bool();
          const_num[dst as usize] = c.is_number();
        },
        Instr::LoadBool { dst, .. } => {
          let Some(b) = is_bool.get_mut(dst as usize) else {
            return false;
          };
          *b = true;
          const_num[dst as usize] = false;
        },
        Instr::Move { dst, src } => {
          let v = *is_bool.get(src as usize).unwrap_or(&false);
          let n = const_num[src as usize];
          let Some(b) = is_bool.get_mut(dst as usize) else {
            return false;
          };
          *b = v;
          const_num[dst as usize] = n;
        },
        // Folded at compile time, so no numeric guard and no slow path.
        Instr::Neg { dst, src } => {
          if !const_num[src as usize] {
            return false;
          }
          const_num[dst as usize] = true;
          is_bool[dst as usize] = false;
        },
        // Only against `nil`, where tagged equality is a bit compare. Any
        // other pair would need `Value::equals`, which is a call. The
        // `LoadNil` has to be the instruction immediately before, and that
        // instruction must not be reachable by a jump, so it genuinely
        // dominates this compare on every path.
        Instr::Eq { dst, a, b } | Instr::Neq { dst, a, b } => {
          if i == 0 || is_target[i] || is_target[i - 1] {
            return false;
          }
          let nil_reg = match code[i - 1] {
            Instr::LoadNil { dst: n } => n,
            _ => return false,
          };
          if nil_reg != a && nil_reg != b {
            return false;
          }
          let Some(flag) = is_bool.get_mut(dst as usize) else {
            return false;
          };
          *flag = true;
          const_num[dst as usize] = false;
        },
        // Reading a field of the receiver, whose class the call site's
        // guard has already pinned to the compiling function's own.
        Instr::GetField {
          dst,
          obj,
          name_const,
        } => {
          if obj != 0 {
            return false;
          }
          let name = callee.chunk.constants[name_const as usize];
          if !name.is_string() || !self.self_field_slots.contains_key(name.as_str()) {
            return false;
          }
          let Some(b) = is_bool.get_mut(dst as usize) else {
            return false;
          };
          *b = false;
          const_num[dst as usize] = false;
        },
        Instr::JmpIfFalse { cond, .. } => {
          if !*is_bool.get(cond as usize).unwrap_or(&false) {
            return false;
          }
          forks += 1;
          if forks > Self::MAX_INVOKE_INLINE_FORKS {
            return false;
          }
        },
        _ => return false,
      }
    }

    true
  }

  /// `Instr::Invoke` emitted as the callee's own body, behind the same
  /// receiver-class + method-table-generation guard `emit_self_invoke`
  /// uses. See `invoke_inline_target` for what makes a callee eligible.
  ///
  /// The guard is what licenses everything the body then assumes: that the
  /// receiver is an `Obj::Instance`, that its class resolves this name to
  /// exactly `callee`, and that `self_field_slots` describes its layout. A
  /// miss falls through to the ordinary helper path, which resolves for
  /// real.
  #[allow(clippy::too_many_arguments)]
  fn emit_invoke_inline(
    &mut self,
    ip: usize,
    dst: u8,
    obj: u8,
    method_const: u16,
    num_args: u8,
    class_bits: u64,
    generation: u64,
    callee: &'static ObjFunction,
  ) {
    if crate::jit::log_enabled() {
      eprintln!(
        "[jit] invoke-inlined '{}' ({} ops) into '{}' at ip {}",
        callee.name,
        callee.chunk.code.len(),
        self.proto.name,
        ip
      );
    }

    let vm_p = self.vm_param;
    // The bytecode compiler already duplicated the receiver into `obj + 1`,
    // which is the callee's own register 0.
    let receiver = self.load_reg(obj + 1);

    let slow_block = self.fb.create_block();
    let done_block = self.fb.create_block();
    self.fb.append_block_param(done_block, types::I64);

    let obj_block = self.fb.create_block();
    let class_check_block = self.fb.create_block();
    let is_obj = self.is_obj(receiver);
    self.fb.ins().brif(is_obj, obj_block, &[], slow_block, &[]);

    self.fb.switch_to_block(obj_block);
    let ptr = self.obj_ptr(receiver);
    let tag = self.obj_tag(ptr);
    let tag_instance = self.i64c(object::OBJ_TAG_INSTANCE as i64);
    let is_instance = self.fb.ins().icmp(IntCC::Equal, tag, tag_instance);
    self
      .fb
      .ins()
      .brif(is_instance, class_check_block, &[], slow_block, &[]);

    self.fb.switch_to_block(class_check_block);
    let flags = cranelift_codegen::ir::MemFlagsData::trusted();
    let class_off = object::obj_instance_class_offset() as i32;
    let class_val = self.fb.ins().load(types::I64, flags, ptr, class_off);
    let target_class = self.u64c(class_bits);
    let class_hit = self.fb.ins().icmp(IntCC::Equal, class_val, target_class);
    let gen_check_block = self.fb.create_block();
    self
      .fb
      .ins()
      .brif(class_hit, gen_check_block, &[], slow_block, &[]);

    // A class can be monkey-patched after the fact; without this the baked
    // method would outlive the resolution that justified it. Same reasoning
    // as `emit_self_invoke`'s own generation check.
    self.fb.switch_to_block(gen_check_block);
    let cur_gen = self
      .fb
      .ins()
      .load(types::I64, flags, vm_p, METHOD_TABLE_GENERATION_OFFSET);
    let target_gen = self.u64c(generation);
    let gen_hit = self.fb.ins().icmp(IntCC::Equal, cur_gen, target_gen);
    let body_block = self.fb.create_block();
    self
      .fb
      .ins()
      .brif(gen_hit, body_block, &[], slow_block, &[]);

    // Computed once here, in a block that dominates every path the body
    // forks into.
    self.fb.switch_to_block(body_block);
    let fields_ptr = self.load_instance_fields_ptr(ptr);

    let mut regs: Vec<IrValue> = Vec::with_capacity(callee.num_registers as usize);
    let nil = self.u64c(crate::vm::value::NIL_VAL);
    regs.resize(callee.num_registers as usize, nil);
    regs[0] = receiver;
    for i in 0..num_args {
      regs[1 + i as usize] = self.load_reg(obj + 2 + i);
    }

    let consts = vec![None; callee.num_registers as usize];
    self.emit_invoke_inline_path(callee, fields_ptr, regs, consts, 0, done_block);

    self.fb.switch_to_block(slow_block);
    self.emit_invoke_helper(ip, dst, obj, method_const, num_args);
    let slow_result = self.load_reg(dst);
    self.fb.ins().jump(done_block, &[slow_result.into()]);

    self.fb.switch_to_block(done_block);
    let result = self.fb.block_params(done_block)[0];
    self.store_reg(dst, result);
  }

  /// Emits one straight-line path of an invoke-inlined body, forking into
  /// two recursive calls at each `JmpIfFalse`. Register state is a plain
  /// `Vec` of tagged `Value` bits cloned down each fork, which is sound
  /// precisely because `invoke_inline_eligible` guarantees paths never
  /// rejoin: nothing merges, so nothing needs a phi.
  fn emit_invoke_inline_path(
    &mut self,
    callee: &'static ObjFunction,
    fields_ptr: IrValue,
    mut regs: Vec<IrValue>,
    mut consts: Vec<Option<f64>>,
    mut pc: usize,
    done_block: Block,
  ) {
    let flags = cranelift_codegen::ir::MemFlagsData::trusted();

    loop {
      match callee.chunk.code[pc] {
        Instr::Return { src } => {
          let v = regs[src as usize];
          self.fb.ins().jump(done_block, &[v.into()]);
          return;
        },
        Instr::LoadNil { dst } => {
          regs[dst as usize] = self.u64c(crate::vm::value::NIL_VAL);
          consts[dst as usize] = None;
        },
        Instr::LoadBool { dst, val } => {
          let bits = if val {
            crate::vm::value::TRUE_VAL
          } else {
            crate::vm::value::FALSE_VAL
          };
          regs[dst as usize] = self.u64c(bits);
          consts[dst as usize] = None;
        },
        Instr::LoadConst { dst, const_idx } => {
          let c = callee.chunk.constants[const_idx as usize];
          regs[dst as usize] = self.u64c(c.to_bits());
          consts[dst as usize] = c.is_number().then(|| c.as_number());
        },
        Instr::Move { dst, src } => {
          regs[dst as usize] = regs[src as usize];
          consts[dst as usize] = consts[src as usize];
        },
        // `invoke_inline_eligible` only admits this on a register it
        // already proved holds a compile-time number, so the negation
        // happens here rather than at runtime.
        Instr::Neg { dst, src } => {
          let n = -consts[src as usize].expect("invoke_inline_eligible proved this constant");
          regs[dst as usize] = self.u64c(crate::vm::value::Value::number(n).to_bits());
          consts[dst as usize] = Some(n);
        },
        Instr::GetField {
          dst, name_const, ..
        } => {
          let name = callee.chunk.constants[name_const as usize];
          let slot = self.self_field_slots[name.as_str()];
          regs[dst as usize] = self
            .fb
            .ins()
            .load(types::I64, flags, fields_ptr, (slot as i32) * 8);
          consts[dst as usize] = None;
        },
        Instr::Eq { dst, a, b } | Instr::Neq { dst, a, b } => {
          let negate = matches!(callee.chunk.code[pc], Instr::Neq { .. });
          let cc = if negate {
            IntCC::NotEqual
          } else {
            IntCC::Equal
          };
          let cmp = self.fb.ins().icmp(cc, regs[a as usize], regs[b as usize]);
          let t = self.u64c(crate::vm::value::TRUE_VAL);
          let f = self.u64c(crate::vm::value::FALSE_VAL);
          regs[dst as usize] = self.fb.ins().select(cmp, t, f);
          consts[dst as usize] = None;
        },
        Instr::Jmp { offset } => {
          // `invoke_inline_eligible` already rejected every negative offset.
          debug_assert!(offset >= 0, "invoke inliner reached a backward jump");
          pc = pc + 1 + offset as usize;
          continue;
        },
        Instr::JmpIfFalse { cond, offset } => {
          debug_assert!(offset >= 0, "invoke inliner reached a backward jump");
          let taken = pc + 1 + offset as usize;
          let fallthrough = pc + 1;

          let true_block = self.fb.create_block();
          let false_block = self.fb.create_block();
          let t = self.u64c(crate::vm::value::TRUE_VAL);
          // `cond` is proven bool, so this is a bit test, not Zuri
          // truthiness.
          let is_true = self.fb.ins().icmp(IntCC::Equal, regs[cond as usize], t);
          self
            .fb
            .ins()
            .brif(is_true, true_block, &[], false_block, &[]);

          self.fb.switch_to_block(true_block);
          self.emit_invoke_inline_path(
            callee,
            fields_ptr,
            regs.clone(),
            consts.clone(),
            fallthrough,
            done_block,
          );

          self.fb.switch_to_block(false_block);
          self.emit_invoke_inline_path(callee, fields_ptr, regs, consts, taken, done_block);
          return;
        },
        _ => unreachable!("invoke_inline_eligible admitted an unsupported instruction"),
      }
      pc += 1;
    }
  }

  /// `Instr::Invoke` when `typeflow::StringFacts` proves `obj` is
  /// ALWAYS a `Value::String` at this call site.
  ///
  /// `emit_generic_invoke`'s own `zuri_jit_invoke_prepare` attempt
  /// exists to catch a compiled user-class method (`receiver.
  /// is_instance()`, checked as that helper's very first line) --
  /// something a String can structurally never be. Trying it anyway on
  /// a proven-string receiver is a real, always-wasted call: a second
  /// full C call into `jit::runtime`, argument marshaling included, for
  /// an answer already known at compile time. This skips straight to
  /// `zuri_jit_invoke_string`, the lean runtime entry point that starts
  /// exactly where `zuri_jit_invoke` ends up after its own
  /// is_instance/is_class/is_module chain concludes "none of those, try
  /// a builtin method"; same per-call-site `InvokeCacheCell` (keyed
  /// by `builtins::method_table_key`, so every call past the first
  /// skips `builtins::lookup`'s hash+memcmp too), just reached without
  /// the two dead branches and the wasted prepare call in front of it.
  fn emit_string_invoke(&mut self, ip: usize, dst: u8, obj: u8, method_const: u16, num_args: u8) {
    let base = self.base_param;
    let vm_p = self.vm_param;
    let obj_i = self.idx(obj);
    let num_args_i = self.idx(num_args);
    let dst_i = self.idx(dst);
    let name = self.bake_const(method_const);
    let cache = self.invoke_cache_addr(ip);
    self.call_checked(
      "zuri_jit_invoke_string",
      &[vm_p, base, obj_i, num_args_i, dst_i, name, cache],
    );
    self.resync_dst_from_memory(dst);
  }

  /// The method name an `Instr::Invoke` names, read straight out of the
  /// constant table at compile time; a compile-time lookup only, like
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
    let arg_reg = obj + 2;
    if matches!(op, NumberIntrinsic::Max | NumberIntrinsic::Min)
      && num_args == 1
      && self.both_proven_int(ip, obj, arg_reg)
    {
      let ia = self.fb.use_var(self.reg_vars_int[obj as usize]);
      let ib = self.fb.use_var(self.reg_vars_int[arg_reg as usize]);
      let cc = match op {
        NumberIntrinsic::Max => IntCC::SignedGreaterThan,
        NumberIntrinsic::Min => IntCC::SignedLessThan,
        _ => unreachable!(),
      };
      let cmp = self.fb.ins().icmp(cc, ia, ib);
      let ir = self.fb.ins().select(cmp, ia, ib);
      self.store_reg_int(dst, ir);
      return;
    }

    let recv = self.load_reg(obj);
    let arg = (num_args == 1).then(|| self.load_reg(arg_reg));

    let seeds_int = ip + 1 < self.proto.chunk.code.len() && self.int_facts.is_int(ip + 1, dst);

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
      if seeds_int {
        let f = self.to_f64(v);
        let iv = self.fb.ins().fcvt_to_sint_sat(types::I64, f);
        self.fb.def_var(self.reg_vars_int[dst as usize], iv);
      }
      return;
    }

    let mut guard = self.is_number(recv);
    if let Some(a) = arg {
      let arg_is_num = self.is_number(a);
      guard = self.fb.ins().band(guard, arg_is_num);
    }

    let fast_block = self.fb.create_block();
    let slow_block = self.fb.create_block();
    let done_block = self.fb.create_block();
    self.fb.ins().brif(guard, fast_block, &[], slow_block, &[]);

    self.fb.switch_to_block(fast_block);
    let v = self.emit_intrinsic_value(op, recv, arg);
    self.store_reg(dst, v);
    if seeds_int {
      let f = self.to_f64(v);
      let iv = self.fb.ins().fcvt_to_sint_sat(types::I64, f);
      self.fb.def_var(self.reg_vars_int[dst as usize], iv);
    }
    self.fb.ins().jump(done_block, &[]);

    // The full ordinary dispatch, safepoint included, for a receiver
    // that turned out not to be a number after all; a string, a list,
    // an instance whose class happens to declare a method by this name.
    self.fb.switch_to_block(slow_block);
    self.emit_safepoint();
    self.emit_generic_invoke(ip, dst, obj, method_const, num_args);
    // Both arms must leave the same story behind: the fast arm defines
    // `dst` in its `Variable` and writes no memory, while everything on
    // the slow arm flushed and stale-marked both `dst` and the
    // receiver. Re-reading them here makes the `Variable` authoritative
    // either way, so the merged `Dirty` state below is honest: see
    // `resync_receiver_from_memory`'s own docs for the bug the
    // alternative produces.
    self.resync_dst_from_memory(dst);
    self.resync_receiver_from_memory(obj);
    if seeds_int {
      let v = self.load_reg(dst);
      let f = self.to_f64(v);
      let iv = self.fb.ins().fcvt_to_sint_sat(types::I64, f);
      self.fb.def_var(self.reg_vars_int[dst as usize], iv);
    }
    self.fb.ins().jump(done_block, &[]);

    self.fb.switch_to_block(done_block);
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
      NumberIntrinsic::Max => {
        let f1 = self.to_f64(recv);
        let f2 = self.to_f64(arg.expect("max takes 1 arg"));
        let r = self.fb.ins().fmax(f1, f2);
        self.from_f64(r)
      },
      NumberIntrinsic::Min => {
        let f1 = self.to_f64(recv);
        let f2 = self.to_f64(arg.expect("min takes 1 arg"));
        let r = self.fb.ins().fmin(f1, f2);
        self.from_f64(r)
      },
      NumberIntrinsic::Round => {
        let f = self.to_f64(recv);
        let half = self.fb.ins().f64const(0.5);
        let abs_f = self.fb.ins().fabs(f);
        let sum = self.fb.ins().fadd(abs_f, half);
        let fl = self.fb.ins().floor(sum);
        let r = self.fb.ins().fcopysign(fl, f);
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

  /// `NumberIntrinsic`'s counterpart for a List receiver: see
  /// `ListIntrinsic`'s own docs. `typeflow::ListFacts` plays exactly
  /// the role `proven_numeric` plays there: when `obj` is already
  /// proven a list at `ip`, there is nothing left to guard, so the
  /// whole call collapses to the couple of loads `emit_list_intrinsic_
  /// value` computes, no branch at all.
  fn emit_list_append(&mut self, ip: usize, dst: u8, obj: u8, method_const: u16) {
    let recv = self.load_reg(obj);
    let item = self.load_reg(obj + 2);

    let slow_block = self.fb.create_block();
    let done_block = self.fb.create_block();
    let push_block = self.fb.create_block();

    let ptr = if self.proven_list(ip, obj) {
      let p = self.obj_ptr(recv);
      self.fb.ins().jump(push_block, &[]);
      p
    } else {
      let is_obj = self.is_obj(recv);
      let checked_block = self.fb.create_block();
      self
        .fb
        .ins()
        .brif(is_obj, checked_block, &[], slow_block, &[]);

      self.fb.switch_to_block(checked_block);
      let ptr = self.obj_ptr(recv);
      let tag = self.obj_tag(ptr);
      let tag_list = self.i64c(object::OBJ_TAG_LIST as i64);
      let is_list = self.fb.ins().icmp(IntCC::Equal, tag, tag_list);
      self
        .fb
        .ins()
        .brif(is_list, push_block, &[], slow_block, &[]);
      ptr
    };

    self.fb.switch_to_block(push_block);
    let flags = cranelift_codegen::ir::MemFlagsData::trusted();
    let len32 = self
      .fb
      .ins()
      .load(types::I32, flags, ptr, object::obj_list_len_offset());
    let len = self.fb.ins().uextend(types::I64, len32);
    let heap_ptr = self
      .fb
      .ins()
      .load(types::I64, flags, ptr, object::obj_list_ptr_offset());
    let cap32 = self
      .fb
      .ins()
      .load(types::I32, flags, ptr, object::obj_list_cap_offset());
    let cap = self.fb.ins().uextend(types::I64, cap32);
    let inline_cap = self.i64c(crate::vm::list::INLINE_CAP as i64);
    let zero = self.i64c(0);
    let is_inline = self.fb.ins().icmp(IntCC::Equal, heap_ptr, zero);
    let eff_cap = self.fb.ins().select(is_inline, inline_cap, cap);
    let can_push = self.fb.ins().icmp(IntCC::UnsignedLessThan, len, eff_cap);

    let fast_block = self.fb.create_block();
    self
      .fb
      .ins()
      .brif(can_push, fast_block, &[], slow_block, &[]);

    self.fb.switch_to_block(fast_block);
    let inline_ptr = self
      .fb
      .ins()
      .iadd_imm_s(ptr, object::obj_list_inline_offset() as i64);
    let data_ptr = self.fb.ins().select(is_inline, inline_ptr, heap_ptr);
    let byte_off = self.fb.ins().imul_imm_s(len, 8);
    let elem_addr = self.fb.ins().iadd(data_ptr, byte_off);
    self.fb.ins().store(flags, item, elem_addr, 0);
    let new_len = self.fb.ins().iadd_imm_s(len, 1);
    let new_len32 = self.fb.ins().ireduce(types::I32, new_len);
    self
      .fb
      .ins()
      .store(flags, new_len32, ptr, object::obj_list_len_offset());
    self.emit_write_barrier_for_store(ip, obj + 2, item, ptr);
    let nil = self.u64c(value::NIL_VAL);
    self.store_reg(dst, nil);
    self.fb.ins().jump(done_block, &[]);

    self.fb.switch_to_block(slow_block);
    self.emit_safepoint();
    self.emit_generic_invoke(ip, dst, obj, method_const, 1);
    self.resync_dst_from_memory(dst);
    self.resync_receiver_from_memory(obj);
    self.fb.ins().jump(done_block, &[]);

    self.fb.switch_to_block(done_block);
  }

  /// `Instr::Invoke` of a `DictIntrinsic`, laid out the same way
  /// `emit_list_intrinsic` is: guard the receiver's tag, answer inline
  /// on the fast path, fall back to the generic invoke otherwise.
  ///
  /// No `emit_safepoint`, for the reason `emit_list_intrinsic` gives:
  /// nothing on the fast path can allocate. That is also why the
  /// helper goes through `call_helper_raw`; it cannot collect or
  /// re-enter the VM, so the register flush an ordinary helper call
  /// pays for would be waste, and skipping it is most of the point.
  fn emit_dict_intrinsic(
    &mut self,
    ip: usize,
    dst: u8,
    obj: u8,
    method_const: u16,
    num_args: u8,
    op: DictIntrinsic,
  ) {
    let recv = self.load_reg(obj);

    let is_obj = self.is_obj(recv);
    let checked_block = self.fb.create_block();
    let fast_block = self.fb.create_block();
    let slow_block = self.fb.create_block();
    let done_block = self.fb.create_block();

    // `var counts = {}` proves itself through `MakeDict`, so the
    // overwhelmingly common shape (build a dict, then update it in a
    // loop) pays no tag check at all.
    if self.proven_dict(ip, obj) {
      self.fb.ins().jump(fast_block, &[]);
      self.fb.switch_to_block(fast_block);
      let vm_p = self.vm_param;
      let v = self.emit_dict_intrinsic_value(op, num_args, recv, obj, vm_p);
      self.store_reg(dst, v);
      self.fb.ins().jump(done_block, &[]);

      self.fb.switch_to_block(slow_block);
      self.fb.ins().jump(done_block, &[]);

      self.fb.switch_to_block(done_block);
      return;
    }

    // A receiver already proven a list or a byte stream cannot be the
    // dict this emitter is named for, so it skips the whole tag chain
    // and goes straight to the arm that can answer it.
    if matches!(op, DictIntrinsic::Get) && (self.proven_list(ip, obj) || self.proven_bytes(ip, obj))
    {
      let ptr = self.obj_ptr(recv);
      let tag = self.obj_tag(ptr);
      let pl = self.proven_list(ip, obj);
      let pb = self.proven_bytes(ip, obj);
      self.emit_sequence_get(dst, obj, num_args, ptr, tag, pl, pb, slow_block, done_block);

      self.fb.switch_to_block(slow_block);
      self.emit_safepoint();
      self.emit_generic_invoke(ip, dst, obj, method_const, num_args);
      self.resync_dst_from_memory(dst);
      self.resync_receiver_from_memory(obj);
      self.fb.ins().jump(done_block, &[]);

      self.fb.switch_to_block(done_block);
      return;
    }

    self
      .fb
      .ins()
      .brif(is_obj, checked_block, &[], slow_block, &[]);

    self.fb.switch_to_block(checked_block);
    let ptr = self.obj_ptr(recv);
    let tag = self.obj_tag(ptr);
    let tag_dict = self.i64c(object::OBJ_TAG_DICT as i64);
    let is_dict = self.fb.ins().icmp(IntCC::Equal, tag, tag_dict);

    // `get` is not a dict-only method: lists and byte streams answer it
    // too, so a non-dict receiver gets one more chance before the
    // generic path.
    let sequence_block = if matches!(op, DictIntrinsic::Get) {
      let b = self.fb.create_block();
      self.fb.ins().brif(is_dict, fast_block, &[], b, &[]);
      Some(b)
    } else {
      self
        .fb
        .ins()
        .brif(is_dict, fast_block, &[], slow_block, &[]);
      None
    };

    if let Some(seq_block) = sequence_block {
      self.fb.switch_to_block(seq_block);
      let pl = self.proven_list(ip, obj);
      let pb = self.proven_bytes(ip, obj);
      self.emit_sequence_get(dst, obj, num_args, ptr, tag, pl, pb, slow_block, done_block);
    }

    self.fb.switch_to_block(fast_block);
    let vm_p = self.vm_param;
    let v = self.emit_dict_intrinsic_value(op, num_args, recv, obj, vm_p);
    self.store_reg(dst, v);
    self.fb.ins().jump(done_block, &[]);

    self.fb.switch_to_block(slow_block);
    self.emit_safepoint();
    self.emit_generic_invoke(ip, dst, obj, method_const, num_args);
    self.resync_dst_from_memory(dst);
    self.resync_receiver_from_memory(obj);
    self.fb.ins().jump(done_block, &[]);

    self.fb.switch_to_block(done_block);
  }

  /// The dict answer itself, once the receiver is known to be one.
  fn emit_dict_intrinsic_value(
    &mut self,
    op: DictIntrinsic,
    num_args: u8,
    recv: IrValue,
    obj: u8,
    vm_p: IrValue,
  ) -> IrValue {
    match op {
      DictIntrinsic::Get => {
        let key = self.load_reg(obj + 2);
        // The no-fallback form is documented to yield nil on a miss,
        // which is exactly a fallback of nil.
        let fallback = if num_args == 2 {
          self.load_reg(obj + 3)
        } else {
          self.u64c(value::NIL_VAL)
        };
        self.call_helper_raw("zuri_jit_dict_get", &[vm_p, recv, key, fallback])
      },
      DictIntrinsic::Set => {
        let key = self.load_reg(obj + 2);
        let value = self.load_reg(obj + 3);
        self.call_helper_raw("zuri_jit_dict_set", &[vm_p, recv, key, value])
      },
      DictIntrinsic::Contains => {
        let key = self.load_reg(obj + 2);
        self.call_helper_raw("zuri_jit_dict_contains", &[vm_p, recv, key])
      },
    }
  }

  /// `l.get(i)` / `l.get(i, fallback)` on a list, and `b.get(i)` on a
  /// byte stream; the non-dict half of `emit_dict_intrinsic`'s `get`.
  ///
  /// Only the cases whose answer is unambiguous are inlined. A hit
  /// returns the element. A miss returns the fallback, but only in the
  /// two-argument form: with no fallback both kinds RAISE, and the
  /// message is theirs to phrase, so that goes to the generic path.
  /// So does a non-numeric index, which is a `TypeError` rather than a
  /// miss, and `b.get(i, fallback)`, which byte streams do not accept
  /// at all.
  ///
  /// Note the index rule these two share and `Instr::GetIndex` does
  /// not: `get` has NO negative indexing. A negative index is simply
  /// out of range, and the fraction of a non-integral one is dropped
  /// rather than rejected, matching `builtins::list::get`'s own
  /// `idx < 0.0 || idx as usize >= len`.
  fn emit_sequence_get(
    &mut self,
    dst: u8,
    obj: u8,
    num_args: u8,
    ptr: IrValue,
    tag: IrValue,
    proven_list: bool,
    proven_bytes: bool,
    slow_block: cranelift_codegen::ir::Block,
    done_block: cranelift_codegen::ir::Block,
  ) {
    let key = self.load_reg(obj + 2);
    let is_num = self.is_number(key);
    let numeric_block = self.fb.create_block();
    self
      .fb
      .ins()
      .brif(is_num, numeric_block, &[], slow_block, &[]);

    self.fb.switch_to_block(numeric_block);
    let f = self.to_f64(key);
    let zero_f = self.fb.ins().f64const(0.0);
    let is_neg = self.fb.ins().fcmp(
      cranelift_codegen::ir::condcodes::FloatCC::LessThan,
      f,
      zero_f,
    );
    let idx = self.fb.ins().fcvt_to_sint_sat(types::I64, f);

    // With the receiver's kind already proven there is only ever one
    // arm worth emitting; the others are unreachable blocks that cost
    // register allocation across the whole function.
    let list_block = self.fb.create_block();
    let check_bytes_block = self.fb.create_block();
    match (proven_list, proven_bytes) {
      (true, _) => {
        self.fb.ins().jump(list_block, &[]);
      },
      (_, true) => {
        self.fb.ins().jump(check_bytes_block, &[]);
      },
      _ => {
        let tag_list = self.i64c(object::OBJ_TAG_LIST as i64);
        let is_list = self.fb.ins().icmp(IntCC::Equal, tag, tag_list);
        self
          .fb
          .ins()
          .brif(is_list, list_block, &[], check_bytes_block, &[]);
      },
    }

    // A miss with a fallback answers with it; without one, the raise
    // belongs to the generic path.
    let miss_block = self.fb.create_block();

    self.fb.switch_to_block(list_block);
    let (data_ptr, len) = self.load_list_ptr_len(ptr);
    let in_range = self.fb.ins().icmp(IntCC::SignedLessThan, idx, len);
    // Negating the `< 0` test rather than asking `>= 0` directly, so a
    // NaN index keeps the meaning `builtins::list::get` gives it:
    // `NaN < 0.0` is false there too, and the cast lands on 0, so a
    // non-empty list answers with its first element. `>= 0.0` would be
    // false for NaN and turn that into a miss.
    let not_neg = self.fb.ins().bxor_imm_u(is_neg, 1);
    let ok = self.fb.ins().band(in_range, not_neg);
    let list_hit_block = self.fb.create_block();
    self.fb.ins().brif(ok, list_hit_block, &[], miss_block, &[]);

    self.fb.switch_to_block(list_hit_block);
    let eight = self.fb.ins().iconst(types::I64, 8);
    let off = self.fb.ins().imul(idx, eight);
    let addr = self.fb.ins().iadd(data_ptr, off);
    let elem = self.fb.ins().load(
      types::I64,
      cranelift_codegen::ir::MemFlagsData::trusted(),
      addr,
      0,
    );
    self.store_reg(dst, elem);
    self.fb.ins().jump(done_block, &[]);

    self.fb.switch_to_block(check_bytes_block);
    // `builtins::bytes::get` takes the index and nothing else, so the
    // two-argument form has to reach the generic path to be rejected.
    if num_args != 1 {
      self.fb.ins().jump(slow_block, &[]);
    } else {
      let bytes_block = self.fb.create_block();
      if proven_bytes {
        self.fb.ins().jump(bytes_block, &[]);
      } else {
        let tag_bytes = self.i64c(object::OBJ_TAG_BYTES as i64);
        let is_bytes = self.fb.ins().icmp(IntCC::Equal, tag, tag_bytes);
        self
          .fb
          .ins()
          .brif(is_bytes, bytes_block, &[], slow_block, &[]);
      }

      self.fb.switch_to_block(bytes_block);
      let flags = cranelift_codegen::ir::MemFlagsData::trusted();
      let blen = self
        .fb
        .ins()
        .load(types::I64, flags, ptr, object::obj_bytes_len_offset());
      let b_in_range = self.fb.ins().icmp(IntCC::SignedLessThan, idx, blen);
      let b_not_neg = self.fb.ins().bxor_imm_u(is_neg, 1);
      let b_ok = self.fb.ins().band(b_in_range, b_not_neg);
      let bytes_hit_block = self.fb.create_block();
      self
        .fb
        .ins()
        .brif(b_ok, bytes_hit_block, &[], miss_block, &[]);

      self.fb.switch_to_block(bytes_hit_block);
      let bdata = self
        .fb
        .ins()
        .load(types::I64, flags, ptr, object::obj_bytes_ptr_offset());
      let baddr = self.fb.ins().iadd(bdata, idx);
      let b8 = self.fb.ins().load(types::I8, flags, baddr, 0);
      let b64 = self.fb.ins().uextend(types::I64, b8);
      self.store_reg_int(dst, b64);
      self.fb.ins().jump(done_block, &[]);
    }

    self.fb.switch_to_block(miss_block);
    if num_args == 2 {
      let fallback = self.load_reg(obj + 3);
      self.store_reg(dst, fallback);
      self.fb.ins().jump(done_block, &[]);
    } else {
      self.fb.ins().jump(slow_block, &[]);
    }
  }

  fn emit_list_intrinsic(
    &mut self,
    ip: usize,
    dst: u8,
    obj: u8,
    method_const: u16,
    num_args: u8,
    op: ListIntrinsic,
  ) {
    // No `emit_safepoint` here either, for the identical reason
    // `emit_number_intrinsic` skips it: nothing below can allocate.
    let recv = self.load_reg(obj);

    if self.proven_list(ip, obj) {
      let v = self.emit_list_intrinsic_value(op, recv);
      self.store_reg(dst, v);
      return;
    }

    // A proven byte stream answers `length`/`is_empty` from one field,
    // with no guard and no chain. Worth its own arm rather than
    // falling into the tag chain below: those extra blocks are dead
    // weight on a call site that cannot reach them, and enough of them
    // measurably worsens register allocation for the whole function,
    // hot loop included.
    if matches!(op, ListIntrinsic::Length | ListIntrinsic::IsEmpty) {
      if self.proven_bytes(ip, obj) {
        let ptr = self.obj_ptr(recv);
        let flags = cranelift_codegen::ir::MemFlagsData::trusted();
        let blen = self
          .fb
          .ins()
          .load(types::I64, flags, ptr, object::obj_bytes_len_offset());
        let v = self.emit_count_value(op, blen);
        self.store_reg(dst, v);
        return;
      }
      if self.proven_dict(ip, obj) {
        let vm_p = self.vm_param;
        let dlen = self.call_helper_raw("zuri_jit_dict_len", &[vm_p, recv]);
        let v = self.emit_count_value(op, dlen);
        self.store_reg(dst, v);
        return;
      }
    }

    let is_obj = self.is_obj(recv);
    let checked_block = self.fb.create_block();
    let fast_block = self.fb.create_block();
    let slow_block = self.fb.create_block();
    let done_block = self.fb.create_block();
    self
      .fb
      .ins()
      .brif(is_obj, checked_block, &[], slow_block, &[]);

    // Tag load only happens once `is_obj` is known true; same
    // discipline `emit_list_get_index`'s own unproven arm uses.
    self.fb.switch_to_block(checked_block);
    let ptr = self.obj_ptr(recv);
    let tag = self.obj_tag(ptr);
    let tag_list = self.i64c(object::OBJ_TAG_LIST as i64);
    let is_list = self.fb.ins().icmp(IntCC::Equal, tag, tag_list);

    // `length`/`is_empty` are not list-only questions. Every container
    // answers them, and until this chain existed a bytes or dict
    // receiver fell all the way through to the generic invoke: three
    // and a half times the cost of the list answer, for a single field
    // read. The other variants really are list-shaped, so they keep
    // the plain two-way branch.
    let counts_any_container = matches!(op, ListIntrinsic::Length | ListIntrinsic::IsEmpty);
    let other_container_block = if counts_any_container {
      let b = self.fb.create_block();
      self.fb.ins().brif(is_list, fast_block, &[], b, &[]);
      Some(b)
    } else {
      self
        .fb
        .ins()
        .brif(is_list, fast_block, &[], slow_block, &[]);
      None
    };

    if let Some(other_block) = other_container_block {
      self.fb.switch_to_block(other_block);
      let flags = cranelift_codegen::ir::MemFlagsData::trusted();
      let check_dict_block = self.fb.create_block();

      let bytes_block = self.fb.create_block();
      let tag_bytes = self.i64c(object::OBJ_TAG_BYTES as i64);
      let is_bytes = self.fb.ins().icmp(IntCC::Equal, tag, tag_bytes);
      self
        .fb
        .ins()
        .brif(is_bytes, bytes_block, &[], check_dict_block, &[]);

      self.fb.switch_to_block(bytes_block);
      let blen = self
        .fb
        .ins()
        .load(types::I64, flags, ptr, object::obj_bytes_len_offset());
      let bv = self.emit_count_value(op, blen);
      self.store_reg(dst, bv);
      self.fb.ins().jump(done_block, &[]);

      self.fb.switch_to_block(check_dict_block);
      let dict_block = self.fb.create_block();
      let tag_dict = self.i64c(object::OBJ_TAG_DICT as i64);
      let is_dict = self.fb.ins().icmp(IntCC::Equal, tag, tag_dict);
      self
        .fb
        .ins()
        .brif(is_dict, dict_block, &[], slow_block, &[]);

      self.fb.switch_to_block(dict_block);
      let vm_p = self.vm_param;
      let dlen = self.call_helper_raw("zuri_jit_dict_len", &[vm_p, recv]);
      let dv = self.emit_count_value(op, dlen);
      self.store_reg(dst, dv);
      self.fb.ins().jump(done_block, &[]);
    }

    self.fb.switch_to_block(fast_block);
    let v = self.emit_list_intrinsic_value(op, recv);
    self.store_reg(dst, v);
    self.fb.ins().jump(done_block, &[]);

    // The full ordinary dispatch for a receiver that turned out not to
    // be a list; a string, a dict, an instance whose class happens
    // to declare a method by this name.
    self.fb.switch_to_block(slow_block);
    self.emit_safepoint();
    self.emit_generic_invoke(ip, dst, obj, method_const, num_args);
    self.resync_dst_from_memory(dst);
    self.resync_receiver_from_memory(obj);
    self.fb.ins().jump(done_block, &[]);

    self.fb.switch_to_block(done_block);
  }

  /// The intrinsic itself, on a receiver already known to be a list --
  /// no branching, no register bookkeeping, just the value. `recv`'s
  /// own bit pattern (not a re-derived one) is what `obj_ptr` masks,
  /// matching every other intrinsic/guard site in this file.
  /// Shapes an already-loaded element count into whichever of
  /// `length`/`is_empty` was asked for, so every container kind can
  /// share one place that knows what those two mean.
  fn emit_count_value(&mut self, op: ListIntrinsic, len: IrValue) -> IrValue {
    match op {
      ListIntrinsic::IsEmpty => {
        let zero = self.fb.ins().iconst(types::I64, 0);
        let is_empty = self.fb.ins().icmp(IntCC::Equal, len, zero);
        self.bool_value(is_empty)
      },
      _ => {
        let len_f = self.fb.ins().fcvt_from_sint(types::F64, len);
        self.from_f64(len_f)
      },
    }
  }

  fn emit_list_intrinsic_value(&mut self, op: ListIntrinsic, recv: IrValue) -> IrValue {
    let ptr = self.obj_ptr(recv);
    let (data_ptr, len) = self.load_list_ptr_len(ptr);
    match op {
      ListIntrinsic::Length => {
        let len_f = self.fb.ins().fcvt_from_sint(types::F64, len);
        self.from_f64(len_f)
      },
      ListIntrinsic::IsEmpty => {
        let zero = self.fb.ins().iconst(types::I64, 0);
        let is_empty = self.fb.ins().icmp(IntCC::Equal, len, zero);
        self.bool_value(is_empty)
      },
      // `select` won't do for either of these: its untaken operand is
      // still COMPUTED, and for an empty list that means loading
      // through an address before/at a buffer that may not have any
      // allocated capacity behind it at all (`Vec::new()`'s dangling
      // pointer); genuinely unsound, not just a wasted load. A real
      // branch is required so the load only happens when `len > 0`
      // actually holds.
      ListIntrinsic::First => {
        let zero = self.fb.ins().iconst(types::I64, 0);
        let has_elem = self.fb.ins().icmp(IntCC::SignedGreaterThan, len, zero);
        let elem_block = self.fb.create_block();
        let done_block = self.fb.create_block();
        self.fb.append_block_param(done_block, types::I64);
        let nil = self.u64c(value::NIL_VAL);
        self
          .fb
          .ins()
          .brif(has_elem, elem_block, &[], done_block, &[nil.into()]);

        self.fb.switch_to_block(elem_block);
        let elem0 = self.fb.ins().load(
          types::I64,
          cranelift_codegen::ir::MemFlagsData::trusted(),
          data_ptr,
          0,
        );
        self.fb.ins().jump(done_block, &[elem0.into()]);

        self.fb.switch_to_block(done_block);
        self.fb.block_params(done_block)[0]
      },
      ListIntrinsic::Last => {
        let zero = self.fb.ins().iconst(types::I64, 0);
        let has_elem = self.fb.ins().icmp(IntCC::SignedGreaterThan, len, zero);
        let elem_block = self.fb.create_block();
        let done_block = self.fb.create_block();
        self.fb.append_block_param(done_block, types::I64);
        let nil = self.u64c(value::NIL_VAL);
        self
          .fb
          .ins()
          .brif(has_elem, elem_block, &[], done_block, &[nil.into()]);

        self.fb.switch_to_block(elem_block);
        let last_idx = self.fb.ins().iadd_imm_s(len, -1);
        let byte_off = self.fb.ins().imul_imm_s(last_idx, 8);
        let elem_addr = self.fb.ins().iadd(data_ptr, byte_off);
        let elem_last = self.fb.ins().load(
          types::I64,
          cranelift_codegen::ir::MemFlagsData::trusted(),
          elem_addr,
          0,
        );
        self.fb.ins().jump(done_block, &[elem_last.into()]);

        self.fb.switch_to_block(done_block);
        self.fb.block_params(done_block)[0]
      },
      ListIntrinsic::Append => unreachable!(),
    }
  }

  /// `Instr::Invoke`'s fast path for a `StringIntrinsic` method
  /// (`length`/`is_empty`); `ListIntrinsic`'s counterpart, identical
  /// proven/unproven shape: skip straight to the value when
  /// `typeflow::StringFacts` already proves `obj` a string, otherwise
  /// guard on `is_obj`+tag before computing it, falling back to the
  /// full `emit_generic_invoke` dispatch for anything that turns out
  /// not to be a string after all.
  fn emit_string_intrinsic(
    &mut self,
    ip: usize,
    dst: u8,
    obj: u8,
    method_const: u16,
    num_args: u8,
    op: StringIntrinsic,
  ) {
    // `Ord` is the one variant that can still raise on a genuine string
    // receiver, so it owns its whole shape below instead of sharing the
    // no-fallback one the other two use.
    if op == StringIntrinsic::Ord {
      self.emit_string_ord(ip, dst, obj, method_const, num_args);
      return;
    }

    // No `emit_safepoint` on the fast arm: see `StringIntrinsic`'s
    // own docs: neither remaining variant allocates or can raise for a
    // genuine string receiver.
    let recv = self.load_reg(obj);

    if self.proven_string(ip, obj) {
      let v = self.emit_string_intrinsic_value(op, recv);
      self.store_reg(dst, v);
      return;
    }

    let is_obj = self.is_obj(recv);
    let checked_block = self.fb.create_block();
    let fast_block = self.fb.create_block();
    let slow_block = self.fb.create_block();
    let done_block = self.fb.create_block();
    self
      .fb
      .ins()
      .brif(is_obj, checked_block, &[], slow_block, &[]);

    self.fb.switch_to_block(checked_block);
    let ptr = self.obj_ptr(recv);
    let tag = self.obj_tag(ptr);
    let tag_str = self.i64c(object::OBJ_TAG_STR as i64);
    let is_str = self.fb.ins().icmp(IntCC::Equal, tag, tag_str);
    self.fb.ins().brif(is_str, fast_block, &[], slow_block, &[]);

    self.fb.switch_to_block(fast_block);
    let v = self.emit_string_intrinsic_value(op, recv);
    self.store_reg(dst, v);
    self.fb.ins().jump(done_block, &[]);

    self.fb.switch_to_block(slow_block);
    self.emit_safepoint();
    self.emit_generic_invoke(ip, dst, obj, method_const, num_args);
    self.resync_dst_from_memory(dst);
    self.resync_receiver_from_memory(obj);
    self.fb.ins().jump(done_block, &[]);

    self.fb.switch_to_block(done_block);
  }

  /// `s.ord()`, inlined down to a single byte load.
  ///
  /// The whole method rests on one property of UTF-8: a string whose
  /// BYTE length is 1 is necessarily a single ASCII character, because
  /// any codepoint above U+007F encodes as a lead byte plus at least one
  /// continuation byte. So `byte_len == 1` proves both halves of what
  /// `builtins::string::ord` checks the slow way (`chars().count() == 1`)
  /// and computes (`chars().nth(0)`), and the byte IS the codepoint.
  ///
  /// Everything else; the empty string, a multi-character string, and a
  /// genuine one-character non-ASCII string like `'é'`; fails the length
  /// test and goes to the real builtin, which raises or answers as it
  /// always did.
  fn emit_string_ord(&mut self, ip: usize, dst: u8, obj: u8, method_const: u16, num_args: u8) {
    let recv = self.load_reg(obj);

    let len_block = self.fb.create_block();
    let fast_block = self.fb.create_block();
    let slow_block = self.fb.create_block();
    let done_block = self.fb.create_block();

    let ptr = if self.proven_string(ip, obj) {
      let ptr = self.obj_ptr(recv);
      self.fb.ins().jump(len_block, &[]);
      ptr
    } else {
      let is_obj = self.is_obj(recv);
      let checked_block = self.fb.create_block();
      self
        .fb
        .ins()
        .brif(is_obj, checked_block, &[], slow_block, &[]);

      self.fb.switch_to_block(checked_block);
      let ptr = self.obj_ptr(recv);
      let tag = self.obj_tag(ptr);
      let tag_str = self.i64c(object::OBJ_TAG_STR as i64);
      let is_str = self.fb.ins().icmp(IntCC::Equal, tag, tag_str);
      self.fb.ins().brif(is_str, len_block, &[], slow_block, &[]);
      ptr
    };

    self.fb.switch_to_block(len_block);
    let flags = cranelift_codegen::ir::MemFlagsData::trusted();
    let byte_len = self
      .fb
      .ins()
      .load(types::I64, flags, ptr, object::obj_str_len_offset());
    let one = self.i64c(1);
    let single = self.fb.ins().icmp(IntCC::Equal, byte_len, one);
    self.fb.ins().brif(single, fast_block, &[], slow_block, &[]);

    self.fb.switch_to_block(fast_block);
    let data_ptr = self
      .fb
      .ins()
      .load(types::I64, flags, ptr, object::obj_str_ptr_offset());
    let byte = self.fb.ins().load(types::I8, flags, data_ptr, 0);
    let code = self.fb.ins().uextend(types::I64, byte);
    self.store_reg_int(dst, code);
    self.fb.ins().jump(done_block, &[]);

    self.fb.switch_to_block(slow_block);
    self.emit_safepoint();
    self.emit_generic_invoke(ip, dst, obj, method_const, num_args);
    self.resync_dst_from_memory(dst);
    self.resync_receiver_from_memory(obj);
    self.fb.ins().jump(done_block, &[]);

    self.fb.switch_to_block(done_block);
  }

  /// The intrinsic itself, on a receiver already known to be a string
  ///; no branching, no register bookkeeping, just the value.
  /// `is_empty` is one comparison off the string's raw byte length
  /// (`object::obj_str_len_offset`); `length` is a real loop over the
  /// string's raw bytes counting UTF-8 LEAD bytes (every byte that
  /// isn't a continuation byte, `0b10xxxxxx`), matching `builtins::
  /// string::length`'s `s.chars().count()` exactly; a valid UTF-8
  /// string has exactly one lead byte per codepoint by construction, so
  /// counting them is the same number `chars().count()` computes,
  /// without materializing a `Chars` iterator or decoding each
  /// codepoint's actual value.
  fn emit_string_intrinsic_value(&mut self, op: StringIntrinsic, recv: IrValue) -> IrValue {
    let ptr = self.obj_ptr(recv);
    let flags = cranelift_codegen::ir::MemFlagsData::trusted();
    let data_ptr = self
      .fb
      .ins()
      .load(types::I64, flags, ptr, object::obj_str_ptr_offset());
    let byte_len = self
      .fb
      .ins()
      .load(types::I64, flags, ptr, object::obj_str_len_offset());

    match op {
      // Diverted in `emit_string_intrinsic` before it ever gets here.
      StringIntrinsic::Ord => unreachable!(),
      StringIntrinsic::IsEmpty => {
        let zero = self.fb.ins().iconst(types::I64, 0);
        let is_empty = self.fb.ins().icmp(IntCC::Equal, byte_len, zero);
        self.bool_value(is_empty)
      },
      StringIntrinsic::Length => {
        // `i`/`count` are the loop's own carried state, threaded
        // through `header_block`'s params; Cranelift has no implicit
        // loop-variable storage, every iteration's values are real SSA
        // values passed forward explicitly, same discipline
        // `ListIntrinsic::First`/`Last` above use for their own single-
        // branch merges, just looped instead of taken once.
        let header_block = self.fb.create_block();
        self.fb.append_block_param(header_block, types::I64); // i
        self.fb.append_block_param(header_block, types::I64); // count
        let body_block = self.fb.create_block();
        let done_block = self.fb.create_block();
        self.fb.append_block_param(done_block, types::I64);

        let zero = self.fb.ins().iconst(types::I64, 0);
        self
          .fb
          .ins()
          .jump(header_block, &[zero.into(), zero.into()]);

        self.fb.switch_to_block(header_block);
        let i = self.fb.block_params(header_block)[0];
        let count = self.fb.block_params(header_block)[1];
        let more = self.fb.ins().icmp(IntCC::SignedLessThan, i, byte_len);
        self
          .fb
          .ins()
          .brif(more, body_block, &[], done_block, &[count.into()]);

        self.fb.switch_to_block(body_block);
        let byte_addr = self.fb.ins().iadd(data_ptr, i);
        let byte = self.fb.ins().load(types::I8, flags, byte_addr, 0);
        let masked = self.fb.ins().band_imm_u(byte, 0xC0);
        let cont_pattern = self.fb.ins().iconst(types::I8, 0x80);
        let is_continuation = self.fb.ins().icmp(IntCC::Equal, masked, cont_pattern);
        let zero64 = self.fb.ins().iconst(types::I64, 0);
        let one64 = self.fb.ins().iconst(types::I64, 1);
        let inc = self.fb.ins().select(is_continuation, zero64, one64);
        let count_next = self.fb.ins().iadd(count, inc);
        let i_next = self.fb.ins().iadd_imm_s(i, 1);
        self
          .fb
          .ins()
          .jump(header_block, &[i_next.into(), count_next.into()]);

        self.fb.switch_to_block(done_block);
        let count_final = self.fb.block_params(done_block)[0];
        let count_f = self.fb.ins().fcvt_from_sint(types::F64, count_final);
        self.from_f64(count_f)
      },
    }
  }

  fn emit_iterable_key_common(
    &mut self,
    dst: u8,
    arg: IrValue,
    len: IrValue,
    slow_block: Block,
    done_block: Block,
  ) {
    let zero = self.i64c(0);
    let nil_val = self.u64c(value::NIL_VAL);
    let zero_f64 = self.fb.ins().f64const(0.0);
    let zero_val = self.from_f64(zero_f64);

    let is_nil = self.fb.ins().icmp(IntCC::Equal, arg, nil_val);
    let is_num = self.is_number(arg);
    let valid_arg = self.fb.ins().bor(is_nil, is_num);
    let key_fast_block = self.fb.create_block();
    self
      .fb
      .ins()
      .brif(valid_arg, key_fast_block, &[], slow_block, &[]);

    self.fb.switch_to_block(key_fast_block);
    let len_zero = self.fb.ins().icmp(IntCC::Equal, len, zero);
    let nil_res = self.fb.ins().select(len_zero, nil_val, zero_val);

    let f = self.to_f64(arg);
    let idx = self.fb.ins().fcvt_to_sint_sat(types::I64, f);
    let limit = self.fb.ins().iadd_imm_s(len, -1);
    let in_range = self.fb.ins().icmp(IntCC::SignedLessThan, idx, limit);
    let ge_zero = self
      .fb
      .ins()
      .icmp(IntCC::SignedGreaterThanOrEqual, idx, zero);
    let valid_idx = self.fb.ins().band(ge_zero, in_range);
    let next_idx = self.fb.ins().iadd_imm_s(idx, 1);
    let next_f = self.fb.ins().fcvt_from_sint(types::F64, next_idx);
    let next_val = self.from_f64(next_f);
    let num_res = self.fb.ins().select(valid_idx, next_val, nil_val);

    let res = self.fb.ins().select(is_nil, nil_res, num_res);
    self.store_reg(dst, res);
    self.fb.ins().jump(done_block, &[]);
  }

  fn emit_iterable_list(
    &mut self,
    ip: usize,
    dst: u8,
    obj: u8,
    op: IterableIntrinsic,
    _recv: IrValue,
    arg: IrValue,
    ptr: IrValue,
    slow_block: Block,
    done_block: Block,
  ) {
    let (data_ptr, len) = self.load_list_ptr_len(ptr);
    match op {
      IterableIntrinsic::Key => {
        self.emit_iterable_key_common(dst, arg, len, slow_block, done_block);
      },
      IterableIntrinsic::Value => {
        let is_num = self.is_number(arg);
        let val_fast_block = self.fb.create_block();
        self
          .fb
          .ins()
          .brif(is_num, val_fast_block, &[], slow_block, &[]);

        self.fb.switch_to_block(val_fast_block);
        let zero_f64 = self.fb.ins().f64const(0.0);
        let f = self.to_f64(arg);
        let idx = self.fb.ins().fcvt_to_sint_sat(types::I64, f);
        let f_ge_zero = self.fb.ins().fcmp(FloatCC::GreaterThanOrEqual, f, zero_f64);
        let in_bounds = self.fb.ins().icmp(IntCC::UnsignedLessThan, idx, len);
        let ok_bounds = self.fb.ins().band(f_ge_zero, in_bounds);

        let load_block = self.fb.create_block();
        let nil_block = self.fb.create_block();
        self
          .fb
          .ins()
          .brif(ok_bounds, load_block, &[], nil_block, &[]);

        self.fb.switch_to_block(load_block);
        let byte_off = self.fb.ins().imul_imm_s(idx, 8);
        let elem_addr = self.fb.ins().iadd(data_ptr, byte_off);
        let elem = self.fb.ins().load(
          types::I64,
          cranelift_codegen::ir::MemFlagsData::trusted(),
          elem_addr,
          0,
        );
        self.store_reg(dst, elem);
        let seeds_int = self.int_tracked[dst as usize]
          && (self.proven_int_list(ip, obj)
            || (ip + 1 < self.proto.chunk.code.len() && self.int_facts.is_int(ip + 1, dst)));
        if seeds_int {
          let ef = self.to_f64(elem);
          let iv = self.fb.ins().fcvt_to_sint_sat(types::I64, ef);
          self.fb.def_var(self.reg_vars_int[dst as usize], iv);
        }
        self.fb.ins().jump(done_block, &[]);

        self.fb.switch_to_block(nil_block);
        let nil_val = self.u64c(value::NIL_VAL);
        self.store_reg(dst, nil_val);
        self.fb.ins().jump(done_block, &[]);
      },
    }
  }

  fn emit_iterable_bytes(
    &mut self,
    _ip: usize,
    dst: u8,
    _obj: u8,
    op: IterableIntrinsic,
    _recv: IrValue,
    arg: IrValue,
    ptr: IrValue,
    slow_block: Block,
    done_block: Block,
  ) {
    let flags = cranelift_codegen::ir::MemFlagsData::trusted();
    let bytes_data_ptr = self
      .fb
      .ins()
      .load(types::I64, flags, ptr, object::obj_bytes_ptr_offset());
    let bytes_len = self
      .fb
      .ins()
      .load(types::I64, flags, ptr, object::obj_bytes_len_offset());

    match op {
      IterableIntrinsic::Key => {
        self.emit_iterable_key_common(dst, arg, bytes_len, slow_block, done_block);
      },
      IterableIntrinsic::Value => {
        let is_num = self.is_number(arg);
        let val_fast_block = self.fb.create_block();
        self
          .fb
          .ins()
          .brif(is_num, val_fast_block, &[], slow_block, &[]);

        self.fb.switch_to_block(val_fast_block);
        let zero_f64 = self.fb.ins().f64const(0.0);
        let f = self.to_f64(arg);
        let idx = self.fb.ins().fcvt_to_sint_sat(types::I64, f);
        let f_ge_zero = self.fb.ins().fcmp(FloatCC::GreaterThanOrEqual, f, zero_f64);
        let in_bounds = self.fb.ins().icmp(IntCC::UnsignedLessThan, idx, bytes_len);
        let ok_bounds = self.fb.ins().band(f_ge_zero, in_bounds);

        let load_block = self.fb.create_block();
        let nil_block = self.fb.create_block();
        self
          .fb
          .ins()
          .brif(ok_bounds, load_block, &[], nil_block, &[]);

        self.fb.switch_to_block(load_block);
        let elem_addr = self.fb.ins().iadd(bytes_data_ptr, idx);
        let b8 = self.fb.ins().load(types::I8, flags, elem_addr, 0);
        let b64 = self.fb.ins().uextend(types::I64, b8);
        let bf = self.fb.ins().fcvt_from_uint(types::F64, b64);
        let bval = self.from_f64(bf);
        self.store_reg(dst, bval);
        self.fb.ins().jump(done_block, &[]);

        self.fb.switch_to_block(nil_block);
        let nil_val = self.u64c(value::NIL_VAL);
        self.store_reg(dst, nil_val);
        self.fb.ins().jump(done_block, &[]);
      },
    }
  }

  fn emit_iterable_range(
    &mut self,
    _ip: usize,
    dst: u8,
    _obj: u8,
    op: IterableIntrinsic,
    _recv: IrValue,
    arg: IrValue,
    ptr: IrValue,
    slow_block: Block,
    done_block: Block,
  ) {
    let flags = cranelift_codegen::ir::MemFlagsData::trusted();
    let lower = self
      .fb
      .ins()
      .load(types::F64, flags, ptr, object::obj_range_lower_offset());
    let upper = self
      .fb
      .ins()
      .load(types::F64, flags, ptr, object::obj_range_upper_offset());
    let step = self
      .fb
      .ins()
      .load(types::F64, flags, ptr, object::obj_range_step_offset());

    let zero_f = self.fb.ins().f64const(0.0);
    let one_f = self.fb.ins().f64const(1.0);
    let step_pos = self.fb.ins().fcmp(FloatCC::GreaterThan, step, zero_f);
    let actual_step = self.fb.ins().select(step_pos, step, one_f);

    let diff = self.fb.ins().fsub(upper, lower);
    let width = self.fb.ins().fabs(diff);
    let raw_count = self.fb.ins().fdiv(width, actual_step);
    let ceil_count = self.fb.ins().ceil(raw_count);
    let count = self.fb.ins().fcvt_to_sint_sat(types::I64, ceil_count);

    match op {
      IterableIntrinsic::Key => {
        self.emit_iterable_key_common(dst, arg, count, slow_block, done_block);
      },
      IterableIntrinsic::Value => {
        let is_num = self.is_number(arg);
        let val_fast_block = self.fb.create_block();
        self
          .fb
          .ins()
          .brif(is_num, val_fast_block, &[], slow_block, &[]);

        self.fb.switch_to_block(val_fast_block);
        let nil_val = self.u64c(value::NIL_VAL);
        let f_idx = self.to_f64(arg);
        let ge_zero = self
          .fb
          .ins()
          .fcmp(FloatCC::GreaterThanOrEqual, f_idx, zero_f);
        let lt_count = self.fb.ins().fcmp(FloatCC::LessThan, f_idx, ceil_count);
        let in_range = self.fb.ins().band(ge_zero, lt_count);

        let forward = self
          .fb
          .ins()
          .fcmp(FloatCC::GreaterThanOrEqual, upper, lower);
        let delta = self.fb.ins().fmul(f_idx, actual_step);
        let fwd_val = self.fb.ins().fadd(lower, delta);
        let bwd_val = self.fb.ins().fsub(lower, delta);
        let val_f = self.fb.ins().select(forward, fwd_val, bwd_val);
        let val = self.from_f64(val_f);
        let res = self.fb.ins().select(in_range, val, nil_val);
        self.store_reg(dst, res);
        self.fb.ins().jump(done_block, &[]);
      },
    }
  }

  fn emit_iterable_string(
    &mut self,
    _ip: usize,
    dst: u8,
    _obj: u8,
    op: IterableIntrinsic,
    _recv: IrValue,
    arg: IrValue,
    ptr: IrValue,
    slow_block: Block,
    done_block: Block,
  ) {
    let is_ascii = self.str_ascii_guard(ptr);
    let ascii_block = self.fb.create_block();
    self
      .fb
      .ins()
      .brif(is_ascii, ascii_block, &[], slow_block, &[]);

    self.fb.switch_to_block(ascii_block);
    let flags = cranelift_codegen::ir::MemFlagsData::trusted();
    let str_data_ptr = self
      .fb
      .ins()
      .load(types::I64, flags, ptr, object::obj_str_ptr_offset());
    let str_len = self
      .fb
      .ins()
      .load(types::I64, flags, ptr, object::obj_str_len_offset());

    match op {
      IterableIntrinsic::Key => {
        self.emit_iterable_key_common(dst, arg, str_len, slow_block, done_block);
      },
      IterableIntrinsic::Value => {
        let is_num = self.is_number(arg);
        let val_fast_block = self.fb.create_block();
        self
          .fb
          .ins()
          .brif(is_num, val_fast_block, &[], slow_block, &[]);

        self.fb.switch_to_block(val_fast_block);
        let zero_f64 = self.fb.ins().f64const(0.0);
        let f = self.to_f64(arg);
        let idx = self.fb.ins().fcvt_to_sint_sat(types::I64, f);
        let f_ge_zero = self.fb.ins().fcmp(FloatCC::GreaterThanOrEqual, f, zero_f64);
        let in_bounds = self.fb.ins().icmp(IntCC::UnsignedLessThan, idx, str_len);
        let ok_bounds = self.fb.ins().band(f_ge_zero, in_bounds);

        let load_block = self.fb.create_block();
        let nil_block = self.fb.create_block();
        self
          .fb
          .ins()
          .brif(ok_bounds, load_block, &[], nil_block, &[]);

        self.fb.switch_to_block(load_block);
        let byte_addr = self.fb.ins().iadd(str_data_ptr, idx);
        let b8 = self.fb.ins().load(types::I8, flags, byte_addr, 0);
        let b64 = self.fb.ins().uextend(types::I64, b8);
        let interned_offset = self.fb.ins().imul_imm_s(b64, 8);
        let interned_table = self
          .fb
          .ins()
          .iadd_imm_s(self.vm_param, INTERNED_ASCII_OFFSET as i64);
        let char_addr = self.fb.ins().iadd(interned_table, interned_offset);
        let char_val = self.fb.ins().load(types::I64, flags, char_addr, 0);
        self.store_reg(dst, char_val);
        self.fb.ins().jump(done_block, &[]);

        self.fb.switch_to_block(nil_block);
        let nil_val = self.u64c(value::NIL_VAL);
        self.store_reg(dst, nil_val);
        self.fb.ins().jump(done_block, &[]);
      },
    }
  }

  fn emit_iterable_slow(
    &mut self,
    ip: usize,
    dst: u8,
    obj: u8,
    method_const: u16,
    slow_block: Block,
    done_block: Block,
  ) {
    self.fb.switch_to_block(slow_block);
    self.emit_safepoint();
    self.emit_generic_invoke(ip, dst, obj, method_const, 1);
    self.resync_dst_from_memory(dst);
    self.resync_receiver_from_memory(obj);
    self.fb.ins().jump(done_block, &[]);
  }

  fn emit_iterable_intrinsic(
    &mut self,
    ip: usize,
    dst: u8,
    obj: u8,
    method_const: u16,
    op: IterableIntrinsic,
  ) {
    let recv = self.load_reg(obj);
    let arg = self.load_reg(obj + 2);
    let slow_block = self.fb.create_block();
    let done_block = self.fb.create_block();

    if self.proven_list(ip, obj) {
      let ptr = self.obj_ptr(recv);
      self.emit_iterable_list(ip, dst, obj, op, recv, arg, ptr, slow_block, done_block);
      self.emit_iterable_slow(ip, dst, obj, method_const, slow_block, done_block);
      self.fb.switch_to_block(done_block);
      return;
    }

    if self.proven_string(ip, obj) {
      let ptr = self.obj_ptr(recv);
      self.emit_iterable_string(ip, dst, obj, op, recv, arg, ptr, slow_block, done_block);
      self.emit_iterable_slow(ip, dst, obj, method_const, slow_block, done_block);
      self.fb.switch_to_block(done_block);
      return;
    }

    let is_obj = self.is_obj(recv);
    let check_tag_block = self.fb.create_block();
    self
      .fb
      .ins()
      .brif(is_obj, check_tag_block, &[], slow_block, &[]);

    self.fb.switch_to_block(check_tag_block);
    let ptr = self.obj_ptr(recv);
    let tag = self.obj_tag(ptr);

    let list_block = self.fb.create_block();
    let check_bytes_block = self.fb.create_block();
    let tag_list = self.i64c(object::OBJ_TAG_LIST as i64);
    let is_list = self.fb.ins().icmp(IntCC::Equal, tag, tag_list);
    self
      .fb
      .ins()
      .brif(is_list, list_block, &[], check_bytes_block, &[]);

    self.fb.switch_to_block(list_block);
    self.emit_iterable_list(ip, dst, obj, op, recv, arg, ptr, slow_block, done_block);

    self.fb.switch_to_block(check_bytes_block);
    let bytes_block = self.fb.create_block();
    let check_range_block = self.fb.create_block();
    let tag_bytes = self.i64c(object::OBJ_TAG_BYTES as i64);
    let is_bytes = self.fb.ins().icmp(IntCC::Equal, tag, tag_bytes);
    self
      .fb
      .ins()
      .brif(is_bytes, bytes_block, &[], check_range_block, &[]);

    self.fb.switch_to_block(bytes_block);
    self.emit_iterable_bytes(ip, dst, obj, op, recv, arg, ptr, slow_block, done_block);

    self.fb.switch_to_block(check_range_block);
    let range_block = self.fb.create_block();
    let check_str_block = self.fb.create_block();
    let tag_range = self.i64c(object::OBJ_TAG_RANGE as i64);
    let is_range = self.fb.ins().icmp(IntCC::Equal, tag, tag_range);
    self
      .fb
      .ins()
      .brif(is_range, range_block, &[], check_str_block, &[]);

    self.fb.switch_to_block(range_block);
    self.emit_iterable_range(ip, dst, obj, op, recv, arg, ptr, slow_block, done_block);

    self.fb.switch_to_block(check_str_block);
    let str_block = self.fb.create_block();
    let tag_str = self.i64c(object::OBJ_TAG_STR as i64);
    let is_str = self.fb.ins().icmp(IntCC::Equal, tag, tag_str);
    self.fb.ins().brif(is_str, str_block, &[], slow_block, &[]);

    self.fb.switch_to_block(str_block);
    self.emit_iterable_string(ip, dst, obj, op, recv, arg, ptr, slow_block, done_block);

    self.emit_iterable_slow(ip, dst, obj, method_const, slow_block, done_block);
    self.fb.switch_to_block(done_block);
  }

  /// `object::write_barrier`'s own guard, inlined; owed after EVERY
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
  /// reason; exactly the cost the inline fast path exists to avoid.
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

  /// `emit_write_barrier`'s own guard is already cheap, but it's still
  /// two loads and a branch paid on every single mutation, including
  /// ones where the value going in is a plain number. A number can
  /// never introduce an old->young pointer, so a store of one can
  /// never owe the barrier anything; `emit_ic_set_field` worked this
  /// out first, and this just gives every other field/element mutation
  /// site the same shortcut instead of leaving them to duplicate it by
  /// hand. `proven_numeric` skips the runtime check entirely when
  /// `type_facts` already settled it at compile time; otherwise one
  /// `is_obj` test decides whether `emit_write_barrier` is worth
  /// reaching at all.
  ///
  /// This is what `fannkuch-redux` and friends actually want: their
  /// hot loops are almost pure `list[i] = <number>` traffic, where the
  /// old unconditional barrier call was paying real generation/
  /// remembered-set overhead on every element write for no possible
  /// benefit.
  fn emit_write_barrier_for_store(
    &mut self,
    ip: usize,
    src_reg: u8,
    src_val: IrValue,
    obj_ptr: IrValue,
  ) {
    if self.proven_numeric(ip, src_reg) {
      return;
    }
    let is_obj = self.is_obj(src_val);
    let barrier_block = self.fb.create_block();
    let pass_block = self.fb.create_block();
    self
      .fb
      .ins()
      .brif(is_obj, barrier_block, &[], pass_block, &[]);
    self.fb.switch_to_block(barrier_block);
    self.emit_write_barrier(obj_ptr);
    self.fb.ins().jump(pass_block, &[]);
    self.fb.switch_to_block(pass_block);
  }

  /// `GetUpval`/`SetUpval`'s shared front half: resolves
  /// `upvalues[uidx]` on the running closure and confirms it's really an
  /// `Obj::Upvalue`, leaving its raw `*const Obj` current on return.
  /// Branches to `slow_block` (the ordinary helper) on anything
  /// unexpected.
  ///
  /// The helper reads the closure from the live frame rather than being
  /// handed this function's entry parameter: see
  /// `VM::current_closure_val`.
  ///
  /// One helper call either way (`zuri_jit_closure_upvalues_ptr`), for
  /// the same reason `emit_list_get_index` still pays one for the list
  /// data pointer: `ObjClosure::upvalues` is a real `Vec<Value>`, not a
  /// hand-rolled `#[repr(C)]` vector, so its own internal field layout
  /// isn't something generated code should assume. That call is cheap
  /// (`call_helper_raw`, no live-register flush; it touches no VM
  /// register, can't allocate or fail) and everything after it is real
  /// inline Cranelift code: no bounds check on `uidx` is needed since
  /// it's always a valid index into a closure built from the exact same
  /// prototype's `upvalues` descriptor list this instruction's own
  /// index was compiled against.
  ///
  /// The `is_obj`/tag checks below should be unreachable by that same
  /// construction argument, but stay in anyway; same discipline
  /// `emit_self_get_field`'s unproven path uses for `self`: a calling-
  /// convention invariant, not something to assume in code about to
  /// dereference raw memory.
  fn emit_upvalue_obj_ptr(&mut self, uidx: u8, slow_block: Block) -> IrValue {
    let base_ptr = self.call_helper_raw("zuri_jit_closure_upvalues_ptr", &[self.vm_param]);
    let upval_val = self.fb.ins().load(
      types::I64,
      cranelift_codegen::ir::MemFlagsData::trusted(),
      base_ptr,
      (uidx as i32) * 8,
    );

    let is_obj = self.is_obj(upval_val);
    let obj_block = self.fb.create_block();
    self.fb.ins().brif(is_obj, obj_block, &[], slow_block, &[]);

    self.fb.switch_to_block(obj_block);
    let ptr = self.obj_ptr(upval_val);
    let tag = self.obj_tag(ptr);
    let tag_upvalue = self.i64c(object::OBJ_TAG_UPVALUE as i64);
    let is_upvalue = self.fb.ins().icmp(IntCC::Equal, tag, tag_upvalue);
    let checked_block = self.fb.create_block();
    self
      .fb
      .ins()
      .brif(is_upvalue, checked_block, &[], slow_block, &[]);

    self.fb.switch_to_block(checked_block);
    ptr
  }

  /// `Instr::GetUpval`'s inline fast path: no helper call for either
  /// `UpvalueState` variant. `Open` reads straight out of `regs_var`
  /// (the same absolute registers-array pointer every ordinary register
  /// access already uses; an `Open` upvalue's index is, by
  /// definition, already an absolute index into that exact array, see
  /// `UpvalueState::Open`'s own docs), `Closed` reads the cell's own
  /// payload word directly. Only a genuinely unexpected receiver falls
  /// to `zuri_jit_get_upval`.
  fn emit_get_upval_fast(&mut self, dst: u8, uidx: u8) {
    let slow_block = self.fb.create_block();
    let done_block = self.fb.create_block();

    let ptr = self.emit_upvalue_obj_ptr(uidx, slow_block);
    let flags = cranelift_codegen::ir::MemFlagsData::trusted();
    let tag8 = self.fb.ins().load(
      types::I8,
      flags,
      ptr,
      object::obj_upvalue_state_tag_offset() as i32,
    );
    let state_tag = self.fb.ins().uextend(types::I64, tag8);
    let closed_tag = self.i64c(object::UPVALUE_STATE_TAG_CLOSED as i64);
    let is_closed = self.fb.ins().icmp(IntCC::Equal, state_tag, closed_tag);

    let closed_block = self.fb.create_block();
    let open_block = self.fb.create_block();
    self
      .fb
      .ins()
      .brif(is_closed, closed_block, &[], open_block, &[]);

    let payload_off = object::obj_upvalue_state_payload_offset() as i32;

    self.fb.switch_to_block(closed_block);
    let v = self.fb.ins().load(types::I64, flags, ptr, payload_off);
    self.store_reg(dst, v);
    self.fb.ins().jump(done_block, &[]);

    self.fb.switch_to_block(open_block);
    let abs_idx = self.fb.ins().load(types::I64, flags, ptr, payload_off);
    let regs = self.fb.use_var(self.regs_var);
    let byte_off = self.fb.ins().imul_imm_s(abs_idx, 8);
    let addr = self.fb.ins().iadd(regs, byte_off);
    let v = self.fb.ins().load(types::I64, flags, addr, 0);
    self.store_reg(dst, v);
    self.fb.ins().jump(done_block, &[]);

    self.fb.switch_to_block(slow_block);
    let base = self.base_param;
    let dst_i = self.idx(dst);
    let uidx_i = self.idx(uidx);
    self.call_checked("zuri_jit_get_upval", &[self.vm_param, base, dst_i, uidx_i]);
    self.resync_dst_from_memory(dst);
    self.fb.ins().jump(done_block, &[]);

    self.fb.switch_to_block(done_block);
  }

  /// `Instr::SetUpval`'s inline fast path; `emit_get_upval_fast`'s
  /// write-side counterpart. `Closed` needs the same write barrier any
  /// other heap-object field mutation owes (see `emit_write_barrier`);
  /// `Open` writes straight into `regs_var`, no barrier, exactly like
  /// `zuri_jit_set_upval`'s own `Open` arm (a register is never a GC
  /// root-set boundary the way a heap object's own memory is).
  fn emit_set_upval_fast(&mut self, ip: usize, uidx: u8, src: u8) {
    let src_val = self.load_reg(src);
    let slow_block = self.fb.create_block();
    let done_block = self.fb.create_block();

    let ptr = self.emit_upvalue_obj_ptr(uidx, slow_block);
    let flags = cranelift_codegen::ir::MemFlagsData::trusted();
    let tag8 = self.fb.ins().load(
      types::I8,
      flags,
      ptr,
      object::obj_upvalue_state_tag_offset() as i32,
    );
    let state_tag = self.fb.ins().uextend(types::I64, tag8);
    let closed_tag = self.i64c(object::UPVALUE_STATE_TAG_CLOSED as i64);
    let is_closed = self.fb.ins().icmp(IntCC::Equal, state_tag, closed_tag);

    let closed_block = self.fb.create_block();
    let open_block = self.fb.create_block();
    self
      .fb
      .ins()
      .brif(is_closed, closed_block, &[], open_block, &[]);

    let payload_off = object::obj_upvalue_state_payload_offset() as i32;

    self.fb.switch_to_block(closed_block);
    self.fb.ins().store(flags, src_val, ptr, payload_off);
    self.emit_write_barrier_for_store(ip, src, src_val, ptr);
    self.fb.ins().jump(done_block, &[]);

    self.fb.switch_to_block(open_block);
    let abs_idx = self.fb.ins().load(types::I64, flags, ptr, payload_off);
    let regs = self.fb.use_var(self.regs_var);
    let byte_off = self.fb.ins().imul_imm_s(abs_idx, 8);
    let addr = self.fb.ins().iadd(regs, byte_off);
    self.fb.ins().store(flags, src_val, addr, 0);
    self.fb.ins().jump(done_block, &[]);

    self.fb.switch_to_block(slow_block);
    let base = self.base_param;
    let src_i = self.idx(src);
    let uidx_i = self.idx(uidx);
    self.call_checked("zuri_jit_set_upval", &[self.vm_param, base, src_i, uidx_i]);
    self.resync_receiver_from_memory(src);
    self.fb.ins().jump(done_block, &[]);

    self.fb.switch_to_block(done_block);
  }

  /// Loads an `Obj::Instance`'s `fields` slice base pointer, given a
  /// raw `*const Obj` already proven (by a runtime tag check against
  /// `OBJ_TAG_INSTANCE`, in a block only reachable when `is_obj` was
  /// ALSO already proven true: see `emit_self_get_field`'s two-stage
  /// branch) to actually be one: see
  /// `object::obj_instance_fields_ptr_offset()`'s own docs for why this
  /// fixed offset is sound.
  fn load_instance_fields_ptr(&mut self, obj_ptr: IrValue) -> IrValue {
    let flags = cranelift_codegen::ir::MemFlagsData::trusted();
    let heap_ptr = self.fb.ins().load(
      types::I64,
      flags,
      obj_ptr,
      object::obj_instance_fields_offset() as i32,
    );
    let inline_ptr = self
      .fb
      .ins()
      .iadd_imm_s(obj_ptr, object::obj_instance_fields_inline_offset() as i64);
    let zero = self.i64c(0);
    let is_inline = self.fb.ins().icmp(IntCC::Equal, heap_ptr, zero);
    self.fb.ins().select(is_inline, inline_ptr, heap_ptr)
  }

  /// True when the string at `ptr` is known to hold nothing but ASCII,
  /// which is exactly the condition under which a codepoint index may be
  /// used as a byte offset into its data.
  ///
  /// Reads the flag cached on the string itself rather than inspecting
  /// any bytes: one load and one compare, no scan, and no dependence on
  /// the index. Checking only the byte that lands at the index instead
  /// (which is what this code used to do) is NOT sound; in
  /// `"'name': Alice"` written with a two-codepoint multi-byte name,
  /// byte 8 is an ASCII `:` while codepoint 8 is `i`, so the check
  /// passes and the wrong character is returned.
  ///
  /// The flag starts out unknown and is filled in the first time
  /// anything indexes the string through `Value::str_is_ascii`, so a
  /// freshly built string takes the slow path once and is fast after
  /// that.
  fn str_ascii_guard(&mut self, ptr: IrValue) -> IrValue {
    let flags = cranelift_codegen::ir::MemFlagsData::trusted();
    let flag = self
      .fb
      .ins()
      .load(types::I8, flags, ptr, object::obj_str_ascii_offset());
    let yes = self.fb.ins().iconst(types::I8, object::ASCII_YES as i64);
    self.fb.ins().icmp(IntCC::Equal, flag, yes)
  }

  fn emit_str_get_index(
    &mut self,
    ip: usize,
    dst: u8,
    obj: u8,
    iidx: u8,
    idx_proven_numeric: bool,
    idx_proven_int: bool,
  ) {
    let obj_val = self.load_reg(obj);
    let idx_val = self.load_reg(iidx);
    let proven_str = self.proven_string(ip, obj);

    let slow_block = self.fb.create_block();
    let done_block = self.fb.create_block();
    let resolve_block = self.fb.create_block();

    let (ptr, as_int) = if proven_str {
      let ptr = self.obj_ptr(obj_val);
      let f = self.f64_view(iidx, idx_val);
      let as_int = self.fb.ins().fcvt_to_sint_sat(types::I64, f);
      if idx_proven_int {
        self.fb.ins().jump(resolve_block, &[]);
      } else {
        let roundtrip = self.fb.ins().fcvt_from_sint(types::F64, as_int);
        let is_int = self.fb.ins().fcmp(
          cranelift_codegen::ir::condcodes::FloatCC::Equal,
          f,
          roundtrip,
        );
        let idx_ok = if idx_proven_numeric {
          is_int
        } else {
          let is_num = self.is_number(idx_val);
          self.fb.ins().band(is_num, is_int)
        };
        self
          .fb
          .ins()
          .brif(idx_ok, resolve_block, &[], slow_block, &[]);
      }
      (ptr, as_int)
    } else {
      let is_obj = self.is_obj(obj_val);
      let cheap_guard = if idx_proven_numeric {
        is_obj
      } else {
        let is_num = self.is_number(idx_val);
        self.fb.ins().band(is_obj, is_num)
      };

      let checked_block = self.fb.create_block();
      self
        .fb
        .ins()
        .brif(cheap_guard, checked_block, &[], slow_block, &[]);

      self.fb.switch_to_block(checked_block);
      let ptr = self.obj_ptr(obj_val);
      let tag = self.obj_tag(ptr);
      let tag_str = self.i64c(object::OBJ_TAG_STR as i64);
      let is_str = self.fb.ins().icmp(IntCC::Equal, tag, tag_str);

      let f = self.f64_view(iidx, idx_val);
      let as_int = self.fb.ins().fcvt_to_sint_sat(types::I64, f);
      if idx_proven_int {
        self
          .fb
          .ins()
          .brif(is_str, resolve_block, &[], slow_block, &[]);
      } else {
        let roundtrip = self.fb.ins().fcvt_from_sint(types::F64, as_int);
        let is_int = self.fb.ins().fcmp(
          cranelift_codegen::ir::condcodes::FloatCC::Equal,
          f,
          roundtrip,
        );
        let str_and_int = self.fb.ins().band(is_str, is_int);
        self
          .fb
          .ins()
          .brif(str_and_int, resolve_block, &[], slow_block, &[]);
      }
      (ptr, as_int)
    };

    self.fb.switch_to_block(resolve_block);
    let flags = cranelift_codegen::ir::MemFlagsData::trusted();
    let data_ptr = self
      .fb
      .ins()
      .load(types::I64, flags, ptr, object::obj_str_ptr_offset());
    let len = self
      .fb
      .ins()
      .load(types::I64, flags, ptr, object::obj_str_len_offset());
    // See `str_ascii_guard`: without this the byte length below is not
    // the codepoint count, and the byte at the index is not the
    // codepoint at the index.
    let ascii_ok = self.str_ascii_guard(ptr);
    let bounds_block = self.fb.create_block();
    self
      .fb
      .ins()
      .brif(ascii_ok, bounds_block, &[], slow_block, &[]);

    self.fb.switch_to_block(bounds_block);
    let in_bounds = self.fb.ins().icmp(IntCC::UnsignedLessThan, as_int, len);

    let ascii_block = self.fb.create_block();
    self
      .fb
      .ins()
      .brif(in_bounds, ascii_block, &[], slow_block, &[]);

    self.fb.switch_to_block(ascii_block);
    let byte_addr = self.fb.ins().iadd(data_ptr, as_int);
    let byte_val = self.fb.ins().load(types::I8, flags, byte_addr, 0);
    let byte_ext = self.fb.ins().uextend(types::I64, byte_val);

    let fast_block = self.fb.create_block();
    self.fb.ins().jump(fast_block, &[]);

    self.fb.switch_to_block(fast_block);
    let byte_off = self.fb.ins().imul_imm_s(byte_ext, 8);
    let ascii_table_off = self
      .fb
      .ins()
      .iadd_imm_s(byte_off, crate::vm::vm::VM_INTERNED_ASCII_OFFSET as i64);
    let ascii_addr = self.fb.ins().iadd(self.vm_param, ascii_table_off);
    let char_val = self.fb.ins().load(types::I64, flags, ascii_addr, 0);
    self.store_reg(dst, char_val);
    self.fb.ins().jump(done_block, &[]);

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
  }

  fn emit_str_add(&mut self, _ip: usize, dst: u8, a: u8, b: u8) {
    let va = self.load_reg(a);
    let vb = self.load_reg(b);
    let flags = cranelift_codegen::ir::MemFlagsData::trusted();

    let slow_block = self.fb.create_block();
    let done_block = self.fb.create_block();

    if dst == a {
      let is_obj_a = self.is_obj(va);
      let is_obj_b = self.is_obj(vb);
      let both_obj = self.fb.ins().band(is_obj_a, is_obj_b);

      let check_tags_block = self.fb.create_block();
      self
        .fb
        .ins()
        .brif(both_obj, check_tags_block, &[], slow_block, &[]);

      self.fb.switch_to_block(check_tags_block);
      let ptr_a = self.obj_ptr(va);
      let ptr_b = self.obj_ptr(vb);
      let tag_a = self.obj_tag(ptr_a);
      let tag_b = self.obj_tag(ptr_b);
      let tag_str = self.i64c(object::OBJ_TAG_STR as i64);
      let is_str_a = self.fb.ins().icmp(IntCC::Equal, tag_a, tag_str);
      let is_str_b = self.fb.ins().icmp(IntCC::Equal, tag_b, tag_str);
      let both_str = self.fb.ins().band(is_str_a, is_str_b);

      let check_inplace_block = self.fb.create_block();
      self
        .fb
        .ins()
        .brif(both_str, check_inplace_block, &[], slow_block, &[]);

      self.fb.switch_to_block(check_inplace_block);
      let gen_off = object::obj_to_gcbox_generation_offset();
      let gen_byte = self.fb.ins().load(types::I8, flags, ptr_a, gen_off);
      let zero_u8 = self.fb.ins().iconst(types::I8, 0); // Generation::Young is 0
      let is_young_a = self.fb.ins().icmp(IntCC::Equal, gen_byte, zero_u8);

      let young_inplace_block = self.fb.create_block();
      self
        .fb
        .ins()
        .brif(is_young_a, young_inplace_block, &[], slow_block, &[]);

      self.fb.switch_to_block(young_inplace_block);
      let len_a = self
        .fb
        .ins()
        .load(types::I64, flags, ptr_a, object::obj_str_len_offset());
      let cap_a = self
        .fb
        .ins()
        .load(types::I64, flags, ptr_a, object::obj_str_cap_offset());
      let len_b = self
        .fb
        .ins()
        .load(types::I64, flags, ptr_b, object::obj_str_len_offset());
      let total_len = self.fb.ins().iadd(len_a, len_b);
      let can_fit = self
        .fb
        .ins()
        .icmp(IntCC::UnsignedLessThanOrEqual, total_len, cap_a);

      let append_1_block = self.fb.create_block();
      self
        .fb
        .ins()
        .brif(can_fit, append_1_block, &[], slow_block, &[]);

      self.fb.switch_to_block(append_1_block);
      let data_a = self
        .fb
        .ins()
        .load(types::I64, flags, ptr_a, object::obj_str_ptr_offset());
      let data_b = self
        .fb
        .ins()
        .load(types::I64, flags, ptr_b, object::obj_str_ptr_offset());
      let dst_addr = self.fb.ins().iadd(data_a, len_a);
      let one = self.i64c(1);
      let is_one_char = self.fb.ins().icmp(IntCC::Equal, len_b, one);
      let single_byte_block = self.fb.create_block();
      self
        .fb
        .ins()
        .brif(is_one_char, single_byte_block, &[], slow_block, &[]);

      self.fb.switch_to_block(single_byte_block);
      let b_byte = self.fb.ins().load(types::I8, flags, data_b, 0);
      self.fb.ins().store(flags, b_byte, dst_addr, 0);
      self
        .fb
        .ins()
        .store(flags, total_len, ptr_a, object::obj_str_len_offset());
      self.store_reg(dst, va);
      self.fb.ins().jump(done_block, &[]);
    } else {
      self.fb.ins().jump(slow_block, &[]);
    }

    self.fb.switch_to_block(slow_block);
    let base = self.base_param;
    let dst_i = self.idx(dst);
    let a_i = self.idx(a);
    let b_i = self.idx(b);
    self.call_checked("zuri_jit_str_add", &[self.vm_param, base, dst_i, a_i, b_i]);
    self.resync_dst_from_memory(dst);
    self.fb.ins().jump(done_block, &[]);

    self.fb.switch_to_block(done_block);
  }

  fn emit_str_add_dynamic(&mut self, _ip: usize, dst: u8, a: u8, b: u8, done_block: Block) {
    let va = self.load_reg(a);
    let vb = self.load_reg(b);
    let flags = cranelift_codegen::ir::MemFlagsData::trusted();

    let slow_block = self.fb.create_block();

    if dst == a {
      let is_obj_a = self.is_obj(va);
      let is_obj_b = self.is_obj(vb);
      let both_obj = self.fb.ins().band(is_obj_a, is_obj_b);

      let check_tags_block = self.fb.create_block();
      self
        .fb
        .ins()
        .brif(both_obj, check_tags_block, &[], slow_block, &[]);

      self.fb.switch_to_block(check_tags_block);
      let ptr_a = self.obj_ptr(va);
      let ptr_b = self.obj_ptr(vb);
      let tag_a = self.obj_tag(ptr_a);
      let tag_b = self.obj_tag(ptr_b);
      let tag_str = self.i64c(object::OBJ_TAG_STR as i64);
      let is_str_a = self.fb.ins().icmp(IntCC::Equal, tag_a, tag_str);
      let is_str_b = self.fb.ins().icmp(IntCC::Equal, tag_b, tag_str);
      let both_str = self.fb.ins().band(is_str_a, is_str_b);

      let check_inplace_block = self.fb.create_block();
      self
        .fb
        .ins()
        .brif(both_str, check_inplace_block, &[], slow_block, &[]);

      self.fb.switch_to_block(check_inplace_block);
      let gen_off = object::obj_to_gcbox_generation_offset();
      let gen_byte = self.fb.ins().load(types::I8, flags, ptr_a, gen_off);
      let zero_u8 = self.fb.ins().iconst(types::I8, 0); // Generation::Young is 0
      let is_young_a = self.fb.ins().icmp(IntCC::Equal, gen_byte, zero_u8);

      let young_inplace_block = self.fb.create_block();
      self
        .fb
        .ins()
        .brif(is_young_a, young_inplace_block, &[], slow_block, &[]);

      self.fb.switch_to_block(young_inplace_block);
      let len_a = self
        .fb
        .ins()
        .load(types::I64, flags, ptr_a, object::obj_str_len_offset());
      let cap_a = self
        .fb
        .ins()
        .load(types::I64, flags, ptr_a, object::obj_str_cap_offset());
      let len_b = self
        .fb
        .ins()
        .load(types::I64, flags, ptr_b, object::obj_str_len_offset());
      let total_len = self.fb.ins().iadd(len_a, len_b);
      let can_fit = self
        .fb
        .ins()
        .icmp(IntCC::UnsignedLessThanOrEqual, total_len, cap_a);

      let append_1_block = self.fb.create_block();
      self
        .fb
        .ins()
        .brif(can_fit, append_1_block, &[], slow_block, &[]);

      self.fb.switch_to_block(append_1_block);
      let data_a = self
        .fb
        .ins()
        .load(types::I64, flags, ptr_a, object::obj_str_ptr_offset());
      let data_b = self
        .fb
        .ins()
        .load(types::I64, flags, ptr_b, object::obj_str_ptr_offset());
      let dst_addr = self.fb.ins().iadd(data_a, len_a);
      let one = self.i64c(1);
      let is_one_char = self.fb.ins().icmp(IntCC::Equal, len_b, one);
      let single_byte_block = self.fb.create_block();
      self
        .fb
        .ins()
        .brif(is_one_char, single_byte_block, &[], slow_block, &[]);

      self.fb.switch_to_block(single_byte_block);
      let b_byte = self.fb.ins().load(types::I8, flags, data_b, 0);
      self.fb.ins().store(flags, b_byte, dst_addr, 0);
      self
        .fb
        .ins()
        .store(flags, total_len, ptr_a, object::obj_str_len_offset());
      self.store_reg(dst, va);
      self.fb.ins().jump(done_block, &[]);
    } else {
      self.fb.ins().jump(slow_block, &[]);
    }

    self.fb.switch_to_block(slow_block);
    let base = self.base_param;
    let dst_i = self.idx(dst);
    let a_i = self.idx(a);
    let b_i = self.idx(b);
    self.call_checked("zuri_jit_add_slow", &[self.vm_param, base, dst_i, a_i, b_i]);
    self.resync_dst_from_memory(dst);
    self.fb.ins().jump(done_block, &[]);
  }

  /// `Instr::GetIndex`'s fast path for `x[i]` with an integer index.
  /// When the receiver isn't already proven, this walks a tag chain and
  /// has an inline arm for each of list, string and bytes; a string
  /// additionally needs its ASCII flag set before a codepoint index can
  /// be used as a byte offset (see `str_ascii_guard`). Negative indices
  /// are folded inline against the length rather than punted.
  ///
  /// What still falls back to the general `zuri_jit_get_index` helper:
  /// a dict receiver (no inline arm), anything that isn't a list,
  /// string or bytes, a non-numeric or non-integer index, a non-ASCII
  /// string, or a genuinely out-of-bounds index, which needs the
  /// general path's real `RangeError` message.
  ///
  /// See `zuri_jit_list_data`'s own docs for why resolving a list's
  /// data pointer still costs one small helper call while the bounds
  /// check and element load are real inline Cranelift code either way.
  /// Strings and bytes carry their data pointer and length in the
  /// object header, so their arms load both directly and need no
  /// helper at all.
  /// `x[i]` when `x` is already proven a byte stream: the whole object
  /// shape guard (`is_obj`, tag load, tag compare chain) is knowledge
  /// the compiler already has, so none of it is emitted. Measured at
  /// roughly 1.7ns of the 2.64ns an unproven byte read costs, which is
  /// most of what a byte-shuffling loop spends per element.
  ///
  /// Semantics are exactly the general bytes arm's, negative indexing
  /// included; only the guard in front of it is gone. Anything the
  /// inline path cannot answer (a non-integer index, genuinely out of
  /// range) still falls to `slow_block` so the error stays the general
  /// path's to phrase.
  fn emit_proven_bytes_get_index(
    &mut self,
    ip: usize,
    dst: u8,
    obj: u8,
    iidx: u8,
    idx_proven_numeric: bool,
    idx_proven_int: bool,
  ) {
    let obj_val = self.load_reg(obj);
    let idx_val = self.load_reg(iidx);
    let ptr = self.obj_ptr(obj_val);
    let seeds_int = self.int_tracked[dst as usize]
      && ip + 1 < self.proto.chunk.code.len()
      && self.int_facts.is_int(ip + 1, dst);

    let slow_block = self.fb.create_block();
    let done_block = self.fb.create_block();
    let resolve_block = self.fb.create_block();

    let f = self.f64_view(iidx, idx_val);
    let as_int = self.fb.ins().fcvt_to_sint_sat(types::I64, f);
    if idx_proven_int {
      self.fb.ins().jump(resolve_block, &[]);
    } else {
      let roundtrip = self.fb.ins().fcvt_from_sint(types::F64, as_int);
      let is_int = self.fb.ins().fcmp(
        cranelift_codegen::ir::condcodes::FloatCC::Equal,
        f,
        roundtrip,
      );
      let idx_ok = if idx_proven_numeric {
        is_int
      } else {
        let is_num = self.is_number(idx_val);
        self.fb.ins().band(is_num, is_int)
      };
      self
        .fb
        .ins()
        .brif(idx_ok, resolve_block, &[], slow_block, &[]);
    }

    self.fb.switch_to_block(resolve_block);
    let flags = cranelift_codegen::ir::MemFlagsData::trusted();
    let data_ptr = self
      .fb
      .ins()
      .load(types::I64, flags, ptr, object::obj_bytes_ptr_offset());
    let len = self
      .fb
      .ins()
      .load(types::I64, flags, ptr, object::obj_bytes_len_offset());
    let in_bounds = self.fb.ins().icmp(IntCC::UnsignedLessThan, as_int, len);
    let fast_block = self.fb.create_block();
    let check_neg_block = self.fb.create_block();
    self
      .fb
      .ins()
      .brif(in_bounds, fast_block, &[], check_neg_block, &[]);

    self.fb.switch_to_block(check_neg_block);
    let zero = self.i64c(0);
    let is_neg = self.fb.ins().icmp(IntCC::SignedLessThan, as_int, zero);
    let neg_block = self.fb.create_block();
    self.fb.ins().brif(is_neg, neg_block, &[], slow_block, &[]);

    self.fb.switch_to_block(neg_block);
    let adjusted = self.fb.ins().iadd(len, as_int);
    let neg_in_bounds = self.fb.ins().icmp(IntCC::UnsignedLessThan, adjusted, len);
    let neg_fast_block = self.fb.create_block();
    self
      .fb
      .ins()
      .brif(neg_in_bounds, neg_fast_block, &[], slow_block, &[]);

    self.fb.switch_to_block(neg_fast_block);
    let addr_neg = self.fb.ins().iadd(data_ptr, adjusted);
    let b_neg = self.fb.ins().load(types::I8, flags, addr_neg, 0);
    let b_neg64 = self.fb.ins().uextend(types::I64, b_neg);
    self.store_reg_int(dst, b_neg64);
    self.fb.ins().jump(done_block, &[]);

    self.fb.switch_to_block(fast_block);
    let addr = self.fb.ins().iadd(data_ptr, as_int);
    let b = self.fb.ins().load(types::I8, flags, addr, 0);
    let b64 = self.fb.ins().uextend(types::I64, b);
    self.store_reg_int(dst, b64);
    self.fb.ins().jump(done_block, &[]);

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
    // The inline arms define `dst`'s integer Variable through
    // `store_reg_int`; this path writes only the boxed form, so it has
    // to seed the integer view too or a later read of it would find an
    // undefined Variable on this edge.
    if seeds_int {
      let v = self.load_reg(dst);
      let f = self.to_f64(v);
      let iv = self.fb.ins().fcvt_to_sint_sat(types::I64, f);
      self.fb.def_var(self.reg_vars_int[dst as usize], iv);
    }
    self.fb.ins().jump(done_block, &[]);

    self.fb.switch_to_block(done_block);
  }

  fn emit_list_get_index(
    &mut self,
    ip: usize,
    dst: u8,
    obj: u8,
    iidx: u8,
    idx_proven_numeric: bool,
    idx_proven_int: bool,
  ) {
    let obj_val = self.load_reg(obj);
    let idx_val = self.load_reg(iidx);
    let proven_list = self.proven_list(ip, obj);

    let slow_block = self.fb.create_block();
    let done_block = self.fb.create_block();
    let resolve_block = self.fb.create_block();

    let proven_const_idx = self
      .proven_const(ip, iidx)
      .filter(|c| c.fract() == 0.0 && *c >= i64::MIN as f64 && *c <= i64::MAX as f64);
    let idx_proven_int = idx_proven_int || proven_const_idx.is_some();

    // `proven_list` is a compile-time fact; exactly one of these two
    // arms is ever actually emitted for a given `Instr::GetIndex` site,
    // never both, so `ptr`/`as_int` dominate `resolve_block` either way
    // (a single predecessor chain, just a shorter one when proven).
    let (ptr, as_int) = if proven_list {
      // The object-shape half of the guard (`is_obj` + tag==LIST)
      // already ran once, at `obj`'s own Instr::CheckParamType; go
      // straight to the pointer; only the index still needs checking
      // here.
      let ptr = self.obj_ptr(obj_val);
      let as_int = if let Some(c) = proven_const_idx {
        self.fb.ins().iconst(types::I64, c as i64)
      } else if idx_proven_int && self.int_tracked[iidx as usize] {
        self.fb.use_var(self.reg_vars_int[iidx as usize])
      } else {
        let f = self.f64_view(iidx, idx_val);
        self.fb.ins().fcvt_to_sint_sat(types::I64, f)
      };
      if idx_proven_int {
        // `iidx` is proven to be a genuine whole number (see
        // `typeflow::IntFacts`'s own docs) or compile-time constant;
        // there's nothing left for this guard to prove, so there's no
        // guard: straight through to `resolve_block`, not even a branch.
        self.fb.ins().jump(resolve_block, &[]);
      } else {
        let f = self.f64_view(iidx, idx_val);
        let roundtrip = self.fb.ins().fcvt_from_sint(types::F64, as_int);
        let is_int = self.fb.ins().fcmp(
          cranelift_codegen::ir::condcodes::FloatCC::Equal,
          f,
          roundtrip,
        );
        let idx_ok = if idx_proven_numeric {
          is_int
        } else {
          let is_num = self.is_number(idx_val);
          self.fb.ins().band(is_num, is_int)
        };
        self
          .fb
          .ins()
          .brif(idx_ok, resolve_block, &[], slow_block, &[]);
      }
      (ptr, as_int)
    } else {
      // Both safe to compute unconditionally regardless of the other's
      // truth value; neither dereferences memory, see `is_obj`/
      // `is_number`'s own docs. `type_facts` already proves the index
      // numeric for the overwhelmingly common case (a loop counter
      // indexing a list), so there's no need to pay for a dynamic check
      // of something the compiler already knows; same discipline
      // arithmetic/bitwise ops use via `proven_numeric`.
      let is_obj = self.is_obj(obj_val);
      let cheap_guard = if idx_proven_numeric {
        is_obj
      } else {
        let is_num = self.is_number(idx_val);
        self.fb.ins().band(is_obj, is_num)
      };

      let checked_block = self.fb.create_block();
      self
        .fb
        .ins()
        .brif(cheap_guard, checked_block, &[], slow_block, &[]);

      // `ptr`/`tag` (dereferences memory) and the float round-trip check
      // (pure arithmetic, but only MEANINGFUL once `is_num` is known
      // true) are both only computed here, in a block reachable only
      // when `cheap_guard`; and therefore `is_obj`; was already
      // proven true. Same discipline `emit_self_get_field` uses for its
      // own tag check.
      self.fb.switch_to_block(checked_block);
      let ptr = self.obj_ptr(obj_val);
      let tag = self.obj_tag(ptr);
      let tag_list = self.i64c(object::OBJ_TAG_LIST as i64);
      let is_list = self.fb.ins().icmp(IntCC::Equal, tag, tag_list);

      let (as_int, f) = if let Some(c) = proven_const_idx {
        (self.fb.ins().iconst(types::I64, c as i64), None)
      } else if idx_proven_int && self.int_tracked[iidx as usize] {
        (self.fb.use_var(self.reg_vars_int[iidx as usize]), None)
      } else {
        let f = self.f64_view(iidx, idx_val);
        let as_int = self.fb.ins().fcvt_to_sint_sat(types::I64, f);
        (as_int, Some(f))
      };
      let check_str_block = self.fb.create_block();
      if idx_proven_int {
        // See the `proven_list` arm above; the index half of the
        // guard is a settled fact, only `is_list` still needs checking.
        self
          .fb
          .ins()
          .brif(is_list, resolve_block, &[], check_str_block, &[]);
      } else {
        let f = f.unwrap();
        let roundtrip = self.fb.ins().fcvt_from_sint(types::F64, as_int);
        let is_int = self.fb.ins().fcmp(
          cranelift_codegen::ir::condcodes::FloatCC::Equal,
          f,
          roundtrip,
        );
        let list_and_int = self.fb.ins().band(is_list, is_int);
        self
          .fb
          .ins()
          .brif(list_and_int, resolve_block, &[], check_str_block, &[]);
      }

      self.fb.switch_to_block(check_str_block);
      let tag_str = self.i64c(object::OBJ_TAG_STR as i64);
      let is_str = self.fb.ins().icmp(IntCC::Equal, tag, tag_str);
      let str_resolve_block = self.fb.create_block();
      let check_bytes_get_block = self.fb.create_block();
      if idx_proven_int {
        self
          .fb
          .ins()
          .brif(is_str, str_resolve_block, &[], check_bytes_get_block, &[]);
      } else {
        let f = f.unwrap();
        let roundtrip = self.fb.ins().fcvt_from_sint(types::F64, as_int);
        let is_int = self.fb.ins().fcmp(
          cranelift_codegen::ir::condcodes::FloatCC::Equal,
          f,
          roundtrip,
        );
        let str_and_int = self.fb.ins().band(is_str, is_int);
        self.fb.ins().brif(
          str_and_int,
          str_resolve_block,
          &[],
          check_bytes_get_block,
          &[],
        );
      }

      self.fb.switch_to_block(check_bytes_get_block);
      let tag_bytes = self.i64c(object::OBJ_TAG_BYTES as i64);
      let is_bytes = self.fb.ins().icmp(IntCC::Equal, tag, tag_bytes);
      let bytes_resolve_block = self.fb.create_block();
      if idx_proven_int {
        self
          .fb
          .ins()
          .brif(is_bytes, bytes_resolve_block, &[], slow_block, &[]);
      } else {
        let f = f.unwrap();
        let roundtrip = self.fb.ins().fcvt_from_sint(types::F64, as_int);
        let is_int = self.fb.ins().fcmp(
          cranelift_codegen::ir::condcodes::FloatCC::Equal,
          f,
          roundtrip,
        );
        let bytes_and_int = self.fb.ins().band(is_bytes, is_int);
        self
          .fb
          .ins()
          .brif(bytes_and_int, bytes_resolve_block, &[], slow_block, &[]);
      }

      self.fb.switch_to_block(bytes_resolve_block);
      let flags_b = cranelift_codegen::ir::MemFlagsData::trusted();
      let bytes_data_ptr =
        self
          .fb
          .ins()
          .load(types::I64, flags_b, ptr, object::obj_bytes_ptr_offset());
      let bytes_len = self
        .fb
        .ins()
        .load(types::I64, flags_b, ptr, object::obj_bytes_len_offset());
      let bytes_in_bounds = self
        .fb
        .ins()
        .icmp(IntCC::UnsignedLessThan, as_int, bytes_len);
      let bytes_fast_block = self.fb.create_block();
      let bytes_check_neg_block = self.fb.create_block();
      self.fb.ins().brif(
        bytes_in_bounds,
        bytes_fast_block,
        &[],
        bytes_check_neg_block,
        &[],
      );

      self.fb.switch_to_block(bytes_check_neg_block);
      let zero = self.i64c(0);
      let bytes_is_neg = self.fb.ins().icmp(IntCC::SignedLessThan, as_int, zero);
      let bytes_neg_block = self.fb.create_block();
      self
        .fb
        .ins()
        .brif(bytes_is_neg, bytes_neg_block, &[], slow_block, &[]);

      self.fb.switch_to_block(bytes_neg_block);
      let bytes_adjusted = self.fb.ins().iadd(bytes_len, as_int);
      let bytes_neg_in_bounds =
        self
          .fb
          .ins()
          .icmp(IntCC::UnsignedLessThan, bytes_adjusted, bytes_len);
      let bytes_neg_fast_block = self.fb.create_block();
      self.fb.ins().brif(
        bytes_neg_in_bounds,
        bytes_neg_fast_block,
        &[],
        slow_block,
        &[],
      );

      self.fb.switch_to_block(bytes_neg_fast_block);
      let byte_elem_addr_neg = self.fb.ins().iadd(bytes_data_ptr, bytes_adjusted);
      let b8_neg = self
        .fb
        .ins()
        .load(types::I8, flags_b, byte_elem_addr_neg, 0);
      let b8_u64_neg = self.fb.ins().uextend(types::I64, b8_neg);
      // A byte is a whole number in 0..=255, so this lands in the
      // destination's integer register (when it has one; see
      // `typeflow::transfer_int`'s `GetIndex` arm). Anything consuming
      // it as an index then takes the integer straight out instead of
      // converting back and re-proving it was whole, and the float view
      // `store_reg_int` keeps in step falls to DCE where nothing reads
      // it.
      self.store_reg_int(dst, b8_u64_neg);
      self.fb.ins().jump(done_block, &[]);

      self.fb.switch_to_block(bytes_fast_block);
      let byte_elem_addr = self.fb.ins().iadd(bytes_data_ptr, as_int);
      let b8 = self.fb.ins().load(types::I8, flags_b, byte_elem_addr, 0);
      let b8_u64 = self.fb.ins().uextend(types::I64, b8);
      self.store_reg_int(dst, b8_u64);
      self.fb.ins().jump(done_block, &[]);

      self.fb.switch_to_block(str_resolve_block);
      let flags = cranelift_codegen::ir::MemFlagsData::trusted();
      let str_data_ptr = self
        .fb
        .ins()
        .load(types::I64, flags, ptr, object::obj_str_ptr_offset());
      let str_len = self
        .fb
        .ins()
        .load(types::I64, flags, ptr, object::obj_str_len_offset());
      // A Zuri string is indexed by CODEPOINT but stored as UTF-8, so
      // byte offset `i` only holds codepoint `i` when nothing before it
      // is multi-byte. `str_ascii_guard` is that whole-string fact,
      // which also makes the byte length below the codepoint count.
      let str_ascii_ok = self.str_ascii_guard(ptr);
      let str_bounds_block = self.fb.create_block();
      self
        .fb
        .ins()
        .brif(str_ascii_ok, str_bounds_block, &[], slow_block, &[]);

      self.fb.switch_to_block(str_bounds_block);
      let str_in_bounds = self.fb.ins().icmp(IntCC::UnsignedLessThan, as_int, str_len);
      let str_fast_block = self.fb.create_block();
      let str_check_neg_block = self.fb.create_block();
      self
        .fb
        .ins()
        .brif(str_in_bounds, str_fast_block, &[], str_check_neg_block, &[]);

      self.fb.switch_to_block(str_check_neg_block);
      let zero = self.i64c(0);
      let str_is_neg = self.fb.ins().icmp(IntCC::SignedLessThan, as_int, zero);
      let str_neg_block = self.fb.create_block();
      self
        .fb
        .ins()
        .brif(str_is_neg, str_neg_block, &[], slow_block, &[]);

      self.fb.switch_to_block(str_neg_block);
      let str_adjusted = self.fb.ins().iadd(str_len, as_int);
      let str_neg_in_bounds = self
        .fb
        .ins()
        .icmp(IntCC::UnsignedLessThan, str_adjusted, str_len);
      let str_neg_fast_block = self.fb.create_block();
      self
        .fb
        .ins()
        .brif(str_neg_in_bounds, str_neg_fast_block, &[], slow_block, &[]);

      self.fb.switch_to_block(str_neg_fast_block);
      let byte_addr_neg = self.fb.ins().iadd(str_data_ptr, str_adjusted);
      let byte_val8_neg = self.fb.ins().load(types::I8, flags, byte_addr_neg, 0);
      let byte_val64_neg = self.fb.ins().uextend(types::I64, byte_val8_neg);
      let ascii_load_block_neg = self.fb.create_block();
      self.fb.ins().jump(ascii_load_block_neg, &[]);

      self.fb.switch_to_block(ascii_load_block_neg);
      let interned_offset_neg = self.fb.ins().imul_imm_s(byte_val64_neg, 8);
      let vm_base = self.vm_param;
      let interned_table_addr_neg = self
        .fb
        .ins()
        .iadd_imm_s(vm_base, INTERNED_ASCII_OFFSET as i64);
      let elem_addr_neg = self
        .fb
        .ins()
        .iadd(interned_table_addr_neg, interned_offset_neg);
      let char_val_neg = self.fb.ins().load(types::I64, flags, elem_addr_neg, 0);
      self.store_reg(dst, char_val_neg);
      self.fb.ins().jump(done_block, &[]);

      self.fb.switch_to_block(str_fast_block);
      let byte_addr = self.fb.ins().iadd(str_data_ptr, as_int);
      let byte_val8 = self.fb.ins().load(types::I8, flags, byte_addr, 0);
      let byte_val64 = self.fb.ins().uextend(types::I64, byte_val8);
      let ascii_load_block = self.fb.create_block();
      self.fb.ins().jump(ascii_load_block, &[]);

      self.fb.switch_to_block(ascii_load_block);
      let interned_offset = self.fb.ins().imul_imm_s(byte_val64, 8);
      let vm_base = self.vm_param;
      let interned_table_addr = self
        .fb
        .ins()
        .iadd_imm_s(vm_base, INTERNED_ASCII_OFFSET as i64);
      let elem_addr = self.fb.ins().iadd(interned_table_addr, interned_offset);
      let char_val = self.fb.ins().load(types::I64, flags, elem_addr, 0);
      self.store_reg(dst, char_val);
      self.fb.ins().jump(done_block, &[]);
      (ptr, as_int)
    };

    self.fb.switch_to_block(resolve_block);
    let (data_ptr, len) = self.load_list_ptr_len(ptr);
    let in_bounds = self.fb.ins().icmp(IntCC::UnsignedLessThan, as_int, len);

    let fast_block = self.fb.create_block();
    let check_neg_block = self.fb.create_block();
    self
      .fb
      .ins()
      .brif(in_bounds, fast_block, &[], check_neg_block, &[]);

    self.fb.switch_to_block(check_neg_block);
    let zero = self.i64c(0);
    let is_neg = self.fb.ins().icmp(IntCC::SignedLessThan, as_int, zero);
    let neg_block = self.fb.create_block();
    self.fb.ins().brif(is_neg, neg_block, &[], slow_block, &[]);

    self.fb.switch_to_block(neg_block);
    let adjusted = self.fb.ins().iadd(len, as_int);
    let neg_in_bounds = self.fb.ins().icmp(IntCC::UnsignedLessThan, adjusted, len);
    let neg_fast_block = self.fb.create_block();
    self
      .fb
      .ins()
      .brif(neg_in_bounds, neg_fast_block, &[], slow_block, &[]);

    self.fb.switch_to_block(neg_fast_block);
    let byte_off_neg = self.fb.ins().imul_imm_s(adjusted, 8);
    let elem_addr_neg = self.fb.ins().iadd(data_ptr, byte_off_neg);
    let v_neg = self.fb.ins().load(
      types::I64,
      cranelift_codegen::ir::MemFlagsData::trusted(),
      elem_addr_neg,
      0,
    );
    self.store_reg(dst, v_neg);
    let seeds_int = self.int_tracked[dst as usize]
      && (self.proven_int_list(ip, obj)
        || (ip + 1 < self.proto.chunk.code.len() && self.int_facts.is_int(ip + 1, dst)));
    if seeds_int {
      let f = self.to_f64(v_neg);
      let iv = self.fb.ins().fcvt_to_sint_sat(types::I64, f);
      self.fb.def_var(self.reg_vars_int[dst as usize], iv);
    }
    self.fb.ins().jump(done_block, &[]);

    self.fb.switch_to_block(fast_block);
    let byte_off = self.fb.ins().imul_imm_s(as_int, 8);
    let elem_addr = self.fb.ins().iadd(data_ptr, byte_off);
    let v = self.fb.ins().load(
      types::I64,
      cranelift_codegen::ir::MemFlagsData::trusted(),
      elem_addr,
      0,
    );
    self.store_reg(dst, v);
    if seeds_int {
      let f = self.to_f64(v);
      let iv = self.fb.ins().fcvt_to_sint_sat(types::I64, f);
      self.fb.def_var(self.reg_vars_int[dst as usize], iv);
    }
    self.fb.ins().jump(done_block, &[]);

    // See `emit_list_set_index`'s own docs on why this reset (not just
    // the `restore_dirty_from_snapshot` at `done_block`) is needed:
    // `resolve_block`'s own `call_helper` above already mutated
    // `reg_cache` as a side effect of being GENERATED, regardless of
    // whether it ever runs at runtime; without resetting back to the
    // true pre-instruction state here, `slow_block`'s own
    // `call_checked` would see nothing left to flush and silently emit
    // no flush instructions at all, even on the (here, only) runtime
    // path where IT is the one that actually needs to.
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
    if seeds_int {
      let v = self.load_reg(dst);
      let f = self.to_f64(v);
      let iv = self.fb.ins().fcvt_to_sint_sat(types::I64, f);
      self.fb.def_var(self.reg_vars_int[dst as usize], iv);
    }
    self.fb.ins().jump(done_block, &[]);

    self.fb.switch_to_block(done_block);
  }

  /// `Instr::SetIndex`'s fast path; the write-side counterpart of
  /// `emit_list_get_index`: see its own docs for the shared reasoning,
  /// including why `reg_cache` needs a hard reset before `slow_block`
  /// (two independent `call_helper` sites: `zuri_jit_list_data` here,
  /// `zuri_jit_set_index` there).
  /// `x[i] = v` when `x` is already proven a byte stream; the store
  /// counterpart of `emit_proven_bytes_get_index`, and it drops the
  /// same object-shape guard for the same reason.
  ///
  /// No write barrier: a byte stream holds no references, so an old
  /// one can never come to point at a young object through this.
  fn emit_proven_bytes_set_index(
    &mut self,
    ip: usize,
    obj: u8,
    iidx: u8,
    src: u8,
    idx_proven_numeric: bool,
    idx_proven_int: bool,
  ) {
    let obj_val = self.load_reg(obj);
    let idx_val = self.load_reg(iidx);
    let src_val = self.load_reg(src);
    let ptr = self.obj_ptr(obj_val);

    let slow_block = self.fb.create_block();
    let done_block = self.fb.create_block();
    let resolve_block = self.fb.create_block();

    let f = self.f64_view(iidx, idx_val);
    let as_int = self.fb.ins().fcvt_to_sint_sat(types::I64, f);
    if idx_proven_int {
      self.fb.ins().jump(resolve_block, &[]);
    } else {
      let roundtrip = self.fb.ins().fcvt_from_sint(types::F64, as_int);
      let is_int = self.fb.ins().fcmp(
        cranelift_codegen::ir::condcodes::FloatCC::Equal,
        f,
        roundtrip,
      );
      let idx_ok = if idx_proven_numeric {
        is_int
      } else {
        let is_num = self.is_number(idx_val);
        self.fb.ins().band(is_num, is_int)
      };
      self
        .fb
        .ins()
        .brif(idx_ok, resolve_block, &[], slow_block, &[]);
    }

    self.fb.switch_to_block(resolve_block);
    let flags = cranelift_codegen::ir::MemFlagsData::trusted();
    let data_ptr = self
      .fb
      .ins()
      .load(types::I64, flags, ptr, object::obj_bytes_ptr_offset());
    let len = self
      .fb
      .ins()
      .load(types::I64, flags, ptr, object::obj_bytes_len_offset());
    let in_bounds = self.fb.ins().icmp(IntCC::UnsignedLessThan, as_int, len);
    let fast_block = self.fb.create_block();
    let check_neg_block = self.fb.create_block();
    self
      .fb
      .ins()
      .brif(in_bounds, fast_block, &[], check_neg_block, &[]);

    self.fb.switch_to_block(check_neg_block);
    let zero = self.i64c(0);
    let is_neg = self.fb.ins().icmp(IntCC::SignedLessThan, as_int, zero);
    let neg_block = self.fb.create_block();
    self.fb.ins().brif(is_neg, neg_block, &[], slow_block, &[]);

    self.fb.switch_to_block(neg_block);
    let adjusted = self.fb.ins().iadd(len, as_int);
    let neg_in_bounds = self.fb.ins().icmp(IntCC::UnsignedLessThan, adjusted, len);
    let neg_fast_block = self.fb.create_block();
    self
      .fb
      .ins()
      .brif(neg_in_bounds, neg_fast_block, &[], slow_block, &[]);

    self.fb.switch_to_block(neg_fast_block);
    if !self.proven_byte_value(ip, src) {
      let b = self.fb.create_block();
      self.emit_byte_value_guard(ip, src, src_val, b, slow_block);
      self.fb.switch_to_block(b);
    }
    let addr_neg = self.fb.ins().iadd(data_ptr, adjusted);
    let fn_neg = self.to_f64(src_val);
    let in_neg = self.fb.ins().fcvt_to_sint_sat(types::I64, fn_neg);
    let u8_neg = self.fb.ins().ireduce(types::I8, in_neg);
    self.fb.ins().store(flags, u8_neg, addr_neg, 0);
    self.fb.ins().jump(done_block, &[]);

    self.fb.switch_to_block(fast_block);
    if !self.proven_byte_value(ip, src) {
      let b = self.fb.create_block();
      self.emit_byte_value_guard(ip, src, src_val, b, slow_block);
      self.fb.switch_to_block(b);
    }
    let addr = self.fb.ins().iadd(data_ptr, as_int);
    let fnum = self.to_f64(src_val);
    let inum = self.fb.ins().fcvt_to_sint_sat(types::I64, fnum);
    let u8v = self.fb.ins().ireduce(types::I8, inum);
    self.fb.ins().store(flags, u8v, addr, 0);
    self.fb.ins().jump(done_block, &[]);

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
  }

  fn emit_list_set_index(
    &mut self,
    ip: usize,
    obj: u8,
    iidx: u8,
    src: u8,
    idx_proven_numeric: bool,
    idx_proven_int: bool,
  ) {
    let obj_val = self.load_reg(obj);
    let idx_val = self.load_reg(iidx);
    let src_val = self.load_reg(src);
    // See `emit_list_get_index`'s own docs on skipping the dynamic
    // `is_number` check when `type_facts` already proves it, and on
    // `typeflow::ListFacts` skipping the object-shape half entirely.
    let proven_list = self.proven_list(ip, obj);
    let proven_const_idx = self
      .proven_const(ip, iidx)
      .filter(|c| c.fract() == 0.0 && *c >= i64::MIN as f64 && *c <= i64::MAX as f64);
    let idx_proven_int = idx_proven_int || proven_const_idx.is_some();

    let slow_block = self.fb.create_block();
    let done_block = self.fb.create_block();
    let resolve_block = self.fb.create_block();

    let (ptr, as_int) = if proven_list {
      let ptr = self.obj_ptr(obj_val);
      let as_int = if let Some(c) = proven_const_idx {
        self.fb.ins().iconst(types::I64, c as i64)
      } else if idx_proven_int && self.int_tracked[iidx as usize] {
        self.fb.use_var(self.reg_vars_int[iidx as usize])
      } else {
        let f = self.f64_view(iidx, idx_val);
        self.fb.ins().fcvt_to_sint_sat(types::I64, f)
      };
      if idx_proven_int {
        self.fb.ins().jump(resolve_block, &[]);
      } else {
        let f = self.f64_view(iidx, idx_val);
        let roundtrip = self.fb.ins().fcvt_from_sint(types::F64, as_int);
        let is_int = self.fb.ins().fcmp(
          cranelift_codegen::ir::condcodes::FloatCC::Equal,
          f,
          roundtrip,
        );
        let idx_ok = if idx_proven_numeric {
          is_int
        } else {
          let is_num = self.is_number(idx_val);
          self.fb.ins().band(is_num, is_int)
        };
        self
          .fb
          .ins()
          .brif(idx_ok, resolve_block, &[], slow_block, &[]);
      }
      (ptr, as_int)
    } else {
      let is_obj = self.is_obj(obj_val);
      let cheap_guard = if idx_proven_numeric {
        is_obj
      } else {
        let is_num = self.is_number(idx_val);
        self.fb.ins().band(is_obj, is_num)
      };

      let checked_block = self.fb.create_block();
      self
        .fb
        .ins()
        .brif(cheap_guard, checked_block, &[], slow_block, &[]);

      self.fb.switch_to_block(checked_block);
      let ptr = self.obj_ptr(obj_val);
      let tag = self.obj_tag(ptr);
      let tag_list = self.i64c(object::OBJ_TAG_LIST as i64);
      let is_list = self.fb.ins().icmp(IntCC::Equal, tag, tag_list);

      let (as_int, f) = if let Some(c) = proven_const_idx {
        (self.fb.ins().iconst(types::I64, c as i64), None)
      } else if idx_proven_int && self.int_tracked[iidx as usize] {
        (self.fb.use_var(self.reg_vars_int[iidx as usize]), None)
      } else {
        let f = self.f64_view(iidx, idx_val);
        let as_int = self.fb.ins().fcvt_to_sint_sat(types::I64, f);
        (as_int, Some(f))
      };
      let check_bytes_set_block = self.fb.create_block();
      if idx_proven_int {
        self
          .fb
          .ins()
          .brif(is_list, resolve_block, &[], check_bytes_set_block, &[]);
      } else {
        let f = f.unwrap();
        let roundtrip = self.fb.ins().fcvt_from_sint(types::F64, as_int);
        let is_int = self.fb.ins().fcmp(
          cranelift_codegen::ir::condcodes::FloatCC::Equal,
          f,
          roundtrip,
        );
        let list_and_int = self.fb.ins().band(is_list, is_int);
        self
          .fb
          .ins()
          .brif(list_and_int, resolve_block, &[], check_bytes_set_block, &[]);
      }

      self.fb.switch_to_block(check_bytes_set_block);
      let tag_bytes = self.i64c(object::OBJ_TAG_BYTES as i64);
      let is_bytes = self.fb.ins().icmp(IntCC::Equal, tag, tag_bytes);
      let bytes_resolve_block = self.fb.create_block();
      if idx_proven_int {
        self
          .fb
          .ins()
          .brif(is_bytes, bytes_resolve_block, &[], slow_block, &[]);
      } else {
        let f = f.unwrap();
        let roundtrip = self.fb.ins().fcvt_from_sint(types::F64, as_int);
        let is_int = self.fb.ins().fcmp(
          cranelift_codegen::ir::condcodes::FloatCC::Equal,
          f,
          roundtrip,
        );
        let bytes_and_int = self.fb.ins().band(is_bytes, is_int);
        self
          .fb
          .ins()
          .brif(bytes_and_int, bytes_resolve_block, &[], slow_block, &[]);
      }

      self.fb.switch_to_block(bytes_resolve_block);
      let flags_set = cranelift_codegen::ir::MemFlagsData::trusted();
      let bytes_data_ptr =
        self
          .fb
          .ins()
          .load(types::I64, flags_set, ptr, object::obj_bytes_ptr_offset());
      let bytes_len =
        self
          .fb
          .ins()
          .load(types::I64, flags_set, ptr, object::obj_bytes_len_offset());
      let bytes_in_bounds = self
        .fb
        .ins()
        .icmp(IntCC::UnsignedLessThan, as_int, bytes_len);
      let bytes_fast_block = self.fb.create_block();
      let bytes_check_neg_block = self.fb.create_block();
      self.fb.ins().brif(
        bytes_in_bounds,
        bytes_fast_block,
        &[],
        bytes_check_neg_block,
        &[],
      );

      self.fb.switch_to_block(bytes_check_neg_block);
      let zero = self.i64c(0);
      let bytes_is_neg = self.fb.ins().icmp(IntCC::SignedLessThan, as_int, zero);
      let bytes_neg_block = self.fb.create_block();
      self
        .fb
        .ins()
        .brif(bytes_is_neg, bytes_neg_block, &[], slow_block, &[]);

      self.fb.switch_to_block(bytes_neg_block);
      let bytes_adjusted = self.fb.ins().iadd(bytes_len, as_int);
      let bytes_neg_in_bounds =
        self
          .fb
          .ins()
          .icmp(IntCC::UnsignedLessThan, bytes_adjusted, bytes_len);
      let bytes_neg_fast_block = self.fb.create_block();
      self.fb.ins().brif(
        bytes_neg_in_bounds,
        bytes_neg_fast_block,
        &[],
        slow_block,
        &[],
      );

      // A byte stream only accepts a whole number in 0..=255, and
      // `VM::index_set` raises otherwise. Without this the compiled
      // store truncated instead: `b[0] = 300` quietly wrote 44, and
      // only once the loop had gone hot, so the same code raised
      // while interpreted and corrupted after compiling.
      self.fb.switch_to_block(bytes_neg_fast_block);
      if !self.proven_byte_value(ip, src) {
        let neg_store_block = self.fb.create_block();
        self.emit_byte_value_guard(ip, src, src_val, neg_store_block, slow_block);
        self.fb.switch_to_block(neg_store_block);
      }
      let byte_elem_addr_neg = self.fb.ins().iadd(bytes_data_ptr, bytes_adjusted);
      let f_src_neg = self.to_f64(src_val);
      let i_src_neg = self.fb.ins().fcvt_to_sint_sat(types::I64, f_src_neg);
      let u8_src_neg = self.fb.ins().ireduce(types::I8, i_src_neg);
      self
        .fb
        .ins()
        .store(flags_set, u8_src_neg, byte_elem_addr_neg, 0);
      self.fb.ins().jump(done_block, &[]);

      self.fb.switch_to_block(bytes_fast_block);
      if !self.proven_byte_value(ip, src) {
        let pos_store_block = self.fb.create_block();
        self.emit_byte_value_guard(ip, src, src_val, pos_store_block, slow_block);
        self.fb.switch_to_block(pos_store_block);
      }
      let byte_elem_addr = self.fb.ins().iadd(bytes_data_ptr, as_int);
      let f_src = self.to_f64(src_val);
      let i_src = self.fb.ins().fcvt_to_sint_sat(types::I64, f_src);
      let u8_src = self.fb.ins().ireduce(types::I8, i_src);
      self.fb.ins().store(flags_set, u8_src, byte_elem_addr, 0);
      self.fb.ins().jump(done_block, &[]);
      (ptr, as_int)
    };

    self.fb.switch_to_block(resolve_block);
    let (data_ptr, len) = self.load_list_ptr_len(ptr);
    let in_bounds = self.fb.ins().icmp(IntCC::UnsignedLessThan, as_int, len);

    let fast_block = self.fb.create_block();
    let check_neg_block = self.fb.create_block();
    self
      .fb
      .ins()
      .brif(in_bounds, fast_block, &[], check_neg_block, &[]);

    self.fb.switch_to_block(check_neg_block);
    let zero = self.i64c(0);
    let is_neg = self.fb.ins().icmp(IntCC::SignedLessThan, as_int, zero);
    let neg_block = self.fb.create_block();
    self.fb.ins().brif(is_neg, neg_block, &[], slow_block, &[]);

    self.fb.switch_to_block(neg_block);
    let adjusted = self.fb.ins().iadd(len, as_int);
    let neg_in_bounds = self.fb.ins().icmp(IntCC::UnsignedLessThan, adjusted, len);
    let neg_fast_block = self.fb.create_block();
    self
      .fb
      .ins()
      .brif(neg_in_bounds, neg_fast_block, &[], slow_block, &[]);

    self.fb.switch_to_block(neg_fast_block);
    let byte_off_neg = self.fb.ins().imul_imm_s(adjusted, 8);
    let elem_addr_neg = self.fb.ins().iadd(data_ptr, byte_off_neg);
    self.fb.ins().store(
      cranelift_codegen::ir::MemFlagsData::trusted(),
      src_val,
      elem_addr_neg,
      0,
    );
    self.emit_write_barrier_for_store(ip, src, src_val, ptr);
    self.fb.ins().jump(done_block, &[]);

    self.fb.switch_to_block(fast_block);
    let byte_off = self.fb.ins().imul_imm_s(as_int, 8);
    let elem_addr = self.fb.ins().iadd(data_ptr, byte_off);
    self.fb.ins().store(
      cranelift_codegen::ir::MemFlagsData::trusted(),
      src_val,
      elem_addr,
      0,
    );
    self.emit_write_barrier_for_store(ip, src, src_val, ptr);
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
    // instruction at all; even when, at runtime, only the SECOND
    // block ever actually executes and the first's flush never ran.
    // Resetting back to the snapshot before generating EACH
    // independent branch (not just restoring once at the very end)
    // means every such branch's own `call_helper` sees the TRUE
    // pre-instruction state and emits exactly the flush it actually
    // needs, independent of what any sibling branch's codegen did.
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
  }

  /// Bounded so a single scalar-replaced allocation can't blow up this
  /// compiled function's own native stack usage for a pathological
  /// literal; matches the spirit (not the letter) of similar caps
  /// elsewhere in this file. Ordinary code overwhelmingly writes small,
  /// fixed-size list literals; anything bigger just falls through to
  /// the general, heap-allocating path unchanged.
  const MAX_SCALAR_LIST_LEN: u8 = 16;

  /// Is `Instr::MakeList{dst, start: _, count}` at `alloc_ip` safe to
  /// scalar-replace; keep its `count` elements as ordinary `Value`s
  /// in a Cranelift stack slot (see `emit_scalar_make_list`) instead of
  /// a real heap `Obj::List`, with `dst` never materialized as a
  /// tagged `Value` at all?
  ///
  /// Two independent conditions, both required:
  /// - `jit::escape::analyze_one` proves `dst` never escapes this
  ///   function (see that module's docs for exactly what's whitelisted
  ///  ; `GetIndex`/`SetIndex`'s container position already is, with
  ///   no extension needed here).
  /// - No `Instr::Move` ANYWHERE in this function ever reads `dst`.
  ///   `analyze_one`'s own dataflow WOULD correctly follow a `Move`
  ///   (propagating the "doesn't escape" proof to whatever register it
  ///   copies into), but this compiler's OWN codegen-time tracking
  ///   (`scalar_lists`) does NOT independently replicate that
  ///   propagation; deliberately: reimplementing the same
  ///   reachability/aliasing logic a second time, in a completely
  ///   separate piece of code, is exactly the kind of two-sources-of-
  ///   truth setup that has already produced one real, silent-
  ///   corruption bug (see `emit_list_get_index`). Ruling out `Move`
  ///   entirely, unconditionally
  ///   (not just on paths reachable from `alloc_ip`), is a safe, cheap
  ///   over-approximation instead: `scalar_lists` then only ever needs
  ///   to answer for the EXACT register `analyze_one` already reasoned
  ///   about, with no second analysis to keep in sync. Real code
  ///   essentially never copies a freshly-built temporary list into
  ///   another register before indexing it, so this costs nothing in
  ///   practice.
  ///
  /// - `self.speculative_regs` is `None`, OR no `typeflow::
  ///   conservative_dst`-covered instruction appears anywhere after
  ///   `alloc_ip`. Root cause, found 2026-08-20: a scalar-replaced
  ///   list's `dst` is NEVER materialized as a real `Value` in `VM::
  ///   registers` (the entire point of the optimization); but
  ///   `emit_speculative_guard` (the ONLY thing that can trigger a
  ///   MID-FUNCTION deopt, and it only ever fires for a
  ///   `conservative_dst`-covered instruction whose destination
  ///   `speculative_regs` bet on) resumes execution in the
  ///   INTERPRETER on a failed bet; which has no idea the register
  ///   ever held a list at all, since nothing was ever written there.
  ///   Confirmed by direct repro: a scalar-replaced list created
  ///   before an unrelated dict lookup whose result type varies at
  ///   runtime (always a number during warmup, a string on the
  ///   triggering call) corrupts the list's later reads with `cannot
  ///   index into a number` the instant the dict lookup's own
  ///   speculative guard deopts. `speculative_params` ALONE (no
  ///   `speculative_regs`) cannot cause this: it only ever gates
  ///   `emit_entry_dispatch`'s ordinary/specialized ENTRY choice, and
  ///   `type_facts`'s optimistic seeding for `conservative_dst`
  ///   instructions is gated on `speculative_regs` specifically (see
  ///   `transfer`'s own `conservative, speculative-eligible` arm) --
  ///   with it `None`, that seeding never fires, so `emit_speculative_
  ///   guard`'s own `type_facts.is_numeric` check is always false and
  ///   it never emits a guard at all, regardless of what instructions
  ///   exist. The scan below is a real, if conservative,
  ///   over-approximation of "no possible deopt while `dst` could
  ///   still be needed" (the true condition would need `dst`'s exact
  ///   last-use `ip`, not just "anywhere in the rest of the function")
  ///  ; cheap to check, and safe to be wrong in the "still
  ///   disallowed" direction. `tests/scalar-list-speculative-guard-scope.zu`
  ///   locks in the repro that found this.
  fn scalar_replace_eligible(&self, alloc_ip: usize, dst: u8, count: u8) -> bool {
    if count == 0 || count > Self::MAX_SCALAR_LIST_LEN {
      return false;
    }
    if self.speculative_regs.is_some()
      && self.proto.chunk.code[alloc_ip + 1..]
        .iter()
        .any(|instr| typeflow::conservative_dst(instr).is_some())
    {
      return false;
    }
    for instr in &self.proto.chunk.code {
      if let Instr::Move { src, .. } = instr
        && *src == dst
      {
        return false;
      }
    }
    let rewritten = self
      .proto
      .chunk
      .code
      .iter()
      .enumerate()
      .any(|(i, instr)| i != alloc_ip && typeflow::any_dst(instr) == Some(dst));
    if rewritten {
      return false;
    }
    !self.escape_analyze_one(alloc_ip, None, None).escapes
  }

  /// `Instr::MakeList{dst, start, count}`'s scalar-replaced fast path
  /// (see `scalar_replace_eligible`): copies the `count` source
  /// registers into a fresh Cranelift stack slot; ordinary `Value`s,
  /// never wrapped in a heap `Obj::List`; records `dst -> (slot,
  /// count)` in `scalar_lists` for `Instr::GetIndex`/`SetIndex` to
  /// consult, and registers the slot as a GC root.
  ///
  /// Registration happens LAST, strictly after every element slot has
  /// already been written: `VM::jit_scalar_roots`'s whole soundness
  /// argument depends on every slot a GC walk might visit already
  /// holding a valid `Value` (see that field's own docs); an
  /// uninitialized stack slot is neither `nil` nor any other valid tag
  /// pattern, and a GC safepoint CAN fire between two ordinary
  /// instructions (a nested call inside one of the source expressions,
  /// for instance), so there is a real window here to get right, not a
  /// theoretical one.
  fn emit_scalar_make_list(&mut self, dst: u8, start: u8, count: u8) {
    let slot = self
      .scalar_lists
      .get(&dst)
      .map(|&(s, _)| s)
      .unwrap_or_else(|| {
        let s = self.fb.create_sized_stack_slot(StackSlotData::new(
          StackSlotKind::ExplicitSlot,
          count as u32 * 8,
          3,
        ));
        self.scalar_lists.insert(dst, (s, count));
        s
      });
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
  }

  fn emit_inline_make_list(&mut self, _ip: usize, dst: u8, start: u8, count: u8) {
    let flags = cranelift_codegen::ir::MemFlagsData::trusted();
    let vm = self.vm_param;

    let cur = self
      .fb
      .ins()
      .load(types::I64, flags, vm, HEAP_NURSERY_CUR_OFFSET);
    let end = self
      .fb
      .ins()
      .load(types::I64, flags, vm, HEAP_NURSERY_END_OFFSET);
    let is_full = self.fb.ins().icmp(IntCC::Equal, cur, end);

    let fast_block = self.fb.create_block();
    let slow_block = self.fb.create_block();
    let done_block = self.fb.create_block();

    self
      .fb
      .ins()
      .brif(is_full, slow_block, &[], fast_block, &[]);

    // Fast path: inline nursery bump-allocation
    self.fb.switch_to_block(fast_block);
    let next_cur = self.fb.ins().iadd_imm_s(cur, 64);
    self
      .fb
      .ins()
      .store(flags, next_cur, vm, HEAP_NURSERY_CUR_OFFSET);

    // Update live_count
    let live = self
      .fb
      .ins()
      .load(types::I64, flags, vm, HEAP_LIVE_COUNT_OFFSET);
    let next_live = self.fb.ins().iadd_imm_s(live, 1);
    self
      .fb
      .ins()
      .store(flags, next_live, vm, HEAP_LIVE_COUNT_OFFSET);

    // Update young_bytes_allocated and bytes_allocated
    let size_bytes = 48 + (count as i64) * 8;
    let young = self
      .fb
      .ins()
      .load(types::I64, flags, vm, HEAP_YOUNG_BYTES_ALLOCATED_OFFSET);
    let next_young = self.fb.ins().iadd_imm_s(young, size_bytes);
    self
      .fb
      .ins()
      .store(flags, next_young, vm, HEAP_YOUNG_BYTES_ALLOCATED_OFFSET);

    let total = self
      .fb
      .ins()
      .load(types::I64, flags, vm, HEAP_BYTES_ALLOCATED_OFFSET);
    let next_total = self.fb.ins().iadd_imm_s(total, size_bytes);
    self
      .fb
      .ins()
      .store(flags, next_total, vm, HEAP_BYTES_ALLOCATED_OFFSET);

    // Update jit_gc_needed if young exceeded YOUNG_NEXT_GC. `icmp`
    // already yields i8 (see every other icmp-derived value in this
    // file, which only ever gets uextended UP to i64, never to i8);
    // extending it to its own type is a genuine Cranelift type error,
    // caught by the verifier in debug builds ("arg 0 with type i8
    // failed to satisfy type set") but silently accepted downstream
    // in release, where the verifier is off (see `JitEngine::new`).
    let young_limit = self.i64c(object::Heap::YOUNG_NEXT_GC as i64);
    let need_gc = self
      .fb
      .ins()
      .icmp(IntCC::UnsignedGreaterThan, next_young, young_limit);
    let curr_gc = self
      .fb
      .ins()
      .load(types::I8, flags, vm, HEAP_JIT_GC_NEEDED_OFFSET);
    let combined_gc = self.fb.ins().bor(curr_gc, need_gc);
    self
      .fb
      .ins()
      .store(flags, combined_gc, vm, HEAP_JIT_GC_NEEDED_OFFSET);

    // GcBox header (16 bytes at cur):
    // Word 0 (offset 0): live=1, marked=0, gen=Young(0), remembered=0, chunk_idx=0 -> 1u64
    let one64 = self.i64c(1);
    let zero64 = self.i64c(0);
    self.fb.ins().store(flags, one64, cur, 0);
    // Word 1 (offset 8): list_next = null -> 0u64
    self.fb.ins().store(flags, zero64, cur, 8);

    // Obj::List payload (48 bytes starting at cur + 16):
    // Word 2 (offset 16): Obj tag = OBJ_TAG_LIST (3) -> 3u64
    let tag_list = self.i64c(object::OBJ_TAG_LIST as i64);
    self.fb.ins().store(flags, tag_list, cur, 16);
    // Word 3 (offset 24): RefCell borrow flag = 0
    self.fb.ins().store(flags, zero64, cur, 24);
    // Word 4 (offset 32): ListStorage::ptr = null (inline elements)
    self.fb.ins().store(flags, zero64, cur, 32);
    // Word 5 (offset 40): len (u32) and cap=0 (u32)
    let len_cap = self.i64c(count as i64);
    self.fb.ins().store(flags, len_cap, cur, 40);

    // Load items BEFORE storing to dst (in case start or start+1 == dst)
    let item0 = if count >= 1 {
      self.load_reg(start)
    } else {
      self.u64c(value::NIL_VAL)
    };
    let item1 = if count >= 2 {
      self.load_reg(start + 1)
    } else {
      self.u64c(value::NIL_VAL)
    };
    // Word 6 (offset 48): inline[0]
    self.fb.ins().store(flags, item0, cur, 48);
    // Word 7 (offset 56): inline[1]
    self.fb.ins().store(flags, item1, cur, 56);

    // Tagged pointer: (cur + 16) | (QNAN | SIGN_BIT)
    let obj_ptr = self.fb.ins().iadd_imm_s(cur, 16);
    let tag_mask = self.u64c(value::QNAN | value::SIGN_BIT);
    let list_val = self.fb.ins().bor(obj_ptr, tag_mask);
    self.store_reg(dst, list_val);
    self.fb.ins().jump(done_block, &[]);

    // Slow path: out-of-line chunk refill / full allocation
    self.fb.switch_to_block(slow_block);
    let base = self.base_param;
    let dst_i = self.idx(dst);
    let start_i = self.idx(start);
    let count_i = self.idx(count);
    self.call_checked("zuri_jit_make_list", &[vm, base, dst_i, start_i, count_i]);
    self.resync_dst_from_memory(dst);
    self.fb.ins().jump(done_block, &[]);

    self.fb.switch_to_block(done_block);
  }

  /// `Instr::GetIndex`'s fast path when `obj` is a scalar-replaced
  /// list (`self.scalar_lists`); the SAME bounds-check shape as
  /// `emit_list_get_index` (negative-index wraparound, integer-value
  /// round-trip check), just against a COMPILE-TIME-KNOWN `count` and
  /// a directly-addressable stack slot instead of a runtime-resolved
  /// `Obj::List`: no `is_obj`/tag check at all, since `obj` being a key
  /// in `scalar_lists` already proves what it is.
  ///
  /// Still needs `snapshot_reg_cache`/`restore_dirty_from_snapshot`
  /// even though there's only ONE `call_helper` site here (unlike
  /// `emit_list_get_index`'s two); the relevant condition for needing
  /// this isn't "how many call_helper sites in this instruction," it's
  /// "does this instruction have a call_helper site that ONLY runs on
  /// a CONDITIONAL branch." `slow_block`'s `flush_live` mutates
  /// `reg_cache` (Dirty -> Clean) the moment its code is GENERATED,
  /// regardless of whether the runtime path taken is fast or slow; if
  /// left unrestored, a LATER instruction's own `flush_live`; even a
  /// completely unrelated one several instructions later; would see
  /// a register as already-Clean and skip flushing it, even on a
  /// runtime execution where THIS instruction actually took its fast
  /// path (which never flushes anything) and that register genuinely
  /// is still Dirty. Confirmed by direct reproduction: `tmp[0][0] +
  /// tmp[1][1] + tmp[2][0]` (three separate scalar-list `GetIndex`
  /// sites reading the same `tmp` in one expression) silently returned
  /// a stale value from an EARLIER site's fast-path store once a
  /// LATER site's slow-block flush was skipped this way.
  fn emit_scalar_list_get(
    &mut self,
    dst: u8,
    slot: StackSlot,
    count: u8,
    iidx: u8,
    idx_proven_numeric: bool,
    idx_proven_int: bool,
  ) {
    let idx_val = self.load_reg(iidx);
    let addr = self.fb.ins().stack_addr(types::I64, slot, 0);

    let checked_block = self.fb.create_block();
    let slow_block = self.fb.create_block();
    let done_block = self.fb.create_block();
    // Same discipline `emit_list_get_index`'s own `idx_proven_numeric`
    // skips: nothing left for this half of the guard to prove, so no
    // check, no branch; straight through.
    if idx_proven_numeric {
      self.fb.ins().jump(checked_block, &[]);
    } else {
      let is_num = self.is_number(idx_val);
      self
        .fb
        .ins()
        .brif(is_num, checked_block, &[], slow_block, &[]);
    }

    self.fb.switch_to_block(checked_block);
    let (as_int, f) = if idx_proven_int && self.int_tracked[iidx as usize] {
      (self.fb.use_var(self.reg_vars_int[iidx as usize]), None)
    } else {
      let f = self.f64_view(iidx, idx_val);
      let as_int = self.fb.ins().fcvt_to_sint_sat(types::I64, f);
      (as_int, Some(f))
    };

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
    // Same discipline `emit_list_get_index`'s own `idx_proven_int`
    // skips: `iidx` is proven a genuine whole number (`typeflow::
    // IntFacts`), so the float-roundtrip check has nothing left to
    // prove; only the bounds still matter.
    let ok = if idx_proven_int {
      in_bounds
    } else {
      let roundtrip = self.fb.ins().fcvt_from_sint(types::F64, as_int);
      let is_int = self.fb.ins().fcmp(
        cranelift_codegen::ir::condcodes::FloatCC::Equal,
        f.unwrap(),
        roundtrip,
      );
      self.fb.ins().band(is_int, in_bounds)
    };

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
    if self.int_tracked[dst as usize] {
      let f = self.to_f64(v);
      let iv = self.fb.ins().fcvt_to_sint_sat(types::I64, f);
      self.fb.def_var(self.reg_vars_int[dst as usize], iv);
    }
    self.fb.ins().jump(done_block, &[]);

    // See this function's own docs: undoes `checked_block`'s (never
    // actually flushing anything, so a no-op here) and, more
    // importantly, whatever an EARLIER instruction's own conditional
    // `call_helper` site left behind, before `slow_block`'s OWN
    // `flush_live` runs; otherwise it could see a register as
    // already-Clean from a branch that never actually executed.
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
    // registers`, bypassing `store_reg`/`reg_vars` entirely; matches
    // `emit_list_get_index`'s identical slow path, and for the exact
    // same reason: `call_checked`'s automatic `Stale` mark alone isn't
    // enough here, since `fast_block` DID call `store_reg` (a real
    // `def_var`), so without giving THIS block its own `def_var` too,
    // Cranelift's SSA merge at `done_block` would resolve `dst`'s
    // `Variable` to whatever dominating definition existed BEFORE this
    // instruction on this path; silently stale, not merely absent.
    self.resync_dst_from_memory(dst);
    if self.int_tracked[dst as usize] {
      let v = self.load_reg(dst);
      let f = self.to_f64(v);
      let iv = self.fb.ins().fcvt_to_sint_sat(types::I64, f);
      self.fb.def_var(self.reg_vars_int[dst as usize], iv);
    }
    self.fb.ins().jump(done_block, &[]);

    self.fb.switch_to_block(done_block);
  }

  /// `Instr::SetIndex`'s scalar-replaced fast path; the write-side
  /// counterpart of `emit_scalar_list_get`: see its own docs for the
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
  fn emit_scalar_list_set(
    &mut self,
    slot: StackSlot,
    count: u8,
    iidx: u8,
    src: u8,
    idx_proven_numeric: bool,
    idx_proven_int: bool,
  ) {
    let idx_val = self.load_reg(iidx);
    let src_val = self.load_reg(src);
    let addr = self.fb.ins().stack_addr(types::I64, slot, 0);

    let checked_block = self.fb.create_block();
    let slow_block = self.fb.create_block();
    let done_block = self.fb.create_block();
    if idx_proven_numeric {
      self.fb.ins().jump(checked_block, &[]);
    } else {
      let is_num = self.is_number(idx_val);
      self
        .fb
        .ins()
        .brif(is_num, checked_block, &[], slow_block, &[]);
    }

    self.fb.switch_to_block(checked_block);
    let (as_int, f) = if idx_proven_int && self.int_tracked[iidx as usize] {
      (self.fb.use_var(self.reg_vars_int[iidx as usize]), None)
    } else {
      let f = self.f64_view(iidx, idx_val);
      let as_int = self.fb.ins().fcvt_to_sint_sat(types::I64, f);
      (as_int, Some(f))
    };

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
    let ok = if idx_proven_int {
      in_bounds
    } else {
      let roundtrip = self.fb.ins().fcvt_from_sint(types::F64, as_int);
      let is_int = self.fb.ins().fcmp(
        cranelift_codegen::ir::condcodes::FloatCC::Equal,
        f.unwrap(),
        roundtrip,
      );
      self.fb.ins().band(is_int, in_bounds)
    };

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
  }

  /// `Instr::SetGlobal`/`Instr::AssignGlobal`'s inline-cache-style fast
  /// path; the write-side counterpart of `emit_get_global`, sharing
  /// its cache array (`JitInfo::global_slot_cache`) and the same
  /// unconditional flush/stale-mark discipline around the branch (see
  /// that method's own docs for why). No `resync_dst_from_memory` call
  /// is needed on the miss path here the way `emit_get_global` needs
  /// one for its `dst`; neither instruction defines a register at
  /// all, only reads `src` and writes to `VM::global_slots`, so there's
  /// no register-`Variable` SSA merge at `done_block` to keep
  /// consistent between the two paths.
  fn emit_set_global(&mut self, ip: usize, src: u8, name_const: u16, slow_helper: &'static str) {
    let flags = cranelift_codegen::ir::MemFlagsData::trusted();
    let slot = self.proto.jit.global_slot_cache[ip].get();
    if slot >= 0 {
      if let Some(&var) = self.global_vars.get(&slot) {
        let v = self.load_reg(src);
        self.fb.def_var(var, v);
        return;
      }
      let slots_ptr = self.fb.ins().load(
        types::I64,
        flags,
        self.vm_param,
        GLOBAL_SLOTS_PTR_CACHE_OFFSET,
      );
      let byte_off = (slot * 8) as i32;
      let v = self.load_reg(src);
      self.fb.ins().store(flags, v, slots_ptr, byte_off);
      return;
    }

    let cache_ptr = self.proto.jit.global_slot_cache.as_ptr() as i64;
    let cache_base = self.i64c(cache_ptr);
    let cached = self
      .fb
      .ins()
      .load(types::I64, flags, cache_base, (ip as i32) * 8);
    let neg1 = self.i64c(-1);
    let is_hit = self.fb.ins().icmp(IntCC::NotEqual, cached, neg1);

    let hit_block = self.fb.create_block();
    let miss_block = self.fb.create_block();
    let done_block = self.fb.create_block();
    self.fb.ins().brif(is_hit, hit_block, &[], miss_block, &[]);

    self.fb.switch_to_block(hit_block);
    let slots_ptr = self.fb.ins().load(
      types::I64,
      flags,
      self.vm_param,
      GLOBAL_SLOTS_PTR_CACHE_OFFSET,
    );
    let byte_off = self.fb.ins().imul_imm_s(cached, 8);
    let addr = self.fb.ins().iadd(slots_ptr, byte_off);
    let v = self.load_reg(src);
    self.fb.ins().store(flags, v, addr, 0);
    self.fb.ins().jump(done_block, &[]);

    self.fb.switch_to_block(miss_block);
    self.flush_live(ip);
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
    self.reload_live(ip);
    self.fb.ins().jump(done_block, &[]);

    self.fb.switch_to_block(done_block);
  }

  // The `GetField`/`SetField` inline fast path: see
  // `emit_ic_get_field`/`emit_ic_set_field` for the general receiver
  // and `emit_self_get_field`/`emit_self_set_field` for the
  // compile-time-resolvable `self.field` case. Both rely on
  // `resync_receiver_from_memory` to keep the receiver register
  // consistent across the guard's fast/slow split; don't drop that
  // call when touching this code.

  // ---------------------------------------------------------------
  // Guards
  // ---------------------------------------------------------------

  /// `(bits & QNAN) != QNAN`; `Value::is_number()`'s exact bit test
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

  /// `both_numbers`, but skipping the dynamic `is_number` check on
  /// whichever side `type_facts` already proves numeric; the same
  /// discipline `emit_list_get_index`'s own `idx_proven_numeric` flag
  /// established for the index operand (see `jit::typeflow::TypeFacts`'s
  /// own docs and the `GetIndex` fast path). Every guarded binary
  /// numeric op (`Add`/`Sub`/`Mul`/`Div`/`Pow`/`Floor`/`Mod`, the
  /// bitwise ops, `Eq`/`Neq`) only ever reaches its OWN `_guarded`
  /// codegen when `both_proven_numeric` was already false at that call
  /// site; meaning AT MOST one operand can still be unprovable here,
  /// never both by construction of how those call sites branch. Both
  /// `true` is handled anyway (a static `true`, no check at all) purely
  /// so this function is correct on its own terms regardless of what a
  /// caller passes, not because any current caller reaches it.
  fn combined_numeric_guard(
    &mut self,
    ip: usize,
    va: IrValue,
    vb: IrValue,
    a: u8,
    b: u8,
  ) -> IrValue {
    match (self.proven_numeric(ip, a), self.proven_numeric(ip, b)) {
      (true, true) => self.fb.ins().iconst(types::I8, 1),
      (true, false) => self.is_number(vb),
      (false, true) => self.is_number(va),
      (false, false) => self.both_numbers(va, vb),
    }
  }

  /// `(bits & (QNAN|SIGN_BIT)) == (QNAN|SIGN_BIT)`; `Value::is_obj()`'s
  /// exact bit test (see `value.rs`). Safe to inline because it only
  /// inspects the tagged `u64` itself, never dereferences anything.
  /// Telling WHICH heap type a confirmed object is needs a real
  /// dereference: see `obj_ptr`/`obj_tag` for the (now sound, since
  /// `Obj` is `#[repr(C, u8)]`) way to do that inline too.
  fn is_obj(&mut self, v: IrValue) -> IrValue {
    let mask = self.u64c(value::QNAN | value::SIGN_BIT);
    let masked = self.fb.ins().band(v, mask);
    self.fb.ins().icmp(IntCC::Equal, masked, mask)
  }

  /// Recovers the raw `*const Obj` pointer from a tagged `Value` known
  /// (by an already-checked `is_obj`) to actually hold one; the exact
  /// inverse of `Value::obj`'s own tagging (`SIGN_BIT | QNAN | ptr`),
  /// masking the tag bits back off. Callers must not call this on a
  /// `Value` that hasn't already been proven `is_obj`; the result is
  /// garbage (though not unsound to COMPUTE; it's only unsound to
  /// DEREFERENCE) otherwise.
  fn obj_ptr(&mut self, v: IrValue) -> IrValue {
    let mask = self.u64c(value::PTR_MASK);
    self.fb.ins().band(v, mask)
  }

  /// Reads `Obj`'s own tag byte straight out of memory; sound only
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

  /// If `v` is itself the result of a `bitcast` from `want`, the value
  /// that bitcast started from; `None` otherwise.
  ///
  /// Cranelift has no rule anywhere that cancels a `bitcast` pair, and
  /// on x86-64 each half is a real cross-domain `movq`, so a value that
  /// gets converted and immediately converted back pays for a round
  /// trip through the other register file for nothing. Plenty of code
  /// here hands raw bits to a helper that wants a float (or the other
  /// way round) without either side being in a position to know the
  /// conversion is about to be undone, so it's cheaper to catch it in
  /// one place than to thread the float form through every signature.
  fn bitcast_source(&self, v: IrValue, want: types::Type) -> Option<IrValue> {
    let dfg = &self.fb.func.dfg;
    let cranelift_codegen::ir::ValueDef::Result(inst, 0) = dfg.value_def(v) else {
      return None;
    };
    if dfg.insts[inst].opcode() != cranelift_codegen::ir::Opcode::Bitcast {
      return None;
    }
    let src = dfg.inst_args(inst).first().copied()?;
    (dfg.value_type(src) == want).then_some(src)
  }

  fn to_f64(&mut self, bits: IrValue) -> IrValue {
    if let Some(f) = self.bitcast_source(bits, types::F64) {
      return f;
    }
    self
      .fb
      .ins()
      .bitcast(types::F64, cranelift_codegen::ir::MemFlagsData::new(), bits)
  }

  fn from_f64(&mut self, f: IrValue) -> IrValue {
    if let Some(bits) = self.bitcast_source(f, types::I64) {
      return bits;
    }
    self
      .fb
      .ins()
      .bitcast(types::I64, cranelift_codegen::ir::MemFlagsData::new(), f)
  }

  /// Wraps a boolean condition into a Zuri `Value` bit pattern (`nil`/
  /// `true`/`false` share the same `QNAN`-tagged layout: see
  /// `value.rs`): `cond` selects between the baked `TRUE_VAL`/
  /// `FALSE_VAL` constants directly, with no branch at all.
  fn bool_value(&mut self, cond: IrValue) -> IrValue {
    let t = self.u64c(value::TRUE_VAL);
    let f = self.u64c(value::FALSE_VAL);
    self.fb.ins().select(cond, t, f)
  }

  /// `Value::is_falsey()`; fully inlined, no helper call at all,
  /// UNLESS `cond`'s register holds a heap object at runtime (a
  /// bigint/string/bytes might be empty-and-therefore-falsey; any other
  /// heap type never is: see `value.rs`'s own `is_falsey` doc
  /// comment). `Value`'s tag space is exactly {number, nil, true,
  /// false, object}, and only the "object" case needs a real
  /// dereference to resolve; nil/bool/number are each decidable from
  /// the bit pattern alone: `nil` and `false` are exact bit-pattern
  /// matches, and a number is falsey iff it's `<= 0.0` (real IEEE-754
  /// comparison, not a bit compare, to get `-0.0`/NaN right). This is
  /// the single hottest check in the whole VM (every loop condition and
  /// `if` goes through it), so avoiding a real function call for the
  /// overwhelming majority of cases (a loop counter, a comparison
  /// result, ...) matters far more here than for most other ops.
  fn emit_is_falsey(&mut self, cond: u8) -> IrValue {
    if self.bool_facts.is_bool(self.current_ip, cond) {
      let v = self.load_reg(cond);
      let false_val = self.u64c(value::FALSE_VAL);
      let is_false = self.fb.ins().icmp(IntCC::Equal, v, false_val);
      return self.fb.ins().uextend(types::I64, is_false);
    }

    if self.proven_numeric(self.current_ip, cond) {
      let fv = self.load_reg_f64(cond);
      let zero_f = self.fb.ins().f64const(0.0);
      let le_zero = self.fb.ins().fcmp(
        cranelift_codegen::ir::condcodes::FloatCC::LessThanOrEqual,
        fv,
        zero_f,
      );
      return self.fb.ins().uextend(types::I64, le_zero);
    }

    let v = self.load_reg(cond);
    let is_obj = self.is_obj(v);
    let result_var = self.fb.declare_var(types::I64);

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
    // BigInt; every other heap kind (List, Dict, Instance, Closure,
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
    let vm_p = self.vm_param;
    // The already-loaded value, not `cond`'s register index: this goes
    // through `call_helper_raw`, which does not flush, so the register
    // may still hold what it held before this instruction. See
    // `zuri_jit_is_falsey`'s own docs.
    let falsey = self.call_helper_raw("zuri_jit_is_falsey", &[vm_p, v]);
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
    self.fb.use_var(result_var)
  }

  // ---------------------------------------------------------------
  // Per-instruction codegen. Returns `true` if the instruction's own
  // codegen already ends in a terminator (so `run`'s driver loop must
  // NOT append an automatic fallthrough jump), `false` otherwise.
  // ---------------------------------------------------------------

  fn emit_instruction(&mut self, ip: usize, instr: Instr) -> bool {
    // See `current_ip`'s own docs; every nested helper/`call_indirect`
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
    // fall-through edges, `emit_safepoint` for back edges); this
    // block itself must NOT also flush via `use_var`: on the back
    // edge specifically, that would read whatever Cranelift's own SSA
    // construction is holding for the `Variable` in a machine
    // register/spill slot, a location GC cannot see or fix up, and
    // writing it back over memory can clobber a relocation that
    // already ran (see `flush_before_jump`'s own docs for the full
    // reasoning; this was a real, confirmed bug, not a hypothetical
    // one). By the time control reaches here via ANY edge, memory is
    // already correct; all that's needed is invalidating this
    // compiler's OWN bookkeeping so later code in/after this block
    // re-reads it instead of trusting a stale `Variable`.
    match instr {
      Instr::LoadConst { dst, const_idx } => {
        let const_val = self.proto.chunk.constants[const_idx as usize];
        if const_val.is_number() {
          let num = const_val.as_number();
          if num.fract() == 0.0
            && num >= (i64::MIN as f64)
            && num <= (i64::MAX as f64)
            && self.int_tracked[dst as usize]
          {
            let iv = self.fb.ins().iconst(types::I64, num as i64);
            self.fb.def_var(self.reg_vars_int[dst as usize], iv);
          }
          if self.is_f64_tracked(dst) {
            let f = self.fb.ins().f64const(num);
            self.store_reg_f64(dst, f);
          } else {
            let v = self.bake_const(const_idx);
            self.store_reg(dst, v);
          }
        } else {
          let v = self.bake_const(const_idx);
          self.store_reg(dst, v);
        }
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
        if self.proven_int(ip, src) && self.int_tracked[dst as usize] {
          let iv = self.fb.use_var(self.reg_vars_int[src as usize]);
          self.fb.def_var(self.reg_vars_int[dst as usize], iv);
        }
        if self.is_f64_tracked(src) && self.is_f64_tracked(dst) {
          // `compute_f64_tracked` ties the two ends of a move
          // together, so this is the normal case for anything on a
          // float path: the float view copies across with no bitcast
          // at all, and `store_reg_f64` decides whether the integer
          // one is even worth maintaining.
          let f = self.load_reg_f64(src);
          self.store_reg_f64(dst, f);
        } else {
          let v = self.load_reg(src);
          self.store_reg(dst, v);
        }
        false
      },

      Instr::Add { dst, a, b } => {
        if self.both_proven_int(ip, a, b) {
          // The float result is the value; the integer variable is only
          // a cache for instructions wanting an `i64` operand, and is
          // filled only when one reads it. Taking the result from the
          // integer op instead would wrap past `i64`.
          self.emit_binary_numeric_proven(dst, a, b, |fc, fa, fb| fc.fb.ins().fadd(fa, fb));
          if self.int_tracked[dst as usize] {
            let ia = self.fb.use_var(self.reg_vars_int[a as usize]);
            let ib = self.fb.use_var(self.reg_vars_int[b as usize]);
            let ir = self.fb.ins().iadd(ia, ib);
            self.fb.def_var(self.reg_vars_int[dst as usize], ir);
          }
        } else if self.both_proven_numeric(ip, a, b) {
          self.emit_binary_numeric_proven(dst, a, b, |fc, fa, fb| fc.fb.ins().fadd(fa, fb));
        } else if self.both_proven_string(ip, a, b) {
          self.emit_str_add(ip, dst, a, b);
        } else {
          let va = self.load_reg(a);
          let vb = self.load_reg(b);
          let na = self.is_number(va);
          let nb = self.is_number(vb);
          let both_num = self.fb.ins().band(na, nb);

          let num_block = self.fb.create_block();
          let not_num_block = self.fb.create_block();
          let done_block = self.fb.create_block();
          self
            .fb
            .ins()
            .brif(both_num, num_block, &[], not_num_block, &[]);

          self.fb.switch_to_block(num_block);
          let fa = self.f64_view(a, va);
          let fb_ = self.f64_view(b, vb);
          let res_f = self.fb.ins().fadd(fa, fb_);
          self.store_reg_f64(dst, res_f);
          self.fb.ins().jump(done_block, &[]);

          self.fb.switch_to_block(not_num_block);
          self.emit_str_add_dynamic(ip, dst, a, b, done_block);

          self.fb.switch_to_block(done_block);
        }
        false
      },
      Instr::Sub { dst, a, b } => {
        if self.both_proven_int(ip, a, b) {
          // The float result is the value; the integer variable is only
          // a cache for instructions wanting an `i64` operand, and is
          // filled only when one reads it. Taking the result from the
          // integer op instead would wrap past `i64`.
          self.emit_binary_numeric_proven(dst, a, b, |fc, fa, fb| fc.fb.ins().fsub(fa, fb));
          if self.int_tracked[dst as usize] {
            let ia = self.fb.use_var(self.reg_vars_int[a as usize]);
            let ib = self.fb.use_var(self.reg_vars_int[b as usize]);
            let ir = self.fb.ins().isub(ia, ib);
            self.fb.def_var(self.reg_vars_int[dst as usize], ir);
          }
        } else if self.both_proven_numeric(ip, a, b) {
          self.emit_binary_numeric_proven(dst, a, b, |fc, fa, fb| fc.fb.ins().fsub(fa, fb));
        } else {
          self.emit_binary_numeric_guarded(ip, dst, a, b, "zuri_jit_sub_slow", |fc, fa, fb| {
            fc.fb.ins().fsub(fa, fb)
          });
        }
        false
      },
      Instr::Mul { dst, a, b } => {
        if self.both_proven_int(ip, a, b) {
          // A Zuri number is an f64, so the float product is the value.
          // `imul` cannot stand in for it: it wraps past `i64` and has
          // no negative zero.
          //
          // The integer variable is a cache for instructions that want
          // an `i64` operand. It is filled by `imul` rather than by
          // converting the product back, since `fcvt_to_sint_sat` is a
          // saturating multi-instruction sequence and this is a hot
          // path, and only when a later instruction reads it.
          let fa = self.load_reg_f64(a);
          let fb_ = self.load_reg_f64(b);
          let prod = self.fb.ins().fmul(fa, fb_);
          if self.int_tracked[dst as usize] {
            let ia = self.fb.use_var(self.reg_vars_int[a as usize]);
            let ib = self.fb.use_var(self.reg_vars_int[b as usize]);
            let ir = self.fb.ins().imul(ia, ib);
            self.fb.def_var(self.reg_vars_int[dst as usize], ir);
          }
          self.store_reg_f64(dst, prod);
        } else if self.both_proven_numeric(ip, a, b) {
          self.emit_binary_numeric_proven(dst, a, b, |fc, fa, fb| fc.fb.ins().fmul(fa, fb));
        } else {
          self.emit_binary_numeric_guarded(ip, dst, a, b, "zuri_jit_mul_slow", |fc, fa, fb| {
            fc.fb.ins().fmul(fa, fb)
          });
        }
        false
      },
      Instr::Div { dst, a, b } => {
        if let Some(recip) = self.div_by_pow2_reciprocal(ip, b) {
          // See `div_by_pow2_reciprocal`'s own docs: `b`'s value is a
          // compile-time-known constant, so no guard on IT is needed at
          // all; only `a` still might not be numeric.
          if self.proven_numeric(ip, a) {
            self.emit_binary_numeric_proven(dst, a, b, move |fc, fa, _fb| {
              let r = fc.fb.ins().f64const(recip);
              fc.fb.ins().fmul(fa, r)
            });
          } else {
            self.emit_binary_numeric_guarded(
              ip,
              dst,
              a,
              b,
              "zuri_jit_div_slow",
              move |fc, fa, _fb| {
                let r = fc.fb.ins().f64const(recip);
                fc.fb.ins().fmul(fa, r)
              },
            );
          }
        } else if self.both_proven_numeric(ip, a, b) {
          self.emit_binary_numeric_proven(dst, a, b, |fc, fa, fb| fc.fb.ins().fdiv(fa, fb));
        } else {
          self.emit_binary_numeric_guarded(ip, dst, a, b, "zuri_jit_div_slow", |fc, fa, fb| {
            fc.fb.ins().fdiv(fa, fb)
          });
        }
        false
      },
      Instr::Pow { dst, a, b } => {
        if self.both_proven_numeric(ip, a, b) {
          self.emit_binary_numeric_proven(dst, a, b, |fc, fa, fb| {
            fc.call_f64_intrinsic("zuri_jit_num_powf", fa, fb)
          });
        } else {
          self.emit_binary_numeric_guarded(ip, dst, a, b, "zuri_jit_pow", |fc, fa, fb| {
            fc.call_f64_intrinsic("zuri_jit_num_powf", fa, fb)
          });
        }
        false
      },
      Instr::Floor { dst, a, b } => {
        if self.both_proven_numeric(ip, a, b) {
          self.emit_binary_numeric_proven(dst, a, b, |fc, fa, fb| {
            let q = fc.fb.ins().fdiv(fa, fb);
            fc.fb.ins().floor(q)
          });
        } else {
          self.emit_binary_numeric_guarded(ip, dst, a, b, "zuri_jit_floordiv", |fc, fa, fb| {
            let q = fc.fb.ins().fdiv(fa, fb);
            fc.fb.ins().floor(q)
          });
        }
        if self.both_proven_int(ip, a, b) {
          let f = self.load_reg_f64(dst);
          let iv = self.fb.ins().fcvt_to_sint_sat(types::I64, f);
          self.fb.def_var(self.reg_vars_int[dst as usize], iv);
        }
        false
      },
      Instr::Mod { dst, a, b } => {
        let va = self.load_reg(a);
        let vb = self.load_reg(b);
        let done_block = self.fb.create_block();

        if self.both_proven_numeric(ip, a, b) {
          let fa = self.load_reg_f64(a);
          let fb_ = self.load_reg_f64(b);
          let (ia, ib) = if self.both_proven_int(ip, a, b) {
            (
              self.fb.use_var(self.reg_vars_int[a as usize]),
              self.fb.use_var(self.reg_vars_int[b as usize]),
            )
          } else {
            (
              self.fb.ins().fcvt_to_sint_sat(types::I64, fa),
              self.fb.ins().fcvt_to_sint_sat(types::I64, fb_),
            )
          };
          let zero = self.i64c(0);
          let is_pos_denom = self.fb.ins().icmp(IntCC::SignedGreaterThan, ib, zero);

          let rem_block = self.fb.create_block();
          let fmod_block = self.fb.create_block();

          {
            // `srem` here produces the VALUE, so the integer operands
            // have to be the same numbers as the floats. A register can
            // be a proven int and still hold an integer too large for
            // `i64`, in which case its integer view is not that number.
            // The round trip catches that, and is only emitted for an
            // operand whose value is not already pinned to an in-range
            // integer by `ConstFacts`.
            let mut can_fast = is_pos_denom;
            for (reg, ival, fval) in [(a, ia, fa), (b, ib, fb_)] {
              if self.const_facts.const_value(ip, reg).is_some_and(|n| {
                n.fract() == 0.0 && n >= (i64::MIN as f64) && n <= (i64::MAX as f64)
              }) {
                continue;
              }
              let ok = if self.proven_int(ip, reg) {
                // Integrality is already established, so only the range
                // is in question. Testing the magnitude reads the float
                // alone, which keeps the check off the dependency chain
                // the divide is waiting on.
                let mag = self.fb.ins().fabs(fval);
                let limit = self.fb.ins().f64const(9223372036854775808.0);
                self.fb.ins().fcmp(
                  cranelift_codegen::ir::condcodes::FloatCC::LessThan,
                  mag,
                  limit,
                )
              } else {
                let rt = self.fb.ins().fcvt_from_sint(types::F64, ival);
                self
                  .fb
                  .ins()
                  .fcmp(cranelift_codegen::ir::condcodes::FloatCC::Equal, fval, rt)
              };
              can_fast = self.fb.ins().band(can_fast, ok);
            }
            self
              .fb
              .ins()
              .brif(can_fast, rem_block, &[], fmod_block, &[]);
          }

          self.fb.switch_to_block(rem_block);
          // `value::num_rem` is a truncated remainder carrying the
          // dividend's sign, which is what `srem` already computes.
          // `copysign` restores the sign of a zero result, the one
          // case `srem` cannot express; unconditionally, since a
          // compare and branch to skip one SSE bitwise op against an
          // in-flight divide costs more than the op.
          let rem = self.fb.ins().srem(ia, ib);
          let rem_f = self.fb.ins().fcvt_from_sint(types::F64, rem);
          let signed = self.fb.ins().fcopysign(rem_f, fa);
          if self.int_tracked[dst as usize] {
            self.fb.def_var(self.reg_vars_int[dst as usize], rem);
          }
          self.store_reg_f64(dst, signed);
          self.fb.ins().jump(done_block, &[]);

          self.fb.switch_to_block(fmod_block);
          let fallback = self.call_f64_intrinsic("zuri_jit_num_fmod", fa, fb_);
          self.store_reg_f64(dst, fallback);
          if self.both_proven_int(ip, a, b) {
            let iv = self.fb.ins().fcvt_to_sint_sat(types::I64, fallback);
            self.fb.def_var(self.reg_vars_int[dst as usize], iv);
          }
          self.fb.ins().jump(done_block, &[]);
        } else {
          let guard = self.combined_numeric_guard(ip, va, vb, a, b);
          let num_block = self.fb.create_block();
          let slow_block = self.fb.create_block();

          self.fb.ins().brif(guard, num_block, &[], slow_block, &[]);

          self.fb.switch_to_block(num_block);
          let fa = self.f64_view(a, va);
          let fb_ = self.f64_view(b, vb);
          let ia = self.fb.ins().fcvt_to_sint_sat(types::I64, fa);
          let ib = self.fb.ins().fcvt_to_sint_sat(types::I64, fb_);
          let zero = self.i64c(0);
          let is_pos_denom = self.fb.ins().icmp(IntCC::SignedGreaterThan, ib, zero);

          let rem_block = self.fb.create_block();
          let fmod_block = self.fb.create_block();

          {
            // `srem` here produces the VALUE, so the integer operands
            // have to be the same numbers as the floats. A register can
            // be a proven int and still hold an integer too large for
            // `i64`, in which case its integer view is not that number.
            // The round trip catches that, and is only emitted for an
            // operand whose value is not already pinned to an in-range
            // integer by `ConstFacts`.
            let mut can_fast = is_pos_denom;
            for (reg, ival, fval) in [(a, ia, fa), (b, ib, fb_)] {
              if self.const_facts.const_value(ip, reg).is_some_and(|n| {
                n.fract() == 0.0 && n >= (i64::MIN as f64) && n <= (i64::MAX as f64)
              }) {
                continue;
              }
              let ok = if self.proven_int(ip, reg) {
                // Integrality is already established, so only the range
                // is in question. Testing the magnitude reads the float
                // alone, which keeps the check off the dependency chain
                // the divide is waiting on.
                let mag = self.fb.ins().fabs(fval);
                let limit = self.fb.ins().f64const(9223372036854775808.0);
                self.fb.ins().fcmp(
                  cranelift_codegen::ir::condcodes::FloatCC::LessThan,
                  mag,
                  limit,
                )
              } else {
                let rt = self.fb.ins().fcvt_from_sint(types::F64, ival);
                self
                  .fb
                  .ins()
                  .fcmp(cranelift_codegen::ir::condcodes::FloatCC::Equal, fval, rt)
              };
              can_fast = self.fb.ins().band(can_fast, ok);
            }
            self
              .fb
              .ins()
              .brif(can_fast, rem_block, &[], fmod_block, &[]);
          }

          self.fb.switch_to_block(rem_block);
          // `value::num_rem` is a truncated remainder carrying the
          // dividend's sign, which is what `srem` already computes.
          // `copysign` restores the sign of a zero result, the one
          // case `srem` cannot express; unconditionally, since a
          // compare and branch to skip one SSE bitwise op against an
          // in-flight divide costs more than the op.
          let rem = self.fb.ins().srem(ia, ib);
          let rem_f = self.fb.ins().fcvt_from_sint(types::F64, rem);
          let signed = self.fb.ins().fcopysign(rem_f, fa);
          if self.int_tracked[dst as usize] {
            self.fb.def_var(self.reg_vars_int[dst as usize], rem);
          }
          self.store_reg_f64(dst, signed);
          self.fb.ins().jump(done_block, &[]);

          self.fb.switch_to_block(fmod_block);
          let fallback = self.call_f64_intrinsic("zuri_jit_num_fmod", fa, fb_);
          self.store_reg_f64(dst, fallback);
          if self.both_proven_int(ip, a, b) {
            let iv = self.fb.ins().fcvt_to_sint_sat(types::I64, fallback);
            self.fb.def_var(self.reg_vars_int[dst as usize], iv);
          }
          self.fb.ins().jump(done_block, &[]);

          self.fb.switch_to_block(slow_block);
          let base = self.base_param;
          let dst_i = self.idx(dst);
          let a_i = self.idx(a);
          let b_i = self.idx(b);
          self.call_checked("zuri_jit_mod", &[self.vm_param, base, dst_i, a_i, b_i]);
          self.resync_dst_from_memory(dst);
          if self.both_proven_int(ip, a, b) {
            let v = self.load_reg(dst);
            let f = self.to_f64(v);
            let iv = self.fb.ins().fcvt_to_sint_sat(types::I64, f);
            self.fb.def_var(self.reg_vars_int[dst as usize], iv);
          }
          self.fb.ins().jump(done_block, &[]);
        }

        self.fb.switch_to_block(done_block);
        false
      },

      Instr::BitAnd { dst, a, b } => {
        if self.both_proven_numeric(ip, a, b) {
          self.emit_bitwise_proven(ip, dst, a, b, |fb, ia, ib| fb.ins().band(ia, ib));
        } else {
          self.emit_bitwise_guarded(ip, dst, a, b, "zuri_jit_bitand_slow", |fb, ia, ib| {
            fb.ins().band(ia, ib)
          });
        }
        false
      },
      Instr::BitOr { dst, a, b } => {
        if self.both_proven_numeric(ip, a, b) {
          self.emit_bitwise_proven(ip, dst, a, b, |fb, ia, ib| fb.ins().bor(ia, ib));
        } else {
          self.emit_bitwise_guarded(ip, dst, a, b, "zuri_jit_bitor_slow", |fb, ia, ib| {
            fb.ins().bor(ia, ib)
          });
        }
        false
      },
      Instr::BitXor { dst, a, b } => {
        if self.both_proven_numeric(ip, a, b) {
          self.emit_bitwise_proven(ip, dst, a, b, |fb, ia, ib| fb.ins().bxor(ia, ib));
        } else {
          self.emit_bitwise_guarded(ip, dst, a, b, "zuri_jit_bitxor_slow", |fb, ia, ib| {
            fb.ins().bxor(ia, ib)
          });
        }
        false
      },
      Instr::BitShl { dst, a, b } => {
        if self.both_proven_numeric(ip, a, b) {
          self.emit_bitwise_proven(ip, dst, a, b, Self::shift_left);
        } else {
          self.emit_bitwise_guarded(ip, dst, a, b, "zuri_jit_bitshl", Self::shift_left);
        }
        false
      },
      Instr::BitShr { dst, a, b } => {
        if self.both_proven_numeric(ip, a, b) {
          self.emit_bitwise_proven(ip, dst, a, b, Self::shift_right);
        } else {
          self.emit_bitwise_guarded(ip, dst, a, b, "zuri_jit_bitshr", Self::shift_right);
        }
        false
      },
      Instr::BitUshr { dst, a, b } => {
        if self.both_proven_numeric(ip, a, b) {
          self.emit_bitwise_proven(ip, dst, a, b, Self::shift_right_unsigned);
        } else {
          self.emit_bitwise_guarded(
            ip,
            dst,
            a,
            b,
            "zuri_jit_bitushr",
            Self::shift_right_unsigned,
          );
        }
        false
      },
      Instr::BitNot { dst, src } => {
        if self.proven_int(ip, src) {
          let i = self.fb.use_var(self.reg_vars_int[src as usize]);
          let inv = self.fb.ins().bnot(i);
          self.store_reg_int(dst, inv);
        } else if self.proven_numeric(ip, src) {
          let f = self.load_reg_f64(src);
          let i = self.emit_narrow_to_i64(f);
          let inv = self.fb.ins().bnot(i);
          let r = self.fb.ins().fcvt_from_sint(types::F64, inv);
          self.store_reg_f64(dst, r);
          self.store_reg_int(dst, inv);
        } else {
          let v = self.load_reg(src);
          let is_num = self.is_number(v);
          let fast_block = self.fb.create_block();
          let slow_block = self.fb.create_block();
          let done_block = self.fb.create_block();
          self.fb.ins().brif(is_num, fast_block, &[], slow_block, &[]);

          self.fb.switch_to_block(fast_block);
          let f = self.f64_view(src, v);
          let i = self.fb.ins().fcvt_to_sint_sat(types::I64, f);
          let inv = self.fb.ins().bnot(i);
          let r = self.fb.ins().fcvt_from_sint(types::F64, inv);
          self.store_reg_f64(dst, r);
          self.store_reg_int(dst, inv);
          self.fb.ins().jump(done_block, &[]);

          self.fb.switch_to_block(slow_block);
          let base = self.base_param;
          let dst_i = self.idx(dst);
          let src_i = self.idx(src);
          self.call_checked("zuri_jit_bitnot_slow", &[self.vm_param, base, dst_i, src_i]);
          self.resync_dst_from_memory(dst);
          let v = self.load_reg(dst);
          let f = self.to_f64(v);
          let iv = self.fb.ins().fcvt_to_sint_sat(types::I64, f);
          self.fb.def_var(self.reg_vars_int[dst as usize], iv);
          self.fb.ins().jump(done_block, &[]);

          self.fb.switch_to_block(done_block);
        }
        false
      },
      Instr::Neg { dst, src } => {
        if self.proven_int(ip, src) {
          // `ineg(0)` is `0`, so converting THAT back gives `+0.0` and
          // loses the `-0.0` a negative-zero literal is made of (the
          // source spells it as unary minus on `0.0`). Negating the
          // float instead gets every case right including that one, and
          // it reads the source's INT variable rather than its float
          // one, so this does not force the float view live purely for
          // the sake of the sign.
          // Float negation is the value. `ineg` has no negative zero,
          // which a source-level `-0.0` needs, and no answer for
          // `i64::MIN`; it only fills the integer cache.
          let fs = self.load_reg_f64(src);
          let neg = self.fb.ins().fneg(fs);
          if self.int_tracked[dst as usize] {
            let is = self.fb.use_var(self.reg_vars_int[src as usize]);
            let ir = self.fb.ins().ineg(is);
            self.fb.def_var(self.reg_vars_int[dst as usize], ir);
          }
          self.store_reg_f64(dst, neg);
        } else if self.proven_numeric(ip, src) {
          let f = self.load_reg_f64(src);
          let neg = self.fb.ins().fneg(f);
          self.store_reg_f64(dst, neg);
        } else {
          let v = self.load_reg(src);
          let is_num = self.is_number(v);
          let fast_block = self.fb.create_block();
          let slow_block = self.fb.create_block();
          let done_block = self.fb.create_block();
          self.fb.ins().brif(is_num, fast_block, &[], slow_block, &[]);

          self.fb.switch_to_block(fast_block);
          let f = self.f64_view(src, v);
          let neg = self.fb.ins().fneg(f);
          self.store_reg_f64(dst, neg);
          self.fb.ins().jump(done_block, &[]);

          self.fb.switch_to_block(slow_block);
          let base = self.base_param;
          let dst_i = self.idx(dst);
          let src_i = self.idx(src);
          self.call_checked("zuri_jit_neg_slow", &[self.vm_param, base, dst_i, src_i]);
          self.resync_dst_from_memory(dst);
          self.fb.ins().jump(done_block, &[]);

          self.fb.switch_to_block(done_block);
        }
        false
      },
      Instr::Not { dst, src } => {
        // `emit_is_falsey` already IS the inline fast path (a helper
        // call only for the rare String/Bytes/BigInt-emptiness case) --
        // `!x` is just that result, wrapped as a bool `Value` instead of
        // branched on directly.
        let falsey = self.emit_is_falsey(src);
        let zero = self.i64c(0);
        let is_falsey = self.fb.ins().icmp(IntCC::NotEqual, falsey, zero);
        let v = self.bool_value(is_falsey);
        self.store_reg(dst, v);
        false
      },
      Instr::Concat { dst, a, b } => {
        self.emit_always_helper("zuri_jit_concat", dst, a, b);
        false
      },

      Instr::Eq { dst, a, b } => {
        if self.both_proven_numeric(ip, a, b) {
          self.emit_compare_proven_numeric(ip, dst, a, b, IntCC::Equal);
        } else {
          self.emit_compare_guarded(ip, dst, a, b, "zuri_jit_eq_slow", IntCC::Equal);
        }
        false
      },
      Instr::Neq { dst, a, b } => {
        if self.both_proven_numeric(ip, a, b) {
          self.emit_compare_proven_numeric(ip, dst, a, b, IntCC::NotEqual);
        } else {
          self.emit_compare_guarded(ip, dst, a, b, "zuri_jit_neq_slow", IntCC::NotEqual);
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
          if self.loop_has_allocations(target_ip, ip) {
            self.emit_safepoint();
          } else if crate::modules::os_util::signal::armed() {
            self.emit_signal_safepoint();
          }
        }
        let target_block = self.jump_target_block(ip, target_ip);
        self.fb.ins().jump(target_block, &[]);
        true
      },
      Instr::JmpIfFalse { cond, offset } => {
        let target_ip = (ip as isize + 1 + offset as isize) as usize;
        let target_block = self.jump_target_block(ip, target_ip);
        if offset < 0 {
          if self.loop_has_allocations(target_ip, ip) {
            self.emit_safepoint();
          } else if crate::modules::os_util::signal::armed() {
            self.emit_signal_safepoint();
          }
        }
        if self.bool_facts.is_bool(ip, cond) {
          let v = self.load_reg(cond);
          let false_val = self.u64c(value::FALSE_VAL);
          let is_false = self.fb.ins().icmp(IntCC::Equal, v, false_val);
          self
            .fb
            .ins()
            .brif(is_false, target_block, &[], self.blocks[ip + 1], &[]);
          return true;
        }
        let falsey = self.emit_is_falsey(cond);
        let zero = self.i64c(0);
        let is_falsey = self.fb.ins().icmp(IntCC::NotEqual, falsey, zero);
        self
          .fb
          .ins()
          .brif(is_falsey, target_block, &[], self.blocks[ip + 1], &[]);
        true
      },
      Instr::JmpIfTrue { cond, offset } => {
        let target_ip = (ip as isize + 1 + offset as isize) as usize;
        let target_block = self.jump_target_block(ip, target_ip);
        if offset < 0 {
          if self.loop_has_allocations(target_ip, ip) {
            self.emit_safepoint();
          } else if crate::modules::os_util::signal::armed() {
            self.emit_signal_safepoint();
          }
        }
        if self.bool_facts.is_bool(ip, cond) {
          let v = self.load_reg(cond);
          let true_val = self.u64c(value::TRUE_VAL);
          let is_true = self.fb.ins().icmp(IntCC::Equal, v, true_val);
          self
            .fb
            .ins()
            .brif(is_true, target_block, &[], self.blocks[ip + 1], &[]);
          return true;
        }
        let truthy = self.emit_is_falsey(cond);
        let zero = self.i64c(0);
        let is_truthy = self.fb.ins().icmp(IntCC::Equal, truthy, zero);
        self
          .fb
          .ins()
          .brif(is_truthy, target_block, &[], self.blocks[ip + 1], &[]);
        true
      },

      Instr::Call {
        dst,
        func,
        num_args,
      } => {
        // Inlining is checked BEFORE the safepoint, because an inlined
        // body emits no call, no allocation and no frame push; there
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
          // `CallTarget::Known::entry`); there is no address to jump
          // to, so this falls through to the ordinary resolver exactly
          // as an unresolved site would.
          Some(CallTarget::Known {
            entry,
            guard_bits,
            proto_ptr,
          }) if entry != 0 => {
            self.emit_known_call(dst, func, num_args, entry, guard_bits, proto_ptr)
          },
          Some(CallTarget::Known { .. }) => self.emit_generic_call(dst, func, num_args),
          Some(CallTarget::KnownNative {
            guard_fn,
            native_ptr,
          }) => self.emit_known_native_call(dst, func, num_args, guard_fn, native_ptr),
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
              ip,
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
        self.flush_globals();
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
          &[self.vm_param, base, dst_i, proto_v],
        );
        self.resync_dst_from_memory(dst);
        false
      },
      Instr::GetUpval { dst, idx: uidx } => {
        self.emit_get_upval_fast(dst, uidx);
        false
      },
      Instr::SetUpval { idx: uidx, src } => {
        self.emit_set_upval_fast(ip, uidx, src);
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
        if count <= 2 {
          self.emit_inline_make_list(ip, dst, start, count);
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
        self.resync_dst_from_memory(dst);
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
        self.resync_dst_from_memory(dst);
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
        let func_ptr = self.func_ptr_const();
        self.call_checked(
          "zuri_jit_make_class",
          &[
            self.vm_param,
            base,
            dst_i,
            name,
            has_super,
            super_reg,
            func_ptr,
          ],
        );
        self.resync_dst_from_memory(dst);
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
        // A scalar-replaced instance has no object to read from; the
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
          self.emit_self_get_field(ip, dst, obj, name_const, slot, false);
          return false;
        }
        if let Some(slot) = self.param_field_slot(obj, name_const) {
          self.emit_self_get_field(ip, dst, obj, name_const, slot, true);
          return false;
        }
        if let Some((bits, slot)) = self.construct_field_slot(ip, obj, name_const) {
          self.emit_construct_field(ip, Some(dst), obj, name_const, None, bits, slot);
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
        self.resync_dst_from_memory(dst);
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
          self.emit_self_set_field(ip, obj, name_const, src, slot, false);
          return false;
        }
        if let Some(slot) = self.param_field_slot(obj, name_const) {
          self.emit_self_set_field(ip, obj, name_const, src, slot, true);
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
        if num_args == 1 {
          if let Some(op) = IterableIntrinsic::of(self.method_name(method_const)) {
            self.emit_iterable_intrinsic(ip, dst, obj, method_const, op);
            return false;
          }
        }
        // Checked before `NumberIntrinsic`/`ListIntrinsic`: those two
        // match purely on METHOD NAME, not receiver type, so a proven
        // string calling e.g. `.length()` (a `ListIntrinsic` name too)
        // would otherwise still walk into `emit_list_intrinsic`, pay
        // for its own `proven_list`-false runtime guard, and land in
        // that guard's slow arm anyway; strictly more work than
        // going straight to a proven string's own fast paths
        // (`Value::String` is never a number or a list, so neither
        // intrinsic could ever legally apply here regardless of name).
        if self.proven_string(ip, obj) {
          if let Some(op) = StringIntrinsic::of(self.method_name(method_const))
            && op.arity() == num_args
          {
            self.emit_string_intrinsic(ip, dst, obj, method_const, num_args, op);
          } else {
            self.emit_safepoint();
            self.emit_string_invoke(ip, dst, obj, method_const, num_args);
          }
          return false;
        }
        if let Some(op) = NumberIntrinsic::of(self.method_name(method_const))
          && op.arity() == num_args
        {
          self.emit_number_intrinsic(ip, dst, obj, method_const, num_args, op);
          return false;
        }
        if let Some(op) = ListIntrinsic::of(self.method_name(method_const))
          && op.arity() == num_args
        {
          if matches!(op, ListIntrinsic::Append) {
            self.emit_list_append(ip, dst, obj, method_const);
          } else {
            self.emit_list_intrinsic(ip, dst, obj, method_const, num_args, op);
          }
          return false;
        }
        if let Some(op) = StringIntrinsic::of(self.method_name(method_const))
          && op.arity() == num_args
        {
          self.emit_string_intrinsic(ip, dst, obj, method_const, num_args, op);
          return false;
        }
        if let Some(op) = DictIntrinsic::of(self.method_name(method_const))
          && op.accepts_arity(num_args)
        {
          self.emit_dict_intrinsic(ip, dst, obj, method_const, num_args, op);
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
        self.resync_dst_from_memory(dst);
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
        self.resync_dst_from_memory(dst);
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
        self.resync_dst_from_memory(dst);
        false
      },
      Instr::ImportAll { module, exported } => {
        let base = self.base_param;
        let module_i = self.idx(module);
        let func_ptr = self.func_ptr_const();
        let exported_i = self.i64c(exported as i64);
        self.call_checked(
          "zuri_jit_import_all",
          &[self.vm_param, base, module_i, func_ptr, exported_i],
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
        self.resync_dst_from_memory(dst);
        false
      },

      Instr::GetIndex {
        dst,
        obj,
        idx: iidx,
      } => {
        if let Some(&(slot, count)) = self.scalar_lists.get(&obj) {
          self.emit_scalar_list_get(
            dst,
            slot,
            count,
            iidx,
            self.proven_numeric(ip, iidx),
            self.proven_int(ip, iidx),
          );
          return false;
        }
        if self.proven_string(ip, obj) {
          self.emit_str_get_index(
            ip,
            dst,
            obj,
            iidx,
            self.proven_numeric(ip, iidx),
            self.proven_int(ip, iidx),
          );
          return false;
        }
        // A dict lookup is a hash probe whichever way it is reached,
        // so there is nothing to inline; what a proven receiver buys
        // is skipping the kind dispatch in front of it. The helper
        // raises the missing-key error itself, matching
        // `VM::index_get`.
        if self.proven_dict(ip, obj) {
          let base = self.base_param;
          let dst_i = self.idx(dst);
          let obj_i = self.idx(obj);
          let idx_i = self.idx(iidx);
          self.call_checked(
            "zuri_jit_dict_index_get",
            &[self.vm_param, base, dst_i, obj_i, idx_i],
          );
          self.resync_dst_from_memory(dst);
          return false;
        }
        if self.proven_bytes(ip, obj) {
          self.emit_proven_bytes_get_index(
            ip,
            dst,
            obj,
            iidx,
            self.proven_numeric(ip, iidx),
            self.proven_int(ip, iidx),
          );
          return false;
        }
        self.emit_list_get_index(
          ip,
          dst,
          obj,
          iidx,
          self.proven_numeric(ip, iidx),
          self.proven_int(ip, iidx),
        );
        false
      },
      Instr::SetIndex {
        obj,
        idx: iidx,
        src,
      } => {
        if let Some(&(slot, count)) = self.scalar_lists.get(&obj) {
          self.emit_scalar_list_set(
            slot,
            count,
            iidx,
            src,
            self.proven_numeric(ip, iidx),
            self.proven_int(ip, iidx),
          );
          return false;
        }
        // A dict takes any key and any value and cannot raise, so a
        // proven dict needs no guard whatsoever: straight to the same
        // lean helper `DictIntrinsic::Set` uses. `counts[k] = ...` on
        // a `MakeDict` local is the shape this is for.
        if self.proven_dict(ip, obj) {
          let recv = self.load_reg(obj);
          let key = self.load_reg(iidx);
          let value = self.load_reg(src);
          let vm_p = self.vm_param;
          self.call_helper_raw("zuri_jit_dict_set", &[vm_p, recv, key, value]);
          return false;
        }
        if self.proven_bytes(ip, obj) {
          self.emit_proven_bytes_set_index(
            ip,
            obj,
            iidx,
            src,
            self.proven_numeric(ip, iidx),
            self.proven_int(ip, iidx),
          );
          return false;
        }
        self.emit_list_set_index(
          ip,
          obj,
          iidx,
          src,
          self.proven_numeric(ip, iidx),
          self.proven_int(ip, iidx),
        );
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
        self.resync_dst_from_memory(dst);
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
        self.resync_dst_from_memory(dst);
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
        // position; unlike every other jump, the destination isn't
        // known until runtime, so this can't be a direct `Block`
        // branch. Route through a tiny indirect trampoline instead:
        // `br_table`-free by construction (avoids that API's exact
        // shape entirely); a chain comparing the returned target ip
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

      // See the eligibility scan in `compile`: reaching a raise means
      // leaving compiled code entirely, and the interpreter re-executes
      // this instruction with the register state flushed here.
      Instr::Raise { .. } => {
        self.emit_deopt(ip);
        true
      },

      Instr::PushCatch { .. } | Instr::PopCatch => {
        unreachable!("excluded by the eligibility scan in `compile`")
      },

      Instr::AddImm { dst, a, imm_const } => {
        let imm_val = self.proto.chunk.constants[imm_const as usize].as_number();
        if self.proven_int(ip, a)
          && imm_val.fract() == 0.0
          && imm_val > (i64::MIN as f64)
          && imm_val <= (i64::MAX as f64)
        {
          // The float result is the value; the integer variable is only
          // a cache for instructions wanting an `i64` operand, and is
          // filled only when one reads it. Taking the result from the
          // integer op instead would wrap past `i64`.
          self.emit_addimm_proven(dst, a, imm_const);
          if self.int_tracked[dst as usize] {
            let ia = self.fb.use_var(self.reg_vars_int[a as usize]);
            let ir = self.fb.ins().iadd_imm_s(ia, imm_val as i64);
            self.fb.def_var(self.reg_vars_int[dst as usize], ir);
          }
        } else if self.proven_numeric(ip, a) {
          self.emit_addimm_proven(dst, a, imm_const);
        } else {
          self.emit_addimm(dst, a, imm_const);
        }
        false
      },
      Instr::SubImm { dst, a, imm_const } => {
        let imm_val = self.proto.chunk.constants[imm_const as usize].as_number();
        if self.proven_int(ip, a)
          && imm_val.fract() == 0.0
          && imm_val > (i64::MIN as f64)
          && imm_val <= (i64::MAX as f64)
        {
          // The float result is the value; the integer variable is only
          // a cache for instructions wanting an `i64` operand, and is
          // filled only when one reads it. Taking the result from the
          // integer op instead would wrap past `i64`.
          self
            .emit_imm_numeric_proven(dst, a, imm_const, |fc, fa, fimm| fc.fb.ins().fsub(fa, fimm));
          if self.int_tracked[dst as usize] {
            let ia = self.fb.use_var(self.reg_vars_int[a as usize]);
            let ir = self.fb.ins().iadd_imm_s(ia, -(imm_val as i64));
            self.fb.def_var(self.reg_vars_int[dst as usize], ir);
          }
        } else if self.proven_numeric(ip, a) {
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
        let imm_val = self.proto.chunk.constants[imm_const as usize].as_number();
        if self.proven_int(ip, a)
          && imm_val.fract() == 0.0
          && imm_val > (i64::MIN as f64)
          && imm_val <= (i64::MAX as f64)
        {
          // Value from the float multiply, integer cache from `imul`,
          // for the reasons `Instr::Mul` gives.
          let fa = self.load_reg_f64(a);
          let fimm = self.bake_f64(a, imm_const);
          let prod = self.fb.ins().fmul(fa, fimm);
          if self.int_tracked[dst as usize] {
            let ia = self.fb.use_var(self.reg_vars_int[a as usize]);
            let ir = self.fb.ins().imul_imm_s(ia, imm_val as i64);
            self.fb.def_var(self.reg_vars_int[dst as usize], ir);
          }
          self.store_reg_f64(dst, prod);
        } else if self.proven_numeric(ip, a) {
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
          self.emit_imm_eq_proven(ip, dst, a, imm_const, true);
        } else {
          self.emit_imm_eq(dst, a, imm_const, true);
        }
        false
      },
      Instr::NeqImm { dst, a, imm_const } => {
        if self.proven_numeric(ip, a) {
          self.emit_imm_eq_proven(ip, dst, a, imm_const, false);
        } else {
          self.emit_imm_eq(dst, a, imm_const, false);
        }
        false
      },
      Instr::CheckParamType { reg, check_idx } => {
        self.emit_check_param_type(ip, reg, check_idx);
        false
      },
    }
  }

  /// Whether the instruction at `ip` can allocate, and so can leave a
  /// collection owed. Only consulted through `loop_has_allocations`,
  /// to decide how much of the safepoint a loop back-edge needs.
  fn is_allocating_instr(&self, ip: usize) -> bool {
    let instr = &self.proto.chunk.code[ip];
    match instr {
      Instr::MakeList { .. }
      | Instr::MakeDict { .. }
      | Instr::MakeClass { .. }
      | Instr::Closure { .. }
      | Instr::Concat { .. }
      | Instr::Import { .. }
      | Instr::MakeRange { .. }
      | Instr::GetSlice { .. }
      | Instr::MakePromoted { .. }
      | Instr::InvokeSuper { .. }
      | Instr::CallSuperCtor { .. } => true,
      Instr::Call { func, num_args, .. } => {
        if let Some(CallTarget::Known { proto_ptr, .. }) = self.call_targets.get(&ip).copied() {
          let callee = unsafe { &*(proto_ptr as *const ObjFunction) };
          if self.inline_plan(ip, callee, *func, *num_args).is_some() {
            return false;
          }
        }
        true
      },
      Instr::Invoke { .. } => true,
      _ => false,
    }
  }

  fn loop_has_allocations(&self, target_ip: usize, current_ip: usize) -> bool {
    let start = target_ip.min(current_ip);
    let end = target_ip.max(current_ip);
    (start..=end).any(|ip| self.is_allocating_instr(ip))
  }

  /// Whether this function's body is completely allocation-free and GC-free:
  /// it allocates no heap objects, accesses no globals, performs no I/O,
  /// raises no errors, and only ever makes self-calls back to this function.
  /// When this holds, native self-invocations can safely skip flushing and
  /// reloading live registers to/from `VM::registers`, because GC will never
  /// inspect the caller's frame and Cranelift preserves SSA variables across
  /// native calls.
  fn is_self_allocation_free(&self) -> bool {
    if !self.proto.upvalues.is_empty() {
      return false;
    }
    for (ip, instr) in self.proto.chunk.code.iter().enumerate() {
      match instr {
        Instr::MakeList { .. }
        | Instr::MakeDict { .. }
        | Instr::MakeClass { .. }
        | Instr::Closure { .. }
        | Instr::Concat { .. }
        | Instr::Import { .. }
        | Instr::ImportAll { .. }
        | Instr::MakeRange { .. }
        | Instr::GetSlice { .. }
        | Instr::MakePromoted { .. }
        | Instr::InvokeSuper { .. }
        | Instr::CallSuperCtor { .. }
        | Instr::SetGlobal { .. }
        | Instr::AssignGlobal { .. }
        | Instr::GetGlobal { .. }
        | Instr::Print { .. }
        | Instr::Raise { .. } => return false,
        Instr::LoadConst { const_idx, .. } => {
          if let Some(c) = self.proto.chunk.constants.get(*const_idx as usize) {
            if !c.is_number() && !c.is_nil() && !c.is_bool() {
              return false;
            }
          }
        },
        Instr::Call { .. } => {
          match self.call_targets.get(&ip) {
            Some(CallTarget::SelfRecursive) => continue,
            Some(CallTarget::Known { proto_ptr, .. }) => {
              if *proto_ptr as *const ObjFunction == self.proto as *const ObjFunction {
                continue;
              }
            },
            _ => {},
          }
          return false;
        },
        Instr::Invoke { method_const, .. } => {
          if self.self_invoke_target(*method_const).is_none() {
            return false;
          }
        },
        _ => {},
      }
    }
    true
  }

  /// A safepoint that asks about signals only, for a loop whose body
  /// provably cannot allocate.
  ///
  /// `loop_has_allocations` rules out a collection becoming owed inside
  /// such a loop, and that reasoning still holds. What it never covered
  /// is the other thing a safepoint delivers: a `while true { }` with
  /// nothing in it allocates nothing, so it got no safepoint at all,
  /// and an `on_signal('INT', ...)` handler could never run once the
  /// loop tiered up. Worse than the handler not firing, installing one
  /// replaces the default terminate behaviour, so Ctrl+C stopped
  /// working at all.
  ///
  /// Emitted ONLY when `signal::armed()` says this process has actually
  /// installed a handler. That is what keeps the fix off the hot path:
  /// a program that never calls `on_signal()` gets byte-identical code
  /// on these edges to what it got before, and the tightest arithmetic
  /// loop pays nothing for a feature it does not use.
  fn emit_signal_safepoint(&mut self) {
    self.emit_safepoint_inner(false);
  }

  /// The full safepoint: see `runtime::zuri_jit_safepoint`'s own docs.
  /// Emitted at every function/method call site and at every loop
  /// back-edge whose body can allocate.
  ///
  /// `Heap::needs_major_gc()`/`needs_minor_gc()` collapse to a single
  /// precomputed byte (`Heap::jit_gc_needed`), and the signal flags to
  /// a single hint byte, so the overwhelmingly common case (nothing
  /// owed of either kind) costs two loads, an `or` and a branch rather
  /// than an unconditional FFI call at every iteration and call site.
  /// The helper is only actually invoked on the rare branch where
  /// something really is owed, and it re-checks both itself, so a
  /// stale read here can only ever cost one wasted call.
  fn emit_safepoint(&mut self) {
    self.emit_safepoint_inner(true);
  }

  /// The shared body. `check_gc` selects whether the inline test covers
  /// the GC flag as well as the signal hint; the helper it guards
  /// re-checks both regardless, so omitting the GC half only ever costs
  /// a missed collection opportunity in a loop that cannot allocate one
  /// anyway.
  ///
  /// Everything to do with signals is conditional on `signal::armed()`,
  /// read here at compile time. A process that never calls
  /// `on_signal()` gets the exact instruction sequence this emitted
  /// before signal delivery existed: one byte load, one compare, one
  /// branch, and an unchecked helper call on the cold side. No extra
  /// load, no extra `or`, and no status check; a collection cannot
  /// raise, so there is nothing to check for when no handler can run.
  fn emit_safepoint_inner(&mut self, check_gc: bool) {
    let armed = crate::modules::os_util::signal::armed();
    let flags = cranelift_codegen::ir::MemFlagsData::trusted();

    let gc_needed = if check_gc {
      Some(
        self
          .fb
          .ins()
          .load(types::I8, flags, self.vm_param, HEAP_JIT_GC_NEEDED_OFFSET),
      )
    } else {
      None
    };

    // The four real signal flags live in `os_util::signal`, too far
    // apart to inline a scan of, so that module keeps a single-byte
    // hint at a fixed address purely for this load. Its address is
    // baked in as an immediate; a `static`'s address is fixed for the
    // process, and a one-byte load needs no alignment guarantee.
    let signal_pending = if armed {
      let hint_ptr = self.i64c(crate::modules::os_util::signal::pending_hint_addr() as i64);
      Some(self.fb.ins().load(types::I8, flags, hint_ptr, 0))
    } else {
      None
    };

    let tested = match (gc_needed, signal_pending) {
      (Some(gc), Some(sig)) => self.fb.ins().bor(gc, sig),
      (Some(gc), None) => gc,
      (None, Some(sig)) => sig,
      // `emit_signal_safepoint` is only ever reached when armed, and
      // `emit_safepoint` always checks GC, so there is nothing to test.
      (None, None) => return,
    };

    let zero = self.fb.ins().iconst(types::I8, 0);
    let owed = self.fb.ins().icmp(IntCC::NotEqual, tested, zero);

    let slow_block = self.fb.create_block();
    let done_block = self.fb.create_block();
    self.fb.ins().brif(owed, slow_block, &[], done_block, &[]);

    self.fb.switch_to_block(slow_block);
    self.publish_ip();
    self.flush_live(self.current_ip);
    let status = self.call_helper_raw("zuri_jit_safepoint", &[self.vm_param]);
    self.refresh_regs();
    self.reload_live(self.current_ip);

    if armed {
      // Only a signal callback can raise here; a collection cannot. So
      // the status check exists exactly when a callback could run, and
      // an unarmed process pays nothing for it. The unwind is the one
      // `call_checked` uses: the error is already in
      // `VM::jit_pending_error` for `VM::invoke_compiled` to pick up.
      let zero64 = self.i64c(0);
      let is_err = self.fb.ins().icmp(IntCC::NotEqual, status, zero64);
      let err_block = self.fb.create_block();
      let ok_block = self.fb.create_block();
      self.fb.ins().brif(is_err, err_block, &[], ok_block, &[]);

      self.fb.switch_to_block(err_block);
      let junk = self.i64c(0);
      self.fb.ins().return_(&[junk]);

      self.fb.switch_to_block(ok_block);
    }

    self.fb.ins().jump(done_block, &[]);
    self.fb.switch_to_block(done_block);
  }

  /// `emit_binary_numeric_guarded`'s fast-path body with the guard,
  /// branch, and slow-path fallback removed entirely; valid only
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
    let fa = self.load_reg_f64(a);
    let fb_ = self.load_reg_f64(b);
    let fr = fast(self, fa, fb_);
    self.store_reg_f64(dst, fr);
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
  /// pre-instruction value dominated the slow edge; silently
  /// discarding the slow path's real result if that's the path actually
  /// taken at runtime. Simply marking `dst` `Stale` (what `call_helper`'s
  /// generic `any_dst` handling already does for every helper-written
  /// destination) is NOT sufficient here specifically: `Stale` means
  /// "trust memory on the next read," but the FAST path's own result is
  /// deliberately never written to memory at all (that's the entire
  /// point of `store_reg`'s laziness); a stale-triggered reload would
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
    ip: usize,
    dst: u8,
    a: u8,
    b: u8,
    slow_helper: &'static str,
    fast: impl FnOnce(&mut Self, IrValue, IrValue) -> IrValue,
  ) {
    let va = self.load_reg(a);
    let vb = self.load_reg(b);
    let guard = self.combined_numeric_guard(ip, va, vb, a, b);
    let fast_block = self.fb.create_block();
    let slow_block = self.fb.create_block();
    let done_block = self.fb.create_block();
    self.fb.ins().brif(guard, fast_block, &[], slow_block, &[]);

    self.fb.switch_to_block(fast_block);
    let fa = self.f64_view(a, va);
    let fb_ = self.f64_view(b, vb);
    let fr = fast(self, fa, fb_);
    self.store_reg_f64(dst, fr);
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
  }

  /// `emit_bitwise_guarded`'s fast path, unguarded: see
  /// `emit_binary_numeric_proven`'s docs.
  fn emit_bitwise_proven(
    &mut self,
    _ip: usize,
    dst: u8,
    a: u8,
    b: u8,
    fast: impl FnOnce(&mut FunctionBuilder, IrValue, IrValue) -> IrValue,
  ) {
    // A proven int is read straight out of the integer cache: the cache
    // is filled by wrapping integer arithmetic, which is exactly the
    // narrowing these operators want. Anything else is narrowed from the
    // float. The result always fits `i64`, so it can seed the cache.
    let ia = if self.proven_int(_ip, a) {
      self.fb.use_var(self.reg_vars_int[a as usize])
    } else {
      let va = self.load_reg(a);
      let fa = self.f64_view(a, va);
      self.emit_narrow_to_i64(fa)
    };
    let ib = if self.proven_int(_ip, b) {
      self.fb.use_var(self.reg_vars_int[b as usize])
    } else {
      let vb = self.load_reg(b);
      let fb_ = self.f64_view(b, vb);
      self.emit_narrow_to_i64(fb_)
    };
    let ir = fast(self.fb, ia, ib);
    self.store_reg_int(dst, ir);
  }

  fn emit_bitwise_guarded(
    &mut self,
    ip: usize,
    dst: u8,
    a: u8,
    b: u8,
    slow_helper: &'static str,
    fast: impl FnOnce(&mut FunctionBuilder, IrValue, IrValue) -> IrValue,
  ) {
    let va = self.load_reg(a);
    let vb = self.load_reg(b);
    let guard = self.combined_numeric_guard(ip, va, vb, a, b);
    let fast_block = self.fb.create_block();
    let slow_block = self.fb.create_block();
    let done_block = self.fb.create_block();
    self.fb.ins().brif(guard, fast_block, &[], slow_block, &[]);

    self.fb.switch_to_block(fast_block);
    let fa = self.f64_view(a, va);
    let fb_ = self.f64_view(b, vb);
    let ia = self.fb.ins().fcvt_to_sint_sat(types::I64, fa);
    let ib = self.fb.ins().fcvt_to_sint_sat(types::I64, fb_);
    let ir = fast(self.fb, ia, ib);
    self.store_reg_int(dst, ir);
    self.fb.ins().jump(done_block, &[]);

    self.fb.switch_to_block(slow_block);
    let base = self.base_param;
    let dst_i = self.idx(dst);
    let a_i = self.idx(a);
    let b_i = self.idx(b);
    self.call_checked(slow_helper, &[self.vm_param, base, dst_i, a_i, b_i]);
    self.resync_dst_from_memory(dst);
    if self.int_tracked[dst as usize] {
      let v = self.load_reg(dst);
      let f = self.to_f64(v);
      let iv = self.fb.ins().fcvt_to_sint_sat(types::I64, f);
      self.fb.def_var(self.reg_vars_int[dst as usize], iv);
    }
    self.fb.ins().jump(done_block, &[]);

    self.fb.switch_to_block(done_block);
  }

  /// A binary `f64` operation that has no Cranelift instruction,
  /// reached as a DIRECT call to its `jit::runtime` helper; the
  /// `Instr::Mod`/`Instr::Pow` counterpart of `NumberIntrinsic::Call`.
  ///
  /// Goes through `call_helper_raw`, not `call_helper`: these helpers
  /// touch no VM register and cannot allocate, collect, or raise, so
  /// the register-cache flush and stale-mark `call_helper` would bracket
  /// them with is exactly the cost worth removing here. The helpers
  /// speak raw bits, which for a number are its `f64` bits, so the
  /// conversions either side are bitcasts.
  /// The constant behind `v` when it is an `f64const` this compile
  /// emitted, so a narrowing of it can be folded at compile time.
  fn bitcast_source_f64_const(&self, v: IrValue) -> Option<f64> {
    match self.fb.func.dfg.value_def(v) {
      cranelift_codegen::ir::ValueDef::Result(inst, _) => match self.fb.func.dfg.insts[inst] {
        cranelift_codegen::ir::InstructionData::UnaryIeee64 { imm, .. } => {
          Some(f64::from_bits(imm.bits()))
        },
        _ => None,
      },
      _ => None,
    }
  }

  /// Narrows a number to `i64` the way the bitwise operators need it:
  /// modulo `2^64`, matching `value::num_to_wrapped_i64`.
  ///
  /// `fcvt_to_sint_sat` agrees with that for every operand inside
  /// `i64` and saturates outside it, so the inline sequence handles the
  /// whole representable range and only a value past `2^63` reaches the
  /// helper. That branch is never taken by ordinary code, so the cost
  /// here is the magnitude test.
  fn emit_narrow_to_i64(&mut self, f: IrValue) -> IrValue {
    if let Some(c) = self.bitcast_source_f64_const(f) {
      return self.i64c(crate::vm::value::num_to_wrapped_i64(c));
    }
    let sat = self.fb.ins().fcvt_to_sint_sat(types::I64, f);
    let mag = self.fb.ins().fabs(f);
    let limit = self.fb.ins().f64const(9223372036854775808.0);
    let in_range = self.fb.ins().fcmp(
      cranelift_codegen::ir::condcodes::FloatCC::LessThan,
      mag,
      limit,
    );

    let fast_block = self.fb.create_block();
    let wrap_block = self.fb.create_block();
    let join_block = self.fb.create_block();
    self.fb.append_block_param(join_block, types::I64);
    self
      .fb
      .ins()
      .brif(in_range, fast_block, &[], wrap_block, &[]);

    self.fb.switch_to_block(fast_block);
    self.fb.ins().jump(join_block, &[sat.into()]);

    // Reduction modulo 2^64, entirely in floats and entirely exact.
    // Dividing and multiplying by a power of two only moves the
    // exponent, `trunc` is a single rounding, and the final subtraction
    // has operands within a factor of two of each other, so no step
    // rounds. The result lands inside `i64`, where the conversion is
    // exact too.
    //
    // An infinity falls out as `inf - inf`, which is NaN, and a NaN
    // converts to zero; that is what a non-finite operand narrows to.
    self.fb.switch_to_block(wrap_block);
    let span = self.fb.ins().f64const(18446744073709551616.0);
    let inv_span = self.fb.ins().f64const(5.421010862427522e-20);
    let neg_limit = self.fb.ins().f64const(-9223372036854775808.0);
    let truncated = self.fb.ins().trunc(f);
    let scaled = self.fb.ins().fmul(truncated, inv_span);
    let quotient = self.fb.ins().trunc(scaled);
    let carried = self.fb.ins().fmul(quotient, span);
    let rem = self.fb.ins().fsub(truncated, carried);
    let above = self.fb.ins().fcmp(
      cranelift_codegen::ir::condcodes::FloatCC::GreaterThanOrEqual,
      rem,
      limit,
    );
    let lowered = self.fb.ins().fsub(rem, span);
    let folded = self.fb.ins().select(above, lowered, rem);
    let below = self.fb.ins().fcmp(
      cranelift_codegen::ir::condcodes::FloatCC::LessThan,
      folded,
      neg_limit,
    );
    let raised = self.fb.ins().fadd(folded, span);
    let signed = self.fb.ins().select(below, raised, folded);
    let wrapped = self.fb.ins().fcvt_to_sint_sat(types::I64, signed);
    self.fb.ins().jump(join_block, &[wrapped.into()]);

    self.fb.switch_to_block(join_block);
    self.fb.block_params(join_block)[0]
  }

  fn call_f64_intrinsic(&mut self, helper: &'static str, fa: IrValue, fb_: IrValue) -> IrValue {
    let a_bits = self.from_f64(fa);
    let b_bits = self.from_f64(fb_);
    let vm = self.vm_param;
    let r = self.call_helper_raw(helper, &[vm, a_bits, b_bits]);
    self.to_f64(r)
  }

  /// `<<`'s exact semantics on two already-`f64 as i64` operands:
  /// `x.checked_shl(y as u32).unwrap_or(0)`.
  ///
  /// Both quirks of that expression are reproduced deliberately rather
  /// than normalized away, because the interpreter has them and the two
  /// tiers must agree:
  /// - `y as u32` is Rust's TRUNCATING integer cast, not a saturating
  ///   one, so a negative shift count wraps to a huge `u32` (and then
  ///   falls into the out-of-range case below) rather than clamping
  ///   to zero.
  /// - `checked_shl` yields `None` for any count >= 64, and the
  ///   `unwrap_or(0)` turns that into a plain zero. An unguarded
  ///   `ishl` would instead mask the count to 6 bits and shift by
  ///   `count % 64`, which is a different answer.
  fn shift_left(fb: &mut FunctionBuilder, x: IrValue, y: IrValue) -> IrValue {
    let count = fb.ins().ireduce(types::I32, y);
    let width = fb.ins().iconst(types::I32, 64);
    let out_of_range = fb
      .ins()
      .icmp(IntCC::UnsignedGreaterThanOrEqual, count, width);
    let shifted = fb.ins().ishl(x, count);
    let zero = fb.ins().iconst(types::I64, 0);
    fb.ins().select(out_of_range, zero, shifted)
  }

  /// `>>`'s exact semantics: `x.checked_shr(y as u32).unwrap_or(0)`.
  /// An ARITHMETIC shift (Rust's `>>` on `i64` sign-extends), but still
  /// zero; not `-1`; for a negative `x` shifted by 64 or more, since
  /// that is the `unwrap_or(0)` case rather than a saturating shift.
  /// See `shift_left` for the shared count-cast reasoning.
  fn shift_right(fb: &mut FunctionBuilder, x: IrValue, y: IrValue) -> IrValue {
    let count = fb.ins().ireduce(types::I32, y);
    let width = fb.ins().iconst(types::I32, 64);
    let out_of_range = fb
      .ins()
      .icmp(IntCC::UnsignedGreaterThanOrEqual, count, width);
    let shifted = fb.ins().sshr(x, count);
    let zero = fb.ins().iconst(types::I64, 0);
    fb.ins().select(out_of_range, zero, shifted)
  }

  /// `>>>`'s exact semantics:
  /// `(x as u32).checked_shr(y as u32).unwrap_or(0) as i64`.
  ///
  /// Note this one is 32-bit throughout, unlike the other two: the
  /// operand is truncated to `u32` first, the out-of-range threshold is
  /// therefore 32 rather than 64, the shift is LOGICAL, and the final
  /// `u32 as i64` zero-extends.
  fn shift_right_unsigned(fb: &mut FunctionBuilder, x: IrValue, y: IrValue) -> IrValue {
    let count = fb.ins().ireduce(types::I32, y);
    let x32 = fb.ins().ireduce(types::I32, x);
    let width = fb.ins().iconst(types::I32, 32);
    let out_of_range = fb
      .ins()
      .icmp(IntCC::UnsignedGreaterThanOrEqual, count, width);
    let shifted = fb.ins().ushr(x32, count);
    let zero = fb.ins().iconst(types::I32, 0);
    let result = fb.ins().select(out_of_range, zero, shifted);
    fb.ins().uextend(types::I64, result)
  }

  fn emit_always_helper(&mut self, helper: &'static str, dst: u8, a: u8, b: u8) {
    let base = self.base_param;
    let dst_i = self.idx(dst);
    let a_i = self.idx(a);
    let b_i = self.idx(b);
    self.call_checked(helper, &[self.vm_param, base, dst_i, a_i, b_i]);
    self.resync_dst_from_memory(dst);
  }

  /// `Instr::Eq`/`Instr::Neq`; fully inlined except when BOTH
  /// operands are heap objects (needing `Obj`-aware content comparison
  ///; list/dict structural equality, or pointer identity for
  /// everything else; which always needs a real dereference). Eq/Neq
  /// never consult an operator override (matches the interpreter's own
  /// handler exactly: see `vm.rs`).
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
  /// `emit_compare_guarded`'s `num_block` path directly; valid only
  /// when `a`/`b` are PROVEN numeric, so the object/raw-bits cases can
  /// never apply.
  fn emit_compare_proven_numeric(&mut self, ip: usize, dst: u8, a: u8, b: u8, cc: IntCC) {
    let cmp = if self.both_proven_int(ip, a, b) {
      let ia = self.fb.use_var(self.reg_vars_int[a as usize]);
      let ib = self.fb.use_var(self.reg_vars_int[b as usize]);
      self.fb.ins().icmp(cc, ia, ib)
    } else {
      let fa = self.load_reg_f64(a);
      let fb_ = self.load_reg_f64(b);
      self.fb.ins().fcmp(to_float_cc(cc), fa, fb_)
    };
    let bits = self.bool_value(cmp);
    self.store_reg(dst, bits);
  }

  fn emit_compare_guarded(
    &mut self,
    ip: usize,
    dst: u8,
    a: u8,
    b: u8,
    slow_helper: &'static str,
    cc: IntCC,
  ) {
    let va = self.load_reg(a);
    let vb = self.load_reg(b);
    let both_num = self.combined_numeric_guard(ip, va, vb, a, b);
    let both_obj = self.both_obj(va, vb);

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
    let fa = self.f64_view(a, va);
    let fb_ = self.f64_view(b, vb);
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
  }

  /// `emit_fcompare_guarded`'s fast path, unguarded: see
  /// `emit_binary_numeric_proven`'s docs.
  fn emit_fcompare_proven(
    &mut self,
    dst: u8,
    a: u8,
    b: u8,
    cc: cranelift_codegen::ir::condcodes::FloatCC,
  ) {
    let fa = self.load_reg_f64(a);
    let fb_ = self.load_reg_f64(b);
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
    let fast_block = self.fb.create_block();
    let slow_block = self.fb.create_block();
    let done_block = self.fb.create_block();
    self.fb.ins().brif(guard, fast_block, &[], slow_block, &[]);

    self.fb.switch_to_block(fast_block);
    let fa = self.f64_view(a, va);
    let fb_ = self.f64_view(b, vb);
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
  }

  /// `emit_addimm`'s fast path, unguarded: see
  /// `emit_binary_numeric_proven`'s docs.
  fn emit_addimm_proven(&mut self, dst: u8, a: u8, imm_const: u16) {
    let fa = self.load_reg_f64(a);
    let fimm = self.bake_f64(a, imm_const);
    let fr = self.fb.ins().fadd(fa, fimm);
    self.store_reg_f64(dst, fr);
  }

  fn emit_addimm(&mut self, dst: u8, a: u8, imm_const: u16) {
    let va = self.load_reg(a);
    let guard = self.is_number(va);
    let fast_block = self.fb.create_block();
    let slow_block = self.fb.create_block();
    let done_block = self.fb.create_block();
    self.fb.ins().brif(guard, fast_block, &[], slow_block, &[]);

    self.fb.switch_to_block(fast_block);
    let fa = self.f64_view(a, va);
    let fimm = self.bake_f64(a, imm_const);
    let fr = self.fb.ins().fadd(fa, fimm);
    self.store_reg_f64(dst, fr);
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
  }

  /// `emit_imm_numeric_guarded`'s fast path, unguarded: see
  /// `emit_binary_numeric_proven`'s docs.
  fn emit_imm_numeric_proven(
    &mut self,
    dst: u8,
    a: u8,
    imm_const: u16,
    fast: impl FnOnce(&mut Self, IrValue, IrValue) -> IrValue,
  ) {
    let fa = self.load_reg_f64(a);
    let fimm = self.bake_f64(a, imm_const);
    let fr = fast(self, fa, fimm);
    self.store_reg_f64(dst, fr);
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
    let fast_block = self.fb.create_block();
    let slow_block = self.fb.create_block();
    let done_block = self.fb.create_block();
    self.fb.ins().brif(guard, fast_block, &[], slow_block, &[]);

    self.fb.switch_to_block(fast_block);
    let fa = self.f64_view(a, va);
    let fimm = self.bake_f64(a, imm_const);
    let fr = fast(self, fa, fimm);
    self.store_reg_f64(dst, fr);
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
  }

  /// `emit_imm_compare_guarded`'s fast path, unguarded: see
  /// `emit_binary_numeric_proven`'s docs.
  fn emit_imm_compare_proven(
    &mut self,
    dst: u8,
    a: u8,
    imm_const: u16,
    cc: cranelift_codegen::ir::condcodes::FloatCC,
  ) {
    let fa = self.load_reg_f64(a);
    let fimm = self.bake_f64(a, imm_const);
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
    let fast_block = self.fb.create_block();
    let slow_block = self.fb.create_block();
    let done_block = self.fb.create_block();
    self.fb.ins().brif(guard, fast_block, &[], slow_block, &[]);

    self.fb.switch_to_block(fast_block);
    let fa = self.f64_view(a, va);
    let fimm = self.bake_f64(a, imm_const);
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
  }

  /// `EqImm`/`NeqImm`; like the interpreter's own handler, this is
  /// pure `Value::equals` against a KNOWN-numeric constant, no operator
  /// override lookup ever. Fully inlinable with no helper fallback at
  /// all: when `a` is itself a number, real IEEE-754 equality decides
  /// it; otherwise `Value::equals` falls through to a raw bit compare
  /// (see `value.rs`), which is exactly what comparing the two `u64`s
  /// directly already gives here.
  /// `emit_imm_eq`'s `num_block` path directly; valid only when `a`
  /// is PROVEN numeric.
  fn emit_imm_eq_proven(&mut self, ip: usize, dst: u8, a: u8, imm_const: u16, want_eq: bool) {
    let imm_val = self.proto.chunk.constants[imm_const as usize].as_number();
    let bits_num = if self.proven_int(ip, a)
      && imm_val.fract() == 0.0
      && imm_val >= (i64::MIN as f64)
      && imm_val <= (i64::MAX as f64)
    {
      let ia = self.fb.use_var(self.reg_vars_int[a as usize]);
      let imm_i = self.i64c(imm_val as i64);
      let int_cc = if want_eq {
        IntCC::Equal
      } else {
        IntCC::NotEqual
      };
      let cmp = self.fb.ins().icmp(int_cc, ia, imm_i);
      self.bool_value(cmp)
    } else {
      let fa = self.load_reg_f64(a);
      let fimm = self.bake_f64(a, imm_const);
      let float_cc = if want_eq {
        cranelift_codegen::ir::condcodes::FloatCC::Equal
      } else {
        cranelift_codegen::ir::condcodes::FloatCC::NotEqual
      };
      let cmp_num = self.fb.ins().fcmp(float_cc, fa, fimm);
      self.bool_value(cmp_num)
    };
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
    let fa = self.f64_view(a, va);
    let fimm = self.bake_f64(a, imm_const);
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
