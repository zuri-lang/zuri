#![allow(unused)]

use std::{ops::Deref, rc::Rc};

use crate::{
  compiler::{
    ast::{Decl, Expr, Stmt},
    token::{Token, TokenKind},
  },
  vm::{
    chunk::{Chunk, Instr},
    object::{Heap, ObjFunction},
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

pub struct Compiler<'a> {
  declarations: Vec<Decl>,
  chunk: Chunk,
  heap: &'a mut Heap,

  /// Index of the next free register in the current function's window.
  /// Every expression that produces a value claims one via `alloc_reg`.
  next_reg: u8,
  max_reg: u8,

  // Handle locals
  scope_depth: usize,
  locals: Vec<Local>,
}

impl<'a> Compiler<'a> {
  pub fn new(declarations: Vec<Decl>, heap: &'a mut Heap) -> Self {
    let mut chunk = Chunk::new();

    Compiler {
      declarations,
      chunk,
      heap,
      next_reg: 0,
      max_reg: 0,
      scope_depth: 0,
      locals: Vec::new(),
    }
  }

  /// Claim the next free register for an expression result. There's no
  /// corresponding `free_reg` yet -- see the missing-pieces notes -- so
  /// registers are currently only ever handed out, never reclaimed.
  fn alloc_reg(&mut self) -> u8 {
    let r = self.next_reg;
    self.next_reg = self
      .next_reg
      .checked_add(1)
      .expect("Compiler ran out of registers (>255 live values in one function)");

    self.max_reg = self.max_reg.max(self.next_reg);

    r
  }

  /// Emit a JmpIfFalse with a placeholder offset, returning the index of
  /// that instruction so the offset can be backfilled once the jump target
  /// is known.
  fn emit_jump_if_false(&mut self, cond: u8) -> usize {
    self.chunk.emit(Instr::JmpIfFalse { cond, offset: 0 })
  }

  fn emit_jump(&mut self) -> usize {
    self.chunk.emit(Instr::Jmp { offset: 0 })
  }

  /// Backfill a placeholder from emit_jump/emit_jump_if_false so it lands on
  /// "whatever gets emitted next" -- i.e. right here, right now.
  fn patch_jump(&mut self, jump_at: usize) {
    let target = self.chunk.code.len();
    let offset: i16 = (target as isize - (jump_at as isize + 1))
      .try_into()
      .expect("jump target too far away: offset does not fit in i16");
    match &mut self.chunk.code[jump_at] {
      Instr::Jmp { offset: o } => *o = offset,
      Instr::JmpIfFalse { offset: o, .. } => *o = offset,
      other => panic!(
        "patch_jump: instruction at {} is not a jump, got {:?}",
        jump_at, other
      ),
    }
  }

  /// Emit an unconditional Jmp back to `loop_start` (an index captured
  /// earlier). The target's already known, so no patching needed.
  fn emit_loop(&mut self, loop_start: usize) {
    let jump_at = self.chunk.code.len();
    let offset: i16 = (loop_start as isize - (jump_at as isize + 1))
      .try_into()
      .expect("loop body too large: offset does not fit in i16");
    self.chunk.emit(Instr::Jmp { offset });
  }

  /// Give back every register allocated since `mark` was taken. Call this
  /// once the values in those registers are no longer needed -- e.g. after
  /// a binary op has consumed its operands, or at the end of a block whose
  /// locals just went out of scope.
  fn free_regs_to(&mut self, mark: u8) {
    debug_assert!(
      mark <= self.next_reg,
      "mark must be a previously observed next_reg value"
    );
    self.next_reg = mark;
  }

  fn compile_function(&mut self, token: &Token, params: &[Expr], body: &Stmt, is_variadic: bool) {
    let name = token_to_string(token.clone());

    let mut param_names = Vec::new();

    for param in params {
      if let Expr::Argument(arg, _) = param {
        param_names.push(token_to_string(arg.clone()));
      } else {
        panic!("function parameter must be an identifier");
      }
    }

    // Swap in a totally fresh compile context.
    let saved_chunk = std::mem::replace(&mut self.chunk, Chunk::new());
    let saved_next_reg = std::mem::replace(&mut self.next_reg, 0);
    let saved_max_reg = std::mem::replace(&mut self.max_reg, 0);
    let saved_locals = std::mem::replace(&mut self.locals, Vec::new());
    let saved_scope_depth = std::mem::replace(&mut self.scope_depth, 0);

    // Parameters become locals in registers 0..params.len() -- exactly
    // matching Instr::Call's convention (args already sit contiguously
    // starting at the callee's register 0).
    for pname in &param_names {
      let reg = self.alloc_reg();
      self.locals.push(Local {
        name: pname.clone(),
        reg,
        is_const: false,
        depth: 0,
      });
    }

    self.compile_statement(body);

    // Implicit `return nil` if the body didn't end with one.
    if !matches!(self.chunk.code.last(), Some(Instr::Return { .. })) {
      let nil_reg = self.alloc_reg();
      self.chunk.emit(Instr::LoadNil { dst: nil_reg });
      self.chunk.emit(Instr::Return { src: nil_reg });
    }

    let arity = param_names.len() as u8;
    let num_registers = self.max_reg;
    let finished_chunk = std::mem::replace(&mut self.chunk, saved_chunk);
    self.next_reg = saved_next_reg;
    self.max_reg = saved_max_reg;
    self.locals = saved_locals;
    self.scope_depth = saved_scope_depth;

    let obj_fn = ObjFunction {
      name: name.clone(),
      arity,
      variadic: is_variadic,
      num_registers,
      chunk: finished_chunk,
    };

    let fn_val = self.heap.alloc_function(obj_fn);

    let mark = self.next_reg;
    let dst = self.alloc_reg();
    let const_idx = self.chunk.add_constant(fn_val);
    self.chunk.emit(Instr::LoadConst { dst, const_idx });
    let name_const = self.chunk.add_constant(self.heap.alloc_string(name));
    self.chunk.emit(Instr::SetGlobal {
      name_const,
      src: dst,
    });
    self.free_regs_to(mark);
  }

  fn compile_expression(&mut self, expression: &Expr) -> u8 {
    match expression {
      Expr::Nil => {
        let dst = self.alloc_reg();
        self.chunk.emit(Instr::LoadNil { dst });
        dst
      },
      Expr::Bool(value) => {
        let dst = self.alloc_reg();
        self.chunk.emit(Instr::LoadBool { dst, val: *value });
        dst
      },
      Expr::Integer(value) => {
        let dst = self.alloc_reg();
        let const_idx = self.chunk.add_constant(Value::number(*value as f64));
        self.chunk.emit(Instr::LoadConst { dst, const_idx });
        dst
      },
      Expr::Float(value) => {
        let dst = self.alloc_reg();
        let const_idx = self.chunk.add_constant(Value::number(*value as f64));
        self.chunk.emit(Instr::LoadConst { dst, const_idx });
        dst
      },
      Expr::Literal(literal) => {
        // String literal: intern it on the heap now, at compile time, and
        // reference the resulting Value from the constant pool.
        let dst = self.alloc_reg();
        let str_val = self.heap.alloc_string(literal.clone());
        let const_idx = self.chunk.add_constant(str_val);
        self.chunk.emit(Instr::LoadConst { dst, const_idx });
        dst
      },
      Expr::BigNumber(number) => {
        // String literal: intern it on the heap now, at compile time, and
        // reference the resulting Value from the constant pool.
        let dst = self.alloc_reg();
        let str_val = self.heap.alloc_bigint(number.clone());
        let const_idx = self.chunk.add_constant(str_val);
        self.chunk.emit(Instr::LoadConst { dst, const_idx });
        dst
      },
      Expr::Identifier(token) => {
        let name = match &token.kind {
          TokenKind::Identifier(s) => s.clone(),
          other => panic!(
            "compile_expression: Identifier token is not an identifier: {:?}",
            other
          ),
        };
        if let Some(reg) = self
          .locals
          .iter()
          .rev()
          .find(|l| l.name == name)
          .map(|l| l.reg)
        {
          reg // a local: zero instructions, just point at its already-live register
        } else {
          let dst = self.alloc_reg();
          let str_val = self.heap.alloc_string(name);
          let name_const = self.chunk.add_constant(str_val);
          self.chunk.emit(Instr::GetGlobal { dst, name_const });
          dst
        }
      },
      Expr::Unary(op, expr) => {
        let src = self.compile_expression(expr);

        let instr = match op {
          TokenKind::Minus => Instr::Neg { dst: src, src },
          TokenKind::Bang => Instr::Not { dst: src, src },
          TokenKind::Tilde => Instr::BitNot { dst: src, src },
          _ => panic!("compile_expression: unsupported unary operator: {:?}", op),
        };
        self.chunk.emit(instr);
        src
      },
      Expr::Binary(lhs, op, rhs) => {
        // By the invariant above, `a` is the one register lhs ends up owning.
        let a = self.compile_expression(lhs);

        // Mark right after lhs settles -- this is where rhs's own scratch
        // registers (and its own final result register `b`) will start.
        let mark = self.next_reg;
        let dst = if a >= mark { a } else { self.alloc_reg() };

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
        self.chunk.emit(instr);

        // `b` is dead now that Add/Sub/etc. has consumed it -- give it (and
        // anything rhs allocated above it) back. `a` is deliberately *not*
        // freed: it now holds the result, reused in place as the destination,
        // so the invariant ("returns R, next_reg == R+1") holds for us too.
        self.free_regs_to(mark);
        dst
      },
      Expr::Logical(lhs, op, rhs) => {
        // By the invariant above, `a` is the one register lhs ends up owning.
        let a = self.compile_expression(lhs);

        // Mark right after lhs settles -- this is where rhs's own scratch
        // registers (and its own final result register `b`) will start.
        let mark = self.next_reg;
        let dst = if a >= mark { a } else { self.alloc_reg() };

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
        self.chunk.emit(instr);

        // `b` is dead now that Add/Sub/etc. has consumed it -- give it (and
        // anything rhs allocated above it) back. `a` is deliberately *not*
        // freed: it now holds the result, reused in place as the destination,
        // so the invariant ("returns R, next_reg == R+1") holds for us too.
        self.free_regs_to(mark);
        dst
      },
      Expr::Grouping(expr) => self.compile_expression(expr),
      Expr::Condition(condition, truth, falsey) => {
        let mark = self.next_reg;
        let cond_reg = self.compile_expression(condition);
        let then_jump = self.emit_jump_if_false(cond_reg);
        self.free_regs_to(mark); // cond is dead; also frees the slot the result will land in

        // Both branches compile starting from the same `mark`, so by the
        // compile_expression invariant (a fully-collapsed expression's result
        // always lands in the very first register it allocates) they're
        // guaranteed to land in the SAME register regardless of which one
        // actually runs -- that's what makes this safe without a Move.
        let then_result = self.compile_expression(truth);
        debug_assert_eq!(
          then_result, mark,
          "ternary branches must land in the same register"
        );
        let else_jump = self.emit_jump(); // skip else after then runs
        self.free_regs_to(mark); // reclaim before the other, mutually-exclusive branch

        self.patch_jump(then_jump); // false lands here: start of else
        let else_result = self.compile_expression(falsey);
        debug_assert_eq!(
          else_result, mark,
          "ternary branches must land in the same register"
        );
        self.patch_jump(else_jump); // after else: land here

        mark // whichever branch ran, its result is sitting in `mark`
      },
      Expr::Assign(target, value) => {
        let name = match target.as_ref() {
          Expr::Identifier(token) => match &token.kind {
            TokenKind::Identifier(s) => s.clone(),

            // TODO: Return proper compiler error
            other => panic!("compile: Assign target is not an identifier: {:?}", other),
          },
          // TODO: Return proper compiler error
          other => panic!("compile: unsupported assignment target: {:?}", other),
        };

        // Copy what's needed out of `self.locals` before compiling `value`
        // (needs &mut self) -- can't hold a borrow of locals across that call.
        let resolved = self
          .locals
          .iter()
          .rev()
          .find(|l| l.name == name)
          .map(|l| (l.reg, l.is_const));

        match resolved {
          Some((_, true)) => panic!("compile: cannot assign to constant '{}'", name),
          Some((dst, false)) => {
            let value_reg = self.compile_expression(value);
            if value_reg != dst {
              self.chunk.emit(Instr::Move {
                dst,
                src: value_reg,
              });
              self.free_regs_to(value_reg);
            }
            dst
          },
          None => {
            // Not a known local -- treat it as a global, same mechanism
            // "fib" uses to find itself.
            let value_reg = self.compile_expression(value);
            let str_val = self.heap.alloc_string(name);
            let name_const = self.chunk.add_constant(str_val);
            self.chunk.emit(Instr::SetGlobal {
              name_const,
              src: value_reg,
            });
            value_reg
          },
        }
      },
      Expr::Call(callee, args) => {
        let mark = self.next_reg;
        let raw_func_reg = self.compile_expression(callee);
        let func_reg = if raw_func_reg >= mark {
          raw_func_reg
        } else {
          let fresh = self.alloc_reg();
          self.chunk.emit(Instr::Move {
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
            self.chunk.emit(Instr::Move {
              dst: expected,
              src: arg_reg,
            });
          }
          self.next_reg = expected + 1; // claim the slot either way
          num_args += 1;
        }

        let dst = func_reg; // result overwrites the callee's own register
        self.chunk.emit(Instr::Call {
          dst,
          func: func_reg,
          num_args,
        });
        self.free_regs_to(func_reg + 1);
        dst
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
        let _ = self.compile_expression(expression);
      },
      Stmt::Echo(value) => {
        let reg = self.compile_expression(value);
        self.chunk.emit(Instr::Print { src: reg });
      },
      Stmt::Block(statements) => {
        let mark = self.next_reg;
        let locals_mark = self.locals.len();
        self.scope_depth += 1;

        for stmt in statements {
          self.compile_statement(stmt);
        }

        self.scope_depth -= 1;
        self.locals.truncate(locals_mark); // names declared in this block are gone
        self.free_regs_to(mark);
      },
      Stmt::If(condition, then_branch, else_branch) => {
        let mark = self.next_reg;
        let cond_reg = self.compile_expression(condition);
        let then_jump = self.emit_jump_if_false(cond_reg);
        self.free_regs_to(mark); // cond is consumed by the jump test, dead now

        self.compile_statement(then_branch);

        match else_branch {
          Some(else_stmt) => {
            let else_jump = self.emit_jump(); // skip the else after then runs
            self.patch_jump(then_jump); // false lands here: start of else
            self.compile_statement(else_stmt);
            self.patch_jump(else_jump); // after else: land here
          },
          None => {
            self.patch_jump(then_jump); // false skips straight past then
          },
        }
      },
      Stmt::While(cond, body) => {
        let loop_start = self.chunk.code.len(); // jump-back target

        let mark = self.next_reg;
        let cond_reg = self.compile_expression(cond);
        let exit_jump = self.emit_jump_if_false(cond_reg);
        self.free_regs_to(mark);

        self.compile_statement(body);
        self.emit_loop(loop_start); // back to re-evaluate cond

        self.patch_jump(exit_jump); // false condition lands here, past the loop
      },
      Stmt::VarList(list) => {
        for item in list {
          self.compile_statement(item);
        }
      },
      Stmt::Var(token, initializer, _type_hint, is_const) => {
        let name = match &token.kind {
          TokenKind::Identifier(s) => s.clone(),
          other => panic!("compile: Var token is not an identifier: {:?}", other),
        };

        // Redeclaring the same name in the SAME scope is an error;
        // shadowing an outer scope's variable of the same name is fine.
        let redeclared = self
          .locals
          .iter()
          .rev()
          .take_while(|l| l.depth == self.scope_depth)
          .any(|l| l.name == name);
        if redeclared {
          // TODO: Report compiler error instead!
          panic!("compile: '{}' is already declared in this scope", name);
        }

        // The initializer's result lands wherever the next free register is --
        // and unlike a plain expression statement, we deliberately do NOT free
        // it afterward: that register now belongs to this local for the rest
        // of its scope.
        let reg = self.compile_expression(initializer);

        self.locals.push(Local {
          name,
          reg,
          is_const: *is_const,
          depth: self.scope_depth,
        });
      },
      Stmt::Return(value) => {
        let reg = self.compile_expression(value);
        self.chunk.emit(Instr::Return { src: reg });
      },
      _ => {},
    };
  }

  fn compile_declaration(&mut self, declaration: &Decl) {
    match declaration {
      Decl::Stmt(statement) => self.compile_statement(statement),
      Decl::Function(name, params, body, variadic) => {
        self.compile_function(name, params, body, *variadic);
      },
      _ => {},
    };
  }

  pub fn compile(&mut self) -> ObjFunction {
    for decl in self.declarations.clone().iter() {
      self.compile_declaration(decl);
    }
    return self.finalize();
  }

  /// This function is only meant for the main script
  pub fn finalize(&mut self) -> ObjFunction {
    if !matches!(self.chunk.code.last(), Some(Instr::Return { .. })) {
      let nil_reg = self.alloc_reg();
      self.chunk.emit(Instr::LoadNil { dst: nil_reg });
      self.chunk.emit(Instr::Return { src: nil_reg });
    }

    ObjFunction {
      name: "<main>".to_string(),
      arity: 0,
      num_registers: self.max_reg,
      variadic: false,
      chunk: self.chunk.clone(),
    }
  }
}
