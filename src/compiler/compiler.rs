#![allow(unused)]

use std::{ops::Deref, rc::Rc};

use crate::{
  compiler::{
    ast::{Decl, Expr, Stmt},
    token::{Token, TokenKind},
  },
  vm::{
    chunk::{Chunk, Instr},
    object::{Heap, ObjFunction, UpvalueDescriptor},
    value::Value,
  },
};

fn token_to_string(token: Token) -> String {
  if let TokenKind::Identifier(name) = token.kind {
    name.clone()
  } else {
    token.kind.to_string()
  }
}

struct Local {
  name: String,
  reg: u8,
  is_const: bool,
  depth: usize,
}

/// Everything about compiling ONE function -- its own chunk, register
/// allocator, locals table, scope depth, and the upvalue descriptors it's
/// accumulated so far. `Compiler` holds a STACK of these (one per level of
/// function nesting currently being compiled), which is what makes upvalue
/// resolution possible: compiling a nested function's body can look back
/// into `scopes[enclosing_idx].locals` because the enclosing function's
/// state is a real, still-present stack entry, not swapped away and lost.
struct FunctionScope {
  chunk: Chunk,
  next_reg: u8,
  max_reg: u8,
  locals: Vec<Local>,
  scope_depth: usize,
  upvalues: Vec<UpvalueDescriptor>,
}

impl FunctionScope {
  fn new() -> Self {
    FunctionScope {
      chunk: Chunk::new(),
      next_reg: 0,
      max_reg: 0,
      locals: Vec::new(),
      scope_depth: 0,
      upvalues: Vec::new(),
    }
  }
}

enum VarLoc {
  Local(u8, bool),
  Upvalue(u8),
  Global,
}

pub struct Compiler<'a> {
  declarations: Vec<Decl>,
  heap: &'a mut Heap,
  scopes: Vec<FunctionScope>,
  is_repl: bool,
}

impl<'a> Compiler<'a> {
  pub fn new(declarations: Vec<Decl>, chunk: Box<Chunk>, heap: &'a mut Heap) -> Self {
    let mut top = FunctionScope::new();
    top.chunk = *chunk;
    Compiler {
      declarations,
      heap,
      scopes: vec![top],
      is_repl: false,
    }
  }

  pub fn enable_repl_mode(&mut self) {
    self.is_repl = true;
  }
  pub fn disable_repl_mode(&mut self) {
    self.is_repl = false;
  }

  fn cur(&self) -> &FunctionScope {
    self.scopes.last().unwrap()
  }
  fn cur_mut(&mut self) -> &mut FunctionScope {
    self.scopes.last_mut().unwrap()
  }

  fn emit(&mut self, instr: Instr) -> usize {
    self.cur_mut().chunk.emit(instr)
  }
  fn add_constant(&mut self, v: Value) -> u16 {
    self.cur_mut().chunk.add_constant(v)
  }

  fn alloc_reg(&mut self) -> u8 {
    let scope = self.cur_mut();
    let r = scope.next_reg;
    scope.next_reg = scope
      .next_reg
      .checked_add(1)
      .expect("compiler ran out of registers (>255 live values in one function)");
    scope.max_reg = scope.max_reg.max(scope.next_reg);
    r
  }

  fn free_regs_to(&mut self, mark: u8) {
    let scope = self.cur_mut();
    scope.next_reg = mark;
    if mark > scope.max_reg {
      scope.max_reg = mark;
    }
  }

  fn emit_jump_if_false(&mut self, cond: u8) -> usize {
    self.emit(Instr::JmpIfFalse { cond, offset: 0 })
  }

  fn emit_jump_if_true(&mut self, cond: u8) -> usize {
    self.emit(Instr::JmpIfTrue { cond, offset: 0 })
  }

  fn emit_jump(&mut self) -> usize {
    self.emit(Instr::Jmp { offset: 0 })
  }

  fn patch_jump(&mut self, jump_at: usize) {
    let target = self.cur().chunk.code.len();
    let raw_offset = target as isize - (jump_at as isize + 1);

    let offset: i16 = raw_offset
      .try_into()
      .expect("jump target too far away: offset does not fit in i16");

    match &mut self.cur_mut().chunk.code[jump_at] {
      Instr::Jmp { offset: o } => *o = offset,
      Instr::JmpIfFalse { offset: o, .. } => *o = offset,
      Instr::JmpIfTrue { offset: o, .. } => *o = offset, // NEW
      other => panic!(
        "patch_jump: instruction at {} is not a jump, got {:?}",
        jump_at, other
      ),
    }
  }

