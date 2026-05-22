#ifndef ZURI_COMPILER_H
#define ZURI_COMPILER_H

#include "object.h"
#include "scanner.h"
#include "vm.h"

typedef enum {
  PREC_NONE,
  PREC_ASSIGNMENT, // =, &=, |=, *=, +=, -=, /=, **=, %=, >>=, <<=, ^=, //=
  // ~=
  PREC_CONDITIONAL, // ?:
  PREC_OR,          // or
  PREC_AND,         // and
  PREC_EQUALITY,    // ==, !=
  PREC_COMPARISON,  // <, >, <=, >=
  PREC_BIT_OR,      // |
  PREC_BIT_XOR,     // ^
  PREC_BIT_AND,     // &
  PREC_SHIFT,       // <<, >>
  PREC_TERM,        // +, -
  PREC_FACTOR,      // *, /, %, **, //
  PREC_UNARY, // !, -, ~, (++, -- this two will now be treated as statements)
  PREC_CALL,  // ., ()
  PREC_RANGE,       // ..
  PREC_PRIMARY
} z_precedence;

typedef struct {
  z_token name;
  int depth;
  bool is_captured;
} z_local;

typedef struct {
  uint16_t index;
  bool is_local;
} z_up_value;

struct s_compiler {
  z_compiler *enclosing;

  // current function
  z_obj_func *function;
  z_func_type type;

  z_local locals[UINT8_COUNT];
  int local_count;
  z_up_value up_values[UINT8_COUNT];
  int scope_depth;
};

typedef struct z_class_compiler {
  struct z_class_compiler *enclosing;
  z_token name;
  bool has_superclass;
} z_class_compiler;

typedef struct {
  z_scanner *scanner;
  z_vm *vm;

  z_token current;
  z_token previous;
  bool had_error;
  bool panic_mode;
  int block_count;
  bool is_returning;
  bool repl_can_echo;
  z_class_compiler *current_class;
  const char *current_file;

  // used for tracking loops for the continue statement...
  int innermost_loop_start;
  int innermost_loop_scope_depth;
  z_obj_module *module;
} z_parser;

typedef void (*z_parse_prefix_fn)(z_parser *, bool);

typedef void (*z_parse_infix_fn)(z_parser *, z_token, bool);

typedef struct {
  z_parse_prefix_fn prefix;
  z_parse_infix_fn infix;
  z_precedence precedence;
} z_parse_rule;

z_obj_func *compile(z_vm *vm, z_obj_module *module, const char *source);

void mark_compiler_roots(z_vm *vm);

#endif