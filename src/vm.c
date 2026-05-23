#include "vm.h"
#include "common.h"
#include "compiler.h"
#include "config.h"
#include "memory.h"
#include "module.h"
#include "native.h"
#include "object.h"
#include "utf8.h"

#include "bytes.h"
#include "dict.h"
#include "file.h"
#include "list.h"
#include "zstring.h"
#include "range.h"

#include <math.h>
#include <stdarg.h>
#include <stdio.h>
#include <string.h>
#include <assert.h>

// for debugging...
#include "debug.h"

#define ERR_CANT_ASSIGN_EMPTY "empty cannot be assigned."

static inline void reset_stack(z_vm *vm) {
  vm->stack_top = vm->stack;
  vm->frame_count = 0;
  vm->error_count = 0;
  vm->open_up_values = NULL;
}

static z_value get_stack_trace(z_vm *vm) {
  char *trace = calloc(0, sizeof(char));

  if (trace != NULL) {

    for (int i = vm->frame_count - 1; i >= 0; i--) {
      z_call_frame *frame = &vm->frames[i];
      if (frame == NULL || frame->closure == NULL || frame->closure->function == NULL) {
        continue;
      }
      z_obj_func *function = frame->closure->function;

      // -1 because the IP is sitting on the next instruction to be executed
      size_t instruction = frame->ip - function->blob.code - 1;
      int line = function->blob.lines[instruction];

      const char *trace_format = i != 0
          ? "    %s:%d -> %s()\n"
          : "    %s:%d -> %s()";
      char *fn_name = function->name == NULL ? "@.script": function->name->chars;
      size_t trace_line_length = snprintf(NULL, 0, trace_format, function->module->file, line, fn_name);

      char *trace_line = ALLOCATE(char, trace_line_length + 1);
      if (trace_line != NULL) {
        sprintf(trace_line, trace_format, function->module->file, line, fn_name);
        trace_line[(int) trace_line_length] = '\0';
      }

      trace = append_strings(trace, trace_line);
      free(trace_line);
    }

    return STRING_TT_VAL(trace);
  }

  return STRING_L_VAL("", 0);
}

bool print_exception(z_vm *vm, z_obj_instance *exception, bool is_assert) {
  if(vm->error_count > 0) {
    z_error_frame *error = peek_error(vm);
    error->value = OBJ_VAL(exception);
    return true;
  }

  fflush(stdout); // flush out anything on stdout first

  z_value message, trace;
  if(!is_assert) {
    fprintf(stderr, "Unhandled %s", exception->klass->name->chars);
  } else {
    fprintf(stderr, "Illegal State");
  }
  if (table_get(&exception->properties, STRING_L_VAL("message", 7), &message)) {
    char *error_message = value_to_string(vm, message)->chars;
    if(strlen(error_message) > 0) {
      fprintf(stderr, ": %s", error_message);
    } else {
      fprintf(stderr, ":");
    }
    fprintf(stderr, "\n");
  } else {
    fprintf(stderr, "\n");
  }

  if (table_get(&exception->properties, STRING_L_VAL("stacktrace", 10), &trace)) {
    char *trace_str = value_to_string(vm, trace)->chars;
    fprintf(stderr, "  StackTrace:\n%s\n", trace_str);
  }

  reset_stack(vm);

  return false;
}

bool do_throw_exception(z_vm *vm, const char *type, bool is_assert, const char *format, ...) {

  va_list args;
  va_start(args, format);
  char *message = NULL;
  int length = vasprintf(&message, format, args);
  va_end(args);

  z_obj_instance *instance = create_exception(vm, type, take_string(vm, message, length));
  push(vm, OBJ_VAL(instance));

  z_value stacktrace = get_stack_trace(vm);
  push(vm, stacktrace);
  table_set(vm, &instance->properties, STRING_L_VAL("stacktrace", 10), stacktrace);
  table_set(vm, &instance->properties, STRING_L_VAL("type", 4),
    STRING_L_VAL(instance->klass->name->chars, instance->klass->name->length)
  );
  pop(vm);

  pop(vm); // pop the instance

  bool return_val = print_exception(vm, instance, is_assert);
  if(return_val) {
    // switch context immediately to the catch block
    z_error_frame *error = peek_error(vm);
    vm->frame_count -= vm->current_frame - error->frame;
    vm->current_frame = error->frame;
    vm->current_frame->ip = &error->frame->closure->function->blob.code[error->offset];
    vm->stack_top = error->stack_head;
  }

  return return_val;
}

static void new_exception_class(z_vm * vm, z_obj_class *superclass, const char *name, int name_length) {
  z_obj_string *class_name = copy_string(vm, name, name_length);
  push(vm, OBJ_VAL(class_name));
  z_obj_class *klass = new_class(vm, class_name);
  push(vm, OBJ_VAL(klass));
  klass->initializer = superclass->initializer;
  klass->superclass = superclass;
  table_set(vm, &vm->globals, OBJ_VAL(class_name), OBJ_VAL(klass));
  pop_n(vm, 2);
}

static void initialize_exceptions(z_vm *vm, z_obj_module *module) {
  z_obj_string *class_name = copy_string(vm, "Exception", 9);

  push(vm, OBJ_VAL(class_name));
  z_obj_class *klass = new_class(vm, class_name);
  pop(vm);

  push(vm, OBJ_VAL(klass));
  z_obj_func *function = new_function(vm, module, TYPE_METHOD);

  function->arity = 1;
  function->is_variadic = false;
  push(vm, OBJ_VAL(function));

  // g_loc 0
  write_blob(vm, &function->blob, OP_GET_LOCAL, 0);
  write_blob(vm, &function->blob, (0 >> 8) & 0xff, 0);
  write_blob(vm, &function->blob, 0 & 0xff, 0);

  // g_loc 1
  write_blob(vm, &function->blob, OP_GET_LOCAL, 0);
  write_blob(vm, &function->blob, (1 >> 8) & 0xff, 0);
  write_blob(vm, &function->blob, 1 & 0xff, 0);

  int message_const = add_constant(vm, &function->blob, STRING_L_VAL("message", 7));

  // s_prop 0
  write_blob(vm, &function->blob, OP_SET_PROPERTY, 0);
  write_blob(vm, &function->blob, (message_const >> 8) & 0xff, 0);
  write_blob(vm, &function->blob, message_const & 0xff, 0);

  // pop
  write_blob(vm, &function->blob, OP_POP, 0);
  write_blob(vm, &function->blob, OP_POP, 0);

  // ret
  write_blob(vm, &function->blob, OP_RETURN, 0);

  z_obj_closure *closure = new_closure(vm, function);
  pop(vm);

  // set class constructor
  push(vm, OBJ_VAL(closure));
  table_set(vm, &klass->methods, OBJ_VAL(class_name), OBJ_VAL(closure));
  klass->initializer = OBJ_VAL(closure);

  // set class properties
  table_set(vm, &klass->properties, STRING_L_VAL("message", 7), NIL_VAL);
  table_set(vm, &klass->properties, STRING_L_VAL("stacktrace", 10), NIL_VAL);

  table_set(vm, &vm->globals, OBJ_VAL(class_name), OBJ_VAL(klass));
  pop(vm); // for class

  pop(vm);
  pop(vm); // assert error name

  vm->exception_class = klass;

  // Create other global exceptions
  new_exception_class(vm, klass, "TypeError", 9);
  new_exception_class(vm, klass, "ValueError", 10);
  new_exception_class(vm, klass, "NumericError", 12);
  new_exception_class(vm, klass, "ArgumentError", 13);
  new_exception_class(vm, klass, "NotImplementedError", 19);
  new_exception_class(vm, klass, "RangeError", 10);
  new_exception_class(vm, klass, "AccessError", 11);
  new_exception_class(vm, klass, "PropertyError", 13);
  new_exception_class(vm, klass, "UndefinedError", 14);
}

static z_obj_class *get_exception(z_vm *vm, const char *name) {
  if(!name) {
    return vm->exception_class;
  } else {
    z_value exception_class;
    if(table_get(&vm->globals, STRING_VAL(name), &exception_class)) {
      if(IS_CLASS(exception_class)) {
        return AS_CLASS(exception_class);
      }
    }
    return vm->exception_class;
  }
}

inline z_obj_instance *create_exception(z_vm *vm, const char* type, z_obj_string *message) {
  z_obj_instance *instance = new_instance(vm, get_exception(vm, type));
  push(vm, OBJ_VAL(instance));
  table_set(vm, &instance->properties, STRING_L_VAL("message", 7), OBJ_VAL(message));
  pop(vm);
  return instance;
}

static inline void grow_vm_stack(z_vm *vm, size_t new_capacity) {
  z_value *new_stack = ALLOCATE(z_value, new_capacity);

  size_t old_capacity = vm->stack_capacity;
  // Copy existing initialized region in a single pass
  if (old_capacity > 0) {
    memcpy(new_stack, vm->stack, old_capacity * sizeof(z_value));
  }
  // Initialize only the newly added region to EMPTY_VAL
  for (size_t i = old_capacity; i < new_capacity; i++) {
    new_stack[i] = EMPTY_VAL;
  }

  FREE_ARRAY(z_value, vm->stack, vm->stack_capacity);

  vm->stack = new_stack;
  vm->stack_top = vm->stack + old_capacity;
  vm->stack_capacity = new_capacity;
}

Z_ALWAYS_INLINE void push(z_vm *vm, z_value value) {
  if (Z_UNLIKELY(vm == NULL || vm->stack == NULL || vm->stack_top == NULL)) {
    fprintf(stderr, "Exit: Invalid VM state in push.\n");
    exit(EXIT_TERMINAL);
  }

  if (Z_UNLIKELY(vm->stack_top - vm->stack == vm->stack_capacity)) {
    size_t capacity = GROW_CAPACITY(vm->stack_capacity);
    grow_vm_stack(vm, capacity);
  }

  *vm->stack_top = value;
  vm->stack_top++;
}

inline void push_error(z_vm *vm, z_error_frame *frame) {
  if(vm->error_count >= ERRORS_MAX) {
    fprintf(stderr, "Exit: Maximum open catch blocks %d exceeded.\n", ERRORS_MAX);
    exit(EXIT_RUNTIME);
  }

  vm->errors[vm->error_count] = frame;
  vm->error_count++;
}

inline z_error_frame* pop_error(z_vm *vm) {
  if (vm->error_count == 0) {
    fprintf(stderr, "Exit: Error stack underflow.\n");
    exit(EXIT_TERMINAL);
  }
  z_error_frame *error =  vm->errors[--vm->error_count];
  error->frame = NULL;
  error->stack_head = NULL;
  error->offset = 0;
  return error;
}

inline z_error_frame* peek_error(z_vm *vm) {
  if (vm->error_count == 0) {
    fprintf(stderr, "Exit: Error stack underflow in peek.\n");
    exit(EXIT_TERMINAL);
  }
  return vm->errors[vm->error_count -1];
}

Z_ALWAYS_INLINE z_value pop(z_vm *vm) {
  if (Z_UNLIKELY(vm->stack_top == vm->stack)) {
    fprintf(stderr, "Exit: Stack integrity check at end of stack failed.\n");
    exit(EXIT_TERMINAL);
  }

  vm->stack_top--;
  return *vm->stack_top;
}

Z_ALWAYS_INLINE z_value pop_n(z_vm *vm, int n) {
  if (Z_UNLIKELY(vm->stack_top - vm->stack < n)) {
    fprintf(stderr, "Exit: Stack integrity check at %ld failed.\n", vm->stack_top - vm->stack);
    exit(EXIT_TERMINAL);
  }

  vm->stack_top -= n;
  return *vm->stack_top;
}

Z_ALWAYS_INLINE z_value peek(z_vm *vm, int distance) {
  if (Z_UNLIKELY(vm->stack_top - vm->stack < distance + 1)) {
    fprintf(stderr, "Exit: Stack integrity check at distance %d failed.\n", distance);
    exit(EXIT_TERMINAL);
  }

  return vm->stack_top[-1 - distance]; 
}

static inline void define_native(z_vm *vm, const char *name, z_native_fn function) {
  push(vm, STRING_VAL(name));
  push(vm, OBJ_VAL(new_native(vm, function, name)));
  table_set(vm, &vm->globals, vm->stack[0], vm->stack[1]);
  pop_n(vm, 2);
}

void define_native_method(z_vm *vm, z_table *table, const char *name,
                          z_native_fn function) {
  push(vm, STRING_VAL(name));
  push(vm, OBJ_VAL(new_native(vm, function, name)));
  table_set(vm, table, vm->stack[0], vm->stack[1]);
  pop_n(vm, 2);
}

