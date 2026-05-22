#ifndef ZURI_OBJECT_H
#define ZURI_OBJECT_H

#include "blob.h"
#include "common.h"
#include "table.h"
#include "value.h"

#include <stdio.h>

typedef enum {
  TYPE_ANONYMOUS,
  TYPE_FUNCTION,
  TYPE_METHOD,
  TYPE_INITIALIZER,
  TYPE_PRIVATE,
  TYPE_STATIC,
  TYPE_SCRIPT,
} z_func_type;

#define OBJ_TYPE(v) (AS_OBJ(v)->type)

// object type checks
#define IS_STRING(v) is_obj_type(v, OBJ_STRING)
#define IS_NATIVE(v) is_obj_type(v, OBJ_NATIVE)
#define IS_FUNCTION(v) is_obj_type(v, OBJ_FUNCTION)
#define IS_CLOSURE(v) is_obj_type(v, OBJ_CLOSURE)
#define IS_CLASS(v) is_obj_type(v, OBJ_CLASS)
#define IS_INSTANCE(v) is_obj_type(v, OBJ_INSTANCE)
#define IS_BOUND(v) is_obj_type(v, OBJ_BOUND_METHOD)

// containers
#define IS_BYTES(v) is_obj_type(v, OBJ_BYTES)
#define IS_LIST(v) is_obj_type(v, OBJ_LIST)
#define IS_DICT(v) is_obj_type(v, OBJ_DICT)
#define IS_FILE(v) is_obj_type(v, OBJ_FILE)
#define IS_RANGE(v) is_obj_type(v, OBJ_RANGE)

// promote z_value to object
#define AS_STRING(v) ((z_obj_string *)AS_OBJ(v))
#define AS_NATIVE(v) ((z_obj_native *)AS_OBJ(v))
#define AS_FUNCTION(v) ((z_obj_func *)AS_OBJ(v))
#define AS_CLOSURE(v) ((z_obj_closure *)AS_OBJ(v))
#define AS_CLASS(v) ((z_obj_class *)AS_OBJ(v))
#define AS_INSTANCE(v) ((z_obj_instance *)AS_OBJ(v))
#define AS_BOUND(v) ((z_obj_bound *)AS_OBJ(v))

// non-user objects
#define AS_SWITCH(v) ((z_obj_switch *)AS_OBJ(v))
#define IS_SWITCH(v) is_obj_type(v, OBJ_SWITCH)
#define AS_PTR(v) ((z_obj_ptr *)AS_OBJ(v))
#define IS_PTR(v) is_obj_type(v, OBJ_PTR)
#define AS_MODULE(v) ((z_obj_module *)AS_OBJ(v))
#define IS_MODULE(v) is_obj_type(v, OBJ_MODULE)

// containers
#define AS_BYTES(v) ((z_obj_bytes *)AS_OBJ(v))
#define AS_LIST(v) ((z_obj_list *)AS_OBJ(v))
#define AS_DICT(v) ((z_obj_dict *)AS_OBJ(v))
#define AS_FILE(v) ((z_obj_file *)AS_OBJ(v))
#define AS_RANGE(v) ((z_obj_range *)AS_OBJ(v))

// demote zuri value to c string
#define AS_C_STRING(v) (((z_obj_string *)AS_OBJ(v))->chars)

#define IS_CHAR(v) (IS_STRING(v) && (AS_STRING(v)->length == 1 || AS_STRING(v)->length == 0))

typedef enum {
  // containers
  OBJ_STRING,
  OBJ_RANGE,
  OBJ_LIST,
  OBJ_DICT,
  OBJ_FILE,
  OBJ_BYTES,

  // base object types
  OBJ_UP_VALUE,
  OBJ_BOUND_METHOD,
  OBJ_CLOSURE,
  OBJ_FUNCTION,
  OBJ_INSTANCE,
  OBJ_NATIVE,
  OBJ_CLASS,

  // non-user objects
  OBJ_MODULE,
  OBJ_SWITCH,
  OBJ_PTR,  // object type that can hold any C pointer
} z_obj_type;

struct s_obj {
  z_obj_type type;
  bool mark;
  int vm_id;

  // when an object is marked as stale, it means that the
  // GC will never collect this object. This can be useful
  // for library/package objects that want to reuse native
  // objects in their types/pointers. The GC cannot reach
  // them yet, so it's best for them to be kept stale.
  uint32_t stale;
  struct s_obj *next;
};

struct s_obj_string {
  z_obj obj;
  int length;
  int utf8_length;
  bool is_ascii;
  uint32_t hash;
  char *chars;
};

typedef struct z_obj_up_value {
  z_obj obj;
  z_value closed;
  z_value *location;
  struct z_obj_up_value *next;
} z_obj_up_value;

struct s_obj_module {
  z_obj obj;
  bool imported;
  z_table values;
  char *name;
  char *file;
  void *preloader;
  void *unloader;
  void *handle;
  z_obj_module *parent;
};

