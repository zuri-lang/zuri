#ifndef ZURI_ZURI_DLFCN_H
#define ZURI_ZURI_DLFCN_H

// Based on Stack Overflow answer at https://stackoverflow.com/a/53532799/5125586
// @TODO: Make implementation thread safe...

#define RTLD_GLOBAL 0x100 /* do not hide entries in this module */
#define RTLD_LOCAL  0x000 /* hide entries in this module */

#define RTLD_LAZY   0x000 /* accept unresolved externs */
#define RTLD_NOW    0x001 /* abort if module has unresolved externs */

#include <inttypes.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <windows.h>

#ifdef __cplusplus
extern "C" {
#endif

static struct {
  char *lasterror;
  const char *err_rutin;
} var = {
    0,
    NULL
};

static char *GetLastErrorStr() {
  DWORD error_message_id = GetLastError();
  if(error_message_id == 0) {
    return NULL; //No error message has been recorded
  }

  LPSTR message_buffer = NULL;

  //Ask Win32 to give us the string version of that message ID.
  //The parameters we pass in, tell Win32 to create the buffer that holds the message for us (because we don't yet know how long the message string will be).
  size_t size = FormatMessageA(
  FORMAT_MESSAGE_ALLOCATE_BUFFER | FORMAT_MESSAGE_FROM_SYSTEM | FORMAT_MESSAGE_IGNORE_INSERTS,
         NULL,
         error_message_id,
         MAKELANGID(LANG_NEUTRAL, SUBLANG_DEFAULT),
         (LPSTR)&message_buffer,
         0,
         NULL
  );

  if(size > 0) {
    //Copy the error message into a std::string.
    char *error = (char *) calloc(size, sizeof(char));
    memcpy(error, message_buffer, size - 1);
    error[size - 1] = '\0';

    //Free the Win32's string's buffer.
    LocalFree(message_buffer);

    return error;
  }

  return NULL;
}

static void *dlopen(const char *filename, int flags) {
  HINSTANCE hInst;

  hInst= LoadLibrary (filename);
  if (hInst==NULL) {
    var.lasterror = GetLastErrorStr();
    var.err_rutin = "dlopen";
  }
  return hInst;
}

static int dlclose(void *handle) {
  BOOL ok;
  int rc= 0;

  ok= FreeLibrary ((HINSTANCE)handle);
  if (! ok) {
    var.lasterror = GetLastErrorStr();
    var.err_rutin = "dlclose";
    rc= -1;
  }
  return rc;
}

static void *dlsym(void *handle, const char *name){
  FARPROC fp;

  fp= GetProcAddress ((HINSTANCE)handle, name);
  if (!fp) {
    var.lasterror = GetLastErrorStr();
    var.err_rutin = "dlsym";
  }
  return (void *)(intptr_t)fp;
}

static const char *dlerror(void) {
  return (const char *)var.lasterror;
}

#ifdef __cplusplus
}
#endif

#endif //ZURI_ZURI_DLFCN_H
