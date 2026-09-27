/*
 * The C library the ffi suites call into. Every function is small and
 * deterministic, so a suite can state exactly what it expects back.
 */
#include "fixture.h"

#include <errno.h>
#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#ifdef _WIN32
#include <windows.h>
#define FX_EXPORT __declspec(dllexport)
#else
#include <pthread.h>
#define FX_EXPORT
#endif

#define LAYOUT(T) { #T, sizeof(T), _Alignof(T) }
#define OFFSET(T, f) { #T, #f, offsetof(T, f) }

FX_EXPORT const struct fx_layout fx_layouts[] = {
  LAYOUT(fx_pair),
  LAYOUT(fx_small),
  LAYOUT(fx_three),
  LAYOUT(fx_vec2),
  LAYOUT(fx_vec3),
  LAYOUT(fx_quad),
  LAYOUT(fx_point),
  LAYOUT(fx_mixed),
  LAYOUT(fx_tail),
  LAYOUT(fx_large),
  LAYOUT(fx_label),
  LAYOUT(fx_box),
  LAYOUT(fx_packed),
  LAYOUT(fx_attr_packed),
  LAYOUT(fx_overaligned),
  LAYOUT(fx_pair16),
  LAYOUT(fx_bits),
  LAYOUT(fx_int_or_float),
  LAYOUT(fx_wide_union),
  LAYOUT(fx_tagged),
  LAYOUT(fx_flexible),
  LAYOUT(enum fx_color),
  LAYOUT(fx_size),
  LAYOUT(long double),
  LAYOUT(wchar_t),
  LAYOUT(long),
  { NULL, 0, 0 },
};

FX_EXPORT const struct fx_offset fx_offsets[] = {
  OFFSET(fx_small, s),
  OFFSET(fx_three, c),
  OFFSET(fx_vec3, z),
  OFFSET(fx_mixed, f),
  OFFSET(fx_tail, i),
  OFFSET(fx_large, d),
  OFFSET(fx_box, max),
  OFFSET(fx_packed, i),
  OFFSET(fx_attr_packed, i),
  OFFSET(fx_overaligned, value),
  OFFSET(fx_pair16, hi),
  OFFSET(fx_tagged, number),
  OFFSET(fx_tagged, real),
  OFFSET(fx_flexible, data),
  { NULL, NULL, 0 },
};

FX_EXPORT int8_t fx_i8(int8_t x) { return (int8_t)(x + 1); }
FX_EXPORT uint8_t fx_u8(uint8_t x) { return (uint8_t)(x + 1); }
FX_EXPORT int16_t fx_i16(int16_t x) { return (int16_t)(x + 1); }
FX_EXPORT uint16_t fx_u16(uint16_t x) { return (uint16_t)(x + 1); }
FX_EXPORT int32_t fx_i32(int32_t x) { return x + 1; }
FX_EXPORT uint32_t fx_u32(uint32_t x) { return x + 1; }
FX_EXPORT int64_t fx_i64(int64_t x) { return x + 1; }
FX_EXPORT uint64_t fx_u64(uint64_t x) { return x + 1; }
FX_EXPORT float fx_f32(float x) { return x * 2.0f; }
FX_EXPORT double fx_f64(double x) { return x * 2.0; }
FX_EXPORT long double fx_long_double(long double x) { return x * 2.0L; }
FX_EXPORT bool fx_not(bool x) { return !x; }
FX_EXPORT char fx_upper(char c) { return (c >= 'a' && c <= 'z') ? (char)(c - 32) : c; }
FX_EXPORT wchar_t fx_wide_next(wchar_t c) { return (wchar_t)(c + 1); }
FX_EXPORT long fx_long(long x) { return x - 1; }
FX_EXPORT size_t fx_double_size(size_t x) { return x * 2; }

FX_EXPORT int fx_many(int a, double b, int c, double d, int e, double f, int g, double h,
                      int i, double j, int k, double l, int m, double n, int o, double p) {
  return a + c + e + g + i + k + m + o + (int)(b + d + f + h + j + l + n + p);
}

FX_EXPORT fx_pair fx_pair_make(int a, int b) { fx_pair p = { a, b }; return p; }
FX_EXPORT int fx_pair_sum(fx_pair p) { return p.a + p.b; }
FX_EXPORT fx_small fx_small_make(char c, short s) { fx_small v = { c, s }; return v; }
FX_EXPORT int fx_small_sum(fx_small s) { return s.c + s.s; }
FX_EXPORT fx_three fx_three_make(char a, char b, char c) { fx_three t = { a, b, c }; return t; }
FX_EXPORT int fx_three_sum(fx_three t) { return t.a + t.b + t.c; }

