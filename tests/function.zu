var a = 'outer'

def test() {
  var a = 'inner'
  echo 'It works! ${a}'
}

echo a

echo test
test()

def test2(name, age, ...) {
  echo name
  echo age
  echo __args__
}

test2('Richard', 20, 'James')

def sin(n) {
  if n {
    var t = n, sine = t

    iter var a = 1; a < 24; a++ {
      var mult = -n * n / ((2 * a + 1) * (2 * a))
      t *= mult
      sine += t
    }

    return sine
  }
  return nil
}

echo sin()
echo 'Sin 10 = ${sin(10)}'
