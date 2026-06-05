#include <zuri.h>
#include <sqlite3.h>

void sqlite_bind_params(sqlite3_stmt *stmt, int index, z_value value, int *error) {
  if(IS_NUMBER(value)) {
    double number = AS_NUMBER(value);
    if((int)number == number) {
      sqlite3_bind_int(stmt, index, (int)number);
    } else {
      sqlite3_bind_double(stmt, index, number);
    }
  } else if(IS_STRING(value)) {
    z_obj_string *str = AS_STRING(value);
    sqlite3_bind_text(stmt, index, str->chars, str->length, 0);
  } else if(IS_BYTES(value)) {
    z_obj_bytes *blob = AS_BYTES(value);
    sqlite3_bind_blob(stmt, index, blob->bytes.bytes, blob->bytes.count, SQLITE_STATIC);
  } else if(IS_NIL(value)) {
    sqlite3_bind_null(stmt, index);
  } else {
    *error = -1;
  }
}

DECLARE_MODULE_METHOD(sqlite__open) {
  ENFORCE_ARG_COUNT(_open, 1);
  ENFORCE_ARG_TYPE(_open, 0, IS_STRING);
  z_obj_string *path = AS_STRING(args[0]);
  sqlite3 *db;
  int rc = sqlite3_open(path->chars, &db);
  if(rc != SQLITE_OK) {
    const char *error = sqlite3_errmsg(db);
    sqlite3_close(db);
    RETURN_STRING(error);
  }
  z_obj_ptr *ptr = (z_obj_ptr *)GC(new_ptr(vm, (void*)db));
  ptr->name = "<SQLite3 *>";
  RETURN_OBJ(ptr);
}

DECLARE_MODULE_METHOD(sqlite__close) {
  ENFORCE_ARG_COUNT(_close, 1);
  ENFORCE_ARG_TYPE(_close, 0, IS_PTR);
  sqlite3 *db = AS_PTR(args[0])->pointer;
  if(db != NULL) {
    sqlite3_close(db);
    RETURN_TRUE;
  }
  RETURN_FALSE;
}

// this function doesn't return a result...
// it's similar to Android's SQLiteDatabase db.execSQL (or something like that. Can't remember!)
DECLARE_MODULE_METHOD(sqlite__exec) {
  ENFORCE_ARG_COUNT(_exec, 3);
  ENFORCE_ARG_TYPE(_exec, 0, IS_PTR);
  ENFORCE_ARG_TYPE(_exec, 1, IS_STRING);
  sqlite3 *db = AS_PTR(args[0])->pointer;
  z_obj_string *query = AS_STRING(args[1]);

  if(db != NULL) {
    if(IS_NIL(args[2])) {
      char *err_msg = 0;
      if (sqlite3_exec(db, query->chars, 0, 0, &err_msg) != SQLITE_OK) {
        RETURN_TT_STRING(err_msg);
      }
      RETURN_TRUE;
    } else {
      if(!IS_LIST(args[2]) && !IS_DICT(args[2])) {
        RETURN_ARGUMENT_ERROR("params must be a list or dictionary");
      }

      sqlite3_stmt *stmt;
      if(sqlite3_prepare_v2(db, query->chars, query->length, &stmt, 0) == SQLITE_OK) {
        int total_params_bindable = sqlite3_bind_parameter_count(stmt);
        if(IS_LIST(args[2])) {
          z_obj_list *params = AS_LIST(args[2]);

          if(params->items.count != total_params_bindable) {
            RETURN_ARGUMENT_ERROR("expected %d params, %d given", total_params_bindable, params->items.count);
          }

          for(int i = 0; i < params->items.count; i++) {
            int error = 0;
            sqlite_bind_params(stmt, i + 1, params->items.values[i], &error);
            if(error == -1) {
              RETURN_ERROR("could not bind invalid value at index %d", i + 1);
            }
          }
        } else if(IS_DICT(args[2])) {
          z_obj_dict *params = AS_DICT(args[2]);

          if(params->names.count != total_params_bindable) {
            RETURN_ARGUMENT_ERROR("expected %d params, %d given", total_params_bindable, params->names.count);
          }

          for(int i = 0; i < params->names.count; i++) {
            if(!IS_STRING(params->names.values[i])) {
              RETURN_ARGUMENT_ERROR("SQL params dictionary key must be a string");
            }
            int index = sqlite3_bind_parameter_index(stmt, AS_C_STRING(params->names.values[i]));
            z_value value;
            int error = 0;
            table_get(&params->items, params->names.values[i], &value);
            sqlite_bind_params(stmt, index, value, &error);
            if(error == -1) {
              RETURN_ERROR("could not bind invalid value at index '%s'", AS_C_STRING(params->names.values[i]));
            }
          }
        } else if(total_params_bindable != 0) {
          RETURN_ARGUMENT_ERROR("expected %d params, 0 given", total_params_bindable);
        }

        if(sqlite3_step(stmt) != SQLITE_DONE) {
          const char *error = sqlite3_errmsg(db);
          RETURN_STRING(error);
        }

        sqlite3_finalize(stmt);

        RETURN_TRUE;
      } else {
        const char *error = sqlite3_errmsg(db);
        RETURN_STRING(error);
      }
    }
  }
  RETURN_FALSE;
}

