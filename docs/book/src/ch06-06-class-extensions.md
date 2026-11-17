# Class Extensions

An **extension** adds methods to an existing class. It's written with `>`
instead of `<`, and it's different from inheritance. Inheritance creates a
*new* class based on an existing one, while an extension changes the
existing class directly.

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

`Account` now has two more methods. No new class, subclass or wrapper object
was created. `a` is still the same `Account` instance, and it now responds
to `shout()` and `initials()` as well as the methods declared in the class
itself.

## `<` Compared With `>`

They're easy to tell apart if you think of the arrow as pointing to where
the methods end up.

| | Inheritance | Extension |
| --- | --- | --- |
| Syntax | `class Child < Parent` | `class Anything > Target` |
| Creates | A new class | Nothing; it modifies `Target` |
| Affects existing instances | No | **Yes** |
| Can declare fields | Yes | No |
| Can declare `@new` | Yes | No, because it doesn't hold fields |
| Methods are | Ordinary methods | `static`, with the instance as a parameter |
| `self` inside a method | The instance | Not available |

## Rules for Extensions

The compiler enforces four rules, and its error messages tell you exactly
what's wrong.

### The Name Is Only a Label

The name of an extension is never bound to anything. It's there so the
declaration reads like any other declaration, and so error messages have
something to refer to:

```zuri,ignore
class AccountExtras > Account {
  static shout(account) {
    return 'hi'
  }
}

echo typeof(AccountExtras)
```

```console
Unhandled UndefinedError: undefined global 'AccountExtras'
```

Name an extension after what it adds, such as `AccountExtras`,
`ShapeFormatting` or `NodeDebugging`. The name isn't used anywhere else in
your code.

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

Here, `static` **doesn't** mean that the method is attached to the class
instead of its instances. It means that the method is compiled without an
implicit receiver, so `self` doesn't exist inside it. The instance is passed
as an ordinary argument instead.

### Extensions Can't Add Fields

```zuri,ignore
class Target {}

class Bad > Target {
  var cache = {}
}
```

```console
SyntaxError: extension 'Bad' can only declare static methods but not fields
```

An extension adds behavior, never state. A class's fields are fixed when the
class is declared, and nothing can add one later, not even an extension. If
your extension needs to store something, keep it in the module that declares
the extension, keyed by something that identifies the instance.

### The Target Must Already Exist

The target is evaluated when the extension is declared, so it has to be a
name that's already bound to a class:

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

The name doesn't have to be a plain identifier. Any expression that starts
with an identifier works, so you can name a class from another module
directly through that module:

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

Built-in types aren't classes you can refer to, so you can't extend
`string`, `list`, `number` or the other built-in types. You can extend
`Error` and any class that the standard library exports.

## The Instance Parameter

An extension method receives the instance as its **first parameter**, and
you can call that parameter whatever you like:

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

`r.scaled(3)` passes `r` as `reading` and `3` as `factor`. All the arguments
at the call site come after the instance, as they would for an ordinary
method.

Since argument counts aren't checked, the instance parameter is optional. A
method that doesn't need the instance can simply leave it out:

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

### There's No `self`

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

Use the instance parameter instead: write `t.n`, not `self.n`.

### Private Members Are Still Private

An extension counts as code outside the class, so the underscore rule
applies to it in full:

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

This is the most important limitation of extensions. They can add behavior
built from a class's **public** members, but they can't reach inside it. You
can't use an extension to get around encapsulation.

## Replacing an Existing Method

If an extension method has the same name as an existing method of the class,
it **replaces** that method:

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

Read that example carefully. `g` was created **before** the extension was
declared, and calling `greet()` on it afterwards runs the new version. The
new method doesn't shadow or wrap the old one. The class's method table
itself has changed, and every instance of the class sees the change,
whenever it was created.

This is true however much the original method has already been used. Here,
we build an eight-level tree, call the original `count()` four thousand
times, and then replace it:

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

After four thousand calls to the old method, the next call runs the new one.
Replacement always takes effect, and there's no warm-up state, cached lookup
or earlier result that can keep the old method alive after the extension
runs.

## Decorated Methods

An extension can add decorated methods, which is how you add operators,
iteration or a string form to a class you didn't write. The instance still
comes first, followed by the decorated method's own parameters:

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

`@add(a, b)` receives the left operand as `a` and the right operand as `b`.
`@key(box, previous)` receives the instance and then the previous key, which
is the argument `@key` would normally take on its own.

## Extension Methods Are Instance Methods

`static` in the declaration describes how the method is *compiled*, not
where it ends up. The methods are added to instances:

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

Call the method on an instance, as in `Item().label()`.

## Extensions and Inheritance

There's one interaction between the two that you need to know about, and it
depends on **when** each declaration runs.

An extension changes the target class's method table, and a subclass copies
its parent's methods when the subclass is declared. So a subclass only gets
an extension's methods if the extension was declared first:

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

`Car` was declared after the extension, so it inherited `doubled`. If you
move the `class Car` declaration above the extension, `Car().doubled()`
raises `undefined property 'doubled' on instance of 'Car'`, while `Vehicle`
still has the method.

**Declare extensions before any subclasses that should inherit them.** In
practice, that means at the top of a module, or in a module that's imported
at the top.

Extending a subclass never affects its parent:

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

## Extensions Apply Everywhere

This is what makes extensions powerful, and it's also why you should use
them sparingly. An extension isn't limited to the module that declares it.
It changes the class throughout the whole program, starting as soon as the
declaring module runs. For an imported module, that's as soon as it's
imported.

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

`main.zu` never refers to the definition of `label`. Importing `extras`, for
whatever reason, even one that has nothing to do with `Shape`, added a
method to a class declared in a third file, and the instance created before
the import got the method too.

This has two consequences.

**An import can change behavior you didn't ask it to change.** If a module
extends a class you use, the change affects your code as well, and nothing
at your call site tells you.

**Two extensions that define the same method conflict without any warning.**
The last one to run wins, so which one wins depends on the order of the
imports:

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

## When to Use an Extension

Extensions solve a problem that inheritance can't: adding behavior to a
class whose instances are created somewhere you don't control. A subclass
only helps if you're the one calling the constructor.

Good reasons to use one:

**Adding a view or format to a domain class.** For example, a `to_string()`,
a `@to_json` or a `summary()` method. This is presentation code that doesn't
belong in the model, added by the module that needs it.

**Adding an operator or iteration to a standard library class**, so that it
works with syntax it wasn't originally written for.

**Instrumenting code while debugging.** You can replace a method with one
that logs its calls, and then delete the extension when you're done, without
touching the original file.

Reasons to think twice:

**It's invisible at the call site.** Nothing about `account.shout()` tells
you that `shout` is defined in a different file from `Account`. Someone
searching the class body won't find it.

**It applies everywhere**, as described above.

**An ordinary function is usually enough.** `shout(account)` is a plain
function, it's obvious where it's defined, and it can't conflict with
anything. Use an extension when you really need a *method*, either because
the syntax requires one, as with decorated methods, or because callers only
have the instance and nothing else.
