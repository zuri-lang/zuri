#include "module.h"
#include <pthread.h>
#include <errno.h>

#ifdef __linux__
#include <sys/syscall.h>
#elif defined(__FreeBSD__)
#include <sys/thr.h>
#elif defined(__NetBSD__)
#include <lwp.h>
#endif

#ifdef HAVE_UNISTD_H
#include <unistd.h>
#else
#include "zunistd.h"
#endif /* HAVE_UNISTD_H */

#ifdef _WIN32
#define _POSIX 1
#include <windows.h>
#include <sys/types.h>

# ifndef SIGUSR2
#   define SIGUSR2 2
# endif
# ifndef SIG_BLOCK
#   define SIG_BLOCK 0
# endif
# ifndef SIG_UNBLOCK
#   define SIG_UNBLOCK 1
# endif
# ifndef SIG_SETMASK
#   define SIG_SETMASK 2
# endif
#endif

#include <sched.h>
#include <signal.h>

#ifdef _WIN32
// Include the reimplementation of sigemptyset, sigaddset, and sigaction
// (You can either put this code in the same file or in a separate header and source file)
typedef struct {
    int signals[32];  // Simple signal set (can hold up to 32 signals)
} sigset_t;

// Thread-local storage for the signal mask
#if defined(_MSC_VER)
#define ZURI_TLS __declspec(thread)
#else
#define ZURI_TLS __thread
#endif
static ZURI_TLS sigset_t thread_sigmask;

int sigemptyset(sigset_t *set) {
    if (set == NULL) return -1;
    for (int i = 0; i < 32; i++) {
        set->signals[i] = 0;
    }
    return 0;
}

int sigaddset(sigset_t *set, int signum) {
    if (set == NULL || signum < 1 || signum > 32) return -1;
    set->signals[signum - 1] = 1;
    return 0;
}

typedef void (*sighandler_t)(int);

struct sigaction {
    sighandler_t sa_handler;
    sigset_t sa_mask;
    int sa_flags;
};

// Global handler to store the current signal handler for SIGINT
static sighandler_t sigint_handler = NULL;

BOOL WINAPI CtrlHandler(DWORD fdwCtrlType) {
    if (fdwCtrlType == CTRL_C_EVENT && sigint_handler != NULL) {
        // Check if SIGUSR2 is blocked in the current thread's mask (bounds-safe)
        int idx = SIGUSR2 - 1;
        if (idx >= 0 && idx < 32) {
            if (thread_sigmask.signals[idx] == 0) {
                sigint_handler(SIGUSR2);  // Call the signal handler
            }
        }
        return TRUE;
    }
    return FALSE;
}

int sigaction(int signum, const struct sigaction *act, struct sigaction *oldact) {
    if (signum == SIGINT) {
        if (oldact != NULL) {
            oldact->sa_handler = sigint_handler;
        }
        if (act != NULL) {
            sigint_handler = act->sa_handler;
            SetConsoleCtrlHandler(CtrlHandler, TRUE);
        }
        return 0;
    }
    return -1;
}

// Emulate pthread_sigmask
int pthread_sigmask2(int how, const sigset_t *set, sigset_t *oldset) {
    if (oldset != NULL) {
        // Save the current signal mask to oldset
        *oldset = thread_sigmask;
    }

    if (set == NULL) {
        return 0;  // No changes if set is NULL
    }

    for (int i = 0; i < 32; i++) {
        switch (how) {
            case SIG_BLOCK:
                // Block signals by adding them to the mask
                if (set->signals[i] == 1) {
                    thread_sigmask.signals[i] = 1;
                }
                break;
            case SIG_UNBLOCK:
                // Unblock signals by removing them from the mask
                if (set->signals[i] == 1) {
                    thread_sigmask.signals[i] = 0;
                }
                break;
            case SIG_SETMASK:
                // Set the signal mask to the given set
                thread_sigmask.signals[i] = set->signals[i];
                break;
            default:
                return -1;  // Invalid how value
        }
    }

    return 0;
}
#endif

#define Z_THREAD_MUTEX_NAME "Thread::Mutex"
#define Z_THREAD_CHANNEL_NAME "Thread::Channel"
#define Z_THREAD_SEMAPHORE_NAME "Thread::Semaphore"

typedef struct {
  pthread_t thread;
  z_vm *vm;
  z_obj_closure *closure;
  z_obj_list *args;
} z_thread_handle;

typedef struct {
  pthread_mutex_t mu;
  int locked;          /* advisory flag for is_locked() */
} z_thread_mutex;