DECLARE_MODULE_METHOD(sqlite__last_insert_id) {
  ENFORCE_ARG_COUNT(_last_insert_id, 1);
  ENFORCE_ARG_TYPE(_last_insert_id, 0, IS_PTR);
  sqlite3 *db = AS_PTR(args[0])->pointer;
  if(db != NULL) {
    RETURN_NUMBER(sqlite3_last_insert_rowid(db));
  }
  RETURN_NUMBER(-1);
}

DECLARE_MODULE_METHOD(sqlite__query) {
  ENFORCE_ARG_COUNT(_query, 3);
  ENFORCE_ARG_TYPE(_query, 0, IS_PTR);
  ENFORCE_ARG_TYPE(_query, 1, IS_STRING);
  if(!IS_NIL(args[2]) && !IS_LIST(args[2]) && !IS_DICT(args[2])) {
    RETURN_ARGUMENT_ERROR("params must be a list or dictionary");
  }

  sqlite3 *db = AS_PTR(args[0])->pointer;
  if(db != NULL) {
    z_obj_string *query = AS_STRING(args[1]);
    sqlite3_stmt *stmt;
    if(sqlite3_prepare_v2(db, query->chars, query->length, &stmt, 0) == SQLITE_OK) {
      int total_params_bindable = sqlite3_bind_parameter_count(stmt);
      if(IS_LIST(args[2])) {
        z_obj_list *params = AS_LIST(args[2]);

        if(params->items.count != total_params_bindable) {
          RETURN_ARGUMENT_ERROR("expected %d params, %d given", total_params_bindable, params->items.count);
        }

        for(int i = 0; i < params->items.count; i++) {
          int error = 0;
          sqlite_bind_params(stmt, i + 1, params->items.values[i], &error);
          if(error == -1) {
            RETURN_ERROR("could not bind invalid value at index %d", i + 1);
          }
        }
      } else if(IS_DICT(args[2])) {
        z_obj_dict *params = AS_DICT(args[2]);

        if(params->names.count != total_params_bindable) {
          RETURN_ARGUMENT_ERROR("expected %d params, %d given", total_params_bindable, params->names.count);
        }

        for(int i = 0; i < params->names.count; i++) {
          if(!IS_STRING(params->names.values[i])) {
            RETURN_ARGUMENT_ERROR("SQL params dictionary key must be a string");
          }
          int index = sqlite3_bind_parameter_index(stmt, AS_C_STRING(params->names.values[i]));
          z_value value;
          int error = 0;
          table_get(&params->items, params->names.values[i], &value);
          sqlite_bind_params(stmt, index, value, &error);
          if(error == -1) {
            RETURN_ERROR("could not bind invalid value at index '%s'", AS_C_STRING(params->names.values[i]));
          }
        }
      } else if(total_params_bindable != 0) {
        RETURN_ARGUMENT_ERROR("expected %d params, 0 given", total_params_bindable);
      }

      z_obj_ptr *ptr = (z_obj_ptr *)GC(new_ptr(vm, (void*)stmt));
      ptr->name = "<SQLiteCursor *>";
      RETURN_OBJ(ptr);
    } else {
      const char *error = sqlite3_errmsg(db);
      RETURN_STRING(error);
    }
  }
  RETURN_ERROR("invalid SQLite pointer");
}

DECLARE_MODULE_METHOD(sqlite__cursor_changes) {
  ENFORCE_ARG_COUNT(_cursor_changes, 1);
  ENFORCE_ARG_TYPE(_cursor_changes, 0, IS_PTR);
  sqlite3 *db = AS_PTR(args[0])->pointer;
  if(db != NULL) {
    RETURN_NUMBER(sqlite3_changes(db));
  }
  RETURN_NUMBER(-1);
}

