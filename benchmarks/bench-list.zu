# This code is derived from the SOM benchmarks
# 
# Based on the JavaScript implementation at 
# https://github.com/smarr/are-we-fast-yet/blob/master/benchmarks/JavaScript/list.js

class Element {
  Element(v){
    self.val = v
    self.next = nil
  }

  length() {
    if self.next == nil {
      return 1
    }

    return 1 + self.next.length()
  }
}

class List {
  benchmark() {
    var result = self.tail(
      self.make_list(15),
      self.make_list(10),
      self.make_list(6)
    )

    return result.length()
  }

  make_list(length) {
    if length == 0 {
      return nil
    }

    var e = Element(length)
    e.next = self.make_list(length - 1)
    return e
  }

  is_shorter_than(x, y) {
    var x_tail = x
    var y_tail = y

    while y_tail != nil {
      if x_tail == nil {
        return true
      }

      x_tail = x_tail.next
      y_tail = y_tail.next
    }

    return false
  }

  tail(x, y, z) {
    if self.is_shorter_than(y, x) {
      return self.tail(
        self.tail(x.next, y, z),
        self.tail(y.next, z, x),
        self.tail(z.next, x, y)
      )
    }

    return z
  }
}

var start = microtime()
echo List().benchmark() == 10
var end = microtime()

echo 'Time taken = ${(end - start) / 1000000} seconds'