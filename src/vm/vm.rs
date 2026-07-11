use std::collections::HashMap;

use crate::vm::chunk::Instr;
use crate::vm::object::{Heap, ObjClosure, ObjFunction, UpvalueDescriptor, UpvalueState};
use crate::vm::value::Value;

struct CallFrame {
  function: *const ObjFunction,
  /// The specific closure instance this frame is executing -- needed
  /// whenever GetUpval/SetUpval/Closure look at "my own captured
  /// upvalues". Distinct from `function` (the shared, static prototype)
  /// the same way `ObjClosure` is distinct from `ObjFunction`.
  closure: *const ObjClosure,
  ip: usize,
  /// Index into `VM::registers` where this frame's register window starts.
  base: usize,
  /// Register (in the *caller's* window) that the return value should be
  /// written to. Unused for the outermost frame.
  dst_in_caller: u8,
}

pub struct VM<'a> {
  /// One flat register stack shared by every call frame; each frame just
  /// claims a slice of it (its "window"), exactly like Lua's VM.
  registers: Vec<Value>,
  /// Upvalues that are still Open, as (absolute register index, the
  /// Obj::Upvalue Value at that index). Consulted whenever a new closure
  /// captures a local -- if one's already open for that exact register,
  /// it's reused rather than duplicated, which is what makes two
  /// closures over the same variable see each other's writes.
  open_upvalues: Vec<(usize, Value)>,
  frames: Vec<CallFrame>,
  globals: std::collections::HashMap<String, Value>,
  pub heap: &'a mut Heap,
}

type RunResult<T> = Result<T, String>;

impl<'a> VM<'a> {
  pub fn new(heap: &'a mut Heap, globals: HashMap<String, Value>) -> Self {
    VM {
      registers: Vec::new(),
      frames: Vec::new(),
      open_upvalues: Vec::new(),
      globals,
      heap,
    }
  }

  pub fn heap_mut(&mut self) -> &mut Heap {
    &mut self.heap
  }

  /// Bind a value directly, useful for wiring up a top-level function
  /// (e.g. "fib") before `run` starts executing.
  pub fn define_global(&mut self, name: impl Into<String>, v: Value) {
    self.globals.insert(name.into(), v);
  }