DECLARE_MODULE_METHOD(sqlite__cursor_columns) {
  ENFORCE_ARG_COUNT(_cursor_colcount, 1);
  ENFORCE_ARG_TYPE(_cursor_colcount, 0, IS_PTR);
  sqlite3_stmt *stmt = AS_PTR(args[0])->pointer;
  z_obj_list *list = (z_obj_list*)GC(new_list(vm));
  if(stmt != NULL) {
    int count = sqlite3_column_count(stmt);
    for(int i = 0; i < count; i++) {
      const char *name = sqlite3_column_name(stmt, i);
      write_list(vm, list, GC_STRING(name));
    }
  }
  RETURN_OBJ(list);
}

DECLARE_MODULE_METHOD(sqlite__cursor_has_next) {
  ENFORCE_ARG_COUNT(_cursor_has_next, 1);
  ENFORCE_ARG_TYPE(_cursor_has_next, 0, IS_PTR);
  sqlite3_stmt *stmt = AS_PTR(args[0])->pointer;
  if(stmt != NULL) {
    int state = sqlite3_step(stmt);
    if(state == SQLITE_DONE) {
      sqlite3_reset(stmt);
      RETURN_FALSE;
    } else if(state == SQLITE_ERROR) {
      const char *err_msg = sqlite3_errmsg(sqlite3_db_handle(stmt));
      RETURN_STRING(err_msg);
    } else {
      RETURN_BOOL(state == SQLITE_ROW);
    }
  }
  RETURN_FALSE;
}

DECLARE_MODULE_METHOD(sqlite__cursor_close) {
  ENFORCE_ARG_COUNT(_cursor_close, 1);
  ENFORCE_ARG_TYPE(_cursor_close, 0, IS_PTR);
  sqlite3_stmt *stmt = AS_PTR(args[0])->pointer;
  if(stmt != NULL) {
    RETURN_BOOL(sqlite3_finalize(stmt) == SQLITE_OK);
  }
  RETURN_BOOL(false);
}

DECLARE_MODULE_METHOD(sqlite__cursor_get) {
  ENFORCE_ARG_COUNT(_cursor_close, 2);
  ENFORCE_ARG_TYPE(_cursor_close, 0, IS_PTR);
  ENFORCE_ARG_TYPE(_cursor_close, 1, IS_NUMBER);
  sqlite3_stmt *stmt = AS_PTR(args[0])->pointer;
  int index = AS_NUMBER(args[1]);
  if(stmt != NULL) {
    int col_type = sqlite3_column_type(stmt, index);
    switch (col_type) {
      case SQLITE_INTEGER: {
        RETURN_NUMBER(sqlite3_column_int(stmt, index));
      }
      case SQLITE_FLOAT: {
        RETURN_NUMBER(sqlite3_column_double(stmt, index));
      }
      case SQLITE_NULL: {
        RETURN_NIL;
      }
      case SQLITE_TEXT: {
        const unsigned char *text = sqlite3_column_text(stmt, index);
        RETURN_STRING((char *)text);
      }
      case SQLITE_BLOB: {
        unsigned char *data = (unsigned char *)sqlite3_column_blob(stmt, index);
        z_obj_bytes *bytes = (z_obj_bytes *)GC(new_bytes(vm, sizeof(data)));
        bytes->bytes.bytes = data;
        RETURN_OBJ(bytes);
      }
      default: RETURN_NIL;
    }
  }
  RETURN_NIL;
}

CREATE_MODULE_LOADER(sqlite) {
  static z_func_reg module_functions[] = {
      {"_open",   true,  GET_MODULE_METHOD(sqlite__open)},
      {"_close",   true,  GET_MODULE_METHOD(sqlite__close)},
      {"_exec",   true,  GET_MODULE_METHOD(sqlite__exec)},
      {"_last_insert_id",   true,  GET_MODULE_METHOD(sqlite__last_insert_id)},
      {"_query",   true,  GET_MODULE_METHOD(sqlite__query)},
      {"_cursor_columns",   true,  GET_MODULE_METHOD(sqlite__cursor_columns)},
      {"_cursor_changes",   true,  GET_MODULE_METHOD(sqlite__cursor_changes)},
      {"_cursor_has_next",   true,  GET_MODULE_METHOD(sqlite__cursor_has_next)},
      {"_cursor_close",   true,  GET_MODULE_METHOD(sqlite__cursor_close)},
      {"_cursor_get",   true,  GET_MODULE_METHOD(sqlite__cursor_get)},
      {NULL,    false, NULL},
  };

  static z_module_reg module = {
      .name = "_sqlite",
      .fields = NULL,
      .functions = module_functions,
      .classes = NULL,
      .preloader = NULL,
      .unloader = NULL
  };

  return &module;
}