typedef struct {
  pthread_mutex_t  mu;
  pthread_cond_t   not_full;   /* signalled when an item is removed        */
  pthread_cond_t   not_empty;  /* signalled when an item is added          */
  z_value         *buf;        /* ring buffer of z_value                   */
  int              cap;        /* capacity (0 = unbuffered)                */
  int              head;       /* index of oldest item                     */
  int              count;      /* number of items currently buffered       */
  int              closed;
  /* For unbuffered (rendezvous) channels we use a single-slot hand-off. */
  int              sender_waiting;
  int              receiver_waiting;
  z_value          rendezvous_value;
  int              rendezvous_taken;
} z_thread_channel;

typedef struct {
  pthread_mutex_t mu;
  pthread_cond_t  cond;
  int             count;
  int             max;
} z_thread_semaphore;

static void z_thread_mutex_finalizer(void *ptr) {
  if (ptr) {
    z_thread_mutex *m = (z_thread_mutex *)ptr;
    pthread_mutex_destroy(&m->mu);
    free(m);
  }
}

static void z_thread_channel_finalizer(void *ptr) {
  if (ptr) {
    z_thread_channel *c = (z_thread_channel *)ptr;
    pthread_cond_destroy(&c->not_full);
    pthread_cond_destroy(&c->not_empty);
    pthread_mutex_destroy(&c->mu);
    if (c->buf) free(c->buf);
    free(c);
  }
}

static void z_thread_semaphore_finalizer(void *ptr) {
  if (ptr) {
    z_thread_semaphore *s = (z_thread_semaphore *)ptr;
    pthread_cond_destroy(&s->cond);
    pthread_mutex_destroy(&s->mu);
    free(s);
  }
}


static uint64_t last_thread_vm_id = 0;

#define Z_THREAD_PTR_NAME "<void *thread::thread>"

z_vm *copy_vm(z_vm *src, uint64_t id) {
  z_vm *vm = (z_vm *) malloc(sizeof(z_vm));
  if(!vm) {
    return NULL;
  }

  memset(vm, 0, sizeof(z_vm));

  vm->parent_vm = src;
  vm->stack = ALLOCATE(z_value, COPIED_STACK_MIN);
  vm->stack_capacity = COPIED_STACK_MIN;

  // reset stack
  vm->stack_top = vm->stack;
  vm->frame_count = 0;
  vm->open_up_values = NULL;

  // copies properties
  vm->compiler = src->compiler;
  vm->exception_class = src->exception_class;
  vm->root_file = src->root_file;
  vm->is_repl = src->is_repl;
  vm->show_warnings = src->show_warnings;
  vm->should_print_bytecode = src->should_print_bytecode;
  vm->should_exit_after_bytecode = src->should_exit_after_bytecode;
  vm->std_args = src->std_args;
  vm->std_args_count = src->std_args_count;

  // copied globals
  vm->modules = src->modules;
  vm->globals = src->globals;

  // every thread needs to maintain their own copy of the strings
  // without this, the threads will never terminate since the parent
  // vm always holds the root pointer to the strings
  // this will in turn lead to an infinite hang when creating
  // lots of threads in succession.
  init_table(&vm->strings);
  table_copy(vm, &src->strings, &vm->strings);

  // copied object methods
  vm->methods_string = src->methods_string;
  vm->methods_list = src->methods_list;
  vm->methods_dict = src->methods_dict;
  vm->methods_file = src->methods_file;
  vm->methods_bytes = src->methods_bytes;
  vm->methods_range = src->methods_range;

  // own properties
  vm->objects = NULL;
  vm->current_frame = NULL;
  vm->bytes_allocated = 0;
  vm->next_gc = DEFAULT_GC_START / 4; // default is quarter the original set value
  vm->mark_value = true;
  vm->gray_count = 0;
  vm->gray_capacity = 0;
  vm->gray_stack = NULL;
  vm->error_count = 0;

  vm->id = id;

  return vm;
}

static z_thread_handle *create_thread_handle(z_vm *vm, z_obj_closure *closure, z_obj_list *args) {
  if(last_thread_vm_id == UINT64_MAX) {
    // whatever makes us get here, due to resource constraint on devices,
    // the earlier threads definitely will no longer be in operation.
    last_thread_vm_id = 0;
  }

  z_thread_handle *handle = ALLOCATE(z_thread_handle, 1);
  if(handle != NULL) {
    handle->vm = copy_vm(vm, ++last_thread_vm_id);

    if(handle->vm == NULL) {
      FREE(z_thread_handle, handle);
      return NULL;
    }

    handle->closure = closure;
    handle->args = args;

    ((z_obj *)closure)->stale++;
    ((z_obj *)args)->stale++;
  }

  return handle;
}

