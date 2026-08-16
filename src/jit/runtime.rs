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
//! for that instruction does, by calling the SAME underlying `VM`
//! methods (`binary_add`, `dispatch_call`, `index_get`, ...). There is
//! exactly one place that implements what any given opcode MEANS;
//! this module is glue, not a second implementation.
//!
//! # The error-propagation protocol
//!
//! None of these ever return a `Result` (they're a raw C ABI, and
//! `Result<Value, Value>` isn't a `repr(C)` type worth wrestling into
//! one for every call site). Instead, every helper that can fail
//! returns a `u64` status: `0` for success, `1` for failure. On
//! failure, the helper has ALREADY stored the propagating exception
//! `Value` into `VM::jit_pending_exception` before returning. Compiled
//! code checks this status immediately after the call (see
//! `codegen::FuncCompiler::emit_call`); on failure it abandons the rest
//! of the function entirely and returns up to its own Rust caller
//! (`VM::invoke_compiled`), which reads `jit_pending_exception` and
//! turns it into a real `Err(Value)` -- exactly mirroring how an
//! interpreted `Err(Value)` already propagates via `?`/`tri!`. This is
//! the concrete mechanism behind "exceptions cause a bailout" for
//! compiled code: there is no unwinder in the generated machine code at
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
//! code therefore NEVER caches that pointer across such a call -- it
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
//! constant's `Value` directly out of the ALREADY-COMPILED `ObjFunction`
//! at JIT-COMPILE time and bakes its raw bit pattern into the generated
//! code as an immediate -- there is no `chunk.constants[i]` LOAD at
//! runtime anywhere in compiled code. Helpers below that take a
//! `*_bits: u64` parameter are receiving one of these baked constants,
//! not a live register read. The same trick applies to a stable pointer
//! to the currently-compiling function's own `ObjFunction` itself
//! (`func_ptr: u64`), needed by a few helpers for `Chunk::global_cache`/
//! `Chunk::jump_tables` access.

use std::cell::Cell;

use crate::vm::chunk::JumpKey;
use crate::vm::object::{
  ListStorage, Obj, ObjClosure, ObjFunction, UpvalueDescriptor, UpvalueState, write_barrier,
};
use crate::vm::value::Value;
use crate::vm::vm::VM;

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
/// Real deoptimization -- records `ip` as where compiled code is
/// giving up, so `VM::invoke_compiled` picks it up right after this
/// call's caller returns. Codegen always follows a call to this with
/// an immediate `return_` out of the WHOLE compiled function (never
/// falls through to more translated instructions), so the actual
/// return value here is never observed -- unlike every other helper,
/// its result is dead by construction.
///
/// Sound for the same reason the GC safepoint above is: every VM
/// register a compiled function operates on lives in `VM::registers`
/// at every instruction boundary, never only in a native machine
/// register that would need to be found and translated back. So
/// "deoptimizing" needs no state reconstruction at all -- the
/// interpreter reads the exact same array it always does, starting
/// fresh at `ip`. See `VM::pending_deopt_ip`'s own docs for the full
/// reasoning and `VM::invoke_compiled` for where this is consumed.
pub unsafe extern "C" fn zuri_jit_deopt(vm_ptr: *mut VM, ip: u64) -> u64 {
  let vm = unsafe { vm(vm_ptr) };
  vm.pending_deopt_ip.set(Some(ip as usize));
  OK
}

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
  |x: f64, y: f64| x % y,
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

pub unsafe extern "C" fn zuri_jit_logical_not(
  vm_ptr: *mut VM,
  base: u64,
  dst: u64,
  src: u64,
) -> u64 {
  let vm = unsafe { vm(vm_ptr) };
  let v = vm.get_reg(base as usize, src as u8);
  vm.set_reg(base as usize, dst as u8, Value::bool(v.is_falsey()));
  OK
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
/// `compiler::imm_arith_ctor`), but the REGISTER operand might not be
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
// PRIMARY path once their target has warmed up, bypassing
// `dispatch_call_sync`'s fully general dispatch (which re-derives arity/
// variadic/frame-setup logic AND pays real `Option`/`RunResult`
// plumbing on every single call) entirely for the one case that matters
// most for a JIT's own performance: a call from compiled code straight
// into ANOTHER already-compiled function.
//
// The `*_prepare` helpers below are a PURE peek-and-set-up step: they
// never trigger compilation and never touch `call_count`/the warm-up
// counters. A cold callee (including the very first call that happens
// to cross its own warm-up threshold) always returns `0` here and falls
// through to the fully general slow path (`zuri_jit_call`/
// `zuri_jit_invoke`), which already owns ALL of that bookkeeping --
// duplicating it here would risk double-counting a single call toward
// warm-up. Once a callee is compiled, every LATER call to it takes this
// fast path instead.
//
// `codegen::FuncCompiler::emit_fast_call` is the generated-code half of
// this protocol: call `*_prepare`, passing the address of an 8-byte
// scratch stack slot as its LAST argument; if it returns a non-zero
// entry pointer, read the closure bits `prepare` wrote into that slot
// (needed for `GetUpval`/`SetUpval`/`Instr::Closure` inside the callee
// -- for `Invoke` specifically, the resolved METHOD closure is never
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
/// compiled `Closure`. On success, ALSO performs every bit of frame
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
  // MUST happen before `closure`/`proto` are derived: `closure_out`'s
  // write below becomes the CALLEE's own `closure_param` -- a plain
  // Cranelift SSA value the compiled callee reuses for its WHOLE
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
  vm.jit_depth_enter();
  unsafe { *(closure_out as *mut u64) = callee.to_bits() };
  entry as usize as u64
}

