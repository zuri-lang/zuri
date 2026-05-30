#ifdef _MSC_VER
#pragma warning (disable : 4113)
#pragma warning (disable : 4047)
#pragma warning (disable : 5105)
#endif

#include "module.h"
#ifdef HAVE_UNISTD_H
#include <unistd.h>
#else
#include "zunistd.h"
#endif /* HAVE_UNISTD_H */
#include "utf8.h"

#include <errno.h>
#include <stdio.h>

#ifdef IS_UNIX
#include <sys/ioctl.h>
#endif

#include "capture.h"

#ifdef HAVE_TERMIOS_H
#include <termios.h>
#include <stdlib.h>

static struct termios orig_termios;
static bool set_attr_was_called = false;

void disable_raw_mode(void) {
  tcsetattr(STDIN_FILENO, TCSAFLUSH, &orig_termios);
}

static struct termios n_term;
static struct termios o_term;

static int cbreak(int fd) {
  if((tcgetattr(fd, &o_term)) == -1)
    return -1;
  n_term = o_term;
  n_term.c_lflag = n_term.c_lflag & ~(ECHO|ICANON);
  n_term.c_cc[VMIN] = 1;
  n_term.c_cc[VTIME]= 0;
  if((tcsetattr(fd, TCSAFLUSH, &n_term)) == -1)
    return -1;
  return 1;
}

int getch() {
  int cinput;

  if(cbreak(STDIN_FILENO) == -1) {
    fprintf(stderr, "cbreak failure, exiting \n");
    exit(EXIT_FAILURE);
  }
  cinput = getchar();
  tcsetattr(STDIN_FILENO, TCSANOW, &o_term);

  return cinput;
}
#elif _MSC_VER  || __WIN32__ || __MS_DOS__
#include <windows.h>
#include <conio.h>  // for getch and cbreak support
#endif /* HAVE_TERMIOS_H */

static int read_line(char line[], int max) {
  int nch = 0;
  int c;
  max = max - 1; // leave room for '\0'

  while ((c = getchar()) != EOF && c != '\0' && c != '\n') {
    if (nch < max) {
      char *cc = utf8_encode(c);
      line[nch] = *cc;
      nch = nch + 1;
      free(cc);
    } else {
      break;
    }
  }

  if (c == EOF && nch == 0)
    return EOF;

  line[nch] = '\0';

  return nch;
}

/**
 * tty.tcgetattr()
 *
 * returns the configuration of the current tty input
 */
DECLARE_MODULE_METHOD(io_tty__tcgetattr) {
  ENFORCE_ARG_COUNT(tcgetattr, 2);
  ENFORCE_ARG_TYPE(tcsetattr, 0, IS_FILE);
  ENFORCE_ARG_TYPE(tcsetattr, 1, IS_BOOL);

#ifdef HAVE_TERMIOS_H
  z_obj_file *file = AS_FILE(args[0]);
  bool original = AS_BOOL(args[1]);

  if (!file->is_std) {
    RETURN_VALUE_ERROR("can only use tty on std objects");
  }

  struct termios raw_attr;
  if(!original) {
    if (tcgetattr(fileno(file->file), &raw_attr) != 0) {
      RETURN_ERROR(strerror(errno));
    }
  } else {
    raw_attr = orig_termios;
  }

  // we have our attributes already
  z_obj_dict *dict = (z_obj_dict *)GC(new_dict(vm));
  dict_add_entry(vm, dict, NUMBER_VAL(0), NUMBER_VAL(raw_attr.c_iflag));
  dict_add_entry(vm, dict, NUMBER_VAL(1), NUMBER_VAL(raw_attr.c_oflag));
  dict_add_entry(vm, dict, NUMBER_VAL(2), NUMBER_VAL(raw_attr.c_cflag));
  dict_add_entry(vm, dict, NUMBER_VAL(3), NUMBER_VAL(raw_attr.c_lflag));

#if !defined(__MUSL__)
  dict_add_entry(vm, dict, NUMBER_VAL(4), NUMBER_VAL(raw_attr.c_ispeed));
  dict_add_entry(vm, dict, NUMBER_VAL(5), NUMBER_VAL(raw_attr.c_ospeed));
#else
  dict_add_entry(vm, dict, NUMBER_VAL(4), NUMBER_VAL(raw_attr.__c_ispeed));
  dict_add_entry(vm, dict, NUMBER_VAL(5), NUMBER_VAL(raw_attr.__c_ospeed));
#endif

  RETURN_OBJ(dict);
#else
  RETURN_ERROR("tcgetattr() is not supported on this platform");
#endif /* HAVE_TERMIOS_H */
}

