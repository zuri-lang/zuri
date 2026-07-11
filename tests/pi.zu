/* def pi() {
  var k = 1
  var s = 0

  iter var i = 0; i < 100000000000; i++ {
    if i % 2 == 0 s += 4 / k
    else s -= 4 / k
    k += 2
  }
  return s
} */

# While the first method will eventually reach
# 3.141592653589793, on my box, that took nearly
# 100 billion iterations and a hell lot of time.
# This returns 3.141592653589734 in when K >= 2
# taking just a few microseconds.
# So for now, this is just good enough... 
# the error margin is very minimal!
def pi(K) {
  var A = 545140134, B = 13591409, D = 640320
  var id3 = 1.0 / (D ** 3)

  var sum = 0.0, b = id3 ** 0.5, p = 1.0, a = B
  sum = p * a * b

  iter var k = 1; k < K; k++ {
    # A * k + B
    a += A

    # update denominator
    b *= id3
    p *= (6 * k) * (6 * k - 1) * (6 * k - 2) * (6 * k - 4) * (6 * k - 5)
    p /= (3 * k) * (3 * k - 1) * (3 * k - 2) * (k ** 3)
    p -= p

    sum += p * a * b
  }

  return 1.0 / (12 * sum)
}

echo pi(5)
