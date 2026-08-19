//! Backing storage for `Obj::List`.
//!
//! This is a hand-rolled `Vec<Value>` rather than `Vec` itself or a
//! `SmallVec`, and the reason is entirely about the JIT: `#[repr(C)]`
//! with `ptr` first and `len` second is a layout the generated code is
//! allowed to read directly. Every `list[i]` in compiled code used to
//! cost a real ABI call into `jit::runtime::zuri_jit_list_data` just to
//! learn the data pointer and the length -- in the innermost loop of
//! every array-shaped benchmark, and worse than the call itself, an
//! opaque barrier that forced Cranelift to spill every live register
//! around it. `Vec`'s own field order is explicitly not guaranteed and
//! `SmallVec`'s inline/spilled discriminant needs interpreting, so
//! neither can be read from generated code; this can, with two loads at
//! `LIST_PTR_OFFSET` and `LIST_LEN_OFFSET`.
//!
//! `Value` is `Copy` and has no destructor, which is what keeps the
//! unsafe surface here small: growing is a realloc-and-copy with no
//! element-wise move, and dropping is a single `dealloc`.

use std::alloc::{Layout, alloc, dealloc, handle_alloc_error, realloc};
use std::ops::{Deref, DerefMut};
use std::ptr::NonNull;

use crate::vm::value::Value;

/// Byte offset of the data pointer within `ListStorage`. Mirrored as an
/// immediate in `jit::codegen`; `layout_offsets_match` locks it down.
pub const LIST_PTR_OFFSET: i32 = 0;
/// Byte offset of the element count within `ListStorage`. Same deal.
pub const LIST_LEN_OFFSET: i32 = 8;

/// A list that has never had an element pushed owns no buffer at all,
/// and `ptr` is dangling-but-aligned (never null, exactly as `Vec` does
/// it) so that `Deref` stays branchless and the JIT's inline load needs
/// no null case. `len` is 0 there, so a bounds check rejects every index
/// before the pointer is ever dereferenced.
#[repr(C)]
pub struct ListStorage {
  ptr: *mut Value,
  len: usize,
  cap: usize,
}

// The JIT reads a list's contents from generated code on the VM's own
// thread only; `Obj` is not `Sync` and this changes nothing about that.
impl ListStorage {
  #[inline]
  pub const fn new() -> ListStorage {
    ListStorage {
      ptr: NonNull::<Value>::dangling().as_ptr(),
      len: 0,
      cap: 0,
    }
  }

  pub fn with_capacity(cap: usize) -> ListStorage {
    let mut v = ListStorage::new();
    if cap > 0 {
      v.grow_to(cap);
    }
    v
  }

  #[inline]
  pub fn len(&self) -> usize {
    self.len
  }

  #[inline]
  pub fn is_empty(&self) -> bool {
    self.len == 0
  }

  #[inline]
  pub fn capacity(&self) -> usize {
    self.cap
  }

  fn layout_for(cap: usize) -> Layout {
    Layout::array::<Value>(cap).expect("zuri: list capacity overflowed the address space")
  }

  /// Reallocates the buffer to exactly `new_cap` elements. Callers are
  /// responsible for never shrinking below `len`.
  fn grow_to(&mut self, new_cap: usize) {
    debug_assert!(new_cap >= self.len);
    let new_layout = ListStorage::layout_for(new_cap);
    let new_ptr = if self.cap == 0 {
      unsafe { alloc(new_layout) }
    } else {
      let old_layout = ListStorage::layout_for(self.cap);
      unsafe { realloc(self.ptr as *mut u8, old_layout, new_layout.size()) }
    };
    if new_ptr.is_null() {
      handle_alloc_error(new_layout);
    }
    self.ptr = new_ptr as *mut Value;
    self.cap = new_cap;
  }

