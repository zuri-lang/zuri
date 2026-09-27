//! Native memory and the pointers into it.
//!
//! A pointer is an address, an optional element type, and optionally
//! the block it points into. A block is memory whose extent is known:
//! everything `ffi.alloc` and `ffi.malloc` hand out, and anything a
//! program has taken ownership of with `own()`. Access through a pointer
//! that has a block is checked against the block's bounds and against
//! it having been freed. A pointer C handed back has no block, so only
//! null is caught; its extent is whatever the C library says it is.
//!
//! Blocks are reference counted, and every pointer derived from one
//! (an offset, a field, a cast) shares it. Memory `ffi.alloc` made is
//! released when the last pointer into it is collected, which is why a
//! program must keep a pointer reachable for as long as C holds the
//! address.

use std::alloc::{Layout, alloc_zeroed, dealloc};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use super::Fail;
use super::call::ForeignFunction;
use super::callback;
use super::types::TypeRef;
use crate::vm::vm::VM;

/// Who releases a block, and how.
pub enum Owner {
  /// `ffi.alloc`: this module's own allocation, freed with the layout it
  /// was made with.
  Module(Layout),
  /// `ffi.malloc`: the C allocator. Freed only when the program says so,
  /// because the usual reason to use it is to hand the memory to C.
  Malloc,
  /// `own()`: released by calling a C function with the address.
  Destructor(Arc<ForeignFunction>),
  /// `own()` with no function: released with the C allocator's `free`.
  CFree,
}

pub struct Block {
  pub base: usize,
  /// `None` for memory taken over with `own()`, whose extent this
  /// module was never told.
  pub size: Option<usize>,
  owner: Owner,
  freed: AtomicBool,
}

impl Block {
  pub fn allocate(size: usize, align: usize) -> Result<Arc<Block>, Fail> {
    let align = align.max(16);
    let layout = Layout::from_size_align(size.max(1), align)
      .map_err(|_| Fail::range(format!("cannot allocate {size} bytes aligned to {align}")))?;

    // SAFETY: the layout has a non-zero size.
    let base = unsafe { alloc_zeroed(layout) } as usize;

    if base == 0 {
      return Err(Fail::ffi(format!("out of memory allocating {size} bytes")));
    }

    Ok(Arc::new(Block {
      base,
      size: Some(size),
      owner: Owner::Module(layout),
      freed: AtomicBool::new(false),
    }))
  }

  pub fn malloc(size: usize) -> Result<Arc<Block>, Fail> {
    // SAFETY: calloc with a non-zero count is always safe to call.
    let base = unsafe { libc::calloc(size.max(1), 1) } as usize;

    if base == 0 {
      return Err(Fail::ffi(format!("out of memory allocating {size} bytes")));
    }

    Ok(Arc::new(Block {
      base,
      size: Some(size),
      owner: Owner::Malloc,
      freed: AtomicBool::new(false),
    }))
  }

  /// Takes over memory C allocated, to be released by `destructor`, or
  /// by `free` when there is none.
  pub fn adopt(
    base: usize,
    size: Option<usize>,
    destructor: Option<Arc<ForeignFunction>>,
  ) -> Arc<Block> {
    Arc::new(Block {
      base,
      size,
      owner: match destructor {
        Some(f) => Owner::Destructor(f),
        None => Owner::CFree,
      },
      freed: AtomicBool::new(false),
    })
  }

  pub fn is_freed(&self) -> bool {
    self.freed.load(Ordering::Acquire)
  }

  /// Releases the memory now. A second call is a `PointerError`, not a
  /// double free.
  ///
  /// A destructor runs as any foreign call on `vm` does, so the
  /// callbacks it makes run, and an error one raises is raised here.
  pub fn free(&self, vm: &mut VM) -> Result<(), Fail> {
    if self.freed.swap(true, Ordering::AcqRel) {
      return Err(Fail::pointer("this memory has already been freed"));
    }

    let outcome = {
      let _guard = callback::enter(vm);
      self.release()
    };

    if let Some(error) = callback::take_trapped(vm) {
      return Err(Fail::Raised(error));
    }

    outcome
  }

