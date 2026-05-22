/**
 * ARM64 JIT compilation example.
 */


# -------------- STANDARD IMPLEMENTATION BEGINS ----------------------------

def square(x) {
    var y = 0
    for i in 0..1000000 {
       y += x
    }
    return y
}

var start = microtime()
echo square(9)
echo 'Default: ${(microtime() - start)} microseconds'

# -------------- STANDARD IMPLEMENTATION ENDS ------------------------------

# ------------------- JIT IMPLEMENTATION BEGINS ----------------------------

import process
import clib
import convert

var code_address = process.PagedValue(true, true)

#--------------------- Unoptimized JIT starts ------------------------

# C source
# --------------
# ```
# long square2(long num) {
#     long y = 0;
#     for(int i = 0; i < 1000000; i++) {
#         y += num;
#     }
#     return y;
# }
# ```
#
# Arch: arm64
# Build command: clang -g -O0
#
# Assembly
# ----------------------
# square2:
#    sub	sp, sp, #0x20
#    str	x0, [sp, #24]
#    str	xzr, [sp, #16]
#    str	wzr, [sp, #12]
#    b	14 <square+0x14>
#    ldr	w8, [sp, #12]
#    mov	w9, #0x4240                	// #16960
#    movk	w9, #0xf, lsl #16
#    subs	w8, w8, w9
#    b.ge	50 <square+0x50>  // b.tcont
#    b	2c <square+0x2c>
#    ldr	x9, [sp, #24]
#    ldr	x8, [sp, #16]
#    add	x8, x8, x9
#    str	x8, [sp, #16]
#    b	40 <square+0x40>
#    ldr	w8, [sp, #12]
#    add	w8, w8, #0x1
#    str	w8, [sp, #12]
#    b	14 <square+0x14>
#    ldr	x0, [sp, #16]
#    add	sp, sp, #0x20
#    ret
# -----------------------
var data = convert.hex_to_bytes(hex(0xd10083ff))
data.extend(convert.hex_to_bytes(hex(0xf9000fe0)))
data.extend(convert.hex_to_bytes(hex(0xf9000bff)))
data.extend(convert.hex_to_bytes(hex(0xb9000fff)))
data.extend(convert.hex_to_bytes(hex(0x14000001)))
data.extend(convert.hex_to_bytes(hex(0xb9400fe8)))
data.extend(convert.hex_to_bytes(hex(0x52884809)))
data.extend(convert.hex_to_bytes(hex(0x72a001e9)))
data.extend(convert.hex_to_bytes(hex(0x6b090108)))
data.extend(convert.hex_to_bytes(hex(0x5400016a)))
data.extend(convert.hex_to_bytes(hex(0x14000001)))
data.extend(convert.hex_to_bytes(hex(0xf9400fe9)))
data.extend(convert.hex_to_bytes(hex(0xf9400be8)))
data.extend(convert.hex_to_bytes(hex(0x8b090108)))
data.extend(convert.hex_to_bytes(hex(0xf9000be8)))
data.extend(convert.hex_to_bytes(hex(0x14000001)))
data.extend(convert.hex_to_bytes(hex(0xb9400fe8)))
data.extend(convert.hex_to_bytes(hex(0x11000508)))
data.extend(convert.hex_to_bytes(hex(0xb9000fe8)))
data.extend(convert.hex_to_bytes(hex(0x17fffff2)))
data.extend(convert.hex_to_bytes(hex(0xf9400be0)))
data.extend(convert.hex_to_bytes(hex(0x910083ff)))
data.extend(convert.hex_to_bytes(hex(0xd65f03c0)))

code_address.set(data)

var square2 = clib.function_handle(code_address.raw_pointer(), clib.long, clib.long)

start = microtime()
echo square2(9)
echo 'JITed: ${(microtime() - start)} microseconds'

#--------------------- Unoptimized JIT ends ------------------------

#--------------------- Optimized JIT starts ------------------------

# C source
# --------------
# ```
# long square3(long num) {
#     long y = 0;
#     for(int i = 0; i < 1000000; i++) {
#         y += num;
#     }
#     return y;
# }
# ```
#
# Arch: arm64
# Build command: clang -g -O3
#
# Assembly
# ----------------------
# square3:
#     mov     x1, #0x4240
#     movk    x1, #0xf, lsl #16
#     mul     x0, x0, x1
#     ret
var data = convert.hex_to_bytes(hex(0xd2884801))
data.extend(convert.hex_to_bytes(hex(0xf2a001e1)))
data.extend(convert.hex_to_bytes(hex(0x9b017c00)))
data.extend(convert.hex_to_bytes(hex(0xd65f03c0)))

code_address.set(data)

var square3 = clib.function_handle(code_address.raw_pointer(), clib.long, clib.long)

start = microtime()
echo square3(9)
echo 'JITed and Optimized: ${(microtime() - start)} microseconds'

#--------------------- Optimized JIT ends ------------------------

# ------------------- JIT IMPLEMENTATION ENDS ------------------------------
