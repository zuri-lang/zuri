use core::fmt;

use crate::vm::value::Value;

/// A register-based instruction, modeled loosely on Lua's VM: most ops name
/// a destination register plus one or two source registers, rather than
/// pushing/popping an operand stack.
#[derive(Clone, Copy, Debug)]
#[allow(dead_code)]
pub enum Instr {
  LoadConst {
    dst: u8,
    const_idx: u16,
  },
  LoadNil {
    dst: u8,
  },
  LoadBool {
    dst: u8,
    val: bool,
  },
  Move {
    dst: u8,
    src: u8,
  },

  Add {
    dst: u8,
    a: u8,
    b: u8,
  },
  Sub {
    dst: u8,
    a: u8,
    b: u8,
  },
  Mul {
    dst: u8,
    a: u8,
    b: u8,
  },
  Div {
    dst: u8,
    a: u8,
    b: u8,
  },
  Pow {
    dst: u8,
    a: u8,
    b: u8,
  },
  Floor {
    dst: u8,
    a: u8,
    b: u8,
  },
  Mod {
    dst: u8,
    a: u8,
    b: u8,
  },
  Neg {
    dst: u8,
    src: u8,
  },
  Not {
    dst: u8,
    src: u8,
  },
  Concat {
    dst: u8,
    a: u8,
    b: u8,
  },

  BitAnd {
    dst: u8,
    a: u8,
    b: u8,
  },
  BitOr {
    dst: u8,
    a: u8,
    b: u8,
  },
  BitXor {
    dst: u8,
    a: u8,
    b: u8,
  },
  BitShl {
    dst: u8,
    a: u8,
    b: u8,
  },
  BitShr {
    dst: u8,
    a: u8,
    b: u8,
  },
  BitUshr {
    dst: u8,
    a: u8,
    b: u8,
  },
  BitNot {
    dst: u8,
    src: u8,
  },

  Eq {
    dst: u8,
    a: u8,
    b: u8,
  },
  Neq {
    dst: u8,
    a: u8,
    b: u8,
  },
  Lt {
    dst: u8,
    a: u8,
    b: u8,
  },
  Le {
    dst: u8,
    a: u8,
    b: u8,
  },
  Gt {
    dst: u8,
    a: u8,
    b: u8,
  },
  Ge {
    dst: u8,
    a: u8,
    b: u8,
  },

  /// Unconditional relative jump.
  Jmp {
    offset: i16,
  },
  /// Jump by `offset` if register `cond` is falsey.
  JmpIfFalse {
    cond: u8,
    offset: i16,
  },
  /// Jump by `offset` if register `cond` is truthy -- the complement of
  /// JmpIfFalse, used for short-circuiting `or`.
  JmpIfTrue {
    cond: u8,
    offset: i16,
  },

  /// Call the function in register `func`. Arguments are expected to
  /// already sit in registers `func+1 ..= func+num_args`, matching Zuri's
  /// convention of laying out args right after the callee. The return
  /// value is written into `dst` in the *caller's* window.
  Call {
    dst: u8,
    func: u8,
    num_args: u8,
  },
  Return {
    src: u8,
  },

  Print {
    src: u8,
  },

  /// Globals are looked up by name (a string constant) rather than by a
  /// fixed slot, so that recursive functions can find themselves without
  /// needing a pointer to their own not-yet-allocated ObjFunction baked
  /// into their own constant pool.
  GetGlobal {
    dst: u8,
    name_const: u16,
  },
  SetGlobal {
    name_const: u16,
    src: u8,
  },

  /// Create a new closure from the function prototype stored at
  /// `constants[proto_const]`, capturing upvalues per that prototype's
  /// own (compile-time, static) descriptor list, sourced from the
  /// CURRENTLY EXECUTING frame's registers/closure.
  Closure {
    dst: u8,
    proto_const: u16,
  },
  GetUpval {
    dst: u8,
    idx: u8,
  },
  SetUpval {
    idx: u8,
    src: u8,
  },
  /// Close every open upvalue pointing at a register >= `from` (relative
  /// to the current frame), copying its current value out of the
  /// register into the upvalue's own storage. Emitted at block exit so a
  /// register reused for something else (e.g. the next loop iteration's
  /// local) doesn't silently corrupt a closure that captured it.
  CloseUpvalues {
    from: u8,
  },
}

#[derive(Default, Clone, Debug)]
pub struct Chunk {
  pub code: Vec<Instr>,
  pub constants: Vec<Value>,
}

impl Chunk {
  pub fn new() -> Chunk {
    Chunk {
      code: Vec::new(),
      constants: Vec::new(),
    }
  }

  pub fn add_constant(&mut self, v: Value) -> u16 {
    self.constants.push(v);
    (self.constants.len() - 1) as u16
  }

  pub fn emit(&mut self, instr: Instr) -> usize {
    self.code.push(instr);
    self.code.len() - 1
  }

  pub fn patch(&mut self, index: usize, instr: Instr) {
    self.code[index] = instr;
  }

  pub fn patch_false_jump(&mut self, cond: u8, offset: usize) {
    self.code[offset] = Instr::JmpIfFalse {
      cond,
      offset: (self.code.len() - offset - 1) as i16,
    };
  }

  pub fn patch_jump(&mut self, offset: usize) {
    self.code[offset] = Instr::Jmp {
      offset: (self.code.len() - offset - 1) as i16,
    };
  }
}

impl fmt::Display for Chunk {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    write!(f, "{:?}", self)
  }
}
