//! IR to Cranelift.
//!
//! Each IR block becomes one Cranelift block and each IR value one
//! Cranelift value, with block parameters for the values that flow along
//! edges. The function has the same signature and entry protocol as a
//! baseline-tier function (`jit::EntryFn`), so the interpreter, the other
//! tier and the runtime helpers cannot tell the two apart.
//!
//! Everything that leaves compiled code goes through a frame state. A
//! failed guard branches to a cold block that writes each state value,
//! boxed, into its register and hands the frame to the interpreter; a
//! runtime helper call writes the same values first and reads the
//! registers back afterwards. Frame states may name unboxed values, which
//! is what lets the passes keep a loop's numbers unboxed: the boxing only
//! happens on those cold paths.
//!
//! Code built in from a callee runs its helpers against the callee's own
//! register window and function, and leaves through a deopt that pushes
//! the interpreter frames the calls would have had.

use cranelift_codegen::ir::condcodes::{FloatCC, IntCC};
use cranelift_codegen::ir::{
  AbiParam, Block, InstBuilder, MemFlagsData, SigRef, StackSlot, StackSlotData, StackSlotKind,
  Value as IrValue, types,
};
use cranelift_frontend::{FunctionBuilder, Variable};
use cranelift_jit::JITModule;
use cranelift_module::{FuncId, Module};
use rustc_hash::FxHashMap;

use super::{
  BitOp, BlockId, Cmp, FTest, FUnary, FrameState, Func, GuardKind, InstId, IntOp, Op, Terminator,
  Ty, ValueId,
};
use crate::vm::chunk::Instr;
use crate::vm::object::{self, ObjFunction};
use crate::vm::value;
use crate::vm::vm;

const REGS_PTR_CACHE_OFFSET: i32 = vm::VM_REGS_PTR_CACHE_OFFSET as i32;
const REGS_LEN_CACHE_OFFSET: i32 = vm::VM_REGS_LEN_CACHE_OFFSET as i32;
const INTERNED_ASCII_OFFSET: i32 = vm::VM_INTERNED_ASCII_OFFSET as i32;
const HEAP_NURSERY_CUR_OFFSET: i32 = (vm::VM_HEAP_OFFSET + object::HEAP_NURSERY_CUR_OFFSET) as i32;
const HEAP_NURSERY_END_OFFSET: i32 = (vm::VM_HEAP_OFFSET + object::HEAP_NURSERY_END_OFFSET) as i32;
const HEAP_FINALIZE_CUR_OFFSET: i32 =
  (vm::VM_HEAP_OFFSET + object::HEAP_FINALIZE_CUR_OFFSET) as i32;
const HEAP_FINALIZE_END_OFFSET: i32 =
  (vm::VM_HEAP_OFFSET + object::HEAP_FINALIZE_END_OFFSET) as i32;
const HEAP_LIVE_COUNT_OFFSET: i32 = (vm::VM_HEAP_OFFSET + object::HEAP_LIVE_COUNT_OFFSET) as i32;
const HEAP_YOUNG_BYTES_ALLOCATED_OFFSET: i32 =
  (vm::VM_HEAP_OFFSET + object::HEAP_YOUNG_BYTES_ALLOCATED_OFFSET) as i32;
const HEAP_BYTES_ALLOCATED_OFFSET: i32 =
  (vm::VM_HEAP_OFFSET + object::HEAP_BYTES_ALLOCATED_OFFSET) as i32;
const HEAP_YOUNG_HEADER_OFFSET: i32 =
  (vm::VM_HEAP_OFFSET + object::HEAP_YOUNG_HEADER_OFFSET) as i32;
const JIT_IP_OFFSET: i32 = vm::VM_JIT_IP_OFFSET as i32;
const GLOBAL_SLOTS_PTR_CACHE_OFFSET: i32 = vm::VM_GLOBAL_SLOTS_PTR_CACHE_OFFSET as i32;
const METHOD_TABLE_GENERATION_OFFSET: i32 = vm::VM_METHOD_TABLE_GENERATION_OFFSET as i32;
const HEAP_JIT_GC_NEEDED_OFFSET: i32 =
  (vm::VM_HEAP_OFFSET + object::HEAP_JIT_GC_NEEDED_OFFSET) as i32;
const FRAMES_PTR_OFFSET: i32 = (vm::VM_FRAMES_OFFSET + vm::FRAMESTACK_PTR_OFFSET) as i32;
const FRAMES_LEN_OFFSET: i32 = (vm::VM_FRAMES_OFFSET + vm::FRAMESTACK_LEN_OFFSET) as i32;
const FRAMES_CAP_OFFSET: i32 = (vm::VM_FRAMES_OFFSET + vm::FRAMESTACK_CAP_OFFSET) as i32;
const JIT_CALL_DEPTH_OFFSET: i32 = vm::VM_JIT_CALL_DEPTH_OFFSET as i32;
const JIT_SCALAR_ROOTS_LEN_OFFSET: i32 = vm::VM_JIT_SCALAR_ROOTS_LEN_OFFSET as i32;
const PENDING_DEOPT_IP_OFFSET: i32 = vm::VM_PENDING_DEOPT_IP_OFFSET as i32;
const CALL_FRAME_FUNCTION_OFFSET: i32 = vm::CALL_FRAME_FUNCTION_OFFSET as i32;
const CALL_FRAME_CLOSURE_VAL_OFFSET: i32 = vm::CALL_FRAME_CLOSURE_VAL_OFFSET as i32;
const CALL_FRAME_IP_OFFSET: i32 = vm::CALL_FRAME_IP_OFFSET as i32;
const CALL_FRAME_BASE_OFFSET: i32 = vm::CALL_FRAME_BASE_OFFSET as i32;
const CALL_FRAME_DST_IN_CALLER_OFFSET: i32 = vm::CALL_FRAME_DST_IN_CALLER_OFFSET as i32;
const CALL_FRAME_SCALAR_ROOTS_MARK_OFFSET: i32 = vm::CALL_FRAME_SCALAR_ROOTS_MARK_OFFSET as i32;
const CALL_FRAME_COMPILED_OFFSET: i32 = vm::CALL_FRAME_COMPILED_OFFSET as i32;
const CALL_FRAME_SIZE: i64 = vm::CALL_FRAME_SIZE as i64;

/// What compiled code returns when it is leaving through an error or a
/// deoptimization, matching the baseline tier; the caller looks at the
/// pending error and deopt fields rather than the value.
const PENDING_RETURN: u64 = value::QNAN;

/// The runtime helper that runs `instr` for the generic path, if there
/// is one. The builder refuses a function containing an instruction it
/// would have to run generically without one.
pub fn generic_helper(instr: &Instr) -> Option<&'static str> {
  Some(match instr {
    Instr::Add { .. } => "zuri_jit_add_slow",
    Instr::Sub { .. } => "zuri_jit_sub_slow",
    Instr::Mul { .. } => "zuri_jit_mul_slow",
    Instr::Div { .. } => "zuri_jit_div_slow",
    Instr::Pow { .. } => "zuri_jit_pow",
    Instr::Floor { .. } => "zuri_jit_floordiv",
    Instr::Mod { .. } => "zuri_jit_mod",
    Instr::BitAnd { .. } => "zuri_jit_bitand_slow",
    Instr::BitOr { .. } => "zuri_jit_bitor_slow",
    Instr::BitXor { .. } => "zuri_jit_bitxor_slow",
    Instr::BitShl { .. } => "zuri_jit_bitshl",
    Instr::BitShr { .. } => "zuri_jit_bitshr",
    Instr::BitUshr { .. } => "zuri_jit_bitushr",
    Instr::Neg { .. } => "zuri_jit_neg_slow",
    Instr::BitNot { .. } => "zuri_jit_bitnot_slow",
    Instr::Concat { .. } => "zuri_jit_concat",
    Instr::Eq { .. } => "zuri_jit_eq_slow",
    Instr::Neq { .. } => "zuri_jit_neq_slow",
    Instr::Lt { .. } => "zuri_jit_lt_slow",
    Instr::Le { .. } => "zuri_jit_le_slow",
    Instr::Gt { .. } => "zuri_jit_gt_slow",
    Instr::Ge { .. } => "zuri_jit_ge_slow",
    Instr::AddImm { .. } => "zuri_jit_addimm_slow",
    Instr::SubImm { .. } => "zuri_jit_subimm_slow",
    Instr::MulImm { .. } => "zuri_jit_mulimm_slow",
    Instr::LtImm { .. } => "zuri_jit_ltimm_slow",
    Instr::LeImm { .. } => "zuri_jit_leimm_slow",
    Instr::GtImm { .. } => "zuri_jit_gtimm_slow",
    Instr::GeImm { .. } => "zuri_jit_geimm_slow",
    Instr::Call { .. } => "zuri_jit_call",
    Instr::Invoke { .. } => "zuri_jit_invoke",
    Instr::Print { .. } => "zuri_jit_print",
    Instr::GetGlobal { .. } => "zuri_jit_get_global",
    Instr::SetGlobal { .. } => "zuri_jit_set_global",
    Instr::AssignGlobal { .. } => "zuri_jit_assign_global",
    Instr::MakeList { .. } => "zuri_jit_make_list",
    Instr::MakeDict { .. } => "zuri_jit_make_dict",
    Instr::GetField { .. } => "zuri_jit_get_field",
    Instr::SetField { .. } => "zuri_jit_set_field",
    Instr::GetIndex { .. } => "zuri_jit_get_index",
    Instr::SetIndex { .. } => "zuri_jit_set_index",
    Instr::GetSlice { .. } => "zuri_jit_get_slice",
    Instr::MakeRange { .. } => "zuri_jit_make_range",
    Instr::CheckParamType { .. } => "zuri_jit_check_param_type",
    Instr::Closure { .. } => "zuri_jit_make_closure",
    Instr::GetUpval { .. } => "zuri_jit_get_upval",
    Instr::SetUpval { .. } => "zuri_jit_set_upval",
    Instr::CloseUpvalues { .. } => "zuri_jit_close_upvalues",
    Instr::MakeClass { .. } => "zuri_jit_make_class",
    Instr::DeclareField { .. } => "zuri_jit_declare_field",
    Instr::SetFieldInit { .. } => "zuri_jit_set_field_init",
    Instr::SetMethod { .. } => "zuri_jit_set_method",
    Instr::DeclareStatic { .. } => "zuri_jit_declare_static",
    Instr::FinalizeClass { .. } => "zuri_jit_finalize_class",
    Instr::InvokeSuper { .. } => "zuri_jit_invoke_super",
    Instr::CallSuperCtor { .. } => "zuri_jit_call_super_ctor",
    Instr::Import { .. } => "zuri_jit_import",
    Instr::ImportAll { .. } => "zuri_jit_import_all",
    Instr::MakePromoted { .. } => "zuri_jit_make_promoted",
    _ => return None,
  })
}

fn cl_type(ty: Ty) -> cranelift_codegen::ir::Type {
  match ty {
    Ty::Tagged | Ty::I64 | Ty::Ptr => types::I64,
    Ty::F64 => types::F64,
    Ty::Bool => types::I8,
  }
}

fn float_cc(c: Cmp) -> FloatCC {
  match c {
    Cmp::Eq => FloatCC::Equal,
    Cmp::Ne => FloatCC::NotEqual,
    Cmp::Lt => FloatCC::LessThan,
    Cmp::Le => FloatCC::LessThanOrEqual,
    Cmp::Gt => FloatCC::GreaterThan,
    Cmp::Ge => FloatCC::GreaterThanOrEqual,
  }
}

fn int_cc(c: Cmp) -> IntCC {
  match c {
    Cmp::Eq => IntCC::Equal,
    Cmp::Ne => IntCC::NotEqual,
    Cmp::Lt => IntCC::SignedLessThan,
    Cmp::Le => IntCC::SignedLessThanOrEqual,
    Cmp::Gt => IntCC::SignedGreaterThan,
    Cmp::Ge => IntCC::SignedGreaterThanOrEqual,
  }
}

/// Lowers `ir` into `fb`, returning the on-stack-replacement ids its
/// entry accepts.
pub fn lower(
  fb: &mut FunctionBuilder,
  module: &mut JITModule,
  helpers: &FxHashMap<&'static str, FuncId>,
  proto: &ObjFunction,
  ir: &Func,
) -> Result<FxHashMap<usize, i32>, String> {
  let mut l = Lowering {
    fb,
    module,
    helpers,
    proto,
    ir,
    vm: IrValue::from_u32(0),
    base_bytes: IrValue::from_u32(0),
    base: IrValue::from_u32(0),
    fast_args: [IrValue::from_u32(0); 4],
    regs_var: Variable::from_u32(0),
    blocks: Vec::new(),
    values: vec![None; ir.values.len()],
    armed: crate::modules::os_util::signal::armed(),
    closure_slot: None,
    entry_sig: None,
    osr_param: IrValue::from_u32(0),
    deopt_chains: FxHashMap::default(),
    leave_marker: None,
    positions: FxHashMap::default(),
    small: super::passes::ints::small_integers(ir),
  };
  l.run()?;
  Ok(ir.osr_ids.clone())
}

struct Lowering<'a, 'b> {
  fb: &'a mut FunctionBuilder<'b>,
  module: &'a mut JITModule,
  helpers: &'a FxHashMap<&'static str, FuncId>,
  proto: &'a ObjFunction,
  ir: &'a Func,
  vm: IrValue,
  base: IrValue,
  base_bytes: IrValue,
  fast_args: [IrValue; 4],
  regs_var: Variable,
  blocks: Vec<Block>,
  values: Vec<Option<IrValue>>,
  armed: bool,
  /// Where a call's prepare helper leaves the callee closure.
  closure_slot: Option<StackSlot>,
  /// The compiled entry signature, for calling a callee directly.
  entry_sig: Option<SigRef>,
  /// The entry's on-stack-replacement id argument.
  osr_param: IrValue,
  /// For each frame built in, the interpreter frames a deopt inside it
  /// pushes, outermost first, as the address and length of a table that
  /// lives as long as the code.
  deopt_chains: FxHashMap<u16, (u64, u64)>,
  /// The helper that says why the block's deopt is leaving, set by the
  /// `Unreached` it starts with and called once the registers are out.
  leave_marker: Option<&'static str>,
  /// The encoded position published for an instruction of a frame built
  /// in, by frame and position.
  positions: FxHashMap<(u16, usize), usize>,
  /// `I64` values known to lie within 2^53 of zero.
  small: rustc_hash::FxHashSet<ValueId>,
}

impl<'a, 'b> Lowering<'a, 'b> {
  fn run(&mut self) -> Result<(), String> {
    let entry = self.fb.create_block();
    self.fb.append_block_params_for_function_params(entry);
    self.fb.switch_to_block(entry);
    let params = self.fb.block_params(entry).to_vec();
    self.vm = params[0];
    self.base = params[1];
    let osr_param = params[3];
    self.fast_args = [params[4], params[5], params[6], params[7]];

    self.regs_var = self.fb.declare_var(types::I64);
    self.refresh_regs();
    self.base_bytes = self.fb.ins().imul_imm_s(self.base, 8);
    if self.ir.extent > self.proto.num_registers as usize {
      self.ensure_registers();
    }

    // One Cranelift block per IR block, parameters typed to match.
    for b in &self.ir.blocks {
      let cb = self.fb.create_block();
      for &p in &b.params {
        self.fb.append_block_param(cb, cl_type(self.ir.ty(p)));
      }
      self.blocks.push(cb);
    }
    for (i, b) in self.ir.blocks.iter().enumerate() {
      let params = self.fb.block_params(self.blocks[i]).to_vec();
      for (&p, &cp) in b.params.iter().zip(&params) {
        self.values[p.0 as usize] = Some(cp);
      }
    }

    // The IR's prologue does the dispatching.
    self.osr_param = osr_param;
    self
      .fb
      .ins()
      .jump(self.blocks[self.ir.entry.0 as usize], &[]);

    for b in self.ir.reverse_postorder() {
      self.lower_block(b)?;
    }
    self.fb.seal_all_blocks();
    Ok(())
  }

