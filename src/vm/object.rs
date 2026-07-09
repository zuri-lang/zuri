use big_num::BigInt;

use crate::vm::chunk::Chunk;
use crate::vm::value::Value;

/// Everything a Value's pointer tag can point at.
pub enum Obj {
  Str(String),
  Bytes(Vec<u8>),
  BigInt(BigInt),
  Func(ObjFunction),
}

pub struct ObjFunction {
  pub name: String,
  pub arity: u8,
  /// How many registers this function's window needs. The caller reserves
  /// this many registers starting at the call's `first_arg` register.
  pub num_registers: u8,
  pub chunk: Chunk,
}

/// Owns every heap object for the lifetime of the VM. Values only ever hold
/// *const Obj pointers into this arena, never real ownership, which is what
/// lets a Value stay a plain Copy u64.
#[derive(Default)]
pub struct Heap {
  objects: Vec<Box<Obj>>,
}

impl Heap {
  pub fn new() -> Heap {
    Heap {
      objects: Vec::new(),
    }
  }

  fn alloc(&mut self, obj: Obj) -> Value {
    self.objects.push(Box::new(obj));
    let ptr: *const Obj = self.objects.last().unwrap().as_ref();
    Value::obj(ptr)
  }

  pub fn alloc_string(&mut self, s: impl Into<String>) -> Value {
    self.alloc(Obj::Str(s.into()))
  }

  pub fn alloc_bytes(&mut self, b: impl Into<Vec<u8>>) -> Value {
    self.alloc(Obj::Bytes(b.into()))
  }

  pub fn alloc_bigint(&mut self, s: impl Into<BigInt>) -> Value {
    self.alloc(Obj::BigInt(s.into()))
  }

  pub fn alloc_function(&mut self, f: ObjFunction) -> Value {
    self.alloc(Obj::Func(f))
  }
}
