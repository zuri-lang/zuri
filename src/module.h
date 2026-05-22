#ifndef ZURI_MODULE_H
#define ZURI_MODULE_H

#include "native.h"
#include "object.h"
#include "value.h"

typedef struct {
  const char *name;
  bool is_static;
  z_native_fn function;
} z_func_reg;

typedef z_value (*z_class_field)(z_vm *);

typedef void (*z_module_loader)(z_vm *);

typedef struct {
  const char *name;
  bool is_static;
  z_class_field field_value;
} z_field_reg;

typedef struct {
  const char *name;
  z_field_reg *fields;
  z_func_reg *functions;
} z_class_reg;

typedef struct {
  const char *name;
  z_field_reg *fields;
  z_func_reg *functions;
  z_class_reg *classes;
  z_module_loader preloader;
  z_module_loader unloader;
} z_module_reg;

typedef z_module_reg* (*z_module_init)(z_vm *);

#define CREATE_MODULE_LOADER(module)                                           \
  z_module_reg* zuri_module_loader_##module(z_vm *vm)
#define GET_MODULE_LOADER(module) &zuri_module_loader_##module

void bind_native_modules(z_vm *vm);
void add_native_module(z_vm *vm, z_obj_module *module, const char *as);
bool load_module(z_vm *vm, z_module_init init_fn, char *name, char *source, void *handle);
char* load_user_module(z_vm *vm, const char *path, char *name);
void close_dl_module(void* handle);
void free_module(z_vm *vm, z_obj_module *module);

#endif