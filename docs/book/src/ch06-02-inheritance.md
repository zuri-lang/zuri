# Inheritance

To make a class inherit from another, use `<`:

```zuri
class Shape {
  @new(name) {
    self.name = name
  }

  area() {
    raise NotImplementedError('${self.name} must define area()')
  }

  describe() {
    return '${self.name} with area ${self.area()}'
  }
}

class Circle < Shape {
  @new(radius) {
    parent('circle')
    self.radius = radius
  }

  area() {
    return 3.141592653589793 * self.radius ** 2
  }
}

echo Circle(1).describe()
```

```console
circle with area 3.141592653589793
```

A subclass inherits all of its parent's fields and methods. Zuri only
supports single inheritance, so a class can have at most one parent.

Pay attention to the direction of the arrow. `class Circle < Shape` creates
a **new** class based on `Shape`. Reversing it to `class Anything > Shape`
means something else entirely: it adds methods to `Shape` itself and doesn't
create a class at all. See [Class Extensions](ch06-06-class-extensions.md).

## Specifying the Parent Class

A class is a value like any other, so the parent doesn't have to be a plain
name declared in the current file. Any expression that starts with a name
works, which lets you build on a class from another module:

```zuri
import log

class MemoryTransport < log.Transport {
  @new() {
    self.lines = []
  }

  handle(record) {
    self.lines.append(record)
  }
}

echo instance_of(MemoryTransport(), log.Transport)
```

```console
true
```

Property access, indexing and calls all work the same way, so a parent taken
from a registry (`class Store < backends['redis']`) or returned by a
function (`class Store < chosen_backend()`) is just as valid as a name. The
expression is evaluated once, at the point where the class is declared.

The only rule is that the expression has to start with a name. That's what
stops the `{` that opens the class body from being read as the start of a
dictionary.

## `parent`

`parent` has two related uses.

**`parent(args)`** calls the parent class's constructor. That's the
`parent('circle')` line in `Circle` above. Call it first in `@new`, before
the subclass sets up anything of its own. If you call it later, the parent's
constructor overwrites what you've just assigned, and if you don't call it
at all, the parent's fields are never initialized.

**`parent.method(args)`** calls the parent's version of a method that this
class overrides:

```zuri
class Square < Shape {
  @new(side) {
    parent('square')
    self.side = side
  }

  area() {
    return self.side ** 2
  }

  describe() {
    return 'a ' + parent.describe()
  }
}

echo Square(2).describe()
```

```console
a square with area 4
```

Look at what happened there. `parent.describe()` ran `Shape`'s version of
`describe`, which calls `self.area()`, and that call went to `Square`'s
`area`. Method calls are always resolved on the actual object, not on the
class where the calling code was written.

## Overriding Methods

To override a method, declare it again in the subclass. You don't need a
keyword, and there's no way to prevent a method from being overridden.

`describe()` above shows the most useful version of this: a parent method
written in terms of a method that the child class provides. `Shape.area()`
raises `NotImplementedError`, so a subclass that forgets to override it gets
a clear error:

```zuri
catch {
  Shape('blob').area()
} as e {
  echo e.message
}
```

```console
blob must define area()
```

That's how you write an abstract method in Zuri. There's no `abstract`
keyword, because you don't need one.

## Subclasses Without a Constructor

If a subclass doesn't declare `@new`, it uses its parent's constructor:

```zuri
class NoCtor < Shape {}

echo NoCtor('plain').name
```

```console
plain
```

## Checking a Class's Ancestors

`instance_of()` checks the entire inheritance chain:

```zuri
echo instance_of(Circle(1), Shape)
echo instance_of(Circle(1), Square)
```

```console
true
false
```

`typeof()` returns the name of the most specific class, as a string:

```zuri
echo typeof(Circle(1))
```

```console
Circle
```

## Deep Hierarchies Cost Nothing

Inheritance chains have no cost at run time. Looking up a method on a class
four levels deep is the same operation as looking it up on a class with no
parent, because each class's method table is complete as soon as the class
is declared. Design the hierarchy however your problem needs.

## Alternatives to Interfaces

Zuri has single inheritance and no interfaces, but the two patterns below
cover what you'd use an interface for in other languages.

**A base class that raises an error.** When every subclass needs to provide
a particular method, declare it in the base class and raise an error from
it:

```zuri
class Shape {

  @new(name: string) {
    self.name = name
  }

  area() {
    raise NotImplementedError('${self.name} must define area()')
  }

  describe() {
    return '${self.name} has area ${self.area()}'
  }
}

class Circle < Shape {

  @new(radius: number) {
    parent('circle')
    self.radius = radius
  }

  area() {
    return 3.141592653589793 * self.radius ** 2
  }
}

class Blob < Shape {

  @new() {
    parent('blob')
  }
}

echo Circle(2).describe()

catch {
  echo Blob().describe()
} as e {
  echo '${e.type}: ${e.message}'
}
```

```console
circle has area 12.566370614359172
NotImplementedError: blob must define area()
```

