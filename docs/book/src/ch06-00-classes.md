# Classes and Objects

A dictionary holds data. A class holds data *and* the behaviour that goes
with it, under a name you can check for and inherit from.

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

Three facts about Zuri classes shape everything in this chapter, and it is
worth having them up front.

**The constructor is `@new`.** Methods whose names start with `@` are
**decorated methods**, and each one connects the class to a piece of the
language's own syntax: construction, arithmetic, comparison, iteration,
JSON encoding. There is a fixed set of them, and
[Decorated Methods](ch06-03-decorated-methods.md) covers all of it.

**Fields are reached through `self`.** Inside a method, `self` is the
instance. There is no bare `balance` that means `self.balance`, and no
`self` parameter in the declaration either.

**A class is sealed.** The set of fields and methods is fixed when the
class is declared. Nothing at runtime can add a new field to an instance or
a new method to a class, and an attempt raises an error rather than quietly
creating one. That rules out a family of bugs — a typo in a field name is
an error, not a new field — and it rules out monkey-patching, which is a
technique some languages rely on and Zuri does not offer.

Inheritance is single: a class has at most one parent, written with `<`.
There are no interfaces, no mixins and no abstract keyword. The sections
that follow show what you write instead.

Sealed does not mean unchangeable forever, though. A separate declaration
written with `>` instead of `<` — a **class extension** — can add methods
to a class after the fact, including one you did not write.
[Class Extensions](ch06-06-class-extensions.md) covers it.
