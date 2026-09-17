#![allow(unused)]

use std::{ops::Deref, rc::Rc};

use crate::{
  compiler::{
    ast::{Decl, Expr, Stmt, Type},
    parser::ParserError,
    token::{Token, TokenKind},
  },
  vm::{
    chunk::{Chunk, Instr, JumpKey, ParamType, ParamTypeCheck},
    object::{Heap, JitInfo, ObjFunction, UpvalueDescriptor},
    value::Value,
  },
};

struct Local {
  name: String,
  reg: u8,
  is_const: bool,
  depth: usize,
  /// Set by `resolve_upvalue` the moment some nested closure is found to
  /// capture this local. Since compilation is single-pass and a closure
  /// literal can only appear (and so only be compiled, and so only ever
  /// call `resolve_upvalue`) lexically inside the block that declares
  /// this local, every capture of it is guaranteed to have already been
  /// recorded here by the time that block finishes compiling and has to
  /// decide whether it needs to emit `Instr::CloseUpvalues` at all.
  captured: bool,
}

/// Everything about compiling a function; its own chunk, register
/// allocator, locals table, scope depth, and the upvalue descriptors it's
/// accumulated so far. `Compiler` holds a STACK of these (one per level of
/// function nesting currently being compiled), which is what makes upvalue
/// resolution possible: compiling a nested function's body can look back
/// into `scopes[enclosing_idx].locals` because the enclosing function's
/// state is a real, still-present stack entry, not swapped away and lost.
struct FunctionScope {
  chunk: Chunk,
  /// High-water mark of `next_reg`. Unlike `next_reg`, this never goes
  /// back down when `free_regs_to` runs; it's what becomes
  /// `ObjFunction.num_registers` once this function finishes compiling.
  next_reg: u8,
  max_reg: u8,
  locals: Vec<Local>,
  scope_depth: usize,
  /// Static, compile-time list of what THIS function needs to capture
  /// from its enclosing function, built up as `resolve_upvalue` discovers
  /// references to outer-scope names. Order matches `Instr::GetUpval`'s
  /// `idx` and becomes `ObjFunction.upvalues`.
  upvalues: Vec<UpvalueDescriptor>,
  /// Stack of currently-open loops, innermost last; what `break` and
  /// `continue` target. Doesn't cross function boundaries: a closure
  /// declared inside a loop starts with an empty stack, so `break` inside
  /// it (if it were otherwise valid) can't reach the enclosing loop.
  loops: Vec<LoopContext>,
  /// Every `def` name declared so far in this function, with the scope
  /// depth it was declared at. A second `def` of the same name in the
  /// same scope is an error, mirroring both the duplicate-method check
  /// and `var`'s own "already declared in this scope"; a `def` binds a
  /// module-level name, so two of them silently resolving to whichever
  /// ran last is never what the author meant.
  declared_functions: Vec<(String, usize)>,
  /// Source line of whatever statement is CURRENTLY being compiled --
  /// stamped onto every instruction `Compiler::emit` produces until
  /// it's next updated (see `Compiler::compile`'s top-level loop and
  /// `Stmt::Block`'s own handling).
  current_line: u32,
  /// Set the first time `alloc_reg` hits the 255-register ceiling for
  /// this function, so a pathological expression reports the overflow
  /// exactly once instead of once per further allocation attempt.
  register_overflow_reported: bool,
}

impl FunctionScope {
  fn new() -> Self {
    FunctionScope {
      chunk: Chunk::new(),
      locals: Vec::new(),
      upvalues: Vec::new(),
      loops: Vec::new(),
      declared_functions: Vec::new(),
      scope_depth: 0,
      next_reg: 0,
      max_reg: 0,
      current_line: 0,
      register_overflow_reported: false,
    }
  }
}

/// One active loop's jump targets, live only while compiling that loop's
/// body.
struct LoopContext {
  /// Where `continue` jumps back to; the condition re-check.
  continue_target: usize,
  /// Where `break` jumps forward to; patched once the loop's exit point
  /// is known, after the whole body has compiled.
  break_jumps: Vec<usize>,
  /// Register mark at the loop body's own entry. `break`/`continue` emit
  /// CloseUpvalues back to this point before jumping, since they skip
  /// whatever nested blocks' own natural close-on-exit would have done.
  body_mark: u8,
  /// The bytecode index of the continue instruction
  continue_locales: Vec<usize>,
}

enum VarLoc {
  Local(u8, bool),
  Upvalue(u8),
  Global,
}

pub struct Compiler<'a> {
  decls: Vec<Decl>,
  heap: &'a mut Heap,
  scopes: Vec<FunctionScope>,
  is_repl: bool,
  source_path: Rc<str>,
  /// The module every `ObjFunction` this Compiler produces belongs to
  ///; `None` for the main script/REPL (functions get
  /// `globals_module: None`, i.e. the VM's root table, exactly as
  /// before this feature existed). Set once via
  /// `set_current_module`, right after `Compiler::new`, when
  /// compiling an imported module's own source instead.
  module: Option<Value>,
  pub errors: Vec<ParserError>,
  log_instr: bool,
}

impl<'a> Compiler<'a> {
  pub fn new(
    decls: Vec<Decl>,
    chunk: Box<Chunk>,
    heap: &'a mut Heap,
    source_path: Rc<str>,
  ) -> Self {
    let mut top = FunctionScope::new();
    top.chunk = *chunk;

    // The first constant is always the class constructor name, which is "@new"
    top.chunk.add_constant(heap.alloc_string_old("@new"));

    Compiler {
      decls,
      heap,
      source_path,
      scopes: vec![top],
      is_repl: false,
      module: None,
      log_instr: std::env::var_os("ZURI_INSTR_LOG").is_some(),
      errors: Vec::new(),
    }
  }

  pub fn enable_repl_mode(&mut self) {
    self.is_repl = true;
  }

  pub fn disable_repl_mode(&mut self) {
    self.is_repl = false;
  }

  fn at_module_top_level(&self) -> bool {
    self.scopes.len() == 1 && self.cur().scope_depth == 0
  }

  pub fn set_current_module(&mut self, module: Value) {
    self.module = Some(module);
  }

  fn cur(&self) -> &FunctionScope {
    self.scopes.last().unwrap()
  }

  fn cur_mut(&mut self) -> &mut FunctionScope {
    self.scopes.last_mut().unwrap()
  }

  fn emit(&mut self, instr: Instr) -> usize {
    let line = self.cur().current_line;
    let scope = self.cur_mut();
    let idx = scope.chunk.emit(instr);
    scope.chunk.lines.push(line);
    idx
  }

  fn add_constant(&mut self, v: Value) -> u16 {
    self.cur_mut().chunk.add_constant(v)
  }

  fn add_param_check(&mut self, check: ParamTypeCheck) -> u16 {
    self.cur_mut().chunk.add_param_check(check)
  }

  /// `ast::Type` (what the parser produced) -> `chunk::ParamType` (what
  /// `Instr::CheckParamType` actually reads at runtime); a plain
  /// rename for every built-in, except `Instance`, whose class name
  /// needs interning as a string constant the check can look up by
  /// index. Never called for `Type::Any`: see
  /// `emit_param_type_checks`'s own docs on why that one skips
  /// emission entirely rather than becoming a `ParamType` variant.
  fn ast_type_to_param_type(&mut self, t: &Type) -> ParamType {
    match t {
      Type::Any => unreachable!("Type::Any is filtered out before this is ever called"),
      Type::Bool => ParamType::Bool,
      Type::Int => ParamType::Int,
      Type::Number => ParamType::Number,
      Type::BigInt => ParamType::BigInt,
      Type::String => ParamType::String,
      Type::Bytes => ParamType::Bytes,
      Type::List => ParamType::List,
      Type::Dict => ParamType::Dict,
      Type::Range => ParamType::Range,
      Type::File => ParamType::File,
      Type::Function => ParamType::Function,
      Type::Type => ParamType::Class,
      Type::Callable => ParamType::Callable,
      Type::Iterable => ParamType::Iterable,
      Type::Instance(token) => {
        let name = Self::identifier_name(token);
        let name_val = self.heap.alloc_string_old(name);
        let name_const = self.add_constant(name_val);
        ParamType::Instance(name_const)
      },
    }
  }

  /// Emits one `Instr::CheckParamType` per typed parameter, in
  /// parameter order, right after their registers are allocated but
  /// before the body compiles; so a mismatched argument raises before
  /// the function does anything with it, exactly like the reference C
  /// runtime's own `compile_type_check` placement. `params`/`regs` are
  /// parallel (every params[i] a plain `Expr::Argument`, its register
  /// already bound to a `Local` by the caller); `variadic_tail_idx`
  /// excludes the synthetic `TypeHint([List], false)` the parser always
  /// attaches to a `...args` collector (see `Parser::function_args`) --
  /// that's an implementation artifact, not something the source
  /// actually declared, so it must never be enforced.
  ///
  /// A parameter left untyped, or explicitly typed `any` (with or
  /// without `?`, which would be redundant anyway), gets no instruction
  /// at all: `any` accepts everything including `nil`, so there is
  /// nothing to check.
  fn emit_param_type_checks(
    &mut self,
    params: &[Expr],
    regs: &[u8],
    variadic_tail_idx: Option<usize>,
  ) {
    for (i, (param, &reg)) in params.iter().zip(regs.iter()).enumerate() {
      if Some(i) == variadic_tail_idx {
        continue;
      }
      let Expr::Argument(name, type_hint) = param else {
        panic!(
          "compile: function parameter is not Expr::Argument: {:?}",
          param
        );
      };
      let Expr::TypeHint(types, nullable) = type_hint.deref() else {
        panic!(
          "compile: parameter type hint is not Expr::TypeHint: {:?}",
          type_hint
        );
      };
      if types.iter().any(|t| matches!(t, Type::Any)) {
        continue;
      }
      let param_types: Vec<ParamType> = types
        .iter()
        .map(|t| self.ast_type_to_param_type(t))
        .collect();
      let check = ParamTypeCheck {
        param_name: Self::identifier_name(name),
        position: (i + 1) as u32,
        nullable: *nullable,
        types: param_types,
      };
      let check_idx = self.add_param_check(check);
      // A fresh `FunctionScope` starts `current_line` at 0 (see its own
      // field docs); nothing else sets it before the body's first
      // statement does, and these checks run before that. Without this,
      // an uncaught `TypeError` from a bad argument would blame line 0
      // instead of the parameter's own line.
      self.cur_mut().current_line = name.line as u32;
      self.emit(Instr::CheckParamType { reg, check_idx });
    }
  }

  /// Report a genuine, user-triggerable compile error anchored to a
  /// real token from the AST; exactly the same shape the parser's own
  /// `ParseError`s use, so both phases format identically (see
  /// `format_parse_errors` in zuri.rs).
  fn report_error(&mut self, message: String, token: &Token) {
    self.errors.push(ParserError::new(message, token.clone()));
  }

  /// Same, for the handful of sites with no surviving `Token` in the
  /// AST at all; `Expr::Self_`/`Expr::Parent` are unit variants,
  /// `Stmt::Break`/`Stmt::Continue` carry no payload; so this falls
  /// back to whatever line `current_line` was last stamped with (see
  /// `Compiler::emit`). At worst slightly stale within the same
  /// statement; never the wrong function or file.
  fn report_error_here(&mut self, message: String) {
    let line = self.cur().current_line as usize;
    self.errors.push(ParserError {
      message,
      line_number: line,
      offset: 1,
    });
  }

