# Functions

A function is a piece of behaviour with a name, a list of parameters, and a
result. You have been calling them since Chapter 1; this chapter is about
writing them.

Functions in Zuri are **values**. You can put one in a list, hand it to
another function, return one from a function, store one in a dictionary,
and call whatever comes back:

```zuri
def double(n) {
  return n * 2
}

var operations = { twice: double, thrice: @(n) => n * 3 }

var chosen = operations.twice

echo chosen(5)
echo operations['thrice'](5)
echo [1, 2, 3].map(double)
```

```console
10
15
[2, 4, 6]
```

That one property is what makes `map`, `filter`, `reduce`, every callback
in the standard library, and every route handler in Chapter 20 possible.

Note the shape of those two calls. `operations.twice` *reads* the function
out of the dictionary, and then you call what you read. Writing
`operations.twice(5)` in one step does not work: a dot followed by a call
looks for a **method** on the dictionary, and a dictionary has no method
called `twice`. Either read it into a variable first, as above, or index
with brackets and call the result: `operations['twice'](5)`.

This chapter covers declaring a function, the spellings an anonymous one
can take, how a closure captures the variables around it, and the optional
type annotations that make the runtime check arguments for you.
