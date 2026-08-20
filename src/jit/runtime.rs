//! The fixed set of `extern "C"` functions compiled code calls into.
//!
//! # Why these exist at all
//!
//! `jit::codegen` inlines a handful of extremely hot, extremely simple
//! operations directly as machine code (register loads/stores, a
//! number/number arithmetic fast path, comparisons, branches). Every
//! other bytecode instruction -- field access, calls, string/list/dict
//! operations, class declarations, module imports, anything that can
//! touch the heap in a non-trivial way or fall back to an operator
//! override -- compiles to a call into one of the functions below,
//! which just does exactly what the interpreter's own `vm.rs` match arm
//! for that instruction does, by calling the same underlying `VM`
//! methods (`binary_add`, `dispatch_call`, `index_get`, ...). There's
//! exactly one place that implements what any given opcode means;
//! this module is glue, not a second implementation.
//!
//! # The error-propagation protocol
//!
//! None of these ever return a `Result` (they're a raw C ABI, and
//! `Result<Value, Value>` isn't a `repr(C)` type worth wrestling into
//! one for every call site). Instead, every helper that can fail
//! returns a `u64` status: `0` for success, `1` for failure. On
//! failure, the helper has already stored the propagating exception
//! `Value` into `VM::jit_pending_exception` before returning. Compiled
//! code checks this status immediately after the call (see
//! `codegen::FuncCompiler::emit_call`); on failure it abandons the rest
//! of the function entirely and returns up to its own Rust caller
//! (`VM::invoke_compiled`), which reads `jit_pending_exception` and
//! turns it into a real `Err(Value)` -- exactly mirroring how an
//! interpreted `Err(Value)` already propagates via `?`/`tri!`. This is
//! the concrete mechanism behind "exceptions cause a bailout" for
//! compiled code: there's no unwinder in the generated machine code at
//! all, just this one flag checked after every fallible call.
//!
//! # The register-pointer-staleness protocol
//!
//! `VM::registers` is a `Vec<Value>` that can reallocate (`resize`)
//! whenever a deeper call frame needs more room than is currently
//! allocated. Any helper that can, even transitively, push a new call
//! frame (an ordinary call, an operator-override dispatch, a
//! constructor, a whole module's top-level code running on `import`)
//! can invalidate a previously-fetched register base pointer. Compiled
//! code therefore never caches that pointer across such a call -- it
//! re-reads `VM::regs_ptr_cache` (a plain field VM itself keeps in sync
//! at every point `VM::registers` can reallocate) via a direct memory
//! load at a compile-time-baked offset immediately afterward (see
//! `codegen::FuncCompiler::refresh_regs`), no FFI call needed at all.
//! Helpers that provably never push a frame are exempt purely as a
//! performance optimization, not a correctness shortcut -- when in
//! doubt, a helper is conservatively treated as frame-pushing by
//! `codegen`.
//!
//! # Baked constants
//!
//! `Chunk::constants` never changes after a function finishes compiling
//! to bytecode, and heap objects never move (see `object::Heap`'s own
//! docs) -- so wherever a bytecode instruction references
//! `chunk.constants[i]` (a global/field/method name, a class name, a
//! function prototype, an immediate number), `jit::codegen` reads that
//! constant's `Value` directly out of the already-compiled `ObjFunction`
//! at JIT-compile time and bakes its raw bit pattern into the generated
//! code as an immediate -- there's no `chunk.constants[i]` load at
//! runtime anywhere in compiled code. Helpers below that take a
//! `*_bits: u64` parameter are receiving one of these baked constants,
//! not a live register read. The same trick applies to a stable pointer
//! to the currently-compiling function's own `ObjFunction` itself
//! (`func_ptr: u64`), needed by a few helpers for `Chunk::global_cache`/
//! `Chunk::jump_tables` access.

use std::cell::Cell;

use crate::vm::chunk::{InvokeCacheCell, JumpKey};
use crate::vm::object::{
  ListStorage, NativeFunction, ObjClosure, ObjFunction, UpvalueDescriptor, UpvalueState, write_barrier,
};
use crate::vm::value::Value;
use crate::vm::vm::{CallArgs, VM};

const OK: u64 = 0;
const ERR: u64 = 1;

#[inline(always)]
unsafe fn vm<'a>(ptr: *mut VM) -> &'a mut VM {
  unsafe { &mut *ptr }
}

#[inline(always)]
fn fail(vm: &mut VM, exc: Value) -> u64 {
  vm.jit_pending_exception.set(exc);
  ERR
}

// ---------------------------------------------------------------------
// Register-array / GC-safepoint primitives
// ---------------------------------------------------------------------

/// Real deoptimization -- records `ip` as where compiled code is
/// giving up, so `VM::invoke_compiled` picks it up right after this
/// call's caller returns. Codegen always follows a call to this with
/// an immediate `return_` out of the whole compiled function (never
/// falls through to more translated instructions), so the actual
/// return value here is never observed -- unlike every other helper,
/// its result is dead by construction.
///
/// Sound for the same reason the GC safepoint below is: every VM
/// register a compiled function operates on lives in `VM::registers`
/// at every instruction boundary, never only in a native machine
/// register that would need to be found and translated back. So
/// "deoptimizing" needs no state reconstruction at all -- the
/// interpreter reads the exact same array it always does, starting
/// fresh at `ip`. See `VM::pending_deopt_ip`'s own docs for the full
/// reasoning and `VM::invoke_compiled` for where this is consumed.
pub unsafe extern "C" fn zuri_jit_deopt(vm_ptr: *mut VM, ip: u64) -> u64 {
  let vm = unsafe { vm(vm_ptr) };
  vm.pending_deopt_ip.set(ip as i64);
  OK
}

/// GC safepoint -- called at every loop back-edge and call site in
/// compiled code (see `codegen::FuncCompiler::emit_safepoint`), mirrors
/// the interpreter's own per-instruction `if needs_major_gc() { ... }
/// else if needs_minor_gc() { ... }` check. Sound for exactly the same
/// reason the interpreter's root scan is: this frame's `CallFrame`
/// (base + function pointer) is already on `VM::frames` for the whole
/// duration compiled code runs (pushed by `VM::invoke_compiled`'s
/// caller before entry, popped after), and every register's actual
/// content lives in `VM::registers` at all times (never a separate
/// cached copy) -- so the collector's normal root scan already sees
/// this frame correctly with no JIT-specific support needed, for
/// either a major or a minor collection.
pub unsafe extern "C" fn zuri_jit_gc_safepoint(vm_ptr: *mut VM) -> u64 {
  let vm = unsafe { vm(vm_ptr) };
  if vm.heap.needs_major_gc() {
    vm.collect_garbage();
  } else if vm.heap.needs_minor_gc() {
    vm.collect_minor();
  }
  OK
}

// ---------------------------------------------------------------------
// Falsey / control flow
// ---------------------------------------------------------------------

/// `Value::is_falsey` needs to dereference a heap object for the
/// string/bytes/bigint cases, which is unsafe to replicate as hand-
/// written IR (see `value.rs`'s stable-bit-pattern-only inlining
/// policy in `jit::codegen`) -- so the full check always goes through
/// here rather than being partially inlined.
pub unsafe extern "C" fn zuri_jit_is_falsey(vm_ptr: *mut VM, base: u64, src: u64) -> u64 {
  let vm = unsafe { vm(vm_ptr) };
  let v = vm.get_reg(base as usize, src as u8);
  v.is_falsey() as u64
}

pub unsafe extern "C" fn zuri_jit_print(vm_ptr: *mut VM, base: u64, src: u64) -> u64 {
  let vm = unsafe { vm(vm_ptr) };
  let v = vm.get_reg(base as usize, src as u8);
  println!("{}", v);
  OK
}

// ---------------------------------------------------------------------
// Arithmetic / comparison slow paths -- mirrors vm.rs's Instr::{Add,
// Sub, Mul, Div, Pow, Floor, Mod, BitAnd, BitOr, BitXor, BitShl,
// BitShr, BitUshr, BitNot, Neg, Not, Concat, Eq, Neq, Lt, Le, Gt, Ge,
// *Imm} handlers exactly, by calling the identical underlying `VM`
// methods those handlers call. Only reached once the inline fast path
// (both operands plain numbers) in `codegen` has already failed its
// guard -- e.g. a string/list/bigint operand, or a class instance
// implementing an `@`-operator override.
// ---------------------------------------------------------------------

macro_rules! binary_slow {
  ($name:ident, $method:ident, $op_name:literal, $deco:literal, $op:expr, $big_op:expr) => {
    pub unsafe extern "C" fn $name(vm_ptr: *mut VM, base: u64, dst: u64, a: u64, b: u64) -> u64 {
      let vm = unsafe { vm(vm_ptr) };
      let r = vm.$method(
        base as usize,
        dst as u8,
        a as u8,
        b as u8,
        $op_name,
        $deco,
        $op,
        $big_op,
      );
      match r {
        Ok(()) => OK,
        Err(e) => fail(vm, e),
      }
    }
  };
}

binary_slow!(
  zuri_jit_sub_slow,
  binary_numeric,
  "-",
  "@sub",
  |x: f64, y: f64| x - y,
  |x, y| &x - &y
);
binary_slow!(
  zuri_jit_div_slow,
  binary_numeric,
  "/",
  "@div",
  |x: f64, y: f64| x / y,
  |x, y| &x / &y
);
binary_slow!(
  zuri_jit_pow,
  binary_numeric,
  "**",
  "@pow",
  |x: f64, y: f64| x.powf(y),
  |x, y| &x * &y
);
binary_slow!(
  zuri_jit_mod,
  binary_numeric,
  "%",
  "@mod",
  |x: f64, y: f64| crate::vm::value::num_rem(x, y),
  |x, y| &x % &y
);
binary_slow!(
  zuri_jit_floordiv,
  binary_numeric,
  "//",
  "@floordiv",
  |x: f64, y: f64| (x / y).floor(),
  |x, y| &x / &y
);

binary_slow!(
  zuri_jit_bitand_slow,
  bitwise_numeric,
  "&",
  "@and",
  |x: i64, y: i64| x & y,
  |x, y| &x & &y
);
binary_slow!(
  zuri_jit_bitor_slow,
  bitwise_numeric,
  "|",
  "@or",
  |x: i64, y: i64| x | y,
  |x, y| &x | &y
);
binary_slow!(
  zuri_jit_bitxor_slow,
  bitwise_numeric,
  "^",
  "@xor",
  |x: i64, y: i64| x ^ y,
  |x, y| &x ^ &y
);
binary_slow!(
  zuri_jit_bitshl,
  bitwise_numeric,
  "<<",
  "@lshift",
  |x: i64, y: i64| x.checked_shl(y as u32).unwrap_or(0),
  |x: num_bigint::BigInt, y: num_bigint::BigInt| {
    use num_traits::ToPrimitive;
    use std::ops::Shl;
    x.shl(y.to_i64().unwrap_or(0))
  }
);
binary_slow!(
  zuri_jit_bitshr,
  bitwise_numeric,
  ">>",
  "@rshift",
  |x: i64, y: i64| x.checked_shr(y as u32).unwrap_or(0),
  |x: num_bigint::BigInt, y: num_bigint::BigInt| {
    use num_traits::ToPrimitive;
    use std::ops::Shr;
    x.shr(y.to_i64().unwrap_or(0))
  }
);
binary_slow!(
  zuri_jit_bitushr,
  bitwise_numeric,
  ">>>",
  "@urshift",
  |x: i64, y: i64| (x as u32).checked_shr(y as u32).unwrap_or(0) as i64,
  |x: num_bigint::BigInt, y: num_bigint::BigInt| {
    use num_traits::ToPrimitive;
    use std::ops::Shr;
    x.shr(y.to_i64().unwrap_or(0))
  }
);

macro_rules! compare_slow {
  ($name:ident, $op_name:literal, $deco:literal, $op:expr, $big_op:expr) => {
    pub unsafe extern "C" fn $name(vm_ptr: *mut VM, base: u64, dst: u64, a: u64, b: u64) -> u64 {
      let vm = unsafe { vm(vm_ptr) };
      let r = vm.compare(
        base as usize,
        dst as u8,
        a as u8,
        b as u8,
        $op_name,
        $deco,
        $op,
        $big_op,
      );
      match r {
        Ok(()) => OK,
        Err(e) => fail(vm, e),
      }
    }
  };
}

compare_slow!(
  zuri_jit_lt_slow,
  "<",
  "@lt",
  |x: f64, y: f64| x < y,
  |x, y| &x < &y
);
compare_slow!(
  zuri_jit_le_slow,
  "<=",
  "@lte",
  |x: f64, y: f64| x <= y,
  |x, y| &x <= &y
);
compare_slow!(
  zuri_jit_gt_slow,
  ">",
  "@gt",
  |x: f64, y: f64| x > y,
  |x, y| &x > &y
);
compare_slow!(
  zuri_jit_ge_slow,
  ">=",
  "@gte",
  |x: f64, y: f64| x >= y,
  |x, y| &x >= &y
);

