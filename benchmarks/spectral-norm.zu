/**
 * The Computer Language Benchmarks Game
 * https://salsa.debian.org/benchmarksgame-team/benchmarksgame/
 *
 * contributed by Richard Ore
 * based on contributed work by Ian Osgood
 * modified for Node.js by Isaac Gouy
 */

const ARRAY_LENGTH = 5_500

var u = [1] * ARRAY_LENGTH
var v = []

def eval_A(i: Number, j: Number) {
  return 1.0/((i+j)*(i+j+1)/2+i+1)
}

def vector_times_array(vector: List) {
  var arr = [], i = 0
  while i < ARRAY_LENGTH {
    var sum = 0, j = 0
    while j < ARRAY_LENGTH {
      sum += eval_A(i,j) * vector[j]
      j += 1
    }
    arr.append(sum)
    i += 1
  }

  return arr
}

def vector_times_array_transposed(vector: List) {
  var arr = [], i = 0
  while i < ARRAY_LENGTH {
    var sum = 0, j = 0
    while j < ARRAY_LENGTH {
      sum += eval_A(j,i) * vector[j]
      j += 1
    }
    arr.append(sum)
    i += 1
  }

  return arr
}

def vector_times_array_times_array_transposed(vector: List) {
  return vector_times_array_transposed(vector_times_array(vector))
}

const start = microtime()

iter var i = 0; i < 10; i++ {
  v = vector_times_array_times_array_transposed(u)
  u = vector_times_array_times_array_transposed(v)
}

var vBv = 0, vv = 0, i = 0
while i < ARRAY_LENGTH {
  vBv += u[i]*v[i]
  vv += v[i]*v[i]
  i += 1
}

echo vBv/vv ** 0.5

echo '\nTotal time taken: ${(microtime() - start)/1_000_000}s'