  fn lower_block(&mut self, b: BlockId) -> Result<(), String> {
    self.fb.switch_to_block(self.blocks[b.0 as usize]);
    let insts = self.ir.block(b).insts.clone();
    let mut i = 0;
    while i < insts.len() {
      let id = insts[i];
      if matches!(self.ir.inst(id).op, Op::Safepoint | Op::NewList { .. }) {
        // The reloads that follow a safepoint only have to read memory
        // when it actually collected.
        let mut reloads = Vec::new();
        let mut j = i + 1;
        while j < insts.len() && matches!(self.ir.inst(insts[j]).op, Op::Reload { .. }) {
          reloads.push(insts[j]);
          j += 1;
        }
        if matches!(self.ir.inst(id).op, Op::Safepoint) {
          self.lower_safepoint(id, &reloads);
        } else {
          self.lower_new_list(id, &reloads)?;
        }
        i = j;
        continue;
      }
      self.lower_inst(id)?;
      i += 1;
    }
    self.lower_term(b);
    Ok(())
  }

  fn v(&self, v: ValueId) -> IrValue {
    self.values[v.0 as usize].unwrap_or_else(|| panic!("v{} used before it was lowered", v.0))
  }

  fn def(&mut self, v: Option<ValueId>, x: IrValue) {
    if let Some(v) = v {
      self.values[v.0 as usize] = Some(x);
    }
  }

  fn lower_inst(&mut self, id: InstId) -> Result<(), String> {
    let inst = self.ir.inst(id).clone();
    let av: Vec<IrValue> = inst
      .args
      .iter()
      .map(|&x| self.values[x.0 as usize].unwrap_or(IrValue::from_u32(0)))
      .collect();
    let flags = MemFlagsData::trusted();
    let out: Option<IrValue> = match &inst.op {
      Op::ConstTagged(bits) => Some(self.u64c(*bits)),
      Op::ConstF64(x) => Some(self.fb.ins().f64const(*x)),
      Op::ConstI64(x) => Some(self.fb.ins().iconst(types::I64, *x)),
      Op::ConstBool(b) => Some(self.fb.ins().iconst(types::I8, *b as i64)),
      Op::Param(r) => {
        let r = *r;
        if (r as usize) < 4 && (r as usize) < self.proto.arity as usize {
          Some(self.fast_args[r as usize])
        } else {
          Some(self.load_reg(r))
        }
      },
      Op::Reload { reg } | Op::OsrParam(reg) => Some(self.load_reg(*reg)),
      Op::OsrIndex => Some(self.fb.ins().sextend(types::I64, self.osr_param)),
      Op::MethodGeneration => Some(self.fb.ins().load(
        types::I64,
        flags,
        self.vm,
        METHOD_TABLE_GENERATION_OFFSET,
      )),
      Op::LoadGlobal(slot) => {
        let slots = self
          .fb
          .ins()
          .load(types::I64, flags, self.vm, GLOBAL_SLOTS_PTR_CACHE_OFFSET);
        Some(
          self
            .fb
            .ins()
            .load(types::I64, flags, slots, (*slot as i32) * 8),
        )
      },
      Op::StoreGlobal(slot) => {
        let v = self.tagged_of(inst.args[0]);
        let slots = self
          .fb
          .ins()
          .load(types::I64, flags, self.vm, GLOBAL_SLOTS_PTR_CACHE_OFFSET);
        self.fb.ins().store(flags, v, slots, (*slot as i32) * 8);
        None
      },
      Op::TaggedEq => Some(self.tagged_eq(av[0], av[1], inst.state.as_ref().unwrap())),
      Op::FPow => {
        let x = self.from_f64(av[0]);
        let y = self.from_f64(av[1]);
        let bits = self.call_pure("zuri_jit_num_powf", &[self.vm, x, y]);
        Some(self.to_f64(bits))
      },
      Op::FUnary(u) => Some(self.funary(*u, av[0])),
      Op::FMax => Some(self.fpick(true, av[0], av[1])),
      Op::FMin => Some(self.fpick(false, av[0], av[1])),
      Op::FTest(t) => Some(self.ftest(*t, av[0])),
      Op::FCall(helper) => {
        let mut args = vec![self.vm];
        for &a in &av {
          args.push(self.from_f64(a));
        }
        let bits = self.call_pure(helper, &args);
        Some(self.to_f64(bits))
      },
      Op::BoxF64 => Some(self.from_f64(av[0])),
      Op::BoxBool => Some(self.box_bool(av[0])),
      Op::IntToF64 => Some(self.fb.ins().fcvt_from_sint(types::F64, av[0])),
      Op::F64ToI64 => Some(self.fb.ins().fcvt_to_sint_sat(types::I64, av[0])),
      Op::UnboxF64 => Some(self.to_f64(av[0])),
      Op::UnboxBool => {
        let t = self.u64c(value::TRUE_VAL);
        Some(self.fb.ins().icmp(IntCC::Equal, av[0], t))
      },
      Op::ObjPtr => Some(self.obj_ptr(av[0])),
      Op::Guard(kind) => self.lower_guard(*kind, &inst.args, inst.state.as_ref().unwrap()),
      Op::FAdd => Some(self.fb.ins().fadd(av[0], av[1])),
      Op::FSub => Some(self.fb.ins().fsub(av[0], av[1])),
      Op::FMul => Some(self.fb.ins().fmul(av[0], av[1])),
      Op::FDiv => Some(self.fb.ins().fdiv(av[0], av[1])),
      Op::FNeg => Some(self.fb.ins().fneg(av[0])),
      Op::FMod => Some(match self.whole_constant(inst.args[1]) {
        Some(d) => self.fmod_by(av[0], av[1], d),
        None => self.fmod(av[0], av[1]),
      }),
      Op::FFloorDiv => {
        let q = self.fb.ins().fdiv(av[0], av[1]);
        Some(self.fb.ins().floor(q))
      },
      Op::FCmp(c) => Some(self.fb.ins().fcmp(float_cc(*c), av[0], av[1])),
      Op::IAdd => Some(self.fb.ins().iadd(av[0], av[1])),
      Op::ISub => Some(self.fb.ins().isub(av[0], av[1])),
      Op::IMul => Some(self.fb.ins().imul(av[0], av[1])),
      Op::IBit(bit) => Some(self.ibit(*bit, av[0], av[1])),
      Op::WrapI64 => Some(self.wrap_i64(av[0])),
      Op::Unsaturate => {
        let saturated = self.fb.ins().icmp_imm_s(IntCC::Equal, av[0], i64::MAX);
        let min = self.fb.ins().iconst(types::I64, i64::MIN);
        Some(self.fb.ins().select(saturated, min, av[0]))
      },
      Op::ICmp(c) => Some(self.fb.ins().icmp(int_cc(*c), av[0], av[1])),
      Op::IsFalsey => Some(self.is_falsey(av[0])),
      Op::BNot => Some(self.fb.ins().bxor_imm_u(av[0], 1)),
      Op::EqConst(k) => {
        let v = av[0];
        let is_num = self.is_number(v);
        let f = self.to_f64(v);
        let kc = self.fb.ins().f64const(*k);
        let eq = self.fb.ins().fcmp(FloatCC::Equal, f, kc);
        Some(self.fb.ins().band(is_num, eq))
      },
      Op::ListLen => {
        let len32 = self
          .fb
          .ins()
          .load(types::I32, flags, av[0], object::obj_list_len_offset());
        Some(self.fb.ins().uextend(types::I64, len32))
      },
      Op::ListData => Some(self.list_data(av[0])),
      Op::LoadElem => {
        let off = self.fb.ins().imul_imm_s(av[1], 8);
        let addr = self.fb.ins().iadd(av[0], off);
        Some(self.fb.ins().load(types::I64, flags, addr, 0))
      },
      Op::ListEnd { last } => Some(self.list_end(av[0], *last)),
      Op::ListAppend => {
        self.list_append(av[0], inst.args[1]);
        None
      },
      Op::StoreElem => {
        let list = av[0];
        let off = self.fb.ins().imul_imm_s(av[2], 8);
        let addr = self.fb.ins().iadd(av[1], off);
        let val = self.tagged_of(inst.args[3]);
        self.fb.ins().store(flags, val, addr, 0);
        self.barrier_for_store(inst.args[3], val, list);
        None
      },
      Op::LoadField(slot) => {
        let fields = self.fields_ptr(av[0]);
        Some(
          self
            .fb
            .ins()
            .load(types::I64, flags, fields, (*slot as i32) * 8),
        )
      },
      Op::StoreField(slot) => {
        let obj = av[0];
        let fields = self.fields_ptr(obj);
        let val = self.tagged_of(inst.args[1]);
        self.fb.ins().store(flags, val, fields, (*slot as i32) * 8);
        self.barrier_for_store(inst.args[1], val, obj);
        None
      },
      Op::StoreReg(r) => {
        let v = self.tagged_of(inst.args[0]);
        self.store_reg(*r, v);
        None
      },
      Op::UpvalCell(n) => Some(self.upval_cell(*n, inst.state.as_ref().unwrap())),
      Op::LoadUpval => Some(self.load_upval(av[0])),
      Op::StoreUpval => {
        let val = self.tagged_of(inst.args[1]);
        self.store_upval(inst.args[1], av[0], val);
        None
      },
      Op::UsingTarget { table, reg, frame } => {
        let subject = self.tagged_of(inst.args[0]);
        self.store_reg(*reg, subject);
        let (proto, offset) = self.frame(*frame);
        let func_ptr = self.u64c(proto as *const ObjFunction as u64);
        let base = self.frame_base(*frame);
        let reg_c = self.u64c((*reg - offset) as u64);
        let table_c = self.u64c(*table as u64);
        Some(self.call_pure(
          "zuri_jit_using_jump",
          &[self.vm, base, reg_c, func_ptr, table_c],
        ))
      },
      Op::Unreached => {
        self.leave_marker = Some("zuri_jit_unreached");
        None
      },
      Op::Generic { instr, ip, frame } => {
        self.lower_generic(*instr, *ip, *frame, inst.state.as_ref().unwrap())?;
        None
      },
      Op::Safepoint | Op::NewList { .. } => {
        unreachable!("safepoints and allocations are lowered with their reloads")
      },
      Op::StrAscii => Some(self.str_ascii(av[0])),
      Op::StrByteLen => Some(self.fb.ins().load(
        types::I64,
        flags,
        av[0],
        object::obj_str_len_offset(),
      )),
      Op::StrByte => {
        let data = self
          .fb
          .ins()
          .load(types::I64, flags, av[0], object::obj_str_ptr_offset());
        let addr = self.fb.ins().iadd(data, av[1]);
        let b = self.fb.ins().load(types::I8, flags, addr, 0);
        Some(self.fb.ins().uextend(types::I64, b))
      },
      Op::AsciiChar => {
        let off = self.fb.ins().imul_imm_s(av[0], 8);
        let table = self
          .fb
          .ins()
          .iadd_imm_s(self.vm, INTERNED_ASCII_OFFSET as i64);
        let addr = self.fb.ins().iadd(table, off);
        Some(self.fb.ins().load(types::I64, flags, addr, 0))
      },
      Op::StrLength => Some(self.str_length(av[0])),
      Op::StrOrd => Some(self.str_ord(av[0])),
      Op::WrapIndex => {
        let zero = self.fb.ins().iconst(types::I64, 0);
        let negative = self.fb.ins().icmp(IntCC::SignedLessThan, av[0], zero);
        let from_end = self.fb.ins().iadd(av[0], av[1]);
        Some(self.fb.ins().select(negative, from_end, av[0]))
      },
      Op::GetIndex => {
        let zero = self.fb.ins().f64const(0.0);
        let usable = self.fb.ins().fcmp(FloatCC::GreaterThanOrEqual, av[0], zero);
        let whole = self.fb.ins().fcvt_to_sint_sat(types::I64, av[0]);
        let none = self.fb.ins().iconst(types::I64, -1);
        Some(self.fb.ins().select(usable, whole, none))
      },
      Op::BytesLen => Some(self.fb.ins().load(
        types::I64,
        flags,
        av[0],
        object::obj_bytes_len_offset(),
      )),
      Op::BytesLoad => {
        let data = self
          .fb
          .ins()
          .load(types::I64, flags, av[0], object::obj_bytes_ptr_offset());
        let addr = self.fb.ins().iadd(data, av[1]);
        let b = self.fb.ins().load(types::I8, flags, addr, 0);
        Some(self.fb.ins().uextend(types::I64, b))
      },
      Op::BytesStore => {
        let data = self
          .fb
          .ins()
          .load(types::I64, flags, av[0], object::obj_bytes_ptr_offset());
        let addr = self.fb.ins().iadd(data, av[1]);
        let b = self.fb.ins().ireduce(types::I8, av[2]);
        self.fb.ins().store(flags, b, addr, 0);
        None
      },
      Op::ListGetOr => Some(self.list_get_or(av[0], av[1], av[2])),
      Op::DictGet => {
        let (d, k, fallback) = (
          self.tagged_of(inst.args[0]),
          self.tagged_of(inst.args[1]),
          self.tagged_of(inst.args[2]),
        );
        Some(self.call_pure("zuri_jit_dict_get", &[self.vm, d, k, fallback]))
      },
      Op::DictSet => {
        let (d, k, v) = (
          self.tagged_of(inst.args[0]),
          self.tagged_of(inst.args[1]),
          self.tagged_of(inst.args[2]),
        );
        self.call_pure("zuri_jit_dict_set", &[self.vm, d, k, v]);
        None
      },
      Op::DictContains => {
        let (d, k) = (self.tagged_of(inst.args[0]), self.tagged_of(inst.args[1]));
        let answer = self.call_pure("zuri_jit_dict_contains", &[self.vm, d, k]);
        let t = self.u64c(value::TRUE_VAL);
        Some(self.fb.ins().icmp(IntCC::Equal, answer, t))
      },
      Op::DictLen => {
        let d = self.tagged_of(inst.args[0]);
        Some(self.call_pure("zuri_jit_dict_len", &[self.vm, d]))
      },
      Op::ObjLength => Some(self.obj_length(av[0])),
      Op::IsBits(bits) => {
        let want = self.u64c(*bits);
        Some(self.fb.ins().icmp(IntCC::Equal, av[0], want))
      },
      Op::NextKey => Some(self.next_key(av[0], av[1])),
      Op::RangeCount => {
        let (_, _, _, count) = self.range_shape(av[0]);
        Some(self.fb.ins().fcvt_to_sint_sat(types::I64, count))
      },
      Op::RangeAt => Some(self.range_at(av[0], av[1])),
    };
    if let Some(x) = out {
      self.def(inst.result, x);
    }
    Ok(())
  }