/// `Instr::Add`'s full fallback (string concat, list/bytes concat,
/// bigint, `@add` override) -- reuses `binary_add` verbatim, unlike the
/// other arithmetic ops this can't be expressed through the generic
/// `binary_numeric` helper since string/list handling isn't numeric.
pub unsafe extern "C" fn zuri_jit_add_slow(
  vm_ptr: *mut VM,
  base: u64,
  dst: u64,
  a: u64,
  b: u64,
) -> u64 {
  let vm = unsafe { vm(vm_ptr) };
  match vm.binary_add(base as usize, dst as u8, a as u8, b as u8, "+") {
    Ok(()) => OK,
    Err(e) => fail(vm, e),
  }
}

/// `Instr::Mul`'s full fallback (string/list repeat, bigint, `@mul`
/// override).
pub unsafe extern "C" fn zuri_jit_mul_slow(
  vm_ptr: *mut VM,
  base: u64,
  dst: u64,
  a: u64,
  b: u64,
) -> u64 {
  let vm = unsafe { vm(vm_ptr) };
  match vm.binary_mult(base as usize, dst as u8, a as u8, b as u8, "*") {
    Ok(()) => OK,
    Err(e) => fail(vm, e),
  }
}

pub unsafe extern "C" fn zuri_jit_concat(
  vm_ptr: *mut VM,
  base: u64,
  dst: u64,
  a: u64,
  b: u64,
) -> u64 {
  let vm = unsafe { vm(vm_ptr) };
  let va = vm.get_reg(base as usize, a as u8);
  let vb = vm.get_reg(base as usize, b as u8);
  let s = format!("{}{}", va, vb);
  let v = vm.heap.alloc_string(s);
  vm.set_reg(base as usize, dst as u8, v);
  OK
}

pub unsafe extern "C" fn zuri_jit_bitnot_slow(
  vm_ptr: *mut VM,
  base: u64,
  dst: u64,
  src: u64,
) -> u64 {
  let vm = unsafe { vm(vm_ptr) };
  let v = vm.get_reg(base as usize, src as u8);
  match vm.try_operator_override(v, "@not", &[]) {
    Ok(Some(result)) => {
      vm.set_reg(base as usize, dst as u8, result);
      OK
    },
    Ok(None) => {
      let msg = format!("cannot bitwise not a {}", v.argument_type_name());
      let e = vm.raise("TypeError", msg);
      fail(vm, e)
    },
    Err(e) => fail(vm, e),
  }
}

pub unsafe extern "C" fn zuri_jit_neg_slow(vm_ptr: *mut VM, base: u64, dst: u64, src: u64) -> u64 {
  let vm = unsafe { vm(vm_ptr) };
  let v = vm.get_reg(base as usize, src as u8);
  if v.is_bigint() {
    let nv = vm.heap.alloc_bigint(-v.as_bigint());
    vm.set_reg(base as usize, dst as u8, nv);
    return OK;
  }
  match vm.try_operator_override(v, "@neg", &[]) {
    Ok(Some(result)) => {
      vm.set_reg(base as usize, dst as u8, result);
      OK
    },
    Ok(None) => {
      let msg = format!("cannot negate a {}", v.argument_type_name());
      let e = vm.raise("TypeError", msg);
      fail(vm, e)
    },
    Err(e) => fail(vm, e),
  }
}

/// `Instr::Eq`/`Instr::Neq` -- pure structural `Value::equals`, no
/// operator-override lookup at all (matches the interpreter exactly;
/// see `vm.rs`'s own handler, which never calls `try_operator_override`
/// here either). Only reached when the inline number/number fast path
/// in `codegen` doesn't apply -- i.e. at least one operand is a heap
/// object, which needs a real dereference `codegen` can't inline (see
/// this module's docs on stable bit patterns vs. `Obj`'s layout).
pub unsafe extern "C" fn zuri_jit_eq_slow(
  vm_ptr: *mut VM,
  base: u64,
  dst: u64,
  a: u64,
  b: u64,
) -> u64 {
  let vm = unsafe { vm(vm_ptr) };
  let va = vm.get_reg(base as usize, a as u8);
  let vb = vm.get_reg(base as usize, b as u8);
  vm.set_reg(base as usize, dst as u8, Value::bool(va.equals(&vb)));
  OK
}

pub unsafe extern "C" fn zuri_jit_neq_slow(
  vm_ptr: *mut VM,
  base: u64,
  dst: u64,
  a: u64,
  b: u64,
) -> u64 {
  let vm = unsafe { vm(vm_ptr) };
  let va = vm.get_reg(base as usize, a as u8);
  let vb = vm.get_reg(base as usize, b as u8);
  vm.set_reg(base as usize, dst as u8, Value::bool(!va.equals(&vb)));
  OK
}

macro_rules! imm_arith_slow {
  ($name:ident, $op_name:literal, $deco:literal, $op:expr) => {
    pub unsafe extern "C" fn $name(
      vm_ptr: *mut VM,
      base: u64,
      dst: u64,
      a: u64,
      imm_bits: u64,
    ) -> u64 {
      let vm = unsafe { vm(vm_ptr) };
      let imm = f64::from_bits(imm_bits);
      let r = vm.binary_numeric_imm(base as usize, dst as u8, a as u8, imm, $op_name, $deco, $op);
      match r {
        Ok(()) => OK,
        Err(e) => fail(vm, e),
      }
    }
  };
}

imm_arith_slow!(zuri_jit_subimm_slow, "-", "@sub", |x: f64, y: f64| x - y);

/// `Instr::AddImm`'s fallback -- the literal is always numeric (see
/// `compiler::imm_arith_ctor`), but the register operand might not be
/// (`"score: " + 5`-style string concatenation), so this goes through
/// `binary_add_values` (the same general add `binary_add` itself calls)
/// rather than the numeric-only `binary_numeric_imm`.
pub unsafe extern "C" fn zuri_jit_addimm_slow(
  vm_ptr: *mut VM,
  base: u64,
  dst: u64,
  a: u64,
  imm_bits: u64,
) -> u64 {
  let vm = unsafe { vm(vm_ptr) };
  let va = vm.get_reg(base as usize, a as u8);
  let vb = Value::number(f64::from_bits(imm_bits));
  match vm.binary_add_values(va, vb, "+") {
    Ok(result) => {
      vm.set_reg(base as usize, dst as u8, result);
      OK
    },
    Err(e) => fail(vm, e),
  }
}

/// `Instr::MulImm`'s fallback -- string/list repeat (the literal is
/// always numeric) in addition to `@mul` override, mirroring
/// `binary_mult`'s own string/list arms exactly since
/// `binary_numeric_imm` alone doesn't cover them.
pub unsafe extern "C" fn zuri_jit_mulimm_slow(
  vm_ptr: *mut VM,
  base: u64,
  dst: u64,
  a: u64,
  imm_bits: u64,
) -> u64 {
  let vm = unsafe { vm(vm_ptr) };
  let va = vm.get_reg(base as usize, a as u8);
  let imm = f64::from_bits(imm_bits);
  if va.is_string() {
    let count = imm as usize;
    let s = if count < usize::MAX {
      va.as_str().repeat(count)
    } else {
      String::new()
    };
    let v = vm.heap.alloc_string(s);
    vm.set_reg(base as usize, dst as u8, v);
    return OK;
  }
  if va.is_list() {
    let count = imm as usize;
    let value = if count < usize::MAX {
      va.as_list().to_vec().repeat(count)
    } else {
      Vec::new()
    };
    let v = vm.heap.alloc_list(value);
    vm.set_reg(base as usize, dst as u8, v);
    return OK;
  }
  match vm.binary_numeric_imm(
    base as usize,
    dst as u8,
    a as u8,
    imm,
    "*",
    "@mul",
    |x: f64, y: f64| x * y,
  ) {
    Ok(()) => OK,
    Err(e) => fail(vm, e),
  }
}

macro_rules! imm_compare_slow {
  ($name:ident, $op_name:literal, $deco:literal, $op:expr) => {
    pub unsafe extern "C" fn $name(
      vm_ptr: *mut VM,
      base: u64,
      dst: u64,
      a: u64,
      imm_bits: u64,
    ) -> u64 {
      let vm = unsafe { vm(vm_ptr) };
      let imm = f64::from_bits(imm_bits);
      let r = vm.compare_imm(base as usize, dst as u8, a as u8, imm, $op_name, $deco, $op);
      match r {
        Ok(()) => OK,
        Err(e) => fail(vm, e),
      }
    }
  };
}

imm_compare_slow!(zuri_jit_ltimm_slow, "<", "@lt", |x: f64, y: f64| x < y);
imm_compare_slow!(zuri_jit_leimm_slow, "<=", "@lte", |x: f64, y: f64| x <= y);
imm_compare_slow!(zuri_jit_gtimm_slow, ">", "@gt", |x: f64, y: f64| x > y);
imm_compare_slow!(zuri_jit_geimm_slow, ">=", "@gte", |x: f64, y: f64| x >= y);

// ---------------------------------------------------------------------
// Fast, inline-cache-style direct calls -- `Instr::Call`/`Instr::Invoke`'s
// primary path once their target has warmed up, bypassing
// `dispatch_call_sync`'s fully general dispatch (which re-derives arity/
// variadic/frame-setup logic and pays real `Option`/`RunResult`
// plumbing on every single call) entirely for the one case that matters
// most for a JIT's own performance: a call from compiled code straight
// into another already-compiled function.
//
// The `*_prepare` helpers below are a pure peek-and-set-up step: they
// never trigger compilation and never touch `call_count`/the warm-up
// counters. A cold callee (including the very first call that happens
// to cross its own warm-up threshold) always returns `0` here and falls
// through to the fully general slow path (`zuri_jit_call`/
// `zuri_jit_invoke`), which already owns all of that bookkeeping --
// duplicating it here would risk double-counting a single call toward
// warm-up. Once a callee is compiled, every later call to it takes this
// fast path instead.
//
// `codegen::FuncCompiler::emit_fast_call` is the generated-code half of
// this protocol: call `*_prepare`, passing the address of an 8-byte
// scratch stack slot as its last argument; if it returns a non-zero
// entry pointer, read the closure bits `prepare` wrote into that slot
// (needed for `GetUpval`/`SetUpval`/`Instr::Closure` inside the callee
// -- for `Invoke` specifically, the resolved method closure is never
// sitting in any register the caller's own code has access to, only
// inside `prepare`'s own class-method-table lookup, hence the out-
// param rather than reading a register) and `call_indirect` straight
// to the entry point (a real, direct machine call, no further helper
// indirection at all) with `(vm, new_base, closure_bits, -1)`, then
// call `zuri_jit_call_finish` to close upvalues, pop the frame, and
// write the return value -- exactly what `VM::invoke_compiled` does,
// just split across the call boundary so the actual call is a plain
// `call_indirect` instead of a nested Rust function call.
// ---------------------------------------------------------------------

/// `Instr::Call`'s fast-path peek: `func_reg` must hold an already-
/// compiled `Closure`. On success, also performs every bit of frame
/// setup `dispatch_call`'s Closure arm would (via
/// `VM::setup_closure_call`), writes the callee's own `Value` bits to
/// `*closure_out`, and enters one level of JIT call depth (matching
/// `VM::invoke_compiled`'s bracket, mirrored on the other side by
/// `zuri_jit_call_finish`), so by the time this returns non-zero,
/// generated code can go straight to a `call_indirect` with no further
/// setup at all.
pub unsafe extern "C" fn zuri_jit_call_prepare(
  vm_ptr: *mut VM,
  base: u64,
  func_reg: u64,
  num_args: u64,
  dst: u64,
  closure_out: u64,
) -> u64 {
  let vm = unsafe { vm(vm_ptr) };
  let base = base as usize;
  let func_reg = func_reg as u8;
  let callee = vm.get_reg(base, func_reg);
  if !callee.is_closure() || !vm.jit_depth_ok() {
    return 0;
  }
  // Must happen before `closure`/`proto` are derived: `closure_out`'s
  // write below becomes the callee's own `closure_param` -- a plain
  // Cranelift SSA value the compiled callee reuses for its whole
  // invocation, never reloaded from anywhere GC-scannable -- so this
  // closure must never be free to move for as long as that compiled
  // call might still be running. See
  // `VM::ensure_stable_for_compiled_entry`'s own docs.
  let callee = vm.ensure_stable_for_compiled_entry(callee);
  let closure = callee.as_closure();
  let proto = closure.function.as_func();
  let Some(entry) = proto.jit.entry.get() else {
    return 0;
  };
  let new_base = base + func_reg as usize + 1;
  vm.setup_closure_call(callee, closure, proto, new_base, num_args as u8, dst as u8);
  vm.mark_top_frame_compiled();
  vm.jit_depth_enter();
  unsafe { *(closure_out as *mut u64) = callee.to_bits() };
  entry as usize as u64
}

