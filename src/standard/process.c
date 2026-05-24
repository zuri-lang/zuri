#include "module.h"

#ifdef HAVE_SYSCONF
#include <sys/ipc.h>
#include <sys/shm.h>
#endif
#ifdef HAVE_SYSCTLBYNAME
#include <sys/sysctl.h>
#endif

#ifdef _WIN32
#include "mman-win32/mman.h"
#else
#include <sys/mman.h>
#endif


#ifdef HAVE_UNISTD_H
#include <unistd.h>
#ifndef _WIN32
#include <sys/wait.h>
#endif
#else
#include "zunistd.h"
#endif /* HAVE_UNISTD_H */

#if defined(__OpenBSD__) && defined(__powerpc__)
#include <sys/param.h>
#include <machine/cpu.h>
#elif defined(_WIN32)
#include <windows.h>
#endif
#ifdef HAVE_SETJMP
#include <signal.h>
#endif

#include <errno.h>

// windows build stubs...
#ifdef _WIN32
#define WNOHANG 1
#define SIGKILL 1

static int fork() {
  errno = ENODEV;
  return -1;
}

static int waitpid(int i, void *j, int k) {
  return -1;
}

static int kill(int i, int j) {
  return -1;
}
#endif

z_value __process_cpu_count(z_vm *vm) {
#if defined(HAVE_SYSCONF) && defined(_SC_NPROCESSORS_ONLN)
  return NUMBER_VAL((int)sysconf(_SC_NPROCESSORS_ONLN));
#endif
#ifdef HAVE_SYSCTLBYNAME
  int count = 0;
  size_t size = sizeof(count);
  sysctlbyname("hw.ncpu", &count, &size, NULL, 0);
  return NUMBER_VAL(count);
#endif
#ifdef __WIN32__
  SYSTEM_INFO info;
  GetSystemInfo(&info);
  return NUMBER_VAL(info.dwNumberOfProcessors);
#endif
#ifdef __OS2__
  int count = 0;
  DosQuerySysInfo(QSV_NUMPROCESSORS, QSV_NUMPROCESSORS, &count, sizeof(count));
  return NUMBER_VAL(count);
#endif
  return NUMBER_VAL(1);
}

typedef struct {
  pid_t pid;
} BProcess;

typedef struct {
  char *format;
  char *get_format;
  unsigned char *bytes;
  int format_length;
  int get_format_length;
  int length;
  bool locked;
  int flags;
  int protection;
  int exectuable;
} BProcessPaged;

void z__free_paged_memory(void *data) {
  BProcessPaged *paged = (BProcessPaged *)data;
  munmap(paged->format, paged->format_length * sizeof(char));
  munmap(paged->get_format, paged->get_format_length * sizeof(char));
  munmap(paged->bytes, paged->length * sizeof(unsigned char));
  munmap(paged, sizeof(BProcessPaged));
}

DECLARE_MODULE_METHOD(process_Process) {
  ENFORCE_ARG_RANGE(Process, 0, 1);
  BProcess *process = ALLOCATE(BProcess, 1);
  z_obj_ptr *ptr = (z_obj_ptr *)GC(new_ptr(vm, process));
  ptr->name = "<*Process::Process>";
  process->pid = -1;
  RETURN_OBJ(ptr);
}

DECLARE_MODULE_METHOD(process_create) {
  ENFORCE_ARG_COUNT(create, 1);
  ENFORCE_ARG_TYPE(create, 0, IS_PTR);
  BProcess *process = (BProcess *) AS_PTR(args[0])->pointer;
  pid_t pid = fork();
  if(pid == -1) {
    RETURN_NUMBER(-1);
  } else if(!pid) {
    process->pid = getpid();
    RETURN_NUMBER(0);
  }
  RETURN_NUMBER(getpid());
}

DECLARE_MODULE_METHOD(process_is_alive) {
  ENFORCE_ARG_COUNT(create, 1);
  ENFORCE_ARG_TYPE(create, 0, IS_PTR);
  BProcess *process = (BProcess *) AS_PTR(args[0])->pointer;
  if (!process || process->pid <= 0) RETURN_FALSE;
  int rc;
  do {
    rc = waitpid(process->pid, NULL, WNOHANG);
  } while (rc == -1 && errno == EINTR);
  RETURN_BOOL(rc == 0);
}