  fn emit_loop(&mut self, loop_start: usize) {
    let jump_at = self.cur().chunk.code.len();
    let raw_offset = loop_start as isize - (jump_at as isize + 1);
    let offset: i16 = raw_offset
      .try_into()
      .expect("loop body too large: offset does not fit in i16");
    self.emit(Instr::Jmp { offset });
  }

  fn identifier_name(token: &Token) -> String {
    match &token.kind {
      TokenKind::Identifier(s) => s.clone(),
      other => panic!("compile: expected an identifier token, got {:?}", other),
    }
  }

  /// Resolve a name against: the current function's locals, then an
  /// upvalue chain reaching into enclosing functions, then finally a
  /// global.
  fn resolve_variable(&mut self, name: &str) -> VarLoc {
    if let Some(l) = self.cur().locals.iter().rev().find(|l| l.name == name) {
      return VarLoc::Local(l.reg, l.is_const);
    }
    let top = self.scopes.len() - 1;
    if let Some(idx) = self.resolve_upvalue(top, name) {
      return VarLoc::Upvalue(idx);
    }
    VarLoc::Global
  }

  /// Does `scopes[scope_idx]` have access to `name` as an upvalue? Checks
  /// whether the DIRECTLY enclosing function (`scope_idx - 1`) has it as a
  /// local (capture it directly), and if not, recurses outward in case
  /// some function further out has it -- in which case each intermediate
  /// function threads it through as `UpvalueDescriptor::Upvalue`, chaining
  /// the capture inward one level at a time.
  fn resolve_upvalue(&mut self, scope_idx: usize, name: &str) -> Option<u8> {
    if scope_idx == 0 {
      return None;
    }
    let enclosing_idx = scope_idx - 1;

    if let Some(reg) = self.scopes[enclosing_idx]
      .locals
      .iter()
      .rev()
      .find(|l| l.name == name)
      .map(|l| l.reg)
    {
      return Some(self.add_upvalue(scope_idx, UpvalueDescriptor::Local(reg)));
    }
    if let Some(up_idx) = self.resolve_upvalue(enclosing_idx, name) {
      return Some(self.add_upvalue(scope_idx, UpvalueDescriptor::Upvalue(up_idx)));
    }
    None
  }

  fn add_upvalue(&mut self, scope_idx: usize, desc: UpvalueDescriptor) -> u8 {
    if let Some(pos) = self.scopes[scope_idx]
      .upvalues
      .iter()
      .position(|d| *d == desc)
    {
      return pos as u8;
    }
    self.scopes[scope_idx].upvalues.push(desc);
    (self.scopes[scope_idx].upvalues.len() - 1) as u8
  }

  /// Compile a function's PARAMETERS and BODY into a standalone
  /// `ObjFunction` prototype. Does NOT bind the resulting function
  /// anywhere -- callers (`compile_function_decl`, `Expr::Anonymous`)
  /// decide that.
  fn compile_function_prototype(
    &mut self,
    token: &Token,
    params: &[Expr],
    body: &Stmt,
    is_variadic: bool,
  ) -> ObjFunction {
    let name = Self::identifier_name(token);

    let mut param_names = Vec::with_capacity(params.len());
    for param in params {
      match param {
        Expr::Argument(ptoken, _type_hint) => param_names.push(Self::identifier_name(ptoken)),
        other => panic!(
          "compile: function parameter is not Expr::Argument: {:?}",
          other
        ),
      }
    }
    if is_variadic {
      assert!(
        !param_names.is_empty(),
        "compile: variadic function must have a named last parameter"
      );
    }

    self.scopes.push(FunctionScope::new());

    for pname in &param_names {
      let reg = self.alloc_reg();
      self.cur_mut().locals.push(Local {
        name: pname.clone(),
        reg,
        is_const: false,
        depth: 0,
      });
    }

    self.compile_statement(body);

    if !matches!(self.cur().chunk.code.last(), Some(Instr::Return { .. })) {
      let nil_reg = self.alloc_reg();
      self.emit(Instr::LoadNil { dst: nil_reg });
      self.emit(Instr::Return { src: nil_reg });
    }

    let finished = self.scopes.pop().unwrap();
    ObjFunction {
      name,
      arity: param_names.len() as u8,
      variadic: is_variadic,
      num_registers: finished.max_reg,
      chunk: finished.chunk,
      upvalues: finished.upvalues,
    }
  }

