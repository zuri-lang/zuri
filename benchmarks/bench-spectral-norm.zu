/**
 * The Computer Language Benchmarks Game
 * https://salsa.debian.org/benchmarksgame-team/benchmarksgame/
 * 
 * contributed by Richard Ore
 * based on contributed work by Ian Osgood
 * modified for Node.js by Isaac Gouy
 */ 

def A(i, j) {
  return 1 / (
    (i + j) * (i + j + 1 ) / 2 + i + 1
  )
}

def Au(u, v) {
  var l = u.length()
  for i in 0..l {
    var t = 0
    for j in 0..l t += A(i, j) * u[j]
    v[i] = t
  }
}

def Atu(u, v) {
  var l = u.length()
  for i in 0..l {
    var t = 0
    for j in 0..l t += A(j, i) * u[j]
    v[i] = t
  }
}

def AtAu(u, v, w) {
  Au(u, w)
  Atu(w, v)
}

def spectralnorm(n) {
  var i, u = [1] * n, v = [0] * n, 
      w = [0] * n, vv = 0, vBv = 0

  # for i in 0..n {
  #   u[i] = 1
  #   v[i] = w[i] = 0
  # }

  for i in 0..10 {
    AtAu(u, v, w)
    AtAu(v, u, w)
  }

  for i in 0..n {
    vBv += u[i] * v[i]
    vv  += v[i] * v[i]
  }

  return (vBv / vv) ** 0.5
}

var start = time()
echo spectralnorm(5500)

echo '\nTime taken = ${time() - start}'