//! A set of declarations: the types, functions, variables and constants
//! read from C or Rust source, and what binding them to a library makes
//! of them.
//!
//! One scope collects any number of sources, in C and in Rust, and each
//! sees what the ones before it declared. A scope can include others,
//! whose names it can then use but does not own, which is how two
//! libraries share one set of common types.

use std::sync::{Arc, Mutex};

use rustc_hash::FxHashMap;

use super::types::{Signature, TypeRef, builtin};

pub type ScopeRef = Arc<Mutex<Scope>>;

#[derive(Clone)]
pub enum Constant {
  Int(i128),
  Float(f64),
  Str(String),
  /// A constant cast to a pointer type, as headers spell a sentinel
  /// address: `((void *) -1)`, `((sqlite3_destructor_type) -1)`.
  Pointer {
    ty: TypeRef,
    address: usize,
  },
}

#[derive(Clone)]
pub struct FunctionDecl {
  pub name: String,
  /// The name the library exports it under, which an `asm` label or a
  /// Rust `link_name` can make differ from `name`.
  pub symbol: String,
  pub sig: Arc<Signature>,
}

#[derive(Clone)]
pub struct VariableDecl {
  pub name: String,
  pub symbol: String,
  pub ty: TypeRef,
}

/// A preprocessor macro.
#[derive(Clone)]
pub enum Macro {
  /// `#define NAME tokens`, expanded wherever `NAME` appears.
  Object(Vec<super::cdecl::Token>),
  /// `#define NAME(args) ...`, which is recorded so that using it can
  /// be reported, and is never expanded.
  Function,
}

#[derive(Default)]
pub struct Scope {
  /// Ordinary type names: C typedefs and Rust type names.
  pub typedefs: FxHashMap<String, TypeRef>,
  /// Tagged C types, keyed with their keyword: `struct stat`.
  pub tags: FxHashMap<String, TypeRef>,
  pub functions: Vec<FunctionDecl>,
  pub variables: Vec<VariableDecl>,
  pub constants: Vec<(String, Constant)>,
  pub macros: FxHashMap<String, Macro>,
  /// Object-like macros that did not evaluate where they were defined,
  /// tried again once the source they came in has been read.
  pub deferred: Vec<String>,
  pub includes: Vec<ScopeRef>,
  /// `#pragma pack` state.
  pub pack: Option<usize>,
  pub pack_stack: Vec<Option<usize>>,
}

impl Scope {
  pub fn new() -> ScopeRef {
    Arc::new(Mutex::new(Scope::default()))
  }

  pub fn typedef(&self, name: &str) -> Option<TypeRef> {
    if let Some(t) = self.typedefs.get(name) {
      return Some(t.clone());
    }

    for inc in &self.includes {
      if let Some(t) = inc.lock().unwrap().typedef(name) {
        return Some(t);
      }
    }

    None
  }

  /// A type name as C code would write it without a keyword: a typedef
  /// here or in an included scope, or a built-in name.
  pub fn type_name(&self, name: &str) -> Option<TypeRef> {
    self.typedef(name).or_else(|| {
      if name.contains(' ') || matches!(name, "string" | "wstring" | "rust char") {
        None
      } else {
        builtin(name)
      }
    })
  }

  pub fn tag(&self, key: &str) -> Option<TypeRef> {
    if let Some(t) = self.tags.get(key) {
      return Some(t.clone());
    }

    for inc in &self.includes {
      if let Some(t) = inc.lock().unwrap().tag(key) {
        return Some(t);
      }
    }

    None
  }

  pub fn constant(&self, name: &str) -> Option<Constant> {
    if let Some((_, c)) = self.constants.iter().rev().find(|(n, _)| n == name) {
      return Some(c.clone());
    }

    for inc in &self.includes {
      if let Some(c) = inc.lock().unwrap().constant(name) {
        return Some(c);
      }
    }

    None
  }

  pub fn macro_named(&self, name: &str) -> Option<Macro> {
    if let Some(m) = self.macros.get(name) {
      return Some(m.clone());
    }

    for inc in &self.includes {
      if let Some(m) = inc.lock().unwrap().macro_named(name) {
        return Some(m);
      }
    }

    None
  }

  pub fn add_constant(&mut self, name: &str, value: Constant) {
    self.constants.retain(|(n, _)| n != name);
    self.constants.push((name.to_string(), value));
  }

  pub fn add_function(&mut self, decl: FunctionDecl) {
    self.functions.retain(|f| f.name != decl.name);
    self.functions.push(decl);
  }

  pub fn add_variable(&mut self, decl: VariableDecl) {
    self.variables.retain(|v| v.name != decl.name);
    self.variables.push(decl);
  }
}

/// Whether including `other` into `scope` would make a scope include
/// itself.
pub fn would_cycle(scope: &ScopeRef, other: &ScopeRef) -> bool {
  if Arc::ptr_eq(scope, other) {
    return true;
  }

  let includes = other.lock().unwrap().includes.clone();
  includes.iter().any(|inc| would_cycle(scope, inc))
}
