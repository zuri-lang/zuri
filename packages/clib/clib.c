#include <zuri.h>
#include <ffi.h>
#include <math.h>

#ifdef HAVE_DLFCN_H
#include <dlfcn.h>
#include <stdlib.h>

#else
#include "zdlfcn.h"
#endif

#define CLIB_RETURN_PTR(handle, u, cf, ...) \
  z_obj_ptr *ptr = (z_obj_ptr *)GC(new_closable_ptr(vm, (handle), (u))); \
  const char *format = #cf; \
  int length = snprintf(NULL, 0, format, ##__VA_ARGS__); \
  ptr->name = ALLOCATE(char, length + 1); \
  sprintf((char *)ptr->name, format, ##__VA_ARGS__); \
  ptr->name[length] = '\0';   \
  RETURN_OBJ(ptr);

typedef struct {
  int length;
  int as_int;
  int *types;
  ffi_type *as_ffi;
  z_obj_list *names;
} z_ffi_type;

#define DEFINE_CLIB_TYPE(v) \
  z_value __clib_type_##v(z_vm *vm) { \
    z_ffi_type *f = ALLOCATE(z_ffi_type, 1); \
    f->as_ffi = &ffi_type_##v;        \
    f->as_int = z_clib_type_##v;      \
    f->types = NULL;        \
    f->length = 0; \
    z_obj_ptr *ptr = (z_obj_ptr *)GC(new_ptr(vm, (void *)f)); \
    ptr->name = "<void *clib::type::" #v ">"; \
    return OBJ_VAL(ptr); \
  }

#define DEFINE_CLIB_CTYPE(v) \
  z_value __clib_type_##v(z_vm *vm) { \
    z_ffi_type *f = ALLOCATE(z_ffi_type, 1); \
    f->as_ffi = &ffi_type_pointer;        \
    f->as_int = z_clib_type_##v;      \
    f->types = NULL;         \
    f->length = 0;\
    z_obj_ptr *ptr = (z_obj_ptr *)GC(new_ptr(vm, (void *)f)); \
    ptr->name = "<void *clib::type::" #v ">"; \
    return OBJ_VAL(ptr); \
  }

#define GET_CLIB_TYPE(v) \
  {#v, true, __clib_type_##v}

typedef struct {
  bool is_variadic;
  unsigned int args_count;
  ffi_abi abi;
  ffi_cif *cif;
  z_ffi_type *return_type;
  z_ffi_type **arg_types;
  void *function;
} z_ffi_cif;

typedef struct {
  bool is_variadic;
  unsigned int args_count;
  ffi_abi abi;
  ffi_cif *cif;
  z_ffi_type *return_type;
  z_ffi_type **arg_types;
  void *code;
  void *closure;
  z_obj_closure *zuri_closure;
  z_vm *vm;
} z_ffi_cif_closure;

// forward declare
z_value get_zuri_value(z_vm *vm, int type, size_t size, void *data, size_t *read_len);

#define z_clib_type_void					  (-1)
#define z_clib_type_bool					  0
#define z_clib_type_uint8					  1
#define z_clib_type_sint8					  2
#define z_clib_type_uint16					3
#define z_clib_type_sint16					4
#define z_clib_type_uint32					5
#define z_clib_type_sint32					6
#define z_clib_type_uint64					7
#define z_clib_type_sint64					8
#define z_clib_type_float					  9
#define z_clib_type_double					10
#define z_clib_type_uchar					  11
#define z_clib_type_schar					  12
#define z_clib_type_ushort					13
#define z_clib_type_sshort					14
#define z_clib_type_uint					  15
#define z_clib_type_sint					  16
#define z_clib_type_ulong					  17
#define z_clib_type_slong					  18
#define z_clib_type_longdouble		  19
#define z_clib_type_char_ptr				20
#define z_clib_type_uchar_ptr			  21
#define z_clib_type_pointer					22
#define z_clib_type_struct					23
#if FFI_CLOSURES
#define z_clib_type_closure				  24
#endif

DEFINE_CLIB_TYPE(void);
DEFINE_CLIB_TYPE(uint8);
DEFINE_CLIB_TYPE(sint8);
DEFINE_CLIB_TYPE(uint16);
DEFINE_CLIB_TYPE(sint16);
DEFINE_CLIB_TYPE(uint32);
DEFINE_CLIB_TYPE(sint32);
DEFINE_CLIB_TYPE(uint64);
DEFINE_CLIB_TYPE(sint64);
DEFINE_CLIB_TYPE(float);
DEFINE_CLIB_TYPE(double);
DEFINE_CLIB_TYPE(uchar);
DEFINE_CLIB_TYPE(schar);
DEFINE_CLIB_TYPE(ushort);
DEFINE_CLIB_TYPE(sshort);
DEFINE_CLIB_TYPE(uint);
DEFINE_CLIB_TYPE(sint);
DEFINE_CLIB_TYPE(ulong);
DEFINE_CLIB_TYPE(slong);
DEFINE_CLIB_TYPE(longdouble);
DEFINE_CLIB_TYPE(pointer);
DEFINE_CLIB_CTYPE(char_ptr);
DEFINE_CLIB_CTYPE(uchar_ptr);
#if FFI_CLOSURES
DEFINE_CLIB_CTYPE(closure);
#endif

UNUSED z_value __clib_type_bool(z_vm *vm) {
  z_ffi_type *f = ALLOCATE(z_ffi_type, 1);
  f->as_ffi = &ffi_type_uint8;
  f->as_int = z_clib_type_bool;
  z_obj_ptr *ptr = (z_obj_ptr *)GC(new_ptr(vm, (void *)f));
  const char *format = "%s";
  const char *str = "<void *clib::type::bool>";
  ptr->name = ALLOCATE(char, 25);
  sprintf((char *)ptr->name, format, str);
  ptr->name[24] = '\0';
  return OBJ_VAL(ptr);
}

#define CLIB_GET_C_VALUE(t) { \
    t* g = N_ALLOCATE(t, size);                          \
    *(t *)g = (t) AS_NUMBER(value); \
    return g; \
  }