/// `Instr::Invoke`'s fast-path peek: `obj` must hold an instance whose
/// class resolves `method_name_bits` to an already-compiled `Closure`
/// method (not a field holding a callable, and not a builtin/native
/// fallback -- both of those still go through the fully general
/// `zuri_jit_invoke`). Mirrors `zuri_jit_call_prepare` otherwise (see
/// its docs for the full protocol), except the value written to
/// `*closure_out` is the resolved method, not the receiver in `obj`.
// `closure_out` is declared last here, not next to `method_name_bits`,
// because `codegen::FuncCompiler::emit_fast_call` always appends the
// closure-out-slot address as the final argument to whatever
// `prepare_args` it's given -- the parameter order here must match
// that calling convention exactly, or the wrong register-sized slot
// ends up interpreted as a pointer.
pub unsafe extern "C" fn zuri_jit_invoke_prepare(
  vm_ptr: *mut VM,
  base: u64,
  obj: u64,
  num_args: u64,
  dst: u64,
  method_name_bits: u64,
  func_ptr_bits: u64,
  instr_ip: u64,
  closure_out: u64,
) -> u64 {
  let vm = unsafe { vm(vm_ptr) };
  let base = base as usize;
  let obj = obj as u8;
  let receiver = vm.get_reg(base, obj);
  if !receiver.is_instance() || !vm.jit_depth_ok() {
    return 0;
  }
  let inst = receiver.as_instance();
  let class_bits = inst.class.to_bits();
  let func = unsafe { &*(func_ptr_bits as *const ObjFunction) };
  let instr_ip = instr_ip as usize;

  // Inline cache: same receiver class as the last time this exact
  // `Instr::Invoke` ran -> reuse the resolved method `Value` directly,
  // skipping `ObjClass::methods`'s hash-map probe entirely. See
  // `Chunk::invoke_cache`'s own docs.
  let cell = func.chunk.invoke_cache_cell(instr_ip);
  let cached = cell
    .filter(|c| c.key.get() == class_bits)
    .map(|c| Value::from_bits(c.payload.get()));

  let method = if let Some(m) = cached {
    Some(m)
  } else {
    let method_name = Value::from_bits(method_name_bits);
    let class = inst.class.as_class();
    let resolved = class.methods.get(method_name.as_str()).copied();
    if let (Some(m), Some(c)) = (resolved, cell) {
      c.key.set(class_bits);
      c.payload.set(m.to_bits());
    }
    resolved
  };
  let Some(method) = method else {
    return 0;
  };
  if !method.is_closure() {
    return 0;
  }
  // See `zuri_jit_call_prepare`'s identical call for why this must
  // happen before `closure`/`proto` are derived.
  let method = vm.ensure_stable_for_compiled_entry(method);
  let closure = method.as_closure();
  let proto = closure.function.as_func();
  let Some(entry) = proto.jit.entry.get() else {
    return 0;
  };
  // `1 + num_args`: the receiver the compiler already duplicated into
  // `obj + 1` occupies the callee's own register 0 ("self") -- see
  // `Instr::Invoke`'s own doc comment in chunk.rs, and
  // `VM::invoke_prebound_inner`'s identical convention.
  let new_base = base + obj as usize + 1;
  vm.setup_closure_call(
    method,
    closure,
    proto,
    new_base,
    1 + num_args as u8,
    dst as u8,
  );
  vm.mark_top_frame_compiled();
  vm.jit_depth_enter();
  unsafe { *(closure_out as *mut u64) = method.to_bits() };
  entry as usize as u64
}

/// `Instr::Call`'s fast-path peek for a class callee -- the
/// constructor equivalent of `zuri_jit_call_prepare`, following the
/// exact same generated-code protocol (see that function's docs, and
/// `codegen::FuncCompiler::emit_fast_call`): non-zero return means
/// "frame is set up, `*closure_out` holds the callee closure bits,
/// `call_indirect` straight to this entry point", zero means "nothing
/// happened here, take the general slow path".
///
/// The one place it diverges is the finish half: a constructor call
/// evaluates to the instance, never to whatever the constructor body
/// returned, so generated code must pair this with
/// `zuri_jit_new_finish` and not `zuri_jit_call_finish`. See
/// `VM::prepare_compiled_construction` for which constructor shapes
/// this accepts and why.
pub unsafe extern "C" fn zuri_jit_new_prepare(
  vm_ptr: *mut VM,
  base: u64,
  func_reg: u64,
  num_args: u64,
  dst: u64,
  closure_out: u64,
) -> u64 {
  let vm = unsafe { vm(vm_ptr) };
  let Some((entry, ctor)) =
    vm.prepare_compiled_construction(base as usize, func_reg as u8, num_args as u8, dst as u8)
  else {
    return 0;
  };
  vm.mark_top_frame_compiled();
  unsafe { *(closure_out as *mut u64) = ctor.to_bits() };
  entry as usize as u64
}

/// `zuri_jit_new_prepare`'s proven twin (`jit::CallTarget
/// ::ConstructKnown`): every resolution step the dynamic version pays
/// per call -- the callee tag check, the `ObjClass` borrow for
/// `field_count`, the ancestor field-initializer walk, the
/// constructor tag check, the closure -> function hop, the variadic
/// check -- was discharged at compile time by
/// `VM::resolve_construct_target`, and is licensed here by the class-
/// identity + `method_table_generation` guard generated code ran
/// immediately before calling this.
///
/// What remains is genuinely per-call: the constructor's `jit.entry`
/// read straight off the baked `proto_ptr` (a zero just means "not
/// compiled yet", and generated code falls through to the general path
/// exactly like any other prepare miss), and one read of the guarded
/// class's `constructor`, deliberately not baked -- see
/// `VM::resolve_construct_target` for why.
///
/// Pairs with `zuri_jit_new_finish`, not `zuri_jit_call_finish` -- the
/// call still evaluates to the instance.
pub unsafe extern "C" fn zuri_jit_construct_prepare(
  vm_ptr: *mut VM,
  base: u64,
  func_reg: u64,
  num_args: u64,
  dst: u64,
  ctor_bits: u64,
  proto_ptr: u64,
  field_count: u64,
  closure_out: u64,
) -> u64 {
  let vm = unsafe { vm(vm_ptr) };
  let num_args = num_args as u8;
  if !vm.jit_depth_ok() || num_args == u8::MAX {
    return 0;
  }
  // SAFETY: `proto_ptr` was baked by `VM::resolve_construct_target`
  // from a live `ObjFunction`, which never moves and stays reachable
  // through the very class the caller's guard just matched.
  let proto = unsafe { &*(proto_ptr as *const ObjFunction) };
  let Some(entry) = proto.jit.entry.get() else {
    return 0;
  };
  // Both the `ObjClass` read that used to find `constructor` and the
  // `ensure_stable_for_compiled_entry` generation probe are gone:
  // class methods live in the non-relocating old generation (see
  // `Heap::alloc_closure`), so this closure cannot move and its bits
  // were baked at compile time. Those were two dependent, cache-
  // missing loads on the path of every instance built.
  let ctor = Value::from_bits(ctor_bits);
  vm.prepare_known_construction(
    base as usize,
    func_reg as u8,
    num_args,
    dst as u8,
    ctor,
    proto,
    field_count as usize,
  );
  vm.mark_top_frame_compiled();
  unsafe { *(closure_out as *mut u64) = ctor_bits };
  entry as usize as u64
}

/// `jit::codegen`'s inline construct fast path's ONE remaining real
/// call: allocating the instance and pinning it, exactly `VM::
/// alloc_and_pin_instance`'s job (see its own docs for why this is
/// deliberately NOT paired with a register-window growth check --
/// generated code has already verified the window fits, via the same
/// `emit_call_checks` its matching `emit_frame_construction` call
/// uses). Returns the new instance's `Value` bits.
pub unsafe extern "C" fn zuri_jit_alloc_and_pin_instance(
  vm_ptr: *mut VM,
  base: u64,
  func_reg: u64,
  field_count: u64,
) -> u64 {
  let vm = unsafe { vm(vm_ptr) };
  vm.alloc_and_pin_instance(base as usize, func_reg as u8, field_count as usize)
    .to_bits()
}

/// `jit::codegen`'s inline construct fast path's other mandatory real
/// call, alongside `zuri_jit_alloc_and_pin_instance`: releasing the
/// `gc_pins` entry that call pushed. Unlike `Instr::CloseUpvalues`
/// (usually a no-op, so worth an inline emptiness check first), a
/// construct call's pin is ALWAYS there to release exactly once, so
/// there's no hot-path variant of this worth skipping -- see
/// `VM::take_constructed_instance`'s own docs for why this specific
/// step has to happen exactly here (after any collection the callee's
/// own execution could have triggered, before the instance is ever
/// treated as a plain untracked Rust value again).
pub unsafe extern "C" fn zuri_jit_take_constructed_instance(vm_ptr: *mut VM) -> u64 {
  let vm = unsafe { vm(vm_ptr) };
  vm.take_constructed_instance().to_bits()
}

/// `zuri_jit_new_prepare`'s other half -- `zuri_jit_call_finish` with
/// the constructor's own return value discarded in favour of the
/// instance, which is why it takes no `ret_bits` at all.
pub unsafe extern "C" fn zuri_jit_new_finish(
  vm_ptr: *mut VM,
  base: u64,
  dst: u64,
  new_base: u64,
) -> u64 {
  let vm = unsafe { vm(vm_ptr) };
  vm.jit_depth_exit();

  // Same reasoning (and same ordering requirement) as
  // `zuri_jit_call_finish`'s -- see its docs.
  let deopt = vm.resolve_possible_deopt();

  // Released on every path out of here, error ones included -- the pin
  // is `prepare`'s, and nothing further down would ever drop it.
  //
  // Strictly after `resolve_possible_deopt`, never before: that call
  // resumes the constructor in the interpreter and runs arbitrary Zuri
  // code, collections included. Reading the instance out first would
  // leave it in nothing but a local for that whole window -- the exact
  // stale-after-relocation hazard `gc_pins` exists to close. By here
  // no further Zuri code can run, so the pin has done its job.
  let instance = vm.take_constructed_instance();

  if let Some(result) = deopt {
    return match result {
      Ok(_) => {
        vm.set_reg(base as usize, dst as u8, instance);
        OK
      },
      Err(e) => fail(vm, e),
    };
  }

  if !vm.jit_pending_exception.get().is_nil() {
    return ERR;
  }
  vm.close_upvalues_from(new_base as usize);
  vm.pop_frame();
  vm.set_reg(base as usize, dst as u8, instance);
  OK
}

/// The lean frame-setup half of `codegen::FuncCompiler`'s statically-
/// resolved direct-call paths (`emit_self_call`/`emit_known_call`/
/// `emit_self_invoke` -- see `jit::CallTarget`'s own docs for how those
/// callees get proven ahead of time). Unlike `zuri_jit_call_prepare`/
/// `zuri_jit_invoke_prepare`, this does no resolution work at all --
/// `callee_bits` is already known to be exactly the right closure
/// (either this function's own, for self-recursion, or one already
/// guarded by a value/class-identity check in generated code), so this
/// is purely `VM::setup_closure_call`'s frame push plus the depth
/// bookkeeping every compiled-to-compiled call needs, with none of
/// `is_closure`/`ensure_stable_for_compiled_entry`/`proto.jit.entry
/// .get()`'s per-call overhead. Returns `1` (proceed with the direct
/// call generated code already has the target address for) or `0`
/// (native call-stack depth exhausted -- generated code falls back to
/// the fully general slow helper on this result, exactly like
/// `emit_fast_call`'s own miss path).
pub unsafe extern "C" fn zuri_jit_direct_call_prepare(
  vm_ptr: *mut VM,
  callee_bits: u64,
  new_base: u64,
  num_args: u64,
  dst: u64,
) -> u64 {
  let vm = unsafe { vm(vm_ptr) };
  if !vm.jit_depth_ok() {
    return 0;
  }
  let callee = Value::from_bits(callee_bits);
  let closure = callee.as_closure();
  let proto = closure.function.as_func();
  vm.setup_closure_call(
    callee,
    closure,
    proto,
    new_base as usize,
    num_args as u8,
    dst as u8,
  );
  vm.mark_top_frame_compiled();
  vm.jit_depth_enter();
  1
}