static void free_thread_handle(z_thread_handle *thread) {
  if(thread != NULL && thread->vm != NULL) {
    // move the surviving items to the parent vm's object list.
    if(thread->vm->parent_vm != NULL) {
      migrate_objects(thread->vm, thread->vm->parent_vm);
    }

    free_vm(thread->vm);

    thread->vm = NULL;
    thread->closure = NULL;
    thread->args = NULL;

    ((z_obj *)thread)->stale--;
  }
}

static void z_free_thread_handle(void *data) {
  z_thread_handle *handle = (z_thread_handle *) data;
  free_thread_handle(handle);
  free(handle);
}

static void *z_thread_callback_function(void *data) {
  // Unblock SIGUSR2
  sigset_t mask;
  sigemptyset(&mask);
  sigaddset(&mask, SIGUSR2);
#ifndef _WIN32
  pthread_sigmask(SIG_UNBLOCK, &mask, NULL);
#else
  pthread_sigmask2(SIG_UNBLOCK, &mask, NULL);
#endif

  z_thread_handle *handle = (z_thread_handle *) data;
  if(handle == NULL || handle->vm == NULL) {
    pthread_exit(NULL);
  }

  for(int i = 0; i < handle->args->items.count; i++) {
    push(handle->vm, handle->args->items.values[i]);
  }

  if(run_closure_call(handle->vm, handle->closure, handle->args) == PTR_OK) {
    // do nothing for now...
  }

  ((z_obj *)handle->closure)->stale--;
  ((z_obj *)handle->args)->stale--;

  free_thread_handle(handle);
  pthread_exit(NULL);
}

DECLARE_MODULE_METHOD(thread__new) {
  ENFORCE_ARG_COUNT(new, 2);
  ENFORCE_ARG_TYPE(new, 0, IS_CLOSURE);
  ENFORCE_ARG_TYPE(new, 1, IS_LIST);

  z_thread_handle *thread = create_thread_handle(vm, AS_CLOSURE(args[0]), AS_LIST(args[1]));
  if(thread != NULL) {
    z_obj_ptr *ptr = new_closable_named_ptr(vm, thread, Z_THREAD_PTR_NAME, z_free_thread_handle);
    ((z_obj *)ptr)->stale++;
    RETURN_OBJ(ptr);
  }

  RETURN_NIL;
}

DECLARE_MODULE_METHOD(thread__start) {
  ENFORCE_ARG_COUNT(start, 2);
  ENFORCE_ARG_TYPE(start, 0, IS_PTR);
  ENFORCE_ARG_TYPE(start, 1, IS_NUMBER);
  z_thread_handle *thread = AS_PTR(args[0])->pointer;

  if(thread != NULL) {
    pthread_attr_t attr;
    pthread_attr_init(&attr);
    pthread_attr_setstacksize(&attr, (size_t)AS_NUMBER(args[1]));

    if(pthread_create(&thread->thread, &attr, z_thread_callback_function, thread) == 0) {
      pthread_attr_destroy(&attr);
      RETURN_TRUE;
    }

    pthread_attr_destroy(&attr);
  }

  RETURN_FALSE;
}

DECLARE_MODULE_METHOD(thread__cancel) {
  ENFORCE_ARG_COUNT(cancel, 1);
  ENFORCE_ARG_TYPE(cancel, 0, IS_PTR);
  z_thread_handle *thread = AS_PTR(args[0])->pointer;

  if(thread != NULL && thread->vm != NULL) {
// #ifdef _WIN32
//     // On Windows, avoid signal emulation; use pthread_cancel when available
//     if (pthread_cancel(thread->thread) == 0) {
//       free_thread_handle(thread);
//       RETURN_TRUE;
//     }
// #else
    if(pthread_kill(thread->thread, SIGUSR2) == 0) {
      free_thread_handle(thread);
      RETURN_TRUE;
    }
// #endif
  }

  RETURN_FALSE;
}

DECLARE_MODULE_METHOD(thread__await) {
  ENFORCE_ARG_COUNT(await, 1);
  ENFORCE_ARG_TYPE(await, 0, IS_PTR);
  z_thread_handle *thread = AS_PTR(args[0])->pointer;

  if(thread != NULL && thread->vm != NULL) {
    RETURN_BOOL(pthread_join(thread->thread, NULL) == 0);
  }

  RETURN_TRUE;
}