  /// `function foo(...) { ... }` as a declaration: compile the prototype,
  /// materialize it as a closure at THIS point in the enclosing code
  /// (crucial for recursion), and bind the result as a global.
  fn compile_function_decl(
    &mut self,
    token: &Token,
    params: &[Expr],
    body: &Stmt,
    is_variadic: bool,
  ) {
    let name = Self::identifier_name(token);
    let obj_fn = self.compile_function_prototype(token, params, body, is_variadic);
    let proto_val = self.heap.alloc_function(obj_fn);
    let const_idx = self.add_constant(proto_val);

    let mark = self.cur().next_reg;
    let dst = self.alloc_reg();
    self.emit(Instr::Closure {
      dst,
      proto_const: const_idx,
    });
    let name_val = self.heap.alloc_string(name);
    let name_const = self.add_constant(name_val);
    self.emit(Instr::SetGlobal {
      name_const,
      src: dst,
    });
    self.free_regs_to(mark);
  }

  fn compile_expression(&mut self, expression: &Expr) -> u8 {
    match expression {
      Expr::Nil => {
        let dst = self.alloc_reg();
        self.emit(Instr::LoadNil { dst });
        dst
      },
      Expr::Bool(value) => {
        let dst = self.alloc_reg();
        self.emit(Instr::LoadBool { dst, val: *value });
        dst
      },
      Expr::Integer(value) => {
        let dst = self.alloc_reg();
        let const_idx = self.add_constant(Value::number(*value as f64));
        self.emit(Instr::LoadConst { dst, const_idx });
        dst
      },
      Expr::Float(value) => {
        let dst = self.alloc_reg();
        let const_idx = self.add_constant(Value::number(*value as f64));
        self.emit(Instr::LoadConst { dst, const_idx });
        dst
      },
      Expr::Literal(literal) => {
        // String literal: intern it on the heap now, at compile time, and
        // reference the resulting Value from the constant pool.
        let dst = self.alloc_reg();
        let str_val = self.heap.alloc_string(literal.clone());
        let const_idx = self.add_constant(str_val);
        self.emit(Instr::LoadConst { dst, const_idx });
        dst
      },
      Expr::BigNumber(number) => {
        // String literal: intern it on the heap now, at compile time, and
        // reference the resulting Value from the constant pool.
        let dst = self.alloc_reg();
        let str_val = self.heap.alloc_bigint(number.clone());
        let const_idx = self.add_constant(str_val);
        self.emit(Instr::LoadConst { dst, const_idx });
        dst
      },
      Expr::Identifier(token) => {
        let name = Self::identifier_name(token);
        match self.resolve_variable(&name) {
          VarLoc::Local(reg, _) => reg,
          VarLoc::Upvalue(idx) => {
            let dst = self.alloc_reg();
            self.emit(Instr::GetUpval { dst, idx });
            dst
          },
          VarLoc::Global => {
            let dst = self.alloc_reg();
            let str_val = self.heap.alloc_string(name);
            let name_const = self.add_constant(str_val);
            self.emit(Instr::GetGlobal { dst, name_const });
            dst
          },
        }
      },
      Expr::Unary(op, expr) => {
        let mark = self.cur().next_reg;
        let src = self.compile_expression(expr);
        let dst = if src >= mark { src } else { self.alloc_reg() };

        let instr = match op {
          TokenKind::Minus => Instr::Neg { dst, src },
          TokenKind::Bang => Instr::Not { dst, src },
          TokenKind::Tilde => Instr::BitNot { dst, src },
          _ => panic!("compile_expression: unsupported unary operator: {:?}", op),
        };
        self.emit(instr);
        dst
      },
      Expr::Binary(lhs, op, rhs) => {
        let mark = self.cur().next_reg;
        let a = self.compile_expression(lhs);
        let dst = if a >= mark { a } else { self.alloc_reg() };

        let rhs_mark = self.cur().next_reg;
        let b = self.compile_expression(rhs);

        let instr = match op {
          TokenKind::Plus => Instr::Add { dst, a, b },
          TokenKind::Minus => Instr::Sub { dst, a, b },
          TokenKind::Multiply => Instr::Mul { dst, a, b },
          TokenKind::Divide => Instr::Div { dst, a, b },
          TokenKind::Pow => Instr::Pow { dst, a, b },
          TokenKind::Percent => Instr::Mod { dst, a, b },
          TokenKind::Floor => Instr::Floor { dst, a, b },
          TokenKind::Amp => Instr::BitAnd { dst, a, b },
          TokenKind::Bar => Instr::BitOr { dst, a, b },
          TokenKind::Xor => Instr::BitXor { dst, a, b },
          TokenKind::Lshift => Instr::BitShl { dst, a, b },
          TokenKind::Rshift => Instr::BitShr { dst, a, b },
          TokenKind::Urshift => Instr::BitUshr { dst, a, b },
          _ => panic!("compile_expression: unsupported binary operator: {:?}", op),
        };
        self.emit(instr);

        self.free_regs_to(rhs_mark.max(dst + 1));
        dst
      },
      Expr::Logical(lhs, op, rhs) => {
        let mark = self.cur().next_reg;
        let a = self.compile_expression(lhs);
        let dst = if a >= mark { a } else { self.alloc_reg() };

        let rhs_mark = self.cur().next_reg;
        let b = self.compile_expression(rhs);

        let instr = match op {
          TokenKind::EqualEq => Instr::Eq { dst, a, b },
          TokenKind::BangEq => Instr::Neq { dst, a, b },
          TokenKind::Less => Instr::Lt { dst, a, b },
          TokenKind::LessEq => Instr::Le { dst, a, b },
          TokenKind::Greater => Instr::Gt { dst, a, b },
          TokenKind::GreaterEq => Instr::Ge { dst, a, b },
          _ => panic!("compile_expression: unsupported binary operator: {:?}", op),
        };
        self.emit(instr);

        self.free_regs_to(rhs_mark.max(dst + 1));
        dst
      },
      Expr::Circuit(lhs, op, rhs) => {
        let mark = self.cur().next_reg;
        let result = mark;

        // Force lhs into the fixed result register -- same trick as
        // Condition: mutually-exclusive control-flow paths can share a
        // register number, but a bare local-variable read for lhs won't
        // land there "for free", so Move if needed.
        let lhs_val = self.compile_expression(lhs);
        if lhs_val != result {
          self.emit(Instr::Move {
            dst: result,
            src: lhs_val,
          });
        }
        self.free_regs_to(result + 1); // keep just the result register reserved

        // `and`: short-circuit (skip rhs, keep lhs) when lhs is already falsy.
        // `or`:  short-circuit (skip rhs, keep lhs) when lhs is already truthy.
        let short_circuit_jump = match op {
          TokenKind::And => self.emit_jump_if_false(result),
          TokenKind::Or => self.emit_jump_if_true(result),
          _ => panic!("compile_expression: unsupported Circuit operator: {:?}", op),
        };

        // Didn't short-circuit: the result becomes rhs's value instead.
        let rhs_val = self.compile_expression(rhs);
        if rhs_val != result {
          self.emit(Instr::Move {
            dst: result,
            src: rhs_val,
          });
        }
        self.free_regs_to(result + 1);

        self.patch_jump(short_circuit_jump);
        result
      },
      Expr::Grouping(expr) => self.compile_expression(expr),
      Expr::Condition(condition, truth, falsey) => {
        let mark = self.cur().next_reg;
        let cond_reg = self.compile_expression(condition);
        let then_jump = self.emit_jump_if_false(cond_reg);
        self.free_regs_to(mark);

        let result = mark;
        let then_val = self.compile_expression(truth);
        if then_val != result {
          self.emit(Instr::Move {
            dst: result,
            src: then_val,
          });
        }
        let else_jump = self.emit_jump();
        self.free_regs_to(mark);

        self.patch_jump(then_jump);
        let else_val = self.compile_expression(falsey);
        if else_val != result {
          self.emit(Instr::Move {
            dst: result,
            src: else_val,
          });
        }
        self.patch_jump(else_jump);
        self.free_regs_to(mark + 1);

        result
      },
      Expr::Assign(target, value) => {
        let name = match target.as_ref() {
          Expr::Identifier(token) => Self::identifier_name(token),
          other => panic!(
            "compile_expression: unsupported assignment target: {:?}",
            other
          ),
        };

        match self.resolve_variable(&name) {
          VarLoc::Local(_, true) => panic!("compile: cannot assign to constant '{}'", name),
          VarLoc::Local(dst, false) => {
            let value_reg = self.compile_expression(value);
            if value_reg != dst {
              self.emit(Instr::Move {
                dst,
                src: value_reg,
              });
              self.free_regs_to(value_reg);
            }
            dst
          },
          VarLoc::Upvalue(idx) => {
            let value_reg = self.compile_expression(value);
            self.emit(Instr::SetUpval {
              idx,
              src: value_reg,
            });
            value_reg
          },
          VarLoc::Global => {
            let value_reg = self.compile_expression(value);
            let str_val = self.heap.alloc_string(name);
            let name_const = self.add_constant(str_val);
            self.emit(Instr::SetGlobal {
              name_const,
              src: value_reg,
            });
            value_reg
          },
        }
      },
      Expr::Call(callee, args) => {
        let mark = self.cur().next_reg;

        let raw_func_reg = self.compile_expression(callee);
        let func_reg = if raw_func_reg >= mark {
          raw_func_reg
        } else {
          let fresh = self.alloc_reg();
          self.emit(Instr::Move {
            dst: fresh,
            src: raw_func_reg,
          });
          fresh
        };

        let mut num_args: u8 = 0;
        for arg in args {
          let expected = func_reg + 1 + num_args;
          let arg_reg = self.compile_expression(arg);
          if arg_reg != expected {
            self.emit(Instr::Move {
              dst: expected,
              src: arg_reg,
            });
          }
          self.free_regs_to(expected + 1);
          num_args = num_args
            .checked_add(1)
            .expect("too many arguments in a single call");
        }

        let dst = func_reg;
        self.emit(Instr::Call {
          dst,
          func: func_reg,
          num_args,
        });
        self.free_regs_to(func_reg + 1);
        dst
      },
      Expr::Anonymous(decl) => match decl.as_ref() {
        Decl::Function(token, params, body, is_variadic) => {
          let obj_fn = self.compile_function_prototype(token, params, body, *is_variadic);
          let proto_val = self.heap.alloc_function(obj_fn);
          let const_idx = self.add_constant(proto_val);
          let dst = self.alloc_reg();
          self.emit(Instr::Closure {
            dst,
            proto_const: const_idx,
          });
          dst
        },
        other => panic!(
          "compile_expression: unsupported Anonymous declaration: {:?}",
          other
        ),
      },
      _ => {
        panic!(
          "compile_expression: unsupported expression: {:?}",
          expression
        );
      },
    }
  }