DECLARE_MODULE_METHOD(process_kill) {
  ENFORCE_ARG_COUNT(kill, 1);
  ENFORCE_ARG_TYPE(kill, 0, IS_PTR);
  BProcess *process = (BProcess *) AS_PTR(args[0])->pointer;
  if (!process || process->pid <= 0) RETURN_FALSE;
  int rc;
  do {
    rc = kill(process->pid, SIGKILL);
  } while (rc == -1 && errno == EINTR);
  RETURN_BOOL(rc == 0);
}

DECLARE_MODULE_METHOD(process_wait) {
  ENFORCE_ARG_COUNT(create, 1);
  ENFORCE_ARG_TYPE(create, 0, IS_PTR);
  BProcess *process = (BProcess *) AS_PTR(args[0])->pointer;

  int status;

  pid_t p;
  do {
    p = waitpid(process->pid, &status, 0);
    if (p == -1) {
      if (errno == EINTR)
        continue;
      break;
    }
  } while (p != process->pid);

  if(p == process->pid) {
    RETURN_NUMBER(status);
  }
  RETURN_NUMBER(-1);
}

DECLARE_MODULE_METHOD(process_id) {
  ENFORCE_ARG_COUNT(create, 1);
  ENFORCE_ARG_TYPE(create, 0, IS_PTR);
  BProcess *process = (BProcess *) AS_PTR(args[0])->pointer;
  RETURN_NUMBER(process->pid);
}

DECLARE_MODULE_METHOD(process_new_paged) {
  ENFORCE_ARG_COUNT(new_paged, 2);
  ENFORCE_ARG_TYPE(new_paged, 0, IS_BOOL); // executable
  ENFORCE_ARG_TYPE(new_paged, 1, IS_BOOL); // private

  int protection = PROT_READ | PROT_WRITE;
  int flags = MAP_ANONYMOUS;
  if(AS_BOOL(args[1])) {
    flags |= MAP_PRIVATE;
  } else {
    flags |= MAP_SHARED;
  }

  BProcessPaged *paged = mmap(NULL, sizeof(BProcessPaged), protection, flags, -1, 0);

  if(paged != MAP_FAILED) {
    paged->protection = protection;
    paged->exectuable = AS_BOOL(args[0]);
    paged->flags = flags;

    paged->bytes = NULL;
    paged->format = mmap(NULL, sizeof(char), protection, flags, -1, 0);
    paged->get_format = mmap(NULL, sizeof(char), protection, flags, -1, 0);
    paged->length = paged->get_format_length = paged->format_length = 0;
    z_obj_ptr *ptr = (z_obj_ptr *)GC(new_ptr(vm, paged));
    ptr->name = "<*Process::PagedValue>";
    ptr->free_fn = z__free_paged_memory;
    RETURN_OBJ(ptr);
  }

  RETURN_NIL;
}


#if __APPLE__
// we are doing this to avoid write protection on Apple devices
// most especially M1 and M2 devices.
#include <pthread.h>
#endif
DECLARE_MODULE_METHOD(process_paged_write) {
  ENFORCE_ARG_COUNT(paged_write, 4);
  ENFORCE_ARG_TYPE(paged_write, 0, IS_PTR);
  ENFORCE_ARG_TYPE(paged_write, 1, IS_STRING);
  ENFORCE_ARG_TYPE(paged_write, 2, IS_STRING);
  ENFORCE_ARG_TYPE(paged_write, 3, IS_BYTES);

  BProcessPaged *paged = (BProcessPaged *)AS_PTR(args[0])->pointer;
  if(!paged->locked) {
    z_obj_string *format = AS_STRING(args[1]);
    z_obj_string *get_format = AS_STRING(args[2]);
    z_byte_arr bytes = AS_BYTES(args[3])->bytes;

    memcpy(paged->format, format->chars, format->length);
    paged->format_length = format->length;

    memcpy(paged->get_format, get_format->chars, get_format->length);
    paged->get_format_length = get_format->length;

    if(paged->bytes != NULL) {
      munmap(paged->bytes, paged->length);
      paged->bytes = NULL;
    }

    int data_protection = paged->protection;
    if(paged->exectuable) {
      data_protection |= PROT_EXEC;
    }
    int data_flags = paged->flags;
#if __APPLE__
    if(paged->exectuable) {
      data_flags |= MAP_JIT;
    }

    // we are doing this to avoid write protection on Apple devices
    // most especially M1 and M2 devices.
    pthread_jit_write_protect_np(false);
#endif

    size_t data_size = bytes.count * sizeof(unsigned char);
    paged->bytes = mmap(NULL, data_size, data_protection, data_flags, -1, 0);

    memmove(paged->bytes, bytes.bytes, bytes.count);
    paged->length = bytes.count;

#if __APPLE__
    if(paged->exectuable) {
      // we are doing this to avoid write protection on Apple devices
      // most especially M1 and M2 devices.
      pthread_jit_write_protect_np(true);
    }
#endif

    // return length written
    RETURN_NUMBER(paged->length);
  }

  RETURN_FALSE;
}

