#ifndef ZURI_CAPTURE_H
#define ZURI_CAPTURE_H

#include <stdio.h>
#include <stdlib.h>

typedef enum {
  Z_CAPTURE_STDOUT,
  Z_CAPTURE_STDERR
} z_capture_stream_t;

#if defined(_WIN32) || defined(_WIN64)
#include <io.h>
#include <fcntl.h>
#include <process.h>
#include <windows.h>

typedef struct {
  int read_fd;
  char *buffer;
  size_t size;
} ZWinReaderContext;

typedef struct {
  FILE *target_stream;
  int old_std_fd;
  int pipe_write_fd;
  HANDLE thread_handle;
  ZWinReaderContext reader_ctx;
} z_capture_t;
#else
#include <unistd.h>

typedef struct {
  z_capture_stream_t stream_type;
  FILE *target_stream;
  FILE *saved_stream_fp;
  int old_std_fd;
  FILE *mem_stream;
  char *buffer;
  size_t size;
} z_capture_t;
#endif

// Symmetrical, Analogous API
z_capture_t* capture_start(z_capture_stream_t stream);
char*      capture_stop(z_capture_t *ctx);
void       capture_free(char *buffer);

#endif //ZURI_CAPTURE_H