  /// Run `main` (a top-level, non-nested function with no arguments) to
  /// completion.
  pub fn run(&mut self, main: *const ObjClosure) -> RunResult<()> {
    let proto = unsafe { &*(*main).function };
    let num_registers = proto.num_registers as usize;
    self.registers.resize(num_registers, Value::nil());
    self.frames.push(CallFrame {
      function: proto as *const ObjFunction,
      closure: main,
      ip: 0,
      base: 0,
      dst_in_caller: 0,
    });

    // println!("{:?}", (unsafe { &*main }).chunk);

    loop {
      let frame_idx = self.frames.len() - 1;
      let (func_ptr, closure_ptr, ip, base) = {
        let f = &self.frames[frame_idx];
        (f.function, f.closure, f.ip, f.base)
      };
      let func = unsafe { &*func_ptr };

      if ip >= func.chunk.code.len() {
        return Err(format!(
          "fell off the end of '{}' without a Return",
          func.name
        ));
      }
      let instr = func.chunk.code[ip];
      self.frames[frame_idx].ip += 1;

      match instr {
        Instr::LoadConst { dst, const_idx } => {
          let v = func.chunk.constants[const_idx as usize];
          self.set_reg(base, dst, v);
        },
        Instr::LoadNil { dst } => self.set_reg(base, dst, Value::nil()),
        Instr::LoadBool { dst, val } => self.set_reg(base, dst, Value::bool(val)),
        Instr::Move { dst, src } => {
          let v = self.get_reg(base, src);
          self.set_reg(base, dst, v);
        },

        Instr::Add { dst, a, b } => self.binary_add(base, dst, a, b, "+")?,
        Instr::Sub { dst, a, b } => self.binary_numeric(base, dst, a, b, "-", |x, y| x - y)?,
        Instr::Mul { dst, a, b } => self.binary_numeric(base, dst, a, b, "*", |x, y| x * y)?,
        Instr::Div { dst, a, b } => self.binary_numeric(base, dst, a, b, "/", |x, y| x / y)?,
        Instr::Pow { dst, a, b } => self.binary_numeric(base, dst, a, b, "**", |x, y| x.powf(y))?,
        Instr::Mod { dst, a, b } => self.binary_numeric(base, dst, a, b, "%", |x, y| x % y)?,
        Instr::Floor { dst, a, b } => {
          self.binary_numeric(base, dst, a, b, "//", |x, y| (x / y).floor())?
        },
        Instr::BitAnd { dst, a, b } => self.bitwise_numeric(base, dst, a, b, "&", |x, y| x & y)?,
        Instr::BitOr { dst, a, b } => self.bitwise_numeric(base, dst, a, b, "|", |x, y| x | y)?,
        Instr::BitXor { dst, a, b } => self.bitwise_numeric(base, dst, a, b, "^", |x, y| x ^ y)?,
        Instr::BitShl { dst, a, b } => {
          self.bitwise_numeric(base, dst, a, b, "<<", |x, y| x << y)?
        },
        Instr::BitShr { dst, a, b } => {
          self.bitwise_numeric(base, dst, a, b, ">>", |x, y| x >> y)?
        },
        Instr::BitUshr { dst, a, b } => {
          self.bitwise_numeric(base, dst, a, b, ">>>", |x, y| x >> y)?
        },
        Instr::BitNot { dst, src } => {
          let v = self.get_reg(base, src);
          if !v.is_number() {
            return Err(format!("cannot bitwise not a {}", v.type_name()));
          }
          self.set_reg(base, dst, Value::number((!(v.as_number() as i64)) as f64));
        },
        Instr::Neg { dst, src } => {
          let v = self.get_reg(base, src);
          if !v.is_number() {
            return Err(format!("cannot negate a {}", v.type_name()));
          }
          self.set_reg(base, dst, Value::number(-v.as_number()));
        },
        Instr::Not { dst, src } => {
          let v = self.get_reg(base, src);
          self.set_reg(base, dst, Value::bool(v.is_falsey()));
        },
        Instr::Concat { dst, a, b } => {
          let va = self.get_reg(base, a);
          let vb = self.get_reg(base, b);
          let s = format!("{}{}", va, vb);
          let v = self.heap.alloc_string(s);
          self.set_reg(base, dst, v);
        },

        Instr::Eq { dst, a, b } => {
          let va = self.get_reg(base, a);
          let vb = self.get_reg(base, b);
          self.set_reg(base, dst, Value::bool(va.equals(&vb)));
        },
        Instr::Neq { dst, a, b } => {
          let va = self.get_reg(base, a);
          let vb = self.get_reg(base, b);
          self.set_reg(base, dst, Value::bool(!va.equals(&vb)));
        },
        Instr::Lt { dst, a, b } => self.compare(base, dst, a, b, "<", |x, y| x < y)?,
        Instr::Gt { dst, a, b } => self.compare(base, dst, a, b, ">", |x, y| x > y)?,
        Instr::Le { dst, a, b } => self.compare(base, dst, a, b, "<=", |x, y| x <= y)?,
        Instr::Ge { dst, a, b } => self.compare(base, dst, a, b, ">=", |x, y| x >= y)?,

        Instr::Jmp { offset } => {
          self.jump(frame_idx, offset);
        },
        Instr::JmpIfFalse { cond, offset } => {
          if self.get_reg(base, cond).is_falsey() {
            self.jump(frame_idx, offset);
          }
        },
        Instr::JmpIfTrue { cond, offset } => {
          if !self.get_reg(base, cond).is_falsey() {
            self.jump(frame_idx, offset);
          }
        },

        Instr::Call {
          dst,
          func: func_reg,
          num_args,
        } => {
          let callee = self.get_reg(base, func_reg);
          if !callee.is_closure() {
            return Err(format!("cannot call a {}", callee.type_name()));
          }
          let callee_closure = callee.as_closure();
          let callee_fn = unsafe { &*callee_closure.function };

          let required = if callee_fn.variadic {
            callee_fn.arity - 1
          } else {
            callee_fn.arity
          };

          let new_base = base + func_reg as usize + 1;
          let needed = new_base + callee_fn.num_registers as usize;
          if self.registers.len() < needed {
            self.registers.resize(needed, Value::nil());
          }

          for i in num_args..required {
            self.registers[new_base + i as usize] = Value::nil();
          }

          if callee_fn.variadic {
            let extra_count = num_args.saturating_sub(required);
            let mut items = Vec::with_capacity(extra_count as usize);
            for i in 0..extra_count {
              items.push(self.registers[new_base + required as usize + i as usize]);
            }
            let list_val = self.heap.alloc_list(items);
            self.registers[new_base + required as usize] = list_val;
          }

          self.frames.push(CallFrame {
            function: callee_fn as *const ObjFunction,
            closure: callee_closure as *const ObjClosure,
            ip: 0,
            base: new_base,
            dst_in_caller: dst,
          });
        },
        Instr::Return { src } => {
          let ret = self.get_reg(base, src);
          self.close_upvalues_from(base);
          let finished = self.frames.pop().unwrap();
          if self.frames.is_empty() {
            return Ok(());
          }
          let caller = self.frames.last().unwrap();
          self.set_reg(caller.base, finished.dst_in_caller, ret);
        },

        Instr::Print { src } => {
          let v = self.get_reg(base, src);
          println!("{}", v);
        },

        Instr::GetGlobal { dst, name_const } => {
          let name = self.const_as_str(func, name_const)?;
          let v = self
            .globals
            .get(&name)
            .copied()
            .ok_or_else(|| format!("undefined global '{}'", name))?;
          self.set_reg(base, dst, v);
        },
        Instr::SetGlobal { name_const, src } => {
          let name = self.const_as_str(func, name_const)?;
          let v = self.get_reg(base, src);
          self.globals.insert(name, v);
        },

        Instr::Closure { dst, proto_const } => {
          let proto_val = func.chunk.constants[proto_const as usize];
          if !proto_val.is_func() {
            return Err("Closure operand is not a function".to_string());
          }
          let proto = proto_val.as_func();
          let proto_ptr = proto as *const ObjFunction;

          let mut captured = Vec::with_capacity(proto.upvalues.len());
          for desc in &proto.upvalues {
            let upval = match *desc {
              UpvalueDescriptor::Local(reg) => {
                let abs_index = base + reg as usize;
                self.capture_upvalue(abs_index)
              },
              UpvalueDescriptor::Upvalue(idx) => {
                let current_closure = unsafe { &*closure_ptr };
                current_closure.upvalues[idx as usize]
              },
            };
            captured.push(upval);
          }
          let closure_val = self.heap.alloc_closure(ObjClosure {
            function: proto_ptr,
            upvalues: captured,
          });
          self.set_reg(base, dst, closure_val);
        },
        Instr::GetUpval { dst, idx } => {
          let current_closure = unsafe { &*closure_ptr };
          let upval_val = current_closure.upvalues[idx as usize];
          if !upval_val.is_upvalue() {
            return Err("GetUpval operand is not an upvalue".to_string());
          }
          let v = match upval_val.as_upvalue().get() {
            UpvalueState::Open(abs_idx) => self.registers[abs_idx],
            UpvalueState::Closed(v) => v,
          };
          self.set_reg(base, dst, v);
        },
        Instr::SetUpval { idx, src } => {
          let v = self.get_reg(base, src);
          let current_closure = unsafe { &*closure_ptr };
          let upval_val = current_closure.upvalues[idx as usize];
          if !upval_val.is_upvalue() {
            return Err("SetUpval operand is not an upvalue".to_string());
          }
          let cell = upval_val.as_upvalue();
          match cell.get() {
            UpvalueState::Open(abs_idx) => self.registers[abs_idx] = v,
            UpvalueState::Closed(_) => cell.set(UpvalueState::Closed(v)),
          }
        },
        Instr::CloseUpvalues { from } => {
          self.close_upvalues_from(base + from as usize);
        },
      }
    }
  }

