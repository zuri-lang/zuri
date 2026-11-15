# Functions

A function is a named piece of behavior with a list of parameters and a
result. You've been calling functions since Chapter 1. This chapter is about
writing your own.

In Zuri, functions are **values**. You can put one in a list, pass it to
another function, return it from a function, store it in a dictionary, and
call whatever you get back:

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

That's what makes `map`, `filter`, `reduce`, the callbacks throughout the
standard library and the route handlers in Chapter 15 possible.

Look closely at those two calls. `operations.twice` *reads* the function
from the dictionary, and then you call the value you read. You can't write
`operations.twice(5)` in one step, because a dot followed by a call looks
for a **method** of the dictionary, and dictionaries don't have a method
called `twice`. Either read the function into a variable first, as above, or
use brackets and call the result: `operations['twice'](5)`.

This chapter covers how to declare functions, the different ways to write
anonymous functions, how closures capture the variables around them, and the
optional type annotations that ask the runtime to check arguments for you.
