var test = @(a) {
  echo a
}
test('It works!')

var test_compat = @(a) {
  echo a
}
test_compat('It works still!')

def main() {
  var g = 'Coke'
  echo g

  var concat = @{
    g += ' is the best'
  }
  concat()
  echo g

  var concat_compat = @{
    g += ' still!'
  }
  concat_compat()
  echo g
}

main()
