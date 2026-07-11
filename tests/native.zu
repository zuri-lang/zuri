def fib(n) {
  if n < 2 return n
  return fib(n - 2) + fib(n - 1)
}

def fib2(n) {
  if n < 2 return n

  var i = 1, previous = 0, pprevious = 1, current

  while i <= n {
    current = pprevious + previous
    pprevious = previous
    previous = current
    i++
  }

  return current
}

class A {
  @to_abs() {
    return 300
  }

  @to_string() {
    return 'A class called A'
  }
}

echo abs(-10)
echo abs(A())
echo to_string([1, 2, 3])
echo to_string({name: 'Richard', age: 28})
echo to_string(A())

var start = time()
echo fib(30)
echo 'Time taken for recursive fibonacci: ${time() - start}s'

start = time()
echo fib2(60)
echo 'Time taken for non-recursive fibonacci: ${time() - start}s'