  fn lower_guard(
    &mut self,
    kind: GuardKind,
    args: &[ValueId],
    state: &FrameState,
  ) -> Option<IrValue> {
    let x = self.v(args[0]);
    let (ok, result) = match kind {
      GuardKind::Number => {
        let ok = self.is_number(x);
        (ok, Some(self.to_f64(x)))
      },
      GuardKind::Int => {
        let (f, is_num) = if self.ir.ty(args[0]) == Ty::F64 {
          (x, None)
        } else {
          (self.to_f64(x), Some(self.is_number(x)))
        };
        let (i, exact) = self.f64_to_int(f);
        let ok = match is_num {
          Some(n) => self.fb.ins().band(n, exact),
          None => exact,
        };
        (ok, Some(i))
      },
      GuardKind::Whole => {
        let (f, is_num) = if self.ir.ty(args[0]) == Ty::F64 {
          (x, None)
        } else {
          (self.to_f64(x), Some(self.is_number(x)))
        };
        let (i, exact) = self.f64_to_int(f);
        let bits = self.from_f64(f);
        let minus_zero = self.fb.ins().icmp_imm_s(IntCC::Equal, bits, i64::MIN);
        let ok = self.fb.ins().band_not(exact, minus_zero);
        let ok = match is_num {
          Some(n) => self.fb.ins().band(n, ok),
          None => ok,
        };
        (ok, Some(i))
      },
      GuardKind::Arith(op) => {
        let y = self.v(args[1]);
        // A sum or difference with an operand within 2^53 of zero that
        // wraps lands further than that from zero, so the range check
        // catches the overflow on its own.
        let one_small = self.small.contains(&args[0]) || self.small.contains(&args[1]);
        let (r, overflow) = match op {
          IntOp::Add if one_small => (self.fb.ins().iadd(x, y), None),
          IntOp::Sub if one_small => (self.fb.ins().isub(x, y), None),
          IntOp::Add => {
            let (r, o) = self.fb.ins().sadd_overflow(x, y);
            (r, Some(o))
          },
          IntOp::Sub => {
            let (r, o) = self.fb.ins().ssub_overflow(x, y);
            (r, Some(o))
          },
          IntOp::Mul => {
            let (r, o) = self.fb.ins().smul_overflow(x, y);
            (r, Some(o))
          },
        };
        // Within `-2^53..2^53` exactly when the bits above the 53rd are
        // all zeros or all ones: shifted down, -1 or 0.
        let high = self.fb.ins().sshr_imm_u(r, 53);
        let high = self.fb.ins().iadd_imm_s(high, 1);
        let in_range = self.fb.ins().icmp_imm_u(IntCC::UnsignedLessThan, high, 2);
        let mut ok = match overflow {
          Some(o) => self.fb.ins().band_not(in_range, o),
          None => in_range,
        };
        if op == IntOp::Mul {
          let signs = self.fb.ins().bor(x, y);
          let negative = self.fb.ins().icmp_imm_s(IntCC::SignedLessThan, signs, 0);
          let zero = self.fb.ins().icmp_imm_s(IntCC::Equal, r, 0);
          let minus_zero = self.fb.ins().band(negative, zero);
          ok = self.fb.ins().band_not(ok, minus_zero);
        }
        (ok, Some(r))
      },
      GuardKind::Bool => {
        let t = self.u64c(value::TRUE_VAL);
        let f = self.u64c(value::FALSE_VAL);
        let is_t = self.fb.ins().icmp(IntCC::Equal, x, t);
        let is_f = self.fb.ins().icmp(IntCC::Equal, x, f);
        (self.fb.ins().bor(is_t, is_f), Some(is_t))
      },
      GuardKind::List => {
        let ok = self.obj_tag_is(x, object::OBJ_TAG_LIST);
        (ok, Some(self.obj_ptr(x)))
      },
      GuardKind::Instance(class) => {
        let ok = self.instance_of(x, class);
        (ok, Some(self.obj_ptr(x)))
      },
      GuardKind::Family { class, depth } => {
        let ok = self.instance_in_family(x, class, depth);
        (ok, Some(self.obj_ptr(x)))
      },
      GuardKind::Tag(tag) => {
        let ok = self.obj_tag_is(x, tag);
        (ok, Some(self.obj_ptr(x)))
      },
      GuardKind::Bounds => {
        let len = self.v(args[1]);
        (self.fb.ins().icmp(IntCC::UnsignedLessThan, x, len), None)
      },
      GuardKind::True => (x, None),
      GuardKind::Param { frame, check } => {
        let (proto, _) = self.frame(frame);
        (self.param_check(x, proto, check), None)
      },
      GuardKind::Proto(bits) => (self.closure_of(x, bits), None),
      GuardKind::Elems { whole } => {
        let w = self.u64c(whole as u64);
        let r = self.call_pure("zuri_jit_list_elems_are", &[self.vm, x, w]);
        (self.fb.ins().ireduce(types::I8, r), None)
      },
    };
    let fail = self.fb.create_block();
    let cont = self.fb.create_block();
    self.fb.ins().brif(ok, cont, &[], fail, &[]);
    self.fb.switch_to_block(fail);
    self.fb.set_cold_block(fail);
    let marker =
      matches!(kind, GuardKind::Whole | GuardKind::Arith(_)).then_some("zuri_jit_int_miss");
    self.deopt(state, marker);
    self.fb.switch_to_block(cont);
    result
  }

  /// Runs one instruction through its runtime helper: publish the
  /// position, write the live registers out, call, and leave through the
  /// error path if the helper raised.
  fn lower_generic(
    &mut self,
    instr: Instr,
    ip: usize,
    frame: u16,
    state: &FrameState,
  ) -> Result<(), String> {
    let name = generic_helper(&instr).ok_or_else(|| format!("no runtime helper for {instr:?}"))?;
    self.publish_ip(ip, frame);
    self.flush(state);
    let (proto, offset) = self.frame(frame);
    let base = self.frame_base(frame);
    let args = self.generic_args(instr, ip, proto, base);
    let status = match instr {
      Instr::Call {
        dst,
        func,
        num_args,
      } => {
        let prepare = vec![
          self.vm,
          base,
          self.u64c(func as u64),
          self.u64c(num_args as u64),
          self.u64c(dst as u64),
        ];
        let call = FastCall {
          base,
          offset,
          first_arg: func + 1,
          num_args,
          dst,
        };
        if num_args > 4 {
          self.fast_call("zuri_jit_call_prepare", prepare, call, name, &args)
        } else {
          // A callee named by the feedback whose shape takes this call
          // needs only to be recognised.
          let callee = match self.ir.known_calls.get(&(frame, ip)) {
            Some(&(function, bits)) => {
              // SAFETY: prototypes live in the old generation and never
              // move, and this one is live while the compile runs.
              let f = unsafe { &*(function as *const ObjFunction) };
              if f.arity == num_args && !f.variadic && !f.is_method {
                DirectCallee::Known { function, bits }
              } else {
                DirectCallee::Register
              }
            },
            None => DirectCallee::Register,
          };
          let (miss, done) = self.direct_call(callee, call, ip, frame);
          self.fb.switch_to_block(miss);
          let status = self.fast_call("zuri_jit_call_prepare", prepare, call, name, &args);
          self.fb.ins().jump(done, &[status.into()]);
          self.fb.switch_to_block(done);
          self.fb.block_params(done)[0]
        }
      },
      Instr::Invoke {
        dst,
        obj,
        method_const,
        num_args,
      } => {
        let name_bits = proto.chunk.constants[method_const as usize].to_bits();
        let func_ptr = proto as *const ObjFunction as u64;
        let prepare = vec![
          self.vm,
          base,
          self.u64c(obj as u64),
          self.u64c(num_args as u64),
          self.u64c(dst as u64),
          self.u64c(name_bits),
          self.u64c(func_ptr),
          self.u64c(ip as u64),
        ];
        // The receiver is the callee's first argument.
        let call = FastCall {
          base,
          offset,
          first_arg: obj + 1,
          num_args: num_args + 1,
          dst,
        };
        match proto.chunk.invoke_cache_cell(ip) {
          Some(cell) if num_args < 4 => {
            let cell = cell as *const _ as u64;
            let (miss, done) = self.direct_call(DirectCallee::Cached(cell), call, ip, frame);
            self.fb.switch_to_block(miss);
            let status = self.fast_call("zuri_jit_invoke_prepare", prepare, call, name, &args);
            self.fb.ins().jump(done, &[status.into()]);
            self.fb.switch_to_block(done);
            self.fb.block_params(done)[0]
          },
          _ => self.fast_call("zuri_jit_invoke_prepare", prepare, call, name, &args),
        }
      },
      _ => self.call(name, &args),
    };
    self.leave_on_error(status);
    self.refresh_regs();
    Ok(())
  }

  /// The function running in `frame` and where its register 0 sits.
  fn frame(&self, frame: u16) -> (&'a ObjFunction, u8) {
    let f = &self.ir.frames[frame as usize];
    if frame == 0 {
      return (self.proto, 0);
    }
    // SAFETY: the VM holds every function built in until the compile
    // finishes, and the code keeps each one reachable while it runs:
    // through the callee register a frame state holds, or through the
    // class a method belongs to.
    (unsafe { &*(f.proto as *const ObjFunction) }, f.offset)
  }

  /// `frame`'s register window, as a register index.
  fn frame_base(&mut self, frame: u16) -> IrValue {
    let offset = self.ir.frames[frame as usize].offset;
    if offset == 0 {
      self.base
    } else {
      self.fb.ins().iadd_imm_s(self.base, offset as i64)
    }
  }

  /// Grows the register file, when it has to, to hold every frame built
  /// into this function, which reach past the function's own registers.
  fn ensure_registers(&mut self) {
    let flags = MemFlagsData::trusted();
    let len = self
      .fb
      .ins()
      .load(types::I64, flags, self.vm, REGS_LEN_CACHE_OFFSET);
    let needed = self.fb.ins().iadd_imm_s(self.base, self.ir.extent as i64);
    let short = self.fb.ins().icmp(IntCC::UnsignedLessThan, len, needed);
    let grow = self.fb.create_block();
    let done = self.fb.create_block();
    self.fb.ins().brif(short, grow, &[], done, &[]);
    self.fb.switch_to_block(grow);
    self.fb.set_cold_block(grow);
    self.call("zuri_jit_ensure_registers", &[self.vm, needed]);
    self.refresh_regs();
    self.fb.ins().jump(done, &[]);
    self.fb.switch_to_block(done);
  }

  /// A call that goes straight into the callee's compiled code when it
  /// has some. The prepare helper pushes the callee's frame and hands
  /// back its entry, or zero when the callee is not compiled or is not a
  /// plain closure, which leaves the call to the full resolver. Returns
  /// the call's status either way.
  fn fast_call(
    &mut self,
    prepare: &'static str,
    mut prepare_args: Vec<IrValue>,
    call: FastCall,
    slow: &'static str,
    slow_args: &[IrValue],
  ) -> IrValue {
    let FastCall {
      base,
      offset,
      first_arg,
      num_args,
      dst,
    } = call;
    let slot = self.closure_slot();
    let out = self.fb.ins().stack_addr(types::I64, slot, 0);
    prepare_args.push(out);
    let entry = self.call(prepare, &prepare_args);
    self.refresh_regs();

    let fast = self.fb.create_block();
    let slow_block = self.fb.create_block();
    let done = self.fb.create_block();
    self.fb.append_block_param(done, types::I64);
    self.fb.ins().brif(entry, fast, &[], slow_block, &[]);

    self.fb.switch_to_block(fast);
    let closure = self.fb.ins().stack_load(types::I64, types::I64, slot, 0);
    let new_base = self.fb.ins().iadd_imm_s(base, first_arg as i64);
    let nil = self.u64c(value::NIL_VAL);
    let mut call_args = vec![self.vm, new_base, closure];
    call_args.push(self.fb.ins().iconst(types::I32, -1));
    for k in 0..4u8 {
      let a = if k < num_args {
        self.load_reg(offset + first_arg + k)
      } else {
        nil
      };
      call_args.push(a);
    }
    let sig = self.entry_sig();
    let call = self.fb.ins().call_indirect(sig, entry, &call_args);
    let ret = self.fb.inst_results(call)[0];
    self.refresh_regs();
    let dst_c = self.u64c(dst as u64);
    let status = self.call(
      "zuri_jit_call_finish",
      &[self.vm, base, dst_c, new_base, ret],
    );
    self.fb.ins().jump(done, &[status.into()]);

    self.fb.switch_to_block(slow_block);
    let status = self.call(slow, slow_args);
    self.fb.ins().jump(done, &[status.into()]);

    self.fb.switch_to_block(done);
    self.fb.block_params(done)[0]
  }

