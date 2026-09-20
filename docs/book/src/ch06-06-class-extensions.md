# Class Extensions

An **extension** adds methods to a class that already exists. It is written
with `>` rather than `<`, and it is a different thing from inheritance:
inheritance creates a *new* class that borrows from an old one, while an
extension modifies the old one in place.

```zuri
class Account {

  @new(owner) {
    self.owner = owner
  }

  describe() {
    return 'account for ${self.owner}'
  }
}

class AccountExtras > Account {

  static shout(account) {
    return account.describe().upper()
  }

  static initials(account) {
    return account.owner[0]
  }
}

var a = Account('ada')

echo a.shout()
echo a.initials()
echo a.describe()
```

```console
ACCOUNT FOR ADA
a
account for ada
```

`Account` gained two methods. There is no new class, no subclass, and no
wrapper object: `a` is the same `Account` instance it was, and it now
answers to `shout()` and `initials()` alongside the methods its own
declaration gave it.

## `<` Versus `>`

The two are easy to tell apart once you read the arrow as pointing at
where the methods end up.

| | Inheritance | Extension |
| --- | --- | --- |
| Syntax | `class Child < Parent` | `class Anything > Target` |
| Produces | a new class | nothing; it modifies `Target` |
| Affects existing instances | no | **yes** |
| May declare fields | yes | no |
| May declare `@new` | yes | no, it is not a field-holder |
| Methods are | ordinary methods | `static`, with the receiver as a parameter |
| `self` inside a method | the instance | not available |

## The Rules

Four rules are enforced at compile time, and the error messages say
exactly what is wrong.

### The Name Is a Label

An extension's own name is never bound to anything. It exists so the
declaration reads like a declaration and so the error messages have
something to say:

```zuri,ignore
class AccountExtras > Account {
  static shout(account) { return 'hi' }
}

echo typeof(AccountExtras)
```

```console
Unhandled UndefinedError: undefined global 'AccountExtras'
```

Name it after what it adds. `AccountExtras`, `ShapeFormatting`,
`NodeDebugging` all read well; the name appears in no other code.

### Every Method Must Be `static`

```zuri,ignore
class Target {}

class Bad > Target {
  helper() {
    return 1
  }
}
```

```console
SyntaxError: extension method 'helper' must be declared 'static' and receive the instance explicitly as their own first parameter if desired
```

`static` here does **not** mean the method ends up on the class rather than
on instances. It means the method is compiled without an implicit receiver,
so `self` does not exist inside it. The instance arrives as an ordinary
argument instead.

### No Fields

```zuri,ignore
class Target {}

class Bad > Target {
  var cache = {}
}
```

```console
SyntaxError: extension 'Bad' can only declare static methods but not fields
```

An extension adds behaviour, never state. A class's set of fields is fixed
when the class is declared, and nothing — including an extension — can add
one afterwards. When your extension needs somewhere to keep something, keep
it in the extending module, keyed by whatever identifies the instance.

### The Target Must Exist

The target is evaluated when the extension is declared, so it has to be a
name that is already bound to a class:

```zuri,ignore
class Orphan > NoSuchClass {
  static m(x) {
    return 1
  }
}
```

```console
Unhandled UndefinedError: undefined global 'NoSuchClass'
```

The name does not have to be a bare one. Anything an expression starting
with an identifier reaches works, so a class another module owns can be
named straight through that module:

```zuri
import set

class SetExtras > set.Set {
  static summary(s) {
    return 'set of ${s.length()}'
  }
}

echo set.Set([1, 2, 3]).summary()
```

```console
set of 3
```

Built-in types are not classes in scope, so `string`, `list`, `number` and
the rest cannot be extended. `Error` and every class the standard library
exports can be.

## The Receiver

An extension method receives the instance as its **first parameter**. The
name is yours to choose:

```zuri
class Reading {

  @new(celsius) {
    self.celsius = celsius
  }
}

class ReadingConversion > Reading {

  static fahrenheit(reading) {
    return reading.celsius * 9 / 5 + 32
  }

  static scaled(reading, factor) {
    return reading.celsius * factor
  }
}

var r = Reading(100)

echo r.fahrenheit()
echo r.scaled(3)
```

