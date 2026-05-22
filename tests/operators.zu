echo 2 + 2
echo 2 * 2
echo ~2
echo 2 >>> -1


class A {
  A(val) {
    self.val = val
  }

  def + {
    return A(self.val + __arg__.val)
  }

  def >>> {
    return A(self.val * __arg__.val)
  }

  def ~ {
    return A(self.val ** 2)
  }
}

echo (A(5) + A(11)).val
echo (A(5) >>> A(12)).val
echo (~~A(12)).val

var g = A(24)
g += A(93)

echo g.val

echo ~12
