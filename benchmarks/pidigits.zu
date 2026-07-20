/* The Computer Language Benchmarks Game
   https://salsa.debian.org/benchmarksgame-team/benchmarksgame/

   transliterated from Alexander Fyodorov's program by Isaac Gouy
*/

def pad(i: Number, last: Bool) {
  var res = i.to_string()
  var count = 10 - res.length()
  while count > 0 {
    if last {
      res += ' '
    } else {
      res = '0' + res
    }
    count--
  }
  return res
}

def calculatePi(arg: Number) {
  var i = 0, ns = 0

  var k = 0n
  var k1 = 1n
  var a = 0n
  var d = 1n
  var m = 0n
  var n = 1n
  var t = 0n
  var u = 1n

  while true {
    k += 1n
    k1 += 2n
    t = n << 1n
    n *= k
    a += t
    a *= k1
    d *= k1

    if a > n {
      m = n * 3n + a
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

        a = (a - d * t) * 10n
        n = n * 10n
      }
    }
  }
}


var start = microtime()
calculatePi(10_000)

echo '\nTotal time taken: ${(microtime() - start)/1_000_000}s'