  /// Find-or-create an OPEN upvalue for the given absolute register
  /// index. Reusing an existing one (rather than always allocating a new
  /// one) is what makes two closures created from the same enclosing
  /// scope, over the same local, actually share state.
  fn capture_upvalue(&mut self, abs_index: usize) -> Value {
    if let Some((_, v)) = self.open_upvalues.iter().find(|(idx, _)| *idx == abs_index) {
      return *v;
    }
    let v = self.heap.alloc_upvalue(UpvalueState::Open(abs_index));
    self.open_upvalues.push((abs_index, v));
    v
  }

  /// Close every open upvalue pointing at a register >= `from_abs_index`,
  /// copying the register's current value into the upvalue's own
  /// storage. Called on block exit and on Return.
  fn close_upvalues_from(&mut self, from_abs_index: usize) {
    let mut i = 0;
    while i < self.open_upvalues.len() {
      let (idx, v) = self.open_upvalues[i];
      if idx >= from_abs_index {
        let current_val = self.registers[idx];
        v.as_upvalue().set(UpvalueState::Closed(current_val));
        self.open_upvalues.swap_remove(i);
      } else {
        i += 1;
      }
    }
  }

  fn const_as_str(&self, func: &ObjFunction, idx: u16) -> RunResult<String> {
    let v = func.chunk.constants[idx as usize];
    if !v.is_string() {
      return Err("expected a string constant for a global name".to_string());
    }
    Ok(v.as_str().to_string())
  }

