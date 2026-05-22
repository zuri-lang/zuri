#include "memory.h"
#include "compiler.h"
#include "config.h"
#include "object.h"
#include "module.h"

#include <stdio.h>
#include <stdlib.h>

#if defined(DEBUG_GC) && DEBUG_GC
#include "debug.h"
#include <stdio.h>
#endif

void *c_allocate(z_vm *vm, size_t size, size_t length) {
  if (vm == NULL) {
    return NULL;
  }

  vm->bytes_allocated += length;

  if (vm->bytes_allocated > vm->next_gc) {
    if(vm->current_frame && vm->current_frame->gc_protected == 0) {
      collect_garbage(vm);
    }
  }

  if (size == 0) {
    return NULL;
  }
  void *result = calloc(size, length);

  // just in case reallocation fails... computers ain't infinite!
  if (result == NULL) {
    OUT_OF_MEMORY();
  }
  return result;
}

void *allocate(z_vm *vm, size_t size) {
  if (vm == NULL) {
    return NULL;
  }

  vm->bytes_allocated += size;

  if (vm->bytes_allocated > vm->next_gc) {
    if(vm->current_frame && vm->current_frame->gc_protected == 0) {
      collect_garbage(vm);
    }
  }

  if (size == 0) {
    return NULL;
  }
  void *result = malloc(size);

  // just in case reallocation fails... computers ain't infinite!
  if (result == NULL) {
    OUT_OF_MEMORY();
  }
  return result;
}

void *reallocate(z_vm *vm, void *pointer, size_t old_size, size_t new_size) {
  if (vm == NULL) {
    return NULL;
  }

  vm->bytes_allocated += new_size - old_size;

  if (new_size > old_size && vm->bytes_allocated > vm->next_gc) {
    if(vm->current_frame && vm->current_frame->gc_protected == 0) {
      collect_garbage(vm);
    }
  }

  if (new_size == 0) {
    free(pointer);
    return NULL;
  }
  void *result = realloc(pointer, new_size);

  // just in case reallocation fails... computers ain't infinite!
  if (result == NULL) {
    OUT_OF_MEMORY();
  }
  return result;
}

void mark_object(z_vm *vm, z_obj *object) {
  if (object == NULL)
    return;
  if (object->mark == vm->mark_value || object->vm_id != vm->id)
    return;

#if defined(DEBUG_GC) && DEBUG_GC
  printf("%p mark ", (void *)object);
  print_object(OBJ_VAL(object), false);
  printf("\n");
#endif

  object->mark = vm->mark_value;

  if (vm->gray_capacity < vm->gray_count + 1) {
    vm->gray_capacity = GROW_CAPACITY(vm->gray_capacity);
    vm->gray_stack = (z_obj **) realloc(vm->gray_stack, sizeof(z_obj *) * vm->gray_capacity);

    if (vm->gray_stack == NULL) {
      fflush(stdout); // flush out anything on stdout first
      fprintf(stderr, "GC encountered an error");
      exit(EXIT_TERMINAL);
    }
  }
  vm->gray_stack[vm->gray_count++] = object;
}

void mark_value(z_vm *vm, z_value value) {
  if (IS_OBJ(value))
    mark_object(vm, AS_OBJ(value));
}

static void mark_array(z_vm *vm, z_value_arr *array) {
  if (array == NULL || array->values == NULL) {
    return;
  }
  for (int i = 0; i < array->count; i++) {
    mark_value(vm, array->values[i]);
  }
}

