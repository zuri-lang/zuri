#if !defined(ZURI_PATHINFO_H)
#define ZURI_PATHINFO_H

#include "common.h"

#ifdef _WIN32
#define ZURI_PATH_SEPARATOR "\\"
#define realpath(N, R) _fullpath((R), (N), _MAX_PATH)
#else
#define ZURI_PATH_SEPARATOR "/"
#endif /* ifdef _WIN32 */

char *get_exe_path();

char *get_exe_dir();

char *merge_paths(char *a, char *b);

bool file_exists(char *filepath);

char *get_zuri_filename(char *filename);

char *resolve_import_path(char *module_name, const char *current_file, const char *root_file, bool is_relative);

char *get_core_library_file_path(char *module_name);

char *get_real_file_name(char *path);


#if defined(HAVE_UNISTD_H)
#include <unistd.h>
#else
#include "bunistd.h"
#endif /* HAVE_UNISTD_H */
#include "util.h"

#include <stdlib.h>
#include <string.h>

#if defined(_WIN32)
#include "io.h"
#include <sdkddkver.h>

#define WIN32_LEAN_AND_MEAN
#include <Shlwapi.h>
#include <Windows.h>

// #define access _access_s
#endif

#if defined(__APPLE__)

#include <libgen.h>
#include <limits.h>
#include <mach-o/dyld.h>
#include <printf.h>

#endif

#if defined(__linux__) || defined(__CYGWIN__) ||                               \
defined(__MINGW32_MAJOR_VERSION)
#include <libgen.h>
#include <limits.h>

#if defined(__sun)
#define PROC_SELF_EXE "/proc/self/path/a.out"
#else
#define PROC_SELF_EXE "/proc/self/exe"
#endif

#endif

#if defined(_WIN32) && !defined(HAVE_DIRNAME)

char *dirname(const char *path) {
  char drive[_MAX_DRIVE];
  char dir[_MAX_DIR];

  errno_t err =
      _splitpath_s(path, drive, _MAX_DRIVE, dir, _MAX_DIR, NULL, 0, NULL, 0);
  if (err != 0)
    return NULL;

  char *buf = NULL;
  size_t sz = (strlen(drive) + strlen(dir) + 2) * sizeof(*buf);
  buf = malloc(sz);
  if (buf == NULL)
    return NULL;

  if (strlen(drive) == 0) {
    strcpy_s(buf, sz, dir);
  } else {
    strcpy_s(buf, sz, drive);
    strcat_s(buf, sz, "\\");
    strcat_s(buf, sz, dir);
  }

  return buf;
}

#endif /* defined(_WIN32) && !defined(HAVE_DIRNAME) */
#if defined(_WIN32) && !defined(HAVE_BASENAME)

char *basename(const char *path) {
  char fname[_MAX_FNAME];
  char ext[_MAX_EXT];

  errno_t err =
      _splitpath_s(path, NULL, 0, NULL, 0, fname, _MAX_FNAME, ext, _MAX_EXT);
  if (err != 0)
    return NULL;

  char *buf = NULL;
  size_t sz = (strlen(fname) + strlen(ext) + 2) * sizeof(*buf);
  buf = malloc(sz);
  if (buf == NULL)
    return NULL;

  strcpy_s(buf, sz, fname);
  if (strlen(ext) != 0) {
    strcat_s(buf, sz, ".");
    strcat_s(buf, sz, ext);
  }

  return buf;
}

#endif

#endif