  fn compile_statement(&mut self, statement: &Stmt) {
    match statement {
      Stmt::Expression(expression) => {
        let mark = self.cur().next_reg;
        self.compile_expression(expression);
        self.free_regs_to(mark);
      },
      Stmt::Echo(value) => {
        let reg = self.compile_expression(value);
        self.emit(Instr::Print { src: reg });
      },
      Stmt::Block(statements) => {
        let mark = self.cur().next_reg;
        let locals_mark = self.cur().locals.len();
        self.cur_mut().scope_depth += 1;

        for stmt in statements {
          self.compile_statement(stmt);
        }

        self.cur_mut().scope_depth -= 1;
        let declared_locals = self.cur().locals.len() > locals_mark;
        self.cur_mut().locals.truncate(locals_mark);
        if declared_locals {
          self.emit(Instr::CloseUpvalues { from: mark });
        }
        self.free_regs_to(mark);
      },
      Stmt::If(condition, then_branch, else_branch) => {
        let mark = self.cur().next_reg;
        let cond_reg = self.compile_expression(condition);
        let then_jump = self.emit_jump_if_false(cond_reg);
        self.free_regs_to(mark);

        self.compile_statement(then_branch);

        match else_branch {
          Some(else_stmt) => {
            let else_jump = self.emit_jump();
            self.patch_jump(then_jump);
            self.compile_statement(else_stmt);
            self.patch_jump(else_jump);
          },
          None => {
            self.patch_jump(then_jump);
          },
        }
      },
      Stmt::While(cond, body) => {
        let loop_start = self.cur().chunk.code.len();

        let mark = self.cur().next_reg;
        let cond_reg = self.compile_expression(cond);
        let exit_jump = self.emit_jump_if_false(cond_reg);
        self.free_regs_to(mark);

        self.compile_statement(body);
        self.emit_loop(loop_start);

        self.patch_jump(exit_jump);
      },
      Stmt::Decl(decl) => self.compile_declaration(decl),
      Stmt::VarList(list) => {
        for item in list {
          self.compile_statement(item);
        }
      },
      Stmt::Var(token, initializer, _type_hint, is_const) => {
        let name = Self::identifier_name(token);

        if self.is_repl && self.cur().scope_depth == 0 {
          let src = self.compile_expression(initializer);

          let name_val = self.heap.alloc_string(name.clone());
          let name_const = self.add_constant(name_val);
          self.emit(Instr::SetGlobal { name_const, src });
        } else {
          let redeclared = self
            .cur()
            .locals
            .iter()
            .rev()
            .take_while(|l| l.depth == self.cur().scope_depth)
            .any(|l| l.name == name);
          if redeclared {
            panic!("compile: '{}' is already declared in this scope", name);
          }

          let mark = self.cur().next_reg;
          let raw_reg = self.compile_expression(initializer);
          // If the initializer resolved to a PRE-EXISTING register (e.g. a
          // bare read of another local, like `var t = n`), adopting that
          // register directly as this new local's home would make the two
          // names ALIAS the same physical storage -- any future write to
          // either one would silently corrupt the other. Only a register
          // freshly allocated for this expression (>= mark) is safe to
          // adopt as-is; anything else needs its own copy.
          let reg = if raw_reg >= mark {
            raw_reg
          } else {
            let fresh = self.alloc_reg();
            self.emit(Instr::Move {
              dst: fresh,
              src: raw_reg,
            });
            fresh
          };
          let depth = self.cur().scope_depth;
          self.cur_mut().locals.push(Local {
            name,
            reg,
            is_const: *is_const,
            depth,
          });
        }
      },
      Stmt::Return(value) => {
        let reg = self.compile_expression(value);
        self.emit(Instr::Return { src: reg });
      },
      _ => {},
    };
  }

