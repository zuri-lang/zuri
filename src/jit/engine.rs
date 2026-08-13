//! Owns the Cranelift `JITModule` and turns one `&ObjFunction` into a
//! `CompiledFunction`. See `crate::jit`'s module docs for the overall
//! architecture; this file is pure Cranelift/`cranelift_module`
//! plumbing -- the actual bytecode -> IR translation lives in
//! `jit::codegen`.

use std::collections::HashMap;
use std::sync::Arc;

use cranelift_codegen::Context;
use cranelift_codegen::ir::{AbiParam, UserFuncName, types};
use cranelift_codegen::isa::TargetIsa;
use cranelift_codegen::settings::{self, Configurable};
use cranelift_frontend::FunctionBuilderContext;
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::{FuncId, Linkage, Module, ModuleReloc};
use rustc_hash::FxHashMap;

use crate::jit::runtime;
use crate::jit::{EntryFn, codegen};
use crate::vm::object::ObjFunction;

/// Everything `JitEngine::build_ir` extracts from a `&ObjFunction` --
/// enough to hand off to `jit::background`'s compiler thread for the
/// expensive part (`Context::compile`, see that module's docs on why
/// this needs no further access to `proto` or the `JITModule` at all),
/// then hand the result back to `JitEngine::install_compiled`.
pub struct PendingCompile {
  pub ctx: Context,
  pub func_id: FuncId,
  pub osr_ids: FxHashMap<usize, i32>,
}

pub struct JitEngine {
  module: JITModule,
  builder_ctx: FunctionBuilderContext,
  /// A second handle onto the SAME target configuration `module` was
  /// built with (not the module's own internal ISA -- a fully separate,
  /// independently-constructed instance) -- `Arc<dyn TargetIsa>` is
  /// `Send + Sync`, so cloning this out to `jit::background`'s compiler
  /// thread lets it run `Context::compile` (the expensive register-
  /// allocation/encoding step) with ZERO access to `module` at all, no
  /// locking needed. See `isa_handle`.
  isa: Arc<dyn TargetIsa>,
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
    // The backtracking allocator produces measurably better code
    // (fewer spills/moves) than the single-pass one, at the cost of
    // more compile time -- a trade that only became strictly correct
    // to make once compilation moved off the interpreter's own thread
    // (see `jit::background`): there is no longer a reason to economize
    // on compile time by settling for the cheaper allocator.
    flag_builder.set("regalloc_algorithm", "backtracking").unwrap();

    let isa_builder = cranelift_native::builder().unwrap_or_else(|msg| {
      panic!("zuri: host machine is not supported by the JIT backend: {msg}")
    });
    let isa = isa_builder
      .finish(settings::Flags::new(flag_builder))
      .expect("zuri: failed to build a target ISA for the JIT");
    // Kept alongside (not just inside) `module` -- see `isa`'s own
    // field docs on why `jit::background` needs an independent handle
    // to the same target config.
    let isa_for_module = isa.clone();

    let mut jit_builder =
      JITBuilder::with_isa(isa_for_module, cranelift_module::default_libcall_names());
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

    JitEngine {
      module,
      isa,
      builder_ctx: FunctionBuilderContext::new(),
      helper_ids,
      next_id: 0,
    }
  }

  /// A clone of this engine's target ISA handle, independent of
  /// `module` -- see `isa`'s own field docs. Cheap (an `Arc` bump).
  pub fn isa_handle(&self) -> Arc<dyn TargetIsa> {
    self.isa.clone()
  }

  /// Stage 1 of compiling `proto`: translate its bytecode to Cranelift
  /// IR (`jit::codegen`'s job) and declare a slot for it in `module`.
  /// This is the ONLY stage that touches `proto` at all, so it MUST
  /// run synchronously on the VM's own thread -- see `jit::background`'s
  /// module docs for why everything after this point (the actual
  /// register-allocation/encoding work, `Context::compile`) is safe to
  /// hand off to a background thread with no further `proto`/`module`
  /// access needed. `Err(reason)` means `proto` is permanently
  /// ineligible (see `codegen::compile`'s own eligibility scan) -- the
  /// caller marks it as such and never asks again. `speculative_params`
  /// is passed straight through to `codegen::compile` -- see its own
  /// docs.
  pub fn build_ir(
    &mut self,
    proto: &ObjFunction,
    speculative_params: Option<u64>,
  ) -> Result<PendingCompile, String> {
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

    // A FRESH `Context` per function, not a single reused field --
    // once built, this `Context` (holding the finished IR) is MOVED to
    // the background compiler thread, so nothing here can be shared
    // across compilations the way the old single-body design reused
    // one `Context`/cleared it after each `define_function`.
    let mut ctx = self.module.make_context();
    ctx.func.signature = sig;
    ctx.func.name = UserFuncName::user(0, func_id.as_u32());

    let osr_ids = {
      let mut builder =
        cranelift_frontend::FunctionBuilder::new(&mut ctx.func, &mut self.builder_ctx);
      let osr_ids = codegen::compile(
        &mut builder,
        &mut self.module,
        &self.helper_ids,
        proto,
        speculative_params,
      )?;
      builder.seal_all_blocks();
      builder.finalize(self.module.target_config());
      osr_ids
    };

    if crate::jit::log_ir_enabled() {
      eprintln!("[jit] IR for '{}':\n{}", proto.name, ctx.func.display());
    }

    Ok(PendingCompile {
      ctx,
      func_id,
      osr_ids,
    })
  }

  /// Stage 2 of compiling a function: install ALREADY BACKEND-COMPILED
  /// machine code (produced by `jit::background`'s compiler thread
  /// running `Context::compile` on a `PendingCompile.ctx`) into
  /// `module` and finalize it. Pure bookkeeping -- memcpy into the
  /// module's executable memory plus relocation fixups, no register
  /// allocation -- so this is cheap enough to run synchronously on the
  /// VM's own thread every time a background result is drained (see
  /// `VM::drain_jit_results`).
  pub fn install_compiled(
    &mut self,
    func_id: FuncId,
    alignment: u64,
    bytes: &[u8],
    relocs: &[ModuleReloc],
  ) -> Result<EntryFn, String> {
    self
      .module
      .define_function_bytes(func_id, alignment, bytes, relocs)
      .map_err(|e| format!("failed to install compiled function: {e}"))?;
    self
      .module
      .finalize_definitions()
      .map_err(|e| format!("failed to finalize: {e}"))?;

    let code_ptr = self.module.get_finalized_function(func_id);
    // SAFETY: `code_ptr` was just produced by compiling a function with
    // EXACTLY the signature `EntryFn` describes (I64, I64, I64, I32 ->
    // I64, matching `build_ir`'s `sig`), for the host's native calling
    // convention (via `cranelift_native::builder()` in `JitEngine::new`) --
    // the same convention `extern "C"` uses on every platform this
    // targets.
    let entry: EntryFn = unsafe { std::mem::transmute::<*const u8, EntryFn>(code_ptr) };
    Ok(entry)
  }
}
