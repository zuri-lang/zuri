# Classes and Objects

A dictionary holds data. A class holds data *together with* the behavior
that goes with it, under a name that you can check for and inherit from.

```zuri
class Temperature {

  @new(celsius) {
    self.celsius = celsius
  }

  fahrenheit() {
    return self.celsius * 9 / 5 + 32
  }

  to_string() {
    return '${self.celsius}C'
  }
}

var t = Temperature(100)

echo t.fahrenheit()
echo t.to_string()
```

```console
212
100C
```

Three facts about Zuri classes come up throughout this chapter, so it's
worth knowing them from the start.

**The constructor is called `@new`.** Methods whose names begin with `@` are
called **decorated methods**, and each one connects the class to part of the
language's own syntax, such as construction, arithmetic, comparison,
iteration or JSON encoding. There's a fixed set of them, and [Decorated
Methods](ch06-03-decorated-methods.md) covers them all.

**Fields are accessed through `self`.** Inside a method, `self` refers to
the instance. A bare `balance` doesn't mean `self.balance`, and you don't
declare `self` as a parameter either.

**Classes are sealed.** A class's fields and methods are fixed when the
class is declared. Nothing at run time can add a new field to an instance or
a new method to a class, and trying to do so raises an error instead of
silently creating it. That rules out a whole category of bugs, since a
misspelled field name is an error rather than a new field. It also rules out
monkey patching, a technique that some languages depend on but Zuri doesn't
support.

Zuri has single inheritance: a class can have at most one parent, which you
write with `<`. There are no interfaces, mixins or `abstract` keyword. The
following sections show what to use instead.

Sealed doesn't mean a class can never change, though. A separate declaration
written with `>` instead of `<`, called a **class extension**, can add
methods to an existing class, even one you didn't write. [Class
Extensions](ch06-06-class-extensions.md) explains how.
