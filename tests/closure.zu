var x = "global"
def outer() {
  var x = "outer"
  def inner() {
    echo x
  }
  inner()
}
outer()



# test 2

def outer() {
  var x = "value"
  def middle() {
    def inner() {
      echo x
    }

    echo "create inner closure"
    return inner
  }

  echo "return from outer"
  return middle
}

var mid = outer()
var _in = mid()
_in()


iter var i = 0; i < 10; i++ {
  var start = time()
  var sum = 0
  iter var j = 0; j < 1000; j++ {
    def outer(a, b, c) {
      def inner() {
        return a + b + c
      }
      return inner
    }

    var closure = outer(j, j, j)
    sum = sum + closure()
  }

  echo sum
  echo time() - start
}
