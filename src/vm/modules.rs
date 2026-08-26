//! Module loading, resolution, and caching.
//!
//! A "module" is any `.zu` file (or a directory package with its own
//! `index.zu`) executed AT MOST ONCE per VM run and cached by its
//! canonical filesystem path, giving it a completely separate global
//! namespace from the importing script or any other module: see
//! `ObjFunction::globals_module` for how a compiled function knows
//! which namespace its own top-level `def`/`var`/`class` bindings
//! belong to.
//!
//! Resolution order for a NON-relative import (`import http.status`,
//! `import math`) mirrors the documented precedence: a user's own
//! `.zuri/libs` (relative to the current working directory) always
//! wins, then an install-root `libs` directory, then finally a small
//! set of builtin native modules. A RELATIVE import (`.`/`..` prefix)
//! is always resolved against the directory of the file doing the
//! importing, never searched for elsewhere.

use std::path::{Path, PathBuf};
use std::rc::Rc;

use crate::compiler::{compiler::Compiler, lexer::Lexer, parser::Parser};
use crate::vm::chunk::Chunk;
use crate::vm::object::{ModuleNamespace, ObjModule, write_barrier};
use crate::vm::value::Value;
use crate::vm::vm::VM;

type ImportResult = Result<Value, Value>;

/// Entry point for `Instr::Import`. `raw_path` is exactly what the
/// parser joined together (e.g. `"./sibling"`, `"../pkg/mod"`, or
/// `"http/status"`); `importer_path` is the CURRENTLY COMPILING
/// file's own path, a compile-time constant baked in by
/// `Compiler::compile_import`.
pub fn import(vm: &mut VM, importer_path: &str, raw_path: &str) -> ImportResult {
  if is_relative(raw_path) {
    import_relative(vm, importer_path, raw_path)
  } else {
    import_search(vm, raw_path)
  }
}

fn is_relative(path: &str) -> bool {
  path.starts_with('.')
}

/// Resolves and loads a `.`/`..`-prefixed import against the
/// directory containing `importer_path`. Never falls back to any
/// search path; a relative import that doesn't exist is simply not
/// found, matching every other language's module system.
fn import_relative(vm: &mut VM, importer_path: &str, raw_path: &str) -> ImportResult {
  let importer_dir = Path::new(importer_path)
    .parent()
    .map(Path::to_path_buf)
    .unwrap_or_else(|| PathBuf::from("."));

  let candidate = importer_dir.join(raw_path);
  load_from_candidate(vm, &candidate, raw_path)
}

/// Resolves a bare (non-relative) import in documented precedence
/// order: user-local `.zuri/libs` (so user code always wins), then
/// the install-root `libs`, then a builtin native module.
fn import_search(vm: &mut VM, raw_path: &str) -> ImportResult {
  if let Ok(cwd) = std::env::current_dir() {
    let user_libs = cwd.join(".zuri").join("libs").join(raw_path);
    if resolve_candidate(&user_libs).is_some() {
      return load_from_candidate(vm, &user_libs, raw_path);
    }
  }

  if let Some(root) = install_root_libs() {
    let candidate = root.join(raw_path);
    if resolve_candidate(&candidate).is_some() {
      return load_from_candidate(vm, &candidate, raw_path);
    }
  }

  if let Some(name) = single_segment(raw_path) {
    if let Some(module_val) = builtin_module(vm, name) {
      return Ok(module_val);
    }
  }

  Err(vm.raise(
    "ModuleNotFoundError",
    format!("module '{}' could not be found", raw_path),
  ))
}

/// `$ZURI_ROOT/libs`, falling back to a `libs` directory next to the
/// running executable; this implementation's equivalent of the
/// documented `%BLADE_INSTALL_ROOT%/libs`.
pub fn install_root_libs() -> Option<PathBuf> {
  if let Ok(root) = std::env::var("ZURI_ROOT") {
    return Some(PathBuf::from(root).join("libs"));
  }
  std::env::current_exe()
    .ok()
    .and_then(|p| p.parent().map(|p| p.join("libs")))
}