`describe()` calls `self.area()`, and `self` is the actual instance, so the
subclass's version runs. That's all there is to dynamic dispatch in Zuri.
You don't need to declare anything or mark methods as virtual.

**A parameter annotated with the base class.** A type annotation that names
a class accepts any of its subclasses, so you can use it to say "any kind of
Shape":

```zuri,ignore
def total_area(shapes: list) {
  return shapes.reduce(@(sum, shape: Shape) => sum + shape.area(), 0)
}
```

## Checking the Type of a Value

There are two built-in functions for checking an instance's type, and they
answer **different** questions.

### `instance_of(value, Class)`

`instance_of()` answers the question "can I treat this as a `Class`?", and
it checks the whole inheritance chain to find out:

```zuri
class Shape {}
class Circle < Shape {}
class Square < Shape {}
class Ellipse < Circle {}

var c = Circle()

echo instance_of(c, Circle)
echo instance_of(c, Shape)
echo instance_of(c, Square)
echo instance_of(Ellipse(), Shape)
```

```console
true
true
false
true
```

`Ellipse` is two levels below `Shape`, and the result is still `true`.
There's no limit on depth, because the check follows the parents until it
finds a match or reaches the top. A sibling class, such as `Circle` compared
with `Square`, gives `false`, because siblings share an ancestor but neither
descends from the other.

**It never raises an error because of the first argument.** Anything that
isn't an instance of the class just gives `false`, including values that
aren't instances at all:

```zuri
class Shape {}

echo instance_of(42, Shape)
echo instance_of('circle', Shape)
echo instance_of(nil, Shape)
echo instance_of([1, 2], Shape)
```

```console
false
false
false
false
```

This means it's safe to use as a check on a value you know nothing about,
without calling `is_instance()` first.

**A class isn't an instance of itself.** If you pass the class instead of an
object, the result is `false`:

```zuri
class Shape {}
class Circle < Shape {}

echo instance_of(Circle, Shape)
echo instance_of(Circle(), Shape)
```

```console
false
true
```

This catches people out when a variable might hold either one. `Circle` is a
class and `Circle()` is an instance, and only the instance is a Shape.

**The second argument has to be a class**, and here it does raise an error
if it isn't:

```zuri
class Shape {}

catch {
  instance_of(Shape(), 'Shape')
} as e {
  echo e.message
}
```

```console
instance_of() expects argument 2 to be a class, got string
```

The most common version of this mistake is passing the class name as a
*string*. Pass the class itself instead.

A class stored in a variable works, because the check uses the value, not
the name you wrote:

```zuri
class Base {}
class Derived < Base {}

var Alias = Base

echo instance_of(Derived(), Alias)
```

```console
true
```

So does a class accessed through a module:

```zuri
import set

echo instance_of(set([1, 2]), set.Set)
```

```console
true
```

### How the Error Hierarchy Uses It

All of the built-in errors inherit from `Error`, so `instance_of()` lets a
single handler tell them apart:

```zuri
echo instance_of(ValueError('bad'), Error)
echo instance_of(ValueError('bad'), TypeError)
```

```console
true
false
```

This is how the "handle one kind of error and re-raise the rest" pattern in
[Error Handling](ch07-00-error-handling.md) works, as well as the mapping
from domain errors to HTTP status codes in [Chapter
25](ch25-06-middleware.md).

### `typeof(value)`

`typeof()` answers a different question: "what exactly is this?" For an
instance, it returns the name of its **actual** class, and it never mentions
the parent classes:

```zuri
class Shape {}
class Circle < Shape {}

echo typeof(Circle())
echo typeof(Shape())
echo typeof(Circle)
echo typeof(42)
echo typeof('text')
```

```console
Circle
Shape
class
number
string
```

Look at the third line. `typeof()` on the class itself returns `class`, not
`Circle`. The class is a value of type "class", and `typeof()` reports the
type, not the class's name.

### Which One to Use

| Question | Use |
| --- | --- |
| Can I treat this as a `Shape`? | `instance_of(x, Shape)` |
| Which exact class is this? | `typeof(x)` |
| Is this an instance of any class? | `is_instance(x)` |

Use `instance_of()` by default, because it takes inheritance into account.
Code that checks `typeof(x) == 'Circle'` breaks as soon as someone
subclasses `Circle`, since an instance of the subclass is a perfectly good
`Circle`, but the string comparison says it isn't.

### Type Annotations Use the Same Check

Annotating a parameter with a class name performs the same test as
`instance_of()` on every call, and gives a better error message:

```zuri
class Shape {}
class Circle < Shape {}

def describe(shape: Shape) {
  return 'a shape of type ${typeof(shape)}'
}

echo describe(Circle())
echo describe(Shape())

catch {
  describe(42)
} as e {
  echo e.message
}
```

```console
a shape of type Circle
a shape of type Shape
describe() expects parameter 'shape' (argument 1) to be a Shape, got number
```

Use an annotation when the answer decides whether the function should run at
all, and `instance_of()` when the answer decides which branch to take.
