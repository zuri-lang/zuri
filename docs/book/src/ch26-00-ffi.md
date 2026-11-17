# Foreign Functions: C and Rust

The `ffi` module calls code written in C and Rust. It loads shared
libraries, describes the functions in them and calls those functions with
ordinary Zuri values. It turns Zuri functions into C function pointers so
that a library can call back into your code. And it reads the declarations
that a header file or a Rust crate already contains, so the description only
has to be written once, by the people who wrote the library.

Every value that crosses the boundary is converted and checked against its C
type. An integer that doesn't fit is a `RangeError` at the call, not a
truncated argument inside the library. Memory allocated by the module knows
its own size, and a read past its end is rejected before it happens. When a
check isn't possible, because C returned an address without saying how much
memory is behind it, the module tells you so instead of guessing.

You can use C libraries, Rust `cdylib` libraries, and static libraries
written in either language. The layout of every struct, whether it's packed,
over-aligned or full of bitfields, is the same as the platform's own
compiler would give it.

- [Following Along](#following-along)
- [Introduction](#introduction)
- [Loading a Library](#loading-a-library)
- [Calling a Function](#calling-a-function)
- [Types](#types)
- [Numbers Across the Boundary](#numbers-across-the-boundary)
- [Declaring Functions From C](#declaring-functions-from-c)
- [Declaring Functions From Rust](#declaring-functions-from-rust)
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
- [Design Limits](#design-limits)
- [Module Reference](#module-reference)

> Code blocks in this chapter that list several calls together are
> **reference listings**, not programs. They show the form of each call, not
> a sequence to run. Anything shown as a complete program runs as written.
> The programs that call the C runtime run on Linux as shown, and the ones
> that need a library of your own are shown but not run.

## Following Along

Every machine has the C runtime library, so most of the examples below call
it. `ffi.LIBC` names it for the platform the program is running on, and
`ffi.LIBM` names the math library:

```zuri
import ffi

var libc = ffi.open(ffi.LIBC)
var strlen = libc.function('strlen', ffi.size_t, [ffi.string])

echo strlen('Hello, C')
```

```console
8
```

Some sections use a small library of your own. Save this as `geometry.c`:

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

Then build it as a shared library with the C compiler:

```sh
cc -shared -fPIC geometry.c -o libgeometry.so -lm       # Linux
cc -dynamiclib geometry.c -o libgeometry.dylib          # macOS
cl /LD geometry.c                                       # Windows
```

The Rust sections use a crate built with `crate-type = ["cdylib"]`, which is
shown where it's used.

## Introduction

Sooner or later, every language needs to use code it didn't write, such as a
database engine, a codec, a cryptography library, a scientific computing
kernel, or a system call that the standard library doesn't wrap. That code
already exists, it's fast and well tested, and it uses the C calling
convention, which is the one convention every language agrees on, on every
platform. Rust libraries can use it too, through `extern "C"`.

Calling across that boundary means describing exactly what the other side
expects: how wide each integer is, where each member of a struct is stored,
and which register each value is passed in. The machine doesn't check any of
this. A description that's off by a single byte silently corrupts memory,
and the bug shows up later, somewhere unrelated.

`ffi` turns that description into a value your program can inspect. It
builds the description from the source code the library's authors already
wrote, lays out every type the same way the platform's compiler does, and
checks every value against it as the value crosses the boundary:

```zuri
import ffi

var c = ffi
    .open(ffi.LIBC)
    .declare(
      '
  int abs(int n);
  typedef struct { int quot; int rem; } div_t;
  div_t div(int numerator, int denominator);
',
    )

echo c.abs(-7)
echo c.div(17, 5)
```

```console
7
{quot: 3, rem: 2}
```

The declarations are the same ones `<stdlib.h>` contains. The struct was
returned as a dictionary, and the conversion rules were enforced for the
arguments:

```zuri,ignore
c.abs(3000000000)
```

```console
RangeError: abs() argument 1 'n': 3000000000 is out of range for 'int', which holds -2147483648 to 2147483647
```

## Loading a Library

`ffi.open()` loads a shared library and returns a `Library`. It accepts a
path, a file name, or a bare name that it completes following the platform's
conventions:

```zuri,ignore
var sqlite = ffi.open('sqlite3') # libsqlite3.so, libsqlite3.dylib, sqlite3.dll
var local = ffi.open('./build/libgeometry.so') # a path, as given
var vendored = ffi.open('geometry', {
  paths: ['vendor/lib'],
})
```

A bare name is looked for first in the directories given in `paths`, and
then in the platform's own search order. On Linux, a library's unversioned
`libname.so` file is usually only installed with its development package, so
`ffi.open()` also asks the dynamic loader's cache for the versioned file
that the runtime package installs, such as `libsqlite3.so.0`. On macOS, the
name is tried as a `.dylib` and as a framework.

`ffi.find()` looks for a library the same way, without loading it:

```zuri
import ffi

echo ffi.find('zuri_has_no_such_library')
```

```console
nil
```

Without a name, `ffi.open()` returns the running process, whose symbols
include the C runtime library and everything else that's already loaded
globally.

Two options change how a library is loaded. `lazy: true` resolves its
symbols when they're first used, instead of all at once. `global: true`
makes its symbols visible to libraries loaded after it, and to the process
handle, which a plugin that relies on its host's symbols needs. Both options
are Unix concepts and have no effect on Windows, where every loaded module
is searched whenever the process is asked for a symbol.

If a library can't be found or loaded, `ffi.open()` raises a `LoadError`
that includes the loader's own explanation, such as a missing dependency, a
library built for a different architecture, or a file that isn't a library.

### Symbols

A `Library` can tell you whether it exports a symbol, and where:

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
requests a particular version of a versioned symbol, for the rare program
that needs to pin one.

### Closing a Library

`close()` stops a library from being used, so every function bound from it
raises a `LoadError` from then on. The code itself is only unloaded once
nothing refers to the library anymore, including the functions bound from
it, so a closed library is never unloaded while a call is still running.

## Calling a Function

You bind a function by giving its name, its return type and its parameter
types, and you get back an ordinary Zuri function:

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

You can store it, pass it to `map()`, spawn it in an isolate and call it
like any other function. The number of arguments is checked just as it is
for any function:

```zuri,ignore
pow(2)
```

```console
ArgumentError: 'pow' expects 2 arguments, got 1
```

`ffi.describe()` describes a foreign function, and `ffi.is_foreign()` tells
a foreign function apart from a Zuri one:

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

Binding one function at a time works well for a few functions. For more than
that, you can read the declarations from a header, as the next sections
show.

## Types

Every C type is represented by a `Type`. The module exports the built-in
types:

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
| `ffi.size_t`, `ffi.ssize_t`, `ffi.ptrdiff_t` | The same names | 8 |
| `ffi.intptr_t`, `ffi.uintptr_t` | The same names | 8 |
| `ffi.wchar_t`, `ffi.char16_t`, `ffi.char32_t` | The same names | 4 (2 on Windows), 2, 4 |
| `ffi.float`, `ffi.double` | The same names | 4, 8 |
| `ffi.longdouble` | `long double` | 16, or 8 on Windows and Apple Arm |
| `ffi.complex_float`, `ffi.complex_double` | `float _Complex`, `double _Complex` | 8, 16 |
| `ffi.ptr` | `void *` | 8 |
| `ffi.string` | `const char *`, as text | 8 |
| `ffi.wstring` | `const wchar_t *`, as text | 8 |

The module also exports Rust's names for the same types: `ffi.i8` to
`ffi.u128`, `ffi.isize`, `ffi.usize`, `ffi.f32`, `ffi.f64`, and
`ffi.rust_char` for Rust's `char`. A Rust name and its C equivalent refer to
the same type:

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

A type knows its size, alignment and kind on the current platform:

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

### Building Types

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

`ffi.type()` reads the C name of a built-in type, which is often the
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

Structs, unions and enums are built member by member, as described in
[Structs, Unions and Arrays](#structs-unions-and-arrays) and [Enums and
Constants](#enums-and-constants).

## Numbers Across the Boundary

A Zuri number is a double, which can represent every integer up to 2^53
exactly, but no further. C integer types go up to 2^64, and with `__int128`,
2^128. So integers cross the boundary like this:

- **Going in**, a number must be a whole number that fits in the type, and
  you can pass a bigint for any integer type. A fraction, a string or a bool
  passed for an integer raises a `TypeError`, and a value outside the type's
  range raises a `RangeError`. Values are never truncated or wrapped around.
- **Coming out**, an integer is returned as a number if it's within 2^53 of
  zero, and as a bigint otherwise, so no value is ever rounded.

```zuri
import ffi

var strtoull = ffi.open(ffi.LIBC).function('strtoull', ffi.uint64, [
  ffi.string,
  ffi.ptr,
  ffi.int,
])

echo strtoull('42', nil, 10)
echo strtoull('18446744073709551615', nil, 10)
echo typeof(strtoull('18446744073709551615', nil, 10))
```

```console
42
18446744073709551615n
bigint
```

When a value could be either type, your program has to handle both, for
example with `is_bigint(value)` or by comparing against a bigint, which also
compares correctly with a number.

Floating-point types accept any number. `long double` is wider than a double
on some platforms: it's 80-bit extended precision on x86-64 Linux and macOS,
and 128-bit quad precision on Arm Linux. Values going in are converted
exactly, and values coming out are rounded to the nearest double, as C's own
conversion does.

`bool` accepts and returns a Zuri bool, and a C character type accepts a
number or a one-character string:

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

A complex number is represented as a list of two numbers,
`[real, imaginary]`. You can pass one by value wherever the C compiler
supports `_Complex`, which is everywhere except Windows.

## Declaring Functions From C

`ffi.declare()` reads C declarations and returns a `Declarations` object,
which is a set of types, functions, variables and constants that isn't tied
to any particular library. Binding it to a library produces a namespace:

```zuri
import ffi

var api = ffi.declare(
  '
  typedef struct { long quot; long rem; } ldiv_t;
  ldiv_t ldiv(long numerator, long denominator);
  long labs(long n);
',
)

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

`Library.declare()` does both steps at once, and it's what most programs
use.

The namespace contains a function for each declared function, a `Pointer`
for each declared variable, each constant, and each type that has a name of
its own. You use it in the same way as a module. A tagged type without a
typedef, such as `struct stat`, is accessed through the declarations
instead, as in `api.type('struct stat')`.

If a declared function isn't exported by the library, you get a
`SymbolError` that names every missing function. With `allow_missing: true`,
the other functions are bound and the missing ones are left out, which is
useful for a header that describes several versions of a library.

### What Is Read

Everything a header says about an API:

- `typedef`, `struct`, `union` and `enum`, including forward declarations,
  self-referencing records, nested records, anonymous members, bitfields and
  flexible array members
- Function prototypes, including variadic ones, and function pointer types
  nested to any depth
- `extern` variables
- `_Static_assert`, which is evaluated, so a header that checks its own
  assumptions checks them here too
- `__attribute__((packed))`, `aligned(n)`, `ms_abi` and `sysv_abi`,
  `__declspec(align(n))`, `_Alignas`, and `asm` labels that give a function
  a different symbol name. Other attributes are skipped.
- `extern "C" { ... }`, as written in headers that are shared with C++

A function with a body, such as an inline helper in a header, is read but
not bound, because the library doesn't have to export it.

```zuri
import ffi

var api = ffi.declare(
  '
  struct node;
  typedef struct node node;
  struct node { int value; node *next; };

  typedef struct {
    struct { double x, y; } origin;
    union { int id; char tag[8]; };
  } shape;

  _Static_assert(sizeof(shape) == 24, "shape is 24 bytes");
',
)

echo api.type('node').size()
echo api.type('shape').offset_of('tag')
echo api.type('shape').has_field('id')
```

```console
16
16
true
```

### The Preprocessor

Real headers contain a lot of preprocessor lines, so enough of the
preprocessor is implemented to read them as they're written:

- A `#define` of a value is expanded wherever the name appears, and also
  becomes a constant. Values can be numbers in any base with any suffix,
  floating-point numbers, character constants, strings, adjacent strings
  (which are joined), and expressions that use other constants. A value cast
  to a pointer type, as headers write sentinels such as `((void *) -1)`,
  becomes a `Pointer` of that type at that address, or `nil` when the
  address is zero. A macro can use a type that's declared after it, as C
  allows, since a macro only has a meaning where it's used.
- `#if`, `#ifdef`, `#ifndef`, `#elif`, `#else` and `#endif` are evaluated,
  along with `defined()`, using the macros that the target platform's
  compiler predefines, such as `__linux__`, `__APPLE__`, `_WIN32`,
  `__x86_64__`, `__aarch64__` and `__LP64__`. `__has_include()` and the
  other `__has_` queries always return false.
- `#pragma pack` applies to the records declared after it, exactly where
  it's written.
- `#undef` removes a macro, and `#error` stops with its message.
- Including a C standard header is allowed, because everything it declares
  is already known. Including `<stdio.h>` also declares `FILE`.

```zuri
import ffi

var api = ffi.declare(
  '
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
',
)

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

Two things are rejected, with the line and column of the problem. The first
is including any other file, because declarations are read exactly as given
and files are never fetched, so paste in the declarations the program needs.
The second is using a function-like macro, which would require the
preprocessor's full expansion rules. Defining a function-like macro is
allowed and recorded, and only using one is an error.

### Functions That Return Text

C has no way to say who owns a returned pointer, but it has a convention
that's clear enough to follow. A function returning `const char *` returns
text that it keeps, while one returning `char *` usually hands over memory
to the caller. So a declared function that returns `const char *` returns a
string, or `nil` for a null pointer, and one that returns `char *` returns a
`Pointer`, which your program reads and then frees. The same applies to
`const wchar_t *`.

```zuri
import ffi

var c = ffi
    .open(ffi.LIBC)
    .declare(
      '
  int setenv(const char *name, const char *value, int overwrite);
  const char *getenv(const char *name);
',
    )

c.setenv('ZURI_FFI_DEMO', 'hello', 1)
echo c.getenv('ZURI_FFI_DEMO')
echo c.getenv('ZURI_FFI_NOT_SET')
```

```console
hello
nil
```

In the real header, `getenv` is declared as `char *getenv(const char *)`.
Declaring it as `const char *` here is the program's way of saying that it
won't free the result, which is correct.

### Extending and Sharing Declarations

You add sources to a set one after another, and each one can see what came
before it. A set can include another set and use its types without owning
them, which is how two libraries can share one set of common types:

```zuri
import ffi

var common = ffi.declare('typedef struct { double x, y; } point;')

var shapes = ffi
    .declarations()
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
contains, and `type()` resolves any C type name against it, as in
`shapes.type('segment *[2]')`.

## Declaring Functions From Rust

Rust libraries are called through the C ABI they export, meaning
`extern "C"` functions and the `#[repr(C)]` types they use.
`ffi.declare_rust()` reads those declarations the way a crate writes them,
so you can pass it the crate's source code as it is. Given this `lib.rs`,
built with `crate-type = ["cdylib"]`:

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

The Zuri side reads the same file:

```zuri,ignore
import ffi

var source = file('shapes/src/lib.rs').read()
var shapes = ffi
    .open('shapes', {
      paths: ['shapes/target/release'],
    })
    .declare_rust(source)

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

Here's what's read and what's skipped:

- Functions in `extern "C"` blocks (including `unsafe extern "C"` blocks),
  with `#[link_name]` for a different symbol name
- `extern "C" fn` definitions marked with `#[no_mangle]` or
  `#[export_name]`, with their bodies skipped. One without either attribute
  is rejected, because it's exported under a mangled name that nothing can
  look up.
- `#[repr(C)]`, `#[repr(C, packed)]`, `#[repr(C, align(N))]` and
  `#[repr(transparent)]` structs, tuple structs and unions
- Enums with `#[repr(C)]` or an integer `#[repr]`, with or without fields
- `type` aliases, `const` items and statics
- `#[cfg(...)]`, which is evaluated for the platform the program is running
  on
- Everything else, from `use` statements to `impl` blocks, macros and
  private functions, is skipped

A type can be used before it's declared, as Rust allows. A struct without
`#[repr(C)]` can only be used through a pointer, because Rust doesn't
specify its layout. [Rust in Depth](#rust-in-depth) explains how each Rust
type crosses the boundary.

C and Rust declarations can go in the same set, and each can use the other's
types, which suits a crate that includes a C header alongside its source
code.

## Pointers and Memory

A `Pointer` is an address, optionally with the type of the value it points
to. You get memory from `ffi.alloc()`, which returns it zeroed and typed:

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

`get()` and `set()` read and write elements of the pointer's type, as
`ptr[i]` does in C, and `add()` moves forward by a number of elements, as
`ptr + i` does in C. `read()` and `write()` take their own type and a byte
offset, and work on any pointer:

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

Memory is little-endian on every platform the module supports, as the third
line shows. `cast()` gives a pointer a type, or removes its type.

### Checked Access

Memory allocated by the module knows its size, and every access through a
pointer into it is checked against that size before it happens:

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

If a write fails to convert its value, nothing is written, so a record is
never left half updated. Freed memory rejects every access, and freeing
memory twice raises an error instead of causing a double free.

A pointer returned by C code doesn't have a size, because C didn't say how
much memory is behind it. The only thing that can be caught through such a
pointer is a null pointer. How much memory is actually there depends on what
the library's documentation says.

### Passing Pointers as Arguments

A pointer parameter accepts more than just a `Pointer`. Each of these is
converted for the duration of the call:

| Passed | Becomes |
| --- | --- |
| `nil` | A null pointer |
| A `Pointer` | Its address |
| A string | A NUL-terminated copy, for a pointer to characters |
| Bytes | The bytes' own storage, without copying |
| A list | A temporary array of the target type, copied back after the call |
| A dictionary | A temporary record, copied back after the call |
| A Zuri function | A callback, for a function pointer |
| A `Callback` | Its code |
| A foreign function | Its address |

A list or dictionary is only copied back when the pointer isn't to a `const`
type, so you can use it as an output parameter:

```zuri
import ffi

var frexp = ffi.open(ffi.LIBM).function('frexp', ffi.double, [
  ffi.double,
  ffi.pointer(ffi.int),
])

var exponent = [0]
echo frexp(48, exponent)
echo exponent[0]
```

```console
0.75
6
```

Bytes are passed using their own storage, so a C function that fills a
buffer fills the bytes directly:

```zuri
import ffi

var memset = ffi.open(ffi.LIBC).function('memset', ffi.ptr, [
  ffi.ptr,
  ffi.int,
  ffi.size_t,
])

var buffer = bytes(6)
memset(buffer, 65, 4)
echo buffer
```

```console
(41 41 41 41 00 00)
```

A callback mustn't resize a bytes value that's been passed to a call while
the call is running. Its storage is only guaranteed to stay in the same
place as long as nothing changes its length.

### Other Kinds of Allocation

`ffi.alloc_bytes(size, align)` allocates untyped memory. `ffi.malloc()`
allocates memory from the C allocator, for memory you'll pass to C code that
frees it with `free()`, so it's never freed when the pointer is garbage
collected. `ffi.at(address, type)` creates a pointer from a known address.
`copy_from()`, `fill()` and `compare()` are the equivalents of `memmove`,
`memset` and `memcmp` for checked memory.

## Strings and Text

A string passed where a pointer to characters is expected becomes a
NUL-terminated copy for the duration of the call, using the encoding implied
by the character type: UTF-8 for `char`, UTF-16 for `char16_t`, UTF-32 for
`char32_t`, and the platform's own encoding for `wchar_t`, which is UTF-16
on Windows and UTF-32 everywhere else.

`ffi.string` and `ffi.wstring` are pointer types that also convert values
coming back. A function declared to return one of them returns a Zuri
string, or `nil` for a null pointer.

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

Text that needs to last longer than a call, because it's stored in a struct
or kept by the library, has to be put in memory of its own with
`ffi.alloc_string()`:

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
code units, in any of the four encodings, and raises a `ValueError` for text
that isn't valid in its encoding. `write_string()` writes text followed by
its terminator. If a read in a block of known size finds no terminator, it's
rejected instead of reading past the end.

## Structs, Unions and Arrays

A record is built one member at a time, and laid out in the same way the
platform's C compiler lays it out:

```zuri
import ffi

var Header = ffi
    .struct('Header')
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

A record is sealed the first time anything needs its size, including passing
a value of that type, and can't be changed after that. `set_packed()` packs
it as `#pragma pack` does, `set_align()` increases its alignment, and
`add_field()` accepts a minimum alignment for a single member, as `_Alignas`
does:

```zuri
import ffi

var Packed = ffi
    .struct('Packed')
    .add_field('tag', ffi.uint8)
    .add_field('length', ffi.uint32)
    .set_packed()
var Wide = ffi.struct('Wide').add_field('tag', ffi.uint8).set_align(64)

echo Packed.size()
echo Wide.size()
```

```console
5
64
```

### Record Values

A record value is represented as a dictionary of its members, both going in
and coming out. A member that's missing from a dictionary going in is set to
zero. Nested records are nested dictionaries, and array members are lists,
except that an array of `char` is read as the string it contains, and an
array of other bytes is read as bytes:

```zuri
import ffi

var Item = ffi
    .struct('Item')
    .add_field('name', ffi.char.array(8))
    .add_field('counts', ffi.int.array(3))

var item = ffi.alloc(Item)
item.set(0, {
  name: 'widget',
  counts: [1, 2],
})

echo item.get()
echo item.get_field('counts')
```

```console
{name: widget, counts: [1, 2, 0]}
[1, 2, 0]
```

Through a pointer, `get_field()` and `set_field()` read and write a single
member, as `ptr->name` does, and `field()` returns a pointer to a member, as
`&ptr->name` does. `fields()` lists the members along with their offsets.

### Bitfields

`add_bitfield()` adds a member with a given width in bits. Bitfields follow
the platform's rules for packing them, which are GCC's and Clang's on Linux
and macOS and MSVC's on Windows, including a zero-width bitfield that ends a
storage unit:

```zuri
import ffi

var Flags = ffi
    .struct('Flags')
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

A union value going in is a dictionary that holds a single member, the one
to store. Coming out, it contains every member, each read from the same
bytes, because only your program knows which one is meaningful:

```zuri
import ffi

var Bits = ffi
    .union('Bits')
    .add_field('f', ffi.float)
    .add_field('u', ffi.uint32)
var value = ffi.alloc(Bits)
value.set(0, { f: 1 })

echo value.get()
```

```console
{f: 1, u: 1065353216}
```

A union that's been read back and passed in again is accepted as it is,
because its members agree with each other. If the members disagree, a
`TypeError` is raised.

### Passing Records by Value

Records can be passed to and returned from functions by value, whatever
their shape. Which registers a record is passed in depends on its size and
the types of its members, and differs on every platform. Those rules are
applied to the record's actual layout, including packed and over-aligned
records and bitfields:

```zuri,ignore
var geometry = ffi
    .open('geometry')
    .declare(
      '
  typedef struct { double x, y; } point;
  double distance(point a, point b);
  point midpoint(const point *a, const point *b);
  void scale_all(point *points, size_t n, double k);
',
    )

echo geometry.distance({ x: 0, y: 0 }, { x: 3, y: 4 })
echo geometry.midpoint({ x: 0, y: 0 }, { x: 4, y: 2 })

var points = [
  { x: 1, y: 1 },
  { x: 2, y: 3 },
]
geometry.scale_all(points, 2, 10)
echo points
```

```console
5
{x: 2, y: 1}
[{x: 10, y: 10}, {x: 20, y: 30}]
```

`midpoint` takes pointers to records, so the dictionaries were passed as
temporary records. `scale_all` takes a pointer to an array of records, so a
list of dictionaries was passed as a temporary array and copied back
afterwards.

## Enums and Constants

An enum is an integer type with named constants. A value of an enum type is
a number, and wherever one is passed in, you can use a constant's name
instead:

```zuri
import ffi

var Level = ffi.enum('Level').add_constant('LOW', 1).add_constant('HIGH', 10)

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

Declared enums include their constants. A declared enum is stored as an
`int` unless its values need more space, it specifies a type (as in
`enum x : uint8_t`), or GCC's `packed` attribute asks for the smallest
possible type. Every enum constant, `#define` constant and Rust `const`
becomes a constant of the declarations, and a member of the namespace
they're bound to.

## Callbacks

A Zuri function passed where a function pointer is expected becomes a C
function pointer for the duration of that call:

```zuri
import ffi

var c = ffi
    .open(ffi.LIBC)
    .declare(
      '
  void qsort(void *base, size_t count, size_t size,
             int (*compare)(const void *, const void *));
',
    )

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

The arguments are converted from their C types, with pointers becoming
`Pointer` values and records becoming dictionaries, and the return value is
converted to the callback's return type. A small integer is widened to a
full register, the same way a C compiler returns one.

### Long-Lived Callbacks

A library that stores a function pointer and calls it later, such as a
signal handler, an event callback or a logging hook, needs a callback that
lasts longer than the call that passed it. `ffi.callback()` creates one:

```zuri,ignore
var on_event = ffi.callback(@(code) {
  echo 'event ${code}'
}, ffi.function_type(ffi.void, [ffi.int]))

library.set_handler(on_event)
```

A long-lived callback lasts until you call `release()`. Nothing else frees
it, because there's no way to know when the library has finished with the
pointer, so a callback that's never released lasts as long as the program.
After `release()`, the callback can no longer be passed anywhere, and C code
must not call it again.

### Errors Inside a Callback

A Zuri error can't unwind through C stack frames, because the C code in
between expects to finish. So an error raised inside a callback is caught, C
receives a zero (or the callback's `error_value`), and the error is raised
again, unchanged, as soon as the C function returns:

```zuri
import ffi

var c = ffi
    .open(ffi.LIBC)
    .declare(
      '
  void qsort(void *base, size_t count, size_t size,
             int (*compare)(const void *, const void *));
',
    )

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

The first error is the one that's raised. Any later calls to the callback
during the same C call return the error value.

## Callbacks and Threads

C libraries often run their own threads, and those threads can call
callbacks. A Zuri isolate runs on a single thread, so a call from any other
thread is passed to the isolate that created the callback, and the calling
thread waits until the isolate has run it. The isolate responds at its next
safepoint, which is the same point where it checks for signals, or
immediately if it's inside a foreign call or in `ffi.serve()`.

That covers a library whose threads call back while the program is busy with
something else. It doesn't cover a function that blocks the isolate until
its own threads have finished calling back. In that case, the isolate would
be waiting for the function, the function for its threads, and the threads
for the isolate. `ffi.threaded()` gives that kind of function a version that
runs on a helper thread, while the isolate keeps responding to callbacks:

```zuri
import ffi

var c = ffi
    .open(ffi.LIBC)
    .declare(
      '
  typedef unsigned long pthread_t;
  int pthread_create(pthread_t *thread, const void *attributes,
                     void *(*start)(void *), void *argument);
  int pthread_join(pthread_t thread, void **result);
',
    )

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

The callback ran on the isolate's own thread, partway through the threaded
call. A program that has nothing else to do while it waits for calls from
another thread can wait in `ffi.serve(timeout)`, which handles whatever
calls arrive and returns how many it handled.

## Variadic Functions

A variadic function, such as `printf` and its relatives, takes its fixed
parameters with declared types, followed by any number of other arguments.
Each argument after the fixed ones is passed as the type its value suggests:

| Value | Passed as |
| --- | --- |
| A whole number that fits in an `int` | `int` |
| A larger whole number | `long long` |
| Any other number | `double` |
| A bigint | `long long`, or `unsigned long long` if necessary |
| A bool | `int` |
| A string | `const char *` |
| A pointer, bytes or `nil` | `void *` |

For anything else, give the value a type with `Type.of()`. C's usual
promotions still apply on top of that, so a `float` is passed as a `double`,
and a type narrower than `int` is passed as an `int`.

```zuri
import ffi

var snprintf = ffi
    .open(ffi.LIBC)
    .function(
      'snprintf',
      ffi.int,
      [ffi.pointer(ffi.char), ffi.size_t, ffi.string],
      { variadic: true },
    )

var buffer = ffi.alloc(ffi.char, 64)

snprintf(buffer, 64, '%s has %d items at %.2f each', 'cart', 3, 4.5)
echo buffer.read_string()

snprintf(
  buffer,
  64,
  '%.1f, %ld, %c',
  ffi.double.of(2),
  ffi.long.of(-7),
  ffi.char.of('z'),
)
echo buffer.read_string()
```

```console
cart has 3 items at 4.50 each
2.0, -7, z
```

The second call shows why `of()` exists. `%.1f` with a plain `2` would pass
an `int`, and `printf` would read a double that was never passed.

## Errors and errno

All of the module's own errors are subclasses of `FfiError`:

| Class | Raised when |
| --- | --- |
| `FfiError` | A type can't be used in the way requested, such as a record without a size passed by value, or a signature that libffi can't call |
| `LoadError` | A library can't be found or loaded, or a function is called after its library was closed |
| `SymbolError` | A library doesn't export a symbol |
| `DeclarationError` | C or Rust source can't be read. `line` and `column` point to the problem. |
| `PointerError` | Memory would be accessed out of bounds, through a null pointer or after being freed, or freed twice |
| `CallbackError` | A released callback is used |
| `LinkError` | A static library can't be linked |

A value that can't be converted to its C type raises the usual `TypeError`
or `RangeError`, the same errors any function raises for an invalid
argument, and the message names the function, the argument and the type.

### errno

C reports failures through `errno`, which anything that runs afterwards
might change. So `errno` is cleared just before every foreign call and read
just after it, and `ffi.errno()` returns the value that the most recent call
on this isolate left there:

```zuri
import ffi

var strtol = ffi.open(ffi.LIBC).function('strtol', ffi.long, [
  ffi.string,
  ffi.ptr,
  ffi.int,
])

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

34 is `ERANGE`. On Windows, `ffi.last_error()` reports `GetLastError()` in
the same way, and it's always zero on other platforms. `ffi.set_errno()`
sets `errno`, for the rare function that reads it.

## Ownership and Lifetimes

Three kinds of memory cross the boundary, and each one has a single owner.

**Memory from `ffi.alloc()`** belongs to the program. It's freed when the
last pointer to it is garbage collected, or earlier, when you call `free()`.
Pointers created from it with `add()`, `offset()`, `cast()` and `field()`
share the same memory and keep it alive. The one rule to follow is that a
pointer must stay reachable for as long as C code holds the address. If C
was given some memory and Zuri lost track of it, that memory can be freed
while C is still using it.

**Memory from `ffi.malloc()`** belongs to whoever frees it. It's never freed
by the garbage collector, because the usual reason to allocate it is to pass
it to C code that frees it with `free()`. Otherwise, call `free()` to
release it.

**Memory allocated by C** belongs to C until your program takes ownership of
it with `own()`, passing the function that releases it, or no function for
memory that the C allocator's `free()` releases:

```zuri
import ffi

var c = ffi
    .open(ffi.LIBC)
    .declare(
      '
  char *strdup(const char *text);
  void free(void *pointer);
',
    )

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

An owned pointer is released when it's garbage collected, or when you call
`free()`, and every pointer derived from it rejects any access afterwards. A
destructor can be any foreign function that takes a single pointer, such as
`sqlite3_close`, `png_destroy`, or `CString::from_raw` wrapped in an
`extern "C"` function. Only a pointer to the start of an allocation can free
it.

The garbage collector runs destructors in the middle of its own work, when
no Zuri code can run, so a destructor that calls a callback during garbage
collection always gets zero back, or the callback's `error_value`. When you
call `free()` yourself, the destructor runs from your program, where
callbacks work as they do anywhere else.

## Static Libraries

A static library is object code waiting for a linker, so nothing can load it
at run time as it is. `ffi.link()` passes it to the platform's linker, which
links every object in it into a shared library, and then loads the result:

```zuri,ignore
var geometry = ffi.link('build/libgeometry.a', {
  libraries: ['m'],
})
var rust = ffi.link('shapes/target/release/libshapes.a')
```

On Linux and macOS, the linker is the C compiler, which is `cc` or whatever
`$CC` names. On Windows, it's MSVC's `link.exe`, found through the Visual
Studio installation. The result is cached under a name based on the contents
of the archives and the options, in `ffi.default_link_cache()` or the
directory given in the `cache` option, so the linker only runs once for a
given input, and every time the program starts after that, it loads the
cached library.

A Rust `staticlib` includes the Rust standard library, which in turn needs a
few system libraries. An archive that contains Rust code is recognized and
linked against them automatically.

On Windows, a static library's functions aren't marked for export, so the
linked DLL exports every symbol the archives define with an unmangled name,
or exactly the symbols listed in the `exports` option.

`libraries`, `search_paths` and `flags` pass extra libraries, their
directories and raw arguments to the linker, and `linker` replaces the
linker itself. A failure raises a `LinkError` that includes the linker's own
output.

## Isolates

Types, pointers, libraries, foreign functions, callbacks and declarations
can all be passed to another isolate, and none of them are moved: both sides
keep a working handle to the same thing. A type is a description, a library
is a handle that the loader shares across threads, and a pointer is an
address, so memory written by one isolate can be read by another:

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

Whatever is in that memory is shared without any synchronization, just as it
is between C threads. A callback belongs to the isolate that created it, so
if it's passed somewhere else, its calls still run on the original isolate.

## Rust in Depth

Every Rust type that has a defined C ABI can cross the boundary, and every
other type is rejected with an explanation.

| Rust | Crosses as |
| --- | --- |
| `i8` ... `u128`, `isize`, `usize` | Integers, including 128-bit integers passed by value |
| `f32`, `f64` | Numbers |
| `bool` | A bool |
| `char` | A one-character string, checked to be a Unicode scalar value |
| `*const T`, `*mut T` | A `Pointer`, or `nil` for null |
| `&T`, `&mut T`, `NonNull<T>`, `Box<T>` | A `Pointer`. Passing `nil` is rejected. |
| `Option<&T>`, `Option<NonNull<T>>`, `Option<Box<T>>` | A `Pointer` or `nil` |
| `extern "C" fn(...)` | A function pointer: a Zuri function or a `Callback` going in, and a callable value coming out |
| `Option<extern "C" fn(...)>` | The same, or `nil` |
| `NonZeroU32` and the other `NonZero` types | A number. Zero is rejected. |
| `Option<NonZeroU32>` | A number, or `nil` for `None` |
| `[T; N]` | A list |
| `#[repr(C)]` struct | A dictionary |
| `#[repr(transparent)]` struct | Its single field |
| `#[repr(C)]` or `#[repr(u8)]` enum without fields | A number, or a variant name going in |
| `#[repr(C)]`, `#[repr(C, u8)]` or `#[repr(u8)]` enum with fields | A dictionary with a `variant` key |
| `MaybeUninit<T>`, `ManuallyDrop<T>`, `Cell<T>` | The same as `T` |
| `PhantomData<T>` | Nothing, because it takes up no space |

An enum with fields is laid out according to the rules for its
representation. A value names its variant, and the rest of the dictionary
holds its fields. The fields of a tuple variant are named `'0'`, `'1'` and
so on, and a variant without fields can be passed as just its name:

```zuri,ignore
shapes.area({ variant: 'Circle', radius: 2 })
shapes.area({ variant: 'Rect', w: 2, h: 3 })
tokens.value({ variant: 'Number', '0': 42 })
tokens.value('Plus')
```

A slice crosses as the pointer and length that Rust's own FFI convention
combines it into, and `ffi.slice(type)` builds that `#[repr(C)]` struct:

```zuri,ignore
var Slice = ffi.slice(ffi.i32)
var values = ffi.alloc(ffi.i32, 3)
lib.sum_slice({ ptr: values, len: 3 })
```

These are rejected, each with an explanation: `&str`, `&[T]`, `String`,
`Vec` and every other type without a stable layout, trait objects, tuples,
generic types and functions, `Option` of a type without a niche, a function
without `#[no_mangle]` or `#[export_name]`, and the `extern "Rust"` ABI.

A panic that escapes from a function in a Rust library ends the process. A
library that catches its panics at its own boundary, or keeps them on its
own threads, returns normally. One that lets a panic escape ends the
process, even when it's called from an isolate. An `extern "C"` function
aborts immediately. An `extern "C-unwind"` function unwinds into Zuri, which
can't catch a panic from another copy of Rust's runtime, so it aborts. A
Rust library built with `panic = "abort"` ends the process on any panic.
This is the only kind of panic that isn't contained within its isolate.

## Platform Differences

The module runs on 64-bit little-endian platforms, x86-64 and Arm, on Linux,
macOS and Windows, and it follows each platform's C compiler:

| | Linux x86-64 | Linux Arm | macOS x86-64 | macOS Arm | Windows x86-64 |
| --- | --- | --- | --- | --- | --- |
| `long` | 8 | 8 | 8 | 8 | 4 |
| `char` | Signed | Unsigned | Signed | Signed | Signed |
| `wchar_t` | 4, signed | 4, unsigned | 4, signed | 4, signed | 2, unsigned |
| `long double` | 80-bit | 128-bit | 80-bit | 64-bit | 64-bit |
| `_Complex` passed by value | Yes | Yes | Yes | Yes | No |
| Enum values larger than `int` | Wider type | Wider type | Wider type | Wider type | `int`, truncated |
| Bitfield rules | GCC | GCC | Clang | Clang | MSVC |

`ffi.platform()` reports these values for the running platform. A program
that passes records between platforms through files or sockets gets the same
layouts that the C compiler on each platform produces, which is what the C
code on that platform expects.

Calling conventions follow the platform too: System V on Linux and macOS
x86-64, AAPCS64 on Arm, with Apple's variations of it, and the Microsoft x64
convention on Windows. On x86-64, a function type or `Library.function()`
can request `abi: 'win64'`, or `abi: 'sysv64'` on Unix, for a function
compiled for the other convention. Declarations read `ms_abi` and `sysv_abi`
attributes, and Rust's `extern "win64"` and `extern "sysv64"`.

128-bit integers can be passed by value, in both directions, under every one
of these conventions. The Microsoft x64 convention passes them by reference
and returns them in a vector register, the same way rustc, Clang and GCC
compile them, so a callback that returns an `i128` works there just as it
does everywhere else.

## What Is Checked

A foreign call runs code that the module can't see into, so its guarantees
stop where the C code begins. Up to that point:

- Every value is converted to its declared type, with integers checked
  against their range, and every other kind of value checked against its
  type.
- Memory allocated by the module is checked against its bounds and rejects
  any use after `free()`. Freeing it twice is an error.
- A null pointer is caught before anything is read or written through it.
- A write that fails doesn't change anything.
- A Zuri error inside a callback never unwinds through C code, and a Rust
  panic inside the module never reaches C code.
- A callback called from any thread runs on its own isolate.

Beyond that, what C code does is up to C. A declaration that doesn't match
the library, a pointer read beyond what the library says is there, a
callback that C calls after it was released, or bytes resized by a callback
while C writes to them are all undefined behavior in C, and they still are
here. Reading declarations from the library's own header or crate, instead
of writing them by hand, is the best way to avoid the first of these.

## Design Limits

**It doesn't run the full C preprocessor.** Macros that define values and
conditional blocks are evaluated, but including other files and expanding
function-like macros aren't supported, and they're rejected where they
appear.

**It doesn't compile C code.** Static libraries are linked by the platform's
own linker, which needs a C toolchain on the machine that links them.

**It doesn't guess who owns memory.** A pointer returned by C isn't freed
until your program says how, with `own()`.

**It doesn't free callbacks automatically.** A callback lasts until you call
`release()`, because only your program knows when the library has finished
with it.

**It doesn't unwind through foreign stack frames.** Errors are caught and
raised again once the C code has returned. A panic in a callback is held in
the same way, and then continues and ends its isolate. A panic that escapes
from a Rust library ends the process, as described in [Rust in
Depth](#rust-in-depth).

## Module Reference

The standard library reference documents every class and method. Here's an
overview of the module:

| Function | Description |
| --- | --- |
| `ffi.open(name, options)` | Loads a library, or the current process |
| `ffi.find(name, paths)` | Finds where a library would be loaded from |
| `ffi.link(archives, options)` | Links static libraries and loads the result |
| `ffi.declare(source)`, `ffi.declare_rust(source)`, `ffi.declarations()` | Create sets of declarations |
| `ffi.struct(name)`, `ffi.union(name)`, `ffi.enum(name, type)` | Build records and enums by hand |
| `ffi.pointer(type, options)`, `ffi.array(type, length)`, `ffi.function_type(returns, params, options)`, `ffi.slice(type)`, `ffi.type(spelling)` | Build other types |
| `ffi.alloc(type, count)`, `ffi.alloc_bytes(size)`, `ffi.malloc(size)`, `ffi.alloc_string(text)`, `ffi.at(address, type)` | Allocate or refer to memory |
| `ffi.function(pointer, type)`, `ffi.threaded(function)`, `ffi.describe(function)`, `ffi.is_foreign(value)` | Work with foreign functions |
| `ffi.callback(function, type, options)`, `ffi.serve(timeout)` | Work with callbacks |
| `ffi.errno()`, `ffi.set_errno(n)`, `ffi.last_error()` | Read and set error codes |
| `ffi.platform()`, `ffi.default_link_cache()` | Report facts about the platform |
| `ffi.LIBC`, `ffi.LIBM` | Name the C runtime and math libraries |

Methods of a `Library`: `function`, `variable`, `symbol`, `has`, `declare`,
`declare_rust`, `bind`, `path`, `close`, `is_closed`.

Methods of a `Pointer`: `get`, `set`, `read`, `write`, `get_field`,
`set_field`, `field`, `read_string`, `write_string`, `read_bytes`,
`write_bytes`, `to_list`, `add`, `offset`, `cast`, `copy_from`, `fill`,
`compare`, `own`, `free`, `is_owned`, `is_freed`, `address`, `is_null`,
`type`, `size`, `equals`.

Methods of a `Type`: `name`, `kind`, `size`, `align`, `is_const`, `target`,
`length`, `signature`, `pointer`, `array`, `as_const`, `of`, `equals`. A
`StructType` or `UnionType` also has `add_field`, `add_bitfield`,
`set_packed`, `set_align`, `fields`, `offset_of`, `has_field` and
`variants`, and an `EnumType` has `add_constant`, `constants` and `value`.

Methods of a `Declarations`: `declare`, `declare_rust`, `include`, `type`,
`constant`, `constants`, `types`, `functions`, `variables`, `bind`.

Methods of a `Callback`: `pointer`, `type`, `release`, `is_released`.
