import io { putc }

def mandlebrot(h) {
  var w = h, bit_num = 0, bytes_acc = 0
  var i, it = 50
  var x, y, limit = 2
  var Zr, Zi, Cr, Ci, Tr, Ti

  echo 'P4\n${w} ${h}'

  iter y = 0; y < h; y++ {
    iter x = 0; x < w; x++ {
      Zr = Zi = Tr = Ti = 0
      Cr = 2 * x / w - 1.5; Ci = 2 * y / h - 1

      iter i = 0; i < it and (Tr + Ti <= limit * limit); i++ {
        Zi = 2 * Zr * Zi + Ci
        Zr = Tr - Ti + Cr
        Tr = Zr * Zr
        Ti = Zi * Zi
      }

      bytes_acc <<= 1
      if Tr + Ti <= limit * limit {
        bytes_acc |= 0x01
      }

      bit_num++

      if bit_num == 8 {
        putc(bytes_acc)
        bytes_acc = 0
        bit_num = 0
      } else if x == w - 1 {
        bytes_acc <<= 8 - w % 8
        putc(bytes_acc)
        bytes_acc = 0
        bit_num = 0
      }
    }
  } 
}

var start = microtime()
mandlebrot(8000)
echo '\nTotal time taken = ${(microtime() - start) / 1000000}'