DECLARE_MODULE_METHOD(thread__detach) {
  ENFORCE_ARG_COUNT(detach, 1);
  ENFORCE_ARG_TYPE(detach, 0, IS_PTR);
  z_thread_handle *thread = AS_PTR(args[0])->pointer;

  if(thread != NULL && thread->vm != NULL) {
    RETURN_BOOL(pthread_detach(thread->thread) == 0);
  }

  RETURN_FALSE;
}

DECLARE_MODULE_METHOD(thread__set_name) {
  ENFORCE_ARG_COUNT(set_name, 2);
  ENFORCE_ARG_TYPE(set_name, 0, IS_PTR);
  ENFORCE_ARG_TYPE(set_name, 1, IS_STRING);
#ifdef __APPLE__
  RETURN_BOOL(pthread_setname_np(AS_C_STRING(args[1])) == 0);
#else
  z_thread_handle *thread = AS_PTR(args[0])->pointer;
  if(thread != NULL && thread->vm != NULL) {
# if defined(PTHREAD_MAX_NAMELEN_NP) && PTHREAD_MAX_NAMELEN_NP == 16
    RETURN_BOOL(pthread_setname_np(thread->thread, AS_C_STRING(args[1]), NULL) == 0);
# else
    RETURN_BOOL(pthread_setname_np(thread->thread, AS_C_STRING(args[1])) == 0);
# endif
  }
  RETURN_FALSE;
#endif
}

DECLARE_MODULE_METHOD(thread__get_name) {
  ENFORCE_ARG_COUNT(get_name, 1);
  ENFORCE_ARG_TYPE(get_name, 0, IS_PTR);
  z_thread_handle *thread = AS_PTR(args[0])->pointer;

  if(thread != NULL && thread->vm != NULL) {
    char buffer[255];
    if(pthread_getname_np(thread->thread, buffer, 255) == 0) {
      RETURN_STRING(buffer);
    }
  }

  RETURN_VALUE(EMPTY_STRING_VAL);
}

uint64_t get_thread_id(void) {
#if defined(__linux__)
  return syscall(SYS_gettid);
#elif defined(__FreeBSD__)
  /* thread id is up to INT_MAX */
    long tid;
    thr_self(&tid);
    return (uint64_t)tid;
#elif defined(__NetBSD__)
  return (uint64_t)_lwp_self();
#elif defined(__OpenBSD__)
  return (uint64_t)getthrid();
#elif defined(__APPLE__)
  uint64_t id;
  pthread_threadid_np(NULL, &id);
  return id;
#else
  return (uint64_t)getpid();
#endif
}

DECLARE_MODULE_METHOD(thread__get_id) {
  ENFORCE_ARG_COUNT(get_id, 0);
  RETURN_NUMBER(get_thread_id());
}

DECLARE_MODULE_METHOD(thread__yield) {
  ENFORCE_ARG_COUNT(yield, 0);
  RETURN_BOOL(sched_yield() == 0);
}

DECLARE_MODULE_METHOD(thread__is_alive) {
  ENFORCE_ARG_COUNT(is_alive, 1);
  ENFORCE_ARG_TYPE(get_name, 0, IS_PTR);
  z_thread_handle *thread = AS_PTR(args[0])->pointer;
  RETURN_BOOL(thread != NULL && thread->vm != NULL);
}

// MUTEX IMPL.

DECLARE_MODULE_METHOD(thread__new_mutex) {
  ENFORCE_ARG_COUNT(new, 0);

  z_thread_mutex *m = (z_thread_mutex *)malloc(sizeof(z_thread_mutex));
  if (!m) RETURN_ERROR("out of memory");

  pthread_mutexattr_t attr;
  pthread_mutexattr_init(&attr);
  /* PTHREAD_MUTEX_ERRORCHECK gives us deadlock detection in debug builds. */
  pthread_mutexattr_settype(&attr, PTHREAD_MUTEX_ERRORCHECK);
  int rc = pthread_mutex_init(&m->mu, &attr);
  pthread_mutexattr_destroy(&attr);

  if (rc != 0) {
    free(m);
    RETURN_ERROR("MutexStatus: %d", rc);
  }

  m->locked = 0;
  RETURN_CLOSABLE_NAMED_PTR(m, Z_THREAD_MUTEX_NAME, z_thread_mutex_finalizer);
}

DECLARE_MODULE_METHOD(thread__mutex_lock) {
  ENFORCE_ARG_COUNT(lock, 1);
  z_thread_mutex *m = (z_thread_mutex *)AS_PTR(args[0])->pointer;
  if (!m) RETURN_ERROR("invalid instance");

  int rc = pthread_mutex_lock(&m->mu);
  if (rc != 0) {
    RETURN_ERROR("MutexStatus: %d", rc);
  }
  m->locked = 1;
  RETURN_NIL;
}

