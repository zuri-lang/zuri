# Inheritance

A class extends another with `<`:

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
    return 3.141592653589793 * (self.radius ** 2)
  }
}

echo Circle(1).describe()
```

```console
circle with area 3.141592653589793
```

A subclass inherits every field and method. Single inheritance only; a
class has at most one parent.

Note the direction of the arrow. `class Circle < Shape` creates a **new**
class that borrows from `Shape`. Turning it around —
`class Anything > Shape` — is a different declaration entirely: it adds
methods to `Shape` itself and creates no class at all. See
[Class Extensions](ch06-06-class-extensions.md).

## `parent`

`parent` means two related things.

**`parent(args)`** calls the parent's constructor. That is the
`parent('circle')` line in `Circle` above, and it comes first in `@new`,
before the subclass sets up anything of its own. Calling it late means the
parent's constructor overwrites what you just assigned; not calling it at
all means the parent's fields are never initialised.

**`parent.method(args)`** calls the parent's version of a method that this
class has overridden:

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

Note what happened there. `parent.describe()` ran `Shape`'s `describe`,
which calls `self.area()`, which dispatched back to `Square`'s `area`. A
method always dispatches on the actual object, not on the class the code
was written in.

## Overriding

Redeclaring a method in a subclass replaces it. There is no keyword for it
and no way to forbid it.

`describe()` above is the useful shape of this: a parent method written in
terms of a method the child supplies. `Shape.area()` raises
`NotImplementedError`, so a subclass that forgets to override it says so
clearly:

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

That is Zuri's abstract method. There is no `abstract` keyword because
there does not need to be one.

## A Subclass Without a Constructor

If a subclass declares no `@new`, it uses its parent's:

```zuri
class NoCtor < Shape {}

echo NoCtor('plain').name
```

```console
plain
```

## Testing Ancestry

`instance_of()` walks the whole chain:

```zuri
echo instance_of(Circle(1), Shape)
echo instance_of(Circle(1), Square)
```

```console
true
false
```

`typeof()` gives the most specific class name, as a string:

```zuri
echo typeof(Circle(1))
```

```console
Circle
```

## The Depth Is Free

Inheritance chains cost nothing to walk at runtime. A method lookup on a
class four levels deep is the same operation as one on a class with no
parent, because every class's method table is complete at declaration time.
Write the hierarchy the design wants.

## What You Write Instead of an Interface

Zuri has single inheritance and no interfaces, so the two patterns below do
the jobs an interface would do elsewhere.

**A base class that raises.** When a base class needs every subclass to
supply a method, declare it and raise:

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
    return 3.141592653589793 * (self.radius ** 2)
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

Those parentheses around `self.radius ** 2` are load-bearing. `**` sits at
the same precedence level as `*` and associates left, so
`3.14 * self.radius ** 2` would be `(3.14 * self.radius) ** 2` — a
plausible-looking number that is wrong. When `**` shares an expression with
`*` or `/`, parenthesise.

`describe()` calls `self.area()`, and `self` is the actual instance, so the
subclass's version runs. That is the whole of dynamic dispatch in Zuri:
there is nothing to declare and nothing to mark virtual.

**A parameter typed by the base class.** An annotation naming a class
accepts any subclass of it, which is how you say "anything that is a
Shape":

```zuri,ignore
def total_area(shapes: list) {
  return shapes.reduce(@(sum, shape: Shape) => sum + shape.area(), 0)
}
```

## Checking What Something Is

Two built-ins answer questions about an instance's type, and they answer
**different** questions.

### `instance_of(value, Class)`

`instance_of()` asks "does this behave like a `Class`?", and it walks the
whole inheritance chain to answer:

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

`Ellipse` is two levels below `Shape` and still answers `true`. There is no
depth limit: the check follows parents until it finds a match or runs out.
A sibling — `Circle` against `Square` — is `false`, because siblings share
an ancestor rather than a lineage.

**It never raises for the first argument.** Anything that is not an
instance of the class simply answers `false`, including values that are not
instances at all:

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

That makes it safe to use as a guard on a value you know nothing about; no
`is_instance()` check has to come first.

**A class is not an instance of itself.** Passing the class rather than an
object gives `false`:

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

This trips people up when a variable might hold either. `Circle` is a
class; `Circle()` is an instance; only the second is "a Shape".

**The second argument must be a class**, and here it does raise:

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

The class name as a *string* is the common version of this mistake. Pass
the class itself.

A class held in a variable works, because the check is on the value rather
than on the spelling:

```zuri
class Base {}
class Derived < Base {}

var Alias = Base

echo instance_of(Derived(), Alias)
```

```console
true
```

So does a class reached through a module:

```zuri
import set

echo instance_of(set.set([1, 2]), set.Set)
```

```console
true
```

### It Is How the Error Hierarchy Works

Every built-in error inherits from `Error`, so `instance_of()` is what lets
one handler sort them:

```zuri
echo instance_of(ValueError('bad'), Error)
echo instance_of(ValueError('bad'), TypeError)
```

```console
true
false
```

That is the mechanism behind the "handle one kind, re-raise the rest"
pattern in [Error Handling](ch07-00-error-handling.md), and behind mapping
a domain error to an HTTP status in [Chapter 20](ch21-06-middleware.md).

### `typeof(value)`

`typeof()` asks a different question: "what exactly is this?". On an
instance it names the **concrete** class, and it never mentions ancestors:

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

Note the third line. `typeof()` on the class itself answers `class`, not
`Circle` — the class is a value of kind "class", and its name is not what
`typeof()` reports.

### Which to Use

| Question | Use |
| --- | --- |
| can I treat this as a `Shape`? | `instance_of(x, Shape)` |
| which exact class is this? | `typeof(x)` |
| is this any instance at all? | `is_instance(x)` |

Reach for `instance_of()` by default. It is the one that respects
inheritance, and code written with `typeof(x) == 'Circle'` breaks the day
someone subclasses `Circle` — the subclass is a perfectly good `Circle`,
and the string comparison says otherwise.

### A Type Annotation Is the Same Check

Annotating a parameter with a class name performs exactly the
`instance_of()` test, at every call, with a better error message:

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

Prefer the annotation when the answer decides whether the function should
run at all, and `instance_of()` when the answer decides which branch to
take.
