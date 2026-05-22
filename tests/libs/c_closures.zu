def test(a, i) {
  return to_string(a) + ' ' + i
}

var text = 'all is well'
for i in 0..100 {
  echo text.replace_with('/([a-z]+)/', @(match, val) {
    if val == 'is' return 'is not'
    return test(val, i)
  })
}

[10, 20, 30, 40].each(@(x, y, z) {
  echo 'It works! ${y} -> ${x}'
})

{name: 'Richard', age: 40}.each(@(x, y, z) {
  echo '${y} -> ${x}'
})

bytes([13,219,79]).each(@(b) {
  echo b
})

echo [1,2,3].map(@(v) {
  return v * 2
})

echo [1,2,3,4,5,6].filter(@(x) { return x % 2 != 0 })

echo [1,2,3,4,5].some(@(x) { return x % 2 != 0 })
echo [1,2,3,4,5].every(@(x) { return x % 2 != 0 })

var people = {
  john: {
    name: 'John',
    age: 40,
    address: 'London, England',
  },
  daniel: {
    name: 'Daniel',
    age: 31,
    address: 'Lagos, Nigeria',
  }
}

echo people.filter(@(x) {
  return x.address.index_of('Lagos') != -1
})

echo people.some(@(x){ return x.age > 30 })
echo people.every(@(x){ return x.age > 40 })

echo [1, 100].reduce(@(i, x) { return max(i, x)  }, 50)
echo [1, 100].reduce(@(i, x) { return max(i, x)  })
echo [].reduce(@(i, x) { return max(i, x)  })

echo [15, 16, 17, 18, 19].reduce(
  @(accumulator, currentValue) { return accumulator + currentValue },
  10
)

def reducer(accumulator, currentValue, index) {
  var returns = accumulator + currentValue
  echo 'accumulator: ${accumulator}, currentValue: ${currentValue}, index: ${index}, returns: ${returns}'
  return returns
}

echo [15, 16, 17, 18, 19].reduce(reducer)

var objects = [{ x: 1 }, { x: 2 }, { x: 3 }]
echo objects.reduce(
  @(accumulator, currentValue) { return accumulator + currentValue.x },
  0
)

# A LITTLE COMPLEX EXAMPLE

var pipe = @(...) {
  var functions = __args__
  return @(initialValue) {
    return functions.reduce(@(acc, fn) { return fn(acc) }, initialValue)
  }
}

# Building blocks to use for composition
var double = @(x) { return 2 * x }
var triple = @(x) { return 3 * x }
var quadruple = @(x) { return 4 * x }

#  Composed functions for multiplication of specific values
var multiply6 = pipe(double, triple);
var multiply9 = pipe(triple, triple);
var multiply16 = pipe(quadruple, quadruple);
var multiply24 = pipe(double, triple, quadruple);

#  Usage
echo multiply6(6); # 36
echo multiply9(9); # 81
echo multiply16(16); # 256
echo multiply24(10); # 240


echo [1, 2, nil, 4].reduce(@(a, b) { return a + b })

echo {name: 'Richard', age: 40}.reduce(@(x, y, z){ return x += z + ' => ' + y + '\n' }, '')

'name'.replace_with('/m/', @(match, offset) {
  echo offset
  return match
})

(0..5).loop(@{
  echo 'It has happened'
})