/// Completes a fast-path direct call after generated code's own
/// `call_indirect` returns -- the other half of `zuri_jit_call_prepare`/
/// `zuri_jit_invoke_prepare`'s bracket. `new_base` is the callee's own
/// frame base (needed to close its upvalues); `base`/`dst` are the
/// caller's, to write the return value into the right place. Follows
/// the ordinary OK(0)/ERR(1) helper convention -- on error, the pending
/// exception is left exactly as the failing compiled call already set
/// it (see this module's top-level docs), and the frame is left in
/// place, matching `VM::invoke_compiled`'s own error-path semantics
/// precisely (compiled code never pops its own frame on error).
pub unsafe extern "C" fn zuri_jit_call_finish(
  vm_ptr: *mut VM,
  base: u64,
  dst: u64,
  new_base: u64,
  ret_bits: u64,
) -> u64 {
  let vm = unsafe { vm(vm_ptr) };
  vm.jit_depth_exit();

  // Must be checked before anything else treats `ret_bits` as a real
  // return value: this is the direct compiled-to-compiled fast path
  // (`codegen::FuncCompiler::emit_fast_call`'s own `call_indirect`,
  // never going through `VM::invoke_compiled` at all), so it's the
  // only place that would ever see a deopt from a callee reached this
  // way -- e.g. a self-recursive call. See
  // `VM::resolve_possible_deopt`'s own docs.
  if let Some(result) = vm.resolve_possible_deopt() {
    return match result {
      Ok(v) => {
        vm.set_reg(base as usize, dst as u8, v);
        OK
      },
      Err(e) => fail(vm, e),
    };
  }

  if !vm.jit_pending_exception.get().is_nil() {
    return ERR;
  }
  vm.close_upvalues_from(new_base as usize);
  vm.pop_frame();
  vm.set_reg(base as usize, dst as u8, Value::from_bits(ret_bits));
  OK
}

/// The one part of `zuri_jit_call_finish`'s job that `jit::codegen`'s
/// inline call/return fast path (`emit_inline_frame_finish`) can't
/// safely reproduce inline: resuming a deopting callee through the
/// interpreter. Called ONLY after generated code has already observed
/// `VM::pending_deopt_ip != -1` and already done the depth decrement
/// `zuri_jit_call_finish` itself does unconditionally -- everything
/// else (popping the frame, closing upvalues) is deliberately skipped
/// here because a deopt never reaches that far: `resolve_deopt_slow`
/// resumes the SAME already-pushed frame through the interpreter's own
/// `Instr::Return` handling, which pops it itself by the time this
/// returns. See `VM::resolve_possible_deopt`'s own docs.
pub unsafe extern "C" fn zuri_jit_finish_deopt(vm_ptr: *mut VM, base: u64, dst: u64) -> u64 {
  let vm = unsafe { vm(vm_ptr) };
  match vm.resolve_possible_deopt() {
    Some(Ok(v)) => {
      vm.set_reg(base as usize, dst as u8, v);
      OK
    },
    Some(Err(e)) => fail(vm, e),
    // Unreachable by construction -- generated code only ever calls
    // this after observing a deopt actually pending. Treated as a
    // no-op success rather than a panic: unwinding a Rust panic across
    // this `extern "C"` boundary would be unsound, and this is the
    // strictly safer failure mode if the invariant were ever violated.
    None => OK,
  }
}

// ---------------------------------------------------------------------
// Calls -- mixed-mode dispatch. `dispatch_call_sync`/`invoke_prebound_sync`
// already fully implement "run interpreted, or run compiled if
// warm/available, and return synchronously either way" (see vm.rs) --
// these wrappers just unpack the instruction's register operands.
// ---------------------------------------------------------------------

pub unsafe extern "C" fn zuri_jit_call(
  vm_ptr: *mut VM,
  base: u64,
  func_reg: u64,
  num_args: u64,
  dst: u64,
) -> u64 {
  let vm = unsafe { vm(vm_ptr) };
  match vm.dispatch_call_sync(base as usize, func_reg as u8, num_args as u8, dst as u8) {
    Ok(()) => OK,
    Err(e) => fail(vm, e),
  }
}

/// `builtins::lookup`, backed by this call site's own monomorphic cache.
///
/// The lookup itself is a hash of the method name plus the `memcmp`
/// that confirms it, against a table chosen by the receiver's kind --
/// and the tables are `'static` and immutable after startup, so a site
/// that saw a string receiver last time will resolve the same name to
/// the same function pointer every time it sees a string again. Caching
/// on `method_table_key` reduces the steady state to one integer
/// compare. A `0` key means the receiver has no builtin table at all
/// (see `method_table_key`), and a `None` result is deliberately not
/// cached -- there is nothing to store, and a miss falls straight
/// through to the error path anyway.
fn cached_builtin_lookup(
  cache: Option<&InvokeCacheCell>,
  receiver: Value,
  name: &str,
) -> Option<&'static NativeFunction> {
  let key = crate::builtins::method_table_key(receiver);
  if key == 0 {
    return crate::builtins::lookup(receiver, name);
  }
  if let Some(cell) = cache
    && cell.key.get() == key
  {
    // SAFETY: only ever written just below, from a `&'static
    // NativeFunction` this same function resolved.
    return Some(unsafe { &*(cell.payload.get() as *const NativeFunction) });
  }
  let found = crate::builtins::lookup(receiver, name);
  if let (Some(native), Some(cell)) = (found, cache) {
    cell.key.set(key);
    cell.payload.set(native as *const NativeFunction as u64);
  }
  found
}

/// `Instr::Invoke` -- dynamic dispatch through the receiver's actual
/// runtime class (or builtin-method table, or a field holding a
/// callable), mirroring `vm.rs`'s handler exactly. `method_name_bits`
/// is the method name's baked `Value` (always a string constant).
pub unsafe extern "C" fn zuri_jit_invoke(
  vm_ptr: *mut VM,
  base: u64,
  obj: u64,
  num_args: u64,
  dst: u64,
  method_name_bits: u64,
  cache_addr: u64,
) -> u64 {
  let vm = unsafe { vm(vm_ptr) };
  let base = base as usize;
  let obj = obj as u8;
  let num_args = num_args as u8;
  let dst = dst as u8;
  let method_name = Value::from_bits(method_name_bits);
  let receiver = vm.get_reg(base, obj);
  let cache = unsafe { (cache_addr as *const InvokeCacheCell).as_ref() };

  let result = (|| -> Result<(), Value> {
    if receiver.is_instance() {
      let inst = receiver.as_instance();
      let class_val = inst.class;
      let found = {
        let class = class_val.as_class();
        if let Some(m) = class.methods.get(method_name.as_str()).copied() {
          Some(Ok(m))
        } else if let Some(&idx) = class.field_slots.get(method_name.as_str()) {
          Some(Err(idx))
        } else {
          None
        }
      };
      match found {
        Some(Ok(method)) => vm.invoke_prebound_sync(base, obj, method, num_args, dst),
        Some(Err(idx)) => {
          let field_value = inst.fields[idx as usize].get();
          vm.set_reg(base, obj + 1, field_value);
          vm.dispatch_call_sync(base, obj + 1, num_args, dst)
        },
        None => match cached_builtin_lookup(cache, receiver, method_name.as_str()) {
          Some(native) => {
            let call_args = invoke_native_args(vm, base, obj, num_args, receiver);
            let result = vm.call_native(native, call_args.as_slice())?;
            vm.set_reg(base, dst, result);
            Ok(())
          },
          None => {
            let msg = format!(
              "undefined property '{}' on instance of '{}'",
              method_name.as_str(),
              class_val.as_class().name
            );
            Err(vm.raise("PropertyError", msg))
          },
        },
      }
    } else if receiver.is_class() {
      let callee = crate::vm::vm::lookup_static(receiver, method_name.as_str()).ok_or_else(|| {
        format!(
          "undefined static member '{}' on class '{}'",
          method_name.as_str(),
          receiver.as_class().name
        )
      });
      let callee = match callee {
        Ok(c) => c,
        Err(msg) => return Err(vm.raise("PropertyError", msg)),
      };
      if callee.is_closure() && callee.as_closure().function.as_func().is_method {
        vm.invoke_prebound_sync(base, obj, callee, num_args, dst)
      } else {
        vm.set_reg(base, obj + 1, callee);
        vm.dispatch_call_sync(base, obj + 1, num_args, dst)
      }
    } else if receiver.is_module() || receiver.is_module_binding() {
      let module_val = if receiver.is_module() {
        receiver
      } else {
        receiver.as_module_binding().module
      };
      let member = { module_val.as_module().namespace.get(method_name.as_str()) };
      match member {
        Some(v) => {
          vm.set_reg(base, obj + 1, v);
          vm.dispatch_call_sync(base, obj + 1, num_args, dst)
        },
        None => match cached_builtin_lookup(cache, receiver, method_name.as_str()) {
          Some(native) => {
            let call_args = invoke_native_args(vm, base, obj, num_args, receiver);
            let result = vm.call_native(native, call_args.as_slice())?;
            vm.set_reg(base, dst, result);
            Ok(())
          },
          None => {
            let msg = format!("undefined member '{}' on module", method_name.as_str());
            Err(vm.raise("PropertyError", msg))
          },
        },
      }
    } else {
      invoke_builtin_method(vm, base, obj, num_args, dst, method_name, cache, receiver)
    }
  })();

  match result {
    Ok(()) => OK,
    Err(e) => fail(vm, e),
  }
}

/// The tail both `zuri_jit_invoke`'s own catch-all branch and
/// `zuri_jit_invoke_string` end at: `receiver` isn't an Instance, Class,
/// or Module, so the only thing left to try is a builtin method off
/// `builtins::lookup` -- resolved once per call site and cached in
/// `cache` by `cached_builtin_lookup`, keyed on `builtins::
/// method_table_key(receiver)`.
fn invoke_builtin_method(
  vm: &mut VM,
  base: usize,
  obj: u8,
  num_args: u8,
  dst: u8,
  method_name: Value,
  cache: Option<&InvokeCacheCell>,
  receiver: Value,
) -> Result<(), Value> {
  match cached_builtin_lookup(cache, receiver, method_name.as_str()) {
    Some(native) => {
      let call_args = invoke_native_args(vm, base, obj, num_args, receiver);
      let result = vm.call_native(native, call_args.as_slice())?;
      vm.set_reg(base, dst, result);
      Ok(())
    },
    None => {
      let msg = format!(
        "object of type {} does not define method '{}'",
        receiver.type_name(),
        method_name.as_str()
      );
      Err(vm.raise("TypeError", msg))
    },
  }
}

/// `Instr::Invoke` when `jit::typeflow::StringFacts` proves the
/// receiver is ALWAYS a `Value::String` at this call site -- a leaner
/// entry point than `zuri_jit_invoke` that skips straight to
/// `invoke_builtin_method`, the only branch of `zuri_jit_invoke`'s own
/// `is_instance`/`is_class`/`is_module` chain a String can ever reach.
/// See `codegen::FuncCompiler::emit_string_invoke`'s own docs for why
/// paying for `zuri_jit_invoke_prepare` first (as every OTHER `Instr::
/// Invoke` site does) is pure waste on a proven-string receiver.
pub unsafe extern "C" fn zuri_jit_invoke_string(
  vm_ptr: *mut VM,
  base: u64,
  obj: u64,
  num_args: u64,
  dst: u64,
  method_name_bits: u64,
  cache_addr: u64,
) -> u64 {
  let vm = unsafe { vm(vm_ptr) };
  let base = base as usize;
  let obj = obj as u8;
  let num_args = num_args as u8;
  let dst = dst as u8;
  let method_name = Value::from_bits(method_name_bits);
  let receiver = vm.get_reg(base, obj);
  let cache = unsafe { (cache_addr as *const InvokeCacheCell).as_ref() };

  let result = invoke_builtin_method(vm, base, obj, num_args, dst, method_name, cache, receiver);
  match result {
    Ok(()) => OK,
    Err(e) => fail(vm, e),
  }
}

/// Builds the args `Instr::Invoke`'s native/builtin-method fallback
/// needs (receiver spliced in as `args[0]`, matching `VM::call_native`'s
/// `is_method` convention) -- `CallArgs`, not a bare `Vec`, so the
/// overwhelming majority of calls (`INLINE_ARGS` == 8 args or fewer,
/// which is every string/list/dict method that exists today) pay no
/// heap allocation at all. This is the same `CallArgs` construction
/// `vm.rs`'s own handler does at each of these three call sites, and
/// `zuri_jit_call_native`'s identical fix just above.
fn invoke_native_args(vm: &VM, base: usize, obj: u8, num_args: u8, receiver: Value) -> CallArgs {
  let mut args = CallArgs::new();
  args.push(receiver);
  for i in 0..num_args {
    args.push(vm.get_reg(base, obj + 2 + i));
  }
  args
}

pub unsafe extern "C" fn zuri_jit_invoke_super(
  vm_ptr: *mut VM,
  base: u64,
  superclass: u64,
  num_args: u64,
  dst: u64,
  method_name_bits: u64,
) -> u64 {
  let vm = unsafe { vm(vm_ptr) };
  let base = base as usize;
  let superclass = superclass as u8;
  let num_args = num_args as u8;
  let dst = dst as u8;
  let method_name = Value::from_bits(method_name_bits);
  let super_val = vm.get_reg(base, superclass);

  let result = (|| -> Result<(), Value> {
    if !super_val.is_class() {
      let msg = format!(
        "'parent' does not refer to a class (got a {})",
        super_val.type_name()
      );
      return Err(vm.raise("TypeError", msg));
    }
    let found = {
      let class = super_val.as_class();
      class.methods.get(method_name.as_str()).copied()
    };
    if let Some(method) = found {
      return vm.invoke_prebound_sync(base, superclass, method, num_args, dst);
    }
    let self_val = vm.get_reg(base, superclass + 1);
    if !self_val.is_instance() {
      let msg = format!(
        "undefined method '{}' on superclass '{}'",
        method_name.as_str(),
        super_val.as_class().name
      );
      return Err(vm.raise("PropertyError", msg));
    }
    let inst = self_val.as_instance();
    let idx = match inst
      .class
      .as_class()
      .field_slots
      .get(method_name.as_str())
      .copied()
    {
      Some(idx) => idx,
      None => {
        let msg = format!(
          "undefined method '{}' on superclass '{}'",
          method_name.as_str(),
          super_val.as_class().name
        );
        return Err(vm.raise("PropertyError", msg));
      },
    };
    let field_value = inst.fields[idx as usize].get();
    vm.set_reg(base, superclass + 1, field_value);
    vm.dispatch_call_sync(base, superclass + 1, num_args, dst)
  })();

  match result {
    Ok(()) => OK,
    Err(e) => fail(vm, e),
  }
}

