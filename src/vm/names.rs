//! Interned names.
//!
//! A class's method table, field-slot table and static-slot table are all
//! keyed by name, and looking one up used to mean hashing a `String` and
//! then `memcmp`-ing it to confirm the match. On a method-dispatch-heavy
//! workload that was around a fifth of total runtime; `HashMap<String,
//! _>::get` plus `__memcmp_avx2_movbe`; for a question that is really
//! just "which of this class's members is this".
//!
//! Interning turns the key into a `u32`, so the hash is a multiply and
//! the comparison is a single integer compare.
//!
//! The part that makes this an actual win rather than a relocation of the
//! same cost is that nothing interns at lookup time: every name in the
//! bytecode is a constant-pool entry reached by index (`name_const`), so
//! `Chunk` resolves each one to its `NameId` ONCE and later lookups just
//! index an array. Interning a `&str` is still available for the paths
//! that genuinely have only a string in hand, and it is the slow path.

use std::sync::{Mutex, OnceLock};

use rustc_hash::FxHashMap;

/// An interned name. Unique per distinct string for the life of the
/// process, so equality is `u32` equality.
///
/// `u32` rather than `usize` deliberately: these sit in `ObjClass`'s
/// tables and in `Chunk`, and half the width means half the cache
/// footprint for the same information.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, PartialOrd, Ord)]
pub struct NameId(pub u32);

impl NameId {
  /// Stands for "this constant is not a string", so `Chunk`'s parallel
  /// name-id array can be dense without an `Option` per entry. No real
  /// name ever gets this id; the table would have to hold 4 billion
  /// distinct names first.
  pub const NONE: NameId = NameId(u32::MAX);

  #[inline]
  pub fn is_none(self) -> bool {
    self == NameId::NONE
  }
}

struct Interner {
  ids: FxHashMap<&'static str, NameId>,
  names: Vec<&'static str>,
}

/// Behind a `Mutex` because `jit::background`'s compiler thread exists,
/// not because this is contended: interning happens once per distinct
/// name per chunk, at compile time, never on a dispatch path. `resolve`
/// hands out `&'static str` precisely so that reading a name back needs
/// no lock at all.
fn interner() -> &'static Mutex<Interner> {
  static INTERNER: OnceLock<Mutex<Interner>> = OnceLock::new();
  INTERNER.get_or_init(|| {
    Mutex::new(Interner {
      ids: FxHashMap::default(),
      names: Vec::new(),
    })
  })
}

/// The id for `name`, assigning one if this is the first time it has been
/// seen.
///
/// Leaks the string on first sight. That is bounded by the number of
/// distinct identifiers in the program; a few thousand at most, and
/// fixed once everything is loaded; and it is what lets `resolve`
/// return a `&'static str` with no lifetime plumbing through `ObjClass`.
pub fn intern(name: &str) -> NameId {
  let mut interner = interner().lock().expect("zuri: name interner poisoned");
  if let Some(&id) = interner.ids.get(name) {
    return id;
  }
  let leaked: &'static str = Box::leak(name.to_owned().into_boxed_str());
  let id =
    NameId(u32::try_from(interner.names.len()).expect("zuri: more than u32::MAX distinct names"));
  interner.names.push(leaked);
  interner.ids.insert(leaked, id);
  id
}

/// The string `id` was interned from. Only for error messages and
/// debugging; nothing on a dispatch path needs it.
pub fn resolve(id: NameId) -> &'static str {
  if id.is_none() {
    return "<not a name>";
  }
  let interner = interner().lock().expect("zuri: name interner poisoned");
  interner.names[id.0 as usize]
}

/// The id `name` was interned under, or `None` if it never was.
///
/// Distinct from `intern` for lookup paths that must not grow the table:
/// asking a class for a member it does not have is completely ordinary
/// (it is how `builtins::lookup` gets its turn), and interning every
/// missed name would let a program with computed member names grow this
/// table without bound.
pub fn lookup(name: &str) -> Option<NameId> {
  let interner = interner().lock().expect("zuri: name interner poisoned");
  interner.ids.get(name).copied()
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn same_string_same_id() {
    let a = intern("increment");
    let b = intern("increment");
    assert_eq!(a, b);
    assert_ne!(a, intern("decrement"));
    assert_eq!(resolve(a), "increment");
  }

  #[test]
  fn lookup_does_not_intern() {
    assert!(lookup("a name never interned anywhere else").is_none());
    let id = intern("deliberately interned");
    assert_eq!(lookup("deliberately interned"), Some(id));
  }

  #[test]
  fn none_is_distinguishable() {
    assert!(NameId::NONE.is_none());
    assert!(!intern("real").is_none());
  }
}
