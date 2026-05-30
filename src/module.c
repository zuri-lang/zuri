#ifdef _WIN32
#undef HAVE_DIRNAME
#undef HAVE_BASENAME
#endif

#include "module.h"
#include "pathinfo.h"
#include "value.h"

#include "standard/standard.h"

#ifdef HAVE_DLFCN_H
#include <dlfcn.h>
#else
#include "zdlfcn.h"
#endif

#if !defined(HAVE_DIRENT_H) || defined(_WIN32)
#include "dirent/dirent.h"
#else
#include <dirent.h>
#include <errno.h>
#endif /* HAVE_DIRENT_H */

#include <stdlib.h>
#include <sys/stat.h>

#ifdef HAVE_UNISTD_H
#include <unistd.h>
#else
#include "zunistd.h"
#endif /* HAVE_UNISTD_H */

z_module_init modules[] = {
    GET_MODULE_LOADER(os),         //
    GET_MODULE_LOADER(io),         //
    GET_MODULE_LOADER(base64), //
    GET_MODULE_LOADER(math),     //
    GET_MODULE_LOADER(date),     //
    GET_MODULE_LOADER(socket),     //
    GET_MODULE_LOADER(reflect), //
    GET_MODULE_LOADER(process), //
    GET_MODULE_LOADER(struct), //
    GET_MODULE_LOADER(thread), //
    NULL,
};

void free_module(z_vm *vm, z_obj_module *module) {
  free_table(vm, &module->values);
  free(module->name);
  free(module->file);
  if (module->unloader != NULL && module->imported) {
    ((z_module_loader)module->unloader)(vm);
  }
  if(module->handle != NULL) {
    close_dl_module(module->handle);  // free the shared library...
  }

  module->name = NULL;
  module->file = NULL;
  module->unloader = NULL;
  module->handle = NULL;
}

bool load_module(z_vm *vm, z_module_init init_fn, char *import_name, char *source, void *handle) {
  z_module_reg *module = init_fn(vm);

  if(module != NULL) {
    z_obj_module *the_module = (z_obj_module*)GC(new_module(vm, (char *)module->name, source, NULL));
    the_module->preloader = module->preloader;
    the_module->unloader = module->unloader;

    if (module->fields != NULL) {
      for (int j = 0; module->fields[j].name != NULL; j++) {
        z_field_reg field = module->fields[j];
        z_value field_name = GC_STRING(field.name);

        z_value v = field.field_value(vm);
        push(vm, v);
        table_set(vm, &the_module->values, field_name, v);
        pop(vm);
      }
    }

    if (module->functions != NULL) {
      for (int j = 0; module->functions[j].name != NULL; j++) {
        z_func_reg func = module->functions[j];
        z_value func_name = GC_STRING(func.name);

        z_value func_real_value = OBJ_VAL(GC(new_native(vm, func.function, func.name)));
        push(vm, func_real_value);
        table_set(vm, &the_module->values, func_name, func_real_value);
        pop(vm);
      }
    }

    if (module->classes != NULL) {
      for (int j = 0; module->classes[j].name != NULL; j++) {
        z_class_reg klass_reg = module->classes[j];

        z_obj_string *class_name = (z_obj_string *)GC(copy_string(vm, klass_reg.name, (int)strlen(klass_reg.name)));

        z_obj_class *klass = (z_obj_class *)GC(new_class(vm, class_name));

        if (klass_reg.functions != NULL) {
          for (int k = 0; klass_reg.functions[k].name != NULL; k++) {

            z_func_reg func = klass_reg.functions[k];

            z_value func_name = GC_STRING(func.name);

            z_obj_native *native = (z_obj_native*)GC(new_native(vm, func.function, func.name));

            if (func.is_static) {
              native->type = TYPE_STATIC;
            } else if (strlen(func.name) > 0 && func.name[0] == '_') {
              native->type = TYPE_PRIVATE;
            }

            table_set(vm, &klass->methods, func_name, OBJ_VAL(native));
          }
        }

        if (klass_reg.fields != NULL) {
          for (int k = 0; klass_reg.fields[k].name != NULL; k++) {
            z_field_reg field = klass_reg.fields[k];
            z_value field_name = GC_STRING(field.name);

            z_value v = field.field_value(vm);
            push(vm, v);
            table_set(vm,
                      field.is_static ? &klass->static_properties
                      : &klass->properties,
                      field_name, v);
            pop(vm);
          }
        }

        table_set(vm, &the_module->values, OBJ_VAL(class_name), OBJ_VAL(klass));
      }
    }

    if(handle != NULL) {
      the_module->handle = handle;  // set handle for shared library modules
    }
    add_native_module(vm, the_module, the_module->name);

    CLEAR_GC();
    return true;
  } else {
    WARN("Error loading module: %s\n", import_name);
  }

  return false;
}