  fn release(&self) -> Result<(), Fail> {
    match &self.owner {
      // SAFETY: `base` came from `alloc_zeroed` with exactly this layout
      // and the `freed` flag guarantees this runs once.
      Owner::Module(layout) => unsafe { dealloc(self.base as *mut u8, *layout) },
      // SAFETY: `base` came from `calloc`, or was adopted as memory C
      // allocated with the C allocator.
      Owner::Malloc | Owner::CFree => unsafe { libc::free(self.base as *mut libc::c_void) },
      Owner::Destructor(f) => f.call_destructor(self.base)?,
    }
    Ok(())
  }
}

impl Drop for Block {
  fn drop(&mut self) {
    if self.freed.load(Ordering::Acquire) {
      return;
    }

    // `ffi.malloc` memory is the program's to free, or C's; dropping the
    // last pointer to it from Zuri says nothing about whether C still
    // has it.
    if matches!(self.owner, Owner::Malloc) {
      return;
    }

    self.freed.store(true, Ordering::Release);
    let _ = callback::releasing(|| self.release());
  }
}

/// What a `Pointer` wraps.
#[derive(Clone)]
pub struct PointerData {
  pub address: usize,
  /// The type `get()` and `set()` read and write, and the unit `add()`
  /// steps by. `None` for an untyped pointer.
  pub element: Option<TypeRef>,
  pub block: Option<Arc<Block>>,
}

impl PointerData {
  pub fn raw(address: usize, element: Option<TypeRef>) -> PointerData {
    PointerData {
      address,
      element,
      block: None,
    }
  }

  pub fn is_null(&self) -> bool {
    self.address == 0
  }

  /// Bytes left between this address and the end of its block, when
  /// the block's extent is known.
  pub fn remaining(&self) -> Option<usize> {
    let block = self.block.as_ref()?;
    let size = block.size?;
    Some((block.base + size).saturating_sub(self.address))
  }

  /// Confirms that `len` bytes at `offset` from this pointer may be
  /// touched, and returns their address.
  pub fn check(&self, offset: isize, len: usize) -> Result<usize, Fail> {
    if self.address == 0 {
      return Err(Fail::pointer("cannot read or write through a null pointer"));
    }

    let address = (self.address as isize).checked_add(offset).ok_or_else(|| {
      Fail::pointer(format!(
        "offset {offset} moves the pointer outside the address space"
      ))
    })? as usize;

    if let Some(block) = &self.block {
      if block.is_freed() {
        return Err(Fail::pointer("this memory has been freed"));
      }

      if let Some(size) = block.size {
        let end = block.base + size;
        if address < block.base || address.checked_add(len).is_none_or(|e| e > end) {
          return Err(Fail::pointer(format!(
            "{len} bytes at offset {} fall outside the {size}-byte block this pointer belongs to",
            address as isize - block.base as isize
          )));
        }
      }
    }

    Ok(address)
  }

  /// A pointer `offset` bytes along, sharing this one's block.
  pub fn offset_by(&self, offset: isize, element: Option<TypeRef>) -> Result<PointerData, Fail> {
    let address = (self.address as isize)
      .checked_add(offset)
      .filter(|a| *a >= 0)
      .ok_or_else(|| {
        Fail::pointer(format!(
          "offset {offset} moves the pointer outside the address space"
        ))
      })?;

    Ok(PointerData {
      address: address as usize,
      element,
      block: self.block.clone(),
    })
  }
}

/// Copies `len` bytes out of native memory.
///
/// # Safety
///
/// `address` must be readable for `len` bytes.
pub unsafe fn read_bytes(address: usize, len: usize) -> Vec<u8> {
  let mut out = vec![0u8; len];
  unsafe { std::ptr::copy_nonoverlapping(address as *const u8, out.as_mut_ptr(), len) };
  out
}

/// The length of the NUL-terminated run of `unit`-byte code units at
/// `address`, in units, looking no further than `limit` units.
///
/// # Safety
///
/// `address` must be readable up to and including the terminator, or
/// for `limit` units when there is a limit.
pub unsafe fn terminated_len(address: usize, unit: usize, limit: Option<usize>) -> usize {
  let mut n = 0usize;

  loop {
    if limit.is_some_and(|l| n >= l) {
      return n;
    }

    let at = address + n * unit;
    let zero = unsafe {
      match unit {
        1 => *(at as *const u8) == 0,
        2 => (at as *const u16).read_unaligned() == 0,
        _ => (at as *const u32).read_unaligned() == 0,
      }
    };

    if zero {
      return n;
    }

    n += 1;
  }
}