/// A single, non-dotted, non-separator path segment (e.g. `"math"`),
/// or `None` for anything with more than one component; only a bare
/// top-level name can ever match a builtin module.
fn single_segment(raw_path: &str) -> Option<&str> {
  let mut parts = Path::new(raw_path).components();
  let first = parts.next()?;
  if parts.next().is_some() {
    return None;
  }
  first.as_os_str().to_str()
}

/// `base.zu` if it exists, else `base/index.zu` if THAT exists, else
/// `None`; the two file shapes a module/package can take, per the
/// package-authoring rules in the docs.
fn resolve_candidate(base: &Path) -> Option<PathBuf> {
  let as_file = with_zu_extension(base);
  if as_file.is_file() {
    return Some(as_file);
  }
  let as_package = base.join("index.zu");
  if as_package.is_file() {
    return Some(as_package);
  }
  None
}

fn with_zu_extension(base: &Path) -> PathBuf {
  match base.extension() {
    Some(_) => base.to_path_buf(),
    None => base.with_extension("zu"),
  }
}

/// `<file stem>`, or for a package's `index.zu`, the ENCLOSING
/// directory's own name; so a package displays as `"http"`, not
/// `"index"`.
fn module_display_name(file_path: &Path) -> String {
  let stem = file_path
    .file_stem()
    .and_then(|s| s.to_str())
    .unwrap_or("module");
  if stem == "index" {
    file_path
      .parent()
      .and_then(|p| p.file_name())
      .and_then(|s| s.to_str())
      .unwrap_or(stem)
      .to_string()
  } else {
    stem.to_string()
  }
}

/// Resolves `base` to an actual `.zu` file, loads it (reusing the
/// cache if this exact canonical path has already been loaded), and
/// hands back its Module Value.
///
/// `pub(crate)`, not just `fn`: `modules::isolate_util` reuses this
/// directly to load a module's source into an isolate's own,
/// independent `VM`/`Heap`; the exact same load-and-cache pipeline
/// `import` uses, just invoked with an already-canonical path instead
/// of a raw import string. No new module-loading logic exists for
/// isolates; this is the only one there ever was.
pub(crate) fn load_from_candidate(vm: &mut VM, base: &Path, raw_path: &str) -> ImportResult {
  let Some(file_path) = resolve_candidate(base) else {
    return Err(vm.raise(
      "ModuleNotFoundError",
      format!("module '{}' could not be found", raw_path),
    ));
  };

  let canonical = std::fs::canonicalize(&file_path)
    .map(|p| p.display().to_string())
    .unwrap_or_else(|_| file_path.display().to_string());

  if let Some(&cached) = vm.modules.get(&canonical) {
    return Ok(cached);
  }

  let source = std::fs::read_to_string(&file_path).map_err(|e| {
    vm.raise(
      "Error",
      format!("could not read module '{}': {}", file_path.display(), e),
    )
  })?;

  let display_name = module_display_name(&file_path);
  let module_val = vm.heap.alloc_module(ObjModule {
    name: display_name,
    path: canonical.clone(),
    namespace: ModuleNamespace::new(),
    loaded: false,
  });

  // Cache BEFORE running the body; what lets a circular import (A
  // imports B, B imports A) observe this same in-progress module
  // instead of recursing forever, exactly like Python/Node handle it.
  vm.modules.insert(canonical.clone(), module_val);

  seed_module_vars(vm, module_val, &canonical);

  if let Err(e) = run_module_source(vm, module_val, &source, &canonical) {
    // A module that blew up mid-load shouldn't stay permanently
    // "cached" as broken; drop it so a later import attempt (e.g.
    // from a REPL session after the user fixes the file) gets a
    // genuine retry instead of silently reusing the half-built value.
    vm.modules.remove(&canonical);
    return Err(e);
  }

  // Re-read from `vm.modules` rather than trusting the `module_val`
  // local from before the call above: `run_module_source` executes
  // the module's entire top-level body, arbitrary Zuri code free to
  // allocate and trigger a collection; if `module_val`'s own object
  // was still Young at the `insert` above (common: this module is the
  // very first thing loaded, nothing has promoted it yet) and gets
  // relocated during its own body's execution, the pre-call local
  // would silently go stale exactly like `VM::instantiate`'s old
  // `instance_val` did. `vm.modules`'s own copy, being a real root,
  // is always current.
  let module_val = vm.modules[&canonical];
  module_val.as_module_mut().loaded = true;
  Ok(module_val)
}

