/**
 * The implementation of this module is based on the PHP implementation of
 * pack/unpack which can be found at ext/standard/pack.c in the PHP source code.
 *
 * The original license has been maintained here for reference.
 * @copyright Ore Richard and Zuri contributors.
 */
/*
   +----------------------------------------------------------------------+
   | Copyright (c) The PHP Group                                          |
   +----------------------------------------------------------------------+
   | This source file is subject to version 3.01 of the PHP license,      |
   | that is bundled with this package in the file LICENSE, and is        |
   | available through the world-wide-web at the following url:           |
   | https://www.php.net/license/3_01.txt                                 |
   | If you did not receive a copy of the PHP license and are unable to   |
   | obtain it through the world-wide-web, please send a note to          |
   | license@php.net so we can mail you a copy immediately.               |
   +----------------------------------------------------------------------+
   | Author: Chris Schneider <cschneid@relog.ch>                          |
   +----------------------------------------------------------------------+
 */

#include "module.h"
#include <stdlib.h>

#ifdef _WIN32
typedef uint64_t unaligned_uint64_t __attribute__((__aligned__(1)));
#else
typedef uint64_t unaligned_uint64_t;
#endif

#define INC_OUTPUTPOS(a,b) \
  if ((a) < 0 || ((INT_MAX - outputpos)/((int)(b))) < (a)) { \
    free(formatcodes);	\
    free(formatargs);	\
    RETURN_ERROR("Type %c: integer overflow in format string", code); \
  } \
  outputpos += (a)*(b);

#define MAX_LENGTH_OF_LONG 20
//#define LONG_FMT "%" PRId64

#define UNPACK_REAL_NAME() ( \
  strspn(real_name, "0123456789") == strlen(real_name) ? \
    NUMBER_VAL(strtod(real_name, NULL)) :            \
    (GC_STRING(real_name)) \
    )

#ifndef MAX
#define MAX(a,b) (a > b ? a : b)
#endif

static inline uint16_t reverse_int16(uint16_t arg) {
  return ((arg & 0xFF) << 8) | ((arg >> 8) & 0xFF);
}

static inline uint32_t reverse_int32(uint32_t arg) {
  uint32_t result;
  result = ((arg & 0xFF) << 24) | ((arg & 0xFF00) << 8) | ((arg >> 8) & 0xFF00) | ((arg >> 24) & 0xFF);

  return result;
}

static inline uint64_t reverse_int64(uint64_t arg) {
  union swap_tag {
    uint64_t i;
    uint32_t ul[2];
  } tmp, result;

  tmp.i = arg;
  result.ul[0] = reverse_int32(tmp.ul[1]);
  result.ul[1] = reverse_int32(tmp.ul[0]);

  return result.i;
}

static long to_long(z_vm *vm, z_value value) {
  if (IS_NUMBER(value)) {
    return (long)AS_NUMBER(value);
  } else if (IS_BOOL(value)) {
    return AS_BOOL(value) ? 1L : 0L;
  } else if (IS_NIL(value)) {
    return -1L;
  }

  z_obj_string *v_str = value_to_string(vm, value);
  const char *v = (const char *) v_str->chars;
  int length = v_str->length;

  int start = 0, end = 1, multiplier = 1;
  if(v[0] == '-') {
    start++;
    end++;
    multiplier = -1;
  }

  if(length > (end + 1) && v[start] == '0') {
    char *t = ALLOCATE(char, length - 2);
    memcpy(t, v + (end + 1), length - 2);

    long res = 0;
    if(v[end] == 'b') {
      res = (long)(multiplier * strtoll(t, NULL, 2));
    } else if(v[end] == 'x') {
      res = multiplier * strtol(t, NULL, 16);
    } else if(v[end] == 'c') {
      res = multiplier * strtol(t, NULL, 8);
    }
    FREE_ARRAY(char, t, length - 2);
    return res;
  }

  return (long)strtod(v, NULL);
}

static double to_double(z_vm *vm, z_value value) {
  if (IS_NUMBER(value)) {
    return AS_NUMBER(value);
  } else if (IS_BOOL(value)) {
    return AS_BOOL(value) ? 1 : 0;
  } else if (IS_NIL(value)) {
    return -1;
  }

  z_obj_string *v_str = value_to_string(vm, value);
  const char *v = (const char *) v_str->chars;
  int length = v_str->length;

  int start = 0, end = 1, multiplier = 1;
  if(v[0] == '-') {
    start++;
    end++;
    multiplier = -1;
  }

  if(length > (end + 1) && v[start] == '0') {
    char *t = ALLOCATE(char, length - 2);
    memcpy(t, v + (end + 1), length - 2);

    if(v[end] == 'b') {
      return (double)(multiplier * strtoll(t, NULL, 2));
    } else if(v[end] == 'x') {
      return (double)(multiplier * strtol(t, NULL, 16));
    } else if(v[end] == 'c') {
      return (double)(multiplier * strtol(t, NULL, 8));
    }
  }

  return strtod(v, NULL);
}

