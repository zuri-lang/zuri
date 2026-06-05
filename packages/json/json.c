#include <zuri.h>
#include "json.h"

z_value get_zuri_value(z_vm *vm, json_value * data) {
  z_value value;
  switch (data->type) {
    case json_object: {
      z_obj_dict *dict = (z_obj_dict*)GC(new_dict(vm));
      value = OBJ_VAL(dict);
      for (int i = 0; i < data->u.object.length; i++) {
        z_obj_string *name = (z_obj_string *)GC(copy_string(vm, data->u.object.values[i].name, strlen(data->u.object.values[i].name)));
        z_value _value = get_zuri_value(vm, data->u.object.values[i].value);
        dict_set_entry(vm, dict, OBJ_VAL(name), _value);
      }
      break;
    }
    case json_array: {
      z_obj_list *list = (z_obj_list*)GC(new_list(vm));
      value = OBJ_VAL(list);
      for (int i = 0; i < data->u.array.length; i++) {
        write_list(vm, list, get_zuri_value(vm, data->u.array.values[i]));
      }
      break;
    }
    case json_integer: {
      value = NUMBER_VAL(data->u.integer);
      break;
    }
    case json_double: {
      value = NUMBER_VAL(data->u.dbl);
      break;
    }
    case json_string: {
      value = GC_L_STRING(data->u.string.ptr, data->u.string.length);
      break;
    }
    case json_boolean: {
      value = BOOL_VAL((long)data->u.boolean);
      break;
    }
    default: {
      // covers json_null, json_none
      value = NIL_VAL;
      break;
    }
  }
  return value;
}


DECLARE_MODULE_METHOD(json__decode) {
  ENFORCE_ARG_RANGE(decode, 1, 2);
  ENFORCE_ARG_TYPE(decode, 0, IS_STRING);
  z_obj_string *data = AS_STRING(args[0]);

  json_settings settings;
  memset(&settings, 0, sizeof (json_settings));
  if(arg_count == 2 && !is_false(args[1])) {
    settings.settings = json_enable_comments;
  }
  char error[256];
  json_value * value = json_parse_ex(&settings, data->chars, data->length, error);
  if (value == 0) {
    RETURN_ERROR(error);
  }
  z_value converted = get_zuri_value(vm, value);
  json_value_free(value);

  RETURN_VALUE(converted);
}


CREATE_MODULE_LOADER(json) {
  static z_func_reg module_functions[] = {
      {"_decode",   true,  GET_MODULE_METHOD(json__decode)},
      {NULL,    false, NULL},
  };

  static z_module_reg module = {
      .name = "_json",
      .fields = NULL,
      .functions = module_functions,
      .classes = NULL,
      .preloader = NULL,
      .unloader = NULL
  };

  return &module;
}