#define CLIB_GET_ZURI_VALUE(t, g) { \
    t v; \
    memcpy(&v, data + len, sizeof(t)); \
    len += sizeof(t); \
    zuri_value = g(v);      \
    break; \
  }

#define CLIB_GET_C_CH_VALUE(t) { \
    if(IS_STRING(value)) {\
      t* g = N_ALLOCATE(t, size);                          \
      *(t *)g = (t) AS_C_STRING(value)[0]; \
      return g;                    \
    } else {                     \
      CLIB_GET_C_VALUE(t);                             \
    }\
  }

typedef struct {
  size_t count;
  size_t capacity;
  void **values;
} z_ffi_values;

static inline void add_value(z_vm *vm, z_ffi_values *values, void *object) {
  if (object == NULL)
    return;

  if (values->capacity < values->count + 1) {
    size_t old_capacity = values->capacity;
    values->capacity = GROW_CAPACITY(values->capacity);
    values->values = GROW_ARRAY(void *, values->values, old_capacity, values->capacity);

    if (values->values == NULL) {
      fflush(stdout); // flush out anything on stdout first
      fprintf(stderr, "CLib out of memory");
      exit(1);
    }
  }

  values->values[values->count++] = object;
}

static inline int clib_type_from_zuri_value(z_value v) {
  if(IS_NIL(v)) {
    return 10000; // out of range -> void
  } else if(IS_BOOL(v)) {
    return z_clib_type_bool;
  } else if(IS_NUMBER(v)) {
    if(trunc(AS_NUMBER(v)) != AS_NUMBER(v)) {
      return z_clib_type_double;
    } else if(AS_NUMBER(v) < 0) {
      return z_clib_type_sint32;
    }
    return z_clib_type_sint32;
  } else if(IS_STRING(v)) {
    return z_clib_type_char_ptr;
  } else if(IS_BYTES(v)) {
    return z_clib_type_uchar_ptr;
  } else  {
    return z_clib_type_pointer;
  }
}