  /// A register holding `nil`, handed back from an expression-compiling
  /// site right after `report_error`/`report_error_here`; keeps the
  /// bytecode structurally valid (every caller still gets *a* register)
  /// even though the function is already known to be invalid and will
  /// never actually run. Mirrors the parser's own `EMPTY_TOKEN`
  /// fallback in `consume_tok!`: report once, substitute something
  /// harmless, keep going so later problems in the same file surface
  /// too.
  fn error_reg(&mut self) -> u8 {
    let dst = self.alloc_reg();
    self.emit(Instr::LoadNil { dst });
    dst
  }

  /// Shared overflow check for every "num_args + 1" call-argument
  /// counter (a plain `.checked_add(1).expect(...)` used to panic here
  /// in four different places); one real limit (255 args in a single
  /// call), reported once per call site instead of duplicated
  /// ad-hoc.
  fn checked_arg_count(&mut self, num_args: u8, token: Option<&Token>) -> u8 {
    match num_args.checked_add(1) {
      Some(n) => n,
      None => {
        let msg = "too many arguments in a single call (max 255)".to_string();
        match token {
          Some(t) => self.report_error(msg, t),
          None => self.report_error_here(msg),
        }
        num_args
      },
    }
  }

  /// Purely syntactic privacy check, run at the exact point a `.name`
  /// access (Get, Set, or a method-call routed through compile_invoke) is
  /// compiled. Both facts it depends on; does `name` start with '_',
  /// and is the receiver spelled `self`/`parent`; are fully known from
  /// the AST at THIS point, for every access site in the program,
  /// regardless of what the receiver's value turns out to be once the
  /// program runs. That's also why the rule can't be narrowed to "methods
  /// only, fields/statics exempt": the compiler has no way to know here
  /// whether `obj` will resolve to an instance or a class at runtime, so
  /// a leading underscore is gated uniformly no matter what it turns out
  /// to name.
  fn check_private_access(&mut self, name: &str, privileged: bool, token: &Token) {
    if name.starts_with('_') && !privileged {
      self.report_error(
        format!(
          "'{}' is private and can only be accessed via 'self' or 'parent'",
          name
        ),
        token,
      );
    }
  }

