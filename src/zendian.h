#ifndef ZURI_ENDIAN_H
#define ZURI_ENDIAN_H

#ifndef _WIN32
#include <sys/param.h>
#endif

#include <inttypes.h>
#include <stdint.h>

#ifndef BYTE_ORDER
#if (BSD >= 199103) || defined(__MACH__) || defined(__APPLE__)

# include <machine/endian.h>

#elif defined(__FreeBSD__) || defined(__NetBSD__) || defined(__OpenBSD__)
#include <sys/endian.h>
#elif defined(linux) || defined(__linux__)
# include <endian.h>
#else
#define  LITTLE_ENDIAN  1234  /* least-significant byte first (vax, pc) */
#define  BIG_ENDIAN  4321  /* most-significant byte first (IBM, net) */
#define  PDP_ENDIAN  3412  /* LSB first in word, MSW first in long (pdp)*/

#if defined(vax) || defined(ns32000) || defined(sun386) || defined(__i386__) || \
    defined(MIPSEL) || defined(_MIPSEL) || defined(BIT_ZERO_ON_RIGHT) || \
    defined(__alpha__) || defined(__alpha) || \
     defined(_M_IX86) || defined(_M_X64) || defined(_M_IA64) || /* msvc for intel processors */ \
     defined(_M_ARM) || defined(_WIN32) /* msvc code on arm executes in little endian mode */
#define BYTE_ORDER	LITTLE_ENDIAN
#endif

#if defined(sel) || defined(pyr) || defined(mc68000) || defined(sparc) || \
    defined(is68k) || defined(tahoe) || defined(ibm032) || defined(ibm370) || \
    defined(MIPSEB) || defined(_MIPSEB) || defined(_IBMR2) || defined(DGUX) || \
    defined(apollo) || defined(__convex__) || defined(_CRAY) || \
    defined(__hppa) || defined(__hp9000) || \
    defined(__hp9000s300) || defined(__hp9000s700) || \
    defined (BIT_ZERO_ON_LEFT) || defined(m68k) || defined(__sparc) || \
     defined(_M_PPC)
#define BYTE_ORDER	BIG_ENDIAN
#endif
#endif /* BSD */
#endif /* BYTE_ORDER */

#if defined(__BYTE_ORDER) && !defined(BYTE_ORDER)
#if (__BYTE_ORDER == __LITTLE_ENDIAN)
#define BYTE_ORDER LITTLE_ENDIAN
#else
#define BYTE_ORDER BIG_ENDIAN
#endif
#endif

#if !defined(BYTE_ORDER) || \
    (BYTE_ORDER != BIG_ENDIAN && BYTE_ORDER != LITTLE_ENDIAN && \
    BYTE_ORDER != PDP_ENDIAN)
/* you must determine what the correct bit order is for
   * your compiler - the next line is an intentional error
   * which will force your compiles to bomb until you fix
   * the above macros.
   */
#error "Undefined or invalid BYTE_ORDER"
#endif

#if BYTE_ORDER == LITTLE_ENDIAN
#define IS_LITTLE_ENDIAN 1
#define IS_BIG_ENDIAN 0
#elif BYTE_ORDER == BIG_ENDIAN
#define IS_LITTLE_ENDIAN 0
#define IS_BIG_ENDIAN 1
#else
#error "Cannot determine machine endianess."
#endif

#if defined(__x86_64__) || defined(__LP64__) || defined(_LP64) || defined(_WIN64) || defined(__aarch64__)
# define IS_64_BIT 1
#else
#define IS_64_BIT 0
#endif

#endif //ZURI_ENDIAN_H