static inline void *switch_c_values(z_vm *vm, int i, z_value value, size_t size) {
  switch(i) {
    case z_clib_type_bool: {
      bool* g = N_ALLOCATE(bool, size);
      *(bool *)g = AS_BOOL(value);
      return g;
    }
    case z_clib_type_uint8: CLIB_GET_C_VALUE(uint8_t);
    case z_clib_type_sint8: CLIB_GET_C_VALUE(int8_t);
    case z_clib_type_uint16: CLIB_GET_C_VALUE(uint16_t);
    case z_clib_type_sint16: CLIB_GET_C_VALUE(int16_t);
    case z_clib_type_uint32: CLIB_GET_C_VALUE(uint32_t);
    case z_clib_type_sint32: CLIB_GET_C_VALUE(int32_t);
    case z_clib_type_uint64: CLIB_GET_C_VALUE(uint64_t);
    case z_clib_type_sint64: CLIB_GET_C_VALUE(int64_t);
    case z_clib_type_float: CLIB_GET_C_VALUE(float);
    case z_clib_type_double: CLIB_GET_C_VALUE(double);
    case z_clib_type_uchar: CLIB_GET_C_CH_VALUE(unsigned char);
    case z_clib_type_schar: CLIB_GET_C_CH_VALUE(char);
    case z_clib_type_ushort: CLIB_GET_C_VALUE(unsigned short);
    case z_clib_type_sshort: CLIB_GET_C_VALUE(short);
    case z_clib_type_uint: CLIB_GET_C_VALUE(unsigned int);
    case z_clib_type_sint: CLIB_GET_C_VALUE(int);
    case z_clib_type_ulong: CLIB_GET_C_VALUE(unsigned long);
    case z_clib_type_slong: CLIB_GET_C_VALUE(long);
#ifdef LONG_LONG_MAX
    case z_clib_type_longdouble: CLIB_GET_C_VALUE(long long);
#else
    case z_clib_type_longdouble: CLIB_GET_C_VALUE(long);
#endif
    case z_clib_type_char_ptr: {
      char **v = N_ALLOCATE(char *, 1);
      if(IS_STRING(value)) {
        v[0] = AS_C_STRING(value);
      } else {
        v[0] = NULL;
      }
      return v;
    }
    case z_clib_type_uchar_ptr: {
      unsigned char **v = N_ALLOCATE(unsigned char *, size);
      if(IS_BYTES(value)) {
        v[0] = AS_BYTES(value)->bytes.bytes;
      } else {
        v[0] = NULL;
      }
      return v;
    }
    case z_clib_type_pointer: {
      void **v = N_ALLOCATE(void *, size);
      if(IS_PTR(value)) {
        v[0] = AS_PTR(value)->pointer;
      } else if(IS_FILE(value)) {
        v[0] = AS_FILE(value)->file;
      } else if(IS_BYTES(value)) {
        v[0] = AS_BYTES(value)->bytes.bytes;
      } else if(IS_LIST(value)) {
        z_obj_list *list = AS_LIST(value);
        for(int i = 0; i < list->items.count; i++) {
          v[i] = switch_c_values(
            vm,
            clib_type_from_zuri_value(list->items.values[i]),
            list->items.values[i],
            size / list->items.count // in C, items in an array have equal sizes
          );
        }
        v[0] = AS_BYTES(value)->bytes.bytes;
      } else if(IS_NIL(value)) {
        v[0] = NULL;
      } else {
        v[0] = &value;
      }
      return v;
    }
    case z_clib_type_struct: {
      if(IS_BYTES(value)) {
        z_obj_bytes *bytes = AS_BYTES(value);
        void *v = N_ALLOCATE(void, bytes->bytes.count);
        memcpy(v, bytes->bytes.bytes, bytes->bytes.count);
        return v;
      }
      return 0;
    }
#if FFI_CLOSURES
    case z_clib_type_closure: {
      void **v = N_ALLOCATE(void *, size);

      if(IS_PTR(value)) {
        z_ffi_cif_closure *closure = (z_ffi_cif_closure *)AS_PTR(value)->pointer;
        if(!closure || !closure->code) {
          v[0] = NULL;
        } else {
          v[0] = closure->code;
        }
      } else {
        v[0] = NULL;
      }

      return v;
    }
#endif
    default: {
      break;
    }
  }

  return 0;
}

static inline z_ffi_values *get_c_values(z_vm *vm, z_ffi_cif *cif, z_obj_list *list) {
  z_ffi_values *values = ALLOCATE(z_ffi_values, 1);
  values->count = 0;
  values->capacity = 0;
  values->values = NULL;

  for(int i = 0; i < list->items.count; i++) {
    z_ffi_type *type = cif->arg_types[i];
    z_value value = list->items.values[i];

    void *v = switch_c_values(vm, type->as_int, value, type->as_ffi->size);
    add_value(vm, values, v);
  }

  return values;
}

void clib_load_library_free_fn(void *data) {
  if (data != NULL) {
    dlclose(data);
  }
}

void clib_new_struct_free_fn(void *data) {
  if (data != NULL) {
    z_ffi_type *type = (z_ffi_type *)data;
    if (type != NULL) {
      // make name list available for freeing.
      type->names->obj.stale--;
    }
  }
}

void clib_new_closure_free_fn(void *data) {
  if (data != NULL) {
    z_ffi_cif_closure *ci = (z_ffi_cif_closure *)data;
    if (ci != NULL && ci->closure != NULL) {
      ffi_closure_free(ci->closure);
    }
  }
}

DECLARE_MODULE_METHOD(clib_load_library) {
  ENFORCE_ARG_COUNT(load_library, 1);
  ENFORCE_ARG_TYPE(load_library, 0, IS_STRING);

  z_obj_string *name = AS_STRING(args[0]);

  void *handle;
  if((handle = dlopen(name->chars, RTLD_LAZY)) == NULL) {
    char *error = (char *)dlerror();
    RETURN_ERROR(error);
  }

  CLIB_RETURN_PTR(handle, &clib_load_library_free_fn, <void *clib::Library(%s)>, name->chars);
}

DECLARE_MODULE_METHOD(clib_get_function) {
  ENFORCE_ARG_COUNT(get_function, 2);
  ENFORCE_ARG_TYPE(get_function, 0, IS_PTR);
  ENFORCE_ARG_TYPE(get_function, 1, IS_STRING);

  void *handle = AS_PTR(args[0])->pointer;
  z_obj_string *name = AS_STRING(args[1]);

  if(handle) {
    void* fn = dlsym(handle, name->chars);
    if(fn == NULL) {
      char *error = (char *)dlerror();
      RETURN_ERROR(error);
    }

    CLIB_RETURN_PTR(fn, NULL, <void *clib::function(%s)>, name->chars);
  }

  RETURN_ARGUMENT_ERROR("handle not initialized");
}

