#include "dict.h"

#include <stdlib.h>

#define ENFORCE_VALID_DICT_KEY(name, index)                                    \
  EXCLUDE_ARG_TYPE(name, IS_LIST, index);                                      \
  EXCLUDE_ARG_TYPE(name, IS_DICT, index); \
  EXCLUDE_ARG_TYPE(name, IS_FILE, index);

DECLARE_DICT_METHOD(length) {
  ENFORCE_ARG_COUNT(dictionary.length, 0);
  RETURN_NUMBER(AS_DICT(METHOD_OBJECT)->names.count);
}

DECLARE_DICT_METHOD(add) {
  ENFORCE_ARG_COUNT(add, 2);
  ENFORCE_VALID_DICT_KEY(add, 0);

  z_obj_dict *dict = AS_DICT(METHOD_OBJECT);

  z_value temp_value;
  if (table_get(&dict->items, args[0], &temp_value)) {
    RETURN_ERROR("duplicate key %s at add()", value_to_string(vm, args[0])->chars);
  }

  dict_add_entry(vm, dict, args[0], args[1]);
  RETURN;
}

DECLARE_DICT_METHOD(set) {
    ENFORCE_ARG_COUNT(set, 2);
    ENFORCE_VALID_DICT_KEY(set, 0);

    z_obj_dict *dict = AS_DICT(METHOD_OBJECT);
    z_value value;
    if (!table_get(&dict->items, args[0], &value)) {
        dict_add_entry(vm, dict, args[0], args[1]);
    } else {
        dict_set_entry(vm, dict, args[0], args[1]);
    }
    RETURN;
}

DECLARE_DICT_METHOD(clear) {
  ENFORCE_ARG_COUNT(dict, 0);

  z_obj_dict *dict = AS_DICT(METHOD_OBJECT);
  free_value_arr(vm, &dict->names);
  free_table(vm, &dict->items);
  RETURN;
}

DECLARE_DICT_METHOD(clone) {
  ENFORCE_ARG_COUNT(clone, 0);
  z_obj_dict *dict = AS_DICT(METHOD_OBJECT);
  z_obj_dict *n_dict = (z_obj_dict *) GC(new_dict(vm));

  table_copy(vm, &dict->items, &n_dict->items);

  for (int i = 0; i < dict->names.count; i++) {
    write_value_arr(vm, &n_dict->names, dict->names.values[i]);
  }

  RETURN_OBJ(n_dict);
}

DECLARE_DICT_METHOD(compact) {
  ENFORCE_ARG_COUNT(compact, 0);
  z_obj_dict *dict = AS_DICT(METHOD_OBJECT);
  z_obj_dict *n_dict = (z_obj_dict *) GC(new_dict(vm));

  for (int i = 0; i < dict->names.count; i++) {
    z_value tmp_value;
    table_get(&dict->items, dict->names.values[i], &tmp_value);
    if (!values_equal(tmp_value, NIL_VAL)) {
      dict_add_entry(vm, n_dict, dict->names.values[i], tmp_value);
    }
  }

  RETURN_OBJ(n_dict);
}

DECLARE_DICT_METHOD(contains) {
  ENFORCE_ARG_COUNT(contains, 1);
  ENFORCE_VALID_DICT_KEY(contains, 0);

  z_obj_dict *dict = AS_DICT(METHOD_OBJECT);
  z_value value;
  RETURN_BOOL(table_get(&dict->items, args[0], &value));
}

DECLARE_DICT_METHOD(extend) {
  ENFORCE_ARG_COUNT(extend, 1);
  ENFORCE_ARG_TYPE(extend, 0, IS_DICT);

  z_obj_dict *dict = AS_DICT(METHOD_OBJECT);
  z_obj_dict *dict_cpy = AS_DICT(args[0]);

  for (int i = 0; i < dict_cpy->names.count; i++) {
    z_value tmp;
    if(!table_get(&dict->items, dict_cpy->names.values[i], &tmp)) {
      write_value_arr(vm, &dict->names, dict_cpy->names.values[i]);
    }
  }
  table_add_all(vm, &dict_cpy->items, &dict->items);
  RETURN;
}

DECLARE_DICT_METHOD(get) {
  ENFORCE_ARG_RANGE(get, 1, 2);
  ENFORCE_VALID_DICT_KEY(get, 0);

  z_obj_dict *dict = AS_DICT(METHOD_OBJECT);
  z_value value;
  if (!dict_get_entry(dict, args[0], &value)) {
    if (arg_count == 1) {
      RETURN_NIL;
    } else {
      RETURN_VALUE(args[1]); // return default
    }
  }

  RETURN_VALUE(value);
}

