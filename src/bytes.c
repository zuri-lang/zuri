#include "bytes.h"
#include "memory.h"

#include <ctype.h>
#include <string.h>

DECLARE_NATIVE(bytes) {
  ENFORCE_ARG_COUNT(bytes, 1);
  if (IS_NUMBER(args[0])) {
    RETURN_OBJ(new_bytes(vm, (int) AS_NUMBER(args[0])));
  } else if (IS_LIST(args[0])) {
    z_obj_list *list = AS_LIST(args[0]);
    z_obj_bytes *bytes = (z_obj_bytes *)GC(new_bytes(vm, list->items.count));

    for (int i = 0; i < list->items.count; i++) {
      if (IS_NUMBER(list->items.values[i])) {
        bytes->bytes.bytes[i] = (unsigned char) AS_NUMBER(list->items.values[i]);
      } else {
        bytes->bytes.bytes[i] = 0;
      }
    }

    RETURN_OBJ(bytes);
  }

  RETURN_ARGUMENT_ERROR("expected bytes size or bytes list as argument");
}

DECLARE_BYTES_METHOD(length) {
  ENFORCE_ARG_COUNT(length, 0);
  RETURN_NUMBER(AS_BYTES(METHOD_OBJECT)->bytes.count);
}

DECLARE_BYTES_METHOD(append) {
  ENFORCE_ARG_COUNT(append, 1);

  if (IS_NUMBER(args[0])) {
    int byte = (int) AS_NUMBER(args[0]);
    if (byte < 0 || byte > 255) {
      RETURN_RANGE_ERROR("invalid byte. bytes range from 0 to 255");
    }

    // append here...
    z_obj_bytes *bytes = AS_BYTES(METHOD_OBJECT);
    int old_count = bytes->bytes.count;
    bytes->bytes.count++;
    bytes->bytes.bytes = GROW_ARRAY(unsigned char, bytes->bytes.bytes, old_count,
                                    bytes->bytes.count);
    bytes->bytes.bytes[bytes->bytes.count - 1] = (unsigned char) byte;
    RETURN;
  } else if (IS_LIST(args[0])) {
    z_obj_list *list = AS_LIST(args[0]);
    if (list->items.count > 0) {
      // append here...
      z_obj_bytes *bytes = AS_BYTES(METHOD_OBJECT);
      bytes->bytes.bytes =
          GROW_ARRAY(unsigned char, bytes->bytes.bytes, bytes->bytes.count,
                     (size_t) bytes->bytes.count + (size_t) list->items.count);
      if(bytes->bytes.bytes == NULL) {
        RETURN_ERROR("out of memory");
      }

      for (int i = 0; i < list->items.count; i++) {
        if (!IS_NUMBER(list->items.values[i])) {
          RETURN_TYPE_ERROR("bytes lists can only contain numbers");
        }

        int byte = (int) AS_NUMBER(list->items.values[i]);
        if (byte < 0 || byte > 255) {
          RETURN_RANGE_ERROR("invalid byte. bytes range from 0 to 255");
        }

        bytes->bytes.bytes[bytes->bytes.count + i] = (unsigned char) byte;
      }

      bytes->bytes.count += list->items.count;
    }
    RETURN;
  }

  RETURN_ARGUMENT_ERROR("bytes can only append a byte or a list of bytes");
}

DECLARE_BYTES_METHOD(clone) {
  ENFORCE_ARG_COUNT(clone, 0);
  z_obj_bytes *bytes = AS_BYTES(METHOD_OBJECT);
  z_obj_bytes *n_bytes = (z_obj_bytes *)GC(new_bytes(vm, bytes->bytes.count));

  memcpy(n_bytes->bytes.bytes, bytes->bytes.bytes, bytes->bytes.count);

  RETURN_OBJ(n_bytes);
}

DECLARE_BYTES_METHOD(extend) {
  ENFORCE_ARG_COUNT(extend, 1);
  ENFORCE_ARG_TYPE(extend, 0, IS_BYTES);
  z_obj_bytes *bytes = AS_BYTES(METHOD_OBJECT);
  z_obj_bytes *n_bytes = AS_BYTES(args[0]);

  bytes->bytes.bytes = GROW_ARRAY(unsigned char, bytes->bytes.bytes, bytes->bytes.count,
                                  bytes->bytes.count + n_bytes->bytes.count);
  if(bytes->bytes.bytes == NULL) {
    RETURN_ERROR("out of memory");
  }

  memcpy(bytes->bytes.bytes + bytes->bytes.count, n_bytes->bytes.bytes,
         n_bytes->bytes.count);
  bytes->bytes.count += n_bytes->bytes.count;
  RETURN_OBJ(bytes);
}