DECLARE_MODULE_METHOD(clib_close_library) {
  ENFORCE_ARG_COUNT(close_library, 1);
  ENFORCE_ARG_TYPE(close_library, 0, IS_PTR);

  void *handle = AS_PTR(args[0])->pointer;

  if(handle) {
    dlclose(handle);
    handle = NULL;
    RETURN;
  }

  RETURN_ARGUMENT_ERROR("handle not initialized");
}

DECLARE_MODULE_METHOD(clib_new_struct) {
  ENFORCE_ARG_RANGE(new_struct, 1, 2);
  ENFORCE_ARG_TYPE(new_struct, 0, IS_LIST);

  z_obj_list *args_list = AS_LIST(args[0]);
  z_obj_list *names = NULL;
  if(arg_count == 2) {
    ENFORCE_ARG_TYPE(new_struct, 1, IS_LIST);
    names = AS_LIST(args[1]);
    names->obj.stale++;
  }

  ffi_type *type = ALLOCATE(ffi_type, 1);
  ffi_type **elements = ALLOCATE(ffi_type *, args_list->items.count + 1);
  int *clib_types = ALLOCATE(int, args_list->items.count);

  for(int i = 0; i < args_list->items.count; i++) {
    z_ffi_type *t = AS_PTR(args_list->items.values[i])->pointer;
    clib_types[i] = t->as_int;
    elements[i] = t->as_ffi;
  }
  elements[args_list->items.count] = NULL;

  type->size = type->alignment = 0;
  type->type = FFI_TYPE_STRUCT;
  type->elements = elements;

  size_t sizes[args_list->items.count];
  ffi_get_struct_offsets(FFI_DEFAULT_ABI, type, sizes);

  z_ffi_type *struct_type = ALLOCATE(z_ffi_type, 1);
  struct_type->as_int = z_clib_type_struct;
  struct_type->as_ffi = type;
  struct_type->types = clib_types;
  struct_type->length = args_list->items.count;
  struct_type->names = names;

  CLIB_RETURN_PTR(struct_type, &clib_new_struct_free_fn, <void *clib::struct(%d)>, args_list->items.count);
}

void clib_closure_interface_trampoline(ffi_cif *cif, void *ret, void *args[], void *data) {
  z_ffi_cif_closure *ci = (z_ffi_cif_closure *)data;

  if(ci) {
    z_vm *vm = ci->vm;

    int i = 0;
    size_t read_len = 0;
    z_obj_list *zuri_args = (z_obj_list *)GC(new_list(vm));

    while(i < ci->args_count) {
      z_ffi_type *type = ci->arg_types[i];

      z_value zuri_value = get_zuri_value(
        vm,
        type->as_int,
        type->as_ffi->size,
        args,
        &read_len
      );

      write_list(vm, zuri_args, zuri_value);
      i++;
    }

    z_value return_value = call_closure(vm, ci->zuri_closure, zuri_args);
    (*(void **)ret) = switch_c_values(vm, 0, return_value, ci->return_type->as_ffi->size);
  }
}

DECLARE_MODULE_METHOD(clib_new_closure) {
  ENFORCE_ARG_COUNT(new_closure, 3);
  ENFORCE_ARG_TYPE(new_closure, 0, IS_CLOSURE);
  ENFORCE_ARG_TYPE(new_closure, 1, IS_PTR);
  ENFORCE_ARG_TYPE(new_closure, 2, IS_LIST);

  z_obj_closure *zuri_closure = AS_CLOSURE(args[0]);
  z_ffi_type *return_type = (z_ffi_type *)AS_PTR(args[1])->pointer;
  z_obj_list *args_list = AS_LIST(args[2]);

  if(return_type) {
    ffi_cif *cif = ALLOCATE(ffi_cif, 1);
    z_ffi_cif_closure *ci = ALLOCATE(z_ffi_cif_closure, 1);

    void *code;
    ffi_closure *closure = ffi_closure_alloc(sizeof(ffi_closure), &code);
    if(!closure) {
      FREE_ARRAY(ffi_cif, cif, 1);
      FREE_ARRAY(z_ffi_cif_closure, ci, 1);
      RETURN_ERROR("failed to allocate closure");
    }

    ci->vm = vm;
    ci->closure = closure;
    ci->args_count = args_list->items.count;
    ci->abi = FFI_DEFAULT_ABI;
    ci->return_type = return_type;
    ci->is_variadic = false;
    ci->cif = cif;
    ci->code = code;
    ci->zuri_closure = zuri_closure;

    // populate the argument types...
    ci->arg_types = ALLOCATE(z_ffi_type *, args_list->items.count);
    ffi_type **types = ALLOCATE(ffi_type *, args_list->items.count);

    // extract types out of z_ffi_type to ffi_type and into ci
    for (int i = 0; i < args_list->items.count; i++) {
      z_ffi_type *type = (z_ffi_type *) AS_PTR(args_list->items.values[i])->pointer;
      ci->arg_types[i] = type;
      types[i] = type->as_ffi;
    }

    if(ffi_prep_cif(ci->cif, ci->abi, ci->args_count, ci->return_type->as_ffi, types) == FFI_OK) {

      if(ffi_prep_closure_loc(ci->closure, ci->cif, clib_closure_interface_trampoline, (void *)ci, ci->code) == FFI_OK) {
        CLIB_RETURN_PTR(ci, &clib_new_closure_free_fn, <void *clib::cif::closure(*%d)(%d)>, ci->return_type->as_int, ci->args_count);
      }
    }

    FREE_ARRAY(ffi_cif, cif, 1);
    FREE_ARRAY(z_ffi_cif_closure, ci, 1);
    ffi_closure_free(closure);
    RETURN_ERROR("failed to initialize closure interface");
  }

  RETURN_ARGUMENT_ERROR("closure return type missing");
}

