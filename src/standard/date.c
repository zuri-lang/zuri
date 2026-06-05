#include "module.h"

#ifdef _WIN32
#define localtime_r(o, e) _localtime32_s(e, o)
#define gmtime_r(o, e) _gmtime32_s(e, o)
#endif

#ifdef HAVE_SYS_TIME_H
#include <sys/time.h>
#endif /* ifdef HAVE_SYS_TIME_H */
#include <time.h>

#ifndef HAVE_GETTIMEOFDAY
#include <gettimeofday.h>
#endif /* ifndef HAVE_GETTIMEOFDAY */

#define ADD_TIME(n, l, v)                                                      \
  dict_add_entry(vm, dict, GC_L_STRING(n, l), NUMBER_VAL(v))

#define ADD_B_TIME(n, l, v)                                                     \
  dict_add_entry(vm, dict, GC_L_STRING(n, l), BOOL_VAL(v))

#define ADD_S_TIME(n, l, v, g)                                                  \
  dict_add_entry(vm, dict, GC_L_STRING(n, l), GC_L_STRING(v, g))

#define ADD_G_TIME(n, l, v)                                                  \
  dict_add_entry(vm, dict, GC_L_STRING(n, l), GC_L_STRING(v, (int)strlen(v)))

DECLARE_MODULE_METHOD(date____mktime) {
  ENFORCE_ARG_RANGE(mktime, 1, 8);

  if (arg_count < 7) {
    for (int i = 0; i < arg_count; i++) {
      ENFORCE_ARG_TYPE(mktime, i, IS_NUMBER);
    }
  } else {
    for (int i = 0; i < 6; i++) {
      ENFORCE_ARG_TYPE(mktime, i, IS_NUMBER);
    }
    ENFORCE_ARG_TYPE(mktime, 6, IS_BOOL);
  }

  int year = -1900, month = 1, day = 1, hour = 0, minute = 0, seconds = 0,
      is_dst = 0;
  year += AS_NUMBER(args[0]);

  if (arg_count > 1)
    month = AS_NUMBER(args[1]);
  if (arg_count > 2)
    day = AS_NUMBER(args[2]);
  if (arg_count > 3)
    hour = AS_NUMBER(args[3]);
  if (arg_count > 4)
    minute = AS_NUMBER(args[4]);
  if (arg_count > 5)
    seconds = AS_NUMBER(args[5]);
  if (arg_count > 6)
    is_dst = AS_BOOL(args[6]) ? 1 : 0;

  // Basic range validation to prevent undefined behavior
  if (month < 1 || month > 12 || day < 1 || day > 31 ||
      hour < 0 || hour > 23 || minute < 0 || minute > 59 ||
      seconds < 0 || seconds > 60) {
    RETURN_VALUE_ERROR("invalid date component");
  }

  struct tm t = {0};
  t.tm_year = year;
  t.tm_mon = month - 1;
  t.tm_mday = day;
  t.tm_hour = hour;
  t.tm_min = minute;
  t.tm_sec = seconds;
  t.tm_isdst = is_dst;

  RETURN_NUMBER((long) mktime(&t));
}

DECLARE_MODULE_METHOD(date__localtime) {
  struct timeval raw_time;
  gettimeofday(&raw_time, NULL);
  struct tm now;
  localtime_r(&raw_time.tv_sec, &now);

  z_obj_dict *dict = (z_obj_dict *) GC(new_dict(vm));

  ADD_TIME("year", 4, (double) now.tm_year + 1900);
  ADD_TIME("month", 5, (double) now.tm_mon + 1);
  ADD_TIME("day", 3, now.tm_mday);
  ADD_TIME("week_day", 8, now.tm_wday);
  ADD_TIME("year_day", 8, now.tm_yday);
  ADD_TIME("hour", 4, now.tm_hour);
  ADD_TIME("minute", 6, now.tm_min);
  if (now.tm_sec <= 59) {
    ADD_TIME("seconds", 7, now.tm_sec);
  } else {
    ADD_TIME("seconds", 7, 59);
  }
  ADD_TIME("microseconds", 12, (double) raw_time.tv_usec);

  ADD_B_TIME("is_dst", 6, now.tm_isdst == 1 ? true : false);

#ifndef _WIN32
  // set time zone
  ADD_G_TIME("zone", 4, now.tm_zone);
  // setting gmt offset
  ADD_TIME("gmt_offset", 10, now.tm_gmtoff);
#else
  // set time zone
  ADD_S_TIME("zone", 4, "", 0);
  // setting gmt offset
  ADD_TIME("gmt_offset", 10, 0);
#endif

  RETURN_OBJ(dict);
}

DECLARE_MODULE_METHOD(date__gmtime) {
  struct timeval raw_time;
  gettimeofday(&raw_time, NULL);
  struct tm now;
  gmtime_r(&raw_time.tv_sec, &now);

  z_obj_dict *dict = (z_obj_dict *) GC(new_dict(vm));

  ADD_TIME("year", 4, (double) now.tm_year + 1900);
  ADD_TIME("month", 5, (double) now.tm_mon + 1);
  ADD_TIME("day", 3, now.tm_mday);
  ADD_TIME("week_day", 8, now.tm_wday);
  ADD_TIME("year_day", 8, now.tm_yday);
  ADD_TIME("hour", 4, now.tm_hour);
  ADD_TIME("minute", 6, now.tm_min);
  if (now.tm_sec <= 59) {
    ADD_TIME("seconds", 7, now.tm_sec);
  } else {
    ADD_TIME("seconds", 7, 59);
  }
  ADD_TIME("microseconds", 12, (double) raw_time.tv_usec);

  ADD_B_TIME("is_dst", 6, now.tm_isdst == 1 ? true : false);

#ifndef _WIN32
  // set time zone
  ADD_G_TIME("zone", 4, now.tm_zone);
  // setting gmt offset
  ADD_TIME("gmt_offset", 10, now.tm_gmtoff);
#else
  // set time zone
  ADD_S_TIME("zone", 4, "", 0);
  // setting gmt offset
  ADD_TIME("gmt_offset", 10, 0);
#endif

  RETURN_OBJ(dict);
}

CREATE_MODULE_LOADER(date) {
  static z_func_reg module_functions[] = {
      {"localtime", true,  GET_MODULE_METHOD(date__localtime)},
      {"gmtime",    true,  GET_MODULE_METHOD(date__gmtime)},
      {"mktime",    false, GET_MODULE_METHOD(date____mktime)},
      {NULL,        false, NULL},
  };

  static z_module_reg module = {
      .name = "_date",
      .fields = NULL,
      .functions = module_functions,
      .classes = NULL,
      .preloader = NULL,
      .unloader = NULL
  };
  return &module;
}

#undef ADD_TIME
#undef ADD_B_TIME
#undef ADD_S_TIME