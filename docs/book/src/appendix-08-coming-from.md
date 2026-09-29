# Appendix H: Coming From Another Language

The places Zuri will surprise you are the places it looks most familiar.
This is that list.

## Everyone

| You expect | Zuri does |
| --- | --- |
| `x++` as a statement only | `x++` is postfix only, and in an expression it evaluates to the **new** value. `++x` does not parse. |
| `-1` to be truthy | Every negative number is falsy, alongside `0` and `''`. `var n = maybe or fallback` silently replaces `-1`. |
| `[]` to be falsy | `[]` and `{}` are truthy. Use `is_empty()`. |
| `NaN` to be falsy | `NaN` is truthy. Use `is_nan()`. |
| `finally` | There is none. Code after the `catch` statement runs either way. |
| `try` | The keyword is `catch`, and it takes the block that might fail: `catch { ... } as e { ... }`. |
| `new Thing()` | Call the class: `Thing()`. |
| `switch`/`case` with fall-through | `using`/`when`, first match only, no `break`. |
| a `main` function | A file's top level is the program. |
| declaration hoisting | None. A `def` must appear above the top-level line that calls it. |
| overloading by arity | None. Two `def`s of one name in one scope is a compile error. |
| reopening a class | `class Ext > Target` adds methods to an existing class, globally. See [Extensions](ch06-06-class-extensions.md). |
| string ordering with `<` | `<` is numbers only. Use `compare()`, which returns `-1`, `0` or `1`. |
| `x in collection` | No membership operator. Use `contains()`. |
| `?.` and `??` | Neither exists. `or` covers the common case, with the truthiness caveat above. |
| an `eval()` | There is none, deliberately. See [Metaprogramming](ch21-00-metaprogramming.md). |

## From Python

- Blocks are braces, not indentation, and every control-flow body takes one.
- `def` declares functions and `class` declares classes, but there is no
  `self` parameter: `self` is implicit inside a method and required for
  every field access.
- The constructor is `@new`, not `__init__`. Dunder methods are `@`-prefixed
  decorated methods: `@add`, `@lt`, `@key`, `@to_json`.
- There is no `__eq__`. `==` compares identity for instances and cannot be
  overridden. Write `equals()`.
- No list comprehensions. `map()`, `filter()` and `reduce()` are methods on
  the list.
- `len(x)` is `x.length()`. `str(x)` is `x.to_string()`. `int(x)` is
  `x.int()` or `x.to_number()`.
- Slicing is `s[a, b]`, with a comma, not `s[a:b]`.
- `elif` is `else if`.
- Modules run once and are cached, as in Python. Circular imports behave
  the same way, and have the same caveat.
- `if __name__ == '__main__'` is `if __root__ == __file__`.

## From JavaScript

- `var` is block-scoped and behaves like `let`. `const` prevents rebinding
  and is enforced in local scopes.
- `==` does no coercion. `'1' == 1` is `false`. There is no `===`.
- Arrow functions are `@(x) => x * 2` or `def(x) => x * 2`. The `@` is the
  common spelling.
- There is no `this` rebinding to worry about. `self` is the instance,
  always.
- Objects and dictionaries are the same thing, and a class is not one.
  Classes are sealed: you cannot add a property to an instance, though a
  `class Ext > Target` declaration can add a *method* to the class.
- `null` and `undefined` are both `nil`.
- No `async`/`await` and no event loop. Concurrency is isolates: real
  threads with separate heaps, communicating by copying.
- `JSON.stringify` is `json.encode`, and `compact` defaults to `true`.
- Template literals are `'${expr}'`, in ordinary single or double quotes.

## From Ruby

- No implicit returns. A function without `return` yields `nil`.
- No blocks or `yield`. Pass an anonymous function.
- `nil` is the only nil-like value, and `false` is separate from it.
- Methods do not end in `?` or `!`. Predicates are named `is_*` and
  mutation is documented rather than punctuated.
- `each` hands the callback **value first, index second**. `for` hands you
  **key first, value second**.
- There is no `method_missing`. Adding methods to an existing class is
  possible, but through an explicit `class Ext > Target` declaration rather
  than by reopening the class.
- Modules are files, not a language construct. There is no `include` or
  `extend`.

## From Go

- Dynamically typed, with optional annotations on parameters that are
  checked at every call.
- Errors are raised and caught, not returned. There is no `err !=  nil`
  pattern.
- Isolates are not goroutines. They are OS threads with **separate heaps**,
  and values crossing between them are copied. That is the whole
  concurrency model, and it is why there are no mutexes.
- Channels are bounded queues and behave the way you expect, `select()`
  included.
- No interfaces. A parameter typed `Error` accepts any subclass, and that
  is the whole of the polymorphism story alongside inheritance.
- `defer` has no equivalent. Close what you opened, on both paths.

## From Java or C#

- No static typing, no generics, no interfaces, no packages-as-namespaces.
  A module is a file.
- Single inheritance, and no `abstract` keyword. A base method that raises
  `NotImplementedError` is the idiom.
- `public`/`private` is a leading underscore, enforced at compile time for
  both class members and module members.
- There is no overloading. One name, one method, and a second
  declaration of either is a compile error.
- `toString()` is `to_string()`, and `echo` does **not** call it.

## From C

- Numbers are doubles. There is no integer type, and `/` never truncates.
  `//` is floor division.
- `%` keeps the sign of the left operand, as in C. `//` rounds toward
  negative infinity, which C's `/` does not.
- No pointers, no manual memory management. A generational collector owns
  the heap.
- `bytes` is the buffer type, and `struct` is how you read and write binary
  layouts.
- `switch` is `using`, with no fall-through.

## Things That Will Save You an Hour

**`echo` does not call `to_string()`.** An instance prints as
`<instance of Thing>`. Call the method.

**`list.sort()` mutates and returns; `list.reverse()` does neither to the
original.** That asymmetry is the most common list bug in Zuri code.

**A `def` scopes like a `var`.** At the top level of a file it binds a
module-level name; anywhere else it is a local of the block it is written
in, and it goes away with that block.

**`import` is local by default.** If your module imports something and a
third file cannot see it through you, add the `@`.

**A same-directory import is `import .sibling`**, not the full path from
the project root.

**A conditional expression breaks across lines either way.** `?` and `:`
may each end a line or begin the next, so `cond ?` on one line and a
line *starting* with `?` or `:` both parse.