/**
 * tty.tcsetattr(attrs: dict)
 *
 * sets the attributes of a tty
 * @return true if succeed or false otherwise
 * TODO: support the c_cc flag
 */
DECLARE_MODULE_METHOD(io_tty__tcsetattr) {
  ENFORCE_ARG_COUNT(tcsetattr, 3);
  ENFORCE_ARG_TYPE(tcsetattr, 0, IS_FILE);
  ENFORCE_ARG_TYPE(tcsetattr, 1, IS_NUMBER);
  ENFORCE_ARG_TYPE(tcsetattr, 2, IS_DICT);

#ifdef HAVE_TERMIOS_H
  z_obj_file *file = AS_FILE(args[0]);
  int type = AS_NUMBER(args[1]);
  z_obj_dict *dict = AS_DICT(args[2]);

  if (!file->is_std) {
    RETURN_VALUE_ERROR("can only use tty on std objects");
  }

  if (type < 0) {
    RETURN_TYPE_ERROR("tty options should be one of TTY's TCSAs");
  }

  struct termios raw = orig_termios;

  // make sure we have good values so that we don't freeze the tty
  for (int i = 0; i < dict->names.count; i++) {
    if (!IS_NUMBER(dict->names.values[i]) ||
        AS_NUMBER(dict->names.values[i]) < 0 || // c_iflag
        AS_NUMBER(dict->names.values[i]) > 5) { // ospeed
          RETURN_TYPE_ERROR("attributes must be one of io TTY flags");
    }
    z_value value;
    if (dict_get_entry(dict, dict->names.values[i], &value)) {
      if (!IS_NUMBER(value)) {
        RETURN_TYPE_ERROR("TTY attribute cannot be %s", value_type(value));
      }

      switch (i) {
        case 0: raw.c_iflag = (unsigned long) AS_NUMBER(value); break;
        case 1: raw.c_oflag = (unsigned long) AS_NUMBER(value); break;
        case 2: raw.c_cflag = (unsigned long) AS_NUMBER(value); break;
        case 3: raw.c_lflag = (unsigned long) AS_NUMBER(value); break;
#if !defined(__MUSL__)
        case 4: raw.c_ispeed = (unsigned long) AS_NUMBER(value); break;
        case 5: raw.c_ospeed = (unsigned long) AS_NUMBER(value); break;
#else
        case 4: raw.__c_ispeed = (unsigned long) AS_NUMBER(value); break;
        case 5: raw.__c_ospeed = (unsigned long) AS_NUMBER(value); break;
#endif
        default:;
      }
    }
  }

  set_attr_was_called = true;

  int result = tcsetattr(fileno(file->file), type, &raw);
  RETURN_BOOL(result != -1);
#else
  RETURN_ERROR("tcsetattr() is not supported on this platform");
#endif /* HAVE_TERMIOS_H */
}

/**
 * TTY.flush()
 * flushes the standard output and standard error interface
 * @return nil
 */
DECLARE_MODULE_METHOD(io_tty__flush) {
  ENFORCE_ARG_COUNT(TTY.flush, 0);
  fflush(stdout);
  fflush(stderr);
  RETURN;
}

DECLARE_MODULE_METHOD(io_tty__getsize) {
  ENFORCE_ARG_COUNT(get_size, 1);
  z_obj_file *file = AS_FILE(args[0]);

  int columns = 0, rows = 0;
#ifdef _WIN32
  CONSOLE_SCREEN_BUFFER_INFO csbi;
  if(GetConsoleScreenBufferInfo(GetStdHandle(file->number), &csbi)) {
    columns = csbi.srWindow.Right - csbi.srWindow.Left + 1;
    rows = csbi.srWindow.Bottom - csbi.srWindow.Top + 1;
  }
#elif defined(IS_UNIX)
  struct winsize w;
  if((ioctl(file->number, TIOCGWINSZ, &w)) >= 0) {
    columns = w.ws_col;
    rows = w.ws_row;
  }
#endif

  z_obj_list *list = (z_obj_list *)GC(new_list(vm));
  write_list(vm, list, NUMBER_VAL(columns));
  write_list(vm, list, NUMBER_VAL(rows));
  RETURN_OBJ(list);
}

/**
 * getch()
 *
 * reads character(s) from standard input without printing to standard output
 *
 * when length is given, gets `length` number of characters
 * else, gets a single character
 * @return char
 */
DECLARE_MODULE_METHOD(io_getch) {
  ENFORCE_ARG_COUNT(getch, 0);
  char *result = ALLOCATE(char, 2);
  result[0] = (char)getch();
  result[1] = '\0';
  RETURN_L_STRING(result, 1);
}

