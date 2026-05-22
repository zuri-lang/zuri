/* The Computer Language Benchmarks Game
   https://salsa.debian.org/benchmarksgame-team/benchmarksgame/

   transliterated from Alexander Fyodorov's program by Isaac Gouy
*/
import bigint

def pad(i, last) {
  var res = to_string(i), count
  count = 10 - res.length()
  while count > 0 {
    last ? (res += ' ') : res = '0' + res
    count--
  }
  return res
}

def calculatePi(arg) {
  var i = 0, ns = 0

  var k = bigint(0)
  var k1 = bigint(1)
  var a = bigint(0)
  var d = bigint(1)
  var m = bigint(0)
  var n = bigint(1)
  var t = bigint(0)
  var u = bigint(1)

  while true {
    k += bigint(1)
    k1 += bigint(2)
    t = n << bigint(1)
    n *= k
    a += t
    a *= k1
    d *= k1

    if a > n {
      m = n * bigint(3) + a
      t = m / d
      u = m % d + n

      if d > u {
        ns = ns * 10 + to_number(t)
        i += 1

        var last = i >= arg
        if i % 10 == 0 or last {
                
          echo pad(ns, last) + '\t:' + i
          ns = 0
        }

        if last break
        
        a = (a - d * t) * bigint(10)
        n = n * bigint(10)
      }
    }
  }
}


var start = microtime()
calculatePi(10000)
echo '\nTotal time taken = ${(microtime() - start) / 1000000}'