FX_EXPORT fx_vec2 fx_vec2_scale(fx_vec2 v, float k) {
  fx_vec2 r = { v.x * k, v.y * k };
  return r;
}

FX_EXPORT fx_vec3 fx_vec3_scale(fx_vec3 v, float k) {
  fx_vec3 r = { v.x * k, v.y * k, v.z * k };
  return r;
}

FX_EXPORT fx_quad fx_quad_scale(fx_quad q, float k) {
  fx_quad r;
  for (int i = 0; i < 4; i++) {
    r.v[i] = q.v[i] * k;
  }
  return r;
}

FX_EXPORT fx_point fx_point_add(fx_point a, fx_point b) {
  fx_point r = { a.x + b.x, a.y + b.y };
  return r;
}

FX_EXPORT fx_mixed fx_mixed_make(int i, float f) { fx_mixed m = { i, f }; return m; }
FX_EXPORT double fx_mixed_sum(fx_mixed m) { return m.i + m.f; }
FX_EXPORT fx_tail fx_tail_make(double d, int i) { fx_tail t = { d, i }; return t; }
FX_EXPORT double fx_tail_sum(fx_tail t) { return t.d + t.i; }

FX_EXPORT fx_large fx_large_make(long long base) {
  fx_large l = { base, base + 1, base + 2, base + 3 };
  return l;
}

FX_EXPORT long long fx_large_sum(fx_large l) { return l.a + l.b + l.c + l.d; }

FX_EXPORT fx_label fx_label_make(const char *text) {
  fx_label l;
  memset(&l, 0, sizeof l);
  strncpy(l.text, text, sizeof l.text - 1);
  return l;
}

FX_EXPORT size_t fx_label_length(fx_label l) { return strlen(l.text); }

FX_EXPORT fx_box fx_box_make(double x0, double y0, double x1, double y1) {
  fx_box b = { { x0, y0 }, { x1, y1 } };
  return b;
}

FX_EXPORT double fx_box_area(fx_box b) {
  return (b.max.x - b.min.x) * (b.max.y - b.min.y);
}

FX_EXPORT fx_packed fx_packed_make(char c, int i) { fx_packed p; p.c = c; p.i = i; return p; }
FX_EXPORT int fx_packed_sum(fx_packed p) { return p.c + p.i; }

FX_EXPORT fx_attr_packed fx_attr_packed_make(char c, int i) {
  fx_attr_packed p;
  p.c = c;
  p.i = i;
  return p;
}

FX_EXPORT int fx_attr_packed_sum(fx_attr_packed p) { return p.c + p.i; }

FX_EXPORT fx_overaligned fx_overaligned_make(uint8_t tag, uint32_t value) {
  fx_overaligned o;
  memset(&o, 0, sizeof o);
  o.tag = tag;
  o.value = value;
  return o;
}

FX_EXPORT uint32_t fx_overaligned_sum(fx_overaligned o) { return o.tag + o.value; }

FX_EXPORT uint64_t fx_pair16_after_int(int n, fx_pair16 p) { return p.lo * 1000 + p.hi * 10 + n; }

FX_EXPORT fx_bits fx_bits_make(unsigned ready, unsigned mode, int delta, unsigned count, bool flag) {
  fx_bits b;
  memset(&b, 0, sizeof b);
  b.ready = ready;
  b.mode = mode;
  b.delta = delta;
  b.count = count;
  b.flag = flag;
  return b;
}

FX_EXPORT long fx_bits_encode(fx_bits b) {
  return (long)b.ready + 10L * b.mode + 100L * b.delta + 10000L * b.count + 100000000L * b.flag;
}

FX_EXPORT fx_int_or_float fx_union_from_int(int i) { fx_int_or_float u; u.i = i; return u; }
FX_EXPORT float fx_union_as_float(fx_int_or_float u) { return u.f; }
FX_EXPORT fx_wide_union fx_wide_from_double(double d) { fx_wide_union u; u.d = d; return u; }
FX_EXPORT double fx_wide_as_double(fx_wide_union u) { return u.d; }

FX_EXPORT fx_tagged fx_tagged_real(double r) {
  fx_tagged t;
  t.kind = 2;
  t.real = r;
  return t;
}

FX_EXPORT double fx_tagged_value(fx_tagged t) {
  return t.kind == 2 ? t.real : (double)t.number;
}

FX_EXPORT int fx_stack_pairs(int a, int b, int c, int d, int e, int f, fx_pair p, fx_pair q) {
  return a + b + c + d + e + f + p.a * 100 + p.b * 1000 + q.a * 10000 + q.b * 100000;
}

FX_EXPORT double fx_stack_points(double a, double b, double c, double d, double e, double f,
                                 double g, fx_point p, fx_point q) {
  return a + b + c + d + e + f + g + p.x * 100 + p.y * 1000 + q.x * 10000 + q.y * 100000;
}