  fn alloc_reg(&mut self) -> u8 {
    let r = self.cur().next_reg;
    if r == u8::MAX {
      if !self.cur().register_overflow_reported {
        self.cur_mut().register_overflow_reported = true;
        self.report_error_here(format!(
          "expression too complex: this function needs more than {} live registers at once",
          u8::MAX
        ));
      }
      return r;
    }
    let scope = self.cur_mut();
    scope.next_reg += 1;
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
      Instr::JmpIfTrue { offset: o, .. } => *o = offset,
      Instr::PushCatch { offset: o, .. } => *o = offset,
      other => panic!(
        "patch_jump: instruction at {} is not a jump, got {:?}",
        jump_at, other
      ),
    }
  }

  fn emit_loop(&mut self, loop_start: usize) -> usize {
    let jump_at = self.cur().chunk.code.len();
    let raw_offset = loop_start as isize - (jump_at as isize + 1);
    let offset: i16 = raw_offset
      .try_into()
      .expect("loop body too large: offset does not fit in i16");
    self.emit(Instr::Jmp { offset })
  }

  /// Can execution of `stmt` NEVER fall through past its own end --
  /// every path through it exits some other way (`return`, `raise`,
  /// `break`, `continue`) rather than reaching whatever comes next?
  ///
  /// Deliberately conservative: false is always a safe answer (it just
  /// means a caller keeps the ordinary control-flow jump it would have
  /// emitted anyway), so every arm here only returns true when EVERY
  /// path through `stmt` is provably covered. A `Block`'s own answer
  /// defers entirely to its last statement; sound regardless of what
  /// any EARLIER statement does, since if the last one never falls
  /// through, nothing after it is reachable either way. `While` is
  /// always false here even though `while true { ... }` genuinely can
  /// never fall through either; proving that soundly means proving
  /// every exit from the loop body is covered too, which is more
  /// analysis than this optimization is worth.
  fn stmt_never_falls_through(stmt: &Stmt) -> bool {
    match stmt {
      Stmt::Return(_) | Stmt::Raise(_) | Stmt::Break | Stmt::Continue => true,
      Stmt::Block(stmts) => stmts.last().is_some_and(Self::stmt_never_falls_through),
      Stmt::If(_, then_b, Some(else_b)) => {
        Self::stmt_never_falls_through(then_b) && Self::stmt_never_falls_through(else_b)
      },
      _ => false,
    }
  }

  fn identifier_name(token: &Token) -> String {
    match &token.kind {
      TokenKind::Identifier(s) => s.clone(),
      TokenKind::Decorator(s) => s.clone(),
      TokenKind::Literal(s) => s.clone(),
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
  /// some function further out has it; in which case each intermediate
  /// function threads it through as `UpvalueDescriptor::Upvalue`, chaining
  /// the capture inward one level at a time.
  fn resolve_upvalue(&mut self, scope_idx: usize, name: &str) -> Option<u8> {
    if scope_idx == 0 {
      return None;
    }
    let enclosing_idx = scope_idx - 1;

    if let Some(pos) = self.scopes[enclosing_idx]
      .locals
      .iter()
      .rposition(|l| l.name == name)
    {
      self.scopes[enclosing_idx].locals[pos].captured = true;
      let reg = self.scopes[enclosing_idx].locals[pos].reg;
      return Some(self.add_upvalue(scope_idx, UpvalueDescriptor::Local(reg)));
    }
    if let Some(up_idx) = self.resolve_upvalue(enclosing_idx, name) {
      return Some(self.add_upvalue(scope_idx, UpvalueDescriptor::Upvalue(up_idx)));
    }
    None
  }

  /// Does any local currently in scope, at register `from` or above,
  /// need closing before a `break`/`continue` jumps out from under it?
  /// Used instead of the `locals[locals_mark..]` slice `Stmt::Block`
  /// checks, since a mid-block jump doesn't truncate `locals`; every
  /// local from an enclosing scope is still sitting in the same Vec.
  fn locals_captured_from(&self, from: u8) -> bool {
    self
      .cur()
      .locals
      .iter()
      .any(|l| l.reg >= from && l.captured)
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
  /// anywhere; callers (`compile_function_decl`, `Expr::Anonymous`)
  /// decide that.
  fn compile_function_prototype(
    &mut self,
    token: &Token,
    params: &[Expr],
    body: &Stmt,
    is_variadic: bool,
    is_method: bool,
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

    let mut param_regs = Vec::with_capacity(param_names.len());
    for pname in &param_names {
      let reg = self.alloc_reg();
      param_regs.push(reg);
      self.cur_mut().locals.push(Local {
        name: pname.clone(),
        reg,
        is_const: false,
        depth: 0,
        captured: false,
      });
    }
    let variadic_tail_idx = is_variadic.then(|| params.len() - 1);
    self.emit_param_type_checks(params, &param_regs, variadic_tail_idx);

    self.compile_statement(body);

    // if !matches!(self.cur().chunk.code.last(), Some(Instr::Return { .. })) {
    // Make appending a return unconditional so that we can catch
    // functions that don't return anything but whose last instruction
    // is a Instr::Return.
    let nil_reg = self.alloc_reg();
    self.emit(Instr::LoadNil { dst: nil_reg });
    self.emit(Instr::Return { src: nil_reg });
    // }

    let finished = self.scopes.pop().unwrap();
    let jit = JitInfo::new(finished.chunk.code.len());
    ObjFunction {
      name,
      arity: param_names.len() as u8,
      variadic: is_variadic,
      num_registers: finished.max_reg,
      chunk: finished.chunk,
      upvalues: finished.upvalues,
      source_path: self.source_path.clone(),
      globals_module: self.module,
      is_method,
      owning_class_name: None,
      jit,
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

    // A `def` binds a module-level name, so a second one of the same
    // name in the same scope silently replaces the first with no
    // diagnostic at all. The REPL is exempt: redefining something you
    // just typed is the point of it.
    if !self.is_repl {
      let depth = self.cur().scope_depth;
      if self
        .cur()
        .declared_functions
        .iter()
        .any(|(n, d)| *n == name && *d == depth)
      {
        self.report_error(
          format!("multiple declaration for function '{}' found", name),
          token,
        );
      } else {
        self
          .cur_mut()
          .declared_functions
          .push((name.clone(), depth));
      }
    }

    let obj_fn = self.compile_function_prototype(token, params, body, is_variadic, false);
    let proto_val = self.heap.alloc_function(obj_fn);
    let const_idx = self.add_constant(proto_val);

    let mark = self.cur().next_reg;
    let dst = self.alloc_reg();
    self.emit(Instr::Closure {
      dst,
      proto_const: const_idx,
    });
    let name_val = self.heap.alloc_string_old(name);
    let name_const = self.add_constant(name_val);
    self.emit(Instr::SetGlobal {
      name_const,
      src: dst,
    });
    self.free_regs_to(mark);
  }

  /// Like `compile_function_prototype`, but for a class method: register
  /// 0 is ALWAYS reserved for the receiver; even for a static method,
  /// which never reads it; so every call site (Invoke/InvokeSuper) can
  /// use one uniform register layout regardless of whether the target
  /// turns out to be static or not. Only non-static methods get a named
  /// "self" local pointing at it, which is what makes `self` a compile
  /// error inside a static method (resolve_variable("self") simply
  /// won't find it there).
  fn compile_method_prototype(
    &mut self,
    token: &Token,
    params: &[Expr],
    body: &Stmt,
    is_variadic: bool,
    is_static: bool,
    class_name: &str,
  ) -> ObjFunction {
    let name = Self::identifier_name(token);

    let mut param_names = Vec::with_capacity(params.len());
    for param in params {
      match param {
        Expr::Argument(ptoken, _type_hint) => param_names.push(Self::identifier_name(ptoken)),
        other => panic!(
          "compile: method parameter is not Expr::Argument: {:?}",
          other
        ),
      }
    }
    if is_variadic {
      assert!(
        !param_names.is_empty(),
        "compile: variadic method must have a named last parameter"
      );
    }

    self.scopes.push(FunctionScope::new());

    let self_reg = self.alloc_reg();
    if !is_static {
      self.cur_mut().locals.push(Local {
        name: "self".to_string(),
        reg: self_reg,
        is_const: true,
        depth: 0,
        captured: false,
      });
    }

    let mut param_regs = Vec::with_capacity(param_names.len());
    for pname in &param_names {
      let reg = self.alloc_reg();
      param_regs.push(reg);
      self.cur_mut().locals.push(Local {
        name: pname.clone(),
        reg,
        is_const: false,
        depth: 0,
        captured: false,
      });
    }
    let variadic_tail_idx = is_variadic.then(|| params.len() - 1);
    self.emit_param_type_checks(params, &param_regs, variadic_tail_idx);

    self.compile_statement(body);

    // if !matches!(self.cur().chunk.code.last(), Some(Instr::Return { .. })) {
    // Same as the reason for the compile_function_prototype
    // unconditional return: if the last instruction is a return,
    // we don't want to add another one, but if it's not, we need
    // to ensure that the function returns nil.
    let nil_reg = self.alloc_reg();
    self.emit(Instr::LoadNil { dst: nil_reg });
    self.emit(Instr::Return { src: nil_reg });
    // }

    let finished = self.scopes.pop().unwrap();
    let jit = JitInfo::new(finished.chunk.code.len());
    ObjFunction {
      name,
      arity: (param_names.len() + 1) as u8, // +1 for the always-reserved receiver slot
      variadic: is_variadic,
      num_registers: finished.max_reg,
      chunk: finished.chunk,
      upvalues: finished.upvalues,
      source_path: self.source_path.clone(),
      globals_module: self.module,
      is_method: true,
      owning_class_name: Some(class_name.to_string()),
      jit,
    }
  }

  /// Compile a class's own (non-static) field initializers into a
  /// single arity-1 (self) function, run once per instance at
  /// construction time: see `VM::instantiate`. Own field names were
  /// already registered into the class's field_slots table via
  /// Instr::DeclareField before this runs, but that happens at CLASS
  /// declaration time, well before any instance (and thus any call to
  /// this function) exists, so ordering is never actually a race.
  fn compile_field_initializer(
    &mut self,
    class_token: &Token,
    own_fields: &[&Decl],
  ) -> ObjFunction {
    self.scopes.push(FunctionScope::new());
    let self_reg = self.alloc_reg();
    self.cur_mut().locals.push(Local {
      name: "self".to_string(),
      reg: self_reg,
      is_const: true,
      depth: 0,
      captured: false,
    });

    for prop in own_fields {
      if let Decl::Property(fname, value, ..) = prop {
        let mark = self.cur().next_reg;
        let value_reg = self.compile_expression(value);
        let fname_val = self.heap.alloc_string_old(Self::identifier_name(fname));
        let fname_const = self.add_constant(fname_val);
        self.emit(Instr::SetField {
          obj: self_reg,
          name_const: fname_const,
          src: value_reg,
        });
        self.free_regs_to(mark);
      }
    }

    let nil_reg = self.alloc_reg();
    self.emit(Instr::LoadNil { dst: nil_reg });
    self.emit(Instr::Return { src: nil_reg });

    let finished = self.scopes.pop().unwrap();
    let jit = JitInfo::new(finished.chunk.code.len());
    ObjFunction {
      name: format!("@{}_init_fields", Self::identifier_name(class_token)),
      arity: 1,
      variadic: false,
      num_registers: finished.max_reg,
      chunk: finished.chunk,
      upvalues: finished.upvalues,
      source_path: self.source_path.clone(),
      globals_module: self.module,
      is_method: true,
      owning_class_name: Some(Self::identifier_name(class_token)),
      jit,
    }
  }

  /// `class Name < Superclass { ... }` (and `class Name { ... }`).
  /// Sequence: evaluate the superclass expression once, MakeClass a
  /// shell that inherits its method/field tables, register this
  /// class's own fields and build their initializer, evaluate its own
  /// static members, compile and attach its own methods, resolve the
  /// constructor, then bind the class as a global; mirroring how a
  /// top-level function declaration binds itself, just with a lot more
  /// steps in between.
  fn compile_class_decl(
    &mut self,
    name: &Token,
    superclass: &Option<Box<Expr>>,
    properties: &[Decl],
    methods: &[Decl],
    is_extension: bool,
  ) {
    if is_extension {
      self.compile_extension_decl(name, superclass, properties, methods);
      return;
    }

    let class_name = Self::identifier_name(name);
    let mark = self.cur().next_reg;

    let superclass_reg: Option<u8> = superclass
      .as_ref()
      .map(|expr| self.compile_expression(expr));

    let dst = self.alloc_reg();
    let name_val = self.heap.alloc_string_old(class_name.clone());
    let name_const = self.add_constant(name_val);

    self.cur_mut().current_line = name.line as u32;
    self.emit(Instr::MakeClass {
      dst,
      name_const,
      superclass: superclass_reg,
    });

    // Alias the (already-evaluated) superclass value as a synthetic
    // local named "@superclass"; never reachable as a real
    // identifier, since the lexer never produces one starting with '@'
    //; purely so every method compiled below can find it through the
    // same resolve_variable/resolve_upvalue machinery used for any
    // other captured outer local. That's what lets `parent.foo()` work
    // even from inside a closure nested several levels deep in a
    // method body.
    let locals_mark = self.cur().locals.len();
    if let Some(sreg) = superclass_reg {
      let depth = self.cur().scope_depth;
      self.cur_mut().locals.push(Local {
        name: "@superclass".to_string(),
        reg: sreg,
        is_const: true,
        depth,
        captured: false,
      });
    }

    let own_fields: Vec<&Decl> = properties
      .iter()
      .filter(|p| matches!(p, Decl::Property(_, _, _, is_static, _) if !*is_static))
      .collect();

    for prop in &own_fields {
      if let Decl::Property(fname, ..) = prop {
        let fname_val = self.heap.alloc_string_old(Self::identifier_name(fname));
        let fname_const = self.add_constant(fname_val);
        self.emit(Instr::DeclareField {
          class: dst,
          name_const: fname_const,
        });
      }
    }

    // A field doesn't have to go through an explicit `var`; `self.x =
    // value` anywhere in one of this class's own (non-static) methods,
    // most commonly the constructor, is just as visible to the compiler
    // at class-declaration time and counts as "predeclared" the same
    // way. This is what makes e.g. `Person(name) { self.name = name }`
    // valid without a redundant `var name` line above it. Names already
    // covered by an explicit `var`/`const` are skipped here to avoid a
    // redundant instruction; a name inherited from a superclass is
    // invisible to this scan (the superclass's layout only exists as a
    // runtime Value by now) and is instead deduped where it actually
    // matters; Instr::DeclareField's own handler in vm.rs.
    let explicit_names: Vec<String> = own_fields
      .iter()
      .filter_map(|p| match p {
        Decl::Property(fname, ..) => Some(Self::identifier_name(fname)),
        _ => None,
      })
      .collect();

    let mut implicit_names: Vec<String> = Vec::new();
    for m in methods.iter().filter(|f| f.is_method("@new")) {
      if let Decl::Method(_, _, body, _, is_static) = m {
        if !*is_static {
          collect_self_fields_stmt(body, &mut implicit_names);
        }
      }
    }
    implicit_names.retain(|n| !explicit_names.contains(n));

    for fname in &implicit_names {
      let fname_val = self.heap.alloc_string_old(fname.clone());
      let fname_const = self.add_constant(fname_val);
      self.emit(Instr::DeclareField {
        class: dst,
        name_const: fname_const,
      });
    }

    if !own_fields.is_empty() {
      let init_fn = self.compile_field_initializer(name, &own_fields);
      let proto_val = self.heap.alloc_function(init_fn);
      let const_idx = self.add_constant(proto_val);
      let freg = self.alloc_reg();
      self.emit(Instr::Closure {
        dst: freg,
        proto_const: const_idx,
      });
      self.emit(Instr::SetFieldInit {
        class: dst,
        src: freg,
      });
      self.free_regs_to(freg);
    }

    for prop in properties {
      if let Decl::Property(pname, value, _, true, _) = prop {
        let mark2 = self.cur().next_reg;
        let vreg = self.compile_expression(value);
        let pname_val = self.heap.alloc_string_old(Self::identifier_name(pname));
        let pname_const = self.add_constant(pname_val);
        self.emit(Instr::DeclareStatic {
          class: dst,
          name_const: pname_const,
          src: vreg,
        });
        self.free_regs_to(mark2);
      }
    }

    let mut seen_method_names: Vec<String> = Vec::new();
    for m in methods {
      if let Decl::Method(mname, ..) = m {
        let mname_str = Self::identifier_name(mname);
        if seen_method_names.contains(&mname_str) {
          self.report_error(
            format!(
              "multiple declaration for method '{}' found in class '{}'",
              mname_str, class_name
            ),
            mname,
          );
        } else {
          seen_method_names.push(mname_str);
        }
      }
    }

    for m in methods {
      if let Decl::Method(mname, params, body, is_variadic, is_static) = m {
        let obj_fn =
          self.compile_method_prototype(mname, params, body, *is_variadic, *is_static, &class_name);
        let proto_val = self.heap.alloc_function(obj_fn);
        let const_idx = self.add_constant(proto_val);
        let mreg = self.alloc_reg();
        self.emit(Instr::Closure {
          dst: mreg,
          proto_const: const_idx,
        });
        let mname_val = self.heap.alloc_string_old(Self::identifier_name(mname));
        let mname_const = self.add_constant(mname_val);
        if *is_static {
          self.emit(Instr::DeclareStatic {
            class: dst,
            name_const: mname_const,
            src: mreg,
          });
        } else {
          self.emit(Instr::SetMethod {
            class: dst,
            name_const: mname_const,
            src: mreg,
          });
        }
        self.free_regs_to(mreg);
      }
    }

    self.emit(Instr::FinalizeClass { class: dst });
    self.emit(Instr::SetGlobal {
      name_const,
      src: dst,
    });

    let any_captured = self.cur().locals[locals_mark..].iter().any(|l| l.captured);
    self.cur_mut().locals.truncate(locals_mark);
    if any_captured {
      self.emit(Instr::CloseUpvalues { from: mark });
    }
    self.free_regs_to(mark);
  }

  /// `class Name > Target { ... }`; monkey-patches new methods directly
  /// into an ALREADY-DECLARED class's live method table, rather than
  /// building a new class of its own. Every member must be `static`
  /// (enforced below) and takes the instance it's called on as an
  /// ordinary, explicit first parameter; there's no implicit `self`
  /// binding here, since these compile as plain functions (just tagged
  /// `is_method: true` so Invoke/GetField still treat the installed
  /// closure as receiver-expecting once it's sitting in a real method
  /// table).
  ///
  /// See this method's caller for the load-bearing caveat about WHEN
  /// this patch becomes visible to already-declared subclasses.
  fn compile_extension_decl(
    &mut self,
    name: &Token,
    target: &Option<Box<Expr>>,
    properties: &[Decl],
    methods: &[Decl],
  ) {
    let ext_name = Self::identifier_name(name);

    let target_expr = target.as_ref().unwrap_or_else(|| {
      panic!(
        "compile: extension '{}' must specify a target class with '> Target'",
        ext_name
      )
    });

    if !properties.is_empty() {
      self.report_error(
        format!(
          "extension '{}' can only declare static methods but not fields",
          ext_name
        ),
        name,
      );
    }
    for m in methods {
      if let Decl::Method(mname, _, _, _, is_static) = m {
        if !*is_static {
          self.report_error(
            format!(
              "extension method '{}' must be declared 'static' and receive \
               the instance explicitly as their own first parameter if desired",
              Self::identifier_name(mname)
            ),
            mname,
          );
        }
      }
    }

    let mark = self.cur().next_reg;
    let target_reg = self.compile_expression(target_expr);

    for m in methods {
      if let Decl::Method(mname, params, body, is_variadic, _is_static) = m {
        let obj_fn = self.compile_function_prototype(mname, params, body, *is_variadic, true);
        let proto_val = self.heap.alloc_function(obj_fn);
        let const_idx = self.add_constant(proto_val);
        let mreg = self.alloc_reg();
        self.emit(Instr::Closure {
          dst: mreg,
          proto_const: const_idx,
        });
        let mname_val = self.heap.alloc_string_old(Self::identifier_name(mname));
        let mname_const = self.add_constant(mname_val);
        self.emit(Instr::SetMethod {
          class: target_reg,
          name_const: mname_const,
          src: mreg,
        });
        self.free_regs_to(mreg);
      }
    }

    self.free_regs_to(mark);
  }

  /// Resolve the current method's implicit receiver to a register,
  /// exactly like resolving any other named local; `self` is pushed
  /// as a real (synthetic) Local when compiling a method body
  /// specifically so nested closures can capture it as an upvalue
  /// through the same mechanism as any other outer local (see
  /// `compile_method_prototype`). Panics outside of a method, where no
  /// such local exists.
  fn compile_self_reg(&mut self, parent: bool) -> u8 {
    match self.resolve_variable("self") {
      VarLoc::Local(reg, _) => reg,
      VarLoc::Upvalue(idx) => {
        let dst = self.alloc_reg();
        self.emit(Instr::GetUpval { dst, idx });
        dst
      },
      VarLoc::Global => {
        self.report_error_here(format!(
          "'{}' used outside of a method",
          if parent { "parent" } else { "self" }
        ));
        self.error_reg()
      },
    }
  }

  /// Compile the object half of a `.field`/`.field(...)` access.
  /// `parent` is special-cased to mean "the current self" here --
  /// unlike a method CALL through `parent` (see `compile_invoke_super`),
  /// a plain field read/write isn't virtual (every class's fields live
  /// in one flat, already-inherited slot layout), so `parent.x` and
  /// `self.x` are simply the same access.
  fn compile_receiver(&mut self, expr: &Expr) -> u8 {
    match expr {
      Expr::Parent => self.compile_self_reg(true),
      other => self.compile_expression(other),
    }
  }

  /// `obj.method(args)`; fused into one Invoke instruction rather
  /// than a Get producing a bound-method object followed by a plain
  /// Call, to skip that heap allocation on every method call. The
  /// receiver is duplicated into `obj_reg + 1`: see Instr::Invoke's own
  /// doc comment for why.
  fn compile_invoke(&mut self, obj: &Expr, method: &Token, args: &[Expr]) -> u8 {
    let method_name = Self::identifier_name(method);

    // `parent.foo(...)` never reaches this function; it's intercepted
    // earlier and routed to compile_invoke_super, which is
    // unconditionally privileged (see check_private_access's own doc
    // comment for why this has to be checked here, purely
    // syntactically, rather than deferred to Invoke's runtime handler).
    self.check_private_access(&method_name, matches!(obj, Expr::Self_), method);

    let obj_reg = self.alloc_reg();
    let raw = self.compile_receiver(obj);
    if raw != obj_reg {
      self.emit(Instr::Move {
        dst: obj_reg,
        src: raw,
      });
    }
    self.free_regs_to(obj_reg + 1);

    let self_slot = self.alloc_reg();
    self.emit(Instr::Move {
      dst: self_slot,
      src: obj_reg,
    });

    let mut num_args: u8 = 0;
    for arg in args {
      let expected = self_slot + 1 + num_args;
      let arg_reg = self.compile_expression(arg);
      if arg_reg != expected {
        self.emit(Instr::Move {
          dst: expected,
          src: arg_reg,
        });
      }
      self.free_regs_to(expected + 1);
      num_args = self.checked_arg_count(num_args, Some(method));
    }

    let method_val = self.heap.alloc_string_old(method_name);
    let method_const = self.add_constant(method_val);

    let dst = obj_reg;
    self.cur_mut().current_line = method.line as u32;
    self.emit(Instr::Invoke {
      dst,
      obj: obj_reg,
      method_const,
      num_args,
    });
    self.free_regs_to(obj_reg + 1);
    dst
  }

  /// `parent.method(args)`; statically resolves which class's method
  /// table to look in (the current method's lexical superclass,
  /// captured via the synthetic "@superclass" local: see
  /// `compile_class_decl`) while still binding the CURRENT self, unlike
  /// a plain virtual `self.method()` call.
  fn compile_invoke_super(&mut self, method: &Token, args: &[Expr]) -> u8 {
    let call_reg = self.alloc_reg();
    let raw_super = match self.resolve_variable("@superclass") {
      VarLoc::Local(reg, _) => reg,
      VarLoc::Upvalue(idx) => {
        let dst = self.alloc_reg();
        self.emit(Instr::GetUpval { dst, idx });
        dst
      },
      VarLoc::Global => {
        self.report_error_here("'parent' used in a class with no superclass".to_string());
        self.error_reg()
      },
    };
    if raw_super != call_reg {
      self.emit(Instr::Move {
        dst: call_reg,
        src: raw_super,
      });
    }
    self.free_regs_to(call_reg + 1);

    let self_slot = self.alloc_reg();
    let self_reg = self.compile_self_reg(true);
    self.emit(Instr::Move {
      dst: self_slot,
      src: self_reg,
    });
    self.free_regs_to(self_slot + 1);

    let mut num_args: u8 = 0;
    for arg in args {
      let expected = self_slot + 1 + num_args;
      let arg_reg = self.compile_expression(arg);
      if arg_reg != expected {
        self.emit(Instr::Move {
          dst: expected,
          src: arg_reg,
        });
      }
      self.free_regs_to(expected + 1);
      num_args = self.checked_arg_count(num_args, Some(method));
    }

    let method_name = Self::identifier_name(method);
    let method_val = self.heap.alloc_string_old(method_name);
    let method_const = self.add_constant(method_val);

    let dst = call_reg;
    self.cur_mut().current_line = method.line as u32;
    self.emit(Instr::InvokeSuper {
      dst,
      superclass: call_reg,
      method_const,
      num_args,
    });
    self.free_regs_to(call_reg + 1);
    dst
  }

  // Mirrors compile_invoke_super, minus the method-name lookup
  /// `parent(args)`; calls the SUPERCLASS's own constructor on the
  /// CURRENT self. See `Instr::CallSuperCtor`'s doc comment for why this
  /// reads `ObjClass::constructor` directly instead of a name lookup.
  fn compile_super_ctor_call(&mut self, args: &[Expr]) -> u8 {
    let call_reg = self.alloc_reg();
    let raw_super = match self.resolve_variable("@superclass") {
      VarLoc::Local(reg, _) => reg,
      VarLoc::Upvalue(idx) => {
        let dst = self.alloc_reg();
        self.emit(Instr::GetUpval { dst, idx });
        dst
      },
      VarLoc::Global => {
        self.report_error_here("'parent' used in a class with no superclass".to_string());
        self.error_reg()
      },
    };
    if raw_super != call_reg {
      self.emit(Instr::Move {
        dst: call_reg,
        src: raw_super,
      });
    }
    self.free_regs_to(call_reg + 1);

    let self_slot = self.alloc_reg();
    let self_reg = self.compile_self_reg(true);
    self.emit(Instr::Move {
      dst: self_slot,
      src: self_reg,
    });
    self.free_regs_to(self_slot + 1);

    let mut num_args: u8 = 0;
    for arg in args {
      let expected = self_slot + 1 + num_args;
      let arg_reg = self.compile_expression(arg);
      if arg_reg != expected {
        self.emit(Instr::Move {
          dst: expected,
          src: arg_reg,
        });
      }
      self.free_regs_to(expected + 1);
      num_args = self.checked_arg_count(num_args, None);
    }

    let dst = call_reg;
    self.emit(Instr::CallSuperCtor {
      dst,
      superclass: call_reg,
      num_args,
    });
    self.free_regs_to(call_reg + 1);
    dst
  }

  /// Registers needed per list element is 1. Chosen conservatively so
  /// one chunk's own working set (plus the handful of extra registers
  /// `compile_list_literal`'s merge step needs) stays far under the
  /// 255-register ceiling regardless of how many OTHER registers
  /// surrounding code in the same function has already claimed.
  const LIST_LITERAL_CHUNK_SIZE: usize = 64;

  /// Same idea, but a dict entry needs TWO registers (key + value), so
  /// this is roughly half the list chunk size for the same total
  /// register budget per chunk.
  const DICT_LITERAL_CHUNK_SIZE: usize = 48;

  /// `[a, b, c, ...]`. A short literal compiles straight to one
  /// `MakeList` (the fast path). A literal longer than
  /// `LIST_LITERAL_CHUNK_SIZE` is built INCREMENTALLY instead: an empty
  /// list, then repeated `list.extend(chunk)` calls, each `chunk` a
  /// small, ordinary `MakeList` of its own. This is what lets a
  /// several-hundred-element literal compile at all; a single
  /// `MakeList` over the whole thing would need one register PER
  /// ELEMENT, all live at once, which this VM's `u8`-sized register
  /// operands can't address past ~255.
  fn compile_list_literal(&mut self, elements: &[Expr]) -> u8 {
    if elements.len() <= Self::LIST_LITERAL_CHUNK_SIZE {
      return self.compile_list_chunk(elements);
    }

    let result = self.alloc_reg();
    self.emit(Instr::MakeList {
      dst: result,
      start: result,
      count: 0,
    });

    for chunk in elements.chunks(Self::LIST_LITERAL_CHUNK_SIZE) {
      let chunk_mark = self.cur().next_reg;
      let chunk_reg = self.compile_list_chunk(chunk);
      self.emit_extend_call(result, chunk_reg);
      self.free_regs_to(chunk_mark);
    }

    self.free_regs_to(result.saturating_add(1));
    result
  }

  /// One bounded (`<= LIST_LITERAL_CHUNK_SIZE` elements) list literal,
  /// via a single `MakeList`; the ORIGINAL `Expr::List` compiling
  /// logic, just factored out and switched from raw register arithmetic
  /// to `alloc_reg()` (which reports a proper compile error instead of
  /// panicking if this function is somehow already almost out of
  /// registers: see `alloc_reg`'s own overflow handling).
  fn compile_list_chunk(&mut self, elements: &[Expr]) -> u8 {
    // `dst` is reserved BEFORE the elements, so the elements sit above
    // it and freeing back to `dst + 1` afterwards reclaims every one of
    // them. Allocating `dst` after the elements instead (the obvious
    // order, and what this used to do) leaves the whole run below it
    // pinned for the rest of the function: nothing frees them, because
    // the register this returns belongs to the caller and the caller
    // has no idea how much scratch is stacked underneath it. A handful
    // of literals in one function was enough to exhaust the register
    // file that way.
    //
    // `dst` deliberately stays distinct from every element register.
    // Overlapping it with the first element would be safe for the
    // interpreter, which reads all the sources before writing, but the
    // JIT's scalar-replacement path never writes `dst` at all and
    // keys its stack slot on it, so an alias there silently changes
    // what a later read of that register means.
    let dst = self.alloc_reg();
    let start = self.cur().next_reg;
    let mut count: u8 = 0;
    for elem in elements {
      let expected = self.alloc_reg();
      let elem_reg = self.compile_expression(elem);
      if elem_reg != expected {
        self.emit(Instr::Move {
          dst: expected,
          src: elem_reg,
        });
      }
      self.free_regs_to(expected.saturating_add(1));
      // Safe: callers only ever pass a slice bounded by
      // LIST_LITERAL_CHUNK_SIZE (64), far under u8::MAX.
      count += 1;
    }
    self.emit(Instr::MakeList { dst, start, count });
    self.free_regs_to(dst.saturating_add(1));
    dst
  }

  /// `{k: v, ...}`. Same chunk-and-extend strategy as
  /// `compile_list_literal`, for the same register-ceiling reason --
  /// each entry needs two registers (key + value) instead of one, so
  /// chunking matters here even sooner.
  fn compile_dict_literal(&mut self, keys: &[Expr], values: &[Expr]) -> u8 {
    debug_assert_eq!(
      keys.len(),
      values.len(),
      "Dict keys and values must be the same length"
    );

    if keys.len() <= Self::DICT_LITERAL_CHUNK_SIZE {
      return self.compile_dict_chunk(keys, values);
    }

    let result = self.alloc_reg();
    self.emit(Instr::MakeDict {
      dst: result,
      start: result,
      count: 0,
    });

    let mut idx = 0;
    while idx < keys.len() {
      let end = (idx + Self::DICT_LITERAL_CHUNK_SIZE).min(keys.len());
      let chunk_mark = self.cur().next_reg;
      let chunk_reg = self.compile_dict_chunk(&keys[idx..end], &values[idx..end]);
      self.emit_extend_call(result, chunk_reg);
      self.free_regs_to(chunk_mark);
      idx = end;
    }

    self.free_regs_to(result.saturating_add(1));
    result
  }

  /// One bounded (`<= DICT_LITERAL_CHUNK_SIZE` pairs) dict literal, via
  /// a single `MakeDict`. Keys and values are still compiled as two
  /// back-to-back contiguous register runs (what `MakeDict` requires),
  /// just via `alloc_reg()` instead of raw arithmetic.
  fn compile_dict_chunk(&mut self, keys: &[Expr], values: &[Expr]) -> u8 {
    // Reserved ahead of the pairs for the same reason
    // `compile_list_chunk` reserves it ahead of the elements, and it
    // matters twice as much here: every pair costs two registers, so a
    // dict literal left allocated pins `2 * count` of them.
    let dst = self.alloc_reg();
    let start = self.cur().next_reg;
    // Safe: callers only ever pass slices bounded by
    // DICT_LITERAL_CHUNK_SIZE (48), far under u8::MAX.
    let count = keys.len() as u8;

    for key_expr in keys {
      let expected = self.alloc_reg();
      let key_reg = self.compile_expression(key_expr);
      if key_reg != expected {
        self.emit(Instr::Move {
          dst: expected,
          src: key_reg,
        });
      }
      self.free_regs_to(expected.saturating_add(1));
    }
    for val_expr in values {
      let expected = self.alloc_reg();
      let val_reg = self.compile_expression(val_expr);
      if val_reg != expected {
        self.emit(Instr::Move {
          dst: expected,
          src: val_reg,
        });
      }
      self.free_regs_to(expected.saturating_add(1));
    }

    self.emit(Instr::MakeDict { dst, start, count });
    self.free_regs_to(dst.saturating_add(1));
    dst
  }

  /// Emits `receiver_reg.extend(arg_reg)` as an ordinary `Invoke` --
  /// shared by both the list and dict chunking paths above. Follows the
  /// exact same calling convention `compile_invoke` always uses (the
  /// receiver is duplicated into `obj + 1`: see `Instr::Invoke`'s own
  /// doc comment), even though the native `extend()` methods only
  /// actually read `obj + 2..`; staying consistent with the general
  /// convention is simpler than special-casing "this call happens to
  /// target a native."
  fn emit_extend_call(&mut self, receiver_reg: u8, arg_reg: u8) {
    let name_val = self.heap.alloc_string_old("extend".to_string());
    let name_const = self.add_constant(name_val);

    let obj_reg = self.alloc_reg();
    self.emit(Instr::Move {
      dst: obj_reg,
      src: receiver_reg,
    });
    let self_slot = self.alloc_reg();
    self.emit(Instr::Move {
      dst: self_slot,
      src: obj_reg,
    });
    let arg_slot = self.alloc_reg();
    if arg_slot != arg_reg {
      self.emit(Instr::Move {
        dst: arg_slot,
        src: arg_reg,
      });
    }

    self.emit(Instr::Invoke {
      dst: obj_reg,
      obj: obj_reg,
      method_const: name_const,
      num_args: 1,
    });
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
        let str_val = self.heap.alloc_string_old(literal.clone());
        let const_idx = self.add_constant(str_val);
        self.emit(Instr::LoadConst { dst, const_idx });
        dst
      },
      Expr::BigNumber(number) => {
        // String literal: intern it on the heap now, at compile time, and
        // reference the resulting Value from the constant pool.
        let dst = self.alloc_reg();
        let str_val = self.heap.alloc_bigint_old(number.clone());
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
            let str_val = self.heap.alloc_string_old(name);
            let name_const = self.add_constant(str_val);
            self.cur_mut().current_line = token.line as u32;
            self.emit(Instr::GetGlobal { dst, name_const });
            dst
          },
        }
      },
      Expr::Unary(op, expr, line) => {
        let mark = self.cur().next_reg;
        let src = self.compile_expression(expr);
        let dst = if src >= mark { src } else { self.alloc_reg() };

        let instr = match op {
          TokenKind::Minus => Instr::Neg { dst, src },
          TokenKind::Bang => Instr::Not { dst, src },
          TokenKind::Tilde => Instr::BitNot { dst, src },
          _ => panic!("compile_expression: unsupported unary operator: {:?}", op),
        };
        self.cur_mut().current_line = *line;
        self.emit(instr);
        dst
      },
      Expr::Binary(lhs, op, rhs, line) => {
        if let Some(imm) = literal_as_f64(rhs) {
          if let Some(ctor) = imm_arith_ctor(op) {
            let mark = self.cur().next_reg;
            let a = self.compile_expression(lhs);
            let dst = if a >= mark { a } else { self.alloc_reg() };
            let imm_const = self.add_constant(Value::number(imm));
            self.cur_mut().current_line = *line;
            self.emit(ctor(dst, a, imm_const));
            self.free_regs_to(dst + 1);
            return dst;
          }
        }

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

        self.cur_mut().current_line = *line;
        self.emit(instr);

        self.free_regs_to(rhs_mark.max(dst + 1));
        dst
      },
      Expr::Logical(lhs, op, rhs, line) => {
        if let Some(imm) = literal_as_f64(rhs) {
          if let Some(ctor) = imm_logical_ctor(op) {
            let mark = self.cur().next_reg;
            let a = self.compile_expression(lhs);
            let dst = if a >= mark { a } else { self.alloc_reg() };
            let imm_const = self.add_constant(Value::number(imm));
            self.cur_mut().current_line = *line;
            self.emit(ctor(dst, a, imm_const));
            self.free_regs_to(dst + 1);
            return dst;
          }
        }

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
          _ => panic!(
            "compile_expression: unsupported comparison operator: {:?}",
            op
          ),
        };

        self.cur_mut().current_line = *line;
        self.emit(instr);

        self.free_regs_to(rhs_mark.max(dst + 1));
        dst
      },
      Expr::Circuit(lhs, op, rhs) => {
        let mark = self.cur().next_reg;
        let result = mark;

        // Force lhs into the fixed result register; same trick as
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
      Expr::Assign(target, value) => match target.as_ref() {
        // `obj[idx] = value`; and, via the parser's generic Increment/
        // Decrement desugaring in assign_expr, also `obj[idx]++` /
        // `obj[idx]--`. No explicit free here (matching the VarLoc::Global
        // case below): value_reg is the expression's result and must stay
        // allocated for whatever called compile_expression to consume; the
        // enclosing statement's own mark eventually reclaims obj_reg/idx_reg
        // too.
        Expr::Index(obj, idx, line) => {
          let obj_reg = self.compile_expression(obj);
          let idx_reg = self.compile_expression(idx);
          let value_reg = self.compile_expression(value);
          self.cur_mut().current_line = *line;
          self.emit(Instr::SetIndex {
            obj: obj_reg,
            idx: idx_reg,
            src: value_reg,
          });
          value_reg
        },
        Expr::Identifier(token) => {
          let name = Self::identifier_name(token);
          match self.resolve_variable(&name) {
            VarLoc::Local(_, true) => {
              self.report_error(format!("cannot assign to constant '{}'", name), token);
              // Still compile the RHS and hand its register back, so the
              // surrounding expression stays well-formed for anything
              // still nested inside it.
              self.compile_expression(value)
            },
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
              let str_val = self.heap.alloc_string_old(name);
              let name_const = self.add_constant(str_val);

              if self.scopes.last().unwrap().scope_depth > 0 {
                self.emit(Instr::AssignGlobal {
                  name_const,
                  src: value_reg,
                });
              } else {
                self.emit(Instr::SetGlobal {
                  name_const,
                  src: value_reg,
                });
              }
              value_reg
            },
          }
        },
        // The parser rejects malformed assignment targets (e.g. `5 = 1`,
        // `5++`) before it ever builds this node, so this arm shouldn't be
        // reachable in practice. Still, an unreachable-in-the-happy-path
        // panic is a bad way to fail on user-triggerable input, so degrade
        // the same way the constant-assignment case above does.
        other => {
          self.report_error_here(format!("unsupported assignment target: {:?}", other));
          self.error_reg()
        },
      },
      Expr::Call(callee, args, line) => {
        if let Expr::Get(obj, method) = callee.as_ref() {
          return if matches!(obj.as_ref(), Expr::Parent) {
            self.compile_invoke_super(method, args)
          } else {
            self.compile_invoke(obj, method, args)
          };
        }
        if matches!(callee.as_ref(), Expr::Parent) {
          return self.compile_super_ctor_call(args);
        }

        if let Expr::Get(obj, method) = callee.as_ref() {
          return if matches!(obj.as_ref(), Expr::Parent) {
            self.compile_invoke_super(method, args)
          } else {
            self.compile_invoke(obj, method, args)
          };
        }

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

          num_args = if let Expr::Get(obj, method) = callee.as_ref() {
            self.checked_arg_count(num_args, Some(method))
          } else {
            self.checked_arg_count(num_args, None)
          };
        }

        let dst = func_reg;

        self.cur_mut().current_line = *line;
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
          let obj_fn = self.compile_function_prototype(token, params, body, *is_variadic, false);
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
      Expr::List(elements) => self.compile_list_literal(elements),
      Expr::Dict(keys, values) => self.compile_dict_literal(keys, values),
      Expr::Self_ => self.compile_self_reg(false),
      Expr::Parent => {
        panic!("compile: 'parent' must be followed by '.member' or '.member(...)'")
      },
      Expr::Get(obj, field) => {
        let field_name = Self::identifier_name(field);

        // Both self.x and parent.x count as privileged here; a plain
        // (non-call) property access through `parent` is never virtual
        // to begin with (see compile_receiver's own doc comment).
        self.check_private_access(
          &field_name,
          matches!(obj.as_ref(), Expr::Self_ | Expr::Parent),
          field,
        );

        let obj_reg = self.compile_receiver(obj);
        let dst = self.alloc_reg();
        let fname_val = self.heap.alloc_string_old(field_name);
        let fname_const = self.add_constant(fname_val);
        self.cur_mut().current_line = field.line as u32;
        self.emit(Instr::GetField {
          dst,
          obj: obj_reg,
          name_const: fname_const,
        });
        dst
      },
      Expr::Set(obj, field, value) => {
        let field_name = Self::identifier_name(field);

        self.check_private_access(
          &field_name,
          matches!(obj.as_ref(), Expr::Self_ | Expr::Parent),
          field,
        );

        let obj_reg = self.compile_receiver(obj);
        let value_reg = self.compile_expression(value);
        let fname_val = self.heap.alloc_string_old(field_name);
        let fname_const = self.add_constant(fname_val);
        self.cur_mut().current_line = field.line as u32;
        self.emit(Instr::SetField {
          obj: obj_reg,
          name_const: fname_const,
          src: value_reg,
        });
        value_reg
      },

      Expr::Index(obj, idx, line) => {
        let mark = self.cur().next_reg;
        let obj_reg = self.compile_expression(obj);
        let dst = if obj_reg >= mark {
          obj_reg
        } else {
          self.alloc_reg()
        };

        let idx_mark = self.cur().next_reg;
        let idx_reg = self.compile_expression(idx);

        self.cur_mut().current_line = *line;
        self.emit(Instr::GetIndex {
          dst,
          obj: obj_reg,
          idx: idx_reg,
        });
        self.free_regs_to(idx_mark.max(dst + 1));
        dst
      },
      Expr::Slice(obj, lo, hi, line) => {
        let mark = self.cur().next_reg;
        let obj_reg = self.compile_expression(obj);
        let dst = if obj_reg >= mark {
          obj_reg
        } else {
          self.alloc_reg()
        };

        let operand_mark = self.cur().next_reg;
        let lo_reg = self.compile_expression(lo);
        let hi_reg = self.compile_expression(hi);

        self.cur_mut().current_line = *line;
        self.emit(Instr::GetSlice {
          dst,
          obj: obj_reg,
          lo: lo_reg,
          hi: hi_reg,
        });
        self.free_regs_to(operand_mark.max(dst + 1));
        dst
      },
      Expr::Range(lower, upper, line) => {
        let mark = self.cur().next_reg;
        let lo = self.compile_expression(lower);
        let dst = if lo >= mark { lo } else { self.alloc_reg() };

        let hi_mark = self.cur().next_reg;
        let hi = self.compile_expression(upper);

        self.cur_mut().current_line = *line;
        self.emit(Instr::MakeRange {
          dst,
          lower: lo,
          upper: hi,
        });
        self.free_regs_to(hi_mark.max(dst + 1));
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

  /// `using subject { when a, b { ... } when c { ... } default { ... } }`.
  ///
  /// `labels`/`bodies` are already index-aligned by the parser (a `when
  /// a, b { block }` with multiple labels desugars into one entry per
  /// label, each pointing at its own CLONE of the same block: see
  /// Parser::using_stmt). This groups those clones back together via
  /// structural equality over adjacent entries (always contiguous,
  /// since the parser only ever produces them that way), so each
  /// distinct body is compiled exactly once regardless of how many
  /// labels point at it.
  ///
  /// Every label whose value is compile-time-knowable (see
  /// `expr_as_jump_key`) is registered into a hash-based jump table,
  /// checked FIRST at runtime via one `Instr::UsingJump`, in O(1)
  /// regardless of arm count. Only labels that AREN'T compile-time-
  /// knowable (an arbitrary expression, a variable, a BigNumber, ...)
  /// fall back to sequential evaluate-and-compare; and even then,
  /// only those specific labels, never the constant ones.
  fn compile_using(
    &mut self,
    subject: &Expr,
    labels: &[Expr],
    bodies: &[Stmt],
    default: &Option<Box<Stmt>>,
  ) {
    struct Group<'a> {
      body: &'a Stmt,
      const_keys: Vec<JumpKey>,
      dynamic_labels: Vec<&'a Expr>,
    }

    let mut groups: Vec<Group> = Vec::new();
    for (label, body) in labels.iter().zip(bodies.iter()) {
      let same_as_last = groups.last().is_some_and(|g| g.body == body);
      if !same_as_last {
        groups.push(Group {
          body,
          const_keys: Vec::new(),
          dynamic_labels: Vec::new(),
        });
      }
      let group = groups.last_mut().unwrap();
      match expr_as_jump_key(label) {
        Some(key) => group.const_keys.push(key),
        None => group.dynamic_labels.push(label),
      }
    }

    let stmt_mark = self.cur().next_reg;
    let raw_subject = self.compile_expression(subject);
    let subj = if raw_subject >= stmt_mark {
      raw_subject
    } else {
      let r = self.alloc_reg();
      self.emit(Instr::Move {
        dst: r,
        src: raw_subject,
      });
      r
    };
    // Reserve `subj` for the whole statement; every dynamic-label
    // comparison and, eventually, every group body reads it.
    self.free_regs_to(subj + 1);

    let table_idx = self.cur_mut().chunk.add_jump_table();
    self.emit(Instr::UsingJump {
      subject: subj,
      table_idx,
    });

    // Sequential fallback: only dynamic labels ever get a comparison
    // emitted here. A group with zero dynamic labels (the common case,
    // matching every current test file) contributes nothing to this
    // section at all.
    let mut group_dyn_jumps: Vec<Vec<usize>> = Vec::with_capacity(groups.len());
    for group in &groups {
      let mut jumps = Vec::new();
      for &label in &group.dynamic_labels {
        let mark = self.cur().next_reg;
        let label_reg = self.compile_expression(label);
        let eq_reg = self.alloc_reg();
        self.emit(Instr::Eq {
          dst: eq_reg,
          a: subj,
          b: label_reg,
        });
        let jump_site = self.emit_jump_if_true(eq_reg);
        self.free_regs_to(mark);
        jumps.push(jump_site);
      }
      group_dyn_jumps.push(jumps);
    }

    // Reached only if the jump table missed AND every dynamic check
    // above also missed.
    if let Some(stmt) = default {
      self.compile_statement(stmt);
    }
    let after_default_jump = self.emit_jump();

    let mut end_jumps = Vec::new();
    for (group, dyn_jumps) in groups.iter().zip(group_dyn_jumps.into_iter()) {
      let group_start = self.cur().chunk.code.len();

      for key in &group.const_keys {
        self.cur_mut().chunk.jump_tables[table_idx as usize].insert(key.clone(), group_start);
      }
      for site in dyn_jumps {
        self.patch_jump(site);
      }

      self.compile_statement(group.body);
      end_jumps.push(self.emit_jump());
    }

    let after_point = self.cur().chunk.code.len();
    self.patch_jump(after_default_jump);
    for j in end_jumps {
      self.patch_jump(j);
    }
    debug_assert_eq!(
      self.cur().chunk.code.len(),
      after_point,
      "patching using-statement jumps should not emit new code"
    );

    self.free_regs_to(stmt_mark);
  }

  fn compile_raise(&mut self, expr: &Expr) {
    let reg = self.compile_expression(expr);
    self.emit(Instr::Raise { src: reg });
  }

  /// `assert COND, MSG` desugars into: if COND is falsey, construct
  /// `AssertError(MSG or 'Assertion failed')` via the exact same
  /// global-lookup + Call path a user's own `raise AssertError(...)`
  /// would use (so its constructor runs normally), then `Raise` it --
  /// reusing Stmt::Raise's own instruction for the stacktrace-
  /// attachment and Error-subclass validation, rather than
  /// duplicating either.
  fn compile_assert(&mut self, cond: &Expr, message: &Option<Box<Expr>>) {
    let mark = self.cur().next_reg;
    let cond_reg = self.compile_expression(cond);
    let skip = self.emit_jump_if_true(cond_reg);
    self.free_regs_to(mark);

    let class_reg = self.alloc_reg();
    let class_name_val = self.heap.alloc_string_old("AssertError".to_string());
    let class_name_const = self.add_constant(class_name_val);
    self.emit(Instr::GetGlobal {
      dst: class_reg,
      name_const: class_name_const,
    });

    let expected_msg_reg = class_reg + 1;
    let msg_reg = match message {
      Some(m) => {
        let mreg = self.compile_expression(m);
        if mreg != expected_msg_reg {
          self.emit(Instr::Move {
            dst: expected_msg_reg,
            src: mreg,
          });
        }
        expected_msg_reg
      },
      None => {
        let default_val = self.heap.alloc_string_old("Assertion failed".to_string());
        let default_const = self.add_constant(default_val);
        self.emit(Instr::LoadConst {
          dst: expected_msg_reg,
          const_idx: default_const,
        });
        expected_msg_reg
      },
    };
    self.free_regs_to(msg_reg + 1);

    self.emit(Instr::Call {
      dst: class_reg,
      func: class_reg,
      num_args: 1,
    });
    self.emit(Instr::Raise { src: class_reg });

    self.free_regs_to(mark);
    self.patch_jump(skip);
  }

  /// `catch { body } as var { error_block }` (and the two shorter
  /// forms). `var`, if present, is declared as an ORDINARY Local in the
  /// CURRENT scope (not a nested one); exactly like a `var` statement
  ///; so it stays resolvable for the rest of the enclosing scope, per
  /// spec ("used whenever or wherever in the code"). Deliberately does
  /// NOT run the redeclaration check `var` itself uses: writing several
  /// sequential `catch {...} as e` blocks reusing the same name is the
  /// expected idiom (each rebinds `e` to its own error), not an
  /// error.
  fn compile_catch(
    &mut self,
    body: &Stmt,
    error_block: &Option<Box<Stmt>>,
    var_expr: &Option<Box<Expr>>,
  ) {
    let var_reg = var_expr.as_ref().map(|e| {
      let name = match e.as_ref() {
        Expr::Identifier(token) => Self::identifier_name(token),
        other => panic!(
          "compile: catch variable must be an identifier, got {:?}",
          other
        ),
      };
      let reg = self.alloc_reg();
      self.emit(Instr::LoadNil { dst: reg });
      let depth = self.cur().scope_depth;
      self.cur_mut().locals.push(Local {
        name: name.clone(),
        reg,
        is_const: false,
        depth,
        captured: false,
      });
      (reg, name)
    });

    let push_at = self.emit(Instr::PushCatch {
      var_reg: var_reg.as_ref().map(|(r, _)| *r),
      offset: 0,
    });

    self.compile_statement(body);
    self.emit(Instr::PopCatch);

    // Both the normal-completion fallthrough (right here) and an
    // error's direct jump (via PushCatch's own offset) converge at
    // this exact point: see Instr::PushCatch's doc comment.
    self.patch_jump(push_at);

    // REPL top-level persistence: mirrors Stmt::Var's own is_repl
    // special-case; a REPL line's register file is gone by the time
    // the NEXT line compiles (each is a fresh Chunk), so without this,
    // the caught error would be unreachable outside the exact
    // catch statement that declared it.
    if let Some((reg, name)) = &var_reg {
      if self.is_repl && self.cur().scope_depth == 0 {
        let name_val = self.heap.alloc_string_old(name.clone());
        let name_const = self.add_constant(name_val);
        self.emit(Instr::SetGlobal {
          name_const,
          src: *reg,
        });
      }
    }

    if let Some(err_stmt) = error_block {
      let (reg, _) = var_reg.expect("compile: catch error-block requires an 'as' variable");
      let mark = self.cur().next_reg;
      let nil_reg = self.alloc_reg();
      self.emit(Instr::LoadNil { dst: nil_reg });
      let cond_reg = self.alloc_reg();
      self.emit(Instr::Neq {
        dst: cond_reg,
        a: reg,
        b: nil_reg,
      });
      let skip = self.emit_jump_if_false(cond_reg);
      self.free_regs_to(mark);

      self.compile_statement(err_stmt);
      self.patch_jump(skip);
    }
  }

  /// Binds `value_reg`'s CURRENT value to `name` in the enclosing
  /// scope, exactly like `var name = <value_reg>` would if the value
  /// were already sitting in a register instead of coming from
  /// `compile_expression(initializer)`; shared by every import
  /// binding form (the default promoted binding, and each selectively
  /// imported name).
  ///
  /// Unlike `var`, this is NOT a redeclaration error if `name` is
  /// already a local in this exact scope; `import` is idempotent by
  /// name (`import .foo` twice, or the same name appearing in two
  /// `{ ... }` selective imports, must silently rebind rather than
  /// fail: see the "importing it again should not re-execute the
  /// module" case). In that case the EXISTING local's register is just
  /// repointed at the new value; `value_reg`'s register is otherwise
  /// left permanently allocated (same as any other local; reclaimed
  /// only when the enclosing scope closes), never freed here.
  fn declare_import_binding(&mut self, name: String, token: &Token, value_reg: u8, exported: bool) {
    if (self.is_repl && self.cur().scope_depth == 0) || exported {
      let name_val = self.heap.alloc_string_old(name.clone());
      let name_const = self.add_constant(name_val);
      self.emit(Instr::SetGlobal {
        name_const,
        src: value_reg,
      });
      return;
    }

    let depth = self.cur().scope_depth;

    if let Some(existing) = self
      .cur_mut()
      .locals
      .iter_mut()
      .rev()
      .take_while(|l| l.depth == depth)
      .find(|l| l.name == name)
    {
      existing.reg = value_reg;
      return;
    }

    self.cur_mut().locals.push(Local {
      name,
      reg: value_reg,
      is_const: true,
      depth,
      captured: false,
    });

    // token isn't needed anymore now that redeclaration is never an
    // error here, but kept as a parameter for a consistent call shape
    // with the rest of the compiler's binding-site helpers and in case
    // a future diagnostic (e.g. warning on a *conflicting* re-import)
    // wants it.
    let _ = token;
  }

  /// `import PATH [as NAME] [{ elements... | * }]`. Always starts with
  /// one `Instr::Import` producing the raw module in a temp register,
  /// then dispatches to whichever of the three documented forms this
  /// statement actually is:
  ///  - `{ * }`           -> `Instr::ImportAll`, no binding created.
  ///  - `{ a, b, ... }`    -> one `GetField` + `declare_import_binding`
  ///                         per requested name.
  ///  - default (neither)  -> `Instr::MakePromoted` + one
  ///                         `declare_import_binding` for NAME.
  fn compile_import(
    &mut self,
    path: &str,
    name: &Expr,
    elements: &[Expr],
    imports_all: bool,
    exported: bool,
  ) {
    let path_val = self.heap.alloc_string_old(path.to_string());
    let path_const = self.add_constant(path_val);
    let importer_val = self.heap.alloc_string_old(self.source_path.to_string());
    let importer_const = self.add_constant(importer_val);

    let mod_reg = self.alloc_reg();
    self.emit(Instr::Import {
      dst: mod_reg,
      path_const,
      importer_const,
    });

    if imports_all {
      self.emit(Instr::ImportAll {
        module: mod_reg,
        exported,
      });
      // Nothing persists past this statement; safe to reclaim.
      self.free_regs_to(mod_reg);
      return;
    }

    if !elements.is_empty() {
      for element in elements {
        let Expr::Identifier(token) = element else {
          panic!(
            "compile: import element is not an identifier: {:?}",
            element
          );
        };
        let field_name = Self::identifier_name(token);
        let dst = self.alloc_reg();
        let fname_val = self.heap.alloc_string_old(field_name.clone());
        let fname_const = self.add_constant(fname_val);
        self.cur_mut().current_line = token.line as u32;
        self.emit(Instr::GetField {
          dst,
          obj: mod_reg,
          name_const: fname_const,
        });
        self.declare_import_binding(field_name, token, dst, exported);
      }
      return;
    }

    let Expr::Identifier(name_token) = name else {
      panic!(
        "compile: import binding name is not an identifier: {:?}",
        name
      );
    };
    let bind_name = Self::identifier_name(name_token);
    let name_val = self.heap.alloc_string_old(bind_name.clone());
    let name_const = self.add_constant(name_val);

    let dst = self.alloc_reg();
    self.emit(Instr::MakePromoted {
      dst,
      module: mod_reg,
      name_const,
    });
    self.declare_import_binding(bind_name, name_token, dst, exported);
  }

  fn compile_statement(&mut self, statement: &Stmt) {
    match statement {
      Stmt::Expression(expression) => {
        let mark = self.cur().next_reg;

        // REPL auto-echo: a bare expression statement typed directly
        // at the top level of a REPL line prints its own result --
        // unless that result is `nil`, matching every native/builtin
        // that signals "no meaningful return value" by yielding nil
        // (echoing that on every `a.append(x)`-style call would be
        // pure noise). Nested inside a block (an `if`/`while`/function
        // body typed at the REPL) is deliberately excluded; only a
        // truly top-level statement gets this treatment, so loop
        // bodies don't print once per iteration.
        if self.is_repl && self.cur().scope_depth == 0 {
          let val_reg = self.compile_expression(expression);

          let nil_reg = self.alloc_reg();
          self.emit(Instr::LoadNil { dst: nil_reg });
          let cond_reg = self.alloc_reg();
          self.emit(Instr::Neq {
            dst: cond_reg,
            a: val_reg,
            b: nil_reg,
          });
          let skip = self.emit_jump_if_false(cond_reg);
          self.emit(Instr::Print { src: val_reg });
          self.patch_jump(skip);
        } else {
          self.compile_expression(expression);
        }

        self.free_regs_to(mark);
      },
      Stmt::Echo(value) => {
        let mark = self.cur().next_reg;
        let reg = self.compile_expression(value);
        self.emit(Instr::Print { src: reg });
        self.free_regs_to(mark);
      },
      Stmt::Block(statements) => {
        let mark = self.cur().next_reg;
        let locals_mark = self.cur().locals.len();
        self.cur_mut().scope_depth += 1;

        for stmt in statements {
          self.compile_statement(stmt);
        }

        self.cur_mut().scope_depth -= 1;
        let any_captured = self.cur().locals[locals_mark..].iter().any(|l| l.captured);
        self.cur_mut().locals.truncate(locals_mark);
        if any_captured {
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
            // Skip the usual "hop over `else`" jump when `then_branch`
            // can never fall through to it in the first place (see
            // `Self::stmt_never_falls_through`'s own docs); not just
            // an optimization: with nothing forcing the compiler to pad
            // the chunk with an implicit trailing instruction once its
            // real last one already exits some other way, patching a
            // dead jump to "wherever the if/else ends" can land one
            // past the very last instruction in the whole chunk, which
            // the JIT has nothing valid to compile a jump target into.
            let else_jump = if Self::stmt_never_falls_through(then_branch) {
              None
            } else {
              Some(self.emit_jump())
            };
            self.patch_jump(then_jump);
            self.compile_statement(else_stmt);
            if let Some(else_jump) = else_jump {
              self.patch_jump(else_jump);
            }
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

        self.cur_mut().loops.push(LoopContext {
          continue_target: loop_start,
          break_jumps: Vec::new(),
          body_mark: mark,
          continue_locales: Vec::new(),
        });

        self.compile_statement(body);
        self.emit_loop(loop_start);

        self.patch_jump(exit_jump);

        let finished_loop = self.cur_mut().loops.pop().unwrap();
        let after_loop = self.cur().chunk.code.len();
        for jump_at in finished_loop.break_jumps {
          self.patch_jump(jump_at);
        }
        debug_assert_eq!(
          self.cur().chunk.code.len(),
          after_loop,
          "patching break jumps should not emit new code"
        );
      },
      Stmt::Break => {
        if self.cur().loops.is_empty() {
          self.report_error_here("'break' used outside of a loop".to_string());
        } else {
          let body_mark = self.cur().loops.last().unwrap().body_mark;
          if self.locals_captured_from(body_mark) {
            self.emit(Instr::CloseUpvalues { from: body_mark });
          }
          let jump_at = self.emit_jump();
          self
            .cur_mut()
            .loops
            .last_mut()
            .unwrap()
            .break_jumps
            .push(jump_at);
        }
      },
      Stmt::Continue => {
        if self.cur().loops.is_empty() {
          self.report_error_here("'continue' used outside of a loop".to_string());
        } else {
          let (body_mark, continue_target) = {
            let loop_ctx = self.cur().loops.last().unwrap();
            (loop_ctx.body_mark, loop_ctx.continue_target)
          };
          if self.locals_captured_from(body_mark) {
            self.emit(Instr::CloseUpvalues { from: body_mark });
          }
          let continue_location = self.emit_loop(continue_target);
          self
            .cur_mut()
            .loops
            .last_mut()
            .unwrap()
            .continue_locales
            .push(continue_location);
        }
      },
      Stmt::FixContinue => {
        let locales = self
          .cur_mut()
          .loops
          .last()
          .unwrap()
          .continue_locales
          .clone()
          .iter()
          .for_each(|f| self.patch_jump(*f));
      },
      Stmt::Decl(decl) => self.compile_declaration(decl),
      Stmt::VarList(list) => {
        for item in list {
          self.compile_statement(item);
        }
      },
      Stmt::Var(token, initializer, _type_hint, is_const) => {
        let name = Self::identifier_name(token);

        if self.at_module_top_level() {
          let src = self.compile_expression(initializer);

          let name_val = self.heap.alloc_string_old(name.clone());
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
            self.report_error(
              format!("'{}' is already declared in this scope", name),
              token,
            );
          }

          let mark = self.cur().next_reg;
          let raw_reg = self.compile_expression(initializer);
          // If the initializer resolved to a PRE-EXISTING register (e.g. a
          // bare read of another local, like `var t = n`), adopting that
          // register directly as this new local's home would make the two
          // names ALIAS the same physical storage; any future write to
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
            captured: false,
          });
        }
      },
      Stmt::Return(value) => {
        let reg = self.compile_expression(value);
        self.emit(Instr::Return { src: reg });
      },
      Stmt::Using(subject, labels, bodies, default) => {
        self.compile_using(subject, labels, bodies, default)
      },
      Stmt::Raise(expr) => self.compile_raise(expr),
      Stmt::Assert(cond, message) => self.compile_assert(cond, message),
      Stmt::Catch(body, error_block, var_expr) => self.compile_catch(body, error_block, var_expr),
      Stmt::Import(path, name, elements, imports_all, exported) => self.compile_import(
        path.as_str(),
        name.as_ref(),
        elements.as_slice(),
        *imports_all,
        *exported,
      ),
      _ => {},
    };
  }

  fn compile_declaration(&mut self, declaration: &Decl) {
    match declaration {
      Decl::Stmt(statement) => self.compile_statement(statement),
      Decl::Function(token, params, body, is_variadic) => {
        self.compile_function_decl(token, params, body, *is_variadic)
      },
      Decl::Class(name, superclass, properties, methods, is_extension) => {
        self.compile_class_decl(name, superclass, properties, methods, *is_extension)
      },
      _ => {},
    };
  }

  pub fn compile(mut self) -> Result<ObjFunction, Vec<ParserError>> {
    #[cfg(feature = "ast-log")]
    if std::env::var_os("ZURI_AST_LOG").is_some() {
      println!("{:?}", self.decls.clone());
    }

    for decl in self.decls.clone().iter() {
      self.compile_declaration(decl);
    }

    if !self.errors.is_empty() {
      return Err(self.errors);
    }

    Ok(self.finalize())
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
    let jit = JitInfo::new(top.chunk.code.len());
    let main_fn = ObjFunction {
      name: if self.is_repl {
        "@.repl".to_string()
      } else {
        "@.script".to_string()
      },
      arity: 0,
      variadic: false,
      num_registers: top.max_reg,
      chunk: top.chunk,
      upvalues: Vec::new(),
      source_path: self.source_path.clone(),
      globals_module: self.module,
      is_method: false,
      owning_class_name: None,
      jit,
    };

    if self.log_instr {
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

      // Main entry function
      println!("=== function '<script>' ({} registers) ===", top.max_reg);
      for (i, c) in main_fn.chunk.code.iter().enumerate() {
        println!("{:3}: {:?}", i, c);
      }
    }

    main_fn
  }
}