pub unsafe extern "C" fn zuri_jit_call_super_ctor(
  vm_ptr: *mut VM,
  base: u64,
  superclass: u64,
  num_args: u64,
  dst: u64,
) -> u64 {
  let vm = unsafe { vm(vm_ptr) };
  let base = base as usize;
  let superclass = superclass as u8;
  let super_val = vm.get_reg(base, superclass);

  let result = (|| -> Result<(), Value> {
    if !super_val.is_class() {
      let msg = format!(
        "'parent' does not refer to a class (got a {})",
        super_val.type_name()
      );
      return Err(vm.raise("TypeError", msg));
    }
    match super_val.as_class().constructor {
      Some(ctor) => vm.invoke_prebound_sync(base, superclass, ctor, num_args as u8, dst as u8),
      None => {
        let msg = format!(
          "class '{}' has no constructor to call via parent()",
          super_val.as_class().name
        );
        Err(vm.raise("AccessError", msg))
      },
    }
  })();

  match result {
    Ok(()) => OK,
    Err(e) => fail(vm, e),
  }
}

// ---------------------------------------------------------------------
// Globals -- mirrors GetGlobal/SetGlobal/AssignGlobal's inline-cache
// logic in vm.rs exactly, including the cache keyed by this exact
// instruction's own bytecode `ip` (baked as `instr_ip`, a compile-time
// constant -- this instruction's position in `chunk.code` never
// changes once compiled).
// ---------------------------------------------------------------------

pub unsafe extern "C" fn zuri_jit_get_global(
  vm_ptr: *mut VM,
  base: u64,
  dst: u64,
  func_ptr_bits: u64,
  name_bits: u64,
  instr_ip: u64,
) -> u64 {
  let vm = unsafe { vm(vm_ptr) };
  let func = unsafe { &*(func_ptr_bits as *const ObjFunction) };
  let gmod = func.globals_module;
  let instr_ip = instr_ip as usize;

  let resolved = if let Some(&cached) = func.chunk.global_cache.borrow().get(&instr_ip) {
    Some(cached)
  } else {
    let name_val = Value::from_bits(name_bits);
    vm.resolve_global(gmod, name_val.as_str())
  };

  match resolved {
    Some((is_root, slot)) => {
      func
        .chunk
        .global_cache
        .borrow_mut()
        .insert(instr_ip, (is_root, slot));
      // Also populate the JIT-only array cache (`codegen::FuncCompiler`'s
      // inline fast path -- see `JitInfo::global_slot_cache`'s own docs)
      // so every later execution of this instruction, from compiled
      // code, skips this whole helper call. Root-globals only: a
      // qualified-module resolution (`is_root == false`) would need the
      // module's own namespace-slots pointer cached the same careful
      // way `VM::global_slots_ptr_cache` is, which nothing here does
      // yet -- left as a real helper-call miss every time rather than
      // baking in an unsound fast path.
      if is_root {
        func.jit.global_slot_cache[instr_ip].set(slot as i64);
      }
      let v = vm.read_resolved(gmod, is_root, slot);
      vm.set_reg(base as usize, dst as u8, v);
      OK
    },
    None => {
      let name_val = Value::from_bits(name_bits);
      let msg = format!("undefined global '{}'", name_val.as_str());
      let e = vm.raise("UndefinedError", msg);
      fail(vm, e)
    },
  }
}

pub unsafe extern "C" fn zuri_jit_set_global(
  vm_ptr: *mut VM,
  base: u64,
  src: u64,
  func_ptr_bits: u64,
  name_bits: u64,
  instr_ip: u64,
) -> u64 {
  let vm = unsafe { vm(vm_ptr) };
  let func = unsafe { &*(func_ptr_bits as *const ObjFunction) };
  let gmod = func.globals_module;
  let instr_ip = instr_ip as usize;

  let slot = if let Some(&(_, s)) = func.chunk.global_cache.borrow().get(&instr_ip) {
    s
  } else {
    let name_val = Value::from_bits(name_bits);
    let s = vm.get_or_create_slot_in(gmod, name_val.as_str().to_string());
    let is_root = gmod.is_none();
    func
      .chunk
      .global_cache
      .borrow_mut()
      .insert(instr_ip, (is_root, s));
    // See `zuri_jit_get_global`'s identical comment -- only a root
    // resolution is safe to fast-path from generated code today.
    if is_root {
      func.jit.global_slot_cache[instr_ip].set(s as i64);
    }
    s
  };
  let v = vm.get_reg(base as usize, src as u8);
  vm.write_slot_in(gmod, slot, v);
  OK
}

pub unsafe extern "C" fn zuri_jit_assign_global(
  vm_ptr: *mut VM,
  base: u64,
  src: u64,
  func_ptr_bits: u64,
  name_bits: u64,
  instr_ip: u64,
) -> u64 {
  let vm = unsafe { vm(vm_ptr) };
  let func = unsafe { &*(func_ptr_bits as *const ObjFunction) };
  let gmod = func.globals_module;
  let instr_ip = instr_ip as usize;

  let resolved = if let Some(&cached) = func.chunk.global_cache.borrow().get(&instr_ip) {
    Some(cached)
  } else {
    let name_val = Value::from_bits(name_bits);
    vm.resolve_global(gmod, name_val.as_str())
  };

  match resolved {
    Some((is_root, slot)) => {
      func
        .chunk
        .global_cache
        .borrow_mut()
        .insert(instr_ip, (is_root, slot));
      if is_root {
        func.jit.global_slot_cache[instr_ip].set(slot as i64);
      }
      let v = vm.get_reg(base as usize, src as u8);
      vm.write_resolved(gmod, is_root, slot, v);
      OK
    },
    None => {
      let name_val = Value::from_bits(name_bits);
      let msg = format!("undefined global '{}'", name_val.as_str());
      let e = vm.raise("UndefinedError", msg);
      fail(vm, e)
    },
  }
}

// ---------------------------------------------------------------------
// Closures / upvalues
// ---------------------------------------------------------------------

/// `Instr::Closure` -- `proto_bits` is the baked function-prototype
/// constant; `closure_bits` is the currently executing closure (this
/// compiled function's own `closure` parameter), needed to resolve an
/// `UpvalueDescriptor::Upvalue` (capture-through) entry.
pub unsafe extern "C" fn zuri_jit_make_closure(
  vm_ptr: *mut VM,
  base: u64,
  dst: u64,
  proto_bits: u64,
  closure_bits: u64,
) -> u64 {
  let vm = unsafe { vm(vm_ptr) };
  let base = base as usize;
  let proto_val = Value::from_bits(proto_bits);
  let proto = proto_val.as_func();
  let closure_val = Value::from_bits(closure_bits);
  let current_closure: &ObjClosure = closure_val.as_closure();

  let mut captured = Vec::with_capacity(proto.upvalues.len());
  for desc in &proto.upvalues {
    let upval = match *desc {
      UpvalueDescriptor::Local(reg) => {
        let abs_index = base + reg as usize;
        vm.capture_upvalue(abs_index)
      },
      UpvalueDescriptor::Upvalue(idx) => current_closure.upvalues[idx as usize],
    };
    captured.push(upval);
  }

  let closure_val = vm.heap.alloc_closure(ObjClosure {
    function: proto_val,
    upvalues: captured,
  });
  vm.set_reg(base, dst as u8, closure_val);
  OK
}

/// `Instr::GetUpval`/`SetUpval`'s inline fast path only needs a raw base
/// pointer into `ObjClosure::upvalues` -- a real `Vec<Value>`, not a
/// hand-rolled `#[repr(C)]` one like `ListStorage`, so its own internal
/// field layout isn't something generated code should ever assume.
/// Mirrors `zuri_jit_list_data`'s exact reasoning: one small, call-
/// cheap, allocation-free helper resolves the pointer; the index bounds
/// (always in range by construction -- a closure's `upvalues` has
/// exactly one entry per its prototype's own `upvalues` descriptor list,
/// and `GetUpval`/`SetUpval`'s `idx` is compiled straight from that same
/// list) and the actual element load happen as real inline Cranelift
/// code either side of this call. Never fails, touches no VM register,
/// so `emit_upvalue_fast_path` calls this via `call_helper_raw`, not
/// `call_checked`.
pub unsafe extern "C" fn zuri_jit_closure_upvalues_ptr(_vm_ptr: *mut VM, closure_bits: u64) -> u64 {
  let closure_val = Value::from_bits(closure_bits);
  closure_val.as_closure().upvalues.as_ptr() as u64
}

pub unsafe extern "C" fn zuri_jit_get_upval(
  vm_ptr: *mut VM,
  base: u64,
  dst: u64,
  idx: u64,
  closure_bits: u64,
) -> u64 {
  let vm = unsafe { vm(vm_ptr) };
  let closure_val = Value::from_bits(closure_bits);
  let current_closure = closure_val.as_closure();
  let upval_val = current_closure.upvalues[idx as usize];
  if !upval_val.is_upvalue() {
    let e = vm.raise("TypeError", "GetUpval operand is not an upvalue");
    return fail(vm, e);
  }
  let v = match upval_val.as_upvalue().get() {
    UpvalueState::Open(abs_idx) => vm.get_reg_abs(abs_idx),
    UpvalueState::Closed(v) => v,
  };
  vm.set_reg(base as usize, dst as u8, v);
  OK
}

pub unsafe extern "C" fn zuri_jit_set_upval(
  vm_ptr: *mut VM,
  base: u64,
  src: u64,
  idx: u64,
  closure_bits: u64,
) -> u64 {
  let vm = unsafe { vm(vm_ptr) };
  let v = vm.get_reg(base as usize, src as u8);
  let closure_val = Value::from_bits(closure_bits);
  let current_closure = closure_val.as_closure();
  let upval_val = current_closure.upvalues[idx as usize];
  if !upval_val.is_upvalue() {
    let e = vm.raise("TypeError", "SetUpval operand is not an upvalue");
    return fail(vm, e);
  }
  let cell = upval_val.as_upvalue();
  match cell.get() {
    UpvalueState::Open(abs_idx) => vm.set_reg_abs(abs_idx, v),
    UpvalueState::Closed(_) => {
      cell.set(UpvalueState::Closed(v));
      write_barrier(upval_val.as_obj());
    },
  }
  OK
}

pub unsafe extern "C" fn zuri_jit_close_upvalues(vm_ptr: *mut VM, base: u64, from: u64) -> u64 {
  let vm = unsafe { vm(vm_ptr) };
  vm.close_upvalues_from(base as usize + from as usize);
  OK
}

// ---------------------------------------------------------------------
// Collections
// ---------------------------------------------------------------------

pub unsafe extern "C" fn zuri_jit_make_list(
  vm_ptr: *mut VM,
  base: u64,
  dst: u64,
  start: u64,
  count: u64,
) -> u64 {
  let vm = unsafe { vm(vm_ptr) };
  let base = base as usize;
  // See the interpreter's own `Instr::MakeList` handler for why this
  // collects directly into `ListStorage` rather than a `Vec` first.
  let items: ListStorage = (0..count as u8)
    .map(|i| vm.get_reg(base, start as u8 + i))
    .collect();
  let list_val = vm.heap.alloc_list(items);
  vm.set_reg(base, dst as u8, list_val);
  OK
}

pub unsafe extern "C" fn zuri_jit_make_dict(
  vm_ptr: *mut VM,
  base: u64,
  dst: u64,
  start: u64,
  count: u64,
) -> u64 {
  let vm = unsafe { vm(vm_ptr) };
  let base = base as usize;
  let start = start as u8;
  let count = count as u8;
  let pairs: Vec<(Value, Value)> = (0..count)
    .map(|i| {
      (
        vm.get_reg(base, start + i),
        vm.get_reg(base, start + count + i),
      )
    })
    .collect();
  let dict_val = vm.heap.alloc_dict(pairs);
  vm.set_reg(base, dst as u8, dict_val);
  OK
}