/// `Instr::Invoke`'s fast-path peek: `obj` must hold an instance whose
/// class resolves `method_name_bits` to an already-compiled `Closure`
/// method (NOT a field holding a callable, and not a builtin/native
/// fallback -- both of those still go through the fully general
/// `zuri_jit_invoke`). Mirrors `zuri_jit_call_prepare` otherwise (see
/// its docs for the full protocol), except the value written to
/// `*closure_out` is the resolved METHOD, not the receiver in `obj`.
// NOTE: `closure_out` is declared LAST here, not next to
// `method_name_bits`, because `codegen::FuncCompiler::emit_fast_call`
// always APPENDS the closure-out-slot address as the final argument to
// whatever `prepare_args` it's given -- the parameter order here must
// match that calling convention exactly, or the wrong register-sized
// slot ends up interpreted as a pointer (a real bug this project
// tripped on once already: a misaligned-pointer panic from `func_ptr`/
// `instr_ip` landing where `closure_out` was expected).
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

  // Inline cache: same receiver CLASS as the last time this exact
  // `Instr::Invoke` ran -> reuse the resolved method `Value` directly,
  // skipping `ObjClass::methods`'s hash-map probe entirely. See
  // `Chunk::method_cache`'s own docs.
  let cached = func
    .chunk
    .method_cache
    .borrow()
    .get(&instr_ip)
    .filter(|&&(cached_class, _)| cached_class == class_bits)
    .map(|&(_, method_bits)| Value::from_bits(method_bits));

  let method = if let Some(m) = cached {
    Some(m)
  } else {
    let method_name = Value::from_bits(method_name_bits);
    let class = inst.class.as_class();
    let resolved = class.methods.get(method_name.as_str()).copied();
    if let Some(m) = resolved {
      func
        .chunk
        .method_cache
        .borrow_mut()
        .insert(instr_ip, (class_bits, m.to_bits()));
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
  vm.jit_depth_enter();
  unsafe { *(closure_out as *mut u64) = method.to_bits() };
  entry as usize as u64
}

/// Completes a fast-path direct call after generated code's own
/// `call_indirect` returns -- the other half of `zuri_jit_call_prepare`/
/// `zuri_jit_invoke_prepare`'s bracket. `new_base` is the CALLEE's own
/// frame base (needed to close its upvalues); `base`/`dst` are the
/// CALLER's, to write the return value into the right place. Follows
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

  // MUST be checked before anything else treats `ret_bits` as a real
  // return value: this is the direct compiled-to-compiled fast path
  // (`codegen::FuncCompiler::emit_fast_call`'s own `call_indirect`,
  // never going through `VM::invoke_compiled` at all), so it's the
  // ONLY place that would ever see a deopt from a callee reached this
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
) -> u64 {
  let vm = unsafe { vm(vm_ptr) };
  let base = base as usize;
  let obj = obj as u8;
  let num_args = num_args as u8;
  let dst = dst as u8;
  let method_name = Value::from_bits(method_name_bits);
  let receiver = vm.get_reg(base, obj);

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
        None => match crate::builtins::lookup(receiver, method_name.as_str()) {
          Some(native) => {
            let call_args = invoke_native_args(vm, base, obj, num_args, receiver);
            let result = vm.call_native(native, &call_args)?;
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
        None => match crate::builtins::lookup(receiver, method_name.as_str()) {
          Some(native) => {
            let call_args = invoke_native_args(vm, base, obj, num_args, receiver);
            let result = vm.call_native(native, &call_args)?;
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
      match crate::builtins::lookup(receiver, method_name.as_str()) {
        Some(native) => {
          let call_args = invoke_native_args(vm, base, obj, num_args, receiver);
          let result = vm.call_native(native, &call_args)?;
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
  })();

  match result {
    Ok(()) => OK,
    Err(e) => fail(vm, e),
  }
}

/// Builds the owned args slice `Instr::Invoke`'s native/builtin-method
/// fallback needs (receiver spliced in as `args[0]`, matching
/// `VM::call_native`'s `is_method` convention) -- mirrors the `CallArgs`
/// construction `vm.rs`'s own handler does at each of these three call
/// sites.
fn invoke_native_args(vm: &VM, base: usize, obj: u8, num_args: u8, receiver: Value) -> Vec<Value> {
  let mut args = Vec::with_capacity(num_args as usize + 1);
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
      // so every LATER execution of this instruction, from compiled
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
/// constant; `closure_bits` is the CURRENTLY EXECUTING closure (this
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

/// Resolves an ALREADY-PROVEN `Obj::List`'s current backing-buffer
/// pointer and length -- deliberately minimal, since it exists ONLY
/// for `jit::codegen`'s list-index fast path (`emit_get_index`/
/// `emit_set_index`), which has already confirmed via an inline tag
/// check that `list_obj_ptr` really is one before ever calling this.
/// No type dispatch, no borrow check: `SmallVec`'s own internal
/// tagged-union layout (inline array vs. heap spill) isn't a stable
/// ABI this compiler can replicate as raw offsets the way it does for
/// this project's OWN `#[repr(C)]` types (`ObjInstance`/
/// `FieldStorage`), so resolving the CURRENT data pointer still goes
/// through `SmallVec`'s real accessor here -- everything AROUND that
/// (the bounds check, the actual element load/store) is genuine
/// inline Cranelift code, not part of this call.
///
/// Skips `RefCell`'s runtime borrow flag entirely, matching
/// `Value::list_get`/`list_len`'s own release-build reasoning (see
/// their docs): single-threaded execution with no live Ref/RefMut
/// held across a re-entrant call that could touch this same list means
/// bypassing the flag can't observe real aliasing.
pub unsafe extern "C" fn zuri_jit_list_data(
  _vm_ptr: *mut VM,
  list_obj_ptr: u64,
  len_out: u64,
) -> u64 {
  let obj = unsafe { &*(list_obj_ptr as *const Obj) };
  let Obj::List(cell) = obj else {
    unreachable!("caller already proved this is Obj::List via an inline tag check")
  };
  // SAFETY: matches Value::list_get/list_len's own release-build
  // reasoning -- see this function's own docs.
  let sv: &ListStorage = unsafe { &*cell.as_ptr() };
  unsafe { *(len_out as *mut u64) = sv.len() as u64 };
  sv.as_ptr() as u64
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
  // name is always compiled as constant index 0 of ITS declaring
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

    // Inline cache: same receiver CLASS as the last time this exact
    // `Instr::GetField` ran -> reuse its resolved slot directly,
    // skipping `ObjClass::field_slots`'s hash-map probe entirely. See
    // `Chunk::field_cache`'s own docs.
    let cached_slot = func
      .chunk
      .field_cache
      .borrow()
      .get(&instr_ip)
      .filter(|&&(cached_class, _)| cached_class == class_bits)
      .map(|&(_, slot)| slot);

    if let Some(idx) = cached_slot {
      Ok(inst.fields[idx as usize].get())
    } else {
      let class = inst.class.as_class();
      if let Some(&idx) = class.field_slots.get(name_val.as_str()) {
        func
          .chunk
          .field_cache
          .borrow_mut()
          .insert(instr_ip, (class_bits, idx));
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
    let cached_slot = func
      .chunk
      .field_cache
      .borrow()
      .get(&instr_ip)
      .filter(|&&(cached_class, _)| cached_class == class_bits)
      .map(|&(_, slot)| slot);

    if let Some(idx) = cached_slot {
      inst.fields[idx as usize].set(value);
      write_barrier(receiver.as_obj());
      Ok(())
    } else {
      let class = inst.class.as_class();
      match class.field_slots.get(name_val.as_str()).copied() {
        Some(idx) => {
          func
            .chunk
            .field_cache
            .borrow_mut()
            .insert(instr_ip, (class_bits, idx));
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
  /// Parameter count INCLUDING the leading `vm` pointer -- matches
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
/// (one of the `FnN` aliases above -- NOT a bare function item, which
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
    spec4!(zuri_jit_logical_not),
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
    spec9!(zuri_jit_invoke_prepare),
    spec5!(zuri_jit_call_finish),
    spec5!(zuri_jit_call_super_ctor),
    spec5!(zuri_jit_make_closure),
    spec5!(zuri_jit_get_upval),
    spec5!(zuri_jit_set_upval),
    spec5!(zuri_jit_make_list),
    spec5!(zuri_jit_make_dict),
    spec5!(zuri_jit_make_range),
    spec5!(zuri_jit_get_index),
    spec5!(zuri_jit_set_index),
    spec3!(zuri_jit_list_data),
    spec5!(zuri_jit_set_method),
    spec5!(zuri_jit_declare_static),
    spec7!(zuri_jit_get_field),
    spec7!(zuri_jit_set_field),
    spec5!(zuri_jit_using_jump),
    spec5!(zuri_jit_import),
    spec5!(zuri_jit_make_promoted),
    spec6!(zuri_jit_invoke),
    spec6!(zuri_jit_invoke_super),
    spec6!(zuri_jit_get_global),
    spec6!(zuri_jit_set_global),
    spec6!(zuri_jit_assign_global),
    spec6!(zuri_jit_get_slice),
    spec6!(zuri_jit_make_class),
  ]
}