DECLARE_DICT_METHOD(keys) {
  ENFORCE_ARG_COUNT(keys, 0);
  z_obj_dict *dict = AS_DICT(METHOD_OBJECT);
  z_obj_list *list = (z_obj_list *) GC(new_list(vm));
  for (int i = 0; i < dict->names.count; i++) {
    write_list(vm, list, dict->names.values[i]);
  }
  RETURN_OBJ(list);
}

DECLARE_DICT_METHOD(values) {
  ENFORCE_ARG_COUNT(values, 0);
  z_obj_dict *dict = AS_DICT(METHOD_OBJECT);
  z_obj_list *list = (z_obj_list *) GC(new_list(vm));
  for (int i = 0; i < dict->names.count; i++) {
    z_value tmp_value;
    dict_get_entry(dict, dict->names.values[i], &tmp_value);
    write_list(vm, list, tmp_value);
  }
  RETURN_OBJ(list);
}

DECLARE_DICT_METHOD(remove) {
  ENFORCE_ARG_COUNT(remove, 1);
  ENFORCE_VALID_DICT_KEY(remove, 0);

  z_obj_dict *dict = AS_DICT(METHOD_OBJECT);
  z_value value;
  if (table_get(&dict->items, args[0], &value)) {
    table_delete(&dict->items, args[0]);
    int index = -1;
    for (int i = 0; i < dict->names.count; i++) {
      if (values_equal(dict->names.values[i], args[0])) {
        index = i;
        break;
      }
    }

    for (int i = index; i < dict->names.count; i++) {
      dict->names.values[i] = dict->names.values[i + 1];
    }
    dict->names.count--;
    RETURN_VALUE(value);
  }
  RETURN_NIL;
}

DECLARE_DICT_METHOD(is_empty) {
  ENFORCE_ARG_COUNT(is_empty, 0);
  RETURN_BOOL(AS_DICT(METHOD_OBJECT)->names.count == 0);
}

DECLARE_DICT_METHOD(find_key) {
  ENFORCE_ARG_COUNT(find_key, 1);
  RETURN_VALUE(table_find_key(&AS_DICT(METHOD_OBJECT)->items, args[0]));
}

DECLARE_DICT_METHOD(to_list) {
  ENFORCE_ARG_COUNT(to_list, 0);

  z_obj_dict *dict = AS_DICT(METHOD_OBJECT);
  z_obj_list *name_list = (z_obj_list *) GC(new_list(vm));
  z_obj_list *value_list = (z_obj_list *) GC(new_list(vm));
  for (int i = 0; i < dict->names.count; i++) {
    write_list(vm, name_list, dict->names.values[i]);
    z_value value;
    if (table_get(&dict->items, dict->names.values[i], &value)) {
      write_list(vm, value_list, value);
    } else { // theoretically impossible
      write_list(vm, value_list, NIL_VAL);
    }
  }

  z_obj_list *list = (z_obj_list *) GC(new_list(vm));
  write_list(vm, list, OBJ_VAL(name_list));
  write_list(vm, list, OBJ_VAL(value_list));

  RETURN_OBJ(list);
}

DECLARE_DICT_METHOD(__iter__) {
  ENFORCE_ARG_COUNT(__iter__, 1);
  z_obj_dict *dict = AS_DICT(METHOD_OBJECT);

  z_value result;
  if (table_get(&dict->items, args[0], &result)) {
    RETURN_VALUE(result);
  }

  RETURN_NIL;
}

DECLARE_DICT_METHOD(__itern__) {
  ENFORCE_ARG_COUNT(__itern__, 1);
  z_obj_dict *dict = AS_DICT(METHOD_OBJECT);

  if (IS_NIL(args[0])) {
    if (dict->names.count == 0) RETURN_FALSE;
    RETURN_VALUE(dict->names.values[0]);
  }

  for (int i = 0; i < dict->names.count; i++) {
    if (values_equal(args[0], dict->names.values[i]) &&
        (i + 1) < dict->names.count) {
      RETURN_VALUE(dict->names.values[i + 1]);
    }
  }

  RETURN_NIL;
}

#undef ENFORCE_VALID_DICT_KEY