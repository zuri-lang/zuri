use std::collections::{HashMap, HashSet};
use std::ops::{Neg, Shl, Shr};
use std::sync::LazyLock;

use num_bigint::BigInt;
use num_traits::ToPrimitive;

use crate::vm::chunk::Instr;
use crate::vm::natives;
use crate::vm::object::{
  Heap, Obj, ObjClosure, ObjFunction, UpvalueDescriptor, UpvalueState, ZuriContext,
};
use crate::vm::value::Value;

struct CallFrame {
  function: *const ObjFunction,
  /// The specific closure instance this frame is executing -- needed
  /// whenever GetUpval/SetUpval/Closure look at "my own captured
  /// upvalues". Distinct from `function` (the shared, static prototype)
  /// the same way `ObjClosure` is distinct from `ObjFunction`.
  closure: *const ObjClosure,
  /// The same closure as `closure`, but as the tagged `Value` it was
  /// called through rather than a raw pointer. `function`/`closure`
  /// stay raw pointers purely so the hot instruction-dispatch loop
  /// doesn't pay for a tag check on every fetch; this field is what the
  /// GC's root scan actually walks to keep those raw pointers valid --
  /// see `VM::collect_garbage`.
  closure_val: Value,
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
  /// Upvalues that are still Open, as (absolute register index, the
  /// Obj::Upvalue Value at that index). Consulted whenever a new closure
  /// captures a local -- if one's already open for that exact register,
  /// it's reused rather than duplicated, which is what makes two
  /// closures over the same variable see each other's writes.
  open_upvalues: Vec<(usize, Value)>,
  frames: Vec<CallFrame>,
  globals: std::collections::HashMap<String, Value>,
  pub heap: Heap,
}

type RunResult<T> = Result<T, String>;

const LOG_GC: LazyLock<bool> =
  std::sync::LazyLock::new(|| std::env::var_os("ZURI_GC_LOG").is_some());

impl VM {
  pub fn new(heap: Heap, globals: HashMap<String, Value>) -> Self {
    VM {
      registers: Vec::new(),
      frames: Vec::new(),
      open_upvalues: Vec::new(),
      globals,
      heap,
    }
  }

  pub fn init(&mut self) {
    natives::install(self);
  }

  pub fn heap_mut(&mut self) -> &mut Heap {
    &mut self.heap
  }

  /// Bind a value directly, useful for wiring up a top-level function
  /// (e.g. "fib") before `run` starts executing.
  pub fn define_global(&mut self, name: impl Into<String>, v: Value) {
    self.globals.insert(name.into(), v);
  }

  /// Run `main` (a top-level closure, typically zero-upvalue, taking no
  /// arguments) to completion.
  pub fn run(&mut self, main: Value) -> RunResult<()> {
    let closure = main.as_closure();
    let proto = closure.function.as_func();
    let num_registers = proto.num_registers as usize;
    self.registers.resize(num_registers, Value::nil());
    self.frames.push(CallFrame {
      function: proto as *const ObjFunction,
      closure: closure as *const ObjClosure,
      closure_val: main,
      ip: 0,
      base: 0,
      dst_in_caller: 0,
    });
    self.run_until(0)?;
    Ok(())
  }

  /// Invoke any callable Value -- a closure OR a native -- with the
  /// given (already-evaluated, owned) arguments, run it to completion,
  /// and return its result. This is what lets a native function call
  /// BACK into Zuri code: a future `map(list, fn)` plugin would call
  /// this once per element with `fn` as the callee.
  pub fn call_value(&mut self, callee: Value, args: &[Value]) -> RunResult<Value> {
    if callee.is_native() {
      let native = callee.as_native();
      return self.call_native(native, args);
    }

    if !callee.is_closure() {
      return Err(format!("cannot call a {}", callee.type_name()));
    }

    let closure = callee.as_closure();
    let proto = closure.function.as_func();
    let required = if proto.variadic {
      proto.arity - 1
    } else {
      proto.arity
    };

    // Always place the injected frame at the current top of the
    // shared register stack -- guaranteed not to overlap any
    // currently-live frame, however deep we already are.
    let new_base = self.registers.len();
    let needed = new_base + proto.num_registers as usize;
    self.registers.resize(needed, Value::nil());

    for i in 0..required as usize {
      self.registers[new_base + i] = args.get(i).copied().unwrap_or(Value::nil());
    }
    if proto.variadic {
      let extra: Vec<Value> = args.iter().skip(required as usize).copied().collect();
      let list_val = self.heap.alloc_list(extra);
      self.registers[new_base + required as usize] = list_val;
    }

    let stop_depth = self.frames.len();
    self.frames.push(CallFrame {
      function: proto as *const ObjFunction,
      closure: closure as *const ObjClosure,
      closure_val: callee,
      ip: 0,
      base: new_base,
      dst_in_caller: 0, // unused -- run_until returns the value directly instead
    });
    self.run_until(stop_depth)
  }