DECLARE_MODULE_METHOD(clib_new) {
  ENFORCE_ARG_COUNT(new, 2);
  ENFORCE_ARG_TYPE(new, 0, IS_PTR);
  ENFORCE_ARG_TYPE(new, 1, IS_LIST);

  z_ffi_type *type = (z_ffi_type *)AS_PTR(args[0])->pointer;
  z_obj_list *values = AS_LIST(args[1]);

  unsigned char* data = ALLOCATE(unsigned char, type->as_ffi->size);
  size_t write_length = 0;
  if(type->as_ffi->elements != NULL) {
    for (int i = 0; i < type->length; i++) {
      void *ret = switch_c_values(vm, type->types[i], values->items.values[i], type->as_ffi->elements[i]->size);
      memcpy(data + write_length, ret, type->as_ffi->elements[i]->size);
      write_length += type->as_ffi->elements[i]->size;
    }
  } else {
    void *ret = switch_c_values(vm, type->as_int, values->items.values[0], type->as_ffi->size);
    memcpy(data + write_length, ret, type->as_ffi->size);
  }

  RETURN_OBJ(take_bytes(vm, data, type->as_ffi->size));
}

z_value get_zuri_value(z_vm *vm, int type, size_t size, void *data, size_t *read_len) {
  size_t len = *read_len;

  z_value zuri_value;
  switch(type) {
    case z_clib_type_bool: CLIB_GET_ZURI_VALUE(bool, BOOL_VAL);
    case z_clib_type_uint8: CLIB_GET_ZURI_VALUE(uint8_t, NUMBER_VAL);
    case z_clib_type_sint8: CLIB_GET_ZURI_VALUE(int8_t, NUMBER_VAL);
    case z_clib_type_uint16: CLIB_GET_ZURI_VALUE(uint16_t, NUMBER_VAL);
    case z_clib_type_sint16: CLIB_GET_ZURI_VALUE(int16_t, NUMBER_VAL);
    case z_clib_type_uint32: CLIB_GET_ZURI_VALUE(uint32_t, NUMBER_VAL);
    case z_clib_type_sint32: CLIB_GET_ZURI_VALUE(int32_t, NUMBER_VAL);
    case z_clib_type_uint64: CLIB_GET_ZURI_VALUE(uint64_t, NUMBER_VAL);
    case z_clib_type_sint64: CLIB_GET_ZURI_VALUE(int64_t, NUMBER_VAL);
    case z_clib_type_float: CLIB_GET_ZURI_VALUE(float, NUMBER_VAL);
    case z_clib_type_double: CLIB_GET_ZURI_VALUE(double, NUMBER_VAL);
    case z_clib_type_uchar: CLIB_GET_ZURI_VALUE(unsigned char, NUMBER_VAL);
    case z_clib_type_schar: CLIB_GET_ZURI_VALUE(char, NUMBER_VAL);
    case z_clib_type_ushort: CLIB_GET_ZURI_VALUE(unsigned short, NUMBER_VAL);
    case z_clib_type_sshort: CLIB_GET_ZURI_VALUE(short, NUMBER_VAL);
    case z_clib_type_uint: CLIB_GET_ZURI_VALUE(unsigned int, NUMBER_VAL);
    case z_clib_type_sint: CLIB_GET_ZURI_VALUE(int, NUMBER_VAL);
    case z_clib_type_ulong: CLIB_GET_ZURI_VALUE(unsigned long, NUMBER_VAL);
    case z_clib_type_slong: CLIB_GET_ZURI_VALUE(long, NUMBER_VAL);
#ifdef LONG_LONG_MAX
    case z_clib_type_longdouble: CLIB_GET_ZURI_VALUE(long long, NUMBER_VAL);
#else
    case z_clib_type_longdouble: CLIB_GET_ZURI_VALUE(long, NUMBER_VAL);
#endif
    case z_clib_type_char_ptr: {
      char **v;
      memcpy(&v, data + len, sizeof(char *));

      char* string = *v;
      int length = strlen(string);

      len += size;
      zuri_value = STRING_L_VAL(string, length);
      break;
    }
    case z_clib_type_pointer: CLIB_GET_ZURI_VALUE(void *, PTR_VAL);
    case z_clib_type_uchar_ptr:
    case z_clib_type_struct: {
      unsigned char * result = ALLOCATE(unsigned char, size);
      memcpy(result, data + len, size);
      zuri_value = OBJ_VAL(take_bytes(vm, result, size));
      len += size;
      break;
    }
#if FFI_CLOSURES
    case z_clib_type_closure: {
      z_ffi_cif_closure *v = ALLOCATE(z_ffi_cif_closure, size);
      memcpy(v, data + len, sizeof(z_ffi_cif_closure));
      len += sizeof(z_ffi_cif_closure);
      zuri_value = OBJ_VAL(v->zuri_closure);
      break;
    }
#endif
    default: {
      zuri_value = NIL_VAL;
      len += size;
      break;
    }
  }

  *read_len = len;
  return zuri_value;
}