static void init_builtin_functions(z_vm *vm) {
  DEFINE_NATIVE(abs);
  DEFINE_NATIVE(bin);
  DEFINE_NATIVE(bytes);
  DEFINE_NATIVE(chr);
  DEFINE_NATIVE(delprop);
  DEFINE_NATIVE(file);
  DEFINE_NATIVE(getprop);
  DEFINE_NATIVE(hasprop);
  DEFINE_NATIVE(hex);
  DEFINE_NATIVE(id);
  DEFINE_NATIVE(int);
  DEFINE_NATIVE(is_bool);
  DEFINE_NATIVE(is_callable);
  DEFINE_NATIVE(is_class);
  DEFINE_NATIVE(is_dict);
  DEFINE_NATIVE(is_function);
  DEFINE_NATIVE(is_instance);
  DEFINE_NATIVE(is_int);
  DEFINE_NATIVE(is_list);
  DEFINE_NATIVE(is_number);
  DEFINE_NATIVE(is_object);
  DEFINE_NATIVE(is_string);
  DEFINE_NATIVE(is_bytes);
  DEFINE_NATIVE(is_file);
  DEFINE_NATIVE(is_iterable);
  DEFINE_NATIVE(instance_of);
  DEFINE_NATIVE(max);
  DEFINE_NATIVE(microtime);
  DEFINE_NATIVE(min);
  DEFINE_NATIVE(oct);
  DEFINE_NATIVE(ord);
  DEFINE_NATIVE(print);
  DEFINE_NATIVE(rand);
  DEFINE_NATIVE(setprop);
  DEFINE_NATIVE(sum);
  DEFINE_NATIVE(time);
  DEFINE_NATIVE(to_bool);
  DEFINE_NATIVE(to_dict);
  DEFINE_NATIVE(to_int);
  DEFINE_NATIVE(to_list);
  DEFINE_NATIVE(to_number);
  DEFINE_NATIVE(to_string);
  DEFINE_NATIVE(typeof);
}

static void init_builtin_methods(z_vm *vm) {
#define DEFINE_STRING_METHOD(name) DEFINE_METHOD(string, name)
#define DEFINE_LIST_METHOD(name) DEFINE_METHOD(list, name)
#define DEFINE_DICT_METHOD(name) DEFINE_METHOD(dict, name)
#define DEFINE_FILE_METHOD(name) DEFINE_METHOD(file, name)
#define DEFINE_BYTES_METHOD(name) DEFINE_METHOD(bytes, name)
#define DEFINE_RANGE_METHOD(name) DEFINE_METHOD(range, name)

  // string methods
  DEFINE_STRING_METHOD(length);
  DEFINE_STRING_METHOD(upper);
  DEFINE_STRING_METHOD(lower);
  DEFINE_STRING_METHOD(is_alpha);
  DEFINE_STRING_METHOD(is_alnum);
  DEFINE_STRING_METHOD(is_number);
  DEFINE_STRING_METHOD(is_lower);
  DEFINE_STRING_METHOD(is_upper);
  DEFINE_STRING_METHOD(is_space);
  DEFINE_STRING_METHOD(trim);
  DEFINE_STRING_METHOD(ltrim);
  DEFINE_STRING_METHOD(rtrim);
  DEFINE_STRING_METHOD(join);
  DEFINE_STRING_METHOD(split);
  DEFINE_STRING_METHOD(index_of);
  DEFINE_STRING_METHOD(starts_with);
  DEFINE_STRING_METHOD(ends_with);
  DEFINE_STRING_METHOD(count);
  DEFINE_STRING_METHOD(to_number);
  DEFINE_STRING_METHOD(to_list);
  DEFINE_STRING_METHOD(to_bytes);
  DEFINE_STRING_METHOD(lpad);
  DEFINE_STRING_METHOD(rpad);
  DEFINE_STRING_METHOD(match);
  DEFINE_STRING_METHOD(matches);
  DEFINE_STRING_METHOD(replace);
  DEFINE_STRING_METHOD(replace_with);
  DEFINE_STRING_METHOD(ascii);
  DEFINE_STRING_METHOD(case_fold);
  DEFINE_STRING_METHOD(compare);
  define_native_method(vm, &vm->methods_string, "@iter", native_method_string__iter__);
  define_native_method(vm, &vm->methods_string, "@itern", native_method_string__itern__);

  // list methods
  DEFINE_LIST_METHOD(length);
  DEFINE_LIST_METHOD(append);
  DEFINE_LIST_METHOD(clear);
  DEFINE_LIST_METHOD(clone);
  DEFINE_LIST_METHOD(count);
  DEFINE_LIST_METHOD(extend);
  DEFINE_LIST_METHOD(index_of);
  DEFINE_LIST_METHOD(insert);
  DEFINE_LIST_METHOD(pop);
  DEFINE_LIST_METHOD(shift);
  DEFINE_LIST_METHOD(remove_at);
  DEFINE_LIST_METHOD(remove);
  DEFINE_LIST_METHOD(reverse);
  DEFINE_LIST_METHOD(sort);
  DEFINE_LIST_METHOD(contains);
  DEFINE_LIST_METHOD(delete);
  DEFINE_LIST_METHOD(first);
  DEFINE_LIST_METHOD(last);
  DEFINE_LIST_METHOD(is_empty);
  DEFINE_LIST_METHOD(take);
  DEFINE_LIST_METHOD(get);
  DEFINE_LIST_METHOD(compact);
  DEFINE_LIST_METHOD(unique);
  DEFINE_LIST_METHOD(zip);
  DEFINE_LIST_METHOD(zip_from);
  DEFINE_LIST_METHOD(to_dict);
  define_native_method(vm, &vm->methods_list, "@iter", native_method_list__iter__);
  define_native_method(vm, &vm->methods_list, "@itern", native_method_list__itern__);

  // dictionary methods
  DEFINE_DICT_METHOD(length);
  DEFINE_DICT_METHOD(add);
  DEFINE_DICT_METHOD(set);
  DEFINE_DICT_METHOD(clear);
  DEFINE_DICT_METHOD(clone);
  DEFINE_DICT_METHOD(compact);
  DEFINE_DICT_METHOD(contains);
  DEFINE_DICT_METHOD(extend);
  DEFINE_DICT_METHOD(get);
  DEFINE_DICT_METHOD(keys);
  DEFINE_DICT_METHOD(values);
  DEFINE_DICT_METHOD(remove);
  DEFINE_DICT_METHOD(is_empty);
  DEFINE_DICT_METHOD(find_key);
  DEFINE_DICT_METHOD(to_list);
  define_native_method(vm, &vm->methods_dict, "@iter", native_method_dict__iter__);
  define_native_method(vm, &vm->methods_dict, "@itern", native_method_dict__itern__);

  // file methods
  DEFINE_FILE_METHOD(exists);
  DEFINE_FILE_METHOD(close);
  DEFINE_FILE_METHOD(open);
  DEFINE_FILE_METHOD(read);
  DEFINE_FILE_METHOD(gets);
  DEFINE_FILE_METHOD(write);
  DEFINE_FILE_METHOD(puts);
  DEFINE_FILE_METHOD(number);
  DEFINE_FILE_METHOD(is_tty);
  DEFINE_FILE_METHOD(is_open);
  DEFINE_FILE_METHOD(is_closed);
  DEFINE_FILE_METHOD(flush);
  DEFINE_FILE_METHOD(stats);
  DEFINE_FILE_METHOD(symlink);
  DEFINE_FILE_METHOD(delete);
  DEFINE_FILE_METHOD(rename);
  DEFINE_FILE_METHOD(path);
  DEFINE_FILE_METHOD(abs_path);
  DEFINE_FILE_METHOD(copy);
  DEFINE_FILE_METHOD(truncate);
  DEFINE_FILE_METHOD(chmod);
  DEFINE_FILE_METHOD(set_times);
  DEFINE_FILE_METHOD(seek);
  DEFINE_FILE_METHOD(tell);
  DEFINE_FILE_METHOD(mode);
  DEFINE_FILE_METHOD(name);

  // bytes
  DEFINE_BYTES_METHOD(length);
  DEFINE_BYTES_METHOD(append);
  DEFINE_BYTES_METHOD(clone);
  DEFINE_BYTES_METHOD(extend);
  DEFINE_BYTES_METHOD(index_of);
  DEFINE_BYTES_METHOD(pop);
  DEFINE_BYTES_METHOD(remove);
  DEFINE_BYTES_METHOD(reverse);
  DEFINE_BYTES_METHOD(first);
  DEFINE_BYTES_METHOD(last);
  DEFINE_BYTES_METHOD(get);
  DEFINE_BYTES_METHOD(split);
  DEFINE_BYTES_METHOD(dispose);
  DEFINE_BYTES_METHOD(is_alpha);
  DEFINE_BYTES_METHOD(is_alnum);
  DEFINE_BYTES_METHOD(is_number);
  DEFINE_BYTES_METHOD(is_lower);
  DEFINE_BYTES_METHOD(is_upper);
  DEFINE_BYTES_METHOD(is_space);
  DEFINE_BYTES_METHOD(to_list);
  DEFINE_BYTES_METHOD(to_string);
  define_native_method(vm, &vm->methods_bytes, "@iter", native_method_bytes__iter__);
  define_native_method(vm, &vm->methods_bytes, "@itern", native_method_bytes__itern__);

  // range
  DEFINE_RANGE_METHOD(lower);
  DEFINE_RANGE_METHOD(upper);
  DEFINE_RANGE_METHOD(range);
  DEFINE_RANGE_METHOD(within);
  DEFINE_RANGE_METHOD(step);
  DEFINE_RANGE_METHOD(get_step);
  define_native_method(vm, &vm->methods_range, "@iter", native_method_range__iter__);
  define_native_method(vm, &vm->methods_range, "@itern", native_method_range__itern__);

#undef DEFINE_STRING_METHOD
#undef DEFINE_LIST_METHOD
#undef DEFINE_DICT_METHOD
#undef DEFINE_FILE_METHOD
#undef DEFINE_BYTES_METHOD
#undef DEFINE_RANGE_METHOD
}

void init_vm(z_vm *vm) {

  vm->parent_vm = NULL;
  vm->stack = ALLOCATE(z_value, STACK_MIN);
  vm->stack_capacity = STACK_MIN;

  reset_stack(vm);

  // Ensure error frames array is initialized to NULL to avoid freeing garbage pointers during GC
  for (int i = 0; i < ERRORS_MAX; i++) {
    vm->errors[i] = NULL;
  }

  vm->id = 0;
  vm->compiler = NULL;
  vm->objects = NULL;
  vm->exception_class = NULL;
  vm->current_frame = NULL;
  vm->root_file = NULL;
  vm->bytes_allocated = 0;
  vm->next_gc = DEFAULT_GC_START; // default is 10mb. Can be modified via the -g flag.
  vm->is_repl = false;
  vm->mark_value = true;
  vm->show_warnings = false;
  vm->should_print_bytecode = false;
  vm->should_exit_after_bytecode = false;

  vm->gray_count = 0;
  vm->gray_capacity = 0;
  vm->gray_stack = NULL;

  vm->std_args = NULL;
  vm->std_args_count = 0;

  init_table(&vm->modules);
  init_table(&vm->strings);
  init_table(&vm->globals);

  // object methods tables
  init_table(&vm->methods_string);
  init_table(&vm->methods_list);
  init_table(&vm->methods_dict);
  init_table(&vm->methods_file);
  init_table(&vm->methods_bytes);
  init_table(&vm->methods_range);

  init_builtin_functions(vm);
  init_builtin_methods(vm);
}

void free_vm(z_vm *vm) {
  free_objects(vm);

  // since object in module can exist in globals,
  // it must come before
  if(vm->id == 0) {
    free_table(vm, &vm->modules);
    free_table(vm, &vm->globals);

    free_table(vm, &vm->methods_string);
    free_table(vm, &vm->methods_list);
    free_table(vm, &vm->methods_dict);
    free_table(vm, &vm->methods_file);
    free_table(vm, &vm->methods_bytes);
    free_table(vm, &vm->methods_range);
  }

  // since every vm holds a unique copy.
  free_table(vm, &vm->strings);

  free(vm->stack);

  for(int i = 0; i < vm->error_count; i++) {
    if (vm->errors[i] != NULL) {
      free(vm->errors[i]);
    }
  }

  free(vm);
}

static inline bool is_private(z_obj_string *name) {
  return name->length > 0 && name->chars[0] == '_';
}

