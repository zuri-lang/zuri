class Pair {
  var field = 10

  method() {

  }
}

var pair = Pair()
pair.first = 1
pair.second = 2
echo pair.first + pair.second
echo pair.field

class Scone {
  topping(first, second) {
    echo 'scone with ' + first + ' and ' + second
  }
}

var scone = Scone()
scone.topping('berries', 'cream')

class Person {
  var age = 10
  static var id = 2001

  Person(name) {
    self.name = name
  }

  welcome(age) {
    echo 'welcome ' + self.name + '. You are ' + (self.age + age) + ' years old'
  }

  static shout() {
    echo 'Person is shouting'
  }
}

var person1 = Person('Richard')
var person2 = Person('Jane')

Person.shout()
echo Person.id

person1.welcome(5)
person2.welcome(15)


class A {
  say() {
    echo "A"
  }
}

class B < A {
  getClosure() {
    def closure() {
      parent.say()
    }
    return closure
  }

  say() {
    echo "B"
  }
}

class C < B {
  say() {
    echo "C"
  }
}

C().getClosure()()


class Animal {
  setName() {
    self._echo()
    self._print('hello')
  }

  _echo() {
    echo 'Name is set'
  }

  echoes() {
    return self._echo()
  }

  var _print = @(g) {
    echo g
  }
}

class Dog < Animal {

  var _x = 50

  getName() {
    self._x += 120
    echo self._x
    return parent.echoes()
  }
}

Animal().setName()
Animal()._print('hello again')
Dog().getName()
