#include "module.h"
#include <zlib.h>
#include <assert.h>
#include <errno.h>

#if defined(_WIN32) && defined(_DEBUG)
#define ZLIB_WINAPI
#endif

#if defined(MSDOS) || defined(OS2) || defined(WIN32) || defined(__CYGWIN__)
#  include <fcntl.h>
#  include <io.h>
#  define SET_BINARY_MODE(file) setmode(fileno(file), O_BINARY)
#else
#  define SET_BINARY_MODE(file)
#endif

#define DEFINE_ZLIB_CONSTANT(v) \
  z_value __zlib_##v(z_vm *vm) { \
    return NUMBER_VAL(v); \
  }

#define GET_ZLIB_CONSTANT(v) \
  {#v, true, __zlib_##v}

#define CHECK_ERR(caller, err, msg) { \
  if (err != Z_OK) { \
    RETURN_ERROR(#caller "()::error(%d): " #err, msg); \
  }

#if UINT_MAX < 4294967295
# define ZLIB_CHUNK_SIZE 32768 
#else
# define ZLIB_CHUNK_SIZE 262144
#endif

#define ZLIB_CHECK_RET_ERROR(fn, end) \
  switch (ret) { \
    case Z_OK: { \
      break; \
    } \
    case Z_MEM_ERROR: { \
      RETURN_ERROR(#fn "(): Out of memory while " #fn "ing data"); \
    } \
    case Z_DATA_ERROR: { \
      RETURN_VALUE_ERROR(#fn "(): invalid or incomplete " #fn " data"); \
    } \
    case Z_STREAM_ERROR: { \
      RETURN_VALUE_ERROR(#fn "(): Bad compression level"); \
    } \
    case Z_VERSION_ERROR: { \
      RETURN_ERROR(#fn "(): zlib version mismatch!"); \
    } \
    default: { \
      (void)end(&strm); \
      RETURN_ERROR(#fn "(): error while " #fn "ing data: %s (%d)", strm.msg, ret); \
    } \
  }

/* compression levels */
DEFINE_ZLIB_CONSTANT(Z_NO_COMPRESSION)
DEFINE_ZLIB_CONSTANT(Z_BEST_SPEED)
DEFINE_ZLIB_CONSTANT(Z_BEST_COMPRESSION)
DEFINE_ZLIB_CONSTANT(Z_DEFAULT_COMPRESSION)

/* compression strategy; see deflateInit2() below for details */
DEFINE_ZLIB_CONSTANT(Z_FILTERED)
DEFINE_ZLIB_CONSTANT(Z_HUFFMAN_ONLY)
DEFINE_ZLIB_CONSTANT(Z_RLE)
DEFINE_ZLIB_CONSTANT(Z_FIXED)
DEFINE_ZLIB_CONSTANT(Z_DEFAULT_STRATEGY)

// others
DEFINE_ZLIB_CONSTANT(MAX_WBITS)
z_value __zlib_Z_VERSION(z_vm *vm) {
  const char* version = zlibVersion();
  return STRING_VAL(version);
}



DECLARE_MODULE_METHOD(zlib_adler32) {
  ENFORCE_ARG_COUNT(adler32, 2);
  ENFORCE_ARG_TYPE(adler32, 0, IS_BYTES);
  ENFORCE_ARG_TYPE(adler32, 1, IS_NUMBER);

  uLong adler = (uLong) AS_NUMBER(args[1]); // default = 0

  z_obj_bytes *bytes = AS_BYTES(args[0]);
  RETURN_NUMBER(adler32(adler, (const Bytef *) bytes->bytes.bytes, (uInt) bytes->bytes.count));
}

DECLARE_MODULE_METHOD(zlib_crc32) {
  ENFORCE_ARG_COUNT(adler32, 2);
  ENFORCE_ARG_TYPE(adler32, 0, IS_BYTES);
  ENFORCE_ARG_TYPE(adler32, 1, IS_NUMBER);

  uLong crc = (uLong) AS_NUMBER(args[1]);

  z_obj_bytes *bytes = AS_BYTES(args[0]);
  RETURN_NUMBER(crc32(crc, (const Bytef *) bytes->bytes.bytes, (uInt) bytes->bytes.count));
}

static void get_buffer_remaining(z_stream *zst, uInt *remains) {
  uInt left = *remains;
  if(left > UINT_MAX) {
    left = UINT_MAX;
  }

  zst->avail_in = left;
  *remains -= zst->avail_in;
}

// deflate(data, level = -1)
DECLARE_MODULE_METHOD(zlib_deflate) {
  ENFORCE_ARG_COUNT(deflate, 5);
  ENFORCE_ARG_TYPE(deflate, 0, IS_BYTES);
  ENFORCE_ARG_TYPE(deflate, 1, IS_NUMBER);
  ENFORCE_ARG_TYPE(deflate, 2, IS_NUMBER);
  ENFORCE_ARG_TYPE(deflate, 3, IS_NUMBER);
  ENFORCE_ARG_TYPE(deflate, 4, IS_NUMBER);

  z_obj_bytes *b = AS_BYTES(args[0]);
  Byte * data = (Byte *)b->bytes.bytes;
  uInt data_length = b->bytes.count;

  int level = AS_NUMBER(args[1]);
  int strategy = AS_NUMBER(args[2]);
  int wbits = AS_NUMBER(args[3]);
  int memory_level = AS_NUMBER(args[4]);
  unsigned char out[ZLIB_CHUNK_SIZE];

  int ret, flush;
  unsigned have;
  z_stream strm;

  /* allocate deflate state */
  strm.zalloc = Z_NULL;
  strm.zfree = Z_NULL;
  strm.opaque = Z_NULL;
  ret = deflateInit2(&strm, level, Z_DEFLATED, wbits, memory_level, strategy);

  ZLIB_CHECK_RET_ERROR(deflate, deflateEnd)

  Bytef *output = ALLOCATE(Bytef, 0);
  size_t output_length = 0;

  do {
    get_buffer_remaining(&strm, &data_length);
    flush = data_length == 0UL ? Z_FINISH : Z_NO_FLUSH;
    strm.next_in = data;

    do {
      strm.avail_out = ZLIB_CHUNK_SIZE;
      strm.next_out = out;

      ret = deflate(&strm, flush);
      if (ret == Z_STREAM_ERROR) {
        deflateEnd(&strm);
        RETURN_ERROR("deflate(): Error %d while deflating data", ret);
      }

      have = ZLIB_CHUNK_SIZE - strm.avail_out;

      if(have > 0) {
        output = GROW_ARRAY(Bytef, output, output_length, output_length + have);
        if(output == NULL) {
          RETURN_ERROR("deflate(): out of memory");
        }

        vm->bytes_allocated += have;
        memcpy(output + output_length, out, have);
        output_length += have;
      }

    } while (strm.avail_out == 0);
    assert(strm.avail_in == 0);

  } while (flush != Z_FINISH);
  assert(ret == Z_STREAM_END);

  ret = deflateEnd(&strm);
  if(ret == Z_OK) {
    RETURN_OBJ(take_bytes(vm, output, (int)output_length));
  }

  RETURN_ERROR("deflate(): Error %d while finishing deflation", ret);
}

// inflate(data, level = -1)
DECLARE_MODULE_METHOD(zlib_inflate) {
  ENFORCE_ARG_COUNT(inflate, 2);
  ENFORCE_ARG_TYPE(inflate, 0, IS_BYTES);
  ENFORCE_ARG_TYPE(inflate, 1, IS_NUMBER);

  z_obj_bytes *b = AS_BYTES(args[0]);
  Byte * data = (Byte *)b->bytes.bytes;
  uInt data_length = b->bytes.count;
  int wbits = AS_NUMBER(args[1]);

  unsigned char out[ZLIB_CHUNK_SIZE];

  int ret, flush;
  unsigned have;
  z_stream strm;

  /* allocate deflate state */
  strm.zalloc = Z_NULL;
  strm.zfree = Z_NULL;
  strm.opaque = Z_NULL;
  strm.avail_in = 0;
  strm.next_in = Z_NULL;
  ret = inflateInit2(&strm, wbits);

  ZLIB_CHECK_RET_ERROR(inflate, inflateEnd)

  Bytef *output = ALLOCATE(Bytef, 0);
  size_t output_length = 0;

  do {
    get_buffer_remaining(&strm, &data_length);
    flush = data_length == 0UL ? Z_FINISH : Z_NO_FLUSH;
    strm.next_in = data;

    do {
      strm.avail_out = ZLIB_CHUNK_SIZE;
      strm.next_out = out;

      ret = inflate(&strm, flush);

      switch (ret) {
        case Z_OK:            /* fall through */
        case Z_BUF_ERROR:     /* fall through */
        case Z_STREAM_END:
          break;

        case Z_MEM_ERROR: {
          (void)inflateEnd(&strm);
          RETURN_ERROR("inflate(): out of memory");
        }
        default: {
          (void)inflateEnd(&strm);
          RETURN_ERROR("inflate(): %s (%d)", strm.msg, ret);
        }
      }

      have = ZLIB_CHUNK_SIZE - strm.avail_out;

      if(have > 0) {
        // size_t len = sizeof(Bytef) * have;
        output = GROW_ARRAY(Bytef, output, have, output_length + have);
        if(output == NULL) {
          RETURN_ERROR("inflate(): out of memory");
        }

        memcpy(output + output_length, out, have);
        output_length += have;
      }

    } while (strm.avail_out == 0);

  } while (ret != Z_STREAM_END && ret != Z_BUF_ERROR);

  ret = inflateEnd(&strm);
  if(ret == Z_OK) {
    RETURN_OBJ(take_bytes(vm,output, output_length));
  }

  RETURN_ERROR("inflate(): Error %d while finishing inflation", ret);
}

DECLARE_MODULE_METHOD(zlib_gzopen) {
  ENFORCE_ARG_COUNT(gzopen, 2);
  ENFORCE_ARG_TYPE(gzopen, 0, IS_STRING);
  ENFORCE_ARG_TYPE(gzopen, 1, IS_STRING);

  char *mode = AS_C_STRING(args[1]);

  gzFile *file = ALLOCATE(gzFile, 1);
  file[0] = gzopen(AS_C_STRING(args[0]), mode);

  if(file[0] == NULL) {
    FREE(gzFile, file);
    RETURN_ERROR(strerror(errno));
  }

  RETURN_PTR(file);
}

DECLARE_MODULE_METHOD(zlib_gzread) {
  ENFORCE_ARG_COUNT(gzread, 2);
  ENFORCE_ARG_TYPE(gzread, 0, IS_PTR);
  ENFORCE_ARG_TYPE(gzread, 1, IS_NUMBER);
  int err;

  gzFile *file = (gzFile *) AS_PTR(args[0])->pointer;
  unsigned int length = (unsigned int) AS_NUMBER(args[1]);

  if(file != NULL) {
    unsigned char *buffer = ALLOCATE(unsigned char, length);
    int bytes_read = gzread(file[0], buffer, length);
    if(bytes_read >= 0) {
      RETURN_OBJ(take_bytes(vm, buffer, bytes_read));
    } else {
      RETURN_ERROR("%s", gzerror(file[0], &err));
    }
  } else {
    RETURN_ARGUMENT_ERROR("invalid GZ handle");
  }
}

DECLARE_MODULE_METHOD(zlib_gzwrite) {
  ENFORCE_ARG_COUNT(gzwrite, 2);
  ENFORCE_ARG_TYPE(gzwrite, 0, IS_PTR);
  ENFORCE_ARG_TYPE(gzwrite, 1, IS_BYTES);

  gzFile *file = (gzFile *) AS_PTR(args[0])->pointer;
  z_obj_bytes *bytes = AS_BYTES(args[1]);

  RETURN_NUMBER(gzwrite(file[0], bytes->bytes.bytes, bytes->bytes.count));
}

DECLARE_MODULE_METHOD(zlib_gzeof) {
  ENFORCE_ARG_COUNT(gzeof, 1);
  ENFORCE_ARG_TYPE(gzeof, 0, IS_PTR);

  gzFile *file = (gzFile *) AS_PTR(args[0])->pointer;

  RETURN_BOOL(gzeof(file[0]) == 1);
}

DECLARE_MODULE_METHOD(zlib_gzdirect) {
  ENFORCE_ARG_COUNT(gzdirect, 1);
  ENFORCE_ARG_TYPE(gzdirect, 0, IS_PTR);

  gzFile *file = (gzFile *) AS_PTR(args[0])->pointer;

  RETURN_BOOL(gzdirect(file[0]) == 1);
}

DECLARE_MODULE_METHOD(zlib_gzclose) {
  ENFORCE_ARG_COUNT(gzclose, 1);
  ENFORCE_ARG_TYPE(gzclose, 0, IS_PTR);

  gzFile *file = (gzFile *) AS_PTR(args[0])->pointer;

  RETURN_NUMBER(gzclose(file[0]) == Z_OK);
}

DECLARE_MODULE_METHOD(zlib_gzrewind) {
  ENFORCE_ARG_COUNT(gzrewind, 1);
  ENFORCE_ARG_TYPE(gzrewind, 0, IS_PTR);

  gzFile *file = (gzFile *) AS_PTR(args[0])->pointer;

  RETURN_NUMBER(gzrewind(file[0]));
}

DECLARE_MODULE_METHOD(zlib_gztell) {
  ENFORCE_ARG_COUNT(gztell, 1);
  ENFORCE_ARG_TYPE(gztell, 0, IS_PTR);

  gzFile *file = (gzFile *) AS_PTR(args[0])->pointer;

  RETURN_NUMBER(gztell(file[0]));
}

DECLARE_MODULE_METHOD(zlib_gzoffset) {
  ENFORCE_ARG_COUNT(gzoffset, 1);
  ENFORCE_ARG_TYPE(gzoffset, 0, IS_PTR);

  gzFile *file = (gzFile *) AS_PTR(args[0])->pointer;

  RETURN_NUMBER(gzoffset(file[0]));
}

DECLARE_MODULE_METHOD(zlib_gzclearerr) {
  ENFORCE_ARG_COUNT(gzclearerr, 1);
  ENFORCE_ARG_TYPE(gzclearerr, 0, IS_PTR);
  gzclearerr(((gzFile *) AS_PTR(args[0])->pointer)[0]);
  RETURN;
}

DECLARE_MODULE_METHOD(zlib_gzsetparams) {
  ENFORCE_ARG_COUNT(gzsetparams, 3);
  ENFORCE_ARG_TYPE(gzsetparams, 0, IS_PTR);
  ENFORCE_ARG_TYPE(gzsetparams, 1, IS_NUMBER);
  ENFORCE_ARG_TYPE(gzsetparams, 2, IS_NUMBER);

  gzFile *file = (gzFile *) AS_PTR(args[0])->pointer;
  int ret = gzsetparams(file[0], AS_NUMBER(args[1]), AS_NUMBER(args[1]));

  switch (ret) {
    case Z_OK: RETURN_TRUE;
    case Z_STREAM_ERROR: RETURN_ERROR("stream not open for writing");
    case Z_MEM_ERROR: RETURN_ERROR("memory access failed");
    default: RETURN_ERROR("%s", strerror(errno));
  }
}

DECLARE_MODULE_METHOD(zlib_gzseek) {
  ENFORCE_ARG_COUNT(gzseek, 3);
  ENFORCE_ARG_TYPE(gzseek, 0, IS_PTR);
  ENFORCE_ARG_TYPE(gzseek, 1, IS_NUMBER);
  ENFORCE_ARG_TYPE(gzseek, 2, IS_NUMBER);

  gzFile *file = (gzFile *) AS_PTR(args[0])->pointer;
  z_off_t offset = (z_off_t) AS_NUMBER(args[1]);
  int whence = AS_NUMBER(args[2]);

  RETURN_NUMBER(gzseek(file[0], offset, whence));
}

CREATE_MODULE_LOADER(zlib2) {
  static z_field_reg fields[] = {
      GET_ZLIB_CONSTANT(Z_NO_COMPRESSION),
      GET_ZLIB_CONSTANT(Z_BEST_SPEED),
      GET_ZLIB_CONSTANT(Z_BEST_COMPRESSION),
      GET_ZLIB_CONSTANT(Z_DEFAULT_COMPRESSION),

      GET_ZLIB_CONSTANT(Z_FILTERED),
      GET_ZLIB_CONSTANT(Z_HUFFMAN_ONLY),
      GET_ZLIB_CONSTANT(Z_RLE),
      GET_ZLIB_CONSTANT(Z_FIXED),
      GET_ZLIB_CONSTANT(Z_DEFAULT_STRATEGY),

      GET_ZLIB_CONSTANT(MAX_WBITS),

      /**
       * Zuri extras...
       */
      GET_ZLIB_CONSTANT(Z_VERSION),

      {NULL,       false, NULL},
  };

  static z_func_reg module_functions[] = {
      {"adler32", false, GET_MODULE_METHOD(zlib_adler32)},
      {"crc32", false, GET_MODULE_METHOD(zlib_crc32)},
      {"deflate", false, GET_MODULE_METHOD(zlib_deflate)},
      {"inflate", false, GET_MODULE_METHOD(zlib_inflate)},
      {"gzopen", false, GET_MODULE_METHOD(zlib_gzopen)},
      {"gzread", false, GET_MODULE_METHOD(zlib_gzread)},
      {"gzwrite", false, GET_MODULE_METHOD(zlib_gzwrite)},
      {"gzeof", false, GET_MODULE_METHOD(zlib_gzeof)},
      {"gzdirect", false, GET_MODULE_METHOD(zlib_gzdirect)},
      {"gzclose", false, GET_MODULE_METHOD(zlib_gzclose)},
      {"gzsetparams", false, GET_MODULE_METHOD(zlib_gzsetparams)},
      {"gzseek", false, GET_MODULE_METHOD(zlib_gzseek)},
      {"gzrewind", false, GET_MODULE_METHOD(zlib_gzrewind)},
      {"gztell", false, GET_MODULE_METHOD(zlib_gztell)},
      {"gzoffset", false, GET_MODULE_METHOD(zlib_gzoffset)},
      {"gzclearerr", false, GET_MODULE_METHOD(zlib_gzclearerr)},
      {NULL,     false, NULL},
  };

  static z_module_reg module = {
      .name = "_zlib",
      .fields = fields,
      .functions = module_functions,
      .classes = NULL,
      .preloader = NULL,
      .unloader = NULL
  };

  return &module;
}