static bool call(z_vm *vm, z_obj_closure *closure, int arg_count) {
  // fill empty parameters if not variadic
  for (; !closure->function->is_variadic && arg_count < closure->function->arity; arg_count++) {
    push(vm, NIL_VAL);
  }

  // handle variadic arguments...
  if (closure->function->is_variadic && arg_count >= closure->function->arity - 1) {
    int va_args_start = arg_count - closure->function->arity;
    z_obj_list *args_list = new_list(vm);
    push(vm, OBJ_VAL(args_list));

    for (int i = va_args_start; i >= 0; i--) {
      write_value_arr(vm, &args_list->items, peek(vm, i + 1));
    }
    arg_count -= va_args_start;
    pop_n(vm, va_args_start + 2); // +1 for the gc protection push above
    push(vm, OBJ_VAL(args_list));
  }

  if (arg_count != closure->function->arity) {
    if (arg_count > closure->function->arity) {
      pop_n(vm, arg_count - closure->function->arity);
      // arg_count -= closure->function->arity;
      arg_count = closure->function->arity;
    } else {
      // This should never happen, but just in case some larger-than-life codebase does exist...

      pop_n(vm, arg_count);
      if (closure->function->is_variadic) {
        return throw_argument_error(vm, "expected at least %d arguments but got %d",
                               closure->function->arity - 1, arg_count);
      }

      return throw_argument_error(vm, "expected %d arguments but got %d", closure->function->arity, arg_count);
    }
  }

  if (vm->frame_count == FRAMES_MAX) {
    pop_n(vm, arg_count);
    return throw_exception(vm, "stack overflow");
  }

  z_call_frame *frame = &vm->frames[vm->frame_count++];
  frame->closure = closure;
  frame->ip = closure->function->blob.code;

  frame->slots = vm->stack_top - arg_count - 1;
  return true;
}

static inline bool call_native_method(z_vm *vm, z_obj_native *native, int arg_count) {
  if (native->function(vm, arg_count, vm->stack_top - arg_count)) {
    CLEAR_GC();
    pop_n(vm, arg_count);
  } else {
    CLEAR_GC();
  }
  return true;
}

bool call_value(z_vm *vm, z_value callee, int arg_count) {
  if (IS_OBJ(callee)) {
    switch (OBJ_TYPE(callee)) {
      case OBJ_BOUND_METHOD: {
        z_obj_bound *bound = AS_BOUND(callee);
        vm->stack_top[-arg_count - 1] = bound->receiver;
        return call(vm, bound->method, arg_count);
      }

      case OBJ_CLASS: {
        z_obj_class *klass = AS_CLASS(callee);
        vm->stack_top[-arg_count - 1] = OBJ_VAL(new_instance(vm, klass));
        if (!IS_EMPTY(klass->initializer)) {
          return call(vm, AS_CLOSURE(klass->initializer), arg_count);
        }
        if (klass->superclass != NULL && !IS_EMPTY(klass->superclass->initializer)) {
          return call(vm, AS_CLOSURE(klass->superclass->initializer), arg_count);
        }
        if (arg_count != 0) {
          return throw_type_error(vm, "%s constructor expects 0 arguments, %d given", klass->name->chars, arg_count);
        }
        return true;
      }

      case OBJ_MODULE: {
        z_obj_module *module = AS_MODULE(callee);
        z_value callable;
        if(table_get(&module->values, STRING_VAL(module->name), &callable)) {
          return call_value(vm, callable, arg_count);
        }

        return throw_undefined_error(vm, "module %s does not export a default function", module->name);
      }

      case OBJ_CLOSURE: {
        return call(vm, AS_CLOSURE(callee), arg_count);
      }

      case OBJ_NATIVE: {
        return call_native_method(vm, AS_NATIVE(callee), arg_count);
      }

      default: // non-callable
        break;
    }
  }

  return throw_type_error(vm, "object of type %s is not callable", value_type(callee));
}

static inline z_func_type get_method_type(z_value method) {
  switch (OBJ_TYPE(method)) {
    case OBJ_NATIVE: return AS_NATIVE(method)->type;
    case OBJ_CLOSURE: return AS_CLOSURE(method)->function->type;
    default: return TYPE_FUNCTION;
  }
}

inline bool invoke_from_class(z_vm *vm, z_obj_class *klass, z_obj_string *name, int arg_count) {
  z_value method;
  if (table_get(&klass->methods, OBJ_VAL(name), &method)) {
    z_func_type type = get_method_type(method);

    if (type == TYPE_PRIVATE) {
      return throw_access_error(vm, "cannot call private method '%s' from instance of %s",
                             name->chars, klass->name->chars);
    }
    if (type == TYPE_STATIC) {
      return throw_access_error(vm, "cannot call static method '%s' from instance of %s",
                             name->chars, klass->name->chars);
    }

    return call_value(vm, method, arg_count);
  }

  return throw_undefined_error(vm, "undefined method '%s' in %s", name->chars, klass->name->chars);
}

static bool invoke_self(z_vm *vm, z_obj_string *name, int arg_count) {
  z_value receiver = peek(vm, arg_count);
  z_value value;

  if (IS_OBJ(receiver)) {
    switch (AS_OBJ(receiver)->type) {
      case OBJ_INSTANCE: {
        z_obj_instance *instance = AS_INSTANCE(receiver);

        if (table_get(&instance->klass->methods, OBJ_VAL(name), &value)) {
          if (get_method_type(value) != TYPE_STATIC) {
            return call_value(vm, value, arg_count);
          }

          return throw_access_error(vm, "cannot call static method %s() on instance", name->chars);
        }

        if (table_get(&instance->properties, OBJ_VAL(name), &value)) {
          vm->stack_top[-arg_count - 1] = value;
          return call_value(vm, value, arg_count);
        }
      }
      case OBJ_CLASS: {
        if (table_get(&AS_CLASS(receiver)->methods, OBJ_VAL(name), &value)) {
          if (get_method_type(value) == TYPE_STATIC) {
            return call_value(vm, value, arg_count);
          }

          return throw_access_error(vm, "cannot call non-static method %s() on non instance", name->chars);
        }
      }
      case OBJ_STRING: {
        if (table_get(&vm->methods_string, OBJ_VAL(name), &value)) {
          return call_value(vm, value, arg_count);
        }
      }
      case OBJ_BYTES: {
        if (table_get(&vm->methods_bytes, OBJ_VAL(name), &value)) {
          return call_value(vm, value, arg_count);
        }
      }
      case OBJ_DICT: {
        if (table_get(&vm->methods_dict, OBJ_VAL(name), &value)) {
          return call_value(vm, value, arg_count);
        }
      }
      case OBJ_FILE: {
        if (table_get(&vm->methods_file, OBJ_VAL(name), &value)) {
          return call_value(vm, value, arg_count);
        }
      }
      case OBJ_LIST: {
        if (table_get(&vm->methods_list, OBJ_VAL(name), &value)) {
          return call_value(vm, value, arg_count);
        }
      }
      case OBJ_RANGE: {
        if (table_get(&vm->methods_range, OBJ_VAL(name), &value)) {
          return call_value(vm, value, arg_count);
        }
      }
      default: // fallthrough
        break;
    }
  }

  return throw_type_error(vm, "cannot call method %s on object of type %s",
                         name->chars, value_type(receiver));
}

static bool invoke_operator(z_vm *vm, z_obj_string *name, int arg_count, bool is_binary) {
  z_value receiver = peek(vm, arg_count);
  z_value value;
  z_obj_instance *instance = AS_INSTANCE(receiver);
  if (table_get(&instance->klass->methods, OBJ_VAL(name), &value)) {
    if (IS_CLOSURE(value)) {
      z_obj_func *function = AS_CLOSURE(value)->function;
      if (!function->is_variadic && function->arity == 1) {
        return call_value(vm, value, is_binary ? arg_count : 0);
      }
    }
  }

  if(!is_binary) {
    return throw_numeric_error(vm, "object of type %s does not define unary operation %s", value_type(receiver), name->chars);
  } else {
    return throw_numeric_error(vm, "object of type %s does not define binary operation %s", value_type(receiver), name->chars);
  }
}

static bool invoke(z_vm *vm, z_obj_string *name, int arg_count) {
  z_value receiver = peek(vm, arg_count);
  z_value value;

  if (!IS_OBJ(receiver)) {
    // @TODO: have methods for non-objects as well.
    return throw_type_error(vm, "non-object %s has no method '%s'", value_type(receiver), name->chars);
  } else {
    switch (AS_OBJ(receiver)->type) {
      case OBJ_MODULE: {
        z_obj_module *module = AS_MODULE(receiver);
        if (table_get(&module->values, OBJ_VAL(name), &value)) {
          if (is_private(name)) {
            return throw_access_error(vm, "cannot call private module method '%s'", name->chars);
          }
          return call_value(vm, value, arg_count);
        }
        return throw_undefined_error(vm, "module %s does not define class or method %s()", module->name, name->chars);
        break;
      }
      case OBJ_CLASS: {
        if (table_get(&AS_CLASS(receiver)->methods, OBJ_VAL(name), &value)) {
          if (get_method_type(value) == TYPE_PRIVATE) {
            return throw_access_error(vm, "cannot call private method %s() on %s",
                                   name->chars, AS_CLASS(receiver)->name->chars);
          }
          return call_value(vm, value, arg_count);
        }
        if (table_get(&AS_CLASS(receiver)->static_properties, OBJ_VAL(name), &value)) {
          return call_value(vm, value, arg_count);
        }

        return throw_undefined_error(vm, "unknown method %s() in class %s", name->chars, AS_CLASS(receiver)->name->chars);
      }
      case OBJ_INSTANCE: {
        z_obj_instance *instance = AS_INSTANCE(receiver);

        if (table_get(&instance->properties, OBJ_VAL(name), &value)) {
          vm->stack_top[-arg_count - 1] = value;
          return call_value(vm, value, arg_count);
        }

        return invoke_from_class(vm, instance->klass, name, arg_count);
      }
      case OBJ_STRING: {
        if (table_get(&vm->methods_string, OBJ_VAL(name), &value)) {
          return call_value(vm, value, arg_count);
        }
        return throw_property_error(vm, "String has no method %s()", name->chars);
      }
      case OBJ_LIST: {
        if (table_get(&vm->methods_list, OBJ_VAL(name), &value)) {
          return call_value(vm, value, arg_count);
        }
        return throw_property_error(vm, "List has no method %s()", name->chars);
      }
      case OBJ_RANGE: {
        if (table_get(&vm->methods_range, OBJ_VAL(name), &value)) {
          return call_value(vm, value, arg_count);
        }
        return throw_property_error(vm, "Range has no method %s()", name->chars);
      }
      case OBJ_DICT: {
        if (table_get(&vm->methods_dict, OBJ_VAL(name), &value)) {
          return call_value(vm, value, arg_count);
        }

        else if(table_get(&AS_DICT(receiver)->items, OBJ_VAL(name), &value)) {
          if(IS_CLOSURE(value)) {
            return call_value(vm, value, arg_count);
          }
        }
        return throw_property_error(vm, "Dict has no method %s()", name->chars);
      }
      case OBJ_FILE: {
        if (table_get(&vm->methods_file, OBJ_VAL(name), &value)) {
          return call_value(vm, value, arg_count);
        }
        return throw_property_error(vm, "File has no method %s()", name->chars);
      }
      case OBJ_BYTES: {
        if (table_get(&vm->methods_bytes, OBJ_VAL(name), &value)) {
          return call_value(vm, value, arg_count);
        }
        return throw_property_error(vm, "Bytes has no method %s()", name->chars);
      }
      default: {
        return throw_type_error(vm, "cannot call method %s on object of type %s",
                               name->chars, value_type(receiver));
      }
    }
  }
}

static inline bool bind_method(z_vm *vm, z_obj_class *klass, z_obj_string *name) {
  z_value method;
  if (table_get(&klass->methods, OBJ_VAL(name), &method)) {
    if (get_method_type(method) == TYPE_PRIVATE) {
      return throw_access_error(vm, "cannot get private property '%s' from instance", name->chars);
    }

    z_obj_bound *bound = new_bound_method(vm, peek(vm, 0), AS_CLOSURE(method));
    pop(vm);
    push(vm, OBJ_VAL(bound));
    return true;
  }

  return throw_undefined_error(vm, "undefined property '%s'", name->chars);
}