/// Recursively walk a method's body collecting every field name it
/// assigns via `self.NAME = ...`; these count as "predeclared" just
/// as much as an explicit `var NAME` does, since the compiler can see
/// them by scanning the class's own AST at declaration time, well
/// before any instance exists (see `Compiler::compile_class_decl`).
/// Descends into nested `def`/`@(...)` bodies too, since `self` can be
/// captured by a closure nested inside a method.
fn collect_self_fields_stmt(stmt: &Stmt, out: &mut Vec<String>) {
  match stmt {
    Stmt::Echo(e) | Stmt::Expression(e) | Stmt::Raise(e) | Stmt::Return(e) => {
      collect_self_fields_expr(e, out)
    },
    Stmt::If(cond, then_b, else_b) => {
      collect_self_fields_expr(cond, out);
      collect_self_fields_stmt(then_b, out);
      if let Some(e) = else_b {
        collect_self_fields_stmt(e, out);
      }
    },
    Stmt::While(cond, body) => {
      collect_self_fields_expr(cond, out);
      collect_self_fields_stmt(body, out);
    },
    Stmt::Assert(e, msg) => {
      collect_self_fields_expr(e, out);
      if let Some(m) = msg {
        collect_self_fields_expr(m, out);
      }
    },
    Stmt::Using(subject, labels, bodies, default) => {
      collect_self_fields_expr(subject, out);
      for l in labels {
        collect_self_fields_expr(l, out);
      }
      for b in bodies {
        collect_self_fields_stmt(b, out);
      }
      if let Some(d) = default {
        collect_self_fields_stmt(d, out);
      }
    },
    Stmt::Catch(body, catch_body, _name) => {
      collect_self_fields_stmt(body, out);
      if let Some(cb) = catch_body {
        collect_self_fields_stmt(cb, out);
      }
    },
    Stmt::Block(stmts) => {
      for s in stmts {
        collect_self_fields_stmt(s, out);
      }
    },
    Stmt::Decl(decl) => collect_self_fields_decl(decl, out),
    Stmt::Var(_, init, _, _) => collect_self_fields_expr(init, out),
    Stmt::VarList(list) => {
      for s in list {
        collect_self_fields_stmt(s, out);
      }
    },
    Stmt::None
    | Stmt::FixContinue
    | Stmt::Continue
    | Stmt::Break
    | Stmt::Import(..)
    | Stmt::Trivia(_) => {},
  }
}