DECLARE_BYTES_METHOD(index_of) {
  ENFORCE_ARG_RANGE(index_of, 1, 2);
  ENFORCE_ARG_TYPES(index_of, 0, IS_NUMBER, IS_BYTES);
  z_obj_bytes *bytes = AS_BYTES(METHOD_OBJECT);
  
  int start_index = 0;
  if(arg_count == 2) {
    ENFORCE_ARG_TYPE(index_of, 1, IS_NUMBER);
    start_index = AS_NUMBER(args[1]);
  }

  if(IS_NUMBER(args[0])) {
    uint8_t needle = AS_NUMBER(args[0]);
    if(bytes->bytes.count > 0 && start_index >= 0 && start_index < bytes->bytes.count - 1) {
      for (int i = start_index; i < bytes->bytes.count; i++) {
        if (bytes->bytes.bytes[i] == needle) {
          RETURN_NUMBER(i);
        }
      }
    }
  } else {
    z_obj_bytes *needle = AS_BYTES(args[0]);

    if(bytes->bytes.count > 0 && start_index >= 0 && start_index < bytes->bytes.count - 1) {
      for (int i = start_index; i < bytes->bytes.count; i++) {
        if (memcmp(bytes->bytes.bytes + i, needle->bytes.bytes, needle->bytes.count) == 0) {
          RETURN_NUMBER(i);
        }
      }
    }
  }

  RETURN_NUMBER(-1);
}

DECLARE_BYTES_METHOD(pop) {
  ENFORCE_ARG_COUNT(pop, 0);
  z_obj_bytes *bytes = AS_BYTES(METHOD_OBJECT);
  unsigned char c = bytes->bytes.bytes[bytes->bytes.count - 1];
  bytes->bytes.count--;
  RETURN_NUMBER((double) ((int) c));
}

DECLARE_BYTES_METHOD(remove) {
  ENFORCE_ARG_COUNT(remove, 1);
  ENFORCE_ARG_TYPE(remove, 0, IS_NUMBER);

  z_obj_bytes *bytes = AS_BYTES(METHOD_OBJECT);
  int index = AS_NUMBER(args[0]);

  if (index < 0 || index >= bytes->bytes.count) {
    RETURN_RANGE_ERROR("bytes index %d out of range", index);
  }

  unsigned char val = bytes->bytes.bytes[index];

  for (int i = index; i < bytes->bytes.count; i++) {
    bytes->bytes.bytes[i] = bytes->bytes.bytes[i + 1];
  }
  bytes->bytes.count--;

  RETURN_NUMBER((double) ((int) val));
}

DECLARE_BYTES_METHOD(reverse) {
  ENFORCE_ARG_COUNT(reverse, 0);
  z_obj_bytes *bytes = AS_BYTES(METHOD_OBJECT);

  z_obj_bytes *n_bytes = (z_obj_bytes *)GC(new_bytes(vm, bytes->bytes.count));

  for (int i = 0; i < bytes->bytes.count; i++) {
    n_bytes->bytes.bytes[i] = bytes->bytes.bytes[bytes->bytes.count - i - 1];
  }

  RETURN_OBJ(n_bytes);
}

DECLARE_BYTES_METHOD(split) {
  ENFORCE_ARG_COUNT(split, 1);
  ENFORCE_ARG_TYPE(split, 0, IS_BYTES);

  z_byte_arr object = AS_BYTES(METHOD_OBJECT)->bytes;
  z_byte_arr delimeter = AS_BYTES(args[0])->bytes;

  if (object.count == 0 || delimeter.count > object.count) RETURN_OBJ(new_list(vm));

  z_obj_list *list = (z_obj_list *) GC(new_list(vm));

  // main work here...
  if (delimeter.count > 0) {
    int start = 0;
    for(int i = 0; i <= object.count; i++) {
      // match found.
      if(memcmp(object.bytes + i, delimeter.bytes, delimeter.count) == 0 || i == object.count) {
        z_obj_bytes *bytes = (z_obj_bytes *)GC(new_bytes(vm, i - start));
        memcpy(bytes->bytes.bytes, object.bytes + start, i - start);
        write_list(vm, list, OBJ_VAL(bytes));
        i += delimeter.count - 1;
        start = i + 1;
      }
    }
  } else {
    int length = object.count;
    for (int i = 0; i < length; i++) {
      z_obj_bytes *bytes = (z_obj_bytes *)GC(new_bytes(vm, 1));
      memcpy(bytes->bytes.bytes, object.bytes + i, 1);
      write_list(vm, list, OBJ_VAL(bytes));
    }
  }

  RETURN_OBJ(list);
}

DECLARE_BYTES_METHOD(first) {
  ENFORCE_ARG_COUNT(first, 0);
  RETURN_NUMBER((double) ((int) AS_BYTES(METHOD_OBJECT)->bytes.bytes[0]));
}

DECLARE_BYTES_METHOD(last) {
  ENFORCE_ARG_COUNT(first, 0);
  z_obj_bytes *bytes = AS_BYTES(METHOD_OBJECT);
  RETURN_NUMBER((double) ((int) bytes->bytes.bytes[bytes->bytes.count - 1]));
}

DECLARE_BYTES_METHOD(get) {
  ENFORCE_ARG_COUNT(get, 1);
  ENFORCE_ARG_TYPE(get, 0, IS_NUMBER);

  z_obj_bytes *bytes = AS_BYTES(METHOD_OBJECT);
  int index = AS_NUMBER(args[0]);
  if (index < 0 || index >= bytes->bytes.count) {
    RETURN_RANGE_ERROR("bytes index %d out of range", index);
  }

  RETURN_NUMBER((double) ((int) bytes->bytes.bytes[index]));
}

