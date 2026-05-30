#include "file.h"
#ifdef HAVE_UNISTD_H
#include <unistd.h>
#else
#include "zunistd.h"
#endif /* ifdef HAVE_UNISTD_H */
#include "pathinfo.h"

#include <errno.h>
#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>
#include <sys/stat.h>

#ifdef _MSC_VER
#include <sys/utime.h>
#else
#include <utime.h>
#endif /* ifdef _MSC_VER */

#ifdef HAVE_SYS_TIME_H
#include <sys/time.h>
#endif /* ifdef HAVE_SYS_TIME_H */
#include <time.h>

#ifdef _WIN32
#include <windows.h>

/* Symbolic links aren't really a 'thing' on Windows, so just use plain-old
 * stat() instead of lstat(). */
#define lstat stat

char *ttyname(int fd) {
    static char buf[MAX_PATH] = {0};

    HANDLE handle = (HANDLE)_get_osfhandle(fd);
    if (handle == INVALID_HANDLE_VALUE) {
        return "";
    }

    DWORD file_type = GetFileType(handle);
    if (file_type != FILE_TYPE_CHAR && !GetConsoleMode(handle, &file_type) && !GetConsoleTitleA(buf, MAX_PATH)) {
        return "";
    }

    return buf;
}

#endif /* ifdef _WIN32 */

