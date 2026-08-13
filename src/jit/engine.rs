//! Owns the Cranelift `JITModule` and turns one `&ObjFunction` into a
//! `CompiledFunction`. See `crate::jit`'s module docs for the overall
//! architecture; this file is pure Cranelift/`cranelift_module`
//! plumbing -- the actual bytecode -> IR translation lives in
//! `jit::codegen`.

use std::collections::HashMap;

use cranelift_codegen::Context;
use cranelift_codegen::ir::{AbiParam, UserFuncName, types};
use cranelift_codegen::settings::{self, Configurable};
use cranelift_frontend::FunctionBuilderContext;
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::{FuncId, Linkage, Module};

use crate::jit::runtime;
use crate::jit::{CompiledFunction, EntryFn, codegen};
use crate::vm::object::ObjFunction;

pub struct JitEngine {
  module: JITModule,
  ctx: Context,
  builder_ctx: FunctionBuilderContext,
  /// Every `jit::runtime` helper's `FuncId`, keyed by its registered
  /// name -- declared ONCE, up front, and reused (via
  /// `Module::declare_func_in_func`) by every subsequent function this
  /// engine compiles.
  helper_ids: HashMap<&'static str, FuncId>,
  /// Monotonic counter giving every compiled function a distinct
  /// module-local symbol name (`cranelift_module::Module` requires
  /// unique names for `declare_function`) -- purely an internal detail,
  /// never exposed anywhere else.
  next_id: u64,
}

impl Default for JitEngine {
  fn default() -> JitEngine {
    JitEngine::new()
  }
}

impl JitEngine {
  pub fn new() -> JitEngine {
    let mut flag_builder = settings::builder();
    // Mirrors cranelift-jit's own recommended JIT flags (see its
    // `JITBuilder::with_flags`) -- required on at least AArch64, where
    // "colocated" calls use shorter-range relocations that can't reach
    // every definition in a JIT's address space.
    flag_builder.set("use_colocated_libcalls", "false").unwrap();
    flag_builder.set("is_pic", "false").unwrap();
    flag_builder.set("opt_level", "speed").unwrap();

    let isa_builder = cranelift_native::builder().unwrap_or_else(|msg| {
      panic!("zuri: host machine is not supported by the JIT backend: {msg}")
    });
    let isa = isa_builder
      .finish(settings::Flags::new(flag_builder))
      .expect("zuri: failed to build a target ISA for the JIT");

    let mut jit_builder = JITBuilder::with_isa(isa, cranelift_module::default_libcall_names());
    let specs = runtime::helper_table();
    for spec in &specs {
      jit_builder.symbol(spec.name, spec.ptr);
    }

    let mut module = JITModule::new(jit_builder);

    let mut helper_ids = HashMap::with_capacity(specs.len());
    for spec in &specs {
      let mut sig = module.make_signature();
      for _ in 0..spec.arity {
        sig.params.push(AbiParam::new(types::I64));
      }
      sig.returns.push(AbiParam::new(types::I64));
      let id = module
        .declare_function(spec.name, Linkage::Import, &sig)
        .unwrap_or_else(|e| panic!("zuri: failed to declare JIT helper '{}': {e}", spec.name));
      helper_ids.insert(spec.name, id);
    }

    let ctx = module.make_context();

    JitEngine {
      module,
      ctx,
      builder_ctx: FunctionBuilderContext::new(),
      helper_ids,
      next_id: 0,
    }
  }

  /// Compile `proto` to machine code. `Err(reason)` means `proto` is
  /// permanently ineligible (see `codegen::compile`'s own eligibility
  /// scan) -- the caller (`VM::try_compile`) marks it as such and never
  /// asks again. `speculative_params` is passed straight through to
  /// `codegen::compile` -- see its own docs.
  pub fn compile_function(&mut self, proto: &ObjFunction, speculative_params: Option<u64>) -> Result<CompiledFunction, String> {
    self.next_id += 1;
    let name = format!("zuri_fn_{}", self.next_id);

    let mut sig = self.module.make_signature();
    sig.params.push(AbiParam::new(types::I64)); // vm: *mut VM
    sig.params.push(AbiParam::new(types::I64)); // base: u64
    sig.params.push(AbiParam::new(types::I64)); // closure: u64 (tagged Value bits)
    sig.params.push(AbiParam::new(types::I32)); // osr_id: i32 (-1 = ordinary entry)
    sig.returns.push(AbiParam::new(types::I64)); // return value bits

    let func_id = self
      .module
      .declare_function(&name, Linkage::Local, &sig)
      .map_err(|e| format!("failed to declare function: {e}"))?;

    self.ctx.func.signature = sig;
    self.ctx.func.name = UserFuncName::user(0, func_id.as_u32());

    let osr_ids = {
      let mut builder =
        cranelift_frontend::FunctionBuilder::new(&mut self.ctx.func, &mut self.builder_ctx);
      let osr_ids = codegen::compile(&mut builder, &mut self.module, &self.helper_ids, proto, speculative_params)?;
      builder.seal_all_blocks();
      builder.finalize(self.module.target_config());
      osr_ids
    };

    if crate::jit::log_ir_enabled() {
      eprintln!(
        "[jit] IR for '{}':\n{}",
        proto.name,
        self.ctx.func.display()
      );
    }

    self
      .module
      .define_function(func_id, &mut self.ctx)
      .map_err(|e| format!("failed to define function: {e}"))?;
    self.module.clear_context(&mut self.ctx);

    self
      .module
      .finalize_definitions()
      .map_err(|e| format!("failed to finalize: {e}"))?;

    let code_ptr = self.module.get_finalized_function(func_id);
    // SAFETY: `code_ptr` was just produced by compiling a function with
    // EXACTLY the signature `EntryFn` describes (I64, I64, I64, I32 ->
    // I64, matching `sig` above), for the host's native calling
    // convention (via `cranelift_native::builder()` in `JitEngine::new`) --
    // the same convention `extern "C"` uses on every platform this
    // targets.
    let entry: EntryFn = unsafe { std::mem::transmute::<*const u8, EntryFn>(code_ptr) };

    Ok(CompiledFunction { entry, osr_ids })
  }
}
