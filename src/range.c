#include "range.h"

DECLARE_RANGE_METHOD(lower) {
  ENFORCE_ARG_COUNT(lower, 0);
  RETURN_NUMBER(AS_RANGE(METHOD_OBJECT)->lower);
}

DECLARE_RANGE_METHOD(upper) {
  ENFORCE_ARG_COUNT(upper, 0);
  RETURN_NUMBER(AS_RANGE(METHOD_OBJECT)->upper);
}

DECLARE_RANGE_METHOD(range) {
  ENFORCE_ARG_COUNT(range, 0);
  RETURN_NUMBER(AS_RANGE(METHOD_OBJECT)->range);
}

DECLARE_RANGE_METHOD(within) {
  ENFORCE_ARG_COUNT(within, 1);
  ENFORCE_ARG_TYPE(within, 0, IS_NUMBER);

  const z_obj_range *range = AS_RANGE(METHOD_OBJECT);
  const double number = AS_NUMBER(args[0]);
  if (range->lower > range->upper) {
    RETURN_BOOL(number <= range->lower && number >= range->upper);
  }
  RETURN_BOOL(number >= range->lower && number <= range->upper);
}

DECLARE_RANGE_METHOD(step) {
  ENFORCE_ARG_COUNT(step, 1);
  ENFORCE_ARG_TYPE(step, 0, IS_NUMBER);

  z_obj_range *range = AS_RANGE(METHOD_OBJECT);
  const int number = (int)AS_NUMBER(args[0]);
  range->step = number;
  RETURN_VALUE(METHOD_OBJECT);
}

DECLARE_RANGE_METHOD(get_step) {
  ENFORCE_ARG_COUNT(get_step, 0);
  RETURN_NUMBER(AS_RANGE(METHOD_OBJECT)->step);
}

DECLARE_RANGE_METHOD(__iter__) {
  ENFORCE_ARG_COUNT(__iter__, 1);
  ENFORCE_ARG_TYPE(__iter__, 0, IS_NUMBER);

  z_obj_range *range = AS_RANGE(METHOD_OBJECT);

  int index = AS_NUMBER(args[0]);

  if (index >= 0 && index < range->range) {
    if(index == 0) RETURN_NUMBER(range->lower);
    RETURN_NUMBER(range->lower > range->upper ? range->lower - index : range->lower + index);
  }

  RETURN_NIL;
}

DECLARE_RANGE_METHOD(__itern__) {
  ENFORCE_ARG_COUNT(__itern__, 1);
  z_obj_range *range = AS_RANGE(METHOD_OBJECT);

  if (IS_NIL(args[0])) {
    if (range->range == 0) {
      RETURN_NIL;
    }
    RETURN_NUMBER(0);
  }

  if (!IS_NUMBER(args[0])) {
    RETURN_ARGUMENT_ERROR("ranges are numerically indexed");
  }

  int index = (int)AS_NUMBER(args[0]) + range->step;
  if (index < range->range) {
    RETURN_NUMBER(index);
  }

  RETURN_NIL;
}
