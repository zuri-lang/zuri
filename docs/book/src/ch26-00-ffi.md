# Foreign Functions: C and Rust

The `ffi` module calls code written in C and in Rust. It loads a shared
library, describes the functions in it, and calls them with ordinary Zuri
values; it turns Zuri functions into C function pointers so a library can
call back; and it reads the declarations a header or a crate already
contains, so the description is written once, by the people who wrote the
library.

Every value that crosses is converted and checked against its C type. An
integer that does not fit is a `RangeError` at the call, not a truncated
argument inside the library. Memory the module allocates knows its size,
and a read past its end is refused before it happens. Where a check cannot
be made, because C handed back an address with no extent attached, the
module says so plainly rather than guessing.

A C library, a Rust `cdylib`, and a static library of either are all
reachable, and the layout of every record, packed, over-aligned or full of
bitfields, is the one the platform's own compiler would give it.

- [Following Along](#following-along)
- [Introduction](#introduction)
- [Loading a Library](#loading-a-library)
- [Calling a Function](#calling-a-function)
- [Types](#types)
- [Numbers at the Boundary](#numbers-at-the-boundary)
- [Declaring from C](#declaring-from-c)
- [Declaring from Rust](#declaring-from-rust)
- [Pointers and Memory](#pointers-and-memory)
- [Strings and Text](#strings-and-text)
- [Structs, Unions and Arrays](#structs-unions-and-arrays)
- [Enums and Constants](#enums-and-constants)
- [Callbacks](#callbacks)
- [Callbacks and Threads](#callbacks-and-threads)
- [Variadic Functions](#variadic-functions)
- [Errors and errno](#errors-and-errno)
- [Ownership and Lifetimes](#ownership-and-lifetimes)
- [Static Libraries](#static-libraries)
- [Isolates](#isolates)
- [Rust in Depth](#rust-in-depth)
- [Platform Differences](#platform-differences)
- [What Is Checked](#what-is-checked)
- [What the Module Refuses](#what-the-module-refuses)
- [Module Reference](#module-reference)

> Blocks on this page that list several calls together are **reference
> listings**, not programs: they show the shape of each call rather than a
> sequence to run. Anything presented as a complete program runs as
> written. The programs that call the C runtime run on Linux as shown;
> the ones that need a library of your own are shown rather than run.

## Following Along

The C runtime is on every machine, so most examples below call it.
`ffi.LIBC` names it for the platform the program is running on, and
`ffi.LIBM` names the maths library:

```zuri
import ffi

var libc = ffi.open(ffi.LIBC)
var strlen = libc.function('strlen', ffi.size_t, [ffi.string])

echo strlen('Hello, C')
```

```console
8
```

Some sections use a small library of your own. Save this as
`geometry.c`:

```c
#include <math.h>
#include <stdlib.h>

typedef struct { double x, y; } point;

double distance(point a, point b) {
  return hypot(a.x - b.x, a.y - b.y);
}

point midpoint(const point *a, const point *b) {
  point m = { (a->x + b->x) / 2, (a->y + b->y) / 2 };
  return m;
}

void scale_all(point *points, size_t n, double k) {
  for (size_t i = 0; i < n; i++) {
    points[i].x *= k;
    points[i].y *= k;
  }
}
```

and build it as a shared library with the C compiler:

```sh
cc -shared -fPIC geometry.c -o libgeometry.so -lm       # Linux
cc -dynamiclib geometry.c -o libgeometry.dylib          # macOS
cl /LD geometry.c                                       # Windows
```

The Rust sections use a crate built with `crate-type = ["cdylib"]`, shown
where it is used.

## Introduction

Every language that grows up eventually needs code it did not write: a
database engine, a codec, a cryptography library, a scientific kernel, a
system call the standard library has no wrapper for. The code exists, it
is fast and it is tested, and it speaks the C calling convention, which
is the one convention every language on every platform agrees on. Rust
libraries speak it too, through `extern "C"`.

Calling across that boundary is a matter of describing, exactly, what the
other side expects: how wide each integer is, where each member of a
struct sits, which register a value travels in. The machine does not
check any of it. A description that is off by one byte corrupts memory
silently, and the bug shows up somewhere unrelated, later.

`ffi` makes that description a value the program can inspect, builds it
from the source the library's authors already wrote, lays out every type
the way the platform's compiler does, and checks every value against it
at the moment it crosses:

```zuri
import ffi

var c = ffi.open(ffi.LIBC).declare('
  int abs(int n);
  typedef struct { int quot; int rem; } div_t;
  div_t div(int numerator, int denominator);
')

echo c.abs(-7)
echo c.div(17, 5)
```

```console
7
{quot: 3, rem: 2}
```

The declarations are the ones `<stdlib.h>` contains. The struct came back
as a dictionary. And the conversion rules held on the way in:

```zuri,ignore
c.abs(3000000000)
```

```console
RangeError: abs() argument 1 'n': 3000000000 is out of range for 'int', which holds -2147483648 to 2147483647
```

## Loading a Library

`ffi.open()` loads a shared library and returns a `Library`. It takes a
path, a file name, or a bare name the platform's conventions complete:

```zuri,ignore
var sqlite = ffi.open('sqlite3')                    # libsqlite3.so, libsqlite3.dylib, sqlite3.dll
var local = ffi.open('./build/libgeometry.so')      # a path, as given
var vendored = ffi.open('geometry', { paths: ['vendor/lib'] })
```

A bare name is tried in the directories given as `paths` first, then in
the platform's own search order. On Linux, a library's unversioned
`libname.so` is usually only installed with its development package, so
`ffi.open()` also asks the dynamic loader's cache for the versioned file
the runtime package ships, such as `libsqlite3.so.0`. On macOS the name is
tried as a `.dylib` and as a framework.

`ffi.find()` answers the same question without loading anything:

```zuri
import ffi

echo ffi.find('zuri_has_no_such_library')
```

```console
nil
```

With no name at all, `ffi.open()` returns the running process, whose
symbols include the C runtime and everything already loaded globally.

Two options change how a library is loaded. `lazy: true` resolves its
symbols as they are first used instead of all at once. `global: true`
makes its symbols visible to libraries loaded after it and to the process
handle, which a plugin that expects its host's symbols needs. Both are
Unix concepts and have no effect on Windows, where every loaded module is
searched when the process is asked for a symbol.

A library that cannot be found or loaded raises `LoadError`, carrying the
loader's own explanation: a missing dependency, a library for another
architecture, a file that is not a library.

### Symbols

A `Library` answers whether it exports a name, and where:

```zuri
import ffi

var libc = ffi.open(ffi.LIBC)

echo libc.has('strlen')
echo libc.has('zuri_has_no_such_symbol')
echo libc.symbol('zuri_has_no_such_symbol')
```

```console
true
false
nil
```

`symbol()` returns a `Pointer` to the symbol. On glibc, `name@VERSION`
asks for a particular version of a versioned symbol, for the rare program
that must pin one.

### Closing

`close()` stops a library being used: every function bound from it
raises `LoadError` from then on. The code itself is unloaded once nothing
refers to the library any longer, functions bound from it included, so a
closed library is never unloaded out from under a call in progress.

## Calling a Function

A function is bound by name, return type and parameter types, and comes
back as an ordinary Zuri function:

```zuri
import ffi

var libm = ffi.open(ffi.LIBM)
var pow = libm.function('pow', ffi.double, [ffi.double, ffi.double])

echo pow(2, 10)
echo pow.name()
echo pow.arity()
```

```console
1024
pow
2
```

It can be stored, passed to `map()`, spawned onto an isolate, and called
like any other function. The number of arguments is checked like any
function's:

```zuri,ignore
pow(2)
```

```console
ArgumentError: 'pow' expects 2 arguments, got 1
```

`ffi.describe()` reports what a foreign function is, and
`ffi.is_foreign()` tells a foreign function from a Zuri one:

```zuri
import ffi

var abs = ffi.open(ffi.LIBC).function('abs', ffi.int, [ffi.int])
var about = ffi.describe(abs)

echo about.name
echo about.type.name()
echo ffi.is_foreign(abs)
echo ffi.is_foreign(@(x) => x)
```

```console
abs
int (*)(int)
true
false
```

Binding one function at a time suits a handful. For more, declarations
read a header, as the next sections show.

## Types

Every C type is a `Type`. The module exports the built-in ones:

| Zuri | C | Size |
| --- | --- | --- |
| `ffi.void` | `void` | |
| `ffi.bool` | `bool` | 1 |
| `ffi.char`, `ffi.schar`, `ffi.uchar` | `char`, `signed char`, `unsigned char` | 1 |
| `ffi.short`, `ffi.ushort` | `short`, `unsigned short` | 2 |
| `ffi.int`, `ffi.uint` | `int`, `unsigned int` | 4 |
| `ffi.long`, `ffi.ulong` | `long`, `unsigned long` | 8, or 4 on Windows |
| `ffi.longlong`, `ffi.ulonglong` | `long long`, `unsigned long long` | 8 |
| `ffi.int8` ... `ffi.uint64` | `int8_t` ... `uint64_t` | 1 to 8 |
| `ffi.int128`, `ffi.uint128` | `__int128`, `unsigned __int128` | 16 |
| `ffi.size_t`, `ffi.ssize_t`, `ffi.ptrdiff_t` | the same | 8 |
| `ffi.intptr_t`, `ffi.uintptr_t` | the same | 8 |
| `ffi.wchar_t`, `ffi.char16_t`, `ffi.char32_t` | the same | 4, 2 on Windows; 2; 4 |
| `ffi.float`, `ffi.double` | the same | 4, 8 |
| `ffi.longdouble` | `long double` | 16, or 8 on Windows and Apple Arm |
| `ffi.complex_float`, `ffi.complex_double` | `float _Complex`, `double _Complex` | 8, 16 |
| `ffi.ptr` | `void *` | 8 |
| `ffi.string` | `const char *`, as text | 8 |
| `ffi.wstring` | `const wchar_t *`, as text | 8 |

and Rust's names for the same types: `ffi.i8` through `ffi.u128`,
`ffi.isize`, `ffi.usize`, `ffi.f32`, `ffi.f64`, and `ffi.rust_char` for
Rust's `char`. A Rust name and its C counterpart are the same type:

```zuri
import ffi

echo ffi.i32.equals(ffi.int32)
echo ffi.int32.equals(ffi.int)
echo ffi.usize.equals(ffi.size_t)
```

```console
true
true
true
```

A type knows its size, alignment and kind on this platform:

```zuri
import ffi

echo ffi.int.size()
echo ffi.double.align()
echo ffi.uint16.kind()
echo ffi.string.kind()
```

```console
4
8
int
string
```

### Building types

Pointers, arrays, `const` and function types are built from other types:

```zuri
import ffi

var names = ffi.pointer(ffi.char).array(4)
var compare = ffi.function_type(ffi.int, [ffi.ptr, ffi.ptr])

echo names.name()
echo names.size()
echo compare.pointer().name()
echo ffi.char.as_const().pointer().name()
```

```console
char *[4]
32
int (*)(void *, void *)
const char *
```

`ffi.type()` reads a C spelling of a built-in type, which is often the
shortest way to write one:

```zuri
import ffi

echo ffi.type('unsigned long long').equals(ffi.ulonglong)
echo ffi.type('void (*)(int, const char *)').kind()
```

```console
true
function pointer
```

Structs, unions and enums are built member by member; [Structs, Unions
and Arrays](#structs-unions-and-arrays) and [Enums and
Constants](#enums-and-constants) cover them.

## Numbers at the Boundary

A Zuri number is a double, which holds every integer up to 2^53 exactly
and no further. C integer types reach 2^64 and, with `__int128`, 2^128.
So integers cross the boundary this way:

- **Going in**, a number must be a whole number that fits the type, and
  a bigint may be given for any integer type. A fraction, a string, or a
  bool for an integer is a `TypeError`; a value outside the type's range
  is a `RangeError`. Nothing is ever truncated or wrapped.
- **Coming out**, an integer is a number when it lies within 2^53 of
  zero, and a bigint beyond that, so no value is ever rounded.

```zuri
import ffi

var strtoull = ffi.open(ffi.LIBC).function('strtoull', ffi.uint64,
  [ffi.string, ffi.ptr, ffi.int])

echo strtoull('42', nil, 10)
echo strtoull('18446744073709551615', nil, 10)
echo typeof(strtoull('18446744073709551615', nil, 10))
```

```console
42
18446744073709551615n
bigint
```

A type that can be either is the program's to handle:
`is_bigint(value)`, or comparing against a bigint, which compares
correctly with a number too.

Floating-point types take any number. `long double` is wider than a
double on some platforms, 80-bit extended precision on x86-64 Linux and
macOS and 128-bit quad precision on Arm Linux; going in is exact, and
coming out rounds to the nearest double, as C's own conversion does.

`bool` takes and gives a Zuri bool, and a C character type takes a number
or a one-character string:

```zuri
import ffi

var toupper = ffi.open(ffi.LIBC).function('toupper', ffi.int, [ffi.int])

echo toupper('q'.ord())
echo toupper(97).chr()
```

```console
81
A
```

A complex number is a list of two, `[real, imaginary]`, and passing one
by value is available wherever the C compiler has `_Complex`, which is
everywhere but Windows.

## Declaring from C

`ffi.declare()` reads C declarations and returns a `Declarations`: a set
of types, functions, variables and constants that is not tied to any
library. Binding it to one produces a namespace:

```zuri
import ffi

var api = ffi.declare('
  typedef struct { long quot; long rem; } ldiv_t;
  ldiv_t ldiv(long numerator, long denominator);
  long labs(long n);
')

var c = api.bind(ffi.open(ffi.LIBC))

echo c.labs(-12)
echo c.ldiv(100, 7)
echo c.ldiv_t.size()
```

```console
12
{quot: 14, rem: 2}
16
```

`Library.declare()` does both steps at once, and is what most programs
use.

The namespace holds a function for each declared function, a `Pointer`
for each declared variable, each constant, and each type that has a name
of its own. It is read the way a module is. A tagged type with no typedef,
such as `struct stat`, is reached through the declarations instead:
`api.type('struct stat')`.

A declared function that the library does not export is a `SymbolError`
naming every one that is missing. `allow_missing: true` binds the rest
and leaves those out, for a header describing several versions of a
library.

### What is read

Everything a header says about an API:

- `typedef`, `struct`, `union` and `enum`, including forward declarations,
  self-referencing records, nested records, anonymous members, bitfields
  and flexible array members;
- function prototypes, variadic ones included, and function pointer types
  however deeply they nest;
- `extern` variables;
- `_Static_assert`, which is evaluated, so a header that checks its own
  assumptions checks them here too;
- `__attribute__((packed))`, `aligned(n)`, `ms_abi` and `sysv_abi`,
  `__declspec(align(n))`, `_Alignas`, and `asm` labels that give a
  function a different symbol name; every other attribute is read past;
- `extern "C" { ... }`, as headers shared with C++ write it.

A function with a body, as an inline helper in a header has one, is read
and not bound, because the library need not export it.

```zuri
import ffi

var api = ffi.declare('
  struct node;
  typedef struct node node;
  struct node { int value; node *next; };

  typedef struct {
    struct { double x, y; } origin;
    union { int id; char tag[8]; };
  } shape;

  _Static_assert(sizeof(shape) == 24, "shape is 24 bytes");
')

echo api.type('node').size()
echo api.type('shape').offset_of('tag')
echo api.type('shape').has_field('id')
```

```console
16
16
true
```

### The preprocessor

Real header text is full of preprocessor lines, so enough of the
preprocessor runs for it to read as written:

- `#define` of a value is expanded wherever the name appears and becomes a
  constant: numbers in any base with any suffix, floating-point numbers,
  character constants, strings, adjacent strings joined, and expressions
  over other constants.
- `#if`, `#ifdef`, `#ifndef`, `#elif`, `#else` and `#endif` are evaluated,
  with `defined()`, against the macros the target platform's compiler
  predefines: `__linux__`, `__APPLE__`, `_WIN32`, `__x86_64__`,
  `__aarch64__`, `__LP64__` and the rest. `__has_include()` and the
  other `__has_` queries answer no.
- `#pragma pack` applies to the records declared after it, exactly where
  it is written.
- `#undef` removes a macro, and `#error` stops with its message.
- Including a C standard header is accepted, because everything it
  declares is already known; `<stdio.h>` also declares `FILE`.

```zuri
import ffi

var api = ffi.declare('
  #include <stdint.h>
  #define VERSION_MAJOR 2
  #define VERSION_MINOR 7
  #define VERSION ((VERSION_MAJOR << 8) | VERSION_MINOR)
  #define NAME "geo" "metry"

  #if VERSION >= 0x200
  typedef struct { uint32_t flags; double scale; } options;
  #else
  typedef struct { uint32_t flags; } options;
  #endif

  #pragma pack(push, 1)
  typedef struct { uint8_t kind; uint32_t length; } header;
  #pragma pack(pop)
')

echo api.constant('VERSION')
echo api.constant('NAME')
echo api.type('options').size()
echo api.type('header').size()
```

```console
519
geometry
16
5
```

Two things are refused, each with the line and column of the problem.
Including any other file, because declarations are read as given, never
fetched: paste in the ones the program needs. And using a function-like
macro, which would need the preprocessor's full expansion rules; define a
function-like macro and it is recorded, and only using one is an error.

### Text returns

C cannot say who owns a returned pointer, but its convention is clear
enough to follow: a function returning `const char *` hands back text it
keeps, and one returning `char *` usually hands over memory. So a
declared function returning `const char *` returns a string, `nil` for a
null pointer, and one returning `char *` returns a `Pointer`, which the
program reads and releases. The same holds for `const wchar_t *`.

```zuri
import ffi

var c = ffi.open(ffi.LIBC).declare('
  int setenv(const char *name, const char *value, int overwrite);
  const char *getenv(const char *name);
')

c.setenv('ZURI_FFI_DEMO', 'hello', 1)
echo c.getenv('ZURI_FFI_DEMO')
echo c.getenv('ZURI_FFI_NOT_SET')
```

```console
hello
nil
```

`getenv` is declared `char *getenv(const char *)` in the real header;
writing it as `const char *` here is the program saying it will not free
the result, which is true.

### Growing a set, and sharing one

Sources are added to a set one after another, and each sees what came
before. One set can include another, whose types it can then use without
owning them, which is how two libraries share one set of common types:

```zuri
import ffi

var common = ffi.declare('typedef struct { double x, y; } point;')

var shapes = ffi.declarations()
  .include(common)
  .declare('typedef struct { point from, to; } segment;')
  .declare('double length(segment s);')

echo shapes.type('segment').size()
echo shapes.functions().length.signature().params[0].name()
```

```console
32
segment
```

`types()`, `functions()`, `variables()` and `constants()` list what a set
holds, and `type()` resolves any C spelling against it:
`shapes.type('segment *[2]')`.

## Declaring from Rust

A Rust library is reached through the C ABI it exports: `extern "C"`
functions, and the `#[repr(C)]` types they take. `ffi.declare_rust()`
reads that surface as a crate writes it, so its source can be handed over
as it stands. Given this `lib.rs`, built with `crate-type = ["cdylib"]`:

```rust
use std::ffi::{CStr, CString, c_char};

#[repr(C)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

#[repr(C)]
pub enum Shape {
    Circle { radius: f64 },
    Rect { w: f64, h: f64 },
}

#[unsafe(no_mangle)]
pub extern "C" fn area(shape: Shape) -> f64 {
    match shape {
        Shape::Circle { radius } => std::f64::consts::PI * radius * radius,
        Shape::Rect { w, h } => w * h,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn midpoint(a: &Point, b: &Point) -> Point {
    Point { x: (a.x + b.x) / 2.0, y: (a.y + b.y) / 2.0 }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn greet(name: *const c_char) -> *mut c_char {
    let name = unsafe { CStr::from_ptr(name) }.to_string_lossy();
    CString::new(format!("hello, {name}")).unwrap().into_raw()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn free_greeting(text: *mut c_char) {
    drop(unsafe { CString::from_raw(text) });
}
```

the Zuri side reads the same file:

```zuri,ignore
import ffi

var source = file('shapes/src/lib.rs').read()
var shapes = ffi.open('shapes', { paths: ['shapes/target/release'] }).declare_rust(source)

echo shapes.area({ variant: 'Rect', w: 2, h: 3 })
echo shapes.midpoint({ x: 0, y: 0 }, { x: 4, y: 2 })

var greeting = shapes.greet('zuri').own(shapes.free_greeting)
echo greeting.read_string()
```

```console
6
{x: 2, y: 1}
hello, zuri
```

What is read, and what is skipped:

- functions in `extern "C"` blocks (and `unsafe extern "C"` ones), with
  `#[link_name]` for a different symbol;
- `extern "C" fn` definitions marked `#[no_mangle]` or `#[export_name]`,
  with their bodies skipped; one with neither is refused, because it is
  exported under a mangled name nothing can look up;
- `#[repr(C)]`, `#[repr(C, packed)]`, `#[repr(C, align(N))]` and
  `#[repr(transparent)]` structs, tuple structs and unions;
- enums with `#[repr(C)]` or an integer `#[repr]`, with and without
  fields;
- `type` aliases, `const` items and statics;
- `#[cfg(...)]`, evaluated for the platform the program is running on;
- everything else, from `use` lines to `impl` blocks, macros and private
  functions, is read past.

A type may be used before it is declared, as Rust allows, and a struct
without `#[repr(C)]` can only be used through a pointer, because Rust's
own layout is unspecified. [Rust in Depth](#rust-in-depth) covers how
each Rust type crosses.

C and Rust declarations go in the same set, and each sees the other's
types, which suits a crate that ships a C header beside its source.

## Pointers and Memory

A `Pointer` is an address with, optionally, the type of what it points
at. Memory comes from `ffi.alloc()`, zeroed and typed:

```zuri
import ffi

var numbers = ffi.alloc(ffi.int, 4)
numbers.set(0, 10)
numbers.set(3, 40)

echo numbers.get(0)
echo numbers.to_list(4)
echo numbers.add(3).get()
echo numbers.size()
```

```console
10
[10, 0, 0, 40]
40
16
```

`get()` and `set()` read and write elements of the pointer's type, as C's
`ptr[i]` does, and `add()` steps by elements, as C's `ptr + i` does.
`read()` and `write()` take a type of their own and a byte offset, and work
on any pointer:

```zuri
import ffi

var header = ffi.alloc_bytes(16)
header.write(ffi.uint32, 3405691582)
header.write(ffi.double, 2.5, 8)

echo header.read(ffi.uint32)
echo header.read(ffi.double, 8)
echo header.read(ffi.uint8)
echo header.cast(ffi.uint16).get(1)
```

```console
3405691582
2.5
190
51966
```

Memory is little-endian on every platform the module runs on, which the
third line shows. `cast()` gives a pointer a type, or takes it away.

### Checked access

Memory the module allocated knows its extent, and every access through a
pointer into it is checked against that extent before it happens:

```zuri
import ffi

var numbers = ffi.alloc(ffi.int, 4)

catch {
  numbers.get(4)
} as e {
  echo e.type
  echo e.message
}
```

```console
PointerError
4 bytes at offset 16 fall outside the 16-byte block this pointer belongs to
```

A write that fails to convert writes nothing, so a record is never left
half updated. Freed memory refuses every access, and freeing it twice is
an error rather than a double free.

A pointer C handed back carries no extent, because C did not say how much
memory is behind it. Only a null pointer is caught through one; how much
is there is whatever the library's documentation says.

### Pointers as arguments

A pointer parameter accepts more than a `Pointer`, each for the duration
of the call:

| Passed | Becomes |
| --- | --- |
| `nil` | a null pointer |
| a `Pointer` | its address |
| a string | a NUL-terminated copy, for a pointer to characters |
| bytes | the bytes' own storage, with nothing copied |
| a list | a temporary array of the target type, written back after the call |
| a dictionary | a temporary record, written back after the call |
| a Zuri function | a callback, for a function pointer |
| a `Callback` | its code |
| a foreign function | its address |

A list or dictionary is written back only when the pointer is not to a
`const` type, so it works as an out-parameter:

```zuri
import ffi

var frexp = ffi.open(ffi.LIBM).function('frexp', ffi.double,
  [ffi.double, ffi.pointer(ffi.int)])

var exponent = [0]
echo frexp(48, exponent)
echo exponent[0]
```

```console
0.75
6
```

Bytes pass their own storage, so a C function that fills a buffer fills
the bytes directly:

```zuri
import ffi

var memset = ffi.open(ffi.LIBC).function('memset', ffi.ptr,
  [ffi.ptr, ffi.int, ffi.size_t])

var buffer = bytes(6)
memset(buffer, 65, 4)
echo buffer
```

```console
(41 41 41 41 00 00)
```

A bytes value passed to a call must not be resized by a callback while
the call is running; its storage is only guaranteed to stay where it is
for as long as nothing changes its length.

### Other allocations

`ffi.alloc_bytes(size, align)` allocates untyped memory. `ffi.malloc()`
allocates from the C allocator, for memory that will be handed to C code
which frees it with `free()`; it is never freed when the pointer is
collected. `ffi.at(address, type)` makes a pointer from a known address.
`copy_from()`, `fill()` and `compare()` are `memmove`, `memset` and
`memcmp` over checked memory.

## Strings and Text

A string passed where a pointer to characters is expected becomes a
NUL-terminated copy for the duration of the call, in the encoding the
character type implies: UTF-8 for `char`, UTF-16 for `char16_t`, UTF-32
for `char32_t`, and `wchar_t`'s own, which is UTF-16 on Windows and
UTF-32 elsewhere.

`ffi.string` and `ffi.wstring` are pointer types that also convert back:
a function declared to return one returns a Zuri string, and `nil` for a
null pointer.

```zuri
import ffi

var libc = ffi.open(ffi.LIBC)
var strstr = libc.function('strstr', ffi.string, [ffi.string, ffi.string])

echo strstr('needle in a haystack', 'hay')
echo strstr('needle in a haystack', 'pin')
```

```console
haystack
nil
```

Text that has to outlive a call, because it is stored in a struct or kept
by the library, goes in memory of its own with `ffi.alloc_string()`:

```zuri
import ffi

var text = ffi.alloc_string('naïve')

echo text.read_string()
echo text.read_bytes(6)
echo ffi.alloc_string('naïve', 'utf-16').read_string(nil, 'utf-16')
```

```console
naïve
(6e 61 c3 af 76 65)
naïve
```

`read_string()` reads up to the terminator, or exactly a given number of
code units, in any of the four encodings, and raises `ValueError` for
text that is not valid in its encoding. `write_string()` writes text and
its terminator. When a read has no terminator inside a block of known
size, it is refused rather than run past the end.

## Structs, Unions and Arrays

A record is built member by member, and laid out the way the platform's C
compiler lays it out:

```zuri
import ffi

var Header = ffi.struct('Header')
  .add_field('tag', ffi.uint8)
  .add_field('length', ffi.uint32)
  .add_field('checksum', ffi.uint64)

echo Header.size()
echo Header.offset_of('length')
echo Header.offset_of('checksum')
```

```console
16
4
8
```

It is sealed the first time anything needs its size, including passing a
value of it, and cannot change after that. `set_packed()` packs it as
`#pragma pack` does, `set_align()` raises its alignment, and `add_field()`
takes a minimum alignment for one member, as `_Alignas` gives:

```zuri
import ffi

var Packed = ffi.struct('Packed').add_field('tag', ffi.uint8).add_field('length', ffi.uint32).set_packed()
var Wide = ffi.struct('Wide').add_field('tag', ffi.uint8).set_align(64)

echo Packed.size()
echo Wide.size()
```

```console
5
64
```

### Values

A record value is a dictionary of its members, going in and coming out. A
member left out of a dictionary going in is zero. Nested records are
nested dictionaries, and array members are lists, except that an array of
`char` reads as the string in it and an array of other bytes reads as
bytes:

```zuri
import ffi

var Item = ffi.struct('Item')
  .add_field('name', ffi.char.array(8))
  .add_field('counts', ffi.int.array(3))

var item = ffi.alloc(Item)
item.set(0, { name: 'widget', counts: [1, 2] })

echo item.get()
echo item.get_field('counts')
```

```console
{name: widget, counts: [1, 2, 0]}
[1, 2, 0]
```

Through a pointer, `get_field()` and `set_field()` read and write one
member, as `ptr->name` does, and `field()` points at one, as `&ptr->name`
does. `fields()` lists the members with their offsets.

### Bitfields

`add_bitfield()` adds a member of a given width. Bitfields follow the
platform's rules for packing them, GCC's and Clang's on Linux and macOS
and MSVC's on Windows, including a zero-width bitfield ending a storage
unit:

```zuri
import ffi

var Flags = ffi.struct('Flags')
  .add_bitfield('ready', ffi.uint, 1)
  .add_bitfield('mode', ffi.uint, 3)
  .add_bitfield('delta', ffi.int, 4)

var flags = ffi.alloc(Flags)
flags.set_field('mode', 5)
flags.set_field('delta', -3)

echo flags.get()
echo Flags.fields()[2].bit_offset

catch {
  flags.set_field('mode', 8)
} as e {
  echo e.message
}
```

```console
{ready: 0, mode: 5, delta: -3}
4
8 does not fit in the 3-bit field 'mode', which holds 0 to 7
```

### Unions

A union value going in is a dictionary holding one member, the one to
store. Coming out, it holds every member, each read from the same bytes,
because only the program knows which one is meaningful:

```zuri
import ffi

var Bits = ffi.union('Bits').add_field('f', ffi.float).add_field('u', ffi.uint32)
var value = ffi.alloc(Bits)
value.set(0, { f: 1 })

echo value.get()
```

```console
{f: 1, u: 1065353216}
```

A union read back and passed in again is accepted as it is, because its
members agree; members that disagree are a `TypeError`.

### By value

Records pass to and return from functions by value, whatever their shape.
Which registers a record travels in depends on its size and on the types
of its members, differently on every platform, and those rules are
applied to the record's real layout, packed and over-aligned records and
bitfields included:

```zuri,ignore
var geometry = ffi.open('geometry').declare('
  typedef struct { double x, y; } point;
  double distance(point a, point b);
  point midpoint(const point *a, const point *b);
  void scale_all(point *points, size_t n, double k);
')

echo geometry.distance({ x: 0, y: 0 }, { x: 3, y: 4 })
echo geometry.midpoint({ x: 0, y: 0 }, { x: 4, y: 2 })

var points = [{ x: 1, y: 1 }, { x: 2, y: 3 }]
geometry.scale_all(points, 2, 10)
echo points
```

```console
5
{x: 2, y: 1}
[{x: 10, y: 10}, {x: 20, y: 30}]
```

`midpoint` takes pointers to records, and the dictionaries went in as
temporary records. `scale_all` takes a pointer to an array of them, and a
list of dictionaries went in as a temporary array and was written back.

## Enums and Constants

An enum is an integer type with named constants. A value of one is a
number, and anywhere one goes in, a constant's name may go instead:

```zuri
import ffi

var Level = ffi.enum('Level')
  .add_constant('LOW', 1)
  .add_constant('HIGH', 10)

echo Level.constants()
echo Level.value('HIGH')
echo Level.size()

var level = ffi.alloc(Level)
level.set(0, 'HIGH')
echo level.get()
```

```console
{LOW: 1, HIGH: 10}
10
4
10
```

Declared enums come with their constants, and a declared enum is stored
as `int` unless its values need more, or it says otherwise
(`enum x : uint8_t`), or GCC's `packed` attribute asks for the smallest
type. Every enum constant, `#define` constant and Rust `const` is a
constant of the declarations, and a member of the namespace they bind to.

## Callbacks

A Zuri function passed where a function pointer is expected becomes a C
function pointer for the length of that call:

```zuri
import ffi

var c = ffi.open(ffi.LIBC).declare('
  void qsort(void *base, size_t count, size_t size,
             int (*compare)(const void *, const void *));
')

var numbers = ffi.alloc(ffi.int, 5)
for i, n in [42, 7, 19, 3, 25] {
  numbers.set(i, n)
}

c.qsort(numbers, 5, 4, @(a, b) {
  return a.cast(ffi.int).get() - b.cast(ffi.int).get()
})

echo numbers.to_list(5)
```

```console
[3, 7, 19, 25, 42]
```

The arguments arrive converted from their C types, pointers as
`Pointer`s, records as dictionaries, and the return value is converted to
the callback's return type, a small integer widened to a full register the
way a C compiler returns one.

### Lasting callbacks

A library that keeps a function pointer and calls it later, a signal
handler, an event callback, a logging hook, needs a callback that
outlives the call that handed it over. `ffi.callback()` makes one:

```zuri,ignore
var on_event = ffi.callback(@(code) {
  echo 'event ${code}'
}, ffi.function_type(ffi.void, [ffi.int]))

library.set_handler(on_event)
```

A lasting callback lives until `release()`. Nothing else frees it,
because nothing can know when the library has finished with the pointer;
a callback that is never released lives as long as the program. After
`release()` it can no longer be passed, and C must not call it again.

### Errors inside a callback

A Zuri error cannot unwind through C frames; the C code in between
expects to finish. So an error raised inside a callback is trapped, C gets
a zero back (or the callback's `error_value`), and the error is raised
again, as it was, the moment the C function returns:

```zuri
import ffi

var c = ffi.open(ffi.LIBC).declare('
  void qsort(void *base, size_t count, size_t size,
             int (*compare)(const void *, const void *));
')

var numbers = ffi.alloc(ffi.int, 3)

catch {
  c.qsort(numbers, 3, 4, @(a, b) {
    raise ValueError('comparison refused')
  })
} as e {
  echo '${e.type}: ${e.message}'
}
```

```console
ValueError: comparison refused
```

The first error is the one raised; later calls into the callback during
the same C call see the error value.

## Callbacks and Threads

C libraries run threads of their own, and those threads call callbacks.
A Zuri isolate runs on one thread, so a call from any other thread is
posted to the isolate that made the callback, and the calling thread
waits until the isolate has run it. The isolate answers at its next
safepoint, the same points where it checks for signals, or at once when
it is inside a foreign call or in `ffi.serve()`.

That covers a library whose threads call back while the program does
something else. It does not cover a function that blocks the isolate
until its own threads have finished calling back: the isolate would be
waiting for the function, the function for its threads, and the threads
for the isolate. `ffi.threaded()` gives such a function a variant that
runs on a helper thread while the isolate keeps answering:

```zuri
import ffi

var c = ffi.open(ffi.LIBC).declare('
  typedef unsigned long pthread_t;
  int pthread_create(pthread_t *thread, const void *attributes,
                     void *(*start)(void *), void *argument);
  int pthread_join(pthread_t thread, void **result);
')

var seen = []
var start = ffi.callback(@(argument) {
  seen.append(argument.address())
  return nil
}, ffi.function_type(ffi.ptr, [ffi.ptr]))

var thread = ffi.alloc(ffi.ulong)
c.pthread_create(thread, nil, start, ffi.at(42))

var join = ffi.threaded(c.pthread_join)
join(thread.get(), nil)

echo seen
start.release()
```

```console
[42]
```

The callback ran on the isolate's own thread, in the middle of the
threaded call. A program with nothing else to do while it waits for calls
from another thread can wait in `ffi.serve(timeout)`, which answers
whatever is posted and returns how many it answered.

## Variadic Functions

A variadic function, `printf` and its family, takes its fixed parameters
by type and any number after them. Each argument past the fixed ones
travels as the type its value suggests:

| Value | Travels as |
| --- | --- |
| a whole number that fits an `int` | `int` |
| a larger whole number | `long long` |
| any other number | `double` |
| a bigint | `long long`, or `unsigned long long` when it needs to be |
| a bool | `int` |
| a string | `const char *` |
| a pointer, bytes or `nil` | `void *` |

Anything else is given its type with `Type.of()`, and C's promotions still
apply on top: a `float` travels as a `double`, and a type narrower than
`int` as an `int`.

```zuri
import ffi

var snprintf = ffi.open(ffi.LIBC).function('snprintf', ffi.int,
  [ffi.pointer(ffi.char), ffi.size_t, ffi.string], { variadic: true })

var buffer = ffi.alloc(ffi.char, 64)

snprintf(buffer, 64, '%s has %d items at %.2f each', 'cart', 3, 4.5)
echo buffer.read_string()

snprintf(buffer, 64, '%.1f, %ld, %c', ffi.double.of(2), ffi.long.of(-7), ffi.char.of('z'))
echo buffer.read_string()
```

```console
cart has 3 items at 4.50 each
2.0, -7, z
```

The second call shows why `of()` exists: `%.1f` with a bare `2` would pass
an `int`, and `printf` would read a double that was never there.

## Errors and errno

The module's own errors all descend from `FfiError`:

| Class | Raised when |
| --- | --- |
| `FfiError` | a type cannot be used as asked: a record with no size passed by value, a signature libffi cannot call |
| `LoadError` | a library cannot be found or loaded, or a function is called after its library was closed |
| `SymbolError` | a library does not export a symbol |
| `DeclarationError` | C or Rust source cannot be read; `line` and `column` point at the problem |
| `PointerError` | memory would be accessed out of bounds, through null, after being freed, or freed twice |
| `CallbackError` | a released callback is used |
| `LinkError` | a static library cannot be linked |

A value that does not convert to its C type raises the prelude's
`TypeError` or `RangeError`, the same errors any function raises for a bad
argument, and the message names the function, the argument and the type.

### errno

C reports failure through `errno`, which anything that runs afterwards may
change. So `errno` is cleared immediately before every foreign call and
read immediately after it, and `ffi.errno()` reports what the most recent
call on this isolate left there:

```zuri
import ffi

var strtol = ffi.open(ffi.LIBC).function('strtol', ffi.long,
  [ffi.string, ffi.ptr, ffi.int])

echo strtol('123', nil, 10)
echo ffi.errno()

strtol('99999999999999999999999', nil, 10)
echo ffi.errno()
```

```console
123
0
34
```

34 is `ERANGE`. On Windows, `ffi.last_error()` reports `GetLastError()` the
same way; it is always zero elsewhere. `ffi.set_errno()` sets `errno` for
the rare function that reads it.

## Ownership and Lifetimes

Three kinds of memory cross the boundary, and each has one owner.

**Memory from `ffi.alloc()`** belongs to the program. It is freed when the
last pointer into it is collected, or earlier with `free()`. Pointers made
from it with `add()`, `offset()`, `cast()` and `field()` share it and keep
it alive. The one rule is that a pointer must stay reachable for as long
as C holds the address; memory C was given and Zuri forgot is memory freed
under C's feet.

**Memory from `ffi.malloc()`** belongs to whoever frees it. It is never
freed on collection, because the usual reason to allocate it is to hand
it to C code that frees it with `free()`. `free()` releases it otherwise.

**Memory C allocated** belongs to C until the program takes it over with
`own()`, naming the function that releases it, or with no function for
memory the C allocator's `free()` releases:

```zuri
import ffi

var c = ffi.open(ffi.LIBC).declare('
  char *strdup(const char *text);
  void free(void *pointer);
')

var copy = c.strdup('owned by Zuri now').own(c.free)
echo copy.read_string()
echo copy.is_owned()

copy.free()
echo copy.is_freed()
```

```console
owned by Zuri now
true
true
```

An owned pointer is released when it is collected, or with `free()`; every
pointer derived from it refuses access afterwards. A destructor is any
foreign function of one pointer: `sqlite3_close`, `png_destroy`,
`CString::from_raw` wrapped in an `extern "C"` function. Only a pointer to
the start of an allocation can free it.

The collector runs a destructor in the middle of its own work, where no
Zuri code can run, so a destructor that calls a callback during a
collection gets zero back, or the callback's `error_value`, every time. `free()` runs the
destructor from the program, where callbacks work as they do anywhere
else.

## Static Libraries

A static library is object code waiting for a linker; nothing can load it
at run time as it stands. `ffi.link()` hands it to the platform's linker,
which links every object in it into a shared library, and loads that:

```zuri,ignore
var geometry = ffi.link('build/libgeometry.a', { libraries: ['m'] })
var rust = ffi.link('shapes/target/release/libshapes.a')
```

The linker is the C compiler, `cc` or whatever `$CC` names, on Linux and
macOS, and MSVC's `link.exe` on Windows, found through the Visual Studio
installation. The result is cached under a name derived from the archives'
contents and the options, in `ffi.default_link_cache()` or the `cache`
option's directory, so the linker runs once for a given input and every
later program start loads the cached library.

A Rust `staticlib` carries the Rust standard library with it, which in
turn needs a handful of system libraries. An archive holding Rust code is
recognised and linked against them without being asked.

On Windows, a static library's functions are not marked for export, so
the linked DLL exports every symbol the archives define with an
unmangled name, or exactly the ones listed in the `exports` option.

`libraries`, `search_paths` and `flags` pass further libraries, their
directories and raw arguments to the linker, and `linker` replaces it.
A failure raises `LinkError` carrying the linker's own output.

## Isolates

Types, pointers, libraries, foreign functions, callbacks and declarations
all cross to another isolate, and none of them is moved: both sides keep a
working handle on the same thing. A type is a description, a library a
handle the loader shares across threads, and a pointer an address, so
memory one isolate writes, another reads:

```zuri
import ffi
import isolate

def fill(memory, value) {
  memory.set(0, value)
  return memory.get(0)
}

var shared = ffi.alloc(ffi.int, 1)
echo isolate.spawn(fill, shared, 99).join()
echo shared.get(0)
```

```console
99
99
```

Whatever the memory holds is shared without any synchronisation, as it
is between C threads. A callback belongs to the isolate that made it:
passed elsewhere, its calls still run on that isolate.

## Rust in Depth

Every Rust type that has a defined C ABI crosses, and the rest are refused
with the reason.

| Rust | Crosses as |
| --- | --- |
| `i8` ... `u128`, `isize`, `usize` | integers, 128-bit ones by value included |
| `f32`, `f64` | numbers |
| `bool` | a bool |
| `char` | a one-character string; checked to be a Unicode scalar value |
| `*const T`, `*mut T` | a `Pointer`, `nil` for null |
| `&T`, `&mut T`, `NonNull<T>`, `Box<T>` | a `Pointer`; `nil` is refused going in |
| `Option<&T>`, `Option<NonNull<T>>`, `Option<Box<T>>` | a `Pointer` or `nil` |
| `extern "C" fn(...)` | a function pointer: a Zuri function or a `Callback` in, a callable out |
| `Option<extern "C" fn(...)>` | the same, or `nil` |
| `NonZeroU32` and the other `NonZero` types | a number; zero is refused |
| `Option<NonZeroU32>` | a number, or `nil` for `None` |
| `[T; N]` | a list |
| `#[repr(C)]` struct | a dictionary |
| `#[repr(transparent)]` struct | its field |
| `#[repr(C)]` or `#[repr(u8)]` enum without fields | a number, or a variant name going in |
| `#[repr(C)]`, `#[repr(C, u8)]` or `#[repr(u8)]` enum with fields | a dictionary with a `variant` key |
| `MaybeUninit<T>`, `ManuallyDrop<T>`, `Cell<T>` | as `T` |
| `PhantomData<T>` | nothing; it takes no space |

An enum with fields is laid out by the rules those representations
define. A value names its variant, and its fields are the rest of the
dictionary; a tuple variant's fields are `'0'`, `'1'` and so on, and a
variant without fields may be passed as just its name:

```zuri,ignore
shapes.area({ variant: 'Circle', radius: 2 })
shapes.area({ variant: 'Rect', w: 2, h: 3 })
tokens.value({ variant: 'Number', '0': 42 })
tokens.value('Plus')
```

A slice crosses as the pointer and the length Rust's own FFI convention
pairs it into; `ffi.slice(type)` builds that `#[repr(C)]` struct:

```zuri,ignore
var Slice = ffi.slice(ffi.i32)
var values = ffi.alloc(ffi.i32, 3)
lib.sum_slice({ ptr: values, len: 3 })
```

Refused, each with the reason: `&str`, `&[T]`, `String`, `Vec`, and every
other type without a stable layout; trait objects; tuples; generic types
and functions; `Option` of a type without a niche; a function without
`#[no_mangle]` or `#[export_name]`; and the `extern "Rust"` ABI.

A Rust function declared `extern "C"` aborts the process if it panics,
which is Rust's own rule for that ABI, and a panic never reaches Zuri.
`extern "C-unwind"` functions are called the same way; a panic unwinding
out of one likewise ends the process, because nothing can unwind safely
through a Zuri frame.

## Platform Differences

The module runs on 64-bit little-endian platforms, x86-64 and Arm, on
Linux, macOS and Windows, and follows each one's C compiler:

| | Linux x86-64 | Linux Arm | macOS x86-64 | macOS Arm | Windows x86-64 |
| --- | --- | --- | --- | --- | --- |
| `long` | 8 | 8 | 8 | 8 | 4 |
| `char` | signed | unsigned | signed | signed | signed |
| `wchar_t` | 4, signed | 4, unsigned | 4, signed | 4, signed | 2, unsigned |
| `long double` | 80-bit | 128-bit | 80-bit | 64-bit | 64-bit |
| `_Complex` by value | yes | yes | yes | yes | no |
| enum past `int` | wider type | wider type | wider type | wider type | `int`, truncated |
| bitfield rules | GCC | GCC | Clang | Clang | MSVC |

`ffi.platform()` reports these for the running platform. A program that
passes records between platforms through files or sockets gets the same
layouts the C compiler on each one produces, which is what the C code
there expects.

Calling conventions follow the platform too: System V on Linux and macOS
x86-64, AAPCS64 on Arm, with Apple's variations on it, and the Microsoft
x64 convention on Windows. On x86-64, a function type or `Library.function()`
can ask for `abi: 'win64'`, and on Unix `abi: 'sysv64'`, for a function
compiled for the other convention; declarations read `ms_abi` and
`sysv_abi` attributes and Rust's `extern "win64"` and `extern "sysv64"`.

128-bit integers cross by value under every one of them, in both
directions. The Microsoft x64 convention passes one by reference and
returns it in a vector register, as rustc, Clang and GCC compile it, so a
callback returning `i128` works there as it does everywhere else.

## What Is Checked

A foreign call runs code the module cannot see into, so the guarantees
stop where C begins. Inside them:

- every value is converted to its declared type, with integers checked
  against their range and every other kind against its type;
- memory the module allocated is bounds-checked and refuses use after
  `free()`; freeing twice is an error;
- a null pointer is caught before it is read or written through;
- a failed write changes nothing;
- a Zuri error inside a callback never unwinds through C, and a Rust panic
  inside the module never reaches C;
- a callback called from any thread runs on its own isolate.

Beyond them, what C does is C's. A description that disagrees with the
library, a pointer read past what the library says is there, a callback C
calls after it was released, bytes resized by a callback while C writes to
them: each of these is undefined behaviour in C, and it stays so here.
Reading declarations from the library's own header or crate, rather than
writing them by hand, is the surest way to keep the first of these away.

## What the Module Refuses

**It does not run the C preprocessor in full.** Macros that define values
and conditional blocks are evaluated; including other files and expanding
function-like macros are not, and are refused where they appear.

**It does not compile C.** Static libraries are linked by the platform's
own linker, which needs a C toolchain on the machine that links them.

**It does not guess ownership.** A pointer C returns is not freed until
the program says how, with `own()`.

**It does not free a callback on its own.** A callback lives until
`release()`, because only the program knows when the library is done
with it.

**It does not unwind through foreign frames.** Errors are trapped and
raised again once C has returned; a panic crossing the boundary ends the
process, as it does in Rust.

## Module Reference

The standard library reference documents every class and method. The
shape of the module:

| | |
| --- | --- |
| `ffi.open(name, options)` | loads a library, or the process |
| `ffi.find(name, paths)` | where a library would load from |
| `ffi.link(archives, options)` | links static libraries and loads the result |
| `ffi.declare(source)`, `ffi.declare_rust(source)`, `ffi.declarations()` | sets of declarations |
| `ffi.struct(name)`, `ffi.union(name)`, `ffi.enum(name, type)` | records and enums, built by hand |
| `ffi.pointer(type, options)`, `ffi.array(type, length)`, `ffi.function_type(returns, params, options)`, `ffi.slice(type)`, `ffi.type(spelling)` | other types |
| `ffi.alloc(type, count)`, `ffi.alloc_bytes(size)`, `ffi.malloc(size)`, `ffi.alloc_string(text)`, `ffi.at(address, type)` | memory |
| `ffi.function(pointer, type)`, `ffi.threaded(function)`, `ffi.describe(function)`, `ffi.is_foreign(value)` | foreign functions |
| `ffi.callback(function, type, options)`, `ffi.serve(timeout)` | callbacks |
| `ffi.errno()`, `ffi.set_errno(n)`, `ffi.last_error()` | error codes |
| `ffi.platform()`, `ffi.default_link_cache()` | facts about the platform |
| `ffi.LIBC`, `ffi.LIBM` | the C runtime and maths libraries |

On a `Library`: `function`, `variable`, `symbol`, `has`, `declare`,
`declare_rust`, `bind`, `path`, `close`, `is_closed`.

On a `Pointer`: `get`, `set`, `read`, `write`, `get_field`, `set_field`,
`field`, `read_string`, `write_string`, `read_bytes`, `write_bytes`,
`to_list`, `add`, `offset`, `cast`, `copy_from`, `fill`, `compare`,
`own`, `free`, `is_owned`, `is_freed`, `address`, `is_null`, `type`,
`size`, `equals`.

On a `Type`: `name`, `kind`, `size`, `align`, `is_const`, `target`,
`length`, `signature`, `pointer`, `array`, `as_const`, `of`, `equals`; on
a `StructType` or `UnionType` also `add_field`, `add_bitfield`,
`set_packed`, `set_align`, `fields`, `offset_of`, `has_field`, `variants`;
on an `EnumType`, `add_constant`, `constants`, `value`.

On a `Declarations`: `declare`, `declare_rust`, `include`, `type`,
`constant`, `constants`, `types`, `functions`, `variables`, `bind`.

On a `Callback`: `pointer`, `type`, `release`, `is_released`.