DECLARE_MODULE_METHOD(process_paged_read) {
  ENFORCE_ARG_COUNT(paged_read, 1);
  ENFORCE_ARG_TYPE(paged_read, 0, IS_PTR);
  BProcessPaged *paged = (BProcessPaged *)AS_PTR(args[0])->pointer;

  if(paged->length > 0 || paged->format_length > 0) {
    z_obj_bytes *bytes = (z_obj_bytes *) GC(copy_bytes(vm, paged->bytes, paged->length));

    // return [format, bytes]
    z_obj_list *list = (z_obj_list *)GC(new_list(vm));
    write_list(vm, list, STRING_L_VAL(paged->get_format, paged->get_format_length));
    write_list(vm, list, OBJ_VAL(bytes));

    RETURN_OBJ(list);
  }
  RETURN_NIL;
}

DECLARE_MODULE_METHOD(process_paged_lock) {
  ENFORCE_ARG_COUNT(paged_lock, 1);
  ENFORCE_ARG_TYPE(paged_lock, 0, IS_PTR);
  BProcessPaged *paged = (BProcessPaged *)AS_PTR(args[0])->pointer;
  paged->locked = true;
  RETURN;
}

DECLARE_MODULE_METHOD(process_paged_unlock) {
  ENFORCE_ARG_COUNT(paged_unlock, 1);
  ENFORCE_ARG_TYPE(paged_unlock, 0, IS_PTR);
  BProcessPaged *paged = (BProcessPaged *)AS_PTR(args[0])->pointer;
  paged->locked = false;
  RETURN;
}

DECLARE_MODULE_METHOD(process_paged_islocked) {
  ENFORCE_ARG_COUNT(paged_islocked, 1);
  ENFORCE_ARG_TYPE(paged_islocked, 0, IS_PTR);
  BProcessPaged *paged = (BProcessPaged *)AS_PTR(args[0])->pointer;
  RETURN_BOOL(paged->locked);
}

DECLARE_MODULE_METHOD(process_raw_pointer) {
  ENFORCE_ARG_COUNT(raw_pointer, 1);
  ENFORCE_ARG_TYPE(raw_pointer, 0, IS_PTR);
  BProcessPaged *paged = (BProcessPaged *)AS_PTR(args[0])->pointer;
  RETURN_PTR(paged->bytes);
}

CREATE_MODULE_LOADER(process) {
  static z_func_reg os_module_functions[] = {
      {"Process",     false, GET_MODULE_METHOD(process_Process)},
      {"create",     false, GET_MODULE_METHOD(process_create)},
      {"is_alive",     false, GET_MODULE_METHOD(process_is_alive)},
      {"wait",     false, GET_MODULE_METHOD(process_wait)},
      {"id",     false, GET_MODULE_METHOD(process_id)},
      {"kill",     false, GET_MODULE_METHOD(process_kill)},
      {"new_paged",     false, GET_MODULE_METHOD(process_new_paged)},
      {"paged_write",     false, GET_MODULE_METHOD(process_paged_write)},
      {"paged_read",     false, GET_MODULE_METHOD(process_paged_read)},
      {"paged_lock",     false, GET_MODULE_METHOD(process_paged_lock)},
      {"paged_unlock",     false, GET_MODULE_METHOD(process_paged_unlock)},
      {"raw_pointer",     false, GET_MODULE_METHOD(process_raw_pointer)},
      {NULL,     false, NULL},
  };

  static z_field_reg os_module_fields[] = {
      {"cpu_count", true, __process_cpu_count},
      {NULL,       false, NULL},
  };

  static z_module_reg module = {
      .name = "_process",
      .fields = os_module_fields,
      .functions = os_module_functions,
      .classes = NULL,
      .preloader= NULL,
      .unloader = NULL
  };

  return &module;
}