void add_native_module(z_vm *vm, z_obj_module *module, const char *as) {
  if(as != NULL) {
    module->name = strdup(as);
  }
  z_value name = STRING_VAL(module->name);
  push(vm, name);
  push(vm, OBJ_VAL(module));
  table_set(vm, &vm->modules, name, OBJ_VAL(module));
  pop_n(vm, 2);
}

void bind_user_modules(z_vm *vm, char *pkg_root) {
  if(pkg_root == NULL) return;

  DIR *dir;
  if((dir = opendir(pkg_root)) != NULL) {
    struct dirent *ent;
    while((ent = readdir(dir)) != NULL) {

      int ext_length = (int) strlen(LIBRARY_FILE_EXTENSION);

      // skip . and .. in path
      if ((strlen(ent->d_name) == 1 && ent->d_name[0] == '.') // .
        || (strlen(ent->d_name) == 2 && ent->d_name[0] == '.' && ent->d_name[1] == '.') // ..
        || strlen(ent->d_name) < ext_length + 1) {
        continue;
      }

      char *path = merge_paths(pkg_root, ent->d_name);
      if(!path) continue;

      int path_length = (int) strlen(path);

      struct stat sb;
      if(stat(path, &sb) == 0) {
        // it's not a directory
        if(S_ISDIR(sb.st_mode) < 1) {
          if(memcmp(path + (path_length - ext_length), LIBRARY_FILE_EXTENSION, ext_length) == 0) { // library file

            char *filename = get_real_file_name(path);

            int name_length = (int)strlen(filename) - ext_length;
            char *name = ALLOCATE(char, name_length + 1);
            memcpy(name, filename, name_length);
            name[name_length] = '\0';

            char* error = load_user_module(vm, path, name);
            if(error != NULL) {
              WARN("Failed not load module %s from %s. Error: %s.", name, path, error);
            }
            FREE_ARRAY(char, name, name_length + 1);
          }
        }
      }

      free(path);
    }
    closedir(dir);
  }

  CLEAR_GC();
}

void bind_native_modules(z_vm *vm) {
  for (int i = 0; modules[i] != NULL; i++) {
    load_module(vm, modules[i], NULL, strdup("<__native__>"), NULL);
  }

  bind_user_modules(vm, merge_paths(get_exe_dir(), "dist"));
  bind_user_modules(vm, merge_paths(getcwd(NULL, 0), LOCAL_PACKAGES_DIRECTORY LOCAL_EXT_DIRECTORY));

  // Initialize core...
  char *core_module = get_core_library_file_path("_core");
  if (core_module != NULL) {
    run_file(vm, core_module, true);
  }
}

char* load_user_module(z_vm *vm, const char *path, char *name) {
  int length = (int)strlen(name) + 20; // 20 == strlen("zuri_module_loader_")
  char *fn_name = ALLOCATE(char, length + 1);

  if(fn_name == NULL) {
    return "failed to load module";
  }

  sprintf(fn_name, "zuri_module_loader_%s", name);
  fn_name[length] = '\0'; // terminate the raw string

  void *handle;
  if((handle = dlopen(path, RTLD_LAZY)) == NULL) {
    FREE_ARRAY(char, fn_name, length + 1);
    return (char *)dlerror();
  }

  z_module_init fn = dlsym(handle, fn_name);
  if(fn == NULL) {
    FREE_ARRAY(char, fn_name, length + 1);
    return (char *)dlerror();
  }

  int path_length = (int)strlen(path);
  char *module_file = ALLOCATE(char, path_length + 1);
  memcpy(module_file, path, path_length);
  module_file[path_length] = '\0';

  if(!load_module(vm, fn, name, module_file, handle)) {
    FREE_ARRAY(char, fn_name, length + 1);
    FREE_ARRAY(char, module_file, path_length + 1);
    dlclose(handle);
    return "failed to call module loader";
  }

  FREE_ARRAY(char, fn_name, length + 1);
  return NULL;
}

void close_dl_module(void* handle) {
    dlclose(handle);
}
