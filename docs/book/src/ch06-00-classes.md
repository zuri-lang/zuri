# Classes and Objects

A dictionary is data. A class is data with behaviour attached and a fixed
shape.

Zuri's classes are deliberately small. There is single inheritance and no
interfaces, no mixins, no abstract keyword, no getters and setters. What
there is instead is a set of **decorated methods**, named `@new`, `@add`,
`@key` and so on, which let a class participate in the language's own
syntax: construction, arithmetic, comparison and iteration.

The other thing to know up front is that a class is **sealed** once it is
declared. The set of fields and methods is fixed at declaration time, and
nothing at runtime can add to it. That is what lets the runtime give every
instance a flat, slot-indexed layout, and it is why field access in Zuri is
an array index rather than a hash lookup.