```console
212
300
```

`r.scaled(3)` passes `r` as `reading` and `3` as `factor`. Every argument
at the call site shifts one place to the right of the receiver, exactly as
it would for an ordinary method.

Because arity is not enforced, the receiver parameter is optional. A method
that does not need the instance can simply not declare one:

```zuri
class Widget {

  @new() {}
}

class WidgetVersion > Widget {

  static api_version() {
    return '1.0'
  }
}

echo Widget().api_version()
```

```console
1.0
```

### `self` Does Not Exist Here

```zuri,ignore
class Thing {

  @new(n) {
    self.n = n
  }
}

class ThingBroken > Thing {

  static value(t) {
    return self.n
  }
}
```

```console
SyntaxError: 'self' used outside of a method
```

Use the receiver parameter. `t.n`, not `self.n`.

### Private Members Stay Private

An extension is outside code, and the underscore rule applies to it in
full:

```zuri,ignore
class Safe {
  var _hidden = 'secret'

  @new() {}
}

class Peek > Safe {

  static reveal(s) {
    return s._hidden
  }
}
```

```console
SyntaxError: '_hidden' is private and can only be accessed via 'self' or 'parent'
```

This is the important limit on extensions. They can add behaviour built out
of a class's **public** surface, and they cannot reach inside it. An
extension is not a way around encapsulation.

## Replacing an Existing Method

An extension method with the same name as one the class already has
**replaces** it:

```zuri
class Greeter {

  @new(name) {
    self.name = name
  }

  greet() {
    return 'hello ${self.name}'
  }
}

var g = Greeter('ada')

echo g.greet()

class GreeterLoud > Greeter {

  static greet(greeter) {
    return 'HELLO ${greeter.name.upper()}'
  }
}

echo g.greet()
```

```console
hello ada
HELLO ADA
```

Read that carefully. `g` was constructed **before** the extension was
declared, and calling `greet()` on it after the declaration runs the new
one. The replacement is not a shadow or a wrapper: the method table of the
class itself changed, and every instance of it — past, present and future —
sees the change.

This holds no matter how thoroughly the original has already been used.
Build an eight-level tree, call the original `count()` four thousand times,
then replace it:

```zuri,ignore
var t = tree_with(8)
var total = 0

iter var i = 0; i < 4000; i++ {
  total = total + t.count()
}

echo total

class TreeNodeExt > TreeNode {

  static count(node) {
    return 999
  }
}

echo t.count()
```

```console
2044000
999
```

Four thousand calls to the old method, and the next call runs the new one.
Replacement is unconditional: there is no warm-up state, cached lookup or
earlier result that can keep an old body alive past the extension.

## Decorated Methods

An extension can add decorated methods, which is how you give operators,
iteration or a string form to a class you did not write. The receiver still
comes first, and the decorator's own parameters follow it:

```zuri
class Box {

  @new(n) {
    self.n = n
  }
}

class BoxOps > Box {

  static @add(a, b) {
    return Box(a.n + b.n)
  }

  static @key(box, previous) {
    return previous == nil ? 0 : nil
  }

  static @value(box, key) {
    return box.n
  }

  static to_string(box) {
    return 'Box(${box.n})'
  }
}

echo (Box(2) + Box(3)).n
echo Box(9).to_string()

for value in Box(7) {
  echo value
}
```

```console
5
Box(9)
7
```

`@add(a, b)` gets the left operand as `a` and the right as `b`.
`@key(box, previous)` gets the instance and then the previous key, which is
the argument `@key` would normally take on its own.

## They Are Instance Methods, Not Statics

`static` in the declaration describes how the method is *compiled*, not
where it lands. The methods go onto instances:

```zuri,ignore
class Item {

  @new() {}
}

class ItemExtras > Item {

  static label(item) {
    return 'an item'
  }
}

echo Item.label(Item())
```

```console
Unhandled PropertyError: undefined static member 'label' on class 'Item'
```

Call it on the instance: `Item().label()`.