  /// Doubling growth, with a floor of 4 so a list built one `append` at
  /// a time doesn't realloc on each of its first few elements.
  #[cold]
  #[inline(never)]
  fn grow_for_push(&mut self) {
    let new_cap = if self.cap == 0 {
      4
    } else {
      self.cap.checked_mul(2).expect("zuri: list capacity overflow")
    };
    self.grow_to(new_cap);
  }

  pub fn reserve(&mut self, additional: usize) {
    let needed = self
      .len
      .checked_add(additional)
      .expect("zuri: list capacity overflow");
    if needed > self.cap {
      // Still at least double, so repeated `reserve(1)` stays amortized.
      let doubled = self.cap.saturating_mul(2);
      self.grow_to(needed.max(doubled).max(4));
    }
  }

  #[inline]
  pub fn push(&mut self, value: Value) {
    if self.len == self.cap {
      self.grow_for_push();
    }
    unsafe { self.ptr.add(self.len).write(value) };
    self.len += 1;
  }

  #[inline]
  pub fn pop(&mut self) -> Option<Value> {
    if self.len == 0 {
      return None;
    }
    self.len -= 1;
    Some(unsafe { self.ptr.add(self.len).read() })
  }

  pub fn insert(&mut self, index: usize, value: Value) {
    assert!(index <= self.len, "zuri: list insert index out of bounds");
    if self.len == self.cap {
      self.grow_for_push();
    }
    unsafe {
      let at = self.ptr.add(index);
      std::ptr::copy(at, at.add(1), self.len - index);
      at.write(value);
    }
    self.len += 1;
  }

  pub fn remove(&mut self, index: usize) -> Value {
    assert!(index < self.len, "zuri: list remove index out of bounds");
    unsafe {
      let at = self.ptr.add(index);
      let out = at.read();
      std::ptr::copy(at.add(1), at, self.len - index - 1);
      self.len -= 1;
      out
    }
  }

  #[inline]
  pub fn clear(&mut self) {
    self.len = 0;
  }

  #[inline]
  pub fn truncate(&mut self, len: usize) {
    if len < self.len {
      self.len = len;
    }
  }

  pub fn extend_from_slice(&mut self, other: &[Value]) {
    self.reserve(other.len());
    unsafe {
      std::ptr::copy_nonoverlapping(other.as_ptr(), self.ptr.add(self.len), other.len());
    }
    self.len += other.len();
  }

  /// Removes `range` and hands back the removed elements. Collected
  /// eagerly into a `Vec` rather than returned as a lazy iterator: the
  /// borrow gymnastics a real `Drain` needs buy nothing here (every
  /// caller consumes it immediately) and would be a leak hazard.
  pub fn drain<R: std::ops::RangeBounds<usize>>(&mut self, range: R) -> std::vec::IntoIter<Value> {
    use std::ops::Bound;
    let start = match range.start_bound() {
      Bound::Included(&n) => n,
      Bound::Excluded(&n) => n + 1,
      Bound::Unbounded => 0,
    };
    let end = match range.end_bound() {
      Bound::Included(&n) => n + 1,
      Bound::Excluded(&n) => n,
      Bound::Unbounded => self.len,
    };
    assert!(start <= end && end <= self.len, "zuri: list drain range out of bounds");
    let removed: Vec<Value> = self[start..end].to_vec();
    unsafe {
      std::ptr::copy(self.ptr.add(end), self.ptr.add(start), self.len - end);
    }
    self.len -= end - start;
    removed.into_iter()
  }
}

impl Drop for ListStorage {
  fn drop(&mut self) {
    if self.cap != 0 {
      unsafe { dealloc(self.ptr as *mut u8, ListStorage::layout_for(self.cap)) };
    }
  }
}

impl Default for ListStorage {
  fn default() -> ListStorage {
    ListStorage::new()
  }
}

impl Clone for ListStorage {
  fn clone(&self) -> ListStorage {
    let mut out = ListStorage::with_capacity(self.len);
    out.extend_from_slice(self);
    out
  }
}