  /// A call into the callee's compiled code with its frame pushed and
  /// popped right here, the way the baseline tier makes one, so a call
  /// between compiled functions crosses no helper. Returns two blocks:
  /// the first is where every case this does not cover goes (a callee
  /// that is not a plain compiled closure of the right arity, a method
  /// the site's cache does not hold for this receiver's class, a full
  /// frame stack), for the caller to fill with the helper path; the
  /// second joins both, with the call's status as its parameter.
  fn direct_call(
    &mut self,
    callee: DirectCallee,
    call: FastCall,
    ip: usize,
    frame: u16,
  ) -> (Block, Block) {
    let FastCall {
      base,
      offset,
      first_arg,
      num_args,
      dst,
    } = call;
    let flags = MemFlagsData::trusted();
    let miss = self.fb.create_block();
    let done = self.fb.create_block();
    self.fb.append_block_param(done, types::I64);

    let (closure, proto) = match callee {
      DirectCallee::Register => {
        let closure = self.load_reg(offset + first_arg - 1);
        let is_obj = self.is_obj(closure);
        self.continue_if(is_obj, miss);
        let p = self.obj_ptr(closure);
        let tag = self.fb.ins().load(types::I8, flags, p, 0);
        let is_closure =
          self
            .fb
            .ins()
            .icmp_imm_s(IntCC::Equal, tag, object::OBJ_TAG_CLOSURE as i64);
        let other = self.fb.create_block();
        let next = self.fb.create_block();
        self.fb.ins().brif(is_closure, next, &[], other, &[]);

        // A native is called with its arguments where they are.
        self.fb.switch_to_block(other);
        let is_native = self
          .fb
          .ins()
          .icmp_imm_s(IntCC::Equal, tag, object::OBJ_TAG_NATIVE as i64);
        self.continue_if(is_native, miss);
        let func_c = self.u64c((first_arg - 1) as u64);
        let count_c = self.u64c(num_args as u64);
        let dst_c = self.u64c(dst as u64);
        let status = self.call(
          "zuri_jit_call_native",
          &[self.vm, base, func_c, count_c, dst_c],
        );
        self.fb.ins().jump(done, &[status.into()]);

        self.fb.switch_to_block(next);
        let function = self.fb.ins().load(
          types::I64,
          flags,
          p,
          object::obj_closure_function_offset() as i32,
        );
        let is_function = self.obj_tag_is(function, object::OBJ_TAG_FUNC);
        self.continue_if(is_function, miss);
        (closure, self.function_proto(function))
      },
      DirectCallee::Known { function, bits } => {
        let closure = self.load_reg(offset + first_arg - 1);
        let is_closure = self.obj_tag_is(closure, object::OBJ_TAG_CLOSURE);
        self.continue_if(is_closure, miss);
        let p = self.obj_ptr(closure);
        let held = self.fb.ins().load(
          types::I64,
          flags,
          p,
          object::obj_closure_function_offset() as i32,
        );
        let want = self.u64c(bits);
        let same = self.fb.ins().icmp(IntCC::Equal, held, want);
        self.continue_if(same, miss);
        (closure, self.u64c(function as u64))
      },
      DirectCallee::Cached(cell) => {
        // The receiver's class has to be the one the cache was filled
        // for, and then its method is the closure the cache holds.
        let receiver = self.load_reg(offset + first_arg - 1);
        let is_instance = self.obj_tag_is(receiver, object::OBJ_TAG_INSTANCE);
        self.continue_if(is_instance, miss);
        let p = self.obj_ptr(receiver);
        let class = self.fb.ins().load(
          types::I64,
          flags,
          p,
          object::obj_instance_class_offset() as i32,
        );
        let cell = self.u64c(cell);
        let key = self.fb.ins().load(types::I64, flags, cell, 0);
        let hit = self.fb.ins().icmp(IntCC::Equal, class, key);
        self.continue_if(hit, miss);
        let method = self.fb.ins().load(types::I64, flags, cell, 8);
        let p = self.obj_ptr(method);
        let function = self.fb.ins().load(
          types::I64,
          flags,
          p,
          object::obj_closure_function_offset() as i32,
        );
        (method, self.function_proto(function))
      },
    };

    // The shape a frame is pushed for without a helper, and code to run.
    // A known callee's shape was checked while compiling.
    let entry = self.fb.ins().load(
      types::I64,
      flags,
      proto,
      object::obj_function_jit_entry_offset() as i32,
    );
    let mut ready = self.fb.ins().icmp_imm_s(IntCC::NotEqual, entry, 0);
    if !matches!(callee, DirectCallee::Known { .. }) {
      let arity = self.fb.ins().load(
        types::I8,
        flags,
        proto,
        object::obj_function_arity_offset() as i32,
      );
      let fits = self
        .fb
        .ins()
        .icmp_imm_s(IntCC::Equal, arity, num_args as i64);
      ready = self.fb.ins().band(ready, fits);
      let variadic = self.fb.ins().load(
        types::I8,
        flags,
        proto,
        object::obj_function_variadic_offset() as i32,
      );
      let fixed = self.fb.ins().icmp_imm_s(IntCC::Equal, variadic, 0);
      ready = self.fb.ins().band(ready, fixed);
    }
    if matches!(callee, DirectCallee::Register) {
      // A method called as a plain function has no receiver to take.
      let method = self.fb.ins().load(
        types::I8,
        flags,
        proto,
        object::obj_function_is_method_offset() as i32,
      );
      let plain = self.fb.ins().icmp_imm_s(IntCC::Equal, method, 0);
      ready = self.fb.ins().band(ready, plain);
    }
    self.continue_if(ready, miss);

    // Room for one more compiled frame, its registers and its entry on
    // the frame stack.
    let new_base = self.fb.ins().iadd_imm_s(base, first_arg as i64);
    let depth = self
      .fb
      .ins()
      .load(types::I32, flags, self.vm, JIT_CALL_DEPTH_OFFSET);
    let depth_ok = self.fb.ins().icmp_imm_u(
      IntCC::UnsignedLessThan,
      depth,
      vm::JIT_MAX_CALL_DEPTH as i64,
    );
    self.continue_if(depth_ok, miss);
    let registers = self.fb.ins().load(
      types::I8,
      flags,
      proto,
      object::obj_function_num_registers_offset() as i32,
    );
    let registers = self.fb.ins().uextend(types::I64, registers);
    let needed = self.fb.ins().iadd(new_base, registers);
    let regs_len = self
      .fb
      .ins()
      .load(types::I64, flags, self.vm, REGS_LEN_CACHE_OFFSET);
    let regs_ok = self
      .fb
      .ins()
      .icmp(IntCC::UnsignedGreaterThanOrEqual, regs_len, needed);
    self.continue_if(regs_ok, miss);
    let frames_len = self
      .fb
      .ins()
      .load(types::I64, flags, self.vm, FRAMES_LEN_OFFSET);
    let frames_cap = self
      .fb
      .ins()
      .load(types::I64, flags, self.vm, FRAMES_CAP_OFFSET);
    let frames_ok = self
      .fb
      .ins()
      .icmp(IntCC::UnsignedLessThan, frames_len, frames_cap);
    self.continue_if(frames_ok, miss);

    // The callee's frame. This frame stops being the innermost one, so
    // its position goes into its own entry for stack traces to find.
    let frames = self
      .fb
      .ins()
      .load(types::I64, flags, self.vm, FRAMES_PTR_OFFSET);
    let at = self.fb.ins().imul_imm_s(frames_len, CALL_FRAME_SIZE);
    let at = self.fb.ins().iadd(frames, at);
    let position = if frame == 0 {
      ip + 1
    } else {
      self.position(frame, ip)
    };
    let position = self.u64c(position as u64);
    self.fb.ins().store(
      flags,
      position,
      at,
      CALL_FRAME_IP_OFFSET - CALL_FRAME_SIZE as i32,
    );
    self
      .fb
      .ins()
      .store(flags, proto, at, CALL_FRAME_FUNCTION_OFFSET);
    self
      .fb
      .ins()
      .store(flags, closure, at, CALL_FRAME_CLOSURE_VAL_OFFSET);
    let zero = self.u64c(0);
    self.fb.ins().store(flags, zero, at, CALL_FRAME_IP_OFFSET);
    self
      .fb
      .ins()
      .store(flags, new_base, at, CALL_FRAME_BASE_OFFSET);
    let dst_c = self.fb.ins().iconst(types::I8, dst as i64);
    self
      .fb
      .ins()
      .store(flags, dst_c, at, CALL_FRAME_DST_IN_CALLER_OFFSET);
    let roots = self
      .fb
      .ins()
      .load(types::I64, flags, self.vm, JIT_SCALAR_ROOTS_LEN_OFFSET);
    self
      .fb
      .ins()
      .store(flags, roots, at, CALL_FRAME_SCALAR_ROOTS_MARK_OFFSET);
    let yes = self.fb.ins().iconst(types::I8, 1);
    self
      .fb
      .ins()
      .store(flags, yes, at, CALL_FRAME_COMPILED_OFFSET);
    let pushed = self.fb.ins().iadd_imm_s(frames_len, 1);
    self
      .fb
      .ins()
      .store(flags, pushed, self.vm, FRAMES_LEN_OFFSET);
    let deeper = self.fb.ins().iadd_imm_s(depth, 1);
    self
      .fb
      .ins()
      .store(flags, deeper, self.vm, JIT_CALL_DEPTH_OFFSET);

    let nil = self.u64c(value::NIL_VAL);
    let mut call_args = vec![self.vm, new_base, closure];
    call_args.push(self.fb.ins().iconst(types::I32, -1));
    for k in 0..4u8 {
      let a = if k < num_args {
        self.load_reg(offset + first_arg + k)
      } else {
        nil
      };
      call_args.push(a);
    }
    let sig = self.entry_sig();
    let call = self.fb.ins().call_indirect(sig, entry, &call_args);
    let ret = self.fb.inst_results(call)[0];
    self.refresh_regs();
    let depth = self
      .fb
      .ins()
      .load(types::I32, flags, self.vm, JIT_CALL_DEPTH_OFFSET);
    let shallower = self.fb.ins().iadd_imm_s(depth, -1);
    self
      .fb
      .ins()
      .store(flags, shallower, self.vm, JIT_CALL_DEPTH_OFFSET);

    // The callee returned, deoptimized, or is unwinding an error.
    let pending = self.u64c(PENDING_RETURN);
    let is_pending = self.fb.ins().icmp(IntCC::Equal, ret, pending);
    let returned = self.fb.create_block();
    let left = self.fb.create_block();
    self.fb.ins().brif(is_pending, left, &[], returned, &[]);

    self.fb.switch_to_block(left);
    self.fb.set_cold_block(left);
    let deopt_ip = self
      .fb
      .ins()
      .load(types::I64, flags, self.vm, PENDING_DEOPT_IP_OFFSET);
    let raised = self.fb.ins().icmp_imm_s(IntCC::Equal, deopt_ip, -1);
    let deopted = self.fb.create_block();
    let failed = self.fb.create_block();
    self.fb.ins().brif(raised, failed, &[], deopted, &[]);

    self.fb.switch_to_block(deopted);
    self.fb.set_cold_block(deopted);
    let dst_c = self.u64c(dst as u64);
    let status = self.call("zuri_jit_finish_deopt", &[self.vm, base, dst_c]);
    self.fb.ins().jump(done, &[status.into()]);

    self.fb.switch_to_block(failed);
    self.fb.set_cold_block(failed);
    let one = self.u64c(1);
    self.fb.ins().jump(done, &[one.into()]);

    // Popped as the frame's own return would pop it, retiring any scalar
    // roots it registered.
    self.fb.switch_to_block(returned);
    let frames_len = self
      .fb
      .ins()
      .load(types::I64, flags, self.vm, FRAMES_LEN_OFFSET);
    let top = self.fb.ins().iadd_imm_s(frames_len, -1);
    let frames = self
      .fb
      .ins()
      .load(types::I64, flags, self.vm, FRAMES_PTR_OFFSET);
    let at = self.fb.ins().imul_imm_s(top, CALL_FRAME_SIZE);
    let at = self.fb.ins().iadd(frames, at);
    let mark = self
      .fb
      .ins()
      .load(types::I64, flags, at, CALL_FRAME_SCALAR_ROOTS_MARK_OFFSET);
    self
      .fb
      .ins()
      .store(flags, mark, self.vm, JIT_SCALAR_ROOTS_LEN_OFFSET);
    self.fb.ins().store(flags, top, self.vm, FRAMES_LEN_OFFSET);
    self.store_reg(offset + dst, ret);
    let ok = self.u64c(0);
    self.fb.ins().jump(done, &[ok.into()]);

    (miss, done)
  }

  /// Carries on in a fresh block when `cond` holds, and goes to `miss`
  /// otherwise.
  fn continue_if(&mut self, cond: IrValue, miss: Block) {
    let next = self.fb.create_block();
    self.fb.ins().brif(cond, next, &[], miss, &[]);
    self.fb.switch_to_block(next);
  }

  /// The prototype an `Obj::Func` wrapper holds.
  fn function_proto(&mut self, function: IrValue) -> IrValue {
    let p = self.obj_ptr(function);
    self.fb.ins().load(
      types::I64,
      MemFlagsData::trusted(),
      p,
      object::obj_func_proto_offset() as i32,
    )
  }

  fn closure_slot(&mut self) -> StackSlot {
    if let Some(slot) = self.closure_slot {
      return slot;
    }
    let slot =
      self
        .fb
        .create_sized_stack_slot(StackSlotData::new(StackSlotKind::ExplicitSlot, 8, 3));
    self.closure_slot = Some(slot);
    slot
  }

  fn entry_sig(&mut self) -> SigRef {
    if let Some(sig) = self.entry_sig {
      return sig;
    }
    let sig = self.fb.import_signature(entry_signature(self.module));
    self.entry_sig = Some(sig);
    sig
  }

  fn generic_args(
    &mut self,
    instr: Instr,
    ip: usize,
    proto: &ObjFunction,
    base: IrValue,
  ) -> Vec<IrValue> {
    let vm = self.vm;
    let r = |l: &mut Self, x: u64| l.u64c(x);
    let func_ptr = proto as *const ObjFunction as u64;
    let name_bits = |_: &Self, c: u16| proto.chunk.constants[c as usize].to_bits();
    let imm_bits = |_: &Self, c: u16| proto.chunk.constants[c as usize].as_number().to_bits();
    match instr {
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
      | Instr::Concat { dst, a, b }
      | Instr::Eq { dst, a, b }
      | Instr::Neq { dst, a, b }
      | Instr::Lt { dst, a, b }
      | Instr::Le { dst, a, b }
      | Instr::Gt { dst, a, b }
      | Instr::Ge { dst, a, b } => {
        vec![
          vm,
          base,
          r(self, dst as u64),
          r(self, a as u64),
          r(self, b as u64),
        ]
      },
      Instr::Neg { dst, src } | Instr::BitNot { dst, src } => {
        vec![vm, base, r(self, dst as u64), r(self, src as u64)]
      },
      Instr::AddImm { dst, a, imm_const }
      | Instr::SubImm { dst, a, imm_const }
      | Instr::MulImm { dst, a, imm_const }
      | Instr::LtImm { dst, a, imm_const }
      | Instr::LeImm { dst, a, imm_const }
      | Instr::GtImm { dst, a, imm_const }
      | Instr::GeImm { dst, a, imm_const } => {
        let k = imm_bits(self, imm_const);
        vec![vm, base, r(self, dst as u64), r(self, a as u64), r(self, k)]
      },
      Instr::Call {
        dst,
        func,
        num_args,
      } => vec![
        vm,
        base,
        r(self, func as u64),
        r(self, num_args as u64),
        r(self, dst as u64),
      ],
      Instr::Invoke {
        dst,
        obj,
        method_const,
        num_args,
      } => {
        let name = name_bits(self, method_const);
        let cache = proto
          .chunk
          .invoke_cache_cell(ip)
          .map_or(0, |c| c as *const _ as u64);
        vec![
          vm,
          base,
          r(self, obj as u64),
          r(self, num_args as u64),
          r(self, dst as u64),
          r(self, name),
          r(self, cache),
        ]
      },
      Instr::Print { src } => vec![vm, base, r(self, src as u64)],
      Instr::GetGlobal { dst, name_const } => {
        let name = name_bits(self, name_const);
        vec![
          vm,
          base,
          r(self, dst as u64),
          r(self, func_ptr),
          r(self, name),
          r(self, ip as u64),
        ]
      },
      Instr::SetGlobal { name_const, src } | Instr::AssignGlobal { name_const, src } => {
        let name = name_bits(self, name_const);
        vec![
          vm,
          base,
          r(self, src as u64),
          r(self, func_ptr),
          r(self, name),
          r(self, ip as u64),
        ]
      },
      Instr::MakeList { dst, start, count } | Instr::MakeDict { dst, start, count } => vec![
        vm,
        base,
        r(self, dst as u64),
        r(self, start as u64),
        r(self, count as u64),
      ],
      Instr::GetField {
        dst,
        obj,
        name_const,
      } => {
        let name = name_bits(self, name_const);
        vec![
          vm,
          base,
          r(self, dst as u64),
          r(self, obj as u64),
          r(self, name),
          r(self, func_ptr),
          r(self, ip as u64),
        ]
      },
      Instr::SetField {
        obj,
        name_const,
        src,
      } => {
        let name = name_bits(self, name_const);
        vec![
          vm,
          base,
          r(self, obj as u64),
          r(self, name),
          r(self, src as u64),
          r(self, func_ptr),
          r(self, ip as u64),
        ]
      },
      Instr::GetIndex { dst, obj, idx } => vec![
        vm,
        base,
        r(self, dst as u64),
        r(self, obj as u64),
        r(self, idx as u64),
      ],
      Instr::SetIndex { obj, idx, src } => vec![
        vm,
        base,
        r(self, obj as u64),
        r(self, idx as u64),
        r(self, src as u64),
      ],
      Instr::GetSlice { dst, obj, lo, hi } => vec![
        vm,
        base,
        r(self, dst as u64),
        r(self, obj as u64),
        r(self, lo as u64),
        r(self, hi as u64),
      ],
      Instr::MakeRange { dst, lower, upper } => vec![
        vm,
        base,
        r(self, dst as u64),
        r(self, lower as u64),
        r(self, upper as u64),
      ],
      Instr::CheckParamType { reg, check_idx } => vec![
        vm,
        base,
        r(self, reg as u64),
        r(self, func_ptr),
        r(self, check_idx as u64),
        r(self, ip as u64),
      ],
      Instr::Closure { dst, proto_const } => {
        let proto = name_bits(self, proto_const);
        vec![vm, base, r(self, dst as u64), r(self, proto)]
      },
      Instr::GetUpval { dst, idx } => vec![vm, base, r(self, dst as u64), r(self, idx as u64)],
      Instr::SetUpval { idx, src } => vec![vm, base, r(self, src as u64), r(self, idx as u64)],
      Instr::CloseUpvalues { from } => vec![vm, base, r(self, from as u64)],
      Instr::MakeClass {
        dst,
        name_const,
        superclass,
      } => {
        let name = name_bits(self, name_const);
        let (has_super, super_reg) = superclass.map_or((0, 0), |s| (1, s as u64));
        vec![
          vm,
          base,
          r(self, dst as u64),
          r(self, name),
          r(self, has_super),
          r(self, super_reg),
          r(self, func_ptr),
        ]
      },
      Instr::DeclareField { class, name_const } => {
        let name = name_bits(self, name_const);
        vec![vm, base, r(self, class as u64), r(self, name)]
      },
      Instr::SetFieldInit { class, src } => {
        vec![vm, base, r(self, class as u64), r(self, src as u64)]
      },
      Instr::SetMethod {
        class,
        name_const,
        src,
      }
      | Instr::DeclareStatic {
        class,
        name_const,
        src,
      } => {
        let name = name_bits(self, name_const);
        vec![
          vm,
          base,
          r(self, class as u64),
          r(self, name),
          r(self, src as u64),
        ]
      },
      Instr::FinalizeClass { class } => vec![vm, base, r(self, class as u64), r(self, func_ptr)],
      Instr::InvokeSuper {
        dst,
        superclass,
        method_const,
        num_args,
      } => {
        let name = name_bits(self, method_const);
        vec![
          vm,
          base,
          r(self, superclass as u64),
          r(self, num_args as u64),
          r(self, dst as u64),
          r(self, name),
        ]
      },
      Instr::CallSuperCtor {
        dst,
        superclass,
        num_args,
      } => vec![
        vm,
        base,
        r(self, superclass as u64),
        r(self, num_args as u64),
        r(self, dst as u64),
      ],
      Instr::Import {
        dst,
        path_const,
        importer_const,
      } => {
        let path = name_bits(self, path_const);
        let importer = name_bits(self, importer_const);
        vec![
          vm,
          base,
          r(self, dst as u64),
          r(self, path),
          r(self, importer),
        ]
      },
      Instr::ImportAll { module, exported } => vec![
        vm,
        base,
        r(self, module as u64),
        r(self, func_ptr),
        r(self, exported as u64),
      ],
      Instr::MakePromoted {
        dst,
        module,
        name_const,
      } => {
        let name = name_bits(self, name_const);
        vec![
          vm,
          base,
          r(self, dst as u64),
          r(self, module as u64),
          r(self, name),
        ]
      },
      _ => unreachable!("generic_helper() said {instr:?} has a helper"),
    }
  }