pub unsafe extern "C" fn zuri_jit_make_range(
  vm_ptr: *mut VM,
  base: u64,
  dst: u64,
  lower: u64,
  upper: u64,
) -> u64 {
  let vm = unsafe { vm(vm_ptr) };
  let base = base as usize;
  let lo = vm.get_reg(base, lower as u8);
  let hi = vm.get_reg(base, upper as u8);
  if !lo.is_number() || !hi.is_number() {
    let msg = format!(
      "range bounds must be numbers, got {} and {}",
      lo.type_name(),
      hi.type_name()
    );
    let e = vm.raise("TypeError", msg);
    return fail(vm, e);
  }
  let range_val = vm.heap.alloc_range(lo.as_number(), hi.as_number());
  vm.set_reg(base, dst as u8, range_val);
  OK
}

// ---------------------------------------------------------------------
// Indexing
// ---------------------------------------------------------------------

/// Registers a scalar-replaced `Instr::MakeList` allocation's backing
/// stack memory as a GC root -- see `VM::push_scalar_root`'s own docs
/// for the full mechanism, and `jit::codegen::FuncCompiler::
/// emit_scalar_make_list` for the one call site (always the very last
/// step there, after every element slot has already been populated).
pub unsafe extern "C" fn zuri_jit_push_scalar_root(
  vm_ptr: *mut VM,
  data_ptr: u64,
  count: u64,
) -> u64 {
  let vm = unsafe { vm(vm_ptr) };
  vm.push_scalar_root(data_ptr as *mut Value, count as usize);
  OK
}

/// `Instr::GetIndex`'s slow-path fallback for a scalar-replaced list
/// (`jit::codegen::FuncCompiler::emit_scalar_list_get`'s own docs) --
/// everything the inline fast path didn't prove safe (a non-numeric or
/// non-integer index, or a genuinely out-of-bounds one) still needs the
/// real error-raising logic, but there is no heap `Obj::List` to hand
/// `VM::index_get`: `VM::coerce_index` is the one piece of that logic
/// that operates on a bare `len: usize` instead of a real container,
/// exactly what's needed here.
pub unsafe extern "C" fn zuri_jit_scalar_get_index(
  vm_ptr: *mut VM,
  base: u64,
  dst: u64,
  data_ptr: u64,
  count: u64,
  idx_reg: u64,
) -> u64 {
  let vm = unsafe { vm(vm_ptr) };
  let base = base as usize;
  let idx_val = vm.get_reg(base, idx_reg as u8);
  match vm.coerce_index(idx_val, count as usize) {
    Ok(i) => {
      // SAFETY: `data_ptr`/`count` describe the same live, currently-
      // registered `jit_scalar_roots` entry the fast path itself would
      // have read from -- see `emit_scalar_make_list`'s own docs.
      let slice = unsafe { std::slice::from_raw_parts(data_ptr as *const Value, count as usize) };
      vm.set_reg(base, dst as u8, slice[i]);
      OK
    },
    Err(e) => fail(vm, e),
  }
}

/// `Instr::SetIndex`'s slow-path fallback for a scalar-replaced list --
/// the write-side counterpart of `zuri_jit_scalar_get_index`, see its
/// own docs.
pub unsafe extern "C" fn zuri_jit_scalar_set_index(
  vm_ptr: *mut VM,
  base: u64,
  data_ptr: u64,
  count: u64,
  idx_reg: u64,
  src_reg: u64,
) -> u64 {
  let vm = unsafe { vm(vm_ptr) };
  let base = base as usize;
  let idx_val = vm.get_reg(base, idx_reg as u8);
  match vm.coerce_index(idx_val, count as usize) {
    Ok(i) => {
      let src_val = vm.get_reg(base, src_reg as u8);
      // SAFETY: see `zuri_jit_scalar_get_index`'s identical reasoning.
      let slice = unsafe { std::slice::from_raw_parts_mut(data_ptr as *mut Value, count as usize) };
      slice[i] = src_val;
      OK
    },
    Err(e) => fail(vm, e),
  }
}

pub unsafe extern "C" fn zuri_jit_get_index(
  vm_ptr: *mut VM,
  base: u64,
  dst: u64,
  obj: u64,
  idx: u64,
) -> u64 {
  let vm = unsafe { vm(vm_ptr) };
  let base = base as usize;
  let ov = vm.get_reg(base, obj as u8);
  let iv = vm.get_reg(base, idx as u8);
  match vm.index_get(ov, iv) {
    Ok(result) => {
      vm.set_reg(base, dst as u8, result);
      OK
    },
    Err(e) => fail(vm, e),
  }
}

pub unsafe extern "C" fn zuri_jit_set_index(
  vm_ptr: *mut VM,
  base: u64,
  obj: u64,
  idx: u64,
  src: u64,
) -> u64 {
  let vm = unsafe { vm(vm_ptr) };
  let base = base as usize;
  let ov = vm.get_reg(base, obj as u8);
  let iv = vm.get_reg(base, idx as u8);
  let sv = vm.get_reg(base, src as u8);
  match vm.index_set(ov, iv, sv) {
    Ok(()) => OK,
    Err(e) => fail(vm, e),
  }
}

pub unsafe extern "C" fn zuri_jit_get_slice(
  vm_ptr: *mut VM,
  base: u64,
  dst: u64,
  obj: u64,
  lo: u64,
  hi: u64,
) -> u64 {
  let vm = unsafe { vm(vm_ptr) };
  let base = base as usize;
  let ov = vm.get_reg(base, obj as u8);
  let lov = vm.get_reg(base, lo as u8);
  let hiv = vm.get_reg(base, hi as u8);
  match vm.index_slice(ov, lov, hiv) {
    Ok(result) => {
      vm.set_reg(base, dst as u8, result);
      OK
    },
    Err(e) => fail(vm, e),
  }
}

// ---------------------------------------------------------------------
// Classes -- mirrors vm.rs's MakeClass/DeclareField/SetFieldInit/
// SetMethod/DeclareStatic/FinalizeClass/GetField/SetField handlers.
// Class *declaration* opcodes are cold by nature (a class body runs
// once), so these prioritize exactness over speed.
// ---------------------------------------------------------------------

pub unsafe extern "C" fn zuri_jit_make_class(
  vm_ptr: *mut VM,
  base: u64,
  dst: u64,
  name_bits: u64,
  has_super: u64,
  super_reg: u64,
) -> u64 {
  let vm = unsafe { vm(vm_ptr) };
  let base = base as usize;
  let name = Value::from_bits(name_bits).as_str().to_string();

  let superclass_val = if has_super != 0 {
    let v = vm.get_reg(base, super_reg as u8);
    if !v.is_class() {
      let msg = format!(
        "superclass of '{}' is not a class (got a {})",
        name,
        v.type_name()
      );
      let e = vm.raise("TypeError", msg);
      return fail(vm, e);
    }
    Some(v)
  } else {
    None
  };

  let (methods, field_slots, field_count, constructor) = match superclass_val {
    Some(sup) => {
      let s = sup.as_class();
      (
        s.methods.clone(),
        s.field_slots.clone(),
        s.field_count,
        s.constructor,
      )
    },
    None => (Default::default(), Default::default(), 0, None),
  };

  let class_val = vm.heap.alloc_class(crate::vm::object::ObjClass {
    name,
    superclass: superclass_val,
    methods,
    field_slots,
    field_count,
    own_field_initializer: None,
    constructor,
    static_slots: Default::default(),
    statics: Vec::new(),
  });
  vm.set_reg(base, dst as u8, class_val);
  OK
}

pub unsafe extern "C" fn zuri_jit_declare_field(
  vm_ptr: *mut VM,
  base: u64,
  class: u64,
  name_bits: u64,
) -> u64 {
  let vm = unsafe { vm(vm_ptr) };
  let class_val = vm.get_reg(base as usize, class as u8);
  let name = Value::from_bits(name_bits).as_str().to_string();
  let mut c = class_val.as_class_mut();
  if !c.field_slots.contains_key(&name) {
    let idx = c.field_count;
    c.field_slots.insert(name, idx);
    c.field_count += 1;
  }
  OK
}

pub unsafe extern "C" fn zuri_jit_set_field_init(
  vm_ptr: *mut VM,
  base: u64,
  class: u64,
  src: u64,
) -> u64 {
  let vm = unsafe { vm(vm_ptr) };
  let base = base as usize;
  let class_val = vm.get_reg(base, class as u8);
  let init = vm.get_reg(base, src as u8);
  class_val.as_class_mut().own_field_initializer = Some(init);
  write_barrier(class_val.as_obj());
  OK
}

pub unsafe extern "C" fn zuri_jit_set_method(
  vm_ptr: *mut VM,
  base: u64,
  class: u64,
  name_bits: u64,
  src: u64,
) -> u64 {
  let vm = unsafe { vm(vm_ptr) };
  let base = base as usize;
  let class_val = vm.get_reg(base, class as u8);
  let name = Value::from_bits(name_bits).as_str().to_string();
  let method = vm.get_reg(base, src as u8);
  class_val.as_class_mut().methods.insert(name, method);
  write_barrier(class_val.as_obj());
  vm.bump_method_table_generation();
  OK
}

pub unsafe extern "C" fn zuri_jit_declare_static(
  vm_ptr: *mut VM,
  base: u64,
  class: u64,
  name_bits: u64,
  src: u64,
) -> u64 {
  let vm = unsafe { vm(vm_ptr) };
  let base = base as usize;
  let class_val = vm.get_reg(base, class as u8);
  let name = Value::from_bits(name_bits).as_str().to_string();
  let value = vm.get_reg(base, src as u8);
  let mut c = class_val.as_class_mut();
  let idx = c.statics.len() as u16;
  c.static_slots.insert(name, idx);
  c.statics.push(Cell::new(value));
  drop(c);
  write_barrier(class_val.as_obj());
  OK
}

/// `func_ptr_bits` is the currently-compiling function itself (needed
/// for `globals_module`, to check "class already declared in this
/// scope" exactly like the interpreter does) -- see this module's docs
/// on baked function pointers.
pub unsafe extern "C" fn zuri_jit_finalize_class(
  vm_ptr: *mut VM,
  base: u64,
  class: u64,
  func_ptr_bits: u64,
) -> u64 {
  let vm = unsafe { vm(vm_ptr) };
  let base = base as usize;
  let class_val = vm.get_reg(base, class as u8);
  let func = unsafe { &*(func_ptr_bits as *const ObjFunction) };
  // Matches vm.rs's own `Instr::FinalizeClass` handler: the class's own
  // name is always compiled as constant index 0 of its declaring
  // function's chunk.
  let name = func.chunk.constants[0].as_str().to_string();

  let mut c = class_val.as_class_mut();
  if vm.lookup_slot_in(func.globals_module, &c.name).is_some() {
    let msg = format!("class '{}' already declared in this scope", c.name);
    drop(c);
    let e = vm.raise("Error", msg);
    return fail(vm, e);
  }
  if let Some(ctor) = c.methods.get(&name).copied() {
    c.constructor = Some(ctor);
    drop(c);
    write_barrier(class_val.as_obj());
  }
  OK
}

pub unsafe extern "C" fn zuri_jit_get_field(
  vm_ptr: *mut VM,
  base: u64,
  dst: u64,
  obj: u64,
  name_bits: u64,
  func_ptr_bits: u64,
  instr_ip: u64,
) -> u64 {
  let vm = unsafe { vm(vm_ptr) };
  let base = base as usize;
  let receiver = vm.get_reg(base, obj as u8);
  let name_val = Value::from_bits(name_bits);

  let result: Result<Value, Value> = if receiver.is_instance() {
    let inst = receiver.as_instance();
    let class_bits = inst.class.to_bits();
    let func = unsafe { &*(func_ptr_bits as *const ObjFunction) };
    let instr_ip = instr_ip as usize;

    // Inline cache: same receiver class as the last time this exact
    // `Instr::GetField` ran -> reuse its resolved slot directly,
    // skipping `ObjClass::field_slots`'s hash-map probe entirely. See
    // `Chunk::field_cache`'s own docs.
    let cell = func.chunk.field_cache_cell(instr_ip);
    let hit = cell.is_some_and(|c| c.class_bits.get() == class_bits);

    if hit {
      let idx = (cell.unwrap().byte_offset.get() as usize) / size_of::<Value>();
      Ok(inst.fields[idx].get())
    } else {
      let class = inst.class.as_class();
      if let Some(&idx) = class.field_slots.get(name_val.as_str()) {
        if let Some(c) = cell {
          c.byte_offset.set(idx as u64 * size_of::<Value>() as u64);
          c.class_bits.set(class_bits);
        }
        Ok(inst.fields[idx as usize].get())
      } else if let Some(method) = class.methods.get(name_val.as_str()).copied() {
        Ok(vm.heap.alloc_bound_method(receiver, method))
      } else {
        let msg = format!(
          "undefined property '{}' on instance of '{}'",
          name_val.as_str(),
          class.name
        );
        Err(vm.raise("PropertyError", msg))
      }
    }
  } else if receiver.is_class() {
    match crate::vm::vm::lookup_static(receiver, name_val.as_str()) {
      Some(raw) => {
        if raw.is_closure() && raw.as_closure().function.as_func().is_method {
          Ok(vm.heap.alloc_bound_method(Value::nil(), raw))
        } else {
          Ok(raw)
        }
      },
      None => {
        let msg = format!(
          "undefined static member '{}' on class '{}'",
          name_val.as_str(),
          receiver.as_class().name
        );
        Err(vm.raise("PropertyError", msg))
      },
    }
  } else if receiver.is_module() {
    let m = receiver.as_module();
    match m.namespace.get(name_val.as_str()) {
      Some(v) => Ok(v),
      None => {
        let msg = format!("module '{}' has no member '{}'", m.name, name_val.as_str());
        drop(m);
        Err(vm.raise("PropertyError", msg))
      },
    }
  } else if receiver.is_module_binding() {
    let module_val = receiver.as_module_binding().module;
    let m = module_val.as_module();
    match m.namespace.get(name_val.as_str()) {
      Some(v) => Ok(v),
      None => {
        let msg = format!("module '{}' has no member '{}'", m.name, name_val.as_str());
        drop(m);
        Err(vm.raise("PropertyError", msg))
      },
    }
  } else if receiver.is_dict() {
    match receiver.dict_get(&name_val) {
      Some(v) => Ok(v),
      None => {
        let msg = format!("undefined key '{}' in dict", name_val);
        Err(vm.raise("PropertyError", msg))
      },
    }
  } else {
    let msg = format!(
      "cannot read property '{}' on a {}",
      name_val.as_str(),
      receiver.type_name()
    );
    Err(vm.raise("TypeError", msg))
  };

  match result {
    Ok(v) => {
      vm.set_reg(base, dst as u8, v);
      OK
    },
    Err(e) => fail(vm, e),
  }
}

