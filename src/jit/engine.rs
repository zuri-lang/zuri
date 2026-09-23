//! Owns the Cranelift `JITModule` and turns one `&ObjFunction` into a
//! `CompiledFunction`. See `crate::jit`'s module docs for the overall
//! architecture; this file is pure Cranelift/`cranelift_module`
//! plumbing; the actual bytecode -> IR translation lives in
//! `jit::codegen`.

use std::sync::Arc;

use cranelift_codegen::Context;
use cranelift_codegen::ir::{AbiParam, UserFuncName, types};
use cranelift_codegen::isa::TargetIsa;
use cranelift_codegen::settings::{self, Configurable};
use cranelift_frontend::FunctionBuilderContext;
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::{FuncId, Linkage, Module, ModuleReloc};
use rustc_hash::{FxBuildHasher, FxHashMap};

use crate::jit::runtime;
use crate::jit::{CompileFacts, EntryFn, codegen, typeflow};
use crate::vm::object::ObjFunction;

/// Everything `JitEngine::build_ir` extracts from a `&ObjFunction`;
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
  /// A second handle onto the same target configuration `module` was
  /// built with (not the module's own internal ISA; a fully separate,
  /// independently-constructed instance). `Arc<dyn TargetIsa>` is
  /// `Send + Sync`, so cloning this out to `jit::background`'s compiler
  /// thread lets it run `Context::compile` (the expensive register-
  /// allocation/encoding step) with no access to `module` at all, no
  /// locking needed. See `isa_handle`.
  isa: Arc<dyn TargetIsa>,
  /// Every `jit::runtime` helper's `FuncId`, keyed by its registered
  /// name; declared once, up front, and reused (via
  /// `Module::declare_func_in_func`) by every subsequent function this
  /// engine compiles.
  helper_ids: FxHashMap<&'static str, FuncId>,
  /// `ZURI_JIT_LOG_ASM`'s `FuncId` -> readable name map, so the dumped
  /// machine code lands under the Zuri function's own name rather than
  /// a module-local id that restarts per compiler worker.
  asm_labels: FxHashMap<FuncId, String>,
  /// Monotonic counter giving every compiled function a distinct
  /// module-local symbol name (`cranelift_module::Module` requires
  /// unique names for `declare_function`); purely an internal detail,
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
    // `JITBuilder::with_flags`); required on at least AArch64, where
    // "colocated" calls use shorter-range relocations that can't reach
    // every definition in a JIT's address space.
    flag_builder.set("use_colocated_libcalls", "false").unwrap();
    flag_builder.set("is_pic", "false").unwrap();
    flag_builder.set("enable_alias_analysis", "true").unwrap();
    // Cranelift re-verifies every function it compiles, which showed up
    // as ~9% of a closure-heavy workload's total runtime; all of it on
    // the compiler thread, checking IR this compiler just built. That is
    // a development check, so it stays on in debug builds (where a
    // malformed-IR bug should surface as a clear verifier error rather
    // than as a miscompile) and off in release.
    #[cfg(not(debug_assertions))]
    flag_builder.set("enable_verifier", "false").unwrap();
    flag_builder.set("opt_level", "speed").unwrap();
    // The backtracking allocator produces measurably better code
    // (fewer spills/moves) than the single-pass one, at the cost of
    // more compile time; a trade that only became strictly correct
    // to make once compilation moved off the interpreter's own thread
    // (see `jit::background`): there is no longer a reason to economize
    // on compile time by settling for the cheaper allocator.
    flag_builder
      .set("regalloc_algorithm", "backtracking")
      .unwrap();

    let isa_builder = cranelift_native::builder().unwrap_or_else(|msg| {
      panic!("zuri: host machine is not supported by the JIT backend: {msg}")
    });
    let isa = isa_builder
      .finish(settings::Flags::new(flag_builder))
      .expect("zuri: failed to build a target ISA for the JIT");
    // Kept alongside (not just inside) `module`: see `isa`'s own
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

    let mut helper_ids = FxHashMap::with_capacity_and_hasher(specs.len(), FxBuildHasher::default());
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
      asm_labels: FxHashMap::default(),
      next_id: 0,
    }
  }

  /// Puts the engine back in a usable state after a compile panicked
  /// partway through. The builder context may still hold the abandoned
  /// function's blocks and variables, and `FunctionBuilder` expects to
  /// start from an empty one. Code already installed in `module` is
  /// untouched, since compiled functions elsewhere still run from it.
  pub fn recover_from_panic(&mut self) {
    self.builder_ctx = FunctionBuilderContext::new();
  }

  /// A clone of this engine's target ISA handle, independent of
  /// `module`: see `isa`'s own field docs. Cheap (an `Arc` bump).
  pub fn isa_handle(&self) -> Arc<dyn TargetIsa> {
    self.isa.clone()
  }

  /// Fully compiles `proto` from bytecode to native machine code and
  /// finalizes it into executable memory in `module`, returning the
  /// callable entry point and any loop OSR entry points. Runs
  /// entirely on the dedicated background compiler worker thread.
  pub fn compile_function(
    &mut self,
    proto: &ObjFunction,
    speculative_params: Option<u64>,
    speculative_regs: Option<typeflow::SpeculativeRegs>,
    facts: CompileFacts,
    shutdown: Option<&std::sync::atomic::AtomicBool>,
  ) -> Result<(EntryFn, FxHashMap<usize, i32>), String> {
    let mut pending =
      self.build_ir(proto, speculative_params, speculative_regs, facts, shutdown)?;
    if let Some(shutdown) = shutdown {
      if shutdown.load(std::sync::atomic::Ordering::Relaxed) {
        return Err("compilation aborted: VM shutdown".to_string());
      }
    }
    let mut ctrl_plane = cranelift_codegen::control::ControlPlane::default();
    let compile_result = pending.ctx.compile(&*self.isa, &mut ctrl_plane);
    let (bytes, alignment, relocs) = match compile_result {
      Ok(_) => {
        let compiled_code = pending
          .ctx
          .compiled_code()
          .expect("Context::compile just succeeded");
        let alignment = compiled_code.buffer.alignment as u64;
        let bytes = compiled_code.code_buffer().to_vec();
        let relocs: Vec<ModuleReloc> = compiled_code
          .buffer
          .relocs()
          .iter()
          .map(|r| ModuleReloc::from_mach_reloc(r, &pending.ctx.func, pending.func_id))
          .collect();
        (bytes, alignment, relocs)
      },
      Err(e) => {
        // In a debug build the verifier runs, and a rejection means this
        // compiler emitted malformed IR. Say so on stderr, where every
        // fixture's expected output would catch it, instead of letting
        // the function drop quietly back to the interpreter. Release
        // builds skip the verifier and never get here for this reason.
        #[cfg(debug_assertions)]
        if let cranelift_codegen::CodegenError::Verifier(errors) = &e.inner {
          eprintln!(
            "zuri: JIT emitted invalid IR for '{}': {errors}",
            proto.display_name()
          );
        }
        return Err(format!("backend compile failed: {:?}", e.inner));
      },
    };
    let entry = self.install_compiled(pending.func_id, alignment, &bytes, &relocs)?;
    Ok((entry, std::mem::take(&mut pending.osr_ids)))
  }

  /// Stage 1 of compiling `proto`: translate its bytecode to Cranelift
  /// IR (`jit::codegen`'s job) and declare a slot for it in `module`.
  /// This is the only stage that touches `proto`, so it must run
  /// synchronously on the VM's own thread: see `jit::background`'s
  /// module docs for why everything after this point (the actual
  /// register-allocation/encoding work, `Context::compile`) is safe to
  /// hand off to a background thread with no further `proto`/`module`
  /// access needed. `Err(reason)` means `proto` is permanently
  /// ineligible (see `codegen::compile`'s own eligibility scan); the
  /// caller marks it as such and never asks again. `speculative_params`
  /// is passed straight through to `codegen::compile`: see its own
  /// docs.
  pub fn build_ir(
    &mut self,
    proto: &ObjFunction,
    speculative_params: Option<u64>,
    speculative_regs: Option<typeflow::SpeculativeRegs>,
    facts: CompileFacts,
    shutdown: Option<&std::sync::atomic::AtomicBool>,
  ) -> Result<PendingCompile, String> {
    self.next_id += 1;
    let name = format!("zuri_fn_{}", self.next_id);

    let mut sig = self.module.make_signature();
    sig.params.push(AbiParam::new(types::I64)); // vm: *mut VM
    sig.params.push(AbiParam::new(types::I64)); // base: u64
    sig.params.push(AbiParam::new(types::I64)); // closure: u64 (tagged Value bits)
    sig.params.push(AbiParam::new(types::I32)); // osr_id: i32 (-1 = ordinary entry)
    sig.params.push(AbiParam::new(types::I64)); // a0
    sig.params.push(AbiParam::new(types::I64)); // a1
    sig.params.push(AbiParam::new(types::I64)); // a2
    sig.params.push(AbiParam::new(types::I64)); // a3
    sig.returns.push(AbiParam::new(types::I64)); // return value bits

    let func_id = self
      .module
      .declare_function(&name, Linkage::Local, &sig)
      .map_err(|e| format!("failed to declare function: {e}"))?;

    // A fresh `Context` per function, not a single reused field: once
    // built, this `Context` (holding the finished IR) is moved to the
    // background compiler thread, so nothing here can be shared across
    // compilations the way a single reused-and-cleared `Context` would
    // be.
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
        func_id,
        speculative_params,
        speculative_regs,
        facts,
        shutdown,
      )?;
      builder.seal_all_blocks();
      builder.finalize(self.module.target_config());
      osr_ids
    };

    if crate::jit::log_ir_enabled() {
      eprintln!("[jit] IR for '{}':\n{}", proto.name, ctx.func.display());
    }
    if crate::jit::log_asm_enabled() {
      let label: String = proto
        .display_name()
        .chars()
        .map(|c| {
          if c.is_alphanumeric() || c == '.' {
            c
          } else {
            '_'
          }
        })
        .collect();
      self.asm_labels.insert(func_id, label);
    }

    Ok(PendingCompile {
      ctx,
      func_id,
      osr_ids,
    })
  }

  /// Stage 2 of compiling a function: install already backend-compiled
  /// machine code (produced by `jit::background`'s compiler thread
  /// running `Context::compile` on a `PendingCompile.ctx`) into
  /// `module` and finalize it. Pure bookkeeping; memcpy into the
  /// module's executable memory plus relocation fixups, no register
  /// allocation; so this is cheap enough to run synchronously on the
  /// VM's own thread every time a background result is drained (see
  /// `VM::drain_jit_results`).
  pub fn install_compiled(
    &mut self,
    func_id: FuncId,
    alignment: u64,
    bytes: &[u8],
    relocs: &[ModuleReloc],
  ) -> Result<EntryFn, String> {
    if crate::jit::log_asm_enabled() {
      let dir = std::path::Path::new("tmp/jitasm");
      let _ = std::fs::create_dir_all(dir);
      let label = match self.asm_labels.get(&func_id) {
        Some(l) => l.clone(),
        None => format!("fn{}", func_id.as_u32()),
      };
      let _ = std::fs::write(dir.join(format!("{label}.bin")), bytes);
      eprintln!(
        "[jit] asm for '{label}' -> tmp/jitasm/{label}.bin ({} bytes)",
        bytes.len()
      );
    }
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
    // exactly the signature `EntryFn` describes (I64, I64, I64, I32 ->
    // I64, matching `build_ir`'s `sig`), for the host's native calling
    // convention (via `cranelift_native::builder()` in `JitEngine::new`) --
    // the same convention `extern "C"` uses on every platform this
    // targets.
    let entry: EntryFn = unsafe { std::mem::transmute::<*const u8, EntryFn>(code_ptr) };
    Ok(entry)
  }
}