static z_obj_up_value *capture_up_value(z_vm *vm, z_value *local) {
  z_obj_up_value *prev_up_value = NULL;
  z_obj_up_value *up_value = vm->open_up_values;

  while (up_value != NULL && up_value->location > local) {
    prev_up_value = up_value;
    up_value = up_value->next;
  }

  if (up_value != NULL && up_value->location == local)
    return up_value;

  z_obj_up_value *created_up_value = new_up_value(vm, local);
  created_up_value->next = up_value;

  if (prev_up_value == NULL) {
    vm->open_up_values = created_up_value;
  } else {
    prev_up_value->next = created_up_value;
  }

  return created_up_value;
}

static inline void close_up_values(z_vm *vm, const z_value *last) {
  while (vm->open_up_values != NULL && vm->open_up_values->location >= last) {
    z_obj_up_value *up_value = vm->open_up_values;
    up_value->closed = *up_value->location;
    up_value->location = &up_value->closed;
    vm->open_up_values = up_value->next;
  }
}

static inline void define_method(z_vm *vm, z_obj_string *name) {
  z_value method = peek(vm, 0);
  z_obj_class *klass = AS_CLASS(peek(vm, 1));

  table_set(vm, &klass->methods, OBJ_VAL(name), method);
  if (get_method_type(method) == TYPE_INITIALIZER) {
    klass->initializer = method;
  }
  pop(vm);
}

static inline void define_property(z_vm *vm, z_obj_string *name, bool is_static) {
  z_value property = peek(vm, 0);
  z_obj_class *klass = AS_CLASS(peek(vm, 1));

  if (!is_static) {
    table_set(vm, &klass->properties, OBJ_VAL(name), property);
  } else {
    table_set(vm, &klass->static_properties, OBJ_VAL(name), property);
  }
  pop(vm);
}

Z_ALWAYS_INLINE bool is_false(z_value value) {
  // Fast-path checks first (common cases):
  if (Z_LIKELY(IS_BOOL(value))) return !AS_BOOL(value);
  if (Z_LIKELY(IS_NUMBER(value))) return AS_NUMBER(value) < 0; // -1 is false in Zuri

  // Nil or Empty are falsey
  if (IS_NIL(value) || IS_EMPTY(value)) return true;

  // Strings/bytes/collections: empty => false, non-empty => true
  if (IS_STRING(value)) return AS_STRING(value)->length < 1;
  if (IS_BYTES(value)) return AS_BYTES(value)->bytes.count < 1;
  if (IS_LIST(value)) return AS_LIST(value)->items.count == 0;
  if (IS_DICT(value)) return AS_DICT(value)->names.count == 0;

  // Classes, closures, bound methods, functions are true by default
  return false;
}

bool is_instance_of(z_obj_class *klass1, z_obj_class *klass2) {
  while (klass1 != NULL) {
    // quick exit check
    if(klass1 == klass2) return true;

    // check the class names
    if (klass2->name->length == klass1->name->length
        && memcmp(klass1->name->chars, klass2->name->chars, klass1->name->length) == 0) {

      // TODO: ensure they're actually the same
      if(klass1->initializer == klass2->initializer) {
        return true;
      }
      // return true;
    }
    klass1 = klass1->superclass;
  }

  return false;
}

inline bool dict_set_entry(z_vm *vm, z_obj_dict *dict, z_value key, z_value value) {
  z_value temp_value;
  if (!table_get(&dict->items, key, &temp_value)) {
    write_value_arr(vm, &dict->names, key); // add key if it doesn't exist.
  }
  return table_set(vm, &dict->items, key, value);
}

inline void dict_add_entry(z_vm *vm, z_obj_dict *dict, z_value key, z_value value) {
  dict_set_entry(vm, dict, key, value);
}

inline bool dict_get_entry(z_obj_dict *dict, z_value key, z_value *value) {
  return table_get(&dict->items, key, value);
}

static z_obj_string *multiply_string(z_vm *vm, z_obj_string *str, double number) {
  int times = (int) number;

  if (times <= 0) // 'str' * 0 == '', 'str' * -1 == ''
    return copy_string(vm, "", 0);
  else if (times == 1) // 'str' * 1 == 'str'
    return str;

  int total_length = str->length * times;
  char *result = ALLOCATE(char, (size_t) total_length + 1);

  for (int i = 0; i < times; i++) {
    memcpy(result + (str->length * i), str->chars, str->length);
  }
  result[total_length] = '\0';
  return take_string(vm, result, total_length);
}

static z_obj_list *add_list(z_vm *vm, z_obj_list *a, z_obj_list *b) {
  z_obj_list *list = new_list(vm);
  push(vm, OBJ_VAL(list));

  for (int i = 0; i < a->items.count; i++) {
    write_value_arr(vm, &list->items, a->items.values[i]);
  }

  for (int i = 0; i < b->items.count; i++) {
    write_value_arr(vm, &list->items, b->items.values[i]);
  }

  pop(vm);
  return list;
}

static inline z_obj_bytes *add_bytes(z_vm *vm, z_obj_bytes *a, z_obj_bytes *b) {
  z_obj_bytes *bytes = new_bytes(vm, a->bytes.count + b->bytes.count);

  memcpy(bytes->bytes.bytes, a->bytes.bytes, a->bytes.count);
  memcpy(bytes->bytes.bytes + a->bytes.count, b->bytes.bytes, b->bytes.count);

  return bytes;
}

static inline void multiply_list(z_vm *vm, z_obj_list *a, z_obj_list *new_list, int times) {
  for (int i = 0; i < times; i++) {
    for (int j = 0; j < a->items.count; j++) {
      write_value_arr(vm, &new_list->items, a->items.values[j]);
    }
  }
}

static bool dict_get_index(z_vm *vm, z_obj_dict *dict, bool will_assign) {
  z_value index = peek(vm, 0);

  z_value result;
  if (dict_get_entry(dict, index, &result)) {
    if (!will_assign) {
      pop_n(vm, 2); // we can safely get rid of the index from the stack
    }
    push(vm, result);
    return true;
  }

  pop_n(vm, 1);
  return throw_argument_error(vm, "invalid index %s", value_to_string(vm, index)->chars);
}

static bool module_get_index(z_vm *vm, z_obj_module *module, bool will_assign) {
  z_value index = peek(vm, 0);

  z_value result;
  if (table_get(&module->values, index, &result)) {
    if (!will_assign) {
      pop_n(vm, 2); // we can safely get rid of the index from the stack
    }
    push(vm, result);
    return true;
  }

  pop_n(vm, 1);
  return throw_undefined_error(vm, "%s is undefined in module %s", value_to_string(vm, index)->chars, module->name);
}

static bool string_get_index(z_vm *vm, z_obj_string *string, bool will_assign) {
  z_value lower = peek(vm, 0);

  if (!IS_NUMBER(lower)) {
    pop_n(vm, 1);
    return throw_argument_error(vm, "strings are numerically indexed");
  }

  int index = AS_NUMBER(lower);
  int length = string->is_ascii ? string->length : string->utf8_length;
  int real_index = index;
  if (index < 0)
    index = length + index;

  if (index < length && index >= 0) {

    int start = index, end = index + 1;
    if(!string->is_ascii){
      utf8slice(string->chars, &start, &end);
    }

    if (!will_assign) {
      // we can safely get rid of the index from the stack
      pop_n(vm, 2); // +1 for the string itself
    }

    push(vm, STRING_L_VAL(string->chars + start, end - start));
    return true;
  } else {
    pop_n(vm, 1);
    return throw_range_error(vm, "string index %d out of range", real_index);
  }
}

static bool string_get_ranged_index(z_vm *vm, z_obj_string *string, bool will_assign) {
  z_value upper = peek(vm, 0);
  z_value lower = peek(vm, 1);

  if (!(IS_NIL(lower) || IS_NUMBER(lower)) || !(IS_NUMBER(upper) || IS_NIL(upper))) {
    pop_n(vm, 2);
    return throw_argument_error(vm, "string are numerically indexed");
  }
  int length = string->is_ascii ? string->length : string->utf8_length;

  int lower_index = IS_NUMBER(lower) ? AS_NUMBER(lower) : 0;
  int upper_index = IS_NIL(upper) ? length : AS_NUMBER(upper);

  if (lower_index < 0 || (upper_index < 0 && ((length + upper_index) < 0)) || lower_index >= length) {
    // always return an empty string...
    if (!will_assign) {
      pop_n(vm, 3); // +1 for the string itself
    }
    push(vm, STRING_L_VAL("", 0));
    return true;
  }

  if (upper_index < 0)
    upper_index = length + upper_index;

  if (upper_index > length)
    upper_index = length;

  int start = lower_index, end = upper_index;
  if(!string->is_ascii) {
    utf8slice(string->chars, &start, &end);
  }

  if (!will_assign) {
    pop_n(vm, 3); // +1 for the string itself
  }

  push(vm, STRING_L_VAL(string->chars + start, end - start));
  return true;
}

static bool bytes_get_index(z_vm *vm, z_obj_bytes *bytes, bool will_assign) {
  z_value lower = peek(vm, 0);

  if (!IS_NUMBER(lower)) {
    pop_n(vm, 1);
    return throw_argument_error(vm, "bytes are numerically indexed");
  }

  int index = AS_NUMBER(lower);
  int real_index = index;
  if (index < 0)
    index = bytes->bytes.count + index;

  if (index < bytes->bytes.count && index >= 0) {
    if (!will_assign) {
      // we can safely get rid of the index from the stack
      pop_n(vm, 2); // +1 for the bytes itself
    }

    push(vm, NUMBER_VAL((int) bytes->bytes.bytes[index]));
    return true;
  } else {
    pop_n(vm, 1);
    return throw_range_error(vm, "bytes index %d out of range", real_index);
  }
}

static bool bytes_get_ranged_index(z_vm *vm, z_obj_bytes *bytes, bool will_assign) {
  z_value upper = peek(vm, 0);
  z_value lower = peek(vm, 1);

  if (!(IS_NIL(lower) || IS_NUMBER(lower)) || !(IS_NUMBER(upper) || IS_NIL(upper))) {
    pop_n(vm, 2);
    return throw_argument_error(vm, "bytes are numerically indexed");
  }

  int lower_index = IS_NUMBER(lower) ? AS_NUMBER(lower) : 0;
  int upper_index = IS_NIL(upper) ? bytes->bytes.count : AS_NUMBER(upper);

  if (lower_index < 0 ||
      (upper_index < 0 && ((bytes->bytes.count + upper_index) < 0)) || lower_index >= bytes->bytes.count) {
    // always return an empty bytes object...
    if (!will_assign) {
      pop_n(vm, 3); // +1 for the bytes itself
    }
    push(vm, OBJ_VAL(new_bytes(vm, 0)));
    return true;
  }

  if (upper_index < 0)
    upper_index = bytes->bytes.count + upper_index;

  if (upper_index > bytes->bytes.count)
    upper_index = bytes->bytes.count;

  if (!will_assign) {
    pop_n(vm, 3); // +1 for the list itself
  }
  push(vm, OBJ_VAL(copy_bytes(vm, bytes->bytes.bytes + lower_index,
                              upper_index - lower_index)));
  return true;
}

static bool list_get_index(z_vm *vm, z_obj_list *list, bool will_assign) {
  z_value lower = peek(vm, 0);

  if (!IS_NUMBER(lower)) {
    pop(vm);
    return throw_argument_error(vm, "list are numerically indexed");
  }

  int index = AS_NUMBER(lower);
  int real_index = index;
  if (index < 0)
    index = list->items.count + index;

  if (index < list->items.count && index >= 0) {
    if (!will_assign) {
      // we can safely get rid of the index from the stack
      pop_n(vm, 2); // +1 for the list itself
    }

    push(vm, list->items.values[index]);
    return true;
  } else {
    pop(vm);
    return throw_range_error(vm, "list index %d out of range", real_index);
  }
}

static bool list_get_ranged_index(z_vm *vm, z_obj_list *list, bool will_assign) {
  z_value upper = peek(vm, 0);
  z_value lower = peek(vm, 1);

  if (!(IS_NIL(lower) || IS_NUMBER(lower)) || !(IS_NUMBER(upper) || IS_NIL(upper))) {
    pop_n(vm, 2);
    return throw_argument_error(vm, "list are numerically indexed");
  }

  int lower_index = IS_NUMBER(lower) ? AS_NUMBER(lower) : 0;
  int upper_index = IS_NIL(upper) ? list->items.count : AS_NUMBER(upper);

  if (lower_index < 0 || (upper_index < 0 && ((list->items.count + upper_index) < 0)) || lower_index >= list->items.count) {
    // always return an empty list...
    if (!will_assign) {
      pop_n(vm, 3); // +1 for the list itself
    }
    push(vm, OBJ_VAL(new_list(vm)));
    return true;
  }

  if (upper_index < 0)
    upper_index = list->items.count + upper_index;

  if (upper_index > list->items.count)
    upper_index = list->items.count;

  z_obj_list *n_list = new_list(vm);
  push(vm, OBJ_VAL(n_list)); // gc protect

  for (int i = lower_index; i < upper_index; i++) {
    write_value_arr(vm, &n_list->items, list->items.values[i]);
  }
  pop(vm);  // clear gc protect

  if (!will_assign) {
    pop_n(vm, 3); // +1 for the list itself
  }
  push(vm, OBJ_VAL(n_list));
  return true;
}