pub unsafe extern "C" fn zuri_jit_set_field(
  vm_ptr: *mut VM,
  base: u64,
  obj: u64,
  name_bits: u64,
  src: u64,
  func_ptr_bits: u64,
  instr_ip: u64,
) -> u64 {
  let vm = unsafe { vm(vm_ptr) };
  let base = base as usize;
  let receiver = vm.get_reg(base, obj as u8);
  let value = vm.get_reg(base, src as u8);
  let name_val = Value::from_bits(name_bits);

  let result: Result<(), Value> = if receiver.is_instance() {
    let inst = receiver.as_instance();
    let class_bits = inst.class.to_bits();
    let func = unsafe { &*(func_ptr_bits as *const ObjFunction) };
    let instr_ip = instr_ip as usize;

    // Same inline-cache shape as `zuri_jit_get_field` -- see
    // `Chunk::field_cache`'s docs.
    let cell = func.chunk.field_cache_cell(instr_ip);
    let hit = cell.is_some_and(|c| c.class_bits.get() == class_bits);

    if hit {
      let idx = (cell.unwrap().byte_offset.get() as usize) / size_of::<Value>();
      inst.fields[idx].set(value);
      write_barrier(receiver.as_obj());
      Ok(())
    } else {
      let class = inst.class.as_class();
      match class.field_slots.get(name_val.as_str()).copied() {
        Some(idx) => {
          if let Some(c) = cell {
            c.byte_offset.set(idx as u64 * size_of::<Value>() as u64);
            c.class_bits.set(class_bits);
          }
          inst.fields[idx as usize].set(value);
          write_barrier(receiver.as_obj());
          Ok(())
        },
        None => {
          let msg = format!(
            "undefined field '{}' on instance of '{}'",
            name_val.as_str(),
            class.name
          );
          Err(vm.raise("PropertyError", msg))
        },
      }
    }
  } else if receiver.is_class() {
    crate::vm::vm::set_static(receiver, name_val.as_str(), value)
      .map_err(|msg| vm.raise("PropertyError", msg))
  } else if receiver.is_module() || receiver.is_module_binding() {
    Err(vm.raise(
      "AccessError",
      "cannot assign to a module member from outside the module",
    ))
  } else if receiver.is_dict() {
    receiver.dict_set(name_val, value);
    Ok(())
  } else {
    let msg = format!(
      "cannot set property '{}' on a {}",
      name_val.as_str(),
      receiver.type_name()
    );
    Err(vm.raise("TypeError", msg))
  };

  match result {
    Ok(()) => OK,
    Err(e) => fail(vm, e),
  }
}

/// `Instr::CheckParamType`'s full-generality fallback -- every case
/// `jit::codegen::emit_check_param_type` doesn't (or can't safely)
/// inline: a union with more than one member, `Instance` (needs a
/// possibly-failing global lookup), and `Iterable` (needs a method-
/// table probe). Shares `VM::param_type_matches` with the interpreter,
/// so the two can never disagree on what a given type name accepts.
pub unsafe extern "C" fn zuri_jit_check_param_type(
  vm_ptr: *mut VM,
  base: u64,
  reg: u64,
  func_ptr_bits: u64,
  check_idx: u64,
  instr_ip: u64,
) -> u64 {
  let vm = unsafe { vm(vm_ptr) };
  let base = base as usize;
  let v = vm.get_reg(base, reg as u8);
  let func = unsafe { &*(func_ptr_bits as *const ObjFunction) };
  let check = &func.chunk.param_checks[check_idx as usize];

  if check.nullable && v.is_nil() {
    return OK;
  }

  for &t in &check.types {
    match vm.param_type_matches(v, t, func, instr_ip as usize) {
      Ok(true) => return OK,
      Ok(false) => {},
      Err(e) => return fail(vm, e),
    }
  }

  let check = &func.chunk.param_checks[check_idx as usize];
  let msg = format!(
    "{}() expects parameter '{}' (argument {}) to be {}, got {}",
    func.name,
    check.param_name,
    check.position,
    crate::vm::chunk::describe_param_types(&check.types, &func.chunk),
    v.type_name(),
  );
  let e = vm.raise("TypeError", msg);
  fail(vm, e)
}

// ---------------------------------------------------------------------
// `using` jump table
// ---------------------------------------------------------------------

/// Sentinel meaning "no constant-table hit; fall through to the
/// sequential dynamic-label path" -- `ip`s are always small, so
/// `u64::MAX` is unambiguous.
pub const USING_NO_MATCH: u64 = u64::MAX;

pub unsafe extern "C" fn zuri_jit_using_jump(
  vm_ptr: *mut VM,
  base: u64,
  subject: u64,
  func_ptr_bits: u64,
  table_idx: u64,
) -> u64 {
  let vm = unsafe { vm(vm_ptr) };
  let v = vm.get_reg(base as usize, subject as u8);
  let Some(key) = value_to_jump_key_for_jit(v) else {
    return USING_NO_MATCH;
  };
  let func = unsafe { &*(func_ptr_bits as *const ObjFunction) };
  match func.chunk.jump_tables[table_idx as usize].get(&key) {
    Some(&target) => target as u64,
    None => USING_NO_MATCH,
  }
}

fn value_to_jump_key_for_jit(v: Value) -> Option<JumpKey> {
  if v.is_nil() {
    Some(JumpKey::Nil)
  } else if v.is_bool() {
    Some(JumpKey::Bool(v.as_bool()))
  } else if v.is_number() {
    Some(JumpKey::Number(v.as_number().to_bits()))
  } else if v.is_string() {
    Some(JumpKey::Str(v.as_str().to_string()))
  } else {
    None
  }
}

// ---------------------------------------------------------------------
// Modules
// ---------------------------------------------------------------------

pub unsafe extern "C" fn zuri_jit_import(
  vm_ptr: *mut VM,
  base: u64,
  dst: u64,
  path_bits: u64,
  importer_bits: u64,
) -> u64 {
  let vm = unsafe { vm(vm_ptr) };
  let base = base as usize;
  let path = Value::from_bits(path_bits).as_str().to_string();
  let importer = Value::from_bits(importer_bits).as_str().to_string();
  match crate::vm::modules::import(vm, &importer, &path) {
    Ok(module_val) => {
      vm.set_reg(base, dst as u8, module_val);
      OK
    },
    Err(e) => fail(vm, e),
  }
}

pub unsafe extern "C" fn zuri_jit_import_all(
  vm_ptr: *mut VM,
  base: u64,
  module_reg: u64,
  func_ptr_bits: u64,
) -> u64 {
  let vm = unsafe { vm(vm_ptr) };
  let base = base as usize;
  let mv = vm.get_reg(base, module_reg as u8);
  if !mv.is_module() {
    let e = vm.raise("TypeError", "expected a module for 'import ... { * }'");
    return fail(vm, e);
  }
  let entries: Vec<(String, Value)> = {
    let m = mv.as_module();
    m.namespace
      .names
      .iter()
      .map(|(k, &idx)| (k.clone(), m.namespace.slots[idx as usize].get()))
      .collect()
  };
  let func = unsafe { &*(func_ptr_bits as *const ObjFunction) };
  let target = func.globals_module;
  for (name, val) in entries {
    let slot = vm.get_or_create_slot_in(target, name);
    vm.write_slot_in(target, slot, val);
  }
  OK
}

pub unsafe extern "C" fn zuri_jit_make_promoted(
  vm_ptr: *mut VM,
  base: u64,
  dst: u64,
  module_reg: u64,
  name_bits: u64,
) -> u64 {
  let vm = unsafe { vm(vm_ptr) };
  let base = base as usize;
  let mv = vm.get_reg(base, module_reg as u8);
  let name_val = Value::from_bits(name_bits);
  if !mv.is_module() {
    let e = vm.raise("TypeError", "invalid module promotion");
    return fail(vm, e);
  }
  let promoted = {
    let m = mv.as_module();
    m.namespace
      .get(name_val.as_str())
      .filter(|v| v.is_callable())
  };
  let binding = vm
    .heap
    .alloc_module_binding(crate::vm::object::ObjModuleBinding {
      module: mv,
      promoted,
      bind_name: name_val.as_str().to_string(),
    });
  vm.set_reg(base, dst as u8, binding);
  OK
}

// ---------------------------------------------------------------------
// Number-method intrinsics
//
// One helper per pure `builtins::number` method whose result is a
// number and whose implementation is a single `f64` operation, reached
// by `jit::codegen::FuncCompiler::emit_number_intrinsic` as a direct
// call on a receiver already guarded numeric -- skipping
// `zuri_jit_invoke_prepare`'s resolution, `builtins::lookup`'s string
// hash and `memcmp`, `invoke_native_args`'s per-call `Vec`, and
// `VM::call_native` entirely.
//
// These are not reimplementations. Each one calls the exact same
// `f64` method its `builtins::number` counterpart does, so a compiled
// `x.sin()` is bit-identical to the interpreted one by construction --
// there is deliberately no hand-rolled polynomial approximation
// anywhere here, which would be faster but would make the two tiers
// disagree.
//
// Takes and returns raw bits rather than `Value`s so generated code
// can hand over a register it already holds. Touches no VM state,
// cannot allocate, collect, or raise -- which is what lets `codegen`
// reach them through `call_helper_raw`, with no register-cache
// invalidation around the call.
// ---------------------------------------------------------------------

macro_rules! num_intrinsic {
  ($name:ident, $f:ident) => {
    pub unsafe extern "C" fn $name(_vm_ptr: *mut VM, bits: u64) -> u64 {
      Value::number(f64::from_bits(bits).$f()).to_bits()
    }
  };
}

macro_rules! num_intrinsic2 {
  ($name:ident, $f:ident) => {
    pub unsafe extern "C" fn $name(_vm_ptr: *mut VM, a: u64, b: u64) -> u64 {
      Value::number(f64::from_bits(a).$f(f64::from_bits(b))).to_bits()
    }
  };
}

num_intrinsic!(zuri_jit_num_sin, sin);
num_intrinsic!(zuri_jit_num_cos, cos);
num_intrinsic!(zuri_jit_num_tan, tan);
num_intrinsic!(zuri_jit_num_sinh, sinh);
num_intrinsic!(zuri_jit_num_cosh, cosh);
num_intrinsic!(zuri_jit_num_tanh, tanh);
num_intrinsic!(zuri_jit_num_asin, asin);
num_intrinsic!(zuri_jit_num_acos, acos);
num_intrinsic!(zuri_jit_num_atan, atan);
num_intrinsic!(zuri_jit_num_asinh, asinh);
num_intrinsic!(zuri_jit_num_acosh, acosh);
num_intrinsic!(zuri_jit_num_atanh, atanh);
num_intrinsic!(zuri_jit_num_exp, exp);
num_intrinsic!(zuri_jit_num_expm1, exp_m1);
num_intrinsic!(zuri_jit_num_log, ln);
num_intrinsic!(zuri_jit_num_log2, log2);
num_intrinsic!(zuri_jit_num_log10, log10);
num_intrinsic!(zuri_jit_num_log1p, ln_1p);
num_intrinsic!(zuri_jit_num_cbrt, cbrt);
// `f64::round` breaks ties away from zero; Cranelift's `nearest`
// instruction is IEEE round-half-to-even. They're different
// functions, so `round` is a direct call to the real one rather than
// an inlined instruction -- see `NumberIntrinsic`'s own docs.
num_intrinsic!(zuri_jit_num_round, round);