FX_EXPORT void fx_fill(int *out, size_t n) {
  for (size_t i = 0; i < n; i++) {
    out[i] = (int)(i * i);
  }
}

FX_EXPORT long fx_sum(const int *values, size_t n) {
  long total = 0;
  for (size_t i = 0; i < n; i++) {
    total += values[i];
  }
  return total;
}

FX_EXPORT void fx_scale_point(fx_point *p, double k) {
  p->x *= k;
  p->y *= k;
}

FX_EXPORT double fx_point_length_squared(const fx_point *p) {
  return p->x * p->x + p->y * p->y;
}

FX_EXPORT const char *fx_greeting(void) { return "hello from C"; }

FX_EXPORT char *fx_concat(const char *a, const char *b) {
  size_t la = strlen(a);
  size_t lb = strlen(b);
  char *out = malloc(la + lb + 1);
  memcpy(out, a, la);
  memcpy(out + la, b, lb + 1);
  return out;
}

FX_EXPORT void fx_release(void *p) { free(p); }

FX_EXPORT size_t fx_wide_length(const wchar_t *s) { return wcslen(s); }

FX_EXPORT size_t fx_utf16_length(const char16_t *s) {
  size_t n = 0;
  while (s[n]) {
    n++;
  }
  return n;
}

FX_EXPORT size_t fx_bytes_sum(const uint8_t *data, size_t n) {
  size_t total = 0;
  for (size_t i = 0; i < n; i++) {
    total += data[i];
  }
  return total;
}

FX_EXPORT void fx_bytes_invert(uint8_t *data, size_t n) {
  for (size_t i = 0; i < n; i++) {
    data[i] = (uint8_t)~data[i];
  }
}

FX_EXPORT int *fx_nothing(void) { return NULL; }

FX_EXPORT fx_flexible *fx_flexible_make(uint32_t length) {
  fx_flexible *f = malloc(sizeof(fx_flexible) + length);
  f->length = length;
  for (uint32_t i = 0; i < length; i++) {
    f->data[i] = (uint8_t)(i + 1);
  }
  return f;
}

FX_EXPORT void fx_flexible_free(fx_flexible *f) { free(f); }

FX_EXPORT const char *fx_strings_join(const char **parts, size_t n) {
  static char joined[256];
  joined[0] = 0;
  for (size_t i = 0; i < n; i++) {
    if (i > 0) {
      strcat(joined, "+");
    }
    strcat(joined, parts[i]);
  }
  return joined;
}

FX_EXPORT enum fx_color fx_next_color(enum fx_color c) {
  return c == FX_RED ? FX_GREEN : c == FX_GREEN ? FX_BLUE : FX_RED;
}

FX_EXPORT fx_size fx_flip_size(fx_size s) { return s == FX_SMALL ? FX_LARGE : FX_SMALL; }

FX_EXPORT int fx_apply(int (*f)(int), int x) { return f(x); }

FX_EXPORT double fx_reduce(const double *values, size_t n, double (*f)(double, double), double start) {
  double acc = start;
  for (size_t i = 0; i < n; i++) {
    acc = f(acc, values[i]);
  }
  return acc;
}

FX_EXPORT double fx_with_point(double (*f)(fx_point), fx_point p) { return f(p); }
FX_EXPORT fx_point fx_point_via(fx_point (*f)(double), double x) { return f(x); }
FX_EXPORT int fx_widened(signed char (*f)(void)) { return (int)f(); }

static void (*fx_handler)(int) = NULL;

FX_EXPORT void fx_set_handler(void (*handler)(int)) { fx_handler = handler; }

FX_EXPORT int fx_fire(int value) {
  if (!fx_handler) {
    return 0;
  }
  fx_handler(value);
  return 1;
}

static int fx_fired_releases = 0;

FX_EXPORT void fx_release_firing(void *p) {
  free(p);
  fx_fired_releases++;
  fx_fire(-1);
}

FX_EXPORT int fx_releases_fired(void) { return fx_fired_releases; }

struct fx_job {
  void (*f)(int);
  int n;
};

#ifdef _WIN32
static DWORD WINAPI fx_worker(LPVOID arg) {
  struct fx_job *job = arg;
  for (int i = 0; i < job->n; i++) {
    job->f(i);
  }
  return 0;
}

FX_EXPORT int fx_threaded_calls(void (*f)(int), int n) {
  struct fx_job job = { f, n };
  HANDLE t = CreateThread(NULL, 0, fx_worker, &job, 0, NULL);
  WaitForSingleObject(t, INFINITE);
  CloseHandle(t);
  return n;
}

static HANDLE fx_async_thread = NULL;
static struct fx_job fx_async_job;