DECLARE_MODULE_METHOD(thread__mutex_unlock) {
  ENFORCE_ARG_COUNT(unlock, 1);
  z_thread_mutex *m = (z_thread_mutex *)AS_PTR(args[0])->pointer;
  if (!m) RETURN_ERROR("invalid instance");

  m->locked = 0;
  int rc = pthread_mutex_unlock(&m->mu);
  if (rc != 0) {
    RETURN_ERROR("MutexStatus: %d", rc);
  }
  RETURN_NIL;
}

DECLARE_MODULE_METHOD(thread__mutex_try_lock) {
  ENFORCE_ARG_COUNT(try_lock, 1);
  z_thread_mutex *m = (z_thread_mutex *)AS_PTR(args[0])->pointer;
  if (!m) RETURN_ERROR("invalid instance");

  int rc = pthread_mutex_trylock(&m->mu);
  if (rc == 0) {
    m->locked = 1;
    RETURN_BOOL(true);
  } else if (rc == EBUSY) {
    RETURN_BOOL(false);
  } else {
    RETURN_ERROR("MutexStatus: %d", rc);
  }
}

DECLARE_MODULE_METHOD(thread__mutex_is_locked) {
  ENFORCE_ARG_COUNT(is_locked, 1);
  z_thread_mutex *m = (z_thread_mutex *)AS_PTR(args[0])->pointer;
  if (!m) RETURN_ERROR("invalid instance");
  RETURN_BOOL(m->locked != 0);
}

// CHANNEL IMPL.
DECLARE_MODULE_METHOD(thread__new_channel) {
  ENFORCE_ARG_RANGE(new, 0, 1);

  int cap = 0;
  if (arg_count == 1) {
    ENFORCE_ARG_TYPE(new, 0, IS_NUMBER);
    cap = (int)AS_NUMBER(args[0]);
    if (cap < 0) RETURN_ERROR("capacity must be >= 0");
  }

  z_thread_channel *c = (z_thread_channel *)calloc(1, sizeof(z_thread_channel));
  if (!c) RETURN_ERROR("out of memory");

  if (cap > 0) {
    c->buf = (z_value *)malloc(sizeof(z_value) * cap);
    if (!c->buf) {
      free(c);
      RETURN_ERROR("out of memory");
    }
  }

  c->cap = cap;

  int rc = pthread_mutex_init(&c->mu, NULL);
  if (rc) {
    free(c->buf);
    free(c);
    RETURN_ERROR("ChannelStatus: %d", rc);
  }

  rc = pthread_cond_init(&c->not_full, NULL);
  if (rc) {
    pthread_mutex_destroy(&c->mu);
    free(c->buf);
    free(c);
    RETURN_ERROR("ChannelStatus: %d", rc);
  }

  rc = pthread_cond_init(&c->not_empty, NULL);
  if (rc) {
    pthread_cond_destroy(&c->not_full);
    pthread_mutex_destroy(&c->mu);
    free(c->buf);
    free(c);
    RETURN_ERROR("ChannelStatus: %d", rc);
  }

  RETURN_CLOSABLE_NAMED_PTR(c, Z_THREAD_CHANNEL_NAME, z_thread_channel_finalizer);
}

DECLARE_MODULE_METHOD(thread__channel_send) {
  ENFORCE_ARG_COUNT(send, 2);
  z_thread_channel *c = (z_thread_channel *)AS_PTR(args[0])->pointer;
  if (!c) RETURN_ERROR("invalid instance");

  z_value val = args[1];

  pthread_mutex_lock(&c->mu);

  if (c->closed) {
    pthread_mutex_unlock(&c->mu);
    RETURN_ERROR("Cannot send on a closed Channel");
  }

  if (c->cap == 0) {
    /* Unbuffered: park value and wait for a receiver. */
    c->sender_waiting    = 1;
    c->rendezvous_value  = val;
    c->rendezvous_taken  = 0;
    pthread_cond_signal(&c->not_empty);          /* wake a waiting receiver */

    while (!c->rendezvous_taken && !c->closed) {
      pthread_cond_wait(&c->not_full, &c->mu);   /* wait for receiver ACK   */
    }
    c->sender_waiting = 0;
  } else {
    /* Buffered: wait while full. */
    while (c->count == c->cap && !c->closed) {
      pthread_cond_wait(&c->not_full, &c->mu);
    }
    if (c->closed) {
      pthread_mutex_unlock(&c->mu);
      RETURN_ERROR("Cannot send on a closed Channel");
    }

    int tail = (c->head + c->count) % c->cap;
    c->buf[tail] = val;
    c->count++;
    pthread_cond_signal(&c->not_empty);
  }

  pthread_mutex_unlock(&c->mu);
  RETURN_NIL;
}

