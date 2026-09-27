//! 128-bit integers returned under the Microsoft x64 convention.
//!
//! Microsoft x64 returns a 128-bit integer in `xmm0`, all sixteen bytes
//! of it, which is what rustc, clang and GCC do for `i128` and
//! `__int128`. libffi has no type that comes back that way: it treats
//! any sixteen-byte result as a record returned through a hidden
//! pointer, which is a different function to call and a different one
//! to be.
//!
//! Two thunks bridge the difference. libffi calls `call_thunk` in place
//! of the foreign function, with a `Call` as an extra first argument; the
//! thunk moves every other argument down a place, calls the target, and
//! stores `xmm0` where the result belongs. C reaches a callback through
//! one of a fixed set of stubs instead, each of which enters a common
//! thunk that moves every argument up a place, passes a result buffer
//! first to the libffi closure, and loads `xmm0` from that buffer before
//! returning to C.
//!
//! Both thunks copy the arguments C left on the stack into a frame of
//! their own, so they only need to know how many there are. Every
//! register argument is moved as both an integer and a vector register,
//! as libffi itself loads them, so neither thunk needs the types.

use std::sync::atomic::{AtomicUsize, Ordering};

/// How many callbacks returning a 128-bit integer under this
/// convention can be alive at once.
pub const STUBS: usize = 256;

/// For each stub, the `Entry` of the callback it belongs to, or zero.
static SLOTS: [AtomicUsize; STUBS] = [const { AtomicUsize::new(0) }; STUBS];

/// What `call_thunk` takes as its first argument.
#[repr(C)]
pub struct Call {
  pub target: usize,
  pub dest: *mut u8,
  /// How many of the target's arguments go on the stack.
  pub stack_slots: usize,
}

/// What a stub's slot points at.
#[repr(C)]
struct Entry {
  /// The libffi closure, which takes the result buffer first.
  code: usize,
  /// How many of the callback's own arguments C puts on the stack.
  stack_slots: usize,
}

/// How many of `slots` arguments go on the stack: all but the first
/// four, which travel in registers.
pub fn stack_slots(slots: usize) -> usize {
  slots.saturating_sub(4)
}

unsafe extern "C" {
  fn zuri_ffi_wide_call();
  fn zuri_ffi_wide_stubs();
}

/// The address libffi calls instead of the foreign function.
pub fn call_thunk() -> usize {
  zuri_ffi_wide_call as *const () as usize
}

/// A stub C can call as the callback, released with it.
pub struct Stub {
  index: usize,
  _entry: Box<Entry>,
}

impl Stub {
  /// A stub entering `code`, the libffi closure for a callback taking
  /// `slots` arguments, or `None` when every stub is taken.
  pub fn new(code: usize, slots: usize) -> Option<Stub> {
    let entry = Box::new(Entry {
      code,
      stack_slots: stack_slots(slots),
    });
    let address = &*entry as *const Entry as usize;

    let index = SLOTS.iter().position(|slot| {
      slot
        .compare_exchange(0, address, Ordering::AcqRel, Ordering::Acquire)
        .is_ok()
    })?;

    Some(Stub {
      index,
      _entry: entry,
    })
  }

  pub fn address(&self) -> usize {
    zuri_ffi_wide_stubs as *const () as usize + self.index * 16
  }
}

impl Drop for Stub {
  fn drop(&mut self) {
    SLOTS[self.index].store(0, Ordering::Release);
  }
}

// Unwind information: SEH on Windows, CFI everywhere else.
#[cfg(windows)]
macro_rules! seh {
  ($($line:literal),*) => { concat!($($line, "\n"),*) };
}
#[cfg(not(windows))]
macro_rules! seh {
  ($($line:literal),*) => {
    ""
  };
}
#[cfg(not(windows))]
macro_rules! cfi {
  ($($line:literal),*) => { concat!($($line, "\n"),*) };
}
#[cfg(windows)]
macro_rules! cfi {
  ($($line:literal),*) => {
    ""
  };
}

#[cfg(target_vendor = "apple")]
macro_rules! symbol {
  ($name:literal) => {
    concat!("_", $name)
  };
}
#[cfg(not(target_vendor = "apple"))]
macro_rules! symbol {
  ($name:literal) => {
    $name
  };
}

#[cfg(windows)]
macro_rules! function {
  ($name:literal) => {
    concat!(
      ".globl ",
      symbol!($name),
      "\n",
      ".def ",
      symbol!($name),
      "\n.scl 2\n.type 32\n.endef\n",
      ".p2align 4\n",
      symbol!($name),
      ":\n",
      ".seh_proc ",
      symbol!($name),
      "\n",
    )
  };
}
#[cfg(not(windows))]
macro_rules! function {
  ($name:literal) => {
    concat!(
      ".globl ",
      symbol!($name),
      "\n",
      ".p2align 4\n",
      symbol!($name),
      ":\n",
      ".cfi_startproc\n",
    )
  };
}