#define FILE_ERROR(type, message)                                              \
  file_close(file);                                                            \
  RETURN_ERROR(#type " -> %s", message, file->path->chars);

#define RETURN_STATUS(status)                                                  \
  if ((status) == 0) {                                                         \
    RETURN_TRUE;                                                               \
  } else {                                                                     \
    RETURN_ERROR(strerror(errno));                                             \
  }

#define DENY_STD()                                                             \
  if (file->is_std)                                                 \
    RETURN_ERROR("method not supported for std files");

#define SET_DICT_STRING(d, n, l, v) dict_add_entry(vm, d, GC_L_STRING(n, l), v)

static int file_close(z_obj_file *file) {
  if (file->file != NULL && !file->is_std) {
    fflush(file->file);
    int result = fclose(file->file);
    file->file = NULL;
    file->is_open = false;
    file->number = -1;
    file->is_tty = false;
    return result;
  }
  return -1;
}

static void file_open(z_obj_file *file) {
  if (file->file == NULL && !file->is_std) {
    file->file = fopen(file->path->chars, file->mode->chars);
    file->is_open = true;
    if(file->file != NULL) {
      file->number = fileno(file->file);
      file->is_tty = isatty(file->number);
    } else {
      file->number = -1;
      file->is_tty = false;
    }
  }
}

DECLARE_NATIVE(file) {
  ENFORCE_ARG_RANGE(file, 1, 2);
  ENFORCE_ARG_TYPE(file, 0, IS_STRING);
  z_obj_string *path = AS_STRING(args[0]);

  if (path->length == 0) {
    RETURN_ERROR("file path cannot be empty");
  }

  z_obj_string *mode = NULL;

  if (arg_count == 2) {
    ENFORCE_ARG_TYPE(file, 1, IS_STRING);
    mode = AS_STRING(args[1]);
  } else {
    mode = (z_obj_string *) GC(copy_string(vm, "r", 1));
  }

  z_obj_file *file = (z_obj_file*)GC(new_file(vm, path, mode));
  file_open(file);

  RETURN_OBJ(file);
}

DECLARE_FILE_METHOD(exists) {
  ENFORCE_ARG_COUNT(exists, 0);
  z_obj_file *file = AS_FILE(METHOD_OBJECT);
  DENY_STD();
  RETURN_BOOL(file_exists(file->path->chars));
}

DECLARE_FILE_METHOD(close) {
  ENFORCE_ARG_COUNT(close, 0);
  file_close(AS_FILE(METHOD_OBJECT));
  RETURN;
}

DECLARE_FILE_METHOD(open) {
  ENFORCE_ARG_COUNT(open, 0);
  file_open(AS_FILE(METHOD_OBJECT));
  RETURN;
}

DECLARE_FILE_METHOD(is_open) {
  z_obj_file *file = AS_FILE(METHOD_OBJECT);
  RETURN_BOOL(file->is_std || file->is_open);
}

DECLARE_FILE_METHOD(is_closed) {
  z_obj_file *file = AS_FILE(METHOD_OBJECT);
  RETURN_BOOL(!file->is_std && !file->is_open);
}

DECLARE_FILE_METHOD(read) {
  ENFORCE_ARG_RANGE(read, 0, 1);
  size_t file_size = -1;
  size_t file_size_real = -1;
  if (arg_count == 1) {
    ENFORCE_ARG_TYPE(read, 0, IS_NUMBER);
    file_size = (size_t) AS_NUMBER(args[0]);
  }

  z_obj_file *file = AS_FILE(METHOD_OBJECT);

  bool in_binary_mode = strstr(file->mode->chars, "b") != NULL;

  if (!file->is_std) {
    // file does not exist
    if (!file_exists(file->path->chars)) {
      RETURN_ERROR(strerror(ENOENT));
    }
    // file is in write only mode
    else if (strstr(file->mode->chars, "w") != NULL &&
             strstr(file->mode->chars, "+") == NULL) {
      RETURN_ERROR("cannot read file in write mode: %s", strerror(ENOTSUP));
    }

    if (!file->is_open) { // open the file if it isn't open
      file_open(file);
    } else if (file->file == NULL) {
      RETURN_ERROR("could not read file: %s", strerror(EIO));
    }

    // Get file size
    struct stat stats; // stats is super faster on large files
    if (lstat(file->path->chars, &stats) == 0) {
      file_size_real = (size_t) stats.st_size;
    } else {
      // fallback
      fseek(file->file, 0L, SEEK_END);
      file_size_real = ftell(file->file);
      rewind(file->file);
    }

    if (file_size == (size_t) -1 || file_size > file_size_real) {
      file_size = file_size_real;
    }
  } else {
    // stdout should not read
    if (fileno(stdout) == file->number || fileno(stderr) == file->number) {
      RETURN_ERROR("cannot read from output file: %s", strerror(ENOTSUP));
    }

    // for non-file objects such as stdin
    // minimum read bytes should be 1
    if (file_size == (size_t) -1) {
      file_size = 1;
    }
  }

  if(!file->file) {
    RETURN_ERROR(strerror(errno));
  }

  char *buffer =
      (char *) ALLOCATE(char, file_size + 1); // +1 for terminator '\0'

  if (buffer == NULL && file_size != 0) {
    RETURN_ERROR(strerror(ENOMEM));
  }

  size_t bytes_read = fread(buffer, sizeof(char), file_size, file->file);

  if (bytes_read == 0 && file_size != 0 && file_size == file_size_real && !file->is_std) {
    RETURN_ERROR("could not read file contents: %s", strerror(EIO));
  }

  if(file->is_std && bytes_read == 0) {
    RETURN_VALUE(EMPTY_STRING_VAL);
  }

  // we made use of +1, so we can terminate the string.
  if (buffer != NULL)
    buffer[bytes_read] = '\0';

  // close file
  if(!file->is_std) {
    file_close(file);
  }

  if (!in_binary_mode) {
    RETURN_T_STRING(buffer, bytes_read);
  }

  RETURN_OBJ(take_bytes(vm, (unsigned char *) buffer, bytes_read));
}

DECLARE_FILE_METHOD(gets) {
  ENFORCE_ARG_RANGE(gets, 0, 1);
  size_t length = -1;
  if (arg_count == 1) {
    ENFORCE_ARG_TYPE(read, 0, IS_NUMBER);
    length = (size_t) AS_NUMBER(args[0]);
  }

  z_obj_file *file = AS_FILE(METHOD_OBJECT);

  bool in_binary_mode = strstr(file->mode->chars, "b") != NULL;

  if (!file->is_std) {
    // file does not exist
    if (!file_exists(file->path->chars)) {
      RETURN_ERROR(strerror(ENOENT));
    }
    // file is in write only mode
    else if (strstr(file->mode->chars, "w") != NULL &&
             strstr(file->mode->chars, "+") == NULL) {
      RETURN_ERROR("cannot read file in write mode: %s", strerror(ENOTSUP));
    }

    if (file->file == NULL) {
      RETURN_ERROR("could not read file: %s", strerror(EIO));
    } else if (!file->is_open) { // open the file if it isn't open
      RETURN_ERROR("file not open: %s", strerror(ENOTSUP));
    }

    if(length == -1) {
      long current_pos = ftell(file->file);
      fseek(file->file, 0L, SEEK_END);
      long end = ftell(file->file);

      // go back to where we were before.
      fseek(file->file, current_pos, SEEK_SET);

      length = end - current_pos;
    }
  } else {
    // stdout should not read
    if (fileno(stdout) == file->number || fileno(stderr) == file->number) {
      RETURN_ERROR("cannot read from output file: %s", strerror(ENOTSUP));
    }

    // for non-file objects such as stdin
    // minimum read bytes should be 1
    if (length == (size_t) -1) {
      length = 1;
    }
  }

  char *buffer =
      (char *) ALLOCATE(char, length + 1); // +1 for terminator '\0'

  if (buffer == NULL && length != 0) {
    RETURN_ERROR(strerror(ENOMEM));
  }

  size_t bytes_read = fread(buffer, sizeof(char), length, file->file);

  if (bytes_read == 0 && length != 0 && !file->is_std) {
    RETURN_ERROR("could not read file contents: %s", strerror(EIO));
  }

  if(file->is_std && bytes_read == 0) {
    RETURN_VALUE(EMPTY_STRING_VAL);
  }

  // we made use of +1, so we can terminate the string.
  if (buffer != NULL)
    buffer[bytes_read] = '\0';

  if (!in_binary_mode) {
    RETURN_T_STRING(buffer, bytes_read);
  }

  RETURN_OBJ(take_bytes(vm, (unsigned char *) buffer, bytes_read));
}

DECLARE_FILE_METHOD(write) {
  ENFORCE_ARG_COUNT(write, 1);

  z_obj_file *file = AS_FILE(METHOD_OBJECT);
  bool in_binary_mode = strstr(file->mode->chars, "b") != NULL;
  unsigned char *data;
  int length;

  if (!in_binary_mode || IS_STRING(args[0])) {
    ENFORCE_ARG_TYPE(write, 0, IS_STRING);
    z_obj_string *string = AS_STRING(args[0]);
    data = (unsigned char *)string->chars;
    length = string->length;
  } else {
    ENFORCE_ARG_TYPE(write, 0, IS_BYTES);
    z_obj_bytes *bytes = AS_BYTES(args[0]);
    data = bytes->bytes.bytes;
    length = bytes->bytes.count;
  }

  // file is in read only mode
  if (!file->is_std) {
    if (strstr(file->mode->chars, "r") != NULL &&
        strstr(file->mode->chars, "+") == NULL) {
      RETURN_ERROR("cannot write into non-writable file: %s", strerror(ENOTSUP));
    } else if (length == 0) {
      RETURN_ERROR("cannot write empty buffer to file: %s", strerror(EIO));
    } else if (file->file == NULL || !file->is_open) { // open the file if it isn't open
      file_open(file);
    } else if (file->file == NULL) {
      RETURN_ERROR("could not write to file: %s", strerror(EIO));
    }
  } else {
    // stdin should not write
    if (fileno(stdin) == file->number) {
      RETURN_ERROR("cannot write to input file: %s", strerror(ENOTSUP));
    }
  }

  if(!file->file) {
    RETURN_ERROR(strerror(errno));
  }

  size_t count = fwrite(data, sizeof(unsigned char), length, file->file);
  // close file
  file_close(file);
  if (count > (size_t) 0) {
    RETURN_TRUE;
  }

  RETURN_FALSE;
}

DECLARE_FILE_METHOD(puts) {
  ENFORCE_ARG_COUNT(puts, 1);

  z_obj_file *file = AS_FILE(METHOD_OBJECT);
  bool in_binary_mode = strstr(file->mode->chars, "b") != NULL;
  unsigned char *data;
  int length;

  if (!in_binary_mode || IS_STRING(args[0])) {
    ENFORCE_ARG_TYPE(write, 0, IS_STRING);
    z_obj_string *string = AS_STRING(args[0]);
    data = (unsigned char *)string->chars;
    length = string->length;
  } else {
    ENFORCE_ARG_TYPE(write, 0, IS_BYTES);
    z_obj_bytes *bytes = AS_BYTES(args[0]);
    data = bytes->bytes.bytes;
    length = bytes->bytes.count;
  }

  // file is in read only mode
  if (!file->is_std) {
    if (strstr(file->mode->chars, "r") != NULL &&
        strstr(file->mode->chars, "+") == NULL) {
      RETURN_ERROR("cannot write into non-writable file: %s", strerror(ENOTSUP));
    } else if (length == 0) {
      RETURN_ERROR("cannot write empty buffer to file: %s", strerror(EIO));
    } else if (!file->is_open) { // open the file if it isn't open
      RETURN_ERROR("file not open: %s", strerror(EACCES));
    } else if (file->file == NULL) {
      RETURN_ERROR("could not write to file: %s", strerror(EIO));
    }
  } else {
    // stdin should not write
    if (fileno(stdin) == file->number) {
      RETURN_ERROR("cannot write to input file: %s", strerror(ENOTSUP));
    }
  }

  size_t count = fwrite(data, sizeof(unsigned char), length, file->file);
  if (count > (size_t) 0 || length == 0) {
    RETURN_TRUE;
  }

  RETURN_FALSE;
}

DECLARE_FILE_METHOD(number) {
  ENFORCE_ARG_COUNT(number, 0);
  RETURN_NUMBER(AS_FILE(METHOD_OBJECT)->number);
}

DECLARE_FILE_METHOD(is_tty) {
  ENFORCE_ARG_COUNT(is_tty, 0);
  z_obj_file *file = AS_FILE(METHOD_OBJECT);
  RETURN_BOOL(file->is_tty);
}

DECLARE_FILE_METHOD(flush) {
  ENFORCE_ARG_COUNT(flush, 0);
  z_obj_file *file = AS_FILE(METHOD_OBJECT);

  if (!file->is_open) {
    RETURN_ERROR("I/O operation on closed file: %s", strerror(ENOTSUP));
  }

  if(file->file != NULL) {
#if defined(IS_UNIX)
    // using fflush on stdin has undesired effect on unix environments
    if (fileno(stdin) == file->number) {
      while ((getchar()) != '\n')
        ;
    } else {
      fflush(file->file);
    }
#else
    fflush(file->file);
#endif
  }
  RETURN;
}

DECLARE_FILE_METHOD(stats) {
  ENFORCE_ARG_COUNT(stats, 0);

  z_obj_file *file = AS_FILE(METHOD_OBJECT);
  z_obj_dict *dict = (z_obj_dict *) GC(new_dict(vm));

  if (!file->is_std) {
    if (file_exists(file->path->chars)) {
      struct stat stats;
      if (lstat(file->path->chars, &stats) == 0) {

#ifndef _WIN32
        // read mode
        SET_DICT_STRING(dict, "is_readable", 11, BOOL_VAL(((stats.st_mode & S_IRUSR) != 0)));
        // write mode
        SET_DICT_STRING(dict, "is_writable", 11, BOOL_VAL(((stats.st_mode & S_IWUSR) != 0)));
        // execute mode
        SET_DICT_STRING(dict, "is_executable", 13, BOOL_VAL(((stats.st_mode & S_IXUSR) != 0)));
        // is symbolic link
        SET_DICT_STRING(dict, "is_symbolic", 11, BOOL_VAL((S_ISLNK(stats.st_mode) != 0)));
#else
        // read mode
        SET_DICT_STRING(dict, "is_readable", 11, BOOL_VAL(((stats.st_mode & S_IREAD) != 0)));
        // write mode
        SET_DICT_STRING(dict, "is_writable", 11, BOOL_VAL(((stats.st_mode & S_IWRITE) != 0)));
        // execute mode
        SET_DICT_STRING(dict, "is_executable", 13, BOOL_VAL(((stats.st_mode & S_IEXEC) != 0)));
        // is symbolic link
        SET_DICT_STRING(dict, "is_symbolic", 11, BOOL_VAL(false));
#endif /* ifndef _WIN32 */

        // file details
        SET_DICT_STRING(dict, "size", 4, NUMBER_VAL(stats.st_size));
        SET_DICT_STRING(dict, "mode", 4, NUMBER_VAL(stats.st_mode));
        SET_DICT_STRING(dict, "dev", 3, NUMBER_VAL(stats.st_dev));
        SET_DICT_STRING(dict, "ino", 3, NUMBER_VAL(stats.st_ino));
        SET_DICT_STRING(dict, "nlink", 5, NUMBER_VAL(stats.st_nlink));
        SET_DICT_STRING(dict, "uid", 3, NUMBER_VAL(stats.st_uid));
        SET_DICT_STRING(dict, "gid", 3, NUMBER_VAL(stats.st_gid));

#if !defined(_WIN32) && (!defined(_POSIX_C_SOURCE) || defined(_DARWIN_C_SOURCE)) && !defined(__MUSL__)
        // last modified time in milliseconds
        SET_DICT_STRING(dict, "mtime", 5, NUMBER_VAL(stats.st_mtimespec.tv_sec));
        // last accessed time in milliseconds
        SET_DICT_STRING(dict, "atime", 5, NUMBER_VAL(stats.st_atimespec.tv_sec));
        // last c time in milliseconds
        SET_DICT_STRING(dict, "ctime", 5, NUMBER_VAL(stats.st_ctimespec.tv_sec));
        // blocks
        SET_DICT_STRING(dict, "blocks", 6, NUMBER_VAL(stats.st_blocks));
        SET_DICT_STRING(dict, "blksize", 7, NUMBER_VAL(stats.st_blksize));
#else
        // last modified time in milliseconds
        SET_DICT_STRING(dict, "mtime", 5, NUMBER_VAL(stats.st_mtime));
        // last accessed time in milliseconds
        SET_DICT_STRING(dict, "atime", 5, NUMBER_VAL(stats.st_atime));
        // last c time in milliseconds
        SET_DICT_STRING(dict, "ctime", 5, NUMBER_VAL(stats.st_ctime));
        // blocks
        SET_DICT_STRING(dict, "blocks", 6, NUMBER_VAL(0));
        SET_DICT_STRING(dict, "blksize", 7, NUMBER_VAL(0));
#endif
      }
    } else {
      RETURN_ERROR(strerror(ENOENT));
    }
  } else {
    // we are dealing with an std
    if (fileno(stdin) == file->number) {
      SET_DICT_STRING(dict, "is_readable", 11, TRUE_VAL);
      SET_DICT_STRING(dict, "is_writable", 11, FALSE_VAL);
    } else {
      SET_DICT_STRING(dict, "is_readable", 11, FALSE_VAL);
      SET_DICT_STRING(dict, "is_writable", 11, TRUE_VAL);
    }
    SET_DICT_STRING(dict, "is_executable", 13, FALSE_VAL);
    SET_DICT_STRING(dict, "size", 4, NUMBER_VAL(1));
  }
  RETURN_OBJ(dict);
}

DECLARE_FILE_METHOD(symlink) {
  ENFORCE_ARG_COUNT(symlink, 1);
  ENFORCE_ARG_TYPE(symlink, 0, IS_STRING);
  z_obj_file *file = AS_FILE(METHOD_OBJECT);
  DENY_STD();

#ifdef _WIN32
  RETURN_ERROR("symlink not supported in windows");
#else
  if (file_exists(file->path->chars)) {
    z_obj_string *path = AS_STRING(args[0]);
    RETURN_BOOL(symlink(file->path->chars, path->chars) == 0);
  } else {
    RETURN_ERROR(strerror(ENOENT));
  }
#endif /* ifdef _WIN32 */
}

DECLARE_FILE_METHOD(delete) {
  ENFORCE_ARG_COUNT(delete, 0);
  z_obj_file *file = AS_FILE(METHOD_OBJECT);
  DENY_STD();

  if(file_close(file) != 0) {
    RETURN_ERROR(strerror(EBADF));
  }
  RETURN_STATUS(unlink(file->path->chars));
}

DECLARE_FILE_METHOD(rename) {
  ENFORCE_ARG_COUNT(rename, 1);
  ENFORCE_ARG_TYPE(rename, 0, IS_STRING);

  z_obj_file *file = AS_FILE(METHOD_OBJECT);
  DENY_STD();

  if (file_exists(file->path->chars)) {
    z_obj_string *new_name = AS_STRING(args[0]);
    if (new_name->length == 0) {
      RETURN_ERROR("file name cannot be empty: %s", strerror(ENOTSUP));
    }
    file_close(file);
    RETURN_STATUS(rename(file->path->chars, new_name->chars));
  } else {
    RETURN_ERROR(strerror(ENOENT));
  }
}

DECLARE_FILE_METHOD(path) {
  ENFORCE_ARG_COUNT(path, 0);
  z_obj_file *file = AS_FILE(METHOD_OBJECT);
  DENY_STD();
  RETURN_OBJ(file->path);
}

DECLARE_FILE_METHOD(mode) {
  ENFORCE_ARG_COUNT(mode, 0);
  z_obj_file *file = AS_FILE(METHOD_OBJECT);
  RETURN_OBJ(file->mode);
}

DECLARE_FILE_METHOD(name) {
  ENFORCE_ARG_COUNT(name, 0);
  z_obj_file *file = AS_FILE(METHOD_OBJECT);
  if(!file->is_std) {
    char *name = get_real_file_name(file->path->chars);
    RETURN_STRING(name);
  } else if(file->is_tty) {
    char *name = ttyname(file->number);
    if(name) {
      RETURN_STRING(name);
    }
  }
  RETURN_NIL;
}

DECLARE_FILE_METHOD(abs_path) {
  ENFORCE_ARG_COUNT(abs_path, 0);
  z_obj_file *file = AS_FILE(METHOD_OBJECT);
  DENY_STD();

  char *abs_path = realpath(file->path->chars, NULL);
  if (abs_path != NULL)
    RETURN_STRING(abs_path);
  RETURN_ERROR("%s", strerror(errno));
}

DECLARE_FILE_METHOD(copy) {
  ENFORCE_ARG_COUNT(copy, 1);
  ENFORCE_ARG_TYPE(copy, 0, IS_STRING);
  z_obj_file *file = AS_FILE(METHOD_OBJECT);
  DENY_STD();

  file_open(file);
  if(file->file == NULL) {
    RETURN_ERROR(strerror(errno));
  }

  if (file_exists(file->path->chars)) {
    z_obj_string *name = AS_STRING(args[0]);

    if (strstr(file->mode->chars, "r") == NULL) {
      file_close(file);
      RETURN_ERROR("file not open for reading: %s", strerror(ENOTSUP));
    }

    char *mode = "w";
    // if we are dealing with a binary file
    if (strstr(file->mode->chars, "b") != NULL) {
      mode = "wb";
    }

    FILE *fp = fopen(name->chars, mode);
    if (fp == NULL) {
      file_close(file);
      RETURN_ERROR("unable to create new file: %s", strerror(EPERM));
    }

    size_t n_read, n_write;
    unsigned char buffer[8192];
    do {
      n_read = fread(buffer, 1, sizeof(buffer), file->file);
      if (n_read > 0) {
        n_write = fwrite(buffer, 1, n_read, fp);
      } else {
        n_write = 0;
      }
    } while ((n_read > 0) && (n_read == n_write));

    if (n_write > 0) {
      file_close(file);
      RETURN_ERROR("error copying file: %s", strerror(EIO));
    }

    fflush(fp);
    fclose(fp);
    file_close(file);

    RETURN_BOOL(n_read == n_write);
  } else {
    file_close(file);
    RETURN_ERROR(strerror(ENOENT));
  }
}

#ifdef _WIN32
#define truncate Truncate
#endif /* ifdef _WIN32 */

DECLARE_FILE_METHOD(truncate) {
  ENFORCE_ARG_RANGE(truncate, 0, 1);

  off_t final_size = 0;
  if (arg_count == 1) {
    ENFORCE_ARG_TYPE(truncate, 0, IS_NUMBER);
    final_size = (off_t) AS_NUMBER(args[0]);
  }
  z_obj_file *file = AS_FILE(METHOD_OBJECT);
  DENY_STD();

#ifndef _WIN32
  RETURN_STATUS(truncate(file->path->chars, final_size));
#else
  RETURN_STATUS(_chsize_s(fileno(file->file), final_size));
#endif /* ifndef _WIN32 */
}

DECLARE_FILE_METHOD(chmod) {
  ENFORCE_ARG_COUNT(chmod, 1);
  ENFORCE_ARG_TYPE(chmod, 0, IS_NUMBER);

  z_obj_file *file = AS_FILE(METHOD_OBJECT);
  DENY_STD();

  if (file_exists(file->path->chars)) {
    int mode = AS_NUMBER(args[0]);

#ifndef _WIN32
    RETURN_STATUS(chmod(file->path->chars, (mode_t) mode));
#else
    RETURN_STATUS(_chmod(file->path->chars, mode));
#endif // !_WIN32
  } else {
    RETURN_ERROR(strerror(ENOENT));
  }
}

DECLARE_FILE_METHOD(set_times) {
  ENFORCE_ARG_COUNT(set_times, 2);
  ENFORCE_ARG_TYPE(set_times, 0, IS_NUMBER);
  ENFORCE_ARG_TYPE(set_times, 1, IS_NUMBER);

#ifdef HAVE_UTIME
  z_obj_file *file = AS_FILE(METHOD_OBJECT);
  DENY_STD();

  if (file_exists(file->path->chars)) {

    time_t atime = (time_t) AS_NUMBER(args[0]);
    time_t mtime = (time_t) AS_NUMBER(args[1]);

    struct stat stats;
    int status = lstat(file->path->chars, &stats);
    if (status == 0) {
      struct utimbuf new_times;

#if !defined(_WIN32) && (!defined(_POSIX_C_SOURCE) || defined(_DARWIN_C_SOURCE)) && !defined(__MUSL__)
      if (atime == (time_t) -1)
        new_times.actime = stats.st_atimespec.tv_sec;
      else
        new_times.actime = atime;

      if (mtime == (time_t) -1)
        new_times.modtime = stats.st_mtimespec.tv_sec;
      else
        new_times.modtime = mtime;
#else
      if (atime == (time_t) -1)
        new_times.actime = stats.st_atime;
      else
        new_times.actime = atime;

      if (mtime == (time_t) -1)
        new_times.modtime = stats.st_mtime;
      else
        new_times.modtime = mtime;
#endif

      RETURN_STATUS(utime(file->path->chars, &new_times));
    } else {
      RETURN_STATUS(status);
    }
  } else {
    RETURN_ERROR(strerror(ENOENT));
  }
#else
  RETURN_ERROR("not available: OS does not support utime");
#endif /* ifdef HAVE_UTIME */
}

DECLARE_FILE_METHOD(seek) {
  ENFORCE_ARG_COUNT(seek, 2);
  ENFORCE_ARG_TYPE(seek, 0, IS_NUMBER);
  ENFORCE_ARG_TYPE(seek, 1, IS_NUMBER);

  z_obj_file *file = AS_FILE(METHOD_OBJECT);
  DENY_STD();

  long position = (long) AS_NUMBER(args[0]);
  int seek_type = AS_NUMBER(args[1]);
  RETURN_STATUS(
    file->file != NULL ? fseek(file->file, position, seek_type) : ENOENT
  );
}

DECLARE_FILE_METHOD(tell) {
  ENFORCE_ARG_COUNT(tell, 0);
  z_obj_file *file = AS_FILE(METHOD_OBJECT);
  DENY_STD();
  RETURN_NUMBER(file->file != NULL ? ftell(file->file) : -1);
}

#undef FILE_ERROR
#undef RETURN_STATUS
#undef SET_DICT_STRING
#undef DENY_STD