/// `x % y` and `x ** y` behind the same direct-call contract as the
/// unary intrinsics above -- neither has a Cranelift instruction (a
/// float remainder is a libcall, and there is no pow opcode at all), so
/// the win here is not the arithmetic but everything the ordinary
/// `Instr::Mod`/`Instr::Pow` helper does around it: two register reads,
/// a register write, the numeric/bigint/operator-override dispatch, and
/// `call_checked`'s full register flush and stale-mark.
///
/// Same `f64` operations `VM::binary_numeric` is handed for these, so
/// compiled and interpreted results are bit-identical by construction.
pub unsafe extern "C" fn zuri_jit_num_fmod(_vm_ptr: *mut VM, a: u64, b: u64) -> u64 {
  Value::number(crate::vm::value::num_rem(
    f64::from_bits(a),
    f64::from_bits(b),
  ))
  .to_bits()
}

pub unsafe extern "C" fn zuri_jit_num_powf(_vm_ptr: *mut VM, a: u64, b: u64) -> u64 {
  Value::number(f64::from_bits(a).powf(f64::from_bits(b))).to_bits()
}

num_intrinsic2!(zuri_jit_num_max, max);
num_intrinsic2!(zuri_jit_num_min, min);
num_intrinsic2!(zuri_jit_num_atan2, atan2);

/// Calls a builtin native whose identity generated code has already
/// established (see `CallTarget::KnownNative`), skipping every step of
/// the ordinary path that only exists to work out what the callee is:
/// `zuri_jit_call_prepare`'s closure check (which can only ever fail
/// for a native), `zuri_jit_call`'s re-read of the callee register, and
/// `dispatch_call_sync`'s type dispatch.
///
/// What remains is genuinely per-call: gathering the arguments out of
/// the register window and `VM::call_native`'s own arity check and
/// invocation.
pub unsafe extern "C" fn zuri_jit_call_native(
  vm_ptr: *mut VM,
  base: u64,
  func_reg: u64,
  num_args: u64,
  dst: u64,
) -> u64 {
  let vm = unsafe { vm(vm_ptr) };
  let base = base as usize;
  let func_reg = func_reg as u8;
  // Read out of the register rather than from a baked pointer.
  // `Heap::alloc_native` leaves natives in the nursery, so the object
  // moves when a minor collection promotes it -- an address baked at
  // compile time would dangle. Generated code has already guarded that
  // this register holds the very native this call site resolved (on its
  // `NativeFn` pointer, which relocation cannot change), so this read is
  // the identity check's conclusion, not a re-resolution.
  let callee = vm.get_reg(base, func_reg);
  let native = callee.as_native();

  let mut args = CallArgs::new();
  for i in 0..num_args as u8 {
    args.push(vm.get_reg(base, func_reg + 1 + i));
  }

  match vm.call_native(native, args.as_slice()) {
    Ok(v) => {
      vm.set_reg(base, dst as u8, v);
      OK
    },
    Err(e) => fail(vm, e),
  }
}

/// `object::write_barrier` behind the C ABI, for the rare arm of
/// `jit::codegen`'s inlined barrier check -- generated code has already
/// established (with two byte loads and a branch, no call) that this
/// object really is old and not yet remembered, so reaching here means
/// the remembered-set push is genuinely owed. Takes the raw `*const
/// Obj` directly rather than a register index: the caller already has
/// the untagged pointer in hand from its own field write.
///
/// Touches no VM register and cannot allocate, collect, or raise, which
/// is what lets `codegen` reach it through `call_helper_raw` (no
/// flush/stale bracketing) instead of the register-cache-invalidating
/// `call_helper`. Always returns `OK`; it has no failure mode.
pub unsafe extern "C" fn zuri_jit_write_barrier(_vm_ptr: *mut VM, obj_ptr: u64) -> u64 {
  crate::vm::object::write_barrier(obj_ptr as *const crate::vm::object::Obj);
  OK
}

// ---------------------------------------------------------------------
// Symbol table -- the single source of truth `jit::engine::JitEngine`
// walks to both link every helper above into the JIT (`JITBuilder::
// symbol`) and declare each one's Cranelift signature (a flat N x I64
// -> I64 C ABI throughout, regardless of what each individual
// parameter actually semantically holds -- a register index, a baked
// constant's bit pattern, a stable heap pointer, ...). Adding a new
// helper means adding exactly one line here; nothing else needs to
// know its address.
// ---------------------------------------------------------------------

pub struct HelperSpec {
  pub name: &'static str,
  pub ptr: *const u8,
  /// Parameter count including the leading `vm` pointer -- matches
  /// `jit::engine`'s declared signature (N x I64 params, 1 I64 return)
  /// exactly.
  pub arity: usize,
}

type Fn1 = unsafe extern "C" fn(*mut VM) -> u64;
type Fn2 = unsafe extern "C" fn(*mut VM, u64) -> u64;
type Fn3 = unsafe extern "C" fn(*mut VM, u64, u64) -> u64;
type Fn4 = unsafe extern "C" fn(*mut VM, u64, u64, u64) -> u64;
type Fn5 = unsafe extern "C" fn(*mut VM, u64, u64, u64, u64) -> u64;
type Fn6 = unsafe extern "C" fn(*mut VM, u64, u64, u64, u64, u64) -> u64;
type Fn7 = unsafe extern "C" fn(*mut VM, u64, u64, u64, u64, u64, u64) -> u64;
type Fn9 = unsafe extern "C" fn(*mut VM, u64, u64, u64, u64, u64, u64, u64, u64) -> u64;

/// Reinterprets an already-coerced, concrete function-pointer value
/// (one of the `FnN` aliases above -- not a bare function item, which
/// is a distinct zero-sized type until coerced) as a raw code address.
/// Safe because every `FnN` alias is, by construction, an ordinary
/// pointer-sized function pointer -- the `debug_assert!` documents that
/// invariant rather than substituting for it.
fn as_ptr<F: Copy>(f: F) -> *const u8 {
  debug_assert_eq!(std::mem::size_of::<F>(), std::mem::size_of::<*const u8>());
  // SAFETY: `F` is always one of the `FnN` aliases above at every call
  // site (see the `spec*!` macros below), each a plain, pointer-sized
  // `unsafe extern "C" fn`.
  unsafe { std::mem::transmute_copy::<F, *const u8>(&f) }
}

macro_rules! spec1 {
  ($f:ident) => {
    HelperSpec {
      name: stringify!($f),
      ptr: as_ptr($f as Fn1),
      arity: 1,
    }
  };
}
macro_rules! spec2 {
  ($f:ident) => {
    HelperSpec {
      name: stringify!($f),
      ptr: as_ptr($f as Fn2),
      arity: 2,
    }
  };
}
macro_rules! spec3 {
  ($f:ident) => {
    HelperSpec {
      name: stringify!($f),
      ptr: as_ptr($f as Fn3),
      arity: 3,
    }
  };
}
macro_rules! spec4 {
  ($f:ident) => {
    HelperSpec {
      name: stringify!($f),
      ptr: as_ptr($f as Fn4),
      arity: 4,
    }
  };
}
macro_rules! spec5 {
  ($f:ident) => {
    HelperSpec {
      name: stringify!($f),
      ptr: as_ptr($f as Fn5),
      arity: 5,
    }
  };
}
macro_rules! spec6 {
  ($f:ident) => {
    HelperSpec {
      name: stringify!($f),
      ptr: as_ptr($f as Fn6),
      arity: 6,
    }
  };
}
macro_rules! spec7 {
  ($f:ident) => {
    HelperSpec {
      name: stringify!($f),
      ptr: as_ptr($f as Fn7),
      arity: 7,
    }
  };
}
macro_rules! spec9 {
  ($f:ident) => {
    HelperSpec {
      name: stringify!($f),
      ptr: as_ptr($f as Fn9),
      arity: 9,
    }
  };
}

pub fn helper_table() -> Vec<HelperSpec> {
  vec![
    spec1!(zuri_jit_gc_safepoint),
    spec2!(zuri_jit_deopt),
    spec3!(zuri_jit_is_falsey),
    spec3!(zuri_jit_print),
    spec3!(zuri_jit_close_upvalues),
    spec4!(zuri_jit_bitnot_slow),
    spec4!(zuri_jit_neg_slow),
    spec4!(zuri_jit_declare_field),
    spec4!(zuri_jit_set_field_init),
    spec4!(zuri_jit_finalize_class),
    spec4!(zuri_jit_import_all),
    spec5!(zuri_jit_sub_slow),
    spec5!(zuri_jit_div_slow),
    spec5!(zuri_jit_pow),
    spec5!(zuri_jit_mod),
    spec5!(zuri_jit_floordiv),
    spec5!(zuri_jit_bitand_slow),
    spec5!(zuri_jit_bitor_slow),
    spec5!(zuri_jit_bitxor_slow),
    spec5!(zuri_jit_bitshl),
    spec5!(zuri_jit_bitshr),
    spec5!(zuri_jit_bitushr),
    spec5!(zuri_jit_lt_slow),
    spec5!(zuri_jit_le_slow),
    spec5!(zuri_jit_gt_slow),
    spec5!(zuri_jit_ge_slow),
    spec5!(zuri_jit_add_slow),
    spec5!(zuri_jit_mul_slow),
    spec5!(zuri_jit_concat),
    spec5!(zuri_jit_eq_slow),
    spec5!(zuri_jit_neq_slow),
    spec5!(zuri_jit_subimm_slow),
    spec5!(zuri_jit_addimm_slow),
    spec5!(zuri_jit_mulimm_slow),
    spec5!(zuri_jit_ltimm_slow),
    spec5!(zuri_jit_leimm_slow),
    spec5!(zuri_jit_gtimm_slow),
    spec5!(zuri_jit_geimm_slow),
    spec5!(zuri_jit_call),
    spec6!(zuri_jit_call_prepare),
    spec6!(zuri_jit_new_prepare),
    spec9!(zuri_jit_construct_prepare),
    spec4!(zuri_jit_alloc_and_pin_instance),
    spec1!(zuri_jit_take_constructed_instance),
    spec4!(zuri_jit_new_finish),
    spec9!(zuri_jit_invoke_prepare),
    spec5!(zuri_jit_direct_call_prepare),
    spec5!(zuri_jit_call_finish),
    spec3!(zuri_jit_finish_deopt),
    spec5!(zuri_jit_call_super_ctor),
    spec5!(zuri_jit_make_closure),
    spec2!(zuri_jit_closure_upvalues_ptr),
    spec5!(zuri_jit_get_upval),
    spec5!(zuri_jit_set_upval),
    spec5!(zuri_jit_make_list),
    spec5!(zuri_jit_make_dict),
    spec5!(zuri_jit_make_range),
    spec5!(zuri_jit_get_index),
    spec5!(zuri_jit_set_index),
    spec3!(zuri_jit_push_scalar_root),
    spec6!(zuri_jit_scalar_get_index),
    spec6!(zuri_jit_scalar_set_index),
    spec5!(zuri_jit_set_method),
    spec5!(zuri_jit_declare_static),
    spec7!(zuri_jit_get_field),
    spec7!(zuri_jit_set_field),
    spec6!(zuri_jit_check_param_type),
    spec2!(zuri_jit_write_barrier),
    spec5!(zuri_jit_call_native),
    spec2!(zuri_jit_num_sin),
    spec2!(zuri_jit_num_cos),
    spec2!(zuri_jit_num_tan),
    spec2!(zuri_jit_num_sinh),
    spec2!(zuri_jit_num_cosh),
    spec2!(zuri_jit_num_tanh),
    spec2!(zuri_jit_num_asin),
    spec2!(zuri_jit_num_acos),
    spec2!(zuri_jit_num_atan),
    spec2!(zuri_jit_num_asinh),
    spec2!(zuri_jit_num_acosh),
    spec2!(zuri_jit_num_atanh),
    spec2!(zuri_jit_num_exp),
    spec2!(zuri_jit_num_expm1),
    spec2!(zuri_jit_num_log),
    spec2!(zuri_jit_num_log2),
    spec2!(zuri_jit_num_log10),
    spec2!(zuri_jit_num_log1p),
    spec2!(zuri_jit_num_cbrt),
    spec2!(zuri_jit_num_round),
    spec3!(zuri_jit_num_max),
    spec3!(zuri_jit_num_fmod),
    spec3!(zuri_jit_num_powf),
    spec3!(zuri_jit_num_min),
    spec3!(zuri_jit_num_atan2),
    spec5!(zuri_jit_using_jump),
    spec5!(zuri_jit_import),
    spec5!(zuri_jit_make_promoted),
    spec7!(zuri_jit_invoke),
    spec7!(zuri_jit_invoke_string),
    spec6!(zuri_jit_invoke_super),
    spec6!(zuri_jit_get_global),
    spec6!(zuri_jit_set_global),
    spec6!(zuri_jit_assign_global),
    spec6!(zuri_jit_get_slice),
    spec6!(zuri_jit_make_class),
  ]
}
