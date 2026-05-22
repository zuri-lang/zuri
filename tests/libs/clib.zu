import clib
import reflect
import struct

catch {
  var c = clib.load('libc')

  var tm = clib.named_struct({
    tm_sec: clib.int,
    tm_min: clib.int,
    tm_hour: clib.int,
    tm_mday: clib.int,
    tm_mon: clib.int,
    tm_year: clib.int,
    tm_wday: clib.int,
    tm_yday: clib.int,
    tm_isdst: clib.int,
  })

  var time = c.define('time', clib.long, clib.ptr)
  var localtime = c.define('localtime', clib.ptr, clib.ptr)

  var local_time = localtime(struct.pack('i', time(nil)))
  echo clib.get(tm, local_time)

  var strftime = c.define('strftime', clib.long, clib.ptr, clib.long, clib.char_ptr, clib.ptr)
  var buffer = bytes(80)
  echo strftime(buffer, 80, 'Today is %A, %B %d, %Y.', local_time)
  echo buffer.to_string()
} as e

if e {
  echo 'CLIB TEST ERROR:'
  echo '======================================================'
  echo e.message
  echo e.stacktrace
}


