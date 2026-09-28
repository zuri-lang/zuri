/*
 * The C library the ffi suites call into. The suites hand this header
 * to ffi.declare() exactly as it is, so it doubles as a test of reading
 * real header text.
 */
#ifndef FFI_FIXTURE_H
#define FFI_FIXTURE_H

#include <stddef.h>
#include <stdint.h>
#include <stdbool.h>
#include <wchar.h>
/* The macOS C library ships no <uchar.h>, so the compiler there gets
   char16_t as C11 defines it. Zuri's reader knows the header either way. */
#if defined(__APPLE__) && !defined(__ZURI_FFI__)
typedef uint_least16_t char16_t;
#else
#include <uchar.h>
#endif

#ifdef __cplusplus
extern "C" {
#endif

#define FX_VERSION 3
#define FX_NAME "fixture"
#define FX_MASK (1u << 4)
#define FX_RATIO 0.5
#define FX_NEGATIVE (-12)

#if defined(_WIN32)
#define FX_PLATFORM "windows"
#elif defined(__APPLE__)
#define FX_PLATFORM "macos"
#else
#define FX_PLATFORM "linux"
#endif

/* What the fixture's library exports. fixture.c sets it before it
   includes this header, since MSVC wants a declaration and its
   definition to agree; to anyone else reading the header it is
   nothing. */
#ifndef FX_API
#define FX_API
#endif

#ifdef _MSC_VER
#define FX_HAS_EXTENDED 0
#else
#define FX_HAS_EXTENDED 1
#endif

/* Layouts the C compiler reports, for checking the module's own. */
struct fx_layout {
  const char *name;
  size_t size;
  size_t align;
};

struct fx_offset {
  const char *type;
  const char *field;
  size_t offset;
};

FX_API extern const struct fx_layout fx_layouts[];
FX_API extern const struct fx_offset fx_offsets[];

/* Records of every shape the calling conventions treat differently. */
typedef struct { int a; int b; } fx_pair;
typedef struct { char c; short s; } fx_small;
typedef struct { char a, b, c; } fx_three;
typedef struct { float x, y; } fx_vec2;
typedef struct { float x, y, z; } fx_vec3;
typedef struct { float v[4]; } fx_quad;
typedef struct { double x, y; } fx_point;
typedef struct { int i; float f; } fx_mixed;
typedef struct { double d; int i; } fx_tail;
typedef struct { long long a, b, c, d; } fx_large;
typedef struct { char text[20]; } fx_label;
typedef struct { fx_point min; fx_point max; } fx_box;

#pragma pack(push, 1)
typedef struct { char c; int i; } fx_packed;
#pragma pack(pop)

#ifdef _MSC_VER
#pragma pack(push, 1)
typedef struct { char c; int i; } fx_attr_packed;
#pragma pack(pop)
#else
typedef struct {
  char c;
  int i;
} __attribute__((packed)) fx_attr_packed;
#endif

typedef struct { uint8_t tag; _Alignas(16) uint32_t value; } fx_overaligned;
typedef struct { _Alignas(16) uint64_t lo; uint64_t hi; } fx_pair16;

typedef struct {
  unsigned int ready : 1;
  unsigned int mode : 3;
  int delta : 5;
  unsigned int : 0;
  unsigned int count : 12;
  bool flag : 1;
} fx_bits;

typedef union { int i; float f; } fx_int_or_float;
typedef union { double d; long long l; } fx_wide_union;

typedef struct {
  int kind;
  union {
    int number;
    double real;
  };
} fx_tagged;

typedef struct {
  uint32_t length;
  uint8_t data[];
} fx_flexible;

enum fx_color { FX_RED, FX_GREEN = 5, FX_BLUE };

typedef enum { FX_SMALL = -1, FX_LARGE = 100 } fx_size;

_Static_assert(sizeof(fx_pair) == 8, "fx_pair is two ints");

/* Scalars. */
FX_API int8_t fx_i8(int8_t x);
FX_API uint8_t fx_u8(uint8_t x);
FX_API int16_t fx_i16(int16_t x);
FX_API uint16_t fx_u16(uint16_t x);
FX_API int32_t fx_i32(int32_t x);
FX_API uint32_t fx_u32(uint32_t x);
FX_API int64_t fx_i64(int64_t x);
FX_API uint64_t fx_u64(uint64_t x);
FX_API float fx_f32(float x);
FX_API double fx_f64(double x);
FX_API long double fx_long_double(long double x);
FX_API bool fx_not(bool x);
FX_API char fx_upper(char c);
FX_API wchar_t fx_wide_next(wchar_t c);
FX_API long fx_long(long x);
FX_API size_t fx_double_size(size_t x);
FX_API int fx_many(int a, double b, int c, double d, int e, double f, int g, double h,
                   int i, double j, int k, double l, int m, double n, int o, double p);

/* Records by value. */
FX_API fx_pair fx_pair_make(int a, int b);
FX_API int fx_pair_sum(fx_pair p);
FX_API fx_small fx_small_make(char c, short s);
FX_API int fx_small_sum(fx_small s);
FX_API fx_three fx_three_make(char a, char b, char c);
FX_API int fx_three_sum(fx_three t);
FX_API fx_vec2 fx_vec2_scale(fx_vec2 v, float k);
FX_API fx_vec3 fx_vec3_scale(fx_vec3 v, float k);
FX_API fx_quad fx_quad_scale(fx_quad q, float k);
FX_API fx_point fx_point_add(fx_point a, fx_point b);
FX_API fx_mixed fx_mixed_make(int i, float f);
FX_API double fx_mixed_sum(fx_mixed m);
FX_API fx_tail fx_tail_make(double d, int i);
FX_API double fx_tail_sum(fx_tail t);
FX_API fx_large fx_large_make(long long base);
FX_API long long fx_large_sum(fx_large l);
FX_API fx_label fx_label_make(const char *text);
FX_API size_t fx_label_length(fx_label l);
FX_API fx_box fx_box_make(double x0, double y0, double x1, double y1);
FX_API double fx_box_area(fx_box b);
FX_API fx_packed fx_packed_make(char c, int i);
FX_API int fx_packed_sum(fx_packed p);
FX_API fx_attr_packed fx_attr_packed_make(char c, int i);
FX_API int fx_attr_packed_sum(fx_attr_packed p);
FX_API fx_overaligned fx_overaligned_make(uint8_t tag, uint32_t value);
FX_API uint32_t fx_overaligned_sum(fx_overaligned o);
FX_API uint64_t fx_pair16_after_int(int n, fx_pair16 p);
FX_API fx_bits fx_bits_make(unsigned ready, unsigned mode, int delta, unsigned count, bool flag);
FX_API long fx_bits_encode(fx_bits b);
FX_API fx_int_or_float fx_union_from_int(int i);
FX_API float fx_union_as_float(fx_int_or_float u);
FX_API fx_wide_union fx_wide_from_double(double d);
FX_API double fx_wide_as_double(fx_wide_union u);
FX_API fx_tagged fx_tagged_real(double r);
FX_API double fx_tagged_value(fx_tagged t);
FX_API int fx_stack_pairs(int a, int b, int c, int d, int e, int f, fx_pair p, fx_pair q);
FX_API double fx_stack_points(double a, double b, double c, double d, double e, double f,
                              double g, fx_point p, fx_point q);

/* Pointers and memory. */
FX_API void fx_fill(int *out, size_t n);
FX_API long fx_sum(const int *values, size_t n);
FX_API void fx_scale_point(fx_point *p, double k);
FX_API double fx_point_length_squared(const fx_point *p);
FX_API const char *fx_greeting(void);
FX_API char *fx_concat(const char *a, const char *b);
FX_API void fx_release(void *p);
FX_API size_t fx_wide_length(const wchar_t *s);
FX_API size_t fx_utf16_length(const char16_t *s);
FX_API size_t fx_bytes_sum(const uint8_t *data, size_t n);
FX_API void fx_bytes_invert(uint8_t *data, size_t n);
FX_API int *fx_nothing(void);
FX_API fx_flexible *fx_flexible_make(uint32_t length);
FX_API void fx_flexible_free(fx_flexible *f);
FX_API const char *fx_strings_join(const char **parts, size_t n);

/* Enums. */
FX_API enum fx_color fx_next_color(enum fx_color c);
FX_API fx_size fx_flip_size(fx_size s);

/* Callbacks. */
FX_API int fx_apply(int (*f)(int), int x);
FX_API double fx_reduce(const double *values, size_t n, double (*f)(double, double), double start);
FX_API double fx_with_point(double (*f)(fx_point), fx_point p);
FX_API fx_point fx_point_via(fx_point (*f)(double), double x);
FX_API int fx_widened(signed char (*f)(void));
FX_API void fx_set_handler(void (*handler)(int));
FX_API int fx_fire(int value);
FX_API void fx_release_firing(void *p);
FX_API int fx_releases_fired(void);
FX_API int fx_threaded_calls(void (*f)(int), int n);
FX_API void fx_async_start(void (*f)(int), int n);
FX_API void fx_async_join(void);

/* Function pointers coming back. */
typedef int (*fx_binary)(int, int);
FX_API int fx_add(int a, int b);
FX_API int fx_mul(int a, int b);
FX_API fx_binary fx_operation(int which);

/* Variadic. */
FX_API int fx_sum_ints(int count, ...);
FX_API double fx_sum_doubles(int count, ...);
FX_API int fx_format(char *buffer, size_t size, const char *format, ...);

/* errno and friends. */
FX_API int fx_fail_with(int code);
FX_API void fx_set_last_error(unsigned int code);

/* Globals. */
FX_API extern int fx_counter;
FX_API extern const char *fx_label_text;
FX_API int fx_bump(void);

#if FX_HAS_EXTENDED
FX_API __int128 fx_i128_add(__int128 a, __int128 b);
FX_API unsigned __int128 fx_u128_mul(unsigned __int128 a, unsigned __int128 b);
FX_API __int128 fx_i128_after_int(int n, __int128 v);
FX_API __int128 fx_i128_late(int a, int b, int c, int d, int e, int f, int g, __int128 v);
FX_API __int128 fx_i128_via(__int128 (*f)(int, __int128), int n, __int128 v);
FX_API double _Complex fx_complex_mul(double _Complex a, double _Complex b);
FX_API float _Complex fx_complex_conj(float _Complex a);
#endif

#if defined(__x86_64__) && !defined(_WIN32)
__attribute__((ms_abi)) int fx_win64_sum(int a, int b, int c, int d, int e, int f);
__attribute__((ms_abi)) __int128 fx_win64_i128(int a, __int128 b, int c, int d, int e, __int128 f);
__attribute__((ms_abi)) __int128 fx_win64_i128_via(void *f, __int128 v);
#endif

static inline int fx_inline_twice(int x) {
  return x * 2;
}

#ifdef __cplusplus
}
#endif

#endif