static void do_pack(z_vm *vm, z_value val, size_t size, const int *map, unsigned char *output) {
  size_t i;

  long as_long = to_long(vm, val);
  char *v = (char *) &as_long;

  for (i = 0; i < size; i++) {
    *output++ = v[map[i]];
  }
}

static void copy_float(int is_little_endian, void * dst, float f) {
  union float_tag {
    float f;
    uint32_t i;
  } m;
  m.f = f;

#if IS_BIG_ENDIAN
  if (is_little_endian) {
#else
  if (!is_little_endian) {
#endif
    m.i = reverse_int32(m.i);
  }

  memcpy(dst, &m.f, sizeof(float));
}

static void copy_double(int is_little_endian, void * dst, double d) {
  union double_tag {
    double d;
    uint64_t i;
  } m;
  m.d = d;

#if IS_BIG_ENDIAN
  if (is_little_endian) {
#else
  if (!is_little_endian) {
#endif
    m.i = reverse_int64(m.i);
  }

  memcpy(dst, &m.d, sizeof(double));
}

static inline char *ulong_to_buffer(char *buf, long num) {
  *buf = '\0';
  do {
    *--buf = (char)((char) (num % 10) + '0');
    num /= 10;
  } while (num > 0);
  return buf;
}

static float parse_float(int is_little_endian, void * src) {
    union float_tag {
      float f;
      uint32_t i;
    } m;
    memcpy(&m.i, src, sizeof(float));

#if IS_BIG_ENDIAN
    if (is_little_endian) {
#else
  if (!is_little_endian) {
#endif
      m.i = reverse_int32(m.i);
    }

return m.f;
}

static double parse_double(int is_little_endian, void * src) {
  union double_tag {
    double d;
    uint64_t i;
  } m;
  memcpy(&m.i, src, sizeof(double));

#if IS_BIG_ENDIAN
  if (is_little_endian) {
#else
  if (!is_little_endian) {
#endif
    m.i = reverse_int64(m.i);
  }

return m.d;
}


/* Mapping of byte from char (8bit) to long for machine endian */
static int byte_map[1];

/* Mappings of bytes from int (machine dependent) to int for machine endian */
static int int_map[sizeof(int)];

/* Mappings of bytes from shorts (16bit) for all endian environments */
static int machine_endian_short_map[2];
static int big_endian_short_map[2];
static int little_endian_short_map[2];

/* Mappings of bytes from longs (32bit) for all endian environments */
static int machine_endian_long_map[4];
static int big_endian_long_map[4];
static int little_endian_long_map[4];

#if IS_64_BIT
/* Mappings of bytes from quads (64bit) for all endian environments */
static int machine_endian_longlong_map[8];
static int big_endian_longlong_map[8];
static int little_endian_longlong_map[8];
#endif


DECLARE_MODULE_METHOD(struct_pack) {
  ENFORCE_ARG_COUNT(pack, 2);
  ENFORCE_ARG_TYPE(pack, 0, IS_STRING);
  ENFORCE_ARG_TYPE(pack, 1, IS_LIST);

  z_obj_string *string = AS_STRING(args[0]);
  z_obj_list *params = AS_LIST(args[1]);

  z_value *args_list = params->items.values;
  int param_count = params->items.count;

  size_t i;
  int currentarg;
  char *format = string->chars;
  size_t formatlen = string->length;
  size_t formatcount = 0;
  int outputpos = 0, outputsize = 0;

  /* We have a maximum of <formatlen> format codes to deal with */
  char *formatcodes = ALLOCATE(char, formatlen);
  int *formatargs = ALLOCATE(int, formatlen);
  currentarg = 0;

  for (i = 0; i < formatlen; formatcount++) {
    char code = format[i++];
    int arg = 1;

    /* Handle format arguments if any */
    if (i < formatlen) {
      char c = format[i];

      if (c == '*') {
        arg = -1;
        i++;
      } else if (c >= '0' && c <= '9') {
        arg = (int) strtol(&format[i], NULL, 10);

        while (format[i] >= '0' && format[i] <= '9' && i < formatlen) {
          i++;
        }
      }
    }

    /* Handle special arg '*' for all codes and check argv overflows */
    switch ((int) code) {
      /* Never uses any args_list */
      case 'x':
      case 'X':
      case '@':
        if (arg < 0) {
          WARN("Type %c: '*' ignored", code);
          arg = 1;
        }
        break;

        /* Always uses one arg */
      case 'a':
      case 'A':
      case 'Z':
      case 'h':
      case 'H':
        if (currentarg >= param_count) {
          free(formatcodes);
          free(formatargs);
          RETURN_ERROR("Type %c: not enough arguments", code);
        }

        if (arg < 0) {
          z_obj_string *as_string = value_to_string(vm, args_list[currentarg]);
          arg = as_string->length;
          if (code == 'Z') {
            /* add one because Z is always NUL-terminated:
             * pack("Z*", "aa") === "aa\0"
             * pack("Z2", "aa") === "a\0" */
            arg++;
          }
        }

        currentarg++;
        break;

        /* Use as many args_list as specified */
      case 'q':
      case 'Q':
      case 'J':
      case 'P':
#if !IS_64_BIT
        free(formatcodes);
        free(formatargs);
        RETURN_ERROR("64-bit format codes are not available for 32-bit builds of Zuri");
#endif
      case 'c':
      case 'C':
      case 's':
      case 'S':
      case 'i':
      case 'I':
      case 'l':
      case 'L':
      case 'n':
      case 'N':
      case 'v':
      case 'V':
      case 'f': /* float */
      case 'g': /* little endian float */
      case 'G': /* big endian float */
      case 'd': /* double */
      case 'e': /* little endian double */
      case 'E': /* big endian double */
        if (arg < 0) {
          arg = param_count - currentarg;
        }
        if (currentarg > INT_MAX - arg) {
          goto too_few_args;
        }
        currentarg += arg;

        if (currentarg > param_count) {
too_few_args:
          free(formatcodes);
          free(formatargs);
          RETURN_ERROR("Type %c: too few arguments", code);
        }
        break;

      default:
        free(formatcodes);
        free(formatargs);
        RETURN_ERROR("Type %c: unknown format code", code);
    }

    formatcodes[formatcount] = code;
    formatargs[formatcount] = arg;
  }

  if (currentarg < param_count) {
    WARN("%d arguments unused", (param_count - currentarg));
  }

  /* Calculate output length and upper bound while processing*/
  for (i = 0; i < formatcount; i++) {
    int code = (int) formatcodes[i];
    int arg = formatargs[i];

    switch ((int) code) {
      case 'h':
      case 'H':
        INC_OUTPUTPOS((arg + (arg % 2)) / 2, 1);  /* 4 bit per arg */
        break;

      case 'a':
      case 'A':
      case 'Z':
      case 'c':
      case 'C':
      case 'x':
        INC_OUTPUTPOS(arg, 1);    /* 8 bit per arg */
        break;

      case 's':
      case 'S':
      case 'n':
      case 'v':
        INC_OUTPUTPOS(arg, 2);    /* 16 bit per arg */
        break;

      case 'i':
      case 'I':
        INC_OUTPUTPOS(arg, sizeof(int));
        break;

      case 'l':
      case 'L':
      case 'N':
      case 'V':
        INC_OUTPUTPOS(arg, 4);    /* 32 bit per arg */
        break;

#if IS_64_BIT
        case 'q':
        case 'Q':
        case 'J':
        case 'P':
          INC_OUTPUTPOS(arg,8);		/* 32 bit per arg */
          break;
#endif

      case 'f': /* float */
      case 'g': /* little endian float */
      case 'G': /* big endian float */
        INC_OUTPUTPOS(arg, sizeof(float));
        break;

      case 'd': /* double */
      case 'e': /* little endian double */
      case 'E': /* big endian double */
        INC_OUTPUTPOS(arg, sizeof(double));
        break;

      case 'X':
        outputpos -= arg;

        if (outputpos < 0) {
          WARN("Type %c: outside of string", code);
          outputpos = 0;
        }
        break;

      case '@':
        outputpos = arg;
        break;
    }

    if (outputsize < outputpos) {
      outputsize = outputpos;
    }
  }

  unsigned char *output = ALLOCATE(unsigned char, outputsize + 1);
  outputpos = 0;
  currentarg = 0;

  for (i = 0; i < formatcount; i++) {
    int code = (int) formatcodes[i];
    int arg = formatargs[i];

    switch ((int) code) {
      case 'a':
      case 'A':
      case 'Z': {
        size_t arg_cp = (code != 'Z') ? arg : MAX(0, arg - 1);
        char *str = value_to_string(vm, args_list[currentarg++])->chars;

        memset(&output[outputpos], (code == 'a' || code == 'Z') ? '\0' : ' ', arg);
        memcpy(&output[outputpos], str, (strlen(str) < arg_cp) ? strlen(str) : arg_cp);

        outputpos += arg;
        break;
      }

      case 'h':
      case 'H': {
        int nibbleshift = (code == 'h') ? 0 : 4;
        int first = 1;
        z_obj_string *the_str = value_to_string(vm, args_list[currentarg++]);
        char *str = the_str->chars;

        outputpos--;
        if ((size_t) arg > strlen(str)) {
          WARN("Type %c: not enough characters in string", code);
          arg = the_str->length;
        }

        while (arg-- > 0) {
          char n = *str++;

          if (n >= '0' && n <= '9') {
            n -= '0';
          } else if (n >= 'A' && n <= 'F') {
            n -= ('A' - 10);
          } else if (n >= 'a' && n <= 'f') {
            n -= ('a' - 10);
          } else {
            WARN("Type %c: illegal hex digit %c", code, n);
            n = 0;
          }

          if (first--) {
            output[++outputpos] = 0;
          } else {
            first = 1;
          }

          output[outputpos] |= (n << nibbleshift);
          nibbleshift = (nibbleshift + 4) & 7;
        }

        outputpos++;
        break;
      }

      case 'c':
      case 'C':
        while (arg-- > 0) {
          do_pack(vm, args_list[currentarg++], 1, byte_map, &output[outputpos]);
          outputpos++;
        }
        break;

      case 's':
      case 'S':
      case 'n':
      case 'v': {
        int *map = machine_endian_short_map;

        if (code == 'n') {
          map = big_endian_short_map;
        } else if (code == 'v') {
          map = little_endian_short_map;
        }

        while (arg-- > 0) {
          do_pack(vm, args_list[currentarg++], 2, map, &output[outputpos]);
          outputpos += 2;
        }
        break;
      }

      case 'i':
      case 'I':
        while (arg-- > 0) {
          do_pack(vm, args_list[currentarg++], sizeof(int), int_map, &output[outputpos]);
          outputpos += sizeof(int);
        }
        break;

      case 'l':
      case 'L':
      case 'N':
      case 'V': {
        int *map = machine_endian_long_map;

        if (code == 'N') {
          map = big_endian_long_map;
        } else if (code == 'V') {
          map = little_endian_long_map;
        }

        while (arg-- > 0) {
          do_pack(vm, args_list[currentarg++], 4, map, &output[outputpos]);
          outputpos += 4;
        }
        break;
      }

        case 'q':
        case 'Q':
        case 'J':
        case 'P': {
#if IS_64_BIT
          int *map = machine_endian_longlong_map;

          if (code == 'J') {
            map = big_endian_longlong_map;
          } else if (code == 'P') {
            map = little_endian_longlong_map;
          }

          while (arg-- > 0) {
            do_pack(vm, args_list[currentarg++], 8, map, &output[outputpos]);
            outputpos += 8;
          }
          break;
#else
          RETURN_ERROR("q, Q, J and P are only supported on 64-bit builds of Zuri");
#endif
        }

      case 'f': {
        while (arg-- > 0) {
          float v = (float) to_double(vm, args_list[currentarg++]);
          memcpy(&output[outputpos], &v, sizeof(v));
          outputpos += sizeof(v);
        }
        break;
      }

      case 'g': {
        /* pack little endian float */
        while (arg-- > 0) {
          float v = (float) to_double(vm, args_list[currentarg++]);
          copy_float(1, &output[outputpos], v);
          outputpos += sizeof(v);
        }

        break;
      }
      case 'G': {
        /* pack big endian float */
        while (arg-- > 0) {
          float v = (float) to_double(vm, args_list[currentarg++]);
          copy_float(0, &output[outputpos], v);
          outputpos += sizeof(v);
        }
        break;
      }

      case 'd': {
        while (arg-- > 0) {
          double v = to_double(vm, args_list[currentarg++]);
          memcpy(&output[outputpos], &v, sizeof(v));
          outputpos += sizeof(v);
        }
        break;
      }

      case 'e': {
        /* pack little endian double */
        while (arg-- > 0) {
          double v = to_double(vm, args_list[currentarg++]);
          copy_double(1, &output[outputpos], v);
          outputpos += sizeof(v);
        }
        break;
      }

      case 'E': {
        /* pack big endian double */
        while (arg-- > 0) {
          double v = to_double(vm, args_list[currentarg++]);
          copy_double(0, &output[outputpos], v);
          outputpos += sizeof(v);
        }
        break;
      }

      case 'x':
        memset(&output[outputpos], '\0', arg);
        outputpos += arg;
        break;

      case 'X':
        outputpos -= arg;

        if (outputpos < 0) {
          outputpos = 0;
        }
        break;

      case '@':
        if (arg > outputpos) {
          memset(&output[outputpos], '\0', arg - outputpos);
        }
        outputpos = arg;
        break;
    }
  }

  free(formatcodes);
  free(formatargs);
  output[outputpos] = '\0';

  z_obj_bytes *bytes = (z_obj_bytes *)GC(take_bytes(vm, output, outputpos));
  RETURN_OBJ(bytes);
}

DECLARE_MODULE_METHOD(struct_unpack) {
  ENFORCE_ARG_COUNT(unpack, 3);
  ENFORCE_ARG_TYPE(unpack, 0, IS_STRING);
  ENFORCE_ARG_TYPE(unpack, 1, IS_BYTES);
  ENFORCE_ARG_TYPE(unpack, 2, IS_NUMBER);

  int i;
  z_obj_string *string = AS_STRING(args[0]);
  z_obj_bytes *data = AS_BYTES(args[1]);
  int offset = AS_NUMBER(args[2]);

  char *format = string->chars;
  char *input = (char *)data->bytes.bytes;
  size_t formatlen = string->length,
        inputpos = 0,
        inputlen = data->bytes.count;

  if (offset < 0 || offset > inputlen) {
    RETURN_ERROR("argument 3 (offset) must be within the range of argument 2 (data)");
  }

  input += offset;
  inputlen -= offset;

  z_obj_dict *return_value = (z_obj_dict *)GC(new_dict(vm));

  while (formatlen-- > 0) {
    char type = *(format++);
    char c;
    int repetitions = 1, argb;
    char *name;
    int namelen;
    int size = 0;

    /* Handle format arguments if any */
    if (formatlen > 0) {
      c = *format;

      if (c >= '0' && c <= '9') {
        repetitions = (int)strtol(format, NULL, 10);

        while (formatlen > 0 && *format >= '0' && *format <= '9') {
          format++;
          formatlen--;
        }
      } else if (c == '*') {
        repetitions = -1;
        format++;
        formatlen--;
      }
    }

    /* Get of new value in array */
    name = format;
    argb = repetitions;

    while (formatlen > 0 && *format != '/') {
      formatlen--;
      format++;
    }

    namelen = format - name;

    if (namelen > 200)
      namelen = 200;

    switch ((int) type) {
      /* Never use any input */
      case 'X':
        size = -1;
        if (repetitions < 0) {
          WARN("Type %c: '*' ignored", type);
          repetitions = 1;
        }
        break;

      case '@':
        size = 0;
        break;

      case 'a':
      case 'A':
      case 'Z':
        size = repetitions;
        repetitions = 1;
        break;

      case 'h':
      case 'H':
        size = (repetitions > 0) ? (repetitions + (repetitions % 2)) / 2 : repetitions;
        repetitions = 1;
        break;

        /* Use 1 byte of input */
      case 'c':
      case 'C':
      case 'x':
        size = 1;
        break;

        /* Use 2 bytes of input */
      case 's':
      case 'S':
      case 'n':
      case 'v':
        size = 2;
        break;

        /* Use sizeof(int) bytes of input */
      case 'i':
      case 'I':
        size = sizeof(int);
        break;

        /* Use 4 bytes of input */
      case 'l':
      case 'L':
      case 'N':
      case 'V':
        size = 4;
        break;

        /* Use 8 bytes of input */
      case 'q':
      case 'Q':
      case 'J':
      case 'P':
#if IS_64_BIT
        size = 8;
        break;
#else
        RETURN_ERROR("64-bit format codes are not available for 32-bit Zuri");
#endif

        /* Use sizeof(float) bytes of input */
      case 'f':
      case 'g':
      case 'G':
        size = sizeof(float);
        break;

        /* Use sizeof(double) bytes of input */
      case 'd':
      case 'e':
      case 'E':
        size = sizeof(double);
        break;

      default:
        RETURN_ERROR("Invalid format type %c", type);
    }

    if (size != 0 && size != -1 && size < 0) {
      WARN("Type %c: integer overflow", type);
      RETURN_FALSE;
    }


    /* Do actual unpacking */
    for (i = 0; i != repetitions; i++) {

      if (size != 0 && size != -1 && INT_MAX - size + 1 < inputpos) {
        WARN("Type %c: integer overflow", type);
        RETURN_FALSE;
      }

      if ((inputpos + size) <= inputlen) {

        char *real_name;

        if (repetitions == 1 && namelen > 0) {
          /* Use a part of the formatarg argument directly as the name. */
          real_name = ALLOCATE(char, namelen);
          memcpy(real_name, name, namelen);
          real_name[namelen] = '\0';

        } else {
          /* Need to add the 1-based element number to the name */

          char buf[MAX_LENGTH_OF_LONG + 1];
          char *res = ulong_to_buffer(buf + sizeof(buf) - 1, i + 1);
          size_t digits = buf + sizeof(buf) - 1 - res;

          real_name = ALLOCATE(char, namelen + digits);
          if(real_name == NULL) {
            RETURN_ERROR("out of memory");
          }

          memcpy(real_name, name, namelen);
          memcpy(real_name + namelen, res, digits);
          real_name[namelen + digits] = '\0';
        }

        switch ((int) type) {
          case 'a': {
            /* a will not strip any trailing whitespace or null padding */
            size_t len = inputlen - inputpos;  /* Remaining string */

            /* If size was given take minimum of len and size */
            if ((size >= 0) && (len > size)) {
              len = size;
            }

            size = (int)len;

            dict_set_entry(vm, return_value, UNPACK_REAL_NAME(), GC_L_STRING(&input[inputpos], len));
            break;
          }
          case 'A': {
            /* A will strip any trailing whitespace */
            char padn = '\0';
            char pads = ' ';
            char padt = '\t';
            char padc = '\r';
            char padl = '\n';
            size_t len = inputlen - inputpos;  /* Remaining string */

            /* If size was given take minimum of len and size */
            if ((size >= 0) && (len > size)) {
              len = size;
            }

            size = (int)len;

            /* Remove trailing white space and nulls chars from unpacked data */
            while (--len >= 0) {
              if (input[inputpos + len] != padn
                  && input[inputpos + len] != pads
                  && input[inputpos + len] != padt
                  && input[inputpos + len] != padc
                  && input[inputpos + len] != padl
                  )
                break;
            }

            dict_set_entry(vm, return_value, UNPACK_REAL_NAME(), GC_L_STRING(&input[inputpos], len + 1));
            break;
          }
            /* New option added for Z to remain in-line with the Perl implementation */
          case 'Z': {
            /* Z will strip everything after the first null character */
            char pad = '\0';
            size_t s,
                len = inputlen - inputpos;  /* Remaining string */

            /* If size was given take minimum of len and size */
            if ((size >= 0) && (len > size)) {
              len = size;
            }

            size = (int)len;

            /* Remove everything after the first null */
            for (s = 0; s < len; s++) {
              if (input[inputpos + s] == pad)
                break;
            }
            len = s;

            dict_set_entry(vm, return_value, UNPACK_REAL_NAME(), GC_L_STRING(&input[inputpos], len));
            break;
          }


          case 'h':
          case 'H': {
            size_t len = (inputlen - inputpos) * 2;  /* Remaining */
            int nibbleshift = (type == 'h') ? 0 : 4;
            int first = 1;
            size_t ipos, opos;

            /* If size was given take minimum of len and size */
            if (size >= 0 && len > (size * 2)) {
              len = size * 2;
            }

            if (len > 0 && argb > 0) {
              len -= argb % 2;
            }

            char *buf = ALLOCATE(char, len);

            for (ipos = opos = 0; opos < len; opos++) {
              char cc = (input[inputpos + ipos] >> nibbleshift) & 0xf;

              if (cc < 10) {
                cc += '0';
              } else {
                cc += 'a' - 10;
              }

              buf[opos] = cc;
              nibbleshift = (nibbleshift + 4) & 7;

              if (first-- == 0) {
                ipos++;
                first = 1;
              }
            }

            buf[len] = '\0';

            dict_set_entry(vm, return_value, UNPACK_REAL_NAME(), GC_L_STRING(buf, len));
            break;
          }

          case 'c':   /* signed */
          case 'C': { /* unsigned */
            uint8_t x = input[inputpos];
            long v = (type == 'c') ? (int8_t) x : x;

            dict_set_entry(vm, return_value, UNPACK_REAL_NAME(), NUMBER_VAL(v));
            break;
          }

          case 's':   /* signed machine endian   */
          case 'S':   /* unsigned machine endian */
          case 'n':   /* unsigned big endian     */
          case 'v': { /* unsigned little endian  */
            long v = 0;
            uint16_t x = *((uint16_t * ) & input[inputpos]);

            if (type == 's') {
              v = (int16_t) x;
            } else if ((type == 'n' && IS_LITTLE_ENDIAN) || (type == 'v' && !IS_LITTLE_ENDIAN)) {
              v = reverse_int16(x);
            } else {
              v = x;
            }

            dict_set_entry(vm, return_value, UNPACK_REAL_NAME(), NUMBER_VAL(v));
            break;
          }

          case 'i':   /* signed integer, machine size, machine endian */
          case 'I': { /* unsigned integer, machine size, machine endian */
            long v;
            if (type == 'i') {
              int x = *((int *) & input[inputpos]);
              v = x;
            } else {
              unsigned int x = *((unsigned int *) & input[inputpos]);
              v = x;
            }

            dict_set_entry(vm, return_value, UNPACK_REAL_NAME(), NUMBER_VAL(v));
            break;
          }

          case 'l':   /* signed machine endian   */
          case 'L':   /* unsigned machine endian */
          case 'N':   /* unsigned big endian     */
          case 'V': { /* unsigned little endian  */
            long v = 0;
            uint32_t x = *((uint32_t * ) & input[inputpos]);

            if (type == 'l') {
              v = (int32_t) x;
            } else if ((type == 'N' && IS_LITTLE_ENDIAN) || (type == 'V' && !IS_LITTLE_ENDIAN)) {
              v = reverse_int32(x);
            } else {
              v = x;
            }

            dict_set_entry(vm, return_value, UNPACK_REAL_NAME(), NUMBER_VAL(v));
            break;
          }

            case 'q':   /* signed machine endian   */
            case 'Q':   /* unsigned machine endian */
            case 'J':   /* unsigned big endian     */
            case 'P': { /* unsigned little endian  */
#if IS_64_BIT
              int64_t v = 0;
              uint64_t x = *((unaligned_uint64_t *) &input[inputpos]);

              if (type == 'q') {
                v = (int64_t) x;
              } else if ((type == 'J' && IS_LITTLE_ENDIAN) || (type == 'P' && !IS_LITTLE_ENDIAN)) {
                v = reverse_int64(x);
              } else {
                v = x;
              }

              dict_set_entry(vm, return_value, UNPACK_REAL_NAME(), NUMBER_VAL((double)v));
              break;
#else
              RETURN_ERROR("q, Q, J and P are only valid on 64 bit build of Zuri");
#endif
            }

          case 'f': /* float */
          case 'g': /* little endian float*/
          case 'G': /* big endian float*/
          {
            float v;

            if (type == 'g') {
              v = parse_float(1, &input[inputpos]);
            } else if (type == 'G') {
              v = parse_float(0, &input[inputpos]);
            } else {
              memcpy(&v, &input[inputpos], sizeof(float));
            }

            dict_set_entry(vm, return_value, UNPACK_REAL_NAME(), NUMBER_VAL(v));
            break;
          }


          case 'd': /* double */
          case 'e': /* little endian float */
          case 'E': /* big endian float */
          {
            double v;
            if (type == 'e') {
              v = parse_double(1, &input[inputpos]);
            } else if (type == 'E') {
              v = parse_double(0, &input[inputpos]);
            } else {
              memcpy(&v, &input[inputpos], sizeof(double));
            }

            dict_set_entry(vm, return_value, UNPACK_REAL_NAME(), NUMBER_VAL(v));
            break;
          }

          case 'x':
            /* Do nothing with input, just skip it */
            break;

          case 'X':
            if (inputpos < size) {
              inputpos = -size;
              i = repetitions - 1;    /* Break out of for loop */

              if (repetitions >= 0) {
                WARN("Type %c: outside of string", type);
              }
            }
            break;

          case '@':
            if (repetitions <= inputlen) {
              inputpos = repetitions;
            } else {
               WARN("Type %c: outside of string", type);
            }

            i = repetitions - 1;  /* Done, break out of for loop */
            break;
        }

        inputpos += size;
        if (inputpos < 0) {
          if (size != -1) { /* only print warning if not working with * */
             WARN("Type %c: outside of string", type);
          }
          inputpos = 0;
        }
      } else if (repetitions < 0) {
        /* Reached end of input for '*' repeater */
        break;
      } else {
        WARN("Type %c: not enough input, need %d, have %ld", type, size, inputlen - inputpos);
        RETURN_FALSE;
      }
    }

    if (formatlen > 0) {
      formatlen--;  /* Skip '/' separator, does no harm if inputlen == 0 */
      format++;
    }
  }

  RETURN_OBJ(return_value);
}

void __struct_module_preloader(z_vm *vm) {
  int i;

#if IS_LITTLE_ENDIAN
    /* Where to get lo to hi bytes from */
    byte_map[0] = 0;

    for (i = 0; i < (int)sizeof(int); i++) {
      int_map[i] = i;
    }

    machine_endian_short_map[0] = 0;
    machine_endian_short_map[1] = 1;
    big_endian_short_map[0] = 1;
    big_endian_short_map[1] = 0;
    little_endian_short_map[0] = 0;
    little_endian_short_map[1] = 1;

    machine_endian_long_map[0] = 0;
    machine_endian_long_map[1] = 1;
    machine_endian_long_map[2] = 2;
    machine_endian_long_map[3] = 3;
    big_endian_long_map[0] = 3;
    big_endian_long_map[1] = 2;
    big_endian_long_map[2] = 1;
    big_endian_long_map[3] = 0;
    little_endian_long_map[0] = 0;
    little_endian_long_map[1] = 1;
    little_endian_long_map[2] = 2;
    little_endian_long_map[3] = 3;

#if IS_64_BIT
    machine_endian_longlong_map[0] = 0;
    machine_endian_longlong_map[1] = 1;
    machine_endian_longlong_map[2] = 2;
    machine_endian_longlong_map[3] = 3;
    machine_endian_longlong_map[4] = 4;
    machine_endian_longlong_map[5] = 5;
    machine_endian_longlong_map[6] = 6;
    machine_endian_longlong_map[7] = 7;
    big_endian_longlong_map[0] = 7;
    big_endian_longlong_map[1] = 6;
    big_endian_longlong_map[2] = 5;
    big_endian_longlong_map[3] = 4;
    big_endian_longlong_map[4] = 3;
    big_endian_longlong_map[5] = 2;
    big_endian_longlong_map[6] = 1;
    big_endian_longlong_map[7] = 0;
    little_endian_longlong_map[0] = 0;
    little_endian_longlong_map[1] = 1;
    little_endian_longlong_map[2] = 2;
    little_endian_longlong_map[3] = 3;
    little_endian_longlong_map[4] = 4;
    little_endian_longlong_map[5] = 5;
    little_endian_longlong_map[6] = 6;
    little_endian_longlong_map[7] = 7;
#endif
#else
    int size = sizeof(long);

    /* Where to get hi to lo bytes from */
    byte_map[0] = size - 1;

    for (i = 0; i < (int)sizeof(int); i++) {
      int_map[i] = size - (sizeof(int) - i);
    }

    machine_endian_short_map[0] = size - 2;
    machine_endian_short_map[1] = size - 1;
    big_endian_short_map[0] = size - 2;
    big_endian_short_map[1] = size - 1;
    little_endian_short_map[0] = size - 1;
    little_endian_short_map[1] = size - 2;

    machine_endian_long_map[0] = size - 4;
    machine_endian_long_map[1] = size - 3;
    machine_endian_long_map[2] = size - 2;
    machine_endian_long_map[3] = size - 1;
    big_endian_long_map[0] = size - 4;
    big_endian_long_map[1] = size - 3;
    big_endian_long_map[2] = size - 2;
    big_endian_long_map[3] = size - 1;
    little_endian_long_map[0] = size - 1;
    little_endian_long_map[1] = size - 2;
    little_endian_long_map[2] = size - 3;
    little_endian_long_map[3] = size - 4;

#if ISIS_64_BIT
    machine_endian_longlong_map[0] = size - 8;
    machine_endian_longlong_map[1] = size - 7;
    machine_endian_longlong_map[2] = size - 6;
    machine_endian_longlong_map[3] = size - 5;
    machine_endian_longlong_map[4] = size - 4;
    machine_endian_longlong_map[5] = size - 3;
    machine_endian_longlong_map[6] = size - 2;
    machine_endian_longlong_map[7] = size - 1;
    big_endian_longlong_map[0] = size - 8;
    big_endian_longlong_map[1] = size - 7;
    big_endian_longlong_map[2] = size - 6;
    big_endian_longlong_map[3] = size - 5;
    big_endian_longlong_map[4] = size - 4;
    big_endian_longlong_map[5] = size - 3;
    big_endian_longlong_map[6] = size - 2;
    big_endian_longlong_map[7] = size - 1;
    little_endian_longlong_map[0] = size - 1;
    little_endian_longlong_map[1] = size - 2;
    little_endian_longlong_map[2] = size - 3;
    little_endian_longlong_map[3] = size - 4;
    little_endian_longlong_map[4] = size - 5;
    little_endian_longlong_map[5] = size - 6;
    little_endian_longlong_map[6] = size - 7;
    little_endian_longlong_map[7] = size - 8;
#endif
#endif
}

CREATE_MODULE_LOADER(struct) {
  static z_func_reg module_functions[] = {
      {"pack", true,  GET_MODULE_METHOD(struct_pack)},
      {"unpack", true,  GET_MODULE_METHOD(struct_unpack)},
      {NULL,   false, NULL},
  };

  static z_module_reg module = {
      .name = "_struct",
      .fields = NULL,
      .functions = module_functions,
      .classes = NULL,
      .preloader = &__struct_module_preloader,
      .unloader = NULL
  };

  return &module;
}