DECLARE_MODULE_METHOD(clib_get) {
  ENFORCE_ARG_COUNT(get, 2);
  ENFORCE_ARG_TYPE(get, 0, IS_PTR);
  ENFORCE_ARG_TYPES(get, 1, IS_BYTES, IS_PTR);

  z_ffi_type *type = (z_ffi_type *)AS_PTR(args[0])->pointer;
  if(type->as_ffi->elements == NULL) {
    RETURN_ARGUMENT_ERROR("get can only be used on derived types such as struct, union and arrays.");
  }

  unsigned char *data;
  if(IS_PTR(args[1])) {
    data = (unsigned char *)AS_PTR(args[1])->pointer;
  } else {
    data = AS_BYTES(args[1])->bytes.bytes;
  }

  z_obj_list *list = (z_obj_list *)GC(new_list(vm));

  size_t read_len = 0;
  for(int i = 0; i < type->length; i++) {
    z_value zuri_value = get_zuri_value(
      vm,
      type->types[i],
      type->as_ffi->elements[i]->size,
      data,
      &read_len
    );

    write_list(vm, list, zuri_value);
  }

  if(type->names != NULL) {
    z_obj_dict *dict = (z_obj_dict *)GC(new_dict(vm));
    for(int i = 0; i < type->length; i++) {
      dict_add_entry(vm, dict, type->names->items.values[i], list->items.values[i]);
    }

    RETURN_OBJ(dict);
  }

  RETURN_OBJ(list);
}

DECLARE_MODULE_METHOD(clib_get_value) {
  ENFORCE_ARG_COUNT(get_value, 2);
  ENFORCE_ARG_TYPE(get_value, 0, IS_PTR);
  ENFORCE_ARG_TYPES(get_value, 1, IS_BYTES, IS_PTR);

  z_ffi_type *type = (z_ffi_type *)AS_PTR(args[0])->pointer;
  if(type->as_ffi->elements != NULL) {
    RETURN_ARGUMENT_ERROR("get_value can only be used on non-derived types such as struct, union and arrays.");
  }

  unsigned char *data;
  if(IS_PTR(args[1])) {
    data = (unsigned char *)AS_PTR(args[1])->pointer;
  } else {
    data = AS_BYTES(args[1])->bytes.bytes;
  }

  z_obj_list *list = (z_obj_list *)GC(new_list(vm));

  size_t read_len = 0;
  RETURN_VALUE(get_zuri_value(
    vm,
    type->as_int,
    type->as_ffi->size,
    data,
    &read_len
  ));
}

DECLARE_MODULE_METHOD(clib_define) {
  ENFORCE_ARG_COUNT(define, 4);
  ENFORCE_ARG_TYPE(define, 0, IS_PTR);
  ENFORCE_ARG_TYPE(define, 1, IS_STRING);
  ENFORCE_ARG_TYPE(define, 2, IS_PTR);
  ENFORCE_ARG_TYPE(define, 3, IS_LIST);

  void *function = AS_PTR(args[0])->pointer;
  z_obj_string *fn_name = AS_STRING(args[1]);
  z_ffi_type *return_type = (z_ffi_type *)AS_PTR(args[2])->pointer;
  z_obj_list *args_list = AS_LIST(args[3]);

  if(function) {
    ffi_cif *cif = ALLOCATE(ffi_cif, 1);

    z_ffi_cif *ci = ALLOCATE(z_ffi_cif, 1);
    ci->function = function;
    ci->args_count = args_list->items.count;
    ci->abi = FFI_DEFAULT_ABI;
    ci->return_type = return_type;
    ci->is_variadic = false;
    ci->cif = cif;

    // populate the argument types...
    ci->arg_types = ALLOCATE(z_ffi_type *, args_list->items.count);
    ffi_type **types = ALLOCATE(ffi_type *, args_list->items.count + 1);

    // extract types out of z_ffi_type to ffi_type and into ci
    for (int i = 0; i < args_list->items.count; i++) {
      z_ffi_type *type = (z_ffi_type *) AS_PTR(args_list->items.values[i])->pointer;
      ci->arg_types[i] = type;
      types[i] = type->as_ffi;
    }
    types[args_list->items.count] = NULL;

    if(ffi_prep_cif(ci->cif, ci->abi, ci->args_count, ci->return_type->as_ffi, types) == FFI_OK) {
      CLIB_RETURN_PTR(ci, NULL, <void *clib::cif::%s(%d)>, fn_name->chars, ci->return_type->as_int);
    }

    FREE_ARRAY(ffi_cif, cif, 1);
    FREE_ARRAY(z_ffi_cif, ci, 1);
    RETURN_ERROR("failed to initialize call interface to %s()", fn_name->chars);
  }

  RETURN_VALUE_ERROR("invalid function handle to %s()", fn_name->chars);
}

