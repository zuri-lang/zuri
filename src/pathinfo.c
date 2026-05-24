#if defined(_MSC_VER)
#pragma warning(disable : 5105)
#endif

#include "pathinfo.h"
#include "common.h"

#ifdef _WIN32

char *get_exe_path() {
  char *exe_path = (char *) malloc(sizeof(char) * MAX_PATH);
  if (exe_path != NULL) {
    if ((int) GetModuleFileNameA(NULL, exe_path, MAX_PATH) > 0) {
      return exe_path;
    }
  }
  return NULL;
}

char *get_exe_dir() {
  char *exe_path = get_exe_path();
  if (exe_path != NULL) {
    return dirname(exe_path);
  }
  return NULL;
}

#elif defined(__linux__) || defined(__CYGWIN__)

char *get_exe_path() {
  char raw_path[PATH_MAX];
  ssize_t read_length;
  if ((read_length = readlink(PROC_SELF_EXE, raw_path, sizeof(raw_path))) >
          -1 &&
      read_length < PATH_MAX) {
    raw_path[read_length] = '\0';
    return strdup(raw_path);
  }
  return NULL;
}

#elif defined(__APPLE__)

char *get_exe_path() {
  char raw_path[PATH_MAX];
  char *real_path = (char *) malloc(PATH_MAX * sizeof(char));
  uint32_t raw_size = (uint32_t) sizeof(raw_path);

  if (!_NSGetExecutablePath(raw_path, &raw_size)) {
    realpath(raw_path, real_path);
  }
  return real_path;
}

#endif

#if defined(__CYGWIN__) || defined(__linux__) || defined(__APPLE__)

char *get_exe_dir() {
  char *path = get_exe_path();
  char *result = dirname(path);
#ifdef __APPLE__
  free(path);
#endif
  return result;
}

#endif

static inline void free_tmp(char *tmp_str) {
#ifdef __APPLE__
  free(tmp_str);
  tmp_str = NULL;
#endif
}

char *merge_paths(char *a, char *b) {
  char *final_path = (char *) calloc(1, sizeof(char));

  // by checking b first, we guarantee that b is neither NULL nor
  // empty by the time we are checking a so that we can return a
  // duplicate of b
  int len_b = (int)strlen(b);
  if(b == NULL || len_b == 0) {
    free(final_path);
    return strdup(a); // just in case a is const char*
  } if(a == NULL || strlen(a) == 0) {
    free(final_path);
    return strdup(b); // just in case b is const char*
  }

  final_path = append_strings(final_path, a);

  if(!(len_b == 3 && memcmp(b, ZURI_EXTENSION, len_b) == 0)) {
    final_path = append_strings(final_path, ZURI_PATH_SEPARATOR);
  }
  final_path = append_strings(final_path, b);
  return final_path;
}

bool file_exists(char *filepath) { return access(filepath, F_OK) == 0; }

char *get_zuri_filename(char *filename) {
  return merge_paths(filename, ZURI_EXTENSION);
}

#include <stdio.h>
char *get_core_library_file_path(char *module_name) {
  char *zuri_file_name = get_zuri_filename(module_name);
  if (zuri_file_name != NULL) {
#ifdef __APPLE__
    char *exe_dir = strdup(get_exe_dir());
#else
    char *exe_dir = get_exe_dir();
#endif

    if (exe_dir != NULL) {
      char *zuri_directory = merge_paths(exe_dir, LIBRARY_DIRECTORY);

      // check zuri libs directory for a matching module...
      if (zuri_directory != NULL) {
        char *library_file = merge_paths(zuri_directory, zuri_file_name);

        if (library_file != NULL) {
          if (file_exists(library_file)) {
            // stop a core library from importing itself
            char *path1 = realpath(library_file, NULL);

            if (path1 != NULL) {
              free(library_file);
              free(zuri_directory);
              // free(exe_dir);
              free(zuri_file_name);
              return path1;
            }
          }
          free(library_file);
        }
        free_tmp(zuri_directory);
      }
      free(exe_dir);
    }
    free(zuri_file_name);
  }

  return NULL;
}

