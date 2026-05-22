#ifndef ZURI_BLOB_H
#define ZURI_BLOB_H

#include "common.h"
#include "value.h"

typedef enum {
  OP_DEFINE_GLOBAL,
  OP_GET_GLOBAL,
  OP_SET_GLOBAL,

  OP_GET_LOCAL,
  OP_GET_UP_VALUE,
  OP_SET_LOCAL,
  OP_SET_UP_VALUE,
  OP_CLOSE_UP_VALUE,
  OP_GET_PROPERTY,
  OP_GET_SELF_PROPERTY,
  OP_SET_PROPERTY,

  OP_JUMP_IF_FALSE,
  OP_JUMP,
  OP_LOOP,

  OP_EQUAL,
  OP_GREATER,
  OP_LESS,

  OP_EMPTY,
  OP_NIL,
  OP_TRUE,
  OP_FALSE,
  OP_ADD,
  OP_SUBTRACT,
  OP_MULTIPLY,
  OP_DIVIDE,
  OP_F_DIVIDE, // floor divide
  OP_REMINDER,
  OP_POW,
  OP_NEGATE,
  OP_NOT,
  OP_BIT_NOT,
  OP_AND,
  OP_OR,
  OP_XOR,
  OP_LSHIFT,
  OP_RSHIFT,
  OP_URSHIFT,
  OP_ONE,

  OP_CONSTANT, // 8-bit constant address (0 - 255)
  OP_ECHO,
  OP_POP,
  OP_DUP,
  OP_POP_N,
  OP_ASSERT,
  OP_RAISE,

  OP_CLOSURE,
  OP_CALL,
  OP_INVOKE,
  OP_INVOKE_SELF,
  OP_RETURN,

  OP_CLASS,
  OP_METHOD,
  OP_CLASS_PROPERTY,
  OP_INHERIT,
  OP_EXTEND,
  OP_GET_SUPER,
  OP_SUPER_INVOKE,
  OP_SUPER_INVOKE_SELF,

  OP_RANGE,
  OP_LIST,
  OP_DICT,
  OP_GET_INDEX,
  OP_GET_RANGED_INDEX,
  OP_SET_INDEX,

  OP_CALL_IMPORT,
  OP_NATIVE_MODULE,
  OP_SELECT_IMPORT,
  OP_SELECT_NATIVE_IMPORT,
  OP_IMPORT_ALL_NATIVE,
  OP_EJECT_IMPORT,
  OP_EJECT_NATIVE_IMPORT,
  OP_IMPORT_ALL,

  OP_BEGIN_CATCH,
  OP_END_CATCH,

  OP_STRINGIFY,
  OP_SWITCH,
  OP_CHOICE,

  // the break placeholder... it never gets to the vm
  // care should be taken to
  OP_BREAK_PL,
} z_code;

typedef struct {
  int count;
  int capacity;
  uint8_t *code;
  int *lines;
  z_value_arr constants;
} z_blob;

void init_blob(z_blob *blob);

void free_blob(z_vm *vm, z_blob *blob);

void write_blob(z_vm *vm, z_blob *blob, uint8_t byte, int line);

int add_constant(z_vm *vm, z_blob *blob, z_value value);

#endif