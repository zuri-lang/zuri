#ifndef ZURI_VALUE_H
#define ZURI_VALUE_H

#include <string.h>

#include "common.h"

#ifdef __cplusplus
extern "C" {
#endif // __cplusplus

typedef struct s_obj z_obj;
typedef struct s_obj_string z_obj_string;
typedef struct s_obj_list z_obj_list;
typedef struct s_obj_module z_obj_module;
typedef struct s_vm z_vm;

typedef union {
  uint64_t bits;
  double num;
} z_double_union;

#if defined(USE_NAN_BOXING) && USE_NAN_BOXING

// binary representation = 1111111111111 i.e.
// 11 bits + 1 bit for quiet nan and another
// bit to dodge intel's QNaN Floating-Point Indefinite bit
#define QNAN ((uint64_t)0x7ffc000000000000)

#define SIGN_BIT ((uint64_t)0x8000000000000000)

#define EMPTY_TAG 0 // 00
#define NIL_TAG 1   // 01
#define FALSE_TAG 2 // 10
#define TRUE_TAG 3  // 11

typedef uint64_t z_value;

#define FALSE_VAL ((z_value)(uint64_t)(QNAN | FALSE_TAG))
#define TRUE_VAL ((z_value)(uint64_t)(QNAN | TRUE_TAG))

#define EMPTY_VAL ((z_value)(uint64_t)(QNAN | EMPTY_TAG))
#define NIL_VAL ((z_value)(uint64_t)(QNAN | NIL_TAG))
#define BOOL_VAL(v) ((v) ? TRUE_VAL : FALSE_VAL)
#define NUMBER_VAL(v) number_to_value(v)
#define INTEGER_VAL(v) integer_to_value(v)
#define LONG_VAL(v) long_to_value(v)
#define OBJ_VAL(obj) (z_value)(SIGN_BIT | QNAN | (uint64_t)(uintptr_t)(obj))

#define AS_BOOL(v) ((v) == TRUE_VAL)
#define AS_NUMBER(v) value_to_number(v)
#define AS_OBJ(v) ((z_obj *)(uintptr_t)((v) & ~(SIGN_BIT | QNAN)))

#define IS_EMPTY(v) ((v) == EMPTY_VAL)
#define IS_NIL(v) ((v) == NIL_VAL)
#define IS_BOOL(v) (((v) | 1) == TRUE_VAL)
#define IS_NUMBER(v) (((v)&QNAN) != QNAN)
#define IS_OBJ(v) (((v) & (QNAN | SIGN_BIT)) == (QNAN | SIGN_BIT))

static inline z_value number_to_value(double v) {
  z_value value;
  memcpy(&value, &v, sizeof(double));
  return value;
}

static inline z_value integer_to_value(int v) {
  z_double_union data;
  data.num = (double) v;
  return data.bits;
}

static inline z_value long_to_value(long v) {
  z_double_union data;
  data.num = (double) v;
  return data.bits;
}

static inline double value_to_number(z_value v) {
  double number;
  memcpy(&number, &v, sizeof(z_value));
  return number;
}

#else

typedef enum {
  VAL_NIL,
  VAL_BOOL,
  VAL_NUMBER,
  VAL_OBJ,
  VAL_EMPTY,
} z_val_type;

typedef struct {
  z_val_type type;
  union {
    bool boolean;
    double number;
    z_obj *obj;
  } as;
} z_value;

// promote C values to zuri value
#define EMPTY_VAL ((z_value){VAL_EMPTY, {.number = 0}})
#define NIL_VAL ((z_value){VAL_NIL, {.number = 0}})
#define TRUE_VAL ((z_value){VAL_BOOL, {.boolean = true}})
#define FALSE_VAL ((z_value){VAL_BOOL, {.boolean = false}})
#define BOOL_VAL(v) ((z_value){VAL_BOOL, {.boolean = v}})
#define NUMBER_VAL(v) ((z_value){VAL_NUMBER, {.number = v}})
#define INTEGER_VAL(v) ((z_value){VAL_NUMBER, {.number = v}})
#define LONG_VAL(v) ((z_value){VAL_NUMBER, {.number = v}})
#define OBJ_VAL(v) ((z_value){VAL_OBJ, {.obj = (z_obj *)v}})

// demote zuri values to C value
#define AS_BOOL(v) ((v).as.boolean)
#define AS_NUMBER(v) ((v).as.number)
#define AS_OBJ(v) ((v).as.obj)

// testing zuri value types
#define IS_NIL(v) ((v).type == VAL_NIL)
#define IS_BOOL(v) ((v).type == VAL_BOOL)
#define IS_NUMBER(v) ((v).type == VAL_NUMBER)
#define IS_OBJ(v) ((v).type == VAL_OBJ)
#define IS_EMPTY(v) ((v).type == VAL_EMPTY)

#endif

typedef struct {
  int capacity;
  int count;
  z_value *values;
} z_value_arr;

typedef struct {
  int count;
  unsigned char *bytes;
} z_byte_arr;

void init_value_arr(z_value_arr *array);

void free_value_arr(z_vm *vm, z_value_arr *array);

void write_value_arr(z_vm *vm, z_value_arr *array, z_value value);

void insert_value_arr(z_vm *vm, z_value_arr *array, z_value value, int index);

void print_value(z_value value);

void echo_value(z_value value);

const char *value_type(z_value value);

bool values_equal(z_value a, z_value b);
char *number_to_string(z_vm *vm, double x, int *length);

z_obj_string *value_to_string(z_vm *vm, z_value value);

void init_byte_arr(z_byte_arr *array, int length);

void take_byte_arr(z_vm *vm, z_byte_arr *array, unsigned char data, int length);

void free_byte_arr(z_vm *vm, z_byte_arr *array);

// hash
uint32_t hash_string(const char *key, int length);

uint32_t hash_value(z_value value);

void sort_values(z_value *values, int count);

z_value copy_value(z_vm *vm, z_value value);

#define EMPTY_STRING_VAL OBJ_VAL(copy_string(vm, "", 0))
#define STRING_VAL(val) OBJ_VAL(copy_string(vm, val, (int)strlen(val)))
#define STRING_L_VAL(val, l) OBJ_VAL(copy_string(vm, val, l))
#define STRING_T_VAL(val, l) OBJ_VAL(take_string(vm, val, l))
#define STRING_TT_VAL(val) OBJ_VAL(take_string(vm, val, (int)strlen(val)))
#define BYTES_VAL(val) OBJ_VAL(take_bytes(vm, (unsigned char *)(val), (int)strlen((char *)(val))))
#define PTR_VAL(val) OBJ_VAL(new_ptr(vm, val))

#ifdef __cplusplus
}
#endif // __cplusplus

#endif