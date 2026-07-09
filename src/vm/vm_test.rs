use crate::vm::chunk::{Chunk, Instr};
use crate::vm::object::{Heap, ObjFunction};
use crate::vm::value::Value;
use crate::vm::vm::VM;

/// Hand-assemble:
///
///   function fib(n) {
///       if (n < 2) return n;
///       return fib(n - 1) + fib(n - 2);
///   }
///
/// Register layout for fib:
///   r0 = n (the argument; live for the whole function)
///   r1 = fib(n - 1), once computed; live until the final Add
///   r2 = fib(n - 2), once computed; live until the final Add
///   r3 = scratch (constant 2, the condition, then the final sum -- each use is dead before the next)
///   r4 = scratch (the (n < 2) condition)
///   r5 = scratch holding the callee ("fib" itself, looked up fresh each call)
///   r6 = scratch holding the argument for the recursive call
///
/// A call at register `f` hands the callee a fresh window starting at
/// `f + 1`, so anything the caller still needs afterwards must live at an
/// index <= f, never above it. That's why the two results (r1, r2) are
/// placed *below* the call-setup registers (r5, r6) rather than above them.
fn build_fib(heap: &mut Heap) -> ObjFunction {
  let mut c = Chunk::new();

  let two = c.add_constant(Value::float(2.0));
  let fib_name = c.add_constant(heap.alloc_string("fib"));
  let one = c.add_constant(Value::float(1.0));

  c.emit(Instr::LoadConst {
    dst: 3,
    const_idx: two,
  }); // r3 = 2
  c.emit(Instr::Lt { dst: 4, a: 0, b: 3 }); // r4 = n < 2
  c.emit(Instr::JmpIfFalse { cond: 4, offset: 1 }); // if !(n<2) skip base case
  c.emit(Instr::Return { src: 0 }); // return n

  c.emit(Instr::GetGlobal {
    dst: 5,
    name_const: fib_name,
  }); // r5 = fib
  c.emit(Instr::LoadConst {
    dst: 6,
    const_idx: one,
  }); // r6 = 1
  c.emit(Instr::Sub { dst: 6, a: 0, b: 6 }); // r6 = n - 1 (arg register = r5+1)
  c.emit(Instr::Call {
    dst: 1,
    func: 5,
    num_args: 1,
  }); // r1 = fib(n-1)

  c.emit(Instr::GetGlobal {
    dst: 5,
    name_const: fib_name,
  }); // r5 = fib
  c.emit(Instr::LoadConst {
    dst: 6,
    const_idx: two,
  }); // r6 = 2
  c.emit(Instr::Sub { dst: 6, a: 0, b: 6 }); // r6 = n - 2
  c.emit(Instr::Call {
    dst: 2,
    func: 5,
    num_args: 1,
  }); // r2 = fib(n-2)

  c.emit(Instr::Add { dst: 3, a: 1, b: 2 }); // r3 = r1 + r2
  c.emit(Instr::Return { src: 3 });

  ObjFunction {
    name: "fib".to_string(),
    arity: 1,
    num_registers: 7,
    chunk: c,
  }
}

/// Hand-assemble the top-level script:
///
///   fib_value_wired_in_as_global;
///   print nil;
///   print true;
///   print 42;
///   print "hello " + "world";
///   print fib(10);
fn build_main(heap: &mut Heap, fib_val: Value) -> ObjFunction {
  let mut c = Chunk::new();

  let fib_const = c.add_constant(fib_val);
  let fib_name = c.add_constant(heap.alloc_string("fib"));
  let const_42 = c.add_constant(Value::float(42.0));
  let hello = c.add_constant(heap.alloc_string("hello "));
  let world = c.add_constant(heap.alloc_string("world"));
  let bytes = c.add_constant(heap.alloc_bytes("hello".as_bytes()));
  let ten = c.add_constant(Value::float(10.0));

  c.emit(Instr::LoadConst {
    dst: 0,
    const_idx: fib_const,
  });
  c.emit(Instr::SetGlobal {
    name_const: fib_name,
    src: 0,
  }); // globals["fib"] = fib

  c.emit(Instr::LoadNil { dst: 1 });
  c.emit(Instr::Print { src: 1 }); // nil

  c.emit(Instr::LoadBool { dst: 1, val: true });
  c.emit(Instr::Print { src: 1 }); // true

  c.emit(Instr::LoadConst {
    dst: 1,
    const_idx: const_42,
  });
  c.emit(Instr::Print { src: 1 }); // 42

  c.emit(Instr::LoadConst {
    dst: 1,
    const_idx: hello,
  });
  c.emit(Instr::LoadConst {
    dst: 2,
    const_idx: world,
  });
  c.emit(Instr::Concat { dst: 3, a: 1, b: 2 });
  c.emit(Instr::Print { src: 3 }); // hello world

  c.emit(Instr::GetGlobal {
    dst: 4,
    name_const: fib_name,
  });
  c.emit(Instr::LoadConst {
    dst: 5,
    const_idx: ten,
  });
  c.emit(Instr::Call {
    dst: 6,
    func: 4,
    num_args: 1,
  });
  c.emit(Instr::Print { src: 6 }); // fib(10)

  c.emit(Instr::LoadConst {
    dst: 7,
    const_idx: bytes,
  });
  c.emit(Instr::Print { src: 7 }); // (68 65 6c 6c 6f)

  c.emit(Instr::Return { src: 6 });

  ObjFunction {
    name: "main".to_string(),
    arity: 0,
    num_registers: 7,
    chunk: c,
  }
}

pub fn run_test() {
  let mut heap = Heap::new();

  let fib = build_fib(&mut heap);
  let fib_val = heap.alloc_function(fib);

  let main_fn = build_main(&mut heap, fib_val);
  let main_val = heap.alloc_function(main_fn);

  let mut vm = VM::new(heap);
  // fib needs to find itself by name at call time.
  let main_fn_ptr = match unsafe { &*main_val.as_obj() } {
    crate::vm::object::Obj::Func(f) => f as *const ObjFunction,
    _ => unreachable!(),
  };

  if let Err(e) = vm.run(main_fn_ptr) {
    eprintln!("runtime error: {}", e);
    std::process::exit(1);
  }

  // A quick, explicit look at the NaN-boxed representation itself.
  println!();
  println!("-- value sizes & tag demo --");
  println!(
    "size_of::<Value>() = {} bytes",
    std::mem::size_of::<Value>()
  );
  let i = Value::integer(3);
  let n = Value::float(3.5);
  let b = Value::bool(false);
  let nil = Value::nil();
  println!("int: is_int={} display={}", i.is_int(), i);
  println!("float: is_float={} display={}", n.is_float(), n);
  println!("bool:   is_bool={}   display={}", b.is_bool(), b);
  println!("nil:    is_nil={}    display={}", nil.is_nil(), nil);
}
