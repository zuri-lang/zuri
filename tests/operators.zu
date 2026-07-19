echo 2 + 2
echo 2 * 2
echo ~2
echo 2 >>> -1


class A {
  @new(val) {
    self.val = val
  }

  def + {
    return new A(self.val + __arg__.val)
  }

  def >>> {
    return new A(self.val * __arg__.val)
  }

  def ~ {
    return new A(self.val ** 2)
  }
}

echo (new A(5) + new A(11)).val
echo (new A(5) >>> new A(12)).val
echo (~(~(new A(12)))).val

var g = new A(24)
g += new A(93)

echo g.val

echo ~12