  /// A safepoint and the reloads that follow it. The reloaded values
  /// only come from memory on the path that actually called into the
  /// collector; when nothing was owed, the loop carries on with the
  /// values it already had.
  fn lower_safepoint(&mut self, id: InstId, reloads: &[InstId]) {
    let inst = self.ir.inst(id).clone();
    let state = inst.state.as_ref().unwrap();
    let flags = MemFlagsData::trusted();

    let gc = self
      .fb
      .ins()
      .load(types::I8, flags, self.vm, HEAP_JIT_GC_NEEDED_OFFSET);
    let owed = if self.armed {
      let hint = self.u64c(crate::modules::os_util::signal::pending_hint_addr() as u64);
      let sig = self.fb.ins().load(types::I8, flags, hint, 0);
      self.fb.ins().bor(gc, sig)
    } else {
      gc
    };

    let slow = self.fb.create_block();
    let join = self.fb.create_block();
    for &r in reloads {
      let result = self.ir.inst(r).result.unwrap();
      self
        .fb
        .append_block_param(join, cl_type(self.ir.ty(result)));
    }
    let kept: Vec<_> = reloads
      .iter()
      .map(|&r| self.v(self.ir.inst(r).args[0]).into())
      .collect();
    self.fb.ins().brif(owed, slow, &[], join, &kept);

    self.fb.switch_to_block(slow);
    self.fb.set_cold_block(slow);
    self.publish_ip(state.ip, state.frame);
    self.flush(state);
    let status = self.call("zuri_jit_safepoint", &[self.vm]);
    if self.armed {
      self.leave_on_error(status);
    }
    self.refresh_regs();
    let fresh: Vec<_> = reloads
      .iter()
      .map(|&r| {
        let Op::Reload { reg } = self.ir.inst(r).op else {
          unreachable!()
        };
        self.load_reg(reg).into()
      })
      .collect();
    self.fb.ins().jump(join, &fresh);

    self.fb.switch_to_block(join);
    let params = self.fb.block_params(join).to_vec();
    for (&r, p) in reloads.iter().zip(params) {
      let result = self.ir.inst(r).result;
      self.def(result, p);
    }
  }

  fn lower_term(&mut self, b: BlockId) {
    let term = self.ir.block(b).term.clone();
    match term {
      Terminator::Jump { target, args } => {
        let args: Vec<_> = args.iter().map(|&v| self.v(v).into()).collect();
        self.fb.ins().jump(self.blocks[target.0 as usize], &args);
      },
      Terminator::Branch {
        cond,
        then_block,
        then_args,
        else_block,
        else_args,
      } => {
        let c = self.v(cond);
        let ta: Vec<_> = then_args.iter().map(|&v| self.v(v).into()).collect();
        let ea: Vec<_> = else_args.iter().map(|&v| self.v(v).into()).collect();
        self.fb.ins().brif(
          c,
          self.blocks[then_block.0 as usize],
          &ta,
          self.blocks[else_block.0 as usize],
          &ea,
        );
      },
      Terminator::Return(v) => {
        let x = self.tagged_of(v);
        self.fb.ins().return_(&[x]);
      },
      Terminator::Deopt(state) => {
        let marker = self.leave_marker.take();
        self.deopt(&state, marker);
      },
      Terminator::Unset => unreachable!("an unfinished block reached lowering"),
    }
  }

  // --- leaving compiled code ---------------------------------------------

  /// Leaves compiled code for the interpreter at `state`. `marker`, when
  /// given, is the helper that tells the VM why, called only once every
  /// register is written out: the values a call would clobber are then
  /// dead, so the path out holds none of them across one, and nothing on
  /// the fast path has to live in memory for its sake.
  fn deopt(&mut self, state: &FrameState, marker: Option<&'static str>) {
    self.flush(state);
    if let Some(marker) = marker {
      self.call(marker, &[self.vm]);
    }
    if let Some((frame, ip)) = state.blame {
      let (proto, _) = self.frame(frame);
      let proto_c = self.u64c(proto as *const ObjFunction as u64);
      let ip_c = self.u64c(ip as u64);
      self.call("zuri_jit_blame", &[self.vm, proto_c, ip_c]);
    }
    let ip_c = self.u64c(state.ip as u64);
    if state.frame == 0 {
      self.call("zuri_jit_deopt_tier2", &[self.vm, ip_c]);
    } else {
      let (chain, len) = self.deopt_chain(state.frame);
      let chain_c = self.u64c(chain);
      let len_c = self.u64c(len);
      self.call("zuri_jit_deopt_inlined", &[self.vm, chain_c, len_c, ip_c]);
    }
    let junk = self.u64c(PENDING_RETURN);
    self.fb.ins().return_(&[junk]);
  }

  /// The interpreter frames a deopt inside `frame` pushes, outermost
  /// first. Made once per frame; the table is never freed, like the code
  /// that points at it.
  fn deopt_chain(&mut self, frame: u16) -> (u64, u64) {
    if let Some(&chain) = self.deopt_chains.get(&frame) {
      return chain;
    }
    let mut chain = Vec::new();
    let mut at = frame;
    while at != 0 {
      let f = &self.ir.frames[at as usize];
      chain.push(crate::jit::DeoptFrame {
        proto: f.proto,
        call_ip: f.call_ip,
        offset: f.offset,
        dst: f.dst,
        closure: f.closure,
      });
      at = f.parent;
    }
    chain.reverse();
    let len = chain.len() as u64;
    let table: &'static [crate::jit::DeoptFrame] = Box::leak(chain.into_boxed_slice());
    let entry = (table.as_ptr() as u64, len);
    self.deopt_chains.insert(frame, entry);
    entry
  }

  fn leave_on_error(&mut self, status: IrValue) {
    let err = self.fb.create_block();
    let ok = self.fb.create_block();
    self.fb.ins().brif(status, err, &[], ok, &[]);
    self.fb.switch_to_block(err);
    self.fb.set_cold_block(err);
    let junk = self.u64c(PENDING_RETURN);
    self.fb.ins().return_(&[junk]);
    self.fb.switch_to_block(ok);
  }

  /// Writes every value in `state` into its register, boxed.
  fn flush(&mut self, state: &FrameState) {
    for &(r, v) in &state.regs {
      let x = self.tagged_of(v);
      self.store_reg(r, x);
    }
  }

  /// Tells the VM where compiled code is, for stack traces. Inside a
  /// frame built in, that is a position in several functions at once,
  /// registered for the VM to expand.
  fn publish_ip(&mut self, ip: usize, frame: u16) {
    let position = if frame == 0 {
      ip + 1
    } else {
      self.position(frame, ip)
    };
    let v = self.u64c(position as u64);
    self
      .fb
      .ins()
      .store(MemFlagsData::trusted(), v, self.vm, JIT_IP_OFFSET);
  }

  fn position(&mut self, frame: u16, ip: usize) -> usize {
    if let Some(&p) = self.positions.get(&(frame, ip)) {
      return p;
    }
    // Innermost first, each as the function and the position just past
    // the instruction it is at, the way a frame's own `ip` reads.
    let mut spots = vec![(self.ir.frames[frame as usize].proto, ip + 1)];
    let mut at = frame;
    while at != 0 {
      let f = &self.ir.frames[at as usize];
      let parent = &self.ir.frames[f.parent as usize];
      spots.push((parent.proto, f.call_ip + 1));
      at = f.parent;
    }
    let p = crate::jit::register_inline_position(spots);
    self.positions.insert((frame, ip), p);
    p
  }

  // --- the register file -------------------------------------------------

  fn refresh_regs(&mut self) {
    let p = self.fb.ins().load(
      types::I64,
      MemFlagsData::trusted(),
      self.vm,
      REGS_PTR_CACHE_OFFSET,
    );
    self.fb.def_var(self.regs_var, p);
  }

  fn reg_addr(&mut self, r: u8) -> IrValue {
    let regs = self.fb.use_var(self.regs_var);
    let frame = self.fb.ins().iadd(regs, self.base_bytes);
    self.fb.ins().iadd_imm_s(frame, r as i64 * 8)
  }

  fn load_reg(&mut self, r: u8) -> IrValue {
    let addr = self.reg_addr(r);
    self
      .fb
      .ins()
      .load(types::I64, MemFlagsData::trusted(), addr, 0)
  }

  fn store_reg(&mut self, r: u8, v: IrValue) {
    let addr = self.reg_addr(r);
    self.fb.ins().store(MemFlagsData::trusted(), v, addr, 0);
  }

  // --- representations ---------------------------------------------------

  /// `v` as a tagged `Value`, whatever its representation.
  fn tagged_of(&mut self, v: ValueId) -> IrValue {
    let x = self.v(v);
    match self.ir.ty(v) {
      Ty::Tagged | Ty::Ptr => x,
      Ty::F64 => self.from_f64(x),
      Ty::I64 => {
        let f = self.fb.ins().fcvt_from_sint(types::F64, x);
        self.from_f64(f)
      },
      Ty::Bool => self.box_bool(x),
    }
  }

  fn from_f64(&mut self, f: IrValue) -> IrValue {
    self.fb.ins().bitcast(types::I64, MemFlagsData::new(), f)
  }

  fn to_f64(&mut self, v: IrValue) -> IrValue {
    self.fb.ins().bitcast(types::F64, MemFlagsData::new(), v)
  }

  /// Zuri's `%`, as `value::num_rem` has it: two whole numbers in `i64`
  /// with a positive divisor take an integer remainder, given the
  /// dividend's sign so a zero result keeps it, and everything else goes
  /// to the runtime.
  fn fmod(&mut self, x: IrValue, y: IrValue) -> IrValue {
    let (ix, x_whole) = self.f64_to_int(x);
    let (iy, y_whole) = self.f64_to_int(y);
    // 2^63 saturates to i64::MAX, which converts back to 2^63 and so looks
    // exact; a remainder with it on either side is fmod's.
    let x_saturated = self.fb.ins().icmp_imm_s(IntCC::Equal, ix, i64::MAX);
    let y_saturated = self.fb.ins().icmp_imm_s(IntCC::Equal, iy, i64::MAX);
    let saturated = self.fb.ins().bor(x_saturated, y_saturated);
    let positive = self.fb.ins().icmp_imm_s(IntCC::SignedGreaterThan, iy, 0);
    let whole = self.fb.ins().band(x_whole, y_whole);
    let whole = self.fb.ins().band_not(whole, saturated);
    let fast_ok = self.fb.ins().band(whole, positive);
    let fast = self.fb.create_block();
    let slow = self.fb.create_block();
    let done = self.fb.create_block();
    self.fb.append_block_param(done, types::F64);
    self.fb.ins().brif(fast_ok, fast, &[], slow, &[]);

    self.fb.switch_to_block(fast);
    let r = self.fb.ins().srem(ix, iy);
    let rf = self.fb.ins().fcvt_from_sint(types::F64, r);
    let signed = self.fb.ins().fcopysign(rf, x);
    self.fb.ins().jump(done, &[signed.into()]);

    self.fb.switch_to_block(slow);
    self.fb.set_cold_block(slow);
    let xb = self.from_f64(x);
    let yb = self.from_f64(y);
    let bits = self.call_pure("zuri_jit_num_fmod", &[self.vm, xb, yb]);
    let slow_r = self.to_f64(bits);
    self.fb.ins().jump(done, &[slow_r.into()]);

    self.fb.switch_to_block(done);
    self.fb.block_params(done)[0]
  }

  /// `v` when it is a constant positive whole number a double holds
  /// exactly.
  fn whole_constant(&self, v: ValueId) -> Option<i64> {
    let Op::ConstF64(c) = self.ir.def_inst(v)?.op else {
      return None;
    };
    (c >= 1.0 && c < (1u64 << 53) as f64 && c.fract() == 0.0).then_some(c as i64)
  }

  /// `fmod` by a divisor known to be the positive whole number `d`, which
  /// reaches Cranelift as a constant it can divide by without dividing.
  fn fmod_by(&mut self, x: IrValue, y: IrValue, d: i64) -> IrValue {
    let (ix, x_whole) = self.f64_to_int(x);
    let x_saturated = self.fb.ins().icmp_imm_s(IntCC::Equal, ix, i64::MAX);
    let fast_ok = self.fb.ins().band_not(x_whole, x_saturated);
    let fast = self.fb.create_block();
    let slow = self.fb.create_block();
    let done = self.fb.create_block();
    self.fb.append_block_param(done, types::F64);
    self.fb.ins().brif(fast_ok, fast, &[], slow, &[]);

    self.fb.switch_to_block(fast);
    let dc = self.fb.ins().iconst(types::I64, d);
    let r = self.fb.ins().srem(ix, dc);
    let rf = self.fb.ins().fcvt_from_sint(types::F64, r);
    let signed = self.fb.ins().fcopysign(rf, x);
    self.fb.ins().jump(done, &[signed.into()]);

    self.fb.switch_to_block(slow);
    self.fb.set_cold_block(slow);
    let xb = self.from_f64(x);
    let yb = self.from_f64(y);
    let bits = self.call_pure("zuri_jit_num_fmod", &[self.vm, xb, yb]);
    let slow_r = self.to_f64(bits);
    self.fb.ins().jump(done, &[slow_r.into()]);

    self.fb.switch_to_block(done);
    self.fb.block_params(done)[0]
  }

  fn ibit(&mut self, bit: BitOp, x: IrValue, y: IrValue) -> IrValue {
    let width = match bit {
      BitOp::And => return self.fb.ins().band(x, y),
      BitOp::Or => return self.fb.ins().bor(x, y),
      BitOp::Xor => return self.fb.ins().bxor(x, y),
      BitOp::Ushr => 32,
      BitOp::Shl | BitOp::Shr => 64,
    };
    // The interpreter takes the amount as a u32 and gives 0 once it
    // reaches the width, where Cranelift would take it modulo the width.
    let amount = self.fb.ins().band_imm_u(y, 0xffff_ffff);
    let too_far = self
      .fb
      .ins()
      .icmp_imm_u(IntCC::UnsignedGreaterThanOrEqual, amount, width);
    let shifted = match bit {
      BitOp::Shl => self.fb.ins().ishl(x, amount),
      BitOp::Shr => self.fb.ins().sshr(x, amount),
      _ => {
        let low = self.fb.ins().band_imm_u(x, 0xffff_ffff);
        self.fb.ins().ushr(low, amount)
      },
    };
    let zero = self.fb.ins().iconst(types::I64, 0);
    self.fb.ins().select(too_far, zero, shifted)
  }

