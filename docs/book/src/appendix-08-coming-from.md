# Appendix H: Coming From Another Language

The places where Zuri is most likely to surprise you are the places where it
looks most familiar. This appendix lists them.

## For Everyone

| You might expect | What Zuri does |
| --- | --- |
| `x++` only as a statement | `x++` is postfix only, and inside an expression it evaluates to the **new** value. `++x` doesn't parse. |
| `[]` to be falsy | `[]` and `{}` are truthy. Use `is_empty()`. |
| `finally` | There isn't one. Code after the `catch` statement runs either way. |
| `try` | The keyword is `catch`, and it takes the block that might fail: `catch { ... } as e { ... }`. |
| `new Thing()` | Call the class instead: `Thing()`. |
| `switch` and `case` with fall-through | `using` and `when`, which only run the first match, with no `break` |
| A `main` function | The top level of a file is the program. |
| Declarations to be hoisted | They aren't. A `def` must come before the top-level line that calls it. |
| Overloading by number of arguments | There's no overloading. Two `def` declarations with the same name in the same scope are a compile error. |
| Reopening a class | `class Ext > Target` adds methods to an existing class, everywhere. See [Extensions](ch06-06-class-extensions.md). |
| Comparing strings with `<` | `<` only works with numbers. Use `compare()`, which returns `-1`, `0` or `1`. |
| `x in collection` | There's no membership operator. Use `contains()`. |
| `?.` and `??` | Neither exists. `or` covers the most common case, with the truthiness caveat above. |
| `eval()` | There isn't one, on purpose. See [Metaprogramming](ch21-00-metaprogramming.md). |

## Coming From Python

- Blocks use braces, not indentation, and every control flow statement needs
  them.
- `def` declares functions and `class` declares classes, but there's no
  `self` parameter. `self` is available automatically inside a method, and
  you have to use it for every field access.
- The constructor is `@new`, not `__init__`. Python's dunder methods are
  decorated methods that start with `@`, such as `@add`, `@lt`, `@key` and
  `@to_json`.
- `__eq__` is `@eq`, and `!=` is always its opposite. It only runs when the
  other side is also an object, so `x == nil` never calls it. Without it,
  instances are compared by identity.
- There are no list comprehensions. `map()`, `filter()` and `reduce()` are
  methods of lists.
- `len(x)` is `x.length()`, `str(x)` is `x.to_string()`, and `int(x)` is
  `x.int()` or `x.to_number()`.
- Slices are written `s[a, b]`, with a comma, not `s[a:b]`.
- `elif` is `else if`.
- As in Python, modules run once and are cached. Circular imports work in
  the same way, with the same caveat.
- `if __name__ == '__main__'` is written `if __root__ == __file__`.

## Coming From JavaScript

- `var` is block-scoped and behaves like `let`. `const` prevents a name from
  being reassigned, and it's enforced in local scopes.
- `==` never converts types, so `'1' == 1` is `false`. There's no `===`.
- Arrow functions are written `@(x) => x * 2` or `def(x) => x * 2`, and `@`
  is the more common form.
- You don't have to worry about `this` being rebound, because `self` is
  always the instance.
- Objects and dictionaries are the same thing, but a class instance isn't a
  dictionary. Classes are sealed, so you can't add a property to an
  instance, although a `class Ext > Target` declaration can add a *method*
  to the class.
- `null` and `undefined` are both `nil`.
- There's no `async`/`await` and no event loop. Concurrency uses isolates,
  which are real threads with separate heaps that communicate by copying
  values.
- `JSON.stringify` is `json.encode`, and `compact` defaults to `true`.
- Template literals are written `'${expr}'`, in ordinary single or double
  quotes.

## Coming From Ruby

- There are no implicit returns. A function without a `return` returns
  `nil`.
- There are no blocks or `yield`. Pass an anonymous function instead.
- `nil` is the only nil-like value, and `false` is separate from it.
- Method names don't end in `?` or `!`. Predicates are named `is_*`, and
  whether a method changes its receiver is documented rather than indicated
  by punctuation.
- `each` passes the **value first and the index second** to its callback,
  while `for` gives you the **key first and the value second**.
- There's no `method_missing`. You can add methods to an existing class, but
  only through an explicit `class Ext > Target` declaration, not by
  reopening the class.
- Modules are files, not a language construct, and there's no `include` or
  `extend`.

## Coming From Go

- Zuri is dynamically typed, with optional annotations on parameters that
  are checked on every call.
- Errors are raised and caught, not returned, so there's no `err != nil`
  pattern.
- Isolates aren't goroutines. They're operating system threads with
  **separate heaps**, and values passed between them are copied. That's the
  whole concurrency model, and it's why there are no mutexes.
- Channels are bounded queues and behave the way you'd expect, including
  `select()`.
- There are no interfaces. A parameter annotated as `Error` accepts any
  subclass of `Error`, and that, together with inheritance, is all there is
  to polymorphism.
- There's no equivalent of `defer`. Close what you opened on both the
  success path and the error path.

## Coming From Java or C#

- There's no static typing, no generics, no interfaces, and no packages as
  namespaces. A module is a file.
- There's only single inheritance, and no `abstract` keyword. The usual
  approach is a base method that raises `NotImplementedError`.
- Instead of `public` and `private`, a leading underscore marks something as
  private, and this is enforced at compile time for both class members and
  module members.
- There's no overloading. Each name refers to one method, and declaring it a
  second time is a compile error.
- `equals()` is `@eq`, and `==` calls it.
- `toString()` is `to_string()`, and nothing calls it for you. What `echo`
  shows for an instance comes from `@to_string()`.

## Coming From C

- Numbers are doubles. There's no integer type, and `/` never truncates.
  `//` is floor division.
- As in C, the result of `%` has the sign of the left operand. `//` rounds
  toward negative infinity, which C's `/` doesn't.
- There are no pointers and no manual memory management. A generational
  garbage collector manages the heap.
- `bytes` is the buffer type, and `struct` is how you read and write binary
  layouts.
- `switch` is `using`, and there's no fall-through.

## Things That Can Save You an Hour

**Interpolation doesn't call `to_string()`.** `'${thing}'` gives you
`<instance of Thing>`, so call the method yourself. `echo` displays an
instance using `@to_string()`, which is a separate method.

**`list.sort()` changes the list and returns it, while `list.reverse()`
returns a new list and leaves the original unchanged.** This difference is
the most common cause of list bugs in Zuri code.

**A `def` is scoped like a `var`.** At the top level of a file, it creates a
module-level name. Anywhere else, it creates a local in the block where it's
written, and that local disappears when the block ends.

**Imports are private by default.** If your module imports something and a
third file can't see it through your module, add the `@`.

**To import a module in the same directory, write `import .sibling`**, not
the full path from the project root.

**A conditional expression can be split across lines either way.** `?` and
`:` can each end a line or start the next one, so both `cond ?` at the end
of a line and a line *starting* with `?` or `:` parse correctly.
