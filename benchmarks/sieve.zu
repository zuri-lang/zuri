# The sieve benchmark counts the number of primes below 600000 based on the sieve of Eratosthenes.
# The correct result is 49098.
#
# based on the JavaScript implementation at
# https://github.com/oracle/graal/blob/master/vm/benchmarks/interpreter/sieve.js

def run(number) {
  var primes = (0..(number + 1)).to_list()

  var i = 2
  while i ** 2 <= number {
    if primes[i] != 0 {
      for j in 2..number {
        if primes[i] * j > number break
        else primes[primes[i] * j] = 0
      }
    }
    i += 1
  }

  var count = 0
  for c in 2..(number + 1) {
    if primes[c] != 0 count += 1
  }

  return count
}

var start = microtime()
echo run(600_000)

echo '\nTotal time taken: ${(microtime() - start)/1_000_000}s'