DECLARE_MODULE_METHOD(thread__channel_receive) {
  ENFORCE_ARG_COUNT(receive, 1);
  z_thread_channel *c = (z_thread_channel *)AS_PTR(args[0])->pointer;
  if (!c) RETURN_ERROR("invalid instance");

  z_value result = NIL_VAL;

  pthread_mutex_lock(&c->mu);

  if (c->cap == 0) {
    /* Unbuffered: wait for a sender to park a value. */
    c->receiver_waiting = 1;
    while (!c->sender_waiting && !c->closed) {
      pthread_cond_wait(&c->not_empty, &c->mu);
    }
    if (c->sender_waiting) {
      result               = c->rendezvous_value;
      c->rendezvous_taken  = 1;
      c->sender_waiting    = 0;
      pthread_cond_signal(&c->not_full);         /* wake the parked sender  */
    }
    /* else closed with no sender → return nil */
    c->receiver_waiting = 0;
  } else {
    /* Buffered: wait while empty. */
    while (c->count == 0 && !c->closed) {
      pthread_cond_wait(&c->not_empty, &c->mu);
    }
    if (c->count > 0) {
      result   = c->buf[c->head];
      c->head  = (c->head + 1) % c->cap;
      c->count--;
      pthread_cond_signal(&c->not_full);
    }
    /* else closed + empty → return nil */
  }

  pthread_mutex_unlock(&c->mu);
  RETURN_VALUE(result);
}

DECLARE_MODULE_METHOD(thread__channel_try_send) {
  ENFORCE_ARG_COUNT(try_send, 2);
  z_thread_channel *c = (z_thread_channel *)AS_PTR(args[0])->pointer;
  if (!c) RETURN_ERROR("invalid instance");

  pthread_mutex_lock(&c->mu);
  int ok = 0;
  if (!c->closed) {
    z_value val = args[1];

    if (c->cap == 0) {
      /* Unbuffered: only succeeds if a receiver is already waiting. */
      if (c->receiver_waiting) {
        c->rendezvous_value = val;
        c->rendezvous_taken = 1;
        c->sender_waiting   = 1;
        pthread_cond_signal(&c->not_full);
        ok = 1;
      }
    } else if (c->count < c->cap) {
      int tail = (c->head + c->count) % c->cap;
      c->buf[tail] = val;
      c->count++;
      pthread_cond_signal(&c->not_empty);
      ok = 1;
    }
  }

  pthread_mutex_unlock(&c->mu);
  RETURN_BOOL(ok);
}

DECLARE_MODULE_METHOD(thread__channel_try_receive) {
  ENFORCE_ARG_COUNT(try_receive, 1);
  z_thread_channel *c = (z_thread_channel *)AS_PTR(args[0])->pointer;
  if (!c) RETURN_ERROR("invalid instance");

  z_value result = NIL_VAL;

  pthread_mutex_lock(&c->mu);
  if (c->cap == 0) {
    if (c->sender_waiting) {
      result               = c->rendezvous_value;
      c->rendezvous_taken  = 1;
      c->sender_waiting    = 0;
      pthread_cond_signal(&c->not_full);
    }
  } else if (c->count > 0) {
    result   = c->buf[c->head];
    c->head  = (c->head + 1) % c->cap;
    c->count--;
    pthread_cond_signal(&c->not_full);
  }
  pthread_mutex_unlock(&c->mu);
  RETURN_VALUE(result);
}

DECLARE_MODULE_METHOD(thread__channel_close) {
  ENFORCE_ARG_COUNT(close, 1);
  z_thread_channel *c = (z_thread_channel *)AS_PTR(args[0])->pointer;
  if (!c) RETURN_ERROR("invalid instance");

  pthread_mutex_lock(&c->mu);
  c->closed = 1;
  /* Wake everyone so they see the closed flag. */
  pthread_cond_broadcast(&c->not_empty);
  pthread_cond_broadcast(&c->not_full);
  pthread_mutex_unlock(&c->mu);
  RETURN_NIL;
}

DECLARE_MODULE_METHOD(thread__channel_is_closed) {
  ENFORCE_ARG_COUNT(is_closed, 1);
  z_thread_channel *c = (z_thread_channel *)AS_PTR(args[0])->pointer;
  if (!c) RETURN_ERROR("invalid instance");
  RETURN_BOOL(c->closed != 0);
}