  /// `value::num_to_wrapped_i64`: anything inside the `i64` range
  /// converts directly, the conversion itself dropping the fraction, and
  /// the rest goes to the runtime. Doubles that large are whole, so a
  /// value inside the range is inside it once truncated too.
  fn wrap_i64(&mut self, f: IrValue) -> IrValue {
    let lo = self.fb.ins().f64const(-9223372036854775808.0);
    let hi = self.fb.ins().f64const(9223372036854775808.0);
    let above = self.fb.ins().fcmp(FloatCC::GreaterThanOrEqual, f, lo);
    let below = self.fb.ins().fcmp(FloatCC::LessThan, f, hi);
    let inside = self.fb.ins().band(above, below);
    let fast = self.fb.create_block();
    let slow = self.fb.create_block();
    let done = self.fb.create_block();
    self.fb.append_block_param(done, types::I64);
    self.fb.ins().brif(inside, fast, &[], slow, &[]);

    self.fb.switch_to_block(fast);
    let i = self.fb.ins().fcvt_to_sint_sat(types::I64, f);
    self.fb.ins().jump(done, &[i.into()]);

    self.fb.switch_to_block(slow);
    self.fb.set_cold_block(slow);
    let bits = self.from_f64(f);
    let r = self.call_pure("zuri_jit_wrap_i64", &[self.vm, bits]);
    self.fb.ins().jump(done, &[r.into()]);

    self.fb.switch_to_block(done);
    self.fb.block_params(done)[0]
  }

  fn box_bool(&mut self, c: IrValue) -> IrValue {
    let t = self.u64c(value::TRUE_VAL);
    let f = self.u64c(value::FALSE_VAL);
    self.fb.ins().select(c, t, f)
  }

  /// `f` as an integer, and whether the conversion was exact.
  fn f64_to_int(&mut self, f: IrValue) -> (IrValue, IrValue) {
    let i = self.fb.ins().fcvt_to_sint_sat(types::I64, f);
    let back = self.fb.ins().fcvt_from_sint(types::F64, i);
    let exact = self.fb.ins().fcmp(FloatCC::Equal, back, f);
    (i, exact)
  }

  fn is_number(&mut self, v: IrValue) -> IrValue {
    let qnan = self.u64c(value::QNAN);
    let masked = self.fb.ins().band(v, qnan);
    self.fb.ins().icmp(IntCC::NotEqual, masked, qnan)
  }

  fn is_obj(&mut self, v: IrValue) -> IrValue {
    let mask = self.u64c(value::QNAN | value::SIGN_BIT);
    let masked = self.fb.ins().band(v, mask);
    self.fb.ins().icmp(IntCC::Equal, masked, mask)
  }

  fn obj_ptr(&mut self, v: IrValue) -> IrValue {
    let mask = self.u64c(value::PTR_MASK);
    self.fb.ins().band(v, mask)
  }

  /// Whether `v` is a heap object whose tag is `tag`. Only reads the tag
  /// once the object test has passed.
  fn obj_tag_is(&mut self, v: IrValue, tag: u8) -> IrValue {
    let is_obj = self.is_obj(v);
    let check = self.fb.create_block();
    let done = self.fb.create_block();
    self.fb.append_block_param(done, types::I8);
    let no = self.fb.ins().iconst(types::I8, 0);
    self.fb.ins().brif(is_obj, check, &[], done, &[no.into()]);
    self.fb.switch_to_block(check);
    let p = self.obj_ptr(v);
    let t = self.fb.ins().load(types::I8, MemFlagsData::trusted(), p, 0);
    let yes = self.fb.ins().icmp_imm_s(IntCC::Equal, t, tag as i64);
    self.fb.ins().jump(done, &[yes.into()]);
    self.fb.switch_to_block(done);
    self.fb.block_params(done)[0]
  }

  fn instance_of(&mut self, v: IrValue, class: u64) -> IrValue {
    let is_inst = self.obj_tag_is(v, object::OBJ_TAG_INSTANCE);
    let check = self.fb.create_block();
    let done = self.fb.create_block();
    self.fb.append_block_param(done, types::I8);
    let no = self.fb.ins().iconst(types::I8, 0);
    self.fb.ins().brif(is_inst, check, &[], done, &[no.into()]);
    self.fb.switch_to_block(check);
    let p = self.obj_ptr(v);
    let bits = self.fb.ins().load(
      types::I64,
      MemFlagsData::trusted(),
      p,
      object::obj_instance_class_offset() as i32,
    );
    let want = self.u64c(class);
    let same = self.fb.ins().icmp(IntCC::Equal, bits, want);
    self.fb.ins().jump(done, &[same.into()]);
    self.fb.switch_to_block(done);
    self.fb.block_params(done)[0]
  }

  /// Whether `v` is an instance of `class` or a class descending from
  /// it: the entry at `depth` of its class's display is `class`.
  fn instance_in_family(&mut self, v: IrValue, class: u64, depth: u8) -> IrValue {
    let is_inst = self.obj_tag_is(v, object::OBJ_TAG_INSTANCE);
    let check = self.fb.create_block();
    let done = self.fb.create_block();
    self.fb.append_block_param(done, types::I8);
    let no = self.fb.ins().iconst(types::I8, 0);
    self.fb.ins().brif(is_inst, check, &[], done, &[no.into()]);
    self.fb.switch_to_block(check);
    let flags = MemFlagsData::trusted();
    let p = self.obj_ptr(v);
    let bits = self.fb.ins().load(
      types::I64,
      flags,
      p,
      object::obj_instance_class_offset() as i32,
    );
    let class_obj = self.obj_ptr(bits);
    let (box_offset, display) = object::obj_class_display_offsets();
    let boxed = self
      .fb
      .ins()
      .load(types::I64, flags, class_obj, box_offset as i32);
    let entry = display + depth as usize * std::mem::size_of::<u64>();
    let seen = self.fb.ins().load(types::I64, flags, boxed, entry as i32);
    let want = self.u64c(class);
    let same = self.fb.ins().icmp(IntCC::Equal, seen, want);
    self.fb.ins().jump(done, &[same.into()]);
    self.fb.switch_to_block(done);
    self.fb.block_params(done)[0]
  }

  /// A number method on `f`, computed the way the runtime computes it.
  fn funary(&mut self, u: FUnary, f: IrValue) -> IrValue {
    match u {
      FUnary::Sqrt => self.fb.ins().sqrt(f),
      FUnary::Abs => self.fb.ins().fabs(f),
      FUnary::Floor => self.fb.ins().floor(f),
      FUnary::Ceil => self.fb.ins().ceil(f),
      FUnary::Trunc => self.fb.ins().trunc(f),
      FUnary::Round => {
        let half = self.fb.ins().f64const(0.5);
        let mag = self.fb.ins().fabs(f);
        let up = self.fb.ins().fadd(mag, half);
        let down = self.fb.ins().floor(up);
        self.fb.ins().fcopysign(down, f)
      },
      FUnary::Sign => {
        let zero = self.fb.ins().f64const(0.0);
        let one = self.fb.ins().f64const(1.0);
        let minus_one = self.fb.ins().f64const(-1.0);
        let is_zero = self.fb.ins().fcmp(FloatCC::Equal, f, zero);
        let is_pos = self.fb.ins().fcmp(FloatCC::GreaterThan, f, zero);
        // A zero answers itself, which keeps the sign of -0.
        let nonzero = self.fb.ins().select(is_pos, one, minus_one);
        self.fb.ins().select(is_zero, f, nonzero)
      },
      FUnary::Int => {
        let i = self.fb.ins().fcvt_to_sint_sat(types::I64, f);
        self.fb.ins().fcvt_from_sint(types::F64, i)
      },
    }
  }

  /// `max()` or `min()` as `builtins::number::larger` and `smaller`
  /// answer them: a NaN gives way to the other number, and otherwise
  /// Cranelift's own `fmax`/`fmin`, which count `-0` as less than `0`.
  fn fpick(&mut self, max: bool, a: IrValue, b: IrValue) -> IrValue {
    let m = if max {
      self.fb.ins().fmax(a, b)
    } else {
      self.fb.ins().fmin(a, b)
    };
    let a_nan = self.fb.ins().fcmp(FloatCC::Unordered, a, a);
    let b_nan = self.fb.ins().fcmp(FloatCC::Unordered, b, b);
    let r = self.fb.ins().select(b_nan, a, m);
    self.fb.ins().select(a_nan, b, r)
  }

  fn ftest(&mut self, t: FTest, f: IrValue) -> IrValue {
    match t {
      FTest::IsNan => self.fb.ins().fcmp(FloatCC::NotEqual, f, f),
      FTest::IsInf => {
        let mag = self.fb.ins().fabs(f);
        let inf = self.fb.ins().f64const(f64::INFINITY);
        self.fb.ins().fcmp(FloatCC::Equal, mag, inf)
      },
      FTest::IsFinite => {
        let mag = self.fb.ins().fabs(f);
        let inf = self.fb.ins().f64const(f64::INFINITY);
        self.fb.ins().fcmp(FloatCC::LessThan, mag, inf)
      },
      FTest::Truthy => {
        let zero = self.fb.ins().f64const(0.0);
        self.fb.ins().fcmp(FloatCC::OrderedNotEqual, f, zero)
      },
    }
  }

  /// `StrAscii`: the cached flag when the string has one, the runtime's
  /// classification when it does not yet.
  fn str_ascii(&mut self, s: IrValue) -> IrValue {
    let flags = MemFlagsData::trusted();
    let form = self
      .fb
      .ins()
      .load(types::I8, flags, s, object::obj_str_ascii_offset());
    let yes = self
      .fb
      .ins()
      .icmp_imm_s(IntCC::Equal, form, object::ASCII_YES as i64);
    let no = self
      .fb
      .ins()
      .icmp_imm_s(IntCC::Equal, form, object::ASCII_NO as i64);
    let known = self.fb.ins().bor(yes, no);
    let ask = self.fb.create_block();
    let done = self.fb.create_block();
    self.fb.append_block_param(done, types::I8);
    self.fb.ins().brif(known, done, &[yes.into()], ask, &[]);
    self.fb.switch_to_block(ask);
    self.fb.set_cold_block(ask);
    let r = self.call_pure("zuri_jit_str_ascii", &[self.vm, s]);
    let r8 = self.fb.ins().ireduce(types::I8, r);
    self.fb.ins().jump(done, &[r8.into()]);
    self.fb.switch_to_block(done);
    self.fb.block_params(done)[0]
  }

  /// `length()` on a string: the byte length of an ASCII string, the
  /// runtime's count for anything else.
  fn str_length(&mut self, s: IrValue) -> IrValue {
    let flags = MemFlagsData::trusted();
    let form = self
      .fb
      .ins()
      .load(types::I8, flags, s, object::obj_str_ascii_offset());
    let ascii = self
      .fb
      .ins()
      .icmp_imm_s(IntCC::Equal, form, object::ASCII_YES as i64);
    let fast = self.fb.create_block();
    let count = self.fb.create_block();
    let done = self.fb.create_block();
    self.fb.append_block_param(done, types::I64);
    self.fb.ins().brif(ascii, fast, &[], count, &[]);
    self.fb.switch_to_block(fast);
    let len = self
      .fb
      .ins()
      .load(types::I64, flags, s, object::obj_str_len_offset());
    self.fb.ins().jump(done, &[len.into()]);
    self.fb.switch_to_block(count);
    let n = self.call_pure("zuri_jit_str_length", &[self.vm, s]);
    self.fb.ins().jump(done, &[n.into()]);
    self.fb.switch_to_block(done);
    self.fb.block_params(done)[0]
  }

  /// `ord()` on a string: a flat string one byte long is one ASCII
  /// character, whose code is that byte; the runtime answers the rest,
  /// with NaN where the method raises.
  fn str_ord(&mut self, s: IrValue) -> IrValue {
    let flags = MemFlagsData::trusted();
    let form = self
      .fb
      .ins()
      .load(types::I8, flags, s, object::obj_str_ascii_offset());
    let flat = self
      .fb
      .ins()
      .icmp_imm_s(IntCC::NotEqual, form, object::STR_ROPE as i64);
    let fast = self.fb.create_block();
    let check = self.fb.create_block();
    let slow = self.fb.create_block();
    let done = self.fb.create_block();
    self.fb.append_block_param(done, types::F64);
    self.fb.ins().brif(flat, check, &[], slow, &[]);
    self.fb.switch_to_block(check);
    let len = self
      .fb
      .ins()
      .load(types::I64, flags, s, object::obj_str_len_offset());
    let single = self.fb.ins().icmp_imm_s(IntCC::Equal, len, 1);
    self.fb.ins().brif(single, fast, &[], slow, &[]);
    self.fb.switch_to_block(fast);
    let data = self
      .fb
      .ins()
      .load(types::I64, flags, s, object::obj_str_ptr_offset());
    let b = self.fb.ins().load(types::I8, flags, data, 0);
    let code = self.fb.ins().uextend(types::I64, b);
    let f = self.fb.ins().fcvt_from_sint(types::F64, code);
    self.fb.ins().jump(done, &[f.into()]);
    self.fb.switch_to_block(slow);
    let bits = self.call_pure("zuri_jit_str_ord", &[self.vm, s]);
    let f = self.to_f64(bits);
    self.fb.ins().jump(done, &[f.into()]);
    self.fb.switch_to_block(done);
    self.fb.block_params(done)[0]
  }

  /// `length()` on whatever `v` turns out to be: a list's, a string's, a
  /// byte stream's or a dict's, and -1 for anything without one.
  fn obj_length(&mut self, v: IrValue) -> IrValue {
    let flags = MemFlagsData::trusted();
    let done = self.fb.create_block();
    self.fb.append_block_param(done, types::I64);
    let none = self.fb.ins().iconst(types::I64, -1);
    let is_obj = self.is_obj(v);
    let tagged = self.fb.create_block();
    self
      .fb
      .ins()
      .brif(is_obj, tagged, &[], done, &[none.into()]);
    self.fb.switch_to_block(tagged);
    let p = self.obj_ptr(v);
    let tag = self.fb.ins().load(types::I8, flags, p, 0);
    let cases: [(u8, fn(&mut Self, IrValue, IrValue) -> IrValue); 4] = [
      (object::OBJ_TAG_LIST, |l, p, _| {
        let len32 = l.fb.ins().load(
          types::I32,
          MemFlagsData::trusted(),
          p,
          object::obj_list_len_offset(),
        );
        l.fb.ins().uextend(types::I64, len32)
      }),
      (object::OBJ_TAG_STR, |l, p, _| l.str_length(p)),
      (object::OBJ_TAG_BYTES, |l, p, _| {
        l.fb.ins().load(
          types::I64,
          MemFlagsData::trusted(),
          p,
          object::obj_bytes_len_offset(),
        )
      }),
      (object::OBJ_TAG_DICT, |l, _, v| {
        l.call_pure("zuri_jit_dict_len", &[l.vm, v])
      }),
    ];
    for (want, count) in cases {
      let hit = self.fb.ins().icmp_imm_s(IntCC::Equal, tag, want as i64);
      let yes = self.fb.create_block();
      let next = self.fb.create_block();
      self.fb.ins().brif(hit, yes, &[], next, &[]);
      self.fb.switch_to_block(yes);
      let n = count(self, p, v);
      self.fb.ins().jump(done, &[n.into()]);
      self.fb.switch_to_block(next);
    }
    self.fb.ins().jump(done, &[none.into()]);
    self.fb.switch_to_block(done);
    self.fb.block_params(done)[0]
  }

  /// `get(idx, fallback)` on a list. The load sits behind a real branch:
  /// past the end there may be no buffer to read.
  fn list_get_or(&mut self, list: IrValue, idx: IrValue, fallback: IrValue) -> IrValue {
    let flags = MemFlagsData::trusted();
    let len32 = self
      .fb
      .ins()
      .load(types::I32, flags, list, object::obj_list_len_offset());
    let len = self.fb.ins().uextend(types::I64, len32);
    let inside = self.fb.ins().icmp(IntCC::UnsignedLessThan, idx, len);
    let load = self.fb.create_block();
    let done = self.fb.create_block();
    self.fb.append_block_param(done, types::I64);
    self
      .fb
      .ins()
      .brif(inside, load, &[], done, &[fallback.into()]);
    self.fb.switch_to_block(load);
    let data = self.list_data(list);
    let off = self.fb.ins().imul_imm_s(idx, 8);
    let addr = self.fb.ins().iadd(data, off);
    let v = self.fb.ins().load(types::I64, flags, addr, 0);
    self.fb.ins().jump(done, &[v.into()]);
    self.fb.switch_to_block(done);
    self.fb.block_params(done)[0]
  }

