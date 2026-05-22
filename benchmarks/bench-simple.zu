/**
 * The Computer Language Benchmarks Game
 * https://salsa.debian.org/benchmarksgame-team/benchmarksgame/
 *
 * by Richard Ore
 */

def simple(n) {
  # mtime side-effect
  file('./one', 'w').close()

  var sum = 0
  var flip = -1

  for i in 1..n {
    flip *= -1
    sum += flip / (2 * i - 1)
  }

  echo sum * 4

  # mtime side-effect
  file('./two', 'w').close()
}

var start = microtime()
simple(100000000)
var end = microtime()

echo '\nTime taken = ${(end - start) / 1000000} seconds'