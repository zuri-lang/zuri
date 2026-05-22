#include "module.h"

#include <math.h>

DECLARE_MODULE_METHOD(math__sin) {
  ENFORCE_ARG_COUNT(sin, 1);
  ENFORCE_ARG_TYPE(sin, 0, IS_NUMBER);
  RETURN_NUMBER(sin(AS_NUMBER(args[0])));
}

DECLARE_MODULE_METHOD(math__cos) {
  ENFORCE_ARG_COUNT(cos, 1);
  ENFORCE_ARG_TYPE(cos, 0, IS_NUMBER);
  RETURN_NUMBER(cos(AS_NUMBER(args[0])));
}

DECLARE_MODULE_METHOD(math__tan) {
  ENFORCE_ARG_COUNT(tan, 1);
  ENFORCE_ARG_TYPE(tan, 0, IS_NUMBER);
  RETURN_NUMBER(tan(AS_NUMBER(args[0])));
}

DECLARE_MODULE_METHOD(math__sinh) {
  ENFORCE_ARG_COUNT(sinh, 1);
  ENFORCE_ARG_TYPE(sinh, 0, IS_NUMBER);
  RETURN_NUMBER(sinh(AS_NUMBER(args[0])));
}

DECLARE_MODULE_METHOD(math__cosh) {
  ENFORCE_ARG_COUNT(cosh, 1);
  ENFORCE_ARG_TYPE(cosh, 0, IS_NUMBER);
  RETURN_NUMBER(cosh(AS_NUMBER(args[0])));
}

DECLARE_MODULE_METHOD(math__tanh) {
  ENFORCE_ARG_COUNT(tanh, 1);
  ENFORCE_ARG_TYPE(tanh, 0, IS_NUMBER);
  RETURN_NUMBER(tanh(AS_NUMBER(args[0])));
}

DECLARE_MODULE_METHOD(math__asin) {
  ENFORCE_ARG_COUNT(asin, 1);
  ENFORCE_ARG_TYPE(asin, 0, IS_NUMBER);
  RETURN_NUMBER(asin(AS_NUMBER(args[0])));
}

DECLARE_MODULE_METHOD(math__acos) {
  ENFORCE_ARG_COUNT(acos, 1);
  ENFORCE_ARG_TYPE(acos, 0, IS_NUMBER);
  RETURN_NUMBER(acos(AS_NUMBER(args[0])));
}

DECLARE_MODULE_METHOD(math__atan) {
  ENFORCE_ARG_COUNT(atan, 1);
  ENFORCE_ARG_TYPE(atan, 0, IS_NUMBER);
  RETURN_NUMBER(atan(AS_NUMBER(args[0])));
}

DECLARE_MODULE_METHOD(math__atan2) {
  ENFORCE_ARG_COUNT(atan2, 2);
  ENFORCE_ARG_TYPE(atan2, 0, IS_NUMBER);
  ENFORCE_ARG_TYPE(atan2, 1, IS_NUMBER);
  RETURN_NUMBER(atan2(AS_NUMBER(args[0]), AS_NUMBER(args[1])));
}

DECLARE_MODULE_METHOD(math__asinh) {
  ENFORCE_ARG_COUNT(asinh, 1);
  ENFORCE_ARG_TYPE(asinh, 0, IS_NUMBER);
  RETURN_NUMBER(asinh(AS_NUMBER(args[0])));
}

DECLARE_MODULE_METHOD(math__acosh) {
  ENFORCE_ARG_COUNT(acosh, 1);
  ENFORCE_ARG_TYPE(acosh, 0, IS_NUMBER);
  RETURN_NUMBER(acosh(AS_NUMBER(args[0])));
}

DECLARE_MODULE_METHOD(math__atanh) {
  ENFORCE_ARG_COUNT(atanh, 1);
  ENFORCE_ARG_TYPE(atanh, 0, IS_NUMBER);
  RETURN_NUMBER(atanh(AS_NUMBER(args[0])));
}

DECLARE_MODULE_METHOD(math__exp) {
  ENFORCE_ARG_COUNT(exp, 1);
  ENFORCE_ARG_TYPE(exp, 0, IS_NUMBER);
  RETURN_NUMBER(exp(AS_NUMBER(args[0])));
}