fn collect_self_fields_decl(decl: &Decl, out: &mut Vec<String>) {
  match decl {
    Decl::Stmt(s) => collect_self_fields_stmt(s, out),
    Decl::Block(stmts) => {
      for s in stmts {
        collect_self_fields_stmt(s, out);
      }
    },
    Decl::Function(_, _, body, _) => collect_self_fields_stmt(body, out),
    // Method/Class/Property/Import/None aren't expected nested inside a
    // method body; ignored defensively rather than assumed unreachable.
    _ => {},
  }
}

fn collect_self_fields_expr(expr: &Expr, out: &mut Vec<String>) {
  match expr {
    Expr::Set(target, field, value) => {
      if matches!(target.as_ref(), Expr::Self_) {
        if let TokenKind::Identifier(field_name) = &field.kind {
          if !out.contains(field_name) {
            out.push(field_name.clone());
          }
        }
      }
      collect_self_fields_expr(target, out);
      collect_self_fields_expr(value, out);
    },
    Expr::Unary(_, e, _) | Expr::Grouping(e) => collect_self_fields_expr(e, out),
    Expr::Binary(a, _, b, _)
    | Expr::Logical(a, _, b, _)
    | Expr::Circuit(a, _, b)
    | Expr::Range(a, b, _)
    | Expr::Index(a, b, _)
    | Expr::Assign(a, b) => {
      collect_self_fields_expr(a, out);
      collect_self_fields_expr(b, out);
    },
    Expr::Condition(a, b, c) | Expr::Slice(a, b, c, _) => {
      collect_self_fields_expr(a, out);
      collect_self_fields_expr(b, out);
      collect_self_fields_expr(c, out);
    },
    Expr::Call(callee, args, _) => {
      collect_self_fields_expr(callee, out);
      for a in args {
        collect_self_fields_expr(a, out);
      }
    },
    Expr::Get(obj, _) => collect_self_fields_expr(obj, out),
    Expr::List(items) => {
      for i in items {
        collect_self_fields_expr(i, out);
      }
    },
    Expr::Dict(keys, values) => {
      for k in keys {
        collect_self_fields_expr(k, out);
      }
      for v in values {
        collect_self_fields_expr(v, out);
      }
    },
    Expr::Anonymous(decl) => collect_self_fields_decl(decl, out),
    Expr::Nil
    | Expr::Bool(_)
    | Expr::Integer(_)
    | Expr::Float(_)
    | Expr::BigNumber(_)
    | Expr::Literal(_)
    | Expr::Identifier(_)
    | Expr::Parent
    | Expr::Self_
    | Expr::TypeHint(..)
    | Expr::Argument(..) => {},
  }
}