  fn compile_declaration(&mut self, declaration: &Decl) {
    match declaration {
      Decl::Stmt(statement) => self.compile_statement(statement),
      Decl::Function(token, params, body, is_variadic) => {
        self.compile_function_decl(token, params, body, *is_variadic)
      },
      _ => {},
    };
  }

  pub fn compile(mut self) -> ObjFunction {
    for decl in self.declarations.clone().iter() {
      self.compile_declaration(decl);
    }
    self.finalize()
  }

  /// Finish compilation and hand back the assembled top-level function,
  /// ready for `Heap::alloc_plain_closure` + `VM::run`.
  pub fn finalize(mut self) -> ObjFunction {
    if !matches!(self.cur().chunk.code.last(), Some(Instr::Return { .. })) {
      let nil_reg = self.alloc_reg();
      self.emit(Instr::LoadNil { dst: nil_reg });
      self.emit(Instr::Return { src: nil_reg });
    }
    let top = self.scopes.into_iter().next().unwrap();
    let main_fn = ObjFunction {
      name: "main".to_string(),
      arity: 0,
      variadic: false,
      num_registers: top.max_reg,
      chunk: top.chunk,
      upvalues: Vec::new(),
    };

    if env!("ZURI_DEBUG_OPCODES") == "true" {
      // Dump sin's own compiled bytecode directly from main's constant pool.
      for c in &main_fn.chunk.constants {
        if c.is_func() {
          let proto = c.as_func();
          println!(
            "=== function '{}' ({} registers) ===",
            proto.name, proto.num_registers
          );
          for (i, instr) in proto.chunk.code.iter().enumerate() {
            println!("{:3}: {:?}", i, instr);
          }
        }
      }
    }

    main_fn
  }
}
