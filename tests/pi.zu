# For now, this is just good enough...
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