DECLARE_BYTES_METHOD(is_alpha) {
  ENFORCE_ARG_COUNT(is_alpha, 0);
  z_obj_bytes *bytes = AS_BYTES(METHOD_OBJECT);

  for (int i = 0; i < bytes->bytes.count; i++) {
    if (!isalpha(bytes->bytes.bytes[i])) {
      RETURN_FALSE;
    }
  }
  RETURN_TRUE;
}

DECLARE_BYTES_METHOD(is_alnum) {
  ENFORCE_ARG_COUNT(is_alnum, 0);
  z_obj_bytes *bytes = AS_BYTES(METHOD_OBJECT);

  for (int i = 0; i < bytes->bytes.count; i++) {
    if (!isalnum(bytes->bytes.bytes[i])) {
      RETURN_FALSE;
    }
  }
  RETURN_TRUE;
}

DECLARE_BYTES_METHOD(is_number) {
  ENFORCE_ARG_COUNT(is_number, 0);
  z_obj_bytes *bytes = AS_BYTES(METHOD_OBJECT);

  for (int i = 0; i < bytes->bytes.count; i++) {
    if (!isdigit(bytes->bytes.bytes[i])) {
      RETURN_FALSE;
    }
  }
  RETURN_TRUE;
}

DECLARE_BYTES_METHOD(is_lower) {
  ENFORCE_ARG_COUNT(is_lower, 0);
  z_obj_bytes *bytes = AS_BYTES(METHOD_OBJECT);

  for (int i = 0; i < bytes->bytes.count; i++) {
    if (!islower(bytes->bytes.bytes[i])) {
      RETURN_FALSE;
    }
  }
  RETURN_TRUE;
}

DECLARE_BYTES_METHOD(is_upper) {
  ENFORCE_ARG_COUNT(is_upper, 0);
  z_obj_bytes *bytes = AS_BYTES(METHOD_OBJECT);

  for (int i = 0; i < bytes->bytes.count; i++) {
    if (!isupper(bytes->bytes.bytes[i])) {
      RETURN_FALSE;
    }
  }
  RETURN_TRUE;
}

DECLARE_BYTES_METHOD(is_space) {
  ENFORCE_ARG_COUNT(is_space, 0);
  z_obj_bytes *bytes = AS_BYTES(METHOD_OBJECT);

  for (int i = 0; i < bytes->bytes.count; i++) {
    if (!isspace(bytes->bytes.bytes[i])) {
      RETURN_FALSE;
    }
  }
  RETURN_TRUE;
}

DECLARE_BYTES_METHOD(dispose) {
  ENFORCE_ARG_COUNT(dispose, 0);
  z_obj_bytes *bytes = AS_BYTES(METHOD_OBJECT);
  free_byte_arr(vm, &bytes->bytes);
  RETURN;
}

DECLARE_BYTES_METHOD(to_list) {
  ENFORCE_ARG_COUNT(to_list, 0);
  z_obj_bytes *bytes = AS_BYTES(METHOD_OBJECT);
  z_obj_list *list = (z_obj_list *) GC(new_list(vm));

  for (int i = 0; i < bytes->bytes.count; i++) {
    write_list(vm, list, NUMBER_VAL((double) ((int) bytes->bytes.bytes[i])));
  }

  RETURN_OBJ(list);
}

DECLARE_BYTES_METHOD(to_string) {
  ENFORCE_ARG_COUNT(to_string, 0);
  z_obj_bytes *bytes = AS_BYTES(METHOD_OBJECT);

  char *string = (char *) bytes->bytes.bytes;
  RETURN_L_STRING(string, bytes->bytes.count);
}

DECLARE_BYTES_METHOD(__iter__) {
  ENFORCE_ARG_COUNT(__iter__, 1);
  ENFORCE_ARG_TYPE(__iter__, 0, IS_NUMBER);

  z_obj_bytes *bytes = AS_BYTES(METHOD_OBJECT);

  int index = AS_NUMBER(args[0]);

  if (index > -1 && index < bytes->bytes.count) {
    RETURN_NUMBER((int) bytes->bytes.bytes[index]);
  }

  RETURN_NIL;
}

DECLARE_BYTES_METHOD(__itern__) {
  ENFORCE_ARG_COUNT(__itern__, 1);
  z_obj_bytes *bytes = AS_BYTES(METHOD_OBJECT);

  if (IS_NIL(args[0])) {
    if (bytes->bytes.count == 0) RETURN_FALSE;
    RETURN_NUMBER(0);
  }

  if (!IS_NUMBER(args[0])) {
    RETURN_ARGUMENT_ERROR("bytes are numerically indexed");
  }

  int index = AS_NUMBER(args[0]);
  if (index < bytes->bytes.count - 1) {
    RETURN_NUMBER((double) index + 1);
  }

  RETURN_NIL;
}
