use std::cell::Cell;
use std::collections::{HashMap, HashSet};
use std::ops::{Neg, Shl, Shr};
use std::sync::LazyLock;

use num_bigint::BigInt;
use num_traits::ToPrimitive;

use crate::builtins;
use crate::vm::chunk::{Instr, JumpKey};
use crate::vm::natives;
use crate::vm::object::{
  Heap, Obj, ObjClass, ObjClosure, ObjFunction, UpvalueDescriptor, UpvalueState, ZuriContext,
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

/// One active `catch` statement's unwind target -- see the module-level
/// design note in this diff's accompanying explanation for the full
/// reasoning behind `frame_depth`'s role in distinguishing "this
/// exception is mine to catch" from "belongs to an ancestor
/// `run_until` invocation, possibly across a `call_value` boundary".
struct CatchHandler {
  /// `self.frames.len()` at the moment `PushCatch` executed -- the
  /// frame containing the `catch` statement itself is `frames[frame_depth-1]`.
  frame_depth: usize,
  /// Absolute instruction index (within that same frame) to resume at,
  /// whichever path is taken -- normal completion or an unwind.
  resume_ip: usize,
  var_reg: Option<u8>,
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
  /// Explicit extra GC roots for values internal (non-bytecode) VM code
  /// needs to keep alive across a call that might itself trigger a
  /// collection -- e.g. `instantiate` invoking several field
  /// initializers in sequence. A Value sitting only in a local Rust
  /// variable, with nothing in any register/global/frame pointing at
  /// it, is invisible to the normal root scan; push it here for exactly
  /// as long as it needs to survive, then truncate back off.
  gc_pins: Vec<Value>,
  /// Active `catch` handlers, innermost (most recently pushed) last --
  /// see `CatchHandler`'s own doc comment.
  catch_stack: Vec<CatchHandler>,
  /// Cached by name after `prelude::install` runs, for O(1) lookup from
  /// `VM::raise` rather than a `self.globals` hashmap hit on every
  /// internal error.
  pub(crate) builtin_exceptions: HashMap<&'static str, Value>,
  pub heap: Heap,
}

type RunResult<T> = Result<T, Value>;

const LOG_GC: LazyLock<bool> =
  std::sync::LazyLock::new(|| std::env::var_os("ZURI_GC_LOG").is_some());

impl VM {
  pub fn new(heap: Heap, globals: HashMap<String, Value>) -> Self {
    VM {
      registers: Vec::new(),
      frames: Vec::new(),
      open_upvalues: Vec::new(),
      gc_pins: Vec::new(),
      catch_stack: Vec::new(),
      builtin_exceptions: HashMap::new(),
      globals,
      heap,
    }
  }

  pub fn init(&mut self) {
    natives::install(self);
    crate::vm::prelude::install(self);
  }

  pub fn heap_mut(&mut self) -> &mut Heap {
    &mut self.heap
  }

  /// Bind a value directly, useful for wiring up a top-level function
  /// (e.g. "fib") before `run` starts executing.
  pub fn define_global(&mut self, name: impl Into<String>, v: Value) {
    self.globals.insert(name.into(), v);
  }

  pub fn lookup_global(&self, name: &str) -> Option<Value> {
    self.globals.get(name).copied()
  }

  /// Construct a fresh instance of the builtin exception class named
  /// `class_name` (from `prelude::EXCEPTION_CLASS_NAMES`), with
  /// `message`/`type` set directly by field-slot name (bypassing the
  /// normal constructor-call path entirely, since these are ALWAYS the
  /// prelude's own known classes -- no user override to worry about),
  /// and its `stacktrace` attached. This is what every internal VM
  /// error site calls instead of returning a bare Rust string.
  pub(crate) fn raise(&mut self, class_name: &'static str, message: impl Into<String>) -> Value {
    let message_str = message.into();
    let class_val = *self.builtin_exceptions.get(class_name).unwrap_or_else(|| {
      panic!(
        "internal error: unknown builtin exception class '{}' (prelude not installed?)",
        class_name
      )
    });

    let field_count = class_val.as_class().field_count;
    let instance = self.heap.alloc_instance(class_val, field_count as usize);
    let message_val = self.heap.alloc_string(message_str);
    let type_val = self.heap.alloc_string(class_name.to_string());

    {
      let class = class_val.as_class();
      let inst = instance.as_instance();
      if let Some(&idx) = class.field_slots.get("message") {
        inst.fields[idx as usize].set(message_val);
      }
      if let Some(&idx) = class.field_slots.get("type") {
        inst.fields[idx as usize].set(type_val);
      }
    }

    self.attach_stacktrace(instance)
  }

  /// Is `v` an instance of `Error` or one of its subclasses? What
  /// `Instr::Raise` checks before allowing a value to propagate as an
  /// error, and what `raise`'s own output always satisfies trivially.
  fn is_exception_value(&self, v: Value) -> bool {
    if !v.is_instance() {
      return false;
    }
    let Some(&exception_class) = self.builtin_exceptions.get("Error") else {
      return false;
    };
    let mut cur = Some(v.as_instance().class);
    while let Some(c) = cur {
      if c.equals(&exception_class) {
        return true;
      }
      cur = c.as_class().superclass;
    }
    false
  }

  /// Frame names, innermost first, as a Zuri list of strings -- what
  /// gets attached to every exception's `stacktrace` field. No line
  /// numbers yet (bytecode doesn't carry source positions), just the
  /// call chain by function name.
  fn build_stacktrace(&mut self) -> Value {
    let mut lines = Vec::with_capacity(self.frames.len());

    for frame in self.frames.iter().rev() {
      let func = unsafe { &*frame.function };
      let line = func
        .chunk
        .lines
        .get(frame.ip.saturating_sub(1))
        .copied()
        .unwrap_or(0);
      let entry = format!("{}:{} -> {}()", func.source_path, line, func.name);
      lines.push(self.heap.alloc_string(entry));
    }

    self.heap.alloc_list(lines)
  }

  fn attach_stacktrace(&mut self, instance: Value) -> Value {
    let trace = self.build_stacktrace();
    if instance.is_instance() {
      let inst = instance.as_instance();
      let idx = inst.class.as_class().field_slots.get("stacktrace").copied();
      if let Some(idx) = idx {
        inst.fields[idx as usize].set(trace);
      }
    }
    instance
  }

  /// Formats an uncaught exception for top-level reporting (see
  /// zuri.rs) -- "TYPE: message", falling back gracefully if `exc`
  /// somehow isn't an instance at all.
  pub fn describe_exception(&self, exc: Value) -> String {
    if !exc.is_instance() {
      return format!("{}", exc);
    }
    let inst = exc.as_instance();
    let class = inst.class.as_class();
    let message = class
      .field_slots
      .get("message")
      .map(|&idx| inst.fields[idx as usize].get().to_string())
      .unwrap_or_else(|| "An unexpected error has occurred".to_string());
    let type_name = class
      .field_slots
      .get("type")
      .map(|&idx| inst.fields[idx as usize].get().to_string())
      .unwrap_or_else(|| class.name.clone());
    format!("{}: {}", type_name, message)
  }

  /// Full multi-line "Unhandled ..." block for an uncaught exception --
  /// what the CLI/REPL print at the top level (see zuri.rs).
  /// `describe_exception` stays the short "TYPE: message" summary,
  /// still used e.g. by the prelude's own internal panic path.
  pub fn format_uncaught(&self, exc: Value) -> String {
    let summary = self.describe_exception(exc);
    if !exc.is_instance() {
      return format!("Unhandled {}", summary);
    }
    let inst = exc.as_instance();
    let trace_idx = inst.class.as_class().field_slots.get("stacktrace").copied();
    let mut out = format!("Unhandled {}\n  StackTrace:", summary);
    if let Some(idx) = trace_idx {
      let trace_val = inst.fields[idx as usize].get();
      if trace_val.is_list() {
        for line in trace_val.as_list() {
          out.push_str(&format!("\n    {}", line));
        }
      }
    }
    out
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
      let msg = format!("cannot call a {}", callee.type_name());
      return Err(self.raise("TypeError", msg));
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
      let msg = format!(
        "'{}' expects {}{} argument(s), got {}",
        native.name,
        if native.variadic { "at least " } else { "" },
        native.min_arity,
        args.len()
      );
      return Err(self.raise("ArgumentError", msg));
    }

    let mut ctx = ZuriContext { vm: self, args };
    let result = (native.func)(&mut ctx);
    result.map_err(|msg| self.raise("TypeError", msg))
  }

  /// Construct a new instance of `class_val`: allocate storage sized to
  /// its (already-merged) field layout, run every ancestor's OWN field
  /// initializer root-to-leaf, then call the resolved constructor (if
  /// any) with `args`.
  ///
  /// This is the one place internal VM code makes several SEQUENTIAL
  /// re-entrant calls (`call_value`, which can itself trigger a
  /// collection) while depending on Values that live only in local Rust
  /// variables in between -- the class itself, its ancestors' field
  /// initializers, the constructor, and the caller's own `args`. None of
  /// those are reachable through any register/global/frame during that
  /// window, so each is explicitly pinned for the duration (see
  /// `gc_pins`) rather than trusting the normal root scan to find them.
  fn instantiate(&mut self, class_val: Value, args: &[Value]) -> RunResult<Value> {
    let mut field_inits = Vec::new();
    let mut cur = Some(class_val);
    while let Some(c) = cur {
      let cobj = c.as_class();
      field_inits.push(cobj.own_field_initializer);
      cur = cobj.superclass;
    }
    field_inits.reverse(); // root to leaf
    let constructor = class_val.as_class().constructor;
    let field_count = class_val.as_class().field_count;

    let pin_mark = self.gc_pins.len();
    self.gc_pins.push(class_val);
    for f in field_inits.iter().flatten() {
      self.gc_pins.push(*f);
    }
    if let Some(c) = constructor {
      self.gc_pins.push(c);
    }
    for a in args {
      self.gc_pins.push(*a);
    }

    let instance_val = self.heap.alloc_instance(class_val, field_count as usize);
    self.gc_pins.push(instance_val);

    let result: RunResult<()> = (|| {
      for init in field_inits.iter().flatten() {
        self.call_value(*init, &[instance_val])?;
      }
      if let Some(ctor) = constructor {
        let mut ctor_args = Vec::with_capacity(args.len() + 1);
        ctor_args.push(instance_val);
        ctor_args.extend_from_slice(args);
        self.call_value(ctor, &ctor_args)?;
      }
      Ok(())
    })();

    self.gc_pins.truncate(pin_mark);
    result?;
    Ok(instance_val)
  }

  /// Shared "call whatever's in register `func_reg`" logic -- the exact
  /// dispatch `Instr::Call` performs, factored out so Invoke/InvokeSuper's
  /// field-fallback (a field that happens to hold a callable, e.g. `var
  /// _print = @(g) { ... }`) can reach it too, rather than duplicating
  /// native/class/bound-method/closure dispatch a second time. `func_reg`
  /// and `dst` are relative to `base`; arguments must already sit at
  /// `func_reg+1 ..= func_reg+num_args` -- ordinary data-call convention,
  /// arity does NOT include any implicit receiver.
  fn dispatch_call(&mut self, base: usize, func_reg: u8, num_args: u8, dst: u8) -> RunResult<()> {
    let callee = self.get_reg(base, func_reg);

    if callee.is_native() {
      let args_start = base + func_reg as usize + 1;
      let args_end = args_start + num_args as usize;
      let args: Vec<Value> = self.registers[args_start..args_end].to_vec();
      let native = callee.as_native();
      let result = self.call_native(native, &args)?;
      self.set_reg(base, dst, result);
      return Ok(());
    }

    if callee.is_class() {
      let args_start = base + func_reg as usize + 1;
      let args_end = args_start + num_args as usize;
      let user_args: Vec<Value> = self.registers[args_start..args_end].to_vec();
      let instance = self.instantiate(callee, &user_args)?;
      self.set_reg(base, dst, instance);
      return Ok(());
    }

    if callee.is_bound_method() {
      let args_start = base + func_reg as usize + 1;
      let args_end = args_start + num_args as usize;
      let bound = callee.as_bound_method();
      let mut full_args = Vec::with_capacity(num_args as usize + 1);
      full_args.push(bound.receiver);
      full_args.extend_from_slice(&self.registers[args_start..args_end]);
      let method = bound.method;
      let result = self.call_value(method, &full_args)?;
      self.set_reg(base, dst, result);
      return Ok(());
    }

    if !callee.is_closure() {
      let msg = format!("cannot call a {}", callee.type_name());
      return Err(self.raise("TypeError", msg));
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
    Ok(())
  }

  /// Call a CLOSURE whose implicit receiver has ALREADY been placed by
  /// the compiler at `recv_reg + 1` -- the convention behind a genuine
  /// method call (`is_method` reserved that slot at compile time; see
  /// `ObjFunction::is_method`'s doc comment). Unlike `dispatch_call`,
  /// `callee` itself is never written into any register here -- it's
  /// consulted only for its function pointers, since the receiver
  /// occupying what would otherwise be the callee's register is exactly
  /// the point of the fused Invoke/InvokeSuper instructions.
  fn invoke_prebound(
    &mut self,
    base: usize,
    recv_reg: u8,
    callee: Value,
    num_args: u8,
    dst: u8,
  ) -> RunResult<()> {
    if !callee.is_closure() {
      let msg = format!("cannot call a {}", callee.type_name());
      return Err(self.raise("TypeError", msg));
    }

    let callee_closure = callee.as_closure();
    let callee_fn = callee_closure.function.as_func();

    let required = if callee_fn.variadic {
      callee_fn.arity - 1
    } else {
      callee_fn.arity
    };
    let new_base = base + recv_reg as usize + 1;
    let needed = new_base + callee_fn.num_registers as usize;
    if self.registers.len() < needed {
      self.registers.resize(needed, Value::nil());
    }
    for i in (1 + num_args)..required {
      self.registers[new_base + i as usize] = Value::nil();
    }
    if callee_fn.variadic {
      let extra_count = (1 + num_args).saturating_sub(required);
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
    Ok(())
  }

  //-----------------------------------------------------------------------------------
  // Indexing and slicing
  //-----------------------------------------------------------------------------------

  fn index_get(&mut self, receiver: Value, index: Value) -> RunResult<Value> {
    if receiver.is_list() {
      let i = self.coerce_index(index, receiver.list_len())?;
      Ok(receiver.list_get(i).unwrap())
    } else if receiver.is_bytes() {
      let i = self.coerce_index(index, receiver.bytes_len())?;
      Ok(Value::number(receiver.bytes_get(i).unwrap() as f64))
    } else if receiver.is_string() {
      let chars_len = receiver.as_str().chars().count();
      let i = self.coerce_index(index, chars_len)?;
      let c = receiver.as_str().chars().nth(i).unwrap();
      Ok(self.heap.alloc_string(c.to_string()))
    } else if receiver.is_dict() {
      match receiver.dict_get(&index) {
        Some(v) => Ok(v),
        None => Err(self.raise(
          "PropertyError",
          format!("undefined key '{}' in dict", index),
        )),
      }
    } else {
      Err(self.raise(
        "TypeError",
        format!("cannot index into a {}", receiver.type_name()),
      ))
    }
  }

  fn index_set(&mut self, receiver: Value, index: Value, value: Value) -> RunResult<()> {
    if receiver.is_list() {
      let i = self.coerce_index(index, receiver.list_len())?;
      receiver.list_set(i, value);
      Ok(())
    } else if receiver.is_bytes() {
      let i = self.coerce_index(index, receiver.bytes_len())?;
      if !value.is_number() {
        let msg = format!("bytes element must be a number, got {}", value.type_name());
        return Err(self.raise("TypeError", msg));
      }
      let n = value.as_number();
      if n.fract() != 0.0 || !(0.0..=255.0).contains(&n) {
        let msg = format!("bytes element must be an integer in 0..=255, got {}", n);
        return Err(self.raise("NumericError", msg));
      }
      receiver.bytes_set(i, n as u8);
      Ok(())
    } else if receiver.is_dict() {
      receiver.dict_set(index, value);
      Ok(())
    } else if receiver.is_string() {
      Err(self.raise(
        "TypeError",
        "strings are immutable and do not support index assignment",
      ))
    } else {
      Err(self.raise(
        "TypeError",
        format!("cannot assign into a {}", receiver.type_name()),
      ))
    }
  }

  fn index_slice(&mut self, receiver: Value, lo: Value, hi: Value) -> RunResult<Value> {
    if receiver.is_list() {
      let len = receiver.list_len();
      let bounds = self.resolve_slice_bounds(lo, hi, len)?;
      let items: Vec<Value> = match bounds {
        Some((lo, hi)) => (lo..=hi).map(|i| receiver.list_get(i).unwrap()).collect(),
        None => Vec::new(),
      };
      Ok(self.heap.alloc_list(items))
    } else if receiver.is_bytes() {
      let len = receiver.bytes_len();
      let bounds = self.resolve_slice_bounds(lo, hi, len)?;
      let items: Vec<u8> = match bounds {
        Some((lo, hi)) => (lo..=hi).map(|i| receiver.bytes_get(i).unwrap()).collect(),
        None => Vec::new(),
      };
      Ok(self.heap.alloc_bytes(items))
    } else if receiver.is_string() {
      let chars: Vec<char> = receiver.as_str().chars().collect();
      let bounds = self.resolve_slice_bounds(lo, hi, chars.len())?;
      let s: String = match bounds {
        Some((lo, hi)) => chars[lo..=hi].iter().collect(),
        None => String::new(),
      };
      Ok(self.heap.alloc_string(s))
    } else {
      Err(self.raise(
        "TypeError",
        format!("cannot slice a {}", receiver.type_name()),
      ))
    }
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
        let msg = format!("fell off the end of '{}' without a Return", func.name);
        return Err(self.raise("Error", msg));
      }

      let instr = func.chunk.code[ip];
      self.frames[frame_idx].ip += 1;

      let step: RunResult<Option<Value>> = (|| {
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
          Instr::Mul { dst, a, b } => self.binary_mult(base, dst, a, b, "*")?,
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
              let msg = format!("cannot bitwise not a {}", v.type_name());
              return Err(self.raise("TypeError", msg));
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
              let msg = format!("cannot negate a {}", v.type_name());
              return Err(self.raise("TypeError", msg));
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
            self.dispatch_call(base, func_reg, num_args, dst)?;
          },
          Instr::Return { src } => {
            let ret = self.get_reg(base, src);
            self.close_upvalues_from(base);
            let finished = self.frames.pop().unwrap();
            if self.frames.len() == stop_depth {
              return Ok(Some(ret));
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
            let found = self.globals.get(&name).copied();
            let v = match found {
              Some(v) => v,
              None => {
                return Err(self.raise("UndefinedError", format!("undefined global '{}'", name)));
              },
            };
            self.set_reg(base, dst, v);
          },
          Instr::SetGlobal { name_const, src } => {
            let name = self.const_as_str(func, name_const)?;
            let v = self.get_reg(base, src);
            self.globals.insert(name, v);
          },
          Instr::AssignGlobal { name_const, src } => {
            let name = self.const_as_str(func, name_const)?;
            let value = self.get_reg(base, src);

            let found = self.globals.get(&name).copied();
            match found {
              Some(_) => self.globals.insert(name, value),
              None => {
                return Err(self.raise("UndefinedError", format!("undefined global '{}'", name)));
              },
            };
          },

          Instr::Closure { dst, proto_const } => {
            let proto_val = func.chunk.constants[proto_const as usize];
            if !proto_val.is_func() {
              return Err(self.raise("TypeError", "Closure operand is not a function"));
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
              return Err(self.raise("TypeError", "GetUpval operand is not an upvalue"));
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
              return Err(self.raise("TypeError", "SetUpval operand is not an upvalue"));
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

          Instr::MakeClass {
            dst,
            name_const,
            superclass,
          } => {
            let name = self.const_as_str(func, name_const)?;
            let superclass_val = match superclass {
              Some(r) => {
                let v = self.get_reg(base, r);
                if !v.is_class() {
                  let msg = format!(
                    "superclass of '{}' is not a class (got a {})",
                    name,
                    v.type_name()
                  );
                  return Err(self.raise("TypeError", msg));
                }
                Some(v)
              },
              None => None,
            };

            let (methods, field_slots, field_count, constructor) = match superclass_val {
              Some(sup) => {
                let s = sup.as_class();
                (
                  s.methods.clone(),
                  s.field_slots.clone(),
                  s.field_count,
                  s.constructor,
                )
              },
              None => (HashMap::new(), HashMap::new(), 0, None),
            };

            let class_val = self.heap.alloc_class(ObjClass {
              name,
              superclass: superclass_val,
              methods,
              field_slots,
              field_count,
              own_field_initializer: None,
              constructor,
              static_slots: HashMap::new(),
              statics: Vec::new(),
            });
            self.set_reg(base, dst, class_val);
          },

          Instr::DeclareField { class, name_const } => {
            let class_val = self.get_reg(base, class);
            let name = self.const_as_str(func, name_const)?;
            let mut c = class_val.as_class_mut();

            // Idempotent: a name that's already present -- inherited from
            // the superclass (invisible to the compiler's own self.x=...
            // scan, which only sees this class's own AST -- see
            // Compiler::compile_class_decl) or already declared earlier
            // in this same class -- keeps its EXISTING slot rather than
            // being handed a new one. Reassigning would silently desync
            // an inherited field initializer (which writes to the OLD
            // index) from every later read/write of the same name
            // (which would then resolve to the NEW index instead).
            if !c.field_slots.contains_key(&name) {
              let idx = c.field_count;
              c.field_slots.insert(name, idx);
              c.field_count += 1;
            }
          },

          Instr::SetFieldInit { class, src } => {
            let class_val = self.get_reg(base, class);
            let init = self.get_reg(base, src);
            class_val.as_class_mut().own_field_initializer = Some(init);
          },

          Instr::SetMethod {
            class,
            name_const,
            src,
          } => {
            let class_val = self.get_reg(base, class);
            let name = self.const_as_str(func, name_const)?;
            let method = self.get_reg(base, src);
            class_val.as_class_mut().methods.insert(name, method);
          },

          Instr::DeclareStatic {
            class,
            name_const,
            src,
          } => {
            let class_val = self.get_reg(base, class);
            let name = self.const_as_str(func, name_const)?;
            let value = self.get_reg(base, src);
            let mut c = class_val.as_class_mut();
            let idx = c.statics.len() as u16;
            c.static_slots.insert(name, idx);
            c.statics.push(Cell::new(value));
          },

          Instr::FinalizeClass { class } => {
            let class_val = self.get_reg(base, class);
            let name = self.const_as_str(func, 0)?;
            let mut c = class_val.as_class_mut();

            if self.globals.get(&c.name).is_some() {
              return Err(self.raise(
                "Error",
                format!("class '{}' already declared in this scope", c.name),
              ));
            }

            if let Some(ctor) = c.methods.get(&name).copied() {
              c.constructor = Some(ctor);
            }
          },

          Instr::GetField {
            dst,
            obj,
            name_const,
          } => {
            let receiver = self.get_reg(base, obj);
            let name = self.const_as_str(func, name_const)?;
            let value = if receiver.is_instance() {
              let inst = receiver.as_instance();
              let class = inst.class.as_class();
              if let Some(&idx) = class.field_slots.get(&name) {
                inst.fields[idx as usize].get()
              } else if let Some(method) = class.methods.get(&name).copied() {
                // Accessed without an immediate call -- e.g. `var f =
                // obj.method` -- so unlike Invoke, this can't skip
                // allocation: bind the receiver into a real ObjBoundMethod
                // so the resulting value is independently callable later.
                self.heap.alloc_bound_method(receiver, method)
              } else {
                let msg = format!(
                  "undefined property '{}' on instance of '{}'",
                  name, class.name
                );
                return Err(self.raise("PropertyError", msg));
              }
            } else if receiver.is_class() {
              let raw = lookup_static(receiver, &name)
                .ok_or_else(|| {
                  format!(
                    "undefined static member '{}' on class '{}'",
                    name,
                    receiver.as_class().name
                  )
                })
                .map_err(|msg| self.raise("PropertyError", msg))?;
              // A static METHOD's frame reserves register 0 for a
              // receiver it never reads (see ObjFunction::is_method's doc
              // comment) -- fetched bare like this, nothing would
              // otherwise fill that register at call time, so wrap it
              // with a harmless dummy nil receiver. A static field
              // holding a plain closure value (`static var f = @(x){}`)
              // has no such reservation and passes through unwrapped.
              if raw.is_closure() && raw.as_closure().function.as_func().is_method {
                self.heap.alloc_bound_method(Value::nil(), raw)
              } else {
                raw
              }
            } else {
              let msg = format!(
                "cannot read property '{}' on a {}",
                name,
                receiver.type_name()
              );
              return Err(self.raise("TypeError", msg));
            };
            self.set_reg(base, dst, value);
          },

          Instr::SetField {
            obj,
            name_const,
            src,
          } => {
            let receiver = self.get_reg(base, obj);
            let value = self.get_reg(base, src);
            let name = self.const_as_str(func, name_const)?;
            if receiver.is_instance() {
              let inst = receiver.as_instance();
              let class = inst.class.as_class();
              let idx = *class
                .field_slots
                .get(&name)
                .ok_or_else(|| {
                  format!("undefined field '{}' on instance of '{}'", name, class.name)
                })
                .map_err(|msg| self.raise("PropertyError", msg))?;
              inst.fields[idx as usize].set(value);
            } else if receiver.is_class() {
              set_static(receiver, &name, value).map_err(|msg| self.raise("PropertyError", msg))?;
            } else {
              let msg = format!(
                "cannot set property '{}' on a {}",
                name,
                receiver.type_name()
              );
              return Err(self.raise("TypeError", msg));
            }
          },

          Instr::Invoke {
            dst,
            obj,
            method_const,
            num_args,
          } => {
            let receiver = self.get_reg(base, obj);
            let method_name = self.const_as_str(func, method_const)?;

            if receiver.is_instance() {
              let inst = receiver.as_instance();
              let class_val = inst.class;
              let found = {
                let class = class_val.as_class();
                if let Some(m) = class.methods.get(&method_name).copied() {
                  Some(Ok(m))
                } else if let Some(&idx) = class.field_slots.get(&method_name) {
                  Some(Err(idx))
                } else {
                  None
                }
              };

              match found {
                Some(Ok(method)) => self.invoke_prebound(base, obj, method, num_args, dst)?,
                Some(Err(idx)) => {
                  // A plain field that happens to hold a callable (e.g.
                  // `var _print = @(g) { ... }`) -- called as ordinary
                  // data, NOT as a method: no implicit self, matching
                  // what would happen if it were fetched into a variable
                  // and called from there. Overwriting the compiler's
                  // (here-irrelevant) self-duplicate register with the
                  // field's own value gives dispatch_call exactly what
                  // it needs -- the real arguments already sit right
                  // after it, at obj+2 onward.
                  let field_value = inst.fields[idx as usize].get();
                  self.set_reg(base, obj + 1, field_value);
                  self.dispatch_call(base, obj + 1, num_args, dst)?;
                },
                None => match builtins::lookup(receiver, &method_name) {
                  Some(native) => {
                    let args_start = base + obj as usize + 2; // was + 1
                    let args_end = args_start + num_args as usize;
                    let mut call_args = Vec::with_capacity(num_args as usize + 1);
                    call_args.push(receiver);
                    call_args.extend_from_slice(&self.registers[args_start..args_end]);
                    let result = self.call_native(native, &call_args)?;
                    self.set_reg(base, dst, result);
                  },
                  None => {
                    let msg = format!(
                      "undefined property '{}' on instance of '{}'",
                      method_name,
                      class_val.as_class().name
                    );
                    return Err(self.raise("PropertyError", msg));
                  },
                },
              }
            } else if receiver.is_class() {
              let callee = lookup_static(receiver, &method_name)
                .ok_or_else(|| {
                  format!(
                    "undefined static member '{}' on class '{}'",
                    method_name,
                    receiver.as_class().name
                  )
                })
                .map_err(|msg| self.raise("PropertyError", msg))?;
              if callee.is_closure() && callee.as_closure().function.as_func().is_method {
                self.invoke_prebound(base, obj, callee, num_args, dst)?;
              } else {
                // A static field holding a plain callable (`static var f
                // = @(x){}`), not a declared `static` method -- no
                // reserved receiver slot, so treat exactly like the
                // instance field-fallback above.
                self.set_reg(base, obj + 1, callee);
                self.dispatch_call(base, obj + 1, num_args, dst)?;
              }
            } else {
              match builtins::lookup(receiver, &method_name) {
                Some(native) => {
                  let args_start = base + obj as usize + 2; // was + 1
                  let args_end = args_start + num_args as usize;
                  let mut call_args = Vec::with_capacity(num_args as usize + 1);
                  call_args.push(receiver);
                  call_args.extend_from_slice(&self.registers[args_start..args_end]);
                  let result = self.call_native(native, &call_args)?;
                  self.set_reg(base, dst, result);
                },
                None => {
                  let msg = format!(
                    "object of type {} does not define method '{}'",
                    receiver.type_name(),
                    method_name
                  );
                  return Err(self.raise("TypeError", msg));
                },
              }
            }
          },

          Instr::InvokeSuper {
            dst,
            superclass,
            method_const,
            num_args,
          } => {
            let super_val = self.get_reg(base, superclass);
            if !super_val.is_class() {
              let msg = format!(
                "'parent' does not refer to a class (got a {})",
                super_val.type_name()
              );
              return Err(self.raise("TypeError", msg));
            }

            let method_name = self.const_as_str(func, method_const)?;
            let found = {
              let class = super_val.as_class();
              class.methods.get(&method_name).copied()
            };

            if let Some(method) = found {
              self.invoke_prebound(base, superclass, method, num_args, dst)?;
            } else {
              // Not a method anywhere up the (statically fixed) chain --
              // fields were never virtual to begin with, so fall back to
              // self's own field slots directly, same as Invoke's own
              // fallback. `self` is already sitting in register
              // `superclass + 1` (placed there by
              // Compiler::compile_invoke_super), so reuse it as both the
              // field owner and, if found, the overwritten callee
              // register.
              let self_val = self.get_reg(base, superclass + 1);
              if !self_val.is_instance() {
                let msg = format!(
                  "undefined method '{}' on superclass '{}'",
                  method_name,
                  super_val.as_class().name
                );
                return Err(self.raise("PropertyError", msg));
              }

              let inst = self_val.as_instance();
              let idx = *inst
                .class
                .as_class()
                .field_slots
                .get(&method_name)
                .ok_or_else(|| {
                  format!(
                    "undefined method '{}' on superclass '{}'",
                    method_name,
                    super_val.as_class().name
                  )
                })
                .map_err(|msg| self.raise("PropertyError", msg))?;

              let field_value = inst.fields[idx as usize].get();
              self.set_reg(base, superclass + 1, field_value);
              self.dispatch_call(base, superclass + 1, num_args, dst)?;
            }
          },
          Instr::CallSuperCtor {
            dst,
            superclass,
            num_args,
          } => {
            let super_val = self.get_reg(base, superclass);
            if !super_val.is_class() {
              let msg = format!(
                "'parent' does not refer to a class (got a {})",
                super_val.type_name()
              );
              return Err(self.raise("TypeError", msg));
            }
            let ctor = super_val.as_class().constructor;
            match ctor {
              Some(ctor) => self.invoke_prebound(base, superclass, ctor, num_args, dst)?,
              None => {
                let msg = format!(
                  "class '{}' has no constructor to call via parent()",
                  super_val.as_class().name
                );
                return Err(self.raise("AccessError", msg));
              },
            }
          },

          Instr::GetIndex { dst, obj, idx } => {
            let ov = self.get_reg(base, obj);
            let iv = self.get_reg(base, idx);
            let result = self.index_get(ov, iv)?;
            self.set_reg(base, dst, result);
          },
          Instr::SetIndex { obj, idx, src } => {
            let ov = self.get_reg(base, obj);
            let iv = self.get_reg(base, idx);
            let sv = self.get_reg(base, src);
            self.index_set(ov, iv, sv)?;
          },
          Instr::GetSlice { dst, obj, lo, hi } => {
            let ov = self.get_reg(base, obj);
            let lov = self.get_reg(base, lo);
            let hiv = self.get_reg(base, hi);
            let result = self.index_slice(ov, lov, hiv)?;
            self.set_reg(base, dst, result);
          },

          Instr::MakeRange { dst, lower, upper } => {
            let lo = self.get_reg(base, lower);
            let hi = self.get_reg(base, upper);
            if !lo.is_number() || !hi.is_number() {
              let msg = format!(
                "range bounds must be numbers, got {} and {}",
                lo.type_name(),
                hi.type_name()
              );
              return Err(self.raise("TypeError", msg));
            }
            let range_val = self.heap.alloc_range(lo.as_number(), hi.as_number());
            self.set_reg(base, dst, range_val);
          },
          Instr::UsingJump { subject, table_idx } => {
            let v = self.get_reg(base, subject);
            if let Some(key) = value_to_jump_key(v) {
              if let Some(&target) = func.chunk.jump_tables[table_idx as usize].get(&key) {
                self.frames[frame_idx].ip = target;
              }
            }
            // A miss (unhashable value, or just no matching entry) is not
            // an error -- it simply falls through to the next instruction,
            // where the sequential dynamic-label checks (and the default
            // case) live. See Compiler::compile_using.
          },

          Instr::Raise { src } => {
            let value = self.get_reg(base, src);
            if !self.is_exception_value(value) {
              let msg = format!(
                "can only raise an Error or subclass, got a {}",
                value.type_name()
              );
              return Err(self.raise("TypeError", msg));
            }
            let value = self.attach_stacktrace(value);
            return Err(value);
          },

          Instr::PushCatch { var_reg, offset } => {
            let resume_ip = (self.frames[frame_idx].ip as isize + offset as isize) as usize;
            self.catch_stack.push(CatchHandler {
              frame_depth: self.frames.len(),
              resume_ip,
              var_reg,
            });
          },

          Instr::PopCatch => {
            self.catch_stack.pop();
          },
        }
        Ok(None)
      })();

      match step {
        Ok(Some(ret)) => return Ok(ret),
        Ok(None) => continue,
        Err(exc) => {
          let claims_it = matches!(self.catch_stack.last(), Some(h) if h.frame_depth > stop_depth);
          if claims_it {
            let handler = self.catch_stack.pop().unwrap();
            // Close upvalues into any frame about to be discarded --
            // without this, a closure created (and possibly already
            // escaped) inside the unwound call chain could keep
            // pointing at a register that's now free for reuse.
            if let Some(discard_base) = self.frames.get(handler.frame_depth).map(|f| f.base) {
              self.close_upvalues_from(discard_base);
            }
            self.frames.truncate(handler.frame_depth);
            let top = self.frames.last_mut().expect("catch handler left no frame");
            top.ip = handler.resume_ip;
            let top_base = top.base;
            if let Some(reg) = handler.var_reg {
              self.set_reg(top_base, reg, exc);
            }
            continue;
          }
          return Err(exc);
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

  fn const_as_str(&mut self, func: &ObjFunction, idx: u16) -> RunResult<String> {
    let v = func.chunk.constants[idx as usize];
    if !v.is_string() {
      return Err(self.raise("TypeError", "expected a string constant for a global name"));
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

    let msg = format!(
      "operator '{}' expects numbers, got {} and {}",
      op_name,
      va.type_name(),
      vb.type_name()
    );
    Err(self.raise("TypeError", msg))
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

    let msg = format!(
      "operator '{}' not defined for call signature ({}, {})",
      op_name,
      va.argument_type_name(),
      vb.argument_type_name()
    );
    Err(self.raise("TypeError", msg))
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
    } else if va.is_list() || vb.is_list() {
      let va = self.get_reg(base, a);
      let vb = self.get_reg(base, b);

      let mut value = Vec::new();
      value.extend(va.as_list().iter().cloned());
      value.extend(vb.as_list().iter().cloned());
      let v = self.heap.alloc_list(value);
      return Ok(self.set_reg(base, dst, v));
    } else if va.is_bytes() || vb.is_bytes() {
      let va = self.get_reg(base, a);
      let vb = self.get_reg(base, b);

      let mut value = Vec::new();
      value.extend(va.as_bytes().iter().cloned());
      value.extend(vb.as_bytes().iter().cloned());
      let v = self.heap.alloc_bytes(value);
      return Ok(self.set_reg(base, dst, v));
    }

    let msg = format!(
      "operator '{}' not defined for call signature ({}, {})",
      op_name,
      va.argument_type_name(),
      vb.argument_type_name()
    );
    Err(self.raise("TypeError", msg))
  }

  fn binary_mult(&mut self, base: usize, dst: u8, a: u8, b: u8, op_name: &str) -> RunResult<()> {
    let va = self.get_reg(base, a);
    let vb = self.get_reg(base, b);

    if va.is_number() && vb.is_number() {
      return Ok(self.set_reg(base, dst, Value::number(va.as_number() * vb.as_number())));
    } else if va.is_bigint() && vb.is_bigint() {
      let v = self.heap.alloc_bigint(va.as_bigint() * vb.as_bigint());
      return Ok(self.set_reg(base, dst, v));
    } else if va.is_string() && vb.is_number() {
      let va = self.get_reg(base, a);
      let vb = self.get_reg(base, b);
      let count = vb.as_number() as usize;

      let s = if count < usize::MAX {
        va.as_str().repeat(count)
      } else {
        String::new()
      };
      let v = self.heap.alloc_string(s);
      return Ok(self.set_reg(base, dst, v));
    } else if va.is_list() && vb.is_number() {
      let va = self.get_reg(base, a);
      let vb = self.get_reg(base, b);
      let count = vb.as_number() as usize;

      let value = if count < usize::MAX {
        va.as_list().to_vec().repeat(count)
      } else {
        Vec::new()
      };
      let v = self.heap.alloc_list(value);
      return Ok(self.set_reg(base, dst, v));
    }

    let msg = format!(
      "operator '{}' not defined for call signature ({}, {})",
      op_name,
      va.argument_type_name(),
      vb.argument_type_name()
    );
    Err(self.raise("TypeError", msg))
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
      let msg = format!(
        "operator '{}' not defined for {} and {}",
        op_name,
        va.argument_type_name(),
        vb.argument_type_name()
      );
      return Err(self.raise("TypeError", msg));
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
    for v in &self.gc_pins {
      Self::mark_root(*v, &mut reachable, &mut worklist);
    }

    while let Some(ptr) = worklist.pop() {
      // SAFETY: every pointer on the worklist was pulled out of a Value
      // that was itself still live when we queued it, and nothing is
      // freed until `sweep` runs below -- well after this loop -- so the
      // object behind `ptr` is guaranteed to still be valid here.
      match unsafe { &*ptr } {
        Obj::List(items) => {
          for cell in items {
            Self::mark_root(cell.get(), &mut reachable, &mut worklist);
          }
        },
        Obj::Dict(pairs) => {
          for (k, v) in pairs.borrow().iter() {
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
        Obj::Class(c) => {
          let class = c.borrow();
          if let Some(sup) = class.superclass {
            Self::mark_root(sup, &mut reachable, &mut worklist);
          }
          for m in class.methods.values() {
            Self::mark_root(*m, &mut reachable, &mut worklist);
          }
          if let Some(init) = class.own_field_initializer {
            Self::mark_root(init, &mut reachable, &mut worklist);
          }
          if let Some(ctor) = class.constructor {
            Self::mark_root(ctor, &mut reachable, &mut worklist);
          }
          for cell in &class.statics {
            Self::mark_root(cell.get(), &mut reachable, &mut worklist);
          }
        },
        Obj::Instance(inst) => {
          Self::mark_root(inst.class, &mut reachable, &mut worklist);
          for cell in &inst.fields {
            Self::mark_root(cell.get(), &mut reachable, &mut worklist);
          }
        },
        Obj::BoundMethod(b) => {
          Self::mark_root(b.receiver, &mut reachable, &mut worklist);
          Self::mark_root(b.method, &mut reachable, &mut worklist);
        },
        Obj::Str(_) | Obj::Bytes(_) | Obj::BigInt(_) | Obj::Native(_) | Obj::Range { .. } => {},
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

impl VM {
  fn value_as_index(&mut self, index: Value) -> RunResult<i64> {
    if !index.is_number() {
      let msg = format!("index must be a number, got {}", index.type_name());
      return Err(self.raise("TypeError", msg));
    }
    let n = index.as_number();
    if n.fract() != 0.0 {
      return Err(self.raise("TypeError", format!("index must be an integer, got {}", n)));
    }
    Ok(n as i64)
  }

  fn coerce_index(&mut self, index: Value, len: usize) -> RunResult<usize> {
    let i = self.value_as_index(index)?;
    if i < 0 || i as usize >= len {
      let msg = format!("index {} out of bounds (length {})", i, len);
      return Err(self.raise("RangeError", msg));
    }
    Ok(i as usize)
  }

  fn resolve_slice_bounds(
    &mut self,
    lo: Value,
    hi: Value,
    len: usize,
  ) -> RunResult<Option<(usize, usize)>> {
    if len == 0 {
      return Ok(None);
    }

    let lo = if lo.is_nil() {
      0
    } else {
      let i = self.value_as_index(lo)?;
      if i < 0 {
        return Err(self.raise(
          "RangeError",
          format!("slice lower bound {} cannot be negative", i),
        ));
      }
      i as usize
    };

    let hi = if hi.is_nil() {
      len - 1
    } else {
      let i = self.value_as_index(hi)?;
      if i < 0 {
        return Err(self.raise(
          "RangeError",
          format!("slice upper bound {} cannot be negative", i),
        ));
      }
      i as usize
    };

    if lo >= len || hi >= len {
      let msg = format!("slice bounds {}..{} out of range (length {})", lo, hi, len);
      return Err(self.raise("RangeError", msg));
    }

    if lo > hi {
      return Ok(None);
    }

    Ok(Some((lo, hi)))
  }
}

/// Walk `class_val`'s superclass chain looking for a static member
/// named `name`, checking each class's own (never inherited-in)
/// `static_slots` table -- see `ObjClass`'s doc comment for why statics
/// aren't pre-merged the way methods/fields are.
fn lookup_static(class_val: Value, name: &str) -> Option<Value> {
  let mut cur = Some(class_val);
  while let Some(c) = cur {
    let class = c.as_class();
    if let Some(&idx) = class.static_slots.get(name) {
      return Some(class.statics[idx as usize].get());
    }
    cur = class.superclass;
  }
  None
}

fn set_static(class_val: Value, name: &str, value: Value) -> Result<(), String> {
  let mut cur = Some(class_val);
  while let Some(c) = cur {
    let class = c.as_class();
    if let Some(&idx) = class.static_slots.get(name) {
      class.statics[idx as usize].set(value);
      return Ok(());
    }
    cur = class.superclass;
  }
  Err(format!(
    "undefined static member '{}' on class '{}'",
    name,
    class_val.as_class().name
  ))
}

/// Converts a runtime `using`-subject Value into the same hashable key
/// space `Instr::UsingJump`'s jump table was built in at compile time
/// (see `expr_as_jump_key` in compiler.rs). `None` for anything that
/// was never eligible to be a constant case label to begin with (list,
/// dict, instance, range, etc.) -- always falls through to the
/// sequential dynamic-label path rather than ever consulting the table.
fn value_to_jump_key(v: Value) -> Option<JumpKey> {
  if v.is_nil() {
    Some(JumpKey::Nil)
  } else if v.is_bool() {
    Some(JumpKey::Bool(v.as_bool()))
  } else if v.is_number() {
    Some(JumpKey::Number(v.as_number().to_bits()))
  } else if v.is_string() {
    Some(JumpKey::Str(v.as_str().to_string()))
  } else {
    None
  }
}
