# Functions

Functions in Zuri are values. You can put one in a list, hand it to another
function, return it from a function, and store it in a dictionary. That is
what makes `map`, `filter` and every callback API in the standard library
work.

This chapter covers declaring them, the three ways to write an anonymous
one, how closures capture their surroundings, and the optional type
annotations that let the runtime check your arguments for you.
