/**
 * The Computer Language Benchmarks Game
 * https://salsa.debian.org/benchmarksgame-team/benchmarksgame/
 * 
 * contributed by Richard Ore
 * based on contributed work by Ziad Hatahet
 * based on the Go entry by K P anonymous
 * 
 * This implementation of the spectral norm employs multithreading 
 * to achieve more than 10x the speed over the ordinary version 
 * (depending on how many cores your cpu has).
 */ 

import thread

def spectralnorm(n) {
  if !n n = 100

  var u = [1] * n
  var v = [0] * n

  for i in 0..10 {
    a_times_transp(v, u)
    a_times_transp(u, v)
  }

  var vbv = 0, vv = 0
  for i in 0..n {
    var vi = v[i]
    vbv += u[i] * vi
    vv += vi ** 2
  }

  return ((vbv / vv) ** 0.5)
}

def a_times_transp(v, u) {
  var x = [0] * u.length()
  var t = []
  var ncpu = thread.cpu_count

  for i in 0..ncpu {
    t.append(thread.start(
      times,
      [x, i * v.length() / ncpu, (i + 1) * v.length() / ncpu, u, false,]
    ))
  }

  for i in 0..ncpu {
    t[i].await()
  }

  for i in 0..ncpu {
    t[i] = thread.start(
      times,
      [v, i * v.length() / ncpu, (i + 1) * v.length() / ncpu, x, true,]
    )
  }

  for i in 0..ncpu {
    t[i].await()
  }
}

def times(_, v, ii, n, u, transpose) {
  var ul = u.length()
  for i in ii..n {
    var vi = 0
    for j in 0..ul {
      vi +=  u[j] / (transpose ? a(j, i) : a(i, j))
    }

    v[i] = vi
  }
}

def a(i, j) {
  return (i + j) * (i + j + 1) / 2 + i + 1
}

var start = microtime()
echo spectralnorm(5500)
var end = microtime()

echo '\nTime taken = ${(end - start) / 1000000} seconds'