typedef struct {
  z_obj obj;
  z_func_type type;
  int arity;
  int up_value_count;
  bool is_variadic;
  z_blob blob;
  z_obj_string *name;
  z_obj_module *module;
} z_obj_func;

typedef struct {
  z_obj obj;
  int up_value_count;
  z_obj_func *function;
  z_obj_up_value **up_values;
} z_obj_closure;

typedef struct z_obj_class {
  z_obj obj;
  z_value initializer;
  z_table properties;
  z_table static_properties;
  z_table methods;
  z_obj_string *name;
  struct z_obj_class *superclass;
} z_obj_class;

typedef struct {
  z_obj obj;
  z_table properties;
  z_obj_class *klass;
} z_obj_instance;

typedef struct {
  z_obj obj;
  z_value receiver;
  z_obj_closure *method;
} z_obj_bound; // a bound method

typedef bool (*z_native_fn)(z_vm *, int, z_value *);
typedef void (*z_ptr_free_fn)(void *);

typedef struct z_obj_native {
  z_obj obj;
  z_func_type type;
  const char *name;
  z_native_fn function;
} z_obj_native;

struct s_obj_list {
  z_obj obj;
  z_value_arr items;
};

typedef struct {
  z_obj obj;
  int lower;
  int upper;
  int range;
  int step;
} z_obj_range;

typedef struct {
  z_obj obj;
  z_byte_arr bytes;
} z_obj_bytes;

typedef struct {
  z_obj obj;
  z_value_arr names;
  z_table items;
} z_obj_dict;

typedef struct {
  z_obj obj;
  bool is_open;
  bool is_std;
  bool is_tty;
  int number;
  FILE *file;
  z_obj_string *mode;
  z_obj_string *path;
} z_obj_file;

typedef struct {
  z_obj obj;
  int default_jump;
  int exit_jump;
  z_table table;
} z_obj_switch;

typedef struct {
  z_obj obj;
  void *pointer;
  char *name;
  z_ptr_free_fn free_fn;
  bool name_is_static;
} z_obj_ptr;

// non-user objects...
z_obj_module *new_module(z_vm *vm, char *name, char *file, z_obj_module *parent);

z_obj_switch *new_switch(z_vm *vm);
z_obj_ptr *new_ptr(z_vm *vm, void *pointer);
z_obj_ptr *new_named_ptr(z_vm *vm, void *pointer, char *name);
z_obj_ptr *new_closable_named_ptr(z_vm *vm, void *pointer, char *name, z_ptr_free_fn free_fn);
z_obj_ptr *new_closable_ptr(z_vm *vm, void *pointer, void *free_fn);

// data containers
z_obj_list *new_list(z_vm *vm);
z_obj_range *new_range(z_vm *vm, int lower, int upper);

z_obj_bytes *new_bytes(z_vm *vm, int length);

z_obj_dict *new_dict(z_vm *vm);

z_obj_file *new_file(z_vm *vm, z_obj_string *path, z_obj_string *mode);

// base objects
z_obj_bound *new_bound_method(z_vm *vm, z_value receiver, z_obj_closure *method);

z_obj_class *new_class(z_vm *vm, z_obj_string *name);

z_obj_closure *new_closure(z_vm *vm, z_obj_func *function);

z_obj_func *new_function(z_vm *vm, z_obj_module *module, z_func_type type);

z_obj_instance *new_instance(z_vm *vm, z_obj_class *klass);

z_obj_up_value *new_up_value(z_vm *vm, z_value *slot);

z_obj_native *new_native(z_vm *vm, z_native_fn function, const char *name);

z_obj_string *copy_string(z_vm *vm, const char *chars, int length);

z_obj_string *take_string(z_vm *vm, char *chars, int length);

void print_object(z_value value, bool fix_string);

const char *object_type(z_obj *object);

z_obj_string *object_to_string(z_vm *vm, z_value value);

z_obj_bytes *copy_bytes(z_vm *vm, unsigned char *b, int length);

z_obj_bytes *take_bytes(z_vm *vm, unsigned char *b, int length);

static inline bool is_obj_type(z_value v, z_obj_type t) {
  return IS_OBJ(v) && AS_OBJ(v)->type == t;
}

#define ALLOCATE_OBJ(type, obj_type)                                           \
  (type *)allocate_object(vm, sizeof(type), obj_type)

z_obj *allocate_object(z_vm *vm, size_t size, z_obj_type type);
void migrate_objects(z_vm *src, z_vm *dest);

#define ITER_TOOL_PREPARE() \
  int arity = closure->function->arity; \
  if(arity > 0) { \
    write_list(vm, call_list, NIL_VAL); \
    if(arity > 1) { \
      write_list(vm, call_list, NIL_VAL); \
      if(arity > 2) { \
        write_list(vm, call_list, METHOD_OBJECT); \
      } \
    } \
  }

#endif