## Extensions and Inheritance

The two interact in one way worth knowing, and the rule is about **when**
each declaration runs.

An extension changes the target's method table. A subclass copies its
parent's methods when the subclass is declared. So a subclass sees an
extension only if the extension came first:

```zuri
class Vehicle {

  @new(wheels) {
    self.wheels = wheels
  }
}

class VehicleExtras > Vehicle {

  static doubled(vehicle) {
    return vehicle.wheels * 2
  }
}

class Car < Vehicle {

  @new() {
    parent(4)
  }
}

echo Vehicle(2).doubled()
echo Car().doubled()
```

```console
4
8
```

`Car` was declared after the extension, so it inherited `doubled`. Move the
`class Car` declaration above the extension and `Car().doubled()` raises
`undefined property 'doubled' on instance of 'Car'`, while `Vehicle` keeps
it.

**Declare extensions before the subclasses that should inherit them.** In
practice that means at the top of a module, or in a module imported at the
top.

Extending a subclass never touches its parent:

```zuri
class Animal {

  @new() {}
}

class Dog < Animal {

  @new() {}
}

class DogExtras > Dog {

  static speak(dog) {
    return 'woof'
  }
}

echo Dog().speak()

catch {
  echo Animal().speak()
} as e {
  echo e.message
}
```

```console
woof
undefined property 'speak' on instance of 'Animal'
```

## Extensions Are Global

This is the property that makes extensions powerful and the one that makes
them worth using sparingly. An extension is not scoped to the module that
declares it. It changes the class everywhere in the program, and it takes
effect the moment the declaring module runs — which, for an imported
module, is the moment it is imported.

<span class="filename">Filename: shape.zu</span>

```zuri,ignore
class Shape {

  @new(name) {
    self.name = name
  }
}
```

<span class="filename">Filename: extras.zu</span>

```zuri,ignore
import .shape { Shape }

class ShapeExtras > Shape {

  static label(s) {
    return 'shape: ${s.name}'
  }
}
```

<span class="filename">Filename: main.zu</span>

```zuri,ignore
import .shape { Shape }

var s = Shape('circle')

catch {
  echo s.label()
} as e {
  echo 'before import: ${e.message}'
}

import .extras

echo s.label()
echo Shape('square').label()
```

```console
before import: undefined property 'label' on instance of 'Shape'
shape: circle
shape: square
```

`main.zu` never mentions `label`'s definition. Importing `extras` — for any
reason, including a reason unrelated to `Shape` — added a method to a class
declared in a third file, and the instance created before the import
gained it too.

Two consequences follow.

**An import can change behaviour you did not ask it to change.** A module
that extends a class you use will alter it for your code as well, and
nothing at your call site says so.

**Two extensions of the same method collide silently.** The last one to run
wins, and the winner depends on import order:

```zuri
class Slot {

  @new() {}
}

class First > Slot {

  static which(s) {
    return 'first'
  }
}

class Second > Slot {

  static which(s) {
    return 'second'
  }
}

echo Slot().which()
```

```console
second
```

## When to Use One

Extensions answer a question inheritance cannot: how do you add behaviour
to a class whose instances are created somewhere you do not control? A
subclass only helps if you are the one calling the constructor.

Good reasons:

**Adding a view or a format to a domain class.** `to_string()`, a
`@to_json`, a `summary()` — presentation that does not belong in the model
itself, added from the module that cares about it.

**Giving a standard library class an operator or an iterator** so it works
with syntax it was not written for.

**Instrumenting during debugging.** Replacing a method with one that logs,
then deleting the extension, is a diagnostic technique that costs nothing
in the original file.

Reasons to think twice:

**It is invisible at the call site.** `account.shout()` gives no hint that
`shout` lives in a different file from `Account`. A reader who greps the
class body will not find it.

**It is global.** Everything above.

**A plain function is usually enough.** `shout(account)` is a function, it
is obvious where it lives, and it cannot collide with anything. Reach for
an extension when the thing genuinely needs to be a *method* — because
syntax demands it, as with a decorated method, or because callers already
have the instance and nothing else.