/// `__file__` is this module's own canonical path; `__root__` is
/// whatever the VM was told the application's entry file is (see
/// `VM::set_root_path`); identical across every module loaded
/// during this run. Neither is defined in REPL mode.
fn seed_module_vars(vm: &mut VM, module_val: Value, canonical_path: &str) {
  let file_val = vm.heap.alloc_string(canonical_path.to_string());
  module_val
    .as_module_mut()
    .namespace
    .set("__file__", file_val);
  write_barrier(module_val.as_obj());

  if let Some(root) = vm.root_path.clone() {
    let root_val = vm.heap.alloc_string(root);
    module_val
      .as_module_mut()
      .namespace
      .set("__root__", root_val);
    write_barrier(module_val.as_obj());
  }
}

/// Lex, parse, compile, and run `source` as `module_val`'s own
/// top-level code; every `def`/`var`/`class` it declares lands in
/// `module_val`'s namespace (via `Compiler::set_current_module`),
/// never the VM's root table.
fn run_module_source(
  vm: &mut VM,
  module_val: Value,
  source: &str,
  canonical_path: &str,
) -> Result<(), Value> {
  let mut lexer = Lexer::new(source);
  let mut parser = Parser::new(&mut lexer);
  let decls = match parser.parse() {
    Ok(decls) => decls,
    Err(errors) => {
      let msg = errors
        .iter()
        .map(|e| e.to_string())
        .collect::<Vec<_>>()
        .join("\n  ");
      return Err(vm.raise(
        "Error",
        format!("failed to parse module '{}':\n  {}", canonical_path, msg),
      ));
    },
  };

  let chunk = Box::new(Chunk::new());
  let mut compiler = Compiler::new(decls, chunk, &mut vm.heap, Rc::from(canonical_path));
  compiler.set_current_module(module_val);

  let fn_obj = match compiler.compile() {
    Ok(f) => f,
    Err(errors) => {
      let msg = errors
        .iter()
        .map(|e| e.to_string())
        .collect::<Vec<_>>()
        .join("\n  ");
      return Err(vm.raise(
        "Error",
        format!("failed to compile module '{}':\n  {}", canonical_path, msg),
      ));
    },
  };

  let closure = vm.heap.alloc_plain_closure(fn_obj);
  // `call_value`, NOT `vm.run`; this executes RE-ENTRANTLY, from
  // inside an already-running `Instr::Import`, so it must extend the
  // current call stack (like any other Zuri->Zuri call) rather than
  // assuming an empty one.
  vm.call_value(closure, &[])?;
  Ok(())
}

// Builtin native modules

/// Constructs (and caches, keyed as `"builtin:NAME"`) a synthetic
/// module exposing a handful of already-registered natives under a
/// namespace.
fn builtin_module(vm: &mut VM, name: &str) -> Option<Value> {
  let cache_key = format!("builtin:{}", name);
  if let Some(&cached) = vm.modules.get(&cache_key) {
    return Some(cached);
  }

  let def = crate::modules::find(name)?;

  let module_val = vm.heap.alloc_module(ObjModule {
    name: name.to_string(),
    path: format!("<builtin:{}>", name),
    namespace: ModuleNamespace::new(),
    loaded: true,
  });
  // Cache BEFORE `(def.build)(vm)` runs; that call can allocate
  // (and therefore trigger a collection) arbitrarily, and until this
  // insert, `module_val` was a bare local nothing in `VM` roots on
  // its own, unlike `load_from_candidate`'s own module (see ITS
  // identical early-insert, right above `run_module_source`'s own
  // call, for the same reason).
  vm.modules.insert(cache_key.clone(), module_val);

  let members = (def.build)(vm);
  // Re-read rather than trust the pre-call `module_val` local: see
  // `load_from_candidate`'s identical re-read for why: `def.build`
  // is native Rust code, not Zuri, but nothing here guarantees it
  // never allocates enough to cross a collection threshold, and
  // `vm.modules`'s own copy is guaranteed current either way.
  let module_val = vm.modules[&cache_key];
  for (member, value) in members {
    module_val.as_module_mut().namespace.set(member, value);
    write_barrier(module_val.as_obj());
  }

  Some(module_val)
}
