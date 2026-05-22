import io { putc }

def mandlebrot(h, w) {
  if !w w = h

  echo 'P4\n${w} ${h}'

  var bit_num = 128, bytes_acc = 0,
      yfac = 2/h, xfac = 2/w

  iter var y = 0; y < h; y++ {
    var ci = y * yfac - 1

    iter var x = 0; x < w; x++ {
      var zr = 0, zi = 0, tr = 0, ti = 0
      var cr = x * xfac - 1.5

      var should_break = false
      do {
        iter var i = 0; i < 50; i++ {
          zi = 2 * zr * zi + ci
          zr = tr - ti + cr
          tr = zr * zr
          if tr + (ti = zi * zi) > 4 {
            should_break = true
            break
          }
        }
        if should_break break
        bytes_acc += bit_num
      } while false

      if bit_num == 1 {
        putc(bytes_acc)
        bit_num = 128
        bytes_acc = 0
      } else {
        bit_num >>= 1
      }
    }

    if bit_num != 128 {
      putc(bytes_acc)
      bit_num = 128
      bytes_acc = 0
    }
  }
}

var start = microtime()
mandlebrot(16000)
echo 'Total time taken = ${(microtime() - start) / 1000000}'