void blacken_object(z_vm *vm, z_obj *object) {
#if defined(DEBUG_GC) && DEBUG_GC
  printf("%p blacken ", (void *)object);
  print_object(OBJ_VAL(object), false);
  printf("\n");
#endif

  if(object->vm_id != vm->id) return;

  switch (object->type) {
    case OBJ_MODULE: {
      z_obj_module *module = (z_obj_module *) object;
      mark_table(vm, &module->values);
      break;
    }
    case OBJ_SWITCH: {
      z_obj_switch *sw = (z_obj_switch *) object;
      mark_table(vm, &sw->table);
      break;
    }
    case OBJ_FILE: {
      z_obj_file *file = (z_obj_file *) object;
      mark_object(vm, (z_obj *) file->mode);
      mark_object(vm, (z_obj *) file->path);
      break;
    }
    case OBJ_DICT: {
      z_obj_dict *dict = (z_obj_dict *) object;
      mark_array(vm, &dict->names);
      mark_table(vm, &dict->items);
      break;
    }
    case OBJ_LIST: {
      z_obj_list *list = (z_obj_list *) object;
      mark_array(vm, &list->items);
      break;
    }

    case OBJ_BOUND_METHOD: {
      z_obj_bound *bound = (z_obj_bound *) object;
      mark_value(vm, bound->receiver);
      mark_object(vm, (z_obj*)bound->method);
      break;
    }
    case OBJ_CLASS: {
      z_obj_class *klass = (z_obj_class *) object;
      mark_object(vm, (z_obj *) klass->name);
      mark_table(vm, &klass->methods);
      mark_table(vm, &klass->properties);
      mark_table(vm, &klass->static_properties);
      mark_value(vm, klass->initializer);
      if(klass->superclass != NULL) {
        mark_object(vm, (z_obj *)klass->superclass);
      }
      break;
    }
    case OBJ_CLOSURE: {
      z_obj_closure *closure = (z_obj_closure *) object;
      mark_object(vm, (z_obj *) closure->function);
      for (int i = 0; i < closure->up_value_count; i++) {
        mark_object(vm, (z_obj *) closure->up_values[i]);
      }
      break;
    }

    case OBJ_FUNCTION: {
      z_obj_func *function = (z_obj_func *) object;
      mark_object(vm, (z_obj *) function->name);
      mark_object(vm, (z_obj *) function->module);
      mark_array(vm, &function->blob.constants);
      break;
    }
    case OBJ_INSTANCE: {
      z_obj_instance *instance = (z_obj_instance *) object;
      mark_object(vm, (z_obj *) instance->klass);
      mark_table(vm, &instance->properties);
      break;
    }

    case OBJ_UP_VALUE: {
      mark_value(vm, ((z_obj_up_value *) object)->closed);
      break;
    }

    case OBJ_BYTES:
    case OBJ_RANGE:
    case OBJ_NATIVE:
    case OBJ_PTR:
    case OBJ_STRING:
      break;
  }
}

void free_object(z_vm *vm, z_obj *object) {
#if defined(DEBUG_GC) && DEBUG_GC
  printf("%p free type %d\n", (void *)object, object->type);
#endif

  // Do not free stale objects and objects that do not belong to this vm.
  if(object->stale > 0 || object->vm_id != vm->id) return;

  switch (object->type) {
    case OBJ_MODULE: {
      z_obj_module *module = (z_obj_module *) object;
      free_module(vm, module);
      FREE(z_obj_module, object);
      break;
    }
    case OBJ_BYTES: {
      z_obj_bytes *bytes = (z_obj_bytes *) object;
      free_byte_arr(vm, &bytes->bytes);
      FREE(z_obj_bytes, object);
      break;
    }
    case OBJ_FILE: {
      z_obj_file *file = (z_obj_file *) object;
      if (!file->is_std && file->file != NULL) {
        fclose(file->file);
      }
      FREE(z_obj_file, object);
      break;
    }
    case OBJ_DICT: {
      z_obj_dict *dict = (z_obj_dict *) object;
      free_value_arr(vm, &dict->names);
      free_table(vm, &dict->items);
      FREE(z_obj_dict, object);
      break;
    }
    case OBJ_LIST: {
      z_obj_list *list = (z_obj_list *) object;
      free_value_arr(vm, &list->items);
      FREE(z_obj_list, object);
      break;
    }

    case OBJ_BOUND_METHOD: {
      // a closure may be bound to multiple instances
      // for this reason, we do not free closures when freeing bound methods
      FREE(z_obj_bound, object);
      break;
    }
    case OBJ_CLASS: {
      z_obj_class *klass = (z_obj_class *) object;
      free_table(vm, &klass->methods);
      free_table(vm, &klass->properties);
      free_table(vm, &klass->static_properties);
      // We are not freeing the initializer because it's a closure and will still be freed accordingly later.
      FREE(z_obj_class, object);
      break;
    }
    case OBJ_CLOSURE: {
      z_obj_closure *closure = (z_obj_closure *) object;
      FREE_ARRAY(z_obj_up_value *, closure->up_values, closure->up_value_count);
      // there may be multiple closures that all reference the same function
      // for this reason, we do not free functions when freeing closures
      FREE(z_obj_closure, object);
      break;
    }
    case OBJ_FUNCTION: {
      z_obj_func *function = (z_obj_func *) object;
      free_blob(vm, &function->blob);
      FREE(z_obj_func, object);
      break;
    }
    case OBJ_INSTANCE: {
      z_obj_instance *instance = (z_obj_instance *) object;
      free_table(vm, &instance->properties);
      FREE(z_obj_instance, object);
      break;
    }
    case OBJ_NATIVE: {
      FREE(z_obj_native, object);
      break;
    }
    case OBJ_UP_VALUE: {
      FREE(z_obj_up_value, object);
      break;
    }
    case OBJ_RANGE: {
      FREE(z_obj_range, object);
      break;
    }
    case OBJ_STRING: {
      z_obj_string *string = (z_obj_string *) object;
      FREE_ARRAY(char, string->chars, string->length + 1);
      FREE(z_obj_string, object);
      break;
    }

    case OBJ_SWITCH: {
      z_obj_switch *sw = (z_obj_switch *) object;
      free_table(vm, &sw->table);
      FREE(z_obj_switch, object);
      break;
    }

    case OBJ_PTR: {
      z_obj_ptr *ptr = (z_obj_ptr *) object;
      if(ptr->free_fn) {
        ptr->free_fn(ptr->pointer);
      }
      if (!ptr->name_is_static) {
        free(ptr->name);
      }
      FREE(z_obj_ptr, object);
      break;
    }

    default:
      break;
  }
}