DECLARE_MODULE_METHOD(thread__channel_size) {
  ENFORCE_ARG_COUNT(size, 1);
  z_thread_channel *c = (z_thread_channel *)AS_PTR(args[0])->pointer;
  if (!c) RETURN_ERROR("invalid instance");
  pthread_mutex_lock(&c->mu);
  int n = c->count;
  pthread_mutex_unlock(&c->mu);
  RETURN_NUMBER(n);
}

DECLARE_MODULE_METHOD(thread__channel_capacity) {
  ENFORCE_ARG_COUNT(capacity, 1);
  z_thread_channel *c = (z_thread_channel *)AS_PTR(args[0])->pointer;
  if (!c) RETURN_ERROR("invalid instance");
  RETURN_NUMBER(c->cap);
}

// SEMAPHORE IMPL.

DECLARE_MODULE_METHOD(thread__new_semaphore) {
  ENFORCE_ARG_RANGE(new, 0, 2);
 
  int initial = 1, max = -1;
 
  if (arg_count >= 1) {
    ENFORCE_ARG_TYPE(new, 0, IS_NUMBER);
    initial = (int)AS_NUMBER(args[0]);
    if (initial < 0) RETURN_ERROR("initial count must be >= 0");
  }
  if (arg_count == 2) {
    ENFORCE_ARG_TYPE(new, 1, IS_NUMBER);
    max = (int)AS_NUMBER(args[1]);
    if (max < initial) RETURN_ERROR("max must be >= initial");
  } else {
    max = initial > 0 ? initial : 1;
  }
 
  z_thread_semaphore *s = (z_thread_semaphore *)malloc(sizeof(z_thread_semaphore));
  if (!s) RETURN_ERROR("out of memory");
 
  int rc = pthread_mutex_init(&s->mu, NULL);
  if (rc) {
    free(s);
    RETURN_ERROR("SemaphoreStatus: %d", rc);
  }
 
  rc = pthread_cond_init(&s->cond, NULL);
  if (rc) {
    pthread_mutex_destroy(&s->mu);
    free(s);
    RETURN_ERROR("SemaphoreStatus: %d", rc);
  }
 
  s->count = initial;
  s->max   = max;

  RETURN_CLOSABLE_NAMED_PTR(s, Z_THREAD_SEMAPHORE_NAME, z_thread_semaphore_finalizer);
}
 
/* .acquire() — blocks until count > 0, then decrements */
DECLARE_MODULE_METHOD(thread__semaphore_acquire) {
  ENFORCE_ARG_COUNT(acquire, 1);
  z_thread_semaphore *s = (z_thread_semaphore *)AS_PTR(args[0])->pointer;
  if (!s) RETURN_ERROR("invalid instance");
 
  pthread_mutex_lock(&s->mu);
  while (s->count == 0) {
    pthread_cond_wait(&s->cond, &s->mu);
  }
  s->count--;
  pthread_mutex_unlock(&s->mu);
  RETURN_NIL;
}
 
/* .release() — increments count and wakes one waiter */
DECLARE_MODULE_METHOD(thread__semaphore_release) {
  ENFORCE_ARG_COUNT(release, 1);
  z_thread_semaphore *s = (z_thread_semaphore *)AS_PTR(args[0])->pointer;
  if (!s) RETURN_ERROR("invalid instance");
 
  pthread_mutex_lock(&s->mu);
  if (s->count >= s->max) {
    pthread_mutex_unlock(&s->mu);
    RETURN_ERROR("count would exceed max");
  }
  s->count++;
  pthread_cond_signal(&s->cond);
  pthread_mutex_unlock(&s->mu);
  RETURN_NIL;
}
 
/* .try_acquire() → bool */
DECLARE_MODULE_METHOD(thread__semaphore_try_acquire) {
  ENFORCE_ARG_COUNT(try_acquire, 1);
  z_thread_semaphore *s = (z_thread_semaphore *)AS_PTR(args[0])->pointer;
  if (!s) RETURN_ERROR("invalid instance");
 
  pthread_mutex_lock(&s->mu);
  int ok = 0;
  if (s->count > 0) {
    s->count--;
    ok = 1;
  }
  pthread_mutex_unlock(&s->mu);
  RETURN_BOOL(ok);
}
 
/* .count() → number */
DECLARE_MODULE_METHOD(thread__semaphore_count) {
  ENFORCE_ARG_COUNT(count, 1);
  z_thread_semaphore *s = (z_thread_semaphore *)AS_PTR(args[0])->pointer;
  if (!s) RETURN_ERROR("invalid instance");
  pthread_mutex_lock(&s->mu);
  int n = s->count;
  pthread_mutex_unlock(&s->mu);
  RETURN_NUMBER(n);
}
 