impl Deref for ListStorage {
  type Target = [Value];

  #[inline]
  fn deref(&self) -> &[Value] {
    unsafe { std::slice::from_raw_parts(self.ptr, self.len) }
  }
}

impl DerefMut for ListStorage {
  #[inline]
  fn deref_mut(&mut self) -> &mut [Value] {
    unsafe { std::slice::from_raw_parts_mut(self.ptr, self.len) }
  }
}

impl Extend<Value> for ListStorage {
  fn extend<I: IntoIterator<Item = Value>>(&mut self, iter: I) {
    let it = iter.into_iter();
    self.reserve(it.size_hint().0);
    for v in it {
      self.push(v);
    }
  }
}

impl<'a> Extend<&'a Value> for ListStorage {
  fn extend<I: IntoIterator<Item = &'a Value>>(&mut self, iter: I) {
    self.extend(iter.into_iter().copied());
  }
}

impl FromIterator<Value> for ListStorage {
  fn from_iter<I: IntoIterator<Item = Value>>(iter: I) -> ListStorage {
    let mut out = ListStorage::new();
    out.extend(iter);
    out
  }
}

impl From<Vec<Value>> for ListStorage {
  fn from(v: Vec<Value>) -> ListStorage {
    let mut out = ListStorage::with_capacity(v.len());
    out.extend_from_slice(&v);
    out
  }
}

impl From<&[Value]> for ListStorage {
  fn from(v: &[Value]) -> ListStorage {
    let mut out = ListStorage::with_capacity(v.len());
    out.extend_from_slice(v);
    out
  }
}

impl<const N: usize> From<[Value; N]> for ListStorage {
  fn from(v: [Value; N]) -> ListStorage {
    ListStorage::from(&v[..])
  }
}

impl IntoIterator for ListStorage {
  type Item = Value;
  type IntoIter = std::vec::IntoIter<Value>;

  fn into_iter(self) -> Self::IntoIter {
    self.to_vec().into_iter()
  }
}

impl<'a> IntoIterator for &'a ListStorage {
  type Item = &'a Value;
  type IntoIter = std::slice::Iter<'a, Value>;

  fn into_iter(self) -> Self::IntoIter {
    self.iter()
  }
}

impl std::fmt::Debug for ListStorage {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_list().entries(self.iter()).finish()
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  /// The JIT bakes these two offsets as immediates; a field reorder here
  /// would silently produce wrong list reads in compiled code only.
  #[test]
  fn layout_offsets_match() {
    let s = ListStorage::new();
    let base = &s as *const ListStorage as usize;
    assert_eq!(
      (&s.ptr as *const _ as usize) - base,
      LIST_PTR_OFFSET as usize
    );
    assert_eq!(
      (&s.len as *const _ as usize) - base,
      LIST_LEN_OFFSET as usize
    );
  }

  #[test]
  fn push_pop_grow() {
    let mut s = ListStorage::new();
    for i in 0..1000 {
      s.push(Value::number(i as f64));
    }
    assert_eq!(s.len(), 1000);
    assert_eq!(s[999].as_number(), 999.0);
    for i in (0..1000).rev() {
      assert_eq!(s.pop().unwrap().as_number(), i as f64);
    }
    assert!(s.pop().is_none());
  }

  #[test]
  fn insert_remove_drain() {
    let mut s: ListStorage = (0..5).map(|i| Value::number(i as f64)).collect();
    s.insert(0, Value::number(9.0));
    assert_eq!(s[0].as_number(), 9.0);
    assert_eq!(s.len(), 6);
    assert_eq!(s.remove(0).as_number(), 9.0);
    let taken: Vec<f64> = s.drain(1..3).map(|v| v.as_number()).collect();
    assert_eq!(taken, vec![1.0, 2.0]);
    let rest: Vec<f64> = s.iter().map(|v| v.as_number()).collect();
    assert_eq!(rest, vec![0.0, 3.0, 4.0]);
  }
}
