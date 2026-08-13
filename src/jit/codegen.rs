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

use std::collections::HashMap;

use cranelift_codegen::ir::condcodes::IntCC;
use cranelift_codegen::ir::{AbiParam, Block, InstBuilder, SigRef, StackSlot, StackSlotData, StackSlotKind, Value as IrValue, types};
use cranelift_frontend::{FunctionBuilder, Variable};
use cranelift_jit::JITModule;
use cranelift_module::{FuncId, Module};
use rustc_hash::FxHashMap;

use crate::vm::chunk::Instr;
use crate::vm::object::ObjFunction;
use crate::vm::value::{self};

/// Compiles `proto`'s bytecode into `fb`'s function body. Returns the
/// bytecode-ip -> osr-id map (`CompiledFunction::osr_ids`) on success,
/// or a human-readable ineligibility reason on failure -- the latter is
/// ALWAYS a permanent, sticky "never try this prototype again" signal
/// (see `VM::try_compile`), never a transient error.
pub fn compile(
  fb: &mut FunctionBuilder,
  module: &mut JITModule,
  helpers: &HashMap<&'static str, FuncId>,
  proto: &ObjFunction,
) -> Result<FxHashMap<usize, i32>, String> {
  // Exception-handling bytecode is never compiled -- see this crate's
  // `jit` module docs on why "bail to the interpreter" is implemented
  // as "never enter compiled code for this function at all" rather
  // than generated unwind logic.
  for instr in &proto.chunk.code {
    if matches!(instr, Instr::Raise { .. } | Instr::PushCatch { .. } | Instr::PopCatch) {
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

  let mut fc = FuncCompiler::new(fb, module, helpers, proto, code_len);
  fc.run()
}

struct FuncCompiler<'a, 'b> {
  fb: &'a mut FunctionBuilder<'b>,
  module: &'a mut JITModule,
  helpers: &'a HashMap<&'static str, FuncId>,
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
}

impl<'a, 'b> FuncCompiler<'a, 'b> {
  fn new(
    fb: &'a mut FunctionBuilder<'b>,
    module: &'a mut JITModule,
    helpers: &'a HashMap<&'static str, FuncId>,
    proto: &'a ObjFunction,
    code_len: usize,
  ) -> Self {
    let blocks = (0..code_len).map(|_| fb.create_block()).collect();
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
    }
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

    let entry_block = self.fb.create_block();
    self.fb.append_block_params_for_function_params(entry_block);
    self.fb.switch_to_block(entry_block);
    let params = self.fb.block_params(entry_block).to_vec();
    self.vm_param = params[0];
    self.base_param = params[1];
    self.closure_param = params[2];
    let osr_param = params[3];

    self.regs_var = self.fb.declare_var(types::I64);
    let initial_regs = self.call_helper("zuri_jit_regs_ptr", &[self.vm_param]);
    self.fb.def_var(self.regs_var, initial_regs);

    let eight = self.fb.ins().iconst(types::I64, 8);
    self.base_bytes = self.fb.ins().imul(self.base_param, eight);

    self.emit_osr_dispatch(osr_param);

    for ip in 0..self.blocks.len() {
      self.fb.switch_to_block(self.blocks[ip]);
      let instr = self.proto.chunk.code[ip];
      let terminated = self.emit_instruction(ip, instr);
      if !terminated {
        let next = self.blocks.get(ip + 1).copied().unwrap_or(self.blocks[ip]);
        self.fb.ins().jump(next, &[]);
      }
    }

    Ok(std::mem::take(&mut self.osr_ids))
  }

  /// `osr_param == -1` -> ordinary entry (`blocks[0]`); `osr_param ==
  /// id` -> `blocks[ip]` for whichever `ip` that `id` was assigned to
  /// in `run`. A linear compare chain (not a `br_table`) -- the number
  /// of loop headers in one function is always small, and this avoids
  /// depending on `JumpTableData`'s exact API for what's a cold, one-
  /// time-per-call dispatch anyway.
  fn emit_osr_dispatch(&mut self, osr_param: IrValue) {
    let neg1 = self.fb.ins().iconst(types::I32, -1);
    let is_normal = self.fb.ins().icmp(IntCC::Equal, osr_param, neg1);
    let mut next_check = self.fb.create_block();
    self.fb.ins().brif(is_normal, self.blocks[0], &[], next_check, &[]);

    let mut targets: Vec<(i32, Block)> = self
      .osr_ids
      .iter()
      .map(|(&ip, &id)| (id, self.blocks[ip]))
      .collect();
    targets.sort_by_key(|&(id, _)| id);

    for (id, target_block) in targets {
      self.fb.switch_to_block(next_check);
      let id_const = self.fb.ins().iconst(types::I32, id as i64);
      let is_this = self.fb.ins().icmp(IntCC::Equal, osr_param, id_const);
      let after = self.fb.create_block();
      self.fb.ins().brif(is_this, target_block, &[], after, &[]);
      next_check = after;
    }

    // Defensive fallback for an `osr_id` that matches none of the
    // known loop headers -- unreachable in practice (`VM::maybe_osr`
    // only ever passes an id it read out of THIS SAME function's own
    // `osr_ids` map), but falling through to the ordinary entry is a
    // safe, well-defined default rather than leaving the block
    // unterminated.
    self.fb.switch_to_block(next_check);
    self.fb.ins().jump(self.blocks[0], &[]);
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

  fn load_reg(&mut self, r: u8) -> IrValue {
    let addr = self.reg_addr(r);
    self.fb.ins().load(types::I64, cranelift_codegen::ir::MemFlagsData::trusted(), addr, 0)
  }

  fn store_reg(&mut self, r: u8, v: IrValue) {
    let addr = self.reg_addr(r);
    self.fb.ins().store(cranelift_codegen::ir::MemFlagsData::trusted(), v, addr, 0);
  }

  fn refresh_regs(&mut self) {
    let fresh = self.call_helper("zuri_jit_regs_ptr", &[self.vm_param]);
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
  /// runtime `chunk.constants[idx]` load.
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
  /// baked as an immediate -- sound because heap objects never move
  /// (see `object::Heap`'s docs) and `proto` outlives this compiled
  /// function (it owns the very bytecode this IS the compilation of).
  fn func_ptr_const(&mut self) -> IrValue {
    self.u64c(self.proto as *const ObjFunction as u64)
  }

  // ---------------------------------------------------------------
  // Helper calls
  // ---------------------------------------------------------------

  fn call_helper(&mut self, name: &str, args: &[IrValue]) -> IrValue {
    let func_id = *self
      .helpers
      .get(name)
      .unwrap_or_else(|| panic!("zuri: unregistered JIT helper '{name}'"));
    let func_ref = self.module.declare_func_in_func(func_id, self.fb.func);
    let call = self.fb.ins().call(func_ref, args);
    self.fb.inst_results(call)[0]
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
    let slot = self
      .fb
      .create_sized_stack_slot(StackSlotData::new(StackSlotKind::ExplicitSlot, 8, 3));
    self.closure_out_slot = Some(slot);
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
    self.fb.ins().brif(is_fast, fast_block, &[], slow_block, &[]);

    self.fb.switch_to_block(fast_block);
    let closure_bits = {
      let slot = self.closure_out_slot();
      self.fb.ins().stack_load(types::I64, types::I64, slot, 0)
    };
    let neg1 = self.fb.ins().iconst(types::I32, -1);
    let sig = self.entry_sig_ref();
    let call = self.fb.ins().call_indirect(sig, prepare, &[self.vm_param, new_base, closure_bits, neg1]);
    let ret_bits = self.fb.inst_results(call)[0];
    self.refresh_regs();
    let base = self.base_param;
    let dst_i = self.idx(dst);
    self.call_checked("zuri_jit_call_finish", &[self.vm_param, base, dst_i, new_base, ret_bits]);
    self.fb.ins().jump(done_block, &[]);

    self.fb.switch_to_block(slow_block);
    self.call_checked(slow_helper, slow_args);
    self.fb.ins().jump(done_block, &[]);

    self.fb.switch_to_block(done_block);
  }

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

  fn to_f64(&mut self, bits: IrValue) -> IrValue {
    self.fb.ins().bitcast(types::F64, cranelift_codegen::ir::MemFlagsData::new(), bits)
  }

  fn from_f64(&mut self, f: IrValue) -> IrValue {
    self.fb.ins().bitcast(types::I64, cranelift_codegen::ir::MemFlagsData::new(), f)
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

  // ---------------------------------------------------------------
  // Per-instruction codegen. Returns `true` if the instruction's own
  // codegen already ends in a terminator (so `run`'s driver loop must
  // NOT append an automatic fallthrough jump), `false` otherwise.
  // ---------------------------------------------------------------

  fn emit_instruction(&mut self, ip: usize, instr: Instr) -> bool {
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
        let v = self.u64c(if val { value::TRUE_VAL } else { value::FALSE_VAL });
        self.store_reg(dst, v);
        false
      },
      Instr::Move { dst, src } => {
        let v = self.load_reg(src);
        self.store_reg(dst, v);
        false
      },

      Instr::Add { dst, a, b } => {
        self.emit_binary_numeric_guarded(dst, a, b, "zuri_jit_add_slow", |fc, fa, fb| fc.fb.ins().fadd(fa, fb));
        false
      },
      Instr::Sub { dst, a, b } => {
        self.emit_binary_numeric_guarded(dst, a, b, "zuri_jit_sub_slow", |fc, fa, fb| fc.fb.ins().fsub(fa, fb));
        false
      },
      Instr::Mul { dst, a, b } => {
        self.emit_binary_numeric_guarded(dst, a, b, "zuri_jit_mul_slow", |fc, fa, fb| fc.fb.ins().fmul(fa, fb));
        false
      },
      Instr::Div { dst, a, b } => {
        self.emit_binary_numeric_guarded(dst, a, b, "zuri_jit_div_slow", |fc, fa, fb| fc.fb.ins().fdiv(fa, fb));
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
        self.emit_bitwise_guarded(dst, a, b, "zuri_jit_bitand_slow", |fb, ia, ib| fb.ins().band(ia, ib));
        false
      },
      Instr::BitOr { dst, a, b } => {
        self.emit_bitwise_guarded(dst, a, b, "zuri_jit_bitor_slow", |fb, ia, ib| fb.ins().bor(ia, ib));
        false
      },
      Instr::BitXor { dst, a, b } => {
        self.emit_bitwise_guarded(dst, a, b, "zuri_jit_bitxor_slow", |fb, ia, ib| fb.ins().bxor(ia, ib));
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
        let v = self.load_reg(src);
        let is_num = self.is_number(v);
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
        self.fb.ins().jump(done_block, &[]);

        self.fb.switch_to_block(done_block);
        false
      },
      Instr::Neg { dst, src } => {
        let v = self.load_reg(src);
        let is_num = self.is_number(v);
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
        self.fb.ins().jump(done_block, &[]);

        self.fb.switch_to_block(done_block);
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
        self.emit_compare_guarded(dst, a, b, "zuri_jit_eq_slow", IntCC::Equal);
        false
      },
      Instr::Neq { dst, a, b } => {
        self.emit_compare_guarded(dst, a, b, "zuri_jit_neq_slow", IntCC::NotEqual);
        false
      },
      Instr::Lt { dst, a, b } => {
        self.emit_fcompare_guarded(dst, a, b, "zuri_jit_lt_slow", cranelift_codegen::ir::condcodes::FloatCC::LessThan);
        false
      },
      Instr::Le { dst, a, b } => {
        self.emit_fcompare_guarded(
          dst,
          a,
          b,
          "zuri_jit_le_slow",
          cranelift_codegen::ir::condcodes::FloatCC::LessThanOrEqual,
        );
        false
      },
      Instr::Gt { dst, a, b } => {
        self.emit_fcompare_guarded(dst, a, b, "zuri_jit_gt_slow", cranelift_codegen::ir::condcodes::FloatCC::GreaterThan);
        false
      },
      Instr::Ge { dst, a, b } => {
        self.emit_fcompare_guarded(
          dst,
          a,
          b,
          "zuri_jit_ge_slow",
          cranelift_codegen::ir::condcodes::FloatCC::GreaterThanOrEqual,
        );
        false
      },

      Instr::Jmp { offset } => {
        let target_ip = (ip as isize + 1 + offset as isize) as usize;
        if offset < 0 {
          self.emit_safepoint();
        }
        self.fb.ins().jump(self.blocks[target_ip], &[]);
        true
      },
      Instr::JmpIfFalse { cond, offset } => {
        let target_ip = (ip as isize + 1 + offset as isize) as usize;
        let base = self.base_param;
        let cond_i = self.idx(cond);
        let falsey = self.call_helper("zuri_jit_is_falsey", &[self.vm_param, base, cond_i]);
        self.refresh_regs();
        let zero = self.i64c(0);
        let is_falsey = self.fb.ins().icmp(IntCC::NotEqual, falsey, zero);
        if offset < 0 {
          self.emit_safepoint();
        }
        self.fb.ins().brif(is_falsey, self.blocks[target_ip], &[], self.blocks[ip + 1], &[]);
        true
      },
      Instr::JmpIfTrue { cond, offset } => {
        let target_ip = (ip as isize + 1 + offset as isize) as usize;
        let base = self.base_param;
        let cond_i = self.idx(cond);
        let truthy = self.call_helper("zuri_jit_is_falsey", &[self.vm_param, base, cond_i]);
        self.refresh_regs();
        let zero = self.i64c(0);
        let is_truthy = self.fb.ins().icmp(IntCC::Equal, truthy, zero);
        if offset < 0 {
          self.emit_safepoint();
        }
        self.fb.ins().brif(is_truthy, self.blocks[target_ip], &[], self.blocks[ip + 1], &[]);
        true
      },

      Instr::Call { dst, func, num_args } => {
        self.emit_safepoint();
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
        false
      },
      Instr::Return { src } => {
        let base = self.base_param;
        let zero = self.i64c(0);
        self.call_checked("zuri_jit_close_upvalues", &[self.vm_param, base, zero]);
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
        let base = self.base_param;
        let dst_i = self.idx(dst);
        let func_ptr = self.func_ptr_const();
        let name = self.bake_const(name_const);
        let ip_c = self.u64c(ip as u64);
        self.call_checked("zuri_jit_get_global", &[self.vm_param, base, dst_i, func_ptr, name, ip_c]);
        false
      },
      Instr::SetGlobal { name_const, src } => {
        let base = self.base_param;
        let src_i = self.idx(src);
        let func_ptr = self.func_ptr_const();
        let name = self.bake_const(name_const);
        let ip_c = self.u64c(ip as u64);
        self.call_checked("zuri_jit_set_global", &[self.vm_param, base, src_i, func_ptr, name, ip_c]);
        false
      },
      Instr::AssignGlobal { name_const, src } => {
        let base = self.base_param;
        let src_i = self.idx(src);
        let func_ptr = self.func_ptr_const();
        let name = self.bake_const(name_const);
        let ip_c = self.u64c(ip as u64);
        self.call_checked("zuri_jit_assign_global", &[self.vm_param, base, src_i, func_ptr, name, ip_c]);
        false
      },

      Instr::Closure { dst, proto_const } => {
        let base = self.base_param;
        let dst_i = self.idx(dst);
        let proto_v = self.bake_const(proto_const);
        self.call_checked("zuri_jit_make_closure", &[self.vm_param, base, dst_i, proto_v, self.closure_param]);
        false
      },
      Instr::GetUpval { dst, idx: uidx } => {
        let base = self.base_param;
        let dst_i = self.idx(dst);
        let uidx_i = self.idx(uidx);
        self.call_checked("zuri_jit_get_upval", &[self.vm_param, base, dst_i, uidx_i, self.closure_param]);
        false
      },
      Instr::SetUpval { idx: uidx, src } => {
        let base = self.base_param;
        let src_i = self.idx(src);
        let uidx_i = self.idx(uidx);
        self.call_checked("zuri_jit_set_upval", &[self.vm_param, base, src_i, uidx_i, self.closure_param]);
        false
      },
      Instr::CloseUpvalues { from } => {
        let base = self.base_param;
        let from_i = self.idx(from);
        self.call_checked("zuri_jit_close_upvalues", &[self.vm_param, base, from_i]);
        false
      },

      Instr::MakeList { dst, start, count } => {
        let base = self.base_param;
        let dst_i = self.idx(dst);
        let start_i = self.idx(start);
        let count_i = self.idx(count);
        self.call_checked("zuri_jit_make_list", &[self.vm_param, base, dst_i, start_i, count_i]);
        false
      },
      Instr::MakeDict { dst, start, count } => {
        let base = self.base_param;
        let dst_i = self.idx(dst);
        let start_i = self.idx(start);
        let count_i = self.idx(count);
        self.call_checked("zuri_jit_make_dict", &[self.vm_param, base, dst_i, start_i, count_i]);
        false
      },

      Instr::MakeClass { dst, name_const, superclass } => {
        let base = self.base_param;
        let dst_i = self.idx(dst);
        let name = self.bake_const(name_const);
        let (has_super, super_reg) = match superclass {
          Some(r) => (self.i64c(1), self.idx(r)),
          None => (self.i64c(0), self.i64c(0)),
        };
        self.call_checked("zuri_jit_make_class", &[self.vm_param, base, dst_i, name, has_super, super_reg]);
        false
      },
      Instr::DeclareField { class, name_const } => {
        let base = self.base_param;
        let class_i = self.idx(class);
        let name = self.bake_const(name_const);
        self.call_checked("zuri_jit_declare_field", &[self.vm_param, base, class_i, name]);
        false
      },
      Instr::SetFieldInit { class, src } => {
        let base = self.base_param;
        let class_i = self.idx(class);
        let src_i = self.idx(src);
        self.call_checked("zuri_jit_set_field_init", &[self.vm_param, base, class_i, src_i]);
        false
      },
      Instr::SetMethod { class, name_const, src } => {
        let base = self.base_param;
        let class_i = self.idx(class);
        let name = self.bake_const(name_const);
        let src_i = self.idx(src);
        self.call_checked("zuri_jit_set_method", &[self.vm_param, base, class_i, name, src_i]);
        false
      },
      Instr::DeclareStatic { class, name_const, src } => {
        let base = self.base_param;
        let class_i = self.idx(class);
        let name = self.bake_const(name_const);
        let src_i = self.idx(src);
        self.call_checked("zuri_jit_declare_static", &[self.vm_param, base, class_i, name, src_i]);
        false
      },
      Instr::FinalizeClass { class } => {
        let base = self.base_param;
        let class_i = self.idx(class);
        let func_ptr = self.func_ptr_const();
        self.call_checked("zuri_jit_finalize_class", &[self.vm_param, base, class_i, func_ptr]);
        false
      },
      Instr::GetField { dst, obj, name_const } => {
        let base = self.base_param;
        let dst_i = self.idx(dst);
        let obj_i = self.idx(obj);
        let name = self.bake_const(name_const);
        self.call_checked("zuri_jit_get_field", &[self.vm_param, base, dst_i, obj_i, name]);
        false
      },
      Instr::SetField { obj, name_const, src } => {
        let base = self.base_param;
        let obj_i = self.idx(obj);
        let name = self.bake_const(name_const);
        let src_i = self.idx(src);
        self.call_checked("zuri_jit_set_field", &[self.vm_param, base, obj_i, name, src_i]);
        false
      },

      Instr::Invoke { dst, obj, method_const, num_args } => {
        self.emit_safepoint();
        let base = self.base_param;
        let vm_p = self.vm_param;
        let obj_i = self.idx(obj);
        let num_args_i = self.idx(num_args);
        let dst_i = self.idx(dst);
        let name = self.bake_const(method_const);
        let new_base = self.fb.ins().iadd_imm_s(base, obj as i64 + 1);
        self.emit_fast_call(
          "zuri_jit_invoke_prepare",
          &[vm_p, base, obj_i, num_args_i, dst_i, name],
          new_base,
          dst,
          "zuri_jit_invoke",
          &[vm_p, base, obj_i, num_args_i, dst_i, name],
        );
        false
      },
      Instr::InvokeSuper { dst, superclass, method_const, num_args } => {
        self.emit_safepoint();
        let base = self.base_param;
        let super_i = self.idx(superclass);
        let num_args_i = self.idx(num_args);
        let dst_i = self.idx(dst);
        let name = self.bake_const(method_const);
        self.call_checked("zuri_jit_invoke_super", &[self.vm_param, base, super_i, num_args_i, dst_i, name]);
        false
      },
      Instr::CallSuperCtor { dst, superclass, num_args } => {
        self.emit_safepoint();
        let base = self.base_param;
        let super_i = self.idx(superclass);
        let num_args_i = self.idx(num_args);
        let dst_i = self.idx(dst);
        self.call_checked("zuri_jit_call_super_ctor", &[self.vm_param, base, super_i, num_args_i, dst_i]);
        false
      },

      Instr::Import { dst, path_const, importer_const } => {
        self.emit_safepoint();
        let base = self.base_param;
        let dst_i = self.idx(dst);
        let path = self.bake_const(path_const);
        let importer = self.bake_const(importer_const);
        self.call_checked("zuri_jit_import", &[self.vm_param, base, dst_i, path, importer]);
        false
      },
      Instr::ImportAll { module } => {
        let base = self.base_param;
        let module_i = self.idx(module);
        let func_ptr = self.func_ptr_const();
        self.call_checked("zuri_jit_import_all", &[self.vm_param, base, module_i, func_ptr]);
        false
      },
      Instr::MakePromoted { dst, module, name_const } => {
        let base = self.base_param;
        let dst_i = self.idx(dst);
        let module_i = self.idx(module);
        let name = self.bake_const(name_const);
        self.call_checked("zuri_jit_make_promoted", &[self.vm_param, base, dst_i, module_i, name]);
        false
      },

      Instr::GetIndex { dst, obj, idx: iidx } => {
        let base = self.base_param;
        let dst_i = self.idx(dst);
        let obj_i = self.idx(obj);
        let idx_i = self.idx(iidx);
        self.call_checked("zuri_jit_get_index", &[self.vm_param, base, dst_i, obj_i, idx_i]);
        false
      },
      Instr::SetIndex { obj, idx: iidx, src } => {
        let base = self.base_param;
        let obj_i = self.idx(obj);
        let idx_i = self.idx(iidx);
        let src_i = self.idx(src);
        self.call_checked("zuri_jit_set_index", &[self.vm_param, base, obj_i, idx_i, src_i]);
        false
      },
      Instr::GetSlice { dst, obj, lo, hi } => {
        let base = self.base_param;
        let dst_i = self.idx(dst);
        let obj_i = self.idx(obj);
        let lo_i = self.idx(lo);
        let hi_i = self.idx(hi);
        self.call_checked("zuri_jit_get_slice", &[self.vm_param, base, dst_i, obj_i, lo_i, hi_i]);
        false
      },

      Instr::MakeRange { dst, lower, upper } => {
        let base = self.base_param;
        let dst_i = self.idx(dst);
        let lower_i = self.idx(lower);
        let upper_i = self.idx(upper);
        self.call_checked("zuri_jit_make_range", &[self.vm_param, base, dst_i, lower_i, upper_i]);
        false
      },

      Instr::UsingJump { subject, table_idx } => {
        let base = self.base_param;
        let subject_i = self.idx(subject);
        let func_ptr = self.func_ptr_const();
        let table_i = self.i64c(table_idx as i64);
        let target = self.call_helper("zuri_jit_using_jump", &[self.vm_param, base, subject_i, func_ptr, table_i]);
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
        self.fb.ins().brif(matched, next_check, &[], miss_block, &[]);

        let mut targets: Vec<usize> = self.proto.chunk.jump_tables[table_idx as usize].values().copied().collect();
        targets.sort_unstable();
        targets.dedup();
        for target_ip in targets {
          self.fb.switch_to_block(next_check);
          let want = self.u64c(target_ip as u64);
          let is_this = self.fb.ins().icmp(IntCC::Equal, target, want);
          let after = self.fb.create_block();
          self.fb.ins().brif(is_this, self.blocks[target_ip], &[], after, &[]);
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
        self.emit_addimm(dst, a, imm_const);
        false
      },
      Instr::SubImm { dst, a, imm_const } => {
        self.emit_imm_numeric_guarded(dst, a, imm_const, "zuri_jit_subimm_slow", |fc, fa, fimm| fc.fb.ins().fsub(fa, fimm));
        false
      },
      Instr::MulImm { dst, a, imm_const } => {
        // No inline fast path beyond the numeric guard -- the
        // non-numeric fallback (string/list repeat) is common enough
        // (and cheap enough to check for) that `zuri_jit_mulimm_slow`
        // handles the WHOLE non-fast-path case uniformly; see its docs.
        self.emit_imm_numeric_guarded(dst, a, imm_const, "zuri_jit_mulimm_slow", |fc, fa, fimm| fc.fb.ins().fmul(fa, fimm));
        false
      },
      Instr::LtImm { dst, a, imm_const } => {
        self.emit_imm_compare_guarded(dst, a, imm_const, "zuri_jit_ltimm_slow", cranelift_codegen::ir::condcodes::FloatCC::LessThan);
        false
      },
      Instr::LeImm { dst, a, imm_const } => {
        self.emit_imm_compare_guarded(
          dst,
          a,
          imm_const,
          "zuri_jit_leimm_slow",
          cranelift_codegen::ir::condcodes::FloatCC::LessThanOrEqual,
        );
        false
      },
      Instr::GtImm { dst, a, imm_const } => {
        self.emit_imm_compare_guarded(
          dst,
          a,
          imm_const,
          "zuri_jit_gtimm_slow",
          cranelift_codegen::ir::condcodes::FloatCC::GreaterThan,
        );
        false
      },
      Instr::GeImm { dst, a, imm_const } => {
        self.emit_imm_compare_guarded(
          dst,
          a,
          imm_const,
          "zuri_jit_geimm_slow",
          cranelift_codegen::ir::condcodes::FloatCC::GreaterThanOrEqual,
        );
        false
      },
      Instr::EqImm { dst, a, imm_const } => {
        self.emit_imm_eq(dst, a, imm_const, true);
        false
      },
      Instr::NeqImm { dst, a, imm_const } => {
        self.emit_imm_eq(dst, a, imm_const, false);
        false
      },
    }
  }

  /// GC safepoint -- see `runtime::zuri_jit_gc_safepoint`'s own docs.
  /// Emitted at every loop back-edge and function/method call site,
  /// matching the standard "safepoints at back-edges and calls"
  /// baseline-JIT policy this project's design calls for.
  fn emit_safepoint(&mut self) {
    self.call_helper("zuri_jit_gc_safepoint", &[self.vm_param]);
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
    self.fb.ins().jump(done_block, &[]);

    self.fb.switch_to_block(done_block);
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
    self.fb.ins().jump(done_block, &[]);

    self.fb.switch_to_block(done_block);
  }

  fn emit_always_helper(&mut self, helper: &'static str, dst: u8, a: u8, b: u8) {
    let base = self.base_param;
    let dst_i = self.idx(dst);
    let a_i = self.idx(a);
    let b_i = self.idx(b);
    self.call_checked(helper, &[self.vm_param, base, dst_i, a_i, b_i]);
  }

  fn emit_compare_guarded(&mut self, dst: u8, a: u8, b: u8, slow_helper: &'static str, cc: IntCC) {
    // Eq/Neq never consult an operator override (matches the
    // interpreter's own handler exactly -- see `vm.rs`), so the ONLY
    // reason to fall to the helper is a non-number operand needing a
    // real (potentially heap-dereferencing) `Value::equals`.
    let va = self.load_reg(a);
    let vb = self.load_reg(b);
    let guard = self.both_numbers(va, vb);
    let fast_block = self.fb.create_block();
    let slow_block = self.fb.create_block();
    let done_block = self.fb.create_block();
    self.fb.ins().brif(guard, fast_block, &[], slow_block, &[]);

    self.fb.switch_to_block(fast_block);
    let fa = self.to_f64(va);
    let fb_ = self.to_f64(vb);
    let cmp = self.fb.ins().fcmp(to_float_cc(cc), fa, fb_);
    let bits = self.bool_value(cmp);
    self.store_reg(dst, bits);
    self.fb.ins().jump(done_block, &[]);

    self.fb.switch_to_block(slow_block);
    let base = self.base_param;
    let dst_i = self.idx(dst);
    let a_i = self.idx(a);
    let b_i = self.idx(b);
    self.call_checked(slow_helper, &[self.vm_param, base, dst_i, a_i, b_i]);
    self.fb.ins().jump(done_block, &[]);

    self.fb.switch_to_block(done_block);
  }

  fn emit_fcompare_guarded(&mut self, dst: u8, a: u8, b: u8, slow_helper: &'static str, cc: cranelift_codegen::ir::condcodes::FloatCC) {
    let va = self.load_reg(a);
    let vb = self.load_reg(b);
    let guard = self.both_numbers(va, vb);
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
    self.fb.ins().jump(done_block, &[]);

    self.fb.switch_to_block(done_block);
  }

  fn emit_addimm(&mut self, dst: u8, a: u8, imm_const: u16) {
    let va = self.load_reg(a);
    let guard = self.is_number(va);
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
    self.call_checked("zuri_jit_addimm_slow", &[self.vm_param, base, dst_i, a_i, imm_bits]);
    self.fb.ins().jump(done_block, &[]);

    self.fb.switch_to_block(done_block);
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
    self.fb.ins().jump(done_block, &[]);

    self.fb.switch_to_block(done_block);
  }

  fn emit_imm_compare_guarded(&mut self, dst: u8, a: u8, imm_const: u16, slow_helper: &'static str, cc: cranelift_codegen::ir::condcodes::FloatCC) {
    let va = self.load_reg(a);
    let guard = self.is_number(va);
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
    self.fb.ins().jump(done_block, &[]);

    self.fb.switch_to_block(done_block);
  }

  /// `EqImm`/`NeqImm` -- like the interpreter's own handler, this is
  /// pure `Value::equals` against a KNOWN-numeric constant, no operator
  /// override lookup ever. Fully inlinable with no helper fallback at
  /// all: when `a` is itself a number, real IEEE-754 equality decides
  /// it; otherwise `Value::equals` falls through to a raw bit compare
  /// (see `value.rs`), which is exactly what comparing the two `u64`s
  /// directly already gives here.
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
    let int_cc = if want_eq { IntCC::Equal } else { IntCC::NotEqual };
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