DECLARE_MODULE_METHOD(io_startcapture) {
  ENFORCE_ARG_COUNT(start_capture, 1);
  ENFORCE_ARG_TYPE(start_capture, 0, IS_NUMBER);
  z_capture_t *capture = capture_start(AS_NUMBER(args[0]) == 0 ? Z_CAPTURE_STDOUT : Z_CAPTURE_STDERR);
  RETURN_NAMED_PTR(capture, "CaptureStream");
}

DECLARE_MODULE_METHOD(io_stopcapture) {
  ENFORCE_ARG_COUNT(stop_capture, 1);
  ENFORCE_ARG_TYPE(stop_capture, 0, IS_PTR);
  z_capture_t *capture = (z_capture_t *)AS_PTR(args[0])->pointer;
  if (capture == NULL) {
    RETURN_ERROR("Invalid capture stream");
  }

  char *output = capture_stop(capture);
  RETURN_TT_STRING(output);
}

/**
 * stdin()
 *
 * returns the standard input
 */
z_value io_module_stdin(z_vm *vm) {
  z_obj_file *file =
      new_file(vm, copy_string(vm, "<stdin>", 7), copy_string(vm, "r", 1));
  file->file = stdin;
  file->is_open = true;
  file->is_std = true;
  file->number = STDIN_FILENO;
  file->is_tty = isatty(STDIN_FILENO);
  return OBJ_VAL(file);
}

/**
 * stdout()
 *
 * returns the standard output interface
 */
z_value io_module_stdout(z_vm *vm) {
  z_obj_file *file =
      new_file(vm, copy_string(vm, "<stdout>", 8), copy_string(vm, "wb", 2));
  file->file = stdout;
  file->is_open = true;
  file->is_std = true;
  file->number = STDOUT_FILENO;
  file->is_tty = isatty(STDOUT_FILENO);
  return OBJ_VAL(file);
}

z_value io_module_is_repl(z_vm *vm) {
  return BOOL_VAL(vm->is_repl);
}

/**
 * stderr()
 *
 * returns the standard error interface
 */
z_value io_module_stderr(z_vm *vm) {
  z_obj_file *file =
      new_file(vm, copy_string(vm, "<stderr>", 8), copy_string(vm, "wb", 2));
  file->file = stderr;
  file->is_open = true;
  file->is_std = true;
  file->number = STDERR_FILENO;
  file->is_tty = isatty(STDERR_FILENO);
  return OBJ_VAL(file);
}

void __io_module_unload(z_vm *vm) {
#ifdef HAVE_TERMIOS_H
  if (set_attr_was_called) {
    disable_raw_mode();
  }
#endif /* ifdef HAVE_TERMIOS_H */
}

void __io_module_preload(z_vm *vm) {
#ifdef HAVE_TERMIOS_H
  tcgetattr(STDIN_FILENO, &orig_termios);
  atexit(disable_raw_mode);
#endif /* ifdef HAVE_TERMIOS_H */
}

CREATE_MODULE_LOADER(io) {
  static z_field_reg io_module_fields[] = {
      {"stdin",  false, io_module_stdin},
      {"stdout", false, io_module_stdout},
      {"stderr", false, io_module_stderr},
      {"isrepl", false, io_module_is_repl},
      {NULL,     false, NULL},
  };

  static z_func_reg io_functions[] = {
      {"getch",  false, GET_MODULE_METHOD(io_getch)},
      {"startcapture", false, GET_MODULE_METHOD(io_startcapture)},
      {"stopcapture", false, GET_MODULE_METHOD(io_stopcapture)},
      {NULL,    false, NULL},
  };

  static z_func_reg tty_class_functions[] = {
      {"tcgetattr", false, GET_MODULE_METHOD(io_tty__tcgetattr)},
      {"tcsetattr", false, GET_MODULE_METHOD(io_tty__tcsetattr)},
      {"getsize",     false, GET_MODULE_METHOD(io_tty__getsize)},
      {"flush",     false, GET_MODULE_METHOD(io_tty__flush)},
      {NULL,        false, NULL},
  };

  static z_class_reg classes[] = {
      {"TTY", NULL, tty_class_functions},
      {NULL,  NULL, NULL},
  };

  static z_module_reg module = {
      .name = "_io",
      .fields = io_module_fields,
      .functions = io_functions,
      .classes = classes,
      .preloader = &__io_module_preload,
      .unloader = &__io_module_unload
  };

  return &module;
}