  /// The iteration protocol's next key over `len` positions.
  fn next_key(&mut self, prev: IrValue, len: IrValue) -> IrValue {
    let nil = self.u64c(value::NIL_VAL);
    let invalid = self.u64c(super::NO_VALUE);
    let is_nil = self.fb.ins().icmp(IntCC::Equal, prev, nil);
    let is_num = self.is_number(prev);

    let zero = self.fb.ins().iconst(types::I64, 0);
    let empty = self.fb.ins().icmp(IntCC::Equal, len, zero);
    let zero_f = self.fb.ins().f64const(0.0);
    let first = self.from_f64(zero_f);
    let after_nil = self.fb.ins().select(empty, nil, first);

    let f = self.to_f64(prev);
    let k = self.fb.ins().fcvt_to_sint_sat(types::I64, f);
    let last = self.fb.ins().iadd_imm_s(len, -1);
    let below = self.fb.ins().icmp(IntCC::SignedLessThan, k, last);
    let from_zero = self.fb.ins().icmp(IntCC::SignedGreaterThanOrEqual, k, zero);
    let more = self.fb.ins().band(below, from_zero);
    let next = self.fb.ins().iadd_imm_s(k, 1);
    let next_f = self.fb.ins().fcvt_from_sint(types::F64, next);
    let next_v = self.from_f64(next_f);
    let after_num = self.fb.ins().select(more, next_v, nil);

    let known = self.fb.ins().select(is_num, after_num, invalid);
    self.fb.ins().select(is_nil, after_nil, known)
  }

  /// A range's lower bound, its step (a positive one, 1 standing in for
  /// any other), whether it counts up, and how many values it yields.
  fn range_shape(&mut self, r: IrValue) -> (IrValue, IrValue, IrValue, IrValue) {
    let flags = MemFlagsData::trusted();
    let lower = self
      .fb
      .ins()
      .load(types::F64, flags, r, object::obj_range_lower_offset());
    let upper = self
      .fb
      .ins()
      .load(types::F64, flags, r, object::obj_range_upper_offset());
    let step = self
      .fb
      .ins()
      .load(types::F64, flags, r, object::obj_range_step_offset());
    let zero = self.fb.ins().f64const(0.0);
    let one = self.fb.ins().f64const(1.0);
    let positive = self.fb.ins().fcmp(FloatCC::GreaterThan, step, zero);
    let step = self.fb.ins().select(positive, step, one);
    let diff = self.fb.ins().fsub(upper, lower);
    let width = self.fb.ins().fabs(diff);
    let steps = self.fb.ins().fdiv(width, step);
    let count = self.fb.ins().ceil(steps);
    let forward = self
      .fb
      .ins()
      .fcmp(FloatCC::GreaterThanOrEqual, upper, lower);
    (lower, step, forward, count)
  }

  /// A range's value at position `pos`, or nil past its end.
  fn range_at(&mut self, r: IrValue, pos: IrValue) -> IrValue {
    let (lower, step, forward, count) = self.range_shape(r);
    let zero = self.fb.ins().f64const(0.0);
    let from_zero = self.fb.ins().fcmp(FloatCC::GreaterThanOrEqual, pos, zero);
    let below = self.fb.ins().fcmp(FloatCC::LessThan, pos, count);
    let inside = self.fb.ins().band(from_zero, below);
    let delta = self.fb.ins().fmul(pos, step);
    let up = self.fb.ins().fadd(lower, delta);
    let down = self.fb.ins().fsub(lower, delta);
    let v = self.fb.ins().select(forward, up, down);
    let v = self.from_f64(v);
    let nil = self.u64c(value::NIL_VAL);
    self.fb.ins().select(inside, v, nil)
  }

  /// A small list, bumped out of the nursery inline, with the reloads
  /// that follow it: they read memory only on the path where the runtime
  /// made the list instead and may have collected.
  fn lower_new_list(&mut self, id: InstId, reloads: &[InstId]) -> Result<(), String> {
    let inst = self.ir.inst(id).clone();
    let Op::NewList {
      dst,
      start,
      count,
      frame,
    } = inst.op
    else {
      unreachable!()
    };
    let state = inst.state.as_ref().unwrap();
    let flags = MemFlagsData::trusted();
    let vm = self.vm;

    let slow = self.fb.create_block();
    let join = self.fb.create_block();
    self.fb.append_block_param(join, types::I64);
    for &r in reloads {
      let result = self.ir.inst(r).result.unwrap();
      self
        .fb
        .append_block_param(join, cl_type(self.ir.ty(result)));
    }

    // Room in the nursery, and on the finalization list every list joins
    // since it may spill to a buffer of its own later.
    let cur = self
      .fb
      .ins()
      .load(types::I64, flags, vm, HEAP_NURSERY_CUR_OFFSET);
    let end = self
      .fb
      .ins()
      .load(types::I64, flags, vm, HEAP_NURSERY_END_OFFSET);
    let fcur = self
      .fb
      .ins()
      .load(types::I64, flags, vm, HEAP_FINALIZE_CUR_OFFSET);
    let fend = self
      .fb
      .ins()
      .load(types::I64, flags, vm, HEAP_FINALIZE_END_OFFSET);
    let full = self.fb.ins().icmp(IntCC::Equal, cur, end);
    let fin_full = self.fb.ins().icmp(IntCC::Equal, fcur, fend);
    let no_room = self.fb.ins().bor(full, fin_full);
    let fast = self.fb.create_block();
    self.fb.ins().brif(no_room, slow, &[], fast, &[]);

    self.fb.switch_to_block(fast);
    let size = object::SLOT_BYTES as i64 + count as i64 * 8;
    let next = self.fb.ins().iadd_imm_s(cur, 64);
    self
      .fb
      .ins()
      .store(flags, next, vm, HEAP_NURSERY_CUR_OFFSET);
    self.fb.ins().store(flags, cur, fcur, 0);
    let fnext = self.fb.ins().iadd_imm_s(fcur, 8);
    self
      .fb
      .ins()
      .store(flags, fnext, vm, HEAP_FINALIZE_CUR_OFFSET);
    for (offset, delta) in [
      (HEAP_LIVE_COUNT_OFFSET, 1),
      (HEAP_BYTES_ALLOCATED_OFFSET, size),
    ] {
      let old = self.fb.ins().load(types::I64, flags, vm, offset);
      let new = self.fb.ins().iadd_imm_s(old, delta);
      self.fb.ins().store(flags, new, vm, offset);
    }
    let young = self
      .fb
      .ins()
      .load(types::I64, flags, vm, HEAP_YOUNG_BYTES_ALLOCATED_OFFSET);
    let young = self.fb.ins().iadd_imm_s(young, size);
    self
      .fb
      .ins()
      .store(flags, young, vm, HEAP_YOUNG_BYTES_ALLOCATED_OFFSET);
    // Owe a collection once the young budget is spent; the next
    // safepoint pays it.
    let budget = self.u64c(self.ir.young_budget);
    let owed = self
      .fb
      .ins()
      .icmp(IntCC::UnsignedGreaterThan, young, budget);
    let flag = self
      .fb
      .ins()
      .load(types::I8, flags, vm, HEAP_JIT_GC_NEEDED_OFFSET);
    let flag = self.fb.ins().bor(flag, owed);
    self
      .fb
      .ins()
      .store(flags, flag, vm, HEAP_JIT_GC_NEEDED_OFFSET);
    let header = self
      .fb
      .ins()
      .load(types::I64, flags, vm, HEAP_YOUNG_HEADER_OFFSET);
    self.fb.ins().store(flags, header, cur, 0);
    let zero = self.u64c(0);
    self.fb.ins().store(flags, zero, cur, 8);
    // The `Obj::List` payload: tag, borrow flag, no heap buffer, the
    // length with a capacity of zero, then the inline elements.
    let tag = self.u64c(object::OBJ_TAG_LIST as u64);
    self.fb.ins().store(flags, tag, cur, 16);
    self.fb.ins().store(flags, zero, cur, 24);
    self.fb.ins().store(flags, zero, cur, 32);
    let len = self.u64c(count as u64);
    self.fb.ins().store(flags, len, cur, 40);
    let nil = self.u64c(value::NIL_VAL);
    for k in 0..2usize {
      let v = match inst.args.get(k) {
        Some(&a) => self.tagged_of(a),
        None => nil,
      };
      self.fb.ins().store(flags, v, cur, 48 + 8 * k as i32);
    }
    let obj = self.fb.ins().iadd_imm_s(cur, 16);
    let mask = self.u64c(value::QNAN | value::SIGN_BIT);
    let list = self.fb.ins().bor(obj, mask);
    let mut kept = vec![list.into()];
    for &r in reloads {
      kept.push(self.v(self.ir.inst(r).args[0]).into());
    }
    self.fb.ins().jump(join, &kept);

    self.fb.switch_to_block(slow);
    self.fb.set_cold_block(slow);
    self.publish_ip(state.ip, frame);
    self.flush(state);
    let base = self.frame_base(frame);
    let (dst_c, start_c, count_c) = (
      self.u64c(dst as u64),
      self.u64c(start as u64),
      self.u64c(count as u64),
    );
    let status = self.call("zuri_jit_make_list", &[vm, base, dst_c, start_c, count_c]);
    self.leave_on_error(status);
    self.refresh_regs();
    let offset = self.ir.frames[frame as usize].offset;
    let made = self.load_reg(offset + dst);
    let mut fresh = vec![made.into()];
    for &r in reloads {
      let Op::Reload { reg } = self.ir.inst(r).op else {
        unreachable!()
      };
      fresh.push(self.load_reg(reg).into());
    }
    self.fb.ins().jump(join, &fresh);

    self.fb.switch_to_block(join);
    let params = self.fb.block_params(join).to_vec();
    self.def(inst.result, params[0]);
    for (&r, &p) in reloads.iter().zip(&params[1..]) {
      let result = self.ir.inst(r).result;
      self.def(result, p);
    }
    Ok(())
  }

  /// Whether `v` is a closure over the function whose `Value` bits are
  /// `proto_bits`.
  fn closure_of(&mut self, v: IrValue, proto_bits: u64) -> IrValue {
    let is_closure = self.obj_tag_is(v, object::OBJ_TAG_CLOSURE);
    let check = self.fb.create_block();
    let done = self.fb.create_block();
    self.fb.append_block_param(done, types::I8);
    let no = self.fb.ins().iconst(types::I8, 0);
    self
      .fb
      .ins()
      .brif(is_closure, check, &[], done, &[no.into()]);
    self.fb.switch_to_block(check);
    let p = self.obj_ptr(v);
    let function = self.fb.ins().load(
      types::I64,
      MemFlagsData::trusted(),
      p,
      object::obj_closure_function_offset() as i32,
    );
    let want = self.u64c(proto_bits);
    let same = self.fb.ins().icmp(IntCC::Equal, function, want);
    self.fb.ins().jump(done, &[same.into()]);
    self.fb.switch_to_block(done);
    self.fb.block_params(done)[0]
  }

  /// Zuri truthiness: nil, false, zero and NaN are falsey; so are empty
  /// strings, byte strings and zero bigints, which take a runtime call
  /// to decide.
  fn is_falsey(&mut self, v: IrValue) -> IrValue {
    let is_obj = self.is_obj(v);
    let obj_block = self.fb.create_block();
    let other_block = self.fb.create_block();
    let done = self.fb.create_block();
    self.fb.append_block_param(done, types::I8);
    self.fb.ins().brif(is_obj, obj_block, &[], other_block, &[]);

    self.fb.switch_to_block(obj_block);
    let p = self.obj_ptr(v);
    let tag8 = self.fb.ins().load(types::I8, MemFlagsData::trusted(), p, 0);
    let is_str = self
      .fb
      .ins()
      .icmp_imm_s(IntCC::Equal, tag8, object::OBJ_TAG_STR as i64);
    let is_bytes = self
      .fb
      .ins()
      .icmp_imm_s(IntCC::Equal, tag8, object::OBJ_TAG_BYTES as i64);
    let is_big = self
      .fb
      .ins()
      .icmp_imm_s(IntCC::Equal, tag8, object::OBJ_TAG_BIGINT as i64);
    let a = self.fb.ins().bor(is_str, is_bytes);
    let maybe = self.fb.ins().bor(a, is_big);
    let ask = self.fb.create_block();
    let no = self.fb.ins().iconst(types::I8, 0);
    self.fb.ins().brif(maybe, ask, &[], done, &[no.into()]);

    self.fb.switch_to_block(ask);
    let r = self.call_pure("zuri_jit_is_falsey", &[self.vm, v]);
    let r8 = self.fb.ins().ireduce(types::I8, r);
    self.fb.ins().jump(done, &[r8.into()]);

    self.fb.switch_to_block(other_block);
    let nil = self.u64c(value::NIL_VAL);
    let fals = self.u64c(value::FALSE_VAL);
    let is_nil = self.fb.ins().icmp(IntCC::Equal, v, nil);
    let is_false = self.fb.ins().icmp(IntCC::Equal, v, fals);
    let is_num = self.is_number(v);
    let f = self.to_f64(v);
    let zero = self.fb.ins().f64const(0.0);
    let zero_or_nan = self.fb.ins().fcmp(FloatCC::UnorderedOrEqual, f, zero);
    let num_falsey = self.fb.ins().band(is_num, zero_or_nan);
    let x = self.fb.ins().bor(is_nil, is_false);
    let r = self.fb.ins().bor(x, num_falsey);
    self.fb.ins().jump(done, &[r.into()]);

    self.fb.switch_to_block(done);
    self.fb.block_params(done)[0]
  }

  /// Zuri's `==`: numbers by value, two heap objects through the
  /// runtime's structural comparison, anything else by its bits. A left
  /// operand whose class defines `@eq` leaves for the interpreter at
  /// `state`, to call it there.
  fn tagged_eq(&mut self, a: IrValue, b: IrValue, state: &FrameState) -> IrValue {
    let na = self.is_number(a);
    let nb = self.is_number(b);
    let both_num = self.fb.ins().band(na, nb);
    let num_block = self.fb.create_block();
    let other = self.fb.create_block();
    let obj_block = self.fb.create_block();
    let bits_block = self.fb.create_block();
    let done = self.fb.create_block();
    self.fb.append_block_param(done, types::I8);
    self.fb.ins().brif(both_num, num_block, &[], other, &[]);

    self.fb.switch_to_block(num_block);
    let fa = self.to_f64(a);
    let fb_ = self.to_f64(b);
    let eq = self.fb.ins().fcmp(FloatCC::Equal, fa, fb_);
    self.fb.ins().jump(done, &[eq.into()]);

    self.fb.switch_to_block(other);
    let oa = self.is_obj(a);
    let ob = self.is_obj(b);
    let both_obj = self.fb.ins().band(oa, ob);
    self
      .fb
      .ins()
      .brif(both_obj, obj_block, &[], bits_block, &[]);

    self.fb.switch_to_block(obj_block);
    let r = self.call_pure("zuri_jit_values_equal", &[self.vm, a, b]);
    let overridden =
      self
        .fb
        .ins()
        .icmp_imm_u(IntCC::Equal, r, crate::jit::runtime::EQ_OVERRIDDEN as i64);
    let leave = self.fb.create_block();
    let answered = self.fb.create_block();
    self.fb.ins().brif(overridden, leave, &[], answered, &[]);
    self.fb.switch_to_block(leave);
    self.fb.set_cold_block(leave);
    self.deopt(state, None);
    self.fb.switch_to_block(answered);
    let r8 = self.fb.ins().ireduce(types::I8, r);
    self.fb.ins().jump(done, &[r8.into()]);

    self.fb.switch_to_block(bits_block);
    let same = self.fb.ins().icmp(IntCC::Equal, a, b);
    self.fb.ins().jump(done, &[same.into()]);

    self.fb.switch_to_block(done);
    self.fb.block_params(done)[0]
  }