// Both thunks keep the same frame: four saved registers, then 24 bytes
// that leave `rbp` sixteen-byte aligned, with a result buffer at `rbp`.
// C's return address is at `rbp + 56`, the first stack argument at
// `rbp + 96`.
macro_rules! prologue {
  () => {
    concat!(
      "push rbp\n",
      seh!(".seh_pushreg rbp"),
      cfi!(".cfi_def_cfa_offset 16", ".cfi_offset rbp, -16"),
      "push rbx\n",
      seh!(".seh_pushreg rbx"),
      cfi!(".cfi_def_cfa_offset 24", ".cfi_offset rbx, -24"),
      "push rsi\n",
      seh!(".seh_pushreg rsi"),
      cfi!(".cfi_def_cfa_offset 32", ".cfi_offset rsi, -32"),
      "push rdi\n",
      seh!(".seh_pushreg rdi"),
      cfi!(".cfi_def_cfa_offset 40", ".cfi_offset rdi, -40"),
      "sub rsp, 24\n",
      seh!(".seh_stackalloc 24"),
      cfi!(".cfi_def_cfa_offset 64"),
      "mov rbp, rsp\n",
      seh!(".seh_setframe rbp, 0", ".seh_endprologue"),
      cfi!(".cfi_def_cfa_register rbp"),
    )
  };
}

macro_rules! epilogue {
  () => {
    concat!(
      "lea rsp, [rbp + 24]\n",
      "pop rdi\n",
      "pop rsi\n",
      "pop rbx\n",
      "pop rbp\n",
      "ret\n",
      seh!(".seh_endproc"),
      cfi!(".cfi_endproc"),
    )
  };
}

std::arch::global_asm!(
  concat!(
    ".text\n",
    // libffi calls this with a `Call` first and the target's arguments
    // after it.
    function!("zuri_ffi_wide_call"),
    prologue!(),
    "mov rbx, rcx\n",
    // A frame for the target: its register arguments' home area, then
    // its stack arguments, copied from one place further up.
    "mov rax, [rbx + 16]\n",
    "lea rax, [rax * 8 + 32 + 15]\n",
    "and rax, -16\n",
    "sub rsp, rax\n",
    "mov rcx, [rbx + 16]\n",
    "lea rsi, [rbp + 104]\n",
    "lea rdi, [rsp + 32]\n",
    "rep movsq\n",
    "mov rcx, rdx\n",
    "movaps xmm0, xmm1\n",
    "mov rdx, r8\n",
    "movaps xmm1, xmm2\n",
    "mov r8, r9\n",
    "movaps xmm2, xmm3\n",
    "mov r9, [rbp + 96]\n",
    "movq xmm3, r9\n",
    "call qword ptr [rbx]\n",
    "mov rcx, [rbx + 8]\n",
    "movdqu [rcx], xmm0\n",
    epilogue!(),
    // C calls one of these as the callback. Each finds its own index
    // from where it is and enters the common thunk with it.
    ".globl ", symbol!("zuri_ffi_wide_stubs"), "\n",
    ".p2align 4\n",
    symbol!("zuri_ffi_wide_stubs"), ":\n",
    ".rept {stubs}\n",
    ".p2align 4\n",
    "lea r10, [rip]\n",
    "jmp ", symbol!("zuri_ffi_wide_entry"), "\n",
    ".endr\n",
    function!("zuri_ffi_wide_entry"),
    prologue!(),
    "lea r11, [rip + ", symbol!("zuri_ffi_wide_stubs"), "]\n",
    "sub r10, r11\n",
    "shr r10, 4\n",
    "lea r11, [rip + {slots}]\n",
    "mov rbx, [r11 + r10 * 8]\n",
    // A frame for the closure: the home area, the argument C passed in
    // `r9`, now a stack argument, and C's own stack arguments after it.
    "mov rax, [rbx + 8]\n",
    "lea rax, [rax * 8 + 40 + 15]\n",
    "and rax, -16\n",
    "sub rsp, rax\n",
    "mov r11, rcx\n",
    "mov [rsp + 32], r9\n",
    "mov rcx, [rbx + 8]\n",
    "lea rsi, [rbp + 96]\n",
    "lea rdi, [rsp + 40]\n",
    "rep movsq\n",
    "mov r9, r8\n",
    "movaps xmm3, xmm2\n",
    "mov r8, rdx\n",
    "movaps xmm2, xmm1\n",
    "mov rdx, r11\n",
    "movaps xmm1, xmm0\n",
    "mov rcx, rbp\n",
    "call qword ptr [rbx]\n",
    "movdqa xmm0, [rbp]\n",
    epilogue!(),
  ),
  slots = sym SLOTS,
  stubs = const STUBS,
);