  fn call_native(
    &mut self,
    native: &crate::vm::object::NativeFunction,
    args: &[Value],
  ) -> RunResult<Value> {
    let ok_arity = if native.variadic {
      args.len() as u8 >= native.min_arity
    } else {
      args.len() as u8 == native.min_arity
    };
    if !ok_arity {
      return Err(format!(
        "'{}' expects {}{} argument(s), got {}",
        native.name,
        if native.variadic { "at least " } else { "" },
        native.min_arity,
        args.len()
      ));
    }
    let mut ctx = ZuriContext { vm: self, args };
    (native.func)(&mut ctx)
  }

  fn run_until(&mut self, stop_depth: usize) -> RunResult<Value> {
    loop {
      if self.heap.needs_gc() {
        self.collect_garbage();
      }

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
        Instr::Sub { dst, a, b } => {
          self.binary_numeric(base, dst, a, b, "-", |x, y| x - y, |x, y| &x - &y)?
        },
        Instr::Mul { dst, a, b } => {
          self.binary_numeric(base, dst, a, b, "*", |x, y| x * y, |x, y| &x * &y)?
        },
        Instr::Div { dst, a, b } => {
          self.binary_numeric(base, dst, a, b, "/", |x, y| x / y, |x, y| &x / &y)?
        },
        Instr::Pow { dst, a, b } => {
          self.binary_numeric(base, dst, a, b, "**", |x, y| x.powf(y), |x, y| &x * &y)?
        },
        Instr::Mod { dst, a, b } => {
          self.binary_numeric(base, dst, a, b, "%", |x, y| x % y, |x, y| &x % &y)?
        },
        Instr::Floor { dst, a, b } => self.binary_numeric(
          base,
          dst,
          a,
          b,
          "//",
          |x, y| (x / y).floor(),
          |x, y| &x / &y,
        )?,
        Instr::BitAnd { dst, a, b } => {
          self.bitwise_numeric(base, dst, a, b, "&", |x, y| x & y, |x, y| &x & &y)?
        },
        Instr::BitOr { dst, a, b } => {
          self.bitwise_numeric(base, dst, a, b, "|", |x, y| x | y, |x, y| &x | &y)?
        },
        Instr::BitXor { dst, a, b } => {
          self.bitwise_numeric(base, dst, a, b, "^", |x, y| x ^ y, |x, y| &x ^ &y)?
        },
        Instr::BitShl { dst, a, b } => self.bitwise_numeric(
          base,
          dst,
          a,
          b,
          "<<",
          |x, y| x.checked_shl(y as u32).unwrap_or(0),
          |x, y| x.shl(y.to_i64().unwrap_or(0)),
        )?,
        Instr::BitShr { dst, a, b } => self.bitwise_numeric(
          base,
          dst,
          a,
          b,
          ">>",
          |x, y| x.checked_shr(y as u32).unwrap_or(0),
          |x, y| x.shr(y.to_i64().unwrap_or(0)),
        )?,
        Instr::BitUshr { dst, a, b } => self.bitwise_numeric(
          base,
          dst,
          a,
          b,
          ">>>",
          |x, y| (x as u32).checked_shr(y as u32).unwrap_or(0) as i64,
          |x, y| x.shr(y.to_i64().unwrap_or(0)),
        )?,
        Instr::BitNot { dst, src } => {
          let v = self.get_reg(base, src);
          if !v.is_number() {
            return Err(format!("cannot bitwise not a {}", v.type_name()));
          }
          self.set_reg(base, dst, Value::number((!(v.as_number() as i64)) as f64));
        },
        Instr::Neg { dst, src } => {
          let v = self.get_reg(base, src);
          if v.is_number() {
            self.set_reg(base, dst, Value::number(-v.as_number()));
          } else if v.is_bigint() {
            let v = self.heap.alloc_bigint(v.as_bigint().neg());
            self.set_reg(base, dst, v);
          } else {
            return Err(format!("cannot negate a {}", v.type_name()));
          }
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

          if callee.is_native() {
            // Copy args out into an owned buffer BEFORE
            // calling -- once the native gets &mut VM (so it
            // can call back into Zuri closures), a slice
            // borrowed straight from self.registers would
            // alias that &mut VM. This is the real cost of
            // supporting reentrant native plugins: no longer
            // zero-copy the way a plain &mut Heap native was.
            let args_start = base + func_reg as usize + 1;
            let args_end = args_start + num_args as usize;
            let args: Vec<Value> = self.registers[args_start..args_end].to_vec();
            let native = callee.as_native();
            let result = self.call_native(native, &args)?;
            self.set_reg(base, dst, result);
            continue;
          }

          if !callee.is_closure() {
            return Err(format!("cannot call a {}", callee.type_name()));
          }
          let callee_closure = callee.as_closure();
          let callee_fn = callee_closure.function.as_func();

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
            closure_val: callee,
            ip: 0,
            base: new_base,
            dst_in_caller: dst,
          });
        },
        Instr::Return { src } => {
          let ret = self.get_reg(base, src);
          self.close_upvalues_from(base);
          let finished = self.frames.pop().unwrap();
          if self.frames.len() == stop_depth {
            return Ok(ret);
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
            function: proto_val,
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
        Instr::MakeList { dst, start, count } => {
          let items: Vec<Value> = (0..count).map(|i| self.get_reg(base, start + i)).collect();
          let list_val = self.heap.alloc_list(items);
          self.set_reg(base, dst, list_val);
        },
        Instr::MakeDict { dst, start, count } => {
          let pairs: Vec<(Value, Value)> = (0..count)
            .map(|i| {
              (
                self.get_reg(base, start + i),
                self.get_reg(base, start + count + i),
              )
            })
            .collect();
          let dict_val = self.heap.alloc_dict(pairs);
          self.set_reg(base, dst, dict_val);
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
    big_op: fn(BigInt, BigInt) -> BigInt,
  ) -> RunResult<()> {
    let va = self.get_reg(base, a);
    let vb = self.get_reg(base, b);
    if va.is_number() && vb.is_number() {
      return Ok(self.set_reg(
        base,
        dst,
        Value::number(op(va.as_number() as i64, vb.as_number() as i64) as f64),
      ));
    } else if va.is_bigint() && vb.is_bigint() {
      let v = self
        .heap
        .alloc_bigint(big_op(va.as_bigint().clone(), vb.as_bigint().clone()));
      return Ok(self.set_reg(base, dst, v));
    }

    Err(format!(
      "operator '{}' expects numbers, got {} and {}",
      op_name,
      va.type_name(),
      vb.type_name()
    ))
  }

  fn binary_numeric(
    &mut self,
    base: usize,
    dst: u8,
    a: u8,
    b: u8,
    op_name: &str,
    op: fn(f64, f64) -> f64,
    big_op: fn(BigInt, BigInt) -> BigInt,
  ) -> RunResult<()> {
    let va = self.get_reg(base, a);
    let vb = self.get_reg(base, b);

    if va.is_number() && vb.is_number() {
      return Ok(self.set_reg(base, dst, Value::number(op(va.as_number(), vb.as_number()))));
    } else if va.is_bigint() && vb.is_bigint() {
      let v = self
        .heap
        .alloc_bigint(big_op(va.as_bigint().clone(), vb.as_bigint().clone()));
      return Ok(self.set_reg(base, dst, v));
    }

    Err(format!(
      "operator '{}' not defined for {} and {}",
      op_name,
      va.type_name(),
      vb.type_name()
    ))
  }

  fn binary_add(&mut self, base: usize, dst: u8, a: u8, b: u8, op_name: &str) -> RunResult<()> {
    let va = self.get_reg(base, a);
    let vb = self.get_reg(base, b);
    if va.is_number() && vb.is_number() {
      return Ok(self.set_reg(base, dst, Value::number(va.as_number() + vb.as_number())));
    } else if va.is_bigint() && vb.is_bigint() {
      let v = self.heap.alloc_bigint(va.as_bigint() + vb.as_bigint());
      return Ok(self.set_reg(base, dst, v));
    } else if va.is_string() || vb.is_string() {
      let va = self.get_reg(base, a);
      let vb = self.get_reg(base, b);
      let s = format!("{}{}", va, vb);
      let v = self.heap.alloc_string(s);
      return Ok(self.set_reg(base, dst, v));
    }

    Err(format!(
      "operator '{}' not defined for {} and {}",
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
        "operator '{}' not defined for {} and {}",
        op_name,
        va.type_name(),
        vb.type_name()
      ));
    }
    self.set_reg(base, dst, Value::bool(op(va.as_number(), vb.as_number())));
    Ok(())
  }

  //-----------------------------------------------------------------------------------
  // Garbage collection
  //-----------------------------------------------------------------------------------

  /// Mark-and-sweep collection. Roots are: every register within reach
  /// of a currently active frame, every global, the closure each active
  /// call frame is executing, and any upvalue still open. From there,
  /// every `Value` those objects transitively hold is walked with an
  /// explicit work-list (not recursion, so a long chain can't blow the
  /// stack) before anything unreached gets swept.
  ///
  /// Called automatically from `run_until` once the heap has grown past
  /// its threshold; also exposed to native code (see the `gc` native)
  /// for forcing a collection on demand.
  pub(crate) fn collect_garbage(&mut self) {
    let before_bytes = self.heap.bytes_allocated();
    let before_count = self.heap.object_count();

    let mut reachable: HashSet<*const Obj> = HashSet::new();
    let mut worklist: Vec<*const Obj> = Vec::new();

    // Only the register range actually within reach of a currently
    // active frame can hold live data -- registers past the innermost
    // active frame's window are left over from calls that have already
    // returned (the register stack is never shrunk, purely as a perf
    // tradeoff), so scanning them would just pin down garbage forever.
    let regs_top = self
      .frames
      .last()
      .map(|f| f.base + unsafe { &*f.function }.num_registers as usize)
      .unwrap_or(0)
      .min(self.registers.len());

    for v in &self.registers[..regs_top] {
      Self::mark_root(*v, &mut reachable, &mut worklist);
    }
    for v in self.globals.values() {
      Self::mark_root(*v, &mut reachable, &mut worklist);
    }
    for frame in &self.frames {
      Self::mark_root(frame.closure_val, &mut reachable, &mut worklist);
    }
    for (_, v) in &self.open_upvalues {
      Self::mark_root(*v, &mut reachable, &mut worklist);
    }

    while let Some(ptr) = worklist.pop() {
      // SAFETY: every pointer on the worklist was pulled out of a Value
      // that was itself still live when we queued it, and nothing is
      // freed until `sweep` runs below -- well after this loop -- so the
      // object behind `ptr` is guaranteed to still be valid here.
      match unsafe { &*ptr } {
        Obj::List(items) => {
          for item in items {
            Self::mark_root(*item, &mut reachable, &mut worklist);
          }
        },
        Obj::Dict(pairs) => {
          for (k, v) in pairs {
            Self::mark_root(*k, &mut reachable, &mut worklist);
            Self::mark_root(*v, &mut reachable, &mut worklist);
          }
        },
        Obj::Func(f) => {
          for c in &f.chunk.constants {
            Self::mark_root(*c, &mut reachable, &mut worklist);
          }
        },
        Obj::Closure(c) => {
          Self::mark_root(c.function, &mut reachable, &mut worklist);
          for u in &c.upvalues {
            Self::mark_root(*u, &mut reachable, &mut worklist);
          }
        },
        Obj::Upvalue(cell) => {
          if let UpvalueState::Closed(v) = cell.get() {
            Self::mark_root(v, &mut reachable, &mut worklist);
          }
        },
        Obj::Str(_) | Obj::Bytes(_) | Obj::BigInt(_) | Obj::Native(_) => {},
      }
    }

    let freed = self.heap.sweep(&reachable);

    if *LOG_GC {
      eprintln!(
        "[gc] freed {}/{} objects, {} -> {} bytes (next collection at {} bytes)",
        freed,
        before_count,
        before_bytes,
        self.heap.bytes_allocated(),
        self.heap.next_gc()
      );
    }
  }

  /// Add `v` to the reachable set and, the first time it's seen, queue
  /// it so `collect_garbage` walks its children too. A no-op on repeat
  /// visits, which is what makes cycles (e.g. a closure capturing a
  /// variable that in turn points back at the closure) safe to trace.
  fn mark_root(v: Value, reachable: &mut HashSet<*const Obj>, worklist: &mut Vec<*const Obj>) {
    if !v.is_obj() {
      return;
    }
    let ptr = v.as_obj();
    if reachable.insert(ptr) {
      worklist.push(ptr);
    }
  }
}