char *resolve_import_path(char *module_name, const char *current_file, const char *root_file, bool is_relative) {
  char *zuri_file_name = get_zuri_filename(module_name);

  // search system library if we are not looking for a relative module.
  if (!is_relative) {
    char *tmp_str = NULL;

    // firstly, search the local vendor directory for a matching module
    char *root_dir = NULL;
    if(root_file == NULL) {
      root_dir = getcwd(NULL, 0);
    } else {
      tmp_str = strdup(root_file);
      root_dir = dirname(tmp_str);
      free_tmp(tmp_str);
      tmp_str = NULL;
    }

    char *current_dir = getcwd(NULL, 0);
    // fixing last path / if exists (looking at windows)...
    int root_dir_length = (int) strlen(root_dir);
    if (root_dir[root_dir_length - 1] == '\\') {
      root_dir[root_dir_length - 1] = '\0';
    }

    tmp_str = merge_paths(root_dir, LOCAL_PACKAGES_DIRECTORY LOCAL_SRC_DIRECTORY);
    char *vendor_file = merge_paths(tmp_str, zuri_file_name);
    free(tmp_str);
    tmp_str = NULL;
    if (file_exists(vendor_file)) {
      // stop a core library from importing itself
      char *path1 = realpath(vendor_file, NULL);
      char *path2 = realpath(current_file, NULL);

      if (path1 != NULL) {
        if (path2 == NULL || memcmp(path1, path2, (int) strlen(path2)) != 0) {
          free(current_dir);
          free(path2);
          free(vendor_file);
          free(zuri_file_name);
          return path1;
        }
      }
    }
    free(vendor_file);

    // or a matching package
    tmp_str = merge_paths(root_dir, LOCAL_PACKAGES_DIRECTORY LOCAL_SRC_DIRECTORY);
    char *merged_path = merge_paths(tmp_str, module_name);
    char *vendor_index_file = merge_paths(merged_path, LIBRARY_DIRECTORY_INDEX ZURI_EXTENSION);
    free(merged_path);
    free(tmp_str);
    tmp_str = NULL;
    if (file_exists(vendor_index_file)) {
      // stop a core library from importing itself
      char *path1 = realpath(vendor_index_file, NULL);
      char *path2 = realpath(current_file, NULL);

      if (path1 != NULL) {
        if (path2 == NULL || memcmp(path1, path2, (int) strlen(path2)) != 0) {
          free(current_dir);
          free(path2);
          free(vendor_index_file);
          free(zuri_file_name);
          return path1;
        }
      }
    }
    free(vendor_index_file);

    tmp_str = merge_paths(current_dir, LOCAL_PACKAGES_DIRECTORY LOCAL_SRC_DIRECTORY);
    char *current_vendor_file = merge_paths(tmp_str, zuri_file_name);
    free(tmp_str);
    tmp_str = NULL;
    if (file_exists(current_vendor_file)) {
      // stop a core library from importing itself
      char *path1 = realpath(current_vendor_file, NULL);
      char *path2 = realpath(current_file, NULL);

      if (path1 != NULL) {
        if (path2 == NULL || memcmp(path1, path2, (int) strlen(path2)) != 0) {
          free(current_dir);
          free(path2);
          free(current_vendor_file);
          free(zuri_file_name);
          return path1;
        }
      }
    }
    free(current_vendor_file);

    // or a matching package
    tmp_str = merge_paths(current_dir, LOCAL_PACKAGES_DIRECTORY LOCAL_SRC_DIRECTORY);
    char *merged_path2 = merge_paths(tmp_str, module_name);
    char *current_vendor_index_file = merge_paths(merged_path2, LIBRARY_DIRECTORY_INDEX ZURI_EXTENSION);
    free(merged_path2);
    free(tmp_str);
    tmp_str = NULL;
    if (file_exists(current_vendor_index_file)) {
      // stop a core library from importing itself
      char *path1 = realpath(current_vendor_index_file, NULL);
      char *path2 = realpath(current_file, NULL);

      if (path1 != NULL) {
        if (path2 == NULL || memcmp(path1, path2, (int) strlen(path2)) != 0) {
          free(current_dir);
          free(path2);
          free(current_vendor_index_file);
          free(zuri_file_name);
          return path1;
        }
      }
    }
    free(current_vendor_index_file);
    free(current_dir);

    // then, check in zuri's default locations
    char *exe_dir = get_exe_dir();
    char *zuri_directory = merge_paths(exe_dir, LIBRARY_DIRECTORY);

    // check zuri libs directory for a matching module...
    char *library_file = merge_paths(zuri_directory, zuri_file_name);
    if (file_exists(library_file)) {
      // stop a core library from importing itself
      char *path1 = realpath(library_file, NULL);
      char *path2 = realpath(current_file, NULL);

      if (path1 != NULL) {
        if (path2 == NULL || memcmp(path1, path2, (int) strlen(path2)) != 0) {
          free(zuri_directory);
          free(path2);
          free(library_file);
          free(zuri_file_name);
          return path1;
        }
      }
    }
    free(library_file);

    // check zuri libs directory for a matching package...
    tmp_str = merge_paths(zuri_directory, module_name);
    char *library_index_file = merge_paths(tmp_str, LIBRARY_DIRECTORY_INDEX ZURI_EXTENSION);
    free(tmp_str);
    tmp_str = NULL;
    if (file_exists(library_index_file)) {
      char *path1 = realpath(library_index_file, NULL);
      char *path2 = realpath(current_file, NULL);

      if (path1 != NULL) {
        if (path2 == NULL || memcmp(path1, path2, (int) strlen(path2)) != 0) {
          free(zuri_directory);
          free(path2);
          free(library_index_file);
          free(zuri_file_name);
          return path1;
        }
      }
    }
    free(library_index_file);

    // check zuri vendor directory installed module...
    char *zuri_package_directory = merge_paths(exe_dir, PACKAGES_DIRECTORY);
    char *package_file = merge_paths(zuri_package_directory, zuri_file_name);
    if (file_exists(package_file)) {
      char *path1 = realpath(package_file, NULL);
      char *path2 = realpath(current_file, NULL);

      if (path1 != NULL) {
        if (path2 == NULL || memcmp(path1, path2, (int) strlen(path2)) != 0) {
          free(path2);
          free(package_file);
          free(zuri_file_name);
          return path1;
        }
      }
    }
    free(package_file);

    // check zuri vendor directory installed package...
    tmp_str = merge_paths(zuri_package_directory, module_name);
    char *package_index_file = merge_paths(tmp_str, LIBRARY_DIRECTORY_INDEX ZURI_EXTENSION);
    free(tmp_str);
    tmp_str = NULL;
    if (file_exists(package_index_file)) {
      char *path1 = realpath(package_index_file, NULL);
      char *path2 = realpath(current_file, NULL);

      if (path1 != NULL) {
        if (path2 == NULL || memcmp(path1, path2, (int) strlen(path2)) != 0) {
          free(path2);
          free(package_index_file);
          free(zuri_package_directory);
          free(zuri_directory);
          free(zuri_file_name);
          return path1;
        }
      }
    }

    free(zuri_package_directory);
    free(zuri_directory);
  } else {

    // check relative to the current file...
    char *tmp_str = strdup(current_file);
    char *file_directory = dirname((char *) tmp_str);
    free_tmp(tmp_str);

    // fixing last path / if exists (looking at windows)...
    int file_directory_length = (int) strlen(file_directory);
    if (file_directory[file_directory_length - 1] == '\\') {
      file_directory[file_directory_length - 1] = '\0';
    }

    // otherwise, search the relative path for a matching module
    char *relative_file = merge_paths(file_directory, zuri_file_name);
    if (file_exists(relative_file)) {
      // stop a user module from importing itself
      char *path1 = realpath(relative_file, NULL);
      char *path2 = realpath(current_file, NULL);

      if (path1 != NULL) {
        if (path2 == NULL || memcmp(path1, path2, (int) strlen(path2)) != 0) {
          free(path2);
          free(relative_file);
          free(zuri_file_name);
          return path1;
        }
      }
    }
    free(relative_file);

    // or a matching package
    tmp_str = merge_paths(file_directory, module_name);
    char *relative_index_file = merge_paths(tmp_str, LIBRARY_DIRECTORY_INDEX ZURI_EXTENSION);
    free(tmp_str);
    tmp_str = NULL;
    if (file_exists(relative_index_file)) {
      char *path1 = realpath(relative_index_file, NULL);
      char *path2 = realpath(current_file, NULL);

      if (path1 != NULL) {
        if (path2 == NULL || memcmp(path1, path2, (int) strlen(path2)) != 0) {
          free(path2);
          free(relative_index_file);
          free(zuri_file_name);
          return path1;
        }
      }
    }
    free(relative_index_file);
  }

  free(zuri_file_name);
  return NULL;
}

char *get_real_file_name(char *path) { return basename(path); }
