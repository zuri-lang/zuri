#include "capture.h"
#include <string.h>

#if defined(_WIN32) || defined(_WIN64)
#define fileno _fileno
#define dup2 _dup2
#define close _close
#endif

// Helper to resolve the correct standard stream pointers and file descriptors
static int resolve_stream(z_capture_stream_t stream, FILE **stream_fp, int *fd) {
  if (stream == Z_CAPTURE_STDOUT) {
    *stream_fp = stdout;
  } else if (stream == Z_CAPTURE_STDERR) {
    *stream_fp = stderr;
  } else {
    return -1;
  }

  // Dynamically retrieve the underlying file descriptor instead of hardcoding it
  *fd = fileno(*stream_fp);
  return (*fd == -1) ? -1 : 0;
}

#if defined(_WIN32) || defined(_WIN64)

// Windows background thread to continually drain the pipe and prevent deadlocks
static unsigned __stdcall win_pipe_reader(void *arg) {
  ZWinReaderContext *ctx = (ZWinReaderContext *)arg;
  char chunk;
  int bytes_read;

  while ((bytes_read = _read(ctx->read_fd, &chunk, sizeof(chunk))) > 0) {
    char *new_buf = realloc(ctx->buffer, ctx->size + bytes_read + 1);
    if (!new_buf) break; // Out of memory, break and return what we have

    ctx->buffer = new_buf;
    memcpy(ctx->buffer + ctx->size, &chunk, bytes_read);
    ctx->size += bytes_read;
    ctx->buffer[ctx->size] = '\0';
  }
  close(ctx->read_fd);
  return 0;
}

z_capture_t* capture_start(z_capture_stream_t stream) {
  FILE *target_fp; int target_fd;
  if (resolve_stream(stream, &target_fp, &target_fd) != 0) return NULL;

  z_capture_t *ctx = malloc(sizeof(z_capture_t));
  if (!ctx) return NULL;

  int pipe_fds[2];
  if (_pipe(pipe_fds, 4096, _O_BINARY) == -1) {
    free(ctx);
    return NULL;
  }

  ctx->target_stream = target_fp;
  ctx->old_std_fd = _dup(fileno(target_fp));
  ctx->pipe_write_fd = pipe_fds[1];

  ctx->reader_ctx.read_fd = pipe_fds[0];
  ctx->reader_ctx.buffer = NULL;
  ctx->reader_ctx.size = 0;

  // Redirect the target standard stream to the write-end of our pipe
  dup2(ctx->pipe_write_fd, fileno(target_fp));

  // Spawn the async consumer thread to prevent pipe capacity freezes
  ctx->thread_handle = (HANDLE)_beginthreadex(NULL, 0, win_pipe_reader, &ctx->reader_ctx, 0, NULL);
  return ctx;
}

char* capture_stop(z_capture_t *ctx) {
  if (!ctx) return NULL;

  // Flush any data left in C's user-space buffers into the OS pipe
  fflush(ctx->target_stream);

  // Instantly restore original stream descriptor back to console
  dup2(ctx->old_std_fd, fileno(ctx->target_stream));
  close(ctx->old_std_fd);
  close(ctx->pipe_write_fd); // Closing write-end triggers EOF in reader thread

  // Wait for reader thread to finish pulling remaining bytes
  WaitForSingleObject(ctx->thread_handle, INFINITE);
  CloseHandle(ctx->thread_handle);

  char *result = ctx->reader_ctx.buffer;
  free(ctx);
  return result;
}

#else // Linux / macOS (POSIX) Implementation

z_capture_t* capture_start(z_capture_stream_t stream) {
  FILE *target_fp; int target_fd;
  if (resolve_stream(stream, &target_fp, &target_fd) != 0) return NULL;

  z_capture_t *ctx = malloc(sizeof(z_capture_t));
  if (!ctx) return NULL;

  ctx->stream_type = stream;
  ctx->target_stream = target_fp;
  ctx->buffer = NULL;
  ctx->size = 0;

  // Leverage native POSIX dynamic in-memory stream allocation
  ctx->mem_stream = open_memstream(&ctx->buffer, &ctx->size);
  if (!ctx->mem_stream) {
    free(ctx);
    return NULL;
  }

  ctx->old_std_fd = dup(target_fd);
  ctx->saved_stream_fp = target_fp;

  // Redirect global standard file pointers to our open memory stream
  if (stream == Z_CAPTURE_STDOUT) stdout = ctx->mem_stream;
  else stderr = ctx->mem_stream;

  return ctx;
}

char* capture_stop(z_capture_t *ctx) {
  if (!ctx) return NULL;

  // Finalize the memstream buffer properties (updates buffer and size pointers)
  fclose(ctx->mem_stream);

  // Restore standard global stream pointers
  if (ctx->stream_type == Z_CAPTURE_STDOUT) stdout = ctx->saved_stream_fp;
  else stderr = ctx->saved_stream_fp;

  // Restore standard low-level descriptors
  int target_fd = (ctx->stream_type == Z_CAPTURE_STDOUT) ? 1 : 2;
  dup2(ctx->old_std_fd, target_fd);
  close(ctx->old_std_fd);

  char *result = ctx->buffer;
  free(ctx);
  return result;
}

#endif

void capture_free(char *buffer) {
  if (buffer) {
    free(buffer);
  }
}