  #[inline]
  fn get_reg(&self, base: usize, r: u8) -> Value {
    self.registers[base + r as usize]
  }

  #[inline]
  fn set_reg(&mut self, base: usize, r: u8, v: Value) {
    self.registers[base + r as usize] = v;
  }

  fn jump(&mut self, frame_idx: usize, offset: i16) {
    let f = &mut self.frames[frame_idx];
    // ip already advanced past the jump instruction itself, matching a
    // typical "offset is relative to the instruction after this one" scheme.
    f.ip = (f.ip as isize + offset as isize) as usize;
  }

  fn bitwise_numeric(
    &mut self,
    base: usize,
    dst: u8,
    a: u8,
    b: u8,
    op_name: &str,
    op: fn(i64, i64) -> i64,
  ) -> RunResult<()> {
    let va = self.get_reg(base, a);
    let vb = self.get_reg(base, b);
    if !va.is_number() || !vb.is_number() {
      return Err(format!(
        "operator '{}' expects numbers, got {} and {}",
        op_name,
        va.type_name(),
        vb.type_name()
      ));
    }
    self.set_reg(
      base,
      dst,
      Value::number(op(va.as_number() as i64, vb.as_number() as i64) as f64),
    );
    Ok(())
  }

  fn binary_numeric(
    &mut self,
    base: usize,
    dst: u8,
    a: u8,
    b: u8,
    op_name: &str,
    op: fn(f64, f64) -> f64,
  ) -> RunResult<()> {
    let va = self.get_reg(base, a);
    let vb = self.get_reg(base, b);
    if !va.is_number() || !vb.is_number() {
      return Err(format!(
        "operator '{}' expects numbers, got {} and {}",
        op_name,
        va.type_name(),
        vb.type_name()
      ));
    }
    self.set_reg(base, dst, Value::number(op(va.as_number(), vb.as_number())));
    Ok(())
  }

  fn binary_add(&mut self, base: usize, dst: u8, a: u8, b: u8, op_name: &str) -> RunResult<()> {
    let va = self.get_reg(base, a);
    let vb = self.get_reg(base, b);
    if va.is_number() && vb.is_number() {
      return Ok(self.set_reg(base, dst, Value::number(va.as_number() + vb.as_number())));
    } else if va.is_string() || vb.is_string() {
      let va = self.get_reg(base, a);
      let vb = self.get_reg(base, b);
      let s = format!("{}{}", va, vb);
      let v = self.heap.alloc_string(s);
      return Ok(self.set_reg(base, dst, v));
    }

    Err(format!(
      "operator '{}' expects numbers, got {} and {}",
      op_name,
      va.type_name(),
      vb.type_name()
    ))
  }

  fn compare(
    &mut self,
    base: usize,
    dst: u8,
    a: u8,
    b: u8,
    op_name: &str,
    op: fn(f64, f64) -> bool,
  ) -> RunResult<()> {
    let va = self.get_reg(base, a);
    let vb = self.get_reg(base, b);
    if !va.is_number() || !vb.is_number() {
      return Err(format!(
        "operator '{}' expects numbers, got {} and {}",
        op_name,
        va.type_name(),
        vb.type_name()
      ));
    }
    self.set_reg(base, dst, Value::bool(op(va.as_number(), vb.as_number())));
    Ok(())
  }
}
