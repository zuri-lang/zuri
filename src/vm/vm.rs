use crate::vm::chunk::Instr;
use crate::vm::object::{Heap, Obj, ObjFunction};
use crate::vm::value::Value;

struct CallFrame {
  function: *const ObjFunction,
  ip: usize,
  /// Index into `VM::registers` where this frame's register window starts.
  base: usize,
  /// Register (in the *caller's* window) that the return value should be
  /// written to. Unused for the outermost frame.
  dst_in_caller: u8,
}

pub struct VM {
  /// One flat register stack shared by every call frame; each frame just
  /// claims a slice of it (its "window"), exactly like Lua's VM.
  registers: Vec<Value>,
  frames: Vec<CallFrame>,
  globals: std::collections::HashMap<String, Value>,
  pub heap: Heap,
}

type RunResult<T> = Result<T, String>;

impl VM {
  pub fn new(heap: Heap) -> VM {
    VM {
      registers: Vec::new(),
      frames: Vec::new(),
      globals: std::collections::HashMap::new(),
      heap,
    }
  }

  /// Bind a value directly, useful for wiring up a top-level function
  /// (e.g. "fib") before `run` starts executing.
  #[allow(dead_code)]
  pub fn define_global(&mut self, name: impl Into<String>, v: Value) {
    self.globals.insert(name.into(), v);
  }

  /// Run `main` (a top-level, non-nested function with no arguments) to
  /// completion.
  pub fn run(&mut self, main: *const ObjFunction) -> RunResult<()> {
    let num_registers = unsafe { (*main).num_registers } as usize;
    self.registers.resize(num_registers, Value::nil());
    self.frames.push(CallFrame {
      function: main,
      ip: 0,
      base: 0,
      dst_in_caller: 0,
    });

    // println!("{:?}", (unsafe { &*main }).chunk);

    loop {
      let frame_idx = self.frames.len() - 1;
      let (func_ptr, ip, base) = {
        let f = &self.frames[frame_idx];
        (f.function, f.ip, f.base)
      };
      let func = unsafe { &*func_ptr };

      // println!("{:?}", func.chunk);

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

        Instr::Add { dst, a, b } => self.binary_numeric(base, dst, a, b, "+", |x, y| x + y)?,
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

        Instr::Call {
          dst,
          func: func_reg,
          num_args,
        } => {
          let callee = self.get_reg(base, func_reg);
          if !callee.is_obj() {
            return Err(format!("cannot call a {}", callee.type_name()));
          }
          let obj = unsafe { &*callee.as_obj() };
          let Obj::Func(callee_fn) = obj else {
            return Err(format!("cannot call a {}", callee.type_name()));
          };
          if callee_fn.arity != num_args {
            return Err(format!(
              "'{}' expects {} argument(s), got {}",
              callee_fn.name, callee_fn.arity, num_args
            ));
          }
          let new_base = base + func_reg as usize + 1;
          let needed = new_base + callee_fn.num_registers as usize;
          if self.registers.len() < needed {
            self.registers.resize(needed, Value::nil());
          }
          self.frames.push(CallFrame {
            function: callee_fn as *const ObjFunction,
            ip: 0,
            base: new_base,
            dst_in_caller: dst,
          });
        },
        Instr::Return { src } => {
          let ret = self.get_reg(base, src);
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
      }
    }
  }

  fn const_as_str(&self, func: &ObjFunction, idx: u16) -> RunResult<String> {
    let v = func.chunk.constants[idx as usize];
    if !v.is_obj() {
      return Err("expected a string constant for a global name".to_string());
    }
    match unsafe { &*v.as_obj() } {
      Obj::Str(s) => Ok(s.clone()),
      _ => Err("expected a string constant for a global name".to_string()),
    }
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