/* .max() → number */
DECLARE_MODULE_METHOD(thread__semaphore_max) {
  ENFORCE_ARG_COUNT(max, 1);
  z_thread_semaphore *s = (z_thread_semaphore *)AS_PTR(args[0])->pointer;
  if (!s) RETURN_ERROR("invalid instance");
  RETURN_NUMBER(s->max);
}


void z_thread_SIGUSR2_signal_handler(int signum) {
  pthread_exit(NULL);
}

void z_thread_init_function(z_vm *vm) {
#ifndef _WIN32
  struct sigaction sa;
  sa.sa_handler = z_thread_SIGUSR2_signal_handler;
  sigemptyset(&sa.sa_mask);
  sa.sa_flags = 0;
  // Install handler for SIGUSR2 on POSIX
  sigaction(SIGUSR2, &sa, NULL);
#else
  (void)vm; // no-op on Windows
#endif
}

void z_thread_unload_function(z_vm *vm) {
  // Unblock SIGUSR2
  sigset_t mask;
  sigemptyset(&mask);
  sigaddset(&mask, SIGUSR2);

#ifndef _WIN32
  pthread_sigmask(SIG_UNBLOCK, &mask, NULL);
#else
  pthread_sigmask2(SIG_UNBLOCK, &mask, NULL);
#endif
}

CREATE_MODULE_LOADER(thread) {
  static z_func_reg module_functions[] = {
      {"new", false, GET_MODULE_METHOD(thread__new)},
      {"start", false, GET_MODULE_METHOD(thread__start)},
      {"cancel", false, GET_MODULE_METHOD(thread__cancel)},
      {"await", false, GET_MODULE_METHOD(thread__await)},
      {"detach", false, GET_MODULE_METHOD(thread__detach)},
      {"yield", false, GET_MODULE_METHOD(thread__yield)},
      {"set_name", false, GET_MODULE_METHOD(thread__set_name)},
      {"get_name", false, GET_MODULE_METHOD(thread__get_name)},
      {"get_id", false, GET_MODULE_METHOD(thread__get_id)},
      {"is_alive", false, GET_MODULE_METHOD(thread__is_alive)},

    // Mutex
    {"new_mutex", false, GET_MODULE_METHOD(thread__new_mutex)},
    {"mutex_lock", false, GET_MODULE_METHOD(thread__mutex_lock)},
    {"mutex_unlock", false, GET_MODULE_METHOD(thread__mutex_unlock)},
    {"mutex_try_lock", false, GET_MODULE_METHOD(thread__mutex_try_lock)},
    {"mutex_is_locked", false, GET_MODULE_METHOD(thread__mutex_is_locked)},

    // Channels
    {"new_channel",     false, GET_MODULE_METHOD(thread__new_channel)},
    {"channel_send",     false, GET_MODULE_METHOD(thread__channel_send)},
    {"channel_try_send",     false, GET_MODULE_METHOD(thread__channel_try_send)},
    {"channel_receive",     false, GET_MODULE_METHOD(thread__channel_receive)},
    {"channel_try_receive",     false, GET_MODULE_METHOD(thread__channel_try_receive)},
    {"channel_close",     false, GET_MODULE_METHOD(thread__channel_close)},
    {"channel_is_closed",     false, GET_MODULE_METHOD(thread__channel_is_closed)},
    {"channel_size",     false, GET_MODULE_METHOD(thread__channel_size)},
    {"channel_capacity",     false, GET_MODULE_METHOD(thread__channel_capacity)},

    // Semaphore
    {"new_semaphore", false, GET_MODULE_METHOD(thread__new_semaphore)},
    {"semaphore_acquire", false, GET_MODULE_METHOD(thread__semaphore_acquire)},
    {"semaphore_try_acquire", false, GET_MODULE_METHOD(thread__semaphore_try_acquire)},
    {"semaphore_release", false, GET_MODULE_METHOD(thread__semaphore_release)},
    {"semaphore_count", false, GET_MODULE_METHOD(thread__semaphore_count)},
    {"semaphore_max", false, GET_MODULE_METHOD(thread__semaphore_max)},

      {NULL,     false, NULL},
  };

  static z_module_reg module = {
      .name = "_thread",
      .fields = NULL,
      .functions = module_functions,
      .classes = NULL,
      .preloader = z_thread_init_function,
      .unloader = z_thread_unload_function
  };

  return &module;
}

#undef Z_THREAD_SEMAPHORE_NAME
#undef Z_THREAD_CHANNEL_NAME
#undef Z_THREAD_MUTEX_NAME

#undef Z_THREAD_PTR_NAME
