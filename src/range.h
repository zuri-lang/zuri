#ifndef ZURI_RANGE_H
#define ZURI_RANGE_H

#include "common.h"
#include "native.h"
#include "vm.h"

#define DECLARE_RANGE_METHOD(name) DECLARE_METHOD(range##name)

DECLARE_RANGE_METHOD(lower);
DECLARE_RANGE_METHOD(upper);
DECLARE_RANGE_METHOD(__iter__);
DECLARE_RANGE_METHOD(__itern__);
DECLARE_RANGE_METHOD(step);
DECLARE_RANGE_METHOD(get_step);
DECLARE_RANGE_METHOD(range);
DECLARE_RANGE_METHOD(within);

#endif