/// Recognizes exactly the AST shapes `Value::equals` treats as
/// content-comparable primitives; nil, bool, number, string; as
/// eligible for `Instr::UsingJump`'s O(1) jump table. Anything else (an
/// arbitrary runtime expression, a BigNumber literal, etc.) returns
/// None and falls back to the sequential dynamic-label path in
/// `Compiler::compile_using`; correctness is unaffected either way,
/// this only decides which path a given label's comparison takes.
fn expr_as_jump_key(expr: &Expr) -> Option<JumpKey> {
  match expr {
    Expr::Nil => Some(JumpKey::Nil),
    Expr::Bool(b) => Some(JumpKey::Bool(*b)),
    Expr::Integer(i) => Some(JumpKey::Number((*i as f64).to_bits())),
    Expr::Float(f) => Some(JumpKey::Number(f.to_bits())),
    Expr::Literal(s) => Some(JumpKey::Str(s.clone())),
    _ => None,
  }
}

/// Does `expr` name a compile-time-known numeric literal? Only
/// `Integer`/`Float`; NOT `BigNumber`, which needs its own bigint
/// arithmetic path and never participates in this fusion.
fn literal_as_f64(expr: &Expr) -> Option<f64> {
  match expr {
    Expr::Integer(i) => Some(*i as f64),
    Expr::Float(f) => Some(*f),
    _ => None,
  }
}

