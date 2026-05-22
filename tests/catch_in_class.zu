class A {
  var a = 5

  test2() {
    echo 'Test 2 called...'
  }

  test() {
    var b = 20
    catch {
      echo [][10]
    } as e

    echo 'Finally...'
    self.test2()
    if self.a {
      echo self.a
    }
    echo b

    if e {
      echo 'Error occurred:'
      echo e.message
      echo e.stacktrace
    }
  }
}

A().test()