#define CLIB_CALL(t, r) {\
    t rc; \
    ffi_call(handle->cif, handle->function, &rc, values->values); \
    RETURN_##r(rc);                 \
  }

#define CLIB_STRUCT_VALUE(t, o) { \
    t v = *((t *)(data + total_size)); \
    write_list(vm, list, o(v));   \
    total_size += el->size > 0 ? el->size : sizeof(t); \
    break; \
  }

DECLARE_MODULE_METHOD(clib_call) {
  ENFORCE_ARG_COUNT(call, 2);
  ENFORCE_ARG_TYPE(call, 0, IS_PTR);
  ENFORCE_ARG_TYPE(call, 1, IS_LIST);

  z_ffi_cif *handle = (z_ffi_cif *)AS_PTR(args[0])->pointer;
  if(handle) {
    z_obj_list *args_list = AS_LIST(args[1]);
    if(args_list->items.count > handle->args_count && !handle->is_variadic) {
      RETURN_ARGUMENT_ERROR("invalid number of arguments");
    }

    z_ffi_values *values = get_c_values(vm, handle, args_list);

    switch (handle->return_type->as_int) {
      case z_clib_type_void: {
        ffi_call(handle->cif, handle->function, NULL, values->values);
        RETURN;
      }
      case z_clib_type_bool: {
        int b;
        ffi_call(handle->cif, handle->function, &b, values->values);
        RETURN_BOOL(b > 0);
      }
      case z_clib_type_uint8: CLIB_CALL(uint8_t, NUMBER);
      case z_clib_type_sint8: CLIB_CALL(int8_t, NUMBER);
      case z_clib_type_uint16: CLIB_CALL(uint16_t, NUMBER);
      case z_clib_type_sint16: CLIB_CALL(int16_t, NUMBER);
      case z_clib_type_uint32: CLIB_CALL(uint32_t, NUMBER);
      case z_clib_type_sint32: CLIB_CALL(int32_t, NUMBER);
      case z_clib_type_uint64: CLIB_CALL(uint64_t, NUMBER);
      case z_clib_type_sint64: CLIB_CALL(int64_t, NUMBER);
      case z_clib_type_float: CLIB_CALL(float, NUMBER);
      case z_clib_type_double: CLIB_CALL(double, NUMBER);
      case z_clib_type_uchar: CLIB_CALL(unsigned char, NUMBER);
      case z_clib_type_schar: CLIB_CALL(char, NUMBER);
      case z_clib_type_ushort: CLIB_CALL(unsigned short, NUMBER);
      case z_clib_type_sshort: CLIB_CALL(short, NUMBER);
      case z_clib_type_uint: CLIB_CALL(unsigned int, NUMBER);
      case z_clib_type_sint: CLIB_CALL(int, NUMBER);
      case z_clib_type_ulong: CLIB_CALL(unsigned long, NUMBER);
      case z_clib_type_slong: CLIB_CALL(long, NUMBER);
#ifdef LONG_LONG_MAX
      case z_clib_type_longdouble: CLIB_CALL(long long, NUMBER);
#else
      case z_clib_type_longdouble: CLIB_CALL(long, NUMBER);
#endif
      case z_clib_type_char_ptr: CLIB_CALL(char *, STRING);
      case z_clib_type_uchar_ptr:
      case z_clib_type_struct: {
//#ifndef __linux__
//        ffi_arg result;
//        ffi_call(handle->cif, handle->function, &result, values->values);
//        z_obj_bytes *bytes = (z_obj_bytes *)GC(new_bytes(vm, handle->cif->rtype->size));
//        memcpy(bytes->bytes.bytes, &result, handle->cif->rtype->size);
//#else
        unsigned char * result = ALLOCATE(unsigned char, handle->cif->rtype->size);
        ffi_call(handle->cif, handle->function, result, values->values);
        z_obj_bytes *bytes = (z_obj_bytes *)GC(take_bytes(vm, result, handle->cif->rtype->size));
//#endif
        RETURN_OBJ(bytes);
      }
      case z_clib_type_pointer: CLIB_CALL(void *, PTR);
      default: RETURN;
    }
  }

  RETURN_ERROR("function handle not initialized");
}