static void mark_roots(z_vm *vm) {
  if (vm->stack != NULL && vm->stack_top != NULL) {
    for (z_value *slot = vm->stack; slot < vm->stack_top; slot++) {
      mark_value(vm, *slot);
    }
  }

  for (int i = 0; i < vm->frame_count; i++) {
    mark_object(vm, (z_obj *) vm->frames[i].closure);
  }

  for(int i = 0; i < vm->error_count; i++) {
    if (vm->errors[i] != NULL) {
      mark_value(vm, vm->errors[i]->value);
      if (vm->errors[i]->frame != NULL) {
        mark_object(vm, (z_obj *)vm->errors[i]->frame->closure);
      }
    }
  }

  for (z_obj_up_value *up_value = vm->open_up_values; up_value != NULL;
       up_value = up_value->next) {
    mark_object(vm, (z_obj *) up_value);
  }

  mark_table(vm, &vm->globals);
  mark_table(vm, &vm->modules);

  mark_table(vm, &vm->methods_string);
  mark_table(vm, &vm->methods_bytes);
  mark_table(vm, &vm->methods_file);
  mark_table(vm, &vm->methods_list);
  mark_table(vm, &vm->methods_dict);
  mark_table(vm, &vm->methods_range);

  mark_object(vm, (z_obj*)vm->exception_class);
  if (vm->current_frame != NULL) {
    mark_object(vm, (z_obj *)vm->current_frame->closure);
  }

  mark_compiler_roots(vm);
}

static void trace_references(z_vm *vm) {
  while (vm->gray_count > 0) {
    z_obj *object = vm->gray_stack[--vm->gray_count];
    blacken_object(vm, object);
  }
}

static void sweep(z_vm *vm) {
  z_obj *previous = NULL;
  z_obj *object = vm->objects;

  while (object != NULL) {
    if (object->mark == vm->mark_value) {
      previous = object;
      object = object->next;
    } else {
      z_obj *unreached = object;

      object = object->next;
      if (previous != NULL) {
        previous->next = object;
      } else {
        vm->objects = object;
      }

      if(unreached->vm_id == vm->id) {
        free_object(vm, unreached);
      }
    }
  }
}

void free_objects(z_vm *vm) {
  z_obj *object = vm->objects;
  while (object != NULL) {
    z_obj *next = object->next;
    free_object(vm, object);
    object = next;
  }

  free(vm->gray_stack);
  vm->gray_stack = NULL;
}

void free_error_stacks(z_vm *vm) {
  if (vm == NULL) {
    return;
  }

  for(int index = vm->error_count; index < ERRORS_MAX && vm->errors[index] != NULL; index++) {
    free(vm->errors[index]);
    vm->errors[index] = NULL;
  }
}

void collect_garbage(z_vm *vm) {
#if defined(DEBUG_GC) && DEBUG_GC
  printf("-- gc begins for vm %llu\n", vm->id);
  size_t before = vm->bytes_allocated;
#endif

//  REMOVE THE NEXT LINE TO DISABLE NESTED collect_garbage() POSSIBILITY!
//  vm->next_gc = vm->bytes_allocated;

  mark_roots(vm);
  trace_references(vm);
  table_remove_whites(vm, &vm->strings);
  table_remove_whites(vm, &vm->modules);
  sweep(vm);
  free_error_stacks(vm);

  vm->next_gc = vm->bytes_allocated * GC_HEAP_GROWTH_FACTOR;
  if(vm->next_gc < MINIMUM_GC_START) {
    vm->next_gc = MINIMUM_GC_START;
  }

  vm->mark_value = !vm->mark_value;

#if defined(DEBUG_GC) && DEBUG_GC
  printf("-- gc ends\n");
  printf("   collected %zu bytes (from %zu to %zu), next at %zu\n",
         before - vm->bytes_allocated, before, vm->bytes_allocated,
         vm->next_gc);
#endif
}