DECLARE_MODULE_METHOD(math__expm1) {
  ENFORCE_ARG_COUNT(expm1, 1);
  ENFORCE_ARG_TYPE(expm1, 0, IS_NUMBER);
  RETURN_NUMBER(expm1(AS_NUMBER(args[0])));
}

DECLARE_MODULE_METHOD(math__ceil) {
  ENFORCE_ARG_COUNT(ceil, 1);
  ENFORCE_ARG_TYPE(ceil, 0, IS_NUMBER);
  RETURN_NUMBER(ceil(AS_NUMBER(args[0])));
}

DECLARE_MODULE_METHOD(math__round) {
  ENFORCE_ARG_COUNT(round, 1);
  ENFORCE_ARG_TYPE(round, 0, IS_NUMBER);
  RETURN_NUMBER(round(AS_NUMBER(args[0])));
}

DECLARE_MODULE_METHOD(math__log) {
  ENFORCE_ARG_COUNT(log, 1);
  ENFORCE_ARG_TYPE(log, 0, IS_NUMBER);
  RETURN_NUMBER(log(AS_NUMBER(args[0])));
}

DECLARE_MODULE_METHOD(math__log10) {
  ENFORCE_ARG_COUNT(log10, 1);
  ENFORCE_ARG_TYPE(log10, 0, IS_NUMBER);
  RETURN_NUMBER(log10(AS_NUMBER(args[0])));
}

DECLARE_MODULE_METHOD(math__log2) {
  ENFORCE_ARG_COUNT(log2, 1);
  ENFORCE_ARG_TYPE(log2, 0, IS_NUMBER);
  RETURN_NUMBER(log2(AS_NUMBER(args[0])));
}

DECLARE_MODULE_METHOD(math__log1p) {
  ENFORCE_ARG_COUNT(log1p, 1);
  ENFORCE_ARG_TYPE(log1p, 0, IS_NUMBER);
  RETURN_NUMBER(log1p(AS_NUMBER(args[0])));
}

DECLARE_MODULE_METHOD(math__floor) {
  ENFORCE_ARG_COUNT(floor, 1);
  if (IS_NIL(args[0])) {
    RETURN_NUMBER(0);
  }
  ENFORCE_ARG_TYPE(floor, 0, IS_NUMBER);
  RETURN_NUMBER(floor(AS_NUMBER(args[0])));
}

CREATE_MODULE_LOADER(math) {
  static z_func_reg module_functions[] = {
      {"sin",   true,  GET_MODULE_METHOD(math__sin)},
      {"cos",   true,  GET_MODULE_METHOD(math__cos)},
      {"tan",   true,  GET_MODULE_METHOD(math__tan)},
      {"sinh",  true,  GET_MODULE_METHOD(math__sinh)},
      {"cosh",  true,  GET_MODULE_METHOD(math__cosh)},
      {"tanh",  true,  GET_MODULE_METHOD(math__tanh)},
      {"asin",  true,  GET_MODULE_METHOD(math__asin)},
      {"acos",  true,  GET_MODULE_METHOD(math__acos)},
      {"atan",  true,  GET_MODULE_METHOD(math__atan)},
      {"atan2", true,  GET_MODULE_METHOD(math__atan2)},
      {"asinh", true,  GET_MODULE_METHOD(math__asinh)},
      {"acosh", true,  GET_MODULE_METHOD(math__acosh)},
      {"atanh", true,  GET_MODULE_METHOD(math__atanh)},
      {"exp",   true,  GET_MODULE_METHOD(math__exp)},
      {"expm1", true,  GET_MODULE_METHOD(math__expm1)},
      {"ceil",  true,  GET_MODULE_METHOD(math__ceil)},
      {"round", true,  GET_MODULE_METHOD(math__round)},
      {"log",   true,  GET_MODULE_METHOD(math__log)},
      {"log2",  true,  GET_MODULE_METHOD(math__log2)},
      {"log10", true,  GET_MODULE_METHOD(math__log10)},
      {"log1p", true,  GET_MODULE_METHOD(math__log1p)},
      {"floor", true,  GET_MODULE_METHOD(math__floor)},
      {NULL,    false, NULL},
  };

  static z_module_reg module = {
      .name = "_math",
      .fields = NULL,
      .functions = module_functions,
      .classes = NULL,
      .preloader = NULL,
      .unloader = NULL
  };

  return &module;
}