DECLARE_MODULE_METHOD(clib_get_ptr_index) {
  ENFORCE_ARG_COUNT(get_ptr_index, 3);
  ENFORCE_ARG_TYPE(get_ptr_index, 0, IS_PTR);
  ENFORCE_ARG_TYPE(get_ptr_index, 1, IS_PTR);
  ENFORCE_ARG_TYPE(get_ptr_index, 2, IS_NUMBER);

  void *ptr = AS_PTR(args[0])->pointer;
  z_ffi_type *type = (z_ffi_type *) AS_PTR(args[1])->pointer;
  unsigned int index = AS_NUMBER(args[2]);

  z_obj_bytes *bytes = (z_obj_bytes *)GC(new_bytes(vm, type->as_ffi->size));
  memcpy(bytes->bytes.bytes, ptr + (type->as_ffi->size * index), type->as_ffi->size);
  RETURN_OBJ(bytes);
}

DECLARE_MODULE_METHOD(clib_set_ptr_index) {
  ENFORCE_ARG_COUNT(set_ptr_index, 4);
  ENFORCE_ARG_TYPE(set_ptr_index, 0, IS_PTR);
  ENFORCE_ARG_TYPE(set_ptr_index, 1, IS_PTR);
  ENFORCE_ARG_TYPE(set_ptr_index, 2, IS_NUMBER);

  void *ptr = AS_PTR(args[0])->pointer;
  z_ffi_type *type = (z_ffi_type *) AS_PTR(args[1])->pointer;
  unsigned int index = AS_NUMBER(args[2]);

  void *v = switch_c_values(vm, type->as_int, args[3], type->as_ffi->size);
  memcpy(ptr + (type->as_ffi->size * index), v, type->as_ffi->size);
  RETURN;
}

CREATE_MODULE_LOADER(clib) {
  static z_field_reg module_fields[] = {
      GET_CLIB_TYPE(void),
      GET_CLIB_TYPE(bool),
      GET_CLIB_TYPE(uint8),
      GET_CLIB_TYPE(sint8),
      GET_CLIB_TYPE(uint16),
      GET_CLIB_TYPE(sint16),
      GET_CLIB_TYPE(uint32),
      GET_CLIB_TYPE(sint32),
      GET_CLIB_TYPE(uint64),
      GET_CLIB_TYPE(sint64),
      GET_CLIB_TYPE(float),
      GET_CLIB_TYPE(double),
      GET_CLIB_TYPE(uchar),
      GET_CLIB_TYPE(schar),
      GET_CLIB_TYPE(ushort),
      GET_CLIB_TYPE(sshort),
      GET_CLIB_TYPE(uint),
      GET_CLIB_TYPE(sint),
      GET_CLIB_TYPE(ulong),
      GET_CLIB_TYPE(slong),
      GET_CLIB_TYPE(longdouble),
      GET_CLIB_TYPE(pointer),
      GET_CLIB_TYPE(char_ptr),
      GET_CLIB_TYPE(uchar_ptr),
#if FFI_CLOSURES
      GET_CLIB_TYPE(closure),
#endif
      {NULL, false, NULL}
  };

  static z_func_reg module_functions[] = {
      {"new",   true,  GET_MODULE_METHOD(clib_new)},
      {"get",   true,  GET_MODULE_METHOD(clib_get)},
      {"get_value",   true,  GET_MODULE_METHOD(clib_get_value)},
      {"load",   true,  GET_MODULE_METHOD(clib_load_library)},
      {"function",   true,  GET_MODULE_METHOD(clib_get_function)},
      {"close",   true,  GET_MODULE_METHOD(clib_close_library)},
      {"define",   true,  GET_MODULE_METHOD(clib_define)},
      {"call",   true,  GET_MODULE_METHOD(clib_call)},
      {"new_struct",   true,  GET_MODULE_METHOD(clib_new_struct)},
      {"new_closure",   true,  GET_MODULE_METHOD(clib_new_closure)},
      {"get_ptr_index",   true,  GET_MODULE_METHOD(clib_get_ptr_index)},
      {"set_ptr_index",   true,  GET_MODULE_METHOD(clib_set_ptr_index)},
      {NULL,    false, NULL},
  };

  static z_module_reg module = {
      .name = "_clib",
      .fields = module_fields,
      .functions = module_functions,
      .classes = NULL,
      .preloader = NULL,
      .unloader = NULL
  };

  return &module;
}

#undef CLIB_STRUCT_VALUE
#undef DEFINE_CLIB_TYPE
#undef GET_CLIB_TYPE
#undef CLIB_CALL
#undef CLIB_GET_C_VALUE
#undef CLIB_GET_C_CH_VALUE
#undef CLIB_RETURN_PTR