/// `EXPR + LITERAL` / `EXPR - LITERAL` / `EXPR * LITERAL`; collapses
/// what would otherwise be LoadConst+{Add,Sub,Mul} (two dispatches)
/// into one fused instruction with the literal embedded as a
/// constant-pool reference rather than loaded into its own register.
/// Justified directly by profiling: `LoadConst` immediately followed
/// by one of these was among the single most common instruction pairs
/// across every benchmark profiled.
fn imm_arith_ctor(op: &TokenKind) -> Option<fn(u8, u8, u16) -> Instr> {
  match op {
    TokenKind::Plus => Some(|dst, a, imm_const| Instr::AddImm { dst, a, imm_const }),
    TokenKind::Minus => Some(|dst, a, imm_const| Instr::SubImm { dst, a, imm_const }),
    TokenKind::Multiply => Some(|dst, a, imm_const| Instr::MulImm { dst, a, imm_const }),
    _ => None,
  }
}

/// Same idea as `imm_arith_ctor`, for comparisons; `n < 2`,
/// `depth <= 0`, `x == 0`, all extremely common terminal-check/loop-
/// bound patterns.
fn imm_logical_ctor(op: &TokenKind) -> Option<fn(u8, u8, u16) -> Instr> {
  match op {
    TokenKind::Less => Some(|dst, a, imm_const| Instr::LtImm { dst, a, imm_const }),
    TokenKind::LessEq => Some(|dst, a, imm_const| Instr::LeImm { dst, a, imm_const }),
    TokenKind::Greater => Some(|dst, a, imm_const| Instr::GtImm { dst, a, imm_const }),
    TokenKind::GreaterEq => Some(|dst, a, imm_const| Instr::GeImm { dst, a, imm_const }),
    TokenKind::EqualEq => Some(|dst, a, imm_const| Instr::EqImm { dst, a, imm_const }),
    TokenKind::BangEq => Some(|dst, a, imm_const| Instr::NeqImm { dst, a, imm_const }),
    _ => None,
  }
}

