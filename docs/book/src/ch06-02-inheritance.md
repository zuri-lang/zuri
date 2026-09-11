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

```zuri
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