static inline void dict_set_index(z_vm *vm, z_obj_dict *dict, z_value index, z_value value) {
  dict_set_entry(vm, dict, index, value);
  pop_n(vm, 3); // pop the value, index and dict out

  // leave the value on the stack for consumption
  // e.g., variable = dict[index] = 10
  push(vm, value);
}

static inline void module_set_index(z_vm *vm, z_obj_module *module, z_value index, z_value value) {
  table_set(vm, &module->values, index, value);
  pop_n(vm, 3); // pop the value, index and dict out

  // leave the value on the stack for consumption
  // e.g., variable = dict[index] = 10
  push(vm, value);
}

static bool list_set_index(z_vm *vm, z_obj_list *list, z_value index, z_value value) {
  if (!IS_NUMBER(index)) {
    pop_n(vm, 3); // pop the value, index and list out
    return throw_argument_error(vm, "list are numerically indexed");
  }

  int _position = AS_NUMBER(index);
  int position = _position < 0 ? list->items.count + _position : _position;

  if (position < list->items.count && position > -(list->items.count)) {
    list->items.values[position] = value;
    pop_n(vm, 3); // pop the value, index and list out

    // leave the value on the stack for consumption
    // e.g., variable = list[index] = 10
    push(vm, value);
    return true;
  }

  pop_n(vm, 3); // pop the value, index and list out
  return throw_range_error(vm, "lists index %d out of range", _position);
}

static bool bytes_set_index(z_vm *vm, z_obj_bytes *bytes, z_value index, z_value value) {
  if (!IS_NUMBER(index)) {
    pop_n(vm, 3); // pop the value, index and bytes out
    return throw_argument_error(vm, "bytes are numerically indexed");
  } else if (!IS_NUMBER(value) || AS_NUMBER(value) < 0 || AS_NUMBER(value) > 255) {
    pop_n(vm, 3); // pop the value, index and bytes out
    return throw_value_error(vm, "invalid byte. bytes are numbers between 0 and 255.");
  }

  int _position = AS_NUMBER(index);
  int byte = AS_NUMBER(value);

  int position = _position < 0 ? bytes->bytes.count + _position : _position;

  if (position < bytes->bytes.count && position > -(bytes->bytes.count)) {
    bytes->bytes.bytes[position] = (unsigned char) byte;
    pop_n(vm, 3); // pop the value, index and bytes out

    // leave the value on the stack for consumption
    // e.g., variable = bytes[index] = 10
    push(vm, value);
    return true;
  }

  pop_n(vm, 3); // pop the value, index and bytes out
  return throw_range_error(vm, "bytes index %d out of range", _position);
}

static bool concatenate(z_vm *vm) {
  z_value _b = peek(vm, 0);
  z_value _a = peek(vm, 1);

  if (IS_NIL(_a)) {
    pop_n(vm, 2);
    push(vm, _b);
  } else if (IS_NIL(_b)) {
    pop(vm);
  } else if (IS_NUMBER(_a)) {
    double a = AS_NUMBER(_a);

    int num_length = 0;
    char *num_str = number_to_string(vm, a, &num_length);

    z_obj_string *b = AS_STRING(_b);

    int length = num_length + b->length;
    char *chars = ALLOCATE(char, (size_t) length + 1);
    memcpy(chars, num_str, num_length);
    memcpy(chars + num_length, b->chars, b->length);
    chars[length] = '\0';

    int utf8len = utf8length(chars);
    z_obj_string *result = take_string(vm, chars, length);
    result->utf8_length = utf8len;

    FREE(char, num_str);

    pop_n(vm, 2);
    push(vm, OBJ_VAL(result));
  } else if (IS_NUMBER(_b)) {
    z_obj_string *a = AS_STRING(_a);
    double b = AS_NUMBER(_b);

    int num_length = 0;
    char *num_str = number_to_string(vm, b, &num_length);

    int length = num_length + a->length;
    char *chars = ALLOCATE(char, (size_t) length + 1);
    memcpy(chars, a->chars, a->length);
    memcpy(chars + a->length, num_str, num_length);
    chars[length] = '\0';

    int utf8len = utf8length(chars);
    z_obj_string *result = take_string(vm, chars, length);
    result->utf8_length = utf8len;

    FREE(char, num_str);

    pop_n(vm, 2);
    push(vm, OBJ_VAL(result));
  } else if (IS_STRING(_a) && IS_STRING(_b)) {
    z_obj_string *b = AS_STRING(_b);
    z_obj_string *a = AS_STRING(_a);

    int length = a->length + b->length;
    char *chars = ALLOCATE(char, length + 1);
    memcpy(chars, a->chars, a->length);
    memcpy(chars + a->length, b->chars, b->length);
    chars[length] = '\0';

    // int new_utf8_length = utf8length(chars);
    z_obj_string *result = take_string(vm, chars, length);
    result->utf8_length = a->utf8_length + b->utf8_length;
    // result->utf8_length = new_utf8_length;

    pop_n(vm, 2);
    push(vm, OBJ_VAL(result));
  } else {
    return false;
  }

  return true;
}

static inline int floor_div(double a, double b) {
  int d = (int) a / (int) b;
  return d - ((d * b == a) & ((a < 0) ^ (b < 0)));
}

static inline double modulo(double a, double b) {
  double r = fmod(a, b);
  if (r!=0 && ((r<0) != (b<0))) {
    r += b;
  }
  return r;
}

static double z_int_bin_op(z_code op, double a, double b) {
  int32_t ia = isnan(a) || isinf(a) ? 0 : (int32_t)(int64_t) a;
  int32_t ib = isnan(b) || isinf(b) ? 0 : (int32_t)(int64_t) b;

  switch (op) {
    case OP_LSHIFT:
      return (int32_t)((uint32_t) ia << ((uint32_t) ib & 31));
    case OP_RSHIFT:
      return ia >> ((uint32_t) ib & 31);
    case OP_URSHIFT:
      return (uint32_t) ia >> ((uint32_t) ib & 31);
    case OP_OR:
      return ia | ib;
    case OP_XOR:
      return ia ^ ib;
    case OP_AND:
      return ia & ib;
    default:
      assert(0);
  }
  return 0;
}