#[cfg(test)]
mod owning_class_name_tests {
  use super::*;
  use crate::compiler::lexer::Lexer;
  use crate::compiler::parser::Parser;

  /// Returns the compiled top-level `ObjFunction` TOGETHER WITH the
  /// `Heap` it was compiled against, and the caller must keep BOTH
  /// alive for as long as it inspects the result; the returned
  /// function's own constant pool holds raw `Value` pointers into
  /// this exact heap's backing storage (every string/nested-function
  /// constant the compiler allocates via `heap.alloc_string_old`/
  /// `heap.alloc_function`), so dropping the heap first and reading
  /// the function's constants after is a genuine dangling-pointer
  /// use-after-free.
  fn compile_source(src: &str) -> (Heap, ObjFunction) {
    let mut lex = Lexer::new(src);
    let mut parser = Parser::new(&mut lex);
    let decls = parser.parse().expect("test source must parse");
    let mut heap = Heap::new();
    let chunk = Box::new(Chunk::new());
    let compiler = Compiler::new(decls, chunk, &mut heap, Rc::from("test"));
    let result = compiler.compile().expect("test source must compile");
    (heap, result)
  }

  /// A method's `owning_class_name` must name the class it was
  /// actually declared inside; the whole point of the link (see
  /// `ObjFunction::owning_class_name`'s own docs).
  #[test]
  fn method_gets_owning_class_name() {
    let (_heap, main) = compile_source(
      r#"
      class TreeNode {
        @new(left, right) {
          self.left = left
          self.right = right
        }

        count() {
          if self.left == nil return 1
          return 1 + self.left.count() + self.right.count()
        }
      }
      "#,
    );
    let mut found_new = false;
    let mut found_count = false;
    for c in &main.chunk.constants {
      if c.is_func() {
        let f = c.as_func();
        if f.name == "@new" {
          found_new = true;
          assert_eq!(f.owning_class_name.as_deref(), Some("TreeNode"));
        }
        if f.name == "count" {
          found_count = true;
          assert_eq!(f.owning_class_name.as_deref(), Some("TreeNode"));
        }
      }
    }
    assert!(found_new, "expected to find the @new method constant");
    assert!(found_count, "expected to find the count method constant");
  }

  /// An ordinary (non-method) function must NOT get an owning class.
  #[test]
  fn plain_function_has_no_owning_class_name() {
    let (_heap, main) = compile_source("def f(n) { return n }");
    let mut found = false;
    for c in &main.chunk.constants {
      if c.is_func() {
        let f = c.as_func();
        if f.name == "f" {
          found = true;
          assert_eq!(f.owning_class_name, None);
        }
      }
    }
    assert!(found, "expected to find the 'f' function constant");
  }

  /// The field-initializer function synthesized for a class with
  /// property declarations should ALSO carry the owning class name --
  /// it's just as much "code that belongs to this class" as an
  /// explicit method.
  #[test]
  fn field_initializer_gets_owning_class_name() {
    let (_heap, main) = compile_source(
      r#"
      class Point {
        var x = 0
        var y = 0
      }
      "#,
    );
    let mut found = false;
    for c in &main.chunk.constants {
      if c.is_func() {
        let f = c.as_func();
        if f.name.contains("init_fields") {
          found = true;
          assert_eq!(f.owning_class_name.as_deref(), Some("Point"));
        }
      }
    }
    assert!(found, "expected to find the field-initializer constant");
  }
}