  /// Whether `v` passes parameter check `check_idx`, testing each type
  /// the way `VM::param_type_matches` does. The builder only makes this
  /// guard for checks with no instance or iterable type.
  fn param_check(&mut self, v: IrValue, proto: &ObjFunction, check_idx: u16) -> IrValue {
    use crate::vm::chunk::ParamType;
    let check = proto.chunk.param_checks[check_idx as usize].clone();
    let mut ok = self.fb.ins().iconst(types::I8, 0);
    if check.nullable {
      let nil = self.u64c(value::NIL_VAL);
      let is_nil = self.fb.ins().icmp(IntCC::Equal, v, nil);
      ok = self.fb.ins().bor(ok, is_nil);
    }
    for t in &check.types {
      let tags: &[u8] = match t {
        ParamType::Bool => {
          let t = self.u64c(value::TRUE_VAL);
          let f = self.u64c(value::FALSE_VAL);
          let is_t = self.fb.ins().icmp(IntCC::Equal, v, t);
          let is_f = self.fb.ins().icmp(IntCC::Equal, v, f);
          let c = self.fb.ins().bor(is_t, is_f);
          ok = self.fb.ins().bor(ok, c);
          continue;
        },
        ParamType::Number => {
          let c = self.is_number(v);
          ok = self.fb.ins().bor(ok, c);
          continue;
        },
        ParamType::Int => {
          // A whole number: its fractional part is zero, which is never
          // true of an infinity or NaN.
          let is_num = self.is_number(v);
          let f = self.to_f64(v);
          let whole = self.fb.ins().trunc(f);
          let frac = self.fb.ins().fsub(f, whole);
          let zero = self.fb.ins().f64const(0.0);
          let no_frac = self.fb.ins().fcmp(FloatCC::Equal, frac, zero);
          let c = self.fb.ins().band(is_num, no_frac);
          ok = self.fb.ins().bor(ok, c);
          continue;
        },
        ParamType::BigInt => &[object::OBJ_TAG_BIGINT],
        ParamType::String => &[object::OBJ_TAG_STR],
        ParamType::Bytes => &[object::OBJ_TAG_BYTES],
        ParamType::List => &[object::OBJ_TAG_LIST],
        ParamType::Dict => &[object::OBJ_TAG_DICT],
        ParamType::Range => &[object::OBJ_TAG_RANGE],
        ParamType::File => &[object::OBJ_TAG_FILE],
        ParamType::Class => &[object::OBJ_TAG_CLASS],
        ParamType::Function => &[
          object::OBJ_TAG_CLOSURE,
          object::OBJ_TAG_NATIVE,
          object::OBJ_TAG_BOUND_METHOD,
        ],
        ParamType::Callable => &[
          object::OBJ_TAG_CLOSURE,
          object::OBJ_TAG_NATIVE,
          object::OBJ_TAG_BOUND_METHOD,
          object::OBJ_TAG_CLASS,
        ],
        ParamType::Instance(_) | ParamType::Iterable => {
          unreachable!("the builder keeps these checks on the runtime path")
        },
      };
      for &tag in tags {
        let c = self.obj_tag_is(v, tag);
        ok = self.fb.ins().bor(ok, c);
      }
    }
    ok
  }

  /// Upvalue cell `n` of the running closure. The closure is read from
  /// the live frame, the same way the baseline tier finds it.
  fn upval_cell(&mut self, n: u8, state: &FrameState) -> IrValue {
    let upvalues = self.call_pure("zuri_jit_closure_upvalues_ptr", &[self.vm]);
    let slot = self
      .fb
      .ins()
      .load(types::I64, MemFlagsData::trusted(), upvalues, n as i32 * 8);
    let ok = self.obj_tag_is(slot, object::OBJ_TAG_UPVALUE);
    let fail = self.fb.create_block();
    let cont = self.fb.create_block();
    self.fb.ins().brif(ok, cont, &[], fail, &[]);
    self.fb.switch_to_block(fail);
    self.fb.set_cold_block(fail);
    self.deopt(state, None);
    self.fb.switch_to_block(cont);
    self.obj_ptr(slot)
  }

  /// Whether an upvalue cell is closed, and the payload word: the value
  /// itself when closed, the absolute register index when open.
  fn upval_state(&mut self, cell: IrValue) -> (IrValue, IrValue) {
    let flags = MemFlagsData::trusted();
    let tag = self.fb.ins().load(
      types::I8,
      flags,
      cell,
      object::obj_upvalue_state_tag_offset() as i32,
    );
    let closed =
      self
        .fb
        .ins()
        .icmp_imm_s(IntCC::Equal, tag, object::UPVALUE_STATE_TAG_CLOSED as i64);
    let payload = self.fb.ins().load(
      types::I64,
      flags,
      cell,
      object::obj_upvalue_state_payload_offset() as i32,
    );
    (closed, payload)
  }

  /// The register an open cell points at, anywhere in the register file.
  fn open_upval_addr(&mut self, abs_idx: IrValue) -> IrValue {
    let regs = self.fb.use_var(self.regs_var);
    let off = self.fb.ins().imul_imm_s(abs_idx, 8);
    self.fb.ins().iadd(regs, off)
  }

  fn load_upval(&mut self, cell: IrValue) -> IrValue {
    let (closed, payload) = self.upval_state(cell);
    let open = self.fb.create_block();
    let done = self.fb.create_block();
    self.fb.append_block_param(done, types::I64);
    self
      .fb
      .ins()
      .brif(closed, done, &[payload.into()], open, &[]);
    self.fb.switch_to_block(open);
    let addr = self.open_upval_addr(payload);
    let v = self
      .fb
      .ins()
      .load(types::I64, MemFlagsData::trusted(), addr, 0);
    self.fb.ins().jump(done, &[v.into()]);
    self.fb.switch_to_block(done);
    self.fb.block_params(done)[0]
  }

  fn store_upval(&mut self, stored: ValueId, cell: IrValue, val: IrValue) {
    let flags = MemFlagsData::trusted();
    let (closed, payload) = self.upval_state(cell);
    let closed_block = self.fb.create_block();
    let open = self.fb.create_block();
    let done = self.fb.create_block();
    self.fb.ins().brif(closed, closed_block, &[], open, &[]);

    self.fb.switch_to_block(closed_block);
    self.fb.ins().store(
      flags,
      val,
      cell,
      object::obj_upvalue_state_payload_offset() as i32,
    );
    self.barrier_for_store(stored, val, cell);
    self.fb.ins().jump(done, &[]);

    // A register is never behind a barrier.
    self.fb.switch_to_block(open);
    let addr = self.open_upval_addr(payload);
    self.fb.ins().store(flags, val, addr, 0);
    self.fb.ins().jump(done, &[]);
    self.fb.switch_to_block(done);
  }

  /// A list's first or last element, or nil when it is empty. The load
  /// sits behind a real branch: an empty list's buffer may not exist.
  fn list_end(&mut self, list: IrValue, last: bool) -> IrValue {
    let len32 = self.fb.ins().load(
      types::I32,
      MemFlagsData::trusted(),
      list,
      object::obj_list_len_offset(),
    );
    let len = self.fb.ins().uextend(types::I64, len32);
    let elem = self.fb.create_block();
    let done = self.fb.create_block();
    self.fb.append_block_param(done, types::I64);
    let nil = self.u64c(value::NIL_VAL);
    self.fb.ins().brif(len, elem, &[], done, &[nil.into()]);
    self.fb.switch_to_block(elem);
    let data = self.list_data(list);
    let addr = if last {
      let idx = self.fb.ins().iadd_imm_s(len, -1);
      let off = self.fb.ins().imul_imm_s(idx, 8);
      self.fb.ins().iadd(data, off)
    } else {
      data
    };
    let v = self
      .fb
      .ins()
      .load(types::I64, MemFlagsData::trusted(), addr, 0);
    self.fb.ins().jump(done, &[v.into()]);
    self.fb.switch_to_block(done);
    self.fb.block_params(done)[0]
  }

  /// `append()`: stores straight into the list when it has room, and has
  /// the runtime grow it when it has none.
  fn list_append(&mut self, list: IrValue, item: ValueId) {
    let flags = MemFlagsData::trusted();
    let val = self.tagged_of(item);
    let len32 = self
      .fb
      .ins()
      .load(types::I32, flags, list, object::obj_list_len_offset());
    let len = self.fb.ins().uextend(types::I64, len32);
    let heap = self
      .fb
      .ins()
      .load(types::I64, flags, list, object::obj_list_ptr_offset());
    let cap32 = self
      .fb
      .ins()
      .load(types::I32, flags, list, object::obj_list_cap_offset());
    let cap = self.fb.ins().uextend(types::I64, cap32);
    let inline_cap = self.u64c(crate::vm::list::INLINE_CAP as u64);
    let zero = self.fb.ins().iconst(types::I64, 0);
    let is_inline = self.fb.ins().icmp(IntCC::Equal, heap, zero);
    let room = self.fb.ins().select(is_inline, inline_cap, cap);
    let fits = self.fb.ins().icmp(IntCC::UnsignedLessThan, len, room);

    let store = self.fb.create_block();
    let grow = self.fb.create_block();
    let done = self.fb.create_block();
    self.fb.ins().brif(fits, store, &[], grow, &[]);

    self.fb.switch_to_block(store);
    let inline = self
      .fb
      .ins()
      .iadd_imm_s(list, object::obj_list_inline_offset() as i64);
    let data = self.fb.ins().select(is_inline, inline, heap);
    let off = self.fb.ins().imul_imm_s(len, 8);
    let addr = self.fb.ins().iadd(data, off);
    self.fb.ins().store(flags, val, addr, 0);
    let new_len = self.fb.ins().iadd_imm_s(len, 1);
    let new_len32 = self.fb.ins().ireduce(types::I32, new_len);
    self
      .fb
      .ins()
      .store(flags, new_len32, list, object::obj_list_len_offset());
    self.barrier_for_store(item, val, list);
    self.fb.ins().jump(done, &[]);

    self.fb.switch_to_block(grow);
    self.fb.set_cold_block(grow);
    self.call_pure("zuri_jit_list_push", &[self.vm, list, val]);
    self.fb.ins().jump(done, &[]);
    self.fb.switch_to_block(done);
  }

  /// A list's element buffer: its heap storage, or the inline buffer
  /// inside the object when it has none.
  fn list_data(&mut self, list: IrValue) -> IrValue {
    let heap = self.fb.ins().load(
      types::I64,
      MemFlagsData::trusted(),
      list,
      object::obj_list_ptr_offset(),
    );
    let inline = self
      .fb
      .ins()
      .iadd_imm_s(list, object::obj_list_inline_offset() as i64);
    let zero = self.fb.ins().iconst(types::I64, 0);
    let is_inline = self.fb.ins().icmp(IntCC::Equal, heap, zero);
    self.fb.ins().select(is_inline, inline, heap)
  }

  /// An instance's field storage, inline or on the heap.
  fn fields_ptr(&mut self, obj: IrValue) -> IrValue {
    let heap = self.fb.ins().load(
      types::I64,
      MemFlagsData::trusted(),
      obj,
      object::obj_instance_fields_offset() as i32,
    );
    let inline = self
      .fb
      .ins()
      .iadd_imm_s(obj, object::obj_instance_fields_inline_offset() as i64);
    let zero = self.fb.ins().iconst(types::I64, 0);
    let is_inline = self.fb.ins().icmp(IntCC::Equal, heap, zero);
    self.fb.ins().select(is_inline, inline, heap)
  }

  /// The write barrier after storing `val` into `container`: an old
  /// container not yet remembered has to be told it now points at
  /// something that may be young. Skipped outright when the stored value
  /// is known not to be an object.
  fn barrier_for_store(&mut self, stored: ValueId, val: IrValue, container: IrValue) {
    if matches!(self.ir.ty(stored), Ty::F64 | Ty::I64 | Ty::Bool) {
      return;
    }
    // A boxed number or boolean, or a constant that is not an object, is
    // never a pointer the collector has to hear about. Skipping the check
    // also keeps its call out of the loop, where it would make every
    // double live across the store spill.
    let never_object = match self.ir.def_inst(stored).map(|i| &i.op) {
      Some(Op::BoxF64 | Op::BoxBool) => true,
      Some(Op::ConstTagged(bits)) => !value::Value::from_bits(*bits).is_obj(),
      _ => false,
    };
    if never_object {
      return;
    }
    let is_obj = self.is_obj(val);
    let check = self.fb.create_block();
    let barrier = self.fb.create_block();
    let done = self.fb.create_block();
    self.fb.ins().brif(is_obj, check, &[], done, &[]);

    self.fb.switch_to_block(check);
    let flags = MemFlagsData::trusted();
    let generation = self.fb.ins().load(
      types::I8,
      flags,
      container,
      object::obj_to_gcbox_generation_offset(),
    );
    let remembered = self.fb.ins().load(
      types::I8,
      flags,
      container,
      object::obj_to_gcbox_remembered_offset(),
    );
    let is_old =
      self
        .fb
        .ins()
        .icmp_imm_s(IntCC::Equal, generation, object::GENERATION_OLD_BYTE as i64);
    let fresh = self.fb.ins().icmp_imm_s(IntCC::Equal, remembered, 0);
    let owed = self.fb.ins().band(is_old, fresh);
    self.fb.ins().brif(owed, barrier, &[], done, &[]);

    self.fb.switch_to_block(barrier);
    self.fb.set_cold_block(barrier);
    self.call_pure("zuri_jit_write_barrier", &[self.vm, container]);
    self.fb.ins().jump(done, &[]);
    self.fb.switch_to_block(done);
  }

  // --- calls ---------------------------------------------------------------

  fn u64c(&mut self, x: u64) -> IrValue {
    self.fb.ins().iconst(types::I64, x as i64)
  }

  fn call(&mut self, name: &str, args: &[IrValue]) -> IrValue {
    let id = *self
      .helpers
      .get(name)
      .unwrap_or_else(|| panic!("zuri: unregistered JIT helper '{name}'"));
    let func_ref = self.module.declare_func_in_func(id, self.fb.func);
    let call = self.fb.ins().call(func_ref, args);
    self.fb.inst_results(call)[0]
  }

  /// A helper that cannot collect, raise or touch the register file.
  fn call_pure(&mut self, name: &str, args: &[IrValue]) -> IrValue {
    self.call(name, args)
  }
}

/// Where a call made through `Lowering::fast_call` puts its callee: the
/// calling frame's window and where it sits, the callee's first argument
/// register and count in that frame, and where the result goes.
#[derive(Clone, Copy)]
struct FastCall {
  base: IrValue,
  offset: u8,
  first_arg: u8,
  num_args: u8,
  dst: u8,
}

/// Where a direct call finds its callee: in the register before its
/// arguments; there too but expected to hold the function the feedback
/// named, whose `ObjFunction` address and `Value` bits are given; or, for
/// a method call, in the site's cache, keyed by the receiver's class, at
/// the address given.
#[derive(Clone, Copy)]
enum DirectCallee {
  Register,
  Known { function: usize, bits: u64 },
  Cached(u64),
}

/// The signature every compiled entry shares; built the same way the
/// engine builds it for the baseline tier.
pub fn entry_signature(module: &JITModule) -> cranelift_codegen::ir::Signature {
  let mut sig = module.make_signature();
  for ty in [
    types::I64,
    types::I64,
    types::I64,
    types::I32,
    types::I64,
    types::I64,
    types::I64,
    types::I64,
  ] {
    sig.params.push(AbiParam::new(ty));
  }
  sig.returns.push(AbiParam::new(types::I64));
  sig
}
