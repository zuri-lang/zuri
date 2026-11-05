//! Describes compiled functions to the platform unwinder.
//!
//! A Rust panic unwinds with the same tables a C++ exception uses, and
//! it stops dead at a frame it has no entry for. Compiled code is not
//! in any binary's tables, so each function registers its own here. The
//! frames have no landing pads, since nothing needs to run in them: all
//! a panic needs is a way past, to the `catch_unwind` that discards the
//! VM (see `jit::runtime`'s notes on panics).
//!
//! Entries are never removed, since machine code is never unloaded.
//! They are only read while a panic unwinds, so a function that never
//! sees one pays for registration and nothing else.

#[cfg(not(windows))]
pub use system_v::Entry;
#[cfg(windows)]
pub use windows::Entry;

#[cfg(not(windows))]
mod system_v {
  use cranelift_codegen::isa::TargetIsa;
  use cranelift_codegen::isa::unwind::{UnwindInfo, systemv};
  use gimli::write::{Address, CommonInformationEntry, EhFrame, EndianVec, FrameTable};

  unsafe extern "C" {
    fn __register_frame(entry: *const u8);
  }

  /// A function's unwind entry, ready to register once its code has an
  /// address.
  pub struct Entry {
    cie: CommonInformationEntry,
    info: systemv::UnwindInfo,
  }

  impl Entry {
    pub fn new(isa: &dyn TargetIsa, info: UnwindInfo, _code: &mut Vec<u8>) -> Option<Entry> {
      let UnwindInfo::SystemV(info) = info else {
        return None;
      };
      Some(Entry {
        cie: isa.create_systemv_cie()?,
        info,
      })
    }

    /// Registers the entry for code installed at `address`.
    ///
    /// # Safety
    ///
    /// `address` must be where the function was installed, and that
    /// memory must never be released.
    pub unsafe fn register(self, address: *const u8) {
      let mut table = FrameTable::default();
      let cie = table.add_cie(self.cie);
      table.add_fde(cie, self.info.to_fde(Address::Constant(address as u64)));

      let mut frame = EhFrame(EndianVec::new(gimli::RunTimeEndian::default()));
      table
        .write_eh_frame(&mut frame)
        .expect("zuri: failed to write a compiled function's unwind entry");

      // The unwinder keeps reading this for as long as the process
      // lives. A zero length ends the section.
      let mut bytes = frame.0.into_vec();
      bytes.extend_from_slice(&[0; 4]);
      let bytes: &'static [u8] = Vec::leak(bytes);

      // libgcc takes a whole section. libunwind takes one FDE at a time,
      // and finds the CIE in front of it through the FDE's back pointer.
      if cfg!(target_vendor = "apple") {
        let mut offset = 0;
        while offset + 4 < bytes.len() {
          let length = u32::from_ne_bytes(bytes[offset..offset + 4].try_into().unwrap()) as usize;
          if offset != 0 {
            unsafe { __register_frame(bytes[offset..].as_ptr()) };
          }
          offset += length + 4;
        }
      } else {
        unsafe { __register_frame(bytes.as_ptr()) };
      }
    }
  }
}

#[cfg(windows)]
mod windows {
  use cranelift_codegen::isa::TargetIsa;
  use cranelift_codegen::isa::unwind::UnwindInfo;

  /// `RUNTIME_FUNCTION`: offsets from the base given with the table.
  #[repr(C)]
  struct RuntimeFunction {
    begin: u32,
    end: u32,
    unwind: u32,
  }

  #[link(name = "kernel32")]
  unsafe extern "system" {
    fn RtlAddFunctionTable(table: *const RuntimeFunction, count: u32, base: u64) -> u8;
  }

  /// A function's unwind entry, ready to register once its code has an
  /// address.
  pub struct Entry {
    code_len: u32,
    unwind_at: u32,
  }

  impl Entry {
    /// Appends the unwind data to `code`, which must then be installed
    /// as a whole.
    pub fn new(_isa: &dyn TargetIsa, info: UnwindInfo, code: &mut Vec<u8>) -> Option<Entry> {
      let UnwindInfo::WindowsX64(info) = info else {
        return None;
      };

      // The table holds unsigned offsets from the base, so the unwind
      // data has to sit after the code it describes. Going in the same
      // blob guarantees that, wherever the code lands.
      let code_len = code.len();
      let unwind_at = code_len.next_multiple_of(4);
      code.resize(unwind_at + info.emit_size(), 0);
      info.emit(&mut code[unwind_at..]);

      Some(Entry {
        code_len: code_len as u32,
        unwind_at: unwind_at as u32,
      })
    }

    /// Registers the entry for code installed at `address`.
    ///
    /// # Safety
    ///
    /// `address` must be where `code` was installed, and that memory
    /// must never be released.
    pub unsafe fn register(self, address: *const u8) {
      let table = Box::leak(Box::new(RuntimeFunction {
        begin: 0,
        end: self.code_len,
        unwind: self.unwind_at,
      }));
      unsafe { RtlAddFunctionTable(table, 1, address as u64) };
    }
  }
}
