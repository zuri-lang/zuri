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

## `parent`

`parent` means two related things.

**`parent(args)`** calls the parent's constructor. Do it first in `@new`,
before you set up anything of your own:

```zuri,ignore
class Circle < Shape {
  @new(radius) {
    parent('circle')
    self.radius = radius
  }
}
```

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

```zuri,ignore
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

`instance_of()` walks the whole chain, and `typeof()` reports the concrete
class:

```zuri
class Shape {}
class Circle < Shape {}
class Square < Shape {}

var c = Circle()

echo instance_of(c, Circle)
echo instance_of(c, Shape)
echo instance_of(c, Square)
echo typeof(c)
```

```console
true
true
false
Circle
```

`typeof()` on an instance gives the name of its concrete class, not the
name of any base class it inherits from. Use `instance_of()` when the
question is "does this behave like a Shape?", and `typeof()` when the
question is "what exactly is this?".
