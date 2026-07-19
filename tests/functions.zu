var a = 'outer'

def test() {
  var a = 'inner'
  echo 'It works! ${a}'
}

echo a

test()

def test2(name, age, ...args) {
  echo name
  echo age
  echo args
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
