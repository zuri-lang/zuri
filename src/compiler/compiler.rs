#![allow(unused)]

use std::{ops::Deref, rc::Rc};

use crate::{
  compiler::{
    ast::{Decl, Expr, Stmt},
    token::TokenKind,
  },
  vm::{
    chunk::{Chunk, Instr},
    object::{Heap, ObjFunction},
    value::Value,
  },
};

pub struct Compiler<'a> {
  declarations: Vec<Decl>,
  chunk: Chunk,
  heap: &'a mut Heap,

  /// Index of the next free register in the current function's window.
  /// Every expression that produces a value claims one via `alloc_reg`.
  next_reg: u8,
  max_reg: u8,
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

    if r + 1 > self.max_reg {
      self.max_reg = r + 1;
    }

    r
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
        let const_idx = self.chunk.add_constant(Value::float(*value as f64));
        self.chunk.emit(Instr::LoadConst { dst, const_idx });
        dst
      },
      Expr::Float(value) => {
        let dst = self.alloc_reg();
        let const_idx = self.chunk.add_constant(Value::float(*value as f64));
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
      Expr::Unary(op, expr) => {
        let src = self.compile_expression(expr);

        let instr = match op {
          TokenKind::Minus => Instr::Neg { dst: src, src },
          TokenKind::Bang => Instr::Not { dst: src, src },
          // TODO: Handle Tilde (bitwise NOT)
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
        let b = self.compile_expression(rhs);

        let instr = match op {
          TokenKind::Plus => Instr::Add { dst: a, a, b },
          TokenKind::Minus => Instr::Sub { dst: a, a, b },
          TokenKind::Multiply => Instr::Mul { dst: a, a, b },
          TokenKind::Divide => Instr::Div { dst: a, a, b },
          TokenKind::Pow => Instr::Pow { dst: a, a, b },
          TokenKind::Percent => Instr::Mod { dst: a, a, b },
          TokenKind::Floor => Instr::Floor { dst: a, a, b },
          _ => panic!("compile_expression: unsupported binary operator: {:?}", op),
        };
        self.chunk.emit(instr);

        // `b` is dead now that Add/Sub/etc. has consumed it -- give it (and
        // anything rhs allocated above it) back. `a` is deliberately *not*
        // freed: it now holds the result, reused in place as the destination,
        // so the invariant ("returns R, next_reg == R+1") holds for us too.
        self.free_regs_to(mark);
        a
      },
      Expr::Logical(lhs, op, rhs) => {
        // By the invariant above, `a` is the one register lhs ends up owning.
        let a = self.compile_expression(lhs);

        // Mark right after lhs settles -- this is where rhs's own scratch
        // registers (and its own final result register `b`) will start.
        let mark = self.next_reg;
        let b = self.compile_expression(rhs);

        let instr = match op {
          TokenKind::EqualEq => Instr::Eq { dst: a, a, b },
          TokenKind::BangEq => Instr::Neq { dst: a, a, b },
          TokenKind::Less => Instr::Lt { dst: a, a, b },
          TokenKind::LessEq => Instr::Le { dst: a, a, b },
          TokenKind::Greater => Instr::Gt { dst: a, a, b },
          TokenKind::GreaterEq => Instr::Ge { dst: a, a, b },
          _ => panic!("compile_expression: unsupported binary operator: {:?}", op),
        };
        self.chunk.emit(instr);

        // `b` is dead now that Add/Sub/etc. has consumed it -- give it (and
        // anything rhs allocated above it) back. `a` is deliberately *not*
        // freed: it now holds the result, reused in place as the destination,
        // so the invariant ("returns R, next_reg == R+1") holds for us too.
        self.free_regs_to(mark);
        a
      },
      Expr::Grouping(expr) => self.compile_expression(expr),
      Expr::Condition(condition, truth, falsey) => {
        let mark = self.next_reg;
        let a = self.compile_expression(condition);
        self.free_regs_to(mark);

        let false_jmp = self.chunk.emit(Instr::JmpIfFalse { cond: a, offset: 0 });

        self.compile_expression(truth);
        self.free_regs_to(mark);
        let exit_jmp = self.chunk.emit(Instr::Jmp { offset: 0 });

        self.chunk.patch_false_jump(a, false_jmp);

        self.compile_expression(falsey);
        self.free_regs_to(mark);

        self.chunk.patch_jump(exit_jmp);

        a
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
        for stmt in statements {
          self.compile_statement(stmt);
        }
        self.free_regs_to(mark);
      },
      _ => {},
    };
  }

  fn compile_declaration(&mut self, declaration: &Decl) {
    match declaration {
      Decl::Stmt(statement) => self.compile_statement(statement),
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
    // Add a return statement irrespective of the last expression or statement
    let reg = self.alloc_reg();
    self.chunk.emit(Instr::LoadNil { dst: reg });
    self.chunk.emit(Instr::Return { src: reg });

    ObjFunction {
      name: "<main>".to_string(),
      arity: 0,
      num_registers: self.max_reg,
      chunk: self.chunk.clone(),
    }
  }
}