FX_EXPORT void fx_async_start(void (*f)(int), int n) {
  fx_async_job.f = f;
  fx_async_job.n = n;
  fx_async_thread = CreateThread(NULL, 0, fx_worker, &fx_async_job, 0, NULL);
}

FX_EXPORT void fx_async_join(void) {
  if (fx_async_thread) {
    WaitForSingleObject(fx_async_thread, INFINITE);
    CloseHandle(fx_async_thread);
    fx_async_thread = NULL;
  }
}
#else
static void *fx_worker(void *arg) {
  struct fx_job *job = arg;
  for (int i = 0; i < job->n; i++) {
    job->f(i);
  }
  return NULL;
}

FX_EXPORT int fx_threaded_calls(void (*f)(int), int n) {
  struct fx_job job = { f, n };
  pthread_t t;
  pthread_create(&t, NULL, fx_worker, &job);
  pthread_join(t, NULL);
  return n;
}

static pthread_t fx_async_thread;
static int fx_async_running = 0;
static struct fx_job fx_async_job;

FX_EXPORT void fx_async_start(void (*f)(int), int n) {
  fx_async_job.f = f;
  fx_async_job.n = n;
  fx_async_running = 1;
  pthread_create(&fx_async_thread, NULL, fx_worker, &fx_async_job);
}

FX_EXPORT void fx_async_join(void) {
  if (fx_async_running) {
    pthread_join(fx_async_thread, NULL);
    fx_async_running = 0;
  }
}
#endif

FX_EXPORT int fx_add(int a, int b) { return a + b; }
FX_EXPORT int fx_mul(int a, int b) { return a * b; }
FX_EXPORT fx_binary fx_operation(int which) { return which == 0 ? fx_add : which == 1 ? fx_mul : NULL; }

FX_EXPORT int fx_sum_ints(int count, ...) {
  va_list args;
  va_start(args, count);
  int total = 0;
  for (int i = 0; i < count; i++) {
    total += va_arg(args, int);
  }
  va_end(args);
  return total;
}

FX_EXPORT double fx_sum_doubles(int count, ...) {
  va_list args;
  va_start(args, count);
  double total = 0;
  for (int i = 0; i < count; i++) {
    total += va_arg(args, double);
  }
  va_end(args);
  return total;
}

FX_EXPORT int fx_format(char *buffer, size_t size, const char *format, ...) {
  va_list args;
  va_start(args, format);
  int n = vsnprintf(buffer, size, format, args);
  va_end(args);
  return n;
}

FX_EXPORT int fx_fail_with(int code) {
  errno = code;
  return -1;
}

FX_EXPORT void fx_set_last_error(unsigned int code) {
#ifdef _WIN32
  SetLastError(code);
#else
  (void)code;
#endif
}

FX_EXPORT int fx_counter = 0;
FX_EXPORT const char *fx_label_text = "fixture label";

FX_EXPORT int fx_bump(void) { return ++fx_counter; }

#if FX_HAS_EXTENDED
FX_EXPORT __int128 fx_i128_add(__int128 a, __int128 b) { return a + b; }
FX_EXPORT unsigned __int128 fx_u128_mul(unsigned __int128 a, unsigned __int128 b) { return a * b; }
FX_EXPORT __int128 fx_i128_after_int(int n, __int128 v) { return v + n; }

FX_EXPORT __int128 fx_i128_late(int a, int b, int c, int d, int e, int f, int g, __int128 v) {
  return v * 10 + a + b + c + d + e + f + g;
}

FX_EXPORT __int128 fx_i128_via(__int128 (*f)(int, __int128), int n, __int128 v) {
  return f(n, v) + 1;
}
FX_EXPORT double _Complex fx_complex_mul(double _Complex a, double _Complex b) { return a * b; }
FX_EXPORT float _Complex fx_complex_conj(float _Complex a) { return __builtin_conjf(a); }
#endif

#if defined(__x86_64__) && !defined(_WIN32)
__attribute__((ms_abi)) int fx_win64_sum(int a, int b, int c, int d, int e, int f) {
  return a + 2 * b + 3 * c + 4 * d + 5 * e + 6 * f;
}

typedef __int128 (__attribute__((ms_abi)) *fx_win64_wide)(int, __int128, int, int, int, __int128);

__attribute__((ms_abi)) __int128 fx_win64_i128(int a, __int128 b, int c, int d, int e, __int128 f) {
  return b * 2 + f + a + 10 * c + 100 * d + 1000 * e;
}

__attribute__((ms_abi)) __int128 fx_win64_i128_via(void *f, __int128 v) {
  return ((fx_win64_wide)f)(1, v, 2, 3, 4, v) + 1;
}
#endif
