/**
 * The Computer Language Benchmarks Game
 * https://salsa.debian.org/benchmarksgame-team/benchmarksgame/
 *
 * by Richard Ore
 * based on the Java version contributed by by Jon Edvardsson
 * added parallel processing to the original
 * program by Anthony Donnefort and Enotus.
 */

import thread
import os

var map = bytes(128)
var mm = [
  "ACBDGHK\nMNSRUTWVYacbdghkmnsrutwvy",
  "TGVHCDM\nKNSYAAWBRTGVHCDMKNSYAAWBR"
]

iter var i = 0; i < mm[0].length(); i++ {
  map[ord(mm[0][i])] = ord(mm[1][i])
}

var buf = file(os.join_paths(os.dir_name(__file__), 'revcomp-input.txt')).read().to_bytes()
var pools = []

var start = microtime()

iter var i = 0; i < buf.length(); {
  while buf[i++ - 1] != 10 {} # 10 == '\n'
  var start = i
  var b
  while i < buf.length() and (b = buf[i++ - 1]) != 62 { # 62 == '>'
    buf[i - 1] = map[b]
  }
  var end = i - 2

  pools.append(thread.start(@(_, buf, begin, end){
#    echo buf
    while true {
      var bb = buf[begin]
      if bb == 10 {
        bb = buf[begin++]
      }
      var be = buf[end]
      if be == 10 {
        be = buf[end--]
      }
      if begin > end {
        break
      }
      buf[begin] = be
      buf[end] = bb
      begin++
      end--
    }
  }, [buf, start, end]))
}

for p in pools {
  p.await()
}

echo buf.to_string()

var end = microtime()

echo '\nTime taken = ${(end - start) / 1000000} seconds'