z_ptr_result run(z_vm *vm, int exit_frame) {
  vm->current_frame = &vm->frames[vm->frame_count - 1];

#define READ_BYTE() (*vm->current_frame->ip++)

#define READ_SHORT()                                                           \
  (vm->current_frame->ip += 2, (uint16_t)((vm->current_frame->ip[-2] << 8) | vm->current_frame->ip[-1]))

#define READ_CONSTANT()                                                        \
  (vm->current_frame->closure->function->blob.constants.values[READ_SHORT()])

#define READ_STRING() (AS_STRING(READ_CONSTANT()))

#define PRE_BINARY_OP() \
      z_value __b = peek(vm, 0); \
      z_value __a = peek(vm, 1)

#define BINARY_ON_NON_NUMBERS() \
  (!IS_NUMBER(__b) && !IS_BOOL(__b)) || (!IS_NUMBER(__a) && !IS_BOOL(__a))

#define UNSUPPORTED_OPERAND(op) \
  numeric_error("unsupported operand %s for %s and %s", #op, value_type(__a), value_type(__b)); \
  break

#define CLASS_BINARY_OPERATION(op) \
  if(IS_INSTANCE(__a)) {                               \
    z_obj_instance *a_instance = AS_INSTANCE(__a); \
    if(!invoke_operator(vm, copy_string(vm, (op), strlen((op))), 1, true)) { \
      EXIT_VM();                                                       \
    }                                                                    \
    vm->current_frame = &vm->frames[vm->frame_count - 1];\
    break; \
  } \
  UNSUPPORTED_OPERAND(op)

#define CLASS_UNARY_OPERATION(op) \
  if(IS_INSTANCE(a)) {                               \
    z_obj_instance *a_instance = AS_INSTANCE(a); \
    if(!invoke_operator(vm, copy_string(vm, (op), strlen((op))), 0, false)) { \
      EXIT_VM();                                                       \
    }                                                                    \
    vm->current_frame = &vm->frames[vm->frame_count - 1];\
    break; \
  } \
  numeric_error("operator %s not defined for object of type %s", #op, value_type(a))

#define BINARY_OP(type, op)                                                    \
  do { \
    PRE_BINARY_OP();          \
    /* Fast path: both operands are numbers (most common). */ \
    if (Z_LIKELY(IS_NUMBER(__a) && IS_NUMBER(__b))) { \
      double b = AS_NUMBER(pop(vm)); \
      double a = AS_NUMBER(pop(vm)); \
      push(vm, type(a op b)); \
      break; \
    } \
    /* Fallback: handle mixed number/bool and non-number types via operator overloading. */ \
    if (BINARY_ON_NON_NUMBERS()) { \
      CLASS_BINARY_OPERATION(#op);    \
    }                                                                          \
    z_value _b = pop(vm);                                                      \
    double b = IS_BOOL(_b) ? (AS_BOOL(_b) ? 1 : 0) : AS_NUMBER(_b);            \
    z_value _a = pop(vm);                                                      \
    double a = IS_BOOL(_a) ? (AS_BOOL(_a) ? 1 : 0) : AS_NUMBER(_a);            \
    push(vm, type(a op b));                                                    \
  } while (false)

#define BINARY_BIT_OP(op, original_op)                                                \
  do {          \
    PRE_BINARY_OP();          \
    if (BINARY_ON_NON_NUMBERS()) { \
      CLASS_BINARY_OPERATION(#original_op);    \
    }                                                                          \
    double b = AS_NUMBER(pop(vm));                                       \
    double a = AS_NUMBER(pop(vm));                        \
    push(vm, NUMBER_VAL(z_int_bin_op(op, a, b)));                                          \
  } while (false)

#define BINARY_MOD_OP(type, op, original_op)                                                \
  do {  \
    PRE_BINARY_OP();          \
    /* Fast path: both operands are numbers (most common). */ \
    if (Z_LIKELY(IS_NUMBER(__a) && IS_NUMBER(__b))) { \
      double b = AS_NUMBER(pop(vm)); \
      double a = AS_NUMBER(pop(vm)); \
      push(vm, type(op(a, b))); \
      break; \
    } \
    if (BINARY_ON_NON_NUMBERS()) { \
      CLASS_BINARY_OPERATION(original_op);    \
    }                                                    \
    z_value _b = pop(vm);                                                      \
    double b = IS_BOOL(_b) ? (AS_BOOL(_b) ? 1 : 0) : AS_NUMBER(_b);            \
    z_value _a = pop(vm);                                                      \
    double a = IS_BOOL(_a) ? (AS_BOOL(_a) ? 1 : 0) : AS_NUMBER(_a);            \
    push(vm, type(op(a, b)));                                                  \
  } while (false)

#define TRY_STRING_OVERRIDE(val) if(IS_INSTANCE((val))) { \
    z_value tmp_fn; \
    if(table_get(&AS_INSTANCE((val))->klass->methods, STRING_L_VAL("@to_string", 10), &tmp_fn)) { \
      vm->current_frame->ip--; \
      if(call_value(vm, tmp_fn, 0)) { \
        vm->current_frame = &vm->frames[vm->frame_count - 1]; \
        break; \
      } \
      vm->current_frame->ip++;  \
    } \
  }

  for (;;) {
    // try...finally... (i.e., try without a catch but finally
    // whose try body raises an exception)
    // can cause us to go into an invalid mode where frame count == 0;
    // to fix this, we need to exit with an appropriate mode here.
    if (vm->frame_count == 0) {
      return PTR_RUNTIME_ERR;
    }

#if defined(DEBUG_STACK) && DEBUG_STACK
      printf("          ");
      for (z_value *slot = vm->stack; slot < vm->stack_top; slot++) {
        printf("[ ");
        print_value(*slot);
        printf(" ]");
      }
      printf("\n");
      disassemble_instruction(
          &vm->current_frame->closure->function->blob,
          (int) (vm->current_frame->ip - vm->current_frame->closure->function->blob.code));
#endif

    switch (READ_BYTE()) {

      case OP_CONSTANT: {
        z_value constant = READ_CONSTANT();
        push(vm, constant);
        break;
      }

      case OP_ADD: {
        if (IS_STRING(peek(vm, 0)) || IS_STRING(peek(vm, 1))) {
          if (!concatenate(vm)) {
            numeric_error("unsupported operand + for %s and %s", value_type(peek(vm, 0)), value_type(peek(vm, 1)));
            break;
          }
        } else if (IS_LIST(peek(vm, 0)) && IS_LIST(peek(vm, 1))) {
          z_value result =
              OBJ_VAL(add_list(vm, AS_LIST(peek(vm, 1)), AS_LIST(peek(vm, 0))));
          pop_n(vm, 2);
          push(vm, result);
        } else if (IS_BYTES(peek(vm, 0)) && IS_BYTES(peek(vm, 1))) {
          z_value result = OBJ_VAL(
              add_bytes(vm, AS_BYTES(peek(vm, 1)), AS_BYTES(peek(vm, 0))));
          pop_n(vm, 2);
          push(vm, result);
        } else {
          BINARY_OP(NUMBER_VAL, +);
        }
        break;
      }
      case OP_SUBTRACT: {
        BINARY_OP(NUMBER_VAL, -);
        break;
      }
      case OP_MULTIPLY: {
        if (IS_STRING(peek(vm, 1)) && IS_NUMBER(peek(vm, 0))) {
          double number = AS_NUMBER(peek(vm, 0));
          z_obj_string *string = AS_STRING(peek(vm, 1));
          z_value result = OBJ_VAL(multiply_string(vm, string, number));
          pop_n(vm, 2);
          push(vm, result);
          break;
        } else if (IS_LIST(peek(vm, 1)) && IS_NUMBER(peek(vm, 0))) {
          int number = (int) AS_NUMBER(pop(vm));
          z_obj_list *list = AS_LIST(peek(vm, 0));
          z_obj_list *n_list = new_list(vm);
          push(vm, OBJ_VAL(n_list));
          multiply_list(vm, list, n_list, number);
          pop_n(vm, 2);
          push(vm, OBJ_VAL(n_list));
          break;
        }
        BINARY_OP(NUMBER_VAL, *);
        break;
      }
      case OP_DIVIDE: {
        BINARY_OP(NUMBER_VAL, /);
        break;
      }
      case OP_REMINDER: {
        BINARY_MOD_OP(NUMBER_VAL, modulo, "%");
        break;
      }
      case OP_POW: {
        BINARY_MOD_OP(NUMBER_VAL, pow, "**");
        break;
      }
      case OP_F_DIVIDE: {
        BINARY_MOD_OP(NUMBER_VAL, floor_div, "//");
        break;
      }
      case OP_NEGATE: {
        z_value a = peek(vm, 0);
        if (!IS_NUMBER(a)) {
          CLASS_UNARY_OPERATION("-");
        }
        push(vm, NUMBER_VAL(-AS_NUMBER(pop(vm))));
        break;
      }
      case OP_BIT_NOT: {
        z_value a = peek(vm, 0);
        if (!IS_NUMBER(a)) {
          CLASS_UNARY_OPERATION("~");
        }
        push(vm, INTEGER_VAL(~((int) AS_NUMBER(pop(vm)))));
        break;
      }
      case OP_AND: {
        BINARY_BIT_OP(OP_AND, &);
        break;
      }
      case OP_OR: {
        BINARY_BIT_OP(OP_OR, |);
        break;
      }
      case OP_XOR: {
        BINARY_BIT_OP(OP_XOR, ^);
        break;
      }
      case OP_LSHIFT: {
        BINARY_BIT_OP(OP_LSHIFT, <<);
        break;
      }
      case OP_RSHIFT: {
        BINARY_BIT_OP(OP_RSHIFT, >>);
        break;
      }
      case OP_URSHIFT: {
        BINARY_BIT_OP(OP_URSHIFT, >>>);
        break;
      }
      case OP_ONE: {
        push(vm, NUMBER_VAL(1));
        break;
      }

        // comparisons
      case OP_EQUAL: {
        PRE_BINARY_OP();

        if(IS_INSTANCE(__a)) {
          z_value dummy;
          if(table_get(&AS_INSTANCE(__a)->klass->methods, STRING_VAL("="), &dummy)) {
            CLASS_BINARY_OPERATION("=");
          }
        }

        pop_n(vm, 2); // pop __a and __b
        push(vm, BOOL_VAL(values_equal(__a, __b)));
        break;
      }
      case OP_GREATER: {
        BINARY_OP(BOOL_VAL, >);
        break;
      }
      case OP_LESS: {
        BINARY_OP(BOOL_VAL, <);
        break;
      }

      case OP_NOT:
        push(vm, BOOL_VAL(is_false(pop(vm))));
        break;
      case OP_NIL:
        push(vm, NIL_VAL);
        break;
      case OP_EMPTY:
        push(vm, EMPTY_VAL);
        break;
      case OP_TRUE:
        push(vm, BOOL_VAL(true));
        break;
      case OP_FALSE:
        push(vm, BOOL_VAL(false));
        break;

      case OP_JUMP: {
        uint16_t offset = READ_SHORT();
        vm->current_frame->ip += offset;
        break;
      }
      case OP_JUMP_IF_FALSE: {
        uint16_t offset = READ_SHORT();
        if (is_false(peek(vm, 0))) {
          vm->current_frame->ip += offset;
        }
        break;
      }
      case OP_LOOP: {
        uint16_t offset = READ_SHORT();
        vm->current_frame->ip -= offset;
        break;
      }

      case OP_ECHO: {
        z_value val = peek(vm, 0);

        // check if it's a class with @to_string() override first.
        TRY_STRING_OVERRIDE(val);

        if (vm->is_repl) {
          echo_value(val);
        } else {
          print_value(val);
        }
        if(!IS_EMPTY(val)) {
          printf("\n");
        }
        pop(vm);
        break;
      }

      case OP_STRINGIFY: {
        z_value val = peek(vm, 0);
        if (!IS_STRING(val) && !IS_NIL(val)) {
          // check if it's a class with @to_string() override first.
          TRY_STRING_OVERRIDE(val);

          z_obj_string *value = value_to_string(vm, pop(vm));
          if (value->length != 0) {
            push(vm, OBJ_VAL(value));
          } else {
            push(vm, NIL_VAL);
          }
        }
        break;
      }

      case OP_DUP: {
        push(vm, peek(vm, 0));
        break;
      }
      case OP_POP: {
        pop(vm);
        break;
      }
      case OP_POP_N: {
        pop_n(vm, READ_SHORT());
        break;
      }
      case OP_CLOSE_UP_VALUE: {
        close_up_values(vm, vm->stack_top - 1);
        pop(vm);
        break;
      }

      case OP_DEFINE_GLOBAL: {
        z_obj_string *name = READ_STRING();
        if(IS_EMPTY(peek(vm, 0))) {
          runtime_error(ERR_CANT_ASSIGN_EMPTY);
          break;
        }
        table_set(vm, &vm->current_frame->closure->function->module->values, OBJ_VAL(name), peek(vm, 0));
        pop(vm);

#if defined(DEBUG_TABLE) && DEBUG_TABLE
        table_print(&vm->globals);
#endif
        break;
      }

      case OP_GET_GLOBAL: {
        z_obj_string *name = READ_STRING();
        z_value value;
        if (!table_get(&vm->current_frame->closure->function->module->values, OBJ_VAL(name), &value)) {
          if (!table_get(&vm->globals, OBJ_VAL(name), &value)) {
            dbg(printf("Name requested: '%s' with length %d\n", name->chars, name->length));
            cond_dbg(vm->current_frame, table_print(&vm->current_frame->closure->function->module->values));

            undefined_error("'%s' is undefined in this scope", name->chars);
            break;
          }
        }
        push(vm, value);
        break;
      }

      case OP_SET_GLOBAL: {
        if(IS_EMPTY(peek(vm, 0))) {
          runtime_error(ERR_CANT_ASSIGN_EMPTY);
          break;
        }

        z_obj_string *name = READ_STRING();
        z_table *table = &vm->current_frame->closure->function->module->values;
        if (table_set(vm, table, OBJ_VAL(name), peek(vm, 0))) {
          table_delete(table, OBJ_VAL(name));
          undefined_error("%s is undefined in this scope", name->chars);
          break;
        }
        break;
      }

      case OP_GET_LOCAL: {
        uint16_t slot = READ_SHORT();
        push(vm, vm->current_frame->slots[slot]);
        break;
      }
      case OP_SET_LOCAL: {
        uint16_t slot = READ_SHORT();
        if(IS_EMPTY(peek(vm, 0))) {
          runtime_error(ERR_CANT_ASSIGN_EMPTY);
          break;
        }
        vm->current_frame->slots[slot] = peek(vm, 0);
        break;
      }

      case OP_GET_PROPERTY: {
        z_obj_string *name = READ_STRING();

        if (IS_OBJ(peek(vm, 0))) {
          z_value value;

          switch (AS_OBJ(peek(vm, 0))->type) {
            case OBJ_MODULE: {
              z_obj_module *module = AS_MODULE(peek(vm, 0));
              if (table_get(&module->values, OBJ_VAL(name), &value)) {
                if (is_private(name)) {
                  access_error("cannot get private module property '%s'", name->chars);
                  break;
                }

                pop(vm); // pop the module...
                push(vm, value);
                break;
              }

              property_error("%s module does not define '%s'", module->name, name->chars);
              break;
            }
            case OBJ_CLASS: {
              if (table_get(&AS_CLASS(peek(vm, 0))->methods, OBJ_VAL(name), &value)) {
                if (get_method_type(value) == TYPE_STATIC) {
                  if (is_private(name)) {
                    access_error("cannot call private property '%s' of class %s",
                                  name->chars, AS_CLASS(peek(vm, 0))->name->chars);
                    break;
                  }
                  pop(vm); // pop the class...
                  push(vm, value);
                  break;
                }
              } else if (table_get(&AS_CLASS(peek(vm, 0))->static_properties, OBJ_VAL(name), &value)) {
                if (is_private(name)) {
                  access_error("cannot call private property '%s' of class %s",
                                name->chars, AS_CLASS(peek(vm, 0))->name->chars);
                  break;
                }
                pop(vm); // pop the class...
                push(vm, value);
                break;
              }

              property_error("class %s does not have a static property or method named '%s'",
                            AS_CLASS(peek(vm, 0))->name->chars, name->chars);
              break;
            }
            case OBJ_INSTANCE: {
              z_obj_instance *instance = AS_INSTANCE(peek(vm, 0));
              if (table_get(&instance->properties, OBJ_VAL(name), &value)) {
                if (is_private(name)) {
                  access_error("cannot call private property '%s' from instance of %s",
                                name->chars, instance->klass->name->chars);
                  break;
                }
                pop(vm); // pop the instance...
                push(vm, value);
                break;
              }

              if (is_private(name)) {
                access_error("cannot bind private property '%s' to instance of %s",
                              name->chars, instance->klass->name->chars);
                break;
              }

              if (bind_method(vm, instance->klass, name)) {
                break;
              }

              property_error("instance of class %s does not have a property or method named '%s'",
                            AS_INSTANCE(peek(vm, 0))->klass->name->chars, name->chars);
              break;
            }
            case OBJ_STRING: {
              if (table_get(&vm->methods_string, OBJ_VAL(name), &value)) {
                pop(vm); // pop the string...
                push(vm, value);
                break;
              }

              property_error("class String has no named property '%s'", name->chars);
              break;
            }
            case OBJ_LIST: {
              if (table_get(&vm->methods_list, OBJ_VAL(name), &value)) {
                pop(vm); // pop the list...
                push(vm, value);
                break;
              }

              property_error("class List has no named property '%s'", name->chars);
              break;
            }
            case OBJ_RANGE: {
              if (table_get(&vm->methods_range, OBJ_VAL(name), &value)) {
                pop(vm); // pop the range...
                push(vm, value);
                break;
              }

              property_error("class Range has no named property '%s'", name->chars);
              break;
            }
            case OBJ_DICT: {
              if (table_get(&AS_DICT(peek(vm, 0))->items, OBJ_VAL(name), &value) ||
                  table_get(&vm->methods_dict, OBJ_VAL(name), &value)) {
                pop(vm); // pop the dictionary...
                push(vm, value);
                break;
              }

              property_error("unknown key or class Dict property '%s'", name->chars);
              break;
            }
            case OBJ_BYTES: {
              if (table_get(&vm->methods_bytes, OBJ_VAL(name), &value)) {
                pop(vm); // pop the bytes...
                push(vm, value);
                break;
              }

              property_error("class Bytes has no named property '%s'", name->chars);
              break;
            }
            case OBJ_FILE: {
              if (table_get(&vm->methods_file, OBJ_VAL(name), &value)) {
                pop(vm); // pop the file...
                push(vm, value);
                break;
              }

              property_error("class File has no named property '%s'", name->chars);
              break;
            }
            default: {
              type_error("object of type %s does not carry properties", value_type(peek(vm, 0)));
              break;
            }
          }
        } else {
          type_error("'%s' of type %s does not have properties", value_to_string(vm, peek(vm, 0))->chars, value_type(peek(vm, 0)));
          break;
        }
        break;
      }

      case OP_GET_SELF_PROPERTY: {
        z_obj_string *name = READ_STRING();
        z_value value;

        if (IS_INSTANCE(peek(vm, 0))) {
          z_obj_instance *instance = AS_INSTANCE(peek(vm, 0));
          if (table_get(&instance->properties, OBJ_VAL(name), &value)) {
            pop(vm); // pop the instance...
            push(vm, value);
            break;
          }

          if (bind_method(vm, instance->klass, name)) {
            break;
          }

          property_error("instance of class %s does not have a property or method named '%s'",
                        AS_INSTANCE(peek(vm, 0))->klass->name->chars, name->chars);
          break;
        } else if (IS_CLASS(peek(vm, 0))) {
          z_obj_class *klass = AS_CLASS(peek(vm, 0));
          if (table_get(&klass->methods, OBJ_VAL(name), &value)) {
            if (get_method_type(value) == TYPE_STATIC) {
              pop(vm); // pop the class...
              push(vm, value);
              break;
            }
          } else if (table_get(&klass->static_properties, OBJ_VAL(name), &value)) {
            pop(vm); // pop the class...
            push(vm, value);
            break;
          }
          property_error("class %s does not have a static property or method named '%s'",
                        klass->name->chars, name->chars);
          break;
        } else if (IS_MODULE(peek(vm, 0))) {
          z_obj_module *module = AS_MODULE(peek(vm, 0));
          if (table_get(&module->values, OBJ_VAL(name), &value)) {
            pop(vm); // pop the module...
            push(vm, value);
            break;
          }

          property_error("module %s does not define '%s'", module->name, name->chars);
          break;
        }

        type_error("'%s' of type %s does not have properties", value_to_string(vm, peek(vm, 0))->chars, value_type(peek(vm, 0)));
        break;
      }

      case OP_SET_PROPERTY: {
        if (!IS_INSTANCE(peek(vm, 1)) && !IS_DICT(peek(vm, 1)) && !IS_CLASS(peek(vm, 1))) {
          type_error("object of type %s can not carry properties", value_type(peek(vm, 1)));
          break;
        } else  if(IS_EMPTY(peek(vm, 0))) {
          runtime_error(ERR_CANT_ASSIGN_EMPTY);
          break;
        }

        z_obj_string *name = READ_STRING();

        if (IS_INSTANCE(peek(vm, 1))) {
          z_obj_instance *instance = AS_INSTANCE(peek(vm, 1));
          table_set(vm, &instance->properties, OBJ_VAL(name), peek(vm, 0));

          z_value value = pop(vm);
          pop(vm); // removing the instance object
          push(vm, value);
        } else if (IS_CLASS(peek(vm, 1))) {
          z_obj_class *klass = AS_CLASS(peek(vm, 1));
          table_set(vm, &klass->static_properties, OBJ_VAL(name), peek(vm, 0));

          z_value value = pop(vm);
          pop(vm); // removing the instance object
          push(vm, value);
        } else {
          z_obj_dict *dict = AS_DICT(peek(vm, 1));
          dict_set_entry(vm, dict, OBJ_VAL(name), peek(vm, 0));

          z_value value = pop(vm);
          pop(vm); // removing the dictionary object
          push(vm, value);
        }
        break;
      }

      case OP_CLOSURE: {
        z_obj_func *function = AS_FUNCTION(READ_CONSTANT());
        z_obj_closure *closure = new_closure(vm, function);
        push(vm, OBJ_VAL(closure));

        for (int i = 0; i < closure->up_value_count; i++) {
          uint8_t is_local = READ_BYTE();
          int index = READ_SHORT();

          if (is_local) {
            closure->up_values[i] = capture_up_value(vm, vm->current_frame->slots + index);
          } else {
            closure->up_values[i] =
                ((z_obj_closure *) vm->current_frame->closure)->up_values[index];
          }
        }

        break;
      }
      case OP_GET_UP_VALUE: {
        int index = READ_SHORT();
        push(vm, *((z_obj_closure *) vm->current_frame->closure)->up_values[index]->location);
        break;
      }
      case OP_SET_UP_VALUE: {
        int index = READ_SHORT();
        if(IS_EMPTY(peek(vm, 0))) {
          runtime_error(ERR_CANT_ASSIGN_EMPTY);
          break;
        }
        *((z_obj_closure *) vm->current_frame->closure)->up_values[index]->location =
            peek(vm, 0);
        break;
      }

      case OP_CALL: {
        int arg_count = READ_BYTE();
        if (!call_value(vm, peek(vm, arg_count), arg_count)) {
          EXIT_VM();
        }
        vm->current_frame = &vm->frames[vm->frame_count - 1];
        break;
      }
      case OP_INVOKE: {
        z_obj_string *method = READ_STRING();
        int arg_count = READ_BYTE();
        if (!invoke(vm, method, arg_count)) {
          EXIT_VM();
        }
        vm->current_frame = &vm->frames[vm->frame_count - 1];
        break;
      }
      case OP_INVOKE_SELF: {
        z_obj_string *method = READ_STRING();
        int arg_count = READ_BYTE();
        if (!invoke_self(vm, method, arg_count)) {
          EXIT_VM();
        }
        vm->current_frame = &vm->frames[vm->frame_count - 1];
        break;
      }

      case OP_CLASS: {
        z_obj_string *name = READ_STRING();
        push(vm, OBJ_VAL(new_class(vm, name)));
        break;
      }
      case OP_METHOD: {
        z_obj_string *name = READ_STRING();
        define_method(vm, name);
        break;
      }
      case OP_CLASS_PROPERTY: {
        z_obj_string *name = READ_STRING();
        int is_static = READ_BYTE();
        define_property(vm, name, is_static == 1);
        break;
      }
      case OP_INHERIT: {
        if (!IS_CLASS(peek(vm, 1))) {
          type_error("cannot inherit from non-class object");
          break;
        }

        z_obj_class *superclass = AS_CLASS(peek(vm, 1));
        z_obj_class *subclass = AS_CLASS(peek(vm, 0));
        table_add_all(vm, &superclass->properties, &subclass->properties);
        table_add_all(vm, &superclass->methods, &subclass->methods);
        subclass->superclass = superclass;
        pop(vm); // pop the subclass
        break;
      }
      case OP_EXTEND: {
        z_obj_class *ext_class = AS_CLASS(peek(vm, 0));

        if (IS_STRING(peek(vm, 1))) {
          z_obj_string *klass = AS_STRING(peek(vm, 1));

          z_table *table = NULL;
          if (memcmp(klass->chars, "string", klass->length) == 0) {
            table = &vm->methods_string;
          } else if (memcmp(klass->chars, "list", klass->length) == 0) {
            table = &vm->methods_list;
          } else if (memcmp(klass->chars, "range", klass->length) == 0) {
            table = &vm->methods_range;
          } else if (memcmp(klass->chars, "dict", klass->length) == 0) {
            table = &vm->methods_dict;
          } else if (memcmp(klass->chars, "bytes", klass->length) == 0) {
            table = &vm->methods_bytes;
          } else if (memcmp(klass->chars, "file", klass->length) == 0) {
            table = &vm->methods_file;
          }

          if (table != NULL) {
            table_copy_extensions(vm, &ext_class->methods, table);
            break;
          }

          undefined_error("cannot extend undefined class %*.s", klass->length, klass->chars);
        } else if (!IS_CLASS(peek(vm, 1))) {
          type_error("cannot extend non-class object");
          break;
        }

        z_obj_class *actual_class = AS_CLASS(peek(vm, 1));
        table_copy_extensions(vm, &ext_class->methods, &actual_class->methods);
        pop(vm); // pop the subclass
        break;
      }
      case OP_GET_SUPER: {
        z_obj_string *name = READ_STRING();
        z_obj_class *klass = AS_CLASS(peek(vm, 0));
        if (!bind_method(vm, klass->superclass, name)) {
          property_error("class %s does not define a function %s", klass->name->chars, name->chars);
        }
        break;
      }
      case OP_SUPER_INVOKE: {
        z_obj_string *method = READ_STRING();
        int arg_count = READ_BYTE();
        z_obj_class *klass = AS_CLASS(pop(vm));
        if (!invoke_from_class(vm, klass, method, arg_count)) {
          EXIT_VM();
        }
        vm->current_frame = &vm->frames[vm->frame_count - 1];
        break;
      }
      case OP_SUPER_INVOKE_SELF: {
        int arg_count = READ_BYTE();
        z_obj_class *klass = AS_CLASS(pop(vm));
        if (!invoke_from_class(vm, klass, klass->name, arg_count)) {
          EXIT_VM();
        }
        vm->current_frame = &vm->frames[vm->frame_count - 1];
        break;
      }

      case OP_LIST: {
        int count = READ_SHORT();
        z_obj_list *list = new_list(vm);
        vm->stack_top[-count - 1] = OBJ_VAL(list);

        for (int i = count - 1; i >= 0; i--) {
          write_list(vm, list, peek(vm, i));
        }
        pop_n(vm, count);
        break;
      }
      case OP_RANGE: {
        z_value _upper = peek(vm, 0), _lower = peek(vm, 1);

        if (!IS_NUMBER(_upper) || !IS_NUMBER(_lower)) {
          range_error("invalid range boundaries");
          break;
        }

        double lower = AS_NUMBER(_lower), upper = AS_NUMBER(_upper);
        pop_n(vm, 2);
        push(vm, OBJ_VAL(new_range(vm, lower, upper)));
        break;
      }
      case OP_DICT: {
        int count = READ_SHORT() * 2; // 1 for key, 1 for value
        z_obj_dict *dict = new_dict(vm);
        vm->stack_top[-count - 1] = OBJ_VAL(dict);

        for (int i = 0; i < count; i += 2) {
          z_value name = vm->stack_top[-count + i];
          if(!IS_STRING(name) && !IS_NUMBER(name) && !IS_BOOL(name)) {
            type_error("dictionary key must be one of string, number or boolean");
          }
          z_value value = vm->stack_top[-count + i + 1];
          dict_set_entry(vm, dict, name, value);
        }
        pop_n(vm, count);
        break;
      }

      case OP_GET_RANGED_INDEX: {
        uint8_t will_assign = READ_BYTE();

        bool is_gotten = true;
        if (IS_OBJ(peek(vm, 2))) {
          switch (AS_OBJ(peek(vm, 2))->type) {
            case OBJ_STRING: {
              if (!string_get_ranged_index(vm, AS_STRING(peek(vm, 2)), will_assign == (uint8_t) 1)) {
                EXIT_VM();
              }
              break;
            }
            case OBJ_LIST: {
              if (!list_get_ranged_index(vm, AS_LIST(peek(vm, 2)), will_assign == (uint8_t) 1)) {
                EXIT_VM();
              }
              break;
            }
            case OBJ_BYTES: {
              if (!bytes_get_ranged_index(vm, AS_BYTES(peek(vm, 2)), will_assign == (uint8_t) 1)) {
                EXIT_VM();
              }
              break;
            }
            default: {
              is_gotten = false;
              break;
            }
          }
        } else {
          is_gotten = false;
        }

        if (!is_gotten) {
          type_error("cannot range index object of type %s", value_type(peek(vm, 2)));
        }
        break;
      }
      case OP_GET_INDEX: {
        uint8_t will_assign = READ_BYTE();

        bool is_gotten = true;
        if (IS_OBJ(peek(vm, 1))) {
          switch (AS_OBJ(peek(vm, 1))->type) {
            case OBJ_STRING: {
              if (!string_get_index(vm, AS_STRING(peek(vm, 1)), will_assign == (uint8_t) 1)) {
                EXIT_VM();
              }
              break;
            }
            case OBJ_LIST: {
              if (!list_get_index(vm, AS_LIST(peek(vm, 1)), will_assign == (uint8_t) 1)) {
                EXIT_VM();
              }
              break;
            }
            case OBJ_DICT: {
              if (!dict_get_index(vm, AS_DICT(peek(vm, 1)), will_assign == (uint8_t) 1)) {
                EXIT_VM();
              }
              break;
            }
            case OBJ_MODULE: {
              if (!module_get_index(vm, AS_MODULE(peek(vm, 1)), will_assign == (uint8_t) 1)) {
                EXIT_VM();
              }
              break;
            }
            case OBJ_BYTES: {
              if (!bytes_get_index(vm, AS_BYTES(peek(vm, 1)), will_assign == (uint8_t) 1)) {
                EXIT_VM();
              }
              break;
            }
            default: {
              is_gotten = false;
              break;
            }
          }
        } else {
          is_gotten = false;
        }

        if (!is_gotten) {
          type_error("cannot index object of type %s", value_type(peek(vm, 1)));
        }
        break;
      }

      case OP_SET_INDEX: {
        bool is_set = true;
        if (IS_OBJ(peek(vm, 2))) {

          z_value value = peek(vm, 0);
          z_value index = peek(vm, 1);

          if(IS_EMPTY(value)) {
            runtime_error(ERR_CANT_ASSIGN_EMPTY);
            break;
          }

          switch (AS_OBJ(peek(vm, 2))->type) {
            case OBJ_LIST: {
              if (!list_set_index(vm, AS_LIST(peek(vm, 2)), index, value)) {
                EXIT_VM();
              }
              break;
            }
            case OBJ_STRING: {
              numeric_error("strings do not support object assignment");
              break;
            }
            case OBJ_DICT: {
              dict_set_index(vm, AS_DICT(peek(vm, 2)), index, value);
              break;
            }
            case OBJ_MODULE: {
              module_set_index(vm, AS_MODULE(peek(vm, 2)), index, value);
              break;
            }
            case OBJ_BYTES: {
              if (!bytes_set_index(vm, AS_BYTES(peek(vm, 2)), index, value)) {
                EXIT_VM();
              }
              break;
            }
            default: {
              is_set = false;
              break;
            }
          }
        } else {
          is_set = false;
        }

        if (!is_set) {
          type_error("type of %s is not a valid iterable", value_type(peek(vm, 3)));
        }
        break;
      }

      case OP_RETURN: {
        z_value result = pop(vm);

        close_up_values(vm, vm->current_frame->slots);

        if (vm->error_count > 0 && vm->errors[vm->error_count - 1]->frame == vm->current_frame) {
          pop_error(vm);
        }

        vm->frame_count--;
        if (vm->frame_count == 0) {
          pop(vm);
          return PTR_OK;
        }

        vm->stack_top = vm->current_frame->slots;
        push(vm, result);

        vm->current_frame = &vm->frames[vm->frame_count - 1];

        if (vm->frame_count == exit_frame) {
          return PTR_OK;
        }

        break;
      }

      case OP_CALL_IMPORT: {
        z_obj_closure *closure = AS_CLOSURE(READ_CONSTANT());

        z_value existing_module;
        if(table_get(&vm->modules, STRING_VAL(closure->function->module->file), &existing_module)) {
          add_known_module(vm, AS_MODULE(existing_module), closure->function->module->name);
          // attach same module to import closure for selective import
          closure->function->module = AS_MODULE(existing_module);
        } else {
          add_module(vm, closure->function->module);
          register_module__FILE__(vm, closure->function->module);
          call(vm, closure, 0);
          vm->current_frame = &vm->frames[vm->frame_count - 1];
        }
        break;
      }

      case OP_NATIVE_MODULE: {
        z_obj_string *module_name = READ_STRING();
        z_value value;
        if (table_get(&vm->modules, OBJ_VAL(module_name), &value)) {
          z_obj_module *module = AS_MODULE(value);
          if(module->preloader != NULL) {
            ((z_module_loader)module->preloader)(vm);
          }
          module->imported = true;
          table_set(vm, &vm->current_frame->closure->function->module->values, OBJ_VAL(module_name), value);
          break;
        }
        undefined_error("module '%s' not found", module_name->chars);
        break;
      }

      case OP_SELECT_IMPORT: {
        z_obj_string *entry_name = READ_STRING();
        z_obj_func *function = AS_CLOSURE(peek(vm, 0))->function;
        z_value value;
        if (table_get(&function->module->values, OBJ_VAL(entry_name), &value)) {
          table_set(vm, &vm->current_frame->closure->function->module->values, OBJ_VAL(entry_name), value);
        } else {
          property_error("module %s does not define '%s'", function->module->name, entry_name->chars);
        }
        break;
      }

      case OP_SELECT_NATIVE_IMPORT: {
        z_obj_string *module_name = AS_STRING(peek(vm, 0));
        z_obj_string *value_name = READ_STRING();
        z_value mod;
        if (table_get(&vm->modules, OBJ_VAL(module_name), &mod)) {
          z_obj_module *module = AS_MODULE(mod);
          z_value value;
          if (table_get(&module->values, OBJ_VAL(value_name), &value)) {
            table_set(vm, &vm->current_frame->closure->function->module->values, OBJ_VAL(value_name), value);
          } else {
            property_error("module %s does not define '%s'", module->name, value_name->chars);
          }
        } else{
          undefined_error("module '%s' not found", module_name->chars);
        }
        break;
      }

      case OP_IMPORT_ALL: {
        table_import_all(vm, &AS_CLOSURE(peek(vm, 0))->function->module->values, &vm->current_frame->closure->function->module->values);
        break;
      }

      case OP_IMPORT_ALL_NATIVE: {
        z_obj_string *name = AS_STRING(peek(vm, 0));
        z_value mod;
        if (table_get(&vm->modules, OBJ_VAL(name), &mod)) {
          table_import_all(vm, &AS_MODULE(mod)->values, &vm->current_frame->closure->function->module->values);
        }
        break;
      }

      case OP_EJECT_IMPORT: {
        z_obj_func *function = AS_CLOSURE(READ_CONSTANT())->function;
        z_table *current_module = &vm->current_frame->closure->function->module->values;
        z_value module_name = STRING_VAL(function->module->name);

        z_value tmp;
        if(table_get(current_module, module_name, &tmp)) {
          if(!IS_MODULE(tmp)) {
            break;
          }
        }

        table_delete(current_module, module_name);
        break;
      }

      case OP_EJECT_NATIVE_IMPORT: {
        z_value mod;
        z_obj_string *name = READ_STRING();
        if (table_get(&vm->modules, OBJ_VAL(name), &mod)) {
          z_table *current_module = &vm->current_frame->closure->function->module->values;

          table_import_all(vm, &AS_MODULE(mod)->values, current_module);
          table_delete(current_module, OBJ_VAL(name));
        }
        break;
      }

      case OP_ASSERT: {
        z_value message = pop(vm);
        z_value expression = pop(vm);
        if (is_false(expression)) {
          if (!IS_NIL(message)) {
            do_throw_exception(vm, NULL, true, value_to_string(vm, message)->chars);
          } else {
            do_throw_exception(vm, NULL, true, "");
          }
        }
        break;
      }

      case OP_RAISE: {
        if (!IS_INSTANCE(peek(vm, 0)) ||
            !is_instance_of(AS_INSTANCE(peek(vm, 0))->klass, vm->exception_class)) {
          type_error("instance of Exception expected");
          break;
        }

        z_value exception = peek(vm, 0);
        z_value stacktrace = get_stack_trace(vm);
        z_obj_instance *instance = AS_INSTANCE(exception);
        table_set(vm, &instance->properties, STRING_L_VAL("stacktrace", 10), stacktrace);
        table_set(vm, &instance->properties, STRING_L_VAL("type", 4),
          STRING_L_VAL(instance->klass->name->chars, instance->klass->name->length)
        );
        pop(vm); // pop the exception

        if(print_exception(vm, instance, false)) {
          z_error_frame *error = peek_error(vm);
          vm->frame_count -= vm->current_frame - error->frame;
          vm->current_frame = error->frame;
          vm->current_frame->ip = &error->frame->closure->function->blob.code[error->offset];
          vm->stack_top = error->stack_head;
          break;
        }

        EXIT_VM();
        break;
      }

      case OP_SWITCH: {
        z_obj_switch *sw = AS_SWITCH(READ_CONSTANT());
        z_value expr = peek(vm, 0);

        z_value value;
        if (table_get(&sw->table, expr, &value)) {
          vm->current_frame->ip += (int) AS_NUMBER(value);
        } else if (sw->default_jump != -1) {
          vm->current_frame->ip += sw->default_jump;
        } else {
          vm->current_frame->ip += sw->exit_jump;
        }
        pop(vm);
        break;
      }

      case OP_CHOICE: {
        z_value _else = peek(vm, 0);
        z_value _then = peek(vm, 1);
        z_value _condition = peek(vm, 2);

        pop_n(vm, 3);
        if (!is_false(_condition)) {
          push(vm, _then);
        } else {
          push(vm, _else);
        }
        break;
      }

      case OP_BEGIN_CATCH: {
        uint16_t offset = READ_SHORT();

        z_error_frame *error = ALLOCATE(z_error_frame, 1);
        error->frame = vm->current_frame;
        error->offset = offset;
        error->value = NIL_VAL;
        error->stack_head = vm->stack_top;

        push_error(vm, error);
        break;
      }

      case OP_END_CATCH: {
        z_error_frame *error = pop_error(vm);
        push(vm, error->value);
        error->value = NIL_VAL;
        break;
      }

      default:
        break;
    }
  }

#undef TRY_STRING_OVERRIDE
#undef READ_BYTE
#undef READ_SHORT
#undef READ_CONSTANT
#undef READ_LCONSTANT
#undef READ_STRING
#undef READ_LSTRING
#undef BINARY_OP
#undef BINARY_MOD_OP
}

void register_module__FILE__(z_vm *vm, z_obj_module *module) {
  // register module __file__
  push(vm, STRING_L_VAL("__file__", 8));
  push(vm, STRING_VAL(module->file));
  table_set(vm, &module->values, peek(vm, 1), peek(vm, 0));
  pop_n(vm, 2);
}

void register__ROOT__(z_vm *vm) {
  // register module __file__
  push(vm, STRING_L_VAL("__root__", 8));
  push(vm, STRING_VAL(vm->root_file));
  table_set(vm, &vm->globals, peek(vm, 1), peek(vm, 0));
  pop_n(vm, 2);
}

// helper function to access call outside the vm file.
z_value raw_closure_call(z_vm *vm, z_obj_closure *closure, z_obj_list *args, bool must_push) {
  z_value *stack_top = vm->stack_top;

  // set the closure before the args
  if(must_push) {
    push(vm, OBJ_VAL(closure));
  }

  int arg_count = 0;
  if(args && (arg_count = args->items.count)) {
    for(int i = 0; i < args->items.count; i++) {
      push(vm, args->items.values[i]);
    }
  }

  call(vm, closure, arg_count);
  z_ptr_result vm_result = run(vm, vm->frame_count - 1);

  if(vm_result != PTR_OK) {
    exit(EXIT_RUNTIME);
  }


  z_value result = vm->stack_top[-1];
  pop_n(vm, arg_count + 1);

  vm->stack_top = stack_top;
  return result;
}

// helper function to access call outside the vm file.
z_ptr_result run_closure_call(z_vm *vm, z_obj_closure *closure, z_obj_list *args) {
  // set the closure before the args
  push(vm, OBJ_VAL(closure));

  int arg_count = 0;
  if(args && (arg_count = args->items.count)) {
    for(int i = 0; i < args->items.count; i++) {
      push(vm, args->items.values[i]);
    }
  }

  call(vm, closure, arg_count);
  return run(vm, vm->frame_count - 1);
}

z_value call_closure(z_vm *vm, z_obj_closure *closure, z_obj_list *args) {
  return raw_closure_call(vm, closure, args, true);
}

// helper function to access call outside the vm file.
bool queue_closure(z_vm *vm, z_obj_closure *closure) {
  return call(vm, closure, 0);
}

z_ptr_result interpret(z_vm *vm, z_obj_module *module, const char *source) {
  push(vm, OBJ_VAL(module));

  if(vm->exception_class == NULL) {
    initialize_exceptions(vm, module);
  }

  z_obj_func *function = compile(vm, module, source);
  if(function == NULL) {
    return PTR_COMPILE_ERR;
  }

  if (vm->should_exit_after_bytecode) {
    return PTR_OK;
  }

  push(vm, OBJ_VAL(function));
  z_obj_closure *closure = new_closure(vm, function);
  pop(vm);
  push(vm, OBJ_VAL(closure));

  register_module__FILE__(vm, module);

  call(vm, closure, 0);
  return run(vm, 0);
}

#undef ERR_CANT_ASSIGN_EMPTY
